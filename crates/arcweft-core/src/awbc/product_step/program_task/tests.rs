use super::*;
use crate::awbc::schema::*;
use crate::plan::FlowOp;
use std::sync::Arc;

#[test]
fn ordinary_program_need_starts_from_its_source_and_survives_each_public_save() {
    use crate::task::{RuntimeNeedHandle, TaskHandleType, TaskRequest};
    use crate::value::RuntimeValue;

    let mut code = crate::plan::task_source::program_need_executable_fixture();
    code.runtime_types.push(AwbcRuntimeType::new(
        crate::pattern::RuntimeSemanticTypeId::from_bytes([20; 32]),
        AwbcRuntimeTypeShape::Need(AwbcTypeId(0)),
    ));
    code.signatures.push(AwbcSignature {
        params: vec![],
        result: Some(AwbcTypeId(0)),
        effects: AwbcEffectSetId(0),
    });
    let mut caller = code.functions[0].clone();
    caller.definition =
        crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([1; 32]);
    caller.kind = AwbcFunctionKind::Flow;
    caller.semantic_role = crate::plan::RuntimeFunctionSemanticRole::Flow;
    caller.public_id = Some(AwbcStringId(2));
    caller.signature = AwbcSignatureId(1);
    caller.input_ownership.clear();
    caller.frame_layout = AwbcFrameLayoutId(1);
    caller.blocks = AwbcTableRange::new(1, 1);
    caller.entry_block = AwbcBlockId(1);
    code.functions.push(caller);
    code.frame_layouts.push(AwbcFrameLayout {
        slots: [AwbcTypeId(1), AwbcTypeId(2), AwbcTypeId(0)]
            .into_iter()
            .map(|ty| AwbcFrameSlot {
                name: None,
                ty,
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            })
            .collect(),
        scopes: vec![],
        max_scope_depth: 0,
    });
    code.constants.push(AwbcConstant::Bool(true));
    code.instructions.extend([
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::StartProgram {
            dst: AwbcRegisterId(1),
            plan: AwbcTaskPlanId(0),
            args: vec![AwbcRegisterId(0)],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(0),
        },
    ]);
    code.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(1, 3),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(2)),
        },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    code.strings.push("entry.program_need".into());
    let flow =
        crate::plan::FlowRuntimeId::from_checked_declaration_digest([0x93; 32], "program.need")
            .unwrap();
    code.flow_bindings.push(AwbcFlowBinding {
        flow: flow.clone(),
        function: AwbcFunctionId(1),
    });
    code.flow_executables.push(AwbcFlowExecutable {
        metadata: crate::plan::RuntimeFlowExecutable {
            flow,
            contract: crate::plan::FlowContractHash::from_bytes([0x94; 32]),
            controller: None,
        },
        function: AwbcFunctionId(1),
    });
    code.entries.push(AwbcEntry {
        runtime_id: crate::plan::EntryRuntimeId::canonical("program_need").unwrap(),
        binding: crate::entry::EntryBindingIdentity::from_bytes([0x92; 32]),
        public_id: AwbcStringId(2),
        kind: AwbcEntryKind::Cli,
        target: AwbcEntryTarget::Function {
            function: AwbcFunctionId(1),
        },
        roles: crate::entry::RuntimeEntryRoles::None,
    });
    code.canonicalize_string_table();
    let bytes = code.encode_canonical().unwrap();
    let decoded =
        AwbcProgram::decode_canonical(&bytes, crate::awbc::codec::AwbcDecodeBudget::default())
            .unwrap();
    let mut executor = AwbcProductStepExecutor::for_entry(decoded, AwbcEntryId(0), 1).unwrap();
    let mut submissions = vec![];
    let mut completions = vec![];
    for _ in 0..256 {
        let step = executor.step(Default::default(), Default::default());
        assert!(step.stats.executed_ops <= 1);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        assert!(step.output.requests.host_calls.is_empty());
        submissions.extend(step.output.requests.tasks);
        completions.extend(step.output.requests.program_task_completions);
        let snapshot = executor.snapshot_for_save().unwrap();
        let save = super::super::AwbcProductExecutorSaveSnapshot::from_live(
            &snapshot,
            executor.admitted_program(),
        )
        .unwrap();
        let encoded = serde_json::to_vec(&save).unwrap();
        let decoded: super::super::AwbcProductExecutorSaveSnapshot =
            serde_json::from_slice(&encoded).unwrap();
        let restored = decoded
            .into_live_for_program(executor.admitted_program())
            .unwrap();
        executor = executor.restore_snapshot_owned(restored).unwrap();
        if !completions.is_empty() && executor.child_fibers.is_empty() {
            break;
        }
    }
    assert_eq!(submissions.len(), 1);
    assert_eq!(completions.len(), 1);
    let task = submissions.pop().unwrap();
    let handle = RuntimeNeedHandle::try_from(task.clone()).unwrap();
    assert!(matches!(handle.spec().request, TaskRequest::Program(_)));
    assert!(matches!(
        handle.spec().producer.handle_type(),
        TaskHandleType::Need { .. }
    ));
    assert_eq!(
        executor
            .need_producers
            .take_ready_for_correlation(&task.handle().correlation)
            .unwrap()
            .into_value(),
        RuntimeValue::Unit
    );
    assert!(
        executor
            .need_producers
            .take_ready_for_correlation(&task.handle().correlation)
            .is_err()
    );
}

