use super::*;
use crate::presentation::resources::{
    PresentationBanks, PresentationBody, PresentationLedgerLimits, PresentationRegistrationSite,
};
use crate::runtime_id::{RuntimePresentationBankId, RuntimePresentationHandleToken};
use arcweft_interaction_model::presentation::PresentationResourceKind;

#[test]
fn vm_terminal_return_preserves_the_affine_result_until_scope_custody_is_available() {
    use crate::program_invocation::RuntimeProgramInput;
    let (executor, program, token) = completed_handle_program();
    let mut executor = executor
        .continue_program(program, vec![RuntimeProgramInput::PreviousResult])
        .unwrap();
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let mut banks = PresentationBanks::<PresentationBody<RuntimePayload>>::new(
        owner.clone(),
        executor.artifact_fingerprint,
        PresentationLedgerLimits::for_schema(crate::entry::RuntimeSchemaLimits::engine_default()),
    );
    let ledger = banks.ledger_mut(RuntimePresentationBankId::Root).unwrap();
    executor
        .fiber
        .bind_presentation_scope(&owner, ledger)
        .unwrap();
    let cursor = executor.fiber.cursor;
    let error = crate::awbc::vm::step(
        &executor.program,
        &mut executor.fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 64,
        },
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains(&crate::presentation::RuntimePresentationError::MissingLedger.to_string()),
        "{error}"
    );
    let retained = executor.fiber.active_frame().unwrap().registers[0]
        .as_ref()
        .expect("failed return must retain the affine register");
    assert_eq!(retained.affine_line_handles().unwrap()[0].token(), &token);
    assert_eq!(executor.fiber.cursor, cursor);
    assert_eq!(executor.fiber.status, FiberStatus::Running);
    assert!(
        executor
            .fiber
            .release_presentation_scope(&owner, ledger)
            .unwrap()
            .is_empty()
    );
    let output = crate::awbc::vm::step(
        &executor.program,
        &mut executor.fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 64,
        },
    )
    .unwrap();
    let crate::awbc::vm::VmExit::Returned(Some(value)) = output.exit else {
        panic!("return resumes after original scope retirement")
    };
    assert_eq!(value.affine_line_handles().unwrap()[0].token(), &token);
}

