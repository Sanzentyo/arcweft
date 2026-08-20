use super::*;

use arcweft_core::awbc::fiber::FiberState;
use arcweft_core::awbc::schema::{
    AwbcEntryId, AwbcEntryTarget, AwbcInstruction, AwbcProgram, AwbcSafePointKind, AwbcTerminator,
};
use arcweft_core::awbc::vm::{self, VmExit, VmStepOptions};
use arcweft_core::entry::{EntryBindingIdentity, RuntimeEntryRoles};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeAwaitPendingObserverSeed, RuntimeAwaitTargetSeed,
    RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFlowOpSeed, RuntimeFlowSeed, RuntimeHostCallTargetSeed,
    RuntimeHostTaskRequestTemplateSeed, RuntimeLocalDeclarationSeed, RuntimePatternSeed,
    RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection,
    RuntimePlanTypeSeed, RuntimeRouteSpec,
};
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::task::{HostCapabilityId, NeedId, TaskId, TaskOutcomeContract};
use arcweft_core::value::RuntimeValue;

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::canonical(value).expect("test flow ID is valid")
}

fn entry_id(value: &str) -> EntryRuntimeId {
    EntryRuntimeId::canonical(value).expect("test entry ID is valid")
}

fn type_id(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn string_expr(value: &str) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        type_id(1),
        RuntimeExprSeedKind::Value(RuntimeValue::String(value.to_owned())),
    )
}

fn unit_expr() -> RuntimeExprSeed {
    RuntimeExprSeed::new(type_id(2), RuntimeExprSeedKind::Value(RuntimeValue::Unit))
}

fn bool_expr(value: bool) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        type_id(3),
        RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)),
    )
}

fn build_plan(
    flows: impl IntoIterator<Item = (FlowRuntimeId, Vec<RuntimeFlowOpSeed>)>,
    entries: impl IntoIterator<Item = RuntimeEntrySpec>,
) -> RuntimePlan {
    let flows = flows.into_iter().collect::<Vec<_>>();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(type_id(2), RuntimePlanTypeProjection::Unit),
            ],
            [],
            [],
            [],
        )
        .expect("test semantic facts admit");
    for (id, ops) in flows {
        builder
            .push_flow_seed(RuntimeFlowSeed::new(id, [], ops))
            .expect("test flow admits");
    }
    for entry in entries {
        builder.push_entry(entry).expect("test entry admits");
    }
    builder.finish().expect("test runtime plan seals")
}

fn flow_entry(id: &str, flow: FlowRuntimeId) -> RuntimeEntrySpec {
    RuntimeEntrySpec {
        id: entry_id(id),
        kind: RuntimeEntryKind::Cli,
        binding: EntryBindingIdentity::from_bytes([1; 32]),
        target: RuntimeEntryTarget::Flow(flow),
        roles: RuntimeEntryRoles::None,
    }
}

fn lower_plan(plan: &RuntimePlan) -> AwbcLowerReport {
    AwbcLowerer::new(
        plan,
        &arcweft_text_model::DialogueContentCatalog::new(),
        "test.arcw",
    )
    .lower()
    .expect("AWBC lowers builder-sealed runtime plan")
}

fn run_entry(program: &AwbcProgram) -> VmExit {
    let mut fiber =
        FiberState::for_entry(program, AwbcEntryId(0), 0, 256).expect("AWBC fiber initializes");
    vm::step(
        program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 128,
        },
    )
    .expect("AWBC VM executes entry")
    .exit
}

#[test]
fn builder_sealed_constant_return_lowers_and_executes() {
    let main = flow_id("main");
    let plan = build_plan(
        [(
            main.clone(),
            vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("ok"))],
        )],
        [flow_entry("main", main)],
    );
    let report = lower_plan(&plan);
    assert!(!report.program.functions.is_empty());
    assert_eq!(
        run_entry(&report.program),
        VmExit::Returned(Some(RuntimeValue::String("ok".to_owned())))
    );
}

