//! Request projection owned by the original Host task template.

use super::{
    AssetLoadKind, AudioDecodeRequest, FileReadBytesRequest, FileReadTextRequest,
    FileWriteBytesRequest, FileWriteTextRequest, HostCapabilityId, HostTaskRequest,
    HostTaskRequestKind, HostTaskRequestTemplate, HttpFetchRequest, HttpRespondRequest,
    ProcessRunRequest, ShaderRequest, SystemInfoKind, SystemInfoRequest, TtsRequest,
    WasmCallRequest,
};
use crate::value::{
    RuntimePayload, RuntimeValue, RuntimeVirtualPath, runtime_value_into_sequence_values,
};
use std::collections::VecDeque;

/// The complete operation selected by an original template before seal.
/// A custom capability is an explicit operation in its own right.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostTaskTemplateOperation {
    FileReadText,
    FileReadBytes,
    FileWriteText,
    FileWriteBytes,
    HttpFetch,
    HttpRespond,
    ProcessRun,
    AssetLoad(AssetLoadKind),
    ShaderCompile,
    AudioDecode,
    TtsSynthesis,
    WasmCall,
    SystemInfo(SystemInfoKind),
    Custom {
        capability: HostCapabilityId,
        operation: String,
    },
}

impl HostTaskTemplateOperation {
    /// Namespace used for the executable capability check.
    #[must_use]
    pub fn capability(&self) -> &str {
        match self {
            Self::FileReadText
            | Self::FileReadBytes
            | Self::FileWriteText
            | Self::FileWriteBytes => "fs",
            Self::HttpFetch | Self::HttpRespond => "http",
            Self::ProcessRun => "process",
            Self::AssetLoad(_) => "asset",
            Self::ShaderCompile => "shader",
            Self::AudioDecode => "audio",
            Self::TtsSynthesis => "tts",
            Self::WasmCall => "wasm",
            Self::SystemInfo(_) => "system",
            Self::Custom { capability, .. } => &capability.0,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> HostTaskRequestKind {
        match self {
            Self::FileReadText => HostTaskRequestKind::FileReadText,
            Self::FileReadBytes => HostTaskRequestKind::FileReadBytes,
            Self::FileWriteText => HostTaskRequestKind::FileWriteText,
            Self::FileWriteBytes => HostTaskRequestKind::FileWriteBytes,
            Self::HttpFetch => HostTaskRequestKind::HttpFetch,
            Self::HttpRespond => HostTaskRequestKind::HttpRespond,
            Self::ProcessRun => HostTaskRequestKind::ProcessRun,
            Self::AssetLoad(_) => HostTaskRequestKind::AssetLoad,
            Self::ShaderCompile => HostTaskRequestKind::ShaderCompile,
            Self::AudioDecode => HostTaskRequestKind::AudioDecode,
            Self::TtsSynthesis => HostTaskRequestKind::TtsSynthesis,
            Self::WasmCall => HostTaskRequestKind::WasmCall,
            Self::SystemInfo(_) => HostTaskRequestKind::SystemInfo,
            Self::Custom { .. } => HostTaskRequestKind::Custom,
        }
    }

    pub(super) fn encode_control(&self, encoder: &mut super::semantic::TaskSemanticEncoder<'_>) {
        encoder.tag(self.kind().semantic_tag());
        match self {
            Self::AssetLoad(kind) => encoder.tag(kind.semantic_tag()),
            Self::SystemInfo(kind) => encoder.tag(kind.semantic_tag()),
            Self::Custom {
                capability,
                operation,
            } => {
                encoder.string(&capability.0);
                encoder.string(operation);
            }
            _ => {}
        }
    }
}

impl HostTaskRequestTemplate {
    pub(crate) fn project_evaluated(
        &self,
        values: Vec<RuntimeValue>,
    ) -> Result<HostTaskRequest, String> {
        let mut arguments = EvaluatedArguments::default();
        for (argument, value) in self.args.iter().zip(values) {
            if argument.is_spread() {
                arguments.positional.extend(sequence(value)?);
            } else if let Some(name) = argument.name() {
                arguments.named.push((name.to_owned(), value));
            } else {
                arguments.positional.push_back(value);
            }
        }
        arguments.project(&self.operation)
    }
}

#[derive(Default)]
struct EvaluatedArguments {
    positional: VecDeque<RuntimeValue>,
    named: Vec<(String, RuntimeValue)>,
}

impl EvaluatedArguments {
    fn required(&mut self) -> Result<RuntimeValue, String> {
        self.positional
            .pop_front()
            .ok_or_else(|| "missing positional task request argument".into())
    }

    fn named(&mut self, name: &str) -> Option<RuntimeValue> {
        let index = self
            .named
            .iter()
            .position(|(candidate, _)| candidate == name)?;
        Some(self.named.remove(index).1)
    }

    fn named_string(&mut self, name: &str) -> Result<Option<String>, String> {
        self.named(name).map(string).transpose()
    }

    fn path(&mut self) -> Result<String, String> {
        RuntimeVirtualPath::try_from(&self.required()?)
            .map(|path| path.runtime_label())
            .map_err(|error| error.to_string())
    }

    fn project(mut self, operation: &HostTaskTemplateOperation) -> Result<HostTaskRequest, String> {
        use HostTaskTemplateOperation as Operation;
        let request = match operation {
            Operation::FileReadText => {
                HostTaskRequest::FileReadText(FileReadTextRequest { path: self.path()? })
            }
            Operation::FileReadBytes => {
                HostTaskRequest::FileReadBytes(FileReadBytesRequest { path: self.path()? })
            }
            Operation::FileWriteText => HostTaskRequest::FileWriteText(FileWriteTextRequest {
                path: self.path()?,
                text: string(self.required()?)?,
            }),
            Operation::FileWriteBytes => HostTaskRequest::FileWriteBytes(FileWriteBytesRequest {
                path: self.path()?,
                bytes: sequence(self.required()?)?
                    .into_iter()
                    .map(unsigned::<u8>)
                    .collect::<Result<_, _>>()?,
            }),
            Operation::HttpFetch => HostTaskRequest::HttpFetch(HttpFetchRequest {
                url: string(self.required()?)?,
                method: self.named_string("method")?.unwrap_or_else(|| "GET".into()),
                headers: self
                    .named("headers")
                    .map(pairs)
                    .transpose()?
                    .unwrap_or_default(),
                body: self.named("body").map(RuntimePayload::new),
            }),
            Operation::HttpRespond => HostTaskRequest::HttpRespond(HttpRespondRequest {
                request_id: string(self.required()?)?,
                status: self
                    .named("status")
                    .map(unsigned::<u16>)
                    .transpose()?
                    .unwrap_or(200),
                headers: self
                    .named("headers")
                    .map(pairs)
                    .transpose()?
                    .unwrap_or_default(),
                body: self.named("body").map(RuntimePayload::new),
            }),
            Operation::ProcessRun => HostTaskRequest::ProcessRun(ProcessRunRequest {
                program: string(self.required()?)?,
                args: self
                    .named("args")
                    .map(|value| sequence(value)?.into_iter().map(string).collect())
                    .transpose()?
                    .unwrap_or_default(),
                env: self
                    .named("env")
                    .map(pairs)
                    .transpose()?
                    .unwrap_or_default(),
            }),
            Operation::AssetLoad(kind) => {
                let RuntimeValue::EntityRef(reference) = self.required()? else {
                    return Err("asset request requires a catalog reference".into());
                };
                HostTaskRequest::AssetLoad(
                    kind.project_catalog_request(&reference)
                        .map_err(|error| error.to_string())?,
                )
            }
            Operation::ShaderCompile => HostTaskRequest::ShaderCompile(ShaderRequest {
                id: string(self.required()?)?,
                entry: self.named_string("entry")?,
            }),
            Operation::AudioDecode => HostTaskRequest::AudioDecode(AudioDecodeRequest {
                id: string(self.required()?)?,
            }),
            Operation::TtsSynthesis => HostTaskRequest::TtsSynthesis(TtsRequest {
                voice: self.named_string("voice")?,
                text: match self.named_string("text")? {
                    Some(text) => text,
                    None => string(self.required()?)?,
                },
            }),
            Operation::WasmCall => HostTaskRequest::WasmCall(WasmCallRequest {
                module: string(self.required()?)?,
                function: string(self.required()?)?,
                args: self.positional.drain(..).map(RuntimePayload::new).collect(),
            }),
            Operation::SystemInfo(kind) => {
                HostTaskRequest::SystemInfo(SystemInfoRequest { kind: *kind })
            }
            Operation::Custom {
                capability,
                operation,
            } => HostTaskRequest::custom_with_named_args(
                capability.0.clone(),
                operation.clone(),
                self.positional.drain(..).map(RuntimePayload::new),
                self.named
                    .drain(..)
                    .map(|(name, value)| (name, RuntimePayload::new(value))),
            ),
        };
        if !self.positional.is_empty() || !self.named.is_empty() {
            return Err("unexpected task request arguments for the selected operation".into());
        }
        Ok(request)
    }
}

fn sequence(value: RuntimeValue) -> Result<Vec<RuntimeValue>, String> {
    runtime_value_into_sequence_values(value)
        .map_err(|_| "task request argument requires a sequence".into())
}

fn string(value: RuntimeValue) -> Result<String, String> {
    match value {
        RuntimeValue::String(value) => Ok(value),
        _ => Err("task request argument requires a String".into()),
    }
}

fn unsigned<T: TryFrom<i64>>(value: RuntimeValue) -> Result<T, String> {
    let RuntimeValue::UInt(value) = value else {
        return Err("task request argument requires an unsigned integer".into());
    };
    value
        .try_into_i64()
        .and_then(|value| T::try_from(value).ok())
        .ok_or_else(|| "task request integer is outside the selected field range".into())
}

fn pairs(value: RuntimeValue) -> Result<Vec<(String, String)>, String> {
    sequence(value)?
        .into_iter()
        .map(|entry| {
            let RuntimeValue::Tuple(values) = entry else {
                return Err("task request field requires pairs of strings".into());
            };
            let [first, second] = values
                .try_into()
                .map_err(|_| "task request field requires pairs of strings")?;
            Ok((string(first)?, string(second)?))
        })
        .collect()
}
