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
    /// Line option assignments such as `voice = auto`.
    pub options: Vec<LineOptionRequest>,
    /// Bindings introduced by `let PAT = EXPR` in a line plan.
    pub bindings: Vec<LineBindingRequest>,
    /// Values exported from the line plan with `out`.
    pub out: Vec<LineOutRequest>,
    /// Cancellation branches attached to this line.
    pub cancel_rules: Vec<LineCancelRuleRequest>,
    /// Memoization directives local to this line plan.
    pub memo: Vec<LineMemoRequest>,
    /// Runtime-checkable assertions attached to this line plan.
    pub assertions: Vec<LineAssertionRequest>,
    /// Cleanup registered by `defer` inside the init scope.
    pub init_defer_stack: Vec<Vec<LineEffectRequest>>,
    /// Line-scoped child tasks such as `thread` and `on .mark` handlers.
    pub children: Vec<LineChildTask>,
    /// Cleanup registered directly on the line scope.
    pub defer_stack: Vec<Vec<LineEffectRequest>>,
    /// Cleanup registered with `defer on completed`.
    pub completed_defer_stack: Vec<Vec<LineEffectRequest>>,
    /// Cleanup registered with `defer on cancelled`.
    pub cancelled_defer_stack: Vec<Vec<LineEffectRequest>>,
    /// Cleanup registered with `defer on failed`.
    pub failed_defer_stack: Vec<Vec<LineEffectRequest>>,
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

/// Option assignment preserved from a line plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineOptionRequest {
    pub name: String,
    pub value: String,
}

/// Binding preserved from a line plan before full HIR execution exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineBindingRequest {
    pub pattern: String,
    pub value: String,
}

/// `out` value exported from a line plan or cancel branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineOutRequest {
    pub label: Option<String>,
    pub value: String,
}

/// Runtime representation of `cancel on ... { ... }`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineCancelRuleRequest {
    pub trigger: String,
    pub action: Vec<LineEffectRequest>,
}

/// Line-local memo directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineMemoRequest {
    pub name: String,
    pub options: Vec<RuntimeField>,
}

/// Runtime-checkable line assertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineAssertionRequest {
    pub debug: bool,
    pub expr: String,
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
    RegisterHandle {
        key: String,
        handle: String,
    },
    DropHandle {
        key: String,
    },
    WaitMark(String),
    Wait(LogicalDuration),
    Call(RuntimeCall),
    Log(RuntimeLog),
    SignalWrite(RuntimeAssignment),
    MetricWrite(RuntimeAssignment),
    EmitEvent(RuntimeEvent),
    Command(RuntimeCommand),
    Out(LineOutRequest),
    Return(String),
    Goto(String),
    Yield(String),
    Panic(String),
    Fail(String),
    Bail(String),
    Ensure {
        condition: String,
        message: String,
    },
    Close(String),
    Select(String),
    Break {
        label: Option<String>,
        value: Option<String>,
    },
    Continue {
        label: Option<String>,
    },
    DeferOn {
        outcome: String,
        effects: Vec<LineEffectRequest>,
    },
}

/// Call-shaped runtime request. The runtime adapter decides whether the callee
/// maps to presentation, audio, state, or user code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCall {
    pub callee: String,
    pub args: Vec<String>,
}

/// Structured log request preserved for defmt-style template interning later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeLog {
    pub level: String,
    pub message: String,
    pub fields: Vec<RuntimeField>,
}

/// Assignment-like runtime request used by signal and metric updates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssignment {
    pub target: String,
    pub value: String,
}

/// Structured event emission request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEvent {
    pub event: String,
    pub fields: Vec<RuntimeField>,
}

/// Statement-like command retained until the command family is canonicalized as
/// ordinary calls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCommand {
    pub name: String,
    pub args: Vec<String>,
}

/// Named expression payload preserved in runtime IR without performing I/O.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeField {
    pub name: String,
    pub value: String,
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
