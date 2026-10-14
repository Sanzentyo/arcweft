//! Request proof uses actual Thread and `AwaitMany` rows from the aggregate
//! builder, including each producer's original executable/outcome authority.

use super::*;
use crate::plan::*;
use crate::task::*;

fn producer(
    marker: u8,
    family: NeedProducerFamily,
    payload: RuntimeSemanticTypeId,
    arguments: Vec<RuntimeHostArgumentSeed>,
) -> RuntimeNeedProducerTemplateSeed {
    RuntimeNeedProducerTemplateSeed {
        definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([marker; 32]),
        family,
        contract: NeedProducerContractDigest::from_bytes([marker; 32]),
        plan: TaskPlanSemanticDigest::from_bytes([marker; 32]),
        producer_site: NeedProducerSiteDigest::from_bytes([marker; 32]),
        payload_type: RuntimeTypeSemanticDigest::from_bytes(*payload.as_bytes()),
        class: TaskClass::Cpu,
        priority: TaskPriority::default(),
        cancel_scope: CancelScopeId("test".to_owned()),
        policy: TaskPolicy::AlwaysStart,
        outcome: TaskOutcomeContract::program(payload),
        request: RuntimeHostTaskRequestTemplateSeed {
            capability: HostCapabilityId("test".to_owned()),
            operation: "request".to_owned(),
            args: arguments,
        },
        debug_label: "request endpoint".to_owned(),
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

fn fixture(await_many: bool) -> (RuntimePlan, crate::plan::RuntimeTaskPlanCoordinateOwner) {
    let mut builder = RuntimePlanBuilder::new();
    let unit = RuntimeSemanticTypeId::from_bytes([62; 32]);
    let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
    let sequence = RuntimeSemanticTypeId::from_bytes([64; 32]);
    let locals = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    sequence,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: boolean,
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
                    73,
                    NeedProducerFamily::AwaitManyBase,
                    sequence,
                    argument(91, source),
                ),
                child: producer(
                    74,
                    NeedProducerFamily::AwaitManyChild,
                    unit,
                    argument(92, scalar),
                ),
            },
            pending: vec![],
        }
    } else {
        RuntimeFlowOpSeed::Thread {
            name: None,
            captures: vec![],
            producer: producer(
                74,
                NeedProducerFamily::StructuredTaskPlan,
                unit,
                argument(92, scalar),
            ),
            body: vec![RuntimeFlowOpSeed::ReturnExpr(returned.clone())],
        }
    };
    builder
        .define_function_site_seed(
            &root,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([operation, RuntimeFlowOpSeed::ReturnExpr(returned)]),
            }),
        )
        .unwrap();
    let owner = builder.task_coordinate_owner(if await_many { 2 } else { 1 });
    (builder.finish().unwrap(), owner)
}

#[test]
fn thread_and_both_await_many_endpoints_use_their_own_request_definition() {
    use crate::plan::body_semantic::request::{
        RuntimeTaskRequestTemplate, codec::RuntimeTaskRequestCodecLimits,
    };
    for await_many in [false, true] {
        let (plan, owner) = fixture(await_many);
        let context = RuntimeBodySemanticContext::new(&plan);
        let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
        let function =
            crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(NonZeroU32::MIN);
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
        let proof =
            context
                .producer_function(
                    &mut meter,
                    function,
                    &owner,
                    &mut |source| {
                        let crate::plan::body_semantic::flow::RuntimeBodyTaskSource::Producer(
                            producer,
                        ) = source
                        else {
                            panic!("actual producer")
                        };
                        Ok(owner
                            .resolve(u32::from(
                                producer.family == NeedProducerFamily::AwaitManyChild,
                            ))
                            .unwrap())
                    },
                    RuntimeTaskPlanSealLimits::default(),
                )
                .unwrap();
        let mut digests = Vec::new();
        for ordinal in 0..owner.len() {
            let endpoint = proof.endpoint(ordinal).unwrap();
            let original = context
                .host_request_template(&mut meter, endpoint, RuntimeTaskPlanSealLimits::default())
                .unwrap();
            let limits = RuntimeTaskRequestCodecLimits {
                roles: 1,
                path_steps: 1,
                encoded_bytes: 1000,
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
