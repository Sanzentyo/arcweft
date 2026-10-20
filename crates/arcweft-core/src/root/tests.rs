use super::*;
use crate::{
    awbc::schema::{
        AwbcProgram, AwbcRecordField, AwbcRuntimeType, AwbcRuntimeTypeShape as AwbcType,
        AwbcStringId, AwbcTypeId, AwbcVariantCase, AwbcVariantIdentity,
    },
    entry::{
        CallableContractHash, FlowContractHash, RuntimeCallableId, RuntimeCommandConstructorId,
        RuntimeCommandPolicy, RuntimeCommandTargetId, RuntimeFlowRole,
        RuntimeNominalRecordShape as Shape, RuntimeNominalRole, RuntimeNominalSchemaBody,
        RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField,
        RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
        RuntimeTypeSchema as Schema,
    },
    pattern::{
        RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
        RuntimeSemanticTypeId,
    },
    plan::{
        RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlanBuilder,
        RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed, RuntimeVariantCaseSeed,
        RuntimeVariantDomainSeed,
    },
    program_types::RuntimeProgramTypes,
    value::{
        RuntimeHandleKind, RuntimeNominalRecordValue, RuntimeOpaquePersistence,
        RuntimeOpaqueValueClass, RuntimeRecordFieldId,
    },
};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn nominal(tag: u8) -> RuntimeNominalTypeId {
    RuntimeNominalTypeId::try_new(match tag {
        1 => "root.State",
        2 => "root.Event",
        3 => "root.Payload",
        _ => unreachable!(),
    })
    .unwrap()
}
fn identity(tag: u8) -> RuntimeNominalSchemaIdentity {
    RuntimeNominalSchemaIdentity::new(nominal(tag), semantic(tag))
}
fn field() -> RuntimeRecordFieldId {
    RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap()
}

struct Fixture {
    plan: RuntimePlan,
    awbc: AwbcProgram,
    contract: RootStartupContract,
    payload_layout: TypeLayoutHash,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum AffineField {
    None,
    State,
    Event,
}

fn affine_owner() -> RuntimeOpaqueTypeOwner {
    RuntimeOpaqueTypeOwner::exact_with(
        RuntimeOpaqueTypeProducerId::try_new("test.affine").unwrap(),
        semantic(6),
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
        RuntimeOpaquePersistence::SnapshotOnly,
    )
}

fn affine_value() -> RuntimeValue {
    affine_owner().try_wrap(RuntimeValue::Unit).unwrap()
}

impl Fixture {
    fn new() -> Self {
        Self::with_affine_field(AffineField::None)
    }

    fn with_affine_field(affine: AffineField) -> Self {
        Self::with_type_qualifiers(affine, false)
    }

