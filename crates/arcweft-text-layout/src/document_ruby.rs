//! Ruby shaping, placement, and collision resolution for canonical layout.

use std::collections::BTreeMap;

use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRuby, RichTextInlineDirection, RichTextRange,
    RichTextRubyPosition, RichTextWritingMode,
};

use crate::{
    GlyphOrientation, LayoutPoint, LayoutRect, LayoutSize, ShapedTextRun, TextLayoutError,
    TextLayoutGlyph, TextLayoutRequest, TextLayoutRuby, TextLayoutRubyGlyph, TextLayoutRun,
    TextShapeRequest, TextShaper,
    document_layout::{
        milli_to_pixels, ranges_overlap, saturating_u32, translate_rect, union_rects,
    },
};

const DEFAULT_RUBY_GAP: f32 = 2.0;
const DEFAULT_COLLISION_GAP: f32 = 2.0;
const HORIZONTAL_OVERLAP_EM: f32 = 0.36;
const VERTICAL_OVERLAP_EM: f32 = 0.46;

#[derive(Clone, Copy, Debug)]
struct RubyMetrics {
    font_size: f32,
    gap: f32,
    overhang: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct RubyBodyInsets {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

#[derive(Clone, Copy)]
struct LocalRubyGlyph {
    key: crate::ShapedGlyphKey,
    text_range: RichTextRange,
    cluster_index: u32,
    origin: LayoutPoint,
    advance: f32,
    ink_bounds: LayoutRect,
}

pub(crate) fn body_request<E: std::error::Error + 'static>(
    document: &ResolvedTextDocument<'_>,
    request: TextLayoutRequest,
) -> Result<TextLayoutRequest, TextLayoutError<E>> {
    let mut insets = RubyBodyInsets::default();
    for annotation in document.ruby() {
        let writing_mode = document
            .runs()
            .iter()
            .find(|run| ranges_overlap(run.source_range(), annotation.source_base_range()))
            .map_or(annotation.style().writing_mode(), |run| {
                run.style().writing_mode()
            });
        let metrics = ruby_metrics(annotation);
        let overlap = match writing_mode {
            RichTextWritingMode::HorizontalTb => metrics.font_size * HORIZONTAL_OVERLAP_EM,
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
                metrics.font_size * VERTICAL_OVERLAP_EM
            }
        };
        let reserve = (metrics.font_size + metrics.gap - overlap).max(0.0);
        match writing_mode {
            RichTextWritingMode::HorizontalTb => {
                if matches!(ruby_position(annotation), RichTextRubyPosition::Under) {
                    insets.bottom = insets.bottom.max(reserve);
                } else {
                    insets.top = insets.top.max(reserve);
                }
            }
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr
                if !matches!(
                    ruby_position(annotation),
                    RichTextRubyPosition::InterCharacter
                ) =>
            {
                if ruby_track_is_right(writing_mode, ruby_position(annotation)) {
                    insets.right = insets.right.max(reserve);
                } else {
                    insets.left = insets.left.max(reserve);
                }
            }
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {}
        }
    }
    let width = request.size.width - insets.left - insets.right;
    let height = request.size.height - insets.top - insets.bottom;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(TextLayoutError::InsufficientRubyLayoutSpace);
    }
    Ok(TextLayoutRequest {
        origin: LayoutPoint::new(
            request.origin.x + insets.left,
            request.origin.y + insets.top,
        ),
        size: LayoutSize::new(width, height),
        ..request
    })
}

