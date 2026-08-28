use super::compile::{exact_i64_result, runtime_value_kind};
use super::{
    AvroValue, BTreeMap, Bytes, BytesFormat, Codec, DataError, DataErrorKind, DataFormat,
    DecodeOptions, DenseMatrixF32, DenseSeq, DenseTensorF32, EncodeOptions, FieldShape,
    MatrixBinaryShapeSignature, MatrixBinaryValueSignature, MatrixMatmulBiasShapeSignature,
    MatrixMatmulBiasValueSignature, Number, PreparedMatrixAddCache,
    PreparedMatrixMatmulBiasAddCache, PreparedMatrixMatmulCache, PreparedTensorAddCache, Reader,
    RecordPolicy, RuntimeCallTarget, RuntimeEvalError, RuntimeExternalCallBackend, RuntimeI64Args,
    RuntimePureAccelerator, RuntimePureHelperRef, RuntimeSeq, RuntimeValue, Schema,
    TensorBinaryShapeSignature, TensorBinaryValueSignature, TypeShape, Value,
    VmPureFunctionScratch, Writer, fmt, math, runtime_sequence_dense_bytes,
    runtime_sequence_dense_usize,
};
use arcweft_core::{
    entry::RuntimeNominalTypeId,
    pattern::{RuntimeSemanticTypeId, RuntimeVariantIdentity},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeAcceleratorExternalCall {
    InferMatmulF32,
    InferAddF32,
    InferBiasAddF32,
    InferMatmulBiasAddF32,
    Conv2dValidF32,
    InferReluF32,
    InferMaxPool2dF32,
    InferSoftmaxLastDimF32,
    InferArgmaxLastDimF32,
    InferFlattenOuterF32,
}

impl RuntimeAcceleratorExternalCall {
    fn from_label(label: &str) -> Option<Self> {
        match label {
            "infer.matmul_f32" => Some(Self::InferMatmulF32),
            "infer.add_f32" => Some(Self::InferAddF32),
            "infer.bias_add_f32" => Some(Self::InferBiasAddF32),
            "infer.matmul_bias_add_f32" => Some(Self::InferMatmulBiasAddF32),
            "conv2d.valid_f32" => Some(Self::Conv2dValidF32),
            "infer.relu_f32" => Some(Self::InferReluF32),
            "infer.max_pool2d_f32" => Some(Self::InferMaxPool2dF32),
            "infer.softmax_last_dim_f32" => Some(Self::InferSoftmaxLastDimF32),
            "infer.argmax_last_dim_f32" => Some(Self::InferArgmaxLastDimF32),
            "infer.flatten_outer_f32" => Some(Self::InferFlattenOuterF32),
            _ => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::InferMatmulF32 => "infer.matmul_f32",
            Self::InferAddF32 => "infer.add_f32",
            Self::InferBiasAddF32 => "infer.bias_add_f32",
            Self::InferMatmulBiasAddF32 => "infer.matmul_bias_add_f32",
            Self::Conv2dValidF32 => "conv2d.valid_f32",
            Self::InferReluF32 => "infer.relu_f32",
            Self::InferMaxPool2dF32 => "infer.max_pool2d_f32",
            Self::InferSoftmaxLastDimF32 => "infer.softmax_last_dim_f32",
            Self::InferArgmaxLastDimF32 => "infer.argmax_last_dim_f32",
            Self::InferFlattenOuterF32 => "infer.flatten_outer_f32",
        }
    }
}

fn call_data_external(
    label: &str,
    args: &[RuntimeValue],
) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
    Some(match (label, args) {
        ("data.encode", [value, format]) => {
            data_format_arg(label, format).and_then(|format| encode_runtime_data(value, format))
        }
        ("data.decode", [bytes, format]) => data_format_arg(label, format).and_then(|format| {
            runtime_value_to_bytes(label, bytes)
                .and_then(|bytes| decode_runtime_data(&bytes, format))
        }),
        ("data.decode", [bytes, format, shape]) => {
            data_format_arg(label, format).and_then(|format| {
                runtime_value_to_bytes(label, bytes).and_then(|bytes| {
                    runtime_value_to_type_shape(shape)
                        .and_then(|shape| decode_runtime_data_with_shape(&bytes, format, &shape))
                })
            })
        }
        ("data.shape", [value]) => runtime_value_to_data_value(value)
            .map(|value| type_shape_to_runtime_value(&infer_data_shape(&value))),
        ("data.encode" | "data.decode" | "data.shape", _) => {
            Err(data_runtime_error(label, "invalid data call arity"))
        }
        _ => return None,
    })
}

fn encode_runtime_data(
    value: &RuntimeValue,
    format: DataFormat,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let value = runtime_value_to_data_value(value)?;
    let shape = infer_data_shape(&value);
    let bytes = match format {
        DataFormat::Json => {
            arcweft_codec_json::JsonCodec.encode_value(&value, &shape, &EncodeOptions::default())
        }
        DataFormat::Toml => {
            arcweft_codec_toml::TomlCodec.encode_value(&value, &shape, &EncodeOptions::default())
        }
        DataFormat::Yaml => {
            arcweft_codec_yaml::YamlCodec.encode_value(&value, &shape, &EncodeOptions::default())
        }
        DataFormat::MessagePack => arcweft_codec_msgpack::MessagePackCodec.encode_value(
            &value,
            &shape,
            &EncodeOptions::default(),
        ),
        DataFormat::Cbor => {
            arcweft_codec_cbor::CborCodec.encode_value(&value, &shape, &EncodeOptions::default())
        }
        DataFormat::Avro => encode_dynamic_avro(&value),
        DataFormat::Csv => {
            arcweft_codec_csv::CsvCodec.encode_value(&value, &shape, &EncodeOptions::default())
        }
        DataFormat::ArrowIpc => arcweft_codec_arrow::ArrowIpcCodec.encode_value(
            &value,
            &shape,
            &EncodeOptions::default(),
        ),
        DataFormat::Parquet => arcweft_codec_arrow::ParquetCodec.encode_value(
            &value,
            &shape,
            &EncodeOptions::default(),
        ),
        DataFormat::ArcweftBinary => arcweft_codec_binary::ArcweftBinaryCodec.encode_value(
            &value,
            &shape,
            &EncodeOptions::default(),
        ),
    }
    .map_err(|error| data_runtime_error(format.id(), error.to_string()))?;
    Ok(runtime_sequence_dense_bytes(bytes))
}

fn decode_runtime_data(input: &[u8], format: DataFormat) -> Result<RuntimeValue, RuntimeEvalError> {
    let value = match format {
        DataFormat::Json => decode_dynamic_json(input),
        DataFormat::Avro => decode_dynamic_avro(input),
        DataFormat::Toml
        | DataFormat::Yaml
        | DataFormat::MessagePack
        | DataFormat::Cbor
        | DataFormat::Csv
        | DataFormat::ArrowIpc
        | DataFormat::Parquet
        | DataFormat::ArcweftBinary => Err(dynamic_decode_shape_error(format)),
    }
    .map_err(|error| data_runtime_error(format.id(), error.to_string()))?;
    data_value_to_runtime_value(value)
}