    fn with_type_qualifiers(affine: AffineField, qualifiers: bool) -> Self {
        let opaque_schema = || Schema::ExactOpaque {
            owner: affine_owner(),
            arguments: Box::new([]),
        };
        let graph = RuntimeNominalSchemaGraph::try_new(
            vec![
                RuntimeNominalSchemaDefinition::new(
                    crate::entry::RuntimeNominalDeclarationId::from_bytes(
                        *(identity(1)).semantic_identity().as_bytes(),
                    ),
                    identity(1),
                    vec![],
                    RuntimeNominalSchemaBody::Record {
                        shape: Shape::Record,
                        fields: vec![RuntimeNominalSchemaField::new(
                            field(),
                            Some("payload".into()),
                            Schema::NominalRef(identity(3)),
                        )]
                        .into(),
                    },
                ),
                RuntimeNominalSchemaDefinition::new(
                    crate::entry::RuntimeNominalDeclarationId::from_bytes(
                        *(identity(2)).semantic_identity().as_bytes(),
                    ),
                    identity(2),
                    vec![],
                    RuntimeNominalSchemaBody::Variant {
                        cases: vec![
                            RuntimeNominalSchemaCase::new(0, "Tick".into(), None),
                            RuntimeNominalSchemaCase::new(
                                1,
                                "Set".into(),
                                Some(Schema::Tuple(
                                    vec![if affine == AffineField::Event {
                                        opaque_schema()
                                    } else {
                                        Schema::Bool
                                    }]
                                    .into(),
                                )),
                            ),
                        ]
                        .into(),
                    },
                ),
                RuntimeNominalSchemaDefinition::new(
                    crate::entry::RuntimeNominalDeclarationId::from_bytes(
                        *(identity(3)).semantic_identity().as_bytes(),
                    ),
                    identity(3),
                    vec![],
                    RuntimeNominalSchemaBody::Record {
                        shape: Shape::Newtype,
                        fields: vec![RuntimeNominalSchemaField::new(
                            field(),
                            None,
                            if affine == AffineField::State {
                                opaque_schema()
                            } else {
                                Schema::Bool
                            },
                        )]
                        .into(),
                    },
                ),
            ],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
        let layout = |tag| graph.try_layout_hash(semantic(tag)).unwrap();
        let mut types = (1..=3)
            .map(|tag| {
                RuntimePlanTypeSeed::new(
                    semantic(tag),
                    Type::Nominal {
                        nominal: nominal(tag),
                        layout: layout(tag),
                        arguments: Box::new([]),
                    },
                )
            })
            .collect::<Vec<_>>();
        types.extend([
            RuntimePlanTypeSeed::new(semantic(4), Type::Bool),
            RuntimePlanTypeSeed::new(
                semantic(5),
                Type::Tuple(
                    vec![semantic(if affine == AffineField::Event { 6 } else { 4 })].into(),
                ),
            ),
        ]);
        if affine != AffineField::None {
            types.push(RuntimePlanTypeSeed::new(
                semantic(6),
                Type::Opaque {
                    producer: affine_owner().producer().clone(),
                    admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                    value_class: RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
                    persistence: RuntimeOpaquePersistence::SnapshotOnly,
                    arguments: Box::new([]),
                },
            ));
        }
        if qualifiers {
            types.extend([
                RuntimePlanTypeSeed::new(semantic(7), Type::Reference(semantic(1))),
                RuntimePlanTypeSeed::new(semantic(8), Type::Shared(semantic(7))),
                RuntimePlanTypeSeed::new(semantic(9), Type::Reference(semantic(8))),
            ]);
        }
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_semantic_batch(
                types,
                [],
                [
                    RuntimeNominalRecordDomainSeed::new(
                        semantic(1),
                        Shape::Record,
                        [RuntimeNominalRecordDomainFieldSeed::new(
                            field(),
                            Some("payload".into()),
                            semantic(3),
                        )],
                    ),
                    RuntimeNominalRecordDomainSeed::new(
                        semantic(3),
                        Shape::Newtype,
                        [RuntimeNominalRecordDomainFieldSeed::new(
                            field(),
                            None,
                            semantic(if affine == AffineField::State { 6 } else { 4 }),
                        )],
                    ),
                ],
                [RuntimeVariantDomainSeed::new(
                    semantic(2),
                    nominal(2),
                    layout(2),
                    [
                        RuntimeVariantCaseSeed::new("Tick", None),
                        RuntimeVariantCaseSeed::new("Set", Some(semantic(5))),
                    ],
                )],
                &graph,
            )
            .unwrap();
        let plan = builder.finish().unwrap();
        let mut runtime_types = vec![
            AwbcRuntimeType::new(
                semantic(1),
                AwbcType::NominalRecord {
                    public_id: AwbcStringId(0),
                    layout: *layout(1).as_bytes(),
                    arguments: vec![],
                    shape: Shape::Record,
                    fields: vec![AwbcRecordField {
                        field: field(),
                        name: Some(AwbcStringId(3)),
                        ty: AwbcTypeId(2),
                    }],
                },
            ),
            AwbcRuntimeType::new(
                semantic(2),
                AwbcType::Variant {
                    owner: AwbcVariantIdentity::Nominal {
                        public_id: AwbcStringId(1),
                        layout: *layout(2).as_bytes(),
                    },
                    arguments: vec![],
                    cases: vec![
                        AwbcVariantCase {
                            name: AwbcStringId(4),
                            payload: None,
                        },
                        AwbcVariantCase {
                            name: AwbcStringId(5),
                            payload: Some(AwbcTypeId(4)),
                        },
                    ],
                },
            ),
            AwbcRuntimeType::new(
                semantic(3),
                AwbcType::NominalRecord {
                    public_id: AwbcStringId(2),
                    layout: *layout(3).as_bytes(),
                    arguments: vec![],
                    shape: Shape::Newtype,
                    fields: vec![AwbcRecordField {
                        field: field(),
                        name: None,
                        ty: AwbcTypeId(if affine == AffineField::State { 5 } else { 3 }),
                    }],
                },
            ),
            AwbcRuntimeType::new(semantic(4), AwbcType::Bool),
            AwbcRuntimeType::new(
                semantic(5),
                AwbcType::Tuple(vec![AwbcTypeId(if affine == AffineField::Event {
                    5
                } else {
                    3
                })]),
            ),
        ];
        if affine != AffineField::None {
            runtime_types.push(AwbcRuntimeType::new(
                semantic(6),
                AwbcType::Opaque {
                    producer: AwbcStringId(6),
                    admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                    value_class: RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
                    persistence: RuntimeOpaquePersistence::SnapshotOnly,
                    arguments: vec![],
                },
            ));
        }
        if qualifiers {
            let reference = AwbcTypeId(u32::try_from(runtime_types.len()).unwrap());
            let shared = AwbcTypeId(reference.0 + 1);
            let cycle = AwbcTypeId(reference.0 + 3);
            runtime_types.extend([
                AwbcRuntimeType::new(semantic(7), AwbcType::Reference(AwbcTypeId(0))),
                AwbcRuntimeType::new(semantic(8), AwbcType::Shared(reference)),
                AwbcRuntimeType::new(semantic(9), AwbcType::Reference(shared)),
                AwbcRuntimeType::new(semantic(10), AwbcType::Reference(cycle)),
            ]);
        }
        let awbc = AwbcProgram {
            strings: vec![
                "root.State".into(),
                "root.Event".into(),
                "root.Payload".into(),
                "payload".into(),
                "Tick".into(),
                "Set".into(),
                "test.affine".into(),
            ],
            runtime_types,
            ..AwbcProgram::default()
        };
        let flow = FlowRuntimeId::from_runtime_target_value("flow.root").unwrap();
        let contract = RootStartupContract {
            entry: EntryRuntimeId::from_source_entity_body("entry.root").unwrap(),
            roles: RuntimeStatefulEntryRoles {
                binding: EntryBindingIdentity::from_bytes([9; 32]),
                state: RuntimeNominalRole {
                    identity: nominal(1),
                    semantic_identity: semantic(1),
                    layout: layout(1),
                },
                event: RuntimeNominalRole {
                    identity: nominal(2),
                    semantic_identity: semantic(2),
                    layout: layout(2),
                },
                initializer: RuntimeCallableRole {
                    callable: RuntimeCallableId::try_new("root.initial").unwrap(),
                    contract: CallableContractHash::from_bytes([10; 32]),
                },
                reducer: RuntimeCallableRole {
                    callable: RuntimeCallableId::try_new("root.reduce").unwrap(),
                    contract: CallableContractHash::from_bytes([11; 32]),
                },
                initial_flow: RuntimeFlowRole {
                    flow: flow.clone(),
                    contract: FlowContractHash::from_bytes([12; 32]),
                },
                command_policy: RuntimeCommandPolicy::deny_all(
                    RootExecutionLimits::engine_default(),
                ),
            },
            initial_flow: flow,
            initial_state_parameter: crate::entry::FlowParameterCoordinate::from_position(0),
        };
        Self {
            plan,
            awbc,
            contract,
            payload_layout: layout(3),
        }
    }

    fn programs(&self) -> [RuntimeProgramTypes<'_>; 2] {
        [
            RuntimeProgramTypes::Plan(&self.plan),
            RuntimeProgramTypes::Awbc(&self.awbc),
        ]
    }
    fn state(&self, field_value: RuntimeValue) -> RuntimeValue {
        RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            nominal(1),
            semantic(1),
            self.contract.roles.state.layout,
            vec![RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                nominal(3),
                semantic(3),
                self.payload_layout,
                vec![field_value],
            ))],
        ))
    }
    fn event(&self, field_value: RuntimeValue) -> RootEventInput {
        RootEventInput::new(RuntimePayload(
            RuntimeProgramTypes::Plan(&self.plan)
                .try_variant_value(
                    semantic(2),
                    1,
                    Some(RuntimeValue::Tuple(vec![field_value])),
                    RuntimeSchemaLimits::engine_default(),
                )
                .unwrap(),
        ))
    }
    fn start(&self, program: RuntimeProgramTypes<'_>) -> RootRuntime {
        RootRuntime::prepare(self.contract.clone(), program)
            .unwrap()
            .complete_initialization(self.state(RuntimeValue::Bool(true)), program)
            .unwrap()
            .root
    }
}

