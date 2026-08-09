pub mod entry_inventory;

use crate::effect::{LineEffectRequest, RuntimeEffectExpr};
pub use crate::entry::{
    AgentBudget, AgentPolicyHash, CallableContractHash, EntryBindingIdentity, FlowContractHash,
    RuntimeAgentEntryRoles, RuntimeCallableExecutable, RuntimeCallableExecutableCode,
    RuntimeCallableId, RuntimeCallableRole, RuntimeCommandConstructorId, RuntimeCommandContract,
    RuntimeCommandPolicy, RuntimeCommandTargetId, RuntimeEntryRoles, RuntimeFlowExecutable,
    RuntimeFlowExecutableParameter, RuntimeFlowParameterMode, RuntimeFlowRole, RuntimeNominalRole,
    RuntimeNominalTypeId, RuntimeSchemaField, RuntimeSchemaLimits, RuntimeSchemaVariant,
    RuntimeStatefulEntryRoles, RuntimeTypeSchema, RuntimeValueDigest, TypeLayoutHash,
};
use crate::line_task::{LineOutRequest, LineTaskGroup};
use crate::pattern::RuntimePattern;
use crate::runtime_id::{RuntimeIdError, RuntimeIdFamily, RuntimeIdPath, RuntimePublicLabel};
use crate::source::SourcePlan;
use crate::step::RuntimeHostCallMode;
use crate::stream::StreamPlan;
use crate::task::{AwaitManyTarget, AwaitTarget, NeedId, TaskId};
use crate::value::{RuntimeBinding, RuntimeExpr, RuntimeIterator, RuntimePayload};
pub use entry_inventory::{
    EntryRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimePlanError,
    RuntimeRouteBinding, RuntimeRouteBindingSource, RuntimeRouteSpec,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RuntimePlan {
    pub entries: Vec<RuntimeEntrySpec>,
    pub callable_executables: Vec<RuntimeCallableExecutable>,
    pub flow_executables: Vec<RuntimeFlowExecutable>,
    pub flows: Vec<RuntimeFlow>,
    pub pure_helpers: Vec<RuntimePureHelper>,
    pub trait_methods: Vec<RuntimeTraitMethod>,
    pub line_task_groups: Vec<LineTaskGroup>,
    pub stream_plans: Vec<StreamPlan>,
    pub source_plans: Vec<SourcePlan>,
}

/// Runtime identifier for a lowered flow.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FlowRuntimeId {
    path: RuntimeIdPath,
    public_label: RuntimePublicLabel,
}

/// Dynamic runtime Flow target lookup failure.
///
/// Runtime-authored text may select an accepted manual canonical identity
/// exactly, or select one checked/generated declaration through its unique
/// public label. It never reconstructs a checked/generated semantic identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeFlowTargetError {
    #[error(transparent)]
    Invalid(#[from] RuntimeIdError),
    #[error("runtime Flow target `{target}` is not present in the accepted plan")]
    Missing { target: String },
    #[error("runtime Flow target `{target}` matches {matches} accepted declarations")]
    Ambiguous { target: String, matches: usize },
}

/// Runtime identifier for a lowered dialogue line.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeLineId {
    path: RuntimeIdPath,
}

/// Lowered flow program.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeFlow {
    pub id: FlowRuntimeId,
    pub ops: Vec<FlowOp>,
}

impl FlowRuntimeId {
    pub fn canonical(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_canonical_str(RuntimeIdFamily::Flow, value).map(Self::from_runtime_path)
    }

    pub fn from_source_entity_body(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_source_entity_body(
            RuntimeIdFamily::Flow,
            value,
            RuntimeIdFamily::flow_source_families(),
        )
        .map(Self::from_runtime_path)
    }

    pub fn from_runtime_target_value(value: &str) -> Result<Self, RuntimeIdError> {
        let Some((family, _)) = value.split_once('.') else {
            return Self::canonical(value);
        };
        if RuntimeIdFamily::flow_source_families().contains(&family) {
            Self::from_source_entity_body(value)
        } else {
            Self::canonical(value)
        }
    }

