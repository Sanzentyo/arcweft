//! Ruby shaping, placement, and collision resolution for canonical layout.

use std::collections::BTreeMap;

use arcweft_render_text::{ResolvedTextDocument, ResolvedTextRuby};
use arcweft_text_model::{
    Milli, RichTextInlineDirection, RichTextRange, RichTextRubyPosition, RichTextWritingMode,
};

use crate::{
    GlyphOrientation, LayoutPoint, LayoutRect, LayoutSize, ShapedTextRun, TextLayoutBodyAllocation,
    TextLayoutError, TextLayoutGlyph, TextLayoutRequest, TextLayoutRuby, TextLayoutRubyGlyph,
    TextShapeRequest, TextShaper,
    document_layout::{
        VerticalPlacement, add_canonical_count, milli_to_pixels, ranges_overlap, saturating_u32,
        translate_rect, union_rects,
    },
};

const DEFAULT_RUBY_GAP: f32 = 0.0;
const DEFAULT_COLLISION_GAP: f32 = 0.0;

#[derive(Clone, Copy, Debug)]
struct RubyMetrics {
    font_size: f32,
    gap: f32,
    overhang: f32,
    collision_gap: f32,
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

/// One shaping/measurement phase belongs to the same final layout owner.
/// It is consumed when the immutable body and annotation geometry is issued.
pub(crate) struct RubyMeasurements<'a> {
    annotations: Vec<MeasuredRuby<'a>>,
}

struct MeasuredRuby<'a> {
    index: usize,
    annotation: &'a ResolvedTextRuby,
    writing_mode: RichTextWritingMode,
    authored_position: RichTextRubyPosition,
    position: RichTextRubyPosition,
    style: arcweft_render_text::ResolvedTextStyle,
    metrics: RubyMetrics,
    shaped: ShapedTextRun,
}

