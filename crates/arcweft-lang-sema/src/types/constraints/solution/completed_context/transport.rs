//! Capture-preserving projection between an active and a completed context.

use super::*;
use crate::types::constraints::{
    ConstraintGenericParameterId, TypeConstraintAbort, TypeConstraintConstEligibility, references,
};
use crate::{
    effect_row::{EffectPredicate, EffectRow},
    effects::EffectSet,
    types::{GenericScopeError, TypeProjectionError},
};

#[derive(Clone, Copy)]
enum Direction {
    Reify,
    Reopen,
}

struct ContextReferenceMap<'a> {
    generic: &'a CompletedGenericContext,
    source: ConstraintLexicalScope,
    target: ConstraintLexicalScope,
    direction: Direction,
}

fn fold_error(error: TypeProjectionError<TypeConstraintError>) -> TypeConstraintError {
    match error {
        TypeProjectionError::Control(error) => error,
        TypeProjectionError::Instantiation(TypeInstantiationError::Scope(error)) => error.into(),
        TypeProjectionError::Instantiation(TypeInstantiationError::Effect(error)) => error.into(),
        TypeProjectionError::Instantiation(_) => TypeConstraintRejection::UnresolvedType.into(),
    }
}

impl CompletedGenericContext {
    fn reference_map<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
        direction: Direction,
    ) -> Result<ContextReferenceMap<'_>, TypeConstraintError> {
        let occurrence = context.capture_lexical_scope()?;
        Ok(match direction {
            Direction::Reify => ContextReferenceMap {
                generic: self,
                source: occurrence,
                target: self.ambient.clone(),
                direction,
            },
            Direction::Reopen => ContextReferenceMap {
                generic: self,
                source: self.ambient.clone(),
                target: application.introduction_scope().clone(),
                direction,
            },
        })
    }

    pub(in crate::types::constraints::solution) fn reify_type<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        ty: &TypeKind,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        self.reference_map(application, context, Direction::Reify)?
            .map_type(ty, path, application, context)
    }

    pub(in crate::types::constraints::solution) fn reopen_type<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        ty: &TypeKind,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        self.reference_map(application, context, Direction::Reopen)?
            .map_type(ty, path, application, context)
    }

    pub(in crate::types::constraints::solution) fn reify_length<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        length: &ArrayLength,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeConstraintError> {
        let mapping = self.reference_map(application, context, Direction::Reify)?;
        mapping.length(
            length,
            mapping.source_scope(),
            mapping.target_scope(),
            path,
            application,
            context,
        )
    }

    pub(in crate::types::constraints::solution) fn reopen_length<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        length: &ArrayLength,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeConstraintError> {
        let mapping = self.reference_map(application, context, Direction::Reopen)?;
        mapping.length(
            length,
            mapping.source_scope(),
            mapping.target_scope(),
            path,
            application,
            context,
        )
    }

    pub(in crate::types::constraints::solution) fn reify_effect_row<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        row: &EffectRow,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectRow, TypeConstraintError> {
        self.reference_map(application, context, Direction::Reify)?
            .map_row(row, path, application, context)
    }

    pub(in crate::types::constraints::solution) fn reopen_effect_row<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        row: &EffectRow,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectRow, TypeConstraintError> {
        self.reference_map(application, context, Direction::Reopen)?
            .map_row(row, path, application, context)
    }

    pub(in crate::types::constraints::solution) fn reify_effect_predicate<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        predicate: &EffectPredicate,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectPredicate, TypeConstraintError> {
        self.reference_map(application, context, Direction::Reify)?
            .map_predicate(predicate, path, application, context)
    }

    pub(in crate::types::constraints::solution) fn reopen_effect_predicate<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        predicate: &EffectPredicate,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectPredicate, TypeConstraintError> {
        self.reference_map(application, context, Direction::Reopen)?
            .map_predicate(predicate, path, application, context)
    }

    pub(in crate::types::constraints::solution) fn validate_reified_future_type<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        ty: &TypeKind,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        context.with_original_scope(self.value_lexical_scope().clone(), |context| {
            let projected = project_type(
                ty,
                path.projection_view(),
                ConstraintClosurePolicy::ProjectionFuture,
                context,
            )?;
            if projected.value != *ty || !projected.remaining.is_empty() {
                return Err(TypeConstraintRejection::UnresolvedType.into());
            }
            Ok(())
        })
    }
}

