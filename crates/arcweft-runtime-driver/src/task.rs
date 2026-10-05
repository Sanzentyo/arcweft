use crate::session_save::BundleSessionTaskDispatchSnapshot;
use arcweft_core::task::{
    CancelScopeId, GenerationId, HostRestartPolicy, LogicalEpoch, RuntimeNeedProducerDispatch,
    RuntimeTaskFailure, RuntimeTaskFailureKind, TaskDispatchIdentity, TaskDispatchStart, TaskEvent,
    TaskEventKind, TaskId, TaskPublicationCursor, TaskPublicationRevision, TaskSequence,
    TaskSubmission,
};
use arcweft_core::value::{RuntimeBundleAssetContext, RuntimePayload};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
            TaskEventKind::InfrastructureFailure(_) => Self::Failed,
            TaskEventKind::Cancelled => Self::Cancelled,
            TaskEventKind::Progress(_) => Self::Running,
        }
    }
}

/// Accepted correlation, private attempt fence and publication position for one task.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTaskRecord {
    pub dispatch: TaskDispatchIdentity,
    pub status: RuntimeTaskStatus,
    pub cursor: Option<TaskPublicationCursor>,
    pub cancel_scope: CancelScopeId,
}

impl RuntimeTaskRecord {
    #[must_use]
    pub fn from_dispatch(dispatch: &HostTaskDispatch) -> Self {
        Self {
            dispatch: dispatch.identity(),
            status: RuntimeTaskStatus::Pending,
            cursor: dispatch
                .last_publication_revision
                .map(|revision| TaskPublicationCursor {
                    logical_epoch: dispatch.logical_epoch,
                    sequence: TaskSequence(revision.get()),
                }),
            cancel_scope: dispatch.task.spec().cancel_scope.clone(),
        }
    }
    fn cancel_event(&self, revision: TaskPublicationRevision) -> TaskEvent {
        TaskEvent::from_dispatch(self.dispatch.clone(), revision, TaskEventKind::Cancelled)
    }
    #[must_use]
    pub fn matches_cancel_target(&self, target: &RuntimeTaskCancelTarget) -> bool {
        match target {
            RuntimeTaskCancelTarget::All => true,
            RuntimeTaskCancelTarget::Task(id) => self.dispatch.correlation.task_id == *id,
            RuntimeTaskCancelTarget::Scope(scope) => self.cancel_scope == *scope,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTaskListOptions {
    pub include_completed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTaskCancelTarget {
    All,
    Task(TaskId),
    Scope(CancelScopeId),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTaskCancelOutcome {
    pub cancelled: usize,
    pub pending_after: usize,
}

pub trait RuntimeTaskOwner {
    fn runtime_tasks(&self, options: RuntimeTaskListOptions) -> Vec<RuntimeTaskRecord>;
    fn cancel_runtime_tasks(&mut self, target: RuntimeTaskCancelTarget)
    -> RuntimeTaskCancelOutcome;
}

/// Driver projection indexed by accepted task identity, never publication sequence.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeTaskRegistry {
    records: BTreeMap<TaskId, RuntimeTaskRecord>,
    pending_events: Vec<TaskEvent>,
}

impl RuntimeTaskRegistry {
    #[must_use]
    pub(crate) fn contains_sequence(&self, sequence: TaskSequence) -> bool {
        self.records
            .values()
            .any(|record| record.dispatch.sequence == sequence)
    }
    pub(crate) fn snapshot_restartable(
        &self,
        launches: &[RuntimeNeedProducerDispatch],
        next_sequence: u64,
    ) -> Result<Vec<BundleSessionTaskDispatchSnapshot>, String> {
        if self
            .records
            .values()
            .filter(|record| record.status.is_active())
            .count()
            != launches.len()
        {
            return Err("active dispatches and Restartable Need launches differ".to_owned());
        }
        let mut seen = BTreeSet::new();
        let mut rows = Vec::with_capacity(launches.len());
        for launch in launches {
            let correlation = launch.submission.handle().correlation;
            if launch.restart != HostRestartPolicy::Restartable || !seen.insert(correlation) {
                return Err("invalid or duplicate Restartable Need launch".to_owned());
            }
            let record = self
                .records
                .get(&correlation.task_id)
                .filter(|record| record.status.is_active())
                .ok_or_else(|| "Restartable Need has no active driver dispatch".to_owned())?;
            if record.dispatch.correlation != correlation
                || record.dispatch.sequence.0 >= next_sequence
                || record.cancel_scope != launch.submission.spec().cancel_scope
                || record.cursor != launch.publication
                || (record.status == RuntimeTaskStatus::Running) != record.cursor.is_some()
            {
                return Err(
                    "Restartable Need dispatch or publication disagrees with driver".to_owned(),
                );
            }
            let last_publication_revision = record
                .cursor
                .map(|cursor| {
                    std::num::NonZeroU64::new(cursor.sequence.0)
                        .map(TaskPublicationRevision::new)
                        .ok_or_else(|| "accepted publication sequence cannot be zero".to_owned())
                })
                .transpose()?;
            rows.push(BundleSessionTaskDispatchSnapshot {
                correlation,
                logical_epoch: record.dispatch.logical_epoch,
                sequence: record.dispatch.sequence,
                last_publication_revision,
                status: record.status,
            });
        }
        rows.sort_by_key(|row| row.sequence);
        Ok(rows)
    }
    pub(crate) fn restore_restartable(
        rows: &[BundleSessionTaskDispatchSnapshot],
        launches: &[RuntimeNeedProducerDispatch],
        next_sequence: u64,
    ) -> Result<Self, String> {
        if rows.len() != launches.len() {
            return Err("saved dispatches and restored Need launches differ".to_owned());
        }
        let mut registry = Self::default();
        for row in rows {
            let launch = launches
                .iter()
                .find(|launch| launch.submission.handle().correlation == row.correlation)
                .ok_or_else(|| "saved dispatch has no restored accepted Need launch".to_owned())?;
            if !launch.needs_reensure
                || launch.restart != HostRestartPolicy::Restartable
                || row.sequence.0 >= next_sequence
                || !row.status.is_active()
                || (row.status == RuntimeTaskStatus::Running)
                    != row.last_publication_revision.is_some()
                || registry.contains_sequence(row.sequence)
                || registry.records.contains_key(&row.correlation.task_id)
            {
                return Err("saved Restartable Need dispatch cannot be re-ensured".to_owned());
            }
            registry.register_dispatch(&HostTaskDispatch {
                logical_epoch: row.logical_epoch,
                sequence: row.sequence,
                task: launch.submission.clone(),
                last_publication_revision: row.last_publication_revision,
                bundle_asset_context: None,
            });
            registry
                .records
                .get_mut(&row.correlation.task_id)
                .expect("registered dispatch exists")
                .status = row.status;
        }
        if registry.snapshot_restartable(launches, next_sequence)? != rows {
            return Err("restored Restartable Need dispatch projection differs".to_owned());
        }
        Ok(registry)
    }
    pub fn register_dispatch(&mut self, dispatch: &HostTaskDispatch) {
        assert!(
            !self.contains_sequence(dispatch.sequence),
            "dispatch attempt sequence must be unique"
        );
        assert!(
            self.records
                .insert(
                    dispatch.task.task_id(),
                    RuntimeTaskRecord::from_dispatch(dispatch)
                )
                .is_none(),
            "accepted task must have one driver dispatch"
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
        let events = arcweft_core::task::normalize_task_events(events);
        let mut batch = BTreeMap::new();
        for event in &events {
            let Some(record) = self.record_for_event(event) else {
                return Vec::new();
            };
            let (status, cursor) = batch
                .entry(event.correlation.task_id)
                .or_insert((record.status, record.cursor));
            if status.is_terminal()
                || cursor.is_some_and(|previous| event.cursor <= previous)
                || (matches!(event.kind, TaskEventKind::Progress(_))
                    && event.cursor.sequence.0 == u64::MAX)
            {
                return Vec::new();
            }
            *status = RuntimeTaskStatus::from_event_kind(&event.kind);
            *cursor = Some(event.cursor);
        }
        for (id, (status, cursor)) in batch {
            let record = self
                .records
                .get_mut(&id)
                .expect("event preflight retained task");
            record.status = status;
            record.cursor = cursor;
        }
        events
    }
    fn record_for_event(&self, event: &TaskEvent) -> Option<&RuntimeTaskRecord> {
        self.records
            .get(&event.correlation.task_id)
            .filter(|record| {
                record.dispatch.correlation == event.correlation
                    && record.dispatch.logical_epoch == event.cursor.logical_epoch
            })
    }
    #[must_use]
    pub fn generation_for_event(&self, event: &TaskEvent) -> Option<GenerationId> {
        self.record_for_event(event)
            .map(|record| record.dispatch.correlation.generation)
    }
    #[must_use]
    pub fn dispatch_sequence_for_event(&self, event: &TaskEvent) -> Option<TaskSequence> {
        self.record_for_event(event)
            .map(|record| record.dispatch.sequence)
    }
    pub fn cancel(&mut self, target: &RuntimeTaskCancelTarget) -> RuntimeTaskCancelOutcome {
        let mut cancelled = 0;
        for record in self
            .records
            .values_mut()
            .filter(|record| record.status.is_active() && record.matches_cancel_target(target))
        {
            let next = record
                .cursor
                .map_or(Some(TaskPublicationRevision::FIRST), |cursor| {
                    std::num::NonZeroU64::new(cursor.sequence.0)
                        .map(TaskPublicationRevision::new)
                        .and_then(TaskPublicationRevision::checked_next)
                })
                .expect("accepted progress reserves a cancellation publication");
            let event = record.cancel_event(next);
            record.status = RuntimeTaskStatus::Cancelled;
            record.cursor = Some(event.cursor);
            self.pending_events.push(event);
            cancelled += 1;
        }
        self.pending_events
            .sort_by(arcweft_core::task::compare_task_events);
        RuntimeTaskCancelOutcome {
            cancelled,
            pending_after: self
                .records
                .values()
                .filter(|record| record.status.is_active())
                .count(),
        }
    }
    pub fn drain_task_events(&mut self) -> Vec<TaskEvent> {
        std::mem::take(&mut self.pending_events)
    }
    #[must_use]
    pub fn queued_task_event_count(&self) -> usize {
        self.pending_events.len()
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

/// Accepted task with its private physical dispatch order and restart frontier.
#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskDispatch {
    pub logical_epoch: LogicalEpoch,
    pub sequence: TaskSequence,
    pub task: TaskSubmission,
    pub last_publication_revision: Option<TaskPublicationRevision>,
    pub(crate) bundle_asset_context: Option<RuntimeBundleAssetContext>,
}
impl HostTaskDispatch {
    #[must_use]
    pub fn identity(&self) -> TaskDispatchIdentity {
        TaskDispatchIdentity::new(
            self.task.handle().correlation,
            self.logical_epoch,
            self.sequence,
        )
    }
    #[must_use]
    pub fn dispatch_start(&self) -> TaskDispatchStart {
        TaskDispatchStart::new(self.identity(), self.last_publication_revision)
    }
    #[must_use]
    pub const fn bundle_asset_context(&self) -> Option<RuntimeBundleAssetContext> {
        self.bundle_asset_context
    }
    pub fn ready(self, value: RuntimePayload) -> TaskEvent {
        self.into_event(TaskEventKind::Ready(value))
    }
    pub fn failed(self, message: impl Into<String>) -> TaskEvent {
        self.into_event(TaskEventKind::InfrastructureFailure(
            RuntimeTaskFailure::new(RuntimeTaskFailureKind::WorkerFailure, message),
        ))
    }
    pub fn cancelled(self) -> TaskEvent {
        self.into_event(TaskEventKind::Cancelled)
    }
    pub fn progress(self, value: arcweft_core::value::Progress) -> TaskEvent {
        self.into_event(TaskEventKind::Progress(value))
    }
    pub fn into_event(self, kind: TaskEventKind) -> TaskEvent {
        let revision = self
            .dispatch_start()
            .next_publication_revision()
            .expect("active dispatch reserves its next publication");
        self.into_event_at_revision(revision, kind)
    }
    pub fn into_event_at_revision(
        self,
        revision: TaskPublicationRevision,
        kind: TaskEventKind,
    ) -> TaskEvent {
        TaskEvent::from_dispatch(self.identity(), revision, kind)
    }
    pub fn ordering_key(&self) -> (LogicalEpoch, TaskSequence, TaskId) {
        (self.logical_epoch, self.sequence, self.task.task_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::task::{
        CancelScopeId, HostRestartPolicy, HostTaskRequest, TaskClass, TaskPolicy, TaskPriority,
    };
    use arcweft_core::value::RuntimeValue;

    #[test]
    fn completion_preserves_correlation_and_separates_publication_from_attempt() {
        let dispatch = task_dispatch("task.test", "test", 7);
        let identity = dispatch.identity();
        let event = dispatch.ready(unit_payload());
        assert_eq!(event.correlation, identity.correlation);
        assert_eq!(event.cursor.logical_epoch, identity.logical_epoch);
        assert_eq!(event.cursor.sequence, TaskSequence(1));
        assert_eq!(identity.sequence, TaskSequence(7));
    }

    #[test]
    fn registry_lists_active_tasks_and_filters_completed_by_default() {
        let first = task_dispatch("task.first", "scope.a", 0);
        let second = task_dispatch("task.second", "scope.b", 1);
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&first);
        registry.register_dispatch(&second);

        let accepted = registry.apply_task_events(vec![first.clone().ready(unit_payload())]);

        assert_eq!(accepted.len(), 1);
        assert_eq!(
            registry.list(RuntimeTaskListOptions::default()),
            vec![RuntimeTaskRecord::from_dispatch(&second)]
        );
        let all = registry.list(RuntimeTaskListOptions {
            include_completed: true,
        });
        assert_eq!(all.len(), 2);
        assert_eq!(
            all.iter()
                .find(|record| record.dispatch.correlation == first.task.handle().correlation)
                .unwrap()
                .status,
            RuntimeTaskStatus::Completed
        );
        assert_eq!(
            all.iter()
                .find(|record| record.dispatch.correlation == second.task.handle().correlation)
                .unwrap()
                .status,
            RuntimeTaskStatus::Pending
        );
    }

    #[test]
    fn registry_rejects_unknown_or_mismatched_dispatch_events() {
        let dispatch = task_dispatch("task.first", "scope.a", 7);
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&dispatch);

        let mut wrong_id = dispatch.clone().ready(unit_payload());
        wrong_id.correlation = task_dispatch("task.other", "other", 8)
            .task
            .handle()
            .correlation;
        let mut wrong_epoch = dispatch.clone().ready(unit_payload());
        wrong_epoch.cursor.logical_epoch = LogicalEpoch(wrong_epoch.cursor.logical_epoch.0 + 1);
        let mut unknown = dispatch.clone().ready(unit_payload());
        unknown.correlation = task_dispatch("unknown", "other", 8)
            .task
            .handle()
            .correlation;
        let mut wrong_generation = dispatch.clone().ready(unit_payload());
        wrong_generation.correlation.generation = GenerationId::new(5);

        assert!(
            registry
                .apply_task_events(vec![wrong_id, wrong_epoch, unknown, wrong_generation])
                .is_empty()
        );
        assert_eq!(
            registry.generation_for_event(&dispatch.clone().ready(unit_payload())),
            Some(dispatch.task.spec().generation)
        );
        assert_eq!(
            registry.list(RuntimeTaskListOptions::default())[0].status,
            RuntimeTaskStatus::Pending
        );
        let valid = dispatch.clone().ready(unit_payload());
        let mut invalid = valid.clone();
        invalid.correlation.generation = GenerationId::new(5);
        assert!(registry.apply_task_events(vec![valid, invalid]).is_empty());
        assert_eq!(
            registry.list(RuntimeTaskListOptions::default())[0].status,
            RuntimeTaskStatus::Pending
        );
    }

    #[test]
    fn registry_requires_new_revision_within_one_dispatch() {
        use std::num::NonZeroU64;

        let dispatch = task_dispatch("task.first", "scope.a", 7);
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&dispatch);
        let progress = |revision| {
            dispatch.clone().into_event_at_revision(
                TaskPublicationRevision::new(NonZeroU64::new(revision).unwrap()),
                TaskEventKind::Progress(arcweft_core::value::Progress::new(0.5).unwrap()),
            )
        };
        let accepted = registry.apply_task_events(vec![progress(2), progress(1)]);
        assert_eq!(
            accepted
                .iter()
                .map(|event| event.cursor.sequence.0)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(
            registry
                .apply_task_events(vec![progress(2), progress(1)])
                .is_empty()
        );
        let ready = dispatch.clone().into_event_at_revision(
            TaskPublicationRevision::new(NonZeroU64::new(3).unwrap()),
            TaskEventKind::Ready(unit_payload()),
        );
        assert_eq!(registry.apply_task_events(vec![ready]).len(), 1);
        assert!(registry.apply_task_events(vec![progress(4)]).is_empty());
    }

    #[test]
    fn local_cancellation_advances_the_last_publication_revision() {
        use std::num::NonZeroU64;

        let dispatch = task_dispatch("task.first", "scope.a", 7);
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&dispatch);
        let progress = dispatch.clone().into_event_at_revision(
            TaskPublicationRevision::new(NonZeroU64::new(2).unwrap()),
            TaskEventKind::Progress(arcweft_core::value::Progress::new(0.5).unwrap()),
        );
        assert_eq!(registry.apply_task_events(vec![progress]).len(), 1);
        registry.cancel(&RuntimeTaskCancelTarget::All);
        let cancel = registry.drain_task_events().pop().unwrap();
        assert_eq!(cancel.cursor.sequence.0, 3);
        assert!(
            registry
                .apply_task_events(vec![dispatch.into_event_at_revision(
                    TaskPublicationRevision::new(NonZeroU64::new(4).unwrap()),
                    TaskEventKind::Ready(unit_payload()),
                )])
                .is_empty()
        );
    }

    #[test]
    fn restartable_dispatch_restore_preserves_tuple_and_publication_frontier() {
        use std::num::NonZeroU64;

        let dispatch = task_dispatch("task.first", "scope.a", 7);
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&dispatch);
        let progress = dispatch.clone().into_event_at_revision(
            TaskPublicationRevision::new(NonZeroU64::new(2).unwrap()),
            TaskEventKind::Progress(arcweft_core::value::Progress::new(0.5).unwrap()),
        );
        assert_eq!(registry.apply_task_events(vec![progress.clone()]).len(), 1);
        let mut launch = RuntimeNeedProducerDispatch {
            submission: dispatch.task.clone(),
            restart: HostRestartPolicy::Restartable,
            publication: Some(TaskPublicationCursor::from_event(&progress)),
            needs_reensure: false,
        };
        let saved = registry.snapshot_restartable(&[launch.clone()], 8).unwrap();
        assert_eq!(saved[0].last_publication_revision.unwrap().get(), 2);
        launch.needs_reensure = true;
        let mut changed_sequence = saved.clone();
        changed_sequence[0].sequence = TaskSequence(8);
        assert!(
            RuntimeTaskRegistry::restore_restartable(&changed_sequence, &[launch.clone()], 8)
                .is_err()
        );
        let mut changed_need = saved.clone();
        changed_need[0].correlation = task_dispatch("need.other", "other", 8)
            .task
            .handle()
            .correlation;
        assert!(
            RuntimeTaskRegistry::restore_restartable(&changed_need, &[launch.clone()], 8).is_err()
        );
        let mut restored = RuntimeTaskRegistry::restore_restartable(&saved, &[launch], 8).unwrap();
        let terminal = dispatch.into_event_at_revision(
            TaskPublicationRevision::new(NonZeroU64::new(3).unwrap()),
            TaskEventKind::Ready(unit_payload()),
        );
        assert_eq!(restored.apply_task_events(vec![terminal]).len(), 1);
        assert_eq!(restored.list(RuntimeTaskListOptions::default()).len(), 0);
    }

    #[test]
    fn registry_cancels_one_task_by_task_id() {
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&task_dispatch("task.first", "scope.a", 0));
        registry.register_dispatch(&task_dispatch("task.second", "scope.b", 1));

        let outcome = registry.cancel(&RuntimeTaskCancelTarget::Task(
            task_dispatch("task.first", "scope.a", 0).task.task_id(),
        ));

        assert_eq!(
            outcome,
            RuntimeTaskCancelOutcome {
                cancelled: 1,
                pending_after: 1,
            }
        );
        let events = registry.drain_task_events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].correlation.task_id,
            task_dispatch("task.first", "scope.a", 0).task.task_id()
        );
        assert!(matches!(events[0].kind, TaskEventKind::Cancelled));
        let active = registry.list(RuntimeTaskListOptions::default());
        assert_eq!(active.len(), 1);
        assert_eq!(
            active[0].dispatch.correlation.task_id,
            task_dispatch("task.second", "scope.b", 1).task.task_id()
        );
    }

    #[test]
    fn registry_cancels_tasks_by_scope() {
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&task_dispatch("task.first", "scope.shared", 0));
        registry.register_dispatch(&task_dispatch("task.second", "scope.shared", 1));
        registry.register_dispatch(&task_dispatch("task.third", "scope.other", 2));

        let outcome = registry.cancel(&RuntimeTaskCancelTarget::Scope(CancelScopeId(
            "scope.shared".to_owned(),
        )));

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
                .map(|event| event.correlation.task_id)
                .collect::<Vec<_>>(),
            vec![
                task_dispatch("task.first", "scope.shared", 0)
                    .task
                    .task_id(),
                task_dispatch("task.second", "scope.shared", 1)
                    .task
                    .task_id()
            ]
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
                .map(|event| registry.dispatch_sequence_for_event(event).unwrap().0)
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

        registry.apply_task_events(vec![failed.clone().failed("boom")]);
        registry.cancel(&RuntimeTaskCancelTarget::Task(
            task_dispatch("task.cancelled", "scope.b", 1).task.task_id(),
        ));

        assert!(registry.list(RuntimeTaskListOptions::default()).is_empty());
        let all = registry.list(RuntimeTaskListOptions {
            include_completed: true,
        });
        assert_eq!(all.len(), 2);
        assert_eq!(
            all.iter()
                .find(|record| record.dispatch.correlation == failed.task.handle().correlation)
                .unwrap()
                .status,
            RuntimeTaskStatus::Failed
        );
        assert_eq!(
            all.iter()
                .find(|record| record.dispatch.correlation == cancelled.task.handle().correlation)
                .unwrap()
                .status,
            RuntimeTaskStatus::Cancelled
        );
    }

