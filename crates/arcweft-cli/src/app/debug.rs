use super::local_embedding::{
    DEFAULT_LOCAL_EMBEDDING_DIMENSIONS, DEFAULT_LOCAL_EMBEDDING_MODEL_ID,
    DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION, LocalHashEmbeddingProvider,
    MAX_LOCAL_EMBEDDING_DIMENSIONS,
};
use super::remote_embedding::RemoteCommandEmbeddingProvider;
use super::shared::print_json;
use arcweft_agent_protocol::ids::{AgentRunId, SessionId, StableHash};
use arcweft_debug_model::chunk::PrivacyClass;
use arcweft_debug_model::diagnostic::DebugDiagnostic;
use arcweft_debug_model::embedding::{
    EmbeddingInput, EmbeddingInputPolicy, EmbeddingModelDescriptor, EmbeddingProvider,
};
use arcweft_debug_model::graph::{DebugGraphEdge, DebugGraphSymbol};
use arcweft_debug_model::rag::{RagContextPack, SearchChannel};
use arcweft_debug_model::repl::DebugReplCell;
use arcweft_debug_model::script::{DebugScriptRun, DebugScriptRunOutcome};
use arcweft_debug_model::session::{DebugSession, DebugSessionStatus};
use arcweft_debug_model::source::DebugSourceFile;
use arcweft_debug_sqlite::store::{
    ChunkSearchResult, DebugStore, DebugStoreBlobRecord, DebugStoreError,
    DebugStoreForeignKeyViolation, DebugStoreStats, DebugStoreValidationReport, DebugTimelineEvent,
};
use clap::{Args, Subcommand};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

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
    Prune(DebugDbPruneOptions),
    Vacuum(DebugDbOptions),
    Sessions(DebugDbSessionsOptions),
    Sources(DebugDbSourcesOptions),
    Graph(DebugDbGraphOptions),
    CloseStaleSessions(DebugDbCloseStaleSessionsOptions),
    Runs(DebugDbRunsOptions),
    Rag(DebugDbRagOptions),
    Timeline(DebugDbTimelineOptions),
    ReplCells(DebugDbReplCellsOptions),
    Embed(DebugDbEmbedOptions),
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
pub(super) struct DebugDbPruneOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long = "older-than", value_parser = parse_debug_retention_duration_millis)]
    older_than_millis: i64,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbSessionsOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long, value_parser = parse_debug_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbSourcesOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long = "program-hash")]
    program_hash: String,
    #[arg(long, value_parser = parse_debug_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbGraphOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long = "program-hash")]
    program_hash: String,
    #[arg(long, value_parser = parse_debug_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbCloseStaleSessionsOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long = "stale-after", value_parser = parse_debug_retention_duration_millis)]
    stale_after_millis: i64,
    #[arg(long, default_value = "stale_running_session")]
    reason: String,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbRunsOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long = "session-id")]
    session_id: Option<String>,
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long, value_parser = parse_debug_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
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
pub(super) struct DebugDbReplCellsOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long = "session-id")]
    session_id: String,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbEmbedOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long, value_enum, default_value = "local-hash")]
    provider: DebugDbEmbeddingProvider,
    #[arg(long = "model-id", default_value = DEFAULT_LOCAL_EMBEDDING_MODEL_ID)]
    model_id: String,
    #[arg(long = "model-revision", default_value = DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION)]
    model_revision: String,
    #[arg(long = "dimensions", default_value_t = DEFAULT_LOCAL_EMBEDDING_DIMENSIONS)]
    dimensions: u32,
    #[arg(long = "remote-command")]
    remote_command: Option<String>,
    #[arg(long = "remote-arg")]
    remote_args: Vec<String>,
    #[arg(long, value_parser = parse_debug_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
enum DebugDbEmbeddingProvider {
    LocalHash,
    Remote,
}

impl DebugDbEmbeddingProvider {
    const fn label(self) -> &'static str {
        match self {
            Self::LocalHash => "local_hash",
            Self::Remote => "remote",
        }
    }

    const fn scope_label(self) -> &'static str {
        match self {
            Self::LocalHash => "local",
            Self::Remote => "remote",
        }
    }

    const fn input_policy(self, max_privacy: PrivacyClass) -> EmbeddingInputPolicy {
        match self {
            Self::LocalHash => EmbeddingInputPolicy::local(max_privacy),
            Self::Remote => EmbeddingInputPolicy::remote(max_privacy),
        }
    }
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
    #[arg(long = "diagnostic-query")]
    diagnostic_query: Option<String>,
    #[arg(long = "test-query")]
    test_query: Option<String>,
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
    source_files: u64,
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
struct DebugDbPruneReport {
    path: String,
    user_version: u32,
    older_than_millis: i64,
    cutoff_unix_ms: i64,
    deleted: DebugDbPruneDeletedReport,
    stats_after: DebugDbStatsReport,
}

