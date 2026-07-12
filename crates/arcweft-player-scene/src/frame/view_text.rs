//! Canonical View text-source resolution and frame-local preparation.

use super::{milli_i32_to_f32, milli_u32_to_f32, scroll_adjusted_bounds};
use crate::input::InputController;
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::resource_codec::view::{
    RgbaColor, ViewRuntimeControlState, ViewTextSelectionPolicy,
};
use arcweft_id::PublicId;
use arcweft_layout::{ContentRect, LayoutRect as FitLayoutRect};
use arcweft_presentation::fx::{
    FxApplication, FxApplicationResolver, FxDiagnostic, FxDiagnosticCode, FxDiagnosticContext,
    FxEvaluationBinding,
};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_render_text::{
    LineDisplayFrame, ResolvedTextDocument, ResolvedTextRun, ResolvedTextRunSource,
    ResolvedTextStyle, RichTextInlineDirection, RichTextPresentation, RichTextRange,
    RichTextWritingMode, TextColor, TextDocumentRevision, TextFontFamily, TextSlant,
    TextStyleCascade, TextWeight,
};
use arcweft_render_wgpu::geometry::{
    FramePlanError, PreparedDialogueViewState, PreparedFrame, PreparedRichTextStageRequest,
    PreparedTextDocumentRequest, PreparedTextOwner, PreparedTextOwnerKind, RenderScene,
    SharedFramePlanContext,
};
use arcweft_render_wgpu::view_scene::PreparedTextId;
use arcweft_runtime_driver::view_runtime::{
    BundleViewMountOutput, BundleViewTextOutput, BundleViewTextTarget, BundleViewTextValue,
};
use arcweft_runtime_driver::{
    dialogue::{DialogueEntryState, DialogueInstanceId, DialoguePresentation},
    display::BundlePresentationSnapshot,
    presentation_handles::PresentationHandleId,
};
use arcweft_text_layout::{LayoutPoint, LayoutRect, LayoutSize};
use std::collections::BTreeMap;

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

#[derive(Clone, Copy)]
struct DialogueTextContext<'a> {
    dialogue: &'a DialoguePresentation,
    entry: &'a DialogueEntryState,
}

struct DialogueFxResolver<'a> {
    context: DialogueTextContext<'a>,
    definitions: &'a FxDefinitions,
    runtime: &'a arcweft_runtime_driver::fx_runtime::BundleFxRuntimeSnapshot,
}

impl FxApplicationResolver for DialogueFxResolver<'_> {
    fn resolve<'a>(
        &'a self,
        application: &FxApplication,
    ) -> Result<FxEvaluationBinding<'a>, Box<FxDiagnostic>> {
        let instance_id = self
            .context
            .entry
            .fx_instance_id(self.context.dialogue.id(), application);
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
                        "bundle has no definition `{}` for dialogue RichText application",
                        application.definition()
                    ),
                ))
            })?;
        let instance = self.runtime.instance(instance_id).ok_or_else(|| {
            Box::new(FxDiagnostic::error(
                FxDiagnosticCode::ProgramValidation,
                context,
                "runtime did not retain the dialogue RichText Fx application instance",
            ))
        })?;
        Ok(FxEvaluationBinding {
            definition,
            instance,
            runtime_time: self.runtime.logical_time,
        })
    }
}

#[derive(Clone, Copy)]
struct PushedTextValue {
    text: PreparedTextId,
    source_origin: usize,
    reveal_complete: Option<bool>,
}

struct PreparedTargetRecord<'a> {
    mount: &'a BundleViewMountOutput,
    output: &'a BundleViewTextOutput,
    target: &'a BundleViewTextTarget,
    dialogue: Option<DialogueTextContext<'a>>,
    semantic_id: PublicId,
    bounds: HitRect,
    clip: Option<HitRect>,
    pushed: PushedTextValue,
}

