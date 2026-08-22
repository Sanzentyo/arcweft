//! Typed direct-child authority for final HIR statements.

use crate::body_edges::HirBodyChildEdge;
use crate::identity::{ExprId, LocalId, PatternId, StmtId, TypeId};

use super::{
    HirConditionalElseBranch, HirContextualStmtBody, HirSelectBranchHead, HirSelectStmt,
    HirStmtKind, HirStmtMatchArmBody, HirTriggerPattern, HirUnsafeLifetimeBody,
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
    Defer,
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
    /// Returns all direct typed children in semantic/source order.
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive match is the sole direct-child order authority for all statement families"
    )]
    pub fn child_edges(&self) -> Vec<HirStatementChildEdge> {
        let mut edges = Vec::new();
        match self {
            Self::Assertion { conditions, .. } => {
                for (ordinal, condition) in conditions.iter().enumerate() {
                    push_expression(
                        &mut edges,
                        *condition,
                        HirStatementChildRole::AssertionCondition {
                            ordinal: checked_ordinal(ordinal),
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
                push_statements(&mut edges, HirStatementBodyRole::LetElse, else_body);
            }
            Self::LetChoice {
                pattern,
                choice,
                locals: _,
            }
            | Self::LetScope {
                pattern,
                scope_expr: choice,
                locals: _,
            }
            | Self::LetActionReceive {
                pattern,
                action: choice,
                locals: _,
            } => {
                push_pattern(&mut edges, *pattern, HirStatementChildRole::Pattern);
                push_expression(&mut edges, *choice, HirStatementChildRole::Input);
            }
            Self::Return { value } | Self::Out { value, .. } => {
                push_expression(&mut edges, *value, HirStatementChildRole::Value);
            }
            Self::Goto { target } | Self::Wait { target } | Self::Close { target } => {
                push_expression(&mut edges, *target, HirStatementChildRole::Target);
            }
            Self::DeferBlock { body, .. } => {
                push_statements(&mut edges, HirStatementBodyRole::Defer, body);
            }
            Self::Defer { expression, .. }
            | Self::Yield { expression }
            | Self::Expression { expression }
            | Self::ProofCall { call: expression } => {
                push_expression(&mut edges, *expression, HirStatementChildRole::Value);
            }
            Self::On { trigger, body, .. } => {
                push_trigger(&mut edges, trigger);
                push_statements(&mut edges, HirStatementBodyRole::On, body);
            }
            Self::UnsafeLifetime { audit, body } => {
                if let Some(reason) = audit.reason() {
                    push_expression(&mut edges, reason, HirStatementChildRole::UnsafeReason);
                }
                if let HirUnsafeLifetimeBody::Block { statements, .. } = body {
                    push_statements(&mut edges, HirStatementBodyRole::UnsafeLifetime, statements);
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
                );
                push_else(&mut edges, statement.else_branch());
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
                );
                push_else(&mut edges, statement.else_branch());
            }
            Self::Match(statement) => {
                push_expression(
                    &mut edges,
                    statement.scrutinee(),
                    HirStatementChildRole::Scrutinee,
                );
                for (arm, row) in statement.arms().iter().enumerate() {
                    let arm = checked_ordinal(arm);
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
                        ),
                    }
                }
            }
            Self::While(statement) => {
                push_expression(
                    &mut edges,
                    statement.condition(),
                    HirStatementChildRole::Condition,
                );
                push_contextual_body(&mut edges, HirStatementBodyRole::While, statement.body());
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
                push_contextual_body(&mut edges, HirStatementBodyRole::WhileLet, statement.body());
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
                push_contextual_body(&mut edges, HirStatementBodyRole::For, statement.body());
            }
            Self::Select(HirSelectStmt::Operand(operand)) => {
                push_expression(&mut edges, *operand, HirStatementChildRole::SelectOperand);
            }
            Self::Select(HirSelectStmt::Branches { branches, .. }) => {
                for (branch, row) in branches.iter().enumerate() {
                    let branch = checked_ordinal(branch);
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
                    );
                }
            }
            Self::SourceLocale(statement) => push_contextual_body(
                &mut edges,
                HirStatementBodyRole::SourceLocale,
                statement.body(),
            ),
            Self::Scope(statement) => {
                push_contextual_body(&mut edges, HirStatementBodyRole::Scope, statement.body())
            }
            Self::Break { value, .. } => {
                if let Some(value) = value {
                    push_expression(&mut edges, *value, HirStatementChildRole::Value);
                }
            }
            Self::Include(_) | Self::Continue { .. } | Self::Error => {}
        }
        edges
    }

    /// Returns heterogeneous Thread bodies nested directly in this statement.
    /// Ordinary bodies are already represented by [`Self::child_edges`]; a
    /// Thread body needs the HIR body-edge authority because it can retain both
    /// statement and dialogue-application expression roots.
    pub(crate) fn thread_body_edges(&self) -> Vec<(HirStatementBodyRole, Vec<HirBodyChildEdge>)> {
        let mut bodies = Vec::new();
        match self {
            Self::If(statement) => {
                push_thread_body(
                    &mut bodies,
                    HirStatementBodyRole::Then,
                    statement.then_body(),
                );
                if let Some(HirConditionalElseBranch::Body(body)) = statement.else_branch() {
                    push_thread_body(&mut bodies, HirStatementBodyRole::Else, body);
                }
            }
            Self::IfLet(statement) => {
                push_thread_body(
                    &mut bodies,
                    HirStatementBodyRole::Then,
                    statement.then_body(),
                );
                if let Some(HirConditionalElseBranch::Body(body)) = statement.else_branch() {
                    push_thread_body(&mut bodies, HirStatementBodyRole::Else, body);
                }
            }
            Self::Match(statement) => {
                for (arm, row) in statement.arms().iter().enumerate() {
                    if let HirStmtMatchArmBody::Body(body) = row.body() {
                        push_thread_body(
                            &mut bodies,
                            HirStatementBodyRole::MatchArm {
                                arm: checked_ordinal(arm),
                            },
                            body,
                        );
                    }
                }
            }
            Self::While(statement) => {
                push_thread_body(&mut bodies, HirStatementBodyRole::While, statement.body());
            }
            Self::WhileLet(statement) => {
                push_thread_body(
                    &mut bodies,
                    HirStatementBodyRole::WhileLet,
                    statement.body(),
                );
            }
            Self::For(statement) => {
                push_thread_body(&mut bodies, HirStatementBodyRole::For, statement.body());
            }
            Self::Select(HirSelectStmt::Branches { branches, .. }) => {
                for (branch, row) in branches.iter().enumerate() {
                    push_thread_body(
                        &mut bodies,
                        HirStatementBodyRole::SelectBranch {
                            branch: checked_ordinal(branch),
                        },
                        row.body(),
                    );
                }
            }
            Self::SourceLocale(statement) => {
                push_thread_body(
                    &mut bodies,
                    HirStatementBodyRole::SourceLocale,
                    statement.body(),
                );
            }
            Self::Scope(statement) => {
                push_thread_body(&mut bodies, HirStatementBodyRole::Scope, statement.body());
            }
            _ => {}
        }
        bodies
    }
}

