use super::{
    ActiveChoice, ActiveDialogue, AwbcProductExecutorSaveSnapshot, AwbcProductStepBuildError,
    AwbcProductStepExecutor, ProductChildFiber, ProductChildFiberOwner, ProductDialoguePhase,
    ProductLineTaskFiberPhase, ProductStepError,
};
use crate::awbc::fiber::{AwbcFiberStateSnapshot, FiberState, FiberStatus, FiberTerminalValue};
use crate::awbc::product_step::mapping::MappedEffect;
use crate::awbc::schema::{
    AwbcAudioArg, AwbcAudioCommand, AwbcAudioCommandId, AwbcAudioValueRef, AwbcBlock, AwbcBlockId,
    AwbcChoice, AwbcChoiceId, AwbcChoiceOption, AwbcConstant, AwbcConstantId, AwbcContentUnit,
    AwbcContentUnitId, AwbcDialogueContentTemplate, AwbcEffectKind, AwbcEffectPlan,
    AwbcEffectPlanId, AwbcEffectSetId, AwbcEntryId, AwbcFlowBinding, AwbcFlowExecutable,
    AwbcFrameLayout, AwbcFrameLayoutId, AwbcFrameSlot, AwbcFrameSlotRole, AwbcFunction,
    AwbcFunctionFlag, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionInputOwnership,
    AwbcFunctionKind, AwbcHostArgument, AwbcHostCall, AwbcHostCallId, AwbcHostCallMode,
    AwbcInstruction, AwbcLineCancelHandler, AwbcPattern, AwbcPatternId, AwbcProgram,
    AwbcRegisterId, AwbcResumePoint, AwbcResumePointId, AwbcRuntimeType, AwbcRuntimeTypeShape,
    AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcStringId, AwbcTableRange, AwbcTaskClass,
    AwbcTaskPlan, AwbcTaskPlanKind, AwbcTaskPolicy, AwbcTaskRequestProjection,
    AwbcTaskRestartPolicy, AwbcTerminator, AwbcTrapCode, AwbcTypeId,
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
    GenerationId, LogicalEpoch, NeedId, RuntimeNeedState, TaskEvent, TaskEventKind, TaskId,
    TaskPublicationRevision, TaskSequence,
};
use crate::value::{RuntimeFlowParameterBinding, RuntimePayload, RuntimeValue};
use arcweft_need::{Need, Progress};
use std::collections::BTreeMap;

fn product_snapshot(
    executor: &AwbcProductStepExecutor,
) -> crate::awbc::product_step::AwbcProductExecutorSnapshot {
    executor
        .snapshot()
        .expect("fixture Product snapshot is valid")
}

/// These line-task fixtures need a second test fiber from a root fixture whose
/// entire runtime-value graph is unrestricted. Keep the test-only copy proof
/// explicit and reconstruct through the inert snapshot representation.
fn copyable_fixture_fiber(executor: &AwbcProductStepExecutor) -> FiberState {
    executor
        .fiber
        .visit_runtime_values(|value| {
            assert!(
                value.ownership().permits_copy(),
                "fixture fiber must be unrestricted before test reconstruction"
            );
            Ok::<_, std::convert::Infallible>(())
        })
        .expect("copyable test fiber visits cleanly");
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    AwbcFiberStateSnapshot::from_live(&executor.fiber)
        .expect("copyable fixture fiber has an inert snapshot")
        .into_live_for_program(&owner)
        .expect("copyable fixture fiber restores")
}

fn commit_test_dialogue_transaction(
    executor: &mut AwbcProductStepExecutor,
    transaction: super::dialogue::ProductDialogueTransaction,
) -> crate::line_task::RuntimeDialogueRegistryCommitReceipt {
    let proof = executor
        .dialogues
        .inspect_commit(&transaction)
        .expect("fixture dialogue transaction is committable");
    executor.dialogues.commit_prepared(transaction, proof)
}

#[path = "tests/context.rs"]
mod context;

#[test]
fn detached_program_result_rejection_retains_awbc_value_and_saved_owner() {
    let fixture = crate::tests::program_custody::issued_program_handle();
    let mut executor = AwbcProductStepExecutor::for_root_arc_with_context_proof(
        crate::tests::program_custody::awbc_handle_program(fixture.program),
        crate::awbc::fiber::AwbcFiberRoot::Program(fixture.program),
        64,
        GenerationId::new(0),
        None,
    )
    .unwrap();
    executor.fiber.status = FiberStatus::Returned;
    executor.fiber.frames.clear();
    let destination = crate::value::ownership::RuntimeOwnedSlotId::ProgramResult {
        execution: executor.facade_fiber.execution,
        fiber: crate::runtime_id::RuntimePersistentFiberId::from_allocated(
            executor.fiber.instance.get().get(),
        ),
    };
    let (value, custody) = crate::tests::program_custody::publish_program_handle(
        fixture.ledger,
        fixture.value,
        destination,
    );
    executor.dialogues = super::dialogue::ProductDialogueStore::from_published(custody);
    executor.fiber.return_summary = Some(crate::value::runtime_value_label(&value));
    executor.fiber.terminal = Some(FiberTerminalValue::Returned(Some(value)));
    assert!(executor.take_program_result().is_err());
    assert!(executor.take_program_result().is_err());
    let saved = executor.inert_rollback_image().unwrap().product;
    let executor = executor
        .restore_inert_snapshot_owned(saved.clone())
        .unwrap();
    let Some(FiberTerminalValue::Returned(Some(value))) = &executor.fiber.terminal else {
        panic!("rejected export must keep its live result")
    };
    assert_eq!(
        value.affine_line_handles().unwrap()[0].token(),
        &fixture.token
    );
    assert_eq!(executor.inert_rollback_image().unwrap().product, saved);
}

#[test]
fn empty_fiber_restore_has_no_executable_cursor_or_return_owner() {
    let executor = AwbcProductStepExecutor::for_root_arc_with_context_proof(
        std::sync::Arc::new(AwbcProgram::default()),
        crate::awbc::fiber::AwbcFiberRoot::Empty,
        64,
        GenerationId::new(7),
        None,
    )
    .expect("empty program admits an inert terminal executor");
    executor
        .fiber
        .validate_for_program(&executor.program)
        .unwrap();
    let saved = executor.inert_rollback_image().unwrap().product;
    let executor = executor
        .restore_inert_snapshot_owned(saved.clone())
        .unwrap();
    executor
        .fiber
        .validate_for_program(&executor.program)
        .unwrap();

    let mut forged = saved;
    forged.fiber.status = FiberStatus::Returned;
    forged.fiber.terminal = Some(crate::awbc::fiber::AwbcFiberTerminalSnapshot::Returned(
        Some(crate::value::AwbcRuntimeValueSnapshot::Bool(true)),
    ));
    let (executor, _) = executor.restore_inert_snapshot_owned(forged).unwrap_err();
    assert_eq!(
        executor.fiber.root,
        crate::awbc::fiber::AwbcFiberRoot::Empty
    );
    executor
        .fiber
        .validate_for_program(&executor.program)
        .unwrap();
}

fn fixture_dialogue_target() -> crate::value::RuntimeOpaqueValue {
    let owner = crate::pattern::RuntimeOpaqueTypeOwner::exact_with(
        crate::value::RuntimeCharacterDialogueProducerId::get(),
        RuntimeSemanticTypeId::from_bytes([0x24; 32]),
        crate::value::RuntimeOpaqueValueClass::Plain,
        crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
    );
    crate::value::RuntimeOpaqueValue::new_exact(&owner, RuntimeValue::Unit)
}

fn fixture_bundle_image_handle() -> RuntimeValue {
    let artifact = crate::value::RuntimeBundleAssetArtifactDigest::try_from_bytes([0x32; 32])
        .expect("fixture artifact digest is nonzero");
    let context = crate::value::RuntimeBundleAssetContext::new(GenerationId::new(0), artifact);
    let resource = crate::value::RuntimeBundleAssetResourceId::try_new("asset.bg.room")
        .expect("fixture asset identity is canonical");
    let content = crate::value::RuntimeAssetContentDigest::try_for_bytes(b"image")
        .expect("fixture content digest fits");
    let binding = crate::value::RuntimeBundleAssetBinding::try_new(context, resource, content)
        .expect("fixture asset binding is valid");
    crate::value::RuntimeImageHandleValue::from_binding(binding)
        .into_runtime_value()
        .expect("fixture ImageHandle is standard-owned")
}

#[test]
fn product_snapshot_visitor_finds_asset_handles_in_dialogue_captures() {
    let mut executor = AwbcProductStepExecutor::for_entry(
        return_program(),
        crate::awbc::schema::AwbcEntryId(0),
        64,
    )
    .expect("fixture Product executor starts");
    let content_id = AwbcContentUnitId(0);
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
            activation,
            content: content_id,
            target: fixture_dialogue_target(),
            target_type: AwbcTypeId(0),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.asset.capture")
                .expect("fixture line identity"),
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(0),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
            captures: Box::new([RuntimeValue::Tuple(vec![fixture_bundle_image_handle()])]),
            task_inputs: Box::new([]),
            values: Box::new([]),
            effect_callbacks: BTreeMap::new(),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            phase: ProductDialoguePhase::Activating {
                fiber: copyable_fixture_fiber(&executor),
                pending: None,
            },
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
            pending_activation_host_call: None,
        })
        .expect("dialogue capture remains owned by Product state");
    let mut roles = Vec::new();

    executor
        .visit_live_runtime_values(|value| {
            if let Some(role) = crate::value::runtime_bundle_asset_opaque_role(value) {
                roles.push(role);
            }
            Ok::<(), std::convert::Infallible>(())
        })
        .expect("Product snapshot visits every runtime value");

    assert_eq!(
        roles,
        vec![crate::value::RuntimeBundleAssetOpaqueRole::ImageHandle]
    );
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
fn only_explicit_selector_terminal_selects_dialogue_result_for_its_owner() {
    let action = arcweft_interaction_model::input::InputActionId::new("dialogue.cancel.result")
        .expect("valid cancellation action");

    for (terminal, selected) in [
        (
            FiberTerminalValue::DialogueResultSelected(RuntimeValue::String(
                "selected-result".to_owned(),
            )),
            true,
        ),
        (
            FiberTerminalValue::Returned(Some(RuntimeValue::String("implicit-return".to_owned()))),
            false,
        ),
    ] {
        let mut executor = AwbcProductStepExecutor::for_entry(
            mark_selector_program(action.clone()),
            crate::awbc::schema::AwbcEntryId(0),
            64,
        )
        .expect("product executor starts");
        let content_id = AwbcContentUnitId(0);
        let content = crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
            std::num::NonZeroU32::MIN,
        );
        let activation = crate::runtime_id::DialogueActivationId::new(
            executor.artifact_fingerprint,
            executor.facade_fiber.persistent_id,
            content,
            0,
        );
        let view = executor
            .line_task_view(content_id)
            .expect("line-task group view");
        let mut reducer = crate::line_task::LineTaskLiveState::new(&view, activation.clone());
        let no_marks = std::collections::BTreeSet::new();
        crate::line_task::progress_live_line_task_group(
            &view,
            crate::time::LogicalDuration::default(),
            crate::line_task::LineTaskReadyEvents::new(&no_marks),
            &mut reducer,
        )
        .expect("arm line task before cancellation");
        let actions = [action.clone()];
        let cancellation = crate::line_task::cancel_live_line_task_group(
            &view,
            crate::line_task::LineTaskReadyEvents::new(&std::collections::BTreeSet::new())
                .with_input_actions(&actions),
            &mut reducer,
        )
        .expect("matching cancellation action begins close");
        let tag = cancellation
            .1
            .commands
            .iter()
            .find_map(|command| match command {
                crate::line_task::LineTaskCommand::Run { tag, .. } => Some(tag.clone()),
                crate::line_task::LineTaskCommand::Cancel { .. } => None,
            })
            .unwrap_or_else(|| {
                panic!(
                    "cancellation handler is scheduled as joined work: {:?}",
                    cancellation.1.commands
                )
            });
        executor
            .dialogues
            .begin(ActiveDialogue {
                activation: activation.clone(),
                content: content_id,
                target: fixture_dialogue_target(),
                target_type: AwbcTypeId(2),
                line: crate::plan::RuntimeLineId::from_runtime_line_value("line.cancel.result")
                    .expect("line identity"),
                captures: Box::new([]),
                task_inputs: Box::new([]),
                values: Box::new([]),
                effect_callbacks: BTreeMap::new(),
                voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
                result: crate::awbc::schema::AwbcDialogueResultTarget {
                    ty: AwbcTypeId(1),
                    pattern: AwbcPatternId(0),
                    destination: AwbcRegisterId(0),
                },
                phase: ProductDialoguePhase::Reducing { line_task: reducer },
                elapsed_nanos: 0,
                pending_content_events: Vec::new(),
                pending_advance: false,
                pending_line_outcomes: Vec::new(),
                pending_activation_host_call: None,
            })
            .expect("active dialogue begins");
        let mut transaction = executor
            .dialogues
            .begin_transaction(&activation)
            .expect("active dialogue transaction");
        transaction
            .line_mut()
            .commit_result(
                AwbcTypeId(1),
                RuntimeValue::String("committed-result".to_owned()),
            )
            .expect("normal dialogue result commits");
        let mut child = copyable_fixture_fiber(&executor);
        child.status = FiberStatus::Returned;
        child.terminal = Some(terminal);
        let expected_tag = tag.clone();
        let batch = super::ProductLineTaskExecutionBatch {
            child_fibers: std::collections::VecDeque::new(),
            existing_child_actions: BTreeMap::new(),
            line_task_activations: Vec::new(),
            line_task_baseline: None,
            line_task_reserved_runs: Vec::new(),
            dialogue_effect_callback_activations: std::collections::BTreeSet::new(),
            next_generation: executor.next_generation,
            next_fiber_instance: executor.next_fiber_instance,
            observations: Vec::new(),
            pure_stats: None,
        };

        executor
            .prepare_owned_line_task_completion(
                &mut transaction,
                content_id,
                tag,
                &mut child,
                false,
                true,
                true,
                batch,
            )
            .expect("joined cancellation work completes");

        match (selected, transaction.line().result()) {
            (
                true,
                crate::line_task::RuntimeDialogueResultState::Selected {
                    ty,
                    value: RuntimeValue::String(value),
                    source,
                },
            ) => {
                assert_eq!(*ty, AwbcTypeId(1));
                assert_eq!(source, &expected_tag);
                assert_eq!(value, "selected-result");
            }
            (
                false,
                crate::line_task::RuntimeDialogueResultState::Committed {
                    ty,
                    value: RuntimeValue::String(value),
                },
            ) => {
                assert_eq!(*ty, AwbcTypeId(1));
                assert_eq!(value, "committed-result");
            }
            other => panic!("unexpected result disposition: {other:?}"),
        }
    }
}

