use crate::pattern::RuntimeCheckedType;
use crate::value::{RuntimeExpr, RuntimePayload};
use arcweft_need::Need;
use serde::{Deserialize, Serialize};

use crate::runtime_id::RuntimeLocalDeclarationId;

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TaskId(pub String);

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TaskKey(pub String);

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct NeedId(pub String);

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CancelScopeId(pub String);

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct LogicalEpoch(pub u64);

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct TaskSequence(pub u64);

/// One producer-owned, in-memory state publication for a typed `Need<T, E>`.
///
/// This boundary deliberately does not add a `RuntimeValue` or AWBC wire
/// surrogate. The handle carried by a verified `NeedHandle` register names the
/// `NeedId`; the producer publishes the typed success/error payload here for
/// the current deterministic runtime step.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedState {
    logical_epoch: LogicalEpoch,
    need: NeedId,
    sequence: TaskSequence,
    state: Need<RuntimePayload, RuntimePayload>,
}

/// The typed terminal outcomes a host task may publish.
///
/// `Ready` and `Error` are both completed task outcomes.  `Failed` is reserved
/// for an infrastructure failure which cannot be represented by the task's
/// authored error type and therefore cannot be resumed through the normal
/// `Need<T, E>` boundary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskOutcomeContract {
    pub ready: RuntimeCheckedType,
    pub error: RuntimeCheckedType,
}

impl TaskOutcomeContract {
    #[must_use]
    pub const fn new(ready: RuntimeCheckedType, error: RuntimeCheckedType) -> Self {
        Self { ready, error }
    }
}

impl Default for TaskOutcomeContract {
    fn default() -> Self {
        Self {
            ready: RuntimeCheckedType::Unit,
            error: RuntimeCheckedType::String,
        }
    }
}

impl RuntimeNeedState {
    pub const fn new(
        logical_epoch: LogicalEpoch,
        need: NeedId,
        sequence: TaskSequence,
        state: Need<RuntimePayload, RuntimePayload>,
    ) -> Self {
        Self {
            logical_epoch,
            need,
            sequence,
            state,
        }
    }

    pub const fn logical_epoch(&self) -> LogicalEpoch {
        self.logical_epoch
    }

    pub const fn need(&self) -> &NeedId {
        &self.need
    }

    pub const fn sequence(&self) -> TaskSequence {
        self.sequence
    }

