use super::super::invocations::{
    PreparedRuntimeInvocationSources, RuntimeInvocationSourceLimits, tests as graph,
};
use super::super::pattern::tests::meter;
use super::*;
use crate::entry::*;
use crate::pattern::RuntimeCheckedType;
use crate::plan::*;

fn limits() -> RuntimeEntrySourceLimits {
    RuntimeEntrySourceLimits {
        entries: 100,
        routes: 100,
        commands: 100,
        schema_nodes: 100_000,
        schema_depth: 64,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 27,
    }
}
fn encode(plan: &RuntimePlan) -> Vec<u8> {
    let prepared = plan
        .prepare_entry_sources_encoding(limits(), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}
pub(in crate::plan::construction::source) fn restore(
    plan: &RuntimePlan,
    source: &mut RuntimePlanSourceConstruction,
) {
    let bytes = encode(plan);
    PreparedRuntimeEntrySources::preflight(&bytes, limits(), &mut source.meter)
        .unwrap()
        .materialize(source)
        .unwrap();
}
fn invocation(
    plan: &RuntimePlan,
    source: &mut RuntimePlanSourceConstruction,
    functions: &[RuntimeFunctionSiteSeedId],
) -> Box<[crate::runtime_id::RuntimeDeferSiteId]> {
    let limits = RuntimeInvocationSourceLimits {
        flows: 100,
        schemas: 100,
        flow_executables: 100,
        callables: 100,
        defers: 100,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 27,
        type_count: plan.type_table().len(),
        function_count: plan.function_sites().len(),
    };
    let prepared = plan
        .prepare_invocation_sources_encoding(limits, &mut source.meter)
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    PreparedRuntimeInvocationSources::preflight(&bytes, limits, &mut source.meter)
        .unwrap()
        .materialize(source, functions)
        .unwrap()
}
fn adapter_fixture() -> RuntimePlan {
    let payload = RuntimeTypeSchema::Tuple(Box::new([
        RuntimeTypeSchema::Bool,
        RuntimeTypeSchema::Seq(Box::new(RuntimeTypeSchema::option(
            RuntimeTypeSchema::String,
        ))),
    ]));
    let mut builder = crate::plan::body_semantic::stateful_root_builder_fixture(
        payload,
        RootExecutionLimits::engine_default(),
    );
    for (index, kind) in [RuntimeEntryKind::Editor, RuntimeEntryKind::Test]
        .into_iter()
        .enumerate()
    {
        let mut entry = builder.entries[0].clone();
        entry.id = EntryRuntimeId::canonical(&format!("stateful_{index}")).unwrap();
        entry.kind = kind;
        entry.binding = EntryBindingIdentity::from_bytes([30 + u8::try_from(index).unwrap(); 32]);
        let RuntimeEntryRoles::Stateful(role) = &mut entry.roles else {
            unreachable!("actual stateful owner fixture")
        };
        role.binding = entry.binding;
        builder.push_entry(entry).unwrap();
    }
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    let string = RuntimeCheckedType::String.semantic_identity_digest();
    let identity = RuntimeFunctionParameterIdentity::from_accepted_identity([13; 32]);
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(string, RuntimePlanTypeProjection::String),
            ],
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalDeclarationSource::Parameter(identity),
                string,
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    let direct = FlowRuntimeId::canonical("source_direct").unwrap();
    let route_target = FlowRuntimeId::canonical("source_routed").unwrap();
    for (index, flow) in [direct.clone(), route_target.clone()]
        .into_iter()
        .enumerate()
    {
        let inputs = if index == 0 {
            vec![]
        } else {
            vec![RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Formal,
                origin: RuntimeFunctionInputOrigin::Parameter(identity),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: RuntimeFunctionParameterPassing::Value,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    string,
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local: local.clone(),
                    },
                ),
                ownership: RuntimeValueOwnershipRequirement::Owned,
                unrestricted_bindings: Box::new([]),
            }]
        };
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: flow.clone(),
                parameters: if index == 0 {
                    vec![]
                } else {
                    vec![RuntimeFlowExecutableParameter {
                        identity,
                        coordinate: FlowParameterCoordinate::from_position(0),
                        name: "capture".into(),
                        mode: RuntimeFlowParameterMode::Owned,
                        passing: RuntimeFunctionParameterPassing::Value,
                        semantic_identity: string,
                    }]
                },
            })
            .unwrap();
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                flow.clone(),
                RuntimeFunctionSiteDeclarationSeed::flow(
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [14 + u8::try_from(index).unwrap(); 32],
                    ),
                    None,
                    inputs.into_boxed_slice(),
                    unit,
                    RuntimeEffectSet::empty(),
                ),
                RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([]),
                },
            ))
            .unwrap();
        builder
            .push_flow_executable(RuntimeFlowExecutable {
                flow,
                contract: FlowContractHash::from_bytes([16 + u8::try_from(index).unwrap(); 32]),
                controller: None,
            })
            .unwrap();
    }
    for (index, kind) in [
        RuntimeEntryKind::Cli,
        RuntimeEntryKind::Server,
        RuntimeEntryKind::Activity,
        RuntimeEntryKind::Bench,
        RuntimeEntryKind::Custom("独自アダプター".into()),
    ]
    .into_iter()
    .enumerate()
    {
        builder
            .push_entry(RuntimeEntrySpec {
                id: EntryRuntimeId::canonical(&format!("source_adapter_{index}")).unwrap(),
                kind,
                binding: EntryBindingIdentity::from_bytes([20 + u8::try_from(index).unwrap(); 32]),
                target: RuntimeEntryTarget::Flow(direct.clone()),
                roles: RuntimeEntryRoles::None,
            })
            .unwrap();
    }
    let routes = [
        RuntimeHttpMethod::Get,
        RuntimeHttpMethod::Post,
        RuntimeHttpMethod::Put,
        RuntimeHttpMethod::Patch,
        RuntimeHttpMethod::Delete,
        RuntimeHttpMethod::Head,
        RuntimeHttpMethod::Options,
    ]
    .into_iter()
    .map(|method| RuntimeRouteSpec {
        method,
        path: RuntimeRoutePath::try_new(vec![
            RuntimeRoutePathSegment::Literal("source_route".into()),
            RuntimeRoutePathSegment::Capture(RouteCaptureCoordinate::from_position(0)),
        ])
        .unwrap(),
        target: route_target.clone(),
        bindings: vec![RuntimeRouteBinding {
            parameter: FlowParameterCoordinate::from_position(0),
            source: RuntimeRouteBindingSource::PathCapture(RouteCaptureCoordinate::from_position(
                0,
            )),
        }],
    })
    .collect();
    builder
        .push_entry(RuntimeEntrySpec {
            id: EntryRuntimeId::canonical("source_server").unwrap(),
            kind: RuntimeEntryKind::Server,
            binding: EntryBindingIdentity::from_bytes([26; 32]),
            target: RuntimeEntryTarget::Routes(routes),
            roles: RuntimeEntryRoles::None,
        })
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn entry_sources_restore_all_kinds_targets_roles_schemas_routes_and_budgets_in_the_actual_program_seal()
 {
    for plan in [graph::fixture(), adapter_fixture()] {
        let bytes = encode(&plan);
        let (mut source, functions, helpers) = graph::source(&plan);
        let defers = invocation(&plan, &mut source, &functions);
        let encoded = plan
            .prepare_entry_sources_encoding(limits(), &mut meter())
            .unwrap();
        let prepared =
            PreparedRuntimeEntrySources::preflight(&bytes, limits(), &mut source.meter).unwrap();
        assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
        assert_eq!(prepared.elements(), encoded.elements());
        prepared.materialize(&mut source).unwrap();
        graph::define_bodies(&plan, &mut source, &functions, &helpers, &defers);
        let rebuilt = graph::finish(source);
        assert_eq!(rebuilt.entries(), plan.entries());
        assert_eq!(
            rebuilt.executable_semantic_digest(),
            plan.executable_semantic_digest()
        );
        assert_eq!(encode(&rebuilt), bytes);
    }
}

