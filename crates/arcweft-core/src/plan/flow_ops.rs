//! Owned executable operation traversal, independent of call-graph edges.

use super::code::RuntimeCodeBodyRef;
use super::{FlowOp, RuntimeAwaitPendingObserver, RuntimeMatchArm, RuntimePlan};

mod location;
pub(crate) use location::{
    RuntimeFlowBodyLocation, RuntimeFlowLocationStep, RuntimeFlowOperationLocation,
};

/// Structural role of one directly owned operation body.
///
/// Ordinals are positions in the owning observer/arm list, including arms
/// without a guard. These are traversal coordinates, not task identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeFlowBodyRole {
    Body,
    Then,
    Else,
    AwaitObserver { ordinal: usize },
    MatchGuard { arm: usize },
    MatchArm { arm: usize },
}
impl RuntimeFlowBodyRole {
    pub(crate) const fn tag_and_ordinal(self) -> (u8, Option<usize>) {
        match self {
            Self::Body => (0, None),
            Self::Then => (1, None),
            Self::Else => (2, None),
            Self::AwaitObserver { ordinal } => (3, Some(ordinal)),
            Self::MatchGuard { arm } => (4, Some(arm)),
            Self::MatchArm { arm } => (5, Some(arm)),
        }
    }

    pub(crate) fn encode_semantic_path(
        self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        let (tag, ordinal) = self.tag_and_ordinal();
        encoder.tag(tag);
        if let Some(ordinal) = ordinal {
            encoder.count(ordinal);
        }
    }
}

/// Borrowed direct children of an operation, in structural source order.
pub struct RuntimeFlowOwnedBodies<'a> {
    op: &'a FlowOp,
    ordinal: usize,
    guard: bool,
}

/// One borrowed schema supplies traversal and indexed lookup of owned bodies.
enum RuntimeFlowBodySchema<'a> {
    Empty,
    One(RuntimeFlowBodyRole, &'a [FlowOp]),
    Branches(&'a [FlowOp], &'a [FlowOp]),
    Match(&'a [RuntimeMatchArm]),
    Await(&'a [RuntimeAwaitPendingObserver]),
}

impl<'a> RuntimeFlowBodySchema<'a> {
    fn get(self, role: RuntimeFlowBodyRole) -> Option<&'a [FlowOp]> {
        use RuntimeFlowBodyRole as Role;
        match (self, role) {
            (Self::One(expected, body), actual) if expected == actual => Some(body),
            (Self::Branches(then_ops, _), Role::Then) => Some(then_ops),
            (Self::Branches(_, else_ops), Role::Else) => Some(else_ops),
            (Self::Match(arms), Role::MatchGuard { arm }) => {
                Some(&arms.get(arm)?.guard.as_ref()?.ops)
            }
            (Self::Match(arms), Role::MatchArm { arm }) => Some(&arms.get(arm)?.ops),
            (Self::Await(observers), Role::AwaitObserver { ordinal }) => {
                Some(&observers.get(ordinal)?.ops)
            }
            _ => None,
        }
    }
}

/// Role of a directly owned expression or pattern in one operation.
/// Ordinals retain source positions; referenced catalog definitions are edges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeFlowValueRole {
    Pattern,
    Value,
    Target,
    Result,
    Source,
    Condition,
    Guard,
    Callee,
    Scrutinee,
    Argument {
        ordinal: usize,
    },
    AwaitObserverPattern {
        ordinal: usize,
    },
    MatchPattern {
        arm: usize,
    },
    MatchGuard {
        arm: usize,
    },
    BaseRequestArgument {
        ordinal: usize,
    },
    ChildRequestArgument {
        ordinal: usize,
    },
    Capture {
        ordinal: usize,
    },
    LineArgument {
        ordinal: usize,
    },
    EffectArgument {
        ordinal: usize,
    },
    AudioArgument {
        ordinal: usize,
    },
    ChoiceAudioArgument {
        option: usize,
        effect: usize,
        argument: usize,
    },
}

