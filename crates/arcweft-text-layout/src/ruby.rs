use crate::{
    LaidOutGlyph, LaidOutRuby, LayoutRect, TextLayoutConfig,
    geometry::{ranges_overlap, union_bounds, usize_to_f32},
    ruby_metrics::{
        HORIZONTAL_RUBY_HTML_OVERLAP_EM, RubyMetrics, VERTICAL_RUBY_HTML_OVERLAP_EM,
        max_ruby_chars_per_vertical_segment, ruby_metrics, ruby_metrics_from_presentation,
        ruby_position, vertical_ruby_continuation_track_step,
    },
};
use arcweft_render_text::{
    LineDisplayFrame, RichTextRubyAnnotation, RichTextRubyPosition, RichTextWritingMode,
};

pub(super) fn layout_ruby(
    frame: &LineDisplayFrame,
    glyphs: &[LaidOutGlyph],
    config: TextLayoutConfig,
) -> Vec<LaidOutRuby> {
    let mut ruby = frame
        .display_map
        .ruby_annotations
        .iter()
        .enumerate()
        .flat_map(|(ruby_index, annotation)| {
            layout_one_ruby(ruby_index, annotation, glyphs, config)
        })
        .collect::<Vec<_>>();
    resolve_ruby_collisions(&mut ruby, config);
    ruby
}

fn layout_one_ruby(
    ruby_index: usize,
    annotation: &RichTextRubyAnnotation,
    glyphs: &[LaidOutGlyph],
    config: TextLayoutConfig,
) -> Vec<LaidOutRuby> {
    let base_bounds = union_bounds(
        glyphs
            .iter()
            .filter(|glyph| ranges_overlap(glyph.range, annotation.base_range))
            .map(|glyph| glyph.bounds),
    );
    let Some(base_bounds) = base_bounds else {
        return Vec::new();
    };
    let vertical = glyphs
        .iter()
        .find(|glyph| ranges_overlap(glyph.range, annotation.base_range))
        .is_some_and(|glyph| !matches!(glyph.writing_mode, RichTextWritingMode::HorizontalTb));
    let writing_mode = glyphs
        .iter()
        .find(|glyph| ranges_overlap(glyph.range, annotation.base_range))
        .map_or(RichTextWritingMode::HorizontalTb, |glyph| {
            glyph.writing_mode
        });
    let metrics = ruby_metrics(annotation, config);
    let ruby_extent = ruby_text_extent(&annotation.ruby, metrics.font_size);
    let base_bounds = if vertical {
        if matches!(
            ruby_position(annotation),
            RichTextRubyPosition::InterCharacter
        ) {
            base_bounds
        } else {
            expand_vertical_ruby_base(base_bounds, ruby_extent, config)
        }
    } else {
        expand_horizontal_ruby_base(base_bounds, ruby_extent, config)
    };
    if vertical
        && matches!(
            ruby_position(annotation),
            RichTextRubyPosition::InterCharacter
        )
    {
        layout_vertical_inter_character_ruby(ruby_index, annotation, base_bounds, glyphs, metrics)
    } else if vertical && ruby_extent > config.size.height {
        layout_overheight_vertical_ruby(
            ruby_index,
            annotation,
            base_bounds,
            writing_mode,
            config,
            metrics,
        )
    } else if vertical {
        vec![laid_out_ruby_segment(
            ruby_index,
            annotation,
            annotation.ruby.clone(),
            base_bounds,
            LayoutRect::new(
                vertical_ruby_track_x(base_bounds, writing_mode, annotation, config, metrics),
                ruby_annotation_start(
                    base_bounds.y,
                    base_bounds.height,
                    ruby_extent,
                    ruby_overhang_limit(metrics),
                ),
                metrics.font_size,
                ruby_extent,
            ),
            writing_mode,
        )]
    } else {
        vec![laid_out_ruby_segment(
            ruby_index,
            annotation,
            annotation.ruby.clone(),
            base_bounds,
            LayoutRect::new(
                ruby_annotation_start(
                    base_bounds.x,
                    base_bounds.width,
                    ruby_extent,
                    ruby_overhang_limit(metrics),
                ),
                horizontal_ruby_track_y(base_bounds, annotation, metrics),
                ruby_extent,
                metrics.font_size,
            ),
            writing_mode,
        )]
    }
}

