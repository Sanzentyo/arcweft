//! Typed direct-child authority for final HIR statements.

use crate::body_edges::{
    HirBodyChildEdgeError, HirBodyKind, HirBodyProjection, HirBodyProjectionError,
    HirBodyRoleProjection, try_statement_edges,
};
use crate::identity::{ExprId, LocalId, PatternId, StmtId, TypeId};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirStatementChildEdgeError {
    #[error("a statement child ordinal does not fit u32")]
    OrdinalOverflow,
}

/// One actual body container owned by a statement.
pub type HirStatementBodyProjection = HirBodyRoleProjection<HirStatementBodyRole>;

/// Construction error for the exhaustive statement body inventory.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirStatementBodyProjectionError {
    #[error("a statement body ordinal does not fit u32")]
    OrdinalOverflow,
    #[error(transparent)]
    Body(HirBodyProjectionError),
}

impl From<HirBodyProjectionError> for HirStatementBodyProjectionError {
    fn from(error: HirBodyProjectionError) -> Self {
        Self::Body(error)
    }
}

impl From<HirBodyChildEdgeError> for HirStatementBodyProjectionError {
    fn from(error: HirBodyChildEdgeError) -> Self {
        Self::Body(error.into())
    }
}

impl From<HirStatementChildEdgeError> for HirStatementBodyProjectionError {
    fn from(error: HirStatementChildEdgeError) -> Self {
        match error {
            HirStatementChildEdgeError::OrdinalOverflow => Self::OrdinalOverflow,
        }
    }
}

use super::{
    HirConditionalElseBranch, HirContextualStmtBody, HirSelectBranchHead, HirSelectStmt,
    HirStmtKind, HirStmtMatchArmBody, HirTrigger, HirUnsafeLifetimeBody,
};

/// One typed child owned directly by a statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStatementChild {
    Expression(ExprId),
    Statement(StmtId),
    Pattern(PatternId),
    Type(TypeId),
    Local(LocalId),
}

/// Stable semantic role of a nested statement body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStatementBodyRole {
    LetElse,
    On,
    UnsafeLifetime,
    Then,
    Else,
    MatchArm { arm: u32 },
    While,
    WhileLet,
    For,
    SelectBranch { branch: u32 },
    SourceLocale,
    Scope,
}

/// Closed direct-child role inventory for every [`HirStmtKind`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStatementChildRole {
    AssertionCondition {
        ordinal: u32,
    },
    Pattern,
    Annotation,
    Initializer,
    Input,
    Target,
    Value,
    BodyItem {
        body: HirStatementBodyRole,
        ordinal: u32,
    },
    ElseIf,
    TriggerExpression,
    TriggerPattern,
    TriggerSignalTarget,
    TriggerSignalValue,
    UnsafeReason,
    Condition,
    Scrutinee,
    Guard,
    MatchPattern {
        arm: u32,
    },
    MatchGuard {
        arm: u32,
    },
    MatchValue {
        arm: u32,
    },
    ForSource,
    ForIterator,
    ForNextValue,
    SelectOperand,
    SelectBinding {
        branch: u32,
    },
    SelectSource {
        branch: u32,
    },
    SelectPattern {
        branch: u32,
    },
}

/// One typed statement child and its exact semantic role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirStatementChildEdge {
    child: HirStatementChild,
    role: HirStatementChildRole,
}

impl HirStatementChildEdge {
    const fn new(child: HirStatementChild, role: HirStatementChildRole) -> Self {
        Self { child, role }
    }

    pub const fn child(&self) -> HirStatementChild {
        self.child
    }

    pub const fn role(&self) -> HirStatementChildRole {
        self.role
    }
}

