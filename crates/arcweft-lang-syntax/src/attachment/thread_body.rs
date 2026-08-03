//! Shared attached ownership for statement-only Flow and Thread bodies.

use super::access::IfStatementHeadNode;
use super::expression::AttachedExpressionNode;
use super::family::{FamilyNode, StatementFamily, StatementNode};
use super::node::{
    AstNode, AwaitWithStatementKind, BlockKind, ChoiceStatementKind, CloseBraceKind,
    DialogueContentApplicationExpressionKind, ErrorStatementKind, FlowBodyKind, ForStatementKind,
    IfStatementKind, IncludeStatementKind, LoopStatementKind, MatchStatementKind, MissingBodyKind,
    OpenBraceKind, ScopeStatementKind, SelectStatementKind, SourceLocaleStatementKind,
    ThreadExpressionKind, WhileLetStatementKind, WhileStatementKind,
};
use super::source_file::AttachedDelimiterState;
use super::{SyntaxAccessError, SyntaxNodeHandle};
use crate::grammar::{SyntaxKind, SyntaxRole, SyntaxRoleClass};

/// Closed family selected for one direct statement-only body child.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AttachedThreadFlowItemFamily {
    Statement,
    DialogueApplication,
    Choice,
    If,
    IfLet,
    Match,
    Loop,
    While,
    WhileLet,
    For,
    Select,
    SourceLocale,
    Scope,
    Include,
    AwaitWith,
    Error,
}

/// One direct source-ordered child of a statement-only Flow or Thread body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedThreadFlowItem {
    Statement(StatementNode),
    DialogueApplication(AstNode<DialogueContentApplicationExpressionKind>),
    Choice(AstNode<ChoiceStatementKind>),
    If(AstNode<IfStatementKind>),
    IfLet(AstNode<IfStatementKind>),
    Match(AstNode<MatchStatementKind>),
    Loop(AstNode<LoopStatementKind>),
    While(AstNode<WhileStatementKind>),
    WhileLet(AstNode<WhileLetStatementKind>),
    For(AstNode<ForStatementKind>),
    Select(AstNode<SelectStatementKind>),
    SourceLocale(AstNode<SourceLocaleStatementKind>),
    Scope(AstNode<ScopeStatementKind>),
    Include(AstNode<IncludeStatementKind>),
    AwaitWith(AstNode<AwaitWithStatementKind>),
    Error(AstNode<ErrorStatementKind>),
}

impl AttachedThreadFlowItem {
    pub(super) fn from_syntax(syntax: SyntaxNodeHandle) -> Result<Self, SyntaxAccessError> {
        let kind = syntax.kind();
        if !kind.is_thread_flow_item() {
            return Err(SyntaxAccessError::InvalidThreadFlowItemShape { id: syntax.id() });
        }
        match kind {
            SyntaxKind::DialogueContentApplicationExpression => {
                Ok(Self::DialogueApplication(syntax.cast()?))
            }
            SyntaxKind::ChoiceStatement => Ok(Self::Choice(syntax.cast()?)),
            SyntaxKind::IfStatement => {
                let conditional = syntax.cast::<IfStatementKind>()?;
                match conditional.head()? {
                    IfStatementHeadNode::Condition(_) => Ok(Self::If(conditional)),
                    IfStatementHeadNode::Let { .. } => Ok(Self::IfLet(conditional)),
                }
            }
            SyntaxKind::MatchStatement => Ok(Self::Match(syntax.cast()?)),
            SyntaxKind::LoopStatement => Ok(Self::Loop(syntax.cast()?)),
            SyntaxKind::WhileStatement => Ok(Self::While(syntax.cast()?)),
            SyntaxKind::WhileLetStatement => Ok(Self::WhileLet(syntax.cast()?)),
            SyntaxKind::ForStatement => Ok(Self::For(syntax.cast()?)),
            SyntaxKind::SelectStatement => Ok(Self::Select(syntax.cast()?)),
            SyntaxKind::SourceLocaleStatement => Ok(Self::SourceLocale(syntax.cast()?)),
            SyntaxKind::ScopeStatement => Ok(Self::Scope(syntax.cast()?)),
            SyntaxKind::IncludeStatement => Ok(Self::Include(syntax.cast()?)),
            SyntaxKind::AwaitWithStatement => Ok(Self::AwaitWith(syntax.cast()?)),
            SyntaxKind::ErrorStatement => Ok(Self::Error(syntax.cast()?)),
            _ if kind.is_statement() => {
                Ok(Self::Statement(FamilyNode::<StatementFamily>::new(syntax)?))
            }
            _ => Err(SyntaxAccessError::InvalidThreadFlowItemShape { id: syntax.id() }),
        }
    }

