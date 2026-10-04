//! Stable generic-Match semantic transcripts.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_dialogue::rich_text::{
    DialogueControlProperty, DialogueHostEventKind, DialogueHostProperty,
};

use super::{
    CheckedCompileTimeCallee, CheckedExpressionResolution, CheckedExpressionSemanticDigest,
    CheckedMatchLimits, CheckedMatchRef, CheckedMatchSemanticDigest, CheckedPatternSemanticDigest,
    CheckedSelectResolution, CheckedValueResolution, FinalSemanticAnalysis,
    FinalSemanticAnalysisControl,
    match_coverage::{
        CheckedCoverageWitness, CheckedGuardClass, CheckedMatchBudget, CheckedMatchBuildError,
        CheckedMatchCoverage, CheckedMatchLimitKind, CheckedSequencePartitionWitness,
        CheckedUnreachableReason, CheckedVariantCoverageWitness,
        CheckedVariantRecordCoverageWitnessField, CoverageArmInput, MatchCoverageAnalyzer,
        StableMatchArmCoordinate,
    },
    transcript_writer::{TranscriptByteCounter, TranscriptHasher, TranscriptWriteError},
};
use crate::checked_compile_time::CheckedCompileTimeScalar;
use crate::checked_rich_text::{
    CheckedDialogueToken, CheckedFieldOrigin, CheckedRichTextAction, CheckedRichTextProperty,
    CheckedRichTextReport, CheckedRichTextValue,
};
use crate::semantic_coordinate::{
    AcceptedDeclarationSemanticId, AcceptedSemanticRoot, AcceptedSemanticRootCatalogError,
    CheckedSemanticPath, SemanticCoordinateEncodingError, SemanticCoordinateIndex,
    SemanticCoordinateIndexError, StableCheckedBodyCoordinate, StableCheckedValueCoordinate,
    StablePatternCoordinate, StablePatternCoordinateStep, StableSemanticCoordinate,
};
use crate::types::{ArrayLength, SemanticTypeDigest, TypeKind};
use arcweft_lang_hir::{
    body_edges::{HirBodyChild, HirBodyProjection},
    expr::{
        HirCallInvocation, HirCallTypeApplication, HirCallTypeApplicationTerminator,
        HirCallTypeArgument, HirChoiceCompactAction, HirChoiceItem, HirExprKind, HirMatchExpr,
    },
    identity::{ExprId, PatternId},
    leaf::{HirLiteral, HirNumericSequenceRecovery},
    module::HirModule,
    pattern::{HirPatternChild, HirPatternChildRole, HirPatternKind},
    project::{
        HirAnalysisProjectView, HirSemanticBodyLocator, HirSemanticBodyOwner, HirSemanticPathRoot,
    },
    stmt::{HirStatementChild, HirStatementChildRole, HirStmtKind},
    symbol::ProjectSymbolTable,
};
use thiserror::Error;

macro_rules! transcript_update {
    ($hasher:expr, $bytes:expr $(,)?) => {
        $hasher.update($bytes)?
    };
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum SemanticTranscriptError {
    #[error(transparent)]
    GenericScope(#[from] crate::types::GenericScopeError),
    #[error(transparent)]
    Generation(Box<super::FinalSemanticAnalysisError>),
    #[error("expression is not a Match")]
    NotMatch,
    #[error("checked Match evidence is missing or stale")]
    MissingMatchFact,
    #[error("checked expression evidence is missing")]
    MissingExpression,
    #[error("checked pattern evidence is missing")]
    MissingPattern,
    #[error("checked child-edge evidence is missing")]
    MissingChildEdges,
    #[error("selected callable join is missing")]
    MissingCallableJoin,
    #[error("selected call application is missing or stale")]
    MissingSelectedCallApplication,
    #[error("recovered semantic owner cannot be transcribed")]
    RecoveredOwner,
    #[error("semantic transcript work limit exceeded")]
    WorkLimit,
    #[error("semantic transcript byte accounting overflow")]
    TranscriptArithmeticOverflow,
    #[error("semantic transcript byte limit {limit} exceeded by attempt {attempted}")]
    TranscriptLimitExceeded { limit: u64, attempted: u64 },
    #[error(transparent)]
    MatchBuild(#[from] CheckedMatchBuildError),
    #[error("semantic transcript cannot resolve an accepted identity")]
    MissingIdentity,
    #[error(transparent)]
    AcceptedRootCatalog(AcceptedSemanticRootCatalogError),
    #[error(transparent)]
    CoordinateEncoding(#[from] SemanticCoordinateEncodingError),
    #[error("Match is not exhaustive; coverage witness is retained in the error")]
    NonExhaustive { witness: CheckedCoverageWitness },
}

impl From<TranscriptWriteError> for SemanticTranscriptError {
    fn from(error: TranscriptWriteError) -> Self {
        match error {
            TranscriptWriteError::ArithmeticOverflow => Self::TranscriptArithmeticOverflow,
            TranscriptWriteError::LimitExceeded { limit, attempted } => {
                Self::TranscriptLimitExceeded { limit, attempted }
            }
        }
    }
}

impl From<super::FinalSemanticAnalysisError> for SemanticTranscriptError {
    fn from(error: super::FinalSemanticAnalysisError) -> Self {
        Self::Generation(Box::new(error))
    }
}

impl From<SemanticCoordinateIndexError> for SemanticTranscriptError {
    fn from(error: SemanticCoordinateIndexError) -> Self {
        match error {
            SemanticCoordinateIndexError::RootCatalog(error) => Self::AcceptedRootCatalog(error),
            SemanticCoordinateIndexError::ControlTransferLookup(_) => Self::MissingIdentity,
            SemanticCoordinateIndexError::MissingChildEdges => Self::MissingChildEdges,
            SemanticCoordinateIndexError::MissingOwner { .. }
            | SemanticCoordinateIndexError::MissingBody { .. }
            | SemanticCoordinateIndexError::InvalidBindingPath { .. }
            | SemanticCoordinateIndexError::ExpressionRoleMismatch
            | SemanticCoordinateIndexError::InvalidRootPath => Self::MissingIdentity,
        }
    }
}

/// Public classification of a structured missing-case witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedMatchWitnessKind {
    Unit,
    Bool,
    Literal,
    Entity,
    Other,
    Variant,
    Tuple,
    Record,
    Array,
    Sequence,
    Choice,
}

/// A missing-case witness retained by a rejected Match query.
///
/// The matrix algebra and its private identity rows remain owned by sema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchWitness(CheckedCoverageWitness);

impl CheckedMatchWitness {
    pub fn view(&self) -> CheckedMatchWitnessView<'_> {
        CheckedMatchWitnessRef(&self.0).view()
    }

    pub const fn kind(&self) -> CheckedMatchWitnessKind {
        match self.0 {
            CheckedCoverageWitness::Unit => CheckedMatchWitnessKind::Unit,
            CheckedCoverageWitness::Bool(_) => CheckedMatchWitnessKind::Bool,
            CheckedCoverageWitness::Literal(_) => CheckedMatchWitnessKind::Literal,
            CheckedCoverageWitness::Entity(_) => CheckedMatchWitnessKind::Entity,
            CheckedCoverageWitness::Other { .. } => CheckedMatchWitnessKind::Other,
            CheckedCoverageWitness::Variant { .. } => CheckedMatchWitnessKind::Variant,
            CheckedCoverageWitness::Tuple(_) => CheckedMatchWitnessKind::Tuple,
            CheckedCoverageWitness::Record { .. } => CheckedMatchWitnessKind::Record,
            CheckedCoverageWitness::Array(_) => CheckedMatchWitnessKind::Array,
            CheckedCoverageWitness::Sequence { .. } => CheckedMatchWitnessKind::Sequence,
            CheckedCoverageWitness::Choice { .. } => CheckedMatchWitnessKind::Choice,
        }
    }

    pub const fn boolean(&self) -> Option<bool> {
        match self.0 {
            CheckedCoverageWitness::Bool(value) => Some(value),
            _ => None,
        }
    }
}

/// A borrowed child in a structured non-exhaustiveness witness.
#[derive(Clone, Copy, Debug)]
pub struct CheckedMatchWitnessRef<'a>(&'a CheckedCoverageWitness);

impl<'a> CheckedMatchWitnessRef<'a> {
    pub fn view(self) -> CheckedMatchWitnessView<'a> {
        match self.0 {
            CheckedCoverageWitness::Unit => CheckedMatchWitnessView::Unit,
            CheckedCoverageWitness::Bool(value) => CheckedMatchWitnessView::Bool(*value),
            CheckedCoverageWitness::Literal(literal) => CheckedMatchWitnessView::Literal {
                semantic_type: literal.semantic_type,
                canonical_bytes: &literal.bytes,
            },
            CheckedCoverageWitness::Entity(item) => CheckedMatchWitnessView::Entity {
                semantic_id: item.as_bytes(),
            },
            CheckedCoverageWitness::Other { type_digest } => CheckedMatchWitnessView::Other {
                type_digest: *type_digest,
            },
            CheckedCoverageWitness::Variant { case, payload } => {
                let payload = match payload {
                    CheckedVariantCoverageWitness::Unit => CheckedMatchVariantPayloadView::Unit,
                    CheckedVariantCoverageWitness::Tuple(values) => {
                        CheckedMatchVariantPayloadView::Tuple(CheckedMatchWitnessList(values))
                    }
                    CheckedVariantCoverageWitness::Record(fields) => {
                        CheckedMatchVariantPayloadView::Record(CheckedMatchVariantFieldList(fields))
                    }
                };
                CheckedMatchWitnessView::Variant {
                    case_semantic_id: case.as_bytes(),
                    payload,
                }
            }
            CheckedCoverageWitness::Tuple(values) => {
                CheckedMatchWitnessView::Tuple(CheckedMatchWitnessList(values))
            }
            CheckedCoverageWitness::Record { owner, fields } => CheckedMatchWitnessView::Record {
                owner: *owner,
                fields: CheckedMatchWitnessList(fields),
            },
            CheckedCoverageWitness::Array(values) => {
                CheckedMatchWitnessView::Array(CheckedMatchWitnessList(values))
            }
            CheckedCoverageWitness::Sequence {
                partition,
                visible_prefix,
            } => CheckedMatchWitnessView::Sequence {
                partition: match partition {
                    CheckedSequencePartitionWitness::Exact(length) => {
                        CheckedMatchSequencePartition::Exact(*length)
                    }
                    CheckedSequencePartitionWitness::Interval {
                        lower,
                        upper_exclusive,
                    } => CheckedMatchSequencePartition::Interval {
                        lower: *lower,
                        upper_exclusive: *upper_exclusive,
                    },
                },
                visible_prefix: CheckedMatchWitnessList(visible_prefix),
            },
            CheckedCoverageWitness::Choice {
                ordinal,
                alternative,
                value,
            } => CheckedMatchWitnessView::Choice {
                ordinal: *ordinal,
                alternative: *alternative,
                value: CheckedMatchWitnessRef(value),
            },
        }
    }
}

/// Borrowed children of one tuple, record, array, sequence, or variant witness.
#[derive(Clone, Copy, Debug)]
pub struct CheckedMatchWitnessList<'a>(&'a [CheckedCoverageWitness]);

impl<'a> CheckedMatchWitnessList<'a> {
    pub const fn len(self) -> usize {
        self.0.len()
    }

    pub const fn is_empty(self) -> bool {
        self.0.is_empty()
    }

    pub fn get(self, index: usize) -> Option<CheckedMatchWitnessRef<'a>> {
        self.0.get(index).map(CheckedMatchWitnessRef)
    }

    pub fn iter(self) -> impl ExactSizeIterator<Item = CheckedMatchWitnessRef<'a>> + 'a {
        self.0.iter().map(CheckedMatchWitnessRef)
    }
}

/// Borrowed field in a variant's record payload witness.
#[derive(Clone, Copy, Debug)]
pub struct CheckedMatchVariantFieldRef<'a>(&'a CheckedVariantRecordCoverageWitnessField);

impl<'a> CheckedMatchVariantFieldRef<'a> {
    pub const fn semantic_id(self) -> &'a [u8; 32] {
        self.0.semantic_id_bytes()
    }

    pub const fn value(self) -> CheckedMatchWitnessRef<'a> {
        CheckedMatchWitnessRef(self.0.value())
    }
}

/// Borrowed source-ordered fields in a variant's record payload witness.
#[derive(Clone, Copy, Debug)]
pub struct CheckedMatchVariantFieldList<'a>(&'a [CheckedVariantRecordCoverageWitnessField]);

impl<'a> CheckedMatchVariantFieldList<'a> {
    pub const fn len(self) -> usize {
        self.0.len()
    }

    pub const fn is_empty(self) -> bool {
        self.0.is_empty()
    }

    pub fn get(self, index: usize) -> Option<CheckedMatchVariantFieldRef<'a>> {
        self.0.get(index).map(CheckedMatchVariantFieldRef)
    }

    pub fn iter(self) -> impl ExactSizeIterator<Item = CheckedMatchVariantFieldRef<'a>> + 'a {
        self.0.iter().map(CheckedMatchVariantFieldRef)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedMatchSequencePartition {
    Exact(u64),
    Interval {
        lower: u64,
        upper_exclusive: Option<u64>,
    },
}

/// Read-only shape of a variant payload coverage witness.
#[derive(Clone, Copy, Debug)]
pub enum CheckedMatchVariantPayloadView<'a> {
    Unit,
    Tuple(CheckedMatchWitnessList<'a>),
    Record(CheckedMatchVariantFieldList<'a>),
}

/// Read-only traversal of the same witness retained by the coverage matrix.
#[derive(Clone, Copy, Debug)]
pub enum CheckedMatchWitnessView<'a> {
    Unit,
    Bool(bool),
    Literal {
        semantic_type: SemanticTypeDigest,
        canonical_bytes: &'a [u8],
    },
    Entity {
        semantic_id: &'a [u8; 32],
    },
    Other {
        type_digest: SemanticTypeDigest,
    },
    Variant {
        case_semantic_id: &'a [u8; 32],
        payload: CheckedMatchVariantPayloadView<'a>,
    },
    Tuple(CheckedMatchWitnessList<'a>),
    Record {
        owner: SemanticTypeDigest,
        fields: CheckedMatchWitnessList<'a>,
    },
    Array(CheckedMatchWitnessList<'a>),
    Sequence {
        partition: CheckedMatchSequencePartition,
        visible_prefix: CheckedMatchWitnessList<'a>,
    },
    Choice {
        ordinal: u32,
        alternative: SemanticTypeDigest,
        value: CheckedMatchWitnessRef<'a>,
    },
}

/// Failure of an atomic, generation-checked semantic transcript query.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedSemanticTranscriptError {
    #[error(transparent)]
    Generation(#[from] super::FinalSemanticAnalysisError),
    #[error("expression is missing from the accepted HIR generation")]
    MissingExpression,
    #[error("expression is not a Match")]
    NotMatch,
    #[error("checked Match evidence is missing or stale")]
    MissingMatchFact,
    #[error("accepted checked Match evidence is invalid or incomplete")]
    InvalidEvidence,
    #[error("Match is non-exhaustive")]
    NonExhaustive { witness: CheckedMatchWitness },
    #[error("Match has no exact coverage domain for type {type_digest:?}")]
    UnsupportedDomain { type_digest: SemanticTypeDigest },
    #[error("checked Match {kind:?} limit {limit} exceeded by attempt {attempted}")]
    LimitExceeded {
        kind: CheckedMatchLimitKind,
        limit: u64,
        attempted: u64,
    },
    #[error("checked Match {kind:?} accounting overflow")]
    ArithmeticOverflow { kind: CheckedMatchLimitKind },
    #[error("checked Match coordinate or ordinal exceeds its representable range")]
    CoordinateOverflow,
    #[error("checked Match construction was cancelled")]
    Cancelled,
}

impl From<SemanticTranscriptError> for CheckedSemanticTranscriptError {
    fn from(error: SemanticTranscriptError) -> Self {
        match error {
            SemanticTranscriptError::Generation(error) => Self::Generation(*error),
            SemanticTranscriptError::MissingExpression => Self::MissingExpression,
            SemanticTranscriptError::NotMatch => Self::NotMatch,
            SemanticTranscriptError::MissingMatchFact => Self::MissingMatchFact,
            SemanticTranscriptError::NonExhaustive { witness } => Self::NonExhaustive {
                witness: CheckedMatchWitness(witness),
            },
            SemanticTranscriptError::MatchBuild(CheckedMatchBuildError::UnsupportedDomain {
                type_digest,
            }) => Self::UnsupportedDomain { type_digest },
            SemanticTranscriptError::MatchBuild(CheckedMatchBuildError::LimitExceeded {
                kind,
                limit,
                attempted,
            }) => Self::LimitExceeded {
                kind,
                limit,
                attempted,
            },
            SemanticTranscriptError::MatchBuild(CheckedMatchBuildError::ArithmeticOverflow {
                kind,
            }) => Self::ArithmeticOverflow { kind },
            SemanticTranscriptError::MatchBuild(CheckedMatchBuildError::Cancelled) => {
                Self::Cancelled
            }
            SemanticTranscriptError::TranscriptArithmeticOverflow => Self::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::TranscriptBytes,
            },
            SemanticTranscriptError::TranscriptLimitExceeded { limit, attempted } => {
                Self::LimitExceeded {
                    kind: CheckedMatchLimitKind::TranscriptBytes,
                    limit,
                    attempted,
                }
            }
            SemanticTranscriptError::WorkLimit => Self::CoordinateOverflow,
            SemanticTranscriptError::GenericScope(_)
            | SemanticTranscriptError::MissingPattern
            | SemanticTranscriptError::MissingChildEdges
            | SemanticTranscriptError::MissingCallableJoin
            | SemanticTranscriptError::MissingSelectedCallApplication
            | SemanticTranscriptError::RecoveredOwner
            | SemanticTranscriptError::MatchBuild(_)
            | SemanticTranscriptError::MissingIdentity
            | SemanticTranscriptError::AcceptedRootCatalog(_)
            | SemanticTranscriptError::CoordinateEncoding(_) => Self::InvalidEvidence,
        }
    }
}

type MatchTranscriptHasher<'a> = TranscriptHasher<'a, CheckedMatchBudget>;

/// Version-one semantic identity of a checked rich-text content stream.
///
/// This is deliberately an opaque transcript-local digest.  Source spans,
/// HIR IDs, and diagnostic spellings are never part of its byte stream.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CheckedRichTextSemanticDigest([u8; 32]);

impl CheckedRichTextSemanticDigest {
    const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Version-one semantic identity of one checked statement.
///
/// Statement digests are private to the final semantic authority.  A caller
/// can observe the enclosing expression/body products, but cannot mint or
/// deserialize a statement digest independently of the accepted report.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CheckedStatementSemanticDigest([u8; 32]);

impl CheckedStatementSemanticDigest {
    const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Version-one semantic identity of one checked body container.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CheckedBodySemanticDigest([u8; 32]);

impl CheckedBodySemanticDigest {
    const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchBinding {
    coordinate: StableCheckedValueCoordinate,
    ty: SemanticTypeDigest,
}

impl CheckedMatchBinding {
    pub const fn coordinate(&self) -> &StableCheckedValueCoordinate {
        &self.coordinate
    }

    pub const fn ty(&self) -> SemanticTypeDigest {
        self.ty
    }
}

/// The checked meaning of an authored Match guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedGuardSemantic {
    Absent,
    ConstantTrue(CheckedExpressionSemanticDigest),
    ConstantFalse(CheckedExpressionSemanticDigest),
    Dynamic(CheckedExpressionSemanticDigest),
}

impl CheckedGuardSemantic {
    const fn expression(self) -> Option<CheckedExpressionSemanticDigest> {
        match self {
            Self::Absent => None,
            Self::ConstantTrue(digest) | Self::ConstantFalse(digest) | Self::Dynamic(digest) => {
                Some(digest)
            }
        }
    }

    const fn class(self) -> CheckedGuardClass {
        match self {
            Self::Absent => CheckedGuardClass::Absent,
            Self::ConstantTrue(_) => CheckedGuardClass::ConstantTrue,
            Self::ConstantFalse(_) => CheckedGuardClass::ConstantFalse,
            Self::Dynamic(_) => CheckedGuardClass::Dynamic,
        }
    }
}

/// One source-ordered checked Match arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchArm {
    coordinate: StableMatchArmCoordinate,
    pattern: CheckedPatternSemanticDigest,
    guard: CheckedGuardSemantic,
    result: CheckedExpressionSemanticDigest,
    bindings: Box<[CheckedMatchBinding]>,
}

impl CheckedMatchArm {
    pub const fn coordinate(&self) -> &StableMatchArmCoordinate {
        &self.coordinate
    }

    pub const fn pattern(&self) -> CheckedPatternSemanticDigest {
        self.pattern
    }

    pub const fn guard(&self) -> CheckedGuardSemantic {
        self.guard
    }

    pub const fn result(&self) -> CheckedExpressionSemanticDigest {
        self.result
    }

    pub fn bindings(&self) -> &[CheckedMatchBinding] {
        &self.bindings
    }
}

/// Version-one digest and local checked byte length of a complete Match transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatchSemanticTranscript {
    version: u8,
    digest: CheckedMatchSemanticDigest,
    byte_len: u64,
}

impl MatchSemanticTranscript {
    pub const VERSION: u8 = 1;

    pub const fn version(&self) -> u8 {
        self.version
    }

    pub const fn digest(&self) -> CheckedMatchSemanticDigest {
        self.digest
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }
}

/// One exhaustive, generation-bound, completely transcribed Match result.
#[derive(Clone, Debug)]
pub struct CheckedMatch {
    lookup: CheckedMatchRef,
    path: CheckedSemanticPath,
    scrutinee: CheckedExpressionSemanticDigest,
    scrutinee_type: SemanticTypeDigest,
    arms: Box<[CheckedMatchArm]>,
    transcript: MatchSemanticTranscript,
    coverage: CheckedMatchCoverage,
}

impl CheckedMatch {
    pub const fn path(&self) -> &CheckedSemanticPath {
        &self.path
    }

    pub const fn root(&self) -> AcceptedSemanticRoot {
        self.path.root()
    }

    pub const fn callable_owner(&self) -> Option<AcceptedDeclarationSemanticId> {
        match self.root() {
            AcceptedSemanticRoot::Declaration(owner) => Some(owner),
            AcceptedSemanticRoot::Item(_) => None,
        }
    }

    pub const fn scrutinee(&self) -> CheckedExpressionSemanticDigest {
        self.scrutinee
    }

    pub const fn scrutinee_type(&self) -> SemanticTypeDigest {
        self.scrutinee_type
    }

    pub const fn transcript(&self) -> &MatchSemanticTranscript {
        &self.transcript
    }

    pub const fn semantic_digest(&self) -> CheckedMatchSemanticDigest {
        self.transcript.digest()
    }
    pub fn arms(&self) -> &[CheckedMatchArm] {
        &self.arms
    }
    pub const fn coverage(&self) -> &CheckedMatchCoverage {
        &self.coverage
    }
}

/// Runs one bounded transcript transaction for a validated Match reference.
/// The builder is discarded on every error, so no partial Match product can
/// escape the transaction.
fn build_checked_match_transaction(
    analysis: &FinalSemanticAnalysis,
    project: HirAnalysisProjectView<'_>,
    owner: ExprId,
    limits: CheckedMatchLimits,
    control: FinalSemanticAnalysisControl<'_>,
) -> Result<CheckedMatch, SemanticTranscriptError> {
    let module = project
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module.as_ref()))
        .ok_or(SemanticTranscriptError::MissingExpression)?;
    let mut builder = SemanticTranscriptGraph::new(analysis, module, limits, control);
    builder.expression_digest(owner)?;
    let mut product = builder
        .match_products
        .remove(&owner)
        .ok_or(SemanticTranscriptError::MissingMatchFact)?;
    product
        .coverage
        .finish_transaction_work(builder.budget.work());
    Ok(product)
}

