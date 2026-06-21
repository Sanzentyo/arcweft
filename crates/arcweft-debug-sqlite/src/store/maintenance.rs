use std::path::Path;

use rusqlite::Connection;

use super::{
    DebugStoreBlobRecord, DebugStoreError, DebugStorePruneReport, DebugStoreReindexReport,
    DebugStoreStats, DebugStoreVacuumReport, DebugStoreValidationReport, helpers::delete_count,
    schema::MIGRATION_V1,
};

use super::DebugStore;
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
}
