//! Vertical glyph placement and ruby-aware run integration.

use crate::{
    JlreqStrictness, LaidOutGlyph, LayoutPoint, LayoutRect, LayoutSize, TextLayoutConfig,
    effects::{LayoutEffectReserve, layout_phase_effect_reserve},
    geometry::ranges_overlap,
    horizontal::horizontal_text_layout_advance,
    jlreq_punctuation,
    layout::{LayoutCursor, TextLayoutState},
    ruby::ruby_text_extent,
    ruby_metrics::{ruby_metrics, ruby_position},
    vertical_breaks::vertical_cluster_origin_y,
    vertical_clusters::{
        VerticalCluster, cluster_is_sideways_latin_run, is_vertical_line_break_cluster,
        vertical_clusters,
    },
    vertical_columns::plan_vertical_columns,
};
use arcweft_render_text::{
    RichTextPresentation, RichTextRange, RichTextRubyAnnotation, RichTextRubyPosition,
    RichTextTextSource, RichTextVerticalLatinMode, RichTextWritingMode,
};
use std::ops::Range;

fn vertical_glyph_bounds(
    column_x: f32,
    glyph_y: f32,
    cluster: &VerticalCluster,
    config: TextLayoutConfig,
) -> LayoutRect {
    let width = config.font_size.max(1.0).min(config.line_advance.max(1.0));
    let height = if cluster_is_sideways_latin_run(cluster) {
        vertical_cluster_advance(cluster, config)
    } else {
        config.font_size.max(1.0)
    };
    LayoutRect::new(column_x, glyph_y, width, height)
}

fn vertical_glyph_bounds_with_reserve(
    column_x: f32,
    glyph_y: f32,
    cluster: &VerticalCluster,
    config: TextLayoutConfig,
    reserve: LayoutEffectReserve,
) -> LayoutRect {
    let mut bounds = vertical_glyph_bounds(column_x, glyph_y, cluster, config);
    bounds.x -= reserve.x;
    bounds.y -= reserve.y;
    bounds.width += reserve.x * 2.0;
    bounds.height += reserve.y * 2.0;
    bounds
}

pub(crate) fn layout_vertical_run(
    glyphs: &mut Vec<LaidOutGlyph>,
    text: &str,
    writing_mode: RichTextWritingMode,
    vertical_latin: RichTextVerticalLatinMode,
    context: RunLayoutContext<'_>,
    state: &mut TextLayoutState,
) {
    let config = context.config;
    let column_step = vertical_column_step(writing_mode, context.presentation, config);
    let previous_cluster = match writing_mode {
        RichTextWritingMode::VerticalRl => state.vertical_rl_previous_cluster.clone(),
        RichTextWritingMode::VerticalLr => state.vertical_lr_previous_cluster.clone(),
        RichTextWritingMode::HorizontalTb => {
            unreachable!("horizontal runs use layout_horizontal_run")
        }
    };
    let cursor = match writing_mode {
        RichTextWritingMode::VerticalRl => &mut state.vertical_rl,
        RichTextWritingMode::VerticalLr => &mut state.vertical_lr,
        RichTextWritingMode::HorizontalTb => {
            unreachable!("horizontal runs use layout_horizontal_run")
        }
    };
    let clusters = vertical_clusters(text, vertical_latin);
    let column_plan =
        plan_vertical_columns(&clusters, context, *cursor, previous_cluster.as_deref());
    let mut next_previous_cluster = previous_cluster;
    let mut cluster_index = 0usize;
    while cluster_index < clusters.len() {
        let cluster = &clusters[cluster_index];
        if is_vertical_line_break_cluster(&cluster.text) {
            cursor.x += column_step;
            cursor.y = config.origin.y;
            next_previous_cluster = None;
            cluster_index += 1;
            continue;
        }
        let start = context.range_start + cluster.range.start;
        let end = context.range_start + cluster.range.end;
        let range = RichTextRange::new(start, end);
        if column_plan.breaks_before(cluster_index) {
            cursor.x += column_step;
            cursor.y = config.origin.y;
        }
        if let Some((next_cluster_index, previous_cluster)) = layout_vertical_side_ruby_base(
            glyphs,
            &clusters,
            cluster_index,
            cursor,
            writing_mode,
            context,
        ) {
            next_previous_cluster = previous_cluster;
            cluster_index = next_cluster_index;
            continue;
        }
        let reserve = layout_phase_effect_reserve(context.presentation);
        let advance = vertical_cluster_layout_advance(cluster, config, reserve);
        let glyph_y =
            vertical_cluster_origin_y(&cluster.text, cursor.y + reserve.y, advance, config);
        push_vertical_glyph(
            glyphs,
            cluster,
            range,
            cursor.x,
            glyph_y,
            writing_mode,
            context,
        );
        cursor.y += advance;
        cursor.y += vertical_inter_character_ruby_extent_after(range, context);
        next_previous_cluster = Some(cluster.text.clone());
        cluster_index += 1;
    }
    match writing_mode {
        RichTextWritingMode::VerticalRl => {
            state.vertical_rl_previous_cluster = next_previous_cluster;
        }
        RichTextWritingMode::VerticalLr => {
            state.vertical_lr_previous_cluster = next_previous_cluster;
        }
        RichTextWritingMode::HorizontalTb => {
            unreachable!("horizontal runs use layout_horizontal_run")
        }
    }
}

