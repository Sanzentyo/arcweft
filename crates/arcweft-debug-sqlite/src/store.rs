use crate::encoding::{VectorBlobError, decode_f32_le, encode_f32_le};
use arcweft_agent_protocol::ids::{AgentRunId, IdentifierError, PublicId, SessionId, StableHash};
use arcweft_debug_model::{
    chunk::{ChunkId, ChunkSourceKind, DebugChunk, PrivacyClass, SourceAnchor},
    diagnostic::DebugDiagnostic,
    embedding::{
        EmbeddingInput, EmbeddingInputPolicy, EmbeddingModelDescriptor, StoredEmbedding,
        embedding_inputs_for_chunks,
    },
    event::DebugEvent,
    graph::{DebugGraphEdge, DebugGraphSymbol},
    history::DebugHistoryEntry,
    rag::{RagContextItem, RagContextPack, RagQuery, SearchChannel, SearchHit},
    repl::DebugReplCell,
    script::{DebugScriptRun, DebugScriptRunFinish, DebugScriptRunOutcome},
    session::{DebugSession, DebugSessionStatus},
    sink::DebugEventSink,
    source::DebugSourceFile,
    test_result::DebugTestResult,
};
use arcweft_rag::vector::{VectorCandidate, VectorSearchError, rank_vectors};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use thiserror::Error;

const MIGRATION_V1: &str = include_str!("../migrations/0001_init.sql");

/// `SQLite` adapter failure.
#[derive(Debug, Error)]
pub enum DebugStoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    VectorBlob(#[from] VectorBlobError),
    #[error("program hash is not indexed: {0}")]
    ProgramNotIndexed(String),
    #[error("stored embedding dimensions do not match its blob")]
    StoredDimensionMismatch,
    #[error("integer value is too large for SQLite column `{0}`")]
    IntegerOverflow(&'static str),
    #[error("invalid privacy class stored in debug database: {0}")]
    InvalidPrivacyClass(String),
    #[error("invalid chunk source kind stored in debug database: {0}")]
    InvalidChunkSourceKind(String),
    #[error("invalid RAG search channel stored in debug database: {0}")]
    InvalidSearchChannel(String),
    #[error("invalid debug session status stored in debug database: {0}")]
    InvalidSessionStatus(String),
    #[error("invalid script run outcome stored in debug database: {0}")]
    InvalidScriptRunOutcome(String),
    #[error("RAG query is not indexed: {0}")]
    RagQueryNotIndexed(String),
    #[error("debug session is not indexed: {0}")]
    SessionNotIndexed(String),
    #[error("script run is not indexed: {0}")]
    ScriptRunNotIndexed(String),
    #[error(transparent)]
    VectorSearch(#[from] VectorSearchError),
    #[error(transparent)]
    Identifier(#[from] IdentifierError),
}

/// One chunk search result returned from a debug-store search channel.
#[derive(Clone, Debug, PartialEq)]
pub struct ChunkSearchResult {
    pub hit: SearchHit,
    pub title: String,
    pub body: String,
    pub source_kind: String,
    pub source_key: String,
    pub privacy: PrivacyClass,
}

/// One full debug chunk returned from a debug-store search channel.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugChunkSearchResult {
    pub hit: SearchHit,
    pub chunk: DebugChunk,
}

/// One event row returned from the debug-store session timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugTimelineEvent {
    pub session_id: String,
    pub run_id: Option<String>,
    pub sequence: u64,
    pub tick: Option<u64>,
    pub event_kind: String,
    pub payload: serde_json::Value,
    pub privacy: PrivacyClass,
    pub created_unix_ms: i64,
}

/// One persisted RAG query audit reconstructed from `rag_queries` and selected hits.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugRagQueryAudit {
    pub pack: RagContextPack,
    pub session_id: Option<SessionId>,
    pub run_id: Option<AgentRunId>,
    pub status: String,
    pub created_unix_ms: i64,
}

#[derive(Debug)]
struct RawDebugSession {
    session_id: String,
    program_hash: Option<String>,
    profile: String,
    transport: String,
    started_unix_ms: i64,
    ended_unix_ms: Option<i64>,
    status: String,
    metadata_json: String,
}

#[derive(Debug)]
struct RawDebugScriptRun {
    run_id: String,
    session_id: String,
    agent_id: Option<String>,
    artifact_hash: Option<String>,
    source_hash: Option<String>,
    project_binding_mode: String,
    started_sequence: i64,
    finished_sequence: Option<i64>,
    outcome: String,
    partially_effectful: i64,
    trace_uri: Option<String>,
    error_json: Option<String>,
    metadata_json: String,
}

#[derive(Debug)]
struct RawDebugChunk {
    chunk_id: String,
    program_hash: Option<String>,
    source_kind: String,
    source_key: String,
    title: String,
    body: String,
    content_hash: String,
    semantic_hash: Option<String>,
    source_path: Option<String>,
    entity_ids_json: String,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
    privacy_class: String,
    metadata_json: String,
    created_unix_ms: i64,
}

#[derive(Debug)]
struct RawDebugSourceFile {
    program_hash: String,
    path: String,
    language: String,
    content_hash: String,
    byte_len: i64,
    metadata_json: String,
}

/// Current row counts for the rebuildable debug index.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DebugStoreStats {
    pub programs: u64,
    pub source_files: u64,
    pub sessions: u64,
    pub script_runs: u64,
    pub debug_events: u64,
    pub frames: u64,
    pub actions: u64,
    pub captures: u64,
    pub blobs: u64,
    pub chunks: u64,
    pub embeddings: u64,
    pub rag_queries: u64,
    pub repl_cells: u64,
}

/// Result of a debug-store integrity validation pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugStoreValidationReport {
    pub integrity_messages: Vec<String>,
    pub foreign_key_violations: Vec<DebugStoreForeignKeyViolation>,
    pub missing_capture_blob_refs: u64,
    pub invalid_embedding_blobs: u64,
}

/// One row returned by `SQLite` `PRAGMA foreign_key_check`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugStoreForeignKeyViolation {
    pub table: String,
    pub rowid: i64,
    pub parent: String,
    pub fkid: i64,
}

/// Result of rebuilding derived debug indexes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DebugStoreReindexReport {
    pub chunks_indexed: u64,
}

/// Result of compacting the `SQLite` debug store with `VACUUM`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DebugStoreVacuumReport {
    pub page_count_before: u64,
    pub freelist_count_before: u64,
    pub page_count_after: u64,
    pub freelist_count_after: u64,
}

/// Row counts removed by a debug-store retention prune.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DebugStorePruneReport {
    pub sessions: u64,
    pub rag_queries: u64,
    pub chunks: u64,
    pub diagnostics: u64,
    pub history_entries: u64,
    pub test_results: u64,
    pub blobs: u64,
    pub programs: u64,
}

/// Blob row metadata needed by CLI-side byte store lifecycle operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugStoreBlobRecord {
    pub blob_hash: String,
    pub byte_len: u64,
    pub relative_path: String,
}

/// Rebuildable `SQLite` index. The connection should be owned by one writer.
pub struct DebugStore {
    connection: Connection,
}

