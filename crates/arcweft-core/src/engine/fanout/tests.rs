use super::*;
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::task::*;
use crate::value::{RuntimeLocalReadMode, RuntimeSeq};

fn fixture(items: &[bool], limit: u32) -> Engine {
    let mut builder = RuntimePlanBuilder::new();
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
    let unsigned = RuntimeCheckedType::Unsigned(crate::value::RuntimeUnsignedIntWidth::U32)
        .semantic_identity_digest();
    let vector = RuntimeSemanticTypeId::from_bytes([0xb1; 32]);
    let need_vector = RuntimeSemanticTypeId::from_bytes([0xb2; 32]);
    let need_bool = RuntimeSemanticTypeId::from_bytes([0xb3; 32]);
    let batch = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    unsigned,
                    RuntimePlanTypeProjection::Unsigned(crate::value::RuntimeUnsignedIntWidth::U32),
                ),
                RuntimePlanTypeSeed::new(
                    vector,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: boolean,
                    },
                ),
                RuntimePlanTypeSeed::new(need_vector, RuntimePlanTypeProjection::Need(vector)),
                RuntimePlanTypeSeed::new(need_bool, RuntimePlanTypeProjection::Need(boolean)),
            ],
            [vector, unsigned, need_vector, boolean, vector]
                .into_iter()
                .enumerate()
                .map(|(index, ty)| {
                    RuntimeLocalDeclarationSeed::new(
                        RuntimeLocalDeclarationSource::Binding {
                            identity: [0xb4 + index as u8; 32],
                            declaration: RuntimeLocalBindingDeclaration::new(
                                RuntimeLocalBindingKind::PatternBinding,
                                false,
                                RuntimeLocalBindingStorage::Derived,
                            ),
                        },
                        ty,
                    )
                }),
        )
        .unwrap();
    let source = batch.local_ids()[0].clone();
    let bound = batch.local_ids()[1].clone();
    let handle = batch.local_ids()[2].clone();
    let item = batch.local_ids()[3].clone();
    let result = batch.local_ids()[4].clone();
    let pattern = |ty, local: &RuntimeLocalSeedId| {
        RuntimePatternSeed::new(
            ty,
            RuntimePatternSeedKind::Bind {
                local: local.clone(),
                mutable: false,
            },
        )
    };
    let read = |ty, local: &RuntimeLocalSeedId| {
        RuntimeExprSeed::new(
            ty,
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                local.clone(),
                RuntimeLocalReadMode::Move,
            )),
        )
    };
    let base_coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    let child_coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    let producer =
        |coordinate, family, payload: RuntimeSemanticTypeId, marker| NeedProducerDefinition {
            function: RuntimeFunctionDefinitionIdentity::from_accepted_identity([marker; 32]),
            family,
            contract: NeedProducerContractDigest::from_bytes([marker + 1; 32]),
            plan: coordinate,
            producer_site: NeedProducerSiteDigest::from_bytes([marker + 2; 32]),
            handle_type: TaskHandleType::Need {
                payload: RuntimeTypeSemanticDigest::from_bytes(*payload.as_bytes()),
            },
            class: TaskClass::Cpu,
            priority: TaskPriority(0),
            cancel_scope: CancelScopeId("native.program.fanout".into()),
            policy: TaskPolicy::AlwaysStart,
            debug_label: "original Program fanout".into(),
        };
    let flow = FlowRuntimeId::canonical("native_program_fanout").unwrap();
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
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xc9; 32]),
                None,
                Box::new([]),
                unit,
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: vec![
                    RuntimeFlowOpSeed::Let {
                        pattern: pattern(vector, &source),
                        expr: RuntimeExprSeed::new(
                            vector,
                            RuntimeExprSeedKind::Value(RuntimeValue::Seq(RuntimeSeq::values(
                                items.iter().copied().map(RuntimeValue::Bool).collect(),
                            ))),
                        ),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: pattern(unsigned, &bound),
                        expr: RuntimeExprSeed::new(
                            unsigned,
                            RuntimeExprSeedKind::Value(RuntimeValue::UInt(
                                crate::value::RuntimeUInt::U32(limit),
                            )),
                        ),
                    },
                    RuntimeFlowOpSeed::ProgramTask {
                        template: RuntimeProgramTaskTemplate {
                            producer: producer(
                                base_coordinate,
                                NeedProducerFamily::AwaitManyBase,
                                vector,
                                0xc0,
                            ),
                            delivery: ProgramTaskDelivery::LocalOnly,
                            captures: vec![
                                RuntimeProgramTaskCapture::moved(source.clone()),
                                RuntimeProgramTaskCapture::copy(bound.clone()),
                            ],
                            body: RuntimeProgramTaskBody::Fanout(Box::new(RuntimeProgramFanout {
                                source: source.clone(),
                                limit: bound.clone(),
                                item: item.clone(),
                                worker: RuntimeProgramTaskTemplate {
                                    producer: producer(
                                        child_coordinate,
                                        NeedProducerFamily::AwaitManyChild,
                                        boolean,
                                        0xc4,
                                    ),
                                    delivery: ProgramTaskDelivery::LocalOnly,
                                    captures: vec![RuntimeProgramTaskCapture::moved(item.clone())],
                                    body: vec![
                                        RuntimeFlowOpSeed::If {
                                            condition: RuntimeExprSeed::new(
                                                boolean,
                                                RuntimeExprSeedKind::Local(
                                                    RuntimeLocalReadSeed::new(
                                                        item.clone(),
                                                        RuntimeLocalReadMode::Copy,
                                                    ),
                                                ),
                                            ),
                                            then_ops: vec![RuntimeFlowOpSeed::Noop; 5],
                                            else_ops: vec![],
                                        },
                                        RuntimeFlowOpSeed::ReturnExpr(read(boolean, &item)),
                                    ],
                                },
                            })),
                        },
                        name: None,
                        binding: pattern(need_vector, &handle),
                    },
                    RuntimeFlowOpSeed::Await {
                        binding: Some(pattern(vector, &result)),
                        target: RuntimeAwaitTargetSeed {
                            source: read(need_vector, &handle),
                        },
                        observers: vec![],
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        unit,
                        RuntimeExprSeedKind::Value(RuntimeValue::Unit),
                    )),
                ]
                .into_boxed_slice(),
            },
        ))
        .unwrap();
    Engine::for_flow(builder.finish().unwrap(), &flow).unwrap()
}

