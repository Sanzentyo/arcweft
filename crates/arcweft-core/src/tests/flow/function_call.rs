//! Function-value invocation retains the same fiber across suspension.

use super::*;

#[test]
fn executable_function_value_retains_captures_and_return_binding_across_await() {
    let string = string_type();
    let function = RuntimeSemanticTypeId::from_bytes([0x79; 32]);
    let need_string = RuntimeSemanticTypeId::from_bytes([0x7a; 32]);
    let entry = flow_id("flow.callback_await");
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(string, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    function,
                    RuntimePlanTypeProjection::Function {
                        contract: Default::default(),
                        parameters: Box::new([]),
                        result: string,
                    },
                ),
                RuntimePlanTypeSeed::new(need_string, RuntimePlanTypeProjection::Need(string)),
            ],
            [
                RuntimeLocalDeclarationSeed::new(string),
                RuntimeLocalDeclarationSeed::new(string),
                RuntimeLocalDeclarationSeed::new(need_string),
                RuntimeLocalDeclarationSeed::new(need_string),
            ],
        )
        .expect("callback ABI admits");
    let capture = admission.local_ids()[0].clone();
    let result = admission.local_ids()[1].clone();
    let need_capture = admission.local_ids()[2].clone();
    let need_argument = admission.local_ids()[3].clone();
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            inputs: Box::new([
                RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Capture { position: 0 },
                    input_local: capture.clone(),
                    pattern: RuntimePatternSeed::new(string, RuntimePatternSeedKind::Discard),
                },
                RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Capture { position: 1 },
                    input_local: need_capture.clone(),
                    pattern: RuntimePatternSeed::new(need_string, RuntimePatternSeedKind::Discard),
                },
            ]),
            result: string,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .expect("callback site reserves");
    builder
        .define_function_site_seed(
            &site,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::Await {
                        binding: None,
                        target: RuntimeAwaitTargetSeed {
                            source: RuntimeExprSeed::new(
                                need_string,
                                RuntimeExprSeedKind::Local(need_capture),
                            ),
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
        .push_flow_schema(RuntimeFlowSchema {
            flow: entry.clone(),
            parameters: vec![crate::entry::RuntimeFlowExecutableParameter {
                coordinate: crate::entry::FlowParameterCoordinate::from_position(0),
                name: "pending".to_owned(),
                mode: crate::entry::RuntimeFlowParameterMode::Owned,
                semantic_identity: need_string,
            }],
        })
        .expect("caller schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [need_argument.clone()],
            crate::plan::RuntimeEffectSet::empty(),
            vec![
                RuntimeFlowOpSeed::ApplyFunction {
                    callee: RuntimeExprSeed::new(
                        function,
                        RuntimeExprSeedKind::Function {
                            site,
                            captures: Box::new([
                                string_value("captured"),
                                RuntimeExprSeed::new(
                                    need_string,
                                    RuntimeExprSeedKind::Local(need_argument),
                                ),
                            ]),
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
    let invocation = plan
        .seal_flow_invocation(
            entry,
            [crate::value::RuntimeFlowParameterBinding {
                parameter: crate::entry::FlowParameterCoordinate::from_position(0),
                value: RuntimeValue::Need(NeedId("need.callback".to_owned())),
            }],
        )
        .expect("callback Need argument admits");
    let mut engine = Engine::for_flow_invocation(invocation).expect("caller starts");
    for _ in 0..8 {
        let started = step(&mut engine);
        assert!(started.diagnostics.is_empty(), "{:?}", started.diagnostics);
        if matches!(engine.fiber().status, FlowFiberStatus::NeedWaiting(_)) {
            break;
        }
        assert!(
            matches!(engine.fiber().status, FlowFiberStatus::Running),
            "{:?}",
            engine.fiber().status
        );
    }
    assert!(
        matches!(engine.fiber().status, FlowFiberStatus::NeedWaiting(_)),
        "{:?}",
        engine.fiber().status
    );
    let resumed = engine
        .step(
            RuntimeStepInput {
                need_states: vec![RuntimeNeedState::new(
                    LogicalEpoch(1),
                    NeedId("need.callback".to_owned()),
                    TaskSequence(1),
                    arcweft_need::Need::Ready(crate::value::RuntimePayload::from("ready")),
                )],
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