    /// Projects one accepted structural Flow declaration into a one-way
    /// runtime identity while retaining its separately selected public label.
    pub fn from_checked_declaration_digest(
        digest: [u8; 32],
        public_id: &str,
    ) -> Result<Self, RuntimeIdError> {
        let public_label = Self::from_source_entity_body(public_id)?.public_label;
        Ok(Self {
            path: RuntimeIdPath::for_checked_flow_declaration(digest),
            public_label,
        })
    }

    pub(crate) fn from_runtime_contract(
        identity: &str,
        public_id: &str,
    ) -> Result<Self, RuntimeIdError> {
        let path = RuntimeIdPath::from_runtime_contract_str(RuntimeIdFamily::Flow, identity)?;
        let public_label = Self::from_source_entity_body(public_id)?.public_label;
        Ok(Self { path, public_label })
    }

    #[must_use]
    pub const fn path(&self) -> &RuntimeIdPath {
        &self.path
    }

    #[must_use]
    pub fn canonical_label(&self) -> String {
        self.path.label()
    }

    #[must_use]
    pub fn public_label(&self) -> RuntimePublicLabel {
        self.public_label.clone()
    }

    /// Selects one exact accepted Flow identity for a runtime-authored target.
    ///
    /// Canonical identities admitted by the public/manual `RuntimePlan`
    /// boundary remain exact. Public labels select only when exactly one
    /// accepted declaration owns that label; checked/generated semantic
    /// identity is never reconstructed from runtime-authored text.
    pub fn resolve_runtime_target<'a>(
        value: &str,
        candidates: impl IntoIterator<Item = &'a Self>,
    ) -> Result<&'a Self, RuntimeFlowTargetError> {
        let projected = Self::from_runtime_target_value(value)?;
        let public_label = projected.public_label();
        let mut public_match = None;
        let mut public_matches = 0_usize;
        for candidate in candidates {
            if *candidate == projected {
                return Ok(candidate);
            }
            if candidate.public_label() == public_label {
                public_matches = public_matches.saturating_add(1);
                public_match.get_or_insert(candidate);
            }
        }
        match (public_match, public_matches) {
            (Some(candidate), 1) => Ok(candidate),
            (None, _) => Err(RuntimeFlowTargetError::Missing {
                target: value.to_owned(),
            }),
            (Some(_), matches) => Err(RuntimeFlowTargetError::Ambiguous {
                target: value.to_owned(),
                matches,
            }),
        }
    }

    fn from_runtime_path(path: RuntimeIdPath) -> Self {
        let public_label = RuntimePublicLabel::for_family(RuntimeIdFamily::Flow, &path);
        Self { path, public_label }
    }
}

impl fmt::Display for FlowRuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.fmt(f)
    }
}

impl RuntimeLineId {
    pub fn canonical(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_canonical_str(RuntimeIdFamily::Line, value).map(|path| Self { path })
    }

    pub fn from_source_entity_body(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_source_entity_body(
            RuntimeIdFamily::Line,
            value,
            RuntimeIdFamily::Line.source_families(),
        )
        .map(|path| Self { path })
    }

    pub fn from_runtime_line_value(value: &str) -> Result<Self, RuntimeIdError> {
        let Some((family, _)) = value.split_once('.') else {
            return Self::canonical(value);
        };
        if RuntimeIdFamily::Line.source_families().contains(&family) {
            Self::from_source_entity_body(value)
        } else {
            Self::canonical(value)
        }
    }

    #[must_use]
    pub const fn path(&self) -> &RuntimeIdPath {
        &self.path
    }

    #[must_use]
    pub fn canonical_label(&self) -> String {
        self.path.label()
    }

    #[must_use]
    pub fn public_label(&self) -> RuntimePublicLabel {
        RuntimePublicLabel::for_family(RuntimeIdFamily::Line, &self.path)
    }
}

