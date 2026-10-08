use super::*;
use crate::plan::body_semantic::request::*;
use crate::plan::*;

#[derive(Clone, Copy)]
enum Transport {
    Binding,
    RetainedFormal,
}

impl Transport {
    fn local_source(self, identity: [u8; 32]) -> RuntimeLocalDeclarationSource {
        match self {
            Self::Binding => fixture_binding_source(identity, false),
            Self::RetainedFormal => RuntimeLocalDeclarationSource::Parameter(
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
    RuntimePlan,
    RuntimeTaskPlanCoordinateOwner,
    crate::runtime_id::RuntimeFunctionSiteId,
) {
    producer_host_plan_with_setup(true, |builder| {
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
    })
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
            &mut |_| Ok(owner.resolve(0).unwrap()),
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let endpoint = producer.endpoint(0).unwrap();
    let before = meter.totals();
    let actual = context
        .host_request_template_digest(&mut meter, endpoint, RuntimeTaskPlanSealLimits::default())
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
            &RuntimeTaskRequestTemplate::new(
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
