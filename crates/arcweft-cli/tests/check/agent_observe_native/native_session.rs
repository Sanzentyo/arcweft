/// Owns one public MCP transport and its issued frame/resource context.
struct NativeMcpDialogueSession {
    child: std::process::Child,
    input: Option<std::process::ChildStdin>,
    output: std::io::BufReader<std::process::ChildStdout>,
    stderr: Option<std::thread::JoinHandle<String>>,
    next_id: u64,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl NativeMcpDialogueSession {
    fn new() -> Self {
        use std::io::Read as _;
        let guard = super::agent_mcp_stdio_guard();
        let mut child = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .args(["agent", "mcp", "--content-policy-mode", "local-dev"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start native MCP fixture session");
        let input = child.stdin.take();
        let output = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut errors = child.stderr.take().unwrap();
        let stderr = std::thread::spawn(move || {
            let mut text = String::new();
            errors
                .read_to_string(&mut text)
                .expect("drain native MCP stderr");
            text
        });
        let mut session = Self {
            child,
            input,
            output,
            stderr: Some(stderr),
            next_id: 0,
            _guard: guard,
        };
        session.rpc("initialize", serde_json::json!({}));
        session
    }

    fn rpc(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        use std::io::{BufRead as _, Write as _};
        self.next_id += 1;
        let request = serde_json::json!({ "jsonrpc":"2.0", "id":self.next_id, "method":method, "params":params });
        let input = self.input.as_mut().expect("MCP fixture is open");
        writeln!(input, "{request}").expect("write native MCP request");
        input.flush().expect("flush native MCP request");
        let mut line = String::new();
        assert!(
            self.output
                .read_line(&mut line)
                .expect("read native MCP response")
                > 0,
            "native MCP closed stdout"
        );
        let response: serde_json::Value =
            serde_json::from_str(&line).expect("native MCP JSON response");
        assert_eq!(response["id"], self.next_id);
        assert!(
            response.get("error").is_none(),
            "native MCP rejected {method}: {response}"
        );
        let result = response["result"].clone();
        assert_ne!(
            result["isError"], true,
            "native MCP tool rejected: {result}"
        );
        result
    }

    fn tool(&mut self, name: &str, arguments: serde_json::Value) -> serde_json::Value {
        self.rpc(
            "tools/call",
            serde_json::json!({ "name":name, "arguments":arguments }),
        )
    }

    fn observation(&mut self) -> serde_json::Value {
        let list = self.rpc("resources/list", serde_json::json!({}));
        let resources = list["resources"].as_array().unwrap();
        let latest = resources
            .iter()
            .filter(|resource| resource["title"] == "Latest observation")
            .collect::<Vec<_>>();
        assert_eq!(latest.len(), 1, "one advertised current observation");
        let read = self.rpc(
            "resources/read",
            serde_json::json!({ "uri":latest[0]["uri"] }),
        );
        let text = read["contents"][0]["text"].as_str().unwrap();
        arcweft_agent_protocol::observation::AgentObservationReport::decode_json(text.as_bytes())
            .expect("issued observation retains its admitted frame text context");
        serde_json::from_str(text).unwrap()
    }

    fn observe_fixture(&mut self, fixture: &NativeCliFixture, sample: Option<f32>, max_ops: u64) {
        let mut args = serde_json::json!({
            "source": fixture.path, "entry": fixture.entry.public_label().as_str(),
            "page": 0, "steps": 4, "max_ops": max_ops,
            "viewport_width": 1280, "viewport_height": 720,
        });
        if let Some(sample) = sample {
            args["capture_time"] = serde_json::json!(sample);
        }
        self.tool("arcweft.observe", args);
    }

    /// Progress the real session; observing again would restart the source.
    fn advance_to_line(&mut self, line: &str) -> serde_json::Value {
        for _ in 0..80 {
            let report = self.observation();
            assert!(
                report["diagnostics"].as_array().unwrap().is_empty(),
                "{report}"
            );
            if report["objects"].as_array().unwrap().iter().any(|object| {
                object["role"] == "dialogue_view"
                    && object["content"]["kind"] == "rich_text"
                    && object["rich_text_ref"].is_null()
                    && observed_object_rich_text_line(&report, object) == fixture_runtime_line(line)
            }) {
                return report;
            }
            if report["actions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|action| action["action"] == "advance_text" && action["enabled"] == true)
            {
                self.tool("arcweft.action", serde_json::json!({"kind":"advance_text"}));
            } else {
                self.tool(
                    "arcweft.session.step_frames",
                    serde_json::json!({"count":4}),
                );
            }
        }
        panic!("bounded native dialogue progression did not reach {line}");
    }

    fn image_resource(&mut self, uri: &str) -> (serde_json::Value, Vec<u8>) {
        let read = self.rpc(
            "resources/read",
            serde_json::json!({"uri":uri, "max_privacy":"secret"}),
        );
        let [resource] = read["contents"].as_array().unwrap().as_slice() else {
            panic!("one issued image resource: {read}");
        };
        assert_eq!(resource["uri"], uri);
        arcweft_agent_protocol::image::AgentImageMetadata::decode_json(
            &serde_json::to_vec(&resource["image"]).unwrap(),
        )
        .expect("actual MCP image metadata retains its admitted context");
        let bytes = general_purpose::STANDARD
            .decode(resource["blob"].as_str().unwrap())
            .unwrap();
        (resource.clone(), bytes)
    }

    fn capture(&mut self, arguments: serde_json::Value) -> (serde_json::Value, Vec<u8>) {
        let result = self.tool("arcweft.capture", arguments);
        let metadata = mcp_content_metadata(&result["content"][0], "issued MCP capture metadata");
        let uri = metadata["uri"].as_str().unwrap();
        let (resource, bytes) = self.image_resource(uri);
        assert_eq!(resource["image"], metadata["image"]);
        (resource, bytes)
    }

    fn assert_rich_text_layer_capture_has_content(&mut self) {
        let (resource, bytes) = self.capture(serde_json::json!({
            "layer":"dialogue.rich_text", "page":0, "format":"png",
        }));
        let image = &resource["image"];
        assert_eq!(image["kind"], "color");
        assert_eq!(image["renderer"], "native");
        assert_eq!(image["scope"]["kind"], "layer");
        assert_eq!(image["scope"]["id"], "dialogue.rich_text");
        assert_eq!(image["composition"], "masked_framebuffer_crop");
        assert_eq!(resource["mimeType"], "image/png");
        assert!(image["content_pixels"].as_u64().unwrap() > 0);
        assert_eq!(
            png_dimensions(&bytes),
            Some((
                image["width"].as_u64().unwrap().try_into().unwrap(),
                image["height"].as_u64().unwrap().try_into().unwrap(),
            ))
        );
    }

    fn close(mut self) {
        self.input.take();
        let status = self.child.wait().expect("finish native MCP fixture");
        let stderr = self
            .stderr
            .take()
            .unwrap()
            .join()
            .expect("MCP stderr thread");
        assert!(
            status.success(),
            "native MCP exit: {status}, stderr: {stderr}"
        );
    }
}

impl Drop for NativeMcpDialogueSession {
    fn drop(&mut self) {
        self.input.take();
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(stderr) = self.stderr.take() {
            if let Ok(text) = stderr.join() {
                if !text.is_empty() {
                    eprintln!("native MCP fixture stderr: {text}");
                }
            }
        }
    }
}

#[test]
fn agent_mcp_native_dialogue_progress_does_not_reuse_initial_capture_scope() {
    let fixture = temp_authored_native_cli_fixture(
        "native-mcp-dialogue-progress",
        r#"
pub character alice { display = "Alice" }
flow main() -> Unit {
    alice(id=@say.progress.first)[First[p]]
    alice(id=@say.progress.second)[Second[p]]
}
entry cli @entry.main { goto @flow.main }
"#,
        "entry.main",
    );
    let mut session = NativeMcpDialogueSession::new();
    session.tool(
        "arcweft.observe",
        serde_json::json!({
            "source":fixture.path, "entry":"entry.main", "image":"png",
            "layer":"dialogue.rich_text", "page":0, "steps":4, "max_ops":128,
        }),
    );
    let first = session.observation();
    let first_object = find_dialogue_view_object(&first);
    assert_eq!(first_object["text"], "First");
    assert!(!first["images"].as_array().unwrap().is_empty());
    session.tool("arcweft.action", serde_json::json!({"kind":"advance_text"}));
    let mut current = session.observation();
    assert_ne!(current["frame_id"], first["frame_id"]);
    assert!(
        current["images"].as_array().unwrap().is_empty(),
        "a progress request does not repeat a previous image request"
    );
    for _ in 0..12 {
        if current["objects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|object| object["role"] == "dialogue_view" && object["text"] == "Second")
        {
            break;
        }
        let tick = current["tick"].as_u64().unwrap();
        session.tool(
            "arcweft.session.step_frames",
            serde_json::json!({"count":1}),
        );
        current = session.observation();
        assert_eq!(current["tick"].as_u64().unwrap(), tick + 1);
    }
    let second = find_dialogue_view_object(&current);
    assert_eq!(second["text"], "Second");
    assert_eq!(
        observed_object_rich_text_line(&current, second),
        fixture_runtime_line("say.progress.second")
    );
    assert!(
        current["diagnostics"].as_array().unwrap().is_empty(),
        "no stale initial-layer diagnostics: {current}"
    );
    session.close();
}
