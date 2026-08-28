//! Checked nested paths, child roles, and atomic edge facts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use arcweft_lang_hir::identity::ExprId;

use super::super::{
    CheckedExpressionResolution, CheckedPatternResolution, CheckedSelectResolution,
    CheckedValueResolution,
};
use crate::callable::{CheckedCallableJoin, CheckedCallableJoinError};
use crate::semantic_coordinate::{CheckedExpressionChildRole, CheckedNestedPathV1};

const SELECT_METHOD_TAG: u16 = 0x0400;
const SELECT_DIALOGUE_VIEW_TAG: u16 = 0x0401;
const SELECT_AGENT_FIELD_TAG: u16 = 0x0402;
const SELECT_PROGRESS_FIELD_TAG: u16 = 0x0403;
const SELECT_FIELD_TAG: u16 = 0x0404;
pub(crate) const REMOVED_SELECT_TUPLE_ELEMENT_TAG: u16 = 0x0405;
pub(crate) const REMOVED_SELECT_RECORD_ELEMENT_TAG: u16 = 0x0406;
const _: () = {
    assert!(REMOVED_SELECT_TUPLE_ELEMENT_TAG > SELECT_FIELD_TAG);
    assert!(REMOVED_SELECT_RECORD_ELEMENT_TAG > REMOVED_SELECT_TUPLE_ELEMENT_TAG);
};

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

/// Failure while enriching HIR-only edges with checked evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedChildEdgeError {
    MissingExpression,
    ChildCountMismatch,
    ChildIdentityMismatch,
    MissingCheckedRecordField,
    UnexpectedCheckedRecordField,
    CheckedRecordFieldOrderMismatch,
    CheckedRecordFieldSourceMismatch,
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

impl fmt::Display for CheckedChildEdgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "checked child-edge invariant {self:?}")
    }
}

impl std::error::Error for CheckedChildEdgeError {}

/// One publication-time edge fact for a final-HIR owner.
///
/// The edge vector and the optional callable join are one atomic semantic
/// product.  A caller can never observe accepted edges while the callable
/// evidence for the same owner is missing or invalid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpressionEdgeFact {
    edges: Box<[(ExprId, CheckedExpressionChildRole)]>,
    record_fields: Box<[super::super::CheckedExpressionRecordField]>,
    callable: Option<CheckedCallableJoin>,
}

impl CheckedExpressionEdgeFact {
    pub(super) fn try_new(
        edges: Box<[(ExprId, CheckedExpressionChildRole)]>,
        record_fields: Box<[super::super::CheckedExpressionRecordField]>,
        callable: Option<CheckedCallableJoin>,
    ) -> Result<Self, CheckedChildEdgeError> {
        validate_record_field_plan(&edges, &record_fields)?;
        Ok(Self {
            edges,
            record_fields,
            callable,
        })
    }

    /// Returns the accepted ordered child edges.
    pub(crate) fn edges(&self) -> &[(ExprId, CheckedExpressionChildRole)] {
        &self.edges
    }

    /// Returns the complete source-ordered record value plan. Non-record
    /// expressions retain an empty plan.
    pub fn record_fields(&self) -> &[super::super::CheckedExpressionRecordField] {
        &self.record_fields
    }

    /// Returns the accepted callable join when this owner is a Call.
    pub const fn callable(&self) -> Option<&CheckedCallableJoin> {
        self.callable.as_ref()
    }
}

