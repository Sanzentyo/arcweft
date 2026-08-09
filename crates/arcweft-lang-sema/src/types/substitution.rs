//! Declaration-owned generic substitution for semantic types.

use std::collections::BTreeMap;

use super::{
    AcceptedNominalType, EntityType, GenericTypeParameterId, OpenNominalType, ProjectNominalType,
    TypeKind,
};

/// One call-site instantiation of declaration-owned generic type parameters.
///
/// Inference observes already checked argument types and only commits a set of
/// bindings when every repeated generic identity remains consistent. Source
/// spellings never participate in the lookup.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TypeParameterSubstitutions {
    bindings: BTreeMap<GenericTypeParameterId, TypeKind>,
}

impl TypeParameterSubstitutions {
    /// Infers bindings from one declared parameter shape and actual argument.
    pub(crate) fn observe(&mut self, declared: &TypeKind, actual: &TypeKind) -> bool {
        let mut bindings = self.bindings.clone();
        if !observe_type_parameters(declared, actual, &mut bindings) {
            return false;
        }
        self.bindings = bindings;
        true
    }

    /// Applies all call-site bindings to a semantic type.
    pub(crate) fn apply(&self, ty: &TypeKind) -> TypeKind {
        ty.substitute_type_parameters(&self.bindings)
    }

    /// Applies the known bindings only when the resulting type is concrete.
    ///
    /// Contextual expression checking must not treat an unbound declaration
    /// parameter as a value type. Once every parameter in the shape is bound,
    /// however, the specialized declaration type is the authoritative expected
    /// type for literals and other context-sensitive expressions.
    pub(crate) fn apply_resolved(&self, ty: &TypeKind) -> Option<TypeKind> {
        let applied = self.apply(ty);
        (!contains_generic_parameter(&applied)).then_some(applied)
    }
}

fn contains_generic_parameter(ty: &TypeKind) -> bool {
    match ty {
        TypeKind::GenericParam(_) => true,
        TypeKind::Range(inner)
        | TypeKind::Probe(inner)
        | TypeKind::Vec(inner)
        | TypeKind::Slice(inner)
        | TypeKind::Seq(inner)
        | TypeKind::Option(inner)
        | TypeKind::ThreadHandle(inner)
        | TypeKind::Shared(inner)
        | TypeKind::BorrowRef { inner, .. } => contains_generic_parameter(inner),
        TypeKind::IteratorState { item, .. } | TypeKind::Array { item, .. } => {
            contains_generic_parameter(item)
        }
        TypeKind::Ref(entity) => entity.value().is_some_and(contains_generic_parameter),
        TypeKind::Map { key, value, .. } => {
            contains_generic_parameter(key) || contains_generic_parameter(value)
        }
        TypeKind::Need { ready, error } => {
            contains_generic_parameter(ready) || contains_generic_parameter(error)
        }
        TypeKind::Stream { item, error } | TypeKind::Source { item, error } => {
            contains_generic_parameter(item) || contains_generic_parameter(error)
        }
        TypeKind::Result { ok, error } => {
            contains_generic_parameter(ok) || contains_generic_parameter(error)
        }
        TypeKind::Function {
            params,
            return_type,
            ..
        } => {
            params.iter().any(contains_generic_parameter) || contains_generic_parameter(return_type)
        }
        TypeKind::ProjectNominal(nominal) => {
            nominal.arguments().iter().any(contains_generic_parameter)
        }
        TypeKind::AcceptedNominal(nominal) => {
            nominal.arguments().iter().any(contains_generic_parameter)
        }
        TypeKind::OpenNominal(nominal) => {
            nominal.arguments().iter().any(contains_generic_parameter)
        }
        TypeKind::Projection { subject, .. } => contains_generic_parameter(subject),
        TypeKind::Tuple(items) | TypeKind::Choice(items) => {
            items.iter().any(contains_generic_parameter)
        }
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
        | TypeKind::TextCluster
        | TypeKind::Duration
        | TypeKind::DisplayText
        | TypeKind::DebugStatePath
        | TypeKind::ObservationFieldPath
        | TypeKind::Predicate
        | TypeKind::Observation
        | TypeKind::ObservedObject
        | TypeKind::AgentBBox
        | TypeKind::ActionName
        | TypeKind::ActionTarget
        | TypeKind::ActionResult
        | TypeKind::AgentValue
        | TypeKind::DataFormat
        | TypeKind::DataShape
        | TypeKind::AgentEntityMetadata
        | TypeKind::AgentSourceAnchor
        | TypeKind::AgentProjectGraphNeighborhood
        | TypeKind::AgentProjectGraphSymbol
        | TypeKind::AgentProjectGraphEdge
        | TypeKind::CaptureTarget
        | TypeKind::CaptureRef
        | TypeKind::AgentResource
        | TypeKind::AgentResourceBody
        | TypeKind::RagContextPack
        | TypeKind::Handle { .. }
        | TypeKind::Error(_)
        | TypeKind::CharacterPatch(_)
        | TypeKind::FocusPatch
        | TypeKind::CharacterNominal(_)
        | TypeKind::Named(_)
        | TypeKind::Unit
        | TypeKind::Never => false,
    }
}

