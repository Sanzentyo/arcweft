//! Open the declaration's scalar ports through the shared prepared grammar.

use super::*;
use crate::{
    effect_row::{
        EffectRow,
        prepared::{
            PreparedEffectFailure, PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow,
            PreparedEffectTermEdit,
        },
    },
    types::{
        PreparedEffects, TypeProjectionControl, TypeProjectionError, TypeProjectionNodeKind,
        constraints::lexical::ConstraintLexicalScope, structural_projection::ScopedTypeProjection,
    },
};

pub(in crate::types::constraints) fn map_type<A: TypeConstraintAccounting, D: ConstraintDomain>(
    ty: &TypeKind<PreparedEffects>,
    mapping: &impl ConstraintReferenceMap,
    application: &ConstraintApplicationScope<D>,
    path: &ConstraintPath<D>,
    target: &ConstraintLexicalScope,
    graph: &mut PreparedEffectGraph,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<TypeKind<PreparedEffects>, TypeConstraintError> {
    let mut projection = Opening {
        references: ScopedTemplateReferences {
            mapping,
            application,
            path,
            target,
        },
        edit: graph.edit_terms(),
        context,
    };
    let value = ty
        .project_structure(
            application
                .parameters()
                .completed_contract()
                .template_scope(),
            target.shape(),
            1,
            &mut projection,
        )
        .map_err(|error| match error {
            TypeProjectionError::Control(error) => error,
            TypeProjectionError::Instantiation(error) => {
                super::super::TypeConstraintInvariant::Instantiation(error).into()
            }
        })?;
    projection.edit.commit()?;
    Ok(value)
}

struct Opening<'a, 'graph, 'control, A: TypeConstraintAccounting, D: ConstraintDomain, M> {
    references: ScopedTemplateReferences<'a, D, M>,
    edit: PreparedEffectTermEdit<'graph>,
    context: &'a mut TypeConstraintContext<'control, A, D>,
}

struct ScopedTemplateReferences<'a, D: ConstraintDomain, M> {
    mapping: &'a M,
    application: &'a ConstraintApplicationScope<D>,
    path: &'a ConstraintPath<D>,
    target: &'a ConstraintLexicalScope,
}
impl<D: ConstraintDomain, M: ConstraintReferenceMap> ScopedTemplateReferences<'_, D, M> {
    fn effect<A: TypeConstraintAccounting>(
        &self,
        reference: &GenericEffectReference,
        source: &GenericScope,
        target: &GenericScope,
        edit: &mut PreparedEffectTermEdit<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Option<PreparedEffectRow>, PreparedEffectFailure<TypeConstraintError>> {
        let opened = self
            .mapping
            .effect_reference(reference, source, self.application, self.path, context)
            .map_err(PreparedEffectFailure::Control)?;
        if matches!(opened, GenericEffectReference::Inference(_)) {
            return self
                .path
                .applications
                .effect_application(&opened)
                .ok_or(crate::effect_row::prepared::PreparedEffectInvariant::ParameterContract)?
                .transport_into(self.target, target, edit, context)
                .map(Some)
                .map_err(PreparedEffectFailure::Control);
        }
        if let GenericEffectReference::Bound(bound) = &opened {
            target
                .bound_effect(bound.depth(), bound.slot())
                .map_err(TypeConstraintError::from)
                .map_err(PreparedEffectFailure::Control)?;
        }
        Ok((&opened != reference).then(|| {
            PreparedEffectRow::Canonical(EffectRow::open(crate::effects::EffectSet::new(), opened))
        }))
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain, M> TypeProjectionControl
    for Opening<'_, '_, '_, A, D, M>
{
    type Error = TypeConstraintError;
    fn check(&mut self) -> Result<(), Self::Error> {
        self.context.check_cancelled()
    }
    fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Self::Error> {
        self.context.enter_node()
    }
    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.context.enter_node()
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain, M: ConstraintReferenceMap>
    ScopedTypeProjection<PreparedEffects, PreparedEffects> for Opening<'_, '_, '_, A, D, M>
{
    fn project_type_reference(
        &mut self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<TypeKind<PreparedEffects>, TypeProjectionError<Self::Error>> {
        let opened = self
            .references
            .mapping
            .type_reference(
                reference,
                source,
                self.references.application,
                self.references.path,
                self.context,
            )
            .map_err(TypeProjectionError::Control)?;
        if let GenericTypeReference::Bound(bound) = &opened {
            target.bound_type(bound.depth(), bound.slot())?;
        }
        Ok(TypeKind::GenericParam(opened))
    }

    fn project_length(
        &mut self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<ArrayLength, TypeProjectionError<Self::Error>> {
        self.context
            .enter_node()
            .map_err(TypeProjectionError::Control)?;
        match length {
            ArrayLength::Generic(reference) => {
                let opened = self
                    .references
                    .mapping
                    .const_reference(
                        reference,
                        source,
                        self.references.application,
                        self.references.path,
                        self.context,
                    )
                    .map_err(TypeProjectionError::Control)?;
                if let GenericConstReference::Bound(bound) = &opened {
                    target.bound_const(bound.depth(), bound.slot())?;
                }
                Ok(ArrayLength::Generic(opened))
            }
            ArrayLength::Const(_) => Ok(length.clone()),
            ArrayLength::Inferred | ArrayLength::Error(_) => Err(TypeProjectionError::Control(
                super::super::TypeConstraintRejection::UnresolvedType.into(),
            )),
        }
    }

    fn project_row(
        &mut self,
        row: &PreparedEffectRow,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Self::Error>> {
        let references = &self.references;
        self.edit
            .map_row_parameters_with_edit(row, self.context, &mut |reference, edit, context| {
                references.effect(reference, source, target, edit, context)
            })
            .map_err(TypeConstraintError::from)
            .map_err(TypeProjectionError::Control)
    }

    fn project_predicate(
        &mut self,
        predicate: &PreparedEffectPredicate,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        let references = &self.references;
        self.edit
            .map_predicate_parameters_with_edit(
                predicate,
                self.context,
                &mut |reference, edit, context| {
                    references.effect(reference, source, target, edit, context)
                },
            )
            .map_err(TypeConstraintError::from)
            .map_err(TypeProjectionError::Control)
    }
}
