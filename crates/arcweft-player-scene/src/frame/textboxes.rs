//! Rust-backed standard `TextBox` View preparation.

use super::view_text::{map_rect, plain_document};
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_id::PublicId;
use arcweft_layout::ContentRect;
use arcweft_presentation::fx::{
    FxApplication, FxApplicationResolver, FxDiagnostic, FxDiagnosticCode, FxDiagnosticContext,
    FxEvaluationBinding,
};
use arcweft_presentation::hit::HitRect;
use arcweft_render_text::{
    ResolvedTextStyle, RichTextInlineDirection, RichTextWritingMode, TextColor, TextFontFamily,
    TextStyleCascade, TextWeight,
};
use arcweft_render_wgpu::geometry::{
    FramePlanError, PreparedFrame, PreparedRichTextStageRequest, PreparedTextBoxPart,
    PreparedTextBoxState, PreparedTextDocumentRequest, PreparedTextOwner, PreparedTextOwnerKind,
    PreparedViewScene, RenderPreferences, RenderScene, RenderViewport, SharedFramePlanContext,
};
use arcweft_render_wgpu::view_scene::{
    PreparedTextId, ViewAffine2D, ViewClip, ViewColorRgba8, ViewCornerRadii, ViewPrimitive,
    ViewPrimitiveRange, ViewScene, ViewSceneContext, ViewSurfaceBackground, ViewSurfacePaint,
    ViewTextPrimitive,
};
use arcweft_runtime_driver::dialogue::{TextBoxEntryState, TextBoxRuntimeId};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_text_layout::{LayoutPoint, LayoutRect, LayoutSize};
use num_traits::ToPrimitive;

const PANEL_INSET: f32 = 28.0;
const PANEL_GAP: f32 = 12.0;

struct TextBoxFxResolver<'a> {
    textbox: TextBoxRuntimeId,
    entry: &'a TextBoxEntryState,
    definitions: &'a FxDefinitions,
    runtime: &'a arcweft_runtime_driver::fx_runtime::BundleFxRuntimeSnapshot,
}

impl FxApplicationResolver for TextBoxFxResolver<'_> {
    fn resolve<'a>(
        &'a self,
        application: &FxApplication,
    ) -> Result<FxEvaluationBinding<'a>, Box<FxDiagnostic>> {
        let instance_id = self.entry.fx_instance_id(self.textbox, application);
        let context = FxDiagnosticContext {
            definition: Some(application.definition().clone()),
            instance: Some(instance_id),
            source_range: application.source_range(),
            ..FxDiagnosticContext::default()
        };
        let definition = self
            .definitions
            .get(application.definition())
            .ok_or_else(|| {
                Box::new(FxDiagnostic::error(
                    FxDiagnosticCode::MissingDefinition,
                    context.clone(),
                    format!(
                        "bundle has no definition `{}` for RichText application",
                        application.definition()
                    ),
                ))
            })?;
        let instance = self.runtime.instance(instance_id).ok_or_else(|| {
            Box::new(FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                context,
                "runtime did not retain the RichText Fx application instance",
            ))
        })?;
        Ok(FxEvaluationBinding {
            definition,
            instance,
            runtime_time: self.runtime.logical_time,
        })
    }
}

pub(super) struct TextBoxViewFrameRequest<'a> {
    scene: &'a RenderScene,
    presentation: &'a BundlePresentationSnapshot,
    fx_definitions: &'a FxDefinitions,
    visual_time_millis: u64,
    latest_reveal_complete: bool,
    content: Option<ContentRect>,
}

impl<'a> TextBoxViewFrameRequest<'a> {
    pub(super) const fn new(
        scene: &'a RenderScene,
        presentation: &'a BundlePresentationSnapshot,
        fx_definitions: &'a FxDefinitions,
        visual_time_millis: u64,
        latest_reveal_complete: bool,
        content: Option<ContentRect>,
    ) -> Self {
        Self {
            scene,
            presentation,
            fx_definitions,
            visual_time_millis,
            latest_reveal_complete,
            content,
        }
    }
}

#[derive(Clone, Copy)]
struct TextBoxGeometry {
    panel: HitRect,
    speaker: HitRect,
    body: HitRect,
    metric_scale: f32,
}

impl TextBoxGeometry {
    fn new(
        design_panel: HitRect,
        preferences: RenderPreferences,
        content: Option<ContentRect>,
    ) -> Self {
        Self {
            panel: map_rect(design_panel, content),
            speaker: map_rect(speaker_bounds(design_panel, preferences), content),
            body: map_rect(body_bounds(design_panel), content),
            metric_scale: content.map_or(1.0, |content| {
                ((content.scale_x.abs() + content.scale_y.abs()) * 0.5).max(f32::EPSILON)
            }),
        }
    }
}

