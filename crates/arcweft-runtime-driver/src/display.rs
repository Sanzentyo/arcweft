use crate::dialogue::{
    BundlePresentationTransition, DialogueAdvanceTarget, DialoguePresentationOperation,
    DialoguePresentationStore, DialoguePresentationStoreError, DialogueViewDefinition,
};
use crate::fx_runtime::{BundleFxRuntimeError, BundleFxRuntimeSnapshot};
use crate::presentation_handles::{
    PresentationHandleDiagnostic, PresentationHandleRecord, apply_presentation_handle_operations,
    apply_presentation_image_handles, filter_presentation_action_buttons,
    filter_presentation_focus_groups, filter_presentation_focus_navigation,
    filter_presentation_scroll_regions, filter_presentation_surfaces,
    filter_presentation_text_inputs, hidden_focus_diagnostics,
    presentation_handle_operations_from_effects,
};
use crate::view_projection::ProjectedViewResources;
use crate::view_runtime::BundleViewFrame;
use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButton, ViewRuntimeFocusGroup, ViewRuntimeFocusNavigation,
    ViewRuntimeScrollRegion, ViewRuntimeSurface, ViewRuntimeTextControl,
};
use arcweft_bundle::{
    BundleImageObject, BundleImageObjectAlignment, BundleImageObjectBounds, BundleImageObjectFit,
    BundleImageObjectPlayback, BundleImageObjectTransform,
};
use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::{plan::FlowEvent, value::RuntimeBinding};
use arcweft_layout::ScalePolicy;
use arcweft_layout::stage_placement::{StageAnchor, StagePlacement, StageRect, StageSize};
use arcweft_presentation::{
    BackgroundSlotAddress, PresentationSlot, PresentationTarget, fx::FxDiagnostic,
};
use arcweft_render_text::{RuntimeLineContext, resolve_frame};
use arcweft_text_model::{DialogueContentCatalog, DialogueContentSpec};
use core::fmt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Choice metadata shared by native and Web presentation hosts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleChoice {
    pub id: String,
    pub label: String,
}

/// Runtime-selected player viewport fit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleViewportFit {
    pub design_width: u32,
    pub design_height: u32,
    pub scale_policy: ScalePolicy,
}

/// Display frames and non-fatal display diagnostics resolved from one VM step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayResolution {
    pub dialogue_operations: Vec<DialoguePresentationOperation>,
    pub diagnostics: Vec<String>,
}

/// Checked dynamic presentation context for one static dialogue-content row.
///
/// The static bundle catalog deliberately cannot manufacture character or
/// `CharacterDialogue` values. Runtime integrations provide this boundary
/// only after validating the exact value selected for the emitted line.
pub trait DialogueRuntimeContextProvider {
    fn context_for(
        &self,
        content: &DialogueContentSpec,
        bindings: &[RuntimeBinding],
    ) -> Result<RuntimeLineContext, DialogueRuntimeContextError>;
}

/// Failure to obtain one checked dynamic dialogue presentation context.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DialogueRuntimeContextError {
    #[error("checked runtime CharacterDialogue context is unavailable for line {line:?}")]
    Unavailable {
        line: arcweft_core::plan::RuntimeLineId,
    },
    #[error("checked runtime CharacterDialogue context was rejected for line {line:?}: {reason}")]
    Rejected {
        line: arcweft_core::plan::RuntimeLineId,
        reason: String,
    },
}

/// Failure to atomically apply one presentation update.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BundlePresentationUpdateError {
    #[error(transparent)]
    Dialogue(#[from] DialoguePresentationStoreError),
    #[error("presentation command `{callee}` requires argument `{argument}`")]
    MissingCommandArgument {
        callee: &'static str,
        argument: &'static str,
    },
    #[error(
        "presentation command `{callee}` argument `{argument}` has invalid value `{value}`; expected {expected}"
    )]
    InvalidCommandArgument {
        callee: &'static str,
        argument: &'static str,
        value: String,
        expected: &'static str,
    },
}

impl BundlePresentationUpdateError {
    fn missing_argument(callee: &'static str, argument: &'static str) -> Self {
        Self::MissingCommandArgument { callee, argument }
    }

    fn invalid_argument(
        callee: &'static str,
        argument: &'static str,
        value: impl Into<String>,
        expected: &'static str,
    ) -> Self {
        Self::InvalidCommandArgument {
            callee,
            argument,
            value: value.into(),
            expected,
        }
    }
}

