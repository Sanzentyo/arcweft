use crate::line_task::{
    LineCleanupPolicy, LineTaskCleanup, LineTaskGroup, LineTaskLiveState, LineTaskNode, ScopeExit,
    complete_live_line_task_work, finish_live_line_task_group, progress_live_line_task_group,
};
use crate::plan::FlowOp;
use crate::runtime_id::RuntimeLineTaskNodeId;
use crate::time::LogicalDuration;
use std::collections::BTreeSet;

fn group(actions: Vec<FlowOp>, cleanup: Vec<FlowOp>) -> LineTaskGroup {
    LineTaskGroup::new(
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
    let mut state = LineTaskLiveState::new(&group, 41);
    let action = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        &BTreeSet::new(),
        &mut state,
    )
    .commands
    .pop()
    .expect("action starts");

    let closing = finish_live_line_task_group(&group, &mut state);
    assert!(matches!(
        closing.commands.as_slice(),
        [crate::line_task::LineTaskCommand::Cancel { activation, node }]
            if activation.value() == 41 && node.index() == 0
    ));
    assert!(state.is_closing());

    let cleanup = complete_live_line_task_work(&group, &mut state, action_tag(&action), false)
        .commands
        .pop()
        .expect("cleanup starts after joined action");
    let cleanup_tag = action_tag(&cleanup);
    assert!(matches!(
        cleanup_tag.work,
        crate::line_task::LineTaskWork::Cleanup(ScopeExit::Completed)
    ));

    assert!(
        complete_live_line_task_work(&group, &mut state, cleanup_tag, false)
            .commands
            .is_empty()
    );
    assert!(state.is_closed());
}

#[test]
fn stale_activation_completion_is_ignored() {
    let group = group(vec![FlowOp::Noop], Vec::new());
    let mut state = LineTaskLiveState::new(&group, 7);
    let action = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        &BTreeSet::new(),
        &mut state,
    )
    .commands
    .pop()
    .expect("action starts");
    let mut action = action_tag(&action);
    action.activation = crate::line_task::LineTaskActivationId::from_value(8);

    assert!(
        complete_live_line_task_work(&group, &mut state, action, false)
            .commands
            .is_empty()
    );
    assert!(!state.is_closing());
    assert_eq!(
        state.node_state(RuntimeLineTaskNodeId::from_zero_based(0).expect("zero node id")),
        Some(crate::line_task::LineTaskNodeState::Running)
    );
}

#[test]
fn start_keeps_nested_sequence_active_after_parent_completes() {
    let node = |index| RuntimeLineTaskNodeId::from_zero_based(index).expect("node id");
    let group = LineTaskGroup::new(
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
    let mut state = LineTaskLiveState::new(&group, 99);
    let first = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        &BTreeSet::new(),
        &mut state,
    )
    .commands
    .pop()
    .expect("first nested action starts");
    assert_eq!(
        state.node_state(node(0)),
        Some(crate::line_task::LineTaskNodeState::Completed)
    );

    let next = complete_live_line_task_work(&group, &mut state, action_tag(&first), false)
        .commands
        .pop()
        .expect("active nested root starts successor");
    assert!(
        matches!(action_tag(&next).work, crate::line_task::LineTaskWork::Node(id) if id == node(3))
    );
}

#[test]
fn sequence_failure_immediately_cancels_later_siblings() {
    let node = |index| RuntimeLineTaskNodeId::from_zero_based(index).expect("node id");
    let group = LineTaskGroup::new(
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
    let mut state = LineTaskLiveState::new(&group, 100);
    let first = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        &BTreeSet::new(),
        &mut state,
    )
    .commands
    .pop()
    .expect("first action starts");
    let _ = complete_live_line_task_work(&group, &mut state, action_tag(&first), true);

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
    let mut state = LineTaskLiveState::new(&group, 13);
    let _ = progress_live_line_task_group(
        &group,
        LogicalDuration::default(),
        &BTreeSet::new(),
        &mut state,
    );
    let snapshot = state.snapshot();
    let restored = LineTaskLiveState::restore(&group, snapshot.clone()).expect("valid snapshot");
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn reducer_restore_rejects_tampered_node_state_count() {
    let group = group(Vec::new(), Vec::new());
    let snapshot = crate::line_task::LineTaskLiveSnapshot::new(
        1,
        crate::line_task::LineTaskPhase::Active,
        Box::default(),
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

fn action_tag(command: &crate::line_task::LineTaskCommand) -> crate::line_task::LineTaskWorkTag {
    match command {
        crate::line_task::LineTaskCommand::Run { tag, .. } => *tag,
        crate::line_task::LineTaskCommand::Cancel { .. } => panic!("expected run command"),
    }
}
