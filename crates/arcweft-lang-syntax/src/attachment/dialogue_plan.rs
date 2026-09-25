//! Snapshot-bound typed Dialogue line-plan ownership.

use super::expression::AttachedExpressionNode;
use super::family::{FamilyNode, StatementFamily, StatementNode};
use super::node::{
    AstNode, BlockKind, ColonKind, DialogueCancelRuleBodyKind, DialogueCancelRuleStatementKind,
    DialogueLinePlanBodyKind, DialogueLinePlanInitKind, DialogueLinePlanKind, ErrorNodeKind,
    MissingBodyKind,
};
use super::statement::{invalid as statement_invalid, optional_recovery, require_roles};
use super::thread_body::{AttachedRequiredNestedThreadFlowBody, AttachedThreadFlowItem};
use super::trigger::{AttachedTriggerPattern, attach_trigger_pattern};
use super::{AttachedPatternNode, SyntaxAccessError, SyntaxNodeHandle};
use crate::expressions::ExpressionProjection;
use crate::grammar::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::name::{SyntaxName, SyntaxNameIssue};
use crate::patterns::{
    PatternComponentRole, PatternNameSyntax, PatternSyntaxKind, PatternSyntaxState,
    PatternUnqualifiedVariantForm, PatternVariantHead, PatternVariantHeadSyntax,
    PatternVariantPayloadSyntax,
};
use arcweft_source::SourceSpan;

/// Complete typed `cancel on TRIGGER { ... }` Dialogue line-plan relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedDialogueCancelRuleStatement {
    syntax: AstNode<DialogueCancelRuleStatementKind>,
    trigger: AttachedTriggerPattern,
    body: AttachedDialogueCancelRuleBody,
    header_recovery: Option<AstNode<ErrorNodeKind>>,
    trailing_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedDialogueCancelRuleStatement {
    pub const fn syntax(&self) -> &AstNode<DialogueCancelRuleStatementKind> {
        &self.syntax
    }

    pub const fn trigger(&self) -> &AttachedTriggerPattern {
        &self.trigger
    }

    /// Returns the dedicated selector projection used only by a cancellation
    /// `input(.Variant)` trigger. Ordinary `on input(pattern)` retains its
    /// generic Pattern attachment and lowering path.
    pub fn input_action_selector(
        &self,
    ) -> Result<Option<AttachedInputActionSelector>, SyntaxAccessError> {
        let AttachedTriggerPattern::Input(trigger) = &self.trigger else {
            return Ok(None);
        };
        Ok(Some(AttachedInputActionSelector::from_pattern(
            trigger.pattern(),
        )?))
    }

    pub const fn body(&self) -> &AttachedDialogueCancelRuleBody {
        &self.body
    }

    pub const fn header_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.header_recovery.as_ref()
    }

    pub const fn trailing_recovery(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.trailing_recovery.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.trigger.has_recovery()
            || self.body.has_recovery()
            || self.header_recovery.is_some()
            || self.trailing_recovery.is_some()
    }
}

/// Parser-owned typed projection of one cancellation input action selector.
/// Its source is the exact variant-name component, not the whole input
/// pattern, so HIR can publish one precise source-index role without
/// retaining or lowering a generic Pattern node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedInputActionSelector {
    Resolved {
        name: SyntaxName,
        source: SourceSpan,
    },
    Recovered {
        issue: AttachedInputActionSelectorIssue,
        source: Option<SourceSpan>,
    },
}

impl AttachedInputActionSelector {
    fn from_pattern(pattern: &AttachedPatternNode) -> Result<Self, SyntaxAccessError> {
        let root = pattern.root()?;
        let source = root.component(PatternComponentRole::VariantName);
        let recovered = |issue| Self::Recovered {
            issue,
            source: source.clone(),
        };
        if !matches!(root.state(), PatternSyntaxState::Valid) {
            return Ok(recovered(
                AttachedInputActionSelectorIssue::RecoveredPattern,
            ));
        }

        let PatternSyntaxKind::Variant(variant) = root.value().kind() else {
            return Ok(recovered(
                AttachedInputActionSelectorIssue::UnsupportedPattern,
            ));
        };
        let dot_shorthand = matches!(
            variant.head(),
            PatternVariantHeadSyntax::Resolved(PatternVariantHead::Unqualified(
                PatternUnqualifiedVariantForm::DotShorthand
            ))
        );
        if !dot_shorthand || !matches!(variant.payload(), PatternVariantPayloadSyntax::Absent) {
            return Ok(recovered(
                AttachedInputActionSelectorIssue::UnsupportedPattern,
            ));
        }

        match variant.name() {
            PatternNameSyntax::Resolved(name) => {
                let Some(source) = source.clone() else {
                    return Err(SyntaxAccessError::InvalidPatternProjection { id: root.id() });
                };
                Ok(Self::Resolved {
                    name: name.clone(),
                    source,
                })
            }
            PatternNameSyntax::Recovered(SyntaxNameIssue::Missing) | PatternNameSyntax::Absent => {
                Ok(recovered(AttachedInputActionSelectorIssue::MissingName))
            }
            PatternNameSyntax::Recovered(issue) => Ok(recovered(
                AttachedInputActionSelectorIssue::InvalidName(issue.clone()),
            )),
        }
    }