fn decode_runtime_data_with_shape(
    input: &[u8],
    format: DataFormat,
    shape: &TypeShape,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let value = match format {
        DataFormat::Json => arcweft_codec_json::JsonCodec.decode_value(
            input,
            shape,
            &DecodeOptions::default(),
        ),
        DataFormat::Toml => arcweft_codec_toml::TomlCodec.decode_value(
            input,
            shape,
            &DecodeOptions::default(),
        ),
        DataFormat::Yaml => arcweft_codec_yaml::YamlCodec.decode_value(
            input,
            shape,
            &DecodeOptions::default(),
        ),
        DataFormat::MessagePack => arcweft_codec_msgpack::MessagePackCodec.decode_value(
            input,
            shape,
            &DecodeOptions::default(),
        ),
        DataFormat::Cbor => arcweft_codec_cbor::CborCodec.decode_value(
            input,
            shape,
            &DecodeOptions::default(),
        ),
        DataFormat::Csv => arcweft_codec_csv::CsvCodec.decode_value(
            input,
            shape,
            &DecodeOptions::default(),
        ),
        DataFormat::ArrowIpc => arcweft_codec_arrow::ArrowIpcCodec.decode_value(
            input,
            shape,
            &DecodeOptions::default(),
        ),
        DataFormat::Parquet => arcweft_codec_arrow::ParquetCodec.decode_value(
            input,
            shape,
            &DecodeOptions::default(),
        ),
        DataFormat::ArcweftBinary => arcweft_codec_binary::ArcweftBinaryCodec.decode_value(
            input,
            shape,
            &DecodeOptions::default(),
        ),
        DataFormat::Avro => Err(DataError::unsupported(
            "runtime Avro data.decode with explicit TypeShape requires an Avro schema-bearing codec",
        )),
    }
    .map_err(|error| data_runtime_error(format.id(), error.to_string()))?;
    data_value_to_runtime_value_with_shape(value, shape)
}

fn data_format_arg(label: &str, value: &RuntimeValue) -> Result<DataFormat, RuntimeEvalError> {
    match value {
        RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Nominal { nominal, .. },
            ordinal,
            name,
            payload: None,
        } if nominal.as_str() == "DataFormat" => DataFormat::from_variant_name(name)
            .filter(|format| {
                DataFormat::ALL.get(usize::try_from(*ordinal).unwrap_or(usize::MAX)) == Some(format)
            })
            .ok_or_else(|| {
                data_runtime_error(
                    label,
                    format!("unknown DataFormat case #{ordinal} `{name}`"),
                )
            }),
        other => Err(data_runtime_error(
            label,
            format!(
                "format must be a DataFormat enum value, found {}",
                runtime_value_label_for_data(other)
            ),
        )),
    }
}

fn runtime_value_to_bytes(label: &str, value: &RuntimeValue) -> Result<Vec<u8>, RuntimeEvalError> {
    let bytes = match value {
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(values) | DenseSeq::U8(values))) => {
            values.as_slice().to_vec()
        }
        RuntimeValue::Seq(seq) => seq
            .clone()
            .into_values()
            .into_iter()
            .map(|item| match item {
                RuntimeValue::UInt(value) => value
                    .try_into_u32()
                    .and_then(|value| u8::try_from(value).ok()),
                RuntimeValue::Int(value) => value
                    .try_into_i32()
                    .and_then(|value| u8::try_from(value).ok()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                data_runtime_error(label, "bytes argument must be Bytes or u8 sequence")
            })?,
        other => {
            return Err(data_runtime_error(
                label,
                format!(
                    "bytes argument must be Bytes or u8 sequence, found {}",
                    runtime_value_label_for_data(other)
                ),
            ));
        }
    };
    Ok(bytes)
}

