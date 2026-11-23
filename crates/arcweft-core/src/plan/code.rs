//! Original executable roots, independent of a fiber's active position.

use super::flow_ops::RuntimeFlowBodyLocation;
use super::{FlowOp, RuntimeFunctionSiteBody, RuntimePlanInventory};
use crate::line_task::{LineTaskNode, ScopeExit};
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLineTaskGroupId, RuntimeLineTaskNodeId};
use crate::value::RuntimeExpr;
use serde::{Deserialize, Serialize};

/// An untrusted coordinate until resolved against its original inventory.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeCodeOwner {
    Function(#[serde(with = "function_coordinate")] RuntimeFunctionSiteId),
    LineTask {
        group: RuntimeLineTaskGroupId,
        body: RuntimeLineCodeBody,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeLineCodeBody {
    Activation,
    Action(RuntimeLineTaskNodeId),
    Cancellation { rule: usize },
    Cleanup(ScopeExit),
}

// A persisted code coordinate is inert until resolved against its Plan.
// Give only this field a reader, keeping function site IDs constructor-bound.
pub(crate) mod function_coordinate {
    use super::*;
    use std::num::NonZeroU32;

    pub(crate) fn serialize<S: serde::Serializer>(
        site: &RuntimeFunctionSiteId,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        site.get().serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<RuntimeFunctionSiteId, D::Error> {
        NonZeroU32::deserialize(deserializer).map(RuntimeFunctionSiteId::from_accepted_ordinal)
    }
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
        self.code_body_at(owner, &RuntimeFlowBodyLocation::default())
    }

    /// Selects a structural body inside its named original row. Expression
    /// roots remain expressions and cannot authorize an operation-body path.
    pub(crate) fn code_body_at(
        &self,
        owner: RuntimeCodeOwner,
        location: &RuntimeFlowBodyLocation,
    ) -> Option<RuntimeCodeBodyRef<'_>> {
        let operations = match owner {
            RuntimeCodeOwner::Function(site) => {
                return match self.function_sites().get(site)?.body() {
                    RuntimeFunctionSiteBody::Expression(expression) if location.is_root() => {
                        Some(RuntimeCodeBodyRef::Expression(expression))
                    }
                    RuntimeFunctionSiteBody::Expression(_) => None,
                    RuntimeFunctionSiteBody::Executable(body) => location
                        .resolve(body.ops())
                        .map(RuntimeCodeBodyRef::Operations),
                };
            }
            RuntimeCodeOwner::LineTask { group, body } => self
                .line_task_groups()
                .get(group.index())?
                .code_body(body)?,
        };
        location
            .resolve(operations)
            .map(RuntimeCodeBodyRef::Operations)
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
            group.try_visit_code_roots(|body, ops| {
                visitor(
                    RuntimeCodeOwner::LineTask {
                        group: group_id,
                        body,
                    },
                    RuntimeCodeBodyRef::Operations(ops),
                )
            })?;
        }
        Ok(())
    }
}

impl crate::line_task::LineTaskGroup {
    pub(super) fn code_body(&self, body: RuntimeLineCodeBody) -> Option<&[FlowOp]> {
        Some(match body {
            RuntimeLineCodeBody::Activation => self.activation_ops(),
            RuntimeLineCodeBody::Action(node) => {
                let LineTaskNode::Action(ops) = self.node(node)? else {
                    return None;
                };
                ops
            }
            RuntimeLineCodeBody::Cancellation { rule } => self.cancel_rules().get(rule)?.action(),
            RuntimeLineCodeBody::Cleanup(exit) => self.cleanup().actions(exit),
        })
    }

    pub(super) fn try_visit_code_roots<'a, E>(
        &'a self,
        mut visit: impl FnMut(RuntimeLineCodeBody, &'a [FlowOp]) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(RuntimeLineCodeBody::Activation, self.activation_ops())?;
        for (ordinal, node) in self.nodes().iter().enumerate() {
            if let LineTaskNode::Action(ops) = node {
                let node = RuntimeLineTaskNodeId::from_zero_based(ordinal)
                    .expect("admitted line-task node ordinal");
                visit(RuntimeLineCodeBody::Action(node), ops)?;
            }
        }
        for (rule, cancellation) in self.cancel_rules().iter().enumerate() {
            visit(
                RuntimeLineCodeBody::Cancellation { rule },
                cancellation.action(),
            )?;
        }
        for exit in [
            ScopeExit::Completed,
            ScopeExit::Cancelled,
            ScopeExit::Failed,
        ] {
            visit(
                RuntimeLineCodeBody::Cleanup(exit),
                self.cleanup().actions(exit),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan::{RuntimeFlowBodyRole, RuntimeFunctionSiteBodyKind},
        tests::function_application::returning_function_plan,
    };

    #[test]
    fn body_lookup_retains_original_expression_and_operation_rows_and_rejects_invalid_paths() {
        let plan = returning_function_plan(RuntimeFunctionSiteBodyKind::Executable);
        let root = RuntimeFlowBodyLocation::default();
        let mut nested = RuntimeFlowBodyLocation::default();
        nested.enter(0, RuntimeFlowBodyRole::Body);
        let mut visited = 0;
        for (site, declaration) in plan.function_sites().iter_with_ids() {
            let owner = RuntimeCodeOwner::Function(site);
            let resolved = plan.inventory.code_body_at(owner, &root).unwrap();
            match (declaration.body(), resolved) {
                (
                    RuntimeFunctionSiteBody::Expression(expected),
                    RuntimeCodeBodyRef::Expression(actual),
                ) => {
                    assert!(std::ptr::eq(actual, expected));
                }
                (
                    RuntimeFunctionSiteBody::Executable(expected),
                    RuntimeCodeBodyRef::Operations(actual),
                ) => {
                    assert!(std::ptr::eq(actual, expected.ops()));
                }
                _ => panic!("an original code root cannot change its body kind"),
            }
            assert!(plan.inventory.code_body_at(owner, &nested).is_none());
            visited += 1;
        }
        assert_eq!(visited, 3);

        let missing_site = crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(
            std::num::NonZeroU32::new(u32::MAX).unwrap(),
        );
        assert!(
            plan.inventory
                .code_body_at(RuntimeCodeOwner::Function(missing_site), &root)
                .is_none()
        );
    }
}