#[test]
fn mark_action_selection_restores_and_cancellation_can_override_it() {
    let cancellation_action =
        arcweft_interaction_model::input::InputActionId::new("dialogue.cancel.mark")
            .expect("valid cancellation action");
    let mark = crate::runtime_id::RuntimeDialogueMarkId::from_zero_based(0).expect("mark identity");
    let mut executor = AwbcProductStepExecutor::for_entry(
        mark_selector_program(cancellation_action.clone()),
        crate::awbc::schema::AwbcEntryId(0),
        64,
    )
    .expect("product executor starts");
    let content = AwbcContentUnitId(0);
    let activation = crate::runtime_id::DialogueActivationId::new(
        executor.artifact_fingerprint,
        executor.facade_fiber.persistent_id,
        crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
            std::num::NonZeroU32::MIN,
        ),
        0,
    );
    let view = executor
        .line_task_view(content)
        .expect("mark line-task view");
    let mut reducer = crate::line_task::LineTaskLiveState::new(&view, activation.clone());
    let no_marks = std::collections::BTreeSet::new();
    crate::line_task::progress_live_line_task_group(
        &view,
        crate::time::LogicalDuration::default(),
        crate::line_task::LineTaskReadyEvents::new(&no_marks),
        &mut reducer,
    )
    .expect("arm mark child");
    let accepted = reducer
        .accept_content_event_kinds(
            &[crate::step::RuntimeDialogueContentEventKind::Mark(mark)],
            |_| true,
        )
        .expect("consume exact mark");
    let activation_commands = crate::line_task::progress_live_line_task_group(
        &view,
        crate::time::LogicalDuration::default(),
        accepted.ready(),
        &mut reducer,
    )
    .expect("start mark action");
    let mark_tag = activation_commands
        .commands
        .into_iter()
        .find_map(|command| match command {
            crate::line_task::LineTaskCommand::Run { tag, .. } => Some(tag),
            crate::line_task::LineTaskCommand::Cancel { .. } => None,
        })
        .expect("joined mark action tag");

    executor
        .dialogues
        .begin(ActiveDialogue {
            activation: activation.clone(),
            content,
            target: fixture_dialogue_target(),
            target_type: AwbcTypeId(2),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.mark.result")
                .expect("line identity"),
            captures: Box::new([]),
            task_inputs: Box::new([]),
            values: Box::new([]),
            effect_callbacks: BTreeMap::new(),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(1),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
            phase: ProductDialoguePhase::Reducing { line_task: reducer },
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
            pending_activation_host_call: None,
        })
        .expect("active mark dialogue begins");
    let mut transaction = executor
        .dialogues
        .begin_transaction(&activation)
        .expect("active dialogue transaction");
    let mut child = copyable_fixture_fiber(&executor);
    child.status = FiberStatus::Returned;
    child.terminal = Some(FiberTerminalValue::DialogueResultSelected(
        RuntimeValue::String("selected-result".to_owned()),
    ));
    let batch = super::ProductLineTaskExecutionBatch {
        child_fibers: std::collections::VecDeque::new(),
        existing_child_actions: BTreeMap::new(),
        line_task_activations: Vec::new(),
        line_task_baseline: None,
        line_task_reserved_runs: Vec::new(),
        dialogue_effect_callback_activations: std::collections::BTreeSet::new(),
        next_generation: executor.next_generation,
        next_fiber_instance: executor.next_fiber_instance,
        observations: Vec::new(),
        pure_stats: None,
    };
    executor
        .prepare_owned_line_task_completion(
            &mut transaction,
            content,
            mark_tag.clone(),
            &mut child,
            false,
            false,
            true,
            batch,
        )
        .expect("mark action selects its dialogue result");
    assert!(matches!(
        transaction.line().result(),
        crate::line_task::RuntimeDialogueResultState::Selected {
            ty: AwbcTypeId(1),
            value: RuntimeValue::String(value),
            source,
        } if source == &mark_tag && value == "selected-result"
    ));

    commit_test_dialogue_transaction(&mut executor, transaction);
    let snapshot = product_snapshot(&executor);
    let mut restored = AwbcProductStepExecutor::for_entry_arc(
        std::sync::Arc::clone(&executor.program),
        crate::awbc::schema::AwbcEntryId(0),
        64,
    )
    .expect("restore candidate starts against the same AWBC program");
    restored
        .restore_snapshot(snapshot)
        .expect("product snapshot admits the completed AWBC Mark source");
    assert!(matches!(
        restored.dialogues.active_line().expect("restored line").result(),
        crate::line_task::RuntimeDialogueResultState::Selected {
            ty: AwbcTypeId(1),
            value: RuntimeValue::String(value),
            source,
        } if value == "selected-result" && source == &mark_tag
    ));

    let actions = [cancellation_action.clone()];
    let no_marks = std::collections::BTreeSet::new();
    let mut transaction = restored
        .dialogues
        .begin_transaction(&activation)
        .expect("restored mark selection transaction");
    let view = restored
        .line_task_view(content)
        .expect("restored AWBC Mark graph view");
    let cancellation = crate::line_task::cancel_live_line_task_group(
        &view,
        crate::line_task::LineTaskReadyEvents::new(&no_marks).with_input_actions(&actions),
        transaction
            .frame_mut()
            .line_task_mut()
            .expect("live reducer"),
    )
    .expect("cancellation begins after mark selection");
    let cancel_tag = cancellation
        .1
        .commands
        .into_iter()
        .find_map(|command| match command {
            crate::line_task::LineTaskCommand::Run { tag, .. } => Some(tag),
            crate::line_task::LineTaskCommand::Cancel { .. } => None,
        })
        .expect("cancellation handler tag");
    let mut child = copyable_fixture_fiber(&restored);
    child.status = FiberStatus::Returned;
    child.terminal = Some(FiberTerminalValue::DialogueResultSelected(
        RuntimeValue::String("cancelled-result".to_owned()),
    ));
    let batch = super::ProductLineTaskExecutionBatch {
        child_fibers: std::collections::VecDeque::new(),
        existing_child_actions: BTreeMap::new(),
        line_task_activations: Vec::new(),
        line_task_baseline: None,
        line_task_reserved_runs: Vec::new(),
        dialogue_effect_callback_activations: std::collections::BTreeSet::new(),
        next_generation: restored.next_generation,
        next_fiber_instance: restored.next_fiber_instance,
        observations: Vec::new(),
        pure_stats: None,
    };
    restored
        .prepare_owned_line_task_completion(
            &mut transaction,
            content,
            cancel_tag.clone(),
            &mut child,
            false,
            true,
            true,
            batch,
        )
        .expect("cancellation selector supersedes pending mark result");
    assert!(matches!(
        transaction.line().result(),
        crate::line_task::RuntimeDialogueResultState::Selected {
            ty: AwbcTypeId(1),
            value: RuntimeValue::String(value),
            source,
        } if source == &cancel_tag && value == "cancelled-result"
    ));
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
            input_ownership: vec![
                AwbcFunctionInputOwnership::default();
                program.signatures[AwbcSignatureId(1).index()].params.len()
            ],
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, 1),
            entry_block: AwbcBlockId(1),
            flags: AwbcFunctionFlags::default(),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(2),
            input_ownership: vec![
                AwbcFunctionInputOwnership::default();
                program.signatures[AwbcSignatureId(2).index()].params.len()
            ],
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
        crate::awbc::fiber::AwbcFiberRoot::Function(activation),
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
            task_inputs: Box::new([]),
            values: Box::new([]),
            effect_callbacks: BTreeMap::new(),
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
            pending_activation_host_call: None,
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
        fiber.active_frame().unwrap().registers[0].as_ref().cloned(),
        None,
        "a reached RegisterDefer transfers its capture out of the source register"
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
fn init_out_unwinds_reached_scope_defer_before_reveal_and_skips_tail() {
    let program = init_scope_defer_host_call_program();
    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64)
        .expect("Init scope-defer executor starts");
    let activation = crate::runtime_id::DialogueActivationId::new(
        executor.artifact_fingerprint,
        executor.facade_fiber.persistent_id,
        crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
            std::num::NonZeroU32::MIN,
        ),
        0,
    );
    let mut activation_fiber = crate::awbc::fiber::FiberState::for_function(
        &executor.program,
        crate::awbc::fiber::AwbcFiberRoot::Function(AwbcFunctionId(1)),
        AwbcFunctionId(1),
        1,
        64,
    )
    .expect("Init activation fiber initializes");
    activation_fiber
        .active_frame_mut()
        .expect("Init activation frame")
        .bind_positional_arguments(
            &executor.program,
            &[RuntimeValue::String("before host result".to_owned())],
        )
        .expect("Init activation input seeds the typed destination register");
    let target_owner = executor
        .program
        .opaque_owner(AwbcTypeId(2))
        .expect("dialogue target owner resolves")
        .expect("dialogue target type is opaque");
    executor
        .dialogues
        .begin(ActiveDialogue {
            activation: activation.clone(),
            content: AwbcContentUnitId(0),
            target: crate::value::RuntimeOpaqueValue::new_exact(&target_owner, RuntimeValue::Unit),
            target_type: AwbcTypeId(2),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.init.out.defer")
                .expect("line identity"),
            captures: Box::new([RuntimeValue::String("before host result".to_owned())]),
            task_inputs: Box::new([]),
            values: Box::new([]),
            effect_callbacks: BTreeMap::new(),
            voice: crate::presentation::RuntimeDialogueVoiceState::Ready(
                crate::presentation::RuntimeVoiceSessionId::try_new("init.voice")
                    .expect("voice session identity"),
            ),
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
            pending_activation_host_call: None,
        })
        .expect("Init activation begins");

    fn activation_step(
        executor: &mut AwbcProductStepExecutor,
        activation: &crate::runtime_id::DialogueActivationId,
        mut host_results: Vec<RuntimeHostCallResult>,
        backend: &mut crate::pure::VmRuntimePureCallBackend,
    ) -> (
        crate::awbc::product_step::line::ProductActivationProgress,
        crate::step::RuntimeStepOutput,
    ) {
        let mut transaction = executor
            .dialogues
            .begin_transaction(activation)
            .expect("activation transaction");
        let cursor = match &transaction.frame().phase {
            ProductDialoguePhase::Activating { fiber, .. } => fiber.cursor,
            _ => panic!("activation transaction has left its running phase"),
        };
        let mut progress = executor
            .step_dialogue_activation_with_host_results(
                &mut transaction,
                &mut host_results,
                backend,
            )
            .unwrap_or_else(|error| panic!("activation step at {cursor:?} failed: {error:?}"));
        if let Some(ticket) = progress.host_result_take.take() {
            executor.commit_activation_host_result(&mut transaction, &mut host_results, ticket);
        }
        let receipt = commit_test_dialogue_transaction(executor, transaction);
        let mut output = crate::step::RuntimeStepOutput::default();
        if let Some(batch) = progress.execution.take() {
            executor.commit_line_task_commands(batch, &mut output);
        }
        output
            .requests
            .host_calls
            .extend(std::mem::take(&mut progress.host_calls));
        output
            .requests
            .line_commands
            .extend(receipt.into_line().into_commands());
        (progress, output)
    }

    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    let (host_progress, host_output) =
        activation_step(&mut executor, &activation, Vec::new(), &mut backend);
    assert_eq!(host_output.requests.host_calls.len(), 1);
    assert!(!host_progress.progressed);
    let host_id = host_output.requests.host_calls[0].id.clone();

    let snapshot = product_snapshot(&executor);
    let save = AwbcProductExecutorSaveSnapshot::from_live(&snapshot)
        .expect("suspended activation host call is saveable");
    let owner = crate::task::RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&executor.program));
    let restored_snapshot = save
        .into_live_for_program(&owner)
        .expect("suspended activation host call decodes");
    executor
        .restore_snapshot(restored_snapshot)
        .expect("activation host call restores with its suspended fiber");

    let (_, repeated_output) =
        activation_step(&mut executor, &activation, Vec::new(), &mut backend);
    assert_eq!(repeated_output.requests.host_calls[0].id, host_id);

    let (resumed, resumed_output) = activation_step(
        &mut executor,
        &activation,
        vec![RuntimeHostCallResult {
            id: host_id,
            outcome: Ok(RuntimePayload(RuntimeValue::String(
                "captured from Init".to_owned(),
            ))),
        }],
        &mut backend,
    );
    assert!(resumed.progressed);
    assert!(resumed_output.requests.host_calls.is_empty());

    activation_step(&mut executor, &activation, Vec::new(), &mut backend); // EnterScope
    activation_step(&mut executor, &activation, Vec::new(), &mut backend); // Copy effect field
    activation_step(&mut executor, &activation, Vec::new(), &mut backend); // Copy effect message
    let (effect_progress, effect_output) =
        activation_step(&mut executor, &activation, Vec::new(), &mut backend); // EvaluatedEffect
    assert!(effect_progress.progressed);
    assert_eq!(
        effect_output.effects.line,
        [crate::effect::LineEffectRequest::Log(
            crate::effect::RuntimeLog {
                level: "info".to_owned(),
                message: "captured from Init".to_owned(),
                fields: vec![crate::effect::RuntimeField {
                    name: "source".to_owned(),
                    value: "captured from Init".to_owned(),
                }],
            }
        )]
    );
    activation_step(&mut executor, &activation, Vec::new(), &mut backend); // VoiceHandle
    activation_step(&mut executor, &activation, Vec::new(), &mut backend); // RegisterDefer
    let (deferred_id, voice_token) = {
        let transaction = executor
            .dialogues
            .begin_transaction(&activation)
            .expect("registered scope transaction");
        let ProductDialoguePhase::Activating { fiber, .. } = &transaction.frame().phase else {
            panic!("Init remains in activation before out");
        };
        let scope = fiber
            .active_frame()
            .expect("activation frame")
            .scopes
            .last()
            .expect("lexical Init scope is active");
        let [registration] = scope.defers.as_slice() else {
            panic!("one reached CurrentScope statement registers one defer");
        };
        assert_eq!(registration.site.index(), 0);
        assert_eq!(
            registration.captures[0],
            RuntimeValue::String("captured from Init".to_owned())
        );
        let voice_value = &registration.captures[1];
        assert!(matches!(voice_value, RuntimeValue::Opaque(_)));
        let token = crate::line_task::RuntimeLineHandleLedger::token_from_value(voice_value)
            .expect("captured voice handle token");
        assert!(fiber.active_frame().expect("activation frame").registers[2].is_vacant());
        assert_eq!(
            transaction
                .line()
                .ledger()
                .lease(&token)
                .expect("captured voice lease")
                .owner(),
            &crate::line_task::RuntimeHandleOwnerSlot::ScopedDefer(registration.id)
        );
        let result = (registration.id, token);
        executor
            .dialogues
            .restore_transaction(transaction)
            .expect("inspection restores the active dialogue owner");
        result
    };

    let save = AwbcProductExecutorSaveSnapshot::from_live(&product_snapshot(&executor))
        .expect("reached lexical defer packet is saveable");
    let owner = crate::task::RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&executor.program));
    let restored_snapshot = save
        .into_live_for_program(&owner)
        .expect("reached lexical defer packet decodes");
    executor
        .restore_snapshot(restored_snapshot)
        .expect("lexical scope and defer packet restore together");
    {
        let transaction = executor
            .dialogues
            .begin_transaction(&activation)
            .expect("restored affine capture transaction");
        assert_eq!(
            transaction
                .line()
                .ledger()
                .lease(&voice_token)
                .expect("restored voice lease")
                .owner(),
            &crate::line_task::RuntimeHandleOwnerSlot::ScopedDefer(deferred_id)
        );
        executor
            .dialogues
            .restore_transaction(transaction)
            .expect("restored lease inspection returns the active dialogue owner");
    }

    activation_step(&mut executor, &activation, Vec::new(), &mut backend); // LoadConst Unit
    activation_step(&mut executor, &activation, Vec::new(), &mut backend); // CommitDialogueResult
    {
        let transaction = executor
            .dialogues
            .begin_transaction(&activation)
            .expect("staged result transaction");
        assert!(matches!(
            transaction.line().result(),
            crate::line_task::RuntimeDialogueResultState::Committed {
                ty: AwbcTypeId(0),
                value: RuntimeValue::Unit,
            }
        ));
        assert!(matches!(
            transaction.frame().phase,
            ProductDialoguePhase::Activating { .. }
        ));
        executor
            .dialogues
            .restore_transaction(transaction)
            .expect("result inspection returns the active dialogue owner");
    }

    let (cleanup, cleanup_output) =
        activation_step(&mut executor, &activation, Vec::new(), &mut backend);
    assert!(cleanup.progressed);
    assert!(cleanup.presented.is_none());
    assert!(cleanup_output.requests.line_commands.is_empty());
    assert_eq!(executor.child_fibers.len(), 1);
    {
        let transaction = executor
            .dialogues
            .begin_transaction(&activation)
            .expect("cleanup is waiting for its defer child");
        assert!(matches!(
            transaction.line().result(),
            crate::line_task::RuntimeDialogueResultState::Committed { .. }
        ));
        assert!(matches!(
            transaction.frame().phase,
            ProductDialoguePhase::Activating { .. }
        ));
        executor
            .dialogues
            .restore_transaction(transaction)
            .expect("cleanup inspection returns the active dialogue owner");
    }

    let mut release = None;
    for _ in 0..4 {
        if executor.child_fibers.is_empty() {
            break;
        }
        let mut child_output = crate::step::RuntimeStepOutput::default();
        assert!(executor.step_next_child(
            &mut child_output,
            &mut backend,
            &mut RuntimeStepInput::default(),
            &mut Vec::new(),
        ));
        for command in child_output.requests.line_commands {
            if let crate::presentation::RuntimeLineHostCommand::Voice(
                crate::presentation::RuntimeVoiceCommand::ReleaseDialogueVoice {
                    command,
                    handle,
                    ..
                },
            ) = command
            {
                release = Some((command, handle));
            }
        }
    }
    assert!(executor.child_fibers.is_empty());
    let (command, handle) = release.expect("defer child drop requests voice release");
    assert_eq!(handle, voice_token);
    {
        let mut transaction = executor
            .dialogues
            .begin_transaction(&activation)
            .expect("voice release outcome transaction");
        transaction.frame_mut().pending_line_outcomes.push(
            crate::presentation::RuntimeLineHostOutcome::Voice(
                crate::presentation::RuntimeVoiceCommandOutcome::Released { command, handle },
            ),
        );
        commit_test_dialogue_transaction(&mut executor, transaction);
    }
    let (released, _) = activation_step(&mut executor, &activation, Vec::new(), &mut backend);
    assert!(released.progressed);

    let (popped, _) = activation_step(&mut executor, &activation, Vec::new(), &mut backend);
    assert!(popped.progressed);
    assert!(popped.presented.is_none());
    let (revealed, _) = activation_step(&mut executor, &activation, Vec::new(), &mut backend);
    assert!(revealed.progressed);
    assert!(revealed.presented.is_some());
    let transaction = executor
        .dialogues
        .begin_transaction(&activation)
        .expect("revealed dialogue transaction");
    assert!(matches!(
        transaction.frame().phase,
        ProductDialoguePhase::Reducing { .. }
    ));
    assert!(transaction.line().deferred_registrations().is_empty());
    executor
        .dialogues
        .restore_transaction(transaction)
        .expect("revealed dialogue inspection returns the active owner");
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
            task_inputs: Box::new([]),
            values: Box::new([]),
            effect_callbacks: BTreeMap::new(),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(0),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
            phase: ProductDialoguePhase::Activating {
                fiber: crate::awbc::fiber::FiberState::for_function(
                    &executor.program,
                    crate::awbc::fiber::AwbcFiberRoot::Function(AwbcFunctionId(1)),
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
            pending_activation_host_call: None,
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
    commit_test_dialogue_transaction(&mut executor, transaction);

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
            child_fibers: std::collections::VecDeque::new(),
            existing_child_actions: BTreeMap::new(),
            line_task_activations: Vec::new(),
            line_task_baseline: None,
            line_task_reserved_runs: Vec::new(),
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
        let receipt = commit_test_dialogue_transaction(executor, transaction);
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
            &mut RuntimeStepInput::default(),
            &mut Vec::new(),
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
        let mut unpaired = product_snapshot(executor);
        unpaired.child_fibers[0].owner = super::AwbcProductChildFiberOwnerSnapshot::Independent;
        let mut rejected = AwbcProductStepExecutor::for_entry_arc(
            std::sync::Arc::clone(&executor.program),
            AwbcEntryId(0),
            64,
        )
        .expect("unpaired restore candidate starts");
        assert!(rejected.restore_snapshot(unpaired).is_err());

        let save = AwbcProductExecutorSaveSnapshot::from_live(&product_snapshot(executor))
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
            &mut RuntimeStepInput {
                host_call_results: vec![result],
                ..RuntimeStepInput::default()
            },
            &mut Vec::new(),
        ));
        assert!(output.requests.host_calls.is_empty());
        for _ in 0..4 {
            if executor.child_fibers.is_empty() {
                break;
            }
            let mut followup = crate::step::RuntimeStepOutput::default();
            assert!(executor.step_next_child(
                &mut followup,
                backend,
                &mut RuntimeStepInput::default(),
                &mut Vec::new(),
            ));
            assert!(followup.requests.host_calls.is_empty());
        }
        assert!(executor.child_fibers.is_empty());
        let transaction = executor
            .dialogues
            .begin_transaction(activation)
            .expect("completed child activation transaction");
        assert_eq!(transaction.line().deferred_inflight(), None);
        executor
            .dialogues
            .restore_transaction(transaction)
            .expect("completed child inspection restores its activation");
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

    assert!(executor.child_fibers.is_empty());
    let transaction = executor
        .dialogues
        .begin_transaction(&activation)
        .expect("all line-root defer children leave the activation available");
    assert!(transaction.line().deferred_registrations().is_empty());
    assert_eq!(transaction.line().deferred_inflight(), None);
    let ProductDialoguePhase::Activating { fiber, .. } = &transaction.frame().phase else {
        panic!("the unstepped activation return remains in its running phase");
    };
    assert_eq!(
        fiber.cursor,
        crate::awbc::fiber::FiberCursor {
            function: AwbcFunctionId(1),
            block: AwbcBlockId(1),
            instruction_offset: 6,
        },
        "line-root defers finish before the activation return instruction"
    );
    executor
        .dialogues
        .restore_transaction(transaction)
        .expect("final defer inspection restores the activation owner");
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
            task_inputs: Box::new([]),
            values: Box::new([]),
            effect_callbacks: BTreeMap::new(),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(0),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
            phase: ProductDialoguePhase::Activating {
                fiber: copyable_fixture_fiber(&executor),
                pending: None,
            },
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
            pending_activation_host_call: None,
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
            activation_exports: Vec::new(),
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
            task_inputs: Box::new([]),
            values: Box::new([]),
            effect_callbacks: BTreeMap::new(),
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
            pending_activation_host_call: None,
        })
        .expect("activation begins");
    executor.child_fibers.push_back(ProductChildFiber {
        owner: ProductChildFiberOwner::LineTask {
            content: AwbcContentUnitId(0),
            tag,
            policy,
            phase: ProductLineTaskFiberPhase::Active,
        },
        fiber: copyable_fixture_fiber(&executor),
        runtime_generation: executor.runtime_generation,
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
        generation: GenerationId::new(0),
        logical_epoch: LogicalEpoch(7),
        task_id: TaskId("task.snapshot".to_owned()),
        sequence: TaskSequence(3),
        publication_revision: TaskPublicationRevision::FIRST,
        kind: TaskEventKind::Progress(Progress::new(0.25).expect("fixture Progress is valid")),
    };
    let mut output = crate::step::RuntimeStepOutput::default();
    executor.latch_task_events(vec![event.clone()], &mut output);
    assert!(output.diagnostics.is_empty());

    let saved = AwbcProductExecutorSaveSnapshot::from_live(&product_snapshot(&executor))
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
    let snapshot = product_snapshot(&executor);

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
    let mut snapshot = product_snapshot(&executor);
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

fn actor_look_program() -> AwbcProgram {
    use crate::awbc::schema::{
        AwbcLineHandleSite, AwbcLineOperation, AwbcLineTaskGroup, AwbcLineTaskNode,
    };
    use crate::pattern::{RuntimeCheckedType, RuntimeOpaqueTypeAdmission};
    use crate::value::RuntimeEntityReference;

    let character = arcweft_character::id::CharacterId::try_new("character.alice")
        .expect("fixture character identity");
    let mut program = return_program();
    program.strings = vec![
        "entry.main".to_owned(),
        "std.line.stage_actor_handle".to_owned(),
        "std.line.cue_handle".to_owned(),
        "std.character_dialogue".to_owned(),
        "content.actor_look".to_owned(),
    ];
    program.runtime_types = vec![
        AwbcRuntimeType::unit(),
        AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([0x31; 32]),
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(1),
                admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: crate::value::RuntimeOpaqueValueClass::AffineHandle(
                    crate::value::RuntimeHandleKind::StageActor,
                ),
                persistence: crate::value::RuntimeOpaquePersistence::SnapshotOnly,
                arguments: Vec::new(),
            },
        ),
        AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([0x32; 32]),
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(2),
                admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: crate::value::RuntimeOpaqueValueClass::AffineHandle(
                    crate::value::RuntimeHandleKind::Cue,
                ),
                persistence: crate::value::RuntimeOpaquePersistence::SnapshotOnly,
                arguments: Vec::new(),
            },
        ),
        AwbcRuntimeType::new(
            RuntimeCheckedType::EntityReference.semantic_identity_digest(),
            AwbcRuntimeTypeShape::EntityRef,
        ),
        AwbcRuntimeType::new(
            RuntimeCheckedType::Duration.semantic_identity_digest(),
            AwbcRuntimeTypeShape::Duration,
        ),
        AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([0x35; 32]),
            AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(1), AwbcTypeId(2), AwbcTypeId(2)]),
        ),
        AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([0x24; 32]),
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(3),
                admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: crate::value::RuntimeOpaqueValueClass::Plain,
                persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                arguments: Vec::new(),
            },
        ),
    ];
    program.signatures = vec![AwbcSignature {
        params: Vec::new(),
        result: None,
        effects: AwbcEffectSetId(0),
    }];
    program.constants = vec![
        AwbcConstant::EntityRef(RuntimeEntityReference::CharacterLook {
            character: character.clone(),
            look: arcweft_character::id::CharacterLookId::try_new("normal")
                .expect("normal look identity"),
        }),
        AwbcConstant::DurationNanos(0),
        AwbcConstant::EntityRef(RuntimeEntityReference::CharacterLook {
            character: character.clone(),
            look: arcweft_character::id::CharacterLookId::try_new("bright")
                .expect("bright look identity"),
        }),
        AwbcConstant::DurationNanos(120_000_000),
    ];
    program.frame_layouts = vec![
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(5),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            }],
            max_scope_depth: 0,
        },
        AwbcFrameLayout {
            scopes: Vec::new(),
            slots: [
                AwbcTypeId(1),
                AwbcTypeId(3),
                AwbcTypeId(4),
                AwbcTypeId(2),
                AwbcTypeId(3),
                AwbcTypeId(4),
                AwbcTypeId(2),
                AwbcTypeId(5),
            ]
            .into_iter()
            .map(|ty| AwbcFrameSlot {
                name: None,
                ty,
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            })
            .collect(),
            max_scope_depth: 0,
        },
    ];
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(1),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(4),
            constant: AwbcConstantId(2),
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(5),
            constant: AwbcConstantId(3),
        },
        AwbcInstruction::ExecuteLineOperation {
            dst: AwbcRegisterId(0),
            operation: crate::awbc::schema::AwbcLineOperationId(0),
            args: Vec::new(),
        },
        AwbcInstruction::ExecuteLineOperation {
            dst: AwbcRegisterId(3),
            operation: crate::awbc::schema::AwbcLineOperationId(1),
            args: vec![AwbcRegisterId(0), AwbcRegisterId(1), AwbcRegisterId(2)],
        },
        AwbcInstruction::ExecuteLineOperation {
            dst: AwbcRegisterId(6),
            operation: crate::awbc::schema::AwbcLineOperationId(2),
            args: vec![AwbcRegisterId(0), AwbcRegisterId(4), AwbcRegisterId(5)],
        },
        AwbcInstruction::MakeTuple {
            dst: AwbcRegisterId(7),
            items: vec![AwbcRegisterId(0), AwbcRegisterId(3), AwbcRegisterId(6)],
        },
        AwbcInstruction::CommitDialogueResult {
            source: AwbcRegisterId(7),
        },
    ];
    program.blocks = vec![
        AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: AwbcTableRange::new(0, 9),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
    ];
    program.functions[0].blocks = AwbcTableRange::new(0, 1);
    program.functions[0].entry_block = AwbcBlockId(0);
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::LineActivation,
        signature: AwbcSignatureId(0),
        input_ownership: Vec::new(),
        frame_layout: AwbcFrameLayoutId(1),
        blocks: AwbcTableRange::new(1, 1),
        entry_block: AwbcBlockId(1),
        flags: AwbcFunctionFlags::default(),
    });
    program.line_operations = vec![
        AwbcLineOperation::AcquireActor {
            group: crate::awbc::schema::AwbcLineTaskGroupId(0),
            site: crate::awbc::schema::AwbcLineHandleSiteId(0),
            character: character.clone(),
            scope: crate::line_task::RuntimeLineHandleScope::Line,
            result_type: AwbcTypeId(1),
        },
        AwbcLineOperation::ActorLook {
            group: crate::awbc::schema::AwbcLineTaskGroupId(0),
            site: crate::awbc::schema::AwbcLineHandleSiteId(1),
            character: character.clone(),
            actor_type: AwbcTypeId(1),
            look_type: AwbcTypeId(3),
            result_type: AwbcTypeId(2),
        },
        AwbcLineOperation::ActorLook {
            group: crate::awbc::schema::AwbcLineTaskGroupId(0),
            site: crate::awbc::schema::AwbcLineHandleSiteId(2),
            character: character.clone(),
            actor_type: AwbcTypeId(1),
            look_type: AwbcTypeId(3),
            result_type: AwbcTypeId(2),
        },
    ];
    let template = crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
        .expect("actor look template identity");
    program.content_templates = vec![AwbcDialogueContentTemplate {
        id: template,
        digest: crate::entry::RuntimeDialogueContentTemplateDigest::ZERO,
        slots: Vec::new(),
        effects: Vec::new(),
    }];
    program.content_units = vec![AwbcContentUnit {
        public_id: AwbcStringId(4),
        template,
        marks: Vec::new(),
        effect_site_count: 0,
        line_task_group: Some(crate::awbc::schema::AwbcLineTaskGroupId(0)),
        display: None,
        source: None,
        resources: Vec::new(),
    }];
    program.line_task_nodes = vec![AwbcLineTaskNode::Sequence(Vec::new())];
    program.line_task_groups = vec![AwbcLineTaskGroup {
        captures: Vec::new(),
        activation_exports: Vec::new(),
        activation: AwbcFunctionId(1),
        result_type: AwbcTypeId(5),
        handle_sites: vec![
            AwbcLineHandleSite {
                source_ordinal: 0,
                kind: crate::value::RuntimeHandleKind::StageActor,
                result_type: AwbcTypeId(1),
                character: Some(character.clone()),
                scheduled_child: None,
            },
            AwbcLineHandleSite {
                source_ordinal: 1,
                kind: crate::value::RuntimeHandleKind::Cue,
                result_type: AwbcTypeId(2),
                character: Some(character.clone()),
                scheduled_child: None,
            },
            AwbcLineHandleSite {
                source_ordinal: 2,
                kind: crate::value::RuntimeHandleKind::Cue,
                result_type: AwbcTypeId(2),
                character: Some(character),
                scheduled_child: None,
            },
        ],
        root: crate::awbc::schema::AwbcLineTaskNodeId(0),
        nodes: AwbcTableRange::new(0, 1),
        cancel_handlers: Vec::new(),
        cleanup_completed: None,
        cleanup_cancelled: None,
        cleanup_failed: None,
        cleanup: crate::awbc::schema::AwbcLineCleanupPolicy {
            child_tasks: crate::awbc::schema::AwbcChildCleanup::Finish,
            presentation: crate::awbc::schema::AwbcPresentationCleanup::KeepRegistered,
            audio: crate::awbc::schema::AwbcAudioCleanup::KeepRegistered,
        },
    }];
    program.patterns = vec![AwbcPattern::Bind {
        target: AwbcRegisterId(0),
        mutable: false,
        expected: Some(AwbcTypeId(5)),
    }];
    program.canonicalize_string_table();
    program
        .verify(Default::default(), Default::default())
        .expect("two-look/out-return AWBC program verifies");
    program
}

