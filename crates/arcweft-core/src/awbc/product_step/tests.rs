use super::{
    ActiveChoice, ActiveDialogue, AwbcProductExecutorSaveSnapshot, AwbcProductStepBuildError,
    AwbcProductStepExecutor, ProductChildFiber, ProductChildFiberOwner, ProductDialoguePhase,
    ProductLineTaskFiberPhase, ProductStepError,
};
use crate::awbc::fiber::{FiberStatus, FiberTerminalValue};
use crate::awbc::product_step::mapping::MappedEffect;
use crate::awbc::schema::{
    AwbcAudioArg, AwbcAudioCommand, AwbcAudioCommandId, AwbcAudioValueRef, AwbcBlock, AwbcBlockId,
    AwbcChoice, AwbcChoiceId, AwbcChoiceOption, AwbcConstant, AwbcConstantId, AwbcContentUnit,
    AwbcContentUnitId, AwbcDialogueContentTemplate, AwbcEffectKind, AwbcEffectPlan,
    AwbcEffectPlanId, AwbcEffectSetId, AwbcEntryId, AwbcFlowBinding, AwbcFlowExecutable,
    AwbcFrameLayout, AwbcFrameLayoutId, AwbcFrameSlot, AwbcFrameSlotRole, AwbcFunction,
    AwbcFunctionFlag, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind, AwbcHostCall,
    AwbcHostCallId, AwbcHostCallMode, AwbcInstruction, AwbcPattern, AwbcPatternId, AwbcProgram,
    AwbcRegisterId, AwbcResumePoint, AwbcResumePointId, AwbcRuntimeType, AwbcRuntimeTypeShape,
    AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcStringId, AwbcTableRange,
    AwbcTerminator, AwbcTrapCode, AwbcTypeId,
};
use crate::effect::{LineEffectRequest, RuntimeAssertionGuardId, RuntimeAssertionProfile};
use crate::engine::{FlowExit, FlowFiberStatus};
use crate::entry::{FlowContractHash, RuntimeFlowExecutable};
use crate::pattern::RuntimeSemanticTypeId;
use crate::step::{
    RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeHostCallMode, RuntimeHostCallResult,
    RuntimeStepInput, RuntimeStepOptions, RuntimeStepStopReason,
};
use crate::task::{
    LogicalEpoch, NeedId, RuntimeNeedState, TaskEvent, TaskEventKind, TaskId, TaskSequence,
};
use crate::value::{RuntimeFlowParameterBinding, RuntimePayload, RuntimeValue};
use arcweft_need::{Need, Progress};

fn fixture_dialogue_target() -> crate::value::RuntimeOpaqueValue {
    let owner = crate::pattern::RuntimeOpaqueTypeOwner::exact_with(
        crate::value::RuntimeCharacterDialogueProducerId::get(),
        RuntimeSemanticTypeId::from_bytes([0x24; 32]),
        crate::value::RuntimeOpaqueValueClass::Plain,
        crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
    );
    crate::value::RuntimeOpaqueValue::new_exact(&owner, RuntimeValue::Unit)
}

#[test]
fn minimal_return_program_finishes_without_diagnostics() {
    let mut executor = AwbcProductStepExecutor::for_entry(
        return_program(),
        crate::awbc::schema::AwbcEntryId(0),
        64,
    )
    .expect("minimal product AWBC executor starts");

    let result = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
    assert!(result.output.diagnostics.is_empty());
    assert!(matches!(result.fiber_status, FlowFiberStatus::Done(_)));
}

#[test]
fn line_activation_register_defer_commits_captures_and_cursor() {
    let mut program = return_program();
    let capture_string = AwbcStringId(u32::try_from(program.strings.len()).expect("string index"));
    program.strings.push("captured at defer".to_owned());
    program.runtime_types = vec![
        AwbcRuntimeType::unit(),
        AwbcRuntimeType::new(
            crate::pattern::RuntimeCheckedType::String.semantic_identity_digest(),
            AwbcRuntimeTypeShape::String,
        ),
    ];
    program.constants = vec![AwbcConstant::String(capture_string), AwbcConstant::Unit];
    program.signatures.extend([
        AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: vec![AwbcTypeId(1)],
            result: Some(AwbcTypeId(0)),
            effects: AwbcEffectSetId(0),
        },
    ]);
    program.frame_layouts.extend([
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            }],
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::ReturnValue,
                    scope_depth: 0,
                },
            ],
            max_scope_depth: 0,
        },
    ]);
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::RegisterDefer {
            site: crate::runtime_id::RuntimeDeferSiteId::from_zero_based(0)
                .expect("first defer site"),
            outcome: crate::line_task::RuntimeDeferOutcomeFilter::Always,
            owner: crate::awbc::schema::AwbcDeferOwner::LineRoot,
            captures: vec![AwbcRegisterId(0)],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(1),
        },
    ];
    let activation = AwbcFunctionId(1);
    let defer_body = AwbcFunctionId(2);
    program.functions.extend([
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::LineActivation,
            signature: AwbcSignatureId(1),
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, 1),
            entry_block: AwbcBlockId(1),
            flags: AwbcFunctionFlags::default(),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(2),
            frame_layout: AwbcFrameLayoutId(2),
            blocks: AwbcTableRange::new(2, 1),
            entry_block: AwbcBlockId(2),
            flags: AwbcFunctionFlags::default(),
        },
    ]);
    program.blocks.extend([
        AwbcBlock {
            owner: activation,
            instructions: AwbcTableRange::new(0, 2),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: defer_body,
            instructions: AwbcTableRange::new(2, 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(1)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
    ]);
    program.defer_sites.push(defer_body);
    program.canonicalize_string_table();
    program
        .verify(Default::default(), Default::default())
        .expect("line-root defer program verifies");

    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64)
        .expect("line-root defer executor starts");
    let content = crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
        std::num::NonZeroU32::MIN,
    );
    let activation_id = crate::runtime_id::DialogueActivationId::new(
        executor.artifact_fingerprint,
        executor.facade_fiber.persistent_id,
        content,
        0,
    );
    let activation_fiber = crate::awbc::fiber::FiberState::for_function(
        &executor.program,
        AwbcEntryId(0),
        activation,
        1,
        64,
    )
    .expect("line activation fiber initializes");
    executor
        .dialogues
        .begin(ActiveDialogue {
            activation: activation_id.clone(),
            content: AwbcContentUnitId(0),
            target: fixture_dialogue_target(),
            target_type: AwbcTypeId(0),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.defer")
                .expect("line identity"),
            captures: Box::new([]),
            values: Box::new([]),
            effect_callbacks: Box::new([]),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(0),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
            phase: ProductDialoguePhase::Activating {
                fiber: activation_fiber,
                pending: None,
            },
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
        })
        .expect("line activation begins");
    let mut transaction = executor
        .dialogues
        .begin_transaction(&activation_id)
        .expect("line activation transaction");
    let mut pure_backend = crate::pure::VmRuntimePureCallBackend::default();

    executor
        .step_dialogue_activation(&mut transaction, &mut pure_backend)
        .expect("line activation materializes the capture");
    executor
        .step_dialogue_activation(&mut transaction, &mut pure_backend)
        .expect("line-root defer registration executes");

    let ProductDialoguePhase::Activating { fiber, .. } = &transaction.frame().phase else {
        panic!("activation remains in its running phase");
    };
    assert_eq!(fiber.cursor.instruction_offset, 2);
    assert_eq!(
        fiber.active_frame().unwrap().registers[0],
        Some(RuntimeValue::String("captured at defer".to_owned()))
    );
    let [registration] = transaction.line().deferred_registrations() else {
        panic!("one reached statement creates one line-root registration");
    };
    assert_eq!(registration.site().index(), 0);
    assert_eq!(
        registration.outcome_filter(),
        crate::line_task::RuntimeDeferOutcomeFilter::Always
    );
    assert_eq!(
        registration.captures(),
        [RuntimeValue::String("captured at defer".to_owned())]
    );
}

