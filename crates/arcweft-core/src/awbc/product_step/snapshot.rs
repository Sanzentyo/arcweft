use super::{
    ActiveChoice, ActiveDialogue, AwbcLineTaskPlanView, AwbcProductSnapshotRestoreTarget,
    AwbcProductStepBuildError, AwbcProductStepExecutor, PendingHostCall, ProductChildFiber,
    ProductChildFiberOwner, ProductLineTaskFiberPhase, stream_id_for,
};
mod task_publication;
use crate::awbc::fiber::{AwbcFiberStateSnapshot, FiberStatus};
use crate::awbc::schema::{
    AwbcChoiceId, AwbcContentUnitId, AwbcFlowBinding, AwbcFunctionKind, AwbcHostCallId,
    AwbcStreamPlanId, AwbcTypeId,
};
use crate::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineTaskExitPolicy, LineTaskLiveSnapshot,
    LineTaskLiveState, LineTaskWork, LineTaskWorkInstance, LineTaskWorkTag, ScopeExit,
};
use crate::runtime_id::{
    DialogueActivationId, RuntimeDialogueEffectCallbackActivationId, RuntimeLineHandleToken,
    RuntimeLineTaskNodeId, RuntimePersistentFiberId,
};
use crate::step::{RuntimeDialogueContentEventKind, RuntimeHostCallId};
use crate::stream::StreamRuntimeState;
use crate::task::{
    HostRestartPolicy, NeedProducerInvocationFrontier, NeedProducerInvocationToken,
    NeedProducerLaunchFrontier, NeedProducerLaunchOriginRestore, NeedProducerLaunchRestore,
    NeedProducerRegistry, NeedProducerRegistrySaveSnapshot, NeedProducerRuntimeArgument,
    NeedProducerSiteDigest, RuntimeNeedProducerState, TaskId, TaskPublicationCursor,
};
use crate::value::{AwbcRuntimeValueSnapshot, RuntimePayload, RuntimeValue};
use std::cmp::Ordering;
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

fn validate_product_dialogue_phase<V, O, E, F, L, P, H, R>(
    program: &crate::awbc::schema::AwbcProgram,
    activation: &DialogueActivationId,
    frame: &super::AwbcProductActiveDialogue<V, O, E, super::AwbcProductDialoguePhase<F, L, P>, H>,
    result: &crate::line_task::RuntimeDialogueResultState<AwbcTypeId, R>,
    is_closed: impl Fn(&L) -> bool,
) -> Result<(), crate::line_task::RuntimeDialogueRegistrySnapshotError> {
    use crate::line_task::RuntimeDialogueResultState;

    let valid = activation == &frame.activation
        && match (&frame.phase, result) {
            (
                super::AwbcProductDialoguePhase::Activating { .. },
                RuntimeDialogueResultState::Uncommitted,
            )
            | (
                super::AwbcProductDialoguePhase::Closing(_),
                RuntimeDialogueResultState::Abandoned,
            ) => frame.pending_content_events.is_empty() && !frame.pending_advance,
            (
                super::AwbcProductDialoguePhase::Activating { .. },
                RuntimeDialogueResultState::Committed { ty, .. },
            ) => {
                *ty == frame.result.ty
                    && frame.pending_content_events.is_empty()
                    && !frame.pending_advance
            }
            (
                super::AwbcProductDialoguePhase::Reducing { .. },
                RuntimeDialogueResultState::Committed { ty, .. }
                | RuntimeDialogueResultState::Selected { ty, .. },
            ) => *ty == frame.result.ty,
            (
                super::AwbcProductDialoguePhase::Reducing { .. },
                RuntimeDialogueResultState::Uncommitted,
            ) => program
                .content_units
                .get(frame.content.index())
                .and_then(|content| content.line_task_group)
                .and_then(|group_id| program.line_task_groups.get(group_id.index()))
                .is_some_and(|group| {
                    group.result_type == frame.result.ty
                        && super::AwbcLineTaskPlanView::new(program, group)
                            .is_some_and(|view| view.has_mark_result_selector())
                }),
            (
                super::AwbcProductDialoguePhase::Publishing { line_task },
                RuntimeDialogueResultState::Publishing { ty, .. },
            ) => {
                is_closed(line_task)
                    && *ty == frame.result.ty
                    && frame.pending_content_events.is_empty()
                    && !frame.pending_advance
            }
            (
                super::AwbcProductDialoguePhase::Activating { .. }
                | super::AwbcProductDialoguePhase::Reducing { .. }
                | super::AwbcProductDialoguePhase::Publishing { .. }
                | super::AwbcProductDialoguePhase::Closing(_),
                RuntimeDialogueResultState::Uncommitted
                | RuntimeDialogueResultState::Committed { .. }
                | RuntimeDialogueResultState::Selected { .. }
                | RuntimeDialogueResultState::Publishing { .. }
                | RuntimeDialogueResultState::Published
                | RuntimeDialogueResultState::Abandoned,
            )
            | (super::AwbcProductDialoguePhase::Transitioning, _) => false,
        };
    valid.then_some(()).ok_or_else(|| {
        crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
            message: "dialogue frame phase/result authority is not isomorphic".to_owned(),
        }
    })
}

fn activation_fiber_in_phase<V, O, E, F, L, P, H>(
    frame: &super::AwbcProductActiveDialogue<V, O, E, super::AwbcProductDialoguePhase<F, L, P>, H>,
) -> Option<&F> {
    match &frame.phase {
        super::AwbcProductDialoguePhase::Activating { fiber, .. }
        | super::AwbcProductDialoguePhase::Closing(super::AwbcProductDialogueClosing {
            state: super::AwbcProductDialogueClosingState::Activation { fiber, .. },
            ..
        }) => Some(fiber),
        super::AwbcProductDialoguePhase::Reducing { .. }
        | super::AwbcProductDialoguePhase::Publishing { .. }
        | super::AwbcProductDialoguePhase::Transitioning
        | super::AwbcProductDialoguePhase::Closing(super::AwbcProductDialogueClosing {
            state: super::AwbcProductDialogueClosingState::LineTask { .. },
            ..
        }) => None,
    }
}

fn snapshot_work(work: &LineTaskWork) -> AwbcProductLineTaskWorkSnapshot {
    match work {
        LineTaskWork::Node(node) => {
            AwbcProductLineTaskWorkSnapshot::Node(snapshot_node_index(*node))
        }
        LineTaskWork::Cancellation(action) => {
            AwbcProductLineTaskWorkSnapshot::Cancellation(action.clone())
        }
        LineTaskWork::Cleanup(exit) => {
            AwbcProductLineTaskWorkSnapshot::Cleanup(snapshot_exit(*exit))
        }
        LineTaskWork::Defer(id) => AwbcProductLineTaskWorkSnapshot::Defer(*id),
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
        AwbcProductLineTaskWorkSnapshot::Cancellation(action) => {
            Some(LineTaskWork::Cancellation(action))
        }
        AwbcProductLineTaskWorkSnapshot::Cleanup(exit) => {
            Some(LineTaskWork::Cleanup(restore_exit(exit)))
        }
        AwbcProductLineTaskWorkSnapshot::Defer(id) => Some(LineTaskWork::Defer(id)),
    }
}