pub(crate) fn layout_ruby<S: TextShaper>(
    document: &ResolvedTextDocument<'_>,
    request: TextLayoutRequest,
    shaper: &mut S,
    body_glyphs: &[TextLayoutGlyph],
    runs: &[TextLayoutRun],
) -> Result<Vec<TextLayoutRuby>, TextLayoutError<S::Error>> {
    document
        .ruby()
        .iter()
        .enumerate()
        .filter_map(|(ruby_index, annotation)| {
            let base_glyphs = body_glyphs
                .iter()
                .filter(|glyph| ranges_overlap(glyph.source_range, annotation.source_base_range()))
                .collect::<Vec<_>>();
            let base_bounds = union_rects(
                base_glyphs
                    .iter()
                    .map(|glyph| glyph.layout_bounds.union(glyph.ink_bounds)),
            )?;
            let inter_character_y = base_glyphs
                .iter()
                .min_by_key(|glyph| (glyph.source_range.start, glyph.cluster_index))
                .map(|first| first.cluster_index)
                .and_then(|first_cluster| {
                    union_rects(
                        base_glyphs
                            .iter()
                            .filter(|glyph| glyph.cluster_index == first_cluster)
                            .map(|glyph| glyph.layout_bounds.union(glyph.ink_bounds)),
                    )
                })
                .map(LayoutRect::bottom);
            Some(layout_one_ruby(
                ruby_index,
                annotation,
                base_bounds,
                inter_character_y,
                request,
                shaper,
                runs,
            ))
        })
        .collect()
}

fn layout_one_ruby<S: TextShaper>(
    ruby_index: usize,
    annotation: &ResolvedTextRuby,
    base_bounds: LayoutRect,
    inter_character_y: Option<f32>,
    request: TextLayoutRequest,
    shaper: &mut S,
    runs: &[TextLayoutRun],
) -> Result<TextLayoutRuby, TextLayoutError<S::Error>> {
    let writing_mode = runs
        .iter()
        .find(|run| ranges_overlap(run.source_range, annotation.source_base_range()))
        .map_or(annotation.style().writing_mode(), |run| run.writing_mode);
    let metrics = ruby_metrics(annotation);
    let font_size_milli = pixels_to_milli(metrics.font_size)
        .ok_or(TextLayoutError::InvalidRubyStyle { ruby_index })?;
    let style = annotation
        .style()
        .clone()
        .with_font_metrics(font_size_milli, font_size_milli)
        .map_err(|_| TextLayoutError::InvalidRubyStyle { ruby_index })?;
    let ruby_run = shaper
        .shape_run(TextShapeRequest {
            text: annotation.text(),
            source_range: RichTextRange::new(0, annotation.text().len()),
            style: &style,
            locale: style.language(),
            direction: RichTextInlineDirection::Auto,
            writing_mode,
        })
        .map_err(|source| TextLayoutError::ShapeRuby { ruby_index, source })?;
    validate_ruby_shape(ruby_index, annotation.text(), &ruby_run)?;
    let glyphs = match writing_mode {
        RichTextWritingMode::HorizontalTb => {
            place_horizontal(annotation, base_bounds, request, metrics, &ruby_run)
        }
        RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => place_vertical(
            annotation,
            base_bounds,
            request,
            metrics,
            writing_mode,
            inter_character_y,
            &ruby_run,
        ),
    };
    let ruby_bounds = union_rects(
        glyphs
            .iter()
            .map(|glyph| glyph.layout_bounds.union(glyph.ink_bounds)),
    )
    .unwrap_or(base_bounds);
    Ok(TextLayoutRuby {
        ruby_index: saturating_u32(ruby_index),
        base_range: annotation.source_base_range(),
        text: annotation.text().to_owned(),
        base_bounds,
        ruby_bounds,
        glyphs,
        writing_mode,
        style,
        presentation: annotation.presentation().clone(),
    })
}