#[test]
fn vm_terminal_dialogue_selection_preserves_its_value_until_scope_custody_is_available() {
    let program = mark_selector_program(
        arcweft_interaction_model::input::InputActionId::new("dialogue.cancel.cleanup").unwrap(),
    );
    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
    let activation = issue_test_dialogue(&mut executor, AwbcContentUnitId(0));
    let mut fiber = create_test_dialogue_fiber(&mut executor, activation, AwbcFunctionId(2));
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let mut banks = PresentationBanks::<PresentationBody<RuntimePayload>>::new(
        owner.clone(),
        executor.artifact_fingerprint,
        PresentationLedgerLimits::for_schema(crate::entry::RuntimeSchemaLimits::engine_default()),
    );
    let ledger = banks
        .ledger_mut(RuntimePresentationBankId::Dialogue { activation })
        .unwrap();
    fiber.bind_presentation_scope(&owner, ledger).unwrap();
    crate::awbc::vm::step(
        &executor.program,
        &mut fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap();
    let cursor = fiber.cursor;
    let error = crate::awbc::vm::step(
        &executor.program,
        &mut fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains(&crate::presentation::RuntimePresentationError::MissingLedger.to_string()),
        "{error}"
    );
    assert_eq!(
        fiber.active_frame().unwrap().registers[0].as_ref(),
        Some(&RuntimeValue::String("selected-result".into()))
    );
    assert_eq!(fiber.cursor, cursor);
    assert_eq!(fiber.status, FiberStatus::Running);
    assert!(
        fiber
            .release_presentation_scope(&owner, ledger)
            .unwrap()
            .is_empty()
    );
    let output = crate::awbc::vm::step(
        &executor.program,
        &mut fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap();
    assert_eq!(
        output.exit,
        crate::awbc::vm::VmExit::DialogueResultSelected(RuntimeValue::String(
            "selected-result".into()
        ))
    );
}

fn fixture() -> (AwbcProductStepExecutor, RuntimePresentationHandleToken) {
    fixture_with_program(return_program())
}

fn fixture_with_program(
    mut program: AwbcProgram,
) -> (AwbcProductStepExecutor, RuntimePresentationHandleToken) {
    let expected = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    program
        .runtime_types
        .push(AwbcRuntimeType::new(expected, AwbcRuntimeTypeShape::Bool));
    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let mut banks = PresentationBanks::new(
        owner.clone(),
        executor.artifact_fingerprint,
        PresentationLedgerLimits::for_schema(crate::entry::RuntimeSchemaLimits::engine_default()),
    );
    let ledger = banks.ledger_mut(RuntimePresentationBankId::Root).unwrap();
    let scope = executor
        .fiber
        .bind_presentation_scope(&owner, ledger)
        .unwrap();
    let (token, _) = ledger
        .register(
            &owner,
            scope,
            PresentationRegistrationSite::Independent(PresentationResourceKind::View),
            PresentationBody {
                expected,
                binding: None,
                value: RuntimePayload(RuntimeValue::Bool(true)),
            },
        )
        .unwrap();
    executor.presentation = Some(banks);
    (executor, token)
}

#[test]
fn execution_checkpoint_restores_the_original_bank_body_grant_and_cursors() {
    let (mut executor, _) = fixture();
    let before = executor.snapshot().unwrap();
    let checkpoint = executor
        .execution_checkpoint(super::super::checkpoint::ProductCheckpointTarget::Main)
        .unwrap();
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let ledger = executor
        .presentation
        .as_mut()
        .unwrap()
        .ledger_mut(RuntimePresentationBankId::Root)
        .unwrap();
    let retired = executor
        .fiber
        .release_presentation_scope(&owner, ledger)
        .unwrap();
    assert_eq!(retired.len(), 1);
    let scope = executor
        .fiber
        .bind_presentation_scope(&owner, ledger)
        .unwrap();
    ledger
        .register(
            &owner,
            scope,
            PresentationRegistrationSite::Independent(PresentationResourceKind::View),
            PresentationBody {
                expected: crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
                binding: None,
                value: RuntimePayload(RuntimeValue::Bool(false)),
            },
        )
        .unwrap();
    checkpoint
        .restore(
            &mut executor.fiber,
            &mut executor.presentation,
            &executor.program,
        )
        .unwrap();
    assert_eq!(executor.snapshot().unwrap(), before);
}

#[test]
fn execution_checkpoint_rejects_a_foreign_program_without_mutating_the_live_graph() {
    let (mut executor, _) = fixture();
    let before = executor.snapshot().unwrap();
    let checkpoint = executor
        .execution_checkpoint(super::super::checkpoint::ProductCheckpointTarget::Main)
        .unwrap();
    let foreign =
        AwbcProductStepExecutor::for_entry(executor.program().clone(), AwbcEntryId(0), 64).unwrap();
    assert!(
        checkpoint
            .restore(
                &mut executor.fiber,
                &mut executor.presentation,
                &foreign.program
            )
            .is_err()
    );
    assert_eq!(executor.snapshot().unwrap(), before);
}

#[test]
fn execution_checkpoint_includes_root_and_the_actual_withdrawn_dialogue_grants() {
    let (mut executor, activation) = initial_actor_dialogue_executor();
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let bank = RuntimePresentationBankId::Dialogue { activation };
    let mut banks = PresentationBanks::<PresentationBody<RuntimePayload>>::new(
        owner.clone(),
        executor.artifact_fingerprint,
        PresentationLedgerLimits::for_schema(crate::entry::RuntimeSchemaLimits::engine_default()),
    );
    executor
        .fiber
        .bind_presentation_scope(
            &owner,
            banks.ledger_mut(RuntimePresentationBankId::Root).unwrap(),
        )
        .unwrap();
    let mut transaction = executor.dialogues.begin_transaction(&activation).unwrap();
    let ProductDialoguePhase::Activating { fiber, .. } = &mut transaction.frame_mut().phase else {
        unreachable!()
    };
    fiber
        .bind_presentation_scope(&owner, banks.ledger_mut(bank).unwrap())
        .unwrap();
    executor.presentation = Some(banks);
    commit_test_dialogue_transaction(&mut executor, transaction);
    let before = executor.snapshot().unwrap();
    let mut transaction = executor.dialogues.begin_transaction(&activation).unwrap();
    assert!(
        executor
            .execution_checkpoint(super::super::checkpoint::ProductCheckpointTarget::Main)
            .is_err(),
        "a complete contour cannot silently omit the withdrawn owner"
    );
    let checkpoint = executor
        .execution_checkpoint(super::super::checkpoint::ProductCheckpointTarget::Dialogue(
            &transaction,
        ))
        .unwrap();
    let ProductDialoguePhase::Activating { fiber, .. } = &mut transaction.frame_mut().phase else {
        unreachable!()
    };
    fiber
        .release_presentation_scope(
            &owner,
            executor
                .presentation
                .as_mut()
                .unwrap()
                .ledger_mut(bank)
                .unwrap(),
        )
        .unwrap();
    checkpoint
        .restore(fiber, &mut executor.presentation, &executor.program)
        .unwrap();
    executor.dialogues.restore_transaction(transaction).unwrap();
    assert_eq!(executor.snapshot().unwrap(), before);
}

#[test]
fn product_one_op_runs_with_a_live_bank_before_its_lexical_exit() {
    let mut program = return_program();
    program.frame_layouts[0]
        .scopes
        .push(crate::awbc::schema::AwbcScopeDefinition {
            kind: crate::scope::RuntimeScopeFrameKind::EmittedLexical,
            parent: None,
            identity: crate::scope::RuntimeScopeIdentity::Anonymous,
        });
    program.frame_layouts[0].max_scope_depth = 1;
    program.instructions = vec![
        AwbcInstruction::EnterScope {
            scope: crate::awbc::schema::AwbcScopeId(0),
        },
        AwbcInstruction::ExitScope {
            scope: crate::awbc::schema::AwbcScopeId(0),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    let (mut executor, _) = fixture_with_program(program);
    let output = executor.step(
        Default::default(),
        RuntimeStepOptions {
            budget: crate::step::RuntimeStepBudget { max_ops: 1 },
            ..RuntimeStepOptions::default()
        },
    );
    assert!(
        output.output.diagnostics.is_empty(),
        "{:?}",
        output.output.diagnostics
    );
    assert_eq!(executor.fiber.cursor.instruction_offset, 1);
    assert_eq!(executor.fiber.active_frame().unwrap().scopes.len(), 1);
    executor.snapshot().unwrap();
}

#[test]
fn product_public_save_restores_actual_scope_bank_body_and_original_program() {
    let (executor, _) = fixture();
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let value = serde_json::to_value(save).unwrap();
    assert!(
        value["presentation"]["ledgers"][0][1]
            .get("limits")
            .is_none()
    );
    assert!(value["presentation"].get("program").is_none());
    let save = serde_json::from_value(value).unwrap();
    let restored = executor.restore_inert_snapshot_owned(save).unwrap();
    assert_eq!(restored.snapshot().unwrap(), before);
    let mut bool_roots = 0;
    restored
        .visit_live_runtime_values(&mut |value: &RuntimeValue| {
            bool_roots += usize::from(matches!(value, RuntimeValue::Bool(true)));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(bool_roots, 1, "resource bodies are part of the live graph");
}

#[test]
fn product_public_restore_rejects_missing_bank_without_losing_the_live_owner() {
    let (executor, _) = fixture();
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let mut value = serde_json::to_value(save).unwrap();
    value["presentation"] = serde_json::Value::Null;
    let save = serde_json::from_value(value).unwrap();
    let (restored, error) = executor.restore_inert_snapshot_owned(save).unwrap_err();
    assert!(error.to_string().contains("presentation"), "{error}");
    assert_eq!(restored.snapshot().unwrap(), before);
}

#[test]
fn product_save_visitors_include_retirements_and_body_headers_reject_wrong_types() {
    let (mut executor, _) = fixture();
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let mut banks = executor.presentation.take().unwrap();
    let ledger = banks.ledger_mut(RuntimePresentationBankId::Root).unwrap();
    let retired = executor
        .fiber
        .release_presentation_scope(&owner, ledger)
        .unwrap();
    banks.retirements_mut().extend(retired);
    executor.presentation = Some(banks);
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let mut value = serde_json::to_value(save).unwrap();
    let restored = executor
        .restore_inert_snapshot_owned(serde_json::from_value(value.clone()).unwrap())
        .unwrap();
    assert_eq!(restored.snapshot().unwrap(), before);
    value["presentation"]["retirements"][0]["body"]["expected"] =
        serde_json::to_value(crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest())
            .unwrap();
    let (restored, _) = restored
        .restore_inert_snapshot_owned(serde_json::from_value(value).unwrap())
        .unwrap_err();
    assert_eq!(restored.snapshot().unwrap(), before);
    let mut bool_roots = 0;
    restored
        .visit_live_runtime_values(&mut |value: &RuntimeValue| {
            bool_roots += usize::from(matches!(value, RuntimeValue::Bool(true)));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(bool_roots, 1);
}

#[test]
fn public_restore_rejects_forged_bank_and_namespace_frontiers_atomically() {
    let (mut executor, _) = fixture();
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let original = serde_json::to_value(save).unwrap();
    let activation = crate::runtime_id::DialogueActivationId::new(
        executor.artifact_fingerprint,
        crate::runtime_id::RuntimePersistentFiberId::from_allocated(1),
        crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
            std::num::NonZeroU32::MIN,
        ),
        0,
    );
    let row = serde_json::json!({"owner": activation.owner_fiber(), "content": activation.content(), "next": 1});
    for mutation in 0..6 {
        let mut wire = original.clone();
        match mutation {
            0 => {
                wire["dialogue_occurrences"] = serde_json::json!([row]);
                wire["fiber"]["presentation_bank"] =
                    serde_json::to_value(RuntimePresentationBankId::Dialogue { activation })
                        .unwrap();
            }
            1 => wire["dialogue_occurrences"] = serde_json::json!([row, row]),
            2 => {
                wire["dialogue_occurrences"] = serde_json::json!([row]);
                wire["dialogue_occurrences"][0]["next"] = 0.into();
            }
            3 => {
                wire["dialogue_occurrences"] = serde_json::json!([row]);
                wire["dialogue_occurrences"][0]["owner"] = serde_json::to_value(
                    crate::runtime_id::RuntimePersistentFiberId::from_allocated(2),
                )
                .unwrap();
            }
            4 => {
                wire["fiber"]
                    .as_object_mut()
                    .unwrap()
                    .remove("presentation_bank");
            }
            5 => {
                wire["dialogue_occurrences"] = serde_json::json!([row, row]);
                let body = wire["presentation"]["ledgers"][0][1]["leases"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap();
                body["body"]["expected"] = serde_json::to_value(
                    crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest(),
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let save = match serde_json::from_value(wire) {
            Ok(save) => save,
            Err(_) if mutation == 4 => continue,
            Err(error) => panic!("mutation {mutation} must reach retained admission: {error}"),
        };
        let (retained, error) = executor.restore_inert_snapshot_owned(save).unwrap_err();
        if mutation == 5 {
            assert!(
                error.to_string().contains(
                    &crate::presentation::RuntimePresentationError::InvalidState.to_string()
                ),
                "metadata must reject before invalid body admission: {error}"
            );
        }
        executor = retained;
        assert_eq!(executor.snapshot().unwrap(), before, "mutation {mutation}");
    }
}

#[test]
fn dialogue_bank_roundtrips_and_duplicate_activation_fiber_rejection_keeps_the_owner() {
    let (executor, activation) = initial_actor_dialogue_executor();
    let bank = RuntimePresentationBankId::Dialogue { activation };
    let fiber = executor
        .dialogues
        .active_frame()
        .unwrap()
        .activation_fiber()
        .unwrap();
    assert_ne!(fiber.instance, executor.fiber.instance);
    assert_eq!(fiber.presentation_bank(), bank);
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let mut wire = serde_json::to_value(save).unwrap();
    let executor = executor
        .restore_inert_snapshot_owned(serde_json::from_value(wire.clone()).unwrap())
        .unwrap();
    assert_eq!(executor.snapshot().unwrap(), before);
    assert_eq!(
        executor
            .dialogues
            .active_frame()
            .unwrap()
            .activation_fiber()
            .unwrap()
            .presentation_bank(),
        bank
    );
    wire["dialogues"]["entries"][0]["Active"]["frame"]["phase"]["Activating"]["fiber"]["instance"] =
        serde_json::to_value(executor.fiber.instance).unwrap();
    let (executor, error) = executor
        .restore_inert_snapshot_owned(serde_json::from_value(wire).unwrap())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains(&crate::presentation::RuntimePresentationError::InvalidState.to_string()),
        "{error}"
    );
    assert_eq!(executor.snapshot().unwrap(), before);
}