fn reduction(state: RuntimeValue) -> RuntimeValue {
    let owner = RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("std.reduction").unwrap(),
        semantic(20),
    );
    RuntimeValue::result_ok(RuntimeValue::Reduction(
        RuntimeReductionValue::try_unchanged(owner, state).unwrap(),
    ))
}

#[test]
fn qualified_values_preserve_referents_and_the_canonical_value_allowance() {
    let fixture = Fixture::with_type_qualifiers(AffineField::None, true);
    let value = fixture.state(RuntimeValue::Bool(true));
    let mut limits = RuntimeSchemaLimits::engine_default();
    let encoded = crate::entry::schema::canonical_runtime_value_bytes(
        &value,
        usize::try_from(limits.max_encoded_bytes).unwrap(),
    )
    .unwrap();
    limits.max_encoded_bytes = u64::try_from(encoded.len()).unwrap();
    for program in fixture.programs() {
        let digest = program.accepts_value(semantic(1), &value, limits).unwrap();
        for qualifier in 7..=9 {
            assert_eq!(
                program
                    .accepts_value(semantic(qualifier), &value, limits)
                    .unwrap(),
                digest
            );
            program
                .validate_live_value(semantic(qualifier), &value, limits)
                .unwrap();
            assert_eq!(
                program
                    .accepts_value_view(
                        semantic(qualifier),
                        value.view(),
                        limits,
                        crate::entry::schema::value_encoding::ValueAdmissionOrigin::InertSnapshot,
                    )
                    .unwrap(),
                digest
            );
            assert!(
                program
                    .validate_live_value(semantic(qualifier), &RuntimeValue::Bool(true), limits)
                    .is_err()
            );
            assert!(
                program
                    .validate_live_value(
                        semantic(qualifier),
                        &fixture.state(RuntimeValue::String("wrong".into())),
                        limits
                    )
                    .is_err()
            );
        }
        let too_small = RuntimeSchemaLimits {
            max_encoded_bytes: limits.max_encoded_bytes - 1,
            ..limits
        };
        assert!(matches!(
            value_schema_error(
                program
                    .accepts_value(semantic(9), &value, too_small)
                    .unwrap_err()
            ),
            crate::entry::RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes"
            }
        ));
    }
}

fn value_schema_error(
    error: crate::program_types::RuntimeProgramTypeError,
) -> crate::entry::RuntimeSchemaError {
    use crate::program_types::RuntimeProgramTypeError as Error;
    match error {
        Error::Resource(error) => *error,
        Error::PlanValue(error) => match *error {
            crate::plan::RuntimePlanValueAdmissionError::Value { source, .. } => source,
            other => panic!("expected value admission failure: {other:?}"),
        },
        Error::AwbcValue(error) => match *error {
            crate::awbc::value_admission::AwbcValueAdmissionError::Value { source, .. } => source,
            other => panic!("expected value admission failure: {other:?}"),
        },
        other => panic!("expected schema failure: {other:?}"),
    }
}

#[test]
fn qualifier_cycles_and_affine_referents_preserve_validation_and_ownership_limits() {
    let mut invalid = RuntimePlanBuilder::new();
    assert!(matches!(
        invalid.admit_type_batch(
            [RuntimePlanTypeSeed::new(
                semantic(10),
                Type::Reference(semantic(10))
            )],
            [],
        ),
        Err(crate::plan::RuntimePlanBuildError::TypeGraph(
            crate::plan::RuntimePlanTypeTableError::Cycle { .. }
        ))
    ));
    let fixture = Fixture::with_type_qualifiers(AffineField::State, true);
    let value = fixture.state(affine_value());
    let limits = RuntimeSchemaLimits::engine_default();
    assert!(!value.ownership().permits_copy());
    for program in fixture.programs() {
        program
            .validate_live_value(semantic(9), &value, limits)
            .unwrap();
        assert!(program.accepts_value(semantic(9), &value, limits).is_err());
        let bounded = RuntimeSchemaLimits {
            max_validation_work: 1,
            ..limits
        };
        assert!(matches!(
            value_schema_error(
                program
                    .validate_live_value(semantic(9), &value, bounded)
                    .unwrap_err()
            ),
            crate::entry::RuntimeSchemaError::ValidationWork { limit: 1, .. }
        ));
    }
    let bounded = RuntimeSchemaLimits {
        max_depth: 4,
        ..limits
    };
    assert!(matches!(
        value_schema_error(
            RuntimeProgramTypes::Awbc(&fixture.awbc)
                .validate_live_value(semantic(10), &value, bounded)
                .unwrap_err()
        ),
        crate::entry::RuntimeSchemaError::ValidationDepth { limit: 4, .. }
    ));
}

