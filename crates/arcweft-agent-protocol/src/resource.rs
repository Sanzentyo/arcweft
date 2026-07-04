use crate::image::AgentImageMetadata;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

/// MCP-addressable Agent Debug Bus resource.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentResource {
    pub uri: String,
    pub kind: AgentResourceKind,
    pub mime_type: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<AgentImageMetadata>,
    pub body: AgentResourceBody,
}

impl AgentResource {
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
    Components,
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
            Self::Components => "components",
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
            Self::Components => "Observed components",
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
                | Self::Components
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
