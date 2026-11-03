use super::*;
use crate::awbc::product_step::checkpoint::ProductCheckpointTarget;
use crate::awbc::schema::*;
use crate::pattern::{RuntimeCheckedType, RuntimeOpaqueTypeOwner};
use crate::value::RuntimeOpaqueValue;

fn handle_program(nested: bool) -> (AwbcProgram, AwbcTypeId) {
    let mut program = return_program();
    let handle_owner = RuntimeOpaqueTypeOwner::presentation_handle(
        PresentationResourceKind::Image,
        RuntimeSemanticTypeId::from_bytes([0x9a; 32]),
    );
    let producer = AwbcStringId(program.strings.len().try_into().unwrap());
    program
        .strings
        .push(handle_owner.producer().as_str().into());
    let handle = AwbcTypeId(program.runtime_types.len().try_into().unwrap());
    program.runtime_types.push(AwbcRuntimeType::new(
        handle_owner.semantic_identity(),
        AwbcRuntimeTypeShape::Opaque {
            producer,
            admission: handle_owner.admission(),
            value_class: handle_owner.value_class(),
            persistence: handle_owner.persistence(),
            arguments: vec![],
        },
    ));
    program.runtime_types.push(AwbcRuntimeType::new(
        RuntimeCheckedType::Bool.semantic_identity_digest(),
        AwbcRuntimeTypeShape::Bool,
    ));
    program.signatures[0].params = vec![handle, handle];
    program.signatures[0].result = Some(handle);
    program.frame_layouts[0].slots = (0..2)
        .map(|_| AwbcFrameSlot {
            name: None,
            ty: handle,
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        })
        .collect();
    program.functions[0].input_ownership = (0..2)
        .map(|position| {
            AwbcFunctionInputOwnership::parameter(
                manual_awbc_formal_identity(&format!("presentation.transfer.input-{position}")),
                position,
                crate::plan::RuntimeFunctionParameterPassing::Affine,
            )
        })
        .collect();
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(0)),
    };
    if nested {
        let mut callee = program.functions[0].clone();
        callee.definition = crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
            *blake3::hash(b"presentation.transfer.callee.v1\0").as_bytes(),
        );
        callee.kind = AwbcFunctionKind::Ordinary;
        callee.public_id = None;
        callee.blocks = AwbcTableRange::new(2, 1);
        callee.entry_block = AwbcBlockId(2);
        program.functions.push(callee);
        program.functions[0].blocks = AwbcTableRange::new(0, 2);
        program.blocks[0].terminator = AwbcTerminator::CallFunction {
            function: AwbcFunctionId(1),
            args: vec![AwbcRegisterId(0), AwbcRegisterId(1)],
            dst: Some(AwbcRegisterId(0)),
            resume: AwbcResumePointId(0),
        };
        for owner in [AwbcFunctionId(0), AwbcFunctionId(1)] {
            program.blocks.push(AwbcBlock {
                owner,
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return {
                    value: Some(AwbcRegisterId(0)),
                },
                safe_point: if owner == AwbcFunctionId(1) {
                    AwbcSafePointKind::CallableBoundary
                } else {
                    AwbcSafePointKind::Return
                },
                source_map: None,
            });
        }
        program.resume_points.push(AwbcResumePoint {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(1),
            frame_layout: AwbcFrameLayoutId(0),
            kind: AwbcSafePointKind::CallableBoundary,
        });
    }
    program.canonicalize_string_table();
    (program, handle)
}

fn fixture(
    nested: bool,
    queued: bool,
) -> (AwbcProductStepExecutor, Vec<RuntimePresentationHandleToken>) {
    let (program, handle) = handle_program(nested);
    fixture_with_program(program, handle, queued)
}

