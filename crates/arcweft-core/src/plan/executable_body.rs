//! Closed effects and structured executable bodies owned by a runtime plan.

use arcweft_id::EffectId;
use thiserror::Error;

use super::FlowOp;

/// The one runtime projection of a checked closed effect row.
///
/// The executable body or its signature reservation owns this row. It is a
/// typed slice of the foundational `EffectId` values rather than a debug
/// label, source spelling, or encoded integer. The checked constructor sorts
/// the members and rejects duplicates.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeEffectSet(Box<[EffectId]>);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeEffectSetError {
    #[error("runtime effect set contains duplicate effect `{effect}`")]
    Duplicate { effect: EffectId },
}

impl RuntimeEffectSet {
    #[must_use]
    pub fn empty() -> Self {
        Self(Vec::new().into_boxed_slice())
    }

    /// Constructs the canonical sorted/unique runtime projection of one
    /// checked effect row.
    pub fn try_from_effects(
        effects: impl IntoIterator<Item = EffectId>,
    ) -> Result<Self, RuntimeEffectSetError> {
        let mut effects = effects.into_iter().collect::<Vec<_>>();
        effects.sort();
        if let Some(effect) = effects
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then(|| pair[0].clone()))
        {
            return Err(RuntimeEffectSetError::Duplicate { effect });
        }
        Ok(Self(effects.into_boxed_slice()))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[EffectId] {
        &self.0
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &EffectId> + DoubleEndedIterator {
        self.0.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// Closed execution contract shared by Flow roots and structured function
/// sites. Effects remain paired with the operations they authorize.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeExecutableBody {
    effects: RuntimeEffectSet,
    ops: Box<[FlowOp]>,
}

impl RuntimeExecutableBody {
    pub(crate) fn new(effects: RuntimeEffectSet, ops: Box<[FlowOp]>) -> Self {
        Self { effects, ops }
    }

    #[must_use]
    pub const fn effects(&self) -> &RuntimeEffectSet {
        &self.effects
    }

    #[must_use]
    pub fn is_effect_free(&self) -> bool {
        self.effects.is_empty()
    }

    #[must_use]
    pub fn ops(&self) -> &[FlowOp] {
        &self.ops
    }
}