impl RuntimeFlowValueRole {
    pub(crate) fn encode_semantic_path(
        self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        match self {
            Self::Pattern => encoder.tag(0),
            Self::Value => encoder.tag(1),
            Self::Target => encoder.tag(2),
            Self::Result => encoder.tag(3),
            Self::Source => encoder.tag(4),
            Self::Condition => encoder.tag(5),
            Self::Guard => encoder.tag(6),
            Self::Callee => encoder.tag(7),
            Self::Scrutinee => encoder.tag(8),
            Self::Argument { ordinal } => {
                encoder.tag(9);
                encoder.count(ordinal);
            }
            Self::AwaitObserverPattern { ordinal } => {
                encoder.tag(10);
                encoder.count(ordinal);
            }
            Self::MatchPattern { arm } => {
                encoder.tag(11);
                encoder.count(arm);
            }
            Self::MatchGuard { arm } => {
                encoder.tag(12);
                encoder.count(arm);
            }
            Self::BaseRequestArgument { ordinal } => {
                encoder.tag(13);
                encoder.count(ordinal);
            }
            Self::ChildRequestArgument { ordinal } => {
                encoder.tag(14);
                encoder.count(ordinal);
            }
            Self::Capture { ordinal } => {
                encoder.tag(15);
                encoder.count(ordinal);
            }
            Self::LineArgument { ordinal } => {
                encoder.tag(16);
                encoder.count(ordinal);
            }
            Self::EffectArgument { ordinal } => {
                encoder.tag(17);
                encoder.count(ordinal);
            }
            Self::AudioArgument { ordinal } => {
                encoder.tag(18);
                encoder.count(ordinal);
            }
            Self::ChoiceAudioArgument {
                option,
                effect,
                argument,
            } => {
                encoder.tag(19);
                encoder.count(option);
                encoder.count(effect);
                encoder.count(argument);
            }
        }
    }
}

impl FlowOp {
    pub(crate) const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Let { .. } => 1,
            Self::FormatOperandAttempt { .. } => 2,
            Self::LetElse { .. } => 4,
            Self::Assign { .. } => 5,
            Self::LineOperation { .. } => 6,
            Self::CommitDialogueResult { .. } => 7,
            Self::SelectDialogueResult { .. } => 8,
            Self::Dialogue { .. } => 9,
            Self::Choice { .. } => 10,
            Self::Await { .. } => 11,
            Self::StartNeedProducer { .. } => 12,
            Self::AwaitMany { .. } => 13,
            Self::HostCall { .. } => 14,
            Self::ProjectCall { .. } => 15,
            Self::ApplyGroup { .. } => 16,
            Self::If { .. } => 17,
            Self::IfLet { .. } => 18,
            Self::Match { .. } => 19,
            Self::Loop { .. } => 20,
            Self::While { .. } => 22,
            Self::WhileLet { .. } => 24,
            Self::For { .. } => 26,
            Self::ProgramTask { .. } => 28,
            Self::Scope { .. } => 29,
            Self::Break(_) => 31,
            Self::Continue => 32,
            Self::Goto(_) => 33,
            Self::GotoExpr(_) => 34,
            Self::Return(_) => 35,
            Self::ReturnExpr(_) => 36,
            Self::Effect(_) => 37,
            Self::EvaluatedEffect(_) => 38,
            Self::RegisterDefer { .. } => 39,
            Self::RegisterCleanup { .. } => 40,
            Self::CancelCleanup { .. } => 41,
            Self::EnterScope { .. } => 42,
            Self::ExitScope => 43,
            Self::ExitScopeBind { .. } => 45,
            Self::Noop => 46,
            Self::StandardMap(_) => 49,
        }
    }
}