#[test]
fn line_root_defer_children_run_lifo_filter_outcomes_and_resume_host_calls() {
    let program = defer_host_call_program();
    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64)
        .expect("deferred host-call executor starts");
    let target_owner = executor
        .program
        .opaque_owner(AwbcTypeId(2))
        .expect("dialogue target owner resolves")
        .expect("dialogue target is opaque");
    let activation = crate::runtime_id::DialogueActivationId::new(
        executor.artifact_fingerprint,
        executor.facade_fiber.persistent_id,
        crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
            std::num::NonZeroU32::MIN,
        ),
        0,
    );
    executor
        .dialogues
        .begin(ActiveDialogue {
            activation: activation.clone(),
            content: AwbcContentUnitId(0),
            target: crate::value::RuntimeOpaqueValue::new_exact(&target_owner, RuntimeValue::Unit),
            target_type: AwbcTypeId(2),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.defer.lifo")
                .expect("line identity"),
            captures: Box::new([]),
            values: Box::new([]),
            effect_callbacks: Box::new([]),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(0),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
            phase: ProductDialoguePhase::Activating {
                fiber: crate::awbc::fiber::FiberState::for_function(
                    &executor.program,
                    AwbcEntryId(0),
                    AwbcFunctionId(1),
                    1,
                    64,
                )
                .expect("line activation fiber initializes"),
                pending: None,
            },
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
        })
        .expect("dialogue activation begins");

    let mut transaction = executor
        .dialogues
        .begin_transaction(&activation)
        .expect("activation transaction");
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    for _ in 0..6 {
        executor
            .step_dialogue_activation(&mut transaction, &mut backend)
            .expect("activation reaches one defer instruction");
    }
    assert_eq!(transaction.line().deferred_registrations().len(), 3);
    executor
        .dialogues
        .commit(transaction)
        .expect("registrations commit");

    fn prepare_next(
        executor: &mut AwbcProductStepExecutor,
        activation: &crate::runtime_id::DialogueActivationId,
    ) -> (
        bool,
        Option<(
            crate::runtime_id::RuntimeDeferRegistrationId,
            crate::runtime_id::RuntimeDeferSiteId,
        )>,
    ) {
        let mut transaction = executor
            .dialogues
            .begin_transaction(activation)
            .expect("deferred activation transaction");
        let mut batch = super::ProductLineTaskExecutionBatch {
            child_fibers: executor.child_fibers.clone(),
            dialogue_effect_callback_activations: executor
                .dialogue_effect_callback_activations
                .clone(),
            next_generation: executor.next_generation,
            next_fiber_instance: executor.next_fiber_instance,
            observations: Vec::new(),
            pure_stats: None,
        };
        let progressed = executor
            .prepare_next_deferred_child(
                &mut transaction,
                crate::line_task::ScopeExit::Completed,
                &mut batch,
            )
            .expect("next defer transition prepares");
        let inflight = transaction.line().deferred_inflight();
        let receipt = executor
            .dialogues
            .commit(transaction)
            .expect("deferred transition commits");
        let mut output = crate::step::RuntimeStepOutput::default();
        executor.commit_line_task_commands(batch, &mut output);
        assert!(output.requests.line_commands.is_empty());
        assert!(receipt.into_line().into_commands().is_empty());
        (progressed, inflight)
    }

    fn run_deferred_host_call(
        executor: &mut AwbcProductStepExecutor,
        activation: &crate::runtime_id::DialogueActivationId,
        backend: &mut crate::pure::VmRuntimePureCallBackend,
        expected_capture: &str,
    ) {
        let mut output = crate::step::RuntimeStepOutput::default();
        assert!(executor.step_next_child(
            &mut output,
            backend,
            &RuntimeStepInput::default(),
            &[],
            &[],
        ));
        let [request] = output.requests.host_calls.as_slice() else {
            panic!("deferred body emits its captured host call before suspending");
        };
        assert_eq!(
            request.args,
            [RuntimePayload(RuntimeValue::String(
                expected_capture.to_owned()
            ))]
        );
        let mut unpaired = executor.snapshot();
        unpaired.child_fibers[0].owner = super::AwbcProductChildFiberOwnerSnapshot::Independent;
        let mut rejected = AwbcProductStepExecutor::for_entry_arc(
            std::sync::Arc::clone(&executor.program),
            AwbcEntryId(0),
            64,
        )
        .expect("unpaired restore candidate starts");
        assert!(rejected.restore_snapshot(unpaired).is_err());

        let save = AwbcProductExecutorSaveSnapshot::from_live(&executor.snapshot())
            .expect("inflight defer saves with its child packet");
        let owner =
            crate::task::RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&executor.program));
        let restored_snapshot = save
            .into_live_for_program(&owner)
            .expect("inflight defer snapshot decodes");
        let mut restored = AwbcProductStepExecutor::for_entry_arc(
            std::sync::Arc::clone(&executor.program),
            AwbcEntryId(0),
            64,
        )
        .expect("paired restore candidate starts");
        restored
            .restore_snapshot(restored_snapshot)
            .expect("inflight defer restores only with its exact child owner");
        *executor = restored;

        let result = RuntimeHostCallResult {
            id: request.id.clone(),
            outcome: Ok(RuntimePayload(RuntimeValue::Unit)),
        };
        let mut output = crate::step::RuntimeStepOutput::default();
        assert!(executor.step_next_child(
            &mut output,
            backend,
            &RuntimeStepInput {
                host_call_results: vec![result],
                ..RuntimeStepInput::default()
            },
            &[],
            &[],
        ));
        assert!(output.requests.host_calls.is_empty());
        assert!(executor.child_fibers.is_empty());
        let transaction = executor
            .dialogues
            .begin_transaction(activation)
            .expect("completed child activation transaction");
        assert_eq!(transaction.line().deferred_inflight(), None);
    }

    let (progressed, inflight) = prepare_next(&mut executor, &activation);
    assert!(progressed);
    assert_eq!(inflight.map(|(_, site)| site.index()), Some(2));
    run_deferred_host_call(&mut executor, &activation, &mut backend, "last");

    let (progressed, inflight) = prepare_next(&mut executor, &activation);
    assert!(progressed);
    assert_eq!(
        inflight, None,
        "the Failed-only middle registration is skipped"
    );

    let (progressed, inflight) = prepare_next(&mut executor, &activation);
    assert!(progressed);
    assert_eq!(inflight.map(|(_, site)| site.index()), Some(0));
    run_deferred_host_call(&mut executor, &activation, &mut backend, "first");

    let (progressed, inflight) = prepare_next(&mut executor, &activation);
    assert!(!progressed);
    assert_eq!(inflight, None);
    let transaction = executor
        .dialogues
        .begin_transaction(&activation)
        .expect("drained activation transaction");
    assert!(transaction.line().deferred_registrations().is_empty());
    assert_eq!(transaction.line().deferred_inflight(), None);
}

