pub mod prelude {
    pub use arcweft_dialogue::{
        CancelAction, CancelOnDrop, CancelRule, CancelScope, CancelTrigger, Cue, CueAction,
        DialogueBuildError, DialogueBuildErrorKind, DialogueContent, DialogueContentPart,
        DialogueLine, DialogueLineBuilder, DialogueOptions, DialogueTag, InputEventKind, LineExit,
        LinePlan, LinePlanBuilder, LinePlanStep, OutPayload, PlanArg, PlanCall, PlanExpr,
        SayOptions, SpeakerPreset, SpeakerRef, TagArg, TextBoxRef, TimelineAnchor, TimelineCue,
        VoicePolicy, VoiceRef, character, line_id, textbox,
    };
    pub use arcweft_id::{EntityId, IdError, IdErrorKind, PublicId, TextKey};
    pub use arcweft_need::{Need, Progress, ProgressError};
    pub use arcweft_source::{SourceAnchor, SourceName, SourcePosition};
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TickId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogicalDuration {
    nanos: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameOutput {
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub line_effects: Vec<LineEffectRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDiagnostic {
    pub message: String,
}

/// Sans I/O runtime model for a dialogue line's scoped task group.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LineTaskGroup {
    /// Effects that run before line reveal and child tasks start.
    pub init: Vec<LineEffectRequest>,
    /// Cleanup registered by `defer` inside the init scope.
    pub init_defer_stack: Vec<Vec<LineEffectRequest>>,
    /// Line-scoped child tasks such as `thread` and `on .mark` handlers.
    pub children: Vec<LineChildTask>,
    /// Cleanup registered directly on the line scope.
    pub defer_stack: Vec<Vec<LineEffectRequest>>,
    /// Explicit line-level finalizer effects.
    pub finally: Vec<LineEffectRequest>,
    /// Automatic cleanup policy for line-owned handles and child tasks.
    pub cleanup: LineCleanupPolicy,
}

/// A child task declared by `thread name { ... }` inside a line plan.
///
/// Thread-local cleanup is modeled as a scoped defer stack, not as line-level
/// `finally`. That keeps cancellation semantics identical for flow, handler,
/// and line-plan threads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineChildTask {
    pub name: Option<String>,
    pub body: Vec<LineEffectRequest>,
    pub defer_stack: Vec<Vec<LineEffectRequest>>,
}

/// Declarative cleanup policy applied when the line scope exits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LineCleanupPolicy {
    pub child_tasks: ChildTaskCleanup,
    pub presentation: PresentationCleanup,
    pub audio: AudioCleanup,
}

/// How line-scoped child tasks are treated on cleanup.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChildTaskCleanup {
    #[default]
    CancelAndJoin,
    Detach,
    Finish,
}

/// How presentation handles registered in the line lifetime are cleaned up.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PresentationCleanup {
    #[default]
    DropRegistered,
    KeepRegistered,
}

/// How line-scoped audio handles are cleaned up.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AudioCleanup {
    #[default]
    StopRegistered,
    FadeRegistered,
    KeepRegistered,
}

/// Effect request emitted by core runtime without performing the effect itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineEffectRequest {
    RegisterHandle { key: String, handle: String },
    DropHandle { key: String },
    WaitMark(String),
    Wait(LogicalDuration),
    EmitSignal(String),
}

impl LogicalDuration {
    pub const fn from_nanos(nanos: u64) -> Self {
        Self { nanos }
    }

    pub const fn as_nanos(self) -> u64 {
        self.nanos
    }
}

impl Default for LogicalDuration {
    fn default() -> Self {
        Self::from_nanos(0)
    }
}
