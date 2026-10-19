use super::super::RuntimePlanSourceConstruction;
use super::super::callable_states::tests::reserve_executable_function;
use super::super::expression::{
    RuntimeExpressionSourceHandles, tests::limits as expression_limits,
};
use super::super::functions::RuntimeFunctionSourceLimits;
use super::super::pattern::tests::{declarations, frames, meter, resources};
use super::{Counts, DOMAIN, Error, PreparedRuntimeHostCallSource, RuntimeHostCallSourceLimits};
use crate::engine::Engine;
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::{
    FlowOp, RuntimeControlEffectContractId, RuntimeEffectSet, RuntimeExecutableBodySeed,
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFunctionSiteBodySeed,
    RuntimeFunctionSiteSeedId, RuntimeFunctionSiteTable, RuntimeHostCallTargetSeed,
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind, RuntimeLocalBindingStorage,
    RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource, RuntimePatternSeed,
    RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuildError, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimePureProgramBindingSeed,
    RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanIndex, RuntimeTaskPlanSealLimits,
    RuntimeTaskPlanSeed, RuntimeTaskSemanticBinding,
};
use crate::step::{
    HostCallContractDigest, RuntimeHostCallMode, RuntimeHostCallRequest, RuntimeHostCallResult,
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::task::{
    HostCallProducerDefinition, NeedProducerContractDigest, NeedProducerFamily,
    NeedProducerSiteDigest, RuntimeHostArgumentTemplate, RuntimeRequestRoleIdentity,
};
use crate::value::{RuntimePayload, RuntimeValue};
use arcweft_id::runtime_program::RuntimePureProgramId;
use std::sync::Arc;

fn unit() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Unit.semantic_identity_digest()
}
fn boolean() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Bool.semantic_identity_digest()
}
fn pair() -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([0xd1; 32])
}
fn program() -> RuntimePureProgramId {
    RuntimePureProgramId::from_checked_digest([0xd2; 32])
}
fn value(ty: RuntimeSemanticTypeId, value: RuntimeValue) -> RuntimeExprSeed {
    RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(value))
}
fn handles() -> RuntimeExpressionSourceHandles<'static> {
    RuntimeExpressionSourceHandles {
        states: &[],
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &[],
    }
}
fn limits(plan: &RuntimePlan) -> RuntimeHostCallSourceLimits {
    RuntimeHostCallSourceLimits {
        nodes: 100_000,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        task_count: plan.task_plans().len(),
        expression: expression_limits(plan),
    }
}
fn body(host: RuntimeFlowOpSeed) -> RuntimeFunctionSiteBodySeed {
    RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
        effects: RuntimeEffectSet::empty(),
        ops: Box::new([
            host,
            RuntimeFlowOpSeed::ReturnExpr(value(unit(), RuntimeValue::Unit)),
        ]),
    })
}
pub(in crate::plan::construction::source) fn fixture(
    mode: RuntimeHostCallMode,
    contract: bool,
    binding: bool,
    empty: bool,
) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit(), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean(), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    pair(),
                    RuntimePlanTypeProjection::Tuple(Box::new([boolean(), boolean()])),
                ),
            ],
            binding.then(|| {
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Binding {
                        identity: [0xd3; 32],
                        declaration: RuntimeLocalBindingDeclaration::new(
                            RuntimeLocalBindingKind::PatternBinding,
                            false,
                            RuntimeLocalBindingStorage::Derived,
                        ),
                    },
                    boolean(),
                )
            }),
        )
        .unwrap();
    let role = |tag| RuntimeRequestRoleIdentity::from_accepted_identity([tag; 32]);
    let args = if empty {
        vec![]
    } else {
        vec![
            RuntimeHostArgumentTemplate::positional(
                role(0xd4),
                value(boolean(), RuntimeValue::Bool(true)),
            ),
            RuntimeHostArgumentTemplate::named(
                role(0xd5),
                "条件",
                value(boolean(), RuntimeValue::Bool(false)),
            ),
            RuntimeHostArgumentTemplate::spread(
                role(0xd6),
                RuntimeExprSeed::new(
                    pair(),
                    RuntimeExprSeedKind::Tuple(Box::new([
                        value(boolean(), RuntimeValue::Bool(false)),
                        value(boolean(), RuntimeValue::Bool(true)),
                    ])),
                ),
            ),
        ]
    };
    let root = reserve_executable_function(&mut builder, 0xd7, &[], 0, unit());
    let coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    builder
        .define_function_site_seed(
            &root,
            body(RuntimeFlowOpSeed::HostCall {
                binding: binding.then(|| {
                    RuntimePatternSeed::new(
                        boolean(),
                        RuntimePatternSeedKind::Bind {
                            local: admitted.local_ids()[0].clone(),
                            mutable: false,
                        },
                    )
                }),
                target: RuntimeHostCallTargetSeed {
                    producer: HostCallProducerDefinition {
                        contract: NeedProducerContractDigest::from_bytes([0xd8; 32]),
                        plan: coordinate,
                        site: NeedProducerSiteDigest::from_bytes([0xd9; 32]),
                    },
                    public_id: "呼出.読む".into(),
                    capability: "source".into(),
                    operation: "読む".into(),
                    contract: contract.then(|| HostCallContractDigest::from_bytes([0xda; 32])),
                    args,
                    result: boolean(),
                    mode,
                    deterministic: !empty,
                },
            }),
        )
        .unwrap();
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program: program(),
            site: root,
        })
        .unwrap();
    builder.finish().unwrap()
}
fn operation(plan: &RuntimePlan) -> &FlowOp {
    &plan
        .function_sites()
        .iter()
        .next()
        .unwrap()
        .body()
        .executable()
        .unwrap()
        .ops()[0]
}
fn source(
    plan: &RuntimePlan,
    task_count: usize,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    Vec<RuntimeTaskPlanBuildCoordinate>,
) {
    let mut bank = meter();
    let original_frames = frames(plan);
    let declarations = declarations(&original_frames, &mut bank);
    let bounds = RuntimeFunctionSourceLimits {
        rows: 1000,
        elements: 1_000_000,
        pattern_nodes: 100_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        type_count: plan.type_table().len(),
        local_count: plan.local_declarations().len(),
    };
    let encoded = plan
        .function_sites()
        .prepare_declaration_sources_encoding(bounds, &resources(), &mut bank)
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
    encoded.append_to(&mut bytes).unwrap();
    let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        bounds,
        &resources(),
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let functions = prepared.reserve(&mut source, &mut resources()).unwrap();
    let coordinates = (0..task_count)
        .map(|_| source.builder.reserve_task_plan().unwrap())
        .collect();
    (source, functions, coordinates)
}
fn replay(plan: &RuntimePlan, bytes: &[u8]) -> RuntimePlan {
    replay_body(plan, Some(bytes))
}
pub(in crate::plan::construction::source) fn replay_flow(plan: &RuntimePlan) -> RuntimePlan {
    replay_body(plan, None)
}
fn replay_body(plan: &RuntimePlan, host_bytes: Option<&[u8]>) -> RuntimePlan {
    let (mut source, functions, coordinates) = source(plan, 1);
    let body = if let Some(bytes) = host_bytes {
        let prepared = PreparedRuntimeHostCallSource::preflight(
            bytes,
            limits(plan),
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        body(
            prepared
                .materialize(&mut source, &coordinates[0], &handles(), &mut resources())
                .unwrap(),
        )
    } else {
        let bounds = super::super::flow::tests::limits(plan);
        let original = plan
            .function_sites()
            .iter()
            .next()
            .unwrap()
            .body()
            .executable()
            .unwrap();
        let encoded = plan
            .encode_flow_source(original.ops(), bounds, &resources(), &mut source.meter)
            .unwrap();
        let prepared = super::super::flow::PreparedRuntimeFlowSource::preflight(
            encoded.bytes(),
            bounds,
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        let expressions = handles();
        let flow_handles = super::super::flow::RuntimeFlowSourceHandles {
            expression: &expressions,
            contents: &[],
            defers: &[],
            tasks: &coordinates,
        };
        let ops = prepared
            .materialize(&mut source, &flow_handles, &mut resources())
            .unwrap();
        RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
            effects: original.effects().clone(),
            ops: ops.into_boxed_slice(),
        })
    };
    source
        .builder
        .define_function_site_seed(&functions[0], body)
        .unwrap();
    source
        .builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program: program(),
            site: functions[0].clone(),
        })
        .unwrap();
    let seal = RuntimeTaskPlanSealLimits::default();
    let request = source
        .builder
        .task_request_template_for_source(&functions[0], &coordinates[0], seal)
        .unwrap();
    let control = source
        .control(RuntimeControlEffectContractId::for_index(0).unwrap())
        .unwrap()
        .clone();
    source
        .builder
        .define_task_plan(
            &coordinates[0],
            RuntimeTaskPlanSeed::new(
                functions[0].clone(),
                NeedProducerFamily::HostAdapterTask,
                plan.task_plans()
                    .by_index(RuntimeTaskPlanIndex(0))
                    .unwrap()
                    .definition()
                    .class()
                    .clone(),
                request,
                control,
                RuntimeTaskSemanticBinding::Ordinary,
            ),
        )
        .unwrap();
    source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan()
}
pub(in crate::plan::construction::source) fn launch(
    plan: RuntimePlan,
) -> (Engine, RuntimeHostCallRequest) {
    let FlowOp::HostCall { target, .. } = operation(&plan) else {
        unreachable!()
    };
    let expected = plan.task_plan(&target.producer.plan).unwrap().key();
    let mut engine = Engine::for_program_invocation(Arc::new(plan), program(), vec![]).unwrap();
    let report = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 64 },
            ..RuntimeStepOptions::default()
        },
    );
    assert!(
        report.output.diagnostics.is_empty(),
        "{:?}",
        report.output.diagnostics
    );
    let mut calls = report.output.requests.host_calls.into_iter();
    let call = calls
        .next()
        .expect("the original program must emit its Host call");
    assert!(calls.next().is_none());
    assert_eq!(call.submission().spec().producer.plan(), expected);
    (engine, call)
}