#[test]
fn selected_entry_lowering_keeps_its_static_flow_closure() {
    let selected = entry_id("selected");
    let selected_flow = flow_id("selected");
    let shared = flow_id("shared");
    let unselected = flow_id("unselected");
    let plan = build_plan(
        [
            (
                selected_flow.clone(),
                vec![RuntimeFlowOpSeed::Goto(shared.clone())],
            ),
            (
                shared.clone(),
                vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("done"))],
            ),
            (
                unselected.clone(),
                vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("other"))],
            ),
        ],
        [
            RuntimeEntrySpec {
                id: selected.clone(),
                kind: RuntimeEntryKind::Cli,
                binding: EntryBindingIdentity::from_bytes([1; 32]),
                target: RuntimeEntryTarget::Flow(selected_flow.clone()),
                roles: RuntimeEntryRoles::None,
            },
            flow_entry("unselected", unselected.clone()),
        ],
    );
    let report = AwbcLowerer::for_entry(
        &plan,
        &arcweft_text_model::DialogueContentCatalog::new(),
        "test.arcw",
        &selected,
    )
    .lower()
    .expect("selected entry lowers");
    assert_eq!(report.program.entries.len(), 1);
    assert!(report.program.flow_function(&selected_flow).is_some());
    assert!(report.program.flow_function(&shared).is_some());
    assert!(report.program.flow_function(&unselected).is_none());
}

#[test]
fn dynamic_goto_keeps_all_accepted_flow_targets() {
    let selected = entry_id("selected");
    let selected_flow = flow_id("selected");
    let other = flow_id("other");
    let plan = build_plan(
        [
            (
                selected_flow.clone(),
                vec![RuntimeFlowOpSeed::GotoExpr(string_expr("other"))],
            ),
            (
                other.clone(),
                vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("done"))],
            ),
        ],
        [RuntimeEntrySpec {
            id: selected.clone(),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([1; 32]),
            target: RuntimeEntryTarget::Flow(selected_flow),
            roles: RuntimeEntryRoles::None,
        }],
    );
    let report = AwbcLowerer::for_entry(
        &plan,
        &arcweft_text_model::DialogueContentCatalog::new(),
        "test.arcw",
        &selected,
    )
    .lower()
    .expect("dynamic selected entry lowers");
    assert_eq!(report.program.flow_bindings.len(), 2);
    assert!(report.program.flow_function(&other).is_some());
}

