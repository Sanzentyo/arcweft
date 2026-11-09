use super::*;
use crate::entry::{
    FlowParameterCoordinate, RuntimeFlowExecutableParameter, RuntimeFlowParameterMode,
};
use crate::pattern::{RuntimeCheckedType, RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::value::{
    RuntimeFlowParameterBinding, RuntimeInputOrder, RuntimeLocalReadMode, RuntimeStoredValue,
};

struct Fixture {
    engine: Engine,
    destination: FlowRuntimeId,
    destination_local: crate::runtime_id::RuntimeLocalDeclarationId,
    escaping: crate::runtime_id::RuntimePresentationHandleToken,
    sibling: crate::runtime_id::RuntimePresentationHandleToken,
}

fn fixture(full_retirement_queue: bool) -> Fixture {
    let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
    let handle = RuntimeSemanticTypeId::from_bytes([0xd3; 32]);
    let tuple = RuntimeSemanticTypeId::from_bytes([0xd4; 32]);
    let handle_owner =
        RuntimeOpaqueTypeOwner::presentation_handle(PresentationResourceKind::View, handle);
    let parameters = [0xd5, 0xd6]
        .map(|marker| RuntimeFunctionParameterIdentity::from_accepted_identity([marker; 32]));
    let flows = ["transfer.source", "transfer.destination"]
        .map(|label| FlowRuntimeId::canonical(label).unwrap());
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    handle,
                    RuntimePlanTypeProjection::Opaque {
                        producer: handle_owner.producer().clone(),
                        admission: handle_owner.admission(),
                        value_class: handle_owner.value_class(),
                        persistence: handle_owner.persistence(),
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    tuple,
                    RuntimePlanTypeProjection::Tuple(Box::new([handle])),
                ),
            ],
            parameters.map(|parameter| {
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Parameter(parameter),
                    tuple,
                )
            }),
        )
        .unwrap();
    for index in 0..2 {
        let local = admitted.local_ids()[index].clone();
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: flows[index].clone(),
                parameters: vec![RuntimeFlowExecutableParameter {
                    identity: parameters[index],
                    coordinate: FlowParameterCoordinate::from_position(0),
                    name: "input".into(),
                    mode: RuntimeFlowParameterMode::Owned,
                    passing: RuntimeFunctionParameterPassing::Affine,
                    semantic_identity: tuple,
                }],
            })
            .unwrap();
        let body = if index == 0 {
            RuntimeFlowOpSeed::Goto(RuntimeFlowTransfer {
                target: RuntimeFlowTransferTarget::Static(flows[1].clone()),
                arguments: Box::new([RuntimeFlowParameterBinding {
                    parameter: FlowParameterCoordinate::from_position(0),
                    value: RuntimeExprSeed::new(
                        tuple,
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            local.clone(),
                            RuntimeLocalReadMode::Move,
                        )),
                    ),
                }]),
            })
        } else {
            RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                boolean,
                RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
            ))
        };
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                flows[index].clone(),
                RuntimeFunctionSiteDeclarationSeed::flow(
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [0xd7 + index as u8; 32],
                    ),
                    None,
                    Box::new([RuntimeFunctionInputBindingSeed {
                        transfer: RuntimeFunctionInputTransfer::Formal,
                        origin: RuntimeFunctionInputOrigin::Parameter(parameters[index]),
                        source: RuntimeFunctionInputSource::Parameter {
                            position: 0,
                            passing: RuntimeFunctionParameterPassing::Affine,
                        },
                        input_local: local.clone(),
                        pattern: RuntimePatternSeed::new(
                            tuple,
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local,
                            },
                        ),
                        ownership: RuntimeValueOwnershipRequirement::Owned,
                        unrestricted_bindings: Box::new([]),
                    }]),
                    boolean,
                    RuntimeEffectSet::empty(),
                ),
                RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([body]),
                },
            ))
            .unwrap();
    }
    let mut plan = builder.finish().unwrap();
    plan.bind_artifact(
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0xd9; 32]).unwrap(),
    )
    .unwrap();
    let mut engine = Engine::new(plan);
    let source = engine.plan.flows()[0].function_site();
    let source_local = engine
        .plan
        .function_sites()
        .get(source)
        .unwrap()
        .parameter_inputs()
        .next()
        .unwrap()
        .input_local();
    let destination_local = engine
        .plan
        .function_sites()
        .get(engine.plan.flows()[1].function_site())
        .unwrap()
        .parameter_inputs()
        .next()
        .unwrap()
        .input_local();
    // The fixture supplies the initial host-owned input before the first step.
    // Its handle is issued by this engine's actual scope and ledger, rather
    // than embedding a foreign handle in a Plan constant.
    engine.fiber.frame.bind_root_function(
        source,
        None,
        (Vec::new(), RuntimeInputOrder::canonical(1)),
    );
    engine.fiber.frame.pending_ops = engine.prepare_function_root_work(source).unwrap();
    engine.fiber.frame.status = FlowFiberStatus::Running;
    engine.main_started = true;
    if full_retirement_queue {
        engine.presentation = Some(crate::presentation::resources::PresentationBanks::new(
            engine.fiber.frame.program.clone(),
            engine.plan.artifact().unwrap(),
            PresentationLedgerLimits {
                handles: 2,
                scopes: 8,
            },
        ));
        for _ in 0..2 {
            let token = engine
                .register_presentation_body(
                    view_site(),
                    boolean,
                    RuntimePayload(RuntimeValue::Bool(false)),
                )
                .unwrap()
                .0;
            let value =
                RuntimeValue::Tuple(vec![handle_owner.try_wrap(token.encode_payload()).unwrap()]);
            engine.discard_pending_return_presentation(&value).unwrap();
        }
    }
    let escaping = engine
        .register_presentation_body(
            view_site(),
            boolean,
            RuntimePayload(RuntimeValue::Bool(true)),
        )
        .unwrap()
        .0;
    engine.fiber.frame.env.set_root(
        source_local,
        RuntimeValue::Tuple(vec![
            handle_owner.try_wrap(escaping.encode_payload()).unwrap(),
        ]),
    );
    let sibling = engine
        .register_presentation_body(
            view_site(),
            boolean,
            RuntimePayload(RuntimeValue::Bool(false)),
        )
        .unwrap()
        .0;
    Fixture {
        engine,
        destination: flows[1].clone(),
        destination_local,
        escaping,
        sibling,
    }
}

