fn temp_mcp_arcw(name: &str, source: &str) -> PathBuf {
    temp_arcw(
        name,
        &format!("entry cli @entry.main {{ goto @flow.main }}\n{source}"),
    )
}

fn observe_mcp_fixture(path: &Path, entry: &str) -> serde_json::Value {
    observe_mcp_fixture_with_timing(path, entry, 4, None)
}

fn observe_mcp_fixture_with_timing(
    path: &Path,
    entry: &str,
    steps: u64,
    capture_time: Option<f64>,
) -> serde_json::Value {
    let mut arguments = serde_json::json!({
        "source": path.display().to_string(),
        "entry": entry,
        "steps": steps,
        "max_ops": 64
    });
    if let Some(capture_time) = capture_time {
        arguments["capture_time"] = serde_json::json!(capture_time);
    }
    read_mcp_observation_resource(&arguments)
}

fn observe_mcp_viewport_capture(path: &Path, entry: &str) -> serde_json::Value {
    let arguments = serde_json::json!({
        "source": path.display().to_string(),
        "entry": entry,
        "image": "png",
        "steps": 4,
        "max_ops": 64
    });
    read_mcp_observation_resource(&arguments)
}

fn read_mcp_observation_resource(arguments: &serde_json::Value) -> serde_json::Value {
    let _guard = agent_mcp_stdio_guard();
    let mut child = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Tier 2 MCP discovery");
    let mut stdin = child.stdin.take().expect("MCP discovery stdin is piped");
    let mut stdout = BufReader::new(child.stdout.take().expect("MCP discovery stdout is piped"));

    let initialize = exchange_mcp_request(
        &mut stdin,
        &mut stdout,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
    );
    assert_eq!(initialize["result"]["serverInfo"]["name"], "arcweft-agent");

    let observation = exchange_mcp_request(
        &mut stdin,
        &mut stdout,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": arguments
            }
        }),
    );
    assert_eq!(
        observation["result"]["isError"], false,
        "Tier 2 MCP fixture discovery must observe its selected entry: {observation}"
    );
    let report_uri = mcp_observation_report_uri(&observation);

    let resource = exchange_mcp_request(
        &mut stdin,
        &mut stdout,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "resources/read",
            "params": {
                "uri": report_uri,
                "max_privacy": "sensitive"
            }
        }),
    );
    let report = mcp_observation_report(&resource, &report_uri);

    drop(stdin);
    drop(stdout);
    let output = child.wait_with_output().expect("wait for MCP discovery");
    assert!(
        output.status.success(),
        "Tier 2 MCP discovery should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    report
}

fn mcp_observation_report_uri(observation: &serde_json::Value) -> String {
    observation["result"]["content"]
        .as_array()
        .and_then(|content| {
            content.iter().find(|block| {
                block["type"] == "resource_link" && block["title"] == "Latest observation"
            })
        })
        .and_then(|block| block["uri"].as_str())
        .unwrap_or_else(|| {
            panic!("MCP observation must publish its typed report resource: {observation}")
        })
        .to_owned()
}

fn mcp_observation_report(resource: &serde_json::Value, report_uri: &str) -> serde_json::Value {
    let report_text = resource["result"]["contents"]
        .as_array()
        .and_then(|contents| contents.iter().find(|content| content["uri"] == report_uri))
        .and_then(|content| content["text"].as_str())
        .unwrap_or_else(|| {
            panic!("MCP observation report must be readable as JSON text: {resource}")
        });
    serde_json::from_str(report_text).expect("MCP observation report is JSON")
}

fn exchange_mcp_request(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    request: &serde_json::Value,
) -> serde_json::Value {
    writeln!(
        stdin,
        "{}",
        serde_json::to_string(request).expect("MCP discovery request serializes")
    )
    .expect("write MCP discovery request");
    stdin.flush().expect("flush MCP discovery request");

    let mut line = String::new();
    let read = stdout
        .read_line(&mut line)
        .expect("read MCP discovery response");
    assert!(read > 0, "MCP discovery must return a response");
    serde_json::from_str(&line).expect("MCP discovery response is JSON")
}

fn observed_object_id(object: &serde_json::Value) -> String {
    object["id"]
        .as_str()
        .expect("observed object has a semantic ID")
        .to_owned()
}

fn observed_object_layer(object: &serde_json::Value) -> String {
    object["layer"]
        .as_str()
        .expect("observed object has a semantic layer")
        .to_owned()
}

fn observed_capture_uri(scope: &serde_json::Value, capture_kind: &str, mime_type: &str) -> String {
    scope["capture_refs"]["captures"]
        .as_array()
        .expect("observed scope publishes typed capture references")
        .iter()
        .find(|capture| capture["kind"] == capture_kind && capture["mime_type"] == mime_type)
        .and_then(|capture| capture["uri"].as_str())
        .unwrap_or_else(|| {
            panic!("observed scope must publish {capture_kind}/{mime_type} capture URI: {scope}")
        })
        .to_owned()
}

fn find_observed_layer<'a>(report: &'a serde_json::Value, layer_id: &str) -> &'a serde_json::Value {
    report["layers"]
        .as_array()
        .expect("MCP fixture layers are observed")
        .iter()
        .find(|layer| layer["id"] == layer_id)
        .unwrap_or_else(|| panic!("authored layer `{layer_id}` should be observed"))
}

fn find_observed_rich_text_page<'a>(
    report: &'a serde_json::Value,
    text: &str,
    page: u64,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("MCP fixture objects are observed")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_page"
                && object["text"] == text
                && object["rich_text_ref"]["page"].as_u64() == Some(page)
        })
        .unwrap_or_else(|| {
            panic!("authored rich-text page `{text}` at page {page} should be observed")
        })
}

