//! Expression-owned pattern, statement, and heterogeneous body topology.

use thiserror::Error;

use super::child_edges::extend_path;
use super::{
    HirChoiceItem, HirChoiceOptionField, HirChoicePlanItem, HirExprKind, HirNestedExpressionPath,
    HirNestedExpressionPathSegment,
};
use crate::body_edges::HirBodyChildEdge;
use crate::dialogue_application::{HirDialogueContentApplication, HirLinePlanItem};
use crate::identity::{PatternId, StmtId};
use crate::stmt::HirTriggerPattern;

/// One non-expression child rooted directly in an expression-owned body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirExpressionOwnedChild {
    Pattern(PatternId),
    Statement(StmtId),
    Body(HirBodyChildEdge),
}

/// One typed, declaration-relative edge from an expression into a pattern,
/// statement, or heterogeneous body child not represented by expression
/// child edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpressionOwnedChildEdge {
    child: HirExpressionOwnedChild,
    role: HirExpressionOwnedBodyRole,
}

impl HirExpressionOwnedChildEdge {
    pub const fn child(&self) -> HirExpressionOwnedChild {
        self.child
    }

    pub const fn role(&self) -> &HirExpressionOwnedBodyRole {
        &self.role
    }

    const fn new(child: HirExpressionOwnedChild, role: HirExpressionOwnedBodyRole) -> Self {
        Self { child, role }
    }
}

/// Closed roles for every non-expression root retained below an expression.
/// Nested Choice and line-plan coordinates reuse the same typed path segments
/// as ordinary expression children.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirExpressionOwnedBodyRole {
    ClosureParameterPattern {
        parameter: u32,
    },
    IfLetPattern,
    AwaitBranchPattern {
        branch: u32,
    },
    AwaitBranchBody {
        branch: u32,
    },
    ChoiceLetStatement {
        path: HirNestedExpressionPath,
    },
    ChoiceForPattern {
        path: HirNestedExpressionPath,
    },
    ChoiceMatchArmPattern {
        path: HirNestedExpressionPath,
        arm: u32,
    },
    ChoiceOptionForPattern {
        path: HirNestedExpressionPath,
    },
    ChoiceOptionSelectBody {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoiceOptionLetStatement {
        path: HirNestedExpressionPath,
        field: u32,
    },
    ChoicePlanTimeoutBody {
        path: HirNestedExpressionPath,
    },
    ChoicePlanCancelTrigger {
        path: HirNestedExpressionPath,
    },
    ChoicePlanCancelBody {
        path: HirNestedExpressionPath,
    },
    ChoicePlanOnSelectPattern {
        path: HirNestedExpressionPath,
    },
    ChoicePlanOnSelectBody {
        path: HirNestedExpressionPath,
    },
    DialogueLinePlanStatement {
        path: HirNestedExpressionPath,
        role: HirLinePlanStatementRole,
    },
    DialogueLinePlanLet {
        path: HirNestedExpressionPath,
    },
}

/// Source role of a statement retained by a dialogue line plan.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLinePlanStatementRole {
    Init { statement: u32 },
    Thread,
    On,
    Statement,
    CancelRule,
    Error,
}

/// Construction error for expression-owned topology.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirExpressionOwnedChildEdgeError {
    #[error("an expression-owned child ordinal does not fit u32")]
    OrdinalOverflow,
    #[error("an expression-owned child has no nested structural coordinate")]
    EmptyNestedPath,
}

