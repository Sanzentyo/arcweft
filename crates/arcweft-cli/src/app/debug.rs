use super::shared::print_json;
use arcweft_agent_protocol::ids::{AgentRunId, SessionId};
use arcweft_debug_model::chunk::PrivacyClass;
use arcweft_debug_model::embedding::EmbeddingModelDescriptor;
use arcweft_debug_model::rag::{RagContextPack, SearchChannel};
use arcweft_debug_model::script::{DebugScriptRun, DebugScriptRunOutcome};
use arcweft_debug_model::session::{DebugSession, DebugSessionStatus};
use arcweft_debug_sqlite::store::{
    ChunkSearchResult, DebugStore, DebugStoreBlobRecord, DebugStoreError,
    DebugStoreForeignKeyViolation, DebugStoreStats, DebugStoreValidationReport, DebugTimelineEvent,
};
use clap::{Args, Subcommand};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

const DEFAULT_DEBUG_DB_PATH: &str = ".arcweft/cache/agent-debug.sqlite3";

#[derive(Debug, Subcommand)]
pub(super) enum DebugCommand {
    Db {
        #[command(subcommand)]
        command: DebugDbCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum DebugDbCommand {
    Status(DebugDbOptions),
    Migrate(DebugDbOptions),
    Validate(DebugDbOptions),
    Reindex(DebugDbOptions),
    Vacuum(DebugDbOptions),
    Sessions(DebugDbSessionsOptions),
    Runs(DebugDbRunsOptions),
    Rag(DebugDbRagOptions),
    Timeline(DebugDbTimelineOptions),
    Search(DebugDbSearchOptions),
    Delete(DebugDbDeleteOptions),
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbOptions {
    #[arg(long, default_value = DEFAULT_DEBUG_DB_PATH)]
    path: PathBuf,
    #[arg(long = "blob-dir")]
    blob_dir: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbDeleteOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long)]
    unreferenced_blobs: bool,
    #[arg(long)]
    validate: bool,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbSessionsOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbRunsOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long = "session-id")]
    session_id: Option<String>,
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbRagOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long = "query-id")]
    query_id: String,
    #[arg(long, value_parser = parse_debug_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbTimelineOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long = "session-id")]
    session_id: Option<String>,
    #[arg(long = "run-id")]
    run_id: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
    #[arg(long, value_parser = parse_debug_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbSearchOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long)]
    query: Option<String>,
    #[arg(long = "query-vector")]
    query_vector: Option<String>,
    #[arg(long = "graph-query")]
    graph_query: Option<String>,
    #[arg(long = "history-query")]
    history_query: Option<String>,
    #[arg(long = "graph-depth", default_value_t = 1)]
    graph_depth: u32,
    #[arg(long = "model-id")]
    model_id: Option<String>,
    #[arg(long = "model-revision")]
    model_revision: Option<String>,
    #[arg(long, default_value_t = 10)]
    limit: usize,
    #[arg(long, value_parser = parse_debug_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
}

#[derive(serde::Serialize)]
struct DebugDbReport {
    path: String,
    user_version: u32,
    stats: DebugDbStatsReport,
}

#[derive(serde::Serialize)]
struct DebugDbStatsReport {
    programs: u64,
    sessions: u64,
    script_runs: u64,
    debug_events: u64,
    frames: u64,
    actions: u64,
    captures: u64,
    blobs: u64,
    chunks: u64,
    embeddings: u64,
    rag_queries: u64,
    repl_cells: u64,
}

#[derive(serde::Serialize)]
struct DebugDbValidationCliReport {
    path: String,
    blob_dir: Option<String>,
    user_version: u32,
    valid: bool,
    integrity_messages: Vec<String>,
    foreign_key_violations: Vec<DebugDbForeignKeyViolationReport>,
    missing_capture_blob_refs: u64,
    invalid_embedding_blobs: u64,
    blob_files: Option<DebugDbBlobFileValidationReport>,
    stats: DebugDbStatsReport,
}

#[derive(serde::Serialize)]
struct DebugDbForeignKeyViolationReport {
    table: String,
    rowid: i64,
    parent: String,
    fkid: i64,
}

#[derive(serde::Serialize)]
struct DebugDbReindexReport {
    path: String,
    user_version: u32,
    chunks_indexed: u64,
}

#[derive(serde::Serialize)]
struct DebugDbVacuumReport {
    path: String,
    user_version: u32,
    page_count_before: u64,
    freelist_count_before: u64,
    page_count_after: u64,
    freelist_count_after: u64,
}

#[derive(serde::Serialize)]
struct DebugDbSessionsReport {
    path: String,
    user_version: u32,
    limit: usize,
    sessions: Vec<DebugDbSessionReport>,
}

