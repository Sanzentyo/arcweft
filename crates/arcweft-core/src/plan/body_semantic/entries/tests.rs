use super::super::entry_fixtures::controller_plan;
use super::*;
use crate::entry::*;
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
};
use crate::plan::*;
use crate::task::semantic::TaskSemanticEncodingError;
use crate::value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

fn nominal(tag: u8) -> RuntimeNominalTypeId {
    RuntimeNominalTypeId::from_checked_digest([tag; 32])
}

fn opaque(
    producer: &str,
    arguments: Box<[RuntimeSemanticTypeId]>,
) -> RuntimePlanTypeProjection<RuntimeSemanticTypeId> {
    RuntimePlanTypeProjection::Opaque {
        producer: RuntimeOpaqueTypeProducerId::try_new(producer).unwrap(),
        admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
        value_class: RuntimeOpaqueValueClass::Plain,
        persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
        arguments,
    }
}

fn parameter(
    local: RuntimeLocalSeedId,
    ty: RuntimeSemanticTypeId,
    identity: u8,
    position: u32,
    passing: RuntimeFunctionParameterPassing,
) -> RuntimeFunctionInputBindingSeed {
    RuntimeFunctionInputBindingSeed {
        transfer: RuntimeFunctionInputTransfer::Formal,
        origin: RuntimeFunctionInputOrigin::Parameter(
            RuntimeFunctionParameterIdentity::from_accepted_identity([identity; 32]),
        ),
        source: RuntimeFunctionInputSource::Parameter { position, passing },
        input_local: local.clone(),
        pattern: RuntimePatternSeed::new(
            ty,
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        ),
        ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
        unrestricted_bindings: Box::new([]),
    }
}