/// Current portable presentation state consumed by renderer adapters.
///
/// This value is a diagnostic/render input model, not a DOM instruction set.
#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct BundlePresentationSnapshot {
    pub revision: u64,
    pub dialogue: DialoguePresentationStore,
    pub choices: Vec<BundleChoice>,
    pub images: Vec<BundleImageObject>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub presentation_handle_epoch: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presentation_handles: Vec<PresentationHandleRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport_fit: Option<BundleViewportFit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_inputs: Vec<ViewRuntimeTextControl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_buttons: Vec<ViewRuntimeActionButton>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scroll_regions: Vec<ViewRuntimeScrollRegion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<ViewRuntimeSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_groups: Vec<ViewRuntimeFocusGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_navigation: Vec<ViewRuntimeFocusNavigation>,
    /// Mount-scoped executable View output for this presentation revision.
    #[serde(default, skip_serializing_if = "is_default")]
    pub view: BundleViewFrame,
    /// Activation-relative logical clock and complete live Fx save state.
    pub fx: BundleFxRuntimeSnapshot,
    /// Typed failures consumed unchanged by native, Web, headless, and Agent observers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx_diagnostics: Vec<FxDiagnostic>,
}

/// Runtime resources that affect portable presentation state.
#[derive(Clone, Copy)]
pub(crate) struct BundlePresentationResources<'a> {
    pub(crate) image_objects: &'a [BundleImageObject],
    pub(crate) text_inputs: &'a [ViewRuntimeTextControl],
    pub(crate) action_buttons: &'a [ViewRuntimeActionButton],
    pub(crate) scroll_regions: &'a [ViewRuntimeScrollRegion],
    pub(crate) surfaces: &'a [ViewRuntimeSurface],
    pub(crate) focus_groups: &'a [ViewRuntimeFocusGroup],
    pub(crate) focus_navigation: &'a [ViewRuntimeFocusNavigation],
}

/// Resolves dialogue flow events into host-renderable, Sans I/O display frames.
pub fn resolve_display_frames(
    catalog: &DialogueContentCatalog,
    events: &[FlowEvent],
    context_provider: Option<&dyn DialogueRuntimeContextProvider>,
) -> DisplayResolution {
    events
        .iter()
        .fold(DisplayResolution::default(), |mut resolution, event| {
            if let FlowEvent::DialogueLine { line, bindings } = event
                && let Some(spec) = catalog.find(line)
            {
                let Some(provider) = context_provider else {
                    resolution.diagnostics.push(
                        DialogueRuntimeContextError::Unavailable { line: line.clone() }.to_string(),
                    );
                    return resolution;
                };
                let context = match provider.context_for(spec, bindings) {
                    Ok(context) => context,
                    Err(error) => {
                        resolution.diagnostics.push(error.to_string());
                        return resolution;
                    }
                };
                match resolve_frame(spec, &context) {
                    Ok(frame) => {
                        let view = frame.effective.view.clone();
                        resolution
                            .dialogue_operations
                            .push(DialoguePresentationOperation::append(
                                DialogueViewDefinition::new(view),
                                frame,
                            ));
                    }
                    Err(error) => resolution.diagnostics.push(error.to_string()),
                }
            }
            resolution
        })
}

impl BundlePresentationSnapshot {
    pub(crate) fn advance_fx_clock(&mut self, milliseconds: u64) {
        match self.fx.advance_millis(milliseconds) {
            Ok(()) => {
                self.fx_diagnostics.clear();
                self.revision = self.revision.saturating_add(1);
            }
            Err(error) => self.record_fx_error(&error),
        }
    }

    pub(crate) fn record_fx_error(&mut self, error: &BundleFxRuntimeError) {
        let diagnostic = error.diagnostic();
        if self.fx_diagnostics.as_slice() != [diagnostic.clone()] {
            self.fx_diagnostics = vec![diagnostic];
            self.revision = self.revision.saturating_add(1);
        }
    }

