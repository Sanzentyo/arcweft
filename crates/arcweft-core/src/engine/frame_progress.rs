use super::work::{NativePendingWorkKind, NativeSourceRangeRole};
use super::{Engine, FlowFiber, FlowFiberStatus, FlowOp, NativeFlowExecutionFrame};
use crate::plan::RuntimePlan;

impl NativeFlowExecutionFrame {
    pub(super) fn original_operation_remainder<'a>(
        &self,
        plan: &'a RuntimePlan,
    ) -> Option<&'a [FlowOp]> {
        for work in &self.pending_ops {
            match &work.kind {
                NativePendingWorkKind::Source { position, role } => {
                    let operations = position.operations(plan)?;
                    if operations.is_empty() && *role != NativeSourceRangeRole::FunctionRoot {
                        continue;
                    }
                    return Some(operations);
                }
                NativePendingWorkKind::Resume(_) => return None,
            }
        }
        self.cursor.as_ref()?.operations(plan)
    }

    pub(super) fn normalize_pending_source_ranges(&mut self, plan: &RuntimePlan) {
        while self.pending_ops.front().is_some_and(|work| {
            matches!(&work.kind, NativePendingWorkKind::Source { position, role }
                if *role != NativeSourceRangeRole::FunctionRoot
                    && position.operations(plan).is_some_and(<[FlowOp]>::is_empty))
        }) {
            self.pending_ops.pop_front();
        }
    }

    /// A task return stays at its original position until its descendants drain.
    /// Returning to a caller continues the same task and does not join it.
    fn next_transition_completes_task(&self, plan: &RuntimePlan) -> bool {
        let function = self.active_function();
        if function.is_some_and(|frame| frame.continuation.returns_to_caller()) {
            return false;
        }
        for work in &self.pending_ops {
            match &work.kind {
                NativePendingWorkKind::Resume(_) => return false,
                NativePendingWorkKind::Source { position, role } => {
                    if let Some(operations) = position.operations(plan) {
                        if operations.is_empty() && *role != NativeSourceRangeRole::FunctionRoot {
                            continue;
                        }
                        return operations.first().is_none_or(|operation| {
                            matches!(operation, FlowOp::Return(_) | FlowOp::ReturnExpr(_))
                        });
                    }
                    return position.expression(plan).is_some();
                }
            }
        }
        if let Some(cursor) = &self.cursor {
            return cursor.expression(plan).is_some()
                || cursor.operations(plan).is_some_and(|operations| {
                    operations.first().is_none_or(|operation| {
                        matches!(operation, FlowOp::Return(_) | FlowOp::ReturnExpr(_))
                    })
                });
        }
        true
    }
}

impl Engine {
    pub(super) fn fiber_waits_for_join(&self, fiber: &FlowFiber) -> bool {
        if fiber.owner.requests_scope_close() || !self.has_joined_work_for(fiber.id) {
            return false;
        }
        match fiber.frame.status {
            FlowFiberStatus::Running => fiber.frame.next_transition_completes_task(&self.plan),
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_) => true,
            _ => false,
        }
    }

    /// Skip joined parents without consuming their code, rotating their frame,
    /// or charging a source operation. Suspended children retain their ordinary
    /// publication/input resumption path; closing owners still run cleanup.
    pub(super) fn next_child_fiber_index(&self) -> Option<usize> {
        self.child_fibers
            .iter()
            .position(|child| !self.fiber_waits_for_join(child))
    }
}
