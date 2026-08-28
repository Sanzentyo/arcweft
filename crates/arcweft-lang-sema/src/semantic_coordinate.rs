//! Stable structural coordinates shared by semantic-analysis authorities.
//!
//! This module owns the coordinate algebra and its canonical byte grammar.
//! HIR arena identities, spans, source spelling, callable facts, and final-
//! analysis state are deliberately outside this lower-level authority.

use crate::record_field::CheckedRecordFieldSemanticId;
use arcweft_lang_hir::{
    body_edges::{HirBodyChildRole, HirBodyKind},
    expr::{
        HirExpressionOwnedBodyRole, HirLinePlanStatementRole, HirNestedExpressionPath,
        HirNestedExpressionPathSegment,
    },
    identity::{ExprId, LocalId, PatternId, StmtId},
    pattern::HirPatternChildRole,
    project::{
        HirControlTransferKind, HirDeclarationBodyRootRole, HirDeclarationContractRootRole,
        HirDeclarationItemRootRole, HirFlowContractRootFamily, HirItemAttributeOwner,
        HirItemEvaluationEntryRole, HirItemRecoveryRootOwner, HirLayerExpressionRootField,
        HirLoopTargetFamily, HirSemanticBodyOwner, HirSemanticBodyOwnerRole,
        HirStyleRootPathSegment,
    },
    stmt::{HirStatementBodyRole, HirStatementChildRole},
};
use thiserror::Error;
mod catalog;

pub use catalog::AcceptedSemanticRootCatalogError;
pub(crate) use catalog::{
    AcceptedSemanticRootCatalog, CheckedExpressionEdgeAuthority, SemanticCoordinateIndex,
    SemanticCoordinateIndexError,
};

const CHECKED_DECLARATION_BODY_STEP_TAG: u8 = 8;
const CHECKED_EXPRESSION_OWNED_STEP_TAG: u8 = 9;
const CHECKED_DECLARATION_CONTRACT_STEP_TAG: u8 = 10;
const CHECKED_DECLARATION_ITEM_STEP_TAG: u8 = 11;
const CHECKED_DECLARATION_MEMBER_STEP_TAG: u8 = 12;
const CHECKED_DECLARATION_RESULT_STEP_TAG: u8 = 13;
const CHECKED_BODY_COORDINATE_SUFFIX_TAG: u8 = 0;
#[allow(dead_code)]
const CHECKED_OUTPUT_TARGET_COORDINATE_SUFFIX_TAG: u8 = 1;
#[allow(dead_code)]
const CHECKED_OUTPUT_TARGET_DIALOGUE_LINE_PLAN_FAMILY_TAG: u8 = 0;

/// Stable semantic identity of one accepted declaration root.
///
/// The bytes are minted by the sealed accepted-root catalog. HIR arena
/// identifiers, source spans, and retained display names are excluded.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedDeclarationSemanticId([u8; 32]);

impl AcceptedDeclarationSemanticId {
    const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable semantic identity of one accepted non-callable item root.
///
/// Item IDs, names, spans, and source content are deliberately absent.  The
/// catalog is the only authority allowed to mint this value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedItemSemanticId([u8; 32]);

impl AcceptedItemSemanticId {
    const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Accepted semantic root carried by every checked coordinate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedSemanticRoot {
    Declaration(AcceptedDeclarationSemanticId),
    Item(AcceptedItemSemanticId),
}

impl AcceptedSemanticRoot {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        match self {
            Self::Declaration(id) => id.as_bytes(),
            Self::Item(id) => id.as_bytes(),
        }
    }

    pub const fn tag(self) -> u8 {
        match self {
            Self::Declaration(_) => 0x00,
            Self::Item(_) => 0x01,
        }
    }

    fn write_canonical(&self, output: &mut Vec<u8>) {
        output.push(self.tag());
        output.extend_from_slice(self.as_bytes());
    }
}

/// A nonempty semantic path through a nested Choice/dialogue/line-plan owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedNestedPathV1(Box<[CheckedNestedPathSegmentV1]>);

impl CheckedNestedPathV1 {
    /// Constructs a checked path. Empty paths cannot identify a nested owner.
    pub fn try_from_segments(
        segments: Box<[CheckedNestedPathSegmentV1]>,
    ) -> Result<Self, CheckedNestedPathError> {
        (!segments.is_empty())
            .then_some(Self(segments))
            .ok_or(CheckedNestedPathError::Empty)
    }

    pub fn segments(&self) -> &[CheckedNestedPathSegmentV1] {
        &self.0
    }

