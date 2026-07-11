//! Horizontal line layout, wrapping, and ruby-base allocation.

use crate::{
    GlyphOrientation, GlyphVerticalForm, LaidOutGlyph, LayoutPoint, LayoutRect, LayoutSize,
    TextLayoutConfig,
    effects::{LayoutEffectReserve, layout_phase_effect_reserve},
    layout::{LayoutCursor, TextLayoutState},
    ruby::{horizontal_ruby_base_allocation_width, ruby_text_extent},
    ruby_metrics::ruby_metrics,
};
use arcweft_render_text::{
    RichTextPresentation, RichTextRange, RichTextRubyAnnotation, RichTextWritingMode,
};

#[derive(Clone, Copy)]
pub(crate) struct HorizontalRunLayoutContext<'a> {
    pub(crate) run_index: usize,
    pub(crate) range_start: usize,
    pub(crate) presentation: &'a RichTextPresentation,
    pub(crate) ruby_annotations: &'a [RichTextRubyAnnotation],
    pub(crate) config: TextLayoutConfig,
}

pub(crate) fn layout_horizontal_run(
    glyphs: &mut Vec<LaidOutGlyph>,
    text: &str,
    context: HorizontalRunLayoutContext<'_>,
    state: &mut TextLayoutState,
) {
    let cursor = &mut state.horizontal;
    let char_indices = text.char_indices().collect::<Vec<_>>();
    let mut char_index = 0usize;
    while let Some((offset, ch)) = char_indices.get(char_index).copied() {
        if ch == '\n' {
            cursor.x = context.config.origin.x;
            cursor.y += context.config.line_advance;
            char_index += 1;
            continue;
        }
        let absolute_start = context.range_start + offset;
        if let Some(annotation) = horizontal_ruby_annotation_starting_at(
            context.ruby_annotations,
            context.range_start,
            text,
            absolute_start,
        ) {
            let base_end = annotation.base_range.end;
            let next_index = layout_horizontal_ruby_base(glyphs, text, annotation, context, cursor);
            char_index = char_indices
                .iter()
                .position(|(candidate_offset, _)| {
                    context.range_start + *candidate_offset >= base_end
                })
                .unwrap_or(next_index);
            continue;
        }
        let reserve = layout_phase_effect_reserve(context.presentation);
        let width = horizontal_advance(ch, context.config.font_size);
        let allocation_width = horizontal_layout_advance(width, reserve);
        if horizontal_cluster_should_wrap(cursor.x, allocation_width, context.config) {
            cursor.x = context.config.origin.x;
            cursor.y += context.config.line_advance;
        }
        let start = context.range_start + offset;
        let end = start + ch.len_utf8();
        let origin_x = cursor.x + reserve.x;
        let bounds = horizontal_glyph_bounds_with_reserve(
            origin_x,
            cursor.y,
            width,
            context.config,
            reserve,
        );
        glyphs.push(LaidOutGlyph {
            run_index: context.run_index,
            range: RichTextRange::new(start, end),
            text: ch.to_string(),
            origin: LayoutPoint::new(origin_x, cursor.y),
            advance: LayoutSize::new(allocation_width, 0.0),
            bounds,
            writing_mode: RichTextWritingMode::HorizontalTb,
            orientation: GlyphOrientation::Upright,
            vertical_form: GlyphVerticalForm::None,
            presentation: context.presentation.clone(),
        });
        cursor.x += allocation_width;
        char_index += 1;
    }
}

fn horizontal_ruby_annotation_starting_at<'a>(
    annotations: &'a [RichTextRubyAnnotation],
    range_start: usize,
    text: &str,
    absolute_start: usize,
) -> Option<&'a RichTextRubyAnnotation> {
    let range_end = range_start + text.len();
    annotations.iter().find(|annotation| {
        annotation.base_range.start == absolute_start
            && annotation.base_range.start < annotation.base_range.end
            && annotation.base_range.end <= range_end
            && text
                .get(
                    (annotation.base_range.start - range_start)
                        ..(annotation.base_range.end - range_start),
                )
                .is_some_and(|base| !base.contains('\n'))
    })
}