#[derive(serde::Serialize)]
struct DebugDbRunsReport {
    path: String,
    user_version: u32,
    session_id: Option<String>,
    limit: usize,
    runs: Vec<DebugDbRunReport>,
}

#[derive(serde::Serialize)]
struct DebugDbRunReport {
    run_id: String,
    session_id: String,
    agent_id: Option<String>,
    artifact_hash: Option<String>,
    source_hash: Option<String>,
    project_binding_mode: String,
    started_sequence: u64,
    finished_sequence: Option<u64>,
    outcome: DebugScriptRunOutcome,
    partially_effectful: bool,
    trace_uri: Option<String>,
    error: Option<serde_json::Value>,
    metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(serde::Serialize)]
struct DebugDbRagReport {
    path: String,
    user_version: u32,
    query_id: String,
    max_privacy: PrivacyClass,
    status: String,
    created_unix_ms: i64,
    pack: RagContextPack,
}

#[derive(serde::Serialize)]
struct DebugDbTimelineReport {
    path: String,
    user_version: u32,
    session_id: Option<String>,
    run_id: Option<String>,
    limit: usize,
    max_privacy: PrivacyClass,
    events: Vec<DebugDbTimelineEventReport>,
}

#[derive(serde::Serialize)]
struct DebugDbTimelineEventReport {
    session_id: String,
    run_id: Option<String>,
    sequence: u64,
    tick: Option<u64>,
    kind: String,
    privacy: PrivacyClass,
    payload: serde_json::Value,
    created_unix_ms: i64,
}

#[derive(serde::Serialize)]
struct DebugDbSessionReport {
    session_id: String,
    program_hash: Option<String>,
    profile: String,
    transport: String,
    started_unix_ms: i64,
    ended_unix_ms: Option<i64>,
    status: DebugSessionStatus,
    metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(serde::Serialize)]
struct DebugDbDeleteReport {
    path: String,
    blob_dir: Option<String>,
    user_version: u32,
    deleted_unreferenced_blobs: u64,
    deleted_unreferenced_blob_files: u64,
    deleted_unreferenced_blob_file_bytes: u64,
    missing_unreferenced_blob_files: u64,
    unsafe_unreferenced_blob_paths: u64,
    validation: Option<DebugDbValidationCliReport>,
}

#[derive(serde::Serialize)]
struct DebugDbSearchReport {
    path: String,
    query: Option<String>,
    query_vector_dimensions: Option<usize>,
    graph_query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    graph_depth: Option<u32>,
    history_query: Option<String>,
    model: Option<DebugDbSearchModelReport>,
    limit: usize,
    max_privacy: PrivacyClass,
    hits: Vec<DebugDbSearchHitReport>,
}

#[derive(serde::Serialize)]
struct DebugDbSearchModelReport {
    model_id: String,
    model_revision: String,
    dimensions: u32,
}

#[derive(serde::Serialize)]
struct DebugDbSearchHitReport {
    chunk_id: String,
    title: String,
    body: String,
    source_kind: String,
    source_key: String,
    privacy: PrivacyClass,
    channel: String,
    rank: usize,
    score: Option<f64>,
}

