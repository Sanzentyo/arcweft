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
                RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.tests.flow.function_call.executable_function_value_retains_captures_and_return_binding_across_await.binding_a"), string),
                RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.tests.flow.function_call.executable_function_value_retains_captures_and_return_binding_across_await.binding_b"), string),
                RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.tests.flow.function_call.executable_function_value_retains_captures_and_return_binding_across_await.binding_c"), need_string),
                RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.tests.flow.function_call.executable_function_value_retains_captures_and_return_binding_across_await.binding_d"), need_string),
                RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.tests.flow.function_call.executable_function_value_retains_captures_and_return_binding_across_await.binding_e"), string),
                RuntimeLocalDeclarationSeed::new(manual_local_origin("arcweft-core.fixture.tests.flow.function_call.executable_function_value_retains_captures_and_return_binding_across_await.binding_f"), need_string),
            ],
        )
        .expect("callback ABI admits");
    let capture = admission.local_ids()[0].clone();
    let result = admission.local_ids()[1].clone();
    let need_capture = admission.local_ids()[2].clone();
    let need_argument = admission.local_ids()[3].clone();
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                [41; 32],
            ),
            role: crate::plan::RuntimeFunctionSemanticRole::Closure,
            function_type: None,
            inputs: Box::new([
                RuntimeFunctionInputBindingSeed {
                    transfer: crate::plan::RuntimeFunctionInputTransfer::Transferred(
                        crate::plan::RuntimeFunctionCaptureMode::Move,
                    ),
                    origin: crate::plan::RuntimeFunctionInputOrigin::Binding([81; 32]),
                    ownership: Default::default(),
                    unrestricted_bindings: Box::new([]),
                    source: RuntimeFunctionInputSource::Capture { position: 0 },
                    input_local: admission.local_ids()[4].clone(),
                    pattern: RuntimePatternSeed::new(
                        string,
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: capture.clone(),
                        },
                    ),
                },
                RuntimeFunctionInputBindingSeed {
                    transfer: crate::plan::RuntimeFunctionInputTransfer::Transferred(
                        crate::plan::RuntimeFunctionCaptureMode::Move,
                    ),
                    origin: crate::plan::RuntimeFunctionInputOrigin::Binding([81; 32]),
                    ownership: Default::default(),
                    unrestricted_bindings: Box::new([]),
                    source: RuntimeFunctionInputSource::Capture { position: 1 },
                    input_local: admission.local_ids()[5].clone(),
                    pattern: RuntimePatternSeed::new(
                        need_string,
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: need_capture.clone(),
                        },
                    ),
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
                                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                    need_capture,
                                    RuntimeLocalReadMode::Move,
                                )),
                            ),
                        },
                        observers: Vec::new(),
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        string,
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            capture,
                            RuntimeLocalReadMode::Copy,
                        )),
                    )),
                ]),
            }),
        )
        .expect("callback body admits");
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: entry.clone(),
            parameters: vec![crate::entry::RuntimeFlowExecutableParameter {
                identity: crate::plan::RuntimeFunctionParameterIdentity::from_accepted_identity(
                    [92; 32],
                ),
                coordinate: crate::entry::FlowParameterCoordinate::from_position(0),
                name: "pending".to_owned(),
                mode: crate::entry::RuntimeFlowParameterMode::Owned,
                passing: crate::plan::RuntimeFunctionParameterPassing::Affine,
                semantic_identity: need_string,
            }],
        })
        .expect("caller schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]),
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
                                    RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                        need_argument,
                                        RuntimeLocalReadMode::Move,
                                    )),
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
                    RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                        result,
                        RuntimeLocalReadMode::Copy,
                    )),
                )),
            ],
        ))
        .expect("caller application admits");
    let plan = builder.finish().expect("callback plan seals");
    let (registry, need) = crate::tests::pending_need(string);
    let invocation = plan
        .seal_flow_invocation(
            entry,
            [crate::value::RuntimeFlowParameterBinding {
                parameter: crate::entry::FlowParameterCoordinate::from_position(0),
                value: RuntimeValue::NeedHandle(need.clone()),
            }],
        )
        .expect("callback Need argument admits");
    let mut engine = Engine::for_flow_invocation_with_need_context(
        invocation,
        crate::task::GenerationId::new(0),
        registry,
    )
    .expect("caller starts");
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
                task_events: vec![crate::task::TaskEvent {
                    correlation: need.correlation(),
                    cursor: crate::task::TaskPublicationCursor {
                        logical_epoch: LogicalEpoch(1),
                        sequence: TaskSequence(1),
                    },
                    kind: crate::task::TaskEventKind::Ready(crate::value::RuntimePayload::from(
                        "ready",
                    )),
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

fn manual_local_origin(declaration: &str) -> crate::plan::RuntimeLocalOrigin {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    crate::plan::RuntimeLocalOrigin::Binding(*identity.finalize().as_bytes())
}