    pub const fn state(&self) -> &Need<RuntimePayload, RuntimePayload> {
        &self.state
    }
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct TaskPriority(pub i32);

#[derive(Clone, Debug, PartialEq)]
pub struct AwaitTarget {
    pub need: NeedId,
    pub task: TaskId,
    pub outcome: TaskOutcomeContract,
    pub request: HostTaskRequestTemplate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwaitManyTarget {
    pub need: NeedId,
    pub task: TaskId,
    pub outcome: TaskOutcomeContract,
    pub source: RuntimeExpr,
    pub item_binding: RuntimeLocalDeclarationId,
    pub limit: usize,
    pub request: HostTaskRequestTemplate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRequestTemplate {
    pub capability: HostCapabilityId,
    pub operation: String,
    pub args: Vec<RuntimeHostArgumentTemplate>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeHostArgumentTemplate {
    Positional(RuntimeExpr),
    Named(NamedHostArg<RuntimeExpr>),
    Spread(RuntimeExpr),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NamedHostArg<T> {
    pub name: String,
    pub value: T,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskSpec {
    pub id: TaskId,
    pub key: TaskKey,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContract,
    pub request: HostTaskRequest,
    pub debug_label: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskHandle {
    pub id: TaskId,
    pub key: TaskKey,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SchedulerBudget {
    pub max_events: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum TaskClass {
    LocalView,
    Io,
    Cpu,
    GpuPrepare,
    ShaderCompile,
    WasmCall,
    AssetDecode,
    AudioDecode,
    AudioRender,
    TtsSynthesis,
    BgmPrecompose,
    Lsp,
    Background,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum TaskPolicy {
    JoinSameKey,
    AlwaysStart,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct HostCapabilityId(pub String);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum HostTaskRequest {
    FileReadText(FileReadTextRequest),
    FileReadBytes(FileReadBytesRequest),
    FileWriteText(FileWriteTextRequest),
    FileWriteBytes(FileWriteBytesRequest),
    HttpFetch(HttpFetchRequest),
    HttpRespond(HttpRespondRequest),
    ProcessRun(ProcessRunRequest),
    AssetLoad(AssetRequest),
    ShaderCompile(ShaderRequest),
    AudioDecode(AudioDecodeRequest),
    TtsSynthesis(TtsRequest),
    WasmCall(WasmCallRequest),
    SystemInfo(SystemInfoRequest),
    Custom {
        capability: HostCapabilityId,
        operation: String,
        args: Vec<RuntimePayload>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        named_args: Vec<NamedHostArg<RuntimePayload>>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FileReadTextRequest {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FileReadBytesRequest {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FileWriteTextRequest {
    pub path: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FileWriteBytesRequest {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HttpFetchRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<RuntimePayload>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HttpRespondRequest {
    pub request_id: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<RuntimePayload>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProcessRunRequest {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AssetRequest {
    pub id: String,
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ShaderRequest {
    pub id: String,
    pub entry: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioDecodeRequest {
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TtsRequest {
    pub voice: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WasmCallRequest {
    pub module: String,
    pub function: String,
    pub args: Vec<RuntimePayload>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SystemInfoRequest {
    pub kind: SystemInfoKind,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum SystemInfoKind {
    CoreCount,
    ThreadCount,
    AvailableParallelism,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskEvent {
    pub logical_epoch: LogicalEpoch,
    pub task_id: TaskId,
    pub sequence: TaskSequence,
    pub kind: TaskEventKind,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum TaskEventKind {
    Ready(RuntimePayload),
    Error(RuntimePayload),
    Failed(String),
    Cancelled,
    Progress(RuntimePayload),
}

pub trait TaskHost {
    fn ensure_task(&mut self, spec: TaskSpec) -> TaskHandle;
    fn cancel_scope(&mut self, scope: CancelScopeId);
    fn poll_frame(&mut self, budget: SchedulerBudget) -> Vec<TaskEvent>;
}

impl TaskSpec {
    pub fn new(
        id: TaskId,
        key: TaskKey,
        class: TaskClass,
        priority: TaskPriority,
        cancel_scope: CancelScopeId,
        policy: TaskPolicy,
        request: HostTaskRequest,
    ) -> Self {
        Self::new_with_outcome(
            id,
            key,
            class,
            priority,
            cancel_scope,
            policy,
            TaskOutcomeContract::default(),
            request,
        )
    }

    pub fn new_with_outcome(
        id: TaskId,
        key: TaskKey,
        class: TaskClass,
        priority: TaskPriority,
        cancel_scope: CancelScopeId,
        policy: TaskPolicy,
        outcome: TaskOutcomeContract,
        request: HostTaskRequest,
    ) -> Self {
        let debug_label = request.debug_label();
        Self {
            id,
            key,
            class,
            priority,
            cancel_scope,
            policy,
            outcome,
            request,
            debug_label,
        }
    }
}

impl AwaitTarget {
    pub fn new(need: NeedId, task: TaskId, request: HostTaskRequestTemplate) -> Self {
        Self {
            need,
            task,
            outcome: TaskOutcomeContract::default(),
            request,
        }
    }

    pub fn with_outcome(
        need: NeedId,
        task: TaskId,
        outcome: TaskOutcomeContract,
        request: HostTaskRequestTemplate,
    ) -> Self {
        Self {
            need,
            task,
            outcome,
            request,
        }
    }
}

impl AwaitManyTarget {
    pub fn new(
        need: NeedId,
        task: TaskId,
        source: RuntimeExpr,
        item_binding: RuntimeLocalDeclarationId,
        limit: usize,
        request: HostTaskRequestTemplate,
    ) -> Self {
        Self {
            need,
            task,
            outcome: TaskOutcomeContract::default(),
            source,
            item_binding,
            limit,
            request,
        }
    }
}

impl HostTaskRequestTemplate {
    pub fn new(
        capability: impl Into<String>,
        operation: impl Into<String>,
        args: impl IntoIterator<Item = RuntimeHostArgumentTemplate>,
    ) -> Self {
        Self {
            capability: HostCapabilityId(capability.into()),
            operation: operation.into(),
            args: args.into_iter().collect(),
        }
    }
}

impl RuntimeHostArgumentTemplate {
    pub fn positional(value: RuntimeExpr) -> Self {
        Self::Positional(value)
    }

    pub fn named(name: impl Into<String>, value: RuntimeExpr) -> Self {
        Self::Named(NamedHostArg {
            name: name.into(),
            value,
        })
    }

    pub fn spread(value: RuntimeExpr) -> Self {
        Self::Spread(value)
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Named(argument) => Some(&argument.name),
            Self::Positional(_) | Self::Spread(_) => None,
        }
    }

    pub fn value(&self) -> &RuntimeExpr {
        match self {
            Self::Positional(value) | Self::Spread(value) => value,
            Self::Named(argument) => &argument.value,
        }
    }

    pub const fn is_spread(&self) -> bool {
        matches!(self, Self::Spread(_))
    }
}

impl HostTaskRequest {
    pub fn custom(
        capability: impl Into<String>,
        operation: impl Into<String>,
        args: impl IntoIterator<Item = RuntimePayload>,
    ) -> Self {
        Self::Custom {
            capability: HostCapabilityId(capability.into()),
            operation: operation.into(),
            args: args.into_iter().collect(),
            named_args: Vec::new(),
        }
    }

    pub fn custom_with_named_args(
        capability: impl Into<String>,
        operation: impl Into<String>,
        args: impl IntoIterator<Item = RuntimePayload>,
        named_args: impl IntoIterator<Item = (String, RuntimePayload)>,
    ) -> Self {
        Self::Custom {
            capability: HostCapabilityId(capability.into()),
            operation: operation.into(),
            args: args.into_iter().collect(),
            named_args: named_args
                .into_iter()
                .map(|(name, value)| NamedHostArg { name, value })
                .collect(),
        }
    }

    pub fn debug_label(&self) -> String {
        match self {
            Self::FileReadText(request) => format!("file.read_text {}", request.path),
            Self::FileReadBytes(request) => format!("file.read_bytes {}", request.path),
            Self::FileWriteText(request) => format!("file.write_text {}", request.path),
            Self::FileWriteBytes(request) => format!("file.write_bytes {}", request.path),
            Self::HttpFetch(request) => format!("http.fetch {} {}", request.method, request.url),
            Self::HttpRespond(request) => {
                format!("http.respond {} {}", request.request_id, request.status)
            }
            Self::ProcessRun(request) => format!("process.run {}", request.program),
            Self::AssetLoad(request) => format!("asset.load {} {}", request.kind, request.id),
            Self::ShaderCompile(request) => format!("shader.compile {}", request.id),
            Self::AudioDecode(request) => format!("audio.decode {}", request.id),
            Self::TtsSynthesis(request) => {
                format!(
                    "tts.synthesis {}",
                    request.voice.as_deref().unwrap_or("default")
                )
            }
            Self::WasmCall(request) => {
                format!("wasm.call {}::{}", request.module, request.function)
            }
            Self::SystemInfo(request) => format!("system.{}", request.kind.as_str()),
            Self::Custom {
                capability,
                operation,
                ..
            } => format!("{}.{}", capability.0, operation),
        }
    }

    pub fn host_call_id(&self) -> String {
        match self {
            Self::FileReadText(_) => "fs.read_text".to_owned(),
            Self::FileReadBytes(_) => "fs.read_bytes".to_owned(),
            Self::FileWriteText(_) => "fs.write_text".to_owned(),
            Self::FileWriteBytes(_) => "fs.write_bytes".to_owned(),
            Self::HttpFetch(_) => "http.fetch".to_owned(),
            Self::HttpRespond(_) => "http.respond".to_owned(),
            Self::ProcessRun(_) => "process.run".to_owned(),
            Self::AssetLoad(request) => format!("asset.{}", request.kind),
            Self::ShaderCompile(_) => "shader.compile".to_owned(),
            Self::AudioDecode(_) => "audio.decode".to_owned(),
            Self::TtsSynthesis(_) => "tts.synthesize".to_owned(),
            Self::WasmCall(_) => "wasm.call".to_owned(),
            Self::SystemInfo(request) => format!("system.{}", request.kind.as_str()),
            Self::Custom {
                capability,
                operation,
                ..
            } => format!("{}.{}", capability.0, operation),
        }
    }

    pub const fn task_class(&self) -> TaskClass {
        match self {
            Self::FileReadText(_)
            | Self::FileReadBytes(_)
            | Self::FileWriteText(_)
            | Self::FileWriteBytes(_)
            | Self::HttpFetch(_)
            | Self::HttpRespond(_)
            | Self::ProcessRun(_) => TaskClass::Io,
            Self::AssetLoad(_) => TaskClass::AssetDecode,
            Self::ShaderCompile(_) => TaskClass::ShaderCompile,
            Self::AudioDecode(_) => TaskClass::AudioDecode,
            Self::TtsSynthesis(_) => TaskClass::TtsSynthesis,
            Self::WasmCall(_) => TaskClass::WasmCall,
            Self::SystemInfo(_) => TaskClass::Cpu,
            Self::Custom { .. } => TaskClass::Background,
        }
    }
}

impl SystemInfoKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CoreCount => "core_count",
            Self::ThreadCount => "thread_count",
            Self::AvailableParallelism => "available_parallelism",
        }
    }
}

impl From<&str> for HostCapabilityId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for HostCapabilityId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Returns task events in replay-stable completion order.
pub fn normalize_task_events(mut events: Vec<TaskEvent>) -> Vec<TaskEvent> {
    if events.len() > 1 && !task_events_are_normalized(&events) {
        events.sort_by(compare_task_events);
    }
    events
}

/// Returns true when task events are already in replay-stable completion order.
pub fn task_events_are_normalized(events: &[TaskEvent]) -> bool {
    events
        .windows(2)
        .all(|pair| compare_task_events(&pair[0], &pair[1]).is_le())
}

/// Compares task events by replay-stable completion order.
pub fn compare_task_events(left: &TaskEvent, right: &TaskEvent) -> std::cmp::Ordering {
    left.logical_epoch
        .cmp(&right.logical_epoch)
        .then_with(|| left.task_id.cmp(&right.task_id))
        .then_with(|| left.sequence.cmp(&right.sequence))
}

/// Returns producer-owned Need states in replay-stable publication order.
pub fn normalize_runtime_need_states(mut states: Vec<RuntimeNeedState>) -> Vec<RuntimeNeedState> {
    if states.len() > 1 && !runtime_need_states_are_normalized(&states) {
        states.sort_by(compare_runtime_need_states);
    }
    states
}

/// Returns true when Need states are already in replay-stable order.
pub fn runtime_need_states_are_normalized(states: &[RuntimeNeedState]) -> bool {
    states
        .windows(2)
        .all(|pair| compare_runtime_need_states(&pair[0], &pair[1]).is_le())
}

/// Compares Need states by the same deterministic epoch/identity/sequence
/// vocabulary used by task events.
pub fn compare_runtime_need_states(
    left: &RuntimeNeedState,
    right: &RuntimeNeedState,
) -> std::cmp::Ordering {
    left.logical_epoch()
        .cmp(&right.logical_epoch())
        .then_with(|| left.need().cmp(right.need()))
        .then_with(|| left.sequence().cmp(&right.sequence()))
}

/// Selects the current state for one Need from a normalized publication list.
///
/// Progress and `NotStarted` publications may advance until the first terminal
/// publication. Once Ready, Error, or Cancelled is committed, later publications
/// for the same identity cannot replace it.
pub fn resolved_runtime_need_state<'a>(
    states: &'a [RuntimeNeedState],
    need: &NeedId,
) -> Option<&'a RuntimeNeedState> {
    let mut current = None;
    for candidate in states.iter().filter(|candidate| candidate.need() == need) {
        current = Some(candidate);
        if candidate.state().is_terminal() {
            break;
        }
    }
    current
}