#[derive(serde::Serialize)]
struct DebugDbPruneDeletedReport {
    sessions: u64,
    rag_queries: u64,
    chunks: u64,
    diagnostics: u64,
    history_entries: u64,
    test_results: u64,
    blobs: u64,
    programs: u64,
}

#[derive(serde::Serialize)]
struct DebugDbSessionsReport {
    path: String,
    user_version: u32,
    limit: usize,
    max_privacy: PrivacyClass,
    sessions: Vec<DebugDbSessionReport>,
}

#[derive(serde::Serialize)]
struct DebugDbSourcesReport {
    path: String,
    user_version: u32,
    program_hash: String,
    max_privacy: PrivacyClass,
    sources: Vec<DebugDbSourceFileReport>,
}

#[derive(serde::Serialize)]
struct DebugDbGraphReport {
    path: String,
    user_version: u32,
    program_hash: String,
    max_privacy: PrivacyClass,
    symbol_count: usize,
    edge_count: usize,
    symbols: Vec<DebugDbGraphSymbolReport>,
    edges: Vec<DebugDbGraphEdgeReport>,
}

#[derive(serde::Serialize)]
struct DebugDbCloseStaleSessionsReport {
    path: String,
    user_version: u32,
    stale_after_millis: i64,
    cutoff_unix_ms: i64,
    closed_unix_ms: i64,
    reason: String,
    dry_run: bool,
    matched_sessions: Vec<DebugDbSessionReport>,
    closed_sessions: Vec<DebugDbSessionReport>,
}

#[derive(serde::Serialize)]
struct DebugDbRunsReport {
    path: String,
    user_version: u32,
    session_id: Option<String>,
    limit: usize,
    max_privacy: PrivacyClass,
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
    project: Option<serde_json::Value>,
    metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(serde::Serialize)]
struct DebugDbSourceFileReport {
    program_hash: String,
    path: String,
    language: String,
    content_hash: String,
    byte_len: u64,
    metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(serde::Serialize)]
struct DebugDbGraphSymbolReport {
    program_hash: String,
    symbol_id: String,
    public_id: Option<String>,
    qualified_name: Option<String>,
    kind: String,
    type_json: Option<serde_json::Value>,
    source_path: Option<String>,
    source_content_hash: Option<String>,
    start_byte: Option<u64>,
    end_byte: Option<u64>,
    semantic_hash: Option<String>,
    summary: String,
    metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(serde::Serialize)]
struct DebugDbGraphEdgeReport {
    program_hash: String,
    from_symbol_id: String,
    to_symbol_id: String,
    edge_kind: String,
    weight: f64,
    metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(serde::Serialize)]