impl FlowOp {
    /// Borrows directly owned value/pattern roots in source order. Referenced
    /// project-call/default/content definitions remain catalog edges.
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive borrowed projection owns the complete Flow expression/pattern root algebra"
    )]
    pub fn try_visit_value_roots<E>(
        &self,
        visitor: &mut impl FnMut(
            RuntimeFlowValueRole,
            crate::value::RuntimeExpressionNode<'_>,
        ) -> Result<(), E>,
    ) -> Result<(), E> {
        use crate::value::RuntimeExpressionNode as Node;
        use RuntimeFlowValueRole as Role;
        let expression = |role, value, visitor: &mut dyn FnMut(Role, Node<'_>) -> Result<(), E>| {
            visitor(role, Node::Expression(value))
        };
        match self {
            FlowOp::Let { pattern, expr }
            | FlowOp::LetElse { pattern, expr, .. }
            | FlowOp::WhileLet { pattern, expr, .. }
            | FlowOp::ExitScopeBind { pattern, expr } => {
                visitor(Role::Pattern, Node::Pattern(pattern))?;
                expression(Role::Value, expr, visitor)?;
                if let FlowOp::WhileLet {
                    guard: Some(guard), ..
                } = self
                {
                    expression(Role::Guard, guard, visitor)?;
                }
            }
            FlowOp::FormatOperandAttempt { value, .. }
            | FlowOp::Assign { value, .. }
            | FlowOp::CommitDialogueResult { value }
            | FlowOp::SelectDialogueResult { value }
            | FlowOp::GotoExpr(value)
            | FlowOp::ReturnExpr(value)
            | FlowOp::Break(Some(value)) => expression(Role::Value, value, visitor)?,
            FlowOp::Dialogue { target, result, .. } => {
                expression(Role::Target, target, visitor)?;
                visitor(Role::Result, Node::Pattern(result.pattern()))?;
            }
            FlowOp::Await {
                binding,
                target,
                observers,
            } => {
                if let Some(binding) = binding {
                    visitor(Role::Pattern, Node::Pattern(binding))?;
                }
                expression(Role::Source, target.source(), visitor)?;
                for (ordinal, observer) in observers.iter().enumerate() {
                    visitor(
                        Role::AwaitObserverPattern { ordinal },
                        Node::Pattern(&observer.pattern),
                    )?;
                }
            }
            FlowOp::StartNeedProducer { binding, target } => {
                visitor(Role::Pattern, Node::Pattern(binding))?;
                for (ordinal, argument) in target.arguments().iter().enumerate() {
                    expression(Role::Argument { ordinal }, argument.value(), visitor)?;
                }
            }
            FlowOp::AwaitMany {
                binding, target, ..
            } => {
                if let Some(binding) = binding {
                    visitor(Role::Pattern, Node::Pattern(binding))?;
                }
                expression(Role::Source, &target.source, visitor)?;
                for (ordinal, argument) in target.base.request.args.iter().enumerate() {
                    expression(
                        Role::BaseRequestArgument { ordinal },
                        argument.value(),
                        visitor,
                    )?;
                }
                for (ordinal, argument) in target.child.request.args.iter().enumerate() {
                    expression(
                        Role::ChildRequestArgument { ordinal },
                        argument.value(),
                        visitor,
                    )?;
                }
            }
            FlowOp::HostCall { binding, target } => {
                if let Some(binding) = binding {
                    visitor(Role::Pattern, Node::Pattern(binding))?;
                }
                for (ordinal, argument) in target.args.iter().enumerate() {
                    expression(Role::Argument { ordinal }, argument.value(), visitor)?;
                }
            }
            FlowOp::ApplyGroup {
                callee,
                args,
                result,
            } => {
                expression(Role::Callee, callee, visitor)?;
                for (ordinal, argument) in args.iter().enumerate() {
                    expression(Role::Argument { ordinal }, argument.value(), visitor)?;
                }
                visitor(Role::Result, Node::Pattern(result))?;
            }
            FlowOp::If { condition, .. } | FlowOp::While { condition, .. } => {
                expression(Role::Condition, condition, visitor)?;
            }
            FlowOp::IfLet {
                pattern,
                expr,
                guard,
                ..
            } => {
                visitor(Role::Pattern, Node::Pattern(pattern))?;
                expression(Role::Value, expr, visitor)?;
                if let Some(guard) = guard {
                    expression(Role::Guard, guard, visitor)?;
                }
            }
            FlowOp::Match { scrutinee, arms } => {
                expression(Role::Scrutinee, scrutinee, visitor)?;
                for (arm_index, arm) in arms.iter().enumerate() {
                    visitor(
                        Role::MatchPattern { arm: arm_index },
                        Node::Pattern(&arm.pattern),
                    )?;
                    if let Some(condition) = arm
                        .guard
                        .as_ref()
                        .and_then(|guard| guard.condition.as_ref())
                    {
                        expression(Role::MatchGuard { arm: arm_index }, condition, visitor)?;
                    }
                }
            }
            FlowOp::Loop { result, .. } => {
                if let Some(result) = result {
                    visitor(Role::Result, Node::Pattern(result))?;
                }
            }
            FlowOp::For {
                pattern, source, ..
            } => {
                visitor(Role::Pattern, Node::Pattern(pattern))?;
                expression(Role::Source, source, visitor)?;
            }
            FlowOp::ProgramTask { binding, .. } => {
                visitor(Role::Result, Node::Pattern(binding))?;
            }
            FlowOp::RegisterDefer { captures, .. } => {
                for (ordinal, capture) in captures.iter().enumerate() {
                    expression(Role::Capture { ordinal }, capture, visitor)?;
                }
            }
            FlowOp::LineOperation { binding, operation } => {
                if let Some(binding) = binding {
                    visitor(Role::Pattern, Node::Pattern(binding))?;
                }
                for (ordinal, argument) in operation.argument_exprs().into_iter().enumerate() {
                    expression(Role::LineArgument { ordinal }, argument, visitor)?;
                }
            }
            FlowOp::EvaluatedEffect(effect) => {
                for (ordinal, argument) in effect.argument_exprs().into_iter().enumerate() {
                    expression(Role::EffectArgument { ordinal }, argument, visitor)?;
                }
            }
            FlowOp::Effect(effect) | FlowOp::RegisterCleanup { effect, .. } => {
                if let crate::effect::LineEffectRequest::Audio(command) = effect {
                    for (ordinal, argument) in command.argument_exprs().into_iter().enumerate() {
                        expression(Role::AudioArgument { ordinal }, argument, visitor)?;
                    }
                }
            }
            FlowOp::Choice { options, .. } => {
                for (option, choice) in options.iter().enumerate() {
                    for (effect, request) in choice.effects.iter().enumerate() {
                        if let crate::effect::LineEffectRequest::Audio(command) = request {
                            for (argument, value) in
                                command.argument_exprs().into_iter().enumerate()
                            {
                                expression(
                                    Role::ChoiceAudioArgument {
                                        option,
                                        effect,
                                        argument,
                                    },
                                    value,
                                    visitor,
                                )?;
                            }
                        }
                    }
                }
            }
            FlowOp::StandardMap(_)
            | FlowOp::ProjectCall { .. }
            | FlowOp::Scope { .. }
            | FlowOp::Break(None)
            | FlowOp::Continue
            | FlowOp::Goto(_)
            | FlowOp::Return(_)
            | FlowOp::CancelCleanup { .. }
            | FlowOp::EnterScope { .. }
            | FlowOp::ExitScope
            | FlowOp::Noop => {}
        }
        Ok(())
    }

    /// Authored operations can enter an inert image only when all owned
    /// expression and pattern literals permit Copy. Runtime continuation
    /// values use their dedicated snapshot owners instead.
    pub(crate) fn literals_permit_copy(&self) -> bool {
        try_visit_ops(std::slice::from_ref(self), &mut |op| {
            op.try_visit_value_roots(&mut |_, node| {
                node.literals_permit_copy().then_some(()).ok_or(())
            })
        })
        .is_ok()
    }

    /// Enumerates all directly owned bodies, retaining empty bodies and their
    /// roles. Referenced function sites are not owned children.
    #[must_use]
    pub const fn owned_bodies(&self) -> RuntimeFlowOwnedBodies<'_> {
        RuntimeFlowOwnedBodies {
            op: self,
            ordinal: 0,
            guard: true,
        }
    }
}