fn stateful_plan(schema: RuntimeTypeSchema) -> RuntimePlan {
    let graph = RuntimeNominalSchemaGraph::try_new(
        (1..=2)
            .map(|tag| {
                RuntimeNominalSchemaDefinition::new(
                    RuntimeNominalDeclarationId::from_bytes([tag; 32]),
                    RuntimeNominalSchemaIdentity::new(nominal(tag), semantic(tag)),
                    vec![],
                    RuntimeNominalSchemaBody::Record {
                        shape: RuntimeNominalRecordShape::Unit,
                        fields: Box::new([]),
                    },
                )
            })
            .collect::<Vec<_>>(),
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let mut builder = RuntimePlanBuilder::new();
    let locals = admit_stateful_types(&mut builder, &graph);
    let (initializer_role, reducer_role) = admit_stateful_callables(&mut builder, &locals);
    let flow_role = admit_stateful_flow(&mut builder, locals[2].clone());
    let binding = EntryBindingIdentity::from_bytes([76; 32]);
    builder
        .push_entry(RuntimeEntrySpec {
            id: EntryRuntimeId::canonical("stateful").unwrap(),
            kind: RuntimeEntryKind::Game,
            binding,
            target: RuntimeEntryTarget::Flow(flow_role.flow.clone()),
            roles: RuntimeEntryRoles::Stateful(Box::new(RuntimeStatefulEntryRoles {
                binding,
                state: RuntimeNominalRole {
                    identity: nominal(1),
                    semantic_identity: semantic(1),
                    layout: graph.try_layout_hash(semantic(1)).unwrap(),
                },
                event: RuntimeNominalRole {
                    identity: nominal(2),
                    semantic_identity: semantic(2),
                    layout: graph.try_layout_hash(semantic(2)).unwrap(),
                },
                initializer: initializer_role,
                reducer: reducer_role,
                initial_flow: flow_role,
                command_policy: RuntimeCommandPolicy::new(
                    [RuntimeCommandContract {
                        constructor: RuntimeCommandConstructorId::try_new("set").unwrap(),
                        target: RuntimeCommandTargetId::try_new("output").unwrap(),
                        payload_layout: schema.try_layout_hash().unwrap(),
                        payload_schema: schema,
                    }],
                    RootExecutionLimits::engine_default(),
                ),
            })),
        })
        .unwrap();
    builder.finish().unwrap()
}

fn admit_stateful_types(
    builder: &mut RuntimePlanBuilder,
    graph: &RuntimeNominalSchemaGraph,
) -> Box<[RuntimeLocalSeedId]> {
    let mut types = (1..=2)
        .map(|tag| {
            RuntimePlanTypeSeed::new(
                semantic(tag),
                RuntimePlanTypeProjection::Nominal {
                    nominal: nominal(tag),
                    layout: graph.try_layout_hash(semantic(tag)).unwrap(),
                    arguments: Box::new([]),
                },
            )
        })
        .collect::<Vec<_>>();
    types.extend([
        RuntimePlanTypeSeed::new(
            semantic(3),
            RuntimePlanTypeProjection::Reference(semantic(1)),
        ),
        RuntimePlanTypeSeed::new(
            semantic(4),
            opaque("std.reduction", Box::new([semantic(1)])),
        ),
        RuntimePlanTypeSeed::new(semantic(5), opaque("std.reducer_error", Box::new([]))),
        RuntimePlanTypeSeed::new(
            semantic(6),
            RuntimePlanTypeProjection::Tuple(Box::new([semantic(4)])),
        ),
        RuntimePlanTypeSeed::new(
            semantic(7),
            RuntimePlanTypeProjection::Tuple(Box::new([semantic(5)])),
        ),
        RuntimePlanTypeSeed::new(
            semantic(8),
            RuntimePlanTypeProjection::Result {
                value: semantic(4),
                error: semantic(5),
                value_payload: semantic(6),
                error_payload: semantic(7),
            },
        ),
        RuntimePlanTypeSeed::new(semantic(9), RuntimePlanTypeProjection::Unit),
    ]);
    let locals = builder
        .admit_semantic_batch(
            types,
            [(3, 31), (2, 32), (1, 33)].map(|(ty, id)| {
                RuntimeLocalDeclarationSeed::new(
                    crate::plan::RuntimeLocalDeclarationSource::Parameter(
                        RuntimeFunctionParameterIdentity::from_accepted_identity([id; 32]),
                    ),
                    semantic(ty),
                )
            }),
            (1..=2).map(|tag| {
                RuntimeNominalRecordDomainSeed::new(
                    semantic(tag),
                    RuntimeNominalRecordShape::Unit,
                    [],
                )
            }),
            [],
            graph,
        )
        .unwrap();
    locals.local_ids().to_vec().into_boxed_slice()
}

fn admit_stateful_callables(
    builder: &mut RuntimePlanBuilder,
    locals: &[RuntimeLocalSeedId],
) -> (RuntimeCallableRole, RuntimeCallableRole) {
    let state = || {
        RuntimeExprSeed::new(
            semantic(1),
            RuntimeExprSeedKind::NominalRecord(Box::new([])),
        )
    };
    let initializer = builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [],
            state(),
        )
        .unwrap();
    let reducer = builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([62; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [
                parameter(
                    locals[0].clone(),
                    semantic(3),
                    31,
                    0,
                    RuntimeFunctionParameterPassing::Shared,
                ),
                parameter(
                    locals[1].clone(),
                    semantic(2),
                    32,
                    1,
                    RuntimeFunctionParameterPassing::Value,
                ),
            ],
            RuntimeExprSeed::new(
                semantic(8),
                RuntimeExprSeedKind::Variant {
                    ordinal: 0,
                    payload: Some(Box::new(RuntimeExprSeed::new(
                        semantic(6),
                        RuntimeExprSeedKind::Tuple(Box::new([RuntimeExprSeed::new(
                            semantic(4),
                            RuntimeExprSeedKind::ReductionUnchanged {
                                state: Box::new(state()),
                            },
                        )])),
                    ))),
                },
            ),
        )
        .unwrap();
    let initializer_role = RuntimeCallableRole {
        callable: RuntimeCallableId::from_checked_digest([71; 32]),
        contract: CallableContractHash::from_bytes([72; 32]),
    };
    let reducer_role = RuntimeCallableRole {
        callable: RuntimeCallableId::from_checked_digest([73; 32]),
        contract: CallableContractHash::from_bytes([74; 32]),
    };
    for (role, function) in [(&initializer_role, initializer), (&reducer_role, reducer)] {
        builder
            .push_callable_executable_seed(RuntimeCallableExecutableSeed {
                callable: role.callable.clone(),
                contract: role.contract,
                code: RuntimeCallableExecutableSeedCode::FunctionSite(function),
            })
            .unwrap();
    }
    (initializer_role, reducer_role)
}

fn admit_stateful_flow(
    builder: &mut RuntimePlanBuilder,
    local: RuntimeLocalSeedId,
) -> RuntimeFlowRole {
    let flow = FlowRuntimeId::canonical("opening").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: vec![RuntimeFlowExecutableParameter {
                identity: RuntimeFunctionParameterIdentity::from_accepted_identity([33; 32]),
                coordinate: FlowParameterCoordinate::from_position(0),
                name: "state".into(),
                mode: RuntimeFlowParameterMode::Owned,
                passing: RuntimeFunctionParameterPassing::Value,
                semantic_identity: semantic(1),
            }],
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([63; 32]),
                None,
                Box::new([parameter(
                    local,
                    semantic(1),
                    33,
                    0,
                    RuntimeFunctionParameterPassing::Value,
                )]),
                semantic(9),
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([]),
            },
        ))
        .unwrap();
    let flow_role = RuntimeFlowRole {
        flow: flow.clone(),
        contract: FlowContractHash::from_bytes([75; 32]),
    };
    builder
        .push_flow_executable(RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: flow_role.contract,
            controller: None,
        })
        .unwrap();
    flow_role
}

