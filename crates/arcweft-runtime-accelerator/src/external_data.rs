use std::{cell::RefCell, collections::BTreeMap};

use apache_avro::{Reader, Schema, Writer, types::Value as AvroValue};
use arcweft_core::{
    entry::{RuntimeNominalRecordShape, RuntimeSchemaLimits},
    pattern::{
        RuntimeBuiltinVariantCaseIdentity, RuntimeCheckedType, RuntimeSemanticTypeId,
        RuntimeVariantIdentity,
    },
    program_types::{
        RuntimeDataFieldDefaultRequest, RuntimeProgramDataShapes, RuntimeProgramTypes,
    },
    task::RuntimeProgramOwner,
    value::{
        RuntimeAgentValue, RuntimeDataShape, RuntimeInt, RuntimeSeq, RuntimeUInt, RuntimeValue,
        runtime_sequence_dense_bytes,
    },
};
use arcweft_data::{
    Bytes, Codec, DataError, DataErrorKind, DataFormat, DecodeOptions, DecodeShapeAccess,
    EncodeOptions, FieldDefaultProvider, FieldDefaultRequest, Number, PathSegment, ShapeAccess,
    ShapeId, ShapeRef, TypeShape, Value,
};

use super::{
    RuntimeEvalError, RuntimeExternalCallContext, RuntimePureAccelerator, data_runtime_error,
    runtime_value_label_for_data,
};

struct DataCall<'a> {
    owner: &'a RuntimeProgramOwner,
    types: RuntimeProgramTypes<'a>,
    argument_types: &'a [RuntimeSemanticTypeId],
    result_type: RuntimeSemanticTypeId,
    limits: RuntimeSchemaLimits,
}

pub(super) fn call_data_external(
    accelerator: &mut RuntimePureAccelerator,
    context: &RuntimeExternalCallContext,
    label: &str,
    args: &[RuntimeValue],
) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
    if !matches!(label, "data.encode" | "data.decode" | "data.shape") {
        return None;
    }
    let call = match validate_data_call_context(context, label, args) {
        Ok(call) => call,
        Err(error) => return Some(Err(error)),
    };
    if matches!(label, "data.encode" | "data.decode") && args.len() == 3 {
        let value_type = if label == "data.encode" {
            call.argument_types[0]
        } else {
            match result_value_and_error_types(&call, label) {
                Ok((value_type, _)) => value_type,
                Err(error) => return Some(Err(error)),
            }
        };
        if let Err(error) = validate_shape_value(&call, label, 2, value_type, &args[2]) {
            return Some(Err(error));
        }
    }
    Some(match label {
        "data.shape" => runtime_data_shape(&call, args.first()),
        "data.encode" | "data.decode" => {
            let value_type = match result_value_and_error_types(&call, label) {
                Ok((value_type, _)) if label == "data.decode" => value_type,
                Ok(_) => call.argument_types[0],
                Err(error) => return Some(Err(error)),
            };
            let shapes = RuntimeProgramDataShapes::new(call.types);
            let root = match shapes.root(value_type, call.limits) {
                Ok(root) => root,
                Err(error) => {
                    return Some(wrap_data_result(
                        &call,
                        label,
                        Err(DataError::unsupported(error.to_string())),
                    ));
                }
            };
            let result = if label == "data.encode" {
                encode_call(accelerator, &call, args, value_type, root, &shapes)
            } else {
                decode_call(accelerator, &call, args, value_type, root, &shapes)
            };
            match result {
                Ok(value) => wrap_data_result(&call, label, Ok(value)),
                Err(error) => wrap_data_result(&call, label, Err(error)),
            }
        }
        _ => unreachable!("data external label was filtered above"),
    })
}

fn validate_data_call_context<'a>(
    context: &'a RuntimeExternalCallContext,
    label: &str,
    args: &[RuntimeValue],
) -> Result<DataCall<'a>, RuntimeEvalError> {
    let expected_arity = match (label, args.len()) {
        ("data.shape", 0 | 1) | ("data.encode" | "data.decode", 2 | 3) => args.len(),
        _ => return Err(data_runtime_error(label, "invalid data call arity")),
    };
    let owner = context.program_owner().ok_or_else(|| {
        data_runtime_error(
            label,
            "requires a selected program-bound external call context",
        )
    })?;
    let argument_types = context.argument_types().ok_or_else(|| {
        data_runtime_error(label, "selected call has no argument type identities")
    })?;
    if argument_types.len() != expected_arity {
        return Err(data_runtime_error(
            label,
            "runtime argument arity differs from selected call context",
        ));
    }
    let result_type = context
        .result_type()
        .ok_or_else(|| data_runtime_error(label, "selected call has no result type identity"))?;
    let limits = context
        .limits()
        .ok_or_else(|| data_runtime_error(label, "selected call has no runtime schema limits"))?;
    let types = owner.types();
    for (semantic_type, value) in argument_types.iter().copied().zip(args) {
        types
            .validate_live_value(semantic_type, value, limits)
            .map_err(|error| data_runtime_error(label, error.to_string()))?;
    }

    if label == "data.shape" {
        let shape = RuntimeDataShape::bind(owner.clone(), result_type)
            .map_err(|error| data_runtime_error(label, error.to_string()))?;
        if let ([value_type], [_]) = (argument_types, args)
            && *value_type != shape.value_type()
        {
            return Err(data_runtime_error(
                label,
                "value argument differs from the selected DataShape child",
            ));
        }
    } else {
        let Some((ok_type, error_type)) = types
            .result_types(result_type)
            .map_err(|error| data_runtime_error(label, error.to_string()))?
        else {
            return Err(data_runtime_error(
                label,
                "selected data codec callable must return Result<T, DataError>",
            ));
        };
        require_nominal_type(&types, error_type, "DataError", label)?;
        let format_type = argument_types[1];
        require_data_format_type(&types, format_type, label)?;
        if label == "data.encode" {
            if !matches!(
                types
                    .checked_type(ok_type)
                    .map_err(|error| data_runtime_error(label, error.to_string()))?,
                RuntimeCheckedType::Bytes
            ) {
                return Err(data_runtime_error(
                    label,
                    "selected data.encode result must be Result<Bytes, DataError>",
                ));
            }
            if let [value_type, _, shape_type] = argument_types {
                validate_shape_child(owner, label, *shape_type, *value_type)?;
            }
        } else {
            if args.len() == 2 {
                require_nominal_variant_type(&types, ok_type, "DataValue", label)?;
            } else {
                validate_shape_child(owner, label, argument_types[2], ok_type)?;
            }
            if !matches!(
                types
                    .checked_type(argument_types[0])
                    .map_err(|error| data_runtime_error(label, error.to_string()))?,
                RuntimeCheckedType::Bytes
            ) {
                return Err(data_runtime_error(
                    label,
                    "selected data.decode input must be Bytes",
                ));
            }
        }
    }
    Ok(DataCall {
        owner,
        types,
        argument_types,
        result_type,
        limits,
    })
}

