//! Font-shaped layout of the canonical resolved-text document.

use std::collections::BTreeMap;

use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRun, RichTextRange, RichTextVerticalLatinMode,
    RichTextWritingMode,
};

use crate::{
    GlyphOrientation, GlyphVerticalForm, JlreqStrictness, LayoutPoint, LayoutRect, LayoutSize,
    ShapedTextGlyph, ShapedTextRun, TextLayout, TextLayoutError, TextLayoutGlyph,
    TextLayoutGlyphSource, TextLayoutLine, TextLayoutRequest, TextLayoutRun, TextLayoutSourceMap,
    TextShapeRequest, TextShaper,
    document_hash::layout_hash,
    document_vertical::{VerticalPlanCluster, plan_vertical_segment},
    jlreq_punctuation,
    vertical_clusters::{line_break_offsets, vertical_clusters},
};

/// Shapes and lays out one canonical resolved-text document.
pub fn layout_document<S: TextShaper>(
    document: &ResolvedTextDocument<'_>,
    request: TextLayoutRequest,
    shaper: &mut S,
) -> Result<TextLayout, TextLayoutError<S::Error>> {
    validate_request(request)?;
    let font_inventory = shaper.font_inventory_hash();
    let logical_ordinals = logical_ordinals(document);
    let body_request = crate::document_ruby::body_request::<S::Error>(document, request)?;
    let mut shaped_runs = Vec::with_capacity(document.runs().len());
    for (run_index, run) in document.runs().iter().enumerate() {
        let text = document
            .text()
            .get(run.range().start..run.range().end)
            .ok_or(TextLayoutError::InvalidRange { range: run.range() })?;
        let writing_mode = run.style().writing_mode();
        let shaped_run = shaper
            .shape_run(TextShapeRequest {
                text,
                source_range: run.source_range(),
                style: run.style(),
                locale: run.style().language(),
                direction: run.style().direction(),
                writing_mode,
            })
            .map_err(|source| TextLayoutError::Shape { run_index, source })?;
        validate_shaped_run(document, run_index, run, &shaped_run)?;
        shaped_runs.push(ShapedDocumentRun {
            run_index,
            run,
            text,
            shaped: shaped_run,
        });
    }

    let mut state = DocumentLayoutState::new(body_request);
    let mut run_index = 0;
    while run_index < shaped_runs.len() {
        let input = &shaped_runs[run_index];
        let Some(group_key) = vertical_group_key(input.run, body_request) else {
            state.place_horizontal_run(document, input, &logical_ordinals);
            run_index += 1;
            continue;
        };
        let mut group_end = run_index + 1;
        while shaped_runs.get(group_end).is_some_and(|candidate| {
            vertical_group_key(candidate.run, body_request) == Some(group_key)
        }) {
            group_end += 1;
        }
        state.place_vertical_group(
            document,
            &shaped_runs[run_index..group_end],
            group_key.0,
            group_key.1,
            &logical_ordinals,
        );
        run_index = group_end;
    }
    state.finish_line();

    let mut ruby =
        crate::document_ruby::layout_ruby(document, request, shaper, &state.glyphs, &state.runs)?;
    crate::document_ruby::resolve_collisions(&mut ruby);
    for annotation in &ruby {
        state.include_bounds(annotation.ruby_bounds);
    }
    let source_map = TextLayoutSourceMap::new(
        state
            .glyphs
            .iter()
            .map(|glyph| TextLayoutGlyphSource {
                run_index: glyph.run_index,
                source_range: glyph.source_range,
                line_index: glyph.line_index,
                cluster_index: glyph.cluster_index,
                logical_ordinal: glyph.logical_ordinal,
            })
            .collect(),
    );
    let hash = layout_hash(document, request, font_inventory, &state.glyphs, &ruby);
    Ok(TextLayout {
        lines: state.lines,
        runs: state.runs,
        glyphs: state.glyphs,
        ruby,
        bounds: state.bounds,
        source_map,
        hash,
        font_inventory,
    })
}

