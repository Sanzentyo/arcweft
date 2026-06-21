use arcweft_agent_protocol::ids::{PublicId, SessionId, StableHash};
use arcweft_debug_model::{
    chunk::{DebugChunk, PrivacyClass},
    embedding::{
        EmbeddingInput, EmbeddingInputPolicy, EmbeddingModelDescriptor, embedding_inputs_for_chunks,
    },
    rag::{SearchChannel, SearchHit},
};
use arcweft_rag::vector::{VectorCandidate, rank_vectors};
use rusqlite::params;

use super::DebugStore;
use super::{
    ChunkSearchResult, DebugChunkSearchResult, DebugStoreError,
    convert::{debug_chunk_from_raw, raw_debug_chunk_from_row, sqlite_i64},
    helpers::quote_fts_literal,
};

impl DebugStore {
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
}