fn validate_shape_child(
    owner: &RuntimeProgramOwner,
    label: &str,
    shape_type: RuntimeSemanticTypeId,
    value_type: RuntimeSemanticTypeId,
) -> Result<(), RuntimeEvalError> {
    let shape = RuntimeDataShape::bind(owner.clone(), shape_type)
        .map_err(|error| data_runtime_error(label, error.to_string()))?;
    shape
        .validate_for(owner, shape_type, value_type)
        .map_err(|error| data_runtime_error(label, error.to_string()))
}

fn require_nominal_type(
    types: &RuntimeProgramTypes<'_>,
    semantic_type: RuntimeSemanticTypeId,
    expected: &str,
    label: &str,
) -> Result<(), RuntimeEvalError> {
    let checked = types
        .checked_type(semantic_type)
        .map_err(|error| data_runtime_error(label, error.to_string()))?;
    let is_expected = match checked {
        RuntimeCheckedType::Nominal { nominal, .. } => nominal.as_str() == expected,
        RuntimeCheckedType::Variant {
            owner: RuntimeVariantIdentity::Nominal { nominal, .. },
            ..
        } => nominal.as_str() == expected,
        _ => false,
    };
    if !is_expected {
        return Err(data_runtime_error(
            label,
            format!("selected type must be nominal {expected}"),
        ));
    }
    Ok(())
}

fn require_nominal_variant_type(
    types: &RuntimeProgramTypes<'_>,
    semantic_type: RuntimeSemanticTypeId,
    expected: &str,
    label: &str,
) -> Result<(), RuntimeEvalError> {
    require_nominal_type(types, semantic_type, expected, label)?;
    if !matches!(
        types
            .checked_type(semantic_type)
            .map_err(|error| data_runtime_error(label, error.to_string()))?,
        RuntimeCheckedType::Variant { .. }
    ) {
        return Err(data_runtime_error(
            label,
            format!("selected type {expected} must be a closed enum"),
        ));
    }
    Ok(())
}

fn require_data_format_type(
    types: &RuntimeProgramTypes<'_>,
    semantic_type: RuntimeSemanticTypeId,
    label: &str,
) -> Result<(), RuntimeEvalError> {
    require_nominal_variant_type(types, semantic_type, "DataFormat", label)
}

fn result_value_and_error_types(
    call: &DataCall<'_>,
    label: &str,
) -> Result<(RuntimeSemanticTypeId, RuntimeSemanticTypeId), RuntimeEvalError> {
    call.types
        .result_types(call.result_type)
        .map_err(|error| data_runtime_error(label, error.to_string()))?
        .ok_or_else(|| data_runtime_error(label, "selected result type is not Result<T, E>"))
}

fn runtime_data_shape(
    call: &DataCall<'_>,
    value: Option<&RuntimeValue>,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let shape = RuntimeDataShape::bind(call.owner.clone(), call.result_type)
        .map_err(|error| data_runtime_error("data.shape", error.to_string()))?;
    match (value, call.argument_types) {
        (None, []) => {}
        (Some(value), [argument_type]) if *argument_type == shape.value_type() => {
            shape
                .validate_value(value, call.limits)
                .map_err(|error| data_runtime_error("data.shape", error.to_string()))?;
        }
        (Some(_), [argument_type]) => {
            return Err(data_runtime_error(
                "data.shape",
                format!(
                    "argument type {argument_type:?} differs from the selected DataShape child {:?}",
                    shape.value_type()
                ),
            ));
        }
        _ => {
            return Err(data_runtime_error(
                "data.shape",
                "runtime argument arity differs from selected call context",
            ));
        }
    }
    Ok(RuntimeValue::Agent(RuntimeAgentValue::DataShape(shape)))
}

fn encode_call(
    _accelerator: &mut RuntimePureAccelerator,
    call: &DataCall<'_>,
    args: &[RuntimeValue],
    _value_type: RuntimeSemanticTypeId,
    root: ShapeRef<'_>,
    shapes: &RuntimeProgramDataShapes<'_>,
) -> arcweft_data::Result<Value> {
    let format = data_format_arg(call, "data.encode", &args[1])
        .map_err(|error| data_error_from_runtime(error))?;
    let value = runtime_value_to_data_value(&args[0], root, shapes, call.types, call.limits)?;
    let bytes = encode_value(format, &value, root, shapes)?;
    Ok(Value::Bytes(Bytes::new(bytes)))
}

fn decode_call(
    accelerator: &mut RuntimePureAccelerator,
    call: &DataCall<'_>,
    args: &[RuntimeValue],
    _value_type: RuntimeSemanticTypeId,
    root: ShapeRef<'_>,
    shapes: &RuntimeProgramDataShapes<'_>,
) -> arcweft_data::Result<Value> {
    let format = data_format_arg(call, "data.decode", &args[1]).map_err(data_error_from_runtime)?;
    let bytes = runtime_value_to_bytes(&args[0])?;
    let defaults = RuntimeDataDefaultProvider {
        accelerator: RefCell::new(accelerator),
        owner: call.owner,
        shapes,
        limits: call.limits,
    };
    let access = DecodeShapeAccess::new(shapes, &defaults);
    decode_value(format, &bytes, root, &access)
}

fn validate_shape_value(
    call: &DataCall<'_>,
    label: &str,
    argument: usize,
    value_type: RuntimeSemanticTypeId,
    value: &RuntimeValue,
) -> Result<(), RuntimeEvalError> {
    let shape_type = call.argument_types[argument];
    let RuntimeValue::Agent(RuntimeAgentValue::DataShape(witness)) = value else {
        return Err(data_runtime_error(
            label,
            "shape argument must be DataShape<T>",
        ));
    };
    witness
        .validate_for(call.owner, shape_type, value_type)
        .map_err(|error| data_runtime_error(label, error.to_string()))
}

