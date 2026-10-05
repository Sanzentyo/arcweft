//! Sans I/O runtime task scheduler.
//!
//! The scheduler owns deterministic task submission, key-based joining,
//! cancellation bookkeeping, and dispatch ordering. Host adapters still own
//! actual I/O, worker pools, clocks, and OS integration.

use arcweft_core::task::{
    BoundTaskSpec, CancelScopeId, GenerationId, LogicalEpoch, SchedulerBudget, TaskClass,
    TaskCompletionError, TaskEnsureError, TaskEvent, TaskEventKind, TaskId, TaskKey, TaskPolicy,
    TaskPublicationRevision, TaskSequence, compare_task_events, task_events_are_normalized,
};
use std::collections::{BTreeMap, BTreeSet};

/// Scheduler policy chosen by a host adapter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeSchedulerConfig {
    pub default_budget: SchedulerBudget,
}

/// Deterministic runtime scheduler state.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeScheduler {
    config: RuntimeSchedulerConfig,
    next_order: u64,
    pending: Vec<ScheduledTask>,
    pending_sorted: bool,
    in_flight: BTreeMap<TaskId, InFlightTask>,
    accepted_specs: BTreeMap<TaskId, BoundTaskSpec>,
    terminal_task_ids: BTreeSet<TaskId>,
    publication_cursors: BTreeMap<TaskId, arcweft_core::task::TaskPublicationCursor>,
    cancel_scopes: BTreeSet<CancelScopeId>,
    stats: RuntimeSchedulerStats,
}

/// Tasks and cancellations ready for a host adapter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchedulerDispatchBatch {
    pub tasks: Vec<BoundTaskSpec>,
    pub cancel_scopes: Vec<CancelScopeId>,
}

/// Cumulative scheduler counters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimeSchedulerStats {
    pub submitted: usize,
    pub joined: usize,
    pub dispatched: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub cancel_requested: usize,
    pub in_flight: usize,
    pub max_in_flight: usize,
    pub dispatch_sorts: usize,
    pub dispatch_sort_items: usize,
    pub completion_sorts: usize,
    pub completion_sort_items: usize,
    pub completion_normalization_passes: usize,
    pub completion_normalization_checks: usize,
    pub completion_events_in: usize,
    pub completion_events_out: usize,
    pub completion_sort_skipped_items: usize,
    pub completion_sort_performed_items: usize,
    pub submitted_by_class: TaskClassCounts,
    pub dispatched_by_class: TaskClassCounts,
    pub completed_by_class: TaskClassCounts,
}

/// Cumulative task counters split by scheduler task class.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskClassCounts {
    pub local_view: usize,
    pub io: usize,
    pub cpu: usize,
    pub gpu_prepare: usize,
    pub shader_compile: usize,
    pub wasm_call: usize,
    pub asset_decode: usize,
    pub audio_decode: usize,
    pub audio_render: usize,
    pub tts_synthesis: usize,
    pub bgm_precompose: usize,
    pub lsp: usize,
    pub background: usize,
}

impl TaskClassCounts {
    const fn empty() -> Self {
        Self {
            local_view: 0,
            io: 0,
            cpu: 0,
            gpu_prepare: 0,
            shader_compile: 0,
            wasm_call: 0,
            asset_decode: 0,
            audio_decode: 0,
            audio_render: 0,
            tts_synthesis: 0,
            bgm_precompose: 0,
            lsp: 0,
            background: 0,
        }
    }