#[test]
fn product_dialogue_failure_commits_abandoned_before_trapping_parent() {
    let mut executor = AwbcProductStepExecutor::for_entry(
        return_program(),
        crate::awbc::schema::AwbcEntryId(0),
        64,
    )
    .expect("product executor starts");
    let content = crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
        std::num::NonZeroU32::MIN,
    );
    let activation = crate::runtime_id::DialogueActivationId::new(
        executor.artifact_fingerprint,
        executor.facade_fiber.persistent_id,
        content,
        0,
    );
    executor
        .dialogues
        .begin(ActiveDialogue {
            activation: activation.clone(),
            content: AwbcContentUnitId(0),
            target: fixture_dialogue_target(),
            target_type: AwbcTypeId(0),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.fixture")
                .expect("fixture line identity"),
            captures: Box::new([]),
            values: Box::new([]),
            effect_callbacks: Box::new([]),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(0),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
            phase: ProductDialoguePhase::Activating {
                fiber: executor.fiber.clone(),
                pending: None,
            },
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
        })
        .expect("fixture activation begins");
    let transaction = executor
        .dialogues
        .begin_transaction(&activation)
        .expect("fixture activation transaction");
    let mut output = crate::step::RuntimeStepOutput::default();

    assert!(executor.begin_product_dialogue_failure(
        transaction,
        ProductStepError::Internal("fixture activation failure".to_owned()),
        &mut output,
    ));

    assert!(executor.dialogues.active_frame().is_none());
    assert_eq!(executor.fiber.status, FiberStatus::Trapped);
    assert!(matches!(
        executor.fiber.terminal,
        Some(FiberTerminalValue::Trapped(ref trap))
            if trap.message.as_deref() == Some("fixture activation failure")
    ));
    assert!(output.requests.line_commands.is_empty());
    assert_eq!(output.diagnostics.len(), 1);
}

#[test]
fn product_dialogue_failure_cancels_joined_child_before_abandoning() {
    let mut executor = AwbcProductStepExecutor::for_entry(
        return_program(),
        crate::awbc::schema::AwbcEntryId(0),
        64,
    )
    .expect("product executor starts");
    let program = std::sync::Arc::make_mut(&mut executor.program);
    program.content_units.push(AwbcContentUnit {
        public_id: AwbcStringId(0),
        template: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
            .expect("template identity"),
        marks: Vec::new(),
        effect_site_count: 0,
        line_task_group: Some(crate::awbc::schema::AwbcLineTaskGroupId(0)),
        display: None,
        source: None,
        resources: Vec::new(),
    });
    program
        .line_task_nodes
        .push(crate::awbc::schema::AwbcLineTaskNode::Action(
            AwbcFunctionId(0),
        ));
    program
        .line_task_groups
        .push(crate::awbc::schema::AwbcLineTaskGroup {
            captures: Vec::new(),
            activation: AwbcFunctionId(0),
            result_type: AwbcTypeId(0),
            handle_sites: Vec::new(),
            root: crate::awbc::schema::AwbcLineTaskNodeId(0),
            nodes: AwbcTableRange::new(0, 1),
            cancel_handlers: Vec::new(),
            cleanup_completed: None,
            cleanup_cancelled: None,
            cleanup_failed: None,
            cleanup: crate::awbc::schema::AwbcLineCleanupPolicy {
                child_tasks: crate::awbc::schema::AwbcChildCleanup::CancelAndJoin,
                presentation: crate::awbc::schema::AwbcPresentationCleanup::DropRegistered,
                audio: crate::awbc::schema::AwbcAudioCleanup::StopRegistered,
            },
        });
    let content = crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
        std::num::NonZeroU32::MIN,
    );
    let activation = crate::runtime_id::DialogueActivationId::new(
        executor.artifact_fingerprint,
        executor.facade_fiber.persistent_id,
        content,
        1,
    );
    let (live, tag, policy) = {
        let view = executor
            .line_task_view(AwbcContentUnitId(0))
            .expect("line-task view");
        let mut live = crate::line_task::LineTaskLiveState::new(&view, activation.clone());
        let activation_batch = crate::line_task::progress_live_line_task_group(
            &view,
            crate::time::LogicalDuration::default(),
            crate::line_task::LineTaskReadyEvents::new(&std::collections::BTreeSet::new()),
            &mut live,
        )
        .expect("activate action");
        let [crate::line_task::LineTaskCommand::Run { tag, policy }] =
            activation_batch.commands.as_slice()
        else {
            panic!("fixture must start one joined child")
        };
        (live, tag.clone(), *policy)
    };
    executor
        .dialogues
        .begin(ActiveDialogue {
            activation: activation.clone(),
            content: AwbcContentUnitId(0),
            target: fixture_dialogue_target(),
            target_type: AwbcTypeId(0),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.fixture")
                .expect("line"),
            captures: Box::new([]),
            values: Box::new([]),
            effect_callbacks: Box::new([]),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(0),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
            phase: ProductDialoguePhase::Reducing { line_task: live },
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
        })
        .expect("activation begins");
    executor.child_fibers.push_back(ProductChildFiber {
        owner: ProductChildFiberOwner::LineTask {
            content: AwbcContentUnitId(0),
            tag,
            policy,
            phase: ProductLineTaskFiberPhase::Active,
        },
        fiber: executor.fiber.clone(),
        pending_host_call: None,
    });
    let transaction = executor
        .dialogues
        .begin_transaction(&activation)
        .expect("failure transaction");
    let mut output = crate::step::RuntimeStepOutput::default();

    assert!(executor.begin_product_dialogue_failure(
        transaction,
        ProductStepError::Internal("joined child failure".to_owned()),
        &mut output,
    ));
    assert!(executor.child_fibers.is_empty());
    assert!(executor.dialogues.active_frame().is_none());
    assert_eq!(executor.fiber.status, FiberStatus::Trapped);
}

#[test]
fn save_snapshot_preserves_queued_progress_publications() {
    let mut executor = AwbcProductStepExecutor::for_entry(return_program(), AwbcEntryId(0), 64)
        .expect("product executor starts");
    let event = TaskEvent {
        logical_epoch: LogicalEpoch(7),
        task_id: TaskId("task.snapshot".to_owned()),
        sequence: TaskSequence(3),
        kind: TaskEventKind::Progress(Progress::new(0.25).expect("fixture Progress is valid")),
    };
    executor.latch_task_events(std::slice::from_ref(&event));

    let saved = AwbcProductExecutorSaveSnapshot::from_live(&executor.snapshot())
        .expect("queued Progress snapshots");
    let restored = saved
        .into_live_for_program(&crate::task::RuntimeProgramOwner::Awbc(
            std::sync::Arc::clone(&executor.program),
        ))
        .expect("queued Progress restores");

    assert_eq!(
        restored.queued_task_events,
        std::collections::VecDeque::from([event])
    );
}

#[test]
fn snapshot_restore_and_hot_swap_require_exact_semantic_flow_identity() {
    let program = return_program();
    let original = program.flow_bindings[0].flow.clone();
    let replacement =
        crate::plan::FlowRuntimeId::from_checked_declaration_digest([0x52; 32], "flow.main")
            .expect("replacement Flow identity is valid");
    let mut executor = AwbcProductStepExecutor::for_entry(program.clone(), AwbcEntryId(0), 64)
        .expect("product executor starts");
    let snapshot = executor.snapshot();

    let mut replacement_program = program;
    replacement_program.flow_bindings[0].flow = replacement.clone();
    replacement_program.flow_executables[0].metadata.flow = replacement.clone();
    let error = executor
        .replace_program_preserving_state(replacement_program)
        .expect_err("same-label declaration replacement must not preserve live state");
    assert!(matches!(
        error,
        AwbcProductStepBuildError::RestoreSnapshot { ref message }
            if message.contains("no longer owns AWBC function 0")
    ));
    assert_eq!(executor.program.flow_bindings[0].flow, original);

    let mut tampered = snapshot;
    tampered.live_flow_bindings[0].flow = replacement;
    let error = executor
        .restore_snapshot(tampered)
        .expect_err("snapshot identity must not be inferred from a matching public label");
    assert!(matches!(
        error,
        AwbcProductStepBuildError::RestoreSnapshot { ref message }
            if message.contains("no longer owns AWBC function 0")
    ));
}

