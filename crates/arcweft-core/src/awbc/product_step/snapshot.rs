use super::{
    ActiveChoice, ActiveDialogue, AwbcProductStepBuildError, AwbcProductStepExecutor,
    PendingHostCall, ProductChildFiber, ProductChildFiberOwner, ProductLineTaskFiberPhase,
    stream_id_for,
};
use crate::awbc::fiber::{AwbcFiberStateSnapshot, FiberState};
use crate::awbc::schema::{
    AwbcChoiceId, AwbcContentUnitId, AwbcFlowBinding, AwbcFunctionKind, AwbcHostCallId,
    AwbcStreamPlanId,
};
use crate::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineTaskExitPolicy, LineTaskLiveSnapshot,
    LineTaskLiveState, LineTaskNodeState, LineTaskPhase, LineTaskWork, LineTaskWorkTag, ScopeExit,
};
use crate::observation::RuntimeObservationState;
use crate::runtime_id::{RuntimeDialogueMarkId, RuntimeLineTaskNodeId};
use crate::step::RuntimeHostCallId;
use crate::stream::StreamRuntimeState;
use crate::task::TaskId;
use crate::value::RuntimePayload;
use std::collections::{BTreeMap, BTreeSet};

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
        activation: tag.activation.value(),
        work: snapshot_work(tag.work),
    }
}

