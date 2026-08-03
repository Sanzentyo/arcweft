//! Typed failures for grammar attachment and snapshot lookup.

use thiserror::Error;

use arcweft_source::identity::SourceGeneration;

use super::family::AstNodeFamily;
use super::{SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotId};
use crate::grammar::kinds::{AstTag, SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::patterns::{PatternNodeStep, PatternTypeChildRelation};
use crate::types::TypeRefNodeStep;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AttachmentFailure {
    #[error("the grammar index has no source-file root")]
    MissingRoot,
    #[error("the grammar identity map is missing an attachment identity")]
    MissingIdentity,
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
pub enum SyntaxLookupError {
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
    #[error("syntax lineage is not registered in this database")]
    UnknownLineage { lineage: SyntaxLineageId },
    #[error("syntax handle belongs to another immutable snapshot")]
    WrongSnapshot {
        expected: SyntaxSnapshotId,
        actual: SyntaxSnapshotId,
    },
    #[error("Rowan node is not owned by this immutable syntax root")]
    ForeignRowanRoot { expected: SyntaxSnapshotId },
    #[error("syntax generation is stale")]
    StaleGeneration {
        current: SourceGeneration,
        supplied: SourceGeneration,
    },
    #[error("syntax identity {id:?} has no node in this snapshot")]
    MissingNode { id: SyntaxNodeId },
    #[error("syntax identity {id:?} has kind {actual:?}, expected {expected:?}")]
    KindMismatch {
        id: SyntaxNodeId,
        expected: SyntaxKind,
        actual: SyntaxKind,
    },
    #[error(
        "syntax identity {id:?} has kind {actual:?}, which is outside the expected {expected:?} family"
    )]
    KindPredicateMismatch {
        id: SyntaxNodeId,
        expected: AstTag,
        actual: SyntaxKind,
    },
    #[error("syntax identity {id:?} has AST tag {actual:?}, expected {expected:?}")]
    AstTagMismatch {
        id: SyntaxNodeId,
        expected: AstTag,
        actual: AstTag,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SyntaxAccessError {
    #[error(transparent)]
    Lookup(#[from] SyntaxLookupError),
    #[error("syntax identity {parent:?} has no {expected:?} child at exact role {role:?}")]
    MissingExactChild {
        parent: SyntaxNodeId,
        role: SyntaxRole,
        expected: SyntaxKind,
    },
    #[error("syntax identity {parent:?} has no {expected:?} family child at exact role {role:?}")]
    MissingFamilyChild {
        parent: SyntaxNodeId,
        role: SyntaxRole,
        expected: AstNodeFamily,
    },
    #[error(
        "syntax identity {parent:?} has {count} children at exact role {role:?}; unique access requires at most one"
    )]
    AmbiguousChild {
        parent: SyntaxNodeId,
        role: SyntaxRole,
        count: usize,
    },
    #[error("role class {role:?} is not ordinal and cannot drive ordered child access")]
    NonOrdinalRoleClass { role: SyntaxRoleClass },
    #[error(
        "syntax identity {parent:?} has non-contiguous {role:?} children: expected ordinal {expected}, found {actual}"
    )]
    NonContiguousRole {
        parent: SyntaxNodeId,
        role: SyntaxRoleClass,
        expected: u32,
        actual: u32,
    },
    #[error(
        "syntax identity {id:?} has concrete kind {actual_kind:?} and tag {actual_tag:?}, which is not in {expected:?}"
    )]
    FamilyMismatch {
        id: SyntaxNodeId,
        expected: AstNodeFamily,
        actual_kind: SyntaxKind,
        actual_tag: AstTag,
    },
    #[error("syntax type identity {id:?} has no semantic type projection")]
    MissingTypeProjection { id: SyntaxNodeId },
    #[error("syntax type identity {id:?} carries an invalid semantic type projection")]
    InvalidTypeProjection { id: SyntaxNodeId },
    #[error("syntax expression identity {id:?} has no semantic expression projection")]
    MissingExpressionProjection { id: SyntaxNodeId },
    #[error("syntax expression identity {id:?} carries an invalid semantic expression projection")]
    InvalidExpressionProjection { id: SyntaxNodeId },
    #[error("syntax keyword-statement identity {id:?} has no parser-owned semantic projection")]
    MissingKeywordStatementProjection { id: SyntaxNodeId },
    #[error("syntax keyword-statement identity {id:?} carries an invalid semantic projection")]
    InvalidKeywordStatementProjection { id: SyntaxNodeId },
    #[error("syntax Entry identity {id:?} has no parser-owned semantic projection")]
    MissingEntryProjection { id: SyntaxNodeId },
    #[error("syntax Entry identity {id:?} carries an invalid semantic projection")]
    InvalidEntryProjection { id: SyntaxNodeId },
    #[error("syntax Source identity {id:?} has no parser-owned semantic projection")]
    MissingSourceDeclarationProjection { id: SyntaxNodeId },
    #[error("syntax Source identity {id:?} carries an invalid semantic projection")]
    InvalidSourceDeclarationProjection { id: SyntaxNodeId },
    #[error("syntax outer-attribute identity {id:?} has no semantic attribute projection")]
    MissingAttributeProjection { id: SyntaxNodeId },
    #[error(
        "syntax outer-attribute identity {id:?} carries an invalid semantic attribute projection"
    )]
    InvalidAttributeProjection { id: SyntaxNodeId },
    #[error("syntax Pattern identity {id:?} has no semantic Pattern projection")]
    MissingPatternProjection { id: SyntaxNodeId },
    #[error("syntax Pattern identity {id:?} carries an invalid semantic Pattern projection")]
    InvalidPatternProjection { id: SyntaxNodeId },
    #[error("syntax method receiver identity {id:?} has no parser-owned receiver projection")]
    MissingMethodReceiverProjection { id: SyntaxNodeId },
    #[error("syntax method receiver identity {id:?} carries an invalid receiver projection")]
    InvalidMethodReceiverProjection { id: SyntaxNodeId },
    #[error("semantic Pattern identity {parent:?} has no attached child projection for {step:?}")]
    MissingPatternChildProjection {
        parent: SyntaxNodeId,
        step: PatternNodeStep,
    },
    #[error(
        "semantic Pattern identity {parent:?} has no attached type child projection for {relation:?}"
    )]
    MissingPatternTypeChildProjection {
        parent: SyntaxNodeId,
        relation: PatternTypeChildRelation,
    },
    #[error("semantic type identity {parent:?} has no attached child projection for {step:?}")]
    MissingTypeChildProjection {
        parent: SyntaxNodeId,
        step: TypeRefNodeStep,
    },
    #[error("syntax Path identity {id:?} has no parser-owned semantic path projection")]
    MissingPathProjection { id: SyntaxNodeId },
    #[error("syntax Path identity {id:?} carries an invalid semantic path projection")]
    InvalidPathProjection { id: SyntaxNodeId },
    #[error("syntax UseDeclaration identity {id:?} has no parser-owned import-tree projection")]
    MissingUseProjection { id: SyntaxNodeId },
    #[error("syntax UseDeclaration identity {id:?} carries an invalid import-tree projection")]
    InvalidUseProjection { id: SyntaxNodeId },
    #[error("syntax Visibility identity {id:?} has no parser-owned visibility projection")]
    MissingVisibilityProjection { id: SyntaxNodeId },
    #[error("syntax Character identity {id:?} has no parser-owned declaration projection")]
    MissingCharacterProjection { id: SyntaxNodeId },
    #[error("syntax retained header identity {id:?} has no parser-owned header projection")]
    MissingRetainedHeaderProjection { id: SyntaxNodeId },
    #[error("syntax retained header identity {id:?} carries an invalid header projection")]
    InvalidRetainedHeaderProjection { id: SyntaxNodeId },
    #[error("syntax Character identity {id:?} carries an invalid declaration projection")]
    InvalidCharacterProjection { id: SyntaxNodeId },
    #[error("syntax Test identity {id:?} has no parser-owned adapter-kind projection")]
    MissingTestKindProjection { id: SyntaxNodeId },
    #[error("syntax Test identity {id:?} carries an invalid adapter-kind projection")]
    InvalidTestKindProjection { id: SyntaxNodeId },
    #[error("syntax Layer identity {id:?} has no parser-owned declaration projection")]
    MissingLayerProjection { id: SyntaxNodeId },
    #[error("syntax Layer identity {id:?} carries an invalid declaration projection")]
    InvalidLayerProjection { id: SyntaxNodeId },
    #[error("syntax View export identity {id:?} has no parser-owned structural projection")]
    MissingViewExportProjection { id: SyntaxNodeId },
    #[error("syntax View identity {id:?} carries an invalid declaration projection")]
    InvalidViewProjection { id: SyntaxNodeId },
    #[error("syntax Style identity {id:?} has no parser-owned declaration projection")]
    MissingStyleProjection { id: SyntaxNodeId },
    #[error("syntax Style identity {id:?} carries an invalid declaration projection")]
    InvalidStyleProjection { id: SyntaxNodeId },
    #[error("syntax Flow contract identity {id:?} has no parser-owned clause projection")]
    MissingFlowContractProjection { id: SyntaxNodeId },
    #[error("syntax Flow contract identity {id:?} carries an invalid clause projection")]
    InvalidFlowContractProjection { id: SyntaxNodeId },
    #[error("syntax Flow contract identity {id:?} has an invalid typed child shape")]
    InvalidFlowContractShape { id: SyntaxNodeId },
    #[error("syntax Flow identity {id:?} has no parser-owned declaration projection")]
    MissingFlowDeclarationProjection { id: SyntaxNodeId },
    #[error("syntax Flow identity {id:?} carries an invalid declaration projection")]
    InvalidFlowDeclarationProjection { id: SyntaxNodeId },
    #[error("syntax Flow identity {id:?} has an invalid typed declaration shape")]
    InvalidFlowDeclarationShape { id: SyntaxNodeId },
    #[error("syntax identity {id:?} is not an admitted Thread/Flow body item")]
    InvalidThreadFlowItemShape { id: SyntaxNodeId },
    #[error("syntax identity {id:?} has an invalid statement-only Thread/Flow body")]
    InvalidThreadFlowBodyShape { id: SyntaxNodeId },
    #[error("syntax identity {id:?} has an invalid typed Choice shape")]
    InvalidChoiceShape { id: SyntaxNodeId },
    #[error("syntax identity {id:?} has an invalid typed trigger-pattern shape")]
    InvalidTriggerShape { id: SyntaxNodeId },
    #[error("syntax item identity {id:?} carries an invalid typed projection")]
    InvalidItemProjection { id: SyntaxNodeId },
    #[error("source-file identity {parent:?} has unsupported direct child role {role:?}")]
    InvalidSourceFileChildRole {
        parent: SyntaxNodeId,
        role: SyntaxRole,
    },
}
