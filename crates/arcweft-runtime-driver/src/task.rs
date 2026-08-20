use crate::swap::GenerationId;
use arcweft_core::task::{LogicalEpoch, TaskEvent, TaskEventKind, TaskId, TaskSequence, TaskSpec};
use arcweft_core::value::RuntimePayload;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Runtime-owned lifecycle status for a host task projection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTaskStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
}

impl RuntimeTaskStatus {
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        !self.is_active()
    }

    #[must_use]
    pub fn from_event_kind(kind: &TaskEventKind) -> Self {
        match kind {
            TaskEventKind::Ready(_) => Self::Completed,
            TaskEventKind::Failed(_) => Self::Failed,
            TaskEventKind::Cancelled => Self::Cancelled,
            TaskEventKind::Progress(_) => Self::Running,
        }
    }
}

/// Stable runtime-driver-owned projection of one host task.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeTaskRecord {
    pub id: String,
    pub status: RuntimeTaskStatus,
    pub generation: Option<u64>,
    pub logical_epoch: Option<u64>,
    pub sequence: Option<u64>,
    pub cancel_scope: Option<String>,
}

impl RuntimeTaskRecord {
    #[must_use]
    pub fn from_dispatch(dispatch: &HostTaskDispatch) -> Self {
        Self {
            id: dispatch.task.id.0.clone(),
            status: RuntimeTaskStatus::Pending,
            generation: Some(dispatch.generation.0),
            logical_epoch: Some(dispatch.logical_epoch.0),
            sequence: Some(dispatch.sequence.0),
            cancel_scope: Some(dispatch.task.cancel_scope.0.clone()),
        }
    }

    #[must_use]
    pub fn cancel_event(&self) -> TaskEvent {
        TaskEvent {
            logical_epoch: LogicalEpoch(self.logical_epoch.unwrap_or_default()),
            task_id: TaskId(self.id.clone()),
            sequence: TaskSequence(self.sequence.unwrap_or_default()),
            kind: TaskEventKind::Cancelled,
        }
    }

    #[must_use]
    pub fn matches_cancel_target(&self, target: &RuntimeTaskCancelTarget) -> bool {
        match target {
            RuntimeTaskCancelTarget::All => true,
            RuntimeTaskCancelTarget::Task(id) => self.id == *id,
            RuntimeTaskCancelTarget::Scope(scope) => self.cancel_scope.as_deref() == Some(scope),
        }
    }
}

/// Runtime task list filter options.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeTaskListOptions {
    pub include_completed: bool,
}

/// Runtime-owned cancellation target used by host/adapters.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTaskCancelTarget {
    All,
    Task(String),
    Scope(String),
}

/// Deterministic cancellation outcome from the runtime task owner.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeTaskCancelOutcome {
    pub cancelled: usize,
    pub pending_after: usize,
}

/// Minimal ownership boundary for adapters that need task inspection/cancellation.
pub trait RuntimeTaskOwner {
    fn runtime_tasks(&self, options: RuntimeTaskListOptions) -> Vec<RuntimeTaskRecord>;

    fn cancel_runtime_tasks(&mut self, target: RuntimeTaskCancelTarget)
    -> RuntimeTaskCancelOutcome;
}

/// Runtime-driver-owned task lifecycle projection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeTaskRegistry {
    records: BTreeMap<TaskSequence, RuntimeTaskRecord>,
    pending_events: Vec<TaskEvent>,
}

impl RuntimeTaskRegistry {
    pub fn register_dispatch(&mut self, dispatch: &HostTaskDispatch) {
        self.records.insert(
            dispatch.sequence,
            RuntimeTaskRecord::from_dispatch(dispatch),
        );
    }

    #[must_use]
    pub fn list(&self, options: RuntimeTaskListOptions) -> Vec<RuntimeTaskRecord> {
        self.records
            .values()
            .filter(|record| options.include_completed || record.status.is_active())
            .cloned()
            .collect()
    }

