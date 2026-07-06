#[test]
fn jit_check_json_compares_cranelift_and_vm() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("jit")
        .arg("check")
        .arg("--json")
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("2")
        .arg("--input-seed")
        .arg("7")
        .output()
        .expect("arcw jit check runs");

    assert!(
        output.status.success(),
        "jit check should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_jit_check_json(&stdout, "score", "builtin", &["base", "bonus"], 7);
}

#[test]
fn toolchain_profile_json_plans_path_free_workspace_commands() {
    let output = toolchain_profile_dry_run_output();

    assert!(
        output.status.success(),
        "toolchain profile dry-run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&std::env::current_dir().unwrap().display().to_string()),
        "toolchain profile JSON must not record absolute workspace paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("toolchain profile output is structured JSON");
    assert_eq!(json["status"], "ok");
    assert!(json["host_system"]["physical_cores"].as_u64().unwrap_or(0) > 0);
    assert!(json["host_system"]["logical_threads"].as_u64().unwrap_or(0) > 0);
    assert!(
        json["host_system"]["available_parallelism"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
    assert_eq!(json["commands"].as_array().unwrap().len(), 43);
    assert_eq!(json["commands"][0]["status"], "planned");
    assert_eq!(json["commands"][0]["repeat"], 2);
    assert_eq!(json["commands"][0]["warmup"], 1);
    assert_eq!(json["commands"][0]["elapsed_ns"], 0);
    assert_eq!(json["commands"][0]["timing"]["median"], 0);
    assert_eq!(
        json["commands"][0]["arcweft_bench"],
        serde_json::Value::Null
    );
    assert_eq!(
        json["commands"][0]["warmup_samples"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(json["commands"][0]["samples"].as_array().unwrap().len(), 2);
    assert_toolchain_profile_workspace_commands(&json);
    assert_toolchain_profile_bench_commands(&json);
    assert_toolchain_profile_math_commands(&json);
    assert_toolchain_profile_object_commands(&json);
    assert_toolchain_profile_width_commands(&json);
    assert_toolchain_profile_flow_math_commands(&json);
    assert_eq!(
        json["commands"][6]["arcweft_bench"],
        serde_json::Value::Null
    );
    assert_eq!(
        json["commands"][6]["samples"][0]["arcweft_bench"],
        serde_json::Value::Null
    );
    assert_eq!(json["commands"][8]["math_bench"], serde_json::Value::Null);
    assert_eq!(
        json["commands"][8]["samples"][0]["math_bench"],
        serde_json::Value::Null
    );
}

#[test]
fn agent_script_run_json_executes_cli_session_smoke() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/trace-{}.arcwx",
        std::process::id()
    ));
    let bundle_trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/bundle-trace-{}.arcwx",
        std::process::id()
    ));
    let bundle_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/agent-{}.awfb",
        std::process::id()
    ));
    let _ = fs::remove_file(&trace_path);
    let _ = fs::remove_file(&bundle_trace_path);
    let _ = fs::remove_file(&bundle_path);

    assert_agent_script_build(&bundle_path);
    assert_agent_script_bundle_run(&bundle_path, &bundle_trace_path);
    assert_agent_script_source_run_trace(&trace_path);
    assert_agent_script_trace_report(&trace_path);
    assert_agent_script_replay_matches(&trace_path, &bundle_trace_path);
    assert_agent_rag_query_trace(&trace_path);
    assert_agent_rag_query_trace_respects_privacy(&trace_path);
}

#[test]
fn agent_script_run_persists_debug_session_and_script_run() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/debug-db-trace-{}.arcwx",
        std::process::id()
    ));
    let second_trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/debug-db-trace-second-{}.arcwx",
        std::process::id()
    ));
    let debug_db_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/script-run-{}.sqlite3",
        std::process::id()
    ));
    let run_id = format!("run.debug.{}", std::process::id());
    let second_run_id = format!("run.debug.second.{}", std::process::id());
    let _ = fs::remove_file(&trace_path);
    let _ = fs::remove_file(&second_trace_path);
    let _ = fs::remove_file(&debug_db_path);
    let stale_session_id = seed_stale_script_session(&debug_db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_run_smoke_path())
        .arg("--json")
        .arg("--trace-out")
        .arg(&trace_path)
        .arg("--debug-db")
        .arg(&debug_db_path)
        .arg("--run-id")
        .arg(&run_id)
        .output()
        .expect("arcw agent script run persists debug DB");
    assert!(
        output.status.success(),
        "agent script debug DB run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("script run output is JSON");
    assert_eq!(report["ok"], true);
    assert_eq!(report["debug_db"], debug_db_path.display().to_string());
    let second_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_run_smoke_path())
        .arg("--json")
        .arg("--trace-out")
        .arg(&second_trace_path)
        .arg("--debug-db")
        .arg(&debug_db_path)
        .arg("--run-id")
        .arg(&second_run_id)
        .output()
        .expect("arcw agent script run persists second debug DB run");
    assert!(
        second_output.status.success(),
        "second agent script debug DB run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&second_output.stdout),
        String::from_utf8_lossy(&second_output.stderr)
    );

    let store = DebugStore::open(&debug_db_path).expect("open Agent Script debug database");
    let session_id = SessionId::new("session.cli").expect("session id");
    let session = store
        .session(&session_id)
        .expect("load script session")
        .expect("script session persisted");
    assert_eq!(session.status, DebugSessionStatus::Finished);
    assert_eq!(session.metadata["last_run_id"], second_run_id.as_str());
    assert_agent_script_project_metadata(&session.metadata);
    assert_stale_script_session_abandoned(&store, &stale_session_id);
    let expected_trace_path = trace_path.display().to_string();
    let persisted_run = store
        .script_run(&arcweft_agent_protocol::ids::AgentRunId::new(run_id).expect("run id"))
        .expect("load script run")
        .expect("script run persisted");
    let second_persisted_run = store
        .script_run(
            &arcweft_agent_protocol::ids::AgentRunId::new(&second_run_id).expect("second run id"),
        )
        .expect("load second script run")
        .expect("second script run persisted");
    assert_eq!(persisted_run.outcome, DebugScriptRunOutcome::Done);
    assert_eq!(second_persisted_run.outcome, DebugScriptRunOutcome::Done);
    assert_eq!(
        persisted_run.trace_uri.as_deref(),
        Some(expected_trace_path.as_str())
    );
    assert!(
        second_persisted_run.started_sequence > persisted_run.finished_sequence.unwrap(),
        "second run should be appended after the first run sequence range"
    );
    assert_eq!(persisted_run.metadata["steps"], report["steps"]);
    assert_agent_script_project_metadata(&persisted_run.metadata);
    assert_agent_script_project_metadata(&second_persisted_run.metadata);
    assert_eq!(store.stats().expect("stats").script_runs, 2);
    assert!(store.stats().expect("stats").debug_events > 0);

    assert_debug_db_runs_report(&debug_db_path, &second_run_id);
}

fn assert_agent_script_project_metadata(metadata: &BTreeMap<String, serde_json::Value>) {
    let entities = &metadata["project_entities"];
    assert_eq!(entities["count"], 0);
    assert_eq!(entities["kind_counts"].as_object().unwrap().len(), 0);

    let graph = &metadata["project_graph"];
    assert_eq!(graph["has_project_summary"], true);
    assert_eq!(graph["summary_symbol_id"], "project:summary");
    assert_eq!(graph["symbol_count"], 1);
    assert_eq!(graph["edge_count"], 0);
    assert_eq!(graph["symbol_kind_counts"]["project_summary"], 1);
    assert_eq!(graph["edge_kind_counts"].as_object().unwrap().len(), 0);
    assert_eq!(graph["project_summary"]["entity_count"], 0);
    assert_eq!(graph["project_summary"]["agent_action_count"], 0);
    assert_eq!(graph["project_summary"]["project_callable_count"], 0);
    assert_eq!(graph["project_summary"]["relation_count"], 0);
    assert_eq!(graph["project_summary"]["dependency_edge_count"], 0);
    assert_eq!(graph["project_summary"]["dynamic_control_flow_count"], 0);
    assert_eq!(graph["project_summary"]["debug_query_count"], 0);
}

fn seed_stale_script_session(debug_db_path: &Path) -> SessionId {
    let stale_session_id = SessionId::new("session.script.stale").expect("stale session id");
    let stale_started_unix_ms = current_unix_millis_for_test() - (2 * 86_400_000);
    DebugStore::open(debug_db_path)
        .expect("seed debug DB")
        .start_session(
            &stale_session_id,
            None,
            "script",
            "cli",
            stale_started_unix_ms,
        )
        .expect("seed stale running session");
    stale_session_id
}

fn assert_stale_script_session_abandoned(store: &DebugStore, stale_session_id: &SessionId) {
    let stale_session = store
        .session(stale_session_id)
        .expect("load stale script session")
        .expect("stale script session persists");
    assert_eq!(stale_session.status, DebugSessionStatus::Abandoned);
    assert_eq!(
        stale_session.metadata["lifecycle_policy"]["reason"],
        "runtime_session_start"
    );
    assert_eq!(
        stale_session.metadata["lifecycle_policy"]["operation"],
        "abandon_stale_running_sessions"
    );
}

fn assert_debug_db_runs_report(debug_db_path: &Path, second_run_id: &str) {
    let runs_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("runs")
        .arg("--path")
        .arg(debug_db_path)
        .arg("--session-id")
        .arg("session.cli")
        .arg("--json")
        .output()
        .expect("arcw debug db runs reads script runs");
    assert!(
        runs_output.status.success(),
        "debug db runs should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&runs_output.stdout),
        String::from_utf8_lossy(&runs_output.stderr)
    );
    let runs_report: serde_json::Value =
        serde_json::from_slice(&runs_output.stdout).expect("debug db runs output is JSON");
    assert_eq!(runs_report["max_privacy"], "project");
    let runs = runs_report["runs"].as_array().expect("runs array");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["run_id"], second_run_id);
    assert_eq!(runs[0]["outcome"], "done");
    assert_eq!(runs[0]["session_id"], "session.cli");
    assert_eq!(
        runs[0]["metadata"]["project_graph"]["has_project_summary"],
        true
    );
    assert_eq!(
        runs[0]["metadata"]["project_graph"]["summary_symbol_id"],
        "project:summary"
    );
    assert_eq!(runs[0]["project"]["entity_count"], 0);
    assert_eq!(runs[0]["project"]["graph_symbol_count"], 1);
    assert_eq!(runs[0]["project"]["graph_edge_count"], 0);
    assert_eq!(
        runs[0]["project"]["graph_summary_symbol_id"],
        "project:summary"
    );
    assert_eq!(
        runs[0]["project"]["project_summary"]["dynamic_control_flow_count"],
        0
    );
    assert!(
        runs[0]["started_sequence"].as_u64().expect("sequence")
            > runs[1]["finished_sequence"].as_u64().expect("sequence")
    );

    let public_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("runs")
        .arg("--path")
        .arg(debug_db_path)
        .arg("--session-id")
        .arg("session.cli")
        .arg("--max-privacy")
        .arg("public")
        .arg("--json")
        .output()
        .expect("arcw debug db runs reads public script runs");
    assert!(
        public_output.status.success(),
        "debug db runs public should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&public_output.stdout),
        String::from_utf8_lossy(&public_output.stderr)
    );
    let public_report: serde_json::Value =
        serde_json::from_slice(&public_output.stdout).expect("debug db runs public output is JSON");
    assert_eq!(public_report["max_privacy"], "public");
    let public_runs = public_report["runs"].as_array().expect("public runs array");
    assert_eq!(public_runs.len(), 2);
    assert_eq!(public_runs[0]["run_id"], second_run_id);
    assert!(public_runs[0]["project"].is_null());
    assert_eq!(
        public_runs[0]["metadata"]
            .as_object()
            .map(serde_json::Map::len),
        Some(0),
        "project-private lifecycle metadata should be omitted at public ceiling"
    );
}

#[test]
fn agent_script_run_trace_records_capture_blob_refs() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/capture-trace-{}.arcwx",
        std::process::id()
    ));
    let invalid_trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/capture-trace-missing-blob-{}.arcwx",
        std::process::id()
    ));
    let blob_dir = workspace_path(&format!(
        "target/codex-agent-script-run-test/capture-blobs-{}",
        std::process::id()
    ));
    let _ = fs::remove_file(&trace_path);
    let _ = fs::remove_file(&invalid_trace_path);
    let _ = fs::remove_dir_all(&blob_dir);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_capture_smoke_path())
        .arg("--json")
        .arg("--trace-out")
        .arg(&trace_path)
        .arg("--blob-dir")
        .arg(&blob_dir)
        .output()
        .expect("arcw agent script run writes capture trace");
    assert!(
        output.status.success(),
        "agent script capture run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let run_json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("capture run output is JSON");
    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["host_calls"], 1);
    assert_eq!(run_json["responses"][0]["kind"], "capture");
    assert_eq!(run_json["trace_records"], 5);
    assert_eq!(run_json["blobs_written"], 1);
    assert!(run_json["blob_bytes"].as_u64().expect("blob_bytes is u64") > 0);

    let trace: serde_json::Value = serde_json::from_slice(
        &fs::read(&trace_path).expect("agent script run writes capture .arcwx trace"),
    )
    .expect("capture trace is JSON");
    let capture = trace
        .as_array()
        .expect("trace is array")
        .iter()
        .find(|record| record["kind"] == "capture_stored")
        .expect("trace records capture event");
    let content_hash = capture["payload"]["content_hash"]
        .as_str()
        .expect("capture payload has content_hash");
    assert_eq!(capture["blob_refs"][0], content_hash);
    let blob_path = agent_test_blob_path(&blob_dir, content_hash);
    assert!(
        blob_path.exists(),
        "capture blob should be stored at {}",
        blob_path.display()
    );

    let trace_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("trace")
        .arg(&trace_path)
        .arg("--blob-dir")
        .arg(&blob_dir)
        .arg("--json")
        .output()
        .expect("arcw agent script trace validates capture blob refs");
    assert!(
        trace_output.status.success(),
        "agent script trace should validate capture blob refs\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&trace_output.stdout),
        String::from_utf8_lossy(&trace_output.stderr)
    );
    let trace_report: serde_json::Value =
        serde_json::from_slice(&trace_output.stdout).expect("capture trace report is JSON");
    assert_eq!(trace_report["ok"], true);
    assert_eq!(trace_report["blob_refs"], 1);
    assert_eq!(trace_report["blobs_validated"], 1);
    assert!(
        trace_report["blob_bytes"]
            .as_u64()
            .expect("blob_bytes is u64")
            > 0
    );
    assert_eq!(trace_report["kinds"]["capture_stored"], 1);

    write_capture_trace_without_blob_ref(&trace, &invalid_trace_path);
    assert_agent_script_trace_rejects_missing_capture_blob_ref(&invalid_trace_path);
}

#[test]
fn agent_script_run_json_executes_advance_text_smoke() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_advance_text_smoke_path())
        .arg("--json")
        .output()
        .expect("arcw agent script run executes advance_text smoke");

    assert!(
        output.status.success(),
        "agent script advance_text run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("advance_text run output is JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["host_calls"], 1);
    assert_eq!(json["responses"][0]["kind"], "action");
    assert_eq!(json["responses"][0]["response"]["accepted"], true);
}

#[test]
fn agent_script_run_json_executes_pointer_click_smoke() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_pointer_click_smoke_path())
        .arg("--json")
        .output()
        .expect("arcw agent script run executes pointer.click smoke");

    assert!(
        output.status.success(),
        "agent script pointer.click run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("pointer.click run output is JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["host_calls"], 2);
    assert_eq!(json["responses"][0]["kind"], "action");
    assert_eq!(json["responses"][0]["response"]["accepted"], true);
    assert_eq!(json["responses"][1]["kind"], "unit");
}

#[test]
fn agent_script_run_json_executes_read_resource_smoke() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/read-resource-trace-{}.arcwx",
        std::process::id()
    ));
    let _ = fs::remove_file(&trace_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_read_resource_smoke_path())
        .arg("--json")
        .arg("--trace-out")
        .arg(&trace_path)
        .output()
        .expect("arcw agent script run executes read_resource smoke");

    assert!(
        output.status.success(),
        "agent script read_resource run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("read_resource run output is JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["host_calls"], 2);
    assert_eq!(json["responses"][0]["kind"], "resource");
    assert_eq!(
        json["responses"][0]["response"]["uri"],
        "arcweft://session/cli/observation/latest.json"
    );
    assert_eq!(
        json["responses"][0]["response"]["kind"],
        "observation_latest"
    );
    assert_eq!(json["responses"][1]["kind"], "unit");
    assert_eq!(json["trace_records"], 8);
    let trace: serde_json::Value = serde_json::from_slice(
        &fs::read(&trace_path).expect("agent script run writes read_resource .arcwx trace"),
    )
    .expect("read_resource trace is JSON");
    let resource_record = trace
        .as_array()
        .expect("trace is array")
        .iter()
        .find(|record| record["kind"] == "resource_read_completed")
        .expect("trace records resource read event");
    assert_eq!(
        resource_record["payload"]["uri"],
        "arcweft://session/cli/observation/latest.json"
    );
    assert_eq!(
        json["final_status"],
        "Done(Return(\"{\\\"source\\\":\\\"arcw agent script run\\\",\\\"uri\\\":\\\"arcweft://session/cli/observation/latest.json\\\"}\"))"
    );
}

#[test]
fn agent_script_run_executes_read_resource_metadata_projection_smoke() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_read_resource_metadata_smoke_path())
        .arg("--json")
        .output()
        .expect("arcw agent script run executes read_resource metadata smoke");

    assert!(
        output.status.success(),
        "agent script read_resource metadata run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("read_resource metadata output is JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["responses"][0]["kind"], "resource");
    assert_eq!(
        json["responses"][0]["response"]["uri"],
        "arcweft://session/cli/observation/latest.json"
    );
    assert_eq!(
        json["responses"][0]["response"]["kind"],
        "observation_latest"
    );
    assert_eq!(
        json["responses"][0]["response"]["mime_type"],
        "application/json"
    );
    assert_eq!(json["responses"][0]["response"]["hash"], "cli-resource");
    assert!(
        json["final_status"]
            .as_str()
            .is_some_and(|status| status.contains("cli-resource")),
        "final status should return projected resource hash: {json}"
    );
}