fn data_format_arg(
    call: &DataCall<'_>,
    label: &str,
    value: &RuntimeValue,
) -> Result<DataFormat, RuntimeEvalError> {
    let semantic_type = call.argument_types[1];
    let RuntimeCheckedType::Variant {
        owner: expected_owner,
        cases,
        ..
    } = call
        .types
        .checked_type(semantic_type)
        .map_err(|error| data_runtime_error(label, error.to_string()))?
    else {
        return Err(data_runtime_error(
            label,
            "format type must be nominal DataFormat",
        ));
    };
    let RuntimeVariantIdentity::Nominal { nominal, .. } = &expected_owner else {
        return Err(data_runtime_error(
            label,
            "format type must be nominal DataFormat",
        ));
    };
    if nominal.as_str() != "DataFormat" {
        return Err(data_runtime_error(
            label,
            "format type must be nominal DataFormat",
        ));
    }
    let RuntimeValue::Variant {
        owner,
        ordinal,
        name,
        payload: None,
    } = value
    else {
        return Err(data_runtime_error(
            label,
            format!(
                "format must be a DataFormat enum value, found {}",
                runtime_value_label_for_data(value)
            ),
        ));
    };
    if owner != &expected_owner {
        return Err(data_runtime_error(
            label,
            "format value has the wrong DataFormat owner",
        ));
    }
    let case = usize::try_from(*ordinal)
        .ok()
        .and_then(|ordinal| cases.get(ordinal))
        .filter(|case| case.name == *name && case.payload.is_none())
        .ok_or_else(|| {
            data_runtime_error(
                label,
                format!("unknown DataFormat case #{ordinal} `{name}`"),
            )
        })?;
    DataFormat::from_variant_name(&case.name).ok_or_else(|| {
        data_runtime_error(
            label,
            format!("selected DataFormat case `{}` is unsupported", case.name),
        )
    })
}

fn runtime_value_to_bytes(value: &RuntimeValue) -> arcweft_data::Result<Vec<u8>> {
    match value {
        RuntimeValue::Seq(RuntimeSeq::Dense(arcweft_core::value::DenseSeq::Bytes(values))) => {
            Ok(values.as_slice().to_vec())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(arcweft_core::value::DenseSeq::U8(values))) => {
            Ok(values.as_slice().to_vec())
        }
        RuntimeValue::Seq(sequence) => sequence
            .clone()
            .into_values()
            .into_iter()
            .map(|value| match value {
                RuntimeValue::UInt(value) => value
                    .try_into_u32()
                    .and_then(|value| u8::try_from(value).ok()),
                RuntimeValue::Int(value) => value
                    .try_into_i32()
                    .and_then(|value| u8::try_from(value).ok()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| DataError::invalid_type("Bytes", "non-byte sequence")),
        other => Err(DataError::invalid_type(
            "Bytes",
            runtime_value_label_for_data(other),
        )),
    }
}

fn data_error_from_runtime(error: RuntimeEvalError) -> DataError {
    DataError::new(DataErrorKind::Custom, error.to_string())
}

fn wrap_data_result(
    call: &DataCall<'_>,
    label: &str,
    result: arcweft_data::Result<Value>,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let (value_type, error_type) = result_value_and_error_types(call, label)?;
    let shapes = RuntimeProgramDataShapes::new(call.types);
    let (ordinal, payload) = match result {
        Ok(value) => {
            let root = shapes
                .root(value_type, call.limits)
                .map_err(|error| data_runtime_error(label, error.to_string()))?;
            (
                0,
                data_value_to_runtime_value(value, root, &shapes, call.types, call.limits)
                    .map_err(|error| data_runtime_error(label, error.to_string()))?,
            )
        }
        Err(error) => {
            let root = shapes
                .root(error_type, call.limits)
                .map_err(|error| data_runtime_error(label, error.to_string()))?;
            let value = data_error_value(&error, root, &shapes)
                .and_then(|value| {
                    data_value_to_runtime_value(value, root, &shapes, call.types, call.limits)
                })
                .map_err(|error| data_runtime_error(label, error.to_string()))?;
            (1, value)
        }
    };
    call.types
        .try_variant_value(
            call.result_type,
            ordinal,
            Some(RuntimeValue::Tuple(vec![payload])),
            call.limits,
        )
        .map_err(|error| data_runtime_error(label, error.to_string()))
}

fn data_error_value(
    error: &DataError,
    shape_ref: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> arcweft_data::Result<Value> {
    let shape = shape_ref.resolve(access)?;
    let TypeShape::Record { fields, .. } = shape.as_ref() else {
        return Err(DataError::unsupported(
            "selected DataError must have a record data codec shape",
        ));
    };
    let mut values = BTreeMap::new();
    for field in fields {
        let value = match field.rust_name.as_str() {
            "kind" => data_enum_case_value(
                data_error_kind_name(error.kind()),
                None,
                ShapeRef::Inline(&field.shape),
                access,
            )?,
            "path" => data_path_value(
                error.path().segments(),
                ShapeRef::Inline(&field.shape),
                access,
            )?,
            "message" => Value::String(error.message().to_owned()),
            other => {
                return Err(DataError::unsupported(format!(
                    "selected DataError contains unsupported field `{other}`"
                )));
            }
        };
        values.insert(field.wire_name.clone(), value);
    }
    Ok(Value::Record(values))
}

fn data_error_kind_name(kind: &DataErrorKind) -> &'static str {
    match kind {
        DataErrorKind::MissingField => "MissingField",
        DataErrorKind::UnknownField => "UnknownField",
        DataErrorKind::DuplicateField => "DuplicateField",
        DataErrorKind::InvalidType => "InvalidType",
        DataErrorKind::InvalidEnumTag => "InvalidEnumTag",
        DataErrorKind::NumberOutOfRange => "NumberOutOfRange",
        DataErrorKind::InvalidEncoding => "InvalidEncoding",
        DataErrorKind::TrailingData => "TrailingData",
        DataErrorKind::LimitExceeded => "LimitExceeded",
        DataErrorKind::UnsupportedFormat => "UnsupportedFormat",
        DataErrorKind::Io => "Io",
        DataErrorKind::Custom => "Custom",
    }
}

fn data_path_value(
    segments: &[PathSegment],
    shape_ref: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> arcweft_data::Result<Value> {
    let shape = shape_ref.resolve(access)?;
    let TypeShape::Record { fields, .. } = shape.as_ref() else {
        return Err(DataError::unsupported(
            "selected DataPath must have a record data codec shape",
        ));
    };
    let mut values = BTreeMap::new();
    for field in fields {
        if field.rust_name != "segments" {
            return Err(DataError::unsupported(format!(
                "selected DataPath contains unsupported field `{}`",
                field.rust_name
            )));
        }
        let segment_shape = field.resolve_value_shape(access)?;
        let TypeShape::Seq(item_shape) = segment_shape.as_ref() else {
            return Err(DataError::unsupported(
                "selected DataPath.segments must be a sequence",
            ));
        };
        let segment_values = segments
            .iter()
            .map(|segment| {
                let (name, payload) = match segment {
                    PathSegment::Field(name) => (
                        "Field",
                        Some(Value::Tuple(vec![Value::String(name.clone())])),
                    ),
                    PathSegment::Index(index) => (
                        "Index",
                        Some(Value::Tuple(vec![Value::Number(Number::U(*index as u128))])),
                    ),
                    PathSegment::Variant(name) => (
                        "Variant",
                        Some(Value::Tuple(vec![Value::String(name.clone())])),
                    ),
                };
                data_enum_case_value(name, payload, ShapeRef::Inline(item_shape), access)
            })
            .collect::<arcweft_data::Result<Vec<_>>>()?;
        values.insert(field.wire_name.clone(), Value::Seq(segment_values));
    }
    Ok(Value::Record(values))
}

fn data_enum_case_value(
    name: &str,
    payload: Option<Value>,
    shape_ref: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> arcweft_data::Result<Value> {
    let shape = shape_ref.resolve(access)?;
    let TypeShape::Enum {
        variants,
        name: enum_name,
        ..
    } = shape.as_ref()
    else {
        return Err(DataError::unsupported(format!(
            "selected `{name}` value does not have an enum codec shape"
        )));
    };
    let case = variants
        .iter()
        .find(|case| case.rust_name == name || case.wire_name == name)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::InvalidEnumTag,
                format!("selected enum `{enum_name}` has no case `{name}`"),
            )
        })?;
    match (case.payload.as_ref(), payload) {
        (None, None) => Ok(Value::Enum {
            variant: case.wire_name.clone(),
            payload: None,
        }),
        (Some(_), Some(value)) => Ok(Value::Enum {
            variant: case.wire_name.clone(),
            payload: Some(Box::new(value)),
        }),
        _ => Err(DataError::invalid_type(
            format!("payload matching `{}`", case.wire_name),
            "different payload presence",
        )),
    }
}

struct RuntimeDataDefaultProvider<'a, 'program> {
    accelerator: RefCell<&'a mut RuntimePureAccelerator>,
    owner: &'a RuntimeProgramOwner,
    shapes: &'a RuntimeProgramDataShapes<'program>,
    limits: RuntimeSchemaLimits,
}

impl FieldDefaultProvider for RuntimeDataDefaultProvider<'_, '_> {
    fn default_value(&self, request: FieldDefaultRequest<'_>) -> arcweft_data::Result<Value> {
        let record = request.record_id().ok_or_else(|| {
            DataError::new(
                DataErrorKind::MissingField,
                "runtime defaults require an exact selected record occurrence",
            )
        })?;
        let default = self
            .shapes
            .field_default_request(record, request.field_ordinal())
            .map_err(|error| DataError::new(DataErrorKind::MissingField, error.to_string()))?
            .ok_or_else(|| {
                DataError::new(
                    DataErrorKind::MissingField,
                    "codec requested a default for a field without an admitted producer",
                )
            })?;
        let mut accelerator = self.accelerator.try_borrow_mut().map_err(|_| {
            DataError::new(
                DataErrorKind::Custom,
                "runtime default producer reentered the active codec backend",
            )
        })?;
        let value = evaluate_default_program(&mut accelerator, self.owner, &default)
            .map_err(|error| DataError::new(DataErrorKind::Custom, error.to_string()))?;
        default
            .validate_result(&value, self.limits)
            .map_err(|error| DataError::new(DataErrorKind::InvalidType, error.to_string()))?;
        runtime_value_to_data_value(
            &value,
            ShapeRef::Id(default.result_shape()),
            self.shapes,
            default.program_types(),
            self.limits,
        )
    }
}

fn evaluate_default_program(
    accelerator: &mut RuntimePureAccelerator,
    owner: &RuntimeProgramOwner,
    request: &RuntimeDataFieldDefaultRequest<'_>,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match owner {
        RuntimeProgramOwner::Plan(plan) => arcweft_core::pure::evaluate_pure_program_with_backend(
            plan,
            request.program(),
            &[],
            accelerator,
        ),
        RuntimeProgramOwner::Awbc(program_owner) => {
            arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                program_owner,
                request.program(),
                &[],
                accelerator,
            )
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: "data field default".to_owned(),
                reason: error.to_string(),
            })
        }
    }
}

