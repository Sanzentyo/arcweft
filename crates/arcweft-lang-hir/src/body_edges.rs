//! Typed declaration and nested-body roots for semantic path construction.

use crate::{
    expr::{HirThreadBody, HirThreadFlowItem},
    identity::{ExprId, StmtId},
    item::{HirFunctionBody, HirPredicateBody, HirProofBody},
    stmt::HirContextualStmtBody,
};

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
    pub fn child_edges(&self) -> Vec<HirBodyChildEdge> {
        match self {
            Self::Block {
                statements, tail, ..
            } => block_edges(statements, *tail),
            Self::Error(expression) => vec![HirBodyChildEdge::new(
                HirBodyChild::Expression(*expression),
                HirBodyChildRole::RecoveryExpression,
            )],
        }
    }
}

impl HirPredicateBody {
    pub fn child_edges(&self) -> Vec<HirBodyChildEdge> {
        logical_body_edges(
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
    pub fn child_edges(&self) -> Vec<HirBodyChildEdge> {
        logical_body_edges(
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

    pub fn child_edges(&self) -> Vec<HirBodyChildEdge> {
        match self {
            Self::Ordinary { statements, .. } => statement_edges(statements),
            Self::Thread(body) => body.child_edges(),
        }
    }
}

impl HirThreadBody {
    pub fn child_edges(&self) -> Vec<HirBodyChildEdge> {
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
                HirBodyChildEdge::new(
                    child,
                    HirBodyChildRole::ThreadItem {
                        ordinal: u32::try_from(ordinal)
                            .expect("accepted Thread body items fit checked u32 limits"),
                    },
                )
            })
            .collect()
    }
}

fn logical_body_edges<T>(
    body: &T,
    expression: impl FnOnce(&T) -> Option<(ExprId, bool)>,
    block: impl FnOnce(&T) -> Option<(&[StmtId], ExprId)>,
) -> Vec<HirBodyChildEdge> {
    if let Some((expression, recovery)) = expression(body) {
        return vec![HirBodyChildEdge::new(
            HirBodyChild::Expression(expression),
            if recovery {
                HirBodyChildRole::RecoveryExpression
            } else {
                HirBodyChildRole::Expression
            },
        )];
    }
    let (statements, tail) = block(body).expect("closed logical body family has one projection");
    block_edges(statements, tail)
}

fn block_edges(statements: &[StmtId], tail: ExprId) -> Vec<HirBodyChildEdge> {
    let mut edges = statement_edges(statements);
    edges.push(HirBodyChildEdge::new(
        HirBodyChild::Expression(tail),
        HirBodyChildRole::Tail,
    ));
    edges
}

fn statement_edges(statements: &[StmtId]) -> Vec<HirBodyChildEdge> {
    statements
        .iter()
        .enumerate()
        .map(|(ordinal, statement)| {
            HirBodyChildEdge::new(
                HirBodyChild::Statement(*statement),
                HirBodyChildRole::Statement {
                    ordinal: u32::try_from(ordinal)
                        .expect("accepted HIR statement bodies fit checked u32 limits"),
                },
            )
        })
        .collect()
}
