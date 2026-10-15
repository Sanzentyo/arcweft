use super::*;
use crate::plan::body_semantic::request::*;
use crate::plan::*;

#[derive(Clone, Copy)]
enum Transport {
    Binding,
    RetainedFormal,
    Formal,
}

impl Transport {
    fn local_source(self, identity: [u8; 32]) -> RuntimeLocalDeclarationSource {
        match self {
            Self::Binding => fixture_binding_source(identity, false),
            Self::RetainedFormal | Self::Formal => RuntimeLocalDeclarationSource::Parameter(
                RuntimeFunctionParameterIdentity::from_accepted_identity(identity),
            ),
        }
    }

    fn input(
        self,
        identity: [u8; 32],
        position: u32,
        local: RuntimeLocalSeedId,
    ) -> RuntimeFunctionInputBindingSeed {
        let (transfer, origin, source) = match self {
            Self::Binding => (
                RuntimeFunctionInputTransfer::Transferred(RuntimeFunctionCaptureMode::Copy),
                RuntimeFunctionInputOrigin::Binding(identity),
                RuntimeFunctionInputSource::Capture { position },
            ),
            Self::RetainedFormal => (
                RuntimeFunctionInputTransfer::Formal,
                RuntimeFunctionInputOrigin::Parameter(
                    RuntimeFunctionParameterIdentity::from_accepted_identity(identity),
                ),
                RuntimeFunctionInputSource::CapturedParameter {
                    position,
                    passing: RuntimeFunctionParameterPassing::Value,
                },
            ),
            Self::Formal => (
                RuntimeFunctionInputTransfer::Formal,
                RuntimeFunctionInputOrigin::Parameter(
                    RuntimeFunctionParameterIdentity::from_accepted_identity(identity),
                ),
                RuntimeFunctionInputSource::Parameter {
                    position,
                    passing: RuntimeFunctionParameterPassing::Value,
                },
            ),
        };
        RuntimeFunctionInputBindingSeed {
            transfer,
            origin,
            source,
            input_local: local.clone(),
            pattern: RuntimePatternSeed::new(
                RuntimeSemanticTypeId::from_bytes([61; 32]),
                RuntimePatternSeedKind::Bind {
                    mutable: false,
                    local,
                },
            ),
            ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
            unrestricted_bindings: Box::new([]),
        }
    }
}

fn fixture(
    count: u32,
    transport: Transport,
) -> (
    RuntimePlanInventory,
    RuntimeTaskPlanCoordinateOwner,
    crate::runtime_id::RuntimeFunctionSiteId,
) {
    producer_host_plan_with_setup(true, |builder| fixture_inputs(count, transport, builder))
}

fn fixture_inputs(
    count: u32,
    transport: Transport,
    builder: &mut RuntimePlanBuilder,
) -> (
    Vec<crate::plan::RuntimeHostArgumentSeed>,
    Box<[crate::plan::RuntimeFunctionInputBindingSeed]>,
) {
    let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
    let identities = (0..count)
        .map(|ordinal| {
            let mut identity = [31; 32];
            identity[..4].copy_from_slice(&ordinal.to_le_bytes());
            identity
        })
        .collect::<Vec<_>>();
    let locals = builder
        .admit_type_batch(
            [],
            identities.iter().map(|identity| {
                RuntimeLocalDeclarationSeed::new(transport.local_source(*identity), boolean)
            }),
        )
        .unwrap();
    let argument = |index: usize| {
        let mut identity = [42; 32];
        identity[..8].copy_from_slice(&u64::try_from(index).unwrap().to_le_bytes());
        RuntimeHostArgumentSeed::Positional(
            RuntimeRequestRoleIdentity::from_accepted_identity(identity),
            RuntimeExprSeed::new(
                boolean,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    locals.local_ids()[index].clone(),
                    crate::value::RuntimeLocalReadMode::Copy,
                )),
            ),
        )
    };
    let arguments = vec![argument(0), argument(usize::try_from(count).unwrap() - 1)];
    let inputs = locals
        .local_ids()
        .iter()
        .zip(identities)
        .enumerate()
        .map(|(ordinal, (local, identity))| {
            transport.input(identity, u32::try_from(ordinal).unwrap(), local.clone())
        })
        .collect();
    (arguments, inputs)
}

