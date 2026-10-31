use super::*;
use crate::presentation::resources::{
    PresentationBanks, PresentationBody, PresentationLedgerLimits, PresentationRegistrationSite,
};
use crate::runtime_id::{RuntimePresentationBankId, RuntimePresentationHandleToken};
use arcweft_interaction_model::presentation::PresentationResourceKind;

mod cancellation {
    use super::*;
    use crate::awbc::schema::*;
    use crate::line_task::{
        LineTaskActivation, LineTaskCommand, LineTaskLiveState, LineTaskWorkTag,
    };
    use crate::presentation::RuntimePresentationError;
    use crate::runtime_id::DialogueActivationId;
    use crate::step::RuntimeStepOutput;
    use std::collections::BTreeSet;

    fn fixture(
        queued: bool,
    ) -> (
        AwbcProductStepExecutor,
        DialogueActivationId,
        Vec<LineTaskWorkTag>,
        Vec<RuntimePresentationHandleToken>,
    ) {
        let mut program = mark_selector_program(
            arcweft_interaction_model::input::InputActionId::new("presentation.cancel").unwrap(),
        );
        program.runtime_types.push(AwbcRuntimeType::new(
            crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
            AwbcRuntimeTypeShape::Bool,
        ));
        program.line_task_nodes = vec![
            AwbcLineTaskNode::Parallel {
                policy: AwbcParallelPolicy::JoinAll,
                children: vec![AwbcLineTaskNodeId(1), AwbcLineTaskNodeId(3)],
            },
            AwbcLineTaskNode::Child {
                trigger: AwbcLineTaskTrigger::Immediate,
                join: AwbcChildJoinPolicy::Join,
                cancel: AwbcChildCancelPolicy::CancelAndJoin,
                scope: AwbcLineTaskNodeId(2),
            },
            AwbcLineTaskNode::Action(AwbcFunctionId(2)),
            AwbcLineTaskNode::Child {
                trigger: AwbcLineTaskTrigger::Immediate,
                join: AwbcChildJoinPolicy::Join,
                cancel: AwbcChildCancelPolicy::CancelAndJoin,
                scope: AwbcLineTaskNodeId(4),
            },
            AwbcLineTaskNode::Action(AwbcFunctionId(2)),
        ];
        program.line_task_groups[0].nodes = AwbcTableRange::new(0, 5);
        let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
        let activation = issue_test_dialogue(&mut executor, AwbcContentUnitId(0));
        let view = executor.line_task_view(AwbcContentUnitId(0)).unwrap();
        let mut live = LineTaskLiveState::new(&view, activation);
        let run = crate::line_task::progress_live_line_task_group(
            &view,
            Default::default(),
            crate::line_task::LineTaskReadyEvents::new(&BTreeSet::new()),
            &mut live,
        )
        .unwrap();
        assert_eq!(run.commands.len(), 2);
        executor
            .dialogues
            .begin(ActiveDialogue {
                activation,
                content: AwbcContentUnitId(0),
                target: fixture_dialogue_target(),
                target_type: AwbcTypeId(2),
                line: crate::plan::RuntimeLineId::from_runtime_line_value(
                    "line.presentation.cancel",
                )
                .unwrap(),
                captures: Box::new([]),
                task_inputs: Box::new([]),
                values: Box::new([]),
                effect_callbacks: BTreeMap::new(),
                voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
                result: AwbcDialogueResultTarget {
                    ty: AwbcTypeId(1),
                    pattern: AwbcPatternId(0),
                    destination: AwbcRegisterId(0),
                },
                phase: ProductDialoguePhase::Reducing { line_task: live },
                elapsed_nanos: 0,
                pending_content_events: vec![],
                pending_advance: false,
                pending_line_outcomes: vec![],
                pending_activation_host_call: None,
            })
            .unwrap();
        let mut transaction = executor.dialogues.begin_transaction(&activation).unwrap();
        transaction
            .line_mut()
            .commit_result(AwbcTypeId(1), RuntimeValue::String("base-result".into()))
            .unwrap();
        commit_test_dialogue_transaction(&mut executor, transaction);
        let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
        let mut banks = PresentationBanks::new(
            owner.clone(),
            executor.artifact_fingerprint,
            PresentationLedgerLimits {
                handles: 3,
                scopes: 8,
            },
        );
        let body = |value| PresentationBody {
            expected: crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
            binding: None,
            value: RuntimePayload(RuntimeValue::Bool(value)),
        };
        let site = || PresentationRegistrationSite::Independent(PresentationResourceKind::Image);
        if queued {
            let ledger = banks.ledger_mut(RuntimePresentationBankId::Root).unwrap();
            let scope = executor
                .fiber
                .bind_presentation_scope(&owner, ledger)
                .unwrap();
            for value in [false, true] {
                ledger.register(&owner, scope, site(), body(value)).unwrap();
            }
            executor
                .fiber
                .retire_presentation_scope(&owner, &mut banks)
                .unwrap();
        }
        let ledger = banks.ledger_mut(RuntimePresentationBankId::Root).unwrap();
        let root = executor
            .fiber
            .bind_presentation_scope(&owner, ledger)
            .unwrap();
        ledger.register(&owner, root, site(), body(true)).unwrap();
        let mut tags = Vec::new();
        let mut tokens = Vec::new();
        for command in run.commands {
            let LineTaskCommand::Run { tag, policy } = command else {
                unreachable!()
            };
            let mut fiber =
                create_test_dialogue_fiber(&mut executor, activation, AwbcFunctionId(2));
            let ledger = banks
                .ledger_mut(RuntimePresentationBankId::Dialogue { activation })
                .unwrap();
            let scope = fiber.bind_presentation_scope(&owner, ledger).unwrap();
            tokens.push(
                ledger
                    .register(&owner, scope, site(), body(false))
                    .unwrap()
                    .0,
            );
            tags.push(tag.clone());
            executor.child_fibers.push_back(ProductChildFiber {
                owner: ProductChildFiberOwner::LineTask {
                    content: AwbcContentUnitId(0),
                    tag,
                    policy,
                    phase: ProductLineTaskFiberPhase::Active,
                },
                fiber,
                runtime_generation: executor.runtime_generation,
                pending_host_call: None,
            });
        }
        executor.presentation = Some(banks);
        executor.snapshot().unwrap();
        (executor, activation, tags, tokens)
    }