fn runtime_value_to_data_value(value: &RuntimeValue) -> Result<Value, RuntimeEvalError> {
    match value {
        RuntimeValue::Unit => Ok(Value::Unit),
        RuntimeValue::Bool(value) => Ok(Value::Bool(*value)),
        RuntimeValue::Int(value) => Ok(Value::Number(Number::I(runtime_int_to_i128(*value)))),
        RuntimeValue::UInt(value) => Ok(Value::Number(Number::U(runtime_uint_to_u128(*value)))),
        RuntimeValue::F32(value) => Ok(Value::Number(Number::F32(*value))),
        RuntimeValue::F64(value) => Ok(Value::Number(Number::F64(*value))),
        RuntimeValue::String(value) => Ok(Value::String(value.clone())),
        RuntimeValue::EntityRef(value) => Ok(Value::String(value.runtime_label())),
        RuntimeValue::Char(value) => Ok(Value::Char(*value)),
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(values))) => {
            Ok(Value::Bytes(Bytes::new(values.as_slice().to_vec())))
        }
        RuntimeValue::Seq(seq) => seq
            .clone()
            .into_values()
            .into_iter()
            .map(|value| runtime_value_to_data_value(&value))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Seq),
        RuntimeValue::Tuple(values) => values
            .iter()
            .map(runtime_value_to_data_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Seq),
        RuntimeValue::Record(fields) => fields
            .iter()
            .map(|field| {
                runtime_value_to_data_value(field.value())
                    .map(|value| (field.name().to_owned(), value))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(Value::Record),
        RuntimeValue::Variant {
            owner: _,
            ordinal: _,
            name,
            payload,
        } => payload
            .as_ref()
            .map(|payload| runtime_value_to_data_value(payload))
            .transpose()
            .map(|payload| Value::Enum {
                variant: name.clone(),
                payload: payload.map(Box::new),
            }),
        RuntimeValue::Duration(_)
        | RuntimeValue::Progress(_)
        | RuntimeValue::NominalRecord(_)
        | RuntimeValue::Opaque(_)
        | RuntimeValue::Agent(_)
        | RuntimeValue::Reduction(_)
        | RuntimeValue::Function(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::Range(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_) => Err(data_runtime_error(
            "data",
            format!(
                "runtime value {} is not data-serializable yet",
                runtime_value_label_for_data(value)
            ),
        )),
    }
}

fn data_value_to_runtime_value(value: Value) -> Result<RuntimeValue, RuntimeEvalError> {
    match value {
        Value::Unit => Ok(RuntimeValue::Unit),
        Value::Bool(value) => Ok(RuntimeValue::Bool(value)),
        Value::Number(Number::I(value)) => Ok(RuntimeValue::i128(value)),
        Value::Number(Number::U(value)) => Ok(RuntimeValue::u128(value)),
        Value::Number(Number::F32(value)) => Ok(RuntimeValue::F32(value)),
        Value::Number(Number::F64(value)) => Ok(RuntimeValue::F64(value)),
        Value::String(value) => Ok(RuntimeValue::String(value)),
        Value::Char(value) => Ok(RuntimeValue::Char(value)),
        Value::Bytes(bytes) => Ok(runtime_sequence_dense_bytes(bytes.into_vec())),
        Value::Seq(values) => values
            .into_iter()
            .map(data_value_to_runtime_value)
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeSeq::Values)
            .map(RuntimeValue::Seq),
        Value::Map(values) | Value::Record(values) => values
            .into_iter()
            .map(|(name, value)| data_value_to_runtime_value(value).map(|value| (name, value)))
            .collect::<Result<Vec<_>, _>>()
            .and_then(|fields| {
                RuntimeValue::try_record(fields)
                    .map_err(|error| data_runtime_error("data.decode", error.to_string()))
            }),
        Value::Enum { .. } => Err(data_runtime_error(
            "data.decode",
            "enum values require an explicit TypeShape so their typed owner and case ordinal are preserved",
        )),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive typed data-shape conversion matrix must remain visibly total"
)]
fn data_value_to_runtime_value_with_shape(
    value: Value,
    shape: &TypeShape,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match (value, shape) {
        (Value::Unit, TypeShape::Unit) => Ok(RuntimeValue::Unit),
        (Value::Bool(value), TypeShape::Bool) => Ok(RuntimeValue::Bool(value)),
        (Value::Number(Number::I(value)), TypeShape::I8) => i8::try_from(value)
            .map(RuntimeValue::i8)
            .map_err(|_| data_shape_value_error("i8")),
        (Value::Number(Number::I(value)), TypeShape::I16) => i16::try_from(value)
            .map(RuntimeValue::i16)
            .map_err(|_| data_shape_value_error("i16")),
        (Value::Number(Number::I(value)), TypeShape::I32) => i32::try_from(value)
            .map(RuntimeValue::i32)
            .map_err(|_| data_shape_value_error("i32")),
        (Value::Number(Number::I(value)), TypeShape::I64) => i64::try_from(value)
            .map(RuntimeValue::i64)
            .map_err(|_| data_shape_value_error("i64")),
        (Value::Number(Number::I(value)), TypeShape::I128) => Ok(RuntimeValue::i128(value)),
        (Value::Number(Number::I(value)), TypeShape::Isize) => i64::try_from(value)
            .map(RuntimeValue::isize)
            .map_err(|_| data_shape_value_error("isize")),
        (Value::Number(Number::U(value)), TypeShape::U8) => u8::try_from(value)
            .map(RuntimeValue::u8)
            .map_err(|_| data_shape_value_error("u8")),
        (Value::Number(Number::U(value)), TypeShape::U16) => u16::try_from(value)
            .map(RuntimeValue::u16)
            .map_err(|_| data_shape_value_error("u16")),
        (Value::Number(Number::U(value)), TypeShape::U32) => u32::try_from(value)
            .map(RuntimeValue::u32)
            .map_err(|_| data_shape_value_error("u32")),
        (Value::Number(Number::U(value)), TypeShape::U64) => u64::try_from(value)
            .map(RuntimeValue::u64)
            .map_err(|_| data_shape_value_error("u64")),
        (Value::Number(Number::U(value)), TypeShape::U128) => Ok(RuntimeValue::u128(value)),
        (Value::Number(Number::U(value)), TypeShape::Usize) => u64::try_from(value)
            .map(RuntimeValue::usize)
            .map_err(|_| data_shape_value_error("usize")),
        (Value::Number(Number::F32(value)), TypeShape::F32) => Ok(RuntimeValue::F32(value)),
        (Value::Number(Number::F64(value)), TypeShape::F64) => Ok(RuntimeValue::F64(value)),
        (Value::String(value), TypeShape::String) => Ok(RuntimeValue::String(value)),
        (Value::Char(value), TypeShape::Char) => Ok(RuntimeValue::Char(value)),
        (Value::Bytes(bytes), TypeShape::Bytes { .. }) => {
            Ok(runtime_sequence_dense_bytes(bytes.into_vec()))
        }
        (Value::Unit, TypeShape::Option(_)) => Ok(RuntimeValue::option_none()),
        (value, TypeShape::Option(inner)) => {
            data_value_to_runtime_value_with_shape(value, inner).map(RuntimeValue::option_some)
        }
        (Value::Seq(values), TypeShape::Seq(item)) => values
            .into_iter()
            .map(|value| data_value_to_runtime_value_with_shape(value, item))
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeSeq::Values)
            .map(RuntimeValue::Seq),
        (Value::Map(values), TypeShape::Map { value, .. }) => values
            .into_iter()
            .map(|(name, item)| {
                data_value_to_runtime_value_with_shape(item, value).map(|value| (name, value))
            })
            .collect::<Result<Vec<_>, _>>()
            .and_then(|fields| {
                RuntimeValue::try_record(fields)
                    .map_err(|error| data_runtime_error("data.decode", error.to_string()))
            }),
        (Value::Record(values), TypeShape::Record { fields, .. }) => values
            .into_iter()
            .map(|(name, value)| {
                let field = fields
                    .iter()
                    .find(|field| field.wire_name == name)
                    .ok_or_else(|| {
                        data_runtime_error(
                            "data.decode",
                            format!("decoded record field `{name}` is absent from TypeShape"),
                        )
                    })?;
                data_value_to_runtime_value_with_shape(value, &field.value_shape())
                    .map(|value| (name, value))
            })
            .collect::<Result<Vec<_>, _>>()
            .and_then(|fields| {
                RuntimeValue::try_record(fields)
                    .map_err(|error| data_runtime_error("data.decode", error.to_string()))
            }),
        (Value::Enum { variant, payload }, shape @ TypeShape::Enum { name, variants, .. }) => {
            let (ordinal, case) = variants
                .iter()
                .enumerate()
                .find(|(_, case)| case.wire_name == variant)
                .ok_or_else(|| {
                    data_runtime_error(
                        "data.decode",
                        format!("decoded enum case `{variant}` is absent from TypeShape `{name}`"),
                    )
                })?;
            let payload = match (payload, case.payload.as_ref()) {
                (None, None) => None,
                (Some(payload), Some(payload_shape)) => Some(Box::new(
                    data_value_to_runtime_value_with_shape(*payload, payload_shape)?,
                )),
                _ => {
                    return Err(data_runtime_error(
                        "data.decode",
                        format!(
                            "decoded enum case `{variant}` payload does not match TypeShape `{name}`"
                        ),
                    ));
                }
            };
            Ok(RuntimeValue::Variant {
                owner: runtime_data_enum_identity(shape, name)?,
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    data_runtime_error("data.decode", "enum case ordinal exceeds u32")
                })?,
                name: variant,
                payload,
            })
        }
        (_, TypeShape::Named(name)) => Err(data_runtime_error(
            "data.decode",
            format!("named TypeShape `{name}` has no admitted runtime value projection"),
        )),
        (value, shape) => Err(data_runtime_error(
            "data.decode",
            format!(
                "decoded {} value does not match explicit {} TypeShape",
                value.type_name(),
                shape.type_name()
            ),
        )),
    }
}

fn data_shape_value_error(expected: &'static str) -> RuntimeEvalError {
    data_runtime_error(
        "data.decode",
        format!("decoded numeric value is outside the explicit {expected} TypeShape"),
    )
}

