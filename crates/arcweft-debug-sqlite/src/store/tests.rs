use super::*;
use arcweft_agent_protocol::ids::{AgentRunId, PublicId, SessionId, StableHash};
use arcweft_debug_model::{
    chunk::{ChunkId, ChunkSourceKind, DebugChunk, PrivacyClass, SourceAnchor},
    diagnostic::DebugDiagnostic,
    embedding::{EmbeddingInputPolicy, EmbeddingModelDescriptor, StoredEmbedding},
    event::{DebugEvent, DebugEventKind},
    graph::{DebugGraphEdge, DebugGraphSymbol},
    history::DebugHistoryEntry,
    rag::{RagContextItem, RagContextPack, RagQuery, SearchChannel},
    repl::DebugReplCell,
    script::{DebugScriptRun, DebugScriptRunFinish, DebugScriptRunOutcome},
    session::{DebugSession, DebugSessionStatus},
    sink::DebugEventSink,
    source::DebugSourceFile,
    test_result::DebugTestResult,
};
use std::collections::{BTreeMap, BTreeSet};

fn hash(value: &str) -> StableHash {
    StableHash::new(value).expect("non-empty hash")
}

fn seed_rag_audit_fixture(store: &DebugStore) -> RagContextPack {
    let program_hash = hash("blake3:rag-program");
    store
        .upsert_program(&program_hash, None, Some("."), 0)
        .expect("program");
    let secret_chunk = rag_fixture_chunk(
        "chunk:secret-rag",
        Some(program_hash.clone()),
        ChunkSourceKind::AgentTrace,
        PrivacyClass::Secret,
        "secret trace",
        "secret body should not be returned to public readback",
    );
    let public_chunk = rag_fixture_chunk(
        "chunk:public-rag",
        Some(program_hash.clone()),
        ChunkSourceKind::Documentation,
        PrivacyClass::Public,
        "public doc",
        "public body remains visible",
    );
    store.upsert_chunk(&secret_chunk).expect("secret chunk");
    store.upsert_chunk(&public_chunk).expect("public chunk");
    RagContextPack {
        schema_version: 1,
        query: RagQuery {
            query_id: "rag:query:opening".to_owned(),
            text: "opening".to_owned(),
            program_hash,
            roots: vec![PublicId::new("@flow.opening").expect("root")],
            graph_depth: 2,
            limit: 1,
            max_context_bytes: 1024,
        },
        items: vec![
            RagContextItem {
                chunk_id: secret_chunk.id.clone(),
                kind: secret_chunk.source_kind,
                title: secret_chunk.title.clone(),
                body: secret_chunk.body.clone(),
                fused_score: 9.0,
                channels: BTreeSet::from([SearchChannel::Trace, SearchChannel::Vector]),
                entity_ids: secret_chunk.entity_ids.clone(),
                source_anchor: secret_chunk.source_anchor.clone(),
            },
            RagContextItem {
                chunk_id: public_chunk.id.clone(),
                kind: public_chunk.source_kind,
                title: public_chunk.title.clone(),
                body: public_chunk.body.clone(),
                fused_score: 1.0,
                channels: BTreeSet::from([SearchChannel::Lexical]),
                entity_ids: public_chunk.entity_ids.clone(),
                source_anchor: public_chunk.source_anchor.clone(),
            },
        ],
        truncated: false,
    }
}

fn rag_fixture_chunk(
    id: &str,
    program_hash: Option<StableHash>,
    source_kind: ChunkSourceKind,
    privacy: PrivacyClass,
    title: &str,
    body: &str,
) -> DebugChunk {
    DebugChunk {
        id: ChunkId::new(id),
        program_hash,
        source_kind,
        source_key: id.replace("chunk:", ""),
        title: title.to_owned(),
        body: body.to_owned(),
        content_hash: hash(format!("blake3:{id}").as_str()),
        semantic_hash: None,
        source_anchor: (privacy == PrivacyClass::Secret).then(|| SourceAnchor {
            path: "trace.arcwx".to_owned(),
            start_byte: 7,
            end_byte: 13,
        }),
        entity_ids: vec![PublicId::new(format!("@flow.{title}")).expect("public id")],
        privacy,
        metadata: BTreeMap::new(),
        created_unix_ms: 0,
    }
}

#[test]
fn migration_and_japanese_fts_work() {
    let store = DebugStore::open_in_memory().expect("open store");
    assert_eq!(store.user_version().expect("version"), 1);
    let program_hash = hash("b3:program");
    store
        .upsert_program(&program_hash, None, Some("."), 0)
        .expect("program");
    let chunk = DebugChunk {
        id: ChunkId::new("chunk:opening"),
        program_hash: Some(program_hash),
        source_kind: ChunkSourceKind::Source,
        source_key: "flow.opening".to_owned(),
        title: "opening".to_owned(),
        body: "選択肢を選ぶとアリスの場面へ移動する".to_owned(),
        content_hash: hash("b3:content"),
        semantic_hash: None,
        source_anchor: None,
        entity_ids: Vec::new(),
        privacy: PrivacyClass::Project,
        metadata: BTreeMap::new(),
        created_unix_ms: 0,
    };
    store.upsert_chunk(&chunk).expect("chunk");
    let hits = store.lexical_search("アリス", 10).expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].hit.chunk_id.as_str(), "chunk:opening");
    let chunk_hits = store
        .lexical_chunk_search_with_max_privacy("アリス", 10, PrivacyClass::Project)
        .expect("full chunk search");
    assert_eq!(chunk_hits.len(), 1);
    assert_eq!(chunk_hits[0].chunk, chunk);
}

