//! Checked normalization, hard-constraint filtering, DP scoring, and trace construction.

use core::{cmp::Ordering, ops::Range};

use crate::{JlreqStrictness, jlreq_punctuation};

use super::model::{
    MAX_VERTICAL_BREAK_CLUSTERS, VERTICAL_BREAK_UNITS_PER_EM, VerticalBreakCluster,
    VerticalBreakColumnExplain, VerticalBreakError, VerticalBreakExplain,
    VerticalBreakHardConstraint, VerticalBreakMetricRole, VerticalBreakPlan,
    VerticalBreakPlanStatus, VerticalBreakPolicy, VerticalBreakRejectionCounts, VerticalBreakScore,
    VerticalBreakTieBreakReason,
};

const MAX_NORMALIZED_UNITS: u64 = 16_777_216;
const MAX_EXPLAIN_COLUMNS: usize = 64;

#[derive(Clone, Copy, Debug)]
struct NormalizedCluster<'a> {
    text: &'a str,
    advance: u64,
    break_allowed_before: bool,
}

#[derive(Clone, Copy, Debug)]
struct BreakBoundary {
    allowed: bool,
    pair_penalty: u16,
}

impl BreakBoundary {
    const fn terminal() -> Self {
        Self {
            allowed: true,
            pair_penalty: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DpState {
    pub(super) score: VerticalBreakScore,
    pub(super) previous_break: usize,
    pub(super) tie_break_used: bool,
}

#[derive(Clone, Debug)]
struct SegmentPlan {
    break_offsets: Vec<usize>,
    score: VerticalBreakScore,
    tie_break_used: bool,
}

#[derive(Clone, Debug)]
struct PreparedSegment<'a> {
    clusters: Vec<NormalizedCluster<'a>>,
    prefix: Vec<u64>,
    boundaries: Vec<BreakBoundary>,
    rejected: VerticalBreakRejectionCounts,
    reference_advance: f32,
    initial_capacity: u64,
    full_capacity: u64,
    restarted_partial_column: bool,
}

#[derive(Clone, Copy, Debug)]
struct ColumnEvaluation {
    capacity_units: u64,
    used_units: u64,
    remaining_units: u64,
    allowed_hanging_units: u64,
    used_hanging_units: u64,
    forced_overflow_units: u64,
    raggedness_cost: u64,
    final_shortness_cost: u64,
    hanging_cost: u64,
    intermediate_break_cost: u64,
    pair_preference_cost: u64,
}

impl ColumnEvaluation {
    fn score(self) -> Result<VerticalBreakScore, VerticalBreakError> {
        let soft_cost = [
            self.raggedness_cost,
            self.final_shortness_cost,
            self.hanging_cost,
            self.intermediate_break_cost,
            self.pair_preference_cost,
        ]
        .into_iter()
        .try_fold(0_u64, |sum, value| {
            sum.checked_add(value)
                .ok_or(VerticalBreakError::ArithmeticOverflow)
        })?;
        Ok(VerticalBreakScore {
            forced_overflow_units: self.forced_overflow_units,
            forced_overflow_columns: u32::from(self.forced_overflow_units > 0),
            soft_cost,
            column_count: 1,
        })
    }

    fn explain(self, cluster_range: Range<usize>) -> VerticalBreakColumnExplain {
        VerticalBreakColumnExplain {
            cluster_range,
            capacity_units: self.capacity_units,
            used_units: self.used_units,
            remaining_units: self.remaining_units,
            allowed_hanging_units: self.allowed_hanging_units,
            used_hanging_units: self.used_hanging_units,
            forced_overflow_units: self.forced_overflow_units,
            raggedness_cost: self.raggedness_cost,
            final_shortness_cost: self.final_shortness_cost,
            hanging_cost: self.hanging_cost,
            intermediate_break_cost: self.intermediate_break_cost,
            pair_preference_cost: self.pair_preference_cost,
        }
    }
}

/// Plans one source hard-line using shaped inline advances.
///
/// Hard JLREQ/UAX constraints remove candidate edges. Soft quality terms are
/// evaluated after all physical inputs are normalized to 1/4096 of the lower
/// median positive cluster advance. The final deterministic order is:
/// non-hanging overflow amount, forced-overflow column count, soft cost, column
/// count, then lexicographically later break offsets.
pub fn plan_vertical_breaks(
    clusters: &[VerticalBreakCluster<'_>],
    origin_y: f32,
    initial_y: f32,
    height: f32,
    strictness: JlreqStrictness,
    policy: VerticalBreakPolicy,
) -> Result<VerticalBreakPlan, VerticalBreakError> {
    validate_finite(origin_y, VerticalBreakMetricRole::Origin, None)?;
    validate_finite(initial_y, VerticalBreakMetricRole::InitialCursor, None)?;
    validate_nonnegative(height, VerticalBreakMetricRole::Height, None)?;
    if initial_y < origin_y {
        return Err(VerticalBreakError::InitialCursorBeforeOrigin);
    }
    if clusters.len() > MAX_VERTICAL_BREAK_CLUSTERS {
        return Err(VerticalBreakError::ResourceLimitExceeded {
            clusters: clusters.len(),
            maximum: MAX_VERTICAL_BREAK_CLUSTERS,
        });
    }
    if clusters.is_empty() {
        return Ok(empty_plan(policy));
    }

    let prepared = prepare_segment(clusters, origin_y, initial_y, height, strictness, policy)?;
    select_segment(&prepared, policy)
}

fn empty_plan(policy: VerticalBreakPolicy) -> VerticalBreakPlan {
    VerticalBreakPlan {
        break_before: Vec::new(),
        break_offsets: Vec::new(),
        explain: VerticalBreakExplain {
            policy,
            reference_advance_bits: 0,
            normalized_units_per_em: VERTICAL_BREAK_UNITS_PER_EM,
            initial_capacity_units: 0,
            full_capacity_units: 0,
            restarted_partial_column: false,
            status: VerticalBreakPlanStatus::Normal,
            score: VerticalBreakScore::default(),
            columns: Vec::new(),
            omitted_columns: 0,
            rejected: VerticalBreakRejectionCounts::default(),
            tie_break: VerticalBreakTieBreakReason::ObjectiveTuple,
        },
    }
}

fn prepare_segment<'a>(
    clusters: &[VerticalBreakCluster<'a>],
    origin_y: f32,
    initial_y: f32,
    height: f32,
    strictness: JlreqStrictness,
    policy: VerticalBreakPolicy,
) -> Result<PreparedSegment<'a>, VerticalBreakError> {
    let reference_advance = representative_advance(clusters)?;
    let normalized = normalize_clusters(clusters, reference_advance)?;
    let prefix = prefix_advances(&normalized)?;
    let (boundaries, rejected) = break_boundaries(&normalized, strictness);
    let full_capacity = normalize_metric(
        height,
        reference_advance,
        VerticalBreakMetricRole::Height,
        None,
    )?;
    let partial_capacity = remaining_capacity(origin_y, initial_y, height, reference_advance)?;
    let first_fragment_end =
        first_legal_end(&boundaries, 0, normalized.len()).ok_or(VerticalBreakError::NoPlan)?;
    let first_fragment = evaluate_column(
        &normalized,
        &prefix,
        &boundaries,
        0,
        first_fragment_end,
        partial_capacity,
        policy,
    )?;
    let restarted_partial_column = initial_y > origin_y && first_fragment.forced_overflow_units > 0;
    let initial_capacity = if restarted_partial_column {
        full_capacity
    } else {
        partial_capacity
    };

    Ok(PreparedSegment {
        clusters: normalized,
        prefix,
        boundaries,
        rejected,
        reference_advance,
        initial_capacity,
        full_capacity,
        restarted_partial_column,
    })
}

fn normalize_clusters<'a>(
    clusters: &[VerticalBreakCluster<'a>],
    reference_advance: f32,
) -> Result<Vec<NormalizedCluster<'a>>, VerticalBreakError> {
    clusters
        .iter()
        .enumerate()
        .map(|(index, cluster)| {
            validate_nonnegative(
                cluster.advance,
                VerticalBreakMetricRole::ClusterAdvance,
                Some(index),
            )?;
            Ok(NormalizedCluster {
                text: cluster.text,
                advance: normalize_metric(
                    cluster.advance,
                    reference_advance,
                    VerticalBreakMetricRole::ClusterAdvance,
                    Some(index),
                )?,
                break_allowed_before: cluster.break_allowed_before,
            })
        })
        .collect()
}

fn remaining_capacity(
    origin_y: f32,
    initial_y: f32,
    height: f32,
    reference_advance: f32,
) -> Result<u64, VerticalBreakError> {
    let consumed = initial_y - origin_y;
    if !consumed.is_finite() {
        return Err(VerticalBreakError::NormalizedMetricOutOfRange {
            role: VerticalBreakMetricRole::InitialCursor,
            cluster_index: None,
            maximum_units: MAX_NORMALIZED_UNITS,
        });
    }
    let remaining = if consumed >= height {
        0.0
    } else {
        height - consumed
    };
    normalize_metric(
        remaining,
        reference_advance,
        VerticalBreakMetricRole::Height,
        None,
    )
}

fn select_segment(
    prepared: &PreparedSegment<'_>,
    policy: VerticalBreakPolicy,
) -> Result<VerticalBreakPlan, VerticalBreakError> {
    let selected = solve_segment(
        &prepared.clusters,
        &prepared.prefix,
        &prepared.boundaries,
        prepared.initial_capacity,
        prepared.full_capacity,
        policy,
    )?;
    let mut break_offsets = selected.break_offsets;
    if prepared.restarted_partial_column {
        break_offsets.insert(0, 0);
    }
    let mut break_before = vec![false; prepared.clusters.len()];
    for &offset in &break_offsets {
        if let Some(value) = break_before.get_mut(offset) {
            *value = true;
        }
    }

    let internal_breaks = break_offsets
        .iter()
        .copied()
        .filter(|offset| *offset > 0)
        .collect::<Vec<_>>();
    let mut starts = Vec::with_capacity(internal_breaks.len() + 1);
    starts.push(0);
    starts.extend(internal_breaks.iter().copied());
    let mut ends = internal_breaks;
    ends.push(prepared.clusters.len());
    let total_columns = starts.len();
    let mut columns = Vec::with_capacity(total_columns.min(MAX_EXPLAIN_COLUMNS));
    for (column_index, (start, end)) in starts.into_iter().zip(ends).enumerate() {
        if column_index >= MAX_EXPLAIN_COLUMNS {
            break;
        }
        let capacity = if column_index == 0 {
            prepared.initial_capacity
        } else {
            prepared.full_capacity
        };
        columns.push(
            evaluate_column(
                &prepared.clusters,
                &prepared.prefix,
                &prepared.boundaries,
                start,
                end,
                capacity,
                policy,
            )?
            .explain(start..end),
        );
    }
    let omitted_columns = u32::try_from(total_columns.saturating_sub(columns.len()))
        .map_err(|_| VerticalBreakError::ArithmeticOverflow)?;
    let status = if selected.score.forced_overflow_units == 0 {
        VerticalBreakPlanStatus::Normal
    } else {
        VerticalBreakPlanStatus::ForcedOverflow
    };

    Ok(VerticalBreakPlan {
        break_before,
        break_offsets,
        explain: VerticalBreakExplain {
            policy,
            reference_advance_bits: prepared.reference_advance.to_bits(),
            normalized_units_per_em: VERTICAL_BREAK_UNITS_PER_EM,
            initial_capacity_units: prepared.initial_capacity,
            full_capacity_units: prepared.full_capacity,
            restarted_partial_column: prepared.restarted_partial_column,
            status,
            score: selected.score,
            columns,
            omitted_columns,
            rejected: prepared.rejected,
            tie_break: if selected.tie_break_used {
                VerticalBreakTieBreakReason::LaterBreakOffsets
            } else {
                VerticalBreakTieBreakReason::ObjectiveTuple
            },
        },
    })
}

fn solve_segment(
    clusters: &[NormalizedCluster<'_>],
    prefix: &[u64],
    boundaries: &[BreakBoundary],
    initial_capacity: u64,
    full_capacity: u64,
    policy: VerticalBreakPolicy,
) -> Result<SegmentPlan, VerticalBreakError> {
    let mut states = vec![None; clusters.len() + 1];
    states[0] = Some(DpState {
        score: VerticalBreakScore::default(),
        previous_break: 0,
        tie_break_used: false,
    });

    for start in 0..clusters.len() {
        let Some(start_state) = states[start] else {
            continue;
        };
        let capacity = if start == 0 {
            initial_capacity
        } else {
            full_capacity
        };
        let mut first_legal = None;
        for end in start + 1..=clusters.len() {
            if !boundaries[end].allowed {
                continue;
            }
            let first_legal_end = *first_legal.get_or_insert(end);
            let evaluation =
                evaluate_column(clusters, prefix, boundaries, start, end, capacity, policy)?;
            if evaluation.forced_overflow_units > 0 && end != first_legal_end {
                continue;
            }
            let score = start_state.score.checked_add(evaluation.score()?)?;
            let current = states[end];
            let comparison = compare_candidate(&states, current, score, start)?;
            if comparison.is_better {
                states[end] = Some(DpState {
                    score,
                    previous_break: start,
                    tie_break_used: start_state.tie_break_used || comparison.used_tie_break,
                });
            }
        }
    }

    let terminal = states[clusters.len()].ok_or(VerticalBreakError::NoPlan)?;
    Ok(SegmentPlan {
        break_offsets: state_break_offsets(&states, clusters.len())?,
        score: terminal.score,
        tie_break_used: terminal.tie_break_used,
    })
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CandidateComparison {
    pub(super) is_better: bool,
    pub(super) used_tie_break: bool,
}

pub(super) fn compare_candidate(
    states: &[Option<DpState>],
    current: Option<DpState>,
    candidate_score: VerticalBreakScore,
    candidate_previous: usize,
) -> Result<CandidateComparison, VerticalBreakError> {
    let Some(current) = current else {
        return Ok(CandidateComparison {
            is_better: true,
            used_tie_break: false,
        });
    };
    match candidate_score.cmp(&current.score) {
        Ordering::Less => Ok(CandidateComparison {
            is_better: true,
            used_tie_break: false,
        }),
        Ordering::Greater => Ok(CandidateComparison {
            is_better: false,
            used_tie_break: false,
        }),
        Ordering::Equal => {
            let candidate = candidate_path(states, candidate_previous)?;
            let existing = candidate_path(states, current.previous_break)?;
            Ok(CandidateComparison {
                is_better: candidate > existing,
                used_tie_break: candidate != existing,
            })
        }
    }
}

fn candidate_path(
    states: &[Option<DpState>],
    previous: usize,
) -> Result<Vec<usize>, VerticalBreakError> {
    let mut path = state_break_offsets(states, previous)?;
    if previous > 0 {
        path.push(previous);
    }
    Ok(path)
}

fn state_break_offsets(
    states: &[Option<DpState>],
    endpoint: usize,
) -> Result<Vec<usize>, VerticalBreakError> {
    let mut cursor = endpoint;
    let mut offsets = Vec::new();
    while cursor > 0 {
        let state = states
            .get(cursor)
            .copied()
            .flatten()
            .ok_or(VerticalBreakError::NoPlan)?;
        if state.previous_break > 0 {
            offsets.push(state.previous_break);
        }
        cursor = state.previous_break;
    }
    offsets.reverse();
    Ok(offsets)
}

fn evaluate_column(
    clusters: &[NormalizedCluster<'_>],
    prefix: &[u64],
    boundaries: &[BreakBoundary],
    start: usize,
    end: usize,
    capacity: u64,
    policy: VerticalBreakPolicy,
) -> Result<ColumnEvaluation, VerticalBreakError> {
    let coefficients = policy.coefficients();
    let used = prefix[end]
        .checked_sub(prefix[start])
        .ok_or(VerticalBreakError::ArithmeticOverflow)?;
    let trailing = clusters[end - 1];
    let allowed_hanging = if jlreq_punctuation::is_hanging_cluster(trailing.text) {
        trailing.advance / 2
    } else {
        0
    };
    let overflow = used.saturating_sub(capacity);
    let used_hanging = overflow.min(allowed_hanging);
    let forced_overflow = overflow.saturating_sub(used_hanging);
    let remaining = capacity.saturating_sub(used);
    let is_final = end == clusters.len();

    let raggedness_cost = if is_final || capacity == 0 {
        0
    } else {
        ratio_cost(coefficients.intermediate_raggedness, remaining, capacity, 3)?
    };
    let final_threshold = (capacity / 3).min(2 * u64::from(VERTICAL_BREAK_UNITS_PER_EM));
    let final_shortness_cost = if is_final && start > 0 && final_threshold > 0 {
        ratio_cost(
            coefficients.final_shortness,
            final_threshold.saturating_sub(used),
            final_threshold,
            2,
        )?
    } else {
        0
    };
    let hanging_cost = if used_hanging == 0 || allowed_hanging == 0 {
        0
    } else {
        ratio_cost(coefficients.hanging, used_hanging, allowed_hanging, 2)?
    };
    let intermediate_break_cost = if is_final {
        0
    } else {
        coefficients.intermediate_break
    };
    let pair_preference_cost = if is_final {
        0
    } else {
        u64::from(boundaries[end].pair_penalty)
            .checked_mul(coefficients.pair_penalty_unit)
            .ok_or(VerticalBreakError::ArithmeticOverflow)?
    };

    Ok(ColumnEvaluation {
        capacity_units: capacity,
        used_units: used,
        remaining_units: remaining,
        allowed_hanging_units: allowed_hanging,
        used_hanging_units: used_hanging,
        forced_overflow_units: forced_overflow,
        raggedness_cost,
        final_shortness_cost,
        hanging_cost,
        intermediate_break_cost,
        pair_preference_cost,
    })
}

fn ratio_cost(
    weight: u64,
    numerator: u64,
    denominator: u64,
    exponent: u32,
) -> Result<u64, VerticalBreakError> {
    if numerator == 0 || denominator == 0 {
        return Ok(0);
    }
    let numerator = u128::from(numerator);
    let denominator = u128::from(denominator);
    let numerator_power = checked_power(numerator, exponent)?;
    let denominator_power = checked_power(denominator, exponent)?;
    let scaled = u128::from(weight)
        .checked_mul(numerator_power)
        .ok_or(VerticalBreakError::ArithmeticOverflow)?;
    let rounded = scaled
        .checked_add(denominator_power / 2)
        .ok_or(VerticalBreakError::ArithmeticOverflow)?
        / denominator_power;
    u64::try_from(rounded).map_err(|_| VerticalBreakError::ArithmeticOverflow)
}

fn checked_power(value: u128, exponent: u32) -> Result<u128, VerticalBreakError> {
    match exponent {
        2 => value
            .checked_mul(value)
            .ok_or(VerticalBreakError::ArithmeticOverflow),
        3 => value
            .checked_mul(value)
            .and_then(|square| square.checked_mul(value))
            .ok_or(VerticalBreakError::ArithmeticOverflow),
        _ => Err(VerticalBreakError::ArithmeticOverflow),
    }
}

fn prefix_advances(clusters: &[NormalizedCluster<'_>]) -> Result<Vec<u64>, VerticalBreakError> {
    let mut prefix = Vec::with_capacity(clusters.len() + 1);
    prefix.push(0_u64);
    for cluster in clusters {
        let next = prefix
            .last()
            .copied()
            .unwrap_or_default()
            .checked_add(cluster.advance)
            .ok_or(VerticalBreakError::ArithmeticOverflow)?;
        prefix.push(next);
    }
    Ok(prefix)
}

fn break_boundaries(
    clusters: &[NormalizedCluster<'_>],
    strictness: JlreqStrictness,
) -> (Vec<BreakBoundary>, VerticalBreakRejectionCounts) {
    let mut boundaries = vec![BreakBoundary::terminal(); clusters.len() + 1];
    let mut rejected = VerticalBreakRejectionCounts::default();
    for index in 1..clusters.len() {
        let left = clusters[index - 1];
        let right = clusters[index];
        let pair =
            jlreq_punctuation::pair_adjustment_for_clusters(left.text, right.text, strictness);
        let rejection = if !right.break_allowed_before {
            Some(VerticalBreakHardConstraint::Uax14Prohibited)
        } else if jlreq_punctuation::is_line_end_prohibited_cluster(left.text) {
            Some(VerticalBreakHardConstraint::JlreqLineEndProhibited)
        } else if jlreq_punctuation::is_line_head_prohibited_cluster(right.text) {
            Some(VerticalBreakHardConstraint::JlreqLineHeadProhibited)
        } else if pair.keep_together {
            Some(VerticalBreakHardConstraint::JlreqKeepTogether)
        } else {
            None
        };
        if let Some(constraint) = rejection {
            rejected.record(constraint);
        }
        boundaries[index] = BreakBoundary {
            allowed: rejection.is_none(),
            pair_penalty: pair.break_penalty,
        };
    }
    (boundaries, rejected)
}

fn first_legal_end(boundaries: &[BreakBoundary], start: usize, terminal: usize) -> Option<usize> {
    (start + 1..=terminal).find(|&end| boundaries[end].allowed)
}

fn representative_advance(
    clusters: &[VerticalBreakCluster<'_>],
) -> Result<f32, VerticalBreakError> {
    let mut positive = clusters
        .iter()
        .enumerate()
        .map(|(index, cluster)| {
            validate_nonnegative(
                cluster.advance,
                VerticalBreakMetricRole::ClusterAdvance,
                Some(index),
            )?;
            Ok(cluster.advance)
        })
        .collect::<Result<Vec<_>, VerticalBreakError>>()?;
    positive.retain(|advance| *advance > 0.0);
    positive.sort_by(f32::total_cmp);
    let reference = positive
        .get((positive.len().saturating_sub(1)) / 2)
        .copied()
        .ok_or(VerticalBreakError::ZeroReferenceAdvance)?;
    validate_nonnegative(reference, VerticalBreakMetricRole::ReferenceAdvance, None)?;
    Ok(reference)
}

/// Rounds a positive finite IEEE-754 ratio to the nearest fixed-point quantum
/// without converting through an architecture-dependent floating ordering.
pub(super) fn normalize_metric(
    value: f32,
    reference: f32,
    role: VerticalBreakMetricRole,
    cluster_index: Option<usize>,
) -> Result<u64, VerticalBreakError> {
    if !value.is_finite() || value < 0.0 || !reference.is_finite() || reference <= 0.0 {
        return Err(VerticalBreakError::InvalidMetric {
            role,
            cluster_index,
        });
    }
    if value == 0.0 {
        return Ok(0);
    }

    let (value_significand, value_exponent) = positive_binary_parts(value)?;
    let (reference_significand, reference_exponent) = positive_binary_parts(reference)?;
    let exponent_delta = i32::from(value_exponent) - i32::from(reference_exponent);
    let mut numerator = u128::from(value_significand)
        .checked_mul(u128::from(VERTICAL_BREAK_UNITS_PER_EM))
        .ok_or(VerticalBreakError::ArithmeticOverflow)?;
    let mut denominator = u128::from(reference_significand);

    if exponent_delta >= 0 {
        let shift =
            u32::try_from(exponent_delta).map_err(|_| VerticalBreakError::ArithmeticOverflow)?;
        numerator = checked_left_shift(numerator, shift).ok_or(
            VerticalBreakError::NormalizedMetricOutOfRange {
                role,
                cluster_index,
                maximum_units: MAX_NORMALIZED_UNITS,
            },
        )?;
    } else {
        let shift = exponent_delta.unsigned_abs();
        let Some(shifted) = checked_left_shift(denominator, shift) else {
            return Ok(0);
        };
        denominator = shifted;
    }

    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let half = denominator / 2;
    let round_up = remainder > half || (denominator % 2 == 0 && remainder == half);
    let round_increment = u128::from(round_up);
    let rounded = quotient
        .checked_add(round_increment)
        .ok_or(VerticalBreakError::ArithmeticOverflow)?;
    let normalized =
        u64::try_from(rounded).map_err(|_| VerticalBreakError::NormalizedMetricOutOfRange {
            role,
            cluster_index,
            maximum_units: MAX_NORMALIZED_UNITS,
        })?;
    if normalized > MAX_NORMALIZED_UNITS {
        return Err(VerticalBreakError::NormalizedMetricOutOfRange {
            role,
            cluster_index,
            maximum_units: MAX_NORMALIZED_UNITS,
        });
    }
    Ok(normalized)
}

fn positive_binary_parts(value: f32) -> Result<(u64, i16), VerticalBreakError> {
    let bits = value.to_bits();
    let exponent_bits =
        u8::try_from((bits >> 23) & 0xff).map_err(|_| VerticalBreakError::ArithmeticOverflow)?;
    let fraction = bits & 0x7f_ffff;
    if exponent_bits == 0 {
        Ok((u64::from(fraction), -149))
    } else {
        Ok((
            u64::from((1_u32 << 23) | fraction),
            i16::from(exponent_bits) - 127 - 23,
        ))
    }
}

fn checked_left_shift(value: u128, shift: u32) -> Option<u128> {
    if shift >= u128::BITS || value > (u128::MAX >> shift) {
        None
    } else {
        Some(value << shift)
    }
}

fn validate_finite(
    value: f32,
    role: VerticalBreakMetricRole,
    cluster_index: Option<usize>,
) -> Result<(), VerticalBreakError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(VerticalBreakError::InvalidMetric {
            role,
            cluster_index,
        })
    }
}

fn validate_nonnegative(
    value: f32,
    role: VerticalBreakMetricRole,
    cluster_index: Option<usize>,
) -> Result<(), VerticalBreakError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(VerticalBreakError::InvalidMetric {
            role,
            cluster_index,
        })
    }
}
