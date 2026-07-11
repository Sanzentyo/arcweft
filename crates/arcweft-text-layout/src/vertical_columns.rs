//! Dynamic-programming column planning for vertical text.

use crate::{
    JlreqStrictness,
    effects::layout_phase_effect_reserve,
    jlreq_punctuation,
    layout::LayoutCursor,
    ruby::ruby_text_extent,
    ruby_metrics::ruby_metrics,
    vertical::{
        RunLayoutContext, vertical_cluster_layout_advance, vertical_cluster_required_inline_extent,
        vertical_cluster_span_layout_advance, vertical_inter_character_ruby_extent_after,
        vertical_ruby_base_allocation_height, vertical_ruby_base_cluster_span,
        vertical_run_can_restart_at_boundary, vertical_side_ruby_annotation_starting_at,
    },
    vertical_breaks::{
        vertical_cluster_can_start_column, vertical_column_ends_with_line_end_prohibited,
        vertical_column_segment_overhang_allowance,
        vertical_column_segment_overhang_uses_linebreak_continuation,
    },
    vertical_clusters::{VerticalCluster, is_vertical_line_break_cluster},
};
use arcweft_render_text::RichTextRange;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerticalColumnPlan {
    pub(crate) break_before: Vec<bool>,
}

impl VerticalColumnPlan {
    fn new(cluster_count: usize) -> Self {
        Self {
            break_before: vec![false; cluster_count],
        }
    }

    fn set_break_before(&mut self, cluster_index: usize) {
        if let Some(value) = self.break_before.get_mut(cluster_index) {
            *value = true;
        }
    }

    pub(crate) fn breaks_before(&self, cluster_index: usize) -> bool {
        self.break_before
            .get(cluster_index)
            .copied()
            .unwrap_or_default()
    }
}

pub(crate) fn plan_vertical_columns(
    clusters: &[VerticalCluster],
    context: RunLayoutContext<'_>,
    initial_cursor: LayoutCursor,
    previous_cluster_text: Option<&str>,
) -> VerticalColumnPlan {
    let mut plan = VerticalColumnPlan::new(clusters.len());
    let mut segment_start = 0;
    let mut segment_initial_y = initial_cursor.y;
    for (cluster_index, cluster) in clusters.iter().enumerate() {
        if is_vertical_line_break_cluster(&cluster.text) {
            plan_vertical_column_segment(
                &mut plan,
                clusters,
                segment_start,
                cluster_index,
                context,
                segment_initial_y,
                previous_cluster_text,
            );
            segment_start = cluster_index + 1;
            segment_initial_y = context.config.origin.y;
        }
    }
    plan_vertical_column_segment(
        &mut plan,
        clusters,
        segment_start,
        clusters.len(),
        context,
        segment_initial_y,
        previous_cluster_text,
    );
    plan
}

#[derive(Clone, Copy, Debug)]
struct VerticalColumnDpState {
    cost: f32,
    previous_break: usize,
}

