//! Declaration-owned generic substitution for semantic types.

use std::collections::BTreeMap;

use super::{
    AcceptedNominalType, GenericTypeParameterId, OpenNominalType, ProjectNominalType, TypeKind,
};

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
}
