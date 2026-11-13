//! Capture-avoiding application of completed rows to declaration templates.

use crate::effect_row::{EffectPredicate, EffectRow};
use crate::types::{
    ArrayLength, GenericBinder, GenericConstReference, GenericDeclarationBinder,
    GenericEffectReference, GenericParameterKind, GenericScope, GenericScopeError,
    GenericTypeReference, ScopedType, ScopedTypeView, TypeKind, TypeProjectionControl,
    TypeProjectionError, TypeProjectionNodeKind,
};

use super::super::shape::{TypeConstraintChildren, TypeConstraintShape};
use crate::types::projection_control::{EffectProjectionControl, UnmeteredTypeProjection};
use crate::types::projection_control::{visit_effect_predicate, visit_effect_row};

use super::{TypeConstraintSolution, TypeInstantiationError};

mod localized;
pub(crate) use localized::{LocalizedTypeTemplate, TypeTemplateLocalizer};

impl EffectRow {
    pub(crate) fn clone_with_projection_control<C: TypeProjectionControl>(
        &self,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        visit_effect_row(control, self, 1)?;
        Ok(self.clone())
    }
}

impl TypeConstraintSolution {
    /// A returned callable carries the exact residual predicate owned by the
    /// same solution that projects its parameter and result terms.
    pub(crate) fn apply_result_template(
        &self,
        ty: &TypeKind,
    ) -> Result<TypeKind, TypeInstantiationError> {
        self.apply_result_template_with_control(ty, &mut UnmeteredTypeProjection)
            .map_err(TypeProjectionError::into_instantiation)
    }

    pub(crate) fn apply_result_template_with_control<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>> {
        let value = self.apply_contextual_result_template_with_control(ty, control)?;
        if self.context.ambient.shape().binders().is_empty() {
            return Ok(value.into_value());
        }
        clone_scoped_term_with_control(value.view().value(), &GenericScope::default(), 1, control)
    }

    /// Generalize the continuation slots only. Ambient frames stay in the
    /// returned scope until an enclosing instantiation supplies their values.
    pub(crate) fn apply_contextual_result_template_with_control<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>> {
        self.apply_template_with_control(ty, control)?
            .view()
            .quantify_suffix_with_predicate_and_control(
                self.effect_predicate.value(),
                self.context.ambient.shape(),
                control,
            )
    }

    /// Template references name this solution's formal parameters. Replacement
    /// values are caller-owned: their free declarations are never looked up in
    /// this solution again, including same-declaration recursive applications.
    pub(crate) fn apply_template(
        &self,
        ty: &TypeKind,
    ) -> Result<ScopedType, TypeInstantiationError> {
        self.apply_template_with_control(ty, &mut UnmeteredTypeProjection)
            .map_err(TypeProjectionError::into_instantiation)
    }

    pub(crate) fn apply_template_with_control<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>> {
        self.apply_template_under(ty, &GenericScope::default(), control)
    }

