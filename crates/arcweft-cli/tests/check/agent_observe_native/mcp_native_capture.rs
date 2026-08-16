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

fn mcp_content_metadata(block: &serde_json::Value, parse_message: &str) -> serde_json::Value {
    serde_json::from_str(block["text"].as_str().unwrap()).expect(parse_message)
}
