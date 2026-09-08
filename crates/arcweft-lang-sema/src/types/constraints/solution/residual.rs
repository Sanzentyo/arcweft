//! Completed residual quantification and fresh continuation opening.
//!
//! This carrier contains declaration origins and lexical slots only. An active
//! application's issuer is consulted during the transition and is never saved.

use std::collections::BTreeMap;

use super::super::super::{
    ArrayLength, GenericBinder, GenericConstReference, GenericParameterKind, GenericScope,
    GenericScopeError, GenericTypeReference, TypeKind,
};
use super::super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::super::references::{self, ConstraintReferenceMap};
use super::super::{
    ConstraintDomain, TypeConstraintConstEligibility, TypeConstraintError,
    TypeConstraintParameterEligibility,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResidualGenericBinder {
    scope: GenericScope,
    type_origins: Box<[GenericTypeReference]>,
    const_origins: Box<[GenericConstReference]>,
}

impl ResidualGenericBinder {
    pub(super) fn for_path<A: TypeConstraintAccounting, D: ConstraintDomain>(
        bindings: &BTreeMap<GenericTypeReference, TypeKind>,
        const_bindings: &BTreeMap<GenericConstReference, ArrayLength>,
        context: &TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        let type_origins = context
            .parameter_scope
            .iter()
            .filter_map(|(parameter, eligibility)| {
                (*eligibility == TypeConstraintParameterEligibility::FutureEligible
                    && context
                        .parameter_scope
                        .type_reference(parameter)
                        .is_some_and(|reference| !bindings.contains_key(&reference)))
                .then(|| parameter.clone())
            })
            .collect::<Box<[_]>>();
        let const_origins = context
            .parameter_scope
            .const_iter()
            .filter_map(|(parameter, eligibility)| {
                (*eligibility == TypeConstraintConstEligibility::FutureEligible
                    && context
                        .parameter_scope
                        .const_reference(parameter)
                        .is_some_and(|reference| !const_bindings.contains_key(&reference)))
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
        Ok(Self {
            scope: GenericScope::default().with_binder(GenericBinder::new(types, const_lengths, 0)),
            type_origins,
            const_origins,
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

    pub(super) fn reify_type<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        ty: &TypeKind,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction: Direction::Reify,
        };
        context.with_binder(self.binder(), |context| {
            references::map_type(ty, &mapping, context)
        })
    }

    pub(super) fn reify_length<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        length: &ArrayLength,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction: Direction::Reify,
        };
        context.with_binder(self.binder(), |context| {
            references::map_length(length, &mapping, context)
        })
    }

    pub(super) fn reopen_type<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        ty: &TypeKind,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction: Direction::Reopen,
        };
        context.with_binder(self.binder(), |context| {
            references::map_type(ty, &mapping, context)
        })
    }

    pub(super) fn reopen_length<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        length: &ArrayLength,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction: Direction::Reopen,
        };
        context.with_binder(self.binder(), |context| {
            references::map_length(length, &mapping, context)
        })
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Reify,
    Reopen,
}

struct ResidualReferenceMap<'a> {
    residual: &'a ResidualGenericBinder,
    outer_depth: usize,
    direction: Direction,
}

impl ResidualReferenceMap<'_> {
    fn residual_depth(&self, scope: &GenericScope) -> Result<u32, TypeConstraintError> {
        let depth = scope
            .binders()
            .len()
            .checked_sub(self.outer_depth)
            .and_then(|distance| distance.checked_sub(1))
            .and_then(|depth| u32::try_from(depth).ok())
            .ok_or(GenericScopeError::UnknownDepth { depth: 0 })?;
        Ok(depth)
    }
}

impl ConstraintReferenceMap for ResidualReferenceMap<'_> {
    fn type_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericTypeReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericTypeReference, TypeConstraintError> {
        match (self.direction, reference) {
            (Direction::Reify, GenericTypeReference::Inference(_)) => {
                let parameter = context
                    .parameter_scope
                    .type_declaration(reference)
                    .ok_or_else(|| references::type_out_of_scope(reference))?;
                let slot = self
                    .residual
                    .type_origins
                    .binary_search(parameter)
                    .ok()
                    .and_then(|slot| u16::try_from(slot).ok())
                    .ok_or_else(|| references::type_out_of_scope(reference))?;
                Ok(context
                    .lexical_scope()
                    .bound_type(self.residual_depth(context.lexical_scope())?, slot)?)
            }
            (Direction::Reopen, GenericTypeReference::Inference(_)) => {
                Err(GenericScopeError::EscapedInference {
                    kind: GenericParameterKind::Type,
                }
                .into())
            }
            (Direction::Reopen, GenericTypeReference::Bound(parameter))
                if !self.residual.binder().is_empty() =>
            {
                context
                    .lexical_scope()
                    .bound_type(parameter.depth(), parameter.slot())?;
                let depth = self.residual_depth(context.lexical_scope())?;
                if parameter.depth() == depth {
                    let origin = self
                        .residual
                        .type_origins
                        .get(usize::from(parameter.slot()))
                        .ok_or_else(|| references::type_out_of_scope(reference))?;
                    context
                        .parameter_scope
                        .type_reference(origin)
                        .ok_or_else(|| references::type_out_of_scope(reference))
                } else if parameter.depth() < depth {
                    Ok(reference.clone())
                } else {
                    Err(GenericScopeError::UnknownDepth {
                        depth: parameter.depth(),
                    }
                    .into())
                }
            }
            _ => {
                context
                    .parameter_eligibility(reference)
                    .ok_or_else(|| references::type_out_of_scope(reference))?;
                Ok(reference.clone())
            }
        }
    }

    fn const_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericConstReference, TypeConstraintError> {
        match (self.direction, reference) {
            (Direction::Reify, GenericConstReference::Inference(_)) => {
                let parameter = context
                    .parameter_scope
                    .const_declaration(reference)
                    .ok_or_else(|| references::const_out_of_scope(reference))?;
                let slot = self
                    .residual
                    .const_origins
                    .binary_search(parameter)
                    .ok()
                    .and_then(|slot| u16::try_from(slot).ok())
                    .ok_or_else(|| references::const_out_of_scope(reference))?;
                Ok(context
                    .lexical_scope()
                    .bound_const(self.residual_depth(context.lexical_scope())?, slot)?)
            }
            (Direction::Reopen, GenericConstReference::Inference(_)) => {
                Err(GenericScopeError::EscapedInference {
                    kind: GenericParameterKind::Const,
                }
                .into())
            }
            (Direction::Reopen, GenericConstReference::Bound(parameter))
                if !self.residual.binder().is_empty() =>
            {
                context
                    .lexical_scope()
                    .bound_const(parameter.depth(), parameter.slot())?;
                let depth = self.residual_depth(context.lexical_scope())?;
                if parameter.depth() == depth {
                    let origin = self
                        .residual
                        .const_origins
                        .get(usize::from(parameter.slot()))
                        .ok_or_else(|| references::const_out_of_scope(reference))?;
                    context
                        .parameter_scope
                        .const_reference(origin)
                        .ok_or_else(|| references::const_out_of_scope(reference))
                } else if parameter.depth() < depth {
                    Ok(reference.clone())
                } else {
                    Err(GenericScopeError::UnknownDepth {
                        depth: parameter.depth(),
                    }
                    .into())
                }
            }
            _ => {
                context
                    .const_parameter_eligibility(reference)
                    .ok_or_else(|| references::const_out_of_scope(reference))?;
                Ok(reference.clone())
            }
        }
    }
}