#[test]
fn snapshot_restore_rejects_same_label_choice_target_substitution() {
    let first =
        crate::plan::FlowRuntimeId::from_checked_declaration_digest([0x61; 32], "flow.main")
            .expect("first Flow identity");
    let second =
        crate::plan::FlowRuntimeId::from_checked_declaration_digest([0x62; 32], "flow.main")
            .expect("second Flow identity");
    let mut program = return_program();
    program.flow_bindings[0].flow = first.clone();
    program.flow_executables[0].metadata.flow = first.clone();
    let mut second_function = program.functions[0].clone();
    second_function.blocks = AwbcTableRange::new(1, 1);
    second_function.entry_block = AwbcBlockId(1);
    program.functions.push(second_function);
    let mut second_block = program.blocks[0].clone();
    second_block.owner = AwbcFunctionId(1);
    program.blocks.push(second_block);
    program.flow_bindings.push(AwbcFlowBinding {
        flow: second.clone(),
        function: AwbcFunctionId(1),
    });
    let label = AwbcStringId(u32::try_from(program.strings.len()).expect("test string index"));
    program.strings.push("zz.continue".to_owned());
    program.choices.push(AwbcChoice {
        public_id: None,
        options: AwbcTableRange::new(0, 1),
    });
    program.choice_options.push(AwbcChoiceOption {
        public_id: None,
        label,
        condition: None,
        target: Some(AwbcFunctionId(0)),
        out_effect: None,
        effects: Vec::new(),
    });
    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64)
        .expect("choice snapshot executor starts");
    let option = executor.choice_runtime_option(&executor.program.choice_options[0]);
    executor.active_choice = Some(ActiveChoice {
        choice: AwbcChoiceId(0),
        public_id: None,
        options: vec![option],
        option_indices: vec![0],
    });
    let mut snapshot = executor.snapshot();
    snapshot
        .live_flow_bindings
        .push(executor.program.flow_bindings[1].clone());
    snapshot
        .active_choice
        .as_mut()
        .expect("active choice")
        .option_indices[0] = 1;

    let error = executor
        .restore_snapshot(snapshot)
        .expect_err("same-label target substitution must not restore");
    assert!(matches!(
        error,
        AwbcProductStepBuildError::RestoreSnapshot { ref message }
            if message.contains("does not match its exact typed source option")
    ));
    assert_eq!(
        executor
            .active_choice
            .as_ref()
            .and_then(|choice| choice.options[0].target.as_ref()),
        Some(&first)
    );
}

fn return_program() -> AwbcProgram {
    let mut program = trap_program(AwbcTrapCode::InternalInvariant, "unused");
    program.blocks[0].terminator = AwbcTerminator::Return { value: None };
    program
}

fn defer_host_call_program() -> AwbcProgram {
    let mut program = return_program();
    program.strings = vec![
        "entry.main".to_owned(),
        "defer.record".to_owned(),
        "test".to_owned(),
        "append".to_owned(),
        "first".to_owned(),
        "middle".to_owned(),
        "last".to_owned(),
        "std.character_dialogue".to_owned(),
    ];
    program.runtime_types = vec![
        AwbcRuntimeType::unit(),
        AwbcRuntimeType::new(
            crate::pattern::RuntimeCheckedType::String.semantic_identity_digest(),
            AwbcRuntimeTypeShape::String,
        ),
        AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([0x24; 32]),
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(7),
                admission: crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: crate::value::RuntimeOpaqueValueClass::Plain,
                persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                arguments: Vec::new(),
            },
        ),
    ];
    program.constants = vec![
        AwbcConstant::String(AwbcStringId(4)),
        AwbcConstant::String(AwbcStringId(5)),
        AwbcConstant::String(AwbcStringId(6)),
        AwbcConstant::Unit,
    ];
    program.signatures.extend([
        AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: vec![AwbcTypeId(1)],
            result: Some(AwbcTypeId(0)),
            effects: AwbcEffectSetId(0),
        },
    ]);
    program.frame_layouts.extend([
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
            ],
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::ReturnValue,
                    scope_depth: 0,
                },
            ],
            max_scope_depth: 0,
        },
    ]);
    program.host_calls = vec![AwbcHostCall {
        public_id: AwbcStringId(1),
        capability: AwbcStringId(2),
        operation: AwbcStringId(3),
        contract: None,
        signature: AwbcSignatureId(2),
        mode: AwbcHostCallMode::Suspend,
        deterministic: true,
        arguments: vec![crate::awbc::schema::AwbcHostArgument {
            name: None,
            spread: false,
        }],
    }];
    program.resume_points = vec![AwbcResumePoint {
        function: AwbcFunctionId(2),
        block: AwbcBlockId(3),
        frame_layout: AwbcFrameLayoutId(2),
        kind: AwbcSafePointKind::HostCall,
    }];
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::RegisterDefer {
            site: crate::runtime_id::RuntimeDeferSiteId::from_zero_based(0)
                .expect("first defer site"),
            outcome: crate::line_task::RuntimeDeferOutcomeFilter::Always,
            owner: crate::awbc::schema::AwbcDeferOwner::LineRoot,
            captures: vec![AwbcRegisterId(0)],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::RegisterDefer {
            site: crate::runtime_id::RuntimeDeferSiteId::from_zero_based(1)
                .expect("middle defer site"),
            outcome: crate::line_task::RuntimeDeferOutcomeFilter::Failed,
            owner: crate::awbc::schema::AwbcDeferOwner::LineRoot,
            captures: vec![AwbcRegisterId(1)],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(2),
        },
        AwbcInstruction::RegisterDefer {
            site: crate::runtime_id::RuntimeDeferSiteId::from_zero_based(2)
                .expect("last defer site"),
            outcome: crate::line_task::RuntimeDeferOutcomeFilter::Always,
            owner: crate::awbc::schema::AwbcDeferOwner::LineRoot,
            captures: vec![AwbcRegisterId(2)],
        },
    ];
    program.functions.extend([
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::LineActivation,
            signature: AwbcSignatureId(1),
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, 1),
            entry_block: AwbcBlockId(1),
            flags: AwbcFunctionFlags::default(),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(2),
            frame_layout: AwbcFrameLayoutId(2),
            blocks: AwbcTableRange::new(2, 2),
            entry_block: AwbcBlockId(2),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::MaySuspend),
        },
    ]);
    program.blocks.extend([
        AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: AwbcTableRange::new(0, 6),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(2),
            instructions: AwbcTableRange::new(6, 0),
            terminator: AwbcTerminator::HostCall {
                call: AwbcHostCallId(0),
                args: vec![AwbcRegisterId(0)],
                dst: Some(AwbcRegisterId(1)),
                resume: AwbcResumePointId(0),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(2),
            instructions: AwbcTableRange::new(6, 0),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(1)),
            },
            safe_point: AwbcSafePointKind::Return,
            source_map: None,
        },
    ]);
    program.defer_sites = vec![AwbcFunctionId(2); 3];
    let template = crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
        .expect("template identity");
    program.content_templates = vec![AwbcDialogueContentTemplate {
        id: template,
        digest: crate::entry::RuntimeDialogueContentTemplateDigest::ZERO,
        slots: Vec::new(),
        effects: Vec::new(),
    }];
    program.content_units = vec![AwbcContentUnit {
        public_id: AwbcStringId(0),
        template,
        marks: Vec::new(),
        effect_site_count: 0,
        line_task_group: Some(crate::awbc::schema::AwbcLineTaskGroupId(0)),
        display: None,
        source: None,
        resources: Vec::new(),
    }];
    program.line_task_nodes = vec![crate::awbc::schema::AwbcLineTaskNode::Sequence(Vec::new())];
    program.line_task_groups = vec![crate::awbc::schema::AwbcLineTaskGroup {
        captures: Vec::new(),
        activation: AwbcFunctionId(1),
        result_type: AwbcTypeId(0),
        handle_sites: Vec::new(),
        root: crate::awbc::schema::AwbcLineTaskNodeId(0),
        nodes: AwbcTableRange::new(0, 1),
        cancel_handlers: Vec::new(),
        cleanup_completed: None,
        cleanup_cancelled: None,
        cleanup_failed: None,
        cleanup: crate::awbc::schema::AwbcLineCleanupPolicy {
            child_tasks: crate::awbc::schema::AwbcChildCleanup::CancelAndJoin,
            presentation: crate::awbc::schema::AwbcPresentationCleanup::DropRegistered,
            audio: crate::awbc::schema::AwbcAudioCleanup::StopRegistered,
        },
    }];
    program.canonicalize_string_table();
    program
        .verify(Default::default(), Default::default())
        .expect("deferred host-call program verifies");
    program
}

