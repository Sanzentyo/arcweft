#![forbid(unsafe_code)]
//! Sans I/O MCP-backed `AgentSession`.
//!
//! This crate owns no child process, stdio, socket, runtime, or filesystem
//! behavior. CLI adapters provide a transport; this crate validates the Arcweft
//! MCP tool surface and maps typed Agent host calls onto MCP requests.

use std::collections::BTreeMap;
use std::time::Duration;

use arcweft_agent_mcp::model::{
    McpCallToolResult, McpContentBlock, McpReadResourceResult, McpResourceContents,
    McpToolDescriptor,
};
use arcweft_agent_protocol::{
    protocol::{
        ActionResult, AgentAction, AgentSessionInfo, CaptureRequest, CaptureResult,
        ObservationEnvelope, ObserveRequest,
    },
    resource::{AgentResource, AgentResourceBody, AgentResourceKind},
};
use arcweft_agent_runner::session::AgentSession;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const ARCWEFT_MCP_PROTOCOL_VERSION: &str = "2024-11-05";

pub trait McpClientTransport {
    type Error: std::error::Error + Send + Sync + 'static;

    fn initialize(&mut self, request: InitializeRequest) -> Result<InitializeResult, Self::Error>;
    fn list_tools(&mut self) -> Result<Vec<McpToolDescriptor>, Self::Error>;
    fn call_tool(
        &mut self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpCallToolResult, Self::Error>;
    fn read_resource(&mut self, uri: &str) -> Result<McpReadResourceResult, Self::Error>;
    fn shutdown(&mut self) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InitializeRequest {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "clientName")]
    pub client_name: String,
}

impl Default for InitializeRequest {
    fn default() -> Self {
        Self {
            protocol_version: ARCWEFT_MCP_PROTOCOL_VERSION.to_owned(),
            client_name: "arcweft-agent-mcp-client".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "serverName")]
    #[serde(default)]
    pub server_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectOptions {
    pub initialize: InitializeRequest,
    pub timeout: Duration,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            initialize: InitializeRequest::default(),
            timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug)]
pub struct McpAgentSession<T> {
    transport: T,
    info: AgentSessionInfo,
    tools: BTreeMap<String, McpToolDescriptor>,
    action_tool: &'static str,
    timeout: Duration,
}

impl<T> McpAgentSession<T>
where
    T: McpClientTransport,
{
    pub fn connect(
        mut transport: T,
        options: ConnectOptions,
    ) -> Result<Self, McpAgentSessionError> {
        let initialized = transport
            .initialize(options.initialize.clone())
            .map_err(|error| McpAgentSessionError::transport("initialize", error))?;
        if initialized.protocol_version != options.initialize.protocol_version {
            return Err(McpAgentSessionError::ProtocolVersion {
                expected: options.initialize.protocol_version,
                found: initialized.protocol_version,
            });
        }
        let tools = transport
            .list_tools()
            .map_err(|error| McpAgentSessionError::transport("tools/list", error))?
            .into_iter()
            .map(|tool| (tool.name.clone(), tool))
            .collect::<BTreeMap<_, _>>();
        validate_required_tools(&tools)?;
        let action_tool = resolve_action_tool(&tools)?;
        let info_result = transport
            .call_tool("arcweft.session.info", serde_json::json!({}))
            .map_err(|error| McpAgentSessionError::transport("arcweft.session.info", error))?;
        let info = decode_tool_result("arcweft.session.info", &info_result)?;
        Ok(Self {
            transport,
            info,
            tools,
            action_tool,
            timeout: options.timeout,
        })
    }

    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn tool_names(&self) -> impl Iterator<Item = &str> {
        self.tools.keys().map(String::as_str)
    }

    fn call_typed<TOut>(
        &mut self,
        name: &'static str,
        arguments: serde_json::Value,
    ) -> Result<TOut, McpAgentSessionError>
    where
        TOut: serde::de::DeserializeOwned,
    {
        let result = self
            .transport
            .call_tool(name, arguments)
            .map_err(|error| McpAgentSessionError::transport(name, error))?;
        decode_tool_result(name, &result)
    }
}

impl<T> AgentSession for McpAgentSession<T>
where
    T: McpClientTransport,
{
    type Error = McpAgentSessionError;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(self.info.clone())
    }

    fn observe(&mut self, request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        self.call_typed("arcweft.observe", serde_json::to_value(request)?)
    }

    fn act(&mut self, action: AgentAction) -> Result<ActionResult, Self::Error> {
        self.call_typed(self.action_tool, serde_json::to_value(action)?)
    }

    fn capture(&mut self, request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        self.call_typed("arcweft.capture", serde_json::to_value(request)?)
    }

    fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error> {
        let read = self
            .transport
            .read_resource(uri)
            .map_err(|error| McpAgentSessionError::transport("resources/read", error))?;
        decode_agent_resource(uri, read)
    }

    fn step_frames(&mut self, count: u32) -> Result<ObservationEnvelope, Self::Error> {
        self.call_typed(
            "arcweft.session.step_frames",
            serde_json::json!({ "count": count }),
        )
    }
}

