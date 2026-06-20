use arcweft_debug_model::embedding::{
    EmbeddingError, EmbeddingInput, EmbeddingModelDescriptor, EmbeddingProvider, StoredEmbedding,
};
use std::error::Error;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

pub(in crate::app) const DEFAULT_LOCAL_EMBEDDING_MODEL_ID: &str = "arcweft-local-hash";
pub(in crate::app) const DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION: &str = "1";
pub(in crate::app) const DEFAULT_LOCAL_EMBEDDING_DIMENSIONS: u32 = 32;
pub(in crate::app) const MAX_LOCAL_EMBEDDING_DIMENSIONS: u32 = 4096;

#[derive(Debug)]
pub(in crate::app) struct LocalHashEmbeddingError {
    source: EmbeddingError,
}

impl fmt::Display for LocalHashEmbeddingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.source)
    }
}

impl Error for LocalHashEmbeddingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

pub(in crate::app) struct LocalHashEmbeddingProvider {
    descriptor: EmbeddingModelDescriptor,
}

impl LocalHashEmbeddingProvider {
    pub(in crate::app) fn new(descriptor: EmbeddingModelDescriptor) -> Self {
        Self { descriptor }
    }
}

impl EmbeddingProvider for LocalHashEmbeddingProvider {
    type Error = LocalHashEmbeddingError;

    fn descriptor(&self) -> EmbeddingModelDescriptor {
        self.descriptor.clone()
    }

    fn embed(&mut self, inputs: &[EmbeddingInput]) -> Result<Vec<StoredEmbedding>, Self::Error> {
        let descriptor = self.descriptor();
        let created_unix_ms = current_unix_millis_for_embedding();
        inputs
            .iter()
            .map(|input| {
                let values = local_hash_embedding_values(
                    &input.text,
                    input.chunk_id.as_str(),
                    descriptor.dimensions,
                );
                StoredEmbedding::normalized(
                    input.chunk_id.clone(),
                    descriptor.clone(),
                    values,
                    input.content_hash.clone(),
                    created_unix_ms,
                )
                .map_err(|source| LocalHashEmbeddingError { source })
            })
            .collect()
    }
}

fn current_unix_millis_for_embedding() -> i64 {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

pub(in crate::app) fn local_hash_query_embedding(text: &str, dimensions: u32) -> Vec<f32> {
    local_hash_embedding_values(text, "query", dimensions)
}

fn local_hash_embedding_values(text: &str, fallback_identity: &str, dimensions: u32) -> Vec<f32> {
    let dimensions = usize::try_from(dimensions).expect("u32 dimensions fit usize");
    let mut values = vec![0.0; dimensions];
    let dimensions_u64 = u64::try_from(dimensions).expect("dimensions fit u64");
    let source = if text.trim().is_empty() {
        fallback_identity
    } else {
        text
    };
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in source.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        let index = usize::try_from(hash % dimensions_u64).expect("index fits usize");
        let sign = if hash & 1 == 0 { 1.0 } else { -1.0 };
        let weight = 1.0 + f32::from(byte % 7) / 7.0;
        values[index] += sign * weight;
    }
    if values.iter().all(|value| value.abs() <= f32::EPSILON) {
        values[0] = 1.0;
    }
    values
}
