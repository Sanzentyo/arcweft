//! The single HIR-owned expression-child edge inventory.
//!
//! Child order is semantic HIR data, not a source-index reconstruction.  The
//! edge vector is the authority used by direct-child consumers and by the
//! recovery-owner checks; roles add the stable coordinates that later semantic
//! layers use to enrich an edge without changing its child or its order.

use super::{
    HirChoiceCompactAction, HirChoiceItem, HirChoiceOptionField, HirChoicePlanItem, HirExprKind,
    HirRecordField,
};
use crate::dialogue_application::{
    HirDialogueContentApplication, HirDialogueNodeKind, HirLinePlanItem,
    HirPostfixBracketCandidates,
};
use crate::identity::ExprId;
use crate::stmt::HirTriggerPattern;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirExpressionChildEdgeError {
    #[error("an expression child ordinal does not fit u32")]
    OrdinalOverflow,
}

/// One ordered expression-to-expression edge owned by a HIR expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpressionChildEdge {
    child: ExprId,
    role: HirExpressionChildRole,
}

impl HirExpressionChildEdge {
    /// Returns the qualified child expression identity.
    pub const fn child(&self) -> ExprId {
        self.child
    }

    /// Returns the HIR-only role and coordinate of this child edge.
    pub const fn role(&self) -> &HirExpressionChildRole {
        &self.role
    }

    fn new(child: ExprId, role: HirExpressionChildRole) -> Self {
        Self { child, role }
    }
}

/// A nonempty structural path into a nested Choice or line-plan owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirNestedExpressionPath(Box<[HirNestedExpressionPathSegment]>);

impl HirNestedExpressionPath {
    /// Builds a path while rejecting the absence of a structural coordinate.
    pub fn try_from_segments(
        segments: Box<[HirNestedExpressionPathSegment]>,
    ) -> Result<Self, HirNestedExpressionPathError> {
        (!segments.is_empty())
            .then_some(Self(segments))
            .ok_or(HirNestedExpressionPathError::Empty)
    }

    /// Returns the path's typed structural segments.
    pub fn segments(&self) -> &[HirNestedExpressionPathSegment] {
        &self.0
    }

    fn from_segments(segments: Vec<HirNestedExpressionPathSegment>) -> Self {
        debug_assert!(!segments.is_empty());
        Self(segments.into_boxed_slice())
    }
}

/// Construction failure for a nested expression path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirNestedExpressionPathError {
    Empty,
}

/// Typed coordinates for nested Choice and line-plan expression children.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirNestedExpressionPathSegment {
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