fn runtime_data_enum_identity(
    shape: &TypeShape,
    name: &str,
) -> Result<RuntimeVariantIdentity, RuntimeEvalError> {
    let digest = semantic_type_shape_value(shape)
        .try_digest(1_048_576)
        .map_err(|error| {
            data_runtime_error(
                "data.decode",
                format!("failed to derive enum TypeShape identity: {error}"),
            )
        })?;
    let nominal = RuntimeNominalTypeId::try_new(format!("arcweft.data.{name}"))
        .map_err(|error| data_runtime_error("data.decode", error.to_string()))?;
    Ok(RuntimeVariantIdentity::Nominal {
        nominal,
        semantic_identity: RuntimeSemanticTypeId::from_bytes(*digest.as_bytes()),
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive semantic TypeShape encoding matrix must remain visibly total"
)]
fn semantic_type_shape_value(shape: &TypeShape) -> RuntimeValue {
    let tuple = |kind: &str, values: Vec<RuntimeValue>| {
        let mut output = Vec::with_capacity(values.len() + 1);
        output.push(RuntimeValue::String(kind.to_owned()));
        output.extend(values);
        RuntimeValue::Tuple(output)
    };
    match shape {
        TypeShape::Unit => tuple("unit", Vec::new()),
        TypeShape::Bool => tuple("bool", Vec::new()),
        TypeShape::I8 => tuple("i8", Vec::new()),
        TypeShape::I16 => tuple("i16", Vec::new()),
        TypeShape::I32 => tuple("i32", Vec::new()),
        TypeShape::I64 => tuple("i64", Vec::new()),
        TypeShape::I128 => tuple("i128", Vec::new()),
        TypeShape::Isize => tuple("isize", Vec::new()),
        TypeShape::U8 => tuple("u8", Vec::new()),
        TypeShape::U16 => tuple("u16", Vec::new()),
        TypeShape::U32 => tuple("u32", Vec::new()),
        TypeShape::U64 => tuple("u64", Vec::new()),
        TypeShape::U128 => tuple("u128", Vec::new()),
        TypeShape::Usize => tuple("usize", Vec::new()),
        TypeShape::F32 => tuple("f32", Vec::new()),
        TypeShape::F64 => tuple("f64", Vec::new()),
        TypeShape::String => tuple("string", Vec::new()),
        TypeShape::Char => tuple("char", Vec::new()),
        TypeShape::Bytes { format } => tuple(
            "bytes",
            vec![RuntimeValue::String(
                bytes_format_identity(*format).to_owned(),
            )],
        ),
        TypeShape::Option(item) => tuple("option", vec![semantic_type_shape_value(item)]),
        TypeShape::Seq(item) => tuple("seq", vec![semantic_type_shape_value(item)]),
        TypeShape::Map { key, value } => tuple(
            "map",
            vec![
                semantic_type_shape_value(key),
                semantic_type_shape_value(value),
            ],
        ),
        TypeShape::Record {
            name,
            fields,
            policy,
        } => tuple(
            "record",
            vec![
                RuntimeValue::String(name.clone()),
                RuntimeValue::Bool(policy.deny_unknown_fields),
                RuntimeValue::Seq(RuntimeSeq::Values(
                    fields
                        .iter()
                        .map(|field| {
                            tuple(
                                "field",
                                vec![
                                    RuntimeValue::String(field.rust_name.clone()),
                                    RuntimeValue::String(field.wire_name.clone()),
                                    semantic_type_shape_value(&field.shape),
                                    RuntimeValue::Bool(field.has_default),
                                    RuntimeValue::Bool(field.skip),
                                    field.bytes_format.map_or_else(
                                        RuntimeValue::option_none,
                                        |format| {
                                            RuntimeValue::option_some(RuntimeValue::String(
                                                bytes_format_identity(format).to_owned(),
                                            ))
                                        },
                                    ),
                                ],
                            )
                        })
                        .collect(),
                )),
            ],
        ),
        TypeShape::Enum {
            name,
            variants,
            tag,
            repr,
        } => tuple(
            "enum",
            vec![
                RuntimeValue::String(name.clone()),
                RuntimeValue::Seq(RuntimeSeq::Values(
                    variants
                        .iter()
                        .map(|variant| {
                            tuple(
                                "case",
                                vec![
                                    RuntimeValue::String(variant.rust_name.clone()),
                                    RuntimeValue::String(variant.wire_name.clone()),
                                    variant.payload.as_ref().map_or_else(
                                        RuntimeValue::option_none,
                                        |payload| {
                                            RuntimeValue::option_some(semantic_type_shape_value(
                                                payload,
                                            ))
                                        },
                                    ),
                                    variant.discriminant.map_or_else(
                                        RuntimeValue::option_none,
                                        |value| {
                                            RuntimeValue::option_some(RuntimeValue::i128(value))
                                        },
                                    ),
                                ],
                            )
                        })
                        .collect(),
                )),
                enum_tag_identity(tag),
                repr.map_or_else(RuntimeValue::option_none, |repr| {
                    RuntimeValue::option_some(RuntimeValue::String(
                        enum_repr_identity(repr).to_owned(),
                    ))
                }),
            ],
        ),
        TypeShape::Named(name) => tuple("named", vec![RuntimeValue::String(name.clone())]),
    }
}

const fn bytes_format_identity(format: BytesFormat) -> &'static str {
    match format {
        BytesFormat::Binary => "binary",
        BytesFormat::Base64 => "base64",
        BytesFormat::Hex => "hex",
        BytesFormat::Array => "array",
    }
}

fn enum_tag_identity(tag: &arcweft_data::EnumTagStyle) -> RuntimeValue {
    match tag {
        arcweft_data::EnumTagStyle::External => {
            RuntimeValue::Tuple(vec![RuntimeValue::String("external".to_owned())])
        }
        arcweft_data::EnumTagStyle::Internal { tag } => RuntimeValue::Tuple(vec![
            RuntimeValue::String("internal".to_owned()),
            RuntimeValue::String(tag.clone()),
        ]),
        arcweft_data::EnumTagStyle::Adjacent { tag, content } => RuntimeValue::Tuple(vec![
            RuntimeValue::String("adjacent".to_owned()),
            RuntimeValue::String(tag.clone()),
            RuntimeValue::String(content.clone()),
        ]),
    }
}

const fn enum_repr_identity(repr: arcweft_data::EnumRepr) -> &'static str {
    match repr {
        arcweft_data::EnumRepr::I8 => "i8",
        arcweft_data::EnumRepr::I16 => "i16",
        arcweft_data::EnumRepr::I32 => "i32",
        arcweft_data::EnumRepr::I64 => "i64",
        arcweft_data::EnumRepr::I128 => "i128",
        arcweft_data::EnumRepr::Isize => "isize",
        arcweft_data::EnumRepr::U8 => "u8",
        arcweft_data::EnumRepr::U16 => "u16",
        arcweft_data::EnumRepr::U32 => "u32",
        arcweft_data::EnumRepr::U64 => "u64",
        arcweft_data::EnumRepr::U128 => "u128",
        arcweft_data::EnumRepr::Usize => "usize",
    }
}

fn infer_data_shape(value: &Value) -> TypeShape {
    match value {
        Value::Unit => TypeShape::Unit,
        Value::Bool(_) => TypeShape::Bool,
        Value::Number(Number::I(_)) => TypeShape::I128,
        Value::Number(Number::U(_)) => TypeShape::U128,
        Value::Number(Number::F32(_)) => TypeShape::F32,
        Value::Number(Number::F64(_)) => TypeShape::F64,
        Value::String(_) => TypeShape::String,
        Value::Char(_) => TypeShape::Char,
        Value::Bytes(_) => TypeShape::Bytes {
            format: BytesFormat::Binary,
        },
        Value::Seq(values) => values.first().map_or_else(
            || TypeShape::Seq(Box::new(TypeShape::Named("Unknown".to_owned()))),
            |first| {
                let first_shape = infer_data_shape(first);
                let item_shape = if values
                    .iter()
                    .skip(1)
                    .all(|value| infer_data_shape(value) == first_shape)
                {
                    first_shape
                } else {
                    TypeShape::Named("Value".to_owned())
                };
                TypeShape::Seq(Box::new(item_shape))
            },
        ),
        Value::Map(values) | Value::Record(values) => TypeShape::Record {
            name: "RuntimeRecord".to_owned(),
            fields: values
                .iter()
                .map(|(name, value)| {
                    FieldShape::new(name.clone(), name.clone(), infer_data_shape(value))
                })
                .collect(),
            policy: RecordPolicy::default(),
        },
        Value::Enum { variant, payload } => TypeShape::enumeration(
            "RuntimeEnum",
            [
                arcweft_data::VariantShape::unit(variant.clone(), variant.clone()).with_payload(
                    payload
                        .as_ref()
                        .map_or(TypeShape::Unit, |payload| infer_data_shape(payload)),
                ),
            ],
        ),
    }
}

