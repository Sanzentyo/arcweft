//! Typed declaration and nested-body roots for semantic path construction.

use crate::{
    expr::{HirThreadBody, HirThreadFlowItem},
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
    const fn new(child: HirBodyChild, role: HirBodyChildRole) -> Self {
        Self { child, role }
    }

    pub const fn child(&self) -> HirBodyChild {
        self.child
    }

    pub const fn role(&self) -> HirBodyChildRole {
        self.role
    }
}

impl HirFunctionBody {
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
    /// Ordinary statement bodies project exact roots here. Thread bodies keep
    /// their heterogeneous `HirThreadFlowItem` authority and are projected by
    /// the dedicated Thread body bridge.
    pub fn ordinary_child_edges(&self) -> Option<Vec<HirBodyChildEdge>> {
        self.ordinary_statements().map(statement_edges)
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

fn statement_edges(statements: &[StmtId]) -> Vec<HirBodyChildEdge> {
    try_statement_edges(statements).expect("accepted HIR statement body fits checked u32 limits")
}

fn try_statement_edges(
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
