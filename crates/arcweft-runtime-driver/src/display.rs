use arcweft_bundle::BundleImageObject;
use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::plan::FlowEvent;
use arcweft_render_text::{LineDisplayCatalog, LineDisplayFrame, RuntimeLineContext};
use serde::{Deserialize, Serialize};

/// Choice metadata shared by native and Web presentation hosts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleChoice {
    pub id: String,
    pub label: String,
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
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct BundlePresentationSnapshot {
    pub revision: u64,
    pub dialogue: Option<LineDisplayFrame>,
    pub choices: Vec<BundleChoice>,
    pub images: Vec<BundleImageObject>,
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
        image_objects: &[BundleImageObject],
    ) {
        let next_dialogue = resolution
            .frames
            .last()
            .cloned()
            .or_else(|| self.dialogue.clone());
        let next_choices = choices_from_status(status);
        let next_images = images_from_effects(&self.images, effects, image_objects);
        if self.dialogue != next_dialogue
            || self.choices != next_choices
            || self.images != next_images
        {
            self.revision = self.revision.saturating_add(1);
            self.dialogue = next_dialogue;
            self.choices = next_choices;
            self.images = next_images;
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum PresentationImageEffect {
    Object(String),
    BackgroundAsset(String),
}

impl PresentationImageEffect {
    fn from_call(call: &RuntimeCall) -> Option<Self> {
        match call.callee.as_str() {
            "image" | "image.show" => call
                .args
                .first()
                .or_else(|| named_arg(&call.args, "id"))
                .and_then(|arg| public_id_arg(arg))
                .filter(|id| id.starts_with("image."))
                .map(Self::Object),
            "bg" | "background" => call
                .args
                .first()
                .or_else(|| named_arg(&call.args, "asset"))
                .and_then(|arg| public_id_arg(arg))
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
                    if let Some(object) = image_objects.iter().find(|object| object.id == id) {
                        upsert_image_object(&mut active, object.clone());
                    }
                }
                PresentationImageEffect::BackgroundAsset(asset) => {
                    image_objects
                        .iter()
                        .filter(|object| object.asset == asset)
                        .cloned()
                        .for_each(|object| upsert_image_object(&mut active, object));
                }
            }
            active.sort_by(|left, right| left.id.cmp(&right.id));
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

fn named_arg<'a>(args: &'a [String], name: &str) -> Option<&'a String> {
    args.iter().find(|arg| {
        arg.split_once(" = ")
            .is_some_and(|(arg_name, _)| arg_name.trim() == name)
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
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        .then(|| value.to_owned())
}