fn snapshot_work_tag(tag: &LineTaskWorkTag) -> AwbcProductLineTaskWorkTagSnapshot {
    AwbcProductLineTaskWorkTagSnapshot {
        instance: match tag.instance() {
            LineTaskWorkInstance::Activation(activation) => {
                AwbcProductLineTaskWorkInstanceSnapshot::Activation(activation.clone())
            }
            LineTaskWorkInstance::Scheduled(token) => {
                AwbcProductLineTaskWorkInstanceSnapshot::Scheduled(token.clone())
            }
        },
        work: snapshot_work(&tag.work()),
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

impl ProductChildFiberOwner {
    fn snapshot(&self) -> AwbcProductChildFiberOwnerSnapshot {
        match self {
            Self::Independent => AwbcProductChildFiberOwnerSnapshot::Independent,
            Self::Program { launch, parent } => AwbcProductChildFiberOwnerSnapshot::Program {
                handle: launch.handle(),
                parent: *parent,
            },
            Self::LineTask {
                content,
                tag,
                policy,
                phase,
            } => AwbcProductChildFiberOwnerSnapshot::LineTask {
                content: *content,
                tag: snapshot_work_tag(tag),
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
            Self::Deferred {
                content,
                activation,
                registration,
                site,
            } => AwbcProductChildFiberOwnerSnapshot::Deferred {
                content: *content,
                activation: activation.clone(),
                registration: *registration,
                site: *site,
            },
            Self::ScopedDeferred {
                content,
                activation,
                frame,
                scope,
                registration,
                site,
            } => AwbcProductChildFiberOwnerSnapshot::ScopedDeferred {
                content: *content,
                activation: activation.clone(),
                frame: *frame,
                scope: *scope,
                registration: *registration,
                site: *site,
            },
        }
    }

    fn restore(
        snapshot: &AwbcProductChildFiberOwnerSnapshot,
        registry: &NeedProducerRegistry,
    ) -> Result<Self, String> {
        Ok(match snapshot {
            AwbcProductChildFiberOwnerSnapshot::Independent => Self::Independent,
            AwbcProductChildFiberOwnerSnapshot::Program { handle, parent } => Self::Program {
                launch: registry
                    .program_launch_for_handle(*handle)
                    .ok_or_else(|| {
                        "Program child has no corresponding executing launch".to_owned()
                    })?
                    .clone(),
                parent: *parent,
            },
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
            AwbcProductChildFiberOwnerSnapshot::Deferred {
                content,
                activation,
                registration,
                site,
            } => Self::Deferred {
                content: *content,
                activation: activation.clone(),
                registration: *registration,
                site: *site,
            },
            AwbcProductChildFiberOwnerSnapshot::ScopedDeferred {
                content,
                activation,
                frame,
                scope,
                registration,
                site,
            } => Self::ScopedDeferred {
                content: *content,
                activation: activation.clone(),
                frame: *frame,
                scope: *scope,
                registration: *registration,
                site: *site,
            },
        })
    }
}

/// Inert Product state retaining the original Source program leases.
/// [`AwbcProductExecutorSaveSnapshot::from_live`] creates its persistence data.
///
/// Retained Source authority has no direct wire encoder or decoder:
///
/// ```compile_fail
/// use arcweft_core::awbc::product_step::AwbcProductExecutorSnapshot;
/// fn require_wire<T: serde::Serialize>() {}
/// require_wire::<AwbcProductExecutorSnapshot>();
/// ```
///
/// ```compile_fail
/// use arcweft_core::awbc::product_step::AwbcProductExecutorSnapshot;
/// fn require_wire<T: for<'de> serde::Deserialize<'de>>() {}
/// require_wire::<AwbcProductExecutorSnapshot>();
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct AwbcProductExecutorSnapshot {
    pub root: Option<crate::root::RootRuntimeRollbackImage>,
    pub runtime_generation: crate::task::GenerationId,
    pub fiber: AwbcFiberStateSnapshot,
    pub child_fibers: Vec<AwbcProductChildFiberSnapshot>,
    pub dialogue_effect_callback_activations: BTreeSet<RuntimeDialogueEffectCallbackActivationId>,
    /// Exact semantic identities for every live Flow function and retained
    /// choice target. Dense function indices alone are not restore authority.
    pub live_flow_bindings: Vec<AwbcFlowBinding>,
    pub entry_bound: bool,
    pub(super) dialogues: AwbcProductDialogueSnapshotState,
    pub active_choice: Option<AwbcProductActiveChoiceSnapshot>,
    pub pending_host_call: Option<AwbcProductPendingHostCallSnapshot>,
    pub started_tasks: BTreeSet<TaskId>,
    pub task_publications: BTreeMap<TaskId, TaskPublicationCursor>,
    pub need_publications: BTreeMap<
        (RuntimePersistentFiberId, crate::task::TaskCorrelation),
        crate::task::RuntimeNeedPublicationCursor,
    >,
    pub need_producers: NeedProducerRegistrySaveSnapshot,
    pub queued_task_events: VecDeque<AwbcProductTaskEventSaveSnapshot>,
    pub emitted_content: BTreeSet<AwbcContentUnitId>,
    pub stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    pub next_generation: u64,
    pub next_fiber_instance: crate::runtime_id::RuntimeIdCursor,
    pub dialogue_occurrences: Vec<AwbcProductDialogueOccurrenceSnapshot>,
    pub next_host_call_sequence: u64,
    pub next_audio_sequence: u64,
    pub compact_pure_stats: crate::step::RuntimePureCallStats,
    pub observations: crate::observation::RuntimeObservationSaveSnapshot,
}

/// AWBC session-save projection of [`AwbcProductExecutorSnapshot`].
///
/// The in-memory snapshot remains useful to embedders and tests, but the
/// persistence payload must not deserialize live `RuntimeValue` directly.
#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductExecutorSaveSnapshot {
    pub root: Option<crate::root::RootStateSaveSnapshotV1>,
    pub runtime_generation: crate::task::GenerationId,
    pub fiber: AwbcFiberStateSnapshot,
    pub child_fibers: Vec<AwbcProductChildFiberSaveSnapshot>,
    pub dialogue_effect_callback_activations: BTreeSet<RuntimeDialogueEffectCallbackActivationId>,
    pub live_flow_bindings: Vec<AwbcFlowBinding>,
    pub entry_bound: bool,
    dialogues: crate::line_task::RuntimeDialogueRegistrySaveSnapshot<
        AwbcProductActiveDialogueSaveSnapshot,
        AwbcTypeId,
    >,
    pub active_choice: Option<AwbcProductActiveChoiceSnapshot>,
    pub pending_host_call: Option<AwbcProductPendingHostCallSnapshot>,
    pub started_tasks: BTreeSet<TaskId>,
    #[serde(with = "task_publication::rows")]
    pub task_publications: BTreeMap<TaskId, TaskPublicationCursor>,
    pub need_publications: Vec<AwbcNeedPublicationSaveSnapshot>,
    pub need_producers: AwbcNeedProducerRegistrySaveSnapshot,
    pub queued_task_events: VecDeque<AwbcProductTaskEventSaveSnapshot>,
    pub emitted_content: BTreeSet<AwbcContentUnitId>,
    pub stream_sequences: BTreeMap<AwbcStreamPlanId, u64>,
    pub next_generation: u64,
    pub next_fiber_instance: crate::runtime_id::RuntimeIdCursor,
    pub dialogue_occurrences: Vec<AwbcProductDialogueOccurrenceSnapshot>,
    pub next_host_call_sequence: u64,
    pub next_audio_sequence: u64,
    pub compact_pure_stats: crate::step::RuntimePureCallStats,
    pub observations: crate::observation::RuntimeObservationSaveSnapshot,
}

/// One waiter-scoped Need publication cursor. This is a row instead of a
/// tuple-keyed map because persisted snapshots must have a portable serde
/// representation (including JSON).
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcNeedPublicationSaveSnapshot {
    pub waiter: RuntimePersistentFiberId,
    pub correlation: crate::task::TaskCorrelation,
    pub cursor: crate::task::RuntimeNeedPublicationCursor,
}

/// A save request cannot complete synchronously while an accepted producer
/// whose contract requires quiescence is still live.
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum AwbcProductSaveError {
    #[error("AWBC Product save requires quiescence for Needs {needs:?}")]
    NeedsQuiescence { needs: Vec<crate::task::NeedId> },
    #[error("invalid AWBC Product save snapshot: {message}")]
    InvalidSnapshot { message: String },
}

/// Serialized registry journal. Producer plans are reprojected from the
/// verified AWBC task-plan table during restore; saved digests only locate the
/// sealed row and are never accepted as plan authority.
#[derive(Clone, Debug, Default, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcNeedProducerRegistrySaveSnapshot {
    pub task_admissions: Vec<crate::task::TaskSubmissionSaveSnapshot>,
    pub launches: Vec<AwbcNeedProducerLaunchSaveSnapshot>,
    pub invocations: Vec<AwbcNeedProducerInvocationSaveSnapshot>,
    pub invocation_frontiers: Vec<AwbcNeedProducerInvocationFrontierSaveSnapshot>,
    pub launch_frontiers: Vec<AwbcNeedProducerLaunchFrontierSaveSnapshot>,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcNeedProducerLaunchSaveSnapshot {
    pub origin: AwbcNeedProducerLaunchOriginSaveSnapshot,
    pub correlation: crate::task::TaskCorrelation,
    pub state: AwbcNeedStateSaveSnapshot,
    pub publication: Option<TaskPublicationCursor>,
    pub task_submitted: bool,
    pub task_terminal: bool,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcNeedProducerLaunchOriginSaveSnapshot {
    Need {
        invocation: NeedProducerInvocationToken,
        source: crate::plan::RuntimeTaskSourceAssertion,
        arguments: Vec<AwbcNeedProducerArgumentSaveSnapshot>,
        task_spec: crate::task::TaskSpecSaveSnapshot,
    },
    Program {
        retirement: crate::task::ProgramTaskRetirementPhase,
    },
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcNeedProducerArgumentSaveSnapshot {
    pub name: Option<String>,
    pub value: AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcNeedProducerInvocationSaveSnapshot {
    pub invocation: NeedProducerInvocationToken,
    pub correlation: crate::task::TaskCorrelation,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcNeedProducerInvocationFrontierSaveSnapshot {
    pub generation: crate::task::GenerationId,
    pub fiber: RuntimePersistentFiberId,
    pub producer_site: NeedProducerSiteDigest,
    pub next_sequence: u64,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcNeedProducerLaunchFrontierSaveSnapshot {
    pub generation: crate::task::GenerationId,
    pub instance_key: crate::task::NeedProducerInstanceKey,
    pub next_ordinal: u64,
}

pub type AwbcNeedStateSaveSnapshot = RuntimeNeedProducerState<AwbcRuntimeValueSnapshot>;

fn visit_runtime_value_graph<E>(
    value: &RuntimeValue,
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    crate::value::visit_runtime_value_graph(value, |nested| visitor(nested))
}

fn visit_runtime_payload<E>(
    payload: &RuntimePayload,
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    visit_runtime_value_graph(payload.value(), visitor)
}

fn visit_task_request_values<E>(
    spec: &crate::task::TaskSpec,
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    for payload in spec.request.payloads() {
        visit_runtime_payload(payload, visitor)?;
    }
    Ok(())
}
fn save_need_producer_registry(
    registry: &NeedProducerRegistrySaveSnapshot,
) -> Result<AwbcNeedProducerRegistrySaveSnapshot, String> {
    Ok(AwbcNeedProducerRegistrySaveSnapshot {
        task_admissions: registry
            .task_admissions
            .iter()
            .cloned()
            .map(|row| row.map_source_admitted(|source| Ok::<_, String>(source.assertion())))
            .collect::<Result<_, _>>()?,
        launches: registry
            .launches
            .iter()
            .map(|launch| {
                Ok(AwbcNeedProducerLaunchSaveSnapshot {
                    origin: match &launch.origin {
                        NeedProducerLaunchOriginRestore::Need { invocation, start } => {
                            AwbcNeedProducerLaunchOriginSaveSnapshot::Need {
                                invocation: *invocation,
                                source: start.source().assertion(),
                                arguments: start
                                    .arguments()
                                    .iter()
                                    .map(|argument| {
                                        Ok(AwbcNeedProducerArgumentSaveSnapshot {
                                            name: argument.name.clone(),
                                            value: argument.value.clone(),
                                        })
                                    })
                                    .collect::<Result<_, String>>()?,
                                task_spec: start.task_spec().clone().map_source_admitted(
                                    |source| Ok::<_, String>(source.assertion()),
                                )?,
                            }
                        }
                        NeedProducerLaunchOriginRestore::Program { retirement } => {
                            AwbcNeedProducerLaunchOriginSaveSnapshot::Program {
                                retirement: *retirement,
                            }
                        }
                    },
                    correlation: launch.correlation,
                    state: launch.state.clone(),
                    publication: launch.publication,
                    task_submitted: launch.task_submitted,
                    task_terminal: launch.task_terminal,
                })
            })
            .collect::<Result<_, String>>()?,
        invocations: registry
            .invocations
            .iter()
            .map(
                |(invocation, need)| AwbcNeedProducerInvocationSaveSnapshot {
                    invocation: *invocation,
                    correlation: *need,
                },
            )
            .collect(),
        invocation_frontiers: registry
            .invocation_frontiers
            .iter()
            .map(|frontier| AwbcNeedProducerInvocationFrontierSaveSnapshot {
                generation: frontier.generation,
                fiber: frontier.fiber,
                producer_site: frontier.producer_site,
                next_sequence: frontier.next_sequence,
            })
            .collect(),
        launch_frontiers: registry
            .launch_frontiers
            .iter()
            .map(|frontier| AwbcNeedProducerLaunchFrontierSaveSnapshot {
                generation: frontier.generation,
                instance_key: frontier.instance_key,
                next_ordinal: frontier.next_ordinal,
            })
            .collect(),
    })
}

fn restore_need_producer_registry(
    saved: AwbcNeedProducerRegistrySaveSnapshot,
    source_program: Option<&crate::plan::task_source::RuntimeTaskSourceProgram>,
    admission: &mut crate::value::RuntimeRetainedValueAdmission,
) -> Result<NeedProducerRegistrySaveSnapshot, String> {
    admission
        .validate_metadata_collection(saved.task_admissions.len())
        .map_err(|error| error.to_string())?;
    admission
        .validate_metadata_collection(saved.launches.len())
        .map_err(|error| error.to_string())?;
    for row in &saved.task_admissions {
        row.admit_retained(admission)
            .map_err(|error| error.to_string())?;
    }
    let task_owner = admission.program_owner().cloned();
    let task_admissions = saved
        .task_admissions
        .into_iter()
        .map(|row| {
            row.map_source_admitted(|assertion| {
                let program =
                    source_program.ok_or("program task has no admitted original Source")?;
                let owner = task_owner
                    .as_ref()
                    .ok_or("program task has no enclosing executable")?;
                crate::task::TaskSnapshotSource::restore_source(
                    assertion,
                    owner,
                    Some(crate::task::TaskSourceRestore { program, admission }),
                )
                .map_err(|error| error.to_string())
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut launches = Vec::with_capacity(saved.launches.len());
    for launch in saved.launches {
        let origin = match launch.origin {
            AwbcNeedProducerLaunchOriginSaveSnapshot::Program { retirement } => {
                NeedProducerLaunchOriginRestore::Program { retirement }
            }
            AwbcNeedProducerLaunchOriginSaveSnapshot::Need {
                invocation,
                source: saved_source,
                arguments: saved_arguments,
                task_spec: saved_spec,
            } => {
                admission
                    .validate_metadata(&saved_source)
                    .map_err(|error| error.to_string())?;
                admission
                    .validate_metadata_collection(saved_arguments.len())
                    .map_err(|error| error.to_string())?;
                let program = source_program
                    .ok_or("selected saved launch has no admitted original Source")?;
                let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
                let mut meter = admission
                    .task_source_meter(limits)
                    .map_err(|error| error.to_string())?;
                let source = program
                    .borrow_assertion(&saved_source, limits, &mut meter)
                    .map_err(|error| error.to_string())?;
                let prepared = source
                    .prepare_selected_producer(program, admission)
                    .map_err(|error| error.to_string())?;
                if saved_arguments.len() != prepared.arguments().len() {
                    return Err(crate::task::RuntimeTaskProducerSourceError::ArgumentCount {
                        actual: saved_arguments.len(),
                        expected: prepared.arguments().len(),
                    }
                    .to_string());
                }
                let arguments = saved_arguments
                    .into_iter()
                    .map(|argument| NeedProducerRuntimeArgument {
                        name: argument.name,
                        value: argument.value,
                    })
                    .collect();
                let task_spec = saved_spec.map_source_admitted(|_| {
                    Err::<crate::plan::RuntimeTaskSource, _>(
                        "a selected host producer cannot restore program execution".to_owned(),
                    )
                })?;
                let start = prepared
                    .restore_start(arguments, task_spec)
                    .map_err(|error| error.to_string())?;
                NeedProducerLaunchOriginRestore::Need { invocation, start }
            }
        };
        launches.push(NeedProducerLaunchRestore {
            origin,
            correlation: launch.correlation,
            state: launch.state,
            publication: launch.publication,
            task_submitted: launch.task_submitted,
            task_terminal: launch.task_terminal,
        });
    }
    Ok(NeedProducerRegistrySaveSnapshot {
        task_admissions,
        launches,
        invocations: saved
            .invocations
            .into_iter()
            .map(|row| (row.invocation, row.correlation))
            .collect(),
        invocation_frontiers: saved
            .invocation_frontiers
            .into_iter()
            .map(|frontier| NeedProducerInvocationFrontier {
                generation: frontier.generation,
                fiber: frontier.fiber,
                producer_site: frontier.producer_site,
                next_sequence: frontier.next_sequence,
            })
            .collect(),
        launch_frontiers: saved
            .launch_frontiers
            .into_iter()
            .map(|frontier| NeedProducerLaunchFrontier {
                generation: frontier.generation,
                instance_key: frontier.instance_key,
                next_ordinal: frontier.next_ordinal,
            })
            .collect(),
    })
}

pub type AwbcProductActiveDialogueSaveSnapshot = super::AwbcProductActiveDialogue<
    AwbcRuntimeValueSnapshot,
    AwbcRuntimeValueSnapshot,
    Vec<AwbcProductDialogueEffectSaveSnapshot>,
    AwbcProductDialoguePhaseSaveSnapshot,
    AwbcProductPendingHostCallSnapshot,
>;

impl Clone for AwbcProductActiveDialogueSaveSnapshot {
    fn clone(&self) -> Self {
        Self {
            activation: self.activation.clone(),
            content: self.content,
            target: self.target.clone(),
            target_type: self.target_type,
            line: self.line.clone(),
            result: self.result.clone(),
            captures: self.captures.clone(),
            task_inputs: self.task_inputs.clone(),
            values: self.values.clone(),
            effect_callbacks: self.effect_callbacks.clone(),
            voice: self.voice.clone(),
            phase: self.phase.clone(),
            elapsed_nanos: self.elapsed_nanos,
            pending_content_events: self.pending_content_events.clone(),
            pending_advance: self.pending_advance,
            pending_line_outcomes: self.pending_line_outcomes.clone(),
            pending_activation_host_call: self.pending_activation_host_call.clone(),
        }
    }
}
#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductDialogueEffectSaveSnapshot {
    pub site: crate::runtime_id::RuntimeDialogueEffectSiteId,
    pub callback: crate::value::AwbcRuntimeValueSnapshot,
}

impl<V, O, E, N, B, R, G, L, P, H>
    super::AwbcProductActiveDialogue<
        V,
        O,
        E,
        super::AwbcProductDialoguePhase<crate::awbc::fiber::FiberState<V, O, N, B, R, G>, L, P>,
        H,
    >
{
    /// Borrows the original frame grammar before either storage role is projected.
    fn admit_retained_values<'a>(
        &'a self,
        program: &crate::awbc::schema::AwbcProgram,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        target: crate::value::RuntimeValueView<'a>,
        callbacks: impl Clone
        + ExactSizeIterator<
            Item = (
                crate::runtime_id::RuntimeDialogueEffectSiteId,
                crate::value::RuntimeValueView<'a>,
            ),
        >,
    ) -> Result<(), crate::line_task::RuntimeDialogueRegistrySnapshotError>
    where
        V: crate::value::RuntimeStoredValue,
    {
        use crate::value::RuntimeValueView;
        let invalid =
            |message: &str| crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                message: message.to_owned(),
            };
        admission.admit_composite(V::admission_origin(), |owner, _| {
            if matches!(owner, Some(crate::task::RuntimeProgramOwner::Awbc(original)) if std::ptr::eq(original.as_ref(), program)) {
                Ok(())
            } else {
                Err(crate::value::AwbcRuntimeValueSnapshotError::Message { message: "dialogue frame admission requires its original AWBC program owner".to_owned() })
            }
        })?;
        admission.validate_metadata(&(
            &self.activation,
            self.content,
            self.target_type,
            &self.line,
            &self.result,
            &self.voice,
            self.elapsed_nanos,
            &self.pending_content_events,
            self.pending_advance,
            &self.pending_line_outcomes,
        ))?;
        for count in [
            self.captures.len(),
            self.task_inputs.len(),
            self.values.len(),
            callbacks.len(),
        ] {
            admission.validate_metadata_collection(count)?;
        }
        let content = program
            .content_units
            .get(self.content.index())
            .ok_or_else(|| invalid("active dialogue snapshot references missing AWBC content"))?;
        let group = content
            .line_task_group
            .and_then(|id| program.line_task_groups.get(id.index()))
            .ok_or_else(|| invalid("active dialogue snapshot content has no line-task group"))?;
        if activation_fiber_in_phase(self).is_some_and(|fiber| {
            fiber.root != crate::awbc::fiber::AwbcFiberRoot::Function(group.activation)
        }) {
            return Err(invalid(
                "dialogue activation fiber root differs from its original content group",
            ));
        }
        let has_reducer = matches!(
            &self.phase,
            super::AwbcProductDialoguePhase::Reducing { .. }
                | super::AwbcProductDialoguePhase::Publishing { .. }
                | super::AwbcProductDialoguePhase::Closing(super::AwbcProductDialogueClosing {
                    state: super::AwbcProductDialogueClosingState::LineTask { .. },
                    ..
                })
        );
        admission.validate_metadata_collection(program.content_templates.len())?;
        let template = program
            .content_templates
            .get(content.template.index())
            .filter(|template| template.id == content.template)
            .ok_or_else(|| invalid("active dialogue template is absent from its AWBC program"))?;
        if self.values.len() != template.slots.len() {
            return Err(invalid(
                "active dialogue value slots disagree with its content template",
            ));
        }
        for (binding, declared) in self.values.iter().zip(&template.slots) {
            let role = match declared.role {
                crate::awbc::schema::AwbcDialogueValueRole::Interpolation => {
                    crate::plan::RuntimeDialogueValueRole::Interpolation
                }
                crate::awbc::schema::AwbcDialogueValueRole::Content => {
                    crate::plan::RuntimeDialogueValueRole::Content
                }
                crate::awbc::schema::AwbcDialogueValueRole::Formatted => {
                    crate::plan::RuntimeDialogueValueRole::Formatted
                }
            };
            if binding.slot != declared.slot || binding.role != role {
                return Err(invalid(
                    "active dialogue value slot has a different template role",
                ));
            }
        }
        let signature = program
            .functions
            .get(group.activation.index())
            .and_then(|function| program.signatures.get(function.signature.index()))
            .ok_or_else(|| invalid("line task group activation signature is absent"))?;
        if self.captures.len() != group.captures.len()
            || self.captures.len() != signature.params.len()
        {
            return Err(invalid(
                "active dialogue snapshot capture arity disagrees with its content group",
            ));
        }
        let expected_inputs = if has_reducer {
            group.captures.len() + group.activation_exports.len()
        } else {
            0
        };
        if self.task_inputs.len() != expected_inputs
            || has_reducer
                && self
                    .task_inputs
                    .iter()
                    .zip(&self.captures)
                    .any(|(left, right)| {
                        !crate::value::runtime_value_views_equal(
                            left.borrowed_view(),
                            right.borrowed_view(),
                        )
                    })
        {
            return Err(invalid(
                "active dialogue snapshot line-task input ABI is invalid",
            ));
        }
        if !matches!(target, RuntimeValueView::Opaque(_))
            || !crate::awbc::fiber::dialogue_target_type_matches_program(program, self.target_type)
        {
            return Err(invalid(
                "active dialogue target is not admitted by its AWBC executable",
            ));
        }
        let mut effects = BTreeSet::new();
        for binding in &self.values {
            admission.validate_metadata(&(binding.slot, binding.role))?;
        }
        for (site, callback) in callbacks.clone() {
            admission.validate_metadata(&site)?;
            if !effects.insert(site) {
                return Err(invalid("dialogue effect snapshot repeats a site"));
            }
            if !matches!(callback, RuntimeValueView::Callable(_)) {
                return Err(invalid("dialogue effect snapshot is not a callable"));
            }
        }
        admission.admit_composite(V::admission_origin(), |owner, resources| {
            resources.validate_view(target, 0, None)?;
            AwbcRuntimeValueSnapshot::validate_admitted_storage_view(target, owner, resources)?;
            for value in self.captures.iter().chain(self.task_inputs.iter()) {
                let view = value.borrowed_view();
                view.admit_ownership(
                    crate::value::ownership::RuntimeValueOwnershipRequirement::Unrestricted,
                    resources,
                    0,
                )?;
                AwbcRuntimeValueSnapshot::validate_admitted_storage_view(view, owner, resources)?;
            }
            for value in self
                .values
                .iter()
                .map(|binding| binding.value.borrowed_view())
                .chain(callbacks.clone().map(|(_, callback)| callback))
            {
                resources.validate_view(value, 0, None)?;
                AwbcRuntimeValueSnapshot::validate_admitted_storage_view(value, owner, resources)?;
            }
            crate::program_types::RuntimeProgramValueType::Awbc {
                program,
                ty: self.target_type,
            }
            .validate_snapshot_storage(
                target,
                &resources.work(),
                V::admission_origin(),
            )?;
            for (capture, ty) in self.captures.iter().zip(&signature.params) {
                crate::program_types::RuntimeProgramValueType::Awbc { program, ty: *ty }
                    .validate_snapshot_storage(
                        capture.borrowed_view(),
                        &resources.work(),
                        V::admission_origin(),
                    )?;
            }
            for (value, export) in self
                .task_inputs
                .iter()
                .skip(group.captures.len())
                .zip(&group.activation_exports)
            {
                crate::program_types::RuntimeProgramValueType::Awbc {
                    program,
                    ty: export.ty,
                }
                .validate_snapshot_storage(
                    value.borrowed_view(),
                    &resources.work(),
                    V::admission_origin(),
                )?;
            }
            for (binding, declared) in self.values.iter().zip(&template.slots) {
                crate::program_types::RuntimeProgramValueType::Awbc {
                    program,
                    ty: declared.semantic_type,
                }
                .validate_snapshot_storage(
                    binding.value.borrowed_view(),
                    &resources.work(),
                    V::admission_origin(),
                )?;
            }
            for (site, callback) in callbacks {
                let declared = template
                    .effects
                    .get(site.index())
                    .filter(|declared| declared.site == site)
                    .ok_or_else(|| crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message:
                            "active dialogue callback site is absent from its content template"
                                .to_owned(),
                    })?;
                let RuntimeValueView::Callable(callback) = callback else {
                    unreachable!("borrowed callback shape was admitted")
                };
                if callback.retained().len() != declared.capture_types.len()
                    || callback
                        .remaining_arity(owner.map(crate::task::RuntimeProgramOwner::types))
                        .map_err(
                            |error| crate::value::AwbcRuntimeValueSnapshotError::Message {
                                message: error.to_string(),
                            },
                        )?
                        != 0
                {
                    return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message: "active dialogue callback has a different capture ABI".to_owned(),
                    });
                }
                for (value, ty) in callback.retained().iter().zip(&declared.capture_types) {
                    crate::program_types::RuntimeProgramValueType::Awbc { program, ty: *ty }
                        .validate_snapshot_storage(
                            value,
                            &resources.work(),
                            V::admission_origin(),
                        )?;
                }
            }
            Ok(())
        })?;
        Ok(())
    }
}
impl super::ActiveDialogue {
    pub(super) fn admit_retained(
        &self,
        program: &crate::awbc::schema::AwbcProgram,
        activation: &DialogueActivationId,
        line: &crate::line_task::RuntimeDialogueActivationState<AwbcTypeId>,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::line_task::RuntimeDialogueRegistrySnapshotError> {
        let invalid = |message: String| {
            crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame { message }
        };
        if let super::ProductDialoguePhase::Closing(closing) = &self.phase {
            admission.validate_metadata(&closing.failure)?;
        }
        admission.validate_metadata(
            &self
                .pending_activation_host_call
                .as_ref()
                .map(|pending| (pending.call, &pending.id.0)),
        )?;
        self.admit_retained_values(
            program,
            admission,
            self.target.view(),
            self.effect_callbacks.iter().map(|(site, callback)| {
                (
                    *site,
                    crate::value::RuntimeValueView::Callable(
                        crate::value::RuntimeCallableView::Live(callback),
                    ),
                )
            }),
        )?;
        match &self.phase {
            super::ProductDialoguePhase::Transitioning => {
                return Err(invalid(
                    "cannot snapshot an in-flight dialogue phase transition".to_owned(),
                ));
            }
            super::ProductDialoguePhase::Activating { fiber, pending }
            | super::ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                state: super::ProductDialogueClosingState::Activation { fiber, pending },
                ..
            }) => {
                fiber.admit_retained(admission)?;
                match pending {
                    Some(
                        super::ProductPendingLineOperation::AcquireActor {
                            cursor,
                            destination,
                            command,
                            value,
                            token,
                        }
                        | super::ProductPendingLineOperation::ActorLook {
                            cursor,
                            destination,
                            command,
                            value,
                            token,
                        },
                    ) => {
                        admission.validate_metadata(&(cursor, destination, command, token))?;
                        admission.validate(value)?;
                    }
                    Some(super::ProductPendingLineOperation::StartVoice {
                        cursor,
                        destination,
                        command,
                        site,
                    }) => admission.validate_metadata(&(cursor, destination, command, site))?,
                    None => {}
                }
            }
            _ => {}
        }
        validate_product_dialogue_phase(
            program,
            activation,
            self,
            line.result(),
            LineTaskLiveState::is_closed,
        )?;
        if let Some(reducer) = self.line_task() {
            let group = program
                .content_units
                .get(self.content.index())
                .and_then(|content| content.line_task_group)
                .and_then(|group| program.line_task_groups.get(group.index()))
                .ok_or_else(|| {
                    invalid("active dialogue snapshot content has no line-task group".to_owned())
                })?;
            let view = AwbcLineTaskPlanView::new(program, group).ok_or_else(|| {
                invalid("active dialogue line task graph is malformed".to_owned())
            })?;
            reducer.admit_retained(&view, admission)?;
            line.restore_admit_reducer(&self.activation, &view, reducer)?;
        }
        Ok(())
    }
}
impl AwbcProductActiveDialogueSaveSnapshot {
    fn line_task(&self) -> Option<&AwbcProductLineTaskLiveSnapshot> {
        match &self.phase {
            super::AwbcProductDialoguePhase::Reducing { line_task }
            | super::AwbcProductDialoguePhase::Publishing { line_task }
            | super::AwbcProductDialoguePhase::Closing(super::AwbcProductDialogueClosing {
                state: super::AwbcProductDialogueClosingState::LineTask { line_task },
                ..
            }) => Some(line_task),
            _ => None,
        }
    }

    pub(super) fn admit_retained(
        &self,
        program: &crate::awbc::schema::AwbcProgram,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::line_task::RuntimeDialogueRegistrySnapshotError> {
        let invalid =
            |message: &str| crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                message: message.to_owned(),
            };
        if let AwbcProductDialoguePhaseSaveSnapshot::Closing(closing) = &self.phase {
            admission.validate_metadata(&closing.failure)?;
        }
        admission.validate_metadata(&self.pending_activation_host_call)?;
        self.admit_retained_values(
            program,
            admission,
            self.target.view(),
            self.effect_callbacks
                .iter()
                .map(|binding| (binding.site, binding.callback.view())),
        )?;
        match &self.phase {
            super::AwbcProductDialoguePhase::Transitioning => {
                return Err(invalid("dialogue transition cannot be restored"));
            }
            super::AwbcProductDialoguePhase::Activating { fiber, pending }
            | super::AwbcProductDialoguePhase::Closing(super::AwbcProductDialogueClosing {
                state: super::AwbcProductDialogueClosingState::Activation { fiber, pending },
                ..
            }) => {
                fiber.admit_retained(admission)?;
                match pending {
                    Some(
                        super::AwbcProductPendingLineOperation::AcquireActor {
                            cursor,
                            destination,
                            command,
                            value,
                            token,
                        }
                        | super::AwbcProductPendingLineOperation::ActorLook {
                            cursor,
                            destination,
                            command,
                            value,
                            token,
                        },
                    ) => {
                        admission.validate_metadata(&(cursor, destination, command, token))?;
                        admission.validate_snapshot(value)?;
                    }
                    Some(super::AwbcProductPendingLineOperation::StartVoice {
                        cursor,
                        destination,
                        command,
                        site,
                    }) => admission.validate_metadata(&(cursor, destination, command, site))?,
                    None => {}
                }
            }
            _ => {}
        }
        if let Some(reducer) = self.line_task() {
            let group = program
                .content_units
                .get(self.content.index())
                .and_then(|content| content.line_task_group)
                .and_then(|group| program.line_task_groups.get(group.index()))
                .ok_or_else(|| {
                    invalid("active dialogue snapshot content has no line-task group")
                })?;
            let view = AwbcLineTaskPlanView::new(program, group)
                .ok_or_else(|| invalid("active dialogue line task graph is malformed"))?;
            reducer.admit_retained(&view, admission)?;
        }
        Ok(())
    }

    fn visit_saved_value_roots<E>(
        &self,
        visit: &mut impl FnMut(&AwbcRuntimeValueSnapshot) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(&self.target)?;
        for value in self.captures.iter().chain(&self.task_inputs) {
            visit(value)?;
        }
        for value in &self.values {
            visit(&value.value)?;
        }
        for effect in &self.effect_callbacks {
            visit(&effect.callback)?;
        }
        let activation = match &self.phase {
            AwbcProductDialoguePhaseSaveSnapshot::Activating { fiber, pending }
            | AwbcProductDialoguePhaseSaveSnapshot::Closing(
                AwbcProductDialogueClosingSaveSnapshot {
                    state:
                        AwbcProductDialogueClosingStateSaveSnapshot::Activation { fiber, pending },
                    ..
                },
            ) => Some((fiber, pending)),
            AwbcProductDialoguePhaseSaveSnapshot::Transitioning
            | AwbcProductDialoguePhaseSaveSnapshot::Reducing { .. }
            | AwbcProductDialoguePhaseSaveSnapshot::Publishing { .. }
            | AwbcProductDialoguePhaseSaveSnapshot::Closing(
                AwbcProductDialogueClosingSaveSnapshot {
                    state: AwbcProductDialogueClosingStateSaveSnapshot::LineTask { .. },
                    ..
                },
            ) => None,
        };
        if let Some((fiber, pending)) = activation {
            fiber.visit_saved_value_roots(&mut *visit)?;
            match pending {
                Some(
                    AwbcProductPendingLineOperationSaveSnapshot::AcquireActor { value, .. }
                    | AwbcProductPendingLineOperationSaveSnapshot::ActorLook { value, .. },
                ) => visit(value)?,
                Some(AwbcProductPendingLineOperationSaveSnapshot::StartVoice { .. }) | None => {}
            }
        }
        Ok(())
    }
}

pub type AwbcProductDialogueValueSaveSnapshot =
    crate::plan::RuntimeDialogueValueBinding<AwbcRuntimeValueSnapshot>;

pub type AwbcProductDialoguePhaseSaveSnapshot = super::AwbcProductDialoguePhase<
    AwbcFiberStateSnapshot,
    AwbcProductLineTaskLiveSnapshot,
    AwbcProductPendingLineOperationSaveSnapshot,
>;
pub type AwbcProductDialogueClosingSaveSnapshot = super::AwbcProductDialogueClosing<
    AwbcFiberStateSnapshot,
    AwbcProductLineTaskLiveSnapshot,
    AwbcProductPendingLineOperationSaveSnapshot,
>;
pub type AwbcProductDialogueClosingStateSaveSnapshot = super::AwbcProductDialogueClosingState<
    AwbcFiberStateSnapshot,
    AwbcProductLineTaskLiveSnapshot,
    AwbcProductPendingLineOperationSaveSnapshot,
>;
pub type AwbcProductPendingLineOperationSaveSnapshot =
    super::AwbcProductPendingLineOperation<AwbcRuntimeValueSnapshot>;

impl Clone for AwbcProductPendingLineOperationSaveSnapshot {
    fn clone(&self) -> Self {
        match self {
            Self::AcquireActor {
                cursor,
                destination,
                command,
                value,
                token,
            } => Self::AcquireActor {
                cursor: *cursor,
                destination: *destination,
                command: command.clone(),
                value: value.clone(),
                token: token.clone(),
            },
            Self::ActorLook {
                cursor,
                destination,
                command,
                value,
                token,
            } => Self::ActorLook {
                cursor: *cursor,
                destination: *destination,
                command: command.clone(),
                value: value.clone(),
                token: token.clone(),
            },
            Self::StartVoice {
                cursor,
                destination,
                command,
                site,
            } => Self::StartVoice {
                cursor: *cursor,
                destination: *destination,
                command: command.clone(),
                site: *site,
            },
        }
    }
}

impl Clone for AwbcProductDialogueClosingStateSaveSnapshot {
    fn clone(&self) -> Self {
        match self {
            Self::Activation { fiber, pending } => Self::Activation {
                fiber: fiber.clone(),
                pending: pending.clone(),
            },
            Self::LineTask { line_task } => Self::LineTask {
                line_task: line_task.clone(),
            },
        }
    }
}

impl Clone for AwbcProductDialogueClosingSaveSnapshot {
    fn clone(&self) -> Self {
        Self {
            failure: self.failure.clone(),
            state: self.state.clone(),
        }
    }
}

impl Clone for AwbcProductDialoguePhaseSaveSnapshot {
    fn clone(&self) -> Self {
        match self {
            Self::Transitioning => Self::Transitioning,
            Self::Activating { fiber, pending } => Self::Activating {
                fiber: fiber.clone(),
                pending: pending.clone(),
            },
            Self::Reducing { line_task } => Self::Reducing {
                line_task: line_task.clone(),
            },
            Self::Publishing { line_task } => Self::Publishing {
                line_task: line_task.clone(),
            },
            Self::Closing(closing) => Self::Closing(closing.clone()),
        }
    }
}
pub(super) type AwbcProductDialogueSnapshotState =
    crate::line_task::RuntimeDialogueRegistrySaveSnapshot<
        AwbcProductActiveDialogueSaveSnapshot,
        AwbcTypeId,
    >;

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductChildFiberSaveSnapshot {
    pub owner: AwbcProductChildFiberOwnerSnapshot,
    pub fiber: AwbcFiberStateSnapshot,
    pub runtime_generation: crate::task::GenerationId,
    pub pending_host_call: Option<AwbcProductPendingHostCallSnapshot>,
}

fn snapshot_pending_line_operation_for_save(
    pending: &super::ProductPendingLineOperation,
    owner: &crate::task::RuntimeProgramOwner,
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
            value: crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                value, owner,
            )?,
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
            value: crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                value, owner,
            )?,
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
    owner: &crate::task::RuntimeProgramOwner,
) -> Result<AwbcProductDialoguePhaseSaveSnapshot, crate::value::AwbcRuntimeValueSnapshotError> {
    Ok(match phase {
        super::ProductDialoguePhase::Transitioning => {
            return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: "cannot snapshot an in-flight dialogue phase transition".to_owned(),
            });
        }
        super::ProductDialoguePhase::Activating { fiber, pending } => {
            AwbcProductDialoguePhaseSaveSnapshot::Activating {
                fiber: AwbcFiberStateSnapshot::from_admitted_live(fiber, owner)?,
                pending: pending
                    .as_ref()
                    .map(|pending| snapshot_pending_line_operation_for_save(pending, owner))
                    .transpose()?,
            }
        }
        super::ProductDialoguePhase::Reducing { line_task } => {
            AwbcProductDialoguePhaseSaveSnapshot::Reducing {
                line_task: line_task.snapshot(),
            }
        }
        super::ProductDialoguePhase::Publishing { line_task } => {
            AwbcProductDialoguePhaseSaveSnapshot::Publishing {
                line_task: line_task.snapshot(),
            }
        }
        super::ProductDialoguePhase::Closing(closing) => {
            AwbcProductDialoguePhaseSaveSnapshot::Closing(AwbcProductDialogueClosingSaveSnapshot {
                failure: closing.failure.clone(),
                state: match &closing.state {
                    super::ProductDialogueClosingState::Activation { fiber, pending } => {
                        AwbcProductDialogueClosingStateSaveSnapshot::Activation {
                            fiber: AwbcFiberStateSnapshot::from_admitted_live(fiber, owner)?,
                            pending: pending
                                .as_ref()
                                .map(|pending| {
                                    snapshot_pending_line_operation_for_save(pending, owner)
                                })
                                .transpose()?,
                        }
                    }
                    super::ProductDialogueClosingState::LineTask { line_task } => {
                        AwbcProductDialogueClosingStateSaveSnapshot::LineTask {
                            line_task: line_task.snapshot(),
                        }
                    }
                },
            })
        }
    })
}