fn validate_record_field_plan(
    edges: &[(ExprId, CheckedExpressionChildRole)],
    fields: &[super::super::CheckedExpressionRecordField],
) -> Result<(), CheckedChildEdgeError> {
    let mut expression_rows = BTreeSet::new();
    let mut declaration_ordinals = BTreeSet::new();
    let mut runtime_fields = BTreeSet::new();
    let mut semantic_ids = BTreeSet::new();
    for (expected_source_ordinal, field) in fields.iter().enumerate() {
        let expected_source_ordinal = u32::try_from(expected_source_ordinal)
            .map_err(|_| CheckedChildEdgeError::CheckedRecordFieldOrderMismatch)?;
        if field.source_ordinal() != expected_source_ordinal
            || field.runtime_field().zero_based() != field.declaration_ordinal()
            || !declaration_ordinals.insert(field.declaration_ordinal())
            || !runtime_fields.insert(field.runtime_field())
            || !semantic_ids.insert(field.semantic_id())
        {
            return Err(CheckedChildEdgeError::CheckedRecordFieldOrderMismatch);
        }
        match field.source() {
            super::super::CheckedRecordValueSource::Expression(source) => {
                let mut matching = edges.iter().filter(|(child, role)| {
                    *child == source.raw()
                        && matches!(
                            role,
                            CheckedExpressionChildRole::RecordField {
                                source_ordinal,
                                accepted_field,
                            } if *source_ordinal == field.source_ordinal()
                                && *accepted_field == field.semantic_id()
                        )
                });
                if matching.next().is_none() || matching.next().is_some() {
                    return Err(CheckedChildEdgeError::CheckedRecordFieldSourceMismatch);
                }
                expression_rows.insert(field.source_ordinal());
            }
            super::super::CheckedRecordValueSource::Binding(_) => {
                if edges.iter().any(|(_, role)| {
                    matches!(
                        role,
                        CheckedExpressionChildRole::RecordField { source_ordinal, .. }
                            if *source_ordinal == field.source_ordinal()
                    )
                }) {
                    return Err(CheckedChildEdgeError::CheckedRecordFieldSourceMismatch);
                }
            }
        }
    }
    for (_, role) in edges {
        if let CheckedExpressionChildRole::RecordField { source_ordinal, .. } = role
            && !expression_rows.contains(source_ordinal)
        {
            return Err(CheckedChildEdgeError::MissingCheckedRecordField);
        }
    }
    Ok(())
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

impl fmt::Display for CheckedExpressionEdgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Child(error) => error.fmt(formatter),
            Self::Callable(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CheckedExpressionEdgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Child(error) => error,
            Self::Callable(error) => error,
        })
    }
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
            Self::Closure(_) => 0x021B,
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
            Self::Method(_) => SELECT_METHOD_TAG,
            Self::DialogueView { .. } => SELECT_DIALOGUE_VIEW_TAG,
            Self::AgentField { .. } => SELECT_AGENT_FIELD_TAG,
            Self::ProgressField { .. } => SELECT_PROGRESS_FIELD_TAG,
            Self::Field(_) => SELECT_FIELD_TAG,
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
            Self::Record(_) => 0x0603,
            Self::Variant(_) => 0x0604,
            Self::TypedBinding(_) => 0x0605,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        REMOVED_SELECT_RECORD_ELEMENT_TAG, REMOVED_SELECT_TUPLE_ELEMENT_TAG,
        SELECT_AGENT_FIELD_TAG, SELECT_DIALOGUE_VIEW_TAG, SELECT_FIELD_TAG, SELECT_METHOD_TAG,
        SELECT_PROGRESS_FIELD_TAG,
    };

    #[test]
    fn live_and_removed_select_tags_are_disjoint_and_never_reassigned() {
        let tags = [
            SELECT_METHOD_TAG,
            SELECT_DIALOGUE_VIEW_TAG,
            SELECT_AGENT_FIELD_TAG,
            SELECT_PROGRESS_FIELD_TAG,
            SELECT_FIELD_TAG,
            REMOVED_SELECT_TUPLE_ELEMENT_TAG,
            REMOVED_SELECT_RECORD_ELEMENT_TAG,
        ];
        assert_eq!(tags.into_iter().collect::<BTreeSet<_>>().len(), tags.len());
        assert_eq!(REMOVED_SELECT_TUPLE_ELEMENT_TAG, 0x0405);
        assert_eq!(REMOVED_SELECT_RECORD_ELEMENT_TAG, 0x0406);
    }
}