fn runtime_value_to_data_value(
    value: &RuntimeValue,
    shape_ref: ShapeRef<'_>,
    shapes: &RuntimeProgramDataShapes<'_>,
    types: RuntimeProgramTypes<'_>,
    limits: RuntimeSchemaLimits,
) -> arcweft_data::Result<Value> {
    if let Some((_, child, wrapper)) = transparent_codec_child(shape_ref, shapes, types)? {
        let inner = match (wrapper, value) {
            (TransparentValueWrapper::NominalRecord, RuntimeValue::NominalRecord(record)) => {
                let [inner] = record.fields() else {
                    return Err(DataError::invalid_type(
                        "single-field nominal wrapper",
                        format!("nominal record with {} fields", record.fields().len()),
                    ));
                };
                inner
            }
            (TransparentValueWrapper::Tuple, RuntimeValue::Tuple(values)) if values.len() == 1 => {
                &values[0]
            }
            (TransparentValueWrapper::NominalRecord, other) => {
                return Err(DataError::invalid_type(
                    "nominal transparent wrapper",
                    runtime_value_label_for_data(other),
                ));
            }
            (TransparentValueWrapper::Tuple, other) => {
                return Err(DataError::invalid_type(
                    "one-item tuple transparent wrapper",
                    runtime_value_label_for_data(other),
                ));
            }
        };
        return runtime_value_to_data_value(inner, ShapeRef::Id(child), shapes, types, limits);
    }
    let shape = shape_ref.resolve(shapes)?;
    let semantic_type = shape_semantic_type(shape_ref, shapes);
    match (value, shape.as_ref()) {
        (RuntimeValue::Unit, TypeShape::Unit) => Ok(Value::Unit),
        (RuntimeValue::NominalRecord(record), TypeShape::Unit)
            if nominal_shape(semantic_type, types)? == Some(RuntimeNominalRecordShape::Unit)
                && record.fields().is_empty() =>
        {
            Ok(Value::Unit)
        }
        (RuntimeValue::Bool(value), TypeShape::Bool) => Ok(Value::Bool(*value)),
        (RuntimeValue::Int(value), shape) if shape.signed_bounds().is_some() => {
            Ok(Value::Number(Number::I(runtime_int_to_i128(*value))))
        }
        (RuntimeValue::UInt(value), shape) if shape.unsigned_max().is_some() => {
            Ok(Value::Number(Number::U(runtime_uint_to_u128(*value))))
        }
        (RuntimeValue::F32(value), TypeShape::F32) => Ok(Value::Number(Number::F32(*value))),
        (RuntimeValue::F64(value), TypeShape::F64) => Ok(Value::Number(Number::F64(*value))),
        (RuntimeValue::String(value), TypeShape::String) => Ok(Value::String(value.clone())),
        (RuntimeValue::Char(value), TypeShape::Char) => Ok(Value::Char(*value)),
        (RuntimeValue::Seq(_), TypeShape::Bytes { .. }) => {
            runtime_value_to_bytes(value).map(|bytes| Value::Bytes(Bytes::new(bytes)))
        }
        (_, TypeShape::Option(inner)) => {
            let Some((case, payload)) = value.builtin_variant_case() else {
                return Err(DataError::invalid_type(
                    "builtin Option",
                    runtime_value_label_for_data(value),
                ));
            };
            match (case, payload) {
                (RuntimeBuiltinVariantCaseIdentity::OptionNone, None) => Ok(Value::Option(None)),
                (RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(payload)) => {
                    runtime_value_to_data_value(
                        payload,
                        ShapeRef::Inline(inner),
                        shapes,
                        types,
                        limits,
                    )
                    .map(Box::new)
                    .map(Some)
                    .map(Value::Option)
                }
                _ => Err(DataError::invalid_type(
                    "canonical Option case",
                    runtime_value_label_for_data(value),
                )),
            }
        }
        (RuntimeValue::Seq(values), TypeShape::Seq(item_shape)) => values
            .clone()
            .into_values()
            .iter()
            .enumerate()
            .map(|(index, value)| {
                runtime_value_to_data_value(
                    value,
                    ShapeRef::Inline(item_shape),
                    shapes,
                    types,
                    limits,
                )
                .map_err(|error| error.at_index(index))
            })
            .collect::<arcweft_data::Result<Vec<_>>>()
            .map(Value::Seq),
        (RuntimeValue::Tuple(values), TypeShape::Tuple(items)) => {
            runtime_tuple_to_data(values, items, shapes, types, limits)
        }
        (RuntimeValue::NominalRecord(record), TypeShape::Tuple(items))
            if nominal_shape(semantic_type, types)? == Some(RuntimeNominalRecordShape::Tuple) =>
        {
            runtime_tuple_to_data(record.fields(), items, shapes, types, limits)
        }
        (RuntimeValue::Seq(entries), TypeShape::Map { key, value, kind }) => {
            let mut converted = Vec::with_capacity(entries.len());
            for (index, entry) in entries.clone().into_values().into_iter().enumerate() {
                let RuntimeValue::Tuple(pair) = entry else {
                    return Err(DataError::invalid_type(
                        "map entry tuple",
                        runtime_value_label_for_data(&entry),
                    )
                    .at_index(index));
                };
                let [runtime_key, runtime_value] = pair.as_slice() else {
                    return Err(DataError::invalid_type(
                        "two-item map entry",
                        format!("tuple with {} items", pair.len()),
                    )
                    .at_index(index));
                };
                let key = runtime_value_to_data_value(
                    runtime_key,
                    ShapeRef::Inline(key),
                    shapes,
                    types,
                    limits,
                )
                .map_err(|error| error.at_index(index))?;
                let value = runtime_value_to_data_value(
                    runtime_value,
                    ShapeRef::Inline(value),
                    shapes,
                    types,
                    limits,
                )
                .map_err(|error| error.at_index(index))?;
                converted.push((key, value));
            }
            Ok(Value::map(*kind, converted))
        }
        (
            RuntimeValue::Record(_) | RuntimeValue::NominalRecord(_),
            TypeShape::Record { fields, .. },
        ) => {
            let runtime_fields = runtime_record_values(value)?;
            if runtime_fields.len() != fields.len() {
                return Err(DataError::invalid_type(
                    format!("record with {} fields", fields.len()),
                    format!("record with {} fields", runtime_fields.len()),
                ));
            }
            let mut converted = BTreeMap::new();
            for (field, runtime_field) in fields.iter().zip(runtime_fields) {
                let field_shape = field.resolve_value_shape(shapes)?;
                let child_ref = if field.bytes_format.is_some()
                    && matches!(field_shape.as_ref(), TypeShape::Bytes { .. })
                {
                    ShapeRef::Inline(field_shape.as_ref())
                } else {
                    ShapeRef::Inline(&field.shape)
                };
                let value =
                    runtime_value_to_data_value(runtime_field, child_ref, shapes, types, limits)
                        .map_err(|error| error.at_field(field.wire_name.clone()))?;
                converted.insert(field.wire_name.clone(), value);
            }
            Ok(Value::Record(converted))
        }
        (
            RuntimeValue::Variant {
                ordinal,
                name,
                payload,
                ..
            },
            TypeShape::Enum {
                name: enum_name,
                variants,
                ..
            },
        ) => {
            let case = usize::try_from(*ordinal)
                .ok()
                .and_then(|ordinal| variants.get(ordinal))
                .filter(|case| case.rust_name == *name)
                .ok_or_else(|| {
                    DataError::new(
                        DataErrorKind::InvalidEnumTag,
                        format!("runtime case `{name}` is absent from selected enum `{enum_name}`"),
                    )
                })?;
            let value = match (payload.as_deref(), case.payload.as_ref()) {
                (None, None) => None,
                (Some(payload), Some(payload_shape)) => Some(Box::new(
                    runtime_value_to_data_value(
                        payload,
                        ShapeRef::Inline(payload_shape),
                        shapes,
                        types,
                        limits,
                    )
                    .map_err(|error| error.at_variant(case.wire_name.clone()))?,
                )),
                _ => {
                    return Err(DataError::invalid_type(
                        format!("payload shape for enum case `{}`", case.wire_name),
                        "different payload presence",
                    ));
                }
            };
            Ok(Value::Enum {
                variant: case.wire_name.clone(),
                payload: value,
            })
        }
        (_, shape) => Err(DataError::invalid_type(
            shape.type_name(),
            runtime_value_label_for_data(value),
        )),
    }
}