fn scheduled_actor_look_program() -> AwbcProgram {
    let mut program = actor_look_program();
    program.line_operations[2] = crate::awbc::schema::AwbcLineOperation::Schedule {
        group: crate::awbc::schema::AwbcLineTaskGroupId(0),
        site: crate::awbc::schema::AwbcLineHandleSiteId(2),
        child: crate::awbc::schema::AwbcLineTaskNodeId(1),
        captures: vec![
            crate::awbc::schema::AwbcLineScheduledCapture {
                local: crate::runtime_id::RuntimeLocalDeclarationId::from_accepted_ordinal(
                    std::num::NonZeroU32::new(1).expect("first capture local"),
                ),
                ty: AwbcTypeId(1),
            },
            crate::awbc::schema::AwbcLineScheduledCapture {
                local: crate::runtime_id::RuntimeLocalDeclarationId::from_accepted_ordinal(
                    std::num::NonZeroU32::new(2).expect("second capture local"),
                ),
                ty: AwbcTypeId(2),
            },
        ],
        result_type: AwbcTypeId(2),
    };
    program.instructions[6] = AwbcInstruction::ExecuteLineOperation {
        dst: AwbcRegisterId(6),
        operation: crate::awbc::schema::AwbcLineOperationId(2),
        args: vec![AwbcRegisterId(5), AwbcRegisterId(0), AwbcRegisterId(3)],
    };
    program.instructions.truncate(7);
    program.blocks[1].instructions = AwbcTableRange::new(0, 7);
    program.line_task_nodes = vec![
        crate::awbc::schema::AwbcLineTaskNode::Sequence(vec![
            crate::awbc::schema::AwbcLineTaskNodeId(1),
        ]),
        crate::awbc::schema::AwbcLineTaskNode::Child {
            trigger: crate::awbc::schema::AwbcLineTaskTrigger::Scheduled(
                crate::awbc::schema::AwbcLineHandleSiteId(2),
            ),
            join: crate::awbc::schema::AwbcChildJoinPolicy::Join,
            cancel: crate::awbc::schema::AwbcChildCancelPolicy::CancelAndJoin,
            scope: crate::awbc::schema::AwbcLineTaskNodeId(2),
        },
        crate::awbc::schema::AwbcLineTaskNode::Action(AwbcFunctionId(2)),
    ];
    let group = &mut program.line_task_groups[0];
    group.nodes = AwbcTableRange::new(0, 3);
    group.root = crate::awbc::schema::AwbcLineTaskNodeId(0);
    group.handle_sites[2].scheduled_child = Some(crate::awbc::schema::AwbcLineTaskNodeId(1));
    group.handle_sites[2].character = None;

    program.signatures.push(AwbcSignature {
        params: vec![AwbcTypeId(1), AwbcTypeId(2)],
        result: None,
        effects: AwbcEffectSetId(0),
    });
    program.frame_layouts.push(AwbcFrameLayout {
        scopes: Vec::new(),
        slots: [AwbcTypeId(1), AwbcTypeId(2)]
            .into_iter()
            .map(|ty| AwbcFrameSlot {
                name: None,
                ty,
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            })
            .collect(),
        max_scope_depth: 0,
    });
    program.functions.push(AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::LineTask,
        signature: AwbcSignatureId(1),
        input_ownership: vec![AwbcFunctionInputOwnership::default(); 2],
        frame_layout: AwbcFrameLayoutId(2),
        blocks: AwbcTableRange::new(2, 1),
        entry_block: AwbcBlockId(2),
        flags: AwbcFunctionFlags::default(),
    });
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(2),
        instructions: AwbcTableRange::new(7, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    program
        .verify(Default::default(), Default::default())
        .expect("valid scheduled-capture program verifies before runtime corruption");
    program
}