impl FlowOp {
    fn body_schema(&self) -> RuntimeFlowBodySchema<'_> {
        use RuntimeFlowBodyRole as Role;
        use RuntimeFlowBodySchema as Schema;
        match self {
            Self::StandardMap(control) => Schema::One(Role::Body, control.body()),
            Self::Await { observers, .. } => Schema::Await(observers),
            Self::LetElse { else_ops, .. } => Schema::One(Role::Else, else_ops),
            Self::If {
                then_ops, else_ops, ..
            }
            | Self::IfLet {
                then_ops, else_ops, ..
            } => Schema::Branches(then_ops, else_ops),
            Self::Match { arms, .. } => Schema::Match(arms),
            Self::FormatOperandAttempt { body, .. }
            | Self::Loop { body, .. }
            | Self::While { body, .. }
            | Self::WhileLet { body, .. }
            | Self::For { body, .. }
            | Self::ProgramTask { body, .. }
            | Self::Scope { body, .. } => Schema::One(Role::Body, body),
            FlowOp::Let { .. }
            | FlowOp::Assign { .. }
            | FlowOp::LineOperation { .. }
            | FlowOp::CommitDialogueResult { .. }
            | FlowOp::SelectDialogueResult { .. }
            | FlowOp::Dialogue { .. }
            | FlowOp::Choice { .. }
            | FlowOp::AwaitMany { .. }
            | FlowOp::StartNeedProducer { .. }
            | FlowOp::HostCall { .. }
            | FlowOp::ProjectCall { .. }
            | FlowOp::ApplyGroup { .. }
            | FlowOp::Break(_)
            | FlowOp::Continue
            | FlowOp::Goto(_)
            | FlowOp::GotoExpr(_)
            | FlowOp::Return(_)
            | FlowOp::ReturnExpr(_)
            | FlowOp::Effect(_)
            | FlowOp::EvaluatedEffect(_)
            | FlowOp::RegisterDefer { .. }
            | FlowOp::RegisterCleanup { .. }
            | FlowOp::CancelCleanup { .. }
            | FlowOp::EnterScope { .. }
            | FlowOp::ExitScope
            | FlowOp::ExitScopeBind { .. }
            | FlowOp::Noop => Schema::Empty,
        }
    }

    pub(crate) fn owned_body(&self, role: RuntimeFlowBodyRole) -> Option<&[FlowOp]> {
        self.body_schema().get(role)
    }
}

