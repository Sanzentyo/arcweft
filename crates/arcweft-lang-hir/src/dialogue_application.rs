//! Final dialogue-content and `RichText` records for the shared HIR expression arena.
//!
//! Source sites are owned by the HIR source index. Arena liveness, expression
//! kind checks, lexical-scope admission, and `RichText` limit accounting are
//! reported to the lowering transaction through [`HirDialogueTransactionContext`].

use crate::expr::{HirCallArgument, HirCallArgumentOrdinal, HirCallArgumentOrdinalError};
use crate::identity::{ExprId, HirModuleId, ItemId, PatternId, ScopeId, StmtId, SyntheticRole};
use crate::leaf::HirName;

mod content;
mod rich_text;

pub use self::content::{
    HirDialogueContent, HirDialogueContentError, HirDialogueContentId, HirDialogueIssue,
    HirDialogueNode, HirDialogueNodeId, HirDialogueNodeKind, HirLineBreakKind, HirRuby,
    HirTextFragment,
};
pub use self::rich_text::{
    HirBuiltinRichTextFx, HirBuiltinRichTextTag, HirRichTextArgument, HirRichTextArgumentId,
    HirRichTextArgumentIssue, HirRichTextConditionalTag, HirRichTextDirectStyle, HirRichTextEndTag,
    HirRichTextHostEvent, HirRichTextIssue, HirRichTextLayoutSelector, HirRichTextObjectSelector,
    HirRichTextStyleSelector, HirRichTextTag, HirRichTextTagId, HirRichTextTagIdentity,
    HirRichTextTagPayload, HirRichTextTransformSelector, HirRichTextValue,
    HirUnresolvedRichTextTag,
};

/// A dialogue-content application in the shared expression arena.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueContentApplication {
    target: ExprId,
    content: HirDialogueContent,
    plan: Option<HirLinePlan>,
    coordinates: Box<[HirDialogueCoordinate]>,
}

impl HirDialogueContentApplication {
    pub(crate) fn try_new(
        owner: ExprId,
        target: ExprId,
        content: HirDialogueContent,
        plan: Option<HirLinePlan>,
        coordinates: Box<[HirDialogueCoordinate]>,
    ) -> Result<Self, HirDialogueInvariantError> {
        if content.id().owner() != owner {
            return Err(HirDialogueInvariantError::InvalidContentOwner);
        }
        validate_coordinate_order(&coordinates)?;
        let application = Self {
            target,
            content,
            plan,
            coordinates,
        };
        application
            .validate_module(owner.module())
            .map_err(|actual| HirDialogueInvariantError::ForeignChild {
                expected: owner.module(),
                actual,
            })?;
        Ok(application)
    }

    /// Returns the expression being configured or applied.
    pub const fn target(&self) -> ExprId {
        self.target
    }

    /// Returns the complete ordered dialogue content.
    pub const fn content(&self) -> &HirDialogueContent {
        &self.content
    }

    /// Returns the optional line plan.
    pub const fn plan(&self) -> Option<&HirLinePlan> {
        self.plan.as_ref()
    }

    /// Returns immediate outer-call coordinates in authored argument order.
    pub const fn coordinates(&self) -> &[HirDialogueCoordinate] {
        &self.coordinates
    }

    pub(crate) fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        validate_module(expected, self.target.module())?;
        self.content.validate_module(expected)?;
        if let Some(plan) = &self.plan {
            plan.validate_module(expected)?;
        }
        for coordinate in &self.coordinates {
            validate_module(expected, coordinate.value.module())?;
        }
        Ok(())
    }

    pub(crate) fn has_recovery(&self) -> bool {
        self.content.has_recovery() || self.plan.as_ref().is_some_and(HirLinePlan::has_recovery)
    }

    pub(crate) fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        context
            .require(HirDialogueTransactionRequirement::Expression {
                id: self.target,
                expected: HirDialogueExpressionExpectation::Any,
            })
            .map_err(HirDialogueTransactionError::Context)?;
        for coordinate in &self.coordinates {
            context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: coordinate.value,
                    expected: HirDialogueExpressionExpectation::Any,
                })
                .map_err(HirDialogueTransactionError::Context)?;
        }
        self.content.validate_transaction(context)?;
        if let Some(plan) = &self.plan {
            plan.validate_transaction(context)?;
        }
        Ok(())
    }
}

/// One immediate outer-call configuration coordinate.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueCoordinate {
    kind: HirDialogueCoordinateKind,
    argument: HirCallArgumentOrdinal,
    value: ExprId,
}

impl HirDialogueCoordinate {
    #[cfg(test)]
    pub(crate) const fn new(
        kind: HirDialogueCoordinateKind,
        argument: HirCallArgumentOrdinal,
        value: ExprId,
    ) -> Self {
        Self {
            kind,
            argument,
            value,
        }
    }