fn header_offsets(bytes: &[u8], bounds: RuntimeHostCallSourceLimits) -> (usize, usize) {
    let row = super::reading::Cursor::new(bytes)
        .row(&mut Counts::new(bounds).unwrap())
        .unwrap();
    let contract = DOMAIN.len()
        + 1
        + 4
        + 64
        + [row.public_id, row.capability, row.operation]
            .iter()
            .map(|text| 4 + text.len())
            .sum::<usize>();
    (
        contract,
        contract + 1 + if row.contract.is_some() { 32 } else { 0 },
    )
}

#[test]
fn host_sources_restore_all_modes_contracts_bindings_and_ordered_arguments_then_execute_the_actual_program()
 {
    for mode in [RuntimeHostCallMode::Immediate, RuntimeHostCallMode::Suspend] {
        for contract in [false, true] {
            for binding in [false, true] {
                for empty in [false, true] {
                    let plan = fixture(mode, contract, binding, empty);
                    let encoded = operation(&plan)
                        .encode_host_call_source(&plan, limits(&plan), &resources(), &mut meter())
                        .unwrap();
                    let prepared = PreparedRuntimeHostCallSource::preflight(
                        encoded.bytes(),
                        limits(&plan),
                        &resources(),
                        &mut meter(),
                    )
                    .unwrap();
                    assert_eq!(prepared.node_count(), encoded.node_count());
                    assert_eq!(prepared.elements(), encoded.elements());
                    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
                    let restored = replay(&plan, encoded.bytes());
                    let FlowOp::HostCall {
                        target: original,
                        binding: original_binding,
                    } = operation(&plan)
                    else {
                        unreachable!()
                    };
                    let FlowOp::HostCall {
                        target: decoded,
                        binding: decoded_binding,
                    } = operation(&restored)
                    else {
                        unreachable!()
                    };
                    assert_eq!(decoded_binding, original_binding);
                    assert_eq!(decoded.producer.contract, original.producer.contract);
                    assert_eq!(decoded.producer.site, original.producer.site);
                    assert!(restored.task_plan(&original.producer.plan).is_none());
                    assert_eq!(decoded.public_id, original.public_id);
                    assert_eq!(decoded.capability, original.capability);
                    assert_eq!(decoded.operation, original.operation);
                    assert_eq!(decoded.contract, original.contract);
                    assert_eq!(decoded.result, original.result);
                    assert_eq!(decoded.mode, original.mode);
                    assert_eq!(decoded.deterministic, original.deterministic);
                    assert_eq!(decoded.args, original.args);
                    assert_eq!(
                        restored.executable_semantic_digest(),
                        plan.executable_semantic_digest()
                    );
                    assert_eq!(
                        restored.task_plan(&decoded.producer.plan).unwrap().key(),
                        plan.task_plan(&original.producer.plan).unwrap().key()
                    );
                    let (_original_engine, original) = launch(plan);
                    let (_restored_engine, restored) = launch(restored);
                    assert_eq!(original.mode, restored.mode);
                    assert_eq!(original.deterministic, restored.deterministic);
                    assert_eq!(original.capability(), restored.capability());
                    assert_eq!(original.operation(), restored.operation());
                    assert_eq!(original.contract(), restored.contract());
                    assert_eq!(original.args(), restored.args());
                    assert_eq!(original.named_args(), restored.named_args());
                    assert_eq!(restored.args().len(), if empty { 0 } else { 3 });
                    assert_eq!(restored.named_args().len(), usize::from(!empty));
                    if !empty {
                        assert_eq!(
                            restored
                                .args()
                                .iter()
                                .map(|value| &value.0)
                                .collect::<Vec<_>>(),
                            [
                                &RuntimeValue::Bool(true),
                                &RuntimeValue::Bool(false),
                                &RuntimeValue::Bool(true)
                            ]
                        );
                        assert_eq!(restored.named_args()[0].name, "条件");
                        assert_eq!(restored.named_args()[0].value.0, RuntimeValue::Bool(false));
                    }
                }
            }
        }
    }
}