/// Issues the acyclic digest for one declaration-owned parameter or attached
/// content default while the checked callable interfaces are still unsealed.
pub(crate) fn checked_declaration_default_expression_digest(
    analysis: &FinalSemanticAnalysis,
    project: HirAnalysisProjectView<'_>,
    owner: ExprId,
    control: FinalSemanticAnalysisControl<'_>,
) -> Result<crate::callable::CheckedDeclarationDefaultExpressionDigest, SemanticTranscriptError> {
    if analysis
        .checked_callables()
        .records()
        .any(|facts| facts.sealed_interface_digest().is_some())
    {
        return Err(SemanticTranscriptError::MissingIdentity);
    }
    let module = project
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module.as_ref()))
        .ok_or(SemanticTranscriptError::MissingExpression)?;
    let coordinates = SemanticCoordinateIndex::new(analysis.accepted_root_catalog(), analysis);
    let coordinate = coordinates.expression(owner)?;
    if !matches!(
        coordinate.steps().last(),
        Some(
            crate::semantic_coordinate::CheckedSemanticPathStep::AttachedContentDefault
                | crate::semantic_coordinate::CheckedSemanticPathStep::ParameterDefault { .. }
        )
    ) {
        return Err(SemanticTranscriptError::MissingIdentity);
    }
    let mut builder =
        SemanticTranscriptGraph::new(analysis, module, CheckedMatchLimits::PRODUCTION, control);
    let expression = builder.expression_digest(owner)?;
    let mut hasher = TranscriptHasher::new(&mut builder.budget);
    transcript_update!(
        hasher,
        b"arcweft.lang.checked-declaration-default-expression.v1\0"
    );
    write_bytes(&mut hasher, &coordinate.canonical_bytes()?)?;
    transcript_update!(hasher, expression.as_bytes());
    Ok(crate::callable::CheckedDeclarationDefaultExpressionDigest::from_bytes(hasher.finalize()))
}

/// Issues the canonical semantic digest of one accepted parameter pattern for
/// a declaration-default capture row. The digest commits stable coordinates,
/// the complete checked pattern tree, and every binding/type child; raw
/// generation-local pattern IDs never enter an interface digest.
pub(crate) fn checked_declaration_default_pattern_digest(
    analysis: &FinalSemanticAnalysis,
    module: &HirModule,
    owner: PatternId,
    control: FinalSemanticAnalysisControl<'_>,
) -> Result<CheckedPatternSemanticDigest, SemanticTranscriptError> {
    SemanticTranscriptGraph::new(analysis, module, CheckedMatchLimits::PRODUCTION, control)
        .generic_pattern_digest(owner)
}

impl FinalSemanticAnalysis {
    /// Commits the complete accepted expression subtree, including exhaustive
    /// nested Match products, from this exact HIR generation.
    pub fn checked_expression_semantic_digest(
        &self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        expression: ExprId,
        limits: CheckedMatchLimits,
    ) -> Result<CheckedExpressionSemanticDigest, CheckedSemanticTranscriptError> {
        static NOT_CANCELLED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        self.validate_generation(project, symbols)?;
        let module = project
            .modules()
            .find_map(|(_, module)| {
                (module.module_id() == expression.module()).then_some(module.as_ref())
            })
            .ok_or(CheckedSemanticTranscriptError::MissingExpression)?;
        SemanticTranscriptGraph::new(
            self,
            module,
            limits,
            FinalSemanticAnalysisControl::new(&NOT_CANCELLED),
        )
        .expression_digest(expression)
        .map_err(CheckedSemanticTranscriptError::from)
    }

    /// Returns one complete exhaustive Match product from this exact accepted
    /// HIR generation. A failed query publishes no partial semantic result.
    pub fn checked_match(
        &self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        expression: ExprId,
        limits: CheckedMatchLimits,
    ) -> Result<CheckedMatch, CheckedSemanticTranscriptError> {
        static NOT_CANCELLED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        self.checked_match_with_control(
            project,
            symbols,
            expression,
            limits,
            FinalSemanticAnalysisControl::new(&NOT_CANCELLED),
        )
        .map_err(CheckedSemanticTranscriptError::from)
    }

    /// Constructs one checked Match while observing caller-owned cancellation.
    /// The transaction is rooted at the requested Match and includes only its
    /// accepted expression/pattern/statement/body subtree and nested Matches.
    pub(crate) fn checked_match_with_control(
        &self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        expression: ExprId,
        limits: CheckedMatchLimits,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<CheckedMatch, SemanticTranscriptError> {
        control.check()?;
        self.validate_generation(project, symbols)?;
        let module = project
            .modules()
            .find_map(|(_, module)| {
                (module.module_id() == expression.module()).then_some(module.as_ref())
            })
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        let owner = module
            .resolve_expr(expression)
            .map_err(|_| SemanticTranscriptError::MissingExpression)?;
        let HirExprKind::Match(authored) = owner.kind() else {
            return Err(SemanticTranscriptError::NotMatch);
        };
        let checked = self
            .expression(expression)
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        let fact = checked
            .match_fact()
            .ok_or(SemanticTranscriptError::MissingMatchFact)?;
        if fact.scrutinee() != authored.scrutinee() || fact.arms().len() != authored.arms().len() {
            return Err(SemanticTranscriptError::MissingMatchFact);
        }
        let product = build_checked_match_transaction(self, project, expression, limits, control)?;
        if product.lookup != CheckedMatchRef::new(module.snapshot_id(), expression) {
            return Err(SemanticTranscriptError::MissingMatchFact);
        }
        Ok(product)
    }
}

struct SemanticTranscriptGraph<'analysis, 'control> {
    analysis: &'analysis FinalSemanticAnalysis,
    module: &'analysis HirModule,
    coordinates: SemanticCoordinateIndex<'analysis, 'analysis>,
    control: FinalSemanticAnalysisControl<'control>,
    budget: CheckedMatchBudget,
    expression_digests: BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    expression_paths: BTreeMap<StableSemanticCoordinate, ExprId>,
    expression_visiting: BTreeSet<ExprId>,
    pattern_digests: BTreeMap<PatternId, CheckedPatternSemanticDigest>,
    pattern_coordinates: BTreeMap<PatternId, StableSemanticCoordinate>,
    pattern_paths: BTreeMap<StableSemanticCoordinate, PatternId>,
    pattern_visiting: BTreeSet<PatternId>,
    coverage_pattern_visiting: BTreeSet<PatternId>,
    observed_patterns: Vec<(PatternId, StableSemanticCoordinate)>,
    statement_digests: BTreeMap<arcweft_lang_hir::identity::StmtId, CheckedStatementSemanticDigest>,
    statement_visiting: BTreeSet<arcweft_lang_hir::identity::StmtId>,
    body_digests: BTreeMap<
        crate::semantic_coordinate::StableCheckedBodyCoordinate,
        CheckedBodySemanticDigest,
    >,
    body_visiting: BTreeSet<crate::semantic_coordinate::StableCheckedBodyCoordinate>,
    match_products: BTreeMap<ExprId, CheckedMatch>,
}

impl<'analysis, 'control> SemanticTranscriptGraph<'analysis, 'control> {
    fn new(
        analysis: &'analysis FinalSemanticAnalysis,
        module: &'analysis HirModule,
        limits: CheckedMatchLimits,
        control: FinalSemanticAnalysisControl<'control>,
    ) -> Self {
        Self {
            analysis,
            module,
            coordinates: SemanticCoordinateIndex::new(analysis.accepted_root_catalog(), analysis),
            control,
            budget: CheckedMatchBudget::new(limits),
            expression_digests: BTreeMap::new(),
            expression_paths: BTreeMap::new(),
            expression_visiting: BTreeSet::new(),
            pattern_digests: BTreeMap::new(),
            pattern_coordinates: BTreeMap::new(),
            pattern_paths: BTreeMap::new(),
            pattern_visiting: BTreeSet::new(),
            coverage_pattern_visiting: BTreeSet::new(),
            observed_patterns: Vec::new(),
            statement_digests: BTreeMap::new(),
            statement_visiting: BTreeSet::new(),
            body_digests: BTreeMap::new(),
            body_visiting: BTreeSet::new(),
            match_products: BTreeMap::new(),
        }
    }
    fn build(
        &mut self,
        owner: ExprId,
        authored: &HirMatchExpr,
    ) -> Result<CheckedMatch, SemanticTranscriptError> {
        let scrutinee = self.expression_digest(authored.scrutinee())?;
        self.control.check()?;
        let scrutinee_ty = self
            .analysis
            .expression(authored.scrutinee())
            .ok_or(SemanticTranscriptError::MissingExpression)?
            .value_type()
            .ok_or_else(|| {
                SemanticTranscriptError::from(
                    super::FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: authored.scrutinee(),
                    },
                )
            })?
            .clone();
        let scrutinee_type = scrutinee_ty.semantic_identity_digest()?;
        let checked_owner = self
            .analysis
            .expression(owner)
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        let fact = checked_owner
            .match_fact()
            .ok_or(SemanticTranscriptError::MissingMatchFact)?;
        let match_path = self.checked_path(owner)?;
        let arm_count = u64::try_from(authored.arms().len()).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::Arms,
            }
        })?;
        self.budget.charge(CheckedMatchLimitKind::Arms, arm_count)?;
        let mut arms = Vec::new();
        arms.try_reserve_exact(authored.arms().len()).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::Arms,
            }
        })?;
        let mut coverage_arms = Vec::new();
        coverage_arms
            .try_reserve_exact(authored.arms().len())
            .map_err(|_| CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::Arms,
            })?;
        for (ordinal, (arm, checked)) in authored.arms().iter().zip(fact.arms()).enumerate() {
            let ordinal = u32::try_from(ordinal).map_err(|_| SemanticTranscriptError::WorkLimit)?;
            let arm_coordinate = StableMatchArmCoordinate::new(match_path.clone(), ordinal);
            let pattern = self.pattern_digest(
                arm.pattern(),
                &arm_coordinate,
                &StablePatternCoordinate::new([]),
            )?;
            let mut bindings = Vec::new();
            self.collect_pattern_bindings(
                arm.pattern(),
                &StablePatternCoordinate::new([]),
                &mut bindings,
            )?;
            let guard = match (arm.guard(), checked.guard()) {
                (None, None) => CheckedGuardSemantic::Absent,
                (Some(authored), Some(checked)) if authored == checked => {
                    let digest = self.expression_digest(authored)?;
                    match guard_class(self.analysis, authored)? {
                        CheckedGuardClass::ConstantTrue => {
                            CheckedGuardSemantic::ConstantTrue(digest)
                        }
                        CheckedGuardClass::ConstantFalse => {
                            CheckedGuardSemantic::ConstantFalse(digest)
                        }
                        CheckedGuardClass::Dynamic => CheckedGuardSemantic::Dynamic(digest),
                        CheckedGuardClass::Absent => {
                            return Err(SemanticTranscriptError::MissingMatchFact);
                        }
                    }
                }
                _ => return Err(SemanticTranscriptError::MissingMatchFact),
            };
            if arm.value() != checked.value() {
                return Err(SemanticTranscriptError::MissingMatchFact);
            }
            arms.push(CheckedMatchArm {
                coordinate: arm_coordinate.clone(),
                pattern,
                guard,
                result: self.expression_digest(arm.value())?,
                bindings: bindings.into_boxed_slice(),
            });
            coverage_arms.push(CoverageArmInput {
                coordinate: arm_coordinate,
                pattern: arm.pattern(),
                guard: guard.class(),
            });
        }
        let mut coverage = MatchCoverageAnalyzer::new(
            self.analysis,
            self.module,
            self.control,
            &mut self.budget,
            StableSemanticCoordinate::new(match_path.clone()),
            std::mem::take(&mut self.observed_patterns),
        )
        .analyze(&scrutinee_ty, &coverage_arms)?;
        if let Some(witness) = coverage.witness().cloned() {
            return Err(SemanticTranscriptError::NonExhaustive { witness });
        }
        let transcript = match_digest(
            &mut self.budget,
            scrutinee,
            scrutinee_type,
            &arms,
            &coverage,
        )?;
        coverage.finish_transaction_work(self.budget.work());
        Ok(CheckedMatch {
            lookup: CheckedMatchRef::new(self.module.snapshot_id(), owner),
            path: match_path,
            scrutinee,
            scrutinee_type,
            arms: arms.into_boxed_slice(),
            transcript,
            coverage,
        })
    }

    fn expression_digest(
        &mut self,
        owner: ExprId,
    ) -> Result<CheckedExpressionSemanticDigest, SemanticTranscriptError> {
        self.expression_digest_at(owner, 0)
    }

    fn checked_path(&self, owner: ExprId) -> Result<CheckedSemanticPath, SemanticTranscriptError> {
        Ok(self.coordinates.expression(owner)?)
    }

    fn pattern_digest(
        &mut self,
        owner: PatternId,
        arm: &StableMatchArmCoordinate,
        coordinate: &StablePatternCoordinate,
    ) -> Result<CheckedPatternSemanticDigest, SemanticTranscriptError> {
        self.control.check()?;
        let depth = u64::try_from(coordinate.steps().len())
            .map_err(|_| SemanticTranscriptError::WorkLimit)?;
        self.budget.observe_depth(depth)?;
        let semantic_coordinate = arm.pattern_coordinate(coordinate.clone());
        let accepted_coordinate = self.coordinates.pattern(owner)?;
        if !accepted_coordinate
            .path()
            .is_match_pattern_under(arm.owner(), arm.ordinal())
        {
            return Err(SemanticTranscriptError::MissingPattern);
        }
        if self
            .pattern_coordinates
            .get(&owner)
            .is_some_and(|existing| {
                existing != &semantic_coordinate || self.coverage_pattern_visiting.contains(&owner)
            })
        {
            return Err(CheckedMatchBuildError::DuplicateSemanticPath {
                coordinate: semantic_coordinate,
            }
            .into());
        }
        if self
            .pattern_paths
            .get(&semantic_coordinate)
            .is_some_and(|existing| *existing != owner)
        {
            return Err(CheckedMatchBuildError::DuplicateSemanticPath {
                coordinate: semantic_coordinate,
            }
            .into());
        }
        self.pattern_coordinates
            .insert(owner, semantic_coordinate.clone());
        self.pattern_paths
            .insert(semantic_coordinate.clone(), owner);
        if !self.coverage_pattern_visiting.insert(owner) {
            return Err(CheckedMatchBuildError::DuplicateSemanticPath {
                coordinate: semantic_coordinate,
            }
            .into());
        }
        self.observed_patterns.try_reserve_exact(1).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::PatternNodes,
            }
        })?;
        self.observed_patterns
            .push((owner, semantic_coordinate.clone()));
        let hir = self
            .module
            .resolve_pattern(owner)
            .map_err(|_| SemanticTranscriptError::MissingPattern)?;
        if matches!(hir.kind(), HirPatternKind::Error(_)) {
            return Err(SemanticTranscriptError::RecoveredOwner);
        }
        let checked = self
            .analysis
            .pattern(owner)
            .ok_or(SemanticTranscriptError::MissingPattern)?;
        if let super::CheckedPatternResolution::Record(record) = checked.resolution() {
            for field in record.fields() {
                let Some(child) = field.source().raw_pattern() else {
                    continue;
                };
                let child_coordinate = record_pattern_child_coordinate(coordinate, field)?;
                self.pattern_digest(child, arm, &child_coordinate)?;
            }
        } else {
            for edge in hir.kind().child_edges() {
                let HirPatternChild::Pattern(child) = edge.child() else {
                    continue;
                };
                let child_coordinate =
                    child_pattern_coordinate(coordinate, hir.kind(), edge.role())?;
                self.pattern_digest(child, arm, &child_coordinate)?;
            }
        }
        // The digest itself is issued by the one accepted-rooted generic
        // pattern memo.  The arm coordinate above is coverage evidence only;
        // it is never a second digest grammar.
        let digest = self.generic_pattern_digest(owner)?;
        self.coverage_pattern_visiting.remove(&owner);
        Ok(digest)
    }

    fn collect_pattern_bindings(
        &self,
        owner: PatternId,
        coordinate: &StablePatternCoordinate,
        bindings: &mut Vec<CheckedMatchBinding>,
    ) -> Result<(), SemanticTranscriptError> {
        self.control.check()?;
        let hir = self
            .module
            .resolve_pattern(owner)
            .map_err(|_| SemanticTranscriptError::MissingPattern)?;
        if matches!(hir.kind(), HirPatternKind::Error(_)) {
            return Err(SemanticTranscriptError::RecoveredOwner);
        }
        let checked = self
            .analysis
            .pattern(owner)
            .ok_or(SemanticTranscriptError::MissingPattern)?;
        if let super::CheckedPatternResolution::Record(record) = checked.resolution() {
            for field in record.fields() {
                if let Some(binding) = field.source().binding() {
                    bindings.push(CheckedMatchBinding {
                        coordinate: StableCheckedValueCoordinate::Binding(
                            binding.coordinate().clone(),
                        ),
                        ty: field.field_type_digest(),
                    });
                } else if let Some(child) = field.source().raw_pattern() {
                    let child_coordinate = record_pattern_child_coordinate(coordinate, field)?;
                    self.collect_pattern_bindings(child, &child_coordinate, bindings)?;
                } else {
                    return Err(SemanticTranscriptError::MissingIdentity);
                }
            }
            if let super::CheckedRecordPatternRest::Binding(binding) = record.rest() {
                let ty = self
                    .analysis
                    .local(binding.raw())
                    .ok_or(SemanticTranscriptError::MissingIdentity)?
                    .ty()
                    .semantic_identity_digest()?;
                bindings.push(CheckedMatchBinding {
                    coordinate: StableCheckedValueCoordinate::Binding(binding.coordinate().clone()),
                    ty,
                });
            }
            return Ok(());
        }
        for edge in hir.kind().child_edges() {
            match edge.child() {
                HirPatternChild::Local(local) => {
                    let ty = self
                        .analysis
                        .local(local)
                        .ok_or(SemanticTranscriptError::MissingIdentity)?
                        .ty()
                        .semantic_identity_digest()?;
                    bindings.push(CheckedMatchBinding {
                        coordinate: StableCheckedValueCoordinate::Binding(
                            self.coordinates.binding(local)?,
                        ),
                        ty,
                    });
                }
                HirPatternChild::Pattern(child) => {
                    let child_coordinate =
                        child_pattern_coordinate(coordinate, hir.kind(), edge.role())?;
                    self.collect_pattern_bindings(child, &child_coordinate, bindings)?;
                }
                HirPatternChild::Type(_) => {}
            }
        }
        Ok(())
    }
}

