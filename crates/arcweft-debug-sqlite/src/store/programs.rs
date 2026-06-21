use arcweft_agent_protocol::ids::StableHash;
use arcweft_debug_model::source::DebugSourceFile;
use rusqlite::params;

use super::DebugStore;
use super::{
    DebugStoreError,
    convert::{debug_source_file_from_raw, raw_debug_source_file_from_row, sqlite_i64},
};

impl DebugStore {
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
}