struct ShapedDocumentRun<'a> {
    run_index: usize,
    run: &'a ResolvedTextRun,
    text: &'a str,
    shaped: ShapedTextRun,
}

fn vertical_group_key(
    run: &ResolvedTextRun,
    request: TextLayoutRequest,
) -> Option<(RichTextWritingMode, JlreqStrictness)> {
    let writing_mode = run.style().writing_mode();
    if writing_mode == RichTextWritingMode::HorizontalTb {
        return None;
    }
    let strictness = run
        .presentation()
        .layout
        .as_ref()
        .map_or(request.jlreq_strictness, |layout| {
            request.jlreq_strictness.resolve(layout.jlreq_strictness)
        });
    Some((writing_mode, strictness))
}

struct DocumentLayoutState {
    request: TextLayoutRequest,
    horizontal: LayoutPoint,
    horizontal_hard_line: usize,
    horizontal_line_height: f32,
    vertical_rl: VerticalCursor,
    vertical_lr: VerticalCursor,
    lines: Vec<TextLayoutLine>,
    runs: Vec<TextLayoutRun>,
    glyphs: Vec<TextLayoutGlyph>,
    bounds: Option<LayoutRect>,
    active_line: Option<ActiveLine>,
    next_cluster_index: u32,
}

#[derive(Clone, Debug)]
struct VerticalCursor {
    boundary_x: f32,
    y: f32,
    hard_line: usize,
    previous_cluster: Option<String>,
}

#[derive(Clone, Copy)]
struct ActiveLine {
    glyph_start: usize,
    writing_mode: RichTextWritingMode,
    track_bits: u32,
    line_index: u32,
}

#[derive(Clone, Copy)]
struct LocalGlyph {
    key: crate::ShapedGlyphKey,
    source_range: RichTextRange,
    origin: LayoutPoint,
    ink_bounds: LayoutRect,
}

struct VerticalPlacement {
    run_index: usize,
    font_size: f32,
    column_step: f32,
    source_range: RichTextRange,
    text: String,
    orientation: GlyphOrientation,
    vertical_form: GlyphVerticalForm,
    local: Vec<LocalGlyph>,
    local_bounds: LayoutRect,
    inline_advance: f32,
    post_inline_advance: f32,
    hard_line: usize,
    break_allowed_before: bool,
}

#[derive(Clone, Copy)]
struct VerticalGlyphContext<'a> {
    run_index: usize,
    line_index: u32,
    cluster_index: u32,
    logical_ordinals: &'a BTreeMap<(usize, usize), u32>,
    orientation: GlyphOrientation,
    vertical_form: GlyphVerticalForm,
    cell: LayoutRect,
}

impl DocumentLayoutState {
    fn new(request: TextLayoutRequest) -> Self {
        let right = request.origin.x + request.size.width;
        Self {
            request,
            horizontal: request.origin,
            horizontal_hard_line: 0,
            horizontal_line_height: 0.0,
            vertical_rl: VerticalCursor {
                boundary_x: right,
                y: request.origin.y,
                hard_line: 0,
                previous_cluster: None,
            },
            vertical_lr: VerticalCursor {
                boundary_x: request.origin.x,
                y: request.origin.y,
                hard_line: 0,
                previous_cluster: None,
            },
            lines: Vec::new(),
            runs: Vec::new(),
            glyphs: Vec::new(),
            bounds: None,
            active_line: None,
            next_cluster_index: 0,
        }
    }

    fn place_horizontal_run(
        &mut self,
        document: &ResolvedTextDocument<'_>,
        input: &ShapedDocumentRun<'_>,
        logical_ordinals: &BTreeMap<(usize, usize), u32>,
    ) {
        let glyph_start = self.glyphs.len();
        self.place_horizontal(
            document,
            input.run_index,
            input.run,
            &input.shaped,
            logical_ordinals,
        );
        let glyph_end = self.glyphs.len();
        self.record_run(input.run_index, input.run, glyph_start, glyph_end);
    }