#[test]
fn agent_script_run_persists_attach_resource_debug_record() {
    let debug_db_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/attach-resource-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&debug_db_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_attach_resource_smoke_path())
        .arg("--json")
        .arg("--debug-db")
        .arg(&debug_db_path)
        .output()
        .expect("arcw agent script run persists attach debug record");

    assert!(
        output.status.success(),
        "agent script attach resource run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("attach resource run output is JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["host_calls"], 3);
    assert_eq!(json["responses"][0]["kind"], "resource");
    assert_eq!(json["responses"][1]["kind"], "unit");
    assert_eq!(json["responses"][2]["kind"], "unit");

    let timeline_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("timeline")
        .arg("--path")
        .arg(&debug_db_path)
        .arg("--session-id")
        .arg("session.cli")
        .arg("--run-id")
        .arg("run.cli")
        .arg("--limit")
        .arg("32")
        .arg("--json")
        .output()
        .expect("arcw debug db timeline reads attach debug record");
    assert!(
        timeline_output.status.success(),
        "debug db timeline should expose attach record\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&timeline_output.stdout),
        String::from_utf8_lossy(&timeline_output.stderr)
    );
    let timeline: serde_json::Value =
        serde_json::from_slice(&timeline_output.stdout).expect("timeline output is JSON");
    let events = timeline["events"]
        .as_array()
        .expect("timeline events array");
    assert!(
        events.iter().any(|event| {
            event["kind"] == "diagnostic"
                && event["payload"]["attachment"]["uri"]
                    == "arcweft://session/cli/observation/latest.json"
                && event["payload"]["attachment"]["kind"] == "observation_latest"
        }),
        "timeline should contain attached AgentResource payload: {timeline}"
    );
    assert!(
        events.iter().any(|event| {
            event["kind"] == "diagnostic"
                && event["payload"]["checkpoint"] == "after-attach-resource"
        }),
        "timeline should contain checkpoint after attach: {timeline}"
    );
}

#[test]
fn agent_script_run_persists_attach_capture_debug_record() {
    let debug_db_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/attach-capture-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&debug_db_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_attach_capture_smoke_path())
        .arg("--json")
        .arg("--debug-db")
        .arg(&debug_db_path)
        .output()
        .expect("arcw agent script run persists attached capture debug record");

    assert!(
        output.status.success(),
        "agent script attach capture run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("attach capture run output is JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["host_calls"], 3);
    assert_eq!(json["responses"][0]["kind"], "capture");
    assert_eq!(json["responses"][1]["kind"], "unit");
    assert_eq!(json["responses"][2]["kind"], "unit");

    let timeline_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("timeline")
        .arg("--path")
        .arg(&debug_db_path)
        .arg("--session-id")
        .arg("session.cli")
        .arg("--run-id")
        .arg("run.cli")
        .arg("--limit")
        .arg("32")
        .arg("--json")
        .output()
        .expect("arcw debug db timeline reads attached capture debug record");
    assert!(
        timeline_output.status.success(),
        "debug db timeline should expose attached capture record\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&timeline_output.stdout),
        String::from_utf8_lossy(&timeline_output.stderr)
    );
    let timeline: serde_json::Value =
        serde_json::from_slice(&timeline_output.stdout).expect("timeline output is JSON");
    let events = timeline["events"]
        .as_array()
        .expect("timeline events array");
    let capture = events
        .iter()
        .find(|event| event["kind"] == "capture")
        .expect("timeline records capture event");
    let capture_uri = capture["payload"]["uri"]
        .as_str()
        .expect("capture payload has uri");
    assert!(
        events.iter().any(|event| {
            event["kind"] == "diagnostic"
                && event["payload"]["attachment"]["uri"] == capture_uri
                && event["payload"]["attachment"]["media_type"] == "image/png"
                && event["payload"]["attachment"]["byte_len"]
                    .as_u64()
                    .is_some_and(|bytes| bytes > 0)
        }),
        "timeline should contain attached CaptureRef payload: {timeline}"
    );
    assert!(
        events.iter().any(|event| {
            event["kind"] == "diagnostic"
                && event["payload"]["checkpoint"] == "after-attach-capture"
        }),
        "timeline should contain checkpoint after capture attach: {timeline}"
    );
}

fn agent_test_blob_path(root: &Path, content_hash: &str) -> PathBuf {
    let hex = content_hash
        .strip_prefix("blake3:")
        .expect("test capture hash is blake3");
    root.join("blake3").join(hex)
}

fn write_capture_trace_without_blob_ref(trace: &serde_json::Value, path: &Path) {
    let mut invalid_trace = trace.clone();
    invalid_trace
        .as_array_mut()
        .expect("invalid trace is array")
        .iter_mut()
        .find(|record| record["kind"] == "capture_stored")
        .expect("invalid trace records capture event")["blob_refs"] = serde_json::json!([]);
    fs::write(
        path,
        serde_json::to_vec_pretty(&invalid_trace).expect("invalid trace serializes"),
    )
    .expect("writes invalid capture trace");
}

fn assert_agent_script_trace_rejects_missing_capture_blob_ref(path: &Path) {
    let invalid_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("trace")
        .arg(path)
        .arg("--json")
        .output()
        .expect("arcw agent script trace rejects missing capture blob ref");
    assert!(
        !invalid_output.status.success(),
        "trace validation should reject missing capture blob ref\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&invalid_output.stdout),
        String::from_utf8_lossy(&invalid_output.stderr)
    );
    let invalid_report: serde_json::Value =
        serde_json::from_slice(&invalid_output.stdout).expect("invalid trace report is JSON");
    assert_eq!(invalid_report["ok"], false);
    assert!(
        invalid_report["error"]
            .as_str()
            .expect("invalid trace report includes error")
            .contains("capture blob_refs does not include content_hash")
    );
}

#[test]
#[ignore = "tier 2 native Agent Script E2E: requires native-capture feature subprocess"]
fn agent_script_run_native_source_captures_native_resource() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/native-capture-trace-{}.arcwx",
        std::process::id()
    ));
    let blob_dir = workspace_path(&format!(
        "target/codex-agent-script-run-test/native-capture-blobs-{}",
        std::process::id()
    ));
    let _ = fs::remove_file(&trace_path);
    let _ = fs::remove_dir_all(&blob_dir);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_capture_smoke_path())
        .arg("--native-source")
        .arg(rich_text_showcase_path())
        .arg("--json")
        .arg("--trace-out")
        .arg(&trace_path)
        .arg("--blob-dir")
        .arg(&blob_dir)
        .output()
        .expect("arcw agent script run captures native source");
    assert!(
        output.status.success(),
        "native agent script capture should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let run_json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native run output is JSON");
    let capture = &run_json["responses"][0]["response"];
    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["responses"][0]["kind"], "capture");
    assert!(
        capture["uri"]
            .as_str()
            .expect("native capture uri")
            .starts_with("arcweft://session/cli/frame/0/")
    );
    assert_eq!(capture["media_type"], "image/png");
    assert!(capture["byte_len"].as_u64().unwrap_or(0) > 0);
    assert_ne!(capture["content_hash"], "cli-capture-0000000000000001");
    assert_eq!(run_json["blobs_written"], 1);
    let content_hash = capture["content_hash"].as_str().expect("content hash");
    let blob_path = agent_test_blob_path(&blob_dir, content_hash);
    assert!(
        blob_path.exists(),
        "native capture blob should be stored at {}",
        blob_path.display()
    );

    let trace: serde_json::Value = serde_json::from_slice(
        &fs::read(&trace_path).expect("native agent script run writes .arcwx trace"),
    )
    .expect("native trace is JSON");
    let trace_capture = trace
        .as_array()
        .expect("trace is array")
        .iter()
        .find(|record| record["kind"] == "capture_stored")
        .expect("trace records native capture event");
    assert_eq!(trace_capture["blob_refs"][0], capture["content_hash"]);

    let trace_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("trace")
        .arg(&trace_path)
        .arg("--blob-dir")
        .arg(&blob_dir)
        .arg("--json")
        .output()
        .expect("arcw agent script trace validates native capture blob bytes");
    assert!(
        trace_output.status.success(),
        "native trace blob validation should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&trace_output.stdout),
        String::from_utf8_lossy(&trace_output.stderr)
    );
    let trace_report: serde_json::Value =
        serde_json::from_slice(&trace_output.stdout).expect("native trace report is JSON");
    assert_eq!(trace_report["blobs_validated"], 1);
}

#[test]
#[ignore = "tier 2 native Agent Script E2E: requires native-capture feature subprocess"]
fn agent_script_run_native_source_resolves_project_entities() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/native-flow-wait-trace-{}.arcwx",
        std::process::id()
    ));
    let _ = fs::remove_file(&trace_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_native_flow_wait_smoke_path())
        .arg("--native-source")
        .arg(agent_script_native_project_index_path())
        .arg("--json")
        .arg("--trace-out")
        .arg(&trace_path)
        .output()
        .expect("arcw agent script run resolves native source entities");
    assert!(
        output.status.success(),
        "native agent script project entity run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let run_json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native entity run output is JSON");

    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["responses"][0]["kind"], "observation");
    assert_eq!(
        run_json["responses"][0]["response"]["signals"]["signal.current_flow"]["kind"],
        "entity"
    );
    assert_eq!(
        run_json["responses"][0]["response"]["signals"]["signal.current_flow"]["value"],
        "flow.opening"
    );
    assert!(
        trace_path.exists(),
        "native entity run should write trace at {}",
        trace_path.display()
    );
}

#[test]
#[ignore = "tier 2 native Agent Script E2E: requires native-capture feature subprocess"]
fn agent_script_run_native_source_dispatches_semantic_choice_action() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/native-choice-dispatch-trace-{}.arcwx",
        std::process::id()
    ));
    let _ = fs::remove_file(&trace_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_native_choice_dispatch_path())
        .arg("--native-source")
        .arg(agent_script_native_choice_dispatch_source_path())
        .arg("--json")
        .arg("--trace-out")
        .arg(&trace_path)
        .output()
        .expect("arcw agent script run dispatches native semantic choice action");
    assert!(
        output.status.success(),
        "native agent script semantic choice dispatch should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let run_json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native choice dispatch output is JSON");

    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["responses"][0]["kind"], "observation");
    assert_eq!(run_json["responses"][1]["kind"], "action");
    assert_eq!(run_json["responses"][1]["response"]["accepted"], true);
    assert_eq!(run_json["responses"][2]["kind"], "observation");
    assert_eq!(
        run_json["responses"][2]["response"]["signals"]["signal.current_flow"]["value"],
        "flow.alice_intro"
    );
    assert!(
        trace_path.exists(),
        "native semantic choice dispatch should write trace at {}",
        trace_path.display()
    );
}

#[test]
#[ignore = "tier 2 native Agent Script E2E: requires native-capture feature subprocess"]
fn agent_script_run_native_source_dispatches_advance_text_in_game_mode() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/native-advance-text-game-trace-{}.arcwx",
        std::process::id()
    ));
    let _ = fs::remove_file(&trace_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_native_advance_text_game_path())
        .arg("--native-source")
        .arg(rich_text_showcase_path())
        .arg("--native-mode")
        .arg("game")
        .arg("--native-steps")
        .arg("1")
        .arg("--json")
        .arg("--trace-out")
        .arg(&trace_path)
        .output()
        .expect("arcw agent script run dispatches native advance_text in game mode");
    assert!(
        output.status.success(),
        "native agent script advance_text game-mode dispatch should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let run_json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native advance_text output is JSON");

    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["responses"][0]["kind"], "observation");
    assert_eq!(run_json["responses"][0]["response"]["tick"], 0);
    assert_eq!(run_json["responses"][1]["kind"], "action");
    assert_eq!(run_json["responses"][1]["response"]["accepted"], true);
    assert_eq!(run_json["responses"][1]["response"]["before_tick"], 0);
    assert_eq!(run_json["responses"][1]["response"]["after_tick"], 1);
    assert!(
        trace_path.exists(),
        "native advance_text game-mode dispatch should write trace at {}",
        trace_path.display()
    );
}

#[test]
#[ignore = "tier 2 native Agent Script E2E: requires native-capture feature subprocess"]
fn agent_script_run_native_source_dispatches_semantic_invoke_action() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/native-invoke-action-trace-{}.arcwx",
        std::process::id()
    ));
    let _ = fs::remove_file(&trace_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_native_invoke_action_path())
        .arg("--native-source")
        .arg(image_animation_sample_path())
        .arg("--flow")
        .arg("image_clipped_object")
        .arg("--json")
        .arg("--trace-out")
        .arg(&trace_path)
        .output()
        .expect("arcw agent script run dispatches native semantic invoke action");
    assert!(
        output.status.success(),
        "native agent script semantic invoke dispatch should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let run_json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native invoke dispatch output is JSON");

    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["responses"][0]["kind"], "observation");
    assert_eq!(run_json["responses"][1]["kind"], "action");
    assert_eq!(run_json["responses"][1]["response"]["accepted"], true);
    assert!(
        trace_path.exists(),
        "native semantic invoke dispatch should write trace at {}",
        trace_path.display()
    );
}

#[test]
#[ignore = "tier 2 native Agent REPL E2E: requires native-capture feature subprocess"]
fn agent_repl_observes_and_lists_actions_from_input_session() {
    let debug_db_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/repl-cells-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&debug_db_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("repl")
        .arg(rich_text_showcase_path())
        .arg("--input")
        .arg(agent_repl_smoke_path())
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("64")
        .arg("--debug-db")
        .arg(&debug_db_path)
        .arg("--json")
        .output()
        .expect("arcw agent repl runs deterministic input session");
    assert!(
        output.status.success(),
        "agent repl deterministic input session should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent repl output is JSON");

    assert_eq!(report["ok"], true);
    assert_eq!(report["final_tick"], 0);
    assert_eq!(report["debug_db"], debug_db_path.display().to_string());
    assert_eq!(report["persisted_cells"], 1);
    let cells = report["cells"].as_array().expect("cells are present");
    assert!(cells.iter().any(|cell| cell["input"] == ":observe"));
    let actions = cells
        .iter()
        .find(|cell| cell["input"] == ":actions")
        .expect("actions cell is present");
    assert_eq!(actions["status"], "ok");
    assert!(
        actions["value"]["actions"]
            .as_array()
            .is_some_and(|actions| !actions.is_empty()),
        "actions cell should expose observed semantic action targets: {actions}"
    );
    let query = cells
        .iter()
        .find(|cell| cell["input"] == ":query opening")
        .expect("query cell is present");
    assert_eq!(query["status"], "ok");
    assert_eq!(query["value"]["query"]["text"], "opening");
    assert!(
        query["value"]["items"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "query cell should expose RAG context items: {query}"
    );
    let bindings = cells
        .iter()
        .find(|cell| cell["input"] == ":bindings")
        .expect("bindings cell is present");
    assert_eq!(bindings["status"], "ok");
    let cell = cells
        .iter()
        .find(|cell| cell["input"] == "let observed = observe()")
        .expect("compiled cell is present");
    assert_eq!(cell["status"], "ok");
    assert_eq!(cell["kind"], "cell");
    assert_eq!(cell["value"]["compiled"], true);
    assert_eq!(cell["value"]["host_calls"], 1);
    assert_eq!(cell["value"]["persisted"], true);
    assert!(
        cell["value"]["bindings"]
            .as_array()
            .is_some_and(|bindings| bindings.iter().any(|binding| binding == "observed")),
        "compiled cell should expose VM-local binding names: {cell}"
    );
    let binding_list = bindings["value"]["bindings"]
        .as_array()
        .expect("bindings are listed");
    assert_agent_repl_observe_bindings(binding_list, bindings);
    let history = cells
        .iter()
        .find(|cell| cell["input"] == ":history")
        .expect("history cell is present");
    assert_eq!(history["status"], "ok");
    assert!(
        history["value"]["cells"]
            .as_array()
            .is_some_and(|history| history.iter().any(|cell| cell["input"] == ":query opening")),
        "history cell should include previous query cell: {history}"
    );

    assert_repl_debug_cell_persisted(&debug_db_path);
}

#[test]
#[ignore = "tier 2 native Agent REPL E2E: requires native-capture feature subprocess"]
fn agent_repl_reuses_serialized_live_bindings_between_cells() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("repl")
        .arg("--input")
        .arg(agent_repl_live_binding_smoke_path())
        .arg("--json")
        .output()
        .expect("arcw agent repl runs live binding input session");
    assert!(
        output.status.success(),
        "agent repl live binding session should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent repl output is JSON");

    assert_eq!(report["ok"], true);
    let cells = report["cells"].as_array().expect("cells are present");
    assert_agent_repl_live_binding_cells(cells);

    let bindings = cells
        .iter()
        .find(|cell| cell["input"] == ":bindings")
        .expect("bindings cell is present");
    let binding_list = bindings["value"]["bindings"]
        .as_array()
        .expect("bindings are listed");
    assert_agent_repl_live_bindings(binding_list);
}

fn assert_agent_repl_live_binding_cells(cells: &[serde_json::Value]) {
    assert_agent_repl_cell_status(cells, "let answer = 42i64", false);
    assert_agent_repl_cell_status(cells, "return answer", true);
    assert_agent_repl_cell_status(cells, "let frame = observe()", false);
    assert_agent_repl_cell_status(cells, "return frame.tick", true);
    assert_agent_repl_cell_status(
        cells,
        "let resource = read_resource(\"arcweft://session/cli/observation/latest.json\")",
        false,
    );
    assert_agent_repl_cell_status(cells, "return resource.uri", true);
    assert_agent_repl_cell_status(cells, "let context = try rag.query(\"opening\")", false);
    assert_agent_repl_cell_status(cells, "return context.summary()", true);
}

fn assert_agent_repl_cell_status(
    cells: &[serde_json::Value],
    input: &str,
    expect_compiled_value: bool,
) {
    let cell = cells
        .iter()
        .find(|cell| cell["input"] == input)
        .unwrap_or_else(|| panic!("REPL cell `{input}` is present"));
    assert_eq!(cell["status"], "ok");
    if expect_compiled_value {
        assert_eq!(cell["value"]["compiled"], true);
    }
}

fn assert_agent_repl_live_bindings(bindings: &[serde_json::Value]) {
    assert_agent_repl_live_binding(bindings, "answer", "42i64", "literal");
    assert_agent_repl_live_binding(bindings, "frame", "observe()", "observation");
    assert_agent_repl_live_binding(
        bindings,
        "resource",
        "read_resource(\"arcweft://session/cli/observation/latest.json\")",
        "resource",
    );
    assert_agent_repl_live_binding(
        bindings,
        "context",
        "try rag.query(\"opening\")",
        "rag_context",
    );
}

fn assert_agent_repl_live_binding(
    bindings: &[serde_json::Value],
    name: &str,
    serialized_source: &str,
    snapshot_kind: &str,
) {
    let binding = bindings
        .iter()
        .find(|binding| binding["name"] == name)
        .unwrap_or_else(|| panic!("REPL binding `{name}` is present"));
    assert_eq!(binding["binding_kind"], "local");
    assert_eq!(binding["serializable"], true);
    assert_eq!(binding["serialized_source"], serialized_source);
    assert_eq!(binding["snapshot_kind"], snapshot_kind);
}