fn validate_required_tools(
    tools: &BTreeMap<String, McpToolDescriptor>,
) -> Result<(), McpAgentSessionError> {
    for required in [
        "arcweft.session.info",
        "arcweft.observe",
        "arcweft.capture",
        "arcweft.resource.read",
        "arcweft.session.step_frames",
    ] {
        if !tools.contains_key(required) {
            return Err(McpAgentSessionError::MissingTool { name: required });
        }
    }
    let _ = resolve_action_tool(tools)?;
    Ok(())
}

fn resolve_action_tool(
    tools: &BTreeMap<String, McpToolDescriptor>,
) -> Result<&'static str, McpAgentSessionError> {
    if tools.contains_key("arcweft.action") {
        Ok("arcweft.action")
    } else if tools.contains_key("arcweft.act") {
        Ok("arcweft.act")
    } else {
        Err(McpAgentSessionError::MissingTool {
            name: "arcweft.action",
        })
    }
}

fn decode_tool_result<T>(
    operation: &'static str,
    result: &McpCallToolResult,
) -> Result<T, McpAgentSessionError>
where
    T: serde::de::DeserializeOwned,
{
    if result.is_error {
        let message = result
            .content
            .iter()
            .find_map(|block| match block {
                McpContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .unwrap_or("remote MCP tool returned an error")
            .to_owned();
        return Err(McpAgentSessionError::RemoteTool {
            name: operation.to_owned(),
            code: None,
            message,
        });
    }
    let Some(text) = result.content.iter().find_map(|block| match block {
        McpContentBlock::Text { text } => Some(text),
        _ => None,
    }) else {
        return Err(McpAgentSessionError::Decode {
            operation,
            message: "tool result did not contain a text JSON block".to_owned(),
        });
    };
    serde_json::from_str(text).map_err(|error| McpAgentSessionError::Decode {
        operation,
        message: error.to_string(),
    })
}

fn decode_agent_resource(
    uri: &str,
    read: McpReadResourceResult,
) -> Result<AgentResource, McpAgentSessionError> {
    let Some(content) = read.contents.into_iter().next() else {
        return Err(McpAgentSessionError::Decode {
            operation: "resources/read",
            message: "resource read returned no contents".to_owned(),
        });
    };
    match content {
        McpResourceContents::Text(text) => {
            if let Ok(resource) = serde_json::from_str::<AgentResource>(&text.text) {
                return Ok(resource);
            }
            Ok(AgentResource {
                uri: text.uri,
                kind: AgentResourceKind::ObservationLatest,
                mime_type: text
                    .mime_type
                    .unwrap_or_else(|| "text/plain; charset=utf-8".to_owned()),
                hash: format!("mcp-resource:{uri}"),
                image: None,
                body: AgentResourceBody::Text(text.text),
            })
        }
        McpResourceContents::Blob(blob) => Ok(AgentResource {
            uri: blob.uri,
            kind: AgentResourceKind::Image,
            mime_type: blob
                .mime_type
                .unwrap_or_else(|| "application/octet-stream".to_owned()),
            hash: format!("mcp-resource:{uri}"),
            image: None,
            body: AgentResourceBody::BytesBase64(
                arcweft_agent_protocol::resource::AgentBinaryResourceBody {
                    encoding: arcweft_agent_protocol::resource::AgentBinaryEncoding::Base64,
                    data: blob.blob,
                },
            ),
        }),
    }
}

#[derive(Debug, Error)]
pub enum McpAgentSessionError {
    #[error("MCP transport error during {operation}: {message}")]
    Transport {
        operation: &'static str,
        message: String,
    },
    #[error("MCP protocol version mismatch: expected {expected}, found {found}")]
    ProtocolVersion { expected: String, found: String },
    #[error("MCP server is missing required tool `{name}`")]
    MissingTool { name: &'static str },
    #[error("MCP remote tool `{name}` failed: {message}")]
    RemoteTool {
        name: String,
        code: Option<i64>,
        message: String,
    },
    #[error("MCP decode error during {operation}: {message}")]
    Decode {
        operation: &'static str,
        message: String,
    },
    #[error("MCP JSON encode error: {0}")]
    Encode(#[from] serde_json::Error),
}

impl McpAgentSessionError {
    fn transport(operation: &'static str, error: impl std::error::Error) -> McpAgentSessionError {
        Self::Transport {
            operation,
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::fmt::{self, Display, Formatter};

    use arcweft_agent_protocol::protocol::AgentProjectGraph;

    #[derive(Debug)]
    struct FakeTransport {
        tools: Vec<McpToolDescriptor>,
        calls: VecDeque<(&'static str, McpCallToolResult)>,
        shutdown: bool,
    }

    #[derive(Debug)]
    struct FakeError;

    impl Display for FakeError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("fake transport error")
        }
    }

    impl std::error::Error for FakeError {}

    impl McpClientTransport for FakeTransport {
        type Error = FakeError;

        fn initialize(
            &mut self,
            request: InitializeRequest,
        ) -> Result<InitializeResult, Self::Error> {
            Ok(InitializeResult {
                protocol_version: request.protocol_version,
                server_name: Some("fake".to_owned()),
            })
        }

        fn list_tools(&mut self) -> Result<Vec<McpToolDescriptor>, Self::Error> {
            Ok(self.tools.clone())
        }

        fn call_tool(
            &mut self,
            name: &str,
            _arguments: serde_json::Value,
        ) -> Result<McpCallToolResult, Self::Error> {
            let (expected, result) = self.calls.pop_front().expect("queued MCP call");
            assert_eq!(name, expected);
            Ok(result)
        }

        fn read_resource(&mut self, _uri: &str) -> Result<McpReadResourceResult, Self::Error> {
            Ok(McpReadResourceResult {
                contents: vec![McpResourceContents::Text(
                    arcweft_agent_mcp::model::McpTextResourceContents {
                        uri: "arcweft://session/fake/observation/latest.json".to_owned(),
                        mime_type: Some("application/json".to_owned()),
                        image: None,
                        text: "{}".to_owned(),
                    },
                )],
            })
        }

        fn shutdown(&mut self) -> Result<(), Self::Error> {
            self.shutdown = true;
            Ok(())
        }
    }

    #[test]
    fn mcp_agent_session_connects_and_dispatches_step_frames() {
        let info = AgentSessionInfo {
            session_id: "session.fake".to_owned(),
            program_hash: "program.fake".to_owned(),
            project_entities: Vec::new(),
            project_graph: AgentProjectGraph::default(),
            profile: Some("fake".to_owned()),
            capabilities: vec!["step_frames".to_owned()],
        };
        let observe = ObservationEnvelope {
            tick: 3,
            frame_id: "frame.3".to_owned(),
            state_hash: "state.3".to_owned(),
            render_hash: "render.3".to_owned(),
            actions: Vec::new(),
            signals: BTreeMap::new(),
            payload: serde_json::json!({ "tick": 3 }),
        };
        let transport = FakeTransport {
            tools: required_tools(),
            calls: VecDeque::from([
                ("arcweft.session.info", text_result(&info)),
                ("arcweft.session.step_frames", text_result(&observe)),
            ]),
            shutdown: false,
        };
        let mut session =
            McpAgentSession::connect(transport, ConnectOptions::default()).expect("connect");

        assert_eq!(session.info().expect("info"), info);
        assert_eq!(session.step_frames(2).expect("step"), observe);
    }

    #[test]
    fn mcp_agent_session_accepts_action_alias_when_canonical_is_absent() {
        let info = AgentSessionInfo {
            session_id: "session.fake".to_owned(),
            program_hash: "program.fake".to_owned(),
            project_entities: Vec::new(),
            project_graph: AgentProjectGraph::default(),
            profile: None,
            capabilities: Vec::new(),
        };
        let result = ActionResult {
            accepted: true,
            before_tick: 1,
            after_tick: 2,
            before_state_hash: "before".to_owned(),
            after_state_hash: "after".to_owned(),
        };
        let mut tools = required_tools();
        tools.retain(|tool| tool.name != "arcweft.action");
        tools.push(tool("arcweft.act"));
        let transport = FakeTransport {
            tools,
            calls: VecDeque::from([
                ("arcweft.session.info", text_result(&info)),
                ("arcweft.act", text_result(&result)),
            ]),
            shutdown: false,
        };
        let mut session =
            McpAgentSession::connect(transport, ConnectOptions::default()).expect("connect");

        assert_eq!(session.act(AgentAction::AdvanceText).expect("act"), result);
    }

    fn required_tools() -> Vec<McpToolDescriptor> {
        vec![
            tool("arcweft.session.info"),
            tool("arcweft.observe"),
            tool("arcweft.action"),
            tool("arcweft.capture"),
            tool("arcweft.resource.read"),
            tool("arcweft.session.step_frames"),
        ]
    }

    fn tool(name: &str) -> McpToolDescriptor {
        McpToolDescriptor {
            name: name.to_owned(),
            title: None,
            description: String::new(),
            input_schema: serde_json::json!({ "type": "object" }),
        }
    }

    fn text_result(value: &impl Serialize) -> McpCallToolResult {
        McpCallToolResult {
            content: vec![McpContentBlock::Text {
                text: serde_json::to_string(value).expect("value serializes"),
            }],
            is_error: false,
        }
    }
}