#[test]
fn builder_sealed_plan_roundtrips_through_canonical_awbc() {
    let main = flow_id("main");
    let report = lower_plan(&build_plan(
        [(
            main.clone(),
            vec![RuntimeFlowOpSeed::ReturnExpr(unit_expr())],
        )],
        [flow_entry("main", main)],
    ));
    let encoded = report
        .program
        .encode_canonical()
        .expect("AWBC encoder accepts lowered plan");
    let decoded = AwbcProgram::decode_canonical(
        &encoded,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .expect("canonical AWBC decodes");
    assert_eq!(
        run_entry(&decoded),
        VmExit::Returned(Some(RuntimeValue::Unit))
    );
}

#[test]
fn discarded_host_call_result_still_has_an_awbc_destination() {
    let main = flow_id("main");
    let report = lower_plan(&build_plan(
        [(
            main.clone(),
            vec![
                RuntimeFlowOpSeed::HostCall {
                    binding: None,
                    target: RuntimeHostCallTargetSeed {
                        public_id: "test.notify".to_owned(),
                        capability: "test".to_owned(),
                        operation: "notify".to_owned(),
                        args: Vec::new(),
                        mode: RuntimeHostCallMode::Suspend,
                        deterministic: false,
                    },
                },
                RuntimeFlowOpSeed::ReturnExpr(unit_expr()),
            ],
        )],
        [flow_entry("main", main)],
    ));
    let destination = report.program.blocks.iter().find_map(|block| {
        let AwbcTerminator::HostCall { dst, .. } = block.terminator else {
            return None;
        };
        dst
    });
    assert!(destination.is_some());
}

#[test]
fn loop_break_paths_initialize_one_typed_result_before_binding() {
    let main = flow_id("main");
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(type_id(3), RuntimePlanTypeProjection::Bool),
            ],
            [RuntimeLocalDeclarationSeed::new(type_id(1))],
            [],
            [],
        )
        .expect("loop result facts admit");
    let result = admission.local_ids()[0].clone();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            main.clone(),
            [],
            vec![
                RuntimeFlowOpSeed::Loop {
                    result: Some(RuntimePatternSeed::new(
                        type_id(1),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: result.clone(),
                        },
                    )),
                    body: vec![RuntimeFlowOpSeed::If {
                        condition: bool_expr(true),
                        then_ops: vec![RuntimeFlowOpSeed::Break(Some(string_expr("then")))],
                        else_ops: vec![RuntimeFlowOpSeed::Break(Some(string_expr("else")))],
                    }],
                },
                RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                    type_id(1),
                    RuntimeExprSeedKind::Local(result),
                )),
            ],
        ))
        .expect("loop flow admits");
    builder
        .push_entry(flow_entry("main", main))
        .expect("loop entry admits");

    let report = lower_plan(&builder.finish().expect("loop plan seals"));
    assert_eq!(
        run_entry(&report.program),
        VmExit::Returned(Some(RuntimeValue::String("then".to_owned())))
    );
    assert!(
        report
            .program
            .blocks
            .iter()
            .any(|block| block.safe_point == AwbcSafePointKind::LoopBackedge)
    );
    assert!(!report.program.intrinsics.iter().any(|intrinsic| {
        matches!(
            report.program.strings[intrinsic.public_id.index()].as_str(),
            "flow.break" | "flow.continue"
        )
    }));
}

#[test]
fn nested_loops_bind_the_nearest_break_result() {
    let main = flow_id("main");
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::String,
            )],
            [
                RuntimeLocalDeclarationSeed::new(type_id(1)),
                RuntimeLocalDeclarationSeed::new(type_id(1)),
            ],
            [],
            [],
        )
        .expect("nested loop result facts admit");
    let inner_result = admission.local_ids()[0].clone();
    let outer_result = admission.local_ids()[1].clone();
    let binding = |local| {
        RuntimePatternSeed::new(
            type_id(1),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        )
    };
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            main.clone(),
            [],
            vec![
                RuntimeFlowOpSeed::Loop {
                    result: Some(binding(outer_result.clone())),
                    body: vec![
                        RuntimeFlowOpSeed::Loop {
                            result: Some(binding(inner_result.clone())),
                            body: vec![RuntimeFlowOpSeed::Break(Some(string_expr("nested")))],
                        },
                        RuntimeFlowOpSeed::Break(Some(RuntimeExprSeed::new(
                            type_id(1),
                            RuntimeExprSeedKind::Local(inner_result),
                        ))),
                    ],
                },
                RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                    type_id(1),
                    RuntimeExprSeedKind::Local(outer_result),
                )),
            ],
        ))
        .expect("nested loop flow admits");
    builder
        .push_entry(flow_entry("main", main))
        .expect("nested loop entry admits");

    let report = lower_plan(&builder.finish().expect("nested loop plan seals"));
    assert_eq!(
        run_entry(&report.program),
        VmExit::Returned(Some(RuntimeValue::String("nested".to_owned())))
    );
}

