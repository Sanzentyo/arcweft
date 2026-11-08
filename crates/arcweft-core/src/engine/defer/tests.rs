use super::*;
use crate::engine::NativeCodeCursor;
use crate::entry::RuntimeSchemaLimits;
use crate::plan::{
    FlowRuntimeId, RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeFlowBodyRole,
    RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteDeclarationSeed,
};
use crate::tests::function_application::returning_function_builder;

mod choice;

#[derive(Clone, Copy, PartialEq, Eq)]
enum OrdinaryFlowExit {
    End,
    Fail,
    Return,
    CallReturn,
    FailDuringReturn,
    Goto(GotoCase),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GotoCase {
    Static,
    Dynamic,
    Effect,
    InvalidAbi,
    CleanupFailure,
    Choice,
    ChoiceFailure,
    ChoiceContinue,
}

fn ordinary_flow_defer_engine(exit: OrdinaryFlowExit) -> crate::engine::Engine {
    use crate::effect::{LineEffectRequest, RuntimeLog, RuntimeRegistryReadKind};
    use crate::entry::FlowParameterCoordinate;
    use crate::pattern::RuntimeSemanticTypeId;
    use crate::plan::*;
    use crate::scope::{RuntimeDeferOutcomeFilter as Filter, RuntimeScopeIdentity};
    use crate::value::RuntimeLocalReadMode;
    use arcweft_id::{DeclarationName, EffectId, LifetimeRegistryKey, LifetimeScopeKind};

    let mut builder = RuntimePlanBuilder::new();
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let boolean = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    let string = crate::pattern::RuntimeCheckedType::String.semantic_identity_digest();
    let function_type = RuntimeSemanticTypeId::from_bytes([98; 32]);
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([99; 32]);
    let local_source = crate::engine::manual_local_source(
        "arcweft-core.fixture.engine.defer.flow_scope.parent_binding",
    );
    let RuntimeLocalDeclarationSource::Binding { identity, .. } = local_source else {
        unreachable!("fixture uses a binding source");
    };
    let locals = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(string, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    function_type,
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::default(),
                        parameters: Box::new([]),
                        result: boolean,
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(local_source, boolean),
                RuntimeLocalDeclarationSeed::new(
                    crate::engine::manual_local_source(
                        "arcweft-core.fixture.engine.defer.flow_scope.capture_binding",
                    ),
                    boolean,
                ),
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Parameter(parameter),
                    boolean,
                ),
                RuntimeLocalDeclarationSeed::new(
                    crate::engine::manual_local_source(
                        "arcweft-core.fixture.engine.defer.flow_scope.call_result",
                    ),
                    boolean,
                ),
            ],
        )
        .unwrap();
    let [local, capture, argument, call_result] = locals.local_ids() else {
        panic!("four admitted bindings")
    };
    let bind = |local: &RuntimeLocalSeedId| {
        RuntimePatternSeed::new(
            boolean,
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local: local.clone(),
            },
        )
    };
    let read = |local: &RuntimeLocalSeedId, mode| {
        RuntimeExprSeed::new(
            boolean,
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(local.clone(), mode)),
        )
    };
    let boolean_value = |value| {
        RuntimeExprSeed::new(
            boolean,
            RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)),
        )
    };
    let key = LifetimeRegistryKey::new(
        LifetimeScopeKind::Flow,
        vec![DeclarationName::try_new("completed").unwrap()].into_boxed_slice(),
    );
    let effects = RuntimeEffectSet::try_from_effects([EffectId::state_write(&key)]).unwrap();
    let write = |value| {
        RuntimeFlowOpSeed::EvaluatedEffect(RuntimeEvaluatedEffectSeed::RegistryWrite {
            key: key.clone(),
            value,
        })
    };
    let log = |message: &str| {
        RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Static(LineEffectRequest::Log(
            RuntimeLog {
                level: "info".to_owned(),
                message: message.to_owned(),
                fields: Vec::new(),
            },
        )))
    };
    let mut body = vec![RuntimeFlowOpSeed::Let {
        pattern: bind(local),
        expr: boolean_value(true),
    }];
    let mut inner_defer = None;
    for (index, (message, outcome)) in [
        ("always cleanup", Filter::Always),
        ("completed cleanup", Filter::Completed),
        ("failed cleanup", Filter::Failed),
        ("inner cleanup", Filter::Always),
    ]
    .into_iter()
    .enumerate()
    {
        let writes_flow = outcome == Filter::Completed;
        let mut ops = vec![log(message)];
        if index == 3
            && matches!(
                exit,
                OrdinaryFlowExit::FailDuringReturn
                    | OrdinaryFlowExit::Goto(GotoCase::CleanupFailure)
                    | OrdinaryFlowExit::Goto(GotoCase::ChoiceFailure)
            )
        {
            ops.push(RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Static(
                LineEffectRequest::Fail("inner cleanup failed".to_owned()),
            )));
        }
        let (inputs, callback_effects): (Box<[RuntimeFunctionInputBindingSeed]>, _) = if writes_flow
        {
            ops.push(write(read(capture, RuntimeLocalReadMode::Move)));
            (
                vec![RuntimeFunctionInputBindingSeed {
                    transfer: RuntimeFunctionInputTransfer::Transferred(
                        RuntimeFunctionCaptureMode::Move,
                    ),
                    origin: RuntimeFunctionInputOrigin::Binding(identity),
                    source: RuntimeFunctionInputSource::Capture { position: 0 },
                    input_local: capture.clone(),
                    pattern: bind(capture),
                    ownership: RuntimeValueOwnershipRequirement::Owned,
                    unrestricted_bindings: Box::new([]),
                }]
                .into_boxed_slice(),
                effects.clone(),
            )
        } else {
            (Box::new([]), RuntimeEffectSet::empty())
        };
        let function = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [90 + index as u8; 32],
                ),
                role: RuntimeFunctionSemanticRole::Effect,
                function_type: None,
                inputs,
                result: unit,
                body_kind: RuntimeFunctionSiteBodyKind::Executable,
                effects: callback_effects.clone(),
            })
            .unwrap();
        builder
            .define_function_site_seed(
                &function,
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: callback_effects,
                    ops: ops.into_boxed_slice(),
                }),
            )
            .unwrap();
        let registration = RuntimeFlowOpSeed::RegisterDefer {
            site: builder.reserve_defer_site_seed(&function).unwrap(),
            outcome,
            captures: writes_flow
                .then(|| read(local, RuntimeLocalReadMode::Move))
                .into_iter()
                .collect(),
            owner: RuntimeDeferOwner::CurrentScope,
        };
        if index == 3 {
            inner_defer = Some(registration);
        } else {
            body.push(registration);
        }
    }
    body.push(log("inside"));
    let registry_result = || {
        RuntimeExprSeed::new(
            boolean,
            RuntimeExprSeedKind::RegistryRead {
                key: key.clone(),
                kind: RuntimeRegistryReadKind::Required,
            },
        )
    };
    let nested_transfer = |transfer| {
        let mut body = vec![inner_defer.clone().unwrap(), transfer];
        if exit == OrdinaryFlowExit::Goto(GotoCase::ChoiceContinue) {
            body.push(RuntimeFlowOpSeed::ReturnExpr(boolean_value(true)));
        }
        RuntimeFlowOpSeed::Loop {
            result: None,
            body: vec![RuntimeFlowOpSeed::Scope {
                identity: RuntimeScopeIdentity::Anonymous,
                body,
            }],
        }
    };
    let callee_transfer = match exit {
        OrdinaryFlowExit::CallReturn => Some(RuntimeFlowOpSeed::ReturnExpr(boolean_value(false))),
        OrdinaryFlowExit::Goto(case) => {
            let target = FlowRuntimeId::canonical("test.goto_destination").unwrap();
            let alternate = FlowRuntimeId::canonical("test.goto_alternate").unwrap();
            let needs_argument = FlowRuntimeId::canonical("test.goto_needs_argument").unwrap();
            for (index, (flow, with_argument)) in [
                (&target, false),
                (&alternate, false),
                (&needs_argument, true),
            ]
            .into_iter()
            .enumerate()
            {
                let parameters = with_argument
                    .then(|| RuntimeFlowExecutableParameter {
                        identity: parameter,
                        coordinate: FlowParameterCoordinate::from_position(0),
                        name: "arg".into(),
                        mode: RuntimeFlowParameterMode::Owned,
                        passing: RuntimeFunctionParameterPassing::Value,
                        semantic_identity: boolean,
                    })
                    .into_iter()
                    .collect();
                let inputs = with_argument
                    .then(|| RuntimeFunctionInputBindingSeed {
                        transfer: RuntimeFunctionInputTransfer::Formal,
                        origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                        source: RuntimeFunctionInputSource::Parameter {
                            position: 0,
                            passing: RuntimeFunctionParameterPassing::Value,
                        },
                        input_local: argument.clone(),
                        pattern: bind(argument),
                        ownership: RuntimeValueOwnershipRequirement::Owned,
                        unrestricted_bindings: Box::new([]),
                    })
                    .into_iter()
                    .collect();
                builder
                    .push_flow_schema(RuntimeFlowSchema {
                        flow: flow.clone(),
                        parameters,
                    })
                    .unwrap();
                builder
                    .push_flow_seed(RuntimeFlowSeed::new(
                        flow.clone(),
                        RuntimeFunctionSiteDeclarationSeed::flow(
                            RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                                [100 + index as u8; 32],
                            ),
                            None,
                            inputs,
                            boolean,
                            RuntimeEffectSet::empty(),
                        ),
                        RuntimeExecutableBodySeed {
                            effects: RuntimeEffectSet::empty(),
                            ops: Box::new([
                                log(if index == 0 {
                                    "destination started"
                                } else {
                                    "wrong destination"
                                }),
                                RuntimeFlowOpSeed::ReturnExpr(boolean_value(index == 0)),
                            ]),
                        },
                    ))
                    .unwrap();
            }
            let target_label = |flow: &FlowRuntimeId| {
                RuntimeExprSeed::new(
                    string,
                    RuntimeExprSeedKind::Value(RuntimeValue::String(flow.canonical_label())),
                )
            };
            Some(match case {
                GotoCase::Static | GotoCase::CleanupFailure => RuntimeFlowOpSeed::Goto(
                    crate::plan::RuntimeFlowTransfer::static_without_arguments(target),
                ),
                GotoCase::Dynamic => RuntimeFlowOpSeed::Goto(
                    crate::plan::RuntimeFlowTransfer::dynamic_without_arguments(
                        RuntimeExprSeed::new(
                            string,
                            RuntimeExprSeedKind::If {
                                condition: Box::new(registry_result()),
                                then_expr: Box::new(target_label(&alternate)),
                                else_expr: Box::new(target_label(&target)),
                            },
                        ),
                    ),
                ),
                GotoCase::Effect => RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Static(
                    LineEffectRequest::Goto(target.canonical_label()),
                )),
                GotoCase::InvalidAbi => RuntimeFlowOpSeed::Goto(
                    crate::plan::RuntimeFlowTransfer::dynamic_without_arguments(target_label(
                        &needs_argument,
                    )),
                ),
                GotoCase::Choice | GotoCase::ChoiceFailure | GotoCase::ChoiceContinue => {
                    RuntimeFlowOpSeed::Choice {
                        id: Some("test.choice".into()),
                        options: vec![
                            RuntimeChoiceOptionSeed {
                                id: Some("next".into()),
                                label: "Next".into(),
                                target: (case != GotoCase::ChoiceContinue).then_some(target),
                                out: None,
                                effects: vec![RuntimeLineEffectSeed::Static(
                                    LineEffectRequest::Log(RuntimeLog {
                                        level: "info".into(),
                                        message: "selected".into(),
                                        fields: Vec::new(),
                                    }),
                                )],
                            },
                            RuntimeChoiceOptionSeed {
                                id: Some("alternate".into()),
                                label: "Next".into(),
                                target: Some(alternate),
                                out: None,
                                effects: Vec::new(),
                            },
                        ],
                    }
                }
            })
        }
        _ => None,
    };
    let returned = if let Some(transfer) = callee_transfer {
        let callee = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([97; 32]),
                role: RuntimeFunctionSemanticRole::Closure,
                function_type: None,
                inputs: Box::new([]),
                result: boolean,
                body_kind: RuntimeFunctionSiteBodyKind::Executable,
                effects: RuntimeEffectSet::empty(),
            })
            .unwrap();
        builder
            .define_function_site_seed(
                &callee,
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([nested_transfer(transfer)]),
                }),
            )
            .unwrap();
        body.push(RuntimeFlowOpSeed::ApplyFunction {
            callee: RuntimeExprSeed::new(
                function_type,
                RuntimeExprSeedKind::Function {
                    site: callee,
                    captures: Box::new([]),
                },
            ),
            args: Box::new([]),
            result: bind(call_result),
        });
        body.push(log("caller resumed"));
        body.push(RuntimeFlowOpSeed::ReturnExpr(read(
            call_result,
            RuntimeLocalReadMode::Move,
        )));
        boolean_value(true)
    } else {
        registry_result()
    };
    if matches!(
        exit,
        OrdinaryFlowExit::Return | OrdinaryFlowExit::FailDuringReturn
    ) {
        body.push(nested_transfer(RuntimeFlowOpSeed::ReturnExpr(
            registry_result(),
        )));
    }
    if exit == OrdinaryFlowExit::Fail {
        body.push(RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Static(
            LineEffectRequest::Fail("ordinary Flow failed".to_owned()),
        )));
    }
    let flow = FlowRuntimeId::canonical("test.ordinary_scope_cleanup").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: vec![],
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([96; 32]),
                None,
                Box::new([]),
                boolean,
                effects.clone(),
            ),
            RuntimeExecutableBodySeed {
                effects,
                ops: Box::new([
                    write(boolean_value(false)),
                    RuntimeFlowOpSeed::Scope {
                        identity: RuntimeScopeIdentity::Anonymous,
                        body,
                    },
                    log("after"),
                    RuntimeFlowOpSeed::ReturnExpr(returned),
                ]),
            },
        ))
        .unwrap();
    crate::engine::Engine::for_flow(builder.finish().unwrap(), &flow).unwrap()
}

