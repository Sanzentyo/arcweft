//! Construction-only task coordinates share the sole aggregate issuer.

use super::{RuntimePlanBuilder, seed::RuntimePlanConstructionIssuer};
use std::sync::Arc;

pub(crate) struct RuntimeTaskPlanCoordinateOwner {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    rows: u32,
}

/// Opaque source-order coordinate. Only the candidate owner resolves ordinals.
pub(crate) struct RuntimeTaskPlanBuildCoordinate {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    ordinal: u32,
}

impl RuntimeTaskPlanBuildCoordinate {
    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

impl RuntimeTaskPlanCoordinateOwner {
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
