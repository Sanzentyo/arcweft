use crate::engine::{Engine, FlowExit, FlowFiberStatus, FlowFiberStatusRollbackImage};
use crate::pattern::RuntimeCheckedType;
use crate::plan::{
    FlowRuntimeId, RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeExprSeed,
    RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed,
    RuntimeFunctionDefinitionIdentity, RuntimeFunctionSiteDeclarationSeed,
    RuntimeHostCallTargetSeed, RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind,
    RuntimeLocalBindingStorage, RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource,
    RuntimeLocalReadSeed, RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use crate::step::{
    RuntimeHostCallMode, RuntimeHostCallResult, RuntimeStepBudget, RuntimeStepInput,
    RuntimeStepMode, RuntimeStepOptions,
};
use crate::task::{
    HostCallProducerDefinition, NeedProducerContractDigest, NeedProducerSiteDigest,
    RuntimeProgramOwner, TaskCorrelation, TaskLaunchOrdinal,
};
use crate::value::{RuntimeLocalReadMode, RuntimePayload, RuntimeValue};

fn host_call_flow(bound: bool) -> Engine {
    let semantic = RuntimeCheckedType::String.semantic_identity_digest();
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                semantic,
                RuntimePlanTypeProjection::String,
            )],
            bound.then(|| {
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Binding {
                        identity: [0x7a; 32],
                        declaration: RuntimeLocalBindingDeclaration::new(
                            RuntimeLocalBindingKind::PatternBinding,
                            false,
                            RuntimeLocalBindingStorage::Derived,
                        ),
                    },
                    semantic,
                )
            }),
        )
        .unwrap();
    let local = admitted.local_ids().first();
    let flow = FlowRuntimeId::canonical("host.probe").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .unwrap();
    let plan = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    let binding = local.map(|local| {
        RuntimePatternSeed::new(
            semantic,
            RuntimePatternSeedKind::Bind {
                local: local.clone(),
                mutable: false,
            },
        )
    });
    let result = local.map_or_else(
        || RuntimeExprSeedKind::Value(RuntimeValue::String("unbound".into())),
        |local| {
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                local.clone(),
                RuntimeLocalReadMode::Move,
            ))
        },
    );
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x7b; 32]),
                None,
                Box::new([]),
                semantic,
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::HostCall {
                        binding,
                        target: RuntimeHostCallTargetSeed {
                            producer: HostCallProducerDefinition {
                                contract: NeedProducerContractDigest::from_bytes([0x7c; 32]),
                                plan,
                                site: NeedProducerSiteDigest::from_bytes([0x7d; 32]),
                            },
                            public_id: "host.probe".into(),
                            capability: "host".into(),
                            operation: "probe".into(),
                            contract: None,
                            args: Vec::new(),
                            result: semantic,
                            mode: RuntimeHostCallMode::Suspend,
                            deterministic: false,
                        },
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(semantic, result)),
                ]),
            },
        ))
        .unwrap();
    Engine::for_flow(builder.finish().expect("valid plan"), &flow).unwrap()
}

#[test]
fn native_host_call_restores_issued_request_and_original_result_binding() {
    for bound in [false, true] {
        let mut engine = host_call_flow(bound);
        engine.next_host_call_sequence = 7;
        let owner = RuntimeProgramOwner::Plan(engine.program_plan());
        let drain = RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            ..Default::default()
        };
        let started = engine.step(Default::default(), drain);
        assert!(started.output.diagnostics.is_empty(), "{started:?}");
        let [request] = started.output.requests.host_calls.as_slice() else {
            panic!("one host request")
        };
        let id = request.id.clone();
        assert_eq!(id.0, "host.probe.7");

        let mut image = engine.inert_rollback_image().unwrap();
        let wire = serde_json::to_value(&image.fiber.frame.status).unwrap();
        image.fiber.frame.status = serde_json::from_value(wire.clone()).unwrap();
        let mut with_binding = wire;
        with_binding["HostCall"]["binding"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<FlowFiberStatusRollbackImage>(with_binding).is_err());

        let mut forged = image.clone();
        let FlowFiberStatus::HostCall(state) = &mut forged.fiber.frame.status else {
            unreachable!()
        };
        state.sequence += 1;
        assert!(Engine::from_rollback_image(forged, &owner).is_err());
        let mut with_unissued_task = image.clone();
        let FlowFiberStatus::HostCall(state) = &mut with_unissued_task.fiber.frame.status else {
            unreachable!()
        };
        let spec = request.submission().spec();
        let ordinal = TaskLaunchOrdinal::try_for_policy(
            spec.policy,
            state.correlation.launch_ordinal.get() + 1,
        )
        .unwrap();
        state.correlation =
            TaskCorrelation::try_for(spec.generation, &spec.producer, spec.policy, ordinal)
                .unwrap();
        let rejected = Engine::from_rollback_image(with_unissued_task, &owner).unwrap_err();
        assert!(rejected.contains("issued task receipt"), "{rejected}");

        let reply = |value| RuntimeStepInput {
            host_call_results: vec![RuntimeHostCallResult {
                id: id.clone(),
                outcome: Ok(RuntimePayload(value)),
            }],
            ..Default::default()
        };
        let ingress = RuntimeStepOptions {
            budget: RuntimeStepBudget { max_ops: 0 },
            ..Default::default()
        };
        let mut restored = Engine::from_rollback_image(image.clone(), &owner).unwrap();
        let rejected = restored.step(reply(RuntimeValue::Bool(true)), ingress);
        assert!(matches!(rejected.fiber_status, FlowFiberStatus::Failed(_)));
        assert!(!rejected.output.diagnostics.is_empty());
        assert!(rejected.output.flow_events.is_empty());

        let mut restored = Engine::from_rollback_image(image, &owner).unwrap();
        let resumed = restored.step(reply(RuntimeValue::String("from host".into())), ingress);
        assert_eq!(resumed.stats.executed_ops, 0);
        assert!(resumed.output.diagnostics.is_empty(), "{resumed:?}");
        assert!(resumed.output.requests.host_calls.is_empty());
        let finished = restored.step(Default::default(), drain);
        assert_eq!(
            finished.fiber_status,
            FlowFiberStatus::Done(FlowExit::Return(
                if bound { "from host" } else { "unbound" }.into()
            ))
        );
        assert!(finished.output.requests.host_calls.is_empty());
    }
}
