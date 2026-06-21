use crate::encoding::VectorBlobError;
use arcweft_agent_protocol::ids::{AgentRunId, IdentifierError, SessionId};
use arcweft_debug_model::{
    chunk::{DebugChunk, PrivacyClass},
    rag::{RagContextPack, SearchHit},
};
use arcweft_rag::vector::VectorSearchError;
use rusqlite::Connection;
use thiserror::Error;

mod artifacts;
mod chunks;
mod convert;
mod embeddings;
mod graph;
mod helpers;
mod internal;
mod maintenance;
mod programs;
mod rag;
mod raw;
mod schema;
mod search;
mod sessions;
mod sink;
mod timeline;

#[cfg(test)]
mod tests;

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
    pub(crate) connection: Connection,
}