fn layout_vertical_side_ruby_base(
    glyphs: &mut Vec<LaidOutGlyph>,
    clusters: &[VerticalCluster],
    cluster_index: usize,
    cursor: &mut LayoutCursor,
    writing_mode: RichTextWritingMode,
    context: RunLayoutContext<'_>,
) -> Option<(usize, Option<String>)> {
    let annotation = vertical_side_ruby_annotation_starting_at(
        context.ruby_annotations,
        context.range_start,
        clusters,
        cluster_index,
    )?;
    let base_span =
        vertical_ruby_base_cluster_span(annotation, context.range_start, clusters, cluster_index);
    let reserve = layout_phase_effect_reserve(context.presentation);
    let base_extent =
        vertical_cluster_span_layout_advance(&clusters[base_span.clone()], context.config, reserve);
    let metrics = ruby_metrics(annotation, context.config);
    let allocation_extent = vertical_ruby_base_allocation_height(
        base_extent,
        ruby_text_extent(&annotation.ruby, metrics.font_size),
        context.config,
    );
    let mut previous_cluster = None;
    let mut glyph_y = cursor.y;
    for base_cluster in &clusters[base_span.clone()] {
        let base_range = RichTextRange::new(
            context.range_start + base_cluster.range.start,
            context.range_start + base_cluster.range.end,
        );
        push_vertical_glyph(
            glyphs,
            base_cluster,
            base_range,
            cursor.x,
            glyph_y + reserve.y,
            writing_mode,
            context,
        );
        glyph_y += vertical_cluster_layout_advance(base_cluster, context.config, reserve);
        previous_cluster = Some(base_cluster.text.clone());
    }
    cursor.y += allocation_extent;
    Some((base_span.end, previous_cluster))
}

fn push_vertical_glyph(
    glyphs: &mut Vec<LaidOutGlyph>,
    cluster: &VerticalCluster,
    range: RichTextRange,
    column_x: f32,
    glyph_y: f32,
    writing_mode: RichTextWritingMode,
    context: RunLayoutContext<'_>,
) {
    let reserve = layout_phase_effect_reserve(context.presentation);
    let advance = vertical_cluster_layout_advance(cluster, context.config, reserve);
    glyphs.push(LaidOutGlyph {
        run_index: context.run_index,
        range,
        text: cluster.text.clone(),
        origin: LayoutPoint::new(column_x, glyph_y),
        advance: LayoutSize::new(0.0, advance),
        bounds: vertical_glyph_bounds_with_reserve(
            column_x,
            glyph_y,
            cluster,
            context.config,
            reserve,
        ),
        writing_mode,
        orientation: cluster.orientation,
        vertical_form: cluster.vertical_form,
        presentation: context.presentation.clone(),
    });
}

