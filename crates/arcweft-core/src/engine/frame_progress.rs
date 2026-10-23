use super::{Engine, FlowFiber, FlowFiberStatus, FlowOp, NativeFlowExecutionFrame};
use crate::plan::RuntimePlan;

impl NativeFlowExecutionFrame {
    pub(super) fn original_operation_remainder<'a>(
        &self,
        plan: &'a RuntimePlan,
    ) -> Option<&'a [FlowOp]> {
        self.cursor.as_ref()?.operations(plan)
    }

    /// A task return stays at its original position until its descendants drain.
    /// Returning to a caller continues the same task and does not join it.
    fn next_transition_completes_task(&self, plan: &RuntimePlan) -> bool {
        let function = self.active_function();
        if function.is_some_and(|frame| frame.continuation.returns_to_caller()) {
            return false;
        }
        if let Some(operation) = self.pending_ops.front() {
            return matches!(operation, FlowOp::Return(_) | FlowOp::ReturnExpr(_));
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