impl ContextReferenceMap<'_> {
    fn source_scope(&self) -> &GenericScope {
        match self.direction {
            Direction::Reify => self.source.shape(),
            Direction::Reopen => self.generic.value_scope(),
        }
    }
    fn target_scope(&self) -> &GenericScope {
        match self.direction {
            Direction::Reify => self.generic.value_scope(),
            Direction::Reopen => self.target.shape(),
        }
    }
    fn nested(&self, source: &GenericScope) -> Result<usize, TypeConstraintError> {
        source
            .binders()
            .len()
            .checked_sub(self.source_scope().binders().len())
            .ok_or_else(|| GenericScopeError::UnknownDepth { depth: 0 }.into())
    }

    /// None denotes precisely this completed context's residual binder.
    fn depth<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        source: &GenericScope,
        depth: u32,
        parameter: ConstraintGenericParameterId,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Option<u32>, TypeConstraintError> {
        let nested = self.nested(source)?;
        let depth_index =
            usize::try_from(depth).map_err(|_| TypeConstraintAbort::ArithmeticOverflow)?;
        if depth_index < nested {
            return Ok(Some(depth));
        }
        let residual = usize::from(!self.generic.residual.binder().is_empty());
        if matches!(self.direction, Direction::Reopen) && residual != 0 && depth_index == nested {
            return Ok(None);
        }
        let (source_inner, target_inner) = match self.direction {
            Direction::Reify => (nested, nested.checked_add(residual)),
            Direction::Reopen => (
                nested
                    .checked_add(residual)
                    .ok_or(TypeConstraintAbort::ArithmeticOverflow)?,
                Some(nested),
            ),
        };
        let target_inner = target_inner.ok_or(TypeConstraintAbort::ArithmeticOverflow)?;
        self.target
            .ambient_depth(&self.source, depth, source_inner, target_inner, context)
            .map(Some)
            .map_err(|error| match error {
                TypeProjectionError::Control(error) => error,
                TypeProjectionError::Instantiation(_) => {
                    TypeConstraintRejection::EscapingInstantiation { parameter }.into()
                }
            })
    }

    fn type_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericTypeReference, TypeConstraintError> {
        match reference {
            GenericTypeReference::Bound(bound) => {
                source.bound_type(bound.depth(), bound.slot())?;
                match self.depth(source, bound.depth(), reference.clone().into(), context)? {
                    Some(depth) => Ok(target.bound_type(depth, bound.slot())?),
                    None => {
                        let origin = self
                            .generic
                            .residual
                            .type_origins()
                            .get(usize::from(bound.slot()))
                            .ok_or_else(|| references::type_out_of_scope(reference))?;
                        application
                            .parameters()
                            .type_reference(origin)
                            .ok_or_else(|| references::type_out_of_scope(reference))
                    }
                }
            }
            GenericTypeReference::Inference(_) if matches!(self.direction, Direction::Reify) => {
                let origin = application
                    .parameters()
                    .type_declaration(reference)
                    .ok_or_else(|| references::type_out_of_scope(reference))?;
                let slot = self
                    .generic
                    .residual
                    .type_slot(origin)
                    .ok_or_else(|| references::type_out_of_scope(reference))?;
                Ok(target.bound_type(
                    u32::try_from(self.nested(source)?)
                        .map_err(|_| TypeConstraintAbort::ArithmeticOverflow)?,
                    slot,
                )?)
            }
            GenericTypeReference::Inference(_) => Err(references::type_out_of_scope(reference)),
            GenericTypeReference::Free(_) => {
                if path.applications.parameter_eligibility(reference)
                    != Some(TypeConstraintParameterEligibility::Rigid)
                    && !self.generic.captures.contains(&reference.clone().into())
                {
                    return Err(references::type_out_of_scope(reference));
                }
                Ok(reference.clone())
            }
        }
    }

    fn const_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericConstReference,
        source: &GenericScope,
        target: &GenericScope,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericConstReference, TypeConstraintError> {
        match reference {
            GenericConstReference::Bound(bound) => {
                source.bound_const(bound.depth(), bound.slot())?;
                match self.depth(source, bound.depth(), reference.clone().into(), context)? {
                    Some(depth) => Ok(target.bound_const(depth, bound.slot())?),
                    None => {
                        let origin = self
                            .generic
                            .residual
                            .const_origins()
                            .get(usize::from(bound.slot()))
                            .ok_or_else(|| references::const_out_of_scope(reference))?;
                        application
                            .parameters()
                            .const_reference(origin)
                            .ok_or_else(|| references::const_out_of_scope(reference))
                    }
                }
            }
            GenericConstReference::Inference(_) if matches!(self.direction, Direction::Reify) => {
                let origin = application
                    .parameters()
                    .const_declaration(reference)
                    .ok_or_else(|| references::const_out_of_scope(reference))?;
                let slot = self
                    .generic
                    .residual
                    .const_slot(origin)
                    .ok_or_else(|| references::const_out_of_scope(reference))?;
                Ok(target.bound_const(
                    u32::try_from(self.nested(source)?)
                        .map_err(|_| TypeConstraintAbort::ArithmeticOverflow)?,
                    slot,
                )?)
            }
            GenericConstReference::Inference(_) => Err(references::const_out_of_scope(reference)),
            GenericConstReference::Free(_) => {
                if path.applications.const_parameter_eligibility(reference)
                    != Some(TypeConstraintConstEligibility::Rigid)
                    && !self.generic.captures.contains(&reference.clone().into())
                {
                    return Err(references::const_out_of_scope(reference));
                }
                Ok(reference.clone())
            }
        }
    }

    fn effect_reference<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        reference: &GenericEffectReference,
        source: &GenericScope,
        target: &GenericScope,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<GenericEffectReference, TypeConstraintError> {
        match reference {
            GenericEffectReference::Bound(bound) => {
                source.bound_effect(bound.depth(), bound.slot())?;
                match self.depth(source, bound.depth(), reference.clone().into(), context)? {
                    Some(depth) => Ok(target.bound_effect(depth, bound.slot())?),
                    None => {
                        let origin = self
                            .generic
                            .residual
                            .effect_origins()
                            .get(
                                usize::try_from(bound.slot())
                                    .map_err(|_| TypeConstraintAbort::ArithmeticOverflow)?,
                            )
                            .ok_or_else(|| references::effect_out_of_scope(reference))?;
                        application
                            .parameters()
                            .effect_reference(origin)
                            .ok_or_else(|| references::effect_out_of_scope(reference))
                    }
                }
            }
            GenericEffectReference::Inference(_) if matches!(self.direction, Direction::Reify) => {
                let origin = application
                    .parameters()
                    .effect_declaration(reference)
                    .ok_or_else(|| references::effect_out_of_scope(reference))?;
                let slot = self
                    .generic
                    .residual
                    .effect_slot(origin)
                    .ok_or_else(|| references::effect_out_of_scope(reference))?;
                Ok(target.bound_effect(
                    u32::try_from(self.nested(source)?)
                        .map_err(|_| TypeConstraintAbort::ArithmeticOverflow)?,
                    slot,
                )?)
            }
            GenericEffectReference::Inference(_) => Err(references::effect_out_of_scope(reference)),
            GenericEffectReference::Free(_) => {
                if path.applications.effect_eligibility(reference)
                    != Some(EffectConstraintEligibility::Rigid)
                    && !self.generic.captures.contains(&reference.clone().into())
                {
                    return Err(references::effect_out_of_scope(reference));
                }
                Ok(reference.clone())
            }
        }
    }

    fn length<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeConstraintError> {
        context.enter_node()?;
        match length {
            ArrayLength::Generic(reference) => self
                .const_reference(reference, source, target, path, application, context)
                .map(ArrayLength::Generic),
            ArrayLength::Const(_) => Ok(length.clone()),
            ArrayLength::Error(_) | ArrayLength::Inferred => {
                Err(TypeConstraintRejection::UnresolvedType.into())
            }
        }
    }

    fn map_type<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        ty: &TypeKind,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        super::super::map_term_with_control(
            ty,
            self.source_scope(),
            self.target_scope(),
            1,
            context,
            &|reference, source, target, _, context| {
                self.type_reference(reference, source, target, path, application, context)
                    .map(TypeKind::GenericParam)
                    .map_err(TypeProjectionError::Control)
            },
            &|length, source, target, _, context| {
                self.length(length, source, target, path, application, context)
                    .map_err(TypeProjectionError::Control)
            },
            &|row, source, target, depth, context| {
                super::super::map_effects_with_control(
                    row,
                    depth,
                    context,
                    &|reference, context| {
                        self.effect_reference(reference, source, target, path, application, context)
                            .map(|reference| EffectRow::open(EffectSet::new(), reference))
                            .map_err(TypeProjectionError::Control)
                    },
                )
            },
        )
        .map_err(fold_error)
    }

    fn map_row<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        row: &EffectRow,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectRow, TypeConstraintError> {
        super::super::map_effects_with_control(row, 1, context, &|reference, context| {
            self.effect_reference(
                reference,
                self.source_scope(),
                self.target_scope(),
                path,
                application,
                context,
            )
            .map(|reference| EffectRow::open(EffectSet::new(), reference))
            .map_err(TypeProjectionError::Control)
        })
        .map_err(fold_error)
    }

    fn map_predicate<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        predicate: &EffectPredicate,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectPredicate, TypeConstraintError> {
        super::super::map_predicate_with_control(predicate, 1, context, &|reference, context| {
            self.effect_reference(
                reference,
                self.source_scope(),
                self.target_scope(),
                path,
                application,
                context,
            )
            .map(|reference| EffectRow::open(EffectSet::new(), reference))
            .map_err(TypeProjectionError::Control)
        })
        .map_err(fold_error)
    }
}