fn fixture_with_program(
    program: AwbcProgram,
    handle: AwbcTypeId,
    queued: bool,
) -> (AwbcProductStepExecutor, Vec<RuntimePresentationHandleToken>) {
    let program = crate::awbc::product_step::AwbcProductProgram::admit(
        std::sync::Arc::new(program),
        Default::default(),
        Default::default(),
        Default::default(),
        None,
    )
    .unwrap();
    let artifact = AwbcProductStepExecutor::artifact_fingerprint(&program).unwrap();
    let fiber = FiberState::for_function(
        &program,
        crate::awbc::fiber::AwbcFiberRoot::Function(AwbcFunctionId(0)),
        AwbcFunctionId(0),
        0,
        64,
    )
    .unwrap();
    let mut executor =
        AwbcProductStepExecutor::for_fiber(program, fiber, artifact, GenerationId::new(0));
    executor.entry_bound = true;
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    executor.presentation = Some(PresentationBanks::new(
        owner.clone(),
        executor.artifact_fingerprint,
        if queued {
            PresentationLedgerLimits {
                handles: 2,
                scopes: 4,
            }
        } else {
            PresentationLedgerLimits::for_schema(crate::entry::RuntimeSchemaLimits::engine_default())
        },
    ));
    if queued {
        let ledger = executor
            .presentation
            .as_mut()
            .unwrap()
            .ledger_mut(RuntimePresentationBankId::Root)
            .unwrap();
        let scope = executor
            .fiber
            .bind_presentation_scope(&owner, ledger)
            .unwrap();
        for _ in 0..2 {
            ledger
                .register(
                    &owner,
                    scope,
                    PresentationRegistrationSite::Independent(PresentationResourceKind::Image),
                    PresentationBody {
                        expected: RuntimeCheckedType::Bool.semantic_identity_digest(),
                        binding: None,
                        value: RuntimePayload(RuntimeValue::Bool(false)),
                    },
                )
                .unwrap();
        }
        let retired = executor
            .fiber
            .release_presentation_scope(
                &owner,
                executor
                    .presentation
                    .as_mut()
                    .unwrap()
                    .ledger_mut(RuntimePresentationBankId::Root)
                    .unwrap(),
            )
            .unwrap();
        executor
            .presentation
            .as_mut()
            .unwrap()
            .retirements_mut()
            .extend(retired);
    }
    let ledger = executor
        .presentation
        .as_mut()
        .unwrap()
        .ledger_mut(RuntimePresentationBankId::Root)
        .unwrap();
    let scope = executor
        .fiber
        .bind_presentation_scope(&owner, ledger)
        .unwrap();
    let tokens: Vec<_> = [true, false]
        .into_iter()
        .map(|value| {
            ledger
                .register(
                    &owner,
                    scope,
                    PresentationRegistrationSite::Independent(PresentationResourceKind::Image),
                    PresentationBody {
                        expected: RuntimeCheckedType::Bool.semantic_identity_digest(),
                        binding: None,
                        value: RuntimePayload(RuntimeValue::Bool(value)),
                    },
                )
                .unwrap()
                .0
        })
        .collect();
    let opaque = executor.program.opaque_owner(handle).unwrap().unwrap();
    let values = tokens
        .iter()
        .map(|token| {
            RuntimeValue::Opaque(RuntimeOpaqueValue::new_exact(
                &opaque,
                token.encode_payload(),
            ))
        })
        .collect();
    executor
        .fiber
        .active_frame_mut()
        .unwrap()
        .bind_positional_arguments_owned(&executor.program, values)
        .unwrap();
    executor.snapshot().unwrap();
    (executor, tokens)
}

fn step(executor: &mut AwbcProductStepExecutor) {
    let output = executor.step(Default::default(), one_op());
    assert!(
        output.output.diagnostics.is_empty(),
        "{:?}",
        output.output.diagnostics
    );
}

fn one_op() -> RuntimeStepOptions {
    RuntimeStepOptions {
        budget: crate::step::RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    }
}

fn handle_method_program(
    method: arcweft_interaction_model::presentation::PresentationHandleMethodId,
    inspect: bool,
) -> (AwbcProgram, AwbcTypeId) {
    let (mut program, handle) = handle_program(false);
    let unit = AwbcTypeId(
        program
            .runtime_types
            .iter()
            .position(|ty| matches!(ty.shape(), AwbcRuntimeTypeShape::Unit))
            .unwrap()
            .try_into()
            .unwrap(),
    );
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: unit,
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    let place = AwbcMutablePlace::Local(AwbcRegisterId(0));
    let target = if inspect {
        AwbcValueTarget::Inspect(place)
    } else {
        AwbcValueTarget::Take(place)
    };
    program.instructions = vec![AwbcInstruction::PresentationHandleMethod {
        dst: AwbcRegisterId(2),
        target,
        method,
    }];
    program.blocks[0].instructions = AwbcTableRange::new(0, 1);
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(1)),
    };
    (program, handle)
}