#[test]
fn initializer_and_snapshot_validate_nested_values_through_each_program() {
    let fixture = Fixture::new();
    for program in fixture.programs() {
        let root = fixture.start(program);
        let snapshot = root.snapshot_state().unwrap();
        assert_eq!(
            RootRuntime::from_snapshot(
                fixture.contract.clone(),
                root.snapshot_state().unwrap(),
                program
            )
            .unwrap()
            .snapshot_state()
            .unwrap(),
            snapshot
        );
        let invalid = fixture.state(RuntimeValue::String("not Bool".into()));
        assert!(matches!(
            RootRuntime::prepare(fixture.contract.clone(), program)
                .unwrap()
                .complete_initialization(invalid.clone(), program)
                .map_err(|rejected| rejected.error),
            Err(RootRuntimeError::InvalidInitialValue(_))
        ));
        let mut snapshot = snapshot;
        snapshot.phase = RootPhase::Initialized {
            value: RuntimePayload(invalid),
            reducer: RootReducerPhase::Ready,
        };
        assert!(matches!(
            RootRuntime::from_snapshot(fixture.contract.clone(), snapshot, program),
            Err(RootRuntimeError::InvalidSnapshotValue(_))
        ));
        let mut wrong_role = fixture.contract.clone();
        wrong_role.roles.event.layout = TypeLayoutHash::from_bytes([0; 32]);
        assert!(matches!(
            RootRuntime::prepare(wrong_role, program),
            Err(RootRuntimeError::InvalidRole { role: "event", .. })
        ));
    }
}

#[test]
fn root_state_copy_obligation_rejects_nested_affine_value_at_both_ingresses() {
    let fixture = Fixture::with_affine_field(AffineField::State);
    for program in fixture.programs() {
        let value = fixture.state(affine_value());
        assert!(matches!(
            RootRuntime::prepare(fixture.contract.clone(), program)
                .unwrap()
                .complete_initialization(value, program)
                .map_err(|rejected| rejected.error),
            Err(RootRuntimeError::AffineRootState {
                phase: "initializer"
            })
        ));
        let snapshot = RootStateSnapshotV1 {
            state_identity: fixture.contract.roles.state.identity.clone(),
            state_layout: fixture.contract.roles.state.layout,
            event_identity: fixture.contract.roles.event.identity.clone(),
            event_layout: fixture.contract.roles.event.layout,
            phase: RootPhase::Initialized {
                value: RuntimePayload(fixture.state(affine_value())),
                reducer: RootReducerPhase::Ready,
            },
            next_sequence: TransitionSequence::ZERO,
            failure: None,
        };
        assert!(matches!(
            RootRuntime::from_snapshot(fixture.contract.clone(), snapshot, program),
            Err(RootRuntimeError::AffineRootState { phase: "saved" })
        ));
    }
}

#[test]
fn reducer_receives_one_owned_affine_event_without_copying_it() {
    let fixture = Fixture::with_affine_field(AffineField::Event);
    for program in fixture.programs() {
        let mut root = fixture.start(program);
        root.ingress(vec![fixture.event(affine_value())], program)
            .unwrap();
        let (callable, args) = root.begin_reduction().unwrap().unwrap().into_parts();
        assert_eq!(callable, fixture.contract.roles.reducer);
        assert_eq!(args.len(), 2);
        assert!(args[1].ownership() == crate::value::ownership::RuntimeValueOwnership::Affine);
        let result = root
            .complete_reduction(
                Ok(reduction(fixture.state(RuntimeValue::Bool(false)))),
                program,
            )
            .unwrap();
        assert!(matches!(result, RootTransitionOutcome::Committed { .. }));
        assert!(root.begin_reduction().unwrap().is_none());
        assert_eq!(
            root.snapshot_state()
                .unwrap()
                .phase
                .state_value()
                .unwrap()
                .0,
            fixture.state(RuntimeValue::Bool(false))
        );
    }
}

