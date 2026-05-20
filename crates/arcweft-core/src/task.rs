use crate::value::RuntimePayload;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitTarget {
    pub need: NeedId,
    pub task: TaskId,
    pub request: HostTaskRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskHandle {
    pub id: TaskId,
    pub key: TaskKey,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SchedulerBudget {
    pub max_events: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskPolicy {
    JoinSameKey,
    AlwaysStart,
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostCapabilityId(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
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
    Custom {
        capability: HostCapabilityId,
        operation: String,
        args: Vec<RuntimePayload>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileReadTextRequest {
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileReadBytesRequest {
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileWriteTextRequest {
    pub path: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileWriteBytesRequest {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpFetchRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<RuntimePayload>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpRespondRequest {
    pub request_id: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<RuntimePayload>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessRunRequest {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRequest {
    pub id: String,
    pub kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShaderRequest {
    pub id: String,
    pub entry: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioDecodeRequest {
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TtsRequest {
    pub voice: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WasmCallRequest {
    pub module: String,
    pub function: String,
    pub args: Vec<RuntimePayload>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskEvent {
    pub logical_epoch: LogicalEpoch,
    pub task_id: TaskId,
    pub sequence: TaskSequence,
    pub kind: TaskEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskEventKind {
    Ready(String),
    Err(String),
    Cancelled,
    Progress(String),
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
    pub fn new(need: NeedId, task: TaskId, request: HostTaskRequest) -> Self {
        Self {
            need,
            task,
            request,
        }
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
            Self::Custom { .. } => TaskClass::Background,
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
    events.sort_by_key(|event| (event.logical_epoch, event.task_id.clone(), event.sequence));
    events
}