    fn task_dispatch(id: &str, scope: &str, sequence: u64) -> HostTaskDispatch {
        use arcweft_core::{
            pattern::RuntimeCheckedType,
            task::{
                NeedProducerContractDigest, NeedProducerFamily, NeedProducerInstance,
                NeedProducerSiteDigest, NeedProducerSpec, RuntimeTypeSemanticDigest,
                TaskAdmissionJournal, TaskOutcomeContract, TaskPlanSemanticDigest, TaskSpec,
            },
        };
        let outcome = TaskOutcomeContract::new(RuntimeCheckedType::Unit);
        let producer = NeedProducerSpec::new(
            NeedProducerFamily::HostAdapterTask,
            NeedProducerContractDigest::from_bytes([1; 32]),
            TaskPlanSemanticDigest::from_bytes([2; 32]),
            NeedProducerSiteDigest::from_bytes([3; 32]),
            RuntimeTypeSemanticDigest::from_bytes(*outcome.payload_semantic_identity().as_bytes()),
            RuntimeValue::Tuple(vec![RuntimeValue::String(id.into())])
                .try_digest(1024)
                .unwrap(),
        );
        let spec = TaskSpec {
            generation: GenerationId::new(4),
            producer: NeedProducerInstance::try_from(&producer).unwrap(),
            class: TaskClass::Background,
            priority: TaskPriority(0),
            cancel_scope: CancelScopeId(scope.into()),
            policy: TaskPolicy::AlwaysStart,
            outcome,
            request: HostTaskRequest::custom("test", "unit", [RuntimePayload::from(id)]),
            debug_label: id.into(),
        };
        let mut journal = TaskAdmissionJournal::default();
        let handle = journal.ensure_task(spec).unwrap();
        HostTaskDispatch {
            logical_epoch: LogicalEpoch(12 + sequence),
            sequence: TaskSequence(sequence),
            last_publication_revision: None,
            bundle_asset_context: None,
            task: journal.submission(handle).unwrap(),
        }
    }

    fn unit_payload() -> RuntimePayload {
        RuntimePayload::new(RuntimeValue::Unit)
    }
}