/// HIR-only role vocabulary for one expression child edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirExpressionChildRole {
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
        path: HirNestedExpressionPath,
    },
    LinePlanLetValue {
        path: HirNestedExpressionPath,
    },
    LinePlanOut {
        path: HirNestedExpressionPath,
    },
    LinePlanTimelineAssert {
        path: HirNestedExpressionPath,
    },
    LinePlanExpression {
        path: HirNestedExpressionPath,
    },
    LinePlanTimedCueAnchor {
        path: HirNestedExpressionPath,
    },
    LinePlanTimedCueBody {
        path: HirNestedExpressionPath,
    },
    PostfixIndexCandidate,
    PostfixDialogueCandidate,
    ForInput,
    ChoiceIfCondition {
        path: HirNestedExpressionPath,
        branch: u32,
    },
    ChoiceForSource {
        path: HirNestedExpressionPath,
    },
    ChoiceMatchScrutinee {
        path: HirNestedExpressionPath,
    },
    ChoiceMatchGuard {
        path: HirNestedExpressionPath,
        arm: u32,
    },
    ChoiceOptionId {
        path: HirNestedExpressionPath,
    },
    ChoiceOptionForSource {
        path: HirNestedExpressionPath,
    },
    ChoiceCompactLabel {
        path: HirNestedExpressionPath,
    },
    ChoiceCompactCondition {
        path: HirNestedExpressionPath,
    },
    ChoiceCompactOut {
        path: HirNestedExpressionPath,
    },
    ChoiceOptionLabel {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionFieldId {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionValue {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionVisible {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionEnabled {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionOrder {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionHotkey {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionViewKey {
        path: HirNestedExpressionPath,
        field: u32,
        entry: u32,
    },
    ChoiceOptionViewValue {
        path: HirNestedExpressionPath,
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

/// One exact semantic slot which may own a synthetic `RecoveryOperand`.
///
/// `SyntheticOnly` is limited to accepted invalid carriers whose public schema
/// deliberately has no fabricated valid value. The synthetic key still owns
/// the recovery expression at this exact ordinal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirRecoveryOperandSlot {
    Retained(ExprId),
    SyntheticOnly,
}

impl HirExprKind {
    /// Returns every expression edge owned directly by this HIR payload.
    ///
    /// This is the one child-order authority. Statement IDs retained by block,
    /// Choice, or line-plan bodies, plus `FlowItem` owners retained by Thread or
    /// Choice bodies, remain roots in their own typed inventories.
    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive 38-variant projection is the single child-order authority"
    )]
    ///
    /// # Panics
    ///
    /// Panics if an unrejected HIR sequence exceeds the accepted `u32`
    /// ordinal space. Fallible semantic consumers must use
    /// [`Self::try_child_edges`].
    pub fn child_edges(&self) -> Vec<HirExpressionChildEdge> {
        self.try_child_edges()
            .expect("accepted HIR expression children fit checked u32 limits")
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive 38-variant projection is the single child-order authority"
    )]
    pub fn try_child_edges(
        &self,
    ) -> Result<Vec<HirExpressionChildEdge>, HirExpressionChildEdgeError> {
        let mut edges = Vec::new();
        match self {
            Self::Unit
            | Self::Literal(_)
            | Self::EntityReference(_)
            | Self::LifetimePath(_)
            | Self::Path(_)
            | Self::ShortVariant(_)
            | Self::Placeholder(_)
            | Self::NumericBracketSequence(_)
            | Self::Thread(_)
            | Self::Error(_) => {}
            Self::Tuple(expression) => {
                for (ordinal, child) in expression.elements().iter().copied().enumerate() {
                    push_edge(
                        &mut edges,
                        child,
                        HirExpressionChildRole::Element {
                            ordinal: ordinal_u32(ordinal)?,
                        },
                    );
                }
            }
            Self::BracketSequence(expression) => {
                for (ordinal, child) in expression.elements().iter().copied().enumerate() {
                    push_edge(
                        &mut edges,
                        child,
                        HirExpressionChildRole::Element {
                            ordinal: ordinal_u32(ordinal)?,
                        },
                    );
                }
            }
            Self::ArrayRepeat(expression) => {
                push_edge(
                    &mut edges,
                    expression.value(),
                    HirExpressionChildRole::RepeatedValue,
                );
                push_edge(
                    &mut edges,
                    expression.length(),
                    HirExpressionChildRole::RepeatLength,
                );
            }
            Self::Call(expression) => {
                if let Some(callee) = expression.callee().value_expression() {
                    push_edge(&mut edges, callee, HirExpressionChildRole::Callee);
                }
                for (ordinal, argument) in expression.arguments().iter().enumerate() {
                    push_edge(
                        &mut edges,
                        argument.value(),
                        HirExpressionChildRole::Argument {
                            ordinal: ordinal_u32(ordinal)?,
                        },
                    );
                }
            }
            Self::Select(expression) => {
                push_edge(
                    &mut edges,
                    expression.target(),
                    HirExpressionChildRole::Target,
                );
            }
            Self::Index(expression) => {
                push_edge(
                    &mut edges,
                    expression.target(),
                    HirExpressionChildRole::Target,
                );
                push_edge(
                    &mut edges,
                    expression.index(),
                    HirExpressionChildRole::Index,
                );
            }
            Self::Pipe(expression) => {
                push_edge(
                    &mut edges,
                    expression.left(),
                    HirExpressionChildRole::PipeLeft,
                );
                push_edge(
                    &mut edges,
                    expression.right(),
                    HirExpressionChildRole::PipeRight,
                );
            }
            Self::Try(expression) => {
                push_edge(
                    &mut edges,
                    expression.operand(),
                    HirExpressionChildRole::Operand,
                );
            }
            Self::Await(expression) => {
                push_edge(
                    &mut edges,
                    expression.operand(),
                    HirExpressionChildRole::Operand,
                );
            }
            Self::Choice(expression) => append_choice_expression_edges(expression, &mut edges)?,
            Self::Range(expression) => {
                if let Some(start) = expression.start() {
                    push_edge(&mut edges, start, HirExpressionChildRole::RangeStart);
                }
                if let Some(end) = expression.end() {
                    push_edge(&mut edges, end, HirExpressionChildRole::RangeEnd);
                }
            }
            Self::Record(expression) => append_record_edges(expression.fields(), &mut edges)?,
            Self::RecordLiteral(expression) => {
                append_record_edges(expression.fields(), &mut edges)?;
            }
            Self::Binary(expression) => {
                push_edge(
                    &mut edges,
                    expression.left(),
                    HirExpressionChildRole::BinaryLeft,
                );
                push_edge(
                    &mut edges,
                    expression.right(),
                    HirExpressionChildRole::BinaryRight,
                );
            }
            Self::Borrow(expression) => {
                push_edge(
                    &mut edges,
                    expression.operand(),
                    HirExpressionChildRole::Operand,
                );
            }
            Self::Dereference(expression) => {
                push_edge(
                    &mut edges,
                    expression.operand(),
                    HirExpressionChildRole::Operand,
                );
            }
            Self::Closure(expression) => {
                push_edge(
                    &mut edges,
                    expression.body(),
                    HirExpressionChildRole::ClosureBody,
                );
            }
            Self::Unary(expression) => {
                push_edge(
                    &mut edges,
                    expression.operand(),
                    HirExpressionChildRole::Operand,
                );
            }
            Self::Block(expression) => {
                push_edge(
                    &mut edges,
                    expression.tail(),
                    HirExpressionChildRole::BlockTail,
                );
            }
            Self::ComputationBlock(expression) => {
                push_edge(
                    &mut edges,
                    expression.tail(),
                    HirExpressionChildRole::BlockTail,
                );
            }
            Self::NamedBlock(expression) => {
                push_edge(
                    &mut edges,
                    expression.tail(),
                    HirExpressionChildRole::BlockTail,
                );
            }
            Self::Loop(expression) => {
                push_edge(
                    &mut edges,
                    expression.tail(),
                    HirExpressionChildRole::LoopTail,
                );
            }
            Self::If(expression) => {
                push_edge(
                    &mut edges,
                    expression.condition(),
                    HirExpressionChildRole::Condition,
                );
                push_edge(
                    &mut edges,
                    expression.then_branch(),
                    HirExpressionChildRole::ThenBranch,
                );
                push_edge(
                    &mut edges,
                    expression.else_branch(),
                    HirExpressionChildRole::ElseBranch,
                );
            }
            Self::IfLet(expression) => {
                push_edge(
                    &mut edges,
                    expression.scrutinee(),
                    HirExpressionChildRole::Scrutinee,
                );
                if let Some(guard) = expression.guard() {
                    push_edge(&mut edges, guard, HirExpressionChildRole::IfLetGuard);
                }
                push_edge(
                    &mut edges,
                    expression.then_branch(),
                    HirExpressionChildRole::ThenBranch,
                );
                push_edge(
                    &mut edges,
                    expression.else_branch(),
                    HirExpressionChildRole::ElseBranch,
                );
            }
            Self::Match(expression) => {
                push_edge(
                    &mut edges,
                    expression.scrutinee(),
                    HirExpressionChildRole::Scrutinee,
                );
                for (arm, value) in expression.arms().iter().enumerate() {
                    let arm = ordinal_u32(arm)?;
                    if let Some(guard) = value.guard() {
                        push_edge(&mut edges, guard, HirExpressionChildRole::Guard { arm });
                    }
                    push_edge(
                        &mut edges,
                        value.value(),
                        HirExpressionChildRole::ArmValue { arm },
                    );
                }
            }
            Self::DialogueContentApplication(expression) => {
                append_dialogue_application_edges(expression, &mut edges)?;
            }
            Self::PostfixBracket(expression) => {
                push_edge(
                    &mut edges,
                    expression.target(),
                    HirExpressionChildRole::Target,
                );
                if let HirPostfixBracketCandidates::Ambiguous { index, dialogue } =
                    expression.candidates()
                {
                    push_edge(
                        &mut edges,
                        *index,
                        HirExpressionChildRole::PostfixIndexCandidate,
                    );
                    push_edge(
                        &mut edges,
                        *dialogue,
                        HirExpressionChildRole::PostfixDialogueCandidate,
                    );
                }
            }
            Self::ForSynthetic(expression) => {
                push_edge(
                    &mut edges,
                    expression.input(),
                    HirExpressionChildRole::ForInput,
                );
            }
        }
        Ok(edges)
    }

    /// Projects the one edge authority to the legacy child-ID view.
    pub fn direct_expression_children(&self) -> Vec<ExprId> {
        self.child_edges()
            .into_iter()
            .map(|edge| edge.child())
            .collect()
    }

    /// Resolves a recovery operand through the edge authority while preserving
    /// source semantic ordinals for optional and recovered slots.
    pub(crate) fn recovery_operand_slot(&self, ordinal: u32) -> Option<HirRecoveryOperandSlot> {
        if let Self::Choice(expression) = self {
            let slot = expression
                .required_expression_slots()
                .get(usize::try_from(ordinal).ok()?)
                .copied()?;
            return match slot {
                super::HirChoiceRequiredExpressionSlot::Retained(expected) => self
                    .child_edges()
                    .into_iter()
                    .find(|edge| edge.child() == expected)
                    .map(|edge| HirRecoveryOperandSlot::Retained(edge.child())),
                super::HirChoiceRequiredExpressionSlot::UnretainedInvalidAssignmentValue => None,
            };
        }

        let edges = self.child_edges();
        if let Some(child) = recovery_edge_child(self, &edges, ordinal) {
            return Some(HirRecoveryOperandSlot::Retained(child));
        }

        let field_ordinal = usize::try_from(ordinal).ok()?;
        match self {
            Self::Record(expression) => {
                recovery_record_field_slot(expression.fields(), field_ordinal)
            }
            Self::RecordLiteral(expression) => {
                recovery_record_field_slot(expression.fields(), field_ordinal)
            }
            _ => None,
        }
    }
}

fn append_record_edges(
    fields: &[HirRecordField],
    edges: &mut Vec<HirExpressionChildEdge>,
) -> Result<(), HirExpressionChildEdgeError> {
    for (source_ordinal, field) in fields.iter().enumerate() {
        if let Some(child) = field.value() {
            push_edge(
                edges,
                child,
                HirExpressionChildRole::RecordField {
                    source_ordinal: ordinal_u32(source_ordinal)?,
                },
            );
        }
    }
    Ok(())
}

fn append_dialogue_application_edges(
    application: &HirDialogueContentApplication,
    edges: &mut Vec<HirExpressionChildEdge>,
) -> Result<(), HirExpressionChildEdgeError> {
    push_edge(
        edges,
        application.target(),
        HirExpressionChildRole::DialogueTarget,
    );
    for coordinate in application.coordinates() {
        push_edge(
            edges,
            coordinate.value(),
            HirExpressionChildRole::DialogueCoordinate {
                ordinal: u32::from(coordinate.argument().get()),
            },
        );
    }
    for node in application.content().nodes() {
        if let HirDialogueNodeKind::Interpolation(expression) = node.kind() {
            push_edge(
                edges,
                *expression,
                HirExpressionChildRole::DialogueInterpolation {
                    ordinal: node.id().ordinal(),
                },
            );
        }
    }
    for tag in application.content().tags() {
        if let Some(expression) = tag.payload().expression() {
            push_edge(
                edges,
                expression,
                HirExpressionChildRole::DialogueTagPayload {
                    ordinal: tag.id().ordinal(),
                },
            );
        }
    }
    if let Some(plan) = application.plan() {
        append_line_plan_edges(plan.items(), edges)?;
    }
    Ok(())
}

fn append_line_plan_edges(
    items: &[HirLinePlanItem],
    edges: &mut Vec<HirExpressionChildEdge>,
) -> Result<(), HirExpressionChildEdgeError> {
    #[derive(Clone, Copy)]
    enum GroupKind {
        Start,
        Together,
    }

    let mut pending = vec![(items, Vec::new(), None)];
    while let Some((items, prefix, group)) = pending.pop() {
        for (ordinal, item) in items.iter().enumerate() {
            let item_path = match group {
                None => extend_path(
                    &prefix,
                    HirNestedExpressionPathSegment::LinePlanItem {
                        ordinal: ordinal_u32(ordinal)?,
                    },
                ),
                Some(GroupKind::Start) => extend_path(
                    &prefix,
                    HirNestedExpressionPathSegment::LinePlanStartGroupItem {
                        ordinal: ordinal_u32(ordinal)?,
                    },
                ),
                Some(GroupKind::Together) => extend_path(
                    &prefix,
                    HirNestedExpressionPathSegment::LinePlanTogetherGroupItem {
                        ordinal: ordinal_u32(ordinal)?,
                    },
                ),
            };
            let path = || HirNestedExpressionPath::from_segments(item_path.clone());
            match item {
                HirLinePlanItem::Option { value, .. } => push_edge(
                    edges,
                    *value,
                    HirExpressionChildRole::LinePlanOptionValue { path: path() },
                ),
                HirLinePlanItem::Let { value, .. } => push_edge(
                    edges,
                    *value,
                    HirExpressionChildRole::LinePlanLetValue { path: path() },
                ),
                HirLinePlanItem::Out(value) => push_edge(
                    edges,
                    *value,
                    HirExpressionChildRole::LinePlanOut { path: path() },
                ),
                HirLinePlanItem::TimelineAssert { condition, .. } => push_edge(
                    edges,
                    *condition,
                    HirExpressionChildRole::LinePlanTimelineAssert { path: path() },
                ),
                HirLinePlanItem::Expression(value) => push_edge(
                    edges,
                    *value,
                    HirExpressionChildRole::LinePlanExpression { path: path() },
                ),
                HirLinePlanItem::TimedCue { anchor, body } => {
                    push_edge(
                        edges,
                        *anchor,
                        HirExpressionChildRole::LinePlanTimedCueAnchor { path: path() },
                    );
                    push_edge(
                        edges,
                        *body,
                        HirExpressionChildRole::LinePlanTimedCueBody { path: path() },
                    );
                }
                HirLinePlanItem::StartGroup(nested) => {
                    pending.push((nested, item_path, Some(GroupKind::Start)));
                }
                HirLinePlanItem::TogetherGroup(nested) => {
                    pending.push((nested, item_path, Some(GroupKind::Together)));
                }
                HirLinePlanItem::Init(_)
                | HirLinePlanItem::Thread(_)
                | HirLinePlanItem::On(_)
                | HirLinePlanItem::Statement(_)
                | HirLinePlanItem::CancelRule(_)
                | HirLinePlanItem::Error(_) => {}
            }
        }
    }
    Ok(())
}

fn append_choice_expression_edges(
    expression: &super::HirChoiceExpr,
    edges: &mut Vec<HirExpressionChildEdge>,
) -> Result<(), HirExpressionChildEdgeError> {
    append_choice_body_edges(expression.body(), edges)?;
    if let Some(plan) = expression.plan() {
        for (item, plan_item) in plan.items().iter().enumerate() {
            let item = ordinal_u32(item)?;
            match plan_item {
                HirChoicePlanItem::Assignment { value, .. } => push_edge(
                    edges,
                    *value,
                    HirExpressionChildRole::ChoicePlanAssignment { item },
                ),
                HirChoicePlanItem::Timeout { duration, .. } => push_edge(
                    edges,
                    *duration,
                    HirExpressionChildRole::ChoicePlanTimeout { item },
                ),
                HirChoicePlanItem::Cancel { trigger, .. } => {
                    append_choice_trigger_edge(trigger, item, edges);
                }
                HirChoicePlanItem::OnSelect { .. } | HirChoicePlanItem::Error(_) => {}
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "the explicit choice-body walk preserves deep LIFO source order and typed paths"
)]
fn append_choice_body_edges(
    body: &super::HirChoiceBody,
    edges: &mut Vec<HirExpressionChildEdge>,
) -> Result<(), HirExpressionChildEdgeError> {
    let mut pending = vec![(body, Vec::new())];
    while let Some((body, prefix)) = pending.pop() {
        for (ordinal, item) in body.items().iter().enumerate() {
            let item_path = extend_path(
                &prefix,
                HirNestedExpressionPathSegment::ChoiceBodyItem {
                    ordinal: ordinal_u32(ordinal)?,
                },
            );
            match item {
                HirChoiceItem::Let(_) | HirChoiceItem::Error => {}
                HirChoiceItem::If(value) => {
                    for (branch, branch_value) in value.branches().iter().enumerate() {
                        let branch = ordinal_u32(branch)?;
                        let branch_path = extend_path(
                            &item_path,
                            HirNestedExpressionPathSegment::ChoiceIfBranch { ordinal: branch },
                        );
                        let path = HirNestedExpressionPath::from_segments(branch_path.clone());
                        push_edge(
                            edges,
                            branch_value.condition(),
                            HirExpressionChildRole::ChoiceIfCondition { path, branch },
                        );
                        pending.push((branch_value.body(), branch_path));
                    }
                    if let Some(else_body) = value.else_body() {
                        let else_path =
                            extend_path(&item_path, HirNestedExpressionPathSegment::ChoiceIfElse);
                        pending.push((else_body, else_path));
                    }
                }
                HirChoiceItem::For(value) => {
                    push_edge(
                        edges,
                        value.source(),
                        HirExpressionChildRole::ChoiceForSource {
                            path: HirNestedExpressionPath::from_segments(item_path.clone()),
                        },
                    );
                    pending.push((
                        value.body(),
                        extend_path(&item_path, HirNestedExpressionPathSegment::ChoiceForBody),
                    ));
                }
                HirChoiceItem::Match(value) => {
                    push_edge(
                        edges,
                        value.scrutinee(),
                        HirExpressionChildRole::ChoiceMatchScrutinee {
                            path: HirNestedExpressionPath::from_segments(item_path.clone()),
                        },
                    );
                    for (arm, arm_value) in value.arms().iter().enumerate() {
                        let arm = ordinal_u32(arm)?;
                        let arm_path = extend_path(
                            &item_path,
                            HirNestedExpressionPathSegment::ChoiceMatchArm { ordinal: arm },
                        );
                        if let Some(guard) = arm_value.guard() {
                            push_edge(
                                edges,
                                guard,
                                HirExpressionChildRole::ChoiceMatchGuard {
                                    path: HirNestedExpressionPath::from_segments(arm_path.clone()),
                                    arm,
                                },
                            );
                        }
                        pending.push((arm_value.body(), arm_path));
                    }
                }
                HirChoiceItem::Option(value) => {
                    push_edge(
                        edges,
                        value.id(),
                        HirExpressionChildRole::ChoiceOptionId {
                            path: HirNestedExpressionPath::from_segments(item_path.clone()),
                        },
                    );
                    append_choice_option_edges(
                        value.body(),
                        &extend_path(&item_path, HirNestedExpressionPathSegment::ChoiceOptionBody),
                        edges,
                    )?;
                }
                HirChoiceItem::OptionFor(value) => {
                    push_edge(
                        edges,
                        value.source(),
                        HirExpressionChildRole::ChoiceOptionForSource {
                            path: HirNestedExpressionPath::from_segments(item_path.clone()),
                        },
                    );
                    append_choice_option_edges(
                        value.body(),
                        &extend_path(&item_path, HirNestedExpressionPathSegment::ChoiceOptionBody),
                        edges,
                    )?;
                }
                HirChoiceItem::CompactArm(value) => {
                    push_edge(
                        edges,
                        value.label(),
                        HirExpressionChildRole::ChoiceCompactLabel {
                            path: HirNestedExpressionPath::from_segments(item_path.clone()),
                        },
                    );
                    if let Some(condition) = value.condition() {
                        push_edge(
                            edges,
                            condition,
                            HirExpressionChildRole::ChoiceCompactCondition {
                                path: HirNestedExpressionPath::from_segments(item_path.clone()),
                            },
                        );
                    }
                    if let HirChoiceCompactAction::Out(value) = value.action() {
                        push_edge(
                            edges,
                            *value,
                            HirExpressionChildRole::ChoiceCompactOut {
                                path: HirNestedExpressionPath::from_segments(item_path),
                            },
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn append_choice_option_edges(
    body: &super::HirChoiceOptionBody,
    prefix: &[HirNestedExpressionPathSegment],
    edges: &mut Vec<HirExpressionChildEdge>,
) -> Result<(), HirExpressionChildEdgeError> {
    for (field, value) in body.fields().iter().enumerate() {
        let field = ordinal_u32(field)?;
        let field_path = extend_path(
            prefix,
            HirNestedExpressionPathSegment::ChoiceOptionField { ordinal: field },
        );
        let path = || HirNestedExpressionPath::from_segments(field_path.clone());
        match value {
            HirChoiceOptionField::Label { value, .. } => push_edge(
                edges,
                *value,
                HirExpressionChildRole::ChoiceOptionLabel {
                    path: path(),
                    field,
                },
            ),
            HirChoiceOptionField::Id(value) => push_edge(
                edges,
                *value,
                HirExpressionChildRole::ChoiceOptionFieldId {
                    path: path(),
                    field,
                },
            ),
            HirChoiceOptionField::Value(value) => push_edge(
                edges,
                *value,
                HirExpressionChildRole::ChoiceOptionValue {
                    path: path(),
                    field,
                },
            ),
            HirChoiceOptionField::Visible(value) => push_edge(
                edges,
                *value,
                HirExpressionChildRole::ChoiceOptionVisible {
                    path: path(),
                    field,
                },
            ),
            HirChoiceOptionField::Enabled(value) => push_edge(
                edges,
                *value,
                HirExpressionChildRole::ChoiceOptionEnabled {
                    path: path(),
                    field,
                },
            ),
            HirChoiceOptionField::Order(value) => push_edge(
                edges,
                *value,
                HirExpressionChildRole::ChoiceOptionOrder {
                    path: path(),
                    field,
                },
            ),
            HirChoiceOptionField::Hotkey(value) => push_edge(
                edges,
                *value,
                HirExpressionChildRole::ChoiceOptionHotkey {
                    path: path(),
                    field,
                },
            ),
            HirChoiceOptionField::View(view) => {
                for (entry, value) in view.entries().iter().enumerate() {
                    let entry = ordinal_u32(entry)?;
                    let entry_path = extend_path(
                        &field_path,
                        HirNestedExpressionPathSegment::ChoiceViewEntry { ordinal: entry },
                    );
                    push_edge(
                        edges,
                        value.key(),
                        HirExpressionChildRole::ChoiceOptionViewKey {
                            path: HirNestedExpressionPath::from_segments(entry_path.clone()),
                            field,
                            entry,
                        },
                    );
                    push_edge(
                        edges,
                        value.value(),
                        HirExpressionChildRole::ChoiceOptionViewValue {
                            path: HirNestedExpressionPath::from_segments(entry_path),
                            field,
                            entry,
                        },
                    );
                }
            }
            HirChoiceOptionField::Select(_)
            | HirChoiceOptionField::Let(_)
            | HirChoiceOptionField::Error => {}
        }
    }
    Ok(())
}

fn append_choice_trigger_edge(
    trigger: &HirTriggerPattern,
    item: u32,
    edges: &mut Vec<HirExpressionChildEdge>,
) {
    match trigger {
        HirTriggerPattern::Signal { target, .. } => push_edge(
            edges,
            *target,
            HirExpressionChildRole::ChoicePlanCancelSignal { item },
        ),
        HirTriggerPattern::Timeout(target) => push_edge(
            edges,
            *target,
            HirExpressionChildRole::ChoicePlanCancelTimeout { item },
        ),
        HirTriggerPattern::Expr(target) => push_edge(
            edges,
            *target,
            HirExpressionChildRole::ChoicePlanCancelExpr { item },
        ),
        HirTriggerPattern::Input(_)
        | HirTriggerPattern::Event(_)
        | HirTriggerPattern::Mark(_)
        | HirTriggerPattern::Select(_)
        | HirTriggerPattern::Task(_)
        | HirTriggerPattern::Scope(_) => {}
    }
}

#[allow(
    clippy::match_same_arms,
    clippy::unnested_or_patterns,
    reason = "recovery projections intentionally group equivalent fixed source-ordinal roles"
)]
fn recovery_edge_child(
    owner: &HirExprKind,
    edges: &[HirExpressionChildEdge],
    ordinal: u32,
) -> Option<ExprId> {
    edges.iter().find_map(|edge| {
        let matches = match (owner, edge.role()) {
            (
                HirExprKind::Tuple(_),
                HirExpressionChildRole::Element {
                    ordinal: edge_ordinal,
                },
            )
            | (
                HirExprKind::BracketSequence(_),
                HirExpressionChildRole::Element {
                    ordinal: edge_ordinal,
                },
            ) => *edge_ordinal == ordinal,
            (HirExprKind::ArrayRepeat(_), HirExpressionChildRole::RepeatedValue) => ordinal == 0,
            (HirExprKind::ArrayRepeat(_), HirExpressionChildRole::RepeatLength) => ordinal == 1,
            (HirExprKind::Call(expression), HirExpressionChildRole::Callee) => {
                ordinal == 0 && expression.callee().value_expression().is_some()
            }
            (HirExprKind::Call(_), HirExpressionChildRole::Argument { ordinal: argument }) => {
                argument
                    .checked_add(1)
                    .is_some_and(|candidate| candidate == ordinal)
            }
            (HirExprKind::Select(_), HirExpressionChildRole::Target)
            | (HirExprKind::Index(_), HirExpressionChildRole::Target)
            | (HirExprKind::Pipe(_), HirExpressionChildRole::PipeLeft)
            | (HirExprKind::Try(_), HirExpressionChildRole::Operand)
            | (HirExprKind::Await(_), HirExpressionChildRole::Operand)
            | (HirExprKind::Borrow(_), HirExpressionChildRole::Operand)
            | (HirExprKind::Dereference(_), HirExpressionChildRole::Operand)
            | (HirExprKind::Unary(_), HirExpressionChildRole::Operand)
            | (HirExprKind::Closure(_), HirExpressionChildRole::ClosureBody)
            | (HirExprKind::Loop(_), HirExpressionChildRole::LoopTail) => ordinal == 0,
            (HirExprKind::Index(_), HirExpressionChildRole::Index)
            | (HirExprKind::Pipe(_), HirExpressionChildRole::PipeRight)
            | (HirExprKind::Binary(_), HirExpressionChildRole::BinaryRight) => ordinal == 1,
            (HirExprKind::Range(_), HirExpressionChildRole::RangeStart)
            | (HirExprKind::Binary(_), HirExpressionChildRole::BinaryLeft) => ordinal == 0,
            (HirExprKind::Range(_), HirExpressionChildRole::RangeEnd) => ordinal == 1,
            (HirExprKind::Record(_), HirExpressionChildRole::RecordField { source_ordinal })
            | (
                HirExprKind::RecordLiteral(_),
                HirExpressionChildRole::RecordField { source_ordinal },
            ) => *source_ordinal == ordinal,
            (HirExprKind::If(_), HirExpressionChildRole::Condition) => ordinal == 0,
            (HirExprKind::If(_), HirExpressionChildRole::ThenBranch) => ordinal == 1,
            (HirExprKind::If(_), HirExpressionChildRole::ElseBranch) => ordinal == 2,
            (HirExprKind::IfLet(_), HirExpressionChildRole::Scrutinee) => ordinal == 0,
            (HirExprKind::IfLet(_), HirExpressionChildRole::IfLetGuard) => ordinal == 1,
            (HirExprKind::IfLet(_), HirExpressionChildRole::ThenBranch) => ordinal == 2,
            (HirExprKind::IfLet(_), HirExpressionChildRole::ElseBranch) => ordinal == 3,
            _ => false,
        };
        matches.then_some(edge.child())
    })
}

fn recovery_record_field_slot(
    fields: &[HirRecordField],
    ordinal: usize,
) -> Option<HirRecoveryOperandSlot> {
    matches!(
        fields.get(ordinal),
        Some(HirRecordField::Invalid {
            issue: super::HirRecordFieldIssue::MissingValue,
        })
    )
    .then_some(HirRecoveryOperandSlot::SyntheticOnly)
}

fn push_edge(edges: &mut Vec<HirExpressionChildEdge>, child: ExprId, role: HirExpressionChildRole) {
    edges.push(HirExpressionChildEdge::new(child, role));
}

pub(super) fn extend_path(
    prefix: &[HirNestedExpressionPathSegment],
    segment: HirNestedExpressionPathSegment,
) -> Vec<HirNestedExpressionPathSegment> {
    let mut path = prefix.to_vec();
    path.push(segment);
    path
}

fn ordinal_u32(ordinal: usize) -> Result<u32, HirExpressionChildEdgeError> {
    u32::try_from(ordinal).map_err(|_| HirExpressionChildEdgeError::OrdinalOverflow)
}

#[cfg(test)]
mod ordinal_tests {
    use super::*;

    #[test]
    fn expression_child_ordinal_is_exact_and_rejects_one_over() {
        let exact = usize::try_from(u32::MAX).unwrap();
        assert_eq!(ordinal_u32(exact), Ok(u32::MAX));
        if let Ok(one_over) = usize::try_from(u64::from(u32::MAX) + 1) {
            assert_eq!(
                ordinal_u32(one_over),
                Err(HirExpressionChildEdgeError::OrdinalOverflow)
            );
        }
    }
}