#[test]
#[ignore = "tier 2 native Agent REPL E2E: requires native-capture feature subprocess"]
fn agent_repl_inspects_fragments_and_captures_from_input_session() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("repl")
        .arg(rich_text_showcase_path())
        .arg("--input")
        .arg(agent_repl_inspection_smoke_path())
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw agent repl runs inspection input session");
    assert!(
        output.status.success(),
        "agent repl inspection session should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent repl output is JSON");

    assert_eq!(report["ok"], true);
    let cells = report["cells"].as_array().expect("cells are present");
    assert_agent_repl_meta_ok(cells, ":type 1u32");
    let classify = cells
        .iter()
        .find(|cell| cell["input"] == ":classify let frame = try observe()")
        .expect("classify cell is present");
    assert_eq!(classify["status"], "ok");
    assert_eq!(classify["value"]["completion"]["kind"], "complete");
    assert_eq!(classify["value"]["fragment_kind"], "statements");
    let incomplete = cells
        .iter()
        .find(|cell| cell["input"] == ":classify note(\"unterminated")
        .expect("incomplete classify cell is present");
    assert_eq!(incomplete["status"], "ok");
    assert_eq!(incomplete["value"]["completion"]["kind"], "incomplete");
    assert_eq!(
        incomplete["value"]["completion"]["expected"],
        serde_json::json!(["\""])
    );
    let incomplete_expr = cells
        .iter()
        .find(|cell| cell["input"] == ":classify let value =")
        .expect("incomplete expression classify cell is present");
    assert_eq!(incomplete_expr["status"], "ok");
    assert_eq!(incomplete_expr["value"]["completion"]["kind"], "incomplete");
    assert_eq!(
        incomplete_expr["value"]["completion"]["expected"],
        serde_json::json!(["expression"])
    );
    let state_completion = cells
        .iter()
        .find(|cell| cell["input"] == ":complete state_")
        .expect("state_path completion cell is present");
    assert_eq!(state_completion["status"], "ok");
    assert!(
        state_completion["value"]["items"]
            .as_array()
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["label"] == "state_path" && item["kind"] == "prelude_function")
            }),
        "completion should expose state_path: {state_completion}"
    );
    let read_resource_completion = cells
        .iter()
        .find(|cell| cell["input"] == ":complete read_resource(")
        .expect("read_resource completion cell is present");
    assert_eq!(read_resource_completion["status"], "ok");
    assert!(
        read_resource_completion["value"]["items"]
            .as_array()
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item["label"] == "uri"
                        && item["kind"] == "named_parameter"
                        && item["insert_text"] == "uri = "
                })
            }),
        "completion should expose read_resource uri parameter: {read_resource_completion}"
    );
    assert_agent_repl_meta_ok(cells, ":ast signal(\"ready\").eq(true)");
    assert_agent_repl_meta_ok(cells, ":hir return \"ok\"");
    assert_agent_repl_meta_ok(cells, ":bytecode return \"ok\"");
    let capture = cells
        .iter()
        .find(|cell| cell["input"] == ":capture viewport")
        .expect("capture cell is present");
    assert_eq!(capture["status"], "ok");
    assert!(
        capture["value"]["images"]
            .as_array()
            .is_some_and(|images| !images.is_empty()),
        "capture cell should expose image resources: {capture}"
    );
}

#[test]
#[ignore = "tier 2 native Agent REPL E2E: requires native-capture feature subprocess"]
fn agent_repl_connects_source_from_input_session() {
    let input_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/repl-connect-{}.txt",
        std::process::id()
    ));
    fs::create_dir_all(input_path.parent().expect("input target dir"))
        .expect("create REPL connect input target dir");
    let _ = fs::remove_file(&input_path);
    fs::write(
        &input_path,
        ":help\n:observe\n:actions\n:complete :capture layer\n:highlight let frame = try observe(@flow.opening)\n:capture viewport\n:quit\n",
    )
    .expect("write REPL connect input");

    let connect_target = format!("source {}", rich_text_showcase_path().display());
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("repl")
        .arg("--connect")
        .arg(&connect_target)
        .arg("--input")
        .arg(&input_path)
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw agent repl connects from input session");
    assert!(
        output.status.success(),
        "agent repl connect session should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent repl output is JSON");

    assert_eq!(report["ok"], true);
    assert_eq!(report["final_tick"], 0);
    assert_eq!(report["connection"]["kind"], "source");
    assert_eq!(
        report["connection"]["path"],
        rich_text_showcase_path().display().to_string()
    );
    let cells = report["cells"].as_array().expect("cells are present");
    assert_agent_repl_meta_ok(cells, ":observe");
    assert_agent_repl_meta_ok(cells, ":actions");
    let complete = cells
        .iter()
        .find(|cell| cell["input"] == ":complete :capture layer")
        .expect("completion cell is present");
    assert_eq!(complete["status"], "ok");
    assert!(
        complete["value"]["items"].as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item["kind"] == "layer_id" && item["label"] == "dialogue.rich_text")
        }),
        "completion cell should expose observed layer ids: {complete}"
    );
    let highlight = cells
        .iter()
        .find(|cell| cell["input"] == ":highlight let frame = try observe(@flow.opening)")
        .expect("highlight cell is present");
    assert_eq!(highlight["status"], "ok");
    assert!(
        highlight["value"]["tokens"]
            .as_array()
            .is_some_and(|tokens| {
                tokens
                    .iter()
                    .any(|token| token["kind"] == "entity_id" && token["text"] == "@flow.opening")
            }),
        "highlight cell should expose structured entity-id tokens: {highlight}"
    );
    let capture = cells
        .iter()
        .find(|cell| cell["input"] == ":capture viewport")
        .expect("capture cell is present");
    assert_eq!(capture["status"], "ok");
    assert!(
        capture["value"]["images"]
            .as_array()
            .is_some_and(|images| !images.is_empty()),
        "connected capture should expose image resources: {capture}"
    );
}

#[test]
#[ignore = "tier 2 native Agent REPL E2E: requires native-capture feature subprocess"]
fn agent_repl_reads_trace_in_read_only_input_session() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/repl-readonly-trace-{}.arcwx",
        std::process::id()
    ));
    fs::create_dir_all(trace_path.parent().expect("trace target dir"))
        .expect("create REPL trace target dir");
    let _ = fs::remove_file(&trace_path);

    let run = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_run_smoke_path())
        .arg("--json")
        .arg("--trace-out")
        .arg(&trace_path)
        .output()
        .expect("arcw agent script run writes trace for read-only REPL");
    assert!(
        run.status.success(),
        "agent script run should write read-only REPL trace\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("repl")
        .arg("--trace")
        .arg(&trace_path)
        .arg("--read-only")
        .arg("--input")
        .arg(agent_repl_trace_readonly_smoke_path())
        .arg("--json")
        .output()
        .expect("arcw agent repl reads trace in read-only input session");
    assert!(
        output.status.success(),
        "agent repl read-only trace session should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent repl output is JSON");

    assert_eq!(report["ok"], true);
    assert_eq!(report["read_only"], true);
    assert_eq!(report["trace_path"], trace_path.display().to_string());
    assert!(
        report["trace_records"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "REPL report should expose loaded trace record count: {report}"
    );
    let cells = report["cells"].as_array().expect("cells are present");
    let trace = cells
        .iter()
        .find(|cell| cell["input"] == ":trace")
        .expect("trace cell is present");
    assert_eq!(trace["status"], "ok");
    assert_eq!(trace["value"]["loaded"], true);
    assert_eq!(trace["value"]["path"], trace_path.display().to_string());
    assert!(
        trace["value"]["resources"]
            .as_array()
            .is_some_and(
                |resources| resources.iter().any(|resource| resource["mimeType"]
                    == "application/vnd.arcweft.agent-trace+json"
                    && resource["uri"]
                        .as_str()
                        .is_some_and(|uri| uri.ends_with("/trace.arcwx")))
            ),
        "trace cell should expose loaded trace resource descriptor: {trace}"
    );
    let query = cells
        .iter()
        .find(|cell| cell["input"] == ":query observation_received")
        .expect("trace query cell is present");
    assert_eq!(query["status"], "ok");
    assert_eq!(query["value"]["query"]["text"], "observation_received");
    assert!(
        query["value"]["items"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "read-only trace query should expose RAG context items: {query}"
    );
    assert_agent_repl_meta_ok(cells, ":classify let frame = try observe()");
    assert_agent_repl_meta_ok(cells, ":highlight :query observation_received");

    assert_agent_repl_readonly_rejects_mutating_cell(&trace_path);
}

fn assert_agent_repl_readonly_rejects_mutating_cell(trace_path: &Path) {
    let mutating_input_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/repl-readonly-mutating-{}.txt",
        std::process::id()
    ));
    fs::write(&mutating_input_path, "let frame = observe()\n")
        .expect("write mutating read-only REPL input");
    let rejected = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("repl")
        .arg("--trace")
        .arg(trace_path)
        .arg("--read-only")
        .arg("--input")
        .arg(&mutating_input_path)
        .arg("--json")
        .output()
        .expect("arcw agent repl rejects mutating cell in read-only mode");
    assert!(
        !rejected.status.success(),
        "read-only REPL should reject non-meta cells\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rejected.stdout),
        String::from_utf8_lossy(&rejected.stderr)
    );
    let rejected_report: serde_json::Value =
        serde_json::from_slice(&rejected.stdout).expect("read-only rejection output is JSON");
    assert_eq!(rejected_report["ok"], false);
    assert_eq!(rejected_report["cells"][0]["kind"], "cell");
    assert_eq!(rejected_report["cells"][0]["status"], "error");
    assert_eq!(
        rejected_report["cells"][0]["message"],
        "read-only Agent REPL does not execute Agent cells"
    );
}

fn assert_agent_repl_meta_ok(cells: &[serde_json::Value], input: &str) {
    let cell = cells
        .iter()
        .find(|cell| cell["input"] == input)
        .unwrap_or_else(|| panic!("{input} cell is present"));
    assert_eq!(cell["status"], "ok", "{input} should succeed: {cell}");
    assert_eq!(cell["kind"], "meta");
}

fn assert_agent_repl_observe_bindings(
    binding_list: &[serde_json::Value],
    bindings: &serde_json::Value,
) {
    assert!(
        binding_list
            .iter()
            .any(|binding| binding["name"] == "cell.6"
                && binding["binding_kind"] == "cell"
                && binding["source"] == "let observed = observe()"
                && binding["host_calls"] == 1),
        "bindings should include the compiled observe cell: {bindings}"
    );
    assert!(
        binding_list
            .iter()
            .any(|binding| binding["name"] == "observed"
                && binding["binding_kind"] == "local"
                && binding["source"] == "let observed = observe()"
                && binding["host_calls"] == 1),
        "bindings should include the extracted local observe binding: {bindings}"
    );
}

fn assert_repl_debug_cell_persisted(debug_db_path: &Path) {
    let store = DebugStore::open(debug_db_path).expect("open REPL debug database");
    let session = SessionId::new("session.cli").expect("CLI session id is valid");
    let persisted_session = store
        .session(&session)
        .expect("load persisted REPL session")
        .expect("REPL session is persisted");
    assert_eq!(persisted_session.status, DebugSessionStatus::Finished);
    assert!(persisted_session.started_unix_ms > 0);
    assert!(
        persisted_session
            .ended_unix_ms
            .is_some_and(|ended| ended >= persisted_session.started_unix_ms),
        "REPL debug session should record a real end timestamp: {persisted_session:?}"
    );
    assert_eq!(
        persisted_session.metadata["persisted_cells"].as_u64(),
        Some(1)
    );
    assert_eq!(persisted_session.metadata["read_only"], false);
    let repl_cells = store
        .repl_cells_for_session(&session)
        .expect("load persisted REPL cells");
    assert_eq!(repl_cells.len(), 1);
    let persisted = &repl_cells[0];
    assert_eq!(persisted.ordinal, 6);
    assert_eq!(persisted.source, "let observed = observe()");
    assert_eq!(persisted.status, "ok");
    assert!(persisted.partially_effectful);
    assert_eq!(
        persisted
            .display
            .as_ref()
            .and_then(|display| display["host_calls"].as_u64()),
        Some(1)
    );
}

#[test]
#[ignore = "tier 2 native Agent REPL E2E: requires native-capture feature subprocess"]
fn agent_repl_marks_failed_cells_partially_effectful_from_host_events() {
    let input_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/repl-partial-effects-{}.txt",
        std::process::id()
    ));
    let debug_db_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/repl-partial-effects-{}.sqlite3",
        std::process::id()
    ));
    fs::create_dir_all(input_path.parent().expect("input target dir"))
        .expect("create REPL partial-effects input target dir");
    let _ = fs::remove_file(&input_path);
    let _ = fs::remove_file(&debug_db_path);
    fs::write(
        &input_path,
        "while true {}\nexpect(false, message = \"boom\")\nlet shot = capture(viewport())\n:bindings\n",
    )
    .expect("write REPL partial-effects input");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("repl")
        .arg("--input")
        .arg(&input_path)
        .arg("--debug-db")
        .arg(&debug_db_path)
        .arg("--json")
        .output()
        .expect("arcw agent repl runs partial-effects input session");
    assert!(
        !output.status.success(),
        "partial-effects REPL session should fail while returning JSON\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent repl output is JSON");
    assert_eq!(report["ok"], false);
    let cells = report["cells"].as_array().expect("cells are present");
    assert_repl_failed_cell_effect_summary(cells, "while true {}", 0, false);
    assert_repl_failed_cell_effect_summary(cells, "expect(false, message = \"boom\")", 1, true);
    assert_repl_failed_cell_effect_summary(cells, "let shot = capture(viewport())", 1, true);
    let bindings = cells
        .iter()
        .find(|cell| cell["input"] == ":bindings")
        .and_then(|cell| cell["value"]["bindings"].as_array());
    let bindings = bindings.expect(":bindings cell reports visible bindings");
    assert!(
        !bindings.iter().any(|binding| binding["name"] == "shot"),
        "failed snapshot escape must not commit `shot` binding: {cells:?}"
    );

    let store = DebugStore::open(&debug_db_path).expect("open REPL debug database");
    let session = SessionId::new("session.cli").expect("CLI session id is valid");
    let persisted_session = store
        .session(&session)
        .expect("load failed REPL session")
        .expect("REPL session is persisted");
    assert_eq!(persisted_session.status, DebugSessionStatus::Failed);
    assert_eq!(
        persisted_session.metadata["persisted_cells"].as_u64(),
        Some(3)
    );
    let repl_cells = store
        .repl_cells_for_session(&session)
        .expect("load persisted REPL cells");
    assert_eq!(repl_cells.len(), 3);
    assert_eq!(repl_cells[0].source, "while true {}");
    assert!(!repl_cells[0].partially_effectful);
    assert_eq!(
        repl_cells[0]
            .display
            .as_ref()
            .and_then(|display| display["host_calls"].as_u64()),
        Some(0)
    );
    assert_eq!(repl_cells[1].source, "expect(false, message = \"boom\")");
    assert!(repl_cells[1].partially_effectful);
    assert_eq!(
        repl_cells[1]
            .display
            .as_ref()
            .and_then(|display| display["host_calls"].as_u64()),
        Some(1)
    );
    assert_eq!(repl_cells[2].source, "let shot = capture(viewport())");
    assert!(repl_cells[2].partially_effectful);
    assert_eq!(
        repl_cells[2]
            .display
            .as_ref()
            .and_then(|display| display["snapshot_error"].as_str()),
        Some("REPL local binding(s) cannot cross cells without a supported snapshot: shot")
    );
}

fn assert_repl_failed_cell_effect_summary(
    cells: &[serde_json::Value],
    input: &str,
    host_calls: u64,
    partially_effectful: bool,
) {
    let cell = cells
        .iter()
        .find(|cell| cell["input"] == input)
        .unwrap_or_else(|| panic!("{input} cell is present"));
    assert_eq!(cell["status"], "error", "{input} should fail: {cell}");
    assert_eq!(cell["value"]["host_calls"], host_calls);
    assert_eq!(cell["value"]["partially_effectful"], partially_effectful);
}

#[test]
#[ignore = "tier 2 native Agent REPL E2E: requires native-capture feature subprocess"]
fn agent_repl_saves_loads_and_drops_bindings_from_input_session() {
    let input_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/repl-save-load-{}.txt",
        std::process::id()
    ));
    let saved_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/repl-saved-{}.awfagent",
        std::process::id()
    ));
    fs::create_dir_all(input_path.parent().expect("input target dir"))
        .expect("create REPL input target dir");
    let _ = fs::remove_file(&input_path);
    let _ = fs::remove_file(&saved_path);
    fs::write(
        &input_path,
        format!(
            ":help\nlet answer = 1u32\n:bindings\n:drop answer\n:bindings\n:save {}\n:load {}\n:quit\n",
            saved_path.display(),
            saved_path.display()
        ),
    )
    .expect("write REPL save/load input");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("repl")
        .arg("--input")
        .arg(&input_path)
        .arg("--json")
        .output()
        .expect("arcw agent repl runs save/load input session");
    assert!(
        output.status.success(),
        "agent repl save/load session should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent repl output is JSON");
    assert_eq!(report["ok"], true);

    let cells = report["cells"].as_array().expect("cells are present");
    let drop_cell = cells
        .iter()
        .find(|cell| cell["input"] == ":drop answer")
        .expect("drop cell is present");
    assert_eq!(drop_cell["status"], "ok");
    assert_eq!(drop_cell["value"]["dropped"], "answer");
    let saved_cell = cells
        .iter()
        .find(|cell| {
            cell["input"]
                .as_str()
                .is_some_and(|input| input.starts_with(":save "))
        })
        .expect("save cell is present");
    assert_eq!(saved_cell["status"], "ok");
    assert_eq!(
        saved_cell["value"]["saved"],
        saved_path.display().to_string()
    );
    let load_cell = cells
        .iter()
        .find(|cell| {
            cell["input"]
                .as_str()
                .is_some_and(|input| input.starts_with(":load "))
        })
        .expect("load cell is present");
    assert_eq!(load_cell["status"], "ok");
    assert!(
        load_cell["value"]["binding"]
            .as_str()
            .is_some_and(|binding| binding.starts_with("loaded.")),
        "load should create a loaded_agent binding: {load_cell}"
    );

    let check_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("check")
        .arg(&saved_path)
        .arg("--json")
        .output()
        .expect("arcw agent script check validates saved REPL agent");
    assert!(
        check_output.status.success(),
        "saved REPL agent should pass script check\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check_output.stdout),
        String::from_utf8_lossy(&check_output.stderr)
    );
    let check_json: serde_json::Value =
        serde_json::from_slice(&check_output.stdout).expect("script check output is JSON");
    assert_eq!(check_json["ok"], true);
    assert_eq!(check_json["agents"], 1);
}

