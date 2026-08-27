use crate::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineCleanupPolicy, LineTaskCleanup,
    LineTaskCompletionError, LineTaskExitPolicy, LineTaskGroup, LineTaskLiveState, LineTaskNode,
    LineTaskNodeView, LineTaskPlanView, LineTaskReadyEvents, LineTaskTrigger, ScopeExit,
    complete_live_line_task_work, finish_live_line_task_group, progress_live_line_task_group,
};
use crate::plan::FlowOp;
use crate::runtime_id::{
    DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeDialogueEffectSiteId,
    RuntimeLineHandleSiteId, RuntimeLineHandleToken, RuntimeLineTaskNodeId,
    RuntimePersistentFiberId, RuntimePlanTypeId,
};
use crate::step::RuntimeDialogueContentEventKind;
use crate::time::LogicalDuration;
use std::collections::BTreeSet;
use std::num::NonZeroU32;

fn activation(occurrence: u64) -> DialogueActivationId {
    DialogueActivationId::new(
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x51; 32])
            .expect("fixture artifact"),
        RuntimePersistentFiberId::from_allocated(1),
        RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
        occurrence,
    )
}

fn result_type() -> RuntimePlanTypeId {
    RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN)
}

fn group(actions: Vec<FlowOp>, cleanup: Vec<FlowOp>) -> LineTaskGroup {
    LineTaskGroup::new(
        Box::default(),
        Box::default(),
        result_type(),
        Box::default(),
        RuntimeLineTaskNodeId::from_zero_based(0).expect("zero node id"),
        vec![LineTaskNode::Action(actions.into_boxed_slice())].into_boxed_slice(),
        Box::default(),
        LineTaskCleanup::new(
            cleanup.into_boxed_slice(),
            Box::default(),
            Box::default(),
            LineCleanupPolicy::default(),
        ),
    )
}

#[test]
fn finish_waits_for_joined_action_before_running_cleanup() {
    let group = group(vec![FlowOp::Noop], vec![FlowOp::Noop]);
    let mut state = LineTaskLiveState::new(&group, activation(41));
    let action = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        LineTaskReadyEvents::new(&BTreeSet::new(), &BTreeSet::new()),
        &mut state,
    )
    .expect("progress")
    .commands
    .pop()
    .expect("action starts");

    let closing = finish_live_line_task_group(&group, &mut state);
    assert!(matches!(
        closing.commands.as_slice(),
        [crate::line_task::LineTaskCommand::Cancel { tag }]
            if tag.activation_id().occurrence() == 41
                && tag.work()
                    == crate::line_task::LineTaskWork::Node(
                        RuntimeLineTaskNodeId::from_zero_based(0).expect("zero node id")
                    )
    ));
    assert!(state.is_closing());

    let cleanup = complete_live_line_task_work(&group, &mut state, action_tag(&action), false)
        .expect("joined action completion")
        .commands
        .pop()
        .expect("cleanup starts after joined action");
    let cleanup_tag = action_tag(&cleanup);
    assert!(matches!(
        cleanup_tag.work(),
        crate::line_task::LineTaskWork::Cleanup(ScopeExit::Completed)
    ));

    assert!(
        complete_live_line_task_work(&group, &mut state, cleanup_tag, false)
            .expect("cleanup completion")
            .commands
            .is_empty()
    );
    assert!(state.is_closed());
}

#[test]
fn stale_activation_completion_is_rejected_without_reducer_mutation() {
    let group = group(vec![FlowOp::Noop], Vec::new());
    let mut state = LineTaskLiveState::new(&group, activation(7));
    let action = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        LineTaskReadyEvents::new(&BTreeSet::new(), &BTreeSet::new()),
        &mut state,
    )
    .expect("progress")
    .commands
    .pop()
    .expect("action starts");
    let action = action_tag(&action);
    let action = crate::line_task::LineTaskWorkTag::activation(activation(8), action.work());

    assert!(matches!(
        complete_live_line_task_work(&group, &mut state, action, false),
        Err(LineTaskCompletionError::StaleActivation { .. })
    ));
    assert!(
        !state.is_closing(),
        "rejecting stale completion preserves phase"
    );
    assert_eq!(
        state.node_state(RuntimeLineTaskNodeId::from_zero_based(0).expect("zero node id")),
        Some(crate::line_task::LineTaskNodeState::Running)
    );
}

