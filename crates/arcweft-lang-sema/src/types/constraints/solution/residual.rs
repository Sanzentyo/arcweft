//! Completed residual quantification and fresh continuation opening.
//!
//! This carrier contains declaration origins and lexical slots only. An active
//! application's issuer is consulted during the transition and is never saved.

use super::super::super::{
    ArrayLength, GenericBinder, GenericConstReference, GenericEffectReference,
    GenericParameterKind, GenericScope, GenericScopeError, GenericTypeReference, TypeKind,
};
use super::super::application::ConstraintApplicationScope;
use super::super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::super::normalization::project_type;
use super::super::references::{self, ConstraintReferenceMap};
use super::super::{
    ConstraintClosurePolicy, ConstraintDomain, ConstraintPath, TypeConstraintConstEligibility,
    TypeConstraintError, TypeConstraintInvariant, TypeConstraintParameterEligibility,
    TypeConstraintProjectionInvariant, TypeConstraintRejection,
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
    pub(super) fn reify_effect_predicate<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        predicate: &crate::effect_row::EffectPredicate,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<crate::effect_row::EffectPredicate, TypeConstraintError> {
        self.map_effect_predicate(predicate, path, application, context, Direction::Reify)
    }

    pub(super) fn reopen_effect_predicate<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        predicate: &crate::effect_row::EffectPredicate,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<crate::effect_row::EffectPredicate, TypeConstraintError> {
        self.map_effect_predicate(predicate, path, application, context, Direction::Reopen)
    }

    fn map_effect_predicate<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        predicate: &crate::effect_row::EffectPredicate,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
        direction: Direction,
    ) -> Result<crate::effect_row::EffectPredicate, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction,
        };
        context.with_binder(self.binder(), |context| {
            predicate.map_references(context, &mut |reference, context| {
                mapping.effect_reference(reference, application, path, context)
            })
        })
    }

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

    pub(super) fn reify_effect_row<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        row: &crate::effect_row::EffectRow,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<crate::effect_row::EffectRow, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction: Direction::Reify,
        };
        context.with_binder(self.binder(), |context| {
            references::map_effect_row(row, &mapping, application, path, context)
        })
    }

    pub(super) fn reopen_effect_row<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        row: &crate::effect_row::EffectRow,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<crate::effect_row::EffectRow, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction: Direction::Reopen,
        };
        context.with_binder(self.binder(), |context| {
            references::map_effect_row(row, &mapping, application, path, context)
        })
    }

    pub(super) fn reify_type<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        ty: &TypeKind,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction: Direction::Reify,
        };
        context.with_binder(self.binder(), |context| {
            references::map_type(ty, &mapping, application, path, context)
        })
    }

    pub(super) fn validate_reified_future_type<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        ty: &TypeKind,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        context.with_binder(self.binder(), |context| {
            let projected = project_type(
                ty,
                path.projection_view(),
                ConstraintClosurePolicy::ProjectionFuture,
                context,
            )?;
            if projected.value != *ty || !projected.remaining.is_empty() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::Projection(
                        TypeConstraintProjectionInvariant::Mismatch(
                            TypeConstraintRejection::UnresolvedType,
                        ),
                    ),
                ));
            }
            Ok(())
        })
    }

    pub(super) fn reify_length<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        length: &ArrayLength,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction: Direction::Reify,
        };
        context.with_binder(self.binder(), |context| {
            references::map_length(length, &mapping, application, path, context)
        })
    }

    pub(super) fn reopen_type<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        ty: &TypeKind,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction: Direction::Reopen,
        };
        context.with_binder(self.binder(), |context| {
            references::map_type(ty, &mapping, application, path, context)
        })
    }

    pub(super) fn reopen_length<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        length: &ArrayLength,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeConstraintError> {
        let mapping = ResidualReferenceMap {
            residual: self,
            outer_depth: context.lexical_scope().binders().len(),
            direction: Direction::Reopen,
        };
        context.with_binder(self.binder(), |context| {
            references::map_length(length, &mapping, application, path, context)
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
        application: &ConstraintApplicationScope<D>,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericTypeReference, TypeConstraintError> {
        match (self.direction, reference) {
            (Direction::Reify, GenericTypeReference::Inference(_)) => {
                let parameter = application
                    .parameters()
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
                    application
                        .parameters()
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
                    .parameter_eligibility(reference, path.projection_view())
                    .ok_or_else(|| references::type_out_of_scope(reference))?;
                Ok(reference.clone())
            }
        }
    }

    fn const_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericConstReference,
        application: &ConstraintApplicationScope<D>,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericConstReference, TypeConstraintError> {
        match (self.direction, reference) {
            (Direction::Reify, GenericConstReference::Inference(_)) => {
                let parameter = application
                    .parameters()
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
                    application
                        .parameters()
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
                    .const_parameter_eligibility(reference, path.projection_view())
                    .ok_or_else(|| references::const_out_of_scope(reference))?;
                Ok(reference.clone())
            }
        }
    }
    fn effect_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericEffectReference,
        application: &ConstraintApplicationScope<D>,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericEffectReference, TypeConstraintError> {
        match (self.direction, reference) {
            (Direction::Reify, GenericEffectReference::Inference(_)) => {
                let parameter = application
                    .parameters()
                    .effect_declaration(reference)
                    .ok_or_else(|| references::effect_out_of_scope(reference))?;
                let slot = self
                    .residual
                    .effect_origins
                    .binary_search(parameter)
                    .ok()
                    .and_then(|slot| u32::try_from(slot).ok())
                    .ok_or_else(|| references::effect_out_of_scope(reference))?;
                Ok(context
                    .lexical_scope()
                    .bound_effect(self.residual_depth(context.lexical_scope())?, slot)?)
            }
            (Direction::Reopen, GenericEffectReference::Inference(_)) => {
                Err(GenericScopeError::EscapedInference {
                    kind: GenericParameterKind::Effect,
                }
                .into())
            }
            (Direction::Reopen, GenericEffectReference::Bound(parameter))
                if !self.residual.binder().is_empty() =>
            {
                context
                    .lexical_scope()
                    .bound_effect(parameter.depth(), parameter.slot())?;
                let depth = self.residual_depth(context.lexical_scope())?;
                if parameter.depth() == depth {
                    let origin = self
                        .residual
                        .effect_origins
                        .get(usize::try_from(parameter.slot()).expect("effect slots fit usize"))
                        .ok_or_else(|| references::effect_out_of_scope(reference))?;
                    application
                        .parameters()
                        .effect_reference(origin)
                        .ok_or_else(|| references::effect_out_of_scope(reference))
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
                    .effect_eligibility(reference, path.projection_view())
                    .ok_or_else(|| references::effect_out_of_scope(reference))?;
                Ok(reference.clone())
            }
        }
    }
}