    pub(crate) fn from_immediate_arguments(
        arguments: &[HirCallArgument],
    ) -> Result<Box<[Self]>, HirCallArgumentOrdinalError> {
        arguments
            .iter()
            .enumerate()
            .filter_map(|(ordinal, argument)| {
                let kind = match argument.resolved_name().map(HirName::as_str) {
                    Some("id") => HirDialogueCoordinateKind::Id,
                    Some("text_key") => HirDialogueCoordinateKind::TextKey,
                    _ => return None,
                };
                Some(
                    HirCallArgumentOrdinal::try_new(ordinal).map(|argument_ordinal| Self {
                        kind,
                        argument: argument_ordinal,
                        value: argument.value(),
                    }),
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }

    /// Returns the reserved coordinate family.
    pub const fn kind(&self) -> HirDialogueCoordinateKind {
        self.kind
    }

    /// Returns the authored ordinary-call argument position.
    pub const fn argument(&self) -> HirCallArgumentOrdinal {
        self.argument
    }

    /// Returns the unchanged same-arena value expression.
    pub const fn value(&self) -> ExprId {
        self.value
    }
}

/// Reserved dialogue configuration coordinate families.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueCoordinateKind {
    Id,
    TextKey,
}

/// A typed line plan whose children use the module's existing arenas.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirLinePlan {
    root_scope: ScopeId,
    label: Option<HirName>,
    items: Box<[HirLinePlanItem]>,
}

impl HirLinePlan {
    #[cfg(test)]
    pub(crate) fn try_new(
        root_scope: ScopeId,
        label: Option<HirName>,
        items: Box<[HirLinePlanItem]>,
    ) -> Result<Self, HirDialogueInvariantError> {
        validate_line_plan_items(root_scope.module(), &items).map_err(|actual| {
            HirDialogueInvariantError::ForeignChild {
                expected: root_scope.module(),
                actual,
            }
        })?;
        Ok(Self {
            root_scope,
            label,
            items,
        })
    }

    /// Returns the plan's one child block scope.
    pub const fn root_scope(&self) -> ScopeId {
        self.root_scope
    }

    /// Returns the optional semantic label.
    pub const fn label(&self) -> Option<&HirName> {
        self.label.as_ref()
    }

    /// Returns source-ordered plan items.
    pub const fn items(&self) -> &[HirLinePlanItem] {
        &self.items
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        validate_module(expected, self.root_scope.module())?;
        validate_line_plan_items(expected, &self.items)
    }

    fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        context
            .require(HirDialogueTransactionRequirement::Scope(self.root_scope))
            .map_err(HirDialogueTransactionError::Context)?;
        report_line_plan_items(&self.items, context)
    }

    fn has_recovery(&self) -> bool {
        line_plan_items_have_recovery(&self.items)
    }
}

/// Semantic line-plan item projection.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLinePlanItem {
    Init(Box<[StmtId]>),
    Thread(StmtId),
    On(StmtId),
    Option {
        name: HirName,
        value: ExprId,
    },
    Let {
        pattern: PatternId,
        value: ExprId,
    },
    Statement(StmtId),
    Out(ExprId),
    CancelRule(StmtId),
    TimedCue {
        anchor: ExprId,
        body: ExprId,
    },
    StartGroup(Box<[HirLinePlanItem]>),
    TogetherGroup(Box<[HirLinePlanItem]>),
    TimelineAssert {
        policy: TimelineAssertPolicy,
        condition: ExprId,
    },
    Expression(ExprId),
    Error(StmtId),
}

/// Runtime policy retained by a line-timeline assertion.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TimelineAssertPolicy {
    Always,
    DebugOnly,
}

/// One generic postfix bracket with exactly two bounded interpretations.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPostfixBracket {
    target: ExprId,
    candidates: HirPostfixBracketCandidates,
}

impl HirPostfixBracket {
    pub(crate) fn try_new(
        target: ExprId,
        candidates: HirPostfixBracketCandidates,
    ) -> Result<Self, HirDialogueInvariantError> {
        if let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = candidates {
            if index == dialogue || index == target || dialogue == target {
                return Err(HirDialogueInvariantError::InvalidPostfixCandidate);
            }
            validate_module(target.module(), index.module()).map_err(|actual| {
                HirDialogueInvariantError::ForeignChild {
                    expected: target.module(),
                    actual,
                }
            })?;
            validate_module(target.module(), dialogue.module()).map_err(|actual| {
                HirDialogueInvariantError::ForeignChild {
                    expected: target.module(),
                    actual,
                }
            })?;
        }
        Ok(Self { target, candidates })
    }

