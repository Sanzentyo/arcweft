//! Snapshot-bound typed Dialogue line-plan ownership.

use super::expression::AttachedExpressionNode;
use super::family::{FamilyNode, StatementFamily, StatementNode};
use super::node::{AstNode, DialogueLinePlanBodyKind, DialogueLinePlanKind, MissingBodyKind};
use super::{SyntaxAccessError, SyntaxNodeHandle};
use crate::expressions::ExpressionProjection;
use crate::grammar::{SyntaxKind, SyntaxRole, SyntaxRoleClass};

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
    items: Box<[StatementNode]>,
    missing: Option<AstNode<MissingBodyKind>>,
}

impl AttachedDialogueLinePlanBody {
    pub const fn syntax(&self) -> &AstNode<DialogueLinePlanBodyKind> {
        &self.syntax
    }

    pub fn items(&self) -> &[StatementNode] {
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
                .any(|item| syntax_has_recovery(&item.syntax()))
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
            .map(|node| FamilyNode::<StatementFamily>::new(node))
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
    let ExpressionProjection::DialogueContentApplication(projection) = application.projection()
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