fn restore_pending_line_operation_from_save(
    pending: AwbcProductPendingLineOperationSaveSnapshot,
    owner: crate::value::RuntimeValueRestoreContext<'_>,
) -> Result<super::ProductPendingLineOperation, crate::value::AwbcRuntimeValueSnapshotError> {
    Ok(match pending {
        AwbcProductPendingLineOperationSaveSnapshot::AcquireActor {
            cursor,
            destination,
            command,
            value,
            token,
        } => super::ProductPendingLineOperation::AcquireActor {
            cursor,
            destination,
            command,
            value: value.into_admitted_runtime_value(owner)?,
            token,
        },
        AwbcProductPendingLineOperationSaveSnapshot::ActorLook {
            cursor,
            destination,
            command,
            value,
            token,
        } => super::ProductPendingLineOperation::ActorLook {
            cursor,
            destination,
            command,
            value: value.into_admitted_runtime_value(owner)?,
            token,
        },
        AwbcProductPendingLineOperationSaveSnapshot::StartVoice {
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
    })
}

fn restore_dialogue_phase_from_save(
    phase: AwbcProductDialoguePhaseSaveSnapshot,
    owner: crate::value::RuntimeValueRestoreContext<'_>,
) -> Result<super::ProductDialoguePhase, String> {
    Ok(match phase {
        AwbcProductDialoguePhaseSaveSnapshot::Transitioning => {
            return Err("dialogue transition cannot be restored".into());
        }
        AwbcProductDialoguePhaseSaveSnapshot::Activating { fiber, pending } => {
            super::ProductDialoguePhase::Activating {
                fiber: fiber
                    .into_admitted_live_for_program(owner)
                    .map_err(|error| error.to_string())?,
                pending: pending
                    .map(|pending| restore_pending_line_operation_from_save(pending, owner))
                    .transpose()
                    .map_err(|error| error.to_string())?,
            }
        }
        AwbcProductDialoguePhaseSaveSnapshot::Reducing { line_task } => {
            super::ProductDialoguePhase::Reducing {
                line_task: LineTaskLiveState::from_admitted_snapshot(line_task),
            }
        }
        AwbcProductDialoguePhaseSaveSnapshot::Publishing { line_task } => {
            super::ProductDialoguePhase::Publishing {
                line_task: LineTaskLiveState::from_admitted_snapshot(line_task),
            }
        }
        AwbcProductDialoguePhaseSaveSnapshot::Closing(closing) => {
            super::ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                failure: closing.failure,
                state: match closing.state {
                    AwbcProductDialogueClosingStateSaveSnapshot::Activation { fiber, pending } => {
                        super::ProductDialogueClosingState::Activation {
                            fiber: fiber
                                .into_admitted_live_for_program(owner)
                                .map_err(|error| error.to_string())?,
                            pending: pending
                                .map(|pending| {
                                    restore_pending_line_operation_from_save(pending, owner)
                                })
                                .transpose()
                                .map_err(|error| error.to_string())?,
                        }
                    }
                    AwbcProductDialogueClosingStateSaveSnapshot::LineTask { line_task } => {
                        super::ProductDialogueClosingState::LineTask {
                            line_task: LineTaskLiveState::from_admitted_snapshot(line_task),
                        }
                    }
                },
            })
        }
    })
}

