use super::*;
use crate::awbc::schema::*;
use crate::plan::FlowOp;
use std::sync::Arc;

fn program_need_entry_code() -> AwbcProgram {
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
    let flow = crate::plan::FlowRuntimeId::from_checked_declaration_digest(
        [0x93; 32],
        "flow.program_need",
    )
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
    code
}

#[test]
fn ordinary_program_need_starts_from_its_source_and_survives_each_public_save() {
    use crate::task::{RuntimeNeedHandle, TaskHandleType, TaskRequest};
    use crate::value::RuntimeValue;

    let code = program_need_entry_code();
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

fn owned_program_entry_code(
    mode: crate::value::RuntimeLocalReadMode,
    delivery: crate::task::ProgramTaskDelivery,
) -> AwbcProgram {
    use crate::plan::{RuntimeGeneratedFunctionRole, RuntimeTaskSourceReadContext};
    let (original, flow, coordinates) =
        crate::plan::task_source::owned_program_need_fixture(mode, delivery);
    let body = original
        .task_program_body(
            &coordinates[1],
            &mut RuntimeTaskSourceReadContext::new(Default::default()),
        )
        .unwrap();
    let producer = body.producer();
    let capture = original
        .local_declarations()
        .get(body.captures()[0].local)
        .unwrap();
    let string = crate::pattern::RuntimeCheckedType::String.semantic_identity_digest();
    let input_type = original
        .type_table()
        .get(capture.ty())
        .unwrap()
        .semantic_identity();
    let mut code = program_need_entry_code();
    code.original_source = Some(original.task_source_artifact(Default::default()).unwrap());
    code.runtime_types[1] = AwbcRuntimeType::new(string, AwbcRuntimeTypeShape::String);
    code.runtime_types[2] =
        AwbcRuntimeType::new(input_type, AwbcRuntimeTypeShape::Need(AwbcTypeId(1)));
    code.runtime_types.push(AwbcRuntimeType::new(
        crate::pattern::RuntimeSemanticTypeId::from_bytes([0xa4; 32]),
        AwbcRuntimeTypeShape::Need(AwbcTypeId(2)),
    ));
    code.functions[0].definition = producer
        .function
        .generated_child(RuntimeGeneratedFunctionRole::TaskBody);
    code.functions[0].input_ownership[0] = AwbcFunctionInputOwnership::owned(
        capture.origin(),
        crate::plan::RuntimeFunctionInputSource::Capture { position: 0 },
        crate::plan::RuntimeFunctionInputTransfer::ExternalBinding,
    );
    code.functions[0].blocks = AwbcTableRange::new(0, 1);
    code.signatures[0].params = vec![AwbcTypeId(2)];
    code.signatures[0].result = Some(AwbcTypeId(2));
    for slot in &mut code.frame_layouts[0].slots {
        slot.ty = AwbcTypeId(2);
    }
    code.blocks[0].instructions = AwbcTableRange::new(0, 0);
    code.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(0)),
    };
    code.functions[1].definition =
        crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xa9; 32]);
    code.frame_layouts[1].slots[0].ty = AwbcTypeId(2);
    code.frame_layouts[1].slots[1].ty = AwbcTypeId(3);
    code.blocks[1].instructions = AwbcTableRange::new(1, 3);
    code.flow_bindings[0].flow = flow.clone();
    code.flow_executables[0].metadata.flow = flow;
    let second = AwbcTaskPlan {
        class: AwbcTaskClass::Cpu,
        priority: producer.priority.0,
        cancel_scope: code.task_plans[0].cancel_scope,
        policy: AwbcTaskPolicy::AlwaysStart,
        handle_type: crate::task::TaskHandleType::Need {
            payload: AwbcTypeId(2),
        },
        execution: AwbcTaskPlanExecution::Program(AwbcProgramTaskPlan {
            body: crate::awbc::schema::AwbcProgramTaskKernel::Flow,
            delivery,
            source: body.task_plan().key().into(),
            kernel: AwbcFunctionId(0),
            capture_modes: vec![mode],
        }),
    };
    let first_source = original
        .task_program_body(
            &coordinates[0],
            &mut RuntimeTaskSourceReadContext::new(Default::default()),
        )
        .unwrap();
    let first_row = first_source.task_plan();
    let mut first_kernel = code.functions[0].clone();
    first_kernel.definition = first_source
        .producer()
        .function
        .generated_child(RuntimeGeneratedFunctionRole::TaskBody);
    first_kernel.signature = AwbcSignatureId(2);
    first_kernel.frame_layout = AwbcFrameLayoutId(2);
    first_kernel.input_ownership.clear();
    first_kernel.blocks = AwbcTableRange::new(2, 1);
    first_kernel.entry_block = AwbcBlockId(2);
    code.functions.push(first_kernel);
    code.signatures.push(AwbcSignature {
        params: vec![],
        result: Some(AwbcTypeId(1)),
        effects: AwbcEffectSetId(0),
    });
    let mut first_frame = code.frame_layouts[0].clone();
    first_frame.slots.truncate(1);
    first_frame.slots[0].ty = AwbcTypeId(1);
    first_frame.slots[0].role = AwbcFrameSlotRole::Temporary;
    code.frame_layouts.push(first_frame);
    code.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(2),
        instructions: AwbcTableRange::new(0, 1),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(0)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    code.strings.push("owned result".into());
    code.constants = vec![
        AwbcConstant::Unit,
        AwbcConstant::String(AwbcStringId(u32::try_from(code.strings.len() - 1).unwrap())),
    ];
    code.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::StartProgram {
            dst: AwbcRegisterId(0),
            plan: AwbcTaskPlanId(0),
            args: vec![],
        },
        AwbcInstruction::StartProgram {
            dst: AwbcRegisterId(1),
            plan: AwbcTaskPlanId(1),
            args: vec![AwbcRegisterId(0)],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(0),
        },
    ];
    let mut first = second.clone();
    first.handle_type = crate::task::TaskHandleType::Need {
        payload: AwbcTypeId(1),
    };
    let AwbcTaskPlanExecution::Program(first_body) = &mut first.execution else {
        unreachable!()
    };
    first_body.source = first_row.key().into();
    first_body.kernel = AwbcFunctionId(2);
    first_body.capture_modes.clear();
    code.task_plans = vec![first, second];
    // Source admission also checks the distinct scope strings for both rows.
    for (row, scope) in [(0, "owned.program.input"), (1, "owned.program.capture")] {
        code.strings.push(scope.into());
        code.task_plans[row].cancel_scope =
            AwbcStringId(u32::try_from(code.strings.len() - 1).unwrap());
    }
    code.canonicalize_string_table();
    code
}