fn layout_vertical_inter_character_ruby(
    ruby_index: usize,
    annotation: &RichTextRubyAnnotation,
    base_bounds: LayoutRect,
    glyphs: &[LaidOutGlyph],
    metrics: RubyMetrics,
) -> Vec<LaidOutRuby> {
    let Some(first_base) = glyphs
        .iter()
        .filter(|glyph| ranges_overlap(glyph.range, annotation.base_range))
        .min_by(|left, right| {
            left.origin
                .y
                .partial_cmp(&right.origin.y)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    else {
        return Vec::new();
    };
    let ruby_extent = ruby_text_extent(&annotation.ruby, metrics.font_size);
    let ruby_bounds = LayoutRect::new(
        base_bounds.x,
        first_base.bounds.bottom(),
        base_bounds.width,
        ruby_extent,
    );
    vec![laid_out_ruby_segment(
        ruby_index,
        annotation,
        annotation.ruby.clone(),
        base_bounds,
        ruby_bounds,
        first_base.writing_mode,
    )]
}

fn layout_overheight_vertical_ruby(
    ruby_index: usize,
    annotation: &RichTextRubyAnnotation,
    base_bounds: LayoutRect,
    writing_mode: RichTextWritingMode,
    config: TextLayoutConfig,
    metrics: RubyMetrics,
) -> Vec<LaidOutRuby> {
    let max_chars_per_segment = max_ruby_chars_per_vertical_segment(config, metrics);
    let track_x = vertical_ruby_track_x(base_bounds, writing_mode, annotation, config, metrics);
    split_ruby_text(&annotation.ruby, max_chars_per_segment)
        .into_iter()
        .enumerate()
        .map(|(segment_index, ruby)| {
            let ruby_extent = ruby_text_extent(&ruby, metrics.font_size);
            laid_out_ruby_segment(
                ruby_index,
                annotation,
                ruby,
                base_bounds,
                LayoutRect::new(
                    track_x
                        + vertical_ruby_continuation_step(writing_mode, metrics)
                            * usize_to_f32(segment_index),
                    config.origin.y,
                    metrics.font_size,
                    ruby_extent,
                ),
                writing_mode,
            )
        })
        .collect()
}

fn laid_out_ruby_segment(
    ruby_index: usize,
    annotation: &RichTextRubyAnnotation,
    ruby: String,
    base_bounds: LayoutRect,
    ruby_bounds: LayoutRect,
    writing_mode: RichTextWritingMode,
) -> LaidOutRuby {
    LaidOutRuby {
        ruby_index,
        base_range: annotation.base_range,
        ruby,
        base_bounds,
        ruby_bounds,
        writing_mode,
        presentation: annotation.presentation.clone(),
    }
}

fn split_ruby_text(ruby: &str, max_chars_per_segment: usize) -> Vec<String> {
    let mut segments = Vec::new();
    let mut segment = String::new();
    for ch in ruby.chars() {
        if segment.chars().count() >= max_chars_per_segment {
            segments.push(segment);
            segment = String::new();
        }
        segment.push(ch);
    }
    if !segment.is_empty() {
        segments.push(segment);
    }
    segments
}

pub(super) fn ruby_text_extent(ruby: &str, ruby_font_size: f32) -> f32 {
    usize_to_f32(ruby.chars().count().max(1)) * ruby_font_size
}

fn ruby_overhang_limit(metrics: RubyMetrics) -> f32 {
    metrics.overhang
}

fn ruby_annotation_start(
    base_start: f32,
    base_extent: f32,
    ruby_extent: f32,
    overhang: f32,
) -> f32 {
    let ideal = base_start + (base_extent - ruby_extent) * 0.5;
    let min_start = base_start - overhang;
    let max_start = base_start + base_extent + overhang - ruby_extent;
    if min_start <= max_start {
        ideal.max(min_start).min(max_start)
    } else {
        ideal
    }
}

fn expand_horizontal_ruby_base(
    base_bounds: LayoutRect,
    ruby_width: f32,
    config: TextLayoutConfig,
) -> LayoutRect {
    let width = horizontal_ruby_base_allocation_width(base_bounds.width, ruby_width, config);
    let max_right = config.origin.x + config.size.width;
    let centered_x = base_bounds.x + (base_bounds.width - width) * 0.5;
    let x = centered_x
        .max(config.origin.x)
        .min((max_right - width).max(config.origin.x));
    LayoutRect::new(x, base_bounds.y, width, base_bounds.height)
}

pub(super) fn horizontal_ruby_base_allocation_width(
    base_width: f32,
    ruby_width: f32,
    config: TextLayoutConfig,
) -> f32 {
    ruby_width.max(base_width).min(config.size.width)
}

fn expand_vertical_ruby_base(
    base_bounds: LayoutRect,
    ruby_height: f32,
    config: TextLayoutConfig,
) -> LayoutRect {
    let height = ruby_height.max(base_bounds.height).min(config.size.height);
    let max_bottom = config.origin.y + config.size.height;
    let centered_y = base_bounds.y + (base_bounds.height - height) * 0.5;
    let y = centered_y
        .max(config.origin.y)
        .min((max_bottom - height).max(config.origin.y));
    LayoutRect::new(base_bounds.x, y, base_bounds.width, height)
}

fn vertical_ruby_track_x(
    base_bounds: LayoutRect,
    writing_mode: RichTextWritingMode,
    annotation: &RichTextRubyAnnotation,
    config: TextLayoutConfig,
    metrics: RubyMetrics,
) -> f32 {
    let natural_overlap = vertical_ruby_html_overlap(metrics);
    let x = match (writing_mode, ruby_position(annotation)) {
        (RichTextWritingMode::VerticalRl, RichTextRubyPosition::Under)
        | (
            RichTextWritingMode::VerticalLr,
            RichTextRubyPosition::Auto
            | RichTextRubyPosition::Over
            | RichTextRubyPosition::InterCharacter,
        ) => base_bounds.x - metrics.font_size - metrics.gap + natural_overlap,
        (RichTextWritingMode::VerticalRl | RichTextWritingMode::HorizontalTb, _)
        | (RichTextWritingMode::VerticalLr, RichTextRubyPosition::Under) => {
            base_bounds.right() + metrics.gap - natural_overlap
        }
    };
    x.max(config.origin.x)
        .min((config.origin.x + config.size.width - metrics.font_size).max(config.origin.x))
}

fn horizontal_ruby_track_y(
    base_bounds: LayoutRect,
    annotation: &RichTextRubyAnnotation,
    metrics: RubyMetrics,
) -> f32 {
    let natural_overlap = horizontal_ruby_html_overlap(metrics);
    match ruby_position(annotation) {
        RichTextRubyPosition::Under => base_bounds.bottom() + metrics.gap - natural_overlap,
        _ => (base_bounds.y - metrics.font_size - metrics.gap + natural_overlap).max(0.0),
    }
}

pub(super) fn horizontal_ruby_html_overlap(metrics: RubyMetrics) -> f32 {
    metrics.font_size * HORIZONTAL_RUBY_HTML_OVERLAP_EM
}

pub(super) fn vertical_ruby_html_overlap(metrics: RubyMetrics) -> f32 {
    metrics.font_size * VERTICAL_RUBY_HTML_OVERLAP_EM
}

fn vertical_ruby_continuation_step(writing_mode: RichTextWritingMode, metrics: RubyMetrics) -> f32 {
    match writing_mode {
        RichTextWritingMode::VerticalRl => vertical_ruby_continuation_track_step(metrics),
        RichTextWritingMode::VerticalLr | RichTextWritingMode::HorizontalTb => {
            -vertical_ruby_continuation_track_step(metrics)
        }
    }
}

fn resolve_ruby_collisions(ruby: &mut [LaidOutRuby], config: TextLayoutConfig) {
    let mut placed = Vec::new();
    for annotation in ruby {
        if matches!(
            annotation
                .presentation
                .layout
                .as_ref()
                .map_or(RichTextRubyPosition::Auto, |layout| layout.ruby_position),
            RichTextRubyPosition::InterCharacter
        ) {
            continue;
        }
        let metrics = ruby_metrics_from_presentation(&annotation.presentation, config);
        let resolved = match annotation.writing_mode {
            RichTextWritingMode::HorizontalTb => {
                resolve_horizontal_ruby_collision(annotation.ruby_bounds, &placed, config, metrics)
            }
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
                resolve_vertical_ruby_collision(
                    annotation.ruby_bounds,
                    annotation.writing_mode,
                    &placed,
                    config,
                    metrics,
                )
            }
        };
        annotation.ruby_bounds = resolved;
        placed.push(RubyTrackPlacement {
            writing_mode: annotation.writing_mode,
            bounds: resolved,
        });
    }
}

#[derive(Clone, Copy, Debug)]
struct RubyTrackPlacement {
    writing_mode: RichTextWritingMode,
    bounds: LayoutRect,
}

fn resolve_horizontal_ruby_collision(
    mut bounds: LayoutRect,
    placed: &[RubyTrackPlacement],
    config: TextLayoutConfig,
    metrics: RubyMetrics,
) -> LayoutRect {
    for previous in placed
        .iter()
        .filter(|placement| matches!(placement.writing_mode, RichTextWritingMode::HorizontalTb))
    {
        if bounds.intersects(previous.bounds) {
            bounds.x = previous.bounds.right() + metrics.collision_gap;
        }
    }
    let overhang = ruby_overhang_limit(metrics);
    let min_left = config.origin.x - overhang;
    let max_right = config.origin.x + config.size.width + overhang;
    if bounds.x < min_left {
        bounds.x = min_left;
    }
    if bounds.right() > max_right {
        bounds.x = (max_right - bounds.width).max(min_left);
        bounds.y = (bounds.y - metrics.font_size - metrics.collision_gap).max(0.0);
    }
    bounds
}

fn resolve_vertical_ruby_collision(
    mut bounds: LayoutRect,
    writing_mode: RichTextWritingMode,
    placed: &[RubyTrackPlacement],
    config: TextLayoutConfig,
    metrics: RubyMetrics,
) -> LayoutRect {
    for previous in placed.iter().filter(|placement| {
        matches!(
            placement.writing_mode,
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr
        )
    }) {
        if bounds.intersects(previous.bounds) {
            bounds.y = previous.bounds.bottom() + metrics.collision_gap;
        }
    }
    let overhang = ruby_overhang_limit(metrics);
    let min_top = config.origin.y - overhang;
    let max_bottom = config.origin.y + config.size.height + overhang;
    if bounds.y < min_top {
        bounds.y = min_top;
    }
    if bounds.bottom() > max_bottom {
        bounds.y = min_top;
        bounds.x += match writing_mode {
            RichTextWritingMode::VerticalRl => vertical_ruby_continuation_track_step(metrics),
            RichTextWritingMode::VerticalLr | RichTextWritingMode::HorizontalTb => {
                -vertical_ruby_continuation_track_step(metrics)
            }
        };
    }
    bounds
}
