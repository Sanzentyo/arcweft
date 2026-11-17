//! Lift one retained value beneath additional binders without closing its rows.

use super::*;
use crate::effect_row::prepared::{PreparedEffectGraph, PreparedEffectTermEdit};
use crate::types::structural_projection::ScopedTypeProjection;
use crate::types::{ArrayLength, GenericTypeReference, TypeInstantiationError};

#[cfg(test)]
mod tests;

struct Lifting<'a, 'g, C> {
    original: &'a GenericScope,
    edit: &'a mut PreparedEffectTermEdit<'g>,
    control: &'a mut C,
}

impl<C: TypeProjectionControl> TypeProjectionControl for Lifting<'_, '_, C> {
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
    for Lifting<'_, '_, C>
{
    fn project_type_reference(
        &mut self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<TypeKind<PreparedEffects>, TypeProjectionError<Self::Error>> {
        Ok(TypeKind::GenericParam(reference.lift_into(
            self.original,
            source,
            target,
        )?))
    }

    fn project_length(
        &mut self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<ArrayLength, TypeProjectionError<Self::Error>> {
        self.visit_node(TypeProjectionNodeKind::Const, depth)
            .map_err(TypeProjectionError::Control)?;
        match length {
            ArrayLength::Generic(reference) => Ok(ArrayLength::Generic(reference.lift_into(
                self.original,
                source,
                target,
            )?)),
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
                    control: self.control,
                    depth,
                },
                &mut |reference, _| {
                    let reference =
                        reference
                            .lift_into(self.original, source, target)
                            .map_err(|error| {
                                PreparedEffectFailure::Control(TypeProjectionError::from(error))
                            })?;
                    Ok(Some(PreparedEffectRow::Canonical(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        reference,
                    ))))
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
        self.edit
            .map_predicate_parameters(
                predicate,
                &mut EffectProjectionControl {
                    control: self.control,
                    depth,
                },
                &mut |reference, _| {
                    let reference =
                        reference
                            .lift_into(self.original, source, target)
                            .map_err(|error| {
                                PreparedEffectFailure::Control(TypeProjectionError::from(error))
                            })?;
                    Ok(Some(PreparedEffectRow::Canonical(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        reference,
                    ))))
                },
            )
            .map_err(PreparedTypeEffectProjectionFailure::from_effect)
    }
}

impl TypeKind<PreparedEffects> {
    /// Transport a value from its actual incoming scope to an extension. The
    /// shared type fold preserves inner binders; the original graph remaps live
    /// row/predicate references in one affine edit. This does not admit a Γ,
    /// prepare a body, solve the relation, or issue a canonical identity.
    pub(crate) fn lift_under_with_control<C: TypeProjectionControl>(
        &self,
        original: &GenericScope,
        target: &GenericScope,
        graph: &mut PreparedEffectGraph,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>> {
        control.check().map_err(|error| {
            TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Control(error))
        })?;
        target.extension_depth(original)?;
        let mut edit = graph.edit_terms();
        let result = self.lift_under_in(original, target, 1, &mut edit, control)?;
        edit.commit()
            .map_err(|error| PreparedTypeEffectProjectionFailure::from_effect(error.into()))?;
        Ok(result)
    }

    /// Enclosing instantiations retain the affine edit until every replacement
    /// succeeds. A replacement cannot commit its own partial graph change.
    pub(in crate::types) fn lift_under_in<C: TypeProjectionControl>(
        &self,
        original: &GenericScope,
        target: &GenericScope,
        depth: u64,
        edit: &mut PreparedEffectTermEdit<'_>,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>> {
        let result = (|| {
            control.check().map_err(|error| {
                TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Control(error))
            })?;
            target.extension_depth(original)?;
            self.project_structure(
                original,
                target,
                depth,
                &mut Lifting {
                    original,
                    edit,
                    control,
                },
            )
        })();
        if result.is_err() {
            edit.invalidate();
        }
        result
    }
}
