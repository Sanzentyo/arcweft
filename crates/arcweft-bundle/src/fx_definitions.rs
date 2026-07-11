//! Deterministic first-class bundle inventory for executable Fx definitions.

use std::collections::BTreeSet;

use arcweft_presentation::fx::{
    FX_MAX_DEFINITIONS_PER_SECTION, FX_MAX_TOTAL_GRAPH_NODES_PER_SECTION, FxDefinition, FxGraph,
    FxNode,
};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

const MAGIC: [u8; 8] = *b"AWFXDEF\0";
const CODEC_VERSION: u32 = 1;
const HEADER_LEN: usize = 56;
const MAX_SECTION_BYTES: usize = 32 * 1024 * 1024;

/// Canonically ordered executable definitions stored in one AWFB section.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FxDefinitions(Vec<FxDefinition>);

/// Invalid inventory or deterministic section bytes.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxDefinitionsError {
    #[error("Fx definitions section has {actual} definitions, exceeding the limit of {limit}")]
    TooManyDefinitions { actual: usize, limit: usize },
    #[error("Fx definitions section has {actual} graph nodes, exceeding the limit of {limit}")]
    TooManyGraphNodes { actual: usize, limit: usize },
    #[error("Fx definitions section repeats identity `{id}`")]
    DuplicateDefinition { id: String },
    #[error("Fx definitions section exceeds the byte limit of {limit}")]
    SectionTooLarge { limit: usize },
    #[error("Fx definitions section header is truncated")]
    TruncatedHeader,
    #[error("Fx definitions section has invalid magic")]
    InvalidMagic,
    #[error("unsupported Fx definitions codec version {actual}")]
    UnsupportedVersion { actual: u32 },
    #[error("Fx definitions section length does not match its header")]
    LengthMismatch,
    #[error("Fx definitions section count does not match its payload")]
    CountMismatch,
    #[error("Fx definitions section digest mismatch")]
    DigestMismatch,
    #[error("Fx definitions payload encode failed: {message}")]
    Encode { message: String },
    #[error("Fx definitions payload decode failed: {message}")]
    Decode { message: String },
}

impl FxDefinitions {
    /// Validates limits and produces canonical `FxId` order.
    pub fn try_new(
        definitions: impl IntoIterator<Item = FxDefinition>,
    ) -> Result<Self, FxDefinitionsError> {
        let mut definitions = definitions.into_iter().collect::<Vec<_>>();
        if definitions.len() > FX_MAX_DEFINITIONS_PER_SECTION {
            return Err(FxDefinitionsError::TooManyDefinitions {
                actual: definitions.len(),
                limit: FX_MAX_DEFINITIONS_PER_SECTION,
            });
        }
        definitions.sort_by(|left, right| left.id().cmp(right.id()));
        let mut identities = BTreeSet::new();
        for definition in &definitions {
            if !identities.insert(definition.id()) {
                return Err(FxDefinitionsError::DuplicateDefinition {
                    id: definition.id().to_string(),
                });
            }
        }
        let node_count = definitions
            .iter()
            .map(|definition| graph_node_count(definition.graph()))
            .fold(0_usize, usize::saturating_add);
        if node_count > FX_MAX_TOTAL_GRAPH_NODES_PER_SECTION {
            return Err(FxDefinitionsError::TooManyGraphNodes {
                actual: node_count,
                limit: FX_MAX_TOTAL_GRAPH_NODES_PER_SECTION,
            });
        }
        Ok(Self(definitions))
    }

    pub fn definitions(&self) -> &[FxDefinition] {
        &self.0
    }

