use super::{
    ActiveChoice, ActiveDialogue, AwbcProductStepBuildError, AwbcProductStepExecutor,
    PendingHostCall, ProductChildFiber, ProductChildFiberOwner, ProductLineTaskFiberPhase,
    stream_id_for,
};
mod task_publication;
use crate::awbc::fiber::{AwbcFiberStateSnapshot, FiberState};
use crate::awbc::schema::{
    AwbcChoiceId, AwbcContentUnitId, AwbcDialogueResultTarget, AwbcFlowBinding, AwbcFunctionKind,
    AwbcHostCallId, AwbcStreamPlanId, AwbcTypeId,
};
use crate::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineTaskActiveRoot, LineTaskExecutionLaneSnapshot,
    LineTaskExitPolicy, LineTaskLiveSnapshot, LineTaskLiveState, LineTaskNodeState, LineTaskPhase,
    LineTaskScheduledLaneSnapshot, LineTaskWork, LineTaskWorkInstance, LineTaskWorkTag, ScopeExit,
};
use crate::observation::RuntimeObservationState;
use crate::runtime_id::{
    DialogueActivationId, RuntimeDialogueMarkId, RuntimeLineHandleToken, RuntimeLineTaskNodeId,
};
use crate::step::{RuntimeDialogueContentEventKind, RuntimeHostCallId};
use crate::stream::StreamRuntimeState;
use crate::task::{TaskEvent, TaskId, TaskPublicationCursor};
use crate::value::RuntimePayload;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub use task_publication::{
    AwbcProductTaskEventKindSaveSnapshot, AwbcProductTaskEventSaveSnapshot,
};

fn snapshot_exit(exit: ScopeExit) -> AwbcProductLineTaskExitSnapshot {
    match exit {
        ScopeExit::Completed => AwbcProductLineTaskExitSnapshot::Completed,
        ScopeExit::Cancelled => AwbcProductLineTaskExitSnapshot::Cancelled,
        ScopeExit::Failed => AwbcProductLineTaskExitSnapshot::Failed,
    }
}

fn restore_exit(exit: AwbcProductLineTaskExitSnapshot) -> ScopeExit {
    match exit {
        AwbcProductLineTaskExitSnapshot::Completed => ScopeExit::Completed,
        AwbcProductLineTaskExitSnapshot::Cancelled => ScopeExit::Cancelled,
        AwbcProductLineTaskExitSnapshot::Failed => ScopeExit::Failed,
    }
}

fn validate_product_dialogue_phase(
    activation: &DialogueActivationId,
    frame: &super::ActiveDialogue,
    line: &crate::line_task::RuntimeDialogueActivationState<AwbcTypeId>,
) -> Result<(), crate::line_task::RuntimeDialogueRegistrySnapshotError> {
    use crate::line_task::RuntimeDialogueResultState;

    let valid = activation == &frame.activation
        && match (&frame.phase, line.result()) {
            (
                super::ProductDialoguePhase::Activating { .. },
                RuntimeDialogueResultState::Uncommitted,
            ) => frame.pending_content_events.is_empty() && !frame.pending_advance,
            (
                super::ProductDialoguePhase::Reducing { line_task },
                RuntimeDialogueResultState::Committed { ty, .. },
            ) => !line_task.is_closing() && !line_task.is_closed() && *ty == frame.result.ty,
            (
                super::ProductDialoguePhase::Publishing { line_task },
                RuntimeDialogueResultState::Publishing { ty, .. },
            ) => {
                line_task.is_closed()
                    && *ty == frame.result.ty
                    && frame.pending_content_events.is_empty()
                    && !frame.pending_advance
            }
            (super::ProductDialoguePhase::Closing(_), RuntimeDialogueResultState::Abandoned) => {
                frame.pending_content_events.is_empty() && !frame.pending_advance
            }
            (
                super::ProductDialoguePhase::Activating { .. }
                | super::ProductDialoguePhase::Reducing { .. }
                | super::ProductDialoguePhase::Publishing { .. }
                | super::ProductDialoguePhase::Closing(_),
                RuntimeDialogueResultState::Uncommitted
                | RuntimeDialogueResultState::Committed { .. }
                | RuntimeDialogueResultState::Publishing { .. }
                | RuntimeDialogueResultState::Published
                | RuntimeDialogueResultState::Abandoned,
            ) => false,
        };
    valid.then_some(()).ok_or_else(|| {
        crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
            message: "dialogue frame phase/result authority is not isomorphic".to_owned(),
        }
    })
}

fn snapshot_work(work: LineTaskWork) -> AwbcProductLineTaskWorkSnapshot {
    match work {
        LineTaskWork::Node(node) => {
            AwbcProductLineTaskWorkSnapshot::Node(snapshot_node_index(node))
        }
        LineTaskWork::Cancellation(mark) => AwbcProductLineTaskWorkSnapshot::Cancellation(mark),
        LineTaskWork::Cleanup(exit) => {
            AwbcProductLineTaskWorkSnapshot::Cleanup(snapshot_exit(exit))
        }
    }
}

fn snapshot_node_index(node: RuntimeLineTaskNodeId) -> u32 {
    u32::try_from(node.index()).expect("runtime line-task node index fits the AWBC snapshot")
}

fn restore_work(work: AwbcProductLineTaskWorkSnapshot) -> Option<LineTaskWork> {
    match work {
        AwbcProductLineTaskWorkSnapshot::Node(node) => {
            RuntimeLineTaskNodeId::from_zero_based(usize::try_from(node).ok()?)
                .map(LineTaskWork::Node)
        }
        AwbcProductLineTaskWorkSnapshot::Cancellation(mark) => {
            Some(LineTaskWork::Cancellation(mark))
        }
        AwbcProductLineTaskWorkSnapshot::Cleanup(exit) => {
            Some(LineTaskWork::Cleanup(restore_exit(exit)))
        }
    }
}

fn snapshot_work_tag(tag: LineTaskWorkTag) -> AwbcProductLineTaskWorkTagSnapshot {
    AwbcProductLineTaskWorkTagSnapshot {
        instance: match tag.instance() {
            LineTaskWorkInstance::Activation(activation) => {
                AwbcProductLineTaskWorkInstanceSnapshot::Activation(activation.clone())
            }
            LineTaskWorkInstance::Scheduled(token) => {
                AwbcProductLineTaskWorkInstanceSnapshot::Scheduled(token.clone())
            }
        },
        work: snapshot_work(tag.work()),
    }
}

fn restore_work_tag(tag: AwbcProductLineTaskWorkTagSnapshot) -> Option<LineTaskWorkTag> {
    let work = restore_work(tag.work)?;
    Some(match tag.instance {
        AwbcProductLineTaskWorkInstanceSnapshot::Activation(activation) => {
            LineTaskWorkTag::activation(activation, work)
        }
        AwbcProductLineTaskWorkInstanceSnapshot::Scheduled(token) => {
            let LineTaskWork::Node(node) = work else {
                return None;
            };
            LineTaskWorkTag::scheduled(token, node)
        }
    })
}

fn snapshot_exit_policy(policy: LineTaskExitPolicy) -> AwbcProductLineTaskExitPolicySnapshot {
    AwbcProductLineTaskExitPolicySnapshot {
        join: match policy.join {
            ChildJoinPolicy::Join => AwbcProductLineTaskJoinSnapshot::Join,
            ChildJoinPolicy::Detached => AwbcProductLineTaskJoinSnapshot::Detached,
        },
        cancel: match policy.cancel {
            ChildCancelPolicy::CancelAndJoin => AwbcProductLineTaskCancelSnapshot::CancelAndJoin,
            ChildCancelPolicy::Finish => AwbcProductLineTaskCancelSnapshot::Finish,
            ChildCancelPolicy::Detach => AwbcProductLineTaskCancelSnapshot::Detach,
        },
    }
}

fn restore_exit_policy(policy: AwbcProductLineTaskExitPolicySnapshot) -> LineTaskExitPolicy {
    LineTaskExitPolicy {
        join: match policy.join {
            AwbcProductLineTaskJoinSnapshot::Join => ChildJoinPolicy::Join,
            AwbcProductLineTaskJoinSnapshot::Detached => ChildJoinPolicy::Detached,
        },
        cancel: match policy.cancel {
            AwbcProductLineTaskCancelSnapshot::CancelAndJoin => ChildCancelPolicy::CancelAndJoin,
            AwbcProductLineTaskCancelSnapshot::Finish => ChildCancelPolicy::Finish,
            AwbcProductLineTaskCancelSnapshot::Detach => ChildCancelPolicy::Detach,
        },
    }
}

fn snapshot_node_state(state: LineTaskNodeState) -> AwbcProductLineTaskNodeStateSnapshot {
    match state {
        LineTaskNodeState::Armed => AwbcProductLineTaskNodeStateSnapshot::Armed,
        LineTaskNodeState::Running => AwbcProductLineTaskNodeStateSnapshot::Running,
        LineTaskNodeState::Cancelling => AwbcProductLineTaskNodeStateSnapshot::Cancelling,
        LineTaskNodeState::Detached => AwbcProductLineTaskNodeStateSnapshot::Detached,
        LineTaskNodeState::Completed => AwbcProductLineTaskNodeStateSnapshot::Completed,
        LineTaskNodeState::Cancelled => AwbcProductLineTaskNodeStateSnapshot::Cancelled,
        LineTaskNodeState::Failed => AwbcProductLineTaskNodeStateSnapshot::Failed,
    }
}

