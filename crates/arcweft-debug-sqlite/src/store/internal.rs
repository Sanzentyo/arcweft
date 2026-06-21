use crate::encoding::decode_f32_le;
use arcweft_agent_protocol::ids::StableHash;
use arcweft_debug_model::chunk::PrivacyClass;
use arcweft_debug_model::rag::SearchHit;
use rusqlite::{OptionalExtension, params};

use super::DebugStore;
use super::{
    ChunkSearchResult, DebugStoreBlobRecord, DebugStoreError, DebugStoreForeignKeyViolation,
};

impl DebugStore {
    pub(crate) fn chunk_search_result_for_hit(
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

    pub(crate) fn program_id(&self, hash: &StableHash) -> Result<Option<i64>, DebugStoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT program_id FROM programs WHERE program_hash = ?1",
                [hash.as_str()],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub(crate) fn require_program_id(&self, hash: &StableHash) -> Result<i64, DebugStoreError> {
        self.program_id(hash)?
            .ok_or_else(|| DebugStoreError::ProgramNotIndexed(hash.as_str().to_owned()))
    }

    pub(crate) fn source_file_id(
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

    pub(crate) fn table_count(&self, table: &'static str) -> Result<u64, DebugStoreError> {
        let count =
            self.connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })?;
        u64::try_from(count).map_err(|_| DebugStoreError::IntegerOverflow(table))
    }

    pub(crate) fn pragma_count(&self, pragma: &'static str) -> Result<u64, DebugStoreError> {
        let count = self
            .connection
            .query_row(&format!("PRAGMA {pragma}"), [], |row| row.get::<_, i64>(0))?;
        u64::try_from(count).map_err(|_| DebugStoreError::IntegerOverflow(pragma))
    }

    pub(crate) fn integrity_messages(&self) -> Result<Vec<String>, DebugStoreError> {
        let mut statement = self.connection.prepare("PRAGMA integrity_check")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let messages = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(messages
            .into_iter()
            .filter(|message| message != "ok")
            .collect())
    }

    pub(crate) fn foreign_key_violations(
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

    pub(crate) fn missing_capture_blob_refs(&self) -> Result<u64, DebugStoreError> {
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

    pub(crate) fn invalid_embedding_blobs(&self) -> Result<u64, DebugStoreError> {
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

    pub(crate) fn blob_records_where(
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
