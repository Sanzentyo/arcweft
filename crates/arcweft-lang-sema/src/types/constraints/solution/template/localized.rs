//! Projection of selected argument rows in the lexical context that supplied them.

use std::sync::Arc;

use crate::{
    effect_row::EffectRow,
    types::{
        ArrayLength, GenericConstReference, GenericScope, GenericTypeReference,
        ScopedArrayLengthView, ScopedEffectRowView, ScopedType, ScopedTypeView, TypeKind,
        TypeProjectionControl, TypeProjectionError,
    },
};

use super::{TypeConstraintSolution, TypeInstantiationError};

pub(crate) trait TypeTemplateLocalizer<C: TypeProjectionControl> {
    fn type_term(
        &self,
        term: ScopedTypeView<'_>,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>>;
    fn const_term(
        &self,
        term: ScopedArrayLengthView<'_>,
        control: &mut C,
    ) -> Result<ArrayLength, TypeProjectionError<C::Error>>;
    fn effect_term(
        &self,
        term: ScopedEffectRowView<'_>,
        control: &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>>;
}

/// A projection of one completed application. The application still owns
/// the keys, residual slots and their order. Only the replacement terms have
/// been projected through the lexical context that supplied them.
pub(crate) struct LocalizedTypeTemplate {
    solution: Arc<TypeConstraintSolution>,
    values: LocalizedValues,
}

pub(super) struct LocalizedValues {
    types: Box<[TypeKind]>,
    consts: Box<[ArrayLength]>,
    effects: Box<[EffectRow]>,
}

impl LocalizedTypeTemplate {
    pub(crate) fn new<C: TypeProjectionControl>(
        solution: Arc<TypeConstraintSolution>,
        localizer: &impl TypeTemplateLocalizer<C>,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        let types = solution
            .bindings()
            .map(|(_, term)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                let projected = localizer.type_term(term, control)?;
                if projected.view().scope() != term.scope() {
                    return Err(TypeProjectionError::Instantiation(
                        TypeInstantiationError::SpecializationScopeMismatch,
                    ));
                }
                Ok(projected.into_value())
            })
            .collect::<Result<_, _>>()?;
        let consts = solution
            .const_bindings()
            .map(|(_, term)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                localizer.const_term(term, control)
            })
            .collect::<Result<_, _>>()?;
        let effects = solution
            .effect_bindings()
            .map(|(_, term)| {
                control
                    .visit_binding()
                    .map_err(TypeProjectionError::Control)?;
                localizer.effect_term(term, control)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            solution,
            values: LocalizedValues {
                types,
                consts,
                effects,
            },
        })
    }

    pub(crate) fn source_scope(&self) -> &GenericScope {
        self.solution.template_scope()
    }

    pub(crate) fn result_scope(&self) -> &GenericScope {
        self.solution.context.value_scope()
    }

    pub(crate) fn body_type_parameters(&self) -> impl Iterator<Item = &GenericTypeReference> {
        self.solution
            .bindings
            .iter()
            .map(|row| &row.parameter)
            .chain(self.solution.context.residual.type_origins())
    }

    pub(crate) fn body_const_parameters(&self) -> impl Iterator<Item = &GenericConstReference> {
        self.solution
            .const_bindings
            .iter()
            .map(|row| &row.parameter)
            .chain(self.solution.context.residual.const_origins())
    }

    pub(crate) fn project_type<C: TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<ScopedType, TypeProjectionError<C::Error>> {
        self.solution
            .template_projection(Some(&self.values))
            .project_type(ty, under, control)
    }

    pub(crate) fn project_length<C: TypeProjectionControl>(
        &self,
        length: &ArrayLength,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<ArrayLength, TypeProjectionError<C::Error>> {
        self.solution
            .template_projection(Some(&self.values))
            .project_length(length, under, control)
    }

    pub(crate) fn project_effects<C: TypeProjectionControl>(
        &self,
        row: &EffectRow,
        under: &GenericScope,
        control: &mut C,
    ) -> Result<EffectRow, TypeProjectionError<C::Error>> {
        self.solution
            .template_projection(Some(&self.values))
            .project_effects(row, under, control)
    }
}

pub(super) struct TemplateProjection<'a> {
    pub(super) solution: &'a TypeConstraintSolution,
    pub(super) values: Option<&'a LocalizedValues>,
}

impl TemplateProjection<'_> {
    pub(super) fn type_value(&self, index: usize) -> &TypeKind {
        self.values.map_or_else(
            || &self.solution.bindings[index].value,
            |values| &values.types[index],
        )
    }

    pub(super) fn const_value(&self, index: usize) -> &ArrayLength {
        self.values.map_or_else(
            || &self.solution.const_bindings[index].value,
            |values| &values.consts[index],
        )
    }

    pub(super) fn effect_value(&self, index: usize) -> &EffectRow {
        self.values.map_or_else(
            || &self.solution.effect_bindings[index].value,
            |values| &values.effects[index],
        )
    }
}
