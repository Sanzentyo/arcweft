use crate::chunk::ChunkId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Versioned embedding model identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EmbeddingModelDescriptor {
    pub model_id: String,
    pub model_revision: String,
    pub dimensions: u32,
}

/// Normalized vector stored for one debug chunk.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StoredEmbedding {
    pub chunk_id: ChunkId,
    pub model: EmbeddingModelDescriptor,
    pub values: Vec<f32>,
    pub original_norm: f32,
    pub content_hash: String,
    pub created_unix_ms: i64,
}

/// Embedding input after privacy filtering and redaction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EmbeddingInput {
    pub chunk_id: ChunkId,
    pub text: String,
}

/// Invalid vector supplied by an embedding adapter.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EmbeddingError {
    #[error("embedding dimensions do not match the model descriptor")]
    DimensionMismatch,
    #[error("embedding contains a non-finite value")]
    NonFinite,
    #[error("embedding has zero norm")]
    ZeroNorm,
}

impl StoredEmbedding {
    pub fn normalized(
        chunk_id: ChunkId,
        model: EmbeddingModelDescriptor,
        values: Vec<f32>,
        content_hash: impl Into<String>,
        created_unix_ms: i64,
    ) -> Result<Self, EmbeddingError> {
        if values.len() != model.dimensions as usize {
            return Err(EmbeddingError::DimensionMismatch);
        }
        if values.iter().any(|value| !value.is_finite()) {
            return Err(EmbeddingError::NonFinite);
        }
        let squared_norm = values.iter().map(|value| value * value).sum::<f32>();
        if squared_norm <= f32::EPSILON {
            return Err(EmbeddingError::ZeroNorm);
        }
        let original_norm = squared_norm.sqrt();
        let values = values
            .into_iter()
            .map(|value| value / original_norm)
            .collect();
        Ok(Self {
            chunk_id,
            model,
            values,
            original_norm,
            content_hash: content_hash.into(),
            created_unix_ms,
        })
    }
}

/// Provider boundary. Concrete local/remote I/O belongs in adapter crates.
pub trait EmbeddingProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn descriptor(&self) -> EmbeddingModelDescriptor;

    fn embed(&mut self, inputs: &[EmbeddingInput]) -> Result<Vec<StoredEmbedding>, Self::Error>;
}
