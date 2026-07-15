use crate::{
    effect_row::{EffectRow, EffectRowTail},
    types::TypeKind,
};

pub(super) fn types_compatible(expected: &TypeKind, actual: &TypeKind) -> bool {
    if expected.first_mismatch(actual).is_none()
        || matches!(expected, TypeKind::Named(name) if name == "_")
    {
        return true;
    }
    if matches!(actual, TypeKind::Never) {
        return true;
    }
    match (expected, actual) {
        (TypeKind::Bytes, TypeKind::Vec(inner) | TypeKind::Slice(inner) | TypeKind::Seq(inner)) => {
            matches!(inner.as_ref(), TypeKind::U8)
        }
        (TypeKind::ActionName, TypeKind::String | TypeKind::Named(_)) => true,
        (TypeKind::AgentValue, actual) => is_agent_value_type(actual),
        (TypeKind::Choice(alternatives), TypeKind::Choice(actual_alternatives)) => {
            actual_alternatives
                .iter()
                .all(|actual| choice_injection_target(alternatives, actual).is_some())
        }
        (TypeKind::Choice(alternatives), actual) => {
            choice_injection_target(alternatives, actual).is_some()
        }
        (expected, TypeKind::Choice(alternatives)) => alternatives
            .iter()
            .all(|actual| types_compatible(expected, actual)),
        (
            TypeKind::Result {
                ok: expected_ok,
                error: expected_error,
            },
            TypeKind::Result {
                ok: actual_ok,
                error: actual_error,
            },
        ) => {
            types_compatible(expected_ok, actual_ok)
                && (types_compatible(expected_error, actual_error)
                    || matches!(actual_error.as_ref(), TypeKind::Named(name) if name == "_"))
        }
        (TypeKind::Option(expected), TypeKind::Option(actual)) => {
            types_compatible(expected, actual)
                || matches!(actual.as_ref(), TypeKind::Named(name) if name == "_")
        }
        (TypeKind::Vec(expected), TypeKind::Vec(actual))
        | (TypeKind::Seq(expected), TypeKind::Seq(actual))
        | (TypeKind::Slice(expected), TypeKind::Slice(actual)) => {
            types_compatible(expected, actual)
        }
        (
            TypeKind::Array {
                item: expected_item,
                len: expected_len,
            },
            TypeKind::Array {
                item: actual_item,
                len: actual_len,
            },
        ) => expected_len == actual_len && types_compatible(expected_item, actual_item),
        (TypeKind::Range(expected), TypeKind::Range(actual)) => types_compatible(expected, actual),
        (TypeKind::Tuple(expected), TypeKind::Tuple(actual)) => {
            expected.len() == actual.len()
                && expected
                    .iter()
                    .zip(actual)
                    .all(|(expected, actual)| types_compatible(expected, actual))
        }
        (
            TypeKind::Function {
                params: expected_params,
                return_type: expected_return,
                effects: expected_effects,
            },
            TypeKind::Function {
                params: actual_params,
                return_type: actual_return,
                effects: actual_effects,
            },
        ) => {
            expected_params.len() == actual_params.len()
                && expected_params
                    .iter()
                    .zip(actual_params.iter())
                    .all(|(expected, actual)| types_compatible(expected, actual))
                && types_compatible(expected_return, actual_return)
                && effect_rows_compatible(expected_effects, actual_effects)
        }
        _ => false,
    }
}

fn effect_rows_compatible(expected: &EffectRow, actual: &EffectRow) -> bool {
    match (expected.tail(), actual.tail()) {
        (EffectRowTail::Unknown, _) | (_, EffectRowTail::Unknown) => true,
        (EffectRowTail::Closed, EffectRowTail::Closed)
        | (EffectRowTail::Variable(_), EffectRowTail::Closed | EffectRowTail::Variable(_)) => {
            actual
                .concrete()
                .effects_not_covered_by(expected.concrete())
                .is_empty()
        }
        (EffectRowTail::Closed, EffectRowTail::Variable(_)) => false,
    }
}

fn is_agent_value_type(ty: &TypeKind) -> bool {
    match ty {
        TypeKind::Bool
        | TypeKind::I8
        | TypeKind::I16
        | TypeKind::I32
        | TypeKind::I64
        | TypeKind::I128
        | TypeKind::ISize
        | TypeKind::U8
        | TypeKind::U16
        | TypeKind::U32
        | TypeKind::U64
        | TypeKind::U128
        | TypeKind::USize
        | TypeKind::F32
        | TypeKind::F64
        | TypeKind::String
        | TypeKind::Char
        | TypeKind::Bytes
        | TypeKind::Duration
        | TypeKind::DisplayText
        | TypeKind::ActionName
        | TypeKind::AgentValue
        | TypeKind::ObservedObject
        | TypeKind::AgentBBox
        | TypeKind::Ref(_)
        | TypeKind::CaptureRef
        | TypeKind::AgentResource
        | TypeKind::AgentResourceBody => true,
        TypeKind::Vec(inner)
        | TypeKind::Array { item: inner, .. }
        | TypeKind::Slice(inner)
        | TypeKind::Range(inner)
        | TypeKind::Option(inner) => is_agent_value_type(inner),
        TypeKind::Map { key, value, .. } => {
            types_compatible(&TypeKind::String, key) && is_agent_value_type(value)
        }
        TypeKind::Choice(alternatives) => alternatives.iter().all(is_agent_value_type),
        _ => false,
    }
}

fn choice_injection_target<'a>(
    alternatives: &'a [TypeKind],
    actual: &TypeKind,
) -> Option<&'a TypeKind> {
    let mut compatible_alternatives = alternatives
        .iter()
        .filter(|alternative| types_compatible(alternative, actual));
    let selected = compatible_alternatives.next()?;
    compatible_alternatives.next().is_none().then_some(selected)
}