fn write_generic_pattern_resolution(
    hasher: &mut MatchTranscriptHasher<'_>,
    resolution: &super::CheckedPatternResolution,
    ty: &TypeKind,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    match resolution {
        super::CheckedPatternResolution::Structural => {}
        super::CheckedPatternResolution::Literal(literal) => {
            // Pattern literals are checked against the pattern type before
            // this boundary; their exact canonical scalar is still needed in
            // the transcript.
            write_literal(hasher, literal, ty)?;
        }
        super::CheckedPatternResolution::Entity(item) => {
            transcript_update!(hasher, item.semantic_id().as_bytes());
            transcript_update!(hasher, item.value_type().as_bytes());
        }
        super::CheckedPatternResolution::Record(record) => {
            transcript_update!(hasher, record.owner().semantic_type().as_bytes());
            match record.rest() {
                super::CheckedRecordPatternRest::Absent => transcript_update!(hasher, &[0]),
                super::CheckedRecordPatternRest::Ignore => transcript_update!(hasher, &[1]),
                super::CheckedRecordPatternRest::Binding(binding) => {
                    transcript_update!(hasher, &[2]);
                    transcript_update!(hasher, &binding.coordinate().canonical_bytes()?);
                }
            }
            write_len(hasher, record.fields().len())?;
            for field in record.fields() {
                transcript_update!(hasher, &field.source_ordinal().to_le_bytes());
                transcript_update!(hasher, &field.declaration_ordinal().to_le_bytes());
                transcript_update!(hasher, field.semantic_id().as_bytes());
                transcript_update!(hasher, field.field_type_digest().as_bytes());
                match field.source().value() {
                    super::CheckedRecordPatternSourceRef::Pattern(pattern) => {
                        transcript_update!(hasher, &[0]);
                        transcript_update!(
                            hasher,
                            &coordinates_for_pattern(analysis, pattern)?.canonical_bytes()?
                        );
                    }
                    super::CheckedRecordPatternSourceRef::Binding(binding) => {
                        transcript_update!(hasher, &[1]);
                        transcript_update!(hasher, &binding.coordinate().canonical_bytes()?);
                    }
                }
            }
        }
        super::CheckedPatternResolution::Variant(variant) => {
            write_variant_resolution(hasher, variant)?;
        }
        super::CheckedPatternResolution::TypedBinding(binding) => {
            transcript_update!(hasher, binding.annotation_digest().as_bytes());
            write_len(hasher, binding.choice_alternatives().len())?;
            for alternative in binding.choice_alternatives() {
                transcript_update!(hasher, &alternative.to_le_bytes());
            }
        }
    }
    Ok(())
}

fn coordinates_for_pattern(
    analysis: &FinalSemanticAnalysis,
    pattern: PatternId,
) -> Result<crate::semantic_coordinate::StableCheckedPatternOwnerCoordinate, SemanticTranscriptError>
{
    let coordinates = SemanticCoordinateIndex::new(analysis.accepted_root_catalog(), analysis);
    coordinates.pattern(pattern).map_err(Into::into)
}

