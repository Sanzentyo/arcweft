use std::collections::BTreeMap;

use arcweft_agent_protocol::{
    ids::PublicId,
    predicate::{CompareOp, DebugStatePath, ObservationFieldPath, Predicate, Probe},
    protocol::{CaptureFormat, CaptureTarget},
    value::AgentValue,
};
use arcweft_core::value::{
    RuntimeAgentCaptureTarget, RuntimeAgentCompareOp, RuntimeAgentPredicate, RuntimeAgentProbe,
    RuntimeAgentValue, RuntimePayload, RuntimeValue,
};

use crate::label_parse::{
    parse_bool_label, parse_capture_format, parse_public_id_arg, parse_public_id_list,
};

pub(crate) fn runtime_value_to_json(value: &RuntimeValue) -> serde_json::Value {
    match value {
        RuntimeValue::Unit => serde_json::Value::Null,
        RuntimeValue::Bool(value) => serde_json::Value::Bool(*value),
        RuntimeValue::Int(value) => runtime_int_to_json(*value),
        RuntimeValue::UInt(value) => runtime_uint_to_json(*value),
        RuntimeValue::F32(value) => serde_json::json!(*value),
        RuntimeValue::F64(value) => serde_json::json!(*value),
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => {
            serde_json::Value::String(value.clone())
        }
        RuntimeValue::Char(value) => serde_json::Value::String(value.to_string()),
        RuntimeValue::Tuple(values) => {
            serde_json::Value::Array(values.iter().map(runtime_value_to_json).collect())
        }
        RuntimeValue::Seq(values) => {
            serde_json::to_value(values).unwrap_or(serde_json::Value::Null)
        }
        RuntimeValue::Record(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|field| {
                    (
                        field.name().to_owned(),
                        runtime_value_to_json(field.value()),
                    )
                })
                .collect(),
        ),
        RuntimeValue::NominalRecord(record) => serde_json::json!({
            "kind": "nominal_record",
            "type": record.type_id().as_str(),
            "layout": record.layout(),
            "fields": record
                .fields()
                .iter()
                .map(runtime_value_to_json)
                .collect::<Vec<_>>(),
        }),
        RuntimeValue::Opaque(value) => runtime_value_to_json(value.payload()),
        RuntimeValue::Agent(value) => runtime_agent_to_json(value),
        RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            payload,
        } => serde_json::json!({
            "owner": owner,
            "ordinal": ordinal,
            "name": name,
            "payload": payload.as_deref().map(runtime_value_to_json),
        }),
        RuntimeValue::Range(range) => {
            serde_json::to_value(range).unwrap_or(serde_json::Value::Null)
        }
        RuntimeValue::Iterator(_) => serde_json::json!({
            "kind": "runtime_internal",
            "value": "iterator",
        }),
        RuntimeValue::Reduction(_) => serde_json::json!({
            "kind": "runtime_internal",
            "value": "reduction",
        }),
        RuntimeValue::Function(function) => serde_json::json!({
            "kind": "runtime_internal",
            "value": "function",
            "arity": function.remaining_arity().ok(),
        }),
        RuntimeValue::Duration(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_) => {
            serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
        }
    }
}

fn runtime_agent_to_json(value: &RuntimeAgentValue) -> serde_json::Value {
    match value {
        RuntimeAgentValue::ActionTarget(target) => serde_json::json!({
            "id": target.id().as_str(),
            "target": target.target().as_str(),
            "action": target.action().as_label(),
            "kind": target.dispatch().as_label(),
            "enabled": target.enabled(),
        }),
        RuntimeAgentValue::CaptureTarget(RuntimeAgentCaptureTarget::Viewport) => {
            serde_json::json!({ "kind": "viewport" })
        }
        RuntimeAgentValue::CaptureTarget(RuntimeAgentCaptureTarget::Layer { target }) => {
            serde_json::json!({ "kind": "layer", "target": target.as_str() })
        }
        RuntimeAgentValue::CaptureTarget(RuntimeAgentCaptureTarget::Object { target }) => {
            serde_json::json!({ "kind": "object", "target": target.as_str() })
        }
        RuntimeAgentValue::DebugStatePath(path) => serde_json::json!({
            "kind": "state_path",
            "path": path.as_str(),
        }),
        RuntimeAgentValue::ObservationFieldPath(path) => serde_json::json!({
            "kind": "observation_field",
            "path": path.as_str(),
        }),
        RuntimeAgentValue::Probe(probe) => runtime_agent_probe_to_json(probe),
        RuntimeAgentValue::Diagnostics => serde_json::json!({ "kind": "diagnostics" }),
        RuntimeAgentValue::Predicate(predicate) => runtime_agent_predicate_to_json(predicate),
        RuntimeAgentValue::ViewportPoint { x, y } => serde_json::json!({ "x": x, "y": y }),
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

fn runtime_agent_predicate_to_json(predicate: &RuntimeAgentPredicate) -> serde_json::Value {
    match predicate {
        RuntimeAgentPredicate::Compare { probe, op, value } => serde_json::json!({
            "kind": "compare",
            "probe": runtime_agent_probe_to_json(probe),
            "op": op.as_label(),
            "value": runtime_value_to_json(value),
        }),
        RuntimeAgentPredicate::Exists { probe } => serde_json::json!({
            "kind": "exists",
            "probe": runtime_agent_probe_to_json(probe),
        }),
        RuntimeAgentPredicate::ActionEnabled { target } => serde_json::json!({
            "kind": "action_enabled",
            "target": target.as_str(),
        }),
        RuntimeAgentPredicate::DiagnosticsHasError => {
            serde_json::json!({ "kind": "diagnostics_has_error" })
        }
        RuntimeAgentPredicate::All { predicates } => serde_json::json!({
            "kind": "all",
            "predicates": predicates
                .iter()
                .map(runtime_agent_predicate_to_json)
                .collect::<Vec<_>>(),
        }),
        RuntimeAgentPredicate::Any { predicates } => serde_json::json!({
            "kind": "any",
            "predicates": predicates
                .iter()
                .map(runtime_agent_predicate_to_json)
                .collect::<Vec<_>>(),
        }),
        RuntimeAgentPredicate::Not { predicate } => serde_json::json!({
            "kind": "not",
            "predicate": runtime_agent_predicate_to_json(predicate),
        }),
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
            .map(|predicates| Predicate::All { predicates }),
        RuntimeAgentPredicate::Any { predicates } => predicates
            .iter()
            .map(protocol_predicate)
            .collect::<Result<Vec<_>, _>>()
            .map(|predicates| Predicate::Any { predicates }),
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
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => Ok(value.clone()),
        RuntimeValue::Variant { name, .. } => Ok(name.clone()),
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
        RuntimeValue::EntityRef(value) => parse_public_id_arg(value).map(AgentValue::Entity),
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