fn snapshot_active_dialogue(
    active: &super::ActiveDialogue,
    owner: &crate::task::RuntimeProgramOwner,
) -> Result<
    AwbcProductActiveDialogueSaveSnapshot,
    crate::line_task::RuntimeDialogueRegistrySnapshotError,
> {
    Ok(AwbcProductActiveDialogueSaveSnapshot {
        activation: active.activation.clone(),
        content: active.content,
        target: crate::value::AwbcRuntimeValueSnapshot::from_admitted_opaque(
            &active.target,
            owner,
        )?,
        target_type: active.target_type,
        line: active.line.clone(),
        result: active.result.clone(),
        captures: active
            .captures
            .iter()
            .map(|value| {
                crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)
            })
            .collect::<Result<_, _>>()?,
        task_inputs: active
            .task_inputs
            .iter()
            .map(|value| {
                crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)
            })
            .collect::<Result<_, _>>()?,
        values: active
            .values
            .iter()
            .map(|binding| {
                Ok(AwbcProductDialogueValueSaveSnapshot {
                    slot: binding.slot,
                    role: binding.role,
                    value: crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                        &binding.value,
                        owner,
                    )?,
                })
            })
            .collect::<Result<_, crate::value::AwbcRuntimeValueSnapshotError>>()?,
        effect_callbacks: active
            .effect_callbacks
            .iter()
            .map(|(site, callback)| {
                Ok(AwbcProductDialogueEffectSaveSnapshot {
                    site: *site,
                    callback: crate::value::AwbcRuntimeValueSnapshot::from_admitted_callable(
                        callback, owner,
                    )?,
                })
            })
            .collect::<Result<_, crate::value::AwbcRuntimeValueSnapshotError>>()?,
        voice: active.voice.clone(),
        phase: snapshot_dialogue_phase_for_save(&active.phase, owner)?,
        elapsed_nanos: active.elapsed_nanos,
        pending_content_events: active.pending_content_events.clone(),
        pending_advance: active.pending_advance,
        pending_line_outcomes: active.pending_line_outcomes.clone(),
        pending_activation_host_call: active.pending_activation_host_call.as_ref().map(|pending| {
            AwbcProductPendingHostCallSnapshot {
                call: pending.call,
                id: pending.id.0.clone(),
            }
        }),
    })
}

impl AwbcProductExecutorSnapshot {
    /// Uses the same lifecycle rule before any live executor is constructed.
    #[must_use]
    pub fn requires_runtime_generation_pin(&self) -> bool {
        self.root.as_ref().is_some_and(|root| !root.is_terminal())
            || !self.fiber.status.is_terminal()
            || self
                .child_fibers
                .iter()
                .any(|child| !child.fiber.status.is_terminal())
    }

    pub fn active_entry_snapshot_identity(
        &self,
        program: &crate::awbc::schema::AwbcProgram,
    ) -> Result<crate::entry::ActiveEntrySnapshotV1, crate::root::RootRuntimeError> {
        self.fiber.root.active_entry_snapshot_identity(program)
    }