struct DebugDbRagReport {
    path: String,
    user_version: u32,
    query_id: String,
    session_id: Option<String>,
    run_id: Option<String>,
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
struct DebugDbReplCellsReport {
    path: String,
    user_version: u32,
    session_id: String,
    limit: usize,
    cells: Vec<DebugDbReplCellReport>,
}

#[derive(serde::Serialize)]
struct DebugDbReplCellReport {
    cell_id: String,
    session_id: String,
    run_id: Option<String>,
    ordinal: i64,
    source: String,
    source_hash: String,
    status: String,
    inferred_type: Option<serde_json::Value>,
    display: Option<serde_json::Value>,
    partially_effectful: bool,
    diagnostic_ids: Vec<String>,
    created_unix_ms: i64,
}

#[derive(serde::Serialize)]
struct DebugDbEmbedReport {
    path: String,
    user_version: u32,
    provider: String,
    scope: String,
    model: DebugDbSearchModelReport,
    max_privacy: PrivacyClass,
    input_chunks: usize,
    embedded_chunks: usize,
    skipped_chunks: u64,
    embedded_chunk_ids: Vec<String>,
    stats_after: DebugDbStatsReport,
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
    project: Option<serde_json::Value>,
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
    diagnostic_query: Option<String>,
    test_query: Option<String>,
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
        DebugDbCommand::Prune(options) => debug_db_prune_command(&options),
        DebugDbCommand::Vacuum(options) => debug_db_vacuum_command(&options),
        DebugDbCommand::Sessions(options) => debug_db_sessions_command(&options),
        DebugDbCommand::Sources(options) => debug_db_sources_command(&options),
        DebugDbCommand::Graph(options) => debug_db_graph_command(&options),
        DebugDbCommand::CloseStaleSessions(options) => {
            debug_db_close_stale_sessions_command(&options)
        }
        DebugDbCommand::Runs(options) => debug_db_runs_command(&options),
        DebugDbCommand::Rag(options) => debug_db_rag_command(&options),
        DebugDbCommand::Timeline(options) => debug_db_timeline_command(&options),
        DebugDbCommand::ReplCells(options) => debug_db_repl_cells_command(&options),
        DebugDbCommand::Embed(options) => debug_db_embed_command(&options),
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

fn debug_db_prune_command(options: &DebugDbPruneOptions) -> Result<(), ExitCode> {
    let now_unix_ms = current_unix_millis().map_err(|error| {
        eprintln!("error: failed to read system clock for debug db prune: {error}");
        ExitCode::FAILURE
    })?;
    let cutoff_unix_ms = now_unix_ms.saturating_sub(options.older_than_millis);
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let deleted = store.prune_before(cutoff_unix_ms).map_err(|error| {
        eprintln!(
            "error: failed to prune debug database {}: {error}",
            options.db.path.display()
        );
        ExitCode::FAILURE
    })?;
    let stats_after = store.stats().map_err(|error| {
        eprintln!(
            "error: failed to read debug database {} after prune: {error}",
            options.db.path.display()
        );
        ExitCode::FAILURE
    })?;
    let report = DebugDbPruneReport {
        path,
        user_version,
        older_than_millis: options.older_than_millis,
        cutoff_unix_ms,
        deleted: DebugDbPruneDeletedReport {
            sessions: deleted.sessions,
            rag_queries: deleted.rag_queries,
            chunks: deleted.chunks,
            diagnostics: deleted.diagnostics,
            history_entries: deleted.history_entries,
            test_results: deleted.test_results,
            blobs: deleted.blobs,
            programs: deleted.programs,
        },
        stats_after: stats_report(stats_after),
    };
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: pruned rows older than {}ms cutoff={}",
        report.path, report.older_than_millis, report.cutoff_unix_ms
    );
    println!(
        "deleted: sessions={}, rag_queries={}, chunks={}, blobs={}, programs={}",
        report.deleted.sessions,
        report.deleted.rag_queries,
        report.deleted.chunks,
        report.deleted.blobs,
        report.deleted.programs
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
        max_privacy: options.max_privacy,
        sessions: sessions
            .into_iter()
            .map(|session| debug_db_session_report_with_privacy(session, options.max_privacy))
            .collect(),
    };
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: {} session(s) max_privacy={}",
        report.path,
        report.sessions.len(),
        report.max_privacy.as_str()
    );
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

fn debug_db_sources_command(options: &DebugDbSourcesOptions) -> Result<(), ExitCode> {
    let program_hash = StableHash::new(options.program_hash.trim()).map_err(|error| {
        eprintln!("error: invalid debug db sources --program-hash: {error}");
        ExitCode::from(2)
    })?;
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let sources = if PrivacyClass::Project.is_allowed_by(options.max_privacy) {
        store
            .source_files_for_program(&program_hash)
            .map_err(|error| {
                eprintln!(
                    "error: failed to read debug source files from {}: {error}",
                    options.db.path.display()
                );
                ExitCode::FAILURE
            })?
    } else {
        Vec::new()
    };
    let report = DebugDbSourcesReport {
        path,
        user_version,
        program_hash: program_hash.as_str().to_owned(),
        max_privacy: options.max_privacy,
        sources: sources
            .into_iter()
            .map(debug_db_source_file_report)
            .collect(),
    };
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: {} source file(s) for {} max_privacy={}",
        report.path,
        report.sources.len(),
        report.program_hash,
        report.max_privacy.as_str()
    );
    for source in &report.sources {
        println!(
            "{} language={} bytes={} hash={}",
            source.path, source.language, source.byte_len, source.content_hash
        );
    }
    Ok(())
}

fn debug_db_graph_command(options: &DebugDbGraphOptions) -> Result<(), ExitCode> {
    let program_hash = StableHash::new(options.program_hash.trim()).map_err(|error| {
        eprintln!("error: invalid debug db graph --program-hash: {error}");
        ExitCode::from(2)
    })?;
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let (symbols, edges) = if PrivacyClass::Project.is_allowed_by(options.max_privacy) {
        let symbols = store
            .graph_symbols_for_program(&program_hash)
            .map_err(|error| {
                eprintln!(
                    "error: failed to read debug graph symbols from {}: {error}",
                    options.db.path.display()
                );
                ExitCode::FAILURE
            })?;
        let edges = store
            .graph_edges_for_program(&program_hash)
            .map_err(|error| {
                eprintln!(
                    "error: failed to read debug graph edges from {}: {error}",
                    options.db.path.display()
                );
                ExitCode::FAILURE
            })?;
        (symbols, edges)
    } else {
        (Vec::new(), Vec::new())
    };
    let report = DebugDbGraphReport {
        path,
        user_version,
        program_hash: program_hash.as_str().to_owned(),
        max_privacy: options.max_privacy,
        symbol_count: symbols.len(),
        edge_count: edges.len(),
        symbols: symbols
            .into_iter()
            .map(debug_db_graph_symbol_report)
            .collect(),
        edges: edges.into_iter().map(debug_db_graph_edge_report).collect(),
    };
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: {} graph symbol(s), {} edge(s) for {} max_privacy={}",
        report.path,
        report.symbol_count,
        report.edge_count,
        report.program_hash,
        report.max_privacy.as_str()
    );
    for edge in &report.edges {
        println!(
            "{} --{}--> {} weight={}",
            edge.from_symbol_id, edge.edge_kind, edge.to_symbol_id, edge.weight
        );
    }
    Ok(())
}

