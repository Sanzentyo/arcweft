use crate::presentation_handles::{
    PresentationHandleDiagnostic, PresentationHandleRecord, apply_presentation_handle_operations,
    apply_presentation_image_handles, filter_presentation_action_buttons,
    filter_presentation_focus_groups, filter_presentation_focus_navigation,
    filter_presentation_text_inputs, hidden_focus_diagnostics,
    presentation_handle_operations_from_effects,
};
use arcweft_bundle::resource_codec::{
    UiRuntimeActionButton, UiRuntimeFocusGroup, UiRuntimeFocusNavigation, UiRuntimeTextControl,
};
use arcweft_bundle::{
    BundleImageObject, BundleImageObjectAlignment, BundleImageObjectBounds, BundleImageObjectFit,
    BundleImageObjectPlayback, BundleImageObjectTransform,
};
use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::plan::FlowEvent;
use arcweft_layout::ScalePolicy;
use arcweft_layout::stage_placement::{StagePlacement, StageRect};
use arcweft_render_text::{LineDisplayCatalog, LineDisplayFrame, RuntimeLineContext};
use core::fmt;
use serde::{Deserialize, Serialize};

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
    pub frames: Vec<LineDisplayFrame>,
    pub diagnostics: Vec<String>,
}

/// Current portable presentation state consumed by renderer adapters.
///
/// This value is a diagnostic/render input model, not a DOM instruction set.
#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct BundlePresentationSnapshot {
    pub revision: u64,
    pub dialogue: Option<LineDisplayFrame>,
    pub choices: Vec<BundleChoice>,
    pub images: Vec<BundleImageObject>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presentation_handles: Vec<PresentationHandleRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport_fit: Option<BundleViewportFit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_inputs: Vec<UiRuntimeTextControl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_buttons: Vec<UiRuntimeActionButton>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_groups: Vec<UiRuntimeFocusGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_navigation: Vec<UiRuntimeFocusNavigation>,
}

/// Runtime resources that affect portable presentation state.
#[derive(Clone, Copy)]
pub(crate) struct BundlePresentationResources<'a> {
    pub(crate) image_objects: &'a [BundleImageObject],
    pub(crate) text_inputs: &'a [UiRuntimeTextControl],
    pub(crate) action_buttons: &'a [UiRuntimeActionButton],
    pub(crate) focus_groups: &'a [UiRuntimeFocusGroup],
    pub(crate) focus_navigation: &'a [UiRuntimeFocusNavigation],
}

/// Resolves dialogue flow events into host-renderable, Sans I/O display frames.
pub fn resolve_display_frames(
    catalog: &LineDisplayCatalog,
    events: &[FlowEvent],
) -> DisplayResolution {
    events
        .iter()
        .fold(DisplayResolution::default(), |mut resolution, event| {
            if let FlowEvent::DialogueLine { line, bindings } = event
                && let Some(spec) = catalog.find(line)
            {
                match spec.resolve_frame(&RuntimeLineContext::new(bindings.clone())) {
                    Ok(frame) => resolution.frames.push(frame),
                    Err(error) => resolution.diagnostics.push(error.to_string()),
                }
            }
            resolution
        })
}

impl BundlePresentationSnapshot {
    pub(crate) fn update(
        &mut self,
        resolution: &DisplayResolution,
        status: &FlowFiberStatus,
        effects: &[LineEffectRequest],
        resources: BundlePresentationResources<'_>,
    ) -> Vec<PresentationHandleDiagnostic> {
        let next_dialogue = resolution
            .frames
            .last()
            .cloned()
            .or_else(|| self.dialogue.clone());
        let next_choices = choices_from_status(status);
        let (handle_operations, mut handle_diagnostics) =
            presentation_handle_operations_from_effects(effects);
        let mut next_presentation_handles = self.presentation_handles.clone();
        handle_diagnostics.extend(apply_presentation_handle_operations(
            &mut next_presentation_handles,
            &handle_operations,
        ));
        let mut next_images = images_from_effects(&self.images, effects, resources.image_objects);
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
            || self.presentation_handles != next_presentation_handles
            || self.viewport_fit != next_viewport_fit
            || self.text_inputs != next_text_inputs
            || self.action_buttons != next_action_buttons
            || self.focus_groups != next_focus_groups
            || self.focus_navigation != next_focus_navigation
        {
            self.revision = self.revision.saturating_add(1);
            self.dialogue = next_dialogue;
            self.choices = next_choices;
            self.images = next_images;
            self.presentation_handles = next_presentation_handles;
            self.viewport_fit = next_viewport_fit;
            self.text_inputs = next_text_inputs;
            self.action_buttons = next_action_buttons;
            self.focus_groups = next_focus_groups;
            self.focus_navigation = next_focus_navigation;
        }
        handle_diagnostics
    }

