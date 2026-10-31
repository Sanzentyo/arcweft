use super::*;
use crate::awbc::fiber::{AwbcFiberRoot, FiberCreationContext, FiberTerminalValue};
use crate::awbc::schema::*;
use crate::presentation::RuntimePresentationError;
use crate::step::RuntimeStepOutput;

fn roundtrip(executor: AwbcProductStepExecutor) -> AwbcProductStepExecutor {
    let snapshot = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&snapshot, &executor.program).unwrap();
    let wire = serde_json::to_vec(&save).unwrap();
    let restored = executor
        .restore_inert_snapshot_owned(serde_json::from_slice(&wire).unwrap())
        .unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    restored
}

#[test]
fn lifecycle_trap_retires_the_original_nested_contour_once_and_public_save_restores_it() {
    let (mut executor, tokens) = super::traps::nested_fixture(false);
    let mut output = RuntimeStepOutput::default();
    executor.fail_with_trap(
        AwbcTrapCode::HostAbiMismatch,
        "outside VM".into(),
        None,
        &mut output,
    );
    assert_eq!(executor.fiber.status, FiberStatus::Trapped);
    let Some(FiberTerminalValue::Trapped(trap)) = &executor.fiber.terminal else {
        unreachable!()
    };
    assert_eq!(trap.code, AwbcTrapCode::HostAbiMismatch);
    assert_eq!(trap.message.as_deref(), Some("outside VM"));
    assert_eq!(
        executor
            .presentation
            .as_ref()
            .unwrap()
            .retirements()
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        tokens
    );
    let after = executor.snapshot().unwrap();
    executor.fail_with_trap(
        AwbcTrapCode::InternalInvariant,
        "duplicate".into(),
        None,
        &mut output,
    );
    assert_eq!(executor.snapshot().unwrap(), after);
    roundtrip(executor);
}

#[test]
fn lifecycle_trap_refusal_preserves_all_grants_and_retries_without_a_fresh_owner() {
    let (mut executor, tokens) = super::traps::nested_fixture(true);
    let before = executor.snapshot().unwrap();
    for _ in 0..2 {
        let mut output = RuntimeStepOutput::default();
        executor.fail_with_trap(
            AwbcTrapCode::HostAbiMismatch,
            "outside VM".into(),
            None,
            &mut output,
        );
        assert_eq!(output.diagnostics.len(), 1);
        assert!(
            output.diagnostics[0]
                .message
                .contains(&RuntimePresentationError::RetirementCapacity.to_string())
        );
        assert!(output.requests.line_commands.is_empty());
        assert!(output.flow_events.is_empty());
        assert_eq!(executor.snapshot().unwrap(), before);
    }
    executor
        .presentation
        .as_mut()
        .unwrap()
        .retirements_mut()
        .clear();
    executor.fail_with_trap(
        AwbcTrapCode::HostAbiMismatch,
        "outside VM".into(),
        None,
        &mut RuntimeStepOutput::default(),
    );
    assert_eq!(executor.fiber.status, FiberStatus::Trapped);
    assert_eq!(
        executor
            .presentation
            .as_ref()
            .unwrap()
            .retirements()
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        tokens
    );
}

#[test]
fn lifecycle_trap_cannot_discard_real_grants_when_the_bank_is_unavailable() {
    let (mut executor, _) = super::traps::nested_fixture(false);
    let before = executor.snapshot().unwrap();
    let banks = executor.presentation.take();
    let mut output = RuntimeStepOutput::default();
    executor.fail_with_trap(
        AwbcTrapCode::HostAbiMismatch,
        "outside VM".into(),
        None,
        &mut output,
    );
    assert_eq!(output.diagnostics.len(), 1);
    assert!(
        output.diagnostics[0]
            .message
            .contains(&RuntimePresentationError::MissingLedger.to_string())
    );
    assert_eq!(executor.fiber.status, FiberStatus::Running);
    assert_eq!(
        executor.fiber.presentation_unwind_scopes().unwrap().len(),
        3
    );
    executor.presentation = banks;
    assert_eq!(executor.snapshot().unwrap(), before);
}

fn bind_body(
    fiber: &mut FiberState,
    banks: &mut PresentationBanks<PresentationBody<RuntimePayload>>,
    owner: &crate::task::RuntimeProgramOwner,
) -> RuntimePresentationHandleToken {
    let ledger = banks.ledger_mut(fiber.presentation_bank()).unwrap();
    let scope = fiber.bind_presentation_scope(owner, ledger).unwrap();
    ledger
        .register(
            owner,
            scope,
            PresentationRegistrationSite::Independent(PresentationResourceKind::Image),
            PresentationBody {
                expected: crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
                binding: None,
                value: RuntimePayload(RuntimeValue::Bool(true)),
            },
        )
        .unwrap()
        .0
}

