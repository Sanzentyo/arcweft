//! Structural coordinates in the original operation tree.
//!
//! A location is metadata, not an executable-owner or value-copy proof. Its
//! owning program must authenticate the root before resolving the coordinate.

use super::{FlowOp, RuntimeFlowBodyRole};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnedBodyLocation {
    operation: usize,
    role: RuntimeFlowBodyRole,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeFlowOperationLocation {
    parents: Vec<OwnedBodyLocation>,
    operation: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeFlowLocationStep {
    Body(RuntimeFlowBodyRole),
    Operation(usize),
}

impl RuntimeFlowOperationLocation {
    pub(in crate::plan) fn set_operation(&mut self, operation: usize) {
        self.operation = operation;
    }

    pub(in crate::plan) fn enter_body(&mut self, role: RuntimeFlowBodyRole) {
        self.parents.push(OwnedBodyLocation {
            operation: self.operation,
            role,
        });
        self.operation = 0;
    }

    pub(in crate::plan) fn leave_body(&mut self) -> bool {
        let Some(parent) = self.parents.pop() else {
            return false;
        };
        self.operation = parent.operation;
        true
    }

    pub(in crate::plan) fn step_count(&self) -> Option<usize> {
        self.parents.len().checked_mul(2)?.checked_add(2)
    }

    pub(in crate::plan) fn steps(&self) -> impl Iterator<Item = RuntimeFlowLocationStep> + '_ {
        use RuntimeFlowLocationStep::{Body, Operation};
        std::iter::once(Body(RuntimeFlowBodyRole::Body))
            .chain(
                self.parents
                    .iter()
                    .flat_map(|parent| [Operation(parent.operation), Body(parent.role)]),
            )
            .chain(std::iter::once(Operation(self.operation)))
    }

    /// Borrows the actual operation without reconstructing or copying its body.
    pub(in crate::plan) fn resolve<'a>(&self, root: &'a [FlowOp]) -> Option<&'a FlowOp> {
        let mut body = root;
        for parent in &self.parents {
            let operation = body.get(parent.operation)?;
            body = operation.owned_body(parent.role)?;
        }
        body.get(self.operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(body: Vec<FlowOp>) -> FlowOp {
        FlowOp::Scope {
            identity: crate::scope::RuntimeScopeIdentity::Anonymous,
            body,
        }
    }

    #[test]
    fn locations_borrow_original_nested_operations_and_reject_foreign_roles_and_indices() {
        let root = [FlowOp::Noop, scope(vec![scope(vec![FlowOp::Noop])])];
        let mut location = RuntimeFlowOperationLocation::default();
        location.set_operation(1);
        assert!(std::ptr::eq(
            location.resolve(&root).unwrap(),
            &raw const root[1]
        ));
        location.enter_body(RuntimeFlowBodyRole::Body);
        location.enter_body(RuntimeFlowBodyRole::Body);
        assert!(matches!(location.resolve(&root), Some(FlowOp::Noop)));
        assert_eq!(location.step_count(), Some(6));
        assert_eq!(location.steps().count(), 6);
        location.set_operation(1);
        assert!(location.resolve(&root).is_none());
        assert!(location.leave_body());
        assert!(location.leave_body());
        location.enter_body(RuntimeFlowBodyRole::Else);
        assert!(location.resolve(&root).is_none());
        assert!(location.leave_body());
        assert!(!location.leave_body());
    }

    #[test]
    fn structural_steps_retain_root_body_parent_operations_and_exact_child_roles() {
        let mut location = RuntimeFlowOperationLocation::default();
        location.set_operation(3);
        location.enter_body(RuntimeFlowBodyRole::MatchGuard { arm: 7 });
        location.set_operation(2);
        location.enter_body(RuntimeFlowBodyRole::AwaitObserver { ordinal: 4 });
        location.set_operation(5);
        assert_eq!(
            location.steps().collect::<Vec<_>>(),
            vec![
                RuntimeFlowLocationStep::Body(RuntimeFlowBodyRole::Body),
                RuntimeFlowLocationStep::Operation(3),
                RuntimeFlowLocationStep::Body(RuntimeFlowBodyRole::MatchGuard { arm: 7 }),
                RuntimeFlowLocationStep::Operation(2),
                RuntimeFlowLocationStep::Body(RuntimeFlowBodyRole::AwaitObserver { ordinal: 4 }),
                RuntimeFlowLocationStep::Operation(5),
            ]
        );
    }
}
