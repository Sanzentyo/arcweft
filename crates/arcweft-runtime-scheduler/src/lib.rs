//! Sans I/O runtime task scheduler.
//!
//! The scheduler owns deterministic task submission, key-based joining,
//! cancellation bookkeeping, and dispatch ordering. Host adapters still own
//! actual I/O, worker pools, clocks, and OS integration.

use arcweft_core::task::{
    BoundTaskSpec, CancelScopeId, SchedulerBudget, TaskClass, TaskCompletionError, TaskEnsureError,
    TaskEvent, TaskEventKind, TaskId, TaskKey, TaskPolicy, compare_task_events,
    task_events_are_normalized,
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
    in_flight_by_key: BTreeMap<TaskKey, TaskId>,
    accepted_specs: BTreeMap<TaskId, BoundTaskSpec>,
    joined_waiters: BTreeMap<TaskId, Vec<TaskId>>,
    joined_waiter_owners: BTreeMap<TaskId, TaskId>,
    terminal_task_ids: BTreeSet<TaskId>,
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
    pub joined_completed: usize,
    pub in_flight: usize,
    pub max_in_flight: usize,
    pub dispatch_sorts: usize,
    pub dispatch_sort_items: usize,
    pub completion_sorts: usize,
    pub completion_sort_items: usize,
    pub completion_normalization_passes: usize,
    pub completion_normalization_checks: usize,
    pub completion_events_in: usize,
    pub completion_events_joined: usize,
    pub completion_events_out: usize,
    pub completion_sort_skipped_items: usize,
    pub completion_sort_performed_items: usize,
    pub joined_completion_events_emitted: usize,
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
    key: TaskKey,
    class: TaskClass,
    policy: TaskPolicy,
}