impl<'a> Iterator for RuntimeFlowOwnedBodies<'a> {
    type Item = (RuntimeFlowBodyRole, &'a [FlowOp]);

    fn next(&mut self) -> Option<Self::Item> {
        use RuntimeFlowBodyRole as Role;
        let next = match self.op.body_schema() {
            RuntimeFlowBodySchema::Empty => None,
            RuntimeFlowBodySchema::One(role, body) => (self.ordinal == 0).then_some((role, body)),
            RuntimeFlowBodySchema::Branches(then_ops, else_ops) => match self.ordinal {
                0 => Some((Role::Then, then_ops)),
                1 => Some((Role::Else, else_ops)),
                _ => None,
            },
            RuntimeFlowBodySchema::Await(observers) => {
                observers.get(self.ordinal).map(|observer| {
                    (
                        Role::AwaitObserver {
                            ordinal: self.ordinal,
                        },
                        observer.ops.as_slice(),
                    )
                })
            }
            RuntimeFlowBodySchema::Match(arms) => {
                let arm = arms.get(self.ordinal)?;
                if self.guard {
                    self.guard = false;
                    if let Some(guard) = &arm.guard {
                        return Some((Role::MatchGuard { arm: self.ordinal }, &guard.ops));
                    }
                }
                self.guard = true;
                Some((Role::MatchArm { arm: self.ordinal }, arm.ops.as_slice()))
            }
        };
        if next.is_some() {
            self.ordinal += 1;
        }
        next
    }
}

impl RuntimePlan {
    /// Visits every owned flow operation in deterministic inventory/preorder.
    ///
    /// Includes executable function sites (including Root Flows), line
    /// activation/actions, cancellation rules, and cleanup bodies. Each
    /// function body is visited once from its
    /// owning table; `ProjectCall` references do not recursively revisit them.
    /// This is a whole-plan inventory, not a path-sensitive execution trace.
    pub fn visit_flow_ops(&self, visitor: &mut impl FnMut(&FlowOp)) {
        self.inventory.visit_flow_ops(visitor)
    }