    /// Returns the shared target excluded from both candidate inventories.
    pub const fn target(&self) -> ExprId {
        self.target
    }

    /// Returns the exact ambiguous or invalid two-result carrier.
    pub const fn candidates(&self) -> &HirPostfixBracketCandidates {
        &self.candidates
    }

    pub(crate) fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        validate_module(expected, self.target.module())?;
        if let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = self.candidates {
            validate_module(expected, index.module())?;
            validate_module(expected, dialogue.module())?;
        }
        Ok(())
    }

    pub(crate) fn validate_transaction<C: HirDialogueTransactionContext>(
        &self,
        owner: ExprId,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        self.validate_transaction_with_roles(
            owner,
            SyntheticRole::PostfixIndexCandidateExpression,
            SyntheticRole::DialogueContentCandidateExpression,
            context,
        )
    }

    pub(crate) fn validate_candidate_transaction<C: HirDialogueTransactionContext>(
        &self,
        owner: ExprId,
        inherited_role: SyntheticRole,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        self.validate_transaction_with_roles(owner, inherited_role, inherited_role, context)
    }

    fn validate_transaction_with_roles<C: HirDialogueTransactionContext>(
        &self,
        owner: ExprId,
        index_role: SyntheticRole,
        dialogue_role: SyntheticRole,
        context: &mut C,
    ) -> Result<(), HirDialogueTransactionError<C::Error>> {
        context
            .require(HirDialogueTransactionRequirement::Expression {
                id: self.target,
                expected: HirDialogueExpressionExpectation::Any,
            })
            .map_err(HirDialogueTransactionError::Context)?;
        if let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = self.candidates {
            context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: index,
                    expected: HirDialogueExpressionExpectation::PostfixIndexCandidate {
                        owner,
                        role: index_role,
                        target: self.target,
                    },
                })
                .map_err(HirDialogueTransactionError::Context)?;
            context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: dialogue,
                    expected: HirDialogueExpressionExpectation::DialogueContentCandidate {
                        owner,
                        role: dialogue_role,
                        target: self.target,
                    },
                })
                .map_err(HirDialogueTransactionError::Context)?;
        }
        Ok(())
    }

    pub(crate) const fn has_recovery(&self) -> bool {
        matches!(self.candidates, HirPostfixBracketCandidates::Invalid { .. })
    }
}

/// The exact two-result postfix carrier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPostfixBracketCandidates {
    Ambiguous {
        index: ExprId,
        dialogue: ExprId,
    },
    Invalid {
        index: HirPostfixCandidateFailure,
        dialogue: HirPostfixCandidateFailure,
    },
}

/// One bounded typed candidate failure.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPostfixCandidateFailure {
    kind: HirPostfixCandidateFailureKind,
}

impl HirPostfixCandidateFailure {
    pub(crate) const fn new(kind: HirPostfixCandidateFailureKind) -> Self {
        Self { kind }
    }

    /// Returns the grammar-owned failure family.
    pub const fn kind(&self) -> HirPostfixCandidateFailureKind {
        self.kind
    }
}