fn write_statement_payload(
    hasher: &mut MatchTranscriptHasher<'_>,
    payload: &super::CheckedStatementPayload,
    owner_coordinate: &[u8],
    analysis: &FinalSemanticAnalysis,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<(), SemanticTranscriptError> {
    use super::{CheckedAssertionDisposition, CheckedStatementPayload};
    match payload {
        CheckedStatementPayload::Structural => {}
        CheckedStatementPayload::Assignment(assignment) => {
            let place = assignment.place();
            let field_place = place
                .nominal_field()
                .ok_or(SemanticTranscriptError::MissingIdentity)?;
            transcript_update!(
                hasher,
                &coordinates.binding(place.local_id())?.canonical_bytes()?
            );
            write_nominal(hasher, field_place.nominal(), analysis)?;
            write_field_selection(hasher, field_place.field())?;
            transcript_update!(
                hasher,
                field_place
                    .field_type()
                    .semantic_identity_digest()?
                    .as_bytes()
            );
            transcript_update!(
                hasher,
                assignment
                    .value_type()
                    .semantic_identity_digest()?
                    .as_bytes()
            );
        }
        CheckedStatementPayload::Assertion(disposition) => {
            transcript_update!(
                hasher,
                &[match disposition {
                    CheckedAssertionDisposition::PendingProof => 0,
                    CheckedAssertionDisposition::Discharged => 1,
                    CheckedAssertionDisposition::Runtime(policy) => {
                        match policy {
                            crate::assertion::AssertionRuntimePolicy::AlwaysGuard => 2,
                            crate::assertion::AssertionRuntimePolicy::DebugGuard => 3,
                        }
                    }
                    CheckedAssertionDisposition::OmittedDebug => 4,
                }]
            );
        }
        CheckedStatementPayload::Defer(defer) => {
            transcript_update!(
                hasher,
                &[match defer.outcome() {
                    arcweft_lang_syntax::ast::line_plan::DeferOutcome::Always => 0,
                    arcweft_lang_syntax::ast::line_plan::DeferOutcome::Completed => 1,
                    arcweft_lang_syntax::ast::line_plan::DeferOutcome::Cancelled => 2,
                    arcweft_lang_syntax::ast::line_plan::DeferOutcome::Failed => 3,
                }]
            );
            write_bytes(
                hasher,
                &coordinates.expression(defer.body())?.canonical_bytes()?,
            )?;
            write_len(hasher, defer.captures().len())?;
            for capture in defer.captures() {
                write_bytes(hasher, &capture.origin().canonical_bytes()?)?;
                transcript_update!(hasher, capture.ty().semantic_identity_digest()?.as_bytes());
            }
        }
        CheckedStatementPayload::EvaluatedEffect(effect) => {
            // The statement's checked expression child carries the selected
            // callable and argument meaning. The application digest is only
            // the generation-bound validation join.
            write_bytes(
                hasher,
                &coordinates
                    .expression(effect.site_root())?
                    .canonical_bytes()?,
            )?;
        }
        CheckedStatementPayload::Iteration(iteration) => {
            write_iteration(hasher, iteration, analysis)?;
        }
        CheckedStatementPayload::ControlTransfer(target) => {
            write_control_transfer(hasher, target)?;
        }
        CheckedStatementPayload::Trigger(trigger) => {
            transcript_update!(hasher, &[trigger.semantic_tag()]);
            match trigger.view() {
                super::CheckedTriggerView::Mark(coordinate) => {
                    transcript_update!(hasher, &coordinate.canonical_bytes()?);
                }
                super::CheckedTriggerView::InputAction(action) => {
                    write_len(hasher, action.as_str().len())?;
                    transcript_update!(hasher, action.as_str().as_bytes());
                }
                _ => {}
            }
        }
        CheckedStatementPayload::UnsafeAudit(audit) => {
            transcript_update!(hasher, audit.semantic_id().as_bytes());
            transcript_update!(hasher, &[u8::from(audit.has_safety_doc())]);
        }
        CheckedStatementPayload::Select(select) => {
            transcript_update!(hasher, &[select.semantic_tag()]);
            if let super::CheckedSelectStatementView::Branches(branches) = select.view() {
                write_len(hasher, branches.len())?;
                for branch in branches {
                    transcript_update!(hasher, &[branch.semantic_tag()]);
                }
            }
        }
        CheckedStatementPayload::SourceLocale(locale) => {
            transcript_update!(hasher, locale.semantic_digest().as_bytes());
        }
        CheckedStatementPayload::Scope(scope) => {
            write_scope_identity(hasher, scope, owner_coordinate)?;
        }
        CheckedStatementPayload::Include(target) => {
            transcript_update!(hasher, target.declaration().as_bytes());
        }
        CheckedStatementPayload::Suspension(suspension) => {
            transcript_update!(
                hasher,
                &[match suspension.as_ref() {
                    super::CheckedSuspensionStatement::Wait => 0,
                }]
            );
        }
        CheckedStatementPayload::Yield => {}
    }
    Ok(())
}

fn write_field_selection(
    hasher: &mut MatchTranscriptHasher<'_>,
    selection: &super::CheckedFieldSelection,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(hasher, selection.owner_type().as_bytes());
    transcript_update!(hasher, selection.field().as_bytes());
    transcript_update!(hasher, &selection.declaration_ordinal().to_le_bytes());
    transcript_update!(hasher, selection.field_type().as_bytes());
    transcript_update!(hasher, &[u8::from(selection.runtime_field().is_some())]);
    Ok(())
}

fn write_scope_identity(
    hasher: &mut MatchTranscriptHasher<'_>,
    scope: &super::CheckedScopeIdentity,
    owner_coordinate: &[u8],
) -> Result<(), SemanticTranscriptError> {
    match scope {
        super::CheckedScopeIdentity::Anonymous => transcript_update!(hasher, &[0]),
        super::CheckedScopeIdentity::Named(name) => {
            transcript_update!(hasher, &[1]);
            write_bytes(hasher, name.canonical_identity_bytes())?;
        }
    }
    write_bytes(hasher, owner_coordinate)
}

fn write_iteration(
    hasher: &mut MatchTranscriptHasher<'_>,
    iteration: &super::CheckedIteration,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    use super::CheckedIteration;
    match iteration {
        CheckedIteration::Builtin { family, item } => {
            transcript_update!(hasher, &[0, iterator_family_tag(*family)]);
            transcript_update!(hasher, item.semantic_identity_digest()?.as_bytes());
        }
        CheckedIteration::Witness {
            source,
            item,
            into_iter,
            into_iterator,
            iterator,
        } => {
            transcript_update!(hasher, &[1]);
            transcript_update!(hasher, source.semantic_identity_digest()?.as_bytes());
            transcript_update!(hasher, item.semantic_identity_digest()?.as_bytes());
            transcript_update!(hasher, into_iter.semantic_identity_digest()?.as_bytes());
            write_trait_conformance(hasher, into_iterator, analysis)?;
            write_trait_conformance(hasher, iterator, analysis)?;
        }
        CheckedIteration::IteratorWitness {
            source,
            item,
            iterator,
        } => {
            transcript_update!(hasher, &[2]);
            transcript_update!(hasher, source.semantic_identity_digest()?.as_bytes());
            transcript_update!(hasher, item.semantic_identity_digest()?.as_bytes());
            write_trait_conformance(hasher, iterator, analysis)?;
        }
    }
    Ok(())
}

const fn iterator_family_tag(family: super::CheckedIteratorFamily) -> u8 {
    match family {
        super::CheckedIteratorFamily::Range => 0,
        super::CheckedIteratorFamily::Seq => 1,
        super::CheckedIteratorFamily::Stream => 2,
        super::CheckedIteratorFamily::Vec => 3,
        super::CheckedIteratorFamily::Array => 4,
        super::CheckedIteratorFamily::Slice => 5,
    }
}

fn write_trait_conformance(
    hasher: &mut MatchTranscriptHasher<'_>,
    conformance: &super::CheckedTraitConformance,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    match conformance.trait_identity() {
        super::CheckedTraitIdentity::Project(item) => {
            transcript_update!(hasher, &[0]);
            let root = analysis
                .accepted_root_catalog()
                .item_for_hir(*item)
                .map_err(SemanticTranscriptError::AcceptedRootCatalog)?;
            transcript_update!(hasher, &[root.tag()]);
            transcript_update!(hasher, root.as_bytes());
        }
        super::CheckedTraitIdentity::StandardIterator => transcript_update!(hasher, &[1]),
        super::CheckedTraitIdentity::StandardIntoIterator => transcript_update!(hasher, &[2]),
    }
    transcript_update!(hasher, &conformance.method().to_le_bytes());
    transcript_update!(
        hasher,
        arcweft_lang_hir::symbol::CallableDeclarationKey::ImplMethod(
            conformance.declaration().clone(),
        )
        .semantic_digest()
        .as_bytes()
    );
    Ok(())
}

fn write_control_transfer(
    hasher: &mut MatchTranscriptHasher<'_>,
    target: &crate::semantic_coordinate::CheckedControlTransferTarget,
) -> Result<(), SemanticTranscriptError> {
    match target {
        crate::semantic_coordinate::CheckedControlTransferTarget::Return(boundary) => {
            transcript_update!(hasher, &[2]);
            write_callable_boundary(hasher, boundary)?;
        }
        crate::semantic_coordinate::CheckedControlTransferTarget::Output(output) => {
            transcript_update!(hasher, &[0]);
            write_bytes(hasher, &output.coordinate().canonical_bytes()?)?;
        }
        crate::semantic_coordinate::CheckedControlTransferTarget::Loop(loop_target) => {
            transcript_update!(hasher, &[1]);
            transcript_update!(
                hasher,
                &[match loop_target.family() {
                    arcweft_lang_hir::project::HirLoopTargetFamily::LoopExpression => 0,
                    arcweft_lang_hir::project::HirLoopTargetFamily::WhileStatement => 1,
                    arcweft_lang_hir::project::HirLoopTargetFamily::WhileLetStatement => 2,
                    arcweft_lang_hir::project::HirLoopTargetFamily::ForStatement => 3,
                }]
            );
            write_bytes(hasher, &loop_target.body().canonical_bytes()?)?;
        }
    }
    Ok(())
}

fn write_evaluated_effect(
    hasher: &mut MatchTranscriptHasher<'_>,
    effect: &super::CheckedEvaluatedEffect,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(
        hasher,
        &effect.application().coordinate().canonical_bytes()?
    );
    transcript_update!(
        hasher,
        effect.result().semantic_identity_digest()?.as_bytes()
    );
    write_effect_operation(hasher, effect.operation())
}

fn write_effect_operation(
    hasher: &mut MatchTranscriptHasher<'_>,
    operation: &super::CheckedEvaluatedEffectOperation,
) -> Result<(), SemanticTranscriptError> {
    use super::CheckedEvaluatedEffectOperation;
    match operation {
        CheckedEvaluatedEffectOperation::Log {
            level,
            message,
            fields,
        } => {
            transcript_update!(hasher, &[0, log_level_tag(*level)]);
            write_effect_operand(hasher, message)?;
            write_effect_fields(hasher, fields)?;
        }
        CheckedEvaluatedEffectOperation::SignalWrite { target, value } => {
            transcript_update!(hasher, &[1]);
            write_effect_operand(hasher, target)?;
            write_effect_operand(hasher, value)?;
        }
        CheckedEvaluatedEffectOperation::MetricWrite { target, value } => {
            transcript_update!(hasher, &[2]);
            write_effect_operand(hasher, target)?;
            write_effect_operand(hasher, value)?;
        }
        CheckedEvaluatedEffectOperation::EmitEvent { event, fields } => {
            transcript_update!(hasher, &[3]);
            write_effect_operand(hasher, event)?;
            write_effect_fields(hasher, fields)?;
        }
        CheckedEvaluatedEffectOperation::Panic { message } => {
            transcript_update!(hasher, &[4]);
            write_effect_operand(hasher, message)?;
        }
        CheckedEvaluatedEffectOperation::Fail { message } => {
            transcript_update!(hasher, &[5]);
            write_effect_operand(hasher, message)?;
        }
        CheckedEvaluatedEffectOperation::Bail { message } => {
            transcript_update!(hasher, &[6]);
            write_effect_operand(hasher, message)?;
        }
        CheckedEvaluatedEffectOperation::Ensure { condition, message } => {
            transcript_update!(hasher, &[7]);
            write_effect_operand(hasher, condition)?;
            write_effect_operand(hasher, message)?;
        }
        CheckedEvaluatedEffectOperation::Drop { target, invocation } => {
            transcript_update!(hasher, &[8]);
            write_effect_operand(hasher, target)?;
            match invocation {
                super::CheckedDropInvocation::Drop => transcript_update!(hasher, &[0]),
                super::CheckedDropInvocation::DropOptional => transcript_update!(hasher, &[1]),
                super::CheckedDropInvocation::DropWithPolicy { source, policy } => {
                    transcript_update!(hasher, &[2]);
                    write_effect_operand(hasher, source.operand())?;
                    write_drop_policy(hasher, policy)?;
                }
            }
        }
    }
    Ok(())
}

const fn log_level_tag(level: crate::callable::CallableLogLevel) -> u8 {
    match level {
        crate::callable::CallableLogLevel::Trace => 0,
        crate::callable::CallableLogLevel::Debug => 1,
        crate::callable::CallableLogLevel::Info => 2,
        crate::callable::CallableLogLevel::Warn => 3,
        crate::callable::CallableLogLevel::Error => 4,
    }
}

fn write_effect_fields(
    hasher: &mut MatchTranscriptHasher<'_>,
    fields: &[super::CheckedEffectField],
) -> Result<(), SemanticTranscriptError> {
    write_len(hasher, fields.len())?;
    for field in fields {
        transcript_update!(hasher, field.open_argument().semantic_digest().as_bytes());
        write_effect_operand(hasher, field.operand())?;
    }
    Ok(())
}

fn write_effect_operand(
    hasher: &mut MatchTranscriptHasher<'_>,
    operand: &super::CheckedEvaluatedEffectOperand,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(
        hasher,
        &operand.ty().semantic_identity_digest()?.as_bytes()[..]
    );
    transcript_update!(hasher, &operand.source().coordinate().canonical_bytes()?);
    Ok(())
}

fn write_drop_policy(
    hasher: &mut MatchTranscriptHasher<'_>,
    policy: &super::CheckedExplicitDropPolicy,
) -> Result<(), SemanticTranscriptError> {
    match policy {
        super::CheckedExplicitDropPolicy::Cancel => transcript_update!(hasher, &[0]),
        super::CheckedExplicitDropPolicy::Stop { fade } => {
            transcript_update!(hasher, &[1]);
            match fade {
                super::CheckedDropFade::Constant(value) => {
                    transcript_update!(hasher, &[0]);
                    transcript_update!(hasher, &value.as_nanos().to_le_bytes());
                }
                super::CheckedDropFade::Operand(value) => {
                    transcript_update!(hasher, &[1]);
                    write_effect_operand(hasher, value.operand())?;
                }
            }
        }
        super::CheckedExplicitDropPolicy::Finish => transcript_update!(hasher, &[2]),
        super::CheckedExplicitDropPolicy::Release => transcript_update!(hasher, &[3]),
        super::CheckedExplicitDropPolicy::Detach => transcript_update!(hasher, &[4]),
    }
    Ok(())
}

fn write_rich_text_token(
    hasher: &mut MatchTranscriptHasher<'_>,
    token: &CheckedDialogueToken,
    child_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    content_body_digests: &BTreeMap<ExprId, CheckedRichTextSemanticDigest>,
) -> Result<(), SemanticTranscriptError> {
    match token {
        CheckedDialogueToken::Text(value) | CheckedDialogueToken::RawLiteral(value) => {
            write_bytes(hasher, value.as_bytes())?;
        }
        CheckedDialogueToken::Escape(value) => {
            transcript_update!(hasher, &u32::from(*value).to_le_bytes());
        }
        CheckedDialogueToken::PointAction(action) => {
            write_rich_text_point_action(hasher, action, child_digests)?;
        }
        CheckedDialogueToken::Interpolation {
            expression,
            witness,
            ..
        } => {
            let digest = child_digests
                .get(expression)
                .ok_or(SemanticTranscriptError::MissingIdentity)?;
            transcript_update!(hasher, digest.as_bytes());
            transcript_update!(
                hasher,
                &[witness.as_ref().map_or(
                    3,
                    crate::checked_rich_text::CheckedDisplayWitness::semantic_tag
                )]
            );
        }
        CheckedDialogueToken::ContentInsert(insertion) => {
            let fragment_digest = insertion
                .fragment_coordinate()
                .semantic_digest()
                .map_err(|_| SemanticTranscriptError::MissingIdentity)?;
            transcript_update!(hasher, fragment_digest.as_bytes());
            let digest = child_digests
                .get(&insertion.site().raw())
                .ok_or(SemanticTranscriptError::MissingIdentity)?;
            transcript_update!(hasher, digest.as_bytes());
            match content_body_digests.get(&insertion.site().raw()) {
                Some(body_digest) => {
                    transcript_update!(hasher, &[1]);
                    transcript_update!(hasher, body_digest.as_bytes());
                }
                None => transcript_update!(hasher, &[0]),
            }
        }
        CheckedDialogueToken::LineBreak(kind) => {
            transcript_update!(
                hasher,
                &[match kind {
                    arcweft_lang_hir::dialogue_application::HirLineBreakKind::Line => 0,
                    arcweft_lang_hir::dialogue_application::HirLineBreakKind::Paragraph => 1,
                    arcweft_lang_hir::dialogue_application::HirLineBreakKind::Page => 2,
                }]
            );
        }
    }
    Ok(())
}

fn write_rich_text_point_action(
    hasher: &mut MatchTranscriptHasher<'_>,
    action: &crate::checked_rich_text::CheckedRichTextAction,
    child_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
) -> Result<(), SemanticTranscriptError> {
    match action {
        CheckedRichTextAction::Control { action, fields } => {
            transcript_update!(hasher, &[0]);
            write_checked_dialogue_control(hasher, action)?;
            write_rich_text_fields(hasher, fields, child_digests)?;
        }
        CheckedRichTextAction::Host {
            owner,
            action,
            fields,
        } => {
            transcript_update!(hasher, &[1, dialogue_host_event_tag(*owner)]);
            write_checked_host_event(hasher, action, child_digests)?;
            write_rich_text_fields(hasher, fields, child_digests)?;
        }
        CheckedRichTextAction::Marker(marker) => {
            transcript_update!(hasher, &[2]);
            transcript_update!(hasher, &marker.coordinate().canonical_bytes()?);
        }
    }
    Ok(())
}

fn write_checked_host_event(
    hasher: &mut MatchTranscriptHasher<'_>,
    action: &crate::checked_rich_text::CheckedDialogueHostEvent,
    child_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
) -> Result<(), SemanticTranscriptError> {
    use crate::checked_rich_text::CheckedDialogueHostEvent;
    match action {
        CheckedDialogueHostEvent::Voice { source } => {
            transcript_update!(hasher, &[0]);
            match source {
                crate::checked_rich_text::CheckedVoiceSource::Auto => {
                    transcript_update!(hasher, &[0])
                }
                crate::checked_rich_text::CheckedVoiceSource::Identity(id) => {
                    transcript_update!(hasher, &[1]);
                    write_bytes(hasher, id.canonical_identity_bytes())?;
                }
            }
        }
        CheckedDialogueHostEvent::Face { expression } => {
            transcript_update!(hasher, &[1]);
            write_bytes(hasher, expression.canonical_identity_bytes())?;
        }
        CheckedDialogueHostEvent::Pose { pose } => {
            transcript_update!(hasher, &[2]);
            write_bytes(hasher, pose.canonical_identity_bytes())?;
        }
        CheckedDialogueHostEvent::Show { entity } => {
            transcript_update!(hasher, &[3]);
            write_bytes(hasher, entity.canonical_identity_bytes())?;
        }
        CheckedDialogueHostEvent::Hide { entity } => {
            transcript_update!(hasher, &[4]);
            write_bytes(hasher, entity.canonical_identity_bytes())?;
        }
        CheckedDialogueHostEvent::Move { x, y } => {
            transcript_update!(hasher, &[5]);
            write_checked_length(hasher, *x)?;
            write_checked_length(hasher, *y)?;
        }
        CheckedDialogueHostEvent::Scale { x, y } => {
            transcript_update!(hasher, &[6]);
            transcript_update!(hasher, &x.0.to_le_bytes());
            transcript_update!(hasher, &y.0.to_le_bytes());
        }
        CheckedDialogueHostEvent::Rotate { angle } => {
            transcript_update!(hasher, &[7]);
            transcript_update!(hasher, &angle.milli_degrees.to_le_bytes());
        }
        CheckedDialogueHostEvent::Animation { animation } => {
            transcript_update!(hasher, &[8]);
            write_bytes(hasher, animation.canonical_identity_bytes())?;
        }
        CheckedDialogueHostEvent::Shake { amplitude } => {
            transcript_update!(hasher, &[9]);
            write_checked_length(hasher, *amplitude)?;
        }
        CheckedDialogueHostEvent::TimedCue { at, call } => {
            transcript_update!(hasher, &[10]);
            transcript_update!(hasher, &at.millis.to_le_bytes());
            write_child_expression_digest(hasher, child_digests, *call)?;
        }
        CheckedDialogueHostEvent::Call { call } => {
            transcript_update!(hasher, &[11]);
            write_child_expression_digest(hasher, child_digests, *call)?;
        }
        CheckedDialogueHostEvent::Signal { signal } => {
            transcript_update!(hasher, &[12]);
            write_bytes(hasher, signal.canonical_identity_bytes())?;
        }
    }
    Ok(())
}

fn write_rich_text_fields(
    hasher: &mut MatchTranscriptHasher<'_>,
    fields: &crate::checked_rich_text::CheckedOwnerFields,
    child_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
) -> Result<(), SemanticTranscriptError> {
    write_len(hasher, fields.fields().len())?;
    for field in fields.fields() {
        write_rich_text_property(hasher, field.property())?;
        write_checked_rich_text_value(hasher, field.value())?;
        match field.origin() {
            CheckedFieldOrigin::Authored { .. } => transcript_update!(hasher, &[0]),
            CheckedFieldOrigin::Defaulted { default_id } => {
                transcript_update!(hasher, &[1]);
                transcript_update!(hasher, &default_id.get().to_le_bytes());
            }
            CheckedFieldOrigin::TextProxyDefault { expression } => {
                transcript_update!(hasher, &[2]);
                write_child_expression_digest(hasher, child_digests, *expression)?;
            }
        }
    }
    Ok(())
}

fn write_child_expression_digest(
    hasher: &mut MatchTranscriptHasher<'_>,
    child_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    expression: ExprId,
) -> Result<(), SemanticTranscriptError> {
    let digest = child_digests
        .get(&expression)
        .ok_or(SemanticTranscriptError::MissingIdentity)?;
    transcript_update!(hasher, digest.as_bytes());
    Ok(())
}

/// Writes a resolution-owned expression reference.  The ordinary expression
/// digest is sufficient outside an owner-bound body.  Inside one, references
/// to the enclosing expression itself are represented by its accepted
/// coordinate, which closes the body-resolution cycle without reopening raw
/// HIR lookup identity.
fn write_resolution_child_digest(
    hasher: &mut MatchTranscriptHasher<'_>,
    child_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    cycle_owner: Option<ExprId>,
    expression: ExprId,
) -> Result<(), SemanticTranscriptError> {
    if cycle_owner.is_some() {
        if cycle_owner == Some(expression) {
            transcript_update!(hasher, &[1]);
            write_bytes(
                hasher,
                &coordinates.expression(expression)?.canonical_bytes()?,
            )?;
        } else {
            transcript_update!(hasher, &[0]);
            write_child_expression_digest(hasher, child_digests, expression)?;
        }
    } else {
        write_child_expression_digest(hasher, child_digests, expression)?;
    }
    Ok(())
}

fn write_checked_dialogue_control(
    hasher: &mut MatchTranscriptHasher<'_>,
    action: &crate::checked_rich_text::CheckedDialogueControl,
) -> Result<(), SemanticTranscriptError> {
    use crate::checked_rich_text::CheckedDialogueControl;
    match action {
        CheckedDialogueControl::Page => transcript_update!(hasher, &[0]),
        CheckedDialogueControl::LineWait => transcript_update!(hasher, &[1]),
        CheckedDialogueControl::HardBreak => transcript_update!(hasher, &[2]),
        CheckedDialogueControl::TimedWait { duration } => {
            transcript_update!(hasher, &[3]);
            transcript_update!(hasher, &duration.millis.to_le_bytes());
        }
        CheckedDialogueControl::Clear => transcript_update!(hasher, &[4]),
        CheckedDialogueControl::Reset => transcript_update!(hasher, &[5]),
        CheckedDialogueControl::RevealRate { milli_cps } => {
            transcript_update!(hasher, &[6]);
            transcript_update!(hasher, &milli_cps.0.to_le_bytes());
        }
    }
    Ok(())
}

fn write_checked_length(
    hasher: &mut MatchTranscriptHasher<'_>,
    value: crate::checked_rich_text::CheckedLength,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(hasher, &value.milli.to_le_bytes());
    transcript_update!(
        hasher,
        &[match value.unit {
            crate::checked_rich_text::LengthUnit::Px => 0,
            crate::checked_rich_text::LengthUnit::Pt => 1,
            crate::checked_rich_text::LengthUnit::Ch => 2,
            crate::checked_rich_text::LengthUnit::Em => 3,
        }]
    );
    Ok(())
}

fn write_checked_enum(
    hasher: &mut MatchTranscriptHasher<'_>,
    value: arcweft_id::closed_enum::ClosedEnumValueId,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(hasher, &value.domain().canonical_bytes());
    transcript_update!(hasher, &value.variant().to_le_bytes());
    Ok(())
}

fn write_checked_color(
    hasher: &mut MatchTranscriptHasher<'_>,
    value: &crate::checked_rich_text::CheckedColor,
) -> Result<(), SemanticTranscriptError> {
    match value {
        crate::checked_rich_text::CheckedColor::Rgba8(rgba) => {
            transcript_update!(hasher, &[0]);
            transcript_update!(hasher, rgba);
        }
        crate::checked_rich_text::CheckedColor::Resource(resource) => {
            transcript_update!(hasher, &[1]);
            write_bytes(hasher, resource.canonical_identity_bytes())?;
        }
    }
    Ok(())
}

fn write_checked_rich_text_value(
    hasher: &mut MatchTranscriptHasher<'_>,
    value: &CheckedRichTextValue,
) -> Result<(), SemanticTranscriptError> {
    match value {
        CheckedRichTextValue::Bool(value) => {
            transcript_update!(hasher, &[0, u8::from(*value)]);
        }
        CheckedRichTextValue::Int(value) => {
            transcript_update!(hasher, &[1]);
            transcript_update!(hasher, &value.to_le_bytes());
        }
        CheckedRichTextValue::Milli(value) => {
            transcript_update!(hasher, &[2]);
            transcript_update!(hasher, &value.0.to_le_bytes());
        }
        CheckedRichTextValue::Ratio(value) => {
            transcript_update!(hasher, &[3]);
            transcript_update!(hasher, &value.0.to_le_bytes());
        }
        CheckedRichTextValue::Length(value) => {
            transcript_update!(hasher, &[4]);
            write_checked_length(hasher, *value)?;
        }
        CheckedRichTextValue::Angle(value) => {
            transcript_update!(hasher, &[5]);
            transcript_update!(hasher, &value.milli_degrees.to_le_bytes());
        }
        CheckedRichTextValue::Duration(value) => {
            transcript_update!(hasher, &[6]);
            transcript_update!(hasher, &value.millis.to_le_bytes());
        }
        CheckedRichTextValue::Enum(value) => {
            transcript_update!(hasher, &[7]);
            write_checked_enum(hasher, *value)?;
        }
        CheckedRichTextValue::PublicId(value) => {
            transcript_update!(hasher, &[8]);
            write_bytes(hasher, value.canonical_identity_bytes())?;
        }
        CheckedRichTextValue::Text(value) => {
            transcript_update!(hasher, &[9]);
            write_bytes(hasher, value.as_bytes())?;
        }
        CheckedRichTextValue::Color(value) => {
            transcript_update!(hasher, &[10]);
            write_checked_color(hasher, value)?;
        }
        CheckedRichTextValue::Vec2(value) => {
            transcript_update!(hasher, &[11]);
            transcript_update!(hasher, &value.x.0.to_le_bytes());
            transcript_update!(hasher, &value.y.0.to_le_bytes());
        }
        CheckedRichTextValue::Seed(value) => {
            transcript_update!(hasher, &[12]);
            transcript_update!(hasher, &value.0.to_le_bytes());
        }
    }
    Ok(())
}

fn write_rich_text_property(
    hasher: &mut MatchTranscriptHasher<'_>,
    property: CheckedRichTextProperty,
) -> Result<(), SemanticTranscriptError> {
    match property {
        CheckedRichTextProperty::Control(value) => {
            transcript_update!(hasher, &[0, dialogue_control_property_tag(value)]);
        }
        CheckedRichTextProperty::Host(value) => {
            transcript_update!(hasher, &[1, dialogue_host_property_tag(value)]);
        }
    }
    Ok(())
}

const fn dialogue_control_property_tag(value: DialogueControlProperty) -> u8 {
    match value {
        DialogueControlProperty::Time => 0,
        DialogueControlProperty::Cps => 1,
    }
}

const fn dialogue_host_event_tag(value: DialogueHostEventKind) -> u8 {
    match value {
        DialogueHostEventKind::Voice => 0,
        DialogueHostEventKind::Face => 1,
        DialogueHostEventKind::Pose => 2,
        DialogueHostEventKind::Show => 3,
        DialogueHostEventKind::Hide => 4,
        DialogueHostEventKind::Move => 5,
        DialogueHostEventKind::Scale => 6,
        DialogueHostEventKind::Rotate => 7,
        DialogueHostEventKind::Animation => 8,
        DialogueHostEventKind::Shake => 9,
        DialogueHostEventKind::TimedCue => 10,
        DialogueHostEventKind::Call => 11,
        DialogueHostEventKind::Signal => 12,
    }
}

const fn dialogue_host_property_tag(value: DialogueHostProperty) -> u8 {
    match value {
        DialogueHostProperty::Source => 0,
        DialogueHostProperty::Expression => 1,
        DialogueHostProperty::Pose => 2,
        DialogueHostProperty::Entity => 3,
        DialogueHostProperty::X => 4,
        DialogueHostProperty::Y => 5,
        DialogueHostProperty::Angle => 6,
        DialogueHostProperty::Animation => 7,
        DialogueHostProperty::Amp => 8,
        DialogueHostProperty::At => 9,
        DialogueHostProperty::Call => 10,
        DialogueHostProperty::Signal => 11,
    }
}

fn write_record_expression_fields(
    hasher: &mut MatchTranscriptHasher<'_>,
    edges: &super::CheckedExpressionEdgeFact,
) -> Result<(), SemanticTranscriptError> {
    write_len(hasher, edges.record_fields().len())?;
    for field in edges.record_fields() {
        transcript_update!(hasher, &field.source_ordinal().to_le_bytes());
        transcript_update!(hasher, &field.declaration_ordinal().to_le_bytes());
        transcript_update!(hasher, field.semantic_id().as_bytes());
        transcript_update!(hasher, field.field_type().as_bytes());
        match field.source() {
            super::CheckedRecordValueSource::Expression(source) => {
                transcript_update!(hasher, &[0]);
                transcript_update!(hasher, &source.coordinate().canonical_bytes()?);
            }
            super::CheckedRecordValueSource::Binding(source) => {
                transcript_update!(hasher, &[1]);
                transcript_update!(hasher, &source.coordinate().canonical_bytes()?);
            }
        }
    }
    Ok(())
}

fn guard_class(
    analysis: &FinalSemanticAnalysis,
    owner: ExprId,
) -> Result<CheckedGuardClass, SemanticTranscriptError> {
    let guard = analysis
        .expression(owner)
        .ok_or(SemanticTranscriptError::MissingExpression)?;
    Ok(match guard.resolution() {
        CheckedExpressionResolution::Literal(HirLiteral::Boolean(true)) => {
            CheckedGuardClass::ConstantTrue
        }
        CheckedExpressionResolution::Literal(HirLiteral::Boolean(false)) => {
            CheckedGuardClass::ConstantFalse
        }
        _ => CheckedGuardClass::Dynamic,
    })
}

fn match_digest(
    budget: &mut CheckedMatchBudget,
    scrutinee: CheckedExpressionSemanticDigest,
    scrutinee_type: SemanticTypeDigest,
    arms: &[CheckedMatchArm],
    coverage: &CheckedMatchCoverage,
) -> Result<MatchSemanticTranscript, SemanticTranscriptError> {
    let mut hasher = TranscriptHasher::new(budget);
    transcript_update!(hasher, b"arcweft.lang.checked-match-semantic.v1\0");
    transcript_update!(hasher, scrutinee.as_bytes());
    transcript_update!(hasher, scrutinee_type.as_bytes());
    write_len(&mut hasher, arms.len())?;
    for arm in arms {
        transcript_update!(hasher, &arm.coordinate.ordinal().to_le_bytes());
        transcript_update!(hasher, arm.pattern.as_bytes());
        write_len(&mut hasher, arm.bindings.len())?;
        for binding in &arm.bindings {
            transcript_update!(hasher, &binding.coordinate.canonical_bytes()?);
            transcript_update!(hasher, binding.ty.as_bytes());
        }
        match arm.guard.expression() {
            Some(digest) => {
                transcript_update!(hasher, &[1]);
                transcript_update!(hasher, digest.as_bytes());
                transcript_update!(hasher, &[guard_tag(arm.guard.class())]);
            }
            None => {
                transcript_update!(hasher, &[0]);
            }
        }
        transcript_update!(hasher, arm.result.as_bytes());
    }
    transcript_update!(hasher, &[u8::from(coverage.exhaustive())]);
    transcript_update!(hasher, coverage.domain_digest().as_bytes());
    write_len(&mut hasher, coverage.unreachable().len())?;
    for row in coverage.unreachable() {
        transcript_update!(hasher, &row.arm().owner().canonical_bytes()?);
        transcript_update!(hasher, &row.arm().ordinal().to_le_bytes());
        match row.alternative() {
            Some(alternative) => {
                transcript_update!(hasher, &[1]);
                transcript_update!(hasher, &alternative.canonical_bytes()?);
            }
            None => transcript_update!(hasher, &[0]),
        }
        transcript_update!(hasher, &[unreachable_tag(row.reason())]);
    }
    let byte_len = hasher.byte_len();
    Ok(MatchSemanticTranscript {
        version: MatchSemanticTranscript::VERSION,
        digest: CheckedMatchSemanticDigest::from_bytes(hasher.finalize()),
        byte_len,
    })
}

fn child_pattern_coordinate(
    parent: &StablePatternCoordinate,
    owner: &HirPatternKind,
    role: HirPatternChildRole,
) -> Result<StablePatternCoordinate, SemanticTranscriptError> {
    let mut steps = parent.steps().to_vec();
    let next = match role {
        HirPatternChildRole::VariantPayload => StablePatternCoordinateStep::VariantPayload,
        HirPatternChildRole::Element { ordinal } => match owner {
            HirPatternKind::BracketSequence { .. } => {
                StablePatternCoordinateStep::SequenceElement(ordinal)
            }
            _ => StablePatternCoordinateStep::TupleElement(ordinal),
        },
        HirPatternChildRole::RecordField { .. } => {
            // The current checked pattern fact does not retain the accepted
            // record-field identity for this child. Re-resolving it through
            // authored field spelling would make a semantic transcript
            // source-dependent, so this family remains fail-closed until the
            // exact checked field row is added to the pattern authority.
            return Err(SemanticTranscriptError::MissingIdentity);
        }
        HirPatternChildRole::NestedPattern => StablePatternCoordinateStep::WholeBindingInner,
        HirPatternChildRole::OrAlternative { ordinal } => {
            StablePatternCoordinateStep::OrAlternative(ordinal)
        }
        HirPatternChildRole::TypedBindingType
        | HirPatternChildRole::BindingLocal
        | HirPatternChildRole::MutableBindingLocal
        | HirPatternChildRole::RecordShorthandLocal { .. }
        | HirPatternChildRole::RecordRestLocal { .. }
        | HirPatternChildRole::SequenceRestLocal
        | HirPatternChildRole::WholeBindingLocal
        | HirPatternChildRole::TypedBindingLocal => StablePatternCoordinateStep::TypedBindingInner,
    };
    steps.push(next);
    Ok(StablePatternCoordinate::new(steps))
}

fn record_pattern_child_coordinate(
    parent: &StablePatternCoordinate,
    field: &super::CheckedRecordPatternField,
) -> Result<StablePatternCoordinate, SemanticTranscriptError> {
    let relative = field
        .source()
        .pattern_coordinate()
        .ok_or(SemanticTranscriptError::MissingIdentity)?;
    let mut steps = parent.steps().to_vec();
    steps.extend_from_slice(relative.steps());
    Ok(StablePatternCoordinate::new(steps))
}

fn write_literal(
    hasher: &mut MatchTranscriptHasher<'_>,
    literal: &HirLiteral,
    ty: &TypeKind,
) -> Result<(), SemanticTranscriptError> {
    super::match_coverage::encode_canonical_literal(literal, ty, |bytes| hasher.update(bytes))
        .map_err(map_canonical_literal_error)
}

fn map_canonical_literal_error(
    error: super::match_coverage::CanonicalLiteralEncodingError<CheckedMatchBuildError>,
) -> SemanticTranscriptError {
    match error {
        super::match_coverage::CanonicalLiteralEncodingError::Invalid => {
            SemanticTranscriptError::RecoveredOwner
        }
        super::match_coverage::CanonicalLiteralEncodingError::ArithmeticOverflow => {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::TranscriptBytes,
            }
            .into()
        }
        super::match_coverage::CanonicalLiteralEncodingError::Sink(error) => error.into(),
    }
}

