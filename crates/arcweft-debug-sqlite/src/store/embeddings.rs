use crate::encoding::{decode_f32_le, encode_f32_le};
use arcweft_debug_model::{
    chunk::{ChunkId, PrivacyClass},
    embedding::{EmbeddingModelDescriptor, StoredEmbedding},
};
use rusqlite::params;

use super::DebugStore;
use super::DebugStoreError;

impl DebugStore {
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
        self.load_embeddings_with_filter(model, None)
    }

    pub(crate) fn load_embeddings_with_max_privacy(
        &self,
        model: &EmbeddingModelDescriptor,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<StoredEmbedding>, DebugStoreError> {
        self.load_embeddings_with_filter(model, Some(max_privacy))
    }

    fn load_embeddings_with_filter(
        &self,
        model: &EmbeddingModelDescriptor,
        max_privacy: Option<PrivacyClass>,
    ) -> Result<Vec<StoredEmbedding>, DebugStoreError> {
        let max_privacy_filter = max_privacy.map(|privacy| privacy.as_str().to_owned());
        let mut statement = self.connection.prepare(
            "SELECT e.chunk_id, e.original_norm, e.vector_le_f32, e.content_hash, e.created_unix_ms\n             FROM embeddings AS e\n             JOIN chunks AS c ON c.chunk_id = e.chunk_id\n             WHERE e.model_id = ?1 AND e.model_revision = ?2 AND e.dimensions = ?3\n               AND (\n                 ?4 IS NULL\n                 OR c.privacy_class = 'public'\n                 OR (?4 IN ('project', 'sensitive', 'secret') AND c.privacy_class = 'project')\n                 OR (?4 IN ('sensitive', 'secret') AND c.privacy_class = 'sensitive')\n                 OR (?4 = 'secret' AND c.privacy_class = 'secret')\n               )\n             ORDER BY e.chunk_id",
        )?;
        let rows = statement.query_map(
            params![
                &model.model_id,
                &model.model_revision,
                model.dimensions,
                max_privacy_filter.as_deref()
            ],
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
}
