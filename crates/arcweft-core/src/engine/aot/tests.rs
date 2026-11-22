use crate::{
    aot::AotProgram,
    effect::{LineEffectRequest, RuntimeLog},
    engine::{Engine, FlowControlStackEntryKind, FlowExit, FlowFiberStatus, NativeCodeCursor},
    executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor},
    pattern::RuntimeCheckedType,
    plan::{
        FlowRuntimeId, RuntimeDropPolicySeed, RuntimeEffectFieldSeed, RuntimeEffectSet,
        RuntimeEvaluatedEffectSeed, RuntimeExecutableBodySeed, RuntimeExprSeed,
        RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed,
        RuntimeFunctionDefinitionIdentity, RuntimeFunctionSiteDeclarationSeed,
        RuntimeLineEffectSeed, RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection,
        RuntimePlanTypeSeed,
    },
    pure::VmRuntimePureCallBackend,
    root::RootEventInput,
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    value::RuntimeValue,
};

#[test]
fn aot_blocks_use_function_identity_when_flow_and_function_orders_differ() {
    let plan = crate::tests::function_application::returning_function_plan(
        crate::plan::RuntimeFunctionSiteBodyKind::Executable,
    );
    let flow = &plan.flows()[0];
    assert_ne!(flow.function_site().get().get(), 1);
    let program = AotProgram::from_runtime_plan(&plan);
    let block = program.flow_block(flow.function_site()).unwrap();
    assert_eq!(block.id, flow.id);
    assert_eq!(block.ops, flow.body().ops().len());
    assert!(std::ptr::eq(block, &program.flows()[0]));
    for (site, _) in plan.function_sites().iter_with_ids() {
        if site != flow.function_site() {
            assert!(program.flow_block(site).is_none());
        }
    }
}

#[test]
fn typed_log_effect_uses_the_pre_lowered_path_with_native_one_op_parity() {
    let (plan, flow) = effect_plan(None);
    assert_eq!(
        AotProgram::from_runtime_plan(&plan).flows()[0].lowered_linear_ops(),
        2
    );
    for options in budgets() {
        let (effects, status, fast) = compare_execution(&plan, &flow, options);
        assert_eq!(
            status,
            FlowFiberStatus::Done(FlowExit::Return("done".into()))
        );
        assert_eq!(fast, 2);
        assert_one_log(&effects);
    }
}

#[test]
fn typed_control_effect_keeps_native_fallback_without_replaying_the_prefix() {
    let (plan, flow) = effect_plan(Some(RuntimeEvaluatedEffectSeed::Panic(string("stop"))));
    assert_eq!(
        AotProgram::from_runtime_plan(&plan).flows()[0].lowered_linear_ops(),
        1
    );
    for options in budgets() {
        let (effects, status, fast) = compare_execution(&plan, &flow, options);
        assert_eq!(status, FlowFiberStatus::Failed("stop".into()));
        assert_eq!(fast, 1);
        assert_one_log(&effects);
        assert_eq!(effects.iter().filter(|effect| matches!(effect, LineEffectRequest::Panic(message) if message == "stop")).count(), 1);
    }
}

#[test]
fn typed_drop_policy_keeps_the_native_activation_owner() {
    let (plan, flow) = effect_plan(Some(RuntimeEvaluatedEffectSeed::Drop {
        target: RuntimeExprSeed::new(
            RuntimeCheckedType::Unit.semantic_identity_digest(),
            RuntimeExprSeedKind::Value(RuntimeValue::Unit),
        ),
        policy: RuntimeDropPolicySeed::Default,
    }));
    assert_eq!(
        AotProgram::from_runtime_plan(&plan).flows()[0].lowered_linear_ops(),
        1
    );
    for options in budgets() {
        let (effects, status, fast) = compare_execution(&plan, &flow, options);
        assert_eq!(
            status,
            FlowFiberStatus::Done(FlowExit::Return("done".into()))
        );
        assert_eq!(fast, 1);
        assert_one_log(&effects);
    }
}

#[test]
fn deferred_root_event_uses_native_ingress_before_resuming_aot() {
    let (plan, flow) = effect_plan(None);
    let invocation = || plan.clone().seal_flow_invocation(flow.clone(), []).unwrap();
    let mut native = ArcweftRuntimeExecutor::from_runtime_flow_invocation(
        invocation(),
        ArcweftExecutionTier::RuntimePlanVm,
    )
    .unwrap();
    let mut aot = ArcweftRuntimeExecutor::from_runtime_flow_invocation(
        invocation(),
        ArcweftExecutionTier::StructuredAot,
    )
    .unwrap();
    let event = RootEventInput::new("deferred".into());
    let input = || RuntimeStepInput {
        deferred_root_events: vec![event.clone()],
        ..RuntimeStepInput::default()
    };
    let options = budgets()[0];
    let mut native_backend = VmRuntimePureCallBackend::default();
    let mut aot_backend = VmRuntimePureCallBackend::default();
    let expected = native.step_with_pure_backend(input(), options, &mut native_backend);
    let actual = aot.step_with_pure_backend(input(), options, &mut aot_backend);
    assert_eq!(actual, expected);
    assert_eq!(actual.output.requests.root_events_next_step, vec![event]);
    assert_one_log(&actual.output.effects.line);
    assert_eq!(aot.fast_path_ops(), 0);

    let expected =
        native.step_with_pure_backend(RuntimeStepInput::default(), options, &mut native_backend);
    let actual = aot.step_with_pure_backend(RuntimeStepInput::default(), options, &mut aot_backend);
    assert_eq!(actual, expected);
    assert_eq!(
        actual.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return("done".into()))
    );
    assert!(actual.output.effects.line.is_empty());
    assert!(actual.output.requests.root_events_next_step.is_empty());
    assert_eq!(aot.fast_path_ops(), 1);
}