    fn record_run(
        &mut self,
        run_index: usize,
        run: &ResolvedTextRun,
        glyph_start: usize,
        glyph_end: usize,
    ) {
        let run_bounds = union_rects(self.glyphs[glyph_start..glyph_end].iter().map(glyph_bounds));
        if let Some(bounds) = run_bounds {
            self.include_bounds(bounds);
        }
        self.runs.push(TextLayoutRun {
            run_index: saturating_u32(run_index),
            source_range: run.source_range(),
            glyph_range: saturating_u32(glyph_start)..saturating_u32(glyph_end),
            bounds: run_bounds.unwrap_or(LayoutRect::new(
                self.request.origin.x,
                self.request.origin.y,
                0.0,
                0.0,
            )),
            writing_mode: run.style().writing_mode(),
            style: run.style().clone(),
            presentation: run.presentation().clone(),
        });
    }

    fn place_horizontal(
        &mut self,
        document: &ResolvedTextDocument<'_>,
        run_index: usize,
        run: &ResolvedTextRun,
        shaped: &ShapedTextRun,
        logical_ordinals: &BTreeMap<(usize, usize), u32>,
    ) {
        let line_height = milli_to_pixels(run.style().line_height_milli());
        for cluster in cluster_slices(shaped.glyphs()) {
            let hard_line = source_hard_line(document, cluster[0].source_range.start);
            self.sync_horizontal_hard_line(hard_line, line_height);
            let cluster_width = cluster.iter().map(|glyph| glyph.advance.width).sum::<f32>();
            let right = self.request.origin.x + self.request.size.width;
            if self.request.horizontal_wrap == crate::HorizontalWrap::Wrap
                && self.horizontal.x > self.request.origin.x
                && self.horizontal.x + cluster_width > right
            {
                self.break_horizontal_line(line_height);
            }
            self.horizontal_line_height = self.horizontal_line_height.max(line_height);
            let line_index = self.ensure_line(RichTextWritingMode::HorizontalTb, self.horizontal.y);
            let cluster_index = self.allocate_cluster();
            for glyph in cluster {
                let layout_x = self.horizontal.x;
                let origin = LayoutPoint::new(
                    layout_x + glyph.offset.x,
                    self.horizontal.y + glyph.offset.y,
                );
                let ink_bounds = translate_rect(glyph.ink_bounds, origin.x, origin.y);
                let layout_bounds = LayoutRect::new(
                    layout_x,
                    self.horizontal.y,
                    glyph.advance.width,
                    line_height,
                )
                .union(ink_bounds);
                self.glyphs.push(TextLayoutGlyph {
                    run_index: saturating_u32(run_index),
                    source_range: glyph.source_range,
                    line_index,
                    cluster_index,
                    logical_ordinal: logical_ordinal(logical_ordinals, glyph.source_range),
                    origin,
                    advance: glyph.advance,
                    layout_bounds,
                    ink_bounds,
                    orientation: GlyphOrientation::Upright,
                    vertical_form: GlyphVerticalForm::None,
                    inline_scale: 1.0,
                    shape_key: glyph.key,
                });
                self.horizontal.x += glyph.advance.width;
            }
        }
    }