#[test]
fn embedding_round_trips_without_unsafe_casts() {
    let store = DebugStore::open_in_memory().expect("open store");
    let chunk = DebugChunk {
        id: ChunkId::new("chunk:vector"),
        program_hash: None,
        source_kind: ChunkSourceKind::Documentation,
        source_key: "doc".to_owned(),
        title: "doc".to_owned(),
        body: "vector".to_owned(),
        content_hash: hash("b3:content"),
        semantic_hash: None,
        source_anchor: None,
        entity_ids: Vec::new(),
        privacy: PrivacyClass::Project,
        metadata: BTreeMap::new(),
        created_unix_ms: 0,
    };
    store.upsert_chunk(&chunk).expect("chunk");
    let model = EmbeddingModelDescriptor {
        model_id: "fixture".to_owned(),
        model_revision: "1".to_owned(),
        dimensions: 2,
    };
    let embedding = StoredEmbedding::normalized(
        chunk.id.clone(),
        model.clone(),
        vec![3.0, 4.0],
        "b3:content",
        0,
    )
    .expect("embedding");
    store.upsert_embedding(&embedding).expect("store embedding");
    let loaded = store.load_embeddings(&model).expect("load embedding");
    assert_eq!(loaded.len(), 1);
    assert!((loaded[0].values[0] - 0.6).abs() < 0.000_1);
    assert!((loaded[0].values[1] - 0.8).abs() < 0.000_1);
}

#[test]
fn reindex_rebuilds_fts_and_reports_chunk_count() {
    let store = DebugStore::open_in_memory().expect("open store");
    let chunk = DebugChunk {
        id: ChunkId::new("chunk:reindex"),
        program_hash: None,
        source_kind: ChunkSourceKind::Documentation,
        source_key: "doc".to_owned(),
        title: "manual".to_owned(),
        body: "debug store lifecycle".to_owned(),
        content_hash: hash("b3:content"),
        semantic_hash: None,
        source_anchor: None,
        entity_ids: Vec::new(),
        privacy: PrivacyClass::Project,
        metadata: BTreeMap::new(),
        created_unix_ms: 0,
    };
    store.upsert_chunk(&chunk).expect("chunk");
    let report = store.reindex().expect("reindex");
    assert_eq!(report.chunks_indexed, 1);
    let hits = store.lexical_search("lifecycle", 10).expect("search");
    assert_eq!(hits.len(), 1);
}

#[test]
fn lexical_search_filters_by_max_privacy_before_limit() {
    let store = DebugStore::open_in_memory().expect("open store");
    let chunks = [
        DebugChunk {
            id: ChunkId::new("chunk:secret"),
            program_hash: None,
            source_kind: ChunkSourceKind::Documentation,
            source_key: "secret".to_owned(),
            title: "opening secret".to_owned(),
            body: "opening secret evidence".to_owned(),
            content_hash: hash("b3:secret"),
            semantic_hash: None,
            source_anchor: None,
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Secret,
            metadata: BTreeMap::new(),
            created_unix_ms: 0,
        },
        DebugChunk {
            id: ChunkId::new("chunk:public"),
            program_hash: None,
            source_kind: ChunkSourceKind::Documentation,
            source_key: "public".to_owned(),
            title: "opening public".to_owned(),
            body: "opening public evidence".to_owned(),
            content_hash: hash("b3:public"),
            semantic_hash: None,
            source_anchor: None,
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Public,
            metadata: BTreeMap::new(),
            created_unix_ms: 0,
        },
    ];
    for chunk in &chunks {
        store.upsert_chunk(chunk).expect("chunk");
    }

    let hits = store
        .lexical_search_with_max_privacy("opening", 1, PrivacyClass::Public)
        .expect("search");

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].hit.chunk_id.as_str(), "chunk:public");
    assert_eq!(hits[0].privacy, PrivacyClass::Public);
}

#[test]
fn vector_search_filters_by_max_privacy_before_limit() {
    let store = DebugStore::open_in_memory().expect("open store");
    let model = EmbeddingModelDescriptor {
        model_id: "fixture".to_owned(),
        model_revision: "1".to_owned(),
        dimensions: 2,
    };
    let chunks = [
        (
            DebugChunk {
                id: ChunkId::new("chunk:secret-vector"),
                program_hash: None,
                source_kind: ChunkSourceKind::Documentation,
                source_key: "secret".to_owned(),
                title: "secret vector".to_owned(),
                body: "secret vector evidence".to_owned(),
                content_hash: hash("b3:secret-vector"),
                semantic_hash: None,
                source_anchor: None,
                entity_ids: Vec::new(),
                privacy: PrivacyClass::Secret,
                metadata: BTreeMap::new(),
                created_unix_ms: 0,
            },
            vec![1.0, 0.0],
        ),
        (
            DebugChunk {
                id: ChunkId::new("chunk:public-vector"),
                program_hash: None,
                source_kind: ChunkSourceKind::Documentation,
                source_key: "public".to_owned(),
                title: "public vector".to_owned(),
                body: "public vector evidence".to_owned(),
                content_hash: hash("b3:public-vector"),
                semantic_hash: None,
                source_anchor: None,
                entity_ids: Vec::new(),
                privacy: PrivacyClass::Public,
                metadata: BTreeMap::new(),
                created_unix_ms: 0,
            },
            vec![0.9, 0.1],
        ),
    ];
    for (chunk, vector) in chunks {
        store.upsert_chunk(&chunk).expect("chunk");
        let embedding = StoredEmbedding::normalized(
            chunk.id,
            model.clone(),
            vector,
            chunk.content_hash.as_str(),
            0,
        )
        .expect("embedding");
        store.upsert_embedding(&embedding).expect("store embedding");
    }

    let hits = store
        .vector_search_with_max_privacy(&model, &[1.0, 0.0], 1, PrivacyClass::Public)
        .expect("vector search");

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].hit.chunk_id.as_str(), "chunk:public-vector");
    assert_eq!(hits[0].hit.channel, SearchChannel::Vector);
    assert_eq!(hits[0].privacy, PrivacyClass::Public);
}