    /// Exact revision-bound syntax owner.
    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Statement(node) => node.syntax(),
            Self::DialogueApplication(node) => node.syntax(),
            Self::Choice(node) => node.syntax(),
            Self::If(node) | Self::IfLet(node) => node.syntax(),
            Self::Match(node) => node.syntax(),
            Self::Loop(node) => node.syntax(),
            Self::While(node) => node.syntax(),
            Self::WhileLet(node) => node.syntax(),
            Self::For(node) => node.syntax(),
            Self::Select(node) => node.syntax(),
            Self::SourceLocale(node) => node.syntax(),
            Self::Scope(node) => node.syntax(),
            Self::Include(node) => node.syntax(),
            Self::AwaitWith(node) => node.syntax(),
            Self::Error(node) => node.syntax(),
        }
    }

    pub fn kind(&self) -> SyntaxKind {
        self.syntax().kind()
    }

    pub const fn family(&self) -> AttachedThreadFlowItemFamily {
        match self {
            Self::Statement(_) => AttachedThreadFlowItemFamily::Statement,
            Self::DialogueApplication(_) => AttachedThreadFlowItemFamily::DialogueApplication,
            Self::Choice(_) => AttachedThreadFlowItemFamily::Choice,
            Self::If(_) => AttachedThreadFlowItemFamily::If,
            Self::IfLet(_) => AttachedThreadFlowItemFamily::IfLet,
            Self::Match(_) => AttachedThreadFlowItemFamily::Match,
            Self::Loop(_) => AttachedThreadFlowItemFamily::Loop,
            Self::While(_) => AttachedThreadFlowItemFamily::While,
            Self::WhileLet(_) => AttachedThreadFlowItemFamily::WhileLet,
            Self::For(_) => AttachedThreadFlowItemFamily::For,
            Self::Select(_) => AttachedThreadFlowItemFamily::Select,
            Self::SourceLocale(_) => AttachedThreadFlowItemFamily::SourceLocale,
            Self::Scope(_) => AttachedThreadFlowItemFamily::Scope,
            Self::Include(_) => AttachedThreadFlowItemFamily::Include,
            Self::AwaitWith(_) => AttachedThreadFlowItemFamily::AwaitWith,
            Self::Error(_) => AttachedThreadFlowItemFamily::Error,
        }
    }

    pub fn has_recovery(&self) -> bool {
        syntax_has_recovery(&self.syntax())
    }

    /// Returns the one existing statement-family owner for statement-backed
    /// Thread/Flow items. Dialogue application remains expression-owned.
    pub fn statement(&self) -> Option<StatementNode> {
        match self {
            Self::DialogueApplication(_) => None,
            Self::Statement(statement) => Some(statement.clone()),
            _ => Some(
                FamilyNode::<StatementFamily>::new(self.syntax())
                    .expect("checked Thread/Flow statement family remains a statement"),
            ),
        }
    }

    /// Returns the exact attached expression owner for the sole
    /// expression-backed Thread/Flow item family.
    pub fn dialogue_application(&self) -> Option<AttachedExpressionNode> {
        match self {
            Self::DialogueApplication(expression) => Some(
                AttachedExpressionNode::from_syntax(expression.syntax())
                    .expect("checked Dialogue application remains an expression"),
            ),
            _ => None,
        }
    }
}

/// One statement-only ordinary Flow body with no value-tail reader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedFlowStatementBody {
    syntax: AstNode<FlowBodyKind>,
    open: AstNode<OpenBraceKind>,
    items: Box<[AttachedThreadFlowItem]>,
    close: AstNode<CloseBraceKind>,
}