fn push_thread_body(
    bodies: &mut Vec<(HirStatementBodyRole, Vec<HirBodyChildEdge>)>,
    role: HirStatementBodyRole,
    body: &HirContextualStmtBody,
) {
    if let Some(body) = body.thread_body() {
        bodies.push((role, body.child_edges()));
    }
}

fn checked_ordinal(value: usize) -> u32 {
    u32::try_from(value).expect("accepted HIR child sequences fit the checked u32 limits")
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
) {
    for (ordinal, statement) in statements.iter().enumerate() {
        edges.push(HirStatementChildEdge::new(
            HirStatementChild::Statement(*statement),
            HirStatementChildRole::BodyItem {
                body,
                ordinal: checked_ordinal(ordinal),
            },
        ));
    }
}

fn push_contextual_body(
    edges: &mut Vec<HirStatementChildEdge>,
    role: HirStatementBodyRole,
    body: &HirContextualStmtBody,
) {
    if let Some(statements) = body.ordinary_statements() {
        push_statements(edges, role, statements);
    }
}

fn push_else(edges: &mut Vec<HirStatementChildEdge>, branch: Option<&HirConditionalElseBranch>) {
    match branch {
        Some(HirConditionalElseBranch::Body(body)) => {
            push_contextual_body(edges, HirStatementBodyRole::Else, body);
        }
        Some(HirConditionalElseBranch::ElseIf(statement)) => {
            edges.push(HirStatementChildEdge::new(
                HirStatementChild::Statement(*statement),
                HirStatementChildRole::ElseIf,
            ));
        }
        None => {}
    }
}

fn push_trigger(edges: &mut Vec<HirStatementChildEdge>, trigger: &HirTriggerPattern) {
    match trigger {
        HirTriggerPattern::Input(pattern)
        | HirTriggerPattern::Event(pattern)
        | HirTriggerPattern::Mark(pattern)
        | HirTriggerPattern::Select(pattern)
        | HirTriggerPattern::Task(pattern)
        | HirTriggerPattern::Scope(pattern) => {
            push_pattern(edges, *pattern, HirStatementChildRole::TriggerPattern)
        }
        HirTriggerPattern::Signal { target, value } => {
            push_expression(edges, *target, HirStatementChildRole::TriggerSignalTarget);
            if let Some(value) = value {
                push_pattern(edges, *value, HirStatementChildRole::TriggerSignalValue);
            }
        }
        HirTriggerPattern::Timeout(expression) | HirTriggerPattern::Expr(expression) => {
            push_expression(edges, *expression, HirStatementChildRole::TriggerExpression);
        }
    }
}