#[test]
#[ignore = "tier 2 native Agent REPL E2E: requires native-capture feature subprocess"]
fn agent_repl_executes_and_saves_physical_pointer_cell() {
    let input_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/repl-pointer-click-{}.txt",
        std::process::id()
    ));
    let saved_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/repl-pointer-click-saved-{}.awfagent",
        std::process::id()
    ));
    fs::create_dir_all(input_path.parent().expect("input target dir"))
        .expect("create REPL pointer input target dir");
    let _ = fs::remove_file(&input_path);
    let _ = fs::remove_file(&saved_path);
    fs::write(
        &input_path,
        format!(
            "try pointer.click(viewport_point(24u32, 48u32), button = .primary)\n:save {}\n:quit\n",
            saved_path.display()
        ),
    )
    .expect("write REPL pointer input");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("repl")
        .arg("--input")
        .arg(&input_path)
        .arg("--json")
        .output()
        .expect("arcw agent repl runs physical pointer input session");
    assert!(
        output.status.success(),
        "agent repl physical pointer session should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent repl output is JSON");
    assert_eq!(report["ok"], true);
    let cells = report["cells"].as_array().expect("cells are present");
    let action_cell = cells
        .iter()
        .find(|cell| {
            cell["input"] == "try pointer.click(viewport_point(24u32, 48u32), button = .primary)"
        })
        .expect("pointer action cell is present");
    assert_eq!(action_cell["status"], "ok");
    assert_eq!(action_cell["value"]["host_calls"], 1);
    assert_eq!(action_cell["value"]["responses"][0]["kind"], "action");
    assert_eq!(
        action_cell["value"]["responses"][0]["response"]["accepted"],
        true
    );

    let saved_source = fs::read_to_string(&saved_path).expect("saved REPL source exists");
    assert!(saved_source.contains("agent.act.physical"));
    assert!(saved_source.contains("pointer.click"));

    let check_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("check")
        .arg(&saved_path)
        .arg("--json")
        .output()
        .expect("arcw agent script check validates saved physical REPL agent");
    assert!(
        check_output.status.success(),
        "saved physical REPL agent should pass script check\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check_output.stdout),
        String::from_utf8_lossy(&check_output.stderr)
    );
    let check_json: serde_json::Value =
        serde_json::from_slice(&check_output.stdout).expect("script check output is JSON");
    assert_eq!(check_json["ok"], true);
    assert_eq!(check_json["agents"], 1);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: requires native-capture feature subprocess"]