fn test_flow_binding() -> AwbcFlowBinding {
    AwbcFlowBinding {
        flow: crate::plan::FlowRuntimeId::from_checked_declaration_digest([0x51; 32], "flow.main")
            .expect("test Flow identity is valid"),
        function: AwbcFunctionId(0),
    }
}

fn test_flow_executable() -> AwbcFlowExecutable {
    AwbcFlowExecutable {
        metadata: RuntimeFlowExecutable {
            flow: test_flow_binding().flow,
            contract: FlowContractHash::from_bytes([0x5a; 32]),
            controller: None,
        },
        function: AwbcFunctionId(0),
    }
}

#[test]
fn host_call_request_and_result_resume_at_runtime_step_boundary() {
    let mut executor = AwbcProductStepExecutor::for_entry(host_call_program(), AwbcEntryId(0), 64)
        .expect("host-call product executor starts");

    let first = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert_eq!(first.output.requests.host_calls.len(), 1);
    let request = &first.output.requests.host_calls[0];
    assert_eq!(request.id, RuntimeHostCallId("host.probe".to_owned()));
    assert_eq!(request.public_id, "host.probe");
    assert_eq!(request.capability, "probe");
    assert_eq!(request.operation, "read");
    assert_eq!(request.args, Vec::<RuntimePayload>::new());
    assert_eq!(request.mode, RuntimeHostCallMode::Suspend);
    assert!(request.deterministic);
    assert_eq!(first.stop_reason, RuntimeStepStopReason::Output);

    let second = executor.step(
        RuntimeStepInput {
            host_call_results: vec![RuntimeHostCallResult {
                id: request.id.clone(),
                outcome: Ok(RuntimePayload(RuntimeValue::String("host-ok".to_owned()))),
            }],
            ..RuntimeStepInput::default()
        },
        RuntimeStepOptions::default(),
    );

    assert!(second.output.requests.host_calls.is_empty());
    assert_eq!(second.fiber_status, FlowFiberStatus::Running);

    let third = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
    assert!(
        third
            .output
            .flow_events
            .contains(&crate::plan::FlowEvent::Return {
                value: "host-ok".to_owned()
            })
    );
    assert_eq!(
        third.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return("host-ok".to_owned()))
    );
}

#[test]
fn raw_host_result_must_match_pending_signature_even_when_discarded() {
    let mut program = host_call_program();
    let AwbcTerminator::HostCall { dst, .. } = &mut program.blocks[0].terminator else {
        panic!("host-call fixture terminator");
    };
    *dst = None;
    program.signatures[0].result = None;
    program.blocks[1].terminator = AwbcTerminator::Return { value: None };
    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64)
        .expect("host-call product executor starts");
    let first = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
    let id = first.output.requests.host_calls[0].id.clone();
    let second = executor.step(
        RuntimeStepInput {
            host_call_results: vec![RuntimeHostCallResult {
                id,
                outcome: Ok(RuntimePayload(RuntimeValue::Bool(true))),
            }],
            ..RuntimeStepInput::default()
        },
        RuntimeStepOptions::default(),
    );
    assert!(matches!(second.fiber_status, FlowFiberStatus::Failed(_)));
    assert!(!second.output.diagnostics.is_empty());
    assert!(second.output.flow_events.is_empty());
}

