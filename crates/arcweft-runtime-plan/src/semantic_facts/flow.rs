//! Normalized execution contract for a structural Flow declaration.

use arcweft_core::plan::{FlowRuntimeId, RuntimeEffectSet};
use arcweft_lang_sema::final_analysis::{CheckedExecutionInputAbi, CheckedFlowExecutionDefinition};
use std::sync::Arc;

/// The Flow identity and its closed, exposed execution effects are admitted
/// together under the owning HIR item and semantic generation.
#[derive(Clone, Debug)]
pub struct RuntimeFlowFact {
    identity: FlowRuntimeId,
    effects: RuntimeEffectSet,
    definition: Arc<CheckedFlowExecutionDefinition>,
}

impl RuntimeFlowFact {
    pub fn try_new(
        identity: FlowRuntimeId,
        definition: Arc<CheckedFlowExecutionDefinition>,
    ) -> Result<Self, arcweft_core::plan::RuntimeEffectSetError> {
        let effects = RuntimeEffectSet::try_from_effects(definition.effects().iter().cloned())?;
        Ok(Self {
            identity,
            effects,
            definition,
        })
    }

    #[must_use]
    pub const fn identity(&self) -> &FlowRuntimeId {
        &self.identity
    }

    #[must_use]
    pub const fn effects(&self) -> &RuntimeEffectSet {
        &self.effects
    }

    #[must_use]
    pub fn definition(&self) -> &Arc<CheckedExecutionInputAbi> {
        self.definition.body()
    }

    pub fn owner(&self) -> arcweft_lang_hir::identity::ItemId {
        self.definition.owner()
    }
}