impl HirExprKind {
    /// Returns every pattern, statement, and heterogeneous body child rooted
    /// below this expression but absent from [`Self::child_edges`].
    pub fn expression_owned_child_edges(
        &self,
    ) -> Result<Vec<HirExpressionOwnedChildEdge>, HirExpressionOwnedChildEdgeError> {
        let mut edges = Vec::new();
        match self {
            Self::Closure(expression) => {
                for (parameter, value) in expression.parameters().iter().enumerate() {
                    push_owned_edge(
                        &mut edges,
                        HirExpressionOwnedChild::Pattern(value.pattern()),
                        HirExpressionOwnedBodyRole::ClosureParameterPattern {
                            parameter: owned_ordinal(parameter)?,
                        },
                    );
                }
            }
            Self::IfLet(expression) => {
                push_owned_edge(
                    &mut edges,
                    HirExpressionOwnedChild::Pattern(expression.pattern()),
                    HirExpressionOwnedBodyRole::IfLetPattern,
                );
            }
            Self::Await(expression) => {
                for (branch, value) in expression.branches().iter().enumerate() {
                    let branch = owned_ordinal(branch)?;
                    if let Some(pattern) = value.pattern() {
                        push_owned_edge(
                            &mut edges,
                            HirExpressionOwnedChild::Pattern(pattern),
                            HirExpressionOwnedBodyRole::AwaitBranchPattern { branch },
                        );
                    }
                    push_owned_body_edges(
                        &mut edges,
                        value
                            .body()
                            .try_child_edges()
                            .map_err(|_| HirExpressionOwnedChildEdgeError::OrdinalOverflow)?,
                        &HirExpressionOwnedBodyRole::AwaitBranchBody { branch },
                    );
                }
            }
            Self::Choice(expression) => append_choice_owned_edges(expression, &mut edges)?,
            Self::DialogueContentApplication(expression) => {
                append_dialogue_owned_edges(expression, &mut edges)?;
            }
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
            | Self::Thread(_)
            | Self::Range(_)
            | Self::Record(_)
            | Self::RecordLiteral(_)
            | Self::Binary(_)
            | Self::Borrow(_)
            | Self::Dereference(_)
            | Self::Unary(_)
            | Self::Block(_)
            | Self::ComputationBlock(_)
            | Self::NamedBlock(_)
            | Self::Loop(_)
            | Self::If(_)
            | Self::Match(_)
            | Self::PostfixBracket(_)
            | Self::Error(_)
            | Self::ForSynthetic(_) => {}
        }
        Ok(edges)
    }
}

enum ChoiceOwnedWork<'choice> {
    Body(
        &'choice super::HirChoiceBody,
        Vec<HirNestedExpressionPathSegment>,
    ),
    Item(&'choice HirChoiceItem, Vec<HirNestedExpressionPathSegment>),
    MatchArm(
        &'choice super::HirChoiceMatchArm,
        Vec<HirNestedExpressionPathSegment>,
        u32,
    ),
    OptionBody(
        &'choice super::HirChoiceOptionBody,
        Vec<HirNestedExpressionPathSegment>,
    ),
    OptionField(
        &'choice HirChoiceOptionField,
        Vec<HirNestedExpressionPathSegment>,
        u32,
    ),
}

fn append_choice_owned_edges(
    expression: &super::HirChoiceExpr,
    edges: &mut Vec<HirExpressionOwnedChildEdge>,
) -> Result<(), HirExpressionOwnedChildEdgeError> {
    let mut work = vec![ChoiceOwnedWork::Body(expression.body(), Vec::new())];
    while let Some(next) = work.pop() {
        match next {
            ChoiceOwnedWork::Body(body, prefix) => {
                for (item, value) in body.items().iter().enumerate().rev() {
                    work.push(ChoiceOwnedWork::Item(
                        value,
                        extend_path(
                            &prefix,
                            HirNestedExpressionPathSegment::ChoiceBodyItem {
                                ordinal: owned_ordinal(item)?,
                            },
                        ),
                    ));
                }
            }
            ChoiceOwnedWork::Item(item, item_path) => {
                append_choice_item(item, item_path, edges, &mut work)?;
            }
            ChoiceOwnedWork::MatchArm(value, path, arm) => {
                push_owned_edge(
                    edges,
                    HirExpressionOwnedChild::Pattern(value.pattern()),
                    HirExpressionOwnedBodyRole::ChoiceMatchArmPattern {
                        path: owned_path(path.clone())?,
                        arm,
                    },
                );
                work.push(ChoiceOwnedWork::Body(value.body(), path));
            }
            ChoiceOwnedWork::OptionBody(body, prefix) => {
                for (field, value) in body.fields().iter().enumerate().rev() {
                    let field = owned_ordinal(field)?;
                    work.push(ChoiceOwnedWork::OptionField(
                        value,
                        extend_path(
                            &prefix,
                            HirNestedExpressionPathSegment::ChoiceOptionField { ordinal: field },
                        ),
                        field,
                    ));
                }
            }
            ChoiceOwnedWork::OptionField(value, path, field) => {
                append_choice_option_field(value, path, field, edges)?;
            }
        }
    }
    append_choice_plan_edges(expression, edges)
}