#[derive(Clone, Copy)]
pub(crate) struct RunLayoutContext<'a> {
    pub(crate) run_index: usize,
    pub(crate) range_start: usize,
    pub(crate) source: RichTextTextSource,
    pub(crate) presentation: &'a RichTextPresentation,
    pub(crate) ruby_annotations: &'a [RichTextRubyAnnotation],
    pub(crate) config: TextLayoutConfig,
}

pub(crate) fn vertical_run_can_restart_at_boundary(
    source: RichTextTextSource,
    clusters: &[VerticalCluster],
    segment_start: usize,
    strictness: JlreqStrictness,
    previous_cluster_text: Option<&str>,
) -> bool {
    if !matches!(
        source,
        RichTextTextSource::Text
            | RichTextTextSource::Interpolation
            | RichTextTextSource::InterpolationFallback
            | RichTextTextSource::ControlRaw
    ) {
        return false;
    }
    if segment_start > 0 {
        return true;
    }
    let Some(first_cluster) = clusters[segment_start..]
        .iter()
        .find(|cluster| !is_vertical_line_break_cluster(&cluster.text))
    else {
        return true;
    };
    if jlreq_punctuation::is_line_head_prohibited_cluster(&first_cluster.text) {
        return false;
    }
    if let Some(previous) = previous_cluster_text {
        let rule = jlreq_punctuation::pair_adjustment_for_clusters(
            previous,
            &first_cluster.text,
            strictness,
        );
        if rule.keep_together || rule.break_penalty > 0 {
            return false;
        }
    }
    true
}

pub(crate) fn vertical_cluster_required_inline_extent(
    range: RichTextRange,
    range_start: usize,
    clusters: &[VerticalCluster],
    ruby_annotations: &[RichTextRubyAnnotation],
    config: TextLayoutConfig,
) -> f32 {
    ruby_annotations
        .iter()
        .filter(|annotation| annotation.base_range.start == range.start)
        .filter(|annotation| ranges_overlap(annotation.base_range, range))
        .filter(|annotation| {
            !matches!(
                ruby_position(annotation),
                RichTextRubyPosition::InterCharacter
            )
        })
        .map(|annotation| {
            let base_cluster_extent = vertical_ruby_base_cluster_extent(
                annotation.base_range,
                range_start,
                clusters,
                config,
            );
            ruby_text_extent(&annotation.ruby, ruby_metrics(annotation, config).font_size)
                .max(base_cluster_extent)
        })
        .fold(config.font_size, f32::max)
        .min(config.size.height)
}

pub(crate) fn vertical_side_ruby_annotation_starting_at<'a>(
    ruby_annotations: &'a [RichTextRubyAnnotation],
    range_start: usize,
    clusters: &[VerticalCluster],
    cluster_index: usize,
) -> Option<&'a RichTextRubyAnnotation> {
    let cluster = clusters.get(cluster_index)?;
    let absolute_start = range_start + cluster.range.start;
    let range_end = range_start + clusters.last().map_or(0, |cluster| cluster.range.end);
    ruby_annotations.iter().find(|annotation| {
        annotation.base_range.start == absolute_start
            && annotation.base_range.start < annotation.base_range.end
            && annotation.base_range.end <= range_end
            && !matches!(
                ruby_position(annotation),
                RichTextRubyPosition::InterCharacter
            )
    })
}