    pub const fn name(&self) -> Option<&SyntaxName> {
        match self {
            Self::Resolved { name, .. } => Some(name),
            Self::Recovered { .. } => None,
        }
    }

    pub const fn issue(&self) -> Option<&AttachedInputActionSelectorIssue> {
        match self {
            Self::Resolved { .. } => None,
            Self::Recovered { issue, .. } => Some(issue),
        }
    }

    pub const fn source_span(&self) -> Option<&SourceSpan> {
        match self {
            Self::Resolved { source, .. } => Some(source),
            Self::Recovered { source, .. } => source.as_ref(),
        }
    }
}

/// Closed syntax-level recovery for an input action selector that cannot be
/// admitted as one unqualified, payload-free variant name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedInputActionSelectorIssue {
    MissingName,
    InvalidName(SyntaxNameIssue),
    UnsupportedPattern,
    RecoveredPattern,
}

/// Braced or indentation-owned cancellation action body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedDialogueCancelRuleBody {
    Nested(AttachedRequiredNestedThreadFlowBody),
    Indented(AttachedDialogueCancelRuleIndentedBody),
}

impl AttachedDialogueCancelRuleBody {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Nested(body) => body.has_recovery(),
            Self::Indented(body) => body.has_recovery(),
        }
    }
}

/// Indentation-owned Thread body for one line-plan cancellation rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedDialogueCancelRuleIndentedBody {
    syntax: AstNode<DialogueCancelRuleBodyKind>,
    colon: AstNode<ColonKind>,
    items: Box<[AttachedThreadFlowItem]>,
}

impl AttachedDialogueCancelRuleIndentedBody {
    pub const fn syntax(&self) -> &AstNode<DialogueCancelRuleBodyKind> {
        &self.syntax
    }

    pub const fn colon(&self) -> &AstNode<ColonKind> {
        &self.colon
    }

    pub fn items(&self) -> &[AttachedThreadFlowItem] {
        &self.items
    }

    pub fn has_recovery(&self) -> bool {
        self.items.iter().any(AttachedThreadFlowItem::has_recovery)
    }
}

impl AstNode<DialogueCancelRuleStatementKind> {
    pub fn semantics(&self) -> Result<AttachedDialogueCancelRuleStatement, SyntaxAccessError> {
        require_roles(
            self,
            &[
                SyntaxRole::Condition,
                SyntaxRole::Body,
                SyntaxRole::Recovery(0),
                SyntaxRole::TrailingRecovery(0),
            ],
        )?;
        let triggers = self.syntax().children_with_role(SyntaxRole::Condition);
        let [trigger] = triggers.as_slice() else {
            return Err(statement_invalid(self));
        };
        let trigger = attach_trigger_pattern(trigger.clone())?;
        let bodies = self.syntax().children_with_role(SyntaxRole::Body);
        let [body] = bodies.as_slice() else {
            return Err(statement_invalid(self));
        };
        let body = body
            .clone()
            .cast::<DialogueCancelRuleBodyKind>()?
            .semantics()?;
        Ok(AttachedDialogueCancelRuleStatement {
            syntax: self.clone(),
            trigger,
            body,
            header_recovery: optional_recovery(self)?,
            trailing_recovery: self
                .syntax()
                .children_with_role(SyntaxRole::TrailingRecovery(0))
                .first()
                .cloned()
                .map(|node| node.cast::<ErrorNodeKind>())
                .transpose()?,
        })
    }
}