struct DialoguePreparedState {
    dialogue: u64,
    entry: u64,
    mount: u64,
    revision: u64,
    instance: u64,
    stage: u32,
    bounds: Option<HitRect>,
    reveal_complete: bool,
    advance_available: bool,
    primary_action: Option<arcweft_view::DialogueAdvanceTarget>,
}

pub(super) struct RuntimeViewTextRequest<'a> {
    pub(super) input: &'a InputController,
    pub(super) scene: &'a RenderScene,
    pub(super) presentation: &'a BundlePresentationSnapshot,
    pub(super) fx_definitions: &'a FxDefinitions,
    pub(super) visual_time_millis: u64,
    pub(super) latest_reveal_complete: bool,
    pub(super) content: Option<ContentRect>,
}

#[derive(Clone, Copy)]
struct TextValuePreparationContext<'a> {
    dialogue: Option<DialogueTextContext<'a>>,
    fx_definitions: &'a FxDefinitions,
    presentation: &'a BundlePresentationSnapshot,
    visual_time_millis: u64,
    latest_dialogue_instance: Option<DialogueInstanceId>,
    latest_reveal_complete: bool,
}

struct RuntimeViewTextPreparer<'a, 'request> {
    shared: &'a mut SharedFramePlanContext,
    frame: &'a mut PreparedFrame,
    request: RuntimeViewTextRequest<'request>,
    latest_dialogue_instance: Option<DialogueInstanceId>,
    prepared: Vec<PreparedMountedViewText>,
    dialogue_states: BTreeMap<PresentationHandleId, DialoguePreparedState>,
}

pub(super) fn prepare_runtime_view_text(
    shared: &mut SharedFramePlanContext,
    frame: &mut PreparedFrame,
    request: RuntimeViewTextRequest<'_>,
) -> Result<Vec<PreparedMountedViewText>, FramePlanError> {
    RuntimeViewTextPreparer::new(shared, frame, request).prepare()
}

impl<'a, 'request> RuntimeViewTextPreparer<'a, 'request> {
    fn new(
        shared: &'a mut SharedFramePlanContext,
        frame: &'a mut PreparedFrame,
        request: RuntimeViewTextRequest<'request>,
    ) -> Self {
        let latest_dialogue_instance = request
            .presentation
            .dialogue
            .latest_active()
            .map(|(_, entry)| entry.instance());
        Self {
            shared,
            frame,
            request,
            latest_dialogue_instance,
            prepared: Vec::new(),
            dialogue_states: BTreeMap::new(),
        }
    }

    fn prepare(mut self) -> Result<Vec<PreparedMountedViewText>, FramePlanError> {
        let mounts = self.request.presentation.view.mounts.clone();
        for mount in &mounts {
            self.prepare_mount(mount)?;
        }
        for state in self.dialogue_states.into_values() {
            self.frame.push_dialogue_view(PreparedDialogueViewState {
                dialogue: state.dialogue,
                entry: state.entry,
                mount: state.mount,
                revision: state.revision,
                instance: state.instance,
                stage: state.stage,
                bounds: state.bounds.unwrap_or(HitRect::new(0.0, 0.0, 0.0, 0.0)),
                reveal_complete: state.reveal_complete,
                advance_available: state.advance_available,
                primary_action: state.primary_action,
            });
        }
        Ok(self.prepared)
    }

    fn prepare_mount(&mut self, mount: &BundleViewMountOutput) -> Result<(), FramePlanError> {
        let dialogue = dialogue_context(self.request.presentation, &mount.handle);
        if let Some(dialogue) = dialogue {
            self.retain_dialogue_state(mount, dialogue);
        }
        for output in &mount.text {
            for target in &output.targets {
                if mount.active_targets.contains(&target.public_id) {
                    self.prepare_target(mount, output, target, dialogue)?;
                }
            }
        }
        Ok(())
    }

