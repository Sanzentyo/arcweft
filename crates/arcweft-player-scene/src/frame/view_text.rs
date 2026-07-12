//! Canonical View text-source resolution and frame-local preparation.

use super::{milli_i32_to_f32, milli_u32_to_f32, scroll_adjusted_bounds};
use crate::input::InputController;
use arcweft_bundle::resource_codec::view::{
    RgbaColor, ViewRuntimeControlState, ViewTextSelectionPolicy,
};
use arcweft_id::PublicId;
use arcweft_layout::{ContentRect, LayoutRect as FitLayoutRect};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRun, ResolvedTextRunSource, ResolvedTextStyle,
    RichTextPresentation, RichTextRange, TextColor, TextDocumentRevision, TextFontFamily,
    TextSlant, TextStyleCascade, TextWeight,
};
use arcweft_render_wgpu::geometry::{
    FramePlanError, PreparedFrame, PreparedTextDocumentRequest, RenderScene, SharedFramePlanContext,
};
use arcweft_render_wgpu::view_scene::PreparedTextId;
use arcweft_runtime_driver::view_runtime::{
    BundleViewFrame, BundleViewMountOutput, BundleViewTextTarget, BundleViewTextValue,
};
use arcweft_text_layout::{LayoutPoint, LayoutRect, LayoutSize};

/// Prepared ID associated with one exact mounted View text paint target.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PreparedMountedViewText {
    pub(super) mount: u64,
    pub(super) source_id: String,
    pub(super) target: String,
    pub(super) text: PreparedTextId,
    pub(super) bounds: HitRect,
    pub(super) clip: Option<HitRect>,
}

pub(super) fn prepare_runtime_view_text(
    shared: &mut SharedFramePlanContext,
    frame: &mut PreparedFrame,
    input: &InputController,
    scene: &RenderScene,
    view: &BundleViewFrame,
    content: Option<ContentRect>,
) -> Result<Vec<PreparedMountedViewText>, FramePlanError> {
    let mut prepared = Vec::new();
    for mount in &view.mounts {
        for output in &mount.text {
            for target in &output.targets {
                if !mount
                    .active_targets
                    .iter()
                    .any(|active| active == &target.public_id)
                {
                    continue;
                }
                let Some((bounds, clip)) = target_geometry(scene, mount, target, content) else {
                    continue;
                };
                let interaction_target = PublicId::try_new(mount.scoped_id(&target.public_id))
                    .ok()
                    .map(InteractionTarget::new);
                let visible_text = visible_text(&output.value)?;
                let selection = interaction_target
                    .as_ref()
                    .and_then(|target| input.text_block_selection_for(target, visible_text))
                    .map(|selection| {
                        RichTextRange::new(
                            usize::try_from(selection.start().get()).unwrap_or(usize::MAX),
                            usize::try_from(selection.end().get()).unwrap_or(usize::MAX),
                        )
                    });
                let visual = target
                    .style
                    .visual_for_state(ViewRuntimeControlState::Normal);
                let fit_scale = content.map_or(1.0, |content| {
                    ((content.scale_x.abs() + content.scale_y.abs()) * 0.5).max(f32::EPSILON)
                });
                let text_scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
                let style = resolved_style(&visual, fit_scale * text_scale)?;
                let request = PreparedTextDocumentRequest {
                    origin: LayoutPoint::new(bounds.x, bounds.y),
                    size: LayoutSize::new(bounds.width, bounds.height),
                    container_bounds: LayoutRect::new(
                        bounds.x,
                        bounds.y,
                        bounds.width,
                        bounds.height,
                    ),
                    clip: clip.map(layout_rect),
                    target: interaction_target,
                    selection_enabled: target.selection_policy == ViewTextSelectionPolicy::Enabled,
                    selection,
                    selection_rgba: rgba_f32(
                        visual
                            .selection
                            .unwrap_or(RgbaColor::rgba(64, 128, 255, 90)),
                    ),
                };
                let prepared_id = push_text_value(shared, frame, &output.value, style, &request)?;
                prepared.push(PreparedMountedViewText {
                    mount: mount.mount.get(),
                    source_id: output.source_id.clone(),
                    target: target.public_id.clone(),
                    text: prepared_id,
                    bounds,
                    clip,
                });
            }
        }
    }
    Ok(prepared)
}

fn push_text_value(
    shared: &mut SharedFramePlanContext,
    frame: &mut PreparedFrame,
    value: &BundleViewTextValue,
    style: ResolvedTextStyle,
    request: &PreparedTextDocumentRequest,
) -> Result<PreparedTextId, FramePlanError> {
    let cascade = TextStyleCascade::new(style.clone());
    match value {
        BundleViewTextValue::Plain { value } => {
            let document = plain_document(value, style)?;
            shared.push_prepared_text_document(frame, &document, request)
        }
        BundleViewTextValue::Localized { document, .. } => {
            let document = document
                .resolve_document_with_source(&cascade, ResolvedTextRunSource::Localized)?;
            shared.push_prepared_text_document(frame, &document, request)
        }
        BundleViewTextValue::RichTextDocument { document } => {
            let document = document.resolve_document(&cascade)?;
            shared.push_prepared_text_document(frame, &document, request)
        }
        BundleViewTextValue::DisplayFrame {
            frame: display,
            stage_index,
        } => {
            let stage_index = usize::try_from(*stage_index).map_err(|_| {
                FramePlanError::ResolveText(
                    arcweft_render_text::TextResolveError::InvalidDisplayStage {
                        index: usize::MAX,
                    },
                )
            })?;
            let stage = display
                .stage(stage_index)
                .ok_or(FramePlanError::ResolveText(
                    arcweft_render_text::TextResolveError::InvalidDisplayStage {
                        index: stage_index,
                    },
                ))?;
            let document = display.resolve_stage_document(stage, &cascade)?;
            shared.push_prepared_text_document(frame, &document, request)
        }
    }
}