impl AstNode<DialogueCancelRuleBodyKind> {
    pub fn semantics(&self) -> Result<AttachedDialogueCancelRuleBody, SyntaxAccessError> {
        if self.syntax().children().iter().any(|child| {
            !matches!(
                child.role(),
                SyntaxRole::Colon
                    | SyntaxRole::Element(0)
                    | SyntaxRole::Recovery(0)
                    | SyntaxRole::ThreadFlowItem(_)
            )
        }) {
            return Err(invalid(&self.syntax()));
        }
        let elements = self.syntax().children_with_role(SyntaxRole::Element(0));
        match elements.as_slice() {
            [body]
                if body.kind() == SyntaxKind::Block
                    && self
                        .syntax()
                        .children_with_role(SyntaxRole::Colon)
                        .is_empty() =>
            {
                let block = body.clone().cast::<BlockKind>()?;
                Ok(AttachedDialogueCancelRuleBody::Nested(
                    AttachedRequiredNestedThreadFlowBody::Present(block.thread_flow_body()?),
                ))
            }
            [] if !self
                .syntax()
                .children_with_role(SyntaxRole::Colon)
                .is_empty() =>
            {
                let colon = self.required_exact_child::<ColonKind>(SyntaxRole::Colon)?;
                let missing =
                    self.optional_exact_child::<MissingBodyKind>(SyntaxRole::Recovery(0))?;
                if let Some(missing) = missing {
                    return Ok(AttachedDialogueCancelRuleBody::Nested(
                        AttachedRequiredNestedThreadFlowBody::Missing(missing),
                    ));
                }
                let items = self
                    .syntax()
                    .ordered_children(SyntaxRoleClass::ThreadFlowItem)?
                    .into_iter()
                    .map(AttachedThreadFlowItem::from_syntax)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice();
                if items.is_empty() {
                    return Err(invalid(&self.syntax()));
                }
                Ok(AttachedDialogueCancelRuleBody::Indented(
                    AttachedDialogueCancelRuleIndentedBody {
                        syntax: self.clone(),
                        colon,
                        items,
                    },
                ))
            }
            [] => {
                let missing =
                    self.required_exact_child::<MissingBodyKind>(SyntaxRole::Recovery(0))?;
                Ok(AttachedDialogueCancelRuleBody::Nested(
                    AttachedRequiredNestedThreadFlowBody::Missing(missing),
                ))
            }
            _ => Err(invalid(&self.syntax())),
        }
    }
}

/// One typed line plan attached directly to its Dialogue application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedDialogueLinePlan {
    syntax: AstNode<DialogueLinePlanKind>,
    body: AttachedDialogueLinePlanBody,
}

impl AttachedDialogueLinePlan {
    pub const fn syntax(&self) -> &AstNode<DialogueLinePlanKind> {
        &self.syntax
    }

    pub const fn body(&self) -> &AttachedDialogueLinePlanBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.body.has_recovery()
    }
}

/// Source-ordered plan body using the existing statement family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedDialogueLinePlanBody {
    syntax: AstNode<DialogueLinePlanBodyKind>,
    items: Box<[AttachedDialogueLinePlanItem]>,
    missing: Option<AstNode<MissingBodyKind>>,
}

/// One direct line-plan item, retaining the typed Init body boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedDialogueLinePlanItem {
    Init(AttachedDialogueLinePlanInit),
    Statement(StatementNode),
}

impl AttachedDialogueLinePlanItem {
    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Init(item) => item.syntax().syntax(),
            Self::Statement(statement) => statement.syntax(),
        }
    }

    pub fn kind(&self) -> SyntaxKind {
        self.syntax().kind()
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Init(item) => item.has_recovery(),
            Self::Statement(statement) => syntax_has_recovery(&statement.syntax()),
        }
    }
}

/// Typed `init { ... }` / `init: ...` source boundary and its ordered statements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedDialogueLinePlanInit {
    syntax: AstNode<DialogueLinePlanInitKind>,
    body: AstNode<BlockKind>,
    statements: Box<[StatementNode]>,
}

impl AttachedDialogueLinePlanInit {
    pub const fn syntax(&self) -> &AstNode<DialogueLinePlanInitKind> {
        &self.syntax
    }

    /// The exact typed block that owns the Init statements and lexical scope.
    pub const fn body(&self) -> &AstNode<BlockKind> {
        &self.body
    }

    pub fn statements(&self) -> &[StatementNode] {
        &self.statements
    }

    pub fn has_recovery(&self) -> bool {
        syntax_has_recovery(&self.syntax.syntax())
    }