/// Writes only HIR-owned non-child meaning. Checked resolution, type, field,
/// callable, and body facts are written by their existing owners.
fn write_expression_shape_atoms(
    hasher: &mut MatchTranscriptHasher<'_>,
    owner: ExprId,
    kind: &HirExprKind,
    checked: &super::CheckedExpression,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    match kind {
        HirExprKind::Unit
        | HirExprKind::Literal(_)
        | HirExprKind::EntityReference(_)
        | HirExprKind::LifetimePath(_)
        | HirExprKind::Path(_)
        | HirExprKind::ShortVariant(_)
        | HirExprKind::ArrayRepeat(_)
        | HirExprKind::Select(_)
        | HirExprKind::Index(_)
        | HirExprKind::Pipe(_)
        | HirExprKind::Try(_)
        | HirExprKind::Record(_)
        | HirExprKind::RecordLiteral(_)
        | HirExprKind::Dereference(_)
        | HirExprKind::Closure(_)
        | HirExprKind::Block(_)
        | HirExprKind::NamedBlock(_)
        | HirExprKind::Loop(_)
        | HirExprKind::If(_)
        | HirExprKind::IfLet(_)
        | HirExprKind::Match(_)
        | HirExprKind::PostfixBracket(_) => {}
        HirExprKind::Placeholder(placeholder) => {
            transcript_update!(hasher, &[placeholder.semantic_transcript_tag()]);
        }
        HirExprKind::Tuple(tuple) => write_len(hasher, tuple.elements().len())?,
        HirExprKind::BracketSequence(sequence) => write_len(hasher, sequence.elements().len())?,
        HirExprKind::NumericBracketSequence(sequence) => {
            if !matches!(sequence.recovery(), HirNumericSequenceRecovery::Complete) {
                return Err(SemanticTranscriptError::RecoveredOwner);
            }
            let ty = checked.value_type().ok_or_else(|| {
                SemanticTranscriptError::from(
                    super::FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                )
            })?;
            let item = match ty {
                TypeKind::Vec(item) => item.as_ref(),
                TypeKind::Array {
                    item,
                    len: ArrayLength::Const(length),
                } if *length == sequence.elements().len() => item.as_ref(),
                _ => return Err(SemanticTranscriptError::MissingIdentity),
            };
            if !super::type_rules::is_integer(item) {
                return Err(SemanticTranscriptError::MissingIdentity);
            }
            write_len(hasher, sequence.elements().len())?;
            for element in sequence.elements() {
                super::canonical_literal::encode_canonical_integer_magnitude(
                    element.magnitude(),
                    &mut |bytes| hasher.update(bytes),
                )
                .map_err(map_canonical_literal_error)?;
            }
        }
        HirExprKind::Choice(choice) => {
            write_len(hasher, choice.body().items().len())?;
            for item in choice.body().items() {
                let HirChoiceItem::CompactArm(arm) = item else {
                    return Err(SemanticTranscriptError::RecoveredOwner);
                };
                transcript_update!(hasher, &[0]);
                transcript_update!(
                    hasher,
                    &[match arm.action() {
                        HirChoiceCompactAction::Goto(_) => 0,
                        HirChoiceCompactAction::Out(_) => 1,
                        HirChoiceCompactAction::Missing => {
                            return Err(SemanticTranscriptError::RecoveredOwner);
                        }
                    }],
                );
            }
        }
        HirExprKind::Call(call) => {
            transcript_update!(hasher, &[call.form().semantic_transcript_tag()]);
            write_call_type_application(hasher, call, analysis)?;
        }
        HirExprKind::AttachedContentApplication(application) => {
            if let Some(call) = application.family().invocation() {
                write_call_type_application(hasher, call, analysis)?;
            }
        }
        HirExprKind::Await(awaited) => {
            write_len(hasher, awaited.branches().len())?;
            for branch in awaited.branches() {
                transcript_update!(
                    hasher,
                    &[branch
                        .kind()
                        .semantic_transcript_tag()
                        .ok_or(SemanticTranscriptError::RecoveredOwner)?],
                );
            }
        }
        HirExprKind::Thread(thread) => {
            transcript_update!(hasher, &[thread.mode().semantic_transcript_tag()]);
        }
        HirExprKind::Range(range) => {
            transcript_update!(
                hasher,
                &[
                    u8::from(range.inclusive()),
                    u8::from(range.start().is_some()),
                    u8::from(range.end().is_some()),
                ],
            );
        }
        HirExprKind::Binary(binary) => {
            transcript_update!(hasher, &[binary.operator().semantic_transcript_tag()]);
        }
        HirExprKind::Borrow(borrow) => {
            transcript_update!(hasher, &[borrow.kind().semantic_transcript_tag()]);
        }
        HirExprKind::Unary(unary) => {
            transcript_update!(hasher, &[unary.operator().semantic_transcript_tag()]);
        }
        HirExprKind::ComputationBlock(block) => {
            transcript_update!(hasher, &[block.kind().semantic_transcript_tag()]);
        }
        HirExprKind::ForSynthetic(synthetic) => {
            transcript_update!(hasher, &[synthetic.semantic_transcript_tag()]);
        }
        HirExprKind::Error(_) => return Err(SemanticTranscriptError::RecoveredOwner),
    }
    Ok(())
}

/// The invocation owns whether type arguments were written and their order;
/// final checked types own their meaning. Source syntax and raw `TypeId` values do not
/// enter the transcript.
fn write_call_type_application(
    hasher: &mut MatchTranscriptHasher<'_>,
    call: &HirCallInvocation,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    match call.explicit_type_application() {
        HirCallTypeApplication::Absent => transcript_update!(hasher, &[0]),
        HirCallTypeApplication::Present {
            arguments,
            terminator: HirCallTypeApplicationTerminator::Closed,
            ..
        } => {
            transcript_update!(hasher, &[1]);
            write_len(hasher, arguments.len())?;
            for argument in arguments {
                let HirCallTypeArgument::Resolved { ty } = argument else {
                    return Err(SemanticTranscriptError::RecoveredOwner);
                };
                let checked = analysis
                    .ty(*ty)
                    .ok_or(SemanticTranscriptError::MissingIdentity)?;
                transcript_update!(hasher, checked.semantic_identity_digest()?.as_bytes());
            }
        }
        HirCallTypeApplication::Present { .. } => {
            return Err(SemanticTranscriptError::RecoveredOwner);
        }
    }
    Ok(())
}

fn write_complete_resolution_payload(
    hasher: &mut MatchTranscriptHasher<'_>,
    owner: ExprId,
    resolution: &CheckedExpressionResolution,
    ty: Option<&TypeKind>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    analysis: &FinalSemanticAnalysis,
    expression_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    pattern_digests: &BTreeMap<PatternId, CheckedPatternSemanticDigest>,
    match_products: &BTreeMap<ExprId, CheckedMatch>,
    content_body_digest: Option<&CheckedRichTextSemanticDigest>,
    cycle_owner: Option<ExprId>,
) -> Result<(), SemanticTranscriptError> {
    let owner_coordinate = coordinates.expression(owner)?.canonical_bytes()?;
    transcript_update!(hasher, &resolution.semantic_tag().to_le_bytes());
    if let CheckedExpressionResolution::Literal(literal) = resolution {
        write_literal(
            hasher,
            literal,
            ty.ok_or_else(|| {
                SemanticTranscriptError::from(
                    super::FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                )
            })?,
        )?;
    }
    write_resolution_payload(
        hasher,
        owner,
        &owner_coordinate,
        resolution,
        ty,
        coordinates,
        analysis,
        expression_digests,
        pattern_digests,
        match_products,
        content_body_digest,
        cycle_owner,
    )
}