#[test]
fn awbc_handle_retirement_capacity_rejects_before_moving_its_register_or_bank() {
    use arcweft_interaction_model::presentation::PresentationHandleMethodId;
    for method in [
        PresentationHandleMethodId::Unmount,
        PresentationHandleMethodId::Release,
        PresentationHandleMethodId::Destroy,
    ] {
        let (program, handle) = handle_method_program(method, false);
        let (mut executor, tokens) = fixture_with_program(program, handle, true);
        let before = executor.snapshot().unwrap();
        let context = crate::awbc::vm::VmExecutionContext::for_program(
            executor.artifact_fingerprint,
            executor.program.clone(),
        );
        let error = crate::awbc::vm::step_with_presentation(
            &executor.program,
            &mut executor.fiber,
            crate::awbc::vm::VmStepOptions {
                max_instructions: 1,
            },
            &context,
            &mut executor.presentation,
            &mut crate::awbc::vm::RejectingVmHost,
        )
        .unwrap_err();
        assert_eq!(
            error,
            crate::awbc::vm::VmError::Presentation(
                crate::presentation::RuntimePresentationError::RetirementCapacity
            )
        );
        assert_eq!(executor.snapshot().unwrap(), before);
        executor
            .presentation
            .as_mut()
            .unwrap()
            .retirements_mut()
            .clear();
        step(&mut executor);
        assert!(
            executor
                .fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(0))
                .is_err()
        );
        assert!(
            executor
                .fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(1))
                .is_ok()
        );
        assert_eq!(
            executor.presentation.as_ref().unwrap().retirements()[0].token,
            tokens[0]
        );
    }
}

#[test]
fn awbc_handle_visibility_changes_its_original_lease_without_consuming_the_register() {
    let (program, handle) = handle_method_program(
        arcweft_interaction_model::presentation::PresentationHandleMethodId::Hide,
        true,
    );
    let (mut executor, tokens) = fixture_with_program(program, handle, false);
    let before = executor
        .presentation
        .as_ref()
        .unwrap()
        .snapshot_admitted()
        .unwrap();
    step(&mut executor);
    assert_ne!(
        executor
            .presentation
            .as_ref()
            .unwrap()
            .snapshot_admitted()
            .unwrap(),
        before
    );
    let value = executor
        .fiber
        .active_frame()
        .unwrap()
        .register(AwbcRegisterId(0))
        .unwrap();
    let handles = value.view().affine_handles().unwrap();
    assert!(
        matches!(&handles[..], [crate::value::ownership::RuntimeAffineHandle::Presentation { token, .. }] if token == &tokens[0])
    );
    let image = executor.snapshot().unwrap();
    executor = executor
        .restore_snapshot_owned(image.clone())
        .unwrap_or_else(|(_, error)| panic!("{error}"));
    assert_eq!(executor.snapshot().unwrap(), image);
}

#[test]
fn root_return_retains_the_original_handle_and_drops_exited_storage_before_snapshot() {
    let (mut executor, tokens) = fixture(false, false);
    let before = executor.snapshot().unwrap();
    let checkpoint = executor
        .execution_checkpoint(ProductCheckpointTarget::Main)
        .unwrap();
    step(&mut executor);
    assert_eq!(executor.fiber.status, FiberStatus::Returned);
    assert!(
        executor.fiber.frames[0]
            .registers
            .iter()
            .all(|slot| slot.is_vacant())
    );
    assert!(executor.fiber.frames[0].root_presentation_scope.is_none());
    assert_eq!(
        executor.presentation.as_ref().unwrap().retirements()[0].token,
        tokens[1]
    );
    let Some(FiberTerminalValue::Returned(Some(value))) = &executor.fiber.terminal else {
        panic!("root owns result")
    };
    let handles = value.view().affine_handles().unwrap();
    assert!(
        matches!(&handles[..], [crate::value::ownership::RuntimeAffineHandle::Presentation { token, .. }] if token == &tokens[0])
    );
    executor.snapshot().unwrap();
    checkpoint
        .restore(
            &mut executor.fiber,
            &mut executor.presentation,
            &executor.program,
        )
        .unwrap();
    assert_eq!(executor.snapshot().unwrap(), before);
    step(&mut executor);
    let saved = AwbcProductExecutorSaveSnapshot::from_live(
        &executor.snapshot_for_save().unwrap(),
        executor.admitted_program(),
    )
    .unwrap();
    let bytes = serde_json::to_vec(&saved).unwrap();
    let saved = serde_json::from_slice(&bytes).unwrap();
    let before_restore = executor.snapshot().unwrap();
    let restored = executor.restore_inert_snapshot_owned(saved).unwrap();
    assert_eq!(restored.snapshot().unwrap(), before_restore);
}

#[test]
fn return_capacity_refusal_preserves_actual_handles_and_budget_then_retries() {
    let (mut executor, tokens) = fixture(false, true);
    let before = executor.snapshot().unwrap();
    for _ in 0..2 {
        let result = executor.step(Default::default(), one_op());
        assert!(!result.output.diagnostics.is_empty());
        assert_eq!(executor.snapshot().unwrap(), before);
    }
    executor
        .presentation
        .as_mut()
        .unwrap()
        .retirements_mut()
        .clear();
    step(&mut executor);
    assert_eq!(executor.fiber.status, FiberStatus::Returned);
    assert_eq!(
        executor.presentation.as_ref().unwrap().retirements()[0].token,
        tokens[1]
    );
    executor.snapshot().unwrap();
}