fn digest(
    plan: &RuntimePlan,
    work: u64,
    bytes: u64,
) -> (Result<blake3::Hash, RuntimeBodySemanticError>, (u64, u64)) {
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let result = RuntimeBodySemanticContext::new(plan).entry_row_digest(&mut meter, 0);
    (result, meter.totals())
}

#[test]
fn stateful_entry_commits_actual_command_schema_layout_and_rejects_a_false_leaf() {
    let first = stateful_plan(RuntimeTypeSchema::Bool);
    let changed = stateful_plan(RuntimeTypeSchema::String);
    let expected = digest(&first, 100_000, 1_000_000).0.unwrap();
    assert_ne!(expected, digest(&changed, 100_000, 1_000_000).0.unwrap());
    let mut forged = first.clone();
    let RuntimeEntryRoles::Stateful(roles) = &mut forged.inventory.entries[0].roles else {
        panic!("stateful")
    };
    roles.command_policy.admitted[0].payload_schema = RuntimeTypeSchema::String;
    assert!(matches!(
        forged.verify(),
        Err(RuntimePlanError::CommandLayoutMismatch { .. })
    ));
}

#[test]
fn stateful_entry_runtime_limits_change_the_row() {
    let first = stateful_plan(RuntimeTypeSchema::Bool);
    let expected = digest(&first, 100_000, 1_000_000).0.unwrap();
    let mutations: [fn(&mut RootExecutionLimits); 10] = [
        |limits| limits.schema.max_depth += 1,
        |limits| limits.schema.max_nodes += 1,
        |limits| limits.schema.max_sequence_items += 1,
        |limits| limits.schema.max_string_bytes += 1,
        |limits| limits.schema.max_encoded_bytes += 1,
        |limits| limits.schema.max_validation_work += 1,
        |limits| limits.max_commands_per_transition += 1,
        |limits| limits.max_command_bytes_per_transition += 1,
        |limits| limits.max_pending_events += 1,
        |limits| limits.max_pending_commands += 1,
    ];
    for mutate in mutations {
        let mut changed = first.clone();
        let RuntimeEntryRoles::Stateful(roles) = &mut changed.inventory.entries[0].roles else {
            panic!("stateful")
        };
        mutate(&mut roles.command_policy.root_limits);
        changed.verify().unwrap();
        assert_ne!(expected, digest(&changed, 100_000, 1_000_000).0.unwrap());
    }
}

