//! Completed residual quantification and fresh continuation opening.
//!
//! This carrier contains declaration origins and lexical slots only. An active
//! application's issuer is consulted during the transition and is never saved.

use super::super::super::{
    GenericBinder, GenericConstReference, GenericEffectReference, GenericParameterKind,
    GenericScope, GenericScopeError, GenericTypeReference,
};
use super::super::application::ConstraintApplicationScope;
use super::super::{
    ConstraintDomain, ConstraintPath, TypeConstraintConstEligibility, TypeConstraintError,
    TypeConstraintParameterEligibility,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResidualGenericBinder {
    scope: GenericScope,
    type_origins: Box<[GenericTypeReference]>,
    const_origins: Box<[GenericConstReference]>,
    effect_origins: Box<[GenericEffectReference]>,
}

impl ResidualGenericBinder {
    pub(super) fn for_application<D: ConstraintDomain>(
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
    ) -> Result<Self, TypeConstraintError> {
        let scope = application.parameters();
        let type_origins = scope
            .iter()
            .filter_map(|(parameter, eligibility)| {
                (*eligibility == TypeConstraintParameterEligibility::FutureEligible
                    && scope
                        .type_reference(parameter)
                        .is_some_and(|reference| !path.bindings.contains_key(&reference)))
                .then(|| parameter.clone())
            })
            .collect::<Box<[_]>>();
        let const_origins = scope
            .const_iter()
            .filter_map(|(parameter, eligibility)| {
                (*eligibility == TypeConstraintConstEligibility::FutureEligible
                    && scope
                        .const_reference(parameter)
                        .is_some_and(|reference| !path.const_bindings.contains_key(&reference)))
                .then(|| parameter.clone())
            })
            .collect::<Box<[_]>>();
        let types = u16::try_from(type_origins.len()).map_err(|_| {
            GenericScopeError::BinderArityOverflow {
                kind: GenericParameterKind::Type,
                count: type_origins.len(),
            }
        })?;
        let const_lengths = u16::try_from(const_origins.len()).map_err(|_| {
            GenericScopeError::BinderArityOverflow {
                kind: GenericParameterKind::Const,
                count: const_origins.len(),
            }
        })?;
        let effect_origins = scope
            .effect_contract()
            .variables()
            .filter(|row| {
                row.eligibility() == crate::effect_row::EffectConstraintEligibility::FutureEligible
            })
            .map(|row| row.variable().clone())
            .collect::<Box<[_]>>();
        let effects = u32::try_from(effect_origins.len()).map_err(|_| {
            GenericScopeError::BinderArityOverflow {
                kind: GenericParameterKind::Effect,
                count: effect_origins.len(),
            }
        })?;
        Ok(Self {
            scope: GenericScope::default().with_binder(GenericBinder::new(
                types,
                const_lengths,
                effects,
            )),
            type_origins,
            const_origins,
            effect_origins,
        })
    }

    pub(super) fn binder(&self) -> GenericBinder {
        self.scope
            .binders()
            .first()
            .copied()
            .unwrap_or(GenericBinder::EMPTY)
    }

    pub(super) const fn scope(&self) -> &GenericScope {
        &self.scope
    }

    pub(super) fn type_origins(&self) -> &[GenericTypeReference] {
        &self.type_origins
    }

    pub(super) fn const_origins(&self) -> &[GenericConstReference] {
        &self.const_origins
    }

    pub(super) fn effect_origins(&self) -> &[GenericEffectReference] {
        &self.effect_origins
    }

    pub(super) fn contains_type(&self, parameter: &GenericTypeReference) -> bool {
        self.type_origins.binary_search(parameter).is_ok()
    }

    pub(super) fn contains_const(&self, parameter: &GenericConstReference) -> bool {
        self.const_origins.binary_search(parameter).is_ok()
    }

    pub(super) fn type_slot(&self, parameter: &GenericTypeReference) -> Option<u16> {
        self.type_origins
            .binary_search(parameter)
            .ok()
            .and_then(|slot| u16::try_from(slot).ok())
    }

    pub(super) fn const_slot(&self, parameter: &GenericConstReference) -> Option<u16> {
        self.const_origins
            .binary_search(parameter)
            .ok()
            .and_then(|slot| u16::try_from(slot).ok())
    }

    pub(super) fn contains_effect(&self, parameter: &GenericEffectReference) -> bool {
        self.effect_origins.binary_search(parameter).is_ok()
    }

    pub(super) fn effect_slot(&self, parameter: &GenericEffectReference) -> Option<u32> {
        self.effect_origins
            .binary_search(parameter)
            .ok()
            .and_then(|slot| u32::try_from(slot).ok())
    }
}
