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
