//! Source-to-document lowering for text that enters the canonical prepared batch.

use arcweft_glyphon::{GlyphonTextEngine, PreparedTextItem, TextInteractionPlan, TextPaintPlan};
use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRun, ResolvedTextRunSource, ResolvedTextStyle,
    RichTextPresentation, RichTextRange, TextColor, TextDocumentRevision, TextFontFamily,
    TextSlant, TextWeight,
};
use arcweft_text_layout::{LayoutRect, TextLayoutRequest, layout_document};

use super::{FramePlanError, PreparedTextDocumentRequest, RenderViewport};

pub(super) fn prepare_plain_text(
    engine: &mut GlyphonTextEngine,
    text: &str,
    style: ResolvedTextStyle,
    source: ResolvedTextRunSource,
    request: &PreparedTextDocumentRequest,
    viewport: RenderViewport,
) -> Result<PreparedTextItem, FramePlanError> {
    let runs = if text.is_empty() {
        Vec::new()
    } else {
        let range = RichTextRange::new(0, text.len());
        vec![ResolvedTextRun::new(
            range,
            range,
            style,
            RichTextPresentation::default(),
            source,
        )?]
    };
    let document =
        ResolvedTextDocument::new(text, 0, runs, Vec::new(), TextDocumentRevision::new(0))?;
    prepare_text_document(engine, &document, request, viewport)
}

pub(super) fn prepare_text_document(
    engine: &mut GlyphonTextEngine,
    document: &ResolvedTextDocument<'_>,
    request: &PreparedTextDocumentRequest,
    viewport: RenderViewport,
) -> Result<PreparedTextItem, FramePlanError> {
    let layout = layout_document(
        document,
        TextLayoutRequest {
            origin: request.origin,
            size: request.size,
            ..TextLayoutRequest::default()
        },
        engine,
    )?;
    let paint = TextPaintPlan::from_layout(&layout);
    let mut interaction = TextInteractionPlan::from_layout(&layout, request.target.clone())
        .with_text_and_selection_color(document.text().to_owned(), request.selection_rgba)
        .with_container_bounds(request.container_bounds)
        .with_selection_enabled(request.selection_enabled);
    if request.selection_enabled
        && let Some(selection) = request.selection
    {
        interaction = interaction.with_selection(selection);
    }
    engine
        .prepare_text_item(
            layout,
            paint,
            interaction,
            request.clip,
            viewport.physical_scale_factor_f32(),
        )
        .map_err(FramePlanError::from)
}

pub(super) fn resolved_plain_style(
    font_families: Vec<TextFontFamily>,
    font_size: f32,
    line_height: f32,
    weight: TextWeight,
    slant: TextSlant,
    rgba: [u8; 4],
) -> Result<ResolvedTextStyle, FramePlanError> {
    Ok(ResolvedTextStyle::new(
        font_families,
        pixels_to_milli("font_size", font_size)?,
        pixels_to_milli("line_height", line_height)?,
    )?
    .with_weight(weight)
    .with_slant(slant)
    .with_color(TextColor::rgba(rgba[0], rgba[1], rgba[2], rgba[3])))
}

pub(super) fn font_families_from_stack(stack: Option<&str>) -> Vec<TextFontFamily> {
    let Some(stack) = stack else {
        return vec![TextFontFamily::SansSerif];
    };
    let families = stack
        .split(',')
        .map(|family| family.trim().trim_matches('"').trim_matches('\'').trim())
        .filter(|family| !family.is_empty())
        .map(|family| match family.to_ascii_lowercase().as_str() {
            "serif" => TextFontFamily::Serif,
            "sans-serif" | "sans_serif" => TextFontFamily::SansSerif,
            "monospace" => TextFontFamily::Monospace,
            "cursive" => TextFontFamily::Cursive,
            "fantasy" => TextFontFamily::Fantasy,
            _ => TextFontFamily::Named(family.to_owned()),
        })
        .collect::<Vec<_>>();
    if families.is_empty() {
        vec![TextFontFamily::SansSerif]
    } else {
        families
    }
}

pub(super) fn pixels_to_milli(field: &'static str, value: f32) -> Result<u32, FramePlanError> {
    if !value.is_finite() || value <= 0.0 || value > 65_535.0 {
        return Err(FramePlanError::InvalidTextMetric { field });
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok((value * 1_000.0).round() as u32)
}

pub(super) fn hit_rect_to_layout_rect(rect: arcweft_presentation::hit::HitRect) -> LayoutRect {
    LayoutRect::new(rect.x, rect.y, rect.width, rect.height)
}