impl DebugStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DebugStoreError> {
        let connection = Connection::open(path)?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, DebugStoreError> {
        let connection = Connection::open_in_memory()?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    pub fn migrate(&self) -> Result<(), DebugStoreError> {
        self.connection.execute_batch(MIGRATION_V1)?;
        Ok(())
    }

    pub fn user_version(&self) -> Result<u32, DebugStoreError> {
        let version = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))?;
        Ok(version)
    }

    pub fn stats(&self) -> Result<DebugStoreStats, DebugStoreError> {
        Ok(DebugStoreStats {
            programs: self.table_count("programs")?,
            source_files: self.table_count("source_files")?,
            sessions: self.table_count("sessions")?,
            script_runs: self.table_count("script_runs")?,
            debug_events: self.table_count("debug_events")?,
            frames: self.table_count("frames")?,
            actions: self.table_count("actions")?,
            captures: self.table_count("captures")?,
            blobs: self.table_count("blobs")?,
            chunks: self.table_count("chunks")?,
            embeddings: self.table_count("embeddings")?,
            rag_queries: self.table_count("rag_queries")?,
            repl_cells: self.table_count("repl_cells")?,
        })
    }

    pub fn validate(&self) -> Result<DebugStoreValidationReport, DebugStoreError> {
        let integrity_messages = self.integrity_messages()?;
        let foreign_key_violations = self.foreign_key_violations()?;
        let missing_capture_blob_refs = self.missing_capture_blob_refs()?;
        let invalid_embedding_blobs = self.invalid_embedding_blobs()?;
        Ok(DebugStoreValidationReport {
            integrity_messages,
            foreign_key_violations,
            missing_capture_blob_refs,
            invalid_embedding_blobs,
        })
    }

    pub fn reindex(&self) -> Result<DebugStoreReindexReport, DebugStoreError> {
        self.connection
            .execute("INSERT INTO chunks_fts(chunks_fts) VALUES ('rebuild')", [])?;
        self.connection
            .execute("INSERT INTO chunks_fts(chunks_fts) VALUES ('optimize')", [])?;
        Ok(DebugStoreReindexReport {
            chunks_indexed: self.table_count("chunks")?,
        })
    }

    pub fn vacuum(&self) -> Result<DebugStoreVacuumReport, DebugStoreError> {
        let page_count_before = self.pragma_count("page_count")?;
        let freelist_count_before = self.pragma_count("freelist_count")?;
        self.connection.execute_batch("VACUUM")?;
        Ok(DebugStoreVacuumReport {
            page_count_before,
            freelist_count_before,
            page_count_after: self.pragma_count("page_count")?,
            freelist_count_after: self.pragma_count("freelist_count")?,
        })
    }

    pub fn prune_before(
        &self,
        cutoff_unix_ms: i64,
    ) -> Result<DebugStorePruneReport, DebugStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        let report = DebugStorePruneReport {
            sessions: delete_count(
                transaction.execute(
                    "DELETE FROM sessions
                     WHERE coalesce(ended_unix_ms, started_unix_ms) < ?1",
                    [cutoff_unix_ms],
                )?,
                "sessions.deleted",
            )?,
            rag_queries: delete_count(
                transaction.execute(
                    "DELETE FROM rag_queries WHERE created_unix_ms < ?1",
                    [cutoff_unix_ms],
                )?,
                "rag_queries.deleted",
            )?,
            chunks: delete_count(
                transaction.execute(
                    "DELETE FROM chunks WHERE created_unix_ms < ?1",
                    [cutoff_unix_ms],
                )?,
                "chunks.deleted",
            )?,
            diagnostics: delete_count(
                transaction.execute(
                    "DELETE FROM diagnostics WHERE created_unix_ms < ?1",
                    [cutoff_unix_ms],
                )?,
                "diagnostics.deleted",
            )?,
            history_entries: delete_count(
                transaction.execute(
                    "DELETE FROM history_entries WHERE created_unix_ms < ?1",
                    [cutoff_unix_ms],
                )?,
                "history_entries.deleted",
            )?,
            test_results: delete_count(
                transaction.execute(
                    "DELETE FROM test_results WHERE created_unix_ms < ?1",
                    [cutoff_unix_ms],
                )?,
                "test_results.deleted",
            )?,
            blobs: delete_count(
                transaction.execute(
                    "DELETE FROM blobs
                     WHERE last_access_unix_ms < ?1
                       AND NOT EXISTS (
                         SELECT 1 FROM captures WHERE captures.blob_hash = blobs.blob_hash
                       )",
                    [cutoff_unix_ms],
                )?,
                "blobs.deleted",
            )?,
            programs: delete_count(
                transaction.execute(
                    "DELETE FROM programs
                     WHERE created_unix_ms < ?1
                       AND NOT EXISTS (
                         SELECT 1 FROM sessions WHERE sessions.program_id = programs.program_id
                       )
                       AND NOT EXISTS (
                         SELECT 1 FROM chunks WHERE chunks.program_id = programs.program_id
                       )
                       AND NOT EXISTS (
                         SELECT 1 FROM rag_queries WHERE rag_queries.program_id = programs.program_id
                       )
                       AND NOT EXISTS (
                         SELECT 1 FROM diagnostics WHERE diagnostics.program_id = programs.program_id
                       )
                       AND NOT EXISTS (
                         SELECT 1 FROM history_entries WHERE history_entries.program_id = programs.program_id
                       )
                       AND NOT EXISTS (
                         SELECT 1 FROM test_results WHERE test_results.program_id = programs.program_id
                       )",
                    [cutoff_unix_ms],
                )?,
                "programs.deleted",
            )?,
        };
        transaction.commit()?;
        Ok(report)
    }

    pub fn delete_unreferenced_blobs(&self) -> Result<u64, DebugStoreError> {
        let deleted = self.connection.execute(
            "DELETE FROM blobs
             WHERE NOT EXISTS (
               SELECT 1 FROM captures WHERE captures.blob_hash = blobs.blob_hash
             )",
            [],
        )?;
        u64::try_from(deleted).map_err(|_| DebugStoreError::IntegerOverflow("blobs.deleted"))
    }

    pub fn blob_records(&self) -> Result<Vec<DebugStoreBlobRecord>, DebugStoreError> {
        self.blob_records_where("")
    }

    pub fn unreferenced_blob_records(&self) -> Result<Vec<DebugStoreBlobRecord>, DebugStoreError> {
        self.blob_records_where(
            "WHERE NOT EXISTS (
               SELECT 1 FROM captures WHERE captures.blob_hash = blobs.blob_hash
             )",
        )
    }

    pub fn upsert_program(
        &self,
        program_hash: &StableHash,
        bundle_hash: Option<&StableHash>,
        source_root: Option<&str>,
        created_unix_ms: i64,
    ) -> Result<i64, DebugStoreError> {
        self.connection.execute(
            "INSERT INTO programs(program_hash, bundle_hash, source_root, created_unix_ms)\n             VALUES (?1, ?2, ?3, ?4)\n             ON CONFLICT(program_hash) DO UPDATE SET\n               bundle_hash = excluded.bundle_hash,\n               source_root = excluded.source_root",
            params![
                program_hash.as_str(),
                bundle_hash.map(StableHash::as_str),
                source_root,
                created_unix_ms,
            ],
        )?;
        self.program_id(program_hash)?
            .ok_or_else(|| DebugStoreError::ProgramNotIndexed(program_hash.as_str().to_owned()))
    }

    pub fn upsert_source_file(&self, source: &DebugSourceFile) -> Result<(), DebugStoreError> {
        let program_id = self.require_program_id(&source.program_hash)?;
        let byte_len = sqlite_i64(source.byte_len, "source_files.byte_len")?;
        self.connection.execute(
            "INSERT INTO source_files(
               program_id, path, language, content_hash, byte_len, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(program_id, path, content_hash) DO UPDATE SET
               language = excluded.language,
               byte_len = excluded.byte_len,
               metadata_json = excluded.metadata_json",
            params![
                program_id,
                &source.path,
                &source.language,
                source.content_hash.as_str(),
                byte_len,
                serde_json::to_string(&source.metadata)?,
            ],
        )?;
        Ok(())
    }

    pub fn source_files_for_program(
        &self,
        program_hash: &StableHash,
    ) -> Result<Vec<DebugSourceFile>, DebugStoreError> {
        let Some(program_id) = self.program_id(program_hash)? else {
            return Ok(Vec::new());
        };
        let mut statement = self.connection.prepare(
            "SELECT p.program_hash, sf.path, sf.language, sf.content_hash,
                    sf.byte_len, sf.metadata_json
             FROM source_files AS sf
             JOIN programs AS p ON p.program_id = sf.program_id
             WHERE sf.program_id = ?1
             ORDER BY sf.path ASC, sf.content_hash ASC",
        )?;
        let rows = statement.query_map([program_id], raw_debug_source_file_from_row)?;
        rows.map(|row| {
            row.map_err(DebugStoreError::from)
                .and_then(debug_source_file_from_raw)
        })
        .collect()
    }

    pub fn start_session(
        &self,
        session_id: &SessionId,
        program_hash: Option<&StableHash>,
        profile: &str,
        transport: &str,
        started_unix_ms: i64,
    ) -> Result<(), DebugStoreError> {
        self.upsert_session(&DebugSession {
            session_id: session_id.clone(),
            program_hash: program_hash.cloned(),
            profile: profile.to_owned(),
            transport: transport.to_owned(),
            started_unix_ms,
            ended_unix_ms: None,
            status: DebugSessionStatus::Running,
            metadata: BTreeMap::new(),
        })
    }

    pub fn upsert_session(&self, session: &DebugSession) -> Result<(), DebugStoreError> {
        let program_id = session
            .program_hash
            .as_ref()
            .map(|hash| self.require_program_id(hash))
            .transpose()?;
        let metadata_json = serde_json::to_string(&session.metadata)?;
        self.connection.execute(
            "INSERT INTO sessions(
               session_id, program_id, profile, transport, started_unix_ms,
               ended_unix_ms, status, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(session_id) DO UPDATE SET
               program_id = excluded.program_id,
               profile = excluded.profile,
               transport = excluded.transport,
               started_unix_ms = excluded.started_unix_ms,
               ended_unix_ms = excluded.ended_unix_ms,
               status = excluded.status,
               metadata_json = excluded.metadata_json",
            params![
                session.session_id.as_str(),
                program_id,
                &session.profile,
                &session.transport,
                session.started_unix_ms,
                session.ended_unix_ms,
                session.status.as_str(),
                metadata_json,
            ],
        )?;
        Ok(())
    }

    pub fn finish_session(
        &self,
        session_id: &SessionId,
        status: DebugSessionStatus,
        ended_unix_ms: i64,
        metadata: &BTreeMap<String, serde_json::Value>,
    ) -> Result<(), DebugStoreError> {
        let updated = self.connection.execute(
            "UPDATE sessions
             SET ended_unix_ms = ?2, status = ?3, metadata_json = ?4
             WHERE session_id = ?1",
            params![
                session_id.as_str(),
                ended_unix_ms,
                status.as_str(),
                serde_json::to_string(metadata)?,
            ],
        )?;
        if updated == 0 {
            return Err(DebugStoreError::SessionNotIndexed(
                session_id.as_str().to_owned(),
            ));
        }
        Ok(())
    }

    pub fn session(&self, session_id: &SessionId) -> Result<Option<DebugSession>, DebugStoreError> {
        self.connection
            .query_row(
                "SELECT s.session_id, p.program_hash, s.profile, s.transport,
                        s.started_unix_ms, s.ended_unix_ms, s.status, s.metadata_json
                 FROM sessions AS s
                 LEFT JOIN programs AS p ON p.program_id = s.program_id
                 WHERE s.session_id = ?1",
                [session_id.as_str()],
                raw_debug_session_from_row,
            )
            .optional()?
            .map(debug_session_from_raw)
            .transpose()
    }

    pub fn sessions(&self, limit: usize) -> Result<Vec<DebugSession>, DebugStoreError> {
        let limit =
            i64::try_from(limit).map_err(|_| DebugStoreError::IntegerOverflow("sessions.limit"))?;
        let mut statement = self.connection.prepare(
            "SELECT s.session_id, p.program_hash, s.profile, s.transport,
                    s.started_unix_ms, s.ended_unix_ms, s.status, s.metadata_json
             FROM sessions AS s
             LEFT JOIN programs AS p ON p.program_id = s.program_id
             ORDER BY s.started_unix_ms DESC, s.session_id ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], raw_debug_session_from_row)?;
        rows.map(|row| {
            row.map_err(DebugStoreError::from)
                .and_then(debug_session_from_raw)
        })
        .collect()
    }

    pub fn stale_running_sessions(
        &self,
        cutoff_unix_ms: i64,
    ) -> Result<Vec<DebugSession>, DebugStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT s.session_id, p.program_hash, s.profile, s.transport,
                    s.started_unix_ms, s.ended_unix_ms, s.status, s.metadata_json
             FROM sessions AS s
             LEFT JOIN programs AS p ON p.program_id = s.program_id
             WHERE s.status = 'running'
               AND s.ended_unix_ms IS NULL
               AND s.started_unix_ms <= ?1
             ORDER BY s.started_unix_ms ASC, s.session_id ASC",
        )?;
        let rows = statement.query_map([cutoff_unix_ms], raw_debug_session_from_row)?;
        rows.map(|row| {
            row.map_err(DebugStoreError::from)
                .and_then(debug_session_from_raw)
        })
        .collect()
    }

    pub fn abandon_stale_running_sessions(
        &self,
        cutoff_unix_ms: i64,
        ended_unix_ms: i64,
        reason: &str,
    ) -> Result<Vec<DebugSession>, DebugStoreError> {
        let stale_sessions = self.stale_running_sessions(cutoff_unix_ms)?;
        stale_sessions
            .into_iter()
            .map(|mut session| {
                session.status = DebugSessionStatus::Abandoned;
                session.ended_unix_ms = Some(ended_unix_ms);
                session.metadata.insert(
                    "lifecycle_policy".to_owned(),
                    serde_json::json!({
                        "operation": "abandon_stale_running_sessions",
                        "reason": reason,
                        "cutoff_unix_ms": cutoff_unix_ms,
                        "closed_unix_ms": ended_unix_ms,
                    }),
                );
                self.finish_session(
                    &session.session_id,
                    session.status,
                    ended_unix_ms,
                    &session.metadata,
                )?;
                Ok(session)
            })
            .collect()
    }

    pub fn upsert_script_run(&self, run: &DebugScriptRun) -> Result<(), DebugStoreError> {
        self.connection.execute(
            "INSERT INTO script_runs(
               run_id, session_id, agent_id, artifact_hash, source_hash,
               project_binding_mode, started_sequence, finished_sequence, outcome,
               partially_effectful, trace_uri, error_json, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(run_id) DO UPDATE SET
               session_id = excluded.session_id,
               agent_id = excluded.agent_id,
               artifact_hash = excluded.artifact_hash,
               source_hash = excluded.source_hash,
               project_binding_mode = excluded.project_binding_mode,
               started_sequence = excluded.started_sequence,
               finished_sequence = excluded.finished_sequence,
               outcome = excluded.outcome,
               partially_effectful = excluded.partially_effectful,
               trace_uri = excluded.trace_uri,
               error_json = excluded.error_json,
               metadata_json = excluded.metadata_json",
            params![
                run.run_id.as_str(),
                run.session_id.as_str(),
                run.agent_id.as_ref().map(PublicId::as_str),
                run.artifact_hash.as_ref().map(StableHash::as_str),
                run.source_hash.as_ref().map(StableHash::as_str),
                &run.project_binding_mode,
                sqlite_i64(run.started_sequence, "script_runs.started_sequence")?,
                run.finished_sequence
                    .map(|sequence| sqlite_i64(sequence, "script_runs.finished_sequence"))
                    .transpose()?,
                run.outcome.as_str(),
                sqlite_bool(run.partially_effectful),
                run.trace_uri.as_deref(),
                run.error.as_ref().map(serde_json::to_string).transpose()?,
                serde_json::to_string(&run.metadata)?,
            ],
        )?;
        Ok(())
    }

    pub fn finish_script_run(
        &self,
        run_id: &AgentRunId,
        finish: &DebugScriptRunFinish,
    ) -> Result<(), DebugStoreError> {
        let updated = self.connection.execute(
            "UPDATE script_runs
             SET finished_sequence = ?2,
                 outcome = ?3,
                 partially_effectful = ?4,
                 trace_uri = ?5,
                 error_json = ?6,
                 metadata_json = ?7
             WHERE run_id = ?1",
            params![
                run_id.as_str(),
                sqlite_i64(finish.finished_sequence, "script_runs.finished_sequence")?,
                finish.outcome.as_str(),
                sqlite_bool(finish.partially_effectful),
                finish.trace_uri.as_deref(),
                finish
                    .error
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                serde_json::to_string(&finish.metadata)?,
            ],
        )?;
        if updated == 0 {
            return Err(DebugStoreError::ScriptRunNotIndexed(
                run_id.as_str().to_owned(),
            ));
        }
        Ok(())
    }

    pub fn script_run(
        &self,
        run_id: &AgentRunId,
    ) -> Result<Option<DebugScriptRun>, DebugStoreError> {
        self.connection
            .query_row(
                "SELECT run_id, session_id, agent_id, artifact_hash, source_hash,
                        project_binding_mode, started_sequence, finished_sequence,
                        outcome, partially_effectful, trace_uri, error_json, metadata_json
                 FROM script_runs
                 WHERE run_id = ?1",
                [run_id.as_str()],
                raw_debug_script_run_from_row,
            )
            .optional()?
            .map(debug_script_run_from_raw)
            .transpose()
    }

    pub fn script_runs(
        &self,
        session_id: Option<&SessionId>,
        limit: usize,
    ) -> Result<Vec<DebugScriptRun>, DebugStoreError> {
        let limit = i64::try_from(limit)
            .map_err(|_| DebugStoreError::IntegerOverflow("script_runs.limit"))?;
        let mut statement = self.connection.prepare(
            "SELECT run_id, session_id, agent_id, artifact_hash, source_hash,
                    project_binding_mode, started_sequence, finished_sequence,
                    outcome, partially_effectful, trace_uri, error_json, metadata_json
             FROM script_runs
             WHERE (?1 IS NULL OR session_id = ?1)
             ORDER BY started_sequence DESC, run_id ASC
             LIMIT ?2",
        )?;
        let rows = statement
            .query_map(params![session_id.map(SessionId::as_str), limit], |row| {
                raw_debug_script_run_from_row(row)
            })?;
        rows.map(|row| {
            row.map_err(DebugStoreError::from)
                .and_then(debug_script_run_from_raw)
        })
        .collect()
    }

    pub fn next_session_sequence(&self, session_id: &SessionId) -> Result<u64, DebugStoreError> {
        let max_sequence = self.connection.query_row(
            "SELECT MAX(sequence) FROM (
               SELECT sequence FROM debug_events WHERE session_id = ?1
               UNION ALL
               SELECT started_sequence AS sequence FROM script_runs WHERE session_id = ?1
               UNION ALL
               SELECT finished_sequence AS sequence
                 FROM script_runs
                WHERE session_id = ?1 AND finished_sequence IS NOT NULL
             )",
            [session_id.as_str()],
            |row| row.get::<_, Option<i64>>(0),
        )?;
        max_sequence.map_or(Ok(0), |sequence| {
            u64::try_from(sequence)
                .map_err(|_| DebugStoreError::IntegerOverflow("sessions.sequence"))
                .map(|sequence| sequence.saturating_add(1))
        })
    }

    pub fn upsert_chunk(&self, chunk: &DebugChunk) -> Result<(), DebugStoreError> {
        let program_id = chunk
            .program_hash
            .as_ref()
            .map(|hash| self.require_program_id(hash))
            .transpose()?;
        let source_path = chunk
            .source_anchor
            .as_ref()
            .map(|anchor| anchor.path.as_str());
        let start_byte = chunk
            .source_anchor
            .as_ref()
            .map(|anchor| sqlite_i64(anchor.start_byte, "chunks.start_byte"))
            .transpose()?;
        let end_byte = chunk
            .source_anchor
            .as_ref()
            .map(|anchor| sqlite_i64(anchor.end_byte, "chunks.end_byte"))
            .transpose()?;
        let entity_ids = chunk
            .entity_ids
            .iter()
            .map(PublicId::as_str)
            .collect::<Vec<_>>();

        self.connection.execute(
            "INSERT INTO chunks(\n               chunk_id, program_id, source_kind, source_key, title, body,\n               content_hash, semantic_hash, source_path, entity_ids_json,\n               start_byte, end_byte, privacy_class, metadata_json, created_unix_ms\n             ) VALUES (\n               ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15\n             )\n             ON CONFLICT(chunk_id) DO UPDATE SET\n               program_id = excluded.program_id,\n               source_kind = excluded.source_kind,\n               source_key = excluded.source_key,\n               title = excluded.title,\n               body = excluded.body,\n               content_hash = excluded.content_hash,\n               semantic_hash = excluded.semantic_hash,\n               source_path = excluded.source_path,\n               entity_ids_json = excluded.entity_ids_json,\n               start_byte = excluded.start_byte,\n               end_byte = excluded.end_byte,\n               privacy_class = excluded.privacy_class,\n               metadata_json = excluded.metadata_json",
            params![
                chunk.id.as_str(),
                program_id,
                chunk.source_kind.as_str(),
                &chunk.source_key,
                &chunk.title,
                &chunk.body,
                chunk.content_hash.as_str(),
                chunk.semantic_hash.as_ref().map(StableHash::as_str),
                source_path,
                serde_json::to_string(&entity_ids)?,
                start_byte,
                end_byte,
                chunk.privacy.as_str(),
                serde_json::to_string(&chunk.metadata)?,
                chunk.created_unix_ms,
            ],
        )?;
        Ok(())
    }

    pub fn chunk_content_hash_exists(
        &self,
        content_hash: &StableHash,
    ) -> Result<bool, DebugStoreError> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM chunks WHERE content_hash = ?1)",
                [content_hash.as_str()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(DebugStoreError::from)
    }

    pub fn lexical_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
        self.lexical_search_with_max_privacy(query, limit, PrivacyClass::Secret)
    }

    pub fn lexical_search_with_max_privacy(
        &self,
        query: &str,
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
        self.lexical_chunk_search_with_max_privacy(query, limit, max_privacy)
            .map(|results| {
                results
                    .into_iter()
                    .map(|result| ChunkSearchResult {
                        hit: result.hit,
                        title: result.chunk.title,
                        body: result.chunk.body,
                        source_kind: result.chunk.source_kind.as_str().to_owned(),
                        source_key: result.chunk.source_key,
                        privacy: result.chunk.privacy,
                    })
                    .collect()
            })
    }

    pub fn lexical_chunk_search_with_max_privacy(
        &self,
        query: &str,
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<DebugChunkSearchResult>, DebugStoreError> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let fts_query = quote_fts_literal(query.trim());
        let mut statement = self.connection.prepare(
            "SELECT c.chunk_id, p.program_hash, c.source_kind, c.source_key,
                    c.title, c.body, c.content_hash, c.semantic_hash,
                    c.source_path, c.entity_ids_json, c.start_byte, c.end_byte,
                    c.privacy_class, c.metadata_json, c.created_unix_ms,
                    bm25(chunks_fts, 2.0, 1.0)
             FROM chunks_fts
             JOIN chunks AS c ON c.rowid = chunks_fts.rowid
             LEFT JOIN programs AS p ON p.program_id = c.program_id
             WHERE chunks_fts MATCH ?1
               AND (
                 c.privacy_class = 'public'
                 OR (?3 IN ('project', 'sensitive', 'secret') AND c.privacy_class = 'project')
                 OR (?3 IN ('sensitive', 'secret') AND c.privacy_class = 'sensitive')
                 OR (?3 = 'secret' AND c.privacy_class = 'secret')
               )
             ORDER BY bm25(chunks_fts, 2.0, 1.0), c.chunk_id
             LIMIT ?2",
        )?;
        let limit = i64::try_from(limit)
            .map_err(|_| DebugStoreError::IntegerOverflow("chunks_fts.limit"))?;
        let rows = statement.query_map(params![fts_query, limit, max_privacy.as_str()], |row| {
            Ok((raw_debug_chunk_from_row(row)?, row.get::<_, f64>(15)?))
        })?;
        let values = rows.collect::<Result<Vec<_>, _>>()?;
        values
            .into_iter()
            .enumerate()
            .map(|(index, (raw, bm25))| {
                debug_chunk_from_raw(raw).map(|chunk| DebugChunkSearchResult {
                    hit: SearchHit {
                        chunk_id: chunk.id.clone(),
                        channel: SearchChannel::Lexical,
                        rank: index + 1,
                        score: Some(-bm25),
                    },
                    chunk,
                })
            })
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn vector_search_with_max_privacy(
        &self,
        model: &EmbeddingModelDescriptor,
        query: &[f32],
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let candidates = self
            .load_embeddings_with_max_privacy(model, max_privacy)?
            .into_iter()
            .map(|embedding| VectorCandidate {
                chunk_id: embedding.chunk_id,
                values: embedding.values,
            })
            .collect::<Vec<_>>();
        let hits = rank_vectors(query, &candidates, limit)?;
        hits.into_iter()
            .map(|hit| self.chunk_search_result_for_hit(hit))
            .collect()
    }

    pub fn embedding_inputs_with_policy(
        &self,
        policy: EmbeddingInputPolicy,
    ) -> Result<Vec<EmbeddingInput>, DebugStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT c.chunk_id, p.program_hash, c.source_kind, c.source_key,
                    c.title, c.body, c.content_hash, c.semantic_hash,
                    c.source_path, c.entity_ids_json, c.start_byte, c.end_byte,
                    c.privacy_class, c.metadata_json, c.created_unix_ms
             FROM chunks AS c
             LEFT JOIN programs AS p ON p.program_id = c.program_id
             WHERE c.privacy_class = 'public'
                OR (?1 IN ('project', 'sensitive', 'secret') AND c.privacy_class = 'project')
                OR (?1 IN ('sensitive', 'secret') AND c.privacy_class = 'sensitive')
                OR (?1 = 'secret' AND c.privacy_class = 'secret')
             ORDER BY c.chunk_id",
        )?;
        let rows = statement.query_map([policy.max_privacy.as_str()], raw_debug_chunk_from_row)?;
        let chunks = rows
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(debug_chunk_from_raw)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(embedding_inputs_for_chunks(chunks.iter(), policy))
    }

    pub fn upsert_graph_symbol(&self, symbol: &DebugGraphSymbol) -> Result<(), DebugStoreError> {
        let program_id = self.require_program_id(&symbol.program_hash)?;
        let source_file_id = match (&symbol.source_path, &symbol.source_content_hash) {
            (Some(path), Some(content_hash)) => {
                self.source_file_id(program_id, path.as_str(), content_hash.as_str())?
            }
            _ => None,
        };
        let start_byte = symbol
            .start_byte
            .map(|value| sqlite_i64(value, "symbols.start_byte"))
            .transpose()?;
        let end_byte = symbol
            .end_byte
            .map(|value| sqlite_i64(value, "symbols.end_byte"))
            .transpose()?;
        self.connection.execute(
            "INSERT INTO symbols(
               symbol_id, program_id, public_id, qualified_name, kind, type_json,
               source_file_id, start_byte, end_byte, semantic_hash, summary, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(symbol_id) DO UPDATE SET
               program_id = excluded.program_id,
               public_id = excluded.public_id,
               qualified_name = excluded.qualified_name,
               kind = excluded.kind,
               type_json = excluded.type_json,
               source_file_id = excluded.source_file_id,
               start_byte = excluded.start_byte,
               end_byte = excluded.end_byte,
               semantic_hash = excluded.semantic_hash,
               summary = excluded.summary,
               metadata_json = excluded.metadata_json",
            params![
                &symbol.symbol_id,
                program_id,
                symbol.public_id.as_ref().map(PublicId::as_str),
                symbol.qualified_name.as_deref(),
                &symbol.kind,
                symbol
                    .type_json
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                source_file_id,
                start_byte,
                end_byte,
                symbol.semantic_hash.as_ref().map(StableHash::as_str),
                &symbol.summary,
                serde_json::to_string(&symbol.metadata)?,
            ],
        )?;
        Ok(())
    }

    pub fn upsert_graph_edge(&self, edge: &DebugGraphEdge) -> Result<(), DebugStoreError> {
        let program_id = self.require_program_id(&edge.program_hash)?;
        self.connection.execute(
            "INSERT INTO graph_edges(
               program_id, from_symbol_id, to_symbol_id, edge_kind, weight, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(program_id, from_symbol_id, to_symbol_id, edge_kind) DO UPDATE SET
               weight = excluded.weight,
               metadata_json = excluded.metadata_json",
            params![
                program_id,
                &edge.from_symbol_id,
                &edge.to_symbol_id,
                &edge.edge_kind,
                edge.weight,
                serde_json::to_string(&edge.metadata)?,
            ],
        )?;
        Ok(())
    }

    pub fn graph_search_with_max_privacy(
        &self,
        query: &str,
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
        self.graph_search_with_depth_and_max_privacy(query, 1, limit, max_privacy)
    }

    pub fn graph_search_with_depth_and_max_privacy(
        &self,
        query: &str,
        graph_depth: u32,
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
        if query.trim().is_empty()
            || limit == 0
            || !PrivacyClass::Project.is_allowed_by(max_privacy)
        {
            return Ok(Vec::new());
        }
        let edge_rows = self.graph_search_rows(query, graph_depth, limit)?;
        let mut excluded_symbol_ids = BTreeSet::new();
        for row in &edge_rows {
            excluded_symbol_ids.insert(row.from_symbol_id.clone());
            excluded_symbol_ids.insert(row.to_symbol_id.clone());
        }
        let mut results = edge_rows
            .iter()
            .enumerate()
            .map(|(index, row)| graph_chunk_search_result(query, index, row))
            .collect::<Vec<_>>();
        let remaining = limit.saturating_sub(results.len());
        if remaining > 0 {
            let base_rank = results.len();
            results.extend(
                self.graph_symbol_search_rows(query)?
                    .into_iter()
                    .filter(|row| !excluded_symbol_ids.contains(&row.symbol_id))
                    .take(remaining)
                    .enumerate()
                    .map(|(index, row)| {
                        graph_symbol_chunk_search_result(query, base_rank + index, &row)
                    }),
            );
        }
        Ok(results)
    }

    fn graph_search_rows(
        &self,
        query: &str,
        graph_depth: u32,
        limit: usize,
    ) -> Result<Vec<GraphSearchRow>, DebugStoreError> {
        let mut statement = self.connection.prepare(
            "WITH RECURSIVE frontier(symbol_id, depth) AS (
                SELECT symbol_id, 0
                FROM symbols
                WHERE instr(lower(coalesce(public_id, '')), ?1) > 0
                   OR instr(lower(coalesce(qualified_name, '')), ?1) > 0
                   OR instr(lower(kind), ?1) > 0
                   OR instr(lower(summary), ?1) > 0
                UNION
                SELECT CASE
                         WHEN ge.from_symbol_id = frontier.symbol_id THEN ge.to_symbol_id
                         ELSE ge.from_symbol_id
                       END,
                       frontier.depth + 1
                FROM frontier
                JOIN graph_edges AS ge
                  ON ge.from_symbol_id = frontier.symbol_id
                  OR ge.to_symbol_id = frontier.symbol_id
                WHERE frontier.depth < ?2
             ),
             edge_matches(edge_id, distance) AS (
                SELECT ge.edge_id, MIN(frontier.depth + 1)
                FROM graph_edges AS ge
                JOIN frontier
                  ON ge.from_symbol_id = frontier.symbol_id
                  OR ge.to_symbol_id = frontier.symbol_id
                WHERE frontier.depth < ?2
                GROUP BY ge.edge_id
                UNION ALL
                SELECT ge.edge_id, 0
                FROM graph_edges AS ge
                WHERE instr(lower(ge.edge_kind), ?1) > 0
             ),
             ranked_edges(edge_id, distance) AS (
                SELECT edge_id, MIN(distance)
                FROM edge_matches
                GROUP BY edge_id
             )
             SELECT ge.edge_id, ge.edge_kind, ge.weight, ranked_edges.distance,
                    from_symbol.symbol_id, from_symbol.public_id,
                    from_symbol.qualified_name, from_symbol.kind, from_symbol.summary,
                    to_symbol.symbol_id, to_symbol.public_id,
                    to_symbol.qualified_name, to_symbol.kind, to_symbol.summary
             FROM ranked_edges
             JOIN graph_edges AS ge ON ge.edge_id = ranked_edges.edge_id
             JOIN symbols AS from_symbol ON from_symbol.symbol_id = ge.from_symbol_id
             JOIN symbols AS to_symbol ON to_symbol.symbol_id = ge.to_symbol_id
             ORDER BY (ge.weight / (ranked_edges.distance + 1.0)) DESC,
                      ranked_edges.distance,
                      ge.edge_id
             LIMIT ?3",
        )?;
        let like_query = query.trim().to_lowercase();
        let graph_depth = i64::from(graph_depth);
        let limit = i64::try_from(limit)
            .map_err(|_| DebugStoreError::IntegerOverflow("graph_edges.limit"))?;
        let rows = statement.query_map(params![like_query, graph_depth, limit], |row| {
            Ok(GraphSearchRow {
                edge_id: row.get(0)?,
                edge_kind: row.get(1)?,
                weight: row.get(2)?,
                distance: row.get(3)?,
                from_symbol_id: row.get(4)?,
                from_public_id: row.get(5)?,
                from_qualified_name: row.get(6)?,
                from_kind: row.get(7)?,
                from_summary: row.get(8)?,
                to_symbol_id: row.get(9)?,
                to_public_id: row.get(10)?,
                to_qualified_name: row.get(11)?,
                to_kind: row.get(12)?,
                to_summary: row.get(13)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DebugStoreError::from)
    }

    fn graph_symbol_search_rows(
        &self,
        query: &str,
    ) -> Result<Vec<GraphSymbolSearchRow>, DebugStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT symbol_id, public_id, qualified_name, kind, summary,
                    semantic_hash, start_byte, end_byte
             FROM symbols
             WHERE instr(lower(coalesce(public_id, '')), ?1) > 0
                OR instr(lower(coalesce(qualified_name, '')), ?1) > 0
                OR instr(lower(kind), ?1) > 0
                OR instr(lower(summary), ?1) > 0
             ORDER BY
                CASE
                  WHEN lower(coalesce(public_id, '')) = ?1 THEN 0
                  WHEN lower(coalesce(qualified_name, '')) = ?1 THEN 1
                  ELSE 2
                END,
                symbol_id",
        )?;
        let like_query = query.trim().to_lowercase();
        let rows = statement.query_map([like_query], |row| {
            Ok(GraphSymbolSearchRow {
                symbol_id: row.get(0)?,
                public_id: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                summary: row.get(4)?,
                semantic_hash: row.get(5)?,
                start_byte: row.get(6)?,
                end_byte: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DebugStoreError::from)
    }

    pub fn upsert_history_entry(&self, entry: &DebugHistoryEntry) -> Result<(), DebugStoreError> {
        let program_id = entry
            .program_hash
            .as_ref()
            .map(|hash| self.require_program_id(hash))
            .transpose()?;
        self.connection.execute(
            "INSERT INTO history_entries(
               history_id, program_id, symbol_id, change_id, operation_id, ordinal,
               semantic_hash_before, semantic_hash_after, summary, metadata_json, created_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(history_id) DO UPDATE SET
               program_id = excluded.program_id,
               symbol_id = excluded.symbol_id,
               change_id = excluded.change_id,
               operation_id = excluded.operation_id,
               ordinal = excluded.ordinal,
               semantic_hash_before = excluded.semantic_hash_before,
               semantic_hash_after = excluded.semantic_hash_after,
               summary = excluded.summary,
               metadata_json = excluded.metadata_json,
               created_unix_ms = excluded.created_unix_ms",
            params![
                &entry.history_id,
                program_id,
                entry.symbol_id.as_deref(),
                &entry.change_id,
                entry.operation_id.as_deref(),
                entry.ordinal,
                entry.semantic_hash_before.as_ref().map(StableHash::as_str),
                entry.semantic_hash_after.as_ref().map(StableHash::as_str),
                &entry.summary,
                serde_json::to_string(&entry.metadata)?,
                entry.created_unix_ms,
            ],
        )?;
        Ok(())
    }

    pub fn upsert_repl_cell(&self, cell: &DebugReplCell) -> Result<(), DebugStoreError> {
        self.connection.execute(
            "INSERT INTO repl_cells(
               cell_id, session_id, run_id, ordinal, source, source_hash, status,
               inferred_type_json, display_json, partially_effectful,
               diagnostic_ids_json, created_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(cell_id) DO UPDATE SET
               session_id = excluded.session_id,
               run_id = excluded.run_id,
               ordinal = excluded.ordinal,
               source = excluded.source,
               source_hash = excluded.source_hash,
               status = excluded.status,
               inferred_type_json = excluded.inferred_type_json,
               display_json = excluded.display_json,
               partially_effectful = excluded.partially_effectful,
               diagnostic_ids_json = excluded.diagnostic_ids_json,
               created_unix_ms = excluded.created_unix_ms",
            params![
                &cell.cell_id,
                cell.session_id.as_str(),
                cell.run_id.as_ref().map(AgentRunId::as_str),
                cell.ordinal,
                &cell.source,
                cell.source_hash.as_str(),
                &cell.status,
                cell.inferred_type
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                cell.display
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                i64::from(cell.partially_effectful),
                serde_json::to_string(&cell.diagnostic_ids)?,
                cell.created_unix_ms,
            ],
        )?;
        Ok(())
    }

    pub fn repl_cells_for_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<DebugReplCell>, DebugStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT cell_id, session_id, run_id, ordinal, source, source_hash, status,
                    inferred_type_json, display_json, partially_effectful,
                    diagnostic_ids_json, created_unix_ms
             FROM repl_cells
             WHERE session_id = ?1
             ORDER BY ordinal, cell_id",
        )?;
        let rows = statement.query_map([session_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, i64>(11)?,
            ))
        })?;
        rows.map(|row| {
            let (
                cell_id,
                session_id,
                run_id,
                ordinal,
                source,
                source_hash,
                status,
                inferred_type_json,
                display_json,
                partially_effectful,
                diagnostic_ids_json,
                created_unix_ms,
            ) = row?;
            Ok(DebugReplCell {
                cell_id,
                session_id: SessionId::new(session_id)?,
                run_id: run_id.map(AgentRunId::new).transpose()?,
                ordinal,
                source,
                source_hash: StableHash::new(source_hash)?,
                status,
                inferred_type: inferred_type_json
                    .map(|value| serde_json::from_str(&value))
                    .transpose()?,
                display: display_json
                    .map(|value| serde_json::from_str(&value))
                    .transpose()?,
                partially_effectful: partially_effectful != 0,
                diagnostic_ids: serde_json::from_str(&diagnostic_ids_json)?,
                created_unix_ms,
            })
        })
        .collect()
    }

    pub fn history_search_with_max_privacy(
        &self,
        query: &str,
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
        if query.trim().is_empty()
            || limit == 0
            || !PrivacyClass::Project.is_allowed_by(max_privacy)
        {
            return Ok(Vec::new());
        }
        let like_query = query.trim().to_lowercase();
        let mut statement = self.connection.prepare(
            "SELECT history_id, change_id, operation_id, ordinal, summary
             FROM history_entries
             WHERE instr(lower(change_id), ?1) > 0
                OR instr(lower(coalesce(operation_id, '')), ?1) > 0
                OR instr(lower(summary), ?1) > 0
             ORDER BY ordinal DESC, history_id
             LIMIT ?2",
        )?;
        let limit = i64::try_from(limit)
            .map_err(|_| DebugStoreError::IntegerOverflow("history_entries.limit"))?;
        let rows = statement.query_map(params![like_query, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .enumerate()
            .map(
                |(index, (history_id, change_id, operation_id, ordinal, summary))| {
                    let title = format!("History {change_id}");
                    let body = operation_id.map_or_else(
                        || format!("ordinal={ordinal}\n{summary}"),
                        |operation_id| {
                            format!("operation={operation_id}\nordinal={ordinal}\n{summary}")
                        },
                    );
                    Ok(ChunkSearchResult {
                        hit: SearchHit {
                            chunk_id: ChunkId::new(format!("history:{history_id}")),
                            channel: SearchChannel::History,
                            rank: index + 1,
                            score: Some(history_score(query, &change_id, &body)),
                        },
                        title,
                        body,
                        source_kind: "history".to_owned(),
                        source_key: history_id,
                        privacy: PrivacyClass::Project,
                    })
                },
            )
            .collect()
    }

    pub fn upsert_diagnostic(&self, diagnostic: &DebugDiagnostic) -> Result<(), DebugStoreError> {
        let program_id = diagnostic
            .program_hash
            .as_ref()
            .map(|hash| self.require_program_id(hash))
            .transpose()?;
        let sequence = diagnostic
            .sequence
            .map(|value| sqlite_i64(value, "diagnostics.sequence"))
            .transpose()?;
        let start_byte = diagnostic
            .start_byte
            .map(|value| sqlite_i64(value, "diagnostics.start_byte"))
            .transpose()?;
        let end_byte = diagnostic
            .end_byte
            .map(|value| sqlite_i64(value, "diagnostics.end_byte"))
            .transpose()?;
        self.connection.execute(
            "INSERT INTO diagnostics(
               diagnostic_id, program_id, session_id, run_id, sequence, code,
               severity, phase, message, source_path, start_byte, end_byte,
               related_ids_json, payload_json, created_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(diagnostic_id) DO UPDATE SET
               program_id = excluded.program_id,
               session_id = excluded.session_id,
               run_id = excluded.run_id,
               sequence = excluded.sequence,
               code = excluded.code,
               severity = excluded.severity,
               phase = excluded.phase,
               message = excluded.message,
               source_path = excluded.source_path,
               start_byte = excluded.start_byte,
               end_byte = excluded.end_byte,
               related_ids_json = excluded.related_ids_json,
               payload_json = excluded.payload_json,
               created_unix_ms = excluded.created_unix_ms",
            params![
                &diagnostic.diagnostic_id,
                program_id,
                diagnostic.session_id.as_ref().map(SessionId::as_str),
                diagnostic.run_id.as_ref().map(AgentRunId::as_str),
                sequence,
                diagnostic.code.as_deref(),
                &diagnostic.severity,
                &diagnostic.phase,
                &diagnostic.message,
                diagnostic.source_path.as_deref(),
                start_byte,
                end_byte,
                serde_json::to_string(&diagnostic.related_ids)?,
                serde_json::to_string(&diagnostic.payload)?,
                diagnostic.created_unix_ms,
            ],
        )?;
        Ok(())
    }

    pub fn diagnostic_search_with_max_privacy(
        &self,
        query: &str,
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
        if query.trim().is_empty()
            || limit == 0
            || !PrivacyClass::Project.is_allowed_by(max_privacy)
        {
            return Ok(Vec::new());
        }
        let like_query = query.trim().to_lowercase();
        let mut statement = self.connection.prepare(
            "SELECT diagnostic_id, code, severity, phase, message, source_path,
                    start_byte, end_byte, related_ids_json, payload_json, sequence
             FROM diagnostics
             WHERE instr(lower(diagnostic_id), ?1) > 0
                OR instr(lower(coalesce(code, '')), ?1) > 0
                OR instr(lower(severity), ?1) > 0
                OR instr(lower(phase), ?1) > 0
                OR instr(lower(message), ?1) > 0
                OR instr(lower(coalesce(source_path, '')), ?1) > 0
                OR instr(lower(related_ids_json), ?1) > 0
                OR instr(lower(payload_json), ?1) > 0
             ORDER BY CASE severity
                        WHEN 'error' THEN 0
                        WHEN 'warning' THEN 1
                        ELSE 2
                      END,
                      coalesce(sequence, 9223372036854775807),
                      diagnostic_id
             LIMIT ?2",
        )?;
        let limit = i64::try_from(limit)
            .map_err(|_| DebugStoreError::IntegerOverflow("diagnostics.limit"))?;
        let rows = statement.query_map(params![like_query, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, Option<i64>>(10)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .enumerate()
            .map(
                |(
                    index,
                    (
                        diagnostic_id,
                        code,
                        severity,
                        phase,
                        message,
                        source_path,
                        start_byte,
                        end_byte,
                        related_ids_json,
                        payload_json,
                        sequence,
                    ),
                )| {
                    let title = code.as_ref().map_or_else(
                        || format!("{severity} diagnostic {diagnostic_id}"),
                        |code| format!("{severity} diagnostic {code}"),
                    );
                    let body = diagnostic_search_body(DiagnosticSearchBodyFields {
                        phase: &phase,
                        message: &message,
                        source_path: source_path.as_deref(),
                        start_byte,
                        end_byte,
                        sequence,
                        related_ids_json: &related_ids_json,
                        payload_json: &payload_json,
                    });
                    let score = diagnostic_score(query, code.as_deref(), &severity, &body);
                    Ok(ChunkSearchResult {
                        hit: SearchHit {
                            chunk_id: ChunkId::new(format!("diagnostic:{diagnostic_id}")),
                            channel: SearchChannel::Diagnostics,
                            rank: index + 1,
                            score: Some(score),
                        },
                        title,
                        body,
                        source_kind: "diagnostic".to_owned(),
                        source_key: diagnostic_id,
                        privacy: PrivacyClass::Project,
                    })
                },
            )
            .collect()
    }

    pub fn upsert_test_result(&self, result: &DebugTestResult) -> Result<(), DebugStoreError> {
        let program_id = result
            .program_hash
            .as_ref()
            .map(|hash| self.require_program_id(hash))
            .transpose()?;
        let duration_millis = result
            .duration_millis
            .map(|value| sqlite_i64(value, "test_results.duration_millis"))
            .transpose()?;
        self.connection.execute(
            "INSERT INTO test_results(
               test_result_id, program_id, run_id, test_id, kind, outcome,
               duration_millis, diagnostic_ids_json, artifact_refs_json, summary,
               created_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(test_result_id) DO UPDATE SET
               program_id = excluded.program_id,
               run_id = excluded.run_id,
               test_id = excluded.test_id,
               kind = excluded.kind,
               outcome = excluded.outcome,
               duration_millis = excluded.duration_millis,
               diagnostic_ids_json = excluded.diagnostic_ids_json,
               artifact_refs_json = excluded.artifact_refs_json,
               summary = excluded.summary,
               created_unix_ms = excluded.created_unix_ms",
            params![
                &result.test_result_id,
                program_id,
                result.run_id.as_ref().map(AgentRunId::as_str),
                &result.test_id,
                &result.kind,
                &result.outcome,
                duration_millis,
                serde_json::to_string(&result.diagnostic_ids)?,
                serde_json::to_string(&result.artifact_refs)?,
                &result.summary,
                result.created_unix_ms,
            ],
        )?;
        Ok(())
    }

    pub fn test_result_search_with_max_privacy(
        &self,
        query: &str,
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
        if query.trim().is_empty()
            || limit == 0
            || !PrivacyClass::Project.is_allowed_by(max_privacy)
        {
            return Ok(Vec::new());
        }
        let like_query = query.trim().to_lowercase();
        let mut statement = self.connection.prepare(
            "SELECT test_result_id, test_id, kind, outcome, duration_millis,
                    diagnostic_ids_json, artifact_refs_json, summary
             FROM test_results
             WHERE instr(lower(test_result_id), ?1) > 0
                OR instr(lower(test_id), ?1) > 0
                OR instr(lower(kind), ?1) > 0
                OR instr(lower(outcome), ?1) > 0
                OR instr(lower(summary), ?1) > 0
                OR instr(lower(diagnostic_ids_json), ?1) > 0
                OR instr(lower(artifact_refs_json), ?1) > 0
             ORDER BY CASE outcome
                        WHEN 'failed' THEN 0
                        WHEN 'error' THEN 0
                        WHEN 'flaky' THEN 1
                        ELSE 2
                      END,
                      created_unix_ms DESC,
                      test_result_id
             LIMIT ?2",
        )?;
        let limit = i64::try_from(limit)
            .map_err(|_| DebugStoreError::IntegerOverflow("test_results.limit"))?;
        let rows = statement.query_map(params![like_query, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .enumerate()
            .map(
                |(
                    index,
                    (
                        test_result_id,
                        test_id,
                        kind,
                        outcome,
                        duration_millis,
                        diagnostic_ids_json,
                        artifact_refs_json,
                        summary,
                    ),
                )| {
                    let title = format!("{outcome} {kind} {test_id}");
                    let body = test_result_search_body(
                        duration_millis,
                        &diagnostic_ids_json,
                        &artifact_refs_json,
                        &summary,
                    );
                    Ok(ChunkSearchResult {
                        hit: SearchHit {
                            chunk_id: ChunkId::new(format!("test_result:{test_result_id}")),
                            channel: SearchChannel::Diagnostics,
                            rank: index + 1,
                            score: Some(test_result_score(query, &test_id, &outcome, &body)),
                        },
                        title,
                        body,
                        source_kind: "test_result".to_owned(),
                        source_key: test_result_id,
                        privacy: PrivacyClass::Project,
                    })
                },
            )
            .collect()
    }

    pub fn session_timeline_with_max_privacy(
        &self,
        session_id: Option<&str>,
        run_id: Option<&str>,
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<DebugTimelineEvent>, DebugStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut statement = self.connection.prepare(
            "SELECT session_id, run_id, sequence, tick, event_kind, payload_json, created_unix_ms
             FROM debug_events
             WHERE (?1 IS NULL OR session_id = ?1)
               AND (?2 IS NULL OR run_id = ?2)
             ORDER BY created_unix_ms, sequence",
        )?;
        let rows = statement.query_map(params![session_id, run_id], |row| {
            let payload_text: String = row.get(5)?;
            let payload =
                serde_json::from_str::<serde_json::Value>(&payload_text).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            let sequence = row.get::<_, i64>(2)?;
            let tick = row.get::<_, Option<i64>>(3)?;
            let created_unix_ms = row.get::<_, i64>(6)?;
            Ok(DebugTimelineEvent {
                session_id: row.get(0)?,
                run_id: row.get(1)?,
                sequence: u64::try_from(sequence)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(2, sequence))?,
                tick: tick
                    .map(|value| {
                        u64::try_from(value)
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, value))
                    })
                    .transpose()?,
                event_kind: row.get(4)?,
                privacy: debug_event_payload_privacy(&payload),
                payload,
                created_unix_ms,
            })
        })?;
        let mut events = Vec::new();
        for row in rows {
            let event = row?;
            if !event.privacy.is_allowed_by(max_privacy) {
                continue;
            }
            events.push(event);
            if events.len() >= limit {
                break;
            }
        }
        Ok(events)
    }

    pub fn next_event_sequence(&self, session_id: &SessionId) -> Result<u64, DebugStoreError> {
        let next = self.connection.query_row(
            "SELECT COALESCE(MAX(sequence) + 1, 0)
             FROM debug_events
             WHERE session_id = ?1",
            [session_id.as_str()],
            |row| row.get::<_, i64>(0),
        )?;
        u64::try_from(next).map_err(|_| DebugStoreError::IntegerOverflow("debug_events.sequence"))
    }

    pub fn record_rag_context_pack(
        &self,
        pack: &RagContextPack,
        session_id: Option<&SessionId>,
        run_id: Option<&AgentRunId>,
        model: Option<&EmbeddingModelDescriptor>,
        status: &str,
        created_unix_ms: i64,
    ) -> Result<(), DebugStoreError> {
        let program_id =
            self.upsert_program(&pack.query.program_hash, None, None, created_unix_ms)?;
        let query_hash = &pack.query.query_id;
        let policy = serde_json::json!({
            "schema_version": pack.schema_version,
            "program_hash": pack.query.program_hash.as_str(),
            "roots": pack
                .query
                .roots
                .iter()
                .map(PublicId::as_str)
                .collect::<Vec<_>>(),
            "graph_depth": pack.query.graph_depth,
            "limit": pack.query.limit,
            "max_context_bytes": pack.query.max_context_bytes,
            "truncated": pack.truncated,
        });
        self.connection.execute(
            "INSERT INTO rag_queries(
               query_id, program_id, session_id, run_id, query_text, query_hash,
               model_id, model_revision, policy_json, status, created_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(query_id) DO UPDATE SET
               program_id = excluded.program_id,
               session_id = excluded.session_id,
               run_id = excluded.run_id,
               query_text = excluded.query_text,
               query_hash = excluded.query_hash,
               model_id = excluded.model_id,
               model_revision = excluded.model_revision,
               policy_json = excluded.policy_json,
               status = excluded.status,
               created_unix_ms = excluded.created_unix_ms",
            params![
                &pack.query.query_id,
                program_id,
                session_id.map(SessionId::as_str),
                run_id.map(AgentRunId::as_str),
                &pack.query.text,
                query_hash,
                model.map(|value| value.model_id.as_str()),
                model.map(|value| value.model_revision.as_str()),
                serde_json::to_string(&policy)?,
                status,
                created_unix_ms,
            ],
        )?;
        self.connection.execute(
            "DELETE FROM rag_query_hits WHERE query_id = ?1",
            [&pack.query.query_id],
        )?;
        for (index, item) in pack.items.iter().enumerate() {
            let channel_rank = sqlite_i64(
                u64::try_from(index + 1)
                    .map_err(|_| DebugStoreError::IntegerOverflow("rag_query_hits.rank"))?,
                "rag_query_hits.rank",
            )?;
            let explanation = serde_json::json!({
                "title": item.title,
                "kind": item.kind.as_str(),
                "body_bytes": item.body.len(),
                "entity_ids": item
                    .entity_ids
                    .iter()
                    .map(PublicId::as_str)
                    .collect::<Vec<_>>(),
                "source_anchor": item.source_anchor,
            });
            for channel in &item.channels {
                self.connection.execute(
                    "INSERT INTO rag_query_hits(
                       query_id, chunk_id, channel, channel_rank, channel_score,
                       fused_score, selected, explanation_json
                     ) VALUES (?1, ?2, ?3, ?4, NULL, ?5, 1, ?6)
                     ON CONFLICT(query_id, chunk_id, channel) DO UPDATE SET
                       channel_rank = excluded.channel_rank,
                       channel_score = excluded.channel_score,
                       fused_score = excluded.fused_score,
                       selected = excluded.selected,
                       explanation_json = excluded.explanation_json",
                    params![
                        &pack.query.query_id,
                        item.chunk_id.as_str(),
                        search_channel_label(*channel),
                        channel_rank,
                        item.fused_score,
                        serde_json::to_string(&explanation)?,
                    ],
                )?;
            }
        }
        Ok(())
    }

    pub fn rag_query_audit_with_max_privacy(
        &self,
        query_id: &str,
        max_privacy: PrivacyClass,
    ) -> Result<DebugRagQueryAudit, DebugStoreError> {
        let row = self.rag_query_row(query_id)?;
        let policy = serde_json::from_str::<serde_json::Value>(&row.policy_json)?;
        let query = rag_query_from_audit_row(query_id, &row, &policy)?;
        let rows = self.selected_rag_hit_rows(query_id, max_privacy)?;
        let (items, truncated) = rag_context_items_from_hit_rows(&query, &policy, rows)?;
        Ok(DebugRagQueryAudit {
            pack: RagContextPack {
                schema_version: rag_policy_u32(&policy, "schema_version", 1)?,
                query,
                items,
                truncated,
            },
            session_id: row.session_id.map(SessionId::new).transpose()?,
            run_id: row.run_id.map(AgentRunId::new).transpose()?,
            status: row.status,
            created_unix_ms: row.created_unix_ms,
        })
    }

    fn rag_query_row(&self, query_id: &str) -> Result<RagQueryRow, DebugStoreError> {
        self.connection
            .query_row(
                "SELECT q.query_text, p.program_hash, q.session_id, q.run_id,
                        q.policy_json, q.status, q.created_unix_ms
                 FROM rag_queries AS q
                 LEFT JOIN programs AS p ON p.program_id = q.program_id
                 WHERE q.query_id = ?1",
                [query_id],
                |row| {
                    Ok(RagQueryRow {
                        query_text: row.get(0)?,
                        program_hash: row.get(1)?,
                        session_id: row.get(2)?,
                        run_id: row.get(3)?,
                        policy_json: row.get(4)?,
                        status: row.get(5)?,
                        created_unix_ms: row.get(6)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| DebugStoreError::RagQueryNotIndexed(query_id.to_owned()))
    }

    fn selected_rag_hit_rows(
        &self,
        query_id: &str,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<RagHitRow>, DebugStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT c.chunk_id, c.source_kind, c.title, c.body, h.fused_score,
                    c.entity_ids_json, c.source_path, c.start_byte, c.end_byte,
                    h.channel, h.channel_rank
             FROM rag_query_hits AS h
             JOIN chunks AS c ON c.chunk_id = h.chunk_id
             WHERE h.query_id = ?1
               AND h.selected = 1
               AND (
                 c.privacy_class = 'public'
                 OR (?2 IN ('project', 'sensitive', 'secret') AND c.privacy_class = 'project')
                 OR (?2 IN ('sensitive', 'secret') AND c.privacy_class = 'sensitive')
                 OR (?2 = 'secret' AND c.privacy_class = 'secret')
               )
             ORDER BY h.fused_score DESC, h.channel_rank, c.chunk_id, h.channel",
        )?;
        let rows = statement.query_map(params![query_id, max_privacy.as_str()], |row| {
            Ok(RagHitRow {
                chunk_id: row.get(0)?,
                source_kind: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                fused_score: row.get(4)?,
                entity_ids_json: row.get(5)?,
                source_path: row.get(6)?,
                start_byte: row.get(7)?,
                end_byte: row.get(8)?,
                channel: row.get(9)?,
                channel_rank: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DebugStoreError::from)
    }

    pub fn upsert_embedding(&self, embedding: &StoredEmbedding) -> Result<(), DebugStoreError> {
        if embedding.values.len() != embedding.model.dimensions as usize {
            return Err(DebugStoreError::StoredDimensionMismatch);
        }
        self.connection.execute(
            "INSERT INTO embeddings(\n               chunk_id, model_id, model_revision, dimensions, original_norm,\n               vector_le_f32, content_hash, created_unix_ms\n             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)\n             ON CONFLICT(chunk_id, model_id, model_revision, dimensions) DO UPDATE SET\n               original_norm = excluded.original_norm,\n               vector_le_f32 = excluded.vector_le_f32,\n               content_hash = excluded.content_hash,\n               created_unix_ms = excluded.created_unix_ms",
            params![
                embedding.chunk_id.as_str(),
                &embedding.model.model_id,
                &embedding.model.model_revision,
                embedding.model.dimensions,
                embedding.original_norm,
                encode_f32_le(&embedding.values),
                &embedding.content_hash,
                embedding.created_unix_ms,
            ],
        )?;
        Ok(())
    }

    pub fn load_embeddings(
        &self,
        model: &EmbeddingModelDescriptor,
    ) -> Result<Vec<StoredEmbedding>, DebugStoreError> {
        self.load_embeddings_with_filter(model, None)
    }

    fn load_embeddings_with_max_privacy(
        &self,
        model: &EmbeddingModelDescriptor,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<StoredEmbedding>, DebugStoreError> {
        self.load_embeddings_with_filter(model, Some(max_privacy))
    }

    fn load_embeddings_with_filter(
        &self,
        model: &EmbeddingModelDescriptor,
        max_privacy: Option<PrivacyClass>,
    ) -> Result<Vec<StoredEmbedding>, DebugStoreError> {
        let max_privacy_filter = max_privacy.map(|privacy| privacy.as_str().to_owned());
        let mut statement = self.connection.prepare(
            "SELECT e.chunk_id, e.original_norm, e.vector_le_f32, e.content_hash, e.created_unix_ms\n             FROM embeddings AS e\n             JOIN chunks AS c ON c.chunk_id = e.chunk_id\n             WHERE e.model_id = ?1 AND e.model_revision = ?2 AND e.dimensions = ?3\n               AND (\n                 ?4 IS NULL\n                 OR c.privacy_class = 'public'\n                 OR (?4 IN ('project', 'sensitive', 'secret') AND c.privacy_class = 'project')\n                 OR (?4 IN ('sensitive', 'secret') AND c.privacy_class = 'sensitive')\n                 OR (?4 = 'secret' AND c.privacy_class = 'secret')\n               )\n             ORDER BY e.chunk_id",
        )?;
        let rows = statement.query_map(
            params![
                &model.model_id,
                &model.model_revision,
                model.dimensions,
                max_privacy_filter.as_deref()
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f32>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )?;
        rows.map(|row| {
            let (chunk_id, original_norm, blob, content_hash, created_unix_ms) = row?;
            let values = decode_f32_le(&blob)?;
            if values.len() != model.dimensions as usize {
                return Err(DebugStoreError::StoredDimensionMismatch);
            }
            Ok(StoredEmbedding {
                chunk_id: ChunkId::new(chunk_id),
                model: model.clone(),
                values,
                original_norm,
                content_hash,
                created_unix_ms,
            })
        })
        .collect()
    }

    fn chunk_search_result_for_hit(
        &self,
        hit: SearchHit,
    ) -> Result<ChunkSearchResult, DebugStoreError> {
        let (title, body, source_kind, source_key, privacy) = self.connection.query_row(
            "SELECT title, body, source_kind, source_key, privacy_class\n             FROM chunks\n             WHERE chunk_id = ?1",
            [hit.chunk_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )?;
        let privacy = PrivacyClass::parse(&privacy)
            .ok_or_else(|| DebugStoreError::InvalidPrivacyClass(privacy.clone()))?;
        Ok(ChunkSearchResult {
            hit,
            title,
            body,
            source_kind,
            source_key,
            privacy,
        })
    }

    fn program_id(&self, hash: &StableHash) -> Result<Option<i64>, DebugStoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT program_id FROM programs WHERE program_hash = ?1",
                [hash.as_str()],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn require_program_id(&self, hash: &StableHash) -> Result<i64, DebugStoreError> {
        self.program_id(hash)?
            .ok_or_else(|| DebugStoreError::ProgramNotIndexed(hash.as_str().to_owned()))
    }

    fn source_file_id(
        &self,
        program_id: i64,
        path: &str,
        content_hash: &str,
    ) -> Result<Option<i64>, DebugStoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT source_file_id
                 FROM source_files
                 WHERE program_id = ?1 AND path = ?2 AND content_hash = ?3",
                params![program_id, path, content_hash],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn table_count(&self, table: &'static str) -> Result<u64, DebugStoreError> {
        let count =
            self.connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })?;
        u64::try_from(count).map_err(|_| DebugStoreError::IntegerOverflow(table))
    }

    fn pragma_count(&self, pragma: &'static str) -> Result<u64, DebugStoreError> {
        let count = self
            .connection
            .query_row(&format!("PRAGMA {pragma}"), [], |row| row.get::<_, i64>(0))?;
        u64::try_from(count).map_err(|_| DebugStoreError::IntegerOverflow(pragma))
    }

    fn integrity_messages(&self) -> Result<Vec<String>, DebugStoreError> {
        let mut statement = self.connection.prepare("PRAGMA integrity_check")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let messages = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(messages
            .into_iter()
            .filter(|message| message != "ok")
            .collect())
    }

    fn foreign_key_violations(
        &self,
    ) -> Result<Vec<DebugStoreForeignKeyViolation>, DebugStoreError> {
        let mut statement = self.connection.prepare("PRAGMA foreign_key_check")?;
        let rows = statement.query_map([], |row| {
            Ok(DebugStoreForeignKeyViolation {
                table: row.get(0)?,
                rowid: row.get(1)?,
                parent: row.get(2)?,
                fkid: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DebugStoreError::from)
    }

    fn missing_capture_blob_refs(&self) -> Result<u64, DebugStoreError> {
        let count = self.connection.query_row(
            "SELECT COUNT(*)
             FROM captures
             LEFT JOIN blobs ON blobs.blob_hash = captures.blob_hash
             WHERE captures.blob_hash IS NOT NULL
               AND blobs.blob_hash IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        u64::try_from(count)
            .map_err(|_| DebugStoreError::IntegerOverflow("captures.missing_blob_refs"))
    }

    fn invalid_embedding_blobs(&self) -> Result<u64, DebugStoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT dimensions, vector_le_f32 FROM embeddings")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut invalid = 0_u64;
        for row in rows {
            let (dimensions, blob) = row?;
            let expected_len = usize::try_from(dimensions)
                .ok()
                .and_then(|dimensions| dimensions.checked_mul(4));
            if expected_len != Some(blob.len()) || decode_f32_le(&blob).is_err() {
                invalid = invalid.saturating_add(1);
            }
        }
        Ok(invalid)
    }

    fn blob_records_where(
        &self,
        clause: &'static str,
    ) -> Result<Vec<DebugStoreBlobRecord>, DebugStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT blob_hash, byte_len, relative_path FROM blobs {clause} ORDER BY blob_hash"
        ))?;
        let rows = statement.query_map([], |row| {
            let byte_len = row.get::<_, i64>(1)?;
            let byte_len = u64::try_from(byte_len)
                .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(1, byte_len))?;
            Ok(DebugStoreBlobRecord {
                blob_hash: row.get(0)?,
                byte_len,
                relative_path: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DebugStoreError::from)
    }
}

impl DebugEventSink for DebugStore {
    type Error = DebugStoreError;

    fn append(&mut self, event: &DebugEvent) -> Result<(), Self::Error> {
        self.connection.execute(
            "INSERT INTO debug_events(\n               session_id, run_id, sequence, tick, event_kind, payload_json, created_unix_ms\n             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.session_id.as_str(),
                event.run_id.as_ref().map(AgentRunId::as_str),
                sqlite_i64(event.sequence, "debug_events.sequence")?,
                event
                    .tick
                    .map(|tick| sqlite_i64(tick, "debug_events.tick"))
                    .transpose()?,
                event.kind.as_str(),
                serde_json::to_string(&event.payload)?,
                event.created_unix_ms,
            ],
        )?;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(PASSIVE);")?;
        Ok(())
    }
}

fn quote_fts_literal(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

fn debug_event_payload_privacy(payload: &serde_json::Value) -> PrivacyClass {
    payload
        .get("privacy_class")
        .or_else(|| payload.get("privacy"))
        .or_else(|| {
            payload
                .get("payload")
                .and_then(|value| value.get("privacy_class"))
        })
        .or_else(|| {
            payload
                .get("payload")
                .and_then(|value| value.get("privacy"))
        })
        .and_then(serde_json::Value::as_str)
        .and_then(PrivacyClass::parse)
        .unwrap_or(PrivacyClass::Project)
}

const fn search_channel_label(channel: SearchChannel) -> &'static str {
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

fn parse_search_channel(value: &str) -> Option<SearchChannel> {
    match value {
        "exact_entity" => Some(SearchChannel::ExactEntity),
        "lexical" => Some(SearchChannel::Lexical),
        "vector" => Some(SearchChannel::Vector),
        "graph" => Some(SearchChannel::Graph),
        "history" => Some(SearchChannel::History),
        "diagnostics" => Some(SearchChannel::Diagnostics),
        "trace" => Some(SearchChannel::Trace),
        "summary" => Some(SearchChannel::Summary),
        _ => None,
    }
}

fn parse_chunk_source_kind(value: &str) -> Option<ChunkSourceKind> {
    match value {
        "source" => Some(ChunkSourceKind::Source),
        "symbol" => Some(ChunkSourceKind::Symbol),
        "graph_summary" => Some(ChunkSourceKind::GraphSummary),
        "diagnostic" => Some(ChunkSourceKind::Diagnostic),
        "test_result" => Some(ChunkSourceKind::TestResult),
        "agent_trace" => Some(ChunkSourceKind::AgentTrace),
        "history" => Some(ChunkSourceKind::History),
        "documentation" => Some(ChunkSourceKind::Documentation),
        _ => None,
    }
}

fn delete_count(count: usize, column: &'static str) -> Result<u64, DebugStoreError> {
    u64::try_from(count).map_err(|_| DebugStoreError::IntegerOverflow(column))
}

fn rag_policy_roots(policy: &serde_json::Value) -> Result<Vec<PublicId>, DebugStoreError> {
    policy
        .get("roots")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(|value| PublicId::new(value.to_owned()).map_err(DebugStoreError::from))
        .collect()
}

fn rag_query_from_audit_row(
    query_id: &str,
    row: &RagQueryRow,
    policy: &serde_json::Value,
) -> Result<RagQuery, DebugStoreError> {
    let program_hash = row
        .program_hash
        .clone()
        .or_else(|| {
            policy
                .get("program_hash")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .ok_or_else(|| DebugStoreError::RagQueryNotIndexed(query_id.to_owned()))?;
    Ok(RagQuery {
        query_id: query_id.to_owned(),
        text: row.query_text.clone(),
        program_hash: StableHash::new(program_hash)?,
        roots: rag_policy_roots(policy)?,
        graph_depth: rag_policy_u32(policy, "graph_depth", 0)?,
        limit: rag_policy_usize(policy, "limit", usize::MAX)?,
        max_context_bytes: rag_policy_usize(policy, "max_context_bytes", usize::MAX)?,
    })
}

fn rag_context_items_from_hit_rows(
    query: &RagQuery,
    policy: &serde_json::Value,
    rows: Vec<RagHitRow>,
) -> Result<(Vec<RagContextItem>, bool), DebugStoreError> {
    let (mut grouped, order) = grouped_rag_hit_rows(rows)?;
    let mut used_bytes = 0usize;
    let mut truncated = policy
        .get("truncated")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let mut items = Vec::new();
    for chunk_id in order {
        let Some(accumulator) = grouped.remove(&chunk_id) else {
            continue;
        };
        if items.len() >= query.limit {
            truncated = true;
            break;
        }
        let remaining = query.max_context_bytes.saturating_sub(used_bytes);
        if remaining == 0 {
            truncated = true;
            break;
        }
        let (item, body_truncated) =
            rag_context_item_from_accumulator(chunk_id, accumulator, remaining)?;
        truncated |= body_truncated;
        used_bytes = used_bytes.saturating_add(item.body.len());
        items.push(item);
        if body_truncated {
            break;
        }
    }
    Ok((items, truncated))
}

fn grouped_rag_hit_rows(
    rows: Vec<RagHitRow>,
) -> Result<(BTreeMap<String, RagHitAccumulator>, Vec<String>), DebugStoreError> {
    let mut grouped = BTreeMap::<String, RagHitAccumulator>::new();
    let mut order = Vec::<String>::new();
    for row in rows {
        let channel = parse_search_channel(&row.channel)
            .ok_or_else(|| DebugStoreError::InvalidSearchChannel(row.channel.clone()))?;
        let entry = grouped.entry(row.chunk_id.clone()).or_insert_with(|| {
            order.push(row.chunk_id.clone());
            RagHitAccumulator {
                source_kind: row.source_kind.clone(),
                title: row.title.clone(),
                body: row.body.clone(),
                fused_score: row.fused_score,
                entity_ids_json: row.entity_ids_json.clone(),
                source_path: row.source_path.clone(),
                start_byte: row.start_byte,
                end_byte: row.end_byte,
                channel_rank: row.channel_rank,
                channels: BTreeSet::new(),
            }
        });
        entry.channel_rank = entry.channel_rank.min(row.channel_rank);
        entry.channels.insert(channel);
    }
    Ok((grouped, order))
}

fn rag_context_item_from_accumulator(
    chunk_id: String,
    accumulator: RagHitAccumulator,
    max_body_bytes: usize,
) -> Result<(RagContextItem, bool), DebugStoreError> {
    let (body, body_truncated) = truncate_utf8(&accumulator.body, max_body_bytes);
    let kind = parse_chunk_source_kind(&accumulator.source_kind)
        .ok_or_else(|| DebugStoreError::InvalidChunkSourceKind(accumulator.source_kind.clone()))?;
    let entity_ids = serde_json::from_str::<Vec<String>>(&accumulator.entity_ids_json)?
        .into_iter()
        .map(PublicId::new)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        RagContextItem {
            chunk_id: ChunkId::new(chunk_id),
            kind,
            title: accumulator.title,
            body,
            fused_score: accumulator.fused_score,
            channels: accumulator.channels,
            entity_ids,
            source_anchor: source_anchor_from_row(
                accumulator.source_path,
                accumulator.start_byte,
                accumulator.end_byte,
            )?,
        },
        body_truncated,
    ))
}

fn rag_policy_u32(
    policy: &serde_json::Value,
    key: &'static str,
    default: u32,
) -> Result<u32, DebugStoreError> {
    policy
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .map(|value| u32::try_from(value).map_err(|_| DebugStoreError::IntegerOverflow(key)))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn rag_policy_usize(
    policy: &serde_json::Value,
    key: &'static str,
    default: usize,
) -> Result<usize, DebugStoreError> {
    policy
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .map(|value| usize::try_from(value).map_err(|_| DebugStoreError::IntegerOverflow(key)))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn source_anchor_from_row(
    path: Option<String>,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
) -> Result<Option<SourceAnchor>, DebugStoreError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let Some(start_byte) = start_byte else {
        return Ok(None);
    };
    let Some(end_byte) = end_byte else {
        return Ok(None);
    };
    Ok(Some(SourceAnchor {
        path,
        start_byte: u64::try_from(start_byte)
            .map_err(|_| DebugStoreError::IntegerOverflow("chunks.start_byte"))?,
        end_byte: u64::try_from(end_byte)
            .map_err(|_| DebugStoreError::IntegerOverflow("chunks.end_byte"))?,
    }))
}

fn raw_debug_chunk_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawDebugChunk> {
    Ok(RawDebugChunk {
        chunk_id: row.get(0)?,
        program_hash: row.get(1)?,
        source_kind: row.get(2)?,
        source_key: row.get(3)?,
        title: row.get(4)?,
        body: row.get(5)?,
        content_hash: row.get(6)?,
        semantic_hash: row.get(7)?,
        source_path: row.get(8)?,
        entity_ids_json: row.get(9)?,
        start_byte: row.get(10)?,
        end_byte: row.get(11)?,
        privacy_class: row.get(12)?,
        metadata_json: row.get(13)?,
        created_unix_ms: row.get(14)?,
    })
}

fn raw_debug_source_file_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawDebugSourceFile> {
    Ok(RawDebugSourceFile {
        program_hash: row.get(0)?,
        path: row.get(1)?,
        language: row.get(2)?,
        content_hash: row.get(3)?,
        byte_len: row.get(4)?,
        metadata_json: row.get(5)?,
    })
}

fn raw_debug_session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawDebugSession> {
    Ok(RawDebugSession {
        session_id: row.get(0)?,
        program_hash: row.get(1)?,
        profile: row.get(2)?,
        transport: row.get(3)?,
        started_unix_ms: row.get(4)?,
        ended_unix_ms: row.get(5)?,
        status: row.get(6)?,
        metadata_json: row.get(7)?,
    })
}

fn raw_debug_script_run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawDebugScriptRun> {
    Ok(RawDebugScriptRun {
        run_id: row.get(0)?,
        session_id: row.get(1)?,
        agent_id: row.get(2)?,
        artifact_hash: row.get(3)?,
        source_hash: row.get(4)?,
        project_binding_mode: row.get(5)?,
        started_sequence: row.get(6)?,
        finished_sequence: row.get(7)?,
        outcome: row.get(8)?,
        partially_effectful: row.get(9)?,
        trace_uri: row.get(10)?,
        error_json: row.get(11)?,
        metadata_json: row.get(12)?,
    })
}

fn debug_chunk_from_raw(raw: RawDebugChunk) -> Result<DebugChunk, DebugStoreError> {
    let source_kind = parse_chunk_source_kind(&raw.source_kind)
        .ok_or_else(|| DebugStoreError::InvalidChunkSourceKind(raw.source_kind.clone()))?;
    let privacy = PrivacyClass::parse(&raw.privacy_class)
        .ok_or_else(|| DebugStoreError::InvalidPrivacyClass(raw.privacy_class.clone()))?;
    let entity_ids = serde_json::from_str::<Vec<String>>(&raw.entity_ids_json)?
        .into_iter()
        .map(PublicId::new)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DebugChunk {
        id: ChunkId::new(raw.chunk_id),
        program_hash: raw.program_hash.map(StableHash::new).transpose()?,
        source_kind,
        source_key: raw.source_key,
        title: raw.title,
        body: raw.body,
        content_hash: StableHash::new(raw.content_hash)?,
        semantic_hash: raw.semantic_hash.map(StableHash::new).transpose()?,
        source_anchor: source_anchor_from_row(raw.source_path, raw.start_byte, raw.end_byte)?,
        entity_ids,
        privacy,
        metadata: serde_json::from_str(&raw.metadata_json)?,
        created_unix_ms: raw.created_unix_ms,
    })
}

fn debug_source_file_from_raw(raw: RawDebugSourceFile) -> Result<DebugSourceFile, DebugStoreError> {
    Ok(DebugSourceFile {
        program_hash: StableHash::new(raw.program_hash)?,
        path: raw.path,
        language: raw.language,
        content_hash: StableHash::new(raw.content_hash)?,
        byte_len: u64::try_from(raw.byte_len)
            .map_err(|_| DebugStoreError::IntegerOverflow("source_files.byte_len"))?,
        metadata: serde_json::from_str(&raw.metadata_json)?,
    })
}

fn debug_session_from_raw(raw: RawDebugSession) -> Result<DebugSession, DebugStoreError> {
    let status = DebugSessionStatus::parse(&raw.status)
        .ok_or_else(|| DebugStoreError::InvalidSessionStatus(raw.status.clone()))?;
    let metadata = serde_json::from_str(&raw.metadata_json)?;
    Ok(DebugSession {
        session_id: SessionId::new(raw.session_id)?,
        program_hash: raw.program_hash.map(StableHash::new).transpose()?,
        profile: raw.profile,
        transport: raw.transport,
        started_unix_ms: raw.started_unix_ms,
        ended_unix_ms: raw.ended_unix_ms,
        status,
        metadata,
    })
}

fn debug_script_run_from_raw(raw: RawDebugScriptRun) -> Result<DebugScriptRun, DebugStoreError> {
    let outcome = DebugScriptRunOutcome::parse(&raw.outcome)
        .ok_or_else(|| DebugStoreError::InvalidScriptRunOutcome(raw.outcome.clone()))?;
    Ok(DebugScriptRun {
        run_id: AgentRunId::new(raw.run_id)?,
        session_id: SessionId::new(raw.session_id)?,
        agent_id: raw.agent_id.map(PublicId::new).transpose()?,
        artifact_hash: raw.artifact_hash.map(StableHash::new).transpose()?,
        source_hash: raw.source_hash.map(StableHash::new).transpose()?,
        project_binding_mode: raw.project_binding_mode,
        started_sequence: u64::try_from(raw.started_sequence)
            .map_err(|_| DebugStoreError::IntegerOverflow("script_runs.started_sequence"))?,
        finished_sequence: raw
            .finished_sequence
            .map(|sequence| {
                u64::try_from(sequence)
                    .map_err(|_| DebugStoreError::IntegerOverflow("script_runs.finished_sequence"))
            })
            .transpose()?,
        outcome,
        partially_effectful: raw.partially_effectful != 0,
        trace_uri: raw.trace_uri,
        error: raw
            .error_json
            .map(|json| serde_json::from_str(&json))
            .transpose()?,
        metadata: serde_json::from_str(&raw.metadata_json)?,
    })
}

fn truncate_utf8(value: &str, max_bytes: usize) -> (String, bool) {
    if value.len() <= max_bytes {
        return (value.to_owned(), false);
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_owned(), true)
}

fn sqlite_i64(value: u64, column: &'static str) -> Result<i64, DebugStoreError> {
    i64::try_from(value).map_err(|_| DebugStoreError::IntegerOverflow(column))
}

const fn sqlite_bool(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn history_score(query: &str, change_id: &str, body: &str) -> f64 {
    let query = query.trim().to_lowercase();
    if change_id.eq_ignore_ascii_case(&query) {
        2.0
    } else if body.to_lowercase().contains(&query) {
        1.0
    } else {
        0.5
    }
}

#[derive(Debug)]
struct GraphSearchRow {
    edge_id: i64,
    edge_kind: String,
    weight: f64,
    distance: i32,
    from_symbol_id: String,
    from_public_id: Option<String>,
    from_qualified_name: Option<String>,
    from_kind: String,
    from_summary: String,
    to_symbol_id: String,
    to_public_id: Option<String>,
    to_qualified_name: Option<String>,
    to_kind: String,
    to_summary: String,
}

#[derive(Debug)]
struct GraphSymbolSearchRow {
    symbol_id: String,
    public_id: Option<String>,
    qualified_name: Option<String>,
    kind: String,
    summary: String,
    semantic_hash: Option<String>,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
}

#[derive(Debug)]
struct RagQueryRow {
    query_text: String,
    program_hash: Option<String>,
    session_id: Option<String>,
    run_id: Option<String>,
    policy_json: String,
    status: String,
    created_unix_ms: i64,
}

#[derive(Debug)]
struct RagHitRow {
    chunk_id: String,
    source_kind: String,
    title: String,
    body: String,
    fused_score: f64,
    entity_ids_json: String,
    source_path: Option<String>,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
    channel: String,
    channel_rank: i64,
}

#[derive(Debug)]
struct RagHitAccumulator {
    source_kind: String,
    title: String,
    body: String,
    fused_score: f64,
    entity_ids_json: String,
    source_path: Option<String>,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
    channel_rank: i64,
    channels: BTreeSet<SearchChannel>,
}

fn graph_chunk_search_result(query: &str, index: usize, row: &GraphSearchRow) -> ChunkSearchResult {
    let from_label = graph_symbol_label(
        &row.from_symbol_id,
        row.from_public_id.as_deref(),
        row.from_qualified_name.as_deref(),
    );
    let to_label = graph_symbol_label(
        &row.to_symbol_id,
        row.to_public_id.as_deref(),
        row.to_qualified_name.as_deref(),
    );
    let title = format!("{from_label} --{}--> {to_label}", row.edge_kind);
    let body = format!(
        "edge_kind={}\nweight={:.6}\ndistance={}\nfrom_kind={}\nfrom_summary={}\nto_kind={}\nto_summary={}",
        row.edge_kind,
        row.weight,
        row.distance,
        row.from_kind,
        row.from_summary,
        row.to_kind,
        row.to_summary
    );
    ChunkSearchResult {
        hit: SearchHit {
            chunk_id: ChunkId::new(format!("graph:{}", row.edge_id)),
            channel: SearchChannel::Graph,
            rank: index + 1,
            score: Some(graph_score(query, row)),
        },
        title,
        body,
        source_kind: "graph_edge".to_owned(),
        source_key: row.edge_id.to_string(),
        privacy: PrivacyClass::Project,
    }
}

fn graph_symbol_chunk_search_result(
    query: &str,
    index: usize,
    row: &GraphSymbolSearchRow,
) -> ChunkSearchResult {
    let label = graph_symbol_label(
        &row.symbol_id,
        row.public_id.as_deref(),
        row.qualified_name.as_deref(),
    );
    let body = format!(
        "symbol_id={}\nkind={}\nsummary={}\nsemantic_hash={}\nstart_byte={}\nend_byte={}",
        row.symbol_id,
        row.kind,
        row.summary,
        row.semantic_hash.as_deref().unwrap_or("-"),
        row.start_byte
            .map_or_else(|| "-".to_owned(), |value| value.to_string()),
        row.end_byte
            .map_or_else(|| "-".to_owned(), |value| value.to_string())
    );
    ChunkSearchResult {
        hit: SearchHit {
            chunk_id: ChunkId::new(format!("graph_symbol:{}", row.symbol_id)),
            channel: SearchChannel::Graph,
            rank: index + 1,
            score: Some(graph_symbol_score(query, row)),
        },
        title: format!("Graph symbol {label}"),
        body,
        source_kind: "graph_symbol".to_owned(),
        source_key: row.symbol_id.clone(),
        privacy: PrivacyClass::Project,
    }
}

fn graph_symbol_label(
    symbol_id: &str,
    public_id: Option<&str>,
    qualified_name: Option<&str>,
) -> String {
    public_id.or(qualified_name).unwrap_or(symbol_id).to_owned()
}

fn graph_score(query: &str, row: &GraphSearchRow) -> f64 {
    let query = query.trim().to_lowercase();
    let base = if row
        .from_public_id
        .as_deref()
        .is_some_and(|id| id.eq_ignore_ascii_case(&query))
        || row
            .to_public_id
            .as_deref()
            .is_some_and(|id| id.eq_ignore_ascii_case(&query))
        || row.edge_kind.eq_ignore_ascii_case(&query)
    {
        row.weight + 2.0
    } else if row.from_summary.to_lowercase().contains(&query)
        || row.to_summary.to_lowercase().contains(&query)
    {
        row.weight + 1.0
    } else {
        row.weight
    };
    base / f64::from(row.distance.max(0) + 1)
}

fn graph_symbol_score(query: &str, row: &GraphSymbolSearchRow) -> f64 {
    let query = query.trim().to_lowercase();
    if row
        .public_id
        .as_deref()
        .is_some_and(|id| id.eq_ignore_ascii_case(&query))
        || row
            .qualified_name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(&query))
    {
        2.0
    } else if row.summary.to_lowercase().contains(&query) {
        1.0
    } else {
        0.5
    }
}

#[derive(Clone, Copy)]
struct DiagnosticSearchBodyFields<'a> {
    phase: &'a str,
    message: &'a str,
    source_path: Option<&'a str>,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
    sequence: Option<i64>,
    related_ids_json: &'a str,
    payload_json: &'a str,
}

fn diagnostic_search_body(fields: DiagnosticSearchBodyFields<'_>) -> String {
    let mut lines = vec![format!("phase={}", fields.phase), fields.message.to_owned()];
    if let Some(sequence) = fields.sequence {
        lines.push(format!("sequence={sequence}"));
    }
    if let Some(source_path) = fields.source_path {
        lines.push(format!("source_path={source_path}"));
    }
    if let (Some(start), Some(end)) = (fields.start_byte, fields.end_byte) {
        lines.push(format!("range={start}..{end}"));
    }
    if fields.related_ids_json != "[]" {
        lines.push(format!("related_ids={}", fields.related_ids_json));
    }
    if fields.payload_json != "{}" {
        lines.push(format!("payload={}", fields.payload_json));
    }
    lines.join("\n")
}

fn diagnostic_score(query: &str, code: Option<&str>, severity: &str, body: &str) -> f64 {
    let query = query.trim().to_lowercase();
    let exact = if code.is_some_and(|code| code.eq_ignore_ascii_case(&query)) {
        4.0
    } else {
        0.0
    };
    let severity_boost = match severity {
        "error" => 2.0,
        "warning" => 1.0,
        _ => 0.0,
    };
    let body_match = if body.to_lowercase().contains(&query) {
        1.0
    } else {
        0.0
    };
    exact + severity_boost + body_match
}

fn test_result_search_body(
    duration_millis: Option<i64>,
    diagnostic_ids_json: &str,
    artifact_refs_json: &str,
    summary: &str,
) -> String {
    let mut lines = Vec::new();
    if let Some(duration) = duration_millis {
        lines.push(format!("duration_millis={duration}"));
    }
    if diagnostic_ids_json != "[]" {
        lines.push(format!("diagnostic_ids={diagnostic_ids_json}"));
    }
    if artifact_refs_json != "[]" {
        lines.push(format!("artifact_refs={artifact_refs_json}"));
    }
    if !summary.is_empty() {
        lines.push(summary.to_owned());
    }
    lines.join("\n")
}

fn test_result_score(query: &str, test_id: &str, outcome: &str, body: &str) -> f64 {
    let query = query.trim().to_lowercase();
    let exact = if test_id.eq_ignore_ascii_case(&query) {
        4.0
    } else {
        0.0
    };
    let outcome_boost = match outcome {
        "failed" | "error" => 2.0,
        "flaky" => 1.0,
        _ => 0.0,
    };
    let body_match = if body.to_lowercase().contains(&query) {
        1.0
    } else {
        0.0
    };
    exact + outcome_boost + body_match
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_debug_model::{
        chunk::{ChunkSourceKind, PrivacyClass},
        diagnostic::DebugDiagnostic,
        embedding::{EmbeddingInputPolicy, StoredEmbedding},
        event::DebugEventKind,
        graph::{DebugGraphEdge, DebugGraphSymbol},
        history::DebugHistoryEntry,
        repl::DebugReplCell,
        script::{DebugScriptRun, DebugScriptRunFinish, DebugScriptRunOutcome},
        session::{DebugSession, DebugSessionStatus},
        sink::DebugEventSink,
        test_result::DebugTestResult,
    };
    use std::collections::BTreeMap;

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
        assert!(remote_inputs.iter().all(|input| {
            matches!(input.privacy, PrivacyClass::Public | PrivacyClass::Project)
        }));
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
                source_path: Some("samples/rich-text-effects-animation.arcw".to_owned()),
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
            .test_result_search_with_max_privacy(
                "rich-text-visual-regression",
                1,
                PrivacyClass::Public,
            )
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
            .session_timeline_with_max_privacy(
                Some(session_id.as_str()),
                None,
                1,
                PrivacyClass::Public,
            )
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
                symbol_id: "symbol:textbox.main".to_owned(),
                program_hash: program_hash.clone(),
                public_id: Some(PublicId::new("@textbox.main").expect("public id")),
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
            .expect("expanded symbol");
        store
            .upsert_graph_edge(&DebugGraphEdge {
                program_hash,
                from_symbol_id: "symbol:choice.alice".to_owned(),
                to_symbol_id: "symbol:textbox.main".to_owned(),
                edge_kind: "uses_textbox".to_owned(),
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
            expanded_hits.iter().any(
                |hit| hit.hit.chunk_id.as_str() == "graph:2" && hit.body.contains("distance=2")
            )
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
}