fn agent_mcp_stdio_reads_agent_trace_resource() {
    let trace_path = workspace_path(&format!(
        "target/codex-agent-script-run-test/mcp-trace-{}.arcwx",
        std::process::id()
    ));
    let _ = fs::remove_file(&trace_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_run_smoke_path())
        .arg("--json")
        .arg("--trace-out")
        .arg(&trace_path)
        .output()
        .expect("arcw agent script run writes trace for MCP");
    assert!(
        output.status.success(),
        "agent script run should write trace for MCP\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let trace_uri = "arcweft://run/run.cli/trace.arcwx";
    let requests = vec![
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.trace.read",
                "arguments": { "path": trace_path.display().to_string() }
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "id": 4, "method": "resources/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "resources/read",
            "params": { "uri": trace_uri }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp trace read should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .expect("tools list is array")
            .iter()
            .any(|tool| tool["name"] == "arcweft.trace.read")
    );
    assert_eq!(responses[2]["result"]["content"][0]["type"], "resource");
    assert_eq!(
        responses[2]["result"]["content"][0]["resource"]["uri"],
        trace_uri
    );
    assert!(
        responses[3]["result"]["resources"]
            .as_array()
            .expect("resource list is array")
            .iter()
            .any(|resource| resource["uri"] == trace_uri
                && resource["mimeType"] == "application/vnd.arcweft.agent-trace+json")
    );
    let trace_text = responses[4]["result"]["contents"][0]["text"]
        .as_str()
        .expect("trace resource text");
    let trace_json: serde_json::Value =
        serde_json::from_str(trace_text).expect("trace resource text is JSON");
    assert_eq!(trace_json.as_array().unwrap().len(), 5);
    assert_eq!(trace_json[2]["kind"], "observation_received");
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: requires native-capture feature subprocess"]
fn agent_mcp_stdio_waits_for_observation_predicate() {
    let source = workspace_path("samples/agent-script/native-project-index.arcw");
    let requests = vec![
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.wait",
                "arguments": {
                    "source": source.display().to_string(),
                    "predicate": {
                        "kind": "compare",
                        "probe": { "kind": "observation_field", "path": "tick" },
                        "op": "greater_or_equal",
                        "value": { "kind": "u64", "value": 1 }
                    },
                    "timeout_millis": 4,
                    "stable_frames": 1,
                    "poll_frames": 1
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp wait should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .expect("tools list is array")
            .iter()
            .any(|tool| tool["name"] == "arcweft.wait")
    );
    assert_eq!(responses[2]["result"]["isError"], false);
    let text = responses[2]["result"]["content"][0]["text"]
        .as_str()
        .expect("wait result text");
    let wait: serde_json::Value = serde_json::from_str(text).expect("wait result is JSON");
    assert_eq!(wait["matched"], true);
    assert_eq!(wait["stable_seen"], 1);
    assert!(
        wait["observation"]["tick"].as_u64().unwrap_or_default() >= 1,
        "wait should return the matching observation: {wait}"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: shared Agent Script runner subprocess with native-capture feature"]
fn agent_mcp_stdio_runs_agent_script() {
    let signal_script = agent_script_cli_composite_wait_smoke_path();
    let state_script = agent_script_cli_state_wait_smoke_path();
    let requests = vec![
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.script.run",
                "arguments": {
                    "path": signal_script.display().to_string(),
                    "signals": {
                        "@signal.ready": true
                    },
                    "max_steps": 16,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "arcweft.script.run",
                "arguments": {
                    "path": state_script.display().to_string(),
                    "state": {
                        "route.phase": "opening"
                    },
                    "max_steps": 16,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp script.run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .expect("tools list is array")
            .iter()
            .any(|tool| tool["name"] == "arcweft.script.run")
    );
    for index in [2, 3] {
        assert_eq!(responses[index]["result"]["isError"], false);
        let text = responses[index]["result"]["content"][0]["text"]
            .as_str()
            .expect("script.run result text");
        let run: serde_json::Value = serde_json::from_str(text).expect("script.run result is JSON");
        assert_eq!(run["ok"], true);
        assert_eq!(run["agents"], 1);
        assert_eq!(run["host_calls"], 1);
        assert_eq!(run["final_status"], "Done(Return(\"done\"))");
    }
}

#[test]
fn agent_script_run_json_executes_read_resource_value_smoke() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_read_resource_value_smoke_path())
        .arg("--json")
        .output()
        .expect("arcw agent script run executes read_resource value smoke");

    assert!(
        output.status.success(),
        "agent script read_resource value run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("read_resource value run output is JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["host_calls"], 1);
    assert_eq!(json["responses"][0]["kind"], "resource");
    assert_eq!(
        json["responses"][0]["response"]["body"]["body"]["source"],
        "arcw agent script run"
    );
    assert_eq!(json["final_status"], "Done(Return(\"record/2\"))");
}

fn assert_agent_script_build(bundle_path: &Path) {
    let build_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("build")
        .arg(agent_script_cli_run_smoke_path())
        .arg("--output")
        .arg(bundle_path)
        .arg("--json")
        .output()
        .expect("arcw agent script build writes an agent bundle");
    assert!(
        build_output.status.success(),
        "agent script build should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );
    let build_json: serde_json::Value =
        serde_json::from_slice(&build_output.stdout).expect("build output is JSON");
    assert_eq!(build_json["ok"], true);
    assert_eq!(build_json["bundle_kind"], "agent_controller");
    assert_eq!(build_json["agent_id"], "agent.cli.run_smoke");
    let bundle_json: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle_path).expect("build writes .awfb bundle"))
            .expect("bundle is JSON");
    assert_eq!(bundle_json["bundle_kind"], "agent_controller");
    assert_eq!(bundle_json["agent"]["agent_id"], "agent.cli.run_smoke");
}

fn assert_agent_script_bundle_run(bundle_path: &Path, trace_path: &Path) {
    let bundle_run_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(bundle_path)
        .arg("--json")
        .arg("--trace-out")
        .arg(trace_path)
        .output()
        .expect("arcw agent script run executes built bundle");
    assert!(
        bundle_run_output.status.success(),
        "agent script run should execute built .awfb\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&bundle_run_output.stdout),
        String::from_utf8_lossy(&bundle_run_output.stderr)
    );
    let bundle_run_json: serde_json::Value =
        serde_json::from_slice(&bundle_run_output.stdout).expect("bundle run output is JSON");
    assert_eq!(bundle_run_json["ok"], true);
    assert_eq!(bundle_run_json["agents"], 1);
    assert_eq!(bundle_run_json["host_calls"], 1);
    assert_eq!(bundle_run_json["trace_records"], 5);
    assert_eq!(bundle_run_json["responses"][0]["kind"], "observation");
}

fn assert_agent_script_source_run_trace(trace_path: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("run")
        .arg(agent_script_cli_run_smoke_path())
        .arg("--json")
        .arg("--trace-out")
        .arg(trace_path)
        .output()
        .expect("arcw agent script run executes");

    assert!(
        output.status.success(),
        "agent script run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("run output is JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["host_calls"], 1);
    assert_eq!(json["trace_records"], 5);
    assert_eq!(json["responses"][0]["kind"], "observation");
    let trace: serde_json::Value = serde_json::from_slice(
        &fs::read(trace_path).expect("agent script run writes .arcwx trace"),
    )
    .expect("trace is JSON");
    assert_eq!(trace.as_array().unwrap().len(), 5);
    assert_eq!(trace[0]["kind"], "run_started");
    assert_eq!(trace[2]["kind"], "observation_received");
    assert_eq!(trace[4]["kind"], "run_finished");
}

fn assert_agent_script_trace_report(trace_path: &Path) {
    let trace_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("trace")
        .arg(trace_path)
        .arg("--json")
        .output()
        .expect("arcw agent script trace validates run trace");
    assert!(
        trace_output.status.success(),
        "agent script trace should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&trace_output.stdout),
        String::from_utf8_lossy(&trace_output.stderr)
    );
    let trace_report: serde_json::Value =
        serde_json::from_slice(&trace_output.stdout).expect("trace output is JSON");
    assert_eq!(trace_report["ok"], true);
    assert_eq!(trace_report["records"], 5);
    assert_eq!(trace_report["run_id"], "run.cli");
    assert_eq!(trace_report["started"], true);
    assert_eq!(trace_report["finished"], true);
    assert_eq!(trace_report["kinds"]["vm_step"], 2);
}

fn assert_agent_script_replay_matches(trace_path: &Path, expected_path: &Path) {
    let replay_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("script")
        .arg("replay")
        .arg(trace_path)
        .arg("--expect")
        .arg(expected_path)
        .arg("--json")
        .output()
        .expect("arcw agent script replay compares traces");
    assert!(
        replay_output.status.success(),
        "agent script replay should match source and bundle traces\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&replay_output.stdout),
        String::from_utf8_lossy(&replay_output.stderr)
    );
    let replay_report: serde_json::Value =
        serde_json::from_slice(&replay_output.stdout).expect("replay output is JSON");
    assert_eq!(replay_report["ok"], true);
    assert_eq!(replay_report["events"], 5);
    assert_eq!(replay_report["matched_expected"], true);
    assert_eq!(
        replay_report["logical_sequence"][2]["kind"],
        "observation_received"
    );
}

fn assert_agent_rag_query_trace(trace_path: &Path) {
    let db_path = workspace_path(&format!(
        "target/codex-agent-rag-cli-smoke/rag-audit-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("RAG audit DB parent"))
        .expect("create RAG audit DB parent");
    let rag_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("query")
        .arg("--trace")
        .arg(trace_path)
        .arg("--debug-db")
        .arg(&db_path)
        .arg("--query")
        .arg("observation_received")
        .arg("--root")
        .arg("observation_received")
        .arg("--json")
        .output()
        .expect("arcw agent rag query reads trace");
    assert!(
        rag_output.status.success(),
        "agent rag query should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rag_output.stdout),
        String::from_utf8_lossy(&rag_output.stderr)
    );
    let pack: serde_json::Value =
        serde_json::from_slice(&rag_output.stdout).expect("RAG output is JSON");
    assert_eq!(pack["schema_version"], 1);
    assert_eq!(pack["query"]["text"], "observation_received");
    assert_eq!(
        pack["query"]["roots"],
        serde_json::json!(["observation_received"])
    );
    assert!(
        pack["items"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(pack["items"].as_array().unwrap().iter().any(|item| {
        item["title"]
            .as_str()
            .is_some_and(|title| title.contains("observation_received"))
            && item["channels"]
                .as_array()
                .is_some_and(|channels| channels.contains(&serde_json::json!("exact_entity")))
    }));
    let query_id = pack["query"]["query_id"]
        .as_str()
        .expect("RAG query id")
        .to_owned();
    assert_agent_rag_query_persisted_debug_store(&db_path, &query_id);
    let _ = fs::remove_file(&db_path);
    let _ = fs::remove_file(db_path.with_extension("sqlite3-shm"));
    let _ = fs::remove_file(db_path.with_extension("sqlite3-wal"));
}

fn assert_agent_rag_query_persisted_debug_store(db_path: &Path, query_id: &str) {
    let status_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("status")
        .arg("--path")
        .arg(db_path)
        .arg("--json")
        .output()
        .expect("arcw debug db status reads RAG audit DB");
    assert!(
        status_output.status.success(),
        "debug db status should read RAG audit DB\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&status_output.stdout),
        String::from_utf8_lossy(&status_output.stderr)
    );
    let status: serde_json::Value =
        serde_json::from_slice(&status_output.stdout).expect("debug db status JSON");
    assert_eq!(status["stats"]["rag_queries"], 1);
    assert_eq!(status["stats"]["sessions"], 1);
    assert!(
        status["stats"]["chunks"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "agent rag query should index trace-derived chunks: {status}"
    );

    let search_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(db_path)
        .arg("--query")
        .arg("observation_received")
        .arg("--json")
        .output()
        .expect("arcw debug db search reads indexed RAG chunks");
    assert!(
        search_output.status.success(),
        "debug db search should find RAG chunks\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&search_output.stdout),
        String::from_utf8_lossy(&search_output.stderr)
    );
    let search: serde_json::Value =
        serde_json::from_slice(&search_output.stdout).expect("debug db search JSON");
    assert!(
        search["hits"]
            .as_array()
            .is_some_and(|hits| !hits.is_empty()),
        "agent rag query should create searchable RAG chunks: {search}"
    );

    let rag_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("rag")
        .arg("--path")
        .arg(db_path)
        .arg("--query-id")
        .arg(query_id)
        .arg("--json")
        .output()
        .expect("arcw debug db rag reads persisted RAG audit session");
    assert!(
        rag_output.status.success(),
        "debug db rag should find RAG session ownership\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rag_output.stdout),
        String::from_utf8_lossy(&rag_output.stderr)
    );
    let rag: serde_json::Value =
        serde_json::from_slice(&rag_output.stdout).expect("debug db rag JSON");
    let session_id = rag["session_id"].as_str().expect("RAG session id");
    assert!(
        session_id.starts_with("session.rag.cli.blake3."),
        "RAG audit should carry a product session id: {rag}"
    );
    assert_eq!(rag["run_id"], serde_json::Value::Null);

    let sessions_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("sessions")
        .arg("--path")
        .arg(db_path)
        .arg("--json")
        .output()
        .expect("arcw debug db sessions reads RAG query session");
    assert!(
        sessions_output.status.success(),
        "debug db sessions should list RAG query session\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&sessions_output.stdout),
        String::from_utf8_lossy(&sessions_output.stderr)
    );
    let sessions: serde_json::Value =
        serde_json::from_slice(&sessions_output.stdout).expect("debug db sessions JSON");
    assert_eq!(sessions["sessions"][0]["session_id"], session_id);
    assert_eq!(sessions["sessions"][0]["profile"], "rag");
    assert_eq!(sessions["sessions"][0]["transport"], "cli");
    assert_eq!(sessions["sessions"][0]["status"], "finished");
    assert_eq!(sessions["sessions"][0]["metadata"]["query_id"], query_id);
}

fn assert_agent_rag_query_trace_respects_privacy(trace_path: &Path) {
    let rag_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("query")
        .arg("--trace")
        .arg(trace_path)
        .arg("--query")
        .arg("observation_received")
        .arg("--max-privacy")
        .arg("public")
        .arg("--json")
        .output()
        .expect("arcw agent rag query enforces privacy");
    assert!(
        rag_output.status.success(),
        "agent rag query with public privacy should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rag_output.stdout),
        String::from_utf8_lossy(&rag_output.stderr)
    );
    let pack: serde_json::Value =
        serde_json::from_slice(&rag_output.stdout).expect("RAG output is JSON");
    assert_eq!(pack["query"]["text"], "observation_received");
    assert_eq!(pack["items"].as_array().unwrap().len(), 0);
}

#[test]
fn agent_rag_index_persists_source_chunks_and_skips_unchanged() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-rag-source/index-rag-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("RAG index DB parent"))
        .expect("create RAG index DB parent");
    let source_path = workspace_path(&format!(
        "target/codex-agent-rag-source/project-callables-{}.arcw",
        std::process::id()
    ));
    write_project_callable_rag_fixture(&source_path);

    let first = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("index")
        .arg("--source")
        .arg(&source_path)
        .arg("--debug-db")
        .arg(&db_path)
        .arg("--json")
        .output()
        .expect("arcw agent rag index runs");
    assert!(
        first.status.success(),
        "agent rag index should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let first_report: serde_json::Value =
        serde_json::from_slice(&first.stdout).expect("agent rag index output is JSON");
    assert_eq!(first_report["changed_only"], false);
    let first_session_id = first_report["session_id"]
        .as_str()
        .expect("first RAG index session id")
        .to_owned();
    assert!(
        first_session_id.starts_with("session.rag.index.cli.blake3."),
        "agent rag index should report a product session id: {first_report}"
    );
    assert_eq!(first_report["indexed_sources"], 1);
    assert_eq!(first_report["sources"][0]["source_file_recorded"], true);
    assert!(
        first_report["indexed_chunks"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "agent rag index should persist chunks: {first_report}"
    );

    let second = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("index")
        .arg("--source")
        .arg(&source_path)
        .arg("--debug-db")
        .arg(&db_path)
        .arg("--changed")
        .arg("--json")
        .output()
        .expect("arcw agent rag index --changed runs");
    assert!(
        second.status.success(),
        "agent rag index --changed should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );
    let second_report: serde_json::Value =
        serde_json::from_slice(&second.stdout).expect("agent rag index changed output is JSON");
    assert_eq!(second_report["changed_only"], true);
    let second_session_id = second_report["session_id"]
        .as_str()
        .expect("second RAG index session id");
    assert!(
        second_session_id.starts_with("session.rag.index.cli.blake3."),
        "agent rag index --changed should report a product session id: {second_report}"
    );
    assert_ne!(second_session_id, first_session_id);
    assert_eq!(second_report["indexed_sources"], 0);
    assert_eq!(second_report["skipped_unchanged_sources"], 1);
    assert_eq!(second_report["sources"][0]["source_file_recorded"], true);
    assert_eq!(second_report["indexed_chunks"], 0);
    assert!(
        second_report["skipped_unchanged_chunks"]
            .as_u64()
            .is_some_and(|count| count == first_report["indexed_chunks"].as_u64().unwrap()),
        "agent rag index --changed should skip already indexed chunks: {second_report}"
    );

    assert_debug_db_search_exposes_persisted_source_chunks(&db_path);
    assert_debug_db_source_files_are_persisted(&db_path, &first_report, &source_path);
    assert_debug_db_sources_command_reports_indexed_source_files(
        &db_path,
        &first_report,
        &source_path,
    );
    assert_debug_db_graph_command_reports_indexed_project_graph(&db_path, &first_report);
    assert_debug_db_graph_search_exposes_indexed_project_graph_edges(&db_path);
    assert_agent_rag_index_sessions_are_persisted(&db_path, &first_session_id, second_session_id);
    assert_agent_rag_query_reads_persisted_source_index(&db_path);

    remove_sqlite_files(&db_path);
}

fn remove_sqlite_files(db_path: &Path) {
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(db_path.with_extension("sqlite3-shm"));
    let _ = fs::remove_file(db_path.with_extension("sqlite3-wal"));
}

fn write_project_callable_rag_fixture(source_path: &Path) {
    fs::write(
        source_path,
        r#"
signal @signal.current_flow: Watch<Ref<Flow>>

entry game @entry.main {
    goto @flow.opening
}

pub reducer update_route(state: GameState, event: GameEvent) -> GameState {
    let route = current_route(state)
    state
}

pub reducer current_route(state: GameState) -> Ref<Flow> {
    @flow.opening
}

flow opening effects { signal.write } {
    signal.set(@signal.current_flow, @flow.opening)
    let route = current_route()
    goto route

    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
        @choice.opening.silent "黙っている" -> @flow.quiet_intro
    }
}

flow alice_intro effects { signal.write } {
    signal.set(@signal.current_flow, @flow.alice_intro)
    return "alice_intro"
}

flow quiet_intro effects { signal.write } {
    signal.set(@signal.current_flow, @flow.quiet_intro)
    return "quiet_intro"
}
"#,
    )
    .expect("write temporary RAG project source");
}

fn assert_debug_db_source_files_are_persisted(
    db_path: &Path,
    report: &serde_json::Value,
    source_path: &Path,
) {
    let program_hash = stable_hash(report["program_hash"].as_str().expect("program hash"));
    let source_hash = stable_hash(
        report["sources"][0]["source_hash"]
            .as_str()
            .expect("source hash"),
    );
    let store = DebugStore::open(db_path).expect("open RAG debug DB");
    let source_files = store
        .source_files_for_program(&program_hash)
        .expect("source files for indexed program");
    assert_eq!(source_files.len(), 1);
    assert_eq!(source_files[0].path, source_path.display().to_string());
    assert_eq!(source_files[0].language, "arcw");
    assert_eq!(source_files[0].content_hash, source_hash);
    assert!(source_files[0].byte_len > 0);
    assert_eq!(store.stats().expect("stats").source_files, 1);
}

fn assert_debug_db_sources_command_reports_indexed_source_files(
    db_path: &Path,
    report: &serde_json::Value,
    source_path: &Path,
) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("sources")
        .arg("--path")
        .arg(db_path)
        .arg("--program-hash")
        .arg(report["program_hash"].as_str().expect("program hash"))
        .arg("--json")
        .output()
        .expect("arcw debug db sources runs");
    assert!(
        output.status.success(),
        "debug db sources should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let sources_report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db sources output is JSON");
    assert_eq!(sources_report["sources"].as_array().map(Vec::len), Some(1));
    assert_eq!(sources_report["max_privacy"], "project");
    assert_eq!(
        sources_report["program_hash"], report["program_hash"],
        "debug db sources should report the queried program hash"
    );
    assert_eq!(
        sources_report["sources"][0]["path"],
        serde_json::json!(source_path.display().to_string())
    );
    assert_eq!(sources_report["sources"][0]["language"], "arcw");
    assert_eq!(
        sources_report["sources"][0]["content_hash"],
        report["sources"][0]["source_hash"]
    );
    assert!(
        sources_report["sources"][0]["byte_len"]
            .as_u64()
            .is_some_and(|byte_len| byte_len > 0),
        "debug db sources should preserve byte length: {sources_report}"
    );

    let public_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("sources")
        .arg("--path")
        .arg(db_path)
        .arg("--program-hash")
        .arg(report["program_hash"].as_str().expect("program hash"))
        .arg("--max-privacy")
        .arg("public")
        .arg("--json")
        .output()
        .expect("arcw debug db sources public runs");
    assert!(
        public_output.status.success(),
        "debug db sources public should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&public_output.stdout),
        String::from_utf8_lossy(&public_output.stderr)
    );
    let public_report: serde_json::Value = serde_json::from_slice(&public_output.stdout)
        .expect("debug db sources public output is JSON");
    assert_eq!(public_report["max_privacy"], "public");
    assert_eq!(
        public_report["sources"].as_array().map(Vec::len),
        Some(0),
        "project-private source inventory should be omitted at public ceiling"
    );
}

fn assert_debug_db_graph_command_reports_indexed_project_graph(
    db_path: &Path,
    report: &serde_json::Value,
) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("graph")
        .arg("--path")
        .arg(db_path)
        .arg("--program-hash")
        .arg(report["program_hash"].as_str().expect("program hash"))
        .arg("--json")
        .output()
        .expect("arcw debug db graph runs");
    assert!(
        output.status.success(),
        "debug db graph should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let graph_report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db graph output is JSON");
    assert_eq!(
        graph_report["program_hash"], report["program_hash"],
        "debug db graph should report the queried program hash"
    );
    assert_eq!(graph_report["max_privacy"], "project");
    assert!(
        graph_report["symbol_count"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "debug db graph should expose indexed symbols: {graph_report}"
    );
    assert!(
        graph_report["edge_count"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "debug db graph should expose indexed edges: {graph_report}"
    );
    let symbols = graph_report["symbols"].as_array().expect("symbols");
    assert!(
        symbols.iter().any(|symbol| {
            symbol["public_id"] == "choice.opening.listen"
                && symbol["kind"] == "ChoiceOption"
                && symbol["source_path"]
                    .as_str()
                    .is_some_and(|path| path.contains("project-callables-"))
        }),
        "debug db graph should expose source-owned project entity symbols: {graph_report}"
    );
    assert!(
        symbols.iter().any(|symbol| {
            symbol["public_id"] == "flow.opening"
                && symbol["metadata"]["flow_control"]["dynamic_goto_count"] == 1
                && symbol["metadata"]["flow_control"]["has_dynamic_control"] == true
        }),
        "debug db graph should expose dynamic flow-control summaries: {graph_report}"
    );
    let edges = graph_report["edges"].as_array().expect("edges");
    assert!(
        edges
            .iter()
            .any(|edge| edge["edge_kind"] == "contains_entity"),
        "debug db graph should expose ownership edges: {graph_report}"
    );
    assert_debug_db_graph_exposes_source_file_ownership(symbols, edges, report, &graph_report);
    assert_debug_db_graph_exposes_project_callables(symbols, edges, &graph_report);
    assert_debug_db_graph_exposes_project_domain_summary(symbols, &graph_report);

    let public_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("graph")
        .arg("--path")
        .arg(db_path)
        .arg("--program-hash")
        .arg(report["program_hash"].as_str().expect("program hash"))
        .arg("--max-privacy")
        .arg("public")
        .arg("--json")
        .output()
        .expect("arcw debug db graph public runs");
    assert!(
        public_output.status.success(),
        "debug db graph public should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&public_output.stdout),
        String::from_utf8_lossy(&public_output.stderr)
    );
    let public_graph_report: serde_json::Value = serde_json::from_slice(&public_output.stdout)
        .expect("debug db graph public output is JSON");
    assert_eq!(public_graph_report["max_privacy"], "public");
    assert_eq!(public_graph_report["symbol_count"], 0);
    assert_eq!(public_graph_report["edge_count"], 0);
    assert_eq!(
        public_graph_report["symbols"].as_array().map(Vec::len),
        Some(0),
        "project-private graph symbols should be omitted at public ceiling"
    );
    assert_eq!(
        public_graph_report["edges"].as_array().map(Vec::len),
        Some(0),
        "project-private graph edges should be omitted at public ceiling"
    );
}

fn assert_debug_db_graph_exposes_source_file_ownership(
    symbols: &[serde_json::Value],
    edges: &[serde_json::Value],
    report: &serde_json::Value,
    graph_report: &serde_json::Value,
) {
    assert!(
        symbols.iter().any(|symbol| {
            symbol["kind"] == "source_file"
                && symbol["qualified_name"]
                    .as_str()
                    .is_some_and(|path| path.contains("project-callables-"))
                && symbol["metadata"]["content_hash"] == report["sources"][0]["source_hash"]
        }),
        "debug db graph should expose source-file graph symbols: {graph_report}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["edge_kind"] == "contains_project_graph"
                && edge["metadata"]["source_content_hash"] == report["sources"][0]["source_hash"]
        }),
        "debug db graph should expose source-file project-graph ownership edges: {graph_report}"
    );
}

fn assert_debug_db_graph_exposes_project_callables(
    symbols: &[serde_json::Value],
    edges: &[serde_json::Value],
    graph_report: &serde_json::Value,
) {
    assert!(
        symbols.iter().any(|symbol| {
            symbol["qualified_name"] == "update_route"
                && symbol["kind"] == "project_reducer"
                && symbol["semantic_hash"]
                    .as_str()
                    .is_some_and(|hash| hash.contains("hir:callable:reducer:update_route"))
        }),
        "debug db graph should expose project reducer symbols: {graph_report}"
    );
    assert!(
        symbols.iter().any(|symbol| {
            symbol["qualified_name"] == "current_route" && symbol["kind"] == "project_reducer"
        }),
        "debug db graph should expose project callable symbols: {graph_report}"
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge["edge_kind"] == "contains_callable"),
        "debug db graph should expose callable ownership edges: {graph_report}"
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge["edge_kind"] == "calls_callable"),
        "debug db graph should expose callable dependency edges: {graph_report}"
    );
}

fn assert_debug_db_graph_exposes_project_domain_summary(
    symbols: &[serde_json::Value],
    graph_report: &serde_json::Value,
) {
    let summary = symbols
        .iter()
        .find(|symbol| symbol["kind"] == "project_summary")
        .expect("debug db graph should expose a project summary symbol");
    assert!(
        summary["metadata"]["entity_kind_counts"]["Flow"]
            .as_u64()
            .is_some_and(|count| count >= 3),
        "project summary should expose entity kind counts: {graph_report}"
    );
    assert_eq!(
        summary["metadata"]["relation_kind_counts"]["contains_choice"],
        serde_json::json!(1),
        "project summary should expose domain relation kinds: {graph_report}"
    );
    assert_eq!(
        summary["metadata"]["relation_kind_counts"]["contains_choice_option"],
        serde_json::json!(2),
        "project summary should count choice option ownership: {graph_report}"
    );
    assert_eq!(
        summary["metadata"]["relation_kind_counts"]["choice_option_goto"],
        serde_json::json!(2),
        "project summary should count choice option goto edges: {graph_report}"
    );
    assert!(
        summary["metadata"]["dependency_edge_kind_counts"]["calls_callable"]
            .as_u64()
            .is_some_and(|count| count >= 2),
        "project summary should expose callable dependency kinds: {graph_report}"
    );
    assert_eq!(
        summary["metadata"]["flow_control_counts"]["dynamic_goto_count"],
        serde_json::json!(1),
        "project summary should expose aggregate flow-control counts: {graph_report}"
    );
}

fn assert_debug_db_graph_search_exposes_indexed_project_graph_edges(db_path: &Path) {
    let public_report =
        debug_db_search_json(db_path, "--graph-query", "choice.opening.listen", "public");
    assert_eq!(public_report["hits"].as_array().map(Vec::len), Some(0));

    let report = debug_db_search_json(db_path, "--graph-query", "choice.opening.listen", "project");
    let hits = report["hits"].as_array().expect("graph hits");
    assert!(
        hits.iter().any(|hit| {
            hit["channel"] == "graph"
                && hit["source_kind"] == "graph_edge"
                && hit["title"]
                    .as_str()
                    .is_some_and(|title| title.contains("--contains_entity-->"))
                && hit["body"].as_str().is_some_and(|body| {
                    body.contains("to_summary=Project entity choice.opening.listen")
                })
        }),
        "debug db graph search should expose indexed project graph ownership edges: {report}"
    );
    let source_report = debug_db_search_json(
        db_path,
        "--graph-query",
        "contains_project_graph",
        "project",
    );
    let source_hits = source_report["hits"].as_array().expect("source graph hits");
    assert!(
        source_hits.iter().any(|hit| {
            hit["channel"] == "graph"
                && hit["source_kind"] == "graph_edge"
                && hit["title"]
                    .as_str()
                    .is_some_and(|title| title.contains("--contains_project_graph-->"))
                && hit["body"]
                    .as_str()
                    .is_some_and(|body| body.contains("from_kind=source_file"))
        }),
        "debug db graph search should expose source-file project graph ownership: {source_report}"
    );
}

fn assert_agent_rag_index_sessions_are_persisted(
    db_path: &Path,
    first_session_id: &str,
    second_session_id: &str,
) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("sessions")
        .arg("--path")
        .arg(db_path)
        .arg("--limit")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw debug db sessions reads RAG index sessions");
    assert!(
        output.status.success(),
        "debug db sessions should list RAG index sessions\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db sessions JSON");
    let sessions = report["sessions"].as_array().expect("sessions array");
    for (session_id, changed_only, indexed_chunks) in [
        (first_session_id, false, 1_u64),
        (second_session_id, true, 0_u64),
    ] {
        let session = sessions
            .iter()
            .find(|session| session["session_id"] == session_id)
            .unwrap_or_else(|| panic!("missing RAG index session {session_id}: {report}"));
        assert_eq!(session["profile"], "rag");
        assert_eq!(session["transport"], "cli");
        assert_eq!(session["status"], "finished");
        assert_eq!(session["metadata"]["operation"], "index");
        assert_eq!(session["metadata"]["changed_only"], changed_only);
        assert!(
            session["metadata"]["indexed_chunks"]
                .as_u64()
                .is_some_and(|count| count >= indexed_chunks),
            "RAG index session should preserve indexed chunk count: {session}"
        );
    }
}

#[test]
fn agent_rag_query_indexes_source_project_chunks() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-rag-source/source-rag-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("source RAG DB parent"))
        .expect("create source RAG DB parent");
    let source_path = workspace_path("samples/agent-script/native-choice-dispatch.arcw");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("query")
        .arg("--source")
        .arg(&source_path)
        .arg("--debug-db")
        .arg(&db_path)
        .arg("--query")
        .arg("choice.opening")
        .arg("--root")
        .arg("choice.opening.listen")
        .arg("--json")
        .output()
        .expect("arcw agent rag query indexes source");
    assert!(
        output.status.success(),
        "agent rag query should index source/project chunks\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pack: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("source RAG output is JSON");
    assert_eq!(pack["query"]["text"], "choice.opening");
    let items = pack["items"].as_array().expect("items array");
    assert!(
        items.iter().any(|item| {
            item["kind"] == "symbol"
                && item["entity_ids"]
                    .as_array()
                    .is_some_and(|ids| ids.contains(&serde_json::json!("choice.opening.listen")))
                && item["channels"]
                    .as_array()
                    .is_some_and(|channels| channels.contains(&serde_json::json!("exact_entity")))
        }),
        "source RAG should return rooted project symbol chunk: {pack}"
    );
    let search_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(&db_path)
        .arg("--query")
        .arg("choice.opening.listen")
        .arg("--json")
        .output()
        .expect("arcw debug db search reads source RAG chunks");
    assert!(
        search_output.status.success(),
        "debug db search should find source RAG chunks\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&search_output.stdout),
        String::from_utf8_lossy(&search_output.stderr)
    );
    let search: serde_json::Value =
        serde_json::from_slice(&search_output.stdout).expect("debug db search JSON");
    assert!(
        search["hits"].as_array().is_some_and(|hits| {
            hits.iter().any(|hit| {
                hit["source_kind"] == "symbol"
                    && hit["chunk_id"]
                        .as_str()
                        .is_some_and(|id| id.contains("choice.opening.listen"))
            })
        }),
        "debug db search should expose persisted project symbol chunks: {search}"
    );
    assert_debug_db_search_exposes_persisted_source_chunks(&db_path);

    assert_debug_db_rag_reads_source_project_chunks(&db_path, &pack);

    let _ = fs::remove_file(&db_path);
    let _ = fs::remove_file(db_path.with_extension("sqlite3-shm"));
    let _ = fs::remove_file(db_path.with_extension("sqlite3-wal"));
}

fn assert_agent_rag_query_reads_persisted_source_index(db_path: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("query")
        .arg("--debug-db")
        .arg(db_path)
        .arg("--query")
        .arg("choice.opening")
        .arg("--root")
        .arg("choice.opening.listen")
        .arg("--json")
        .output()
        .expect("arcw agent rag query reads persisted source index");
    assert!(
        output.status.success(),
        "agent rag query should read persisted source index\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pack: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("persisted source index RAG output is JSON");
    assert_eq!(pack["query"]["text"], "choice.opening");
    assert!(
        pack["items"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["kind"] == "symbol"
                    && item["entity_ids"].as_array().is_some_and(|ids| {
                        ids.contains(&serde_json::json!("choice.opening.listen"))
                    })
                    && item["channels"].as_array().is_some_and(|channels| {
                        channels.contains(&serde_json::json!("exact_entity"))
                    })
            })
        }),
        "agent rag query should return rooted persisted project symbol chunk: {pack}"
    );
}

fn assert_debug_db_search_exposes_persisted_source_chunks(db_path: &Path) {
    let search_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(db_path)
        .arg("--query")
        .arg("signal.set")
        .arg("--json")
        .output()
        .expect("arcw debug db search reads persisted source chunks");
    assert!(
        search_output.status.success(),
        "debug db search should find source RAG chunks\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&search_output.stdout),
        String::from_utf8_lossy(&search_output.stderr)
    );
    let search: serde_json::Value =
        serde_json::from_slice(&search_output.stdout).expect("debug db source search JSON");
    assert!(
        search["hits"].as_array().is_some_and(|hits| {
            hits.iter().any(|hit| {
                hit["source_kind"] == "source"
                    && hit["body"]
                        .as_str()
                        .is_some_and(|body| body.contains("signal.set"))
            })
        }),
        "debug db search should expose persisted source text chunks: {search}"
    );
}

fn assert_debug_db_rag_reads_source_project_chunks(db_path: &Path, pack: &serde_json::Value) {
    let rag_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("rag")
        .arg("--path")
        .arg(db_path)
        .arg("--query-id")
        .arg(
            pack["query"]["query_id"]
                .as_str()
                .expect("RAG pack has query id"),
        )
        .arg("--json")
        .output()
        .expect("arcw debug db rag reads persisted RAG audit");
    assert!(
        rag_output.status.success(),
        "debug db rag should read persisted RAG audit\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rag_output.stdout),
        String::from_utf8_lossy(&rag_output.stderr)
    );
    let rag: serde_json::Value =
        serde_json::from_slice(&rag_output.stdout).expect("debug db rag output is JSON");
    assert_eq!(rag["query_id"], pack["query"]["query_id"]);
    assert_eq!(rag["max_privacy"], "project");
    assert_eq!(rag["pack"]["query"]["text"], "choice.opening");
    assert!(
        rag["pack"]["items"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["kind"] == "symbol"
                    && item["entity_ids"].as_array().is_some_and(|ids| {
                        ids.contains(&serde_json::json!("choice.opening.listen"))
                    })
            })
        }),
        "debug db rag should reconstruct selected persisted RAG items: {rag}"
    );
    assert_agent_rag_explain_reads_source_project_chunks(db_path, pack);
    assert_agent_rag_context_read_reads_one_persisted_chunk(db_path, pack);
}

