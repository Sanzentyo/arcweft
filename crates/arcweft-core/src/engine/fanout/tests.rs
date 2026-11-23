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
                            effects: crate::plan::RuntimeEffectSet::empty(),
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
                                    effects: crate::plan::RuntimeEffectSet::empty(),
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
fn native_program_fanout_drain_reaches_quiescence_without_empty_host_turns() {
    let mut engine = fixture(&[true, false, true], 2);
    let result = engine.step(
        crate::step::RuntimeStepInput::default(),
        crate::step::RuntimeStepOptions {
            mode: crate::step::RuntimeStepMode::Drain,
            budget: crate::step::RuntimeStepBudget { max_ops: 256 },
            ..Default::default()
        },
    );
    assert!(result.stats.executed_ops <= 256);
    assert!(
        result.output.diagnostics.is_empty(),
        "{:?}",
        result.output.diagnostics
    );
    assert!(result.output.requests.tasks.is_empty());
    assert!(result.output.requests.program_task_completions.is_empty());
    assert_eq!(
        result.stop_reason,
        crate::step::RuntimeStepStopReason::Done,
        "{result:?}"
    );
    assert!(matches!(result.fiber_status, FlowFiberStatus::Done(_)));
    assert!(result.output.flow_events.iter().any(|event| matches!(event,
        crate::plan::FlowEvent::AwaitReady { value: Some(value), .. }
        if value.value() == &crate::value::runtime_sequence_values(vec![RuntimeValue::Bool(true), RuntimeValue::Bool(false), RuntimeValue::Bool(true)])
    )));
}