    fn write_transcript(
        &self,
        output: &mut Vec<u8>,
    ) -> Result<(), SemanticCoordinateEncodingError> {
        write_len(output, self.0.len())?;
        for segment in &self.0 {
            segment.write_transcript(output);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedNestedPathError {
    Empty,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedNestedPathSegmentV1 {
    ChoiceBodyItem { ordinal: u32 },
    ChoiceIfBranch { ordinal: u32 },
    ChoiceIfElse,
    ChoiceForBody,
    ChoiceMatchArm { ordinal: u32 },
    ChoiceOptionBody,
    ChoiceOptionField { ordinal: u32 },
    ChoiceViewEntry { ordinal: u32 },
    ChoicePlanItem { ordinal: u32 },
    LinePlanItem { ordinal: u32 },
    LinePlanStartGroupItem { ordinal: u32 },
    LinePlanTogetherGroupItem { ordinal: u32 },
}

impl CheckedNestedPathSegmentV1 {
    pub const fn semantic_tag(&self) -> u8 {
        match self {
            Self::ChoiceBodyItem { .. } => 0,
            Self::ChoiceIfBranch { .. } => 1,
            Self::ChoiceIfElse => 2,
            Self::ChoiceForBody => 3,
            Self::ChoiceMatchArm { .. } => 4,
            Self::ChoiceOptionBody => 5,
            Self::ChoiceOptionField { .. } => 6,
            Self::ChoiceViewEntry { .. } => 7,
            Self::ChoicePlanItem { .. } => 8,
            Self::LinePlanItem { .. } => 9,
            Self::LinePlanStartGroupItem { .. } => 10,
            Self::LinePlanTogetherGroupItem { .. } => 11,
        }
    }

    fn write_transcript(&self, output: &mut Vec<u8>) {
        output.push(self.semantic_tag());
        match self {
            Self::ChoiceBodyItem { ordinal }
            | Self::ChoiceIfBranch { ordinal }
            | Self::ChoiceMatchArm { ordinal }
            | Self::ChoiceOptionField { ordinal }
            | Self::ChoiceViewEntry { ordinal }
            | Self::ChoicePlanItem { ordinal }
            | Self::LinePlanItem { ordinal }
            | Self::LinePlanStartGroupItem { ordinal }
            | Self::LinePlanTogetherGroupItem { ordinal } => {
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
            Self::ChoiceIfElse | Self::ChoiceForBody | Self::ChoiceOptionBody => {}
        }
    }
}

/// Semantic role attached to one checked expression child.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CheckedExpressionChildRole {
    Element {
        ordinal: u32,
    },
    RepeatedValue,
    RepeatLength,
    Callee,
    Argument {
        ordinal: u32,
    },
    Target,
    Index,
    PipeLeft,
    PipeRight,
    Operand,
    RangeStart,
    RangeEnd,
    RecordField {
        source_ordinal: u32,
        accepted_field: CheckedRecordFieldSemanticId,
    },
    BinaryLeft,
    BinaryRight,
    ClosureBody,
    BlockTail,
    LoopTail,
    Condition,
    ThenBranch,
    ElseBranch,
    Scrutinee,
    Guard {
        arm: u32,
    },
    ArmValue {
        arm: u32,
    },
    IfLetGuard,
    DialogueTarget,
    DialogueCoordinate {
        ordinal: u32,
    },
    DialogueInterpolation {
        ordinal: u32,
    },
    DialogueTagPayload {
        ordinal: u32,
    },
    LinePlanOptionValue {
        path: CheckedNestedPathV1,
    },
    LinePlanLetValue {
        path: CheckedNestedPathV1,
    },
    LinePlanOut {
        path: CheckedNestedPathV1,
    },
    LinePlanTimelineAssert {
        path: CheckedNestedPathV1,
    },
    LinePlanExpression {
        path: CheckedNestedPathV1,
    },
    PostfixIndexCandidate,
    PostfixDialogueCandidate,
    ForInput,
    ChoiceIfCondition {
        path: CheckedNestedPathV1,
        branch: u32,
    },
    ChoiceForSource {
        path: CheckedNestedPathV1,
    },
    ChoiceMatchScrutinee {
        path: CheckedNestedPathV1,
    },
    ChoiceMatchGuard {
        path: CheckedNestedPathV1,
        arm: u32,
    },
    ChoiceOptionId {
        path: CheckedNestedPathV1,
    },
    ChoiceOptionForSource {
        path: CheckedNestedPathV1,
    },
    ChoiceCompactLabel {
        path: CheckedNestedPathV1,
    },
    ChoiceCompactCondition {
        path: CheckedNestedPathV1,
    },
    ChoiceCompactOut {
        path: CheckedNestedPathV1,
    },
    ChoiceOptionLabel {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionFieldId {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionValue {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionVisible {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionEnabled {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionOrder {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionHotkey {
        path: CheckedNestedPathV1,
        field: u32,
    },
    ChoiceOptionViewKey {
        path: CheckedNestedPathV1,
        field: u32,
        entry: u32,
    },
    ChoiceOptionViewValue {
        path: CheckedNestedPathV1,
        field: u32,
        entry: u32,
    },
    ChoicePlanAssignment {
        item: u32,
    },
    ChoicePlanTimeout {
        item: u32,
    },
    ChoicePlanCancelSignal {
        item: u32,
    },
    ChoicePlanCancelTimeout {
        item: u32,
    },
    ChoicePlanCancelExpr {
        item: u32,
    },
}

impl CheckedExpressionChildRole {
    pub(crate) const fn semantic_tag(&self) -> u16 {
        match self {
            Self::Element { .. } => 0x1000,
            Self::RepeatedValue => 0x1001,
            Self::RepeatLength => 0x1002,
            Self::Callee => 0x1003,
            Self::Argument { .. } => 0x1004,
            Self::Target => 0x1005,
            Self::Index => 0x1006,
            Self::PipeLeft => 0x1007,
            Self::PipeRight => 0x1008,
            Self::Operand => 0x1009,
            Self::RangeStart => 0x100A,
            Self::RangeEnd => 0x100B,
            Self::RecordField { .. } => 0x100C,
            Self::BinaryLeft => 0x100D,
            Self::BinaryRight => 0x100E,
            Self::ClosureBody => 0x100F,
            Self::BlockTail => 0x1010,
            Self::LoopTail => 0x1011,
            Self::Condition => 0x1012,
            Self::ThenBranch => 0x1013,
            Self::ElseBranch => 0x1014,
            Self::Scrutinee => 0x1015,
            Self::Guard { .. } => 0x1016,
            Self::ArmValue { .. } => 0x1017,
            Self::IfLetGuard => 0x1018,
            Self::DialogueTarget => 0x1019,
            Self::DialogueCoordinate { .. } => 0x101A,
            Self::DialogueInterpolation { .. } => 0x101B,
            Self::DialogueTagPayload { .. } => 0x101C,
            Self::LinePlanOptionValue { .. } => 0x101D,
            Self::LinePlanLetValue { .. } => 0x101E,
            Self::LinePlanOut { .. } => 0x101F,
            Self::LinePlanTimelineAssert { .. } => 0x1020,
            Self::LinePlanExpression { .. } => 0x1021,
            Self::PostfixIndexCandidate => 0x1024,
            Self::PostfixDialogueCandidate => 0x1025,
            Self::ForInput => 0x1026,
            Self::ChoiceIfCondition { .. } => 0x1027,
            Self::ChoiceForSource { .. } => 0x1028,
            Self::ChoiceMatchScrutinee { .. } => 0x1029,
            Self::ChoiceMatchGuard { .. } => 0x102A,
            Self::ChoiceOptionId { .. } => 0x102B,
            Self::ChoiceOptionForSource { .. } => 0x102C,
            Self::ChoiceCompactLabel { .. } => 0x102D,
            Self::ChoiceCompactCondition { .. } => 0x102E,
            Self::ChoiceCompactOut { .. } => 0x102F,
            Self::ChoiceOptionLabel { .. } => 0x1030,
            Self::ChoiceOptionFieldId { .. } => 0x1031,
            Self::ChoiceOptionValue { .. } => 0x1032,
            Self::ChoiceOptionVisible { .. } => 0x1033,
            Self::ChoiceOptionEnabled { .. } => 0x1034,
            Self::ChoiceOptionOrder { .. } => 0x1035,
            Self::ChoiceOptionHotkey { .. } => 0x1036,
            Self::ChoiceOptionViewKey { .. } => 0x1037,
            Self::ChoiceOptionViewValue { .. } => 0x1038,
            Self::ChoicePlanAssignment { .. } => 0x1039,
            Self::ChoicePlanTimeout { .. } => 0x103A,
            Self::ChoicePlanCancelSignal { .. } => 0x103B,
            Self::ChoicePlanCancelTimeout { .. } => 0x103C,
            Self::ChoicePlanCancelExpr { .. } => 0x103D,
        }
    }

    pub(crate) fn write_transcript(
        &self,
        output: &mut Vec<u8>,
    ) -> Result<(), SemanticCoordinateEncodingError> {
        output.extend_from_slice(&self.semantic_tag().to_le_bytes());
        match self {
            Self::Element { ordinal }
            | Self::Argument { ordinal }
            | Self::DialogueCoordinate { ordinal }
            | Self::DialogueInterpolation { ordinal }
            | Self::DialogueTagPayload { ordinal }
            | Self::ChoicePlanAssignment { item: ordinal }
            | Self::ChoicePlanTimeout { item: ordinal }
            | Self::ChoicePlanCancelSignal { item: ordinal }
            | Self::ChoicePlanCancelTimeout { item: ordinal }
            | Self::ChoicePlanCancelExpr { item: ordinal } => {
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
            Self::RecordField {
                source_ordinal,
                accepted_field,
            } => {
                output.extend_from_slice(&source_ordinal.to_le_bytes());
                output.extend_from_slice(accepted_field.as_bytes());
            }
            Self::Guard { arm } | Self::ArmValue { arm } => {
                output.extend_from_slice(&arm.to_le_bytes());
            }
            Self::ChoiceIfCondition { path, branch } => {
                output.extend_from_slice(&branch.to_le_bytes());
                path.write_transcript(output)?;
            }
            Self::ChoiceMatchGuard { path, arm } => {
                output.extend_from_slice(&arm.to_le_bytes());
                path.write_transcript(output)?;
            }
            Self::LinePlanOptionValue { path }
            | Self::LinePlanLetValue { path }
            | Self::LinePlanOut { path }
            | Self::LinePlanTimelineAssert { path }
            | Self::LinePlanExpression { path }
            | Self::ChoiceForSource { path }
            | Self::ChoiceMatchScrutinee { path }
            | Self::ChoiceOptionId { path }
            | Self::ChoiceOptionForSource { path }
            | Self::ChoiceCompactLabel { path }
            | Self::ChoiceCompactCondition { path }
            | Self::ChoiceCompactOut { path } => path.write_transcript(output)?,
            Self::ChoiceOptionLabel { path, field }
            | Self::ChoiceOptionFieldId { path, field }
            | Self::ChoiceOptionValue { path, field }
            | Self::ChoiceOptionVisible { path, field }
            | Self::ChoiceOptionEnabled { path, field }
            | Self::ChoiceOptionOrder { path, field }
            | Self::ChoiceOptionHotkey { path, field } => {
                path.write_transcript(output)?;
                output.extend_from_slice(&field.to_le_bytes());
            }
            Self::ChoiceOptionViewKey { path, field, entry }
            | Self::ChoiceOptionViewValue { path, field, entry } => {
                path.write_transcript(output)?;
                output.extend_from_slice(&field.to_le_bytes());
                output.extend_from_slice(&entry.to_le_bytes());
            }
            Self::RepeatedValue
            | Self::RepeatLength
            | Self::Callee
            | Self::Target
            | Self::Index
            | Self::PipeLeft
            | Self::PipeRight
            | Self::Operand
            | Self::RangeStart
            | Self::RangeEnd
            | Self::BinaryLeft
            | Self::BinaryRight
            | Self::ClosureBody
            | Self::BlockTail
            | Self::LoopTail
            | Self::Condition
            | Self::ThenBranch
            | Self::ElseBranch
            | Self::Scrutinee
            | Self::IfLetGuard
            | Self::DialogueTarget
            | Self::PostfixIndexCandidate
            | Self::PostfixDialogueCandidate
            | Self::ForInput => {}
        }
        Ok(())
    }

    pub(crate) fn transcript_bytes(&self) -> Result<Vec<u8>, SemanticCoordinateEncodingError> {
        let mut bytes = Vec::new();
        self.write_transcript(&mut bytes)?;
        Ok(bytes)
    }
}

/// One structural step in a declaration-rooted checked semantic path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CheckedSemanticPathStep {
    DeclarationBody(HirDeclarationBodyRootRole),
    DeclarationContract(HirDeclarationContractRootRole),
    DeclarationItem(HirDeclarationItemRootRole),
    ExpressionOwned(HirExpressionOwnedBodyRole),
    Body(HirBodyChildRole),
    Statement(HirStatementChildRole),
    StatementBody(HirStatementBodyRole),
    Expression(CheckedExpressionChildRole),
    MatchPattern { arm: u32 },
    Pattern(HirPatternChildRole),
    ParameterPattern { group: u32, parameter: u32 },
    ParameterDefault { group: u32, parameter: u32 },
    DeclarationMember { member: u32 },
    DeclarationResult,
}

/// Stable accepted-rooted path for a checked semantic owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedSemanticPath {
    root: AcceptedSemanticRoot,
    steps: Box<[CheckedSemanticPathStep]>,
}

impl CheckedSemanticPath {
    pub(crate) fn new(
        root: AcceptedSemanticRoot,
        steps: impl Into<Box<[CheckedSemanticPathStep]>>,
    ) -> Self {
        Self {
            root,
            steps: steps.into(),
        }
    }

    pub const fn root(&self) -> AcceptedSemanticRoot {
        self.root
    }

    pub(crate) fn steps(&self) -> &[CheckedSemanticPathStep] {
        &self.steps
    }

    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>, SemanticCoordinateEncodingError> {
        let mut output = Vec::new();
        self.root.write_canonical(&mut output);
        write_len(&mut output, self.steps.len())?;
        for step in &self.steps {
            write_checked_path_step(&mut output, step)?;
        }
        Ok(output)
    }
}

/// Opaque accepted-rooted coordinate carried by compiler-local semantic
/// construction failures. It deliberately exposes neither HIR arena owners
/// nor a serialization contract.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StableSemanticCoordinate {
    owner: CheckedSemanticPath,
    arm: Option<u32>,
    pattern: Option<StablePatternCoordinate>,
}

impl StableSemanticCoordinate {
    pub(crate) const fn new(path: CheckedSemanticPath) -> Self {
        Self {
            owner: path,
            arm: None,
            pattern: None,
        }
    }

    pub(crate) fn pattern(
        path: CheckedSemanticPath,
        arm: u32,
        pattern: StablePatternCoordinate,
    ) -> Self {
        Self {
            owner: path,
            arm: Some(arm),
            pattern: Some(pattern),
        }
    }
}

impl From<CheckedSemanticPath> for StableSemanticCoordinate {
    fn from(path: CheckedSemanticPath) -> Self {
        Self::new(path)
    }
}

/// Move-only evidence that one generation-local expression owner resolves to
/// one accepted stable semantic coordinate.
///
/// Only [`SemanticCoordinateIndex`] can mint this pair. Checked semantic
/// carriers consume it instead of accepting an independently selected HIR
/// owner and stable coordinate.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CheckedExpressionCoordinateEvidence {
    owner: ExprId,
    coordinate: CheckedSemanticPath,
}

impl CheckedExpressionCoordinateEvidence {
    fn new(owner: ExprId, coordinate: CheckedSemanticPath) -> Self {
        Self { owner, coordinate }
    }

    pub(crate) const fn owner(&self) -> ExprId {
        self.owner
    }

    pub(crate) fn into_coordinate(self) -> CheckedSemanticPath {
        self.coordinate
    }
}

/// Stable accepted-rooted coordinate for one checked HIR pattern owner.
///
/// This is distinct from [`StablePatternCoordinate`], which addresses a
/// pattern relative to one Match arm after checked field identities have been
/// substituted. The owner coordinate names the HIR pattern's unique place in
/// the accepted project topology.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(
    dead_code,
    reason = "the semantic transcript graph consumes this typed owner coordinate before raw pattern IDs are removed"
)]
pub(crate) struct StableCheckedPatternOwnerCoordinate {
    path: CheckedSemanticPath,
}

#[allow(
    dead_code,
    reason = "the semantic transcript graph consumes this typed owner coordinate before raw pattern IDs are removed"
)]
impl StableCheckedPatternOwnerCoordinate {
    fn new(path: CheckedSemanticPath) -> Self {
        Self { path }
    }

    pub(crate) const fn path(&self) -> &CheckedSemanticPath {
        &self.path
    }

    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>, SemanticCoordinateEncodingError> {
        self.path.canonical_bytes()
    }
}

/// Move-only proof that a generation-local pattern owner was resolved by the
/// sealed accepted-root issuer.
#[derive(Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "checked pattern transcript publication will consume the owner and coordinate together"
)]
pub(crate) struct CheckedPatternCoordinateEvidence {
    owner: PatternId,
    coordinate: StableCheckedPatternOwnerCoordinate,
}

#[allow(
    dead_code,
    reason = "checked pattern transcript publication will consume the owner and coordinate together"
)]
impl CheckedPatternCoordinateEvidence {
    fn new(owner: PatternId, coordinate: StableCheckedPatternOwnerCoordinate) -> Self {
        Self { owner, coordinate }
    }

