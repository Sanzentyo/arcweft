//! Inert task persistence. Executable request values use the program-bound
//! value codec, never generic serde of the live runtime graph.

use super::*;
use crate::value::{AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSubmissionSaveSnapshot {
    spec: TaskSpecSnapshot,
    handle: TaskHandle,
}

impl TaskSubmissionSaveSnapshot {
    pub(crate) fn from_live(
        submission: &TaskSubmission,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Ok(Self {
            spec: TaskSpecSnapshot::from_live(submission.spec(), owner)?,
            handle: submission.handle(),
        })
    }
    pub(crate) fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<TaskSubmission, AwbcRuntimeValueSnapshotError> {
        let spec = self.spec.into_live(owner)?;
        TaskSubmission::try_from_accepted(spec, self.handle).map_err(|error| {
            AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            }
        })
    }
    pub(crate) fn request_values(&self) -> impl Iterator<Item = &AwbcRuntimeValueSnapshot> {
        self.spec.request.values()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpecSnapshot {
    pub generation: GenerationId,
    pub producer: NeedProducerInstance,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContract,
    pub request: HostTaskRequestSnapshot,
    pub debug_label: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum HostTaskRequestSnapshot {
    FileReadText(FileReadTextRequest),
    FileReadBytes(FileReadBytesRequest),
    FileWriteText(FileWriteTextRequest),
    FileWriteBytes(FileWriteBytesRequest),
    HttpFetch {
        url: String,
        method: String,
        headers: Vec<(String, String)>,
        body: Option<AwbcRuntimeValueSnapshot>,
    },
    HttpRespond {
        request_id: String,
        status: u16,
        headers: Vec<(String, String)>,
        body: Option<AwbcRuntimeValueSnapshot>,
    },
    ProcessRun(ProcessRunRequest),
    AssetLoad(AssetRequest),
    ShaderCompile(ShaderRequest),
    AudioDecode(AudioDecodeRequest),
    TtsSynthesis(TtsRequest),
    WasmCall {
        module: String,
        function: String,
        args: Vec<AwbcRuntimeValueSnapshot>,
    },
    SystemInfo(SystemInfoRequest),
    Custom {
        capability: HostCapabilityId,
        operation: String,
        args: Vec<AwbcRuntimeValueSnapshot>,
        named_args: Vec<NamedHostArg<AwbcRuntimeValueSnapshot>>,
        manifest_contract: Option<crate::step::HostCallContractDigest>,
    },
}

impl TaskSpecSnapshot {
    pub(crate) fn from_live(
        spec: &TaskSpec,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Ok(Self {
            generation: spec.generation,
            producer: spec.producer.clone(),
            class: spec.class.clone(),
            priority: spec.priority,
            cancel_scope: spec.cancel_scope.clone(),
            policy: spec.policy,
            outcome: spec.outcome.clone(),
            request: HostTaskRequestSnapshot::from_live(&spec.request, owner)?,
            debug_label: spec.debug_label.clone(),
        })
    }

    pub(crate) fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<TaskSpec, AwbcRuntimeValueSnapshotError> {
        Ok(TaskSpec {
            generation: self.generation,
            producer: self.producer,
            class: self.class,
            priority: self.priority,
            cancel_scope: self.cancel_scope,
            policy: self.policy,
            outcome: self.outcome,
            request: self.request.into_live(owner)?,
            debug_label: self.debug_label,
        })
    }
}

impl HostTaskRequestSnapshot {
    fn from_live(
        request: &HostTaskRequest,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let value = |payload: &RuntimePayload| match owner {
            Some(owner) => {
                AwbcRuntimeValueSnapshot::from_runtime_value_for_program(payload.value(), owner)
            }
            None => AwbcRuntimeValueSnapshot::from_runtime_value(payload.value()),
        };
        Ok(match request {
            HostTaskRequest::FileReadText(r) => Self::FileReadText(r.clone()),
            HostTaskRequest::FileReadBytes(r) => Self::FileReadBytes(r.clone()),
            HostTaskRequest::FileWriteText(r) => Self::FileWriteText(r.clone()),
            HostTaskRequest::FileWriteBytes(r) => Self::FileWriteBytes(r.clone()),
            HostTaskRequest::HttpFetch(r) => Self::HttpFetch {
                url: r.url.clone(),
                method: r.method.clone(),
                headers: r.headers.clone(),
                body: r.body.as_ref().map(value).transpose()?,
            },
            HostTaskRequest::HttpRespond(r) => Self::HttpRespond {
                request_id: r.request_id.clone(),
                status: r.status,
                headers: r.headers.clone(),
                body: r.body.as_ref().map(value).transpose()?,
            },
            HostTaskRequest::ProcessRun(r) => Self::ProcessRun(r.clone()),
            HostTaskRequest::AssetLoad(r) => Self::AssetLoad(r.clone()),
            HostTaskRequest::ShaderCompile(r) => Self::ShaderCompile(r.clone()),
            HostTaskRequest::AudioDecode(r) => Self::AudioDecode(r.clone()),
            HostTaskRequest::TtsSynthesis(r) => Self::TtsSynthesis(r.clone()),
            HostTaskRequest::WasmCall(r) => Self::WasmCall {
                module: r.module.clone(),
                function: r.function.clone(),
                args: r.args.iter().map(value).collect::<Result<_, _>>()?,
            },
            HostTaskRequest::SystemInfo(r) => Self::SystemInfo(r.clone()),
            HostTaskRequest::Custom {
                capability,
                operation,
                args,
                named_args,
                manifest_contract,
            } => Self::Custom {
                capability: capability.clone(),
                operation: operation.clone(),
                args: args.iter().map(value).collect::<Result<_, _>>()?,
                named_args: named_args
                    .iter()
                    .map(|arg| {
                        Ok(NamedHostArg {
                            name: arg.name.clone(),
                            value: value(&arg.value)?,
                        })
                    })
                    .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
                manifest_contract: *manifest_contract,
            },
        })
    }

    fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<HostTaskRequest, AwbcRuntimeValueSnapshotError> {
        let value = |snapshot: AwbcRuntimeValueSnapshot| {
            snapshot
                .into_runtime_value_for_program(owner)
                .map(RuntimePayload)
        };
        Ok(match self {
            Self::FileReadText(r) => HostTaskRequest::FileReadText(r),
            Self::FileReadBytes(r) => HostTaskRequest::FileReadBytes(r),
            Self::FileWriteText(r) => HostTaskRequest::FileWriteText(r),
            Self::FileWriteBytes(r) => HostTaskRequest::FileWriteBytes(r),
            Self::HttpFetch {
                url,
                method,
                headers,
                body,
            } => HostTaskRequest::HttpFetch(HttpFetchRequest {
                url,
                method,
                headers,
                body: body.map(value).transpose()?,
            }),
            Self::HttpRespond {
                request_id,
                status,
                headers,
                body,
            } => HostTaskRequest::HttpRespond(HttpRespondRequest {
                request_id,
                status,
                headers,
                body: body.map(value).transpose()?,
            }),
            Self::ProcessRun(r) => HostTaskRequest::ProcessRun(r),
            Self::AssetLoad(r) => HostTaskRequest::AssetLoad(r),
            Self::ShaderCompile(r) => HostTaskRequest::ShaderCompile(r),
            Self::AudioDecode(r) => HostTaskRequest::AudioDecode(r),
            Self::TtsSynthesis(r) => HostTaskRequest::TtsSynthesis(r),
            Self::WasmCall {
                module,
                function,
                args,
            } => HostTaskRequest::WasmCall(WasmCallRequest {
                module,
                function,
                args: args.into_iter().map(value).collect::<Result<_, _>>()?,
            }),
            Self::SystemInfo(r) => HostTaskRequest::SystemInfo(r),
            Self::Custom {
                capability,
                operation,
                args,
                named_args,
                manifest_contract,
            } => HostTaskRequest::Custom {
                capability,
                operation,
                args: args.into_iter().map(value).collect::<Result<_, _>>()?,
                named_args: named_args
                    .into_iter()
                    .map(|arg| {
                        Ok(NamedHostArg {
                            name: arg.name,
                            value: value(arg.value)?,
                        })
                    })
                    .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
                manifest_contract,
            },
        })
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &AwbcRuntimeValueSnapshot> {
        let (body, args, named): (Option<&AwbcRuntimeValueSnapshot>, &[_], &[_]) = match self {
            Self::HttpFetch { body, .. } | Self::HttpRespond { body, .. } => {
                (body.as_ref(), &[], &[])
            }
            Self::WasmCall { args, .. } => (None, args, &[]),
            Self::Custom {
                args, named_args, ..
            } => (None, args, named_args),
            Self::FileReadText(_)
            | Self::FileReadBytes(_)
            | Self::FileWriteText(_)
            | Self::FileWriteBytes(_)
            | Self::ProcessRun(_)
            | Self::AssetLoad(_)
            | Self::ShaderCompile(_)
            | Self::AudioDecode(_)
            | Self::TtsSynthesis(_)
            | Self::SystemInfo(_) => (None, &[], &[]),
        };
        body.into_iter()
            .chain(args.iter())
            .chain(named.iter().map(|arg| &arg.value))
    }
}
