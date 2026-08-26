//! Resolved annotation authority retained by typed-binding patterns.

use crate::types::{SemanticTypeDigest, TypeKind};

/// Exact resolved annotation retained by a typed-binding pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTypedBinding {
    annotation: TypeKind,
    annotation_digest: SemanticTypeDigest,
}

impl CheckedTypedBinding {
    pub(in crate::final_analysis) fn new(annotation: TypeKind) -> Self {
        let annotation_digest = annotation.semantic_identity_digest();
        Self {
            annotation,
            annotation_digest,
        }
    }

    /// Returns the exact resolved annotation type.
    pub const fn annotation(&self) -> &TypeKind {
        &self.annotation
    }

    /// Returns the stable semantic identity of the resolved annotation.
    pub const fn annotation_digest(&self) -> SemanticTypeDigest {
        self.annotation_digest
    }

    pub(crate) fn has_valid_semantic_identity(&self) -> bool {
        self.annotation.semantic_identity_digest() == self.annotation_digest
    }
}

#[cfg(test)]
#[path = "typed_binding_tests.rs"]
mod tests;