fn assert_agent_rag_explain_reads_source_project_chunks(db_path: &Path, pack: &serde_json::Value) {
    let explain_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("explain")
        .arg(
            pack["query"]["query_id"]
                .as_str()
                .expect("RAG pack has query id"),
        )
        .arg("--debug-db")
        .arg(db_path)
        .arg("--json")
        .output()
        .expect("arcw agent rag explain reads persisted RAG audit");
    assert!(
        explain_output.status.success(),
        "agent rag explain should read persisted audit\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&explain_output.stdout),
        String::from_utf8_lossy(&explain_output.stderr)
    );
    let explain: serde_json::Value =
        serde_json::from_slice(&explain_output.stdout).expect("agent rag explain output is JSON");

    assert_eq!(explain["query_id"], pack["query"]["query_id"]);
    assert_eq!(explain["query"]["text"], "choice.opening");
    assert_eq!(explain["max_privacy"], "project");
    assert_eq!(
        explain["item_count"].as_u64(),
        pack["items"].as_array().map(|items| items.len() as u64)
    );
    assert!(
        explain["items"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["kind"] == "symbol"
                    && item.get("body").is_none()
                    && item["entity_ids"].as_array().is_some_and(|ids| {
                        ids.contains(&serde_json::json!("choice.opening.listen"))
                    })
            })
        }),
        "agent rag explain should expose metadata without inlining bodies: {explain}"
    );
}

fn assert_agent_rag_context_read_reads_one_persisted_chunk(
    db_path: &Path,
    pack: &serde_json::Value,
) {
    let chunk_id = pack["items"]
        .as_array()
        .expect("RAG items")
        .iter()
        .find(|item| item["kind"] == "symbol")
        .and_then(|item| item["chunk_id"].as_str())
        .expect("symbol chunk id");
    let read_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("context-read")
        .arg(
            pack["query"]["query_id"]
                .as_str()
                .expect("RAG pack has query id"),
        )
        .arg(chunk_id)
        .arg("--debug-db")
        .arg(db_path)
        .arg("--max-bytes")
        .arg("48")
        .arg("--json")
        .output()
        .expect("arcw agent rag context-read reads one persisted chunk");
    assert!(
        read_output.status.success(),
        "agent rag context-read should read one persisted chunk\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&read_output.stdout),
        String::from_utf8_lossy(&read_output.stderr)
    );
    let read: serde_json::Value =
        serde_json::from_slice(&read_output.stdout).expect("agent rag context-read output is JSON");

    assert_eq!(read["query_id"], pack["query"]["query_id"]);
    assert_eq!(read["chunk_id"], chunk_id);
    assert_eq!(read["max_bytes"], 48);
    assert!(
        read["item"]["body"]
            .as_str()
            .is_some_and(|body| !body.is_empty() && body.len() <= 48),
        "agent rag context-read should return capped body: {read}"
    );
}

#[test]
fn agent_rag_query_indexes_multiple_sources() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-rag-source/multi-source-rag-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("multi-source RAG DB parent"))
        .expect("create multi-source RAG DB parent");
    let choice_source = workspace_path("samples/agent-script/native-choice-dispatch.arcw");
    let rich_text_source = workspace_path("samples/rich-text-showcase.arcw");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("query")
        .arg("--source")
        .arg(&choice_source)
        .arg("--source")
        .arg(&rich_text_source)
        .arg("--debug-db")
        .arg(&db_path)
        .arg("--query")
        .arg("choice.opening rich_text")
        .arg("--limit")
        .arg("12")
        .arg("--json")
        .output()
        .expect("arcw agent rag query indexes multiple sources");
    assert!(
        output.status.success(),
        "agent rag query should index repeated --source inputs\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pack: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("multi-source RAG output is JSON");
    assert_eq!(pack["query"]["text"], "choice.opening rich_text");
    assert!(
        pack["items"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["chunk_id"]
                    .as_str()
                    .is_some_and(|id| id.contains("source.blake3:"))
                    && item["source_anchor"]["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with("rich-text-showcase.arcw"))
            })
        }),
        "multi-source RAG should namespace source/project chunks by source input: {pack}"
    );
    assert_debug_db_graph_reports_multi_source_program_root(&db_path, &pack);
    assert_debug_db_rag_query_uses_program_summary(&db_path);

    for query in ["choice.opening.listen", "flow.rich_text_showcase"] {
        let search_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("debug")
            .arg("db")
            .arg("search")
            .arg("--path")
            .arg(&db_path)
            .arg("--query")
            .arg(query)
            .arg("--json")
            .output()
            .expect("arcw debug db search reads multi-source RAG chunks");
        assert!(
            search_output.status.success(),
            "debug db search should find multi-source RAG chunks for {query}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&search_output.stdout),
            String::from_utf8_lossy(&search_output.stderr)
        );
        let search: serde_json::Value =
            serde_json::from_slice(&search_output.stdout).expect("multi-source search JSON");
        assert!(
            search["hits"].as_array().is_some_and(|hits| {
                hits.iter().any(|hit| {
                    hit["source_kind"] == "symbol"
                        && hit["chunk_id"]
                            .as_str()
                            .is_some_and(|id| id.contains(query))
                })
            }),
            "debug db search should expose persisted project symbol chunks for {query}: {search}"
        );
    }

    let _ = fs::remove_file(&db_path);
    let _ = fs::remove_file(db_path.with_extension("sqlite3-shm"));
    let _ = fs::remove_file(db_path.with_extension("sqlite3-wal"));
}

fn assert_debug_db_graph_reports_multi_source_program_root(
    db_path: &Path,
    pack: &serde_json::Value,
) {
    let graph_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("graph")
        .arg("--path")
        .arg(db_path)
        .arg("--program-hash")
        .arg(
            pack["query"]["program_hash"]
                .as_str()
                .expect("program hash"),
        )
        .arg("--json")
        .output()
        .expect("arcw debug db graph reads multi-source graph");
    assert!(
        graph_output.status.success(),
        "debug db graph should read multi-source graph\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&graph_output.stdout),
        String::from_utf8_lossy(&graph_output.stderr)
    );
    let graph: serde_json::Value =
        serde_json::from_slice(&graph_output.stdout).expect("multi-source graph JSON");
    let symbols = graph["symbols"].as_array().expect("graph symbols");
    let edges = graph["edges"].as_array().expect("graph edges");

    let program = symbols
        .iter()
        .find(|symbol| symbol["kind"] == "program")
        .expect("multi-source graph should expose a combined program root");
    assert_eq!(program["metadata"]["source_count"], serde_json::json!(2));
    assert!(
        program["metadata"]["candidate_chunk_count"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "combined program root should summarize indexed RAG chunks: {graph}"
    );
    assert!(
        program["metadata"]["source_byte_count"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "combined program root should summarize indexed source bytes: {graph}"
    );
    assert!(
        program["metadata"]["source_graph_symbol_count"]
            .as_u64()
            .is_some_and(|count| count >= 2),
        "combined program root should summarize source graph symbols: {graph}"
    );
    assert!(
        program["metadata"]["source_graph_edge_count"]
            .as_u64()
            .is_some_and(|count| count >= 2),
        "combined program root should summarize source graph edges: {graph}"
    );
    assert_eq!(
        program["metadata"]["source_graph_symbol_kinds"]["source_file"],
        serde_json::json!(2),
        "combined program root should summarize graph symbol kinds: {graph}"
    );
    assert_eq!(
        program["metadata"]["source_graph_edge_kinds"]["contains_project_graph"],
        serde_json::json!(2),
        "combined program root should summarize graph edge kinds: {graph}"
    );
    assert_eq!(
        symbols
            .iter()
            .filter(|symbol| symbol["kind"] == "source_file")
            .count(),
        2,
        "multi-source graph should expose both source-file symbols: {graph}"
    );
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge["edge_kind"] == "contains_source_file")
            .count(),
        2,
        "multi-source graph should connect program root to both source files: {graph}"
    );
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge["edge_kind"] == "contains_project_graph")
            .count(),
        2,
        "multi-source graph should connect both source files to project graph slices: {graph}"
    );
}

fn assert_debug_db_rag_query_uses_program_summary(db_path: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("query")
        .arg("--debug-db")
        .arg(db_path)
        .arg("--query")
        .arg("program_rag_index")
        .arg("--limit")
        .arg("4")
        .arg("--json")
        .output()
        .expect("arcw agent rag query reads persisted program RAG summary");
    assert!(
        output.status.success(),
        "agent rag query should read persisted program RAG summary\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pack: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("program summary RAG query JSON");
    assert!(
        pack["items"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["kind"] == "graph_summary"
                    && item["chunk_id"]
                        .as_str()
                        .is_some_and(|id| id.contains("cli:program."))
                    && item["body"].as_str().is_some_and(|body| {
                        body.contains("\"program_rag_index\"")
                            && body.contains("\"source_graph_symbol_kinds\"")
                            && body.contains("\"source_graph_edge_kinds\"")
                            && body.contains("\"graph_symbol_kinds\"")
                            && body.contains("\"graph_edge_kinds\"")
                            && body.contains("\"flow_control_counts\"")
                            && body.contains("\"flow_control_symbols\"")
                            && body.contains("\"project_summary\"")
                            && body.contains("\"entity_kind_counts\"")
                            && body.contains("\"relation_kind_counts\"")
                            && body.contains("\"dependency_edge_kind_counts\"")
                    })
            })
        }),
        "persisted debug DB query should surface the program-level RAG summary: {pack}"
    );
}

#[test]
fn agent_rag_query_indexes_source_directories() {
    let directory_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("query")
        .arg("--source")
        .arg(workspace_path("samples/agent-script"))
        .arg("--query")
        .arg("signal.current_flow")
        .arg("--root")
        .arg("signal.current_flow")
        .arg("--json")
        .output()
        .expect("arcw agent rag query indexes a source directory");
    assert!(
        directory_output.status.success(),
        "agent rag query should index .arcw files under a source directory\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&directory_output.stdout),
        String::from_utf8_lossy(&directory_output.stderr)
    );
    let directory_pack: serde_json::Value =
        serde_json::from_slice(&directory_output.stdout).expect("directory RAG output is JSON");
    assert!(
        directory_pack["items"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["kind"] == "symbol"
                    && item["entity_ids"]
                        .as_array()
                        .is_some_and(|ids| ids.contains(&serde_json::json!("signal.current_flow")))
                    && item["source_anchor"]["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with("native-project-index.arcw"))
            })
        }),
        "directory source RAG should expand .arcw files and return rooted symbols: {directory_pack}"
    );
}

#[test]
fn debug_db_sessions_reports_persisted_product_sessions() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/sessions-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug sessions target dir"))
        .expect("create debug sessions target dir");
    let store = DebugStore::open(&db_path).expect("open debug sessions database");
    let session_id = SessionId::new("session.product").expect("session id");
    let mut metadata = BTreeMap::new();
    metadata.insert("target".to_owned(), serde_json::json!("native-player"));
    metadata.extend(project_shape_metadata(2, 3, 2, 1));
    store
        .upsert_session(&DebugSession {
            session_id: session_id.clone(),
            program_hash: None,
            profile: "developer".to_owned(),
            transport: "native".to_owned(),
            started_unix_ms: 10,
            ended_unix_ms: None,
            status: DebugSessionStatus::Running,
            metadata,
        })
        .expect("seed session");
    let mut finished_metadata = BTreeMap::new();
    finished_metadata.insert("outcome".to_owned(), serde_json::json!("done"));
    finished_metadata.extend(project_shape_metadata(2, 3, 2, 1));
    store
        .finish_session(
            &session_id,
            DebugSessionStatus::Finished,
            25,
            &finished_metadata,
        )
        .expect("finish session");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("sessions")
        .arg("--path")
        .arg(&db_path)
        .arg("--limit")
        .arg("4")
        .arg("--json")
        .output()
        .expect("arcw debug db sessions runs");
    assert!(
        output.status.success(),
        "debug db sessions should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db sessions output is JSON");

    assert_eq!(report["limit"], 4);
    assert_eq!(report["max_privacy"], "project");
    assert_eq!(report["sessions"][0]["session_id"], "session.product");
    assert_eq!(report["sessions"][0]["profile"], "developer");
    assert_eq!(report["sessions"][0]["transport"], "native");
    assert_eq!(report["sessions"][0]["status"], "finished");
    assert_eq!(report["sessions"][0]["ended_unix_ms"], 25);
    assert_eq!(report["sessions"][0]["metadata"]["outcome"], "done");
    assert_eq!(report["sessions"][0]["project"]["entity_count"], 2);
    assert_eq!(
        report["sessions"][0]["project"]["entity_kind_counts"]["flow"],
        2
    );
    assert_eq!(report["sessions"][0]["project"]["graph_symbol_count"], 3);
    assert_eq!(report["sessions"][0]["project"]["graph_edge_count"], 2);
    assert_eq!(
        report["sessions"][0]["project"]["graph_summary_symbol_id"],
        "project:summary"
    );
    assert_eq!(
        report["sessions"][0]["project"]["project_summary"]["agent_action_count"],
        1
    );

    assert_debug_db_sessions_public_privacy(&db_path);
}

fn assert_debug_db_sessions_public_privacy(db_path: &Path) {
    let public_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("sessions")
        .arg("--path")
        .arg(db_path)
        .arg("--limit")
        .arg("4")
        .arg("--max-privacy")
        .arg("public")
        .arg("--json")
        .output()
        .expect("arcw debug db sessions public runs");
    assert!(
        public_output.status.success(),
        "debug db sessions public should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&public_output.stdout),
        String::from_utf8_lossy(&public_output.stderr)
    );
    let public_report: serde_json::Value = serde_json::from_slice(&public_output.stdout)
        .expect("debug db sessions public output is JSON");
    assert_eq!(public_report["max_privacy"], "public");
    assert_eq!(
        public_report["sessions"][0]["session_id"],
        "session.product"
    );
    assert!(public_report["sessions"][0]["project"].is_null());
    assert_eq!(
        public_report["sessions"][0]["metadata"]
            .as_object()
            .map(serde_json::Map::len),
        Some(0),
        "project-private session metadata should be omitted at public ceiling"
    );
}

fn project_shape_metadata(
    entity_count: u64,
    graph_symbol_count: u64,
    graph_edge_count: u64,
    agent_action_count: u64,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        (
            "project_entities".to_owned(),
            serde_json::json!({
                "count": entity_count,
                "kind_counts": { "flow": entity_count }
            }),
        ),
        (
            "project_graph".to_owned(),
            serde_json::json!({
                "symbol_count": graph_symbol_count,
                "edge_count": graph_edge_count,
                "summary_symbol_id": "project:summary",
                "has_project_summary": true,
                "symbol_kind_counts": {
                    "project_summary": 1,
                    "flow": graph_symbol_count.saturating_sub(1)
                },
                "edge_kind_counts": { "contains_entity": graph_edge_count },
                "project_summary": {
                    "entity_count": entity_count,
                    "relation_count": graph_edge_count,
                    "agent_action_count": agent_action_count,
                    "dynamic_control_flow_count": 0
                }
            }),
        ),
    ])
}