fn target_geometry(
    scene: &RenderScene,
    mount: &BundleViewMountOutput,
    target: &BundleViewTextTarget,
    content: Option<ContentRect>,
) -> Option<(HitRect, Option<HitRect>)> {
    let bounds = HitRect::new(
        milli_i32_to_f32(target.bounds.x_milli),
        milli_i32_to_f32(target.bounds.y_milli),
        milli_u32_to_f32(target.bounds.width_milli),
        milli_u32_to_f32(target.bounds.height_milli),
    );
    let scroll = target.containing_scroll_region.as_deref().map(|region| {
        let scoped = mount.scoped_id(region);
        if scene
            .scroll_regions
            .iter()
            .any(|candidate| candidate.id == scoped)
        {
            scoped
        } else {
            region.to_owned()
        }
    });
    let (bounds, clip) = scroll_adjusted_bounds(scene, scroll.as_deref(), bounds)?;
    Some((
        map_rect(bounds, content),
        clip.map(|clip| map_rect(clip, content)),
    ))
}

fn visible_text(value: &BundleViewTextValue) -> Result<&str, FramePlanError> {
    match value {
        BundleViewTextValue::Plain { value } => Ok(value),
        BundleViewTextValue::Localized { document, .. }
        | BundleViewTextValue::RichTextDocument { document } => Ok(document.resolved_text()),
        BundleViewTextValue::DisplayFrame { frame, stage_index } => {
            let index = usize::try_from(*stage_index).map_err(|_| {
                FramePlanError::ResolveText(
                    arcweft_render_text::TextResolveError::InvalidDisplayStage {
                        index: usize::MAX,
                    },
                )
            })?;
            frame
                .stage(index)
                .map(arcweft_render_text::LineDisplayStage::text)
                .ok_or(FramePlanError::ResolveText(
                    arcweft_render_text::TextResolveError::InvalidDisplayStage { index },
                ))
        }
    }
}

fn plain_document(
    text: &str,
    style: ResolvedTextStyle,
) -> Result<ResolvedTextDocument<'_>, FramePlanError> {
    let runs = if text.is_empty() {
        Vec::new()
    } else {
        let range = RichTextRange::new(0, text.len());
        vec![ResolvedTextRun::new(
            range,
            range,
            style,
            RichTextPresentation::default(),
            ResolvedTextRunSource::Plain,
        )?]
    };
    Ok(ResolvedTextDocument::new(
        text,
        0,
        runs,
        Vec::new(),
        TextDocumentRevision::new(0),
    )?)
}

fn resolved_style(
    visual: &arcweft_bundle::resource_codec::view::ViewRuntimeControlVisualStyle,
    scale: f32,
) -> Result<ResolvedTextStyle, FramePlanError> {
    let font_size = scaled_milli("font_size", visual.font_size_milli.unwrap_or(20_000), scale)?;
    let line_height = scaled_milli(
        "line_height",
        visual
            .line_height_milli
            .unwrap_or_else(|| visual.font_size_milli.unwrap_or(20_000).saturating_mul(6) / 5),
        scale,
    )?;
    let color = visual.text.unwrap_or(RgbaColor::rgb(245, 245, 245));
    Ok(ResolvedTextStyle::new(
        font_families(visual.font_family.as_deref()),
        font_size,
        line_height,
    )?
    .with_weight(text_weight(visual.font_weight.unwrap_or(400)))
    .with_slant(TextSlant::Upright)
    .with_color(TextColor::rgba(
        color.red,
        color.green,
        color.blue,
        color.alpha,
    )))
}

fn font_families(value: Option<&str>) -> Vec<TextFontFamily> {
    let Some(value) = value else {
        return vec![TextFontFamily::SansSerif];
    };
    let families = value
        .split(',')
        .map(str::trim)
        .map(|family| family.trim_matches(['"', '\'']))
        .filter(|family| !family.is_empty())
        .map(|family| match family.to_ascii_lowercase().as_str() {
            "serif" => TextFontFamily::Serif,
            "sans-serif" | "sans serif" => TextFontFamily::SansSerif,
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

const fn text_weight(value: u16) -> TextWeight {
    match value {
        0..=149 => TextWeight::Thin,
        150..=249 => TextWeight::ExtraLight,
        250..=349 => TextWeight::Light,
        350..=449 => TextWeight::Normal,
        450..=549 => TextWeight::Medium,
        550..=649 => TextWeight::SemiBold,
        650..=749 => TextWeight::Bold,
        750..=849 => TextWeight::ExtraBold,
        _ => TextWeight::Black,
    }
}

fn scaled_milli(field: &'static str, value: u32, scale: f32) -> Result<u32, FramePlanError> {
    let value = f64::from(value) * f64::from(scale);
    if !value.is_finite() || value <= 0.0 || value > f64::from(u32::MAX) {
        return Err(FramePlanError::InvalidTextMetric { field });
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(value.round() as u32)
}

fn map_rect(rect: HitRect, content: Option<ContentRect>) -> HitRect {
    let Some(content) = content else {
        return rect;
    };
    let mapped = content.map_rect(FitLayoutRect::from_xywh(
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    ));
    HitRect::new(
        mapped.origin.x,
        mapped.origin.y,
        mapped.size.width,
        mapped.size.height,
    )
}

const fn layout_rect(rect: HitRect) -> LayoutRect {
    LayoutRect::new(rect.x, rect.y, rect.width, rect.height)
}

fn rgba_f32(color: RgbaColor) -> [f32; 4] {
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        f32::from(color.alpha) / 255.0,
    ]
}
