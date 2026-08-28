//! Typed declaration and nested-body roots for semantic path construction.

use crate::{
    expr::{
        HirBlockExpr, HirComputationBlockExpr, HirExprKind, HirLoopExpr, HirNamedBlockExpr,
        HirThreadBody, HirThreadExpr, HirThreadFlowItem,
    },
    identity::{ExprId, StmtId},
    item::{HirFunctionBody, HirPredicateBody, HirProofBody},
    stmt::HirContextualStmtBody,
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirBodyChildEdgeError {
    #[error("a HIR body child ordinal does not fit u32")]
    OrdinalOverflow,
}

/// The executable shape of one HIR-owned body container.
///
/// This is the one shared kind authority used by declaration, expression, and
/// statement body projections. `Expression` is a body whose sole child is the
/// expression value; `Ordinary` is a statement body with an optional tail; and
/// `Thread` is the heterogeneous source-ordered Flow/Thread body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirBodyKind {
    Expression,
    Ordinary,
    Thread,
}

/// Checked source-ordered projection of one executable HIR body container.
///
/// The constructor is crate-private so callers cannot pair an arbitrary body
/// kind with an unrelated edge sequence. Empty ordinary and Thread bodies are
/// valid projections and retain their container through the kind alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBodyProjection {
    kind: HirBodyKind,
    children: Box<[HirBodyChildEdge]>,
}

impl HirBodyProjection {
    pub(crate) fn expression(expression: ExprId) -> Self {
        Self {
            kind: HirBodyKind::Expression,
            children: Box::new([HirBodyChildEdge::new(
                HirBodyChild::Expression(expression),
                HirBodyChildRole::Expression,
            )]),
        }
    }

    pub const fn kind(&self) -> HirBodyKind {
        self.kind
    }

    pub const fn children(&self) -> &[HirBodyChildEdge] {
        &self.children
    }

    pub(crate) fn try_new(
        kind: HirBodyKind,
        children: Vec<HirBodyChildEdge>,
    ) -> Result<Self, HirBodyProjectionError> {
        validate_projection_children(kind, &children)?;
        Ok(Self {
            kind,
            children: children.into_boxed_slice(),
        })
    }
}

/// Terminal failure while projecting an executable body container.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirBodyProjectionError {
    #[error("a HIR body child ordinal does not fit u32")]
    OrdinalOverflow,
    #[error("the body retains recovery syntax")]
    Recovery,
    #[error("the required body is missing")]
    Missing,
    #[error("the body retains an error item")]
    Error,
    #[error("the body child sequence does not match its executable kind")]
    InvalidChildren,
}

impl From<HirBodyChildEdgeError> for HirBodyProjectionError {
    fn from(error: HirBodyChildEdgeError) -> Self {
        match error {
            HirBodyChildEdgeError::OrdinalOverflow => Self::OrdinalOverflow,
        }
    }
}

/// A role-qualified body projection emitted by an expression or statement
/// owner. The body kind and child order remain the shared projection above;
/// only the owner-specific role is parameterized here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBodyRoleProjection<R> {
    role: R,
    projection: HirBodyProjection,
}

impl<R> HirBodyRoleProjection<R> {
    pub const fn role(&self) -> &R {
        &self.role
    }

    pub const fn kind(&self) -> HirBodyKind {
        self.projection.kind()
    }

    pub const fn children(&self) -> &[HirBodyChildEdge] {
        self.projection.children()
    }

    pub const fn projection(&self) -> &HirBodyProjection {
        &self.projection
    }

