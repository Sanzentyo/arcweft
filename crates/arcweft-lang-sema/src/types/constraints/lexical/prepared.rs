//! Transport live row/predicate applications using their actual graph edit.

use super::*;
use crate::{
    effect_row::{
        EffectRow,
        prepared::{
            PreparedEffectFailure, PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow,
            PreparedEffectTermEdit,
        },
    },
    effects::EffectSet,
    types::{
        TypeProjectionControl, TypeProjectionNodeKind, projection_control::EffectProjectionControl,
        structural_projection::ScopedTypeProjection,
    },
};

#[cfg(test)]
mod tests;

struct Transport<'s, 'g, 'a, 'c, A: TypeConstraintAccounting, D: ConstraintDomain> {
    original: &'s ConstraintLexicalScope,
    target: &'s ConstraintLexicalScope,
    edit: &'a mut PreparedEffectTermEdit<'g>,
    context: &'a mut TypeConstraintContext<'c, A, D>,
}

fn effect_failure(
    error: PreparedEffectFailure<TypeProjectionError<TypeConstraintError>>,
) -> TypeProjectionError<TypeConstraintError> {
    match error.split_control() {
        Ok(error) => error,
        Err(error) => TypeProjectionError::Control(
            crate::types::constraints::TypeConstraintInvariant::PreparedEffect(error).into(),
        ),
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain> TypeProjectionControl
    for Transport<'_, '_, '_, '_, A, D>
{
    type Error = TypeConstraintError;
    fn check(&mut self) -> Result<(), Self::Error> {
        TypeProjectionControl::check(self.context)
    }
    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        TypeProjectionControl::visit_node(self.context, kind, depth)
    }
    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        TypeProjectionControl::visit_binding(self.context)
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain>
    ScopedTypeProjection<PreparedEffects, PreparedEffects> for Transport<'_, '_, '_, '_, A, D>
{
    fn project_type_reference(
        &mut self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<TypeKind<PreparedEffects>, TypeProjectionError<Self::Error>> {
        self.target
            .type_reference(self.original, source, target, reference, self.context)
            .map(TypeKind::GenericParam)
    }
    fn project_length(
        &mut self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<ArrayLength, TypeProjectionError<Self::Error>> {
        match length {
            ArrayLength::Generic(reference) => self
                .target
                .const_reference(self.original, source, target, reference, self.context)
                .map(ArrayLength::Generic),
            ArrayLength::Const(_) => Ok(length.clone()),
            ArrayLength::Error(_) | ArrayLength::Inferred => {
                Err(TypeInstantiationError::UnresolvedType.into())
            }
        }
    }
    fn project_row(
        &mut self,
        row: &PreparedEffectRow,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Self::Error>> {
        self.edit
            .map_row_parameters(
                row,
                &mut EffectProjectionControl {
                    control: self.context,
                    depth,
                },
                &mut |reference, control| {
                    let reference = self
                        .target
                        .effect_reference(self.original, source, target, reference, control.control)
                        .map_err(PreparedEffectFailure::Control)?;
                    Ok(Some(PreparedEffectRow::Canonical(EffectRow::open(
                        EffectSet::new(),
                        reference,
                    ))))
                },
            )
            .map_err(effect_failure)
    }
    fn project_predicate(
        &mut self,
        predicate: &PreparedEffectPredicate,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        self.edit
            .map_predicate_parameters(
                predicate,
                &mut EffectProjectionControl {
                    control: self.context,
                    depth,
                },
                &mut |reference, control| {
                    let reference = self
                        .target
                        .effect_reference(self.original, source, target, reference, control.control)
                        .map_err(PreparedEffectFailure::Control)?;
                    Ok(Some(PreparedEffectRow::Canonical(EffectRow::open(
                        EffectSet::new(),
                        reference,
                    ))))
                },
            )
            .map_err(effect_failure)
    }
}

impl ConstraintLexicalScope {
    /// One edit spans every nested field. Failure rolls back new terms while
    /// retaining spent work and the actual input/function identities.
    pub(in crate::types::constraints) fn transport_type<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        original: &Self,
        value: &TypeKind<PreparedEffects>,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<PreparedEffects>, TypeProjectionError<TypeConstraintError>> {
        let mut edit = graph.edit_terms();
        let result = value.project_structure(
            original.shape(),
            self.shape(),
            1,
            &mut Transport {
                original,
                target: self,
                edit: &mut edit,
                context,
            },
        );
        let value = result?;
        edit.commit()
            .map_err(|error| effect_failure(error.into()))?;
        Ok(value)
    }
}
