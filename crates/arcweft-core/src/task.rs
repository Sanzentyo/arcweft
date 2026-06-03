use crate::value::{RuntimeExpr, RuntimePayload};

pub const AWAIT_MANY_ITEM_BINDING: &str = "__arcweft_await_many_item";

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId(pub String);

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskKey(pub String);

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NeedId(pub String);

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CancelScopeId(pub String);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogicalEpoch(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskSequence(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskPriority(pub i32);

#[derive(Clone, Debug, PartialEq)]
pub struct AwaitTarget {
    pub need: NeedId,
    pub task: TaskId,
    pub request: HostTaskRequestTemplate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwaitManyTarget {
    pub need: NeedId,
    pub task: TaskId,
    pub source: RuntimeExpr,
    pub item_binding: String,
    pub limit: usize,
    pub request: HostTaskRequestTemplate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskRequestTemplate {
    pub capability: HostCapabilityId,
    pub operation: String,
    pub args: Vec<HostTaskArgTemplate>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HostTaskArgTemplate {
    Positional(RuntimeExpr),
    Named { name: String, value: RuntimeExpr },
    Spread(RuntimeExpr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskSpec {
    pub id: TaskId,
    pub key: TaskKey,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub request: HostTaskRequest,
    pub debug_label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskHandle {
    pub id: TaskId,
    pub key: TaskKey,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SchedulerBudget {
    pub max_events: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskClass {
    LocalUi,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaskPolicy {
    JoinSameKey,
    AlwaysStart,
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostCapabilityId(pub String);

#[derive(Clone, Debug, PartialEq)]
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
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileReadTextRequest {
    pub path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileReadBytesRequest {
    pub path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileWriteTextRequest {
    pub path: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileWriteBytesRequest {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HttpFetchRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<RuntimePayload>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HttpRespondRequest {
    pub request_id: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<RuntimePayload>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessRunRequest {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssetRequest {
    pub id: String,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShaderRequest {
    pub id: String,
    pub entry: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioDecodeRequest {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TtsRequest {
    pub voice: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WasmCallRequest {
    pub module: String,
    pub function: String,
    pub args: Vec<RuntimePayload>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SystemInfoRequest {
    pub kind: SystemInfoKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SystemInfoKind {
    CoreCount,
    ThreadCount,
    AvailableParallelism,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskEvent {
    pub logical_epoch: LogicalEpoch,
    pub task_id: TaskId,
    pub sequence: TaskSequence,
    pub kind: TaskEventKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskEventKind {
    Ready(RuntimePayload),
    Err(String),
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
        let debug_label = request.debug_label();
        Self {
            id,
            key,
            class,
            priority,
            cancel_scope,
            policy,
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
            request,
        }
    }
}

impl AwaitManyTarget {
    pub fn new(
        need: NeedId,
        task: TaskId,
        source: RuntimeExpr,
        item_binding: impl Into<String>,
        limit: usize,
        request: HostTaskRequestTemplate,
    ) -> Self {
        Self {
            need,
            task,
            source,
            item_binding: item_binding.into(),
            limit,
            request,
        }
    }
}

impl HostTaskRequestTemplate {
    pub fn new(
        capability: impl Into<String>,
        operation: impl Into<String>,
        args: impl IntoIterator<Item = HostTaskArgTemplate>,
    ) -> Self {
        Self {
            capability: HostCapabilityId(capability.into()),
            operation: operation.into(),
            args: args.into_iter().collect(),
        }
    }
}

impl HostTaskArgTemplate {
    pub fn positional(value: RuntimeExpr) -> Self {
        Self::Positional(value)
    }

    pub fn named(name: impl Into<String>, value: RuntimeExpr) -> Self {
        Self::Named {
            name: name.into(),
            value,
        }
    }

    pub fn spread(value: RuntimeExpr) -> Self {
        Self::Spread(value)
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Named { name, .. } => Some(name),
            Self::Positional(_) | Self::Spread(_) => None,
        }
    }

    pub fn value(&self) -> &RuntimeExpr {
        match self {
            Self::Positional(value) | Self::Named { value, .. } | Self::Spread(value) => value,
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