pub(super) fn push_textbox_views(
    shared: &mut SharedFramePlanContext,
    frame: &mut PreparedFrame,
    request: &TextBoxViewFrameRequest<'_>,
) -> Result<(), FramePlanError> {
    let active = request
        .presentation
        .textboxes
        .iter()
        .filter_map(|textbox| textbox.active_entry().map(|entry| (textbox, entry)))
        .collect::<Vec<_>>();
    let latest = request
        .presentation
        .textboxes
        .latest_active()
        .map(|(_, entry)| entry.instance());
    let panel_bounds = standard_textbox_bounds(request.scene.viewport, active.len());
    for ((textbox, entry), design_panel) in active.into_iter().zip(panel_bounds) {
        push_textbox_view(shared, frame, request, textbox, entry, design_panel, latest)?;
    }
    Ok(())
}

fn push_textbox_view(
    shared: &mut SharedFramePlanContext,
    frame: &mut PreparedFrame,
    request: &TextBoxViewFrameRequest<'_>,
    textbox: &arcweft_runtime_driver::dialogue::TextBoxPresentation,
    entry: &TextBoxEntryState,
    design_panel: HitRect,
    latest: Option<arcweft_runtime_driver::dialogue::DialogueInstanceId>,
) -> Result<(), FramePlanError> {
    let Some(stage) = entry.current_stage() else {
        return Ok(());
    };
    let geometry = TextBoxGeometry::new(design_panel, request.scene.preferences, request.content);
    let fallback = fallback_body_style(request.scene.preferences, geometry.metric_scale)?;
    let inherited =
        TextStyleCascade::new(fallback.clone()).resolve_style(entry.frame().base_styles.iter())?;
    let label_style = speaker_style(
        inherited,
        request.scene.preferences,
        geometry.metric_scale,
        entry.frame().base_styles.is_empty(),
    )?;
    let parent_id = public_id(format!("view.textbox.{}", textbox.mount().get()))?;
    let speaker_text = prepare_speaker_text(
        shared,
        frame,
        textbox,
        entry,
        geometry,
        label_style,
        &parent_id,
    )?;
    let fx_resolver = TextBoxFxResolver {
        textbox: textbox.id(),
        entry,
        definitions: request.fx_definitions,
        runtime: &request.presentation.fx,
    };
    let body_result = shared.push_prepared_rich_text_stage(
        frame,
        stage,
        &PreparedRichTextStageRequest {
            bounds: geometry.body,
            default_style: fallback,
            visual_time_millis: request.visual_time_millis,
            reveal_complete: latest != Some(entry.instance()) || request.latest_reveal_complete,
        },
        &fx_resolver,
    )?;
    frame.push_prepared_text_owner(
        PreparedTextOwner::new(
            body_result.text,
            text_part_id(textbox, entry, "body")?,
            owner_kind(textbox, entry, PreparedTextBoxPart::Body),
            body_result.source_origin,
            geometry.panel,
        )
        .with_parent(parent_id),
    )?;
    frame.push_view_scene(PreparedViewScene::new(textbox_view_scene(
        frame.viewport,
        geometry.panel,
        geometry.speaker,
        speaker_text,
        geometry.body,
        body_result.text,
        request.scene.preferences,
    )));
    frame.push_textbox(PreparedTextBoxState {
        textbox: textbox.id().get(),
        entry: entry.id().get(),
        mount: textbox.mount().get(),
        revision: textbox.revision().get(),
        instance: entry.instance().get(),
        stage: entry.stage_index().get(),
        bounds: geometry.panel,
        reveal_complete: body_result.reveal_complete,
        advance_available: entry.is_waiting_for_advance(),
    });
    Ok(())
}

fn prepare_speaker_text(
    shared: &mut SharedFramePlanContext,
    frame: &mut PreparedFrame,
    textbox: &arcweft_runtime_driver::dialogue::TextBoxPresentation,
    entry: &TextBoxEntryState,
    geometry: TextBoxGeometry,
    style: ResolvedTextStyle,
    parent_id: &PublicId,
) -> Result<PreparedTextId, FramePlanError> {
    let label = entry
        .frame()
        .speaker_label
        .as_deref()
        .unwrap_or(&entry.frame().callee);
    let document = plain_document(label, style)?;
    let text = shared.push_prepared_text_document(
        frame,
        &document,
        &PreparedTextDocumentRequest {
            origin: LayoutPoint::new(geometry.speaker.x, geometry.speaker.y),
            size: LayoutSize::new(geometry.speaker.width, geometry.speaker.height),
            container_bounds: layout_rect(geometry.speaker),
            clip: Some(layout_rect(geometry.speaker)),
            target: None,
            selection_enabled: false,
            selection: None,
            selection_rgba: [0.0; 4],
        },
    )?;
    frame.push_prepared_text_owner(
        PreparedTextOwner::new(
            text,
            text_part_id(textbox, entry, "speaker")?,
            owner_kind(textbox, entry, PreparedTextBoxPart::Speaker),
            document.source_origin(),
            geometry.panel,
        )
        .with_parent(parent_id.clone()),
    )?;
    Ok(text)
}