#[test]
fn native_program_fanout_runs_original_workers_and_restores_every_owned_frontier() {
    for count in [0, 1, 3, 17] {
        let input: Vec<_> = (0..count).map(|index| index % 3 == 0).collect();
        for limit in [1, 2, 5] {
            for (restore, decoded) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut engine = fixture(&input, limit);
                if decoded {
                    let limits = RuntimeTaskSourceArtifactLimits::default();
                    let artifact = engine.plan.task_source_artifact(limits).unwrap();
                    engine = Engine::for_flow(
                        artifact.admit(limits, None).unwrap(),
                        &FlowRuntimeId::canonical("native_program_fanout").unwrap(),
                    )
                    .unwrap();
                }
                let mut result = None;
                let mut max_flights = 0;
                let mut finished = false;
                let mut executed = 0;
                // Include the authored branch/delay and parent/controller polling.
                // Each production step retains its original one-op budget.
                let driver_bound = 64 + count * 32;
                for _ in 0..driver_bound {
                    let step = engine.step(
                        crate::step::RuntimeStepInput::default(),
                        crate::step::RuntimeStepOptions::default(),
                    );
                    assert!(step.stats.executed_ops <= 1);
                    executed += step.stats.executed_ops;
                    assert!(
                        step.output.diagnostics.is_empty(),
                        "{:?}",
                        step.output.diagnostics
                    );
                    assert!(step.output.requests.tasks.is_empty());
                    assert!(step.output.requests.program_task_completions.is_empty());
                    for event in step.output.flow_events {
                        if let crate::plan::FlowEvent::AwaitReady {
                            value: Some(value), ..
                        } = event
                            && let RuntimeValue::Seq(values) = value.into_value()
                        {
                            result = Some(values.into_values());
                        }
                    }
                    for fiber in std::iter::once(&engine.fiber).chain(&engine.child_fibers) {
                        if let FlowFiberStatus::WaitingMany(WaitingManyStatus::Program(state)) =
                            &fiber.frame.status
                        {
                            max_flights = max_flights.max(state.in_flight.len());
                            assert!(state.in_flight.len() <= limit as usize);
                            assert!(state.items[..state.next_index].iter().all(Option::is_none));
                            assert!(state.items[state.next_index..].iter().all(Option::is_some));
                        }
                    }
                    if restore {
                        let owner = RuntimeProgramOwner::Plan(Arc::clone(&engine.plan));
                        engine = Engine::from_rollback_image(
                            engine.inert_rollback_image().unwrap(),
                            &owner,
                        )
                        .unwrap();
                    }
                    if matches!(engine.fiber.frame.status, FlowFiberStatus::Done(_)) {
                        finished = true;
                        break;
                    }
                }
                assert!(
                    finished,
                    "fanout count={count}, limit={limit}, restore={restore}, decoded={decoded}, executed={executed}, main={}, children={:?}",
                    engine.fiber.frame.status.runtime_status_label(),
                    engine
                        .child_fibers
                        .iter()
                        .map(|fiber| (
                            fiber.id,
                            fiber.frame.status.runtime_status_label(),
                            fiber.frame.pending_ops.len()
                        ))
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    result,
                    Some(input.iter().copied().map(RuntimeValue::Bool).collect())
                );
                assert_eq!(engine.need_producers.task_admissions().count(), count + 1);
                assert_eq!(max_flights, count.min(limit as usize));
            }
        }
    }
}