#[test]
fn agent_entry_budget_binding_and_abi_are_semantic_but_parameter_name_and_arena_padding_are_not() {
    let first = controller_plan(false, true, RuntimeFunctionParameterPassing::Value, "input").0;
    let padded = controller_plan(
        true,
        true,
        RuntimeFunctionParameterPassing::Value,
        "renamed",
    )
    .0;
    let expected = digest(&first, 100_000, 1_000_000).0.unwrap();
    assert_eq!(expected, digest(&padded, 100_000, 1_000_000).0.unwrap());
    let changed_abi = controller_plan(
        false,
        true,
        RuntimeFunctionParameterPassing::Shared,
        "input",
    )
    .0;
    assert_ne!(
        expected,
        digest(&changed_abi, 100_000, 1_000_000).0.unwrap()
    );
    let changed_body = controller_plan(
        false,
        false,
        RuntimeFunctionParameterPassing::Value,
        "input",
    )
    .0;
    assert_eq!(
        expected,
        digest(&changed_body, 100_000, 1_000_000).0.unwrap()
    );
    for change in 0..3 {
        let mut changed = first.clone();
        let RuntimeEntryRoles::Agent(roles) = &mut changed.inventory.entries[0].roles else {
            panic!("agent")
        };
        if change == 0 {
            roles.budget.max_vm_steps += 1;
        } else if change == 1 {
            roles.policy = AgentPolicyHash::from_bytes([90; 32]);
        } else {
            roles.binding = EntryBindingIdentity::from_bytes([91; 32]);
            changed.inventory.entries[0].binding = roles.binding;
        }
        changed.verify().unwrap();
        assert_ne!(expected, digest(&changed, 100_000, 1_000_000).0.unwrap());
    }
}

#[test]
fn entry_row_has_exact_shared_limits_and_missing_row_preserves_the_first_error() {
    let plan = controller_plan(false, true, RuntimeFunctionParameterPassing::Value, "input").0;
    let (expected, (work, bytes)) = digest(&plan, 100_000, 1_000_000);
    assert_eq!(expected.unwrap(), digest(&plan, work, bytes).0.unwrap());
    for (work, bytes, error) in [
        (work - 1, bytes, TaskSemanticEncodingError::SemanticWork),
        (work, bytes - 1, TaskSemanticEncodingError::TranscriptBytes),
    ] {
        assert!(
            matches!(digest(&plan, work, bytes).0, Err(RuntimeBodySemanticError::Encoding(actual)) if actual == error)
        );
    }
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        context.entry_row_digest(&mut meter, 1),
        Err(RuntimeBodySemanticError::MissingRow {
            table: "entries",
            ordinal: 1
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    let mut meter = TaskSemanticMeter::new(0, 1_000_000);
    meter.charge_work(1).unwrap_err();
    assert!(matches!(
        context.entry_row_digest(&mut meter, 99),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(meter.totals(), (0, 0));
}

fn stateless_plan(kind: RuntimeEntryKind, routed: bool) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let string = crate::pattern::RuntimeCheckedType::String.semantic_identity_digest();
    let identity = RuntimeFunctionParameterIdentity::from_accepted_identity([41; 32]);
    let batch = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(string, RuntimePlanTypeProjection::String),
            ],
            routed.then(|| {
                RuntimeLocalDeclarationSeed::new(
                    crate::plan::RuntimeLocalDeclarationSource::Parameter(identity),
                    string,
                )
            }),
        )
        .unwrap();
    let flow = FlowRuntimeId::canonical("serve").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: if routed {
                vec![RuntimeFlowExecutableParameter {
                    identity,
                    coordinate: FlowParameterCoordinate::from_position(0),
                    name: "capture".into(),
                    mode: RuntimeFlowParameterMode::Owned,
                    passing: RuntimeFunctionParameterPassing::Value,
                    semantic_identity: string,
                }]
            } else {
                vec![]
            },
        })
        .unwrap();
    let inputs = if routed {
        vec![parameter(
            batch.local_ids()[0].clone(),
            string,
            41,
            0,
            RuntimeFunctionParameterPassing::Value,
        )]
    } else {
        vec![]
    };
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([42; 32]),
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
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([43; 32]),
            controller: None,
        })
        .unwrap();
    let target = if routed {
        RuntimeEntryTarget::Routes(
            ["alpha", "omega"]
                .map(|literal| route(&flow, literal))
                .into(),
        )
    } else {
        RuntimeEntryTarget::Flow(flow)
    };
    builder
        .push_entry(RuntimeEntrySpec {
            id: EntryRuntimeId::canonical("serve").unwrap(),
            kind,
            binding: EntryBindingIdentity::from_bytes([44; 32]),
            target,
            roles: RuntimeEntryRoles::None,
        })
        .unwrap();
    builder.finish().unwrap()
}

