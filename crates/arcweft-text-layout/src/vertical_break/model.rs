//! Public policy, error, score, and explain models for vertical breaking.

use core::ops::Range;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Fixed-point units used for normalized policy terms.
pub const VERTICAL_BREAK_UNITS_PER_EM: u32 = 4_096;
/// Maximum grapheme clusters accepted by one hard-line planning request.
pub const MAX_VERTICAL_BREAK_CLUSTERS: usize = 4_096;

/// Closed vertical-break quality preset.
///
/// The first contract deliberately exposes no arbitrary coefficient surface.
/// Unknown serialized names are rejected by `serde`; adding or changing a
/// preset requires a new stable identity and corpus review.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum VerticalBreakPolicy {
    /// Reviewed Japanese narrative default, version 1.
    #[default]
    BalancedV1,
}

impl VerticalBreakPolicy {
    /// Stable serialized/cache identity of the policy.
    #[must_use]
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::BalancedV1 => "balanced_v1",
        }
    }

    pub(super) const fn coefficients(self) -> PolicyCoefficients {
        match self {
            Self::BalancedV1 => PolicyCoefficients {
                intermediate_raggedness: 4_096,
                final_shortness: 1_536,
                hanging: 192,
                intermediate_break: 64,
                pair_penalty_unit: 16,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PolicyCoefficients {
    pub(super) intermediate_raggedness: u64,
    pub(super) final_shortness: u64,
    pub(super) hanging: u64,
    pub(super) intermediate_break: u64,
    pub(super) pair_penalty_unit: u64,
}

/// Metric role attached to checked vertical-planning failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerticalBreakMetricRole {
    Origin,
    InitialCursor,
    Height,
    ClusterAdvance,
    ReferenceAdvance,
}

/// Vertical planning failed before a deterministic plan could be selected.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum VerticalBreakError {
    /// A required metric was non-finite or negative where non-negative input is required.
    #[error(
        "vertical break metric {role:?} at cluster {cluster_index:?} is non-finite or negative"
    )]
    InvalidMetric {
        role: VerticalBreakMetricRole,
        cluster_index: Option<usize>,
    },
    /// A continuation cursor appeared before the container origin.
    #[error("vertical break initial cursor precedes the container origin")]
    InitialCursorBeforeOrigin,
    /// No positive cluster advance was available to establish normalized em units.
    #[error("vertical break planning requires at least one positive cluster advance")]
    ZeroReferenceAdvance,
    /// A normalized metric exceeded the bounded fixed-point domain.
    #[error(
        "vertical break metric {role:?} at cluster {cluster_index:?} exceeds {maximum_units} normalized units"
    )]
    NormalizedMetricOutOfRange {
        role: VerticalBreakMetricRole,
        cluster_index: Option<usize>,
        maximum_units: u64,
    },
    /// Checked integer evaluation overflowed.
    #[error("vertical break cost arithmetic overflowed")]
    ArithmeticOverflow,
    /// The paragraph exceeded the bounded dynamic-programming work limit.
    #[error(
        "vertical break segment contains {clusters} clusters, exceeding the limit of {maximum}"
    )]
    ResourceLimitExceeded { clusters: usize, maximum: usize },
    /// No terminal state could be reconstructed.
    #[error("vertical break planner produced no terminal plan")]
    NoPlan,
}

/// One shaped cluster submitted to the shared vertical planner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalBreakCluster<'a> {
    /// Visible grapheme text used only for typed JLREQ classification.
    pub text: &'a str,
    /// Shaped inline advance in logical layout units.
    pub advance: f32,
    /// UAX #14 opportunity before this cluster.
    pub break_allowed_before: bool,
}

/// Hard reason a source boundary cannot be selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerticalBreakHardConstraint {
    Uax14Prohibited,
    JlreqLineEndProhibited,
    JlreqLineHeadProhibited,
    JlreqKeepTogether,
}

/// Bounded counts of rejected source boundaries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VerticalBreakRejectionCounts {
    pub uax14_prohibited: u32,
    pub jlreq_line_end_prohibited: u32,
    pub jlreq_line_head_prohibited: u32,
    pub jlreq_keep_together: u32,
}