fn write_resolution_payload(
    hasher: &mut MatchTranscriptHasher<'_>,
    owner: ExprId,
    owner_coordinate: &[u8],
    resolution: &CheckedExpressionResolution,
    ty: Option<&TypeKind>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    analysis: &FinalSemanticAnalysis,
    expression_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    pattern_digests: &BTreeMap<PatternId, CheckedPatternSemanticDigest>,
    match_products: &BTreeMap<ExprId, CheckedMatch>,
    content_body_digest: Option<&CheckedRichTextSemanticDigest>,
    cycle_owner: Option<ExprId>,
) -> Result<(), SemanticTranscriptError> {
    match resolution {
        CheckedExpressionResolution::Structural
        | CheckedExpressionResolution::Literal(_)
        | CheckedExpressionResolution::Call => {}
        CheckedExpressionResolution::Scope(scope) => {
            write_scope_identity(hasher, scope, owner_coordinate)?;
        }
        CheckedExpressionResolution::Value(value) => {
            write_value_resolution(hasher, value, coordinates, analysis, owner, ty)?;
        }
        CheckedExpressionResolution::Select(select) => {
            transcript_update!(hasher, &select.semantic_transcript_tag().to_le_bytes());
            match select {
                CheckedSelectResolution::Field(selection) => {
                    let receiver = selection.receiver();
                    let selection = selection.selection();
                    match receiver {
                        super::CheckedFieldReceiver::Binding(local) => {
                            transcript_update!(hasher, &[0]);
                            write_bytes(hasher, &coordinates.binding(local)?.canonical_bytes()?)?;
                        }
                        super::CheckedFieldReceiver::Expression(expression) => {
                            transcript_update!(hasher, &[1]);
                            write_bytes(
                                hasher,
                                &coordinates.expression(expression)?.canonical_bytes()?,
                            )?;
                        }
                    }
                    transcript_update!(hasher, selection.owner_type().as_bytes());
                    transcript_update!(hasher, selection.field().as_bytes());
                    transcript_update!(hasher, &selection.declaration_ordinal().to_le_bytes());
                    transcript_update!(hasher, selection.field_type().as_bytes());
                }
                CheckedSelectResolution::ProgressField { field } => {
                    transcript_update!(hasher, &[field.semantic_tag()]);
                }
                CheckedSelectResolution::Method(method) => {
                    transcript_update!(hasher, method.transcript_callable().as_bytes());
                    transcript_update!(hasher, method.receiver_type().as_bytes());
                    match method.receiver_mode() {
                        crate::callable::CallableReceiverMode::None => {
                            return Err(SemanticTranscriptError::MissingIdentity);
                        }
                        crate::callable::CallableReceiverMode::Value { .. } => {
                            transcript_update!(hasher, &[0]);
                        }
                        crate::callable::CallableReceiverMode::Type { .. } => {
                            transcript_update!(hasher, &[1]);
                        }
                        crate::callable::CallableReceiverMode::Extension {
                            group,
                            parameter,
                            ..
                        } => {
                            transcript_update!(hasher, &[2]);
                            transcript_update!(
                                hasher,
                                &u64::try_from(group.get())
                                    .map_err(|_| SemanticTranscriptError::MissingIdentity)?
                                    .to_le_bytes(),
                            );
                            transcript_update!(
                                hasher,
                                &u64::try_from(parameter.get())
                                    .map_err(|_| SemanticTranscriptError::MissingIdentity)?
                                    .to_le_bytes(),
                            );
                        }
                    }
                }
                CheckedSelectResolution::DialogueView { projection, field } => {
                    transcript_update!(hasher, &[projection.semantic_tag()]);
                    write_field_selection(hasher, field)?;
                }
                CheckedSelectResolution::AgentField { field } => {
                    write_agent_field(hasher, *field)?;
                }
            }
        }
        CheckedExpressionResolution::Nominal(nominal) => {
            write_nominal(hasher, nominal, analysis)?;
        }
        CheckedExpressionResolution::Variant(variant) => {
            write_variant_resolution(hasher, variant)?;
        }
        CheckedExpressionResolution::CompileTimeEnum(value) => {
            transcript_update!(hasher, &value.domain().canonical_bytes());
            transcript_update!(hasher, &value.variant().to_le_bytes());
        }
        CheckedExpressionResolution::Effect(effect) => {
            transcript_update!(hasher, effect.semantic_digest().as_bytes());
        }
        CheckedExpressionResolution::StageLook(look) => {
            transcript_update!(hasher, look.character_nominal().as_bytes());
            transcript_update!(hasher, look.look().as_bytes());
        }
        CheckedExpressionResolution::CompileTimeCallee(callee) => {
            write_compile_time_callee(hasher, callee)?;
        }
        CheckedExpressionResolution::TypeValue(value) => {
            write_type_value_payload(hasher, value)?;
        }
        CheckedExpressionResolution::ContentApplication(application) => {
            write_content_application_resolution(hasher, application)?;
        }
        CheckedExpressionResolution::ViewFxApplication(application) => {
            write_checked_view_fx_application(hasher, application)?;
        }
        CheckedExpressionResolution::CompileTimeScalar(scalar) => {
            write_compile_time_scalar(
                hasher,
                owner,
                scalar,
                ty.ok_or_else(|| {
                    SemanticTranscriptError::from(
                        super::FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                    )
                })?,
                coordinates,
                analysis,
                expression_digests,
                pattern_digests,
                match_products,
                content_body_digest,
                cycle_owner,
            )?;
        }
        CheckedExpressionResolution::Await(awaited) => {
            transcript_update!(hasher, &[0]);
            write_resolution_child_digest(
                hasher,
                expression_digests,
                coordinates,
                cycle_owner,
                awaited.operand(),
            )?;
            write_len(hasher, awaited.observers().len())?;
            for observer in awaited.observers() {
                transcript_update!(
                    hasher,
                    &coordinates.pattern(observer.pattern())?.canonical_bytes()?
                );
            }
        }
        CheckedExpressionResolution::Choice(choice) => {
            transcript_update!(hasher, &[1]);
            match choice.public_id() {
                Some(id) => {
                    transcript_update!(hasher, &[1]);
                    write_bytes(hasher, id.canonical_identity_bytes())?;
                }
                None => transcript_update!(hasher, &[0]),
            }
            write_len(hasher, choice.option_ids().len())?;
            for id in choice.option_ids() {
                write_bytes(hasher, id.canonical_identity_bytes())?;
            }
            write_len(hasher, choice.gotos().len())?;
            for goto in choice.gotos() {
                transcript_update!(hasher, &goto.arm().to_le_bytes());
                transcript_update!(hasher, goto.target().semantic_id().as_bytes());
                transcript_update!(hasher, goto.target().value_type().as_bytes());
            }
            match choice.plan() {
                Some(plan) => {
                    transcript_update!(hasher, &[1]);
                    write_len(hasher, plan.items().len())?;
                    for item in plan.items() {
                        transcript_update!(hasher, &[item.semantic_tag()]);
                        match item {
                            super::CheckedChoicePlanItem::Assignment(key) => {
                                transcript_update!(hasher, &[key.semantic_tag()]);
                            }
                            super::CheckedChoicePlanItem::Cancel(trigger) => {
                                if matches!(
                                    trigger.view(),
                                    super::CheckedTriggerView::InputAction(_)
                                        | super::CheckedTriggerView::Mark(_)
                                ) {
                                    return Err(SemanticTranscriptError::MissingIdentity);
                                }
                                transcript_update!(hasher, &[trigger.semantic_tag()]);
                            }
                            super::CheckedChoicePlanItem::Timeout
                            | super::CheckedChoicePlanItem::OnSelect => {}
                        }
                    }
                }
                None => transcript_update!(hasher, &[0]),
            }
        }
        CheckedExpressionResolution::Try(checked_try) => {
            transcript_update!(hasher, &[2]);
            write_try_carrier(hasher, checked_try.carrier())?;
            write_try_boundary(hasher, checked_try.boundary())?;
        }
        CheckedExpressionResolution::ImplicitCallable(callable) => {
            transcript_update!(hasher, &[3]);
            transcript_update!(hasher, callable.identity().as_bytes());
            transcript_update!(hasher, callable.function_type().as_bytes());
            transcript_update!(
                hasher,
                callable.parameter().semantic_identity_digest()?.as_bytes(),
            );
            transcript_update!(
                hasher,
                callable.result().semantic_identity_digest()?.as_bytes(),
            );
            write_len(hasher, callable.parameter_occurrences().len())?;
            for occurrence in callable.parameter_occurrences() {
                transcript_update!(hasher, &occurrence.ordinal().to_le_bytes());
                write_bytes(hasher, &occurrence.coordinate().canonical_bytes()?)?;
            }
            write_len(hasher, callable.capture_occurrences().len())?;
            for occurrence in callable.capture_occurrences() {
                transcript_update!(hasher, &occurrence.ordinal().to_le_bytes());
                write_bytes(hasher, &occurrence.coordinate().canonical_bytes()?)?;
                write_bytes(hasher, &occurrence.origin().canonical_bytes()?)?;
                transcript_update!(hasher, occurrence.value_type().as_bytes());
                transcript_update!(hasher, &[capture_access_tag(occurrence.access())]);
            }
            write_len(hasher, callable.captures().len())?;
            for capture in callable.captures() {
                write_bytes(hasher, &capture.origin().canonical_bytes()?)?;
                transcript_update!(hasher, capture.value_type().as_bytes());
                transcript_update!(hasher, &[capture_access_tag(capture.mode())]);
            }
            match callable.body() {
                super::CheckedImplicitCallableBody::Plain(resolution) => {
                    write_complete_resolution_payload(
                        hasher,
                        owner,
                        resolution,
                        Some(callable.result()),
                        coordinates,
                        analysis,
                        expression_digests,
                        pattern_digests,
                        match_products,
                        content_body_digest,
                        Some(owner),
                    )?;
                }
                super::CheckedImplicitCallableBody::Try(tried) => {
                    let resolution = CheckedExpressionResolution::Try(tried.clone());
                    write_complete_resolution_payload(
                        hasher,
                        owner,
                        &resolution,
                        Some(callable.result()),
                        coordinates,
                        analysis,
                        expression_digests,
                        pattern_digests,
                        match_products,
                        content_body_digest,
                        Some(owner),
                    )?;
                }
                super::CheckedImplicitCallableBody::Pipe(pipe) => {
                    let resolution = CheckedExpressionResolution::Pipe(pipe.clone());
                    write_complete_resolution_payload(
                        hasher,
                        owner,
                        &resolution,
                        Some(callable.result()),
                        coordinates,
                        analysis,
                        expression_digests,
                        pattern_digests,
                        match_products,
                        content_body_digest,
                        Some(owner),
                    )?;
                }
            }
        }
        CheckedExpressionResolution::Closure(closure) => {
            transcript_update!(hasher, &[4]);
            write_len(hasher, closure.captures().len())?;
            for capture in closure.captures() {
                transcript_update!(
                    hasher,
                    &coordinates.binding(capture.local())?.canonical_bytes()?
                );
                transcript_update!(hasher, &[capture_access_tag(capture.mode())]);
            }
        }
        CheckedExpressionResolution::ImplicitParameter(parameter) => {
            transcript_update!(hasher, &[5]);
            transcript_update!(hasher, parameter.callable().as_bytes());
            transcript_update!(hasher, &parameter.occurrence_ordinal().to_le_bytes());
            transcript_update!(hasher, parameter.parameter_type().as_bytes());
        }
        CheckedExpressionResolution::Pipe(pipe) => {
            transcript_update!(hasher, &[6]);
            transcript_update!(hasher, pipe.binding_identity().as_bytes());
            transcript_update!(hasher, pipe.value_type().as_bytes());
            write_len(hasher, pipe.occurrences().len())?;
            for occurrence in pipe.occurrences() {
                transcript_update!(hasher, &occurrence.ordinal().to_le_bytes());
                write_bytes(hasher, &occurrence.coordinate().canonical_bytes()?)?;
            }
        }
        CheckedExpressionResolution::PipeLeft(pipe) => {
            transcript_update!(hasher, &[7]);
            transcript_update!(hasher, pipe.binding_identity().as_bytes());
            transcript_update!(hasher, &pipe.occurrence_ordinal().to_le_bytes());
            transcript_update!(hasher, pipe.value_type().as_bytes());
        }
        CheckedExpressionResolution::ViewCall(view) => {
            transcript_update!(hasher, &[8]);
            transcript_update!(hasher, &[view_call_tag(view)]);
        }
        CheckedExpressionResolution::StyleValue(value) => {
            transcript_update!(hasher, &[9]);
            transcript_update!(hasher, value.semantic_digest().as_bytes());
        }
        CheckedExpressionResolution::DialogueLineReference(line) => {
            transcript_update!(hasher, &[10]);
            write_bytes(hasher, line.canonical_identity_bytes())?;
        }
        CheckedExpressionResolution::DialogueLineCoordinate(line) => {
            transcript_update!(hasher, &[11]);
            write_bytes(hasher, line.canonical_identity_bytes())?;
        }
        CheckedExpressionResolution::DialogueTextKeyCoordinate(key) => {
            transcript_update!(hasher, &[12]);
            write_bytes(hasher, key.canonical_identity_bytes())?;
        }
        CheckedExpressionResolution::CharacterDialogueFactory(factory) => {
            transcript_update!(hasher, &[13]);
            write_character_dialogue_target(hasher, factory.target(), expression_digests)?;
            write_character_dialogue_patch(hasher, factory.patch(), expression_digests)?;
        }
        CheckedExpressionResolution::CharacterDialogueReconfigure(reconfigure) => {
            transcript_update!(hasher, &[14]);
            write_character_dialogue_target(hasher, reconfigure.target(), expression_digests)?;
            write_character_dialogue_patch(hasher, reconfigure.patch(), expression_digests)?;
        }
        CheckedExpressionResolution::DialogueApplication {
            target,
            application_patch,
            rich_text: _,
            line_result,
        } => {
            transcript_update!(hasher, &[15]);
            write_character_dialogue_target(hasher, target, expression_digests)?;
            match application_patch {
                Some(patch) => {
                    transcript_update!(hasher, &[1]);
                    write_character_dialogue_patch(hasher, patch, expression_digests)?;
                }
                None => transcript_update!(hasher, &[0]),
            }
            let rich_text = content_body_digest.ok_or(SemanticTranscriptError::MissingIdentity)?;
            transcript_update!(hasher, rich_text.as_bytes());
            transcript_update!(hasher, line_result.semantic_identity_digest()?.as_bytes());
        }
        CheckedExpressionResolution::PostfixBracket(postfix) => {
            transcript_update!(hasher, &[16]);
            match postfix {
                super::PostfixBracketResolution::Index { candidate } => {
                    transcript_update!(hasher, &[0]);
                    write_resolution_child_digest(
                        hasher,
                        expression_digests,
                        coordinates,
                        cycle_owner,
                        *candidate,
                    )?;
                }
                super::PostfixBracketResolution::Dialogue { candidate } => {
                    transcript_update!(hasher, &[1]);
                    write_resolution_child_digest(
                        hasher,
                        expression_digests,
                        coordinates,
                        cycle_owner,
                        *candidate,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn write_mutable_place_payload(
    hasher: &mut MatchTranscriptHasher<'_>,
    checked: &super::CheckedExpression,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    let Some(place) = checked
        .mutable_place()
        .filter(|place| place.nominal_field().is_some())
    else {
        transcript_update!(hasher, &[0]);
        return Ok(());
    };
    let field_place = place
        .nominal_field()
        .ok_or(SemanticTranscriptError::MissingIdentity)?;
    transcript_update!(hasher, &[1]);
    write_bytes(
        hasher,
        &coordinates.binding(place.local_id())?.canonical_bytes()?,
    )?;
    write_nominal(hasher, field_place.nominal(), analysis)?;
    write_field_selection(hasher, field_place.field())?;
    transcript_update!(
        hasher,
        field_place
            .field_type()
            .semantic_identity_digest()?
            .as_bytes()
    );
    Ok(())
}

fn write_value_resolution(
    hasher: &mut MatchTranscriptHasher<'_>,
    value: &CheckedValueResolution,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    analysis: &FinalSemanticAnalysis,
    owner: ExprId,
    ty: Option<&TypeKind>,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(hasher, &value.semantic_transcript_tag().to_le_bytes());
    match value {
        CheckedValueResolution::Local(local) => {
            transcript_update!(hasher, &coordinates.binding(*local)?.canonical_bytes()?);
        }
        CheckedValueResolution::LineContext => {}
        CheckedValueResolution::CharacterField {
            receiver,
            character,
            field,
        } => {
            write_value_resolution(hasher, receiver, coordinates, analysis, owner, ty)?;
            write_bytes(hasher, character.canonical_identity_bytes())?;
            transcript_update!(
                hasher,
                &[match field {
                    crate::types::CharacterField::Stage => 0,
                }]
            );
        }
        CheckedValueResolution::ProjectCallable(callable) => {
            write_project_callable(hasher, analysis, coordinates, callable)?;
        }
        CheckedValueResolution::ProjectItem(item) => {
            if !item.has_valid_semantic_identity() {
                return Err(SemanticTranscriptError::RecoveredOwner);
            }
            transcript_update!(hasher, item.semantic_id().as_bytes());
            transcript_update!(hasher, item.value_type().as_bytes());
        }
        CheckedValueResolution::Entry(entry) => {
            transcript_update!(hasher, entry.binding().as_bytes());
            transcript_update!(hasher, entry.value_type().as_bytes());
        }
        CheckedValueResolution::Registered(value) => {
            transcript_update!(hasher, value.as_bytes());
        }
        CheckedValueResolution::Constant(literal) => {
            write_literal(
                hasher,
                literal,
                ty.ok_or_else(|| {
                    SemanticTranscriptError::from(
                        super::FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                    )
                })?,
            )?;
        }
    }
    Ok(())
}

fn write_agent_field(
    hasher: &mut MatchTranscriptHasher<'_>,
    field: arcweft_core::value::RuntimeAgentField,
) -> Result<(), SemanticTranscriptError> {
    match field.owner() {
        arcweft_core::value::RuntimeAgentFieldOwner::Agent(owner) => {
            transcript_update!(hasher, &[0, owner.semantic_tag()]);
        }
        arcweft_core::value::RuntimeAgentFieldOwner::Reference => {
            transcript_update!(hasher, &[1]);
        }
    }
    transcript_update!(hasher, &field.semantic_tag().to_le_bytes());
    match field.result() {
        arcweft_core::value::RuntimeAgentFieldResult::Required(value) => {
            transcript_update!(hasher, &[0]);
            write_agent_field_value(hasher, value)?;
        }
        arcweft_core::value::RuntimeAgentFieldResult::Optional(value) => {
            transcript_update!(hasher, &[1]);
            write_agent_field_value(hasher, value)?;
        }
    }
    Ok(())
}

fn write_agent_field_value(
    hasher: &mut MatchTranscriptHasher<'_>,
    value: arcweft_core::value::RuntimeAgentFieldValue,
) -> Result<(), SemanticTranscriptError> {
    match value {
        arcweft_core::value::RuntimeAgentFieldValue::Bool => transcript_update!(hasher, &[0]),
        arcweft_core::value::RuntimeAgentFieldValue::String => transcript_update!(hasher, &[1]),
        arcweft_core::value::RuntimeAgentFieldValue::U32 => transcript_update!(hasher, &[2]),
        arcweft_core::value::RuntimeAgentFieldValue::U64 => transcript_update!(hasher, &[3]),
        arcweft_core::value::RuntimeAgentFieldValue::Agent(owner) => {
            transcript_update!(hasher, &[4, owner.semantic_tag()]);
        }
        arcweft_core::value::RuntimeAgentFieldValue::BuiltinVariant(owner) => {
            transcript_update!(hasher, &[5, owner.semantic_tag()]);
        }
        arcweft_core::value::RuntimeAgentFieldValue::VecAgent(owner) => {
            transcript_update!(hasher, &[6, owner.semantic_tag()]);
        }
        arcweft_core::value::RuntimeAgentFieldValue::AgentValueMap => {
            transcript_update!(hasher, &[7]);
        }
    }
    Ok(())
}

fn write_try_carrier(
    hasher: &mut MatchTranscriptHasher<'_>,
    carrier: &super::CheckedTryCarrier,
) -> Result<(), SemanticTranscriptError> {
    match carrier {
        super::CheckedTryCarrier::Result { .. } => {
            transcript_update!(hasher, &[0]);
        }
        super::CheckedTryCarrier::Option { .. } => {
            transcript_update!(hasher, &[1]);
        }
    }
    transcript_update!(hasher, carrier.semantic_type_digest()?.as_bytes());
    Ok(())
}

fn write_try_boundary(
    hasher: &mut MatchTranscriptHasher<'_>,
    boundary: &super::CheckedTryBoundary,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(
        hasher,
        boundary
            .boundary_type()
            .semantic_identity_digest()?
            .as_bytes()
    );
    match boundary.owner() {
        super::CheckedTryBoundaryOwner::Infallible => transcript_update!(hasher, &[0]),
        super::CheckedTryBoundaryOwner::CarrierBlock(boundary) => {
            transcript_update!(hasher, &[1]);
            write_bytes(hasher, &boundary.coordinate().canonical_bytes()?)?;
        }
        super::CheckedTryBoundaryOwner::Callable(boundary) => {
            transcript_update!(hasher, &[2]);
            write_callable_boundary(hasher, boundary)?;
        }
    }
    Ok(())
}

fn write_callable_boundary(
    hasher: &mut MatchTranscriptHasher<'_>,
    boundary: &super::CheckedCallableBoundary,
) -> Result<(), SemanticTranscriptError> {
    match boundary {
        super::CheckedCallableBoundary::FunctionSite(site) => {
            transcript_update!(hasher, &[0]);
            match site {
                super::CheckedFunctionSiteBoundary::Explicit(boundary) => {
                    transcript_update!(hasher, &[0]);
                    write_bytes(hasher, &boundary.coordinate().canonical_bytes()?)?;
                }
                super::CheckedFunctionSiteBoundary::Implicit { site, callable } => {
                    transcript_update!(hasher, &[1]);
                    write_bytes(hasher, &site.coordinate().canonical_bytes()?)?;
                    transcript_update!(hasher, callable.as_bytes());
                }
            }
        }
        super::CheckedCallableBoundary::Declaration(boundary) => {
            transcript_update!(hasher, &[1]);
            transcript_update!(hasher, boundary.accepted().as_bytes());
        }
    }
    Ok(())
}

const fn capture_access_tag(value: arcweft_lang_hir::scope::CaptureAccess) -> u8 {
    match value {
        arcweft_lang_hir::scope::CaptureAccess::Read => 0,
        arcweft_lang_hir::scope::CaptureAccess::Reassign => 1,
    }
}

const fn view_call_tag(value: &super::CheckedViewCall) -> u8 {
    match value {
        super::CheckedViewCall::Element(_) => 0,
        super::CheckedViewCall::Text => 1,
        super::CheckedViewCall::RichText => 2,
    }
}

fn write_character_dialogue_target(
    hasher: &mut MatchTranscriptHasher<'_>,
    target: &super::CheckedCharacterDialogueTarget,
    expression_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(hasher, target.ty().semantic_identity_digest()?.as_bytes());
    write_child_expression_digest(hasher, expression_digests, target.expression())?;
    match target {
        super::CheckedCharacterDialogueTarget::Character {
            item, character, ..
        } => {
            transcript_update!(hasher, &[0]);
            match item {
                Some(item) => {
                    if !item.has_valid_semantic_identity() {
                        return Err(SemanticTranscriptError::RecoveredOwner);
                    }
                    transcript_update!(hasher, &[1]);
                    transcript_update!(hasher, item.semantic_id().as_bytes());
                }
                None => transcript_update!(hasher, &[0]),
            }
            match character {
                arcweft_dialogue::CharacterDialogueCharacterType::Exact(character) => {
                    transcript_update!(hasher, &[0]);
                    write_bytes(hasher, character.canonical_identity_bytes())?;
                }
                arcweft_dialogue::CharacterDialogueCharacterType::Any => {
                    return Err(SemanticTranscriptError::RecoveredOwner);
                }
            }
        }
        super::CheckedCharacterDialogueTarget::Dialogue { ty, .. } => {
            transcript_update!(hasher, &[1]);
            match ty.character() {
                arcweft_dialogue::CharacterDialogueCharacterType::Exact(character) => {
                    transcript_update!(hasher, &[0]);
                    write_bytes(hasher, character.canonical_identity_bytes())?;
                }
                arcweft_dialogue::CharacterDialogueCharacterType::Any => {
                    return Err(SemanticTranscriptError::RecoveredOwner);
                }
            }
        }
    }
    Ok(())
}

fn write_character_dialogue_patch(
    hasher: &mut MatchTranscriptHasher<'_>,
    patch: &super::CheckedCharacterDialoguePatch,
    expression_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(
        hasher,
        &[match patch.context() {
            super::CharacterDialoguePatchContext::ReusableValue => 0,
            super::CharacterDialoguePatchContext::ImmediateContentApplication => 1,
        }]
    );
    write_len(hasher, patch.fields().len())?;
    for field in patch.fields() {
        transcript_update!(hasher, &[field.coordinate().semantic_tag()]);
        if let super::CharacterDialogueFieldCoordinate::Custom(id) = field.coordinate() {
            write_bytes(hasher, id.canonical_identity_bytes())?;
        }
        match field.operation() {
            super::CheckedPatchOperation::Set { value, ty } => {
                transcript_update!(hasher, &[0]);
                write_child_expression_digest(hasher, expression_digests, *value)?;
                transcript_update!(hasher, ty.semantic_identity_digest()?.as_bytes());
            }
            super::CheckedPatchOperation::Clear => transcript_update!(hasher, &[1]),
        }
    }
    Ok(())
}

fn write_effect_plan(
    hasher: &mut MatchTranscriptHasher<'_>,
    plan: &super::CheckedDialogueEffectPlan,
) -> Result<(), SemanticTranscriptError> {
    write_len(hasher, plan.effect_sites().len())?;
    for site in plan.effect_sites() {
        transcript_update!(hasher, &site.id().get().to_le_bytes());
        match site.trigger() {
            super::CheckedDialogueEffectTrigger::Content => transcript_update!(hasher, &[0]),
            super::CheckedDialogueEffectTrigger::Delay(duration) => {
                transcript_update!(hasher, &[1]);
                transcript_update!(hasher, &duration.millis.to_le_bytes());
            }
        }
        write_effects(hasher, site.effects())?;
        match site.operation() {
            super::CheckedDialogueEffectOperation::EvaluatedEffect(effect) => {
                transcript_update!(hasher, &[0]);
                write_evaluated_effect(hasher, effect)?;
            }
            super::CheckedDialogueEffectOperation::Call {
                application,
                result,
                ..
            } => {
                transcript_update!(hasher, &[1]);
                // The point-action token already commits its checked call
                // expression, and the plan sealer proves the same site root.
                write_bytes(hasher, &application.coordinate().canonical_bytes()?)?;
                transcript_update!(hasher, result.semantic_identity_digest()?.as_bytes());
            }
        }
        write_len(hasher, site.captures().len())?;
        for capture in site.captures() {
            write_bytes(hasher, &capture.origin().canonical_bytes()?)?;
            transcript_update!(hasher, capture.ty().semantic_identity_digest()?.as_bytes());
        }
    }
    Ok(())
}

fn write_compile_time_scalar(
    hasher: &mut MatchTranscriptHasher<'_>,
    owner: ExprId,
    scalar: &super::CheckedCompileTimeScalarExpression,
    ty: &TypeKind,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    analysis: &FinalSemanticAnalysis,
    expression_digests: &BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    pattern_digests: &BTreeMap<PatternId, CheckedPatternSemanticDigest>,
    match_products: &BTreeMap<ExprId, CheckedMatch>,
    content_body_digest: Option<&CheckedRichTextSemanticDigest>,
    cycle_owner: Option<ExprId>,
) -> Result<(), SemanticTranscriptError> {
    let owner_coordinate = coordinates.expression(owner)?.canonical_bytes()?;
    match scalar.value() {
        CheckedCompileTimeScalar::Bool(value) => {
            transcript_update!(hasher, &[0, u8::from(*value)]);
        }
        CheckedCompileTimeScalar::Int(value) => {
            transcript_update!(hasher, &[1]);
            transcript_update!(hasher, &value.to_le_bytes());
        }
        CheckedCompileTimeScalar::Milli(value) => {
            transcript_update!(hasher, &[2]);
            transcript_update!(hasher, &value.0.to_le_bytes());
        }
        CheckedCompileTimeScalar::Ratio(value) => {
            transcript_update!(hasher, &[3]);
            transcript_update!(hasher, &value.0.to_le_bytes());
        }
        CheckedCompileTimeScalar::Length(value) => {
            transcript_update!(hasher, &[4]);
            transcript_update!(hasher, &value.milli.to_le_bytes());
            transcript_update!(
                hasher,
                &[match value.unit {
                    crate::checked_rich_text::LengthUnit::Px => 0,
                    crate::checked_rich_text::LengthUnit::Pt => 1,
                    crate::checked_rich_text::LengthUnit::Ch => 2,
                    crate::checked_rich_text::LengthUnit::Em => 3,
                }],
            );
        }
        CheckedCompileTimeScalar::Angle(value) => {
            transcript_update!(hasher, &[5]);
            transcript_update!(hasher, &value.milli_degrees.to_le_bytes());
        }
        CheckedCompileTimeScalar::Duration(value) => {
            transcript_update!(hasher, &[6]);
            transcript_update!(hasher, &value.millis.to_le_bytes());
        }
        CheckedCompileTimeScalar::Enum(value) => {
            transcript_update!(hasher, &[7]);
            let declaration = TypeKind::ProjectNominal(crate::types::ProjectNominalType::new(
                value.declaration().clone(),
                Box::<[TypeKind]>::default(),
            ));
            transcript_update!(hasher, declaration.semantic_identity_digest()?.as_bytes());
            write_bytes(hasher, value.semantic_id().as_bytes())?;
            transcript_update!(hasher, &value.ordinal().to_le_bytes());
        }
        CheckedCompileTimeScalar::PublicId(value) => {
            transcript_update!(hasher, &[8]);
            write_bytes(hasher, value.canonical_identity_bytes())?;
        }
        CheckedCompileTimeScalar::Text(value) => {
            transcript_update!(hasher, &[9]);
            write_bytes(hasher, value.as_bytes())?;
        }
        CheckedCompileTimeScalar::Color(value) => {
            transcript_update!(hasher, &[10]);
            match value {
                crate::checked_rich_text::CheckedColor::Rgba8(rgba) => {
                    transcript_update!(hasher, &[0]);
                    write_bytes(hasher, rgba)?;
                }
                crate::checked_rich_text::CheckedColor::Resource(id) => {
                    transcript_update!(hasher, &[1]);
                    write_bytes(hasher, id.canonical_identity_bytes())?;
                }
            }
        }
    }
    write_resolution_payload(
        hasher,
        owner,
        &owner_coordinate,
        scalar.original(),
        Some(ty),
        coordinates,
        analysis,
        expression_digests,
        pattern_digests,
        match_products,
        content_body_digest,
        cycle_owner,
    )
}

fn write_type_value_payload(
    hasher: &mut MatchTranscriptHasher<'_>,
    value: &super::CheckedTypeValue,
) -> Result<(), SemanticTranscriptError> {
    // Keep the closed TypeValue payload family explicit in the transcript.
    // The current member is ProjectNominal; a future member must not become
    // byte-compatible with it by accident.
    transcript_update!(hasher, &[0]);
    transcript_update!(hasher, value.nominal().identity().as_bytes());
    transcript_update!(hasher, value.semantic_definition_digest().as_bytes());
    write_len(hasher, value.nominal().arguments().len())?;
    for argument in value.nominal().arguments() {
        transcript_update!(hasher, argument.semantic_identity_digest()?.as_bytes());
    }
    Ok(())
}

fn write_compile_time_callee(
    hasher: &mut MatchTranscriptHasher<'_>,
    callee: &CheckedCompileTimeCallee,
) -> Result<(), SemanticTranscriptError> {
    match callee {
        CheckedCompileTimeCallee::View(view) => {
            transcript_update!(hasher, &[0, view.semantic_tag()]);
        }
        CheckedCompileTimeCallee::Style(style) => {
            transcript_update!(hasher, &[1, style.semantic_tag()]);
        }
    }
    Ok(())
}

fn write_content_application_resolution(
    hasher: &mut MatchTranscriptHasher<'_>,
    application: &super::CheckedContentApplication,
) -> Result<(), SemanticTranscriptError> {
    // The expression owns the accepted content application carrier.  No
    // insertion index or later HIR reconstruction is consulted here. The
    // selected callable join and ordered argument children are written by
    // the enclosing expression, including the nested Fx producer call.
    write_bytes(hasher, &application.id().path().canonical_bytes()?)?;
    match application {
        super::CheckedContentApplication::Value { source, .. } => {
            transcript_update!(hasher, &[0]);
            write_bytes(hasher, &source.path().canonical_bytes()?)?;
        }
        super::CheckedContentApplication::ContentResultCall { .. } => {
            transcript_update!(hasher, &[1]);
        }
        super::CheckedContentApplication::EmissionCall { edges, .. } => {
            transcript_update!(hasher, &[2]);
            let Some(plan) = edges.fx_plan() else {
                transcript_update!(hasher, &[0]);
                return Ok(());
            };
            transcript_update!(hasher, &[1]);
            transcript_update!(hasher, plan.outer().schema().as_bytes());
            transcript_update!(hasher, plan.producer().digest().as_bytes());
            transcript_update!(hasher, plan.producer().inner().schema().as_bytes());
        }
    }
    Ok(())
}

fn write_checked_view_fx_application(
    hasher: &mut MatchTranscriptHasher<'_>,
    application: &crate::final_analysis::CheckedViewFxApplication,
) -> Result<(), SemanticTranscriptError> {
    // Content and View share the same sealed application authority; the
    // context tag is already part of this issued digest.
    transcript_update!(hasher, application.semantic_digest().as_bytes());
    // Retain the owner-specific producer edge as an explicit transcript atom
    // so the edge remains visible even when the application digest is reused
    // by another checked projection.
    transcript_update!(
        hasher,
        application.plan_ref().producer().digest().as_bytes()
    );
    Ok(())
}

fn write_nominal(
    hasher: &mut MatchTranscriptHasher<'_>,
    nominal: &super::CheckedProjectNominal,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), SemanticTranscriptError> {
    transcript_update!(hasher, nominal.identity().as_bytes());
    let definition = analysis
        .project_nominal_semantic(nominal.identity())
        .filter(|definition| definition.nominal() == nominal)
        .ok_or(SemanticTranscriptError::MissingIdentity)?;
    transcript_update!(hasher, definition.digest().as_bytes());
    write_len(hasher, nominal.arguments().len())?;
    for argument in nominal.arguments() {
        transcript_update!(hasher, argument.semantic_identity_digest()?.as_bytes());
    }
    Ok(())
}

fn write_project_callable(
    hasher: &mut MatchTranscriptHasher<'_>,
    analysis: &FinalSemanticAnalysis,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    callable: &super::CheckedProjectCallable,
) -> Result<(), SemanticTranscriptError> {
    let facts = analysis
        .checked_callables()
        .project_callable(callable.declaration())
        .map_err(|_| SemanticTranscriptError::MissingIdentity)?;
    transcript_update!(
        hasher,
        coordinates
            .accepted_declaration(callable.declaration())?
            .as_bytes(),
    );
    if let Some(interface) = facts.sealed_interface_digest() {
        transcript_update!(hasher, interface.as_bytes());
        return Ok(());
    }
    transcript_update!(hasher, facts.signature().semantic_digest().as_bytes());
    match facts.execution() {
        crate::callable::CheckedCallableExecution::RetainedView => {
            transcript_update!(hasher, &[3]);
        }
        crate::callable::CheckedCallableExecution::DispatchContract => {
            transcript_update!(hasher, &[0]);
        }
        crate::callable::CheckedCallableExecution::Runtime(
            super::CheckedFunctionExecution::DirectFrame,
        ) => {
            transcript_update!(hasher, &[1]);
        }
        crate::callable::CheckedCallableExecution::Runtime(
            super::CheckedFunctionExecution::StreamFactory {
                item,
                error,
                own_scope_yields,
            },
        ) => {
            transcript_update!(hasher, &[2]);
            transcript_update!(hasher, item.semantic_identity_digest()?.as_bytes());
            transcript_update!(hasher, error.semantic_identity_digest()?.as_bytes());
            transcript_update!(hasher, &own_scope_yields.to_le_bytes());
        }
    }
    transcript_update!(
        hasher,
        &[match facts.suspension() {
            super::CheckedSuspensionRole::NonSuspending => 0,
            super::CheckedSuspensionRole::MaySuspend => 1,
        }],
    );
    transcript_update!(
        hasher,
        &[match facts.control() {
            super::CheckedExecutableControlRole::ExpressionCompatible => 0,
            super::CheckedExecutableControlRole::FlowRequired => 1,
        }],
    );
    let effects = facts
        .exposed_row()
        .closed_value()
        .ok_or(SemanticTranscriptError::MissingIdentity)?;
    write_effects(hasher, &effects)?;
    Ok(())
}

fn write_variant_resolution(
    hasher: &mut MatchTranscriptHasher<'_>,
    resolution: &super::CheckedVariantResolution,
) -> Result<(), SemanticTranscriptError> {
    let owner_tag = resolution.owner().payload_owner_family().canonical_tag();
    transcript_update!(hasher, &[owner_tag]);
    transcript_update!(hasher, resolution.owner().semantic_type().as_bytes());
    let selected = resolution.selected();
    transcript_update!(hasher, selected.semantic_id().as_bytes());
    transcript_update!(hasher, &selected.ordinal().to_le_bytes());
    match selected.payload() {
        crate::types::VariantPayloadShape::Unit => {
            transcript_update!(hasher, &[0]);
        }
        crate::types::VariantPayloadShape::Tuple(fields) => {
            transcript_update!(hasher, &[1]);
            write_len(hasher, fields.len())?;
            for field in fields {
                transcript_update!(hasher, &field.ordinal().to_le_bytes());
                transcript_update!(hasher, field.semantic_id().as_bytes());
                transcript_update!(hasher, field.ty().semantic_identity_digest()?.as_bytes());
            }
        }
        crate::types::VariantPayloadShape::Record(fields) => {
            transcript_update!(hasher, &[2]);
            write_len(hasher, fields.len())?;
            for field in fields {
                transcript_update!(hasher, &field.ordinal().to_le_bytes());
                transcript_update!(hasher, field.semantic_id().as_bytes());
                transcript_update!(hasher, field.ty().semantic_identity_digest()?.as_bytes());
            }
        }
    }
    Ok(())
}

fn write_effects(
    hasher: &mut MatchTranscriptHasher<'_>,
    effects: &crate::effects::EffectSet,
) -> Result<(), SemanticTranscriptError> {
    write_len(hasher, effects.len())?;
    for effect in effects.iter() {
        transcript_update!(hasher, effect.semantic_digest().as_bytes());
    }
    Ok(())
}

pub(crate) fn write_len<C: TranscriptByteCounter + ?Sized>(
    hasher: &mut TranscriptHasher<'_, C>,
    value: usize,
) -> Result<(), C::Error>
where
    C::Error: From<TranscriptWriteError>,
{
    let value = u64::try_from(value)
        .map_err(|_| C::Error::from(TranscriptWriteError::ArithmeticOverflow))?;
    hasher.update(&value.to_le_bytes())
}

fn write_bytes(
    hasher: &mut MatchTranscriptHasher<'_>,
    value: &[u8],
) -> Result<(), SemanticTranscriptError> {
    write_len(hasher, value.len())?;
    transcript_update!(hasher, value);
    Ok(())
}

fn guard_tag(value: CheckedGuardClass) -> u8 {
    match value {
        CheckedGuardClass::Absent => 0,
        CheckedGuardClass::ConstantTrue => 1,
        CheckedGuardClass::ConstantFalse => 2,
        CheckedGuardClass::Dynamic => 3,
    }
}

fn unreachable_tag(value: CheckedUnreachableReason) -> u8 {
    match value {
        CheckedUnreachableReason::CoveredByPriorUsefulArms => 0,
        CheckedUnreachableReason::ConstantFalseGuard => 1,
        CheckedUnreachableReason::CoveredByEarlierOrAlternative => 2,
        CheckedUnreachableReason::UninhabitedDomain => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked_match_digest(source: &str) -> [u8; 32] {
        let fixture = crate::final_analysis::tests::fixture(source, None);
        let analysis = crate::final_analysis::tests::analyze(&fixture)
            .expect("fixture has a checked exhaustive Match");
        let project = fixture.project.analysis_view().expect("executable HIR");
        let module = project
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root HIR module");
        let owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
            })
            .expect("Match expression");
        let product = analysis
            .checked_match(
                project,
                &fixture.symbols,
                owner,
                CheckedMatchLimits::PRODUCTION,
            )
            .expect("accepted Match transcript");
        *product.semantic_digest().as_bytes()
    }

    #[test]
    fn checked_progress_field_payload_discriminates_selected_field_without_type_input() {
        let fixture = crate::final_analysis::tests::fixture(
            r"
fn observe(need: Need<i64>) -> i64 {
    await need with {
        pending progress => {
            let selected = match true {
                true => {
                    let ratio = progress.ratio
                    let label = progress.label
                    1i64
                }
                false => 0i64
            }
        }
    }
}
",
            None,
        );
        let analysis = crate::final_analysis::tests::analyze(&fixture)
            .expect("Pending Match with both checked Progress fields");
        let project = fixture.project.analysis_view().expect("executable HIR");
        let module = project
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root HIR module");
        let match_owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
            })
            .expect("Pending body Match expression");
        analysis
            .checked_match(
                project,
                &fixture.symbols,
                match_owner,
                CheckedMatchLimits::PRODUCTION,
            )
            .expect("accepted Pending Match transcript");

        let coordinates = SemanticCoordinateIndex::new(analysis.accepted_root_catalog(), &analysis);
        let match_path = coordinates
            .expression(match_owner)
            .expect("accepted Match root path");
        let selections = analysis
            .expressions()
            .filter_map(|(owner, expression)| {
                let CheckedExpressionResolution::Select(CheckedSelectResolution::ProgressField {
                    field,
                }) = expression.resolution()
                else {
                    return None;
                };
                let path = coordinates
                    .expression(owner)
                    .expect("checked Progress field path");
                (path.root() == match_path.root() && path.steps().starts_with(match_path.steps()))
                    .then_some((owner, *field, expression))
            })
            .collect::<Vec<_>>();
        let [ratio, label] = selections.as_slice() else {
            panic!("the Match body has exactly Ratio and Label selections");
        };
        assert_eq!(ratio.1, crate::types::ProgressField::Ratio);
        assert_eq!(ratio.2.value_type(), Some(&TypeKind::F32));
        assert_eq!(label.1, crate::types::ProgressField::Label);
        assert_eq!(
            label.2.value_type(),
            Some(&TypeKind::Option(Box::new(TypeKind::String))),
        );
        assert_eq!(ratio.1.semantic_tag(), 0);
        assert_eq!(label.1.semantic_tag(), 1);

        let owner = ratio.0;
        let owner_coordinate = coordinates
            .expression(owner)
            .expect("accepted Progress selection path")
            .canonical_bytes()
            .expect("canonical Progress selection path");
        let expression_digests = BTreeMap::new();
        let pattern_digests = BTreeMap::new();
        let match_products = BTreeMap::new();
        let payload_digest = |resolution: &CheckedExpressionResolution| {
            let mut budget = CheckedMatchBudget::new(CheckedMatchLimits::PRODUCTION);
            let mut hasher = TranscriptHasher::new(&mut budget);
            hasher
                .update(b"progress-field-payload-test\0")
                .expect("prefix fits");
            write_resolution_payload(
                &mut hasher,
                owner,
                &owner_coordinate,
                resolution,
                None,
                &coordinates,
                &analysis,
                &expression_digests,
                &pattern_digests,
                &match_products,
                None,
                None,
            )
            .expect("checked Progress selection payload");
            let byte_len = hasher.byte_len();
            (hasher.finalize(), byte_len)
        };
        let ratio_payload = payload_digest(ratio.2.resolution());
        let label_payload = payload_digest(label.2.resolution());
        assert_eq!(ratio_payload.1, label_payload.1);
        assert_ne!(ratio_payload.0, label_payload.0);
    }

    #[test]
    fn type_value_transcript_discriminates_its_closed_payload_family() {
        let fixture = crate::final_analysis::tests::fixture(
            r#"
#[text_proxy]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit)[typed]]
    return "done"
}
"#,
            None,
        );
        let analysis = crate::final_analysis::tests::analyze(&fixture)
            .expect("the fixture publishes a checked TypeValue fact");
        let value = analysis
            .expressions()
            .find_map(|(_, expression)| match expression.resolution() {
                CheckedExpressionResolution::TypeValue(value) => Some(value),
                _ => None,
            })
            .expect("Object type discriminator TypeValue fact");
        let definition = analysis
            .project_nominal_semantic(value.nominal().identity())
            .expect("TypeValue nominal semantic owner");
        assert_eq!(value.nominal(), definition.nominal());
        assert_eq!(value.semantic_definition_digest(), definition.digest());

        let with_family_tag = {
            let mut budget = CheckedMatchBudget::new(CheckedMatchLimits::PRODUCTION);
            let mut hasher = TranscriptHasher::new(&mut budget);
            write_type_value_payload(&mut hasher, value).expect("TypeValue payload transcript");
            hasher.finalize()
        };
        let without_family_tag = {
            let mut budget = CheckedMatchBudget::new(CheckedMatchLimits::PRODUCTION);
            let mut hasher = TranscriptHasher::new(&mut budget);
            write_nominal(&mut hasher, value.nominal(), &analysis)
                .expect("nominal payload transcript");
            hasher.finalize()
        };

        assert_ne!(
            with_family_tag, without_family_tag,
            "TypeValue's ProjectNominal member must occupy an explicit payload domain"
        );
    }

    #[test]
    fn match_transcript_accepts_object_span_inside_interpolation() {
        let fixture = crate::final_analysis::tests::fixture(
            r#"
#[text_proxy]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main(flag: bool) -> String {
    alice[#object(id = @.hotspot, type = KeywordHit)[#[match flag {
        true => 1i64
        false => 0i64
    }]]]
    return "done"
}
"#,
            None,
        );
        let analysis = crate::final_analysis::tests::analyze(&fixture)
            .expect("Object interpolation with exhaustive Match");
        let project = fixture.project.analysis_view().expect("executable HIR");
        let module = project
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root HIR module");
        let owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
            })
            .expect("interpolation Match expression");
        let product = analysis
            .checked_match(
                project,
                &fixture.symbols,
                owner,
                CheckedMatchLimits::PRODUCTION,
            )
            .expect("ObjectSpan must contribute its owner digest to the Match transcript");
        assert!(product.coverage().exhaustive());
    }

    #[test]
    fn match_digest_commits_named_scope_identities_in_statement_and_expression_scopes() {
        let statement_scope = |label: &str| {
            format!(
                "fn root(flag: bool) -> bool {{\n    match flag {{\n        true => {{\n            scope {label} {{}}\n            true\n        }}\n        false => false\n    }}\n}}\n"
            )
        };
        assert_ne!(
            checked_match_digest(&statement_scope("first")),
            checked_match_digest(&statement_scope("renamed")),
        );

        let expression_scope = |label: &str| {
            format!(
                "fn root(flag: bool) -> bool {{\n    match flag {{\n        true => scope {label} {{ true }}\n        false => false\n    }}\n}}\n"
            )
        };
        assert_ne!(
            checked_match_digest(&expression_scope("first")),
            checked_match_digest(&expression_scope("renamed")),
        );
    }

    #[test]
    fn match_digest_tracks_the_accepted_coordinate_of_a_named_scope() {
        let statement_scope_in_true_arm = r#"
fn root(flag: bool) -> bool {
    match flag {
        true => {
            scope stable {}
            true
        }
        false => false
    }
}
"#;
        let statement_scope_in_false_arm = r#"
fn root(flag: bool) -> bool {
    match flag {
        true => true
        false => {
            scope stable {}
            false
        }
    }
}
"#;
        assert_ne!(
            checked_match_digest(statement_scope_in_true_arm),
            checked_match_digest(statement_scope_in_false_arm),
        );

        let expression_scope_in_true_arm = r#"
fn root(flag: bool) -> bool {
    match flag {
        true => scope stable { true }
        false => false
    }
}
"#;
        let expression_scope_in_false_arm = r#"
fn root(flag: bool) -> bool {
    match flag {
        true => true
        false => scope stable { false }
    }
}
"#;
        assert_ne!(
            checked_match_digest(expression_scope_in_true_arm),
            checked_match_digest(expression_scope_in_false_arm),
        );
    }

    #[test]
    fn match_digest_distinguishes_named_and_anonymous_scopes() {
        let named_scope = r#"
fn root(flag: bool) -> bool {
    match flag {
        true => scope stable { true }
        false => false
    }
}
"#;
        let anonymous_scope = r#"
fn root(flag: bool) -> bool {
    match flag {
        true => scope { true }
        false => false
    }
}
"#;
        assert_ne!(
            checked_match_digest(named_scope),
            checked_match_digest(anonymous_scope),
        );
    }
}

