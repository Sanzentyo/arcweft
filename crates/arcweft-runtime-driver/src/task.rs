use crate::session_save::BundleSessionTaskDispatchSnapshot;
use arcweft_core::task::GenerationId;
use arcweft_core::task::{
    HostRestartPolicy, LogicalEpoch, RuntimeNeedProducerDispatch, TaskDispatchIdentity,
    TaskDispatchStart, TaskEvent, TaskEventKind, TaskId, TaskPublicationCursor,
    TaskPublicationRevision, TaskSequence, TaskSpec,
};
use arcweft_core::value::{RuntimeBundleAssetContext, RuntimePayload};
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
            generation: Some(dispatch.generation.get()),
            logical_epoch: Some(dispatch.logical_epoch.0),
            sequence: Some(dispatch.sequence.0),
            cancel_scope: Some(dispatch.task.cancel_scope.0.clone()),
        }
    }

    #[must_use]
    fn cancel_event(&self, publication_revision: TaskPublicationRevision) -> TaskEvent {
        TaskEvent {
            generation: GenerationId::new(
                self.generation.expect("registered dispatch has generation"),
            ),
            logical_epoch: LogicalEpoch(self.logical_epoch.unwrap_or_default()),
            task_id: TaskId(self.id.clone()),
            sequence: TaskSequence(self.sequence.unwrap_or_default()),
            publication_revision,
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
    publication_revisions: BTreeMap<TaskSequence, TaskPublicationRevision>,
    pending_events: Vec<TaskEvent>,
}

impl RuntimeTaskRegistry {
    #[must_use]
    pub(crate) fn contains_sequence(&self, sequence: TaskSequence) -> bool {
        self.records.contains_key(&sequence)
    }

    /// Seal driver-owned dispatch identities against Product's active Need
    /// registry. Neither side alone is sufficient to authorize a save.
    pub(crate) fn snapshot_restartable(
        &self,
        launches: &[RuntimeNeedProducerDispatch],
        next_sequence: u64,
    ) -> Result<Vec<BundleSessionTaskDispatchSnapshot>, String> {
        let active_count = self
            .records
            .values()
            .filter(|record| record.status.is_active())
            .count();
        if active_count != launches.len() {
            return Err("active dispatches and Restartable Need launches differ".to_owned());
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut rows = Vec::with_capacity(launches.len());
        for launch in launches {
            if launch.restart != HostRestartPolicy::Restartable {
                return Err("non-Restartable Need passed the driver save boundary".to_owned());
            }
            if !seen.insert(&launch.task_id) {
                return Err("duplicate Restartable Need task identity".to_owned());
            }
            let (sequence, record) = self
                .records
                .iter()
                .find(|(_, record)| record.id == launch.task_id.0 && record.status.is_active())
                .ok_or_else(|| "Restartable Need has no active driver dispatch".to_owned())?;
            if sequence.0 >= next_sequence
                || record.generation != Some(launch.generation.get())
                || record.logical_epoch.is_none()
                || record.sequence != Some(sequence.0)
                || record.cancel_scope.as_deref() != Some(launch.task_spec.cancel_scope.0.as_str())
                || launch.task_spec.id != launch.task_id
            {
                return Err("Restartable Need dispatch identity disagrees with driver".to_owned());
            }
            let last_publication_revision = self.publication_revisions.get(sequence).copied();
            if (record.status == RuntimeTaskStatus::Running) != last_publication_revision.is_some()
            {
                return Err(
                    "Restartable Need publication frontier disagrees with status".to_owned(),
                );
            }
            match (last_publication_revision, launch.publication) {
                (None, None) => {}
                (
                    Some(revision),
                    Some(TaskPublicationCursor::LocalTaskEvent {
                        generation,
                        logical_epoch,
                        dispatch_sequence,
                        publication_revision,
                    }),
                ) if generation == launch.generation
                    && logical_epoch.0 == record.logical_epoch.unwrap_or_default()
                    && dispatch_sequence == *sequence
                    && publication_revision == revision => {}
                _ => {
                    return Err(
                        "Restartable Need publication cursor disagrees with driver".to_owned()
                    );
                }
            }
            rows.push(BundleSessionTaskDispatchSnapshot {
                need_id: launch.need_id.clone(),
                generation: launch.generation,
                logical_epoch: LogicalEpoch(record.logical_epoch.unwrap_or_default()),
                sequence: *sequence,
                task_id: launch.task_id.clone(),
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
            return Err("saved driver dispatches and restored Need launches differ".to_owned());
        }
        let mut registry = Self::default();
        for row in rows {
            let launch = launches
                .iter()
                .find(|launch| launch.need_id == row.need_id && launch.task_id == row.task_id)
                .ok_or_else(|| "saved dispatch has no restored Need launch".to_owned())?;
            if !launch.needs_reensure
                || launch.restart != HostRestartPolicy::Restartable
                || row.generation != launch.generation
                || row.sequence.0 >= next_sequence
                || !row.status.is_active()
                || (row.status == RuntimeTaskStatus::Running)
                    != row.last_publication_revision.is_some()
                || registry.contains_sequence(row.sequence)
            {
                return Err("saved Restartable Need dispatch cannot be re-ensured".to_owned());
            }
            registry.register_dispatch(&HostTaskDispatch {
                generation: row.generation,
                logical_epoch: row.logical_epoch,
                sequence: row.sequence,
                task: launch.task_spec.clone(),
                last_publication_revision: row.last_publication_revision,
                bundle_asset_context: None,
            });
            registry
                .records
                .get_mut(&row.sequence)
                .expect("just registered")
                .status = row.status;
            if let Some(revision) = row.last_publication_revision {
                registry
                    .publication_revisions
                    .insert(row.sequence, revision);
            }
        }
        if registry.snapshot_restartable(launches, next_sequence)? != rows {
            return Err("restored Restartable Need dispatch projection differs".to_owned());
        }
        Ok(registry)
    }

    pub fn register_dispatch(&mut self, dispatch: &HostTaskDispatch) {
        use std::collections::btree_map::Entry;
        match self.records.entry(dispatch.sequence) {
            Entry::Vacant(entry) => {
                entry.insert(RuntimeTaskRecord::from_dispatch(dispatch));
            }
            Entry::Occupied(_) => {
                panic!("task dispatch sequence was already registered");
            }
        }
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
        if !self.preflight_task_events(&events) {
            return Vec::new();
        }
        for event in &events {
            assert!(
                self.apply_task_event(event),
                "preflight accepted every event"
            );
        }
        events
    }

    fn preflight_task_events(&self, events: &[TaskEvent]) -> bool {
        let mut batch = BTreeMap::new();
        for event in events {
            if self.generation_for_event(event).is_none() {
                return false;
            }
            let Some(record) = self.records.get(&event.sequence) else {
                return false;
            };
            let (status, last_revision) = batch.entry(event.sequence).or_insert_with(|| {
                (
                    record.status,
                    self.publication_revisions.get(&event.sequence).copied(),
                )
            });
            if status.is_terminal()
                || last_revision.is_some_and(|last| event.publication_revision <= last)
                || (matches!(event.kind, TaskEventKind::Progress(_))
                    && event.publication_revision.checked_next().is_none())
            {
                return false;
            }
            *status = RuntimeTaskStatus::from_event_kind(&event.kind);
            *last_revision = Some(event.publication_revision);
        }
        true
    }

    /// Returns the dispatch generation only for the exact registered event
    /// identity. A sequence alone does not authorize releasing its generation.
    #[must_use]
    pub fn generation_for_event(&self, event: &TaskEvent) -> Option<GenerationId> {
        let record = self.records.get(&event.sequence)?;
        (record.id == event.task_id.0
            && record.generation == Some(event.generation.get())
            && record.sequence == Some(event.sequence.0)
            && record.logical_epoch == Some(event.logical_epoch.0))
        .then(|| record.generation.map(GenerationId::new))
        .flatten()
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
                let revision = self
                    .publication_revisions
                    .get(&sequence)
                    .map_or(Some(TaskPublicationRevision::FIRST), |last| {
                        last.checked_next()
                    })
                    .expect("accepted progress reserves a cancellation revision");
                self.publication_revisions.insert(sequence, revision);
                self.pending_events.push(record.cancel_event(revision));
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
        if self.generation_for_event(event).is_none() {
            return false;
        }
        let Some(record) = self.records.get_mut(&event.sequence) else {
            return false;
        };
        if record.status.is_terminal() {
            return false;
        }
        if self
            .publication_revisions
            .get(&event.sequence)
            .is_some_and(|last| event.publication_revision <= *last)
            || (matches!(event.kind, TaskEventKind::Progress(_))
                && event.publication_revision.checked_next().is_none())
        {
            return false;
        }
        self.publication_revisions
            .insert(event.sequence, event.publication_revision);
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
    /// The accepted publication frontier when a saved dispatch is re-ensured.
    pub last_publication_revision: Option<TaskPublicationRevision>,
    pub(crate) bundle_asset_context: Option<RuntimeBundleAssetContext>,
}

impl HostTaskDispatch {
    #[must_use]
    pub fn identity(&self) -> TaskDispatchIdentity {
        TaskDispatchIdentity::new(
            self.generation,
            self.logical_epoch,
            self.sequence,
            self.task.id.clone(),
        )
    }

    #[must_use]
    pub fn dispatch_start(&self) -> TaskDispatchStart {
        TaskDispatchStart::new(self.identity(), self.last_publication_revision)
    }

    /// Returns the artifact context for a bundle asset load, derived from the
    /// retained generation image when this dispatch was created or restored.
    #[must_use]
    pub const fn bundle_asset_context(&self) -> Option<RuntimeBundleAssetContext> {
        self.bundle_asset_context
    }

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

    /// Creates the next publication for this dispatch.
    pub fn into_event(self, kind: TaskEventKind) -> TaskEvent {
        let revision = self
            .dispatch_start()
            .next_publication_revision()
            .expect("an active dispatch has a next publication revision");
        self.into_event_at_revision(revision, kind)
    }

    /// Creates a later publication with the host-owned monotone revision.
    pub fn into_event_at_revision(
        self,
        publication_revision: TaskPublicationRevision,
        kind: TaskEventKind,
    ) -> TaskEvent {
        TaskEvent::from_dispatch(self.identity(), publication_revision, kind)
    }

    pub fn ordering_key(&self) -> (LogicalEpoch, TaskSequence, &str) {
        (self.logical_epoch, self.sequence, self.task.id.0.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::task::{
        CancelScopeId, HostRestartPolicy, HostTaskRequest, NeedId, TaskClass, TaskId, TaskKey,
        TaskPolicy, TaskPriority,
    };
    use arcweft_core::value::RuntimeValue;

    #[test]
    fn completion_preserves_request_epoch_and_sequence() {
        let dispatch = HostTaskDispatch {
            generation: GenerationId::new(4),
            logical_epoch: LogicalEpoch(12),
            sequence: TaskSequence(7),
            last_publication_revision: None,
            bundle_asset_context: None,
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
        assert_eq!(dispatch.generation, GenerationId::new(4));

        let identity = dispatch.identity();
        let event = dispatch.ready(RuntimePayload::new(RuntimeValue::Unit));
        assert_eq!(event.dispatch_identity(), identity);
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
    fn registry_rejects_unknown_or_mismatched_dispatch_events() {
        let dispatch = task_dispatch("task.first", "scope.a", 7);
        let mut registry = RuntimeTaskRegistry::default();
        registry.register_dispatch(&dispatch);

        let mut wrong_id = dispatch.clone().ready(unit_payload());
        wrong_id.task_id = TaskId("task.other".to_owned());
        let mut wrong_epoch = dispatch.clone().ready(unit_payload());
        wrong_epoch.logical_epoch = LogicalEpoch(wrong_epoch.logical_epoch.0 + 1);
        let mut unknown = dispatch.clone().ready(unit_payload());
        unknown.sequence = TaskSequence(8);
        let mut wrong_generation = dispatch.clone().ready(unit_payload());
        wrong_generation.generation = GenerationId::new(5);

        assert!(
            registry
                .apply_task_events(vec![wrong_id, wrong_epoch, unknown, wrong_generation])
                .is_empty()
        );
        assert_eq!(
            registry.generation_for_event(&dispatch.clone().ready(unit_payload())),
            Some(dispatch.generation)
        );
        assert_eq!(
            registry.list(RuntimeTaskListOptions::default())[0].status,
            RuntimeTaskStatus::Pending
        );
        let valid = dispatch.clone().ready(unit_payload());
        let mut invalid = valid.clone();
        invalid.generation = GenerationId::new(5);
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
                .map(|event| event.publication_revision.get())
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
        assert_eq!(cancel.publication_revision.get(), 3);
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
            generation: dispatch.generation,
            need_id: NeedId("need.first".to_owned()),
            task_id: dispatch.task.id.clone(),
            task_spec: dispatch.task.clone(),
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
        changed_need[0].need_id = NeedId("need.other".to_owned());
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
            generation: GenerationId::new(4),
            logical_epoch: LogicalEpoch(12 + sequence),
            sequence: TaskSequence(sequence),
            last_publication_revision: None,
            bundle_asset_context: None,
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
