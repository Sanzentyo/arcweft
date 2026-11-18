//! Final closure reads actual bodies and inputs while preserving original C.

use super::*;
use crate::effect_row::prepared::read::SnapshotSource;

#[cfg(test)]
mod tests;

impl ScopedEffectCompletion {
    /// A final mathematical closure requires every actual Future row and the
    /// actual original Named bodies. It neither prepares a body nor chooses a
    /// convenient replacement for an unavailable input. No live facts change.
    pub(crate) fn close_original<C: DecisionControl>(
        &self,
        receiving: PreparedEffectReadView<'_>,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        bindings: &[(&PreparedEffectUnknownRef, &EffectRow)],
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        let owner = PreparedEffectGraphOwner(Arc::clone(&self.graph.issuer));
        receiving.validate_owner(&owner)?;
        if !self.fixed.is_empty() {
            return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
        }
        receiving
            .snapshot(
                query_rows,
                query_predicates,
                &[],
                SnapshotSource::Final(&self.graph),
                control,
            )?
            .specialize_principal(bindings, control)
    }
}