#[test]
fn duplicate_completion_is_rejected_without_reducer_mutation() {
    let group = group(vec![FlowOp::Noop], Vec::new());
    let mut state = LineTaskLiveState::new(&group, activation(9));
    let action = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        LineTaskReadyEvents::new(&BTreeSet::new(), &BTreeSet::new()),
        &mut state,
    )
    .expect("progress")
    .commands
    .pop()
    .expect("action starts");
    let tag = action_tag(&action);
    complete_live_line_task_work(&group, &mut state, tag.clone(), false).expect("first completion");
    let after_first = state.snapshot();

    assert_eq!(
        complete_live_line_task_work(&group, &mut state, tag.clone(), false),
        Err(LineTaskCompletionError::UnknownOrDuplicateWork { tag })
    );
    assert_eq!(state.snapshot(), after_first);
}

#[test]
fn start_keeps_nested_sequence_active_after_parent_completes() {
    let node = |index| RuntimeLineTaskNodeId::from_zero_based(index).expect("node id");
    let group = LineTaskGroup::new(
        Box::default(),
        Box::default(),
        result_type(),
        Box::default(),
        node(0),
        vec![
            LineTaskNode::Start(vec![node(1)].into_boxed_slice()),
            LineTaskNode::Sequence(vec![node(2), node(3)].into_boxed_slice()),
            LineTaskNode::Action(vec![FlowOp::Noop].into_boxed_slice()),
            LineTaskNode::Action(vec![FlowOp::Noop].into_boxed_slice()),
        ]
        .into_boxed_slice(),
        Box::default(),
        LineTaskCleanup::default(),
    );
    let mut state = LineTaskLiveState::new(&group, activation(99));
    let first = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        LineTaskReadyEvents::new(&BTreeSet::new(), &BTreeSet::new()),
        &mut state,
    )
    .expect("progress")
    .commands
    .pop()
    .expect("first nested action starts");
    assert_eq!(
        state.node_state(node(0)),
        Some(crate::line_task::LineTaskNodeState::Completed)
    );

    let next = complete_live_line_task_work(&group, &mut state, action_tag(&first), false)
        .expect("first action completion")
        .commands
        .pop()
        .expect("active nested root starts successor");
    assert!(
        matches!(action_tag(&next).work(), crate::line_task::LineTaskWork::Node(id) if id == node(3))
    );
}

#[test]
fn sequence_failure_immediately_cancels_later_siblings() {
    let node = |index| RuntimeLineTaskNodeId::from_zero_based(index).expect("node id");
    let group = LineTaskGroup::new(
        Box::default(),
        Box::default(),
        result_type(),
        Box::default(),
        node(0),
        vec![
            LineTaskNode::Sequence(vec![node(1), node(2)].into_boxed_slice()),
            LineTaskNode::Action(vec![FlowOp::Noop].into_boxed_slice()),
            LineTaskNode::Action(vec![FlowOp::Noop].into_boxed_slice()),
        ]
        .into_boxed_slice(),
        Box::default(),
        LineTaskCleanup::default(),
    );
    let mut state = LineTaskLiveState::new(&group, activation(100));
    let first = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        LineTaskReadyEvents::new(&BTreeSet::new(), &BTreeSet::new()),
        &mut state,
    )
    .expect("progress")
    .commands
    .pop()
    .expect("first action starts");
    complete_live_line_task_work(&group, &mut state, action_tag(&first), true)
        .expect("failed action completion");

    assert_eq!(
        state.node_state(node(0)),
        Some(crate::line_task::LineTaskNodeState::Failed)
    );
    assert_eq!(
        state.node_state(node(2)),
        Some(crate::line_task::LineTaskNodeState::Cancelling)
    );
}

#[test]
fn reducer_snapshot_round_trips_through_owner_validation() {
    let group = group(vec![FlowOp::Noop], Vec::new());
    let mut state = LineTaskLiveState::new(&group, activation(13));
    let _ = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        LineTaskReadyEvents::new(&BTreeSet::new(), &BTreeSet::new()),
        &mut state,
    )
    .expect("progress");
    let snapshot = state.snapshot();
    let restored = LineTaskLiveState::restore(&group, snapshot.clone()).expect("valid snapshot");
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn reducer_restore_rejects_tampered_node_state_count() {
    let group = group(Vec::new(), Vec::new());
    let snapshot = crate::line_task::LineTaskLiveSnapshot::new(
        activation(1),
        crate::line_task::LineTaskPhase::Active,
        crate::line_task::LineTaskExecutionLaneSnapshot::new(
            Box::default(),
            Box::default(),
            Box::default(),
            Box::default(),
        ),
        Box::default(),
        Box::default(),
        Box::default(),
        false,
    );
    assert!(matches!(
        LineTaskLiveState::restore(&group, snapshot),
        Err(crate::line_task::LineTaskSnapshotError::NodeStateCount { .. })
    ));
}

