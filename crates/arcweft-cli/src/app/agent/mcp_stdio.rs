use std::{
    fmt::{self, Display, Formatter},
    io::{BufRead as _, BufReader, Write as _},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use arcweft_agent_mcp::model::{McpCallToolResult, McpReadResourceResult, McpToolDescriptor};
use arcweft_agent_mcp_client::{InitializeRequest, InitializeResult, McpClientTransport};
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

#[derive(Debug)]
pub(in crate::app::agent) struct StdioMcpEndpoint {
    pub(in crate::app::agent) program: String,
    pub(in crate::app::agent) args: Vec<String>,
}

#[derive(Debug)]
pub(in crate::app::agent) struct StdioMcpTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl StdioMcpTransport {
    pub(in crate::app::agent) fn spawn(
        endpoint: &StdioMcpEndpoint,
    ) -> Result<Self, StdioMcpTransportError> {
        let mut child = Command::new(&endpoint.program)
            .args(&endpoint.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| StdioMcpTransportError::Spawn {
                program: endpoint.redacted_command(),
                message: error.to_string(),
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or(StdioMcpTransportError::MissingPipe { pipe: "stdin" })?;
        let stdout = child
            .stdout
            .take()
            .ok_or(StdioMcpTransportError::MissingPipe { pipe: "stdout" })?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    fn request<T>(
        &mut self,
        method: &'static str,
        params: &serde_json::Value,
    ) -> Result<T, StdioMcpTransportError>
    where
        T: serde::de::DeserializeOwned,
    {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        serde_json::to_writer(&mut self.stdin, &request).map_err(|error| {
            StdioMcpTransportError::Write {
                operation: method,
                message: error.to_string(),
            }
        })?;
        self.stdin
            .write_all(b"\n")
            .and_then(|()| self.stdin.flush())
            .map_err(|error| StdioMcpTransportError::Write {
                operation: method,
                message: error.to_string(),
            })?;

        let mut line = String::new();
        let bytes =
            self.stdout
                .read_line(&mut line)
                .map_err(|error| StdioMcpTransportError::Read {
                    operation: method,
                    message: error.to_string(),
                })?;
        if bytes == 0 {
            return Err(StdioMcpTransportError::Closed { operation: method });
        }
        let response = serde_json::from_str::<JsonRpcResponse>(&line).map_err(|error| {
            StdioMcpTransportError::Decode {
                operation: method,
                message: error.to_string(),
            }
        })?;
        if response.id != Some(id) {
            return Err(StdioMcpTransportError::RequestId {
                operation: method,
                expected: id,
                found: response.id,
            });
        }
        if let Some(error) = response.error {
            return Err(StdioMcpTransportError::Remote {
                operation: method,
                code: error.code,
                message: error.message,
            });
        }
        let result = response
            .result
            .ok_or(StdioMcpTransportError::MissingResult { operation: method })?;
        serde_json::from_value(result).map_err(|error| StdioMcpTransportError::Decode {
            operation: method,
            message: error.to_string(),
        })
    }
}

impl McpClientTransport for StdioMcpTransport {
    type Error = StdioMcpTransportError;

    fn initialize(&mut self, request: InitializeRequest) -> Result<InitializeResult, Self::Error> {
        let params = serde_json::to_value(request)?;
        let value = self.request::<serde_json::Value>("initialize", &params)?;
        let protocol_version = value
            .get("protocolVersion")
            .and_then(serde_json::Value::as_str)
            .ok_or(StdioMcpTransportError::MissingField {
                operation: "initialize",
                field: "protocolVersion",
            })?
            .to_owned();
        let server_name = value
            .get("serverName")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                value
                    .get("serverInfo")
                    .and_then(|server| server.get("name"))
                    .and_then(serde_json::Value::as_str)
            })
            .map(str::to_owned);
        Ok(InitializeResult {
            protocol_version,
            server_name,
        })
    }

    fn list_tools(&mut self) -> Result<Vec<McpToolDescriptor>, Self::Error> {
        #[derive(Deserialize)]
        struct ToolsList {
            tools: Vec<McpToolDescriptor>,
        }
        self.request::<ToolsList>("tools/list", &json!({}))
            .map(|list| list.tools)
    }

    fn call_tool(
        &mut self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpCallToolResult, Self::Error> {
        self.request(
            "tools/call",
            &json!({
                "name": name,
                "arguments": arguments,
            }),
        )
    }

    fn read_resource(&mut self, uri: &str) -> Result<McpReadResourceResult, Self::Error> {
        self.request("resources/read", &json!({ "uri": uri }))
    }

    fn shutdown(&mut self) -> Result<(), Self::Error> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for StdioMcpTransport {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

impl StdioMcpEndpoint {
    pub(in crate::app::agent) fn redacted_command(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

#[derive(Debug, Error)]
pub(in crate::app::agent) enum StdioMcpTransportError {
    #[error("failed to spawn MCP stdio endpoint `{program}`: {message}")]
    Spawn { program: String, message: String },
    #[error("MCP stdio child did not expose {pipe}")]
    MissingPipe { pipe: &'static str },
    #[error("failed to write MCP {operation} request: {message}")]
    Write {
        operation: &'static str,
        message: String,
    },
    #[error("failed to read MCP {operation} response: {message}")]
    Read {
        operation: &'static str,
        message: String,
    },
    #[error("MCP stdio endpoint closed during {operation}")]
    Closed { operation: &'static str },
    #[error("failed to decode MCP {operation} response: {message}")]
    Decode {
        operation: &'static str,
        message: String,
    },
    #[error("MCP {operation} response id mismatch: expected {expected}, found {found:?}")]
    RequestId {
        operation: &'static str,
        expected: u64,
        found: Option<u64>,
    },
    #[error("MCP {operation} remote error {code}: {message}")]
    Remote {
        operation: &'static str,
        code: i64,
        message: String,
    },
    #[error("MCP {operation} response did not include a result")]
    MissingResult { operation: &'static str },
    #[error("MCP {operation} response is missing field `{field}`")]
    MissingField {
        operation: &'static str,
        field: &'static str,
    },
    #[error("failed to encode MCP request params: {0}")]
    Encode(#[from] serde_json::Error),
}

impl Display for StdioMcpEndpoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.redacted_command())
    }
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_agent_mcp_client::{ConnectOptions, McpAgentSession};
    use arcweft_agent_protocol::{
        ids::AgentResourceUri,
        protocol::{
            ActionResult, AgentAction, AgentProjectGraph, AgentSessionInfo, CaptureFormat,
            CaptureRequest, CaptureResult, CaptureTarget, ObservationEnvelope, ObserveRequest,
        },
        resource::AgentResourceBody,
    };
    use arcweft_agent_runner::session::AgentSession;
    use serde::Serialize;
    use std::collections::BTreeMap;

    #[test]
    fn stdio_transport_roundtrips_agent_session_calls_through_fake_child() {
        let info = AgentSessionInfo {
            session_id: "session.fake".to_owned(),
            program_hash: "program.fake".to_owned(),
            project_entities: Vec::new(),
            project_graph: AgentProjectGraph::default(),
            profile: Some("fake".to_owned()),
            capabilities: vec![
                "observe".to_owned(),
                "act".to_owned(),
                "capture".to_owned(),
                "resource_read".to_owned(),
                "step_frames".to_owned(),
            ],
        };
        let observation = ObservationEnvelope {
            tick: 4,
            frame_id: "frame.4".to_owned(),
            state_hash: "state.4".to_owned(),
            render_hash: "render.4".to_owned(),
            actions: Vec::new(),
            signals: BTreeMap::new(),
            payload: serde_json::json!({ "source": "fake-child" }),
        };
        let action = ActionResult {
            accepted: true,
            before_tick: 4,
            after_tick: 5,
            before_state_hash: "state.4".to_owned(),
            after_state_hash: "state.5".to_owned(),
        };
        let capture = CaptureResult {
            uri: AgentResourceUri::new("agent://capture/fake/viewport.png")
                .expect("valid resource uri"),
            content_hash: "hash.capture".to_owned(),
            media_type: "image/png".to_owned(),
            byte_len: 7,
        };
        let endpoint = fake_child_endpoint(vec![
            rpc_result(
                1,
                &serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": { "name": "fake-child" }
                }),
            ),
            rpc_result(2, &serde_json::json!({ "tools": required_tools() })),
            rpc_result(3, &tool_result(&info)),
            rpc_result(4, &tool_result(&observation)),
            rpc_result(5, &tool_result(&action)),
            rpc_result(6, &tool_result(&observation)),
            rpc_result(7, &tool_result(&capture)),
            rpc_result(
                8,
                &serde_json::json!({
                    "contents": [{
                        "uri": "agent://capture/fake/viewport.png",
                        "mimeType": "text/plain",
                        "text": "fake resource"
                    }]
                }),
            ),
        ]);
        let transport = StdioMcpTransport::spawn(&endpoint).expect("fake child spawns");
        let mut session =
            McpAgentSession::connect(transport, ConnectOptions::default()).expect("connect");

        assert_eq!(session.info().expect("info"), info);
        assert_eq!(
            session.observe(ObserveRequest::default()).expect("observe"),
            observation
        );
        assert_eq!(session.act(AgentAction::AdvanceText).expect("act"), action);
        assert_eq!(session.step_frames(1).expect("step"), observation);
        assert_eq!(
            session
                .capture(CaptureRequest {
                    target: CaptureTarget::Viewport,
                    format: CaptureFormat::Png,
                    capture_kind: "color".to_owned(),
                    name: "fake".to_owned(),
                })
                .expect("capture"),
            capture
        );
        assert!(matches!(
            session
                .read_resource("agent://capture/fake/viewport.png")
                .expect("resource")
                .body,
            AgentResourceBody::Text(_)
        ));
    }

    fn required_tools() -> Vec<serde_json::Value> {
        [
            "arcweft.session.info",
            "arcweft.observe",
            "arcweft.action",
            "arcweft.capture",
            "arcweft.resource.read",
            "arcweft.session.step_frames",
        ]
        .into_iter()
        .map(|name| {
            serde_json::json!({
                "name": name,
                "title": null,
                "description": "",
                "inputSchema": { "type": "object" }
            })
        })
        .collect()
    }

    fn tool_result(value: &impl Serialize) -> serde_json::Value {
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(value).expect("serializes")
            }],
            "isError": false
        })
    }

    fn rpc_result(id: u64, result: &serde_json::Value) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        })
        .to_string()
    }

    #[cfg(windows)]
    fn fake_child_endpoint(responses: Vec<String>) -> StdioMcpEndpoint {
        let cases = responses
            .into_iter()
            .enumerate()
            .map(|(index, response)| {
                format!(
                    "{} {{ [Console]::Out.WriteLine('{}') }}",
                    index + 1,
                    response.replace('\'', "''")
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        StdioMcpEndpoint {
            program: "powershell".to_owned(),
            args: vec![
                "-NoProfile".to_owned(),
                "-Command".to_owned(),
                format!(
                    "$i=0; while (($line=[Console]::In.ReadLine()) -ne $null) {{ $i++; switch ($i) {{ {cases} default {{ exit 0 }} }} [Console]::Out.Flush() }}"
                ),
            ],
        }
    }

    #[cfg(not(windows))]
    fn fake_child_endpoint(responses: Vec<String>) -> StdioMcpEndpoint {
        let cases = responses
            .into_iter()
            .enumerate()
            .map(|(index, response)| {
                format!("{}) printf '%s\\n' '{}' ;;", index + 1, sh_quote(&response))
            })
            .collect::<Vec<_>>()
            .join(" ");
        StdioMcpEndpoint {
            program: "/bin/sh".to_owned(),
            args: vec![
                "-c".to_owned(),
                format!(
                    "i=0; while IFS= read -r line; do i=$((i+1)); case $i in {cases} *) exit 0 ;; esac; done"
                ),
            ],
        }
    }

    #[cfg(not(windows))]
    fn sh_quote(value: &str) -> String {
        value.replace('\'', "'\\''")
    }
}