impl SemanticTranscriptGraph<'_, '_> {
    fn expression_digest_at(
        &mut self,
        owner: ExprId,
        depth: u64,
    ) -> Result<CheckedExpressionSemanticDigest, SemanticTranscriptError> {
        self.control.check()?;
        let analysis = self.analysis;
        let module = self.module;
        self.budget.observe_depth(depth)?;
        let checked = analysis
            .expression(owner)
            .ok_or(SemanticTranscriptError::MissingExpression)?;
        let hir = module
            .resolve_expr(owner)
            .map_err(|_| SemanticTranscriptError::MissingExpression)?;
        if matches!(hir.kind(), HirExprKind::Error(_)) {
            return Err(SemanticTranscriptError::RecoveredOwner);
        }
        let checked_type = checked.value_type();
        let path = self.coordinates.expression(owner)?;
        let semantic_coordinate = StableSemanticCoordinate::new(path.clone());
        if self
            .expression_paths
            .get(&semantic_coordinate)
            .is_some_and(|existing| *existing != owner)
        {
            return Err(CheckedMatchBuildError::DuplicateSemanticPath {
                coordinate: semantic_coordinate,
            }
            .into());
        }
        if self.expression_visiting.contains(&owner) {
            return Err(CheckedMatchBuildError::DuplicateSemanticPath {
                coordinate: semantic_coordinate,
            }
            .into());
        }
        if let Some(digest) = self.expression_digests.get(&owner) {
            return Ok(*digest);
        }
        self.budget
            .charge(CheckedMatchLimitKind::ExpressionNodes, 1)?;
        self.expression_paths.insert(semantic_coordinate, owner);
        self.expression_visiting.insert(owner);
        let path_depth = u64::try_from(path.steps().len()).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::Depth,
            }
        })?;
        self.budget.observe_depth(path_depth)?;
        let edges = analysis
            .checked_expression_edge_fact(owner)
            .map_err(|_| SemanticTranscriptError::MissingChildEdges)?;
        let child_depth =
            depth
                .checked_add(1)
                .ok_or(CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::Depth,
                })?;
        let mut child_digests = Vec::new();
        for edge in edges.edges() {
            let digest = self.expression_digest_at(edge.child(), child_depth)?;
            child_digests.try_reserve_exact(1).map_err(|_| {
                CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::ExpressionNodes,
                }
            })?;
            child_digests.push(digest);
        }
        let mut owned_body_digests = Vec::new();
        let mut owned_child_digests = Vec::new();
        for edge in hir
            .kind()
            .expression_owned_child_edges()
            .map_err(|_| SemanticTranscriptError::RecoveredOwner)?
        {
            let role =
                crate::semantic_coordinate::expression_owned_role_transcript_bytes(edge.role())?;
            match edge.child() {
                arcweft_lang_hir::expr::HirExpressionOwnedChild::Pattern(pattern) => {
                    let digest = self.generic_pattern_digest(pattern)?;
                    owned_child_digests.push((role, digest.as_bytes().to_vec()));
                }
                arcweft_lang_hir::expr::HirExpressionOwnedChild::Statement(statement) => {
                    let digest = self.statement_digest_at(statement, child_depth)?;
                    owned_child_digests.push((role, digest.as_bytes().to_vec()));
                }
                arcweft_lang_hir::expr::HirExpressionOwnedChild::Body(_) => {}
            }
        }
        if let Some(body) = hir
            .kind()
            .try_body_projection()
            .map_err(|_| SemanticTranscriptError::RecoveredOwner)?
        {
            let owner = HirSemanticBodyOwner::direct_expression(owner);
            let (coordinate, digest) = self.body_digest_at(owner, &body, None, child_depth)?;
            owned_body_digests.push((coordinate, digest));
        }
        for owned in hir
            .kind()
            .expression_owned_body_projections()
            .map_err(|_| SemanticTranscriptError::RecoveredOwner)?
        {
            let owner = HirSemanticBodyOwner::try_expression_owned(owner, owned.role().clone())
                .map_err(|_| SemanticTranscriptError::RecoveredOwner)?;
            let (coordinate, digest) =
                self.body_digest_at(owner, owned.projection(), None, child_depth)?;
            owned_body_digests.push((coordinate, digest));
        }
        let content_body_digest = match checked.resolution() {
            CheckedExpressionResolution::ContentApplication(_) => None,
            CheckedExpressionResolution::DialogueApplication { rich_text, .. } => {
                Some(self.rich_text_semantic_digest(rich_text, child_depth)?)
            }
            _ => None,
        };
        if let HirExprKind::Match(authored) = hir.kind() {
            let product = self.build(owner, authored)?;
            if self.match_products.insert(owner, product).is_some() {
                return Err(SemanticTranscriptError::MissingMatchFact);
            }
        }
        let owner_coordinate = path.canonical_bytes()?;
        let mut hasher = TranscriptHasher::new(&mut self.budget);
        transcript_update!(hasher, b"arcweft.lang.checked-expression-semantic.v1\0");
        transcript_update!(hasher, &owner_coordinate);
        transcript_update!(hasher, &hir.kind().semantic_transcript_tag().to_le_bytes());
        transcript_update!(hasher, &checked.resolution().semantic_tag().to_le_bytes());
        match checked.result() {
            super::CheckedExpressionResult::Value(value) => {
                if let Some(specialization) = value.specialization() {
                    transcript_update!(hasher, &[3]);
                    transcript_update!(hasher, specialization.digest().as_bytes());
                } else {
                    transcript_update!(hasher, &[0]);
                }
                transcript_update!(hasher, value.ty().semantic_identity_digest()?.as_bytes());
            }
            super::CheckedExpressionResult::NonValue(
                super::CheckedNonValueExpressionResult::ContentEmission(callable),
            ) => {
                transcript_update!(hasher, &[1, callable.semantic_tag()]);
            }
            super::CheckedExpressionResult::Unavailable => {
                transcript_update!(hasher, &[2]);
            }
        }
        if let CheckedExpressionResolution::Literal(literal) = checked.resolution() {
            write_literal(
                &mut hasher,
                literal,
                checked_type.ok_or_else(|| {
                    SemanticTranscriptError::from(
                        super::FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                    )
                })?,
            )?;
        }
        write_expression_shape_atoms(&mut hasher, owner, hir.kind(), checked, analysis)?;
        write_resolution_payload(
            &mut hasher,
            owner,
            &owner_coordinate,
            checked.resolution(),
            checked.source_value_type(),
            &self.coordinates,
            analysis,
            &self.expression_digests,
            &self.pattern_digests,
            &self.match_products,
            content_body_digest.as_ref(),
            None,
        )?;
        write_mutable_place_payload(&mut hasher, checked, &self.coordinates, analysis)?;
        write_record_expression_fields(&mut hasher, edges)?;
        write_effects(&mut hasher, checked.effects())?;
        match checked.evaluated_effect() {
            Some(effect) => {
                transcript_update!(hasher, &[1]);
                write_evaluated_effect(&mut hasher, effect)?;
            }
            None => transcript_update!(hasher, &[0]),
        }
        if matches!(hir.kind(), HirExprKind::Match(_)) {
            // Match patterns, arm bindings, coverage, witnesses, and unreachable
            // rows are not ordinary expression edges.  The complete accepted
            // Match product is therefore an explicit atom of its expression;
            // hashing only the lightweight CheckedMatchFact would lose those
            // semantics when a Match is nested in another expression.
            let checked_match = self
                .match_products
                .get(&owner)
                .ok_or(SemanticTranscriptError::MissingMatchFact)?;
            transcript_update!(hasher, checked_match.semantic_digest().as_bytes());
        }
        write_len(&mut hasher, owned_body_digests.len())?;
        for (coordinate, digest) in owned_body_digests {
            write_bytes(&mut hasher, &coordinate.canonical_bytes()?)?;
            transcript_update!(hasher, digest.as_bytes());
        }
        write_len(&mut hasher, owned_child_digests.len())?;
        for (role, digest) in owned_child_digests {
            write_bytes(&mut hasher, &role)?;
            transcript_update!(hasher, &digest);
        }
        if let Some(site) = checked.resolution().checked_call_site(owner) {
            let callable = edges
                .callable()
                .ok_or(SemanticTranscriptError::MissingCallableJoin)?;
            transcript_update!(
                hasher,
                callable
                    .stable_transcript_digest(&self.coordinates)
                    .map_err(|_| SemanticTranscriptError::MissingCallableJoin)?
                    .as_bytes(),
            );
            let application = analysis
                .call(owner)
                .and_then(super::CallTargetFacts::selected_application)
                .ok_or(SemanticTranscriptError::MissingSelectedCallApplication)?;
            if application.core().site() != site
                || application.core().stable_site()
                    != &StableCheckedValueCoordinate::Expression(
                        self.coordinates.expression(owner)?,
                    )
            {
                return Err(SemanticTranscriptError::MissingSelectedCallApplication);
            }
            let arguments = application.core().execution().arguments();
            write_len(&mut hasher, arguments.len())?;
            for argument in arguments {
                transcript_update!(hasher, &[argument.passing().semantic_tag()]);
            }
        }
        write_len(&mut hasher, child_digests.len())?;
        for (edge, child_digest) in edges.edges().iter().zip(child_digests) {
            write_bytes(&mut hasher, &edge.role().transcript_bytes()?)?;
            transcript_update!(hasher, child_digest.as_bytes());
        }
        let digest = CheckedExpressionSemanticDigest::from_bytes(hasher.finalize());
        self.expression_visiting.remove(&owner);
        self.expression_digests.insert(owner, digest);
        Ok(digest)
    }

    fn generic_pattern_digest(
        &mut self,
        owner: PatternId,
    ) -> Result<CheckedPatternSemanticDigest, SemanticTranscriptError> {
        self.control.check()?;
        let analysis = self.analysis;
        let module = self.module;
        let coordinate = self.coordinates.pattern(owner)?;
        let depth = u64::try_from(coordinate.path().steps().len()).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::Depth,
            }
        })?;
        self.budget.observe_depth(depth)?;
        if let Some(digest) = self.pattern_digests.get(&owner) {
            return Ok(*digest);
        }
        if !self.pattern_visiting.insert(owner) {
            return Err(SemanticTranscriptError::MissingChildEdges);
        }
        let checked = analysis
            .pattern(owner)
            .ok_or(SemanticTranscriptError::MissingPattern)?;
        let hir = module
            .resolve_pattern(owner)
            .map_err(|_| SemanticTranscriptError::MissingPattern)?;
        if matches!(hir.kind(), HirPatternKind::Error(_)) {
            return Err(SemanticTranscriptError::RecoveredOwner);
        }
        let coordinate = coordinate.canonical_bytes()?;
        self.budget.charge(CheckedMatchLimitKind::PatternNodes, 1)?;
        let mut children = Vec::new();
        for edge in hir.kind().child_edges() {
            let child = match edge.child() {
                HirPatternChild::Pattern(child) => {
                    let digest = self.generic_pattern_digest(child)?;
                    Some(digest.as_bytes().to_vec())
                }
                HirPatternChild::Local(local) => {
                    let binding = analysis
                        .local(local)
                        .ok_or(SemanticTranscriptError::MissingIdentity)?;
                    let mut hasher = TranscriptHasher::new(&mut self.budget);
                    transcript_update!(hasher, b"arcweft.lang.checked-pattern-binding.v1\0");
                    transcript_update!(
                        hasher,
                        &self.coordinates.binding(local)?.canonical_bytes()?
                    );
                    transcript_update!(hasher, binding.ty().semantic_identity_digest()?.as_bytes());
                    Some(
                        CheckedPatternSemanticDigest::from_bytes(hasher.finalize())
                            .as_bytes()
                            .to_vec(),
                    )
                }
                HirPatternChild::Type(type_id) => {
                    let ty = analysis
                        .ty(type_id)
                        .ok_or(SemanticTranscriptError::MissingIdentity)?;
                    let mut hasher = TranscriptHasher::new(&mut self.budget);
                    transcript_update!(hasher, b"arcweft.lang.checked-pattern-type.v1\0");
                    transcript_update!(hasher, ty.semantic_identity_digest()?.as_bytes());
                    Some(
                        CheckedPatternSemanticDigest::from_bytes(hasher.finalize())
                            .as_bytes()
                            .to_vec(),
                    )
                }
            };
            if let Some(child) = child {
                children.push((
                    crate::semantic_coordinate::pattern_child_role_transcript_bytes(edge.role())?,
                    child,
                ));
            }
        }
        let mut hasher = TranscriptHasher::new(&mut self.budget);
        transcript_update!(hasher, b"arcweft.lang.checked-pattern-semantic.v1\0");
        write_bytes(&mut hasher, &coordinate)?;
        transcript_update!(hasher, &hir.kind().semantic_transcript_tag().to_le_bytes());
        transcript_update!(hasher, &checked.resolution().semantic_tag().to_le_bytes());
        transcript_update!(hasher, checked.ty().semantic_identity_digest()?.as_bytes());
        if let HirPatternKind::BracketSequence { rest, .. } = hir.kind() {
            transcript_update!(
                hasher,
                &[rest
                    .semantic_transcript_tag()
                    .ok_or(SemanticTranscriptError::RecoveredOwner)?],
            );
        }
        write_generic_pattern_resolution(
            &mut hasher,
            checked.resolution(),
            checked.ty(),
            analysis,
        )?;
        write_len(&mut hasher, children.len())?;
        for (role, digest) in children {
            write_bytes(&mut hasher, &role)?;
            transcript_update!(hasher, &digest);
        }
        let digest = CheckedPatternSemanticDigest::from_bytes(hasher.finalize());
        self.pattern_visiting.remove(&owner);
        self.pattern_digests.insert(owner, digest);
        Ok(digest)
    }

    fn statement_digest_at(
        &mut self,
        owner: arcweft_lang_hir::identity::StmtId,
        depth: u64,
    ) -> Result<CheckedStatementSemanticDigest, SemanticTranscriptError> {
        self.control.check()?;
        let analysis = self.analysis;
        let module = self.module;
        self.budget.observe_depth(depth)?;
        if let Some(digest) = self.statement_digests.get(&owner) {
            return Ok(*digest);
        }
        if !self.statement_visiting.insert(owner) {
            return Err(SemanticTranscriptError::MissingChildEdges);
        }
        let hir = module
            .resolve_stmt(owner)
            .map_err(|_| SemanticTranscriptError::MissingIdentity)?;
        if hir.is_poisoned() || matches!(hir.kind(), HirStmtKind::Error) {
            return Err(SemanticTranscriptError::RecoveredOwner);
        }
        let checked = analysis
            .statement(owner)
            .ok_or(SemanticTranscriptError::MissingIdentity)?;
        let mut children = Vec::new();
        for edge in hir
            .kind()
            .try_child_edges()
            .map_err(|_| SemanticTranscriptError::RecoveredOwner)?
        {
            let role =
                crate::semantic_coordinate::statement_child_role_transcript_bytes(edge.role())?;
            let child = match edge.child() {
                HirStatementChild::Expression(expression) => {
                    let digest = self.expression_digest_at(
                        expression,
                        depth
                            .checked_add(1)
                            .ok_or(SemanticTranscriptError::WorkLimit)?,
                    )?;
                    Some(digest.as_bytes().to_vec())
                }
                HirStatementChild::Pattern(pattern) => {
                    let digest = self.generic_pattern_digest(pattern)?;
                    Some(digest.as_bytes().to_vec())
                }
                HirStatementChild::Statement(_statement)
                    if matches!(edge.role(), HirStatementChildRole::BodyItem { .. }) =>
                {
                    None
                }
                HirStatementChild::Statement(statement) => {
                    let digest = self.statement_digest_at(
                        statement,
                        depth
                            .checked_add(1)
                            .ok_or(SemanticTranscriptError::WorkLimit)?,
                    )?;
                    Some(digest.as_bytes().to_vec())
                }
                HirStatementChild::Type(type_id) => Some(
                    analysis
                        .ty(type_id)
                        .ok_or(SemanticTranscriptError::MissingIdentity)?
                        .semantic_identity_digest()?
                        .as_bytes()
                        .to_vec(),
                ),
                HirStatementChild::Local(local) => {
                    let binding = analysis
                        .local(local)
                        .ok_or(SemanticTranscriptError::MissingIdentity)?;
                    let mut value = self.coordinates.binding(local)?.canonical_bytes()?;
                    value.extend_from_slice(binding.ty().semantic_identity_digest()?.as_bytes());
                    Some(value)
                }
            };
            children.push((role, child));
        }
        let mut bodies = Vec::new();
        for body in hir
            .kind()
            .body_projections()
            .map_err(|_| SemanticTranscriptError::RecoveredOwner)?
        {
            let (coordinate, digest) = self.body_digest_at(
                HirSemanticBodyOwner::statement_body(owner, *body.role()),
                body.projection(),
                None,
                depth
                    .checked_add(1)
                    .ok_or(SemanticTranscriptError::WorkLimit)?,
            )?;
            bodies.push((coordinate, digest));
        }
        let coordinate = self.coordinates.statement(owner)?.canonical_bytes()?;
        let mut hasher = TranscriptHasher::new(&mut self.budget);
        transcript_update!(hasher, b"arcweft.lang.checked-statement-semantic.v1\0");
        write_bytes(&mut hasher, &coordinate)?;
        transcript_update!(hasher, &hir.kind().semantic_transcript_tag().to_le_bytes());
        transcript_update!(hasher, &[checked.payload().semantic_tag()]);
        write_statement_payload(
            &mut hasher,
            checked.payload(),
            &coordinate,
            analysis,
            &self.coordinates,
        )?;
        write_effects(&mut hasher, checked.effects())?;
        write_len(&mut hasher, children.len())?;
        for (role, child) in children {
            write_bytes(&mut hasher, &role)?;
            match child {
                Some(child) => {
                    transcript_update!(hasher, &[1]);
                    write_bytes(&mut hasher, &child)?;
                }
                None => transcript_update!(hasher, &[0]),
            }
        }
        write_len(&mut hasher, bodies.len())?;
        for (coordinate, digest) in bodies {
            write_bytes(&mut hasher, &coordinate.canonical_bytes()?)?;
            transcript_update!(hasher, digest.as_bytes());
        }
        let digest = CheckedStatementSemanticDigest::from_bytes(hasher.finalize());
        self.statement_visiting.remove(&owner);
        self.statement_digests.insert(owner, digest);
        Ok(digest)
    }

    fn body_digest_at(
        &mut self,
        owner: HirSemanticBodyOwner,
        projection: &HirBodyProjection,
        root_override: Option<HirSemanticPathRoot>,
        depth: u64,
    ) -> Result<(StableCheckedBodyCoordinate, CheckedBodySemanticDigest), SemanticTranscriptError>
    {
        self.control.check()?;
        let analysis = self.analysis;
        let root = match root_override {
            Some(root) => root,
            None => match owner.expression_owner() {
                Some(expression) => analysis
                    .hir_topology()
                    .semantic_path(expression.into())
                    .map_err(|_| SemanticTranscriptError::MissingIdentity)?
                    .ok_or(SemanticTranscriptError::MissingIdentity)?
                    .root()
                    .clone(),
                None => match owner.statement_owner() {
                    Some(statement) => analysis
                        .hir_topology()
                        .semantic_path(statement.into())
                        .map_err(|_| SemanticTranscriptError::MissingIdentity)?
                        .ok_or(SemanticTranscriptError::MissingIdentity)?
                        .root()
                        .clone(),
                    None => return Err(SemanticTranscriptError::MissingIdentity),
                },
            },
        };
        let locator = HirSemanticBodyLocator::new(root, owner);
        let coordinate = self.coordinates.body(&locator)?;
        if let Some(digest) = self.body_digests.get(&coordinate) {
            return Ok((coordinate, *digest));
        }
        if !self.body_visiting.insert(coordinate.clone()) {
            return Err(SemanticTranscriptError::MissingChildEdges);
        }
        self.budget.observe_depth(depth)?;
        let mut children = Vec::new();
        for edge in projection.children() {
            let role = crate::semantic_coordinate::body_child_role_transcript_bytes(edge.role())?;
            let digest = match edge.child() {
                HirBodyChild::Expression(expression) => self
                    .expression_digest_at(
                        expression,
                        depth
                            .checked_add(1)
                            .ok_or(SemanticTranscriptError::WorkLimit)?,
                    )?
                    .as_bytes()
                    .to_owned(),
                HirBodyChild::Statement(statement) => self
                    .statement_digest_at(
                        statement,
                        depth
                            .checked_add(1)
                            .ok_or(SemanticTranscriptError::WorkLimit)?,
                    )?
                    .as_bytes()
                    .to_owned(),
            };
            children.push((role, digest));
        }
        let mut hasher = TranscriptHasher::new(&mut self.budget);
        transcript_update!(hasher, b"arcweft.lang.checked-body-semantic.v1\0");
        write_bytes(&mut hasher, &coordinate.canonical_bytes()?)?;
        transcript_update!(
            hasher,
            &[match projection.kind() {
                arcweft_lang_hir::body_edges::HirBodyKind::Expression => 0,
                arcweft_lang_hir::body_edges::HirBodyKind::Ordinary => 1,
                arcweft_lang_hir::body_edges::HirBodyKind::Thread => 2,
            }]
        );
        write_len(&mut hasher, children.len())?;
        for (role, digest) in children {
            write_bytes(&mut hasher, &role)?;
            transcript_update!(hasher, &digest);
        }
        let digest = CheckedBodySemanticDigest::from_bytes(hasher.finalize());
        self.body_visiting.remove(&coordinate);
        self.body_digests.insert(coordinate.clone(), digest);
        Ok((coordinate, digest))
    }

    fn rich_text_semantic_digest(
        &mut self,
        report: &CheckedRichTextReport,
        depth: u64,
    ) -> Result<CheckedRichTextSemanticDigest, SemanticTranscriptError> {
        self.control.check()?;
        self.budget.observe_depth(depth)?;
        if !report.is_valid() {
            return Err(SemanticTranscriptError::MissingIdentity);
        }
        let token_depth =
            depth
                .checked_add(1)
                .ok_or(CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::Depth,
                })?;
        let mut child_digests = BTreeMap::new();
        let mut content_body_digests = BTreeMap::new();
        self.collect_rich_text_expression_digests(
            report,
            token_depth,
            &mut child_digests,
            &mut content_body_digests,
        )?;
        let mut hasher = TranscriptHasher::new(&mut self.budget);
        transcript_update!(hasher, b"arcweft.lang.checked-rich-text-semantic.v1\0");
        let fragment_digest = report
            .fragment_coordinate()
            .semantic_digest()
            .map_err(|_| SemanticTranscriptError::MissingIdentity)?;
        transcript_update!(hasher, fragment_digest.as_bytes());
        transcript_update!(hasher, &[report.admission().semantic_tag()]);
        write_len(&mut hasher, report.content().tokens().len())?;
        for token in report.content().tokens() {
            transcript_update!(hasher, &[token.semantic_tag()]);
            write_rich_text_token(&mut hasher, token, &child_digests, &content_body_digests)?;
        }
        write_effect_plan(&mut hasher, report.effect_plan())?;
        Ok(CheckedRichTextSemanticDigest::from_bytes(hasher.finalize()))
    }

    fn collect_rich_text_expression_digests(
        &mut self,
        report: &CheckedRichTextReport,
        depth: u64,
        digests: &mut BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
        content_body_digests: &mut BTreeMap<ExprId, CheckedRichTextSemanticDigest>,
    ) -> Result<(), SemanticTranscriptError> {
        self.control.check()?;
        for token in report.content().tokens() {
            match token {
                CheckedDialogueToken::Interpolation { expression, .. } => {
                    self.remember_rich_text_expression(*expression, depth, digests)?;
                }
                CheckedDialogueToken::ContentInsert(insertion) => {
                    if let Some(checked_content) = insertion.argument().checked_content() {
                        let digest = self.rich_text_semantic_digest(checked_content, depth)?;
                        if content_body_digests
                            .insert(insertion.site().raw(), digest)
                            .is_some_and(|existing| existing != digest)
                        {
                            return Err(SemanticTranscriptError::MissingIdentity);
                        }
                    }
                    self.remember_rich_text_expression(insertion.site().raw(), depth, digests)?;
                }
                CheckedDialogueToken::PointAction(action) => {
                    self.collect_rich_text_action_expressions(action, depth, digests)?;
                }
                CheckedDialogueToken::Text(_)
                | CheckedDialogueToken::Escape(_)
                | CheckedDialogueToken::RawLiteral(_)
                | CheckedDialogueToken::LineBreak(_) => {}
            }
        }
        Ok(())
    }

    fn remember_rich_text_expression(
        &mut self,
        expression: ExprId,
        depth: u64,
        digests: &mut BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    ) -> Result<(), SemanticTranscriptError> {
        self.control.check()?;
        let digest = self.expression_digest_at(expression, depth)?;
        if digests
            .insert(expression, digest)
            .is_some_and(|existing| existing != digest)
        {
            return Err(SemanticTranscriptError::MissingIdentity);
        }
        Ok(())
    }

    fn collect_rich_text_action_expressions(
        &mut self,
        action: &crate::checked_rich_text::CheckedRichTextAction,
        depth: u64,
        digests: &mut BTreeMap<ExprId, CheckedExpressionSemanticDigest>,
    ) -> Result<(), SemanticTranscriptError> {
        self.control.check()?;
        if let CheckedRichTextAction::Host { action, fields, .. } = action {
            use crate::checked_rich_text::CheckedDialogueHostEvent;
            let expression = match action {
                CheckedDialogueHostEvent::TimedCue { call, .. }
                | CheckedDialogueHostEvent::Call { call } => Some(*call),
                CheckedDialogueHostEvent::Voice { .. }
                | CheckedDialogueHostEvent::Face { .. }
                | CheckedDialogueHostEvent::Pose { .. }
                | CheckedDialogueHostEvent::Show { .. }
                | CheckedDialogueHostEvent::Hide { .. }
                | CheckedDialogueHostEvent::Move { .. }
                | CheckedDialogueHostEvent::Scale { .. }
                | CheckedDialogueHostEvent::Rotate { .. }
                | CheckedDialogueHostEvent::Animation { .. }
                | CheckedDialogueHostEvent::Shake { .. }
                | CheckedDialogueHostEvent::Signal { .. } => None,
            };
            if let Some(expression) = expression {
                self.remember_rich_text_expression(expression, depth, digests)?;
            }
            for field in fields.fields() {
                if let CheckedFieldOrigin::TextProxyDefault { expression } = field.origin() {
                    self.remember_rich_text_expression(*expression, depth, digests)?;
                }
            }
        } else if let Some(fields) = action.fields() {
            for field in fields.fields() {
                if let CheckedFieldOrigin::TextProxyDefault { expression } = field.origin() {
                    self.remember_rich_text_expression(*expression, depth, digests)?;
                }
            }
        }
        Ok(())
    }
}
