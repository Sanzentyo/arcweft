use super::super::expression::RuntimeExpressionSourceHandles;
use super::super::functions::RuntimeFunctionSourceLimits;
use super::super::pattern::tests::{declarations, frames, meter, resources};
use super::super::pure_callables::{
    PreparedRuntimePureCallableSources, RuntimePureCallableSourceLimits,
};
use super::*;
use crate::entry::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::*;
use crate::value::{RuntimeLocalReadMode, RuntimeValue};

const BOOL: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([221; 32]);
const UNIT: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([222; 32]);
fn value(ty: RuntimeSemanticTypeId, value: RuntimeValue) -> RuntimeExprSeed {
    RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(value))
}
fn callable(tag: u8) -> RuntimeCallableId {
    RuntimeCallableId::from_checked_digest([tag; 32])
}
fn limits(plan: &RuntimePlan) -> RuntimeInvocationSourceLimits {
    RuntimeInvocationSourceLimits {
        flows: 100,
        schemas: 100,
        flow_executables: 100,
        callables: 100,
        defers: 100,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
        function_count: plan.function_sites().len(),
    }
}
pub(in crate::plan::construction::source) fn fixture() -> RuntimePlan {
    use super::super::callable_states::tests as graph;
    let mut builder = crate::plan::body_semantic::stateful_root_builder_fixture(
        RuntimeTypeSchema::Bool,
        RootExecutionLimits::engine_default(),
    );
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(BOOL, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(UNIT, RuntimePlanTypeProjection::Unit),
            ],
            [],
        )
        .unwrap();
    let function = graph::reserve_function(&mut builder, 223, &[], 0, BOOL);
    builder
        .define_function_site_seed(&function, value(BOOL, RuntimeValue::Bool(true)))
        .unwrap();
    let deferred = graph::reserve_executable_function(&mut builder, 224, &[BOOL], 1, UNIT);
    builder
        .define_function_site_seed(
            &deferred,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(value(
                    UNIT,
                    RuntimeValue::Unit,
                ))]),
            }),
        )
        .unwrap();
    let defer = builder.reserve_defer_site_seed(&deferred).unwrap();
    let controller = RuntimeCallableRole {
        callable: callable(230),
        contract: CallableContractHash::from_bytes([231; 32]),
    };
    let flow_ids = [
        FlowRuntimeId::canonical("z").unwrap(),
        FlowRuntimeId::from_checked_declaration_digest([226; 32], "flow.source_opening").unwrap(),
        FlowRuntimeId::for_agent_controller_callable(&controller.callable),
        FlowRuntimeId::from_source_entity_body("flow.flow.nested").unwrap(),
    ];
    for (index, id) in flow_ids.iter().enumerate() {
        let parameter =
            RuntimeFunctionParameterIdentity::from_accepted_identity([232 + index as u8; 32]);
        let admitted = builder
            .admit_type_batch(
                [],
                [RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Parameter(parameter),
                    BOOL,
                )],
            )
            .unwrap();
        let local = admitted.local_ids()[0].clone();
        let passing = [
            RuntimeFunctionParameterPassing::Value,
            RuntimeFunctionParameterPassing::Shared,
            RuntimeFunctionParameterPassing::Affine,
        ][index % 3];
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: id.clone(),
                parameters: vec![RuntimeFlowExecutableParameter {
                    identity: parameter,
                    coordinate: FlowParameterCoordinate::from_position(0),
                    name: format!("入力_{index}"),
                    mode: RuntimeFlowParameterMode::Owned,
                    passing,
                    semantic_identity: BOOL,
                }],
            })
            .unwrap();
        let read = || {
            RuntimeExprSeed::new(
                BOOL,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    local.clone(),
                    RuntimeLocalReadMode::Copy,
                )),
            )
        };
        let mut ops = vec![];
        if index == 0 {
            ops.push(RuntimeFlowOpSeed::RegisterDefer {
                site: defer,
                outcome: crate::line_task::RuntimeDeferOutcomeFilter::Always,
                captures: vec![read()],
                owner: RuntimeDeferOwner::CurrentScope,
            });
        }
        ops.push(RuntimeFlowOpSeed::ReturnExpr(read()));
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                id.clone(),
                RuntimeFunctionSiteDeclarationSeed::flow(
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [236 + index as u8; 32],
                    ),
                    None,
                    Box::new([RuntimeFunctionInputBindingSeed {
                        transfer: RuntimeFunctionInputTransfer::Formal,
                        origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                        source: RuntimeFunctionInputSource::Parameter {
                            position: 0,
                            passing,
                        },
                        input_local: local.clone(),
                        pattern: RuntimePatternSeed::new(
                            BOOL,
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: local.clone(),
                            },
                        ),
                        ownership: RuntimeValueOwnershipRequirement::Owned,
                        unrestricted_bindings: Box::new([]),
                    }]),
                    BOOL,
                    RuntimeEffectSet::empty(),
                ),
                RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: ops.into_boxed_slice(),
                },
            ))
            .unwrap();
        builder
            .push_flow_executable(RuntimeFlowExecutable {
                flow: id.clone(),
                contract: FlowContractHash::from_bytes([240 + index as u8; 32]),
                controller: (index == 2).then(|| controller.clone()),
            })
            .unwrap();
        let binding = EntryBindingIdentity::from_bytes([248 + index as u8; 32]);
        builder
            .push_entry(RuntimeEntrySpec {
                id: EntryRuntimeId::canonical(&format!("source_entry_{index}")).unwrap(),
                kind: if index == 2 {
                    RuntimeEntryKind::Agent
                } else {
                    RuntimeEntryKind::Bench
                },
                binding,
                target: if index == 2 {
                    RuntimeEntryTarget::Controller(id.clone())
                } else {
                    RuntimeEntryTarget::Flow(id.clone())
                },
                roles: if index == 2 {
                    RuntimeEntryRoles::Agent(Box::new(RuntimeAgentEntryRoles {
                        binding,
                        controller: controller.clone(),
                        policy: AgentPolicyHash::from_bytes([252; 32]),
                        budget: AgentBudget::default(),
                    }))
                } else {
                    RuntimeEntryRoles::None
                },
            })
            .unwrap();
    }
    builder
        .push_callable_executable_seed(RuntimeCallableExecutableSeed {
            callable: controller.callable,
            contract: controller.contract,
            code: RuntimeCallableExecutableSeedCode::ControllerFlow(flow_ids[2].clone()),
        })
        .unwrap();
    builder.finish().unwrap()
}
fn encode(plan: &RuntimePlan) -> Vec<u8> {
    let prepared = plan
        .prepare_invocation_sources_encoding(limits(plan), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut output).unwrap();
    output
}
fn source_with_bank(
    plan: &RuntimePlan,
    mut bank: TaskSemanticMeter,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    Box<[RuntimePureHelperSeedId]>,
) {
    let declaration_bytes = frames(plan);
    let declarations = declarations(&declaration_bytes, &mut bank);
    let function_limits = RuntimeFunctionSourceLimits {
        rows: 100,
        elements: 100_000,
        pattern_nodes: 10_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
        local_count: plan.local_declarations().len(),
    };
    let encoded = plan
        .function_sites()
        .prepare_declaration_sources_encoding(function_limits, &resources(), &mut bank)
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
    encoded.append_to(&mut bytes).unwrap();
    let functions = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        function_limits,
        &resources(),
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let functions = functions.reserve(&mut source, &mut resources()).unwrap();
    let pure_limits = RuntimePureCallableSourceLimits {
        helpers: 100,
        methods: 100,
        bindings: 100,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
        local_count: plan.local_declarations().len(),
        function_count: functions.len(),
    };
    let encoded = plan
        .prepare_pure_callable_declaration_sources_encoding(pure_limits, &mut source.meter)
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
    encoded.append_to(&mut bytes).unwrap();
    let helpers =
        PreparedRuntimePureCallableSources::preflight(&bytes, pure_limits, &mut source.meter)
            .unwrap()
            .reserve(&mut source, &functions)
            .unwrap();
    assert!(helpers.methods.is_empty());
    (source, functions, helpers.helpers)
}
pub(in crate::plan::construction::source) fn source(
    plan: &RuntimePlan,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    Box<[RuntimePureHelperSeedId]>,
) {
    source_with_bank(plan, meter())
}
pub(in crate::plan::construction::source) fn define_bodies(
    plan: &RuntimePlan,
    source: &mut RuntimePlanSourceConstruction,
    functions: &[RuntimeFunctionSiteSeedId],
    helpers: &[RuntimePureHelperSeedId],
    defers: &[RuntimeDeferSiteId],
) {
    let handles = RuntimeExpressionSourceHandles {
        states: &[],
        specializations: &[],
        helpers,
        methods: &[],
        attempts: &[],
    };
    let flow_handles = super::super::flow::RuntimeFlowSourceHandles {
        expression: &handles,
        contents: &[],
        defers,
        tasks: &[],
    };
    super::super::function_bodies::tests::restore(plan, source, functions, &flow_handles);
}
pub(in crate::plan::construction::source) fn finish(
    source: RuntimePlanSourceConstruction,
) -> RuntimePlan {
    let seal = RuntimeTaskPlanSealLimits::default();
    source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan()
}
#[test]
fn invocation_sources_restore_original_flow_schema_roles_code_links_and_defer_captures_then_seal() {
    let plan = fixture();
    let bytes = encode(&plan);
    let encoded = plan
        .prepare_invocation_sources_encoding(limits(&plan), &mut meter())
        .unwrap();
    let (mut source, functions, helpers) = source(&plan);
    let prepared =
        PreparedRuntimeInvocationSources::preflight(&bytes, limits(&plan), &mut source.meter)
            .unwrap();
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    assert_eq!(prepared.elements(), encoded.elements());
    let defers = prepared.materialize(&mut source, &functions).unwrap();
    assert_eq!(defers.len(), plan.defer_sites().len());
    super::super::entries::tests::restore(&plan, &mut source);
    define_bodies(&plan, &mut source, &functions, &helpers, &defers);
    let restored = finish(source);
    assert_eq!(restored.flows(), plan.flows());
    assert_eq!(restored.flow_schemas(), plan.flow_schemas());
    assert_eq!(restored.flow_executables(), plan.flow_executables());
    assert_eq!(restored.callable_executables(), plan.callable_executables());
    assert_eq!(restored.defer_sites(), plan.defer_sites());
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(encode(&restored), bytes);
    assert_eq!(
        restored
            .resolve_flow_target_value("source_opening")
            .unwrap(),
        plan.flows()[2].id
    );
    assert_eq!(
        restored
            .resolve_flow_target_value("flow.flow.nested")
            .unwrap(),
        plan.flows()[4].id
    );
}
#[test]
fn invocation_sources_count_exact_storage_and_reject_one_less_for_every_table_and_resource_cap() {
    let plan = fixture();
    let bytes = encode(&plan);
    let prepared =
        PreparedRuntimeInvocationSources::preflight(&bytes, limits(&plan), &mut meter()).unwrap();
    let exact = RuntimeInvocationSourceLimits {
        flows: 5,
        schemas: 5,
        flow_executables: 5,
        callables: 3,
        defers: 1,
        elements: prepared.elements() as u32,
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        ..limits(&plan)
    };
    PreparedRuntimeInvocationSources::preflight(&bytes, exact, &mut meter()).unwrap();
    plan.prepare_invocation_sources_encoding(exact, &mut meter())
        .unwrap();
    for short in [
        RuntimeInvocationSourceLimits { flows: 4, ..exact },
        RuntimeInvocationSourceLimits {
            schemas: 4,
            ..exact
        },
        RuntimeInvocationSourceLimits {
            flow_executables: 4,
            ..exact
        },
        RuntimeInvocationSourceLimits {
            callables: 2,
            ..exact
        },
        RuntimeInvocationSourceLimits { defers: 0, ..exact },
        RuntimeInvocationSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeInvocationSourceLimits {
            encoded_bytes: bytes.len() - 1,
            ..exact
        },
        RuntimeInvocationSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = meter();
        assert!(PreparedRuntimeInvocationSources::preflight(&bytes, short, &mut bank).is_err());
        assert!(bank.status().is_err());
        assert!(
            plan.prepare_invocation_sources_encoding(short, &mut meter())
                .is_err()
        );
    }
}
#[test]
fn invocation_sources_reject_truncation_domain_version_flags_refs_and_trailing_data() {
    let plan = fixture();
    let bytes = encode(&plan);
    for end in 0..bytes.len() {
        assert!(
            PreparedRuntimeInvocationSources::preflight(&bytes[..end], limits(&plan), &mut meter())
                .is_err(),
            "prefix {end}"
        );
    }
    let mut bad = bytes.clone();
    bad.push(0);
    assert!(matches!(
        PreparedRuntimeInvocationSources::preflight(&bad, limits(&plan), &mut meter()),
        Err(Error::Trailing { .. })
    ));
    for (offset, value) in [(0, 0), (DOMAIN.len(), 2), (bytes.len() - 1, 255)] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(
            PreparedRuntimeInvocationSources::preflight(&bad, limits(&plan), &mut meter()).is_err()
        );
    }
    let identity = callable(230);
    let needle = identity.as_str().as_bytes();
    let start = bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .unwrap();
    let mut bad = bytes.clone();
    bad[start - 5] = 2;
    assert!(matches!(
        PreparedRuntimeInvocationSources::preflight(&bad, limits(&plan), &mut meter()),
        Err(Error::Tag {
            kind: "flag",
            tag: 2
        })
    ));
    let code = bytes
        .windows(needle.len())
        .rposition(|window| window == needle)
        .unwrap()
        + needle.len()
        + 32;
    let mut bad = bytes;
    bad[code] = 0;
    assert!(matches!(
        PreparedRuntimeInvocationSources::preflight(&bad, limits(&plan), &mut meter()),
        Err(Error::Tag {
            kind: "callable executable",
            tag: 0
        })
    ));
}
#[test]
fn invocation_sources_authenticate_all_function_handles_before_retaining_rows() {
    let plan = fixture();
    let bytes = encode(&plan);
    for kind in 0..2 {
        let (mut source, mut functions, _) = source(&plan);
        let (_, other_functions, _) = source_with_bank(&plan, meter());
        match kind {
            0 => functions.swap(0, 1),
            _ => functions = other_functions,
        }
        let prepared =
            PreparedRuntimeInvocationSources::preflight(&bytes, limits(&plan), &mut source.meter)
                .unwrap();
        assert!(matches!(
            prepared.materialize(&mut source, &functions),
            Err(Error::Handle { .. })
        ));
        assert!(source.builder.flows.is_empty());
        assert!(source.builder.defer_sites.is_empty());
        assert!(source.meter.status().is_err());
    }
}
#[test]
fn invocation_sources_reject_late_invalid_defer_without_publishing_earlier_valid_bindings() {
    let plan = fixture();
    let mut bytes = encode(&plan);
    let end = bytes.len();
    bytes[end - 4..].copy_from_slice(&1u32.to_le_bytes());
    let (mut source, functions, _) = source(&plan);
    let prepared =
        PreparedRuntimeInvocationSources::preflight(&bytes, limits(&plan), &mut source.meter)
            .unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &functions),
        Err(Error::Construction(
            RuntimePlanBuildError::InvalidDeferFunctionSite
        ))
    ));
    assert_eq!(source.builder.flows.len(), 5);
    assert_eq!(source.builder.callable_executables.len(), 3);
    assert!(source.builder.defer_sites.is_empty());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}
#[test]
fn invocation_sources_preserve_the_original_work_bank_and_output_prefix_capacity() {
    let plan = fixture();
    let bytes = encode(&plan);
    let prepared = plan
        .prepare_invocation_sources_encoding(limits(&plan), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(prepared.encoded_bytes());
    output.push(91);
    assert!(matches!(
        prepared.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [91]);
    let (trial, _, _) = source(&plan);
    let used = trial.meter.totals().0;
    let constrained = crate::task::semantic::TaskSemanticMeter::new(used + 1, u64::MAX);
    let (mut actual, _, _) = source_with_bank(&plan, constrained);
    assert!(matches!(
        PreparedRuntimeInvocationSources::preflight(&bytes, limits(&plan), &mut actual.meter),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert!(
        actual
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}
#[test]
fn invocation_declarations_never_make_missing_bodies_executable() {
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut source, functions, _) = source(&plan);
    PreparedRuntimeInvocationSources::preflight(&bytes, limits(&plan), &mut source.meter)
        .unwrap()
        .materialize(&mut source, &functions)
        .unwrap();
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}