#[test]
fn original_builder_root_projection_matches_all_formal_and_capture_transports() {
    for transport in [
        Transport::Binding,
        Transport::RetainedFormal,
        Transport::Formal,
    ] {
        let (builder, function) = try_producer_request_builder_with_setup(true, None, |builder| {
            fixture_inputs(3, transport, builder)
        })
        .unwrap();
        let coordinate = builder
            .task_plan_construction()
            .reserve_original_source()
            .unwrap();
        let owner = builder.task_coordinate_owner(1);
        let limits = RuntimeTaskPlanSealLimits::default();
        assert!(matches!(
            builder.task_request_template_for_function_root(
                &function,
                RuntimeTaskPlanSealLimits {
                    max_request_roles: 2,
                    ..limits
                }
            ),
            Err(crate::plan::RuntimeTaskPlanSealError::Body(
                RuntimeBodySemanticError::RequestRoles {
                    actual: 3,
                    maximum: 2
                }
            ))
        ));
        assert!(matches!(
            builder.task_request_template_for_function_root(
                &function,
                RuntimeTaskPlanSealLimits {
                    max_semantic_work: 0,
                    ..limits
                }
            ),
            Err(crate::plan::RuntimeTaskPlanSealError::Body(
                RuntimeBodySemanticError::Encoding(
                    crate::task::semantic::TaskSemanticEncodingError::SemanticWork
                )
            ))
        ));
        let projected = builder
            .task_request_template_for_function_root(&function, limits)
            .unwrap();
        assert_eq!(projected.source(), RuntimeTaskRequestSource::FunctionRoot);
        assert_eq!(projected.arguments().len(), 3);
        assert!(coordinate.sealed_plan().is_none());
        let plan = builder.finish().unwrap();
        let context = RuntimeBodySemanticContext::new(&plan.inventory);
        let mut meter = TaskSemanticMeter::new(1_000_000, 10_000_000);
        let producer = context
            .producer_function(&mut meter, function.id(), &owner, limits)
            .unwrap();
        let endpoint = producer
            .request_endpoint(RuntimeTaskRequestSource::FunctionRoot)
            .unwrap();
        let admitted = context
            .static_request_template(&mut meter, endpoint, limits)
            .unwrap()
            .try_map_types(|ty| {
                plan.type_table()
                    .get(ty)
                    .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                    .ok_or(())
            })
            .unwrap();
        assert_eq!(projected, admitted);
        assert!(coordinate.sealed_plan().is_some());
    }
}

fn projection_cost(count: u32, transport: Transport) -> (u64, u64) {
    let (plan, owner, function) = fixture(count, transport);
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut meter = TaskSemanticMeter::new(1_000_000, 10_000_000);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            &owner,
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let endpoint = producer.endpoint(0).unwrap();
    let before = meter.totals();
    let actual = context
        .static_request_template_digest(&mut meter, endpoint, RuntimeTaskPlanSealLimits::default())
        .unwrap();
    let after = meter.totals();
    let arguments = endpoint
        .host_arguments()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(ordinal, argument)| RuntimeRequestArgument {
            role: RuntimeRequestArgumentRole::Positional,
            identity: None,
            ty: argument.value().ty(),
            source: RuntimeRequestValueSource::Capture,
            path: Box::new([RuntimeRequestPathStep::Operand(
                u32::try_from(ordinal).unwrap(),
            )]),
        })
        .collect::<Vec<_>>();
    let expected = context
        .request_template_digest(
            &mut meter,
            endpoint,
            &RuntimeTaskRequestTemplate::at_operation(
                endpoint.ordinal(),
                arguments.into_boxed_slice(),
                Box::new([]),
            ),
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    assert_eq!(actual, expected);
    (after.0 - before.0, after.1 - before.1)
}

#[test]
fn request_capture_projection_work_does_not_scan_unreferenced_inputs() {
    for transport in [Transport::Binding, Transport::RetainedFormal] {
        assert_eq!(
            projection_cost(2, transport),
            projection_cost(512, transport)
        );
    }
}