fn assert_strict_auxiliary_capture_review(response: &serde_json::Value) {
    assert_eq!(response["result"]["isError"], false);
    let metadata = mcp_content_metadata(
        &response["result"]["content"][0],
        "content-policy review metadata is JSON",
    );
    assert_eq!(
        metadata["content_policy"]["disposition"], "review",
        "non-color MCP capture must return a typed review receipt: {metadata}"
    );
    assert_eq!(
        metadata["content_policy"]["reason_codes"],
        serde_json::json!(["auxiliary_capture_not_publishable"]),
        "review receipt must retain the strict auxiliary-capture policy reason: {metadata}"
    );
    assert_eq!(
        metadata["content_policy"]["profile_id"],
        "arcweft.agent.strict"
    );
    assert_eq!(metadata["content_policy"]["sanitized"], false);
    assert_eq!(metadata["mime_type"], "application/json");
    assert!(
        metadata["uri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("arcweft://moderated/"))
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_observes_and_reads_rich_text_child_image() {
    let path = temp_mcp_arcw(
        "agent-mcp-rich-text-image",
        r#"
pub dialogue defaults {
    font = serif
}

character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let observed = observe_mcp_fixture(&path, "entry.main");
    let ruby = find_rich_text_ruby_object(&observed, 0);
    let ruby_id = observed_object_id(ruby);
    let rich_text_layer_id = observed_object_layer(ruby);
    let object_uri = observed_capture_uri(ruby, "color", "image/png");
    let rich_text_layer = find_observed_layer(&observed, &rich_text_layer_id);
    let layer_uri = observed_capture_uri(rich_text_layer, "color", "image/png");
    for source_uri in [&object_uri, &layer_uri] {
        assert!(
            source_uri.starts_with("arcweft://session/cli/frame/"),
            "observe capture refs retain their canonical source URI: {source_uri}"
        );
    }
    let requests = agent_mcp_rich_text_requests(
        &path,
        &ruby_id,
        &rich_text_layer_id,
        &object_uri,
        &layer_uri,
    );
    let responses = run_agent_mcp_rich_text_session(&requests);
    fs::remove_file(&path).expect("remove temp agent mcp source");
    assert_agent_mcp_rich_text_capture_responses(
        &responses,
        &ruby_id,
        &rich_text_layer_id,
        &object_uri,
        &layer_uri,
        &observed["tick"],
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_lists_resource_templates_before_observe() {
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "resources/templates/list", "params": {}}),
    ];
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp resource templates should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    let templates = responses[1]["result"]["resourceTemplates"]
        .as_array()
        .expect("resource templates are listed");
    assert!(templates.iter().any(|template| {
        template["name"] == "viewport-capture"
            && template["uriTemplate"]
                .as_str()
                .is_some_and(|uri| uri.contains("/{capture}.{extension}"))
    }));
    assert!(templates.iter().any(|template| {
        template["name"] == "presentation-tree"
            && template["uriTemplate"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/presentation-tree.json"))
    }));
    assert!(templates.iter().any(|template| {
        template["name"] == "presentation-tree-filter"
            && template["uriTemplate"].as_str().is_some_and(|uri| {
                uri.contains("presentation-tree.json?{filter_key}={filter_value}")
            })
    }));
    assert!(templates.iter().any(|template| {
        template["name"] == "layer-mask-capture"
            && template["uriTemplate"]
                .as_str()
                .is_some_and(|uri| uri.contains("layer.{layer_id}.mask.{extension}"))
    }));
    assert!(templates.iter().any(|template| {
        template["name"] == "layer-object-id-capture"
            && template["uriTemplate"]
                .as_str()
                .is_some_and(|uri| uri.contains("layer.{layer_id}.object-id.{extension}"))
    }));
    assert!(templates.iter().any(|template| {
        template["name"] == "object-color-capture"
            && template["description"]
                .as_str()
                .is_some_and(|description| description.contains("rich-text child objects"))
    }));
    assert!(templates.iter().any(|template| {
        template["name"] == "object-object-id-capture"
            && template["uriTemplate"]
                .as_str()
                .is_some_and(|uri| uri.contains("object.{object_id}.object-id.{extension}"))
            && template["description"]
                .as_str()
                .is_some_and(|description| description.contains("rich-text child objects"))
    }));
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: requires native-capture feature subprocess"]
fn agent_mcp_stdio_dispatches_semantic_action() {
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": {
                    "source": rich_text_showcase_path().display().to_string(),
                    "entry": "entry.main",
                    "steps": 1,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "arcweft.action",
                "arguments": {
                    "action_id": "action.advance_text.object.dialogue.0.0"
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp action should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 4);
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .expect("tools list is array")
            .iter()
            .any(|tool| tool["name"] == "arcweft.action")
    );
    assert_eq!(responses[2]["result"]["isError"], false);
    assert_eq!(responses[3]["result"]["isError"], false);
    let action = mcp_content_metadata(
        &responses[3]["result"]["content"][0],
        "MCP action result is JSON",
    );
    assert_eq!(action["accepted"], true);
    assert_eq!(action["before_tick"], 0);
    assert_eq!(action["after_tick"], 1);
    assert_eq!(action["action"]["kind"], "advance_text");
    assert_eq!(action["after"]["tick"], 1);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: requires native-capture feature subprocess"]
fn agent_mcp_stdio_debug_script_runs_reads_persisted_runs() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/mcp-script-runs-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug script runs target dir"))
        .expect("create debug script runs target dir");
    let store = DebugStore::open(&db_path).expect("open debug script runs db");
    let session_id = SessionId::new("session.mcp.script").expect("session id");
    store
        .start_session(&session_id, None, "script", "mcp", 0)
        .expect("seed session");
    let project_metadata = project_shape_metadata(1, 2, 1, 0);
    for (run_id, started_sequence) in [("run.mcp.first", 1), ("run.mcp.second", 3)] {
        let mut metadata = project_metadata.clone();
        metadata.insert("steps".to_owned(), serde_json::json!(2));
        store
            .upsert_script_run(&DebugScriptRun {
                run_id: AgentRunId::new(run_id).expect("run id"),
                session_id: session_id.clone(),
                agent_id: Some(PublicId::new("agent.mcp").expect("agent id")),
                artifact_hash: None,
                source_hash: Some(stable_hash("blake3:mcp-script-source")),
                project_binding_mode: "strict".to_owned(),
                started_sequence,
                finished_sequence: Some(started_sequence + 1),
                outcome: DebugScriptRunOutcome::Done,
                partially_effectful: started_sequence > 1,
                trace_uri: Some(format!("target/{run_id}.arcwx")),
                error: None,
                metadata,
            })
            .expect("seed script run");
    }
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.debug.script.runs",
                "arguments": {
                    "path": db_path.display().to_string(),
                    "session_id": "session.mcp.script",
                    "limit": 2
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp debug script runs should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .expect("tools list is array")
            .iter()
            .any(|tool| tool["name"] == "arcweft.debug.script.runs")
    );
    let report = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "MCP debug script runs result is JSON",
    );
    assert_eq!(report["session_id"], "session.mcp.script");
    assert_eq!(report["max_privacy"], "project");
    let runs = report["runs"].as_array().expect("runs array");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["run_id"], "run.mcp.second");
    assert_eq!(runs[0]["outcome"], "done");
    assert_eq!(runs[0]["partially_effectful"], true);
    assert_eq!(runs[0]["project"]["entity_count"], 1);
    assert_eq!(runs[0]["project"]["graph_symbol_count"], 2);
    assert_eq!(
        runs[0]["project"]["graph_summary_symbol_id"],
        "project:summary"
    );
    assert_eq!(runs[1]["run_id"], "run.mcp.first");
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: requires native-capture feature subprocess"]
fn agent_mcp_stdio_debug_search_filters_chunks_by_privacy() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/mcp-search-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug search target dir"))
        .expect("create debug search target dir");
    seed_debug_search_db(&db_path);
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.debug.search",
                "arguments": {
                    "path": db_path.display().to_string(),
                    "query": "opening",
                    "limit": 4,
                    "max_privacy": "public"
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "arcweft.debug.search",
                "arguments": {
                    "path": db_path.display().to_string(),
                    "graph_query": "opening",
                    "graph_depth": 2,
                    "limit": 10,
                    "max_privacy": "project"
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "arcweft.debug.search",
                "arguments": {
                    "path": db_path.display().to_string(),
                    "history_query": "opening",
                    "limit": 4,
                    "max_privacy": "project"
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "arcweft.debug.search",
                "arguments": {
                    "path": db_path.display().to_string(),
                    "query_vector": [1.0, 0.0],
                    "model_id": "fixture",
                    "model_revision": "1",
                    "limit": 4,
                    "max_privacy": "secret"
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp debug search should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 6);
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .expect("tools list is array")
            .iter()
            .any(|tool| tool["name"] == "arcweft.debug.search")
    );
    assert_mcp_debug_search_lexical_response(&responses[2]);
    assert_mcp_debug_search_graph_response(&responses[3]);
    assert_mcp_debug_search_history_response(&responses[4]);
    assert_mcp_debug_search_vector_response(&responses[5]);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: requires native-capture feature subprocess"]
fn agent_mcp_stdio_rag_query_indexes_source_project_context() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-rag-source/mcp-source-rag-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("MCP source RAG DB parent"))
        .expect("create MCP source RAG DB parent");
    let requests = agent_mcp_source_rag_requests(&db_path);
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp source RAG should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 4);
    let pack = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "MCP source RAG result is JSON",
    );
    assert_eq!(pack["query"]["text"], "choice.opening rich_text");
    let items = pack["items"].as_array().expect("RAG items array");
    assert!(
        items.iter().any(|item| {
            item["kind"] == "symbol"
                && item["entity_ids"]
                    .as_array()
                    .is_some_and(|ids| ids.contains(&serde_json::json!("flow.rich_text_showcase")))
                && item["chunk_id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("mcp:source.blake3:"))
        }),
        "MCP source RAG should include project symbols with MCP chunk ids: {pack}"
    );
    let explanation = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "MCP source RAG explanation is JSON",
    );
    assert_eq!(explanation["item_count"], items.len());
    let persisted = mcp_content_metadata(
        &responses[3]["result"]["content"][0],
        "MCP source RAG debug search is JSON",
    );
    assert!(
        persisted["hits"].as_array().is_some_and(|hits| {
            hits.iter().any(|hit| {
                hit["source_kind"] == "source"
                    && hit["chunk_id"]
                        .as_str()
                        .is_some_and(|id| id.starts_with("mcp:source.blake3:"))
            })
        }),
        "MCP source RAG should persist source text chunks for debug search: {persisted}"
    );

    let _ = fs::remove_file(&db_path);
    let _ = fs::remove_file(db_path.with_extension("sqlite3-shm"));
    let _ = fs::remove_file(db_path.with_extension("sqlite3-wal"));
}

fn agent_mcp_source_rag_requests(db_path: &Path) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.rag.query",
                "arguments": {
                    "sources": [
                        workspace_path("samples/agent-script/native-choice-dispatch.arcw").display().to_string(),
                        workspace_path("samples/rich-text-showcase.arcw").display().to_string()
                    ],
                    "path": db_path.display().to_string(),
                    "query": "choice.opening rich_text",
                    "roots": ["flow.rich_text_showcase"],
                    "limit": 12
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.rag.explain",
                "arguments": { "path": db_path.display().to_string() }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "arcweft.debug.search",
                "arguments": {
                    "path": db_path.display().to_string(),
                    "query": "rich_text_showcase",
                    "limit": 20,
                    "max_privacy": "project"
                }
            }
        }),
    ]
}

