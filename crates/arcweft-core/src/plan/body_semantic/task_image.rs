//! Private task candidate ownership shared by construction and strict decode.
//! There is one executable inventory; expected keys remain untrusted assertions.

pub(in crate::plan) mod codec;
mod preflight;
#[allow(
    dead_code,
    reason = "Private complete E14 preparation remains unexposed until the atomic task/View public switch"
)]
mod preparation;

use crate::plan::{RuntimePlanInventory, RuntimeTaskPlanCoordinateOwner};
use crate::runtime_id::RuntimeLineTaskGroupId;

pub(crate) type RuntimeTaskPlan = crate::plan::RuntimeTaskPlan<RuntimeLineTaskGroupId>;
type RuntimeTaskSemanticBinding = crate::plan::RuntimeTaskSemanticBinding<RuntimeLineTaskGroupId>;

pub(in crate::plan) use crate::task::ExpectedTaskPlanKey;

/// Owns the sole aggregate storage until every semantic check has succeeded.
/// No public plan/table is constructed by this preparation shape.
pub(in crate::plan) struct UnsealedRuntimePlanImage {
    pub(super) inventory: RuntimePlanInventory,
    pub(super) coordinate_owner: RuntimeTaskPlanCoordinateOwner,
    pub(super) task_plans: Box<[RuntimeTaskPlan]>,
    expected_task_plan_keys: Option<Box<[ExpectedTaskPlanKey]>>,
    expected_executable_key: Option<[u8; 32]>,
    preparation_meter: Option<crate::task::semantic::TaskSemanticMeter>,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum RuntimeTaskPlanSealError {
    #[error("the original builder has consumed its task construction slots")]
    ConstructionClosed,
    #[error("the original task construction namespace is already sealed")]
    AlreadySealed,
    #[error("task definition coordinate belongs to another builder")]
    ForeignCoordinate,
    #[error("task row {ordinal} is already defined")]
    DuplicateDefinition { ordinal: u32 },
    #[error("task row {ordinal} has no source definition")]
    IncompleteDefinition { ordinal: usize },
    #[error(transparent)]
    Construction(Box<crate::plan::RuntimePlanBuildError>),
    #[error(transparent)]
    Verification(Box<crate::plan::entry_inventory::RuntimePlanError>),
    #[error("task completion has {completed_rows} rows for {source_rows} source definitions")]
    CompletionCount {
        source_rows: usize,
        completed_rows: usize,
    },
    #[error("View task row {ordinal} requires the original View task-plan authority")]
    MissingViewAuthority { ordinal: usize },
    #[error("View task row {ordinal} rejected: {source}")]
    ViewAuthority {
        ordinal: usize,
        source: crate::plan::ViewTaskPlanValidationError,
    },
    #[error("task row {ordinal} differs from its expected decoded key")]
    ExpectedKeyMismatch { ordinal: usize },
    #[error("the executable differs from its expected decoded key")]
    ExpectedExecutableKeyMismatch,
    #[error("task row {ordinal} duplicates completed task key from row {first}")]
    DuplicateKey { first: usize, ordinal: usize },
    #[error("executable table {table} row {ordinal} has {actual} children; maximum {maximum}")]
    Children {
        table: u8,
        ordinal: usize,
        actual: usize,
        maximum: u32,
    },
    #[error("View marker rows {actual} exceed {maximum}")]
    ViewRows { actual: usize, maximum: u32 },
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
    #[error("task row {ordinal} definition rejected: {source}")]
    TaskDefinition {
        ordinal: usize,
        source: crate::plan::RuntimeTaskPlanValidationError,
    },
}

impl From<crate::task::semantic::TaskSemanticEncodingError> for RuntimeTaskPlanSealError {
    fn from(error: crate::task::semantic::TaskSemanticEncodingError) -> Self {
        Self::Encoding(error)
    }
}

impl From<crate::plan::RuntimePlanBuildError> for RuntimeTaskPlanSealError {
    fn from(error: crate::plan::RuntimePlanBuildError) -> Self {
        Self::Construction(Box::new(error))
    }
}

impl From<crate::plan::entry_inventory::RuntimePlanError> for RuntimeTaskPlanSealError {
    fn from(error: crate::plan::entry_inventory::RuntimePlanError) -> Self {
        Self::Verification(Box::new(error))
    }
}

impl UnsealedRuntimePlanImage {
    /// Stores only assertion data. The actual semantic encoder compares it
    /// before any task completion or coordinate publication.
    pub(in crate::plan) fn with_expected_executable_key(mut self, expected: [u8; 32]) -> Self {
        self.expected_executable_key = Some(expected);
        self
    }

    pub(in crate::plan) fn new(
        inventory: RuntimePlanInventory,
        coordinate_owner: RuntimeTaskPlanCoordinateOwner,
        task_plans: Box<[RuntimeTaskPlan]>,
        expected_task_plan_keys: Option<Box<[ExpectedTaskPlanKey]>>,
        preparation_meter: crate::task::semantic::TaskSemanticMeter,
    ) -> Result<Self, RuntimeTaskPlanSealError> {
        let rows = task_plans.len();
        let coordinates = coordinate_owner.len();
        if rows != coordinates as usize {
            return Err(RuntimeTaskPlanSealError::CoordinateCount { rows, coordinates });
        }
        if let Some(keys) = &expected_task_plan_keys
            && keys.len() != rows
        {
            return Err(RuntimeTaskPlanSealError::ExpectedKeyCount {
                rows,
                keys: keys.len(),
            });
        }
        Ok(Self {
            inventory,
            coordinate_owner,
            task_plans,
            expected_task_plan_keys,
            expected_executable_key: None,
            preparation_meter: Some(preparation_meter),
        })
    }

    #[cfg(test)]
    pub(in crate::plan) fn new_for_test(
        inventory: RuntimePlanInventory,
        coordinate_owner: RuntimeTaskPlanCoordinateOwner,
        task_plans: Box<[RuntimeTaskPlan]>,
        expected_task_plan_keys: Option<Box<[ExpectedTaskPlanKey]>>,
    ) -> Result<Self, RuntimeTaskPlanSealError> {
        let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
        Self::new(
            inventory,
            coordinate_owner,
            task_plans,
            expected_task_plan_keys,
            crate::task::semantic::TaskSemanticMeter::new(
                limits.max_semantic_work,
                limits.max_transcript_bytes,
            ),
        )
    }

    pub(super) fn expected_key(&self, ordinal: usize) -> Option<&ExpectedTaskPlanKey> {
        self.expected_task_plan_keys
            .as_ref()
            .and_then(|keys| keys.get(ordinal))
    }
}

#[cfg(test)]
mod tests;