#[test]
fn content_event_kind_batch_rejects_duplicate_without_mutation() {
    let group = group(Vec::new(), Vec::new());
    let expected = activation(21);
    let mut state = LineTaskLiveState::new(&group, expected.clone());
    let site = RuntimeDialogueEffectSiteId::from_zero_based(0).expect("effect site");
    let event = RuntimeDialogueContentEventKind::Effect(site);
    let before = state.snapshot();

    assert_eq!(
        state.accept_content_event_kinds(&[event, event], |_| true),
        Err(crate::line_task::LineRuntimeError::DuplicateContentEvent {
            event: RuntimeDialogueContentEventKind::Effect(site),
        }),
    );
    assert_eq!(state.snapshot(), before);
}

#[test]
fn consumed_content_event_survives_snapshot_restore_and_rejects_replay() {
    let group = group(Vec::new(), Vec::new());
    let id = activation(23);
    let mut state = LineTaskLiveState::new(&group, id.clone());
    let site = RuntimeDialogueEffectSiteId::from_zero_based(0).expect("effect site");
    let event = RuntimeDialogueContentEventKind::Effect(site);
    state
        .accept_content_event_kinds(std::slice::from_ref(&event), |_| true)
        .expect("first event is accepted");
    let mut restored = LineTaskLiveState::restore(&group, state.snapshot()).expect("restore");

    assert_eq!(
        restored.accept_content_event_kinds(&[event], |_| true),
        Err(crate::line_task::LineRuntimeError::ConsumedContentEvent {
            event: RuntimeDialogueContentEventKind::Effect(site),
        }),
    );
}

fn action_tag(command: &crate::line_task::LineTaskCommand) -> crate::line_task::LineTaskWorkTag {
    match command {
        crate::line_task::LineTaskCommand::Run { tag, .. } => tag.clone(),
        crate::line_task::LineTaskCommand::Cancel { .. } => panic!("expected run command"),
    }
}

struct ScheduledPlan {
    root_children: Box<[RuntimeLineTaskNodeId]>,
    action_present: bool,
}

impl ScheduledPlan {
    fn new(action_present: bool) -> Self {
        Self {
            root_children: vec![scheduled_node(1)].into_boxed_slice(),
            action_present,
        }
    }
}

impl LineTaskPlanView for ScheduledPlan {
    fn node_count(&self) -> usize {
        4
    }

    fn root_node(&self) -> RuntimeLineTaskNodeId {
        scheduled_node(0)
    }

    fn node_view(&self, id: RuntimeLineTaskNodeId) -> Option<LineTaskNodeView<'_>> {
        match id.index() {
            0 => Some(LineTaskNodeView::Start(&self.root_children)),
            1 => Some(LineTaskNodeView::Child {
                trigger: LineTaskTrigger::Scheduled(scheduled_site()),
                policy: LineTaskExitPolicy {
                    join: ChildJoinPolicy::Join,
                    cancel: ChildCancelPolicy::CancelAndJoin,
                },
                scope: scheduled_node(2),
            }),
            2 | 3 => Some(LineTaskNodeView::Action),
            _ => None,
        }
    }

    fn has_action(&self, id: RuntimeLineTaskNodeId) -> bool {
        id == scheduled_node(2) && self.action_present
    }

    fn cancellation_mark(
        &self,
        _marks: &BTreeSet<crate::runtime_id::RuntimeDialogueMarkId>,
    ) -> Option<crate::runtime_id::RuntimeDialogueMarkId> {
        None
    }

    fn has_cancellation_work(&self, _mark: crate::runtime_id::RuntimeDialogueMarkId) -> bool {
        false
    }

    fn has_cleanup(&self, _exit: ScopeExit) -> bool {
        false
    }

    fn scheduled_child(&self, site: RuntimeLineHandleSiteId) -> Option<RuntimeLineTaskNodeId> {
        (site == scheduled_site()).then(|| scheduled_node(1))
    }
}

fn scheduled_node(index: usize) -> RuntimeLineTaskNodeId {
    RuntimeLineTaskNodeId::from_zero_based(index).expect("line-task node")
}

fn scheduled_site() -> RuntimeLineHandleSiteId {
    RuntimeLineHandleSiteId::from_zero_based(0)
}

fn scheduled_token(id: &DialogueActivationId, issuance: u32) -> RuntimeLineHandleToken {
    RuntimeLineHandleToken::new(id.clone(), scheduled_site(), issuance)
}

