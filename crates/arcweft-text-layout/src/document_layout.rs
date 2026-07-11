//! Font-shaped layout of the canonical resolved-text document.

use std::collections::BTreeMap;

use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRun, ResolvedTextStyle, RichTextInlineDirection,
    RichTextRange, RichTextVerticalLatinMode, RichTextWritingMode, TextFontFamily, TextSlant,
    TextWeight,
};

use crate::{
    GlyphOrientation, GlyphVerticalForm, JlreqStrictness, LayoutPoint, LayoutRect, LayoutSize,
    ShapedTextGlyph, ShapedTextRun, TextLayout, TextLayoutError, TextLayoutGlyph,
    TextLayoutGlyphSource, TextLayoutHash, TextLayoutLine, TextLayoutRequest, TextLayoutRun,
    TextLayoutSourceMap, TextShapeRequest, TextShaper, vertical_clusters::vertical_clusters,
};

/// Shapes and lays out one canonical resolved-text document.
pub fn layout_document<S: TextShaper>(
    document: &ResolvedTextDocument<'_>,
    request: TextLayoutRequest,
    shaper: &mut S,
) -> Result<TextLayout, TextLayoutError<S::Error>> {
    let font_inventory = shaper.font_inventory_hash();
    let mut state = DocumentLayoutState::new(request);
    let logical_ordinals = logical_ordinals(document);

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
        state.place_run(run_index, run, text, &shaped_run, &logical_ordinals);
    }
    state.finish_line();

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
    let hash = layout_hash(document, request, font_inventory.as_bytes(), &state);
    Ok(TextLayout {
        lines: state.lines,
        runs: state.runs,
        glyphs: state.glyphs,
        ruby: Vec::new(),
        bounds: state.bounds,
        source_map,
        hash,
        font_inventory,
    })
}

struct DocumentLayoutState {
    request: TextLayoutRequest,
    horizontal: LayoutPoint,
    vertical_rl: LayoutPoint,
    vertical_lr: LayoutPoint,
    lines: Vec<TextLayoutLine>,
    runs: Vec<TextLayoutRun>,
    glyphs: Vec<TextLayoutGlyph>,
    bounds: Option<LayoutRect>,
    active_line: Option<ActiveLine>,
}

#[derive(Clone, Copy)]
struct ActiveLine {
    glyph_start: usize,
    writing_mode: RichTextWritingMode,
    track_bits: u32,
}

impl DocumentLayoutState {
    fn new(request: TextLayoutRequest) -> Self {
        let right = request.origin.x + request.size.width;
        Self {
            request,
            horizontal: request.origin,
            vertical_rl: LayoutPoint::new(right, request.origin.y),
            vertical_lr: request.origin,
            lines: Vec::new(),
            runs: Vec::new(),
            glyphs: Vec::new(),
            bounds: None,
            active_line: None,
        }
    }

    fn place_run(
        &mut self,
        run_index: usize,
        run: &ResolvedTextRun,
        text: &str,
        shaped: &ShapedTextRun,
        logical_ordinals: &BTreeMap<(usize, usize), u32>,
    ) {
        let glyph_start = self.glyphs.len();
        match run.style().writing_mode() {
            RichTextWritingMode::HorizontalTb => {
                self.place_horizontal(run_index, run, shaped, logical_ordinals);
            }
            RichTextWritingMode::VerticalRl | RichTextWritingMode::VerticalLr => {
                self.place_vertical(run_index, run, text, shaped, logical_ordinals);
            }
        }
        let glyph_end = self.glyphs.len();
        if let Some(bounds) = union_rects(
            self.glyphs[glyph_start..glyph_end]
                .iter()
                .map(|glyph| glyph.ink_bounds),
        ) {
            self.runs.push(TextLayoutRun {
                run_index: saturating_u32(run_index),
                source_range: run.source_range(),
                glyph_range: saturating_u32(glyph_start)..saturating_u32(glyph_end),
                bounds,
                writing_mode: run.style().writing_mode(),
                style: run.style().clone(),
                presentation: run.presentation().clone(),
            });
            self.bounds = Some(
                self.bounds
                    .map_or(bounds, |existing| existing.union(bounds)),
            );
        }
    }

