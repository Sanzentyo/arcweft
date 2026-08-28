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
const EXPRESSION_RESOLUTION_TAG_BASE: u16 = 0x0200;
const EXPRESSION_RESOLUTION_TAG_END: u16 = 0x021B;
const EXPRESSION_RESOLUTION_TAG_COUNT: u16 = 28;
const VALUE_RESOLUTION_TAG_BASE: u16 = 0x0300;
const VALUE_RESOLUTION_TAG_END: u16 = 0x0307;
const VALUE_RESOLUTION_TAG_COUNT: u16 = 8;
const PATTERN_RESOLUTION_TAG_BASE: u16 = 0x0600;
const PATTERN_RESOLUTION_TAG_END: u16 = 0x0605;
const PATTERN_RESOLUTION_TAG_COUNT: u16 = 6;
const _: () = {
    assert!(
        EXPRESSION_RESOLUTION_TAG_END
            == EXPRESSION_RESOLUTION_TAG_BASE + EXPRESSION_RESOLUTION_TAG_COUNT - 1
    );
    assert!(
        PATTERN_RESOLUTION_TAG_END
            == PATTERN_RESOLUTION_TAG_BASE + PATTERN_RESOLUTION_TAG_COUNT - 1
    );
    assert!(VALUE_RESOLUTION_TAG_END == VALUE_RESOLUTION_TAG_BASE + VALUE_RESOLUTION_TAG_COUNT - 1);
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
            Self::Structural => EXPRESSION_RESOLUTION_TAG_BASE,
            Self::Literal(_) => EXPRESSION_RESOLUTION_TAG_BASE + 1,
            Self::Value(_) => EXPRESSION_RESOLUTION_TAG_BASE + 2,
            Self::Select(_) => EXPRESSION_RESOLUTION_TAG_BASE + 3,
            Self::Nominal(_) => EXPRESSION_RESOLUTION_TAG_BASE + 4,
            Self::Variant(_) => EXPRESSION_RESOLUTION_TAG_BASE + 5,
            Self::StageLook(_) => EXPRESSION_RESOLUTION_TAG_BASE + 6,
            Self::Effect(_) => EXPRESSION_RESOLUTION_TAG_BASE + 7,
            Self::Call => EXPRESSION_RESOLUTION_TAG_BASE + 8,
            Self::Await(_) => EXPRESSION_RESOLUTION_TAG_BASE + 9,
            Self::Choice(_) => EXPRESSION_RESOLUTION_TAG_BASE + 10,
            Self::Try(_) => EXPRESSION_RESOLUTION_TAG_BASE + 11,
            Self::ImplicitCallable(_) => EXPRESSION_RESOLUTION_TAG_BASE + 12,
            Self::ImplicitParameter { .. } => EXPRESSION_RESOLUTION_TAG_BASE + 13,
            Self::Pipe(_) => EXPRESSION_RESOLUTION_TAG_BASE + 14,
            Self::PipeLeft { .. } => EXPRESSION_RESOLUTION_TAG_BASE + 15,
            Self::ViewCall(_) => EXPRESSION_RESOLUTION_TAG_BASE + 16,
            Self::ViewCallee(_) => EXPRESSION_RESOLUTION_TAG_BASE + 17,
            Self::StyleValue(_) => EXPRESSION_RESOLUTION_TAG_BASE + 18,
            Self::StyleCallee(_) => EXPRESSION_RESOLUTION_TAG_BASE + 19,
            Self::DialogueLineReference(_) => EXPRESSION_RESOLUTION_TAG_BASE + 20,
            Self::DialogueLineCoordinate(_) => EXPRESSION_RESOLUTION_TAG_BASE + 21,
            Self::DialogueTextKeyCoordinate(_) => EXPRESSION_RESOLUTION_TAG_BASE + 22,
            Self::CharacterDialogueFactory(_) => EXPRESSION_RESOLUTION_TAG_BASE + 23,
            Self::CharacterDialogueReconfigure(_) => EXPRESSION_RESOLUTION_TAG_BASE + 24,
            Self::DialogueApplication { .. } => EXPRESSION_RESOLUTION_TAG_BASE + 25,
            Self::PostfixBracket(_) => EXPRESSION_RESOLUTION_TAG_BASE + 26,
            Self::Closure(_) => EXPRESSION_RESOLUTION_TAG_END,
        }
    }
}