fn debug_db_close_stale_sessions_command(
    options: &DebugDbCloseStaleSessionsOptions,
) -> Result<(), ExitCode> {
    let report = debug_db_close_stale_sessions_report(options)?;
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: matched {} stale running session(s), closed {} (dry_run={})",
        report.path,
        report.matched_sessions.len(),
        report.closed_sessions.len(),
        report.dry_run
    );
    Ok(())
}

fn debug_db_close_stale_sessions_report(
    options: &DebugDbCloseStaleSessionsOptions,
) -> Result<DebugDbCloseStaleSessionsReport, ExitCode> {
    if options.stale_after_millis <= 0 {
        eprintln!("error: debug db close-stale-sessions --stale-after must be positive");
        return Err(ExitCode::from(2));
    }
    let reason = options.reason.trim();
    if reason.is_empty() {
        eprintln!("error: debug db close-stale-sessions --reason must not be empty");
        return Err(ExitCode::from(2));
    }
    let now_unix_ms = current_unix_millis().map_err(|error| {
        eprintln!("error: failed to read current time for stale session cutoff: {error}");
        ExitCode::FAILURE
    })?;
    let cutoff_unix_ms = now_unix_ms.saturating_sub(options.stale_after_millis);
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let matched_sessions = store
        .stale_running_sessions(cutoff_unix_ms)
        .map_err(|error| {
            eprintln!(
                "error: failed to read stale running sessions from {}: {error}",
                options.db.path.display()
            );
            ExitCode::FAILURE
        })?;
    let closed_sessions = if options.dry_run {
        Vec::new()
    } else {
        store
            .abandon_stale_running_sessions(cutoff_unix_ms, now_unix_ms, reason)
            .map_err(|error| {
                eprintln!(
                    "error: failed to close stale running sessions in {}: {error}",
                    options.db.path.display()
                );
                ExitCode::FAILURE
            })?
    };
    Ok(DebugDbCloseStaleSessionsReport {
        path,
        user_version,
        stale_after_millis: options.stale_after_millis,
        cutoff_unix_ms,
        closed_unix_ms: now_unix_ms,
        reason: reason.to_owned(),
        dry_run: options.dry_run,
        matched_sessions: matched_sessions
            .into_iter()
            .map(debug_db_session_report)
            .collect(),
        closed_sessions: closed_sessions
            .into_iter()
            .map(debug_db_session_report)
            .collect(),
    })
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
        max_privacy: options.max_privacy,
        runs: runs
            .into_iter()
            .map(|run| debug_db_run_report_with_privacy(run, options.max_privacy))
            .collect(),
    };
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: {} run(s) max_privacy={}",
        report.path,
        report.runs.len(),
        report.max_privacy.as_str()
    );
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
        session_id: audit.session_id.as_ref().map(|id| id.as_str().to_owned()),
        run_id: audit.run_id.as_ref().map(|id| id.as_str().to_owned()),
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

fn debug_db_repl_cells_command(options: &DebugDbReplCellsOptions) -> Result<(), ExitCode> {
    if options.limit == 0 {
        eprintln!("error: debug db repl-cells --limit must be at least 1");
        return Err(ExitCode::from(2));
    }
    let session_id = SessionId::new(options.session_id.trim()).map_err(|error| {
        eprintln!("error: invalid debug db repl-cells --session-id: {error}");
        ExitCode::from(2)
    })?;
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let cells = store.repl_cells_for_session(&session_id).map_err(|error| {
        eprintln!(
            "error: failed to read debug REPL cells from {}: {error}",
            options.db.path.display()
        );
        ExitCode::FAILURE
    })?;
    let report = DebugDbReplCellsReport {
        path,
        user_version,
        session_id: session_id.as_str().to_owned(),
        limit: options.limit,
        cells: cells
            .into_iter()
            .take(options.limit)
            .map(debug_db_repl_cell_report)
            .collect(),
    };
    if options.db.json {
        return print_json(&report);
    }
    println!("{}: {} REPL cell(s)", report.path, report.cells.len());
    for cell in &report.cells {
        println!(
            "{} {} status={} effectful={} source={:?}",
            cell.ordinal, cell.cell_id, cell.status, cell.partially_effectful, cell.source
        );
    }
    Ok(())
}