    fn place_horizontal(
        &mut self,
        run_index: usize,
        run: &ResolvedTextRun,
        shaped: &ShapedTextRun,
        logical_ordinals: &BTreeMap<(usize, usize), u32>,
    ) {
        let line_height = milli_to_pixels(run.style().line_height_milli()).max(1.0);
        for cluster in cluster_slices(shaped.glyphs()) {
            let cluster_width = cluster
                .iter()
                .map(|glyph| glyph.advance.width.max(0.0))
                .sum::<f32>();
            let right = self.request.origin.x + self.request.size.width;
            if self.horizontal.x > self.request.origin.x
                && self.horizontal.x + cluster_width > right
            {
                self.horizontal.x = self.request.origin.x;
                self.horizontal.y += line_height;
            }
            self.ensure_line(run.style().writing_mode(), self.horizontal.y);
            for glyph in cluster {
                let origin = LayoutPoint::new(
                    self.horizontal.x + glyph.offset.x,
                    self.horizontal.y + glyph.offset.y,
                );
                let ink_bounds = translate_rect(glyph.ink_bounds, origin.x, origin.y);
                self.glyphs.push(TextLayoutGlyph {
                    run_index: saturating_u32(run_index),
                    source_range: glyph.source_range,
                    line_index: glyph.line_index,
                    cluster_index: glyph.cluster_index,
                    logical_ordinal: logical_ordinal(logical_ordinals, glyph),
                    origin,
                    advance: glyph.advance,
                    ink_bounds,
                    orientation: GlyphOrientation::Upright,
                    vertical_form: GlyphVerticalForm::None,
                    shape_key: glyph.key,
                });
                self.horizontal.x += glyph.advance.width.max(0.0);
            }
        }
    }

    fn place_vertical(
        &mut self,
        run_index: usize,
        run: &ResolvedTextRun,
        text: &str,
        shaped: &ShapedTextRun,
        logical_ordinals: &BTreeMap<(usize, usize), u32>,
    ) {
        let writing_mode = run.style().writing_mode();
        let font_size = milli_to_pixels(run.style().font_size_milli()).max(1.0);
        let column_advance = milli_to_pixels(run.style().line_height_milli()).max(1.0);
        let clusters = vertical_clusters(text, RichTextVerticalLatinMode::Mixed);
        for cluster_glyphs in cluster_slices(shaped.glyphs()) {
            let source_start = cluster_glyphs[0]
                .source_range
                .start
                .saturating_sub(run.source_range().start);
            let source_end = cluster_glyphs
                .iter()
                .map(|glyph| {
                    glyph
                        .source_range
                        .end
                        .saturating_sub(run.source_range().start)
                })
                .max()
                .unwrap_or(source_start);
            let orientation = clusters
                .iter()
                .find(|cluster| {
                    cluster.range.start < source_end && source_start < cluster.range.end
                })
                .map_or(GlyphOrientation::Upright, |cluster| cluster.orientation);
            let vertical_form = clusters
                .iter()
                .find(|cluster| {
                    cluster.range.start < source_end && source_start < cluster.range.end
                })
                .map_or(GlyphVerticalForm::None, |cluster| cluster.vertical_form);
            let shaped_advance = cluster_glyphs
                .iter()
                .map(|glyph| glyph.advance.width.max(0.0))
                .sum::<f32>();
            let inline_advance = match orientation {
                GlyphOrientation::SidewaysCw => shaped_advance.max(font_size),
                GlyphOrientation::Upright | GlyphOrientation::TextCombineUpright => font_size,
            };
            let bottom = self.request.origin.y + self.request.size.height;
            let (cursor_x, cursor_y) = {
                let cursor = match writing_mode {
                    RichTextWritingMode::VerticalRl => &mut self.vertical_rl,
                    RichTextWritingMode::VerticalLr => &mut self.vertical_lr,
                    RichTextWritingMode::HorizontalTb => {
                        unreachable!("vertical placement selected")
                    }
                };
                if cursor.y > self.request.origin.y && cursor.y + inline_advance > bottom {
                    cursor.y = self.request.origin.y;
                    match writing_mode {
                        RichTextWritingMode::VerticalRl => cursor.x -= column_advance,
                        RichTextWritingMode::VerticalLr => cursor.x += column_advance,
                        RichTextWritingMode::HorizontalTb => {
                            unreachable!("vertical placement selected")
                        }
                    }
                }
                (cursor.x, cursor.y)
            };
            self.ensure_line(writing_mode, cursor_x);
            for glyph in cluster_glyphs {
                let origin = LayoutPoint::new(cursor_x + glyph.offset.x, cursor_y + glyph.offset.y);
                let ink_bounds = translate_rect(glyph.ink_bounds, origin.x, origin.y);
                self.glyphs.push(TextLayoutGlyph {
                    run_index: saturating_u32(run_index),
                    source_range: glyph.source_range,
                    line_index: glyph.line_index,
                    cluster_index: glyph.cluster_index,
                    logical_ordinal: logical_ordinal(logical_ordinals, glyph),
                    origin,
                    advance: LayoutSize::new(0.0, inline_advance),
                    ink_bounds,
                    orientation,
                    vertical_form,
                    shape_key: glyph.key,
                });
            }
            match writing_mode {
                RichTextWritingMode::VerticalRl => self.vertical_rl.y += inline_advance,
                RichTextWritingMode::VerticalLr => self.vertical_lr.y += inline_advance,
                RichTextWritingMode::HorizontalTb => unreachable!("vertical placement selected"),
            }
        }
    }