#[derive(serde::Serialize)]
struct DebugDbBlobFileValidationReport {
    root: String,
    checked: u64,
    missing: u64,
    byte_len_mismatches: u64,
    unsafe_relative_paths: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct DebugDbBlobFileDeleteReport {
    deleted: u64,
    bytes: u64,
    missing: u64,
    unsafe_paths: u64,
}

pub(super) fn debug_command(command: DebugCommand) -> Result<(), ExitCode> {
    match command {
        DebugCommand::Db { command } => debug_db_command(command),
    }
}

fn debug_db_command(command: DebugDbCommand) -> Result<(), ExitCode> {
    match command {
        DebugDbCommand::Status(options) | DebugDbCommand::Migrate(options) => {
            debug_db_status_or_migrate_command(&options)
        }
        DebugDbCommand::Validate(options) => debug_db_validate_command(&options),
        DebugDbCommand::Reindex(options) => debug_db_reindex_command(&options),
        DebugDbCommand::Vacuum(options) => debug_db_vacuum_command(&options),
        DebugDbCommand::Sessions(options) => debug_db_sessions_command(&options),
        DebugDbCommand::Runs(options) => debug_db_runs_command(&options),
        DebugDbCommand::Rag(options) => debug_db_rag_command(&options),
        DebugDbCommand::Timeline(options) => debug_db_timeline_command(&options),
        DebugDbCommand::Search(options) => debug_db_search_command(&options),
        DebugDbCommand::Delete(options) => debug_db_delete_command(&options),
    }
}

fn debug_db_status_or_migrate_command(options: &DebugDbOptions) -> Result<(), ExitCode> {
    let report = open_debug_db(options)?;
    if options.json {
        return print_json(&report);
    }
    println!(
        "{}: schema version {}, chunks {}, blobs {}, captures {}",
        report.path,
        report.user_version,
        report.stats.chunks,
        report.stats.blobs,
        report.stats.captures
    );
    Ok(())
}

fn debug_db_validate_command(options: &DebugDbOptions) -> Result<(), ExitCode> {
    let (store, path, user_version, stats) = open_debug_store(options)?;
    let validation = store.validate().map_err(|error| {
        eprintln!(
            "error: failed to validate debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let report = validation_report(&store, path, user_version, stats, validation, options)
        .map_err(|error| {
            eprintln!(
                "error: failed to validate debug blob files for {}: {error}",
                options.path.display()
            );
            ExitCode::FAILURE
        })?;
    if options.json {
        return print_json(&report);
    }
    println!(
        "{}: {}",
        report.path,
        if report.valid { "valid" } else { "invalid" }
    );
    println!(
        "integrity_messages={}, foreign_key_violations={}, missing_capture_blob_refs={}, invalid_embedding_blobs={}",
        report.integrity_messages.len(),
        report.foreign_key_violations.len(),
        report.missing_capture_blob_refs,
        report.invalid_embedding_blobs
    );
    if let Some(blob_files) = &report.blob_files {
        println!(
            "blob_files: checked={}, missing={}, byte_len_mismatches={}, unsafe_relative_paths={}",
            blob_files.checked,
            blob_files.missing,
            blob_files.byte_len_mismatches,
            blob_files.unsafe_relative_paths
        );
    }
    Ok(())
}

fn debug_db_reindex_command(options: &DebugDbOptions) -> Result<(), ExitCode> {
    let (store, path, user_version, _) = open_debug_store(options)?;
    let report = store.reindex().map_err(|error| {
        eprintln!(
            "error: failed to reindex debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let report = DebugDbReindexReport {
        path,
        user_version,
        chunks_indexed: report.chunks_indexed,
    };
    if options.json {
        return print_json(&report);
    }
    println!(
        "{}: rebuilt chunk FTS index for {} chunks",
        report.path, report.chunks_indexed
    );
    Ok(())
}

fn debug_db_vacuum_command(options: &DebugDbOptions) -> Result<(), ExitCode> {
    let (store, path, user_version, _) = open_debug_store(options)?;
    let vacuum = store.vacuum().map_err(|error| {
        eprintln!(
            "error: failed to vacuum debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let report = DebugDbVacuumReport {
        path,
        user_version,
        page_count_before: vacuum.page_count_before,
        freelist_count_before: vacuum.freelist_count_before,
        page_count_after: vacuum.page_count_after,
        freelist_count_after: vacuum.freelist_count_after,
    };
    if options.json {
        return print_json(&report);
    }
    println!(
        "{}: vacuumed pages {} -> {}, freelist {} -> {}",
        report.path,
        report.page_count_before,
        report.page_count_after,
        report.freelist_count_before,
        report.freelist_count_after
    );
    Ok(())
}

fn debug_db_sessions_command(options: &DebugDbSessionsOptions) -> Result<(), ExitCode> {
    if options.limit == 0 {
        eprintln!("error: debug db sessions --limit must be at least 1");
        return Err(ExitCode::from(2));
    }
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let sessions = store.sessions(options.limit).map_err(|error| {
        eprintln!(
            "error: failed to read debug sessions from {}: {error}",
            options.db.path.display()
        );
        ExitCode::FAILURE
    })?;
    let report = DebugDbSessionsReport {
        path,
        user_version,
        limit: options.limit,
        sessions: sessions.into_iter().map(debug_db_session_report).collect(),
    };
    if options.db.json {
        return print_json(&report);
    }
    println!("{}: {} session(s)", report.path, report.sessions.len());
    for session in &report.sessions {
        println!(
            "{} {} profile={} transport={} started={} ended={}",
            session.session_id,
            session.status.as_str(),
            session.profile,
            session.transport,
            session.started_unix_ms,
            session
                .ended_unix_ms
                .map_or_else(|| "-".to_owned(), |value| value.to_string())
        );
    }
    Ok(())
}

fn debug_db_runs_command(options: &DebugDbRunsOptions) -> Result<(), ExitCode> {
    if options.limit == 0 {
        eprintln!("error: debug db runs --limit must be at least 1");
        return Err(ExitCode::from(2));
    }
    let session_id = options
        .session_id
        .as_deref()
        .map(SessionId::new)
        .transpose()
        .map_err(|error| {
            eprintln!("error: invalid debug db runs --session-id: {error}");
            ExitCode::from(2)
        })?;
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let runs = store
        .script_runs(session_id.as_ref(), options.limit)
        .map_err(|error| {
            eprintln!(
                "error: failed to read debug script runs from {}: {error}",
                options.db.path.display()
            );
            ExitCode::FAILURE
        })?;
    let report = DebugDbRunsReport {
        path,
        user_version,
        session_id: session_id.as_ref().map(|id| id.as_str().to_owned()),
        limit: options.limit,
        runs: runs.into_iter().map(debug_db_run_report).collect(),
    };
    if options.db.json {
        return print_json(&report);
    }
    println!("{}: {} run(s)", report.path, report.runs.len());
    for run in &report.runs {
        println!(
            "{} {} session={} seq={}..{} agent={}",
            run.run_id,
            run.outcome.as_str(),
            run.session_id,
            run.started_sequence,
            run.finished_sequence
                .map_or_else(|| "-".to_owned(), |value| value.to_string()),
            run.agent_id.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}

fn debug_db_rag_command(options: &DebugDbRagOptions) -> Result<(), ExitCode> {
    let query_id = options.query_id.trim();
    if query_id.is_empty() {
        eprintln!("error: debug db rag --query-id must not be empty");
        return Err(ExitCode::from(2));
    }
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let audit = store
        .rag_query_audit_with_max_privacy(query_id, options.max_privacy)
        .map_err(|error| {
            eprintln!(
                "error: failed to read persisted RAG query from {}: {error}",
                options.db.path.display()
            );
            ExitCode::FAILURE
        })?;
    let report = DebugDbRagReport {
        path,
        user_version,
        query_id: query_id.to_owned(),
        max_privacy: options.max_privacy,
        status: audit.status,
        created_unix_ms: audit.created_unix_ms,
        pack: audit.pack,
    };
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: RAG query {} status={} item(s)={} truncated={} max_privacy={}",
        report.path,
        report.query_id,
        report.status,
        report.pack.items.len(),
        report.pack.truncated,
        report.max_privacy.as_str()
    );
    for item in &report.pack.items {
        println!(
            "- {} [{}] score={:.6}",
            item.title,
            item.chunk_id.as_str(),
            item.fused_score
        );
    }
    Ok(())
}

fn debug_db_timeline_command(options: &DebugDbTimelineOptions) -> Result<(), ExitCode> {
    if options.limit == 0 {
        eprintln!("error: debug db timeline --limit must be at least 1");
        return Err(ExitCode::from(2));
    }
    let session_id = options
        .session_id
        .as_deref()
        .map(SessionId::new)
        .transpose()
        .map_err(|error| {
            eprintln!("error: invalid debug db timeline --session-id: {error}");
            ExitCode::from(2)
        })?;
    let run_id = options
        .run_id
        .as_deref()
        .map(AgentRunId::new)
        .transpose()
        .map_err(|error| {
            eprintln!("error: invalid debug db timeline --run-id: {error}");
            ExitCode::from(2)
        })?;
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let events = store
        .session_timeline_with_max_privacy(
            session_id.as_ref().map(SessionId::as_str),
            run_id.as_ref().map(AgentRunId::as_str),
            options.limit,
            options.max_privacy,
        )
        .map_err(|error| {
            eprintln!(
                "error: failed to read debug timeline from {}: {error}",
                options.db.path.display()
            );
            ExitCode::FAILURE
        })?;
    let report = DebugDbTimelineReport {
        path,
        user_version,
        session_id: session_id.as_ref().map(|id| id.as_str().to_owned()),
        run_id: run_id.as_ref().map(|id| id.as_str().to_owned()),
        limit: options.limit,
        max_privacy: options.max_privacy,
        events: events
            .into_iter()
            .map(debug_db_timeline_event_report)
            .collect(),
    };
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: {} timeline event(s) max_privacy={}",
        report.path,
        report.events.len(),
        report.max_privacy.as_str()
    );
    for event in &report.events {
        println!(
            "{} {} session={} run={} tick={}",
            event.sequence,
            event.kind,
            event.session_id,
            event.run_id.as_deref().unwrap_or("-"),
            event
                .tick
                .map_or_else(|| "-".to_owned(), |tick| tick.to_string())
        );
    }
    Ok(())
}

fn debug_db_search_command(options: &DebugDbSearchOptions) -> Result<(), ExitCode> {
    let report = debug_db_search_report(options)?;
    if options.db.json {
        return print_json(&report);
    }
    print_debug_db_search_report(&report);
    Ok(())
}

fn debug_db_search_report(options: &DebugDbSearchOptions) -> Result<DebugDbSearchReport, ExitCode> {
    let query = options
        .query
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty());
    let query_vector = options
        .query_vector
        .as_deref()
        .map(parse_debug_query_vector)
        .transpose()
        .map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::from(2)
        })?;
    let has_query_vector = query_vector.is_some();
    let graph_query = options
        .graph_query
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty());
    let history_query = options
        .history_query
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty());
    let selector_count = usize::from(query.is_some())
        + usize::from(has_query_vector)
        + usize::from(graph_query.is_some())
        + usize::from(history_query.is_some());
    validate_debug_db_search_selection(selector_count, options.limit)?;
    let (store, path, _, _) = open_debug_store(&options.db)?;
    let model = query_vector
        .as_ref()
        .map(|vector| {
            debug_search_model(options, vector.len()).map_err(|error| {
                eprintln!("error: {error}");
                ExitCode::from(2)
            })
        })
        .transpose()?;
    let hits = debug_db_search_hits(
        &store,
        options,
        query,
        query_vector.as_deref(),
        graph_query,
        history_query,
        model.as_ref(),
    )
    .map_err(|error| {
        eprintln!(
            "error: failed to search debug database {}: {error}",
            options.db.path.display()
        );
        ExitCode::FAILURE
    })?;
    let hits = hits
        .into_iter()
        .map(|result| DebugDbSearchHitReport {
            chunk_id: result.hit.chunk_id.as_str().to_owned(),
            title: result.title,
            body: result.body,
            source_kind: result.source_kind,
            source_key: result.source_key,
            privacy: result.privacy,
            channel: debug_search_channel_label(result.hit.channel).to_owned(),
            rank: result.hit.rank,
            score: result.hit.score,
        })
        .collect::<Vec<_>>();
    Ok(DebugDbSearchReport {
        path,
        query: query.map(str::to_owned),
        query_vector_dimensions: query_vector.as_ref().map(Vec::len),
        graph_query: graph_query.map(str::to_owned),
        graph_depth: graph_query.map(|_| options.graph_depth),
        history_query: history_query.map(str::to_owned),
        model: model.map(|model| DebugDbSearchModelReport {
            model_id: model.model_id,
            model_revision: model.model_revision,
            dimensions: model.dimensions,
        }),
        limit: options.limit,
        max_privacy: options.max_privacy,
        hits,
    })
}

fn debug_db_session_report(session: DebugSession) -> DebugDbSessionReport {
    DebugDbSessionReport {
        session_id: session.session_id.as_str().to_owned(),
        program_hash: session.program_hash.map(|hash| hash.as_str().to_owned()),
        profile: session.profile,
        transport: session.transport,
        started_unix_ms: session.started_unix_ms,
        ended_unix_ms: session.ended_unix_ms,
        status: session.status,
        metadata: session.metadata,
    }
}

fn debug_db_run_report(run: DebugScriptRun) -> DebugDbRunReport {
    DebugDbRunReport {
        run_id: run.run_id.as_str().to_owned(),
        session_id: run.session_id.as_str().to_owned(),
        agent_id: run.agent_id.map(|id| id.as_str().to_owned()),
        artifact_hash: run.artifact_hash.map(|hash| hash.as_str().to_owned()),
        source_hash: run.source_hash.map(|hash| hash.as_str().to_owned()),
        project_binding_mode: run.project_binding_mode,
        started_sequence: run.started_sequence,
        finished_sequence: run.finished_sequence,
        outcome: run.outcome,
        partially_effectful: run.partially_effectful,
        trace_uri: run.trace_uri,
        error: run.error,
        metadata: run.metadata,
    }
}

fn debug_db_timeline_event_report(event: DebugTimelineEvent) -> DebugDbTimelineEventReport {
    DebugDbTimelineEventReport {
        session_id: event.session_id,
        run_id: event.run_id,
        sequence: event.sequence,
        tick: event.tick,
        kind: event.event_kind,
        privacy: event.privacy,
        payload: event.payload,
        created_unix_ms: event.created_unix_ms,
    }
}

fn validate_debug_db_search_selection(selector_count: usize, limit: usize) -> Result<(), ExitCode> {
    if selector_count == 0 {
        eprintln!(
            "error: debug db search requires one of --query, --query-vector, --graph-query, or --history-query"
        );
        return Err(ExitCode::from(2));
    }
    if selector_count > 1 {
        eprintln!(
            "error: debug db search accepts only one of --query, --query-vector, --graph-query, or --history-query"
        );
        return Err(ExitCode::from(2));
    }
    if limit == 0 {
        eprintln!("error: debug db search --limit must be at least 1");
        return Err(ExitCode::from(2));
    }
    Ok(())
}

fn print_debug_db_search_report(report: &DebugDbSearchReport) {
    println!(
        "{}: {} hit(s) for {:?} (max_privacy={})",
        report.path,
        report.hits.len(),
        report.query,
        report.max_privacy.as_str()
    );
    for hit in &report.hits {
        println!(
            "{}. {} [{}:{} privacy={} score={}]",
            hit.rank,
            hit.title,
            hit.source_kind,
            hit.source_key,
            hit.privacy.as_str(),
            hit.score
                .map_or_else(|| "none".to_owned(), |score| format!("{score:.6}"))
        );
    }
}

fn debug_db_search_hits(
    store: &DebugStore,
    options: &DebugDbSearchOptions,
    query: Option<&str>,
    query_vector: Option<&[f32]>,
    graph_query: Option<&str>,
    history_query: Option<&str>,
    model: Option<&EmbeddingModelDescriptor>,
) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
    if let Some(query) = query {
        return store.lexical_search_with_max_privacy(query, options.limit, options.max_privacy);
    }
    if let Some(query) = graph_query {
        return store.graph_search_with_depth_and_max_privacy(
            query,
            options.graph_depth,
            options.limit,
            options.max_privacy,
        );
    }
    if let Some(query) = history_query {
        return store.history_search_with_max_privacy(query, options.limit, options.max_privacy);
    }
    let vector = query_vector.expect("query vector is validated before search");
    let model = model.expect("embedding model is validated before vector search");
    store.vector_search_with_max_privacy(model, vector, options.limit, options.max_privacy)
}

fn debug_search_model(
    options: &DebugDbSearchOptions,
    dimensions: usize,
) -> Result<EmbeddingModelDescriptor, String> {
    let model_id = options
        .model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "debug db search --query-vector requires --model-id".to_owned())?;
    let model_revision = options
        .model_revision
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "debug db search --query-vector requires --model-revision".to_owned())?;
    let dimensions = u32::try_from(dimensions)
        .map_err(|_| "debug db search --query-vector has too many dimensions".to_owned())?;
    Ok(EmbeddingModelDescriptor {
        model_id: model_id.to_owned(),
        model_revision: model_revision.to_owned(),
        dimensions,
    })
}