fn layout_horizontal_ruby_base(
    glyphs: &mut Vec<LaidOutGlyph>,
    text: &str,
    annotation: &RichTextRubyAnnotation,
    context: HorizontalRunLayoutContext<'_>,
    cursor: &mut LayoutCursor,
) -> usize {
    let base_start = annotation.base_range.start - context.range_start;
    let base_end = annotation.base_range.end - context.range_start;
    let Some(base_text) = text.get(base_start..base_end) else {
        return 0;
    };
    let reserve = layout_phase_effect_reserve(context.presentation);
    let base_width = horizontal_text_layout_advance(base_text, context.config.font_size, reserve);
    let metrics = ruby_metrics(annotation, context.config);
    let allocation_width = horizontal_ruby_base_allocation_width(
        base_width,
        ruby_text_extent(&annotation.ruby, metrics.font_size),
        context.config,
    );
    if horizontal_cluster_should_wrap(cursor.x, allocation_width, context.config) {
        cursor.x = context.config.origin.x;
        cursor.y += context.config.line_advance;
    }
    let mut glyph_x = cursor.x + (allocation_width - base_width).max(0.0) * 0.5;
    for (offset, ch) in base_text.char_indices() {
        let width = horizontal_advance(ch, context.config.font_size);
        let allocation_width = horizontal_layout_advance(width, reserve);
        let start = context.range_start + base_start + offset;
        let end = start + ch.len_utf8();
        let origin_x = glyph_x + reserve.x;
        glyphs.push(LaidOutGlyph {
            run_index: context.run_index,
            range: RichTextRange::new(start, end),
            text: ch.to_string(),
            origin: LayoutPoint::new(origin_x, cursor.y),
            advance: LayoutSize::new(allocation_width, 0.0),
            bounds: horizontal_glyph_bounds_with_reserve(
                origin_x,
                cursor.y,
                width,
                context.config,
                reserve,
            ),
            writing_mode: RichTextWritingMode::HorizontalTb,
            orientation: GlyphOrientation::Upright,
            vertical_form: GlyphVerticalForm::None,
            presentation: context.presentation.clone(),
        });
        glyph_x += allocation_width;
    }
    cursor.x += allocation_width;
    text[..base_end].chars().count()
}

fn horizontal_glyph_bounds(
    x: f32,
    line_y: f32,
    width: f32,
    config: TextLayoutConfig,
) -> LayoutRect {
    let height = config.font_size.max(1.0).min(config.line_advance.max(1.0));
    let y = line_y + (config.line_advance - height).max(0.0) * 0.5;
    LayoutRect::new(x, y, width.max(1.0), height)
}

fn horizontal_glyph_bounds_with_reserve(
    x: f32,
    line_y: f32,
    width: f32,
    config: TextLayoutConfig,
    reserve: LayoutEffectReserve,
) -> LayoutRect {
    let mut bounds = horizontal_glyph_bounds(x, line_y, width, config);
    bounds.x -= reserve.x;
    bounds.y -= reserve.y;
    bounds.width += reserve.x * 2.0;
    bounds.height += reserve.y * 2.0;
    bounds
}

fn horizontal_layout_advance(width: f32, reserve: LayoutEffectReserve) -> f32 {
    width + reserve.x * 2.0
}

pub(crate) fn horizontal_text_layout_advance(
    text: &str,
    font_size: f32,
    reserve: LayoutEffectReserve,
) -> f32 {
    text.chars()
        .filter(|ch| *ch != '\n')
        .map(|ch| horizontal_layout_advance(horizontal_advance(ch, font_size), reserve))
        .sum()
}

fn horizontal_cluster_should_wrap(cursor_x: f32, width: f32, config: TextLayoutConfig) -> bool {
    let line_start = config.origin.x;
    let line_end = config.origin.x + config.size.width.max(1.0);
    cursor_x > line_start + f32::EPSILON && cursor_x + width > line_end + f32::EPSILON
}

pub(crate) fn horizontal_advance(ch: char, font_size: f32) -> f32 {
    if ch.is_ascii_whitespace() {
        font_size * 0.33
    } else if matches!(
        ch,
        'i' | 'j' | 'l' | 'I' | '!' | '|' | '.' | ',' | ':' | ';'
    ) {
        font_size * 0.28
    } else if matches!(ch, 'm' | 'w' | 'M' | 'W') {
        font_size * 0.82
    } else if ch.is_ascii_alphanumeric() {
        font_size * 0.54
    } else if ch.is_ascii_punctuation() {
        font_size * 0.36
    } else {
        font_size
    }
}