fn runtime_tuple_to_data(
    values: &[RuntimeValue],
    items: &[TypeShape],
    shapes: &RuntimeProgramDataShapes<'_>,
    types: RuntimeProgramTypes<'_>,
    limits: RuntimeSchemaLimits,
) -> arcweft_data::Result<Value> {
    if values.len() != items.len() {
        return Err(DataError::invalid_type(
            format!("tuple with {} items", items.len()),
            format!("tuple with {} items", values.len()),
        ));
    }
    values
        .iter()
        .zip(items)
        .enumerate()
        .map(|(index, (value, item))| {
            runtime_value_to_data_value(value, ShapeRef::Inline(item), shapes, types, limits)
                .map_err(|error| error.at_index(index))
        })
        .collect::<arcweft_data::Result<Vec<_>>>()
        .map(Value::Tuple)
}

fn runtime_record_values(value: &RuntimeValue) -> arcweft_data::Result<Vec<&RuntimeValue>> {
    match value {
        RuntimeValue::Record(fields) => Ok(fields
            .fields()
            .iter()
            .map(arcweft_core::value::RuntimeFieldValue::value)
            .collect()),
        RuntimeValue::NominalRecord(record) => Ok(record.fields().iter().collect()),
        other => Err(DataError::invalid_type(
            "record value",
            runtime_value_label_for_data(other),
        )),
    }
}

