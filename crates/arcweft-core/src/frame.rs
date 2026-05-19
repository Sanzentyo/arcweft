use crate::effect::LineEffectRequest;
use crate::plan::FlowEvent;
use crate::source::{SourceEvent, SourceId};
use crate::stream::StreamEvent;
use crate::task::{CancelScopeId, TaskEvent, TaskSpec};
use crate::time::{LogicalDuration, TickId};
use crate::value::RuntimeBinding;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
    pub external_values: Vec<RuntimeBinding>,
    pub input_events: Vec<InputEvent>,
    pub task_events: Vec<TaskEvent>,
    pub ui_events: Vec<UiEvent>,
    pub audio_events: Vec<AudioEvent>,
    pub source_events: Vec<SourceEvent<String, String>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameOutput {
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub flow_events: Vec<FlowEvent>,
    pub line_effects: Vec<LineEffectRequest>,
    pub task_requests: Vec<TaskSpec>,
    pub cancel_requests: Vec<CancelScopeId>,
    pub source_events: Vec<SourceEvent<String, String>>,
    pub stream_events: Vec<StreamEvent<String, String>>,
    pub source_close_requests: Vec<SourceId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDiagnostic {
    pub message: String,
}

/// Named value provided by adapters or earlier runtime operations.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputEvent {
    pub kind: String,
    pub payload: Option<String>,
}

/// UI event placeholder kept as Sans I/O data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiEvent {
    pub kind: String,
    pub payload: Option<String>,
}

/// Audio event placeholder kept as Sans I/O data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioEvent {
    pub kind: String,
    pub payload: Option<String>,
}

impl FrameOutput {
    pub(crate) fn merge(&mut self, other: Self) {
        self.diagnostics.extend(other.diagnostics);
        self.flow_events.extend(other.flow_events);
        self.line_effects.extend(other.line_effects);
        self.task_requests.extend(other.task_requests);
        self.cancel_requests.extend(other.cancel_requests);
        self.source_events.extend(other.source_events);
        self.stream_events.extend(other.stream_events);
        self.source_close_requests
            .extend(other.source_close_requests);
    }
}