#[test]
fn host_sources_resume_the_original_native_continuation_after_the_actual_host_result() {
    let plan = fixture(RuntimeHostCallMode::Suspend, true, true, false);
    let encoded = operation(&plan)
        .encode_host_call_source(&plan, limits(&plan), &resources(), &mut meter())
        .unwrap();
    let restored = replay(&plan, encoded.bytes());
    for plan in [plan, restored] {
        let (mut engine, call) = launch(plan);
        assert!(engine.take_program_result().unwrap().is_none());
        let report = engine.step(
            RuntimeStepInput {
                host_call_results: vec![RuntimeHostCallResult {
                    id: call.id.clone(),
                    outcome: Ok(RuntimePayload(RuntimeValue::Bool(true))),
                }],
                ..RuntimeStepInput::default()
            },
            RuntimeStepOptions {
                mode: RuntimeStepMode::Drain,
                budget: RuntimeStepBudget { max_ops: 64 },
                ..RuntimeStepOptions::default()
            },
        );
        assert!(
            report.output.diagnostics.is_empty(),
            "{:?}",
            report.output.diagnostics
        );
        assert!(report.output.requests.host_calls.is_empty());
        assert_eq!(
            engine.take_program_result().unwrap(),
            Some((program(), RuntimeValue::Unit))
        );
    }
}

