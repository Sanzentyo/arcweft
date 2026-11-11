//! Controlled entry into the effect evidence retained during body preparation.

use crate::effect_row::prepared::{PreparedEffectPredicate, PreparedEffectRow};

use super::*;
use crate::types::projection_control::{visit_effect_predicate, visit_effect_row};

/// Prepared identity is operational. This domain cannot issue a checked type
/// digest, specialization, runtime projection or completed constraint solution.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PreparedEffects;

impl sealed::Sealed for PreparedEffects {}

impl TypeEffectDomain for PreparedEffects {
    type Row = PreparedEffectRow;
    type Predicate = PreparedEffectPredicate;
}

struct EffectPreparation<'a, C> {
    control: &'a mut C,
    depth: u64,
}

impl<C: TypeProjectionControl> TypeProjectionControl for EffectPreparation<'_, C> {
    type Error = C::Error;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.control.check()
    }

    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.control.visit_node(kind, depth)?;
        self.depth = depth;
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.control.visit_binding()
    }
}

impl<C: TypeProjectionControl> TypeEffectProjection<CanonicalEffects, PreparedEffects>
    for EffectPreparation<'_, C>
{
    fn project_row(
        &mut self,
        row: &EffectRow,
        _: &GenericScope,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Self::Error>> {
        visit_effect_row(self.control, row, self.depth)?;
        Ok(PreparedEffectRow::Canonical(row.clone()))
    }

    fn project_predicate(
        &mut self,
        predicate: &EffectPredicate,
        _: &GenericScope,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        visit_effect_predicate(self.control, predicate, self.depth)?;
        Ok(PreparedEffectPredicate::Canonical(predicate.clone()))
    }
}

impl TypeKind {
    /// Retains canonical annotations, including omitted rows, in the one shared
    /// type grammar. Subsequent body analysis can carry issuer-owned terms in
    /// any nested arrow without issuing a completed semantic identity.
    pub(crate) fn prepare_effects<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        control: &mut C,
    ) -> Result<TypeKind<PreparedEffects>, TypeProjectionError<C::Error>> {
        self.project_effect_domain(scope, &mut EffectPreparation { control, depth: 1 })
    }
}
