use super::*;
use crate::line_task::{LineCancelRule, LineCleanupPolicy, LineTaskCleanup, LineTaskGroup};
use crate::pattern::{RuntimePattern, RuntimePatternKind, RuntimeSemanticTypeId};
use crate::plan::{
    RuntimeAwaitPendingObserver, RuntimeMatchArm, RuntimeMatchGuard, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use crate::runtime_id::{RuntimeLineTaskNodeId, RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::value::{RuntimeExpr, RuntimeExprKind, RuntimeValue};
use arcweft_interaction_model::input::InputActionId;
use std::num::NonZeroU32;

fn ty() -> RuntimePlanTypeId {
    RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN)
}

fn value() -> RuntimeExpr {
    RuntimeExpr::from_admitted_parts(ty(), RuntimeExprKind::Value(RuntimeValue::Bool(true)))
}

fn pattern() -> RuntimePattern {
    RuntimePattern::from_admitted_parts(ty(), RuntimePatternKind::Discard)
}

fn marker(label: &str) -> FlowOp {
    FlowOp::Return(label.to_owned())
}

#[test]
fn roles_retain_empty_branches_and_sparse_guard_arm_ordinals() {
    use RuntimeFlowBodyRole as Role;
    let op = FlowOp::Match {
        scrutinee: value(),
        arms: vec![
            RuntimeMatchArm {
                pattern: pattern(),
                guard: None,
                ops: vec![FlowOp::If {
                    condition: value(),
                    then_ops: vec![],
                    else_ops: vec![marker("else")],
                }],
            },
            RuntimeMatchArm {
                pattern: pattern(),
                guard: Some(RuntimeMatchGuard {
                    candidate: RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN),
                    condition: Some(value()),
                    copy_locals: Box::new([]),
                    ops: vec![marker("guard")],
                }),
                ops: vec![marker("arm")],
            },
        ],
    };
    assert_eq!(
        op.owned_bodies().map(|(role, _)| role).collect::<Vec<_>>(),
        [
            Role::MatchArm { arm: 0 },
            Role::MatchGuard { arm: 1 },
            Role::MatchArm { arm: 1 }
        ]
    );
    let (_, first) = op.owned_bodies().next().unwrap();
    assert_eq!(
        first[0]
            .owned_bodies()
            .map(|(role, ops)| (role, ops.len()))
            .collect::<Vec<_>>(),
        [(Role::Then, 0), (Role::Else, 1)]
    );
    let mut returns = Vec::new();
    try_visit_ops(&[op], &mut |op| {
        if let FlowOp::Return(label) = op {
            returns.push(label.clone());
        }
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(returns, ["else", "guard", "arm"]);
}

#[test]
fn observer_bodies_are_owned_in_source_order() {
    let op = FlowOp::Await {
        binding: None,
        target: crate::plan::RuntimeNeedAwaitTarget::new(value()),
        observers: vec![
            RuntimeAwaitPendingObserver {
                pattern: pattern(),
                ops: vec![],
            },
            RuntimeAwaitPendingObserver {
                pattern: pattern(),
                ops: vec![marker("pending")],
            },
        ],
    };
    assert_eq!(
        op.owned_bodies()
            .map(|(role, ops)| (role, ops.len()))
            .collect::<Vec<_>>(),
        [
            (RuntimeFlowBodyRole::AwaitObserver { ordinal: 0 }, 0),
            (RuntimeFlowBodyRole::AwaitObserver { ordinal: 1 }, 1)
        ]
    );
}

fn inventory_plan(activation: Box<[FlowOp]>) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                RuntimeSemanticTypeId::from_bytes([1; 32]),
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    let mut plan = builder.finish().unwrap();
    // This isolated inventory fixture bypasses unrelated group attachment
    // validation; it exercises the same owned bodies as finished group rows.
    plan.line_task_groups.push(LineTaskGroup::new(
        crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([71; 32]),
        Box::new([]),
        Box::new([]),
        activation,
        ty(),
        Box::new([]),
        RuntimeLineTaskNodeId::from_zero_based(0).unwrap(),
        Box::new([LineTaskNode::Action(Box::new([marker("action")]))]),
        Box::new([LineCancelRule::new(
            InputActionId::new("Cancel").unwrap(),
            Box::new([marker("cancel")]),
        )]),
        LineTaskCleanup::new(
            Box::new([marker("completed")]),
            Box::new([marker("cancelled")]),
            Box::new([marker("failed")]),
            LineCleanupPolicy::default(),
        ),
    ));
    plan
}

#[test]
fn whole_plan_inventory_includes_activation_before_action_cancel_and_cleanup() {
    let plan = inventory_plan(Box::new([FlowOp::Loop {
        result: None,
        body: vec![marker("activation")],
    }]));
    let mut labels = Vec::new();
    plan.visit_flow_ops(&mut |op| {
        if let FlowOp::Return(label) = op {
            labels.push(label.clone());
        }
    });
    assert_eq!(
        labels,
        [
            "activation",
            "action",
            "cancel",
            "completed",
            "cancelled",
            "failed"
        ]
    );
}

#[test]
fn deep_owned_bodies_visit_without_native_recursion() {
    let depth = 20_000;
    let mut ops = vec![marker("leaf")];
    for _ in 0..depth {
        ops = vec![FlowOp::Loop {
            result: None,
            body: ops,
        }];
    }
    let mut count = 0;
    try_visit_ops(&ops, &mut |_| {
        count += 1;
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(count, depth + 1);
    // Drop the fixture iteratively too: this test concerns traversal, not the
    // recursive drop glue of a deliberately very deep synthetic tree.
    while let Some(op) = ops.pop() {
        if let FlowOp::Loop { body, .. } = op {
            ops = body;
        }
    }
}

#[test]
fn visitor_rejection_stops_before_later_children_and_inventory_rows() {
    let plan = inventory_plan(Box::new([FlowOp::If {
        condition: value(),
        then_ops: vec![marker("before"), marker("reject"), marker("after")],
        else_ops: vec![marker("else")],
    }]));
    let mut visited = Vec::new();
    let result = plan.try_visit_flow_ops(&mut |op| {
        if let FlowOp::Return(label) = op {
            visited.push(label.clone());
            if label == "reject" {
                return Err("first rejection");
            }
        }
        Ok(())
    });
    assert_eq!(result, Err("first rejection"));
    assert_eq!(visited, ["before", "reject"]);
}

#[test]
fn executable_site_verification_rejects_the_first_nested_activation_reference() {
    use crate::plan::entry_inventory::RuntimePlanError;
    use crate::runtime_id::RuntimeProjectCallSiteId;
    let first = RuntimeProjectCallSiteId::from_accepted_ordinal(NonZeroU32::MIN);
    let later = RuntimeProjectCallSiteId::from_accepted_ordinal(NonZeroU32::new(2).unwrap());
    let plan = inventory_plan(Box::new([FlowOp::If {
        condition: value(),
        then_ops: vec![FlowOp::Loop {
            result: None,
            body: vec![FlowOp::ProjectCall { site: first }],
        }],
        else_ops: vec![FlowOp::ProjectCall { site: later }],
    }]));
    assert!(
        matches!(plan.verify(), Err(RuntimePlanError::MissingProjectCallSite { site }) if site == first)
    );
}
