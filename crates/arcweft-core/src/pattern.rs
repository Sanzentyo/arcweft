use crate::value::{RuntimeBinding, RuntimeEvalError, RuntimeValue};
use std::collections::BTreeSet;

/// Pattern subset executable by the Sans I/O flow runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRecordPatternField {
    pub name: String,
    pub pattern: RuntimePattern,
}

pub(crate) fn match_runtime_pattern(
    pattern: &RuntimePattern,
    value: &RuntimeValue,
) -> Result<Option<Vec<RuntimeBinding>>, RuntimeEvalError> {
    let mut bindings = Vec::new();
    if collect_pattern_bindings(pattern, value, &mut bindings)? {
        reject_duplicate_bindings(&bindings)?;
        Ok(Some(bindings))
    } else {
        Ok(None)
    }
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
        RuntimePattern::Ident(name)
        | RuntimePattern::MutIdent(name)
        | RuntimePattern::Typed { name, .. } => {
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
            let RuntimeValue::Record(values) = value else {
                return Ok(false);
            };
            if !rest && fields.len() != values.len() {
                return Ok(false);
            }
            for field in fields {
                let Some(value_field) =
                    values.iter().find(|candidate| candidate.name == field.name)
                else {
                    return Ok(false);
                };
                if !collect_pattern_bindings(&field.pattern, &value_field.value, bindings)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        RuntimePattern::BracketSeq { items, rest } => {
            let RuntimeValue::BracketSeq(values) = value else {
                return Ok(false);
            };
            if rest.is_none() && items.len() != values.len() {
                return Ok(false);
            }
            if rest.is_some() && items.len() > values.len() {
                return Ok(false);
            }
            if !collect_pattern_list(items, &values[..items.len()], bindings)? {
                return Ok(false);
            }
            if let Some(name) = rest {
                bindings.push(RuntimeBinding {
                    name: name.clone(),
                    value: RuntimeValue::BracketSeq(values[items.len()..].to_vec()),
                });
            }
            Ok(true)
        }
        RuntimePattern::Variant {
            path,
            name,
            payload,
        } => {
            let RuntimeValue::Variant {
                path: actual_path,
                name: actual_name,
                payload: actual_payload,
            } = value
            else {
                return Ok(false);
            };
            if path != actual_path || name != actual_name {
                return Ok(false);
            }
            match (payload, actual_payload) {
                (Some(pattern), Some(value)) => collect_pattern_bindings(pattern, value, bindings),
                (None, None | Some(_)) => Ok(true),
                (Some(_), None) => Ok(false),
            }
        }
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