fn owner_kind(
    textbox: &arcweft_runtime_driver::dialogue::TextBoxPresentation,
    entry: &TextBoxEntryState,
    part: PreparedTextBoxPart,
) -> PreparedTextOwnerKind {
    PreparedTextOwnerKind::TextBox {
        textbox: textbox.id().get(),
        entry: entry.id().get(),
        mount: textbox.mount().get(),
        part,
    }
}

fn text_part_id(
    textbox: &arcweft_runtime_driver::dialogue::TextBoxPresentation,
    entry: &TextBoxEntryState,
    part: &str,
) -> Result<PublicId, FramePlanError> {
    public_id(format!(
        "textbox.runtime.{}.entry.{}.{part}",
        textbox.id().get(),
        entry.id().get()
    ))
}

fn textbox_view_scene(
    viewport: RenderViewport,
    panel: HitRect,
    speaker_bounds: HitRect,
    speaker_text: PreparedTextId,
    body_bounds: HitRect,
    body_text: PreparedTextId,
    preferences: RenderPreferences,
) -> ViewScene {
    let mut scene = ViewScene::new(viewport.logical_width, viewport.logical_height);
    let panel_paint = ViewSurfacePaint::new().with_background(ViewSurfaceBackground::Solid {
        color: panel_color(preferences),
        radii: ViewCornerRadii::ZERO,
    });
    if let Some(range) = scene.push_surface_primitives(panel, &panel_paint) {
        push_direct(&mut scene, range, None);
    }
    let speaker_range = push_text_primitive(&mut scene, speaker_text);
    push_direct(
        &mut scene,
        speaker_range,
        Some(ViewClip::Rect(speaker_bounds)),
    );
    let body_range = push_text_primitive(&mut scene, body_text);
    push_direct(&mut scene, body_range, Some(ViewClip::Rect(body_bounds)));
    scene
}

fn push_text_primitive(scene: &mut ViewScene, text: PreparedTextId) -> ViewPrimitiveRange {
    let start = u32::try_from(scene.primitives().len()).unwrap_or(u32::MAX);
    scene.push_primitive(ViewPrimitive::Text(ViewTextPrimitive { text }));
    let end = u32::try_from(scene.primitives().len()).unwrap_or(u32::MAX);
    ViewPrimitiveRange { start, end }
}

fn push_direct(scene: &mut ViewScene, range: ViewPrimitiveRange, clip: Option<ViewClip>) {
    scene.push_context(ViewSceneContext {
        transform: ViewAffine2D::IDENTITY,
        opacity: 1.0,
        clip,
        primitive_range: range,
    });
}

pub(super) fn standard_textbox_bounds(viewport: RenderViewport, count: usize) -> Vec<HitRect> {
    if count == 0 {
        return Vec::new();
    }
    let margin = (viewport.logical_width * 0.045).max(24.0);
    let preferred_height = (viewport.logical_height * 0.28).clamp(180.0, 320.0);
    let count_f32 = count
        .to_f32()
        .expect("usize values are inside the finite f32 exponent range");
    let gap_count = count
        .saturating_sub(1)
        .to_f32()
        .expect("usize values are inside the finite f32 exponent range");
    let gaps = PANEL_GAP * gap_count;
    let available_height = (viewport.logical_height - margin * 2.0 - gaps).max(1.0);
    let height = preferred_height.min(available_height / count_f32);
    let total_height = height * count_f32 + gaps;
    let start_y = viewport.logical_height - margin - total_height;
    (0..count)
        .map(|index| {
            let index = index
                .to_f32()
                .expect("usize values are inside the finite f32 exponent range");
            HitRect::new(
                margin,
                start_y + index * (height + PANEL_GAP),
                viewport.logical_width - margin * 2.0,
                height,
            )
        })
        .collect()
}

fn speaker_bounds(panel: HitRect, preferences: RenderPreferences) -> HitRect {
    let scale = f32::from(preferences.text_scale_milli) / 1_000.0;
    HitRect::new(
        panel.x + PANEL_INSET,
        panel.y + 20.0,
        panel.width - PANEL_INSET * 2.0,
        28.0 * scale,
    )
}