    fn place_vertical_group(
        &mut self,
        document: &ResolvedTextDocument<'_>,
        runs: &[ShapedDocumentRun<'_>],
        writing_mode: RichTextWritingMode,
        strictness: JlreqStrictness,
        logical_ordinals: &BTreeMap<(usize, usize), u32>,
    ) {
        let group_glyph_start = self.glyphs.len();
        let mut placements = runs
            .iter()
            .flat_map(|input| {
                let vertical_latin = input
                    .run
                    .presentation()
                    .layout
                    .as_ref()
                    .map_or(RichTextVerticalLatinMode::Mixed, |layout| {
                        layout.vertical_latin
                    });
                vertical_placements(
                    document,
                    input.run_index,
                    input.run,
                    input.text,
                    &input.shaped,
                    vertical_latin,
                )
            })
            .collect::<Vec<_>>();
        let paragraph_breaks = line_break_offsets(document.text());
        for cluster in &mut placements {
            cluster.break_allowed_before = paragraph_breaks.contains(&cluster.source_range.start);
        }
        let mut segment_start = 0;
        while segment_start < placements.len() {
            let hard_line = placements[segment_start].hard_line;
            let segment_end = placements[segment_start..]
                .iter()
                .position(|cluster| cluster.hard_line != hard_line)
                .map_or(placements.len(), |offset| segment_start + offset);
            self.sync_vertical_hard_line(
                writing_mode,
                hard_line,
                placements[segment_start].column_step,
            );
            let plan_input = placements[segment_start..segment_end]
                .iter()
                .map(|cluster| VerticalPlanCluster {
                    text: &cluster.text,
                    advance: cluster.inline_advance + cluster.post_inline_advance,
                    break_allowed_before: cluster.break_allowed_before,
                })
                .collect::<Vec<_>>();
            let plan = plan_vertical_segment(
                &plan_input,
                self.request.origin.y,
                self.vertical_cursor(writing_mode).y,
                self.request.size.height,
                strictness,
            );
            for (offset, cluster) in placements[segment_start..segment_end].iter().enumerate() {
                if plan.breaks_before(offset) {
                    self.break_vertical_line(writing_mode, cluster.column_step);
                }
                self.place_vertical_cluster(writing_mode, cluster, logical_ordinals);
            }
            segment_start = segment_end;
        }

        let group_glyph_end = self.glyphs.len();
        let mut glyph_start = group_glyph_start;
        for input in runs {
            let run_index = saturating_u32(input.run_index);
            let mut glyph_end = glyph_start;
            while glyph_end < group_glyph_end && self.glyphs[glyph_end].run_index == run_index {
                glyph_end += 1;
            }
            self.record_run(input.run_index, input.run, glyph_start, glyph_end);
            glyph_start = glyph_end;
        }
        debug_assert_eq!(glyph_start, group_glyph_end);
    }

    fn place_vertical_cluster(
        &mut self,
        writing_mode: RichTextWritingMode,
        cluster: &VerticalPlacement,
        logical_ordinals: &BTreeMap<(usize, usize), u32>,
    ) {
        let font_size = cluster.font_size;
        let (boundary_x, cursor_y) = {
            let cursor = self.vertical_cursor(writing_mode);
            (cursor.boundary_x, cursor.y)
        };
        let cell_x = match writing_mode {
            RichTextWritingMode::VerticalRl => boundary_x - font_size,
            RichTextWritingMode::VerticalLr => boundary_x,
            RichTextWritingMode::HorizontalTb => unreachable!("vertical run selected"),
        };
        let line_index = self.ensure_line(writing_mode, cell_x);
        let cluster_index = self.allocate_cluster();
        let context = VerticalGlyphContext {
            run_index: cluster.run_index,
            line_index,
            cluster_index,
            logical_ordinals,
            orientation: cluster.orientation,
            vertical_form: cluster.vertical_form,
            cell: LayoutRect::new(cell_x, cursor_y, font_size, cluster.inline_advance),
        };
        self.glyphs.extend(place_vertical_glyphs(
            &cluster.local,
            context,
            cluster.local_bounds,
        ));
        let cursor = self.vertical_cursor_mut(writing_mode);
        cursor.y += cluster.inline_advance + cluster.post_inline_advance;
        cursor.previous_cluster = Some(cluster.text.clone());
    }

    fn sync_horizontal_hard_line(&mut self, target: usize, line_height: f32) {
        while self.horizontal_hard_line < target {
            let step = self.horizontal_line_height.max(line_height);
            self.finish_line();
            self.horizontal.x = self.request.origin.x;
            self.horizontal.y += step;
            self.horizontal_hard_line += 1;
            self.horizontal_line_height = 0.0;
        }
    }

    fn break_horizontal_line(&mut self, line_height: f32) {
        let step = self.horizontal_line_height.max(line_height);
        self.finish_line();
        self.horizontal.x = self.request.origin.x;
        self.horizontal.y += step;
        self.horizontal_line_height = 0.0;
    }

    fn sync_vertical_hard_line(
        &mut self,
        writing_mode: RichTextWritingMode,
        target: usize,
        column_step: f32,
    ) {
        while self.vertical_cursor(writing_mode).hard_line < target {
            self.break_vertical_line(writing_mode, column_step);
            self.vertical_cursor_mut(writing_mode).hard_line += 1;
        }
    }

    fn break_vertical_line(&mut self, writing_mode: RichTextWritingMode, column_step: f32) {
        self.finish_line();
        let origin_y = self.request.origin.y;
        let cursor = self.vertical_cursor_mut(writing_mode);
        cursor.y = origin_y;
        cursor.boundary_x += match writing_mode {
            RichTextWritingMode::VerticalRl => -column_step,
            RichTextWritingMode::VerticalLr => column_step,
            RichTextWritingMode::HorizontalTb => unreachable!("vertical run selected"),
        };
        cursor.previous_cluster = None;
    }

    fn vertical_cursor(&self, writing_mode: RichTextWritingMode) -> &VerticalCursor {
        match writing_mode {
            RichTextWritingMode::VerticalRl => &self.vertical_rl,
            RichTextWritingMode::VerticalLr => &self.vertical_lr,
            RichTextWritingMode::HorizontalTb => unreachable!("vertical run selected"),
        }
    }

    fn vertical_cursor_mut(&mut self, writing_mode: RichTextWritingMode) -> &mut VerticalCursor {
        match writing_mode {
            RichTextWritingMode::VerticalRl => &mut self.vertical_rl,
            RichTextWritingMode::VerticalLr => &mut self.vertical_lr,
            RichTextWritingMode::HorizontalTb => unreachable!("vertical run selected"),
        }
    }

    fn allocate_cluster(&mut self) -> u32 {
        let value = self.next_cluster_index;
        self.next_cluster_index = self.next_cluster_index.saturating_add(1);
        value
    }

    fn ensure_line(&mut self, writing_mode: RichTextWritingMode, track: f32) -> u32 {
        let requested = (writing_mode, track.to_bits());
        if self
            .active_line
            .is_some_and(|line| (line.writing_mode, line.track_bits) != requested)
        {
            self.finish_line();
        }
        if self.active_line.is_none() {
            self.active_line = Some(ActiveLine {
                glyph_start: self.glyphs.len(),
                writing_mode,
                track_bits: track.to_bits(),
                line_index: saturating_u32(self.lines.len()),
            });
        }
        self.active_line.map_or(u32::MAX, |line| line.line_index)
    }

    fn finish_line(&mut self) {
        let Some(line) = self.active_line.take() else {
            return;
        };
        let glyph_end = self.glyphs.len();
        let Some(bounds) = union_rects(
            self.glyphs[line.glyph_start..glyph_end]
                .iter()
                .map(glyph_bounds),
        ) else {
            return;
        };
        let source_start = self.glyphs[line.glyph_start..glyph_end]
            .iter()
            .map(|glyph| glyph.source_range.start)
            .min()
            .unwrap_or(0);
        let source_end = self.glyphs[line.glyph_start..glyph_end]
            .iter()
            .map(|glyph| glyph.source_range.end)
            .max()
            .unwrap_or(source_start);
        self.lines.push(TextLayoutLine {
            source_range: RichTextRange::new(source_start, source_end),
            glyph_range: saturating_u32(line.glyph_start)..saturating_u32(glyph_end),
            bounds,
            writing_mode: line.writing_mode,
        });
    }

    fn include_bounds(&mut self, bounds: LayoutRect) {
        self.bounds = Some(
            self.bounds
                .map_or(bounds, |existing| existing.union(bounds)),
        );
    }
}

fn vertical_placements(
    document: &ResolvedTextDocument<'_>,
    run_index: usize,
    run: &ResolvedTextRun,
    text: &str,
    shaped: &ShapedTextRun,
    vertical_latin: RichTextVerticalLatinMode,
) -> Vec<VerticalPlacement> {
    let font_size = milli_to_pixels(run.style().font_size_milli());
    let column_step = vertical_column_step(run);
    vertical_clusters(text, vertical_latin)
        .into_iter()
        .filter_map(|cluster| {
            if cluster
                .text
                .chars()
                .all(|character| matches!(character, '\r' | '\n'))
            {
                return None;
            }
            let source_range = RichTextRange::new(
                run.source_range().start + cluster.range.start,
                run.source_range().start + cluster.range.end,
            );
            let shaped_glyphs = shaped
                .glyphs()
                .iter()
                .filter(|glyph| ranges_overlap(glyph.source_range, source_range))
                .collect::<Vec<_>>();
            if shaped_glyphs.is_empty() {
                return None;
            }
            let local = local_glyphs(&shaped_glyphs);
            let bounds = local_bounds(&local, font_size);
            let inline_advance = if jlreq_punctuation::is_compressible_cluster(&cluster.text) {
                font_size * 0.5
            } else {
                match cluster.orientation {
                    GlyphOrientation::SidewaysCw => bounds.width.max(font_size),
                    GlyphOrientation::Upright | GlyphOrientation::TextCombineUpright => font_size,
                }
            };
            let post_inline_advance =
                crate::document_ruby::inter_character_extent_after(document, source_range);
            Some(VerticalPlacement {
                run_index,
                font_size,
                column_step,
                source_range,
                text: cluster.text,
                orientation: cluster.orientation,
                vertical_form: cluster.vertical_form,
                local,
                local_bounds: bounds,
                inline_advance,
                post_inline_advance,
                hard_line: source_hard_line(document, source_range.start),
                break_allowed_before: cluster.break_allowed_before,
            })
        })
        .collect()
}

fn place_vertical_glyphs(
    local: &[LocalGlyph],
    context: VerticalGlyphContext<'_>,
    local_bounds: LayoutRect,
) -> Vec<TextLayoutGlyph> {
    let inline_scale = if context.orientation == GlyphOrientation::TextCombineUpright
        && local_bounds.width > context.cell.width
    {
        context.cell.width / local_bounds.width
    } else {
        1.0
    };
    let transformed_bounds = match context.orientation {
        GlyphOrientation::SidewaysCw => rotate_rect_cw(local_bounds),
        GlyphOrientation::Upright | GlyphOrientation::TextCombineUpright => {
            scale_rect_x(local_bounds, inline_scale)
        }
    };
    let translation = LayoutPoint::new(
        context.cell.x + (context.cell.width - transformed_bounds.width) * 0.5
            - transformed_bounds.x,
        context.cell.y + (context.cell.height - transformed_bounds.height) * 0.5
            - transformed_bounds.y,
    );
    local
        .iter()
        .enumerate()
        .map(|(index, glyph)| {
            let (origin, ink_bounds) = match context.orientation {
                GlyphOrientation::SidewaysCw => (
                    LayoutPoint::new(
                        -glyph.origin.y + translation.x,
                        glyph.origin.x + translation.y,
                    ),
                    translate_rect(
                        rotate_rect_cw(glyph.ink_bounds),
                        translation.x,
                        translation.y,
                    ),
                ),
                GlyphOrientation::Upright | GlyphOrientation::TextCombineUpright => (
                    LayoutPoint::new(
                        glyph.origin.x * inline_scale + translation.x,
                        glyph.origin.y + translation.y,
                    ),
                    translate_rect(
                        scale_rect_x(glyph.ink_bounds, inline_scale),
                        translation.x,
                        translation.y,
                    ),
                ),
            };
            TextLayoutGlyph {
                run_index: saturating_u32(context.run_index),
                source_range: glyph.source_range,
                line_index: context.line_index,
                cluster_index: context.cluster_index,
                logical_ordinal: logical_ordinal(context.logical_ordinals, glyph.source_range),
                origin,
                advance: if index + 1 == local.len() {
                    LayoutSize::new(0.0, context.cell.height)
                } else {
                    LayoutSize::default()
                },
                layout_bounds: context.cell,
                ink_bounds,
                orientation: context.orientation,
                vertical_form: context.vertical_form,
                inline_scale,
                shape_key: glyph.key,
            }
        })
        .collect()
}

fn local_glyphs(glyphs: &[&ShapedTextGlyph]) -> Vec<LocalGlyph> {
    let mut cursor = 0.0_f32;
    glyphs
        .iter()
        .map(|glyph| {
            let origin = LayoutPoint::new(cursor + glyph.offset.x, glyph.offset.y);
            let ink_bounds = translate_rect(glyph.ink_bounds, origin.x, origin.y);
            cursor += glyph.advance.width;
            LocalGlyph {
                key: glyph.key,
                source_range: glyph.source_range,
                origin,
                ink_bounds,
            }
        })
        .collect()
}

fn local_bounds(glyphs: &[LocalGlyph], font_size: f32) -> LayoutRect {
    union_rects(glyphs.iter().map(|glyph| glyph.ink_bounds)).map_or(
        LayoutRect::new(0.0, 0.0, font_size, font_size),
        |bounds| {
            if bounds.width == 0.0 && bounds.height == 0.0 {
                LayoutRect::new(0.0, 0.0, font_size, font_size)
            } else {
                bounds
            }
        },
    )
}

fn validate_request<E: std::error::Error + 'static>(
    request: TextLayoutRequest,
) -> Result<(), TextLayoutError<E>> {
    let values = [
        request.origin.x,
        request.origin.y,
        request.size.width,
        request.size.height,
    ];
    if values.iter().any(|value| !value.is_finite())
        || request.size.width < 0.0
        || request.size.height < 0.0
    {
        return Err(TextLayoutError::InvalidRequestGeometry);
    }
    Ok(())
}

