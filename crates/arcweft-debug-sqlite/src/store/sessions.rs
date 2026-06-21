use std::collections::BTreeMap;

use arcweft_agent_protocol::ids::{AgentRunId, PublicId, SessionId, StableHash};
use arcweft_debug_model::{
    script::{DebugScriptRun, DebugScriptRunFinish},
    session::{DebugSession, DebugSessionStatus},
};
use rusqlite::{OptionalExtension, params};

use super::DebugStore;
use super::{
    DebugStoreError,
    convert::{
        debug_script_run_from_raw, debug_session_from_raw, raw_debug_script_run_from_row,
        raw_debug_session_from_row, sqlite_bool, sqlite_i64,
    },
};

impl DebugStore {
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
}
