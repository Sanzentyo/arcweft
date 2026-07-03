use crate::effect::LineEffectRequest;
use crate::line_task::{LineOutRequest, LineTaskGroup};
use crate::pattern::RuntimePattern;
use crate::source::SourcePlan;
use crate::step::RuntimeHostCallMode;
use crate::stream::StreamPlan;
use crate::task::{AwaitManyTarget, AwaitTarget, NeedId, TaskId};
use crate::value::{RuntimeBinding, RuntimeExpr, RuntimeIterator, RuntimePayload};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RuntimePlan {
    pub entry_flow: Option<FlowRuntimeId>,
    pub entries: Vec<RuntimeEntrySpec>,
    pub flows: Vec<RuntimeFlow>,
    pub pure_helpers: Vec<RuntimePureHelper>,
    pub line_task_groups: Vec<LineTaskGroup>,
    pub stream_plans: Vec<StreamPlan>,
    pub source_plans: Vec<SourcePlan>,
}

/// Runtime identifier for a lowered flow.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FlowRuntimeId(pub String);

/// Runtime identifier for a source-declared entry.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EntryRuntimeId(pub String);

/// Adapter family of a source-declared entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeEntryKind {
    Game,
    Cli,
    Server,
    Activity,
    Test,
    Bench,
    Custom(String),
}

/// Launch target selected by an entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeEntryTarget {
    Flow(FlowRuntimeId),
    Routes(Vec<RuntimeRouteSpec>),
}

/// Route declaration in a server-like entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeRouteSpec {
    pub method: String,
    pub path: String,
    pub target: FlowRuntimeId,
    pub bindings: Vec<RuntimeRouteBinding>,
}

/// Explicit route parameter binding for a target flow invocation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeRouteBinding {
    pub name: String,
    pub source: RuntimeRouteBindingSource,
}

/// Adapter route value source used by a route binding.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeRouteBindingSource {
    PathParam(String),
}

/// Lowered entry declaration preserved for CLI/LSP/runtime launch selection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeEntrySpec {
    pub id: EntryRuntimeId,
    pub kind: RuntimeEntryKind,
    pub target: RuntimeEntryTarget,
}

/// Runtime identifier for a lowered dialogue line.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeLineId(pub String);

/// Lowered flow program.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RuntimeFlow {
    pub id: FlowRuntimeId,
    pub ops: Vec<FlowOp>,
}

/// Runtime identifier for a lowered deterministic pure helper.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct RuntimePureHelperId(pub usize);

/// Lowered deterministic pure helper callable from runtime expressions.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimePureHelper {
    pub id: RuntimePureHelperId,
    pub name: String,
    pub input_names: Vec<String>,
    pub input_types: Vec<RuntimePureInputType>,
    pub output_type: RuntimePureOutputType,
    pub expr: RuntimeExpr,
    pub scalar_eval_supported: bool,
    pub origin: RuntimePureHelperOrigin,
}

/// Runtime pure helper input representation.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimePureInputType {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    Value,
}

/// Runtime pure helper output representation.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimePureOutputType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    Value,
}

/// Source of a runtime pure helper candidate.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimePureHelperOrigin {
    Annotated,
    Inferred,
}

/// Serializable evidence that a `for` source was resolved through the standard
/// `IntoIterator` / `Iterator` contract before runtime lowering.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIteratorEvidence {
    Builtin(RuntimeBuiltinIteratorEvidence),
    Witness(RuntimeIteratorWitnessEvidence),
}

/// Built-in iterator families that lower directly to runtime iterator state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeBuiltinIteratorEvidence {
    Range,
    Seq,
    Vec,
    Array,
    Slice,
    TupleHomogeneous,
}

/// Lowered witness-backed iterator evidence.
///
/// The current runtime can carry this evidence but intentionally rejects it
/// until trait method bodies lower to executable runtime calls.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeIteratorWitnessEvidence {
    pub item_type: String,
    pub into_iter_type: String,
    pub executable: RuntimeIteratorWitnessExecutable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIteratorWitnessExecutable {
    StaticCalls,
    UnsupportedMethodBodyLowering,
}

impl RuntimeIteratorEvidence {
    #[must_use]
    pub const fn builtin_range() -> Self {
        Self::Builtin(RuntimeBuiltinIteratorEvidence::Range)
    }

    #[must_use]
    pub const fn builtin_seq() -> Self {
        Self::Builtin(RuntimeBuiltinIteratorEvidence::Seq)
    }