    pub(crate) fn replace_text_inputs(&mut self, text_inputs: &[UiRuntimeTextControl]) {
        let next_text_inputs = text_inputs.to_vec();
        if self.text_inputs != next_text_inputs {
            self.revision = self.revision.saturating_add(1);
            self.text_inputs = next_text_inputs;
        }
    }

    #[must_use]
    pub fn redacted_for_observation(&self) -> Self {
        Self {
            text_inputs: self
                .text_inputs
                .iter()
                .map(UiRuntimeTextControl::redacted_for_observation)
                .collect(),
            ..self.clone()
        }
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
            .field("presentation_handles", &self.presentation_handles)
            .field("viewport_fit", &self.viewport_fit)
            .field("text_inputs", &self.redacted_for_observation().text_inputs)
            .field("action_buttons", &self.action_buttons)
            .field("focus_groups", &self.focus_groups)
            .field("focus_navigation", &self.focus_navigation)
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
            "player_viewport"
            | "viewport"
            | "viewport.fit"
            | "player.viewport"
            | "player.viewport.fit" => viewport_effect_from_call(call),
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
    let fit_arg = named_arg(&call.args, "fit")
        .or_else(|| named_arg(&call.args, "policy"))
        .or_else(|| named_arg(&call.args, "scale_policy"))
        .or_else(|| named_arg(&call.args, "scale-policy"))
        .map_or("contain", unquote_arg);
    match fit_arg {
        "default" | "host" | "inherit" => Some(PresentationViewportEffect::Clear),
        "raw" | "none" => Some(PresentationViewportEffect::Set(BundleViewportFit::raw())),
        "contain" | "cover" | "stretch" => {
            let design_width = named_arg(&call.args, "design_width")
                .or_else(|| named_arg(&call.args, "design-width"))
                .or_else(|| named_arg(&call.args, "width"))
                .and_then(parse_positive_u32_px)
                .unwrap_or(1280);
            let design_height = named_arg(&call.args, "design_height")
                .or_else(|| named_arg(&call.args, "design-height"))
                .or_else(|| named_arg(&call.args, "height"))
                .and_then(parse_positive_u32_px)
                .unwrap_or(720);
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
    BackgroundAsset(String),
}

impl PresentationImageEffect {
    fn from_call(call: &RuntimeCall) -> Option<Self> {
        match call.callee.as_str() {
            "image" | "image.show" => inline_image_object(call)
                .map(|object| Self::InlineObject(Box::new(object)))
                .or_else(|| {
                    call.args
                        .first()
                        .map(String::as_str)
                        .or_else(|| named_arg(&call.args, "id"))
                        .and_then(public_id_arg)
                        .filter(|id| id.starts_with("image."))
                        .map(Self::Object)
                }),
            "bg" | "background" => call
                .args
                .first()
                .map(String::as_str)
                .or_else(|| named_arg(&call.args, "asset"))
                .and_then(public_id_arg)
                .filter(|id| id.starts_with("asset."))
                .map(Self::BackgroundAsset),
            _ => None,
        }
    }
}

fn images_from_effects(
    previous: &[BundleImageObject],
    effects: &[LineEffectRequest],
    image_objects: &[BundleImageObject],
) -> Vec<BundleImageObject> {
    effects
        .iter()
        .filter_map(|effect| {
            let LineEffectRequest::Call(call) = effect else {
                return None;
            };
            PresentationImageEffect::from_call(call)
        })
        .fold(previous.to_vec(), |mut active, effect| {
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
                PresentationImageEffect::BackgroundAsset(asset) => {
                    image_objects
                        .iter()
                        .filter(|object| object.asset == asset && object.visible)
                        .cloned()
                        .for_each(|object| upsert_image_object(&mut active, object));
                }
            }
            active.sort_by(|left, right| {
                (left.depth_milli, &left.id).cmp(&(right.depth_milli, &right.id))
            });
            active
        })
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
        visible: named_arg(&call.args, "visible")
            .and_then(parse_bool_arg)
            .unwrap_or(true),
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
            .or_else(|| named_arg(&call.args, "align.x"))
            .and_then(|value| parse_alignment_component_milli(value, "x"))
            .unwrap_or(500),
        y_milli: named_arg(&call.args, "alignment.y")
            .or_else(|| named_arg(&call.args, "align.y"))
            .and_then(|value| parse_alignment_component_milli(value, "y"))
            .unwrap_or(500),
    }
}

fn parse_alignment_component_milli(value: &str, axis: &str) -> Option<i32> {
    match (axis, unquote_arg(value)) {
        ("x", "left" | "start") | ("y", "top" | "start") => return Some(0),
        ("x" | "y", "center" | "middle") => return Some(500),
        ("x", "right" | "end") | ("y", "bottom" | "end") => return Some(1_000),
        _ => {}
    }
    let integer = unquote_arg(value).parse::<i32>().ok()?;
    Some(if (0..=1).contains(&integer) {
        integer.saturating_mul(1_000)
    } else {
        integer.clamp(0, 1_000)
    })
}

fn image_playback_arg(call: &RuntimeCall) -> BundleImageObjectPlayback {
    BundleImageObjectPlayback {
        start_time_millis: named_arg(&call.args, "playback.start")
            .or_else(|| named_arg(&call.args, "playback.start_time"))
            .and_then(parse_duration_millis)
            .unwrap_or_default(),
        rate_milli: named_arg(&call.args, "playback.rate")
            .and_then(parse_rate_milli)
            .unwrap_or(1_000),
        paused_at_millis: named_arg(&call.args, "playback.paused_at")
            .and_then(parse_duration_millis),
        pinned_local_time_millis: named_arg(&call.args, "playback.local_time")
            .or_else(|| named_arg(&call.args, "playback.pinned_local_time"))
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
    let milli = parse_milli_arg(value)?.clamp(0, 1_000);
    u16::try_from(milli).ok()
}

fn parse_rate_milli(value: &str) -> Option<u32> {
    let milli = parse_milli_arg(value)?;
    u32::try_from(milli.max(0)).ok()
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
    millis
        .is_finite()
        .then_some(millis.clamp(0.0, u64::MAX as f64) as u64)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_image_call_accepts_runtime_length_labels() {
        let call = RuntimeCall {
            callee: "image".to_owned(),
            args: vec![
                "asset = @asset:.zundamon.normal".to_owned(),
                "id = \"image.zundamon.stand\"".to_owned(),
                "target = @target.zundamon.stand".to_owned(),
                "layer = @layer.character".to_owned(),
                "x = 760".to_owned(),
                "y = 24".to_owned(),
                "width = 360".to_owned(),
                "height = 600".to_owned(),
                "fit = \"contain\"".to_owned(),
                "alignment.y = \"bottom\"".to_owned(),
            ],
        };

        assert_eq!(
            named_arg(&call.args, "asset").and_then(public_id_arg),
            Some("asset.zundamon.normal".to_owned())
        );
        assert_eq!(
            named_arg(&call.args, "id").and_then(public_id_arg),
            Some("image.zundamon.stand".to_owned())
        );
        assert_eq!(
            named_arg(&call.args, "x").and_then(parse_px_milli),
            Some(760_000)
        );
        let object = inline_image_object(&call).expect("inline image object");

        assert_eq!(object.id, "image.zundamon.stand");
        assert_eq!(object.asset, "asset.zundamon.normal");
        assert_eq!(object.target.as_deref(), Some("target.zundamon.stand"));
        assert_eq!(object.layer.as_deref(), Some("layer.character"));
        assert_eq!(object.bounds.x_milli, 760_000);
        assert_eq!(object.bounds.height_milli, 600_000);
        assert_eq!(object.alignment.y_milli, 1_000);
    }

    #[test]
    fn viewport_effect_sets_and_clears_runtime_fit() {
        let contain = RuntimeCall {
            callee: "player.viewport".to_owned(),
            args: vec![
                "design-width = 1920".to_owned(),
                "design-height = 1080px".to_owned(),
                "fit = \"cover\"".to_owned(),
            ],
        };
        let reset = RuntimeCall {
            callee: "viewport".to_owned(),
            args: vec!["fit = \"default\"".to_owned()],
        };

        let fit = viewport_fit_from_effects(None, &[LineEffectRequest::Call(contain.clone())])
            .expect("viewport fit is set");
        assert_eq!(fit.design_width, 1920);
        assert_eq!(fit.design_height, 1080);
        assert_eq!(fit.scale_policy, ScalePolicy::Cover);

        assert_eq!(
            viewport_fit_from_effects(Some(fit), &[LineEffectRequest::Call(reset)]),
            None
        );
    }
}