#[test]
fn program_handle_private_fiber_checkpoint_restores_its_original_source() {
    use crate::awbc::fiber::{
        AwbcFiberRoot, AwbcFiberStateSnapshot, FiberState, validate_program_task_argument_refs,
    };
    use crate::task::{
        GenerationId, NeedProducerInstance, NeedProducerRegistry, RuntimeNeedHandle,
        RuntimeTaskTemplateArguments, TaskRequest, TaskSpec,
    };
    use crate::value::{RuntimeRetainedValueAdmission, RuntimeValue};

    let mut code = crate::plan::task_source::program_task_executable_fixture();
    code.runtime_types.push(AwbcRuntimeType::new(
        crate::pattern::RuntimeSemanticTypeId::from_bytes([0x47; 32]),
        AwbcRuntimeTypeShape::Task(AwbcTypeId(0)),
    ));
    code.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(2),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    let admitted = super::super::AwbcProductProgram::admit(
        Arc::new(code),
        crate::awbc::verify::AwbcVerifyBudget::default(),
        crate::awbc::verify::AwbcVerifyContext {
            require_entrypoint: false,
            ..Default::default()
        },
        crate::plan::RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap();
    let program = admitted.task_source.as_ref().unwrap();
    let owner = crate::task::RuntimeProgramOwner::Awbc(admitted.program_arc());
    let crate::task::RuntimeProgramOwner::Awbc(code) = &owner else {
        unreachable!()
    };
    let source = program
        .awbc_program_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut crate::task::semantic::TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
    let capture = RuntimeValue::Bool(true);
    let mut bank = RuntimeRetainedValueAdmission::for_program(
        owner.clone(),
        crate::entry::RuntimeSchemaLimits::engine_default(),
    );
    let checked = source
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::ProgramTask {
                captures: &[&capture],
            },
            None,
            &mut bank,
        )
        .unwrap();
    let Some(FlowOp::ProgramTask { producer, .. }) = source.request_endpoint().operation() else {
        unreachable!()
    };
    let spec = TaskSpec {
        generation: GenerationId::new(0),
        producer: NeedProducerInstance::try_from(&checked).unwrap(),
        class: producer.class.clone(),
        priority: producer.priority,
        cancel_scope: producer.cancel_scope.clone(),
        policy: producer.policy,
        outcome: producer.outcome(),
        request: TaskRequest::Program(checked.source().clone()),
        debug_label: producer.debug_label.clone(),
    };
    let mut registry = NeedProducerRegistry::default();
    let (_, _, handle) = registry
        .ensure_program_task_with_binding(spec, |receipt| {
            Ok::<_, Box<dyn std::error::Error>>(RuntimeValue::NeedHandle(
                RuntimeNeedHandle::try_from(receipt.clone())?,
            ))
        })
        .unwrap();
    let inputs = validate_program_task_argument_refs(
        code,
        AwbcTaskPlanId(0),
        &[&capture],
        None,
        &bank.work(),
    )
    .unwrap();
    let mut fiber = FiberState::for_function_with_arguments_prepared(
        code,
        AwbcFiberRoot::Task(AwbcTaskPlanId(0)),
        vec![capture],
        inputs,
        crate::runtime_id::RuntimeFiberInstanceId::from_allocated(std::num::NonZeroU64::MIN),
        0,
        64,
    );
    fiber
        .active_frame_mut()
        .unwrap()
        .set_register(AwbcRegisterId(2), handle)
        .unwrap();
    let before = AwbcFiberStateSnapshot::from_live(
        &fiber,
        &mut crate::value::RuntimeRetainedValueAdmission::for_program(
            (owner).clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        ),
    )
    .unwrap();
    let checkpoint = fiber
        .checkpoint(
            &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                (owner).clone(),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            ),
        )
        .unwrap();
    let foreign = super::super::AwbcProductProgram::admit(
        Arc::new((**code).clone()),
        crate::awbc::verify::AwbcVerifyBudget::default(),
        crate::awbc::verify::AwbcVerifyContext {
            require_entrypoint: false,
            ..Default::default()
        },
        crate::plan::RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap();
    assert!(matches!(
        fiber.restore(checkpoint.clone(), &foreign),
        Err(crate::awbc::fiber::FiberStateError::CheckpointProgramOwner)
    ));
    assert_eq!(
        AwbcFiberStateSnapshot::from_live(
            &fiber,
            &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                (owner).clone(),
                crate::entry::RuntimeSchemaLimits::engine_default()
            )
        )
        .unwrap(),
        before
    );
    assert!(matches!(
        fiber
            .active_frame_mut()
            .unwrap()
            .take_register(AwbcRegisterId(2))
            .unwrap(),
        RuntimeValue::NeedHandle(_)
    ));
    fiber
        .restore(checkpoint, &admitted)
        .expect("private rollback retains the actual original Program Source");
    assert_eq!(
        AwbcFiberStateSnapshot::from_live(
            &fiber,
            &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                (owner).clone(),
                crate::entry::RuntimeSchemaLimits::engine_default()
            )
        )
        .unwrap(),
        before
    );
}