#[test]
fn embedding_inputs_apply_provider_privacy_policy_before_adapter_io() {
    let store = DebugStore::open_in_memory().expect("open store");
    for privacy in [
        PrivacyClass::Public,
        PrivacyClass::Project,
        PrivacyClass::Sensitive,
        PrivacyClass::Secret,
    ] {
        store
            .upsert_chunk(&privacy_fixture_chunk(privacy))
            .expect("chunk");
    }

    let local_inputs = store
        .embedding_inputs_with_policy(EmbeddingInputPolicy::local(PrivacyClass::Sensitive))
        .expect("local embedding inputs");
    assert_eq!(
        local_inputs
            .iter()
            .map(|input| input.chunk_id.as_str())
            .collect::<Vec<_>>(),
        vec!["chunk:project", "chunk:public", "chunk:sensitive"]
    );

    let remote_inputs = store
        .embedding_inputs_with_policy(EmbeddingInputPolicy::remote(PrivacyClass::Secret))
        .expect("remote embedding inputs");
    assert_eq!(
        remote_inputs
            .iter()
            .map(|input| input.chunk_id.as_str())
            .collect::<Vec<_>>(),
        vec!["chunk:project", "chunk:public"]
    );
    assert!(
        remote_inputs
            .iter()
            .all(|input| { matches!(input.privacy, PrivacyClass::Public | PrivacyClass::Project) })
    );
}

fn privacy_fixture_chunk(privacy: PrivacyClass) -> DebugChunk {
    let name = privacy.as_str();
    DebugChunk {
        id: ChunkId::new(format!("chunk:{name}")),
        program_hash: None,
        source_kind: ChunkSourceKind::Documentation,
        source_key: name.to_owned(),
        title: format!("{name} title"),
        body: format!("{name} body"),
        content_hash: hash(format!("blake3:{name}").as_str()),
        semantic_hash: None,
        source_anchor: None,
        entity_ids: Vec::new(),
        privacy,
        metadata: BTreeMap::new(),
        created_unix_ms: 0,
    }
}

#[test]
fn history_search_filters_project_privacy_before_limit() {
    let store = DebugStore::open_in_memory().expect("open store");
    let entry = DebugHistoryEntry {
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
    };
    store.upsert_history_entry(&entry).expect("history");

    let public_hits = store
        .history_search_with_max_privacy("opening", 1, PrivacyClass::Public)
        .expect("public history search");
    assert_eq!(public_hits, Vec::new());

    let project_hits = store
        .history_search_with_max_privacy("opening", 1, PrivacyClass::Project)
        .expect("project history search");
    assert_eq!(project_hits.len(), 1);
    assert_eq!(
        project_hits[0].hit.chunk_id.as_str(),
        "history:history:opening-fix"
    );
    assert_eq!(project_hits[0].hit.channel, SearchChannel::History);
    assert_eq!(project_hits[0].privacy, PrivacyClass::Project);
}

#[test]
fn diagnostic_and_test_result_search_filter_project_privacy_before_limit() {
    let store = DebugStore::open_in_memory().expect("open store");
    let program_hash = hash("blake3:diagnostic-test-program");
    store
        .upsert_program(&program_hash, None, Some("."), 0)
        .expect("program");
    store
        .upsert_diagnostic(&DebugDiagnostic {
            diagnostic_id: "diag:missing-shader".to_owned(),
            program_hash: Some(program_hash.clone()),
            session_id: None,
            run_id: None,
            sequence: Some(3),
            code: Some("RT_SHADER_MISSING".to_owned()),
            severity: "error".to_owned(),
            phase: "render".to_owned(),
            message: "missing shader binding for glyph wobble".to_owned(),
            source_path: Some("samples/rich-text-effects-animation/src/main.arcw".to_owned()),
            start_byte: Some(12),
            end_byte: Some(34),
            related_ids: vec![PublicId::new("@effect.wobble").expect("public id")],
            payload: serde_json::json!({ "shader": "glyph_wobble" }),
            created_unix_ms: 0,
        })
        .expect("diagnostic");
    store
        .upsert_test_result(&DebugTestResult {
            test_result_id: "test:visual-regression".to_owned(),
            program_hash: Some(program_hash),
            run_id: None,
            test_id: "rich-text-visual-regression".to_owned(),
            kind: "visual".to_owned(),
            outcome: "failed".to_owned(),
            duration_millis: Some(42),
            diagnostic_ids: vec!["diag:missing-shader".to_owned()],
            artifact_refs: vec!["blob:visual-diff".to_owned()],
            summary: "visual regression detected missing shader output".to_owned(),
            created_unix_ms: 0,
        })
        .expect("test result");

    let public_diagnostics = store
        .diagnostic_search_with_max_privacy("glyph_wobble", 1, PrivacyClass::Public)
        .expect("public diagnostic search");
    assert!(public_diagnostics.is_empty());
    let diagnostics = store
        .diagnostic_search_with_max_privacy("glyph_wobble", 1, PrivacyClass::Project)
        .expect("project diagnostic search");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].source_kind, "diagnostic");
    assert_eq!(diagnostics[0].hit.channel, SearchChannel::Diagnostics);
    assert_eq!(
        diagnostics[0].hit.chunk_id.as_str(),
        "diagnostic:diag:missing-shader"
    );
    assert!(diagnostics[0].body.contains("related_ids"));

    let public_tests = store
        .test_result_search_with_max_privacy("rich-text-visual-regression", 1, PrivacyClass::Public)
        .expect("public test search");
    assert!(public_tests.is_empty());
    let tests = store
        .test_result_search_with_max_privacy(
            "rich-text-visual-regression",
            1,
            PrivacyClass::Project,
        )
        .expect("project test search");
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].source_kind, "test_result");
    assert_eq!(tests[0].hit.channel, SearchChannel::Diagnostics);
    assert_eq!(
        tests[0].hit.chunk_id.as_str(),
        "test_result:test:visual-regression"
    );
    assert!(tests[0].body.contains("diagnostic_ids"));
}

