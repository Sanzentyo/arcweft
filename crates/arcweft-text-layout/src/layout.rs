//! Frame orchestration and persistent writing-mode cursors.

use crate::{
    JlreqStrictness, LaidOutRun, LaidOutText, TextLayoutConfig, TextLayoutError,
    geometry::{ranges_overlap, union_bounds, usize_to_f32},
    horizontal::{HorizontalRunLayoutContext, layout_horizontal_run},
    ruby::layout_ruby,
    ruby_metrics::{
        RubyMetrics, max_ruby_chars_per_vertical_segment, ruby_metrics, ruby_position,
        vertical_ruby_continuation_track_step,
    },
    vertical::{RunLayoutContext, layout_vertical_run, vertical_column_start},
};
use arcweft_render_text::{
    LineDisplayFrame, RichTextJlreqStrictness, RichTextPresentation, RichTextRange,
    RichTextRubyAnnotation, RichTextRubyPosition, RichTextVerticalLatinMode, RichTextWritingMode,
};
use std::ops::Range;

/// Lays out one resolved rich-text frame into renderer-independent geometry.
pub fn layout_frame(
    frame: &LineDisplayFrame,
    config: TextLayoutConfig,
) -> Result<LaidOutText, TextLayoutError> {
    let mut out = LaidOutText::default();
    let mut state = TextLayoutState::new(config, vertical_ruby_track_reservation(frame, config));
    for (run_index, run) in frame.display_map.text_runs.iter().enumerate() {
        let range = valid_range(frame, run.range)?;
        let text = frame
            .text
            .get(range.clone())
            .ok_or(TextLayoutError::InvalidRange { range: run.range })?;
        let writing_mode = run
            .presentation
            .layout
            .as_ref()
            .map_or(config.writing_mode, |layout| layout.writing_mode);
        let vertical_latin = run
            .presentation
            .layout
            .as_ref()
            .map_or(RichTextVerticalLatinMode::Mixed, |layout| {
                layout.vertical_latin
            });
        let run_config = text_layout_config_for_presentation(config, &run.presentation);
        let glyph_start = out.glyphs.len();
        match writing_mode {
            RichTextWritingMode::HorizontalTb => {
                let context = HorizontalRunLayoutContext {
                    run_index,
                    range_start: range.start,
                    presentation: &run.presentation,
                    ruby_annotations: &frame.display_map.ruby_annotations,
                    config: run_config,
                };
                layout_horizontal_run(&mut out.glyphs, text, context, &mut state);
            }
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
                let context = RunLayoutContext {
                    run_index,
                    range_start: range.start,
                    source: run.source,
                    presentation: &run.presentation,
                    ruby_annotations: &frame.display_map.ruby_annotations,
                    config: run_config,
                };
                layout_vertical_run(
                    &mut out.glyphs,
                    text,
                    writing_mode,
                    vertical_latin,
                    context,
                    &mut state,
                );
            }
        }
        if let Some(bounds) =
            union_bounds(out.glyphs[glyph_start..].iter().map(|glyph| glyph.bounds))
        {
            out.runs.push(LaidOutRun {
                run_index,
                range: run.range,
                bounds,
                writing_mode,
                presentation: run.presentation.clone(),
            });
            out.bounds = Some(out.bounds.map_or(bounds, |existing| existing.union(bounds)));
        }
    }
    out.ruby = layout_ruby(frame, &out.glyphs, config);
    for ruby in &out.ruby {
        out.bounds = Some(
            out.bounds
                .map_or(ruby.ruby_bounds, |bounds| bounds.union(ruby.ruby_bounds)),
        );
    }
    Ok(out)
}

