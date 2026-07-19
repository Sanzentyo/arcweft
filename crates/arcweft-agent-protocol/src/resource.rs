use crate::{
    ids::{AgentResourceUri, AgentRunId},
    image::AgentImageMetadata,
    trace::AgentTraceRecord,
};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

/// MIME type of a typed Agent execution trace resource.
pub const AGENT_TRACE_MIME_TYPE: &str = "application/vnd.arcweft.agent-trace+json";

/// MCP-addressable Agent Debug Bus resource.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentResource {
    pub uri: AgentResourceUri,
    pub kind: AgentResourceKind,
    pub mime_type: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<AgentImageMetadata>,
    pub body: AgentResourceBody,
}

/// Failure to construct one internally consistent Agent trace resource.
#[derive(Debug, thiserror::Error)]
pub enum TraceResourceError {
    /// Records from more than one Agent run were supplied as one trace.
    #[error("trace record at index {index} belongs to Agent run `{actual}`; expected `{expected}`")]
    MixedRun {
        expected: AgentRunId,
        actual: AgentRunId,
        index: usize,
    },
    /// The validated trace could not be projected to its JSON resource body.
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

impl AgentResource {
    /// Constructs an ordinary resource without canonical publication authority.
    pub fn new(
        uri: AgentResourceUri,
        kind: AgentResourceKind,
        mime_type: impl Into<String>,
        hash: impl Into<String>,
        image: Option<AgentImageMetadata>,
        body: AgentResourceBody,
    ) -> Self {
        Self {
            uri,
            kind,
            mime_type: mime_type.into(),
            hash: hash.into(),
            image,
            body,
        }
    }

    pub const fn title(&self) -> &'static str {
        self.kind.title()
    }

    pub fn decoded_len(&self) -> Option<u64> {
        self.body.decoded_len()
    }

    pub fn description(&self) -> String {
        if let Some(image) = &self.image {
            return image.description(&self.mime_type);
        }
        match self.kind {
            AgentResourceKind::SessionContext => {
                format!(
                    "Path-redacted Agent session context resource ({})",
                    self.mime_type
                )
            }
            AgentResourceKind::Trace => format!(
                "Agent execution trace resource for read-only replay ({})",
                self.mime_type
            ),
            _ => format!("Agent Debug Bus resource ({})", self.mime_type),
        }
    }

    /// Whether this resource owns a checked canonical public address.
    pub fn has_canonical_public_uri(&self) -> bool {
        self.kind == AgentResourceKind::Trace
            && self.mime_type == AGENT_TRACE_MIME_TYPE
            && self.image.is_none()
            && trace_body_digest(&self.body)
                .is_ok_and(|digest| self.uri.certifies_trace_body(digest))
    }
}

/// Builds one canonical Agent trace resource from same-run typed records.
///
/// This is the only constructor that grants canonical trace publication
/// authority. Generic construction and wire deserialization deliberately
/// produce unsealed resources even when their URI text looks canonical.
pub fn trace_resource(records: &[AgentTraceRecord]) -> Result<AgentResource, TraceResourceError> {
    validate_single_trace_run(records)?;
    let run_id = records
        .first()
        .map_or_else(AgentRunId::unknown_trace, |record| record.run_id.clone());
    let body = AgentResourceBody::Json(serde_json::to_value(records)?);
    let body_digest = trace_body_digest(&body)?;
    Ok(AgentResource {
        uri: AgentResourceUri::sealed_trace(&run_id, body_digest),
        kind: AgentResourceKind::Trace,
        mime_type: AGENT_TRACE_MIME_TYPE.to_owned(),
        hash: trace_resource_hash(records),
        image: None,
        body,
    })
}

fn validate_single_trace_run(records: &[AgentTraceRecord]) -> Result<(), TraceResourceError> {
    let Some(expected) = records.first().map(|record| &record.run_id) else {
        return Ok(());
    };
    if let Some((index, record)) = records
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, record)| record.run_id != *expected)
    {
        return Err(TraceResourceError::MixedRun {
            expected: expected.clone(),
            actual: record.run_id.clone(),
            index,
        });
    }
    Ok(())
}