fn banks(
    executor: &AwbcProductStepExecutor,
    blocked: bool,
) -> PresentationBanks<PresentationBody<RuntimePayload>> {
    PresentationBanks::new(
        crate::task::RuntimeProgramOwner::Awbc(executor.program_arc()),
        executor.artifact_fingerprint,
        if blocked {
            PresentationLedgerLimits {
                handles: 1,
                scopes: 8,
            }
        } else {
            PresentationLedgerLimits::for_schema(crate::entry::RuntimeSchemaLimits::engine_default())
        },
    )
}

#[test]
fn actual_independent_child_vm_refusal_keeps_its_checkpoint_queue_and_original_budget() {
    let mut executor =
        AwbcProductStepExecutor::for_entry(super::traps::nested_program(), AwbcEntryId(0), 64)
            .unwrap();
    let instance = crate::runtime_id::RuntimeFiberInstanceId::from_allocated(
        executor
            .next_fiber_instance
            .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
            .unwrap(),
    );
    let fiber = FiberState::for_function_in_context(
        &executor.program,
        AwbcFiberRoot::Function(AwbcFunctionId(1)),
        AwbcFunctionId(1),
        FiberCreationContext::new(
            instance,
            executor.runtime_generation.get(),
            64,
            RuntimePresentationBankId::Root,
        ),
    )
    .unwrap();
    executor.child_fibers.push_back(ProductChildFiber {
        owner: ProductChildFiberOwner::Independent,
        fiber,
        runtime_generation: executor.runtime_generation,
        pending_host_call: None,
    });
    let mut output = RuntimeStepOutput::default();
    assert!(executor.step_next_child(
        &mut output,
        &mut crate::pure::VmRuntimePureCallBackend::default(),
        &mut RuntimeStepInput::default(),
        &mut Vec::new()
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(
        executor.child_fibers[0]
            .fiber
            .active_frame()
            .unwrap()
            .scopes
            .len(),
        1
    );
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let mut banks = banks(&executor, true);
    let fiber = &mut executor.child_fibers[0].fiber;
    bind_body(fiber, &mut banks, &owner);
    fiber.retire_presentation_scope(&owner, &mut banks).unwrap();
    let token = bind_body(fiber, &mut banks, &owner);
    executor.presentation = Some(banks);
    let before = executor.snapshot().unwrap();
    for _ in 0..2 {
        let mut output = RuntimeStepOutput::default();
        assert!(!executor.step_next_child(
            &mut output,
            &mut crate::pure::VmRuntimePureCallBackend::default(),
            &mut RuntimeStepInput::default(),
            &mut Vec::new()
        ));
        assert_eq!(output.diagnostics.len(), 1);
        assert!(
            output.diagnostics[0]
                .message
                .contains(&RuntimePresentationError::RetirementCapacity.to_string())
        );
        assert!(output.flow_events.is_empty());
        assert_eq!(executor.snapshot().unwrap(), before);
    }
    executor
        .presentation
        .as_mut()
        .unwrap()
        .retirements_mut()
        .clear();
    executor.step_next_child(
        &mut RuntimeStepOutput::default(),
        &mut crate::pure::VmRuntimePureCallBackend::default(),
        &mut RuntimeStepInput::default(),
        &mut Vec::new(),
    );
    assert!(executor.child_fibers.is_empty());
    assert_eq!(
        executor.presentation.as_ref().unwrap().retirements()[0].token,
        token
    );
}

fn dialogue_fixture(blocked: bool) -> (AwbcProductStepExecutor, RuntimePresentationHandleToken) {
    let mut program = mark_selector_program(
        arcweft_interaction_model::input::InputActionId::new("trap.refusal").unwrap(),
    );
    program.runtime_types.push(AwbcRuntimeType::new(
        crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
        AwbcRuntimeTypeShape::Bool,
    ));
    let root_layout = AwbcFrameLayoutId(program.frame_layouts.len().try_into().unwrap());
    program.frame_layouts.push(AwbcFrameLayout {
        scopes: vec![],
        slots: [AwbcTypeId(2), AwbcTypeId(1)]
            .into_iter()
            .map(|ty| AwbcFrameSlot {
                name: None,
                ty,
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            })
            .collect(),
        max_scope_depth: 0,
    });
    program.functions[0].frame_layout = root_layout;
    let resume = AwbcResumePointId(program.resume_points.len().try_into().unwrap());
    program.resume_points.push(AwbcResumePoint {
        function: AwbcFunctionId(0),
        block: AwbcBlockId(0),
        frame_layout: root_layout,
        kind: AwbcSafePointKind::Dialogue,
    });
    let result_pattern = AwbcPatternId(program.patterns.len().try_into().unwrap());
    program.patterns.push(AwbcPattern::Bind {
        target: AwbcRegisterId(1),
        mutable: false,
        expected: Some(AwbcTypeId(1)),
    });
    program.blocks[0].terminator = AwbcTerminator::Dialogue {
        target: AwbcRegisterId(0),
        content: AwbcContentUnitId(0),
        values: vec![],
        effects: vec![],
        line_task_captures: vec![],
        result: AwbcDialogueResultTarget {
            ty: AwbcTypeId(1),
            pattern: result_pattern,
            destination: AwbcRegisterId(1),
        },
        resume,
    };
    let nop = AwbcInstructionId(program.instructions.len().try_into().unwrap());
    program.instructions.push(AwbcInstruction::Nop);
    let activation_block = program.functions[1].entry_block.index();
    program.blocks[activation_block].instructions = AwbcTableRange::new(nop.0, 1);
    program.blocks[activation_block].terminator = AwbcTerminator::Trap {
        code: AwbcTrapCode::ExplicitPanic,
        message: None,
    };
    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
    executor
        .fiber
        .active_frame_mut()
        .unwrap()
        .set_register(
            AwbcRegisterId(0),
            RuntimeValue::Opaque(fixture_dialogue_target()),
        )
        .unwrap();
    for _ in 0..8 {
        let output = executor.step(
            Default::default(),
            RuntimeStepOptions {
                budget: crate::step::RuntimeStepBudget { max_ops: 1 },
                ..Default::default()
            },
        );
        assert!(
            output.output.diagnostics.is_empty(),
            "{:?}",
            output.output.diagnostics
        );
        if executor.dialogues.active_frame().is_some() {
            break;
        }
    }
    let activation = executor
        .dialogues
        .active_frame()
        .expect("actual dialogue instruction issued its activation")
        .activation
        .clone();
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let mut banks = banks(&executor, blocked);
    let mut transaction = executor.dialogues.begin_transaction(&activation).unwrap();
    let ProductDialoguePhase::Activating { fiber, .. } = &mut transaction.frame_mut().phase else {
        panic!("actual activation must still be executing its one-op initializer")
    };
    assert_eq!(fiber.cursor.instruction_offset, 1);
    if blocked {
        bind_body(fiber, &mut banks, &owner);
        fiber.retire_presentation_scope(&owner, &mut banks).unwrap();
    }
    let token = bind_body(fiber, &mut banks, &owner);
    executor.presentation = Some(banks);
    commit_test_dialogue_transaction(&mut executor, transaction);
    executor.snapshot().unwrap();
    (executor, token)
}

#[test]
fn actual_dialogue_vm_refusal_retains_the_activation_instead_of_starting_failure_close() {
    let (mut executor, token) = dialogue_fixture(true);
    let before = executor.snapshot().unwrap();
    for _ in 0..2 {
        let output = executor.step(
            Default::default(),
            RuntimeStepOptions {
                budget: crate::step::RuntimeStepBudget { max_ops: 1 },
                ..Default::default()
            },
        );
        assert_eq!(output.output.diagnostics.len(), 1);
        assert!(
            output.output.diagnostics[0]
                .message
                .contains(&RuntimePresentationError::RetirementCapacity.to_string())
        );
        assert!(output.output.requests.line_commands.is_empty());
        assert_eq!(executor.snapshot().unwrap(), before);
    }
    executor
        .presentation
        .as_mut()
        .unwrap()
        .retirements_mut()
        .clear();
    let output = executor.step(
        Default::default(),
        RuntimeStepOptions {
            budget: crate::step::RuntimeStepBudget { max_ops: 1 },
            ..Default::default()
        },
    );
    assert!(!output.output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains(&RuntimePresentationError::RetirementCapacity.to_string())
    }));
    assert_eq!(
        executor.presentation.as_ref().unwrap().retirements()[0].token,
        token
    );
}
