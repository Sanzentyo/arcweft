use std::sync::Arc;

use crate::plan::code::{RuntimeCodeBodyRef, RuntimeCodeOwner};
use crate::plan::{
    FlowOp, RuntimeFlowBodyLocation, RuntimeFlowBodyRole, RuntimeFlowLocationStep, RuntimePlan,
};
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeExpr, RuntimeRetainedValueAdmission};

/// A position in the original code retained by its enclosing native frame.
/// Coordinates carry no executable authority. Cloning shares only immutable
/// path metadata; it never copies code, expressions, bindings or runtime values.
#[derive(Debug, PartialEq)]
pub struct NativeCodeCursor {
    owner: RuntimeCodeOwner,
    body: Arc<RuntimeFlowBodyLocation>,
    frontier: NativeCodeFrontier,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NativeCodeFrontier {
    Expression,
    Operations { next: usize, end: usize },
}

/// An authored operation retained by a runtime continuation. This shares its
/// body's existing path metadata; it does not allocate an operation wrapper or
/// copy any part of the executable operation.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeSourceOperation {
    position: NativeCodeCursor,
}

impl Clone for NativeCodeCursor {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner,
            body: Arc::clone(&self.body),
            frontier: self.frontier,
        }
    }
}

impl NativeCodeCursor {
    #[cfg(test)]
    pub(super) fn corrupt_operation_for_test(&mut self, next: usize) {
        let end = match self.frontier {
            NativeCodeFrontier::Operations { end, .. } => end,
            NativeCodeFrontier::Expression => 0,
        };
        self.frontier = NativeCodeFrontier::Operations { next, end };
    }

    pub(super) fn root(plan: &RuntimePlan, owner: RuntimeCodeOwner) -> Option<Self> {
        let body = RuntimeFlowBodyLocation::default();
        let frontier = match plan.inventory.code_body_at(owner, &body)? {
            RuntimeCodeBodyRef::Expression(_) => NativeCodeFrontier::Expression,
            RuntimeCodeBodyRef::Operations(operations) => NativeCodeFrontier::Operations {
                next: 0,
                end: operations.len(),
            },
        };
        Some(Self {
            owner,
            body: Arc::new(body),
            frontier,
        })
    }

    pub(super) const fn owner(&self) -> RuntimeCodeOwner {
        self.owner
    }

    pub(super) fn is_root(&self) -> bool {
        self.body.is_root()
    }

    pub(super) const fn operation_index(&self) -> Option<usize> {
        match self.frontier {
            NativeCodeFrontier::Expression => None,
            NativeCodeFrontier::Operations { next, .. } => Some(next),
        }
    }

    pub(super) fn remaining_operation_count(&self) -> Option<usize> {
        match self.frontier {
            NativeCodeFrontier::Expression => None,
            NativeCodeFrontier::Operations { next, end } => end.checked_sub(next),
        }
    }

    pub(super) fn set_operation(&mut self, next: usize) -> bool {
        match &mut self.frontier {
            NativeCodeFrontier::Operations { next: current, end } if next <= *end => {
                *current = next;
                true
            }
            NativeCodeFrontier::Expression | NativeCodeFrontier::Operations { .. } => false,
        }
    }