#[test]
fn aot_step_replenishes_the_same_bounded_task_allowance_as_native() {
    let (plan, flow) = effect_plan(None);
    let program = AotProgram::from_runtime_plan(&plan);
    for max_new_task_requests in [0, 1] {
        let mut native = Engine::for_flow(plan.clone(), &flow).unwrap();
        let mut aot = Engine::for_flow(plan.clone(), &flow).unwrap();
        let mut backend = VmRuntimePureCallBackend::default();
        let options = RuntimeStepOptions {
            max_new_task_requests,
            ..budgets()[0]
        };
        for _ in 0..2 {
            assert!(aot.can_start_aot_linear_step(&program, &RuntimeStepInput::default()));
            let expected = native.step(RuntimeStepInput::default(), options);
            let (actual, fast) =
                aot.step_prechecked_aot_linear_with_pure_backend(&program, options, &mut backend);
            assert_eq!(actual, expected);
            assert_eq!(fast, 1);
            for _ in 0..max_new_task_requests {
                assert!(native.reserve_new_task_request());
                assert!(aot.reserve_new_task_request());
            }
            assert!(!native.reserve_new_task_request());
            assert!(!aot.reserve_new_task_request());
        }
    }
}

#[test]
fn aot_cleanup_restores_original_registration_and_preserves_cancellation() {
    let register = |key: &str| RuntimeFlowOpSeed::RegisterCleanup {
        key: key.into(),
        effect: RuntimeLineEffectSeed::Static(LineEffectRequest::Log(RuntimeLog {
            level: "info".into(),
            message: key.into(),
            fields: Vec::new(),
        })),
    };
    let (plan, flow) = log_plan(vec![
        register("keep"),
        register("cancel"),
        RuntimeFlowOpSeed::CancelCleanup {
            key: "cancel".into(),
        },
    ]);
    let program = AotProgram::from_runtime_plan(&plan);
    let mut engine = Engine::for_flow(plan.clone(), &flow).unwrap();
    let owner = crate::task::RuntimeProgramOwner::Plan(engine.program_plan());
    let options = budgets()[0];
    let mut backend = VmRuntimePureCallBackend::default();
    assert!(engine.can_start_aot_linear_step(&program, &RuntimeStepInput::default()));
    let (result, fast) =
        engine.step_prechecked_aot_linear_with_pure_backend(&program, options, &mut backend);
    assert_eq!(fast, 1);
    assert!(result.output.effects.line.is_empty());

    let mut image = engine.inert_rollback_image().unwrap();
    let wire = serde_json::to_vec(&(
        &image.fiber.frame.control_stack,
        &image.fiber.frame.root_cleanups,
    ))
    .unwrap();
    (
        image.fiber.frame.control_stack,
        image.fiber.frame.root_cleanups,
    ) = serde_json::from_slice(&wire).unwrap();
    let mut foreign = image.clone();
    let frame = &mut foreign.fiber.frame;
    let cleanup = frame
        .control_stack
        .iter_mut()
        .find_map(|entry| match entry {
            FlowControlStackEntryKind::Scope { cleanups, .. } => cleanups.first_mut(),
            _ => None,
        })
        .or_else(|| frame.root_cleanups.first_mut())
        .unwrap();
    let site = plan.flows()[0].function_site();
    cleanup.source =
        NativeCodeCursor::root(&plan, crate::plan::code::RuntimeCodeOwner::Function(site))
            .unwrap()
            .source_operation_at_offset(&plan, 2)
            .unwrap();
    assert!(
        Engine::from_rollback_image(foreign, &owner)
            .unwrap_err()
            .contains("native cleanup has no original registration")
    );

    let mut restored = Engine::from_rollback_image(image, &owner).unwrap();
    let mut effects = Vec::new();
    for _ in 0..3 {
        assert!(restored.can_start_aot_linear_step(&program, &RuntimeStepInput::default()));
        let (result, fast) =
            restored.step_prechecked_aot_linear_with_pure_backend(&program, options, &mut backend);
        assert_eq!(fast, 1);
        effects.extend(result.output.effects.line);
    }
    let (expected, status, fast) = compare_execution(&plan, &flow, options);
    assert_eq!(
        status,
        FlowFiberStatus::Done(FlowExit::Return("done".into()))
    );
    assert_eq!(fast, 4);
    assert_eq!(effects, expected);
    let [LineEffectRequest::Log(log)] = effects.as_slice() else {
        panic!("only the uncanceled cleanup should emit a log");
    };
    assert_eq!(log.message, "keep");
}