fn route(flow: &FlowRuntimeId, literal: &str) -> RuntimeRouteSpec {
    RuntimeRouteSpec {
        method: RuntimeHttpMethod::Get,
        path: RuntimeRoutePath::try_new(vec![
            RuntimeRoutePathSegment::Literal(literal.into()),
            RuntimeRoutePathSegment::Capture(RouteCaptureCoordinate::from_position(0)),
        ])
        .unwrap(),
        target: flow.clone(),
        bindings: vec![RuntimeRouteBinding {
            parameter: FlowParameterCoordinate::from_position(0),
            source: RuntimeRouteBindingSource::PathCapture(RouteCaptureCoordinate::from_position(
                0,
            )),
        }],
    }
}

#[test]
fn route_entry_commits_method_path_and_binding_and_rejects_invalid_dispatch() {
    let first = stateless_plan(RuntimeEntryKind::Server, true);
    let expected = digest(&first, 100_000, 1_000_000).0.unwrap();
    for change in 0..2 {
        let mut changed = first.clone();
        let RuntimeEntryTarget::Routes(routes) = &mut changed.inventory.entries[0].target else {
            panic!("routes")
        };
        if change == 0 {
            routes[1].method = RuntimeHttpMethod::Post;
        } else {
            routes[0].path = route(&routes[0].target, "beta").path;
        }
        changed.verify().unwrap();
        assert_ne!(expected, digest(&changed, 100_000, 1_000_000).0.unwrap());
    }
    let mut unordered = first.clone();
    let RuntimeEntryTarget::Routes(routes) = &mut unordered.inventory.entries[0].target else {
        panic!("routes")
    };
    routes.reverse();
    assert!(matches!(
        unordered.verify(),
        Err(RuntimePlanError::InvalidRouteOrder { .. })
    ));
    let mut bad_capture = first.clone();
    let RuntimeEntryTarget::Routes(routes) = &mut bad_capture.inventory.entries[0].target else {
        panic!("routes")
    };
    routes[0].bindings[0].source =
        RuntimeRouteBindingSource::PathCapture(RouteCaptureCoordinate::from_position(1));
    assert!(matches!(
        bad_capture.verify(),
        Err(RuntimePlanError::InvalidRouteBindings { .. })
    ));
}

#[test]
fn stateless_entry_commits_kind_custom_payload_and_binding() {
    let first = stateless_plan(RuntimeEntryKind::Cli, false);
    let expected = digest(&first, 100_000, 1_000_000).0.unwrap();
    let custom_a = stateless_plan(RuntimeEntryKind::Custom("adapter-a".into()), false);
    let custom_b = stateless_plan(RuntimeEntryKind::Custom("adapter-b".into()), false);
    let a = digest(&custom_a, 100_000, 1_000_000).0.unwrap();
    let b = digest(&custom_b, 100_000, 1_000_000).0.unwrap();
    assert_ne!(expected, a);
    assert_ne!(a, b);
    let mut rebound = first.clone();
    rebound.inventory.entries[0].binding = EntryBindingIdentity::from_bytes([45; 32]);
    rebound.verify().unwrap();
    assert_ne!(expected, digest(&rebound, 100_000, 1_000_000).0.unwrap());
}
