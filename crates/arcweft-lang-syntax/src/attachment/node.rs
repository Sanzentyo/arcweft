//! Snapshot-bound typed grammar handles and syntax-owned marker kinds.

use core::marker::PhantomData;

use arcweft_source::SourceRange;

use super::{SyntaxLookupError, SyntaxNodeHandle, SyntaxNodeId, SyntaxSnapshotId};
use crate::grammar::kinds::{AstTag, SyntaxKind};

/// Exact grammar-kind marker owned by the syntax crate.
pub(crate) trait AstKind: Copy + 'static {
    const KIND: SyntaxKind;
    const TAG: AstTag;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFileKind;

impl AstKind for SourceFileKind {
    const KIND: SyntaxKind = SyntaxKind::SourceFile;
    const TAG: AstTag = AstTag::SourceFile;
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PredicateItemKind;

#[cfg(test)]
impl AstKind for PredicateItemKind {
    const KIND: SyntaxKind = SyntaxKind::PredicateItem;
    const TAG: AstTag = AstTag::Item;
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProofItemKind;

#[cfg(test)]
impl AstKind for ProofItemKind {
    const KIND: SyntaxKind = SyntaxKind::ProofItem;
    const TAG: AstTag = AstTag::Item;
}

/// Typed handle that cannot detach from its immutable grammar snapshot.
pub(crate) struct AstNode<K: AstKind> {
    syntax: SyntaxNodeHandle,
    marker: PhantomData<fn() -> K>,
}

impl<K: AstKind> Clone for AstNode<K> {
    fn clone(&self) -> Self {
        Self {
            syntax: self.syntax.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: AstKind> core::fmt::Debug for AstNode<K> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AstNode")
            .field("kind", &K::KIND)
            .field("tag", &K::TAG)
            .field("id", &self.id())
            .field("snapshot", self.snapshot_id())
            .finish()
    }
}

impl<K: AstKind> PartialEq for AstNode<K> {
    fn eq(&self, other: &Self) -> bool {
        self.syntax == other.syntax
    }
}

impl<K: AstKind> Eq for AstNode<K> {}

impl<K: AstKind> AstNode<K> {
    pub(super) fn new(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxLookupError> {
        if syntax.kind() != K::KIND {
            return Err(SyntaxLookupError::KindMismatch {
                id: syntax.id(),
                expected: K::KIND,
                actual: syntax.kind(),
            });
        }
        if syntax.tag() != K::TAG {
            return Err(SyntaxLookupError::AstTagMismatch {
                id: syntax.id(),
                expected: K::TAG,
                actual: syntax.tag(),
            });
        }
        Ok(Self {
            syntax,
            marker: PhantomData,
        })
    }

    pub(crate) fn id(&self) -> SyntaxNodeId {
        self.syntax.id()
    }

    pub(crate) fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.syntax.snapshot_id()
    }

    pub(crate) fn syntax(&self) -> SyntaxNodeHandle {
        self.syntax.clone()
    }

    pub(crate) fn range(&self) -> SourceRange {
        self.syntax.range()
    }

    pub(crate) fn is_same_reconciled_node(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
