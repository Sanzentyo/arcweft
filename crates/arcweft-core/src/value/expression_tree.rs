//! Borrowed owned expression/pattern trees and their structural child roles.
//!
//! This projection retains the actual admitted nodes, including pattern
//! literals and callback captures. It neither follows referenced definition
//! tables nor creates a second executable representation or a semantic digest.

use super::{RuntimeExpr, RuntimeExprKind, RuntimeStandardMapOperandOrder};
use crate::pattern::{RuntimePattern, RuntimePatternKind};

/// The structural edge from an admitted node to one of its owned children.
/// List ordinals retain source order and are not standalone runtime identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeExpressionChildRole {
    Root,
    AgentOperand { ordinal: usize },
    Initializer,
    Body,
    Item { ordinal: usize },
    ContentValue { ordinal: usize },
    ContentCapture { effect: usize, capture: usize },
    FormatOperand { ordinal: usize },
    Target,
    DialogueField { ordinal: usize },
    RepeatedValue,
    RangeStart,
    RangeEnd,
    RecordInitializer { ordinal: usize },
    VariantPayload,
    AssignmentValue,
    Argument { ordinal: usize },
    CallableCapture { ordinal: usize },
    CallableValue,
    Callee,
    Receiver,
    Mapping,
    Source,
    UnaryOperand,
    Left,
    Right,
    Condition,
    Pattern,
    Guard,
    Then,
    Else,
    Scrutinee,
    MatchPattern { arm: usize },
    MatchGuard { arm: usize },
    MatchValue { arm: usize },
    ReductionState,
    PatternItem { ordinal: usize },
    PatternField { ordinal: usize },
    WholePattern,
}
impl RuntimeExpressionChildRole {
    #[expect(
        clippy::too_many_lines,
        reason = "the closed child-role algebra owns each exact semantic tag and source coordinate"
    )]
    pub(crate) fn encode_semantic_path(
        self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        match self {
            Self::Root => {
                encoder.tag(0);
            }
            Self::AgentOperand { ordinal } => {
                encoder.tag(1);
                encoder.count(ordinal);
            }
            Self::Initializer => {
                encoder.tag(2);
            }
            Self::Body => {
                encoder.tag(3);
            }
            Self::Item { ordinal } => {
                encoder.tag(4);
                encoder.count(ordinal);
            }
            Self::ContentValue { ordinal } => {
                encoder.tag(5);
                encoder.count(ordinal);
            }
            Self::ContentCapture { effect, capture } => {
                encoder.tag(6);
                encoder.count(effect);
                encoder.count(capture);
            }
            Self::FormatOperand { ordinal } => {
                encoder.tag(7);
                encoder.count(ordinal);
            }
            Self::Target => {
                encoder.tag(8);
            }
            Self::DialogueField { ordinal } => {
                encoder.tag(9);
                encoder.count(ordinal);
            }
            Self::RepeatedValue => {
                encoder.tag(10);
            }
            Self::RangeStart => {
                encoder.tag(11);
            }
            Self::RangeEnd => {
                encoder.tag(12);
            }
            Self::RecordInitializer { ordinal } => {
                encoder.tag(13);
                encoder.count(ordinal);
            }
            Self::VariantPayload => {
                encoder.tag(14);
            }
            Self::AssignmentValue => {
                encoder.tag(15);
            }
            Self::Argument { ordinal } => {
                encoder.tag(16);
                encoder.count(ordinal);
            }
            Self::CallableCapture { ordinal } => {
                encoder.tag(17);
                encoder.count(ordinal);
            }
            Self::CallableValue => {
                encoder.tag(18);
            }
            Self::Callee => {
                encoder.tag(19);
            }
            Self::Receiver => {
                encoder.tag(20);
            }
            Self::Mapping => {
                encoder.tag(21);
            }
            Self::Source => {
                encoder.tag(22);
            }
            Self::UnaryOperand => {
                encoder.tag(23);
            }
            Self::Left => {
                encoder.tag(24);
            }
            Self::Right => {
                encoder.tag(25);
            }
            Self::Condition => {
                encoder.tag(26);
            }
            Self::Pattern => {
                encoder.tag(27);
            }
            Self::Guard => {
                encoder.tag(28);
            }
            Self::Then => {
                encoder.tag(29);
            }
            Self::Else => {
                encoder.tag(30);
            }
            Self::Scrutinee => {
                encoder.tag(31);
            }
            Self::MatchPattern { arm } => {
                encoder.tag(32);
                encoder.count(arm);
            }
            Self::MatchGuard { arm } => {
                encoder.tag(33);
                encoder.count(arm);
            }
            Self::MatchValue { arm } => {
                encoder.tag(34);
                encoder.count(arm);
            }
            Self::ReductionState => {
                encoder.tag(35);
            }
            Self::PatternItem { ordinal } => {
                encoder.tag(36);
                encoder.count(ordinal);
            }
            Self::PatternField { ordinal } => {
                encoder.tag(37);
                encoder.count(ordinal);
            }
            Self::WholePattern => {
                encoder.tag(38);
            }
        }
    }
}