fn restore_node_state(state: AwbcProductLineTaskNodeStateSnapshot) -> LineTaskNodeState {
    match state {
        AwbcProductLineTaskNodeStateSnapshot::Armed => LineTaskNodeState::Armed,
        AwbcProductLineTaskNodeStateSnapshot::Running => LineTaskNodeState::Running,
        AwbcProductLineTaskNodeStateSnapshot::Cancelling => LineTaskNodeState::Cancelling,
        AwbcProductLineTaskNodeStateSnapshot::Detached => LineTaskNodeState::Detached,
        AwbcProductLineTaskNodeStateSnapshot::Completed => LineTaskNodeState::Completed,
        AwbcProductLineTaskNodeStateSnapshot::Cancelled => LineTaskNodeState::Cancelled,
        AwbcProductLineTaskNodeStateSnapshot::Failed => LineTaskNodeState::Failed,
    }
}

fn snapshot_live_state(state: &LineTaskLiveState) -> AwbcProductLineTaskLiveSnapshot {
    let state = state.snapshot();
    AwbcProductLineTaskLiveSnapshot {
        activation: state.activation().clone(),
        phase: match state.phase() {
            LineTaskPhase::Active => AwbcProductLineTaskPhaseSnapshot::Active,
            LineTaskPhase::Closing { exit } => AwbcProductLineTaskPhaseSnapshot::Closing {
                exit: snapshot_exit(exit),
            },
            LineTaskPhase::Closed { exit } => AwbcProductLineTaskPhaseSnapshot::Closed {
                exit: snapshot_exit(exit),
            },
        },
        activation_lane: snapshot_execution_lane(
            state.node_states(),
            state.outstanding(),
            state.active_roots(),
            state.cancelling_nodes(),
        ),
        scheduled_lanes: state
            .scheduled_lanes()
            .iter()
            .map(|scheduled| AwbcProductLineTaskScheduledLaneSnapshot {
                token: scheduled.token().clone(),
                lane: snapshot_execution_lane(
                    scheduled.lane().node_states(),
                    scheduled.lane().outstanding(),
                    scheduled.lane().active_roots(),
                    scheduled.lane().cancelling_nodes(),
                ),
            })
            .collect(),
        scheduled_ready: state.scheduled_ready().to_vec(),
        consumed_content_events: state.consumed_content_events().to_vec(),
        cleanup_started: state.cleanup_started(),
    }
}

fn snapshot_execution_lane(
    node_states: &[LineTaskNodeState],
    outstanding: &[LineTaskWork],
    active_roots: &[LineTaskActiveRoot],
    cancelling_nodes: &[RuntimeLineTaskNodeId],
) -> AwbcProductLineTaskExecutionLaneSnapshot {
    AwbcProductLineTaskExecutionLaneSnapshot {
        node_states: node_states
            .iter()
            .copied()
            .map(snapshot_node_state)
            .collect(),
        outstanding: outstanding.iter().copied().map(snapshot_work).collect(),
        active_roots: active_roots
            .iter()
            .copied()
            .map(|root| AwbcProductLineTaskActiveRootSnapshot {
                node: snapshot_node_index(root.node()),
            })
            .collect(),
        cancelling_nodes: cancelling_nodes
            .iter()
            .copied()
            .map(snapshot_node_index)
            .collect(),
    }
}

