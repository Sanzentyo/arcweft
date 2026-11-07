use super::*;
use crate::awbc::fiber::FiberFlowTransferOrigin;
use crate::line_task::{
    LineRuntimeError, RuntimeCueLease, RuntimeCueOrigin, RuntimeHandleOwnerSlot,
    RuntimeHandleResource, RuntimeLineHandleLedger,
};
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner};
use crate::runtime_id::{
    DialogueActivationId, ExecutionInstanceId, RuntimeDialogueContentPlanId,
    RuntimeLineHandleSiteId, RuntimePersistentFiberId,
};
use crate::task::RuntimeProgramOwner;
use crate::value::ownership::RuntimeOwnedSlotId;
use crate::value::{
    RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass,
    RuntimeRetainedValueAdmission,
};
use std::{num::NonZeroU32, sync::Arc};

#[test]
fn pending_flow_transfer_line_handle_has_one_real_slot_in_live_and_saved_storage() {
    let mut program = return_program();
    let kind = RuntimeHandleKind::Cue;
    let producer = kind.try_producer().unwrap();
    let semantic = RuntimeSemanticTypeId::from_bytes([0x65; 32]);
    let opaque_owner = RuntimeOpaqueTypeOwner::exact_with(
        producer.clone(),
        semantic,
        RuntimeOpaqueValueClass::AffineHandle(kind),
        RuntimeOpaquePersistence::SnapshotOnly,
    );
    let producer_id = AwbcStringId(u32::try_from(program.strings.len()).unwrap());
    program.strings.push(producer.as_str().to_owned());
    program.runtime_types = vec![AwbcRuntimeType::new(
        semantic,
        AwbcRuntimeTypeShape::Opaque {
            producer: producer_id,
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::AffineHandle(kind),
            persistence: RuntimeOpaquePersistence::SnapshotOnly,
            arguments: Vec::new(),
        },
    )];
    program.signatures[0].params = vec![AwbcTypeId(0)];
    program.functions[0].input_ownership = vec![AwbcFunctionInputOwnership::parameter(
        manual_awbc_formal_identity("pending-flow-transfer.cue-input"),
        0,
        crate::plan::RuntimeFunctionParameterPassing::Value,
    )];
    program.frame_layouts[0].slots = vec![AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Parameter,
        scope_depth: 0,
    }];
    program.blocks[0].terminator = AwbcTerminator::GotoStatic {
        function: AwbcFunctionId(0),
        args: vec![AwbcRegisterId(0)],
    };
    program.canonicalize_string_table();
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
    let execution = ExecutionInstanceId::from_allocated(fiber.instance.get());
    let old_slot = RuntimeOwnedSlotId::AwbcRegister {
        execution,
        fiber: fiber.instance,
        frame: fiber.active_frame().unwrap().instance,
        register: AwbcRegisterId(0),
    };
    let activation = DialogueActivationId::new(
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x66; 32]).unwrap(),
        RuntimePersistentFiberId::from_allocated(fiber.instance.get().get()),
        RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
        0,
    );
    let mut ledger = RuntimeLineHandleLedger::default();
    let value = ledger
        .issue_exact(
            &activation,
            RuntimeLineHandleSiteId::from_zero_based(0),
            kind,
            &opaque_owner,
            RuntimeHandleResource::Cue(RuntimeCueLease::new(RuntimeCueOrigin::StageLook)),
            RuntimeHandleOwnerSlot::parent_fiber(execution, old_slot),
        )
        .unwrap();
    let token = ledger.leases().keys().next().unwrap().clone();
    fiber
        .bind_function_argument_values_owned(&program, vec![RuntimeValue::Opaque(value)])
        .unwrap();
    assert_eq!(
        super::super::line::physical_fiber_line_handle_slots(execution, &fiber).unwrap(),
        BTreeMap::from([(token.clone(), old_slot)])
    );

    let prepared = fiber
        .prepare_root_replacement(
            &program,
            AwbcFunctionId(0),
            &[fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(0))
                .unwrap()],
        )
        .unwrap();
    let value = fiber
        .active_frame_mut()
        .unwrap()
        .take_register(AwbcRegisterId(0))
        .unwrap();
    let origin = FiberFlowTransferOrigin::Static {
        source: fiber.cursor,
    };
    fiber.stage_flow_transfer(prepared, vec![value], origin);
    let destination = &fiber.pending_flow_transfer.as_ref().unwrap().destination;
    let new_slot = RuntimeOwnedSlotId::AwbcRegister {
        execution,
        fiber: fiber.instance,
        frame: destination.instance,
        register: AwbcRegisterId(0),
    };
    let expected = BTreeMap::from([(token.clone(), new_slot)]);
    assert_eq!(
        super::super::line::physical_fiber_line_handle_slots(execution, &fiber).unwrap(),
        expected
    );
    let owner = RuntimeProgramOwner::Awbc(Arc::new(program));
    let image = AwbcFiberStateSnapshot::from_live(
        &fiber,
        &mut RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        ),
    )
    .unwrap();
    let image: AwbcFiberStateSnapshot =
        serde_json::from_slice(&serde_json::to_vec(&image).unwrap()).unwrap();
    assert_eq!(
        super::super::line::physical_fiber_line_handle_slots(execution, &image).unwrap(),
        expected
    );
    let mut duplicated = image.clone();
    duplicated.frames[0].registers[0] = duplicated
        .pending_flow_transfer
        .as_ref()
        .unwrap()
        .destination
        .registers[0]
        .as_ref()
        .unwrap()
        .clone()
        .into();
    assert!(matches!(
        super::super::line::physical_fiber_line_handle_slots(execution, &duplicated),
        Err(ProductStepError::Line(
            LineRuntimeError::DuplicateHandleOccurrence
        ))
    ));
    let restored = image.into_live_for_program(&owner).unwrap();
    assert_eq!(
        super::super::line::physical_fiber_line_handle_slots(execution, &restored).unwrap(),
        expected
    );
    ledger
        .transfer(
            &token,
            &RuntimeHandleOwnerSlot::parent_fiber(execution, old_slot),
            RuntimeHandleOwnerSlot::parent_fiber(execution, new_slot),
        )
        .unwrap();
    assert_eq!(
        ledger.lease(&token).unwrap().owner(),
        &RuntimeHandleOwnerSlot::parent_fiber(execution, new_slot)
    );
}
