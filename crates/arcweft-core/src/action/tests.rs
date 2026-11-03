use super::*;
use crate::entry::{
    RuntimeNominalDeclarationId, RuntimeNominalRecordShape, RuntimeNominalSchemaBody,
    RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
    RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema,
};
use crate::plan::{
    RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use crate::value::{RuntimeNominalRecordValue, RuntimeRecordFieldId, RuntimeValue};
use std::sync::Arc;

fn identity(byte: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([byte; 32])
}

struct Fixture {
    owner: RuntimeProgramOwner,
    schema: RuntimeNominalSchemaGraph,
}

impl Fixture {
    fn new() -> Self {
        let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
        let definitions = (1..=2)
            .map(|byte| {
                RuntimeNominalSchemaDefinition::new(
                    RuntimeNominalDeclarationId::try_action(
                        [byte; 32],
                        arcweft_id::PublicId::try_new_engine_owned(format!("action.channel{byte}"))
                            .unwrap(),
                    )
                    .unwrap(),
                    RuntimeNominalSchemaIdentity::new(Self::nominal(byte), identity(byte)),
                    vec![],
                    RuntimeNominalSchemaBody::Record {
                        shape: RuntimeNominalRecordShape::Record,
                        fields: vec![RuntimeNominalSchemaField::new(
                            field,
                            Some("value".into()),
                            RuntimeTypeSchema::Bool,
                        )]
                        .into_boxed_slice(),
                    },
                )
            })
            .collect::<Vec<_>>();
        let schema =
            RuntimeNominalSchemaGraph::try_new(definitions, RuntimeSchemaLimits::engine_default())
                .unwrap();
        let mut types = vec![RuntimePlanTypeSeed::new(
            identity(3),
            RuntimePlanTypeProjection::Bool,
        )];
        let mut domains = Vec::new();
        for byte in 1..=2 {
            types.push(RuntimePlanTypeSeed::new(
                identity(byte),
                RuntimePlanTypeProjection::Nominal {
                    nominal: Self::nominal(byte),
                    layout: schema.try_layout_hash(identity(byte)).unwrap(),
                    arguments: Box::default(),
                },
            ));
            domains.push(RuntimeNominalRecordDomainSeed::new(
                identity(byte),
                RuntimeNominalRecordShape::Record,
                [RuntimeNominalRecordDomainFieldSeed::new(
                    field,
                    Some("value".into()),
                    identity(3),
                )],
            ));
        }
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_semantic_batch(types, [], domains, [], &schema)
            .unwrap();
        Self {
            owner: RuntimeProgramOwner::Plan(Arc::new(builder.finish().unwrap())),
            schema,
        }
    }

    fn nominal(byte: u8) -> RuntimeNominalTypeId {
        RuntimeNominalTypeId::try_new(format!("Channel{byte}")).unwrap()
    }

    fn payload(&self, byte: u8, value: bool) -> RuntimePayload {
        RuntimePayload::new(RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            Self::nominal(byte),
            identity(byte),
            self.schema.try_layout_hash(identity(byte)).unwrap(),
            vec![RuntimeValue::Bool(value)],
        )))
    }

    fn admission(&self) -> RuntimeRetainedValueAdmission {
        RuntimeRetainedValueAdmission::for_program(
            self.owner.clone(),
            RuntimeSchemaLimits::engine_default(),
        )
    }
}

fn boolean(payload: RuntimePayload) -> bool {
    let RuntimeValue::NominalRecord(record) = payload.0 else {
        panic!("nominal payload")
    };
    let [RuntimeValue::Bool(value)] = record.fields() else {
        panic!("one Boolean field")
    };
    *value
}

#[test]
fn original_action_messages_keep_fifo_and_isolate_other_channels() {
    let fixture = Fixture::new();
    let mut mailbox = RuntimeActionMailbox::default();
    for (channel, value) in [(1, false), (2, true), (1, true)] {
        mailbox
            .emit(
                identity(channel),
                fixture.payload(channel, value),
                &mut fixture.admission(),
            )
            .unwrap();
    }
    assert!(mailbox.has_message(identity(2)));
    assert!(!boolean(
        mailbox
            .receive(identity(1), &mut fixture.admission())
            .unwrap()
            .unwrap()
    ));
    assert!(boolean(
        mailbox
            .receive(identity(1), &mut fixture.admission())
            .unwrap()
            .unwrap()
    ));
    assert!(
        mailbox
            .receive(identity(1), &mut fixture.admission())
            .unwrap()
            .is_none()
    );
    assert!(boolean(
        mailbox
            .receive(identity(2), &mut fixture.admission())
            .unwrap()
            .unwrap()
    ));
    assert!(mailbox.messages.is_empty());
}

#[test]
fn refused_action_emission_preserves_the_candidate_and_mailbox_and_first_failure() {
    let fixture = Fixture::new();
    let mut mailbox = RuntimeActionMailbox::default();
    mailbox
        .emit(
            identity(1),
            fixture.payload(1, false),
            &mut fixture.admission(),
        )
        .unwrap();
    let mut admission = fixture.admission();
    let rejected = mailbox
        .emit(identity(1), fixture.payload(2, true), &mut admission)
        .unwrap_err();
    assert!(boolean(rejected.payload));
    assert_eq!(mailbox.messages.len(), 1);
    let first = rejected.error;
    let repeated = mailbox
        .emit(
            identity(3),
            RuntimePayload::new(RuntimeValue::Bool(false)),
            &mut admission,
        )
        .unwrap_err();
    assert_eq!(repeated.error, first);
    assert_eq!(repeated.payload.value(), &RuntimeValue::Bool(false));
    let mut small = RuntimeRetainedValueAdmission::for_program(
        fixture.owner.clone(),
        RuntimeSchemaLimits {
            max_sequence_items: 0,
            ..RuntimeSchemaLimits::engine_default()
        },
    );
    let bounded = mailbox
        .emit(identity(1), fixture.payload(1, true), &mut small)
        .unwrap_err();
    assert!(matches!(
        bounded.error,
        AwbcRuntimeValueSnapshotError::Resource(_)
    ));
    assert!(boolean(bounded.payload));
    assert_eq!(mailbox.messages.len(), 1);
    assert!(!boolean(
        mailbox
            .receive(identity(1), &mut fixture.admission())
            .unwrap()
            .unwrap()
    ));
}

#[test]
fn inert_action_messages_require_original_roles_before_restore_materialization() {
    let fixture = Fixture::new();
    let mut mailbox = RuntimeActionMailbox::default();
    mailbox
        .emit(
            identity(1),
            fixture.payload(1, true),
            &mut fixture.admission(),
        )
        .unwrap();
    mailbox.admit_retained(&mut fixture.admission()).unwrap();
    let snapshot = mailbox.snapshot_admitted(&fixture.owner).unwrap();
    let mut forged = snapshot.clone();
    forged.messages[0].payload_type = identity(3);
    assert!(matches!(forged.admit_retained(&mut fixture.admission()),
        Err(AwbcRuntimeValueSnapshotError::ProgramType(error))
        if matches!(*error, crate::program_types::RuntimeProgramTypeError::NotActionPayload { .. })));
    snapshot.admit_retained(&mut fixture.admission()).unwrap();
    let mut restored = snapshot.into_live_admitted(&fixture.owner).unwrap();
    assert!(boolean(
        restored
            .receive(identity(1), &mut fixture.admission())
            .unwrap()
            .unwrap()
    ));
    assert!(boolean(
        mailbox
            .receive(identity(1), &mut fixture.admission())
            .unwrap()
            .unwrap()
    ));
}