    fn retain_dialogue_state(
        &mut self,
        mount: &BundleViewMountOutput,
        dialogue: DialogueTextContext<'request>,
    ) {
        let root_output = root_mount(self.request.presentation, mount);
        let state = DialoguePreparedState {
            dialogue: dialogue.dialogue.id().get(),
            entry: dialogue.entry.id().get(),
            mount: root_output.mount.get(),
            revision: dialogue.dialogue.revision().get(),
            instance: dialogue.entry.instance().get(),
            stage: dialogue.entry.stage_index().get(),
            bounds: dialogue_surface_bounds(
                self.request.scene,
                self.request.presentation,
                root_output,
                self.request.content,
            ),
            reveal_complete: true,
            advance_available: dialogue.entry.is_waiting_for_advance(),
            primary_action: root_output
                .dialogue
                .and_then(|state| state.primary_action.target),
        };
        self.dialogue_states
            .entry(mount.handle.clone())
            .or_insert(state);
    }

    fn prepare_target(
        &mut self,
        mount: &BundleViewMountOutput,
        output: &BundleViewTextOutput,
        target: &BundleViewTextTarget,
        dialogue: Option<DialogueTextContext<'request>>,
    ) -> Result<(), FramePlanError> {
        let Some((bounds, clip)) =
            target_geometry(self.request.scene, mount, target, self.request.content)
        else {
            return Ok(());
        };
        let scoped_id = mount.scoped_id(&target.public_id);
        let semantic_id = PublicId::try_new(&scoped_id)
            .map_err(|_| FramePlanError::InvalidId { value: scoped_id })?;
        let interaction_target = Some(InteractionTarget::new(semantic_id.clone()));
        let selection = interaction_target
            .as_ref()
            .and_then(|target| {
                self.request
                    .input
                    .text_block_selection_for(target, visible_text(&output.value).ok()?)
            })
            .map(|selection| {
                RichTextRange::new(
                    usize::try_from(selection.start().get()).unwrap_or(usize::MAX),
                    usize::try_from(selection.end().get()).unwrap_or(usize::MAX),
                )
            });
        let visual = target
            .style
            .visual_for_state(ViewRuntimeControlState::Normal);
        let fit_scale = self.request.content.map_or(1.0, |content| {
            ((content.scale_x.abs() + content.scale_y.abs()) * 0.5).max(f32::EPSILON)
        });
        let text_scale = f32::from(self.request.scene.preferences.text_scale_milli) / 1_000.0;
        let style = resolved_style(&visual, fit_scale * text_scale)?;
        let text_request = PreparedTextDocumentRequest {
            origin: LayoutPoint::new(bounds.x, bounds.y),
            size: LayoutSize::new(bounds.width, bounds.height),
            container_bounds: LayoutRect::new(bounds.x, bounds.y, bounds.width, bounds.height),
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
        let pushed = push_text_value(
            self.shared,
            self.frame,
            &output.value,
            style,
            &text_request,
            TextValuePreparationContext {
                dialogue,
                fx_definitions: self.request.fx_definitions,
                presentation: self.request.presentation,
                visual_time_millis: self.request.visual_time_millis,
                latest_dialogue_instance: self.latest_dialogue_instance,
                latest_reveal_complete: self.request.latest_reveal_complete,
            },
        )?;
        self.record_prepared_target(PreparedTargetRecord {
            mount,
            output,
            target,
            dialogue,
            semantic_id,
            bounds,
            clip,
            pushed,
        })
    }

    fn record_prepared_target(
        &mut self,
        record: PreparedTargetRecord<'_>,
    ) -> Result<(), FramePlanError> {
        let PreparedTargetRecord {
            mount,
            output,
            target,
            dialogue,
            semantic_id,
            bounds,
            clip,
            pushed,
        } = record;
        let root_output = root_mount(self.request.presentation, mount);
        let parent_value = format!("view.mount.{}", root_output.mount.get());
        let parent_id =
            PublicId::try_new(&parent_value).map_err(|_| FramePlanError::InvalidId {
                value: parent_value,
            })?;
        let dialogue_content =
            dialogue.filter(|_| matches!(output.value, BundleViewTextValue::DisplayFrame { .. }));
        let object_bounds = dialogue_content
            .and_then(|_| {
                dialogue_surface_bounds(
                    self.request.scene,
                    self.request.presentation,
                    root_output,
                    self.request.content,
                )
            })
            .unwrap_or(bounds);
        let owner_kind = dialogue_content.map_or(
            PreparedTextOwnerKind::View {
                mount: mount.mount.get(),
            },
            |dialogue| PreparedTextOwnerKind::DialogueView {
                dialogue: dialogue.dialogue.id().get(),
                entry: dialogue.entry.id().get(),
                mount: mount.mount.get(),
            },
        );
        self.frame.push_prepared_text_owner(
            PreparedTextOwner::new(
                pushed.text,
                semantic_id,
                owner_kind,
                pushed.source_origin,
                object_bounds,
            )
            .with_parent(parent_id),
        )?;
        if let Some(state) = self.dialogue_states.get_mut(&mount.handle) {
            state.bounds = Some(
                state
                    .bounds
                    .map_or(bounds, |current| union(current, bounds)),
            );
            if let Some(reveal_complete) = pushed.reveal_complete {
                state.reveal_complete = reveal_complete;
            }
        }
        self.prepared.push(PreparedMountedViewText {
            mount: mount.mount.get(),
            source_id: output.source_id.clone(),
            target: target.public_id.clone(),
            text: pushed.text,
            bounds,
            clip,
        });
        Ok(())
    }
}

fn push_text_value(
    shared: &mut SharedFramePlanContext,
    frame: &mut PreparedFrame,
    value: &BundleViewTextValue,
    style: ResolvedTextStyle,
    request: &PreparedTextDocumentRequest,
    context: TextValuePreparationContext<'_>,
) -> Result<PushedTextValue, FramePlanError> {
    let cascade = TextStyleCascade::new(style.clone());
    match value {
        BundleViewTextValue::Plain { value } => {
            let document = plain_document(value, style)?;
            let source_origin = document.source_origin();
            let text = shared.push_prepared_text_document(frame, &document, request)?;
            Ok(PushedTextValue {
                text,
                source_origin,
                reveal_complete: None,
            })
        }
        BundleViewTextValue::Localized { document, .. } => {
            let document = document
                .resolve_document_with_source(&cascade, ResolvedTextRunSource::Localized)?;
            let source_origin = document.source_origin();
            let text = shared.push_prepared_text_document(frame, &document, request)?;
            Ok(PushedTextValue {
                text,
                source_origin,
                reveal_complete: None,
            })
        }
        BundleViewTextValue::RichTextDocument { document } => {
            let document = document.resolve_document(&cascade)?;
            let source_origin = document.source_origin();
            let text = shared.push_prepared_text_document(frame, &document, request)?;
            Ok(PushedTextValue {
                text,
                source_origin,
                reveal_complete: None,
            })
        }
        BundleViewTextValue::DialogueSpeaker {
            label,
            frame: display,
        } => {
            let inherited =
                TextStyleCascade::new(style).resolve_style(display.base_styles.iter())?;
            let style = inherited.with_flow(
                RichTextWritingMode::HorizontalTb,
                RichTextInlineDirection::Auto,
            );
            let document = plain_document(label, style)?;
            let source_origin = document.source_origin();
            let text = shared.push_prepared_text_document(frame, &document, request)?;
            Ok(PushedTextValue {
                text,
                source_origin,
                reveal_complete: None,
            })
        }
        BundleViewTextValue::DisplayFrame {
            frame: display,
            stage_index,
        } => push_display_frame(
            shared,
            frame,
            display,
            *stage_index,
            style,
            request,
            context,
        ),
    }
}

fn push_display_frame(
    shared: &mut SharedFramePlanContext,
    frame: &mut PreparedFrame,
    display: &LineDisplayFrame,
    stage_index: u32,
    style: ResolvedTextStyle,
    request: &PreparedTextDocumentRequest,
    context: TextValuePreparationContext<'_>,
) -> Result<PushedTextValue, FramePlanError> {
    let stage_index = usize::try_from(stage_index).map_err(|_| {
        FramePlanError::ResolveText(arcweft_render_text::TextResolveError::InvalidDisplayStage {
            index: usize::MAX,
        })
    })?;
    let stage = display
        .stage(stage_index)
        .ok_or(FramePlanError::ResolveText(
            arcweft_render_text::TextResolveError::InvalidDisplayStage { index: stage_index },
        ))?;
    let Some(dialogue) = context.dialogue else {
        let cascade = TextStyleCascade::new(style);
        let document = display.resolve_stage_document(stage, &cascade)?;
        let source_origin = document.source_origin();
        let text = shared.push_prepared_text_document(frame, &document, request)?;
        return Ok(PushedTextValue {
            text,
            source_origin,
            reveal_complete: None,
        });
    };
    let fx_resolver = DialogueFxResolver {
        context: dialogue,
        definitions: context.fx_definitions,
        runtime: &context.presentation.fx,
    };
    let result = shared.push_prepared_rich_text_stage(
        frame,
        stage,
        &PreparedRichTextStageRequest {
            bounds: HitRect::new(
                request.origin.x,
                request.origin.y,
                request.size.width,
                request.size.height,
            ),
            default_style: style,
            visual_time_millis: context.visual_time_millis,
            reveal_complete: context.latest_dialogue_instance != Some(dialogue.entry.instance())
                || context.latest_reveal_complete,
        },
        &fx_resolver,
    )?;
    Ok(PushedTextValue {
        text: result.text,
        source_origin: result.source_origin,
        reveal_complete: Some(result.reveal_complete),
    })
}

fn dialogue_context<'a>(
    presentation: &'a BundlePresentationSnapshot,
    handle: &PresentationHandleId,
) -> Option<DialogueTextContext<'a>> {
    presentation.dialogue.iter().find_map(|dialogue| {
        dialogue
            .entries()
            .iter()
            .find(|entry| entry.view_handle_id() == *handle)
            .map(|entry| DialogueTextContext { dialogue, entry })
    })
}

