use crate::chunk::{ChunkId, DebugChunk, PrivacyClass};
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
    pub content_hash: String,
    pub privacy: PrivacyClass,
    pub text: String,
}

/// Embedding provider trust boundary used before adapter I/O.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProviderScope {
    Local,
    Remote,
}

impl EmbeddingProviderScope {
    /// Returns whether this provider boundary may receive the privacy class.
    pub const fn allows(self, privacy: PrivacyClass) -> bool {
        let maximum = match self {
            Self::Local => PrivacyClass::Sensitive,
            Self::Remote => PrivacyClass::Project,
        };
        privacy.is_allowed_by(maximum)
    }
}

/// Privacy policy applied before any embedding provider sees chunk text.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EmbeddingInputPolicy {
    pub scope: EmbeddingProviderScope,
    pub max_privacy: PrivacyClass,
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

impl EmbeddingInput {
    pub fn from_chunk(chunk: &DebugChunk) -> Self {
        Self {
            chunk_id: chunk.id.clone(),
            content_hash: chunk.content_hash.as_str().to_owned(),
            privacy: chunk.privacy,
            text: embedding_text(chunk),
        }
    }
}

impl EmbeddingInputPolicy {
    pub const fn local(max_privacy: PrivacyClass) -> Self {
        Self {
            scope: EmbeddingProviderScope::Local,
            max_privacy,
        }
    }

    pub const fn remote(max_privacy: PrivacyClass) -> Self {
        Self {
            scope: EmbeddingProviderScope::Remote,
            max_privacy,
        }
    }

    pub const fn allows(self, privacy: PrivacyClass) -> bool {
        self.scope.allows(privacy) && privacy.is_allowed_by(self.max_privacy)
    }
}

pub fn embedding_inputs_for_chunks<'a>(
    chunks: impl IntoIterator<Item = &'a DebugChunk>,
    policy: EmbeddingInputPolicy,
) -> Vec<EmbeddingInput> {
    chunks
        .into_iter()
        .filter(|chunk| policy.allows(chunk.privacy))
        .map(EmbeddingInput::from_chunk)
        .collect()
}

fn embedding_text(chunk: &DebugChunk) -> String {
    if chunk.title.is_empty() {
        return chunk.body.clone();
    }
    if chunk.body.is_empty() {
        return chunk.title.clone();
    }
    format!("{}\n{}", chunk.title, chunk.body)
}

/// Provider boundary. Concrete local/remote I/O belongs in adapter crates.
pub trait EmbeddingProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn descriptor(&self) -> EmbeddingModelDescriptor;

    fn embed(&mut self, inputs: &[EmbeddingInput]) -> Result<Vec<StoredEmbedding>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{ChunkSourceKind, SourceAnchor};
    use arcweft_agent_protocol::ids::StableHash;
    use std::collections::BTreeMap;

    #[test]
    fn embedding_input_policy_filters_before_provider_visibility() {
        let chunks = [
            chunk("public", PrivacyClass::Public),
            chunk("project", PrivacyClass::Project),
            chunk("sensitive", PrivacyClass::Sensitive),
            chunk("secret", PrivacyClass::Secret),
        ];

        let local = embedding_inputs_for_chunks(
            chunks.iter(),
            EmbeddingInputPolicy::local(PrivacyClass::Sensitive),
        );
        assert_eq!(
            local
                .iter()
                .map(|input| input.chunk_id.as_str())
                .collect::<Vec<_>>(),
            vec!["chunk:public", "chunk:project", "chunk:sensitive"]
        );

        let remote = embedding_inputs_for_chunks(
            chunks.iter(),
            EmbeddingInputPolicy::remote(PrivacyClass::Secret),
        );
        assert_eq!(
            remote
                .iter()
                .map(|input| input.chunk_id.as_str())
                .collect::<Vec<_>>(),
            vec!["chunk:public", "chunk:project"]
        );
    }

    #[test]
    fn embedding_input_preserves_hash_privacy_and_stable_text() {
        let chunk = chunk("project", PrivacyClass::Project);
        let input = EmbeddingInput::from_chunk(&chunk);

        assert_eq!(input.chunk_id.as_str(), "chunk:project");
        assert_eq!(input.content_hash, chunk.content_hash.as_str());
        assert_eq!(input.privacy, PrivacyClass::Project);
        assert_eq!(input.text, "project title\nproject body");
    }

    fn chunk(name: &str, privacy: PrivacyClass) -> DebugChunk {
        DebugChunk {
            id: ChunkId::new(format!("chunk:{name}")),
            program_hash: None,
            source_kind: ChunkSourceKind::Documentation,
            source_key: name.to_owned(),
            title: format!("{name} title"),
            body: format!("{name} body"),
            content_hash: StableHash::new(format!("blake3:{name}")).expect("hash"),
            semantic_hash: None,
            source_anchor: Some(SourceAnchor {
                path: format!("{name}.arcw"),
                start_byte: 0,
                end_byte: 4,
            }),
            entity_ids: Vec::new(),
            privacy,
            metadata: BTreeMap::new(),
            created_unix_ms: 0,
        }
    }
}