impl AttachedFlowStatementBody {
    pub(super) fn from_block(
        syntax: AstNode<FlowBodyKind>,
        block: AstNode<BlockKind>,
    ) -> Result<Self, SyntaxAccessError> {
        let (open, items, close) = attach_body_children(&block)?;
        Ok(Self {
            syntax,
            open,
            items,
            close,
        })
    }

    pub const fn open(&self) -> &AstNode<OpenBraceKind> {
        &self.open
    }

    /// Exact revision-bound Flow body owner.
    pub const fn syntax(&self) -> &AstNode<FlowBodyKind> {
        &self.syntax
    }

    pub fn items(&self) -> &[AttachedThreadFlowItem] {
        &self.items
    }

    pub const fn close(&self) -> &AstNode<CloseBraceKind> {
        &self.close
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        self.close.delimiter_state()
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.close_state(), AttachedDelimiterState::Missing(_))
            || self.items.iter().any(AttachedThreadFlowItem::has_recovery)
    }

    pub(super) fn range(&self) -> arcweft_source::SourceRange {
        self.syntax.range()
    }
}

/// Present statement-only body or exact missing-body recovery for a Thread expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredThreadExpressionBody {
    Present(AttachedThreadExpressionBody),
    Missing {
        owner: AstNode<ThreadExpressionKind>,
        missing: AstNode<MissingBodyKind>,
    },
}

/// Statement-only body owned directly by one Thread expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedThreadExpressionBody {
    owner: AstNode<ThreadExpressionKind>,
    body: AstNode<BlockKind>,
    open: AstNode<OpenBraceKind>,
    items: Box<[AttachedThreadFlowItem]>,
    close: AstNode<CloseBraceKind>,
}

/// Required statement-only body nested below one Thread/Flow statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedRequiredNestedThreadFlowBody {
    Present(AttachedNestedThreadFlowBody),
    Missing(AstNode<MissingBodyKind>),
}

impl AttachedRequiredNestedThreadFlowBody {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Present(body) => body.has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

/// One nested statement-only body with the same closed item family as Flow
/// declarations and Thread expressions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedNestedThreadFlowBody {
    body: AstNode<BlockKind>,
    open: AstNode<OpenBraceKind>,
    items: Box<[AttachedThreadFlowItem]>,
    close: AstNode<CloseBraceKind>,
}

impl AttachedNestedThreadFlowBody {
    fn from_block(body: AstNode<BlockKind>) -> Result<Self, SyntaxAccessError> {
        let (open, items, close) = attach_body_children(&body)?;
        Ok(Self {
            body,
            open,
            items,
            close,
        })
    }

    pub const fn syntax(&self) -> &AstNode<BlockKind> {
        &self.body
    }

    pub const fn open(&self) -> &AstNode<OpenBraceKind> {
        &self.open
    }

    pub fn items(&self) -> &[AttachedThreadFlowItem] {
        &self.items
    }

    pub const fn close(&self) -> &AstNode<CloseBraceKind> {
        &self.close
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        self.close.delimiter_state()
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.close_state(), AttachedDelimiterState::Missing(_))
            || self.items.iter().any(AttachedThreadFlowItem::has_recovery)
    }
}

impl AstNode<BlockKind> {
    /// Binds this exact revision-backed block as a statement-only Thread/Flow
    /// body. The checked owner rejects ordinary statement/tail roles and
    /// exposes no detached range or source-text fallback.
    pub fn thread_flow_body(&self) -> Result<AttachedNestedThreadFlowBody, SyntaxAccessError> {
        AttachedNestedThreadFlowBody::from_block(self.clone())
    }
}

impl AttachedThreadExpressionBody {
    fn from_block(
        owner: AstNode<ThreadExpressionKind>,
        body: AstNode<BlockKind>,
    ) -> Result<Self, SyntaxAccessError> {
        let (open, items, close) = attach_body_children(&body)?;
        Ok(Self {
            owner,
            body,
            open,
            items,
            close,
        })
    }

    /// Exact revision-bound Thread expression that owns this body.
    pub const fn owner(&self) -> &AstNode<ThreadExpressionKind> {
        &self.owner
    }