/// Grammar reasons for a failed postfix interpretation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPostfixCandidateFailureKind {
    EmptyPayload,
    UnexpectedToken,
    MissingOperand,
    TrailingToken,
    InvalidDialogueAtom,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueOrdinalError {
    Node { ordinal: usize },
    Tag { ordinal: usize },
    Argument { ordinal: usize },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueExpressionExpectation {
    Any,
    Call,
    PostfixIndexCandidate {
        owner: ExprId,
        role: SyntheticRole,
        target: ExprId,
    },
    DialogueContentCandidate {
        owner: ExprId,
        role: SyntheticRole,
        target: ExprId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirRichTextCharge {
    ContentTags { observed: usize },
    ContentArguments { observed: usize },
    TagArguments { observed: usize },
    ArgumentKeyBytes { observed: usize },
    ArgumentValueDecodedBytes { observed: usize },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueTransactionRequirement {
    Expression {
        id: ExprId,
        expected: HirDialogueExpressionExpectation,
    },
    Statement(StmtId),
    Pattern(PatternId),
    Scope(ScopeId),
    Item(ItemId),
    RichTextCharge(HirRichTextCharge),
}

pub(crate) trait HirDialogueTransactionContext {
    type Error;

    fn require(
        &mut self,
        requirement: HirDialogueTransactionRequirement,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueTransactionError<E> {
    Invariant(HirDialogueInvariantError),
    Context(E),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirDialogueInvariantError {
    ArithmeticOverflow,
    ForeignChild {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    InvalidArgumentReference,
    InvalidContentOwner,
    InvalidEndTagInference,
    InvalidPostfixCandidate,
    InvalidTagReference,
    NonContiguousArgumentOrdinal,
    NonContiguousNodeOrdinal,
    NonContiguousTagOrdinal,
    UnorderedCoordinates,
}

fn validate_coordinate_order(
    coordinates: &[HirDialogueCoordinate],
) -> Result<(), HirDialogueInvariantError> {
    if coordinates
        .windows(2)
        .any(|pair| pair[0].argument >= pair[1].argument)
    {
        Err(HirDialogueInvariantError::UnorderedCoordinates)
    } else {
        Ok(())
    }
}

fn validate_line_plan_items(
    expected: HirModuleId,
    items: &[HirLinePlanItem],
) -> Result<(), HirModuleId> {
    for item in items {
        match item {
            HirLinePlanItem::Init(statements) => {
                for statement in statements {
                    validate_module(expected, statement.module())?;
                }
            }
            HirLinePlanItem::Thread(statement)
            | HirLinePlanItem::On(statement)
            | HirLinePlanItem::Statement(statement)
            | HirLinePlanItem::CancelRule(statement)
            | HirLinePlanItem::Error(statement) => {
                validate_module(expected, statement.module())?;
            }
            HirLinePlanItem::Option { value, .. }
            | HirLinePlanItem::Out(value)
            | HirLinePlanItem::Expression(value)
            | HirLinePlanItem::TimelineAssert {
                condition: value, ..
            } => validate_module(expected, value.module())?,
            HirLinePlanItem::Let { pattern, value } => {
                validate_module(expected, pattern.module())?;
                validate_module(expected, value.module())?;
            }
            HirLinePlanItem::TimedCue { anchor, body } => {
                validate_module(expected, anchor.module())?;
                validate_module(expected, body.module())?;
            }
            HirLinePlanItem::StartGroup(items) | HirLinePlanItem::TogetherGroup(items) => {
                validate_line_plan_items(expected, items)?;
            }
        }
    }
    Ok(())
}

fn report_line_plan_items<C: HirDialogueTransactionContext>(
    items: &[HirLinePlanItem],
    context: &mut C,
) -> Result<(), HirDialogueTransactionError<C::Error>> {
    for item in items {
        match item {
            HirLinePlanItem::Init(statements) => {
                for statement in statements {
                    context
                        .require(HirDialogueTransactionRequirement::Statement(*statement))
                        .map_err(HirDialogueTransactionError::Context)?;
                }
            }
            HirLinePlanItem::Thread(statement)
            | HirLinePlanItem::On(statement)
            | HirLinePlanItem::Statement(statement)
            | HirLinePlanItem::CancelRule(statement)
            | HirLinePlanItem::Error(statement) => context
                .require(HirDialogueTransactionRequirement::Statement(*statement))
                .map_err(HirDialogueTransactionError::Context)?,
            HirLinePlanItem::Option { value, .. }
            | HirLinePlanItem::Out(value)
            | HirLinePlanItem::Expression(value)
            | HirLinePlanItem::TimelineAssert {
                condition: value, ..
            } => context
                .require(HirDialogueTransactionRequirement::Expression {
                    id: *value,
                    expected: HirDialogueExpressionExpectation::Any,
                })
                .map_err(HirDialogueTransactionError::Context)?,
            HirLinePlanItem::Let { pattern, value } => {
                context
                    .require(HirDialogueTransactionRequirement::Pattern(*pattern))
                    .map_err(HirDialogueTransactionError::Context)?;
                context
                    .require(HirDialogueTransactionRequirement::Expression {
                        id: *value,
                        expected: HirDialogueExpressionExpectation::Any,
                    })
                    .map_err(HirDialogueTransactionError::Context)?;
            }
            HirLinePlanItem::TimedCue { anchor, body } => {
                for expression in [*anchor, *body] {
                    context
                        .require(HirDialogueTransactionRequirement::Expression {
                            id: expression,
                            expected: HirDialogueExpressionExpectation::Any,
                        })
                        .map_err(HirDialogueTransactionError::Context)?;
                }
            }
            HirLinePlanItem::StartGroup(items) | HirLinePlanItem::TogetherGroup(items) => {
                report_line_plan_items(items, context)?;
            }
        }
    }
    Ok(())
}

fn line_plan_items_have_recovery(items: &[HirLinePlanItem]) -> bool {
    items.iter().any(|item| match item {
        HirLinePlanItem::Error(_) => true,
        HirLinePlanItem::StartGroup(items) | HirLinePlanItem::TogetherGroup(items) => {
            line_plan_items_have_recovery(items)
        }
        _ => false,
    })
}

fn validate_module(expected: HirModuleId, actual: HirModuleId) -> Result<(), HirModuleId> {
    if expected == actual {
        Ok(())
    } else {
        Err(actual)
    }
}

#[cfg(test)]
#[path = "dialogue_application/tests.rs"]
mod tests;
