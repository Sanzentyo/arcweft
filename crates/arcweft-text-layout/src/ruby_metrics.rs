//! Shared ruby sizing and placement policy.

use crate::TextLayoutConfig;
use arcweft_render_text::{RichTextPresentation, RichTextRubyAnnotation, RichTextRubyPosition};

pub(crate) const DEFAULT_RUBY_GAP: f32 = 2.0;
pub(crate) const HORIZONTAL_RUBY_HTML_OVERLAP_EM: f32 = 0.36;
pub(crate) const VERTICAL_RUBY_HTML_OVERLAP_EM: f32 = 0.46;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RubyMetrics {
    pub(crate) font_size: f32,
    pub(crate) gap: f32,
    pub(crate) overhang: f32,
    pub(crate) collision_gap: f32,
}

pub(crate) fn ruby_metrics(
    annotation: &RichTextRubyAnnotation,
    config: TextLayoutConfig,
) -> RubyMetrics {
    ruby_metrics_from_presentation(&annotation.presentation, config)
}

pub(crate) fn ruby_metrics_from_presentation(
    presentation: &RichTextPresentation,
    config: TextLayoutConfig,
) -> RubyMetrics {
    let font_size = presentation
        .layout
        .as_ref()
        .and_then(|layout| positive_milli(layout.ruby_font_size))
        .unwrap_or(config.ruby_font_size.max(1.0));
    RubyMetrics {
        font_size,
        gap: presentation
            .layout
            .as_ref()
            .and_then(|layout| nonnegative_milli(layout.ruby_gap))
            .unwrap_or(DEFAULT_RUBY_GAP),
        overhang: presentation
            .layout
            .as_ref()
            .and_then(|layout| positive_milli(layout.ruby_overhang))
            .unwrap_or(font_size * 0.5),
        collision_gap: presentation
            .layout
            .as_ref()
            .and_then(|layout| positive_milli(layout.ruby_collision_gap))
            .unwrap_or(2.0),
    }
}

fn positive_milli(value: Option<arcweft_render_text::Milli>) -> Option<f32> {
    value
        .map(arcweft_render_text::Milli::as_f32)
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn nonnegative_milli(value: Option<arcweft_render_text::Milli>) -> Option<f32> {
    value
        .map(arcweft_render_text::Milli::as_f32)
        .filter(|value| value.is_finite() && *value >= 0.0)
}

pub(crate) fn vertical_ruby_continuation_track_step(metrics: RubyMetrics) -> f32 {
    metrics.font_size + metrics.collision_gap
}

pub(crate) fn max_ruby_chars_per_vertical_segment(
    config: TextLayoutConfig,
    metrics: RubyMetrics,
) -> usize {
    let mut count = 1usize;
    let mut extent = metrics.font_size.max(1.0);
    let max_extent = config.size.height.max(extent);
    while extent + metrics.font_size <= max_extent {
        count += 1;
        extent += metrics.font_size;
    }
    count
}

pub(crate) fn ruby_position(annotation: &RichTextRubyAnnotation) -> RichTextRubyPosition {
    annotation
        .presentation
        .layout
        .as_ref()
        .map_or(RichTextRubyPosition::Auto, |layout| layout.ruby_position)
}