fn debug_db_embed_command(options: &DebugDbEmbedOptions) -> Result<(), ExitCode> {
    let report = debug_db_embed_report(options)?;
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: embedded {} chunk(s) with {}@{}:{} (max_privacy={}, skipped={})",
        report.path,
        report.embedded_chunks,
        report.model.model_id,
        report.model.model_revision,
        report.model.dimensions,
        report.max_privacy.as_str(),
        report.skipped_chunks
    );
    Ok(())
}

fn debug_db_embed_report(options: &DebugDbEmbedOptions) -> Result<DebugDbEmbedReport, ExitCode> {
    let model = debug_db_embed_model(options).map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::from(2)
    })?;
    let (store, path, user_version, stats_before) = open_debug_store(&options.db)?;
    let inputs = store
        .embedding_inputs_with_policy(options.provider.input_policy(options.max_privacy))
        .map_err(|error| {
            eprintln!(
                "error: failed to read embedding inputs from debug database {}: {error}",
                options.db.path.display()
            );
            ExitCode::FAILURE
        })?;
    let embeddings = debug_db_embed_inputs(options, &store, &model, &inputs)?;
    for embedding in &embeddings {
        store.upsert_embedding(embedding).map_err(|error| {
            eprintln!(
                "error: failed to write embedding for {} into debug database {}: {error}",
                embedding.chunk_id.as_str(),
                options.db.path.display()
            );
            ExitCode::FAILURE
        })?;
    }
    let stats_after = store.stats().map_err(|error| {
        eprintln!(
            "error: failed to read debug database {} after embedding: {error}",
            options.db.path.display()
        );
        ExitCode::FAILURE
    })?;
    Ok(DebugDbEmbedReport {
        path,
        user_version,
        provider: options.provider.label().to_owned(),
        scope: options.provider.scope_label().to_owned(),
        model: DebugDbSearchModelReport {
            model_id: model.model_id,
            model_revision: model.model_revision,
            dimensions: model.dimensions,
        },
        max_privacy: options.max_privacy,
        input_chunks: inputs.len(),
        embedded_chunks: embeddings.len(),
        skipped_chunks: stats_before
            .chunks
            .saturating_sub(u64::try_from(inputs.len()).unwrap_or(u64::MAX)),
        embedded_chunk_ids: embeddings
            .into_iter()
            .map(|embedding| embedding.chunk_id.as_str().to_owned())
            .collect(),
        stats_after: stats_report(stats_after),
    })
}

fn debug_db_embed_inputs(
    options: &DebugDbEmbedOptions,
    store: &DebugStore,
    model: &EmbeddingModelDescriptor,
    inputs: &[EmbeddingInput],
) -> Result<Vec<arcweft_debug_model::embedding::StoredEmbedding>, ExitCode> {
    match options.provider {
        DebugDbEmbeddingProvider::LocalHash => {
            let mut provider = LocalHashEmbeddingProvider::new(model.clone());
            provider.embed(inputs).map_err(|error| {
                eprintln!("error: failed to embed debug chunks with local provider: {error}");
                ExitCode::FAILURE
            })
        }
        DebugDbEmbeddingProvider::Remote => {
            let Some(command) = options.remote_command.as_deref().map(str::trim) else {
                record_debug_db_remote_embedding_unavailable(
                    store,
                    &options.db.path,
                    model,
                    options.max_privacy,
                    inputs.len(),
                )?;
                eprintln!(
                    "error: remote embedding provider is not configured; recorded AGENT_DEBUG_EMBEDDING_PROVIDER_UNAVAILABLE diagnostic in {}",
                    options.db.path.display()
                );
                return Err(ExitCode::FAILURE);
            };
            if command.is_empty() {
                eprintln!("error: debug db embed --remote-command must not be empty");
                return Err(ExitCode::from(2));
            }
            let mut provider = RemoteCommandEmbeddingProvider::new(
                model.clone(),
                command.to_owned(),
                options.remote_args.clone(),
            );
            provider.embed(inputs).map_err(|error| {
                eprintln!("error: failed to embed debug chunks with remote provider: {error}");
                ExitCode::FAILURE
            })
        }
    }
}

