use std::{
    collections::VecDeque,
    fmt::{self, Display, Formatter},
    io::{BufRead as _, BufReader, Read as _, Write as _},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
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
    stdout_rx: mpsc::Receiver<Result<String, String>>,
    stderr_tail: Arc<Mutex<BoundedStderrTail>>,
    policy: StdioMcpTransportPolicy,
    next_id: u64,
    shutdown_started: bool,
}

#[derive(Debug, Clone, Copy)]
struct StdioMcpTransportPolicy {
    request_timeout: Duration,
    shutdown_response_timeout: Duration,
    shutdown_grace_timeout: Duration,
    stderr_tail_bytes: usize,
}

impl Default for StdioMcpTransportPolicy {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            shutdown_response_timeout: Duration::from_secs(5),
            shutdown_grace_timeout: Duration::from_secs(2),
            stderr_tail_bytes: 16 * 1024,
        }
    }
}

#[derive(Debug)]
struct BoundedStderrTail {
    bytes: VecDeque<u8>,
    capacity: usize,
}

impl BoundedStderrTail {
    fn new(capacity: usize) -> Self {
        Self {
            bytes: VecDeque::with_capacity(capacity.min(1024)),
            capacity,
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        if self.capacity == 0 {
            return;
        }
        for byte in chunk {
            if self.bytes.len() == self.capacity {
                self.bytes.pop_front();
            }
            self.bytes.push_back(*byte);
        }
    }

    fn text(&self) -> String {
        let bytes = self.bytes.iter().copied().collect::<Vec<_>>();
        String::from_utf8_lossy(&bytes).into_owned()
    }
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
        let stderr = child
            .stderr
            .take()
            .ok_or(StdioMcpTransportError::MissingPipe { pipe: "stderr" })?;
        Ok(Self::from_child(
            child,
            stdin,
            stdout,
            stderr,
            StdioMcpTransportPolicy::default(),
        ))
    }

    fn from_child(
        child: Child,
        stdin: ChildStdin,
        stdout: ChildStdout,
        stderr: ChildStderr,
        policy: StdioMcpTransportPolicy,
    ) -> Self {
        let stdout_rx = spawn_stdout_reader(stdout);
        let stderr_tail = Arc::new(Mutex::new(BoundedStderrTail::new(policy.stderr_tail_bytes)));
        spawn_stderr_reader(stderr, Arc::clone(&stderr_tail));
        Self {
            child,
            stdin,
            stdout_rx,
            stderr_tail,
            policy,
            next_id: 1,
            shutdown_started: false,
        }
    }

    fn request<T>(
        &mut self,
        method: &'static str,
        params: &serde_json::Value,
    ) -> Result<T, StdioMcpTransportError>
    where
        T: serde::de::DeserializeOwned,
    {
        self.request_with_timeout(method, params, self.policy.request_timeout)
    }