fn actor_effect_program() -> AwbcProgram {
    let mut program = actor_look_program();
    let level = constant_string(&mut program, "info");
    let fallback_message = constant_string(&mut program, "actor handle observed");
    program.signatures.push(AwbcSignature {
        params: vec![AwbcTypeId(1)],
        result: None,
        effects: AwbcEffectSetId(0),
    });
    program.effect_plans = vec![AwbcEffectPlan {
        kind: AwbcEffectKind::Log,
        signature: AwbcSignatureId(1),
        capability: None,
        audio: None,
        static_args: vec![level, fallback_message],
        resources: Vec::new(),
    }];
    program.instructions.truncate(6);
    program.instructions[5] = AwbcInstruction::EmitEffect {
        effect: AwbcEffectPlanId(0),
        args: vec![AwbcRegisterId(0)],
    };
    program.blocks[1].instructions = AwbcTableRange::new(0, 6);
    program.canonicalize_string_table();
    program
        .verify(Default::default(), Default::default())
        .expect("affine dynamic-effect program verifies");
    program
}

fn begin_actor_look_dialogue(
    executor: &mut AwbcProductStepExecutor,
) -> crate::runtime_id::DialogueActivationId {
    let activation = crate::runtime_id::DialogueActivationId::new(
        executor.artifact_fingerprint,
        executor.facade_fiber.persistent_id,
        crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
            std::num::NonZeroU32::MIN,
        ),
        0,
    );
    let target_owner = executor
        .program
        .opaque_owner(AwbcTypeId(6))
        .expect("fixture dialogue target type resolves")
        .expect("fixture dialogue target is opaque");
    executor
        .dialogues
        .begin(ActiveDialogue {
            activation: activation.clone(),
            content: AwbcContentUnitId(0),
            target: crate::value::RuntimeOpaqueValue::new_exact(&target_owner, RuntimeValue::Unit),
            target_type: AwbcTypeId(6),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.actor.look")
                .expect("fixture line identity"),
            captures: Box::new([]),
            task_inputs: Box::new([]),
            values: Box::new([]),
            effect_callbacks: BTreeMap::new(),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(5),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
            phase: ProductDialoguePhase::Activating {
                fiber: FiberState::for_function(
                    &executor.program,
                    crate::awbc::fiber::AwbcFiberRoot::Function(AwbcFunctionId(1)),
                    AwbcFunctionId(1),
                    1,
                    64,
                )
                .expect("line activation fiber starts"),
                pending: None,
            },
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
            pending_activation_host_call: None,
        })
        .expect("fixture dialogue activation begins");
    activation
}

fn publish_actor_stage_outcome(
    executor: &mut AwbcProductStepExecutor,
    activation: &crate::runtime_id::DialogueActivationId,
    outcome: crate::presentation::RuntimeStageCommandOutcome,
) {
    let mut transaction = executor
        .dialogues
        .begin_transaction(activation)
        .expect("outcome transaction remains registered");
    transaction
        .frame_mut()
        .pending_line_outcomes
        .push(crate::presentation::RuntimeLineHostOutcome::Stage(outcome));
    commit_test_dialogue_transaction(executor, transaction);
}

fn step_actor_activation_output(
    executor: &mut AwbcProductStepExecutor,
    activation: &crate::runtime_id::DialogueActivationId,
    backend: &mut impl crate::pure::RuntimeCallBackend,
) -> Result<crate::step::RuntimeStepOutput, ProductStepError> {
    let mut transaction = executor
        .dialogues
        .begin_transaction(activation)
        .expect("activation transaction remains registered");
    let mut progress = executor.step_dialogue_activation(&mut transaction, backend)?;
    let receipt = commit_test_dialogue_transaction(executor, transaction);
    let mut output = crate::step::RuntimeStepOutput::default();
    if let Some(batch) = progress.execution.take() {
        executor.commit_line_task_commands(batch, &mut output);
    }
    output
        .requests
        .line_commands
        .extend(receipt.into_line().into_commands());
    Ok(output)
}

