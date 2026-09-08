//! Owned executable operation traversal, independent of call-graph edges.

use super::{FlowOp, RuntimeFunctionSiteBody, RuntimePlan};
use crate::line_task::{LineTaskNode, ScopeExit};

impl RuntimePlan {
    /// Visits every owned flow operation in deterministic inventory/preorder.
    ///
    /// Includes flows, executable function sites, line actions, cancellation
    /// rules, and cleanup bodies. Function sites are visited once from their
    /// owning table; `ProjectCall` references do not recursively revisit them.
    /// This is a whole-plan inventory, not a path-sensitive execution trace.
    pub fn visit_flow_ops(&self, visitor: &mut impl FnMut(&FlowOp)) {
        for flow in self.flows() {
            visit_ops(&flow.ops, visitor);
        }
        for site in self.function_sites().iter() {
            if let RuntimeFunctionSiteBody::Executable(body) = site.body() {
                visit_ops(body.ops(), visitor);
            }
        }
        for group in self.line_task_groups() {
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
    for op in ops {
        visitor(op);
        match op {
            FlowOp::Await { observers, .. } => {
                for observer in observers {
                    visit_ops(&observer.ops, visitor);
                }
            }
            FlowOp::LetElse { else_ops, .. } => visit_ops(else_ops, visitor),
            FlowOp::If {
                then_ops, else_ops, ..
            }
            | FlowOp::IfLet {
                then_ops, else_ops, ..
            } => {
                visit_ops(then_ops, visitor);
                visit_ops(else_ops, visitor);
            }
            FlowOp::Match { arms, .. } => {
                for arm in arms {
                    visit_ops(&arm.ops, visitor);
                }
            }
            FlowOp::Loop { body, .. }
            | FlowOp::While { body, .. }
            | FlowOp::WhileLet { body, .. }
            | FlowOp::For { body, .. }
            | FlowOp::Thread { body, .. }
            | FlowOp::Scope(body)
            | FlowOp::LetScope { ops: body, .. } => visit_ops(body, visitor),
            FlowOp::LoopNext { body }
            | FlowOp::WhileNext { body, .. }
            | FlowOp::WhileLetNext { body, .. }
            | FlowOp::ForNext { body, .. } => visit_ops(body, visitor),
            FlowOp::Bind(_)
            | FlowOp::Let { .. }
            | FlowOp::AssignNominalField { .. }
            | FlowOp::LineOperation { .. }
            | FlowOp::CommitDialogueResult { .. }
            | FlowOp::Dialogue { .. }
            | FlowOp::Choice { .. }
            | FlowOp::AwaitMany { .. }
            | FlowOp::HostCall { .. }
            | FlowOp::ProjectCall { .. }
            | FlowOp::ApplyFunction { .. }
            | FlowOp::Break(_)
            | FlowOp::Continue
            | FlowOp::Goto(_)
            | FlowOp::GotoExpr(_)
            | FlowOp::Return(_)
            | FlowOp::ReturnExpr(_)
            | FlowOp::Effect(_)
            | FlowOp::EvaluatedEffect(_)
            | FlowOp::RegisterCleanup { .. }
            | FlowOp::CancelCleanup { .. }
            | FlowOp::EnterScope
            | FlowOp::ExitScope
            | FlowOp::ExitScopeBind { .. }
            | FlowOp::CompleteAwaitObserver
            | FlowOp::Noop => {}
        }
    }
}