fn trace_resource_hash(records: &[AgentTraceRecord]) -> String {
    records.last().map_or_else(
        || "trace:empty".to_owned(),
        |record| {
            format!(
                "trace:{}:{}:{}",
                record.run_id.as_str(),
                records.len(),
                record.payload_hash.as_str()
            )
        },
    )
}

fn trace_body_digest(body: &AgentResourceBody) -> Result<[u8; 32], serde_json::Error> {
    serde_json::to_vec(body).map(|bytes| *blake3::hash(&bytes).as_bytes())
}

/// Resource body payload.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "body_kind", content = "body", rename_all = "snake_case")]
pub enum AgentResourceBody {
    Json(serde_json::Value),
    Text(String),
    BytesBase64(AgentBinaryResourceBody),
}

impl AgentResourceBody {
    pub fn decoded_len(&self) -> Option<u64> {
        match self {
            Self::Json(value) => serde_json::to_vec(value)
                .ok()
                .and_then(|bytes| u64::try_from(bytes.len()).ok()),
            Self::Text(text) => u64::try_from(text.len()).ok(),
            Self::BytesBase64(body) => body.decoded_len(),
        }
    }

    pub fn decoded_bytes(&self) -> Result<Option<Vec<u8>>, base64::DecodeError> {
        match self {
            Self::BytesBase64(body) => body.decode().map(Some),
            Self::Json(_) | Self::Text(_) => Ok(None),
        }
    }
}

/// Binary resource payload encoded for JSON/MCP transports.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentBinaryResourceBody {
    pub encoding: AgentBinaryEncoding,
    pub data: String,
}

impl AgentBinaryResourceBody {
    pub fn decode(&self) -> Result<Vec<u8>, base64::DecodeError> {
        self.encoding.decode(&self.data)
    }

    pub fn decoded_len(&self) -> Option<u64> {
        self.encoding.decoded_len(&self.data)
    }
}

/// Binary resource encoding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentBinaryEncoding {
    Base64,
}

impl AgentBinaryEncoding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Base64 => "base64",
        }
    }

    pub fn decode(self, value: &str) -> Result<Vec<u8>, base64::DecodeError> {
        match self {
            Self::Base64 => STANDARD.decode(value),
        }
    }

    pub fn decoded_len(self, value: &str) -> Option<u64> {
        self.decode(value)
            .ok()
            .and_then(|bytes| u64::try_from(bytes.len()).ok())
    }
}

/// Agent resource kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentResourceKind {
    SessionContext,
    ObservationLatest,
    Views,
    Objects,
    PresentationTree,
    OverlaySvg,
    Image,
    Logs,
    Signals,
    Audio,
    Trace,
}

impl AgentResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SessionContext => "session_context",
            Self::ObservationLatest => "observation_latest",
            Self::Views => "views",
            Self::Objects => "objects",
            Self::PresentationTree => "presentation_tree",
            Self::OverlaySvg => "overlay_svg",
            Self::Image => "image",
            Self::Logs => "logs",
            Self::Signals => "signals",
            Self::Audio => "audio",
            Self::Trace => "trace",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::SessionContext => "Session context",
            Self::ObservationLatest => "Latest observation",
            Self::Views => "Observed views",
            Self::Objects => "Observed objects",
            Self::PresentationTree => "Presentation tree",
            Self::OverlaySvg => "Overlay SVG",
            Self::Image => "Captured image",
            Self::Logs => "Runtime logs",
            Self::Signals => "Runtime signals",
            Self::Audio => "Audio state",
            Self::Trace => "Agent trace",
        }
    }

    pub const fn may_contain_free_form_text(self) -> bool {
        matches!(
            self,
            Self::SessionContext
                | Self::ObservationLatest
                | Self::Views
                | Self::Objects
                | Self::PresentationTree
                | Self::OverlaySvg
                | Self::Logs
                | Self::Signals
                | Self::Audio
                | Self::Trace
        )
    }
}