#[test]
fn program_need_copy_capture_rejects_affine_input_before_product_launch() {
    let code = owned_program_entry_code(
        crate::value::RuntimeLocalReadMode::Copy,
        crate::task::ProgramTaskDelivery::HostMetadata,
    );
    let result = AwbcProductStepExecutor::for_entry(code, AwbcEntryId(0), 1);
    assert!(
        matches!(result, Err(super::super::AwbcProductStepBuildError::InvalidProgram { message }) if message.contains("original Program capture with unrestricted Copy custody"))
    );
}

#[test]
fn program_move_verifier_allows_destination_alias_and_rejects_duplicate_owners() {
    let mut code = owned_program_entry_code(
        crate::value::RuntimeLocalReadMode::Move,
        crate::task::ProgramTaskDelivery::HostMetadata,
    );
    // This unit tests the register transition. The complete original Source
    // and execution relation is exercised by the public-save test below.
    code.signatures[0].result = Some(AwbcTypeId(1));
    code.frame_layouts[0].slots[1].ty = AwbcTypeId(1);
    code.blocks[0].instructions = AwbcTableRange::new(4, 1);
    code.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(1)),
    };
    code.instructions.push(AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(1),
        constant: AwbcConstantId(1),
    });
    code.task_plans[1].handle_type = crate::task::TaskHandleType::Need {
        payload: AwbcTypeId(1),
    };
    let AwbcInstruction::StartProgram { dst, .. } = &mut code.instructions[2] else {
        unreachable!()
    };
    *dst = AwbcRegisterId(0);
    code.verify(Default::default(), Default::default()).unwrap();

    code.signatures[0].params.push(AwbcTypeId(2));
    let mut parameter = code.frame_layouts[0].slots[0].clone();
    parameter.ty = AwbcTypeId(2);
    code.frame_layouts[0].slots.insert(1, parameter);
    let AwbcInstruction::LoadConst { dst, .. } = &mut code.instructions[4] else {
        unreachable!()
    };
    *dst = AwbcRegisterId(2);
    code.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(2)),
    };
    code.functions[0]
        .input_ownership
        .push(AwbcFunctionInputOwnership::owned(
            crate::plan::RuntimeLocalOrigin::Binding([0xb0; 32]),
            crate::plan::RuntimeFunctionInputSource::Capture { position: 1 },
            crate::plan::RuntimeFunctionInputTransfer::ExternalBinding,
        ));
    let AwbcTaskPlanExecution::Program(body) = &mut code.task_plans[1].execution else {
        unreachable!()
    };
    body.capture_modes
        .push(crate::value::RuntimeLocalReadMode::Move);
    let AwbcInstruction::StartProgram { args, .. } = &mut code.instructions[2] else {
        unreachable!()
    };
    args.push(AwbcRegisterId(0));
    let result = code.verify(Default::default(), Default::default());
    assert!(
        matches!(&result, Err(crate::awbc::verify::AwbcVerifyError::InvalidInvariant { message, .. }) if message == "by-value operand registers are unique"),
        "{result:?}"
    );
}

#[test]
fn program_need_moves_its_original_affine_input_through_every_public_save() {
    exercise_owned_program_capture_delivery(crate::task::ProgramTaskDelivery::HostMetadata);
}