    pub(crate) fn update(
        &mut self,
        resolution: &DisplayResolution,
        status: &FlowFiberStatus,
        effects: &[LineEffectRequest],
        resources: BundlePresentationResources<'_>,
    ) -> Result<Vec<PresentationHandleDiagnostic>, BundlePresentationUpdateError> {
        let mut next_dialogue = self.dialogue.clone();
        next_dialogue.apply_operations(&resolution.dialogue_operations)?;
        next_dialogue.synchronize_waiting_line(waiting_dialogue_line(status))?;
        let next_choices = choices_from_status(status);
        let (handle_operations, mut handle_diagnostics) =
            presentation_handle_operations_from_effects(effects);
        let mut next_presentation_handles = self.presentation_handles.clone();
        let mut next_presentation_handle_epoch = self.presentation_handle_epoch;
        handle_diagnostics.extend(apply_presentation_handle_operations(
            &mut next_presentation_handles,
            &mut next_presentation_handle_epoch,
            &handle_operations,
        ));
        let mut next_images = images_from_effects(&self.images, effects, resources.image_objects)?;
        apply_presentation_image_handles(
            &mut next_images,
            &next_presentation_handles,
            resources.image_objects,
        );
        let next_viewport_fit = viewport_fit_from_effects(self.viewport_fit, effects);
        let next_text_inputs = filter_presentation_text_inputs(
            resources.text_inputs.to_vec(),
            &next_presentation_handles,
        );
        let next_action_buttons = filter_presentation_action_buttons(
            resources.action_buttons.to_vec(),
            &next_presentation_handles,
        );
        let next_scroll_regions = filter_presentation_scroll_regions(
            resources.scroll_regions.to_vec(),
            &next_presentation_handles,
        );
        let next_surfaces =
            filter_presentation_surfaces(resources.surfaces.to_vec(), &next_presentation_handles);
        let next_focus_groups = filter_presentation_focus_groups(
            resources.focus_groups.to_vec(),
            &next_presentation_handles,
        );
        let raw_focus_navigation = resources.focus_navigation.to_vec();
        handle_diagnostics.extend(hidden_focus_diagnostics(
            &next_presentation_handles,
            &raw_focus_navigation,
        ));
        let next_focus_navigation =
            filter_presentation_focus_navigation(raw_focus_navigation, &next_presentation_handles);
        if self.dialogue != next_dialogue
            || self.choices != next_choices
            || self.images != next_images
            || self.presentation_handle_epoch != next_presentation_handle_epoch
            || self.presentation_handles != next_presentation_handles
            || self.viewport_fit != next_viewport_fit
            || self.text_inputs != next_text_inputs
            || self.action_buttons != next_action_buttons
            || self.scroll_regions != next_scroll_regions
            || self.surfaces != next_surfaces
            || self.focus_groups != next_focus_groups
            || self.focus_navigation != next_focus_navigation
        {
            self.revision = self.revision.saturating_add(1);
            self.dialogue = next_dialogue;
            self.choices = next_choices;
            self.images = next_images;
            self.presentation_handle_epoch = next_presentation_handle_epoch;
            self.presentation_handles = next_presentation_handles;
            self.viewport_fit = next_viewport_fit;
            self.text_inputs = next_text_inputs;
            self.action_buttons = next_action_buttons;
            self.scroll_regions = next_scroll_regions;
            self.surfaces = next_surfaces;
            self.focus_groups = next_focus_groups;
            self.focus_navigation = next_focus_navigation;
        }
        Ok(handle_diagnostics)
    }

    pub(crate) fn replace_view_frame(&mut self, view: BundleViewFrame) {
        if self.view != view {
            self.revision = self.revision.saturating_add(1);
            self.view = view;
        }
    }

    pub(crate) fn replace_view_resources(&mut self, resources: ProjectedViewResources) {
        if self.images != resources.images
            || self.text_inputs != resources.text_inputs
            || self.action_buttons != resources.action_buttons
            || self.scroll_regions != resources.scroll_regions
            || self.surfaces != resources.surfaces
            || self.focus_groups != resources.focus_groups
            || self.focus_navigation != resources.focus_navigation
        {
            self.revision = self.revision.saturating_add(1);
            self.images = resources.images;
            self.text_inputs = resources.text_inputs;
            self.action_buttons = resources.action_buttons;
            self.scroll_regions = resources.scroll_regions;
            self.surfaces = resources.surfaces;
            self.focus_groups = resources.focus_groups;
            self.focus_navigation = resources.focus_navigation;
        }
    }

    pub(crate) fn advance_dialogue(
        &mut self,
        target: DialogueAdvanceTarget,
    ) -> (
        BundlePresentationTransition,
        Option<arcweft_core::plan::RuntimeLineId>,
    ) {
        let before = self.dialogue.clone();
        let result = self.dialogue.advance_dialogue(target);
        if self.dialogue != before {
            self.revision = self.revision.saturating_add(1);
        }
        result
    }

    #[must_use]
    pub fn redacted_for_observation(&self) -> Self {
        Self {
            text_inputs: self
                .text_inputs
                .iter()
                .map(ViewRuntimeTextControl::redacted_for_observation)
                .collect(),
            view: self.view.redacted_for_observation(),
            ..self.clone()
        }
    }
}