fn append_choice_item<'choice>(
    item: &'choice HirChoiceItem,
    item_path: Vec<HirNestedExpressionPathSegment>,
    edges: &mut Vec<HirExpressionOwnedChildEdge>,
    work: &mut Vec<ChoiceOwnedWork<'choice>>,
) -> Result<(), HirExpressionOwnedChildEdgeError> {
    match item {
        HirChoiceItem::Let(statement) => push_owned_edge(
            edges,
            HirExpressionOwnedChild::Statement(*statement),
            HirExpressionOwnedBodyRole::ChoiceLetStatement {
                path: owned_path(item_path)?,
            },
        ),
        HirChoiceItem::If(value) => {
            if let Some(body) = value.else_body() {
                work.push(ChoiceOwnedWork::Body(
                    body,
                    extend_path(&item_path, HirNestedExpressionPathSegment::ChoiceIfElse),
                ));
            }
            for (branch, value) in value.branches().iter().enumerate().rev() {
                work.push(ChoiceOwnedWork::Body(
                    value.body(),
                    extend_path(
                        &item_path,
                        HirNestedExpressionPathSegment::ChoiceIfBranch {
                            ordinal: owned_ordinal(branch)?,
                        },
                    ),
                ));
            }
        }
        HirChoiceItem::For(value) => {
            push_owned_edge(
                edges,
                HirExpressionOwnedChild::Pattern(value.pattern()),
                HirExpressionOwnedBodyRole::ChoiceForPattern {
                    path: owned_path(item_path.clone())?,
                },
            );
            work.push(ChoiceOwnedWork::Body(
                value.body(),
                extend_path(&item_path, HirNestedExpressionPathSegment::ChoiceForBody),
            ));
        }
        HirChoiceItem::Match(value) => {
            for (arm, value) in value.arms().iter().enumerate().rev() {
                let arm = owned_ordinal(arm)?;
                work.push(ChoiceOwnedWork::MatchArm(
                    value,
                    extend_path(
                        &item_path,
                        HirNestedExpressionPathSegment::ChoiceMatchArm { ordinal: arm },
                    ),
                    arm,
                ));
            }
        }
        HirChoiceItem::Option(value) => work.push(ChoiceOwnedWork::OptionBody(
            value.body(),
            extend_path(&item_path, HirNestedExpressionPathSegment::ChoiceOptionBody),
        )),
        HirChoiceItem::OptionFor(value) => {
            push_owned_edge(
                edges,
                HirExpressionOwnedChild::Pattern(value.pattern()),
                HirExpressionOwnedBodyRole::ChoiceOptionForPattern {
                    path: owned_path(item_path.clone())?,
                },
            );
            work.push(ChoiceOwnedWork::OptionBody(
                value.body(),
                extend_path(&item_path, HirNestedExpressionPathSegment::ChoiceOptionBody),
            ));
        }
        HirChoiceItem::CompactArm(_) | HirChoiceItem::Error => {}
    }
    Ok(())
}

fn append_choice_option_field(
    value: &HirChoiceOptionField,
    path: Vec<HirNestedExpressionPathSegment>,
    field: u32,
    edges: &mut Vec<HirExpressionOwnedChildEdge>,
) -> Result<(), HirExpressionOwnedChildEdgeError> {
    match value {
        HirChoiceOptionField::Select(body) => push_owned_body_edges(
            edges,
            body.try_child_edges()
                .map_err(|_| HirExpressionOwnedChildEdgeError::OrdinalOverflow)?,
            &HirExpressionOwnedBodyRole::ChoiceOptionSelectBody {
                path: owned_path(path)?,
                field,
            },
        ),
        HirChoiceOptionField::Let(statement) => push_owned_edge(
            edges,
            HirExpressionOwnedChild::Statement(*statement),
            HirExpressionOwnedBodyRole::ChoiceOptionLetStatement {
                path: owned_path(path)?,
                field,
            },
        ),
        HirChoiceOptionField::Label { .. }
        | HirChoiceOptionField::Id(_)
        | HirChoiceOptionField::Value(_)
        | HirChoiceOptionField::Visible(_)
        | HirChoiceOptionField::Enabled(_)
        | HirChoiceOptionField::Order(_)
        | HirChoiceOptionField::Hotkey(_)
        | HirChoiceOptionField::View(_)
        | HirChoiceOptionField::Error => {}
    }
    Ok(())
}