enum Submission {
    Launch(BoundTaskSpec),
    Join {
        owner_id: TaskId,
        spec: BoundTaskSpec,
    },
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
            in_flight_by_key: BTreeMap::new(),
            accepted_specs: BTreeMap::new(),
            joined_waiters: BTreeMap::new(),
            joined_waiter_owners: BTreeMap::new(),
            terminal_task_ids: BTreeSet::new(),
            cancel_scopes: BTreeSet::new(),
            stats: RuntimeSchedulerStats {
                submitted: 0,
                joined: 0,
                dispatched: 0,
                completed: 0,
                failed: 0,
                cancelled: 0,
                cancel_requested: 0,
                joined_completed: 0,
                in_flight: 0,
                max_in_flight: 0,
                dispatch_sorts: 0,
                dispatch_sort_items: 0,
                completion_sorts: 0,
                completion_sort_items: 0,
                completion_normalization_passes: 0,
                completion_normalization_checks: 0,
                completion_events_in: 0,
                completion_events_joined: 0,
                completion_events_out: 0,
                completion_sort_skipped_items: 0,
                completion_sort_performed_items: 0,
                joined_completion_events_emitted: 0,
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
        let mut staged = Vec::new();
        let mut staged_by_id = BTreeMap::new();
        let mut staged_owners_by_key = BTreeMap::new();

        for spec in tasks {
            let task_id = &spec.spec().id;
            if let Some(existing) = staged_by_id
                .get(task_id)
                .or_else(|| self.accepted_specs.get(task_id))
            {
                if existing.same_identity_spec(&spec) {
                    continue;
                }
                return Err(TaskEnsureError::TaskIdSpecificationConflict {
                    task_id: task_id.clone(),
                });
            }

            let task_spec = spec.spec();
            let owner = staged_owners_by_key
                .get(&task_spec.key)
                .or_else(|| self.in_flight_by_key.get(&task_spec.key));
            if task_spec.policy == TaskPolicy::JoinSameKey
                && let Some(owner_id) = owner
            {
                let owner_spec = staged_by_id
                    .get(owner_id)
                    .or_else(|| self.accepted_specs.get(owner_id))
                    .expect("same-key owner has an accepted specification");
                if !owner_spec.same_join_contract(&spec) {
                    return Err(TaskEnsureError::JoinSpecificationConflict {
                        task_id: task_spec.id.clone(),
                        owner_id: owner_id.clone(),
                        key: task_spec.key.clone(),
                    });
                }
                staged_by_id.insert(task_spec.id.clone(), spec.clone());
                staged.push(Submission::Join {
                    owner_id: owner_id.clone(),
                    spec,
                });
            } else {
                staged_by_id.insert(task_spec.id.clone(), spec.clone());
                if task_spec.policy == TaskPolicy::JoinSameKey {
                    staged_owners_by_key.insert(task_spec.key.clone(), task_spec.id.clone());
                }
                staged.push(Submission::Launch(spec));
            }
        }

        for submission in staged {
            match submission {
                Submission::Launch(spec) => self.launch(spec),
                Submission::Join { owner_id, spec } => self.join(owner_id, spec),
            }
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

        let mut joined_events = None;
        for event in &events {
            let completed = self.complete_one(event);
            if !completed.is_empty() {
                joined_events.get_or_insert_with(Vec::new).extend(completed);
            }
        }
        if let Some(joined_events) = joined_events {
            self.stats.completion_events_joined += joined_events.len();
            events.extend(joined_events);
            self.normalize_completion_events(&mut events);
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
        self.accepted_specs
            .insert(task_spec.id.clone(), spec.clone());
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

    fn join(&mut self, owner_id: TaskId, spec: BoundTaskSpec) {
        let task_id = spec.spec().id.clone();
        self.accepted_specs.insert(task_id.clone(), spec);
        self.joined_waiters
            .entry(owner_id.clone())
            .or_default()
            .push(task_id.clone());
        self.joined_waiter_owners.insert(task_id, owner_id);
        self.stats.joined += 1;
    }

    fn track_in_flight(&mut self, spec: &BoundTaskSpec) {
        let task_spec = spec.spec();
        self.in_flight.insert(
            task_spec.id.clone(),
            InFlightTask {
                key: task_spec.key.clone(),
                class: task_spec.class.clone(),
                policy: task_spec.policy,
            },
        );
        if task_spec.policy == TaskPolicy::JoinSameKey {
            self.in_flight_by_key
                .insert(task_spec.key.clone(), task_spec.id.clone());
        }
    }

    fn complete_one(&mut self, event: &TaskEvent) -> Vec<TaskEvent> {
        if matches!(event.kind, TaskEventKind::Progress(_)) {
            return self.progress_joined_waiters(event);
        }
        let task = self
            .in_flight
            .remove(&event.task_id)
            .expect("completion preflight retained every task owner");
        if task.policy == TaskPolicy::JoinSameKey
            && self.in_flight_by_key.get(&task.key) == Some(&event.task_id)
        {
            self.in_flight_by_key.remove(&task.key);
        }
        self.stats.completed_by_class.record(&task.class);
        match event.kind {
            TaskEventKind::Ready(_) => self.stats.completed += 1,
            TaskEventKind::Failed(_) => {
                self.stats.failed += 1;
            }
            TaskEventKind::Cancelled => {
                self.stats.cancelled += 1;
            }
            TaskEventKind::Progress(_) => unreachable!("progress is nonterminal"),
        }
        self.terminal_task_ids.insert(event.task_id.clone());
        self.complete_joined_waiters(event)
    }

    fn preflight_completions(&self, events: &[TaskEvent]) -> Result<(), TaskCompletionError> {
        let mut ordered = events.to_vec();
        ordered.sort_by(compare_task_events);
        let mut terminal_in_batch = BTreeSet::new();

        for event in &ordered {
            if let Some(owner_id) = self.joined_waiter_owners.get(&event.task_id) {
                return Err(TaskCompletionError::JoinedWaiterDirectCompletion {
                    task_id: event.task_id.clone(),
                    owner_id: owner_id.clone(),
                });
            }

            if self.terminal_task_ids.contains(&event.task_id)
                || terminal_in_batch.contains(&event.task_id)
            {
                return Err(if is_terminal_event(&event.kind) {
                    TaskCompletionError::DuplicateTerminalEvent {
                        task_id: event.task_id.clone(),
                    }
                } else {
                    TaskCompletionError::EventAfterTerminal {
                        task_id: event.task_id.clone(),
                    }
                });
            }

            if !self.in_flight.contains_key(&event.task_id) {
                return Err(TaskCompletionError::UnknownTask {
                    task_id: event.task_id.clone(),
                });
            }

            if is_terminal_event(&event.kind) {
                terminal_in_batch.insert(event.task_id.clone());
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
            .get(&event.task_id)
            .expect("completion preflight retained every accepted task");
        match spec.outcome().try_payload(payload.clone().into_value()) {
            Ok(admitted) => event.kind = TaskEventKind::Ready(admitted),
            Err(error) => event.kind = TaskEventKind::Failed(error.to_string()),
        }
    }

    fn progress_joined_waiters(&self, event: &TaskEvent) -> Vec<TaskEvent> {
        self.joined_waiters
            .get(&event.task_id)
            .into_iter()
            .flatten()
            .cloned()
            .map(|task_id| TaskEvent {
                logical_epoch: event.logical_epoch,
                task_id,
                sequence: event.sequence,
                kind: event.kind.clone(),
            })
            .collect()
    }

    fn complete_joined_waiters(&mut self, event: &TaskEvent) -> Vec<TaskEvent> {
        let Some(waiters) = self.joined_waiters.remove(&event.task_id) else {
            return Vec::new();
        };
        self.stats.joined_completed += waiters.len();
        self.stats.joined_completion_events_emitted += waiters.len();
        waiters
            .into_iter()
            .map(|task_id| TaskEvent {
                logical_epoch: event.logical_epoch,
                task_id: {
                    self.joined_waiter_owners.remove(&task_id);
                    self.terminal_task_ids.insert(task_id.clone());
                    task_id
                },
                sequence: event.sequence,
                kind: event.kind.clone(),
            })
            .collect()
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
        TaskEventKind::Ready(_) | TaskEventKind::Failed(_) | TaskEventKind::Cancelled
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
        .then_with(|| left.spec.spec().id.cmp(&right.spec.spec().id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::{
        entry::RuntimeSchemaLimits,
        pattern::RuntimeCheckedType,
        task::{
            FileReadTextRequest, HostTaskRequest, LogicalEpoch, TaskOutcomeContract, TaskPriority,
            TaskSequence, TaskSpec,
        },
        value::{RuntimePayload, RuntimeValue},
    };

    #[test]
    fn joins_same_key_in_flight_tasks() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([task("a", "asset.bg", TaskPolicy::JoinSameKey, 0)])
            .expect("first task is valid");
        scheduler
            .submit([task("b", "asset.bg", TaskPolicy::JoinSameKey, 0)])
            .expect("same-contract task joins");

        let batch = scheduler.dispatch(SchedulerBudget { max_events: 8 });

        assert_eq!(batch.tasks.len(), 1);
        assert_eq!(batch.tasks[0].spec().id, TaskId("a".to_owned()));
        assert_eq!(scheduler.stats().submitted, 1);
        assert_eq!(scheduler.stats().joined, 1);
        assert_eq!(scheduler.stats().in_flight, 1);
        assert_eq!(scheduler.stats().dispatch_sorts, 0);
    }

    #[test]
    fn joined_tasks_receive_owner_completion_events() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([task("owner", "asset.bg", TaskPolicy::JoinSameKey, 0)])
            .expect("owner is valid");
        scheduler.dispatch(SchedulerBudget { max_events: 8 });
        scheduler
            .submit([
                task("waiter-a", "asset.bg", TaskPolicy::JoinSameKey, 0),
                task("waiter-b", "asset.bg", TaskPolicy::JoinSameKey, 0),
            ])
            .expect("waiters share the owner's contract");

        let events = scheduler
            .complete([event(
                "owner",
                1,
                TaskEventKind::Ready(RuntimePayload::from("shared")),
            )])
            .expect("owner can publish a string");
        let ids = events
            .iter()
            .map(|event| event.task_id.0.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, ["owner", "waiter-a", "waiter-b"]);
        assert!(events.iter().all(|event| {
            matches!(&event.kind, TaskEventKind::Ready(value) if value.label() == "shared")
        }));
        assert_eq!(scheduler.stats().completed, 1);
        assert_eq!(scheduler.stats().joined, 2);
        assert_eq!(scheduler.stats().joined_completed, 2);
        assert_eq!(scheduler.stats().joined_completion_events_emitted, 2);
        assert_eq!(scheduler.stats().completion_events_in, 1);
        assert_eq!(scheduler.stats().completion_events_joined, 2);
        assert_eq!(scheduler.stats().completion_events_out, 3);
        assert_eq!(scheduler.stats().completion_normalization_passes, 2);
        assert_eq!(scheduler.stats().completion_normalization_checks, 1);
        assert_eq!(scheduler.stats().completion_sort_skipped_items, 3);
        assert_eq!(scheduler.stats().in_flight, 0);
        assert_eq!(scheduler.stats().completion_sorts, 0);
    }

    #[test]
    fn always_start_does_not_join_same_key_tasks() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([
                task("a", "asset.bg", TaskPolicy::AlwaysStart, 0),
                task("b", "asset.bg", TaskPolicy::AlwaysStart, 0),
            ])
            .expect("always-start tasks do not join");

        let batch = scheduler.dispatch(SchedulerBudget { max_events: 8 });

        assert_eq!(batch.tasks.len(), 2);
        assert_eq!(scheduler.stats().joined, 0);
        assert_eq!(scheduler.stats().max_in_flight, 2);
    }

    #[test]
    fn dispatches_by_priority_then_submission_order() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([
                task("low", "low", TaskPolicy::AlwaysStart, 1),
                task("high-a", "high-a", TaskPolicy::AlwaysStart, 9),
                task("high-b", "high-b", TaskPolicy::AlwaysStart, 9),
            ])
            .expect("tasks have distinct keys");

        let batch = scheduler.dispatch(SchedulerBudget { max_events: 2 });

        let ids = batch
            .tasks
            .iter()
            .map(|task| task.spec().id.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["high-a", "high-b"]);
        assert_eq!(scheduler.stats().dispatched, 2);
        assert_eq!(scheduler.stats().dispatch_sorts, 1);
        assert_eq!(scheduler.stats().dispatch_sort_items, 3);
    }

    #[test]
    fn dispatch_avoids_sort_when_submissions_are_already_ordered() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([
                task("high-a", "high-a", TaskPolicy::AlwaysStart, 9),
                task("high-b", "high-b", TaskPolicy::AlwaysStart, 9),
                task("low", "low", TaskPolicy::AlwaysStart, 1),
            ])
            .expect("tasks have distinct keys");

        let batch = scheduler.dispatch(SchedulerBudget { max_events: 8 });

        let ids = batch
            .tasks
            .iter()
            .map(|task| task.spec().id.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["high-a", "high-b", "low"]);
        assert_eq!(scheduler.stats().dispatch_sorts, 0);
        assert_eq!(scheduler.stats().dispatch_sort_items, 0);
    }

    #[test]
    fn completion_updates_stats_and_normalizes_events() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([
                task("a", "a", TaskPolicy::AlwaysStart, 0),
                task("b", "b", TaskPolicy::AlwaysStart, 0),
            ])
            .expect("tasks have distinct keys");
        scheduler.dispatch(SchedulerBudget { max_events: 8 });

        let events = scheduler
            .complete([
                event("b", 2, TaskEventKind::Failed("failed".to_owned())),
                event("a", 1, TaskEventKind::Ready(RuntimePayload::from("ok"))),
            ])
            .expect("events belong to distinct owners");

        assert_eq!(events[0].task_id, TaskId("a".to_owned()));
        assert_eq!(scheduler.stats().completed, 1);
        assert_eq!(scheduler.stats().failed, 1);
        assert_eq!(scheduler.stats().in_flight, 0);
        assert_eq!(scheduler.stats().completion_sorts, 1);
        assert_eq!(scheduler.stats().completion_sort_items, 2);
        assert_eq!(scheduler.stats().completion_normalization_passes, 1);
        assert_eq!(scheduler.stats().completion_normalization_checks, 1);
        assert_eq!(scheduler.stats().completion_events_in, 2);
        assert_eq!(scheduler.stats().completion_events_out, 2);
        assert_eq!(scheduler.stats().completion_sort_performed_items, 2);
        assert_eq!(scheduler.stats().completion_sort_skipped_items, 0);
    }

    #[test]
    fn progress_keeps_joined_work_in_flight_until_terminal_delivery() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([task("owner", "asset.bg", TaskPolicy::JoinSameKey, 0)])
            .expect("owner is valid");
        scheduler.dispatch(SchedulerBudget { max_events: 8 });
        scheduler
            .submit([task("waiter-a", "asset.bg", TaskPolicy::JoinSameKey, 0)])
            .expect("waiter shares the owner's contract");

        let progress = scheduler
            .complete([event(
                "owner",
                1,
                TaskEventKind::Progress(
                    arcweft_core::value::Progress::new(0.5)
                        .expect("fixture progress is valid")
                        .with_label("halfway"),
                ),
            )])
            .expect("progress belongs to the owner");

        assert_eq!(
            progress
                .iter()
                .map(|event| event.task_id.0.as_str())
                .collect::<Vec<_>>(),
            ["owner", "waiter-a"]
        );
        assert!(progress.iter().all(|event| {
            matches!(&event.kind, TaskEventKind::Progress(value) if value.label() == Some("halfway"))
        }));
        assert_eq!(scheduler.stats().in_flight, 1);
        assert_eq!(scheduler.stats().completed, 0);
        assert_eq!(
            scheduler.stats().completed_by_class,
            TaskClassCounts::default()
        );
        assert_eq!(scheduler.stats().joined_completed, 0);
        assert_eq!(scheduler.stats().joined_completion_events_emitted, 0);

        scheduler
            .submit([task("waiter-b", "asset.bg", TaskPolicy::JoinSameKey, 0)])
            .expect("second waiter shares the owner's contract");
        let terminal = scheduler
            .complete([event(
                "owner",
                2,
                TaskEventKind::Ready(RuntimePayload::from("done")),
            )])
            .expect("owner can publish a string");

        assert_eq!(
            terminal
                .iter()
                .map(|event| event.task_id.0.as_str())
                .collect::<Vec<_>>(),
            ["owner", "waiter-a", "waiter-b"]
        );
        assert_eq!(scheduler.stats().in_flight, 0);
        assert_eq!(scheduler.stats().completed, 1);
        assert_eq!(scheduler.stats().completed_by_class.io, 1);
        assert_eq!(scheduler.stats().joined, 2);
        assert_eq!(scheduler.stats().joined_completed, 2);
        assert_eq!(scheduler.stats().joined_completion_events_emitted, 2);
    }

    #[test]
    fn join_conflicts_reject_the_whole_submission_batch() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([task("owner", "asset.bg", TaskPolicy::JoinSameKey, 0)])
            .expect("owner is valid");
        let before = scheduler.clone();
        let error = scheduler.submit([
            task("valid-waiter", "asset.bg", TaskPolicy::JoinSameKey, 0),
            task("conflicting-waiter", "asset.bg", TaskPolicy::JoinSameKey, 1),
        ]);

        assert_eq!(
            error,
            Err(TaskEnsureError::JoinSpecificationConflict {
                task_id: TaskId("conflicting-waiter".to_owned()),
                owner_id: TaskId("owner".to_owned()),
                key: TaskKey("asset.bg".to_owned()),
            })
        );
        assert_eq!(scheduler, before);
    }