#[test]
fn debug_db_close_stale_sessions_abandons_old_running_sessions() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/close-stale-sessions-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug stale sessions target dir"))
        .expect("create debug stale sessions target dir");
    let store = DebugStore::open(&db_path).expect("open debug sessions database");
    let old = SessionId::new("session.old-running").expect("old session id");
    let fresh = SessionId::new("session.fresh-running").expect("fresh session id");
    store
        .start_session(&old, None, "developer", "cli", 0)
        .expect("seed old running session");
    store
        .start_session(&fresh, None, "developer", "cli", i64::MAX / 2)
        .expect("seed fresh running session");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("close-stale-sessions")
        .arg("--path")
        .arg(&db_path)
        .arg("--stale-after")
        .arg("1ms")
        .arg("--reason")
        .arg("test-stale-close")
        .arg("--json")
        .output()
        .expect("arcw debug db close-stale-sessions runs");
    assert!(
        output.status.success(),
        "debug db close-stale-sessions should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("debug db close-stale-sessions output is JSON");
    assert_eq!(report["dry_run"], false);
    assert_eq!(report["matched_sessions"].as_array().map(Vec::len), Some(1));
    assert_eq!(report["closed_sessions"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        report["closed_sessions"][0]["session_id"],
        "session.old-running"
    );
    assert_eq!(report["closed_sessions"][0]["status"], "abandoned");
    assert_eq!(
        report["closed_sessions"][0]["metadata"]["lifecycle_policy"]["reason"],
        "test-stale-close"
    );

    let old_session = store
        .session(&old)
        .expect("read old session")
        .expect("old session exists");
    assert_eq!(old_session.status, DebugSessionStatus::Abandoned);
    let fresh_session = store
        .session(&fresh)
        .expect("read fresh session")
        .expect("fresh session exists");
    assert_eq!(fresh_session.status, DebugSessionStatus::Running);
}

#[test]
fn debug_db_timeline_reports_privacy_filtered_events() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/timeline-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug timeline target dir"))
        .expect("create debug timeline target dir");
    let mut store = DebugStore::open(&db_path).expect("open debug timeline database");
    let session_id = SessionId::new("session.timeline.cli").expect("session id");
    store
        .start_session(&session_id, None, "developer", "cli", 0)
        .expect("seed timeline session");
    for (sequence, privacy, message) in [
        (1, "secret", "hidden event"),
        (2, "public", "visible event"),
    ] {
        store
            .append(&DebugEvent {
                schema_version: 1,
                session_id: session_id.clone(),
                run_id: None,
                sequence,
                tick: Some(sequence + 10),
                kind: DebugEventKind::Diagnostic,
                payload: serde_json::json!({
                    "privacy_class": privacy,
                    "message": message,
                }),
                created_unix_ms: i64::try_from(sequence).expect("sequence fits i64"),
            })
            .expect("seed timeline event");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("timeline")
        .arg("--path")
        .arg(&db_path)
        .arg("--session-id")
        .arg(session_id.as_str())
        .arg("--limit")
        .arg("1")
        .arg("--max-privacy")
        .arg("public")
        .arg("--json")
        .output()
        .expect("arcw debug db timeline runs");
    assert!(
        output.status.success(),
        "debug db timeline should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db timeline output is JSON");

    assert_eq!(report["session_id"], "session.timeline.cli");
    assert_eq!(report["limit"], 1);
    assert_eq!(report["max_privacy"], "public");
    let events = report["events"].as_array().expect("events array");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["sequence"], 2);
    assert_eq!(events[0]["kind"], "diagnostic");
    assert_eq!(events[0]["privacy"], "public");
    assert_eq!(events[0]["payload"]["message"], "visible event");
}

#[test]
fn debug_db_repl_cells_reports_persisted_cells() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/repl-cells-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug repl cells target dir"))
        .expect("create debug repl cells target dir");
    let store = DebugStore::open(&db_path).expect("open debug repl cells database");
    let session_id = SessionId::new("session.repl.cli").expect("session id");
    store
        .start_session(&session_id, None, "repl", "cli", 0)
        .expect("seed REPL session");
    for (ordinal, source) in [(1, "let observed = observe()"), (2, ":bindings")] {
        store
            .upsert_repl_cell(&DebugReplCell {
                cell_id: format!("repl:{}:{ordinal}", session_id.as_str()),
                session_id: session_id.clone(),
                run_id: None,
                ordinal,
                source: source.to_owned(),
                source_hash: StableHash::new(format!("blake3:repl-cell-{ordinal}"))
                    .expect("source hash"),
                status: "ok".to_owned(),
                inferred_type: None,
                display: Some(serde_json::json!({ "ordinal": ordinal })),
                partially_effectful: ordinal == 1,
                diagnostic_ids: vec![format!("diag.{ordinal}")],
                created_unix_ms: ordinal,
            })
            .expect("seed REPL cell");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("repl-cells")
        .arg("--path")
        .arg(&db_path)
        .arg("--session-id")
        .arg(session_id.as_str())
        .arg("--limit")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw debug db repl-cells runs");
    assert!(
        output.status.success(),
        "debug db repl-cells should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db repl-cells output is JSON");

    assert_eq!(report["session_id"], "session.repl.cli");
    assert_eq!(report["limit"], 1);
    let cells = report["cells"].as_array().expect("cells array");
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0]["cell_id"], "repl:session.repl.cli:1");
    assert_eq!(cells[0]["ordinal"], 1);
    assert_eq!(cells[0]["source"], "let observed = observe()");
    assert_eq!(cells[0]["status"], "ok");
    assert_eq!(cells[0]["display"]["ordinal"], 1);
    assert_eq!(cells[0]["partially_effectful"], true);
    assert_eq!(cells[0]["diagnostic_ids"][0], "diag.1");
}

#[test]
fn debug_db_vacuum_reports_page_counts() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/vacuum-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug vacuum target dir"))
        .expect("create debug vacuum target dir");
    seed_debug_search_db(&db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("vacuum")
        .arg("--path")
        .arg(&db_path)
        .arg("--json")
        .output()
        .expect("arcw debug db vacuum runs");
    assert!(
        output.status.success(),
        "debug db vacuum should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db vacuum output is JSON");

    assert_eq!(report["path"], db_path.display().to_string());
    assert!(report["page_count_before"].as_u64().expect("page count") > 0);
    assert!(report["page_count_after"].as_u64().expect("page count") > 0);
    assert!(
        report["freelist_count_after"]
            .as_u64()
            .expect("freelist after")
            <= report["freelist_count_before"]
                .as_u64()
                .expect("freelist before")
    );
}

#[test]
fn debug_db_prune_removes_rows_older_than_duration() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/prune-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug prune target dir"))
        .expect("create debug prune target dir");
    seed_debug_prune_db(&db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("prune")
        .arg("--path")
        .arg(&db_path)
        .arg("--older-than")
        .arg("1d")
        .arg("--json")
        .output()
        .expect("arcw debug db prune runs");
    assert!(
        output.status.success(),
        "debug db prune should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db prune output is JSON");

    assert_eq!(report["path"], db_path.display().to_string());
    assert_eq!(report["older_than_millis"], 86_400_000);
    assert_eq!(report["deleted"]["sessions"], 1);
    assert_eq!(report["deleted"]["chunks"], 1);
    assert_eq!(report["deleted"]["programs"], 1);
    assert_eq!(report["stats_after"]["sessions"], 1);
    assert_eq!(report["stats_after"]["chunks"], 1);
    assert_eq!(report["stats_after"]["programs"], 1);

    let store = DebugStore::open(&db_path).expect("open pruned debug db");
    let hits = store
        .lexical_search_with_max_privacy("retention", 10, PrivacyClass::Project)
        .expect("search pruned chunks");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].hit.chunk_id.as_str(), "chunk:new-retention");
}

#[test]
fn debug_db_search_filters_chunks_by_privacy() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/search-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug search target dir"))
        .expect("create debug search target dir");
    seed_debug_search_db(&db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(&db_path)
        .arg("--query")
        .arg("opening")
        .arg("--limit")
        .arg("4")
        .arg("--max-privacy")
        .arg("public")
        .arg("--json")
        .output()
        .expect("arcw debug db search runs");
    assert!(
        output.status.success(),
        "debug db search should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db search output is JSON");

    assert_eq!(report["query"], "opening");
    assert_eq!(report["max_privacy"], "public");
    let hits = report["hits"].as_array().expect("hits array");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["chunk_id"], "chunk:public-opening");
    assert_eq!(hits[0]["privacy"], "public");
    assert_eq!(hits[0]["channel"], "lexical");
}

#[test]
fn debug_db_search_vector_filters_chunks_by_privacy() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/vector-search-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug vector search target dir"))
        .expect("create debug vector search target dir");
    seed_debug_search_db(&db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(&db_path)
        .arg("--query-vector")
        .arg("1.0,0.0")
        .arg("--model-id")
        .arg("fixture")
        .arg("--model-revision")
        .arg("1")
        .arg("--limit")
        .arg("1")
        .arg("--max-privacy")
        .arg("public")
        .arg("--json")
        .output()
        .expect("arcw debug db vector search runs");
    assert!(
        output.status.success(),
        "debug db vector search should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db vector search output is JSON");

    assert_eq!(report["query"], serde_json::Value::Null);
    assert_eq!(report["query_vector_dimensions"], 2);
    assert_eq!(report["model"]["model_id"], "fixture");
    assert_eq!(report["model"]["model_revision"], "1");
    assert_eq!(report["model"]["dimensions"], 2);
    assert_eq!(report["max_privacy"], "public");
    let hits = report["hits"].as_array().expect("hits array");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["chunk_id"], "chunk:public-opening");
    assert_eq!(hits[0]["privacy"], "public");
    assert_eq!(hits[0]["channel"], "vector");
}

#[test]
fn debug_db_embed_indexes_privacy_filtered_local_hash_embeddings() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/embed-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug embed target dir"))
        .expect("create debug embed target dir");
    seed_debug_embed_db(&db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("embed")
        .arg("--path")
        .arg(&db_path)
        .arg("--model-id")
        .arg("fixture-local-hash")
        .arg("--model-revision")
        .arg("1")
        .arg("--dimensions")
        .arg("8")
        .arg("--max-privacy")
        .arg("sensitive")
        .arg("--json")
        .output()
        .expect("arcw debug db embed runs");
    assert!(
        output.status.success(),
        "debug db embed should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db embed output is JSON");

    assert_eq!(report["provider"], "local_hash");
    assert_eq!(report["scope"], "local");
    assert_eq!(report["model"]["model_id"], "fixture-local-hash");
    assert_eq!(report["model"]["model_revision"], "1");
    assert_eq!(report["model"]["dimensions"], 8);
    assert_eq!(report["max_privacy"], "sensitive");
    assert_eq!(report["input_chunks"], 3);
    assert_eq!(report["embedded_chunks"], 3);
    assert_eq!(report["skipped_chunks"], 1);
    assert_eq!(report["stats_after"]["embeddings"], 3);
    let embedded = report["embedded_chunk_ids"]
        .as_array()
        .expect("embedded chunk ids");
    assert_eq!(embedded.len(), 3);
    assert!(embedded.contains(&serde_json::json!("chunk:embed-public")));
    assert!(embedded.contains(&serde_json::json!("chunk:embed-project")));
    assert!(embedded.contains(&serde_json::json!("chunk:embed-sensitive")));
    assert!(!embedded.contains(&serde_json::json!("chunk:embed-secret")));

    let store = DebugStore::open(&db_path).expect("open embedded debug db");
    let embeddings = store
        .load_embeddings(&EmbeddingModelDescriptor {
            model_id: "fixture-local-hash".to_owned(),
            model_revision: "1".to_owned(),
            dimensions: 8,
        })
        .expect("load local hash embeddings");
    assert_eq!(embeddings.len(), 3);
    assert!(
        embeddings
            .iter()
            .all(|embedding| embedding.values.len() == 8
                && embedding.values.iter().all(|value| value.is_finite()))
    );
    assert!(
        embeddings
            .iter()
            .all(|embedding| embedding.chunk_id.as_str() != "chunk:embed-secret")
    );
}

#[test]
fn debug_db_embed_records_remote_provider_unavailable_diagnostic() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/embed-remote-unavailable-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug remote embed target dir"))
        .expect("create debug remote embed target dir");
    seed_debug_embed_db(&db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("embed")
        .arg("--path")
        .arg(&db_path)
        .arg("--provider")
        .arg("remote")
        .arg("--model-id")
        .arg("fixture-remote")
        .arg("--model-revision")
        .arg("2026-06-20")
        .arg("--dimensions")
        .arg("8")
        .arg("--max-privacy")
        .arg("secret")
        .arg("--json")
        .output()
        .expect("arcw debug db remote embed runs");
    assert!(
        !output.status.success(),
        "remote embed should fail until a real provider adapter is configured\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("AGENT_DEBUG_EMBEDDING_PROVIDER_UNAVAILABLE"),
        "stderr should mention recorded provider diagnostic\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let diagnostic_report = debug_db_search_json(
        &db_path,
        "--diagnostic-query",
        "AGENT_DEBUG_EMBEDDING_PROVIDER_UNAVAILABLE",
        "project",
    );
    let hits = diagnostic_report["hits"].as_array().expect("hits");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["channel"], "diagnostics");
    assert_eq!(hits[0]["source_kind"], "diagnostic");
    assert!(
        hits[0]["body"]
            .as_str()
            .is_some_and(|body| body.contains("remote embedding provider is not configured")),
        "diagnostic body should explain unavailable remote provider: {diagnostic_report}"
    );

    let store = DebugStore::open(&db_path).expect("open remote embed debug db");
    let embeddings = store
        .load_embeddings(&EmbeddingModelDescriptor {
            model_id: "fixture-remote".to_owned(),
            model_revision: "2026-06-20".to_owned(),
            dimensions: 8,
        })
        .expect("load remote embeddings");
    assert!(
        embeddings.is_empty(),
        "remote provider failure must not synthesize stored embeddings"
    );
}

#[test]
fn debug_db_embed_remote_command_indexes_provider_vectors() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/embed-remote-command-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(
        db_path
            .parent()
            .expect("debug remote command embed target dir"),
    )
    .expect("create debug remote command embed target dir");
    seed_debug_embed_db(&db_path);
    let (remote_command, remote_args) = write_remote_embedding_fixture_provider(
        &format!("embed-remote-command-{}", std::process::id()),
        r#"{"embeddings":[{"chunk_id":"chunk:embed-public","values":[1.0,0.0,0.0,0.0]},{"chunk_id":"chunk:embed-project","values":[0.0,2.0,0.0,0.0]}]}"#,
    );

    let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
    command
        .arg("debug")
        .arg("db")
        .arg("embed")
        .arg("--path")
        .arg(&db_path)
        .arg("--provider")
        .arg("remote")
        .arg("--remote-command")
        .arg(remote_command)
        .arg("--model-id")
        .arg("fixture-remote")
        .arg("--model-revision")
        .arg("2026-06-20")
        .arg("--dimensions")
        .arg("4")
        .arg("--max-privacy")
        .arg("secret")
        .arg("--json");
    for remote_arg in remote_args {
        command.arg("--remote-arg").arg(remote_arg);
    }
    let output = command.output().expect("arcw debug db remote embed runs");
    assert!(
        output.status.success(),
        "remote command embed should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db remote embed output is JSON");

    assert_eq!(report["provider"], "remote");
    assert_eq!(report["scope"], "remote");
    assert_eq!(report["input_chunks"], 2);
    assert_eq!(report["embedded_chunks"], 2);
    assert_eq!(report["skipped_chunks"], 2);
    assert_eq!(report["stats_after"]["embeddings"], 2);
    let embedded = report["embedded_chunk_ids"]
        .as_array()
        .expect("embedded chunk ids");
    assert!(embedded.contains(&serde_json::json!("chunk:embed-public")));
    assert!(embedded.contains(&serde_json::json!("chunk:embed-project")));
    assert!(!embedded.contains(&serde_json::json!("chunk:embed-sensitive")));
    assert!(!embedded.contains(&serde_json::json!("chunk:embed-secret")));

    let store = DebugStore::open(&db_path).expect("open remote command debug db");
    let embeddings = store
        .load_embeddings(&EmbeddingModelDescriptor {
            model_id: "fixture-remote".to_owned(),
            model_revision: "2026-06-20".to_owned(),
            dimensions: 4,
        })
        .expect("load remote command embeddings");
    assert_eq!(embeddings.len(), 2);
    assert!(
        embeddings.iter().all(
            |embedding| embedding.chunk_id.as_str() != "chunk:embed-sensitive"
                && embedding.chunk_id.as_str() != "chunk:embed-secret"
        )
    );
}

fn write_remote_embedding_fixture_provider(name: &str, response: &str) -> (String, Vec<String>) {
    let dir = workspace_path(&format!(
        "target/codex-agent-debug-search-test/{name}-provider"
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create remote embedding fixture provider dir");
    if cfg!(windows) {
        let script = dir.join("provider.cmd");
        fs::write(
            &script,
            format!("@echo off\r\nmore >NUL\r\necho {response}\r\n"),
        )
        .expect("write remote embedding fixture provider cmd");
        (
            "cmd".to_owned(),
            vec!["/C".to_owned(), script.display().to_string()],
        )
    } else {
        let script = dir.join("provider.sh");
        fs::write(
            &script,
            format!("cat >/dev/null\nprintf '%s\\n' '{response}'\n"),
        )
        .expect("write remote embedding fixture provider shell");
        ("sh".to_owned(), vec![script.display().to_string()])
    }
}

#[test]
fn agent_rag_query_uses_local_embedding_debug_db_channel() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/rag-local-embedding-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("rag local embedding target dir"))
        .expect("create rag local embedding target dir");
    seed_debug_embed_db(&db_path);

    let embed_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("embed")
        .arg("--path")
        .arg(&db_path)
        .arg("--model-id")
        .arg("fixture-local-hash")
        .arg("--model-revision")
        .arg("1")
        .arg("--dimensions")
        .arg("8")
        .arg("--max-privacy")
        .arg("sensitive")
        .arg("--json")
        .output()
        .expect("arcw debug db embed runs");
    assert!(
        embed_output.status.success(),
        "debug db embed should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&embed_output.stdout),
        String::from_utf8_lossy(&embed_output.stderr)
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("query")
        .arg("--debug-db")
        .arg(&db_path)
        .arg("--query")
        .arg("sensitive embedding body")
        .arg("--local-embedding")
        .arg("--local-embedding-model-id")
        .arg("fixture-local-hash")
        .arg("--local-embedding-model-revision")
        .arg("1")
        .arg("--local-embedding-dimensions")
        .arg("8")
        .arg("--max-privacy")
        .arg("sensitive")
        .arg("--limit")
        .arg("2")
        .arg("--json")
        .output()
        .expect("arcw agent rag query local embedding runs");
    assert!(
        output.status.success(),
        "agent rag query should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pack: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent rag query output is JSON");
    let items = pack["items"].as_array().expect("rag items");
    assert!(
        items
            .iter()
            .any(|item| item["chunk_id"] == "chunk:embed-sensitive"
                && item["channels"]
                    .as_array()
                    .is_some_and(|channels| channels.contains(&serde_json::json!("vector")))),
        "expected sensitive chunk to include vector channel: {pack}"
    );
}

#[test]
fn agent_rag_query_records_local_embedding_fallback_diagnostic() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/rag-local-embedding-fallback-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(
        db_path
            .parent()
            .expect("rag local embedding fallback target dir"),
    )
    .expect("create rag local embedding fallback target dir");
    seed_debug_embed_db(&db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("rag")
        .arg("query")
        .arg("--debug-db")
        .arg(&db_path)
        .arg("--query")
        .arg("public embedding body")
        .arg("--local-embedding")
        .arg("--local-embedding-model-id")
        .arg("fixture-local-hash")
        .arg("--local-embedding-model-revision")
        .arg("1")
        .arg("--local-embedding-dimensions")
        .arg("8")
        .arg("--max-privacy")
        .arg("project")
        .arg("--limit")
        .arg("2")
        .arg("--json")
        .output()
        .expect("arcw agent rag query local embedding fallback runs");
    assert!(
        output.status.success(),
        "agent rag query should succeed with fallback\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let diagnostic_report = debug_db_search_json(
        &db_path,
        "--diagnostic-query",
        "AGENT_RAG_EMBEDDING_FALLBACK",
        "project",
    );
    let hits = diagnostic_report["hits"].as_array().expect("hits");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["channel"], "diagnostics");
    assert_eq!(hits[0]["source_kind"], "diagnostic");
    assert!(
        hits[0]["chunk_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("diagnostic:agent-rag-local-embedding-fallback:")),
        "diagnostic id should be stable-prefixed: {diagnostic_report}"
    );
    assert!(
        hits[0]["body"]
            .as_str()
            .is_some_and(|body| body.contains("using lexical fallback channels")),
        "diagnostic body should explain fallback: {diagnostic_report}"
    );
}

#[test]
fn debug_db_search_history_filters_chunks_by_privacy() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/history-search-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug history search target dir"))
        .expect("create debug history search target dir");
    seed_debug_search_db(&db_path);

    let public_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(&db_path)
        .arg("--history-query")
        .arg("opening")
        .arg("--limit")
        .arg("1")
        .arg("--max-privacy")
        .arg("public")
        .arg("--json")
        .output()
        .expect("arcw debug db history search public runs");
    assert!(public_output.status.success());
    let public_report: serde_json::Value =
        serde_json::from_slice(&public_output.stdout).expect("history public output is JSON");
    assert_eq!(public_report["hits"].as_array().map(Vec::len), Some(0));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(&db_path)
        .arg("--history-query")
        .arg("opening")
        .arg("--limit")
        .arg("1")
        .arg("--max-privacy")
        .arg("project")
        .arg("--json")
        .output()
        .expect("arcw debug db history search runs");
    assert!(
        output.status.success(),
        "debug db history search should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db history search output is JSON");

    assert_eq!(report["history_query"], "opening");
    assert_eq!(report["max_privacy"], "project");
    let hits = report["hits"].as_array().expect("hits array");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["chunk_id"], "history:history:opening-fix");
    assert_eq!(hits[0]["privacy"], "project");
    assert_eq!(hits[0]["channel"], "history");
}

