//! Normalized execution contract for a structural Flow declaration.

use super::{RuntimeNormalizedType, RuntimeTypeShape};
use arcweft_core::plan::{FlowRuntimeId, RuntimeEffectSet};
use arcweft_lang_sema::final_analysis::{CheckedExecutionInputAbi, CheckedFlowExecutionDefinition};
use std::sync::Arc;

/// The Flow identity and its closed, exposed execution effects are admitted
/// together under the owning HIR item and semantic generation.
#[derive(Clone, Debug)]
pub struct RuntimeFlowFact {
    identity: FlowRuntimeId,
    effects: RuntimeEffectSet,
    function_type: RuntimeNormalizedType,
    definition: Arc<CheckedFlowExecutionDefinition>,
}

impl RuntimeFlowFact {
    pub fn try_new(
        identity: FlowRuntimeId,
        definition: Arc<CheckedFlowExecutionDefinition>,
        function_type: RuntimeNormalizedType,
    ) -> Result<Self, RuntimeFlowFactError> {
        let RuntimeTypeShape::Function { parameters, .. } = function_type.shape() else {
            return Err(RuntimeFlowFactError::NotFunction);
        };
        let expected = definition.body().parameters().len()
            + definition
                .body()
                .inputs()
                .iter()
                .filter(|input| {
                    input.role()
                        == &arcweft_lang_sema::final_analysis::CheckedExecutionInputRole::Free
                })
                .count();
        if parameters.len() != expected {
            return Err(RuntimeFlowFactError::InputArity {
                expected,
                actual: parameters.len(),
            });
        }
        let effects = RuntimeEffectSet::try_from_effects(definition.effects().iter().cloned())?;
        Ok(Self {
            identity,
            effects,
            function_type,
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

    /// Normalized complete invocation type, projected from the accepted Flow ABI.
    #[must_use]
    pub const fn function_type(&self) -> &RuntimeNormalizedType {
        &self.function_type
    }

    #[must_use]
    pub fn definition(&self) -> &Arc<CheckedExecutionInputAbi> {
        self.definition.body()
    }

    pub fn owner(&self) -> arcweft_lang_hir::identity::ItemId {
        self.definition.owner()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeFlowFactError {
    #[error("accepted Flow invocation projection is not a function type")]
    NotFunction,
    #[error("accepted Flow invocation has {actual} normalized inputs, expected {expected}")]
    InputArity { expected: usize, actual: usize },
    #[error(transparent)]
    Effects(#[from] arcweft_core::plan::RuntimeEffectSetError),
}
