//! Capture-preserving projection between an active and a completed context.

use super::*;
use crate::types::constraints::{
    ConstraintGenericParameterId, TypeConstraintAbort, TypeConstraintConstEligibility, references,
};
use crate::types::effect_domain::RebindableEffects;
use crate::{
    effect_row::{EffectPredicate, EffectRow},
    types::{
        GenericScopeError, TypeProjectionControl, TypeProjectionError, TypeProjectionNodeKind,
        projection_control::{EffectProjectionControl, visit_effect_predicate, visit_effect_row},
        structural_projection::{ScopedTypeProjection, child_depth},
    },
};
use std::{cell::RefCell, collections::BTreeSet};

#[cfg(test)]
mod tests;

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
    pub(in crate::types::constraints::solution) fn reify_result_type<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        L,
        E: RebindableEffects,
    >(
        &self,
        value: &TypeKind<E>,
        scope: &ConstraintLexicalScope,
        path: &ConstraintInferenceBranch<D, L>,
        application: &ConstraintApplicationScope<D>,
        retained_predicate: Option<&CompletedValue<EffectPredicate>>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<CompletedValue<TypeKind<E>>, TypeConstraintError> {
        context.with_original_scope(scope.clone(), |context| {
            let mapping = self.reference_map(application, context, Direction::Reify)?;
            let mut value = mapping.map_type(value, path, application, context)?;
            mapping.retain_result_predicate(&mut value, retained_predicate, context)?;
            Ok(mapping.finish(value))
        })
    }

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
        L,
    >(
        &self,
        ty: &super::super::ConstraintOperand<D>,
        path: &ConstraintInferenceBranch<D, L>,
        application: &ConstraintApplicationScope<D>,
        retained_predicate: Option<&CompletedValue<EffectPredicate>>,
        effects: &Arc<ConstraintClosedEffects<D>>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<CompletedValue<TypeKind>, TypeConstraintError> {
        effects.require_branch(path)?;
        path.projection_view()
            .admit_operand(ty.view(), graph, context)?;
        context.with_original_scope(ty.scope().clone(), |context| {
            // Reification moves references, so it consumes the algebra value
            // produced by this exact original completion, never a prepared
            // term interpreted as an already canonical function contract.
            let canonical = effects.canonicalize(ty.value(), ty.scope().shape(), context)?;
            self.reify_result_type(
                &canonical,
                ty.scope(),
                path,
                application,
                retained_predicate,
                context,
            )
        })
    }

    pub(in crate::types::constraints::solution) fn reopen_type<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        L,
    >(
        &self,
        ty: &CompletedValue<TypeKind>,
        path: &ConstraintInferenceBranch<D, L>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        self.reference_map(application, context, Direction::Reopen(ty.captures()))?
            .map_type(ty.value(), path, application, context)
    }

    pub(in crate::types::constraints::solution) fn reify_length<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        L,
    >(
        &self,
        length: &super::super::ConstraintOperand<D, ArrayLength>,
        path: &ConstraintInferenceBranch<D, L>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<CompletedValue<ArrayLength>, TypeConstraintError> {
        context.with_original_scope(length.scope().clone(), |context| {
            let mapping = self.reference_map(application, context, Direction::Reify)?;
            let value = mapping.length(
                length.value(),
                mapping.source_scope(),
                mapping.target_scope(),
                path,
                application,
                context,
            )?;
            Ok(mapping.finish(value))
        })
    }

    pub(in crate::types::constraints::solution) fn reopen_length<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        L,
    >(
        &self,
        length: &CompletedValue<ArrayLength>,
        path: &ConstraintInferenceBranch<D, L>,
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
        L,
    >(
        &self,
        row: &super::super::ConstraintOperand<D, EffectRow>,
        path: &ConstraintInferenceBranch<D, L>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<CompletedValue<EffectRow>, TypeConstraintError> {
        context.with_original_scope(row.scope().clone(), |context| {
            for reference in row.value().variables()? {
                context.enter_node()?;
                row.admit_effect_reference(reference, context)?;
            }
            let mapping = self.reference_map(application, context, Direction::Reify)?;
            let value = mapping.map_row(row.value(), path, application, context)?;
            Ok(mapping.finish(value))
        })
    }

    pub(in crate::types::constraints::solution) fn reopen_effect_row<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        L,
    >(
        &self,
        row: &CompletedValue<EffectRow>,
        path: &ConstraintInferenceBranch<D, L>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectRow, TypeConstraintError> {
        self.reference_map(application, context, Direction::Reopen(row.captures()))?
            .map_row(row.value(), path, application, context)
    }

    pub(in crate::types::constraints::solution) fn reify_effect_predicate<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        L,
    >(
        &self,
        predicate: &super::super::ConstraintOperand<D, EffectPredicate>,
        path: &ConstraintInferenceBranch<D, L>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<CompletedValue<EffectPredicate>, TypeConstraintError> {
        context.with_original_scope(predicate.scope().clone(), |context| {
            for reference in predicate.value().variables() {
                context.enter_node()?;
                predicate.admit_effect_reference(reference, context)?;
            }
            let mapping = self.reference_map(application, context, Direction::Reify)?;
            let value = mapping.map_predicate(predicate.value(), path, application, context)?;
            Ok(mapping.finish(value))
        })
    }

    pub(in crate::types::constraints::solution) fn reopen_effect_predicate<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        L,
    >(
        &self,
        predicate: &CompletedValue<EffectPredicate>,
        path: &ConstraintInferenceBranch<D, L>,
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
}

impl<'scope> ContextReferenceMap<'scope> {
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
    fn retain_result_predicate<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        E: RebindableEffects,
    >(
        &self,
        value: &mut TypeKind<E>,
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
        let mut retain = |reference: &GenericEffectReference| -> Result<(), TypeConstraintError> {
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
            Ok(())
        };
        for row in E::rows(effects) {
            for reference in row.variables()? {
                retain(reference)?;
            }
        }
        for reference in predicate.variables() {
            retain(reference)?;
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

    fn type_reference<A: TypeConstraintAccounting, D: ConstraintDomain, L>(
        &self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        path: &ConstraintInferenceBranch<D, L>,
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

    fn const_reference<A: TypeConstraintAccounting, D: ConstraintDomain, L>(
        &self,
        reference: &GenericConstReference,
        source: &GenericScope,
        target: &GenericScope,
        path: &ConstraintInferenceBranch<D, L>,
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

    fn effect_reference<A: TypeConstraintAccounting, D: ConstraintDomain, L>(
        &self,
        reference: &GenericEffectReference,
        source: &GenericScope,
        target: &GenericScope,
        path: &ConstraintInferenceBranch<D, L>,
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

    fn length<A: TypeConstraintAccounting, D: ConstraintDomain, L>(
        &self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        path: &ConstraintInferenceBranch<D, L>,
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

    fn map_type<A: TypeConstraintAccounting, D: ConstraintDomain, L, E: RebindableEffects>(
        &self,
        ty: &TypeKind<E>,
        path: &ConstraintInferenceBranch<D, L>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<E>, TypeConstraintError> {
        ty.project_structure(
            self.source_scope(),
            self.target_scope(),
            1,
            &mut self.projection(path, application, context),
        )
        .map_err(fold_error)
    }

    fn map_row<A: TypeConstraintAccounting, D: ConstraintDomain, L>(
        &self,
        row: &EffectRow,
        path: &ConstraintInferenceBranch<D, L>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectRow, TypeConstraintError> {
        self.projection(path, application, context)
            .rename_row(row, self.source_scope(), self.target_scope(), 1)
            .map_err(fold_error)
    }

    fn map_predicate<A: TypeConstraintAccounting, D: ConstraintDomain, L>(
        &self,
        predicate: &EffectPredicate,
        path: &ConstraintInferenceBranch<D, L>,
        application: &ConstraintApplicationScope<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectPredicate, TypeConstraintError> {
        self.projection(path, application, context)
            .rename_predicate(predicate, self.source_scope(), self.target_scope(), 1)
            .map_err(fold_error)
    }

    fn projection<'a, 'work, A: TypeConstraintAccounting, D: ConstraintDomain, L>(
        &'a self,
        path: &'a ConstraintInferenceBranch<D, L>,
        application: &'a ConstraintApplicationScope<D>,
        context: &'a mut TypeConstraintContext<'work, A, D>,
    ) -> CompletedTypeProjection<'a, 'scope, 'work, A, D, L> {
        CompletedTypeProjection {
            mapping: self,
            path,
            application,
            context,
        }
    }
}

struct CompletedTypeProjection<
    'a,
    'scope,
    'work,
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    L,
> {
    mapping: &'a ContextReferenceMap<'scope>,
    path: &'a ConstraintInferenceBranch<D, L>,
    application: &'a ConstraintApplicationScope<D>,
    context: &'a mut TypeConstraintContext<'work, A, D>,
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain, L>
    CompletedTypeProjection<'_, '_, '_, A, D, L>
{
    fn rename_row(
        &mut self,
        row: &EffectRow,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<EffectRow, TypeProjectionError<TypeConstraintError>> {
        visit_effect_row(self.context, row, depth)?;
        let (mapping, path, application) = (self.mapping, self.path, self.application);
        row.try_map_variables(
            &mut EffectProjectionControl {
                control: self.context,
                depth,
            },
            &mut |reference, control| {
                mapping
                    .effect_reference(
                        reference,
                        source,
                        target,
                        path,
                        application,
                        depth == 2,
                        control.control,
                    )
                    .map_err(TypeProjectionError::Control)
            },
        )
    }

    fn rename_predicate(
        &mut self,
        predicate: &EffectPredicate,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<EffectPredicate, TypeProjectionError<TypeConstraintError>> {
        visit_effect_predicate(self.context, predicate, depth)?;
        let (mapping, path, application) = (self.mapping, self.path, self.application);
        predicate.map_references(
            &mut EffectProjectionControl {
                control: self.context,
                depth,
            },
            &mut |reference, control| {
                mapping
                    .effect_reference(
                        reference,
                        source,
                        target,
                        path,
                        application,
                        depth == 2,
                        control.control,
                    )
                    .map_err(TypeProjectionError::Control)
            },
        )
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain, L> TypeProjectionControl
    for CompletedTypeProjection<'_, '_, '_, A, D, L>
{
    type Error = TypeConstraintError;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.context.check()
    }

    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.context.visit_node(kind, depth)
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.context.visit_binding()
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain, L, E: RebindableEffects>
    ScopedTypeProjection<E, E> for CompletedTypeProjection<'_, '_, '_, A, D, L>
{
    fn admit_shape(
        &mut self,
        shape: TypeConstraintShape<'_, E>,
    ) -> Result<(), TypeProjectionError<Self::Error>> {
        if matches!(shape, TypeConstraintShape::Unresolved(_)) {
            return Err(TypeProjectionError::Control(
                TypeConstraintRejection::UnresolvedType.into(),
            ));
        }
        Ok(())
    }

    fn project_type_reference(
        &mut self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<TypeKind<E>, TypeProjectionError<Self::Error>> {
        self.mapping
            .type_reference(
                reference,
                source,
                target,
                self.path,
                self.application,
                self.context,
            )
            .map(TypeKind::GenericParam)
            .map_err(TypeProjectionError::Control)
    }

    fn project_length(
        &mut self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<ArrayLength, TypeProjectionError<Self::Error>> {
        let depth = child_depth(depth)?;
        self.check().map_err(TypeProjectionError::Control)?;
        self.visit_node(TypeProjectionNodeKind::Const, depth)
            .map_err(TypeProjectionError::Control)?;
        self.mapping
            .length(
                length,
                source,
                target,
                self.path,
                self.application,
                self.context,
            )
            .map_err(TypeProjectionError::Control)
    }

    fn project_row(
        &mut self,
        row: &E::Row,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<E::Row, TypeProjectionError<Self::Error>> {
        let depth = child_depth(depth)?;
        E::map_row(row, |row| self.rename_row(row, source, target, depth))
    }

    fn project_predicate(
        &mut self,
        predicate: &EffectPredicate,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<EffectPredicate, TypeProjectionError<Self::Error>> {
        self.rename_predicate(predicate, source, target, child_depth(depth)?)
    }
}