    pub fn apply_task_events(&mut self, events: Vec<TaskEvent>) -> Vec<TaskEvent> {
        events
            .into_iter()
            .filter_map(|event| {
                if self.apply_task_event(&event) {
                    Some(event)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn cancel(&mut self, target: &RuntimeTaskCancelTarget) -> RuntimeTaskCancelOutcome {
        let sequences = self
            .records
            .iter()
            .filter(|(_, record)| record.status.is_active() && record.matches_cancel_target(target))
            .map(|(sequence, _)| *sequence)
            .collect::<Vec<_>>();
        let cancelled = sequences.len();
        for sequence in sequences {
            if let Some(record) = self.records.get_mut(&sequence) {
                record.status = RuntimeTaskStatus::Cancelled;
                self.pending_events.push(record.cancel_event());
            }
        }
        self.sort_pending_events();
        RuntimeTaskCancelOutcome {
            cancelled,
            pending_after: self.pending_count(),
        }
    }

    pub fn drain_task_events(&mut self) -> Vec<TaskEvent> {
        std::mem::take(&mut self.pending_events)
    }

    #[must_use]
    pub fn queued_task_event_count(&self) -> usize {
        self.pending_events.len()
    }

    fn apply_task_event(&mut self, event: &TaskEvent) -> bool {
        let Some(record) = self.records.get_mut(&event.sequence) else {
            return true;
        };
        if record.status.is_terminal() {
            return false;
        }
        record.status = RuntimeTaskStatus::from_event_kind(&event.kind);
        true
    }

    fn pending_count(&self) -> usize {
        self.records
            .values()
            .filter(|record| record.status.is_active())
            .count()
    }

    fn sort_pending_events(&mut self) {
        self.pending_events.sort_by(|left, right| {
            (left.logical_epoch, left.sequence, &left.task_id).cmp(&(
                right.logical_epoch,
                right.sequence,
                &right.task_id,
            ))
        });
    }
}

impl RuntimeTaskOwner for RuntimeTaskRegistry {
    fn runtime_tasks(&self, options: RuntimeTaskListOptions) -> Vec<RuntimeTaskRecord> {
        self.list(options)
    }

    fn cancel_runtime_tasks(
        &mut self,
        target: RuntimeTaskCancelTarget,
    ) -> RuntimeTaskCancelOutcome {
        self.cancel(&target)
    }
}

/// Runtime request annotated with the deterministic host-dispatch order.
///
/// The epoch is the logical runtime tick that emitted the request. The sequence
/// is assigned in request order by `BundleSession`; browser completion order is
/// normalized back to this pair before task events enter the VM.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HostTaskDispatch {
    pub generation: GenerationId,
    pub logical_epoch: LogicalEpoch,
    pub sequence: TaskSequence,
    pub task: TaskSpec,
}

impl HostTaskDispatch {
    pub fn ready(self, value: RuntimePayload) -> TaskEvent {
        self.into_event(TaskEventKind::Ready(value))
    }

    pub fn failed(self, message: impl Into<String>) -> TaskEvent {
        self.into_event(TaskEventKind::Failed(message.into()))
    }

    pub fn cancelled(self) -> TaskEvent {
        self.into_event(TaskEventKind::Cancelled)
    }

    pub fn progress(self, value: arcweft_core::value::Progress) -> TaskEvent {
        self.into_event(TaskEventKind::Progress(value))
    }

    pub fn into_event(self, kind: TaskEventKind) -> TaskEvent {
        TaskEvent {
            logical_epoch: self.logical_epoch,
            task_id: self.task.id,
            sequence: self.sequence,
            kind,
        }
    }

    pub fn ordering_key(&self) -> (LogicalEpoch, TaskSequence, &str) {
        (self.logical_epoch, self.sequence, self.task.id.0.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::task::{
        CancelScopeId, HostTaskRequest, TaskClass, TaskId, TaskKey, TaskPolicy, TaskPriority,
    };
    use arcweft_core::value::RuntimeValue;

    #[test]
    fn completion_preserves_request_epoch_and_sequence() {
        let dispatch = HostTaskDispatch {
            generation: GenerationId(4),
            logical_epoch: LogicalEpoch(12),
            sequence: TaskSequence(7),
            task: TaskSpec::new(
                TaskId("task.test".to_owned()),
                TaskKey("task.test".to_owned()),
                TaskClass::Background,
                TaskPriority(0),
                CancelScopeId("test".to_owned()),
                TaskPolicy::AlwaysStart,
                HostTaskRequest::custom("test", "unit", []),
            ),
        };
        assert_eq!(dispatch.generation, GenerationId(4));

        let event = dispatch.ready(RuntimePayload::new(RuntimeValue::Unit));
        assert_eq!(event.logical_epoch, LogicalEpoch(12));
        assert_eq!(event.sequence, TaskSequence(7));
        assert_eq!(event.task_id.0, "task.test");
    }

    #[test]
    fn registry_lists_active_tasks_and_filters_completed_by_default() {
        let first = task_dispatch("task.first", "scope.a", 0);
        let second = task_dispatch("task.second", "scope.b", 1);
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&first);
        registry.register_dispatch(&second);

        let accepted = registry.apply_task_events(vec![first.ready(unit_payload())]);

        assert_eq!(accepted.len(), 1);
        assert_eq!(
            registry.list(RuntimeTaskListOptions::default()),
            vec![RuntimeTaskRecord {
                id: "task.second".to_owned(),
                status: RuntimeTaskStatus::Pending,
                generation: Some(4),
                logical_epoch: Some(13),
                sequence: Some(1),
                cancel_scope: Some("scope.b".to_owned()),
            }]
        );
        let all = registry.list(RuntimeTaskListOptions {
            include_completed: true,
        });
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].status, RuntimeTaskStatus::Completed);
        assert_eq!(all[1].status, RuntimeTaskStatus::Pending);
    }

    #[test]
    fn registry_cancels_one_task_by_task_id() {
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&task_dispatch("task.first", "scope.a", 0));
        registry.register_dispatch(&task_dispatch("task.second", "scope.b", 1));

        let outcome = registry.cancel(&RuntimeTaskCancelTarget::Task("task.first".to_owned()));

        assert_eq!(
            outcome,
            RuntimeTaskCancelOutcome {
                cancelled: 1,
                pending_after: 1,
            }
        );
        let events = registry.drain_task_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].task_id.0, "task.first");
        assert!(matches!(events[0].kind, TaskEventKind::Cancelled));
        let active = registry.list(RuntimeTaskListOptions::default());
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "task.second");
    }

    #[test]
    fn registry_cancels_tasks_by_scope() {
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&task_dispatch("task.first", "scope.shared", 0));
        registry.register_dispatch(&task_dispatch("task.second", "scope.shared", 1));
        registry.register_dispatch(&task_dispatch("task.third", "scope.other", 2));

        let outcome = registry.cancel(&RuntimeTaskCancelTarget::Scope("scope.shared".to_owned()));

        assert_eq!(
            outcome,
            RuntimeTaskCancelOutcome {
                cancelled: 2,
                pending_after: 1,
            }
        );
        let events = registry.drain_task_events();
        assert_eq!(
            events
                .iter()
                .map(|event| event.task_id.0.as_str())
                .collect::<Vec<_>>(),
            vec!["task.first", "task.second"]
        );
    }

    #[test]
    fn registry_cancels_all_tasks_deterministically_and_idempotently() {
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&task_dispatch("task.second", "scope.b", 1));
        registry.register_dispatch(&task_dispatch("task.first", "scope.a", 0));

        let outcome = registry.cancel(&RuntimeTaskCancelTarget::All);

        assert_eq!(
            outcome,
            RuntimeTaskCancelOutcome {
                cancelled: 2,
                pending_after: 0,
            }
        );
        let events = registry.drain_task_events();
        assert_eq!(
            events
                .iter()
                .map(|event| event.sequence.0)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            registry.cancel(&RuntimeTaskCancelTarget::All),
            RuntimeTaskCancelOutcome {
                cancelled: 0,
                pending_after: 0,
            }
        );
    }

    #[test]
    fn registry_reports_failed_and_cancelled_tasks_only_when_completed_are_included() {
        let failed = task_dispatch("task.failed", "scope.a", 0);
        let cancelled = task_dispatch("task.cancelled", "scope.b", 1);
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&failed);
        registry.register_dispatch(&cancelled);

        registry.apply_task_events(vec![failed.failed("boom")]);
        registry.cancel(&RuntimeTaskCancelTarget::Task("task.cancelled".to_owned()));

        assert!(registry.list(RuntimeTaskListOptions::default()).is_empty());
        let all = registry.list(RuntimeTaskListOptions {
            include_completed: true,
        });
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].status, RuntimeTaskStatus::Failed);
        assert_eq!(all[1].status, RuntimeTaskStatus::Cancelled);
    }

    fn task_dispatch(id: &str, scope: &str, sequence: u64) -> HostTaskDispatch {
        HostTaskDispatch {
            generation: GenerationId(4),
            logical_epoch: LogicalEpoch(12 + sequence),
            sequence: TaskSequence(sequence),
            task: TaskSpec::new(
                TaskId(id.to_owned()),
                TaskKey(id.to_owned()),
                TaskClass::Background,
                TaskPriority(0),
                CancelScopeId(scope.to_owned()),
                TaskPolicy::AlwaysStart,
                HostTaskRequest::custom("test", "unit", []),
            ),
        }
    }

    fn unit_payload() -> RuntimePayload {
        RuntimePayload::new(RuntimeValue::Unit)
    }
}
