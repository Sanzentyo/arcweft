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
    plan.inventory.line_task_groups.push(LineTaskGroup::new(
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

fn labelled(label: &str) -> RuntimeExpr {
    RuntimeExpr::from_admitted_parts(
        ty(),
        RuntimeExprKind::Value(RuntimeValue::String(label.to_owned())),
    )
}

#[test]
fn value_roles_preserve_sparse_match_and_observer_source_positions() {
    use RuntimeFlowValueRole as Role;
    let op = FlowOp::Match {
        scrutinee: labelled("subject"),
        arms: vec![
            RuntimeMatchArm {
                pattern: pattern(),
                guard: None,
                ops: vec![],
            },
            RuntimeMatchArm {
                pattern: pattern(),
                guard: Some(RuntimeMatchGuard {
                    candidate: RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN),
                    condition: Some(labelled("guard")),
                    copy_locals: Box::new([]),
                    ops: vec![],
                }),
                ops: vec![],
            },
        ],
    };
    let mut roles = Vec::new();
    op.try_visit_value_roots(&mut |role, _| {
        roles.push(role);
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(
        roles,
        [
            Role::Scrutinee,
            Role::MatchPattern { arm: 0 },
            Role::MatchPattern { arm: 1 },
            Role::MatchGuard { arm: 1 }
        ]
    );
    let await_op = FlowOp::Await {
        binding: None,
        target: crate::plan::RuntimeNeedAwaitTarget::new(value()),
        observers: vec![
            RuntimeAwaitPendingObserver {
                pattern: pattern(),
                ops: vec![],
            },
            RuntimeAwaitPendingObserver {
                pattern: pattern(),
                ops: vec![],
            },
        ],
    };
    roles.clear();
    await_op
        .try_visit_value_roots(&mut |role, _| {
            roles.push(role);
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(
        roles,
        [
            Role::Source,
            Role::AwaitObserverPattern { ordinal: 0 },
            Role::AwaitObserverPattern { ordinal: 1 }
        ]
    );
}

#[test]
fn value_root_rejection_stops_before_later_match_roots() {
    let op = FlowOp::Match {
        scrutinee: value(),
        arms: vec![RuntimeMatchArm {
            pattern: pattern(),
            guard: None,
            ops: vec![],
        }],
    };
    let mut visits = 0;
    let result = op.try_visit_value_roots(&mut |_, _| {
        visits += 1;
        Err("reject subject")
    });
    assert_eq!(result, Err("reject subject"));
    assert_eq!(visits, 1);
}

#[test]
fn static_control_copy_rejects_affine_pattern_audio_and_deep_child_literals() {
    let affine = || {
        RuntimeExpr::from_admitted_parts(
            ty(),
            RuntimeExprKind::Value(RuntimeValue::NeedHandle(crate::tests::reusable_need(
                "need.control",
            ))),
        )
    };
    let affine_pattern = RuntimePattern::from_admitted_parts(
        ty(),
        RuntimePatternKind::Literal(RuntimeValue::NeedHandle(crate::tests::reusable_need(
            "need.pattern",
        ))),
    );
    assert!(
        !FlowOp::Let {
            pattern: affine_pattern,
            expr: value()
        }
        .literals_permit_copy()
    );
    assert!(
        !FlowOp::RegisterCleanup {
            key: "cleanup".to_owned(),
            effect: crate::effect::LineEffectRequest::Audio(Box::new(
                crate::audio::RuntimeAudioCommand::StopAll {
                    fade_out_millis: affine()
                }
            ))
        }
        .literals_permit_copy()
    );
    assert!(
        !FlowOp::Loop {
            result: None,
            body: vec![FlowOp::ReturnExpr(affine())]
        }
        .literals_permit_copy()
    );
    assert!(
        FlowOp::Loop {
            result: None,
            body: vec![FlowOp::ReturnExpr(value())]
        }
        .literals_permit_copy()
    );
}

#[test]
fn choice_audio_values_keep_option_and_effect_positions() {
    let op = FlowOp::Choice {
        id: None,
        options: vec![
            crate::plan::ChoiceRuntimeOption {
                id: None,
                label: "empty".to_owned(),
                target: None,
                out: None,
                effects: vec![],
            },
            crate::plan::ChoiceRuntimeOption {
                id: None,
                label: "audio".to_owned(),
                target: None,
                out: None,
                effects: vec![crate::effect::LineEffectRequest::Audio(Box::new(
                    crate::audio::RuntimeAudioCommand::SetCaptureMonitor {
                        capture: labelled("capture"),
                        bus: None,
                        gain_db_milli: labelled("gain"),
                    },
                ))],
            },
        ],
    };
    let mut actual = Vec::new();
    op.try_visit_value_roots(&mut |role, node| {
        if let crate::value::RuntimeExpressionNode::Expression(expr) = node
            && let RuntimeExprKind::Value(RuntimeValue::String(label)) = expr.kind()
        {
            actual.push((role, label.clone()));
        }
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(
        actual,
        [
            (
                RuntimeFlowValueRole::ChoiceAudioArgument {
                    option: 1,
                    effect: 0,
                    argument: 0
                },
                "capture".to_owned()
            ),
            (
                RuntimeFlowValueRole::ChoiceAudioArgument {
                    option: 1,
                    effect: 0,
                    argument: 1
                },
                "gain".to_owned()
            ),
        ]
    );
}

#[test]
fn balanced_flow_events_distinguish_empty_branches_and_nested_sibling_bodies() {
    let ops = [FlowOp::If {
        condition: value(),
        then_ops: vec![],
        else_ops: vec![
            FlowOp::Loop {
                result: None,
                body: vec![marker("leaf")],
            },
            marker("sibling"),
        ],
    }];
    let mut actual = Vec::new();
    try_visit_ops_events(&ops, &mut |event| {
        let label = match event {
            RuntimeFlowTreeEvent::EnterBody { role, ops } => format!("body:{role:?}:{}", ops.len()),
            RuntimeFlowTreeEvent::ExitBody => "end-body".to_owned(),
            RuntimeFlowTreeEvent::EnterOperation { ordinal, .. } => format!("op:{ordinal}"),
            RuntimeFlowTreeEvent::ExitOperation => "end-op".to_owned(),
        };
        actual.push(label);
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(
        actual,
        [
            "body:Body:1",
            "op:0",
            "body:Then:0",
            "end-body",
            "body:Else:2",
            "op:0",
            "body:Body:1",
            "op:0",
            "end-op",
            "end-body",
            "end-op",
            "op:1",
            "end-op",
            "end-body",
            "end-op",
            "end-body"
        ]
    );
}

#[test]
fn balanced_flow_rejection_stops_before_pending_boundaries() {
    let mut actual = Vec::new();
    let ops = [FlowOp::Loop {
        result: None,
        body: vec![marker("reject"), marker("later")],
    }];
    let result = try_visit_ops_events(&ops, &mut |event| {
        match event {
            RuntimeFlowTreeEvent::EnterOperation {
                op: FlowOp::Return(label),
                ..
            } => {
                actual.push(label.clone());
                return Err("leaf rejection");
            }
            RuntimeFlowTreeEvent::ExitBody | RuntimeFlowTreeEvent::ExitOperation => {
                actual.push("exit".to_owned());
            }
            _ => {}
        }
        Ok(())
    });
    assert_eq!(result, Err("leaf rejection"));
    assert_eq!(actual, ["reject"]);
}

#[test]
fn semantic_body_roles_do_not_renumber_sparse_source_positions() {
    let mut meter = crate::task::semantic::TaskSemanticMeter::new(20, 100);
    let mut encoder =
        crate::task::semantic::TaskSemanticEncoder::new(b"body-role.v1\0", &mut meter);
    for role in [
        RuntimeFlowBodyRole::Then,
        RuntimeFlowBodyRole::MatchGuard { arm: 3 },
        RuntimeFlowBodyRole::AwaitObserver { ordinal: 7 },
    ] {
        role.encode_semantic_path(&mut encoder);
    }
    let mut expected = b"body-role.v1\0".to_vec();
    expected.extend([1, 4]);
    expected.extend(3_u32.to_le_bytes());
    expected.push(3);
    expected.extend(7_u32.to_le_bytes());
    assert_eq!(encoder.finish().unwrap(), blake3::hash(&expected));
}
