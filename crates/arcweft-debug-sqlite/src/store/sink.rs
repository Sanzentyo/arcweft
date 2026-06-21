use arcweft_agent_protocol::ids::AgentRunId;
use arcweft_debug_model::{event::DebugEvent, sink::DebugEventSink};
use rusqlite::params;

use super::{DebugStore, DebugStoreError, convert::sqlite_i64};
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