fn place_horizontal(
    annotation: &ResolvedTextRuby,
    base_bounds: LayoutRect,
    request: TextLayoutRequest,
    metrics: RubyMetrics,
    shaped: &ShapedTextRun,
) -> Vec<TextLayoutRubyGlyph> {
    let local = local_glyphs(shaped);
    let local_bounds = union_rects(local.iter().map(|glyph| glyph.ink_bounds)).unwrap_or(
        LayoutRect::new(0.0, 0.0, shaped.advance().width, metrics.font_size),
    );
    let width = shaped.advance().width.max(local_bounds.width);
    let ideal_x = base_bounds.x + (base_bounds.width - width) * 0.5;
    let min_x = base_bounds.x - metrics.overhang;
    let max_x = base_bounds.right() + metrics.overhang - width;
    let x = if min_x <= max_x {
        ideal_x.max(min_x).min(max_x)
    } else {
        ideal_x
    }
    .max(request.origin.x)
    .min((request.origin.x + request.size.width - width).max(request.origin.x));
    let overlap = metrics.font_size * HORIZONTAL_OVERLAP_EM;
    let y = match ruby_position(annotation) {
        RichTextRubyPosition::Under => base_bounds.bottom() + metrics.gap - overlap,
        RichTextRubyPosition::Auto
        | RichTextRubyPosition::Over
        | RichTextRubyPosition::InterCharacter => {
            base_bounds.y - metrics.font_size - metrics.gap + overlap
        }
    };
    local
        .into_iter()
        .map(|glyph| {
            let origin = LayoutPoint::new(x + glyph.origin.x, y + glyph.origin.y);
            let ink_bounds = translate_rect(glyph.ink_bounds, x, y);
            TextLayoutRubyGlyph {
                text_range: glyph.text_range,
                cluster_index: glyph.cluster_index,
                origin,
                advance: LayoutSize::new(glyph.advance, 0.0),
                layout_bounds: LayoutRect::new(
                    x + glyph.origin.x,
                    y,
                    glyph.advance,
                    metrics.font_size,
                )
                .union(ink_bounds),
                ink_bounds,
                orientation: GlyphOrientation::Upright,
                inline_scale: 1.0,
                shape_key: glyph.key,
            }
        })
        .collect()
}

fn place_vertical(
    annotation: &ResolvedTextRuby,
    base_bounds: LayoutRect,
    request: TextLayoutRequest,
    metrics: RubyMetrics,
    writing_mode: RichTextWritingMode,
    inter_character_y: Option<f32>,
    shaped: &ShapedTextRun,
) -> Vec<TextLayoutRubyGlyph> {
    let local = local_glyphs(shaped);
    let clusters = cluster_slices(&local).collect::<Vec<_>>();
    let extent = usize_to_f32(clusters.len().max(1)) * metrics.font_size;
    let position = ruby_position(annotation);
    let ideal_y = if matches!(position, RichTextRubyPosition::InterCharacter) {
        inter_character_y.unwrap_or(base_bounds.y + (base_bounds.height - extent) * 0.5)
    } else {
        base_bounds.y + (base_bounds.height - extent) * 0.5
    };
    let y = ideal_y
        .max(base_bounds.y - metrics.overhang)
        .min(base_bounds.bottom() + metrics.overhang - extent)
        .max(request.origin.y)
        .min((request.origin.y + request.size.height - extent).max(request.origin.y));
    let overlap = metrics.font_size * VERTICAL_OVERLAP_EM;
    let side_right = ruby_track_is_right(writing_mode, position);
    let x = if matches!(position, RichTextRubyPosition::InterCharacter) {
        base_bounds.x + (base_bounds.width - metrics.font_size) * 0.5
    } else if side_right {
        base_bounds.right() + metrics.gap - overlap
    } else {
        base_bounds.x - metrics.font_size - metrics.gap + overlap
    }
    .max(request.origin.x)
    .min((request.origin.x + request.size.width - metrics.font_size).max(request.origin.x));

    let mut out = Vec::new();
    for (cluster_index, cluster) in clusters.into_iter().enumerate() {
        let cell = LayoutRect::new(
            x,
            y + usize_to_f32(cluster_index) * metrics.font_size,
            metrics.font_size,
            metrics.font_size,
        );
        let cluster_bounds = union_rects(cluster.iter().map(|glyph| glyph.ink_bounds)).unwrap_or(
            LayoutRect::new(0.0, 0.0, metrics.font_size, metrics.font_size),
        );
        let translation = LayoutPoint::new(
            cell.x + (cell.width - cluster_bounds.width) * 0.5 - cluster_bounds.x,
            cell.y + (cell.height - cluster_bounds.height) * 0.5 - cluster_bounds.y,
        );
        for (glyph_index, glyph) in cluster.iter().enumerate() {
            let origin = LayoutPoint::new(
                glyph.origin.x + translation.x,
                glyph.origin.y + translation.y,
            );
            let ink_bounds = translate_rect(glyph.ink_bounds, translation.x, translation.y);
            out.push(TextLayoutRubyGlyph {
                text_range: glyph.text_range,
                cluster_index: saturating_u32(cluster_index),
                origin,
                advance: if glyph_index + 1 == cluster.len() {
                    LayoutSize::new(0.0, metrics.font_size)
                } else {
                    LayoutSize::default()
                },
                layout_bounds: cell,
                ink_bounds,
                orientation: GlyphOrientation::Upright,
                inline_scale: 1.0,
                shape_key: glyph.key,
            });
        }
    }
    out
}

