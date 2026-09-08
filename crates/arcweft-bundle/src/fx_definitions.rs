//! Deterministic first-class bundle inventory for executable Fx definitions.

use std::collections::BTreeSet;

use arcweft_id::canonical::{
    CanonicalVarintDecodeError, append_canonical_varint, canonical_varint_len,
    decode_canonical_varint,
};
use arcweft_presentation::fx::{
    FX_MAX_DEFINITIONS_PER_SECTION, FX_MAX_TOTAL_GRAPH_NODES_PER_SECTION, FxDefinition,
    FxDefinitionDecodeError, FxDefinitionError,
};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

const MAGIC: [u8; 8] = *b"AWFXDEF\0";
const CODEC_VERSION: u8 = 1;
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
    #[error(
        "encoded Fx definitions section has {actual} definitions, exceeding the limit of {limit}"
    )]
    EncodedDefinitionCountTooLarge { actual: u64, limit: u64 },
    #[error("Fx definitions section has {actual} graph nodes, exceeding the limit of {limit}")]
    TooManyGraphNodes { actual: usize, limit: usize },
    #[error("Fx definitions section graph-node count overflowed the host width")]
    GraphNodeCountOverflow,
    #[error("Fx definitions section repeats identity `{id}`")]
    DuplicateDefinition { id: String },
    #[error("Fx definitions section exceeds the byte limit of {limit}")]
    SectionTooLarge { limit: usize },
    #[error("Fx definitions section header is truncated")]
    TruncatedHeader,
    #[error("Fx definitions section has invalid magic")]
    InvalidMagic,
    #[error("unsupported Fx definitions codec version {actual}")]
    UnsupportedVersion { actual: u8 },
    #[error("Fx definitions section length does not match its header")]
    LengthMismatch,
    #[error("Fx definitions section digest mismatch")]
    DigestMismatch,
    #[error("Fx definitions section is not in strictly increasing FxId order")]
    NonCanonicalOrder,
    #[error("Fx definitions section has trailing payload bytes")]
    TrailingPayload,
    #[error("Fx definitions section allocation failed")]
    AllocationFailed,
    #[error(transparent)]
    Varint(#[from] CanonicalVarintDecodeError),
    #[error(transparent)]
    DefinitionEncode(#[from] FxDefinitionError),
    #[error(transparent)]
    DefinitionDecode(#[from] FxDefinitionDecodeError),
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
        let node_count = definitions.iter().try_fold(0_usize, |count, definition| {
            count
                .checked_add(definition.graph().total_node_count())
                .ok_or(FxDefinitionsError::GraphNodeCountOverflow)
        })?;
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

    /// Encodes canonical framing around presentation-owned definition bytes.
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, FxDefinitionsError> {
        let payload_len = self.0.iter().try_fold(0_usize, |length, definition| {
            let definition_len = usize::try_from(definition.canonical_v1_len())
                .map_err(|_| FxDefinitionsError::LengthMismatch)?;
            length
                .checked_add(canonical_varint_len(u64::from(
                    definition.canonical_v1_len(),
                )))
                .and_then(|value| value.checked_add(definition_len))
                .ok_or(FxDefinitionsError::LengthMismatch)
        })?;
        let definition_count =
            u64::try_from(self.0.len()).map_err(|_| FxDefinitionsError::TooManyDefinitions {
                actual: self.0.len(),
                limit: FX_MAX_DEFINITIONS_PER_SECTION,
            })?;
        let payload_len_u64 =
            u64::try_from(payload_len).map_err(|_| FxDefinitionsError::LengthMismatch)?;
        let header_len = MAGIC
            .len()
            .checked_add(1)
            .and_then(|value| value.checked_add(canonical_varint_len(definition_count)))
            .and_then(|value| value.checked_add(canonical_varint_len(payload_len_u64)))
            .and_then(|value| value.checked_add(32))
            .ok_or(FxDefinitionsError::LengthMismatch)?;
        let total_len = header_len
            .checked_add(payload_len)
            .ok_or(FxDefinitionsError::LengthMismatch)?;
        if total_len > MAX_SECTION_BYTES {
            return Err(FxDefinitionsError::SectionTooLarge {
                limit: MAX_SECTION_BYTES,
            });
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total_len)
            .map_err(|_| FxDefinitionsError::AllocationFailed)?;
        bytes.extend_from_slice(&MAGIC);
        bytes.push(CODEC_VERSION);
        append_canonical_varint(&mut bytes, definition_count);
        append_canonical_varint(&mut bytes, payload_len_u64);
        let digest_offset = bytes.len();
        bytes.extend_from_slice(&[0; 32]);
        let payload_offset = bytes.len();
        for definition in &self.0 {
            append_canonical_varint(&mut bytes, u64::from(definition.canonical_v1_len()));
            definition.append_canonical_v1_bytes(&mut bytes)?;
        }
        if bytes.len() != total_len {
            return Err(FxDefinitionsError::LengthMismatch);
        }
        let digest = blake3::hash(&bytes[payload_offset..]);
        bytes[digest_offset..payload_offset].copy_from_slice(digest.as_bytes());
        Ok(bytes)
    }

    /// Decodes and revalidates every definition, stored hash, inventory limit, and digest.
    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, FxDefinitionsError> {
        if bytes.len() > MAX_SECTION_BYTES {
            return Err(FxDefinitionsError::SectionTooLarge {
                limit: MAX_SECTION_BYTES,
            });
        }
        if bytes.len() < MAGIC.len() + 1 + 1 + 1 + 32 {
            return Err(FxDefinitionsError::TruncatedHeader);
        }
        if bytes[..8] != MAGIC {
            return Err(FxDefinitionsError::InvalidMagic);
        }
        let version = bytes[8];
        if version != CODEC_VERSION {
            return Err(FxDefinitionsError::UnsupportedVersion { actual: version });
        }
        let mut cursor = 9;
        let expected_count = read_varint(bytes, &mut cursor)?;
        if expected_count
            > u64::try_from(FX_MAX_DEFINITIONS_PER_SECTION)
                .map_err(|_| FxDefinitionsError::LengthMismatch)?
        {
            return Err(FxDefinitionsError::EncodedDefinitionCountTooLarge {
                actual: expected_count,
                limit: u64::try_from(FX_MAX_DEFINITIONS_PER_SECTION)
                    .map_err(|_| FxDefinitionsError::LengthMismatch)?,
            });
        }
        let payload_len = read_varint(bytes, &mut cursor)?;
        let payload_len =
            usize::try_from(payload_len).map_err(|_| FxDefinitionsError::LengthMismatch)?;
        let digest_end = cursor
            .checked_add(32)
            .ok_or(FxDefinitionsError::LengthMismatch)?;
        let stored_digest = bytes
            .get(cursor..digest_end)
            .ok_or(FxDefinitionsError::TruncatedHeader)?;
        cursor = digest_end;
        if cursor.checked_add(payload_len) != Some(bytes.len()) {
            return Err(FxDefinitionsError::LengthMismatch);
        }
        let payload = &bytes[cursor..];
        if blake3::hash(payload).as_bytes() != stored_digest {
            return Err(FxDefinitionsError::DigestMismatch);
        }
        let expected_count =
            usize::try_from(expected_count).map_err(|_| FxDefinitionsError::LengthMismatch)?;
        let mut definitions = Vec::new();
        definitions
            .try_reserve_exact(expected_count)
            .map_err(|_| FxDefinitionsError::AllocationFailed)?;
        let mut payload_cursor = 0;
        for _ in 0..expected_count {
            let definition_len = read_varint(payload, &mut payload_cursor)?;
            let definition_len =
                usize::try_from(definition_len).map_err(|_| FxDefinitionsError::LengthMismatch)?;
            let end = payload_cursor
                .checked_add(definition_len)
                .ok_or(FxDefinitionsError::LengthMismatch)?;
            let definition_bytes = payload
                .get(payload_cursor..end)
                .ok_or(FxDefinitionsError::LengthMismatch)?;
            payload_cursor = end;
            let definition = FxDefinition::decode_canonical_v1(definition_bytes)?;
            if definitions
                .last()
                .is_some_and(|prior: &FxDefinition| prior.id() >= definition.id())
            {
                return Err(FxDefinitionsError::NonCanonicalOrder);
            }
            definitions.push(definition);
        }
        if payload_cursor != payload.len() {
            return Err(FxDefinitionsError::TrailingPayload);
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

fn read_varint(bytes: &[u8], cursor: &mut usize) -> Result<u64, FxDefinitionsError> {
    let remaining = bytes
        .get(*cursor..)
        .ok_or(FxDefinitionsError::LengthMismatch)?;
    let (value, consumed) = decode_canonical_varint(remaining)?;
    *cursor = cursor
        .checked_add(consumed)
        .ok_or(FxDefinitionsError::LengthMismatch)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use arcweft_id::canonical::CanonicalVarintDecodeError;
    use arcweft_presentation::fx::{FxDefinition, FxDefinitionDecodeError, FxGraph, FxId, FxNode};

    use super::{FxDefinitions, FxDefinitionsError, read_varint};

    fn definition(name: &str) -> FxDefinition {
        FxDefinition::new(
            FxId::try_new("test", name).expect("valid Fx identity"),
            Vec::new(),
            FxGraph::try_new(vec![FxNode::Text {
                properties: Vec::new(),
            }])
            .expect("valid graph"),
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
        let decoded = FxDefinitions::decode_canonical_section(&bytes).expect("inventory decodes");
        assert_eq!(decoded, inventory);
        assert_eq!(
            decoded
                .encode_canonical_section()
                .expect("decoded inventory re-encodes"),
            bytes,
            "canonical section bytes must be stable after typed decode"
        );

        let mut tampered = bytes;
        *tampered.last_mut().expect("payload byte") ^= 1;
        assert_eq!(
            FxDefinitions::decode_canonical_section(&tampered),
            Err(FxDefinitionsError::DigestMismatch)
        );
    }

    #[test]
    fn canonical_section_rejects_noncanonical_framing() {
        let bytes = FxDefinitions::try_new([definition("alpha")])
            .expect("valid inventory")
            .encode_canonical_section()
            .expect("inventory encodes");

        let mut wrong_version = bytes.clone();
        wrong_version[8] = 2;
        assert_eq!(
            FxDefinitions::decode_canonical_section(&wrong_version),
            Err(FxDefinitionsError::UnsupportedVersion { actual: 2 })
        );

        let mut overlong_count = bytes;
        overlong_count.splice(9..10, [0x81, 0x00]);
        assert_eq!(
            FxDefinitions::decode_canonical_section(&overlong_count),
            Err(FxDefinitionsError::Varint(
                CanonicalVarintDecodeError::NonCanonical
            ))
        );
    }

    #[test]
    fn canonical_section_revalidates_definition_and_inventory_order() {
        let inventory = FxDefinitions::try_new([definition("zeta"), definition("alpha")])
            .expect("valid inventory");
        let bytes = inventory
            .encode_canonical_section()
            .expect("inventory encodes");
        let (digest_offset, payload_offset) = section_offsets(&bytes);

        let mut semantic_tamper = bytes.clone();
        *semantic_tamper.last_mut().expect("semantic hash byte") ^= 1;
        refresh_payload_digest(&mut semantic_tamper, digest_offset, payload_offset);
        assert_eq!(
            FxDefinitions::decode_canonical_section(&semantic_tamper),
            Err(FxDefinitionsError::DefinitionDecode(
                FxDefinitionDecodeError::SemanticHashMismatch
            ))
        );

        let payload = &bytes[payload_offset..];
        let mut cursor = 0;
        let first_len = usize::try_from(read_varint(payload, &mut cursor).expect("first length"))
            .expect("test definition length fits usize");
        let first_end = cursor + first_len;
        let first_frame = &payload[..first_end];
        let second_frame = &payload[first_end..];
        let mut wrong_order = bytes.clone();
        wrong_order.truncate(payload_offset);
        wrong_order.extend_from_slice(second_frame);
        wrong_order.extend_from_slice(first_frame);
        refresh_payload_digest(&mut wrong_order, digest_offset, payload_offset);
        assert_eq!(
            FxDefinitions::decode_canonical_section(&wrong_order),
            Err(FxDefinitionsError::NonCanonicalOrder)
        );

        let mut hidden_definition = bytes;
        hidden_definition[9] = 1;
        assert_eq!(
            FxDefinitions::decode_canonical_section(&hidden_definition),
            Err(FxDefinitionsError::TrailingPayload)
        );
    }

    #[test]
    fn duplicate_identity_is_rejected() {
        assert!(matches!(
            FxDefinitions::try_new([definition("same"), definition("same")]),
            Err(FxDefinitionsError::DuplicateDefinition { .. })
        ));
    }

    fn section_offsets(bytes: &[u8]) -> (usize, usize) {
        let mut cursor = 9;
        read_varint(bytes, &mut cursor).expect("definition count");
        read_varint(bytes, &mut cursor).expect("payload length");
        (cursor, cursor + 32)
    }

    fn refresh_payload_digest(bytes: &mut [u8], digest_offset: usize, payload_offset: usize) {
        let digest = blake3::hash(&bytes[payload_offset..]);
        bytes[digest_offset..payload_offset].copy_from_slice(digest.as_bytes());
    }
}