#[test]
fn ready_direct_need_returns_its_payload_unchanged_in_the_same_step() {
    let expected = RuntimeValue::String("profile-ready".to_owned());
    let (mut executor, input) = direct_need_executor_and_input(vec![runtime_need_state(
        0,
        Need::Ready(RuntimePayload(RuntimeValue::String(
            "profile-ready".to_owned(),
        ))),
    )]);

    let result = executor.step(input, direct_need_step_options());

    assert_eq!(
        executor.fiber.terminal,
        Some(FiberTerminalValue::Returned(Some(expected)))
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
    assert_eq!(result.stats.need_states_in, 1);
    assert!(result.output.requests.tasks.is_empty());
    assert!(result.output.flow_events.iter().all(|event| !matches!(
        event,
        crate::plan::FlowEvent::AwaitStarted { .. }
            | crate::plan::FlowEvent::AwaitReady { .. }
            | crate::plan::FlowEvent::AwaitProgress { .. }
    )));
}

#[test]
fn ready_result_error_payload_is_resumed_without_trapping() {
    let expected = RuntimeValue::result_err(RuntimeValue::String("profile-error".to_owned()));
    let (mut executor, input) = direct_need_executor_and_input(vec![runtime_need_state(
        0,
        Need::Ready(RuntimePayload(expected.clone())),
    )]);

    let result = executor.step(input, direct_need_step_options());

    assert_eq!(
        executor.fiber.terminal,
        Some(FiberTerminalValue::Returned(Some(expected)))
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
    assert!(result.output.diagnostics.is_empty());
    assert!(result.output.requests.tasks.is_empty());
}

#[test]
fn unresolved_direct_need_blocks_without_inventing_a_task_request() {
    let unresolved = [
        Need::NotStarted,
        Need::Pending(Progress::new(0.5).expect("fixture progress is valid")),
    ];
    for state in unresolved {
        let (mut executor, input) =
            direct_need_executor_and_input(vec![runtime_need_state(0, state)]);

        let result = executor.step(input, direct_need_step_options());

        assert_eq!(
            result.fiber_status,
            FlowFiberStatus::NeedWaiting(NeedId("need.profile".to_owned()))
        );
        assert_eq!(result.stop_reason, RuntimeStepStopReason::Blocked);
        assert!(result.output.requests.tasks.is_empty());
        assert!(result.output.flow_events.is_empty());
    }
}

#[test]
fn cancelled_direct_need_unwinds_to_a_terminal_fiber() {
    let (mut executor, input) =
        direct_need_executor_and_input(vec![runtime_need_state(0, Need::Cancelled)]);

    let result = executor.step(input, direct_need_step_options());

    assert_eq!(executor.fiber.terminal, Some(FiberTerminalValue::Cancelled));
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
    assert!(result.output.requests.tasks.is_empty());
    assert!(result.output.diagnostics.is_empty());
}

#[test]
fn direct_need_uses_the_first_terminal_sequence() {
    let expected = RuntimeValue::String("first".to_owned());
    let states = vec![
        runtime_need_state(
            2,
            Need::Ready(RuntimePayload(RuntimeValue::String("late".to_owned()))),
        ),
        runtime_need_state(0, Need::NotStarted),
        runtime_need_state(
            1,
            Need::Ready(RuntimePayload(RuntimeValue::String("first".to_owned()))),
        ),
    ];
    let (mut executor, input) = direct_need_executor_and_input(states);

    let result = executor.step(input, direct_need_step_options());

    assert_eq!(
        executor.fiber.terminal,
        Some(FiberTerminalValue::Returned(Some(expected)))
    );
    assert_eq!(result.stats.need_states_in, 3);
    assert!(result.output.requests.tasks.is_empty());
}

#[test]
fn ensure_content_instruction_projects_typed_content_request() {
    let mut executor =
        AwbcProductStepExecutor::for_entry(content_ensure_program(), AwbcEntryId(0), 64)
            .expect("content product executor starts");

    let first = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert_eq!(first.output.requests.ensure_content.len(), 1);
    assert_eq!(
        first.output.requests.ensure_content[0].content,
        "line.content"
    );
    assert!(first.output.requests.ensure_content[0].resources.is_empty());
    assert_eq!(first.stats.executed_ops, 1);

    let second = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(second.output.requests.ensure_content.is_empty());
    assert_eq!(second.stop_reason, RuntimeStepStopReason::Done);
}

#[test]
fn trap_terminators_project_typed_runtime_diagnostics() {
    let cases = [
        (AwbcTrapCode::TypeMismatch, RuntimeDiagnosticCategory::Type),
        (
            AwbcTrapCode::PatternMismatch,
            RuntimeDiagnosticCategory::Pattern,
        ),
        (
            AwbcTrapCode::HostAbiMismatch,
            RuntimeDiagnosticCategory::Host,
        ),
        (
            AwbcTrapCode::CapabilityDenied,
            RuntimeDiagnosticCategory::Capability,
        ),
        (
            AwbcTrapCode::DivisionByZero,
            RuntimeDiagnosticCategory::Runtime,
        ),
        (
            AwbcTrapCode::InvalidIndex,
            RuntimeDiagnosticCategory::Runtime,
        ),
        (
            AwbcTrapCode::MissingDynamicTarget,
            RuntimeDiagnosticCategory::Runtime,
        ),
        (
            AwbcTrapCode::ExplicitPanic,
            RuntimeDiagnosticCategory::Runtime,
        ),
        (
            AwbcTrapCode::UninitializedRegister,
            RuntimeDiagnosticCategory::Runtime,
        ),
        (
            AwbcTrapCode::InternalInvariant,
            RuntimeDiagnosticCategory::Internal,
        ),
    ];

    for (code, category) in cases {
        let message = format!("typed trap {code:?}");
        let mut executor =
            AwbcProductStepExecutor::for_entry(trap_program(code, &message), AwbcEntryId(0), 64)
                .expect("trap product executor starts");

        let result = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

        assert_eq!(result.stop_reason, RuntimeStepStopReason::Failed);
        assert_eq!(result.stats.diagnostics, 1);
        assert_eq!(result.output.diagnostics.len(), 1);
        assert_eq!(result.output.diagnostics[0].category, category);
        assert_eq!(result.output.diagnostics[0].message, message);
        assert_eq!(
            result.fiber_status,
            FlowFiberStatus::Failed(message.clone())
        );
    }
}

#[test]
fn effect_mapping_table_covers_every_awbc_effect_kind() {
    let mut program = AwbcProgram::default();
    let cases = [
        (AwbcEffectKind::Wait, true),
        (AwbcEffectKind::Audio, false),
        (AwbcEffectKind::Call, true),
        (AwbcEffectKind::Log, true),
        (AwbcEffectKind::SignalWrite, true),
        (AwbcEffectKind::MetricWrite, true),
        (AwbcEffectKind::EmitEvent, true),
        (AwbcEffectKind::Out, true),
        (AwbcEffectKind::Return, true),
        (AwbcEffectKind::Goto, true),
        (AwbcEffectKind::Panic, true),
        (AwbcEffectKind::Fail, true),
        (AwbcEffectKind::Bail, true),
        (AwbcEffectKind::Ensure, true),
        (AwbcEffectKind::Assert, true),
        (AwbcEffectKind::Close, true),
        (AwbcEffectKind::Select, true),
        (AwbcEffectKind::Break, true),
        (AwbcEffectKind::Continue, true),
    ];

    for (kind, should_map_to_line_effect) in cases {
        let effect = push_effect_plan(&mut program, kind);
        let mapped = kind.map_product_effect(&program, effect, &[]);
        assert_eq!(
            matches!(mapped, MappedEffect::Line(_)),
            should_map_to_line_effect
        );
        assert_eq!(
            matches!(mapped, MappedEffect::Unsupported(_)),
            !should_map_to_line_effect
        );
    }
}

#[test]
fn wait_effect_mapping_accepts_only_a_typed_duration_target() {
    let mut program = AwbcProgram::default();
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Wait);

    let mapped = AwbcEffectKind::Wait.map_product_effect(&program, effect, &[]);
    assert!(matches!(
        mapped,
        MappedEffect::Line(LineEffectRequest::Wait(
            crate::effect::RuntimeWaitTarget::Duration(duration)
        )) if duration.as_nanos() == 5
    ));

    let non_duration = constant_string(&mut program, ".checkpoint");
    program.effect_plans[effect.index()].static_args[0] = non_duration;
    let mapped = AwbcEffectKind::Wait.map_product_effect(&program, effect, &[]);
    let MappedEffect::Unsupported(diagnostic) = mapped else {
        panic!("a non-Duration wait target must not become a runtime wait request");
    };
    assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Type);
    assert_eq!(
        diagnostic.message,
        "AWBC wait target must evaluate to Duration"
    );

    program.effect_plans[effect.index()].static_args.clear();
    let mapped = AwbcEffectKind::Wait.map_product_effect(&program, effect, &[]);
    let MappedEffect::Unsupported(diagnostic) = mapped else {
        panic!("a missing wait target must not become an empty runtime expression");
    };
    assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Internal);
    assert_eq!(
        diagnostic.message,
        "AWBC wait effect is missing its Duration target"
    );
}

#[test]
fn assertion_effect_mapping_retains_typed_guard_and_payload() {
    let mut program = AwbcProgram::default();
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Assert);
    let message = constant_string(&mut program, "must be ready");
    program.effect_plans[effect.index()].static_args[2] = message;

    let mapped =
        AwbcEffectKind::Assert.map_product_effect(&program, effect, &[RuntimeValue::Bool(false)]);
    let MappedEffect::Line(LineEffectRequest::Assert(assertion)) = mapped else {
        panic!("well-formed assertion effect must map to typed core assertion data");
    };
    assert_eq!(
        assertion.guard(),
        RuntimeAssertionGuardId::try_from_bytes([7; 16]).expect("fixture guard")
    );
    assert_eq!(assertion.condition(), "false");
    assert_eq!(assertion.message(), "must be ready");
    assert_eq!(assertion.profile(), RuntimeAssertionProfile::Always);
}

#[test]
fn pre_materialized_assertion_effect_retains_its_condition_label() {
    let mut program = AwbcProgram::default();
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Assert);
    let condition = constant_string(&mut program, "debug_flag");
    program.effect_plans[effect.index()].static_args[1] = condition;

    let mapped = AwbcEffectKind::Assert.map_product_effect(&program, effect, &[]);
    let MappedEffect::Line(LineEffectRequest::Assert(assertion)) = mapped else {
        panic!("pre-materialized assertion request must remain a typed line effect");
    };
    assert_eq!(assertion.condition(), "debug_flag");
    assert_eq!(assertion.message(), "arg2");
}

