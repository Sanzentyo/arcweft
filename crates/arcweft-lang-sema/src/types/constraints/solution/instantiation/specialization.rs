//! Composition of sealed residual solutions. No active constraint path or
//! inference identity crosses this boundary.

use std::collections::BTreeMap;

use super::{
    ArrayLength, CheckedConstArgumentBinding, CheckedEffectArgumentBinding,
    CheckedTypeArgumentBinding, ClosedTypeInstantiation, EffectRow, GenericBinder,
    GenericConstReference, GenericEffectReference, GenericScope, GenericTypeReference,
    TypeConstraintSolution, TypeInstantiationError, TypeKind, TypeProjectionControl,
    TypeProjectionError, TypeProjectionNodeKind, visit_effect_row,
};
use crate::effects::EffectSet;
use crate::types::constraints::solution::template;

#[cfg(test)]
mod tests;

impl TypeConstraintSolution {
    /// Substitute one closed source-binder environment into this prefix's
    /// residual rows, retaining the original declaration keys. Free references
    /// in those rows still belong to the caller, even in recursive instances.
    pub(crate) fn close_residual_with_control<C: TypeProjectionControl>(
        &self,
        arguments: &ClosedTypeInstantiation,
        enclosing: Option<&ClosedTypeInstantiation>,
        control: &mut C,
    ) -> Result<ClosedTypeInstantiation, TypeProjectionError<C::Error>> {
        control.check().map_err(TypeProjectionError::Control)?;
        if arguments.template_scope != *self.residual.scope() {
            return Err(TypeInstantiationError::SpecializationScopeMismatch.into());
        }
        let environment = arguments.with_enclosing_parameters(enclosing, control)?;
        let predicate = template::map_predicate_with_control(
            &self.effect_predicate,
            1,
            control,
            &|reference, control| {
                let value = environment.effect_binding(reference).ok_or_else(|| {
                    TypeInstantiationError::UnboundEffect {
                        parameter: reference.clone(),
                    }
                })?;
                visit_effect_row(control, value, 1)?;
                Ok(value.clone())
            },
        )?;
        if !predicate.is_unconstrained() {
            return Err(TypeInstantiationError::UnsatisfiedEffectConstraint.into());
        }

        let mut types = BTreeMap::new();
        for row in &self.bindings {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            types.insert(
                row.parameter.clone(),
                environment.project_type_with_control(
                    &row.value,
                    self.residual.scope(),
                    control,
                )?,
            );
        }
        for (slot, parameter) in (0u16..).zip(self.residual.type_origins()) {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            let reference = self.residual.scope().bound_type(0, slot)?;
            let value = environment
                .instantiate_type_with_control(&TypeKind::GenericParam(reference), control)?;
            if types.insert(parameter.clone(), value).is_some() {
                return Err(TypeInstantiationError::SpecializationConflict.into());
            }
        }
        let mut consts = BTreeMap::new();
        for row in &self.const_bindings {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            consts.insert(
                row.parameter.clone(),
                environment.instantiate_array_length_with_control(&row.value, control)?,
            );
        }
        for (slot, parameter) in (0u16..).zip(self.residual.const_origins()) {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            let reference = self.residual.scope().bound_const(0, slot)?;
            let value = environment
                .instantiate_array_length_with_control(&ArrayLength::Generic(reference), control)?;
            if consts.insert(parameter.clone(), value).is_some() {
                return Err(TypeInstantiationError::SpecializationConflict.into());
            }
        }
        let mut effects = BTreeMap::new();
        for row in &self.effect_bindings {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            effects.insert(
                row.variable.clone(),
                EffectRow::closed(
                    environment.project_effect_row_with_control(&row.value, 1, control)?,
                ),
            );
        }
        for (slot, parameter) in (0u32..).zip(self.residual.effect_origins()) {
            if effects.contains_key(parameter) {
                // A future effect row may already contain fixed effects in
                // addition to its residual variable; the row above owns both.
                continue;
            }
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            let reference = self.residual.scope().bound_effect(0, slot)?;
            effects.insert(
                parameter.clone(),
                EffectRow::closed(environment.project_effect_row_with_control(
                    &EffectRow::open(EffectSet::new(), reference),
                    1,
                    control,
                )?),
            );
        }
        Ok(ClosedTypeInstantiation {
            template_scope: self.template_scope().clone(),
            bindings: types
                .into_iter()
                .map(|(key, value)| CheckedTypeArgumentBinding::new(key, value))
                .collect(),
            const_bindings: consts
                .into_iter()
                .map(|(key, value)| CheckedConstArgumentBinding::new(key, value))
                .collect(),
            effect_bindings: effects
                .into_iter()
                .map(|(key, value)| CheckedEffectArgumentBinding::new(key, value))
                .collect(),
        })
    }

