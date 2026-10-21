//! Original executable roots, independent of a fiber's active position.

use super::{FlowOp, RuntimeFunctionSiteBody, RuntimePlanInventory};
use crate::line_task::{LineTaskNode, ScopeExit};
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLineTaskGroupId, RuntimeLineTaskNodeId};
use crate::value::RuntimeExpr;

/// An untrusted coordinate until resolved against its original inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeCodeOwner {
    Function(RuntimeFunctionSiteId),
    LineTask {
        group: RuntimeLineTaskGroupId,
        body: RuntimeLineCodeBody,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeLineCodeBody {
    Activation,
    Action(RuntimeLineTaskNodeId),
    Cancellation { rule: usize },
    Cleanup(ScopeExit),
}

/// A view of the actual owning row. An expression root returns that original
/// expression; it does not allocate a synthetic `ReturnExpr` operation list.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeCodeBodyRef<'a> {
    Expression(&'a RuntimeExpr),
    Operations(&'a [FlowOp]),
}

impl<'a> From<&'a RuntimeFunctionSiteBody> for RuntimeCodeBodyRef<'a> {
    fn from(body: &'a RuntimeFunctionSiteBody) -> Self {
        match body {
            RuntimeFunctionSiteBody::Expression(expression) => Self::Expression(expression),
            RuntimeFunctionSiteBody::Executable(body) => Self::Operations(body.ops()),
        }
    }
}

impl RuntimePlanInventory {
    /// Resolves only the named owning row and role. A structural node, missing
    /// cancellation rule, or foreign ordinal never substitutes another body.
    pub(crate) fn code_body(&self, owner: RuntimeCodeOwner) -> Option<RuntimeCodeBodyRef<'_>> {
        let operations = match owner {
            RuntimeCodeOwner::Function(site) => {
                return Some(self.function_sites().get(site)?.body().into());
            }
            RuntimeCodeOwner::LineTask { group, body } => {
                let group = self.line_task_groups().get(group.index())?;
                match body {
                    RuntimeLineCodeBody::Activation => group.activation_ops(),
                    RuntimeLineCodeBody::Action(node) => match group.node(node)? {
                        LineTaskNode::Action(operations) => operations,
                        LineTaskNode::Sequence(_)
                        | LineTaskNode::Start(_)
                        | LineTaskNode::Parallel { .. }
                        | LineTaskNode::Child { .. } => return None,
                    },
                    RuntimeLineCodeBody::Cancellation { rule } => {
                        group.cancel_rules().get(rule)?.action()
                    }
                    RuntimeLineCodeBody::Cleanup(exit) => group.cleanup().actions(exit),
                }
            }
        };
        Some(RuntimeCodeBodyRef::Operations(operations))
    }

    /// Visits each original root once, including empty roots, in the owning
    /// inventory's order. Referenced functions are separate rows, not children.
    pub(crate) fn try_visit_code_roots<E>(
        &self,
        visitor: &mut impl FnMut(RuntimeCodeOwner, RuntimeCodeBodyRef<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        for (site, declaration) in self.function_sites().iter_with_ids() {
            visitor(RuntimeCodeOwner::Function(site), declaration.body().into())?;
        }
        for (ordinal, group) in self.line_task_groups().iter().enumerate() {
            let group_id = RuntimeLineTaskGroupId::from_zero_based(ordinal)
                .expect("admitted line-task group ordinal");
            let mut visit = |body| {
                let owner = RuntimeCodeOwner::LineTask {
                    group: group_id,
                    body,
                };
                visitor(
                    owner,
                    self.code_body(owner)
                        .expect("code owner belongs to this admitted inventory"),
                )
            };
            visit(RuntimeLineCodeBody::Activation)?;
            for (ordinal, node) in group.nodes().iter().enumerate() {
                match node {
                    LineTaskNode::Action(_) => {
                        let node = RuntimeLineTaskNodeId::from_zero_based(ordinal)
                            .expect("admitted line-task node ordinal");
                        visit(RuntimeLineCodeBody::Action(node))?;
                    }
                    LineTaskNode::Sequence(_)
                    | LineTaskNode::Start(_)
                    | LineTaskNode::Parallel { .. }
                    | LineTaskNode::Child { .. } => {}
                }
            }
            for rule in 0..group.cancel_rules().len() {
                visit(RuntimeLineCodeBody::Cancellation { rule })?;
            }
            for exit in [
                ScopeExit::Completed,
                ScopeExit::Cancelled,
                ScopeExit::Failed,
            ] {
                visit(RuntimeLineCodeBody::Cleanup(exit))?;
            }
        }
        Ok(())
    }
}