fn data_value_to_runtime_value(
    value: Value,
    shape_ref: ShapeRef<'_>,
    shapes: &RuntimeProgramDataShapes<'_>,
    types: RuntimeProgramTypes<'_>,
    limits: RuntimeSchemaLimits,
) -> arcweft_data::Result<RuntimeValue> {
    if let Some((semantic_type, child, wrapper)) =
        transparent_codec_child(shape_ref, shapes, types)?
    {
        let value = data_value_to_runtime_value(value, ShapeRef::Id(child), shapes, types, limits)?;
        return match wrapper {
            TransparentValueWrapper::NominalRecord => types
                .try_record_value(semantic_type, vec![value], limits)
                .map_err(|error| DataError::new(DataErrorKind::InvalidType, error.to_string())),
            TransparentValueWrapper::Tuple => {
                let tuple = RuntimeValue::Tuple(vec![value]);
                types
                    .validate_live_value(semantic_type, &tuple, limits)
                    .map_err(|error| {
                        DataError::new(DataErrorKind::InvalidType, error.to_string())
                    })?;
                Ok(tuple)
            }
        };
    }
    let shape = shape_ref.resolve(shapes)?;
    let semantic_type = shape_semantic_type(shape_ref, shapes);
    match (value, shape.as_ref()) {
        (Value::Unit, TypeShape::Unit) => match nominal_shape(semantic_type, types)? {
            Some(RuntimeNominalRecordShape::Unit) => types
                .try_record_value(require_semantic_type(semantic_type)?, Vec::new(), limits)
                .map_err(|error| DataError::new(DataErrorKind::InvalidType, error.to_string())),
            _ => Ok(RuntimeValue::Unit),
        },
        (Value::Bool(value), TypeShape::Bool) => Ok(RuntimeValue::Bool(value)),
        (Value::Number(Number::I(value)), TypeShape::I8) => i8::try_from(value)
            .map(RuntimeValue::i8)
            .map_err(|_| number_range_error("i8")),
        (Value::Number(Number::I(value)), TypeShape::I16) => i16::try_from(value)
            .map(RuntimeValue::i16)
            .map_err(|_| number_range_error("i16")),
        (Value::Number(Number::I(value)), TypeShape::I32) => i32::try_from(value)
            .map(RuntimeValue::i32)
            .map_err(|_| number_range_error("i32")),
        (Value::Number(Number::I(value)), TypeShape::I64) => i64::try_from(value)
            .map(RuntimeValue::i64)
            .map_err(|_| number_range_error("i64")),
        (Value::Number(Number::I(value)), TypeShape::I128) => Ok(RuntimeValue::i128(value)),
        (Value::Number(Number::I(value)), TypeShape::Isize) => i64::try_from(value)
            .map(RuntimeValue::isize)
            .map_err(|_| number_range_error("isize")),
        (Value::Number(Number::U(value)), TypeShape::U8) => u8::try_from(value)
            .map(RuntimeValue::u8)
            .map_err(|_| number_range_error("u8")),
        (Value::Number(Number::U(value)), TypeShape::U16) => u16::try_from(value)
            .map(RuntimeValue::u16)
            .map_err(|_| number_range_error("u16")),
        (Value::Number(Number::U(value)), TypeShape::U32) => u32::try_from(value)
            .map(RuntimeValue::u32)
            .map_err(|_| number_range_error("u32")),
        (Value::Number(Number::U(value)), TypeShape::U64) => u64::try_from(value)
            .map(RuntimeValue::u64)
            .map_err(|_| number_range_error("u64")),
        (Value::Number(Number::U(value)), TypeShape::U128) => Ok(RuntimeValue::u128(value)),
        (Value::Number(Number::U(value)), TypeShape::Usize) => u64::try_from(value)
            .map(RuntimeValue::usize)
            .map_err(|_| number_range_error("usize")),
        (Value::Number(Number::F32(value)), TypeShape::F32) => Ok(RuntimeValue::F32(value)),
        (Value::Number(Number::F64(value)), TypeShape::F64) => Ok(RuntimeValue::F64(value)),
        (Value::String(value), TypeShape::String) => Ok(RuntimeValue::String(value)),
        (Value::Char(value), TypeShape::Char) => Ok(RuntimeValue::Char(value)),
        (Value::Bytes(bytes), TypeShape::Bytes { .. }) => {
            Ok(runtime_sequence_dense_bytes(bytes.into_vec()))
        }
        (Value::Option(None), TypeShape::Option(_)) => Ok(RuntimeValue::option_none()),
        (Value::Option(Some(value)), TypeShape::Option(inner)) => {
            data_value_to_runtime_value(*value, ShapeRef::Inline(inner), shapes, types, limits)
                .map(RuntimeValue::option_some)
        }
        (Value::Seq(values), TypeShape::Seq(item_shape)) => values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                data_value_to_runtime_value(
                    value,
                    ShapeRef::Inline(item_shape),
                    shapes,
                    types,
                    limits,
                )
                .map_err(|error| error.at_index(index))
            })
            .collect::<arcweft_data::Result<Vec<_>>>()
            .map(|values| RuntimeValue::Seq(RuntimeSeq::Values(values))),
        (Value::Tuple(values), TypeShape::Tuple(items)) => {
            let values = data_tuple_to_runtime(values, items, shapes, types, limits)?;
            match nominal_shape(semantic_type, types)? {
                Some(RuntimeNominalRecordShape::Tuple) => types
                    .try_record_value(require_semantic_type(semantic_type)?, values, limits)
                    .map_err(|error| DataError::new(DataErrorKind::InvalidType, error.to_string())),
                _ => Ok(RuntimeValue::Tuple(values)),
            }
        }
        (
            Value::Map { kind, entries },
            TypeShape::Map {
                key,
                value,
                kind: expected_kind,
            },
        ) => {
            if kind != *expected_kind {
                return Err(DataError::invalid_type(
                    format!("{expected_kind:?} map"),
                    format!("{kind:?} map"),
                ));
            }
            let entries = entries
                .into_iter()
                .enumerate()
                .map(|(index, (map_key, map_value))| {
                    let key = data_value_to_runtime_value(
                        map_key,
                        ShapeRef::Inline(key),
                        shapes,
                        types,
                        limits,
                    )
                    .map_err(|error| error.at_index(index))?;
                    let value = data_value_to_runtime_value(
                        map_value,
                        ShapeRef::Inline(value),
                        shapes,
                        types,
                        limits,
                    )
                    .map_err(|error| error.at_index(index))?;
                    Ok(RuntimeValue::Tuple(vec![key, value]))
                })
                .collect::<arcweft_data::Result<Vec<_>>>()?;
            Ok(RuntimeValue::Seq(RuntimeSeq::Values(entries)))
        }
        (
            Value::Record(fields),
            TypeShape::Record {
                fields: shapes_fields,
                ..
            },
        ) => {
            let converted = shapes_fields
                .iter()
                .map(|field| {
                    let value = fields.get(&field.wire_name).cloned().ok_or_else(|| {
                        DataError::new(
                            DataErrorKind::MissingField,
                            format!("decoded record is missing `{}`", field.wire_name),
                        )
                        .at_field(field.wire_name.clone())
                    })?;
                    let field_shape = field.resolve_value_shape(shapes)?;
                    let child_ref = if field.bytes_format.is_some()
                        && matches!(field_shape.as_ref(), TypeShape::Bytes { .. })
                    {
                        ShapeRef::Inline(field_shape.as_ref())
                    } else {
                        ShapeRef::Inline(&field.shape)
                    };
                    data_value_to_runtime_value(value, child_ref, shapes, types, limits)
                        .map_err(|error| error.at_field(field.wire_name.clone()))
                })
                .collect::<arcweft_data::Result<Vec<_>>>()?;
            types
                .try_record_value(require_semantic_type(semantic_type)?, converted, limits)
                .map_err(|error| DataError::new(DataErrorKind::InvalidType, error.to_string()))
        }
        (Value::Enum { variant, payload }, TypeShape::Enum { name, variants, .. }) => {
            let (ordinal, case) = variants
                .iter()
                .enumerate()
                .find(|(_, case)| case.wire_name == variant)
                .ok_or_else(|| {
                    DataError::new(
                        DataErrorKind::InvalidEnumTag,
                        format!("decoded enum case `{variant}` is absent from `{name}`"),
                    )
                })?;
            let payload = match (payload, case.payload.as_ref()) {
                (None, None) => None,
                (Some(payload), Some(payload_shape)) => Some(
                    data_value_to_runtime_value(
                        *payload,
                        ShapeRef::Inline(payload_shape),
                        shapes,
                        types,
                        limits,
                    )
                    .map_err(|error| error.at_variant(case.wire_name.clone()))?,
                ),
                _ => {
                    return Err(DataError::invalid_type(
                        format!("payload for `{}`", case.wire_name),
                        "different payload presence",
                    ));
                }
            };
            types
                .try_variant_value(
                    require_semantic_type(semantic_type)?,
                    u32::try_from(ordinal).map_err(|_| {
                        DataError::new(DataErrorKind::NumberOutOfRange, "enum ordinal exceeds u32")
                    })?,
                    payload,
                    limits,
                )
                .map_err(|error| DataError::new(DataErrorKind::InvalidType, error.to_string()))
        }
        (value, shape) => Err(DataError::invalid_type(
            shape.type_name(),
            value.type_name(),
        )),
    }
}