    fn request_with_timeout<T>(
        &mut self,
        method: &'static str,
        params: &serde_json::Value,
        timeout: Duration,
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
                stderr_tail: self.stderr_tail_text(),
            }
        })?;
        self.stdin
            .write_all(b"\n")
            .and_then(|()| self.stdin.flush())
            .map_err(|error| StdioMcpTransportError::Write {
                operation: method,
                message: error.to_string(),
                stderr_tail: self.stderr_tail_text(),
            })?;

        let line = match self.stdout_rx.recv_timeout(timeout) {
            Ok(Ok(line)) => line,
            Ok(Err(message)) => {
                return Err(StdioMcpTransportError::Read {
                    operation: method,
                    message,
                    stderr_tail: self.stderr_tail_text(),
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(StdioMcpTransportError::Timeout {
                    operation: method,
                    timeout_millis: timeout.as_millis(),
                    stderr_tail: self.stderr_tail_text(),
                });
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(StdioMcpTransportError::Closed {
                    operation: method,
                    stderr_tail: self.stderr_tail_text(),
                });
            }
        };
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

    fn write_exit_notification(&mut self) -> Result<(), StdioMcpTransportError> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "exit",
        });
        serde_json::to_writer(&mut self.stdin, &notification).map_err(|error| {
            StdioMcpTransportError::Write {
                operation: "exit",
                message: error.to_string(),
                stderr_tail: self.stderr_tail_text(),
            }
        })?;
        self.stdin
            .write_all(b"\n")
            .and_then(|()| self.stdin.flush())
            .map_err(|error| StdioMcpTransportError::Write {
                operation: "exit",
                message: error.to_string(),
                stderr_tail: self.stderr_tail_text(),
            })
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if self.child.try_wait().ok().flatten().is_some() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn stderr_tail_text(&self) -> String {
        self.stderr_tail
            .lock()
            .map(|tail| tail.text())
            .unwrap_or_default()
    }

    #[cfg(test)]
    fn spawn_with_policy(
        endpoint: &StdioMcpEndpoint,
        policy: StdioMcpTransportPolicy,
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
        let stderr = child
            .stderr
            .take()
            .ok_or(StdioMcpTransportError::MissingPipe { pipe: "stderr" })?;
        Ok(Self::from_child(child, stdin, stdout, stderr, policy))
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
        if self.shutdown_started {
            return Ok(());
        }
        self.shutdown_started = true;
        if self.child.try_wait().ok().flatten().is_some() {
            let _ = self.child.wait();
            return Ok(());
        }
        let _ = self.request_with_timeout::<serde_json::Value>(
            "shutdown",
            &json!({}),
            self.policy.shutdown_response_timeout,
        );
        let _ = self.write_exit_notification();
        if !self.wait_for_exit(self.policy.shutdown_grace_timeout) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for StdioMcpTransport {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn spawn_stdout_reader(stdout: ChildStdout) -> mpsc::Receiver<Result<String, String>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if tx.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = tx.send(Err(error.to_string()));
                    break;
                }
            }
        }
    });
    rx
}

