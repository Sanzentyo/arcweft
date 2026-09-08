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
pub(crate) const REMOVED_EXPRESSION_RESOLUTION_VIEW_CALLEE_TAG: u16 = 0x0211;
pub(crate) const REMOVED_EXPRESSION_RESOLUTION_STYLE_CALLEE_TAG: u16 = 0x0213;
pub(crate) const REMOVED_LINE_PLAN_TIMED_CUE_ANCHOR_TAG: u16 = 0x1022;
pub(crate) const REMOVED_LINE_PLAN_TIMED_CUE_BODY_TAG: u16 = 0x1023;
const EXPRESSION_RESOLUTION_TAG_BASE: u16 = 0x0200;
const EXPRESSION_RESOLUTION_TAG_CONTENT_APPLICATION: u16 = 0x021C;
const EXPRESSION_RESOLUTION_TAG_COMPILE_TIME_CALLEE: u16 = 0x021D;
const EXPRESSION_RESOLUTION_TAG_TYPE_VALUE: u16 = 0x021E;
const EXPRESSION_RESOLUTION_TAG_TEXT_PROXY_SCALAR: u16 = 0x021F;
const EXPRESSION_RESOLUTION_TAG_COMPILE_TIME_ENUM: u16 = 0x0220;
const EXPRESSION_RESOLUTION_TAG_VIEW_FX_APPLICATION: u16 = 0x0221;
const EXPRESSION_RESOLUTION_TAG_END: u16 = EXPRESSION_RESOLUTION_TAG_VIEW_FX_APPLICATION;
// This count is the number of live constructors, not the width of the
// numeric range.  0x0211 and 0x0213 are retained tombstones for removed
// constructors and must never be reused.
const EXPRESSION_RESOLUTION_TAG_COUNT: u16 = 32;
const EXPRESSION_RESOLUTION_LIVE_TAGS: [u16; 32] = [
    0x0200, 0x0201, 0x0202, 0x0203, 0x0204, 0x0205, 0x0206, 0x0207, 0x0208, 0x0209, 0x020A, 0x020B,
    0x020C, 0x020D, 0x020E, 0x020F, 0x0210, 0x0212, 0x0214, 0x0215, 0x0216, 0x0217, 0x0218, 0x0219,
    0x021A, 0x021B, 0x021C, 0x021D, 0x021E, 0x021F, 0x0220, 0x0221,
];
const VALUE_RESOLUTION_TAG_BASE: u16 = 0x0300;
const VALUE_RESOLUTION_TAG_END: u16 = 0x0307;
const VALUE_RESOLUTION_TAG_COUNT: u16 = 8;
const PATTERN_RESOLUTION_TAG_BASE: u16 = 0x0600;
const PATTERN_RESOLUTION_TAG_END: u16 = 0x0605;
const PATTERN_RESOLUTION_TAG_COUNT: u16 = 6;
const _: () = {
    assert!(EXPRESSION_RESOLUTION_TAG_END == 0x0221);
    assert!(EXPRESSION_RESOLUTION_LIVE_TAGS.len() == EXPRESSION_RESOLUTION_TAG_COUNT as usize);
    assert!(
        PATTERN_RESOLUTION_TAG_END
            == PATTERN_RESOLUTION_TAG_BASE + PATTERN_RESOLUTION_TAG_COUNT - 1
    );
    assert!(VALUE_RESOLUTION_TAG_END == VALUE_RESOLUTION_TAG_BASE + VALUE_RESOLUTION_TAG_COUNT - 1);
    assert!(REMOVED_SELECT_TUPLE_ELEMENT_TAG > SELECT_FIELD_TAG);
    assert!(REMOVED_SELECT_RECORD_ELEMENT_TAG > REMOVED_SELECT_TUPLE_ELEMENT_TAG);
    assert!(REMOVED_EXPRESSION_RESOLUTION_VIEW_CALLEE_TAG == EXPRESSION_RESOLUTION_TAG_BASE + 17);
    assert!(REMOVED_EXPRESSION_RESOLUTION_STYLE_CALLEE_TAG == EXPRESSION_RESOLUTION_TAG_BASE + 19);
    assert!(
        EXPRESSION_RESOLUTION_TAG_COMPILE_TIME_CALLEE
            > EXPRESSION_RESOLUTION_TAG_CONTENT_APPLICATION
    );
    assert!(EXPRESSION_RESOLUTION_TAG_TYPE_VALUE > EXPRESSION_RESOLUTION_TAG_COMPILE_TIME_CALLEE);
    assert!(EXPRESSION_RESOLUTION_TAG_TEXT_PROXY_SCALAR > EXPRESSION_RESOLUTION_TAG_TYPE_VALUE);
    assert!(REMOVED_LINE_PLAN_TIMED_CUE_ANCHOR_TAG < REMOVED_LINE_PLAN_TIMED_CUE_BODY_TAG);
};