#[test]
fn local_program_need_preserves_owned_results_through_each_public_save_without_host_delivery() {
    exercise_owned_program_capture_delivery(crate::task::ProgramTaskDelivery::LocalOnly);
}

fn exercise_owned_program_capture_delivery(delivery: crate::task::ProgramTaskDelivery) {
    let code = owned_program_entry_code(crate::value::RuntimeLocalReadMode::Move, delivery);
    let keys = [0, 1].map(|index| {
        let AwbcTaskPlanExecution::Program(body) = &code.task_plans[index].execution else {
            unreachable!()
        };
        body.source
    });
    let bytes = code.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, Default::default()).unwrap();
    let mut executor = AwbcProductStepExecutor::for_entry(decoded, AwbcEntryId(0), 1).unwrap();
    let mut submissions = vec![];
    let mut completions = vec![];
    let mut moved_parent_observed = false;
    let mut rejected_wrong_delivery_phase = false;
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
        if executor.need_producers.task_admissions().count() == 2
            && let Ok(frame) = executor.fiber.active_frame()
        {
            assert!(frame.register(AwbcRegisterId(0)).is_err());
            moved_parent_observed = true;
        }
        let snapshot = executor.snapshot_for_save().unwrap();
        let save = super::super::AwbcProductExecutorSaveSnapshot::from_live(
            &snapshot,
            executor.admitted_program(),
        )
        .unwrap();
        let encoded = serde_json::to_vec(&save).unwrap();
        if !rejected_wrong_delivery_phase {
            let mut invalid = serde_json::to_value(&save).unwrap();
            for launch in invalid["need_producers"]["launches"]
                .as_array_mut()
                .unwrap()
            {
                let Some(program) = launch["origin"].get_mut("Program") else {
                    continue;
                };
                if program["retirement"]
                    .as_str()
                    .is_some_and(|phase| phase != "Executing")
                {
                    program["retirement"] = serde_json::json!(match delivery {
                        crate::task::ProgramTaskDelivery::HostMetadata => "Local",
                        crate::task::ProgramTaskDelivery::LocalOnly => "Unacknowledged",
                    });
                    rejected_wrong_delivery_phase = true;
                    break;
                }
            }
            if rejected_wrong_delivery_phase {
                let invalid: super::super::AwbcProductExecutorSaveSnapshot =
                    serde_json::from_value(invalid).unwrap();
                assert!(
                    invalid
                        .into_live_for_program(executor.admitted_program())
                        .is_err()
                );
            }
        }
        let decoded: super::super::AwbcProductExecutorSaveSnapshot =
            serde_json::from_slice(&encoded).unwrap();
        let restored = decoded
            .into_live_for_program(executor.admitted_program())
            .unwrap();
        executor = executor.restore_snapshot_owned(restored).unwrap();
        let complete = match delivery {
            crate::task::ProgramTaskDelivery::HostMetadata => {
                completions.len() == 2 && executor.child_fibers.is_empty()
            }
            crate::task::ProgramTaskDelivery::LocalOnly => {
                executor.fiber.status == crate::awbc::fiber::FiberStatus::Returned
                    && executor.child_fibers.is_empty()
                    && executor.need_producers.task_admissions().count() == 2
            }
        };
        if complete {
            break;
        }
    }
    let expected_metadata = match delivery {
        crate::task::ProgramTaskDelivery::HostMetadata => 2,
        crate::task::ProgramTaskDelivery::LocalOnly => 0,
    };
    assert_eq!(submissions.len(), expected_metadata);
    assert_eq!(completions.len(), expected_metadata);
    assert!(moved_parent_observed);
    assert!(rejected_wrong_delivery_phase);
    if delivery == crate::task::ProgramTaskDelivery::LocalOnly {
        assert!(
            executor
                .need_producers
                .take_program_retirement_replays()
                .is_empty()
        );
    }
    let admissions = executor
        .need_producers
        .task_admissions()
        .collect::<Vec<_>>();
    assert_eq!(admissions.len(), 2);
    let submissions = keys.map(|key| {
        admissions.iter().find(|task| {
            matches!(&task.spec().request, crate::task::TaskRequest::Program(source) if source.task_plan().key().as_bytes() == key.bytes())
        }).unwrap()
    });
    let result = executor
        .need_producers
        .take_ready_for_correlation(&submissions[1].handle().correlation)
        .unwrap()
        .into_value();
    assert!(
        matches!(result, RuntimeValue::NeedHandle(handle) if handle.correlation() == submissions[0].handle().correlation)
    );
    assert!(
        executor
            .need_producers
            .take_ready_for_correlation(&submissions[1].handle().correlation)
            .is_err()
    );
    assert_eq!(
        executor
            .need_producers
            .take_ready_for_correlation(&submissions[0].handle().correlation)
            .unwrap()
            .into_value(),
        RuntimeValue::String("owned result".into())
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
    let Some(FlowOp::ProgramTask {
        template: crate::plan::RuntimeProgramTaskTemplate { producer, .. },
        ..
    }) = source.request_endpoint().operation()
    else {
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