impl HirStmtKind {
    /// Returns every actual nested body owned by this statement in semantic
    /// source order. Empty ordinary and Thread bodies are retained. A Match
    /// arm with an expression is an `Expression` body projection, while a
    /// contextual arm preserves its ordinary/Thread kind.
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive match is the sole body-container inventory for all statement families"
    )]
    pub fn body_projections(
        &self,
    ) -> Result<Vec<HirStatementBodyProjection>, HirStatementBodyProjectionError> {
        let mut bodies = Vec::new();
        match self {
            Self::LetElse { else_body, .. } => push_ordinary_body_projection(
                &mut bodies,
                HirStatementBodyRole::LetElse,
                else_body,
            )?,
            Self::On { body, .. } => {
                push_ordinary_body_projection(&mut bodies, HirStatementBodyRole::On, body)?;
            }
            Self::UnsafeLifetime { body, .. } => match body {
                HirUnsafeLifetimeBody::Block { statements, .. } => push_ordinary_body_projection(
                    &mut bodies,
                    HirStatementBodyRole::UnsafeLifetime,
                    statements,
                )?,
                HirUnsafeLifetimeBody::Missing => {
                    return Err(HirStatementBodyProjectionError::Body(
                        HirBodyProjectionError::Missing,
                    ));
                }
            },
            Self::If(statement) => {
                push_contextual_body_projection(
                    &mut bodies,
                    HirStatementBodyRole::Then,
                    statement.then_body(),
                )?;
                push_else_body_projection(&mut bodies, statement.else_branch())?;
            }
            Self::IfLet(statement) => {
                push_contextual_body_projection(
                    &mut bodies,
                    HirStatementBodyRole::Then,
                    statement.then_body(),
                )?;
                push_else_body_projection(&mut bodies, statement.else_branch())?;
            }
            Self::Match(statement) => {
                for (arm, row) in statement.arms().iter().enumerate() {
                    let role = HirStatementBodyRole::MatchArm {
                        arm: checked_ordinal(arm)?,
                    };
                    match row.body() {
                        HirStmtMatchArmBody::Expression(expression) => {
                            bodies.push(HirBodyRoleProjection::new(
                                role,
                                HirBodyProjection::expression(*expression),
                            ));
                        }
                        HirStmtMatchArmBody::Body(body) => {
                            push_contextual_body_projection(&mut bodies, role, body)?;
                        }
                    }
                }
            }
            Self::While(statement) => push_contextual_body_projection(
                &mut bodies,
                HirStatementBodyRole::While,
                statement.body(),
            )?,
            Self::WhileLet(statement) => push_contextual_body_projection(
                &mut bodies,
                HirStatementBodyRole::WhileLet,
                statement.body(),
            )?,
            Self::For(statement) => push_contextual_body_projection(
                &mut bodies,
                HirStatementBodyRole::For,
                statement.body(),
            )?,
            Self::Select(HirSelectStmt::Branches { branches, .. }) => {
                for (branch, value) in branches.iter().enumerate() {
                    push_contextual_body_projection(
                        &mut bodies,
                        HirStatementBodyRole::SelectBranch {
                            branch: checked_ordinal(branch)?,
                        },
                        value.body(),
                    )?;
                }
            }
            Self::SourceLocale(statement) => push_contextual_body_projection(
                &mut bodies,
                HirStatementBodyRole::SourceLocale,
                statement.body(),
            )?,
            Self::Scope(statement) => push_contextual_body_projection(
                &mut bodies,
                HirStatementBodyRole::Scope,
                statement.body(),
            )?,
            Self::Select(HirSelectStmt::Operand(_))
            | Self::Assertion { .. }
            | Self::Let { .. }
            | Self::Assign { .. }
            | Self::Return { .. }
            | Self::Out { .. }
            | Self::Goto { .. }
            | Self::Defer { .. }
            | Self::Yield { .. }
            | Self::Signal { .. }
            | Self::LifetimeSet { .. }
            | Self::Wait { .. }
            | Self::Choice { .. }
            | Self::Close { .. }
            | Self::Include(_)
            | Self::Break { .. }
            | Self::Continue { .. }
            | Self::Expression { .. }
            | Self::ProofCall { .. }
            | Self::Error => {}
        }
        Ok(bodies)
    }

    /// Returns all direct typed children in semantic/source order.
    ///
    /// # Panics
    ///
    /// Panics if an unrejected HIR sequence exceeds the accepted `u32`
    /// ordinal space. Fallible semantic consumers must use
    /// [`Self::try_child_edges`].
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive match is the sole direct-child order authority for all statement families"
    )]
    pub fn child_edges(&self) -> Vec<HirStatementChildEdge> {
        self.try_child_edges()
            .expect("accepted HIR statement children fit checked u32 limits")
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive match is the sole direct-child order authority for all statement families"
    )]
    pub fn try_child_edges(
        &self,
    ) -> Result<Vec<HirStatementChildEdge>, HirStatementChildEdgeError> {
        let mut edges = Vec::new();
        match self {
            Self::Assertion { conditions, .. } => {
                for (ordinal, condition) in conditions.iter().enumerate() {
                    push_expression(
                        &mut edges,
                        *condition,
                        HirStatementChildRole::AssertionCondition {
                            ordinal: checked_ordinal(ordinal)?,
                        },
                    );
                }
            }
            Self::Let {
                pattern,
                annotation,
                initializer,
                locals: _,
            } => {
                push_pattern(&mut edges, *pattern, HirStatementChildRole::Pattern);
                push_optional_type(&mut edges, *annotation);
                push_expression(&mut edges, *initializer, HirStatementChildRole::Initializer);
            }
            Self::Assign { target, value }
            | Self::Signal { target, value }
            | Self::LifetimeSet { target, value } => {
                push_expression(&mut edges, *target, HirStatementChildRole::Target);
                push_expression(&mut edges, *value, HirStatementChildRole::Value);
            }
            Self::LetElse {
                pattern,
                annotation,
                initializer,
                else_body,
                locals: _,
                ..
            } => {
                push_pattern(&mut edges, *pattern, HirStatementChildRole::Pattern);
                push_optional_type(&mut edges, *annotation);
                push_expression(&mut edges, *initializer, HirStatementChildRole::Initializer);
                push_statements(&mut edges, HirStatementBodyRole::LetElse, else_body)?;
            }
            Self::Return { value } | Self::Out { value, .. } => {
                push_expression(&mut edges, *value, HirStatementChildRole::Value);
            }
            Self::Goto { target } | Self::Wait { target } | Self::Close { target } => {
                push_expression(&mut edges, *target, HirStatementChildRole::Target);
            }
            Self::Defer { expression, .. }
            | Self::Yield { expression }
            | Self::Expression { expression }
            | Self::ProofCall { call: expression } => {
                push_expression(&mut edges, *expression, HirStatementChildRole::Value);
            }
            Self::On { trigger, body, .. } => {
                push_trigger(&mut edges, trigger);
                push_statements(&mut edges, HirStatementBodyRole::On, body)?;
            }
            Self::UnsafeLifetime { audit, body } => {
                if let Some(reason) = audit.reason() {
                    push_expression(&mut edges, reason, HirStatementChildRole::UnsafeReason);
                }
                if let HirUnsafeLifetimeBody::Block { statements, .. } = body {
                    push_statements(&mut edges, HirStatementBodyRole::UnsafeLifetime, statements)?;
                }
            }
            Self::Choice { choice } => {
                push_expression(&mut edges, *choice, HirStatementChildRole::Input);
            }
            Self::If(statement) => {
                push_expression(
                    &mut edges,
                    statement.condition(),
                    HirStatementChildRole::Condition,
                );
                push_contextual_body(
                    &mut edges,
                    HirStatementBodyRole::Then,
                    statement.then_body(),
                )?;
                push_else(&mut edges, statement.else_branch())?;
            }
            Self::IfLet(statement) => {
                push_pattern(
                    &mut edges,
                    statement.pattern(),
                    HirStatementChildRole::Pattern,
                );
                push_expression(
                    &mut edges,
                    statement.scrutinee(),
                    HirStatementChildRole::Scrutinee,
                );
                if let Some(guard) = statement.guard() {
                    push_expression(&mut edges, guard, HirStatementChildRole::Guard);
                }
                push_contextual_body(
                    &mut edges,
                    HirStatementBodyRole::Then,
                    statement.then_body(),
                )?;
                push_else(&mut edges, statement.else_branch())?;
            }
            Self::Match(statement) => {
                push_expression(
                    &mut edges,
                    statement.scrutinee(),
                    HirStatementChildRole::Scrutinee,
                );
                for (arm, row) in statement.arms().iter().enumerate() {
                    let arm = checked_ordinal(arm)?;
                    push_pattern(
                        &mut edges,
                        row.pattern(),
                        HirStatementChildRole::MatchPattern { arm },
                    );
                    if let Some(guard) = row.guard() {
                        push_expression(
                            &mut edges,
                            guard,
                            HirStatementChildRole::MatchGuard { arm },
                        );
                    }
                    match row.body() {
                        HirStmtMatchArmBody::Expression(value) => push_expression(
                            &mut edges,
                            *value,
                            HirStatementChildRole::MatchValue { arm },
                        ),
                        HirStmtMatchArmBody::Body(body) => push_contextual_body(
                            &mut edges,
                            HirStatementBodyRole::MatchArm { arm },
                            body,
                        )?,
                    }
                }
            }
            Self::While(statement) => {
                push_expression(
                    &mut edges,
                    statement.condition(),
                    HirStatementChildRole::Condition,
                );
                push_contextual_body(&mut edges, HirStatementBodyRole::While, statement.body())?;
            }
            Self::WhileLet(statement) => {
                push_pattern(
                    &mut edges,
                    statement.pattern(),
                    HirStatementChildRole::Pattern,
                );
                push_expression(
                    &mut edges,
                    statement.scrutinee(),
                    HirStatementChildRole::Scrutinee,
                );
                if let Some(guard) = statement.guard() {
                    push_expression(&mut edges, guard, HirStatementChildRole::Guard);
                }
                push_contextual_body(&mut edges, HirStatementBodyRole::WhileLet, statement.body())?;
            }
            Self::For(statement) => {
                push_expression(
                    &mut edges,
                    statement.source(),
                    HirStatementChildRole::ForSource,
                );
                push_expression(
                    &mut edges,
                    statement.iterator(),
                    HirStatementChildRole::ForIterator,
                );
                push_expression(
                    &mut edges,
                    statement.next_value(),
                    HirStatementChildRole::ForNextValue,
                );
                push_pattern(
                    &mut edges,
                    statement.pattern(),
                    HirStatementChildRole::Pattern,
                );
                push_contextual_body(&mut edges, HirStatementBodyRole::For, statement.body())?;
            }
            Self::Select(HirSelectStmt::Operand(operand)) => {
                push_expression(&mut edges, *operand, HirStatementChildRole::SelectOperand);
            }
            Self::Select(HirSelectStmt::Branches { branches, .. }) => {
                for (branch, row) in branches.iter().enumerate() {
                    let branch = checked_ordinal(branch)?;
                    match row.head() {
                        HirSelectBranchHead::Bind {
                            binding, source, ..
                        } => {
                            if let Some(local) = binding.resolved() {
                                edges.push(HirStatementChildEdge::new(
                                    HirStatementChild::Local(local),
                                    HirStatementChildRole::SelectBinding { branch },
                                ));
                            }
                            push_expression(
                                &mut edges,
                                *source,
                                HirStatementChildRole::SelectSource { branch },
                            );
                        }
                        HirSelectBranchHead::Frame { pattern, .. }
                        | HirSelectBranchHead::Event { pattern, .. } => push_pattern(
                            &mut edges,
                            *pattern,
                            HirStatementChildRole::SelectPattern { branch },
                        ),
                        HirSelectBranchHead::Recovered => {}
                    }
                    push_contextual_body(
                        &mut edges,
                        HirStatementBodyRole::SelectBranch { branch },
                        row.body(),
                    )?;
                }
            }
            Self::SourceLocale(statement) => push_contextual_body(
                &mut edges,
                HirStatementBodyRole::SourceLocale,
                statement.body(),
            )?,
            Self::Scope(statement) => {
                push_contextual_body(&mut edges, HirStatementBodyRole::Scope, statement.body())?;
            }
            Self::Break { value, .. } => {
                if let Some(value) = value {
                    push_expression(&mut edges, *value, HirStatementChildRole::Value);
                }
            }
            Self::Include(_) | Self::Continue { .. } | Self::Error => {}
        }
        Ok(edges)
    }
}