#[test]
fn invalid_schedule_capture_restores_observation_before_failure_close() {
    let mut executor =
        AwbcProductStepExecutor::for_entry(scheduled_actor_look_program(), AwbcEntryId(0), 64)
            .expect("valid scheduled-capture Product executor starts");
    // This simulates corrupted runtime schema after admission: verifier rejects
    // duplicate capture destinations, while Product must still preserve every
    // yielded affine operand if defensive preflight finds one.
    let operation = match &mut std::sync::Arc::make_mut(&mut executor.program).line_operations[2] {
        crate::awbc::schema::AwbcLineOperation::Schedule { captures, .. } => captures,
        _ => unreachable!("fixture schedule operation remains present"),
    };
    operation[1].local = operation[0].local;

    let activation = begin_actor_look_dialogue(&mut executor);
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    let mut actor_token = None;
    let mut cue_token = None;
    let mut staged_error = None;
    for _ in 0..12 {
        let mut transaction = executor
            .dialogues
            .begin_transaction(&activation)
            .expect("activation transaction remains registered");
        match executor.step_dialogue_activation(&mut transaction, &mut backend) {
            Ok(mut progress) => {
                let receipt = commit_test_dialogue_transaction(&mut executor, transaction);
                let mut output = crate::step::RuntimeStepOutput::default();
                if let Some(batch) = progress.execution.take() {
                    executor.commit_line_task_commands(batch, &mut output);
                }
                output
                    .requests
                    .line_commands
                    .extend(receipt.into_line().into_commands());
                for command in output.requests.line_commands {
                    match command {
                        crate::presentation::RuntimeLineHostCommand::Stage(
                            crate::presentation::RuntimeStageCommand::AcquireActor {
                                command,
                                actor,
                                ..
                            },
                        ) => {
                            actor_token = Some(actor.clone());
                            publish_actor_stage_outcome(
                                &mut executor,
                                &activation,
                                crate::presentation::RuntimeStageCommandOutcome::Acquired {
                                    command,
                                    actor,
                                },
                            );
                        }
                        crate::presentation::RuntimeLineHostCommand::Stage(
                            crate::presentation::RuntimeStageCommand::SetCharacterLook {
                                command,
                                cue,
                                ..
                            },
                        ) => {
                            cue_token = Some(cue.clone());
                            publish_actor_stage_outcome(
                                &mut executor,
                                &activation,
                                crate::presentation::RuntimeStageCommandOutcome::Accepted {
                                    command,
                                    cue,
                                },
                            );
                        }
                        other => panic!("unexpected schedule fixture command: {other:?}"),
                    }
                }
            }
            Err(error) => {
                staged_error = Some((transaction, error));
                break;
            }
        }
    }

    let (transaction, error) = staged_error.expect("duplicate schedule local is rejected");
    assert!(matches!(
        &error,
        ProductStepError::Line(
            crate::line_task::LineRuntimeError::InvalidScheduledCaptureTransition
        )
    ));
    let ProductDialoguePhase::Activating {
        fiber,
        pending: None,
    } = &transaction.frame().phase
    else {
        panic!("failed Schedule returns to the live activation phase");
    };
    let frame = fiber.active_frame().expect("activation frame remains live");
    for register in [AwbcRegisterId(0), AwbcRegisterId(3), AwbcRegisterId(5)] {
        assert!(
            frame.registers[register.index()].as_ref().is_some(),
            "Schedule source register {} is restored",
            register.0
        );
    }
    let line = transaction.line();
    assert!(line.scheduled().is_empty());
    for (register, token) in [
        (
            AwbcRegisterId(0),
            actor_token.expect("acquired actor token"),
        ),
        (AwbcRegisterId(3), cue_token.expect("first look cue token")),
    ] {
        let owner = crate::value::ownership::RuntimeOwnedSlotId::AwbcRegister {
            execution: executor.facade_fiber.execution,
            fiber: fiber.instance,
            frame: frame.instance,
            register,
        };
        assert_eq!(
            line.ledger()
                .lease(&token)
                .expect("restored capture lease remains present")
                .owner(),
            &crate::line_task::RuntimeHandleOwnerSlot::ActivationLocal(owner)
        );
    }
    assert!(line.ledger().leases().values().all(|lease| !matches!(
        lease.owner(),
        crate::line_task::RuntimeHandleOwnerSlot::ActivationLocal(
            crate::value::ownership::RuntimeOwnedSlotId::AwbcLineObservationArg { .. }
        )
    )));

    let mut output = crate::step::RuntimeStepOutput::default();
    let _terminal = executor.begin_product_dialogue_failure(transaction, error, &mut output);
    let closing = executor
        .dialogues
        .begin_transaction(&activation)
        .expect("failure-close activation remains registered");
    let _terminal = executor.resume_product_dialogue_failure_close(closing, &mut output);
    assert!(!output.requests.line_commands.is_empty());
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn activation_effect_moves_affine_handle_through_effect_observation_and_default_drop() {
    let mut executor =
        AwbcProductStepExecutor::for_entry(actor_effect_program(), AwbcEntryId(0), 64)
            .expect("affine-effect Product executor starts");
    let activation = begin_actor_look_dialogue(&mut executor);
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    let mut actor = None;

    for _ in 0..5 {
        let output = step_actor_activation_output(&mut executor, &activation, &mut backend)
            .expect("actor acquisition prefix progresses");
        for command in output.requests.line_commands {
            let crate::presentation::RuntimeLineHostCommand::Stage(
                crate::presentation::RuntimeStageCommand::AcquireActor {
                    command,
                    actor: token,
                    ..
                },
            ) = command
            else {
                panic!("unexpected affine-effect prefix command: {command:?}");
            };
            actor = Some(token.clone());
            publish_actor_stage_outcome(
                &mut executor,
                &activation,
                crate::presentation::RuntimeStageCommandOutcome::Acquired {
                    command,
                    actor: token,
                },
            );
        }
    }
    let actor = actor.expect("AcquireActor produced the affine source value");
    step_actor_activation_output(&mut executor, &activation, &mut backend)
        .expect("activation accepts the actor acquisition outcome");
    let output = step_actor_activation_output(&mut executor, &activation, &mut backend)
        .expect("affine dynamic effect commits with its drop reconciliation");

    assert!(matches!(
        output.effects.line.as_slice(),
        [crate::effect::LineEffectRequest::Log(_)]
    ));
    assert!(output.requests.line_commands.iter().any(|command| matches!(
        command,
        crate::presentation::RuntimeLineHostCommand::Stage(
            crate::presentation::RuntimeStageCommand::ReleaseActor { actor: released, .. }
        ) if released == &actor
    )));
    let transaction = executor
        .dialogues
        .begin_transaction(&activation)
        .expect("effect step leaves a valid activation transaction");
    let ProductDialoguePhase::Activating { fiber, .. } = &transaction.frame().phase else {
        panic!("effect step remains in activation");
    };
    assert!(
        fiber
            .active_frame()
            .expect("activation frame remains live")
            .registers[0]
            .is_vacant()
    );
    let lease = transaction
        .line()
        .ledger()
        .lease(&actor)
        .expect("effect-drop lease remains until host release outcome");
    assert_eq!(
        lease.state(),
        crate::line_task::RuntimeHandleLeaseState::Cancelling
    );
    assert!(matches!(
        lease.owner(),
        crate::line_task::RuntimeHandleOwnerSlot::ActivationLocal(
            crate::value::ownership::RuntimeOwnedSlotId::AwbcEffectObservationArg { .. }
        )
    ));
    commit_test_dialogue_transaction(&mut executor, transaction);
    executor
        .snapshot_for_save()
        .expect("pending effect-triggered release is snapshot-safe");
}

#[test]
fn actor_look_borrows_one_stage_actor_across_two_looks_and_out_return() {
    let mut executor = AwbcProductStepExecutor::for_entry(actor_look_program(), AwbcEntryId(0), 64)
        .expect("two-look Product executor starts");
    let activation = crate::runtime_id::DialogueActivationId::new(
        executor.artifact_fingerprint,
        executor.facade_fiber.persistent_id,
        crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
            std::num::NonZeroU32::MIN,
        ),
        0,
    );
    let target_owner = executor
        .program
        .opaque_owner(AwbcTypeId(6))
        .expect("fixture dialogue target type resolves")
        .expect("fixture dialogue target is opaque");
    executor
        .dialogues
        .begin(ActiveDialogue {
            activation: activation.clone(),
            content: AwbcContentUnitId(0),
            target: crate::value::RuntimeOpaqueValue::new_exact(&target_owner, RuntimeValue::Unit),
            target_type: AwbcTypeId(6),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.actor.look")
                .expect("fixture line identity"),
            captures: Box::new([]),
            task_inputs: Box::new([]),
            values: Box::new([]),
            effect_callbacks: BTreeMap::new(),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(5),
                pattern: AwbcPatternId(0),
                destination: AwbcRegisterId(0),
            },
            phase: ProductDialoguePhase::Activating {
                fiber: FiberState::for_function(
                    &executor.program,
                    crate::awbc::fiber::AwbcFiberRoot::Function(AwbcFunctionId(1)),
                    AwbcFunctionId(1),
                    1,
                    64,
                )
                .expect("line activation fiber starts"),
                pending: None,
            },
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
            pending_activation_host_call: None,
        })
        .expect("two-look dialogue activation begins");

    fn activation_step(
        executor: &mut AwbcProductStepExecutor,
        activation: &crate::runtime_id::DialogueActivationId,
    ) -> Vec<crate::presentation::RuntimeLineHostCommand> {
        let mut transaction = executor
            .dialogues
            .begin_transaction(activation)
            .expect("activation transaction remains registered");
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        let mut progress = executor
            .step_dialogue_activation(&mut transaction, &mut backend)
            .expect("one activation instruction progresses");
        let receipt = commit_test_dialogue_transaction(executor, transaction);
        let mut output = crate::step::RuntimeStepOutput::default();
        if let Some(batch) = progress.execution.take() {
            executor.commit_line_task_commands(batch, &mut output);
        }
        output
            .requests
            .line_commands
            .extend(receipt.into_line().into_commands());
        output.requests.line_commands
    }

    fn publish_stage_outcome(
        executor: &mut AwbcProductStepExecutor,
        activation: &crate::runtime_id::DialogueActivationId,
        outcome: crate::presentation::RuntimeStageCommandOutcome,
    ) {
        let mut transaction = executor
            .dialogues
            .begin_transaction(activation)
            .expect("outcome transaction remains registered");
        transaction
            .frame_mut()
            .pending_line_outcomes
            .push(crate::presentation::RuntimeLineHostOutcome::Stage(outcome));
        commit_test_dialogue_transaction(executor, transaction);
    }

    let mut stage_actor = None;
    let mut look_rows = Vec::new();
    let mut committed = false;
    for _ in 0..24 {
        for command in activation_step(&mut executor, &activation) {
            match command {
                crate::presentation::RuntimeLineHostCommand::Stage(
                    crate::presentation::RuntimeStageCommand::AcquireActor {
                        command, actor, ..
                    },
                ) => {
                    assert!(stage_actor.replace(actor.clone()).is_none());
                    publish_stage_outcome(
                        &mut executor,
                        &activation,
                        crate::presentation::RuntimeStageCommandOutcome::Acquired {
                            command,
                            actor,
                        },
                    );
                }
                crate::presentation::RuntimeLineHostCommand::Stage(
                    crate::presentation::RuntimeStageCommand::SetCharacterLook {
                        command,
                        cue,
                        actor,
                        look,
                        crossfade,
                        ..
                    },
                ) => {
                    assert_eq!(Some(&actor), stage_actor.as_ref());
                    look_rows.push((actor, cue.clone(), look, crossfade));
                    publish_stage_outcome(
                        &mut executor,
                        &activation,
                        crate::presentation::RuntimeStageCommandOutcome::Accepted { command, cue },
                    );
                }
                other => panic!("unexpected two-look command: {other:?}"),
            }
        }
        let line = executor
            .dialogues
            .active_line()
            .expect("activation line remains registered");
        if matches!(
            line.result(),
            crate::line_task::RuntimeDialogueResultState::Committed { .. }
        ) {
            committed = true;
            break;
        }
    }
    assert!(committed, "activation commits its typed out value");
    assert_eq!(look_rows.len(), 2);
    assert_eq!(look_rows[0].0, look_rows[1].0);
    assert_ne!(look_rows[0].1, look_rows[1].1);
    assert_eq!(look_rows[0].2.as_str(), "normal");
    assert_eq!(look_rows[1].2.as_str(), "bright");
    assert_eq!(look_rows[0].3, crate::time::LogicalDuration::default());
    assert_eq!(
        look_rows[1].3,
        crate::time::LogicalDuration::from_nanos(120_000_000)
    );

    let line = executor
        .dialogues
        .active_line()
        .expect("committed out value remains registered");
    let crate::line_task::RuntimeDialogueResultState::Committed { ty, value } = line.result()
    else {
        panic!("two-look activation must commit its out value");
    };
    assert_eq!(*ty, AwbcTypeId(5));
    let RuntimeValue::Tuple(values) = value else {
        panic!("out value contains actor and both look handles");
    };
    assert_eq!(values.len(), 3);
    let actor_token = crate::line_task::RuntimeLineHandleLedger::token_from_value(&values[0])
        .expect("out actor handle token");
    let first_cue = crate::line_task::RuntimeLineHandleLedger::token_from_value(&values[1])
        .expect("first out cue token");
    let second_cue = crate::line_task::RuntimeLineHandleLedger::token_from_value(&values[2])
        .expect("second out cue token");
    assert_eq!(Some(&actor_token), stage_actor.as_ref());
    assert_eq!(first_cue, look_rows[0].1);
    assert_eq!(second_cue, look_rows[1].1);
    for token in [&actor_token, &first_cue, &second_cue] {
        assert!(matches!(
            line.ledger()
                .lease(token)
                .expect("out handle remains in the activation ledger")
                .owner(),
            crate::line_task::RuntimeHandleOwnerSlot::DialogueResult(_)
        ));
    }
}

