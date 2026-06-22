use arcweft_runtime_driver::session::BundleSessionStep;
use serde::{Deserialize, Serialize};

/// Path-free diagnostic/observation envelope emitted to JavaScript.
///
/// It is not a render protocol and contains no DOM construction instructions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebObservationReport {
    pub schema_version: String,
    pub step_index: usize,
    pub logical_tick: u64,
    pub logical_dt_millis: u64,
    pub stop_reason: String,
    pub status: String,
    pub finished: bool,
    pub diagnostics: Vec<String>,
    pub presentation_revision: u64,
    pub dialogue_present: bool,
    pub choice_count: usize,
    pub flow_event_count: usize,
    pub requested_tasks: usize,
    pub queued_task_events: usize,
}

impl WebObservationReport {
    pub fn from_step(step: &BundleSessionStep, queued_task_events: usize) -> Self {
        Self {
            schema_version: "arcweft.web_observation.v2".to_owned(),
            step_index: step.index,
            logical_tick: step.clock.tick().0,
            logical_dt_millis: step.clock.dt_millis(),
            stop_reason: step.stop_reason_label.clone(),
            status: step.status_label.clone(),
            finished: step.finished,
            diagnostics: step.diagnostics.clone(),
            presentation_revision: step.presentation.revision,
            dialogue_present: step.presentation.dialogue.is_some(),
            choice_count: step.presentation.choices.len(),
            flow_event_count: step.flow_events.len(),
            requested_tasks: step.requested_tasks.len(),
            queued_task_events,
        }
    }
}
