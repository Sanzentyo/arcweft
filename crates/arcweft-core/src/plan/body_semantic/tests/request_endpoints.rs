//! Request proof uses actual Thread and `AwaitMany` rows from the aggregate
//! builder, including each producer's original executable/outcome authority.

use super::*;
use crate::plan::*;
use crate::task::*;

fn producer_definition(
    coordinate: RuntimeTaskPlanBuildCoordinate,
    marker: u8,
    family: NeedProducerFamily,
    handle_type: TaskHandleType,
) -> NeedProducerDefinition {
    NeedProducerDefinition {
        function: RuntimeFunctionDefinitionIdentity::from_accepted_identity([marker; 32]),
        family: family,
        contract: NeedProducerContractDigest::from_bytes([marker; 32]),
        plan: coordinate,
        producer_site: NeedProducerSiteDigest::from_bytes([marker; 32]),
        handle_type,
        class: TaskClass::Cpu,
        priority: TaskPriority::default(),
        cancel_scope: CancelScopeId("test".to_owned()),
        policy: TaskPolicy::AlwaysStart,
        debug_label: "request endpoint".to_owned(),
    }
}

fn producer(
    coordinate: RuntimeTaskPlanBuildCoordinate,
    marker: u8,
    family: NeedProducerFamily,
    payload: RuntimeSemanticTypeId,
    arguments: Vec<RuntimeHostArgumentSeed>,
) -> RuntimeNeedProducerTemplateSeed {
    RuntimeNeedProducerTemplateSeed {
        definition: producer_definition(
            coordinate,
            marker,
            family,
            TaskHandleType::Need {
                payload: RuntimeTypeSemanticDigest::from_bytes(*payload.as_bytes()),
            },
        ),
        request: RuntimeHostTaskRequestTemplateSeed {
            capability: HostCapabilityId("test".to_owned()),
            operation: "request".to_owned(),
            args: arguments,
        },
    }
}

fn declaration(marker: u8, result: RuntimeSemanticTypeId) -> RuntimeFunctionSiteDeclarationSeed {
    RuntimeFunctionSiteDeclarationSeed {
        definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([marker; 32]),
        role: RuntimeFunctionSemanticRole::Ordinary,
        function_type: None,
        inputs: Box::new([]),
        result,
        body_kind: RuntimeFunctionSiteBodyKind::Executable,
        effects: RuntimeEffectSet::empty(),
    }
}

fn define_returning(
    builder: &mut RuntimePlanBuilder,
    marker: u8,
    result: RuntimeSemanticTypeId,
    value: RuntimeExprSeed,
) {
    let function = builder
        .reserve_function_site_seed(declaration(marker, result))
        .unwrap();
    builder
        .define_function_site_seed(
            &function,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(value)]),
            }),
        )
        .unwrap();
}

