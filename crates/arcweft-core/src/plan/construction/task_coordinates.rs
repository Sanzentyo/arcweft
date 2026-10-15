//! Construction-only task coordinates share the sole aggregate issuer.

use super::{RuntimePlanBuilder, seed::RuntimePlanConstructionIssuer};
use std::sync::Arc;

pub(crate) struct RuntimeTaskPlanCoordinateOwner {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    rows: u32,
}

/// Opaque source-order coordinate. Only the candidate owner resolves ordinals.
#[derive(Clone)]
pub struct RuntimeTaskPlanBuildCoordinate {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    ordinal: u32,
}

impl std::fmt::Debug for RuntimeTaskPlanBuildCoordinate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeTaskPlanBuildCoordinate")
            .field("ordinal", &self.ordinal)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RuntimeTaskPlanBuildCoordinate {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer) && self.ordinal == other.ordinal
    }
}
impl Eq for RuntimeTaskPlanBuildCoordinate {}

impl PartialOrd for RuntimeTaskPlanBuildCoordinate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for RuntimeTaskPlanBuildCoordinate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Owner admission precedes canonical map construction. Within an
        // admitted candidate every issuer is identical, so order is source order.
        Arc::as_ptr(&self.issuer)
            .cmp(&Arc::as_ptr(&other.issuer))
            .then_with(|| self.ordinal.cmp(&other.ordinal))
    }
}

impl RuntimeTaskPlanBuildCoordinate {
    /// Tests original construction ownership without exposing an ordinal or key.
    #[must_use]
    pub fn same_candidate(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer)
    }

    pub(in crate::plan) fn resolve_in<'a>(
        &self,
        semantics: &'a Arc<crate::plan::task_semantic::RuntimeTaskProgramSemantics>,
    ) -> Option<(
        &'a crate::plan::RuntimeTaskPlan,
        crate::task::TaskPlanSemanticDigest,
    )> {
        let original = self.issuer.sealed_tasks.get()?;
        if !Arc::ptr_eq(original, semantics) {
            return None;
        }
        semantics
            .task_plans
            .by_index(crate::plan::RuntimeTaskPlanIndex(self.ordinal))
    }

    /// Resolves only after this original namespace has completed every row.
    /// No assertion, ordinal from another builder, or partial seal issues a key.
    #[must_use]
    pub fn sealed_plan(
        &self,
    ) -> Option<(
        &crate::plan::RuntimeTaskPlan,
        crate::task::TaskPlanSemanticDigest,
    )> {
        self.issuer
            .sealed_tasks
            .get()?
            .task_plans
            .by_index(crate::plan::RuntimeTaskPlanIndex(self.ordinal))
    }

    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

impl RuntimeTaskPlanCoordinateOwner {
    pub(super) fn new(issuer: Arc<RuntimePlanConstructionIssuer>, rows: u32) -> Self {
        Self { issuer, rows }
    }
    pub(in crate::plan) fn ensure_unsealed(
        &self,
    ) -> Result<(), crate::plan::body_semantic::task_image::RuntimeTaskPlanSealError> {
        if self.issuer.sealed_tasks.get().is_some() {
            Err(crate::plan::body_semantic::task_image::RuntimeTaskPlanSealError::AlreadySealed)
        } else {
            Ok(())
        }
    }

    pub(in crate::plan) fn publish(
        &self,
        semantics: Arc<crate::plan::task_semantic::RuntimeTaskProgramSemantics>,
    ) -> Result<(), crate::plan::body_semantic::task_image::RuntimeTaskPlanSealError> {
        self.issuer.sealed_tasks.set(semantics).map_err(|_| {
            crate::plan::body_semantic::task_image::RuntimeTaskPlanSealError::AlreadySealed
        })
    }

    pub(crate) const fn len(&self) -> u32 {
        self.rows
    }
    pub(crate) fn resolve(&self, ordinal: u32) -> Option<RuntimeTaskPlanBuildCoordinate> {
        (ordinal < self.rows).then(|| RuntimeTaskPlanBuildCoordinate {
            issuer: Arc::clone(&self.issuer),
            ordinal,
        })
    }
    pub(crate) fn contains(&self, coordinate: &RuntimeTaskPlanBuildCoordinate) -> bool {
        Arc::ptr_eq(&self.issuer, &coordinate.issuer) && coordinate.ordinal < self.rows
    }
}

impl RuntimePlanBuilder {
    /// Private preparation seam. The final task candidate table supplies its
    /// admitted row count before either builder or decoder starts body sealing.
    pub(crate) fn task_coordinate_owner(&self, rows: u32) -> RuntimeTaskPlanCoordinateOwner {
        RuntimeTaskPlanCoordinateOwner {
            issuer: Arc::clone(&self.issuer),
            rows,
        }
    }
}
