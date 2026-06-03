use crate::value::{
    RuntimeBinding, RuntimeEvalError, RuntimeSeq, RuntimeValue, runtime_sequence_values,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Pattern subset executable by the Sans I/O flow runtime.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimePattern {
    Ident(String),
    MutIdent(String),
    Discard,
    Literal(RuntimeValue),
    Entity(String),
    Tuple(Vec<RuntimePattern>),
    Record {
        path: Option<String>,
        fields: Vec<RuntimeRecordPatternField>,
        rest: bool,
    },
    BracketSeq {
        items: Vec<RuntimePattern>,
        rest: Option<String>,
    },
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimePattern>>,
    },
    Whole {
        name: String,
        pattern: Box<RuntimePattern>,
    },
    Typed {
        name: String,
        ty: String,
    },
}

/// One field inside a runtime record pattern.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeRecordPatternField {
    pub name: String,
    pub pattern: RuntimePattern,
}

pub(crate) fn match_runtime_pattern(
    pattern: &RuntimePattern,
    value: &RuntimeValue,
) -> Result<Option<Vec<RuntimeBinding>>, RuntimeEvalError> {
    let mut bindings = Vec::with_capacity(pattern_binding_capacity(pattern));
    if collect_pattern_bindings(pattern, value, &mut bindings)? {
        reject_duplicate_bindings(&bindings)?;
        Ok(Some(bindings))
    } else {
        Ok(None)
    }
}

pub(crate) fn pattern_binding_capacity(pattern: &RuntimePattern) -> usize {
    let direct = match pattern {
        RuntimePattern::Ident(_) | RuntimePattern::MutIdent(_) | RuntimePattern::Typed { .. } => 1,
        RuntimePattern::Discard | RuntimePattern::Literal(_) | RuntimePattern::Entity(_) => 0,
        RuntimePattern::Tuple(patterns)
        | RuntimePattern::BracketSeq {
            items: patterns, ..
        } => patterns.iter().map(pattern_binding_capacity).sum(),
        RuntimePattern::Record { fields, .. } => fields
            .iter()
            .map(|field| pattern_binding_capacity(&field.pattern))
            .sum(),
        RuntimePattern::Variant { payload, .. } => {
            payload.as_deref().map_or(0, pattern_binding_capacity)
        }
        RuntimePattern::Whole { pattern, .. } => pattern_binding_capacity(pattern) + 1,
    };
    direct
        + usize::from(matches!(
            pattern,
            RuntimePattern::BracketSeq { rest: Some(_), .. }
        ))
}

fn reject_duplicate_bindings(bindings: &[RuntimeBinding]) -> Result<(), RuntimeEvalError> {
    let mut seen = BTreeSet::<&str>::new();
    for binding in bindings {
        if !seen.insert(binding.name.as_str()) {
            return Err(RuntimeEvalError::DuplicateBinding(binding.name.clone()));
        }
    }
    Ok(())
}

fn collect_pattern_bindings(
    pattern: &RuntimePattern,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    match pattern {
        RuntimePattern::Ident(name) | RuntimePattern::MutIdent(name) => {
            bindings.push(RuntimeBinding {
                name: name.clone(),
                value: value.clone(),
            });
            Ok(true)
        }
        RuntimePattern::Typed { name, ty } => {
            if !runtime_value_matches_type_label(value, ty) {
                return Ok(false);
            }
            bindings.push(RuntimeBinding {
                name: name.clone(),
                value: value.clone(),
            });
            Ok(true)
        }
        RuntimePattern::Discard => Ok(true),
        RuntimePattern::Literal(expected) => Ok(expected == value),
        RuntimePattern::Entity(expected) => {
            Ok(matches!(value, RuntimeValue::EntityRef(actual) if actual == expected))
        }
        RuntimePattern::Tuple(patterns) => {
            let RuntimeValue::Tuple(values) = value else {
                return Ok(false);
            };
            if patterns.len() != values.len() {
                return Ok(false);
            }
            collect_pattern_list(patterns, values, bindings)
        }
        RuntimePattern::Record { fields, rest, .. } => {
            collect_record_pattern_bindings(fields, *rest, value, bindings)
        }
        RuntimePattern::BracketSeq { items, rest } => {
            collect_bracket_seq_pattern_bindings(items, rest.as_deref(), value, bindings)
        }
        RuntimePattern::Variant {
            path,
            name,
            payload,
        } => collect_variant_pattern_bindings(
            path.as_ref(),
            name,
            payload.as_deref(),
            value,
            bindings,
        ),
        RuntimePattern::Whole { name, pattern } => {
            if !collect_pattern_bindings(pattern, value, bindings)? {
                return Ok(false);
            }
            bindings.push(RuntimeBinding {
                name: name.clone(),
                value: value.clone(),
            });
            Ok(true)
        }
    }
}

