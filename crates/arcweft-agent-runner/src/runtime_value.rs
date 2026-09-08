use std::collections::BTreeMap;

use arcweft_agent_protocol::{
    ids::PublicId,
    predicate::{CompareOp, DebugStatePath, ObservationFieldPath, Predicate, Probe},
    protocol::{CaptureFormat, CaptureTarget},
    value::AgentValue,
};
use arcweft_core::value::{
    DenseSeq, RuntimeAgentCaptureTarget, RuntimeAgentCompareOp, RuntimeAgentPredicate,
    RuntimeAgentProbe, RuntimeAgentValue, RuntimePayload, RuntimeSeq, RuntimeValue,
};

use crate::error::AgentRuntimeValueSerializationError;
use crate::label_parse::{
    parse_bool_label, parse_capture_format, parse_public_id_arg, parse_public_id_list,
};

pub(crate) fn runtime_value_to_json(
    value: &RuntimeValue,
) -> Result<serde_json::Value, AgentRuntimeValueSerializationError> {
    ensure_finite_runtime_value(value, "$runtime")?;
    runtime_value_to_json_at(value, "$runtime")
}

fn runtime_value_to_json_at(
    value: &RuntimeValue,
    path: &str,
) -> Result<serde_json::Value, AgentRuntimeValueSerializationError> {
    match value {
        RuntimeValue::Unit => Ok(serde_json::Value::Null),
        RuntimeValue::Bool(value) => Ok(serde_json::Value::Bool(*value)),
        RuntimeValue::Int(value) => Ok(runtime_int_to_json(*value)),
        RuntimeValue::UInt(value) => Ok(runtime_uint_to_json(*value)),
        RuntimeValue::F32(value) => finite_json_number(f64::from(*value), path),
        RuntimeValue::F64(value) => finite_json_number(*value, path),
        RuntimeValue::String(value) => Ok(serde_json::Value::String(value.clone())),
        RuntimeValue::EntityRef(value) => Ok(serde_json::Value::String(value.runtime_label())),
        RuntimeValue::Char(value) => Ok(serde_json::Value::String(value.to_string())),
        RuntimeValue::Tuple(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| runtime_value_to_json_at(value, &format!("{path}[{index}]")))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        RuntimeValue::Seq(values) => serialize_runtime_json(values, path),
        RuntimeValue::Record(fields) => fields
            .iter()
            .map(|field| {
                runtime_value_to_json_at(field.value(), &format!("{path}.{}", field.name()))
                    .map(|value| (field.name().to_owned(), value))
            })
            .collect::<Result<serde_json::Map<_, _>, _>>()
            .map(serde_json::Value::Object),
        RuntimeValue::NominalRecord(record) => {
            let fields = record
                .fields()
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    runtime_value_to_json_at(value, &format!("{path}.fields[{index}]"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(serde_json::json!({
                "kind": "nominal_record",
                "type": record.type_id().as_str(),
                "layout": serialize_runtime_json(&record.layout(), &format!("{path}.layout"))?,
                "fields": fields,
            }))
        }
        RuntimeValue::Opaque(value) => {
            runtime_value_to_json_at(value.payload(), &format!("{path}.payload"))
        }
        RuntimeValue::Agent(value) => runtime_agent_to_json(value, path),
        RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            payload,
        } => {
            let payload = payload
                .as_deref()
                .map(|payload| runtime_value_to_json_at(payload, &format!("{path}.payload")))
                .transpose()?;
            Ok(serde_json::json!({
                "owner": owner,
                "ordinal": ordinal,
                "name": name,
                "payload": payload,
            }))
        }
        RuntimeValue::Range(range) => serialize_runtime_json(range, path),
        RuntimeValue::Iterator(_) => Ok(serde_json::json!({
            "kind": "runtime_internal",
            "value": "iterator",
        })),
        RuntimeValue::Reduction(_) => Ok(serde_json::json!({
            "kind": "runtime_internal",
            "value": "reduction",
        })),
        RuntimeValue::Function(function) => function
            .remaining_arity()
            .map(|arity| {
                serde_json::json!({
                    "kind": "runtime_internal",
                    "value": "function",
                    "arity": arity,
                })
            })
            .map_err(
                |error| AgentRuntimeValueSerializationError::InvalidRuntimeState {
                    path: path.to_owned(),
                    detail: error.to_string(),
                },
            ),
        RuntimeValue::ProjectContinuation(continuation) => Ok(serde_json::json!({
            "kind": "runtime_internal",
            "value": "project_continuation",
            "prefix_count": continuation.prefix_values().len(),
        })),
        RuntimeValue::Duration(_)
        | RuntimeValue::Progress(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_) => serialize_runtime_json(value, path),
    }
}

fn serialize_runtime_json<T: serde::Serialize>(
    value: &T,
    path: &str,
) -> Result<serde_json::Value, AgentRuntimeValueSerializationError> {
    serde_json::to_value(value).map_err(|source| {
        AgentRuntimeValueSerializationError::JsonSerialization {
            path: path.to_owned(),
            source,
        }
    })
}

fn finite_json_number(
    value: f64,
    path: &str,
) -> Result<serde_json::Value, AgentRuntimeValueSerializationError> {
    serde_json::Number::from_f64(value)
        .map(serde_json::Value::Number)
        .ok_or_else(|| AgentRuntimeValueSerializationError::NonFiniteNumber {
            path: path.to_owned(),
            value: value.to_string(),
        })
}

fn ensure_finite_numbers<T>(
    values: &[T],
    path: &str,
) -> Result<(), AgentRuntimeValueSerializationError>
where
    T: Copy + Into<f64>,
{
    values.iter().enumerate().try_for_each(|(index, value)| {
        let value = (*value).into();
        value.is_finite().then_some(()).ok_or_else(|| {
            AgentRuntimeValueSerializationError::NonFiniteNumber {
                path: format!("{path}.values[{index}]"),
                value: value.to_string(),
            }
        })
    })
}

fn ensure_finite_runtime_sequence(
    sequence: &RuntimeSeq,
    path: &str,
) -> Result<(), AgentRuntimeValueSerializationError> {
    match sequence {
        RuntimeSeq::Values(values) => values.iter().enumerate().try_for_each(|(index, value)| {
            ensure_finite_runtime_value(value, &format!("{path}.values[{index}]"))
        }),
        RuntimeSeq::Dense(DenseSeq::F32(values)) => ensure_finite_numbers(values.as_slice(), path),
        RuntimeSeq::Dense(DenseSeq::F64(values)) => ensure_finite_numbers(values.as_slice(), path),
        RuntimeSeq::Dense(_) => Ok(()),
        RuntimeSeq::TupleColumns(columns) => {
            columns
                .columns()
                .iter()
                .enumerate()
                .try_for_each(|(index, column)| {
                    ensure_finite_runtime_sequence(column, &format!("{path}.columns[{index}]"))
                })
        }
        RuntimeSeq::RecordColumns(records) => records.fields().iter().try_for_each(|field| {
            ensure_finite_runtime_sequence(
                field.values(),
                &format!("{path}.fields.{}", field.name()),
            )
        }),
    }
}

fn ensure_finite_runtime_value(
    value: &RuntimeValue,
    path: &str,
) -> Result<(), AgentRuntimeValueSerializationError> {
    match value {
        RuntimeValue::F32(value) => finite_json_number(f64::from(*value), path).map(|_| ()),
        RuntimeValue::F64(value) => finite_json_number(*value, path).map(|_| ()),
        RuntimeValue::Tuple(values) => values.iter().enumerate().try_for_each(|(index, value)| {
            ensure_finite_runtime_value(value, &format!("{path}[{index}]"))
        }),
        RuntimeValue::Seq(sequence) => ensure_finite_runtime_sequence(sequence, path),
        RuntimeValue::Record(fields) => fields.iter().try_for_each(|field| {
            ensure_finite_runtime_value(field.value(), &format!("{path}.{}", field.name()))
        }),
        RuntimeValue::NominalRecord(record) => {
            record
                .fields()
                .iter()
                .enumerate()
                .try_for_each(|(index, value)| {
                    ensure_finite_runtime_value(value, &format!("{path}.fields[{index}]"))
                })
        }
        RuntimeValue::Opaque(value) => {
            ensure_finite_runtime_value(value.payload(), &format!("{path}.payload"))
        }
        RuntimeValue::Agent(RuntimeAgentValue::Predicate(predicate)) => {
            ensure_finite_runtime_predicate(predicate, path)
        }
        RuntimeValue::Variant {
            payload: Some(payload),
            ..
        } => ensure_finite_runtime_value(payload, &format!("{path}.payload")),
        RuntimeValue::ProjectContinuation(continuation) => continuation
            .prefix_values()
            .iter()
            .enumerate()
            .try_for_each(|(index, value)| {
                ensure_finite_runtime_value(value, &format!("{path}.prefix_values[{index}]"))
            }),
        RuntimeValue::MatrixF32(value) => ensure_finite_numbers(value.values(), path),
        RuntimeValue::MatrixF64(value) => ensure_finite_numbers(value.values(), path),
        RuntimeValue::TensorF32(value) => ensure_finite_numbers(value.values(), path),
        RuntimeValue::TensorF64(value) => ensure_finite_numbers(value.values(), path),
        RuntimeValue::Unit
        | RuntimeValue::Bool(_)
        | RuntimeValue::Int(_)
        | RuntimeValue::UInt(_)
        | RuntimeValue::String(_)
        | RuntimeValue::Char(_)
        | RuntimeValue::EntityRef(_)
        | RuntimeValue::Range(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::Reduction(_)
        | RuntimeValue::Function(_)
        | RuntimeValue::Duration(_)
        | RuntimeValue::Progress(_)
        | RuntimeValue::Agent(_)
        | RuntimeValue::Variant { payload: None, .. } => Ok(()),
    }
}

fn ensure_finite_runtime_predicate(
    predicate: &RuntimeAgentPredicate,
    path: &str,
) -> Result<(), AgentRuntimeValueSerializationError> {
    match predicate {
        RuntimeAgentPredicate::Compare { value, .. } => {
            ensure_finite_runtime_value(value, &format!("{path}.value"))
        }
        RuntimeAgentPredicate::All { predicates } | RuntimeAgentPredicate::Any { predicates } => {
            predicates
                .iter()
                .enumerate()
                .try_for_each(|(index, predicate)| {
                    ensure_finite_runtime_predicate(
                        predicate,
                        &format!("{path}.predicates[{index}]"),
                    )
                })
        }
        RuntimeAgentPredicate::Not { predicate } => {
            ensure_finite_runtime_predicate(predicate, &format!("{path}.predicate"))
        }
        RuntimeAgentPredicate::Exists { .. }
        | RuntimeAgentPredicate::ActionEnabled { .. }
        | RuntimeAgentPredicate::DiagnosticsHasError => Ok(()),
    }
}

fn runtime_agent_to_json(
    value: &RuntimeAgentValue,
    path: &str,
) -> Result<serde_json::Value, AgentRuntimeValueSerializationError> {
    match value {
        RuntimeAgentValue::ActionTarget(target) => Ok(serde_json::json!({
            "id": target.id().as_str(),
            "target": target.target().as_str(),
            "action": target.action().as_label(),
            "kind": target.dispatch().as_label(),
            "enabled": target.enabled(),
        })),
        RuntimeAgentValue::CaptureTarget(RuntimeAgentCaptureTarget::Viewport) => {
            Ok(serde_json::json!({ "kind": "viewport" }))
        }
        RuntimeAgentValue::CaptureTarget(RuntimeAgentCaptureTarget::Layer { target }) => {
            Ok(serde_json::json!({ "kind": "layer", "target": target.as_str() }))
        }
        RuntimeAgentValue::CaptureTarget(RuntimeAgentCaptureTarget::Object { target }) => {
            Ok(serde_json::json!({ "kind": "object", "target": target.as_str() }))
        }
        RuntimeAgentValue::DebugStatePath(value) => Ok(serde_json::json!({
            "kind": "state_path",
            "path": value.as_str(),
        })),
        RuntimeAgentValue::ObservationFieldPath(value) => Ok(serde_json::json!({
            "kind": "observation_field",
            "path": value.as_str(),
        })),
        RuntimeAgentValue::Probe(probe) => Ok(runtime_agent_probe_to_json(probe)),
        RuntimeAgentValue::Diagnostics => Ok(serde_json::json!({ "kind": "diagnostics" })),
        RuntimeAgentValue::Predicate(predicate) => runtime_agent_predicate_to_json(predicate, path),
        RuntimeAgentValue::ViewportPoint { x, y } => Ok(serde_json::json!({ "x": x, "y": y })),
        RuntimeAgentValue::BinaryData(data) => Ok(serde_json::json!({ "data": data })),
    }
}

fn runtime_agent_probe_to_json(probe: &RuntimeAgentProbe) -> serde_json::Value {
    match probe {
        RuntimeAgentProbe::Signal { target } => {
            serde_json::json!({ "kind": "signal", "target": target.as_str() })
        }
        RuntimeAgentProbe::Metric { target } => {
            serde_json::json!({ "kind": "metric", "target": target.as_str() })
        }
        RuntimeAgentProbe::StatePath { path } => {
            serde_json::json!({ "kind": "state", "path": path.as_str() })
        }
        RuntimeAgentProbe::ObservationField { path } => {
            serde_json::json!({ "kind": "observation", "path": path.as_str() })
        }
    }
}

fn runtime_agent_predicate_to_json(
    predicate: &RuntimeAgentPredicate,
    path: &str,
) -> Result<serde_json::Value, AgentRuntimeValueSerializationError> {
    match predicate {
        RuntimeAgentPredicate::Compare { probe, op, value } => Ok(serde_json::json!({
            "kind": "compare",
            "probe": runtime_agent_probe_to_json(probe),
            "op": op.as_label(),
            "value": runtime_value_to_json_at(value, &format!("{path}.value"))?,
        })),
        RuntimeAgentPredicate::Exists { probe } => Ok(serde_json::json!({
            "kind": "exists",
            "probe": runtime_agent_probe_to_json(probe),
        })),
        RuntimeAgentPredicate::ActionEnabled { target } => Ok(serde_json::json!({
            "kind": "action_enabled",
            "target": target.as_str(),
        })),
        RuntimeAgentPredicate::DiagnosticsHasError => {
            Ok(serde_json::json!({ "kind": "diagnostics_has_error" }))
        }
        RuntimeAgentPredicate::All { predicates } => predicates
            .iter()
            .enumerate()
            .map(|(index, predicate)| {
                runtime_agent_predicate_to_json(predicate, &format!("{path}.predicates[{index}]"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|predicates| serde_json::json!({ "kind": "all", "predicates": predicates })),
        RuntimeAgentPredicate::Any { predicates } => predicates
            .iter()
            .enumerate()
            .map(|(index, predicate)| {
                runtime_agent_predicate_to_json(predicate, &format!("{path}.predicates[{index}]"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|predicates| serde_json::json!({ "kind": "any", "predicates": predicates })),
        RuntimeAgentPredicate::Not { predicate } => Ok(serde_json::json!({
            "kind": "not",
            "predicate": runtime_agent_predicate_to_json(predicate, &format!("{path}.predicate"))?,
        })),
    }
}

fn runtime_int_to_json(value: arcweft_core::value::RuntimeInt) -> serde_json::Value {
    match value {
        arcweft_core::value::RuntimeInt::I8(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeInt::I16(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeInt::I32(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeInt::I64(value)
        | arcweft_core::value::RuntimeInt::ISize(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeInt::I128(value) => i64::try_from(value).map_or_else(
            |_| serde_json::json!(value.to_string()),
            |value| serde_json::json!(value),
        ),
    }
}

fn runtime_uint_to_json(value: arcweft_core::value::RuntimeUInt) -> serde_json::Value {
    match value {
        arcweft_core::value::RuntimeUInt::U8(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeUInt::U16(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeUInt::U32(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeUInt::U64(value)
        | arcweft_core::value::RuntimeUInt::USize(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeUInt::U128(value) => u64::try_from(value).map_or_else(
            |_| serde_json::json!(value.to_string()),
            |value| serde_json::json!(value),
        ),
    }
}

pub(crate) fn runtime_predicate(value: &RuntimeValue) -> Result<Predicate, String> {
    let RuntimeValue::Agent(RuntimeAgentValue::Predicate(predicate)) = value else {
        return Err(format!(
            "expected typed Agent predicate, got `{}`",
            value_label(value)
        ));
    };
    protocol_predicate(predicate)
}

fn protocol_predicate(predicate: &RuntimeAgentPredicate) -> Result<Predicate, String> {
    match predicate {
        RuntimeAgentPredicate::Compare { probe, op, value } => Ok(Predicate::Compare {
            probe: protocol_probe(probe)?,
            op: protocol_compare_op(*op),
            value: Box::new(runtime_agent_value(value)?),
        }),
        RuntimeAgentPredicate::Exists { probe } => Ok(Predicate::Exists {
            probe: protocol_probe(probe)?,
        }),
        RuntimeAgentPredicate::ActionEnabled { target } => Ok(Predicate::ActionEnabled {
            target: PublicId::new(target.as_str().to_owned()).map_err(|error| error.to_string())?,
        }),
        RuntimeAgentPredicate::DiagnosticsHasError => Ok(Predicate::DiagnosticsHasError),
        RuntimeAgentPredicate::All { predicates } => predicates
            .iter()
            .map(protocol_predicate)
            .collect::<Result<Vec<_>, _>>()
            .and_then(|predicates| {
                Predicate::try_all(predicates).map_err(|error| error.to_string())
            }),
        RuntimeAgentPredicate::Any { predicates } => predicates
            .iter()
            .map(protocol_predicate)
            .collect::<Result<Vec<_>, _>>()
            .and_then(|predicates| {
                Predicate::try_any(predicates).map_err(|error| error.to_string())
            }),
        RuntimeAgentPredicate::Not { predicate } => {
            protocol_predicate(predicate).map(|predicate| Predicate::Not {
                predicate: Box::new(predicate),
            })
        }
    }
}

fn protocol_probe(probe: &RuntimeAgentProbe) -> Result<Probe, String> {
    match probe {
        RuntimeAgentProbe::Signal { target } => Ok(Probe::Signal {
            target: PublicId::new(target.as_str().to_owned()).map_err(|error| error.to_string())?,
        }),
        RuntimeAgentProbe::Metric { target } => Ok(Probe::Metric {
            target: PublicId::new(target.as_str().to_owned()).map_err(|error| error.to_string())?,
        }),
        RuntimeAgentProbe::StatePath { path } => Ok(Probe::StatePath {
            path: DebugStatePath::new(path.as_str().to_owned())?,
        }),
        RuntimeAgentProbe::ObservationField { path } => Ok(Probe::ObservationField {
            path: ObservationFieldPath::new(path.as_str().to_owned())?,
        }),
    }
}

const fn protocol_compare_op(op: RuntimeAgentCompareOp) -> CompareOp {
    match op {
        RuntimeAgentCompareOp::Eq => CompareOp::Eq,
        RuntimeAgentCompareOp::NotEq => CompareOp::NotEq,
        RuntimeAgentCompareOp::Greater => CompareOp::Greater,
        RuntimeAgentCompareOp::GreaterOrEqual => CompareOp::GreaterOrEqual,
        RuntimeAgentCompareOp::Less => CompareOp::Less,
        RuntimeAgentCompareOp::LessOrEqual => CompareOp::LessOrEqual,
    }
}

pub(crate) fn runtime_field(name: &str, value: RuntimeValue) -> (String, RuntimeValue) {
    (name.to_owned(), value)
}

pub(crate) fn runtime_record(fields: Vec<(String, RuntimeValue)>) -> RuntimeValue {
    RuntimeValue::try_record(fields).expect("agent runtime payload record has fixed unique fields")
}

pub(crate) fn runtime_string(value: &RuntimeValue) -> Result<String, String> {
    match value {
        RuntimeValue::String(value) => Ok(value.clone()),
        RuntimeValue::EntityRef(value) => Ok(value.runtime_label()),
        RuntimeValue::Variant { .. } => value
            .builtin_variant_case()
            .and_then(|(case, _)| {
                case.owner()
                    .resolve_case(case)
                    .map(|(_, schema)| schema.name().to_owned())
            })
            .ok_or_else(|| "expected a canonical builtin variant value".to_owned()),
        other => Err(format!(
            "expected string-like value, got `{}`",
            value_label(other)
        )),
    }
}

pub(crate) fn runtime_bool(value: &RuntimeValue) -> Result<bool, String> {
    match value {
        RuntimeValue::Bool(value) => Ok(*value),
        RuntimeValue::String(value) => parse_bool_label(value),
        other => Err(format!(
            "expected boolean value, got `{}`",
            value_label(other)
        )),
    }
}

pub(crate) fn runtime_u32(value: &RuntimeValue) -> Result<u32, String> {
    match value {
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| format!("expected u32-compatible integer, got `{}`", value.label())),
        RuntimeValue::UInt(value) => value
            .try_into_u32()
            .ok_or_else(|| format!("expected u32-compatible integer, got `{}`", value.label())),
        RuntimeValue::String(value) => value
            .parse::<u32>()
            .map_err(|_| format!("expected u32-compatible integer, got `{value}`")),
        other => Err(format!(
            "expected integer value, got `{}`",
            value_label(other)
        )),
    }
}

pub(crate) fn runtime_usize(value: &RuntimeValue) -> Result<usize, String> {
    match value {
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("expected usize-compatible integer, got `{}`", value.label())),
        RuntimeValue::UInt(value) => value
            .try_into_i64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("expected usize-compatible integer, got `{}`", value.label())),
        RuntimeValue::String(value) => value
            .parse::<usize>()
            .map_err(|_| format!("expected usize-compatible integer, got `{value}`")),
        other => Err(format!(
            "expected integer value, got `{}`",
            value_label(other)
        )),
    }
}

pub(crate) fn runtime_duration_millis(value: &RuntimeValue) -> Result<u64, String> {
    match value {
        RuntimeValue::Duration(duration) => {
            let nanos = duration.as_nanos();
            Ok(if nanos == 0 {
                0
            } else {
                nanos.saturating_add(999_999) / 1_000_000
            })
        }
        RuntimeValue::UInt(value) => value
            .exact_u64()
            .or_else(|| {
                value
                    .try_into_i64()
                    .and_then(|value| u64::try_from(value).ok())
            })
            .ok_or_else(|| format!("expected millisecond duration, got `{}`", value.label())),
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| format!("expected millisecond duration, got `{}`", value.label())),
        RuntimeValue::String(value) => value
            .parse::<u64>()
            .map_err(|_| format!("expected millisecond duration, got `{value}`")),
        other => Err(format!(
            "expected duration value, got `{}`",
            value_label(other)
        )),
    }
}

pub(crate) fn runtime_public_id(value: &RuntimeValue) -> Result<PublicId, String> {
    runtime_string(value).and_then(|value| parse_public_id_arg(&value))
}

pub(crate) fn runtime_public_ids(value: &RuntimeValue) -> Result<Vec<PublicId>, String> {
    match value {
        RuntimeValue::Tuple(values) => values.iter().map(runtime_public_id).collect(),
        RuntimeValue::String(value) => parse_public_id_list(value),
        _ => runtime_public_id(value).map(|id| vec![id]),
    }
}

pub(crate) fn runtime_capture_target(value: &RuntimeValue) -> Result<CaptureTarget, String> {
    let RuntimeValue::Agent(RuntimeAgentValue::CaptureTarget(target)) = value else {
        return Err(format!(
            "expected typed Agent capture target, got `{}`",
            value_label(value)
        ));
    };
    match target {
        RuntimeAgentCaptureTarget::Viewport => Ok(CaptureTarget::Viewport),
        RuntimeAgentCaptureTarget::Layer { target } => PublicId::new(target.as_str().to_owned())
            .map(|id| CaptureTarget::Layer { id })
            .map_err(|error| error.to_string()),
        RuntimeAgentCaptureTarget::Object { target } => Ok(CaptureTarget::Object {
            id: target.as_str().to_owned(),
        }),
    }
}

pub(crate) fn runtime_capture_format(value: &RuntimeValue) -> Result<CaptureFormat, String> {
    runtime_string(value).and_then(|value| parse_capture_format(&value))
}

pub(crate) fn runtime_agent_value_map(
    value: &RuntimeValue,
) -> Result<BTreeMap<String, AgentValue>, String> {
    let RuntimeValue::Record(fields) = value else {
        return Err(format!(
            "expected record for invoke args, got `{}`",
            value_label(value)
        ));
    };
    fields
        .iter()
        .map(|field| {
            runtime_agent_value(field.value()).map(|value| (field.name().to_owned(), value))
        })
        .collect()
}

fn runtime_agent_value(value: &RuntimeValue) -> Result<AgentValue, String> {
    match value {
        RuntimeValue::Unit => Ok(AgentValue::Null),
        RuntimeValue::Bool(value) => Ok(AgentValue::Bool(*value)),
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .map(AgentValue::I64)
            .ok_or_else(|| format!("integer is out of i64 range: `{}`", value.label())),
        RuntimeValue::UInt(value) => value
            .exact_u64()
            .or_else(|| {
                value
                    .try_into_i64()
                    .and_then(|value| u64::try_from(value).ok())
            })
            .map(AgentValue::U64)
            .ok_or_else(|| format!("integer is out of u64 range: `{}`", value.label())),
        RuntimeValue::F32(value) => Ok(AgentValue::F64(f64::from(*value))),
        RuntimeValue::F64(value) => Ok(AgentValue::F64(*value)),
        RuntimeValue::String(value) => Ok(AgentValue::String(value.clone())),
        RuntimeValue::EntityRef(value) => {
            parse_public_id_arg(&value.runtime_label()).map(AgentValue::Entity)
        }
        RuntimeValue::Iterator(_) => Err("runtime iterator state is not an Agent value".to_owned()),
        RuntimeValue::Tuple(values) => values
            .iter()
            .map(runtime_agent_value)
            .collect::<Result<Vec<_>, _>>()
            .map(AgentValue::List),
        RuntimeValue::Record(fields) => fields
            .iter()
            .map(|field| {
                runtime_agent_value(field.value()).map(|value| (field.name().to_owned(), value))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(AgentValue::Map),
        other => Err(format!("unsupported Agent value `{}`", value_label(other))),
    }
}

pub(crate) fn value_label(value: &RuntimeValue) -> String {
    RuntimePayload::new(value.clone()).label()
}
