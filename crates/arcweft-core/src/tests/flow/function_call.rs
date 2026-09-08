//! Function-value invocation retains the same fiber across suspension.

use super::*;

#[test]
fn executable_function_value_retains_captures_and_return_binding_across_await() {
    let string = string_type();
    let function = RuntimeSemanticTypeId::from_bytes([0x79; 32]);
    let entry = flow_id("flow.callback_await");
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(string, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    function,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([]),
                        result: string,
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(string),
                RuntimeLocalDeclarationSeed::new(string),
            ],
            [],
            [],
        )
        .expect("callback ABI admits");
    let capture = admission.local_ids()[0].clone();
    let result = admission.local_ids()[1].clone();
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            inputs: Box::new([RuntimeFunctionInputBindingSeed {
                source: RuntimeFunctionInputSource::Capture { position: 0 },
                input_local: capture.clone(),
                pattern: RuntimePatternSeed::new(string, RuntimePatternSeedKind::Discard),
            }]),
            result: string,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeFunctionEffectSet::empty(),
        })
        .expect("callback site reserves");
    builder
        .define_function_site_seed(
            &site,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeFunctionExecutableBodySeed {
                effects: RuntimeFunctionEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::Await {
                        binding: None,
                        target: RuntimeAwaitTargetSeed {
                            need: NeedId("need.callback".to_owned()),
                            task: TaskId("task.callback".to_owned()),
                            outcome: TaskOutcomeContract::new(
                                crate::pattern::RuntimeCheckedType::String,
                            ),
                            request: RuntimeHostTaskRequestTemplateSeed {
                                capability: HostCapabilityId("test".to_owned()),
                                operation: "callback".to_owned(),
                                args: Vec::new(),
                            },
                        },
                        observers: Vec::new(),
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        string,
                        RuntimeExprSeedKind::Local(capture),
                    )),
                ]),
            }),
        )
        .expect("callback body admits");
    builder
        .push_flow_schema(flow_schema(&entry))
        .expect("caller schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            vec![
                RuntimeFlowOpSeed::ApplyFunction {
                    callee: RuntimeExprSeed::new(
                        function,
                        RuntimeExprSeedKind::Function {
                            site,
                            captures: Box::new([string_value("captured")]),
                        },
                    ),
                    args: Box::new([]),
                    result: RuntimePatternSeed::new(
                        string,
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: result.clone(),
                        },
                    ),
                },
                RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                    string,
                    RuntimeExprSeedKind::Local(result),
                )),
            ],
        ))
        .expect("caller application admits");
    let plan = builder.finish().expect("callback plan seals");
    let mut engine = Engine::for_flow(plan, &entry).expect("caller starts");
    for _ in 0..8 {
        let started = step(&mut engine);
        assert!(started.diagnostics.is_empty(), "{:?}", started.diagnostics);
        if matches!(engine.fiber().status, FlowFiberStatus::Waiting(_)) {
            break;
        }
        assert!(
            matches!(engine.fiber().status, FlowFiberStatus::Running),
            "{:?}",
            engine.fiber().status
        );
    }
    assert!(
        matches!(engine.fiber().status, FlowFiberStatus::Waiting(_)),
        "{:?}",
        engine.fiber().status
    );
    let resumed = engine
        .step(
            RuntimeStepInput {
                task_events: vec![TaskEvent {
                    logical_epoch: LogicalEpoch(1),
                    task_id: TaskId("task.callback".to_owned()),
                    sequence: TaskSequence(1),
                    kind: TaskEventKind::Ready(crate::value::RuntimePayload::from("ready")),
                }],
                ..RuntimeStepInput::default()
            },
            RuntimeStepOptions::default(),
        )
        .output;
    assert!(
        !matches!(engine.fiber().status, FlowFiberStatus::Failed(_)),
        "{:?}: {:?}",
        engine.fiber().status,
        resumed.diagnostics
    );
    let mut events = resumed.flow_events;
    events.extend(drain(&mut engine).flow_events);
    assert_eq!(
        events
            .iter()
            .filter_map(|event| match event {
                FlowEvent::Return { value } => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["captured"]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}