    /// Visits the same owned inventory as [`Self::visit_flow_ops`], stopping
    /// immediately at the first visitor error. Later children and table rows
    /// are not visited after rejection.
    pub fn try_visit_flow_ops<E>(
        &self,
        visitor: &mut impl FnMut(&FlowOp) -> Result<(), E>,
    ) -> Result<(), E> {
        self.inventory.try_visit_flow_ops(visitor)
    }
}

impl super::RuntimePlanInventory {
    /// Visits every owned flow operation in deterministic inventory/preorder.
    ///
    /// Includes executable function sites (including Root Flows), line
    /// activation/actions, cancellation rules, and cleanup bodies. Each
    /// function body is visited once from its
    /// owning table; `ProjectCall` references do not recursively revisit them.
    /// This is a whole-plan inventory, not a path-sensitive execution trace.
    pub fn visit_flow_ops(&self, visitor: &mut impl FnMut(&FlowOp)) {
        match self.try_visit_flow_ops(&mut |op| {
            visitor(op);
            Ok::<(), std::convert::Infallible>(())
        }) {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }

    /// Visits the same owned inventory as [`Self::visit_flow_ops`], stopping
    /// immediately at the first visitor error. Later children and table rows
    /// are not visited after rejection.
    pub fn try_visit_flow_ops<E>(
        &self,
        visitor: &mut impl FnMut(&FlowOp) -> Result<(), E>,
    ) -> Result<(), E> {
        self.try_visit_code_roots(&mut |_, body| match body {
            RuntimeCodeBodyRef::Operations(operations) => try_visit_ops(operations, visitor),
            RuntimeCodeBodyRef::Expression(_) => Ok(()),
        })
    }
}

/// Balanced body and operation boundaries used by the semantic owner.
/// Empty bodies still emit both boundaries; operation ordinals are source roles.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeFlowTreeEvent<'a> {
    EnterBody {
        role: RuntimeFlowBodyRole,
        ops: &'a [FlowOp],
    },
    ExitBody,
    EnterOperation {
        ordinal: usize,
        op: &'a FlowOp,
    },
    ExitOperation,
}

fn try_visit_ops<E>(
    ops: &[FlowOp],
    visitor: &mut impl FnMut(&FlowOp) -> Result<(), E>,
) -> Result<(), E> {
    try_visit_ops_events(ops, &mut |event| match event {
        RuntimeFlowTreeEvent::EnterOperation { op, .. } => visitor(op),
        RuntimeFlowTreeEvent::EnterBody { .. }
        | RuntimeFlowTreeEvent::ExitBody
        | RuntimeFlowTreeEvent::ExitOperation => Ok(()),
    })
}

pub(crate) fn try_visit_ops_events<'a, E>(
    ops: &'a [FlowOp],
    visitor: &mut impl FnMut(RuntimeFlowTreeEvent<'a>) -> Result<(), E>,
) -> Result<(), E> {
    use RuntimeFlowTreeEvent as Event;
    enum Frame<'a> {
        Ops(std::iter::Enumerate<std::slice::Iter<'a, FlowOp>>),
        Children(RuntimeFlowOwnedBodies<'a>),
    }
    visitor(Event::EnterBody {
        role: RuntimeFlowBodyRole::Body,
        ops,
    })?;
    let mut stack = vec![Frame::Ops(ops.iter().enumerate())];
    while let Some(frame) = stack.last_mut() {
        match frame {
            Frame::Ops(ops) => {
                if let Some((ordinal, op)) = ops.next() {
                    visitor(Event::EnterOperation { ordinal, op })?;
                    stack.push(Frame::Children(op.owned_bodies()));
                } else {
                    visitor(Event::ExitBody)?;
                    stack.pop();
                }
            }
            Frame::Children(children) => {
                if let Some((role, ops)) = children.next() {
                    visitor(Event::EnterBody { role, ops })?;
                    stack.push(Frame::Ops(ops.iter().enumerate()));
                } else {
                    visitor(Event::ExitOperation)?;
                    stack.pop();
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
