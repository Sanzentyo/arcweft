use super::*;
use crate::awbc::product_step::checkpoint::ProductCheckpointTarget;
use crate::awbc::schema::AwbcRecordField;
use crate::entry::{
    RuntimeNominalDeclarationId, RuntimeNominalRecordShape, RuntimeNominalSchemaBody,
    RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
    RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema,
};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeNominalRecordValue, RuntimeRecordFieldId, RuntimeRetainedValueAdmission,
};

fn semantic(byte: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([byte; 32])
}

fn fixture() -> AwbcProductStepExecutor {
    let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
    let declaration = RuntimeNominalDeclarationId::try_action(
        [81; 32],
        arcweft_id::PublicId::try_new_engine_owned("action.submit").unwrap(),
    )
    .unwrap();
    let schema = RuntimeNominalSchemaGraph::try_new(
        [RuntimeNominalSchemaDefinition::new(
            declaration.clone(),
            RuntimeNominalSchemaIdentity::new(
                RuntimeNominalTypeId::try_new("Submit").unwrap(),
                semantic(81),
            ),
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
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let mut program = return_program();
    let boolean_type = AwbcTypeId(program.runtime_types.len().try_into().unwrap());
    let name = AwbcStringId(program.strings.len().try_into().unwrap());
    program.strings.extend(["Submit".into(), "value".into()]);
    program.runtime_types.extend([
        AwbcRuntimeType::new(semantic(82), AwbcRuntimeTypeShape::Bool),
        AwbcRuntimeType::new(
            semantic(81),
            AwbcRuntimeTypeShape::NominalRecord {
                public_id: name,
                layout: *schema.try_layout_hash(semantic(81)).unwrap().as_bytes(),
                arguments: vec![],
                shape: RuntimeNominalRecordShape::Record,
                fields: vec![AwbcRecordField {
                    field,
                    name: Some(AwbcStringId(name.0 + 1)),
                    ty: boolean_type,
                }],
            },
        )
        .with_nominal_declaration(declaration),
    ]);
    program.canonicalize_string_table();
    AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap()
}

fn admission(executor: &AwbcProductStepExecutor) -> RuntimeRetainedValueAdmission {
    RuntimeRetainedValueAdmission::for_program(
        RuntimeProgramOwner::Awbc(executor.program_arc()),
        RuntimeSchemaLimits::engine_default(),
    )
}

fn emit(executor: &mut AwbcProductStepExecutor, value: bool) {
    let owner = executor.program_arc();
    let payload_type = owner.semantic_type_id(semantic(81)).unwrap();
    let AwbcRuntimeTypeShape::NominalRecord { layout, .. } =
        owner.runtime_types[payload_type.index()].shape()
    else {
        panic!("original Action record");
    };
    let payload = RuntimePayload::new(RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
        RuntimeNominalTypeId::try_new("Submit").unwrap(),
        semantic(81),
        crate::entry::TypeLayoutHash::from_bytes(*layout),
        vec![RuntimeValue::Bool(value)],
    )));
    let mut admission = admission(executor);
    executor
        .actions
        .emit(semantic(81), payload, &mut admission)
        .unwrap();
}

fn receive(executor: &mut AwbcProductStepExecutor) -> Option<bool> {
    let mut admission = admission(executor);
    let value = executor
        .actions
        .receive(semantic(81), &mut admission)
        .unwrap()?;
    let RuntimeValue::NominalRecord(record) = value.0 else {
        panic!("original Action payload");
    };
    let [RuntimeValue::Bool(value)] = record.fields() else {
        panic!("Boolean field");
    };
    Some(*value)
}

#[test]
fn action_messages_survive_product_wire_save_and_cold_restore_in_order() {
    let mut executor = fixture();
    emit(&mut executor, false);
    emit(&mut executor, true);
    let image = executor.snapshot().unwrap();
    let saved = AwbcProductExecutorSaveSnapshot::from_live(&image, &executor.program).unwrap();
    let bytes = serde_json::to_vec(&saved).unwrap();
    let saved: AwbcProductExecutorSaveSnapshot = serde_json::from_slice(&bytes).unwrap();
    let restored = saved.into_live_for_program(&executor.program).unwrap();
    executor = executor
        .restore_snapshot_owned(restored)
        .unwrap_or_else(|(_, error)| panic!("cold restore: {error}"));
    assert_eq!(receive(&mut executor), Some(false));
    assert_eq!(receive(&mut executor), Some(true));
    assert_eq!(receive(&mut executor), None);
}

#[test]
fn action_checkpoint_restores_consumed_and_added_messages_together_with_the_fiber() {
    let mut executor = fixture();
    emit(&mut executor, false);
    let checkpoint = executor
        .execution_checkpoint(ProductCheckpointTarget::Main)
        .unwrap();
    let before = executor.snapshot().unwrap();
    assert_eq!(receive(&mut executor), Some(false));
    emit(&mut executor, true);
    checkpoint
        .restore(
            &mut executor.fiber,
            &mut executor.presentation,
            &mut executor.actions,
            &executor.program,
        )
        .unwrap();
    assert_eq!(executor.snapshot().unwrap(), before);
    assert_eq!(receive(&mut executor), Some(false));
    assert_eq!(receive(&mut executor), None);
}

#[test]
fn forged_action_row_is_rejected_without_changing_the_live_session() {
    let mut executor = fixture();
    emit(&mut executor, true);
    let before = executor.snapshot().unwrap();
    let mut saved = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    saved.actions[0].0 = semantic(82);
    assert!(saved.into_live_for_program(&executor.program).is_err());
    assert_eq!(executor.snapshot().unwrap(), before);
    assert_eq!(receive(&mut executor), Some(true));
}