#[test]
fn debug_db_search_diagnostics_and_tests_filter_by_privacy() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/diagnostic-test-search-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(
        db_path
            .parent()
            .expect("debug diagnostic search target dir"),
    )
    .expect("create debug diagnostic search target dir");
    seed_debug_search_db(&db_path);

    assert_debug_db_search_empty(&db_path, "--diagnostic-query", "glyph_wobble", "public");
    let diagnostic_report =
        debug_db_search_json(&db_path, "--diagnostic-query", "glyph_wobble", "project");
    assert_eq!(diagnostic_report["diagnostic_query"], "glyph_wobble");
    assert_debug_db_search_single_hit(
        &diagnostic_report,
        "diagnostic:diag:missing-shader",
        "diagnostics",
        "diagnostic",
    );

    assert_debug_db_search_empty(
        &db_path,
        "--test-query",
        "rich-text-visual-regression",
        "public",
    );
    let test_report = debug_db_search_json(
        &db_path,
        "--test-query",
        "rich-text-visual-regression",
        "project",
    );
    assert_eq!(test_report["test_query"], "rich-text-visual-regression");
    assert_debug_db_search_single_hit(
        &test_report,
        "test_result:test:visual-regression",
        "diagnostics",
        "test_result",
    );
}

fn debug_db_search_json(
    db_path: &Path,
    selector: &str,
    query: &str,
    max_privacy: &str,
) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(db_path)
        .arg(selector)
        .arg(query)
        .arg("--limit")
        .arg("1")
        .arg("--max-privacy")
        .arg(max_privacy)
        .arg("--json")
        .output()
        .unwrap_or_else(|error| panic!("arcw debug db search runs: {error}"));
    assert!(
        output.status.success(),
        "debug db search should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("debug db search output is JSON")
}

fn assert_debug_db_search_empty(db_path: &Path, selector: &str, query: &str, max_privacy: &str) {
    let report = debug_db_search_json(db_path, selector, query, max_privacy);
    assert_eq!(report["hits"].as_array().map(Vec::len), Some(0));
}

fn assert_debug_db_search_single_hit(
    report: &serde_json::Value,
    chunk_id: &str,
    channel: &str,
    source_kind: &str,
) {
    let hits = report["hits"].as_array().expect("hits array");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["chunk_id"], chunk_id);
    assert_eq!(hits[0]["channel"], channel);
    assert_eq!(hits[0]["source_kind"], source_kind);
}

#[test]
fn debug_db_search_graph_filters_chunks_by_privacy() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/graph-search-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug graph search target dir"))
        .expect("create debug graph search target dir");
    seed_debug_search_db(&db_path);

    let public_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(&db_path)
        .arg("--graph-query")
        .arg("opening")
        .arg("--limit")
        .arg("1")
        .arg("--max-privacy")
        .arg("public")
        .arg("--json")
        .output()
        .expect("arcw debug db graph search public runs");
    assert!(public_output.status.success());
    let public_report: serde_json::Value =
        serde_json::from_slice(&public_output.stdout).expect("graph public output is JSON");
    assert_eq!(public_report["hits"].as_array().map(Vec::len), Some(0));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(&db_path)
        .arg("--graph-query")
        .arg("opening")
        .arg("--limit")
        .arg("1")
        .arg("--max-privacy")
        .arg("project")
        .arg("--json")
        .output()
        .expect("arcw debug db graph search runs");
    assert!(
        output.status.success(),
        "debug db graph search should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("debug db graph search output is JSON");

    assert_eq!(report["graph_query"], "opening");
    assert_eq!(report["max_privacy"], "project");
    let hits = report["hits"].as_array().expect("hits array");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["chunk_id"], "graph:1");
    assert_eq!(hits[0]["privacy"], "project");
    assert_eq!(hits[0]["channel"], "graph");
    assert_eq!(hits[0]["source_kind"], "graph_edge");

    let expanded_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("debug")
        .arg("db")
        .arg("search")
        .arg("--path")
        .arg(&db_path)
        .arg("--graph-query")
        .arg("opening")
        .arg("--graph-depth")
        .arg("2")
        .arg("--limit")
        .arg("10")
        .arg("--max-privacy")
        .arg("project")
        .arg("--json")
        .output()
        .expect("arcw debug db graph depth search runs");
    assert!(
        expanded_output.status.success(),
        "debug db graph depth search should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&expanded_output.stdout),
        String::from_utf8_lossy(&expanded_output.stderr)
    );
    let expanded_report: serde_json::Value =
        serde_json::from_slice(&expanded_output.stdout).expect("graph depth output is JSON");
    assert_eq!(expanded_report["graph_depth"], 2);
    let expanded_hits = expanded_report["hits"].as_array().expect("hits array");
    assert_eq!(expanded_hits.len(), 2);
    assert!(
        expanded_hits.iter().any(|hit| hit["chunk_id"] == "graph:2"
            && hit["body"]
                .as_str()
                .is_some_and(|body| body.contains("distance=2"))),
        "expanded graph search should include a 2-hop graph edge: {expanded_report}"
    );
}

#[test]
fn agent_rag_query_uses_debug_db_graph_and_history_channels() {
    let db_path = workspace_path(&format!(
        "target/codex-agent-debug-search-test/rag-fusion-{}.sqlite3",
        std::process::id()
    ));
    let _ = fs::remove_file(&db_path);
    fs::create_dir_all(db_path.parent().expect("debug RAG fusion target dir"))
        .expect("create debug RAG fusion target dir");
    seed_debug_search_db(&db_path);

    assert_agent_rag_debug_store_item(
        &db_path,
        "uses_textbox",
        Some("2"),
        "graph_summary",
        "graph:2",
        "graph",
    );
    assert_agent_rag_debug_store_item(
        &db_path,
        "change-opening-fix",
        None,
        "history",
        "history:history:opening-fix",
        "history",
    );
    assert_agent_rag_debug_store_item(
        &db_path,
        "glyph_wobble",
        None,
        "diagnostic",
        "diagnostic:diag:missing-shader",
        "diagnostics",
    );
    assert_agent_rag_debug_store_item(
        &db_path,
        "rich-text-visual-regression",
        None,
        "test_result",
        "test_result:test:visual-regression",
        "diagnostics",
    );
}

fn assert_agent_rag_debug_store_item(
    db_path: &Path,
    query: &str,
    graph_depth: Option<&str>,
    kind: &str,
    chunk_id: &str,
    channel: &str,
) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
    command
        .arg("agent")
        .arg("rag")
        .arg("query")
        .arg("--debug-db")
        .arg(db_path)
        .arg("--query")
        .arg(query)
        .arg("--limit")
        .arg("3")
        .arg("--json");
    if let Some(graph_depth) = graph_depth {
        command.arg("--graph-depth").arg(graph_depth);
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("arcw agent rag query `{query}` runs: {error}"));
    assert!(
        output.status.success(),
        "agent rag query should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pack: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("RAG output is JSON");
    assert!(
        pack["items"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["kind"] == kind
                    && item["chunk_id"] == chunk_id
                    && item["channels"]
                        .as_array()
                        .is_some_and(|channels| channels.contains(&serde_json::json!(channel)))
            })
        }),
        "agent rag query should return {kind} debug-store context: {pack}"
    );
}

fn seed_debug_search_db(path: &Path) {
    let store = DebugStore::open(path).expect("open debug search db");
    let program_hash = stable_hash("b3:debug-search-program");
    store
        .upsert_program(&program_hash, None, Some("."), 0)
        .expect("upsert debug search program");
    let model = EmbeddingModelDescriptor {
        model_id: "fixture".to_owned(),
        model_revision: "1".to_owned(),
        dimensions: 2,
    };
    for chunk in [
        DebugChunk {
            id: ChunkId::new("chunk:secret-opening"),
            program_hash: None,
            source_kind: ChunkSourceKind::Documentation,
            source_key: "secret".to_owned(),
            title: "opening secret".to_owned(),
            body: "opening secret investigation note".to_owned(),
            content_hash: stable_hash("b3:secret-opening"),
            semantic_hash: None,
            source_anchor: None,
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Secret,
            metadata: BTreeMap::default(),
            created_unix_ms: 0,
        },
        DebugChunk {
            id: ChunkId::new("chunk:public-opening"),
            program_hash: None,
            source_kind: ChunkSourceKind::Documentation,
            source_key: "public".to_owned(),
            title: "opening public".to_owned(),
            body: "opening public investigation note".to_owned(),
            content_hash: stable_hash("b3:public-opening"),
            semantic_hash: None,
            source_anchor: None,
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Public,
            metadata: BTreeMap::default(),
            created_unix_ms: 0,
        },
    ] {
        let vector = if chunk.privacy == PrivacyClass::Secret {
            vec![1.0, 0.0]
        } else {
            vec![0.9, 0.1]
        };
        let embedding = StoredEmbedding::normalized(
            chunk.id.clone(),
            model.clone(),
            vector,
            chunk.content_hash.as_str(),
            0,
        )
        .expect("debug search embedding");
        store.upsert_chunk(&chunk).expect("upsert debug chunk");
        store
            .upsert_embedding(&embedding)
            .expect("upsert debug embedding");
    }
    store
        .upsert_history_entry(&DebugHistoryEntry {
            history_id: "history:opening-fix".to_owned(),
            program_hash: None,
            symbol_id: None,
            change_id: "change-opening-fix".to_owned(),
            operation_id: Some("op.1".to_owned()),
            ordinal: 7,
            semantic_hash_before: None,
            semantic_hash_after: None,
            summary: "Fixed opening choice dispatch regression".to_owned(),
            metadata: BTreeMap::new(),
            created_unix_ms: 0,
        })
        .expect("upsert debug history");
    seed_debug_search_diagnostics(&store, program_hash.clone());
    seed_debug_search_graph(&store, program_hash);
}

fn seed_debug_embed_db(path: &Path) {
    let store = DebugStore::open(path).expect("open debug embed db");
    for (name, privacy) in [
        ("public", PrivacyClass::Public),
        ("project", PrivacyClass::Project),
        ("sensitive", PrivacyClass::Sensitive),
        ("secret", PrivacyClass::Secret),
    ] {
        store
            .upsert_chunk(&DebugChunk {
                id: ChunkId::new(format!("chunk:embed-{name}")),
                program_hash: None,
                source_kind: ChunkSourceKind::Documentation,
                source_key: format!("embed-{name}"),
                title: format!("{name} embedding title"),
                body: format!("{name} embedding body"),
                content_hash: stable_hash(&format!("b3:embed-{name}")),
                semantic_hash: None,
                source_anchor: None,
                entity_ids: Vec::new(),
                privacy,
                metadata: BTreeMap::default(),
                created_unix_ms: 0,
            })
            .expect("upsert debug embed chunk");
    }
}

fn seed_debug_search_diagnostics(store: &DebugStore, program_hash: StableHash) {
    store
        .upsert_diagnostic(&DebugDiagnostic {
            diagnostic_id: "diag:missing-shader".to_owned(),
            program_hash: Some(program_hash.clone()),
            session_id: None,
            run_id: None,
            sequence: Some(11),
            code: Some("RT_SHADER_MISSING".to_owned()),
            severity: "error".to_owned(),
            phase: "render".to_owned(),
            message: "missing shader binding for glyph wobble".to_owned(),
            source_path: Some("samples/rich-text-effects-animation.arcw".to_owned()),
            start_byte: Some(120),
            end_byte: Some(180),
            related_ids: vec![PublicId::new("@effect.wobble").expect("public id")],
            payload: serde_json::json!({ "shader": "glyph_wobble" }),
            created_unix_ms: 0,
        })
        .expect("upsert debug diagnostic");
    store
        .upsert_test_result(&DebugTestResult {
            test_result_id: "test:visual-regression".to_owned(),
            program_hash: Some(program_hash),
            run_id: None,
            test_id: "rich-text-visual-regression".to_owned(),
            kind: "visual".to_owned(),
            outcome: "failed".to_owned(),
            duration_millis: Some(57),
            diagnostic_ids: vec!["diag:missing-shader".to_owned()],
            artifact_refs: vec!["blob:rich-text-visual-diff".to_owned()],
            summary: "visual regression detected missing shader output".to_owned(),
            created_unix_ms: 0,
        })
        .expect("upsert debug test result");
}

fn seed_debug_search_graph(store: &DebugStore, program_hash: StableHash) {
    store
        .upsert_graph_symbol(&DebugGraphSymbol {
            symbol_id: "symbol:flow.opening".to_owned(),
            program_hash: program_hash.clone(),
            public_id: Some(
                arcweft_agent_protocol::ids::PublicId::new("@flow.opening")
                    .expect("valid public id"),
            ),
            qualified_name: Some("flow.opening".to_owned()),
            kind: "flow".to_owned(),
            type_json: None,
            source_path: None,
            source_content_hash: None,
            start_byte: None,
            end_byte: None,
            semantic_hash: None,
            summary: "Opening flow exposes the first choice".to_owned(),
            metadata: BTreeMap::new(),
        })
        .expect("upsert graph source symbol");
    store
        .upsert_graph_symbol(&DebugGraphSymbol {
            symbol_id: "symbol:choice.alice".to_owned(),
            program_hash: program_hash.clone(),
            public_id: Some(
                arcweft_agent_protocol::ids::PublicId::new("@choice.alice")
                    .expect("valid public id"),
            ),
            qualified_name: Some("choice.alice".to_owned()),
            kind: "choice".to_owned(),
            type_json: None,
            source_path: None,
            source_content_hash: None,
            start_byte: None,
            end_byte: None,
            semantic_hash: None,
            summary: "Alice route choice".to_owned(),
            metadata: BTreeMap::new(),
        })
        .expect("upsert graph target symbol");
    store
        .upsert_graph_edge(&DebugGraphEdge {
            program_hash: program_hash.clone(),
            from_symbol_id: "symbol:flow.opening".to_owned(),
            to_symbol_id: "symbol:choice.alice".to_owned(),
            edge_kind: "offers_choice".to_owned(),
            weight: 1.25,
            metadata: BTreeMap::new(),
        })
        .expect("upsert graph edge");
    store
        .upsert_graph_symbol(&DebugGraphSymbol {
            symbol_id: "symbol:textbox.main".to_owned(),
            program_hash: program_hash.clone(),
            public_id: Some(
                arcweft_agent_protocol::ids::PublicId::new("@textbox.main")
                    .expect("valid public id"),
            ),
            qualified_name: Some("textbox.main".to_owned()),
            kind: "textbox".to_owned(),
            type_json: None,
            source_path: None,
            source_content_hash: None,
            start_byte: None,
            end_byte: None,
            semantic_hash: None,
            summary: "Main textbox reached through Alice choice".to_owned(),
            metadata: BTreeMap::new(),
        })
        .expect("upsert graph expanded symbol");
    store
        .upsert_graph_edge(&DebugGraphEdge {
            program_hash,
            from_symbol_id: "symbol:choice.alice".to_owned(),
            to_symbol_id: "symbol:textbox.main".to_owned(),
            edge_kind: "uses_textbox".to_owned(),
            weight: 1.0,
            metadata: BTreeMap::new(),
        })
        .expect("upsert graph expanded edge");
}

fn seed_debug_prune_db(path: &Path) {
    let store = DebugStore::open(path).expect("open debug prune db");
    let now = current_unix_millis_for_test();
    let old = now - 2 * 86_400_000;
    let old_program = stable_hash("b3:old-retention-program");
    let new_program = stable_hash("b3:new-retention-program");
    store
        .upsert_program(&old_program, None, Some("old"), old)
        .expect("old retention program");
    store
        .upsert_program(&new_program, None, Some("new"), now)
        .expect("new retention program");
    store
        .start_session(
            &SessionId::new("session.old.retention").expect("old session"),
            Some(&old_program),
            "test",
            "cli",
            old,
        )
        .expect("old retention session");
    store
        .start_session(
            &SessionId::new("session.new.retention").expect("new session"),
            Some(&new_program),
            "test",
            "cli",
            now,
        )
        .expect("new retention session");
    for (chunk_id, program_hash, created_unix_ms) in [
        ("chunk:old-retention", old_program, old),
        ("chunk:new-retention", new_program, now),
    ] {
        store
            .upsert_chunk(&DebugChunk {
                id: ChunkId::new(chunk_id),
                program_hash: Some(program_hash),
                source_kind: ChunkSourceKind::Documentation,
                source_key: chunk_id.to_owned(),
                title: chunk_id.to_owned(),
                body: "retention prune marker".to_owned(),
                content_hash: stable_hash(&format!("b3:{chunk_id}")),
                semantic_hash: None,
                source_anchor: None,
                entity_ids: Vec::new(),
                privacy: PrivacyClass::Project,
                metadata: BTreeMap::new(),
                created_unix_ms,
            })
            .expect("retention chunk");
    }
}