    /// Inspects inert values directly, retaining storage provenance and the
    /// original Source leases without projecting a wire image or live owner.
    pub fn visit_borrowed_runtime_values<E>(
        &self,
        mut visit: impl for<'value> FnMut(crate::value::RuntimeBorrowedValue<'value>) -> Result<(), E>,
    ) -> Result<(), crate::value::RuntimeBorrowedValueVisitError<E>> {
        let mut root = |value: &AwbcRuntimeValueSnapshot| {
            crate::value::RuntimeBorrowedValue::from(value).try_visit(
                crate::entry::RuntimeSchemaLimits::engine_default(),
                &mut visit,
            )
        };
        if let Some(owner) = &self.root {
            owner.visit_stored_value_roots(&mut root)?;
        }
        self.fiber.visit_saved_value_roots(&mut root)?;
        for child in &self.child_fibers {
            child.fiber.visit_saved_value_roots(&mut root)?;
        }
        self.dialogues.try_visit_active(|_, frame, line| {
            frame.visit_saved_value_roots(&mut root)?;
            line.visit_saved_value_roots(&mut root)
        })?;
        self.need_producers.visit_saved_value_roots(&mut root)?;
        for event in &self.queued_task_events {
            if let AwbcProductTaskEventKindSaveSnapshot::Ready(value) = &event.kind {
                root(value)?;
            }
        }
        self.observations.visit_saved_value_roots(&mut root)
    }

    /// Admits the complete original-program image before copying or activating any value.
    pub fn admit_retained(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcProductStepBuildError> {
        let crate::task::RuntimeProgramOwner::Awbc(program) = owner else {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "Product snapshot requires its original AWBC program".to_owned(),
            });
        };
        admission
            .admit_composite(
                crate::entry::schema::value_encoding::ValueAdmissionOrigin::InertSnapshot,
                |selected, _| {
                    if selected.is_some_and(|selected| selected.same_program(owner)) {
                        Ok(())
                    } else {
                        Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                            message: "Product admission selected a different executable lease"
                                .to_owned(),
                        })
                    }
                },
            )
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
        let snapshot = self;
        admission
            .validate_metadata(&(
                (
                    self.runtime_generation,
                    &self.dialogue_effect_callback_activations,
                    &self.live_flow_bindings,
                    self.entry_bound,
                    &self.active_choice,
                    &self.pending_host_call,
                    &self.started_tasks,
                    &self.task_publications,
                    &self.need_publications,
                    &self.emitted_content,
                    &self.stream_sequences,
                    self.next_generation,
                    self.next_fiber_instance,
                    &self.dialogue_occurrences,
                    self.next_host_call_sequence,
                    self.next_audio_sequence,
                ),
                &self.compact_pure_stats,
            ))
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
        admission
            .validate_metadata_collection(snapshot.child_fibers.len())
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
        snapshot
            .fiber
            .root
            .admit_executor_roots(
                snapshot.root.as_ref(),
                program,
                snapshot.child_fibers.iter().map(|child| {
                    (
                        child.fiber.root,
                        child.fiber.status,
                        matches!(child.owner, AwbcProductChildFiberOwnerSnapshot::Independent),
                        matches!(
                            child
                                .fiber
                                .suspension
                                .as_ref()
                                .map(|suspension| &suspension.reason),
                            Some(crate::awbc::fiber::FiberSuspensionReason::BudgetYield)
                        ),
                    )
                }),
            )
            .map_err(|error| AwbcProductStepBuildError::RootSnapshot {
                message: error.to_string(),
            })?;
        if let Some(root) = &snapshot.root {
            root.admit_retained(admission).map_err(|error| {
                AwbcProductStepBuildError::RootSnapshot {
                    message: error.to_string(),
                }
            })?;
        }
        if snapshot.fiber.instance.get() != std::num::NonZeroU64::MIN {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "Product main fiber does not own the initial fiber identity".to_owned(),
            });
        }
        let mut fiber_instances = BTreeSet::new();
        for fiber in std::iter::once(&snapshot.fiber)
            .chain(snapshot.child_fibers.iter().map(|child| &child.fiber))
        {
            if snapshot
                .next_fiber_instance
                .last_issued()
                .is_none_or(|last| fiber.instance.get() > last)
                || !fiber_instances.insert(fiber.instance)
            {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "Product fiber instance is duplicated or was not allocated".to_owned(),
                });
            }
        }
        let has_invalid_await_many_identity = |fiber: &AwbcFiberStateSnapshot| {
            fiber.suspension.as_ref().is_some_and(|suspension| {
                matches!(
                    &suspension.reason,
                    crate::awbc::fiber::FiberSuspensionReason::AwaitMany(state)
                        if state.base.as_ref().is_some_and(|base| {
                            base.correlation().generation > snapshot.runtime_generation
                        })
                )
            })
        };
        if snapshot.fiber.generation > snapshot.runtime_generation.get()
            || snapshot
                .child_fibers
                .iter()
                .any(|child| child.runtime_generation > snapshot.runtime_generation)
            || has_invalid_await_many_identity(&snapshot.fiber)
            || snapshot
                .child_fibers
                .iter()
                .any(|child| has_invalid_await_many_identity(&child.fiber))
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message:
                    "Product snapshot generation pin or AwaitMany occurrence identity is invalid"
                        .to_owned(),
            });
        }
        validate_task_publications(snapshot, admission)?;
        if let Err(error) = snapshot.need_producers.admit_retained(admission) {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: format!("invalid Need producer registry snapshot: {error}"),
            });
        }
        snapshot
            .observations
            .admit_retained(admission)
            .map_err(|message| AwbcProductStepBuildError::RestoreSnapshot { message })?;
        validate_need_publication_cursors(snapshot, &snapshot.need_producers)?;

        snapshot.fiber.admit_retained(admission).map_err(|error| {
            AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            }
        })?;
        for child in &snapshot.child_fibers {
            admission
                .validate_metadata(&(
                    &child.owner,
                    child.runtime_generation,
                    &child.pending_host_call,
                ))
                .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                })?;
            child.fiber.admit_retained(admission).map_err(|error| {
                AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                }
            })?;
            Self::validate_child_owner(program, &child.owner)?;
            let host_call =
                child
                    .fiber
                    .suspension
                    .as_ref()
                    .and_then(|suspension| match &suspension.reason {
                        crate::awbc::fiber::FiberSuspensionReason::HostCall { call, .. } => {
                            Some(*call)
                        }
                        _ => None,
                    });
            match (&child.owner, &child.pending_host_call, host_call) {
                (
                    AwbcProductChildFiberOwnerSnapshot::Deferred { .. }
                    | AwbcProductChildFiberOwnerSnapshot::ScopedDeferred { .. },
                    Some(pending),
                    Some(call),
                ) if pending.call == call && program.host_calls.get(call.index()).is_some() => {}
                (
                    AwbcProductChildFiberOwnerSnapshot::Deferred { .. }
                    | AwbcProductChildFiberOwnerSnapshot::ScopedDeferred { .. },
                    None,
                    None,
                ) => {}
                (
                    AwbcProductChildFiberOwnerSnapshot::Deferred { .. }
                    | AwbcProductChildFiberOwnerSnapshot::ScopedDeferred { .. },
                    _,
                    _,
                ) => {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message:
                            "deferred child host-call suspension and pending request are not paired"
                                .to_owned(),
                    });
                }
                (_, None, _) => {}
                _ => {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "non-deferred child carries a pending host call".to_owned(),
                    });
                }
            }
        }
        self.validate_live_flow_bindings(program)?;
        let deferred_children = expected_deferred_children(snapshot)?;
        snapshot
            .dialogues
            .admit_retained(admission, &deferred_children)
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
        let mut active_dialogue = None;
        snapshot
            .dialogues
            .try_visit_active(|activation, frame, line| {
                frame
                    .admit_retained(program, admission)
                    .map_err(|error| error.to_string())?;
                if active_dialogue.replace((activation, frame, line)).is_some() {
                    return Err("Product snapshot repeats an active dialogue".to_owned());
                }
                Ok(())
            })
            .map_err(|message| AwbcProductStepBuildError::RestoreSnapshot { message })?;
        if let Some((activation, active, shared_line)) = active_dialogue {
            let reducer = active.line_task();
            let content = program
                .content_units
                .get(active.content.index())
                .ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                    message: "active dialogue snapshot references missing content".to_owned(),
                })?;
            let group = content
                .line_task_group
                .and_then(|group| program.line_task_groups.get(group.index()));
            let expected_captures = group.map_or(0, |group| group.captures.len());
            if active.captures.len() != expected_captures {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "active dialogue snapshot capture arity disagrees with its content group"
                            .to_owned(),
                });
            }
            let expected_task_inputs = if active.line_task().is_some() {
                group.map_or(0, |group| {
                    group.captures.len() + group.activation_exports.len()
                })
            } else {
                0
            };
            if active.task_inputs.len() != expected_task_inputs
                || active.line_task().is_some()
                    && active
                        .task_inputs
                        .get(..expected_captures)
                        .is_none_or(|prefix| {
                            prefix.iter().zip(&active.captures).any(|(left, right)| {
                                !crate::value::runtime_value_views_equal(left.view(), right.view())
                            })
                        })
            {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "active dialogue snapshot line-task input ABI is invalid".to_owned(),
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
            validate_product_dialogue_phase(
                program,
                activation,
                active,
                shared_line.result(),
                |saved| {
                    matches!(
                        saved.phase(),
                        AwbcProductLineTaskPhaseSnapshot::Closed { .. }
                    )
                },
            )
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
            if let Some(reducer) = reducer {
                let group = program
                    .content_units
                    .get(active.content.index())
                    .and_then(|content| content.line_task_group)
                    .and_then(|group| program.line_task_groups.get(group.index()))
                    .ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                        message: "active dialogue is missing its line task graph".to_owned(),
                    })?;
                let view = AwbcLineTaskPlanView::new(program, group).ok_or_else(|| {
                    AwbcProductStepBuildError::RestoreSnapshot {
                        message: "active dialogue line task graph is malformed".to_owned(),
                    }
                })?;
                shared_line
                    .restore_admit_reducer(&active.activation, &view, reducer)
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
                super::AwbcProductDialoguePhase::Activating { pending, .. }
                | super::AwbcProductDialoguePhase::Closing(super::AwbcProductDialogueClosing {
                    state: super::AwbcProductDialogueClosingState::Activation { pending, .. },
                    ..
                }) => pending.as_ref(),
                super::AwbcProductDialoguePhase::Reducing { .. }
                | super::AwbcProductDialoguePhase::Publishing { .. }
                | super::AwbcProductDialoguePhase::Closing(super::AwbcProductDialogueClosing {
                    state: super::AwbcProductDialogueClosingState::LineTask { .. },
                    ..
                }) => None,
                super::AwbcProductDialoguePhase::Transitioning => {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "cannot validate a transient dialogue transition".to_owned(),
                    });
                }
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
                        super::AwbcProductPendingLineOperation::AcquireActor { .. },
                        crate::presentation::RuntimeLineHostCommand::Stage(
                            crate::presentation::RuntimeStageCommand::AcquireActor { .. }
                        )
                    ) | (
                        super::AwbcProductPendingLineOperation::ActorLook { .. },
                        crate::presentation::RuntimeLineHostCommand::Stage(
                            crate::presentation::RuntimeStageCommand::SetCharacterLook { .. }
                        )
                    ) | (
                        super::AwbcProductPendingLineOperation::StartVoice { .. },
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
            let activation_host_suspension = activation_fiber_in_phase(active)
                .and_then(|fiber| fiber.suspension.as_ref())
                .filter(|_| {
                    activation_fiber_in_phase(active)
                        .is_some_and(|fiber| fiber.status == FiberStatus::Suspended)
                });
            match (
                active.pending_activation_host_call.as_ref(),
                activation_host_suspension,
            ) {
                (Some(pending), Some(suspension))
                    if !pending.id.is_empty()
                        && program.host_calls.get(pending.call.index()).is_some()
                        && matches!(
                            &suspension.reason,
                            crate::awbc::fiber::FiberSuspensionReason::HostCall {
                                call,
                                ..
                            } if *call == pending.call
                        )
                        && matches!(
                            suspension.resume,
                            crate::awbc::fiber::FiberResumeTarget::Declared(_)
                        ) => {}
                (None, Some(suspension))
                    if matches!(
                        &suspension.reason,
                        crate::awbc::fiber::FiberSuspensionReason::DurationWait(_)
                            | crate::awbc::fiber::FiberSuspensionReason::BudgetYield
                    ) => {}
                (None, Some(suspension))
                    if matches!(
                        &suspension.reason,
                        crate::awbc::fiber::FiberSuspensionReason::HostCall { .. }
                    ) =>
                {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "activation host-call suspension is missing its pending request"
                            .to_owned(),
                    });
                }
                (Some(_), _) | (None, Some(_)) => {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message:
                            "activation host-call request is not paired with its exact suspended fiber"
                                .to_owned(),
                    });
                }
                (None, None) => {}
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
            let expected_joined = reducer
                .map(LineTaskLiveSnapshot::outstanding_tags)
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
                let tokens = child
                    .fiber
                    .affine_line_handle_tokens()
                    .map_err(|message| AwbcProductStepBuildError::RestoreSnapshot { message })?
                    .into_iter()
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
            let mut actual_deferred_pair = None;
            for child in &snapshot.child_fibers {
                let AwbcProductChildFiberOwnerSnapshot::Deferred {
                    content,
                    activation,
                    registration,
                    site,
                } = &child.owner
                else {
                    continue;
                };
                let function = program
                    .defer_sites
                    .get(site.index())
                    .copied()
                    .ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                        message: "deferred child references a missing defer site".to_owned(),
                    })?;
                if content != &active.content
                    || activation != &active.activation
                    || child.fiber.frames.first().map(|frame| frame.function) != Some(function)
                    || actual_deferred_pair
                        .replace((*registration, *site))
                        .is_some()
                {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message:
                            "deferred child owner does not match one active dialogue registration"
                                .to_owned(),
                    });
                }
                let tag = LineTaskWorkTag::activation(
                    active.activation.clone(),
                    LineTaskWork::Defer(*registration),
                );
                let tokens = child
                    .fiber
                    .affine_line_handle_tokens()
                    .map_err(|message| AwbcProductStepBuildError::RestoreSnapshot { message })?
                    .into_iter()
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
                if actual_child_tokens.insert(tag, tokens).is_some() {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "deferred child duplicates another line child work tag".to_owned(),
                    });
                }
            }
            let activation_fiber = activation_fiber_in_phase(active);
            let mut actual_scoped_inflight = None;
            for child in &snapshot.child_fibers {
                let AwbcProductChildFiberOwnerSnapshot::ScopedDeferred {
                    content,
                    activation,
                    frame,
                    scope,
                    registration,
                    site,
                } = &child.owner
                else {
                    continue;
                };
                let function = program
                    .defer_sites
                    .get(site.index())
                    .copied()
                    .ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                        message: "scoped deferred child references a missing defer site".to_owned(),
                    })?;
                let expected = (*frame, *scope, *registration, *site);
                let matching_scope = activation_fiber
                    .and_then(|fiber| fiber.frames.iter().find(|row| row.instance == *frame))
                    .and_then(|frame| frame.scopes.iter().find(|row| row.id == *scope));
                if *content != active.content
                    || activation != &active.activation
                    || child.fiber.frames.first().map(|frame| frame.function) != Some(function)
                    || child.fiber.status == FiberStatus::Returned
                    || child.fiber.status == FiberStatus::Cancelled
                    || child.fiber.status == FiberStatus::Trapped
                    || matching_scope.is_none_or(|scope| {
                        scope.defer_exit.is_none()
                            || scope.defer_inflight
                                != Some(crate::awbc::fiber::FiberDeferredInFlight {
                                    registration: *registration,
                                    site: *site,
                                })
                    })
                    || actual_scoped_inflight.replace(expected).is_some()
                {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "scoped defer child does not match one active activation scope"
                            .to_owned(),
                    });
                }
                let tag = LineTaskWorkTag::activation(
                    active.activation.clone(),
                    LineTaskWork::Defer(*registration),
                );
                let tokens = child
                    .fiber
                    .affine_line_handle_tokens()
                    .map_err(|message| AwbcProductStepBuildError::RestoreSnapshot { message })?
                    .into_iter()
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
                if actual_child_tokens.insert(tag, tokens).is_some() {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "scoped defer duplicates another child work tag".to_owned(),
                    });
                }
            }
            if actual_deferred_pair != shared_line.deferred_inflight() {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "deferred child snapshot is not paired with the activation inflight registration"
                        .to_owned(),
                });
            }
            let mut expected_scoped_inflight = None;
            let mut scope_defer_tokens = BTreeMap::new();
            if let Some(fiber) = activation_fiber {
                for frame in &fiber.frames {
                    for scope in &frame.scopes {
                        if let Some(inflight) = scope.defer_inflight {
                            if expected_scoped_inflight
                                .replace((
                                    frame.instance,
                                    scope.id,
                                    inflight.registration,
                                    inflight.site,
                                ))
                                .is_some()
                            {
                                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                                    message: "activation has more than one in-flight scoped defer"
                                        .to_owned(),
                                });
                            }
                        }
                        for deferred in &scope.defers {
                            for capture in &deferred.captures {
                                for handle in
                                    capture.view().affine_line_handles().map_err(|error| {
                                        AwbcProductStepBuildError::RestoreSnapshot {
                                            message: error.to_string(),
                                        }
                                    })?
                                {
                                    if handle.token().activation() != &active.activation
                                        || scope_defer_tokens
                                            .insert(handle.token().clone(), deferred.id)
                                            .is_some()
                                    {
                                        return Err(AwbcProductStepBuildError::RestoreSnapshot {
                                            message: "scoped defer capture ownership is duplicated or stale"
                                                .to_owned(),
                                        });
                                    }
                                }
                            }
                        }
                        for release in &scope.defer_releasing {
                            for token in &release.tokens {
                                if token.activation() != &active.activation
                                    || scope_defer_tokens
                                        .insert(token.clone(), release.registration)
                                        .is_some()
                                {
                                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                                        message:
                                            "scoped defer release token is duplicated or stale"
                                                .to_owned(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            let ledger_scoped_tokens = shared_line
                .ledger()
                .leases()
                .iter()
                .filter_map(|(token, lease)| match lease.owner() {
                    crate::line_task::RuntimeHandleOwnerSlot::ScopedDefer(registration) => {
                        Some((token.clone(), *registration))
                    }
                    _ => None,
                })
                .collect::<BTreeMap<_, _>>();
            if ledger_scoped_tokens != scope_defer_tokens {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "ScopedDefer ledger custody is not isomorphic to scope packets and pending releases"
                            .to_owned(),
                });
            }
            if actual_scoped_inflight != expected_scoped_inflight {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "scoped defer child snapshot is not paired with the exact activation scope"
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
            if let Some((registration, _)) = actual_deferred_pair {
                ledger_child_tokens
                    .entry(LineTaskWorkTag::activation(
                        active.activation.clone(),
                        LineTaskWork::Defer(registration),
                    ))
                    .or_default();
            }
            if actual_child_tokens != ledger_child_tokens {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message:
                        "child fiber affine graph is not isomorphic to ChildScope ledger ownership"
                            .to_owned(),
                });
            }
        } else if !deferred_children.is_empty() {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "deferred child snapshot has no active dialogue registration owner"
                    .to_owned(),
            });
        }
        if let Some(active) = &snapshot.active_choice {
            Self::validate_active_choice(program, active)?;
        }
        if let Some(pending) = &snapshot.pending_host_call
            && program.host_calls.get(pending.call.index()).is_none()
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "pending host-call snapshot references missing AWBC host call".to_owned(),
            });
        }
        if snapshot
            .emitted_content
            .iter()
            .any(|content| program.content_units.get(content.index()).is_none())
            || snapshot
                .stream_sequences
                .keys()
                .any(|stream| program.stream_plans.get(stream.index()).is_none())
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "executor snapshot references missing AWBC content or stream table"
                    .to_owned(),
            });
        }
        Ok(())
    }

    fn validate_child_owner(
        program: &crate::awbc::schema::AwbcProgram,
        owner: &AwbcProductChildFiberOwnerSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        if let AwbcProductChildFiberOwnerSnapshot::Deferred { site, .. }
        | AwbcProductChildFiberOwnerSnapshot::ScopedDeferred { site, .. } = owner
        {
            let function_id = program
                .defer_sites
                .get(site.index())
                .copied()
                .ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                    message: "deferred child snapshot references a missing defer site".to_owned(),
                })?;
            let function = program.functions.get(function_id.index()).ok_or_else(|| {
                AwbcProductStepBuildError::RestoreSnapshot {
                    message: "deferred child snapshot references a missing function".to_owned(),
                }
            })?;
            let signature = program
                .signatures
                .get(function.signature.index())
                .ok_or_else(|| AwbcProductStepBuildError::RestoreSnapshot {
                    message: "deferred child snapshot references a missing signature".to_owned(),
                })?;
            if function.kind != crate::awbc::schema::AwbcFunctionKind::Ordinary
                || !signature.result.is_some_and(|result| {
                    matches!(
                        program
                            .runtime_types
                            .get(result.index())
                            .map(crate::awbc::schema::AwbcRuntimeType::shape),
                        Some(crate::awbc::schema::AwbcRuntimeTypeShape::Unit)
                    )
                })
            {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "deferred child snapshot no longer matches its verified Unit function"
                        .to_owned(),
                });
            }
            return Ok(());
        }
        let AwbcProductChildFiberOwnerSnapshot::LineTask {
            content,
            tag,
            policy,
            ..
        } = owner
        else {
            return Ok(());
        };
        let Some(group) = program
            .content_units
            .get(content.index())
            .and_then(|content| content.line_task_group)
            .and_then(|group| program.line_task_groups.get(group.index()))
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
        match &tag.work {
            AwbcProductLineTaskWorkSnapshot::Node(node) => {
                let Some(global) = group.nodes.start.checked_add(*node) else {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "line-owned child fiber node index overflows its group".to_owned(),
                    });
                };
                if *node >= group.nodes.len
                    || !matches!(
                        program.line_task_nodes.get(global as usize),
                        Some(crate::awbc::schema::AwbcLineTaskNode::Action(_))
                    )
                {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "line-owned child fiber references a non-action node outside its content group"
                            .to_owned(),
                    });
                }
            }
            AwbcProductLineTaskWorkSnapshot::Cancellation(action) => {
                if !group
                    .cancel_handlers
                    .iter()
                    .any(|handler| &handler.trigger == action)
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
            AwbcProductLineTaskWorkSnapshot::Defer(_) => {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "defer child work requires an activation-owned inflight registration"
                        .to_owned(),
                });
            }
        }
        Ok(())
    }

    fn validate_active_choice(
        program: &crate::awbc::schema::AwbcProgram,
        active: &AwbcProductActiveChoiceSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        let Some(choice) = program.choices.get(active.choice.index()) else {
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
            if program.choice_options.get(*source_index).is_none() {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "active choice snapshot references a missing source option".to_owned(),
                });
            }
            previous = Some(*source_index);
        }
        Ok(())
    }

    fn validate_live_flow_bindings(
        &self,
        program: &crate::awbc::schema::AwbcProgram,
    ) -> Result<(), AwbcProductStepBuildError> {
        let snapshot = self;
        let mut flows = BTreeSet::new();
        let mut functions = BTreeSet::new();
        for binding in &snapshot.live_flow_bindings {
            if !flows.insert(binding.flow.clone()) || !functions.insert(binding.function) {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "snapshot repeats a live semantic Flow binding".to_owned(),
                });
            }
            if program.flow_function(&binding.flow) != Some(binding.function) {
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
            let is_flow = program
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
                program
                    .choice_options
                    .get(*source_index)
                    .and_then(|option| option.target)
                    .map(|target| {
                        program
                            .flow_identity(target)
                            .expect("verified choice target has an exact Flow identity")
                    })
            }) {
                if !flows.contains(target) {
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
}

impl AwbcProductExecutorSaveSnapshot {
    /// Inspects every retained inert value before a caller constructs the
    /// executor, using the owning fiber/line/task/observation grammars.
    pub fn visit_borrowed_runtime_values<E>(
        &self,
        mut visit: impl for<'value> FnMut(crate::value::RuntimeBorrowedValue<'value>) -> Result<(), E>,
    ) -> Result<(), crate::value::RuntimeBorrowedValueVisitError<E>> {
        self.visit_saved_value_roots(&mut |value| {
            crate::value::RuntimeBorrowedValue::from(value).try_visit(
                crate::entry::RuntimeSchemaLimits::engine_default(),
                &mut visit,
            )
        })
    }

    fn visit_saved_value_roots<E>(
        &self,
        visit: &mut impl FnMut(&AwbcRuntimeValueSnapshot) -> Result<(), E>,
    ) -> Result<(), E> {
        if let Some(value) = self.root.as_ref().and_then(|root| root.phase.state_value()) {
            visit(value)?;
        }
        self.fiber.visit_saved_value_roots(&mut *visit)?;
        for child in &self.child_fibers {
            child.fiber.visit_saved_value_roots(&mut *visit)?;
        }
        self.dialogues.try_visit_active(|_, frame, line| {
            frame.visit_saved_value_roots(&mut *visit)?;
            line.visit_saved_value_roots(&mut *visit)
        })?;
        for task in &self.need_producers.task_admissions {
            for value in task.request_values() {
                visit(value)?;
            }
        }
        for launch in &self.need_producers.launches {
            if let AwbcNeedProducerLaunchOriginSaveSnapshot::Need {
                arguments,
                task_spec,
                ..
            } = &launch.origin
            {
                for argument in arguments {
                    visit(&argument.value)?;
                }
                for value in task_spec.request.payloads() {
                    visit(value)?;
                }
            }
            if let AwbcNeedStateSaveSnapshot::Ready(value) = &launch.state {
                visit(value)?;
            }
        }
        for event in &self.queued_task_events {
            if let AwbcProductTaskEventKindSaveSnapshot::Ready(value) = &event.kind {
                visit(value)?;
            }
        }
        self.observations.visit_saved_value_roots(visit)
    }

    pub fn from_live(
        snapshot: &AwbcProductExecutorSnapshot,
        program: &super::AwbcProductProgram,
    ) -> Result<Self, AwbcProductSaveError> {
        let owner = &crate::task::RuntimeProgramOwner::Awbc(program.program_arc());
        let mut admission =
            program.retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        snapshot
            .admit_retained(owner, &mut admission)
            .map_err(|error| AwbcProductSaveError::InvalidSnapshot {
                message: error.to_string(),
            })?;
        let needs = snapshot
            .need_producers
            .quiescence_blocking_needs()
            .collect::<Vec<_>>();
        if !needs.is_empty() {
            return Err(AwbcProductSaveError::NeedsQuiescence { needs });
        }
        Self::from_admitted(snapshot)
    }

    pub(crate) fn from_admitted(
        snapshot: &AwbcProductExecutorSnapshot,
    ) -> Result<Self, AwbcProductSaveError> {
        let root = snapshot
            .root
            .as_ref()
            .map(crate::root::RootStateSaveSnapshotV1::from_runtime_image)
            .transpose()
            .map_err(|error| AwbcProductSaveError::InvalidSnapshot {
                message: error.to_string(),
            })?;
        let dialogues = snapshot.dialogues.clone();
        Ok(Self {
            root,
            runtime_generation: snapshot.runtime_generation,
            fiber: snapshot.fiber.clone(),
            child_fibers: snapshot
                .child_fibers
                .iter()
                .map(|child| AwbcProductChildFiberSaveSnapshot {
                    owner: child.owner.clone(),
                    fiber: child.fiber.clone(),
                    runtime_generation: child.runtime_generation,
                    pending_host_call: child.pending_host_call.as_ref().map(|pending| {
                        AwbcProductPendingHostCallSnapshot {
                            call: pending.call,
                            id: pending.id.clone(),
                        }
                    }),
                })
                .collect(),
            dialogue_effect_callback_activations: snapshot
                .dialogue_effect_callback_activations
                .clone(),
            live_flow_bindings: snapshot.live_flow_bindings.clone(),
            entry_bound: snapshot.entry_bound,
            dialogues,
            active_choice: snapshot.active_choice.clone(),
            pending_host_call: snapshot.pending_host_call.clone(),
            started_tasks: snapshot.started_tasks.clone(),
            task_publications: snapshot.task_publications.clone(),
            need_publications: snapshot
                .need_publications
                .iter()
                .map(
                    |((waiter, correlation), cursor)| AwbcNeedPublicationSaveSnapshot {
                        waiter: *waiter,
                        correlation: *correlation,
                        cursor: *cursor,
                    },
                )
                .collect(),
            need_producers: save_need_producer_registry(&snapshot.need_producers)
                .map_err(|message| AwbcProductSaveError::InvalidSnapshot { message })?,
            queued_task_events: snapshot.queued_task_events.clone(),
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

    pub fn into_live_for_program(
        self,
        program: &super::AwbcProductProgram,
    ) -> Result<AwbcProductExecutorSnapshot, AwbcProductStepBuildError> {
        let owner = crate::task::RuntimeProgramOwner::Awbc(program.program_arc());
        let root = self
            .root
            .map(|root| {
                let active = self
                    .fiber
                    .root
                    .active_entry_snapshot_identity(program)
                    .map_err(|error| AwbcProductStepBuildError::RootSnapshot {
                        message: error.to_string(),
                    })?;
                root.into_runtime_image(&active, &owner).map_err(|error| {
                    AwbcProductStepBuildError::RootSnapshot {
                        message: error.to_string(),
                    }
                })
            })
            .transpose()?;
        let mut admission =
            program.retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        admission
            .validate_metadata_collection(self.queued_task_events.len())
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
        let need_publication_count = self.need_publications.len();
        let need_publications = self
            .need_publications
            .into_iter()
            .map(|row| ((row.waiter, row.correlation), row.cursor))
            .collect::<BTreeMap<_, _>>();
        if need_publications.len() != need_publication_count {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "Need publication snapshot contains duplicate waiter/Need rows".to_owned(),
            });
        }
        let need_producers = restore_need_producer_registry(
            self.need_producers,
            program.task_source.as_ref(),
            &mut admission,
        )
        .map_err(|message| AwbcProductStepBuildError::RestoreSnapshot { message })?;
        need_producers
            .admit_retained(&mut admission)
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
        for event in &self.queued_task_events {
            event
                .admit_retained(need_producers.task_spec(event.correlation), &mut admission)
                .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                })?;
        }
        let queued_task_events = self.queued_task_events;
        Ok(AwbcProductExecutorSnapshot {
            root,
            runtime_generation: self.runtime_generation,
            fiber: self.fiber,
            child_fibers: self
                .child_fibers
                .into_iter()
                .map(|child| AwbcProductChildFiberSnapshot {
                    owner: child.owner,
                    fiber: child.fiber,
                    runtime_generation: child.runtime_generation,
                    pending_host_call: child.pending_host_call,
                })
                .collect(),
            dialogue_effect_callback_activations: self.dialogue_effect_callback_activations,
            live_flow_bindings: self.live_flow_bindings,
            entry_bound: self.entry_bound,
            dialogues: self.dialogues,
            active_choice: self.active_choice,
            pending_host_call: self.pending_host_call,
            started_tasks: self.started_tasks,
            task_publications: self.task_publications,
            need_publications,
            need_producers,
            queued_task_events,
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

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct AwbcProductDialogueOccurrenceSnapshot {
    pub owner: crate::runtime_id::RuntimePersistentFiberId,
    pub content: crate::runtime_id::RuntimeDialogueContentPlanId,
    pub next: u64,
}

/// The canonical Core reducer grammar is also the version-one Product wire.
/// Storage aliases retain typed node identities and duplicate inert rows until
/// original-plan admission; no intermediate reducer record is constructed.
pub type AwbcProductLineTaskLiveSnapshot = crate::line_task::LineTaskLiveSnapshot;
pub type AwbcProductLineTaskPhaseSnapshot = crate::line_task::LineTaskPhase;

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductLineTaskExitSnapshot {
    Completed,
    Cancelled,
    Failed,
}

pub type AwbcProductLineTaskNodeStateSnapshot = crate::line_task::LineTaskNodeState;

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcProductLineTaskWorkSnapshot {
    Node(u32),
    Cancellation(arcweft_interaction_model::input::InputActionId),
    Cleanup(AwbcProductLineTaskExitSnapshot),
    Defer(crate::runtime_id::RuntimeDeferRegistrationId),
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
    Program {
        handle: crate::task::TaskHandle,
        parent: crate::runtime_id::RuntimeFiberInstanceId,
    },
    LineTask {
        content: AwbcContentUnitId,
        tag: AwbcProductLineTaskWorkTagSnapshot,
        policy: AwbcProductLineTaskExitPolicySnapshot,
        phase: AwbcProductLineTaskFiberPhaseSnapshot,
    },
    Deferred {
        content: AwbcContentUnitId,
        activation: DialogueActivationId,
        registration: crate::runtime_id::RuntimeDeferRegistrationId,
        site: crate::runtime_id::RuntimeDeferSiteId,
    },
    ScopedDeferred {
        content: AwbcContentUnitId,
        activation: DialogueActivationId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: crate::awbc::schema::AwbcScopeId,
        registration: crate::runtime_id::RuntimeDeferRegistrationId,
        site: crate::runtime_id::RuntimeDeferSiteId,
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
    pub fiber: AwbcFiberStateSnapshot,
    pub runtime_generation: crate::task::GenerationId,
    pub pending_host_call: Option<AwbcProductPendingHostCallSnapshot>,
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
    admission: &mut crate::value::RuntimeRetainedValueAdmission,
) -> Result<(), AwbcProductStepBuildError> {
    admission
        .validate_metadata_collection(snapshot.queued_task_events.len())
        .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
            message: error.to_string(),
        })?;
    let mut queued_through = BTreeMap::<TaskId, TaskPublicationCursor>::new();
    for event in &snapshot.queued_task_events {
        event
            .admit_retained(
                snapshot.need_producers.task_spec(event.correlation),
                admission,
            )
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
        let cursor = TaskPublicationCursor::from_event(event);
        if queued_through
            .insert(event.correlation.task_id.clone(), cursor)
            .is_some_and(|previous| {
                !matches!(
                    cursor.compare_same_source(previous),
                    Some(Ordering::Greater)
                )
            })
            || snapshot
                .task_publications
                .get(&event.correlation.task_id)
                .is_none_or(|observed| {
                    !matches!(
                        cursor.compare_same_source(*observed),
                        Some(Ordering::Equal | Ordering::Less)
                    )
                })
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "queued task publications are not ordered under their retained cursor"
                    .to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_need_publication_cursors(
    snapshot: &AwbcProductExecutorSnapshot,
    producers: &crate::task::NeedProducerRegistrySaveSnapshot,
) -> Result<(), AwbcProductStepBuildError> {
    for ((_, correlation), observed) in &snapshot.need_publications {
        if !producers
            .task_admissions
            .iter()
            .any(|submission| submission.handle().correlation == *correlation)
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "saved Need observation has no accepted journal correlation".to_owned(),
            });
        }
        if let Some(launch) = producers
            .launches
            .iter()
            .find(|launch| launch.correlation == *correlation)
        {
            let accepted = match &launch.origin {
                NeedProducerLaunchOriginRestore::Program { .. } => {
                    crate::task::RuntimeNeedPublicationCursor::program(&launch.state)
                }
                NeedProducerLaunchOriginRestore::Need { .. } => launch
                    .publication
                    .map(crate::task::RuntimeNeedPublicationCursor::Task),
            };
            if !accepted.is_some_and(|accepted| {
                matches!(
                    observed.compare_same_source(accepted),
                    Some(Ordering::Equal | Ordering::Less)
                )
            }) {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "saved Need observation exceeds its accepted producer publication"
                        .to_owned(),
                });
            }
        } else if matches!(
            observed,
            crate::task::RuntimeNeedPublicationCursor::Program(_)
        ) {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "saved Program observation has no accepted local producer".to_owned(),
            });
        }
    }
    Ok(())
}