fn validate_shaped_run<E: std::error::Error + 'static>(
    document: &ResolvedTextDocument<'_>,
    run_index: usize,
    run: &ResolvedTextRun,
    shaped: &ShapedTextRun,
) -> Result<(), TextLayoutError<E>> {
    for (glyph_index, glyph) in shaped.glyphs().iter().enumerate() {
        let range = glyph.source_range;
        let valid_range = range.start < range.end
            && run.source_range().start <= range.start
            && range.end <= run.source_range().end
            && source_range_is_utf8(document, run, range);
        if !valid_range {
            return Err(TextLayoutError::InvalidShapedRange {
                run_index,
                glyph_index,
                range,
            });
        }
        if !shaped_geometry_is_valid(glyph) {
            return Err(TextLayoutError::InvalidShapedGeometry {
                run_index,
                glyph_index,
            });
        }
    }
    Ok(())
}

fn shaped_geometry_is_valid(glyph: &ShapedTextGlyph) -> bool {
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
    values.iter().all(|value| value.is_finite())
        && glyph.advance.width >= 0.0
        && glyph.advance.height >= 0.0
        && glyph.ink_bounds.width >= 0.0
        && glyph.ink_bounds.height >= 0.0
}

fn source_range_is_utf8(
    document: &ResolvedTextDocument<'_>,
    run: &ResolvedTextRun,
    range: RichTextRange,
) -> bool {
    let start = run
        .range()
        .start
        .saturating_add(range.start.saturating_sub(run.source_range().start));
    let end = run
        .range()
        .start
        .saturating_add(range.end.saturating_sub(run.source_range().start));
    document.text().is_char_boundary(start) && document.text().is_char_boundary(end)
}