fn mark_selector_program(
    cancellation_action: arcweft_interaction_model::input::InputActionId,
) -> AwbcProgram {
    let mut program = return_program();
    let mark = crate::runtime_id::RuntimeDialogueMarkId::from_zero_based(0).expect("mark identity");
    program.strings = vec![
        "entry.main".to_owned(),
        "selected-result".to_owned(),
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
                producer: AwbcStringId(2),
                admission: crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: crate::value::RuntimeOpaqueValueClass::Plain,
                persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                arguments: Vec::new(),
            },
        ),
    ];
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(1),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    program.constants = vec![AwbcConstant::String(AwbcStringId(1))];
    program.patterns = vec![AwbcPattern::Bind {
        target: AwbcRegisterId(0),
        mutable: false,
        expected: Some(AwbcTypeId(1)),
    }];
    program.frame_layouts.push(AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(1),
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        }],
        max_scope_depth: 0,
    });
    program.functions.extend([
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::LineActivation,
            signature: AwbcSignatureId(0),
            input_ownership: vec![
                AwbcFunctionInputOwnership::default();
                program.signatures[AwbcSignatureId(0).index()].params.len()
            ],
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(1, 1),
            entry_block: AwbcBlockId(1),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::LineTask,
            signature: AwbcSignatureId(0),
            input_ownership: vec![
                AwbcFunctionInputOwnership::default();
                program.signatures[AwbcSignatureId(0).index()].params.len()
            ],
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(2, 1),
            entry_block: AwbcBlockId(2),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::LineCancellationHandler,
            signature: AwbcSignatureId(0),
            input_ownership: vec![
                AwbcFunctionInputOwnership::default();
                program.signatures[AwbcSignatureId(0).index()].params.len()
            ],
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(3, 1),
            entry_block: AwbcBlockId(3),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        },
    ]);
    program.instructions.push(AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(0),
        constant: AwbcConstantId(0),
    });
    program.blocks.extend([
        AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(2),
            instructions: AwbcTableRange::new(0, 1),
            terminator: AwbcTerminator::SelectDialogueResult {
                value: AwbcRegisterId(0),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(3),
            instructions: AwbcTableRange::new(1, 0),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
    ]);
    let template = crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
        .expect("content template identity");
    program.content_templates = vec![AwbcDialogueContentTemplate {
        id: template,
        digest: crate::entry::RuntimeDialogueContentTemplateDigest::ZERO,
        slots: Vec::new(),
        effects: Vec::new(),
    }];
    program.content_units = vec![AwbcContentUnit {
        public_id: AwbcStringId(0),
        template,
        marks: vec![crate::awbc::schema::AwbcDialogueMark {
            id: mark,
            label: AwbcStringId(1),
        }],
        effect_site_count: 0,
        line_task_group: Some(crate::awbc::schema::AwbcLineTaskGroupId(0)),
        display: None,
        source: None,
        resources: Vec::new(),
    }];
    program.line_task_nodes = vec![
        crate::awbc::schema::AwbcLineTaskNode::Sequence(vec![
            crate::awbc::schema::AwbcLineTaskNodeId(1),
        ]),
        crate::awbc::schema::AwbcLineTaskNode::Child {
            trigger: crate::awbc::schema::AwbcLineTaskTrigger::Mark(mark),
            join: crate::awbc::schema::AwbcChildJoinPolicy::Join,
            cancel: crate::awbc::schema::AwbcChildCancelPolicy::CancelAndJoin,
            scope: crate::awbc::schema::AwbcLineTaskNodeId(2),
        },
        crate::awbc::schema::AwbcLineTaskNode::Action(AwbcFunctionId(2)),
    ];
    program.line_task_groups = vec![crate::awbc::schema::AwbcLineTaskGroup {
        captures: Vec::new(),
        activation_exports: Vec::new(),
        activation: AwbcFunctionId(1),
        result_type: AwbcTypeId(1),
        handle_sites: Vec::new(),
        root: crate::awbc::schema::AwbcLineTaskNodeId(0),
        nodes: AwbcTableRange::new(0, 3),
        cancel_handlers: vec![AwbcLineCancelHandler {
            trigger: cancellation_action,
            function: AwbcFunctionId(3),
        }],
        cleanup_completed: None,
        cleanup_cancelled: None,
        cleanup_failed: None,
        cleanup: crate::awbc::schema::AwbcLineCleanupPolicy {
            child_tasks: crate::awbc::schema::AwbcChildCleanup::Finish,
            presentation: crate::awbc::schema::AwbcPresentationCleanup::KeepRegistered,
            audio: crate::awbc::schema::AwbcAudioCleanup::KeepRegistered,
        },
    }];
    program
}

fn init_scope_defer_host_call_program() -> AwbcProgram {
    let mut program = return_program();
    program.strings = vec![
        "entry.main".to_owned(),
        "host.init".to_owned(),
        "init".to_owned(),
        "read".to_owned(),
        "post-out tail must be skipped".to_owned(),
        "std.character_dialogue".to_owned(),
        "std.line.voice_handle".to_owned(),
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
                producer: AwbcStringId(5),
                admission: crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: crate::value::RuntimeOpaqueValueClass::Plain,
                persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                arguments: Vec::new(),
            },
        ),
        AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([0x34; 32]),
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(6),
                admission: crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: crate::value::RuntimeOpaqueValueClass::AffineHandle(
                    crate::value::RuntimeHandleKind::Voice,
                ),
                persistence: crate::value::RuntimeOpaquePersistence::SnapshotOnly,
                arguments: Vec::new(),
            },
        ),
    ];
    program.signatures.extend([
        AwbcSignature {
            params: Vec::new(),
            result: Some(AwbcTypeId(1)),
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: vec![AwbcTypeId(1), AwbcTypeId(3)],
            result: Some(AwbcTypeId(0)),
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: vec![AwbcTypeId(1), AwbcTypeId(1)],
            result: None,
            effects: AwbcEffectSetId(0),
        },
    ]);
    program.signatures.push(AwbcSignature {
        params: vec![AwbcTypeId(1)],
        result: None,
        effects: AwbcEffectSetId(0),
    });
    program.constants = vec![AwbcConstant::Unit];
    let log_level = constant_string(&mut program, "info");
    let log_fallback_message = constant_string(&mut program, "fallback message");
    let log_field_name = constant_string(&mut program, "source");
    let log_field_value = constant_string(&mut program, "init");
    program.effect_plans = vec![AwbcEffectPlan {
        kind: AwbcEffectKind::Log,
        signature: AwbcSignatureId(3),
        capability: None,
        audio: None,
        static_args: vec![
            log_level,
            log_fallback_message,
            log_field_name,
            log_field_value,
        ],
        resources: Vec::new(),
    }];
    program.frame_layouts.extend([
        AwbcFrameLayout {
            scopes: vec![crate::awbc::schema::AwbcScopeDefinition {
                parent: None,
                identity: crate::scope::RuntimeScopeIdentity::Anonymous,
            }],
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Parameter,
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
                    ty: AwbcTypeId(3),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 1,
                },
            ],
            max_scope_depth: 1,
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
                    ty: AwbcTypeId(3),
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
    program.frame_layouts[1].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(1),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 1,
    });
    program.frame_layouts[1].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(1),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 1,
    });
    program.frame_layouts[1].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 1,
    });
    program.host_calls = vec![AwbcHostCall {
        public_id: AwbcStringId(1),
        capability: AwbcStringId(2),
        operation: AwbcStringId(3),
        contract: None,
        signature: AwbcSignatureId(1),
        mode: AwbcHostCallMode::Suspend,
        deterministic: true,
        arguments: Vec::new(),
    }];
    program.resume_points = vec![AwbcResumePoint {
        function: AwbcFunctionId(1),
        block: AwbcBlockId(2),
        frame_layout: AwbcFrameLayoutId(1),
        kind: AwbcSafePointKind::HostCall,
    }];
    program.instructions = vec![
        AwbcInstruction::EnterScope {
            scope: crate::awbc::schema::AwbcScopeId(0),
        },
        AwbcInstruction::EmitEffect {
            effect: AwbcEffectPlanId(0),
            args: vec![AwbcRegisterId(3), AwbcRegisterId(4)],
        },
        AwbcInstruction::ExecuteLineOperation {
            dst: AwbcRegisterId(2),
            operation: crate::awbc::schema::AwbcLineOperationId(0),
            args: Vec::new(),
        },
        AwbcInstruction::RegisterDefer {
            site: crate::runtime_id::RuntimeDeferSiteId::from_zero_based(0)
                .expect("first defer site"),
            outcome: crate::line_task::RuntimeDeferOutcomeFilter::Always,
            owner: crate::awbc::schema::AwbcDeferOwner::CurrentScope,
            captures: vec![AwbcRegisterId(1), AwbcRegisterId(2)],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(5),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::CommitDialogueResult {
            source: AwbcRegisterId(5),
        },
        AwbcInstruction::Nop,
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(2),
            constant: AwbcConstantId(0),
        },
    ];
    program.instructions.insert(
        1,
        AwbcInstruction::CopyValue {
            dst: AwbcRegisterId(3),
            src: AwbcRegisterId(1),
        },
    );
    program.instructions.insert(
        2,
        AwbcInstruction::CopyValue {
            dst: AwbcRegisterId(4),
            src: AwbcRegisterId(1),
        },
    );
    program.line_operations = vec![crate::awbc::schema::AwbcLineOperation::VoiceHandle {
        group: crate::awbc::schema::AwbcLineTaskGroupId(0),
        site: crate::awbc::schema::AwbcLineHandleSiteId(0),
        result_type: AwbcTypeId(3),
    }];
    program.functions.extend([
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::LineActivation,
            signature: AwbcSignatureId(4),
            input_ownership: vec![
                AwbcFunctionInputOwnership::default();
                program.signatures[AwbcSignatureId(4).index()].params.len()
            ],
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, 2),
            entry_block: AwbcBlockId(1),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::MaySuspend),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(2),
            input_ownership: vec![
                AwbcFunctionInputOwnership::default();
                program.signatures[AwbcSignatureId(2).index()].params.len()
            ],
            frame_layout: AwbcFrameLayoutId(2),
            blocks: AwbcTableRange::new(3, 1),
            entry_block: AwbcBlockId(3),
            flags: AwbcFunctionFlags::default(),
        },
    ]);
    program.blocks.extend([
        AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::HostCall {
                call: AwbcHostCallId(0),
                args: Vec::new(),
                dst: Some(AwbcRegisterId(1)),
                resume: AwbcResumePointId(0),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(1),
            instructions: AwbcTableRange::new(0, 9),
            terminator: AwbcTerminator::Trap {
                code: AwbcTrapCode::InternalInvariant,
                message: Some(AwbcStringId(4)),
            },
            safe_point: AwbcSafePointKind::Trap,
            source_map: None,
        },
        AwbcBlock {
            owner: AwbcFunctionId(2),
            instructions: AwbcTableRange::new(9, 2),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(2)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
    ]);
    program.defer_sites = vec![AwbcFunctionId(2)];
    program.instructions[9] = AwbcInstruction::Drop {
        register: AwbcRegisterId(1),
        policy: crate::awbc::schema::AwbcDropPolicy::Default,
    };
    program.instructions.push(AwbcInstruction::LoadConst {
        dst: AwbcRegisterId(2),
        constant: AwbcConstantId(0),
    });
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
        captures: vec![
            crate::runtime_id::RuntimeLocalDeclarationId::from_accepted_ordinal(
                std::num::NonZeroU32::MIN,
            ),
        ],
        activation_exports: Vec::new(),
        activation: AwbcFunctionId(1),
        result_type: AwbcTypeId(0),
        handle_sites: vec![crate::awbc::schema::AwbcLineHandleSite {
            source_ordinal: 0,
            kind: crate::value::RuntimeHandleKind::Voice,
            result_type: AwbcTypeId(3),
            character: None,
            scheduled_child: None,
        }],
        root: crate::awbc::schema::AwbcLineTaskNodeId(0),
        nodes: AwbcTableRange::new(0, 1),
        cancel_handlers: Vec::new(),
        cleanup_completed: None,
        cleanup_cancelled: None,
        cleanup_failed: None,
        cleanup: crate::awbc::schema::AwbcLineCleanupPolicy {
            child_tasks: crate::awbc::schema::AwbcChildCleanup::Finish,
            presentation: crate::awbc::schema::AwbcPresentationCleanup::KeepRegistered,
            audio: crate::awbc::schema::AwbcAudioCleanup::KeepRegistered,
        },
    }];
    program.canonicalize_string_table();
    program
        .verify(Default::default(), Default::default())
        .expect("Init scope-defer host-call program verifies");
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
            input_ownership: vec![
                AwbcFunctionInputOwnership::default();
                program.signatures[AwbcSignatureId(1).index()].params.len()
            ],
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(1, 1),
            entry_block: AwbcBlockId(1),
            flags: AwbcFunctionFlags::default(),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(2),
            input_ownership: vec![
                AwbcFunctionInputOwnership::default();
                program.signatures[AwbcSignatureId(2).index()].params.len()
            ],
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
        activation_exports: Vec::new(),
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
        Some(FiberTerminalValue::Returned(None))
    );
    assert_eq!(
        executor.fiber.return_summary.as_deref(),
        Some(crate::value::runtime_value_label(&expected).as_str())
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
    assert_eq!(result.stats.need_states_in, 1);
    assert!(result.output.requests.tasks.is_empty());
    assert!(result.output.flow_events.iter().any(|event| matches!(
        event,
        crate::plan::FlowEvent::AwaitStarted { need, task: None }
            if need.0 == "need.profile"
    )));
    assert!(result.output.flow_events.iter().any(|event| matches!(
        event,
        crate::plan::FlowEvent::AwaitReady { need, .. } if need.0 == "need.profile"
    )));
    assert!(
        result
            .output
            .flow_events
            .iter()
            .all(|event| !matches!(event, crate::plan::FlowEvent::AwaitProgress { .. }))
    );
}

#[test]
fn direct_need_parameter_rejects_string_surrogate_and_empty_identity() {
    for value in [
        RuntimeValue::String("need.profile".to_owned()),
        RuntimeValue::Need(NeedId(String::new())),
    ] {
        assert!(
            AwbcProductStepExecutor::for_function_invocation(
                direct_need_program(),
                AwbcEntryId(0),
                AwbcFunctionId(0),
                [RuntimeFlowParameterBinding {
                    parameter: crate::entry::FlowParameterCoordinate::from_position(0),
                    value,
                }],
                64,
            )
            .is_err()
        );
    }
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
        Some(FiberTerminalValue::Returned(None))
    );
    assert_eq!(
        executor.fiber.return_summary.as_deref(),
        Some(crate::value::runtime_value_label(&expected).as_str())
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
    assert!(result.output.diagnostics.is_empty());
    assert!(result.output.requests.tasks.is_empty());
}

#[test]
fn ready_need_payload_must_match_the_selected_item_type() {
    let (mut executor, input) = typed_direct_need_executor_and_input(vec![runtime_need_state(
        0,
        Need::Ready(RuntimePayload(RuntimeValue::Bool(true))),
    )]);

    let result = executor.step(input, direct_need_step_options());

    assert_eq!(result.stop_reason, RuntimeStepStopReason::Failed);
    assert!(matches!(
        executor.fiber.terminal,
        Some(FiberTerminalValue::Trapped(ref trap))
            if trap.code == crate::awbc::schema::AwbcTrapCode::HostAbiMismatch
    ));
    assert!(
        result
            .output
            .flow_events
            .iter()
            .all(|event| !matches!(event, crate::plan::FlowEvent::AwaitReady { .. }))
    );
}