#[test]
fn debug_session_round_trips_and_finishes() {
    let store = DebugStore::open_in_memory().expect("open store");
    let program_hash = hash("blake3:session-program");
    store
        .upsert_program(&program_hash, None, Some("."), 0)
        .expect("program");
    let session_id = SessionId::new("session.product").expect("session id");
    let mut metadata = BTreeMap::new();
    metadata.insert("target".to_owned(), serde_json::json!("native-player"));
    let session = DebugSession {
        session_id: session_id.clone(),
        program_hash: Some(program_hash.clone()),
        profile: "developer".to_owned(),
        transport: "native".to_owned(),
        started_unix_ms: 10,
        ended_unix_ms: None,
        status: DebugSessionStatus::Running,
        metadata,
    };
    store.upsert_session(&session).expect("upsert session");

    assert_eq!(
        store.session(&session_id).expect("read session"),
        Some(session.clone())
    );

    let mut finished_metadata = BTreeMap::new();
    finished_metadata.insert("reason".to_owned(), serde_json::json!("test-complete"));
    store
        .finish_session(
            &session_id,
            DebugSessionStatus::Finished,
            25,
            &finished_metadata,
        )
        .expect("finish session");
    let finished = store
        .session(&session_id)
        .expect("read finished session")
        .expect("session exists");

    assert_eq!(finished.program_hash, Some(program_hash));
    assert_eq!(finished.status, DebugSessionStatus::Finished);
    assert_eq!(finished.ended_unix_ms, Some(25));
    assert_eq!(finished.metadata["reason"], "test-complete");
    assert_eq!(store.sessions(1).expect("list sessions"), vec![finished]);
}

#[test]
fn stale_running_sessions_are_abandoned_by_lifecycle_policy() {
    let store = DebugStore::open_in_memory().expect("open store");
    let old = SessionId::new("session.old-running").expect("session id");
    let fresh = SessionId::new("session.fresh-running").expect("session id");
    let finished = SessionId::new("session.finished").expect("session id");
    store
        .start_session(&old, None, "agent", "cli", 1_000)
        .expect("old session");
    store
        .start_session(&fresh, None, "agent", "cli", 5_000)
        .expect("fresh session");
    store
        .start_session(&finished, None, "agent", "cli", 500)
        .expect("finished session");
    store
        .finish_session(
            &finished,
            DebugSessionStatus::Finished,
            750,
            &BTreeMap::new(),
        )
        .expect("finish session");

    let stale = store
        .stale_running_sessions(2_000)
        .expect("stale running sessions");
    assert_eq!(
        stale
            .iter()
            .map(|session| session.session_id.as_str())
            .collect::<Vec<_>>(),
        vec!["session.old-running"]
    );

    let abandoned = store
        .abandon_stale_running_sessions(2_000, 6_000, "test-stale-policy")
        .expect("abandon stale sessions");
    assert_eq!(abandoned.len(), 1);
    assert_eq!(abandoned[0].session_id, old);
    assert_eq!(abandoned[0].status, DebugSessionStatus::Abandoned);
    assert_eq!(abandoned[0].ended_unix_ms, Some(6_000));
    assert_eq!(
        abandoned[0].metadata["lifecycle_policy"]["reason"],
        "test-stale-policy"
    );

    assert_eq!(
        store
            .session(&fresh)
            .expect("fresh session")
            .expect("fresh exists")
            .status,
        DebugSessionStatus::Running
    );
    assert_eq!(
        store
            .session(&finished)
            .expect("finished session")
            .expect("finished exists")
            .status,
        DebugSessionStatus::Finished
    );
}

#[test]
fn debug_script_run_round_trips_and_finishes() {
    let mut store = DebugStore::open_in_memory().expect("open store");
    let session_id = SessionId::new("session.script").expect("session id");
    let run_id = AgentRunId::new("run.script").expect("run id");
    store
        .start_session(&session_id, None, "script", "cli", 0)
        .expect("session");
    let run = DebugScriptRun {
        run_id: run_id.clone(),
        session_id: session_id.clone(),
        agent_id: Some(PublicId::new("agent.script").expect("agent id")),
        artifact_hash: None,
        source_hash: Some(hash("blake3:script-source")),
        project_binding_mode: "strict".to_owned(),
        started_sequence: 0,
        finished_sequence: None,
        outcome: DebugScriptRunOutcome::Running,
        partially_effectful: false,
        trace_uri: None,
        error: None,
        metadata: BTreeMap::new(),
    };
    store.upsert_script_run(&run).expect("script run");
    store
        .append(&DebugEvent {
            schema_version: 1,
            session_id: session_id.clone(),
            run_id: Some(run_id.clone()),
            sequence: 1,
            tick: Some(7),
            kind: DebugEventKind::Observation,
            payload: serde_json::json!({ "message": "observed" }),
            created_unix_ms: 0,
        })
        .expect("debug event");
    let mut metadata = BTreeMap::new();
    metadata.insert("steps".to_owned(), serde_json::json!(2));
    store
        .finish_script_run(
            &run_id,
            &DebugScriptRunFinish {
                outcome: DebugScriptRunOutcome::Done,
                finished_sequence: 2,
                partially_effectful: true,
                trace_uri: Some("target/run.arcwx".to_owned()),
                error: None,
                metadata,
            },
        )
        .expect("finish script run");

    let persisted = store
        .script_run(&run_id)
        .expect("load script run")
        .expect("script run exists");

    assert_eq!(persisted.outcome, DebugScriptRunOutcome::Done);
    assert_eq!(persisted.finished_sequence, Some(2));
    assert!(persisted.partially_effectful);
    assert_eq!(persisted.trace_uri.as_deref(), Some("target/run.arcwx"));
    assert_eq!(persisted.metadata["steps"], 2);
    assert_eq!(store.stats().expect("stats").script_runs, 1);
    assert_eq!(store.stats().expect("stats").debug_events, 1);
}

