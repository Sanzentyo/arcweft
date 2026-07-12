#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_observes_and_reads_rich_text_child_image() {
    let path = temp_arcw(
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
    let requests = agent_mcp_rich_text_requests(&path);
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp agent mcp source");
    assert!(
        output.status.success(),
        "agent mcp should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_agent_mcp_rich_text_capture_responses(&responses);
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
fn agent_mcp_stdio_observes_profile_selected_dialogue_defaults() {
    let dir = temp_dir("agent-mcp-profile-dialogue-defaults");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).expect("create profiled MCP source dir");
    fs::write(src_dir.join("main.arcw"), profiled_observe_source())
        .expect("write profiled MCP source");
    let manifest_path = dir.join("arcw.toml");
    fs::write(&manifest_path, profiled_observe_manifest()).expect("write profiled MCP manifest");

    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": {
                    "manifest": manifest_path.display().to_string(),
                    "profile": "mobile",
                    "steps": 4,
                    "max_ops": 128
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.session.info",
                "arguments": {}
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_dir_all(&dir).expect("remove profiled MCP fixture");
    assert!(
        output.status.success(),
        "profiled agent mcp should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[1]["result"]["isError"], false);

    let session = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "profiled MCP session info is JSON",
    );
    assert_eq!(session["observed"], true);
    assert_eq!(session["source"], "main.arcw");
    let dialogue_view = session["objects"]
        .as_array()
        .expect("MCP session objects are listed")
        .iter()
        .find(|object| object["role"] == "dialogue_view")
        .unwrap_or_else(|| panic!("MCP profile observation should include dialogue_view: {session}"));
    let contributions = observed_object_rich_text_frame(dialogue_view)["style_contributions"]
        .as_array()
        .expect("MCP dialogue_view style contributions are reported");
    assert!(contributions.iter().any(|contribution| {
        contribution["path"] == "view"
            && contribution["value"] == "@view.MobileDialogue"
            && contribution["source"]["item_id"] == "dialogue.mobile"
            && contribution["active"] == true
    }));
    assert!(contributions.iter().any(|contribution| {
        contribution["path"] == "rich_text.ruby.gap"
            && contribution["value"] == "1px"
            && contribution["source"]["item_id"] == "dialogue.mobile"
            && contribution["active"] == true
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

fn assert_agent_mcp_rich_text_capture_responses(responses: &[serde_json::Value]) {
    assert_eq!(responses.len(), 12);
    assert_eq!(
        responses[0]["result"]["serverInfo"]["name"],
        "arcweft-agent"
    );
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "arcweft.observe")
    );
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "arcweft.capture")
    );
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "arcweft.session.info")
    );
    assert!(
        responses[2]["result"]["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|content| content["type"] == "resource_link"
                && content["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with("/objects.json")))
    );
    assert!(
        responses[3]["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/layer.dialogue.rich_text.png")))
    );
    assert!(
        responses[3]["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/object.object.dialogue.0.0.ruby.0.png")))
    );
    assert_mcp_png_capture_content(&responses[4], "ruby capture metadata is JSON");
    assert_mcp_png_capture_content(&responses[5], "native ruby capture metadata is JSON");
    assert_mcp_raw_capture_content(&responses[6]);
    assert_mcp_session_info_after_capture(&responses[7]);
    assert_raw_resource_read_content(&responses[8], &responses[6]);
    assert_png_resource_read_content(&responses[9]);
    assert_mcp_raw_object_id_capture_content(&responses[10]);
    assert_raw_object_id_resource_read_content(&responses[11], &responses[10]);
}