pub(crate) fn vertical_ruby_base_cluster_span(
    annotation: &RichTextRubyAnnotation,
    range_start: usize,
    clusters: &[VerticalCluster],
    cluster_index: usize,
) -> Range<usize> {
    let mut end = cluster_index;
    while let Some(cluster) = clusters.get(end) {
        if is_vertical_line_break_cluster(&cluster.text) {
            break;
        }
        end += 1;
        if range_start + cluster.range.end >= annotation.base_range.end {
            break;
        }
    }
    cluster_index..end.max(cluster_index + 1).min(clusters.len())
}

pub(crate) fn vertical_cluster_span_layout_advance(
    clusters: &[VerticalCluster],
    config: TextLayoutConfig,
    reserve: LayoutEffectReserve,
) -> f32 {
    clusters
        .iter()
        .filter(|cluster| !is_vertical_line_break_cluster(&cluster.text))
        .map(|cluster| vertical_cluster_layout_advance(cluster, config, reserve))
        .sum::<f32>()
        .max(config.font_size)
}

pub(crate) fn vertical_ruby_base_allocation_height(
    base_extent: f32,
    ruby_extent: f32,
    config: TextLayoutConfig,
) -> f32 {
    ruby_extent.max(base_extent).min(config.size.height)
}

pub(crate) fn vertical_inter_character_ruby_extent_after(
    range: RichTextRange,
    context: RunLayoutContext<'_>,
) -> f32 {
    context
        .ruby_annotations
        .iter()
        .filter(|annotation| annotation.base_range.start == range.start)
        .filter(|annotation| ranges_overlap(annotation.base_range, range))
        .filter(|annotation| {
            matches!(
                ruby_position(annotation),
                RichTextRubyPosition::InterCharacter
            )
        })
        .map(|annotation| ruby_text_extent(&annotation.ruby, context.config.ruby_font_size))
        .sum()
}

pub(crate) fn vertical_cluster_advance(cluster: &VerticalCluster, config: TextLayoutConfig) -> f32 {
    if jlreq_punctuation::is_compressible_cluster(&cluster.text) {
        config.font_size * 0.5
    } else if cluster_is_sideways_latin_run(cluster) {
        horizontal_text_layout_advance(
            &cluster.text,
            config.font_size,
            LayoutEffectReserve::default(),
        )
        .max(config.font_size)
    } else {
        config.font_size
    }
}

pub(crate) fn vertical_cluster_layout_advance(
    cluster: &VerticalCluster,
    config: TextLayoutConfig,
    reserve: LayoutEffectReserve,
) -> f32 {
    vertical_cluster_advance(cluster, config) + reserve.y * 2.0
}

fn vertical_ruby_base_cluster_extent(
    base_range: RichTextRange,
    range_start: usize,
    clusters: &[VerticalCluster],
    config: TextLayoutConfig,
) -> f32 {
    clusters
        .iter()
        .filter(|cluster| {
            let start = range_start + cluster.range.start;
            let end = range_start + cluster.range.end;
            ranges_overlap(RichTextRange::new(start, end), base_range)
                && !is_vertical_line_break_cluster(&cluster.text)
        })
        .map(|cluster| vertical_cluster_advance(cluster, config))
        .sum::<f32>()
        .max(config.font_size)
}

pub(crate) fn vertical_column_start(
    writing_mode: RichTextWritingMode,
    config: TextLayoutConfig,
) -> f32 {
    match writing_mode {
        RichTextWritingMode::VerticalRl => {
            config.origin.x + config.size.width - config.line_advance
        }
        RichTextWritingMode::VerticalLr | RichTextWritingMode::HorizontalTb => config.origin.x,
    }
}

fn vertical_column_step(
    writing_mode: RichTextWritingMode,
    presentation: &RichTextPresentation,
    config: TextLayoutConfig,
) -> f32 {
    let gap = presentation
        .layout
        .as_ref()
        .map_or(8.0, |layout| layout.column_gap.as_f32());
    let step = config.line_advance + gap;
    match writing_mode {
        RichTextWritingMode::VerticalRl => -step,
        RichTextWritingMode::VerticalLr | RichTextWritingMode::HorizontalTb => step,
    }
}