fn collect_record_pattern_bindings(
    fields: &[RuntimeRecordPatternField],
    rest: bool,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    let RuntimeValue::Record(values) = value else {
        return Ok(false);
    };
    if !rest && fields.len() != values.len() {
        return Ok(false);
    }
    for field in fields {
        let Some(value_field) = values.iter().find(|candidate| candidate.name == field.name) else {
            return Ok(false);
        };
        if !collect_pattern_bindings(&field.pattern, &value_field.value, bindings)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn collect_bracket_seq_pattern_bindings(
    items: &[RuntimePattern],
    rest: Option<&str>,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    match value {
        RuntimeValue::Seq(RuntimeSeq::Values(values)) => {
            if !bracket_pattern_len_matches(items.len(), rest, values.len()) {
                return Ok(false);
            }
            if !collect_pattern_list(items, &values[..items.len()], bindings)? {
                return Ok(false);
            }
            if let Some(name) = rest {
                bindings.push(RuntimeBinding {
                    name: name.to_owned(),
                    value: runtime_sequence_values(values[items.len()..].to_vec()),
                });
            }
            Ok(true)
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(values)) => {
            if !bracket_pattern_len_matches(items.len(), rest, values.len()) {
                return Ok(false);
            }
            for (index, pattern) in items.iter().enumerate() {
                if !collect_pattern_bindings(pattern, &values.value_at(index), bindings)? {
                    return Ok(false);
                }
            }
            if let Some(name) = rest {
                bindings.push(RuntimeBinding {
                    name: name.to_owned(),
                    value: RuntimeValue::Seq(RuntimeSeq::Dense(values.tail_from(items.len()))),
                });
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn bracket_pattern_len_matches(pattern_len: usize, rest: Option<&str>, value_len: usize) -> bool {
    if rest.is_none() && pattern_len != value_len {
        return false;
    }
    if rest.is_some() && pattern_len > value_len {
        return false;
    }
    true
}

fn collect_variant_pattern_bindings(
    path: Option<&String>,
    name: &str,
    payload: Option<&RuntimePattern>,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    let RuntimeValue::Variant {
        path: actual_path,
        name: actual_name,
        payload: actual_payload,
    } = value
    else {
        return Ok(false);
    };
    if path != actual_path.as_ref() || name != actual_name {
        return Ok(false);
    }
    match (payload, actual_payload) {
        (Some(pattern), Some(value)) => collect_pattern_bindings(pattern, value, bindings),
        (None, None | Some(_)) => Ok(true),
        (Some(_), None) => Ok(false),
    }
}

fn runtime_value_matches_type_label(value: &RuntimeValue, ty: &str) -> bool {
    let ty = ty.trim();
    if ty.contains('|') {
        return ty
            .split('|')
            .map(str::trim)
            .any(|alternative| runtime_value_matches_type_label(value, alternative));
    }
    if matches!(
        (value, ty),
        (RuntimeValue::Unit, "()" | "Unit")
            | (RuntimeValue::Bool(_), "Bool" | "bool")
            | (
                RuntimeValue::Int(_),
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
            )
            | (
                RuntimeValue::UInt(_),
                "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
            )
            | (RuntimeValue::F32(_), "f32")
            | (RuntimeValue::F64(_), "f64")
            | (RuntimeValue::String(_), "String")
            | (RuntimeValue::Char(_), "Char" | "char")
            | (RuntimeValue::Duration(_), "Duration")
            | (RuntimeValue::Record(_), "Record")
    ) {
        return true;
    }
    match value {
        RuntimeValue::EntityRef(_) if ty.starts_with("Ref<") => true,
        RuntimeValue::Tuple(_) if ty.starts_with('(') => true,
        RuntimeValue::Seq(_)
            if ty.starts_with("Vec<")
                || ty.starts_with("Seq<")
                || ty.starts_with("Array<")
                || ty == "Bytes"
                || ty.starts_with('[') =>
        {
            true
        }
        RuntimeValue::Variant { name, path, .. } => {
            ty == name
                || ty == format!(".{name}")
                || path
                    .as_ref()
                    .is_some_and(|path| ty == format!("{path}::{name}"))
        }
        _ => false,
    }
}

fn collect_pattern_list(
    patterns: &[RuntimePattern],
    values: &[RuntimeValue],
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    for (pattern, value) in patterns.iter().zip(values) {
        if !collect_pattern_bindings(pattern, value, bindings)? {
            return Ok(false);
        }
    }
    Ok(true)
}