#[test]
fn host_sources_reject_a_different_result_binding_through_the_actual_builder_before_body_publication()
 {
    let plan = fixture(RuntimeHostCallMode::Suspend, true, true, false);
    let encoded = operation(&plan)
        .encode_host_call_source(&plan, limits(&plan), &resources(), &mut meter())
        .unwrap();
    let (_, result) = header_offsets(encoded.bytes(), limits(&plan));
    let mut bytes = encoded.bytes().to_vec();
    bytes[result..result + 4].copy_from_slice(
        &plan
            .type_table()
            .id_for_semantic(unit())
            .unwrap()
            .get()
            .get()
            .to_le_bytes(),
    );
    let (mut source, functions, coordinates) = source(&plan, 1);
    let prepared = PreparedRuntimeHostCallSource::preflight(
        &bytes,
        limits(&plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    let host = prepared
        .materialize(&mut source, &coordinates[0], &handles(), &mut resources())
        .unwrap();
    assert!(matches!(
        source
            .builder
            .define_function_site_seed(&functions[0], body(host)),
        Err(RuntimePlanBuildError::TypeMismatch {
            context: "host call binding",
            ..
        })
    ));
    assert!(source.builder.function_sites[0].body.is_none());
    assert!(matches!(
        source.builder.finish(),
        Err(RuntimePlanBuildError::Poisoned)
    ));
}

#[test]
fn host_sources_reject_foreign_equal_key_and_reordered_real_task_reservations_before_materialization()
 {
    let plan = fixture(RuntimeHostCallMode::Suspend, true, true, false);
    let foreign = fixture(RuntimeHostCallMode::Suspend, true, true, false);
    assert_eq!(
        plan.executable_semantic_digest(),
        foreign.executable_semantic_digest()
    );
    assert!(matches!(
        operation(&plan).encode_host_call_source(
            &foreign,
            limits(&foreign),
            &resources(),
            &mut meter()
        ),
        Err(Error::TaskCoordinate)
    ));
    let encoded = operation(&plan)
        .encode_host_call_source(&plan, limits(&plan), &resources(), &mut meter())
        .unwrap();
    for reordered in [false, true] {
        let (mut source, _, coordinates) = source(&plan, if reordered { 2 } else { 1 });
        let mut bounds = limits(&plan);
        if reordered {
            bounds.task_count = 2;
        }
        let prepared = PreparedRuntimeHostCallSource::preflight(
            encoded.bytes(),
            bounds,
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        let FlowOp::HostCall { target, .. } = operation(&plan) else {
            unreachable!()
        };
        let coordinate = if reordered {
            &coordinates[1]
        } else {
            &target.producer.plan
        };
        assert!(matches!(
            prepared.materialize(&mut source, coordinate, &handles(), &mut resources()),
            Err(Error::TaskCoordinate)
        ));
        assert_eq!(
            source.meter.status(),
            Err(TaskSemanticEncodingError::OwnerRejected)
        );
        assert!(matches!(
            prepared.materialize(&mut source, &coordinates[0], &handles(), &mut resources()),
            Err(Error::Resource(TaskSemanticEncodingError::OwnerRejected))
        ));
    }
}

#[test]
fn host_sources_admit_exact_aggregate_resources_and_reject_one_over_without_resetting_the_bank() {
    let plan = fixture(RuntimeHostCallMode::Suspend, true, true, false);
    let encoded = operation(&plan)
        .encode_host_call_source(&plan, limits(&plan), &resources(), &mut meter())
        .unwrap();
    let prepared = PreparedRuntimeHostCallSource::preflight(
        encoded.bytes(),
        limits(&plan),
        &resources(),
        &mut meter(),
    )
    .unwrap();
    let exact = RuntimeHostCallSourceLimits {
        nodes: u32::try_from(prepared.node_count()).unwrap(),
        elements: u32::try_from(prepared.elements()).unwrap(),
        encoded_bytes: encoded.bytes().len(),
        decoded_bytes: prepared.storage_bytes(),
        ..limits(&plan)
    };
    PreparedRuntimeHostCallSource::preflight(encoded.bytes(), exact, &resources(), &mut meter())
        .unwrap();
    for (kind, bounds) in [
        (
            "nodes",
            RuntimeHostCallSourceLimits {
                nodes: exact.nodes - 1,
                ..exact
            },
        ),
        (
            "elements",
            RuntimeHostCallSourceLimits {
                elements: exact.elements - 1,
                ..exact
            },
        ),
        (
            "encoded bytes",
            RuntimeHostCallSourceLimits {
                encoded_bytes: exact.encoded_bytes - 1,
                ..exact
            },
        ),
        (
            "decoded bytes",
            RuntimeHostCallSourceLimits {
                decoded_bytes: exact.decoded_bytes - 1,
                ..exact
            },
        ),
    ] {
        let mut bank = meter();
        assert!(
            PreparedRuntimeHostCallSource::preflight(
                encoded.bytes(),
                bounds,
                &resources(),
                &mut bank
            )
            .is_err(),
            "{kind}"
        );
        assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
        assert!(matches!(
            PreparedRuntimeHostCallSource::preflight(
                encoded.bytes(),
                exact,
                &resources(),
                &mut bank
            ),
            Err(Error::Resource(TaskSemanticEncodingError::OwnerRejected))
        ));
    }
    let mut bank = TaskSemanticMeter::new(0, 100_000_000);
    assert!(matches!(
        PreparedRuntimeHostCallSource::preflight(encoded.bytes(), exact, &resources(), &mut bank),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert_eq!(bank.status(), Err(TaskSemanticEncodingError::SemanticWork));
    assert!(matches!(
        PreparedRuntimeHostCallSource::preflight(encoded.bytes(), exact, &resources(), &mut bank),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
}

#[test]
fn host_sources_reject_every_truncation_bad_version_references_tags_utf8_and_trailing_bytes() {
    let plan = fixture(RuntimeHostCallMode::Suspend, true, true, false);
    let encode = || {
        operation(&plan)
            .encode_host_call_source(&plan, limits(&plan), &resources(), &mut meter())
            .unwrap()
    };
    let encoded = encode();
    for end in 0..encoded.bytes().len() {
        assert!(
            PreparedRuntimeHostCallSource::preflight(
                &encoded.bytes()[..end],
                limits(&plan),
                &resources(),
                &mut meter()
            )
            .is_err(),
            "prefix {end}"
        );
    }
    let mut bad = encoded.bytes().to_vec();
    bad[DOMAIN.len()] = 2;
    assert!(
        PreparedRuntimeHostCallSource::preflight(&bad, limits(&plan), &resources(), &mut meter())
            .is_err()
    );
    bad = encoded.bytes().to_vec();
    bad[DOMAIN.len() + 1..DOMAIN.len() + 5].copy_from_slice(&1_u32.to_le_bytes());
    assert!(matches!(
        PreparedRuntimeHostCallSource::preflight(&bad, limits(&plan), &resources(), &mut meter()),
        Err(Error::Reference { kind: "task", .. })
    ));
    let row = super::reading::Cursor::new(encoded.bytes())
        .row(&mut Counts::new(limits(&plan)).unwrap())
        .unwrap();
    let (contract_flag, result_type) = header_offsets(encoded.bytes(), limits(&plan));
    for flag in [
        contract_flag,
        result_type + 4,
        result_type + 5,
        result_type + 6,
    ] {
        bad = encoded.bytes().to_vec();
        bad[flag] = 2;
        assert!(matches!(
            PreparedRuntimeHostCallSource::preflight(
                &bad,
                limits(&plan),
                &resources(),
                &mut meter()
            ),
            Err(Error::Invalid { .. })
        ));
    }
    bad = encoded.bytes().to_vec();
    bad[result_type..result_type + 4].copy_from_slice(&0_u32.to_le_bytes());
    assert!(matches!(
        PreparedRuntimeHostCallSource::preflight(&bad, limits(&plan), &resources(), &mut meter()),
        Err(Error::Reference {
            kind: "result type",
            ..
        })
    ));
    bad = encoded.bytes().to_vec();
    bad[encoded.bytes().len() - row.arguments.len() + 32] = 3;
    assert!(matches!(
        PreparedRuntimeHostCallSource::preflight(&bad, limits(&plan), &resources(), &mut meter()),
        Err(Error::Invalid {
            kind: "argument kind",
            ..
        })
    ));
    bad = encoded.bytes().to_vec();
    let name = bad
        .windows("条件".len())
        .position(|part| part == "条件".as_bytes())
        .unwrap();
    bad[name] = 0xff;
    assert!(matches!(
        PreparedRuntimeHostCallSource::preflight(&bad, limits(&plan), &resources(), &mut meter()),
        Err(Error::Utf8(_))
    ));
    bad = encoded.bytes().to_vec();
    bad.push(0);
    assert!(matches!(
        PreparedRuntimeHostCallSource::preflight(&bad, limits(&plan), &resources(), &mut meter()),
        Err(Error::Trailing { .. })
    ));
    let mut output = Vec::new();
    output.push(0xa5);
    assert!(output.capacity() - output.len() < encoded.bytes().len());
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [0xa5]);
    let encoded = encode();
    let expected = encoded.bytes().to_vec();
    let mut output = Vec::with_capacity(expected.len());
    encoded.append_to(&mut output).unwrap();
    assert_eq!(output, expected);
}
