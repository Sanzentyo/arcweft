//! Sans I/O runtime task scheduler.
//!
//! The scheduler owns deterministic task submission, key-based joining,
//! cancellation bookkeeping, and dispatch ordering. Host adapters still own
//! actual I/O, worker pools, clocks, and OS integration.

use arcweft_core::task::{
    CancelScopeId, SchedulerBudget, TaskClass, TaskEvent, TaskEventKind, TaskId, TaskKey,
    TaskPolicy, TaskSpec, compare_task_events, task_events_are_normalized,
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
    joined_waiters: BTreeMap<TaskId, Vec<TaskId>>,
    cancel_scopes: BTreeSet<CancelScopeId>,
    stats: RuntimeSchedulerStats,
}

/// Tasks and cancellations ready for a host adapter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchedulerDispatchBatch {
    pub tasks: Vec<TaskSpec>,
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
    pub local_ui: usize,
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
            local_ui: 0,
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
            TaskClass::LocalUi => self.local_ui += 1,
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
    spec: TaskSpec,
    order: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct InFlightTask {
    key: TaskKey,
    class: TaskClass,
    policy: TaskPolicy,
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
            joined_waiters: BTreeMap::new(),
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

    /// Submits runtime tasks and joins same-key work according to policy.
    pub fn submit(&mut self, tasks: impl IntoIterator<Item = TaskSpec>) {
        for spec in tasks {
            self.submit_one(spec);
        }
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
            self.stats.dispatched_by_class.record(&task.class);
        }
        SchedulerDispatchBatch {
            tasks,
            cancel_scopes: std::mem::take(&mut self.cancel_scopes)
                .into_iter()
                .collect(),
        }
    }

    /// Completes in-flight tasks and returns replay-normalized task events.
    pub fn complete(&mut self, events: impl IntoIterator<Item = TaskEvent>) -> Vec<TaskEvent> {
        let mut events = events.into_iter().collect::<Vec<_>>();
        self.stats.completion_events_in += events.len();
        self.normalize_completion_events(&mut events);
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
        events
    }

    /// Returns current cumulative scheduler counters.
    pub fn stats(&self) -> RuntimeSchedulerStats {
        let mut stats = self.stats;
        stats.in_flight = self.in_flight.len();
        stats
    }

    fn submit_one(&mut self, spec: TaskSpec) {
        if spec.policy == TaskPolicy::JoinSameKey
            && let Some(owner) = self.in_flight_by_key.get(&spec.key)
        {
            self.stats.joined += 1;
            self.joined_waiters
                .entry(owner.clone())
                .or_default()
                .push(spec.id);
            return;
        }
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        self.track_in_flight(&spec);
        self.stats.submitted_by_class.record(&spec.class);
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

    fn track_in_flight(&mut self, spec: &TaskSpec) {
        self.in_flight.insert(
            spec.id.clone(),
            InFlightTask {
                key: spec.key.clone(),
                class: spec.class.clone(),
                policy: spec.policy,
            },
        );
        if spec.policy == TaskPolicy::JoinSameKey {
            self.in_flight_by_key
                .insert(spec.key.clone(), spec.id.clone());
        }
    }

    fn complete_one(&mut self, event: &TaskEvent) -> Vec<TaskEvent> {
        let Some(task) = self.in_flight.remove(&event.task_id) else {
            return Vec::new();
        };
        if task.policy == TaskPolicy::JoinSameKey {
            self.in_flight_by_key.remove(&task.key);
        }
        self.stats.completed_by_class.record(&task.class);
        match event.kind {
            TaskEventKind::Ready(_) | TaskEventKind::Progress(_) => {
                self.stats.completed += 1;
            }
            TaskEventKind::Err(_) => {
                self.stats.failed += 1;
            }
            TaskEventKind::Cancelled => {
                self.stats.cancelled += 1;
            }
        }
        self.complete_joined_waiters(event)
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
                task_id,
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
        .priority
        .cmp(&left.spec.priority)
        .then_with(|| left.order.cmp(&right.order))
        .then_with(|| left.spec.id.cmp(&right.spec.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::task::{
        FileReadTextRequest, HostTaskRequest, LogicalEpoch, TaskPriority, TaskSequence,
    };
    use arcweft_core::value::RuntimePayload;

    #[test]
    fn joins_same_key_in_flight_tasks() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler.submit([task("a", "asset.bg", TaskPolicy::JoinSameKey, 0)]);
        scheduler.submit([task("b", "asset.bg", TaskPolicy::JoinSameKey, 0)]);

        let batch = scheduler.dispatch(SchedulerBudget { max_events: 8 });

        assert_eq!(batch.tasks.len(), 1);
        assert_eq!(batch.tasks[0].id, TaskId("a".to_owned()));
        assert_eq!(scheduler.stats().submitted, 1);
        assert_eq!(scheduler.stats().joined, 1);
        assert_eq!(scheduler.stats().in_flight, 1);
        assert_eq!(scheduler.stats().dispatch_sorts, 0);
    }

    #[test]
    fn joined_tasks_receive_owner_completion_events() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler.submit([task("owner", "asset.bg", TaskPolicy::JoinSameKey, 0)]);
        scheduler.dispatch(SchedulerBudget { max_events: 8 });
        scheduler.submit([
            task("waiter-a", "asset.bg", TaskPolicy::JoinSameKey, 0),
            task("waiter-b", "asset.bg", TaskPolicy::JoinSameKey, 0),
        ]);

        let events = scheduler.complete([event(
            "owner",
            1,
            TaskEventKind::Ready(RuntimePayload::from("shared")),
        )]);
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
        scheduler.submit([
            task("a", "asset.bg", TaskPolicy::AlwaysStart, 0),
            task("b", "asset.bg", TaskPolicy::AlwaysStart, 0),
        ]);

        let batch = scheduler.dispatch(SchedulerBudget { max_events: 8 });

        assert_eq!(batch.tasks.len(), 2);
        assert_eq!(scheduler.stats().joined, 0);
        assert_eq!(scheduler.stats().max_in_flight, 2);
    }

    #[test]
    fn dispatches_by_priority_then_submission_order() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler.submit([
            task("low", "low", TaskPolicy::AlwaysStart, 1),
            task("high-a", "high-a", TaskPolicy::AlwaysStart, 9),
            task("high-b", "high-b", TaskPolicy::AlwaysStart, 9),
        ]);

        let batch = scheduler.dispatch(SchedulerBudget { max_events: 2 });

        let ids = batch
            .tasks
            .iter()
            .map(|task| task.id.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["high-a", "high-b"]);
        assert_eq!(scheduler.stats().dispatched, 2);
        assert_eq!(scheduler.stats().dispatch_sorts, 1);
        assert_eq!(scheduler.stats().dispatch_sort_items, 3);
    }

    #[test]
    fn dispatch_avoids_sort_when_submissions_are_already_ordered() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler.submit([
            task("high-a", "high-a", TaskPolicy::AlwaysStart, 9),
            task("high-b", "high-b", TaskPolicy::AlwaysStart, 9),
            task("low", "low", TaskPolicy::AlwaysStart, 1),
        ]);

        let batch = scheduler.dispatch(SchedulerBudget { max_events: 8 });

        let ids = batch
            .tasks
            .iter()
            .map(|task| task.id.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["high-a", "high-b", "low"]);
        assert_eq!(scheduler.stats().dispatch_sorts, 0);
        assert_eq!(scheduler.stats().dispatch_sort_items, 0);
    }

    #[test]
    fn completion_updates_stats_and_normalizes_events() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler.submit([
            task("a", "a", TaskPolicy::AlwaysStart, 0),
            task("b", "b", TaskPolicy::AlwaysStart, 0),
        ]);
        scheduler.dispatch(SchedulerBudget { max_events: 8 });

        let events = scheduler.complete([
            event("b", 2, TaskEventKind::Err("failed".to_owned())),
            event("a", 1, TaskEventKind::Ready(RuntimePayload::from("ok"))),
        ]);

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
    fn cancellation_requests_are_dispatched_once() {
        let mut scheduler = RuntimeScheduler::default();
        scheduler.cancel_scope(CancelScopeId("flow".to_owned()));
        scheduler.cancel_scope(CancelScopeId("flow".to_owned()));

        let batch = scheduler.dispatch(SchedulerBudget { max_events: 0 });

        assert_eq!(batch.cancel_scopes, [CancelScopeId("flow".to_owned())]);
        assert_eq!(scheduler.stats().cancel_requested, 1);
    }

    fn task(id: &str, key: &str, policy: TaskPolicy, priority: i32) -> TaskSpec {
        TaskSpec::new(
            TaskId(id.to_owned()),
            TaskKey(key.to_owned()),
            HostTaskRequest::FileReadText(FileReadTextRequest {
                path: "save:test.txt".to_owned(),
            })
            .task_class(),
            TaskPriority(priority),
            CancelScopeId("test".to_owned()),
            policy,
            HostTaskRequest::FileReadText(FileReadTextRequest {
                path: "save:test.txt".to_owned(),
            }),
        )
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