    fn record(&mut self, class: &TaskClass) {
        match class {
            TaskClass::LocalView => self.local_view += 1,
            TaskClass::Io => self.io += 1,
            TaskClass::Cpu => self.cpu += 1,
            TaskClass::GpuPrepare => self.gpu_prepare += 1,
            TaskClass::ShaderCompile => self.shader_compile += 1,
            TaskClass::WasmCall => self.wasm_call += 1,
            TaskClass::AssetDecode => self.asset_decode += 1,
            TaskClass::AudioDecode => self.audio_decode += 1,
            TaskClass::AudioRender => self.audio_render += 1,
            TaskClass::TtsSynthesis => self.tts_synthesis += 1,
            TaskClass::BgmPrecompose => self.bgm_precompose += 1,
            TaskClass::Lsp => self.lsp += 1,
            TaskClass::Background => self.background += 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ScheduledTask {
    spec: BoundTaskSpec,
    order: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct InFlightTask {
    class: TaskClass,
}

impl RuntimeScheduler {
    /// Creates an empty deterministic scheduler.
    pub const fn new(config: RuntimeSchedulerConfig) -> Self {
        Self {
            config,
            next_order: 0,
            pending: Vec::new(),
            pending_sorted: true,
            in_flight: BTreeMap::new(),
            accepted_specs: BTreeMap::new(),
            terminal_task_ids: BTreeSet::new(),
            publication_cursors: BTreeMap::new(),
            cancel_scopes: BTreeSet::new(),
            stats: RuntimeSchedulerStats {
                submitted: 0,
                joined: 0,
                dispatched: 0,
                completed: 0,
                failed: 0,
                cancelled: 0,
                cancel_requested: 0,
                in_flight: 0,
                max_in_flight: 0,
                dispatch_sorts: 0,
                dispatch_sort_items: 0,
                completion_sorts: 0,
                completion_sort_items: 0,
                completion_normalization_passes: 0,
                completion_normalization_checks: 0,
                completion_events_in: 0,
                completion_events_out: 0,
                completion_sort_skipped_items: 0,
                completion_sort_performed_items: 0,
                submitted_by_class: TaskClassCounts::empty(),
                dispatched_by_class: TaskClassCounts::empty(),
                completed_by_class: TaskClassCounts::empty(),
            },
        }
    }

    /// Submits runtime tasks after validating the whole batch. A rejected batch
    /// leaves scheduler state and counters unchanged.
    pub fn submit(
        &mut self,
        tasks: impl IntoIterator<Item = BoundTaskSpec>,
    ) -> Result<(), TaskEnsureError> {
        let mut staged = BTreeMap::new();
        let mut launches = Vec::new();
        let mut reused_joins = 0;
        for spec in tasks {
            let task_id = spec.task_id();
            if let Some(existing) = staged
                .get(&task_id)
                .or_else(|| self.accepted_specs.get(&task_id))
            {
                if !existing.same_identity_spec(&spec) {
                    return Err(TaskEnsureError::TaskIdSpecificationConflict { task_id });
                }
                if spec.spec().policy == TaskPolicy::JoinSameKey {
                    reused_joins += 1;
                }
                continue;
            }
            staged.insert(task_id, spec.clone());
            launches.push(spec);
        }
        self.stats.joined += reused_joins;
        for spec in launches {
            self.launch(spec);
        }
        Ok(())
    }
    /// Records a cancellation request for the next dispatch batch.
    pub fn cancel_scope(&mut self, scope: CancelScopeId) {
        if self.cancel_scopes.insert(scope) {
            self.stats.cancel_requested += 1;
        }
    }

    /// Dispatches pending tasks in deterministic priority order.
    pub fn dispatch(&mut self, budget: SchedulerBudget) -> SchedulerDispatchBatch {
        let max_events = if budget.max_events == 0 {
            self.config.default_budget.max_events
        } else {
            budget.max_events
        };
        if self.pending.len() > 1 && !self.pending_sorted {
            self.stats.dispatch_sorts += 1;
            self.stats.dispatch_sort_items += self.pending.len();
            self.pending.sort_by(compare_scheduled_tasks);
        }
        self.pending_sorted = true;
        let dispatch_count = self.pending.len().min(max_events);
        let scheduled = if dispatch_count == self.pending.len() {
            std::mem::take(&mut self.pending)
        } else {
            self.pending.drain(..dispatch_count).collect()
        };
        let tasks = scheduled
            .into_iter()
            .map(|scheduled| scheduled.spec)
            .collect::<Vec<_>>();
        self.stats.dispatched += tasks.len();
        for task in &tasks {
            self.stats.dispatched_by_class.record(&task.spec().class);
        }
        SchedulerDispatchBatch {
            tasks,
            cancel_scopes: std::mem::take(&mut self.cancel_scopes)
                .into_iter()
                .collect(),
        }
    }

    /// Completes in-flight tasks and returns replay-normalized task events.
    /// Unknown, duplicate-terminal, and waiter-owned events are rejected before
    /// scheduler state or counters change.
    pub fn complete(
        &mut self,
        events: impl IntoIterator<Item = TaskEvent>,
    ) -> Result<Vec<TaskEvent>, TaskCompletionError> {
        let mut events = events.into_iter().collect::<Vec<_>>();
        self.preflight_completions(&events)?;
        self.stats.completion_events_in += events.len();
        self.normalize_completion_events(&mut events);

        for event in &mut events {
            self.admit_ready_event(event);
        }

        for event in &events {
            self.publication_cursors
                .insert(event.correlation.task_id.clone(), event.cursor);
            self.complete_one(event);
        }
        self.stats.completion_events_out += events.len();
        self.refresh_in_flight_stats();
        Ok(events)
    }

    /// Whether this scheduler has accepted the task identity. Accepted ids stay
    /// reserved after terminal completion so they cannot be reused and confuse
    /// later completion events.
    #[must_use]
    pub fn contains_task_id(&self, task_id: &TaskId) -> bool {
        self.accepted_specs.contains_key(task_id)
    }

    /// Returns current cumulative scheduler counters.
    pub fn stats(&self) -> RuntimeSchedulerStats {
        let mut stats = self.stats;
        stats.in_flight = self.in_flight.len();
        stats
    }

    fn launch(&mut self, spec: BoundTaskSpec) {
        let task_spec = spec.spec();
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        self.track_in_flight(&spec);
        self.accepted_specs.insert(spec.task_id(), spec.clone());
        self.stats.submitted_by_class.record(&task_spec.class);
        let scheduled = ScheduledTask { spec, order };
        self.pending_sorted = self.pending_sorted
            && self
                .pending
                .last()
                .is_none_or(|last| compare_scheduled_tasks(last, &scheduled).is_le());
        self.pending.push(scheduled);
        self.stats.submitted += 1;
        self.refresh_in_flight_stats();
    }

    fn track_in_flight(&mut self, spec: &BoundTaskSpec) {
        let task_spec = spec.spec();
        self.in_flight.insert(
            spec.task_id(),
            InFlightTask {
                class: task_spec.class.clone(),
            },
        );
    }

    fn complete_one(&mut self, event: &TaskEvent) {
        if matches!(event.kind, TaskEventKind::Progress(_)) {
            return;
        }
        let task = self
            .in_flight
            .remove(&event.correlation.task_id)
            .expect("completion preflight retained every task owner");
        self.stats.completed_by_class.record(&task.class);
        match event.kind {
            TaskEventKind::Ready(_) => self.stats.completed += 1,
            TaskEventKind::InfrastructureFailure(_) => {
                self.stats.failed += 1;
            }
            TaskEventKind::Cancelled => {
                self.stats.cancelled += 1;
            }
            TaskEventKind::Progress(_) => unreachable!("progress is nonterminal"),
        }
        self.terminal_task_ids
            .insert(event.correlation.task_id.clone());
    }

    fn preflight_completions(&self, events: &[TaskEvent]) -> Result<(), TaskCompletionError> {
        let mut ordered = events.iter().collect::<Vec<_>>();
        ordered.sort_by(|left, right| compare_task_events(left, right));
        let mut terminal_in_batch = BTreeSet::new();
        let mut batch_cursors = BTreeMap::new();

        for event in &ordered {
            if self.terminal_task_ids.contains(&event.correlation.task_id)
                || terminal_in_batch.contains(&event.correlation.task_id)
            {
                return Err(if is_terminal_event(&event.kind) {
                    TaskCompletionError::DuplicateTerminalEvent {
                        task_id: event.correlation.task_id.clone(),
                    }
                } else {
                    TaskCompletionError::EventAfterTerminal {
                        task_id: event.correlation.task_id.clone(),
                    }
                });
            }

            if !self.in_flight.contains_key(&event.correlation.task_id) {
                return Err(TaskCompletionError::UnknownTask {
                    task_id: event.correlation.task_id.clone(),
                });
            }

            let accepted = self
                .accepted_specs
                .get(&event.correlation.task_id)
                .expect("in-flight task retains its accepted specification");
            if event.correlation != accepted.handle().correlation {
                return Err(TaskCompletionError::DispatchMismatch {
                    task_id: event.correlation.task_id,
                });
            }
            if batch_cursors
                .get(&event.correlation.task_id)
                .or_else(|| self.publication_cursors.get(&event.correlation.task_id))
                .is_some_and(|previous| event.cursor <= *previous)
            {
                return Err(TaskCompletionError::StalePublication {
                    task_id: event.correlation.task_id,
                });
            }
            batch_cursors.insert(event.correlation.task_id, event.cursor);
            if matches!(event.kind, TaskEventKind::Progress(_))
                && event.cursor.sequence.0 == u64::MAX
            {
                return Err(TaskCompletionError::PublicationRevisionExhausted {
                    task_id: event.correlation.task_id,
                });
            }

            if is_terminal_event(&event.kind) {
                terminal_in_batch.insert(event.correlation.task_id.clone());
            }
        }

        Ok(())
    }

    fn admit_ready_event(&self, event: &mut TaskEvent) {
        let TaskEventKind::Ready(payload) = &event.kind else {
            return;
        };
        let spec = self
            .accepted_specs
            .get(&event.correlation.task_id)
            .expect("completion preflight retained every accepted task");
        if let Err(error) = spec.outcome().validate_value(payload.value()) {
            event.kind =
                TaskEventKind::InfrastructureFailure(arcweft_core::task::RuntimeTaskFailure::new(
                    arcweft_core::task::RuntimeTaskFailureKind::AdapterProtocolViolation,
                    error.to_string(),
                ));
        }
    }

    fn refresh_in_flight_stats(&mut self) {
        self.stats.in_flight = self.in_flight.len();
        self.stats.max_in_flight = self.stats.max_in_flight.max(self.in_flight.len());
    }

    fn normalize_completion_events(&mut self, events: &mut [TaskEvent]) {
        self.stats.completion_normalization_passes += 1;
        if events.len() <= 1 {
            return;
        }
        self.stats.completion_normalization_checks += 1;
        if task_events_are_normalized(events) {
            self.stats.completion_sort_skipped_items += events.len();
        } else {
            self.stats.completion_sorts += 1;
            self.stats.completion_sort_items += events.len();
            self.stats.completion_sort_performed_items += events.len();
            events.sort_by(compare_task_events);
        }
    }
}

fn is_terminal_event(kind: &TaskEventKind) -> bool {
    matches!(
        kind,
        TaskEventKind::Ready(_)
            | TaskEventKind::InfrastructureFailure(_)
            | TaskEventKind::Cancelled
    )
}

impl Default for RuntimeScheduler {
    fn default() -> Self {
        Self::new(RuntimeSchedulerConfig::default())
    }
}

impl Default for RuntimeSchedulerConfig {
    fn default() -> Self {
        Self {
            default_budget: SchedulerBudget {
                max_events: usize::MAX,
            },
        }
    }
}

fn compare_scheduled_tasks(left: &ScheduledTask, right: &ScheduledTask) -> std::cmp::Ordering {
    right
        .spec
        .spec()
        .priority
        .cmp(&left.spec.spec().priority)
        .then_with(|| left.order.cmp(&right.order))
        .then_with(|| left.spec.task_id().cmp(&right.spec.task_id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::{
        entry::RuntimeSchemaLimits,
        pattern::RuntimeCheckedType,
        task::{
            FileReadTextRequest, HostTaskRequest, NeedProducerContractDigest, NeedProducerFamily,
            NeedProducerInstance, NeedProducerSiteDigest, NeedProducerSpec, RuntimeTaskFailure,
            RuntimeTaskFailureKind, RuntimeTypeSemanticDigest, TaskAdmissionJournal,
            TaskOutcomeContract, TaskPlanSemanticDigest, TaskPriority, TaskPublicationCursor,
            TaskSpec,
        },
        value::{Progress, RuntimePayload, RuntimeValue},
    };

    fn spec(path: &str, policy: TaskPolicy, priority: i32) -> TaskSpec {
        let producer = NeedProducerSpec::new(
            NeedProducerFamily::HostAdapterTask,
            NeedProducerContractDigest::from_bytes([1; 32]),
            TaskPlanSemanticDigest::from_bytes([2; 32]),
            NeedProducerSiteDigest::from_bytes([3; 32]),
            RuntimeTypeSemanticDigest::from_bytes(
                *RuntimeCheckedType::String
                    .semantic_identity_digest()
                    .as_bytes(),
            ),
            RuntimeValue::Tuple(vec![RuntimeValue::String(path.into())])
                .try_digest(1024)
                .unwrap(),
        );
        TaskSpec {
            generation: GenerationId::new(1),
            producer: NeedProducerInstance::try_from(&producer).unwrap(),
            class: TaskClass::Io,
            priority: TaskPriority(priority),
            cancel_scope: CancelScopeId("test".into()),
            policy,
            outcome: TaskOutcomeContract::new(RuntimeCheckedType::String),
            request: HostTaskRequest::FileReadText(FileReadTextRequest { path: path.into() }),
            debug_label: path.into(),
        }
    }
    fn admit(journal: &mut TaskAdmissionJournal, spec: TaskSpec) -> BoundTaskSpec {
        let handle = journal.ensure_task(spec).unwrap();
        BoundTaskSpec::bind(
            journal.submission(handle).unwrap(),
            None,
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap()
    }
    fn task(path: &str, policy: TaskPolicy, priority: i32) -> BoundTaskSpec {
        admit(
            &mut TaskAdmissionJournal::default(),
            spec(path, policy, priority),
        )
    }
    fn event(task: &BoundTaskSpec, revision: u64, kind: TaskEventKind) -> TaskEvent {
        TaskEvent {
            correlation: task.handle().correlation,
            cursor: TaskPublicationCursor {
                logical_epoch: LogicalEpoch(0),
                sequence: TaskSequence(revision),
            },
            kind,
        }
    }
    fn ready(task: &BoundTaskSpec, revision: u64) -> TaskEvent {
        event(
            task,
            revision,
            TaskEventKind::Ready(RuntimePayload::from("ok")),
        )
    }
    fn progress(task: &BoundTaskSpec, revision: u64) -> TaskEvent {
        event(
            task,
            revision,
            TaskEventKind::Progress(Progress::new(0.5).unwrap()),
        )
    }

    #[test]
    fn joins_same_key_in_flight_tasks() {
        let mut scheduler = RuntimeScheduler::default();
        let mut journal = TaskAdmissionJournal::default();
        let first = admit(&mut journal, spec("asset.bg", TaskPolicy::JoinSameKey, 0));
        let second = admit(&mut journal, spec("asset.bg", TaskPolicy::JoinSameKey, 0));
        assert_eq!(first.handle(), second.handle());
        scheduler.submit([first.clone()]).unwrap();
        scheduler.submit([second]).unwrap();
        let batch = scheduler.dispatch(SchedulerBudget { max_events: 8 });
        assert_eq!(batch.tasks, [first]);
        assert_eq!(scheduler.stats().submitted, 1);
        assert_eq!(scheduler.stats().joined, 1);
        assert_eq!(scheduler.stats().in_flight, 1);
    }
    #[test]
    fn joined_receipt_has_one_owner_publication() {
        let mut scheduler = RuntimeScheduler::default();
        let owner = task("asset.bg", TaskPolicy::JoinSameKey, 0);
        scheduler.submit([owner.clone()]).unwrap();
        scheduler.dispatch(SchedulerBudget { max_events: 8 });
        scheduler.submit([owner.clone(), owner.clone()]).unwrap();
        let events = scheduler.complete([ready(&owner, 1)]).unwrap();
        assert_eq!(events, [ready(&owner, 1)]);
        assert_eq!(scheduler.stats().joined, 2);
        assert_eq!(scheduler.stats().completed, 1);
        assert_eq!(scheduler.stats().completion_events_in, 1);
        assert_eq!(scheduler.stats().completion_events_out, 1);
        assert_eq!(scheduler.stats().in_flight, 0);
    }
    #[test]
    fn always_start_does_not_join_same_key_tasks() {
        let mut journal = TaskAdmissionJournal::default();
        let mut scheduler = RuntimeScheduler::default();
        let a = admit(&mut journal, spec("asset.bg", TaskPolicy::AlwaysStart, 0));
        let b = admit(&mut journal, spec("asset.bg", TaskPolicy::AlwaysStart, 0));
        assert_eq!(
            a.handle().correlation.task_key,
            b.handle().correlation.task_key
        );
        assert_ne!(a.task_id(), b.task_id());
        scheduler.submit([a, b]).unwrap();
        assert_eq!(
            scheduler
                .dispatch(SchedulerBudget { max_events: 8 })
                .tasks
                .len(),
            2
        );
        assert_eq!(scheduler.stats().joined, 0);
        assert_eq!(scheduler.stats().max_in_flight, 2);
    }
    #[test]
    fn dispatches_by_priority_then_submission_order() {
        let mut scheduler = RuntimeScheduler::default();
        let high_a = task("high-a", TaskPolicy::AlwaysStart, 9);
        let high_b = task("high-b", TaskPolicy::AlwaysStart, 9);
        scheduler
            .submit([
                task("low", TaskPolicy::AlwaysStart, 1),
                high_a.clone(),
                high_b.clone(),
            ])
            .unwrap();
        assert_eq!(
            scheduler.dispatch(SchedulerBudget { max_events: 2 }).tasks,
            [high_a, high_b]
        );
        assert_eq!(scheduler.stats().dispatch_sorts, 1);
        assert_eq!(scheduler.stats().dispatch_sort_items, 3);
    }
    #[test]
    fn dispatch_avoids_sort_when_submissions_are_already_ordered() {
        let mut scheduler = RuntimeScheduler::default();
        let tasks = [
            task("high-a", TaskPolicy::AlwaysStart, 9),
            task("high-b", TaskPolicy::AlwaysStart, 9),
            task("low", TaskPolicy::AlwaysStart, 1),
        ];
        scheduler.submit(tasks.clone()).unwrap();
        assert_eq!(
            scheduler.dispatch(SchedulerBudget { max_events: 8 }).tasks,
            tasks
        );
        assert_eq!(scheduler.stats().dispatch_sorts, 0);
    }
    #[test]
    fn completion_updates_stats_and_normalizes_events() {
        let mut scheduler = RuntimeScheduler::default();
        let a = task("a", TaskPolicy::AlwaysStart, 0);
        let b = task("b", TaskPolicy::AlwaysStart, 0);
        scheduler.submit([a.clone(), b.clone()]).unwrap();
        let fault = event(
            &b,
            2,
            TaskEventKind::InfrastructureFailure(RuntimeTaskFailure::new(
                RuntimeTaskFailureKind::WorkerFailure,
                "failed",
            )),
        );
        let mut expected = vec![fault.clone(), ready(&a, 1)];
        expected.sort_by(compare_task_events);
        let mut shuffled = expected.clone();
        shuffled.reverse();
        assert_eq!(scheduler.complete(shuffled).unwrap(), expected);
        assert_eq!(scheduler.stats().completed, 1);
        assert_eq!(scheduler.stats().failed, 1);
        assert_eq!(scheduler.stats().in_flight, 0);
        assert_eq!(scheduler.stats().completion_sorts, 1);
    }
    #[test]
    fn progress_keeps_joined_work_in_flight_until_terminal_delivery() {
        let mut scheduler = RuntimeScheduler::default();
        let owner = task("asset.bg", TaskPolicy::JoinSameKey, 0);
        scheduler.submit([owner.clone(), owner.clone()]).unwrap();
        assert_eq!(
            scheduler.complete([progress(&owner, 1)]).unwrap(),
            [progress(&owner, 1)]
        );
        assert_eq!(scheduler.stats().in_flight, 1);
        assert_eq!(scheduler.stats().completed, 0);
        scheduler.submit([owner.clone()]).unwrap();
        assert_eq!(
            scheduler.complete([ready(&owner, 2)]).unwrap(),
            [ready(&owner, 2)]
        );
        assert_eq!(scheduler.stats().completed_by_class.io, 1);
        assert_eq!(scheduler.stats().in_flight, 0);
    }
    #[test]
    fn conflicting_receipt_rejects_whole_submission_batch() {
        let mut scheduler = RuntimeScheduler::default();
        let owner = task("asset.bg", TaskPolicy::JoinSameKey, 0);
        scheduler.submit([owner.clone()]).unwrap();
        let before = scheduler.clone();
        let conflict = task("asset.bg", TaskPolicy::JoinSameKey, 1);
        assert_eq!(conflict.task_id(), owner.task_id());
        assert_eq!(
            scheduler.submit([task("other", TaskPolicy::AlwaysStart, 0), conflict]),
            Err(TaskEnsureError::TaskIdSpecificationConflict {
                task_id: owner.task_id()
            })
        );
        assert_eq!(scheduler, before);
    }
    #[test]
    fn same_key_join_compares_all_non_identity_task_semantics() {
        let original = spec("asset.bg", TaskPolicy::JoinSameKey, 0);
        let mut diagnostic = original.clone();
        diagnostic.debug_label = "diagnostic only".into();
        assert!(original.same_join_contract(&diagnostic));
        let mut variants = vec![];
        let mut changed = original.clone();
        changed.request = HostTaskRequest::FileReadText(FileReadTextRequest {
            path: "other".into(),
        });
        variants.push(changed);
        let mut changed = original.clone();
        changed.class = TaskClass::Cpu;
        variants.push(changed);
        let mut changed = original.clone();
        changed.priority = TaskPriority(1);
        variants.push(changed);
        let mut changed = original.clone();
        changed.cancel_scope = CancelScopeId("other".into());
        variants.push(changed);
        let mut changed = original.clone();
        changed.policy = TaskPolicy::AlwaysStart;
        variants.push(changed);
        let mut changed = original.clone();
        changed.generation = GenerationId::new(2);
        variants.push(changed);
        let mut changed = original.clone();
        changed.outcome = TaskOutcomeContract::new(RuntimeCheckedType::Bool);
        variants.push(changed);
        variants.push(spec("other", TaskPolicy::JoinSameKey, 0));
        for changed in variants {
            assert!(!original.same_join_contract(&changed));
        }
    }
    #[test]
    fn task_id_resubmission_ignores_only_diagnostic_changes() {
        let mut scheduler = RuntimeScheduler::default();
        let original = task("asset.bg", TaskPolicy::AlwaysStart, 0);
        scheduler.submit([original.clone()]).unwrap();
        let before = scheduler.clone();
        let mut diagnostic = original.spec().clone();
        diagnostic.debug_label = "renamed".into();
        scheduler
            .submit([admit(&mut TaskAdmissionJournal::default(), diagnostic)])
            .unwrap();
        assert_eq!(scheduler, before);
        assert_eq!(
            scheduler.submit([task("asset.bg", TaskPolicy::AlwaysStart, 1)]),
            Err(TaskEnsureError::TaskIdSpecificationConflict {
                task_id: original.task_id()
            })
        );
        assert_eq!(scheduler, before);
    }
    #[test]
    fn wrong_ready_payload_becomes_one_protocol_failure() {
        let mut scheduler = RuntimeScheduler::default();
        let owner = task("asset.bg", TaskPolicy::JoinSameKey, 0);
        scheduler.submit([owner.clone(), owner.clone()]).unwrap();
        let events = scheduler
            .complete([event(
                &owner,
                1,
                TaskEventKind::Ready(RuntimeValue::Bool(true).into()),
            )])
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0].kind, TaskEventKind::InfrastructureFailure(f)
            if f.kind == RuntimeTaskFailureKind::AdapterProtocolViolation)
        );
        assert_eq!(scheduler.stats().failed, 1);
        assert_eq!(scheduler.stats().completed, 0);
    }
    #[test]
    fn invalid_completion_batches_are_rejected_without_state_changes() {
        let mut scheduler = RuntimeScheduler::default();
        let owner = task("asset.bg", TaskPolicy::JoinSameKey, 0);
        scheduler.submit([owner.clone()]).unwrap();
        let unknown = task("unknown", TaskPolicy::AlwaysStart, 0);
        let before = scheduler.clone();
        assert_eq!(
            scheduler.complete([
                ready(&owner, 1),
                event(&unknown, 1, TaskEventKind::Cancelled)
            ]),
            Err(TaskCompletionError::UnknownTask {
                task_id: unknown.task_id()
            })
        );
        assert_eq!(scheduler, before);
        assert_eq!(
            scheduler.complete([ready(&owner, 1), event(&owner, 2, TaskEventKind::Cancelled)]),
            Err(TaskCompletionError::DuplicateTerminalEvent {
                task_id: owner.task_id()
            })
        );
        assert_eq!(scheduler, before);
    }
    #[test]
    fn terminal_task_rejects_later_completion() {
        let mut scheduler = RuntimeScheduler::default();
        let owner = task("done", TaskPolicy::AlwaysStart, 0);
        scheduler.submit([owner.clone()]).unwrap();
        scheduler.complete([ready(&owner, 1)]).unwrap();
        let before = scheduler.clone();
        assert_eq!(
            scheduler.complete([event(&owner, 2, TaskEventKind::Cancelled)]),
            Err(TaskCompletionError::DuplicateTerminalEvent {
                task_id: owner.task_id()
            })
        );
        assert_eq!(scheduler, before);
        assert_eq!(
            scheduler.complete([progress(&owner, 2)]),
            Err(TaskCompletionError::EventAfterTerminal {
                task_id: owner.task_id()
            })
        );
        assert_eq!(scheduler, before);
    }
    #[test]
    fn shuffled_progress_and_ready_revisions_are_normalized_before_commit() {
        let mut scheduler = RuntimeScheduler::default();
        let owner = task("owner", TaskPolicy::AlwaysStart, 0);
        scheduler.submit([owner.clone()]).unwrap();
        let events = scheduler
            .complete([ready(&owner, 3), progress(&owner, 2), progress(&owner, 1)])
            .unwrap();
        assert_eq!(
            events
                .iter()
                .map(|e| e.cursor.sequence.0)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(scheduler.stats().completed, 1);
    }
    #[test]
    fn repeated_or_mismatched_publications_reject_atomically() {
        let mut scheduler = RuntimeScheduler::default();
        let owner = task("owner", TaskPolicy::AlwaysStart, 0);
        scheduler.submit([owner.clone()]).unwrap();
        scheduler.complete([progress(&owner, 2)]).unwrap();
        let before = scheduler.clone();
        assert_eq!(
            scheduler.complete([progress(&owner, 2)]),
            Err(TaskCompletionError::StalePublication {
                task_id: owner.task_id()
            })
        );
        assert_eq!(scheduler, before);
        let mut mismatched = ready(&owner, 3);
        mismatched.correlation.generation = GenerationId::new(2);
        assert_eq!(
            scheduler.complete([mismatched]),
            Err(TaskCompletionError::DispatchMismatch {
                task_id: owner.task_id()
            })
        );
        assert_eq!(scheduler, before);
        assert_eq!(
            scheduler.complete([progress(&owner, u64::MAX)]),
            Err(TaskCompletionError::PublicationRevisionExhausted {
                task_id: owner.task_id()
            })
        );
        assert_eq!(scheduler, before);
    }
    #[test]
    fn cancellation_requests_are_dispatched_once() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler.cancel_scope(CancelScopeId("flow".into()));
        scheduler.cancel_scope(CancelScopeId("flow".into()));
        assert_eq!(
            scheduler
                .dispatch(SchedulerBudget { max_events: 0 })
                .cancel_scopes,
            [CancelScopeId("flow".into())]
        );
        assert_eq!(scheduler.stats().cancel_requested, 1);
    }
}