fn body_bounds(panel: HitRect) -> HitRect {
    HitRect::new(
        panel.x + PANEL_INSET,
        panel.y + 58.0,
        panel.width - PANEL_INSET * 2.0,
        panel.height - 76.0,
    )
}

fn fallback_body_style(
    preferences: RenderPreferences,
    metric_scale: f32,
) -> Result<ResolvedTextStyle, FramePlanError> {
    let text_scale = f32::from(preferences.text_scale_milli) / 1_000.0;
    let scale = text_scale * metric_scale;
    Ok(ResolvedTextStyle::new(
        vec![TextFontFamily::SansSerif],
        scaled_milli(25.0, scale, "font_size")?,
        scaled_milli(34.0, scale, "line_height")?,
    )?
    .with_color(body_color(preferences)))
}

fn speaker_style(
    inherited: ResolvedTextStyle,
    preferences: RenderPreferences,
    metric_scale: f32,
    use_standard_color: bool,
) -> Result<ResolvedTextStyle, FramePlanError> {
    let text_scale = f32::from(preferences.text_scale_milli) / 1_000.0;
    let minimum_size = scaled_milli(16.0, text_scale * metric_scale, "speaker_font_size")?;
    let minimum_line = scaled_milli(24.0, text_scale * metric_scale, "speaker_line_height")?;
    let font_size = inherited.font_size_milli().saturating_mul(4) / 5;
    let line_height = inherited.line_height_milli().saturating_mul(39) / 50;
    let style = inherited
        .with_font_metrics(font_size.max(minimum_size), line_height.max(minimum_line))?
        .with_weight(TextWeight::Bold)
        .with_flow(
            RichTextWritingMode::HorizontalTb,
            RichTextInlineDirection::Auto,
        );
    Ok(if use_standard_color {
        style.with_color(speaker_color(preferences))
    } else {
        style
    })
}

fn scaled_milli(value: f32, scale: f32, field: &'static str) -> Result<u32, FramePlanError> {
    let value = f64::from(value) * f64::from(scale) * 1_000.0;
    if !value.is_finite() || value <= 0.0 || value > f64::from(u32::MAX) {
        return Err(FramePlanError::InvalidTextMetric { field });
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "finite positive range is checked immediately before deterministic rounding"
    )]
    Ok(value.round() as u32)
}

fn public_id(value: String) -> Result<PublicId, FramePlanError> {
    PublicId::try_new(&value).map_err(|_| FramePlanError::InvalidId { value })
}

const fn layout_rect(rect: HitRect) -> LayoutRect {
    LayoutRect::new(rect.x, rect.y, rect.width, rect.height)
}

const fn panel_color(preferences: RenderPreferences) -> ViewColorRgba8 {
    if preferences.high_contrast {
        ViewColorRgba8 {
            red: 5,
            green: 5,
            blue: 5,
            alpha: 250,
        }
    } else {
        ViewColorRgba8 {
            red: 17,
            green: 18,
            blue: 16,
            alpha: 242,
        }
    }
}

const fn body_color(preferences: RenderPreferences) -> TextColor {
    if preferences.high_contrast {
        TextColor::rgba(255, 255, 255, 255)
    } else {
        TextColor::rgba(248, 246, 234, 255)
    }
}

const fn speaker_color(preferences: RenderPreferences) -> TextColor {
    if preferences.high_contrast {
        TextColor::rgba(255, 255, 0, 255)
    } else {
        TextColor::rgba(174, 226, 142, 255)
    }
}

#[cfg(test)]
mod tests {
    use super::standard_textbox_bounds;
    use arcweft_render_wgpu::geometry::RenderViewport;

    fn viewport() -> RenderViewport {
        RenderViewport {
            logical_width: 1_280.0,
            logical_height: 720.0,
            physical_width: 1_280,
            physical_height: 720,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn one_textbox_preserves_the_standard_panel_geometry() {
        let panels = standard_textbox_bounds(viewport(), 1);
        let panel = panels.first().expect("one panel");
        assert!((panel.x - 57.6).abs() < 0.001);
        assert!((panel.y - 460.8).abs() < 0.001);
        assert!((panel.width - 1_164.8).abs() < 0.001);
        assert!((panel.height - 201.6).abs() < 0.001);
    }

    #[test]
    fn multiple_textboxes_are_stably_tiled_without_overlap() {
        let panels = standard_textbox_bounds(viewport(), 3);
        assert_eq!(panels.len(), 3);
        assert!(panels.windows(2).all(|pair| {
            pair[0].y + pair[0].height <= pair[1].y
                && (pair[1].y - pair[0].y - pair[0].height - 12.0).abs() < 0.001
        }));
    }
}
