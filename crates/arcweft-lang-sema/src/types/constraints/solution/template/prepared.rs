//! Bind declaration templates while retaining live invocation rows.

use super::map_predicate_with_control;
use crate::{
    effect_row::prepared::{
        PreparedEffectFailure, PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow,
        PreparedEffectTermEdit,
    },
    types::{
        ArrayLength, GenericDeclarationBinder, GenericScope, GenericTypeReference, PreparedEffects,
        PreparedTypeEffectProjectionFailure, TypeInstantiationError, TypeKind,
        TypeProjectionControl, TypeProjectionError, TypeProjectionNodeKind,
        projection_control::EffectProjectionControl, structural_projection::ScopedTypeProjection,
    },
};

impl GenericDeclarationBinder {
    /// A declaration arrow starts with an empty root binder. Bind its own
    /// formals in that arrow while leaving the scheme body owned by the graph.
    pub(crate) fn quantify_prepared_function<C: TypeProjectionControl>(
        &self,
        value: TypeKind<PreparedEffects>,
        graph: &mut PreparedEffectGraph,
        control: &mut C,
    ) -> Result<
        TypeKind<PreparedEffects>,
        TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>,
    > {
        let Some(&quantifiers) = self.scope().binders().last() else {
            return Ok(value);
        };
        let mut projection = DeclarationProjection {
            binder: self,
            edit: graph.edit_terms(),
            control,
        };
        let mut result =
            value.project_structure(self.template_scope(), self.scope(), 1, &mut projection)?;
        let TypeKind::Function { binder, .. } = &mut result else {
            return Err(TypeInstantiationError::SpecializationScopeMismatch.into());
        };
        if !binder.is_empty() {
            return Err(TypeInstantiationError::SpecializationScopeMismatch.into());
        }
        *binder = quantifiers;
        projection
            .edit
            .commit()
            .map_err(|error| PreparedTypeEffectProjectionFailure::from_effect(error.into()))?;
        Ok(result)
    }
}

struct DeclarationProjection<'a, 'g, C> {
    binder: &'a GenericDeclarationBinder,
    edit: PreparedEffectTermEdit<'g>,
    control: &'a mut C,
}

impl<C: TypeProjectionControl> TypeProjectionControl for DeclarationProjection<'_, '_, C> {
    type Error = PreparedTypeEffectProjectionFailure<C::Error>;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.control
            .check()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }

    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.control
            .visit_node(kind, depth)
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.control
            .visit_binding()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
}

impl<C: TypeProjectionControl> ScopedTypeProjection<PreparedEffects, PreparedEffects>
    for DeclarationProjection<'_, '_, C>
{
    fn project_type_reference(
        &mut self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<TypeKind<PreparedEffects>, TypeProjectionError<Self::Error>> {
        self.binder
            .project_type_at(reference, source, target, self.control)
            .map_err(PreparedTypeEffectProjectionFailure::from_projection)
    }

    fn project_length(
        &mut self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<ArrayLength, TypeProjectionError<Self::Error>> {
        self.control
            .visit_node(TypeProjectionNodeKind::Const, depth)
            .map_err(|error| {
                TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Control(error))
            })?;
        self.binder
            .project_length_at(length, source, target, self.control)
            .map_err(PreparedTypeEffectProjectionFailure::from_projection)
    }

    fn project_row(
        &mut self,
        row: &PreparedEffectRow,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Self::Error>> {
        if let PreparedEffectRow::Canonical(row) = row {
            return self
                .binder
                .project_effects_at(row, source, target, depth, self.control)
                .map(PreparedEffectRow::Canonical)
                .map_err(PreparedTypeEffectProjectionFailure::from_projection);
        }
        self.edit
            .map_row_parameters(
                row,
                &mut EffectProjectionControl {
                    control: self.control,
                    depth,
                },
                &mut |reference, adapter| {
                    self.binder
                        .project_effect_at(reference, source, target, adapter.control)
                        .map(|row| Some(PreparedEffectRow::Canonical(row)))
                        .map_err(PreparedEffectFailure::Control)
                },
            )
            .map_err(PreparedTypeEffectProjectionFailure::from_effect)
    }

    fn project_predicate(
        &mut self,
        predicate: &PreparedEffectPredicate,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        if let PreparedEffectPredicate::Canonical(predicate) = predicate {
            return map_predicate_with_control(
                predicate,
                depth,
                self.control,
                &|reference, control| {
                    self.binder
                        .project_effect_at(reference, source, target, control)
                },
            )
            .map(PreparedEffectPredicate::Canonical)
            .map_err(PreparedTypeEffectProjectionFailure::from_projection);
        }
        self.edit
            .map_predicate_parameters(
                predicate,
                &mut EffectProjectionControl {
                    control: self.control,
                    depth,
                },
                &mut |reference, adapter| {
                    self.binder
                        .project_effect_at(reference, source, target, adapter.control)
                        .map(|row| Some(PreparedEffectRow::Canonical(row)))
                        .map_err(PreparedEffectFailure::Control)
                },
            )
            .map_err(PreparedTypeEffectProjectionFailure::from_effect)
    }
}