fn assert_mcp_debug_search_lexical_response(response: &serde_json::Value) {
    assert_eq!(response["result"]["isError"], false);
    let search = mcp_content_metadata(
        &response["result"]["content"][0],
        "MCP debug search lexical result is JSON",
    );
    assert_eq!(search["query"], "opening");
    assert_eq!(search["max_privacy"], "public");
    let hits = search["hits"].as_array().expect("hits array");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["chunk_id"], "chunk:public-opening");
    assert_eq!(hits[0]["privacy"], "public");
    assert_eq!(hits[0]["channel"], "lexical");
}

fn assert_mcp_debug_search_graph_response(response: &serde_json::Value) {
    assert_eq!(response["result"]["isError"], false);
    let search = mcp_content_metadata(
        &response["result"]["content"][0],
        "MCP debug search graph result is JSON",
    );
    assert_eq!(search["graph_query"], "opening");
    assert_eq!(search["graph_depth"], 2);
    assert_eq!(search["max_privacy"], "project");
    let hits = search["hits"].as_array().expect("graph hits array");
    assert!(
        hits.iter().any(|hit| hit["chunk_id"] == "graph:1"
            && hit["channel"] == "graph"
            && hit["privacy"] == "project"
            && hit["body"]
                .as_str()
                .is_some_and(|body| body.contains("distance=1"))),
        "graph search should include the directly matched edge: {search}"
    );
    assert!(
        hits.iter().any(|hit| hit["chunk_id"] == "graph:2"
            && hit["channel"] == "graph"
            && hit["body"]
                .as_str()
                .is_some_and(|body| body.contains("distance=2"))),
        "graph search should include the 2-hop expanded symbol: {search}"
    );
}

fn assert_mcp_debug_search_history_response(response: &serde_json::Value) {
    assert_eq!(response["result"]["isError"], false);
    let search = mcp_content_metadata(
        &response["result"]["content"][0],
        "MCP debug search history result is JSON",
    );
    assert_eq!(search["history_query"], "opening");
    let hits = search["hits"].as_array().expect("history hits array");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["chunk_id"], "history:history:opening-fix");
    assert_eq!(hits[0]["channel"], "history");
    assert_eq!(hits[0]["privacy"], "project");
}

fn assert_mcp_debug_search_vector_response(response: &serde_json::Value) {
    assert_eq!(response["result"]["isError"], false);
    let search = mcp_content_metadata(
        &response["result"]["content"][0],
        "MCP debug search vector result is JSON",
    );
    assert_eq!(search["query_vector_dimensions"], 2);
    let hits = search["hits"].as_array().expect("vector hits array");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0]["chunk_id"], "chunk:secret-opening");
    assert_eq!(hits[0]["channel"], "vector");
    assert_eq!(hits[0]["privacy"], "secret");
    assert_eq!(hits[1]["chunk_id"], "chunk:public-opening");
}