    fn commands(tags: &[LineTaskWorkTag]) -> LineTaskActivation {
        LineTaskActivation {
            commands: tags
                .iter()
                .cloned()
                .map(|tag| LineTaskCommand::Cancel { tag })
                .collect(),
            scheduled_completions: Vec::new(),
        }
    }

    fn cancel(
        executor: &mut AwbcProductStepExecutor,
        activation: DialogueActivationId,
        tags: &[LineTaskWorkTag],
    ) {
        let mut transaction = executor.dialogues.begin_transaction(&activation).unwrap();
        let mut batch = executor
            .prepare_line_task_commands(&mut transaction, commands(tags))
            .unwrap();
        let prepared = executor
            .preflight_line_task_commands(&mut transaction, &batch)
            .unwrap();
        let proof = executor.dialogues.inspect_commit(&transaction).unwrap();
        executor.realize_line_task_commands(&mut transaction, &mut batch, prepared);
        executor.dialogues.commit_prepared(transaction, proof);
        let mut output = RuntimeStepOutput::default();
        executor.commit_line_task_commands(batch, &mut output);
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn line_cancellation_retires_all_actual_child_scopes_before_removal_and_public_save_roundtrips()
    {
        let (mut executor, activation, mut tags, mut tokens) = fixture(false);
        tags.reverse();
        tokens.reverse();
        cancel(&mut executor, activation, &tags);
        assert!(executor.child_fibers.is_empty());
        assert!(
            executor
                .fiber
                .active_frame()
                .unwrap()
                .presentation_scope_binding()
                .is_some()
        );
        let retired = executor.presentation.as_ref().unwrap().retirements();
        assert_eq!(
            retired
                .iter()
                .map(|entry| entry.token.clone())
                .collect::<Vec<_>>(),
            tokens
        );
        let after = executor.snapshot().unwrap();
        let save = AwbcProductExecutorSaveSnapshot::from_live(&after, &executor.program).unwrap();
        let wire = serde_json::to_vec(&save).unwrap();
        executor = executor
            .restore_inert_snapshot_owned(serde_json::from_slice(&wire).unwrap())
            .unwrap();
        assert_eq!(executor.snapshot().unwrap(), after);
    }

    #[test]
    fn later_child_bank_forgery_rejects_before_any_cancellation_or_grant_move() {
        let (mut executor, activation, tags, _) = fixture(false);
        let before = executor.snapshot().unwrap();
        executor.child_fibers[1].fiber.presentation_bank = RuntimePresentationBankId::Root;
        let mut transaction = executor.dialogues.begin_transaction(&activation).unwrap();
        let error = executor
            .prepare_line_task_commands(&mut transaction, commands(&tags))
            .err()
            .unwrap();
        assert!(
            matches!(
                error,
                ProductStepError::Fiber(crate::awbc::fiber::FiberStateError::Snapshot(
                    crate::value::AwbcRuntimeValueSnapshotError::Presentation(
                        RuntimePresentationError::Owner
                    )
                ))
            ),
            "{error:?}"
        );
        executor.dialogues.restore_transaction(transaction).unwrap();
        executor.child_fibers[1].fiber.presentation_bank =
            RuntimePresentationBankId::Dialogue { activation };
        assert_eq!(executor.snapshot().unwrap(), before);
        cancel(&mut executor, activation, &tags);
        assert!(executor.child_fibers.is_empty());
    }

    #[test]
    fn combined_child_retirement_capacity_rejects_without_partial_cancel_and_retry_preserves_order()
    {
        let (mut executor, activation, tags, tokens) = fixture(true);
        let before = executor.snapshot().unwrap();
        let mut transaction = executor.dialogues.begin_transaction(&activation).unwrap();
        let error = executor
            .prepare_line_task_commands(&mut transaction, commands(&tags))
            .err()
            .unwrap();
        assert!(
            matches!(
                error,
                ProductStepError::Fiber(crate::awbc::fiber::FiberStateError::Snapshot(
                    crate::value::AwbcRuntimeValueSnapshotError::Presentation(
                        RuntimePresentationError::RetirementCapacity
                    )
                ))
            ),
            "{error:?}"
        );
        executor.dialogues.restore_transaction(transaction).unwrap();
        assert_eq!(executor.snapshot().unwrap(), before);
        assert_eq!(executor.child_fibers.len(), 2);
        executor
            .presentation
            .as_mut()
            .unwrap()
            .retirements_mut()
            .clear();
        cancel(&mut executor, activation, &tags);
        assert!(executor.child_fibers.is_empty());
        assert_eq!(
            executor
                .presentation
                .as_ref()
                .unwrap()
                .retirements()
                .iter()
                .map(|entry| entry.token.clone())
                .collect::<Vec<_>>(),
            tokens
        );
        executor.snapshot().unwrap();
    }
}

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
fn execution_checkpoint_preserves_the_typed_bank_metadata_rejection() {
    let (mut executor, activation) = initial_actor_dialogue_executor();
    let before = executor.snapshot().unwrap();
    let mut transaction = executor.dialogues.begin_transaction(&activation).unwrap();
    let ProductDialoguePhase::Activating { fiber, .. } = &mut transaction.frame_mut().phase else {
        unreachable!()
    };
    fiber.presentation_bank = RuntimePresentationBankId::Root;
    let Err(error) = executor.execution_checkpoint(
        super::super::checkpoint::ProductCheckpointTarget::Dialogue(&transaction),
    ) else {
        panic!("a forged Dialogue bank must be rejected")
    };
    assert!(
        matches!(
            error,
            ProductStepError::CheckpointValue(
                crate::value::AwbcRuntimeValueSnapshotError::Presentation(
                    crate::presentation::RuntimePresentationError::Owner
                )
            )
        ),
        "metadata errors retain their typed cause: {error:?}"
    );
    let ProductDialoguePhase::Activating { fiber, .. } = &mut transaction.frame_mut().phase else {
        unreachable!()
    };
    fiber.presentation_bank = RuntimePresentationBankId::Dialogue { activation };
    executor.dialogues.restore_transaction(transaction).unwrap();
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
fn product_one_op_lexical_exit_retires_only_its_actual_scope_and_checkpoint_restores_it() {
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
    let (mut executor, root_token) = fixture_with_program(program);
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
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
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
    let (lexical_token, _) = ledger
        .register(
            &owner,
            scope,
            PresentationRegistrationSite::Independent(PresentationResourceKind::Image),
            PresentationBody {
                expected: crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
                binding: None,
                value: RuntimePayload(RuntimeValue::Bool(false)),
            },
        )
        .unwrap();
    let before = executor.snapshot().unwrap();
    let checkpoint = executor
        .execution_checkpoint(super::super::checkpoint::ProductCheckpointTarget::Main)
        .unwrap();
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
    assert_eq!(executor.fiber.cursor.instruction_offset, 2);
    assert!(executor.fiber.active_frame().unwrap().scopes.is_empty());
    let banks = executor.presentation.as_ref().unwrap();
    assert_eq!(banks.retirements().len(), 1);
    assert_eq!(banks.retirements()[0].token, lexical_token);
    assert_eq!(
        banks.retirements()[0].body.value.0,
        RuntimeValue::Bool(false)
    );
    let after = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&after, &executor.program).unwrap();
    let wire = serde_json::to_vec(&save).unwrap();
    executor = executor
        .restore_inert_snapshot_owned(serde_json::from_slice(&wire).unwrap())
        .unwrap();
    assert_eq!(executor.snapshot().unwrap(), after);
    let root_scope = executor
        .fiber
        .active_frame()
        .unwrap()
        .presentation_scope_binding()
        .expect("the outer root grant remains live");
    assert_eq!(root_scope.bank(), root_token.bank());
    checkpoint
        .restore(
            &mut executor.fiber,
            &mut executor.presentation,
            &executor.program,
        )
        .unwrap();
    assert_eq!(
        executor.snapshot().unwrap(),
        before,
        "the original checkpoint restores body, grant, cursor and queue after an actual VM retirement"
    );
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