fn push_ordinary_body_projection(
    bodies: &mut Vec<HirStatementBodyProjection>,
    role: HirStatementBodyRole,
    statements: &[StmtId],
) -> Result<(), HirStatementBodyProjectionError> {
    bodies.push(HirBodyRoleProjection::new(
        role,
        HirBodyProjection::try_new(HirBodyKind::Ordinary, try_statement_edges(statements)?)?,
    ));
    Ok(())
}

fn push_contextual_body_projection(
    bodies: &mut Vec<HirStatementBodyProjection>,
    role: HirStatementBodyRole,
    body: &HirContextualStmtBody,
) -> Result<(), HirStatementBodyProjectionError> {
    bodies.push(HirBodyRoleProjection::new(
        role,
        body.try_body_projection()?,
    ));
    Ok(())
}

fn push_else_body_projection(
    bodies: &mut Vec<HirStatementBodyProjection>,
    branch: Option<&HirConditionalElseBranch>,
) -> Result<(), HirStatementBodyProjectionError> {
    if let Some(HirConditionalElseBranch::Body(body)) = branch {
        push_contextual_body_projection(bodies, HirStatementBodyRole::Else, body)?;
    }
    Ok(())
}

fn checked_ordinal(value: usize) -> Result<u32, HirStatementChildEdgeError> {
    u32::try_from(value).map_err(|_| HirStatementChildEdgeError::OrdinalOverflow)
}