fn assert_agent_mcp_rich_text_capture_responses(
    responses: &[serde_json::Value],
    ruby_id: &str,
    rich_text_layer_id: &str,
    object_uri: &str,
    layer_uri: &str,
    observed_tick: &serde_json::Value,
) {
    assert_eq!(responses.len(), 14);
    assert_eq!(
        responses[0]["result"]["serverInfo"]["name"],
        "arcweft-agent"
    );
    let tools = responses[1]["result"]["tools"]
        .as_array()
        .expect("MCP tool catalog");
    for name in ["arcweft.observe", "arcweft.capture", "arcweft.session.info"] {
        assert!(tools.iter().any(|tool| tool["name"] == name));
    }
    assert!(
        responses[2]["result"]["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|content| content["type"] == "resource_link"
                && content["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.starts_with("arcweft://moderated/")))
    );
    let initial_layer_resource = responses[3]["result"]["resources"]
        .as_array()
        .expect("resources are listed after native observation")
        .iter()
        .find(|resource| {
            resource["image"]["kind"] == "color"
                && resource["image"]["scope"]["kind"] == "layer"
                && resource["image"]["renderer"] == "native"
        })
        .expect("native observation publishes its selected layer color resource");
    assert_eq!(
        initial_layer_resource["image"]["scope"]["id"],
        initial_layer_resource["image"]["selected_capture"]["scope"]["id"]
    );
    assert_eq!(
        initial_layer_resource["image"]["scope"]["id"],
        initial_layer_resource["image"]["selected_capture"]["source"]["id"]
    );
    assert_published_resource_uri(initial_layer_resource);
    assert_mcp_png_capture_content(&responses[4], "ruby capture metadata is JSON");
    assert_mcp_png_capture_content(&responses[5], "native ruby capture metadata is JSON");
    assert_strict_auxiliary_capture_review(&responses[6]);
    let observation_uri = mcp_observation_report_uri(&responses[2]);
    let mask_review = mcp_content_metadata(
        &responses[6]["result"]["content"][0],
        "mask review metadata is JSON",
    );
    let mask_review_uri = mask_review["uri"].as_str().expect("mask review URI");
    assert_mcp_session_info_after_capture(
        &responses[7],
        ruby_id,
        rich_text_layer_id,
        &observation_uri,
        mask_review_uri,
        observed_tick,
    );
    let source_object_read_uri = assert_png_resource_read_content(&responses[8]);
    let source_layer_read_uri = assert_png_resource_read_content(&responses[9]);
    assert_strict_auxiliary_capture_review(&responses[10]);

    let object_id_review = mcp_content_metadata(
        &responses[10]["result"]["content"][0],
        "object-id review metadata is JSON",
    );
    let resources = responses[11]["result"]["resources"]
        .as_array()
        .expect("resources remain listable after policy review");
    let object_resource = published_capture_resource(resources, source_object_read_uri, "object");
    let layer_resource = published_capture_resource(resources, source_layer_read_uri, "layer");
    assert_published_capture_scope(object_resource);
    assert_published_capture_scope(layer_resource);
    assert_ne!(source_object_read_uri, object_uri);
    assert_ne!(source_layer_read_uri, layer_uri);
    assert_eq!(
        assert_png_resource_read_content(&responses[12]),
        source_object_read_uri
    );
    assert_eq!(
        assert_png_resource_read_content(&responses[13]),
        source_layer_read_uri
    );
    for review_uri in [
        mask_review["uri"].as_str().expect("mask review URI"),
        object_id_review["uri"]
            .as_str()
            .expect("object-id review URI"),
    ] {
        assert!(
            resources
                .iter()
                .any(|resource| resource["uri"] == review_uri),
            "policy review resource should be published: {review_uri}"
        );
    }
}

fn published_capture_resource<'a>(
    resources: &'a [serde_json::Value],
    published_uri: &str,
    scope_kind: &str,
) -> &'a serde_json::Value {
    resources
        .iter()
        .find(|resource| {
            resource["uri"] == published_uri
                && resource["image"]["kind"] == "color"
                && resource["image"]["renderer"] == "native"
                && resource["image"]["scope"]["kind"] == scope_kind
        })
        .unwrap_or_else(|| {
            panic!(
                "resources/list must expose published native {scope_kind} capture `{published_uri}`"
            )
        })
}

fn assert_published_capture_scope(resource: &serde_json::Value) {
    assert_eq!(
        resource["image"]["scope"]["id"],
        resource["image"]["selected_capture"]["scope"]["id"]
    );
    assert_eq!(
        resource["image"]["scope"]["id"],
        resource["image"]["selected_capture"]["source"]["id"]
    );
    assert_published_resource_uri(resource);
}

fn assert_policy_published_image_metadata<'a>(
    metadata: &'a serde_json::Value,
    scope_kind: &str,
) -> &'a str {
    assert_eq!(metadata["image"]["scope"]["kind"], scope_kind);
    assert_eq!(
        metadata["image"]["scope"]["id"],
        metadata["image"]["selected_capture"]["scope"]["id"]
    );
    assert_eq!(
        metadata["image"]["scope"]["id"],
        metadata["image"]["selected_capture"]["source"]["id"]
    );
    metadata["uri"]
        .as_str()
        .filter(|uri| uri.starts_with("arcweft://moderated/"))
        .unwrap_or_else(|| {
            panic!("capture metadata must expose its policy-published URI: {metadata}")
        })
}

fn assert_published_resource_uri(resource: &serde_json::Value) -> &str {
    resource["uri"]
        .as_str()
        .filter(|uri| uri.starts_with("arcweft://moderated/"))
        .unwrap_or_else(|| panic!("resource must expose its policy-published URI: {resource}"))
}

fn assert_mcp_session_info_after_capture(
    response: &serde_json::Value,
    ruby_id: &str,
    rich_text_layer_id: &str,
    observation_uri: &str,
    mask_review_uri: &str,
    observed_tick: &serde_json::Value,
) {
    assert_eq!(response["result"]["content"][0]["type"], "text");
    let info = mcp_content_metadata(
        &response["result"]["content"][0],
        "session info content is JSON",
    );
    assert_eq!(info["observed"], true);
    assert_eq!(info["session_id"], "cli");
    assert_eq!(&info["tick"], observed_tick);
    assert!(info["resource_count"].as_u64().unwrap() > 0);
    assert_eq!(info["capture_resource_count"], 2);
    assert_eq!(info["shared_capture_session_active"], true);
    assert_eq!(
        info["project"]["project_graph"]["has_project_summary"],
        true
    );
    assert!(
        info["project"]["project_graph"]["project_summary"]["entity_count"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "session info should expose native project graph summary: {info}"
    );
    assert!(
        info["latest_capture"].is_null(),
        "a reviewed auxiliary capture must not publish image metadata: {info}"
    );
    assert_eq!(info["latest_capture_uri"], mask_review_uri);
    assert_eq!(info["latest_capture_resource"]["uri"], mask_review_uri);
    assert_eq!(
        info["latest_capture_resource"]["mimeType"],
        "application/json"
    );
    assert!(
        info["latest_capture_resource"]["size"]
            .as_u64()
            .is_some_and(|size| size > 0),
        "latest resource should expose the published review receipt size: {info}"
    );
    assert_mcp_session_info_resource_templates(&info);
    assert!(info["layers"].as_array().unwrap().iter().any(|layer| {
        layer["id"] == rich_text_layer_id
            && layer["capture_refs"]["captures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|capture| {
                    capture["uri"].as_str().is_some_and(|uri| {
                        uri.ends_with(&format!("/layer.{rich_text_layer_id}.png"))
                    })
                })
    }));
    assert!(info["objects"].as_array().unwrap().iter().any(|object| {
        object["id"] == ruby_id
            && object["rich_text_ref"]["kind"] == "ruby"
            && object["capture_refs"]["captures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|capture| {
                    capture["uri"]
                        .as_str()
                        .is_some_and(|uri| uri.ends_with(&format!("/object.{ruby_id}.png")))
                })
    }));
    assert!(
        info["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"] == observation_uri),
        "session catalog should retain the observe tool's published report: {info}"
    );
}