fn root_mount<'a>(
    presentation: &'a BundlePresentationSnapshot,
    mount: &'a BundleViewMountOutput,
) -> &'a BundleViewMountOutput {
    presentation
        .view
        .mounts
        .iter()
        .find(|candidate| candidate.handle == mount.handle && candidate.path.segments().is_empty())
        .unwrap_or(mount)
}

fn union(left: HitRect, right: HitRect) -> HitRect {
    let min_x = left.x.min(right.x);
    let min_y = left.y.min(right.y);
    let max_x = (left.x + left.width).max(right.x + right.width);
    let max_y = (left.y + left.height).max(right.y + right.height);
    HitRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
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

fn dialogue_surface_bounds(
    scene: &RenderScene,
    presentation: &BundlePresentationSnapshot,
    mount: &BundleViewMountOutput,
    content: Option<ContentRect>,
) -> Option<HitRect> {
    let owner = mount.scoped_id(&mount.view);
    presentation
        .surfaces
        .iter()
        .filter(|surface| surface.view.as_deref() == Some(owner.as_str()))
        .filter_map(|surface| {
            let bounds = HitRect::new(
                milli_i32_to_f32(surface.bounds.x_milli),
                milli_i32_to_f32(surface.bounds.y_milli),
                milli_u32_to_f32(surface.bounds.width_milli),
                milli_u32_to_f32(surface.bounds.height_milli),
            );
            scroll_adjusted_bounds(scene, surface.containing_scroll_region.as_deref(), bounds)
                .map(|(bounds, _)| map_rect(bounds, content))
        })
        .reduce(union)
}

fn visible_text(value: &BundleViewTextValue) -> Result<&str, FramePlanError> {
    match value {
        BundleViewTextValue::Plain { value } => Ok(value),
        BundleViewTextValue::DialogueSpeaker { label, .. } => Ok(label),
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

pub(super) fn plain_document(
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

pub(super) fn map_rect(rect: HitRect, content: Option<ContentRect>) -> HitRect {
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