    #[must_use]
    pub const fn builtin_vec() -> Self {
        Self::Builtin(RuntimeBuiltinIteratorEvidence::Vec)
    }

    #[must_use]
    pub const fn builtin_array() -> Self {
        Self::Builtin(RuntimeBuiltinIteratorEvidence::Array)
    }

    #[must_use]
    pub const fn builtin_slice() -> Self {
        Self::Builtin(RuntimeBuiltinIteratorEvidence::Slice)
    }

    #[must_use]
    pub const fn builtin_tuple_homogeneous() -> Self {
        Self::Builtin(RuntimeBuiltinIteratorEvidence::TupleHomogeneous)
    }

    #[must_use]
    pub const fn awbc_label(&self) -> Option<&'static str> {
        match self {
            Self::Builtin(RuntimeBuiltinIteratorEvidence::Range) => Some("range"),
            Self::Builtin(RuntimeBuiltinIteratorEvidence::Seq) => Some("seq"),
            Self::Builtin(RuntimeBuiltinIteratorEvidence::Vec) => Some("vec"),
            Self::Builtin(RuntimeBuiltinIteratorEvidence::Array) => Some("array"),
            Self::Builtin(RuntimeBuiltinIteratorEvidence::Slice) => Some("slice"),
            Self::Builtin(RuntimeBuiltinIteratorEvidence::TupleHomogeneous) => {
                Some("tuple_homogeneous")
            }
            Self::Witness(_) => None,
        }
    }

    #[must_use]
    pub fn from_awbc_label(label: &str) -> Option<Self> {
        match label {
            "range" => Some(Self::builtin_range()),
            "seq" => Some(Self::builtin_seq()),
            "vec" => Some(Self::builtin_vec()),
            "array" => Some(Self::builtin_array()),
            "slice" => Some(Self::builtin_slice()),
            "tuple_homogeneous" => Some(Self::builtin_tuple_homogeneous()),
            _ => None,
        }
    }
}

/// Runtime identifier for a lowered stream transform.

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum FlowOp {
    Bind(Vec<RuntimeBinding>),
    Let {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    LetElse {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        else_ops: Vec<FlowOp>,
    },
    Dialogue {
        line: RuntimeLineId,
        task_group: usize,
    },
    Choice {
        id: Option<String>,
        options: Vec<ChoiceRuntimeOption>,
    },
    Await {
        binding: Option<RuntimePattern>,
        target: AwaitTarget,
        pending: Vec<LineEffectRequest>,
    },
    AwaitMany {
        binding: Option<RuntimePattern>,
        target: AwaitManyTarget,
        pending: Vec<LineEffectRequest>,
    },
    HostCall {
        binding: Option<RuntimePattern>,
        target: RuntimeHostCallTarget,
    },
    If {
        condition: RuntimeExpr,
        then_ops: Vec<FlowOp>,
        else_ops: Vec<FlowOp>,
    },
    IfLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        then_ops: Vec<FlowOp>,
        else_ops: Vec<FlowOp>,
    },
    Match {
        scrutinee: RuntimeExpr,
        arms: Vec<RuntimeMatchArm>,
    },
    Loop {
        body: Vec<FlowOp>,
    },
    LetLoop {
        pattern: RuntimePattern,
        body: Vec<FlowOp>,
    },
    LoopNext {
        body: Arc<[FlowOp]>,
    },
    While {
        condition: RuntimeExpr,
        body: Vec<FlowOp>,
    },
    WhileNext {
        condition: RuntimeExpr,
        body: Arc<[FlowOp]>,
    },
    WhileLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Vec<FlowOp>,
    },
    WhileLetNext {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Arc<[FlowOp]>,
    },
    For {
        pattern: RuntimePattern,
        source: RuntimeExpr,
        evidence: RuntimeIteratorEvidence,
        body: Vec<FlowOp>,
    },
    ForNext {
        pattern: RuntimePattern,
        iterator: RuntimeIterator,
        evidence: RuntimeIteratorEvidence,
        body: Arc<[FlowOp]>,
    },
    Thread {
        name: Option<String>,
        body: Vec<FlowOp>,
    },
    Scope(Vec<FlowOp>),
    LetScope {
        pattern: RuntimePattern,
        ops: Vec<FlowOp>,
        value: RuntimeExpr,
    },
    Break(Option<RuntimeExpr>),
    Continue,
    Goto(FlowRuntimeId),
    GotoExpr(RuntimeExpr),
    Return(String),
    ReturnExpr(RuntimeExpr),
    Effect(LineEffectRequest),
    EnterScope,
    ExitScope,
    ExitScopeBind {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    Noop,
}