fn expected_deferred_children(
    snapshot: &AwbcProductExecutorSnapshot,
) -> Result<
    BTreeMap<
        DialogueActivationId,
        (
            crate::runtime_id::RuntimeDeferRegistrationId,
            crate::runtime_id::RuntimeDeferSiteId,
        ),
    >,
    AwbcProductStepBuildError,
> {
    let mut expected = BTreeMap::new();
    for child in &snapshot.child_fibers {
        let AwbcProductChildFiberOwnerSnapshot::Deferred {
            activation,
            registration,
            site,
            ..
        } = &child.owner
        else {
            continue;
        };
        if matches!(
            child.fiber.status,
            crate::awbc::fiber::FiberStatus::Returned
                | crate::awbc::fiber::FiberStatus::Cancelled
                | crate::awbc::fiber::FiberStatus::Trapped
        ) || expected
            .insert(activation.clone(), (*registration, *site))
            .is_some()
        {
            return Err(AwbcProductStepBuildError::RestoreSnapshot {
                message: "deferred child snapshot is terminal or duplicates an activation inflight registration"
                    .to_owned(),
            });
        }
    }
    Ok(expected)
}

impl AwbcProductStepExecutor {
    pub fn inert_rollback_image(
        &self,
    ) -> Result<AwbcProductExecutorSnapshot, AwbcProductSaveError> {
        self.snapshot()
    }