fn append_choice_plan_edges(
    expression: &super::HirChoiceExpr,
    edges: &mut Vec<HirExpressionOwnedChildEdge>,
) -> Result<(), HirExpressionOwnedChildEdgeError> {
    let Some(plan) = expression.plan() else {
        return Ok(());
    };
    for (item, value) in plan.items().iter().enumerate() {
        let path = owned_path(vec![HirNestedExpressionPathSegment::ChoicePlanItem {
            ordinal: owned_ordinal(item)?,
        }])?;
        match value {
            HirChoicePlanItem::Timeout { body, .. } => push_owned_body_edges(
                edges,
                body.try_child_edges()
                    .map_err(|_| HirExpressionOwnedChildEdgeError::OrdinalOverflow)?,
                &HirExpressionOwnedBodyRole::ChoicePlanTimeoutBody { path },
            ),
            HirChoicePlanItem::Cancel { trigger, body } => {
                let role = HirExpressionOwnedBodyRole::ChoicePlanCancelBody { path };
                if let Some(pattern) = trigger_pattern(trigger) {
                    push_owned_edge(
                        edges,
                        HirExpressionOwnedChild::Pattern(pattern),
                        HirExpressionOwnedBodyRole::ChoicePlanCancelTrigger {
                            path: match &role {
                                HirExpressionOwnedBodyRole::ChoicePlanCancelBody { path } => {
                                    path.clone()
                                }
                                _ => unreachable!("cancel role is body"),
                            },
                        },
                    );
                }
                push_owned_body_edges(
                    edges,
                    body.try_child_edges()
                        .map_err(|_| HirExpressionOwnedChildEdgeError::OrdinalOverflow)?,
                    &role,
                );
            }
            HirChoicePlanItem::OnSelect { pattern, body, .. } => {
                push_owned_edge(
                    edges,
                    HirExpressionOwnedChild::Pattern(*pattern),
                    HirExpressionOwnedBodyRole::ChoicePlanOnSelectPattern { path: path.clone() },
                );
                push_owned_body_edges(
                    edges,
                    body.try_child_edges()
                        .map_err(|_| HirExpressionOwnedChildEdgeError::OrdinalOverflow)?,
                    &HirExpressionOwnedBodyRole::ChoicePlanOnSelectBody { path },
                );
            }
            HirChoicePlanItem::Assignment { .. } | HirChoicePlanItem::Error(_) => {}
        }
    }
    Ok(())
}

fn append_dialogue_owned_edges(
    expression: &HirDialogueContentApplication,
    edges: &mut Vec<HirExpressionOwnedChildEdge>,
) -> Result<(), HirExpressionOwnedChildEdgeError> {
    if let Some(plan) = expression.plan() {
        append_line_plan_owned_edges(plan.items(), edges)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum LinePlanGroupKind {
    Start,
    Together,
}

enum LinePlanOwnedWork<'plan> {
    Body(
        &'plan [HirLinePlanItem],
        Vec<HirNestedExpressionPathSegment>,
        Option<LinePlanGroupKind>,
    ),
    Item(&'plan HirLinePlanItem, Vec<HirNestedExpressionPathSegment>),
}

fn append_line_plan_owned_edges(
    items: &[HirLinePlanItem],
    edges: &mut Vec<HirExpressionOwnedChildEdge>,
) -> Result<(), HirExpressionOwnedChildEdgeError> {
    let mut work = vec![LinePlanOwnedWork::Body(items, Vec::new(), None)];
    while let Some(next) = work.pop() {
        match next {
            LinePlanOwnedWork::Body(items, prefix, group) => {
                for (item, value) in items.iter().enumerate().rev() {
                    let ordinal = owned_ordinal(item)?;
                    let segment = match group {
                        None => HirNestedExpressionPathSegment::LinePlanItem { ordinal },
                        Some(LinePlanGroupKind::Start) => {
                            HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal }
                        }
                        Some(LinePlanGroupKind::Together) => {
                            HirNestedExpressionPathSegment::LinePlanTogetherGroupItem { ordinal }
                        }
                    };
                    work.push(LinePlanOwnedWork::Item(
                        value,
                        extend_path(&prefix, segment),
                    ));
                }
            }
            LinePlanOwnedWork::Item(item, item_path) => {
                append_line_plan_item(item, item_path, edges, &mut work)?;
            }
        }
    }
    Ok(())
}

fn append_line_plan_item<'plan>(
    item: &'plan HirLinePlanItem,
    item_path: Vec<HirNestedExpressionPathSegment>,
    edges: &mut Vec<HirExpressionOwnedChildEdge>,
    work: &mut Vec<LinePlanOwnedWork<'plan>>,
) -> Result<(), HirExpressionOwnedChildEdgeError> {
    let path = || owned_path(item_path.clone());
    match item {
        HirLinePlanItem::Init(statements) => {
            for (statement, owner) in statements.iter().enumerate() {
                push_owned_edge(
                    edges,
                    HirExpressionOwnedChild::Statement(*owner),
                    HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
                        path: path()?,
                        role: HirLinePlanStatementRole::Init {
                            statement: owned_ordinal(statement)?,
                        },
                    },
                );
            }
        }
        HirLinePlanItem::Thread(statement) => {
            push_line_plan_statement(edges, *statement, path()?, HirLinePlanStatementRole::Thread);
        }
        HirLinePlanItem::On(statement) => {
            push_line_plan_statement(edges, *statement, path()?, HirLinePlanStatementRole::On);
        }
        HirLinePlanItem::Let {
            pattern, statement, ..
        } => {
            let nested_path = path()?;
            push_owned_edge(
                edges,
                HirExpressionOwnedChild::Pattern(*pattern),
                HirExpressionOwnedBodyRole::DialogueLinePlanLet {
                    path: nested_path.clone(),
                },
            );
            push_line_plan_statement(
                edges,
                *statement,
                nested_path,
                HirLinePlanStatementRole::Statement,
            );
        }
        HirLinePlanItem::Statement(statement) => push_line_plan_statement(
            edges,
            *statement,
            path()?,
            HirLinePlanStatementRole::Statement,
        ),
        HirLinePlanItem::CancelRule(statement) => push_line_plan_statement(
            edges,
            *statement,
            path()?,
            HirLinePlanStatementRole::CancelRule,
        ),
        HirLinePlanItem::Error(statement) => {
            push_line_plan_statement(edges, *statement, path()?, HirLinePlanStatementRole::Error);
        }
        HirLinePlanItem::StartGroup(items) => work.push(LinePlanOwnedWork::Body(
            items,
            item_path,
            Some(LinePlanGroupKind::Start),
        )),
        HirLinePlanItem::TogetherGroup(items) => work.push(LinePlanOwnedWork::Body(
            items,
            item_path,
            Some(LinePlanGroupKind::Together),
        )),
        HirLinePlanItem::Out { statement, .. } => {
            push_line_plan_statement(
                edges,
                *statement,
                path()?,
                HirLinePlanStatementRole::Statement,
            );
        }
        HirLinePlanItem::Option { .. }
        | HirLinePlanItem::TimedCue { .. }
        | HirLinePlanItem::TimelineAssert { .. }
        | HirLinePlanItem::Expression(_) => {}
    }
    Ok(())
}

