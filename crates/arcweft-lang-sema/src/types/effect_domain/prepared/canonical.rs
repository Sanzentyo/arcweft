//! Canonical algebra projection without a selected-call or checked-type seal.

use super::*;
use crate::effect_row::prepared::PrincipalEffectClosure;

#[cfg(test)]
mod tests;

struct Canonicalization<'a, 'g, C> {
    principal: &'a PrincipalEffectClosure<'g>,
    control: &'a mut C,
    depth: u64,
}

impl<C: TypeProjectionControl> TypeProjectionControl for Canonicalization<'_, '_, C> {
    type Error = PreparedTypeEffectProjectionFailure<C::Error>;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.control
            .check()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }

    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.control
            .visit_node(kind, depth)
            .map_err(PreparedTypeEffectProjectionFailure::Control)?;
        self.depth = depth;
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.control
            .visit_binding()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
}

impl<C: TypeProjectionControl> TypeEffectProjection<PreparedEffects, CanonicalEffects>
    for Canonicalization<'_, '_, C>
{
    fn project_row(
        &mut self,
        row: &PreparedEffectRow,
        _: &GenericScope,
    ) -> Result<EffectRow, TypeProjectionError<Self::Error>> {
        match row {
            PreparedEffectRow::Canonical(row) => {
                visit_effect_row(self.control, row, self.depth)
                    .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
                Ok(row.clone())
            }
            PreparedEffectRow::Term(_) => self
                .principal
                .resolve_row(
                    row,
                    &mut EffectProjectionControl {
                        control: self.control,
                        depth: self.depth,
                    },
                )
                .map_err(PreparedTypeEffectProjectionFailure::from_effect),
        }
    }

    fn project_predicate(
        &mut self,
        predicate: &PreparedEffectPredicate,
        _: &GenericScope,
    ) -> Result<EffectPredicate, TypeProjectionError<Self::Error>> {
        match predicate {
            PreparedEffectPredicate::Canonical(predicate) => {
                visit_effect_predicate(self.control, predicate, self.depth)
                    .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
                Ok(predicate.clone())
            }
            PreparedEffectPredicate::Term(_) => self
                .principal
                .resolve_predicate(
                    predicate,
                    &mut EffectProjectionControl {
                        control: self.control,
                        depth: self.depth,
                    },
                )
                .map_err(PreparedTypeEffectProjectionFailure::from_effect),
        }
    }
}

impl TypeKind<PreparedEffects> {
    /// Project every live term only when it is unconditionally invariant and
    /// representable. Canonical authored annotations, including omitted rows,
    /// retain their original meaning. This algebra transition does not certify
    /// annotation completion, producer selection, scope admission or a checked
    /// semantic identity.
    pub(crate) fn project_canonical_effects<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        principal: &PrincipalEffectClosure<'_>,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>> {
        self.project_effect_domain(
            scope,
            &mut Canonicalization {
                principal,
                control,
                depth: 1,
            },
        )
    }
}