    /// Restores the complete inert executor while retaining producer identity.
    pub fn restore_rollback_image(
        self,
        image: AwbcProductExecutorSnapshot,
    ) -> Result<Self, (Self, AwbcProductStepBuildError)> {
        self.restore_snapshot_transaction(
            image,
            crate::task::NeedProducerRestorePolicy::Rollback,
            AwbcProductSnapshotRestoreTarget::CurrentInvocation,
        )
    }

    pub(crate) fn restore_snapshot_transaction(
        self,
        image: AwbcProductExecutorSnapshot,
        policy: crate::task::NeedProducerRestorePolicy,
        target: AwbcProductSnapshotRestoreTarget<'_>,
    ) -> Result<Self, (Self, AwbcProductStepBuildError)> {
        let admission = match self.validate_snapshot_target(&image, target) {
            Ok(admission) => admission,
            Err(error) => return Err((self, error)),
        };
        if let Err(error) = image.need_producers.validate_restore_policy(policy) {
            return Err((
                self,
                AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                },
            ));
        }
        let rollback = match self.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return Err((
                    self,
                    AwbcProductStepBuildError::RestoreSnapshot {
                        message: error.to_string(),
                    },
                ));
            }
        };
        let admitted = self.program.clone();
        let rollback_admission = match self.validate_snapshot_target(
            &rollback,
            AwbcProductSnapshotRestoreTarget::CurrentInvocation,
        ) {
            Ok(admission) => admission,
            Err(error) => return Err((self, error)),
        };
        let fingerprint = self.artifact_fingerprint;
        let format_context = self.format_context.clone();
        drop(self);
        match Self::from_admitted_snapshot(
            admitted.clone(),
            fingerprint,
            format_context.clone(),
            image,
            policy,
            &admission,
        ) {
            Ok(executor) => Ok(executor),
            Err(error) => {
                let original = Self::from_admitted_snapshot(
                    admitted,
                    fingerprint,
                    format_context,
                    rollback,
                    crate::task::NeedProducerRestorePolicy::Rollback,
                    &rollback_admission,
                )
                .expect("the original complete inert executor image remains restorable");
                Err((original, error))
            }
        }
    }

    /// Replaces this runnable executor from an inert save snapshot without
    /// ever keeping old and restored affine owners live at the same time.
    /// On a target restore failure, the captured inert rollback image rebuilds
    /// the original executor and is returned with the error.
    pub fn restore_inert_snapshot_owned(
        self,
        saved: AwbcProductExecutorSaveSnapshot,
    ) -> Result<Self, (Self, super::AwbcProductStepBuildError)> {
        let saved = match saved.into_live_for_program(&self.program) {
            Ok(saved) => saved,
            Err(error) => {
                return Err((self, error));
            }
        };
        self.restore_snapshot_owned(saved)
    }

    /// Replaces the sole runnable owner from a fully inert snapshot while
    /// retaining its original Source leases. Failure rebuilds the original
    /// executor from the same internal snapshot grammar.
    pub fn restore_snapshot_owned(
        self,
        saved: AwbcProductExecutorSnapshot,
    ) -> Result<Self, (Self, super::AwbcProductStepBuildError)> {
        self.restore_snapshot_transaction(
            saved,
            crate::task::NeedProducerRestorePolicy::Resume,
            AwbcProductSnapshotRestoreTarget::CurrentInvocation,
        )
    }

    /// Visits values in the one live executor graph without materializing a
    /// snapshot or duplicating affine payloads.
    pub fn visit_live_runtime_values<E>(
        &self,
        mut visitor: impl FnMut(&RuntimeValue) -> Result<(), E>,
    ) -> Result<(), E> {
        self.fiber.visit_runtime_values(&mut visitor)?;
        for child in &self.child_fibers {
            child.fiber.visit_runtime_values(&mut visitor)?;
        }
        self.dialogues.visit_runtime_values(&mut visitor)?;
        self.facade_fiber
            .frame
            .observations
            .visit_runtime_values(&mut visitor)?;
        for launch in self.need_producers.launches() {
            for argument in launch.arguments().into_iter().flatten() {
                visit_runtime_value_graph(&argument.value, &mut visitor)?;
            }
            visit_task_request_values(launch.task_spec(), &mut visitor)?;
            if let RuntimeNeedProducerState::Ready(value) = launch.state() {
                visit_runtime_value_graph(value.value(), &mut visitor)?;
            }
        }
        for event in &self.queued_task_events {
            if let crate::task::TaskEventKind::Ready(value) = &event.kind {
                visit_runtime_value_graph(value.value(), &mut visitor)?;
            }
        }
        if let Some(root) = &self.root {
            let mut first_error = None;
            root.visit_runtime_values(&mut |value: &RuntimeValue| {
                if first_error.is_none() {
                    first_error = visitor(value).err();
                }
            });
            if let Some(error) = first_error {
                return Err(error);
            }
        }
        Ok(())
    }

    #[must_use]
    fn admit_task_state(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcProductSaveError> {
        let invalid = |error: crate::value::AwbcRuntimeValueSnapshotError| {
            AwbcProductSaveError::InvalidSnapshot {
                message: error.to_string(),
            }
        };
        admission
            .validate_metadata_collection(self.queued_task_events.len())
            .map_err(invalid)?;
        for event in &self.queued_task_events {
            event
                .admit_retained(self.need_producers.task_spec(event.correlation), admission)
                .map_err(invalid)?;
        }
        self.need_producers
            .admit_retained(admission)
            .map_err(invalid)
    }

    fn admit_fiber_state(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcProductSaveError> {
        let invalid = |error: crate::value::AwbcRuntimeValueSnapshotError| {
            AwbcProductSaveError::InvalidSnapshot {
                message: error.to_string(),
            }
        };
        admission
            .validate_metadata_collection(self.child_fibers.len())
            .map_err(invalid)?;
        self.fiber.admit_retained(admission).map_err(invalid)?;
        for child in &self.child_fibers {
            child.fiber.admit_retained(admission).map_err(invalid)?;
        }
        self.facade_fiber
            .frame
            .observations
            .admit_retained(admission)
            .map_err(|message| AwbcProductSaveError::InvalidSnapshot { message })?;
        Ok(())
    }

    fn snapshot_admitted_task_state(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<
        (
            NeedProducerRegistrySaveSnapshot,
            VecDeque<AwbcProductTaskEventSaveSnapshot>,
        ),
        AwbcProductSaveError,
    > {
        let need_producers = self
            .need_producers
            .snapshot_admitted(owner)
            .map_err(|error| AwbcProductSaveError::InvalidSnapshot {
                message: error.to_string(),
            })?;
        let queued_task_events = self
            .queued_task_events
            .iter()
            .map(|event| {
                AwbcProductTaskEventSaveSnapshot::from_admitted_live(event, owner)
                    .map_err(|message| AwbcProductSaveError::InvalidSnapshot { message })
            })
            .collect::<Result<_, _>>()?;
        Ok((need_producers, queued_task_events))
    }
    pub fn snapshot(&self) -> Result<AwbcProductExecutorSnapshot, AwbcProductSaveError> {
        self.fiber
            .root
            .admit_executor_roots(
                self.root.as_ref(),
                &self.program,
                self.child_fibers.iter().map(|child| {
                    (
                        child.fiber.root,
                        child.fiber.status,
                        matches!(child.owner, ProductChildFiberOwner::Independent),
                        matches!(
                            child
                                .fiber
                                .suspension
                                .as_ref()
                                .map(|suspension| &suspension.reason),
                            Some(crate::awbc::fiber::FiberSuspensionReason::BudgetYield)
                        ),
                    )
                }),
            )
            .map_err(|error| AwbcProductSaveError::InvalidSnapshot {
                message: error.to_string(),
            })?;
        let owner = crate::task::RuntimeProgramOwner::Awbc(self.program_arc());
        let mut admission = self
            .program
            .retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        if let Some(root) = &self.root {
            root.admit_retained(&mut admission).map_err(|error| {
                AwbcProductSaveError::InvalidSnapshot {
                    message: error.to_string(),
                }
            })?;
        }
        self.admit_fiber_state(&mut admission)?;
        self.admit_task_state(&mut admission)?;
        self.dialogues
            .admit_retained(&mut admission, |activation, frame, line, admission| {
                frame.admit_retained(&self.program, activation, line, admission)
            })
            .map_err(|error| AwbcProductSaveError::InvalidSnapshot {
                message: error.to_string(),
            })?;
        let root = self
            .root
            .as_ref()
            .map(|root| root.snapshot_admitted(&owner))
            .transpose()
            .map_err(|message| AwbcProductSaveError::InvalidSnapshot { message })?;
        let (need_producers, queued_task_events) = self.snapshot_admitted_task_state(&owner)?;
        let fiber =
            AwbcFiberStateSnapshot::from_admitted_live(&self.fiber, &owner).map_err(|error| {
                AwbcProductSaveError::InvalidSnapshot {
                    message: error.to_string(),
                }
            })?;
        let child_fibers = self
            .child_fibers
            .iter()
            .map(|child| {
                Ok(AwbcProductChildFiberSnapshot {
                    owner: child.owner.snapshot(),
                    fiber: AwbcFiberStateSnapshot::from_admitted_live(&child.fiber, &owner)
                        .map_err(|error| error.to_string())?,
                    runtime_generation: child.runtime_generation,
                    pending_host_call: child.pending_host_call.as_ref().map(|pending| {
                        AwbcProductPendingHostCallSnapshot {
                            call: pending.call,
                            id: pending.id.0.clone(),
                        }
                    }),
                })
            })
            .collect::<Result<_, String>>()
            .map_err(|message| AwbcProductSaveError::InvalidSnapshot { message })?;
        let dialogues = self
            .dialogues
            .snapshot_admitted(&owner, |active| snapshot_active_dialogue(active, &owner))
            .map_err(|error| AwbcProductSaveError::InvalidSnapshot {
                message: error.to_string(),
            })?;
        Ok(AwbcProductExecutorSnapshot {
            root,
            runtime_generation: self.runtime_generation,
            fiber,
            child_fibers,
            dialogue_effect_callback_activations: self.dialogue_effect_callback_activations.clone(),
            live_flow_bindings: self.live_flow_bindings(),
            entry_bound: self.entry_bound,
            dialogues,
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
            need_producers,
            queued_task_events,
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
            observations:
                crate::observation::RuntimeObservationSaveSnapshot::from_admitted_live_for_program(
                    &self.facade_fiber.frame.observations,
                    &owner,
                )
                .map_err(|message| AwbcProductSaveError::InvalidSnapshot { message })?,
        })
    }

    pub(super) fn from_admitted_snapshot(
        program: super::AwbcProductProgram,
        fingerprint: crate::effect::RuntimeArtifactFingerprint,
        format_context: crate::value::RuntimeFormatContext,
        snapshot: AwbcProductExecutorSnapshot,
        policy: crate::task::NeedProducerRestorePolicy,
        admission: &crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<Self, AwbcProductStepBuildError> {
        snapshot
            .need_producers
            .validate_restore_policy(policy)
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: error.to_string(),
            })?;
        let mut snapshot = snapshot;
        let program_owner = crate::task::RuntimeProgramOwner::Awbc(program.program_arc());
        let source_program = program.task_source.clone();
        let materialization = crate::value::RuntimeValueRestoreContext::new(
            &program_owner,
            source_program
                .as_ref()
                .map(|program| crate::task::TaskSourceRestore { program, admission }),
        );
        let root = snapshot
            .root
            .take()
            .map(|image| {
                crate::root::RootRuntime::from_admitted_rollback_image(image, materialization)
            })
            .transpose()
            .map_err(|message| AwbcProductStepBuildError::RootSnapshot { message })?;
        let queued_task_events = snapshot
            .queued_task_events
            .into_iter()
            .map(|event| event.into_admitted_live(materialization))
            .collect::<Result<VecDeque<_>, _>>()
            .map_err(|message| AwbcProductStepBuildError::RestoreSnapshot { message })?;
        let need_producers = match snapshot.need_producers.into_admitted_live(materialization) {
            Ok(value) => value,
            Err(error) => {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: error.to_string(),
                });
            }
        };
        let mut registry = NeedProducerRegistry::default();
        registry
            .restore_admitted_registry_with_policy(need_producers, policy)
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: format!("invalid Need producer registry snapshot: {error}"),
            })?;
        let need_producers = registry;
        let fibers = std::iter::once(snapshot.fiber.instance)
            .chain(
                snapshot
                    .child_fibers
                    .iter()
                    .map(|child| child.fiber.instance),
            )
            .collect::<BTreeSet<_>>();
        let mut launches = BTreeSet::new();
        for child in &snapshot.child_fibers {
            let AwbcProductChildFiberOwnerSnapshot::Program { handle, parent } = &child.owner
            else {
                if matches!(child.fiber.root, crate::awbc::fiber::AwbcFiberRoot::Task(_)) {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "task root lacks its accepted Program owner".into(),
                    });
                }
                continue;
            };
            let accepted = need_producers.program_launch_for_handle(*handle);
            let matched = accepted.is_some_and(|launch| {
                let crate::awbc::fiber::AwbcFiberRoot::Task(task) = child.fiber.root else { return false; };
                let Some(crate::awbc::schema::AwbcTaskPlan { execution: crate::awbc::schema::AwbcTaskPlanExecution::Program(body), .. }) = program.task_plans.get(task.index()) else { return false; };
                matches!(launch.source().program_owner(), crate::task::RuntimeProgramOwner::Awbc(owner) if std::sync::Arc::ptr_eq(&owner, &program.program_arc()))
                    && body.source.bytes() == launch.source().task_plan().key().as_bytes()
                    && child.runtime_generation == handle.correlation.generation
            });
            if !matched
                || !fibers.contains(parent)
                || parent.get() >= child.fiber.instance.get()
                || !launches.insert(handle.correlation)
            {
                return Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "Program child origin, parent or launch is invalid".into(),
                });
            }
        }
        let fiber = snapshot
            .fiber
            .into_admitted_live_for_program(materialization)
            .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                message: format!("invalid root fiber snapshot: {error}"),
            })?;
        let mut executor =
            Self::for_fiber(program, fiber, fingerprint, snapshot.runtime_generation);
        executor.format_context = format_context;
        executor.child_fibers = snapshot
            .child_fibers
            .into_iter()
            .map(|child| {
                Ok(ProductChildFiber {
                    owner: ProductChildFiberOwner::restore(&child.owner, &need_producers)?,
                    fiber: child
                        .fiber
                        .into_admitted_live_for_program(materialization)
                        .map_err(|error| error.to_string())?,
                    runtime_generation: child.runtime_generation,
                    pending_host_call: child.pending_host_call.map(|pending| PendingHostCall {
                        call: pending.call,
                        id: RuntimeHostCallId(pending.id),
                    }),
                })
            })
            .collect::<Result<Vec<_>, String>>()
            .map_err(|message| AwbcProductStepBuildError::RestoreSnapshot { message })?
            .into();
        executor.dialogue_effect_callback_activations =
            snapshot.dialogue_effect_callback_activations;
        executor.entry_bound = snapshot.entry_bound;
        executor.dialogues =
            executor.restore_dialogue_store(snapshot.dialogues, materialization)?;
        executor.active_choice = snapshot
            .active_choice
            .map(|active| executor.restore_active_choice(active))
            .transpose()?;
        executor.pending_host_call = snapshot.pending_host_call.map(|pending| PendingHostCall {
            call: pending.call,
            id: RuntimeHostCallId(pending.id),
        });
        executor.started_tasks = snapshot.started_tasks;
        executor.task_publications = snapshot.task_publications;
        executor.need_publications = snapshot.need_publications;
        executor.need_producers = need_producers;
        executor.queued_task_events = queued_task_events;
        executor.emitted_content = snapshot.emitted_content;
        executor.stream_sequences = snapshot.stream_sequences;
        executor.next_generation = snapshot.next_generation;
        executor.next_fiber_instance = snapshot.next_fiber_instance;
        executor.dialogue_occurrences = snapshot
            .dialogue_occurrences
            .into_iter()
            .map(|row| ((row.owner, row.content), row.next))
            .collect();
        executor.next_host_call_sequence = snapshot.next_host_call_sequence;
        executor.next_audio_sequence = snapshot.next_audio_sequence;
        executor.compact_pure_stats = snapshot.compact_pure_stats;
        executor.root = root;
        executor.facade_fiber.frame.observations = snapshot
            .observations
            .into_admitted_live_for_program(materialization)
            .map_err(|message| AwbcProductStepBuildError::RestoreSnapshot { message })?;
        executor.rebuild_facade_stream_states_from_compact();
        executor.sync_facade();
        Ok(executor)
    }

    fn restore_dialogue_store(
        &self,
        snapshot: AwbcProductDialogueSnapshotState,
        materialization: crate::value::RuntimeValueRestoreContext<'_>,
    ) -> Result<super::ProductDialogueStore, AwbcProductStepBuildError> {
        super::ProductDialogueStore::from_admitted_save_snapshot(
            snapshot,
            materialization,
            |_, active, _| {
                let invalid = |error: crate::value::AwbcRuntimeValueSnapshotError| {
                    crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                        message: error.to_string(),
                    }
                };
                let effect_callbacks = active
                    .effect_callbacks
                    .into_iter()
                    .map(|binding| {
                        let value = binding
                            .callback
                            .into_admitted_runtime_value(materialization)
                            .map_err(invalid)?;
                        let RuntimeValue::Callable(callback) = value else {
                            return Err(
                                crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                                    message: "admitted dialogue callback is not callable"
                                        .to_owned(),
                                },
                            );
                        };
                        Ok((binding.site, callback))
                    })
                    .collect::<Result<_, _>>()?;
                let RuntimeValue::Opaque(target) = active
                    .target
                    .into_admitted_runtime_value(materialization)
                    .map_err(invalid)?
                else {
                    return Err(
                        crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                            message: "admitted dialogue target is not opaque".to_owned(),
                        },
                    );
                };
                Ok(ActiveDialogue {
                    activation: active.activation,
                    content: active.content,
                    target,
                    target_type: active.target_type,
                    line: active.line,
                    result: active.result,
                    captures: active
                        .captures
                        .into_vec()
                        .into_iter()
                        .map(|value| value.into_admitted_runtime_value(materialization))
                        .collect::<Result<_, _>>()
                        .map_err(invalid)?,
                    task_inputs: active
                        .task_inputs
                        .into_vec()
                        .into_iter()
                        .map(|value| value.into_admitted_runtime_value(materialization))
                        .collect::<Result<_, _>>()
                        .map_err(invalid)?,
                    values: active
                        .values
                        .into_vec()
                        .into_iter()
                        .map(|binding| {
                            Ok(crate::plan::RuntimeDialogueValueBinding {
                                slot: binding.slot,
                                role: binding.role,
                                value: binding
                                    .value
                                    .into_admitted_runtime_value(materialization)?,
                            })
                        })
                        .collect::<Result<_, crate::value::AwbcRuntimeValueSnapshotError>>()
                        .map_err(invalid)?,
                    effect_callbacks,
                    voice: active.voice,
                    phase: restore_dialogue_phase_from_save(active.phase, materialization)
                        .map_err(|message| {
                            crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                                message,
                            }
                        })?,
                    elapsed_nanos: active.elapsed_nanos,
                    pending_content_events: active.pending_content_events,
                    pending_advance: active.pending_advance,
                    pending_line_outcomes: active.pending_line_outcomes,
                    pending_activation_host_call: active.pending_activation_host_call.map(
                        |pending| PendingHostCall {
                            call: pending.call,
                            id: RuntimeHostCallId(pending.id),
                        },
                    ),
                })
            },
        )
        .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
            message: error.to_string(),
        })
    }
    pub(super) fn validate_snapshot(
        &self,
        snapshot: &AwbcProductExecutorSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        self.validate_snapshot_target(
            snapshot,
            AwbcProductSnapshotRestoreTarget::CurrentInvocation,
        )
        .map(|_| ())
    }

    pub(super) fn validate_snapshot_target(
        &self,
        snapshot: &AwbcProductExecutorSnapshot,
        target: AwbcProductSnapshotRestoreTarget<'_>,
    ) -> Result<crate::value::RuntimeRetainedValueAdmission, AwbcProductStepBuildError> {
        use crate::awbc::fiber::AwbcFiberRoot;
        match target {
            AwbcProductSnapshotRestoreTarget::CurrentInvocation => {
                let same_entry_phase = snapshot.fiber.root.entry().is_some()
                    && snapshot.fiber.root.entry() == self.fiber.root.entry()
                    && matches!(
                        (snapshot.fiber.root, self.fiber.root),
                        (AwbcFiberRoot::Entry(_), AwbcFiberRoot::RootInitializer(_))
                            | (AwbcFiberRoot::RootInitializer(_), AwbcFiberRoot::Entry(_))
                    );
                if snapshot.fiber.root != self.fiber.root && !same_entry_phase {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "saved fiber origin differs from the selected invocation"
                            .to_owned(),
                    });
                }
                if snapshot.runtime_generation != self.runtime_generation {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "Product snapshot generation pin or AwaitMany occurrence identity is invalid".to_owned(),
                    });
                }
            }
            AwbcProductSnapshotRestoreTarget::Entry(expected) => {
                let selected = snapshot
                    .active_entry_snapshot_identity(&self.program)
                    .map_err(|error| AwbcProductStepBuildError::RestoreSnapshot {
                        message: error.to_string(),
                    })?;
                if &selected != expected {
                    return Err(AwbcProductStepBuildError::RestoreSnapshot {
                        message: "saved executor differs from its selected entry contract"
                            .to_owned(),
                    });
                }
            }
        }
        let owner = crate::task::RuntimeProgramOwner::Awbc(self.program_arc());
        let mut admission = self
            .program
            .retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        snapshot.admit_retained(&owner, &mut admission)?;
        Ok(admission)
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

    pub(super) fn rebuild_facade_stream_states_from_compact(&mut self) {
        self.facade_fiber.frame.stream_states.clear();
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
            self.facade_fiber.frame.stream_states.insert(id, runtime);
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
    use crate::awbc::schema::{AwbcBlockId, AwbcFunctionId, AwbcTrapCode, AwbcTypeId};
    use crate::runtime_id::{
        DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeFiberInstanceId,
        RuntimeIdCursor, RuntimePersistentFiberId,
    };
    use std::num::{NonZeroU32, NonZeroU64};

    #[test]
    fn transient_dialogue_phase_is_rejected_by_both_codec_and_projection() {
        let phase = AwbcProductDialoguePhaseSaveSnapshot::Transitioning;
        assert!(serde_json::to_vec(&phase).is_err());
        assert!(
            serde_json::from_str::<AwbcProductDialoguePhaseSaveSnapshot>("\"Transitioning\"")
                .is_err()
        );
        let owner = crate::task::RuntimeProgramOwner::Awbc(std::sync::Arc::new(
            crate::awbc::schema::AwbcProgram::default(),
        ));
        assert_eq!(
            super::restore_dialogue_phase_from_save(phase, (&owner).into()).unwrap_err(),
            "dialogue transition cannot be restored"
        );
    }

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
                    root: crate::awbc::fiber::AwbcFiberRoot::Empty,
                    cursor: FiberCursor {
                        function: AwbcFunctionId(0),
                        block: AwbcBlockId(0),
                        instruction_offset: 0,
                    },
                    frames: Vec::new(),
                    status: FiberStatus::Returned,
                    suspension: None,
                    terminal: Some(crate::awbc::fiber::FiberTerminalValue::Returned(None)),
                    return_summary: None,
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
            target: {
                let producer =
                    crate::pattern::RuntimeOpaqueTypeProducerId::try_new("std.character_dialogue")
                        .expect("producer ID");
                let owner = crate::pattern::RuntimeOpaqueTypeOwner::exact_with(
                    producer,
                    crate::pattern::RuntimeSemanticTypeId::from_bytes([0x24; 32]),
                    crate::value::RuntimeOpaqueValueClass::Plain,
                    crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                );
                crate::value::RuntimeOpaqueValue::new_exact(
                    &owner,
                    crate::value::RuntimeValue::Unit,
                )
            },
            target_type: AwbcTypeId(0),
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.fixture")
                .expect("line"),
            captures: Box::new([]),
            task_inputs: Box::new([]),
            values: Box::new([]),
            effect_callbacks: std::collections::BTreeMap::new(),
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
                        fiber: fiber
                            .into_live_for_program(&crate::task::RuntimeProgramOwner::Awbc(
                                std::sync::Arc::new(crate::awbc::schema::AwbcProgram::default()),
                            ))
                            .expect("live fiber"),
                        pending: None,
                    },
                },
            ),
            elapsed_nanos: 0,
            pending_content_events: Vec::new(),
            pending_advance: false,
            pending_line_outcomes: Vec::new(),
            pending_activation_host_call: None,
        };
        let mut line = crate::line_task::RuntimeDialogueActivationState::<AwbcTypeId>::new();
        let program = crate::awbc::schema::AwbcProgram::default();
        assert!(
            validate_product_dialogue_phase(
                &program,
                &activation,
                &frame,
                line.result(),
                crate::line_task::LineTaskLiveState::is_closed
            )
            .is_err()
        );
        line.abandon().expect("abandon result");
        assert!(
            validate_product_dialogue_phase(
                &program,
                &activation,
                &frame,
                line.result(),
                crate::line_task::LineTaskLiveState::is_closed
            )
            .is_ok()
        );
    }
}