fn trigger_pattern(trigger: &HirTriggerPattern) -> Option<PatternId> {
    match trigger {
        HirTriggerPattern::Input(pattern)
        | HirTriggerPattern::Event(pattern)
        | HirTriggerPattern::Mark(pattern)
        | HirTriggerPattern::Select(pattern)
        | HirTriggerPattern::Task(pattern)
        | HirTriggerPattern::Scope(pattern) => Some(*pattern),
        HirTriggerPattern::Signal { value, .. } => *value,
        HirTriggerPattern::Timeout(_) | HirTriggerPattern::Expr(_) => None,
    }
}

fn push_line_plan_statement(
    edges: &mut Vec<HirExpressionOwnedChildEdge>,
    statement: StmtId,
    path: HirNestedExpressionPath,
    role: HirLinePlanStatementRole,
) {
    push_owned_edge(
        edges,
        HirExpressionOwnedChild::Statement(statement),
        HirExpressionOwnedBodyRole::DialogueLinePlanStatement { path, role },
    );
}

fn push_owned_body_edges(
    edges: &mut Vec<HirExpressionOwnedChildEdge>,
    body: impl IntoIterator<Item = HirBodyChildEdge>,
    role: &HirExpressionOwnedBodyRole,
) {
    edges.extend(body.into_iter().map(|edge| {
        HirExpressionOwnedChildEdge::new(HirExpressionOwnedChild::Body(edge), (*role).clone())
    }));
}

fn push_owned_edge(
    edges: &mut Vec<HirExpressionOwnedChildEdge>,
    child: HirExpressionOwnedChild,
    role: HirExpressionOwnedBodyRole,
) {
    edges.push(HirExpressionOwnedChildEdge::new(child, role));
}

fn owned_path(
    segments: Vec<HirNestedExpressionPathSegment>,
) -> Result<HirNestedExpressionPath, HirExpressionOwnedChildEdgeError> {
    HirNestedExpressionPath::try_from_segments(segments.into_boxed_slice())
        .map_err(|_| HirExpressionOwnedChildEdgeError::EmptyNestedPath)
}

fn owned_ordinal(value: usize) -> Result<u32, HirExpressionOwnedChildEdgeError> {
    u32::try_from(value).map_err(|_| HirExpressionOwnedChildEdgeError::OrdinalOverflow)
}

#[cfg(test)]
#[path = "owned_body_edges/tests.rs"]
mod tests;
