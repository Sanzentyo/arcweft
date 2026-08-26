use super::*;

use arcweft_core::awbc::fiber::FiberState;
use arcweft_core::awbc::schema::{
    AwbcEntryId, AwbcEntryTarget, AwbcInstruction, AwbcProgram, AwbcSafePointKind, AwbcTerminator,
};
use arcweft_core::awbc::vm::{self, VmExit, VmStepOptions};
use arcweft_core::entry::{
    EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeAwaitPendingObserverSeed, RuntimeAwaitTargetSeed,
    RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed, RuntimeHostCallTargetSeed,
    RuntimeHostTaskRequestTemplateSeed, RuntimeHttpMethod, RuntimeLocalDeclarationSeed,
    RuntimeLocalSeedId, RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan,
    RuntimePlanBuildError, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    RuntimePureHelperOrigin, RuntimePureHelperSeed, RuntimePureInputType, RuntimePureOutputType,
    RuntimeReceiverMode, RuntimeRoutePath, RuntimeRoutePathSegment, RuntimeRouteSpec,
    RuntimeTraitMethodIdentity, RuntimeTraitMethodSeed,
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

fn flow_schema(flow: &FlowRuntimeId) -> RuntimeFlowSchema {
    RuntimeFlowSchema {
        flow: flow.clone(),
        parameters: Vec::new(),
    }
}

fn flow_executable(flow: &FlowRuntimeId) -> RuntimeFlowExecutable {
    RuntimeFlowExecutable {
        flow: flow.clone(),
        contract: FlowContractHash::from_bytes([0xf0; 32]),
        controller: None,
    }
}

fn build_plan(
    flows: impl IntoIterator<Item = (FlowRuntimeId, Vec<RuntimeFlowOpSeed>)>,
    entries: impl IntoIterator<Item = RuntimeEntrySpec>,
) -> RuntimePlan {
    let flows = flows.into_iter().collect::<Vec<_>>();
    let entries = entries.into_iter().collect::<Vec<_>>();
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
            .push_flow_schema(flow_schema(&id))
            .expect("test flow schema admits");
        builder
            .push_flow_seed(RuntimeFlowSeed::new(id, [], ops))
            .expect("test flow admits");
    }
    let mut executable_flows = Vec::new();
    for entry in &entries {
        match &entry.target {
            RuntimeEntryTarget::Flow(flow) | RuntimeEntryTarget::Controller(flow) => {
                if !executable_flows.contains(flow) {
                    builder
                        .push_flow_executable(flow_executable(flow))
                        .expect("test flow executable admits");
                    executable_flows.push(flow.clone());
                }
            }
            RuntimeEntryTarget::Routes(routes) => {
                for route in routes {
                    if !executable_flows.contains(&route.target) {
                        builder
                            .push_flow_executable(flow_executable(&route.target))
                            .expect("test route flow executable admits");
                        executable_flows.push(route.target.clone());
                    }
                }
            }
        }
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

fn foreign_local_seed() -> RuntimeLocalSeedId {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::Bool,
            )],
            [RuntimeLocalDeclarationSeed::new(type_id(1))],
            [],
            [],
        )
        .expect("foreign local admission");
    admission.local_ids()[0].clone()
}

fn builder_with_local() -> (RuntimePlanBuilder, RuntimeLocalSeedId) {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::Bool,
            )],
            [RuntimeLocalDeclarationSeed::new(type_id(1))],
            [],
            [],
        )
        .expect("local admission");
    (builder, admission.local_ids()[0].clone())
}

fn invalid_let_expression(binding: RuntimeLocalSeedId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        type_id(1),
        RuntimeExprSeedKind::Let {
            binding,
            expr: Box::new(bool_expr(true)),
            body: Box::new(bool_expr(true)),
        },
    )
}

fn plan_with_local() -> (
    RuntimePlan,
    arcweft_core::runtime_id::RuntimeLocalDeclarationId,
) {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::String,
            )],
            [RuntimeLocalDeclarationSeed::new(type_id(1))],
            [],
            [],
        )
        .expect("local plan admission");
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "local".to_owned(),
            inputs: admission.local_ids().to_vec().into_boxed_slice(),
            input_abi: vec![RuntimePureInputType::Value],
            output_abi: RuntimePureOutputType::Value,
            body: string_expr("ok"),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("local helper admission");
    let plan = builder.finish().expect("local plan seals");
    let local = plan.pure_helpers()[0].input_locals[0];
    (plan, local)
}