fn record_debug_db_remote_embedding_unavailable(
    store: &DebugStore,
    db_path: &Path,
    model: &EmbeddingModelDescriptor,
    max_privacy: PrivacyClass,
    input_chunks: usize,
) -> Result<(), ExitCode> {
    let diagnostic_id = format!(
        "debug-db-embedding-provider-unavailable:{}:{}:{}",
        model.model_id, model.model_revision, model.dimensions
    );
    store
        .upsert_diagnostic(&DebugDiagnostic {
            diagnostic_id,
            program_hash: None,
            session_id: None,
            run_id: None,
            sequence: None,
            code: Some("AGENT_DEBUG_EMBEDDING_PROVIDER_UNAVAILABLE".to_owned()),
            severity: "error".to_owned(),
            phase: "debug_db_embed".to_owned(),
            message: format!(
                "remote embedding provider is not configured for model {}@{}:{}",
                model.model_id, model.model_revision, model.dimensions
            ),
            source_path: Some(db_path.display().to_string()),
            start_byte: None,
            end_byte: None,
            related_ids: Vec::new(),
            payload: serde_json::json!({
                "provider": "remote",
                "scope": "remote",
                "model": {
                    "model_id": model.model_id,
                    "model_revision": model.model_revision,
                    "dimensions": model.dimensions,
                },
                "max_privacy": max_privacy.as_str(),
                "input_chunks_after_policy": input_chunks,
                "reason": "provider_not_configured",
            }),
            created_unix_ms: current_unix_millis().unwrap_or(0),
        })
        .map_err(|error| {
            eprintln!(
                "error: failed to record remote embedding provider diagnostic in {}: {error}",
                db_path.display()
            );
            ExitCode::FAILURE
        })
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
    let selectors = debug_db_search_text_selectors(options);
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
    validate_debug_db_search_selection(selectors.count(has_query_vector), options.limit)?;
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
        DebugDbSearchHitRequest {
            selectors: &selectors,
            query_vector: query_vector.as_deref(),
            model: model.as_ref(),
        },
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
        query: selectors.query.map(str::to_owned),
        query_vector_dimensions: query_vector.as_ref().map(Vec::len),
        graph_query: selectors.graph_query.map(str::to_owned),
        graph_depth: selectors.graph_query.map(|_| options.graph_depth),
        history_query: selectors.history_query.map(str::to_owned),
        diagnostic_query: selectors.diagnostic_query.map(str::to_owned),
        test_query: selectors.test_query.map(str::to_owned),
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

#[derive(Clone, Copy)]
struct DebugDbSearchTextSelectors<'a> {
    query: Option<&'a str>,
    graph_query: Option<&'a str>,
    history_query: Option<&'a str>,
    diagnostic_query: Option<&'a str>,
    test_query: Option<&'a str>,
}

impl DebugDbSearchTextSelectors<'_> {
    fn count(self, has_query_vector: bool) -> usize {
        usize::from(self.query.is_some())
            + usize::from(has_query_vector)
            + usize::from(self.graph_query.is_some())
            + usize::from(self.history_query.is_some())
            + usize::from(self.diagnostic_query.is_some())
            + usize::from(self.test_query.is_some())
    }
}

fn debug_db_search_text_selectors(
    options: &DebugDbSearchOptions,
) -> DebugDbSearchTextSelectors<'_> {
    DebugDbSearchTextSelectors {
        query: trimmed_non_empty(options.query.as_deref()),
        graph_query: trimmed_non_empty(options.graph_query.as_deref()),
        history_query: trimmed_non_empty(options.history_query.as_deref()),
        diagnostic_query: trimmed_non_empty(options.diagnostic_query.as_deref()),
        test_query: trimmed_non_empty(options.test_query.as_deref()),
    }
}

fn debug_db_embed_model(options: &DebugDbEmbedOptions) -> Result<EmbeddingModelDescriptor, String> {
    let model_id = options.model_id.trim();
    if model_id.is_empty() {
        return Err("debug db embed --model-id must not be empty".to_owned());
    }
    let model_revision = options.model_revision.trim();
    if model_revision.is_empty() {
        return Err("debug db embed --model-revision must not be empty".to_owned());
    }
    if options.dimensions == 0 {
        return Err("debug db embed --dimensions must be at least 1".to_owned());
    }
    if options.dimensions > MAX_LOCAL_EMBEDDING_DIMENSIONS {
        return Err(format!(
            "debug db embed --dimensions must be at most {MAX_LOCAL_EMBEDDING_DIMENSIONS}"
        ));
    }
    Ok(EmbeddingModelDescriptor {
        model_id: model_id.to_owned(),
        model_revision: model_revision.to_owned(),
        dimensions: options.dimensions,
    })
}

fn trimmed_non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

fn debug_db_session_report(session: DebugSession) -> DebugDbSessionReport {
    debug_db_session_report_with_privacy(session, PrivacyClass::Project)
}

fn debug_db_session_report_with_privacy(
    session: DebugSession,
    max_privacy: PrivacyClass,
) -> DebugDbSessionReport {
    let include_project_metadata = PrivacyClass::Project.is_allowed_by(max_privacy);
    let project = include_project_metadata
        .then(|| debug_project_readback_json(&session.metadata))
        .flatten();
    let metadata = if include_project_metadata {
        session.metadata
    } else {
        BTreeMap::new()
    };
    DebugDbSessionReport {
        session_id: session.session_id.as_str().to_owned(),
        program_hash: session.program_hash.map(|hash| hash.as_str().to_owned()),
        profile: session.profile,
        transport: session.transport,
        started_unix_ms: session.started_unix_ms,
        ended_unix_ms: session.ended_unix_ms,
        status: session.status,
        project,
        metadata,
    }
}