    #[test]
    fn same_key_join_compares_all_non_identity_task_semantics() {
        let base = task("owner", "asset.bg", TaskPolicy::JoinSameKey, 0);
        let mut same_spec = base.spec().clone();
        same_spec.id = TaskId("waiter".to_owned());
        same_spec.debug_label = "diagnostic-only difference".to_owned();
        assert!(base.same_join_contract(&bind(same_spec)));

        let mut different_request = base.spec().clone();
        different_request.request = HostTaskRequest::FileReadText(FileReadTextRequest {
            path: "save:other.txt".to_owned(),
        });
        assert!(!base.same_join_contract(&bind(different_request)));

        let mut different_class = base.spec().clone();
        different_class.class = TaskClass::Cpu;
        assert!(!base.same_join_contract(&bind(different_class)));

        let mut different_priority = base.spec().clone();
        different_priority.priority = TaskPriority(1);
        assert!(!base.same_join_contract(&bind(different_priority)));

        let mut different_scope = base.spec().clone();
        different_scope.cancel_scope = CancelScopeId("other".to_owned());
        assert!(!base.same_join_contract(&bind(different_scope)));

        let mut different_policy = base.spec().clone();
        different_policy.policy = TaskPolicy::AlwaysStart;
        assert!(!base.same_join_contract(&bind(different_policy)));

        let mut different_key = base.spec().clone();
        different_key.key = TaskKey("other-key".to_owned());
        assert!(!base.same_join_contract(&bind(different_key)));

        let mut different_outcome = base.spec().clone();
        different_outcome.outcome = TaskOutcomeContract::new(RuntimeCheckedType::Bool);
        assert!(!base.same_join_contract(&bind(different_outcome)));
    }