#[test]
fn entry_sources_admit_exact_cumulative_schema_and_metadata_resources_and_keep_the_real_bank_failed()
 {
    let plan = adapter_fixture();
    let bytes = encode(&plan);
    let prepared = PreparedRuntimeEntrySources::preflight(&bytes, limits(), &mut meter()).unwrap();
    let exact = RuntimeEntrySourceLimits {
        entries: u32::try_from(plan.entries().len()).unwrap(),
        routes: 7,
        commands: 3,
        schema_nodes: u32::try_from(prepared.footprint.schemas).unwrap(),
        schema_depth: 4,
        elements: u32::try_from(prepared.elements()).unwrap(),
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
    };
    PreparedRuntimeEntrySources::preflight(&bytes, exact, &mut meter()).unwrap();
    plan.prepare_entry_sources_encoding(exact, &mut meter())
        .unwrap();
    for short in [
        RuntimeEntrySourceLimits {
            entries: exact.entries - 1,
            ..exact
        },
        RuntimeEntrySourceLimits { routes: 6, ..exact },
        RuntimeEntrySourceLimits {
            commands: 2,
            ..exact
        },
        RuntimeEntrySourceLimits {
            schema_nodes: exact.schema_nodes - 1,
            ..exact
        },
        RuntimeEntrySourceLimits {
            schema_depth: 3,
            ..exact
        },
        RuntimeEntrySourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeEntrySourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeEntrySourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = meter();
        assert!(PreparedRuntimeEntrySources::preflight(&bytes, short, &mut bank).is_err());
        assert!(PreparedRuntimeEntrySources::preflight(&bytes, exact, &mut bank).is_err());
        assert!(
            plan.prepare_entry_sources_encoding(short, &mut meter())
                .is_err()
        );
    }
    let (mut source, _, _) = graph::source(&plan);
    restore(&plan, &mut source);
    let prepared =
        PreparedRuntimeEntrySources::preflight(&bytes, exact, &mut source.meter).unwrap();
    assert!(matches!(
        prepared.materialize(&mut source),
        Err(Error::ExistingEntries)
    ));
    assert_eq!(source.builder.entries.len(), plan.entries().len());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
    let mut bank = TaskSemanticMeter::new(0, u64::MAX);
    assert!(matches!(
        PreparedRuntimeEntrySources::preflight(&bytes, exact, &mut bank),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
}

#[test]
fn entry_sources_reject_all_truncations_versions_tags_utf8_and_noncanonical_routes_before_construction()
 {
    let plan = adapter_fixture();
    let bytes = encode(&plan);
    for end in 0..bytes.len() {
        assert!(
            PreparedRuntimeEntrySources::preflight(&bytes[..end], limits(), &mut meter()).is_err(),
            "prefix {end}"
        );
    }
    let first_id = plan.entries()[0].id.path().segments()[0].as_str();
    let kind = DOMAIN.len() + 1 + 4 + 4 + 4 + first_id.len();
    for (offset, value) in [(0, 0), (DOMAIN.len(), 2), (kind, 255), (kind + 1 + 32, 255)] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(PreparedRuntimeEntrySources::preflight(&bad, limits(), &mut meter()).is_err());
    }
    let mut bad = bytes.clone();
    bad.push(0);
    assert!(matches!(
        PreparedRuntimeEntrySources::preflight(&bad, limits(), &mut meter()),
        Err(Error::Trailing { .. })
    ));
    let mut bad = bytes.clone();
    let text = "独自アダプター".as_bytes();
    let offset = bad
        .windows(text.len())
        .position(|window| window == text)
        .unwrap();
    bad[offset] = 255;
    assert!(matches!(
        PreparedRuntimeEntrySources::preflight(&bad, limits(), &mut meter()),
        Err(Error::Utf8(_))
    ));
    let mut bad = bytes.clone();
    let text = b"source_route";
    let offset = bad
        .windows(text.len())
        .position(|window| window == text)
        .unwrap()
        + text.len()
        + 1;
    bad[offset..offset + 4].copy_from_slice(&1u32.to_le_bytes());
    assert!(matches!(
        PreparedRuntimeEntrySources::preflight(&bad, limits(), &mut meter()),
        Err(Error::Route(RuntimeRoutePathError::InvalidCapture))
    ));
    let prepared = plan
        .prepare_entry_sources_encoding(limits(), &mut meter())
        .unwrap();
    let mut output = vec![42];
    assert!(matches!(
        prepared.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [42]);
}

#[test]
fn entry_sources_leave_layout_and_role_authority_with_the_actual_common_builder_verifier() {
    let plan = adapter_fixture();
    let mut bytes = encode(&plan);
    let RuntimeEntryRoles::Stateful(role) = &plan.entries()[0].roles else {
        unreachable!()
    };
    let expected = role.command_policy.admitted[0].payload_layout.as_bytes();
    let offset = bytes
        .windows(expected.len())
        .position(|window| window == expected)
        .unwrap();
    bytes[offset] ^= 1;
    let (mut source, functions, helpers) = graph::source(&plan);
    let defers = invocation(&plan, &mut source, &functions);
    PreparedRuntimeEntrySources::preflight(&bytes, limits(), &mut source.meter)
        .unwrap()
        .materialize(&mut source)
        .unwrap();
    graph::define_bodies(&plan, &mut source, &functions, &helpers, &defers);
    let error = source.builder.finish().unwrap_err();
    assert!(matches!(
        error,
        RuntimePlanBuildError::Plan(RuntimePlanError::CommandLayoutMismatch { entry, constructor })
            if entry == "stateful" && constructor == "set"
    ));
}