    pub(crate) const fn owner(&self) -> PatternId {
        self.owner
    }

    pub(crate) fn into_coordinate(self) -> StableCheckedPatternOwnerCoordinate {
        self.coordinate
    }
}

/// Stable accepted-rooted coordinate for one checked HIR statement owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(
    dead_code,
    reason = "the semantic transcript graph consumes this typed coordinate when statement digests publish"
)]
pub(crate) struct StableCheckedStatementCoordinate {
    path: CheckedSemanticPath,
}

#[allow(
    dead_code,
    reason = "the semantic transcript graph consumes this typed coordinate when statement digests publish"
)]
impl StableCheckedStatementCoordinate {
    fn new(path: CheckedSemanticPath) -> Self {
        Self { path }
    }

    pub(crate) const fn path(&self) -> &CheckedSemanticPath {
        &self.path
    }

    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>, SemanticCoordinateEncodingError> {
        self.path.canonical_bytes()
    }
}

/// Move-only proof that a generation-local statement owner was resolved by
/// the sealed accepted-root issuer.
#[derive(Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "checked statement transcript publication will consume the owner and coordinate together"
)]
pub(crate) struct CheckedStatementCoordinateEvidence {
    owner: StmtId,
    coordinate: StableCheckedStatementCoordinate,
}

