//! Shared controls and authored rows for the prepared-effect owner's tests.

use super::*;
use crate::{effects::EffectSet, types::GenericScope};

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
#[error("effect decision budget exhausted")]
pub(super) struct Exhausted;

pub(super) struct Budget(pub(super) usize);
impl DecisionControl for Budget {
    type Error = Exhausted;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Exhausted> {
        self.0 = self.0.checked_sub(1).ok_or(Exhausted)?;
        Ok(())
    }
}

pub(super) fn budget() -> Budget {
    Budget(10_000_000)
}
pub(super) fn literal(labels: &[&str]) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::closed(
        EffectSet::from_labels(labels.iter().copied()).unwrap(),
    ))
}
pub(super) fn parameter(scope: &GenericScope, index: u32) -> GenericEffectReference {
    scope.bound_effect(0, index).unwrap()
}
pub(super) fn open(parameter: &GenericEffectReference) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), parameter.clone()))
}