fn plan_vertical_column_segment(
    plan: &mut VerticalColumnPlan,
    clusters: &[VerticalCluster],
    segment_start: usize,
    segment_end: usize,
    context: RunLayoutContext<'_>,
    initial_cursor_y: f32,
    previous_cluster_text: Option<&str>,
) {
    if segment_start >= segment_end {
        return;
    }

    let continued = solve_vertical_column_segment(
        clusters,
        segment_start,
        segment_end,
        context,
        initial_cursor_y,
    );
    let restarted = (vertical_run_can_restart_at_boundary(
        context.source,
        clusters,
        segment_start,
        context.config.jlreq_strictness,
        previous_cluster_text,
    ) && initial_cursor_y > context.config.origin.y + f32::EPSILON)
        .then(|| {
            solve_vertical_column_segment(
                clusters,
                segment_start,
                segment_end,
                context,
                context.config.origin.y,
            )
            .map(|mut candidate| {
                candidate.cost += 25.0;
                candidate.break_offsets.push(0);
                candidate
            })
        })
        .flatten();
    let candidate = match (continued, restarted) {
        (Some(continued), Some(restarted)) if restarted.cost < continued.cost => restarted,
        (Some(continued), _) => continued,
        (None, Some(restarted)) => restarted,
        (None, None) => return,
    };
    for offset in candidate.break_offsets {
        plan.set_break_before(segment_start + offset);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct VerticalColumnSegmentPlan {
    cost: f32,
    break_offsets: Vec<usize>,
}

fn solve_vertical_column_segment(
    clusters: &[VerticalCluster],
    segment_start: usize,
    segment_end: usize,
    context: RunLayoutContext<'_>,
    initial_cursor_y: f32,
) -> Option<VerticalColumnSegmentPlan> {
    let segment_len = segment_end - segment_start;
    let mut states = vec![None; segment_len + 1];
    states[0] = Some(VerticalColumnDpState {
        cost: 0.0,
        previous_break: 0,
    });

    for relative_start in 0..segment_len {
        let Some(start_state) = states[relative_start] else {
            continue;
        };
        let column_start_y = if relative_start == 0 {
            initial_cursor_y
        } else {
            context.config.origin.y
        };
        let mut relative_end = relative_start + 1;
        while relative_end <= segment_len {
            let absolute_start = segment_start + relative_start;
            let absolute_end = segment_start + relative_end;
            if let Some(column_cost) = vertical_column_segment_cost(
                clusters,
                absolute_start,
                absolute_end,
                segment_end,
                context,
                column_start_y,
            ) {
                let cost = start_state.cost + column_cost;
                if vertical_column_dp_candidate_is_better(
                    states[relative_end],
                    cost,
                    relative_start,
                ) {
                    states[relative_end] = Some(VerticalColumnDpState {
                        cost,
                        previous_break: relative_start,
                    });
                }
            }
            relative_end += 1;
        }
    }

    let mut cursor = segment_len;
    let mut break_offsets = Vec::new();
    while cursor > 0 {
        let state = states[cursor]?;
        if state.previous_break > 0 {
            break_offsets.push(state.previous_break);
        }
        cursor = state.previous_break;
    }
    break_offsets.reverse();
    Some(VerticalColumnSegmentPlan {
        cost: states[segment_len]?.cost,
        break_offsets,
    })
}

fn vertical_column_dp_candidate_is_better(
    current: Option<VerticalColumnDpState>,
    cost: f32,
    previous_break: usize,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    cost < current.cost
        || ((cost - current.cost).abs() <= f32::EPSILON && previous_break > current.previous_break)
}

fn vertical_column_segment_cost(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    segment_end: usize,
    context: RunLayoutContext<'_>,
    column_start_y: f32,
) -> Option<f32> {
    if column_start >= column_end {
        return None;
    }
    if column_end < segment_end
        && !vertical_cluster_can_start_column(column_end, clusters, context.config.jlreq_strictness)
    {
        return None;
    }
    if column_end < segment_end
        && vertical_column_ends_with_line_end_prohibited(column_start, column_end, clusters)
    {
        return None;
    }
    if vertical_column_splits_side_ruby_base(clusters, column_start, column_end, context) {
        return None;
    }

    let capacity = context.config.origin.y + context.config.size.height - column_start_y;
    let used = vertical_column_segment_required_extent(clusters, column_start, column_end, context);
    let overflow = (used - capacity).max(0.0);
    let allowed_overhang = vertical_column_segment_overhang_allowance(
        clusters,
        column_start,
        column_end,
        context.config,
    );
    let overhang_uses_linebreak_continuation =
        vertical_column_segment_overhang_uses_linebreak_continuation(
            clusters,
            column_start,
            column_end,
            context.config,
        );
    if column_end < segment_end && overflow > allowed_overhang + f32::EPSILON {
        return None;
    }
    if column_end == segment_end
        && overflow > allowed_overhang + f32::EPSILON
        && vertical_column_segment_has_valid_overflow_avoiding_break(
            clusters,
            column_start,
            column_end,
            context,
            column_start_y,
        )
    {
        return None;
    }

    let remaining = (capacity - used).max(0.0);
    let capacity = capacity.max(context.config.font_size);
    let badness = 100.0 * (remaining / capacity).powi(3);
    let overflow_penalty =
        ((overflow - allowed_overhang).max(0.0) / context.config.font_size).powi(2) * 10_000.0;
    let allowed_overhang_penalty = if overhang_uses_linebreak_continuation {
        (overflow.min(allowed_overhang) / context.config.font_size).powi(2) * 50.0
    } else {
        0.0
    };
    let break_penalty = if column_end < segment_end {
        5.0 + vertical_column_pair_break_penalty(
            clusters,
            column_start,
            column_end,
            context.config.jlreq_strictness,
        )
    } else {
        0.0
    };
    Some(badness + overflow_penalty + allowed_overhang_penalty + break_penalty)
}

fn vertical_column_segment_has_valid_overflow_avoiding_break(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    context: RunLayoutContext<'_>,
    column_start_y: f32,
) -> bool {
    let capacity = context.config.origin.y + context.config.size.height - column_start_y;
    (column_start + 1..column_end).any(|break_index| {
        if !vertical_cluster_can_start_column(
            break_index,
            clusters,
            context.config.jlreq_strictness,
        ) || vertical_column_ends_with_line_end_prohibited(column_start, break_index, clusters)
            || vertical_column_splits_side_ruby_base(clusters, column_start, break_index, context)
        {
            return false;
        }
        let used =
            vertical_column_segment_required_extent(clusters, column_start, break_index, context);
        let allowed_overhang = vertical_column_segment_overhang_allowance(
            clusters,
            column_start,
            break_index,
            context.config,
        );
        (used - capacity).max(0.0) <= allowed_overhang + f32::EPSILON
    })
}

fn vertical_column_splits_side_ruby_base(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    context: RunLayoutContext<'_>,
) -> bool {
    (0..clusters.len()).any(|cluster_index| {
        let Some(annotation) = vertical_side_ruby_annotation_starting_at(
            context.ruby_annotations,
            context.range_start,
            clusters,
            cluster_index,
        ) else {
            return false;
        };
        let span = vertical_ruby_base_cluster_span(
            annotation,
            context.range_start,
            clusters,
            cluster_index,
        );
        span.start < column_end
            && column_start < span.end
            && (column_start > span.start || column_end < span.end)
    })
}

pub(crate) fn vertical_column_pair_break_penalty(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    strictness: JlreqStrictness,
) -> f32 {
    let Some(left) = clusters[column_start..column_end]
        .iter()
        .rev()
        .find(|cluster| !is_vertical_line_break_cluster(&cluster.text))
    else {
        return 0.0;
    };
    let Some(right) = clusters[column_end..]
        .iter()
        .find(|cluster| !is_vertical_line_break_cluster(&cluster.text))
    else {
        return 0.0;
    };
    f32::from(
        jlreq_punctuation::pair_adjustment_for_clusters(&left.text, &right.text, strictness)
            .break_penalty,
    )
}

fn vertical_column_segment_required_extent(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    context: RunLayoutContext<'_>,
) -> f32 {
    let mut cursor = 0.0f32;
    let mut required = 0.0f32;
    let mut cluster_index = column_start;
    let reserve = layout_phase_effect_reserve(context.presentation);
    while cluster_index < column_end {
        let cluster = &clusters[cluster_index];
        let range = RichTextRange::new(
            context.range_start + cluster.range.start,
            context.range_start + cluster.range.end,
        );
        if let Some(annotation) = vertical_side_ruby_annotation_starting_at(
            context.ruby_annotations,
            context.range_start,
            clusters,
            cluster_index,
        ) {
            let span = vertical_ruby_base_cluster_span(
                annotation,
                context.range_start,
                clusters,
                cluster_index,
            );
            let allocation_extent = vertical_ruby_base_allocation_height(
                vertical_cluster_span_layout_advance(
                    &clusters[span.clone()],
                    context.config,
                    reserve,
                ),
                ruby_text_extent(
                    &annotation.ruby,
                    ruby_metrics(annotation, context.config).font_size,
                ),
                context.config,
            );
            required = required.max(cursor + allocation_extent);
            cursor += allocation_extent;
            cluster_index = span.end.min(column_end);
            continue;
        }
        let required_inline_extent = vertical_cluster_required_inline_extent(
            range,
            context.range_start,
            clusters,
            context.ruby_annotations,
            context.config,
        );
        required = required.max(cursor + required_inline_extent);
        cursor += vertical_cluster_layout_advance(cluster, context.config, reserve);
        cursor += vertical_inter_character_ruby_extent_after(range, context);
        cluster_index += 1;
    }
    required.max(cursor)
}
