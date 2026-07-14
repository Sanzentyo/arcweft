//! Deterministic Japanese vertical-column break quality policy.
//!
//! This boundary owns the closed policy identity, checked fixed-point objective,
//! JLREQ/UAX hard-constraint integration, total tie-break, and bounded explain
//! output. Rendering adapters consume only the selected common layout.

mod model;
mod planner;

#[cfg(test)]
mod tests;

pub use model::{
    MAX_VERTICAL_BREAK_CLUSTERS, VERTICAL_BREAK_UNITS_PER_EM, VerticalBreakCluster,
    VerticalBreakColumnExplain, VerticalBreakError, VerticalBreakExplain,
    VerticalBreakHardConstraint, VerticalBreakMetricRole, VerticalBreakPlan,
    VerticalBreakPlanStatus, VerticalBreakPolicy, VerticalBreakRejectionCounts, VerticalBreakScore,
    VerticalBreakTieBreakReason,
};
pub use planner::plan_vertical_breaks;