    fn from_syntax(syntax: AstNode<DialogueLinePlanInitKind>) -> Result<Self, SyntaxAccessError> {
        if syntax
            .syntax()
            .children()
            .iter()
            .any(|child| !matches!(child.role(), SyntaxRole::Colon | SyntaxRole::Body))
        {
            return Err(invalid(&syntax.syntax()));
        }
        let body = syntax
            .syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .ok_or_else(|| invalid(&syntax.syntax()))?
            .cast::<BlockKind>()?;
        if body.syntax().children().iter().any(|child| {
            !matches!(
                child.role().class(),
                SyntaxRoleClass::Statement
                    | SyntaxRoleClass::OpenDelimiter
                    | SyntaxRoleClass::CloseDelimiter
                    | SyntaxRoleClass::Recovery
            )
        }) {
            return Err(invalid(&body.syntax()));
        }
        let statements = body.statements()?.into_boxed_slice();
        Ok(Self {
            syntax,
            body,
            statements,
        })
    }
}

impl AttachedDialogueLinePlanBody {
    pub const fn syntax(&self) -> &AstNode<DialogueLinePlanBodyKind> {
        &self.syntax
    }

    pub fn items(&self) -> &[AttachedDialogueLinePlanItem] {
        &self.items
    }

    pub const fn missing(&self) -> Option<&AstNode<MissingBodyKind>> {
        self.missing.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.missing.is_some()
            || self
                .items
                .iter()
                .any(AttachedDialogueLinePlanItem::has_recovery)
    }

    fn from_syntax(syntax: AstNode<DialogueLinePlanBodyKind>) -> Result<Self, SyntaxAccessError> {
        if syntax.syntax().children().iter().any(|child| {
            !matches!(
                child.role(),
                SyntaxRole::OpenDelimiter
                    | SyntaxRole::CloseDelimiter
                    | SyntaxRole::Colon
                    | SyntaxRole::DialogueLinePlanItem(_)
                    | SyntaxRole::Recovery(_)
            )
        }) {
            return Err(invalid(&syntax.syntax()));
        }
        let items = syntax
            .syntax()
            .ordered_children(SyntaxRoleClass::DialogueLinePlanItem)?
            .into_iter()
            .map(
                |node| -> Result<AttachedDialogueLinePlanItem, SyntaxAccessError> {
                    match node.kind() {
                        SyntaxKind::DialogueLinePlanInit => Ok(AttachedDialogueLinePlanItem::Init(
                            AttachedDialogueLinePlanInit::from_syntax(node.cast()?)?,
                        )),
                        _ => Ok(AttachedDialogueLinePlanItem::Statement(FamilyNode::<
                            StatementFamily,
                        >::new(
                            node
                        )?)),
                    }
                },
            )
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let missing = syntax
            .syntax()
            .children_with_role(SyntaxRole::Recovery(0))
            .into_iter()
            .next()
            .map(|node| node.cast::<MissingBodyKind>())
            .transpose()?;
        Ok(Self {
            syntax,
            items,
            missing,
        })
    }
}

pub(super) fn attached_dialogue_line_plan(
    application: &AttachedExpressionNode,
) -> Result<Option<AttachedDialogueLinePlan>, SyntaxAccessError> {
    let ExpressionProjection::AttachedContentApplication(projection) = application.projection()
    else {
        return Ok(None);
    };
    let nodes = application
        .syntax()
        .syntax_handle()
        .children_with_role(SyntaxRole::Plan);
    match (projection.has_plan(), nodes.as_slice()) {
        (false, []) => Ok(None),
        (true, [plan]) if plan.kind() == SyntaxKind::DialogueLinePlan => {
            let syntax = plan.clone().cast::<DialogueLinePlanKind>()?;
            let bodies = plan.children_with_role(SyntaxRole::Body);
            let [body] = bodies.as_slice() else {
                return Err(invalid(plan));
            };
            Ok(Some(AttachedDialogueLinePlan {
                syntax,
                body: AttachedDialogueLinePlanBody::from_syntax(
                    body.clone().cast::<DialogueLinePlanBodyKind>()?,
                )?,
            }))
        }
        _ => Err(invalid(application.syntax().syntax_handle())),
    }
}

fn syntax_has_recovery(syntax: &SyntaxNodeHandle) -> bool {
    matches!(
        syntax.kind(),
        SyntaxKind::ErrorStatement
            | SyntaxKind::ErrorExpression
            | SyntaxKind::MissingExpression
            | SyntaxKind::MissingBody
            | SyntaxKind::ErrorNode
    ) || syntax.children().iter().any(syntax_has_recovery)
}

fn invalid(owner: &SyntaxNodeHandle) -> SyntaxAccessError {
    SyntaxAccessError::InvalidExpressionProjection { id: owner.id() }
}

#[cfg(test)]
#[path = "dialogue_plan_tests.rs"]
mod tests;