    #[test]
    fn task_id_resubmission_is_idempotent_except_for_contract_changes() {
        let mut scheduler = RuntimeScheduler::default();
        let original = task("same", "asset.bg", TaskPolicy::JoinSameKey, 0);
        scheduler
            .submit([original.clone()])
            .expect("first submission is valid");
        let before = scheduler.clone();

        let mut same_id_different_label = original.spec().clone();
        same_id_different_label.debug_label = "alternate diagnostic".to_owned();
        scheduler
            .submit([bind(same_id_different_label)])
            .expect("diagnostic labels do not change identity");
        assert_eq!(scheduler, before);

        assert_eq!(
            scheduler.submit([task("same", "asset.bg", TaskPolicy::JoinSameKey, 1)]),
            Err(TaskEnsureError::TaskIdSpecificationConflict {
                task_id: TaskId("same".to_owned())
            })
        );
        assert_eq!(scheduler, before);
    }

    #[test]
    fn wrong_ready_payload_becomes_failed_before_join_fanout() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([
                task("owner", "asset.bg", TaskPolicy::JoinSameKey, 0),
                task("waiter", "asset.bg", TaskPolicy::JoinSameKey, 0),
            ])
            .expect("same contract tasks join");

        let events = scheduler
            .complete([event(
                "owner",
                1,
                TaskEventKind::Ready(RuntimePayload::from(RuntimeValue::Bool(true))),
            )])
            .expect("a wrong value is reported as a failed task");

        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|event| {
            matches!(&event.kind, TaskEventKind::Failed(message) if message.contains("rejected"))
        }));
        assert_eq!(scheduler.stats().failed, 1);
        assert_eq!(scheduler.stats().completed, 0);
        assert_eq!(scheduler.stats().joined_completed, 1);
    }

    #[test]
    fn invalid_completion_batches_are_rejected_without_state_changes() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([
                task("owner", "asset.bg", TaskPolicy::JoinSameKey, 0),
                task("waiter", "asset.bg", TaskPolicy::JoinSameKey, 0),
            ])
            .expect("same contract tasks join");

        let before = scheduler.clone();
        assert_eq!(
            scheduler.complete([event(
                "waiter",
                1,
                TaskEventKind::Ready(RuntimePayload::from("direct")),
            )]),
            Err(TaskCompletionError::JoinedWaiterDirectCompletion {
                task_id: TaskId("waiter".to_owned()),
                owner_id: TaskId("owner".to_owned()),
            })
        );
        assert_eq!(scheduler, before);

        assert_eq!(
            scheduler.complete([
                event(
                    "owner",
                    1,
                    TaskEventKind::Ready(RuntimePayload::from("valid")),
                ),
                event("unknown", 1, TaskEventKind::Cancelled),
            ]),
            Err(TaskCompletionError::UnknownTask {
                task_id: TaskId("unknown".to_owned()),
            })
        );
        assert_eq!(scheduler, before);

        assert_eq!(
            scheduler.complete([
                event(
                    "owner",
                    1,
                    TaskEventKind::Ready(RuntimePayload::from("first")),
                ),
                event("owner", 2, TaskEventKind::Cancelled),
            ]),
            Err(TaskCompletionError::DuplicateTerminalEvent {
                task_id: TaskId("owner".to_owned()),
            })
        );
        assert_eq!(scheduler, before);
    }

    #[test]
    fn terminal_task_rejects_later_completion() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler
            .submit([task("done", "asset.bg", TaskPolicy::AlwaysStart, 0)])
            .expect("task is valid");
        scheduler
            .complete([event(
                "done",
                1,
                TaskEventKind::Ready(RuntimePayload::from("ok")),
            )])
            .expect("first terminal event is accepted");
        let before = scheduler.clone();

        assert_eq!(
            scheduler.complete([event("done", 2, TaskEventKind::Cancelled)]),
            Err(TaskCompletionError::DuplicateTerminalEvent {
                task_id: TaskId("done".to_owned()),
            })
        );
        assert_eq!(scheduler, before);
    }

    #[test]
    fn cancellation_requests_are_dispatched_once() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler.cancel_scope(CancelScopeId("flow".to_owned()));
        scheduler.cancel_scope(CancelScopeId("flow".to_owned()));

        let batch = scheduler.dispatch(SchedulerBudget { max_events: 0 });

        assert_eq!(batch.cancel_scopes, [CancelScopeId("flow".to_owned())]);
        assert_eq!(scheduler.stats().cancel_requested, 1);
    }

    fn task(id: &str, key: &str, policy: TaskPolicy, priority: i32) -> BoundTaskSpec {
        bind(task_spec(id, key, policy, priority))
    }

    fn task_spec(id: &str, key: &str, policy: TaskPolicy, priority: i32) -> TaskSpec {
        let request = HostTaskRequest::FileReadText(FileReadTextRequest {
            path: "save:test.txt".to_owned(),
        });
        TaskSpec::new(
            TaskId(id.to_owned()),
            TaskKey(key.to_owned()),
            request.task_class(),
            TaskPriority(priority),
            CancelScopeId("test".to_owned()),
            policy,
            request,
        )
        .with_outcome(TaskOutcomeContract::new(RuntimeCheckedType::String))
    }

    fn bind(spec: TaskSpec) -> BoundTaskSpec {
        BoundTaskSpec::bind(spec, None, RuntimeSchemaLimits::engine_default())
            .expect("fixture outcome is bindable")
    }

    fn event(id: &str, sequence: u64, kind: TaskEventKind) -> TaskEvent {
        TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: TaskId(id.to_owned()),
            sequence: TaskSequence(sequence),
            kind,
        }
    }
}
