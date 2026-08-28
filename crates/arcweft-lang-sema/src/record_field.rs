//! Shared checked identity of one accepted record field.

use crate::{
    env::nominal::AcceptedEnvironmentFieldSemanticId,
    types::{AcceptedVariantPayloadFieldSemanticId, SemanticTypeDigest},
};

const PROJECT_RECORD_FIELD_DOMAIN: &[u8] = b"arcweft.lang.accepted-record-field.v1\0";

/// Canonical semantic identity of one field in an accepted project record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedRecordFieldSemanticId([u8; 32]);

impl AcceptedRecordFieldSemanticId {
    pub(crate) fn issue(
        owner: SemanticTypeDigest,
        declaration_ordinal: u32,
        field_type: SemanticTypeDigest,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(PROJECT_RECORD_FIELD_DOMAIN);
        hasher.update(owner.as_bytes());
        hasher.update(&declaration_ordinal.to_le_bytes());
        hasher.update(field_type.as_bytes());
        Self(hasher.finalize().into())
    }

    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Shared semantic identity used by project and accepted-environment records.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CheckedRecordFieldSemanticId {
    Project(AcceptedRecordFieldSemanticId),
    Environment(AcceptedEnvironmentFieldSemanticId),
    VariantPayload(AcceptedVariantPayloadFieldSemanticId),
}

impl CheckedRecordFieldSemanticId {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        match self {
            Self::Project(id) => id.as_bytes(),
            Self::Environment(id) => id.as_bytes(),
            Self::VariantPayload(id) => id.as_bytes(),
        }
    }
}
