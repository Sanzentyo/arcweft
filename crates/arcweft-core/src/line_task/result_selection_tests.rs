use super::{
    ChildCancelPolicy, ChildJoinPolicy, LineCancelRule, LineRuntimeError, LineTaskCleanup,
    LineTaskGroup, LineTaskLiveState, LineTaskNode, LineTaskReadyEvents, LineTaskTrigger,
    LineTaskWork, LineTaskWorkTag, RuntimeDialogueActivationState, RuntimeDialogueContentEventKind,
    RuntimeDialogueResultState, RuntimeHandleOwnerSlot, RuntimeHandleResource, RuntimeVoiceLease,
    cancel_live_line_task_group, complete_live_line_task_work, progress_live_line_task_group,
};
use crate::effect::RuntimeArtifactFingerprint;
use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId};
use crate::plan::FlowOp;
use crate::presentation::RuntimeVoiceSessionId;
use crate::runtime_id::{
    DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeDialogueMarkId,
    RuntimeLineHandleSiteId, RuntimeLineHandleToken, RuntimeLineTaskNodeId,
    RuntimePersistentFiberId, RuntimePlanTypeId,
};
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeValue,
};
use arcweft_interaction_model::input::InputActionId;
use std::collections::BTreeSet;
use std::num::NonZeroU32;

fn activation() -> DialogueActivationId {
    DialogueActivationId::new(
        RuntimeArtifactFingerprint::try_from_bytes([0x72; 32]).expect("artifact"),
        RuntimePersistentFiberId::from_allocated(1),
        RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
        1,
    )
}

fn node(index: usize) -> RuntimeLineTaskNodeId {
    RuntimeLineTaskNodeId::from_zero_based(index).expect("node")
}

fn mark_group() -> (LineTaskGroup, RuntimeDialogueMarkId, RuntimePlanTypeId) {
    let mark = RuntimeDialogueMarkId::from_zero_based(0).expect("mark");
    let ty = RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
    let group = LineTaskGroup::new(
        Box::new([]),
        Box::new([]),
        ty,
        Box::new([]),
        node(0),
        vec![
            LineTaskNode::Sequence(vec![node(1)].into_boxed_slice()),
            LineTaskNode::Child {
                trigger: LineTaskTrigger::Mark(mark),
                join_policy: ChildJoinPolicy::Join,
                cancel_policy: ChildCancelPolicy::CancelAndJoin,
                scope: node(2),
            },
            LineTaskNode::Action(vec![FlowOp::Noop].into_boxed_slice()),
        ]
        .into_boxed_slice(),
        Box::new([]),
        LineTaskCleanup::default(),
    );
    (group, mark, ty)
}

#[test]
fn mark_selection_is_reducer_authorized_and_restores_only_after_completed_work() {
    let (group, mark, ty) = mark_group();
    let activation = activation();
    let mut reducer = LineTaskLiveState::new(&group, activation.clone());
    let marks = BTreeSet::new();
    progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        LineTaskReadyEvents::new(&marks),
        &mut reducer,
    )
    .expect("arm mark");
    let accepted = reducer
        .accept_content_event_kinds(&[RuntimeDialogueContentEventKind::Mark(mark)], |_| true)
        .expect("consume mark");
    let commands = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        accepted.ready(),
        &mut reducer,
    )
    .expect("start mark child");
    let tag = commands
        .commands
        .into_iter()
        .find_map(|command| match command {
            super::LineTaskCommand::Run { tag, .. } => Some(tag),
            super::LineTaskCommand::Cancel { .. } => None,
        })
        .expect("joined mark action");
    assert_eq!(tag.work(), LineTaskWork::Node(node(2)));
    assert!(reducer.accepts_result_selection(&group, &tag));
    assert!(!reducer.accepts_result_selection(
        &group,
        &LineTaskWorkTag::activation(activation.clone(), LineTaskWork::Node(node(0))),
    ));

    let mut result = RuntimeDialogueActivationState::<RuntimePlanTypeId>::new();
    result
        .select_result(&tag, ty, RuntimeValue::String("released".to_owned()))
        .expect("select mark result without a pre-reveal normal result");
    assert_eq!(
        result.restore_admit_reducer(&activation, &group, &reducer.snapshot()),
        Err(LineRuntimeError::InvalidRestoredResultState),
        "an outstanding handler cannot be restored as a completed selection",
    );
    complete_live_line_task_work(&group, &mut reducer, tag.clone(), false)
        .expect("complete mark handler");
    result
        .restore_admit_reducer(&activation, &group, &reducer.snapshot())
        .expect("completed mark selection is restorable");
    let mut wrong_source = RuntimeDialogueActivationState::<RuntimePlanTypeId>::new();
    wrong_source
        .select_result(
            &LineTaskWorkTag::activation(activation.clone(), LineTaskWork::Node(node(0))),
            ty,
            RuntimeValue::String("forged".to_owned()),
        )
        .expect("construct an invalid persisted source");
    assert_eq!(
        wrong_source.restore_admit_reducer(&activation, &group, &reducer.snapshot()),
        Err(LineRuntimeError::InvalidRestoredResultState),
    );
    assert_eq!(
        result.result(),
        &RuntimeDialogueResultState::Selected {
            ty,
            value: RuntimeValue::String("released".to_owned()),
            source: tag,
        }
    );
}