    /// Projects the arrow consumed by one physical application. The caller
    /// must join a residual result with the selected code origin's retaining
    /// group before it may issue an executable specialization.
    pub(crate) fn apply_call_input_template_with_control<C: TypeProjectionControl>(
        &self,
        template: &TypeKind,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>> {
        let projected = self.apply_template_with_control(template, control)?;
        let scope = projected.view().scope().clone();
        if scope.binders().is_empty() {
            return projected.view().to_quantified_type_with_control(control);
        }
        let TypeKind::Function {
            binder,
            params,
            return_type,
            effects,
            ..
        } = projected.into_value()
        else {
            return Err(TypeInstantiationError::UnresolvedType.into());
        };
        if !binder.is_empty() {
            return Err(TypeInstantiationError::SpecializationScopeMismatch.into());
        }
        let result = ScopedTypeView::sealed(&return_type, &scope)
            .quantify_suffix_with_predicate_and_control(
                self.effect_predicate.value(),
                self.context.ambient.shape(),
                control,
            )?;
        let arrow = TypeKind::function_with_effects(params, result.into_value(), effects);
        clone_scoped_term_with_control(&arrow, &GenericScope::default(), 1, control)
    }

    pub(crate) fn apply_template_under<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>> {
        self.template_projection(None)
            .project_type(ty, under, control)
    }

    fn template_projection<'a>(
        &'a self,
        values: Option<&'a localized::LocalizedValues>,
    ) -> localized::TemplateProjection<'a> {
        localized::TemplateProjection {
            solution: self,
            values,
        }
    }

    pub(crate) fn apply_length_under<C: TypeProjectionControl>(
        &self,
        length: &ArrayLength,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<ArrayLength, TypeProjectionError<C::Error>> {
        self.template_projection(None)
            .project_length(length, under, control)
    }

    pub(crate) fn apply_effects_under<C: TypeProjectionControl>(
        &self,
        row: &EffectRow,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>> {
        self.template_projection(None)
            .project_effects(row, under, control)
    }

    /// Invocation rows must already be known. A caller-owned symbolic row
    /// may survive projection until the enclosing instance is selected.
    pub(crate) fn apply_call_effects(
        &self,
        row: &EffectRow,
    ) -> Result<EffectRow, TypeInstantiationError> {
        self.apply_call_effects_with_control(row, &mut UnmeteredTypeProjection)
            .map_err(TypeProjectionError::into_instantiation)
    }

    pub(crate) fn apply_call_effects_with_control<C: TypeProjectionControl>(
        &self,
        row: &EffectRow,
        control: &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>> {
        if !row.is_known() {
            return Err(crate::effect_row::EffectRowError::UnknownRow.into());
        }
        self.apply_effects_under(row, &GenericScope::default(), control)
    }
}

impl localized::TemplateProjection<'_> {
    fn scopes(&self, under: &GenericScope) -> (GenericScope, GenericScope) {
        scoped_pair(
            self.solution.template_scope(),
            self.solution.context.value_scope(),
            under,
        )
    }

    pub(super) fn project_type<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>> {
        let residual = self.solution.context.value_scope();
        let template = self.solution.template_scope();
        let (source_scope, target_scope) = self.scopes(under);
        let value = map_term_with_control(
            ty,
            &source_scope,
            &target_scope,
            1,
            control,
            &|reference, source, target, depth, control| {
                if let Some(parameter) = reference.template_key(template, source)? {
                    if let Ok(index) = self
                        .solution
                        .bindings
                        .binary_search_by(|row| row.parameter.cmp(&parameter))
                    {
                        let value = ScopedTypeView::sealed(self.type_value(index), residual);
                        return lift_value(value, target, depth, control);
                    }
                    if let Some(slot) = self.solution.context.residual.type_slot(&parameter) {
                        return Ok(TypeKind::GenericParam(
                            target.bound_type(depth_difference(target, residual)?, slot)?,
                        ));
                    }
                    if matches!(parameter, GenericTypeReference::Bound(_)) {
                        return Err(TypeInstantiationError::UnboundType { parameter }.into());
                    }
                }
                keep_type(reference, source).map_err(Into::into)
            },
            &|length, source, target, _, _| {
                self.apply_length_at(length, source, target)
                    .map_err(Into::into)
            },
            &|row, source, target, depth, control| {
                self.project_effects_at(row, source, target, depth, control)
            },
        )?;
        Ok(ScopedType::new(value, target_scope))
    }

    fn project_effects_at<C: TypeProjectionControl>(
        &self,
        row: &EffectRow,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
        control: &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>> {
        let template = self.solution.template_scope();
        let residual = self.solution.context.value_scope();
        map_effects_with_control(row, depth, control, &|reference, control| {
            if let Some(parameter) = reference.template_key(template, source)? {
                if let Ok(index) = self
                    .solution
                    .effect_bindings
                    .binary_search_by(|row| row.variable.cmp(&parameter))
                {
                    return lift_effects(
                        self.effect_value(index),
                        residual,
                        target,
                        depth,
                        control,
                    );
                }
                if let Some(slot) = self.solution.context.residual.effect_slot(&parameter) {
                    return Ok(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        target.bound_effect(depth_difference(target, residual)?, slot)?,
                    ));
                }
                if matches!(parameter, GenericEffectReference::Bound(_)) {
                    return Err(TypeInstantiationError::UnboundEffect { parameter }.into());
                }
            }
            Ok(EffectRow::open(
                crate::effects::EffectSet::new(),
                keep_effect(reference, source)?,
            ))
        })
    }

    pub(super) fn project_effects<C: TypeProjectionControl>(
        &self,
        row: &EffectRow,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>> {
        let (source, target) = self.scopes(under);
        self.project_effects_at(row, &source, &target, 1, control)
    }

    pub(super) fn project_length<C: TypeProjectionControl>(
        &self,
        length: &ArrayLength,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<ArrayLength, TypeProjectionError<C::Error>> {
        control
            .visit_node(TypeProjectionNodeKind::Const, 1)
            .map_err(TypeProjectionError::Control)?;
        let (source, target) = self.scopes(under);
        self.apply_length_at(length, &source, &target)
            .map_err(Into::into)
    }

    fn apply_length_at(
        &self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
    ) -> Result<ArrayLength, TypeInstantiationError> {
        let ArrayLength::Generic(reference) = length else {
            return match length {
                ArrayLength::Const(_) => Ok(length.clone()),
                _ => Err(TypeInstantiationError::UnresolvedType),
            };
        };
        let template = self.solution.template_scope();
        let residual = self.solution.context.value_scope();
        if let Some(parameter) = reference.template_key(template, source)? {
            if let Ok(index) = self
                .solution
                .const_bindings
                .binary_search_by(|row| row.parameter.cmp(&parameter))
            {
                return lift_length(self.const_value(index), residual, target);
            }
            if let Some(slot) = self.solution.context.residual.const_slot(&parameter) {
                return Ok(ArrayLength::Generic(
                    target.bound_const(depth_difference(target, residual)?, slot)?,
                ));
            }
            if matches!(parameter, GenericConstReference::Bound(_)) {
                return Err(TypeInstantiationError::UnboundConst { parameter });
            }
        }
        keep_const(reference, source)
    }
}

fn scoped_pair(
    source: &GenericScope,
    target: &GenericScope,
    under: &GenericScope,
) -> (GenericScope, GenericScope) {
    under.binders().iter().fold(
        (source.clone(), target.clone()),
        |(source, target), binder| (source.with_binder(*binder), target.with_binder(*binder)),
    )
}

impl GenericDeclarationBinder {
    pub(crate) fn project_length_under<C: TypeProjectionControl>(
        &self,
        length: &ArrayLength,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<ArrayLength, TypeProjectionError<C::Error>> {
        control
            .visit_node(TypeProjectionNodeKind::Const, 1)
            .map_err(TypeProjectionError::Control)?;
        let (source, target) = scoped_pair(self.template_scope(), self.scope(), under);
        self.project_length_at(length, &source, &target, control)
    }

    fn project_length_at<C: TypeProjectionControl>(
        &self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        control: &mut C,
    ) -> Result<ArrayLength, TypeProjectionError<C::Error>> {
        let ArrayLength::Generic(reference) = length else {
            return match length {
                ArrayLength::Const(_) => Ok(length.clone()),
                _ => Err(TypeInstantiationError::UnresolvedType.into()),
            };
        };
        control
            .visit_binding()
            .map_err(TypeProjectionError::Control)?;
        if let Some(key) = reference.template_key(self.template_scope(), source)?
            && let Some(slot) = self
                .const_parameters()
                .iter()
                .position(|candidate| candidate == &key)
        {
            let slot = u16::try_from(slot).map_err(|_| GenericScopeError::BinderArityOverflow {
                kind: GenericParameterKind::Const,
                count: self.const_parameters().len(),
            })?;
            return Ok(ArrayLength::Generic(
                target.bound_const(depth_difference(target, self.scope())?, slot)?,
            ));
        }
        keep_const(reference, source).map_err(Into::into)
    }

    pub(crate) fn project_with_control<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>> {
        self.project_under(ty, &GenericScope::default(), control)
    }

    pub(crate) fn project_under<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>> {
        let arity = |kind, count| GenericScopeError::BinderArityOverflow { kind, count };
        let (source_scope, target_scope) = scoped_pair(self.template_scope(), self.scope(), under);
        let value = map_term_with_control(
            ty,
            &source_scope,
            &target_scope,
            1,
            control,
            &|reference, source, target, _, control| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                if let Some(key) = reference.template_key(self.template_scope(), source)?
                    && let Some(slot) = self
                        .type_parameters()
                        .iter()
                        .position(|candidate| candidate == &key)
                {
                    return Ok(TypeKind::GenericParam(target.bound_type(
                        depth_difference(target, self.scope())?,
                        u16::try_from(slot).map_err(|_| {
                            arity(GenericParameterKind::Type, self.type_parameters().len())
                        })?,
                    )?));
                }
                keep_type(reference, source).map_err(Into::into)
            },
            &|length, source, target, _, control| {
                self.project_length_at(length, source, target, control)
            },
            &|row, source, target, depth, control| {
                self.project_effects_at(row, source, target, depth, control)
            },
        )?;
        Ok(ScopedType::new(value, target_scope))
    }

    pub(crate) fn project_effects_under<C: TypeProjectionControl>(
        &self,
        row: &EffectRow,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>> {
        let (source, target) = scoped_pair(self.template_scope(), self.scope(), under);
        self.project_effects_at(row, &source, &target, 1, control)
    }

    fn project_effects_at<C: TypeProjectionControl>(
        &self,
        row: &EffectRow,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
        control: &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>> {
        map_effects_with_control(row, depth, control, &|reference, control| {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            let reference = if let Some(key) =
                reference.template_key(self.template_scope(), source)?
                && let Some(slot) = self
                    .effect_parameters()
                    .iter()
                    .position(|candidate| candidate == &key)
            {
                target.bound_effect(
                    depth_difference(target, self.scope())?,
                    u32::try_from(slot).map_err(|_| GenericScopeError::BinderArityOverflow {
                        kind: GenericParameterKind::Effect,
                        count: self.effect_parameters().len(),
                    })?,
                )?
            } else {
                keep_effect(reference, source)?
            };
            Ok(EffectRow::open(crate::effects::EffectSet::new(), reference))
        })
    }

    pub(crate) fn quantify_function_with_control<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>> {
        self.project_with_control(ty, control)?
            .view()
            .to_quantified_type_with_control(control)
    }
}

impl ScopedTypeView<'_> {
    pub(crate) fn clone_with_control<C: TypeProjectionControl>(
        self,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>> {
        let value = clone_scoped_term_with_control(self.value(), self.scope(), 1, control)?;
        Ok(ScopedType::new(value, self.scope().clone()))
    }

    /// Closes a value projection without granting it the solution's residual
    /// quantifiers. Unused incoming scope can disappear only after this walk
    /// proves every retained bound reference has an owner in the value itself.
    pub(crate) fn to_root_type(self) -> Result<TypeKind, TypeInstantiationError> {
        let root = GenericScope::default();
        map_term(
            self.value(),
            &root,
            &root,
            &|reference, source, _| keep_type(reference, source),
            &|reference, source, _| keep_const(reference, source),
            &|row, source, _| {
                map_effects(row, &|reference| {
                    Ok(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        keep_effect(reference, source)?,
                    ))
                })
            },
        )
    }

    /// Transfers incoming quantifiers to the returned function itself. All
    /// existing function-local binders are preserved, with depth and slot
    /// remapping when the incoming and root function binders are combined.
    pub(crate) fn to_quantified_type(self) -> Result<TypeKind, TypeInstantiationError> {
        self.to_quantified_type_with_predicate(&EffectPredicate::unconstrained())
    }

    fn to_quantified_type_with_predicate(
        self,
        incoming_predicate: &EffectPredicate,
    ) -> Result<TypeKind, TypeInstantiationError> {
        self.quantify_with_predicate_and_control(incoming_predicate, &mut UnmeteredTypeProjection)
            .map_err(TypeProjectionError::into_instantiation)
    }

    pub(crate) fn to_quantified_type_with_control<C: TypeProjectionControl>(
        self,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>> {
        self.quantify_with_predicate_and_control(&EffectPredicate::unconstrained(), control)
    }

    fn quantify_with_predicate_and_control<C: TypeProjectionControl>(
        self,
        incoming_predicate: &EffectPredicate,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>> {
        self.quantify_suffix_with_predicate_and_control(
            incoming_predicate,
            &GenericScope::default(),
            control,
        )
        .map(ScopedType::into_value)
    }

    pub(in crate::types::constraints::solution) fn quantify_suffix_with_predicate_and_control<
        C: TypeProjectionControl,
    >(
        self,
        incoming_predicate: &EffectPredicate,
        ambient: &GenericScope,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>> {
        control.check().map_err(TypeProjectionError::Control)?;
        if !self.scope().binders().starts_with(ambient.binders()) {
            return Err(TypeInstantiationError::SpecializationScopeMismatch.into());
        }
        let ambient_count = ambient.binders().len();
        if self.scope() == ambient {
            let mut value = clone_scoped_term_with_control(self.value(), ambient, 1, control)?;
            if !incoming_predicate.is_unconstrained() {
                let TypeKind::Function {
                    binder, predicate, ..
                } = &mut value
                else {
                    return Ok(ScopedType::new(value, ambient.clone()));
                };
                let source = ambient.with_binder(*binder);
                let lifted =
                    map_predicate_with_control(incoming_predicate, 1, control, &|reference, _| {
                        Ok(EffectRow::open(
                            crate::effects::EffectSet::new(),
                            lift_effect(
                                reference,
                                ambient,
                                &source,
                                ambient_count,
                                depth_difference(&source, ambient)?,
                            )?,
                        ))
                    })?;
                *predicate =
                    predicate.and(&lifted, &mut EffectProjectionControl { control, depth: 1 })?;
            }
            return Ok(ScopedType::new(value, ambient.clone()));
        }
        let TypeKind::Function {
            binder,
            predicate,
            params,
            return_type,
            effects,
        } = self.value()
        else {
            return Err(TypeInstantiationError::Residual {
                binder: merge_binders(&self.scope().binders()[ambient_count..])?,
            }
            .into());
        };
        let source = self.scope().with_binder(*binder);
        let merged = merge_binders(&source.binders()[ambient_count..])?;
        let target = ambient.with_binder(merged);
        let root_depth = source.binders().len();
        let type_map =
            |reference: &GenericTypeReference, source: &GenericScope, target: &GenericScope| {
                let GenericTypeReference::Bound(parameter) = reference else {
                    return keep_type(reference, source);
                };
                source.bound_type(parameter.depth(), parameter.slot())?;
                let (depth, slot) = fused_coordinate(
                    source,
                    target,
                    root_depth,
                    ambient_count,
                    parameter.depth(),
                    parameter.slot(),
                    GenericParameterKind::Type,
                )?;
                Ok(TypeKind::GenericParam(target.bound_type(depth, slot)?))
            };
        let const_map =
            |reference: &GenericConstReference, source: &GenericScope, target: &GenericScope| {
                let GenericConstReference::Bound(parameter) = reference else {
                    return keep_const(reference, source);
                };
                source.bound_const(parameter.depth(), parameter.slot())?;
                let (depth, slot) = fused_coordinate(
                    source,
                    target,
                    root_depth,
                    ambient_count,
                    parameter.depth(),
                    parameter.slot(),
                    GenericParameterKind::Const,
                )?;
                Ok(ArrayLength::Generic(target.bound_const(depth, slot)?))
            };
        let effect_reference = |reference: &GenericEffectReference,
                                source: &GenericScope,
                                target: &GenericScope|
         -> Result<EffectRow, TypeInstantiationError> {
            let GenericEffectReference::Bound(parameter) = reference else {
                return Ok(EffectRow::open(
                    crate::effects::EffectSet::new(),
                    keep_effect(reference, source)?,
                ));
            };
            source.bound_effect(parameter.depth(), parameter.slot())?;
            let local = source.binders().len() - root_depth;
            let (depth, slot) = if (parameter.depth() as usize) < local {
                (parameter.depth(), parameter.slot())
            } else {
                let owner = source.binders().len() - 1 - parameter.depth() as usize;
                if owner < ambient_count {
                    return Ok(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        target.bound_effect(
                            u32::try_from(target.binders().len() - 1 - owner)
                                .map_err(|_| TypeInstantiationError::DepthOverflow)?,
                            parameter.slot(),
                        )?,
                    ));
                }
                let offset = merge_binders(&source.binders()[ambient_count..owner])?.effects();
                let slot = parameter.slot().checked_add(offset).ok_or(
                    GenericScopeError::BinderArityOverflow {
                        kind: GenericParameterKind::Effect,
                        count: parameter.slot() as usize + offset as usize,
                    },
                )?;
                (
                    u32::try_from(target.binders().len() - ambient_count - 1).map_err(|_| {
                        GenericScopeError::UnknownDepth {
                            depth: parameter.depth(),
                        }
                    })?,
                    slot,
                )
            };
            Ok(EffectRow::open(
                crate::effects::EffectSet::new(),
                target.bound_effect(depth, slot)?,
            ))
        };
        let effect_map = |row: &EffectRow,
                          source: &GenericScope,
                          target: &GenericScope,
                          depth,
                          control: &mut C| {
            map_effects_with_control(row, depth, control, &|reference, _| {
                effect_reference(reference, source, target).map_err(Into::into)
            })
        };
        let project = |ty: &TypeKind, control: &mut C| {
            map_term_with_control(
                ty,
                &source,
                &target,
                2,
                control,
                &|reference, source, target, _, _| {
                    type_map(reference, source, target).map_err(Into::into)
                },
                &|length, source, target, _, _| match length {
                    ArrayLength::Generic(reference) => {
                        const_map(reference, source, target).map_err(Into::into)
                    }
                    ArrayLength::Const(_) => Ok(length.clone()),
                    _ => Err(TypeInstantiationError::UnresolvedType.into()),
                },
                &effect_map,
            )
        };
        let project_predicate = |predicate: &EffectPredicate, control: &mut C| {
            map_predicate_with_control(predicate, 1, control, &|reference, _| {
                effect_reference(reference, &source, &target).map_err(Into::into)
            })
        };
        let lifted_incoming =
            map_predicate_with_control(incoming_predicate, 1, control, &|reference, _| {
                Ok(EffectRow::open(
                    crate::effects::EffectSet::new(),
                    lift_effect(
                        reference,
                        self.scope(),
                        &source,
                        self.scope().binders().len(),
                        depth_difference(&source, self.scope())?,
                    )?,
                ))
            })?;
        let predicate = project_predicate(predicate, control)?.and(
            &project_predicate(&lifted_incoming, control)?,
            &mut EffectProjectionControl { control, depth: 1 },
        )?;
        control
            .visit_node(TypeProjectionNodeKind::Type, 1)
            .map_err(TypeProjectionError::Control)?;
        Ok(ScopedType::new(
            TypeKind::function_with_contract(
                merged,
                predicate,
                params
                    .iter()
                    .map(|ty| project(ty, control))
                    .collect::<Result<Vec<_>, _>>()?,
                project(return_type, control)?,
                effect_map(effects, &source, &target, 1, control)?,
            ),
            ambient.clone(),
        ))
    }
}
fn merge_binders(binders: &[GenericBinder]) -> Result<GenericBinder, TypeInstantiationError> {
    binders
        .iter()
        .try_fold(GenericBinder::EMPTY, |combined, next| {
            combined.checked_append(*next).map_err(Into::into)
        })
}
fn fused_coordinate(
    source: &GenericScope,
    target: &GenericScope,
    root_depth: usize,
    ambient_count: usize,
    depth: u32,
    slot: u16,
    kind: GenericParameterKind,
) -> Result<(u32, u16), TypeInstantiationError> {
    let local = source.binders().len() - root_depth;
    if (depth as usize) < local {
        return Ok((depth, slot));
    }
    let owner = source.binders().len() - 1 - depth as usize;
    if owner < ambient_count {
        return Ok((
            u32::try_from(target.binders().len() - 1 - owner)
                .map_err(|_| TypeInstantiationError::DepthOverflow)?,
            slot,
        ));
    }
    let prefix = merge_binders(&source.binders()[ambient_count..owner])?;
    let offset = match kind {
        GenericParameterKind::Type => prefix.types(),
        GenericParameterKind::Const => prefix.const_lengths(),
        GenericParameterKind::Effect => unreachable!("type and const coordinates use u16 slots"),
    };
    let slot = slot
        .checked_add(offset)
        .ok_or(GenericScopeError::BinderArityOverflow {
            kind,
            count: usize::from(slot) + usize::from(offset),
        })?;
    Ok((
        u32::try_from(target.binders().len() - ambient_count - 1)
            .map_err(|_| GenericScopeError::UnknownDepth { depth })?,
        slot,
    ))
}

fn depth_difference(
    target: &GenericScope,
    source: &GenericScope,
) -> Result<u32, TypeInstantiationError> {
    target
        .binders()
        .len()
        .checked_sub(source.binders().len())
        .and_then(|depth| u32::try_from(depth).ok())
        .ok_or_else(|| GenericScopeError::UnknownDepth { depth: u32::MAX }.into())
}

fn lift_value<C: TypeProjectionControl>(
    value: ScopedTypeView<'_>,
    target: &GenericScope,
    depth: u64,
    control: &mut C,
) -> Result<TypeKind, TypeProjectionError<C::Error>> {
    let inserted = depth_difference(target, value.scope())?;
    let root_depth = value.scope().binders().len();
    map_term_with_control(
        value.value(),
        value.scope(),
        target,
        depth,
        control,
        &|reference, source, target, _, _| {
            let GenericTypeReference::Bound(parameter) = reference else {
                return keep_type(reference, source).map_err(Into::into);
            };
            source.bound_type(parameter.depth(), parameter.slot())?;
            let local = source.binders().len() - root_depth;
            let depth = if (parameter.depth() as usize) < local {
                parameter.depth()
            } else {
                parameter
                    .depth()
                    .checked_add(inserted)
                    .ok_or(GenericScopeError::UnknownDepth {
                        depth: parameter.depth(),
                    })?
            };
            Ok(TypeKind::GenericParam(
                target.bound_type(depth, parameter.slot())?,
            ))
        },
        &|length, source, target, _, _| {
            let ArrayLength::Generic(reference) = length else {
                return match length {
                    ArrayLength::Const(_) => Ok(length.clone()),
                    _ => Err(TypeInstantiationError::UnresolvedType.into()),
                };
            };
            let GenericConstReference::Bound(parameter) = reference else {
                return keep_const(reference, source).map_err(Into::into);
            };
            source.bound_const(parameter.depth(), parameter.slot())?;
            let local = source.binders().len() - root_depth;
            let depth = if (parameter.depth() as usize) < local {
                parameter.depth()
            } else {
                parameter
                    .depth()
                    .checked_add(inserted)
                    .ok_or(GenericScopeError::UnknownDepth {
                        depth: parameter.depth(),
                    })?
            };
            Ok(ArrayLength::Generic(
                target.bound_const(depth, parameter.slot())?,
            ))
        },
        &|row, source, target, depth, control| {
            map_effects_with_control(row, depth, control, &|reference, _| {
                let reference = lift_effect(reference, source, target, root_depth, inserted)?;
                Ok(EffectRow::open(crate::effects::EffectSet::new(), reference))
            })
        },
    )
}

fn lift_length(
    length: &ArrayLength,
    source: &GenericScope,
    target: &GenericScope,
) -> Result<ArrayLength, TypeInstantiationError> {
    match length {
        ArrayLength::Generic(GenericConstReference::Bound(parameter)) => {
            source.bound_const(parameter.depth(), parameter.slot())?;
            let depth = parameter
                .depth()
                .checked_add(depth_difference(target, source)?)
                .ok_or(GenericScopeError::UnknownDepth {
                    depth: parameter.depth(),
                })?;
            Ok(ArrayLength::Generic(
                target.bound_const(depth, parameter.slot())?,
            ))
        }
        ArrayLength::Generic(reference) => keep_const(reference, source),
        ArrayLength::Const(_) => Ok(length.clone()),
        ArrayLength::Error(_) | ArrayLength::Inferred => {
            Err(TypeInstantiationError::UnresolvedType)
        }
    }
}

fn keep_type(
    reference: &GenericTypeReference,
    scope: &GenericScope,
) -> Result<TypeKind, TypeInstantiationError> {
    match reference {
        GenericTypeReference::Free(_) => Ok(TypeKind::GenericParam(reference.clone())),
        GenericTypeReference::Bound(parameter) => Ok(TypeKind::GenericParam(
            scope.bound_type(parameter.depth(), parameter.slot())?,
        )),
        GenericTypeReference::Inference(_) => Err(GenericScopeError::EscapedInference {
            kind: GenericParameterKind::Type,
        }
        .into()),
    }
}

fn keep_const(
    reference: &GenericConstReference,
    scope: &GenericScope,
) -> Result<ArrayLength, TypeInstantiationError> {
    match reference {
        GenericConstReference::Free(_) => Ok(ArrayLength::Generic(reference.clone())),
        GenericConstReference::Bound(parameter) => Ok(ArrayLength::Generic(
            scope.bound_const(parameter.depth(), parameter.slot())?,
        )),
        GenericConstReference::Inference(_) => Err(GenericScopeError::EscapedInference {
            kind: GenericParameterKind::Const,
        }
        .into()),
    }
}

pub(super) fn keep_effect(
    reference: &GenericEffectReference,
    scope: &GenericScope,
) -> Result<GenericEffectReference, TypeInstantiationError> {
    match reference {
        GenericEffectReference::Free(_) => Ok(reference.clone()),
        GenericEffectReference::Bound(parameter) => {
            Ok(scope.bound_effect(parameter.depth(), parameter.slot())?)
        }
        GenericEffectReference::Inference(_) => Err(GenericScopeError::EscapedInference {
            kind: GenericParameterKind::Effect,
        }
        .into()),
    }
}

fn lift_effect(
    reference: &GenericEffectReference,
    source: &GenericScope,
    target: &GenericScope,
    root_depth: usize,
    inserted: u32,
) -> Result<GenericEffectReference, TypeInstantiationError> {
    let GenericEffectReference::Bound(parameter) = reference else {
        return keep_effect(reference, source);
    };
    source.bound_effect(parameter.depth(), parameter.slot())?;
    let local = source.binders().len() - root_depth;
    let depth = if (parameter.depth() as usize) < local {
        parameter.depth()
    } else {
        parameter
            .depth()
            .checked_add(inserted)
            .ok_or(GenericScopeError::UnknownDepth {
                depth: parameter.depth(),
            })?
    };
    Ok(target.bound_effect(depth, parameter.slot())?)
}

fn lift_effects<C: TypeProjectionControl>(
    row: &EffectRow,
    source: &GenericScope,
    target: &GenericScope,
    depth: u64,
    control: &mut C,
) -> Result<EffectRow, TypeProjectionError<C::Error>> {
    let inserted = depth_difference(target, source)?;
    map_effects_with_control(row, depth, control, &|reference, _| {
        Ok(EffectRow::open(
            crate::effects::EffectSet::new(),
            lift_effect(reference, source, target, source.binders().len(), inserted)?,
        ))
    })
}

fn map_effects(
    row: &EffectRow,
    mapping: &impl Fn(&GenericEffectReference) -> Result<EffectRow, TypeInstantiationError>,
) -> Result<EffectRow, TypeInstantiationError> {
    map_effects_with_control(row, 1, &mut UnmeteredTypeProjection, &|reference, _| {
        mapping(reference).map_err(Into::into)
    })
    .map_err(TypeProjectionError::into_instantiation)
}

pub(in crate::types::constraints) fn map_predicate_with_control<C: TypeProjectionControl>(
    predicate: &EffectPredicate,
    depth: u64,
    control: &mut C,
    mapping: &impl Fn(
        &GenericEffectReference,
        &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>>,
) -> Result<EffectPredicate, TypeProjectionError<C::Error>> {
    visit_effect_predicate(control, predicate, depth)?;
    predicate.try_substitute_variables(
        &mut EffectProjectionControl { control, depth },
        &mut |reference, adapter| mapping(reference, adapter.control),
    )
}

pub(in crate::types::constraints) fn map_effects_with_control<C: TypeProjectionControl>(
    row: &EffectRow,
    depth: u64,
    control: &mut C,
    mapping: &impl Fn(
        &GenericEffectReference,
        &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>>,
) -> Result<EffectRow, TypeProjectionError<C::Error>> {
    visit_effect_row(control, row, depth)?;
    row.try_substitute_variables(
        &mut EffectProjectionControl { control, depth },
        &mut |reference, adapter| mapping(reference, adapter.control),
    )
}

pub(super) fn map_term(
    ty: &TypeKind,
    source: &GenericScope,
    target: &GenericScope,
    types: &impl Fn(
        &GenericTypeReference,
        &GenericScope,
        &GenericScope,
    ) -> Result<TypeKind, TypeInstantiationError>,
    consts: &impl Fn(
        &GenericConstReference,
        &GenericScope,
        &GenericScope,
    ) -> Result<ArrayLength, TypeInstantiationError>,
    effects: &impl Fn(
        &EffectRow,
        &GenericScope,
        &GenericScope,
    ) -> Result<EffectRow, TypeInstantiationError>,
) -> Result<TypeKind, TypeInstantiationError> {
    map_term_with_control(
        ty,
        source,
        target,
        1,
        &mut UnmeteredTypeProjection,
        &|reference, source, target, _, _| types(reference, source, target).map_err(Into::into),
        &|length, source, target, _, _| match length {
            ArrayLength::Generic(reference) => {
                consts(reference, source, target).map_err(Into::into)
            }
            ArrayLength::Const(_) => Ok(length.clone()),
            ArrayLength::Error(_) | ArrayLength::Inferred => {
                Err(TypeInstantiationError::UnresolvedType.into())
            }
        },
        &|row, source, target, _, _| effects(row, source, target).map_err(Into::into),
    )
    .map_err(TypeProjectionError::into_instantiation)
}

struct ProjectionFrame<'ty> {
    shape: TypeConstraintShape<'ty>,
    source: GenericScope,
    target: GenericScope,
    depth: u64,
    children: TypeConstraintChildren<'ty>,
    projected: Vec<TypeKind>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "one scoped fold owns source and target binders, accounting, and the three scalar projection operations"
)]
pub(in crate::types::constraints) fn map_term_with_control<C: TypeProjectionControl>(
    ty: &TypeKind,
    source: &GenericScope,
    target: &GenericScope,
    depth: u64,
    control: &mut C,
    types: &impl Fn(
        &GenericTypeReference,
        &GenericScope,
        &GenericScope,
        u64,
        &mut C,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>>,
    consts: &impl Fn(
        &ArrayLength,
        &GenericScope,
        &GenericScope,
        u64,
        &mut C,
    ) -> Result<ArrayLength, TypeProjectionError<C::Error>>,
    effects: &impl Fn(
        &EffectRow,
        &GenericScope,
        &GenericScope,
        u64,
        &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>>,
) -> Result<TypeKind, TypeProjectionError<C::Error>> {
    control.check().map_err(TypeProjectionError::Control)?;
    let mut frames = Vec::<ProjectionFrame<'_>>::new();
    let mut current = (ty, source.clone(), target.clone(), depth);
    loop {
        let (ty, source, target, depth) = current;
        control.check().map_err(TypeProjectionError::Control)?;
        control
            .visit_node(TypeProjectionNodeKind::Type, depth)
            .map_err(TypeProjectionError::Control)?;
        let mut value = if let TypeKind::GenericParam(reference) = ty {
            types(reference, &source, &target, depth, control)?
        } else {
            let shape = ty.constraint_shape();
            let mut frame = ProjectionFrame {
                shape,
                source: source.with_binder(shape.binder()),
                target: target.with_binder(shape.binder()),
                depth,
                children: shape.children(),
                projected: Vec::new(),
            };
            if let Some(child) = frame.children.next() {
                current = (
                    child,
                    frame.source.clone(),
                    frame.target.clone(),
                    child_depth(depth)?,
                );
                frames.push(frame);
                continue;
            }
            frame.finish(control, consts, effects)?
        };
        loop {
            let Some(mut parent) = frames.pop() else {
                return Ok(value);
            };
            parent.projected.push(value);
            if let Some(child) = parent.children.next() {
                current = (
                    child,
                    parent.source.clone(),
                    parent.target.clone(),
                    child_depth(parent.depth)?,
                );
                frames.push(parent);
                break;
            }
            value = parent.finish(control, consts, effects)?;
        }
    }
}

fn child_depth<E: std::error::Error + 'static>(depth: u64) -> Result<u64, TypeProjectionError<E>> {
    depth
        .checked_add(1)
        .ok_or_else(|| TypeInstantiationError::DepthOverflow.into())
}

impl ProjectionFrame<'_> {
    fn finish<C: TypeProjectionControl>(
        self,
        control: &mut C,
        consts: &impl Fn(
            &ArrayLength,
            &GenericScope,
            &GenericScope,
            u64,
            &mut C,
        ) -> Result<ArrayLength, TypeProjectionError<C::Error>>,
        effects: &impl Fn(
            &EffectRow,
            &GenericScope,
            &GenericScope,
            u64,
            &mut C,
        ) -> Result<EffectRow, TypeProjectionError<C::Error>>,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>> {
        self.shape.rebuild_with(
            self.projected,
            control,
            |control, length| {
                let depth = child_depth(self.depth)?;
                control.check().map_err(TypeProjectionError::Control)?;
                control
                    .visit_node(TypeProjectionNodeKind::Const, depth)
                    .map_err(TypeProjectionError::Control)?;
                consts(length, &self.source, &self.target, depth, control)
            },
            |control, row| {
                effects(
                    row,
                    &self.source,
                    &self.target,
                    child_depth(self.depth)?,
                    control,
                )
            },
            |control, predicate| {
                let depth = child_depth(self.depth)?;
                map_predicate_with_control(predicate, depth, control, &|reference, control| {
                    effects(
                        &EffectRow::open(crate::effects::EffectSet::new(), reference.clone()),
                        &self.source,
                        &self.target,
                        depth,
                        control,
                    )
                })
            },
        )
    }
}

/// Copies a rooted replacement without applying the callee's keys to it.
/// Every descendant is visited before copying; nested function binders keep
/// their own lexical coordinates.
pub(super) fn clone_term_with_control<C: TypeProjectionControl>(
    ty: &TypeKind,
    depth: u64,
    control: &mut C,
) -> Result<TypeKind, TypeProjectionError<C::Error>> {
    clone_scoped_term_with_control(ty, &GenericScope::default(), depth, control)
}

pub(in crate::types::constraints::solution) fn clone_scoped_term_with_control<
    C: TypeProjectionControl,
>(
    ty: &TypeKind,
    scope: &GenericScope,
    depth: u64,
    control: &mut C,
) -> Result<TypeKind, TypeProjectionError<C::Error>> {
    map_term_with_control(
        ty,
        scope,
        scope,
        depth,
        control,
        &|reference, scope, _, _, _| keep_type(reference, scope).map_err(Into::into),
        &|length, scope, _, _, _| match length {
            ArrayLength::Generic(reference) => keep_const(reference, scope).map_err(Into::into),
            ArrayLength::Const(_) => Ok(length.clone()),
            ArrayLength::Error(_) | ArrayLength::Inferred => {
                Err(TypeInstantiationError::UnresolvedType.into())
            }
        },
        &|row, source, _, depth, control| {
            map_effects_with_control(row, depth, control, &|reference, _| {
                Ok(EffectRow::open(
                    crate::effects::EffectSet::new(),
                    keep_effect(reference, source)?,
                ))
            })
        },
    )
}

impl TypeKind {
    /// Instantiates declaration parameters simultaneously, charging every
    /// occurrence before copying it. Replacements retain their own scope and
    /// are never substituted through the declaration a second time.
    pub(crate) fn instantiate_type_parameters_with_control<C: TypeProjectionControl>(
        &self,
        substitutions: &std::collections::BTreeMap<
            &crate::types::GenericTypeParameterId,
            &TypeKind,
        >,
        replacement_scope: &GenericScope,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        let root = GenericScope::default();
        map_term_with_control(
            self,
            &root,
            replacement_scope,
            1,
            control,
            &|reference, scope, target, depth, control| match reference {
                GenericTypeReference::Free(parameter) => {
                    control
                        .visit_binding()
                        .map_err(TypeProjectionError::Control)?;
                    let replacement = substitutions.get(parameter).ok_or_else(|| {
                        TypeInstantiationError::UnboundType {
                            parameter: reference.clone(),
                        }
                    })?;
                    lift_value(
                        ScopedTypeView::sealed(replacement, replacement_scope),
                        target,
                        depth,
                        control,
                    )
                }
                _ => keep_type(reference, scope).map_err(Into::into),
            },
            &|length, scope, _, _, _| match length {
                ArrayLength::Generic(reference) => keep_const(reference, scope).map_err(Into::into),
                ArrayLength::Const(_) => Ok(length.clone()),
                ArrayLength::Error(_) | ArrayLength::Inferred => {
                    Err(TypeInstantiationError::UnresolvedType.into())
                }
            },
            &|row, source, _, depth, control| {
                map_effects_with_control(row, depth, control, &|reference, _| {
                    Ok(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        keep_effect(reference, source)?,
                    ))
                })
            },
        )
    }
}
