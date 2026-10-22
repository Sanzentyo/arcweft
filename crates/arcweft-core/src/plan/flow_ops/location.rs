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
pub(crate) struct RuntimeFlowBodyLocation {
    parents: Vec<OwnedBodyLocation>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeFlowOperationLocation {
    body: RuntimeFlowBodyLocation,
    operation: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeFlowLocationStep {
    Body(RuntimeFlowBodyRole),
    Operation(usize),
}

impl RuntimeFlowBodyLocation {
    pub(crate) fn is_root(&self) -> bool {
        self.parents.is_empty()
    }

    pub(crate) fn parent_count(&self) -> usize {
        self.parents.len()
    }

    pub(crate) fn enter(&mut self, operation: usize, role: RuntimeFlowBodyRole) {
        self.parents.push(OwnedBodyLocation { operation, role });
    }

    fn leave(&mut self) -> Option<usize> {
        self.parents.pop().map(|parent| parent.operation)
    }

    pub(crate) fn step_count(&self) -> Option<usize> {
        self.parents.len().checked_mul(2)?.checked_add(1)
    }

    pub(crate) fn steps(&self) -> impl Iterator<Item = RuntimeFlowLocationStep> + '_ {
        use RuntimeFlowLocationStep::{Body, Operation};
        std::iter::once(Body(RuntimeFlowBodyRole::Body)).chain(
            self.parents
                .iter()
                .flat_map(|parent| [Operation(parent.operation), Body(parent.role)]),
        )
    }

    /// Borrows even an empty original body. An absent child or foreign role is
    /// rejected instead of substituting an empty body or another code root.
    pub(crate) fn resolve<'a>(&self, root: &'a [FlowOp]) -> Option<&'a [FlowOp]> {
        let mut body = root;
        for parent in &self.parents {
            let operation = body.get(parent.operation)?;
            body = operation.owned_body(parent.role)?;
        }
        Some(body)
    }

    pub(crate) fn operation(self, operation: usize) -> RuntimeFlowOperationLocation {
        RuntimeFlowOperationLocation {
            body: self,
            operation,
        }
    }
}

impl RuntimeFlowOperationLocation {
    pub(crate) fn set_operation(&mut self, operation: usize) {
        self.operation = operation;
    }

    pub(crate) fn body(&self) -> &RuntimeFlowBodyLocation {
        &self.body
    }

    pub(crate) fn operation_index(&self) -> usize {
        self.operation
    }

    pub(crate) fn enter_body(&mut self, role: RuntimeFlowBodyRole) {
        self.body.enter(self.operation, role);
        self.operation = 0;
    }

    pub(crate) fn leave_body(&mut self) -> bool {
        let Some(operation) = self.body.leave() else {
            return false;
        };
        self.operation = operation;
        true
    }

    pub(crate) fn step_count(&self) -> Option<usize> {
        self.body.step_count()?.checked_add(1)
    }

    pub(crate) fn steps(&self) -> impl Iterator<Item = RuntimeFlowLocationStep> + '_ {
        self.body
            .steps()
            .chain(std::iter::once(RuntimeFlowLocationStep::Operation(
                self.operation,
            )))
    }

    /// Borrows the actual operation without reconstructing or copying its body.
    pub(crate) fn resolve<'a>(&self, root: &'a [FlowOp]) -> Option<&'a FlowOp> {
        self.body.resolve(root)?.get(self.operation)
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

    #[test]
    fn body_coordinates_borrow_empty_and_nested_original_bodies_without_substitution() {
        let root = [scope(vec![FlowOp::Noop, scope(vec![])])];
        let mut body = RuntimeFlowBodyLocation::default();
        assert!(body.is_root());
        assert!(std::ptr::eq(body.resolve(&root).unwrap(), root.as_slice()));
        body.enter(0, RuntimeFlowBodyRole::Body);
        let original = root[0].owned_body(RuntimeFlowBodyRole::Body).unwrap();
        assert!(!body.is_root());
        assert_eq!(body.parent_count(), 1);
        assert!(std::ptr::eq(body.resolve(&root).unwrap(), original));
        let operation = body.clone().operation(0);
        assert_eq!(operation.body(), &body);
        assert_eq!(operation.operation_index(), 0);
        assert!(std::ptr::eq(
            operation.resolve(&root).unwrap(),
            &original[0]
        ));
        body.enter(1, RuntimeFlowBodyRole::Body);
        assert!(body.resolve(&root).unwrap().is_empty());
        assert!(body.clone().operation(0).resolve(&root).is_none());
        body.enter(0, RuntimeFlowBodyRole::Body);
        assert!(body.resolve(&root).is_none());

        let mut foreign = RuntimeFlowBodyLocation::default();
        foreign.enter(0, RuntimeFlowBodyRole::Else);
        assert!(foreign.resolve(&root).is_none());
        assert!(
            RuntimeFlowBodyLocation::default()
                .resolve(&[])
                .unwrap()
                .is_empty()
        );
    }
}