    /// Project the terminal application's already completed declaration rows
    /// back to the exact residual slots issued by this prefix. The forward
    /// composition is checked as well, so unrelated completed environments
    /// cannot authorize a specialization.
    pub(crate) fn residual_arguments_with_control<C: TypeProjectionControl>(
        &self,
        completed: &ClosedTypeInstantiation,
        enclosing: Option<&ClosedTypeInstantiation>,
        control: &mut C,
    ) -> Result<ClosedTypeInstantiation, TypeProjectionError<C::Error>> {
        control.check().map_err(TypeProjectionError::Control)?;
        if self.template_scope() != &completed.template_scope {
            return Err(TypeInstantiationError::SpecializationScopeMismatch.into());
        }
        let scope = self.residual.scope();
        let bindings = (0u16..)
            .zip(self.residual.type_origins())
            .map(|(slot, origin)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                Ok(CheckedTypeArgumentBinding::new(
                    scope.bound_type(0, slot)?,
                    completed.instantiate_type_with_control(
                        &TypeKind::GenericParam(origin.clone()),
                        control,
                    )?,
                ))
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let const_bindings = (0u16..)
            .zip(self.residual.const_origins())
            .map(|(slot, origin)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                Ok(CheckedConstArgumentBinding::new(
                    scope.bound_const(0, slot)?,
                    completed.instantiate_array_length_with_control(
                        &ArrayLength::Generic(origin.clone()),
                        control,
                    )?,
                ))
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let effect_bindings = (0u32..)
            .zip(self.residual.effect_origins())
            .map(|(slot, origin)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                Ok(CheckedEffectArgumentBinding::new(
                    scope.bound_effect(0, slot)?,
                    EffectRow::closed(completed.project_effect_row_with_control(
                        &EffectRow::open(EffectSet::new(), origin.clone()),
                        1,
                        control,
                    )?),
                ))
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let arguments = ClosedTypeInstantiation {
            template_scope: scope.clone(),
            bindings,
            const_bindings,
            effect_bindings,
        };
        let projected = self.close_residual_with_control(&arguments, enclosing, control)?;
        if projected != *completed {
            return Err(TypeInstantiationError::SpecializationConflict.into());
        }
        Ok(arguments)
    }
}

impl ClosedTypeInstantiation {
    /// Re-key source-binder arguments using the same declaration mapping that
    /// produced the source scheme and its parameter types.
    pub(crate) fn for_declaration_with_control<C: TypeProjectionControl>(
        &self,
        declaration: &crate::types::GenericDeclarationBinder,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        control.check().map_err(TypeProjectionError::Control)?;
        if &self.template_scope != declaration.scope() {
            return Err(TypeInstantiationError::SpecializationScopeMismatch.into());
        }
        let mut types = BTreeMap::new();
        for (slot, parameter) in (0u16..).zip(declaration.type_parameters()) {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            let value = self.instantiate_type_with_control(
                &TypeKind::GenericParam(declaration.scope().bound_type(0, slot)?),
                control,
            )?;
            if types.insert(parameter.clone(), value).is_some() {
                return Err(TypeInstantiationError::SpecializationConflict.into());
            }
        }
        let mut consts = BTreeMap::new();
        for (slot, parameter) in (0u16..).zip(declaration.const_parameters()) {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            let value = self.instantiate_array_length_with_control(
                &ArrayLength::Generic(declaration.scope().bound_const(0, slot)?),
                control,
            )?;
            if consts.insert(parameter.clone(), value).is_some() {
                return Err(TypeInstantiationError::SpecializationConflict.into());
            }
        }
        let mut effects = BTreeMap::new();
        for (slot, parameter) in (0u32..).zip(declaration.effect_parameters()) {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
            let value = EffectRow::closed(self.project_effect_row_with_control(
                &EffectRow::open(EffectSet::new(), declaration.scope().bound_effect(0, slot)?),
                1,
                control,
            )?);
            if effects.insert(parameter.clone(), value).is_some() {
                return Err(TypeInstantiationError::SpecializationConflict.into());
            }
        }
        Ok(Self {
            template_scope: declaration.template_scope().clone(),
            bindings: types
                .into_iter()
                .map(|(key, value)| CheckedTypeArgumentBinding::new(key, value))
                .collect(),
            const_bindings: consts
                .into_iter()
                .map(|(key, value)| CheckedConstArgumentBinding::new(key, value))
                .collect(),
            effect_bindings: effects
                .into_iter()
                .map(|(key, value)| CheckedEffectArgumentBinding::new(key, value))
                .collect(),
        })
    }

    /// Extract the source root's arguments from a terminal checked call.
    pub(crate) fn declaration_arguments_with_control<C: TypeProjectionControl>(
        &self,
        declaration: &crate::types::GenericDeclarationBinder,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        control.check().map_err(TypeProjectionError::Control)?;
        if &self.template_scope != declaration.template_scope() {
            return Err(TypeInstantiationError::SpecializationScopeMismatch.into());
        }
        let bindings = (0u16..)
            .zip(declaration.type_parameters())
            .map(|(slot, parameter)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                Ok(CheckedTypeArgumentBinding::new(
                    declaration.scope().bound_type(0, slot)?,
                    self.instantiate_type_with_control(
                        &TypeKind::GenericParam(parameter.clone()),
                        control,
                    )?,
                ))
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let const_bindings = (0u16..)
            .zip(declaration.const_parameters())
            .map(|(slot, parameter)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                Ok(CheckedConstArgumentBinding::new(
                    declaration.scope().bound_const(0, slot)?,
                    self.instantiate_array_length_with_control(
                        &ArrayLength::Generic(parameter.clone()),
                        control,
                    )?,
                ))
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let effect_bindings = (0u32..)
            .zip(declaration.effect_parameters())
            .map(|(slot, parameter)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                Ok(CheckedEffectArgumentBinding::new(
                    declaration.scope().bound_effect(0, slot)?,
                    EffectRow::closed(self.project_effect_row_with_control(
                        &EffectRow::open(EffectSet::new(), parameter.clone()),
                        1,
                        control,
                    )?),
                ))
            })
            .collect::<Result<Box<[_]>, TypeProjectionError<C::Error>>>()?;
        let arguments = Self {
            template_scope: declaration.scope().clone(),
            bindings,
            const_bindings,
            effect_bindings,
        };
        if arguments.for_declaration_with_control(declaration, control)? != *self {
            return Err(TypeInstantiationError::SpecializationConflict.into());
        }
        Ok(arguments)
    }

    /// Close a whole source function binder using this simultaneous argument
    /// environment. Nested function binders and their predicates remain local.
    pub(crate) fn specialize_function_with_control<C: TypeProjectionControl>(
        &self,
        source: &TypeKind,
        enclosing: Option<&ClosedTypeInstantiation>,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<C::Error>> {
        control.check().map_err(TypeProjectionError::Control)?;
        let TypeKind::Function {
            binder,
            predicate,
            params,
            return_type,
            effects,
        } = source
        else {
            return Err(TypeInstantiationError::SpecializationScopeMismatch.into());
        };
        if self.template_scope != GenericScope::default().with_binder(*binder) {
            return Err(TypeInstantiationError::SpecializationScopeMismatch.into());
        }
        let template = TypeKind::function_with_contract(
            GenericBinder::EMPTY,
            predicate.clone(),
            params.clone(),
            return_type.as_ref().clone(),
            effects.clone(),
        );
        let environment = self.with_enclosing_parameters(enclosing, control)?;
        let value = environment.instantiate_type_with_control(&template, control)?;
        if !matches!(&value, TypeKind::Function { predicate, .. } if predicate.is_unconstrained()) {
            return Err(TypeInstantiationError::UnsatisfiedEffectConstraint.into());
        }
        Ok(value)
    }

    /// Bound keys belong to the source scheme. Any free declaration keys in
    /// the same source belong to the enclosing instance, never the prefix's
    /// callee. Closed replacement values are copied without a second lookup.
    fn with_enclosing_parameters<C: TypeProjectionControl>(
        &self,
        enclosing: Option<&Self>,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        let mut types = BTreeMap::new();
        let mut consts = BTreeMap::new();
        let mut effects = BTreeMap::new();
        for environment in [Some(self), enclosing].into_iter().flatten() {
            let is_source = std::ptr::eq(environment, self);
            for row in &environment.bindings {
                if !is_source && !matches!(row.parameter, GenericTypeReference::Free(_)) {
                    continue;
                }
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                let value = template::clone_term_with_control(&row.value, 1, control)?;
                if types.insert(row.parameter.clone(), value).is_some() {
                    return Err(TypeInstantiationError::SpecializationConflict.into());
                }
            }
            for row in &environment.const_bindings {
                if !is_source && !matches!(row.parameter, GenericConstReference::Free(_)) {
                    continue;
                }
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                control
                    .visit_node(TypeProjectionNodeKind::Const, 1)
                    .map_err(TypeProjectionError::Control)?;
                if consts
                    .insert(row.parameter.clone(), row.value.clone())
                    .is_some()
                {
                    return Err(TypeInstantiationError::SpecializationConflict.into());
                }
            }
            for row in &environment.effect_bindings {
                if !is_source && !matches!(row.variable, GenericEffectReference::Free(_)) {
                    continue;
                }
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                visit_effect_row(control, &row.value, 1)?;
                if effects
                    .insert(row.variable.clone(), row.value.clone())
                    .is_some()
                {
                    return Err(TypeInstantiationError::SpecializationConflict.into());
                }
            }
        }
        Ok(Self {
            template_scope: self.template_scope.clone(),
            bindings: types
                .into_iter()
                .map(|(key, value)| CheckedTypeArgumentBinding::new(key, value))
                .collect(),
            const_bindings: consts
                .into_iter()
                .map(|(key, value)| CheckedConstArgumentBinding::new(key, value))
                .collect(),
            effect_bindings: effects
                .into_iter()
                .map(|(key, value)| CheckedEffectArgumentBinding::new(key, value))
                .collect(),
        })
    }
}