fn assert_mcp_session_info_after_capture(response: &serde_json::Value) {
    assert_eq!(response["result"]["content"][0]["type"], "text");
    let info = mcp_content_metadata(
        &response["result"]["content"][0],
        "session info content is JSON",
    );
    assert_eq!(info["observed"], true);
    assert_eq!(info["session_id"], "cli");
    assert_eq!(info["tick"], 0);
    assert!(info["resource_count"].as_u64().unwrap() > 0);
    assert!(info["latest_capture"]["content_pixels"].as_u64().unwrap() > 0);
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
    assert_eq!(info["latest_capture"]["crop_origin"]["space"], "viewport");
    assert_eq!(
        info["latest_capture_uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
    );
    assert_eq!(
        info["latest_capture_resource"]["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
    );
    assert_eq!(
        info["latest_capture_resource"]["mimeType"],
        "application/octet-stream"
    );
    assert!(
        info["latest_capture_resource"]["description"]
            .as_str()
            .is_some_and(|description| description.contains("kind=mask")
                && description.contains("scope=object:object.dialogue.0.0.ruby.0"))
    );
    assert_mcp_session_info_resource_templates(&info);
    assert!(info["layers"].as_array().unwrap().iter().any(|layer| {
        layer["id"] == "dialogue.rich_text"
            && layer["capture_refs"]["captures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|capture| {
                    capture["uri"]
                        .as_str()
                        .is_some_and(|uri| uri.ends_with("/layer.dialogue.rich_text.mask.rgba"))
                })
    }));
    assert!(info["objects"].as_array().unwrap().iter().any(|object| {
        object["id"] == "object.dialogue.0.0.ruby.0"
            && object["rich_text_ref"]["kind"] == "ruby"
            && object["capture_refs"]["captures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|capture| {
                    capture["uri"].as_str().is_some_and(|uri| {
                        uri.ends_with("/object.object.dialogue.0.0.ruby.0.mask.rgba")
                    })
                })
    }));
    assert!(
        info["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["name"] == "objects.json")
    );
    assert!(
        info["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["name"] == "presentation-tree.json")
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
    assert_eq!(response["result"]["content"][0]["type"], "text");
    let metadata = mcp_content_metadata(&response["result"]["content"][0], metadata_context);
    assert!(metadata["image"]["width"].as_u64().unwrap() > 0);
    assert!(metadata["image"]["height"].as_u64().unwrap() > 0);
    assert_eq!(metadata["image"]["kind"], "color");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        response["result"]["content"][1]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
    assert_eq!(response["result"]["content"][1]["mimeType"], "image/png");
}

fn assert_mcp_image_object_rich_text_ref(
    metadata: &serde_json::Value,
    object_id: &str,
    kind: &str,
) {
    assert_eq!(metadata["image"]["scope"]["kind"], "object");
    assert_eq!(metadata["image"]["scope"]["id"], object_id);
    assert_eq!(metadata["image"]["object"]["id"], object_id);
    assert_eq!(metadata["image"]["object"]["bbox"]["space"], "viewport");
    assert!(
        metadata["image"]["object"]["polygon"]
            .as_array()
            .is_some_and(|polygon| polygon.len() >= 4),
        "MCP image object metadata should preserve viewport polygon: {metadata}"
    );
    assert_agent_observe_object_capture_refs(&metadata["image"]["object"]);
    assert_eq!(metadata["image"]["object"]["rich_text_ref"]["kind"], kind);
    if !metadata["image"]["object"]["rich_text_ref"]["object_layer"].is_null() {
        assert_eq!(
            metadata["image"]["object"]["object_layer"],
            metadata["image"]["object"]["rich_text_ref"]["object_layer"]
        );
    }
    if !metadata["image"]["object"]["rich_text_ref"]["object_depth"].is_null() {
        assert_eq!(
            metadata["image"]["object"]["object_depth"],
            metadata["image"]["object"]["rich_text_ref"]["object_depth"]
        );
    }
    assert!(
        metadata["image"]["object"]["rich_text_ref"]["range"]["end"]
            .as_u64()
            .unwrap_or(0)
            > metadata["image"]["object"]["rich_text_ref"]["range"]["start"]
                .as_u64()
                .unwrap_or(0),
        "MCP image metadata should preserve the captured rich-text source range: {metadata}"
    );
}

fn assert_mcp_raw_capture_content(response: &serde_json::Value) {
    assert_eq!(response["result"]["content"][0]["type"], "text");
    let metadata = mcp_content_metadata(
        &response["result"]["content"][0],
        "raw capture metadata is JSON",
    );
    assert_eq!(
        metadata["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
    );
    assert_mcp_image_object_rich_text_ref(&metadata, "object.dialogue.0.0.ruby.0", "ruby");
    assert_eq!(metadata["image"]["pixel_format"], "rgba8_unorm");
    assert_eq!(
        metadata["image"]["row_stride_bytes"],
        metadata["image"]["width"].as_u64().unwrap() * 4
    );
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        response["result"]["content"][1]["resource"]["blob"]
            .as_str()
            .is_some_and(|blob| !blob.is_empty())
    );
    assert_eq!(
        response["result"]["content"][1]["resource"]["mimeType"],
        "application/octet-stream"
    );
}

fn mcp_raw_capture_bytes(response: &serde_json::Value) -> Vec<u8> {
    let blob = response["result"]["content"][1]["resource"]["blob"]
        .as_str()
        .expect("raw capture response has a resource blob");
    general_purpose::STANDARD
        .decode(blob)
        .expect("raw capture blob is base64")
}

fn assert_raw_resource_read_content(
    response: &serde_json::Value,
    source_capture_response: &serde_json::Value,
) {
    let metadata = mcp_content_metadata(
        &source_capture_response["result"]["content"][0],
        "raw source capture metadata is JSON",
    );
    let expected_len = metadata["image"]["row_stride_bytes"].as_u64().unwrap()
        * metadata["image"]["height"].as_u64().unwrap();
    let content = &response["result"]["contents"][0];
    assert_eq!(content["mimeType"], "application/octet-stream");
    assert_eq!(
        content["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
    );
    let blob = content["blob"].as_str().expect("raw resource has a blob");
    let bytes = general_purpose::STANDARD
        .decode(blob)
        .expect("raw resource blob is base64");
    assert_eq!(
        u64::try_from(bytes.len()).unwrap(),
        expected_len,
        "resources/read should return the latest raw capture bytes for this URI"
    );
    assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] > 0));
}

fn assert_mcp_raw_object_id_capture_content(response: &serde_json::Value) {
    assert_eq!(response["result"]["content"][0]["type"], "text");
    let metadata = mcp_content_metadata(
        &response["result"]["content"][0],
        "raw object-id capture metadata is JSON",
    );
    assert_eq!(
        metadata["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.object-id.rgba"
    );
    assert_eq!(metadata["image"]["kind"], "object_id");
    assert_eq!(metadata["image"]["composition"], "object_id_attachment");
    assert_mcp_image_object_rich_text_ref(&metadata, "object.dialogue.0.0.ruby.0", "ruby");
    assert_eq!(metadata["image"]["pixel_format"], "rgba8_unorm");
    assert_eq!(
        metadata["image"]["row_stride_bytes"],
        metadata["image"]["width"].as_u64().unwrap() * 4
    );
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        response["result"]["content"][1]["resource"]["blob"]
            .as_str()
            .is_some_and(|blob| !blob.is_empty())
    );
    assert_eq!(
        response["result"]["content"][1]["resource"]["mimeType"],
        "application/octet-stream"
    );
}

fn assert_raw_object_id_resource_read_content(
    response: &serde_json::Value,
    source_capture_response: &serde_json::Value,
) {
    let metadata = mcp_content_metadata(
        &source_capture_response["result"]["content"][0],
        "raw object-id source capture metadata is JSON",
    );
    let expected_len = metadata["image"]["row_stride_bytes"].as_u64().unwrap()
        * metadata["image"]["height"].as_u64().unwrap();
    let content = &response["result"]["contents"][0];
    assert_eq!(content["mimeType"], "application/octet-stream");
    assert_eq!(
        content["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.object-id.rgba"
    );
    let blob = content["blob"]
        .as_str()
        .expect("raw object-id resource has a blob");
    let bytes = general_purpose::STANDARD
        .decode(blob)
        .expect("raw object-id resource blob is base64");
    assert_eq!(
        u64::try_from(bytes.len()).unwrap(),
        expected_len,
        "resources/read should return the latest raw object-id capture bytes for this URI"
    );
    assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] > 0));
}