#[test]
fn need_await_snapshot_retains_and_validates_selected_item_type() {
    let (mut executor, input) = typed_direct_need_executor_and_input(vec![runtime_need_state(
        0,
        Need::Pending(Progress::new(0.5).expect("fixture progress is valid")),
    )]);
    let result = executor.step(input, direct_need_step_options());
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Output);

    let mut tampered = product_snapshot(&executor);
    assert!(matches!(
        tampered
            .fiber
            .suspension
            .as_ref()
            .map(|suspension| &suspension.reason),
        Some(
            crate::awbc::fiber::AwbcFiberSuspensionReasonSnapshot::Await {
                target: crate::awbc::fiber::AwbcFiberAwaitTargetSnapshot::Need {
                    item_type: AwbcTypeId(2),
                    ..
                },
                ..
            }
        )
    ));

    let Some(crate::awbc::fiber::AwbcFiberSuspensionSnapshot {
        reason:
            crate::awbc::fiber::AwbcFiberSuspensionReasonSnapshot::Await {
                target: crate::awbc::fiber::AwbcFiberAwaitTargetSnapshot::Need { item_type, .. },
                ..
            },
        ..
    }) = tampered.fiber.suspension.as_mut()
    else {
        panic!("typed Need remains suspended");
    };
    *item_type = AwbcTypeId(1);
    assert!(executor.restore_snapshot(tampered).is_err());

    let snapshot = product_snapshot(&executor);
    let saved =
        AwbcProductExecutorSaveSnapshot::from_live(&snapshot).expect("typed Need suspension saves");
    let encoded = serde_json::to_string(&saved).expect("typed Need suspension serializes");
    let decoded: AwbcProductExecutorSaveSnapshot =
        serde_json::from_str(&encoded).expect("typed Need suspension restores");
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program.clone());
    let restored = decoded
        .into_live_for_program(&owner)
        .expect("typed Need suspension converts to live state");
    assert_eq!(restored.fiber, snapshot.fiber);
}

#[test]
fn unresolved_direct_need_blocks_without_inventing_a_task_request() {
    let unresolved = [
        Need::NotStarted,
        Need::Pending(Progress::new(0.5).expect("fixture progress is valid")),
    ];
    for state in unresolved {
        let is_pending = matches!(&state, Need::Pending(_));
        let (mut executor, input) =
            direct_need_executor_and_input(vec![runtime_need_state(0, state)]);

        let result = executor.step(input, direct_need_step_options());

        assert!(matches!(
            &result.fiber_status,
            FlowFiberStatus::NeedWaiting(state) if state.need == NeedId("need.profile".to_owned())
        ));
        assert_eq!(result.stop_reason, RuntimeStepStopReason::Output);
        assert!(result.output.requests.tasks.is_empty());
        assert!(result.output.flow_events.iter().any(|event| matches!(
            event,
            crate::plan::FlowEvent::AwaitStarted { need, task: None }
                if need.0 == "need.profile"
        )));
        if is_pending {
            assert!(result.output.flow_events.iter().any(|event| matches!(
                event,
                crate::plan::FlowEvent::AwaitProgress { need, .. }
                    if need.0 == "need.profile"
            )));
        }
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
        Some(FiberTerminalValue::Returned(None))
    );
    assert_eq!(
        executor.fiber.return_summary.as_deref(),
        Some(crate::value::runtime_value_label(&expected).as_str())
    );
    assert_eq!(result.stats.need_states_in, 3);
    assert!(result.output.requests.tasks.is_empty());
}

#[test]
fn restartable_need_save_restore_reensures_exact_launch_and_accepts_next_revision() {
    let generation = GenerationId::new(7);
    let program = std::sync::Arc::new(need_producer_program(AwbcTaskRestartPolicy::Restartable));
    let mut executor = AwbcProductStepExecutor::for_entry_arc_with_generation(
        std::sync::Arc::clone(&program),
        AwbcEntryId(0),
        64,
        generation,
    )
    .expect("Restartable Need producer verifies");

    let start = executor.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            max_new_task_requests: 1,
            ..RuntimeStepOptions::default()
        },
    );
    assert_eq!(start.output.requests.tasks.len(), 1);
    let task_spec = start.output.requests.tasks[0].clone();
    let need = executor
        .restartable_dispatches()
        .into_iter()
        .find(|dispatch| dispatch.task_id == task_spec.id)
        .expect("accepted producer is visible to dispatch projection")
        .need_id;

    for (revision, amount) in [(1, 0.25), (2, 0.5)] {
        let progress = producer_task_event(
            generation,
            task_spec.id.clone(),
            revision,
            TaskEventKind::Progress(Progress::new(amount).expect("progress is valid")),
        );
        let step = executor.step(
            RuntimeStepInput {
                task_events: vec![progress],
                ..RuntimeStepInput::default()
            },
            RuntimeStepOptions::default(),
        );
        assert!(step.output.flow_events.iter().any(|event| matches!(
            event,
            crate::plan::FlowEvent::AwaitProgress { need: observed, .. }
                if observed == &need
        )));
        assert!(matches!(step.fiber_status, FlowFiberStatus::NeedWaiting(_)));
    }

    let live = executor
        .snapshot_for_save()
        .expect("Restartable in-flight Need can be saved");
    let saved = AwbcProductExecutorSaveSnapshot::from_live(&live)
        .expect("Restartable Need snapshot serializes");
    let mut future_observation = live;
    let observation_key = future_observation
        .need_publications
        .keys()
        .find(|(_, observed_need)| observed_need == &need)
        .cloned()
        .expect("local Await retains its observed Need cursor");
    let crate::task::TaskPublicationCursor::LocalTaskEvent {
        generation: observed_generation,
        logical_epoch,
        dispatch_sequence,
        ..
    } = future_observation.need_publications[&observation_key]
    else {
        panic!("local producer observation uses a task event cursor")
    };
    future_observation.need_publications.insert(
        observation_key,
        crate::task::TaskPublicationCursor::LocalTaskEvent {
            generation: observed_generation,
            logical_epoch,
            dispatch_sequence,
            publication_revision: TaskPublicationRevision::new(
                std::num::NonZeroU64::new(3).expect("future revision is nonzero"),
            ),
        },
    );
    let error = executor
        .restore_snapshot(future_observation)
        .expect_err("a waiter cannot claim a producer revision it has not accepted");
    assert!(matches!(
        error,
        AwbcProductStepBuildError::RestoreSnapshot { ref message }
            if message.contains("not bounded by its accepted producer publication")
    ));
    assert!(executor.restartable_dispatches().iter().any(|dispatch| {
        dispatch.task_id == task_spec.id
            && matches!(
                dispatch.publication,
                Some(crate::task::TaskPublicationCursor::LocalTaskEvent {
                    publication_revision,
                    ..
                }) if publication_revision.get() == 2
            )
    }));
    let encoded = serde_json::to_string(&saved).expect("save snapshot serializes");
    let decoded: AwbcProductExecutorSaveSnapshot =
        serde_json::from_str(&encoded).expect("save snapshot decodes");
    let restored_live = decoded
        .into_live_for_program(&crate::task::RuntimeProgramOwner::Awbc(
            std::sync::Arc::clone(&program),
        ))
        .expect("verified program and typed producer snapshot restore");
    let mut restored = AwbcProductStepExecutor::for_entry_arc_with_generation(
        std::sync::Arc::clone(&program),
        AwbcEntryId(0),
        64,
        generation,
    )
    .expect("restore executor starts");
    restored
        .restore_snapshot(restored_live)
        .expect("Restartable launch and publication revision restore");
    assert!(
        restored
            .restartable_dispatches()
            .iter()
            .any(|dispatch| { dispatch.task_id == task_spec.id && dispatch.needs_reensure })
    );

    let reensure = restored.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            max_new_task_requests: 0,
            ..RuntimeStepOptions::default()
        },
    );
    assert_eq!(reensure.output.requests.tasks, vec![task_spec.clone()]);
    assert!(
        restored
            .restartable_dispatches()
            .iter()
            .all(|dispatch| !dispatch.needs_reensure)
    );

    let ready = producer_task_event(
        generation,
        task_spec.id.clone(),
        3,
        TaskEventKind::Ready(RuntimePayload(RuntimeValue::String("ready".to_owned()))),
    );
    let completed = restored.step(
        RuntimeStepInput {
            task_events: vec![ready],
            ..RuntimeStepInput::default()
        },
        RuntimeStepOptions::default(),
    );
    assert_eq!(completed.stop_reason, RuntimeStepStopReason::OneOp);
    assert!(completed.output.flow_events.iter().any(|event| matches!(
        event,
        crate::plan::FlowEvent::AwaitReady { need: observed, value }
            if observed == &need
                && value.as_ref().is_some_and(|payload| {
                    payload.value() == &RuntimeValue::String("ready".to_owned())
                })
    )));
    let finished = restored.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
    assert_eq!(finished.stop_reason, RuntimeStepStopReason::Done);
}

#[test]
fn must_be_quiescent_need_save_returns_typed_defer() {
    let generation = GenerationId::new(7);
    let mut executor = AwbcProductStepExecutor::for_entry_arc_with_generation(
        std::sync::Arc::new(need_producer_program(
            AwbcTaskRestartPolicy::MustBeQuiescent,
        )),
        AwbcEntryId(0),
        64,
        generation,
    )
    .expect("MustBeQuiescent Need producer verifies");
    let started = executor.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            max_new_task_requests: 1,
            ..RuntimeStepOptions::default()
        },
    );
    let need = executor
        .need_producers
        .launches()
        .next()
        .expect("Need producer was admitted")
        .need()
        .clone();
    assert_eq!(started.output.requests.tasks.len(), 1);
    assert_eq!(
        executor.snapshot_for_save(),
        Err(super::AwbcProductSaveError::NeedsQuiescence { needs: vec![need] })
    );
}

#[test]
fn ready_need_producer_payload_is_checked_before_publication_and_on_restore() {
    let generation = GenerationId::new(7);
    let program = std::sync::Arc::new(need_producer_program(AwbcTaskRestartPolicy::Restartable));
    let mut rejected = AwbcProductStepExecutor::for_entry_arc_with_generation(
        std::sync::Arc::clone(&program),
        AwbcEntryId(0),
        64,
        generation,
    )
    .expect("Restartable Need producer verifies");
    let started = rejected.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            max_new_task_requests: 1,
            ..RuntimeStepOptions::default()
        },
    );
    let task = started.output.requests.tasks[0].clone();
    let invalid_ready = producer_task_event(
        generation,
        task.id.clone(),
        1,
        TaskEventKind::Ready(RuntimePayload(RuntimeValue::Bool(true))),
    );
    let failed = rejected.step(
        RuntimeStepInput {
            task_events: vec![invalid_ready],
            ..RuntimeStepInput::default()
        },
        RuntimeStepOptions::default(),
    );
    assert_eq!(failed.stop_reason, RuntimeStepStopReason::Failed);
    assert!(
        failed
            .output
            .flow_events
            .iter()
            .all(|event| !matches!(event, crate::plan::FlowEvent::AwaitReady { .. }))
    );
    let launch = rejected
        .need_producers
        .launches()
        .next()
        .expect("invalid Ready does not remove the producer");
    assert_eq!(
        launch.state(),
        &crate::task::RuntimeNeedProducerState::NotStarted
    );
    assert!(launch.publication().is_none());
    assert!(!launch.task_terminal());

    let mut accepted = AwbcProductStepExecutor::for_entry_arc_with_generation(
        std::sync::Arc::clone(&program),
        AwbcEntryId(0),
        64,
        generation,
    )
    .expect("second Need producer verifies");
    let started = accepted.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            max_new_task_requests: 1,
            ..RuntimeStepOptions::default()
        },
    );
    let task = started.output.requests.tasks[0].clone();
    let valid_ready = producer_task_event(
        generation,
        task.id,
        1,
        TaskEventKind::Ready(RuntimePayload(RuntimeValue::String("ready".to_owned()))),
    );
    accepted.step(
        RuntimeStepInput {
            task_events: vec![valid_ready],
            ..RuntimeStepInput::default()
        },
        RuntimeStepOptions {
            mode: crate::step::RuntimeStepMode::Drain,
            ..RuntimeStepOptions::default()
        },
    );
    let mut forged = product_snapshot(&accepted);
    forged.need_producers.launches[0].state =
        crate::task::RuntimeNeedProducerState::Ready(RuntimePayload(RuntimeValue::Bool(true)));
    let error = accepted
        .restore_snapshot(forged)
        .expect_err("restored Ready payload must match its selected Need<T>");
    assert!(matches!(
        error,
        AwbcProductStepBuildError::RestoreSnapshot { ref message }
            if message.contains("saved Need producer Ready payload is outside its checked item type")
    ));
}

#[test]
fn await_many_occurrence_frontier_survives_product_save_restore_and_rebind() {
    let generation = GenerationId::new(7);
    let program = std::sync::Arc::new(return_program());
    let mut executor = AwbcProductStepExecutor::for_entry_arc_with_generation(
        std::sync::Arc::clone(&program),
        AwbcEntryId(0),
        64,
        generation,
    )
    .expect("Product executor starts");
    let first = executor
        .fiber
        .take_await_many_invocation(generation)
        .expect("first per-fiber occurrence is allocated");
    let base_task = TaskId("task.await-many-base".to_owned());
    let base_need = NeedId("need.await-many-base".to_owned());
    let first_task = first.task_id(&base_task, 0).expect("item task identity");
    let first_need = first.need_id(&base_need, 0).expect("item Need identity");
    assert_eq!(first.ordinal(), 0);

    let live = executor
        .snapshot_for_save()
        .expect("running fiber occurrence frontier is saveable");
    let saved =
        AwbcProductExecutorSaveSnapshot::from_live(&live).expect("Product save snapshot converts");
    let encoded = serde_json::to_string(&saved).expect("Product snapshot serializes");
    let decoded: AwbcProductExecutorSaveSnapshot =
        serde_json::from_str(&encoded).expect("Product snapshot decodes");
    let restored_live = decoded
        .into_live_for_program(&crate::task::RuntimeProgramOwner::Awbc(
            std::sync::Arc::clone(&program),
        ))
        .expect("fiber occurrence frontier restores");
    let mut restored = AwbcProductStepExecutor::for_entry_arc_with_generation(
        std::sync::Arc::clone(&program),
        AwbcEntryId(0),
        64,
        generation,
    )
    .expect("restore executor starts");
    restored
        .restore_snapshot(restored_live)
        .expect("fiber occurrence frontier validates");
    let next_generation = GenerationId::new(8);
    restored
        .rebind_generation(next_generation)
        .expect("new generation rebinds future occurrences");
    let second = restored
        .fiber
        .take_await_many_invocation(next_generation)
        .expect("next per-fiber occurrence is allocated");

    assert_eq!(second.fiber(), first.fiber());
    assert_eq!(second.generation(), next_generation);
    assert_eq!(second.ordinal(), 1);
    assert_ne!(
        second.task_id(&base_task, 0).expect("second task identity"),
        first_task
    );
    assert_ne!(
        second.need_id(&base_need, 0).expect("second Need identity"),
        first_need
    );
}