#[test]
fn native_program_fanout_rejects_zero_limit_before_moving_parent_inputs_or_publishing() {
    let mut engine = fixture(&[true, false, true], 0);
    for _ in 0..2 {
        let step = engine.step(
            crate::step::RuntimeStepInput::default(),
            crate::step::RuntimeStepOptions::default(),
        );
        assert!(step.output.diagnostics.is_empty());
    }
    let before = engine.fiber.frame.env.bindings_snapshot();
    let frontier = engine.need_producers.launch_frontiers();
    let step = engine.step(
        crate::step::RuntimeStepInput::default(),
        crate::step::RuntimeStepOptions::default(),
    );
    assert!(
        step.output
            .diagnostics
            .iter()
            .any(|error| error.message.contains("positive u32")),
        "{:?}",
        step.output.diagnostics
    );
    assert_eq!(engine.fiber.frame.env.bindings_snapshot(), before);
    assert_eq!(engine.need_producers.launch_frontiers(), frontier);
    assert_eq!(engine.need_producers.task_admissions().count(), 0);
    assert!(step.output.requests.tasks.is_empty());
}

#[test]
fn native_program_fanout_restore_rejects_changed_limit_and_consumed_item_frontiers() {
    let mut engine = fixture(&[true, false, true], 2);
    for _ in 0..64 {
        let step = engine.step(
            crate::step::RuntimeStepInput::default(),
            crate::step::RuntimeStepOptions::default(),
        );
        assert!(step.output.diagnostics.is_empty());
        if engine.child_fibers.iter().any(|fiber| {
            matches!(&fiber.frame.status,
            FlowFiberStatus::WaitingMany(WaitingManyStatus::Program(state)) if state.next_index > 0)
        }) {
            break;
        }
    }
    let original = engine.inert_rollback_image().unwrap();
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&engine.plan));
    for mutation in 0..3 {
        let mut candidate = original.clone();
        let state = candidate
            .child_fibers
            .iter_mut()
            .find_map(|fiber| {
                if let FlowFiberStatus::WaitingMany(WaitingManyStatus::Program(state)) =
                    &mut fiber.frame.status
                {
                    Some(state)
                } else {
                    None
                }
            })
            .expect("the original controller retains pending worker positions");
        match mutation {
            0 => state.limit = 0,
            1 => state.limit += 1,
            2 => {
                state.items[0] = state.items.last().unwrap().clone();
            }
            _ => unreachable!(),
        }
        assert!(
            Engine::from_rollback_image(candidate, &owner).is_err(),
            "mutation {mutation}"
        );
    }
    let restored = Engine::from_rollback_image(original, &owner).unwrap();
    assert_eq!(
        restored.need_producers.launch_frontiers(),
        engine.need_producers.launch_frontiers()
    );
    assert_eq!(
        restored.inert_rollback_image().unwrap(),
        engine.inert_rollback_image().unwrap()
    );
}
