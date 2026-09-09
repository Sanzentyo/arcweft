//! Normalized execution contract for a structural Flow declaration.

use arcweft_core::plan::{FlowRuntimeId, RuntimeEffectSet};

/// The Flow identity and its closed, exposed execution effects are admitted
/// together under the owning HIR item and semantic generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFlowFact {
    identity: FlowRuntimeId,
    effects: RuntimeEffectSet,
}

impl RuntimeFlowFact {
    #[must_use]
    pub const fn new(identity: FlowRuntimeId, effects: RuntimeEffectSet) -> Self {
        Self { identity, effects }
    }

    #[must_use]
    pub const fn identity(&self) -> &FlowRuntimeId {
        &self.identity
    }

    #[must_use]
    pub const fn effects(&self) -> &RuntimeEffectSet {
        &self.effects
    }
}