fn fixture(
    await_many: bool,
    repeat_source: bool,
) -> (
    RuntimePlanInventory,
    crate::plan::RuntimeTaskPlanCoordinateOwner,
    crate::runtime_id::RuntimeFunctionSiteId,
) {
    let mut builder = RuntimePlanBuilder::new();
    let unit = RuntimeSemanticTypeId::from_bytes([62; 32]);
    let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
    let handle = RuntimeSemanticTypeId::from_bytes([63; 32]);
    let sequence = RuntimeSemanticTypeId::from_bytes([64; 32]);
    let aggregate = RuntimeSemanticTypeId::from_bytes([65; 32]);
    let locals = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(handle, RuntimePlanTypeProjection::ThreadHandle(unit)),
                RuntimePlanTypeSeed::new(
                    sequence,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: boolean,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    aggregate,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: unit,
                    },
                ),
            ],
            if await_many {
                vec![RuntimeLocalDeclarationSeed::new(
                    fixture_binding_source([81; 32], false),
                    boolean,
                )]
            } else {
                vec![]
            },
        )
        .unwrap();
    let root = builder
        .reserve_function_site_seed(declaration(72, unit))
        .unwrap();
    let scalar = RuntimeExprSeed::new(
        boolean,
        RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
    );
    let source = RuntimeExprSeed::new(
        sequence,
        RuntimeExprSeedKind::BracketSeq(Box::new([scalar.clone()])),
    );
    let returned = RuntimeExprSeed::new(
        unit,
        RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Unit),
    );
    for (marker, result, value) in [(73, sequence, source.clone()), (74, unit, returned.clone())] {
        define_returning(&mut builder, marker, result, value);
    }
    let argument = |marker, value| {
        vec![RuntimeHostArgumentSeed::Positional(
            RuntimeRequestRoleIdentity::from_accepted_identity([marker; 32]),
            value,
        )]
    };
    let operation = if await_many {
        RuntimeFlowOpSeed::AwaitMany {
            binding: None,
            target: RuntimeAwaitManyTargetSeed {
                source: source.clone(),
                item_binding: locals.local_ids()[0].clone(),
                limit: 2,
                base: producer(
                    builder.task_coordinate_owner(2).resolve(0).unwrap(),
                    73,
                    NeedProducerFamily::AwaitManyBase,
                    aggregate,
                    argument(91, source),
                ),
                child: producer(
                    builder.task_coordinate_owner(2).resolve(1).unwrap(),
                    74,
                    NeedProducerFamily::AwaitManyChild,
                    unit,
                    argument(92, scalar),
                ),
            },
            pending: vec![],
        }
    } else {
        RuntimeFlowOpSeed::ProgramTask {
            template: crate::plan::RuntimeProgramTaskTemplate {
                delivery: crate::task::ProgramTaskDelivery::HostMetadata,
                captures: vec![],
                producer: producer_definition(
                    builder.task_coordinate_owner(1).resolve(0).unwrap(),
                    74,
                    NeedProducerFamily::StructuredTaskPlan,
                    TaskHandleType::ThreadHandle {
                        payload: RuntimeTypeSemanticDigest::from_bytes(*unit.as_bytes()),
                    },
                ),
                body: vec![RuntimeFlowOpSeed::ReturnExpr(returned.clone())],
            },
            name: None,
            binding: RuntimePatternSeed::new(handle, RuntimePatternSeedKind::Discard),
        }
    };
    builder
        .define_function_site_seed(
            &root,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: if repeat_source {
                    vec![
                        operation.clone(),
                        operation,
                        RuntimeFlowOpSeed::ReturnExpr(returned),
                    ]
                    .into_boxed_slice()
                } else {
                    Box::new([operation, RuntimeFlowOpSeed::ReturnExpr(returned)])
                },
            }),
        )
        .unwrap();
    let owner = builder.task_coordinate_owner(if await_many { 2 } else { 1 });
    (
        builder
            .prepare_inventory(RuntimeTaskPlanSealLimits::default(), owner.len())
            .unwrap(),
        owner,
        root.id(),
    )
}

#[test]
fn thread_and_both_await_many_endpoints_use_their_own_request_definition() {
    use crate::plan::body_semantic::request::{
        RuntimeTaskRequestTemplate, codec::RuntimeTaskRequestCodecLimits,
    };
    for await_many in [false, true] {
        let (plan, owner, function) = fixture(await_many, false);
        let context = RuntimeBodySemanticContext::new(&plan);
        let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
        if await_many {
            let RuntimeFunctionSiteBody::Executable(body) =
                plan.function_sites().get(function).unwrap().body()
            else {
                panic!("actual executable");
            };
            let FlowOp::AwaitMany { target, .. } = &body.ops()[0] else {
                panic!("actual AwaitMany");
            };
            let placement = plan
                .local_declarations()
                .get(target.item_binding)
                .unwrap()
                .placement();
            assert_eq!(placement.owner(), RuntimeLocalOwner::Function(function));
            assert_eq!(
                placement.initialization(),
                RuntimeLocalInitialization::AwaitManyItem
            );
            assert!(!placement.is_mutable());
        }
        let proof = context
            .producer_function(
                &mut meter,
                function,
                &owner,
                RuntimeTaskPlanSealLimits::default(),
            )
            .unwrap();
        let mut digests = Vec::new();
        for ordinal in 0..owner.len() {
            let endpoint = proof.endpoint(ordinal).unwrap();
            let original = context
                .static_request_template(&mut meter, endpoint, RuntimeTaskPlanSealLimits::default())
                .unwrap();
            let limits = RuntimeTaskRequestCodecLimits {
                roles: 1,
                path_steps: 1,
                encoded_bytes: 1000,
                decoded_bytes: 16_384,
                type_count: plan.type_table().len(),
            };
            let inert =
                RuntimeTaskRequestTemplate::decode(&original.encode(limits).unwrap(), limits)
                    .unwrap();
            digests.push(
                context
                    .request_template_digest(
                        &mut meter,
                        endpoint,
                        &inert,
                        RuntimeTaskPlanSealLimits::default(),
                    )
                    .unwrap(),
            );
        }
        if await_many {
            assert_ne!(digests[0], digests[1]);
        }
    }
}

