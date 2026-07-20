use crate::{
    effect_row::{EffectRow, EffectRowTail},
    types::TypeKind,
};

impl TypeKind {
    /// Returns whether a value of `actual` can satisfy this expected type.
    ///
    /// This is the shared semantic compatibility rule used by argument
    /// checking and callable applicability. Keeping it on the owned type
    /// prevents resolver families from growing parallel compatibility tables.
    pub(crate) fn accepts(&self, actual: &Self) -> bool {
        if self.first_mismatch(actual).is_none() || matches!(self, Self::Named(name) if name == "_")
        {
            return true;
        }
        if matches!(actual, Self::Never) {
            return true;
        }
        match (self, actual) {
            (Self::Bytes, Self::Vec(inner) | Self::Slice(inner) | Self::Seq(inner)) => {
                matches!(inner.as_ref(), Self::U8)
            }
            (Self::ActionName, Self::String | Self::Named(_)) => true,
            (Self::AgentValue, actual) => is_agent_value_type(actual),
            (Self::Choice(alternatives), Self::Choice(actual_alternatives)) => actual_alternatives
                .iter()
                .all(|actual| choice_injection_target(alternatives, actual).is_some()),
            (Self::Choice(alternatives), actual) => {
                choice_injection_target(alternatives, actual).is_some()
            }
            (expected, Self::Choice(alternatives)) => {
                alternatives.iter().all(|actual| expected.accepts(actual))
            }
            (
                Self::Result {
                    ok: expected_ok,
                    error: expected_error,
                },
                Self::Result {
                    ok: actual_ok,
                    error: actual_error,
                },
            ) => {
                expected_ok.accepts(actual_ok)
                    && (expected_error.accepts(actual_error)
                        || matches!(actual_error.as_ref(), Self::Named(name) if name == "_"))
            }
            (Self::Option(expected), Self::Option(actual)) => {
                expected.accepts(actual)
                    || matches!(actual.as_ref(), Self::Named(name) if name == "_")
            }
            (Self::Vec(expected), Self::Vec(actual))
            | (Self::Seq(expected), Self::Seq(actual))
            | (Self::Slice(expected), Self::Slice(actual))
            | (Self::Range(expected), Self::Range(actual)) => expected.accepts(actual),
            (
                Self::Array {
                    item: expected_item,
                    len: expected_len,
                },
                Self::Array {
                    item: actual_item,
                    len: actual_len,
                },
            ) => expected_len == actual_len && expected_item.accepts(actual_item),
            (Self::Tuple(expected), Self::Tuple(actual)) => {
                expected.len() == actual.len()
                    && expected
                        .iter()
                        .zip(actual)
                        .all(|(expected, actual)| expected.accepts(actual))
            }
            (
                Self::Function {
                    params: expected_params,
                    return_type: expected_return,
                    effects: expected_effects,
                },
                Self::Function {
                    params: actual_params,
                    return_type: actual_return,
                    effects: actual_effects,
                },
            ) => {
                expected_params.len() == actual_params.len()
                    && expected_params
                        .iter()
                        .zip(actual_params)
                        .all(|(expected, actual)| expected.accepts(actual))
                    && expected_return.accepts(actual_return)
                    && effect_rows_compatible(expected_effects, actual_effects)
            }
            _ => false,
        }
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
            TypeKind::String.accepts(key) && is_agent_value_type(value)
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
        .filter(|alternative| alternative.accepts(actual));
    let selected = compatible_alternatives.next()?;
    compatible_alternatives.next().is_none().then_some(selected)
}