#[test]
fn missing_local_type_is_reported_instead_of_becoming_a_success_type() {
    let (_, missing) = plan_with_local();
    let plan = build_plan([], []);
    let mut inventory = AwbcInventory::new("test.arcw", AwbcLowerOptions::default());
    let dynamic = inventory.dynamic_ty();

    assert_eq!(
        crate::awbc_lower::pattern::admitted_local_type(&mut inventory, &plan, missing),
        dynamic
    );
    assert!(inventory.take_diagnostics().iter().any(|diagnostic| {
        diagnostic.path == format!("local.{missing}") && diagnostic.is_error()
    }));
}

#[test]
fn invalid_local_seeds_cannot_produce_an_awbc_plan() {
    let foreign = foreign_local_seed();

    let mut flow_builder = RuntimePlanBuilder::new();
    assert_eq!(
        flow_builder.push_flow_seed(RuntimeFlowSeed::new(
            flow_id("invalid_plan"),
            [foreign.clone()],
            vec![RuntimeFlowOpSeed::Noop],
        )),
        Err(RuntimePlanBuildError::ForeignLocalSeed)
    );
    assert_eq!(flow_builder.finish(), Err(RuntimePlanBuildError::Poisoned));

    let mut pure_builder = RuntimePlanBuilder::new();
    pure_builder
        .admit_semantic_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::Bool,
            )],
            [],
            [],
            [],
        )
        .expect("pure helper type admission");
    assert_eq!(
        pure_builder.push_pure_helper_seed(RuntimePureHelperSeed {
            name: "invalid.local".to_owned(),
            inputs: Box::new([]),
            input_abi: Vec::new(),
            output_abi: RuntimePureOutputType::Value,
            body: invalid_let_expression(foreign.clone()),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        }),
        Err(RuntimePlanBuildError::ForeignLocalSeed)
    );
    assert_eq!(pure_builder.finish(), Err(RuntimePlanBuildError::Poisoned));

    let (mut trait_builder, receiver) = builder_with_local();
    assert_eq!(
        trait_builder.push_trait_method_seed(RuntimeTraitMethodSeed {
            identity: RuntimeTraitMethodIdentity {
                impl_id: 0,
                trait_id: None,
                witness: None,
                trait_name: None,
                self_type: "bool".to_owned(),
                method_name: "invalid_local".to_owned(),
                monomorph_label: "invalid_local".to_owned(),
            },
            receiver: RuntimeReceiverMode::Owned,
            inputs: vec![receiver].into_boxed_slice(),
            input_abi: vec![RuntimePureInputType::Value],
            output_abi: RuntimePureOutputType::Value,
            body: invalid_let_expression(foreign),
        }),
        Err(RuntimePlanBuildError::ForeignLocalSeed)
    );
    assert_eq!(trait_builder.finish(), Err(RuntimePlanBuildError::Poisoned));
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
                        contract: None,
                        args: Vec::new(),
                        result: type_id(2),
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
        .push_flow_executable(flow_executable(&main))
        .expect("loop flow executable admits");
    builder
        .push_flow_schema(flow_schema(&main))
        .expect("loop flow schema admits");
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
            intrinsic.identity.as_label(),
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
        .push_flow_executable(flow_executable(&main))
        .expect("nested loop flow executable admits");
    builder
        .push_flow_schema(flow_schema(&main))
        .expect("nested loop flow schema admits");
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

    assert!(
        !report
            .program
            .intrinsics
            .iter()
            .any(|intrinsic| { intrinsic.identity.as_label() == "flow.continue" })
    );
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
                method: RuntimeHttpMethod::Get,
                path: RuntimeRoutePath::try_new([RuntimeRoutePathSegment::Literal(
                    "next".to_owned(),
                )])
                .expect("route path"),
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
        AwbcEntryTarget::Function { .. } => panic!("test entry must lower as routes"),
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
        .push_flow_executable(flow_executable(&main))
        .expect("Await observer flow executable admits");
    builder
        .push_flow_schema(flow_schema(&main))
        .expect("Await observer flow schema admits");
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
