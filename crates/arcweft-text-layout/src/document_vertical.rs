//! Shaped-cluster column planning for canonical vertical document layout.

use crate::{JlreqStrictness, jlreq_punctuation};

#[derive(Clone, Copy)]
pub(crate) struct VerticalPlanCluster<'a> {
    pub(crate) text: &'a str,
    pub(crate) advance: f32,
    pub(crate) break_allowed_before: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerticalColumnPlan {
    break_before: Vec<bool>,
}

impl VerticalColumnPlan {
    #[must_use]
    pub(crate) fn breaks_before(&self, cluster_index: usize) -> bool {
        self.break_before
            .get(cluster_index)
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
struct DpState {
    cost: f64,
    previous_break: usize,
}

struct SegmentPlan {
    break_offsets: Vec<usize>,
}

/// Plans one source hard-line using shaped inline advances.
///
/// The first column continues an earlier styled run whenever its first legal
/// line fragment still fits. Authored run boundaries must not themselves
/// change column composition.
pub(crate) fn plan_vertical_segment(
    clusters: &[VerticalPlanCluster<'_>],
    origin_y: f32,
    initial_y: f32,
    height: f32,
    strictness: JlreqStrictness,
) -> VerticalColumnPlan {
    if clusters.is_empty() {
        return VerticalColumnPlan {
            break_before: Vec::new(),
        };
    }
    let remaining = (origin_y + height - initial_y).max(0.0);
    let first_fragment_end = (1..=clusters.len())
        .find(|&end| end == clusters.len() || break_is_allowed(clusters, end, strictness))
        .unwrap_or(clusters.len());
    let first_fragment_advance = clusters[..first_fragment_end]
        .iter()
        .map(|cluster| cluster.advance)
        .sum::<f32>();
    let must_restart =
        initial_y > origin_y + f32::EPSILON && first_fragment_advance > remaining + f32::EPSILON;
    let mut selected = solve_segment(
        clusters,
        origin_y,
        if must_restart { origin_y } else { initial_y },
        height,
        strictness,
    );
    if must_restart {
        selected.break_offsets.push(0);
    }
    let mut break_before = vec![false; clusters.len()];
    for offset in selected.break_offsets {
        if let Some(value) = break_before.get_mut(offset) {
            *value = true;
        }
    }
    VerticalColumnPlan { break_before }
}

fn solve_segment(
    clusters: &[VerticalPlanCluster<'_>],
    origin_y: f32,
    initial_y: f32,
    height: f32,
    strictness: JlreqStrictness,
) -> SegmentPlan {
    let mut states = vec![None; clusters.len() + 1];
    states[0] = Some(DpState {
        cost: 0.0,
        previous_break: 0,
    });
    for start in 0..clusters.len() {
        let Some(start_state) = states[start] else {
            continue;
        };
        let column_y = if start == 0 { initial_y } else { origin_y };
        let capacity = (origin_y + height - column_y).max(0.0);
        let mut used = 0.0;
        for end in start + 1..=clusters.len() {
            used += clusters[end - 1].advance;
            if end < clusters.len() && !break_is_allowed(clusters, end, strictness) {
                continue;
            }
            let cost =
                start_state.cost + column_cost(clusters, start, end, capacity, used, strictness);
            if candidate_is_better(states[end], cost, start) {
                states[end] = Some(DpState {
                    cost,
                    previous_break: start,
                });
            }
        }
    }

    let mut cursor = clusters.len();
    let mut break_offsets = Vec::new();
    while cursor > 0 {
        let state = states[cursor]
            .expect("the complete segment is always a valid overflowing terminal column");
        if state.previous_break > 0 {
            break_offsets.push(state.previous_break);
        }
        cursor = state.previous_break;
    }
    break_offsets.reverse();
    SegmentPlan { break_offsets }
}

fn break_is_allowed(
    clusters: &[VerticalPlanCluster<'_>],
    index: usize,
    strictness: JlreqStrictness,
) -> bool {
    let left = clusters[index - 1];
    let right = clusters[index];
    right.break_allowed_before
        && !jlreq_punctuation::is_line_end_prohibited_cluster(left.text)
        && !jlreq_punctuation::is_line_head_prohibited_cluster(right.text)
        && !jlreq_punctuation::pair_adjustment_for_clusters(left.text, right.text, strictness)
            .keep_together
}

fn column_cost(
    clusters: &[VerticalPlanCluster<'_>],
    start: usize,
    end: usize,
    capacity: f32,
    used: f32,
    strictness: JlreqStrictness,
) -> f64 {
    let trailing = clusters[end - 1];
    let allowed_overhang = if jlreq_punctuation::is_hanging_cluster(trailing.text) {
        trailing.advance * 0.5
    } else {
        0.0
    };
    let overflow = (used - capacity).max(0.0);
    let effective_overflow = (overflow - allowed_overhang).max(0.0);
    let scale = clusters[start].advance.max(1.0);
    let remaining = (capacity - used).max(0.0);
    let normalized_capacity = capacity.max(scale);
    let raggedness = 100.0 * f64::from(remaining / normalized_capacity).powi(3);
    let overflow_penalty = f64::from(effective_overflow / scale).powi(2) * 10_000.0;
    let hanging_penalty = f64::from(overflow.min(allowed_overhang) / scale).powi(2) * 50.0;
    let break_penalty = if end < clusters.len() {
        5.0 + f64::from(
            jlreq_punctuation::pair_adjustment_for_clusters(
                clusters[end - 1].text,
                clusters[end].text,
                strictness,
            )
            .break_penalty,
        )
    } else {
        0.0
    };
    raggedness + overflow_penalty + hanging_penalty + break_penalty
}

fn candidate_is_better(current: Option<DpState>, cost: f64, previous_break: usize) -> bool {
    let Some(current) = current else {
        return true;
    };
    cost < current.cost
        || ((cost - current.cost).abs() <= f64::EPSILON && previous_break > current.previous_break)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_keeps_leaders_together() {
        let clusters = [
            cluster("月"),
            cluster("火"),
            cluster("…"),
            cluster("…"),
            cluster("人"),
        ];
        let normal = plan_vertical_segment(&clusters, 0.0, 0.0, 90.0, JlreqStrictness::Normal);

        assert!(!normal.breaks_before(3));
    }

    #[test]
    fn strictness_changes_closing_opening_paragraph_plan() {
        let clusters =
            ["天", "地", "。", "「", "人", "山", "川", "海"].map(|text| VerticalPlanCluster {
                text,
                advance: if text == "。" { 15.0 } else { 30.0 },
                break_allowed_before: true,
            });
        let loose = plan_vertical_segment(&clusters, 0.0, 0.0, 105.0, JlreqStrictness::Loose);
        let strict = plan_vertical_segment(&clusters, 0.0, 0.0, 105.0, JlreqStrictness::Strict);

        assert_ne!(loose, strict, "loose={loose:?}, strict={strict:?}");
    }

    #[test]
    fn vertical_lr_and_rl_share_the_same_inline_break_plan() {
        let clusters = [cluster("天"), cluster("地"), cluster("春"), cluster("夏")];
        let plan = plan_vertical_segment(&clusters, 10.0, 10.0, 60.0, JlreqStrictness::Normal);

        assert!(plan.breaks_before(2));
    }

    #[test]
    fn styled_run_boundary_continues_the_current_column_when_content_fits() {
        let clusters = [cluster("2026")];
        let plan = plan_vertical_segment(&clusters, 10.0, 70.0, 180.0, JlreqStrictness::Normal);

        assert!(!plan.breaks_before(0));
    }

    #[test]
    fn styled_run_boundary_restarts_when_its_first_legal_fragment_does_not_fit() {
        let clusters = [cluster("春"), cluster("夏")];
        let plan = plan_vertical_segment(&clusters, 10.0, 165.0, 180.0, JlreqStrictness::Normal);

        assert!(plan.breaks_before(0));
    }

    const fn cluster(text: &str) -> VerticalPlanCluster<'_> {
        VerticalPlanCluster {
            text,
            advance: 30.0,
            break_allowed_before: true,
        }
    }
}
