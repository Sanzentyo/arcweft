use crate::effect::LineEffectRequest;
use crate::engine::FlowCursor;
use crate::line_task::{LineOutRequest, LineTaskGroup};
use crate::pattern::RuntimePattern;
use crate::source::SourcePlan;
use crate::stream::StreamPlan;
use crate::task::{AwaitManyTarget, AwaitTarget, NeedId, TaskId};
use crate::value::{RuntimeBinding, RuntimeExpr, RuntimeValue};
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FlowRuntimeId(pub String);

/// Runtime identifier for a source-declared entry.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntryRuntimeId(pub String);

/// Adapter family of a source-declared entry.
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeEntryTarget {
    Flow(FlowRuntimeId),
    Routes(Vec<RuntimeRouteSpec>),
}

/// Route declaration in a server-like entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRouteSpec {
    pub method: String,
    pub path: String,
    pub target: FlowRuntimeId,
    pub bindings: Vec<RuntimeRouteBinding>,
}

/// Explicit route parameter binding for a target flow invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRouteBinding {
    pub name: String,
    pub source: RuntimeRouteBindingSource,
}

/// Adapter route value source used by a route binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeRouteBindingSource {
    PathParam(String),
}

/// Lowered entry declaration preserved for CLI/LSP/runtime launch selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEntrySpec {
    pub id: EntryRuntimeId,
    pub kind: RuntimeEntryKind,
    pub target: RuntimeEntryTarget,
}

/// Runtime identifier for a lowered dialogue line.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeLineId(pub String);

/// Lowered flow program.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeFlow {
    pub id: FlowRuntimeId,
    pub ops: Vec<FlowOp>,
}

/// Runtime identifier for a lowered deterministic pure helper.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimePureHelperId(pub usize);

/// Lowered deterministic pure helper callable from runtime expressions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePureHelper {
    pub id: RuntimePureHelperId,
    pub name: String,
    pub input_names: Vec<String>,
    pub expr: RuntimeExpr,
    pub scalar_eval_supported: bool,
    pub origin: RuntimePureHelperOrigin,
}

/// Source of a runtime pure helper candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePureHelperOrigin {
    Annotated,
    Inferred,
}

/// Runtime identifier for a lowered stream transform.

#[derive(Clone, Debug, Eq, PartialEq)]
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
        body: Vec<FlowOp>,
    },
    ForNext {
        pattern: RuntimePattern,
        items: Arc<[RuntimeValue]>,
        index: usize,
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

/// One executable `match` arm in the runtime flow model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub ops: Vec<FlowOp>,
}

pub(crate) type RuntimeMatchSelection = Option<(Vec<RuntimeBinding>, Vec<FlowOp>)>;

/// Runtime choice option visible to adapters and selectable from `RuntimeStepInput`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceRuntimeOption {
    pub id: Option<String>,
    pub label: String,
    pub target: Option<FlowRuntimeId>,
    pub out: Option<LineOutRequest>,
    pub effects: Vec<LineEffectRequest>,
}

/// Replay-observable flow event emitted by the core runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowEvent {
    DialogueLine { line: RuntimeLineId },
    LineCancelled { trigger: String },
    ChoicePresented { id: Option<String> },
    ChoiceSelected { id: Option<String>, option: String },
    AwaitStarted { need: NeedId, task: TaskId },
    AwaitReady { need: NeedId, value: String },
    AwaitProgress { need: NeedId, progress: String },
    Goto { target: FlowRuntimeId },
    Return { value: String },
    Done,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
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

    pub fn entry_cursor(&self) -> Option<FlowCursor> {
        self.entry_flow.as_ref().map(|flow| FlowCursor {
            flow: flow.clone(),
            op_index: 0,
        })
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
