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
use std::{cell::RefCell, collections::BTreeSet};

#[derive(Clone, Copy)]
enum Direction<'a> {
    Reify,
    Reopen(&'a RetainedRigidGenericScope),
}

struct ContextReferenceMap<'a> {
    generic: &'a CompletedGenericContext,
    source: ConstraintLexicalScope,
    target: ConstraintLexicalScope,
    direction: Direction<'a>,
    captures: RefCell<BTreeSet<ConstraintGenericParameterId>>,
    root_header: RefCell<BTreeSet<ConstraintGenericParameterId>>,
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
    fn reference_map<'a, A: TypeConstraintAccounting, D: ConstraintDomain>(
        &'a self,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
        direction: Direction<'a>,
    ) -> Result<ContextReferenceMap<'a>, TypeConstraintError> {
        let occurrence = context.capture_lexical_scope()?;
        Ok(match direction {
            Direction::Reify => ContextReferenceMap {
                generic: self,
                source: occurrence,
                target: self.ambient.clone(),
                direction,
                captures: RefCell::new(BTreeSet::new()),
                root_header: RefCell::new(BTreeSet::new()),
            },
            Direction::Reopen(_) => ContextReferenceMap {
                generic: self,
                source: self.ambient.clone(),
                target: application.introduction_scope().clone(),
                direction,
                captures: RefCell::new(BTreeSet::new()),
                root_header: RefCell::new(BTreeSet::new()),
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
        retained_predicate: Option<&CompletedValue<EffectPredicate>>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<CompletedValue<TypeKind>, TypeConstraintError> {
        let mapping = self.reference_map(application, context, Direction::Reify)?;
        let mut value = mapping.map_type(ty, path, application, context)?;
        mapping.retain_result_predicate(&mut value, retained_predicate, context)?;
        Ok(mapping.finish(value))
    }

    pub(in crate::types::constraints::solution) fn reopen_type<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        ty: &CompletedValue<TypeKind>,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        self.reference_map(application, context, Direction::Reopen(ty.captures()))?
            .map_type(ty.value(), path, application, context)
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
    ) -> Result<CompletedValue<ArrayLength>, TypeConstraintError> {
        let mapping = self.reference_map(application, context, Direction::Reify)?;
        let value = mapping.length(
            length,
            mapping.source_scope(),
            mapping.target_scope(),
            path,
            application,
            context,
        )?;
        Ok(mapping.finish(value))
    }

    pub(in crate::types::constraints::solution) fn reopen_length<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        length: &CompletedValue<ArrayLength>,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeConstraintError> {
        let mapping =
            self.reference_map(application, context, Direction::Reopen(length.captures()))?;
        mapping.length(
            length.value(),
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
    ) -> Result<CompletedValue<EffectRow>, TypeConstraintError> {
        let mapping = self.reference_map(application, context, Direction::Reify)?;
        let value = mapping.map_row(row, path, application, context)?;
        Ok(mapping.finish(value))
    }

    pub(in crate::types::constraints::solution) fn reopen_effect_row<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        row: &CompletedValue<EffectRow>,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectRow, TypeConstraintError> {
        self.reference_map(application, context, Direction::Reopen(row.captures()))?
            .map_row(row.value(), path, application, context)
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
    ) -> Result<CompletedValue<EffectPredicate>, TypeConstraintError> {
        let mapping = self.reference_map(application, context, Direction::Reify)?;
        let value = mapping.map_predicate(predicate, path, application, context)?;
        Ok(mapping.finish(value))
    }

    pub(in crate::types::constraints::solution) fn reopen_effect_predicate<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        predicate: &CompletedValue<EffectPredicate>,
        path: &ConstraintPath<D>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectPredicate, TypeConstraintError> {
        self.reference_map(
            application,
            context,
            Direction::Reopen(predicate.captures()),
        )?
        .map_predicate(predicate.value(), path, application, context)
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
    fn admits_free(&self, parameter: &ConstraintGenericParameterId, visible: bool) -> bool {
        match self.direction {
            Direction::Reify => visible,
            Direction::Reopen(scope) => scope.contains(parameter),
        }
    }

    fn retain_free(&self, parameter: ConstraintGenericParameterId, root_header: bool) {
        if matches!(self.direction, Direction::Reify) {
            if root_header {
                self.root_header.borrow_mut().insert(parameter);
            } else {
                self.captures.borrow_mut().insert(parameter);
            }
        }
    }

    fn finish<T>(self, value: T) -> CompletedValue<T> {
        let mut captures = self.captures.into_inner();
        captures.extend(self.root_header.into_inner());
        CompletedValue {
            value,
            captures: RetainedRigidGenericScope::from_admitted(captures),
        }
    }

    /// Finish the root header before issuing an immutable completed value.
    /// Canonical conjunction may erase references, so retain the final header's
    /// support rather than unioning every input predicate's references.
    fn retain_result_predicate<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        value: &mut TypeKind,
        incoming: Option<&CompletedValue<EffectPredicate>>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        let Some(incoming) = incoming.filter(|value| !value.value().is_unconstrained()) else {
            return Ok(());
        };
        let TypeKind::Function {
            binder,
            effects,
            predicate,
            ..
        } = value
        else {
            return Ok(());
        };
        let source = self.generic.value_scope();
        let target = source.with_binder(*binder);
        let inserted = u32::from(!binder.is_empty());
        let lifted =
            incoming
                .value()
                .map_references(context, &mut |reference, _| match reference {
                    GenericEffectReference::Bound(parameter) => {
                        source.bound_effect(parameter.depth(), parameter.slot())?;
                        let depth = parameter.depth().checked_add(inserted).ok_or(
                            TypeConstraintInvariant::GenericScope(
                                GenericScopeError::UnknownDepth {
                                    depth: parameter.depth(),
                                },
                            ),
                        )?;
                        target
                            .bound_effect(depth, parameter.slot())
                            .map_err(Into::into)
                    }
                    GenericEffectReference::Free(_) => Ok(reference.clone()),
                    GenericEffectReference::Inference(_) => {
                        Err(TypeConstraintInvariant::GenericScope(
                            GenericScopeError::EscapedInference {
                                kind: crate::types::GenericParameterKind::Effect,
                            },
                        )
                        .into())
                    }
                })?;
        *predicate = predicate.and(&lifted, context)?;
        let mut final_header = BTreeSet::new();
        for reference in effects.variables()?.chain(predicate.variables()) {
            if matches!(reference, GenericEffectReference::Free(_)) {
                context.enter_node()?;
                let parameter = reference.clone().into();
                if !self.root_header.borrow().contains(&parameter)
                    && !incoming.captures().contains(&parameter)
                {
                    return Err(references::effect_out_of_scope(reference));
                }
                final_header.insert(parameter);
            }
        }
        self.root_header.replace(final_header);
        Ok(())
    }

    fn source_scope(&self) -> &GenericScope {
        match self.direction {
            Direction::Reify => self.source.shape(),
            Direction::Reopen(_) => self.generic.value_scope(),
        }
    }
    fn target_scope(&self) -> &GenericScope {
        match self.direction {
            Direction::Reify => self.generic.value_scope(),
            Direction::Reopen(_) => self.target.shape(),
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
        if matches!(self.direction, Direction::Reopen(_)) && residual != 0 && depth_index == nested
        {
            return Ok(None);
        }
        let (source_inner, target_inner) = match self.direction {
            Direction::Reify => (nested, nested.checked_add(residual)),
            Direction::Reopen(_) => (
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
                let parameter = reference.clone().into();
                if !self.admits_free(
                    &parameter,
                    path.applications.parameter_eligibility(reference)
                        == Some(TypeConstraintParameterEligibility::Rigid),
                ) {
                    return Err(references::type_out_of_scope(reference));
                }
                self.retain_free(parameter, false);
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
                let parameter = reference.clone().into();
                if !self.admits_free(
                    &parameter,
                    path.applications.const_parameter_eligibility(reference)
                        == Some(TypeConstraintConstEligibility::Rigid),
                ) {
                    return Err(references::const_out_of_scope(reference));
                }
                self.retain_free(parameter, false);
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
        root_header: bool,
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
                let parameter = reference.clone().into();
                if !self.admits_free(
                    &parameter,
                    path.applications.effect_eligibility(reference)
                        == Some(EffectConstraintEligibility::Rigid),
                ) {
                    return Err(references::effect_out_of_scope(reference));
                }
                self.retain_free(parameter, root_header);
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
                        self.effect_reference(
                            reference,
                            source,
                            target,
                            path,
                            application,
                            depth == 2,
                            context,
                        )
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
                false,
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
                false,
                context,
            )
            .map(|reference| EffectRow::open(EffectSet::new(), reference))
            .map_err(TypeProjectionError::Control)
        })
        .map_err(fold_error)
    }
}