#[test]
fn await_many_partial_fanout_survives_restore_when_task_quota_is_exhausted() {
    let generation = GenerationId::new(7);
    let program = await_many_product_program();
    let items = await_many_items();
    let mut executor = await_many_executor(&program, items.clone(), generation);
    let first = executor.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            max_new_task_requests: 1,
            ..RuntimeStepOptions::default()
        },
    );
    assert_eq!(
        first.output.requests.tasks.len(),
        1,
        "stop={:?}, status={:?}, diagnostics={:?}, suspension={:?}",
        first.stop_reason,
        first.fiber_status,
        first.output.diagnostics,
        executor.fiber.suspension
    );
    assert!(first.output.diagnostics.is_empty());
    assert_ne!(first.stop_reason, RuntimeStepStopReason::Failed);
    let first_task = first.output.requests.tasks[0].clone();
    let invocation = match executor
        .fiber
        .suspension
        .as_ref()
        .map(|suspension| &suspension.reason)
    {
        Some(crate::awbc::fiber::FiberSuspensionReason::AwaitMany(state)) => {
            assert_eq!(state.next_index, 1);
            assert_eq!(state.in_flight.len(), 1);
            assert_eq!(state.in_flight[0].task_id, first_task.id.0);
            state.invocation.expect("accepted AwaitMany occurrence")
        }
        _ => panic!("quota leaves the fan-out safely suspended"),
    };

    let live = executor
        .snapshot_for_save()
        .expect("partial AwaitMany continuation is saveable");
    let saved = AwbcProductExecutorSaveSnapshot::from_live(&live)
        .expect("partial AwaitMany Product state serializes");
    let encoded = serde_json::to_string(&saved).expect("Product save serializes");
    let decoded: AwbcProductExecutorSaveSnapshot =
        serde_json::from_str(&encoded).expect("Product save decodes");
    let restored_live = decoded
        .into_live_for_program(&crate::task::RuntimeProgramOwner::Awbc(
            std::sync::Arc::new(program.clone()),
        ))
        .expect("partial AwaitMany continuation restores");
    let mut restored = await_many_executor(&program, items, generation);
    restored
        .restore_snapshot(restored_live)
        .expect("partial AwaitMany identity and links validate");

    let ready = producer_task_event(
        generation,
        first_task.id.clone(),
        1,
        TaskEventKind::Ready(RuntimePayload(RuntimeValue::String("one".to_owned()))),
    );
    let resumed = restored.step(
        RuntimeStepInput {
            task_events: vec![ready],
            ..RuntimeStepInput::default()
        },
        RuntimeStepOptions {
            max_new_task_requests: 2,
            ..RuntimeStepOptions::default()
        },
    );
    assert_eq!(resumed.output.requests.tasks.len(), 2);
    assert_eq!(resumed.output.diagnostics.len(), 1);
    assert_eq!(
        resumed.output.diagnostics[0].message,
        format!("task {} sequence 21 delivered", first_task.id.0)
    );
    assert_ne!(resumed.stop_reason, RuntimeStepStopReason::Failed);
    assert!(
        resumed
            .output
            .requests
            .tasks
            .iter()
            .all(|task| task.id != first_task.id)
    );
    assert_ne!(
        resumed.output.requests.tasks[0].id,
        resumed.output.requests.tasks[1].id
    );
    assert!(matches!(
        restored
            .fiber
            .suspension
            .as_ref()
            .map(|suspension| &suspension.reason),
        Some(crate::awbc::fiber::FiberSuspensionReason::AwaitMany(state))
            if state.invocation == Some(invocation)
                && state.next_index == 3
                && state.in_flight.len() == 2
    ));
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
            input_ownership: Vec::new(),
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
            input_ownership: Vec::new(),
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
            input_ownership: Vec::new(),
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
            value: RuntimeValue::Need(NeedId("need.profile".to_owned())),
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

fn typed_direct_need_executor_and_input(
    need_states: Vec<RuntimeNeedState>,
) -> (AwbcProductStepExecutor, RuntimeStepInput) {
    let executor = AwbcProductStepExecutor::for_function_invocation(
        typed_direct_need_program(),
        AwbcEntryId(0),
        AwbcFunctionId(0),
        [RuntimeFlowParameterBinding {
            parameter: crate::entry::FlowParameterCoordinate::from_position(0),
            value: RuntimeValue::Need(NeedId("need.profile".to_owned())),
        }],
        64,
    )
    .expect("typed Need product executor starts");
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
        max_new_task_requests: usize::MAX,
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
            input_ownership: vec![AwbcFunctionInputOwnership::default()],
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

fn typed_direct_need_program() -> AwbcProgram {
    let mut program = direct_need_program();
    let string_ty = AwbcTypeId(2);
    program.runtime_types[0] = AwbcRuntimeType::new(
        RuntimeSemanticTypeId::from_bytes([91; 32]),
        AwbcRuntimeTypeShape::Need(string_ty),
    );
    program.runtime_types.push(AwbcRuntimeType::new(
        RuntimeSemanticTypeId::from_bytes([92; 32]),
        AwbcRuntimeTypeShape::String,
    ));
    program.signatures[0].result = Some(string_ty);
    program.frame_layouts[0].slots[1].ty = string_ty;
    program.patterns[0] = AwbcPattern::Bind {
        target: AwbcRegisterId(1),
        mutable: false,
        expected: Some(string_ty),
    };
    program
}

fn need_producer_program(restart: AwbcTaskRestartPolicy) -> AwbcProgram {
    let item_ty = AwbcTypeId(1);
    let need_ty = AwbcTypeId(0);
    let item_identity = RuntimeSemanticTypeId::from_bytes([0x91; 32]);
    let contract_bytes = [0x45; 32];
    let contract = crate::task::NeedProducerContractDigest::from_bytes(contract_bytes);
    let site = crate::task::NeedProducerSiteDigest::from_bytes([0x46; 32]);
    let host_contract = crate::step::HostCallContractDigest::from_bytes(contract_bytes);
    let producer_plan = crate::task::NeedProducerTaskPlan::try_new(
        contract,
        site,
        crate::task::NeedProducerRequestProjection::ExternCapability {
            capability: crate::task::HostCapabilityId("probe".to_owned()),
            operation: "read".to_owned(),
            contract: host_contract,
            argument_names: Box::new([]),
        },
        Box::new([]),
        item_identity,
        crate::task::TaskPolicy::JoinSameKey,
        match restart {
            AwbcTaskRestartPolicy::Restartable => crate::task::HostRestartPolicy::Restartable,
            AwbcTaskRestartPolicy::MustBeQuiescent => {
                crate::task::HostRestartPolicy::MustBeQuiescent
            }
        },
        crate::task::TaskClass::Io,
        crate::task::TaskPriority(0),
        crate::task::CancelScopeId("flow".to_owned()),
    )
    .expect("test producer plan is internally consistent");
    let mut program = AwbcProgram {
        strings: vec![
            "entry.main".to_owned(),
            "probe".to_owned(),
            "read".to_owned(),
            "flow".to_owned(),
        ],
        runtime_types: vec![
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x90; 32]),
                AwbcRuntimeTypeShape::Need(item_ty),
            ),
            AwbcRuntimeType::new(item_identity, AwbcRuntimeTypeShape::String),
        ],
        signatures: vec![
            AwbcSignature {
                params: Vec::new(),
                result: Some(item_ty),
                effects: AwbcEffectSetId(0),
            },
            AwbcSignature {
                params: Vec::new(),
                result: None,
                effects: AwbcEffectSetId(0),
            },
        ],
        frame_layouts: vec![AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: need_ty,
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: item_ty,
                    role: AwbcFrameSlotRole::ReturnValue,
                    scope_depth: 0,
                },
            ],
            max_scope_depth: 0,
        }],
        instructions: vec![AwbcInstruction::StartNeed {
            dst: AwbcRegisterId(0),
            plan: crate::awbc::schema::AwbcTaskPlanId(0),
            args: Vec::new(),
        }],
        patterns: vec![AwbcPattern::Bind {
            target: AwbcRegisterId(1),
            mutable: false,
            expected: Some(item_ty),
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
                instructions: AwbcTableRange::new(0, 1),
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
                instructions: AwbcTableRange::new(1, 0),
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
            input_ownership: Vec::new(),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 2),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::empty()
                .with(AwbcFunctionFlag::Deterministic)
                .with(AwbcFunctionFlag::MaySuspend),
        }],
        task_plans: vec![AwbcTaskPlan {
            signature: AwbcSignatureId(1),
            request: AwbcTaskRequestProjection::ExternCapability {
                capability: AwbcStringId(1),
                operation: AwbcStringId(2),
                contract: host_contract,
            },
            class: AwbcTaskClass::Io,
            priority: 0,
            cancel_scope: AwbcStringId(3),
            policy: AwbcTaskPolicy::JoinSameKey,
            payload_type: item_ty,
            arguments: Vec::<AwbcHostArgument>::new(),
            kind: AwbcTaskPlanKind::NeedProducer {
                contract,
                site,
                semantic_digest: producer_plan.semantic_digest(),
                restart,
            },
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
    };
    program.canonicalize_string_table();
    program
}

fn producer_task_event(
    generation: GenerationId,
    task_id: TaskId,
    revision: u64,
    kind: TaskEventKind,
) -> TaskEvent {
    TaskEvent {
        generation,
        logical_epoch: LogicalEpoch(13),
        task_id,
        sequence: TaskSequence(21),
        publication_revision: TaskPublicationRevision::new(
            std::num::NonZeroU64::new(revision).expect("event revision is nonzero"),
        ),
        kind,
    }
}

fn await_many_items() -> RuntimeValue {
    RuntimeValue::Seq(crate::value::RuntimeSeq::values(vec![
        RuntimeValue::String("one".to_owned()),
        RuntimeValue::String("two".to_owned()),
        RuntimeValue::String("three".to_owned()),
    ]))
}

fn await_many_executor(
    program: &AwbcProgram,
    items: RuntimeValue,
    generation: GenerationId,
) -> AwbcProductStepExecutor {
    AwbcProductStepExecutor::for_function_invocation_with_generation(
        program.clone(),
        AwbcEntryId(0),
        AwbcFunctionId(0),
        [RuntimeFlowParameterBinding {
            parameter: crate::entry::FlowParameterCoordinate::from_position(0),
            value: items,
        }],
        64,
        generation,
    )
    .expect("AwaitMany Product executor starts")
}

fn await_many_product_program() -> AwbcProgram {
    let item_ty = AwbcTypeId(1);
    let sequence_ty = AwbcTypeId(2);
    let mut program = AwbcProgram {
        strings: vec![
            "entry.main".to_owned(),
            "probe".to_owned(),
            "read".to_owned(),
            "flow".to_owned(),
            "task.items".to_owned(),
            "need.items".to_owned(),
        ],
        runtime_types: vec![
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x90; 32]),
                AwbcRuntimeTypeShape::Unit,
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x91; 32]),
                AwbcRuntimeTypeShape::String,
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x92; 32]),
                AwbcRuntimeTypeShape::Sequence {
                    kind: crate::plan::RuntimePlanSequenceKind::Vec,
                    item: item_ty,
                },
            ),
        ],
        signatures: vec![
            AwbcSignature {
                params: vec![sequence_ty],
                result: None,
                effects: AwbcEffectSetId(0),
            },
            AwbcSignature {
                params: vec![item_ty],
                result: None,
                effects: AwbcEffectSetId(0),
            },
        ],
        frame_layouts: vec![AwbcFrameLayout {
            scopes: Vec::new(),
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: sequence_ty,
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            }],
            max_scope_depth: 0,
        }],
        resume_points: vec![AwbcResumePoint {
            function: AwbcFunctionId(0),
            block: AwbcBlockId(1),
            frame_layout: AwbcFrameLayoutId(0),
            kind: AwbcSafePointKind::AwaitMany,
        }],
        blocks: vec![
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::AwaitMany {
                    plan: crate::awbc::schema::AwbcTaskPlanId(0),
                    source: AwbcRegisterId(0),
                    binding: None,
                    resume: AwbcResumePointId(0),
                },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            },
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::Return,
                source_map: None,
            },
        ],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            input_ownership: vec![AwbcFunctionInputOwnership::default()],
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 2),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::empty()
                .with(AwbcFunctionFlag::Deterministic)
                .with(AwbcFunctionFlag::MaySuspend),
        }],
        task_plans: vec![AwbcTaskPlan {
            signature: AwbcSignatureId(1),
            request: AwbcTaskRequestProjection::CustomCapability {
                capability: AwbcStringId(1),
                operation: AwbcStringId(2),
            },
            class: AwbcTaskClass::Io,
            priority: 0,
            cancel_scope: AwbcStringId(3),
            policy: AwbcTaskPolicy::JoinSameKey,
            payload_type: item_ty,
            arguments: vec![AwbcHostArgument {
                name: None,
                spread: false,
            }],
            kind: AwbcTaskPlanKind::AwaitMany {
                public_id: AwbcStringId(4),
                need_id: AwbcStringId(5),
                item_binding: AwbcRegisterId(0),
                limit: 3,
            },
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
    };
    program.canonicalize_string_table();
    program
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
