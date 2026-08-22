//! Checked nested paths, child roles, and atomic edge facts.

use std::collections::BTreeMap;

use arcweft_core::value::RuntimeRecordFieldId;
use arcweft_lang_hir::{
    expr::{HirNestedExpressionPath, HirNestedExpressionPathSegment},
    identity::ExprId,
};

use super::super::{
    CheckedExpressionResolution, CheckedPatternResolution, CheckedSelectResolution,
    CheckedValueResolution,
};
use crate::callable::{CheckedCallableJoin, CheckedCallableJoinError};

/// A nonempty semantic path through a nested Choice/dialogue/line-plan owner.
///
/// The path intentionally contains only structural ordinals.  HIR IDs,
/// source ranges, and authored names remain lookup evidence and never enter a
/// checked role or its transcript bytes.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedNestedPathV1(Box<[CheckedNestedPathSegmentV1]>);

/// Closed checked role family for one child recorded under a nested path.
///
/// The role itself, rather than its transcript tag, is the membership
/// authority.  Tags are derived only when a consumer explicitly serializes
/// accepted evidence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedNestedEvidenceRole {
    LinePlanOptionValue,
    LinePlanLetValue,
    LinePlanOut,
    LinePlanTimelineAssert,
    LinePlanExpression,
    LinePlanTimedCueAnchor,
    LinePlanTimedCueBody,
    ChoiceIfCondition { branch: u32 },
    ChoiceForSource,
    ChoiceMatchScrutinee,
    ChoiceMatchGuard { arm: u32 },
    ChoiceOptionId,
    ChoiceOptionForSource,
    ChoiceCompactLabel,
    ChoiceCompactCondition,
    ChoiceCompactOut,
    ChoiceOptionLabel { field: u32 },
    ChoiceOptionFieldId { field: u32 },
    ChoiceOptionValue { field: u32 },
    ChoiceOptionVisible { field: u32 },
    ChoiceOptionEnabled { field: u32 },
    ChoiceOptionOrder { field: u32 },
    ChoiceOptionHotkey { field: u32 },
    ChoiceOptionViewKey { field: u32, entry: u32 },
    ChoiceOptionViewValue { field: u32, entry: u32 },
}

/// Checker-owned nested child evidence indexed by accepted structural path.
pub type NestedPathEvidence =
    BTreeMap<CheckedNestedPathV1, Box<[(CheckedNestedEvidenceRole, ExprId)]>>;

impl CheckedNestedPathV1 {
    /// Constructs a checked path.  Empty paths cannot identify a nested owner.
    pub fn try_from_segments(
        segments: Box<[CheckedNestedPathSegmentV1]>,
    ) -> Result<Self, CheckedNestedPathError> {
        (!segments.is_empty())
            .then_some(Self(segments))
            .ok_or(CheckedNestedPathError::Empty)
    }

    /// Returns the accepted structural path segments in source/semantic order.
    pub fn segments(&self) -> &[CheckedNestedPathSegmentV1] {
        &self.0
    }

