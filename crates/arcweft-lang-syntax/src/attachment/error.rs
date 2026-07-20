//! Typed failures for private grammar attachment and snapshot lookup.

use thiserror::Error;

use super::{SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotId};
use crate::grammar::build::GrammarEventPath;
use crate::grammar::kinds::{AstTag, SyntaxKind};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum AttachmentFailure {
    #[error("the grammar index has no source-file root")]
    MissingRoot,
    #[error("the grammar identity map has no identity for event path {path:?}")]
    MissingIdentity { path: GrammarEventPath },
    #[error("grammar identity {id:?} has no attachable Rowan node")]
    MissingAttachment { id: SyntaxNodeId },
    #[error("grammar identity {id:?} is attached more than once")]
    DuplicateAttachment { id: SyntaxNodeId },
    #[error(
        "grammar identity {id:?} expected {expected:?}, but Rowan retained raw kind {actual:?}"
    )]
    GrammarKindMismatch {
        id: SyntaxNodeId,
        expected: SyntaxKind,
        actual: rowan::SyntaxKind,
    },
    #[error("identity-bearing grammar node {id:?} with kind {kind:?} has no typed AST tag")]
    MissingAstTag { id: SyntaxNodeId, kind: SyntaxKind },
    #[error("grammar identity map has {actual} entries, expected {expected}")]
    IdentityMapMismatch { expected: usize, actual: usize },
    #[error("immutable grammar attachment failed its bidirectional snapshot invariant")]
    SnapshotInvariant,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum SyntaxLookupError {
    #[error("syntax handle belongs to another syntax database")]
    WrongDatabase {
        expected: SyntaxDatabaseId,
        actual: SyntaxDatabaseId,
    },
    #[error("syntax handle belongs to another source lineage")]
    WrongLineage {
        expected: SyntaxLineageId,
        actual: SyntaxLineageId,
    },
    #[error("syntax handle belongs to another immutable snapshot")]
    WrongSnapshot {
        expected: SyntaxSnapshotId,
        actual: SyntaxSnapshotId,
    },
    #[error("Rowan node is not owned by this immutable syntax root")]
    ForeignRowanRoot { expected: SyntaxSnapshotId },
    #[error("syntax identity {id:?} has no node in this snapshot")]
    MissingNode { id: SyntaxNodeId },
    #[error("syntax identity {id:?} has kind {actual:?}, expected {expected:?}")]
    KindMismatch {
        id: SyntaxNodeId,
        expected: SyntaxKind,
        actual: SyntaxKind,
    },
    #[error("syntax identity {id:?} has AST tag {actual:?}, expected {expected:?}")]
    AstTagMismatch {
        id: SyntaxNodeId,
        expected: AstTag,
        actual: AstTag,
    },
}
