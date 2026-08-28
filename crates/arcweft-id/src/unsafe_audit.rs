//! Canonical identity for authored unsafe-lifetime audit records.

use thiserror::Error;

use crate::{IdError, PublicId};

const UNSAFE_AUDIT_PREFIX: &str = "unsafe.";
const UNSAFE_AUDIT_SEMANTIC_DOMAIN: &[u8] = b"arcweft.id.accepted-unsafe-audit-semantic.v1\0";

/// Canonical public identity of one unsafe-lifetime audit record.
///
/// Source reference markers are not part of this value. HIR/sema must resolve
/// and admit an absolute reference before constructing the identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnsafeAuditId(PublicId);

/// Stable semantic identity of one accepted [`UnsafeAuditId`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedUnsafeAuditSemanticId([u8; 32]);

impl AcceptedUnsafeAuditSemanticId {
    /// Returns the exact version-one digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Failure to construct the closed unsafe-audit identity family.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum UnsafeAuditIdError {
    #[error(transparent)]
    InvalidPublicId(#[from] IdError),
    #[error("unsafe audit identity must start with `unsafe.` and contain a nonempty tail")]
    WrongFamily,
}

impl UnsafeAuditId {
    /// Validates one canonical public identity in the `unsafe.*` family.
    pub fn try_new(value: impl Into<String>) -> Result<Self, UnsafeAuditIdError> {
        Self::try_from_public_id(PublicId::try_new(value)?)
    }

    /// Consumes an already-validated public identity and checks its family.
    pub fn try_from_public_id(value: PublicId) -> Result<Self, UnsafeAuditIdError> {
        value
            .as_str()
            .strip_prefix(UNSAFE_AUDIT_PREFIX)
            .filter(|tail| !tail.is_empty())
            .ok_or(UnsafeAuditIdError::WrongFamily)?;
        Ok(Self(value))
    }

    /// Returns the canonical public identity without a source reference marker.
    #[must_use]
    pub const fn as_public_id(&self) -> &PublicId {
        &self.0
    }

    /// Issues the semantic identity of this accepted unsafe-audit ID.
    #[must_use]
    pub fn semantic_id(&self) -> AcceptedUnsafeAuditSemanticId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(UNSAFE_AUDIT_SEMANTIC_DOMAIN);
        hasher.update(self.0.as_str().as_bytes());
        AcceptedUnsafeAuditSemanticId(*hasher.finalize().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::{UnsafeAuditId, UnsafeAuditIdError};
    use crate::PublicId;

    #[test]
    fn unsafe_audit_identity_accepts_only_its_closed_family() {
        let accepted = UnsafeAuditId::try_new("unsafe.borrow.promote").unwrap();
        assert_eq!(accepted.as_public_id().as_str(), "unsafe.borrow.promote");

        assert_eq!(
            UnsafeAuditId::try_new("proof.borrow.promote"),
            Err(UnsafeAuditIdError::WrongFamily)
        );
        assert_eq!(
            UnsafeAuditId::try_from_public_id(PublicId::try_new("unsafe.").unwrap()),
            Err(UnsafeAuditIdError::WrongFamily)
        );
        assert!(UnsafeAuditId::try_new("@unsafe.borrow.promote").is_err());
    }

    #[test]
    fn semantic_identity_is_owner_issued_and_value_sensitive() {
        let first = UnsafeAuditId::try_new("unsafe.borrow.promote").unwrap();
        let same = UnsafeAuditId::try_from_public_id(first.as_public_id().clone()).unwrap();
        let second = UnsafeAuditId::try_new("unsafe.borrow.escape").unwrap();

        assert_eq!(first.semantic_id(), same.semantic_id());
        assert_ne!(first.semantic_id(), second.semantic_id());
        assert_eq!(first.semantic_id().as_bytes().len(), 32);
    }
}
