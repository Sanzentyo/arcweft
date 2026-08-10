use std::collections::BTreeMap;

use arcweft_agent_protocol::{
    ids::PublicId,
    predicate::{CompareOp, DebugStatePath, ObservationFieldPath, Predicate, Probe},
    protocol::{CaptureFormat, CaptureTarget},
    value::AgentValue,
};
use arcweft_core::value::{RuntimeFieldValue, RuntimePayload, RuntimeValue};

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
                .map(|field| (field.name.clone(), runtime_value_to_json(&field.value)))
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
        RuntimeValue::Function(function) => serde_json::json!({
            "kind": "runtime_internal",
            "value": "function",
            "arity": function.arity(),
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
    let fields = runtime_record_fields(value, "predicate")?;
    match runtime_record_string(fields, "kind")?.as_str() {
        "compare" => Ok(Predicate::Compare {
            probe: runtime_record_get(fields, "probe").and_then(runtime_probe)?,
            op: runtime_record_get(fields, "op").and_then(runtime_compare_op)?,
            value: Box::new(runtime_record_get(fields, "value").and_then(runtime_agent_value)?),
        }),
        "exists" => Ok(Predicate::Exists {
            probe: runtime_record_get(fields, "probe").and_then(runtime_probe)?,
        }),
        "action_enabled" => runtime_record_string(fields, "target")
            .and_then(|target| PublicId::new(target).map_err(|error| error.to_string()))
            .map(|target| Predicate::ActionEnabled { target }),
        "all" => runtime_record_get(fields, "predicates")
            .and_then(runtime_predicate_list)
            .map(|predicates| Predicate::All { predicates }),
        "any" => runtime_record_get(fields, "predicates")
            .and_then(runtime_predicate_list)
            .map(|predicates| Predicate::Any { predicates }),
        "not" => runtime_record_get(fields, "predicate")
            .and_then(runtime_predicate)
            .map(|predicate| Predicate::Not {
                predicate: Box::new(predicate),
            }),
        "diagnostics_has_error" => Ok(Predicate::DiagnosticsHasError),
        other => Err(format!("unsupported predicate kind `{other}`")),
    }
}

pub(crate) fn runtime_predicate_list(value: &RuntimeValue) -> Result<Vec<Predicate>, String> {
    let RuntimeValue::Tuple(values) = value else {
        return Err(format!(
            "expected predicate tuple, got `{}`",
            value_label(value)
        ));
    };
    values.iter().map(runtime_predicate).collect()
}

fn runtime_probe(value: &RuntimeValue) -> Result<Probe, String> {
    let fields = runtime_record_fields(value, "probe")?;
    match runtime_record_string(fields, "kind")?.as_str() {
        "signal" => Ok(Probe::Signal {
            target: runtime_record_get(fields, "target").and_then(runtime_public_id)?,
        }),
        "metric" => Ok(Probe::Metric {
            target: runtime_record_get(fields, "target").and_then(runtime_public_id)?,
        }),
        "state" | "state_path" => Ok(Probe::StatePath {
            path: DebugStatePath::new(runtime_record_string(fields, "path")?)?,
        }),
        "observation" | "observation_field" => Ok(Probe::ObservationField {
            path: ObservationFieldPath::new(runtime_record_string(fields, "path")?)?,
        }),
        other => Err(format!("unsupported probe kind `{other}`")),
    }
}

fn runtime_compare_op(value: &RuntimeValue) -> Result<CompareOp, String> {
    match runtime_string(value)?.as_str() {
        "eq" => Ok(CompareOp::Eq),
        "not_eq" | "ne" => Ok(CompareOp::NotEq),
        "greater" | "gt" => Ok(CompareOp::Greater),
        "greater_or_equal" | "ge" => Ok(CompareOp::GreaterOrEqual),
        "less" | "lt" => Ok(CompareOp::Less),
        "less_or_equal" | "le" => Ok(CompareOp::LessOrEqual),
        other => Err(format!("unsupported compare op `{other}`")),
    }
}

fn runtime_record_fields<'a>(
    value: &'a RuntimeValue,
    label: &str,
) -> Result<&'a [RuntimeFieldValue], String> {
    let RuntimeValue::Record(fields) = value else {
        return Err(format!(
            "expected {label} record, got `{}`",
            value_label(value)
        ));
    };
    Ok(fields)
}

pub(crate) fn runtime_record_get<'a>(
    fields: &'a [RuntimeFieldValue],
    name: &str,
) -> Result<&'a RuntimeValue, String> {
    fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| &field.value)
        .ok_or_else(|| format!("record is missing `{name}`"))
}

pub(crate) fn runtime_record_string(
    fields: &[RuntimeFieldValue],
    name: &str,
) -> Result<String, String> {
    runtime_record_get(fields, name).and_then(runtime_string)
}
pub(crate) fn runtime_field(name: &str, value: RuntimeValue) -> RuntimeFieldValue {
    RuntimeFieldValue {
        name: name.to_owned(),
        value,
    }
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
    let fields = runtime_record_fields(value, "capture target")?;
    match runtime_record_string(fields, "kind")?.as_str() {
        "viewport" => Ok(CaptureTarget::Viewport),
        "layer" => runtime_record_get(fields, "target")
            .and_then(runtime_public_id)
            .map(|id| CaptureTarget::Layer { id }),
        "object" => runtime_record_get(fields, "target")
            .and_then(runtime_string)
            .map(|id| CaptureTarget::Object { id }),
        other => Err(format!("unsupported typed capture target kind `{other}`")),
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
        .map(|field| runtime_agent_value(&field.value).map(|value| (field.name.clone(), value)))
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
            .map(|field| runtime_agent_value(&field.value).map(|value| (field.name.clone(), value)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(AgentValue::Map),
        other => Err(format!("unsupported Agent value `{}`", value_label(other))),
    }
}

pub(crate) fn value_label(value: &RuntimeValue) -> String {
    RuntimePayload::new(value.clone()).label()
}