/// An actual borrowed node of a lowered expression's owned tree.
#[derive(Clone, Copy, Debug)]
pub enum RuntimeExpressionNode<'a> {
    Expression(&'a RuntimeExpr),
    Pattern(&'a RuntimePattern),
}

/// Direct children of an admitted node, retaining structural roles.
pub struct RuntimeExpressionChildren<'a> {
    node: RuntimeExpressionNode<'a>,
    ordinal: usize,
    inner: usize,
    phase: u8,
}

impl<'a> RuntimeExpressionNode<'a> {
    #[must_use]
    pub const fn owned_children(self) -> RuntimeExpressionChildren<'a> {
        RuntimeExpressionChildren {
            node: self,
            ordinal: 0,
            inner: 0,
            phase: 0,
        }
    }
}

impl RuntimeExpr {
    /// Direct owned expression and pattern children in structural order.
    #[must_use]
    pub const fn owned_children(&self) -> RuntimeExpressionChildren<'_> {
        RuntimeExpressionNode::Expression(self).owned_children()
    }

    /// Visits every owned node once in preorder without native recursion.
    /// Referenced function/trait/template definitions remain table edges.
    /// Rejection immediately stops before later nodes.
    pub fn try_visit_owned_tree<E>(
        &self,
        visitor: &mut impl FnMut(RuntimeExpressionChildRole, RuntimeExpressionNode<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        RuntimeExpressionNode::Expression(self).try_visit_owned_tree(visitor)
    }
}

impl RuntimePattern {
    /// Direct owned pattern children. Rest bindings are metadata on this
    /// pattern, not an invented recursive node.
    #[must_use]
    pub const fn owned_children(&self) -> RuntimeExpressionChildren<'_> {
        RuntimeExpressionNode::Pattern(self).owned_children()
    }
}

/// Balanced boundaries of an actual borrowed expression/pattern tree.
/// Child roles occur on entry; exit closes the same node without cloning it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeExpressionTreeEvent<'a> {
    Enter {
        role: RuntimeExpressionChildRole,
        node: RuntimeExpressionNode<'a>,
    },
    Exit {
        node: RuntimeExpressionNode<'a>,
    },
}