#[test]
fn assertion_effect_mapping_omits_true_condition() {
    let mut program = AwbcProgram::default();
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Assert);

    let mapped = AwbcEffectKind::Assert.map_product_effect(
        &program,
        effect,
        &[
            RuntimeValue::Bool(true),
            RuntimeValue::String("must be ready".to_owned()),
        ],
    );

    assert!(matches!(mapped, MappedEffect::Omitted));
}

#[test]
fn assertion_effect_mapping_rejects_non_bool_condition() {
    let mut program = AwbcProgram::default();
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Assert);

    let mapped = AwbcEffectKind::Assert.map_product_effect(
        &program,
        effect,
        &[
            RuntimeValue::String("false".to_owned()),
            RuntimeValue::String("must be ready".to_owned()),
        ],
    );

    let MappedEffect::Unsupported(diagnostic) = mapped else {
        panic!("non-Bool assertion conditions must be rejected before label materialization");
    };
    assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Type);
    assert_eq!(
        diagnostic.message,
        "AWBC assertion condition must evaluate to Bool"
    );
}

#[test]
fn assertion_effect_mapping_rejects_unknown_profile_without_fallback() {
    let mut program = AwbcProgram::default();
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Assert);
    let invalid_profile = constant_string(&mut program, "legacy_profile");
    program.effect_plans[effect.index()].static_args[3] = invalid_profile;

    let mapped = AwbcEffectKind::Assert.map_product_effect(&program, effect, &[]);

    let MappedEffect::Unsupported(diagnostic) = mapped else {
        panic!("unknown assertion profiles must not fall back to the always profile");
    };
    assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Internal);
    assert_eq!(diagnostic.message, "malformed AWBC assertion profile");
}

#[test]
fn audio_effect_mapping_without_typed_payload_is_typed_internal_diagnostic() {
    let mut program = AwbcProgram::default();
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Audio);

    let mapped = AwbcEffectKind::Audio.map_product_effect(&program, effect, &[]);

    let MappedEffect::Unsupported(diagnostic) = mapped else {
        panic!("malformed audio payload must not map to a line or audio request");
    };
    assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Internal);
}

#[test]
fn audio_effect_mapping_missing_arg_is_typed_internal_diagnostic() {
    let mut program = AwbcProgram::default();
    program.audio_commands.push(AwbcAudioCommand::StopAll {
        fade_out_millis: AwbcAudioValueRef::Arg(AwbcAudioArg::new(0)),
    });
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Audio);
    program.effect_plans[effect.index()].audio = Some(AwbcAudioCommandId(0));
    program.effect_plans[effect.index()].static_args.clear();

    let mapped = AwbcEffectKind::Audio.map_product_effect(&program, effect, &[]);

    let MappedEffect::Unsupported(diagnostic) = mapped else {
        panic!("missing audio dynamic arg must not map to a request");
    };
    assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Internal);
}

#[test]
fn audio_effect_mapping_invalid_identifier_is_typed_type_diagnostic() {
    let mut program = AwbcProgram::default();
    let invalid_voice = constant_string(&mut program, "   ");
    program.audio_commands.push(AwbcAudioCommand::Stop {
        voice: AwbcAudioValueRef::Const(invalid_voice),
        fade_out_millis: AwbcAudioValueRef::Arg(AwbcAudioArg::new(0)),
    });
    let effect = push_effect_plan(&mut program, AwbcEffectKind::Audio);
    program.effect_plans[effect.index()].audio = Some(AwbcAudioCommandId(0));
    program.effect_plans[effect.index()].static_args.clear();

    let mapped =
        AwbcEffectKind::Audio.map_product_effect(&program, effect, &[RuntimeValue::u32(25)]);

    let MappedEffect::Unsupported(diagnostic) = mapped else {
        panic!("invalid audio identifier must not map to a request");
    };
    assert_eq!(diagnostic.category, RuntimeDiagnosticCategory::Type);
}

fn trap_program(code: AwbcTrapCode, message: &str) -> AwbcProgram {
    let strings = vec!["entry.main".to_owned(), message.to_owned()];
    let signature = AwbcSignature {
        params: Vec::new(),
        result: None,
        effects: AwbcEffectSetId(0),
    };
    AwbcProgram {
        strings,
        signatures: vec![signature],
        frame_layouts: vec![AwbcFrameLayout {
            scopes: Vec::new(),
            slots: Vec::new(),
            max_scope_depth: 0,
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Trap {
                code,
                message: Some(AwbcStringId(1)),
            },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        }],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::default(),
        }],
        flow_bindings: vec![test_flow_binding()],
        flow_executables: vec![test_flow_executable()],
        entries: vec![crate::awbc::schema::AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: crate::awbc::schema::AwbcEntryKind::Cli,
            target: crate::awbc::schema::AwbcEntryTarget::Function {
                function: AwbcFunctionId(0),
            },
            roles: crate::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    }
}

fn content_ensure_program() -> AwbcProgram {
    let strings = vec!["entry.main".to_owned(), "line.content".to_owned()];
    let signature = AwbcSignature {
        params: Vec::new(),
        result: None,
        effects: AwbcEffectSetId(0),
    };
    AwbcProgram {
        strings,
        signatures: vec![signature],
        frame_layouts: vec![AwbcFrameLayout {
            scopes: Vec::new(),
            slots: Vec::new(),
            max_scope_depth: 0,
        }],
        content_templates: vec![AwbcDialogueContentTemplate {
            id: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                .expect("template identity"),
            digest: crate::entry::RuntimeDialogueContentTemplateDigest::ZERO,
            slots: Vec::new(),
            effects: Vec::new(),
        }],
        content_units: vec![AwbcContentUnit {
            public_id: AwbcStringId(1),
            template: crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                .expect("template identity"),
            marks: Vec::new(),
            effect_site_count: 0,
            line_task_group: None,
            display: None,
            source: None,
            resources: Vec::new(),
        }],
        instructions: vec![AwbcInstruction::EnsureContent {
            content: crate::awbc::schema::AwbcContentUnitId(0),
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 1),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        }],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::default(),
        }],
        flow_bindings: vec![test_flow_binding()],
        flow_executables: vec![test_flow_executable()],
        entries: vec![crate::awbc::schema::AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: crate::awbc::schema::AwbcEntryKind::Cli,
            target: crate::awbc::schema::AwbcEntryTarget::Function {
                function: AwbcFunctionId(0),
            },
            roles: crate::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    }
}

fn host_call_program() -> AwbcProgram {
    let strings = vec![
        "entry.main".to_owned(),
        "host.probe".to_owned(),
        "probe".to_owned(),
        "read".to_owned(),
    ];
    let signature = AwbcSignature {
        params: Vec::new(),
        result: Some(AwbcTypeId(1)),
        effects: AwbcEffectSetId(0),
    };
    let frame_layout = AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(1),
            role: AwbcFrameSlotRole::ReturnValue,
            scope_depth: 0,
        }],
        max_scope_depth: 0,
    };
    AwbcProgram {
        strings,
        runtime_types: vec![
            AwbcRuntimeType::unit(),
            AwbcRuntimeType::new(
                crate::pattern::RuntimeCheckedType::String.semantic_identity_digest(),
                AwbcRuntimeTypeShape::String,
            ),
        ],
        signatures: vec![signature],
        frame_layouts: vec![frame_layout],
        host_calls: vec![AwbcHostCall {
            public_id: AwbcStringId(1),
            capability: AwbcStringId(2),
            operation: AwbcStringId(3),
            contract: None,
            signature: AwbcSignatureId(0),
            mode: AwbcHostCallMode::Suspend,
            deterministic: true,
            arguments: Vec::new(),
        }],
        resume_points: vec![AwbcResumePoint {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(1),
            frame_layout: AwbcFrameLayoutId(0),
            kind: AwbcSafePointKind::HostCall,
        }],
        blocks: vec![
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::HostCall {
                    call: AwbcHostCallId(0),
                    args: Vec::new(),
                    dst: Some(AwbcRegisterId(0)),
                    resume: AwbcResumePointId(0),
                },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            },
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return {
                    value: Some(AwbcRegisterId(0)),
                },
                safe_point: AwbcSafePointKind::Return,
                source_map: None,
            },
        ],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 2),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::MaySuspend),
        }],
        flow_bindings: vec![test_flow_binding()],
        flow_executables: vec![test_flow_executable()],
        entries: vec![crate::awbc::schema::AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: crate::awbc::schema::AwbcEntryKind::Cli,
            target: crate::awbc::schema::AwbcEntryTarget::Function {
                function: AwbcFunctionId(0),
            },
            roles: crate::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    }
}