impl fmt::Display for RuntimeLineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.fmt(f)
    }
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

/// Runtime identifier for a lowered trait/impl method body.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct RuntimeTraitMethodId(pub usize);

/// Receiver ownership mode selected by the surface method signature.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeReceiverMode {
    Owned,
    SharedRef,
    MutRef,
}

/// Stable identity of a concrete trait method selected through a sema witness.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeTraitMethodIdentity {
    pub impl_id: usize,
    pub trait_id: Option<usize>,
    pub witness: Option<usize>,
    pub trait_name: Option<String>,
    pub self_type: String,
    pub method_name: String,
    pub monomorph_label: String,
}

/// Lowered deterministic trait/impl method body callable by runtime dispatch.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeTraitMethod {
    pub id: RuntimeTraitMethodId,
    pub identity: RuntimeTraitMethodIdentity,
    pub receiver: RuntimeReceiverMode,
    pub input_names: Vec<String>,
    pub input_types: Vec<RuntimePureInputType>,
    pub output_type: RuntimePureOutputType,
    pub body: RuntimeExpr,
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
    Stream,
    Vec,
    Array,
    Slice,
    TupleHomogeneous,
}

/// Lowered witness-backed iterator evidence.
///
/// Runtime dispatch can execute trait-call witnesses; AWBC lowering still
/// requires a typed trait-method table before it can consume them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeIteratorWitnessEvidence {
    pub item_type: String,
    pub into_iter_type: String,
    pub executable: RuntimeIteratorWitnessExecutable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIteratorWitnessExecutable {
    TraitCalls(RuntimeIteratorWitnessCalls),
    IdentityIntoIterator(RuntimeIteratorIdentityWitnessCalls),
    UnsupportedMethodBodyLowering,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeIteratorWitnessCalls {
    pub into_iter: RuntimeTraitMethodId,
    pub next: RuntimeTraitMethodId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeIteratorIdentityWitnessCalls {
    pub next: RuntimeTraitMethodId,
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
    pub const fn builtin_stream() -> Self {
        Self::Builtin(RuntimeBuiltinIteratorEvidence::Stream)
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
            Self::Builtin(RuntimeBuiltinIteratorEvidence::Stream) => Some("stream"),
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
            "stream" => Some(Self::builtin_stream()),
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
    EvaluatedEffect(RuntimeEffectExpr),
    RegisterCleanup {
        key: String,
        effect: LineEffectRequest,
    },
    CancelCleanup {
        key: String,
    },
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

impl RuntimePlan {
    pub fn new(
        flows: Vec<RuntimeFlow>,
        line_task_groups: Vec<LineTaskGroup>,
    ) -> Result<Self, RuntimePlanError> {
        Ok(Self {
            entries: Vec::new(),
            callable_executables: Vec::new(),
            flow_executables: Vec::new(),
            flows,
            pure_helpers: Vec::new(),
            trait_methods: Vec::new(),
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
    pub fn with_trait_methods(mut self, trait_methods: Vec<RuntimeTraitMethod>) -> Self {
        self.trait_methods = trait_methods;
        self
    }

    pub fn lines_only(line_task_groups: Vec<LineTaskGroup>) -> Self {
        Self {
            entries: Vec::new(),
            callable_executables: Vec::new(),
            flow_executables: Vec::new(),
            flows: Vec::new(),
            pure_helpers: Vec::new(),
            trait_methods: Vec::new(),
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

    /// Resolves one dynamic target against the exact accepted Flow inventory.
    ///
    /// A legacy canonical runtime ID still selects itself exactly. Otherwise
    /// the validated public label must identify one and only one accepted Flow;
    /// duplicate module-local labels are a terminal ambiguity.
    pub fn resolve_flow_target_value(
        &self,
        value: &str,
    ) -> Result<FlowRuntimeId, RuntimeFlowTargetError> {
        FlowRuntimeId::resolve_runtime_target(value, self.flows.iter().map(|flow| &flow.id))
            .cloned()
    }
}