#[test]
fn cancellation_can_supersede_one_mark_selection_without_mutating_on_rejection() {
    let (_, _, ty) = mark_group();
    let activation = activation();
    let mark = LineTaskWorkTag::activation(activation.clone(), LineTaskWork::Node(node(2)));
    let cancellation = LineTaskWorkTag::activation(
        activation,
        LineTaskWork::Cancellation(InputActionId::new("SkipLine").expect("action")),
    );
    let mut result = RuntimeDialogueActivationState::<RuntimePlanTypeId>::new();
    assert_eq!(
        result.begin_result_publication(),
        Err(LineRuntimeError::InvalidResultTransition),
        "an absent normal or selected result cannot publish",
    );
    result
        .select_result(&mark, ty, RuntimeValue::String("mark".to_owned()))
        .expect("mark selects from uncommitted");
    let selected_mark = result.clone();
    assert_eq!(
        result.select_result(&mark, ty, RuntimeValue::String("second".to_owned())),
        Err(LineRuntimeError::InvalidResultTransition),
    );
    assert_eq!(result, selected_mark);
    result
        .select_result(
            &cancellation,
            ty,
            RuntimeValue::String("cancelled".to_owned()),
        )
        .expect("cancellation overrides pending mark result");
    assert_eq!(
        result.result(),
        &RuntimeDialogueResultState::Selected {
            ty,
            value: RuntimeValue::String("cancelled".to_owned()),
            source: cancellation,
        }
    );
}

#[test]
fn selected_affine_handle_leaves_child_custody_before_child_scope_closes() {
    let (_, _, ty) = mark_group();
    let activation = activation();
    let tag = LineTaskWorkTag::activation(activation.clone(), LineTaskWork::Node(node(2)));
    let kind = RuntimeHandleKind::Voice;
    let owner = RuntimeOpaqueTypeOwner::exact_with(
        kind.try_producer().expect("voice producer"),
        RuntimeSemanticTypeId::from_bytes([0x72; 32]),
        RuntimeOpaqueValueClass::AffineHandle(kind),
        RuntimeOpaquePersistence::SnapshotOnly,
    );
    let mut result = RuntimeDialogueActivationState::<RuntimePlanTypeId>::new();
    let mut ledger = result.ledger().clone();
    let value = ledger
        .issue_exact(
            &activation,
            RuntimeLineHandleSiteId::from_zero_based(0),
            kind,
            &owner,
            RuntimeHandleResource::Voice(RuntimeVoiceLease::new(
                RuntimeVoiceSessionId::try_new("selected voice").expect("voice session"),
                0,
                false,
            )),
            RuntimeHandleOwnerSlot::ChildScope(tag.clone()),
        )
        .expect("issue child-owned affine handle");
    let token = RuntimeLineHandleToken::try_decode_payload(value.payload()).expect("token");
    result.commit_ledger(ledger);
    result
        .select_result(&tag, ty, RuntimeValue::Opaque(value))
        .expect("transfer selected affine custody");
    assert!(matches!(
        result
            .ledger()
            .lease(&token)
            .expect("selected lease")
            .owner(),
        RuntimeHandleOwnerSlot::DialogueResult(_)
    ));
    result
        .finish_child_scope(
            &tag,
            &BTreeSet::new(),
            &BTreeSet::new(),
            crate::effect::RuntimeDropPolicy::Default,
        )
        .expect("child closes after selected token leaves its custody");
    assert!(matches!(
        result
            .ledger()
            .lease(&token)
            .expect("retained lease")
            .owner(),
        RuntimeHandleOwnerSlot::DialogueResult(_)
    ));
}

#[test]
fn cancellation_selection_restores_only_after_its_joined_handler_completes() {
    let (_, _, ty) = mark_group();
    let activation = activation();
    let action = InputActionId::new("SkipLine").expect("action");
    let group = LineTaskGroup::new(
        Box::new([]),
        Box::new([]),
        ty,
        Box::new([]),
        node(0),
        vec![LineTaskNode::Sequence(Box::new([]))].into_boxed_slice(),
        vec![LineCancelRule::new(
            action.clone(),
            vec![FlowOp::Noop].into_boxed_slice(),
        )]
        .into_boxed_slice(),
        LineTaskCleanup::default(),
    );
    let mut reducer = LineTaskLiveState::new(&group, activation.clone());
    let input_actions = [action];
    let (_, commands) = cancel_live_line_task_group(
        &group,
        LineTaskReadyEvents::new(&BTreeSet::new()).with_input_actions(&input_actions),
        &mut reducer,
    )
    .expect("route cancellation");
    let tag = commands
        .commands
        .into_iter()
        .find_map(|command| match command {
            super::LineTaskCommand::Run { tag, .. } => Some(tag),
            super::LineTaskCommand::Cancel { .. } => None,
        })
        .expect("joined cancellation handler");
    assert!(reducer.accepts_result_selection(&group, &tag));
    let mut result = RuntimeDialogueActivationState::<RuntimePlanTypeId>::new();
    result
        .select_result(&tag, ty, RuntimeValue::String("cancelled".to_owned()))
        .expect("select cancellation result");
    assert_eq!(
        result.restore_admit_reducer(&activation, &group, &reducer.snapshot()),
        Err(LineRuntimeError::InvalidRestoredResultState),
    );
    complete_live_line_task_work(&group, &mut reducer, tag, false)
        .expect("complete cancellation handler");
    result
        .restore_admit_reducer(&activation, &group, &reducer.snapshot())
        .expect("completed cancellation selection restores");
}