    /// Exact revision-bound block syntax for this body.
    pub const fn syntax(&self) -> &AstNode<BlockKind> {
        &self.body
    }

    pub const fn open(&self) -> &AstNode<OpenBraceKind> {
        &self.open
    }

    pub fn items(&self) -> &[AttachedThreadFlowItem] {
        &self.items
    }

    pub const fn close(&self) -> &AstNode<CloseBraceKind> {
        &self.close
    }

    pub fn close_state(&self) -> AttachedDelimiterState {
        self.close.delimiter_state()
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.close_state(), AttachedDelimiterState::Missing(_))
            || self.items.iter().any(AttachedThreadFlowItem::has_recovery)
    }
}

impl AstNode<ThreadExpressionKind> {
    /// Binds the Thread's required statement-only body to this exact snapshot.
    pub fn statement_body(
        &self,
    ) -> Result<AttachedRequiredThreadExpressionBody, SyntaxAccessError> {
        let body = self
            .syntax()
            .optional_unique_child(SyntaxRole::Body)?
            .ok_or(SyntaxAccessError::InvalidThreadFlowBodyShape { id: self.id() })?;
        match body.kind() {
            SyntaxKind::Block => Ok(AttachedRequiredThreadExpressionBody::Present(
                AttachedThreadExpressionBody::from_block(self.clone(), body.cast()?)?,
            )),
            SyntaxKind::MissingBody => {
                let missing = body.cast::<MissingBodyKind>()?;
                if !missing.range().is_empty() {
                    return Err(SyntaxAccessError::InvalidThreadFlowBodyShape { id: self.id() });
                }
                Ok(AttachedRequiredThreadExpressionBody::Missing {
                    owner: self.clone(),
                    missing,
                })
            }
            _ => Err(SyntaxAccessError::InvalidThreadFlowBodyShape { id: self.id() }),
        }
    }
}

pub(super) fn required_nested_thread_flow_body<K: super::node::AstKind>(
    owner: &AstNode<K>,
) -> Result<AttachedRequiredNestedThreadFlowBody, SyntaxAccessError> {
    let body = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or(SyntaxAccessError::InvalidThreadFlowBodyShape { id: owner.id() })?;
    match body.kind() {
        SyntaxKind::Block => Ok(AttachedRequiredNestedThreadFlowBody::Present(
            AttachedNestedThreadFlowBody::from_block(body.cast()?)?,
        )),
        SyntaxKind::MissingBody => {
            let missing = body.cast::<MissingBodyKind>()?;
            if !missing.range().is_empty() {
                return Err(SyntaxAccessError::InvalidThreadFlowBodyShape { id: owner.id() });
            }
            Ok(AttachedRequiredNestedThreadFlowBody::Missing(missing))
        }
        _ => Err(SyntaxAccessError::InvalidThreadFlowBodyShape { id: owner.id() }),
    }
}

fn attach_body_children(
    body: &AstNode<BlockKind>,
) -> Result<
    (
        AstNode<OpenBraceKind>,
        Box<[AttachedThreadFlowItem]>,
        AstNode<CloseBraceKind>,
    ),
    SyntaxAccessError,
> {
    if body.syntax().children().iter().any(|child| {
        matches!(
            child.role().class(),
            SyntaxRoleClass::Statement | SyntaxRoleClass::Tail
        )
    }) {
        return Err(SyntaxAccessError::InvalidThreadFlowBodyShape { id: body.id() });
    }
    let open = body.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?;
    if open.range().is_empty() {
        return Err(SyntaxAccessError::InvalidThreadFlowBodyShape { id: body.id() });
    }
    let items = body
        .syntax()
        .ordered_children(SyntaxRoleClass::ThreadFlowItem)?
        .into_iter()
        .map(AttachedThreadFlowItem::from_syntax)
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let close = body.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?;
    Ok((open, items, close))
}

fn syntax_has_recovery(syntax: &SyntaxNodeHandle) -> bool {
    syntax.kind().is_missing_node()
        || syntax.kind().is_error_node()
        || syntax.children().iter().any(syntax_has_recovery)
}