#[test]
fn event_batch_admission_is_atomic_and_preserves_the_transition_cursor() {
    for (mut root, owner) in [
        retained_root_fixture(RootExecutionLimits::engine_default()),
        retained_awbc_root_fixture(),
    ] {
        let before = root.inert_rollback_image(&owner).unwrap();
        let event = owner
            .types()
            .try_record_value(
                root.roles.event.semantic_identity,
                vec![],
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap();
        assert!(matches!(
            root.ingress(
                vec![
                    RootEventInput::new(RuntimePayload(event)),
                    RootEventInput::new(RuntimePayload(RuntimeValue::option_some(
                        RuntimeValue::Tuple(vec![RuntimeValue::Unit])
                    )))
                ],
                owner.types()
            ),
            Err(RootRuntimeError::InvalidEvent(_))
        ));
        assert_eq!(root.inert_rollback_image(&owner).unwrap(), before);
    }
}

fn retained_awbc_root_fixture() -> (RootRuntime, crate::task::RuntimeProgramOwner) {
    let program = crate::awbc::stateful_root_fixture();
    program
        .verify(
            crate::awbc::verify::AwbcVerifyBudget::default(),
            crate::awbc::verify::AwbcVerifyContext::default(),
        )
        .unwrap();
    let entry = &program.entries[0];
    let roles = entry.roles.stateful().unwrap();
    let contract = RootStartupContract {
        entry: entry.runtime_id.clone(),
        roles: roles.clone(),
        initial_flow: roles.initial_flow.flow.clone(),
        initial_state_parameter: crate::entry::FlowParameterCoordinate::from_position(0),
    };
    let owner = crate::task::RuntimeProgramOwner::Awbc(std::sync::Arc::new(program));
    let snapshot = RootStateSnapshotV1 {
        state_identity: contract.roles.state.identity.clone(),
        state_layout: contract.roles.state.layout,
        event_identity: contract.roles.event.identity.clone(),
        event_layout: contract.roles.event.layout,
        phase: RootPhase::Initialized {
            value: RuntimePayload(
                owner
                    .types()
                    .try_record_value(
                        contract.roles.state.semantic_identity,
                        vec![],
                        contract.roles.command_policy.root_limits.schema,
                    )
                    .unwrap(),
            ),
            reducer: RootReducerPhase::Ready,
        },
        next_sequence: TransitionSequence::ZERO,
        failure: None,
    };
    (
        RootRuntime::from_snapshot(contract, snapshot, owner.types()).unwrap(),
        owner,
    )
}

fn retained_root_fixture(
    limits: RootExecutionLimits,
) -> (RootRuntime, crate::task::RuntimeProgramOwner) {
    let plan = std::sync::Arc::new(crate::plan::body_semantic::stateful_root_fixture(
        Schema::Bool,
        limits,
    ));
    let owner = crate::task::RuntimeProgramOwner::Plan(std::sync::Arc::clone(&plan));
    let contract = RootStartupContract::from_runtime_plan(&plan, &plan.entries()[0].id).unwrap();
    let snapshot = RootStateSnapshotV1 {
        state_identity: contract.roles.state.identity.clone(),
        state_layout: contract.roles.state.layout,
        event_identity: contract.roles.event.identity.clone(),
        event_layout: contract.roles.event.layout,
        phase: RootPhase::Initialized {
            value: RuntimePayload(
                owner
                    .types()
                    .try_record_value(semantic(1), vec![], limits.schema)
                    .unwrap(),
            ),
            reducer: RootReducerPhase::Ready,
        },
        next_sequence: TransitionSequence::ZERO,
        failure: None,
    };
    (
        RootRuntime::from_snapshot(contract, snapshot, owner.types()).unwrap(),
        owner,
    )
}

fn active_entry_for_root(
    owner: &crate::task::RuntimeProgramOwner,
    contract: &RootStartupContract,
) -> crate::entry::ActiveEntrySnapshotV1 {
    let kind = match owner {
        crate::task::RuntimeProgramOwner::Plan(plan) => plan
            .entries()
            .iter()
            .find(|entry| entry.id == contract.entry)
            .unwrap()
            .kind
            .clone(),
        crate::task::RuntimeProgramOwner::Awbc(program) => program
            .entries
            .iter()
            .find(|entry| entry.runtime_id == contract.entry)
            .unwrap()
            .kind
            .runtime_kind(&program.strings)
            .unwrap(),
    };
    crate::entry::ActiveEntrySnapshotV1 {
        id: contract.entry.clone(),
        kind,
        binding: contract.roles.binding,
    }
}

#[test]
fn failed_root_durable_lifecycle_round_trips_and_rejects_forged_headers() {
    for (mut root, owner) in [
        retained_root_fixture(RootExecutionLimits::engine_default()),
        retained_awbc_root_fixture(),
    ] {
        let event = owner
            .types()
            .try_record_value(
                root.roles.event.semantic_identity,
                vec![],
                root.limits.schema,
            )
            .unwrap();
        root.ingress(
            vec![RootEventInput::new(RuntimePayload(event))],
            owner.types(),
        )
        .unwrap();
        drop(root.begin_reduction().unwrap().unwrap());
        let outcome = root
            .complete_reduction(
                Err(RootCallableEvaluationError::new("failed Root lifecycle")),
                owner.types(),
            )
            .unwrap();
        assert!(matches!(outcome, RootTransitionOutcome::Trapped { .. }));
        let contract =
            RootStartupContract::from_program(owner.types(), &root.active.entry).unwrap();
        let active = active_entry_for_root(&owner, &contract);
        let before = root.snapshot_state().unwrap();
        let saved = RootStateSaveSnapshotV1::from_live(&before, &active, &owner).unwrap();
        let encoded = serde_json::to_vec(&saved).unwrap();
        let decoded: RootStateSaveSnapshotV1 = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, saved);
        let live = decoded.into_live(&active, &owner).unwrap();
        let mut restored = RootRuntime::from_snapshot(contract, live, owner.types()).unwrap();
        assert_eq!(restored.snapshot_state().unwrap(), before);
        assert!(matches!(
            restored.begin_reduction(),
            Err(RootRuntimeError::Failed)
        ));

        for mutation in 0..5 {
            let mut invalid = saved.clone();
            let RootRuntimeFailure::Transition {
                sequence,
                message,
                digest,
            } = invalid.failure.as_mut().unwrap();
            match mutation {
                0 => *sequence = TransitionSequence::from_u64(1),
                1 => message.push('!'),
                2 => *digest = RuntimeValueDigest::from_bytes([0; 32]),
                3 => {
                    let RootPhase::Initialized { reducer, .. } = &mut invalid.phase else {
                        unreachable!()
                    };
                    *reducer = RootReducerPhase::Evaluating;
                }
                4 => {
                    *sequence = TransitionSequence::from_u64(u64::MAX);
                    invalid.next_sequence = *sequence;
                }
                _ => unreachable!(),
            }
            assert!(invalid.into_live(&active, &owner).is_err());
            assert_eq!(restored.snapshot_state().unwrap(), before);
        }
        let mut missing = serde_json::to_value(&saved).unwrap();
        missing.as_object_mut().unwrap().remove("failure");
        assert!(serde_json::from_value::<RootStateSaveSnapshotV1>(missing).is_err());
        let mut unknown = serde_json::to_value(&saved).unwrap();
        unknown["failure"]["kind"] = serde_json::json!("unknown_failure");
        assert!(serde_json::from_value::<RootStateSaveSnapshotV1>(unknown).is_err());
    }
}

#[test]
fn root_state_capture_rejects_oversized_failure_metadata_before_copying() {
    let (mut root, _) = retained_awbc_root_fixture();
    let size =
        usize::try_from(RuntimeSchemaLimits::engine_default().max_metadata_bytes).unwrap() + 1;
    let message = "m".repeat(size);
    let pointer = message.as_ptr();
    let digest = RootRuntimeFailure::message_digest(&message);
    root.failure = Some(RootRuntimeFailure::Transition {
        sequence: root.active.next_sequence,
        message,
        digest,
    });
    assert!(matches!(
        root.snapshot_state(),
        Err(RootRuntimeError::InvalidSnapshotMetadata(_))
    ));
    let RootRuntimeFailure::Transition { message, .. } = root.failure.as_ref().unwrap();
    assert_eq!(message.as_ptr(), pointer);
    assert_eq!(message.len(), size);
}

