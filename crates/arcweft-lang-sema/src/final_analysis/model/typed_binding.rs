//! Resolved annotation authority retained by typed-binding patterns.

use crate::types::{SemanticTypeDigest, TypeKind};

/// Exact resolved annotation retained by a typed-binding pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTypedBinding {
    annotation: TypeKind,
    annotation_digest: SemanticTypeDigest,
    choice_alternatives: Box<[u32]>,
}

impl CheckedTypedBinding {
    pub(in crate::final_analysis) fn try_new(
        annotation: TypeKind,
        scrutinee: &TypeKind,
    ) -> Option<Self> {
        let annotation_digest = annotation.semantic_identity_digest();
        let choice_alternatives = match scrutinee {
            TypeKind::Choice(alternatives) => alternatives
                .iter()
                .enumerate()
                .filter(|(_, alternative)| alternative.accepts(&annotation))
                .map(|(ordinal, _)| u32::try_from(ordinal).ok())
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
            _ => Box::new([]),
        };
        Some(Self {
            annotation,
            annotation_digest,
            choice_alternatives,
        })
    }

    /// Returns the exact resolved annotation type.
    pub const fn annotation(&self) -> &TypeKind {
        &self.annotation
    }

    /// Returns the stable semantic identity of the resolved annotation.
    pub const fn annotation_digest(&self) -> SemanticTypeDigest {
        self.annotation_digest
    }

    /// Exact source-order Choice alternatives selected by type checking.
    pub fn choice_alternatives(&self) -> &[u32] {
        &self.choice_alternatives
    }

    pub(crate) fn has_valid_semantic_identity(&self) -> bool {
        self.annotation.semantic_identity_digest() == self.annotation_digest
            && self
                .choice_alternatives
                .windows(2)
                .all(|window| window[0] < window[1])
    }
}

#[cfg(test)]
#[path = "typed_binding_tests.rs"]
mod tests;
