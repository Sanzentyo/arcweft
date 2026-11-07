use super::*;
use crate::engine::NativeCodeCursor;
use crate::entry::RuntimeSchemaLimits;
use crate::plan::{
    FlowRuntimeId, RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeFlowBodyRole,
    RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteDeclarationSeed,
};
use crate::tests::function_application::returning_function_builder;

#[derive(Clone, Copy, PartialEq, Eq)]
enum OrdinaryFlowExit {
    End,
    Fail,
    Return,
    CallReturn,
    FailDuringReturn,
}

fn ordinary_flow_defer_engine(exit: OrdinaryFlowExit) -> crate::engine::Engine {
    use crate::effect::{LineEffectRequest, RuntimeLog, RuntimeRegistryReadKind};
    use crate::pattern::RuntimeSemanticTypeId;
    use crate::plan::*;
    use crate::scope::{RuntimeDeferOutcomeFilter as Filter, RuntimeScopeIdentity};
    use crate::value::RuntimeLocalReadMode;
    use arcweft_id::{DeclarationName, EffectId, LifetimeRegistryKey, LifetimeScopeKind};

    let mut builder = RuntimePlanBuilder::new();
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let boolean = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    let function_type = RuntimeSemanticTypeId::from_bytes([98; 32]);
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
                    crate::engine::manual_local_source(
                        "arcweft-core.fixture.engine.defer.flow_scope.call_result",
                    ),
                    boolean,
                ),
            ],
        )
        .unwrap();
    let [local, capture, call_result] = locals.local_ids() else {
        panic!("three admitted bindings")
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
        if index == 3 && exit == OrdinaryFlowExit::FailDuringReturn {
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
    let nested_return = |value| RuntimeFlowOpSeed::Loop {
        result: None,
        body: vec![RuntimeFlowOpSeed::Scope {
            identity: RuntimeScopeIdentity::Anonymous,
            body: vec![
                inner_defer.clone().unwrap(),
                RuntimeFlowOpSeed::ReturnExpr(value),
            ],
        }],
    };
    let returned = if exit == OrdinaryFlowExit::CallReturn {
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
                    ops: Box::new([nested_return(boolean_value(false))]),
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
        body.push(nested_return(registry_result()));
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