#[test]
fn script_runs_list_filters_by_session_and_limit() {
    let store = DebugStore::open_in_memory().expect("open store");
    let first_session = SessionId::new("session.script.one").expect("session id");
    let second_session = SessionId::new("session.script.two").expect("session id");
    store
        .start_session(&first_session, None, "script", "cli", 0)
        .expect("first session");
    store
        .start_session(&second_session, None, "script", "cli", 0)
        .expect("second session");

    for (run_id, session_id, started_sequence) in [
        ("run.script.first", &first_session, 1),
        ("run.script.second", &second_session, 2),
        ("run.script.third", &first_session, 3),
    ] {
        store
            .upsert_script_run(&DebugScriptRun {
                run_id: AgentRunId::new(run_id).expect("run id"),
                session_id: session_id.clone(),
                agent_id: Some(PublicId::new("agent.script").expect("agent id")),
                artifact_hash: None,
                source_hash: Some(hash("blake3:script-source")),
                project_binding_mode: "strict".to_owned(),
                started_sequence,
                finished_sequence: Some(started_sequence + 1),
                outcome: DebugScriptRunOutcome::Done,
                partially_effectful: false,
                trace_uri: None,
                error: None,
                metadata: BTreeMap::new(),
            })
            .expect("script run");
    }

    let latest = store.script_runs(None, 1).expect("latest run");
    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].run_id.as_str(), "run.script.third");

    let first_session_runs = store
        .script_runs(Some(&first_session), 10)
        .expect("first session runs");
    assert_eq!(
        first_session_runs
            .iter()
            .map(|run| run.run_id.as_str())
            .collect::<Vec<_>>(),
        vec!["run.script.third", "run.script.first"]
    );
}

#[test]
fn vacuum_reports_page_counts() {
    let store = DebugStore::open_in_memory().expect("open store");
    let report = store.vacuum().expect("vacuum store");

    assert!(report.page_count_before > 0);
    assert!(report.page_count_after > 0);
    assert!(report.freelist_count_after <= report.freelist_count_before);
}

#[test]
fn prune_before_removes_old_rebuildable_debug_rows() {
    let mut store = DebugStore::open_in_memory().expect("open store");
    let old_program = hash("blake3:old-program");
    let new_program = hash("blake3:new-program");
    seed_prune_lifecycle_rows(&mut store, &old_program, &new_program);
    seed_prune_chunks(&store, &old_program, &new_program);
    seed_prune_raw_rows(&store);

    let report = store.prune_before(100).expect("prune old rows");
    assert_eq!(report.sessions, 1);
    assert_eq!(report.rag_queries, 1);
    assert_eq!(report.chunks, 1);
    assert_eq!(report.blobs, 1);
    assert_eq!(report.programs, 1);

    let stats = store.stats().expect("stats");
    assert_eq!(stats.sessions, 1);
    assert_eq!(stats.script_runs, 0);
    assert_eq!(stats.debug_events, 0);
    assert_eq!(stats.repl_cells, 0);
    assert_eq!(stats.rag_queries, 1);
    assert_eq!(stats.chunks, 1);
    assert_eq!(stats.blobs, 1);
    assert_eq!(stats.programs, 1);
    assert_eq!(
        store
            .lexical_search_with_max_privacy("retention", 10, PrivacyClass::Project)
            .expect("search after prune")
            .iter()
            .map(|hit| hit.hit.chunk_id.as_str())
            .collect::<Vec<_>>(),
        vec!["chunk:new-prune"]
    );
}

fn seed_prune_lifecycle_rows(
    store: &mut DebugStore,
    old_program: &StableHash,
    new_program: &StableHash,
) {
    store
        .upsert_program(old_program, None, Some("old"), 10)
        .expect("old program");
    store
        .upsert_program(new_program, None, Some("new"), 200)
        .expect("new program");
    let old_session = SessionId::new("session.old").expect("old session");
    store
        .start_session(&old_session, Some(old_program), "test", "cli", 10)
        .expect("old session row");
    store
        .start_session(
            &SessionId::new("session.new").expect("new session"),
            Some(new_program),
            "test",
            "cli",
            200,
        )
        .expect("new session row");
    let old_run = AgentRunId::new("run.old").expect("old run");
    store
        .upsert_script_run(&prune_script_run(&old_session, &old_run))
        .expect("old script run");
    store
        .append(&prune_debug_event(&old_session, &old_run))
        .expect("old event");
    store
        .upsert_repl_cell(&prune_repl_cell(old_session, old_run))
        .expect("old repl cell");
}

fn prune_script_run(session_id: &SessionId, run_id: &AgentRunId) -> DebugScriptRun {
    DebugScriptRun {
        run_id: run_id.clone(),
        session_id: session_id.clone(),
        agent_id: Some(PublicId::new("agent.old").expect("agent id")),
        artifact_hash: None,
        source_hash: Some(hash("blake3:old-source")),
        project_binding_mode: "strict".to_owned(),
        started_sequence: 1,
        finished_sequence: None,
        outcome: DebugScriptRunOutcome::Running,
        partially_effectful: false,
        trace_uri: None,
        error: None,
        metadata: BTreeMap::new(),
    }
}

