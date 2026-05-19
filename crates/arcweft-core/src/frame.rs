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

/// Borrowed adapter-facing view of frame inputs.
///
/// Adapters should prefer this view when handing input data into lower runtime
/// layers. The view keeps ownership at the adapter/frame boundary and makes it
/// clear that runtime code must not retain borrowed event slices past the frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameInputView<'a> {
    tick: TickId,
    dt: LogicalDuration,
    external_values: &'a [RuntimeBinding],
    input_events: &'a [InputEvent],
    task_events: &'a [TaskEvent],
    ui_events: &'a [UiEvent],
    audio_events: &'a [AudioEvent],
    source_events: &'a [SourceEvent<String, String>],
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

/// Mutable adapter-facing writer for frame outputs.
///
/// The writer gives adapter/runtime integration code a scoped output sink
/// without transferring ownership of the whole `FrameOutput` value.
pub struct FrameOutputWriter<'a> {
    output: &'a mut FrameOutput,
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

impl FrameInput {
    pub fn as_view(&self) -> FrameInputView<'_> {
        FrameInputView {
            tick: self.tick,
            dt: self.dt,
            external_values: self.external_values.as_slice(),
            input_events: self.input_events.as_slice(),
            task_events: self.task_events.as_slice(),
            ui_events: self.ui_events.as_slice(),
            audio_events: self.audio_events.as_slice(),
            source_events: self.source_events.as_slice(),
        }
    }
}

impl<'a> FrameInputView<'a> {
    pub const fn tick(&self) -> TickId {
        self.tick
    }

    pub const fn dt(&self) -> LogicalDuration {
        self.dt
    }

    pub const fn external_values(&self) -> &'a [RuntimeBinding] {
        self.external_values
    }

    pub const fn input_events(&self) -> &'a [InputEvent] {
        self.input_events
    }

    pub const fn task_events(&self) -> &'a [TaskEvent] {
        self.task_events
    }

    pub const fn ui_events(&self) -> &'a [UiEvent] {
        self.ui_events
    }

    pub const fn audio_events(&self) -> &'a [AudioEvent] {
        self.audio_events
    }

    pub const fn source_events(&self) -> &'a [SourceEvent<String, String>] {
        self.source_events
    }
}

impl FrameOutput {
    pub fn writer(&mut self) -> FrameOutputWriter<'_> {
        FrameOutputWriter::new(self)
    }

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

impl<'a> FrameOutputWriter<'a> {
    pub const fn new(output: &'a mut FrameOutput) -> Self {
        Self { output }
    }

    pub fn output(&self) -> &FrameOutput {
        self.output
    }

    pub fn output_mut(&mut self) -> &mut FrameOutput {
        self.output
    }

    pub fn push_diagnostic(&mut self, message: impl Into<String>) {
        self.output.diagnostics.push(RuntimeDiagnostic {
            message: message.into(),
        });
    }

    pub fn merge(&mut self, other: FrameOutput) {
        self.output.merge(other);
    }
}