fn waiting_dialogue_line(status: &FlowFiberStatus) -> Option<&arcweft_core::plan::RuntimeLineId> {
    match status {
        FlowFiberStatus::Dialogue(state) => Some(&state.line),
        _ => None,
    }
}

impl fmt::Debug for BundlePresentationSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BundlePresentationSnapshot")
            .field("revision", &self.revision)
            .field("dialogue", &self.dialogue)
            .field("choices", &self.choices)
            .field("images", &self.images)
            .field("presentation_handle_epoch", &self.presentation_handle_epoch)
            .field("presentation_handles", &self.presentation_handles)
            .field("viewport_fit", &self.viewport_fit)
            .field("text_inputs", &self.redacted_for_observation().text_inputs)
            .field("action_buttons", &self.action_buttons)
            .field("scroll_regions", &self.scroll_regions)
            .field("surfaces", &self.surfaces)
            .field("focus_groups", &self.focus_groups)
            .field("focus_navigation", &self.focus_navigation)
            .field("view", &self.view.redacted_for_observation())
            .field("fx", &self.fx)
            .field("fx_diagnostics", &self.fx_diagnostics)
            .finish()
    }
}

impl BundleViewportFit {
    pub const fn raw() -> Self {
        Self {
            design_width: 0,
            design_height: 0,
            scale_policy: ScalePolicy::Raw,
        }
    }

    pub const fn design(design_width: u32, design_height: u32, scale_policy: ScalePolicy) -> Self {
        Self {
            design_width,
            design_height,
            scale_policy,
        }
    }
}

