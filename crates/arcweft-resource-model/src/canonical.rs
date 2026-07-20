//! Canonical retained-reference transcripts independent of manifest syntax.

use crate::{
    retained::{PresentationTargetScope, ResolvedRetainedIdentityRef},
    value::{ResourceConstValue, ResourceValueType},
};
use arcweft_manifest_model::RawDigest;
use thiserror::Error;

const VALUE_TYPE_PREFIX: &[u8] = b"arcweft.resource.value-type.v1\0";
const CONST_VALUE_PREFIX: &[u8] = b"arcweft.resource.const-value.v1\0";
const FORMAT_VERSION: u32 = 1;
const RETAINED_IDENTITY_TAG: &str = "retained_identity_ref";

/// Failure to encode a value outside the currently frozen retained-reference
/// transcript or a string whose byte length cannot fit the wire.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceCanonicalEncodingError {
    #[error("canonical v1 encoding is not frozen for this resource value type")]
    UnsupportedValueType,
    #[error("canonical v1 encoding is not frozen for this resource constant kind")]
    UnsupportedConstValue,
    #[error("canonical string length {length} exceeds u32::MAX")]
    StringLengthOverflow { length: usize },
}

/// Deterministic little-endian encoder for the frozen resource transcripts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalEncoder {
    bytes: Vec<u8>,
}

impl CanonicalEncoder {
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn u32(&mut self, value: u32) {
        self.raw(&value.to_le_bytes());
    }

    fn string(&mut self, value: &str) -> Result<(), ResourceCanonicalEncodingError> {
        let length = u32::try_from(value.len()).map_err(|_| {
            ResourceCanonicalEncodingError::StringLengthOverflow {
                length: value.len(),
            }
        })?;
        self.u32(length);
        self.raw(value.as_bytes());
        Ok(())
    }
}

impl ResourceValueType {
    /// Encodes the frozen standalone v1 transcript for a retained reference.
    pub fn encode_canonical_v1(
        &self,
        encoder: &mut CanonicalEncoder,
    ) -> Result<(), ResourceCanonicalEncodingError> {
        let Self::RetainedIdentityRef { identity } = self else {
            return Err(ResourceCanonicalEncodingError::UnsupportedValueType);
        };
        encoder.raw(VALUE_TYPE_PREFIX);
        encoder.u32(FORMAT_VERSION);
        encoder.string(RETAINED_IDENTITY_TAG)?;
        encoder.string(identity.as_str())
    }

    pub fn canonical_bytes_v1(&self) -> Result<Vec<u8>, ResourceCanonicalEncodingError> {
        let mut encoder = CanonicalEncoder::default();
        self.encode_canonical_v1(&mut encoder)?;
        Ok(encoder.into_bytes())
    }

    pub fn canonical_digest_v1(&self) -> Result<RawDigest, ResourceCanonicalEncodingError> {
        self.canonical_bytes_v1()
            .map(|bytes| RawDigest::for_bytes(&bytes))
    }
}

impl ResourceConstValue {
    /// Encodes the frozen standalone v1 transcript for a retained reference.
    pub fn encode_canonical_v1(
        &self,
        encoder: &mut CanonicalEncoder,
    ) -> Result<(), ResourceCanonicalEncodingError> {
        let Self::RetainedIdentityRef { value } = self else {
            return Err(ResourceCanonicalEncodingError::UnsupportedConstValue);
        };
        encoder.raw(CONST_VALUE_PREFIX);
        encoder.u32(FORMAT_VERSION);
        encoder.string(RETAINED_IDENTITY_TAG)?;
        encode_resolved_retained_identity(encoder, value)
    }

    pub fn canonical_bytes_v1(&self) -> Result<Vec<u8>, ResourceCanonicalEncodingError> {
        let mut encoder = CanonicalEncoder::default();
        self.encode_canonical_v1(&mut encoder)?;
        Ok(encoder.into_bytes())
    }

    pub fn canonical_digest_v1(&self) -> Result<RawDigest, ResourceCanonicalEncodingError> {
        self.canonical_bytes_v1()
            .map(|bytes| RawDigest::for_bytes(&bytes))
    }
}

fn encode_resolved_retained_identity(
    encoder: &mut CanonicalEncoder,
    value: &ResolvedRetainedIdentityRef,
) -> Result<(), ResourceCanonicalEncodingError> {
    encoder.string(value.kind().as_str())?;
    match value {
        ResolvedRetainedIdentityRef::Character { entity_id }
        | ResolvedRetainedIdentityRef::View { entity_id }
        | ResolvedRetainedIdentityRef::Action { entity_id }
        | ResolvedRetainedIdentityRef::Layer { entity_id }
        | ResolvedRetainedIdentityRef::Signal { entity_id } => encoder.string(entity_id.as_str()),
        ResolvedRetainedIdentityRef::PresentationTarget { scope, target_id } => {
            match scope {
                PresentationTargetScope::Global => encoder.string("global")?,
                PresentationTargetScope::View {
                    owner_view_entity_id,
                } => {
                    encoder.string("view")?;
                    encoder.string(owner_view_entity_id.as_str())?;
                }
            }
            encoder.string(target_id.as_str())
        }
        ResolvedRetainedIdentityRef::ScrollRegion {
            owner_view_entity_id,
            region_id,
        } => {
            encoder.string(owner_view_entity_id.as_str())?;
            encoder.string(region_id.as_str())
        }
    }
}