#[test]
fn original_coordinate_projection_uses_the_same_source_metadata_before_f_completion() {
    let limits = RuntimeTaskPlanSealLimits::default();
    for await_many in [false, true] {
        let (plan, owner, function) = fixture(await_many, false);
        let context = RuntimeBodySemanticContext::new(&plan);
        for ordinal in 0..owner.len() {
            let coordinate = owner.resolve(ordinal).unwrap();
            let mut source_meter = TaskSemanticMeter::new(100_000, 1_000_000);
            let original = context
                .original_task_endpoint(&mut source_meter, function, &coordinate, &owner, limits)
                .unwrap();
            assert_eq!(original.task_source().unwrap().coordinate(), &coordinate);
            let source_template = context
                .static_request_template(&mut source_meter, original, limits)
                .unwrap();
            assert_eq!(
                source_meter.totals().1,
                0,
                "source projection issues no semantic transcript"
            );
            let mut proof_meter = TaskSemanticMeter::new(100_000, 1_000_000);
            let completed = context
                .producer_function(&mut proof_meter, function, &owner, limits)
                .unwrap();
            let endpoint = completed.endpoint(ordinal).unwrap();
            let checked_template = context
                .static_request_template(&mut proof_meter, endpoint, limits)
                .unwrap();
            assert_eq!(source_template, checked_template);
            assert_eq!(original.request_source(), endpoint.request_source());
        }
    }
}

#[test]
fn source_projection_rejects_foreign_namespaces_before_work_or_allocation() {
    let (plan, owner, function) = fixture(true, false);
    let (_, foreign, _) = fixture(true, false);
    let coordinate = foreign.resolve(0).unwrap();
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).original_task_endpoint(
            &mut meter,
            function,
            &coordinate,
            &owner,
            RuntimeTaskPlanSealLimits::default(),
        ),
        Err(RuntimeBodySemanticError::ForeignTaskCoordinate)
    ));
    assert_eq!(meter.totals(), (0, 0));
    assert_eq!(
        meter.status(),
        Err(crate::task::semantic::TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn source_projection_requires_one_exact_original_endpoint() {
    let limits = RuntimeTaskPlanSealLimits::default();
    let (plan, owner, function) = fixture(false, false);
    let (other, _) = plan
        .function_sites()
        .iter_with_ids()
        .find(|(id, _)| *id != function)
        .unwrap();
    let coordinate = owner.resolve(0).unwrap();
    let mut missing = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(
        matches!(RuntimeBodySemanticContext::new(&plan).original_task_endpoint(
        &mut missing, other, &coordinate, &owner, limits,
    ), Err(RuntimeBodySemanticError::MissingOriginalRequestEndpoint { function: rejected }) if rejected == other)
    );
    assert_eq!(missing.totals().1, 0);
    assert_eq!(
        missing.status(),
        Err(crate::task::semantic::TaskSemanticEncodingError::OwnerRejected)
    );
    let (plan, owner, function) = fixture(false, true);
    let coordinate = owner.resolve(0).unwrap();
    let mut ambiguous = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(
        matches!(RuntimeBodySemanticContext::new(&plan).original_task_endpoint(
        &mut ambiguous, function, &coordinate, &owner, limits,
    ), Err(RuntimeBodySemanticError::AmbiguousOriginalRequestEndpoint { function: rejected }) if rejected == function)
    );
    assert_eq!(ambiguous.totals().1, 0);
    assert_eq!(
        ambiguous.status(),
        Err(crate::task::semantic::TaskSemanticEncodingError::OwnerRejected)
    );
}