#[test]
fn nested_return_preserves_the_original_ancestor_grant_and_terminal_result() {
    let (mut executor, tokens) = fixture(true, false);
    step(&mut executor);
    assert_eq!(executor.fiber.frames.len(), 2);
    executor.snapshot().unwrap();
    step(&mut executor);
    assert_eq!(executor.fiber.frames.len(), 1);
    assert!(
        executor
            .presentation
            .as_ref()
            .unwrap()
            .retirements()
            .is_empty()
    );
    executor.snapshot().unwrap();
    step(&mut executor);
    assert_eq!(executor.fiber.status, FiberStatus::Returned);
    assert_eq!(
        executor.presentation.as_ref().unwrap().retirements()[0].token,
        tokens[1]
    );
    executor.snapshot().unwrap();
}

#[test]
fn callee_owned_handle_joins_the_actual_caller_scope_without_changing_its_token() {
    let (mut executor, tokens) = fixture(true, false);
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let source = executor
        .fiber
        .bind_presentation_scope(
            &owner,
            executor
                .presentation
                .as_mut()
                .unwrap()
                .ledger_mut(RuntimePresentationBankId::Root)
                .unwrap(),
        )
        .unwrap();
    step(&mut executor); // The actual CallFunction allocated this callee.
    let banks = executor.presentation.as_mut().unwrap();
    let ledger = banks.ledger_mut(RuntimePresentationBankId::Root).unwrap();
    let callee = executor
        .fiber
        .bind_presentation_scope(&owner, ledger)
        .unwrap();
    for token in &tokens {
        ledger
            .transfer(&owner, token, Some(source), Some(callee))
            .unwrap();
    }
    let instance = executor.fiber.instance;
    banks
        .retire_bound_scope(
            &owner,
            crate::runtime_id::ExecutionInstanceId::from_allocated(instance.get()),
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(instance.get().get()),
            executor.fiber.frames[0].presentation_scope_target(),
        )
        .unwrap();
    assert!(executor.fiber.frames[0].root_presentation_scope.is_none());
    executor.snapshot().unwrap();
    step(&mut executor);
    assert_eq!(executor.fiber.frames.len(), 1);
    assert!(executor.fiber.frames[0].root_presentation_scope.is_some());
    assert_eq!(
        executor.presentation.as_ref().unwrap().retirements()[0].token,
        tokens[1]
    );
    executor.snapshot().unwrap();
    step(&mut executor);
    let Some(FiberTerminalValue::Returned(Some(value))) = &executor.fiber.terminal else {
        panic!("root result")
    };
    assert!(matches!(&value.view().affine_handles().unwrap()[..],
        [crate::value::ownership::RuntimeAffineHandle::Presentation { token, .. }] if token == &tokens[0]));
    executor.snapshot().unwrap();
}

