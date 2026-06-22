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
    pub(crate) fn update(&mut self, resolution: &DisplayResolution, status: &FlowFiberStatus) {
        let next_dialogue = resolution
            .frames
            .last()
            .cloned()
            .or_else(|| self.dialogue.clone());
        let next_choices = choices_from_status(status);
        if self.dialogue != next_dialogue || self.choices != next_choices {
            self.revision = self.revision.saturating_add(1);
            self.dialogue = next_dialogue;
            self.choices = next_choices;
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