    pub(super) fn operations<'a>(&self, plan: &'a RuntimePlan) -> Option<&'a [FlowOp]> {
        let NativeCodeFrontier::Operations { next, end } = self.frontier else {
            return None;
        };
        let RuntimeCodeBodyRef::Operations(operations) =
            plan.inventory.code_body_at(self.owner, &self.body)?
        else {
            return None;
        };
        (end == operations.len())
            .then(|| operations.get(next..end))
            .flatten()
    }

    pub(super) fn expression<'a>(&self, plan: &'a RuntimePlan) -> Option<&'a RuntimeExpr> {
        if !matches!(self.frontier, NativeCodeFrontier::Expression) {
            return None;
        }
        match plan.inventory.code_body_at(self.owner, &self.body)? {
            RuntimeCodeBodyRef::Expression(expression) => Some(expression),
            RuntimeCodeBodyRef::Operations(_) => None,
        }
    }

    pub(super) fn source_operation(&self, plan: &RuntimePlan) -> Option<NativeSourceOperation> {
        self.operations(plan)?.first()?;
        Some(NativeSourceOperation {
            position: self.clone(),
        })
    }

    pub(super) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let invalid = |message: &str| AwbcRuntimeValueSnapshotError::Message {
            message: message.into(),
        };
        // The enclosing frame authenticates its complete original lease first.
        // Bound coordinates before walking their structural path or copying it.
        let owner = admission
            .program_owner()
            .cloned()
            .ok_or_else(|| invalid("native code position requires its original executable"))?;
        let crate::task::RuntimeProgramOwner::Plan(plan) = owner else {
            return Err(invalid(
                "native code position requires its original Plan executable",
            ));
        };
        admission.validate_metadata_collection(self.body.parent_count())?;
        for step in self.body.steps() {
            match step {
                RuntimeFlowLocationStep::Body(role) => {
                    admission.validate_metadata(&role.tag_and_ordinal())?;
                }
                RuntimeFlowLocationStep::Operation(ordinal) => {
                    admission.validate_metadata(&ordinal)?;
                }
            }
        }
        match self.owner {
            RuntimeCodeOwner::Function(site) => admission.validate_metadata(&site.get())?,
            RuntimeCodeOwner::LineTask { group, body } => {
                admission.validate_metadata(&group.index())?;
                match body {
                    crate::plan::code::RuntimeLineCodeBody::Activation => {}
                    crate::plan::code::RuntimeLineCodeBody::Action(node) => {
                        admission.validate_metadata(&node.index())?;
                    }
                    crate::plan::code::RuntimeLineCodeBody::Cancellation { rule } => {
                        admission.validate_metadata(&rule)?;
                    }
                    crate::plan::code::RuntimeLineCodeBody::Cleanup(exit) => {
                        admission.validate_metadata(&exit)?;
                    }
                }
            }
        }
        match self.frontier {
            NativeCodeFrontier::Expression => {
                if self.expression(&plan).is_none() {
                    return Err(invalid(
                        "native expression position differs from its original body",
                    ));
                }
            }
            NativeCodeFrontier::Operations { next, end } => {
                admission.validate_metadata(&(next, end))?;
                if self.operations(&plan).is_none() {
                    return Err(invalid(
                        "native operation range differs from its original body",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl NativeSourceOperation {
    pub(super) const fn owner(&self) -> RuntimeCodeOwner {
        self.position.owner
    }

    pub(super) fn operation<'a>(&self, plan: &'a RuntimePlan) -> Option<&'a FlowOp> {
        self.position.operations(plan)?.first()
    }

    pub(super) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.position.admit_retained(admission)?;
        let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
            unreachable!("the original Native position requires its Plan owner")
        };
        if self.operation(plan).is_none() {
            return Err(AwbcRuntimeValueSnapshotError::Message {
                message: "native continuation does not reference an original operation".into(),
            });
        }
        Ok(())
    }

    /// Resolves through the common original-body grammar. Admission bounds the
    /// complete new path before its metadata is allocated or copied.
    pub(super) fn child_body(
        &self,
        role: RuntimeFlowBodyRole,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<NativeCodeCursor, AwbcRuntimeValueSnapshotError> {
        let invalid = |message: &str| AwbcRuntimeValueSnapshotError::Message {
            message: message.into(),
        };
        self.admit_retained(admission)?;
        admission.validate_metadata(&role.tag_and_ordinal())?;
        let count = self
            .position
            .body
            .parent_count()
            .checked_add(1)
            .ok_or_else(|| invalid("native source body path exceeds metadata capacity"))?;
        admission.validate_metadata_collection(count)?;
        let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
            unreachable!("the original Native position requires its Plan owner")
        };
        let operations = self
            .operation(plan)
            .and_then(|operation| operation.owned_body(role))
            .ok_or_else(|| invalid("native source operation has no body for the selected role"))?;
        let next = self
            .position
            .operation_index()
            .expect("an admitted original operation has an operation ordinal");
        let mut body = self.position.body.as_ref().clone();
        body.enter(next, role);
        Ok(NativeCodeCursor {
            owner: self.position.owner,
            body: Arc::new(body),
            frontier: NativeCodeFrontier::Operations {
                next: 0,
                end: operations.len(),
            },
        })
    }
}