#[allow(
    dead_code,
    reason = "checked statement transcript publication will consume the owner and coordinate together"
)]
impl CheckedStatementCoordinateEvidence {
    fn new(owner: StmtId, coordinate: StableCheckedStatementCoordinate) -> Self {
        Self { owner, coordinate }
    }

    pub(crate) const fn owner(&self) -> StmtId {
        self.owner
    }

    pub(crate) fn into_coordinate(self) -> StableCheckedStatementCoordinate {
        self.coordinate
    }
}

/// Stable accepted-rooted coordinate for one executable body container.
///
/// Raw expression and statement owners are lookup evidence only. The stable
/// grammar retains the typed body-owner family/role, body kind, and checked
/// accepted path so body containers that legitimately share a path remain
/// distinct.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(
    dead_code,
    reason = "the semantic transcript graph consumes this typed coordinate when body digests publish"
)]
pub(crate) struct StableCheckedBodyCoordinate {
    owner: HirSemanticBodyOwnerRole,
    kind: HirBodyKind,
    path: CheckedSemanticPath,
}

#[allow(
    dead_code,
    reason = "the semantic transcript graph consumes this typed coordinate when body digests publish"
)]
impl StableCheckedBodyCoordinate {
    fn new(owner: &HirSemanticBodyOwner, kind: HirBodyKind, path: CheckedSemanticPath) -> Self {
        Self {
            owner: owner.semantic_role(),
            kind,
            path,
        }
    }

    pub(crate) const fn path(&self) -> &CheckedSemanticPath {
        &self.path
    }

    pub(crate) const fn kind(&self) -> HirBodyKind {
        self.kind
    }

    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>, SemanticCoordinateEncodingError> {
        let mut output = self.path.canonical_bytes()?;
        output.push(CHECKED_BODY_COORDINATE_SUFFIX_TAG);
        write_body_owner_role(&mut output, &self.owner)?;
        output.push(match self.kind {
            HirBodyKind::Expression => 0,
            HirBodyKind::Ordinary => 1,
            HirBodyKind::Thread => 2,
        });
        Ok(output)
    }
}

/// Stable accepted-rooted coordinate for one checked line-plan output target.
///
/// Output targets use the accepted application path directly and append a
/// distinct coordinate suffix. This keeps an output target disjoint from the
/// body-coordinate grammar even when both coordinates share a path prefix.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(
    dead_code,
    reason = "the control-transfer coordinate is consumed by the subsequent checked statement cut"
)]
pub(crate) struct StableCheckedOutputTargetCoordinate {
    application: CheckedSemanticPath,
}

#[allow(
    dead_code,
    reason = "the control-transfer coordinate is consumed by the subsequent checked statement cut"
)]
impl StableCheckedOutputTargetCoordinate {
    fn new(application: CheckedSemanticPath) -> Self {
        Self { application }
    }

    pub(crate) const fn application(&self) -> &CheckedSemanticPath {
        &self.application
    }

    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>, SemanticCoordinateEncodingError> {
        let mut output = self.application.canonical_bytes()?;
        output.push(CHECKED_OUTPUT_TARGET_COORDINATE_SUFFIX_TAG);
        output.push(CHECKED_OUTPUT_TARGET_DIALOGUE_LINE_PLAN_FAMILY_TAG);
        Ok(output)
    }
}

fn write_body_owner_role(
    output: &mut Vec<u8>,
    owner: &HirSemanticBodyOwnerRole,
) -> Result<(), SemanticCoordinateEncodingError> {
    match owner {
        HirSemanticBodyOwnerRole::Declaration(role) => {
            output.push(0);
            write_declaration_body_role(output, *role);
        }
        HirSemanticBodyOwnerRole::Item(role) => {
            output.push(1);
            write_declaration_item_role(output, role)?;
        }
        HirSemanticBodyOwnerRole::Expression => output.push(2),
        HirSemanticBodyOwnerRole::ExpressionOwned(role) => {
            output.push(3);
            write_expression_owned_role(output, role)?;
        }
        HirSemanticBodyOwnerRole::Statement(role) => {
            output.push(4);
            output.push(statement_body_tag(*role));
            write_statement_body_payload(output, *role);
        }
    }
    Ok(())
}

/// Move-only proof that one generation-local body locator and its stable
/// coordinate were issued together by the sealed accepted-root catalog.
#[derive(Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "checked body transcript publication consumes this affine evidence"
)]
pub(crate) struct CheckedBodyCoordinateEvidence {
    locator: arcweft_lang_hir::project::HirSemanticBodyLocator,
    coordinate: StableCheckedBodyCoordinate,
}

#[allow(
    dead_code,
    reason = "checked body transcript publication consumes this affine evidence"
)]
impl CheckedBodyCoordinateEvidence {
    fn new(
        locator: arcweft_lang_hir::project::HirSemanticBodyLocator,
        coordinate: StableCheckedBodyCoordinate,
    ) -> Self {
        Self {
            locator,
            coordinate,
        }
    }

    pub(crate) const fn locator(&self) -> &arcweft_lang_hir::project::HirSemanticBodyLocator {
        &self.locator
    }

    pub(crate) fn into_coordinate(self) -> StableCheckedBodyCoordinate {
        self.coordinate
    }
}

/// Checked target selected for one accepted line-plan `out` statement.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(
    dead_code,
    reason = "the control-transfer target is consumed by the subsequent checked statement cut"
)]
pub(crate) struct CheckedOutputTarget {
    coordinate: StableCheckedOutputTargetCoordinate,
}

#[allow(
    dead_code,
    reason = "the control-transfer target is consumed by the subsequent checked statement cut"
)]
impl CheckedOutputTarget {
    fn new(coordinate: StableCheckedOutputTargetCoordinate) -> Self {
        Self { coordinate }
    }

    pub(crate) const fn coordinate(&self) -> &StableCheckedOutputTargetCoordinate {
        &self.coordinate
    }
}

/// Checked loop target selected for one accepted `break` or `continue`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(
    dead_code,
    reason = "the control-transfer target is consumed by the subsequent checked statement cut"
)]
pub(crate) struct CheckedLoopControlTarget {
    family: HirLoopTargetFamily,
    body: StableCheckedBodyCoordinate,
}