fn debug_db_run_report_with_privacy(
    run: DebugScriptRun,
    max_privacy: PrivacyClass,
) -> DebugDbRunReport {
    let include_project_metadata = PrivacyClass::Project.is_allowed_by(max_privacy);
    let project = include_project_metadata
        .then(|| debug_project_readback_json(&run.metadata))
        .flatten();
    let metadata = if include_project_metadata {
        run.metadata
    } else {
        BTreeMap::new()
    };
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
        project,
        metadata,
    }
}

pub(in crate::app) fn debug_project_readback_json(
    metadata: &BTreeMap<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    let entities = metadata.get("project_entities");
    let graph = metadata.get("project_graph");
    if entities.is_none() && graph.is_none() {
        return None;
    }
    let entity_count = entities
        .and_then(|value| value.get("count"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let entity_kind_counts = entities
        .and_then(|value| value.get("kind_counts"))
        .map(json_u64_object)
        .unwrap_or_default();
    let graph_symbol_count = graph
        .and_then(|value| value.get("symbol_count"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let graph_edge_count = graph
        .and_then(|value| value.get("edge_count"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let graph_symbol_kind_counts = graph
        .and_then(|value| value.get("symbol_kind_counts"))
        .map(json_u64_object)
        .unwrap_or_default();
    let graph_edge_kind_counts = graph
        .and_then(|value| value.get("edge_kind_counts"))
        .map(json_u64_object)
        .unwrap_or_default();
    let graph_summary_symbol_id = graph
        .and_then(|value| value.get("summary_symbol_id"))
        .and_then(serde_json::Value::as_str);
    let graph_has_project_summary = graph
        .and_then(|value| value.get("has_project_summary"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let project_summary = graph
        .and_then(|value| value.get("project_summary"))
        .filter(|value| !value.is_null())
        .cloned();
    Some(serde_json::json!({
        "entity_count": entity_count,
        "entity_kind_counts": entity_kind_counts,
        "graph_symbol_count": graph_symbol_count,
        "graph_edge_count": graph_edge_count,
        "graph_summary_symbol_id": graph_summary_symbol_id,
        "graph_has_project_summary": graph_has_project_summary,
        "graph_symbol_kind_counts": graph_symbol_kind_counts,
        "graph_edge_kind_counts": graph_edge_kind_counts,
        "project_summary": project_summary,
    }))
}

fn json_u64_object(value: &serde_json::Value) -> BTreeMap<String, u64> {
    value
        .as_object()
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| value.as_u64().map(|count| (key.clone(), count)))
                .collect()
        })
        .unwrap_or_default()
}

fn debug_db_source_file_report(source: DebugSourceFile) -> DebugDbSourceFileReport {
    DebugDbSourceFileReport {
        program_hash: source.program_hash.as_str().to_owned(),
        path: source.path,
        language: source.language,
        content_hash: source.content_hash.as_str().to_owned(),
        byte_len: source.byte_len,
        metadata: source.metadata,
    }
}

fn debug_db_graph_symbol_report(symbol: DebugGraphSymbol) -> DebugDbGraphSymbolReport {
    DebugDbGraphSymbolReport {
        program_hash: symbol.program_hash.as_str().to_owned(),
        symbol_id: symbol.symbol_id,
        public_id: symbol.public_id.map(|id| id.as_str().to_owned()),
        qualified_name: symbol.qualified_name,
        kind: symbol.kind,
        type_json: symbol.type_json,
        source_path: symbol.source_path,
        source_content_hash: symbol
            .source_content_hash
            .map(|hash| hash.as_str().to_owned()),
        start_byte: symbol.start_byte,
        end_byte: symbol.end_byte,
        semantic_hash: symbol.semantic_hash.map(|hash| hash.as_str().to_owned()),
        summary: symbol.summary,
        metadata: symbol.metadata,
    }
}

fn debug_db_graph_edge_report(edge: DebugGraphEdge) -> DebugDbGraphEdgeReport {
    DebugDbGraphEdgeReport {
        program_hash: edge.program_hash.as_str().to_owned(),
        from_symbol_id: edge.from_symbol_id,
        to_symbol_id: edge.to_symbol_id,
        edge_kind: edge.edge_kind,
        weight: edge.weight,
        metadata: edge.metadata,
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

fn debug_db_repl_cell_report(cell: DebugReplCell) -> DebugDbReplCellReport {
    DebugDbReplCellReport {
        cell_id: cell.cell_id,
        session_id: cell.session_id.as_str().to_owned(),
        run_id: cell.run_id.map(|id| id.as_str().to_owned()),
        ordinal: cell.ordinal,
        source: cell.source,
        source_hash: cell.source_hash.as_str().to_owned(),
        status: cell.status,
        inferred_type: cell.inferred_type,
        display: cell.display,
        partially_effectful: cell.partially_effectful,
        diagnostic_ids: cell.diagnostic_ids,
        created_unix_ms: cell.created_unix_ms,
    }
}

fn validate_debug_db_search_selection(selector_count: usize, limit: usize) -> Result<(), ExitCode> {
    if selector_count == 0 {
        eprintln!(
            "error: debug db search requires one of --query, --query-vector, --graph-query, --history-query, --diagnostic-query, or --test-query"
        );
        return Err(ExitCode::from(2));
    }
    if selector_count > 1 {
        eprintln!(
            "error: debug db search accepts only one of --query, --query-vector, --graph-query, --history-query, --diagnostic-query, or --test-query"
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
    request: DebugDbSearchHitRequest<'_>,
) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
    if let Some(query) = request.selectors.query {
        return store.lexical_search_with_max_privacy(query, options.limit, options.max_privacy);
    }
    if let Some(query) = request.selectors.graph_query {
        return store.graph_search_with_depth_and_max_privacy(
            query,
            options.graph_depth,
            options.limit,
            options.max_privacy,
        );
    }
    if let Some(query) = request.selectors.history_query {
        return store.history_search_with_max_privacy(query, options.limit, options.max_privacy);
    }
    if let Some(query) = request.selectors.diagnostic_query {
        return store.diagnostic_search_with_max_privacy(query, options.limit, options.max_privacy);
    }
    if let Some(query) = request.selectors.test_query {
        return store.test_result_search_with_max_privacy(
            query,
            options.limit,
            options.max_privacy,
        );
    }
    let vector = request
        .query_vector
        .expect("query vector is validated before search");
    let model = request
        .model
        .expect("embedding model is validated before vector search");
    store.vector_search_with_max_privacy(model, vector, options.limit, options.max_privacy)
}

#[derive(Clone, Copy)]
struct DebugDbSearchHitRequest<'a> {
    selectors: &'a DebugDbSearchTextSelectors<'a>,
    query_vector: Option<&'a [f32]>,
    model: Option<&'a EmbeddingModelDescriptor>,
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

fn parse_debug_retention_duration_millis(value: &str) -> Result<i64, String> {
    let value = value.trim();
    let Some((number, unit, multiplier)) = debug_retention_duration_parts(value) else {
        return Err(
            "debug db prune --older-than must use a positive duration such as 30d, 12h, 15m, 10s, or 500ms"
                .to_owned(),
        );
    };
    if number.is_empty() || !number.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!(
            "debug db prune --older-than has an invalid numeric component: `{value}`"
        ));
    }
    let amount = number
        .parse::<u128>()
        .map_err(|error| format!("debug db prune --older-than is too large: {error}"))?;
    if amount == 0 {
        return Err("debug db prune --older-than must be greater than zero".to_owned());
    }
    let millis = amount
        .checked_mul(multiplier)
        .ok_or_else(|| "debug db prune --older-than is too large".to_owned())?;
    i64::try_from(millis)
        .map_err(|_| "debug db prune --older-than exceeds i64 milliseconds".to_owned())
        .and_then(|millis| {
            if millis > 0 {
                Ok(millis)
            } else {
                Err(format!(
                    "debug db prune --older-than unit `{unit}` produced an invalid duration"
                ))
            }
        })
}

fn debug_retention_duration_parts(value: &str) -> Option<(&str, &str, u128)> {
    [
        ("ms", 1_u128),
        ("s", 1_000),
        ("m", 60_000),
        ("h", 3_600_000),
        ("d", 86_400_000),
    ]
    .into_iter()
    .find_map(|(unit, multiplier)| {
        value
            .strip_suffix(unit)
            .map(|number| (number, unit, multiplier))
    })
}

fn current_unix_millis() -> Result<i64, std::time::SystemTimeError> {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    Ok(i64::try_from(millis).unwrap_or(i64::MAX))
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
        source_files: stats.source_files,
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
    fn parse_debug_retention_duration_accepts_explicit_units() {
        assert_eq!(parse_debug_retention_duration_millis("500ms"), Ok(500));
        assert_eq!(parse_debug_retention_duration_millis("10s"), Ok(10_000));
        assert_eq!(parse_debug_retention_duration_millis("15m"), Ok(900_000));
        assert_eq!(parse_debug_retention_duration_millis("12h"), Ok(43_200_000));
        assert_eq!(
            parse_debug_retention_duration_millis("30d"),
            Ok(2_592_000_000)
        );
        assert!(parse_debug_retention_duration_millis("0d").is_err());
        assert!(parse_debug_retention_duration_millis("30").is_err());
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
