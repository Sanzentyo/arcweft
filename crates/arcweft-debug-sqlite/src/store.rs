use crate::encoding::{VectorBlobError, decode_f32_le, encode_f32_le};
use arcweft_agent_protocol::ids::{AgentRunId, PublicId, SessionId, StableHash};
use arcweft_debug_model::{
    chunk::{ChunkId, DebugChunk},
    embedding::{EmbeddingModelDescriptor, StoredEmbedding},
    event::DebugEvent,
    rag::{SearchChannel, SearchHit},
    sink::DebugEventSink,
};
use rusqlite::{Connection, OptionalExtension, params};
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
}

/// One lexical result returned from FTS5.
#[derive(Clone, Debug, PartialEq)]
pub struct LexicalResult {
    pub hit: SearchHit,
    pub title: String,
    pub body: String,
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

    pub fn start_session(
        &self,
        session_id: &SessionId,
        program_hash: Option<&StableHash>,
        profile: &str,
        transport: &str,
        started_unix_ms: i64,
    ) -> Result<(), DebugStoreError> {
        let program_id = program_hash
            .map(|hash| self.require_program_id(hash))
            .transpose()?;
        self.connection.execute(
            "INSERT INTO sessions(\n               session_id, program_id, profile, transport, started_unix_ms, status\n             ) VALUES (?1, ?2, ?3, ?4, ?5, 'running')\n             ON CONFLICT(session_id) DO UPDATE SET\n               program_id = excluded.program_id,\n               profile = excluded.profile,\n               transport = excluded.transport,\n               status = 'running'",
            params![
                session_id.as_str(),
                program_id,
                profile,
                transport,
                started_unix_ms,
            ],
        )?;
        Ok(())
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

    pub fn lexical_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LexicalResult>, DebugStoreError> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let fts_query = quote_fts_literal(query.trim());
        let mut statement = self.connection.prepare(
            "SELECT c.chunk_id, c.title, c.body, bm25(chunks_fts, 2.0, 1.0)\n             FROM chunks_fts\n             JOIN chunks AS c ON c.rowid = chunks_fts.rowid\n             WHERE chunks_fts MATCH ?1\n             ORDER BY bm25(chunks_fts, 2.0, 1.0), c.chunk_id\n             LIMIT ?2",
        )?;
        let limit = i64::try_from(limit)
            .map_err(|_| DebugStoreError::IntegerOverflow("chunks_fts.limit"))?;
        let rows = statement.query_map(params![fts_query, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })?;
        let values = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(values
            .into_iter()
            .enumerate()
            .map(|(index, (chunk_id, title, body, bm25))| LexicalResult {
                hit: SearchHit {
                    chunk_id: ChunkId::new(chunk_id),
                    channel: SearchChannel::Lexical,
                    rank: index + 1,
                    score: Some(-bm25),
                },
                title,
                body,
            })
            .collect())
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
        let mut statement = self.connection.prepare(
            "SELECT chunk_id, original_norm, vector_le_f32, content_hash, created_unix_ms\n             FROM embeddings\n             WHERE model_id = ?1 AND model_revision = ?2 AND dimensions = ?3\n             ORDER BY chunk_id",
        )?;
        let rows = statement.query_map(
            params![&model.model_id, &model.model_revision, model.dimensions],
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

fn sqlite_i64(value: u64, column: &'static str) -> Result<i64, DebugStoreError> {
    i64::try_from(value).map_err(|_| DebugStoreError::IntegerOverflow(column))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_debug_model::{
        chunk::{ChunkSourceKind, PrivacyClass},
        embedding::StoredEmbedding,
    };
    use std::collections::BTreeMap;

    fn hash(value: &str) -> StableHash {
        StableHash::new(value).expect("non-empty hash")
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
}