#[test]
fn pending_initializer_has_no_value_and_round_trips_under_zero_value_allowance() {
    for (initialized, owner) in [
        retained_root_fixture(RootExecutionLimits::engine_default()),
        retained_awbc_root_fixture(),
    ] {
        let contract =
            RootStartupContract::from_program(owner.types(), &initialized.active.entry).unwrap();
        let pending = RootRuntime::prepare(contract.clone(), owner.types()).unwrap();
        assert_eq!(
            pending.initializer_callable().unwrap(),
            &contract.roles.initializer
        );
        assert!(pending.active.phase.state_value().is_none());
        let mut visited = 0;
        pending.visit_runtime_values(|_| visited += 1);
        assert_eq!(visited, 0);
        assert_eq!(pending.save_blockers(), RootSaveBlockers::default());
        let allowance = RuntimeSchemaLimits {
            max_nodes: 0,
            max_encoded_bytes: 0,
            ..RuntimeSchemaLimits::engine_default()
        };
        let mut admission =
            crate::value::RuntimeRetainedValueAdmission::for_program(owner.clone(), allowance);
        pending.admit_retained(&mut admission).unwrap();
        let image = pending.inert_rollback_image(&owner).unwrap();
        assert_eq!(
            RootRuntime::from_rollback_image(image, &owner).unwrap(),
            pending
        );

        let active = active_entry_for_root(&owner, &contract);
        let live = pending.snapshot_state().unwrap();
        let wire = RootStateSaveSnapshotV1::from_live(&live, &active, &owner).unwrap();
        assert!(wire.retained_value().is_none());
        let encoded = serde_json::to_vec(&wire).unwrap();
        let decoded: RootStateSaveSnapshotV1 = serde_json::from_slice(&encoded).unwrap();
        let mut admission =
            crate::value::RuntimeRetainedValueAdmission::for_program(owner.clone(), allowance);
        decoded.admit_retained(&active, &mut admission).unwrap();
        let restored = decoded.into_live(&active, &owner).unwrap();
        assert_eq!(restored, live);
        assert_eq!(
            RootRuntime::from_snapshot(contract.clone(), restored, owner.types()).unwrap(),
            pending
        );

        let mut wrong_coordinate = pending.snapshot_state().unwrap();
        let RootPhase::Initializing {
            state_parameter, ..
        } = &mut wrong_coordinate.phase
        else {
            unreachable!()
        };
        *state_parameter = crate::entry::FlowParameterCoordinate::from_position(1);
        assert!(RootStateSaveSnapshotV1::from_live(&wrong_coordinate, &active, &owner).is_err());
        let mut wrong_sequence = pending.snapshot_state().unwrap();
        wrong_sequence.next_sequence = TransitionSequence::from_u64(1);
        assert!(RootRuntime::from_snapshot(contract, wrong_sequence, owner.types()).is_err());
        assert_eq!(pending.snapshot_state().unwrap(), live);
    }
}

#[test]
fn rejected_initializer_retains_pending_owner_and_queued_events_before_retry() {
    for (initialized, owner) in [
        retained_root_fixture(RootExecutionLimits::engine_default()),
        retained_awbc_root_fixture(),
    ] {
        let contract =
            RootStartupContract::from_program(owner.types(), &initialized.active.entry).unwrap();
        let mut pending = RootRuntime::prepare(contract.clone(), owner.types()).unwrap();
        let event = owner
            .types()
            .try_record_value(
                contract.roles.event.semantic_identity,
                vec![],
                pending.limits.schema,
            )
            .unwrap();
        pending
            .ingress(
                vec![RootEventInput::new(RuntimePayload(event))],
                owner.types(),
            )
            .unwrap();
        assert!(matches!(
            pending.begin_reduction(),
            Err(RootRuntimeError::InitializationPending)
        ));
        let before = pending.inert_rollback_image(&owner).unwrap();
        let rejected = pending
            .complete_initialization(RuntimeValue::Bool(false), owner.types())
            .unwrap_err();
        assert!(matches!(
            rejected.error,
            RootRuntimeError::InvalidInitialValue(_)
        ));
        assert_eq!(rejected.root.inert_rollback_image(&owner).unwrap(), before);
        let value = owner
            .types()
            .try_record_value(
                contract.roles.state.semantic_identity,
                vec![],
                rejected.root.limits.schema,
            )
            .unwrap();
        let mut startup = rejected
            .root
            .complete_initialization(value, owner.types())
            .unwrap();
        assert_eq!(startup.initial_flow, contract.initial_flow);
        assert_eq!(
            startup.initial_state_binding.parameter,
            contract.initial_state_parameter
        );
        assert_eq!(
            &startup.initial_state_binding.value,
            &startup.root.active.phase.state_value().unwrap().0
        );
        assert_eq!(startup.root.active.pending_event_count(), 1);
        assert!(matches!(
            startup.root.initializer_callable(),
            Err(RootRuntimeError::InitializationAlreadyCompleted)
        ));
        let _invocation = startup.root.begin_reduction().unwrap().unwrap();
        assert!(startup.root.active.reducer_active());
        let active = active_entry_for_root(&owner, &contract);
        assert!(
            RootStateSaveSnapshotV1::from_live(
                &startup.root.snapshot_state().unwrap(),
                &active,
                &owner
            )
            .is_err()
        );
    }
}

