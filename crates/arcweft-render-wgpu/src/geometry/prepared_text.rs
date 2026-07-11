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
    FramePlanError, RenderFontFamily, RenderTextBlock, RenderTextSlant, RenderTextStyle,
    RenderTextWeight, RenderViewport,
};

pub(super) fn prepare_text_block(
    engine: &mut GlyphonTextEngine,
    block: &RenderTextBlock,
    viewport: RenderViewport,
) -> Result<PreparedTextItem, FramePlanError> {
    let style = resolved_style(&RenderTextStyle {
        font_size: block.font_size,
        line_height: block.line_height,
        color: block.rgba,
        font_family: block.font_family.clone(),
        weight: block.weight,
        slant: block.slant,
    })?;
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

pub(super) fn resolved_style(style: &RenderTextStyle) -> Result<ResolvedTextStyle, FramePlanError> {
    Ok(ResolvedTextStyle::new(
        resolved_families(&style.font_family),
        pixels_to_milli("font_size", style.font_size)?,
        pixels_to_milli("line_height", style.line_height)?,
    )?
    .with_weight(match style.weight {
        RenderTextWeight::Regular => TextWeight::Normal,
        RenderTextWeight::Bold => TextWeight::Bold,
    })
    .with_slant(match style.slant {
        RenderTextSlant::Upright => TextSlant::Upright,
        RenderTextSlant::Italic => TextSlant::Italic,
    })
    .with_color(TextColor::rgba(
        style.color[0],
        style.color[1],
        style.color[2],
        style.color[3],
    )))
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

pub(super) fn hit_rect_to_layout_rect(rect: arcweft_presentation::hit::HitRect) -> LayoutRect {
    LayoutRect::new(rect.x, rect.y, rect.width, rect.height)
}
