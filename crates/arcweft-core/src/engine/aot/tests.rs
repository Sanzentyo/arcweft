use crate::{
    aot::AotProgram,
    effect::LineEffectRequest,
    engine::{Engine, FlowExit, FlowFiberStatus},
    executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor},
    pattern::RuntimeCheckedType,
    plan::{
        FlowRuntimeId, RuntimeDropPolicySeed, RuntimeEffectFieldSeed, RuntimeEffectSet,
        RuntimeEvaluatedEffectSeed, RuntimeExecutableBodySeed, RuntimeExprSeed,
        RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed,
        RuntimeFunctionDefinitionIdentity, RuntimeFunctionSiteDeclarationSeed, RuntimePlan,
        RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
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