fn push_expression(
    edges: &mut Vec<HirStatementChildEdge>,
    expression: ExprId,
    role: HirStatementChildRole,
) {
    edges.push(HirStatementChildEdge::new(
        HirStatementChild::Expression(expression),
        role,
    ));
}

fn push_pattern(
    edges: &mut Vec<HirStatementChildEdge>,
    pattern: PatternId,
    role: HirStatementChildRole,
) {
    edges.push(HirStatementChildEdge::new(
        HirStatementChild::Pattern(pattern),
        role,
    ));
}

fn push_optional_type(edges: &mut Vec<HirStatementChildEdge>, ty: Option<TypeId>) {
    if let Some(ty) = ty {
        edges.push(HirStatementChildEdge::new(
            HirStatementChild::Type(ty),
            HirStatementChildRole::Annotation,
        ));
    }
}

fn push_statements(
    edges: &mut Vec<HirStatementChildEdge>,
    body: HirStatementBodyRole,
    statements: &[StmtId],
) -> Result<(), HirStatementChildEdgeError> {
    for (ordinal, statement) in statements.iter().enumerate() {
        edges.push(HirStatementChildEdge::new(
            HirStatementChild::Statement(*statement),
            HirStatementChildRole::BodyItem {
                body,
                ordinal: checked_ordinal(ordinal)?,
            },
        ));
    }
    Ok(())
}