    pub(super) fn from_hir(path: &HirNestedExpressionPath) -> Result<Self, CheckedChildEdgeError> {
        let segments = path
            .segments()
            .iter()
            .map(CheckedNestedPathSegmentV1::from_hir)
            .collect::<Vec<_>>();
        Self::try_from_segments(segments.into_boxed_slice())
            .map_err(|_| CheckedChildEdgeError::MissingNestedPath)
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
    fn from_hir(segment: &HirNestedExpressionPathSegment) -> Self {
        match segment {
            HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal } => {
                Self::ChoiceBodyItem { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::ChoiceIfBranch { ordinal } => {
                Self::ChoiceIfBranch { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::ChoiceIfElse => Self::ChoiceIfElse,
            HirNestedExpressionPathSegment::ChoiceForBody => Self::ChoiceForBody,
            HirNestedExpressionPathSegment::ChoiceMatchArm { ordinal } => {
                Self::ChoiceMatchArm { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::ChoiceOptionBody => Self::ChoiceOptionBody,
            HirNestedExpressionPathSegment::ChoiceOptionField { ordinal } => {
                Self::ChoiceOptionField { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::ChoiceViewEntry { ordinal } => {
                Self::ChoiceViewEntry { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::ChoicePlanItem { ordinal } => {
                Self::ChoicePlanItem { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::LinePlanItem { ordinal } => {
                Self::LinePlanItem { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal } => {
                Self::LinePlanStartGroupItem { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::LinePlanTogetherGroupItem { ordinal } => {
                Self::LinePlanTogetherGroupItem { ordinal: *ordinal }
            }
        }
    }

    /// Stable path-segment tag used by the version-1 semantic transcript.
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
pub enum CheckedExpressionChildRole {
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
        accepted_field: RuntimeRecordFieldId,
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
    LinePlanTimedCueAnchor {
        path: CheckedNestedPathV1,
    },
    LinePlanTimedCueBody {
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
    /// Stable version-1 semantic role tag.  These tags are intentionally
    /// independent of enum declaration order and HIR/source coordinates.
    pub const fn semantic_tag(&self) -> u16 {
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
            Self::LinePlanTimedCueAnchor { .. } => 0x1022,
            Self::LinePlanTimedCueBody { .. } => 0x1023,
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

    /// Encodes the stable role tag and payload into a caller-owned sink.
    ///
    /// The sink is intentionally a plain byte vector: publication code can
    /// hash it or append child digests without exposing partial output from a
    /// failed checked-edge projection.
    pub fn write_transcript(&self, output: &mut Vec<u8>) {
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
                output.extend_from_slice(&accepted_field.get().get().to_le_bytes());
            }
            Self::Guard { arm } | Self::ArmValue { arm } => {
                output.extend_from_slice(&arm.to_le_bytes());
            }
            Self::ChoiceIfCondition { path, branch } => {
                output.extend_from_slice(&branch.to_le_bytes());
                write_path_transcript(output, path);
            }
            Self::ChoiceMatchGuard { path, arm } => {
                output.extend_from_slice(&arm.to_le_bytes());
                write_path_transcript(output, path);
            }
            Self::LinePlanOptionValue { path }
            | Self::LinePlanLetValue { path }
            | Self::LinePlanOut { path }
            | Self::LinePlanTimelineAssert { path }
            | Self::LinePlanExpression { path }
            | Self::LinePlanTimedCueAnchor { path }
            | Self::LinePlanTimedCueBody { path }
            | Self::ChoiceForSource { path }
            | Self::ChoiceMatchScrutinee { path }
            | Self::ChoiceOptionId { path }
            | Self::ChoiceOptionForSource { path }
            | Self::ChoiceCompactLabel { path }
            | Self::ChoiceCompactCondition { path }
            | Self::ChoiceCompactOut { path } => write_path_transcript(output, path),
            Self::ChoiceOptionLabel { path, field }
            | Self::ChoiceOptionFieldId { path, field }
            | Self::ChoiceOptionValue { path, field }
            | Self::ChoiceOptionVisible { path, field }
            | Self::ChoiceOptionEnabled { path, field }
            | Self::ChoiceOptionOrder { path, field }
            | Self::ChoiceOptionHotkey { path, field } => {
                write_path_transcript(output, path);
                output.extend_from_slice(&field.to_le_bytes());
            }
            Self::ChoiceOptionViewKey { path, field, entry }
            | Self::ChoiceOptionViewValue { path, field, entry } => {
                write_path_transcript(output, path);
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
    }

    /// Returns the role transcript bytes without exposing a mutable partial
    /// sink to callers.
    pub fn transcript_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.write_transcript(&mut bytes);
        bytes
    }
}

impl CheckedNestedEvidenceRole {
    pub(super) fn from_checked_role(role: &CheckedExpressionChildRole) -> Option<Self> {
        Some(match role {
            CheckedExpressionChildRole::LinePlanOptionValue { .. } => Self::LinePlanOptionValue,
            CheckedExpressionChildRole::LinePlanLetValue { .. } => Self::LinePlanLetValue,
            CheckedExpressionChildRole::LinePlanOut { .. } => Self::LinePlanOut,
            CheckedExpressionChildRole::LinePlanTimelineAssert { .. } => {
                Self::LinePlanTimelineAssert
            }
            CheckedExpressionChildRole::LinePlanExpression { .. } => Self::LinePlanExpression,
            CheckedExpressionChildRole::LinePlanTimedCueAnchor { .. } => {
                Self::LinePlanTimedCueAnchor
            }
            CheckedExpressionChildRole::LinePlanTimedCueBody { .. } => Self::LinePlanTimedCueBody,
            CheckedExpressionChildRole::ChoiceIfCondition { branch, .. } => {
                Self::ChoiceIfCondition { branch: *branch }
            }
            CheckedExpressionChildRole::ChoiceForSource { .. } => Self::ChoiceForSource,
            CheckedExpressionChildRole::ChoiceMatchScrutinee { .. } => Self::ChoiceMatchScrutinee,
            CheckedExpressionChildRole::ChoiceMatchGuard { arm, .. } => {
                Self::ChoiceMatchGuard { arm: *arm }
            }
            CheckedExpressionChildRole::ChoiceOptionId { .. } => Self::ChoiceOptionId,
            CheckedExpressionChildRole::ChoiceOptionForSource { .. } => Self::ChoiceOptionForSource,
            CheckedExpressionChildRole::ChoiceCompactLabel { .. } => Self::ChoiceCompactLabel,
            CheckedExpressionChildRole::ChoiceCompactCondition { .. } => {
                Self::ChoiceCompactCondition
            }
            CheckedExpressionChildRole::ChoiceCompactOut { .. } => Self::ChoiceCompactOut,
            CheckedExpressionChildRole::ChoiceOptionLabel { field, .. } => {
                Self::ChoiceOptionLabel { field: *field }
            }
            CheckedExpressionChildRole::ChoiceOptionFieldId { field, .. } => {
                Self::ChoiceOptionFieldId { field: *field }
            }
            CheckedExpressionChildRole::ChoiceOptionValue { field, .. } => {
                Self::ChoiceOptionValue { field: *field }
            }
            CheckedExpressionChildRole::ChoiceOptionVisible { field, .. } => {
                Self::ChoiceOptionVisible { field: *field }
            }
            CheckedExpressionChildRole::ChoiceOptionEnabled { field, .. } => {
                Self::ChoiceOptionEnabled { field: *field }
            }
            CheckedExpressionChildRole::ChoiceOptionOrder { field, .. } => {
                Self::ChoiceOptionOrder { field: *field }
            }
            CheckedExpressionChildRole::ChoiceOptionHotkey { field, .. } => {
                Self::ChoiceOptionHotkey { field: *field }
            }
            CheckedExpressionChildRole::ChoiceOptionViewKey { field, entry, .. } => {
                Self::ChoiceOptionViewKey {
                    field: *field,
                    entry: *entry,
                }
            }
            CheckedExpressionChildRole::ChoiceOptionViewValue { field, entry, .. } => {
                Self::ChoiceOptionViewValue {
                    field: *field,
                    entry: *entry,
                }
            }
            _ => return None,
        })
    }

    /// Stable semantic tag used only for transcript serialization.
    pub const fn semantic_tag(&self) -> u16 {
        match self {
            Self::LinePlanOptionValue => 0x101D,
            Self::LinePlanLetValue => 0x101E,
            Self::LinePlanOut => 0x101F,
            Self::LinePlanTimelineAssert => 0x1020,
            Self::LinePlanExpression => 0x1021,
            Self::LinePlanTimedCueAnchor => 0x1022,
            Self::LinePlanTimedCueBody => 0x1023,
            Self::ChoiceIfCondition { .. } => 0x1027,
            Self::ChoiceForSource => 0x1028,
            Self::ChoiceMatchScrutinee => 0x1029,
            Self::ChoiceMatchGuard { .. } => 0x102A,
            Self::ChoiceOptionId => 0x102B,
            Self::ChoiceOptionForSource => 0x102C,
            Self::ChoiceCompactLabel => 0x102D,
            Self::ChoiceCompactCondition => 0x102E,
            Self::ChoiceCompactOut => 0x102F,
            Self::ChoiceOptionLabel { .. } => 0x1030,
            Self::ChoiceOptionFieldId { .. } => 0x1031,
            Self::ChoiceOptionValue { .. } => 0x1032,
            Self::ChoiceOptionVisible { .. } => 0x1033,
            Self::ChoiceOptionEnabled { .. } => 0x1034,
            Self::ChoiceOptionOrder { .. } => 0x1035,
            Self::ChoiceOptionHotkey { .. } => 0x1036,
            Self::ChoiceOptionViewKey { .. } => 0x1037,
            Self::ChoiceOptionViewValue { .. } => 0x1038,
        }
    }
}

fn write_path_transcript(output: &mut Vec<u8>, path: &CheckedNestedPathV1) {
    let count = u32::try_from(path.segments().len()).unwrap_or(u32::MAX);
    output.extend_from_slice(&count.to_le_bytes());
    for segment in path.segments() {
        segment.write_transcript(output);
    }
}

/// Failure while enriching HIR-only edges with checked evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedChildEdgeError {
    MissingExpression,
    ChildCountMismatch,
    ChildIdentityMismatch,
    MissingCheckedRecordField,
    MissingCallFacts,
    CallSlotMismatch,
    MatchFactMissing,
    MatchScrutineeMismatch,
    MatchGuardMissing,
    MatchGuardArmMismatch,
    MatchGuardChildMismatch,
    MatchGuardTypeMismatch,
    MatchValueArmMismatch,
    MatchValueChildMismatch,
    MissingNestedPath,
    StaleNestedPath,
    WorkLimit,
}

/// One publication-time edge fact for a final-HIR owner.
///
/// The edge vector and the optional callable join are one atomic semantic
/// product.  A caller can never observe accepted edges while the callable
/// evidence for the same owner is missing or invalid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpressionEdgeFact {
    edges: Box<[(ExprId, CheckedExpressionChildRole)]>,
    callable: Option<CheckedCallableJoin>,
}

impl CheckedExpressionEdgeFact {
    pub(super) fn new(
        edges: Box<[(ExprId, CheckedExpressionChildRole)]>,
        callable: Option<CheckedCallableJoin>,
    ) -> Self {
        Self { edges, callable }
    }

    /// Returns the accepted ordered child edges.
    pub fn edges(&self) -> &[(ExprId, CheckedExpressionChildRole)] {
        &self.edges
    }

    /// Returns the accepted callable join when this owner is a Call.
    pub const fn callable(&self) -> Option<&CheckedCallableJoin> {
        self.callable.as_ref()
    }
}

/// First publication error for one owner-level checked edge fact.
///
/// This wrapper intentionally keeps child and callable evidence in one error
/// channel so queries cannot accidentally return a partial sibling product.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExpressionEdgeError {
    Child(CheckedChildEdgeError),
    Callable(CheckedCallableJoinError),
}

impl CheckedExpressionResolution {
    /// Stable semantic constructor tag retained by the Cut 1 transcript.
    pub const fn semantic_tag(&self) -> u16 {
        match self {
            Self::Structural => 0x0200,
            Self::Literal(_) => 0x0201,
            Self::Value(_) => 0x0202,
            Self::Select(_) => 0x0203,
            Self::Nominal(_) => 0x0204,
            Self::Variant(_) => 0x0205,
            Self::StageLook(_) => 0x0206,
            Self::Effect(_) => 0x0207,
            Self::Call => 0x0208,
            Self::Await(_) => 0x0209,
            Self::Choice(_) => 0x020A,
            Self::Try(_) => 0x020B,
            Self::ImplicitCallable(_) => 0x020C,
            Self::ImplicitParameter { .. } => 0x020D,
            Self::Pipe(_) => 0x020E,
            Self::PipeLeft { .. } => 0x020F,
            Self::ViewCall(_) => 0x0210,
            Self::ViewCallee(_) => 0x0211,
            Self::StyleValue(_) => 0x0212,
            Self::StyleCallee(_) => 0x0213,
            Self::DialogueLineReference(_) => 0x0214,
            Self::DialogueLineCoordinate(_) => 0x0215,
            Self::DialogueTextKeyCoordinate(_) => 0x0216,
            Self::CharacterDialogueFactory(_) => 0x0217,
            Self::CharacterDialogueReconfigure(_) => 0x0218,
            Self::DialogueApplication { .. } => 0x0219,
            Self::PostfixBracket(_) => 0x021A,
        }
    }
}

impl CheckedValueResolution {
    /// Stable semantic constructor tag retained by the Cut 1 transcript.
    pub const fn semantic_tag(&self) -> u16 {
        match self {
            Self::Local(_) => 0x0300,
            Self::LineContext => 0x0301,
            Self::CharacterField { .. } => 0x0302,
            Self::ProjectCallable { .. } => 0x0303,
            Self::ProjectItem(_) => 0x0304,
            Self::Entry(_) => 0x0305,
            Self::Registered(_) => 0x0306,
            Self::Constant(_) => 0x0307,
        }
    }
}

impl CheckedSelectResolution {
    /// Stable semantic constructor tag retained by the Cut 1 transcript.
    pub const fn semantic_tag(&self) -> u16 {
        match self {
            Self::Method { .. } => 0x0400,
            Self::DialogueView { .. } => 0x0401,
            Self::AgentField { .. } => 0x0402,
            Self::ProgressField { .. } => 0x0403,
            Self::Field { .. } => 0x0404,
            Self::TupleElement { .. } => 0x0405,
            Self::RecordElement { .. } => 0x0406,
        }
    }
}

impl CheckedPatternResolution {
    /// Stable semantic constructor tag retained by the Cut 1 transcript.
    pub const fn semantic_tag(&self) -> u16 {
        match self {
            Self::Structural => 0x0600,
            Self::Literal(_) => 0x0601,
            Self::Entity(_) => 0x0602,
            Self::Nominal(_) => 0x0603,
            Self::Variant(_) => 0x0604,
        }
    }
}
