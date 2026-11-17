//! Open the declaration's scalar ports through the shared prepared grammar.

use super::*;
use crate::{
    effect_row::{
        EffectRow,
        prepared::{
            PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow, PreparedEffectTermEdit,
        },
    },
    types::{
        PreparedEffects, TypeProjectionControl, TypeProjectionError, TypeProjectionNodeKind,
        constraints::lexical::ConstraintLexicalScope, structural_projection::ScopedTypeProjection,
    },
};
use std::collections::{BTreeMap, BTreeSet};

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
        mapping,
        application,
        path,
        target,
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
    mapping: &'a M,
    application: &'a ConstraintApplicationScope<D>,
    path: &'a ConstraintPath<D>,
    target: &'a ConstraintLexicalScope,
    edit: PreparedEffectTermEdit<'graph>,
    context: &'a mut TypeConstraintContext<'control, A, D>,
}

enum EffectInput<'a> {
    Row(&'a PreparedEffectRow),
    Predicate(&'a PreparedEffectPredicate),
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain, M: ConstraintReferenceMap>
    Opening<'_, '_, '_, A, D, M>
{
    fn arguments(
        &mut self,
        input: EffectInput<'_>,
        source: &GenericScope,
        target: &GenericScope,
    ) -> Result<BTreeMap<GenericEffectReference, PreparedEffectRow>, TypeConstraintError> {
        let mut references = BTreeSet::new();
        let mut collect =
            |reference: &GenericEffectReference, context: &mut TypeConstraintContext<'_, A, D>| {
                context.enter_node()?;
                references.insert(reference.clone());
                Ok(())
            };
        match input {
            EffectInput::Row(row) => {
                self.edit
                    .read()
                    .visit_row_parameters(row, self.context, &mut collect)?
            }
            EffectInput::Predicate(predicate) => self.edit.read().visit_predicate_parameters(
                predicate,
                self.context,
                &mut collect,
            )?,
        }
        let mut arguments = BTreeMap::new();
        for reference in references {
            self.context.enter_node()?;
            let opened = self.mapping.effect_reference(
                &reference,
                source,
                self.application,
                self.path,
                self.context,
            )?;
            let row = match &opened {
                GenericEffectReference::Inference(_) => self
                    .path
                    .applications
                    .effect_application(&opened)
                    .ok_or(crate::effect_row::prepared::PreparedEffectInvariant::ParameterContract)?
                    .transport_into(self.target, target, &mut self.edit, self.context)?,
                GenericEffectReference::Bound(bound) => {
                    target.bound_effect(bound.depth(), bound.slot())?;
                    if opened == reference {
                        continue;
                    }
                    PreparedEffectRow::Canonical(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        opened,
                    ))
                }
                GenericEffectReference::Free(_) => {
                    if opened == reference {
                        continue;
                    }
                    PreparedEffectRow::Canonical(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        opened,
                    ))
                }
            };
            arguments.insert(reference, row);
        }
        Ok(arguments)
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
            .mapping
            .type_reference(reference, source, self.application, self.path, self.context)
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
                    .mapping
                    .const_reference(reference, source, self.application, self.path, self.context)
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
        let arguments = self
            .arguments(EffectInput::Row(row), source, target)
            .map_err(TypeProjectionError::Control)?;
        self.edit
            .map_row_parameters(row, self.context, &mut |reference, context| {
                crate::effect_row::DecisionControl::charge(
                    context,
                    crate::effect_row::DecisionWork::Visit,
                )
                .map_err(crate::effect_row::prepared::PreparedEffectFailure::Control)?;
                Ok(arguments.get(reference).cloned())
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
        let arguments = self
            .arguments(EffectInput::Predicate(predicate), source, target)
            .map_err(TypeProjectionError::Control)?;
        self.edit
            .map_predicate_parameters(predicate, self.context, &mut |reference, context| {
                crate::effect_row::DecisionControl::charge(
                    context,
                    crate::effect_row::DecisionWork::Visit,
                )
                .map_err(crate::effect_row::prepared::PreparedEffectFailure::Control)?;
                Ok(arguments.get(reference).cloned())
            })
            .map_err(TypeConstraintError::from)
            .map_err(TypeProjectionError::Control)
    }
}