fn data_tuple_to_runtime(
    values: Vec<Value>,
    items: &[TypeShape],
    shapes: &RuntimeProgramDataShapes<'_>,
    types: RuntimeProgramTypes<'_>,
    limits: RuntimeSchemaLimits,
) -> arcweft_data::Result<Vec<RuntimeValue>> {
    if values.len() != items.len() {
        return Err(DataError::invalid_type(
            format!("tuple with {} items", items.len()),
            format!("tuple with {} items", values.len()),
        ));
    }
    values
        .into_iter()
        .zip(items)
        .enumerate()
        .map(|(index, (value, item))| {
            data_value_to_runtime_value(value, ShapeRef::Inline(item), shapes, types, limits)
                .map_err(|error| error.at_index(index))
        })
        .collect()
}

fn shape_semantic_type(
    shape_ref: ShapeRef<'_>,
    shapes: &RuntimeProgramDataShapes<'_>,
) -> Option<RuntimeSemanticTypeId> {
    shape_ref
        .referenced_id()
        .and_then(|id| shapes.semantic_type(id))
}

fn require_semantic_type(
    semantic_type: Option<RuntimeSemanticTypeId>,
) -> arcweft_data::Result<RuntimeSemanticTypeId> {
    semantic_type.ok_or_else(|| {
        DataError::unsupported("selected shape has no original program semantic type coordinate")
    })
}

fn nominal_shape(
    semantic_type: Option<RuntimeSemanticTypeId>,
    types: RuntimeProgramTypes<'_>,
) -> arcweft_data::Result<Option<RuntimeNominalRecordShape>> {
    semantic_type
        .map(|semantic_type| {
            types
                .nominal_record_shape(semantic_type)
                .map_err(|error| DataError::unsupported(error.to_string()))
        })
        .transpose()
        .map(Option::flatten)
}

#[derive(Clone, Copy)]
enum TransparentValueWrapper {
    NominalRecord,
    Tuple,
}

