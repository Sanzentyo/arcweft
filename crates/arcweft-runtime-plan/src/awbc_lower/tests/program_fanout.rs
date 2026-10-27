use super::*;
use arcweft_core::awbc::schema::{
    AwbcProgramFanoutInput, AwbcProgramTaskKernel, AwbcTaskPlanExecution, AwbcTaskPlanId,
};
use arcweft_core::pattern::RuntimeCheckedType;
use arcweft_core::plan::*;
use arcweft_core::task::*;
use arcweft_core::value::RuntimeSeq;
fn fixture(items: &[bool], limit: u32) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
    let unsigned = RuntimeCheckedType::Unsigned(arcweft_core::value::RuntimeUnsignedIntWidth::U32)
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
                    RuntimePlanTypeProjection::Unsigned(
                        arcweft_core::value::RuntimeUnsignedIntWidth::U32,
                    ),
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
        .push_flow_executable(flow_executable(&flow))
        .unwrap();
    builder
        .push_entry(flow_entry("native_program_fanout", flow.clone()))
        .unwrap();
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
                                arcweft_core::value::RuntimeUInt::U32(limit),
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
    builder.finish().unwrap()
}

#[test]
fn original_program_fanout_lowers_to_typed_kernels_and_roundtrips_canonical_awbc() {
    for items in [&[][..], &[true][..], &[true, false, true][..]] {
        for limit in [1, 2] {
            let plan = fixture(items, limit);
            let report = lower_plan(&plan);
            let program = &report.program;
            assert_eq!(program.task_plans.len(), 2);
            let (parent, worker) = program
                .task_plans
                .iter()
                .enumerate()
                .find_map(|(index, row)| {
                    let AwbcTaskPlanExecution::Program(body) = &row.execution else {
                        return None;
                    };
                    let AwbcProgramTaskKernel::Fanout {
                        worker,
                        source,
                        limit,
                        inputs,
                    } = &body.body
                    else {
                        return None;
                    };
                    assert_eq!((*source, *limit), (0, 1));
                    assert_eq!(inputs, &[AwbcProgramFanoutInput::Item]);
                    Some((index, *worker))
                })
                .unwrap();
            let AwbcTaskPlanExecution::Program(worker_body) =
                &program.task_plans[worker.index()].execution
            else {
                panic!("worker must execute original Program code")
            };
            assert_eq!(worker_body.body, AwbcProgramTaskKernel::Flow);
            assert_eq!(worker_body.capture_modes, [RuntimeLocalReadMode::Move]);
            program
                .verify(Default::default(), Default::default())
                .unwrap();
            let bytes = program.encode_canonical().unwrap();
            let decoded = AwbcProgram::decode_canonical(&bytes, Default::default()).unwrap();
            assert_eq!(&decoded, program);
            decoded
                .verify(Default::default(), Default::default())
                .unwrap();
            for mutation in 0..3 {
                let mut forged = decoded.clone();
                let AwbcTaskPlanExecution::Program(body) = &mut forged.task_plans[parent].execution
                else {
                    unreachable!()
                };
                let AwbcProgramTaskKernel::Fanout {
                    worker,
                    source,
                    limit,
                    inputs,
                } = &mut body.body
                else {
                    unreachable!()
                };
                match mutation {
                    0 => std::mem::swap(source, limit),
                    1 => *worker = AwbcTaskPlanId(u32::try_from(parent).unwrap()),
                    2 => inputs[0] = AwbcProgramFanoutInput::Capture { position: *source },
                    _ => unreachable!(),
                }
                assert!(
                    forged
                        .verify(Default::default(), Default::default())
                        .is_err(),
                    "mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn original_program_fanout_executes_owned_workers_and_roundtrips_every_public_save_frontier() {
    use arcweft_core::awbc::fiber::FiberSuspensionReason;
    use arcweft_core::awbc::product_step::{AwbcProductExecutorSaveSnapshot, AwbcProductProgram};
    use arcweft_core::engine::FlowFiberStatus;
    use arcweft_core::executor::{
        ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot, RuntimeExecutor,
    };
    use arcweft_core::plan::FlowEvent;
    use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions};
    use arcweft_core::value::RuntimePayload;

    for count in [0, 1, 3, 17] {
        let items: Vec<_> = (0..count).map(|index| index % 3 != 1).collect();
        let expected = RuntimePayload::from(RuntimeValue::Seq(RuntimeSeq::values(
            items.iter().copied().map(RuntimeValue::Bool).collect(),
        )));
        for limit in [1, 2, 5] {
            for decode in [false, true] {
                for restore in [false, true] {
                    let plan = fixture(&items, limit);
                    let report = lower_plan(&plan);
                    let program = if decode {
                        AwbcProgram::decode_canonical(
                            &report.program.encode_canonical().unwrap(),
                            Default::default(),
                        )
                        .unwrap()
                    } else {
                        report.program
                    };
                    let product = AwbcProductProgram::admit(
                        Arc::new(program),
                        Default::default(),
                        Default::default(),
                        Default::default(),
                        None,
                    )
                    .unwrap();
                    let mut executor =
                        ArcweftRuntimeExecutor::from_admitted_awbc_product_flow_invocation(
                            product.clone(),
                            plan.seal_flow_invocation(flow_id("native_program_fanout"), [])
                                .unwrap(),
                        )
                        .unwrap();
                    let mut max_flights = 0;
                    let mut ready = 0;
                    let mut finished = false;
                    for _ in 0..(64 + count * 32) {
                        let step = executor
                            .step(RuntimeStepInput::default(), RuntimeStepOptions::default());
                        assert!(step.stats.executed_ops <= 1);
                        assert!(
                            step.output.diagnostics.is_empty(),
                            "count={count} limit={limit} decode={decode} restore={restore}: {:?}",
                            step.output.diagnostics
                        );
                        assert!(step.output.requests.tasks.is_empty());
                        assert!(step.output.requests.program_task_completions.is_empty());
                        assert!(step.output.requests.host_calls.is_empty());
                        for event in &step.output.flow_events {
                            if let FlowEvent::AwaitReady {
                                value: Some(value), ..
                            } = event
                            {
                                assert_eq!(value, &expected);
                                ready += 1;
                            }
                        }
                        let ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot) =
                            executor.snapshot().unwrap()
                        else {
                            unreachable!()
                        };
                        for child in &snapshot.child_fibers {
                            if let Some(suspension) = &child.fiber.suspension {
                                if let FiberSuspensionReason::ProgramFanout(state) =
                                    &suspension.reason
                                {
                                    max_flights = max_flights.max(state.in_flight.len());
                                    assert!(state.in_flight.len() <= limit as usize);
                                    for (index, value) in state.items.iter().enumerate() {
                                        assert_eq!(
                                            value.is_some(),
                                            index >= state.next_index as usize
                                        );
                                    }
                                }
                            }
                        }
                        let saved = AwbcProductExecutorSaveSnapshot::from_live(&snapshot, &product)
                            .unwrap();
                        let done =
                            matches!(executor.fiber().frame.status, FlowFiberStatus::Done(_));
                        if done {
                            assert_eq!(saved.need_producers.task_admissions.len(), count + 1);
                        }
                        if restore {
                            let decoded =
                                serde_json::from_slice(&serde_json::to_vec(&saved).unwrap())
                                    .unwrap();
                            let rebound = executor.bind_product_save_snapshot(decoded).unwrap();
                            executor = executor
                                .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(
                                    rebound,
                                ))
                                .unwrap();
                        }
                        if done {
                            finished = true;
                            break;
                        }
                    }
                    assert!(
                        finished,
                        "count={count} limit={limit} decode={decode} restore={restore}"
                    );
                    assert_eq!(ready, 1);
                    assert_eq!(max_flights, count.min(limit as usize));
                }
            }
        }
    }
}
