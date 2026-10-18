use super::super::callable_states::tests::reserve_executable_function;
use super::super::expression::{
    RuntimeExpressionSourceHandles, tests::limits as expression_limits,
};
use super::super::functions::RuntimeFunctionSourceLimits;
use super::super::pattern::tests::{declarations, frames, meter, resources};
use super::*;
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::value::{RuntimeEntityReference, RuntimeValue};

fn unit() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Unit.semantic_identity_digest()
}
fn boolean() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Bool.semantic_identity_digest()
}
fn entity() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::EntityReference.semantic_identity_digest()
}
fn need() -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([0xb7; 32])
}
fn value(ty: RuntimeSemanticTypeId, value: RuntimeValue) -> RuntimeExprSeed {
    RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(value))
}
fn empty_handles() -> RuntimeExpressionSourceHandles<'static> {
    RuntimeExpressionSourceHandles {
        states: &[],
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &[],
    }
}
fn limits(plan: &RuntimePlan) -> RuntimeSelectedProducerSourceLimits {
    RuntimeSelectedProducerSourceLimits {
        nodes: 100_000,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        task_count: plan.task_plans().len(),
        expression: expression_limits(plan),
    }
}
fn source(
    plan: &RuntimePlan,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    RuntimeTaskPlanBuildCoordinate,
) {
    let mut bank = meter();
    let frames = frames(plan);
    let declarations = declarations(&frames, &mut bank);
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
    let functions = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        bounds,
        &resources(),
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let functions = functions.reserve(&mut source, &mut resources()).unwrap();
    let coordinate = source.builder.reserve_task_plan().unwrap();
    (source, functions, coordinate)
}
fn body(start: RuntimeFlowOpSeed) -> RuntimeFunctionSiteBodySeed {
    RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
        effects: RuntimeEffectSet::empty(),
        ops: Box::new([
            start,
            RuntimeFlowOpSeed::ReturnExpr(value(unit(), RuntimeValue::Unit)),
        ]),
    })
}
fn fixture(
    request: u8,
    policy: TaskPolicy,
    restart: HostRestartPolicy,
    class: TaskClass,
) -> RuntimePlan {
    let contract = NeedProducerContractDigest::from_bytes([0x61; 32]);
    let role = |tag| RuntimeRequestRoleIdentity::from_accepted_identity([tag; 32]);
    let (projection, types, arguments) = match request {
        0 | 1 => (
            NeedProducerRequestProjection::AssetLoad {
                kind: if request == 0 {
                    AssetLoadKind::Image
                } else {
                    AssetLoadKind::Voice
                },
                argument_name: "素材".into(),
            },
            Box::new([entity()]) as Box<[_]>,
            vec![RuntimeHostArgumentTemplate::named(
                role(0x62),
                "素材",
                value(
                    entity(),
                    RuntimeValue::EntityRef(RuntimeEntityReference::StructuralFlow(
                        FlowRuntimeId::canonical("asset.source").unwrap(),
                    )),
                ),
            )],
        ),
        2 => (
            NeedProducerRequestProjection::ExternCapability {
                capability: HostCapabilityId("source".into()),
                operation: "読む".into(),
                contract: crate::step::HostCallContractDigest::from_bytes([0x61; 32]),
                argument_names: Box::new([None, Some("条件".into())]),
            },
            Box::new([boolean(), boolean()]) as Box<[_]>,
            vec![
                RuntimeHostArgumentTemplate::positional(
                    role(0x63),
                    value(boolean(), RuntimeValue::Bool(true)),
                ),
                RuntimeHostArgumentTemplate::named(
                    role(0x64),
                    "条件",
                    value(boolean(), RuntimeValue::Bool(false)),
                ),
            ],
        ),
        _ => unreachable!(),
    };
    let plan = NeedProducerTaskPlan::try_new(
        contract,
        NeedProducerSiteDigest::from_bytes([0x65; 32]),
        projection,
        types,
        boolean(),
        policy,
        restart,
        class,
        TaskPriority(-17),
        CancelScopeId("所有者".into()),
    )
    .unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit(), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean(), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(entity(), RuntimePlanTypeProjection::EntityReference),
                RuntimePlanTypeSeed::new(need(), RuntimePlanTypeProjection::Need(boolean())),
            ],
            [],
        )
        .unwrap();
    let function = reserve_executable_function(&mut builder, 0x66, &[], 0, unit());
    let coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    builder
        .define_function_site_seed(
            &function,
            body(RuntimeFlowOpSeed::StartNeedProducer {
                binding: RuntimePatternSeed::new(need(), RuntimePatternSeedKind::Discard),
                target: RuntimeNeedProducerStartTargetSeed {
                    plan,
                    task_plan: coordinate,
                    arguments,
                },
            }),
        )
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
fn replay(plan: &RuntimePlan, bytes: &[u8]) -> RuntimePlan {
    let (mut source, functions, coordinate) = source(plan);
    let prepared = PreparedRuntimeSelectedProducerSource::preflight(
        bytes,
        limits(plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    let start = prepared
        .materialize(&mut source, &coordinate, &empty_handles(), &mut resources())
        .unwrap();
    source
        .builder
        .define_function_site_seed(&functions[0], body(start))
        .unwrap();
    let request = source
        .builder
        .task_request_template_for_source(
            &functions[0],
            &coordinate,
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let control = source
        .control(RuntimeControlEffectContractId::for_index(0).unwrap())
        .unwrap()
        .clone();
    source
        .builder
        .define_task_plan(
            &coordinate,
            RuntimeTaskPlanSeed::new(
                functions[0].clone(),
                NeedProducerFamily::SelectedCallable,
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
    let seal = RuntimeTaskPlanSealLimits::default();
    source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan()
}

#[test]
fn selected_producer_source_replays_all_request_families_through_the_actual_builder_and_common_seal()
 {
    for request in 0..3 {
        for (policy, restart, class) in [
            (
                TaskPolicy::JoinSameKey,
                HostRestartPolicy::Restartable,
                TaskClass::Io,
            ),
            (
                TaskPolicy::AlwaysStart,
                HostRestartPolicy::MustBeQuiescent,
                TaskClass::AssetDecode,
            ),
        ] {
            let plan = fixture(request, policy, restart, class);
            let encoded = operation(&plan)
                .encode_selected_producer_source(&plan, limits(&plan), &resources(), &mut meter())
                .unwrap();
            let prepared = PreparedRuntimeSelectedProducerSource::preflight(
                encoded.bytes(),
                limits(&plan),
                &resources(),
                &mut meter(),
            )
            .unwrap();
            assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
            assert_eq!(prepared.elements(), encoded.elements());
            assert_eq!(prepared.node_count(), encoded.node_count());
            let restored = replay(&plan, encoded.bytes());
            let FlowOp::StartNeedProducer {
                target: original_target,
                ..
            } = operation(&plan)
            else {
                unreachable!()
            };
            let FlowOp::StartNeedProducer {
                target: restored_target,
                ..
            } = operation(&restored)
            else {
                unreachable!()
            };
            assert_eq!(restored_target.plan(), original_target.plan());
            assert_eq!(restored_target.arguments(), original_target.arguments());
            assert!(
                !restored_target
                    .task_plan()
                    .same_candidate(original_target.task_plan())
            );
            assert!(plan.task_plan(restored_target.task_plan()).is_none());
            assert!(restored.task_plan(original_target.task_plan()).is_none());
            assert_eq!(
                restored.executable_semantic_digest(),
                plan.executable_semantic_digest()
            );
            assert_eq!(restored.task_plans(), plan.task_plans());
            let reencoded = operation(&restored)
                .encode_selected_producer_source(
                    &restored,
                    limits(&restored),
                    &resources(),
                    &mut meter(),
                )
                .unwrap();
            assert_eq!(reencoded.bytes(), encoded.bytes());
            let mut appended = Vec::with_capacity(encoded.bytes().len());
            reencoded.append_to(&mut appended).unwrap();
            assert_eq!(appended, encoded.bytes());
        }
    }
}

#[test]
fn selected_producer_source_rejects_foreign_task_custody_before_constructing_body_values() {
    let plan = fixture(
        2,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::Restartable,
        TaskClass::Io,
    );
    let encoded = operation(&plan)
        .encode_selected_producer_source(&plan, limits(&plan), &resources(), &mut meter())
        .unwrap();
    let (mut source, _, _) = source(&plan);
    let (_, _, foreign) = self::source(&plan);
    let prepared = PreparedRuntimeSelectedProducerSource::preflight(
        encoded.bytes(),
        limits(&plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert_eq!(
        prepared.materialize(&mut source, &foreign, &empty_handles(), &mut resources()),
        Err(Error::TaskCoordinate)
    );
    assert!(source.builder.function_sites[0].body.is_none());
    assert_eq!(
        source.meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    let alien = fixture(
        2,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::Restartable,
        TaskClass::Io,
    );
    assert!(matches!(
        operation(&plan).encode_selected_producer_source(
            &alien,
            limits(&alien),
            &resources(),
            &mut meter()
        ),
        Err(Error::TaskCoordinate)
    ));
}

#[test]
fn selected_producer_source_enforces_aggregate_shape_and_resource_limits_before_materialization() {
    let plan = fixture(
        2,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::Restartable,
        TaskClass::Io,
    );
    let encoded = operation(&plan)
        .encode_selected_producer_source(&plan, limits(&plan), &resources(), &mut meter())
        .unwrap();
    let mut exact = limits(&plan);
    exact.nodes = encoded.node_count() as u32;
    exact.elements = encoded.elements() as u32;
    exact.decoded_bytes = encoded.storage_bytes();
    exact.encoded_bytes = encoded.bytes().len();
    PreparedRuntimeSelectedProducerSource::preflight(
        encoded.bytes(),
        exact,
        &resources(),
        &mut meter(),
    )
    .unwrap();
    for field in 0..4 {
        let mut short = exact;
        match field {
            0 => short.nodes -= 1,
            1 => short.elements -= 1,
            2 => short.decoded_bytes -= 1,
            3 => short.encoded_bytes -= 1,
            _ => unreachable!(),
        }
        assert!(
            PreparedRuntimeSelectedProducerSource::preflight(
                encoded.bytes(),
                short,
                &resources(),
                &mut meter()
            )
            .is_err()
        );
        assert!(
            operation(&plan)
                .encode_selected_producer_source(&plan, short, &resources(), &mut meter())
                .is_err()
        );
    }
    for end in [0, DOMAIN.len(), DOMAIN.len() + 1, encoded.bytes().len() - 1] {
        assert!(
            PreparedRuntimeSelectedProducerSource::preflight(
                &encoded.bytes()[..end],
                exact,
                &resources(),
                &mut meter()
            )
            .is_err()
        );
    }
    let mut bytes = encoded.bytes().to_vec();
    bytes[DOMAIN.len()] = 2;
    assert!(matches!(
        PreparedRuntimeSelectedProducerSource::preflight(&bytes, exact, &resources(), &mut meter()),
        Err(Error::Invalid {
            kind: "version",
            ..
        })
    ));
    bytes[DOMAIN.len()] = 1;
    bytes[DOMAIN.len() + 1..DOMAIN.len() + 5].copy_from_slice(&1u32.to_le_bytes());
    assert!(matches!(
        PreparedRuntimeSelectedProducerSource::preflight(&bytes, exact, &resources(), &mut meter()),
        Err(Error::Reference { kind: "task", .. })
    ));
}

#[test]
fn selected_producer_source_rejects_conflicting_contracts_and_source_ordered_argument_names() {
    let plan = fixture(
        2,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::Restartable,
        TaskClass::Io,
    );
    let encoded = operation(&plan)
        .encode_selected_producer_source(&plan, limits(&plan), &resources(), &mut meter())
        .unwrap();
    let mut contract = encoded.bytes().to_vec();
    contract[DOMAIN.len() + 1 + 4] ^= 1;
    assert!(matches!(
        PreparedRuntimeSelectedProducerSource::preflight(
            &contract,
            limits(&plan),
            &resources(),
            &mut meter()
        ),
        Err(Error::Plan(NeedProducerPlanError::ExternalContractMismatch))
    ));
    let mut name = encoded.bytes().to_vec();
    let offset = name
        .windows("条件".len())
        .rposition(|bytes| bytes == "条件".as_bytes())
        .unwrap();
    name[offset..offset + "条件".len()].copy_from_slice("別名".as_bytes());
    assert!(matches!(
        PreparedRuntimeSelectedProducerSource::preflight(
            &name,
            limits(&plan),
            &resources(),
            &mut meter()
        ),
        Err(Error::Invalid {
            kind: "argument binding",
            ..
        })
    ));
    let mut output = Vec::new();
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert!(output.is_empty());
}