#[test]
fn ordinary_native_flow_unwinds_defer_and_restores_its_exact_owner() {
    use crate::effect::LineEffectRequest;
    use crate::engine::{Engine, FlowExit, FlowFiberOwner, FlowFiberStatus};
    use crate::step::{RuntimeStepInput, RuntimeStepOptions};
    use crate::time::TickId;

    for fail in [false, true] {
        let mut engine = ordinary_flow_defer_engine(if fail {
            OrdinaryFlowExit::Fail
        } else {
            OrdinaryFlowExit::End
        });
        let owner = RuntimeProgramOwner::Plan(engine.program_plan());
        let mut logs = Vec::new();
        let mut restored_inflight = false;
        let mut complete = false;
        for tick in 0..80 {
            let result = engine.step(
                RuntimeStepInput {
                    tick: TickId(tick),
                    ..RuntimeStepInput::default()
                },
                RuntimeStepOptions::default(),
            );
            logs.extend(result.output.effects.line.into_iter().filter_map(|effect| {
                if let LineEffectRequest::Log(row) = effect {
                    Some(row.message)
                } else {
                    None
                }
            }));
            let image = engine.inert_rollback_image().unwrap();
            if image
                .child_fibers
                .iter()
                .any(|child| matches!(child.owner, FlowFiberOwner::FlowDefer { .. }))
                && !restored_inflight
            {
                let mut foreign = image.clone();
                foreign.flow_defer_frontier =
                    crate::scope::RuntimeDeferRegistrationFrontier::initial();
                assert!(Engine::from_rollback_image(foreign, &owner).is_err());
                let mut foreign = image.clone();
                let scope = foreign
                    .child_fibers
                    .iter_mut()
                    .find_map(|child| match &mut child.owner {
                        FlowFiberOwner::FlowDefer { scope, .. } => Some(scope),
                        _ => None,
                    })
                    .unwrap();
                *scope += 1;
                assert!(Engine::from_rollback_image(foreign, &owner).is_err());
                restored_inflight = true;
            }
            engine = Engine::from_rollback_image(image, &owner).unwrap();
            match result.fiber_status {
                FlowFiberStatus::Done(FlowExit::Return(value)) => {
                    assert!(!fail);
                    assert_eq!(value, "true");
                    assert_eq!(
                        logs,
                        ["inside", "completed cleanup", "always cleanup", "after"]
                    );
                    complete = true;
                    break;
                }
                FlowFiberStatus::Failed(message) => {
                    assert!(fail, "{message}");
                    assert_eq!(message, "ordinary Flow failed");
                    assert_eq!(logs, ["inside", "failed cleanup", "always cleanup"]);
                    complete = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(complete, "{logs:?}");
        assert!(restored_inflight);
    }
}

#[test]
fn ordinary_native_early_return_drains_the_reached_invocation_after_restore() {
    use crate::effect::LineEffectRequest;
    use crate::engine::{Engine, FlowEvent, FlowExit, FlowFiberStatus};
    use crate::step::{RuntimeStepInput, RuntimeStepOptions};
    use crate::time::TickId;

    for (mode, expected_logs) in [
        (
            OrdinaryFlowExit::Return,
            &[
                "inside",
                "inner cleanup",
                "completed cleanup",
                "always cleanup",
            ][..],
        ),
        (
            OrdinaryFlowExit::CallReturn,
            &[
                "inside",
                "inner cleanup",
                "caller resumed",
                "completed cleanup",
                "always cleanup",
            ][..],
        ),
    ] {
        let mut engine = ordinary_flow_defer_engine(mode);
        let owner = RuntimeProgramOwner::Plan(engine.program_plan());
        let mut observed_return = false;
        let mut complete = false;
        let mut logs = Vec::new();
        for tick in 0..80 {
            let step = engine.step(
                RuntimeStepInput {
                    tick: TickId(tick),
                    ..RuntimeStepInput::default()
                },
                RuntimeStepOptions::default(),
            );
            assert!(
                step.output.diagnostics.is_empty(),
                "{:?}",
                step.output.diagnostics
            );
            logs.extend(
                step.output
                    .effects
                    .line
                    .into_iter()
                    .filter_map(|effect| match effect {
                        LineEffectRequest::Log(row) => Some(row.message),
                        _ => None,
                    }),
            );
            let image = engine.inert_rollback_image().unwrap();
            if !observed_return
                && !image.child_fibers.is_empty()
                && let Some(NativeLexicalExitAction::Return { result, .. }) = image
                    .fiber
                    .frame
                    .lexical_exit
                    .as_ref()
                    .map(|exit| &exit.action)
            {
                assert!(
                    !step
                        .output
                        .flow_events
                        .iter()
                        .any(|event| matches!(event, FlowEvent::Return { .. }))
                );
                let mut wrong_target = image.clone();
                wrong_target
                    .fiber
                    .frame
                    .lexical_exit
                    .as_mut()
                    .unwrap()
                    .target += 1;
                assert!(Engine::from_rollback_image(wrong_target, &owner).is_err());
                if mode == OrdinaryFlowExit::CallReturn {
                    assert!(matches!(result, NativeReturnResult::Value(_)));
                    let mut wrong_value = image.clone();
                    let NativeLexicalExitAction::Return { result, .. } = &mut wrong_value
                        .fiber
                        .frame
                        .lexical_exit
                        .as_mut()
                        .unwrap()
                        .action
                    else {
                        unreachable!()
                    };
                    *result = NativeReturnResult::Value(
                        AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                            &RuntimeValue::Unit,
                            &owner,
                        )
                        .unwrap(),
                    );
                    assert!(Engine::from_rollback_image(wrong_value, &owner).is_err());
                }
                observed_return = true;
            }
            engine = Engine::from_rollback_image(image, &owner).unwrap();
            if let FlowFiberStatus::Done(FlowExit::Return(value)) = step.fiber_status {
                assert_eq!(value, "false");
                assert_eq!(logs, expected_logs);
                assert!(
                    step.output.flow_events.iter().any(
                        |event| matches!(event, FlowEvent::Return { value } if value == "false")
                    )
                );
                complete = true;
                break;
            }
        }
        assert!(observed_return && complete, "{logs:?}");
    }
}

#[test]
fn ordinary_native_return_is_discarded_when_inner_cleanup_fails() {
    use crate::effect::LineEffectRequest;
    use crate::engine::{Engine, FlowEvent, FlowFiberStatus};
    use crate::step::{RuntimeStepInput, RuntimeStepOptions};
    use crate::time::TickId;

    let mut engine = ordinary_flow_defer_engine(OrdinaryFlowExit::FailDuringReturn);
    let owner = RuntimeProgramOwner::Plan(engine.program_plan());
    let mut logs = Vec::new();
    for tick in 0..80 {
        let step = engine.step(
            RuntimeStepInput {
                tick: TickId(tick),
                ..RuntimeStepInput::default()
            },
            RuntimeStepOptions::default(),
        );
        assert!(
            !step
                .output
                .flow_events
                .iter()
                .any(|event| matches!(event, FlowEvent::Return { .. }))
        );
        logs.extend(
            step.output
                .effects
                .line
                .into_iter()
                .filter_map(|effect| match effect {
                    LineEffectRequest::Log(row) => Some(row.message),
                    _ => None,
                }),
        );
        engine =
            Engine::from_rollback_image(engine.inert_rollback_image().unwrap(), &owner).unwrap();
        if let FlowFiberStatus::Failed(message) = step.fiber_status {
            assert_eq!(message, "inner cleanup failed");
            assert_eq!(
                logs,
                [
                    "inside",
                    "inner cleanup",
                    "failed cleanup",
                    "always cleanup"
                ]
            );
            assert!(engine.fiber.frame.lexical_exit.is_none());
            return;
        }
    }
    panic!("cleanup failure must discard the pending return: {logs:?}");
}

#[test]
fn native_goto_finishes_inner_and_caller_cleanup_before_transferring() {
    use crate::effect::LineEffectRequest;
    use crate::engine::{Engine, FlowEvent, FlowExit, FlowFiberStatus};
    use crate::step::{RuntimeStepInput, RuntimeStepOptions};
    use crate::time::TickId;

    for case in [GotoCase::Static, GotoCase::Dynamic, GotoCase::Effect] {
        let mut engine = ordinary_flow_defer_engine(OrdinaryFlowExit::Goto(case));
        let owner = RuntimeProgramOwner::Plan(engine.program_plan());
        let destination = FlowRuntimeId::canonical("test.goto_destination").unwrap();
        let mut logs = Vec::new();
        let mut observed_inside_call = false;
        let mut observed_after_call = false;
        let mut transferred = false;
        let mut finished = false;
        for tick in 0..80 {
            let step = engine.step(
                RuntimeStepInput {
                    tick: TickId(tick),
                    ..RuntimeStepInput::default()
                },
                RuntimeStepOptions::default(),
            );
            assert!(
                step.output.diagnostics.is_empty(),
                "{:?}",
                step.output.diagnostics
            );
            assert!(
                !matches!(step.fiber_status, FlowFiberStatus::Failed(_)),
                "{:?}",
                step.fiber_status
            );
            let effect = step
                .output
                .effects
                .line
                .iter()
                .any(|effect| matches!(effect, LineEffectRequest::Goto(_)));
            logs.extend(
                step.output
                    .effects
                    .line
                    .into_iter()
                    .filter_map(|effect| match effect {
                        LineEffectRequest::Log(row) => Some(row.message),
                        _ => None,
                    }),
            );
            let goto_events = step
                .output
                .flow_events
                .iter()
                .filter_map(|event| match event {
                    FlowEvent::Goto { target } => Some(target),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(effect, !goto_events.is_empty() && case == GotoCase::Effect);
            if !goto_events.is_empty() {
                assert_eq!(goto_events, [&destination]);
                assert!(!transferred);
                assert_eq!(
                    logs,
                    [
                        "inside",
                        "inner cleanup",
                        "completed cleanup",
                        "always cleanup"
                    ]
                );
                assert!(
                    engine
                        .fiber
                        .frame
                        .env
                        .flow_registry()
                        .is_some_and(|registry| registry.is_empty())
                );
                transferred = true;
            }
            let image = engine.inert_rollback_image().unwrap();
            if let Some(exit) = &image.fiber.frame.lexical_exit
                && let NativeLexicalExitAction::Goto { transfer, .. } = &exit.action
            {
                assert_eq!(
                    transfer.target,
                    crate::plan::RuntimeFlowTransferTarget::Static(destination.clone())
                );
                assert!(!transferred);
                if engine.fiber.frame.active_function().is_some() {
                    if !observed_inside_call {
                        let mut wrong_scope = image.clone();
                        wrong_scope
                            .fiber
                            .frame
                            .lexical_exit
                            .as_mut()
                            .unwrap()
                            .target += 1;
                        assert!(Engine::from_rollback_image(wrong_scope, &owner).is_err());
                        let mut wrong_destination = image.clone();
                        let NativeLexicalExitAction::Goto { transfer, .. } = &mut wrong_destination
                            .fiber
                            .frame
                            .lexical_exit
                            .as_mut()
                            .unwrap()
                            .action
                        else {
                            unreachable!()
                        };
                        transfer.target = crate::plan::RuntimeFlowTransferTarget::Static(
                            FlowRuntimeId::canonical(if case == GotoCase::Dynamic {
                                "test.goto_needs_argument"
                            } else {
                                "test.goto_alternate"
                            })
                            .unwrap(),
                        );
                        assert!(Engine::from_rollback_image(wrong_destination, &owner).is_err());
                    }
                    observed_inside_call = true;
                } else {
                    observed_after_call = true;
                }
            }
            engine = Engine::from_rollback_image(image, &owner).unwrap();
            if let FlowFiberStatus::Done(FlowExit::Return(value)) = step.fiber_status {
                assert_eq!(value, "true");
                assert_eq!(
                    logs,
                    [
                        "inside",
                        "inner cleanup",
                        "completed cleanup",
                        "always cleanup",
                        "destination started"
                    ]
                );
                finished = true;
                break;
            }
        }
        assert!(
            finished && transferred && observed_inside_call && observed_after_call,
            "{logs:?}"
        );
    }
}

#[test]
fn native_goto_in_a_call_waits_for_joined_work_at_its_original_position() {
    use crate::effect::LineEffectRequest;
    use crate::engine::{Engine, FlowEvent, FlowExit, FlowFiberStatus};
    use crate::step::{RuntimeStepInput, RuntimeStepOptions};

    let mut engine = ordinary_flow_defer_engine(OrdinaryFlowExit::Goto(GotoCase::Static));
    let owner = RuntimeProgramOwner::Plan(engine.program_plan());
    let mut child_id = None;
    let mut finished = false;
    let mut joined_polls = 0;
    let mut logs = Vec::new();
    for _ in 0..80 {
        if child_id.is_none()
            && matches!(
                engine.active_flow_operations().next(),
                Some(crate::plan::FlowOp::Goto(_))
            )
        {
            assert!(engine.fiber.frame.active_function().is_some());
            let flow = FlowRuntimeId::canonical("test.goto_alternate").unwrap();
            let site = engine
                .plan
                .validate_flow_invocation(&flow, &[])
                .unwrap()
                .site();
            let position = NativeCodeCursor::root(
                &engine.plan,
                crate::plan::code::RuntimeCodeOwner::Function(site),
            )
            .unwrap();
            let work = engine.prepare_task_root_work(position, false).unwrap();
            let (child, next) = engine.prepare_child_fiber(work, &[]).unwrap();
            child_id = Some(child.id);
            engine.next_fiber_id = next;
            engine.child_fibers.push_back(child);
            engine.run_child_next = false;
        }
        let joining =
            child_id.is_some_and(|id| engine.child_fibers.iter().any(|child| child.id == id));
        if joining {
            assert!(!engine.main_fiber_can_attempt_runtime_op());
            assert!(matches!(
                engine.active_flow_operations().next(),
                Some(crate::plan::FlowOp::Goto(_))
            ));
        }
        let step = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        logs.extend(
            step.output
                .effects
                .line
                .into_iter()
                .filter_map(|effect| match effect {
                    LineEffectRequest::Log(row) => Some(row.message),
                    _ => None,
                }),
        );
        if joining {
            assert_eq!(step.stats.executed_ops, 1);
            assert!(engine.fiber.frame.lexical_exit.is_none());
            assert!(matches!(
                engine.active_flow_operations().next(),
                Some(crate::plan::FlowOp::Goto(_))
            ));
            assert!(
                !step
                    .output
                    .flow_events
                    .iter()
                    .any(|event| matches!(event, FlowEvent::Goto { .. }))
            );
            joined_polls += 1;
        }
        engine =
            Engine::from_rollback_image(engine.inert_rollback_image().unwrap(), &owner).unwrap();
        if let FlowFiberStatus::Done(FlowExit::Return(value)) = step.fiber_status {
            assert_eq!(value, "true");
            assert_eq!(
                logs,
                [
                    "inside",
                    "wrong destination",
                    "inner cleanup",
                    "completed cleanup",
                    "always cleanup",
                    "destination started"
                ]
            );
            finished = true;
            break;
        }
    }
    assert!(
        finished && joined_polls == 3,
        "expected scope entry, log, and child return before goto, observed {joined_polls}: {logs:?}"
    );
}

#[test]
fn native_goto_uses_failure_cleanup_without_publishing_an_unaccepted_transfer() {
    use crate::effect::LineEffectRequest;
    use crate::engine::{Engine, FlowEvent, FlowFiberStatus};
    use crate::step::{RuntimeStepInput, RuntimeStepOptions};
    use crate::time::TickId;

    for case in [GotoCase::InvalidAbi, GotoCase::CleanupFailure] {
        let mut engine = ordinary_flow_defer_engine(OrdinaryFlowExit::Goto(case));
        let owner = RuntimeProgramOwner::Plan(engine.program_plan());
        let mut logs = Vec::new();
        let mut reached_goto = false;
        let mut failed = false;
        for tick in 0..80 {
            let step = engine.step(
                RuntimeStepInput {
                    tick: TickId(tick),
                    ..RuntimeStepInput::default()
                },
                RuntimeStepOptions::default(),
            );
            assert!(
                !step
                    .output
                    .flow_events
                    .iter()
                    .any(|event| matches!(event, FlowEvent::Goto { .. }))
            );
            logs.extend(
                step.output
                    .effects
                    .line
                    .into_iter()
                    .filter_map(|effect| match effect {
                        LineEffectRequest::Goto(_) => {
                            panic!("an unaccepted transfer cannot emit goto")
                        }
                        LineEffectRequest::Log(row) => Some(row.message),
                        _ => None,
                    }),
            );
            reached_goto |= engine
                .fiber
                .frame
                .lexical_exit
                .as_ref()
                .is_some_and(|exit| matches!(exit.action, NativeLexicalExitAction::Goto { .. }));
            engine = Engine::from_rollback_image(engine.inert_rollback_image().unwrap(), &owner)
                .unwrap();
            if let FlowFiberStatus::Failed(message) = step.fiber_status {
                assert_eq!(
                    logs,
                    [
                        "inside",
                        "inner cleanup",
                        "failed cleanup",
                        "always cleanup"
                    ]
                );
                assert!(engine.fiber.frame.lexical_exit.is_none());
                assert_eq!(reached_goto, case == GotoCase::CleanupFailure);
                if case == GotoCase::CleanupFailure {
                    assert_eq!(message, "inner cleanup failed");
                } else {
                    assert!(message.contains("test.goto_needs_argument"), "{message}");
                }
                failed = true;
                break;
            }
        }
        assert!(failed, "{logs:?}");
    }
}

#[test]
fn ordinary_native_flow_reports_cleanup_start_failure_after_draining_scope() {
    use crate::engine::FlowFiberStatus;
    use crate::step::{RuntimeStepInput, RuntimeStepOptions};
    use crate::time::TickId;
    use crate::value::RuntimeEvalError;

    let mut engine = ordinary_flow_defer_engine(OrdinaryFlowExit::End);
    engine.next_fiber_id = u64::MAX;
    for tick in 0..80 {
        let result = engine.step(
            RuntimeStepInput {
                tick: TickId(tick),
                ..RuntimeStepInput::default()
            },
            RuntimeStepOptions::default(),
        );
        if let FlowFiberStatus::Failed(message) = result.fiber_status {
            assert!(message.contains(&RuntimeEvalError::FiberIdentityOverflow.to_string()));
            assert!(!engine.fiber.frame.has_scope_defer_work());
            assert!(engine.child_fibers.is_empty());
            assert_eq!(result.output.diagnostics.len(), 1);
            return;
        }
    }
    panic!("exhausted child identities must fail the Flow after cleanup drains");
}

#[test]
fn native_loop_transfer_image_requires_the_original_loop_and_value() {
    let mut builder = returning_function_builder(RuntimeFunctionSiteBodyKind::Executable);
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let flow = FlowRuntimeId::canonical("test.lexical_transfer").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: vec![],
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x79; 32]),
                None,
                Box::new([]),
                unit,
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::Loop {
                    result: None,
                    body: vec![RuntimeFlowOpSeed::Loop {
                        result: None,
                        body: vec![RuntimeFlowOpSeed::Break {
                            depth: 1,
                            value: None,
                        }],
                    }],
                }]),
            },
        ))
        .unwrap();
    let plan = std::sync::Arc::new(builder.finish().unwrap());
    let site = plan.validate_flow_invocation(&flow, &[]).unwrap().site();
    let owner = RuntimeProgramOwner::Plan(plan.clone());
    let admission = || {
        RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            RuntimeSchemaLimits::engine_default(),
        )
    };
    let outer = NativeCodeCursor::root(&plan, crate::plan::code::RuntimeCodeOwner::Function(site))
        .unwrap()
        .source_operation(&plan)
        .unwrap();
    let inner = outer
        .child_body(RuntimeFlowBodyRole::Body, &mut admission())
        .unwrap()
        .source_operation(&plan)
        .unwrap();
    let source = inner
        .child_body(RuntimeFlowBodyRole::Body, &mut admission())
        .unwrap()
        .source_operation(&plan)
        .unwrap();
    let exit = NativeLexicalExit {
        target: 0,
        action: NativeLexicalExitAction::Transfer(NativeLoopTransfer {
            source,
            target: outer,
            kind: NativeLoopTransferKind::Break(RuntimeValue::Unit),
        }),
    };
    exit.admit(&mut admission(), None).unwrap();
    let image = exit.snapshot(&owner).unwrap();
    image.admit(&mut admission(), None).unwrap();
    assert_eq!(image.clone().into_live(&owner).unwrap(), exit);

    let mut wrong_target = image.clone();
    let NativeLexicalExitAction::Transfer(transfer) = &mut wrong_target.action else {
        panic!("image contains its suspended loop transfer");
    };
    transfer.target = inner;
    assert!(wrong_target.admit(&mut admission(), None).is_err());

    let mut wrong_value = image.clone();
    let NativeLexicalExitAction::Transfer(transfer) = &mut wrong_value.action else {
        panic!("image contains its suspended loop transfer");
    };
    transfer.kind = NativeLoopTransferKind::Break(
        AwbcRuntimeValueSnapshot::from_admitted_runtime_value(&RuntimeValue::Bool(true), &owner)
            .unwrap(),
    );
    assert!(wrong_value.admit(&mut admission(), None).is_err());
    assert_eq!(image.into_live(&owner).unwrap(), exit);
}