#[test]
fn complete_root_admission_bounds_state_events_and_committed_commands_together() {
    use crate::task::RuntimeProgramOwner;
    use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission};
    let (mut root, owner) = retained_root_fixture(RootExecutionLimits::engine_default());
    root.active.next_sequence = TransitionSequence::from_u64(1);
    root.ingress(
        vec![RootEventInput::new(RuntimePayload(
            owner
                .types()
                .try_record_value(semantic(2), vec![], root.limits.schema)
                .unwrap(),
        ))],
        owner.types(),
    )
    .unwrap();
    root.active
        .committed_commands
        .push_back(RuntimeCommandEnvelope {
            transition: TransitionSequence::ZERO,
            index: 0,
            command: RuntimeCommand::new_accepted(
                RuntimeCommandConstructorId::try_new("set").unwrap(),
                RuntimeCommandTargetId::try_new("output").unwrap(),
                RuntimePayload(RuntimeValue::Bool(true)),
            ),
        });
    let mut visited = 0;
    root.visit_runtime_values(|_| visited += 1);
    assert_eq!(visited, 3);
    let image = root.inert_rollback_image(&owner).unwrap();
    let before = image.clone();
    let limits = |nodes| RuntimeSchemaLimits {
        max_nodes: nodes,
        ..RuntimeSchemaLimits::engine_default()
    };
    let mut live = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits(2));
    let failure = root.admit_retained(&mut live).unwrap_err();
    assert_eq!(
        failure,
        AwbcRuntimeValueSnapshotError::Resource(crate::entry::RuntimeSchemaError::BudgetExceeded {
            budget: "nodes"
        })
    );
    assert_eq!(root.admit_retained(&mut live).unwrap_err(), failure);
    let mut inert = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits(2));
    assert_eq!(image.admit_retained(&mut inert).unwrap_err(), failure);
    assert_eq!(image, before);
    let mut exact = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits(3));
    root.admit_retained(&mut exact).unwrap();
    assert_eq!(root.snapshot_admitted(&owner).unwrap(), image);
    let mut exact = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits(3));
    image.admit_retained(&mut exact).unwrap();
    assert_eq!(
        RootRuntime::from_admitted_rollback_image(image.clone(), &owner).unwrap(),
        root
    );
    let mut forged = image;
    forged.active.committed_commands[0].command = RuntimeCommand::new_accepted(
        RuntimeCommandConstructorId::try_new("set").unwrap(),
        RuntimeCommandTargetId::try_new("output").unwrap(),
        crate::value::AwbcRuntimeValueSnapshot::Unit,
    );
    let rejected = forged.clone();
    let mut admission = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits(3));
    let failure = forged.admit_retained(&mut admission).unwrap_err();
    assert_eq!(root.admit_retained(&mut admission).unwrap_err(), failure);
    assert_eq!(forged, rejected);
    assert!(RootRuntime::from_rollback_image(forged, &owner).is_err());

    let RuntimeProgramOwner::Plan(original) = owner else {
        unreachable!()
    };
    let foreign = crate::plan::body_semantic::stateful_root_fixture(
        Schema::String,
        RootExecutionLimits::engine_default(),
    );
    assert!(
        root.admit_retained(&mut RuntimeRetainedValueAdmission::for_program(
            RuntimeProgramOwner::Plan(std::sync::Arc::new(foreign)),
            limits(3)
        ))
        .is_err()
    );
    assert!(
        root.admit_retained(&mut RuntimeRetainedValueAdmission::for_program(
            RuntimeProgramOwner::Plan(original),
            limits(3)
        ))
        .is_ok()
    );
}

#[test]
fn canonical_result_payload_reduces_and_bad_nested_state_never_commits() {
    let fixture = Fixture::new();
    for program in fixture.programs() {
        let mut root = fixture.start(program);
        root.ingress(vec![fixture.event(RuntimeValue::Bool(false))], program)
            .unwrap();
        let invocation = root.begin_reduction().unwrap().unwrap();
        drop(invocation);
        let outcome = root
            .complete_reduction(
                Ok(reduction(fixture.state(RuntimeValue::Bool(false)))),
                program,
            )
            .unwrap();
        assert!(matches!(outcome, RootTransitionOutcome::Committed { .. }));
        assert!(root.begin_reduction().unwrap().is_none());
        assert_eq!(
            root.snapshot_state()
                .unwrap()
                .phase
                .state_value()
                .unwrap()
                .0,
            fixture.state(RuntimeValue::Bool(false))
        );
        let before = root.snapshot_state().unwrap();
        root.ingress(vec![fixture.event(RuntimeValue::Bool(true))], program)
            .unwrap();
        let invocation = root.begin_reduction().unwrap().unwrap();
        drop(invocation);
        let outcome = root
            .complete_reduction(Ok(reduction(fixture.state(RuntimeValue::Unit))), program)
            .unwrap();
        let RootTransitionOutcome::Trapped {
            sequence,
            failure_digest,
            message,
        } = outcome
        else {
            panic!("invalid nested state traps the transition");
        };
        let mut expected = before;
        expected.failure = Some(RootRuntimeFailure::Transition {
            sequence,
            message,
            digest: failure_digest,
        });
        assert_eq!(root.snapshot_state().unwrap(), expected);
    }
}

#[test]
fn save_002_active_reducer_reports_exact_blocker() {
    let fixture = Fixture::new();
    for program in fixture.programs() {
        let mut root = fixture.start(program);
        root.ingress(vec![fixture.event(RuntimeValue::Bool(true))], program)
            .unwrap();
        let _invocation = root.begin_reduction().unwrap().unwrap();
        assert_eq!(
            root.save_blockers(),
            RootSaveBlockers {
                reducer_active: true,
                pending_events: 0,
                pending_commands: 0
            }
        );
    }
}