#[allow(
    dead_code,
    reason = "the control-transfer target is consumed by the subsequent checked statement cut"
)]
impl CheckedLoopControlTarget {
    fn new(family: HirLoopTargetFamily, body: StableCheckedBodyCoordinate) -> Self {
        Self { family, body }
    }

    pub(crate) const fn family(&self) -> HirLoopTargetFamily {
        self.family
    }

    pub(crate) const fn body(&self) -> &StableCheckedBodyCoordinate {
        &self.body
    }
}

/// Closed checked target vocabulary for the HIR control-transfer authority.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(
    dead_code,
    reason = "the control-transfer target is consumed by the subsequent checked statement cut"
)]
pub(crate) enum CheckedControlTransferTarget {
    Output(CheckedOutputTarget),
    Loop(CheckedLoopControlTarget),
}

#[allow(
    dead_code,
    reason = "the control-transfer target is consumed by the subsequent checked statement cut"
)]
impl CheckedControlTransferTarget {
    pub(crate) const fn output(&self) -> Option<&CheckedOutputTarget> {
        match self {
            Self::Output(target) => Some(target),
            Self::Loop(_) => None,
        }
    }

    pub(crate) const fn loop_target(&self) -> Option<&CheckedLoopControlTarget> {
        match self {
            Self::Output(_) => None,
            Self::Loop(target) => Some(target),
        }
    }
}

/// Affine evidence that one HIR control-transfer statement owns one checked
/// target and operation kind. The issuer is the only constructor, so callers
/// cannot pair a statement with an independently selected target coordinate.
#[derive(Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "the control-transfer evidence is consumed by the subsequent checked statement cut"
)]
pub(crate) struct CheckedControlTransferEvidence {
    owner: StmtId,
    kind: HirControlTransferKind,
    target: CheckedControlTransferTarget,
}

#[allow(
    dead_code,
    reason = "the control-transfer evidence is consumed by the subsequent checked statement cut"
)]
impl CheckedControlTransferEvidence {
    fn new(
        owner: StmtId,
        kind: HirControlTransferKind,
        target: CheckedControlTransferTarget,
    ) -> Self {
        Self {
            owner,
            kind,
            target,
        }
    }

    pub(crate) const fn owner(&self) -> StmtId {
        self.owner
    }

    pub(crate) const fn kind(&self) -> HirControlTransferKind {
        self.kind
    }

    pub(crate) const fn target(&self) -> &CheckedControlTransferTarget {
        &self.target
    }

    pub(crate) fn into_target(self) -> CheckedControlTransferTarget {
        self.target
    }
}

/// Stable coordinate for one accepted checked binding.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableCheckedBindingCoordinate {
    path: CheckedSemanticPath,
}

impl StableCheckedBindingCoordinate {
    pub(crate) fn new(path: CheckedSemanticPath) -> Self {
        Self { path }
    }

    pub const fn path(&self) -> &CheckedSemanticPath {
        &self.path
    }

    pub const fn root(&self) -> AcceptedSemanticRoot {
        self.path.root()
    }

    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>, SemanticCoordinateEncodingError> {
        self.path.canonical_bytes()
    }
}

/// Move-only evidence that one generation-local binding owner resolves to one
/// accepted stable binding coordinate.
///
/// Construction remains inside [`SemanticCoordinateIndex`], so checked
/// callable identity sealing cannot pair a local with an unrelated binding
/// coordinate.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CheckedBindingCoordinateEvidence {
    owner: LocalId,
    coordinate: StableCheckedBindingCoordinate,
}

impl CheckedBindingCoordinateEvidence {
    fn new(owner: LocalId, coordinate: StableCheckedBindingCoordinate) -> Self {
        Self { owner, coordinate }
    }

    pub(crate) const fn owner(&self) -> LocalId {
        self.owner
    }

    pub(crate) const fn coordinate(&self) -> &StableCheckedBindingCoordinate {
        &self.coordinate
    }