fn spawn_stderr_reader(stderr: ChildStderr, tail: Arc<Mutex<BoundedStderrTail>>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut chunk = [0_u8; 1024];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if let Ok(mut tail) = tail.lock() {
                        tail.push(&chunk[..read]);
                    }
                }
            }
        }
    });
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
        stderr_tail: String,
    },
    #[error("failed to read MCP {operation} response: {message}; stderr tail: {stderr_tail}")]
    Read {
        operation: &'static str,
        message: String,
        stderr_tail: String,
    },
    #[error(
        "MCP stdio endpoint timed out during {operation} after {timeout_millis}ms; stderr tail: {stderr_tail}"
    )]
    Timeout {
        operation: &'static str,
        timeout_millis: u128,
        stderr_tail: String,
    },
    #[error("MCP stdio endpoint closed during {operation}; stderr tail: {stderr_tail}")]
    Closed {
        operation: &'static str,
        stderr_tail: String,
    },
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
    use std::{
        collections::BTreeMap,
        fs,
        path::PathBuf,
        process, thread,
        time::{SystemTime, UNIX_EPOCH},
    };

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

    #[test]
    fn stdio_transport_times_out_and_retains_bounded_stderr_tail() {
        let stderr = format!("prefix-start:{}:tail-end", "x".repeat(128));
        let policy = StdioMcpTransportPolicy {
            request_timeout: Duration::from_millis(250),
            shutdown_response_timeout: Duration::from_millis(50),
            shutdown_grace_timeout: Duration::from_millis(100),
            stderr_tail_bytes: 32,
        };
        let mut transport =
            StdioMcpTransport::spawn_with_policy(&hanging_child_endpoint(&stderr), policy)
                .expect("fake child spawns");

        let deadline = Instant::now() + Duration::from_secs(2);
        while !transport.stderr_tail_text().contains("tail-end") && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let error = transport
            .request::<serde_json::Value>("initialize", &json!({}))
            .expect_err("request should time out");

        let StdioMcpTransportError::Timeout {
            operation,
            stderr_tail,
            ..
        } = error
        else {
            panic!("expected timeout, got {error}");
        };
        assert_eq!(operation, "initialize");
        assert!(stderr_tail.contains("tail-end"));
        assert!(!stderr_tail.contains("prefix-start"));
        assert!(stderr_tail.len() <= policy.stderr_tail_bytes);
    }

    #[test]
    fn stdio_transport_shutdown_requests_protocol_exit_before_kill() {
        let marker = temp_marker_path("graceful-shutdown");
        let _ = fs::remove_file(&marker);
        let policy = StdioMcpTransportPolicy {
            request_timeout: Duration::from_millis(500),
            shutdown_response_timeout: Duration::from_secs(3),
            shutdown_grace_timeout: Duration::from_secs(2),
            stderr_tail_bytes: 1024,
        };
        let mut transport =
            StdioMcpTransport::spawn_with_policy(&graceful_shutdown_endpoint(&marker), policy)
                .expect("fake child spawns");

        transport.shutdown().expect("shutdown");

        let marker_text = fs::read_to_string(&marker).expect("shutdown marker");
        assert_eq!(marker_text.trim(), "graceful");
        let _ = fs::remove_file(marker);
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

    fn temp_marker_path(label: &str) -> PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after unix epoch")
            .as_millis();
        std::env::temp_dir().join(format!("arcweft-{label}-{}-{millis}.txt", process::id()))
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

    #[cfg(windows)]
    fn hanging_child_endpoint(stderr: &str) -> StdioMcpEndpoint {
        powershell_endpoint(format!(
            "[Console]::Error.Write('{}'); [Console]::Error.Flush(); while (($line=[Console]::In.ReadLine()) -ne $null) {{ Start-Sleep -Milliseconds 200 }}",
            ps_single_quote(stderr)
        ))
    }

    #[cfg(windows)]
    fn graceful_shutdown_endpoint(marker: &std::path::Path) -> StdioMcpEndpoint {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {}
        })
        .to_string();
        powershell_endpoint(format!(
            "while (($line=[Console]::In.ReadLine()) -ne $null) {{ if ($line.Contains('shutdown')) {{ Set-Content -LiteralPath '{}' -Value 'graceful'; [Console]::Out.WriteLine('{}'); [Console]::Out.Flush(); exit 0 }} }}",
            ps_single_quote(&marker.display().to_string()),
            ps_single_quote(&response),
        ))
    }

    #[cfg(windows)]
    fn powershell_endpoint(command: String) -> StdioMcpEndpoint {
        StdioMcpEndpoint {
            program: "powershell".to_owned(),
            args: vec!["-NoProfile".to_owned(), "-Command".to_owned(), command],
        }
    }

    #[cfg(windows)]
    fn ps_single_quote(value: &str) -> String {
        value.replace('\'', "''")
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
    fn hanging_child_endpoint(stderr: &str) -> StdioMcpEndpoint {
        StdioMcpEndpoint {
            program: "/bin/sh".to_owned(),
            args: vec![
                "-c".to_owned(),
                format!(
                    "printf '%s' '{}' >&2; while IFS= read -r line; do sleep 1; done",
                    sh_quote(stderr)
                ),
            ],
        }
    }

    #[cfg(not(windows))]
    fn graceful_shutdown_endpoint(marker: &std::path::Path) -> StdioMcpEndpoint {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {}
        })
        .to_string();
        StdioMcpEndpoint {
            program: "/bin/sh".to_owned(),
            args: vec![
                "-c".to_owned(),
                format!(
                    "while IFS= read -r line; do case \"$line\" in *shutdown*) printf '%s\\n' '{}'; printf '%s' graceful > '{}'; exit 0 ;; esac; done",
                    sh_quote(&response),
                    sh_quote(&marker.display().to_string())
                ),
            ],
        }
    }

    #[cfg(not(windows))]
    fn sh_quote(value: &str) -> String {
        value.replace('\'', "'\\''")
    }
}