fn prune_debug_event(session_id: &SessionId, run_id: &AgentRunId) -> DebugEvent {
    DebugEvent {
        schema_version: 1,
        session_id: session_id.clone(),
        run_id: Some(run_id.clone()),
        sequence: 1,
        tick: Some(1),
        kind: DebugEventKind::Observation,
        payload: serde_json::json!({ "message": "old" }),
        created_unix_ms: 10,
    }
}

fn prune_repl_cell(session_id: SessionId, run_id: AgentRunId) -> DebugReplCell {
    DebugReplCell {
        cell_id: "repl:old:1".to_owned(),
        session_id,
        run_id: Some(run_id),
        ordinal: 1,
        source: "observe()".to_owned(),
        source_hash: hash("blake3:old-cell"),
        status: "ok".to_owned(),
        inferred_type: None,
        display: None,
        partially_effectful: false,
        diagnostic_ids: Vec::new(),
        created_unix_ms: 10,
    }
}

fn seed_prune_chunks(store: &DebugStore, old_program: &StableHash, new_program: &StableHash) {
    for (chunk_id, program_hash, created_unix_ms) in [
        ("chunk:old-prune", old_program.clone(), 10),
        ("chunk:new-prune", new_program.clone(), 200),
    ] {
        store
            .upsert_chunk(&DebugChunk {
                id: ChunkId::new(chunk_id),
                program_hash: Some(program_hash),
                source_kind: ChunkSourceKind::Documentation,
                source_key: chunk_id.to_owned(),
                title: chunk_id.to_owned(),
                body: "debug retention body".to_owned(),
                content_hash: hash(&format!("blake3:{chunk_id}")),
                semantic_hash: None,
                source_anchor: None,
                entity_ids: Vec::new(),
                privacy: PrivacyClass::Project,
                metadata: BTreeMap::new(),
                created_unix_ms,
            })
            .expect("chunk row");
    }
}

fn seed_prune_raw_rows(store: &DebugStore) {
    store
        .connection
        .execute_batch(
            "INSERT INTO rag_queries(
                   query_id, query_text, query_hash, policy_json, status, created_unix_ms
                 ) VALUES
                   ('rag:old-prune', 'old', 'hash:old', '{}', 'selected', 10),
                   ('rag:new-prune', 'new', 'hash:new', '{}', 'selected', 200);
                 INSERT INTO blobs(
                   blob_hash, media_type, byte_len, relative_path, privacy_class,
                   created_unix_ms, last_access_unix_ms
                 ) VALUES
                   ('blob:old-prune', 'image/png', 1, 'blake3/old-prune', 'project', 10, 10),
                   ('blob:new-prune', 'image/png', 1, 'blake3/new-prune', 'project', 200, 200);",
        )
        .expect("raw prune rows");
}

#[test]
fn session_timeline_filters_privacy_before_limit() {
    let mut store = DebugStore::open_in_memory().expect("open store");
    let session_id = SessionId::new("session.timeline").expect("session id");
    store
        .start_session(&session_id, None, "test", "in-memory", 0)
        .expect("session");
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
                created_unix_ms: i64::try_from(sequence).expect("test sequence fits i64"),
            })
            .expect("append event");
    }

    let events = store
        .session_timeline_with_max_privacy(Some(session_id.as_str()), None, 1, PrivacyClass::Public)
        .expect("timeline");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].sequence, 2);
    assert_eq!(events[0].privacy, PrivacyClass::Public);
    assert_eq!(events[0].payload["message"], "visible event");
}

#[test]
fn rag_query_audit_round_trips_and_filters_privacy_before_limit() {
    let store = DebugStore::open_in_memory().expect("open store");
    let pack = seed_rag_audit_fixture(&store);
    store
        .record_rag_context_pack(&pack, None, None, None, "selected", 123)
        .expect("record audit");

    let public_audit = store
        .rag_query_audit_with_max_privacy("rag:query:opening", PrivacyClass::Public)
        .expect("public audit");

    assert_eq!(public_audit.status, "selected");
    assert_eq!(public_audit.created_unix_ms, 123);
    assert_eq!(public_audit.pack.query.text, "opening");
    assert_eq!(public_audit.pack.query.graph_depth, 2);
    assert_eq!(public_audit.pack.query.roots.len(), 1);
    assert_eq!(public_audit.pack.items.len(), 1);
    assert_eq!(
        public_audit.pack.items[0].chunk_id.as_str(),
        "chunk:public-rag"
    );
    assert_eq!(
        public_audit.pack.items[0].channels,
        BTreeSet::from([SearchChannel::Lexical])
    );
    assert!(!public_audit.pack.items[0].body.contains("secret"));

    let secret_audit = store
        .rag_query_audit_with_max_privacy("rag:query:opening", PrivacyClass::Secret)
        .expect("secret audit");

    assert_eq!(secret_audit.pack.items.len(), 1);
    assert_eq!(
        secret_audit.pack.items[0].chunk_id.as_str(),
        "chunk:secret-rag"
    );
    assert_eq!(
        secret_audit.pack.items[0].channels,
        BTreeSet::from([SearchChannel::Trace, SearchChannel::Vector])
    );
    assert_eq!(
        secret_audit.pack.items[0]
            .source_anchor
            .as_ref()
            .unwrap()
            .path,
        "trace.arcwx"
    );
    assert_eq!(store.stats().expect("stats").rag_queries, 1);
}