fn goto_fixture(
    dynamic: bool,
    queued: bool,
) -> (AwbcProductStepExecutor, Vec<RuntimePresentationHandleToken>) {
    let (mut program, handle) = handle_program(false);
    program.signatures.push(AwbcSignature {
        params: vec![handle],
        result: Some(handle),
        effects: AwbcEffectSetId(0),
    });
    program.frame_layouts.push(AwbcFrameLayout {
        scopes: vec![],
        max_scope_depth: 0,
        slots: vec![AwbcFrameSlot {
            name: None,
            ty: handle,
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        }],
    });
    let mut target = program.functions[0].clone();
    target.definition = crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
        *blake3::hash(b"presentation.transfer.goto-target.v1\0").as_bytes(),
    );
    target.public_id = None;
    target.signature = AwbcSignatureId(1);
    target.frame_layout = AwbcFrameLayoutId(1);
    target.input_ownership.truncate(1);
    target.blocks = AwbcTableRange::new(1, 1);
    target.entry_block = AwbcBlockId(1);
    program.functions.push(target);
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(0)),
        },
        safe_point: AwbcSafePointKind::FlowEntry,
        source_map: None,
    });
    let flow =
        crate::plan::FlowRuntimeId::from_checked_declaration_digest([0x9b; 32], "flow.after")
            .unwrap();
    program.flow_bindings.push(AwbcFlowBinding {
        flow: flow.clone(),
        function: AwbcFunctionId(1),
    });
    program.flow_executables.push(AwbcFlowExecutable {
        metadata: RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([0x9c; 32]),
            controller: None,
        },
        function: AwbcFunctionId(1),
    });
    let entry_label = AwbcStringId(program.strings.len().try_into().unwrap());
    program.strings.push("entry.after".into());
    let mut entry = program.entries[0].clone();
    entry.runtime_id = crate::plan::EntryRuntimeId::canonical("after").unwrap();
    entry.binding = crate::entry::EntryBindingIdentity::from_bytes([2; 32]);
    entry.public_id = entry_label;
    entry.target = AwbcEntryTarget::Function {
        function: AwbcFunctionId(1),
    };
    program.entries.push(entry);
    program.blocks[0].terminator = if dynamic {
        program.functions[0].flags = program.functions[0]
            .flags
            .with(AwbcFunctionFlag::HasDynamicTarget);
        let target_type = AwbcTypeId(program.runtime_types.len().try_into().unwrap());
        program.runtime_types.push(AwbcRuntimeType::new(
            RuntimeCheckedType::EntityReference(
                crate::value::RuntimeEntityReferenceType::family_only(
                    arcweft_id::ProjectEntityReferenceFamily::Flow,
                ),
            )
            .semantic_identity_digest(),
            AwbcRuntimeTypeShape::EntityRef(crate::value::RuntimeEntityReferenceType::family_only(
                arcweft_id::ProjectEntityReferenceFamily::Flow,
            )),
        ));
        program.frame_layouts[0].slots.push(AwbcFrameSlot {
            name: None,
            ty: target_type,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        });
        program.constants.push(AwbcConstant::EntityRef(
            crate::value::RuntimeEntityReference::StructuralFlow(flow),
        ));
        program.instructions.push(AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(0),
        });
        program.blocks[0].instructions = AwbcTableRange::new(0, 1);
        AwbcTerminator::GotoDynamic {
            target: AwbcRegisterId(2),
            args: vec![AwbcRegisterId(0)],
        }
    } else {
        AwbcTerminator::GotoStatic {
            function: AwbcFunctionId(1),
            args: vec![AwbcRegisterId(0)],
        }
    };
    program.canonicalize_string_table();
    let (mut executor, tokens) = fixture_with_program(program, handle, queued);
    if dynamic {
        step(&mut executor);
    }
    (executor, tokens)
}

#[test]
fn static_and_dynamic_goto_move_the_original_handle_into_the_actual_new_root() {
    for dynamic in [false, true] {
        let (mut executor, tokens) = goto_fixture(dynamic, false);
        let before = executor.snapshot().unwrap();
        let old_frame = executor.fiber.frames[0].instance;
        let checkpoint = executor
            .execution_checkpoint(ProductCheckpointTarget::Main)
            .unwrap();
        step(&mut executor);
        assert_eq!(executor.fiber.cursor.function, AwbcFunctionId(1));
        assert!(executor.fiber.frames[0].instance > old_frame);
        assert!(executor.fiber.frames[0].root_presentation_scope.is_some());
        assert_eq!(
            executor.presentation.as_ref().unwrap().retirements()[0].token,
            tokens[1]
        );
        executor.snapshot().unwrap();
        step(&mut executor);
        assert_eq!(executor.fiber.status, FiberStatus::Returned);
        assert!(
            matches!(&executor.fiber.terminal, Some(FiberTerminalValue::Returned(Some(value)))
            if matches!(&value.view().affine_handles().unwrap()[..],
                [crate::value::ownership::RuntimeAffineHandle::Presentation { token, .. }] if token == &tokens[0]))
        );
        executor.snapshot().unwrap();
        checkpoint
            .restore(
                &mut executor.fiber,
                &mut executor.presentation,
                &executor.program,
            )
            .unwrap();
        assert_eq!(executor.snapshot().unwrap(), before);
    }
}

#[test]
fn goto_capacity_refusal_keeps_arguments_target_frame_cursor_and_budget_for_retry() {
    for dynamic in [false, true] {
        let (mut executor, tokens) = goto_fixture(dynamic, true);
        let before = executor.snapshot().unwrap();
        for _ in 0..2 {
            let output = executor.step(Default::default(), one_op());
            assert!(!output.output.diagnostics.is_empty());
            assert_eq!(executor.snapshot().unwrap(), before);
        }
        executor
            .presentation
            .as_mut()
            .unwrap()
            .retirements_mut()
            .clear();
        step(&mut executor);
        assert_eq!(executor.fiber.cursor.function, AwbcFunctionId(1));
        assert_eq!(
            executor.presentation.as_ref().unwrap().retirements()[0].token,
            tokens[1]
        );
        executor.snapshot().unwrap();
    }
}