pub(crate) fn text_layout_config_for_presentation(
    config: TextLayoutConfig,
    presentation: &RichTextPresentation,
) -> TextLayoutConfig {
    let Some(layout) = &presentation.layout else {
        return config;
    };
    let jlreq_strictness = match layout.jlreq_strictness {
        RichTextJlreqStrictness::Auto => config.jlreq_strictness,
        RichTextJlreqStrictness::Loose => JlreqStrictness::Loose,
        RichTextJlreqStrictness::Normal => JlreqStrictness::Normal,
        RichTextJlreqStrictness::Strict => JlreqStrictness::Strict,
    };
    TextLayoutConfig {
        jlreq_strictness,
        ..config
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TextLayoutState {
    pub(crate) horizontal: LayoutCursor,
    pub(crate) vertical_rl: LayoutCursor,
    pub(crate) vertical_lr: LayoutCursor,
    pub(crate) vertical_rl_previous_cluster: Option<String>,
    pub(crate) vertical_lr_previous_cluster: Option<String>,
}

impl TextLayoutState {
    fn new(config: TextLayoutConfig, ruby_track: VerticalRubyTrackReservation) -> Self {
        Self {
            horizontal: LayoutCursor::new(config.origin.x, config.origin.y),
            vertical_rl: LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalRl, config)
                    - ruby_track.vertical_rl,
                config.origin.y,
            ),
            vertical_lr: LayoutCursor::new(
                vertical_column_start(RichTextWritingMode::VerticalLr, config)
                    + ruby_track.vertical_lr,
                config.origin.y,
            ),
            vertical_rl_previous_cluster: None,
            vertical_lr_previous_cluster: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct VerticalRubyTrackReservation {
    vertical_rl: f32,
    vertical_lr: f32,
}

fn vertical_ruby_track_reservation(
    frame: &LineDisplayFrame,
    config: TextLayoutConfig,
) -> VerticalRubyTrackReservation {
    frame.display_map.ruby_annotations.iter().fold(
        VerticalRubyTrackReservation::default(),
        |mut reservation, annotation| {
            if matches!(
                ruby_position(annotation),
                RichTextRubyPosition::InterCharacter
            ) {
                return reservation;
            }
            let track_width = vertical_ruby_track_reservation_width(annotation, config);
            match ruby_annotation_writing_mode(frame, annotation, config) {
                RichTextWritingMode::VerticalRl => {
                    reservation.vertical_rl = reservation.vertical_rl.max(track_width);
                }
                RichTextWritingMode::VerticalLr => {
                    reservation.vertical_lr = reservation.vertical_lr.max(track_width);
                }
                RichTextWritingMode::HorizontalTb => {}
            }
            reservation
        },
    )
}

fn ruby_annotation_writing_mode(
    frame: &LineDisplayFrame,
    annotation: &RichTextRubyAnnotation,
    config: TextLayoutConfig,
) -> RichTextWritingMode {
    frame
        .display_map
        .text_runs
        .iter()
        .find(|run| ranges_overlap(run.range, annotation.base_range))
        .and_then(|run| run.presentation.layout.as_ref())
        .map_or(config.writing_mode, |layout| layout.writing_mode)
}

fn vertical_ruby_track_reservation_width(
    annotation: &RichTextRubyAnnotation,
    config: TextLayoutConfig,
) -> f32 {
    let metrics = ruby_metrics(annotation, config);
    let segment_count =
        vertical_ruby_segment_count(annotation.ruby.chars().count(), config, metrics).max(1);
    metrics.gap
        + metrics.font_size
        + usize_to_f32(segment_count.saturating_sub(1))
            * vertical_ruby_continuation_track_step(metrics)
}

fn vertical_ruby_segment_count(
    char_count: usize,
    config: TextLayoutConfig,
    metrics: RubyMetrics,
) -> usize {
    let max_chars = max_ruby_chars_per_vertical_segment(config, metrics).max(1);
    char_count.max(1).div_ceil(max_chars)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LayoutCursor {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

impl LayoutCursor {
    pub(crate) const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

fn valid_range(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> Result<Range<usize>, TextLayoutError> {
    let range = range.start..range.end;
    if frame.text.get(range.clone()).is_some() {
        Ok(range)
    } else {
        Err(TextLayoutError::InvalidRange {
            range: RichTextRange::new(range.start, range.end),
        })
    }
}