    pub fn get(&self, id: &arcweft_presentation::fx::FxId) -> Option<&FxDefinition> {
        self.0
            .binary_search_by(|definition| definition.id().cmp(id))
            .ok()
            .map(|index| &self.0[index])
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Encodes canonical header, payload length, payload digest, and typed JSON payload.
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, FxDefinitionsError> {
        let canonical = Self::try_new(self.0.clone())?;
        let payload =
            serde_json::to_vec(&canonical.0).map_err(|source| FxDefinitionsError::Encode {
                message: source.to_string(),
            })?;
        let total_len = HEADER_LEN.saturating_add(payload.len());
        if total_len > MAX_SECTION_BYTES {
            return Err(FxDefinitionsError::SectionTooLarge {
                limit: MAX_SECTION_BYTES,
            });
        }
        let mut bytes = Vec::with_capacity(total_len);
        bytes.extend_from_slice(&MAGIC);
        bytes.extend_from_slice(&CODEC_VERSION.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(canonical.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        bytes.extend_from_slice(
            &u64::try_from(payload.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        bytes.extend_from_slice(blake3::hash(&payload).as_bytes());
        bytes.extend_from_slice(&payload);
        Ok(bytes)
    }

    /// Decodes and revalidates every definition, stored hash, inventory limit, and digest.
    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, FxDefinitionsError> {
        if bytes.len() > MAX_SECTION_BYTES {
            return Err(FxDefinitionsError::SectionTooLarge {
                limit: MAX_SECTION_BYTES,
            });
        }
        if bytes.len() < HEADER_LEN {
            return Err(FxDefinitionsError::TruncatedHeader);
        }
        if bytes[..8] != MAGIC {
            return Err(FxDefinitionsError::InvalidMagic);
        }
        let version = u32::from_le_bytes(
            bytes[8..12]
                .try_into()
                .map_err(|_| FxDefinitionsError::TruncatedHeader)?,
        );
        if version != CODEC_VERSION {
            return Err(FxDefinitionsError::UnsupportedVersion { actual: version });
        }
        let expected_count = u32::from_le_bytes(
            bytes[12..16]
                .try_into()
                .map_err(|_| FxDefinitionsError::TruncatedHeader)?,
        );
        let payload_len = u64::from_le_bytes(
            bytes[16..24]
                .try_into()
                .map_err(|_| FxDefinitionsError::TruncatedHeader)?,
        );
        let payload_len =
            usize::try_from(payload_len).map_err(|_| FxDefinitionsError::LengthMismatch)?;
        if HEADER_LEN.checked_add(payload_len) != Some(bytes.len()) {
            return Err(FxDefinitionsError::LengthMismatch);
        }
        let payload = &bytes[HEADER_LEN..];
        if blake3::hash(payload).as_bytes() != &bytes[24..56] {
            return Err(FxDefinitionsError::DigestMismatch);
        }
        let definitions =
            serde_json::from_slice::<Vec<FxDefinition>>(payload).map_err(|source| {
                FxDefinitionsError::Decode {
                    message: source.to_string(),
                }
            })?;
        if usize::try_from(expected_count).ok() != Some(definitions.len()) {
            return Err(FxDefinitionsError::CountMismatch);
        }
        Self::try_new(definitions)
    }
}

impl<'de> Deserialize<'de> for FxDefinitions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(Vec::<FxDefinition>::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

fn graph_node_count(graph: &FxGraph) -> usize {
    graph.nodes().iter().fold(0_usize, |count, node| {
        let children = match node {
            FxNode::Conditional {
                then_graph,
                else_graph,
                ..
            } => graph_node_count(then_graph).saturating_add(graph_node_count(else_graph)),
            FxNode::Stack { children } => children
                .iter()
                .map(graph_node_count)
                .fold(0_usize, usize::saturating_add),
            _ => 0,
        };
        count.saturating_add(1).saturating_add(children)
    })
}

#[cfg(test)]
mod tests {
    use arcweft_presentation::fx::{FxDefinition, FxGraph, FxId, FxNode};

    use super::{FxDefinitions, FxDefinitionsError};

    fn definition(name: &str) -> FxDefinition {
        FxDefinition::new(
            FxId::try_new("test", name).expect("valid Fx identity"),
            Vec::new(),
            FxGraph::new(vec![FxNode::Text {
                properties: Vec::new(),
            }]),
        )
        .expect("valid definition")
    }

    #[test]
    fn canonical_section_sorts_round_trips_and_rejects_tampering() {
        let inventory = FxDefinitions::try_new([definition("zeta"), definition("alpha")])
            .expect("valid inventory");
        assert_eq!(inventory.definitions()[0].id().function(), "alpha");
        let bytes = inventory
            .encode_canonical_section()
            .expect("inventory encodes");
        assert_eq!(
            FxDefinitions::decode_canonical_section(&bytes).expect("inventory decodes"),
            inventory
        );

        let mut tampered = bytes;
        *tampered.last_mut().expect("payload byte") ^= 1;
        assert_eq!(
            FxDefinitions::decode_canonical_section(&tampered),
            Err(FxDefinitionsError::DigestMismatch)
        );
    }

    #[test]
    fn duplicate_identity_is_rejected() {
        assert!(matches!(
            FxDefinitions::try_new([definition("same"), definition("same")]),
            Err(FxDefinitionsError::DuplicateDefinition { .. })
        ));
    }
}