    fn ensure_line(&mut self, writing_mode: RichTextWritingMode, track: f32) {
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
            });
        }
    }

    fn finish_line(&mut self) {
        if let Some(line) = self.active_line.take() {
            self.push_line(line.glyph_start, line.writing_mode);
        }
    }

    fn push_line(&mut self, glyph_start: usize, writing_mode: RichTextWritingMode) {
        let glyph_end = self.glyphs.len();
        let Some(bounds) = union_rects(
            self.glyphs[glyph_start..glyph_end]
                .iter()
                .map(|glyph| glyph.ink_bounds),
        ) else {
            return;
        };
        let source_start = self.glyphs[glyph_start..glyph_end]
            .iter()
            .map(|glyph| glyph.source_range.start)
            .min()
            .unwrap_or(0);
        let source_end = self.glyphs[glyph_start..glyph_end]
            .iter()
            .map(|glyph| glyph.source_range.end)
            .max()
            .unwrap_or(source_start);
        self.lines.push(TextLayoutLine {
            source_range: RichTextRange::new(source_start, source_end),
            glyph_range: saturating_u32(glyph_start)..saturating_u32(glyph_end),
            bounds,
            writing_mode,
        });
    }
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
            return Err(TextLayoutError::InvalidShapedGeometry {
                run_index,
                glyph_index,
            });
        }
    }
    Ok(())
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