fn direct_need_executor_and_input(
    need_states: Vec<RuntimeNeedState>,
) -> (AwbcProductStepExecutor, RuntimeStepInput) {
    let executor = AwbcProductStepExecutor::for_function_invocation(
        direct_need_program(),
        AwbcEntryId(0),
        AwbcFunctionId(0),
        [RuntimeFlowParameterBinding {
            parameter: crate::entry::FlowParameterCoordinate::from_position(0),
            value: RuntimeValue::String("need.profile".to_owned()),
        }],
        64,
    )
    .expect("direct Need product executor starts");
    let input = RuntimeStepInput {
        need_states,
        ..RuntimeStepInput::default()
    };
    (executor, input)
}

fn direct_need_step_options() -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: crate::step::RuntimeStepMode::Drain,
        budget: crate::step::RuntimeStepBudget { max_ops: 64 },
    }
}

fn runtime_need_state(sequence: u64, state: Need<RuntimePayload>) -> RuntimeNeedState {
    RuntimeNeedState::new(
        LogicalEpoch(7),
        NeedId("need.profile".to_owned()),
        TaskSequence(sequence),
        state,
    )
}

fn direct_need_program() -> AwbcProgram {
    let need_ty = AwbcTypeId(0);
    let dynamic_ty = AwbcTypeId(1);
    AwbcProgram {
        strings: vec!["entry.main".to_owned(), "need".to_owned()],
        runtime_types: vec![
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([91; 32]),
                AwbcRuntimeTypeShape::Need(AwbcTypeId(1)),
            ),
            AwbcRuntimeType::dynamic(),
        ],
        signatures: vec![AwbcSignature {
            params: vec![need_ty],
            result: Some(dynamic_ty),
            effects: AwbcEffectSetId(0),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![
                AwbcFrameSlot {
                    name: Some(AwbcStringId(1)),
                    ty: need_ty,
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: dynamic_ty,
                    role: AwbcFrameSlotRole::ReturnValue,
                    scope_depth: 0,
                },
            ],
            max_scope_depth: 0,
        }],
        patterns: vec![AwbcPattern::Bind {
            target: AwbcRegisterId(1),
            mutable: false,
            expected: Some(dynamic_ty),
        }],
        resume_points: vec![AwbcResumePoint {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(1),
            frame_layout: AwbcFrameLayoutId(0),
            kind: AwbcSafePointKind::Await,
        }],
        blocks: vec![
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Await {
                    handle: AwbcRegisterId(0),
                    binding: Some(AwbcPatternId(0)),
                    observer: None,
                    resume: AwbcResumePointId(0),
                },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            },
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return {
                    value: Some(AwbcRegisterId(1)),
                },
                safe_point: AwbcSafePointKind::Return,
                source_map: None,
            },
        ],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 2),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::empty()
                .with(AwbcFunctionFlag::Deterministic)
                .with(AwbcFunctionFlag::MaySuspend),
        }],
        flow_bindings: vec![test_flow_binding()],
        flow_executables: vec![test_flow_executable()],
        entries: vec![crate::awbc::schema::AwbcEntry {
            runtime_id: crate::plan::EntryRuntimeId::canonical("main")
                .expect("test entry runtime ID is valid"),
            binding: crate::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: crate::awbc::schema::AwbcEntryKind::Cli,
            target: crate::awbc::schema::AwbcEntryTarget::Function {
                function: AwbcFunctionId(0),
            },
            roles: crate::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    }
}

fn push_effect_plan(program: &mut AwbcProgram, kind: AwbcEffectKind) -> AwbcEffectPlanId {
    let static_arg_count = match kind {
        AwbcEffectKind::Audio => 0,
        AwbcEffectKind::Call
        | AwbcEffectKind::SignalWrite
        | AwbcEffectKind::MetricWrite
        | AwbcEffectKind::Out
        | AwbcEffectKind::Ensure
        | AwbcEffectKind::Break => 2,
        AwbcEffectKind::Log | AwbcEffectKind::Assert => 4,
        AwbcEffectKind::EmitEvent => 3,
        AwbcEffectKind::Wait
        | AwbcEffectKind::Return
        | AwbcEffectKind::Goto
        | AwbcEffectKind::Panic
        | AwbcEffectKind::Fail
        | AwbcEffectKind::Bail
        | AwbcEffectKind::Close
        | AwbcEffectKind::Select
        | AwbcEffectKind::Continue => 1,
    };
    let static_args = (0..static_arg_count)
        .map(|index| {
            if kind == AwbcEffectKind::Wait && index == 0 {
                return constant_duration(program, 5);
            }
            if kind == AwbcEffectKind::Assert && index == 0 {
                return constant_bytes(program, &[7; 16]);
            }
            if kind == AwbcEffectKind::Assert && index == 1 {
                return constant_bool(program, false);
            }
            let value = if kind == AwbcEffectKind::Assert && index == 3 {
                "always".to_owned()
            } else {
                format!("arg{index}")
            };
            constant_string(program, &value)
        })
        .collect();
    let effect = AwbcEffectPlanId(
        u32::try_from(program.effect_plans.len()).expect("test effect table index fits u32"),
    );
    program.effect_plans.push(AwbcEffectPlan {
        kind,
        signature: AwbcSignatureId(0),
        capability: None,
        audio: None,
        static_args,
        resources: Vec::new(),
    });
    effect
}

fn constant_duration(program: &mut AwbcProgram, nanos: u64) -> crate::awbc::schema::AwbcConstantId {
    let id = crate::awbc::schema::AwbcConstantId(
        u32::try_from(program.constants.len()).expect("test constant index fits u32"),
    );
    program.constants.push(AwbcConstant::DurationNanos(nanos));
    id
}

fn constant_bytes(program: &mut AwbcProgram, value: &[u8]) -> crate::awbc::schema::AwbcConstantId {
    let id = crate::awbc::schema::AwbcConstantId(
        u32::try_from(program.constants.len()).expect("test constant index fits u32"),
    );
    program
        .constants
        .push(crate::awbc::schema::AwbcConstant::Bytes(value.to_vec()));
    id
}

fn constant_bool(program: &mut AwbcProgram, value: bool) -> crate::awbc::schema::AwbcConstantId {
    let id = crate::awbc::schema::AwbcConstantId(
        u32::try_from(program.constants.len()).expect("test constant index fits u32"),
    );
    program.constants.push(AwbcConstant::Bool(value));
    id
}

fn constant_string(program: &mut AwbcProgram, value: &str) -> crate::awbc::schema::AwbcConstantId {
    let string =
        AwbcStringId(u32::try_from(program.strings.len()).expect("test string index fits u32"));
    program.strings.push(value.to_owned());
    let constant = crate::awbc::schema::AwbcConstantId(
        u32::try_from(program.constants.len()).expect("test constant index fits u32"),
    );
    program.constants.push(AwbcConstant::String(string));
    constant
}