fn transparent_codec_child(
    shape_ref: ShapeRef<'_>,
    shapes: &RuntimeProgramDataShapes<'_>,
    types: RuntimeProgramTypes<'_>,
) -> arcweft_data::Result<Option<(RuntimeSemanticTypeId, ShapeId, TransparentValueWrapper)>> {
    let Some(id) = shape_ref.referenced_id() else {
        return Ok(None);
    };
    let child = shapes
        .transparent_child(id)
        .map_err(|error| DataError::unsupported(error.to_string()))?;
    let Some(child) = child else {
        return Ok(None);
    };
    let Some(semantic_type) = shapes.semantic_type(id) else {
        return Err(DataError::unsupported(format!(
            "selected shape id {} has no program semantic type",
            id.index()
        )));
    };
    let wrapper = match nominal_shape(Some(semantic_type), types)? {
        Some(RuntimeNominalRecordShape::Newtype | RuntimeNominalRecordShape::Tuple) => {
            TransparentValueWrapper::NominalRecord
        }
        Some(RuntimeNominalRecordShape::Unit | RuntimeNominalRecordShape::Record) => {
            return Err(DataError::unsupported(
                "selected transparent codec occurrence is not a one-field source wrapper",
            ));
        }
        None => match types
            .checked_type(semantic_type)
            .map_err(|error| DataError::unsupported(error.to_string()))?
        {
            RuntimeCheckedType::Tuple(items) if items.len() == 1 => TransparentValueWrapper::Tuple,
            _ => {
                return Err(DataError::unsupported(
                    "selected transparent codec occurrence is neither a nominal wrapper nor a one-item tuple",
                ));
            }
        },
    };
    Ok(Some((semantic_type, child, wrapper)))
}

fn runtime_int_to_i128(value: RuntimeInt) -> i128 {
    match value {
        RuntimeInt::I8(value) => i128::from(value),
        RuntimeInt::I16(value) => i128::from(value),
        RuntimeInt::I32(value) => i128::from(value),
        RuntimeInt::I64(value) | RuntimeInt::ISize(value) => i128::from(value),
        RuntimeInt::I128(value) => value,
    }
}

fn runtime_uint_to_u128(value: RuntimeUInt) -> u128 {
    match value {
        RuntimeUInt::U8(value) => u128::from(value),
        RuntimeUInt::U16(value) => u128::from(value),
        RuntimeUInt::U32(value) => u128::from(value),
        RuntimeUInt::U64(value) | RuntimeUInt::USize(value) => u128::from(value),
        RuntimeUInt::U128(value) => value,
    }
}

fn number_range_error(expected: &'static str) -> DataError {
    DataError::new(
        DataErrorKind::NumberOutOfRange,
        format!("decoded integer is outside {expected} range"),
    )
}

fn encode_value(
    format: DataFormat,
    value: &Value,
    shape: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> arcweft_data::Result<Vec<u8>> {
    let options = EncodeOptions::default();
    match format {
        DataFormat::Json => {
            arcweft_codec_json::JsonCodec.encode_value(value, shape, access, &options)
        }
        DataFormat::Toml => {
            arcweft_codec_toml::TomlCodec.encode_value(value, shape, access, &options)
        }
        DataFormat::Yaml => {
            arcweft_codec_yaml::YamlCodec.encode_value(value, shape, access, &options)
        }
        DataFormat::MessagePack => {
            arcweft_codec_msgpack::MessagePackCodec.encode_value(value, shape, access, &options)
        }
        DataFormat::Cbor => {
            arcweft_codec_cbor::CborCodec.encode_value(value, shape, access, &options)
        }
        DataFormat::Avro => encode_avro_json_envelope(value, shape, access),
        DataFormat::Csv => arcweft_codec_csv::CsvCodec.encode_value(value, shape, access, &options),
        DataFormat::ArrowIpc => {
            arcweft_codec_arrow::ArrowIpcCodec.encode_value(value, shape, access, &options)
        }
        DataFormat::Parquet => {
            arcweft_codec_arrow::ParquetCodec.encode_value(value, shape, access, &options)
        }
        DataFormat::ArcweftBinary => {
            arcweft_codec_binary::ArcweftBinaryCodec.encode_value(value, shape, access, &options)
        }
    }
}

fn decode_value(
    format: DataFormat,
    input: &[u8],
    shape: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> arcweft_data::Result<Value> {
    let options = DecodeOptions::default();
    match format {
        DataFormat::Json => {
            arcweft_codec_json::JsonCodec.decode_value(input, shape, access, &options)
        }
        DataFormat::Toml => {
            arcweft_codec_toml::TomlCodec.decode_value(input, shape, access, &options)
        }
        DataFormat::Yaml => {
            arcweft_codec_yaml::YamlCodec.decode_value(input, shape, access, &options)
        }
        DataFormat::MessagePack => {
            arcweft_codec_msgpack::MessagePackCodec.decode_value(input, shape, access, &options)
        }
        DataFormat::Cbor => {
            arcweft_codec_cbor::CborCodec.decode_value(input, shape, access, &options)
        }
        DataFormat::Avro => decode_avro_json_envelope(input, shape, access),
        DataFormat::Csv => arcweft_codec_csv::CsvCodec.decode_value(input, shape, access, &options),
        DataFormat::ArrowIpc => {
            arcweft_codec_arrow::ArrowIpcCodec.decode_value(input, shape, access, &options)
        }
        DataFormat::Parquet => {
            arcweft_codec_arrow::ParquetCodec.decode_value(input, shape, access, &options)
        }
        DataFormat::ArcweftBinary => {
            arcweft_codec_binary::ArcweftBinaryCodec.decode_value(input, shape, access, &options)
        }
    }
}

fn encode_avro_json_envelope(
    value: &Value,
    shape: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> arcweft_data::Result<Vec<u8>> {
    let json = arcweft_codec_json::JsonCodec.encode_value(
        value,
        shape,
        access,
        &EncodeOptions::default(),
    )?;
    let schema = Schema::parse_str(
        r#"{"type":"record","name":"ArcweftDataEnvelope","fields":[{"name":"json","type":"bytes"}]}"#,
    )
    .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
    let mut writer = Writer::new(&schema, Vec::new());
    writer
        .append(AvroValue::Record(vec![(
            "json".to_owned(),
            AvroValue::Bytes(json),
        )]))
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
    writer
        .into_inner()
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
}

fn decode_avro_json_envelope(
    input: &[u8],
    shape: ShapeRef<'_>,
    access: &dyn ShapeAccess,
) -> arcweft_data::Result<Value> {
    let mut reader = Reader::new(input)
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
    let row = reader
        .next()
        .ok_or_else(|| DataError::new(DataErrorKind::MissingField, "Avro data envelope is empty"))?
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
    let AvroValue::Record(fields) = row else {
        return Err(DataError::invalid_type(
            "Avro data envelope record",
            "other",
        ));
    };
    let Some((_, AvroValue::Bytes(json))) = fields.into_iter().find(|(name, _)| name == "json")
    else {
        return Err(DataError::new(
            DataErrorKind::MissingField,
            "Avro data envelope is missing json bytes",
        ));
    };
    arcweft_codec_json::JsonCodec.decode_value(&json, shape, access, &DecodeOptions::default())
}