#[test]
fn loop_continue_targets_the_verified_backedge_header() {
    let main = flow_id("main");
    let plan = build_plan(
        [(
            main.clone(),
            vec![RuntimeFlowOpSeed::Loop {
                result: None,
                body: vec![RuntimeFlowOpSeed::Continue],
            }],
        )],
        [flow_entry("main", main)],
    );
    let report = lower_plan(&plan);

    assert!(!report.program.intrinsics.iter().any(|intrinsic| {
        report.program.strings[intrinsic.public_id.index()] == "flow.continue"
    }));
    assert!(
        report
            .program
            .blocks
            .iter()
            .enumerate()
            .any(|(index, block)| {
                matches!(
                    block.terminator,
                    AwbcTerminator::Jump { target }
                        if target.index() <= index
                            && report.program.blocks[target.index()].safe_point
                                == AwbcSafePointKind::LoopBackedge
                )
            })
    );
}

#[test]
fn typed_runtime_ids_drive_static_goto_and_server_route_targets() {
    let main = flow_id("chapter.main");
    let next = flow_id("chapter.next");
    let plan = build_plan(
        [
            (main, vec![RuntimeFlowOpSeed::Goto(next.clone())]),
            (
                next.clone(),
                vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("ok"))],
            ),
        ],
        [RuntimeEntrySpec {
            id: entry_id("server"),
            kind: RuntimeEntryKind::Server,
            binding: EntryBindingIdentity::from_bytes([1; 32]),
            target: RuntimeEntryTarget::Routes(vec![RuntimeRouteSpec {
                method: "GET".to_owned(),
                path: "/next".to_owned(),
                target: next,
                bindings: Vec::new(),
            }]),
            roles: RuntimeEntryRoles::None,
        }],
    );
    let report = lower_plan(&plan);
    let goto_target = report
        .program
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            AwbcTerminator::GotoStatic { function, .. } => Some(function),
            _ => None,
        })
        .expect("static goto lowers to a function target");
    let route_target = match &report.program.entries[0].target {
        AwbcEntryTarget::Routes(routes) => routes[0].target,
        AwbcEntryTarget::Function(_) => panic!("test entry must lower as routes"),
    };
    assert_eq!(goto_target, route_target);
}

#[test]
fn await_observers_lower_to_progress_dispatch_and_rewait_backedge() {
    let main = flow_id("await.observer");
    let progress_type = type_id(4);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(progress_type, RuntimePlanTypeProjection::Progress),
            ],
            [],
            [],
            [],
        )
        .expect("Await observer types admit");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            main.clone(),
            [],
            vec![RuntimeFlowOpSeed::Await {
                binding: None,
                target: RuntimeAwaitTargetSeed {
                    need: NeedId("need.observe".to_owned()),
                    task: TaskId("task.observe".to_owned()),
                    outcome: TaskOutcomeContract::new(
                        arcweft_core::pattern::RuntimeCheckedType::String,
                    ),
                    request: RuntimeHostTaskRequestTemplateSeed {
                        capability: HostCapabilityId("test".to_owned()),
                        operation: "observe".to_owned(),
                        args: Vec::new(),
                    },
                },
                observers: vec![RuntimeAwaitPendingObserverSeed {
                    pattern: RuntimePatternSeed::new(
                        progress_type,
                        RuntimePatternSeedKind::Discard,
                    ),
                    ops: vec![RuntimeFlowOpSeed::Noop],
                }],
            }],
        ))
        .expect("Await observer flow admits");
    builder
        .push_entry(flow_entry("await.observer", main))
        .expect("Await observer entry admits");
    let report = lower_plan(&builder.finish().expect("Await observer plan seals"));
    let (await_index, observer) = report
        .program
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, block)| match block.terminator {
            AwbcTerminator::Await {
                observer: Some(observer),
                ..
            } => Some((index, observer)),
            _ => None,
        })
        .expect("Await retains a Progress observer resume");

    assert!(report.program.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            AwbcInstruction::TestPattern { value, .. } if *value == observer.destination
        )
    }));
    assert!(report.program.blocks.iter().any(|block| {
        matches!(
            block.terminator,
            AwbcTerminator::Jump { target } if target.index() == await_index
        )
    }));
}