impl<'a> RubyMeasurements<'a> {
    pub(crate) fn new<S: TextShaper>(
        document: &'a ResolvedTextDocument<'_>,
        request: TextLayoutRequest,
        shaper: &mut S,
    ) -> Result<Self, TextLayoutError<S::Error>> {
        let annotations = document
            .ruby()
            .iter()
            .enumerate()
            .map(|(index, annotation)| {
                // ResolvedTextDocument admits a nonempty, exact base-run
                // interval. Borrow that owner directly; source-overlap scans
                // and an annotation-style fallback are unnecessary.
                let writing_mode = document.runs()[annotation.base_runs().as_usize_range().start]
                    .style()
                    .writing_mode();
                let metrics = ruby_metrics(annotation);
                let authored_position = ruby_position(annotation);
                let position = effective_ruby_position(writing_mode, authored_position);
                let font_size_milli = pixels_to_milli(metrics.font_size)
                    .ok_or(TextLayoutError::InvalidRubyStyle { ruby_index: index })?;
                let style = annotation
                    .style()
                    .clone()
                    .with_font_metrics(font_size_milli, font_size_milli)
                    .map_err(|_| TextLayoutError::InvalidRubyStyle { ruby_index: index })?;
                let shaped = shaper
                    .shape_run(TextShapeRequest {
                        text: annotation.text(),
                        source_range: RichTextRange::new(0, annotation.text().len()),
                        style: &style,
                        locale: style.language(),
                        direction: RichTextInlineDirection::Auto,
                        writing_mode: if matches!(
                            (writing_mode, authored_position),
                            (
                                RichTextWritingMode::HorizontalTb,
                                RichTextRubyPosition::InterCharacter
                            )
                        ) {
                            RichTextWritingMode::VerticalRl
                        } else {
                            writing_mode
                        },
                    })
                    .map_err(|source| TextLayoutError::ShapeRuby {
                        ruby_index: index,
                        source,
                    })?;
                validate_ruby_shape(index, annotation.text(), &shaped)?;
                if matches!(
                    writing_mode,
                    RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr
                ) && request.size.height < metrics.font_size
                {
                    return Err(TextLayoutError::InsufficientRubyLayoutSpace);
                }
                Ok(MeasuredRuby {
                    index,
                    annotation,
                    writing_mode,
                    authored_position,
                    position,
                    style,
                    metrics,
                    shaped,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { annotations })
    }

    pub(crate) fn mark_base_runs(
        &self,
        runs: &mut [crate::document_layout::ShapedDocumentRun<'_>],
    ) {
        // No extra allocation interval is recorded outside these exact run
        // owners. A document without Ruby leaves every selection false.
        for annotation in &self.annotations {
            for run in &mut runs[annotation.annotation.base_runs().as_usize_range()] {
                run.record_ruby_allocations = true;
            }
        }
    }

    pub(crate) fn body_request<E: std::error::Error + 'static>(
        &self,
        request: TextLayoutRequest,
    ) -> Result<TextLayoutRequest, TextLayoutError<E>> {
        let mut insets = RubyBodyInsets::default();
        for annotation in self
            .annotations
            .iter()
            .filter(|annotation| annotation.writing_mode == RichTextWritingMode::HorizontalTb)
        {
            if annotation.authored_position == RichTextRubyPosition::InterCharacter {
                continue;
            }
            let reserve = annotation.metrics.font_size + annotation.metrics.gap;
            if annotation.position == RichTextRubyPosition::Under {
                insets.bottom = insets.bottom.max(reserve);
            } else {
                insets.top = insets.top.max(reserve);
            }
        }
        let height = request.size.height - insets.top - insets.bottom;
        for annotation in self
            .annotations
            .iter()
            .filter(|annotation| annotation.writing_mode != RichTextWritingMode::HorizontalTb)
        {
            if height < annotation.metrics.font_size {
                return Err(TextLayoutError::InsufficientRubyLayoutSpace);
            }
            let measure = annotation.vertical_measure(height);
            let reserve = usize_to_f32(measure.tracks) * measure.track_width
                + usize_to_f32(measure.tracks.saturating_sub(1)) * annotation.metrics.collision_gap
                + annotation.metrics.gap;
            if ruby_track_is_right(annotation.writing_mode, annotation.position) {
                insets.right = insets.right.max(reserve);
            } else {
                insets.left = insets.left.max(reserve);
            }
        }
        let width = request.size.width - insets.left - insets.right;
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

    pub(crate) fn allocate_vertical(
        &self,
        runs: &mut [crate::document_layout::ShapedDocumentRun<'_>],
        height: f32,
    ) {
        // Descendant annotations first: an enclosing Ruby measures the actual
        // allocation of its body, including a nested annotation's requirement.
        for annotation in self
            .annotations
            .iter()
            .rev()
            .filter(|annotation| annotation.writing_mode != RichTextWritingMode::HorizontalTb)
        {
            let range = annotation.annotation.source_base_range();
            let run_range = annotation.annotation.base_runs().as_usize_range();
            let base_runs = &mut runs[run_range];
            let covered = |cluster: &&VerticalPlacement| {
                range.start <= cluster.source_range.start && cluster.source_range.end <= range.end
            };
            let mut selected = base_runs
                .iter()
                .flat_map(|run| run.vertical.iter())
                .filter(covered);
            let Some(first) = selected.next() else {
                continue;
            };
            let first_source = first.source_range.start;
            let hard_line = first.hard_line;
            let mut count = 1;
            let mut current = first.allocated_inline_advance;
            let mut same_hard_line = true;
            for cluster in selected {
                count += 1;
                current += cluster.allocated_inline_advance;
                same_hard_line &= cluster.hard_line == hard_line;
            }
            let required = current.max(annotation.vertical_measure(height).inline_extent);
            let extra = (required - current) / usize_to_f32(count);
            for cluster in base_runs.iter_mut().flat_map(|run| run.vertical.iter_mut()) {
                if range.start <= cluster.source_range.start
                    && cluster.source_range.end <= range.end
                {
                    cluster.allocated_inline_advance += extra;
                    if required <= height
                        && same_hard_line
                        && cluster.source_range.start != first_source
                    {
                        cluster.break_allowed_before = false;
                    }
                }
            }
        }
    }

    pub(crate) fn inter_character_inline_advance_after(&self, source_range: RichTextRange) -> f32 {
        self.annotations
            .iter()
            .filter(|annotation| annotation.writing_mode == RichTextWritingMode::HorizontalTb)
            .filter(|annotation| {
                annotation.authored_position == RichTextRubyPosition::InterCharacter
            })
            .filter(|annotation| annotation.annotation.source_base_range().end == source_range.end)
            .filter(|annotation| {
                ranges_overlap(annotation.annotation.source_base_range(), source_range)
            })
            .map(|annotation| annotation.metrics.font_size + annotation.metrics.gap)
            .sum()
    }

    pub(crate) fn finish(
        self,
        request: TextLayoutRequest,
        body_request: TextLayoutRequest,
        body_glyphs: &[TextLayoutGlyph],
        runs: &[crate::TextLayoutRun],
        body_allocations: &[TextLayoutBodyAllocation],
    ) -> Vec<TextLayoutRuby> {
        self.annotations
            .into_iter()
            .filter_map(|annotation| {
                let range = annotation.annotation.source_base_range();
                // The admitted document validates this run interval and layout
                // records every input run before this owning projection.
                let run_range = annotation.annotation.base_runs().as_usize_range();
                let base_runs = &runs[run_range.clone()];
                let glyph_start = base_runs.first()?.glyph_range.start;
                let glyph_end = base_runs.last()?.glyph_range.end;
                let first = body_allocations
                    .partition_point(|allocation| allocation.glyph_range.start < glyph_start);
                let length = body_allocations[first..]
                    .partition_point(|allocation| allocation.glyph_range.end <= glyph_end);
                let mut base_allocations = body_allocations[first..first + length]
                    .iter()
                    .filter(|allocation| {
                        let start = usize::try_from(allocation.glyph_range.start).ok();
                        let end = usize::try_from(allocation.glyph_range.end).ok();
                        start
                            .zip(end)
                            .and_then(|(start, end)| body_glyphs.get(start..end))
                            .is_some_and(|glyphs| {
                                !glyphs.is_empty()
                                    && glyphs.iter().all(|glyph| {
                                        usize::try_from(glyph.run_index)
                                            .is_ok_and(|run| run_range.contains(&run))
                                            && range.start <= glyph.source_range.start
                                            && glyph.source_range.end <= range.end
                                    })
                            })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if base_allocations.is_empty() {
                    return None;
                }
                if matches!(
                    annotation.writing_mode,
                    RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr
                ) {
                    trim_inline_overhang(
                        &mut base_allocations,
                        body_glyphs,
                        annotation.metrics.overhang,
                    );
                }
                let base_bounds =
                    TextLayoutRuby::measure_base_bounds(&base_allocations, body_glyphs)?;
                let glyphs = match annotation.writing_mode {
                    RichTextWritingMode::HorizontalTb
                        if annotation.authored_position == RichTextRubyPosition::InterCharacter =>
                    {
                        place_inter_character(
                            base_bounds,
                            request,
                            annotation.metrics,
                            &annotation.shaped,
                        )
                    }
                    RichTextWritingMode::HorizontalTb => place_horizontal(
                        annotation.position,
                        base_bounds,
                        request,
                        annotation.metrics,
                        &annotation.shaped,
                    ),
                    RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
                        place_vertical(
                            base_bounds,
                            TextLayoutRequest {
                                origin: LayoutPoint::new(request.origin.x, body_request.origin.y),
                                size: LayoutSize::new(request.size.width, body_request.size.height),
                                ..request
                            },
                            annotation.metrics,
                            annotation.writing_mode,
                            annotation.position,
                            &annotation.shaped,
                        )
                    }
                };
                let ruby_bounds = union_rects(
                    glyphs
                        .iter()
                        .map(|glyph| glyph.layout_bounds.union(glyph.ink_bounds)),
                )
                .unwrap_or(base_bounds);
                Some(TextLayoutRuby {
                    ruby_index: saturating_u32(annotation.index),
                    owner_node: annotation.annotation.owner_node(),
                    body_nodes: annotation.annotation.body_nodes(),
                    base_runs: annotation.annotation.base_runs(),
                    base_range: range,
                    text: annotation.annotation.text().to_owned(),
                    base_allocations,
                    base_bounds,
                    ruby_bounds,
                    glyphs,
                    writing_mode: annotation.writing_mode,
                    style: annotation.style,
                    presentation: annotation.annotation.presentation().clone(),
                })
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
struct VerticalRubyMeasure {
    cells_per_track: usize,
    tracks: usize,
    track_width: f32,
    inline_extent: f32,
}

impl MeasuredRuby<'_> {
    fn vertical_measure(&self, height: f32) -> VerticalRubyMeasure {
        vertical_measure(&local_glyphs(&self.shaped), self.metrics, height)
    }
}

fn vertical_measure(
    local: &[LocalRubyGlyph],
    metrics: RubyMetrics,
    height: f32,
) -> VerticalRubyMeasure {
    let clusters = cluster_slices(local).collect::<Vec<_>>();
    let count = clusters.len().max(1);
    let mut cells_per_track = 1;
    while cells_per_track < count && usize_to_f32(cells_per_track + 1) * metrics.font_size <= height
    {
        cells_per_track += 1;
    }
    let track_width = clusters
        .iter()
        .filter_map(|cluster| union_rects(cluster.iter().map(|glyph| glyph.ink_bounds)))
        .map(|bounds| bounds.width)
        .fold(metrics.font_size, f32::max);
    VerticalRubyMeasure {
        cells_per_track,
        tracks: count.div_ceil(cells_per_track),
        track_width,
        inline_extent: usize_to_f32(cells_per_track) * metrics.font_size,
    }
}

fn trim_inline_overhang(
    allocations: &mut [TextLayoutBodyAllocation],
    glyphs: &[TextLayoutGlyph],
    overhang: f32,
) {
    let mut start = 0;
    while start < allocations.len() {
        let first_glyph =
            usize::try_from(allocations[start].glyph_range.start).unwrap_or(glyphs.len());
        let line = glyphs[first_glyph].line_index;
        let end = start
            + allocations[start..].partition_point(|allocation| {
                let index = usize::try_from(allocation.glyph_range.start).unwrap_or(glyphs.len());
                glyphs[index].line_index == line
            });
        let leading = (glyphs[first_glyph].layout_bounds.y - allocations[start].bounds.y)
            .max(0.0)
            .min(overhang);
        allocations[start].bounds.y += leading;
        allocations[start].bounds.height -= leading;
        let last = end - 1;
        let last_glyph =
            usize::try_from(allocations[last].glyph_range.end).unwrap_or(glyphs.len()) - 1;
        let trailing = (allocations[last].bounds.bottom()
            - glyphs[last_glyph].layout_bounds.bottom())
        .max(0.0)
        .min(overhang);
        allocations[last].bounds.height -= trailing;
        start = end;
    }
}

fn place_horizontal(
    position: RichTextRubyPosition,
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
    let y = match position {
        RichTextRubyPosition::Under => base_bounds.bottom() + metrics.gap,
        RichTextRubyPosition::Auto
        | RichTextRubyPosition::Over
        | RichTextRubyPosition::InterCharacter => base_bounds.y - metrics.font_size - metrics.gap,
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

fn place_inter_character(
    base_bounds: LayoutRect,
    request: TextLayoutRequest,
    metrics: RubyMetrics,
    shaped: &ShapedTextRun,
) -> Vec<TextLayoutRubyGlyph> {
    let local = local_glyphs(shaped);
    let clusters = cluster_slices(&local).collect::<Vec<_>>();
    let extent = usize_to_f32(clusters.len().max(1)) * metrics.font_size;
    let x = (base_bounds.right() + metrics.gap)
        .max(request.origin.x)
        .min((request.origin.x + request.size.width - metrics.font_size).max(request.origin.x));
    let y = (base_bounds.y + (base_bounds.height - extent) * 0.5)
        .max(request.origin.y)
        .min((request.origin.y + request.size.height - extent).max(request.origin.y));
    place_vertical_cells(&clusters, x, y, metrics)
}

fn place_vertical(
    base_bounds: LayoutRect,
    request: TextLayoutRequest,
    metrics: RubyMetrics,
    writing_mode: RichTextWritingMode,
    position: RichTextRubyPosition,
    shaped: &ShapedTextRun,
) -> Vec<TextLayoutRubyGlyph> {
    let local = local_glyphs(shaped);
    let clusters = cluster_slices(&local).collect::<Vec<_>>();
    let measure = vertical_measure(&local, metrics, request.size.height);
    let side_right = ruby_track_is_right(writing_mode, position);
    let mut glyphs = Vec::with_capacity(local.len());
    let mut cluster_start = 0;
    while cluster_start < clusters.len() {
        let cluster_end = cluster_start
            .saturating_add(measure.cells_per_track)
            .min(clusters.len());
        let extent = usize_to_f32(cluster_end - cluster_start) * metrics.font_size;
        let ideal_y = base_bounds.y + (base_bounds.height - extent) * 0.5;
        let min_y = (base_bounds.y - metrics.overhang).max(request.origin.y);
        let max_y = (base_bounds.bottom() + metrics.overhang - extent)
            .min(request.origin.y + request.size.height - extent);
        let y = if min_y <= max_y {
            ideal_y.max(min_y).min(max_y)
        } else {
            request.origin.y
        };
        let track = cluster_start / measure.cells_per_track;
        let track_x = if side_right {
            base_bounds.right()
                + metrics.gap
                + usize_to_f32(track) * (measure.track_width + metrics.collision_gap)
        } else {
            base_bounds.x
                - metrics.gap
                - usize_to_f32(track + 1) * measure.track_width
                - usize_to_f32(track) * metrics.collision_gap
        };
        let x = track_x + (measure.track_width - metrics.font_size) * 0.5;
        let mut track_glyphs =
            place_vertical_cells(&clusters[cluster_start..cluster_end], x, y, metrics);
        for glyph in &mut track_glyphs {
            glyph.cluster_index += saturating_u32(cluster_start);
        }
        glyphs.extend(track_glyphs);
        cluster_start = cluster_end;
    }
    glyphs
}

fn place_vertical_cells(
    clusters: &[&[LocalRubyGlyph]],
    x: f32,
    y: f32,
    metrics: RubyMetrics,
) -> Vec<TextLayoutRubyGlyph> {
    let mut out = Vec::new();
    for (cluster_index, cluster) in clusters.iter().enumerate() {
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

fn collision_shift(annotation: &TextLayoutRuby, collision: LayoutRect, gap: f32) -> (f32, f32) {
    match annotation.writing_mode {
        RichTextWritingMode::HorizontalTb => {
            let position = ruby_position_from_presentation(annotation);
            if matches!(position, RichTextRubyPosition::InterCharacter) {
                (collision.right() + gap - annotation.ruby_bounds.x, 0.0)
            } else if matches!(position, RichTextRubyPosition::Under) {
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
    let mut count = 0;
    add_canonical_count(&mut count, shaped.glyphs().len())?;
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
            .and_then(|layout| nonnegative_milli(layout.ruby_overhang))
            .unwrap_or(font_size * 0.5),
        collision_gap: layout
            .and_then(|layout| nonnegative_milli(layout.ruby_collision_gap))
            .unwrap_or(DEFAULT_COLLISION_GAP),
    }
}

fn positive_milli(value: Option<Milli>) -> Option<f32> {
    value
        .map(Milli::as_f32)
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn nonnegative_milli(value: Option<Milli>) -> Option<f32> {
    value
        .map(Milli::as_f32)
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

fn effective_ruby_position(
    writing_mode: RichTextWritingMode,
    position: RichTextRubyPosition,
) -> RichTextRubyPosition {
    if matches!(
        writing_mode,
        RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr
    ) && matches!(position, RichTextRubyPosition::InterCharacter)
    {
        RichTextRubyPosition::Over
    } else {
        position
    }
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
        .and_then(|layout| nonnegative_milli(layout.ruby_collision_gap))
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
    // Shaped Ruby counts and actual emitted body counts are admitted before
    // measurement. Full word conversion also avoids a panicking narrowing
    // conversion and does not collapse counts after u16::MAX.
    value
        .to_le_bytes()
        .chunks_exact(2)
        .rev()
        .fold(0.0, |total, word| {
            total * 65_536.0 + f32::from(u16::from_le_bytes([word[0], word[1]]))
        })
}

#[cfg(test)]
mod count_tests {
    use super::usize_to_f32;

    #[test]
    fn admitted_cluster_counts_preserve_the_high_word_without_clamping() {
        assert_eq!(usize_to_f32(65_535), 65_535.0);
        assert_eq!(usize_to_f32(65_536), 65_536.0);
        assert_eq!(usize_to_f32(65_537), 65_537.0);
        assert_eq!(
            usize_to_f32(usize::try_from(u32::MAX).unwrap()),
            4_294_967_296.0
        );
    }
}