#[test]
fn suspended_reducer_keeps_committed_state_and_sequences_later_ingress() {
    let fixture = Fixture::new();
    for program in fixture.programs() {
        let mut root = fixture.start(program);
        let mut before = root.snapshot_state().unwrap();
        assert!(matches!(
            root.complete_reduction(
                Ok(reduction(fixture.state(RuntimeValue::Bool(false)))),
                program
            ),
            Err(RootRuntimeError::NoActiveReducer)
        ));
        root.ingress(vec![fixture.event(RuntimeValue::Bool(true))], program)
            .unwrap();
        let invocation = root.begin_reduction().unwrap().unwrap();
        let (callable, args) = invocation.into_parts();
        assert_eq!(callable, fixture.contract.roles.reducer);
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], before.phase.state_value().unwrap().0);
        let RootPhase::Initialized { reducer, .. } = &mut before.phase else {
            unreachable!()
        };
        *reducer = RootReducerPhase::Evaluating;
        assert_eq!(root.snapshot_state().unwrap(), before);
        assert!(matches!(
            root.begin_reduction(),
            Err(RootRuntimeError::ReducerAlreadyActive)
        ));
        assert_eq!(root.snapshot_state().unwrap(), before);

        root.ingress(vec![fixture.event(RuntimeValue::Bool(false))], program)
            .unwrap();
        assert_eq!(root.active.queued_events[0].sequence.get(), 1);
        let completed = root
            .complete_reduction(
                Ok(reduction(fixture.state(RuntimeValue::Bool(false)))),
                program,
            )
            .unwrap();
        assert!(matches!(completed, RootTransitionOutcome::Committed { .. }));
        assert_eq!(completed.sequence().get(), 0);
        assert_eq!(root.active.next_sequence.get(), 1);
        let after = root.snapshot_state().unwrap();
        assert!(matches!(
            root.complete_reduction(Ok(RuntimeValue::Unit), program),
            Err(RootRuntimeError::NoActiveReducer)
        ));
        assert_eq!(root.snapshot_state().unwrap(), after);

        let _next = root.begin_reduction().unwrap().unwrap();
        let completed = root
            .complete_reduction(
                Ok(reduction(fixture.state(RuntimeValue::Bool(true)))),
                program,
            )
            .unwrap();
        assert_eq!(completed.sequence().get(), 1);
        assert_eq!(root.active.next_sequence.get(), 2);
        assert!(root.begin_reduction().unwrap().is_none());
    }
}

#[test]
fn suspended_reducer_failure_does_not_publish_and_clears_following_events() {
    let fixture = Fixture::new();
    for program in fixture.programs() {
        for returned in [
            Ok(RuntimeValue::Unit),
            Err(RootCallableEvaluationError::new(
                "executor trapped after yield",
            )),
        ] {
            let mut root = fixture.start(program);
            let before = root.snapshot_state().unwrap();
            root.ingress(vec![fixture.event(RuntimeValue::Bool(true))], program)
                .unwrap();
            let _invocation = root.begin_reduction().unwrap().unwrap();
            root.ingress(vec![fixture.event(RuntimeValue::Bool(false))], program)
                .unwrap();
            let completed = root.complete_reduction(returned, program).unwrap();
            let RootTransitionOutcome::Trapped {
                sequence,
                failure_digest,
                message,
            } = completed
            else {
                panic!("invalid result or executable trap fails the transition");
            };
            assert_eq!(sequence, TransitionSequence::ZERO);
            assert_eq!(root.active.pending_command_count(), 0);
            let mut expected = before;
            expected.failure = Some(RootRuntimeFailure::Transition {
                sequence,
                message,
                digest: failure_digest,
            });
            assert_eq!(root.snapshot_state().unwrap(), expected);
            assert_eq!(root.active.pending_event_count(), 0);
            assert!(!root.active.reducer_active());
            assert!(matches!(
                root.begin_reduction(),
                Err(RootRuntimeError::Failed)
            ));
        }
    }
}

#[test]
fn suspended_reducer_phase_and_event_frontier_survive_inert_rollback() {
    for (mut root, owner) in [
        retained_root_fixture(RootExecutionLimits::engine_default()),
        retained_awbc_root_fixture(),
    ] {
        let event = || {
            RootEventInput::new(RuntimePayload(
                owner
                    .types()
                    .try_record_value(
                        root.roles.event.semantic_identity,
                        vec![],
                        root.limits.schema,
                    )
                    .unwrap(),
            ))
        };
        let first = event();
        let second = event();
        root.ingress(vec![first], owner.types()).unwrap();
        let _invocation = root.begin_reduction().unwrap().unwrap();
        root.ingress(vec![second], owner.types()).unwrap();
        let image = root.inert_rollback_image(&owner).unwrap();
        let mut restored = RootRuntime::from_rollback_image(image.clone(), &owner).unwrap();
        assert_eq!(restored, root);
        assert_eq!(restored.inert_rollback_image(&owner).unwrap(), image);
        assert!(matches!(
            restored.begin_reduction(),
            Err(RootRuntimeError::ReducerAlreadyActive)
        ));
        let outcome = restored
            .complete_reduction(
                Err(RootCallableEvaluationError::new(
                    "restored invocation trapped",
                )),
                owner.types(),
            )
            .unwrap();
        let RootTransitionOutcome::Trapped {
            sequence,
            failure_digest,
            message,
        } = outcome
        else {
            panic!("restored invocation traps without committing state");
        };
        assert_eq!(sequence, TransitionSequence::ZERO);
        let mut expected = root.snapshot_state().unwrap();
        let RootPhase::Initialized { reducer, .. } = &mut expected.phase else {
            unreachable!()
        };
        *reducer = RootReducerPhase::Ready;
        expected.failure = Some(RootRuntimeFailure::Transition {
            sequence,
            message,
            digest: failure_digest,
        });
        assert_eq!(restored.snapshot_state().unwrap(), expected);
        assert_eq!(restored.active.pending_event_count(), 0);
    }
}

#[test]
fn role_wire_metadata_cannot_reintroduce_a_schema_copy() {
    let fixture = Fixture::new();
    let role = fixture.contract.roles.state;
    let mut wire = serde_json::to_value(&role).unwrap();
    assert_eq!(
        serde_json::from_value::<RuntimeNominalRole>(wire.clone()).unwrap(),
        role
    );
    wire.as_object_mut()
        .unwrap()
        .insert("schema".into(), serde_json::to_value(Schema::Bool).unwrap());
    assert!(serde_json::from_value::<RuntimeNominalRole>(wire).is_err());
}