    pub(crate) fn into_coordinate(self) -> StableCheckedBindingCoordinate {
        self.coordinate
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum StablePatternCoordinateStep {
    TupleElement(u32),
    RecordField {
        field: CheckedRecordFieldSemanticId,
        source_ordinal: u32,
    },
    SequenceElement(u32),
    VariantPayload,
    WholeBindingInner,
    OrAlternative(u32),
    TypedBindingInner,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StablePatternCoordinate(Box<[StablePatternCoordinateStep]>);

impl StablePatternCoordinate {
    pub(crate) fn new(steps: impl Into<Box<[StablePatternCoordinateStep]>>) -> Self {
        Self(steps.into())
    }

    pub(crate) fn steps(&self) -> &[StablePatternCoordinateStep] {
        &self.0
    }

    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>, SemanticCoordinateEncodingError> {
        let mut output = Vec::new();
        write_len(&mut output, self.0.len())?;
        for step in &self.0 {
            match step {
                StablePatternCoordinateStep::TupleElement(ordinal) => {
                    output.push(0);
                    output.extend_from_slice(&ordinal.to_le_bytes());
                }
                StablePatternCoordinateStep::RecordField {
                    field,
                    source_ordinal,
                } => {
                    output.push(1);
                    output.extend_from_slice(field.as_bytes());
                    output.extend_from_slice(&source_ordinal.to_le_bytes());
                }
                StablePatternCoordinateStep::SequenceElement(ordinal) => {
                    output.push(2);
                    output.extend_from_slice(&ordinal.to_le_bytes());
                }
                StablePatternCoordinateStep::VariantPayload => output.push(3),
                StablePatternCoordinateStep::WholeBindingInner => output.push(4),
                StablePatternCoordinateStep::OrAlternative(ordinal) => {
                    output.push(5);
                    output.extend_from_slice(&ordinal.to_le_bytes());
                }
                StablePatternCoordinateStep::TypedBindingInner => output.push(6),
            }
        }
        Ok(output)
    }
}

/// Stable coordinate for a checked value retained by Match semantics.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StableCheckedValueCoordinate {
    Expression(CheckedSemanticPath),
    Binding(StableCheckedBindingCoordinate),
}

impl StableCheckedValueCoordinate {
    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>, SemanticCoordinateEncodingError> {
        let mut output = Vec::new();
        self.write_canonical(&mut output)?;
        Ok(output)
    }

    fn write_canonical(&self, output: &mut Vec<u8>) -> Result<(), SemanticCoordinateEncodingError> {
        match self {
            Self::Expression(path) => {
                output.push(0);
                output.extend_from_slice(&path.canonical_bytes()?);
            }
            Self::Binding(binding) => {
                output.push(1);
                output.extend_from_slice(&binding.canonical_bytes()?);
            }
        }
        Ok(())
    }
}

fn write_checked_path_step(
    output: &mut Vec<u8>,
    step: &CheckedSemanticPathStep,
) -> Result<(), SemanticCoordinateEncodingError> {
    match step {
        CheckedSemanticPathStep::DeclarationBody(role) => {
            output.push(CHECKED_DECLARATION_BODY_STEP_TAG);
            write_declaration_body_role(output, *role);
        }
        CheckedSemanticPathStep::DeclarationContract(role) => {
            output.push(CHECKED_DECLARATION_CONTRACT_STEP_TAG);
            write_declaration_contract_role(output, *role);
        }
        CheckedSemanticPathStep::DeclarationItem(role) => {
            output.push(CHECKED_DECLARATION_ITEM_STEP_TAG);
            write_declaration_item_role(output, role)?;
        }
        CheckedSemanticPathStep::ExpressionOwned(role) => {
            output.push(CHECKED_EXPRESSION_OWNED_STEP_TAG);
            write_expression_owned_role(output, role)?;
        }
        CheckedSemanticPathStep::Body(role) => {
            output.extend_from_slice(&[0, body_role_tag(*role)]);
            write_body_role_payload(output, *role);
        }
        CheckedSemanticPathStep::Statement(role) => {
            output.extend_from_slice(&[1, statement_role_tag(*role)]);
            write_statement_role_payload(output, *role);
        }
        CheckedSemanticPathStep::StatementBody(role) => {
            output.extend_from_slice(&[7, statement_body_tag(*role)]);
            write_statement_body_payload(output, *role);
        }
        CheckedSemanticPathStep::Expression(role) => {
            output.push(2);
            write_bytes(output, &role.transcript_bytes()?)?;
        }
        CheckedSemanticPathStep::MatchPattern { arm } => {
            output.push(3);
            output.extend_from_slice(&arm.to_le_bytes());
        }
        CheckedSemanticPathStep::Pattern(role) => {
            output.extend_from_slice(&[4, pattern_role_tag(*role)]);
            write_pattern_role_payload(output, *role);
        }
        CheckedSemanticPathStep::ParameterPattern { group, parameter } => {
            output.push(5);
            output.extend_from_slice(&group.to_le_bytes());
            output.extend_from_slice(&parameter.to_le_bytes());
        }
        CheckedSemanticPathStep::ParameterDefault { group, parameter } => {
            output.push(6);
            output.extend_from_slice(&group.to_le_bytes());
            output.extend_from_slice(&parameter.to_le_bytes());
        }
        CheckedSemanticPathStep::DeclarationMember { member } => {
            output.push(CHECKED_DECLARATION_MEMBER_STEP_TAG);
            output.extend_from_slice(&member.to_le_bytes());
        }
        CheckedSemanticPathStep::DeclarationResult => {
            output.push(CHECKED_DECLARATION_RESULT_STEP_TAG);
        }
    }
    Ok(())
}

fn write_declaration_body_role(output: &mut Vec<u8>, role: HirDeclarationBodyRootRole) {
    match role {
        HirDeclarationBodyRootRole::FunctionBody => output.push(0),
        HirDeclarationBodyRootRole::PredicateBody => output.push(1),
        HirDeclarationBodyRootRole::ProofBody => output.push(2),
        HirDeclarationBodyRootRole::FlowBody => output.push(3),
        HirDeclarationBodyRootRole::ImplFunctionBody => output.push(4),
        HirDeclarationBodyRootRole::ViewValue { ordinal } => {
            output.push(5);
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
    }
}

fn write_declaration_contract_role(output: &mut Vec<u8>, role: HirDeclarationContractRootRole) {
    match role {
        HirDeclarationContractRootRole::Requires { ordinal } => {
            output.push(0);
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirDeclarationContractRootRole::Ensures { ordinal } => {
            output.push(1);
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirDeclarationContractRootRole::Invariant { ordinal } => {
            output.push(2);
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirDeclarationContractRootRole::Assume => output.push(3),
        HirDeclarationContractRootRole::Reads { ordinal } => {
            output.push(4);
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirDeclarationContractRootRole::Modifies { ordinal } => {
            output.push(5);
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirDeclarationContractRootRole::Decreases => output.push(6),
        HirDeclarationContractRootRole::EffectOperand {
            clause,
            family,
            operand,
        } => {
            output.push(7);
            output.extend_from_slice(&clause.to_le_bytes());
            output.push(flow_contract_family_tag(family));
            output.extend_from_slice(&operand.to_le_bytes());
        }
    }
}

fn write_declaration_item_role(
    output: &mut Vec<u8>,
    role: &HirDeclarationItemRootRole,
) -> Result<(), SemanticCoordinateEncodingError> {
    match role {
        HirDeclarationItemRootRole::AttributeArgument {
            owner,
            attribute,
            argument,
        } => {
            output.push(0);
            write_item_attribute_owner(output, *owner);
            output.extend_from_slice(&attribute.to_le_bytes());
            output.extend_from_slice(&argument.to_le_bytes());
        }
        HirDeclarationItemRootRole::ActivityRequires { ordinal } => {
            output.push(1);
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirDeclarationItemRootRole::ActivityEnsures { ordinal } => {
            output.push(2);
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirDeclarationItemRootRole::ResourceField { field } => {
            output.push(3);
            output.extend_from_slice(&field.to_le_bytes());
        }
        HirDeclarationItemRootRole::CharacterDisplayName { member } => {
            output.push(4);
            output.extend_from_slice(&member.to_le_bytes());
        }
        HirDeclarationItemRootRole::MetricUnit { member } => {
            output.push(5);
            output.extend_from_slice(&member.to_le_bytes());
        }
        HirDeclarationItemRootRole::MetricBuckets { member, ordinal } => {
            output.push(6);
            output.extend_from_slice(&member.to_le_bytes());
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirDeclarationItemRootRole::LayerField { member, field } => {
            output.push(7);
            output.extend_from_slice(&member.to_le_bytes());
            output.push(match field {
                HirLayerExpressionRootField::Z => 0,
                HirLayerExpressionRootField::Visible => 1,
                HirLayerExpressionRootField::Transform => 2,
            });
        }
        HirDeclarationItemRootRole::EntryOption { member } => {
            output.push(8);
            output.extend_from_slice(&member.to_le_bytes());
        }
        HirDeclarationItemRootRole::Style { path } => {
            output.push(9);
            write_len(output, path.segments().len())?;
            for segment in path.segments() {
                match segment {
                    HirStyleRootPathSegment::Token { ordinal } => {
                        output.push(0);
                        output.extend_from_slice(&ordinal.to_le_bytes());
                    }
                    HirStyleRootPathSegment::Rule { ordinal } => {
                        output.push(1);
                        output.extend_from_slice(&ordinal.to_le_bytes());
                    }
                    HirStyleRootPathSegment::Declaration { ordinal } => {
                        output.push(2);
                        output.extend_from_slice(&ordinal.to_le_bytes());
                    }
                    HirStyleRootPathSegment::Environment { ordinal } => {
                        output.push(3);
                        output.extend_from_slice(&ordinal.to_le_bytes());
                    }
                    HirStyleRootPathSegment::Clause { ordinal } => {
                        output.push(4);
                        output.extend_from_slice(&ordinal.to_le_bytes());
                    }
                }
            }
        }
        HirDeclarationItemRootRole::TestBody => output.push(10),
        HirDeclarationItemRootRole::BenchBody => output.push(11),
        HirDeclarationItemRootRole::Recovery { owner } => {
            output.push(12);
            match owner {
                HirItemRecoveryRootOwner::Item => output.push(0),
                HirItemRecoveryRootOwner::Attribute {
                    attribute,
                    argument,
                } => {
                    output.push(1);
                    output.extend_from_slice(&attribute.to_le_bytes());
                    output.extend_from_slice(&argument.to_le_bytes());
                }
                HirItemRecoveryRootOwner::DeclarationMember { member } => {
                    output.push(2);
                    output.extend_from_slice(&member.to_le_bytes());
                }
            }
        }
    }
    Ok(())
}

fn write_item_attribute_owner(output: &mut Vec<u8>, owner: HirItemAttributeOwner) {
    match owner {
        HirItemAttributeOwner::Item => output.push(0),
        HirItemAttributeOwner::InlineMember { member } => {
            output.push(1);
            output.extend_from_slice(&member.to_le_bytes());
        }
        HirItemAttributeOwner::CapabilityMember { member } => {
            output.push(2);
            output.extend_from_slice(&member.to_le_bytes());
        }
    }
}

const fn flow_contract_family_tag(family: HirFlowContractRootFamily) -> u8 {
    match family {
        HirFlowContractRootFamily::Requires => 0,
        HirFlowContractRootFamily::Ensures => 1,
        HirFlowContractRootFamily::Invariant => 2,
        HirFlowContractRootFamily::Assume => 3,
        HirFlowContractRootFamily::Effects => 4,
        HirFlowContractRootFamily::NoEffect => 5,
        HirFlowContractRootFamily::Reads => 6,
        HirFlowContractRootFamily::Modifies => 7,
        HirFlowContractRootFamily::Decreases => 8,
    }
}

fn write_expression_owned_role(
    output: &mut Vec<u8>,
    role: &HirExpressionOwnedBodyRole,
) -> Result<(), SemanticCoordinateEncodingError> {
    match role {
        HirExpressionOwnedBodyRole::ClosureParameterPattern { parameter } => {
            output.push(14);
            output.extend_from_slice(&parameter.to_le_bytes());
        }
        HirExpressionOwnedBodyRole::IfLetPattern => output.push(15),
        HirExpressionOwnedBodyRole::AwaitBranchPattern { branch } => {
            output.push(0);
            output.extend_from_slice(&branch.to_le_bytes());
        }
        HirExpressionOwnedBodyRole::AwaitBranchBody { branch } => {
            output.push(1);
            output.extend_from_slice(&branch.to_le_bytes());
        }
        HirExpressionOwnedBodyRole::ChoiceLetStatement { path } => {
            output.push(2);
            write_hir_nested_path(output, path)?;
        }
        HirExpressionOwnedBodyRole::ChoiceForPattern { path } => {
            output.push(3);
            write_hir_nested_path(output, path)?;
        }
        HirExpressionOwnedBodyRole::ChoiceMatchArmPattern { path, arm } => {
            output.push(4);
            write_hir_nested_path(output, path)?;
            output.extend_from_slice(&arm.to_le_bytes());
        }
        HirExpressionOwnedBodyRole::ChoiceOptionForPattern { path } => {
            output.push(5);
            write_hir_nested_path(output, path)?;
        }
        HirExpressionOwnedBodyRole::ChoiceOptionSelectBody { path, field } => {
            output.push(6);
            write_hir_nested_path(output, path)?;
            output.extend_from_slice(&field.to_le_bytes());
        }
        HirExpressionOwnedBodyRole::ChoiceOptionLetStatement { path, field } => {
            output.push(7);
            write_hir_nested_path(output, path)?;
            output.extend_from_slice(&field.to_le_bytes());
        }
        HirExpressionOwnedBodyRole::ChoicePlanTimeoutBody { path } => {
            output.push(8);
            write_hir_nested_path(output, path)?;
        }
        HirExpressionOwnedBodyRole::ChoicePlanCancelTrigger { path } => {
            output.push(15);
            write_hir_nested_path(output, path)?;
        }
        HirExpressionOwnedBodyRole::ChoicePlanCancelBody { path } => {
            output.push(9);
            write_hir_nested_path(output, path)?;
        }
        HirExpressionOwnedBodyRole::ChoicePlanOnSelectPattern { path } => {
            output.push(10);
            write_hir_nested_path(output, path)?;
        }
        HirExpressionOwnedBodyRole::ChoicePlanOnSelectBody { path } => {
            output.push(11);
            write_hir_nested_path(output, path)?;
        }
        HirExpressionOwnedBodyRole::DialogueLinePlanStatement { path, role } => {
            output.push(12);
            write_hir_nested_path(output, path)?;
            write_line_plan_statement_role(output, *role);
        }
        HirExpressionOwnedBodyRole::DialogueLinePlanLet { path } => {
            output.push(13);
            write_hir_nested_path(output, path)?;
        }
    }
    Ok(())
}

fn write_line_plan_statement_role(output: &mut Vec<u8>, role: HirLinePlanStatementRole) {
    match role {
        HirLinePlanStatementRole::Init { statement } => {
            output.push(0);
            output.extend_from_slice(&statement.to_le_bytes());
        }
        HirLinePlanStatementRole::Thread => output.push(1),
        HirLinePlanStatementRole::On => output.push(2),
        HirLinePlanStatementRole::Statement => output.push(3),
        HirLinePlanStatementRole::CancelRule => output.push(4),
        HirLinePlanStatementRole::Error => output.push(5),
    }
}

fn write_hir_nested_path(
    output: &mut Vec<u8>,
    path: &HirNestedExpressionPath,
) -> Result<(), SemanticCoordinateEncodingError> {
    write_len(output, path.segments().len())?;
    for segment in path.segments() {
        match segment {
            HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal } => {
                output.push(0);
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
            HirNestedExpressionPathSegment::ChoiceIfBranch { ordinal } => {
                output.push(1);
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
            HirNestedExpressionPathSegment::ChoiceIfElse => output.push(2),
            HirNestedExpressionPathSegment::ChoiceForBody => output.push(3),
            HirNestedExpressionPathSegment::ChoiceMatchArm { ordinal } => {
                output.push(4);
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
            HirNestedExpressionPathSegment::ChoiceOptionBody => output.push(5),
            HirNestedExpressionPathSegment::ChoiceOptionField { ordinal } => {
                output.push(6);
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
            HirNestedExpressionPathSegment::ChoiceViewEntry { ordinal } => {
                output.push(7);
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
            HirNestedExpressionPathSegment::ChoicePlanItem { ordinal } => {
                output.push(8);
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
            HirNestedExpressionPathSegment::LinePlanItem { ordinal } => {
                output.push(9);
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
            HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal } => {
                output.push(10);
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
            HirNestedExpressionPathSegment::LinePlanTogetherGroupItem { ordinal } => {
                output.push(11);
                output.extend_from_slice(&ordinal.to_le_bytes());
            }
        }
    }
    Ok(())
}

fn body_role_tag(role: HirBodyChildRole) -> u8 {
    match role {
        HirBodyChildRole::Expression => 0,
        HirBodyChildRole::Statement { .. } => 1,
        HirBodyChildRole::Tail => 2,
        HirBodyChildRole::RecoveryExpression => 3,
        HirBodyChildRole::ThreadItem { .. } => 4,
    }
}

fn write_body_role_payload(output: &mut Vec<u8>, role: HirBodyChildRole) {
    match role {
        HirBodyChildRole::Statement { ordinal } | HirBodyChildRole::ThreadItem { ordinal } => {
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirBodyChildRole::Expression
        | HirBodyChildRole::Tail
        | HirBodyChildRole::RecoveryExpression => {}
    }
}

fn statement_role_tag(role: HirStatementChildRole) -> u8 {
    match role {
        HirStatementChildRole::AssertionCondition { .. } => 0,
        HirStatementChildRole::Pattern => 1,
        HirStatementChildRole::Annotation => 2,
        HirStatementChildRole::Initializer => 3,
        HirStatementChildRole::Input => 4,
        HirStatementChildRole::Target => 5,
        HirStatementChildRole::Value => 6,
        HirStatementChildRole::BodyItem { .. } => 7,
        HirStatementChildRole::ElseIf => 8,
        HirStatementChildRole::TriggerExpression => 9,
        HirStatementChildRole::TriggerPattern => 10,
        HirStatementChildRole::TriggerSignalTarget => 11,
        HirStatementChildRole::TriggerSignalValue => 12,
        HirStatementChildRole::UnsafeReason => 13,
        HirStatementChildRole::Condition => 14,
        HirStatementChildRole::Scrutinee => 15,
        HirStatementChildRole::Guard => 16,
        HirStatementChildRole::MatchPattern { .. } => 17,
        HirStatementChildRole::MatchGuard { .. } => 18,
        HirStatementChildRole::MatchValue { .. } => 19,
        HirStatementChildRole::ForSource => 20,
        HirStatementChildRole::ForIterator => 21,
        HirStatementChildRole::ForNextValue => 22,
        HirStatementChildRole::SelectOperand => 23,
        HirStatementChildRole::SelectBinding { .. } => 24,
        HirStatementChildRole::SelectSource { .. } => 25,
        HirStatementChildRole::SelectPattern { .. } => 26,
    }
}

fn write_statement_role_payload(output: &mut Vec<u8>, role: HirStatementChildRole) {
    match role {
        HirStatementChildRole::AssertionCondition { ordinal } => {
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirStatementChildRole::BodyItem { body, ordinal } => {
            output.push(statement_body_tag(body));
            write_statement_body_payload(output, body);
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirStatementChildRole::MatchPattern { arm }
        | HirStatementChildRole::MatchGuard { arm }
        | HirStatementChildRole::MatchValue { arm } => output.extend_from_slice(&arm.to_le_bytes()),
        HirStatementChildRole::SelectBinding { branch }
        | HirStatementChildRole::SelectSource { branch }
        | HirStatementChildRole::SelectPattern { branch } => {
            output.extend_from_slice(&branch.to_le_bytes());
        }
        HirStatementChildRole::Pattern
        | HirStatementChildRole::Annotation
        | HirStatementChildRole::Initializer
        | HirStatementChildRole::Input
        | HirStatementChildRole::Target
        | HirStatementChildRole::Value
        | HirStatementChildRole::ElseIf
        | HirStatementChildRole::TriggerExpression
        | HirStatementChildRole::TriggerPattern
        | HirStatementChildRole::TriggerSignalTarget
        | HirStatementChildRole::TriggerSignalValue
        | HirStatementChildRole::UnsafeReason
        | HirStatementChildRole::Condition
        | HirStatementChildRole::Scrutinee
        | HirStatementChildRole::Guard
        | HirStatementChildRole::ForSource
        | HirStatementChildRole::ForIterator
        | HirStatementChildRole::ForNextValue
        | HirStatementChildRole::SelectOperand => {}
    }
}

fn statement_body_tag(role: HirStatementBodyRole) -> u8 {
    match role {
        HirStatementBodyRole::LetElse => 0,
        HirStatementBodyRole::Defer => 1,
        HirStatementBodyRole::On => 2,
        HirStatementBodyRole::UnsafeLifetime => 3,
        HirStatementBodyRole::Then => 4,
        HirStatementBodyRole::Else => 5,
        HirStatementBodyRole::MatchArm { .. } => 6,
        HirStatementBodyRole::While => 7,
        HirStatementBodyRole::WhileLet => 8,
        HirStatementBodyRole::For => 9,
        HirStatementBodyRole::SelectBranch { .. } => 10,
        HirStatementBodyRole::SourceLocale => 11,
        HirStatementBodyRole::Scope => 12,
    }
}

fn write_statement_body_payload(output: &mut Vec<u8>, role: HirStatementBodyRole) {
    match role {
        HirStatementBodyRole::MatchArm { arm } => output.extend_from_slice(&arm.to_le_bytes()),
        HirStatementBodyRole::SelectBranch { branch } => {
            output.extend_from_slice(&branch.to_le_bytes());
        }
        HirStatementBodyRole::LetElse
        | HirStatementBodyRole::Defer
        | HirStatementBodyRole::On
        | HirStatementBodyRole::UnsafeLifetime
        | HirStatementBodyRole::Then
        | HirStatementBodyRole::Else
        | HirStatementBodyRole::While
        | HirStatementBodyRole::WhileLet
        | HirStatementBodyRole::For
        | HirStatementBodyRole::SourceLocale
        | HirStatementBodyRole::Scope => {}
    }
}

fn pattern_role_tag(role: HirPatternChildRole) -> u8 {
    match role {
        HirPatternChildRole::BindingLocal => 0,
        HirPatternChildRole::MutableBindingLocal => 1,
        HirPatternChildRole::VariantPayload => 2,
        HirPatternChildRole::Element { .. } => 3,
        HirPatternChildRole::RecordField { .. } => 4,
        HirPatternChildRole::RecordShorthandLocal { .. } => 5,
        HirPatternChildRole::RecordRestLocal { .. } => 6,
        HirPatternChildRole::SequenceRestLocal => 7,
        HirPatternChildRole::WholeBindingLocal => 8,
        HirPatternChildRole::NestedPattern => 9,
        HirPatternChildRole::OrAlternative { .. } => 10,
        HirPatternChildRole::TypedBindingLocal => 11,
        HirPatternChildRole::TypedBindingType => 12,
    }
}

fn write_pattern_role_payload(output: &mut Vec<u8>, role: HirPatternChildRole) {
    match role {
        HirPatternChildRole::Element { ordinal }
        | HirPatternChildRole::OrAlternative { ordinal } => {
            output.extend_from_slice(&ordinal.to_le_bytes());
        }
        HirPatternChildRole::RecordField { field }
        | HirPatternChildRole::RecordShorthandLocal { field }
        | HirPatternChildRole::RecordRestLocal { field } => {
            output.extend_from_slice(&field.to_le_bytes());
        }
        HirPatternChildRole::BindingLocal
        | HirPatternChildRole::MutableBindingLocal
        | HirPatternChildRole::VariantPayload
        | HirPatternChildRole::SequenceRestLocal
        | HirPatternChildRole::WholeBindingLocal
        | HirPatternChildRole::NestedPattern
        | HirPatternChildRole::TypedBindingLocal
        | HirPatternChildRole::TypedBindingType => {}
    }
}

fn write_len(output: &mut Vec<u8>, value: usize) -> Result<(), SemanticCoordinateEncodingError> {
    let value =
        u64::try_from(value).map_err(|_| SemanticCoordinateEncodingError::LengthOverflow)?;
    output.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), SemanticCoordinateEncodingError> {
    write_len(output, bytes.len())?;
    output.extend_from_slice(bytes);
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum SemanticCoordinateEncodingError {
    #[error("semantic coordinate length does not fit the canonical u64 grammar")]
    LengthOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_with_the_same_digest_keep_distinct_canonical_tags() {
        let declaration = AcceptedSemanticRoot::Declaration(
            AcceptedDeclarationSemanticId::from_bytes([0xa5; 32]),
        );
        let item = AcceptedSemanticRoot::Item(AcceptedItemSemanticId::from_bytes([0xa5; 32]));
        assert_eq!(declaration.as_bytes(), item.as_bytes());
        assert_eq!(declaration.tag(), 0x00);
        assert_eq!(item.tag(), 0x01);
        let declaration_path = CheckedSemanticPath::new(declaration, []);
        let item_path = CheckedSemanticPath::new(item, []);
        let declaration_bytes = declaration_path.canonical_bytes().unwrap();
        let item_bytes = item_path.canonical_bytes().unwrap();
        assert_ne!(declaration_bytes, item_bytes);
        assert_eq!(declaration_bytes[0], 0x00);
        assert_eq!(item_bytes[0], 0x01);
    }
}
