//! Ordinary text-block lowering into the canonical prepared batch contract.

use arcweft_glyphon::{GlyphonTextEngine, PreparedTextItem, TextInteractionPlan, TextPaintPlan};
use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRun, ResolvedTextRunSource, ResolvedTextStyle,
    RichTextPresentation, RichTextRange, TextColor, TextDocumentRevision, TextFontFamily,
    TextSlant, TextWeight,
};
use arcweft_text_layout::{
    LayoutPoint, LayoutRect, LayoutSize, TextLayoutRequest, layout_document,
};

use super::{
    FramePlanError, RenderFontFamily, RenderTextBlock, RenderTextSlant, RenderTextWeight,
    RenderViewport,
};

pub(super) fn prepare_text_block(
    engine: &mut GlyphonTextEngine,
    block: &RenderTextBlock,
    viewport: RenderViewport,
) -> Result<PreparedTextItem, FramePlanError> {
    let font_size_milli = pixels_to_milli("font_size", block.font_size)?;
    let line_height_milli = pixels_to_milli("line_height", block.line_height)?;
    let style = ResolvedTextStyle::new(
        resolved_families(&block.font_family),
        font_size_milli,
        line_height_milli,
    )?
    .with_weight(match block.weight {
        RenderTextWeight::Regular => TextWeight::Normal,
        RenderTextWeight::Bold => TextWeight::Bold,
    })
    .with_slant(match block.slant {
        RenderTextSlant::Upright => TextSlant::Upright,
        RenderTextSlant::Italic => TextSlant::Italic,
    })
    .with_color(TextColor::rgba(
        block.rgba[0],
        block.rgba[1],
        block.rgba[2],
        block.rgba[3],
    ));
    let runs = if block.text.is_empty() {
        Vec::new()
    } else {
        let range = RichTextRange::new(0, block.text.len());
        vec![ResolvedTextRun::new(
            range,
            range,
            style,
            RichTextPresentation::default(),
            ResolvedTextRunSource::Plain,
        )?]
    };
    let document = ResolvedTextDocument::new(
        &block.text,
        0,
        runs,
        Vec::new(),
        TextDocumentRevision::new(0),
    )?;
    let request = TextLayoutRequest {
        origin: LayoutPoint::new(block.bounds.x, block.bounds.y),
        size: LayoutSize::new(
            block.buffer_width.unwrap_or(block.bounds.width),
            block.buffer_height.unwrap_or(block.bounds.height),
        ),
        ..TextLayoutRequest::default()
    };
    let layout = layout_document(&document, request, engine)?;
    let paint = TextPaintPlan::from_layout(&layout);
    let mut interaction = TextInteractionPlan::from_layout(&layout, block.target.clone())
        .with_text_and_selection_color(block.text.clone(), block.selection_rgba)
        .with_container_bounds(hit_rect_to_layout_rect(block.bounds))
        .with_selection_enabled(block.selection_policy.enabled());
    if block.selection_policy.enabled()
        && let Some(selection) = block.selection
    {
        interaction = interaction.with_selection(RichTextRange::new(
            usize::try_from(selection.start().get()).unwrap_or(usize::MAX),
            usize::try_from(selection.end().get()).unwrap_or(usize::MAX),
        ));
    }
    let clip = block.clip_bounds.map(hit_rect_to_layout_rect);
    engine
        .prepare_text_item(
            layout,
            paint,
            interaction,
            clip,
            viewport.physical_scale_factor_f32(),
        )
        .map_err(FramePlanError::from)
}

fn resolved_families(family: &RenderFontFamily) -> Vec<TextFontFamily> {
    match family {
        RenderFontFamily::Serif => vec![TextFontFamily::Serif],
        RenderFontFamily::SansSerif => vec![TextFontFamily::SansSerif],
        RenderFontFamily::Monospace => vec![TextFontFamily::Monospace],
        RenderFontFamily::Cursive => vec![TextFontFamily::Cursive],
        RenderFontFamily::Fantasy => vec![TextFontFamily::Fantasy],
        RenderFontFamily::Named(name) => vec![TextFontFamily::Named(name.clone())],
        RenderFontFamily::Stack(families) => families
            .iter()
            .cloned()
            .map(TextFontFamily::Named)
            .collect(),
    }
}

fn pixels_to_milli(field: &'static str, value: f32) -> Result<u32, FramePlanError> {
    if !value.is_finite() || value <= 0.0 || value > 65_535.0 {
        return Err(FramePlanError::InvalidTextMetric { field });
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok((value * 1_000.0).round() as u32)
}

fn hit_rect_to_layout_rect(rect: arcweft_presentation::hit::HitRect) -> LayoutRect {
    LayoutRect::new(rect.x, rect.y, rect.width, rect.height)
}