impl RuntimeExpressionNode<'_> {
    pub(crate) fn try_visit_owned_tree<E>(
        self,
        visitor: &mut impl FnMut(RuntimeExpressionChildRole, RuntimeExpressionNode<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.try_visit_owned_events(&mut |event| match event {
            RuntimeExpressionTreeEvent::Enter { role, node } => visitor(role, node),
            RuntimeExpressionTreeEvent::Exit { .. } => Ok(()),
        })
    }

    /// Visits balanced owned-tree boundaries with an explicit depth stack.
    /// Rejection stops before both later children and pending exit events.
    pub(crate) fn try_visit_owned_events<E>(
        self,
        visitor: &mut impl FnMut(RuntimeExpressionTreeEvent<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        use RuntimeExpressionTreeEvent as Event;
        visitor(Event::Enter {
            role: RuntimeExpressionChildRole::Root,
            node: self,
        })?;
        let mut stack = vec![(self, self.owned_children())];
        while let Some((node, children)) = stack.last_mut() {
            if let Some((role, child)) = children.next() {
                visitor(Event::Enter { role, node: child })?;
                stack.push((child, child.owned_children()));
            } else {
                visitor(Event::Exit { node: *node })?;
                stack.pop();
            }
        }
        Ok(())
    }
}

impl<'a> Iterator for RuntimeExpressionChildren<'a> {
    type Item = (RuntimeExpressionChildRole, RuntimeExpressionNode<'a>);

    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive borrowed projection owns the complete expression/pattern child algebra"
    )]
    fn next(&mut self) -> Option<Self::Item> {
        use RuntimeExpressionChildRole as R;
        use RuntimeExpressionNode as N;
        let index = self.ordinal;
        let expr = |role, node| Some((role, N::Expression(node)));
        let pattern = |role, node| Some((role, N::Pattern(node)));
        let next = match self.node {
            N::Expression(node) => match node.kind() {
                RuntimeExprKind::Value(_)
                | RuntimeExprKind::Local(_)
                | RuntimeExprKind::SequencePopFront { .. }
                | RuntimeExprKind::SequencePopBack { .. }
                | RuntimeExprKind::EntityRef(_) => None,
                RuntimeExprKind::Agent(agent) => agent
                    .operand(index)
                    .and_then(|node| expr(R::AgentOperand { ordinal: index }, node)),
                RuntimeExprKind::SequencePush { value, .. } => {
                    (index == 0).then_some((R::Item { ordinal: 0 }, N::Expression(value)))
                }
                RuntimeExprKind::Let {
                    expr: value, body, ..
                } => match index {
                    0 => expr(R::Initializer, value),
                    1 => expr(R::Body, body),
                    _ => None,
                },
                RuntimeExprKind::Scope { body, .. } => {
                    (index == 0).then_some((R::Body, N::Expression(body)))
                }
                RuntimeExprKind::Tuple(items) | RuntimeExprKind::BracketSeq(items) => items
                    .get(index)
                    .and_then(|node| expr(R::Item { ordinal: index }, node)),
                RuntimeExprKind::DialogueContent {
                    values, effects, ..
                } => {
                    if self.phase == 0 {
                        if let Some(value) = values.get(index) {
                            self.ordinal += 1;
                            return expr(R::ContentValue { ordinal: index }, value);
                        }
                        self.phase = 1;
                        self.ordinal = 0;
                    }
                    while let Some(effect) = effects.get(self.ordinal) {
                        if let Some(capture) = effect.captures.get(self.inner) {
                            let role = R::ContentCapture {
                                effect: self.ordinal,
                                capture: self.inner,
                            };
                            self.inner += 1;
                            return expr(role, capture);
                        }
                        self.ordinal += 1;
                        self.inner = 0;
                    }
                    None
                }
                RuntimeExprKind::FormatContent { operands, .. } => operands
                    .get(index)
                    .and_then(|node| expr(R::FormatOperand { ordinal: index }, node.expression())),
                RuntimeExprKind::CharacterDialogue { target, fields, .. } => {
                    if self.phase == 0 {
                        self.phase = 1;
                        return expr(R::Target, target);
                    }
                    while let Some(field) = fields.get(self.ordinal) {
                        let ordinal = self.ordinal;
                        self.ordinal += 1;
                        match &field.operation {
                            arcweft_interaction_model::dialogue::CharacterDialoguePatchOperation::Set(value) => {
                                return expr(R::DialogueField { ordinal }, value);
                            }
                            arcweft_interaction_model::dialogue::CharacterDialoguePatchOperation::Clear => {}
                        }
                    }
                    None
                }
                RuntimeExprKind::RepeatSeq { value, .. } => {
                    (index == 0).then_some((R::RepeatedValue, N::Expression(value)))
                }
                RuntimeExprKind::Range { start, end, .. } => {
                    while self.ordinal < 2 {
                        let next = match self.ordinal {
                            0 => start
                                .as_deref()
                                .and_then(|value| expr(R::RangeStart, value)),
                            _ => end.as_deref().and_then(|value| expr(R::RangeEnd, value)),
                        };
                        self.ordinal += 1;
                        if next.is_some() {
                            return next;
                        }
                    }
                    None
                }
                RuntimeExprKind::NominalRecord(record) => record
                    .initializers()
                    .get(index)
                    .and_then(|node| expr(R::RecordInitializer { ordinal: index }, node.value())),
                RuntimeExprKind::Variant { payload, .. } => {
                    if index == 0 {
                        payload
                            .as_deref()
                            .and_then(|node| expr(R::VariantPayload, node))
                    } else {
                        None
                    }
                }
                RuntimeExprKind::Field { target, .. } => (index == 0)
                    .then(|| target.value())
                    .flatten()
                    .map(|target| (R::Target, N::Expression(target))),
                RuntimeExprKind::ProjectTuple { target, .. }
                | RuntimeExprKind::ProjectRecord { target, .. } => {
                    (index == 0).then_some((R::Target, N::Expression(target)))
                }
                RuntimeExprKind::Assign {
                    expr: value, body, ..
                } => match index {
                    0 => expr(R::AssignmentValue, value),
                    1 => expr(R::Body, body),
                    _ => None,
                },
                RuntimeExprKind::Call { args, .. } | RuntimeExprKind::PureCall { args, .. } => args
                    .get(index)
                    .and_then(|arg| expr(R::Argument { ordinal: index }, arg.value())),
                RuntimeExprKind::MakeCallable { captures, .. } => captures
                    .get(index)
                    .and_then(|node| expr(R::CallableCapture { ordinal: index }, node)),
                RuntimeExprKind::SpecializeCallable { value, .. } => {
                    (index == 0).then_some((R::CallableValue, N::Expression(value)))
                }
                RuntimeExprKind::ApplyGroup { callee, args } => {
                    if index == 0 {
                        expr(R::Callee, callee)
                    } else {
                        args.get(index - 1)
                            .and_then(|arg| expr(R::Argument { ordinal: index - 1 }, arg.value()))
                    }
                }
                RuntimeExprKind::TraitCall { receiver, args, .. } => {
                    if index == 0 {
                        expr(R::Receiver, receiver)
                    } else {
                        args.get(index - 1)
                            .and_then(|arg| expr(R::Argument { ordinal: index - 1 }, arg.value()))
                    }
                }
                RuntimeExprKind::StandardMap {
                    order,
                    mapping,
                    source,
                    ..
                } => match (order, index) {
                    (RuntimeStandardMapOperandOrder::MappingThenReceiver, 0)
                    | (RuntimeStandardMapOperandOrder::ReceiverThenMapping, 1) => {
                        expr(R::Mapping, mapping)
                    }
                    (RuntimeStandardMapOperandOrder::MappingThenReceiver, 1)
                    | (RuntimeStandardMapOperandOrder::ReceiverThenMapping, 0) => {
                        expr(R::Source, source)
                    }
                    (_, _) => None,
                },
                RuntimeExprKind::Sum { source } => {
                    (index == 0).then_some((R::Source, N::Expression(source)))
                }
                RuntimeExprKind::Unary { expr: value, .. } => {
                    (index == 0).then_some((R::UnaryOperand, N::Expression(value)))
                }
                RuntimeExprKind::Binary { lhs, rhs, .. } => match index {
                    0 => expr(R::Left, lhs),
                    1 => expr(R::Right, rhs),
                    _ => None,
                },
                RuntimeExprKind::If {
                    condition,
                    then_expr,
                    else_expr,
                } => match index {
                    0 => expr(R::Condition, condition),
                    1 => expr(R::Then, then_expr),
                    2 => expr(R::Else, else_expr),
                    _ => None,
                },
                RuntimeExprKind::IfLet {
                    pattern: binding,
                    expr: value,
                    guard,
                    then_expr,
                    else_expr,
                } => {
                    while self.ordinal < 5 {
                        let child = match self.ordinal {
                            0 => pattern(R::Pattern, binding),
                            1 => expr(R::Initializer, value),
                            2 => guard.as_deref().and_then(|node| expr(R::Guard, node)),
                            3 => expr(R::Then, then_expr),
                            _ => expr(R::Else, else_expr),
                        };
                        self.ordinal += 1;
                        if child.is_some() {
                            return child;
                        }
                    }
                    None
                }
                RuntimeExprKind::Match { scrutinee, arms } => {
                    if self.phase == 0 {
                        self.phase = 1;
                        return expr(R::Scrutinee, scrutinee);
                    }
                    while let Some(arm) = arms.get(self.ordinal) {
                        let child = match self.phase {
                            1 => pattern(R::MatchPattern { arm: self.ordinal }, arm.pattern()),
                            2 => arm
                                .guard()
                                .and_then(|node| expr(R::MatchGuard { arm: self.ordinal }, node)),
                            _ => expr(R::MatchValue { arm: self.ordinal }, arm.value()),
                        };
                        if self.phase == 3 {
                            self.ordinal += 1;
                            self.phase = 1;
                        } else {
                            self.phase += 1;
                        }
                        if child.is_some() {
                            return child;
                        }
                    }
                    None
                }
                RuntimeExprKind::ReductionUnchanged { state } => {
                    (index == 0).then_some((R::ReductionState, N::Expression(state)))
                }
            },
            N::Pattern(node) => match node.kind() {
                RuntimePatternKind::Bind { .. }
                | RuntimePatternKind::Discard
                | RuntimePatternKind::Literal(_)
                | RuntimePatternKind::Entity(_)
                | RuntimePatternKind::Typed { .. } => None,
                RuntimePatternKind::Tuple(items)
                | RuntimePatternKind::Or(items)
                | RuntimePatternKind::Sequence { items, .. } => items
                    .get(index)
                    .and_then(|node| pattern(R::PatternItem { ordinal: index }, node)),
                RuntimePatternKind::Record { fields, .. } => fields
                    .get(index)
                    .and_then(|field| pattern(R::PatternField { ordinal: index }, field.pattern())),
                RuntimePatternKind::Variant { payload, .. } => {
                    if index == 0 {
                        payload
                            .as_deref()
                            .and_then(|node| pattern(R::VariantPayload, node))
                    } else {
                        None
                    }
                }
                RuntimePatternKind::Whole { pattern: inner, .. } => {
                    (index == 0).then_some((R::WholePattern, N::Pattern(inner)))
                }
            },
        };
        if next.is_some() {
            self.ordinal += 1;
        }
        next
    }
}

#[cfg(test)]
mod tests;