/// Closed checked role family for one child recorded under a nested path.
///
/// The role itself, rather than its transcript tag, is the membership
/// authority.  Tags are derived only when a consumer explicitly serializes
/// accepted evidence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedNestedEvidenceRole {
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
    GenericScope(crate::types::GenericScopeError),
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

impl std::error::Error for CheckedChildEdgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::GenericScope(source) => Some(source),
            _ => None,
        }
    }
}

/// One typed final-HIR child edge. The child and role remain available to
/// coordinate and validation authorities. Every edge is traversed exactly once
/// by the semantic transcript in HIR order.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedExpressionChildEdge {
    child: ExprId,
    role: CheckedExpressionChildRole,
}

impl CheckedExpressionChildEdge {
    const fn new(child: ExprId, role: CheckedExpressionChildRole) -> Self {
        Self { child, role }
    }

    pub const fn child(&self) -> ExprId {
        self.child
    }

    pub(crate) const fn role(&self) -> &CheckedExpressionChildRole {
        &self.role
    }
}

/// Final edge fact and typed owner-level callable join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpressionEdgeFact {
    edges: Box<[CheckedExpressionChildEdge]>,
    record_fields: Box<[super::super::CheckedExpressionRecordField]>,
    callable: Option<CheckedCallableJoin>,
}

