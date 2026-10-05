//! Owned executable operation traversal, independent of call-graph edges.

use super::{FlowOp, RuntimeFunctionSiteBody, RuntimePlan};
use crate::line_task::{LineTaskNode, ScopeExit};

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

/// Borrowed direct children of an operation, in structural source order.
pub struct RuntimeFlowOwnedBodies<'a> {
    op: &'a FlowOp,
    ordinal: usize,
    guard: bool,
}

impl FlowOp {
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

impl<'a> Iterator for RuntimeFlowOwnedBodies<'a> {
    type Item = (RuntimeFlowBodyRole, &'a [FlowOp]);

    fn next(&mut self) -> Option<Self::Item> {
        use RuntimeFlowBodyRole as Role;
        let next = match self.op {
            FlowOp::Await { observers, .. } => observers.get(self.ordinal).map(|observer| {
                (
                    Role::AwaitObserver {
                        ordinal: self.ordinal,
                    },
                    observer.ops.as_slice(),
                )
            }),
            FlowOp::LetElse { else_ops, .. } => {
                (self.ordinal == 0).then_some((Role::Else, else_ops.as_slice()))
            }
            FlowOp::If {
                then_ops, else_ops, ..
            }
            | FlowOp::IfLet {
                then_ops, else_ops, ..
            } => match self.ordinal {
                0 => Some((Role::Then, then_ops.as_slice())),
                1 => Some((Role::Else, else_ops.as_slice())),
                _ => None,
            },
            FlowOp::Match { arms, .. } => {
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
            FlowOp::FormatOperandAttempt { body, .. }
            | FlowOp::Loop { body, .. }
            | FlowOp::While { body, .. }
            | FlowOp::WhileLet { body, .. }
            | FlowOp::For { body, .. }
            | FlowOp::Thread { body, .. }
            | FlowOp::Scope { body, .. }
            | FlowOp::LetScope { ops: body, .. } => {
                (self.ordinal == 0).then_some((Role::Body, body.as_slice()))
            }
            FlowOp::LoopNext { body }
            | FlowOp::WhileNext { body, .. }
            | FlowOp::WhileLetNext { body, .. }
            | FlowOp::ForNext { body, .. } => {
                (self.ordinal == 0).then_some((Role::Body, body.as_ref()))
            }
            FlowOp::Bind(_)
            | FlowOp::Let { .. }
            | FlowOp::CompleteFormatOperand { .. }
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
            | FlowOp::CompleteAwaitObserver
            | FlowOp::Noop => None,
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
    /// Includes flows, executable function sites, line activation/actions, cancellation
    /// rules, and cleanup bodies. Function sites are visited once from their
    /// owning table; `ProjectCall` references do not recursively revisit them.
    /// This is a whole-plan inventory, not a path-sensitive execution trace.
    pub fn visit_flow_ops(&self, visitor: &mut impl FnMut(&FlowOp)) {
        for flow in self.flows() {
            visit_ops(flow.body().ops(), visitor);
        }
        for site in self.function_sites().iter() {
            if let RuntimeFunctionSiteBody::Executable(body) = site.body() {
                visit_ops(body.ops(), visitor);
            }
        }
        for group in self.line_task_groups() {
            visit_ops(group.activation_ops(), visitor);
            for node in group.nodes() {
                match node {
                    LineTaskNode::Action(ops) => visit_ops(ops, visitor),
                    LineTaskNode::Sequence(_)
                    | LineTaskNode::Start(_)
                    | LineTaskNode::Parallel { .. }
                    | LineTaskNode::Child { .. } => {}
                }
            }
            for rule in group.cancel_rules() {
                visit_ops(rule.action(), visitor);
            }
            for exit in [
                ScopeExit::Completed,
                ScopeExit::Cancelled,
                ScopeExit::Failed,
            ] {
                visit_ops(group.cleanup().actions(exit), visitor);
            }
        }
    }
}

fn visit_ops(ops: &[FlowOp], visitor: &mut impl FnMut(&FlowOp)) {
    enum Frame<'a> {
        Ops(std::slice::Iter<'a, FlowOp>),
        Children(RuntimeFlowOwnedBodies<'a>),
    }
    let mut stack = vec![Frame::Ops(ops.iter())];
    while let Some(frame) = stack.last_mut() {
        match frame {
            Frame::Ops(ops) => {
                if let Some(op) = ops.next() {
                    visitor(op);
                    stack.push(Frame::Children(op.owned_bodies()));
                } else {
                    stack.pop();
                }
            }
            Frame::Children(children) => {
                if let Some((_, ops)) = children.next() {
                    stack.push(Frame::Ops(ops.iter()));
                } else {
                    stack.pop();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