fn type_shape_to_runtime_value(shape: &TypeShape) -> RuntimeValue {
    match shape {
        TypeShape::Record { name, fields, .. } => data_record(vec![
            data_field("kind", RuntimeValue::String("record".to_owned())),
            data_field("name", RuntimeValue::String(name.clone())),
            data_field(
                "fields",
                RuntimeValue::Seq(RuntimeSeq::Values(
                    fields
                        .iter()
                        .map(|field| {
                            data_record(vec![
                                data_field("name", RuntimeValue::String(field.wire_name.clone())),
                                data_field("shape", type_shape_to_runtime_value(&field.shape)),
                            ])
                        })
                        .collect(),
                )),
            ),
        ]),
        TypeShape::Seq(item) => data_record(vec![
            data_field("kind", RuntimeValue::String("seq".to_owned())),
            data_field("item", type_shape_to_runtime_value(item)),
        ]),
        TypeShape::Bytes { .. } => data_record(vec![data_field(
            "kind",
            RuntimeValue::String("bytes".to_owned()),
        )]),
        TypeShape::Enum { name, variants, .. } => data_record(vec![
            data_field("kind", RuntimeValue::String("enum".to_owned())),
            data_field("name", RuntimeValue::String(name.clone())),
            data_field(
                "variants",
                RuntimeValue::Seq(RuntimeSeq::Values(
                    variants
                        .iter()
                        .map(|variant| RuntimeValue::String(variant.wire_name.clone()))
                        .collect(),
                )),
            ),
        ]),
        TypeShape::Named(name) => data_record(vec![
            data_field("kind", RuntimeValue::String("named".to_owned())),
            data_field("name", RuntimeValue::String(name.clone())),
        ]),
        other => data_record(vec![data_field(
            "kind",
            RuntimeValue::String(type_shape_kind_label(other).to_owned()),
        )]),
    }
}

fn runtime_value_to_type_shape(value: &RuntimeValue) -> Result<TypeShape, RuntimeEvalError> {
    let fields = runtime_record_fields(value, "data.decode shape")?;
    let kind = runtime_record_string_field(fields, "kind", "data.decode shape")?;
    match kind {
        "unit" => Ok(TypeShape::Unit),
        "bool" => Ok(TypeShape::Bool),
        "i8" => Ok(TypeShape::I8),
        "i16" => Ok(TypeShape::I16),
        "i32" => Ok(TypeShape::I32),
        "i64" => Ok(TypeShape::I64),
        "i128" => Ok(TypeShape::I128),
        "isize" => Ok(TypeShape::Isize),
        "u8" => Ok(TypeShape::U8),
        "u16" => Ok(TypeShape::U16),
        "u32" => Ok(TypeShape::U32),
        "u64" => Ok(TypeShape::U64),
        "u128" => Ok(TypeShape::U128),
        "usize" => Ok(TypeShape::Usize),
        "f32" => Ok(TypeShape::F32),
        "f64" => Ok(TypeShape::F64),
        "string" => Ok(TypeShape::String),
        "char" => Ok(TypeShape::Char),
        "bytes" => Ok(TypeShape::Bytes {
            format: BytesFormat::Binary,
        }),
        "seq" => runtime_record_field(fields, "item", "data.decode seq shape")
            .and_then(runtime_value_to_type_shape)
            .map(|item| TypeShape::Seq(Box::new(item))),
        "record" => runtime_record_to_record_shape(fields),
        "enum" => runtime_record_to_enum_shape(fields),
        "named" | "option" | "map" => Err(data_runtime_error(
            "data.decode",
            format!("runtime shape kind `{kind}` is not supported for explicit decode"),
        )),
        other => Err(data_runtime_error(
            "data.decode",
            format!("unknown runtime shape kind `{other}`"),
        )),
    }
}

fn runtime_record_to_record_shape(
    fields: &[arcweft_core::value::RuntimeFieldValue],
) -> Result<TypeShape, RuntimeEvalError> {
    let name = runtime_record_string_field(fields, "name", "data.decode record shape")?.to_owned();
    let RuntimeValue::Seq(field_values) =
        runtime_record_field(fields, "fields", "data.decode record shape")?
    else {
        return Err(data_runtime_error(
            "data.decode",
            "record shape field `fields` must be a sequence",
        ));
    };
    let fields = field_values
        .clone()
        .into_values()
        .into_iter()
        .map(|field| {
            let field_record = runtime_record_fields(&field, "data.decode record field shape")?;
            let name =
                runtime_record_string_field(field_record, "name", "data.decode record field")?
                    .to_owned();
            let shape = runtime_record_field(field_record, "shape", "data.decode record field")
                .and_then(runtime_value_to_type_shape)?;
            Ok::<_, RuntimeEvalError>(FieldShape::new(name.clone(), name, shape))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TypeShape::Record {
        name,
        fields,
        policy: RecordPolicy::default(),
    })
}

fn runtime_record_to_enum_shape(
    fields: &[arcweft_core::value::RuntimeFieldValue],
) -> Result<TypeShape, RuntimeEvalError> {
    let name = runtime_record_string_field(fields, "name", "data.decode enum shape")?.to_owned();
    let RuntimeValue::Seq(variants) =
        runtime_record_field(fields, "variants", "data.decode enum shape")?
    else {
        return Err(data_runtime_error(
            "data.decode",
            "enum shape field `variants` must be a sequence",
        ));
    };
    let variants = variants
        .clone()
        .into_values()
        .into_iter()
        .map(|variant| match variant {
            RuntimeValue::String(name) => {
                Ok::<_, RuntimeEvalError>(arcweft_data::VariantShape::unit(name.clone(), name))
            }
            other => Err(data_runtime_error(
                "data.decode",
                format!(
                    "enum shape variants must be strings, found {}",
                    runtime_value_label_for_data(&other)
                ),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TypeShape::enumeration(name, variants))
}

fn runtime_record_fields<'a>(
    value: &'a RuntimeValue,
    context: &str,
) -> Result<&'a [arcweft_core::value::RuntimeFieldValue], RuntimeEvalError> {
    match value {
        RuntimeValue::Record(fields) => Ok(fields),
        other => Err(data_runtime_error(
            "data.decode",
            format!(
                "{context} must be a record, found {}",
                runtime_value_label_for_data(other)
            ),
        )),
    }
}

fn runtime_record_field<'a>(
    fields: &'a [arcweft_core::value::RuntimeFieldValue],
    name: &str,
    context: &str,
) -> Result<&'a RuntimeValue, RuntimeEvalError> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(arcweft_core::value::RuntimeFieldValue::value)
        .ok_or_else(|| data_runtime_error("data.decode", format!("{context} is missing `{name}`")))
}

fn runtime_record_string_field<'a>(
    fields: &'a [arcweft_core::value::RuntimeFieldValue],
    name: &str,
    context: &str,
) -> Result<&'a str, RuntimeEvalError> {
    match runtime_record_field(fields, name, context)? {
        RuntimeValue::String(value) => Ok(value),
        other => Err(data_runtime_error(
            "data.decode",
            format!(
                "{context} field `{name}` must be a string, found {}",
                runtime_value_label_for_data(other)
            ),
        )),
    }
}