fn push_contextual_body(
    edges: &mut Vec<HirStatementChildEdge>,
    role: HirStatementBodyRole,
    body: &HirContextualStmtBody,
) -> Result<(), HirStatementChildEdgeError> {
    if let Some(statements) = body.ordinary_statements() {
        push_statements(edges, role, statements)?;
    }
    Ok(())
}

fn push_else(
    edges: &mut Vec<HirStatementChildEdge>,
    branch: Option<&HirConditionalElseBranch>,
) -> Result<(), HirStatementChildEdgeError> {
    match branch {
        Some(HirConditionalElseBranch::Body(body)) => {
            push_contextual_body(edges, HirStatementBodyRole::Else, body)?;
        }
        Some(HirConditionalElseBranch::ElseIf(statement)) => {
            edges.push(HirStatementChildEdge::new(
                HirStatementChild::Statement(*statement),
                HirStatementChildRole::ElseIf,
            ));
        }
        None => {}
    }
    Ok(())
}

fn push_trigger(edges: &mut Vec<HirStatementChildEdge>, trigger: &HirTrigger) {
    match trigger {
        HirTrigger::Input(pattern)
        | HirTrigger::Event(pattern)
        | HirTrigger::Select(pattern)
        | HirTrigger::Task(pattern)
        | HirTrigger::Scope(pattern) => {
            push_pattern(edges, *pattern, HirStatementChildRole::TriggerPattern);
        }
        HirTrigger::Mark(_) | HirTrigger::Recovered(_) => {}
        HirTrigger::Signal { target, value } => {
            push_expression(edges, *target, HirStatementChildRole::TriggerSignalTarget);
            if let Some(value) = value {
                push_pattern(edges, *value, HirStatementChildRole::TriggerSignalValue);
            }
        }
        HirTrigger::Timeout(expression) | HirTrigger::Expression(expression) => {
            push_expression(edges, *expression, HirStatementChildRole::TriggerExpression);
        }
    }
}

#[cfg(test)]
mod ordinal_tests {
    use super::*;

    #[test]
    fn statement_child_ordinal_is_exact_and_rejects_one_over() {
        let exact = usize::try_from(u32::MAX).unwrap();
        assert_eq!(checked_ordinal(exact), Ok(u32::MAX));
        if let Ok(one_over) = usize::try_from(u64::from(u32::MAX) + 1) {
            assert_eq!(
                checked_ordinal(one_over),
                Err(HirStatementChildEdgeError::OrdinalOverflow)
            );
        }
    }
}