fn logical_ordinal(ordinals: &BTreeMap<(usize, usize), u32>, glyph: &ShapedTextGlyph) -> u32 {
    ordinals
        .range(..=(glyph.source_range.start, usize::MAX))
        .rev()
        .find(|((start, end), _)| {
            *start <= glyph.source_range.start && glyph.source_range.end <= *end
        })
        .map_or(glyph.cluster_index, |(_, ordinal)| *ordinal)
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

fn layout_hash(
    document: &ResolvedTextDocument<'_>,
    request: TextLayoutRequest,
    inventory: [u8; 32],
    state: &DocumentLayoutState,
) -> TextLayoutHash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.text-layout.v1\0");
    put_bytes(&mut hasher, document.text().as_bytes());
    hasher.update(&document.revision().get().to_le_bytes());
    hasher.update(&inventory);
    for value in [
        request.origin.x,
        request.origin.y,
        request.size.width,
        request.size.height,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    hasher.update(&[writing_mode_tag(request.default_writing_mode)]);
    hasher.update(&[jlreq_strictness_tag(request.jlreq_strictness)]);
    for run in document.runs() {
        hash_range(&mut hasher, run.source_range());
        hash_layout_style(&mut hasher, run.style());
    }
    for glyph in &state.glyphs {
        hasher.update(&glyph.run_index.to_le_bytes());
        hash_range(&mut hasher, glyph.source_range);
        hasher.update(&glyph.line_index.to_le_bytes());
        hasher.update(&glyph.cluster_index.to_le_bytes());
        hasher.update(&glyph.logical_ordinal.to_le_bytes());
        for value in [
            glyph.origin.x,
            glyph.origin.y,
            glyph.advance.width,
            glyph.advance.height,
            glyph.ink_bounds.x,
            glyph.ink_bounds.y,
            glyph.ink_bounds.width,
            glyph.ink_bounds.height,
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        hasher.update(&glyph.shape_key.face.as_bytes());
        hasher.update(&glyph.shape_key.glyph_id.to_le_bytes());
        hasher.update(&glyph.shape_key.font_size_bits.to_le_bytes());
        hasher.update(&glyph.shape_key.font_weight.to_le_bytes());
        hasher.update(&glyph.shape_key.flags.to_le_bytes());
    }
    TextLayoutHash::from_bytes(*hasher.finalize().as_bytes())
}

fn hash_layout_style(hasher: &mut blake3::Hasher, style: &ResolvedTextStyle) {
    hasher.update(&saturating_u32(style.font_families().len()).to_le_bytes());
    for family in style.font_families() {
        match family {
            TextFontFamily::Serif => {
                hasher.update(&[0]);
            }
            TextFontFamily::SansSerif => {
                hasher.update(&[1]);
            }
            TextFontFamily::Monospace => {
                hasher.update(&[2]);
            }
            TextFontFamily::Cursive => {
                hasher.update(&[3]);
            }
            TextFontFamily::Fantasy => {
                hasher.update(&[4]);
            }
            TextFontFamily::Named(name) => {
                hasher.update(&[5]);
                put_bytes(hasher, name.as_bytes());
            }
        }
    }
    hasher.update(&style.font_size_milli().to_le_bytes());
    hasher.update(&style.line_height_milli().to_le_bytes());
    hasher.update(&[match style.weight() {
        TextWeight::Thin => 0,
        TextWeight::ExtraLight => 1,
        TextWeight::Light => 2,
        TextWeight::Normal => 3,
        TextWeight::Medium => 4,
        TextWeight::SemiBold => 5,
        TextWeight::Bold => 6,
        TextWeight::ExtraBold => 7,
        TextWeight::Black => 8,
    }]);
    match style.slant() {
        TextSlant::Upright => {
            hasher.update(&[0]);
        }
        TextSlant::Italic => {
            hasher.update(&[1]);
        }
        TextSlant::Oblique { angle } => {
            hasher.update(&[2]);
            hasher.update(&angle.degrees.0.to_le_bytes());
        }
    }
    hasher.update(&style.letter_spacing_milli().to_le_bytes());
    hasher.update(&style.word_spacing_milli().to_le_bytes());
    hasher.update(&[writing_mode_tag(style.writing_mode())]);
    hasher.update(&[direction_tag(style.direction())]);
    if let Some(language) = style.language() {
        hasher.update(&[1]);
        put_bytes(hasher, language.as_str().as_bytes());
    } else {
        hasher.update(&[0]);
    }
}

fn writing_mode_tag(value: RichTextWritingMode) -> u8 {
    match value {
        RichTextWritingMode::HorizontalTb => 0,
        RichTextWritingMode::VerticalRl => 1,
        RichTextWritingMode::VerticalLr => 2,
    }
}

fn direction_tag(value: RichTextInlineDirection) -> u8 {
    match value {
        RichTextInlineDirection::Auto => 0,
        RichTextInlineDirection::Ltr => 1,
        RichTextInlineDirection::Rtl => 2,
    }
}

fn jlreq_strictness_tag(value: JlreqStrictness) -> u8 {
    match value {
        JlreqStrictness::Loose => 0,
        JlreqStrictness::Normal => 1,
        JlreqStrictness::Strict => 2,
    }
}

fn hash_range(hasher: &mut blake3::Hasher, range: RichTextRange) {
    hasher.update(&saturating_u64(range.start).to_le_bytes());
    hasher.update(&saturating_u64(range.end).to_le_bytes());
}

fn put_bytes(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&saturating_u64(value.len()).to_le_bytes());
    hasher.update(value);
}

fn translate_rect(rect: LayoutRect, x: f32, y: f32) -> LayoutRect {
    LayoutRect::new(x + rect.x, y + rect.y, rect.width, rect.height)
}

fn union_rects(values: impl IntoIterator<Item = LayoutRect>) -> Option<LayoutRect> {
    values.into_iter().reduce(LayoutRect::union)
}

fn milli_to_pixels(value: u32) -> f32 {
    let whole = u16::try_from(value / 1_000).unwrap_or(u16::MAX);
    let fractional = u16::try_from(value % 1_000).unwrap_or(999);
    f32::from(whole) + f32::from(fractional) / 1_000.0
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
