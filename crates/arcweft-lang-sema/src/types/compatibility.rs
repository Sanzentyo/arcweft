use crate::{
    effect_row::{EffectRow, EffectRowTail},
    types::TypeKind,
};

impl TypeKind {
    /// Returns whether an earlier authoritative resolution failure prevents a
    /// second compatibility diagnostic from adding useful information.
    ///
    /// The underscore name is the checker-local recovery type retained by a
    /// few inference paths. `Error` carries the typed poison identity for an
    /// already reported resolution failure.
    pub(crate) fn is_unresolved_for_compatibility(&self) -> bool {
        matches!(self, Self::Named(name) if name == "_") || matches!(self, Self::Error(_))
    }

    /// Returns whether a value of `actual` can satisfy this expected type.
    ///
    /// This is the shared semantic compatibility rule used by argument
    /// checking and callable applicability. Keeping it on the owned type
    /// prevents resolver families from growing parallel compatibility tables.
    pub(crate) fn accepts(&self, actual: &Self) -> bool {
        if matches!(self, Self::Error(_)) || matches!(actual, Self::Error(_)) {
            return true;
        }
        if self.first_mismatch(actual).is_none() || matches!(self, Self::Named(name) if name == "_")
        {
            return true;
        }
        if matches!(actual, Self::Never) {
            return true;
        }
        if let Some(compatible) = nominal_types_compatible(self, actual) {
            return compatible;
        }
        match (self, actual) {
            (Self::Bytes, Self::Vec(inner) | Self::Slice(inner) | Self::Seq(inner)) => {
                matches!(inner.as_ref(), Self::U8)
            }
            (Self::ActionName, Self::String | Self::Named(_)) => true,
            (Self::AgentValue, actual) => is_agent_value_type(actual),
            (Self::Ref(expected), Self::Ref(actual)) if expected.kind() == actual.kind() => {
                // A payload-free reference is a family constraint used by
                // shared callable schemas. It accepts a retained payload
                // specialization without erasing that payload from `actual`.
                expected.value().is_none()
            }
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
            ) => expected_len.accepts(actual_len) && expected_item.accepts(actual_item),
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

fn nominal_types_compatible(expected: &TypeKind, actual: &TypeKind) -> Option<bool> {
    Some(match (expected, actual) {
        (TypeKind::ProjectNominal(expected), TypeKind::ProjectNominal(actual)) => {
            expected.declaration() == actual.declaration()
                && nominal_arguments_compatible(expected.arguments(), actual.arguments())
        }
        (TypeKind::AcceptedNominal(expected), TypeKind::AcceptedNominal(actual)) => {
            expected.declaration() == actual.declaration()
                && nominal_arguments_compatible(expected.arguments(), actual.arguments())
        }
        (TypeKind::OpenNominal(expected), TypeKind::OpenNominal(actual)) => {
            expected.rule() == actual.rule()
                && expected.path() == actual.path()
                && nominal_arguments_compatible(expected.arguments(), actual.arguments())
        }
        _ => return None,
    })
}

fn nominal_arguments_compatible(expected: &[TypeKind], actual: &[TypeKind]) -> bool {
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .all(|(expected, actual)| expected.accepts(actual))
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
        | TypeKind::AgentResourceBody
        | TypeKind::Error(_) => true,
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
        .filter(|alternative| !matches!(alternative, TypeKind::Error(_)))
        .filter(|alternative| alternative.accepts(actual));
    match (
        compatible_alternatives.next(),
        compatible_alternatives.next(),
    ) {
        (Some(selected), None) => Some(selected),
        (Some(_), Some(_)) => None,
        (None, _) => alternatives
            .iter()
            .find(|alternative| matches!(alternative, TypeKind::Error(_))),
    }
}

#[cfg(test)]
mod tests {
    use super::TypeKind;
    use crate::types::{
        ArrayLength, DetachedTypeOwnerId, EntityKind, GenericTypeOwnerId, GenericTypeParameterId,
        TypePoisonId,
    };

    #[test]
    fn family_entity_reference_accepts_payload_specialization() {
        let family = TypeKind::entity_ref(EntityKind::Signal);
        let typed = TypeKind::entity_ref_with_value(EntityKind::Signal, TypeKind::Bool);

        assert!(family.accepts(&typed));
        assert!(!typed.accepts(&family));
    }

    #[test]
    fn typed_entity_reference_requires_compatible_family_and_payload() {
        let expected = TypeKind::entity_ref_with_value(EntityKind::Signal, TypeKind::Bool);
        let matching = TypeKind::entity_ref_with_value(EntityKind::Signal, TypeKind::Bool);
        let wrong_payload = TypeKind::entity_ref_with_value(EntityKind::Signal, TypeKind::String);
        let wrong_family = TypeKind::entity_ref_with_value(EntityKind::Metric, TypeKind::Bool);

        assert!(expected.accepts(&matching));
        assert!(!expected.accepts(&wrong_payload));
        assert!(!expected.accepts(&wrong_family));
    }

    #[test]
    fn poison_inside_recovered_shapes_does_not_create_follow_on_mismatches() {
        let poison = TypeKind::Error(TypePoisonId::from_index(3));

        assert!(TypeKind::Vec(Box::new(TypeKind::I32)).accepts(&TypeKind::Vec(Box::new(poison))));
        assert!(
            TypeKind::AgentValue.accepts(&TypeKind::Vec(Box::new(TypeKind::Error(
                TypePoisonId::from_index(4)
            ))))
        );
        assert!(
            TypeKind::Choice(vec![
                TypeKind::Error(TypePoisonId::from_index(5)),
                TypeKind::I32,
            ])
            .accepts(&TypeKind::I32)
        );
    }

    #[test]
    fn array_lengths_are_exact_when_concrete_and_open_when_generic_or_recovered() {
        let generic = ArrayLength::Generic(GenericTypeParameterId::new(
            GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(41)),
            0,
        ));

        assert!(ArrayLength::Const(3).accepts(&ArrayLength::Const(3)));
        assert!(!ArrayLength::Const(3).accepts(&ArrayLength::Const(4)));
        assert_eq!(ArrayLength::Const(3).source_label(), "3");
        assert_eq!(ArrayLength::Inferred.source_label(), "_");
        assert!(generic.accepts(&ArrayLength::Const(4)));
        assert!(ArrayLength::Inferred.accepts(&ArrayLength::Const(4)));
        assert!(ArrayLength::Error(TypePoisonId::from_index(6)).accepts(&ArrayLength::Const(4)));
    }
}