/// Direct host-call request surface for runtime-step hosts.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeHostCallTarget {
    pub public_id: String,
    pub capability: String,
    pub operation: String,
    pub args: Vec<RuntimeExpr>,
    pub mode: RuntimeHostCallMode,
    pub deterministic: bool,
}

impl RuntimeHostCallTarget {
    pub fn new(
        public_id: impl Into<String>,
        capability: impl Into<String>,
        operation: impl Into<String>,
        args: impl IntoIterator<Item = RuntimeExpr>,
        mode: RuntimeHostCallMode,
        deterministic: bool,
    ) -> Self {
        Self {
            public_id: public_id.into(),
            capability: capability.into(),
            operation: operation.into(),
            args: args.into_iter().collect(),
            mode,
            deterministic,
        }
    }
}

/// One executable `match` arm in the runtime flow model.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub ops: Vec<FlowOp>,
}

pub(crate) type RuntimeMatchSelection = Option<(Vec<RuntimeBinding>, Vec<FlowOp>)>;

/// Runtime choice option visible to adapters and selectable from `RuntimeStepInput`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChoiceRuntimeOption {
    pub id: Option<String>,
    pub label: String,
    pub target: Option<FlowRuntimeId>,
    pub out: Option<LineOutRequest>,
    pub effects: Vec<LineEffectRequest>,
}

/// Replay-observable flow event emitted by the core runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum FlowEvent {
    DialogueLine {
        line: RuntimeLineId,
        bindings: Vec<RuntimeBinding>,
    },
    LineCancelled {
        trigger: String,
    },
    ChoicePresented {
        id: Option<String>,
        options: Vec<ChoiceRuntimeOption>,
    },
    ChoiceSelected {
        id: Option<String>,
        option: String,
    },
    AwaitStarted {
        need: NeedId,
        task: TaskId,
    },
    AwaitReady {
        need: NeedId,
        value: RuntimePayload,
    },
    AwaitProgress {
        need: NeedId,
        progress: RuntimePayload,
    },
    Goto {
        target: FlowRuntimeId,
    },
    Return {
        value: String,
    },
    Done,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimePlanError {
    #[error("entry flow `{0}` does not exist in runtime plan")]
    MissingEntryFlow(String),
}

impl RuntimePlan {
    pub fn new(
        entry_flow: Option<FlowRuntimeId>,
        flows: Vec<RuntimeFlow>,
        line_task_groups: Vec<LineTaskGroup>,
    ) -> Result<Self, RuntimePlanError> {
        if let Some(entry) = entry_flow.as_ref()
            && !flows.iter().any(|flow| flow.id == *entry)
        {
            return Err(RuntimePlanError::MissingEntryFlow(entry.0.clone()));
        }
        Ok(Self {
            entry_flow,
            entries: Vec::new(),
            flows,
            pure_helpers: Vec::new(),
            line_task_groups,
            stream_plans: Vec::new(),
            source_plans: Vec::new(),
        })
    }

    #[must_use]
    pub fn with_generation_plans(
        mut self,
        stream_plans: Vec<StreamPlan>,
        source_plans: Vec<SourcePlan>,
    ) -> Self {
        self.stream_plans = stream_plans;
        self.source_plans = source_plans;
        self
    }

    #[must_use]
    pub fn with_pure_helpers(mut self, pure_helpers: Vec<RuntimePureHelper>) -> Self {
        self.pure_helpers = pure_helpers;
        self
    }

    #[must_use]
    pub fn with_entries(mut self, entries: Vec<RuntimeEntrySpec>) -> Self {
        self.entries = entries;
        self
    }

    pub fn lines_only(line_task_groups: Vec<LineTaskGroup>) -> Self {
        Self {
            entry_flow: None,
            entries: Vec::new(),
            flows: Vec::new(),
            pure_helpers: Vec::new(),
            line_task_groups,
            stream_plans: Vec::new(),
            source_plans: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.flows.is_empty()
            && self.line_task_groups.is_empty()
            && self.stream_plans.is_empty()
            && self.source_plans.is_empty()
    }
}

impl From<&str> for FlowRuntimeId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&str> for EntryRuntimeId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&str> for RuntimeLineId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}