#[test]
fn graph_search_filters_project_privacy_before_limit() {
    let store = DebugStore::open_in_memory().expect("open store");
    let program_hash = hash("b3:graph-program");
    store
        .upsert_program(&program_hash, None, Some("."), 0)
        .expect("program");
    store
        .upsert_graph_symbol(&DebugGraphSymbol {
            symbol_id: "symbol:flow.opening".to_owned(),
            program_hash: program_hash.clone(),
            public_id: Some(PublicId::new("@flow.opening").expect("public id")),
            qualified_name: Some("flow.opening".to_owned()),
            kind: "flow".to_owned(),
            type_json: None,
            source_path: None,
            source_content_hash: None,
            start_byte: None,
            end_byte: None,
            semantic_hash: None,
            summary: "Opening flow dispatches the first choice".to_owned(),
            metadata: BTreeMap::new(),
        })
        .expect("from symbol");
    store
        .upsert_graph_symbol(&DebugGraphSymbol {
            symbol_id: "symbol:choice.alice".to_owned(),
            program_hash: program_hash.clone(),
            public_id: Some(PublicId::new("@choice.alice").expect("public id")),
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
        .expect("to symbol");
    store
        .upsert_graph_edge(&DebugGraphEdge {
            program_hash: program_hash.clone(),
            from_symbol_id: "symbol:flow.opening".to_owned(),
            to_symbol_id: "symbol:choice.alice".to_owned(),
            edge_kind: "offers_choice".to_owned(),
            weight: 1.25,
            metadata: BTreeMap::new(),
        })
        .expect("edge");
    store
        .upsert_graph_symbol(&DebugGraphSymbol {
            symbol_id: "symbol:view.main".to_owned(),
            program_hash: program_hash.clone(),
            public_id: Some(PublicId::new("@view.main").expect("public id")),
            qualified_name: Some("view.main".to_owned()),
            kind: "view".to_owned(),
            type_json: None,
            source_path: None,
            source_content_hash: None,
            start_byte: None,
            end_byte: None,
            semantic_hash: None,
            summary: "Main View reached through Alice choice".to_owned(),
            metadata: BTreeMap::new(),
        })
        .expect("expanded symbol");
    store
        .upsert_graph_edge(&DebugGraphEdge {
            program_hash,
            from_symbol_id: "symbol:choice.alice".to_owned(),
            to_symbol_id: "symbol:view.main".to_owned(),
            edge_kind: "uses_view".to_owned(),
            weight: 1.0,
            metadata: BTreeMap::new(),
        })
        .expect("expanded edge");

    let public_hits = store
        .graph_search_with_max_privacy("opening", 1, PrivacyClass::Public)
        .expect("public graph search");
    assert_eq!(public_hits, Vec::new());

    let project_hits = store
        .graph_search_with_max_privacy("opening", 1, PrivacyClass::Project)
        .expect("project graph search");
    assert_eq!(project_hits.len(), 1);
    assert_eq!(project_hits[0].hit.chunk_id.as_str(), "graph:1");
    assert_eq!(project_hits[0].hit.channel, SearchChannel::Graph);
    assert_eq!(project_hits[0].privacy, PrivacyClass::Project);
    assert!(project_hits[0].title.contains("@flow.opening"));

    let expanded_hits = store
        .graph_search_with_depth_and_max_privacy("opening", 2, 10, PrivacyClass::Project)
        .expect("expanded graph search");
    assert_eq!(expanded_hits.len(), 2);
    assert!(
        expanded_hits
            .iter()
            .any(|hit| hit.hit.chunk_id.as_str() == "graph:2" && hit.body.contains("distance=2"))
    );
}

#[test]
fn source_file_round_trips_for_program() {
    let store = DebugStore::open_in_memory().expect("open store");
    let program_hash = hash("b3:source-file-program");
    let content_hash = hash("b3:source-file-content");
    store
        .upsert_program(&program_hash, None, Some("."), 0)
        .expect("program");
    store
        .upsert_source_file(&DebugSourceFile {
            program_hash: program_hash.clone(),
            path: "samples/agent-script/native-choice-dispatch.arcw".to_owned(),
            language: "arcw".to_owned(),
            content_hash: content_hash.clone(),
            byte_len: 1234,
            metadata: BTreeMap::from([("extension".to_owned(), serde_json::json!("arcw"))]),
        })
        .expect("source file");

    let files = store
        .source_files_for_program(&program_hash)
        .expect("source files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].program_hash, program_hash);
    assert_eq!(files[0].content_hash, content_hash);
    assert_eq!(files[0].language, "arcw");
    assert_eq!(files[0].byte_len, 1234);
    assert_eq!(store.stats().expect("stats").source_files, 1);
}

#[test]
fn graph_inventory_round_trips_for_program() {
    let store = DebugStore::open_in_memory().expect("open store");
    let program_hash = hash("blake3:graph-inventory-program");
    let content_hash = hash("blake3:graph-inventory-content");
    store
        .upsert_program(&program_hash, None, Some("."), 0)
        .expect("program");
    store
        .upsert_source_file(&DebugSourceFile {
            program_hash: program_hash.clone(),
            path: "samples/agent-script/native-choice-dispatch.arcw".to_owned(),
            language: "arcw".to_owned(),
            content_hash: content_hash.clone(),
            byte_len: 2048,
            metadata: BTreeMap::new(),
        })
        .expect("source file");
    store
        .upsert_graph_symbol(&DebugGraphSymbol {
            symbol_id: "symbol:flow.opening".to_owned(),
            program_hash: program_hash.clone(),
            public_id: Some(PublicId::new("flow.opening").expect("public id")),
            qualified_name: Some("flow.opening".to_owned()),
            kind: "flow".to_owned(),
            type_json: Some(serde_json::json!({"returns": "String"})),
            source_path: Some("samples/agent-script/native-choice-dispatch.arcw".to_owned()),
            source_content_hash: Some(content_hash.clone()),
            start_byte: Some(10),
            end_byte: Some(42),
            semantic_hash: Some(hash("blake3:symbol-flow-opening")),
            summary: "Opening flow".to_owned(),
            metadata: BTreeMap::from([("role".to_owned(), serde_json::json!("entry"))]),
        })
        .expect("from symbol");
    store
        .upsert_graph_symbol(&DebugGraphSymbol {
            symbol_id: "symbol:choice.listen".to_owned(),
            program_hash: program_hash.clone(),
            public_id: Some(PublicId::new("choice.listen").expect("public id")),
            qualified_name: Some("choice.listen".to_owned()),
            kind: "agent_action".to_owned(),
            type_json: None,
            source_path: None,
            source_content_hash: None,
            start_byte: None,
            end_byte: None,
            semantic_hash: None,
            summary: "Listen choice".to_owned(),
            metadata: BTreeMap::new(),
        })
        .expect("to symbol");
    store
        .upsert_graph_edge(&DebugGraphEdge {
            program_hash: program_hash.clone(),
            from_symbol_id: "symbol:flow.opening".to_owned(),
            to_symbol_id: "symbol:choice.listen".to_owned(),
            edge_kind: "offers_action".to_owned(),
            weight: 0.75,
            metadata: BTreeMap::from([("via".to_owned(), serde_json::json!("test"))]),
        })
        .expect("edge");

    let symbols = store
        .graph_symbols_for_program(&program_hash)
        .expect("symbols");
    assert_eq!(symbols.len(), 2);
    let flow = symbols
        .iter()
        .find(|symbol| symbol.symbol_id == "symbol:flow.opening")
        .expect("flow symbol");
    assert_eq!(flow.program_hash, program_hash);
    assert_eq!(
        flow.public_id.as_ref().map(PublicId::as_str),
        Some("flow.opening")
    );
    assert_eq!(flow.source_content_hash, Some(content_hash));
    assert_eq!(flow.start_byte, Some(10));
    assert_eq!(flow.end_byte, Some(42));
    assert_eq!(
        flow.type_json,
        Some(serde_json::json!({"returns": "String"}))
    );
    assert_eq!(flow.metadata["role"], serde_json::json!("entry"));

    let edges = store.graph_edges_for_program(&program_hash).expect("edges");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].from_symbol_id, "symbol:flow.opening");
    assert_eq!(edges[0].to_symbol_id, "symbol:choice.listen");
    assert_eq!(edges[0].edge_kind, "offers_action");
    assert_eq!(edges[0].metadata["via"], serde_json::json!("test"));
}

#[test]
fn repl_cell_round_trips_for_session() {
    let store = DebugStore::open_in_memory().expect("open store");
    let session = SessionId::new("session.repl").expect("session id");
    store
        .start_session(&session, None, "repl", "cli", 0)
        .expect("session row");
    let cell = DebugReplCell {
        cell_id: "repl:session.repl:1".to_owned(),
        session_id: session.clone(),
        run_id: None,
        ordinal: 1,
        source: "let observed = observe()".to_owned(),
        source_hash: hash("blake3:repl-cell"),
        status: "ok".to_owned(),
        inferred_type: None,
        display: Some(serde_json::json!({ "host_calls": 1 })),
        partially_effectful: true,
        diagnostic_ids: vec!["diag.1".to_owned()],
        created_unix_ms: 0,
    };
    store.upsert_repl_cell(&cell).expect("repl cell");

    let cells = store
        .repl_cells_for_session(&session)
        .expect("load repl cells");

    assert_eq!(cells, vec![cell]);
    assert_eq!(store.stats().expect("stats").repl_cells, 1);
}

#[test]
fn delete_unreferenced_blobs_keeps_referenced_capture_blobs() {
    let store = DebugStore::open_in_memory().expect("open store");
    let session = SessionId::new("session.test").expect("session");
    store
        .start_session(&session, None, "default", "test", 0)
        .expect("session row");
    store
        .connection
        .execute(
            "INSERT INTO blobs(
                   blob_hash, media_type, byte_len, relative_path, privacy_class,
                   created_unix_ms, last_access_unix_ms
                 ) VALUES
                   ('blob:kept', 'image/png', 1, 'blake3/kept', 'project', 0, 0),
                   ('blob:deleted', 'image/png', 1, 'blake3/deleted', 'project', 0, 0)",
            [],
        )
        .expect("blob rows");
    store
        .connection
        .execute(
            "INSERT INTO captures(
                   capture_id, session_id, sequence, tick, scope_kind, capture_kind,
                   renderer, composition, blob_hash, resource_uri, width, height,
                   created_unix_ms
                 ) VALUES (
                   'capture:kept', 'session.test', 1, 1, 'viewport', 'color',
                   'native', 'color', 'blob:kept', 'arcweft://capture', 1, 1, 0
                 )",
            [],
        )
        .expect("capture row");

    let deleted = store
        .delete_unreferenced_blobs()
        .expect("delete unreferenced");
    assert_eq!(deleted, 1);
    assert_eq!(
        store.unreferenced_blob_records().expect("unreferenced"),
        Vec::new()
    );
    assert_eq!(
        store.blob_records().expect("blob records"),
        vec![DebugStoreBlobRecord {
            blob_hash: "blob:kept".to_owned(),
            byte_len: 1,
            relative_path: "blake3/kept".to_owned(),
        }]
    );
    let stats = store.stats().expect("stats");
    assert_eq!(stats.blobs, 1);
    let validation = store.validate().expect("validate");
    assert_eq!(validation.integrity_messages, Vec::<String>::new());
    assert_eq!(validation.foreign_key_violations, Vec::new());
    assert_eq!(validation.missing_capture_blob_refs, 0);
    assert_eq!(validation.invalid_embedding_blobs, 0);
}
