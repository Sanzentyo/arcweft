//! Canonical, allocation-budgeted AWBC binary codec.

mod code;
mod metadata;
mod runtime;
mod types;
mod wire;

use super::schema::{AWBC_CODEC_VERSION, AWBC_MAGIC, AwbcProgram};
use thiserror::Error;
use wire::{Reader, Wire, Writer};

const ENVELOPE_BYTES: usize = 20;

/// Decode limits applied before every allocation and nested collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AwbcDecodeBudget {
    pub encoded_bytes: usize,
    pub strings: usize,
    pub string_bytes: usize,
    pub runtime_types: usize,
    pub constants: usize,
    pub effect_sets: usize,
    pub signatures: usize,
    pub frame_layouts: usize,
    pub functions: usize,
    pub blocks: usize,
    pub instructions: usize,
    pub resume_points: usize,
    pub patterns: usize,
    pub match_arms: usize,
    pub intrinsics: usize,
    pub host_calls: usize,
    pub task_plans: usize,
    pub audio_commands: usize,
    pub effect_plans: usize,
    pub choices: usize,
    pub choice_options: usize,
    pub content_units: usize,
    pub line_task_groups: usize,
    pub line_task_nodes: usize,
    pub stream_plans: usize,
    pub source_plans: usize,
    pub pure_helpers: usize,
    pub trait_methods: usize,
    pub display_map: usize,
    pub source_map: usize,
    pub resources: usize,
    pub entries: usize,
    pub collection_items: usize,
    pub tensor_elements: usize,
    pub nesting_depth: usize,
}

impl Default for AwbcDecodeBudget {
    fn default() -> Self {
        Self {
            encoded_bytes: 256 * 1024 * 1024,
            strings: 1_000_000,
            string_bytes: 64 * 1024 * 1024,
            runtime_types: 262_144,
            constants: 1_000_000,
            effect_sets: 262_144,
            signatures: 262_144,
            frame_layouts: 262_144,
            functions: 262_144,
            blocks: 1_000_000,
            instructions: 8_000_000,
            resume_points: 2_000_000,
            patterns: 1_000_000,
            match_arms: 2_000_000,
            intrinsics: 262_144,
            host_calls: 262_144,
            task_plans: 262_144,
            audio_commands: 262_144,
            effect_plans: 1_000_000,
            choices: 262_144,
            choice_options: 1_000_000,
            content_units: 1_000_000,
            line_task_groups: 1_000_000,
            line_task_nodes: 4_000_000,
            stream_plans: 262_144,
            source_plans: 262_144,
            pure_helpers: 262_144,
            trait_methods: 262_144,
            display_map: 2_000_000,
            source_map: 8_000_000,
            resources: 1_000_000,
            entries: 262_144,
            collection_items: 16_000_000,
            tensor_elements: 16_000_000,
            nesting_depth: 64,
        }
    }
}

/// Canonical codec failure with a stable byte offset when applicable.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AwbcCodecError {
    #[error("AWBC payload exceeds `{budget}` budget: {actual} > {limit}")]
    BudgetExceeded {
        budget: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("AWBC magic is invalid")]
    InvalidMagic,
    #[error("unsupported AWBC codec version {actual}; expected {expected}")]
    UnsupportedCodecVersion { actual: u16, expected: u16 },
    #[error("AWBC envelope reserved bits are non-zero")]
    NonZeroReservedBits,
    #[error("AWBC payload length {declared} does not match available {available}")]
    PayloadLengthMismatch { declared: u64, available: usize },
    #[error("AWBC is truncated at byte offset {offset}")]
    Truncated { offset: usize },
    #[error("AWBC contains a non-canonical varint at byte offset {offset}")]
    NonCanonicalVarint { offset: usize },
    #[error("AWBC integer length cannot be represented on this platform")]
    LengthOverflow,
    #[error("AWBC string is not valid UTF-8 at byte offset {offset}")]
    InvalidUtf8 { offset: usize },
    #[error("unknown AWBC {kind} tag {tag} at byte offset {offset}")]
    UnknownTag {
        kind: &'static str,
        tag: u8,
        offset: usize,
    },
    #[error("AWBC contains {count} trailing payload byte(s)")]
    TrailingBytes { count: usize },
    #[error("AWBC nesting depth exceeds {limit}")]
    NestingDepthExceeded { limit: usize },
}

impl AwbcProgram {
    /// Encodes this program into the unique AWBC v1 byte representation.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, AwbcCodecError> {
        let mut payload = Writer::default();
        self.write_wire(&mut payload)?;
        let payload = payload.finish();
        let payload_len =
            u64::try_from(payload.len()).map_err(|_| AwbcCodecError::LengthOverflow)?;
        let mut bytes = Vec::with_capacity(ENVELOPE_BYTES.saturating_add(payload.len()));
        bytes.extend_from_slice(&AWBC_MAGIC);
        bytes.extend_from_slice(&AWBC_CODEC_VERSION.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&payload_len.to_le_bytes());
        bytes.extend_from_slice(&payload);
        Ok(bytes)
    }

    /// Decodes canonical AWBC bytes while enforcing all allocation budgets.
    pub fn decode_canonical(
        bytes: &[u8],
        budget: AwbcDecodeBudget,
    ) -> Result<Self, AwbcCodecError> {
        if bytes.len() > budget.encoded_bytes {
            return Err(AwbcCodecError::BudgetExceeded {
                budget: "encoded_bytes",
                actual: bytes.len(),
                limit: budget.encoded_bytes,
            });
        }
        if bytes.len() < ENVELOPE_BYTES {
            return Err(AwbcCodecError::Truncated {
                offset: bytes.len(),
            });
        }
        if bytes[..8] != AWBC_MAGIC {
            return Err(AwbcCodecError::InvalidMagic);
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version != AWBC_CODEC_VERSION {
            return Err(AwbcCodecError::UnsupportedCodecVersion {
                actual: version,
                expected: AWBC_CODEC_VERSION,
            });
        }
        if u16::from_le_bytes([bytes[10], bytes[11]]) != 0 {
            return Err(AwbcCodecError::NonZeroReservedBits);
        }
        let mut declared_bytes = [0_u8; 8];
        declared_bytes.copy_from_slice(&bytes[12..20]);
        let declared = u64::from_le_bytes(declared_bytes);
        let available = bytes.len() - ENVELOPE_BYTES;
        if declared != available as u64 {
            return Err(AwbcCodecError::PayloadLengthMismatch {
                declared,
                available,
            });
        }
        let mut reader = Reader::new(&bytes[ENVELOPE_BYTES..], &budget);
        let program = Self::read_wire(&mut reader)?;
        reader.finish()?;
        Ok(program)
    }
}