fn restore_live_snapshot(
    snapshot: AwbcProductLineTaskLiveSnapshot,
) -> Result<LineTaskLiveSnapshot, AwbcProductStepBuildError> {
    let restore_nodes = |nodes: Vec<u32>| {
        nodes
            .into_iter()
            .map(|node| {
                let node = usize::try_from(node).map_err(|_| {
                    AwbcProductStepBuildError::RestoreSnapshot {
                        message: "line-task snapshot node index exceeds platform range".to_owned(),
                    }
                })?;
                RuntimeLineTaskNodeId::from_zero_based(node).ok_or_else(|| {
                    AwbcProductStepBuildError::RestoreSnapshot {
                        message: "line-task snapshot node index exceeds runtime identity range"
                            .to_owned(),
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let restore_lane = |lane: AwbcProductLineTaskExecutionLaneSnapshot| {
        let outstanding = lane
            .outstanding
            .into_iter()
            .map(|work| {
                restore_work(work).ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                    message: "line-task snapshot work references an invalid node identity"
                        .to_owned(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let active_roots = restore_nodes(
            lane.active_roots
                .into_iter()
                .map(|root| root.node)
                .collect(),
        )?
        .into_iter()
        .map(LineTaskActiveRoot::new)
        .collect::<Vec<_>>();
        Ok::<_, AwbcProductStepBuildError>(LineTaskExecutionLaneSnapshot::new(
            lane.node_states
                .into_iter()
                .map(restore_node_state)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            outstanding.into_boxed_slice(),
            active_roots.into_boxed_slice(),
            restore_nodes(lane.cancelling_nodes)?.into_boxed_slice(),
        ))
    };
    let activation_lane = restore_lane(snapshot.activation_lane)?;
    let scheduled_lanes = snapshot
        .scheduled_lanes
        .into_iter()
        .map(|scheduled| {
            Ok(LineTaskScheduledLaneSnapshot::new(
                scheduled.token,
                restore_lane(scheduled.lane)?,
            ))
        })
        .collect::<Result<Vec<_>, AwbcProductStepBuildError>>()?;
    Ok(LineTaskLiveSnapshot::new(
        snapshot.activation,
        match snapshot.phase {
            AwbcProductLineTaskPhaseSnapshot::Active => LineTaskPhase::Active,
            AwbcProductLineTaskPhaseSnapshot::Closing { exit } => LineTaskPhase::Closing {
                exit: restore_exit(exit),
            },
            AwbcProductLineTaskPhaseSnapshot::Closed { exit } => LineTaskPhase::Closed {
                exit: restore_exit(exit),
            },
        },
        activation_lane,
        scheduled_lanes.into_boxed_slice(),
        snapshot.scheduled_ready.into_boxed_slice(),
        snapshot.consumed_content_events.into_boxed_slice(),
        snapshot.cleanup_started,
    ))
}

impl ProductChildFiberOwner {
    fn snapshot(&self) -> AwbcProductChildFiberOwnerSnapshot {
        match self {
            Self::Independent => AwbcProductChildFiberOwnerSnapshot::Independent,
            Self::LineTask {
                content,
                tag,
                policy,
                phase,
            } => AwbcProductChildFiberOwnerSnapshot::LineTask {
                content: *content,
                tag: snapshot_work_tag(tag.clone()),
                policy: snapshot_exit_policy(*policy),
                phase: match phase {
                    ProductLineTaskFiberPhase::Active => {
                        AwbcProductLineTaskFiberPhaseSnapshot::Active
                    }
                    ProductLineTaskFiberPhase::Closing => {
                        AwbcProductLineTaskFiberPhaseSnapshot::Closing
                    }
                },
            },
        }
    }

    fn restore(snapshot: &AwbcProductChildFiberOwnerSnapshot) -> Self {
        match snapshot {
            AwbcProductChildFiberOwnerSnapshot::Independent => Self::Independent,
            AwbcProductChildFiberOwnerSnapshot::LineTask {
                content,
                tag,
                policy,
                phase,
            } => Self::LineTask {
                content: *content,
                tag: restore_work_tag(tag.clone())
                    .expect("validated line-task child owner work tag"),
                policy: restore_exit_policy(*policy),
                phase: match phase {
                    AwbcProductLineTaskFiberPhaseSnapshot::Active => {
                        ProductLineTaskFiberPhase::Active
                    }
                    AwbcProductLineTaskFiberPhaseSnapshot::Closing => {
                        ProductLineTaskFiberPhase::Closing
                    }
                },
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcProductExecutorSnapshot {
    pub fiber: FiberState,
    pub child_fibers: Vec<AwbcProductChildFiberSnapshot>,
    /// Exact semantic identities for every live Flow function and retained
    /// choice target. Dense function indices alone are not restore authority.
    pub live_flow_bindings: Vec<AwbcFlowBinding>,
    pub entry_bound: bool,
    pub(super) dialogues: AwbcProductDialogueSnapshotState,
    pub active_choice: Option<AwbcProductActiveChoiceSnapshot>,
    pub pending_host_call: Option<AwbcProductPendingHostCallSnapshot>,
    pub started_tasks: BTreeSet<TaskId>,
    pub task_publications: BTreeMap<TaskId, TaskPublicationCursor>,
    pub need_publications: BTreeMap<crate::task::NeedId, TaskPublicationCursor>,
    pub queued_task_events: VecDeque<TaskEvent>,
    pub emitted_content: BTreeSet<AwbcContentUnitId>,
    pub stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    pub next_generation: u64,
    pub next_fiber_instance: crate::runtime_id::RuntimeIdCursor,
    pub dialogue_occurrences: Vec<AwbcProductDialogueOccurrenceSnapshot>,
    pub next_host_call_sequence: u64,
    pub next_audio_sequence: u64,
    pub compact_pure_stats: crate::step::RuntimePureCallStats,
    pub observations: RuntimeObservationState,
}

/// AWBC session-save projection of [`AwbcProductExecutorSnapshot`].
///
/// The in-memory snapshot remains useful to embedders and tests, but the
/// persistence payload must not deserialize live `RuntimeValue` directly.
#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductExecutorSaveSnapshot {
    pub fiber: AwbcFiberStateSnapshot,
    pub child_fibers: Vec<AwbcProductChildFiberSaveSnapshot>,
    pub live_flow_bindings: Vec<AwbcFlowBinding>,
    pub entry_bound: bool,
    dialogues: crate::line_task::RuntimeDialogueRegistrySaveSnapshot<
        AwbcProductActiveDialogueSaveSnapshot,
        AwbcTypeId,
    >,
    pub active_choice: Option<AwbcProductActiveChoiceSnapshot>,
    pub pending_host_call: Option<AwbcProductPendingHostCallSnapshot>,
    pub started_tasks: BTreeSet<TaskId>,
    pub task_publications: BTreeMap<TaskId, TaskPublicationCursor>,
    pub need_publications: BTreeMap<crate::task::NeedId, TaskPublicationCursor>,
    pub queued_task_events: VecDeque<AwbcProductTaskEventSaveSnapshot>,
    pub emitted_content: BTreeSet<AwbcContentUnitId>,
    pub stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    pub next_generation: u64,
    pub next_fiber_instance: crate::runtime_id::RuntimeIdCursor,
    pub dialogue_occurrences: Vec<AwbcProductDialogueOccurrenceSnapshot>,
    pub next_host_call_sequence: u64,
    pub next_audio_sequence: u64,
    pub compact_pure_stats: crate::step::RuntimePureCallStats,
    pub observations: RuntimeObservationState,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductActiveDialogueSaveSnapshot {
    pub activation: DialogueActivationId,
    pub content: AwbcContentUnitId,
    pub line: crate::plan::RuntimeLineId,
    pub result: AwbcDialogueResultTarget,
    pub captures: Vec<crate::value::AwbcRuntimeValueSnapshot>,
    pub values: Vec<AwbcProductDialogueValueSaveSnapshot>,
    pub voice: crate::presentation::RuntimeDialogueVoiceState,
    pub phase: AwbcProductDialoguePhaseSaveSnapshot,
    pub elapsed_nanos: u64,
    pub pending_content_events: Vec<RuntimeDialogueContentEventKind>,
    pub pending_advance: bool,
    pub pending_line_outcomes: Vec<crate::presentation::RuntimeLineHostOutcome>,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductDialogueValueSaveSnapshot {
    pub slot: crate::runtime_id::RuntimeDialogueValueSlotId,
    pub value: crate::value::AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductDialoguePhaseSaveSnapshot {
    Activating {
        fiber: AwbcFiberStateSnapshot,
        pending: Option<AwbcProductPendingLineOperationSaveSnapshot>,
    },
    Reducing {
        line_task: AwbcProductLineTaskLiveSnapshot,
    },
    Publishing {
        line_task: AwbcProductLineTaskLiveSnapshot,
    },
    Closing(AwbcProductDialogueClosingSaveSnapshot),
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductDialogueClosingSaveSnapshot {
    failure: crate::awbc::fiber::FiberTrap,
    state: AwbcProductDialogueClosingStateSaveSnapshot,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductDialogueClosingStateSaveSnapshot {
    Activation {
        fiber: AwbcFiberStateSnapshot,
        pending: Option<AwbcProductPendingLineOperationSaveSnapshot>,
    },
    LineTask {
        line_task: AwbcProductLineTaskLiveSnapshot,
    },
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductPendingLineOperationSaveSnapshot {
    AcquireActor {
        cursor: crate::awbc::fiber::FiberCursor,
        destination: crate::awbc::schema::AwbcRegisterId,
        command: crate::presentation::RuntimeLineCommandId,
        value: crate::value::AwbcRuntimeValueSnapshot,
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    ActorLook {
        cursor: crate::awbc::fiber::FiberCursor,
        destination: crate::awbc::schema::AwbcRegisterId,
        command: crate::presentation::RuntimeLineCommandId,
        value: crate::value::AwbcRuntimeValueSnapshot,
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    StartVoice {
        cursor: crate::awbc::fiber::FiberCursor,
        destination: crate::awbc::schema::AwbcRegisterId,
        command: crate::presentation::RuntimeLineCommandId,
        site: crate::awbc::schema::AwbcLineHandleSiteId,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum AwbcProductDialogueSnapshotState {
    Live(super::ProductDialogueStore),
    Saved(
        crate::line_task::RuntimeDialogueRegistrySaveSnapshot<
            AwbcProductActiveDialogueSaveSnapshot,
            AwbcTypeId,
        >,
    ),
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductChildFiberSaveSnapshot {
    pub owner: AwbcProductChildFiberOwnerSnapshot,
    pub fiber: AwbcFiberStateSnapshot,
}

fn snapshot_pending_line_operation(
    pending: &super::ProductPendingLineOperation,
) -> AwbcProductPendingLineOperationSnapshot {
    match pending {
        super::ProductPendingLineOperation::AcquireActor {
            cursor,
            destination,
            command,
            value,
            token,
        } => AwbcProductPendingLineOperationSnapshot::AcquireActor {
            cursor: *cursor,
            destination: *destination,
            command: command.clone(),
            value: value.clone(),
            token: token.clone(),
        },
        super::ProductPendingLineOperation::ActorLook {
            cursor,
            destination,
            command,
            value,
            token,
        } => AwbcProductPendingLineOperationSnapshot::ActorLook {
            cursor: *cursor,
            destination: *destination,
            command: command.clone(),
            value: value.clone(),
            token: token.clone(),
        },
        super::ProductPendingLineOperation::StartVoice {
            cursor,
            destination,
            command,
            site,
        } => AwbcProductPendingLineOperationSnapshot::StartVoice {
            cursor: *cursor,
            destination: *destination,
            command: command.clone(),
            site: *site,
        },
    }
}

fn restore_pending_line_operation(
    pending: AwbcProductPendingLineOperationSnapshot,
) -> super::ProductPendingLineOperation {
    match pending {
        AwbcProductPendingLineOperationSnapshot::AcquireActor {
            cursor,
            destination,
            command,
            value,
            token,
        } => super::ProductPendingLineOperation::AcquireActor {
            cursor,
            destination,
            command,
            value,
            token,
        },
        AwbcProductPendingLineOperationSnapshot::ActorLook {
            cursor,
            destination,
            command,
            value,
            token,
        } => super::ProductPendingLineOperation::ActorLook {
            cursor,
            destination,
            command,
            value,
            token,
        },
        AwbcProductPendingLineOperationSnapshot::StartVoice {
            cursor,
            destination,
            command,
            site,
        } => super::ProductPendingLineOperation::StartVoice {
            cursor,
            destination,
            command,
            site,
        },
    }
}

fn snapshot_dialogue_phase(
    phase: &super::ProductDialoguePhase,
) -> AwbcProductDialoguePhaseSnapshot {
    match phase {
        super::ProductDialoguePhase::Activating { fiber, pending } => {
            AwbcProductDialoguePhaseSnapshot::Activating {
                fiber: fiber.clone(),
                pending: pending.as_ref().map(snapshot_pending_line_operation),
            }
        }
        super::ProductDialoguePhase::Reducing { line_task } => {
            AwbcProductDialoguePhaseSnapshot::Reducing {
                line_task: snapshot_live_state(line_task),
            }
        }
        super::ProductDialoguePhase::Publishing { line_task } => {
            AwbcProductDialoguePhaseSnapshot::Publishing {
                line_task: snapshot_live_state(line_task),
            }
        }
        super::ProductDialoguePhase::Closing(closing) => {
            AwbcProductDialoguePhaseSnapshot::Closing(AwbcProductDialogueClosingSnapshot {
                failure: closing.failure.clone(),
                state: match &closing.state {
                    super::ProductDialogueClosingState::Activation { fiber, pending } => {
                        AwbcProductDialogueClosingStateSnapshot::Activation {
                            fiber: fiber.clone(),
                            pending: pending.as_ref().map(snapshot_pending_line_operation),
                        }
                    }
                    super::ProductDialogueClosingState::LineTask { line_task } => {
                        AwbcProductDialogueClosingStateSnapshot::LineTask {
                            line_task: snapshot_live_state(line_task),
                        }
                    }
                },
            })
        }
    }
}

fn snapshot_pending_line_operation_for_save(
    pending: &super::ProductPendingLineOperation,
) -> Result<AwbcProductPendingLineOperationSaveSnapshot, crate::value::AwbcRuntimeValueSnapshotError>
{
    Ok(match pending {
        super::ProductPendingLineOperation::AcquireActor {
            cursor,
            destination,
            command,
            value,
            token,
        } => AwbcProductPendingLineOperationSaveSnapshot::AcquireActor {
            cursor: *cursor,
            destination: *destination,
            command: command.clone(),
            value: crate::value::AwbcRuntimeValueSnapshot::from_runtime_value(value)?,
            token: token.clone(),
        },
        super::ProductPendingLineOperation::ActorLook {
            cursor,
            destination,
            command,
            value,
            token,
        } => AwbcProductPendingLineOperationSaveSnapshot::ActorLook {
            cursor: *cursor,
            destination: *destination,
            command: command.clone(),
            value: crate::value::AwbcRuntimeValueSnapshot::from_runtime_value(value)?,
            token: token.clone(),
        },
        super::ProductPendingLineOperation::StartVoice {
            cursor,
            destination,
            command,
            site,
        } => AwbcProductPendingLineOperationSaveSnapshot::StartVoice {
            cursor: *cursor,
            destination: *destination,
            command: command.clone(),
            site: *site,
        },
    })
}

fn snapshot_dialogue_phase_for_save(
    phase: &super::ProductDialoguePhase,
) -> Result<AwbcProductDialoguePhaseSaveSnapshot, crate::value::AwbcRuntimeValueSnapshotError> {
    Ok(match phase {
        super::ProductDialoguePhase::Activating { fiber, pending } => {
            AwbcProductDialoguePhaseSaveSnapshot::Activating {
                fiber: AwbcFiberStateSnapshot::from_live(fiber)?,
                pending: pending
                    .as_ref()
                    .map(snapshot_pending_line_operation_for_save)
                    .transpose()?,
            }
        }
        super::ProductDialoguePhase::Reducing { line_task } => {
            AwbcProductDialoguePhaseSaveSnapshot::Reducing {
                line_task: snapshot_live_state(line_task),
            }
        }
        super::ProductDialoguePhase::Publishing { line_task } => {
            AwbcProductDialoguePhaseSaveSnapshot::Publishing {
                line_task: snapshot_live_state(line_task),
            }
        }
        super::ProductDialoguePhase::Closing(closing) => {
            AwbcProductDialoguePhaseSaveSnapshot::Closing(AwbcProductDialogueClosingSaveSnapshot {
                failure: closing.failure.clone(),
                state: match &closing.state {
                    super::ProductDialogueClosingState::Activation { fiber, pending } => {
                        AwbcProductDialogueClosingStateSaveSnapshot::Activation {
                            fiber: AwbcFiberStateSnapshot::from_live(fiber)?,
                            pending: pending
                                .as_ref()
                                .map(snapshot_pending_line_operation_for_save)
                                .transpose()?,
                        }
                    }
                    super::ProductDialogueClosingState::LineTask { line_task } => {
                        AwbcProductDialogueClosingStateSaveSnapshot::LineTask {
                            line_task: snapshot_live_state(line_task),
                        }
                    }
                },
            })
        }
    })
}

fn restore_pending_line_operation_from_save(
    pending: AwbcProductPendingLineOperationSaveSnapshot,
) -> Result<AwbcProductPendingLineOperationSnapshot, crate::value::AwbcRuntimeValueSnapshotError> {
    Ok(match pending {
        AwbcProductPendingLineOperationSaveSnapshot::AcquireActor {
            cursor,
            destination,
            command,
            value,
            token,
        } => AwbcProductPendingLineOperationSnapshot::AcquireActor {
            cursor,
            destination,
            command,
            value: value.into_runtime_value()?,
            token,
        },
        AwbcProductPendingLineOperationSaveSnapshot::ActorLook {
            cursor,
            destination,
            command,
            value,
            token,
        } => AwbcProductPendingLineOperationSnapshot::ActorLook {
            cursor,
            destination,
            command,
            value: value.into_runtime_value()?,
            token,
        },
        AwbcProductPendingLineOperationSaveSnapshot::StartVoice {
            cursor,
            destination,
            command,
            site,
        } => AwbcProductPendingLineOperationSnapshot::StartVoice {
            cursor,
            destination,
            command,
            site,
        },
    })
}

fn restore_dialogue_phase_from_save(
    phase: AwbcProductDialoguePhaseSaveSnapshot,
) -> Result<AwbcProductDialoguePhaseSnapshot, String> {
    Ok(match phase {
        AwbcProductDialoguePhaseSaveSnapshot::Activating { fiber, pending } => {
            AwbcProductDialoguePhaseSnapshot::Activating {
                fiber: fiber.into_live().map_err(|error| error.to_string())?,
                pending: pending
                    .map(restore_pending_line_operation_from_save)
                    .transpose()
                    .map_err(|error| error.to_string())?,
            }
        }
        AwbcProductDialoguePhaseSaveSnapshot::Reducing { line_task } => {
            AwbcProductDialoguePhaseSnapshot::Reducing { line_task }
        }
        AwbcProductDialoguePhaseSaveSnapshot::Publishing { line_task } => {
            AwbcProductDialoguePhaseSnapshot::Publishing { line_task }
        }
        AwbcProductDialoguePhaseSaveSnapshot::Closing(closing) => {
            AwbcProductDialoguePhaseSnapshot::Closing(AwbcProductDialogueClosingSnapshot {
                failure: closing.failure,
                state: match closing.state {
                    AwbcProductDialogueClosingStateSaveSnapshot::Activation { fiber, pending } => {
                        AwbcProductDialogueClosingStateSnapshot::Activation {
                            fiber: fiber.into_live().map_err(|error| error.to_string())?,
                            pending: pending
                                .map(restore_pending_line_operation_from_save)
                                .transpose()
                                .map_err(|error| error.to_string())?,
                        }
                    }
                    AwbcProductDialogueClosingStateSaveSnapshot::LineTask { line_task } => {
                        AwbcProductDialogueClosingStateSnapshot::LineTask { line_task }
                    }
                },
            })
        }
    })
}

fn snapshot_active_dialogue(
    active: &super::ActiveDialogue,
) -> Result<
    AwbcProductActiveDialogueSaveSnapshot,
    crate::line_task::RuntimeDialogueRegistrySnapshotError,
> {
    Ok(AwbcProductActiveDialogueSaveSnapshot {
        activation: active.activation.clone(),
        content: active.content,
        line: active.line.clone(),
        result: active.result.clone(),
        captures: active
            .captures
            .iter()
            .map(|capture| crate::value::AwbcRuntimeValueSnapshot::from_runtime_value(capture))
            .collect::<Result<_, _>>()?,
        values: active
            .values
            .iter()
            .map(|binding| {
                Ok(AwbcProductDialogueValueSaveSnapshot {
                    slot: binding.slot,
                    value: crate::value::AwbcRuntimeValueSnapshot::from_runtime_value(
                        &binding.value,
                    )?,
                })
            })
            .collect::<Result<_, crate::value::AwbcRuntimeValueSnapshotError>>()?,
        voice: active.voice.clone(),
        phase: snapshot_dialogue_phase_for_save(&active.phase)?,
        elapsed_nanos: active.elapsed_nanos,
        pending_content_events: active.pending_content_events.clone(),
        pending_advance: active.pending_advance,
        pending_line_outcomes: active.pending_line_outcomes.clone(),
    })
}

impl AwbcProductExecutorSaveSnapshot {
    pub fn from_live(snapshot: &AwbcProductExecutorSnapshot) -> Result<Self, String> {
        let dialogues = match &snapshot.dialogues {
            AwbcProductDialogueSnapshotState::Live(dialogues) => dialogues
                .to_save_snapshot(snapshot_active_dialogue)
                .map_err(|error| error.to_string())?,
            AwbcProductDialogueSnapshotState::Saved(dialogues) => dialogues.clone(),
        };
        Ok(Self {
            fiber: AwbcFiberStateSnapshot::from_live(&snapshot.fiber)
                .map_err(|error| error.to_string())?,
            child_fibers: snapshot
                .child_fibers
                .iter()
                .map(|child| {
                    Ok(AwbcProductChildFiberSaveSnapshot {
                        owner: child.owner.clone(),
                        fiber: AwbcFiberStateSnapshot::from_live(&child.fiber)
                            .map_err(|error| error.to_string())?,
                    })
                })
                .collect::<Result<_, String>>()?,
            live_flow_bindings: snapshot.live_flow_bindings.clone(),
            entry_bound: snapshot.entry_bound,
            dialogues,
            active_choice: snapshot.active_choice.clone(),
            pending_host_call: snapshot.pending_host_call.clone(),
            started_tasks: snapshot.started_tasks.clone(),
            task_publications: snapshot.task_publications.clone(),
            need_publications: snapshot.need_publications.clone(),
            queued_task_events: snapshot
                .queued_task_events
                .iter()
                .map(AwbcProductTaskEventSaveSnapshot::from_live)
                .collect::<Result<VecDeque<_>, _>>()?,
            emitted_content: snapshot.emitted_content.clone(),
            stream_sequences: snapshot.stream_sequences.clone(),
            next_generation: snapshot.next_generation,
            next_fiber_instance: snapshot.next_fiber_instance,
            dialogue_occurrences: snapshot.dialogue_occurrences.clone(),
            next_host_call_sequence: snapshot.next_host_call_sequence,
            next_audio_sequence: snapshot.next_audio_sequence,
            compact_pure_stats: snapshot.compact_pure_stats,
            observations: snapshot.observations.clone(),
        })
    }

    pub fn into_live(self) -> Result<AwbcProductExecutorSnapshot, String> {
        Ok(AwbcProductExecutorSnapshot {
            fiber: self.fiber.into_live().map_err(|error| error.to_string())?,
            child_fibers: self
                .child_fibers
                .into_iter()
                .map(|child| {
                    Ok(AwbcProductChildFiberSnapshot {
                        owner: child.owner,
                        fiber: child.fiber.into_live().map_err(|error| error.to_string())?,
                    })
                })
                .collect::<Result<_, String>>()?,
            live_flow_bindings: self.live_flow_bindings,
            entry_bound: self.entry_bound,
            dialogues: AwbcProductDialogueSnapshotState::Saved(self.dialogues),
            active_choice: self.active_choice,
            pending_host_call: self.pending_host_call,
            started_tasks: self.started_tasks,
            task_publications: self.task_publications,
            need_publications: self.need_publications,
            queued_task_events: self
                .queued_task_events
                .into_iter()
                .map(AwbcProductTaskEventSaveSnapshot::into_live)
                .collect::<Result<VecDeque<_>, _>>()?,
            emitted_content: self.emitted_content,
            stream_sequences: self.stream_sequences,
            next_generation: self.next_generation,
            next_fiber_instance: self.next_fiber_instance,
            dialogue_occurrences: self.dialogue_occurrences,
            next_host_call_sequence: self.next_host_call_sequence,
            next_audio_sequence: self.next_audio_sequence,
            compact_pure_stats: self.compact_pure_stats,
            observations: self.observations,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcProductActiveDialogueSnapshot {
    pub activation: DialogueActivationId,
    pub content: AwbcContentUnitId,
    pub line: crate::plan::RuntimeLineId,
    pub result: AwbcDialogueResultTarget,
    pub captures: Vec<RuntimePayload>,
    pub values: Vec<crate::plan::RuntimeDialogueValueBinding>,
    pub voice: crate::presentation::RuntimeDialogueVoiceState,
    pub phase: AwbcProductDialoguePhaseSnapshot,
    pub elapsed_nanos: u64,
    pub pending_content_events: Vec<RuntimeDialogueContentEventKind>,
    pub pending_advance: bool,
    pub pending_line_outcomes: Vec<crate::presentation::RuntimeLineHostOutcome>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcProductDialoguePhaseSnapshot {
    Activating {
        fiber: FiberState,
        pending: Option<AwbcProductPendingLineOperationSnapshot>,
    },
    Reducing {
        line_task: AwbcProductLineTaskLiveSnapshot,
    },
    Publishing {
        line_task: AwbcProductLineTaskLiveSnapshot,
    },
    Closing(AwbcProductDialogueClosingSnapshot),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcProductDialogueClosingSnapshot {
    pub failure: crate::awbc::fiber::FiberTrap,
    pub state: AwbcProductDialogueClosingStateSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcProductDialogueClosingStateSnapshot {
    Activation {
        fiber: FiberState,
        pending: Option<AwbcProductPendingLineOperationSnapshot>,
    },
    LineTask {
        line_task: AwbcProductLineTaskLiveSnapshot,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum AwbcProductPendingLineOperationSnapshot {
    AcquireActor {
        cursor: crate::awbc::fiber::FiberCursor,
        destination: crate::awbc::schema::AwbcRegisterId,
        command: crate::presentation::RuntimeLineCommandId,
        value: crate::value::RuntimeValue,
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    ActorLook {
        cursor: crate::awbc::fiber::FiberCursor,
        destination: crate::awbc::schema::AwbcRegisterId,
        command: crate::presentation::RuntimeLineCommandId,
        value: crate::value::RuntimeValue,
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    StartVoice {
        cursor: crate::awbc::fiber::FiberCursor,
        destination: crate::awbc::schema::AwbcRegisterId,
        command: crate::presentation::RuntimeLineCommandId,
        site: crate::awbc::schema::AwbcLineHandleSiteId,
    },
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct AwbcProductDialogueOccurrenceSnapshot {
    pub owner: crate::runtime_id::RuntimePersistentFiberId,
    pub content: crate::runtime_id::RuntimeDialogueContentPlanId,
    pub next: u64,
}

/// Complete persisted reducer state for one content-owned dialogue group.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductLineTaskLiveSnapshot {
    pub activation: DialogueActivationId,
    pub phase: AwbcProductLineTaskPhaseSnapshot,
    pub activation_lane: AwbcProductLineTaskExecutionLaneSnapshot,
    pub scheduled_lanes: Vec<AwbcProductLineTaskScheduledLaneSnapshot>,
    pub scheduled_ready: Vec<RuntimeLineHandleToken>,
    pub consumed_content_events: Vec<RuntimeDialogueContentEventKind>,
    pub cleanup_started: bool,
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductLineTaskExecutionLaneSnapshot {
    pub node_states: Vec<AwbcProductLineTaskNodeStateSnapshot>,
    pub outstanding: Vec<AwbcProductLineTaskWorkSnapshot>,
    pub active_roots: Vec<AwbcProductLineTaskActiveRootSnapshot>,
    pub cancelling_nodes: Vec<u32>,
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductLineTaskScheduledLaneSnapshot {
    pub token: RuntimeLineHandleToken,
    pub lane: AwbcProductLineTaskExecutionLaneSnapshot,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductLineTaskPhaseSnapshot {
    Active,
    Closing {
        exit: AwbcProductLineTaskExitSnapshot,
    },
    Closed {
        exit: AwbcProductLineTaskExitSnapshot,
    },
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductLineTaskExitSnapshot {
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductLineTaskNodeStateSnapshot {
    Armed,
    Running,
    Cancelling,
    Detached,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductLineTaskWorkSnapshot {
    Node(u32),
    Cancellation(RuntimeDialogueMarkId),
    Cleanup(AwbcProductLineTaskExitSnapshot),
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductLineTaskWorkTagSnapshot {
    pub instance: AwbcProductLineTaskWorkInstanceSnapshot,
    pub work: AwbcProductLineTaskWorkSnapshot,
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductLineTaskWorkInstanceSnapshot {
    Activation(DialogueActivationId),
    Scheduled(RuntimeLineHandleToken),
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductLineTaskActiveRootSnapshot {
    pub node: u32,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductLineTaskExitPolicySnapshot {
    pub join: AwbcProductLineTaskJoinSnapshot,
    pub cancel: AwbcProductLineTaskCancelSnapshot,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductLineTaskJoinSnapshot {
    Join,
    Detached,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductLineTaskCancelSnapshot {
    CancelAndJoin,
    Finish,
    Detach,
}

/// Durable owner identity for every queued compact child fiber.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductChildFiberOwnerSnapshot {
    Independent,
    LineTask {
        content: AwbcContentUnitId,
        tag: AwbcProductLineTaskWorkTagSnapshot,
        policy: AwbcProductLineTaskExitPolicySnapshot,
        phase: AwbcProductLineTaskFiberPhaseSnapshot,
    },
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductLineTaskFiberPhaseSnapshot {
    Active,
    Closing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AwbcProductChildFiberSnapshot {
    pub owner: AwbcProductChildFiberOwnerSnapshot,
    pub fiber: FiberState,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductActiveChoiceSnapshot {
    pub choice: AwbcChoiceId,
    pub public_id: Option<String>,
    pub option_indices: Vec<usize>,
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductPendingHostCallSnapshot {
    pub call: AwbcHostCallId,
    pub id: String,
}

fn validate_task_publications(
    snapshot: &AwbcProductExecutorSnapshot,
) -> Result<(), AwbcProductStepBuildError> {
    let mut queued_through = BTreeMap::<TaskId, TaskPublicationCursor>::new();
    for event in &snapshot.queued_task_events {
        let cursor = TaskPublicationCursor::from_event(event);
        if queued_through
            .insert(event.task_id.clone(), cursor)
            .is_some_and(|previous| cursor <= previous)
            || snapshot
                .task_publications
                .get(&event.task_id)
                .is_none_or(|observed| cursor > *observed)
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "queued task publications are not ordered under their retained cursor"
                    .to_owned(),
            });
        }
    }
    Ok(())
}

impl AwbcProductStepExecutor {
    #[must_use]
    pub fn snapshot(&self) -> AwbcProductExecutorSnapshot {
        AwbcProductExecutorSnapshot {
            fiber: self.fiber.clone(),
            child_fibers: self
                .child_fibers
                .iter()
                .map(|child| AwbcProductChildFiberSnapshot {
                    owner: child.owner.snapshot(),
                    fiber: child.fiber.clone(),
                })
                .collect(),
            live_flow_bindings: self.live_flow_bindings(),
            entry_bound: self.entry_bound,
            dialogues: AwbcProductDialogueSnapshotState::Live(self.dialogues.clone()),
            active_choice: self.active_choice.as_ref().map(|active| {
                AwbcProductActiveChoiceSnapshot {
                    choice: active.choice,
                    public_id: active.public_id.clone(),
                    option_indices: active.option_indices.clone(),
                }
            }),
            pending_host_call: self.pending_host_call.as_ref().map(|pending| {
                AwbcProductPendingHostCallSnapshot {
                    call: pending.call,
                    id: pending.id.0.clone(),
                }
            }),
            started_tasks: self.started_tasks.clone(),
            task_publications: self.task_publications.clone(),
            need_publications: self.need_publications.clone(),
            queued_task_events: self.queued_task_events.clone(),
            emitted_content: self.emitted_content.clone(),
            stream_sequences: self.stream_sequences.clone(),
            next_generation: self.next_generation,
            next_fiber_instance: self.next_fiber_instance,
            dialogue_occurrences: self
                .dialogue_occurrences
                .iter()
                .map(
                    |(&(owner, content), &next)| AwbcProductDialogueOccurrenceSnapshot {
                        owner,
                        content,
                        next,
                    },
                )
                .collect(),
            next_host_call_sequence: self.next_host_call_sequence,
            next_audio_sequence: self.next_audio_sequence,
            compact_pure_stats: self.compact_pure_stats,
            observations: self.facade_fiber.observations.clone(),
        }
    }

    pub fn restore_snapshot(
        &mut self,
        snapshot: AwbcProductExecutorSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        self.validate_snapshot(&snapshot)?;
        self.fiber = snapshot.fiber;
        self.child_fibers = snapshot
            .child_fibers
            .into_iter()
            .map(|child| ProductChildFiber {
                owner: ProductChildFiberOwner::restore(&child.owner),
                fiber: child.fiber,
            })
            .collect();
        self.entry_bound = snapshot.entry_bound;
        self.dialogues = self.restore_dialogue_store(snapshot.dialogues)?;
        self.active_choice = snapshot
            .active_choice
            .map(|active| self.restore_active_choice(active))
            .transpose()?;
        self.pending_host_call = snapshot.pending_host_call.map(|pending| PendingHostCall {
            call: pending.call,
            id: RuntimeHostCallId(pending.id),
        });
        self.started_tasks = snapshot.started_tasks;
        self.task_publications = snapshot.task_publications;
        self.need_publications = snapshot.need_publications;
        self.queued_task_events = snapshot.queued_task_events;
        self.emitted_content = snapshot.emitted_content;
        self.stream_sequences = snapshot.stream_sequences;
        self.next_generation = snapshot.next_generation;
        self.next_fiber_instance = snapshot.next_fiber_instance;
        self.dialogue_occurrences = snapshot
            .dialogue_occurrences
            .into_iter()
            .map(|row| ((row.owner, row.content), row.next))
            .collect();
        self.next_host_call_sequence = snapshot.next_host_call_sequence;
        self.next_audio_sequence = snapshot.next_audio_sequence;
        self.compact_pure_stats = snapshot.compact_pure_stats;
        self.facade_fiber.observations = snapshot.observations;
        self.rebuild_facade_stream_states_from_compact();
        self.sync_facade();
        Ok(())
    }

    fn restore_active_dialogue(
        &self,
        active: AwbcProductActiveDialogueSnapshot,
    ) -> Result<ActiveDialogue, AwbcProductStepBuildError> {
        let has_group = self
            .program
            .content_units
            .get(active.content.index())
            .ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                message: "active dialogue snapshot references missing AWBC content".to_owned(),
            })?
            .line_task_group
            .is_some();
        if !has_group {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "active dialogue snapshot content has no line-task group".to_owned(),
            });
        }
        let restore_line_task = |snapshot| {
            let view = self.line_task_view(active.content).ok_or_else(|| {
                AwbcProductStepBuildError::RestoreSnapshot {
                    message: "active dialogue snapshot has no verified line-task view".to_owned(),
                }
            })?;
            LineTaskLiveState::restore(&view, restore_live_snapshot(snapshot)?).map_err(|error| {
                AwbcProductStepBuildError::RestoreSnapshot {
                    message: format!("invalid line-task reducer snapshot: {error}"),
                }
            })
        };
        let phase = match active.phase {
            AwbcProductDialoguePhaseSnapshot::Activating { fiber, pending } => {
                fiber.validate_for_program(&self.program).map_err(|error| {
                    AwbcProductStepBuildError::RestoreSnapshot {
                        message: format!("invalid line-activation fiber snapshot: {error}"),
                    }
                })?;
                super::ProductDialoguePhase::Activating {
                    fiber,
                    pending: pending.map(restore_pending_line_operation),
                }
            }
            AwbcProductDialoguePhaseSnapshot::Reducing { line_task } => {
                let line_task = restore_line_task(line_task)?;
                if line_task.is_closed() {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "reducing dialogue phase cannot own a closed reducer".to_owned(),
                    });
                }
                super::ProductDialoguePhase::Reducing { line_task }
            }
            AwbcProductDialoguePhaseSnapshot::Publishing { line_task } => {
                let line_task = restore_line_task(line_task)?;
                if !line_task.is_closed() {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "publishing dialogue phase requires a closed reducer".to_owned(),
                    });
                }
                super::ProductDialoguePhase::Publishing { line_task }
            }
            AwbcProductDialoguePhaseSnapshot::Closing(closing) => {
                super::ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                    failure: closing.failure,
                    state: match closing.state {
                        AwbcProductDialogueClosingStateSnapshot::Activation { fiber, pending } => {
                            fiber.validate_for_program(&self.program).map_err(|error| {
                                AwbcProductStepBuildError::RestoreSnapshot {
                                    message: format!(
                                        "invalid closing line-activation fiber snapshot: {error}"
                                    ),
                                }
                            })?;
                            super::ProductDialogueClosingState::Activation {
                                fiber,
                                pending: pending.map(restore_pending_line_operation),
                            }
                        }
                        AwbcProductDialogueClosingStateSnapshot::LineTask { line_task } => {
                            super::ProductDialogueClosingState::LineTask {
                                line_task: restore_line_task(line_task)?,
                            }
                        }
                    },
                })
            }
        };
        Ok(ActiveDialogue {
            activation: active.activation,
            content: active.content,
            line: active.line,
            result: active.result,
            captures: active
                .captures
                .into_iter()
                .map(RuntimePayload::into_value)
                .collect(),
            values: active.values.into_boxed_slice(),
            voice: active.voice,
            phase,
            elapsed_nanos: active.elapsed_nanos,
            pending_content_events: active.pending_content_events,
            pending_advance: active.pending_advance,
            pending_line_outcomes: active.pending_line_outcomes,
        })
    }

    fn restore_dialogue_store(
        &self,
        snapshot: AwbcProductDialogueSnapshotState,
    ) -> Result<super::ProductDialogueStore, AwbcProductStepBuildError> {
        match snapshot {
            AwbcProductDialogueSnapshotState::Live(dialogues) => Ok(dialogues),
            AwbcProductDialogueSnapshotState::Saved(dialogues) => {
                super::ProductDialogueStore::from_save_snapshot(dialogues, |activation, active, line| {
                    let active = AwbcProductActiveDialogueSnapshot {
                        activation: active.activation,
                        content: active.content,
                        line: active.line,
                        result: active.result,
                        captures: active
                            .captures
                            .into_iter()
                            .map(|capture| {
                                capture
                                    .into_runtime_value()
                                    .map(RuntimePayload::from)
                                    .map_err(|error| {
                                        crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                                            message: error.to_string(),
                                        }
                                    })
                            })
                            .collect::<Result<_, _>>()?,
                        values: active
                            .values
                            .into_iter()
                            .map(|binding| {
                                Ok::<
                                    _,
                                    crate::line_task::RuntimeDialogueRegistrySnapshotError,
                                >(crate::plan::RuntimeDialogueValueBinding {
                                    slot: binding.slot,
                                    value: binding.value.into_runtime_value().map_err(|error| {
                                        crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                                            message: error.to_string(),
                                        }
                                    })?,
                                })
                            })
                            .collect::<Result<_, _>>()?,
                        voice: active.voice,
                        phase: restore_dialogue_phase_from_save(active.phase).map_err(
                            |message| {
                                crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                                    message,
                                }
                            },
                        )?,
                        elapsed_nanos: active.elapsed_nanos,
                        pending_content_events: active.pending_content_events,
                        pending_advance: active.pending_advance,
                        pending_line_outcomes: active.pending_line_outcomes,
                    };
                    let active = self.restore_active_dialogue(active).map_err(|error| {
                        crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                            message: error.to_string(),
                        }
                    })?;
                    validate_product_dialogue_phase(activation, &active, line)?;
                    Ok(active)
                })
                .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                })
            }
        }
    }

    pub(super) fn validate_snapshot(
        &self,
        snapshot: &AwbcProductExecutorSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        snapshot
            .fiber
            .validate_for_program(&self.program)
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
        for child in &snapshot.child_fibers {
            child
                .fiber
                .validate_for_program(&self.program)
                .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                })?;
            self.validate_child_owner(&child.owner)?;
        }
        self.validate_live_flow_bindings(snapshot)?;
        let dialogues = self.restore_dialogue_store(snapshot.dialogues.clone())?;
        if let Some(active) = dialogues.active_frame() {
            let content = self
                .program
                .content_units
                .get(active.content.index())
                .ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                    message: "active dialogue snapshot references missing content".to_owned(),
                })?;
            let expected_captures = content
                .line_task_group
                .and_then(|group| self.program.line_task_groups.get(group.index()))
                .map_or(0, |group| group.captures.len());
            if active.captures.len() != expected_captures {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "active dialogue snapshot capture arity disagrees with its content group"
                            .to_owned(),
                });
            }
            let pending = active
                .pending_content_events
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if pending.len() != active.pending_content_events.len()
                || active.line_task().is_none() && !pending.is_empty()
                || pending.iter().any(|event| match event {
                    RuntimeDialogueContentEventKind::Mark(mark) => content
                        .marks
                        .get(mark.index())
                        .is_none_or(|row| row.id != *mark),
                    RuntimeDialogueContentEventKind::Effect(effect) => {
                        effect.get().get() > content.effect_site_count
                    }
                })
                || active.line_task().is_some_and(|line_task| {
                    line_task
                        .snapshot()
                        .consumed_content_events()
                        .iter()
                        .any(|event| pending.contains(event))
                })
            {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "active dialogue snapshot has invalid pending content ingress"
                        .to_owned(),
                });
            }
            if active
                .captures
                .iter()
                .any(|capture| !capture.ownership().permits_copy())
            {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: crate::line_task::LineRuntimeError::AffineGroupCapture.to_string(),
                });
            }
            let projection = AwbcProductActiveDialogueSnapshot {
                activation: active.activation.clone(),
                content: active.content,
                line: active.line.clone(),
                result: active.result.clone(),
                captures: active
                    .captures
                    .iter()
                    .cloned()
                    .map(RuntimePayload::from)
                    .collect(),
                values: active.values.to_vec(),
                voice: active.voice.clone(),
                phase: snapshot_dialogue_phase(&active.phase),
                elapsed_nanos: active.elapsed_nanos,
                pending_content_events: active.pending_content_events.clone(),
                pending_advance: active.pending_advance,
                pending_line_outcomes: active.pending_line_outcomes.clone(),
            };
            self.restore_active_dialogue(projection)?;

            let shared_line = dialogues.active_line().ok_or_else(|| {
                AwbcProductStepBuildError::RestoreSnapshot {
                    message: "active dialogue snapshot is missing its command authority".to_owned(),
                }
            })?;
            validate_product_dialogue_phase(&active.activation, active, shared_line).map_err(
                |error| AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                },
            )?;
            if let Some(reducer) = dialogues.active_frame().and_then(ActiveDialogue::line_task) {
                shared_line
                    .restore_admit_reducer(&active.activation, &reducer.snapshot())
                    .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                        message: format!(
                            "line reducer is not isomorphic to scheduled handle authority: {error}"
                        ),
                    })?;
            }
            let expected_child_custody = shared_line
                .scheduled_child_custody_keys()
                .into_iter()
                .collect::<BTreeSet<_>>();
            let pending = match &active.phase {
                super::ProductDialoguePhase::Activating { pending, .. } => pending.as_ref(),
                super::ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                    state: super::ProductDialogueClosingState::Activation { pending, .. },
                    ..
                }) => pending.as_ref(),
                super::ProductDialoguePhase::Reducing { .. }
                | super::ProductDialoguePhase::Publishing { .. }
                | super::ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                    state: super::ProductDialogueClosingState::LineTask { .. },
                    ..
                }) => None,
            };
            if let Some(pending) = pending {
                let command = shared_line
                    .issued_command(pending.command())
                    .ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                        message: "pending line operation has no exact issued command".to_owned(),
                    })?;
                let exact = matches!(
                    (pending, command),
                    (
                        super::ProductPendingLineOperation::AcquireActor { .. },
                        crate::presentation::RuntimeLineHostCommand::Stage(
                            crate::presentation::RuntimeStageCommand::AcquireActor { .. }
                        )
                    ) | (
                        super::ProductPendingLineOperation::ActorLook { .. },
                        crate::presentation::RuntimeLineHostCommand::Stage(
                            crate::presentation::RuntimeStageCommand::SetCharacterLook { .. }
                        )
                    ) | (
                        super::ProductPendingLineOperation::StartVoice { .. },
                        crate::presentation::RuntimeLineHostCommand::Voice(
                            crate::presentation::RuntimeVoiceCommand::StartDialogueVoice { .. }
                        )
                    )
                );
                if !exact {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "pending line operation command kind is inconsistent".to_owned(),
                    });
                }
            }
            let mut actual_child_custody = BTreeSet::new();
            for child in &snapshot.child_fibers {
                let AwbcProductChildFiberOwnerSnapshot::LineTask { content, tag, .. } =
                    &child.owner
                else {
                    continue;
                };
                let tag = restore_work_tag(tag.clone()).ok_or_else(|| {
                    AwbcProductStepBuildError::RestoreSnapshot {
                        message: "scheduled child snapshot has an invalid work tag".to_owned(),
                    }
                })?;
                let Some(token) = tag.scheduled_token().cloned() else {
                    continue;
                };
                let key = (tag, token);
                if *content != active.content || !actual_child_custody.insert(key) {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message:
                            "scheduled child snapshot is duplicate or belongs to stale content"
                                .to_owned(),
                    });
                }
            }
            if actual_child_custody != expected_child_custody {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "scheduled packet ChildFiber custody is not isomorphic to child snapshots"
                            .to_owned(),
                });
            }
            let expected_joined = active
                .line_task()
                .map(|live| live.snapshot().outstanding_tags())
                .unwrap_or_default();
            let mut actual_joined = BTreeSet::new();
            let mut actual_child_tokens = BTreeMap::new();
            let mut all_child_tokens = BTreeSet::new();
            for child in &snapshot.child_fibers {
                let AwbcProductChildFiberOwnerSnapshot::LineTask {
                    content,
                    tag,
                    policy,
                    ..
                } = &child.owner
                else {
                    continue;
                };
                if policy.join == AwbcProductLineTaskJoinSnapshot::Detached {
                    continue;
                }
                if *content != active.content {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "joined line-owned child fiber has no active dialogue owner"
                            .to_owned(),
                    });
                }
                let tag = restore_work_tag(tag.clone()).ok_or_else(|| {
                    AwbcProductStepBuildError::RestoreSnapshot {
                        message: "joined line-owned child fiber has an invalid work identity"
                            .to_owned(),
                    }
                })?;
                if !actual_joined.insert(tag.clone()) {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "joined line-task work has more than one child fiber".to_owned(),
                    });
                }
                let tokens = super::line::product_fiber_handle_owners(
                    self.facade_fiber.execution,
                    &child.fiber,
                )
                .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                })?
                .into_keys()
                .map(|token| {
                    if !all_child_tokens.insert(token.clone()) {
                        return Err(AwbcProductStepBuildError::RestoreSnapshot {
                            message: "affine line handle occurs in more than one child fiber"
                                .to_owned(),
                        });
                    }
                    Ok(token)
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
                actual_child_tokens.insert(tag, tokens);
            }
            if actual_joined != expected_joined {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "joined reducer outstanding work is not isomorphic to child snapshots"
                        .to_owned(),
                });
            }
            let mut ledger_child_tokens = BTreeMap::<_, BTreeSet<_>>::new();
            for (token, lease) in shared_line.ledger().leases() {
                if let crate::line_task::RuntimeHandleOwnerSlot::ChildScope(tag) = lease.owner() {
                    ledger_child_tokens
                        .entry(tag.clone())
                        .or_default()
                        .insert(token.clone());
                }
            }
            for tag in &expected_joined {
                ledger_child_tokens.entry(tag.clone()).or_default();
            }
            if actual_child_tokens != ledger_child_tokens {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "child fiber affine graph is not isomorphic to ChildScope ledger ownership"
                            .to_owned(),
                });
            }
        }
        if let Some(active) = &snapshot.active_choice {
            self.validate_active_choice(active)?;
        }
        if let Some(pending) = &snapshot.pending_host_call
            && self.program.host_calls.get(pending.call.index()).is_none()
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "pending host-call snapshot references missing AWBC host call".to_owned(),
            });
        }
        if snapshot
            .emitted_content
            .iter()
            .any(|content| self.program.content_units.get(content.index()).is_none())
            || snapshot
                .stream_sequences
                .keys()
                .any(|stream| self.program.stream_plans.get(stream.index()).is_none())
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "executor snapshot references missing AWBC content or stream table"
                    .to_owned(),
            });
        }
        validate_task_publications(snapshot)?;
        Ok(())
    }

    fn validate_child_owner(
        &self,
        owner: &AwbcProductChildFiberOwnerSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        let AwbcProductChildFiberOwnerSnapshot::LineTask {
            content,
            tag,
            policy,
            ..
        } = owner
        else {
            return Ok(());
        };
        let Some(group) = self
            .program
            .content_units
            .get(content.index())
            .and_then(|content| content.line_task_group)
            .and_then(|group| self.program.line_task_groups.get(group.index()))
        else {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "line-owned child fiber references content without a line-task group"
                    .to_owned(),
            });
        };
        if matches!(policy.cancel, AwbcProductLineTaskCancelSnapshot::Detach) {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "line-owned child fiber uses unverified detach ownership".to_owned(),
            });
        }
        match tag.work {
            AwbcProductLineTaskWorkSnapshot::Node(node) => {
                let Some(global) = group.nodes.start.checked_add(node) else {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "line-owned child fiber node index overflows its group".to_owned(),
                    });
                };
                if node >= group.nodes.len
                    || !matches!(
                        self.program.line_task_nodes.get(global as usize),
                        Some(crate::awbc::schema::AwbcLineTaskNode::Action(_))
                    )
                {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "line-owned child fiber references a non-action node outside its content group"
                            .to_owned(),
                    });
                }
            }
            AwbcProductLineTaskWorkSnapshot::Cancellation(mark) => {
                if !group
                    .cancel_handlers
                    .iter()
                    .any(|handler| handler.trigger == mark)
                {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "line-owned child fiber references unknown cancellation work"
                            .to_owned(),
                    });
                }
            }
            AwbcProductLineTaskWorkSnapshot::Cleanup(exit) => {
                let present = match exit {
                    AwbcProductLineTaskExitSnapshot::Completed => group.cleanup_completed.is_some(),
                    AwbcProductLineTaskExitSnapshot::Cancelled => group.cleanup_cancelled.is_some(),
                    AwbcProductLineTaskExitSnapshot::Failed => group.cleanup_failed.is_some(),
                };
                if !present {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "line-owned child fiber references unavailable cleanup work"
                            .to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_active_choice(
        &self,
        active: &AwbcProductActiveChoiceSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        let Some(choice) = self.program.choices.get(active.choice.index()) else {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "active choice snapshot references missing AWBC choice".to_owned(),
            });
        };
        let start = usize::try_from(choice.options.start).map_err(|_| {
            AwbcProductStepBuildError::RestoreSnapshot {
                message: "active choice source range start exceeds usize".to_owned(),
            }
        })?;
        let end = usize::try_from(choice.options.checked_end().ok_or_else(|| {
            AwbcProductStepBuildError::RestoreSnapshot {
                message: "active choice source range overflows u32".to_owned(),
            }
        })?)
        .map_err(|_| AwbcProductStepBuildError::RestoreSnapshot {
            message: "active choice source range end exceeds usize".to_owned(),
        })?;
        let mut previous = None;
        for source_index in &active.option_indices {
            if *source_index < start
                || *source_index >= end
                || previous.is_some_and(|previous| previous >= *source_index)
            {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "active choice snapshot does not match its exact typed source option range"
                            .to_owned(),
                });
            }
            if self.program.choice_options.get(*source_index).is_none() {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "active choice snapshot references a missing source option".to_owned(),
                });
            }
            previous = Some(*source_index);
        }
        Ok(())
    }

    fn restore_active_choice(
        &self,
        active: AwbcProductActiveChoiceSnapshot,
    ) -> Result<ActiveChoice, AwbcProductStepBuildError> {
        let options = active
            .option_indices
            .iter()
            .map(|source_index| {
                self.program
                    .choice_options
                    .get(*source_index)
                    .map(|option| self.choice_runtime_option(option))
                    .ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                        message: "active choice snapshot references a missing source option"
                            .to_owned(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ActiveChoice {
            choice: active.choice,
            public_id: active.public_id,
            options,
            option_indices: active.option_indices,
        })
    }

    fn live_flow_bindings(&self) -> Vec<AwbcFlowBinding> {
        let mut functions = self
            .fiber
            .frames
            .iter()
            .map(|frame| frame.function)
            .chain(
                self.child_fibers
                    .iter()
                    .flat_map(|child| child.fiber.frames.iter().map(|frame| frame.function)),
            )
            .collect::<BTreeSet<_>>();
        if let Some(active) = &self.active_choice {
            for target in active
                .options
                .iter()
                .filter_map(|option| option.target.as_ref())
            {
                if let Some(function) = self.program.flow_function(target) {
                    functions.insert(function);
                }
            }
        }
        self.program
            .flow_bindings
            .iter()
            .filter(|binding| functions.contains(&binding.function))
            .cloned()
            .collect()
    }

    fn validate_live_flow_bindings(
        &self,
        snapshot: &AwbcProductExecutorSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        let mut flows = BTreeSet::new();
        let mut functions = BTreeSet::new();
        for binding in &snapshot.live_flow_bindings {
            if !flows.insert(binding.flow.clone()) || !functions.insert(binding.function) {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "snapshot repeats a live semantic Flow binding".to_owned(),
                });
            }
            if self.program.flow_function(&binding.flow) != Some(binding.function) {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: format!(
                        "snapshot Flow `{}` no longer owns AWBC function {}",
                        binding.flow.canonical_label(),
                        binding.function.0
                    ),
                });
            }
        }
        for frame in snapshot.fiber.frames.iter().chain(
            snapshot
                .child_fibers
                .iter()
                .flat_map(|child| &child.fiber.frames),
        ) {
            let is_flow = self
                .program
                .functions
                .get(frame.function.index())
                .is_some_and(|function| function.kind == AwbcFunctionKind::Flow);
            if is_flow && !functions.contains(&frame.function) {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: format!(
                        "snapshot Flow frame {} has no semantic identity evidence",
                        frame.function.0
                    ),
                });
            }
        }
        if let Some(active) = &snapshot.active_choice {
            for target in active.option_indices.iter().filter_map(|source_index| {
                self.program
                    .choice_options
                    .get(*source_index)
                    .map(|option| self.choice_runtime_option(option))
                    .and_then(|option| option.target)
            }) {
                if !flows.contains(&target) {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: format!(
                            "snapshot choice target `{}` has no semantic Flow binding evidence",
                            target.canonical_label()
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    pub(super) fn rebuild_facade_stream_states_from_compact(&mut self) {
        self.facade_fiber.stream_states.clear();
        for (index, _) in self.program.stream_plans.iter().enumerate() {
            let Some(index) = u32::try_from(index).ok() else {
                continue;
            };
            let plan = AwbcStreamPlanId(index);
            let id = stream_id_for(&self.program, plan);
            let mut runtime = StreamRuntimeState::new(id.clone());
            if let Some(compact) = self.fiber.streams.iter().find(|state| state.plan == plan) {
                runtime.queue = compact
                    .queue
                    .iter()
                    .cloned()
                    .map(RuntimePayload::from)
                    .collect();
                runtime.closed = compact.closed;
                runtime.emitted_count = compact.emitted_count;
            }
            self.facade_fiber.stream_states.insert(id, runtime);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AwbcProductDialogueClosingSaveSnapshot, AwbcProductDialogueClosingStateSaveSnapshot,
        AwbcProductDialoguePhaseSaveSnapshot, validate_product_dialogue_phase,
    };
    use crate::awbc::fiber::{
        AwbcFiberStateSnapshot, FiberBudget, FiberCursor, FiberStatus, FiberTrap,
    };
    use crate::awbc::schema::{AwbcBlockId, AwbcEntryId, AwbcFunctionId, AwbcTrapCode, AwbcTypeId};
    use crate::runtime_id::{
        DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeFiberInstanceId,
        RuntimeIdCursor, RuntimePersistentFiberId,
    };
    use std::num::{NonZeroU32, NonZeroU64};

    fn closing_phase() -> AwbcProductDialoguePhaseSaveSnapshot {
        AwbcProductDialoguePhaseSaveSnapshot::Closing(AwbcProductDialogueClosingSaveSnapshot {
            failure: FiberTrap {
                code: AwbcTrapCode::InternalInvariant,
                message: Some("fixture failure".to_owned()),
                source_map: None,
            },
            state: AwbcProductDialogueClosingStateSaveSnapshot::Activation {
                fiber: AwbcFiberStateSnapshot {
                    instance: RuntimeFiberInstanceId::from_allocated(NonZeroU64::MIN),
                    next_frame_instance: RuntimeIdCursor::initial(),
                    generation: 1,
                    entry: AwbcEntryId(0),
                    cursor: FiberCursor {
                        function: AwbcFunctionId(0),
                        block: AwbcBlockId(0),
                        instruction_offset: 0,
                    },
                    frames: Vec::new(),
                    status: FiberStatus::Running,
                    suspension: None,
                    terminal: None,
                    budget: FiberBudget {
                        remaining: 1,
                        quantum: 1,
                    },
                    line_cursor: 0,
                    streams: Vec::new(),
                },
                pending: None,
            },
        })
    }

    #[test]
    fn closing_save_payload_rejects_unknown_outer_and_variant_fields() {
        let phase = closing_phase();
        let encoded = serde_json::to_value(&phase).expect("serialize closing phase");
        assert_eq!(
            serde_json::from_value::<AwbcProductDialoguePhaseSaveSnapshot>(encoded.clone())
                .expect("canonical closing phase"),
            phase
        );

        let mut outer = encoded.clone();
        outer["Closing"]["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<AwbcProductDialoguePhaseSaveSnapshot>(outer).is_err());

        let mut variant = encoded;
        variant["Closing"]["state"]["Activation"]["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<AwbcProductDialoguePhaseSaveSnapshot>(variant).is_err());
    }

    #[test]
    fn closing_frame_requires_abandoned_result_authority() {
        let content = RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN);
        let activation = DialogueActivationId::new(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x4c; 32])
                .expect("artifact"),
            RuntimePersistentFiberId::from_allocated(7),
            content,
            0,
        );
        let phase = closing_phase();
        let AwbcProductDialoguePhaseSaveSnapshot::Closing(closing) = phase else {
            unreachable!("fixture is closing")
        };
        let AwbcProductDialogueClosingStateSaveSnapshot::Activation { fiber, .. } = closing.state
        else {
            unreachable!("fixture is activation closing")
        };
        let frame = super::super::ActiveDialogue {
            activation: activation.clone(),
            content: crate::awbc::schema::AwbcContentUnitId(0),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.fixture")
                .expect("line"),
            captures: Box::new([]),
            values: Box::new([]),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            result: crate::awbc::schema::AwbcDialogueResultTarget {
                ty: AwbcTypeId(0),
                pattern: crate::awbc::schema::AwbcPatternId(0),
                destination: crate::awbc::schema::AwbcRegisterId(0),
            },
            phase: super::super::ProductDialoguePhase::Closing(
                super::super::ProductDialogueClosing {
                    failure: closing.failure,
                    state: super::super::ProductDialogueClosingState::Activation {
                        fiber: fiber.into_live().expect("live fiber"),
                        pending: None,
                    },
                },
            ),
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
        };
        let mut line = crate::line_task::RuntimeDialogueActivationState::<AwbcTypeId>::new();
        assert!(validate_product_dialogue_phase(&activation, &frame, &line).is_err());
        line.abandon().expect("abandon result");
        assert!(validate_product_dialogue_phase(&activation, &frame, &line).is_ok());
    }
}
