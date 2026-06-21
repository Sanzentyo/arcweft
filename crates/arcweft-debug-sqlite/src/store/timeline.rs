use arcweft_agent_protocol::ids::{AgentRunId, PublicId, SessionId};
use arcweft_debug_model::{
    chunk::PrivacyClass, embedding::EmbeddingModelDescriptor, rag::RagContextPack,
};
use rusqlite::{OptionalExtension, params};

use super::DebugStore;
use super::{
    DebugRagQueryAudit, DebugStoreError, DebugTimelineEvent,
    convert::sqlite_i64,
    helpers::{debug_event_payload_privacy, search_channel_label},
    rag::{
        RagHitRow, RagQueryRow, rag_context_items_from_hit_rows, rag_policy_u32,
        rag_query_from_audit_row,
    },
};

impl DebugStore {
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
}