fn assert_mcp_session_info_resource_templates(info: &serde_json::Value) {
    let templates = info["resource_templates"]
        .as_array()
        .expect("session info resource templates are returned");
    assert!(templates.iter().any(|template| {
        template["name"] == "object-mask-capture"
            && template["uriTemplate"]
                .as_str()
                .is_some_and(|uri| uri.contains("object.{object_id}.mask.{extension}"))
    }));
    assert!(templates.iter().any(|template| {
        template["name"] == "presentation-tree"
            && template["uriTemplate"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/presentation-tree.json"))
    }));
    assert!(templates.iter().any(|template| {
        template["name"] == "presentation-tree-filter"
            && template["uriTemplate"].as_str().is_some_and(|uri| {
                uri.contains("presentation-tree.json?{filter_key}={filter_value}")
            })
    }));
}

fn assert_mcp_png_capture_content(response: &serde_json::Value, metadata_context: &str) {
    let metadata = assert_mcp_png_capture_transport(response, metadata_context);
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
}

fn assert_mcp_png_capture_transport(
    response: &serde_json::Value,
    metadata_context: &str,
) -> serde_json::Value {
    assert_eq!(
        response["result"]["content"][0]["type"], "text",
        "MCP PNG capture must return typed metadata before image content: {response}"
    );
    let metadata = mcp_content_metadata(&response["result"]["content"][0], metadata_context);
    assert!(metadata["image"]["width"].as_u64().unwrap() > 0);
    assert!(metadata["image"]["height"].as_u64().unwrap() > 0);
    assert_eq!(metadata["image"]["kind"], "color");
    assert!(
        response["result"]["content"][1]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
    assert_eq!(response["result"]["content"][1]["mimeType"], "image/png");
    metadata
}

fn mcp_raw_capture_bytes(response: &serde_json::Value) -> Vec<u8> {
    let blob = response["result"]["content"][1]["resource"]["blob"]
        .as_str()
        .expect("raw capture response has a resource blob");
    general_purpose::STANDARD
        .decode(blob)
        .expect("raw capture blob is base64")
}

fn assert_png_resource_read_content(response: &serde_json::Value) -> &str {
    let content = &response["result"]["contents"][0];
    assert_eq!(content["mimeType"], "image/png");
    let uri = content["uri"]
        .as_str()
        .expect("PNG resource read publishes its resolved URI");
    let blob = content["blob"].as_str().expect("PNG resource has a blob");
    let bytes = general_purpose::STANDARD
        .decode(blob)
        .expect("PNG resource blob is base64");
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "resources/read should keep earlier session capture bytes after later captures"
    );
    uri
}

fn mcp_content_metadata(block: &serde_json::Value, parse_message: &str) -> serde_json::Value {
    serde_json::from_str(block["text"].as_str().unwrap()).expect(parse_message)
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_without_prior_observe() {
    let path = temp_mcp_arcw(
        "agent-mcp-direct-capture",
        r#"
pub dialogue defaults {
    font = serif
}

character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let observed = observe_mcp_fixture(&path, "entry.main");
    let ruby = find_rich_text_ruby_object(&observed, 0);
    let rich_text_layer_id = observed_object_layer(ruby);
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "format": "png",
                    "capture": "color",
                    "layer": rich_text_layer_id,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list", "params": {}}),
    ];
    let output = run_agent_mcp_stdio_local_dev(&requests);
    fs::remove_file(&path).expect("remove temp direct capture source");
    assert!(
        output.status.success(),
        "agent mcp direct capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_agent_mcp_direct_capture_responses(&responses);
}

fn assert_agent_mcp_direct_capture_responses(responses: &[serde_json::Value]) {
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[1]["result"]["content"][0]["type"], "text");
    let direct_capture_metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "direct capture metadata is JSON",
    );
    assert!(
        direct_capture_metadata["image"]["width"]
            .as_u64()
            .is_some_and(|width| width > 0 && width <= 1280),
        "direct rich-text layer capture must stay inside the authored View viewport"
    );
    assert_eq!(direct_capture_metadata["image"]["renderer"], "native");
    assert!(
        matches!(
            direct_capture_metadata["image"]["composition"].as_str(),
            Some("masked_framebuffer_crop" | "framebuffer_crop")
        ),
        "direct rich-text layer capture should use a native composition"
    );
    assert_eq!(direct_capture_metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(
        direct_capture_metadata["image"]["scope"]["id"],
        direct_capture_metadata["image"]["selected_capture"]["scope"]["id"]
    );
    assert_eq!(
        direct_capture_metadata["image"]["scope"]["id"],
        direct_capture_metadata["image"]["selected_capture"]["source"]["id"]
    );
    assert_eq!(
        direct_capture_metadata["image"]["crop_origin"]["space"],
        "viewport"
    );
    assert_eq!(
        direct_capture_metadata["image"]["crop_origin"]["x"].as_f64(),
        direct_capture_metadata["image"]["selected_capture"]["crop"]["clipped"]["origin"]["x"]
            .as_f64()
    );
    assert_eq!(
        direct_capture_metadata["image"]["width"].as_f64(),
        direct_capture_metadata["image"]["selected_capture"]["crop"]["clipped"]["size"]["width"]
            .as_f64()
    );
    assert_eq!(
        direct_capture_metadata["image"]["height"].as_f64(),
        direct_capture_metadata["image"]["selected_capture"]["crop"]["clipped"]["size"]["height"]
            .as_f64()
    );
    assert!(
        direct_capture_metadata["image"]["content_bbox"]["width"]
            .as_u64()
            .is_some_and(|width| width > 0)
    );
    assert!(
        direct_capture_metadata["image"]["content_bbox"]["height"]
            .as_u64()
            .is_some_and(|height| height > 0)
    );
    assert!(
        direct_capture_metadata["image"]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(responses[1]["result"]["content"][1]["type"], "image");
    assert_eq!(
        responses[1]["result"]["content"][1]["mimeType"],
        "image/png"
    );
    assert!(
        responses[1]["result"]["content"][1]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
    assert_agent_mcp_direct_capture_resources(&responses[2]);
}

fn assert_agent_mcp_direct_capture_resources(response: &serde_json::Value) {
    let resources = response["result"]["resources"].as_array().unwrap();
    let layer_image = resources
        .iter()
        .find(|resource| {
            resource["image"]["kind"] == "color"
                && resource["image"]["scope"]["kind"] == "layer"
                && resource["image"]["renderer"] == "native"
        })
        .expect("direct capture should expose the selected layer image resource");
    let canonical_layer_id = layer_image["image"]["scope"]["id"]
        .as_str()
        .expect("selected layer resource has a canonical semantic ID");
    assert!(
        layer_image["description"]
            .as_str()
            .is_some_and(|description| description.contains("kind=color")
                && description.contains("renderer=native")
                && description.contains(&format!("scope=layer:{canonical_layer_id}"))
                && description.contains("width=")
                && description.contains("height=")),
        "direct capture layer descriptor should expose image metadata"
    );
    assert!(
        resources.iter().any(|resource| {
            resource["image"]["scope"]["kind"] == "object"
                && resource["image"]["selected_capture"]["source"]["role"] == "rich_text_ruby"
        }),
        "direct capture should publish authored View rich-text child resources"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_with_native_renderer() {
    let path = temp_mcp_arcw(
        "agent-mcp-native-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let observed = observe_mcp_viewport_capture(&path, "entry.main");
    let dialogue_view_bbox = find_dialogue_view_object(&observed)["bbox"].clone();
    let viewport_uri = observed["images"][0]["uri"]
        .as_str()
        .expect("MCP viewport discovery publishes its typed capture URI")
        .to_owned();
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "format": "png",
                    "capture": "color",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.resource.read",
                "arguments": {
                    "uri": viewport_uri,
                    "max_privacy": "sensitive"
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio_local_dev(&requests);
    fs::remove_file(&path).expect("remove temp native MCP source");
    assert!(
        output.status.success(),
        "agent mcp native capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    assert_mcp_png_capture_content(&responses[1], "native capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native capture metadata is JSON",
    );
    assert_eq!(metadata["image"]["renderer"], "native");
    let published_viewport_uri = assert_policy_published_image_metadata(&metadata, "viewport");
    assert_ne!(published_viewport_uri, viewport_uri);
    assert_eq!(
        metadata["image"]["content_bbox"]["x"],
        dialogue_view_bbox["x"]
    );
    assert_eq!(
        metadata["image"]["content_bbox"]["y"],
        dialogue_view_bbox["y"]
    );
    assert_eq!(
        metadata["image"]["content_bbox"]["width"],
        dialogue_view_bbox["width"]
    );
    assert_eq!(
        metadata["image"]["content_bbox"]["height"],
        dialogue_view_bbox["height"]
    );
    assert_mcp_png_capture_content(&responses[2], "native capture resource metadata is JSON");
    let read_metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "native capture resource metadata is JSON",
    );
    assert_eq!(read_metadata["image"]["renderer"], "native");
    assert_eq!(
        assert_policy_published_image_metadata(&read_metadata, "viewport"),
        published_viewport_uri
    );
    assert_eq!(read_metadata["image"]["composition"], "framebuffer");
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_clear_after_page_object_with_native_renderer() {
    let path = temp_mcp_arcw(
        "agent-mcp-native-page-object-capture",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let observed = observe_mcp_fixture(&path, "entry.main");
    let page = find_observed_rich_text_page(&observed, "After", 1);
    let page_id = observed_object_id(page);
    let page_bbox = page["bbox"].clone();
    let page_uri = observed_capture_uri(page, "color", "image/png");
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "format": "png",
                    "capture": "color",
                    "object": page_id,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio_local_dev(&requests);
    fs::remove_file(&path).expect("remove temp native page-object MCP source");
    assert!(
        output.status.success(),
        "agent mcp native page object capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    let metadata = assert_mcp_png_capture_transport(
        &responses[1],
        "native page object capture metadata is JSON",
    );
    assert_eq!(metadata["image"]["renderer"], "native");
    let published_page_uri = assert_policy_published_image_metadata(&metadata, "object");
    assert_ne!(published_page_uri, page_uri);
    assert!(metadata["image"]["page"].is_null());
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert_eq!(metadata["image"]["crop_origin"]["x"], page_bbox["x"]);
    assert_eq!(metadata["image"]["crop_origin"]["y"], page_bbox["y"]);
    assert_eq!(metadata["image"]["width"], page_bbox["width"]);
    assert_eq!(metadata["image"]["height"], page_bbox["height"]);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_reads_published_semantic_page_capture_ref_with_native_renderer() {
    let path = temp_mcp_arcw(
        "agent-mcp-native-page-query-read",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let observed = observe_mcp_fixture(&path, "entry.main");
    let page = find_observed_rich_text_page(&observed, "After", 1);
    let page_bbox = page["bbox"].clone();
    let uri = observed_capture_uri(page, "color", "image/png");
    assert_eq!(page["rich_text_ref"]["page"], 1);
    let color_capture = page["capture_refs"]["captures"]
        .as_array()
        .expect("semantic page publishes capture refs")
        .iter()
        .find(|capture| capture["kind"] == "color" && capture["mime_type"] == "image/png")
        .expect("semantic page publishes a PNG color capture ref");
    assert!(color_capture["page"].is_null());
    assert!(
        !uri.contains("?page="),
        "semantic rich-text page identity must not become a runtime page query: {uri}"
    );
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.resource.read",
                "arguments": {
                    "uri": uri,
                    "max_privacy": "sensitive"
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio_local_dev(&requests);
    fs::remove_file(&path).expect("remove temp native page-query MCP source");
    assert!(
        output.status.success(),
        "agent mcp native page-query read should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    let metadata =
        assert_mcp_png_capture_transport(&responses[2], "native page-query read metadata is JSON");
    let published_page_uri = assert_policy_published_image_metadata(&metadata, "object");
    assert_ne!(published_page_uri, uri);
    assert_eq!(metadata["image"]["renderer"], "native");
    assert!(metadata["image"]["page"].is_null());
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert_eq!(metadata["image"]["crop_origin"]["x"], page_bbox["x"]);
    assert_eq!(metadata["image"]["crop_origin"]["y"], page_bbox["y"]);
    assert_eq!(metadata["image"]["width"], page_bbox["width"]);
    assert_eq!(metadata["image"]["height"], page_bbox["height"]);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_object_with_native_renderer() {
    let path = temp_mcp_arcw(
        "agent-mcp-native-object-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let observed = observe_mcp_fixture(&path, "entry.main");
    let dialogue_view = find_dialogue_view_object(&observed);
    let dialogue_view_id = observed_object_id(dialogue_view);
    let dialogue_view_bbox = dialogue_view["bbox"].clone();
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "format": "png",
                    "capture": "color",
                    "object": dialogue_view_id,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio_local_dev(&requests);
    fs::remove_file(&path).expect("remove temp native MCP object source");
    assert!(
        output.status.success(),
        "agent mcp native object capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    assert_mcp_png_capture_content(&responses[1], "native object capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native object capture metadata is JSON",
    );
    let capture_width = metadata["image"]["width"].as_u64().unwrap();
    let capture_height = metadata["image"]["height"].as_u64().unwrap();
    assert_eq!(Some(capture_width), dialogue_view_bbox["width"].as_u64());
    assert_eq!(Some(capture_height), dialogue_view_bbox["height"].as_u64());
    assert_eq!(
        metadata["image"]["crop_origin"]["x"],
        dialogue_view_bbox["x"]
    );
    assert_eq!(
        metadata["image"]["crop_origin"]["y"],
        dialogue_view_bbox["y"]
    );
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        metadata["image"]["content_pixels"].as_u64().unwrap() <= capture_width * capture_height,
        "native object color capture must stay bounded by the authored dialogue View crop"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_layer_with_native_renderer() {
    let path = temp_mcp_arcw(
        "agent-mcp-native-layer-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let observed = observe_mcp_fixture(&path, "entry.main");
    let authored_layer = observed_object_layer(find_rich_text_ruby_object(&observed, 0));
    let dialogue_view_bbox = find_dialogue_view_object(&observed)["bbox"].clone();
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "format": "png",
                    "capture": "color",
                    "layer": authored_layer,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list", "params": {}}),
    ];
    let output = run_agent_mcp_stdio_local_dev(&requests);
    fs::remove_file(&path).expect("remove temp native MCP layer source");
    assert!(
        output.status.success(),
        "agent mcp native layer capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    assert_mcp_png_capture_content(&responses[1], "native layer capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native layer capture metadata is JSON",
    );
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(
        metadata["image"]["scope"]["id"],
        metadata["image"]["selected_capture"]["scope"]["id"]
    );
    assert_eq!(
        metadata["image"]["scope"]["id"],
        metadata["image"]["selected_capture"]["source"]["id"]
    );
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert!(
        metadata["image"]["width"].as_u64().unwrap()
            <= dialogue_view_bbox["width"].as_u64().unwrap()
    );
    assert!(
        metadata["image"]["height"].as_u64().unwrap()
            <= dialogue_view_bbox["height"].as_u64().unwrap()
    );
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
    assert_native_layer_resource_descriptor(
        &responses[2],
        metadata["image"]["scope"]["id"]
            .as_str()
            .expect("canonical layer ID"),
    );
}

fn assert_native_layer_resource_descriptor(response: &serde_json::Value, canonical_layer_id: &str) {
    let resources = response["result"]["resources"].as_array().unwrap();
    let layer_image = resources
        .iter()
        .find(|resource| {
            resource["image"]["scope"]["kind"] == "layer"
                && resource["image"]["scope"]["id"] == canonical_layer_id
                && resource["image"]["renderer"] == "native"
                && resource["image"]["kind"] == "color"
        })
        .expect("resources/list should expose the latest native layer capture");
    assert_eq!(layer_image["mimeType"], "image/png");
    assert!(
        layer_image["description"]
            .as_str()
            .is_some_and(|description| description.contains("kind=color")
                && description.contains("renderer=native")
                && description.contains(&format!("scope=layer:{canonical_layer_id}"))
                && description.contains("composition=masked_framebuffer_crop")
                && description.contains("width=")
                && description.contains("height=")),
        "native layer descriptor should expose latest capture metadata"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_reads_latest_native_layer_image_resource() {
    let path = temp_mcp_arcw(
        "agent-mcp-native-layer-read-resource",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let observed = observe_mcp_fixture(&path, "entry.main");
    let ruby = find_rich_text_ruby_object(&observed, 0);
    let rich_text_layer_id = observed_object_layer(ruby);
    let dialogue_view_bbox = find_dialogue_view_object(&observed)["bbox"].clone();
    let rich_text_layer = find_observed_layer(&observed, &rich_text_layer_id);
    let layer_uri = observed_capture_uri(rich_text_layer, "color", "image/png");
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "image": "png",
                    "layer": rich_text_layer_id,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.resource.read",
                "arguments": {
                    "uri": layer_uri,
                    "max_privacy": "sensitive"
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio_local_dev(&requests);
    fs::remove_file(&path).expect("remove temp native MCP layer resource source");
    assert!(
        output.status.success(),
        "agent mcp native layer resource read should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    assert_mcp_png_capture_content(&responses[2], "native layer read metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "native layer read metadata is JSON",
    );
    let published_layer_uri = assert_policy_published_image_metadata(&metadata, "layer");
    assert_ne!(published_layer_uri, layer_uri);
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert!(
        metadata["image"]["width"].as_u64().unwrap()
            <= dialogue_view_bbox["width"].as_u64().unwrap()
    );
    assert!(
        metadata["image"]["height"].as_u64().unwrap()
            <= dialogue_view_bbox["height"].as_u64().unwrap()
    );
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
    assert!(
        metadata["image"]["crop_origin"]["x"].as_u64().unwrap()
            >= dialogue_view_bbox["x"].as_u64().unwrap()
    );
    assert!(
        metadata["image"]["crop_origin"]["y"].as_u64().unwrap()
            >= dialogue_view_bbox["y"].as_u64().unwrap()
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: animated image live resource readback"]
fn agent_mcp_stdio_reads_animated_image_layer_resource() {
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let observed =
        observe_mcp_fixture_with_timing(&source_path, "entry.image_sprite_overlay", 2, Some(0.15));
    let foreground_layer = find_observed_layer(&observed, "layer.foreground");
    let layer_uri = observed_capture_uri(foreground_layer, "color", "application/octet-stream");
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": {
                    "source": source_path.display().to_string(),
                    "entry": "entry.image_sprite_overlay",
                    "steps": 2,
                    "capture_time": 0.15,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.resource.read",
                "arguments": {
                    "uri": layer_uri,
                    "max_privacy": "sensitive"
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "resources/read",
            "params": {
                "uri": layer_uri,
                "max_privacy": "sensitive"
            }
        }),
    ];
    let output = run_agent_mcp_stdio_local_dev(&requests);
    assert!(
        output.status.success(),
        "agent mcp animated image layer resource read should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 4);
    assert_eq!(
        responses[1]["result"]["isError"], false,
        "animated-image observation must establish the selected entry: {}",
        responses[1]
    );

    let metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "animated image MCP tool resource metadata is JSON",
    );
    let published_layer_uri = assert_policy_published_image_metadata(&metadata, "layer");
    assert_ne!(published_layer_uri, layer_uri);
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert_eq!(metadata["image"]["capture_time_millis"], 150);
    assert_eq!(metadata["image"]["width"], 360);
    assert_eq!(metadata["image"]["height"], 180);
    assert_eq!(metadata["image"]["crop_origin"]["x"], 120);
    assert_eq!(metadata["image"]["crop_origin"]["y"], 84);
    assert!(
        metadata["image"]["content_pixels"]
            .as_u64()
            .is_some_and(|pixels| pixels > 0 && pixels <= 360 * 180)
    );
    let tool_bytes = mcp_raw_capture_bytes(&responses[2]);
    assert_eq!(tool_bytes.len(), 360 * 180 * 4);
    let nontransparent_pixels = tool_bytes
        .chunks_exact(4)
        .filter(|pixel| pixel[3] != 0)
        .count() as u64;
    assert_eq!(
        metadata["image"]["content_pixels"].as_u64(),
        Some(nontransparent_pixels),
        "masked animated capture metadata must describe the returned pixel payload"
    );

    let content = &responses[3]["result"]["contents"][0];
    assert_eq!(content["uri"], published_layer_uri);
    assert_eq!(content["mimeType"], "application/octet-stream");
    let read_blob = content["blob"]
        .as_str()
        .expect("animated image resources/read returns a blob");
    let read_bytes = general_purpose::STANDARD
        .decode(read_blob)
        .expect("animated image resources/read blob is base64");
    assert_eq!(read_bytes, tool_bytes);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_ruby_element_with_native_renderer() {
    let path = temp_mcp_arcw(
        "agent-mcp-native-ruby-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let observed = observe_mcp_fixture(&path, "entry.main");
    let ruby = find_rich_text_ruby_object(&observed, 0);
    let ruby_id = observed_object_id(ruby);
    let ruby_bbox = ruby["bbox"].clone();
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "format": "png",
                    "capture": "color",
                    "object": ruby_id,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio_local_dev(&requests);
    fs::remove_file(&path).expect("remove temp native MCP ruby source");
    assert!(
        output.status.success(),
        "agent mcp native ruby capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    assert_mcp_png_capture_content(&responses[1], "native ruby capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native ruby capture metadata is JSON",
    );
    assert_policy_published_image_metadata(&metadata, "object");
    assert_eq!(metadata["image"]["width"], ruby_bbox["width"]);
    assert_eq!(metadata["image"]["height"], ruby_bbox["height"]);
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
    assert_eq!(metadata["image"]["crop_origin"]["x"], ruby_bbox["x"]);
    assert_eq!(metadata["image"]["crop_origin"]["y"], ruby_bbox["y"]);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_requires_review_for_ruby_object_id_capture() {
    let path = temp_mcp_arcw(
        "agent-mcp-native-ruby-object-id",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let observed = observe_mcp_fixture(&path, "entry.main");
    let ruby = find_rich_text_ruby_object(&observed, 0);
    let ruby_id = observed_object_id(ruby);
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "format": "png",
                    "capture": "object-id",
                    "object": ruby_id,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native MCP ruby object-id source");
    assert!(
        output.status.success(),
        "agent mcp native ruby object-id should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    assert_strict_auxiliary_capture_review(&responses[1]);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_requires_review_for_text_combine_mask_capture() {
    let path = temp_mcp_arcw(
        "agent-mcp-strict-text-combine-mask-review",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl][.typewriter cps=1]2026[/][/][p]
}
",
    );
    let observed = observe_mcp_fixture(&path, "entry.main");
    let cluster = find_rich_text_cluster_object(&observed, "2026", 0, 4);
    let object_id = observed_object_id(cluster);
    let requests = mcp_auxiliary_capture_requests(&path, "mask", &object_id);
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp strict MCP text-combine source");
    assert!(
        output.status.success(),
        "strict MCP text-combine mask request should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    assert_strict_auxiliary_capture_review(&responses[1]);
}

fn mcp_auxiliary_capture_requests(
    path: &Path,
    capture_kind: &str,
    object_id: &str,
) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "format": "raw-rgba",
                    "capture": capture_kind,
                    "object": object_id,
                    "capture_time": 4.0,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ]
}

fn run_agent_mcp_rich_text_session(requests: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let _guard = agent_mcp_stdio_guard();
    let mut child = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("mcp")
        .args(["--content-policy-mode", "local-dev"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rich-text MCP session");
    let mut stdin = child.stdin.take().expect("rich-text MCP stdin is piped");
    let mut stdout = BufReader::new(child.stdout.take().expect("rich-text MCP stdout is piped"));
    let mut responses = requests
        .iter()
        .map(|request| exchange_mcp_request(&mut stdin, &mut stdout, request))
        .collect::<Vec<_>>();

    let published_object_uri = responses[8]["result"]["contents"][0]["uri"]
        .as_str()
        .expect("source object alias resolves to a published URI")
        .to_owned();
    let published_layer_uri = responses[9]["result"]["contents"][0]["uri"]
        .as_str()
        .expect("source layer alias resolves to a published URI")
        .to_owned();
    for (id, uri) in [(13, published_object_uri), (14, published_layer_uri)] {
        responses.push(exchange_mcp_request(
            &mut stdin,
            &mut stdout,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "resources/read",
                "params": {
                    "uri": uri,
                    "max_privacy": "sensitive"
                }
            }),
        ));
    }

    drop(stdin);
    drop(stdout);
    let output = child
        .wait_with_output()
        .expect("wait for rich-text MCP session");
    assert!(
        output.status.success(),
        "rich-text MCP session should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    responses
}

fn agent_mcp_rich_text_requests(
    path: &std::path::Path,
    ruby_id: &str,
    rich_text_layer_id: &str,
    object_uri: &str,
    layer_uri: &str,
) -> Vec<serde_json::Value> {
    let mut requests = vec![
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": {
                    "source": path.display().to_string(),
                    "entry": "entry.main",
                    "image": "png",
                    "layer": rich_text_layer_id,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "id": 4, "method": "resources/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "format": "png",
                    "capture": "color",
                    "object": ruby_id
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "format": "png",
                    "capture": "color",
                    "object": ruby_id
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "format": "raw-rgba",
                    "capture": "mask",
                    "object": ruby_id
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "arcweft.session.info",
                "arguments": {}
            }
        }),
    ];
    requests.extend(agent_mcp_rich_text_readback_requests(
        ruby_id, object_uri, layer_uri,
    ));
    requests
}

fn agent_mcp_rich_text_readback_requests(
    ruby_id: &str,
    object_uri: &str,
    layer_uri: &str,
) -> [serde_json::Value; 4] {
    [
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "resources/read",
            "params": {
                "uri": object_uri,
                "max_privacy": "sensitive"
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "resources/read",
            "params": {
                "uri": layer_uri,
                "max_privacy": "sensitive"
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "format": "raw-rgba",
                    "capture": "object-id",
                    "object": ruby_id
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "resources/list",
            "params": {}
        }),
    ]
}
