use super::*;
use crate::{
    awbc::schema::{
        AwbcProgram, AwbcRecordField, AwbcRuntimeType, AwbcRuntimeTypeShape as AwbcType,
        AwbcStringId, AwbcTypeId, AwbcVariantCase, AwbcVariantIdentity,
    },
    entry::{
        CallableContractHash, FlowContractHash, RuntimeCallableId, RuntimeCommandPolicy,
        RuntimeFlowRole, RuntimeNominalRecordShape as Shape, RuntimeNominalRole,
        RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaField, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId, RuntimeTypeSchema as Schema,
    },
    pattern::{RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId},
    plan::{
        RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlanBuilder,
        RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed, RuntimeVariantCaseSeed,
        RuntimeVariantDomainSeed,
    },
    program_types::RuntimeProgramTypes,
    value::{RuntimeNominalRecordValue, RuntimeRecordFieldId},
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

impl Fixture {
    fn new() -> Self {
        let graph = RuntimeNominalSchemaGraph::try_new(
            vec![
                RuntimeNominalSchemaDefinition::new(
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
                    identity(2),
                    vec![],
                    RuntimeNominalSchemaBody::Variant {
                        cases: vec![
                            RuntimeNominalSchemaCase::new(0, "Tick".into(), None),
                            RuntimeNominalSchemaCase::new(
                                1,
                                "Set".into(),
                                Some(Schema::Tuple(vec![Schema::Bool].into())),
                            ),
                        ]
                        .into(),
                    },
                ),
                RuntimeNominalSchemaDefinition::new(
                    identity(3),
                    vec![],
                    RuntimeNominalSchemaBody::Record {
                        shape: Shape::Newtype,
                        fields: vec![RuntimeNominalSchemaField::new(field(), None, Schema::Bool)]
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
            RuntimePlanTypeSeed::new(semantic(5), Type::Tuple(vec![semantic(4)].into())),
        ]);
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
                            semantic(4),
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
        let awbc = AwbcProgram {
            strings: vec![
                "root.State".into(),
                "root.Event".into(),
                "root.Payload".into(),
                "payload".into(),
                "Tick".into(),
                "Set".into(),
            ],
            runtime_types: vec![
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
                            ty: AwbcTypeId(3),
                        }],
                    },
                ),
                AwbcRuntimeType::new(semantic(4), AwbcType::Bool),
                AwbcRuntimeType::new(semantic(5), AwbcType::Tuple(vec![AwbcTypeId(3)])),
            ],
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
        RootEventInput::new(RuntimePayload(RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Nominal {
                nominal: nominal(2),
                semantic_identity: semantic(2),
                layout: self.contract.roles.event.layout,
            },
            ordinal: 1,
            name: "Set".into(),
            payload: Some(Box::new(RuntimeValue::Tuple(vec![field_value]))),
        }))
    }
    fn start(&self, program: RuntimeProgramTypes<'_>) -> RootRuntime {
        RootRuntime::start(
            self.contract.clone(),
            &mut ReturnValue::new(self.state(RuntimeValue::Bool(true))),
            program,
        )
        .unwrap()
        .root
    }
}

struct ReturnValue {
    value: RuntimeValue,
    calls: usize,
}
impl ReturnValue {
    fn new(value: RuntimeValue) -> Self {
        Self { value, calls: 0 }
    }
}
impl RootCallableEvaluator for ReturnValue {
    fn evaluate_root_callable(
        &mut self,
        _: &RuntimeCallableRole,
        _: &[RuntimeValue],
    ) -> Result<RuntimeValue, RootCallableEvaluationError> {
        self.calls += 1;
        Ok(self.value.clone())
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
fn initializer_and_snapshot_validate_nested_values_through_each_program() {
    let fixture = Fixture::new();
    for program in fixture.programs() {
        let root = fixture.start(program);
        let snapshot = root.snapshot_state();
        assert_eq!(
            RootRuntime::from_snapshot(fixture.contract.clone(), snapshot.clone(), program)
                .unwrap()
                .snapshot_state(),
            snapshot
        );
        let invalid = fixture.state(RuntimeValue::String("not Bool".into()));
        assert!(matches!(
            RootRuntime::start(
                fixture.contract.clone(),
                &mut ReturnValue::new(invalid.clone()),
                program
            ),
            Err(RootRuntimeError::InvalidInitialValue(_))
        ));
        let mut snapshot = snapshot;
        snapshot.value = RuntimePayload(invalid);
        assert!(matches!(
            RootRuntime::from_snapshot(fixture.contract.clone(), snapshot, program),
            Err(RootRuntimeError::InvalidSnapshotValue(_))
        ));
        let mut wrong_role = fixture.contract.clone();
        wrong_role.roles.event.layout = TypeLayoutHash::from_bytes([0; 32]);
        let mut evaluator = ReturnValue::new(fixture.state(RuntimeValue::Bool(true)));
        assert!(matches!(
            RootRuntime::start(wrong_role, &mut evaluator, program),
            Err(RootRuntimeError::InvalidRole { role: "event", .. })
        ));
        assert_eq!(evaluator.calls, 0);
    }
}

#[test]
fn event_batch_admission_is_atomic_and_preserves_the_transition_cursor() {
    let fixture = Fixture::new();
    for program in fixture.programs() {
        let mut root = fixture.start(program);
        let before = root.clone();
        assert!(matches!(
            root.ingress(
                vec![
                    fixture.event(RuntimeValue::Bool(true)),
                    fixture.event(RuntimeValue::Unit)
                ],
                program
            ),
            Err(RootRuntimeError::InvalidEvent(_))
        ));
        assert_eq!(root, before);
    }
}

#[test]
fn canonical_result_payload_reduces_and_bad_nested_state_never_commits() {
    let fixture = Fixture::new();
    for program in fixture.programs() {
        let mut root = fixture.start(program);
        let mut evaluator = ReturnValue::new(reduction(fixture.state(RuntimeValue::Bool(false))));
        let outcome = root
            .step(
                vec![fixture.event(RuntimeValue::Bool(false))],
                &mut evaluator,
                program,
            )
            .unwrap();
        assert!(!outcome.failed);
        assert_eq!(outcome.outcomes.len(), 1);
        assert_eq!(evaluator.calls, 1);
        assert_eq!(
            root.snapshot_state().value.0,
            fixture.state(RuntimeValue::Bool(false))
        );
        let before = root.snapshot_state();
        let mut evaluator = ReturnValue::new(reduction(fixture.state(RuntimeValue::Unit)));
        let outcome = root
            .step(
                vec![fixture.event(RuntimeValue::Bool(true))],
                &mut evaluator,
                program,
            )
            .unwrap();
        assert!(outcome.failed);
        assert_eq!(root.snapshot_state(), before);
    }
}

#[test]
fn save_002_active_reducer_reports_exact_blocker() {
    let fixture = Fixture::new();
    for program in fixture.programs() {
        let mut root = fixture.start(program);
        root.active.reducer_active = true;
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