pub(crate) fn resolve_collisions(ruby: &mut [TextLayoutRuby]) {
    let mut placed = Vec::<LayoutRect>::new();
    for annotation in ruby {
        if matches!(
            ruby_position_from_presentation(annotation),
            RichTextRubyPosition::InterCharacter
        ) {
            continue;
        }
        let gap = collision_gap(annotation);
        while let Some(collision) = placed
            .iter()
            .copied()
            .find(|bounds| bounds.intersects(annotation.ruby_bounds))
        {
            let (dx, dy) = collision_shift(annotation, collision, gap);
            shift_annotation(annotation, dx, dy);
        }
        placed.push(annotation.ruby_bounds);
    }
}

pub(crate) fn inter_character_extent_after(
    document: &ResolvedTextDocument<'_>,
    source_range: RichTextRange,
) -> f32 {
    document
        .ruby()
        .iter()
        .filter(|annotation| annotation.source_base_range().start == source_range.start)
        .filter(|annotation| ranges_overlap(annotation.source_base_range(), source_range))
        .filter(|annotation| {
            matches!(
                ruby_position(annotation),
                RichTextRubyPosition::InterCharacter
            )
        })
        .map(|annotation| {
            usize_to_f32(annotation.text().chars().count().max(1))
                * ruby_metrics(annotation).font_size
        })
        .sum()
}

fn collision_shift(annotation: &TextLayoutRuby, collision: LayoutRect, gap: f32) -> (f32, f32) {
    match annotation.writing_mode {
        RichTextWritingMode::HorizontalTb => {
            if matches!(
                ruby_position_from_presentation(annotation),
                RichTextRubyPosition::Under
            ) {
                (0.0, collision.bottom() + gap - annotation.ruby_bounds.y)
            } else {
                (0.0, collision.y - gap - annotation.ruby_bounds.bottom())
            }
        }
        RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
            if ruby_track_is_right(
                annotation.writing_mode,
                ruby_position_from_presentation(annotation),
            ) {
                (collision.right() + gap - annotation.ruby_bounds.x, 0.0)
            } else {
                (collision.x - gap - annotation.ruby_bounds.right(), 0.0)
            }
        }
    }
}

fn shift_annotation(annotation: &mut TextLayoutRuby, dx: f32, dy: f32) {
    annotation.ruby_bounds.x += dx;
    annotation.ruby_bounds.y += dy;
    for glyph in &mut annotation.glyphs {
        glyph.origin.x += dx;
        glyph.origin.y += dy;
        glyph.layout_bounds.x += dx;
        glyph.layout_bounds.y += dy;
        glyph.ink_bounds.x += dx;
        glyph.ink_bounds.y += dy;
    }
}

fn local_glyphs(shaped: &ShapedTextRun) -> Vec<LocalRubyGlyph> {
    let mut cursors = BTreeMap::<u32, f32>::new();
    shaped
        .glyphs()
        .iter()
        .map(|glyph| {
            let cursor = cursors.entry(glyph.line_index).or_default();
            let origin = LayoutPoint::new(*cursor + glyph.offset.x, glyph.offset.y);
            *cursor += glyph.advance.width;
            LocalRubyGlyph {
                key: glyph.key,
                text_range: glyph.source_range,
                cluster_index: glyph.cluster_index,
                origin,
                advance: glyph.advance.width,
                ink_bounds: translate_rect(glyph.ink_bounds, origin.x, origin.y),
            }
        })
        .collect()
}