fn source_hard_line(document: &ResolvedTextDocument<'_>, source_offset: usize) -> usize {
    let local_offset = source_offset.saturating_sub(document.source_origin());
    hard_line_index(document.text(), local_offset.min(document.text().len()))
}

fn hard_line_index(text: &str, offset: usize) -> usize {
    let bytes = &text.as_bytes()[..offset];
    let mut index = 0;
    let mut lines = 0;
    while index < bytes.len() {
        if matches!(bytes[index], b'\r' | b'\n') {
            lines += 1;
            if index + 1 < bytes.len()
                && matches!(
                    (bytes[index], bytes[index + 1]),
                    (b'\r', b'\n') | (b'\n', b'\r')
                )
            {
                index += 2;
                continue;
            }
        }
        index += 1;
    }
    lines
}

fn logical_ordinals(document: &ResolvedTextDocument<'_>) -> BTreeMap<(usize, usize), u32> {
    let mut ranges = document
        .runs()
        .iter()
        .flat_map(|run| {
            unicode_segmentation::UnicodeSegmentation::grapheme_indices(
                &document.text()[run.range().start..run.range().end],
                true,
            )
            .map(move |(start, grapheme)| {
                (
                    run.source_range().start + start,
                    run.source_range().start + start + grapheme.len(),
                )
            })
        })
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    ranges.dedup();
    ranges
        .into_iter()
        .enumerate()
        .map(|(index, range)| (range, saturating_u32(index)))
        .collect()
}

