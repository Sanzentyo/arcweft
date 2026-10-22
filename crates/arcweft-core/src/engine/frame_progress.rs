use super::{Engine, FlowFiber, FlowFiberStatus, FlowOp, NativeFlowExecutionFrame};
use crate::plan::RuntimePlan;
use crate::plan::code::{RuntimeCodeBodyRef, RuntimeCodeOwner};

impl NativeFlowExecutionFrame {
    pub(super) fn original_operation_remainder<'a>(
        &self,
        plan: &'a RuntimePlan,
    ) -> Option<&'a [FlowOp]> {
        if let Some(frame) = self.active_function() {
            return frame.original_operation_remainder(plan);
        }
        let cursor = self.cursor.as_ref()?;
        plan.flows
            .get(cursor.flow_index)?
            .body()
            .ops()
            .get(cursor.op_index..)
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
        if let Some(frame) = function {
            return match plan
                .inventory
                .code_body(RuntimeCodeOwner::Function(frame.site))
            {
                Some(RuntimeCodeBodyRef::Expression(_)) => true,
                Some(RuntimeCodeBodyRef::Operations(operations)) => operations
                    .get(frame.next_instruction)
                    .is_none_or(|operation| {
                        matches!(operation, FlowOp::Return(_) | FlowOp::ReturnExpr(_))
                    }),
                // Invalid code must reach its ordinary diagnostic boundary.
                None => false,
            };
        }
        if let Some(cursor) = &self.cursor {
            return plan.flows.get(cursor.flow_index).is_some_and(|flow| {
                flow.body()
                    .ops()
                    .get(cursor.op_index)
                    .is_none_or(|operation| {
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