impl TypeKind {
    /// Replaces declaration-owned generic parameters throughout this type.
    ///
    /// Callers provide typed parameter identities, so equal source spellings
    /// from different declarations can never alias one another.
    pub(crate) fn substitute_type_parameters(
        &self,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
    ) -> Self {
        if let Some(substituted) = self.substitute_nominal_type_parameters(substitutions) {
            return substituted;
        }
        if let Some(substituted) = self.substitute_unary_type_parameters(substitutions) {
            return substituted;
        }
        match self {
            Self::GenericParam(parameter) => substitutions
                .get(parameter)
                .cloned()
                .unwrap_or_else(|| self.clone()),
            Self::BorrowRef {
                kind,
                lifetime,
                inner,
            } => Self::BorrowRef {
                kind: *kind,
                lifetime: lifetime.clone(),
                inner: Box::new(inner.substitute_type_parameters(substitutions)),
            },
            Self::Ref(entity) => Self::Ref(EntityType::new(
                entity.kind().clone(),
                entity
                    .value()
                    .map(|value| value.substitute_type_parameters(substitutions)),
            )),
            Self::IteratorState { family, item } => Self::IteratorState {
                family: *family,
                item: Box::new(item.substitute_type_parameters(substitutions)),
            },
            Self::Need { ready, error } => Self::Need {
                ready: Box::new(ready.substitute_type_parameters(substitutions)),
                error: Box::new(error.substitute_type_parameters(substitutions)),
            },
            Self::Stream { item, error } => Self::Stream {
                item: Box::new(item.substitute_type_parameters(substitutions)),
                error: Box::new(error.substitute_type_parameters(substitutions)),
            },
            Self::Source { item, error } => Self::Source {
                item: Box::new(item.substitute_type_parameters(substitutions)),
                error: Box::new(error.substitute_type_parameters(substitutions)),
            },
            Self::Result { ok, error } => Self::Result {
                ok: Box::new(ok.substitute_type_parameters(substitutions)),
                error: Box::new(error.substitute_type_parameters(substitutions)),
            },
            Self::Map { kind, key, value } => Self::Map {
                kind: *kind,
                key: Box::new(key.substitute_type_parameters(substitutions)),
                value: Box::new(value.substitute_type_parameters(substitutions)),
            },
            Self::Array { item, len } => Self::Array {
                item: Box::new(item.substitute_type_parameters(substitutions)),
                len: len.clone(),
            },
            Self::Function {
                params,
                return_type,
                effects,
            } => Self::function_with_effects(
                params
                    .iter()
                    .map(|param| param.substitute_type_parameters(substitutions)),
                return_type.substitute_type_parameters(substitutions),
                effects.clone(),
            ),
            Self::Projection {
                subject,
                trait_name,
                assoc,
            } => Self::Projection {
                subject: Box::new(subject.substitute_type_parameters(substitutions)),
                trait_name: trait_name.clone(),
                assoc: assoc.clone(),
            },
            Self::Tuple(items) => Self::Tuple(
                items
                    .iter()
                    .map(|item| item.substitute_type_parameters(substitutions))
                    .collect(),
            ),
            Self::Choice(items) => Self::Choice(
                items
                    .iter()
                    .map(|item| item.substitute_type_parameters(substitutions))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    fn substitute_nominal_type_parameters(
        &self,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
    ) -> Option<Self> {
        let substitute_arguments = |arguments: &[Self]| {
            arguments
                .iter()
                .map(|argument| argument.substitute_type_parameters(substitutions))
                .collect::<Vec<_>>()
        };
        Some(match self {
            Self::ProjectNominal(nominal) => Self::ProjectNominal(ProjectNominalType::new(
                nominal.declaration().clone(),
                substitute_arguments(nominal.arguments()),
            )),
            Self::AcceptedNominal(nominal) => Self::AcceptedNominal(AcceptedNominalType::new(
                nominal.declaration().clone(),
                substitute_arguments(nominal.arguments()),
            )),
            Self::OpenNominal(nominal) => Self::OpenNominal(OpenNominalType::new(
                nominal.rule().clone(),
                nominal.path().clone(),
                substitute_arguments(nominal.arguments()),
            )),
            _ => return None,
        })
    }

    fn substitute_unary_type_parameters(
        &self,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
    ) -> Option<Self> {
        let substitute = |inner: &Self| Box::new(inner.substitute_type_parameters(substitutions));
        Some(match self {
            Self::Vec(inner) => Self::Vec(substitute(inner)),
            Self::Seq(inner) => Self::Seq(substitute(inner)),
            Self::Slice(inner) => Self::Slice(substitute(inner)),
            Self::Range(inner) => Self::Range(substitute(inner)),
            Self::Option(inner) => Self::Option(substitute(inner)),
            Self::ThreadHandle(inner) => Self::ThreadHandle(substitute(inner)),
            Self::Shared(inner) => Self::Shared(substitute(inner)),
            Self::Probe(inner) => Self::Probe(substitute(inner)),
            _ => return None,
        })
    }
}

fn observe_type_parameters(
    declared: &TypeKind,
    actual: &TypeKind,
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> bool {
    if declared.is_unresolved_for_compatibility() || actual.is_unresolved_for_compatibility() {
        return true;
    }
    match declared {
        TypeKind::GenericParam(parameter) => {
            if let Some(bound) = bindings.get(parameter) {
                bound == actual
            } else {
                bindings.insert(parameter.clone(), actual.clone());
                true
            }
        }
        TypeKind::ProjectNominal(declared) => match actual {
            TypeKind::ProjectNominal(actual) if declared.declaration() == actual.declaration() => {
                observe_type_slices(declared.arguments(), actual.arguments(), bindings)
            }
            _ => true,
        },
        TypeKind::AcceptedNominal(declared) => match actual {
            TypeKind::AcceptedNominal(actual) if declared.declaration() == actual.declaration() => {
                observe_type_slices(declared.arguments(), actual.arguments(), bindings)
            }
            _ => true,
        },
        TypeKind::OpenNominal(declared) => match actual {
            TypeKind::OpenNominal(actual)
                if declared.rule() == actual.rule() && declared.path() == actual.path() =>
            {
                observe_type_slices(declared.arguments(), actual.arguments(), bindings)
            }
            _ => true,
        },
        TypeKind::Ref(declared) => match actual {
            TypeKind::Ref(actual) if declared.kind() == actual.kind() => {
                match (declared.value(), actual.value()) {
                    (Some(declared), Some(actual)) => {
                        observe_type_parameters(declared, actual, bindings)
                    }
                    _ => true,
                }
            }
            _ => true,
        },
        _ => observe_unary_type_parameters(declared, actual, bindings)
            .or_else(|| observe_composite_type_parameters(declared, actual, bindings))
            .unwrap_or(true),
    }
}

fn observe_unary_type_parameters(
    declared: &TypeKind,
    actual: &TypeKind,
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> Option<bool> {
    let pair = match (declared, actual) {
        (TypeKind::Range(declared), TypeKind::Range(actual))
        | (TypeKind::Probe(declared), TypeKind::Probe(actual))
        | (TypeKind::Vec(declared), TypeKind::Vec(actual))
        | (TypeKind::Slice(declared), TypeKind::Slice(actual))
        | (TypeKind::Seq(declared), TypeKind::Seq(actual))
        | (TypeKind::Option(declared), TypeKind::Option(actual))
        | (TypeKind::ThreadHandle(declared), TypeKind::ThreadHandle(actual))
        | (TypeKind::Shared(declared), TypeKind::Shared(actual)) => (declared, actual),
        (TypeKind::Array { item: declared, .. }, TypeKind::Array { item: actual, .. }) => {
            (declared, actual)
        }
        (
            TypeKind::BorrowRef {
                kind: declared_kind,
                lifetime: declared_lifetime,
                inner: declared,
            },
            TypeKind::BorrowRef {
                kind: actual_kind,
                lifetime: actual_lifetime,
                inner: actual,
            },
        ) if declared_kind == actual_kind && declared_lifetime == actual_lifetime => {
            (declared, actual)
        }
        (
            TypeKind::IteratorState {
                family: declared_family,
                item: declared,
            },
            TypeKind::IteratorState {
                family: actual_family,
                item: actual,
            },
        ) if declared_family == actual_family => (declared, actual),
        (
            TypeKind::Range(_)
            | TypeKind::Probe(_)
            | TypeKind::Vec(_)
            | TypeKind::Slice(_)
            | TypeKind::Seq(_)
            | TypeKind::Option(_)
            | TypeKind::ThreadHandle(_)
            | TypeKind::Shared(_)
            | TypeKind::Array { .. }
            | TypeKind::BorrowRef { .. }
            | TypeKind::IteratorState { .. },
            _,
        ) => return Some(true),
        _ => return None,
    };
    Some(observe_type_parameters(pair.0, pair.1, bindings))
}

fn observe_composite_type_parameters(
    declared: &TypeKind,
    actual: &TypeKind,
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> Option<bool> {
    let observed = match (declared, actual) {
        (
            TypeKind::Need {
                ready: declared_first,
                error: declared_second,
            }
            | TypeKind::Stream {
                item: declared_first,
                error: declared_second,
            }
            | TypeKind::Source {
                item: declared_first,
                error: declared_second,
            }
            | TypeKind::Result {
                ok: declared_first,
                error: declared_second,
            },
            TypeKind::Need {
                ready: actual_first,
                error: actual_second,
            }
            | TypeKind::Stream {
                item: actual_first,
                error: actual_second,
            }
            | TypeKind::Source {
                item: actual_first,
                error: actual_second,
            }
            | TypeKind::Result {
                ok: actual_first,
                error: actual_second,
            },
        ) if core::mem::discriminant(declared) == core::mem::discriminant(actual) => {
            observe_type_parameters(declared_first, actual_first, bindings)
                && observe_type_parameters(declared_second, actual_second, bindings)
        }
        (
            TypeKind::Map {
                kind: declared_kind,
                key: declared_key,
                value: declared_value,
            },
            TypeKind::Map {
                kind: actual_kind,
                key: actual_key,
                value: actual_value,
            },
        ) if declared_kind == actual_kind => {
            observe_type_parameters(declared_key, actual_key, bindings)
                && observe_type_parameters(declared_value, actual_value, bindings)
        }
        (
            TypeKind::Function {
                params: declared_params,
                return_type: declared_return,
                ..
            },
            TypeKind::Function {
                params: actual_params,
                return_type: actual_return,
                ..
            },
        ) => {
            observe_type_slices(declared_params, actual_params, bindings)
                && observe_type_parameters(declared_return, actual_return, bindings)
        }
        (
            TypeKind::Projection {
                subject: declared_subject,
                trait_name: declared_trait,
                assoc: declared_assoc,
            },
            TypeKind::Projection {
                subject: actual_subject,
                trait_name: actual_trait,
                assoc: actual_assoc,
            },
        ) if declared_trait == actual_trait && declared_assoc == actual_assoc => {
            observe_type_parameters(declared_subject, actual_subject, bindings)
        }
        (
            TypeKind::Need { .. }
            | TypeKind::Stream { .. }
            | TypeKind::Source { .. }
            | TypeKind::Result { .. }
            | TypeKind::Map { .. }
            | TypeKind::Function { .. }
            | TypeKind::Projection { .. },
            _,
        ) => true,
        _ => return observe_sequence_type_parameters(declared, actual, bindings),
    };
    Some(observed)
}

fn observe_sequence_type_parameters(
    declared: &TypeKind,
    actual: &TypeKind,
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> Option<bool> {
    Some(match (declared, actual) {
        (TypeKind::Tuple(declared), TypeKind::Tuple(actual))
        | (TypeKind::Choice(declared), TypeKind::Choice(actual)) => {
            observe_type_slices(declared, actual, bindings)
        }
        (TypeKind::Tuple(_) | TypeKind::Choice(_), _) => true,
        _ => return None,
    })
}

fn observe_type_slices(
    declared: &[TypeKind],
    actual: &[TypeKind],
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> bool {
    declared.len() == actual.len()
        && declared
            .iter()
            .zip(actual)
            .all(|(declared, actual)| observe_type_parameters(declared, actual, bindings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DetachedTypeOwnerId, GenericTypeOwnerId};

    #[test]
    fn typed_identity_controls_recursive_substitution() {
        let owner = GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(7));
        let selected = GenericTypeParameterId::new(owner.clone(), 0);
        let untouched = GenericTypeParameterId::new(owner, 1);
        let ty = TypeKind::Result {
            ok: Box::new(TypeKind::Vec(Box::new(TypeKind::GenericParam(
                selected.clone(),
            )))),
            error: Box::new(TypeKind::GenericParam(untouched.clone())),
        };
        let substitutions = BTreeMap::from([(selected, TypeKind::String)]);

        assert_eq!(
            ty.substitute_type_parameters(&substitutions),
            TypeKind::Result {
                ok: Box::new(TypeKind::Vec(Box::new(TypeKind::String))),
                error: Box::new(TypeKind::GenericParam(untouched)),
            }
        );
    }

    #[test]
    fn observation_specializes_a_nested_result_error() {
        let owner = GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(11));
        let error = GenericTypeParameterId::new(owner, 0);
        let declared = TypeKind::Result {
            ok: Box::new(TypeKind::I64),
            error: Box::new(TypeKind::GenericParam(error.clone())),
        };
        let actual = TypeKind::Result {
            ok: Box::new(TypeKind::I64),
            error: Box::new(TypeKind::String),
        };
        let mut substitutions = TypeParameterSubstitutions::default();

        assert!(substitutions.observe(&declared, &actual));
        assert_eq!(
            substitutions.apply(&TypeKind::GenericParam(error)),
            TypeKind::String
        );
    }

    #[test]
    fn conflicting_observation_does_not_commit_partial_bindings() {
        let owner = GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(12));
        let item = GenericTypeParameterId::new(owner.clone(), 0);
        let error = GenericTypeParameterId::new(owner, 1);
        let mut substitutions = TypeParameterSubstitutions::default();
        assert!(substitutions.observe(&TypeKind::GenericParam(item.clone()), &TypeKind::I64));

        let declared = TypeKind::Tuple(vec![
            TypeKind::GenericParam(error.clone()),
            TypeKind::GenericParam(item.clone()),
        ]);
        let actual = TypeKind::Tuple(vec![TypeKind::Bool, TypeKind::String]);
        assert!(!substitutions.observe(&declared, &actual));

        assert_eq!(
            substitutions.apply(&TypeKind::GenericParam(item)),
            TypeKind::I64
        );
        assert_eq!(
            substitutions.apply(&TypeKind::GenericParam(error.clone())),
            TypeKind::GenericParam(error)
        );
    }

    #[test]
    fn resolved_application_only_exposes_concrete_expected_types() {
        let owner = GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(13));
        let item = GenericTypeParameterId::new(owner, 0);
        let declared = TypeKind::Option(Box::new(TypeKind::GenericParam(item.clone())));
        let mut substitutions = TypeParameterSubstitutions::default();

        assert_eq!(
            substitutions.apply_resolved(&TypeKind::I64),
            Some(TypeKind::I64)
        );
        assert_eq!(substitutions.apply_resolved(&declared), None);
        assert!(substitutions.observe(&TypeKind::GenericParam(item), &TypeKind::I64));
        assert_eq!(
            substitutions.apply_resolved(&declared),
            Some(TypeKind::Option(Box::new(TypeKind::I64)))
        );
    }
}