fn parse_debug_query_vector(value: &str) -> Result<Vec<f32>, String> {
    let values = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f32>()
                .map_err(|error| format!("invalid --query-vector component `{part}`: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.is_empty() {
        return Err("debug db search --query-vector must contain at least one value".to_owned());
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err("debug db search --query-vector must contain only finite values".to_owned());
    }
    Ok(values)
}

fn debug_db_delete_command(options: &DebugDbDeleteOptions) -> Result<(), ExitCode> {
    if !options.unreferenced_blobs {
        eprintln!("error: debug db delete requires --unreferenced-blobs");
        return Err(ExitCode::from(2));
    }
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let unreferenced_blob_records = if options.db.blob_dir.is_some() {
        store.unreferenced_blob_records().map_err(|error| {
            eprintln!(
                "error: failed to list unreferenced blob records from debug database {}: {error}",
                options.db.path.display()
            );
            ExitCode::FAILURE
        })?
    } else {
        Vec::new()
    };
    let deleted_unreferenced_blobs = store.delete_unreferenced_blobs().map_err(|error| {
        eprintln!(
            "error: failed to delete unreferenced blob records from debug database {}: {error}",
            options.db.path.display()
        );
        ExitCode::FAILURE
    })?;
    let deleted_files = if let Some(blob_dir) = &options.db.blob_dir {
        delete_blob_files(blob_dir, &unreferenced_blob_records)?
    } else {
        DebugDbBlobFileDeleteReport::default()
    };
    let validation = options
        .validate
        .then(|| post_delete_validation(&store, path.clone(), user_version, &options.db))
        .transpose()?;
    let report = DebugDbDeleteReport {
        path,
        blob_dir: options
            .db
            .blob_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        user_version,
        deleted_unreferenced_blobs,
        deleted_unreferenced_blob_files: deleted_files.deleted,
        deleted_unreferenced_blob_file_bytes: deleted_files.bytes,
        missing_unreferenced_blob_files: deleted_files.missing,
        unsafe_unreferenced_blob_paths: deleted_files.unsafe_paths,
        validation,
    };
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: deleted {} unreferenced blob records",
        report.path, report.deleted_unreferenced_blobs
    );
    if report.blob_dir.is_some() {
        println!(
            "blob files: deleted={}, bytes={}, missing={}, unsafe_paths={}",
            report.deleted_unreferenced_blob_files,
            report.deleted_unreferenced_blob_file_bytes,
            report.missing_unreferenced_blob_files,
            report.unsafe_unreferenced_blob_paths
        );
    }
    if let Some(validation) = &report.validation {
        println!(
            "validation: {}",
            if validation.valid { "valid" } else { "invalid" }
        );
    }
    Ok(())
}