    pub(crate) fn new(role: R, projection: HirBodyProjection) -> Self {
        Self { role, projection }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirBodyChild {
    Expression(ExprId),
    Statement(StmtId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirBodyChildRole {
    Expression,
    Statement { ordinal: u32 },
    Tail,
    RecoveryExpression,
    ThreadItem { ordinal: u32 },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirBodyChildEdge {
    child: HirBodyChild,
    role: HirBodyChildRole,
}

impl HirBodyChildEdge {
    pub(crate) const fn new(child: HirBodyChild, role: HirBodyChildRole) -> Self {
        Self { child, role }
    }

    pub const fn child(&self) -> HirBodyChild {
        self.child
    }

    pub const fn role(&self) -> HirBodyChildRole {
        self.role
    }
}

impl HirExprKind {
    /// Projects a body owned directly by this expression, if it has one.
    ///
    /// Nested Await and Choice bodies are intentionally excluded; their
    /// expression-owned inventories are issued by the expression owner.
    pub fn try_body_projection(&self) -> Result<Option<HirBodyProjection>, HirBodyProjectionError> {
        match self {
            Self::Thread(expression) => expression.try_body_projection().map(Some),
            Self::Block(expression) => expression.try_body_projection().map(Some),
            Self::ComputationBlock(expression) => expression.try_body_projection().map(Some),
            Self::NamedBlock(expression) => expression.try_body_projection().map(Some),
            Self::Loop(expression) => expression.try_body_projection().map(Some),
            Self::Unit
            | Self::Literal(_)
            | Self::EntityReference(_)
            | Self::LifetimePath(_)
            | Self::Path(_)
            | Self::ShortVariant(_)
            | Self::Placeholder(_)
            | Self::Tuple(_)
            | Self::BracketSequence(_)
            | Self::NumericBracketSequence(_)
            | Self::ArrayRepeat(_)
            | Self::Call(_)
            | Self::Select(_)
            | Self::Index(_)
            | Self::Pipe(_)
            | Self::Try(_)
            | Self::Await(_)
            | Self::Choice(_)
            | Self::Range(_)
            | Self::Record(_)
            | Self::RecordLiteral(_)
            | Self::Binary(_)
            | Self::Borrow(_)
            | Self::Dereference(_)
            | Self::Closure(_)
            | Self::Unary(_)
            | Self::If(_)
            | Self::IfLet(_)
            | Self::Match(_)
            | Self::DialogueContentApplication(_)
            | Self::PostfixBracket(_)
            | Self::Error(_)
            | Self::ForSynthetic(_) => Ok(None),
        }
    }
}

impl HirBlockExpr {
    pub fn try_body_projection(&self) -> Result<HirBodyProjection, HirBodyProjectionError> {
        try_ordinary_body_projection(self.statements(), self.tail())
    }
}

impl HirComputationBlockExpr {
    pub fn try_body_projection(&self) -> Result<HirBodyProjection, HirBodyProjectionError> {
        try_ordinary_body_projection(self.statements(), self.tail())
    }
}

impl HirNamedBlockExpr {
    pub fn try_body_projection(&self) -> Result<HirBodyProjection, HirBodyProjectionError> {
        try_ordinary_body_projection(self.statements(), self.tail())
    }
}

impl HirLoopExpr {
    pub fn try_body_projection(&self) -> Result<HirBodyProjection, HirBodyProjectionError> {
        try_ordinary_body_projection(self.statements(), self.tail())
    }
}

impl HirThreadExpr {
    pub fn try_body_projection(&self) -> Result<HirBodyProjection, HirBodyProjectionError> {
        self.body().try_body_projection()
    }
}

impl HirFunctionBody {
    /// Projects the executable function body, rejecting retained recovery.
    pub fn try_body_projection(&self) -> Result<HirBodyProjection, HirBodyProjectionError> {
        match self {
            Self::Block {
                statements, tail, ..
            } => try_ordinary_body_projection(statements, *tail),
            Self::Error(_) => Err(HirBodyProjectionError::Error),
        }
    }

    /// Returns the accepted body children in semantic order.
    ///
    /// # Panics
    ///
    /// Panics if an unrejected body exceeds the accepted `u32` ordinal space.
    pub fn child_edges(&self) -> Vec<HirBodyChildEdge> {
        self.try_child_edges()
            .expect("accepted HIR function bodies fit checked u32 limits")
    }

    pub fn try_child_edges(&self) -> Result<Vec<HirBodyChildEdge>, HirBodyChildEdgeError> {
        match self {
            Self::Block {
                statements, tail, ..
            } => try_block_edges(statements, *tail),
            Self::Error(expression) => Ok(vec![HirBodyChildEdge::new(
                HirBodyChild::Expression(*expression),
                HirBodyChildRole::RecoveryExpression,
            )]),
        }
    }
}

impl HirPredicateBody {
    /// Projects the executable predicate body, rejecting retained recovery.
    pub fn try_body_projection(&self) -> Result<HirBodyProjection, HirBodyProjectionError> {
        match self {
            Self::Expression { expression, .. } => Ok(HirBodyProjection::expression(*expression)),
            Self::Block {
                statements, tail, ..
            } => try_ordinary_body_projection(statements, *tail),
            Self::Error { .. } => Err(HirBodyProjectionError::Error),
        }
    }

    /// Returns the accepted body children in semantic order.
    ///
    /// # Panics
    ///
    /// Panics if an unrejected body exceeds the accepted `u32` ordinal space.
    pub fn child_edges(&self) -> Vec<HirBodyChildEdge> {
        self.try_child_edges()
            .expect("accepted HIR predicate bodies fit checked u32 limits")
    }

    pub fn try_child_edges(&self) -> Result<Vec<HirBodyChildEdge>, HirBodyChildEdgeError> {
        try_logical_body_edges(
            self,
            |body| match body {
                Self::Expression { expression, .. } => Some((*expression, false)),
                Self::Error { expression, .. } => Some((*expression, true)),
                Self::Block { .. } => None,
            },
            |body| match body {
                Self::Block {
                    statements, tail, ..
                } => Some((statements.as_ref(), *tail)),
                Self::Expression { .. } | Self::Error { .. } => None,
            },
        )
    }
}

impl HirProofBody {
    /// Projects the executable proof body, rejecting retained recovery.
    pub fn try_body_projection(&self) -> Result<HirBodyProjection, HirBodyProjectionError> {
        match self {
            Self::Expression { expression, .. } => Ok(HirBodyProjection::expression(*expression)),
            Self::Block {
                statements, tail, ..
            } => try_ordinary_body_projection(statements, *tail),
            Self::Error { .. } => Err(HirBodyProjectionError::Error),
        }
    }

    /// Returns the accepted body children in semantic order.
    ///
    /// # Panics
    ///
    /// Panics if an unrejected body exceeds the accepted `u32` ordinal space.
    pub fn child_edges(&self) -> Vec<HirBodyChildEdge> {
        self.try_child_edges()
            .expect("accepted HIR proof bodies fit checked u32 limits")
    }

    pub fn try_child_edges(&self) -> Result<Vec<HirBodyChildEdge>, HirBodyChildEdgeError> {
        try_logical_body_edges(
            self,
            |body| match body {
                Self::Expression { expression, .. } => Some((*expression, false)),
                Self::Error { expression, .. } => Some((*expression, true)),
                Self::Block { .. } => None,
            },
            |body| match body {
                Self::Block {
                    statements, tail, ..
                } => Some((statements.as_ref(), *tail)),
                Self::Expression { .. } | Self::Error { .. } => None,
            },
        )
    }
}

impl HirContextualStmtBody {
    /// Projects the exact contextual body, retaining an empty body and
    /// rejecting any terminal Thread recovery item.
    pub fn try_body_projection(&self) -> Result<HirBodyProjection, HirBodyProjectionError> {
        match self {
            Self::Ordinary { statements, .. } => {
                HirBodyProjection::try_new(HirBodyKind::Ordinary, try_statement_edges(statements)?)
            }
            Self::Thread(body) => body.try_body_projection(),
        }
    }

    /// Returns the accepted contextual body children in semantic order.
    ///
    /// # Panics
    ///
    /// Panics if an unrejected body exceeds the accepted `u32` ordinal space.
    pub fn child_edges(&self) -> Vec<HirBodyChildEdge> {
        self.try_child_edges()
            .expect("accepted contextual bodies fit checked u32 limits")
    }

    pub fn try_child_edges(&self) -> Result<Vec<HirBodyChildEdge>, HirBodyChildEdgeError> {
        match self {
            Self::Ordinary { statements, .. } => try_statement_edges(statements),
            Self::Thread(body) => body.try_child_edges(),
        }
    }
}

impl HirThreadBody {
    /// Projects the heterogeneous Thread body in its exact source order.
    /// Explicit error flow items are terminal and cannot enter an executable
    /// body projection; an empty item list remains a valid Thread body.
    pub fn try_body_projection(&self) -> Result<HirBodyProjection, HirBodyProjectionError> {
        if self
            .items()
            .iter()
            .any(|item| matches!(item, HirThreadFlowItem::Error(_)))
        {
            return Err(HirBodyProjectionError::Error);
        }
        HirBodyProjection::try_new(HirBodyKind::Thread, self.try_child_edges()?)
    }

    /// Returns the accepted Thread body children in semantic order.
    ///
    /// # Panics
    ///
    /// Panics if an unrejected body exceeds the accepted `u32` ordinal space.
    pub fn child_edges(&self) -> Vec<HirBodyChildEdge> {
        self.try_child_edges()
            .expect("accepted Thread body items fit checked u32 limits")
    }

    pub fn try_child_edges(&self) -> Result<Vec<HirBodyChildEdge>, HirBodyChildEdgeError> {
        self.items()
            .iter()
            .enumerate()
            .map(|(ordinal, item)| {
                let child = match item {
                    HirThreadFlowItem::DialogueApplication(expression) => {
                        HirBodyChild::Expression(*expression)
                    }
                    HirThreadFlowItem::Statement(statement)
                    | HirThreadFlowItem::Choice(statement)
                    | HirThreadFlowItem::If(statement)
                    | HirThreadFlowItem::IfLet(statement)
                    | HirThreadFlowItem::Match(statement)
                    | HirThreadFlowItem::While(statement)
                    | HirThreadFlowItem::WhileLet(statement)
                    | HirThreadFlowItem::For(statement)
                    | HirThreadFlowItem::Select(statement)
                    | HirThreadFlowItem::SourceLocale(statement)
                    | HirThreadFlowItem::Scope(statement)
                    | HirThreadFlowItem::Include(statement)
                    | HirThreadFlowItem::Error(statement) => HirBodyChild::Statement(*statement),
                };
                Ok(HirBodyChildEdge::new(
                    child,
                    HirBodyChildRole::ThreadItem {
                        ordinal: checked_ordinal(ordinal)?,
                    },
                ))
            })
            .collect()
    }
}

fn try_logical_body_edges<T>(
    body: &T,
    expression: impl FnOnce(&T) -> Option<(ExprId, bool)>,
    block: impl FnOnce(&T) -> Option<(&[StmtId], ExprId)>,
) -> Result<Vec<HirBodyChildEdge>, HirBodyChildEdgeError> {
    if let Some((expression, recovery)) = expression(body) {
        return Ok(vec![HirBodyChildEdge::new(
            HirBodyChild::Expression(expression),
            if recovery {
                HirBodyChildRole::RecoveryExpression
            } else {
                HirBodyChildRole::Expression
            },
        )]);
    }
    let (statements, tail) = block(body).expect("closed logical body family has one projection");
    try_block_edges(statements, tail)
}

fn try_block_edges(
    statements: &[StmtId],
    tail: ExprId,
) -> Result<Vec<HirBodyChildEdge>, HirBodyChildEdgeError> {
    let mut edges = try_statement_edges(statements)?;
    edges.push(HirBodyChildEdge::new(
        HirBodyChild::Expression(tail),
        HirBodyChildRole::Tail,
    ));
    Ok(edges)
}

fn try_ordinary_body_projection(
    statements: &[StmtId],
    tail: ExprId,
) -> Result<HirBodyProjection, HirBodyProjectionError> {
    HirBodyProjection::try_new(HirBodyKind::Ordinary, try_block_edges(statements, tail)?)
}

pub(crate) fn try_statement_edges(
    statements: &[StmtId],
) -> Result<Vec<HirBodyChildEdge>, HirBodyChildEdgeError> {
    statements
        .iter()
        .enumerate()
        .map(|(ordinal, statement)| {
            Ok(HirBodyChildEdge::new(
                HirBodyChild::Statement(*statement),
                HirBodyChildRole::Statement {
                    ordinal: checked_ordinal(ordinal)?,
                },
            ))
        })
        .collect()
}

fn checked_ordinal(value: usize) -> Result<u32, HirBodyChildEdgeError> {
    u32::try_from(value).map_err(|_| HirBodyChildEdgeError::OrdinalOverflow)
}

fn validate_projection_children(
    kind: HirBodyKind,
    children: &[HirBodyChildEdge],
) -> Result<(), HirBodyProjectionError> {
    match kind {
        HirBodyKind::Expression => {
            if !matches!(children, [edge]
                if matches!(edge.child(), HirBodyChild::Expression(_))
                    && edge.role() == HirBodyChildRole::Expression)
            {
                return Err(HirBodyProjectionError::InvalidChildren);
            }
        }
        HirBodyKind::Ordinary => {
            let mut statement_ordinal = 0_u32;
            let mut saw_tail = false;
            for (position, edge) in children.iter().enumerate() {
                match (edge.child(), edge.role()) {
                    (HirBodyChild::Statement(_), HirBodyChildRole::Statement { ordinal })
                        if ordinal == statement_ordinal =>
                    {
                        statement_ordinal = statement_ordinal
                            .checked_add(1)
                            .ok_or(HirBodyProjectionError::OrdinalOverflow)?;
                    }
                    (HirBodyChild::Expression(_), HirBodyChildRole::Tail)
                        if position + 1 == children.len() && !saw_tail =>
                    {
                        saw_tail = true;
                    }
                    (
                        _,
                        HirBodyChildRole::RecoveryExpression
                        | HirBodyChildRole::ThreadItem { .. }
                        | HirBodyChildRole::Expression
                        | HirBodyChildRole::Tail
                        | HirBodyChildRole::Statement { .. },
                    ) => {
                        return Err(HirBodyProjectionError::InvalidChildren);
                    }
                }
            }
        }
        HirBodyKind::Thread => {
            for (ordinal, edge) in children.iter().enumerate() {
                let ordinal =
                    u32::try_from(ordinal).map_err(|_| HirBodyProjectionError::OrdinalOverflow)?;
                if !matches!(
                    (edge.child(), edge.role()),
                    (
                        HirBodyChild::Expression(_) | HirBodyChild::Statement(_),
                        HirBodyChildRole::ThreadItem { ordinal: value }
                    ) if value == ordinal
                ) {
                    return Err(HirBodyProjectionError::InvalidChildren);
                }
            }
        }
    }
    Ok(())
}