fn budgets() -> [RuntimeStepOptions; 3] {
    [
        (RuntimeStepMode::OneOp, 1),
        (RuntimeStepMode::Drain, 1),
        (RuntimeStepMode::Drain, 8),
    ]
    .map(|(mode, max_ops)| RuntimeStepOptions {
        mode,
        budget: RuntimeStepBudget { max_ops },
        ..RuntimeStepOptions::default()
    })
}

fn compare_execution(
    plan: &RuntimePlan,
    flow: &FlowRuntimeId,
    options: RuntimeStepOptions,
) -> (Vec<LineEffectRequest>, FlowFiberStatus, usize) {
    let invocation = || plan.clone().seal_flow_invocation(flow.clone(), []).unwrap();
    let mut native = ArcweftRuntimeExecutor::from_runtime_flow_invocation(
        invocation(),
        ArcweftExecutionTier::RuntimePlanVm,
    )
    .unwrap();
    let mut aot = ArcweftRuntimeExecutor::from_runtime_flow_invocation(
        invocation(),
        ArcweftExecutionTier::StructuredAot,
    )
    .unwrap();
    let mut native_backend = VmRuntimePureCallBackend::default();
    let mut aot_backend = VmRuntimePureCallBackend::default();
    let mut effects = Vec::new();
    for _ in 0..8 {
        let expected = native.step_with_pure_backend(
            RuntimeStepInput::default(),
            options,
            &mut native_backend,
        );
        let actual =
            aot.step_with_pure_backend(RuntimeStepInput::default(), options, &mut aot_backend);
        assert!(actual.stats.executed_ops <= options.budget.max_ops);
        assert_eq!(actual, expected);
        effects.extend(actual.output.effects.line);
        if matches!(
            actual.fiber_status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        ) {
            return (effects, actual.fiber_status, aot.fast_path_ops());
        }
    }
    panic!("the bounded effect fixture must terminate without replay");
}

fn assert_one_log(effects: &[LineEffectRequest]) {
    let logs = effects
        .iter()
        .filter_map(|effect| match effect {
            LineEffectRequest::Log(log) => Some(log),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [log] = logs.as_slice() else {
        panic!("one materialized typed log")
    };
    assert_eq!(log.level, "info");
    assert_eq!(log.message, "typed = \"quotes\"");
    assert_eq!(log.fields.len(), 1);
    assert_eq!(log.fields[0].name, "payload");
    assert_eq!(log.fields[0].value, "value = \"kept\"");
}

fn string(value: &str) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        RuntimeCheckedType::String.semantic_identity_digest(),
        RuntimeExprSeedKind::Value(RuntimeValue::String(value.into())),
    )
}

fn effect_plan(after_log: Option<RuntimeEvaluatedEffectSeed>) -> (RuntimePlan, FlowRuntimeId) {
    let mut ops = vec![RuntimeFlowOpSeed::EvaluatedEffect(
        RuntimeEvaluatedEffectSeed::Log {
            level: "info".into(),
            message: string("typed = \"quotes\""),
            fields: vec![RuntimeEffectFieldSeed {
                name: "payload".into(),
                value: string("value = \"kept\""),
            }],
        },
    )];
    if let Some(effect) = after_log {
        ops.push(RuntimeFlowOpSeed::EvaluatedEffect(effect));
    }
    log_plan(ops)
}

fn log_plan(mut ops: Vec<RuntimeFlowOpSeed>) -> (RuntimePlan, FlowRuntimeId) {
    let mut builder = RuntimePlanBuilder::new();
    let string_type = RuntimeCheckedType::String.semantic_identity_digest();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(string_type, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    RuntimeCheckedType::Unit.semantic_identity_digest(),
                    RuntimePlanTypeProjection::Unit,
                ),
            ],
            [],
        )
        .unwrap();
    let flow = FlowRuntimeId::from_source_entity_body("flow.aot_effect").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .unwrap();
    let effects = RuntimeEffectSet::try_from_effects([arcweft_id::EffectId::log_write()]).unwrap();
    ops.push(RuntimeFlowOpSeed::ReturnExpr(string("done")));
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xa8; 32]),
                None,
                Box::new([]),
                string_type,
                effects.clone(),
            ),
            RuntimeExecutableBodySeed {
                effects,
                ops: ops.into_boxed_slice(),
            },
        ))
        .unwrap();
    (builder.finish().unwrap(), flow)
}
