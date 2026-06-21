use arcweft_agent_protocol::ids::{AgentRunId, SessionId, StableHash};
use arcweft_debug_model::{
    chunk::{ChunkId, PrivacyClass},
    diagnostic::DebugDiagnostic,
    history::DebugHistoryEntry,
    rag::{SearchChannel, SearchHit},
    repl::DebugReplCell,
    test_result::DebugTestResult,
};
use rusqlite::params;

use super::DebugStore;
use super::{
    ChunkSearchResult, DebugStoreError,
    convert::{history_score, sqlite_i64},
    search::{
        DiagnosticSearchBodyFields, diagnostic_score, diagnostic_search_body, test_result_score,
        test_result_search_body,
    },
};

impl DebugStore {
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
}