impl CheckedValueResolution {
    /// Stable semantic constructor tag retained by the Cut 1 transcript.
    pub const fn semantic_tag(&self) -> u16 {
        match self {
            Self::Local(_) => VALUE_RESOLUTION_TAG_BASE,
            Self::LineContext => VALUE_RESOLUTION_TAG_BASE + 1,
            Self::CharacterField { .. } => VALUE_RESOLUTION_TAG_BASE + 2,
            Self::ProjectCallable { .. } => VALUE_RESOLUTION_TAG_BASE + 3,
            Self::ProjectItem(_) => VALUE_RESOLUTION_TAG_BASE + 4,
            Self::Entry(_) => VALUE_RESOLUTION_TAG_BASE + 5,
            Self::Registered(_) => VALUE_RESOLUTION_TAG_BASE + 6,
            Self::Constant(_) => VALUE_RESOLUTION_TAG_END,
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
            Self::Structural => PATTERN_RESOLUTION_TAG_BASE,
            Self::Literal(_) => PATTERN_RESOLUTION_TAG_BASE + 1,
            Self::Entity(_) => PATTERN_RESOLUTION_TAG_BASE + 2,
            Self::Record(_) => PATTERN_RESOLUTION_TAG_BASE + 3,
            Self::Variant(_) => PATTERN_RESOLUTION_TAG_BASE + 4,
            Self::TypedBinding(_) => PATTERN_RESOLUTION_TAG_END,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        EXPRESSION_RESOLUTION_TAG_BASE, EXPRESSION_RESOLUTION_TAG_COUNT,
        EXPRESSION_RESOLUTION_TAG_END, PATTERN_RESOLUTION_TAG_BASE, PATTERN_RESOLUTION_TAG_COUNT,
        PATTERN_RESOLUTION_TAG_END, REMOVED_SELECT_RECORD_ELEMENT_TAG,
        REMOVED_SELECT_TUPLE_ELEMENT_TAG, SELECT_AGENT_FIELD_TAG, SELECT_DIALOGUE_VIEW_TAG,
        SELECT_FIELD_TAG, SELECT_METHOD_TAG, SELECT_PROGRESS_FIELD_TAG, VALUE_RESOLUTION_TAG_BASE,
        VALUE_RESOLUTION_TAG_COUNT, VALUE_RESOLUTION_TAG_END,
    };

    fn assert_unique(tags: &[u16]) {
        assert_eq!(
            tags.iter().collect::<BTreeSet<_>>().len(),
            tags.len(),
            "semantic constructor tags must be unique"
        );
    }

    #[test]
    fn semantic_constructor_tag_layouts_are_exact_and_disjoint() {
        let expression =
            (EXPRESSION_RESOLUTION_TAG_BASE..=EXPRESSION_RESOLUTION_TAG_END).collect::<Vec<_>>();
        assert_eq!(
            expression.len(),
            usize::from(EXPRESSION_RESOLUTION_TAG_COUNT)
        );
        assert_eq!(expression[0], 0x0200);
        assert_eq!(expression[26], 0x021A);
        assert_eq!(expression[27], 0x021B);
        assert_unique(&expression);
        assert_eq!(
            EXPRESSION_RESOLUTION_TAG_END,
            EXPRESSION_RESOLUTION_TAG_BASE + EXPRESSION_RESOLUTION_TAG_COUNT - 1
        );
        assert_eq!(EXPRESSION_RESOLUTION_TAG_END, 0x021B);

        let value = (VALUE_RESOLUTION_TAG_BASE..=VALUE_RESOLUTION_TAG_END).collect::<Vec<_>>();
        assert_eq!(value.len(), usize::from(VALUE_RESOLUTION_TAG_COUNT));
        assert_eq!(value, (0x0300_u16..=0x0307_u16).collect::<Vec<_>>());
        assert_unique(&value);
        assert_eq!(
            VALUE_RESOLUTION_TAG_END,
            VALUE_RESOLUTION_TAG_BASE + VALUE_RESOLUTION_TAG_COUNT - 1
        );

        let select = [
            SELECT_METHOD_TAG,
            SELECT_DIALOGUE_VIEW_TAG,
            SELECT_AGENT_FIELD_TAG,
            SELECT_PROGRESS_FIELD_TAG,
            SELECT_FIELD_TAG,
        ];
        assert_eq!(select, [0x0400, 0x0401, 0x0402, 0x0403, 0x0404]);
        assert_unique(&select);

        let select_with_removed = [
            SELECT_METHOD_TAG,
            SELECT_DIALOGUE_VIEW_TAG,
            SELECT_AGENT_FIELD_TAG,
            SELECT_PROGRESS_FIELD_TAG,
            SELECT_FIELD_TAG,
            REMOVED_SELECT_TUPLE_ELEMENT_TAG,
            REMOVED_SELECT_RECORD_ELEMENT_TAG,
        ];
        assert_unique(&select_with_removed);
        assert_eq!(REMOVED_SELECT_TUPLE_ELEMENT_TAG, 0x0405);
        assert_eq!(REMOVED_SELECT_RECORD_ELEMENT_TAG, 0x0406);

        let pattern =
            (PATTERN_RESOLUTION_TAG_BASE..=PATTERN_RESOLUTION_TAG_END).collect::<Vec<_>>();
        assert_eq!(pattern.len(), usize::from(PATTERN_RESOLUTION_TAG_COUNT));
        assert_eq!(pattern, (0x0600_u16..=0x0605_u16).collect::<Vec<_>>());
        assert_unique(&pattern);
        assert_eq!(
            PATTERN_RESOLUTION_TAG_END,
            PATTERN_RESOLUTION_TAG_BASE + PATTERN_RESOLUTION_TAG_COUNT - 1
        );
        assert_eq!(PATTERN_RESOLUTION_TAG_END, 0x0605);

        let mut all = expression;
        all.extend(value);
        all.extend(select_with_removed);
        all.extend(pattern);
        assert_unique(&all);
    }
}