fn cluster_slices(glyphs: &[LocalRubyGlyph]) -> impl Iterator<Item = &[LocalRubyGlyph]> {
    let mut start = 0;
    std::iter::from_fn(move || {
        let first = glyphs.get(start)?;
        let end = glyphs[start + 1..]
            .iter()
            .position(|glyph| glyph.cluster_index != first.cluster_index)
            .map_or(glyphs.len(), |offset| start + 1 + offset);
        let cluster = &glyphs[start..end];
        start = end;
        Some(cluster)
    })
}

fn validate_ruby_shape<E: std::error::Error + 'static>(
    ruby_index: usize,
    text: &str,
    shaped: &ShapedTextRun,
) -> Result<(), TextLayoutError<E>> {
    for (glyph_index, glyph) in shaped.glyphs().iter().enumerate() {
        let range = glyph.source_range;
        if range.start >= range.end
            || range.end > text.len()
            || !text.is_char_boundary(range.start)
            || !text.is_char_boundary(range.end)
        {
            return Err(TextLayoutError::InvalidRubyShapedRange {
                ruby_index,
                glyph_index,
                range,
            });
        }
        let values = [
            glyph.offset.x,
            glyph.offset.y,
            glyph.advance.width,
            glyph.advance.height,
            glyph.ink_bounds.x,
            glyph.ink_bounds.y,
            glyph.ink_bounds.width,
            glyph.ink_bounds.height,
        ];
        if values.iter().any(|value| !value.is_finite())
            || glyph.advance.width < 0.0
            || glyph.advance.height < 0.0
            || glyph.ink_bounds.width < 0.0
            || glyph.ink_bounds.height < 0.0
        {
            return Err(TextLayoutError::InvalidRubyShapedGeometry {
                ruby_index,
                glyph_index,
            });
        }
    }
    Ok(())
}

fn ruby_metrics(annotation: &ResolvedTextRuby) -> RubyMetrics {
    let default_size = milli_to_pixels(annotation.style().font_size_milli()) * 0.5;
    let layout = annotation.presentation().layout.as_ref();
    let font_size = layout
        .and_then(|layout| positive_milli(layout.ruby_font_size))
        .unwrap_or(default_size);
    RubyMetrics {
        font_size,
        gap: layout
            .and_then(|layout| nonnegative_milli(layout.ruby_gap))
            .unwrap_or(DEFAULT_RUBY_GAP),
        overhang: layout
            .and_then(|layout| positive_milli(layout.ruby_overhang))
            .unwrap_or(font_size * 0.5),
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

fn ruby_position(annotation: &ResolvedTextRuby) -> RichTextRubyPosition {
    annotation
        .presentation()
        .layout
        .as_ref()
        .map_or(RichTextRubyPosition::Auto, |layout| layout.ruby_position)
}

fn ruby_position_from_presentation(annotation: &TextLayoutRuby) -> RichTextRubyPosition {
    annotation
        .presentation
        .layout
        .as_ref()
        .map_or(RichTextRubyPosition::Auto, |layout| layout.ruby_position)
}

fn ruby_track_is_right(writing_mode: RichTextWritingMode, position: RichTextRubyPosition) -> bool {
    match writing_mode {
        RichTextWritingMode::VerticalRl => !matches!(position, RichTextRubyPosition::Under),
        RichTextWritingMode::VerticalLr => matches!(position, RichTextRubyPosition::Under),
        RichTextWritingMode::HorizontalTb => false,
    }
}

fn collision_gap(annotation: &TextLayoutRuby) -> f32 {
    annotation
        .presentation
        .layout
        .as_ref()
        .and_then(|layout| positive_milli(layout.ruby_collision_gap))
        .unwrap_or(DEFAULT_COLLISION_GAP)
}

fn pixels_to_milli(value: f32) -> Option<u32> {
    if !value.is_finite() || value <= 0.0 || value > 65_535.0 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((value * 1_000.0).round() as u32)
}

fn usize_to_f32(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}