fn type_shape_kind_label(shape: &TypeShape) -> &'static str {
    match shape {
        TypeShape::Unit => "unit",
        TypeShape::Bool => "bool",
        TypeShape::I8 => "i8",
        TypeShape::I16 => "i16",
        TypeShape::I32 => "i32",
        TypeShape::I64 => "i64",
        TypeShape::I128 => "i128",
        TypeShape::Isize => "isize",
        TypeShape::U8 => "u8",
        TypeShape::U16 => "u16",
        TypeShape::U32 => "u32",
        TypeShape::U64 => "u64",
        TypeShape::U128 => "u128",
        TypeShape::Usize => "usize",
        TypeShape::F32 => "f32",
        TypeShape::F64 => "f64",
        TypeShape::String => "string",
        TypeShape::Char => "char",
        TypeShape::Bytes { .. } => "bytes",
        TypeShape::Option(_) => "option",
        TypeShape::Seq(_) => "seq",
        TypeShape::Map { .. } => "map",
        TypeShape::Record { .. } => "record",
        TypeShape::Enum { .. } => "enum",
        TypeShape::Named(_) => "named",
    }
}

fn data_field(name: &str, value: RuntimeValue) -> (String, RuntimeValue) {
    (name.to_owned(), value)
}

fn data_record(fields: Vec<(String, RuntimeValue)>) -> RuntimeValue {
    RuntimeValue::try_record(fields).expect("data type-shape record has fixed unique fields")
}

fn encode_dynamic_avro(value: &Value) -> arcweft_data::Result<Vec<u8>> {
    let schema = dynamic_avro_schema()?;
    let json = arcweft_codec_json::JsonCodec.encode_value(
        value,
        &infer_data_shape(value),
        &EncodeOptions::default(),
    )?;
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

fn decode_dynamic_avro(input: &[u8]) -> arcweft_data::Result<Value> {
    let mut reader = Reader::new(input)
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
    let Some(row) = reader.next() else {
        return Err(DataError::new(
            DataErrorKind::MissingField,
            "Avro data envelope is empty",
        ));
    };
    let row =
        row.map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
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
    decode_dynamic_json(&json)
}

fn dynamic_avro_schema() -> arcweft_data::Result<Schema> {
    Schema::parse_str(
        r#"{"type":"record","name":"ArcweftDataEnvelope","fields":[{"name":"json","type":"bytes"}]}"#,
    )
    .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
}

fn decode_dynamic_json(input: &[u8]) -> arcweft_data::Result<Value> {
    let json = serde_json::from_slice(input)
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
    arcweft_codec_json::from_json_value(&json)
}

fn dynamic_decode_shape_error(format: DataFormat) -> DataError {
    DataError::unsupported(format!(
        "{} runtime data.decode requires an explicit TypeShape and is not a dynamic data format",
        format.id()
    ))
}

fn runtime_int_to_i128(value: arcweft_core::value::RuntimeInt) -> i128 {
    match value {
        arcweft_core::value::RuntimeInt::I8(value) => i128::from(value),
        arcweft_core::value::RuntimeInt::I16(value) => i128::from(value),
        arcweft_core::value::RuntimeInt::I32(value) => i128::from(value),
        arcweft_core::value::RuntimeInt::I64(value)
        | arcweft_core::value::RuntimeInt::ISize(value) => i128::from(value),
        arcweft_core::value::RuntimeInt::I128(value) => value,
    }
}

fn runtime_uint_to_u128(value: arcweft_core::value::RuntimeUInt) -> u128 {
    match value {
        arcweft_core::value::RuntimeUInt::U8(value) => u128::from(value),
        arcweft_core::value::RuntimeUInt::U16(value) => u128::from(value),
        arcweft_core::value::RuntimeUInt::U32(value) => u128::from(value),
        arcweft_core::value::RuntimeUInt::U64(value)
        | arcweft_core::value::RuntimeUInt::USize(value) => u128::from(value),
        arcweft_core::value::RuntimeUInt::U128(value) => value,
    }
}

fn data_runtime_error(name: impl Into<String>, reason: impl Into<String>) -> RuntimeEvalError {
    RuntimeEvalError::UnsupportedPure {
        name: name.into(),
        reason: reason.into(),
    }
}

fn runtime_value_label_for_data(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()".to_owned(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::UInt(value) => value.to_string(),
        RuntimeValue::F32(value) => value.to_string(),
        RuntimeValue::F64(value) => value.to_string(),
        RuntimeValue::String(value) => format!("string/{value}"),
        RuntimeValue::Char(value) => format!("char/{value}"),
        RuntimeValue::Seq(seq) => format!("seq/{}", seq.len()),
        RuntimeValue::Tuple(values) => format!("tuple/{}", values.len()),
        RuntimeValue::Record(fields) => format!("record/{}", fields.len()),
        RuntimeValue::NominalRecord(record) => {
            format!("nominal-record/{}", record.type_id().as_str())
        }
        RuntimeValue::Range(range) => range.label(),
        RuntimeValue::Agent(value) => value.label().to_owned(),
        RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            ..
        } => format!("variant/{owner:?}/#{ordinal}/{name}"),
        RuntimeValue::Duration(_)
        | RuntimeValue::Progress(_)
        | RuntimeValue::EntityRef(_)
        | RuntimeValue::Opaque(_)
        | RuntimeValue::Reduction(_)
        | RuntimeValue::Function(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_) => "non-data runtime value".to_owned(),
    }
}

impl RuntimeExternalCallBackend for RuntimePureAccelerator {
    fn call_external(
        &mut self,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        if let Some(result) = call_data_external(callee.as_label(), args) {
            return Some(result);
        }
        let call = RuntimeAcceleratorExternalCall::from_label(callee.as_label())?;
        Some(match (call, args) {
            (
                RuntimeAcceleratorExternalCall::InferMatmulF32,
                [RuntimeValue::TensorF32(lhs), RuntimeValue::TensorF32(rhs)],
            ) => self
                .call_infer_matmul_f32(lhs, rhs)
                .map(RuntimeValue::tensor_f32),
            (
                RuntimeAcceleratorExternalCall::InferAddF32,
                [RuntimeValue::TensorF32(lhs), RuntimeValue::TensorF32(rhs)],
            ) => self
                .call_infer_add_f32(lhs, rhs)
                .map(RuntimeValue::tensor_f32),
            (
                RuntimeAcceleratorExternalCall::InferBiasAddF32,
                [
                    RuntimeValue::TensorF32(tensor),
                    RuntimeValue::TensorF32(bias),
                ],
            ) => self
                .call_infer_bias_add_f32(tensor, bias)
                .map(RuntimeValue::tensor_f32),
            (
                RuntimeAcceleratorExternalCall::InferMatmulBiasAddF32,
                [
                    RuntimeValue::TensorF32(lhs),
                    RuntimeValue::TensorF32(rhs),
                    RuntimeValue::TensorF32(bias),
                ],
            ) => self
                .call_infer_matmul_bias_add_f32(lhs, rhs, bias)
                .map(RuntimeValue::tensor_f32),
            (
                RuntimeAcceleratorExternalCall::Conv2dValidF32,
                [
                    RuntimeValue::TensorF32(input),
                    RuntimeValue::TensorF32(kernel),
                    stride_y,
                    stride_x,
                ],
            ) => runtime_value_to_usize(call, stride_y).and_then(|stride_y| {
                runtime_value_to_usize(call, stride_x).and_then(|stride_x| {
                    self.call_conv2d_valid_f32(input, kernel, stride_y, stride_x)
                        .map(RuntimeValue::tensor_f32)
                })
            }),
            (RuntimeAcceleratorExternalCall::InferReluF32, [RuntimeValue::TensorF32(input)]) => {
                self.call_infer_relu_f32(input)
                    .map(RuntimeValue::tensor_f32)
            }
            (
                RuntimeAcceleratorExternalCall::InferMaxPool2dF32,
                [
                    RuntimeValue::TensorF32(input),
                    kernel_y,
                    kernel_x,
                    stride_y,
                    stride_x,
                ],
            ) => runtime_value_to_usize(call, kernel_y).and_then(|kernel_y| {
                runtime_value_to_usize(call, kernel_x).and_then(|kernel_x| {
                    runtime_value_to_usize(call, stride_y).and_then(|stride_y| {
                        runtime_value_to_usize(call, stride_x).and_then(|stride_x| {
                            self.call_infer_max_pool2d_f32(
                                input, kernel_y, kernel_x, stride_y, stride_x,
                            )
                            .map(RuntimeValue::tensor_f32)
                        })
                    })
                })
            }),
            (
                RuntimeAcceleratorExternalCall::InferSoftmaxLastDimF32,
                [RuntimeValue::TensorF32(input)],
            ) => self
                .call_infer_softmax_last_dim_f32(input)
                .map(RuntimeValue::tensor_f32),
            (
                RuntimeAcceleratorExternalCall::InferArgmaxLastDimF32,
                [RuntimeValue::TensorF32(input)],
            ) => Ok(runtime_class_indices_value(
                self.call_infer_argmax_last_dim_f32(input),
            )),
            (
                RuntimeAcceleratorExternalCall::InferFlattenOuterF32,
                [RuntimeValue::TensorF32(input)],
            ) => self
                .call_infer_flatten_outer_f32(input)
                .map(RuntimeValue::tensor_f32),
            _ => Err(RuntimeEvalError::UnsupportedPure {
                name: call.label().to_owned(),
                reason: "argument shape is not supported by this adapter call".to_owned(),
            }),
        })
    }
}