fn logical_ordinal(ordinals: &BTreeMap<(usize, usize), u32>, source_range: RichTextRange) -> u32 {
    ordinals
        .range(..=(source_range.start, usize::MAX))
        .rev()
        .find(|((start, end), _)| *start <= source_range.start && source_range.start < *end)
        .map_or(u32::MAX, |(_, ordinal)| *ordinal)
}

fn cluster_slices(glyphs: &[ShapedTextGlyph]) -> impl Iterator<Item = &[ShapedTextGlyph]> {
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

fn vertical_column_step(run: &ResolvedTextRun) -> f32 {
    let line_height = milli_to_pixels(run.style().line_height_milli());
    let gap = run
        .presentation()
        .layout
        .as_ref()
        .map_or(8.0, |layout| layout.column_gap.as_f32());
    if gap.is_finite() && gap >= 0.0 {
        line_height + gap
    } else {
        line_height + 8.0
    }
}

fn glyph_bounds(glyph: &TextLayoutGlyph) -> LayoutRect {
    glyph.layout_bounds.union(glyph.ink_bounds)
}

pub(crate) fn ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

pub(crate) fn translate_rect(rect: LayoutRect, x: f32, y: f32) -> LayoutRect {
    LayoutRect::new(x + rect.x, y + rect.y, rect.width, rect.height)
}

pub(crate) fn union_rects(values: impl IntoIterator<Item = LayoutRect>) -> Option<LayoutRect> {
    values.into_iter().reduce(LayoutRect::union)
}

fn rotate_rect_cw(rect: LayoutRect) -> LayoutRect {
    LayoutRect::new(-rect.bottom(), rect.x, rect.height, rect.width)
}

fn scale_rect_x(rect: LayoutRect, scale: f32) -> LayoutRect {
    LayoutRect::new(rect.x * scale, rect.y, rect.width * scale, rect.height)
}

pub(crate) fn milli_to_pixels(value: u32) -> f32 {
    let whole = u16::try_from(value / 1_000).unwrap_or(u16::MAX);
    let fractional = u16::try_from(value % 1_000).unwrap_or(999);
    f32::from(whole) + f32::from(fractional) / 1_000.0
}

pub(crate) fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
