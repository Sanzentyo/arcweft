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
        PreparedTypeEffectProjectionFailure, TypeProjectionControl, TypeProjectionNodeKind,
        projection_control::EffectProjectionControl, structural_projection::ScopedTypeProjection,
    },
};

#[cfg(test)]
mod tests;

struct Transport<'s, 'g, 'a, C> {
    original: &'s ConstraintLexicalScope,
    target: &'s ConstraintLexicalScope,
    edit: &'a mut PreparedEffectTermEdit<'g>,
    context: &'a mut C,
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

impl<C: TypeProjectionControl> Transport<'_, '_, '_, C> {
    fn project_effect_reference(
        original: &ConstraintLexicalScope,
        destination: &ConstraintLexicalScope,
        source: &GenericScope,
        target: &GenericScope,
        reference: &GenericEffectReference,
        control: &mut EffectProjectionControl<'_, C>,
    ) -> Result<Option<PreparedEffectRow>, TypeProjectionError<C::Error>> {
        let projected =
            destination.effect_reference(original, source, target, reference, control.control)?;
        Ok((projected != *reference)
            .then(|| PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), projected))))
    }
}

impl<C: TypeProjectionControl> TypeProjectionControl for Transport<'_, '_, '_, C> {
    type Error = PreparedTypeEffectProjectionFailure<C::Error>;
    fn check(&mut self) -> Result<(), Self::Error> {
        self.context
            .check()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.context
            .visit_node(kind, depth)
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.context
            .visit_binding()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
}

impl<C: TypeProjectionControl> ScopedTypeProjection<PreparedEffects, PreparedEffects>
    for Transport<'_, '_, '_, C>
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
            .map_err(PreparedTypeEffectProjectionFailure::from_projection)
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
                .map(ArrayLength::Generic)
                .map_err(PreparedTypeEffectProjectionFailure::from_projection),
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
        if let PreparedEffectRow::Canonical(row) = row {
            return self
                .target
                .transport_row_at(self.original, row, (source, target), depth, self.context)
                .map(PreparedEffectRow::Canonical)
                .map_err(PreparedTypeEffectProjectionFailure::from_projection);
        }
        let mut control = EffectProjectionControl {
            control: self.context,
            depth,
        };
        let project = |reference: &GenericEffectReference, control: &mut _| {
            Self::project_effect_reference(
                self.original,
                self.target,
                source,
                target,
                reference,
                control,
            )
        };
        if self.original == self.target {
            self.edit
                .read()
                .visit_row_parameters(row, &mut control, &mut |reference, control| {
                    project(reference, control).map(|_| ())
                })
                .map_err(PreparedTypeEffectProjectionFailure::from_effect)?;
            return Ok(row.clone());
        }
        self.edit
            .map_row_parameters(row, &mut control, &mut |reference, control| {
                project(reference, control).map_err(PreparedEffectFailure::Control)
            })
            .map_err(PreparedTypeEffectProjectionFailure::from_effect)
    }
    fn project_predicate(
        &mut self,
        predicate: &PreparedEffectPredicate,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        let mut control = EffectProjectionControl {
            control: self.context,
            depth,
        };
        let project = |reference: &GenericEffectReference, control: &mut _| {
            Self::project_effect_reference(
                self.original,
                self.target,
                source,
                target,
                reference,
                control,
            )
        };
        if self.original == self.target && matches!(predicate, PreparedEffectPredicate::Term(_)) {
            self.edit
                .read()
                .visit_predicate_parameters(predicate, &mut control, &mut |reference, control| {
                    project(reference, control).map(|_| ())
                })
                .map_err(PreparedTypeEffectProjectionFailure::from_effect)?;
            return Ok(predicate.clone());
        }
        self.edit
            .map_predicate_parameters(predicate, &mut control, &mut |reference, control| {
                project(reference, control).map_err(PreparedEffectFailure::Control)
            })
            .map_err(PreparedTypeEffectProjectionFailure::from_effect)
    }
}

impl ConstraintLexicalScope {
    pub(in crate::types) fn transport_type_in<C: TypeProjectionControl>(
        &self,
        original: &Self,
        value: &TypeKind<PreparedEffects>,
        edit: &mut PreparedEffectTermEdit<'_>,
        context: &mut C,
    ) -> Result<
        TypeKind<PreparedEffects>,
        TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>,
    > {
        value
            .project_structure(
                original.shape(),
                self.shape(),
                1,
                &mut Transport {
                    original,
                    target: self,
                    edit,
                    context,
                },
            )
            .inspect_err(|_| edit.invalidate())
    }

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
        let value = self
            .transport_type_in(original, value, &mut edit, context)
            .map_err(|error| match error {
                TypeProjectionError::Instantiation(error) => {
                    TypeProjectionError::Instantiation(error)
                }
                TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Control(
                    error,
                )) => TypeProjectionError::Control(error),
                TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Effects(
                    error,
                )) => effect_failure(error),
            })?;
        edit.commit()
            .map_err(|error| effect_failure(error.into()))?;
        Ok(value)
    }
}