fn assert_png_resource_read_content(response: &serde_json::Value) {
    let content = &response["result"]["contents"][0];
    assert_eq!(content["mimeType"], "image/png");
    assert_eq!(
        content["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.png"
    );
    let blob = content["blob"].as_str().expect("PNG resource has a blob");
    let bytes = general_purpose::STANDARD
        .decode(blob)
        .expect("PNG resource blob is base64");
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "resources/read should keep earlier session capture bytes after later captures"
    );
}

fn mcp_content_metadata(block: &serde_json::Value, parse_message: &str) -> serde_json::Value {
    serde_json::from_str(block["text"].as_str().unwrap()).expect(parse_message)
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: native observe-before-capture coverage"]
fn agent_mcp_stdio_captures_profile_selected_source_without_prior_observe() {
    let dir = temp_dir("agent-mcp-profile-capture");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).expect("create profiled MCP capture source dir");
    fs::write(src_dir.join("main.arcw"), profiled_observe_source())
        .expect("write profiled MCP capture source");
    let manifest_path = dir.join("arcw.toml");
    fs::write(&manifest_path, profiled_observe_manifest())
        .expect("write profiled MCP capture manifest");

    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "manifest": manifest_path.display().to_string(),
                    "profile": "mobile",
                    "format": "png",
                    "layer": "dialogue",
                    "steps": 4,
                    "max_ops": 128
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.session.info",
                "arguments": {}
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_dir_all(&dir).expect("remove profiled MCP capture fixture");
    assert!(
        output.status.success(),
        "profiled agent mcp capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    assert_mcp_png_capture_content(&responses[1], "profiled MCP capture metadata is JSON");
    let capture = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "profiled MCP capture metadata is JSON",
    );
    assert_eq!(capture["image"]["renderer"], "native");
    assert_eq!(capture["image"]["scope"]["kind"], "layer");
    assert_eq!(capture["image"]["scope"]["id"], "dialogue");
    assert!(capture["image"]["content_pixels"].as_u64().unwrap() > 0);

    let session = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "profiled MCP capture session info is JSON",
    );
    assert_eq!(session["observed"], true);
    assert_eq!(session["source"], "main.arcw");
    assert_eq!(session["latest_capture"]["scope"]["kind"], "layer");
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_without_prior_observe() {
    let path = temp_arcw(
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
                    "format": "png",
                    "capture": "color",
                    "layer": "dialogue.rich_text",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list", "params": {}}),
    ];
    let output = run_agent_mcp_stdio(&requests);
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
            .is_some_and(|width| (130..220).contains(&width)),
        "direct rich-text layer capture width should come from observed native-layout child bboxes"
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
        "dialogue.rich_text"
    );
    assert_eq!(
        direct_capture_metadata["image"]["crop_origin"]["space"],
        "viewport"
    );
    assert!(
        direct_capture_metadata["image"]["crop_origin"]["x"]
            .as_u64()
            .unwrap()
            >= 120
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
            let is_layer_png = resource["uri"]
                .as_str()
                .and_then(|uri| uri.split('?').next())
                .is_some_and(|uri| uri.ends_with("/layer.dialogue.rich_text.png"));
            let is_native = resource["description"]
                .as_str()
                .is_some_and(|description| description.contains("renderer=native"));
            is_layer_png && is_native
        })
        .expect("direct capture should expose the selected layer image resource");
    assert!(
        layer_image["description"]
            .as_str()
            .is_some_and(|description| description.contains("kind=color")
                && description.contains("renderer=native")
                && description.contains("scope=layer:dialogue.rich_text")
                && description.contains("width=")
                && description.contains("height=")),
        "direct capture layer descriptor should expose image metadata"
    );
    assert!(
        resources.iter().any(|resource| resource["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/object.object.dialogue.0.0.ruby.0.png"))),
        "direct capture should populate latest observation resources"
    );
    assert!(
        resources.iter().any(|resource| resource["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/layer.dialogue.rich_text.mask.rgba"))),
        "direct capture should expose layer capture refs"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
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
                    "uri": "arcweft://session/cli/frame/0/color.png",
                    "max_privacy": "sensitive"
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
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
    assert!(
        metadata["image"]["content_bbox"]["x"].as_u64().unwrap() >= 96,
        "native MCP capture should align with the observed dialogue_view bbox"
    );
    assert_mcp_png_capture_content(&responses[2], "native capture resource metadata is JSON");
    let read_metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "native capture resource metadata is JSON",
    );
    assert_eq!(read_metadata["image"]["renderer"], "native");
    assert_eq!(read_metadata["image"]["scope"]["kind"], "viewport");
    assert_eq!(read_metadata["image"]["composition"], "framebuffer");
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_clear_after_page_object_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-page-object-capture",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
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
                    "format": "png",
                    "capture": "color",
                    "object": "object.dialogue.0.0.run.1",
                    "page": 1,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native page-object MCP source");
    assert!(
        output.status.success(),
        "agent mcp native page object capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    assert_mcp_png_capture_content(&responses[1], "native page object capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native page object capture metadata is JSON",
    );
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["page"], 1);
    assert_eq!(metadata["image"]["scope"]["kind"], "object");
    assert_eq!(
        metadata["image"]["scope"]["id"],
        "object.dialogue.0.0.run.1"
    );
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_reads_page_query_capture_ref_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-page-query-read",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let uri = "arcweft://session/cli/frame/0/object.object.dialogue.0.0.run.1.png?page=1";
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
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native page-query MCP source");
    assert!(
        output.status.success(),
        "agent mcp native page-query read should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    assert_mcp_png_capture_content(&responses[2], "native page-query read metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "native page-query read metadata is JSON",
    );
    assert_eq!(metadata["uri"], uri);
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["page"], 1);
    assert_eq!(metadata["image"]["scope"]["kind"], "object");
    assert_eq!(
        metadata["image"]["scope"]["id"],
        "object.dialogue.0.0.run.1"
    );
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_object_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-object-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
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
                    "format": "png",
                    "capture": "color",
                    "object": "object.dialogue.0.0",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
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
    assert_eq!(capture_width, 1088);
    assert_eq!(capture_height, 124);
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        metadata["image"]["content_pixels"].as_u64().unwrap() < capture_width * capture_height,
        "native object color capture should isolate glyph regions inside the dialogue_view crop"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_layer_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-layer-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
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
                    "format": "png",
                    "capture": "color",
                    "layer": "dialogue.rich_text",
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
                    "uri": "arcweft://session/cli/frame/0/layer.dialogue.rich_text.png",
                    "max_privacy": "sensitive"
                }
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "id": 4, "method": "resources/list", "params": {}}),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native MCP layer source");
    assert!(
        output.status.success(),
        "agent mcp native layer capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 4);
    assert_mcp_png_capture_content(&responses[1], "native layer capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native layer capture metadata is JSON",
    );
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(metadata["image"]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert!(metadata["image"]["width"].as_u64().unwrap() < 1088);
    assert!(metadata["image"]["height"].as_u64().unwrap() < 124);
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
    assert!(metadata["image"]["crop_origin"]["x"].as_u64().unwrap() >= 96);
    assert_mcp_png_capture_content(&responses[2], "native layer resource metadata is JSON");
    let read_metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "native layer resource metadata is JSON",
    );
    assert_eq!(read_metadata["image"]["renderer"], "native");
    assert_eq!(read_metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(read_metadata["image"]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(
        read_metadata["image"]["composition"],
        "masked_framebuffer_crop"
    );
    assert_native_layer_resource_descriptor(&responses[3]);
}

fn assert_native_layer_resource_descriptor(response: &serde_json::Value) {
    let resources = response["result"]["resources"].as_array().unwrap();
    let layer_image = resources
        .iter()
        .find(|resource| {
            let is_layer_png = resource["uri"]
                .as_str()
                .and_then(|uri| uri.split('?').next())
                .is_some_and(|uri| uri.ends_with("/layer.dialogue.rich_text.png"));
            let is_native = resource["description"]
                .as_str()
                .is_some_and(|description| description.contains("renderer=native"));
            is_layer_png && is_native
        })
        .expect("resources/list should expose the latest native layer capture");
    assert_eq!(layer_image["mimeType"], "image/png");
    assert!(
        layer_image["description"]
            .as_str()
            .is_some_and(|description| description.contains("kind=color")
                && description.contains("renderer=native")
                && description.contains("scope=layer:dialogue.rich_text")
                && description.contains("composition=masked_framebuffer_crop")
                && description.contains("width=")
                && description.contains("height=")),
        "native layer descriptor should expose latest capture metadata"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_reads_latest_native_layer_image_resource() {
    let path = temp_arcw(
        "agent-mcp-native-layer-read-resource",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
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
                    "image": "png",
                    "layer": "dialogue.rich_text",
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
                    "uri": "arcweft://session/cli/frame/0/layer.dialogue.rich_text.png",
                    "max_privacy": "sensitive"
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
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
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(metadata["image"]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(metadata["image"]["composition"], "masked_framebuffer_crop");
    assert!(metadata["image"]["width"].as_u64().unwrap() < 1088);
    assert!(metadata["image"]["height"].as_u64().unwrap() < 124);
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: animated image live resource readback"]
fn agent_mcp_stdio_reads_animated_image_layer_resource() {
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let layer_uri = "arcweft://session/cli/frame/0/layer.layer.foreground.rgba";
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
                    "flow": "image_sprite_overlay",
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
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp animated image layer resource read should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 4);
    assert_eq!(responses[1]["result"]["isError"], false);

    let metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "animated image MCP tool resource metadata is JSON",
    );
    assert_eq!(metadata["uri"], layer_uri);
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(metadata["image"]["scope"]["id"], "layer.foreground");
    assert_eq!(metadata["image"]["composition"], "framebuffer_crop");
    assert_eq!(metadata["image"]["capture_time_millis"], 150);
    assert_eq!(metadata["image"]["width"], 360);
    assert_eq!(metadata["image"]["height"], 180);
    assert_eq!(metadata["image"]["crop_origin"]["x"], 120);
    assert_eq!(metadata["image"]["crop_origin"]["y"], 84);
    assert_eq!(metadata["image"]["content_pixels"], 64_800);
    let tool_bytes = mcp_raw_capture_bytes(&responses[2]);
    assert_eq!(tool_bytes.len(), 360 * 180 * 4);
    assert_eq!(&tool_bytes[..4], &[5, 26, 161, 127]);

    let content = &responses[3]["result"]["contents"][0];
    assert_eq!(content["uri"], layer_uri);
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
    let path = temp_arcw(
        "agent-mcp-native-ruby-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
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
                    "format": "png",
                    "capture": "color",
                    "object": "object.dialogue.0.0.ruby.0",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
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
    assert!(
        metadata["image"]["width"].as_u64().unwrap() < 180,
        "native ruby element crop should be much narrower than the dialogue_view"
    );
    assert!(
        metadata["image"]["height"].as_u64().unwrap() < 120,
        "native ruby element crop should be much shorter than the dialogue_view"
    );
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
    assert!(
        metadata["image"]["crop_origin"]["x"].as_u64().unwrap() >= 96,
        "native ruby crop origin should map back to viewport coordinates"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_ruby_object_id_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-ruby-object-id",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
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
                    "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.object-id.png",
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
    assert_eq!(responses[1]["result"]["content"][0]["type"], "text");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native ruby object-id metadata is JSON",
    );
    assert_eq!(metadata["image"]["kind"], "object_id");
    assert_eq!(metadata["image"]["composition"], "object_id_attachment");
    assert_mcp_image_object_rich_text_ref(&metadata, "object.dialogue.0.0.ruby.0", "ruby");
    assert!(metadata["image"]["width"].as_u64().unwrap() < 180);
    assert!(metadata["image"]["height"].as_u64().unwrap() < 120);
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        responses[1]["result"]["content"][1]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_capture_time_controls_text_combine_mask_with_native_renderer() {
    for writing_mode in ["vertical_rl", "vertical_lr"] {
        let path = temp_arcw(
            &format!("agent-mcp-native-{writing_mode}-typewriter-text-combine-capture-time"),
            &format!(
                r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}][.typewriter cps=1]2026[/][/][p]
}}
"
            ),
        );
        let object_id = "object.dialogue.0.0.cluster.0.0.4";
        let requests = mcp_capture_time_requests(&path, "mask", object_id);
        let output = run_agent_mcp_stdio(&requests);
        fs::remove_file(&path).expect("remove temp native MCP text-combine source");
        assert!(
            output.status.success(),
            "agent mcp native {writing_mode} typewriter text-combine capture-time should succeed, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let responses = agent_mcp_responses(&output.stdout);
        assert_eq!(responses.len(), 3);

        let hidden = mcp_content_metadata(
            &responses[1]["result"]["content"][0],
            "hidden text-combine mask metadata is JSON",
        );
        let visible = mcp_content_metadata(
            &responses[2]["result"]["content"][0],
            "visible text-combine mask metadata is JSON",
        );
        assert_eq!(hidden["image"]["kind"], "mask");
        assert_eq!(visible["image"]["kind"], "mask");
        assert_eq!(hidden["image"]["composition"], "mask_attachment");
        assert_eq!(visible["image"]["composition"], "mask_attachment");
        assert_mcp_image_object_rich_text_ref(&hidden, object_id, "glyph_cluster");
        assert_mcp_image_object_rich_text_ref(&visible, object_id, "glyph_cluster");
        assert_eq!(
            hidden["image"]["crop_origin"],
            visible["image"]["crop_origin"]
        );
        assert_eq!(hidden["image"]["width"], visible["image"]["width"]);
        assert_eq!(hidden["image"]["height"], visible["image"]["height"]);
        assert_eq!(hidden["image"]["content_pixels"], 0);
        assert!(visible["image"]["content_pixels"].as_u64().unwrap() > 0);

        let hidden_bytes = mcp_raw_capture_bytes(&responses[1]);
        let visible_bytes = mcp_raw_capture_bytes(&responses[2]);
        assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
        assert_eq!(
            opaque_pixel_count(&visible_bytes) as u64,
            visible["image"]["content_pixels"].as_u64().unwrap()
        );
    }
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_capture_time_controls_text_combine_object_id_with_native_renderer() {
    for writing_mode in ["vertical_rl", "vertical_lr"] {
        let path = temp_arcw(
            &format!(
                "agent-mcp-native-{writing_mode}-typewriter-text-combine-object-id-capture-time"
            ),
            &format!(
                r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}][.typewriter cps=1]2026[/][/][p]
}}
"
            ),
        );
        let object_id = "object.dialogue.0.0.cluster.0.0.4";
        let requests = mcp_capture_time_requests(&path, "object-id", object_id);
        let output = run_agent_mcp_stdio(&requests);
        fs::remove_file(&path)
            .expect("remove temp native MCP text-combine object-id capture-time source");
        assert!(
            output.status.success(),
            "agent mcp native {writing_mode} typewriter text-combine object-id capture-time should succeed, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let responses = agent_mcp_responses(&output.stdout);
        assert_eq!(responses.len(), 3);

        let hidden = mcp_content_metadata(
            &responses[1]["result"]["content"][0],
            "hidden text-combine object-id metadata is JSON",
        );
        let visible = mcp_content_metadata(
            &responses[2]["result"]["content"][0],
            "visible text-combine object-id metadata is JSON",
        );
        assert_eq!(hidden["image"]["kind"], "object_id");
        assert_eq!(visible["image"]["kind"], "object_id");
        assert_eq!(hidden["image"]["composition"], "object_id_attachment");
        assert_eq!(visible["image"]["composition"], "object_id_attachment");
        assert_mcp_image_object_rich_text_ref(&hidden, object_id, "glyph_cluster");
        assert_mcp_image_object_rich_text_ref(&visible, object_id, "glyph_cluster");
        assert_eq!(
            hidden["image"]["crop_origin"],
            visible["image"]["crop_origin"]
        );
        assert_eq!(hidden["image"]["width"], visible["image"]["width"]);
        assert_eq!(hidden["image"]["height"], visible["image"]["height"]);
        assert_eq!(hidden["image"]["content_pixels"], 0);
        assert!(visible["image"]["content_pixels"].as_u64().unwrap() > 0);

        let hidden_bytes = mcp_raw_capture_bytes(&responses[1]);
        let visible_bytes = mcp_raw_capture_bytes(&responses[2]);
        assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
        assert_eq!(
            opaque_pixel_count(&visible_bytes) as u64,
            visible["image"]["content_pixels"].as_u64().unwrap()
        );
    }
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_capture_time_controls_ruby_object_id_with_native_renderer() {
    for writing_mode in ["vertical_rl", "vertical_lr"] {
        let path = temp_arcw(
            &format!("agent-mcp-native-{writing_mode}-typewriter-ruby-object-id-capture-time"),
            &format!(
                r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬[.typewriter cps=1]|[夢](ながいながいよみ)人外[/][/][p]
}}
"
            ),
        );
        let object_id = "object.dialogue.0.0.ruby.0";
        let requests = mcp_capture_time_requests(&path, "object-id", object_id);
        let output = run_agent_mcp_stdio(&requests);
        fs::remove_file(&path).expect("remove temp native MCP ruby object-id capture-time source");
        assert!(
            output.status.success(),
            "agent mcp native {writing_mode} typewriter ruby object-id capture-time should succeed, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let responses = agent_mcp_responses(&output.stdout);
        assert_eq!(responses.len(), 3);

        let hidden = mcp_content_metadata(
            &responses[1]["result"]["content"][0],
            "hidden ruby object-id metadata is JSON",
        );
        let visible = mcp_content_metadata(
            &responses[2]["result"]["content"][0],
            "visible ruby object-id metadata is JSON",
        );
        assert_eq!(hidden["image"]["kind"], "object_id");
        assert_eq!(visible["image"]["kind"], "object_id");
        assert_eq!(hidden["image"]["composition"], "object_id_attachment");
        assert_eq!(visible["image"]["composition"], "object_id_attachment");
        assert_mcp_image_object_rich_text_ref(&hidden, object_id, "ruby");
        assert_mcp_image_object_rich_text_ref(&visible, object_id, "ruby");
        assert_eq!(
            hidden["image"]["crop_origin"],
            visible["image"]["crop_origin"]
        );
        assert_eq!(hidden["image"]["width"], visible["image"]["width"]);
        assert_eq!(hidden["image"]["height"], visible["image"]["height"]);
        assert_eq!(hidden["image"]["content_pixels"], 0);
        assert!(visible["image"]["content_pixels"].as_u64().unwrap() > 0);

        let hidden_bytes = mcp_raw_capture_bytes(&responses[1]);
        let visible_bytes = mcp_raw_capture_bytes(&responses[2]);
        assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
        assert_eq!(
            opaque_pixel_count(&visible_bytes) as u64,
            visible["image"]["content_pixels"].as_u64().unwrap()
        );
    }
}

fn mcp_capture_time_requests(
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
                    "format": "raw-rgba",
                    "capture": capture_kind,
                    "object": object_id,
                    "capture_time": 0.0,
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
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
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

fn agent_mcp_rich_text_requests(path: &std::path::Path) -> Vec<serde_json::Value> {
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
                    "image": "png",
                    "layer": "dialogue.rich_text",
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
                    "object": "object.dialogue.0.0.ruby.0"
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
                    "object": "object.dialogue.0.0.ruby.0"
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
                    "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
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
    requests.extend(agent_mcp_rich_text_readback_requests());
    requests
}

fn agent_mcp_rich_text_readback_requests() -> [serde_json::Value; 4] {
    [
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "resources/read",
            "params": {
                "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba",
                "max_privacy": "sensitive"
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "resources/read",
            "params": {
                "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.png",
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
                    "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.object-id.rgba"
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "resources/read",
            "params": {
                "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.object-id.rgba",
                "max_privacy": "sensitive"
            }
        }),
    ]
}