fn runtime_class_indices_value(indices: Vec<usize>) -> RuntimeValue {
    runtime_sequence_dense_usize(
        indices
            .into_iter()
            .map(|index| u64::try_from(index).unwrap_or(u64::MAX))
            .collect(),
    )
}

fn runtime_value_to_usize(
    call: RuntimeAcceleratorExternalCall,
    value: &RuntimeValue,
) -> Result<usize, RuntimeEvalError> {
    match value {
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .and_then(|value| usize::try_from(value).ok()),
        RuntimeValue::UInt(value) => value
            .try_into_i64()
            .and_then(|value| usize::try_from(value).ok()),
        _ => None,
    }
    .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
        name: call.label().to_owned(),
        reason: format!("expected usize-compatible integer, got {value:?}"),
    })
}

pub(super) fn infer_runtime_error(name: &str, error: impl fmt::Display) -> RuntimeEvalError {
    RuntimeEvalError::UnsupportedPure {
        name: name.to_owned(),
        reason: error.to_string(),
    }
}

impl RuntimePureAccelerator {
    pub(super) fn cache_entries(&self) -> usize {
        self.cache.iter().filter(|entry| entry.is_some()).count()
    }

    pub(super) fn call_runtime_math_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, math::RuntimeMathAcceleratorError> {
        let selection = self.math.matmul_backend_selection(lhs, rhs);
        if selection.backend() != math::RuntimeMathBackend::Wgpu {
            return self.math.matmul_f32(lhs, rhs);
        }
        if lhs.cols() != rhs.rows() {
            return self.math.matmul_f32(lhs, rhs);
        }
        self.math.record_backend_selection(selection);
        let signature = MatrixBinaryShapeSignature::new(lhs, rhs);
        if let Some(cache) = self.math_prepare_cache.matmul.take()
            && cache.capacity_signature.contains(&signature)
        {
            let mut cache = cache;
            if cache.signature != signature || !cache.value_signature.matches(lhs, rhs) {
                self.math
                    .update_prepared_matrix_matmul_f32(&cache.prepared, lhs, rhs)?;
                cache.signature = signature;
                cache.value_signature.update(lhs, rhs);
            }
            let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
            self.math.run_prepared_matrix_matmul_f32_shape_into(
                &cache.prepared,
                lhs.rows(),
                rhs.cols(),
                &mut out,
            )?;
            let result = DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into);
            self.math_prepare_cache.matmul = Some(cache);
            return result;
        }
        let capacity_signature = MatrixBinaryShapeSignature::capacity_for_matmul(lhs, rhs);
        let prepared = self.math.prepare_matrix_matmul_f32_capacity(
            capacity_signature.lhs.rows,
            capacity_signature.lhs.cols,
            capacity_signature.rhs.cols,
        )?;
        self.math
            .update_prepared_matrix_matmul_f32(&prepared, lhs, rhs)?;
        let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
        self.math.run_prepared_matrix_matmul_f32_shape_into(
            &prepared,
            lhs.rows(),
            rhs.cols(),
            &mut out,
        )?;
        let result = DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into);
        self.math_prepare_cache.matmul = Some(PreparedMatrixMatmulCache {
            signature,
            capacity_signature,
            value_signature: MatrixBinaryValueSignature::new(lhs, rhs),
            prepared,
        });
        result
    }

    pub(super) fn call_runtime_math_matmul_bias_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseMatrixF32, math::RuntimeMathAcceleratorError> {
        let selection = self.math.matmul_backend_selection(lhs, rhs);
        if selection.backend() != math::RuntimeMathBackend::Wgpu {
            return self.math.matmul_bias_add_f32(lhs, rhs, bias);
        }
        if lhs.cols() != rhs.rows() || bias.shape().dims() != [rhs.cols()] {
            return self.math.matmul_bias_add_f32(lhs, rhs, bias);
        }
        self.math.record_backend_selection(selection);
        let signature = MatrixMatmulBiasShapeSignature::new(lhs, rhs, bias);
        if let Some(cache) = self.math_prepare_cache.matmul_bias_add.take()
            && cache.capacity_signature.contains(&signature)
        {
            let mut cache = cache;
            if cache.signature != signature || !cache.value_signature.matches(lhs, rhs, bias) {
                self.math.update_prepared_matrix_matmul_bias_add_f32(
                    &cache.prepared,
                    lhs,
                    rhs,
                    bias,
                )?;
                cache.signature = signature;
                cache.value_signature.update(lhs, rhs, bias);
            }
            let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
            self.math
                .run_prepared_matrix_matmul_bias_add_f32_shape_into(
                    &cache.prepared,
                    lhs.rows(),
                    rhs.cols(),
                    &mut out,
                )?;
            let result = DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into);
            self.math_prepare_cache.matmul_bias_add = Some(cache);
            return result;
        }
        let capacity_signature = MatrixMatmulBiasShapeSignature::capacity_for(lhs, rhs, bias);
        let prepared = self.math.prepare_matrix_matmul_bias_add_f32_capacity(
            capacity_signature.lhs.rows,
            capacity_signature.lhs.cols,
            capacity_signature.rhs.cols,
        )?;
        self.math
            .update_prepared_matrix_matmul_bias_add_f32(&prepared, lhs, rhs, bias)?;
        let mut out = vec![0.0; lhs.rows().saturating_mul(rhs.cols())];
        self.math
            .run_prepared_matrix_matmul_bias_add_f32_shape_into(
                &prepared,
                lhs.rows(),
                rhs.cols(),
                &mut out,
            )?;
        let result = DenseMatrixF32::new(lhs.rows(), rhs.cols(), out).map_err(Into::into);
        self.math_prepare_cache.matmul_bias_add = Some(PreparedMatrixMatmulBiasAddCache {
            signature,
            capacity_signature,
            value_signature: MatrixMatmulBiasValueSignature::new(lhs, rhs, bias),
            prepared,
        });
        result
    }

    pub(super) fn call_runtime_math_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, math::RuntimeMathAcceleratorError> {
        let selection = self.math.elementwise_backend_selection(lhs.values().len());
        if selection.backend() != math::RuntimeMathBackend::Wgpu {
            return self.math.matrix_add_f32(lhs, rhs);
        }
        if lhs.shape() != rhs.shape() {
            return self.math.matrix_add_f32(lhs, rhs);
        }
        self.math.record_backend_selection(selection);
        let signature = MatrixBinaryShapeSignature::new(lhs, rhs);
        if let Some(cache) = self.math_prepare_cache.matrix_add.take()
            && cache.capacity_signature.contains(&signature)
        {
            let mut cache = cache;
            if cache.signature != signature || !cache.value_signature.matches(lhs, rhs) {
                self.math
                    .update_prepared_matrix_add_f32(&cache.prepared, lhs, rhs)?;
                cache.signature = signature;
                cache.value_signature.update(lhs, rhs);
            }
            let mut out = vec![0.0; lhs.values().len()];
            self.math.run_prepared_matrix_add_f32_shape_into(
                &cache.prepared,
                lhs.rows(),
                lhs.cols(),
                &mut out,
            )?;
            let result = DenseMatrixF32::new(lhs.rows(), lhs.cols(), out).map_err(Into::into);
            self.math_prepare_cache.matrix_add = Some(cache);
            return result;
        }
        let capacity_signature = MatrixBinaryShapeSignature::capacity_for_matrix_add(lhs, rhs);
        let prepared = self.math.prepare_matrix_add_f32_capacity(
            capacity_signature.lhs.rows,
            capacity_signature.lhs.cols,
        )?;
        self.math
            .update_prepared_matrix_add_f32(&prepared, lhs, rhs)?;
        let mut out = vec![0.0; lhs.values().len()];
        self.math.run_prepared_matrix_add_f32_shape_into(
            &prepared,
            lhs.rows(),
            lhs.cols(),
            &mut out,
        )?;
        let result = DenseMatrixF32::new(lhs.rows(), lhs.cols(), out).map_err(Into::into);
        self.math_prepare_cache.matrix_add = Some(PreparedMatrixAddCache {
            signature,
            capacity_signature,
            value_signature: MatrixBinaryValueSignature::new(lhs, rhs),
            prepared,
        });
        result
    }

    pub(super) fn call_runtime_math_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, math::RuntimeMathAcceleratorError> {
        let selection = self.math.elementwise_backend_selection(lhs.values().len());
        if selection.backend() != math::RuntimeMathBackend::Wgpu {
            return self.math.tensor_add_f32(lhs, rhs);
        }
        if lhs.shape() != rhs.shape() {
            return self.math.tensor_add_f32(lhs, rhs);
        }
        self.math.record_backend_selection(selection);
        let signature = TensorBinaryShapeSignature::new(lhs, rhs);
        if let Some(cache) = self.math_prepare_cache.tensor_add.take()
            && cache.capacity_signature.contains(&signature)
        {
            let mut cache = cache;
            if cache.signature != signature || !cache.value_signature.matches(lhs, rhs) {
                self.math
                    .update_prepared_tensor_add_f32(&cache.prepared, lhs, rhs)?;
                cache.signature = signature;
                cache.value_signature.update(lhs, rhs);
            }
            let mut out = vec![0.0; lhs.values().len()];
            self.math.run_prepared_tensor_add_f32_len_into(
                &cache.prepared,
                lhs.values().len(),
                &mut out,
            )?;
            let result = DenseTensorF32::new(lhs.shape().dims().to_vec(), out).map_err(Into::into);
            self.math_prepare_cache.tensor_add = Some(cache);
            return result;
        }
        let capacity_signature = TensorBinaryShapeSignature::capacity_for_add(lhs, rhs);
        let prepared = self
            .math
            .prepare_tensor_add_f32_capacity(capacity_signature.lhs.element_count())?;
        self.math
            .update_prepared_tensor_add_f32(&prepared, lhs, rhs)?;
        let mut out = vec![0.0; lhs.values().len()];
        self.math
            .run_prepared_tensor_add_f32_len_into(&prepared, lhs.values().len(), &mut out)?;
        let result = DenseTensorF32::new(lhs.shape().dims().to_vec(), out).map_err(Into::into);
        self.math_prepare_cache.tensor_add = Some(PreparedTensorAddCache {
            signature,
            capacity_signature,
            value_signature: TensorBinaryValueSignature::new(lhs, rhs),
            prepared,
        });
        result
    }

    pub(super) fn record_math_inputs<T>(&mut self, lhs_elements: usize, rhs_elements: usize) {
        self.stats.arg_bytes_borrowed +=
            lhs_elements.saturating_add(rhs_elements) * std::mem::size_of::<T>();
    }

    pub(super) fn record_math_result<T>(&mut self, result_elements: usize) {
        self.stats.result_bytes_copied += result_elements * std::mem::size_of::<T>();
        if !matches!(
            self.math.stats().last_backend,
            Some(math::RuntimeMathBackend::Scalar) | None
        ) {
            self.stats.math_accelerated_calls += 1;
        }
    }

    pub(super) fn call_vm_i64(
        helper: RuntimePureHelperRef<'_>,
        args: RuntimeI64Args,
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<i64, RuntimeEvalError> {
        match scratch.evaluate_i64_args(helper.plan(), helper.id(), args)? {
            value @ RuntimeValue::Int(_) => exact_i64_result(value),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }

    pub(super) fn call_vm_i64_slice(
        helper: RuntimePureHelperRef<'_>,
        args: &[i64],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<i64, RuntimeEvalError> {
        match scratch.evaluate_i64_slice(helper.plan(), helper.id(), args)? {
            value @ RuntimeValue::Int(_) => exact_i64_result(value),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }

    pub(super) fn call_vm_i32_slice(
        helper: RuntimePureHelperRef<'_>,
        args: &[i32],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<i32, RuntimeEvalError> {
        match scratch.evaluate_i32_slice(helper.plan(), helper.id(), args)? {
            RuntimeValue::Int(value) => {
                value
                    .exact_i32()
                    .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: format!("pure i32 result `{value}` is outside i32 range"),
                    })
            }
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }

    pub(super) fn call_vm_f32_slice(
        helper: RuntimePureHelperRef<'_>,
        args: &[f32],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<f32, RuntimeEvalError> {
        match scratch.evaluate_f32_slice(helper.plan(), helper.id(), args)? {
            RuntimeValue::F32(value) => Ok(value),
            value => Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure f32 result expected f32, got {}",
                    runtime_value_kind(&value)
                ),
            }),
        }
    }

    pub(super) fn call_vm_f64_slice(
        helper: RuntimePureHelperRef<'_>,
        args: &[f64],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<f64, RuntimeEvalError> {
        match scratch.evaluate_f64_slice(helper.plan(), helper.id(), args)? {
            RuntimeValue::F64(value) => Ok(value),
            value => Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure f64 result expected f64, got {}",
                    runtime_value_kind(&value)
                ),
            }),
        }
    }
}