#[test]
fn function_root_request_uses_original_formals_and_captures_and_rechecks_decoded_roles() {
    use crate::plan::body_semantic::request::codec::RuntimeTaskRequestCodecLimits;
    for transport in [
        Transport::Binding,
        Transport::RetainedFormal,
        Transport::Formal,
    ] {
        let (plan, owner, function) = fixture(2, transport);
        let context = RuntimeBodySemanticContext::new(&plan);
        let limits = RuntimeTaskPlanSealLimits::default();
        let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
        let producer = context
            .producer_function(&mut meter, function, &owner, limits)
            .unwrap();
        let root = producer
            .request_endpoint(RuntimeTaskRequestSource::FunctionRoot)
            .unwrap();
        let template = context
            .static_request_template(&mut meter, root, limits)
            .unwrap();
        assert_eq!(template.source(), RuntimeTaskRequestSource::FunctionRoot);
        assert_eq!(template.arguments.len(), 2);
        for (ordinal, argument) in template.arguments.iter().enumerate() {
            let expected = match transport {
                Transport::Binding | Transport::RetainedFormal => (
                    RuntimeRequestArgumentRole::Capture,
                    RuntimeRequestValueSource::Capture,
                    RuntimeRequestPathStep::Capture(u32::try_from(ordinal).unwrap()),
                ),
                Transport::Formal => (
                    RuntimeRequestArgumentRole::Positional,
                    RuntimeRequestValueSource::Local,
                    RuntimeRequestPathStep::Operand(u32::try_from(ordinal).unwrap()),
                ),
            };
            assert_eq!(
                (&argument.role, &argument.source, argument.path.as_ref()),
                (&expected.0, &expected.1, std::slice::from_ref(&expected.2))
            );
        }
        let key = context
            .request_template_digest(&mut meter, root, &template, limits)
            .unwrap();
        let codec_limits = RuntimeTaskRequestCodecLimits {
            roles: 2,
            path_steps: 1,
            encoded_bytes: 1024,
        };
        let bytes = template.encode(codec_limits).unwrap();
        let mut decoded = RuntimeTaskRequestTemplate::decode(&bytes, codec_limits).unwrap();
        assert_eq!(decoded.encode(codec_limits).unwrap(), bytes);
        assert_eq!(
            context
                .request_template_digest(&mut meter, root, &decoded, limits)
                .unwrap(),
            key
        );
        assert_ne!(
            context
                .static_request_template_digest(&mut meter, producer.endpoint(0).unwrap(), limits)
                .unwrap(),
            key
        );
        decoded.arguments.swap(0, 1);
        assert!(matches!(
            context.request_template_digest(&mut meter, root, &decoded, limits),
            Err(RuntimeBodySemanticError::RequestTemplateMismatch {
                component: "argument definition",
                ordinal: 0,
                ..
            })
        ));
        assert_eq!(
            meter.status(),
            Err(crate::task::semantic::TaskSemanticEncodingError::OwnerRejected)
        );
    }
}

#[test]
fn function_root_request_rejects_an_equal_content_foreign_source_before_role_extraction() {
    let (original, owner, function) = fixture(2, Transport::Binding);
    let (foreign, _, _) = fixture(2, Transport::Binding);
    let context = RuntimeBodySemanticContext::new(&original);
    let limits = RuntimeTaskPlanSealLimits::default();
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = context
        .producer_function(&mut meter, function, &owner, limits)
        .unwrap();
    let root = producer
        .request_endpoint(RuntimeTaskRequestSource::FunctionRoot)
        .unwrap();
    let template = context
        .static_request_template(&mut meter, root, limits)
        .unwrap();
    let foreign_context = RuntimeBodySemanticContext::new(&foreign);
    let mut rejected = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        foreign_context.request_template_digest(&mut rejected, root, &template, limits),
        Err(RuntimeBodySemanticError::InvalidRequestSource)
    ));
    assert_eq!(rejected.totals(), (0, 0));
    assert_eq!(
        rejected.status(),
        Err(crate::task::semantic::TaskSemanticEncodingError::OwnerRejected)
    );
}