fn restore_work_tag(tag: AwbcProductLineTaskWorkTagSnapshot) -> Option<LineTaskWorkTag> {
    Some(LineTaskWorkTag {
        activation: crate::line_task::LineTaskActivationId::from_value(tag.activation),
        work: restore_work(tag.work)?,
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
        activation: state.activation(),
        phase: match state.phase() {
            LineTaskPhase::Active => AwbcProductLineTaskPhaseSnapshot::Active,
            LineTaskPhase::Closing { exit } => AwbcProductLineTaskPhaseSnapshot::Closing {
                exit: snapshot_exit(exit),
            },
            LineTaskPhase::Closed { exit } => AwbcProductLineTaskPhaseSnapshot::Closed {
                exit: snapshot_exit(exit),
            },
        },
        node_states: state
            .node_states()
            .iter()
            .copied()
            .map(snapshot_node_state)
            .collect(),
        outstanding: state
            .outstanding()
            .iter()
            .copied()
            .map(snapshot_work)
            .collect(),
        active_roots: state
            .active_roots()
            .iter()
            .copied()
            .map(snapshot_node_index)
            .collect(),
        cancelling_nodes: state
            .cancelling_nodes()
            .iter()
            .copied()
            .map(snapshot_node_index)
            .collect(),
        cleanup_started: state.cleanup_started(),
    }
}

fn restore_live_snapshot(
    snapshot: AwbcProductLineTaskLiveSnapshot,
) -> Result<LineTaskLiveSnapshot, AwbcProductStepBuildError> {
    let nodes = |nodes: Vec<u32>| {
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
    let outstanding = snapshot
        .outstanding
        .into_iter()
        .map(|work| {
            restore_work(work).ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                message: "line-task snapshot work references an invalid node identity".to_owned(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
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
        snapshot
            .node_states
            .into_iter()
            .map(restore_node_state)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        outstanding.into_boxed_slice(),
        nodes(snapshot.active_roots)?.into_boxed_slice(),
        nodes(snapshot.cancelling_nodes)?.into_boxed_slice(),
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
                tag: snapshot_work_tag(*tag),
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
                tag: restore_work_tag(*tag).expect("validated line-task child owner work tag"),
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

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
pub struct AwbcProductExecutorSnapshot {
    pub fiber: FiberState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_fibers: Vec<AwbcProductChildFiberSnapshot>,
    /// Exact semantic identities for every live Flow function and retained
    /// choice target. Dense function indices alone are not restore authority.
    pub live_flow_bindings: Vec<AwbcFlowBinding>,
    #[serde(default)]
    pub entry_bound: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_dialogue: Option<AwbcProductActiveDialogueSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_choice: Option<AwbcProductActiveChoiceSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_host_call: Option<AwbcProductPendingHostCallSnapshot>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub started_tasks: BTreeSet<TaskId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub emitted_content: BTreeSet<AwbcContentUnitId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    pub next_generation: u64,
    pub next_line_task_activation: u64,
    pub next_host_call_sequence: u64,
    pub next_audio_sequence: u64,
    #[serde(default)]
    pub compact_pure_stats: crate::step::RuntimePureCallStats,
    #[serde(default)]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_fibers: Vec<AwbcProductChildFiberSaveSnapshot>,
    pub live_flow_bindings: Vec<AwbcFlowBinding>,
    #[serde(default)]
    pub entry_bound: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_dialogue: Option<AwbcProductActiveDialogueSaveSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_choice: Option<AwbcProductActiveChoiceSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_host_call: Option<AwbcProductPendingHostCallSnapshot>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub started_tasks: BTreeSet<TaskId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub emitted_content: BTreeSet<AwbcContentUnitId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    pub next_generation: u64,
    pub next_line_task_activation: u64,
    pub next_host_call_sequence: u64,
    pub next_audio_sequence: u64,
    #[serde(default)]
    pub compact_pure_stats: crate::step::RuntimePureCallStats,
    #[serde(default)]
    pub observations: RuntimeObservationState,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductActiveDialogueSaveSnapshot {
    pub content: AwbcContentUnitId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub captures: Vec<crate::value::AwbcRuntimeValueSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_task: Option<AwbcProductLineTaskLiveSnapshot>,
    pub elapsed_nanos: u64,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductChildFiberSaveSnapshot {
    pub owner: AwbcProductChildFiberOwnerSnapshot,
    pub fiber: AwbcFiberStateSnapshot,
}

impl AwbcProductExecutorSaveSnapshot {
    pub fn from_live(snapshot: &AwbcProductExecutorSnapshot) -> Result<Self, String> {
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
            active_dialogue: snapshot
                .active_dialogue
                .as_ref()
                .map(|active| {
                    Ok::<AwbcProductActiveDialogueSaveSnapshot, String>(
                        AwbcProductActiveDialogueSaveSnapshot {
                            content: active.content,
                            captures: active
                                .captures
                                .iter()
                                .map(|capture| {
                                    crate::value::AwbcRuntimeValueSnapshot::from_runtime_value(
                                        capture.value(),
                                    )
                                    .map_err(|error| error.to_string())
                                })
                                .collect::<Result<_, _>>()?,
                            line_task: active.line_task.clone(),
                            elapsed_nanos: active.elapsed_nanos,
                        },
                    )
                })
                .transpose()?,
            active_choice: snapshot.active_choice.clone(),
            pending_host_call: snapshot.pending_host_call.clone(),
            started_tasks: snapshot.started_tasks.clone(),
            emitted_content: snapshot.emitted_content.clone(),
            stream_sequences: snapshot.stream_sequences.clone(),
            next_generation: snapshot.next_generation,
            next_line_task_activation: snapshot.next_line_task_activation,
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
            active_dialogue: self
                .active_dialogue
                .map(|active| {
                    Ok::<AwbcProductActiveDialogueSnapshot, String>(
                        AwbcProductActiveDialogueSnapshot {
                            content: active.content,
                            captures: active
                                .captures
                                .into_iter()
                                .map(|capture| {
                                    capture
                                        .into_runtime_value()
                                        .map(RuntimePayload::from)
                                        .map_err(|error| error.to_string())
                                })
                                .collect::<Result<_, _>>()?,
                            line_task: active.line_task,
                            elapsed_nanos: active.elapsed_nanos,
                        },
                    )
                })
                .transpose()?,
            active_choice: self.active_choice,
            pending_host_call: self.pending_host_call,
            started_tasks: self.started_tasks,
            emitted_content: self.emitted_content,
            stream_sequences: self.stream_sequences,
            next_generation: self.next_generation,
            next_line_task_activation: self.next_line_task_activation,
            next_host_call_sequence: self.next_host_call_sequence,
            next_audio_sequence: self.next_audio_sequence,
            compact_pure_stats: self.compact_pure_stats,
            observations: self.observations,
        })
    }
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
pub struct AwbcProductActiveDialogueSnapshot {
    pub content: AwbcContentUnitId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub captures: Vec<RuntimePayload>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_task: Option<AwbcProductLineTaskLiveSnapshot>,
    pub elapsed_nanos: u64,
}

/// Complete persisted reducer state for one content-owned dialogue group.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct AwbcProductLineTaskLiveSnapshot {
    pub activation: u64,
    pub phase: AwbcProductLineTaskPhaseSnapshot,
    pub node_states: Vec<AwbcProductLineTaskNodeStateSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outstanding: Vec<AwbcProductLineTaskWorkSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_roots: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cancelling_nodes: Vec<u32>,
    #[serde(default)]
    pub cleanup_started: bool,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
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
pub enum AwbcProductLineTaskExitSnapshot {
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
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
pub enum AwbcProductLineTaskWorkSnapshot {
    Node(u32),
    Cancellation(RuntimeDialogueMarkId),
    Cleanup(AwbcProductLineTaskExitSnapshot),
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct AwbcProductLineTaskWorkTagSnapshot {
    pub activation: u64,
    pub work: AwbcProductLineTaskWorkSnapshot,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct AwbcProductLineTaskExitPolicySnapshot {
    pub join: AwbcProductLineTaskJoinSnapshot,
    pub cancel: AwbcProductLineTaskCancelSnapshot,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub enum AwbcProductLineTaskJoinSnapshot {
    Join,
    Detached,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub enum AwbcProductLineTaskCancelSnapshot {
    CancelAndJoin,
    Finish,
    Detach,
}

/// Durable owner identity for every queued compact child fiber.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
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
pub enum AwbcProductLineTaskFiberPhaseSnapshot {
    Active,
    Closing,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
pub struct AwbcProductChildFiberSnapshot {
    pub owner: AwbcProductChildFiberOwnerSnapshot,
    pub fiber: FiberState,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
pub struct AwbcProductActiveChoiceSnapshot {
    pub choice: AwbcChoiceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub option_indices: Vec<usize>,
}

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct AwbcProductPendingHostCallSnapshot {
    pub call: AwbcHostCallId,
    pub id: String,
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
            active_dialogue: self.active_dialogue.as_ref().map(|active| {
                AwbcProductActiveDialogueSnapshot {
                    content: active.content,
                    captures: active
                        .captures
                        .iter()
                        .cloned()
                        .map(RuntimePayload::from)
                        .collect(),
                    line_task: active.line_task.as_ref().map(snapshot_live_state),
                    elapsed_nanos: active.elapsed_nanos,
                }
            }),
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
            emitted_content: self.emitted_content.clone(),
            stream_sequences: self.stream_sequences.clone(),
            next_generation: self.next_generation,
            next_line_task_activation: self.next_line_task_activation,
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
        self.active_dialogue = snapshot
            .active_dialogue
            .map(|active| self.restore_active_dialogue(active))
            .transpose()?;
        self.active_choice = snapshot
            .active_choice
            .map(|active| self.restore_active_choice(active))
            .transpose()?;
        self.pending_host_call = snapshot.pending_host_call.map(|pending| PendingHostCall {
            call: pending.call,
            id: RuntimeHostCallId(pending.id),
        });
        self.started_tasks = snapshot.started_tasks;
        self.emitted_content = snapshot.emitted_content;
        self.stream_sequences = snapshot.stream_sequences;
        self.next_generation = snapshot.next_generation;
        self.next_line_task_activation = snapshot.next_line_task_activation;
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
        let line_task = match (has_group, active.line_task) {
            (false, None) => None,
            (true, Some(snapshot)) => {
                let view = self.line_task_view(active.content).ok_or_else(|| {
                    AwbcProductStepBuildError::RestoreSnapshot {
                        message: "active dialogue snapshot has no verified line-task view"
                            .to_owned(),
                    }
                })?;
                Some(
                    LineTaskLiveState::restore(&view, restore_live_snapshot(snapshot)?).map_err(
                        |error| AwbcProductStepBuildError::RestoreSnapshot {
                            message: format!("invalid line-task reducer snapshot: {error}"),
                        },
                    )?,
                )
            }
            (false, Some(_)) | (true, None) => {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "active dialogue snapshot line-task state disagrees with content ownership"
                            .to_owned(),
                });
            }
        };
        Ok(ActiveDialogue {
            content: active.content,
            captures: active
                .captures
                .into_iter()
                .map(RuntimePayload::into_value)
                .collect(),
            line_task,
            elapsed_nanos: active.elapsed_nanos,
        })
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
        if let Some(active) = &snapshot.active_dialogue {
            let expected_captures = self
                .program
                .content_units
                .get(active.content.index())
                .and_then(|content| content.line_task_group)
                .and_then(|group| self.program.line_task_groups.get(group.index()))
                .map_or(0, |group| group.captures.len());
            if active.captures.len() != expected_captures {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "active dialogue snapshot capture arity disagrees with its content group"
                            .to_owned(),
                });
            }
            self.restore_active_dialogue(active.clone())?;
        }
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
            let Some(active) = snapshot
                .active_dialogue
                .as_ref()
                .filter(|active| active.content == *content)
            else {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "joined line-owned child fiber has no active dialogue owner"
                        .to_owned(),
                });
            };
            let Some(live) = &active.line_task else {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "joined line-owned child fiber has no active reducer state".to_owned(),
                });
            };
            if tag.activation != live.activation || !live.outstanding.contains(&tag.work) {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "joined line-owned child fiber is not outstanding in its reducer activation"
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