impl VerticalBreakRejectionCounts {
    pub(super) fn record(&mut self, constraint: VerticalBreakHardConstraint) {
        match constraint {
            VerticalBreakHardConstraint::Uax14Prohibited => self.uax14_prohibited += 1,
            VerticalBreakHardConstraint::JlreqLineEndProhibited => {
                self.jlreq_line_end_prohibited += 1;
            }
            VerticalBreakHardConstraint::JlreqLineHeadProhibited => {
                self.jlreq_line_head_prohibited += 1;
            }
            VerticalBreakHardConstraint::JlreqKeepTogether => self.jlreq_keep_together += 1,
        }
    }
}

/// Integer objective tuple. Every field is minimized in declaration order.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct VerticalBreakScore {
    /// Total non-hanging overflow in 1/4096 representative-em units.
    pub forced_overflow_units: u64,
    /// Number of columns using the forced-overflow escape.
    pub forced_overflow_columns: u32,
    /// Sum of bounded soft terms.
    pub soft_cost: u64,
    /// Number of newly planned columns.
    pub column_count: u32,
}

impl VerticalBreakScore {
    pub(super) fn checked_add(self, column: Self) -> Result<Self, VerticalBreakError> {
        Ok(Self {
            forced_overflow_units: self
                .forced_overflow_units
                .checked_add(column.forced_overflow_units)
                .ok_or(VerticalBreakError::ArithmeticOverflow)?,
            forced_overflow_columns: self
                .forced_overflow_columns
                .checked_add(column.forced_overflow_columns)
                .ok_or(VerticalBreakError::ArithmeticOverflow)?,
            soft_cost: self
                .soft_cost
                .checked_add(column.soft_cost)
                .ok_or(VerticalBreakError::ArithmeticOverflow)?,
            column_count: self
                .column_count
                .checked_add(column.column_count)
                .ok_or(VerticalBreakError::ArithmeticOverflow)?,
        })
    }
}

/// How the final winner was distinguished after objective comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerticalBreakTieBreakReason {
    ObjectiveTuple,
    LaterBreakOffsets,
}

/// Overall status of the selected plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerticalBreakPlanStatus {
    Normal,
    ForcedOverflow,
}

/// Normalized terms for one selected column.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerticalBreakColumnExplain {
    pub cluster_range: Range<usize>,
    pub capacity_units: u64,
    pub used_units: u64,
    pub remaining_units: u64,
    pub allowed_hanging_units: u64,
    pub used_hanging_units: u64,
    pub forced_overflow_units: u64,
    pub raggedness_cost: u64,
    pub final_shortness_cost: u64,
    pub hanging_cost: u64,
    pub intermediate_break_cost: u64,
    pub pair_preference_cost: u64,
}

/// Renderer-neutral, bounded explanation of a selected break plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerticalBreakExplain {
    pub policy: VerticalBreakPolicy,
    /// Exact IEEE-754 bits of the representative physical advance.
    pub reference_advance_bits: u32,
    pub normalized_units_per_em: u32,
    pub initial_capacity_units: u64,
    pub full_capacity_units: u64,
    pub restarted_partial_column: bool,
    pub status: VerticalBreakPlanStatus,
    pub score: VerticalBreakScore,
    pub columns: Vec<VerticalBreakColumnExplain>,
    pub omitted_columns: u32,
    pub rejected: VerticalBreakRejectionCounts,
    pub tie_break: VerticalBreakTieBreakReason,
}

/// Completed shared inline break plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerticalBreakPlan {
    pub(super) break_before: Vec<bool>,
    pub(super) break_offsets: Vec<usize>,
    pub(super) explain: VerticalBreakExplain,
}

impl VerticalBreakPlan {
    /// Returns whether a new vertical column starts before `cluster_index`.
    #[must_use]
    pub fn breaks_before(&self, cluster_index: usize) -> bool {
        self.break_before
            .get(cluster_index)
            .copied()
            .unwrap_or_default()
    }

    /// Selected source-cluster offsets. Offset zero denotes a forced restart of
    /// a partially occupied column at a styled-run continuation boundary.
    #[must_use]
    pub fn break_offsets(&self) -> &[usize] {
        &self.break_offsets
    }

    /// Bounded explanation generated by the same evaluation that selected the plan.
    #[must_use]
    pub const fn explain(&self) -> &VerticalBreakExplain {
        &self.explain
    }
}