impl CheckedExpressionEdgeFact {
    pub(super) fn seal(
        edges: Box<[(ExprId, CheckedExpressionChildRole)]>,
        record_fields: Box<[super::super::CheckedExpressionRecordField]>,
        callable: Option<CheckedCallableJoin>,
    ) -> Result<Self, CheckedChildEdgeError> {
        validate_record_field_plan(&edges, &record_fields)?;
        let edges = edges
            .into_iter()
            .map(|(child, role)| CheckedExpressionChildEdge::new(child, role))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self {
            edges,
            record_fields,
            callable,
        })
    }

    /// Returns the accepted ordered child edges.
    pub(crate) fn edges(&self) -> &[CheckedExpressionChildEdge] {
        &self.edges
    }

    /// Returns the complete source-ordered record value plan. Non-record
    /// expressions retain an empty plan.
    pub fn record_fields(&self) -> &[super::super::CheckedExpressionRecordField] {
        &self.record_fields
    }

    /// Iterates the accepted owning expression children in semantic order.
    /// Reference-only HIR relations and unselected candidate edges are absent.
    pub fn child_expressions(&self) -> impl ExactSizeIterator<Item = ExprId> + '_ {
        self.edges.iter().map(CheckedExpressionChildEdge::child)
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
            Self::CompileTimeEnum(_) => EXPRESSION_RESOLUTION_TAG_COMPILE_TIME_ENUM,
            Self::StageLook(_) => EXPRESSION_RESOLUTION_TAG_BASE + 6,
            Self::Effect(_) => EXPRESSION_RESOLUTION_TAG_BASE + 7,
            Self::Call => EXPRESSION_RESOLUTION_TAG_BASE + 8,
            Self::Await(_) => EXPRESSION_RESOLUTION_TAG_BASE + 9,
            Self::Choice(_) => EXPRESSION_RESOLUTION_TAG_BASE + 10,
            Self::Try(_) => EXPRESSION_RESOLUTION_TAG_BASE + 11,
            Self::ImplicitCallable(_) => EXPRESSION_RESOLUTION_TAG_BASE + 12,
            Self::ImplicitParameter(_) => EXPRESSION_RESOLUTION_TAG_BASE + 13,
            Self::Pipe(_) => EXPRESSION_RESOLUTION_TAG_BASE + 14,
            Self::PipeLeft(_) => EXPRESSION_RESOLUTION_TAG_BASE + 15,
            Self::ViewCall(_) => EXPRESSION_RESOLUTION_TAG_BASE + 16,
            Self::CompileTimeCallee(_) => EXPRESSION_RESOLUTION_TAG_COMPILE_TIME_CALLEE,
            Self::StyleValue(_) => EXPRESSION_RESOLUTION_TAG_BASE + 18,
            Self::TypeValue(_) => EXPRESSION_RESOLUTION_TAG_TYPE_VALUE,
            Self::CompileTimeScalar(_) => EXPRESSION_RESOLUTION_TAG_TEXT_PROXY_SCALAR,
            Self::DialogueLineReference(_) => EXPRESSION_RESOLUTION_TAG_BASE + 20,
            Self::DialogueLineCoordinate(_) => EXPRESSION_RESOLUTION_TAG_BASE + 21,
            Self::DialogueTextKeyCoordinate(_) => EXPRESSION_RESOLUTION_TAG_BASE + 22,
            Self::CharacterDialogueFactory(_) => EXPRESSION_RESOLUTION_TAG_BASE + 23,
            Self::CharacterDialogueReconfigure(_) => EXPRESSION_RESOLUTION_TAG_BASE + 24,
            Self::DialogueApplication { .. } => EXPRESSION_RESOLUTION_TAG_BASE + 25,
            Self::PostfixBracket(_) => EXPRESSION_RESOLUTION_TAG_BASE + 26,
            // Preserve the accepted pre-existing Closure tag while appending
            // the attached-content family at the end of the range.
            Self::Closure(_) => EXPRESSION_RESOLUTION_TAG_BASE + 27,
            Self::ContentApplication(_) => EXPRESSION_RESOLUTION_TAG_CONTENT_APPLICATION,
            Self::ViewFxApplication(_) => EXPRESSION_RESOLUTION_TAG_VIEW_FX_APPLICATION,
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
        EXPRESSION_RESOLUTION_LIVE_TAGS, EXPRESSION_RESOLUTION_TAG_COUNT,
        EXPRESSION_RESOLUTION_TAG_END, PATTERN_RESOLUTION_TAG_BASE, PATTERN_RESOLUTION_TAG_COUNT,
        PATTERN_RESOLUTION_TAG_END, REMOVED_EXPRESSION_RESOLUTION_STYLE_CALLEE_TAG,
        REMOVED_EXPRESSION_RESOLUTION_VIEW_CALLEE_TAG, REMOVED_LINE_PLAN_TIMED_CUE_ANCHOR_TAG,
        REMOVED_LINE_PLAN_TIMED_CUE_BODY_TAG, REMOVED_SELECT_RECORD_ELEMENT_TAG,
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
        let expression = EXPRESSION_RESOLUTION_LIVE_TAGS.to_vec();
        assert_eq!(
            expression.len(),
            usize::from(EXPRESSION_RESOLUTION_TAG_COUNT)
        );
        assert_eq!(expression[0], 0x0200);
        assert_eq!(expression[16], 0x0210);
        assert_eq!(expression[17], 0x0212);
        assert_eq!(expression[18], 0x0214);
        assert_eq!(expression[26], 0x021C);
        assert_eq!(expression[27], 0x021D);
        assert_eq!(expression[28], 0x021E);
        assert_eq!(expression[29], 0x021F);
        assert_eq!(expression[30], 0x0220);
        assert_unique(&expression);
        assert!(!expression.contains(&REMOVED_EXPRESSION_RESOLUTION_VIEW_CALLEE_TAG));
        assert!(!expression.contains(&REMOVED_EXPRESSION_RESOLUTION_STYLE_CALLEE_TAG));
        assert_eq!(EXPRESSION_RESOLUTION_TAG_END, 0x0221);

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
        let removed_line_plan = [
            REMOVED_LINE_PLAN_TIMED_CUE_ANCHOR_TAG,
            REMOVED_LINE_PLAN_TIMED_CUE_BODY_TAG,
        ];
        assert_eq!(removed_line_plan, [0x1022, 0x1023]);
        assert_unique(&removed_line_plan);

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
        all.extend(removed_line_plan);
        all.extend(pattern);
        assert_unique(&all);
    }
}