#[test]
fn same_scheduled_site_uses_distinct_runtime_lanes_and_round_trips() {
    let plan = ScheduledPlan::new(true);
    let id = activation(301);
    let first = scheduled_token(&id, 0);
    let second = scheduled_token(&id, 1);
    let mut state = LineTaskLiveState::new(&plan, id);
    state
        .mark_scheduled_ready(first.clone())
        .expect("first runtime instance");
    state
        .mark_scheduled_ready(second.clone())
        .expect("second runtime instance");

    let activation = progress_live_line_task_group(
        &plan,
        LogicalDuration::default(),
        LineTaskReadyEvents::new(&BTreeSet::new(), &BTreeSet::new()),
        &mut state,
    )
    .expect("both runtime instances progress");
    let tags = activation
        .commands
        .iter()
        .map(action_tag)
        .collect::<Vec<_>>();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].scheduled_token(), Some(&first));
    assert_eq!(tags[1].scheduled_token(), Some(&second));

    let snapshot = state.snapshot();
    assert_eq!(snapshot.scheduled_lanes().len(), 2);
    let restored =
        LineTaskLiveState::restore(&plan, snapshot.clone()).expect("exact lanes restore");
    assert_eq!(restored.snapshot(), snapshot);

    complete_live_line_task_work(&plan, &mut state, tags[0].clone(), false)
        .expect("first instance completes independently");
    assert_eq!(state.snapshot().scheduled_lanes().len(), 1);
    assert_eq!(state.snapshot().scheduled_lanes()[0].token(), &second);
}

#[test]
fn empty_scheduled_callback_completes_without_a_synthetic_child() {
    let plan = ScheduledPlan::new(false);
    let id = activation(302);
    let token = scheduled_token(&id, 0);
    let mut state = LineTaskLiveState::new(&plan, id);
    state
        .mark_scheduled_ready(token.clone())
        .expect("runtime instance is ready");

    let activation = progress_live_line_task_group(
        &plan,
        LogicalDuration::default(),
        LineTaskReadyEvents::new(&BTreeSet::new(), &BTreeSet::new()),
        &mut state,
    )
    .expect("empty callback is a completed reducer instance");
    assert!(activation.commands.is_empty());
    assert_eq!(activation.scheduled_completions.len(), 1);
    assert_eq!(activation.scheduled_completions[0].token(), &token);
    assert!(state.snapshot().scheduled_lanes().is_empty());
}

#[test]
fn scheduled_lane_restore_rejects_state_outside_its_static_subtree() {
    let plan = ScheduledPlan::new(true);
    let id = activation(303);
    let token = scheduled_token(&id, 0);
    let mut state = LineTaskLiveState::new(&plan, id.clone());
    state
        .mark_scheduled_ready(token.clone())
        .expect("runtime instance is ready");
    progress_live_line_task_group(
        &plan,
        LogicalDuration::default(),
        LineTaskReadyEvents::new(&BTreeSet::new(), &BTreeSet::new()),
        &mut state,
    )
    .expect("runtime instance starts");
    let live = state.snapshot();
    let scheduled = &live.scheduled_lanes()[0];
    let mut node_states = scheduled.lane().node_states().to_vec();
    node_states[scheduled_node(3).index()] = crate::line_task::LineTaskNodeState::Running;
    let tampered_lane = crate::line_task::LineTaskExecutionLaneSnapshot::new(
        node_states.into_boxed_slice(),
        scheduled.lane().outstanding().to_vec().into_boxed_slice(),
        scheduled.lane().active_roots().to_vec().into_boxed_slice(),
        scheduled
            .lane()
            .cancelling_nodes()
            .to_vec()
            .into_boxed_slice(),
    );
    let tampered = crate::line_task::LineTaskLiveSnapshot::new(
        id,
        live.phase(),
        crate::line_task::LineTaskExecutionLaneSnapshot::new(
            live.node_states().to_vec().into_boxed_slice(),
            live.outstanding().to_vec().into_boxed_slice(),
            live.active_roots().to_vec().into_boxed_slice(),
            live.cancelling_nodes().to_vec().into_boxed_slice(),
        ),
        vec![crate::line_task::LineTaskScheduledLaneSnapshot::new(
            token,
            tampered_lane,
        )]
        .into_boxed_slice(),
        live.scheduled_ready().to_vec().into_boxed_slice(),
        live.consumed_content_events().to_vec().into_boxed_slice(),
        live.cleanup_started(),
    );

    assert!(matches!(
        LineTaskLiveState::restore(&plan, tampered),
        Err(crate::line_task::LineTaskSnapshotError::InvalidScheduledLane { .. })
    ));
}