#[test]
fn native_flow_transfer_keeps_presentation_inputs_until_the_receiving_flow_ends() {
    let Fixture {
        mut engine,
        destination,
        destination_local,
        escaping,
        sibling,
    } = fixture(false);
    let owner = engine.fiber.frame.program.clone();
    let mut transferred = false;
    let mut complete = false;
    for _ in 0..32 {
        let step = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(step.stats.executed_ops <= 1);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        if step
            .output
            .flow_events
            .iter()
            .any(|event| matches!(event, FlowEvent::Goto { target } if target == &destination))
        {
            assert!(!transferred);
            let banks = engine.presentation.as_ref().unwrap();
            assert_eq!(
                banks
                    .retirements()
                    .iter()
                    .map(|retired| &retired.token)
                    .collect::<Vec<_>>(),
                [&sibling]
            );
            assert!(
                engine
                    .fiber
                    .frame
                    .env
                    .get(destination_local)
                    .unwrap()
                    .borrowed_value()
                    .tuple()
                    .is_some()
            );
            assert!(
                banks
                    .handle_scope(
                        &engine.fiber.frame.program,
                        &escaping,
                        engine.fiber.frame.env.presentation_scope_bindings()
                    )
                    .unwrap()
                    .is_some()
            );
            transferred = true;
        }
        let image = engine.inert_rollback_image().unwrap();
        engine = Engine::from_rollback_image(image, &owner).unwrap();
        if let FlowFiberStatus::Done(FlowExit::Return(value)) = step.fiber_status {
            assert_eq!(value, "true");
            assert_eq!(
                engine
                    .presentation
                    .as_ref()
                    .unwrap()
                    .retirements()
                    .iter()
                    .map(|retired| &retired.token)
                    .collect::<Vec<_>>(),
                [&sibling, &escaping]
            );
            complete = true;
            break;
        }
    }
    assert!(transferred && complete);
}

#[test]
fn rejected_immediate_flow_transfer_receiver_keeps_original_inputs_and_scopes_across_restore() {
    let Fixture {
        mut engine,
        destination,
        escaping,
        sibling,
        ..
    } = fixture(true);
    let owner = engine.fiber.frame.program.clone();
    for _ in 0..2 {
        let before = engine.inert_rollback_image().unwrap();
        let source = engine.plan.flows()[0].function_site();
        let Some(crate::plan::code::RuntimeCodeBodyRef::Operations([FlowOp::Goto(transfer)])) =
            engine.plan.inventory.code_body_at(
                crate::plan::code::RuntimeCodeOwner::Function(source),
                &RuntimeFlowBodyLocation::default(),
            )
        else {
            panic!("the original source owns the transfer")
        };
        let target = RuntimeFlowTransferTarget::Static(destination.clone());
        let inputs = engine
            .fiber
            .frame
            .env
            .prepare_flow_inputs(&engine.plan, &target, transfer)
            .unwrap();
        let error = engine
            .prepare_admitted_flow_root(&inputs.values, &inputs.declaration, inputs.order)
            .err()
            .expect("the receiver cannot publish into a full retirement queue");
        assert!(
            matches!(
                error,
                RuntimeEvalError::Presentation(RuntimePresentationError::RetirementCapacity)
            ),
            "{error:?}"
        );
        assert_eq!(engine.inert_rollback_image().unwrap(), before);
        assert!(engine.fiber.frame.lexical_exit.is_none());
        let banks = engine.presentation.as_ref().unwrap();
        assert_eq!(banks.retirements().len(), 2);
        for token in [&escaping, &sibling] {
            assert!(
                banks
                    .handle_scope(
                        &engine.fiber.frame.program,
                        token,
                        engine.fiber.frame.env.presentation_scope_bindings()
                    )
                    .unwrap()
                    .is_some()
            );
        }
        engine = Engine::from_rollback_image(before, &owner).unwrap();
    }
}