#[test]
fn native_program_fanout_cold_json_resumes_executing_receipts() {
    let input = [true, false, true];
    let mut engine = fixture(&input, 2);
    let mut saw_child = false;
    let mut result = None;
    for _ in 0..160 {
        let turn = engine.step(Default::default(), Default::default());
        assert!(turn.output.diagnostics.is_empty(), "{turn:?}");
        assert!(turn.output.requests.tasks.is_empty());
        for event in turn.output.flow_events {
            if let crate::plan::FlowEvent::AwaitReady {
                value: Some(value), ..
            } = event
                && let RuntimeValue::Seq(values) = value.into_value()
            {
                result = Some(values.into_values());
            }
        }
        saw_child |= engine
            .session_fibers()
            .any(|fiber| matches!(fiber.launch, crate::engine::FlowFiberLaunch::Program { .. }));
        let wire = serde_json::to_vec(&engine.snapshot_for_save().unwrap()).unwrap();
        let mut cold = fixture(&input, 2);
        cold.restore_save_snapshot(serde_json::from_slice(&wire).unwrap())
            .unwrap();
        assert_eq!(
            serde_json::to_vec(&cold.snapshot_for_save().unwrap()).unwrap(),
            wire
        );
        engine = cold;
        if matches!(engine.fiber.frame.status, FlowFiberStatus::Done(_)) {
            break;
        }
    }
    assert!(saw_child);
    assert_eq!(result.unwrap(), input.map(RuntimeValue::Bool));
    assert!(matches!(
        engine.fiber.frame.status,
        FlowFiberStatus::Done(_)
    ));
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
                        let mut image = engine.inert_rollback_image().unwrap();
                        for fiber in std::iter::once(&mut image.fiber)
                            .chain(image.parked_main.iter_mut())
                            .chain(&mut image.child_fibers)
                        {
                            let wire = serde_json::to_vec(&*fiber).unwrap();
                            let decoded: crate::engine::FlowFiber<
                                crate::engine::NativeFlowExecutionFrameRollbackImage<()>,
                            > = serde_json::from_slice(&wire).unwrap();
                            *fiber = decoded
                                .try_map_frame::<_, std::convert::Infallible>(|frame| {
                                    Ok(frame.bind_program(owner.clone()))
                                })
                                .unwrap();
                        }
                        engine = Engine::from_rollback_image(image, &owner).unwrap();
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

#[test]
fn native_program_fibers_require_distinct_executing_receipts_after_frame_decode() {
    use crate::engine::{
        FlowFiber, FlowFiberLaunch, NativeFlowExecutionFrameRollbackImage, NativeInvocationRoot,
    };
    use crate::task::{TaskCorrelation, TaskLaunchOrdinal};
    use crate::value::AwbcRuntimeValueSnapshot;

    let mut engine = fixture(&[true, false, true], 2);
    for _ in 0..64 {
        let step = engine.step(Default::default(), Default::default());
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        if engine.child_fibers.len() >= 2 {
            break;
        }
    }
    let original = engine.inert_rollback_image().unwrap();
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&engine.plan));
    let controller = original
        .child_fibers
        .iter()
        .position(|fiber| {
            matches!(
                &fiber.frame.status,
                FlowFiberStatus::WaitingMany(WaitingManyStatus::Program(_))
            )
        })
        .expect("the controller waits on its launched worker");
    let worker = (0..original.child_fibers.len())
        .find(|index| *index != controller)
        .expect("a worker is executing");
    let FlowFiberLaunch::Program {
        task: controller_correlation,
        ..
    } = original.child_fibers[controller].launch
    else {
        unreachable!()
    };
    let FlowFiberLaunch::Program {
        task: worker_correlation,
        origin: ref worker_origin,
    } = original.child_fibers[worker].launch
    else {
        unreachable!()
    };

    let wire = serde_json::to_value(&original.child_fibers[worker]).unwrap();
    assert!(wire["frame"].get("program").is_none());
    assert!(wire["launch"]["Program"].get("receipt").is_none());
    let mut with_receipt = wire.clone();
    with_receipt["launch"]["Program"]["receipt"] = serde_json::Value::Null;
    assert!(
        serde_json::from_value::<FlowFiber<NativeFlowExecutionFrameRollbackImage<()>>>(
            with_receipt
        )
        .is_err()
    );
    let mut with_plan = wire.clone();
    with_plan["frame"]["program"] = serde_json::Value::Null;
    assert!(
        serde_json::from_value::<FlowFiber<NativeFlowExecutionFrameRollbackImage<()>>>(with_plan)
            .is_err()
    );
    let worker_spec = original
        .need_producers
        .task_spec(worker_correlation)
        .unwrap();
    let unissued = TaskCorrelation::try_for(
        worker_spec.generation,
        &worker_spec.producer,
        worker_spec.policy,
        TaskLaunchOrdinal::try_for_policy(
            worker_spec.policy,
            worker_correlation.launch_ordinal.get() + 1,
        )
        .unwrap(),
    )
    .unwrap();
    for (launch, message) in [
        (
            original.child_fibers[controller].launch.clone(),
            "repeats an executing Program receipt",
        ),
        (
            FlowFiberLaunch::Program {
                task: unissued,
                origin: worker_origin.clone(),
            },
            "differs from its executing Program receipt",
        ),
        (
            FlowFiberLaunch::Local,
            "receipts differ from the Native fibers",
        ),
    ] {
        let mut altered = wire.clone();
        altered["launch"] = serde_json::to_value(launch).unwrap();
        let decoded: FlowFiber<NativeFlowExecutionFrameRollbackImage<()>> =
            serde_json::from_value(altered).unwrap();
        let mut candidate = original.clone();
        candidate.child_fibers[worker] = decoded
            .try_map_frame::<_, std::convert::Infallible>(|frame| {
                Ok(frame.bind_program(owner.clone()))
            })
            .unwrap();
        let error = Engine::from_rollback_image(candidate, &owner).unwrap_err();
        assert!(error.contains(message), "{error}");
    }
    let mut swapped = original.clone();
    for (index, replacement) in [
        (controller, worker_correlation),
        (worker, controller_correlation),
    ] {
        let FlowFiberLaunch::Program { task, .. } = &mut swapped.child_fibers[index].launch else {
            unreachable!()
        };
        *task = replacement;
    }
    let error = Engine::from_rollback_image(swapped, &owner).unwrap_err();
    assert!(error.contains("different original Program task"), "{error}");
    let mut wrong_result = original.clone();
    wrong_result.child_fibers[controller]
        .frame
        .invocation_result = Some((
        NativeInvocationRoot::Task(controller_correlation),
        AwbcRuntimeValueSnapshot::Bool(true),
    ));
    assert!(Engine::from_rollback_image(wrong_result, &owner).is_err());
    let mut swapped_result = original.clone();
    swapped_result.child_fibers[worker].frame.invocation_result = Some((
        NativeInvocationRoot::Task(controller_correlation),
        AwbcRuntimeValueSnapshot::Bool(true),
    ));
    let error = Engine::from_rollback_image(swapped_result, &owner).unwrap_err();
    assert!(
        error.contains("result differs from its owning accepted launch"),
        "{error}"
    );
}
