//! Private task candidate ownership shared by construction and strict decode.
//! There is one executable inventory; expected keys remain untrusted assertions.

mod preflight;

use super::request::RuntimeTaskRequestTemplate;
use crate::plan::{
    RuntimeControlEffectContractId, RuntimePlanInventory, RuntimeTaskPlanCoordinateOwner,
};
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLineTaskGroupId};
use crate::task::{NeedProducerFamily, TaskClass};

/// Source definitions only. None of these fields is a completed task key.
pub(super) struct RuntimeTaskPlan {
    pub(super) producer_function: RuntimeFunctionSiteId,
    pub(super) family: NeedProducerFamily,
    pub(super) class: TaskClass,
    pub(super) request_template: RuntimeTaskRequestTemplate,
    pub(super) control_effect: RuntimeControlEffectContractId,
    pub(super) binding: RuntimeTaskSemanticBinding,
}

/// Binding definitions resolve through their actual semantic owner during the
/// common pass. Line retains a group coordinate, never a supplied digest.
pub(super) enum RuntimeTaskSemanticBinding {
    Ordinary,
    View,
    AwaitManyBase,
    AwaitManyChild,
    Timeout {
        contract: crate::task::NeedTimeoutContractDigest,
    },
    Line {
        group: RuntimeLineTaskGroupId,
    },
}

impl RuntimeTaskSemanticBinding {
    pub(super) const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Ordinary => 0,
            Self::View => 1,
            Self::AwaitManyBase => 2,
            Self::AwaitManyChild => 3,
            Self::Timeout { .. } => 4,
            Self::Line { .. } => 5,
        }
    }
}

/// Codec-only integrity assertion, with no conversion into a typed digest.
pub(super) struct ExpectedTaskPlanKey([u8; 32]);
impl ExpectedTaskPlanKey {
    pub(super) const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub(super) const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Owns the sole aggregate storage until every semantic check has succeeded.
/// No public plan/table is constructed by this preparation shape.
pub(super) struct UnsealedRuntimePlanImage {
    pub(super) inventory: RuntimePlanInventory,
    pub(super) coordinate_owner: RuntimeTaskPlanCoordinateOwner,
    pub(super) task_plans: Box<[RuntimeTaskPlan]>,
    expected_task_plan_keys: Option<Box<[ExpectedTaskPlanKey]>>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum RuntimeTaskPlanImageError {
    #[error("task rows {actual} exceed {maximum}")]
    TaskRows { actual: usize, maximum: u32 },
    #[error(transparent)]
    Body(#[from] super::RuntimeBodySemanticError),
    #[error(transparent)]
    Control(#[from] crate::plan::RuntimeControlEffectContractError),
    #[error("task image count encoding failed: {0:?}")]
    Encoding(crate::task::semantic::TaskSemanticEncodingError),
    #[error("task image rows {rows} do not match its coordinate owner {coordinates}")]
    CoordinateCount { rows: usize, coordinates: u32 },
    #[error("task image expected key count {keys} differs from {rows} task rows")]
    ExpectedKeyCount { rows: usize, keys: usize },
}

impl From<crate::task::semantic::TaskSemanticEncodingError> for RuntimeTaskPlanImageError {
    fn from(error: crate::task::semantic::TaskSemanticEncodingError) -> Self {
        Self::Encoding(error)
    }
}

impl UnsealedRuntimePlanImage {
    pub(super) fn new(
        inventory: RuntimePlanInventory,
        coordinate_owner: RuntimeTaskPlanCoordinateOwner,
        task_plans: Box<[RuntimeTaskPlan]>,
        expected_task_plan_keys: Option<Box<[ExpectedTaskPlanKey]>>,
    ) -> Result<Self, RuntimeTaskPlanImageError> {
        let rows = task_plans.len();
        let coordinates = coordinate_owner.len();
        if rows != coordinates as usize {
            return Err(RuntimeTaskPlanImageError::CoordinateCount { rows, coordinates });
        }
        if let Some(keys) = &expected_task_plan_keys
            && keys.len() != rows
        {
            return Err(RuntimeTaskPlanImageError::ExpectedKeyCount {
                rows,
                keys: keys.len(),
            });
        }
        Ok(Self {
            inventory,
            coordinate_owner,
            task_plans,
            expected_task_plan_keys,
        })
    }

    pub(super) fn expected_key(&self, ordinal: usize) -> Option<&ExpectedTaskPlanKey> {
        self.expected_task_plan_keys
            .as_ref()
            .and_then(|keys| keys.get(ordinal))
    }
}

#[cfg(test)]
mod tests;