fn post_delete_validation(
    store: &DebugStore,
    path: String,
    user_version: u32,
    options: &DebugDbOptions,
) -> Result<DebugDbValidationCliReport, ExitCode> {
    let stats = store.stats().map_err(|error| {
        eprintln!(
            "error: failed to read debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let validation = store.validate().map_err(|error| {
        eprintln!(
            "error: failed to validate debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    validation_report(
        store,
        path,
        user_version,
        stats_report(stats),
        validation,
        options,
    )
    .map_err(|error| {
        eprintln!(
            "error: failed to validate debug blob files for {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })
}

fn open_debug_db(options: &DebugDbOptions) -> Result<DebugDbReport, ExitCode> {
    let (_store, path, user_version, stats) = open_debug_store(options)?;
    Ok(DebugDbReport {
        path,
        user_version,
        stats,
    })
}

fn open_debug_store(
    options: &DebugDbOptions,
) -> Result<(DebugStore, String, u32, DebugDbStatsReport), ExitCode> {
    if let Some(parent) = options.path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            eprintln!("error: failed to create {}: {error}", parent.display());
            ExitCode::FAILURE
        })?;
    }
    let store = DebugStore::open(&options.path).map_err(|error| {
        eprintln!(
            "error: failed to open debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let user_version = store.user_version().map_err(|error| {
        eprintln!(
            "error: failed to read debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let stats = store.stats().map_err(|error| {
        eprintln!(
            "error: failed to read debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    Ok((
        store,
        options.path.display().to_string(),
        user_version,
        stats_report(stats),
    ))
}

fn validation_report(
    store: &DebugStore,
    path: String,
    user_version: u32,
    stats: DebugDbStatsReport,
    validation: DebugStoreValidationReport,
    options: &DebugDbOptions,
) -> Result<DebugDbValidationCliReport, String> {
    let blob_files = options
        .blob_dir
        .as_ref()
        .map(|blob_dir| validate_blob_files(store, blob_dir))
        .transpose()?;
    let valid = validation.integrity_messages.is_empty()
        && validation.foreign_key_violations.is_empty()
        && validation.missing_capture_blob_refs == 0
        && validation.invalid_embedding_blobs == 0
        && blob_files.as_ref().is_none_or(|blob_files| {
            blob_files.missing == 0
                && blob_files.byte_len_mismatches == 0
                && blob_files.unsafe_relative_paths == 0
        });
    Ok(DebugDbValidationCliReport {
        path,
        blob_dir: options
            .blob_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        user_version,
        valid,
        integrity_messages: validation.integrity_messages,
        foreign_key_violations: validation
            .foreign_key_violations
            .into_iter()
            .map(foreign_key_violation_report)
            .collect(),
        missing_capture_blob_refs: validation.missing_capture_blob_refs,
        invalid_embedding_blobs: validation.invalid_embedding_blobs,
        blob_files,
        stats,
    })
}

fn validate_blob_files(
    store: &DebugStore,
    blob_dir: &Path,
) -> Result<DebugDbBlobFileValidationReport, String> {
    let records = store
        .blob_records()
        .map_err(|error| format!("failed to list debug blob records: {error}"))?;
    let mut report = DebugDbBlobFileValidationReport {
        root: blob_dir.display().to_string(),
        checked: 0,
        missing: 0,
        byte_len_mismatches: 0,
        unsafe_relative_paths: 0,
    };
    for record in records {
        report.checked = report.checked.saturating_add(1);
        let Some(path) = checked_blob_file_path(blob_dir, &record.relative_path) else {
            report.unsafe_relative_paths = report.unsafe_relative_paths.saturating_add(1);
            continue;
        };
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.missing = report.missing.saturating_add(1);
                continue;
            }
            Err(error) => {
                return Err(format!("failed to stat {}: {error}", path.display()));
            }
        };
        if metadata.len() != record.byte_len {
            report.byte_len_mismatches = report.byte_len_mismatches.saturating_add(1);
        }
    }
    Ok(report)
}

fn delete_blob_files(
    blob_dir: &Path,
    records: &[DebugStoreBlobRecord],
) -> Result<DebugDbBlobFileDeleteReport, ExitCode> {
    let mut report = DebugDbBlobFileDeleteReport::default();
    for record in records {
        let Some(path) = checked_blob_file_path(blob_dir, &record.relative_path) else {
            report.unsafe_paths = report.unsafe_paths.saturating_add(1);
            continue;
        };
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.missing = report.missing.saturating_add(1);
                continue;
            }
            Err(error) => {
                eprintln!(
                    "error: failed to stat blob file {}: {error}",
                    path.display()
                );
                return Err(ExitCode::FAILURE);
            }
        };
        fs::remove_file(&path).map_err(|error| {
            eprintln!(
                "error: failed to delete blob file {}: {error}",
                path.display()
            );
            ExitCode::FAILURE
        })?;
        report.deleted = report.deleted.saturating_add(1);
        report.bytes = report.bytes.saturating_add(metadata.len());
    }
    Ok(report)
}

fn checked_blob_file_path(root: &Path, relative_path: &str) -> Option<PathBuf> {
    let relative = Path::new(relative_path);
    let mut checked = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => checked.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if checked.as_os_str().is_empty() {
        return None;
    }
    Some(root.join(checked))
}

fn parse_debug_privacy_class(value: &str) -> Result<PrivacyClass, String> {
    PrivacyClass::parse(value).ok_or_else(|| {
        format!("privacy class must be one of public, project, sensitive, or secret: `{value}`")
    })
}

const fn debug_search_channel_label(channel: SearchChannel) -> &'static str {
    match channel {
        SearchChannel::ExactEntity => "exact_entity",
        SearchChannel::Lexical => "lexical",
        SearchChannel::Vector => "vector",
        SearchChannel::Graph => "graph",
        SearchChannel::History => "history",
        SearchChannel::Diagnostics => "diagnostics",
        SearchChannel::Trace => "trace",
        SearchChannel::Summary => "summary",
    }
}

fn foreign_key_violation_report(
    violation: DebugStoreForeignKeyViolation,
) -> DebugDbForeignKeyViolationReport {
    DebugDbForeignKeyViolationReport {
        table: violation.table,
        rowid: violation.rowid,
        parent: violation.parent,
        fkid: violation.fkid,
    }
}

fn stats_report(stats: DebugStoreStats) -> DebugDbStatsReport {
    DebugDbStatsReport {
        programs: stats.programs,
        sessions: stats.sessions,
        script_runs: stats.script_runs,
        debug_events: stats.debug_events,
        frames: stats.frames,
        actions: stats.actions,
        captures: stats.captures,
        blobs: stats.blobs,
        chunks: stats.chunks,
        embeddings: stats.embeddings,
        rag_queries: stats.rag_queries,
        repl_cells: stats.repl_cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_blob_file_path_rejects_paths_outside_blob_dir() {
        let root = Path::new("blob-root");

        assert_eq!(
            checked_blob_file_path(root, "blake3/abc"),
            Some(root.join("blake3").join("abc"))
        );
        assert!(checked_blob_file_path(root, "../escape").is_none());
        assert!(checked_blob_file_path(root, "/absolute").is_none());
        assert!(checked_blob_file_path(root, "").is_none());
    }

    #[test]
    fn delete_blob_files_removes_only_safe_existing_files() {
        let root = std::env::temp_dir().join(format!(
            "arcweft-debug-blob-delete-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("blake3")).expect("create blob dir");
        let deleted_path = root.join("blake3").join("deleted");
        fs::write(&deleted_path, [1_u8, 2, 3]).expect("write blob");

        let records = vec![
            DebugStoreBlobRecord {
                blob_hash: "blob:deleted".to_owned(),
                byte_len: 3,
                relative_path: "blake3/deleted".to_owned(),
            },
            DebugStoreBlobRecord {
                blob_hash: "blob:missing".to_owned(),
                byte_len: 1,
                relative_path: "blake3/missing".to_owned(),
            },
            DebugStoreBlobRecord {
                blob_hash: "blob:unsafe".to_owned(),
                byte_len: 1,
                relative_path: "../unsafe".to_owned(),
            },
        ];

        let report = delete_blob_files(&root, &records).expect("delete blob files");
        assert_eq!(report.deleted, 1);
        assert_eq!(report.bytes, 3);
        assert_eq!(report.missing, 1);
        assert_eq!(report.unsafe_paths, 1);
        assert!(!deleted_path.exists());

        fs::remove_dir_all(&root).expect("remove blob dir");
    }
}