fn choices_from_status(status: &FlowFiberStatus) -> Vec<BundleChoice> {
    let FlowFiberStatus::Choice(state) = status else {
        return Vec::new();
    };
    state
        .options
        .iter()
        .map(|option| BundleChoice {
            id: option.id.clone().unwrap_or_else(|| option.label.clone()),
            label: option.label.clone(),
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PresentationViewportEffect {
    Set(BundleViewportFit),
    Clear,
}

impl PresentationViewportEffect {
    fn from_call(call: &RuntimeCall) -> Option<Self> {
        match call.callee.as_str() {
            "player_viewport" => viewport_effect_from_call(call),
            _ => None,
        }
    }
}

fn viewport_fit_from_effects(
    previous: Option<BundleViewportFit>,
    effects: &[LineEffectRequest],
) -> Option<BundleViewportFit> {
    effects
        .iter()
        .filter_map(|effect| {
            let LineEffectRequest::Call(call) = effect else {
                return None;
            };
            PresentationViewportEffect::from_call(call)
        })
        .fold(previous, |_active, effect| match effect {
            PresentationViewportEffect::Set(fit) => Some(fit),
            PresentationViewportEffect::Clear => None,
        })
}

fn viewport_effect_from_call(call: &RuntimeCall) -> Option<PresentationViewportEffect> {
    let width_arg = named_arg(&call.args, "width");
    let height_arg = named_arg(&call.args, "height");
    let fit_arg = named_arg(&call.args, "fit");
    if width_arg.is_none() && height_arg.is_none() && fit_arg.is_none() {
        return None;
    }
    let fit_arg = fit_arg.map_or("contain", unquote_arg);
    match fit_arg {
        "default" | "host" | "inherit" => Some(PresentationViewportEffect::Clear),
        "raw" | "none" => Some(PresentationViewportEffect::Set(BundleViewportFit::raw())),
        "contain" | "cover" | "stretch" => {
            let design_width = width_arg.and_then(parse_positive_u32_px).unwrap_or(1280);
            let design_height = height_arg.and_then(parse_positive_u32_px).unwrap_or(720);
            let scale_policy = match fit_arg {
                "cover" => ScalePolicy::Cover,
                "stretch" => ScalePolicy::Stretch,
                _ => ScalePolicy::Contain,
            };
            Some(PresentationViewportEffect::Set(BundleViewportFit::design(
                design_width,
                design_height,
                scale_policy,
            )))
        }
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PresentationImageEffect {
    Object(String),
    InlineObject(Box<BundleImageObject>),
    Background(Box<BundleImageObject>),
    ClearBackground(String),
}

impl PresentationImageEffect {
    fn from_call(call: &RuntimeCall) -> Result<Option<Self>, BundlePresentationUpdateError> {
        match call.callee.as_str() {
            "image" => Ok(inline_image_object(call)
                .map(|object| Self::InlineObject(Box::new(object)))
                .or_else(|| {
                    call.args
                        .first()
                        .map(String::as_str)
                        .or_else(|| named_arg(&call.args, "id"))
                        .and_then(public_id_arg)
                        .filter(|id| id.starts_with("image."))
                        .map(Self::Object)
                })),
            "bg" => background_image_object(call)
                .map(Box::new)
                .map(Self::Background)
                .map(Some),
            "bg.clear" => background_slot_address(call, "bg.clear")
                .map(|address| address.image_id().as_str().to_owned())
                .map(Self::ClearBackground)
                .map(Some),
            _ => Ok(None),
        }
    }
}

fn images_from_effects(
    previous: &[BundleImageObject],
    effects: &[LineEffectRequest],
    image_objects: &[BundleImageObject],
) -> Result<Vec<BundleImageObject>, BundlePresentationUpdateError> {
    let mut active = previous.to_vec();
    for effect in effects {
        let LineEffectRequest::Call(call) = effect else {
            continue;
        };
        let Some(effect) = PresentationImageEffect::from_call(call)? else {
            continue;
        };
        match effect {
            PresentationImageEffect::Object(id) => {
                if let Some(object) = image_objects
                    .iter()
                    .find(|object| object.id == id && object.visible)
                {
                    upsert_image_object(&mut active, object.clone());
                }
            }
            PresentationImageEffect::InlineObject(object) => {
                if object.visible {
                    upsert_image_object(&mut active, *object);
                }
            }
            PresentationImageEffect::Background(object) => {
                upsert_image_object(&mut active, *object);
            }
            PresentationImageEffect::ClearBackground(id) => {
                active.retain(|object| object.id != id);
            }
        }
        active.sort_by(|left, right| {
            (left.depth_milli, &left.id).cmp(&(right.depth_milli, &right.id))
        });
    }
    Ok(active)
}

fn upsert_image_object(active: &mut Vec<BundleImageObject>, object: BundleImageObject) {
    if let Some(existing) = active.iter_mut().find(|existing| existing.id == object.id) {
        *existing = object;
    } else {
        active.push(object);
    }
}

fn named_arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().find_map(|arg| {
        let (arg_name, value) = arg.split_once(" = ")?;
        (arg_name.trim() == name).then_some(value.trim())
    })
}

fn public_id_arg(arg: &str) -> Option<String> {
    let value = arg
        .split_once(" = ")
        .map_or(arg, |(_, value)| value)
        .trim()
        .trim_matches('"')
        .trim_matches('\'');
    let value = value.strip_prefix('@').unwrap_or(value);
    let normalized = value.split_once(":.").map_or_else(
        || value.to_owned(),
        |(family, suffix)| format!("{family}.{suffix}"),
    );
    (!normalized.is_empty()
        && normalized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')))
    .then_some(normalized)
}

fn inline_image_object(call: &RuntimeCall) -> Option<BundleImageObject> {
    let asset = named_arg(&call.args, "asset")
        .and_then(public_id_arg)
        .filter(|id| id.starts_with("asset."))?;
    let id = named_arg(&call.args, "id")
        .and_then(public_id_arg)
        .filter(|id| id.starts_with("image."))
        .unwrap_or_else(|| {
            let stem = asset.strip_prefix("asset.").unwrap_or(asset.as_str());
            format!("image.{stem}.inline")
        });
    let bounds = BundleImageObjectBounds {
        x_milli: named_arg(&call.args, "x").and_then(parse_px_milli)?,
        y_milli: named_arg(&call.args, "y").and_then(parse_px_milli)?,
        width_milli: u32::try_from(named_arg(&call.args, "width").and_then(parse_px_milli)?)
            .ok()?,
        height_milli: u32::try_from(named_arg(&call.args, "height").and_then(parse_px_milli)?)
            .ok()?,
    };
    let placement = StagePlacement::absolute(StageRect::new(
        bounds.x_milli,
        bounds.y_milli,
        bounds.width_milli,
        bounds.height_milli,
    ));
    Some(BundleImageObject {
        id,
        asset,
        target: named_arg(&call.args, "target").and_then(public_id_arg),
        layer: named_arg(&call.args, "layer").and_then(public_id_arg),
        view: None,
        containing_scroll_region: None,
        bounds,
        placement: Some(placement),
        fit: image_fit_arg(call),
        alignment: image_alignment_arg(call),
        playback: image_playback_arg(call),
        transform: image_transform_arg(call),
        depth_milli: named_arg(&call.args, "depth")
            .and_then(parse_depth_arg)
            .unwrap_or_default(),
        opacity_milli: named_arg(&call.args, "opacity")
            .and_then(parse_opacity_milli)
            .unwrap_or(1_000),
        actions: Vec::new(),
        params: std::collections::BTreeMap::default(),
        proxies: Vec::new(),
        visible: named_arg(&call.args, "visible")
            .and_then(parse_bool_arg)
            .unwrap_or(true),
    })
}

fn background_image_object(
    call: &RuntimeCall,
) -> Result<BundleImageObject, BundlePresentationUpdateError> {
    let asset_arg = call
        .args
        .first()
        .filter(|arg| !arg.contains(" = "))
        .map(String::as_str)
        .or_else(|| named_arg(&call.args, "asset"))
        .ok_or_else(|| BundlePresentationUpdateError::missing_argument("bg", "asset"))?;
    let asset = public_id_arg(asset_arg)
        .filter(|id| id.starts_with("asset."))
        .ok_or_else(|| {
            BundlePresentationUpdateError::invalid_argument(
                "bg",
                "asset",
                asset_arg,
                "an `asset.*` public ID",
            )
        })?;
    let address = background_slot_address(call, "bg")?;
    let id = address.image_id().as_str().to_owned();
    let target = address.target().id().as_str().to_owned();
    let bounds = BundleImageObjectBounds::from_px(0, 0, 1280, 720);
    Ok(BundleImageObject {
        id,
        asset,
        target: Some(target),
        layer: Some("layer.background".to_owned()),
        view: None,
        containing_scroll_region: None,
        bounds,
        placement: Some(StagePlacement::anchor(
            StageAnchor::TopLeft,
            StageAnchor::TopLeft,
            StageSize::new(bounds.width_milli, bounds.height_milli),
        )),
        fit: background_image_fit_arg(call)?,
        alignment: background_image_alignment_arg(call)?,
        playback: background_image_playback_arg(call)?,
        transform: BundleImageObjectTransform::default(),
        depth_milli: 0,
        opacity_milli: background_opacity_arg(call)?,
        actions: Vec::new(),
        params: std::collections::BTreeMap::default(),
        proxies: Vec::new(),
        visible: true,
    })
}

fn background_slot_address(
    call: &RuntimeCall,
    callee: &'static str,
) -> Result<BackgroundSlotAddress, BundlePresentationUpdateError> {
    let target = match named_arg(&call.args, "target") {
        Some(value) => {
            let target = public_id_arg(value)
                .filter(|target| target.starts_with("target."))
                .ok_or_else(|| {
                    BundlePresentationUpdateError::invalid_argument(
                        callee,
                        "target",
                        value,
                        "a `target.*` public ID",
                    )
                })?;
            PresentationTarget::try_new(target).map_err(|_| {
                BundlePresentationUpdateError::invalid_argument(
                    callee,
                    "target",
                    value,
                    "a `target.*` public ID",
                )
            })?
        }
        None => PresentationTarget::scene(),
    };
    let slot = match named_arg(&call.args, "slot") {
        Some(value) => {
            let slot = public_id_arg(value)
                .filter(|slot| slot.starts_with("slot.background."))
                .ok_or_else(|| {
                    BundlePresentationUpdateError::invalid_argument(
                        callee,
                        "slot",
                        value,
                        "a `slot.background.*` public ID",
                    )
                })?;
            PresentationSlot::try_new(slot).map_err(|_| {
                BundlePresentationUpdateError::invalid_argument(
                    callee,
                    "slot",
                    value,
                    "a `slot.background.*` public ID",
                )
            })?
        }
        None => PresentationSlot::default_background(),
    };
    BackgroundSlotAddress::try_new(target, slot).map_err(|_| {
        BundlePresentationUpdateError::invalid_argument(
            callee,
            "target/slot",
            "invalid background address",
            "a valid background target/slot pair",
        )
    })
}

fn background_image_fit_arg(
    call: &RuntimeCall,
) -> Result<BundleImageObjectFit, BundlePresentationUpdateError> {
    match named_arg(&call.args, "fit").map(unquote_arg) {
        None | Some("cover") => Ok(BundleImageObjectFit::Cover),
        Some("contain") => Ok(BundleImageObjectFit::Contain),
        Some("stretch") => Ok(BundleImageObjectFit::Stretch),
        Some("intrinsic") => Ok(BundleImageObjectFit::Intrinsic),
        Some(value) => Err(BundlePresentationUpdateError::invalid_argument(
            "bg",
            "fit",
            value,
            "`cover`, `contain`, `stretch`, or `intrinsic`",
        )),
    }
}

fn background_image_alignment_arg(
    call: &RuntimeCall,
) -> Result<BundleImageObjectAlignment, BundlePresentationUpdateError> {
    let axis = |argument: &'static str, axis: &'static str| {
        let Some(value) = named_arg(&call.args, argument) else {
            return Ok(500);
        };
        parse_alignment_view_milli(value, axis).ok_or_else(|| {
            BundlePresentationUpdateError::invalid_argument(
                "bg",
                argument,
                value,
                "an alignment keyword, a ratio in [0, 1], or integer milli-units in [0, 1000]",
            )
        })
    };
    Ok(BundleImageObjectAlignment {
        x_milli: axis("alignment.x", "x")?,
        y_milli: axis("alignment.y", "y")?,
    })
}

fn background_opacity_arg(call: &RuntimeCall) -> Result<u16, BundlePresentationUpdateError> {
    let Some(value) = named_arg(&call.args, "opacity") else {
        return Ok(1_000);
    };
    parse_opacity_milli(value).ok_or_else(|| {
        BundlePresentationUpdateError::invalid_argument(
            "bg",
            "opacity",
            value,
            "a ratio in [0, 1], percentage in [0%, 100%], or integer milli-units in [0, 1000]",
        )
    })
}

fn background_image_playback_arg(
    call: &RuntimeCall,
) -> Result<BundleImageObjectPlayback, BundlePresentationUpdateError> {
    let duration = |argument: &'static str| {
        let Some(value) = named_arg(&call.args, argument) else {
            return Ok(None);
        };
        parse_duration_millis(value).map(Some).ok_or_else(|| {
            BundlePresentationUpdateError::invalid_argument(
                "bg",
                argument,
                value,
                "a finite non-negative duration",
            )
        })
    };
    let rate_milli = match named_arg(&call.args, "playback.rate") {
        None => 1_000,
        Some(value) => parse_rate_milli(value).ok_or_else(|| {
            BundlePresentationUpdateError::invalid_argument(
                "bg",
                "playback.rate",
                value,
                "a non-negative ratio or integer milli-unit rate",
            )
        })?,
    };
    Ok(BundleImageObjectPlayback {
        start_time_millis: duration("playback.start")?.unwrap_or_default(),
        rate_milli,
        paused_at_millis: duration("playback.paused_at")?,
        pinned_local_time_millis: duration("playback.local_time")?,
    })
}

fn image_fit_arg(call: &RuntimeCall) -> BundleImageObjectFit {
    match named_arg(&call.args, "fit").map(unquote_arg) {
        Some("cover") => BundleImageObjectFit::Cover,
        Some("stretch") => BundleImageObjectFit::Stretch,
        Some("intrinsic") => BundleImageObjectFit::Intrinsic,
        _ => BundleImageObjectFit::Contain,
    }
}

fn image_alignment_arg(call: &RuntimeCall) -> BundleImageObjectAlignment {
    BundleImageObjectAlignment {
        x_milli: named_arg(&call.args, "alignment.x")
            .and_then(|value| parse_alignment_view_milli(value, "x"))
            .unwrap_or(500),
        y_milli: named_arg(&call.args, "alignment.y")
            .and_then(|value| parse_alignment_view_milli(value, "y"))
            .unwrap_or(500),
    }
}

fn parse_alignment_view_milli(value: &str, axis: &str) -> Option<i32> {
    let value = unquote_arg(value);
    match (axis, value) {
        ("x", "left" | "start") | ("y", "top" | "start") => return Some(0),
        ("x" | "y", "center" | "middle") => return Some(500),
        ("x", "right" | "end") | ("y", "bottom" | "end") => return Some(1_000),
        _ => {}
    }
    let numeric = value.parse::<f64>().ok()?;
    if !numeric.is_finite() || !(0.0..=1_000.0).contains(&numeric) {
        return None;
    }
    if numeric <= 1.0 {
        return rounded_i32(numeric * 1_000.0);
    }
    if numeric.fract().abs() <= f64::EPSILON {
        rounded_i32(numeric)
    } else {
        None
    }
}

fn image_playback_arg(call: &RuntimeCall) -> BundleImageObjectPlayback {
    BundleImageObjectPlayback {
        start_time_millis: named_arg(&call.args, "playback.start")
            .and_then(parse_duration_millis)
            .unwrap_or_default(),
        rate_milli: named_arg(&call.args, "playback.rate")
            .and_then(parse_rate_milli)
            .unwrap_or(1_000),
        paused_at_millis: named_arg(&call.args, "playback.paused_at")
            .and_then(parse_duration_millis),
        pinned_local_time_millis: named_arg(&call.args, "playback.local_time")
            .and_then(parse_duration_millis),
    }
}

fn image_transform_arg(call: &RuntimeCall) -> BundleImageObjectTransform {
    BundleImageObjectTransform {
        m11_milli: named_arg(&call.args, "transform.m11")
            .and_then(parse_milli_arg)
            .unwrap_or(1_000),
        m12_milli: named_arg(&call.args, "transform.m12")
            .and_then(parse_milli_arg)
            .unwrap_or_default(),
        m21_milli: named_arg(&call.args, "transform.m21")
            .and_then(parse_milli_arg)
            .unwrap_or_default(),
        m22_milli: named_arg(&call.args, "transform.m22")
            .and_then(parse_milli_arg)
            .unwrap_or(1_000),
        tx_milli: named_arg(&call.args, "transform.tx")
            .and_then(parse_px_milli)
            .unwrap_or_default(),
        ty_milli: named_arg(&call.args, "transform.ty")
            .and_then(parse_px_milli)
            .unwrap_or_default(),
    }
}

fn parse_bool_arg(value: &str) -> Option<bool> {
    match unquote_arg(value) {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_opacity_milli(value: &str) -> Option<u16> {
    let value = unquote_arg(value);
    let milli = if let Some(percent) = value.strip_suffix('%') {
        let percent = percent.trim().parse::<f64>().ok()?;
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return None;
        }
        rounded_i32(percent * 10.0)?
    } else {
        let numeric = value.parse::<f64>().ok()?;
        if !numeric.is_finite() || !(0.0..=1_000.0).contains(&numeric) {
            return None;
        }
        if numeric <= 1.0 {
            rounded_i32(numeric * 1_000.0)?
        } else if numeric.fract().abs() <= f64::EPSILON {
            rounded_i32(numeric)?
        } else {
            return None;
        }
    };
    u16::try_from(milli).ok()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_rate_milli(value: &str) -> Option<u32> {
    let numeric = unquote_arg(value).parse::<f64>().ok()?;
    if !numeric.is_finite() || numeric < 0.0 {
        return None;
    }
    let milli = if numeric <= 1.0 {
        numeric * 1_000.0
    } else if numeric.fract().abs() <= f64::EPSILON {
        numeric
    } else {
        return None;
    };
    (milli <= f64::from(u32::MAX)).then_some(milli as u32)
}

fn parse_depth_arg(value: &str) -> Option<i32> {
    rounded_i32(unquote_arg(value).parse::<f64>().ok()?)
}

fn parse_milli_arg(value: &str) -> Option<i32> {
    let value = unquote_arg(value);
    if let Some(percent) = value.strip_suffix('%') {
        return rounded_i32(percent.trim().parse::<f64>().ok()? * 10.0);
    }
    rounded_i32(value.parse::<f64>().ok()? * 1_000.0)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn parse_duration_millis(value: &str) -> Option<u64> {
    let value = unquote_arg(value);
    let millis = if let Some(ms) = value.strip_suffix("ms") {
        ms.trim().parse::<f64>().ok()?
    } else if let Some(seconds) = value.strip_suffix('s') {
        seconds.trim().parse::<f64>().ok()? * 1_000.0
    } else {
        value.parse::<f64>().ok()?
    };
    let millis = millis.round();
    (millis.is_finite() && millis >= 0.0 && millis <= u64::MAX as f64).then_some(millis as u64)
}

fn parse_px_milli(value: &str) -> Option<i32> {
    let value = unquote_arg(value);
    let pixels = value.strip_suffix("px").unwrap_or(value).trim();
    let parsed = pixels.parse::<f64>().ok()?;
    rounded_i32(parsed * 1_000.0)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_positive_u32_px(value: &str) -> Option<u32> {
    let value = unquote_arg(value);
    let pixels = value.strip_suffix("px").unwrap_or(value).trim();
    let parsed = pixels.parse::<f64>().ok()?.round();
    if !parsed.is_finite() || parsed < 1.0 {
        return None;
    }
    Some(parsed.min(f64::from(u32::MAX)) as u32)
}

#[allow(clippy::cast_possible_truncation)]
fn rounded_i32(value: f64) -> Option<i32> {
    let rounded = value.round();
    rounded
        .is_finite()
        .then_some(rounded.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32)
}

fn unquote_arg(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('\'')
}

fn is_default<T>(value: &T) -> bool
where
    T: Default + PartialEq,
{
    value == &T::default()
}

#[cfg(test)]
mod tests;
