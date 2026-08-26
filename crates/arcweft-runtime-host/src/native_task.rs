use crate::native_system::{HostSystemInfo, host_system_info, system_info_value};
use arcweft_adapter_context::{
    manifest::{AdapterHostCall, AdapterManifest},
    standard,
};
use arcweft_core::pattern::RuntimeCheckedType;
use arcweft_core::step::{
    RuntimeHostCallError, RuntimeHostCallErrorKind, RuntimeHostCallMode, RuntimeHostCallRequest,
    RuntimeHostCallResult,
};
use arcweft_core::task::{
    CancelScopeId, HostTaskRequest, LogicalEpoch, SchedulerBudget, TaskEvent, TaskEventKind,
    TaskId, TaskKey, TaskOutcomeContract, TaskPolicy, TaskPriority, TaskSequence, TaskSpec,
};
use arcweft_core::value::{
    RuntimePayload, RuntimeValue, runtime_sequence_dense_bytes, runtime_sequence_values,
};
use arcweft_host_adapter::{
    HostAdapter, HostAdapterCompletion, HostAdapterError, HostAdapterRegistry,
    HostAdapterRegistryBuilder, HostCallPolicy, HostTaskCompletion, HostTaskMetrics,
    HostTaskOutcome, HostTaskSubmission,
};
use arcweft_runtime_scheduler::{RuntimeScheduler, RuntimeSchedulerStats, TaskClassCounts};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

pub type NativeAdapterRegistrar =
    fn(&Path, HostAdapterRegistryBuilder) -> Result<HostAdapterRegistryBuilder, HostAdapterError>;

pub const INTERNAL_SCHEDULER_ADAPTER_ID: &str = "internal-scheduler";

/// Physical roots mounted behind Arcweft's native virtual file spaces.
///
/// Authored assets are read-only and may live outside the tool-owned state
/// directory. Save, temporary, and export paths remain under `state`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFileRoots {
    asset: PathBuf,
    state: PathBuf,
}

#[derive(Clone, Debug)]
pub struct NativeTaskBridge {
    policy: HostCallPolicy,
    registry: HostAdapterRegistry,
    sequence: u64,
    scheduler: RuntimeScheduler,
    pending_host_calls: BTreeMap<TaskId, PendingRuntimeHostCall>,
    retired_host_call_tasks: BTreeSet<TaskId>,
    seen_host_calls: BTreeSet<arcweft_core::step::RuntimeHostCallId>,
    ready_host_call_results: Vec<RuntimeHostCallResult>,
    stats: NativeTaskStats,
}

#[derive(Clone, Debug)]
struct PendingRuntimeHostCall {
    id: arcweft_core::step::RuntimeHostCallId,
    result: RuntimeCheckedType,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct NativeTaskStats {
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub read_ops: usize,
    pub write_ops: usize,
    pub system_info_ops: usize,
    pub bytes_read: usize,
    pub bytes_written: usize,
    pub parallel_batches: usize,
    pub parallel_tasks: usize,
    pub parallel_io_tasks: usize,
    pub parallel_system_info_tasks: usize,
    pub parallel_marker_tasks: usize,
    pub parallel_workers: usize,
    pub scheduler_submit_elapsed_ns: u128,
    pub scheduler_dispatch_elapsed_ns: u128,
    pub host_complete_elapsed_ns: u128,
    pub event_build_elapsed_ns: u128,
    pub scheduler_complete_elapsed_ns: u128,
    pub scheduler: NativeSchedulerStats,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct NativeSchedulerStats {
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
    pub submitted_by_class: NativeTaskClassCounts,
    pub dispatched_by_class: NativeTaskClassCounts,
    pub completed_by_class: NativeTaskClassCounts,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct NativeTaskClassCounts {
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

impl NativeTaskBridge {
    pub fn try_new(
        source_path: &Path,
        file_roots: NativeFileRoots,
        cli_args: &[String],
        policy: HostCallPolicy,
        registrars: &[NativeAdapterRegistrar],
    ) -> Result<Self, HostAdapterError> {
        let registry = registry_with_registrars(source_path, file_roots, cli_args, registrars)?;
        Self::try_with_registry(policy, registry)
    }

    pub fn try_with_registry(
        policy: HostCallPolicy,
        registry: HostAdapterRegistry,
    ) -> Result<Self, HostAdapterError> {
        policy.ensure_implemented_by(&registry)?;
        Ok(Self {
            policy,
            registry,
            sequence: 0,
            scheduler: RuntimeScheduler::default(),
            pending_host_calls: BTreeMap::new(),
            retired_host_call_tasks: BTreeSet::new(),
            seen_host_calls: BTreeSet::new(),
            ready_host_call_results: Vec::new(),
            stats: NativeTaskStats::default(),
        })
    }

    pub fn standard_policy() -> HostCallPolicy {
        HostCallPolicy::from_manifests([
            standard::native_file_manifest(),
            standard::native_cli_manifest(),
            standard::system_info_manifest(),
            internal_scheduler_manifest(),
        ])
    }

    pub fn policy_from_manifest(manifest: &AdapterManifest) -> HostCallPolicy {
        HostCallPolicy::from_manifests([manifest.clone()])
    }

    pub fn standard_cli_policy_for_manifest(manifest: &AdapterManifest) -> HostCallPolicy {
        Self::standard_policy().union(Self::policy_from_manifest(manifest))
    }

    pub fn stats(&self) -> NativeTaskStats {
        let mut stats = self.stats;
        stats.scheduler = NativeSchedulerStats::from(self.scheduler.stats());
        stats
    }

    pub fn read_text_snapshot(file_roots: &NativeFileRoots, value: &str) -> Result<String, String> {
        virtual_path(file_roots, value, NativeFileAccess::Read)
            .and_then(|path| fs::read_to_string(path).map_err(|error| error.to_string()))
    }

    /// Runs adapter work that is bound to the embedding event-loop thread.
    pub fn pump_main_thread(&self) -> Result<(), HostAdapterError> {
        self.registry.pump_main_thread()
    }

    /// Converts pending adapter completions into deterministic scheduler events.
    pub fn poll_completions(&mut self) -> Vec<TaskEvent> {
        let mut completions = self.registry.drain_completions();
        completions.sort_by(|left, right| left.task_id.cmp(&right.task_id));
        let mut events = Vec::new();
        for HostAdapterCompletion { task_id, outcome } in completions {
            if let Some(pending) = self.pending_host_calls.remove(&task_id) {
                let result = self.host_call_result(pending.id, &pending.result, outcome);
                self.ready_host_call_results.push(result);
            } else if self.retired_host_call_tasks.remove(&task_id) {
                continue;
            } else {
                events.push(self.task_event(TaskCompletion {
                    task_id,
                    completion: outcome.completion,
                    stats: outcome.metrics,
                }));
            }
        }
        self.ready_host_call_results
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.scheduler.complete(events)
    }

    /// Dispatches direct runtime host calls through the same manifest-owned
    /// adapter registry as temporal tasks. Synchronous results are returned in
    /// call-identity order; suspended completions are retrieved with
    /// [`Self::take_host_call_results`] after [`Self::poll_completions`].
    pub fn complete_host_calls(
        &mut self,
        requests: Vec<RuntimeHostCallRequest>,
    ) -> Vec<RuntimeHostCallResult> {
        let mut results = requests
            .into_iter()
            .filter_map(|request| self.complete_host_call(request))
            .collect::<Vec<_>>();
        results.sort_by(|left, right| left.id.cmp(&right.id));
        results
    }

    /// Takes adapter completions for previously suspended direct host calls.
    pub fn take_host_call_results(&mut self) -> Vec<RuntimeHostCallResult> {
        std::mem::take(&mut self.ready_host_call_results)
    }

    fn complete_host_call(
        &mut self,
        request: RuntimeHostCallRequest,
    ) -> Option<RuntimeHostCallResult> {
        if !self.seen_host_calls.insert(request.id.clone()) {
            return Some(host_call_error(
                request.id,
                RuntimeHostCallErrorKind::Rejected,
                "duplicate runtime host-call identity",
            ));
        }
        let runtime_id = request.id.clone();
        let task_id = host_call_task_id(&request.id);
        let contract = request.contract;
        let host_request = HostTaskRequest::custom_with_named_args(
            request.capability,
            request.operation,
            request.args,
            request
                .named_args
                .into_iter()
                .map(|argument| (argument.name, argument.value)),
        );
        if request.public_id != host_request.host_call_id() {
            return Some(host_call_error(
                runtime_id,
                RuntimeHostCallErrorKind::Rejected,
                "host-call public identity does not match its capability and operation",
            ));
        }
        if !self.policy.allows(&host_request) {
            return Some(host_call_error(
                runtime_id,
                RuntimeHostCallErrorKind::UnsupportedCapability,
                "host call is not admitted by the active adapter manifests",
            ));
        }
        if !self.registry.contains(&host_request.host_call_id()) {
            return Some(host_call_error(
                runtime_id,
                RuntimeHostCallErrorKind::UnsupportedCapability,
                "host call has no native adapter implementation",
            ));
        }
        if contract
            != self
                .registry
                .host_call_contract(&host_request.host_call_id())
        {
            return Some(host_call_error(
                runtime_id,
                RuntimeHostCallErrorKind::Rejected,
                "host-call contract does not match the registered adapter manifest",
            ));
        }
        if !self.registry.host_call_accepts_runtime_result(
            &host_request.host_call_id(),
            request.mode,
            &request.result,
        ) {
            return Some(host_call_error(
                runtime_id,
                RuntimeHostCallErrorKind::Rejected,
                "host-call result type does not match the registered adapter manifest",
            ));
        }
        let task = TaskSpec::new(
            task_id.clone(),
            TaskKey(task_id.0.clone()),
            host_request.task_class(),
            TaskPriority(0),
            CancelScopeId("runtime-host-call".to_owned()),
            TaskPolicy::AlwaysStart,
            host_request,
        )
        .with_outcome(TaskOutcomeContract::new(request.result.clone()));
        match self.registry.submit(&task) {
            Some(HostTaskSubmission::Completed(outcome)) => {
                Some(self.host_call_result(runtime_id, &request.result, outcome))
            }
            Some(HostTaskSubmission::Pending) if request.mode == RuntimeHostCallMode::Suspend => {
                self.pending_host_calls.insert(
                    task_id,
                    PendingRuntimeHostCall {
                        id: runtime_id,
                        result: request.result,
                    },
                );
                None
            }
            Some(HostTaskSubmission::Pending) => {
                self.registry.cancel(&task_id);
                self.retired_host_call_tasks.insert(task_id);
                Some(host_call_error(
                    runtime_id,
                    RuntimeHostCallErrorKind::Failed,
                    "immediate host call remained pending",
                ))
            }
            None => Some(host_call_error(
                runtime_id,
                RuntimeHostCallErrorKind::Failed,
                "registered adapter rejected the host-call request shape",
            )),
        }
    }

    fn host_call_result(
        &mut self,
        id: arcweft_core::step::RuntimeHostCallId,
        expected: &RuntimeCheckedType,
        outcome: HostTaskOutcome,
    ) -> RuntimeHostCallResult {
        self.stats.read_ops += outcome.metrics.read_ops;
        self.stats.write_ops += outcome.metrics.write_ops;
        self.stats.system_info_ops += outcome.metrics.system_info_ops;
        self.stats.bytes_read += outcome.metrics.bytes_read;
        self.stats.bytes_written += outcome.metrics.bytes_written;
        match outcome.completion {
            HostTaskCompletion::Ready(value) if expected.accepts_value(value.value()) => {
                self.stats.completed_tasks += 1;
                RuntimeHostCallResult {
                    id,
                    outcome: Ok(value),
                }
            }
            HostTaskCompletion::Ready(_) => {
                self.stats.failed_tasks += 1;
                host_call_error(
                    id,
                    RuntimeHostCallErrorKind::Rejected,
                    "adapter result does not satisfy the checked host-call result contract",
                )
            }
            HostTaskCompletion::Failed(message) => {
                self.stats.failed_tasks += 1;
                host_call_error(id, RuntimeHostCallErrorKind::Failed, message)
            }
        }
    }

    /// Forwards task cancellation to the adapter owning pending host work.
    pub fn cancel_task(&self, task_id: &arcweft_core::task::TaskId) -> bool {
        self.registry.cancel(task_id)
    }

    pub fn complete_tasks(&mut self, tasks: Vec<TaskSpec>) -> Vec<TaskEvent> {
        let (unauthorized, tasks): (Vec<_>, Vec<_>) = tasks
            .into_iter()
            .partition(|task| !self.policy.allows(&task.request));
        let unauthorized_events = unauthorized
            .into_iter()
            .map(|task| self.rejected_task_event(task))
            .collect::<Vec<_>>();
        let (unimplemented, tasks): (Vec<_>, Vec<_>) = tasks
            .into_iter()
            .partition(|task| !self.registry.contains(&task.request.host_call_id()));
        let unimplemented_events = unimplemented
            .into_iter()
            .map(|task| self.unimplemented_task_event(task))
            .collect::<Vec<_>>();

        let started = Instant::now();
        self.scheduler.submit(tasks);
        self.stats.scheduler_submit_elapsed_ns = self
            .stats
            .scheduler_submit_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());

        let started = Instant::now();
        let dispatch = self.scheduler.dispatch(SchedulerBudget {
            max_events: usize::MAX,
        });
        self.stats.scheduler_dispatch_elapsed_ns = self
            .stats
            .scheduler_dispatch_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());

        let started = Instant::now();
        let completions = complete_dispatched_tasks(&self.registry, &dispatch.tasks);
        self.stats.host_complete_elapsed_ns = self
            .stats
            .host_complete_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());

        if completions.parallel {
            self.stats.parallel_batches += 1;
            self.stats.parallel_tasks += completions.items.len();
            self.stats.parallel_io_tasks += dispatch
                .tasks
                .iter()
                .filter(|task| is_io_task(&task.request))
                .count();
            self.stats.parallel_system_info_tasks += dispatch
                .tasks
                .iter()
                .filter(|task| is_system_info_task(&task.request))
                .count();
            self.stats.parallel_marker_tasks += dispatch
                .tasks
                .iter()
                .filter(|task| is_scheduler_marker_task(&task.request))
                .count();
            self.stats.parallel_workers = self.stats.parallel_workers.max(
                rayon::current_num_threads()
                    .min(completions.items.len())
                    .max(1),
            );
        }

        let started = Instant::now();
        let mut events = completions
            .items
            .into_iter()
            .map(|completion| self.task_event(completion))
            .collect::<Vec<_>>();
        events.extend(unauthorized_events);
        events.extend(unimplemented_events);
        self.stats.event_build_elapsed_ns = self
            .stats
            .event_build_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());

        let started = Instant::now();
        let events = self.scheduler.complete(events);
        self.stats.scheduler_complete_elapsed_ns = self
            .stats
            .scheduler_complete_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());
        events
    }

    fn rejected_task_event(&mut self, task: TaskSpec) -> TaskEvent {
        self.stats.failed_tasks += 1;
        let event = TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: task.id,
            sequence: TaskSequence(self.sequence),
            kind: TaskEventKind::Failed(format!(
                "host call `{}` is not provided by the active adapter manifest",
                task.request.host_call_id()
            )),
        };
        self.sequence = self.sequence.saturating_add(1);
        event
    }

    fn unimplemented_task_event(&mut self, task: TaskSpec) -> TaskEvent {
        self.stats.failed_tasks += 1;
        let event = TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: task.id,
            sequence: TaskSequence(self.sequence),
            kind: TaskEventKind::Failed(format!(
                "host call `{}` is provided by the active adapter manifest but no native adapter implementation is registered",
                task.request.host_call_id()
            )),
        };
        self.sequence = self.sequence.saturating_add(1);
        event
    }

    fn task_event(&mut self, completion: TaskCompletion) -> TaskEvent {
        self.stats.read_ops += completion.stats.read_ops;
        self.stats.write_ops += completion.stats.write_ops;
        self.stats.system_info_ops += completion.stats.system_info_ops;
        self.stats.bytes_read += completion.stats.bytes_read;
        self.stats.bytes_written += completion.stats.bytes_written;
        let kind = match completion.completion {
            HostTaskCompletion::Ready(value) => {
                self.stats.completed_tasks += 1;
                TaskEventKind::Ready(value)
            }
            HostTaskCompletion::Failed(error) => {
                self.stats.failed_tasks += 1;
                TaskEventKind::Failed(error)
            }
        };
        let event = TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: completion.task_id,
            sequence: TaskSequence(self.sequence),
            kind,
        };
        self.sequence = self.sequence.saturating_add(1);
        event
    }
}

fn host_call_task_id(id: &arcweft_core::step::RuntimeHostCallId) -> TaskId {
    TaskId(format!("runtime-host-call:{}", id.0))
}

fn host_call_error(
    id: arcweft_core::step::RuntimeHostCallId,
    kind: RuntimeHostCallErrorKind,
    message: impl Into<String>,
) -> RuntimeHostCallResult {
    RuntimeHostCallResult {
        id,
        outcome: Err(RuntimeHostCallError {
            kind,
            message: message.into(),
        }),
    }
}

impl NativeFileRoots {
    pub fn new(asset: impl Into<PathBuf>, state: impl Into<PathBuf>) -> Self {
        Self {
            asset: asset.into(),
            state: state.into(),
        }
    }

    /// Default roots for a standalone source file.
    pub fn for_source(source_path: &Path) -> Self {
        let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
        Self::new(source_dir.join("assets"), source_dir.join(".arcweft"))
    }

    /// Roots for a bundle workspace whose encoded assets were materialized by the host.
    pub fn for_bundle_workspace(source_path: &Path) -> Self {
        let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
        let state = source_dir.join(".arcweft");
        Self::new(state.join("asset"), state)
    }

    pub fn asset(&self) -> &Path {
        &self.asset
    }

    pub fn state(&self) -> &Path {
        &self.state
    }
}

#[derive(Clone, Debug)]
struct TaskCompletions {
    parallel: bool,
    items: Vec<TaskCompletion>,
}

#[derive(Clone, Debug)]
struct TaskCompletion {
    task_id: arcweft_core::task::TaskId,
    completion: HostTaskCompletion,
    stats: HostTaskMetrics,
}

#[derive(Clone, Debug)]
struct NativeFileAdapter {
    manifest: AdapterManifest,
    roots: NativeFileRoots,
}

#[derive(Clone, Debug)]
struct NativeSystemInfoAdapter {
    manifest: AdapterManifest,
    host_system: HostSystemInfo,
}

#[derive(Clone, Debug)]
struct NativeCliAdapter {
    manifest: AdapterManifest,
    args: Box<[String]>,
}

#[derive(Clone, Debug)]
struct InternalSchedulerMarkerAdapter {
    manifest: AdapterManifest,
}

impl HostAdapter for NativeFileAdapter {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn complete(&self, task: &TaskSpec) -> Option<HostTaskOutcome> {
        let (result, metrics) = match &task.request {
            HostTaskRequest::FileReadText(request) => {
                complete_read_text(&self.roots, &request.path)
            }
            HostTaskRequest::FileWriteText(request) => {
                complete_write_text(&self.roots, &request.path, &request.text)
            }
            HostTaskRequest::FileReadBytes(request) => {
                complete_read_bytes(&self.roots, &request.path)
            }
            HostTaskRequest::FileWriteBytes(request) => {
                complete_write_bytes(&self.roots, &request.path, &request.bytes)
            }
            _ => return None,
        };
        Some(HostTaskOutcome {
            completion: file_task_completion(task, result),
            metrics,
        })
    }

    fn can_complete_in_parallel(&self, request: &HostTaskRequest) -> bool {
        matches!(
            request,
            HostTaskRequest::FileReadText(_) | HostTaskRequest::FileReadBytes(_)
        )
    }
}

impl HostAdapter for NativeCliAdapter {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn complete(&self, task: &TaskSpec) -> Option<HostTaskOutcome> {
        let HostTaskRequest::Custom {
            capability,
            operation,
            args,
            named_args,
        } = &task.request
        else {
            return None;
        };
        if capability.0 != "cli" || operation != "args" {
            return None;
        }
        let completion = if args.is_empty() && named_args.is_empty() {
            HostTaskCompletion::Ready(RuntimePayload::new(runtime_sequence_values(
                self.args
                    .iter()
                    .cloned()
                    .map(RuntimeValue::String)
                    .collect(),
            )))
        } else {
            HostTaskCompletion::Failed("cli.args expects no arguments".to_owned())
        };
        Some(HostTaskOutcome {
            completion,
            metrics: HostTaskMetrics::default(),
        })
    }

    fn can_complete_in_parallel(&self, request: &HostTaskRequest) -> bool {
        matches!(
            request,
            HostTaskRequest::Custom {
                capability,
                operation,
                ..
            } if capability.0 == "cli" && operation == "args"
        )
    }
}

fn file_task_completion(
    task: &TaskSpec,
    result: Result<RuntimePayload, String>,
) -> HostTaskCompletion {
    match result {
        Ok(value) => task
            .outcome
            .try_result_ok(value.value().clone())
            .map_or_else(HostTaskCompletion::Failed, HostTaskCompletion::Ready),
        Err(error) => {
            let Some(RuntimeCheckedType::Opaque { owner }) = task.outcome.result_error() else {
                return HostTaskCompletion::Failed(
                    "native file task has no exact opaque domain-error contract".to_owned(),
                );
            };
            if owner.producer().as_str() != "arcweft.adapter.native-file" {
                return HostTaskCompletion::Failed(format!(
                    "native file task error owner uses foreign producer `{}`",
                    owner.producer().as_str()
                ));
            }
            match owner.try_wrap(RuntimeValue::String(error)) {
                Ok(value) => task
                    .outcome
                    .try_result_err(value)
                    .map_or_else(HostTaskCompletion::Failed, HostTaskCompletion::Ready),
                Err(error) => HostTaskCompletion::Failed(format!(
                    "native file task could not materialize its domain error: {error}"
                )),
            }
        }
    }
}

impl HostAdapter for NativeSystemInfoAdapter {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn complete(&self, task: &TaskSpec) -> Option<HostTaskOutcome> {
        let HostTaskRequest::SystemInfo(request) = &task.request else {
            return None;
        };
        Some(HostTaskOutcome {
            completion: task
                .outcome
                .try_result_ok(RuntimeValue::String(
                    system_info_value(self.host_system, request.kind).to_string(),
                ))
                .map_or_else(HostTaskCompletion::Failed, HostTaskCompletion::Ready),
            metrics: HostTaskMetrics {
                system_info_ops: 1,
                ..HostTaskMetrics::default()
            },
        })
    }

    fn can_complete_in_parallel(&self, request: &HostTaskRequest) -> bool {
        matches!(request, HostTaskRequest::SystemInfo(_))
    }
}

impl HostAdapter for InternalSchedulerMarkerAdapter {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn complete(&self, task: &TaskSpec) -> Option<HostTaskOutcome> {
        is_scheduler_marker_task(&task.request).then(|| HostTaskOutcome {
            completion: HostTaskCompletion::Ready(RuntimePayload::new(RuntimeValue::Unit)),
            metrics: HostTaskMetrics::default(),
        })
    }

    fn can_complete_in_parallel(&self, request: &HostTaskRequest) -> bool {
        is_scheduler_marker_task(request)
    }
}

pub fn standard_cli_registry_builder(
    file_roots: NativeFileRoots,
    cli_args: &[String],
) -> Result<HostAdapterRegistryBuilder, HostAdapterError> {
    let builder = HostAdapterRegistry::builder()
        .register(NativeFileAdapter {
            manifest: standard::native_file_manifest(),
            roots: file_roots,
        })?
        .register(NativeSystemInfoAdapter {
            manifest: standard::system_info_manifest(),
            host_system: host_system_info(),
        })?
        .register(NativeCliAdapter {
            manifest: standard::native_cli_manifest(),
            args: cli_args.to_vec().into_boxed_slice(),
        })?
        .register(InternalSchedulerMarkerAdapter {
            manifest: internal_scheduler_manifest(),
        })?;
    Ok(builder)
}

fn registry_with_registrars(
    source_path: &Path,
    file_roots: NativeFileRoots,
    cli_args: &[String],
    registrars: &[NativeAdapterRegistrar],
) -> Result<HostAdapterRegistry, HostAdapterError> {
    registrars
        .iter()
        .try_fold(
            standard_cli_registry_builder(file_roots, cli_args)?,
            |builder, register| register(source_path, builder),
        )
        .map(HostAdapterRegistryBuilder::build)
}

pub fn internal_scheduler_manifest() -> AdapterManifest {
    AdapterManifest::new(INTERNAL_SCHEDULER_ADAPTER_ID, "Internal Scheduler")
        .with_host_call(AdapterHostCall::new("line_task.run_child", []))
        .with_host_call(AdapterHostCall::new("flow_thread.run_child", []))
}

fn complete_dispatched_tasks(
    registry: &HostAdapterRegistry,
    tasks: &[TaskSpec],
) -> TaskCompletions {
    let parallel = should_complete_in_parallel(registry, tasks);
    let items = if parallel {
        tasks
            .par_iter()
            .filter_map(|task| complete_task(registry, task))
            .collect()
    } else {
        tasks
            .iter()
            .filter_map(|task| complete_task(registry, task))
            .collect()
    };
    TaskCompletions { parallel, items }
}

fn should_complete_in_parallel(registry: &HostAdapterRegistry, tasks: &[TaskSpec]) -> bool {
    tasks.len() > 1
        && tasks
            .iter()
            .all(|task| registry.can_complete_in_parallel(&task.request))
        && tasks
            .iter()
            .any(|task| is_parallel_host_work(&task.request))
}

fn complete_task(registry: &HostAdapterRegistry, task: &TaskSpec) -> Option<TaskCompletion> {
    match registry.submit(task)? {
        HostTaskSubmission::Completed(outcome) => Some(TaskCompletion {
            task_id: task.id.clone(),
            completion: outcome.completion,
            stats: outcome.metrics,
        }),
        HostTaskSubmission::Pending => None,
    }
}

fn complete_read_text(
    roots: &NativeFileRoots,
    path: &str,
) -> (Result<RuntimePayload, String>, HostTaskMetrics) {
    match virtual_path(roots, path, NativeFileAccess::Read)
        .and_then(|path| fs::read_to_string(path).map_err(|error| error.to_string()))
    {
        Ok(text) => {
            let bytes_read = text.len();
            (
                Ok(RuntimePayload::from(text)),
                HostTaskMetrics {
                    read_ops: 1,
                    bytes_read,
                    ..HostTaskMetrics::default()
                },
            )
        }
        Err(error) => (Err(error), HostTaskMetrics::default()),
    }
}

fn complete_read_bytes(
    roots: &NativeFileRoots,
    path: &str,
) -> (Result<RuntimePayload, String>, HostTaskMetrics) {
    match virtual_path(roots, path, NativeFileAccess::Read)
        .and_then(|path| fs::read(path).map_err(|error| error.to_string()))
    {
        Ok(bytes) => {
            let bytes_read = bytes.len();
            (
                Ok(RuntimePayload::new(runtime_sequence_dense_bytes(bytes))),
                HostTaskMetrics {
                    read_ops: 1,
                    bytes_read,
                    ..HostTaskMetrics::default()
                },
            )
        }
        Err(error) => (Err(error), HostTaskMetrics::default()),
    }
}

fn complete_write_text(
    roots: &NativeFileRoots,
    path: &str,
    text: &str,
) -> (Result<RuntimePayload, String>, HostTaskMetrics) {
    let result = virtual_path(roots, path, NativeFileAccess::Write).and_then(|path| {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, text).map_err(|error| error.to_string())?;
        Ok(RuntimePayload::new(RuntimeValue::Unit))
    });
    let stats = result.as_ref().map_or_else(
        |_| HostTaskMetrics::default(),
        |_| HostTaskMetrics {
            write_ops: 1,
            bytes_written: text.len(),
            ..HostTaskMetrics::default()
        },
    );
    (result, stats)
}

fn complete_write_bytes(
    roots: &NativeFileRoots,
    path: &str,
    bytes: &[u8],
) -> (Result<RuntimePayload, String>, HostTaskMetrics) {
    let result = virtual_path(roots, path, NativeFileAccess::Write).and_then(|path| {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, bytes).map_err(|error| error.to_string())?;
        Ok(RuntimePayload::new(RuntimeValue::Unit))
    });
    let stats = result.as_ref().map_or_else(
        |_| HostTaskMetrics::default(),
        |_| HostTaskMetrics {
            write_ops: 1,
            bytes_written: bytes.len(),
            ..HostTaskMetrics::default()
        },
    );
    (result, stats)
}

fn is_io_task(request: &HostTaskRequest) -> bool {
    matches!(
        request,
        HostTaskRequest::FileReadText(_) | HostTaskRequest::FileReadBytes(_)
    )
}

fn is_system_info_task(request: &HostTaskRequest) -> bool {
    matches!(request, HostTaskRequest::SystemInfo(_))
}

fn is_parallel_host_work(request: &HostTaskRequest) -> bool {
    is_io_task(request) || is_system_info_task(request)
}

fn is_scheduler_marker_task(request: &HostTaskRequest) -> bool {
    match request {
        HostTaskRequest::Custom {
            capability,
            operation,
            ..
        } => is_scheduler_marker(capability.0.as_str(), operation),
        _ => false,
    }
}

fn is_scheduler_marker(capability: &str, operation: &str) -> bool {
    matches!(capability, "line_task" | "flow_thread") && operation == "run_child"
}

#[derive(Clone, Copy)]
enum NativeFileAccess {
    Read,
    Write,
}

fn virtual_path(
    roots: &NativeFileRoots,
    value: &str,
    access: NativeFileAccess,
) -> Result<PathBuf, String> {
    let (space, relative) = value
        .split_once(':')
        .ok_or_else(|| "file task path must be a virtual path".to_owned())?;
    if !matches!(space, "save" | "asset" | "temp" | "export") {
        return Err(format!("unsupported virtual path space `{space}`"));
    }
    let relative_path = Path::new(relative);
    if relative_path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir | Component::CurDir
        )
    }) {
        return Err("virtual path must be relative and normalized".to_owned());
    }
    match (space, access) {
        ("asset", NativeFileAccess::Read) => Ok(roots.asset().join(relative_path)),
        ("asset", NativeFileAccess::Write) => {
            Err("asset virtual path space is read-only".to_owned())
        }
        ("save" | "temp" | "export", _) => Ok(roots.state().join(space).join(relative_path)),
        _ => unreachable!("virtual path space is validated above"),
    }
}

impl From<RuntimeSchedulerStats> for NativeSchedulerStats {
    fn from(stats: RuntimeSchedulerStats) -> Self {
        Self {
            submitted: stats.submitted,
            joined: stats.joined,
            dispatched: stats.dispatched,
            completed: stats.completed,
            failed: stats.failed,
            cancelled: stats.cancelled,
            cancel_requested: stats.cancel_requested,
            joined_completed: stats.joined_completed,
            in_flight: stats.in_flight,
            max_in_flight: stats.max_in_flight,
            dispatch_sorts: stats.dispatch_sorts,
            dispatch_sort_items: stats.dispatch_sort_items,
            completion_sorts: stats.completion_sorts,
            completion_sort_items: stats.completion_sort_items,
            completion_normalization_passes: stats.completion_normalization_passes,
            completion_normalization_checks: stats.completion_normalization_checks,
            completion_events_in: stats.completion_events_in,
            completion_events_joined: stats.completion_events_joined,
            completion_events_out: stats.completion_events_out,
            completion_sort_skipped_items: stats.completion_sort_skipped_items,
            completion_sort_performed_items: stats.completion_sort_performed_items,
            joined_completion_events_emitted: stats.joined_completion_events_emitted,
            submitted_by_class: NativeTaskClassCounts::from(stats.submitted_by_class),
            dispatched_by_class: NativeTaskClassCounts::from(stats.dispatched_by_class),
            completed_by_class: NativeTaskClassCounts::from(stats.completed_by_class),
        }
    }
}

impl From<TaskClassCounts> for NativeTaskClassCounts {
    fn from(counts: TaskClassCounts) -> Self {
        Self {
            local_view: counts.local_view,
            io: counts.io,
            cpu: counts.cpu,
            gpu_prepare: counts.gpu_prepare,
            shader_compile: counts.shader_compile,
            wasm_call: counts.wasm_call,
            asset_decode: counts.asset_decode,
            audio_decode: counts.audio_decode,
            audio_render: counts.audio_render,
            tts_synthesis: counts.tts_synthesis,
            bgm_precompose: counts.bgm_precompose,
            lsp: counts.lsp,
            background: counts.background,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::pattern::RuntimeCheckedType;
    use arcweft_core::task::{
        CancelScopeId, HostTaskRequest, SystemInfoKind, SystemInfoRequest, TaskClass, TaskId,
        TaskKey, TaskOutcomeContract, TaskPolicy, TaskPriority,
    };

    #[test]
    fn native_bridge_rejects_host_call_missing_from_manifest() {
        let source_path = std::env::temp_dir().join("arcweft-native-bridge-reject.arcw");
        let mut bridge = NativeTaskBridge::try_new(
            &source_path,
            NativeFileRoots::for_source(&source_path),
            &[],
            HostCallPolicy::default(),
            &[],
        )
        .expect("standard native adapters are unique");
        let events = bridge.complete_tasks(vec![task(
            "missing",
            HostTaskRequest::SystemInfo(SystemInfoRequest {
                kind: SystemInfoKind::CoreCount,
            }),
        )]);

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0].kind,
            TaskEventKind::Failed(message)
                if message.contains("host call `system.core_count` is not provided")
        ));
        assert_eq!(bridge.stats().failed_tasks, 1);
        assert_eq!(bridge.stats().scheduler.submitted, 0);
    }

    #[test]
    fn native_bridge_completes_system_info_allowed_by_manifest() {
        let source_path = std::env::temp_dir().join("arcweft-native-bridge-system.arcw");
        let policy = NativeTaskBridge::policy_from_manifest(&standard::system_info_manifest());
        let mut bridge = NativeTaskBridge::try_new(
            &source_path,
            NativeFileRoots::for_source(&source_path),
            &[],
            policy,
            &[],
        )
        .expect("standard native adapters are unique");
        let events = bridge.complete_tasks(vec![task(
            "system",
            HostTaskRequest::SystemInfo(SystemInfoRequest {
                kind: SystemInfoKind::AvailableParallelism,
            }),
        )]);

        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0].kind, TaskEventKind::Ready(value) if !value.label().is_empty())
        );
        assert_eq!(bridge.stats().completed_tasks, 1);
        assert_eq!(bridge.stats().system_info_ops, 1);
        assert_eq!(bridge.stats().scheduler.submitted, 1);
    }

    #[test]
    fn native_bridge_rejects_allowed_host_call_without_native_implementation() {
        let source_path = std::env::temp_dir().join("arcweft-native-bridge-unimplemented.arcw");
        let policy = HostCallPolicy::from_manifests([standard::native_http_manifest()]);

        let error = NativeTaskBridge::try_new(
            &source_path,
            NativeFileRoots::for_source(&source_path),
            &[],
            policy,
            &[],
        )
        .expect_err("missing native implementations are rejected before task execution");
        assert!(matches!(
            error,
            HostAdapterError::MissingHostCallImplementations { host_call_ids }
                if host_call_ids == vec!["http.respond".to_owned()]
        ));
    }

    #[test]
    fn standard_cli_host_policy_is_manifest_derived() {
        let policy = NativeTaskBridge::standard_policy();

        for id in [
            "cli.args",
            "fs.read_text",
            "fs.read_bytes",
            "fs.write_text",
            "fs.write_bytes",
            "system.core_count",
            "system.thread_count",
            "system.available_parallelism",
            "line_task.run_child",
            "flow_thread.run_child",
        ] {
            assert!(policy.contains(id), "missing host call {id}");
        }
    }

    #[test]
    fn native_cli_args_complete_through_the_checked_host_call_contract() {
        let source_path = std::env::temp_dir().join("arcweft-native-cli-args.arcw");
        let policy = NativeTaskBridge::policy_from_manifest(&standard::native_cli_manifest());
        let mut bridge = NativeTaskBridge::try_new(
            &source_path,
            NativeFileRoots::for_source(&source_path),
            &["chapter.arcw".to_owned(), "--fast".to_owned()],
            policy,
            &[],
        )
        .expect("native cli adapter is registered exactly once");

        let results = bridge.complete_host_calls(vec![RuntimeHostCallRequest {
            id: arcweft_core::step::RuntimeHostCallId("cli.args.0".to_owned()),
            public_id: "cli.args".to_owned(),
            capability: "cli".to_owned(),
            operation: "args".to_owned(),
            contract: Some(standard::native_cli_manifest().host_calls()[0].contract_digest()),
            args: Vec::new(),
            named_args: Vec::new(),
            result: RuntimeCheckedType::Sequence(Box::new(RuntimeCheckedType::String)),
            mode: RuntimeHostCallMode::Immediate,
            deterministic: true,
        }]);

        assert_eq!(results.len(), 1);
        let RuntimeValue::Seq(values) = results[0]
            .outcome
            .as_ref()
            .expect("cli.args succeeds")
            .value()
        else {
            panic!("cli.args returns a sequence");
        };
        assert_eq!(
            values.clone().into_values(),
            vec![
                RuntimeValue::String("chapter.arcw".to_owned()),
                RuntimeValue::String("--fast".to_owned()),
            ]
        );
    }

    #[test]
    fn native_cli_args_rejects_a_foreign_manifest_contract_before_dispatch() {
        let source_path = std::env::temp_dir().join("arcweft-native-cli-contract.arcw");
        let policy = NativeTaskBridge::policy_from_manifest(&standard::native_cli_manifest());
        let mut bridge = NativeTaskBridge::try_new(
            &source_path,
            NativeFileRoots::for_source(&source_path),
            &[],
            policy,
            &[],
        )
        .expect("native cli adapter is registered exactly once");

        let results = bridge.complete_host_calls(vec![RuntimeHostCallRequest {
            id: arcweft_core::step::RuntimeHostCallId("cli.args.0".to_owned()),
            public_id: "cli.args".to_owned(),
            capability: "cli".to_owned(),
            operation: "args".to_owned(),
            contract: Some(arcweft_core::step::HostCallContractDigest::from_bytes(
                [0xa5; 32],
            )),
            args: Vec::new(),
            named_args: Vec::new(),
            result: RuntimeCheckedType::Sequence(Box::new(RuntimeCheckedType::String)),
            mode: RuntimeHostCallMode::Immediate,
            deterministic: true,
        }]);

        assert!(matches!(
            results.as_slice(),
            [RuntimeHostCallResult {
                outcome: Err(RuntimeHostCallError {
                    kind: RuntimeHostCallErrorKind::Rejected,
                    ..
                }),
                ..
            }]
        ));
        assert_eq!(bridge.stats().completed_tasks, 0);
    }

    #[test]
    fn native_cli_args_rejects_a_tampered_result_type_before_dispatch() {
        let source_path = std::env::temp_dir().join("arcweft-native-cli-result.arcw");
        let manifest = standard::native_cli_manifest();
        let policy = NativeTaskBridge::policy_from_manifest(&manifest);
        let mut bridge = NativeTaskBridge::try_new(
            &source_path,
            NativeFileRoots::for_source(&source_path),
            &["matching-payload".to_owned()],
            policy,
            &[],
        )
        .expect("native cli adapter is registered exactly once");

        let results = bridge.complete_host_calls(vec![RuntimeHostCallRequest {
            id: arcweft_core::step::RuntimeHostCallId("cli.args.0".to_owned()),
            public_id: "cli.args".to_owned(),
            capability: "cli".to_owned(),
            operation: "args".to_owned(),
            contract: Some(manifest.host_calls()[0].contract_digest()),
            args: Vec::new(),
            named_args: Vec::new(),
            result: RuntimeCheckedType::String,
            mode: RuntimeHostCallMode::Immediate,
            deterministic: true,
        }]);

        assert!(matches!(
            results.as_slice(),
            [RuntimeHostCallResult {
                outcome: Err(RuntimeHostCallError {
                    kind: RuntimeHostCallErrorKind::Rejected,
                    ..
                }),
                ..
            }]
        ));
        assert_eq!(bridge.stats().completed_tasks, 0);
        assert_eq!(bridge.stats().failed_tasks, 0);
    }

    #[test]
    fn native_file_roots_separate_read_only_assets_from_mutable_state() {
        let roots = NativeFileRoots::new("project/assets", "project/.arcweft");

        assert_eq!(
            virtual_path(&roots, "asset:bg/room.png", NativeFileAccess::Read).unwrap(),
            Path::new("project/assets/bg/room.png")
        );
        assert_eq!(
            virtual_path(&roots, "save:slot/one.json", NativeFileAccess::Write).unwrap(),
            Path::new("project/.arcweft/save/slot/one.json")
        );
        assert_eq!(
            virtual_path(&roots, "asset:bg/room.png", NativeFileAccess::Write).unwrap_err(),
            "asset virtual path space is read-only"
        );
    }

    fn task(id: &str, request: HostTaskRequest) -> TaskSpec {
        TaskSpec::new(
            TaskId(id.to_owned()),
            TaskKey(id.to_owned()),
            TaskClass::Cpu,
            TaskPriority(0),
            CancelScopeId("test".to_owned()),
            TaskPolicy::JoinSameKey,
            request,
        )
        .with_outcome(TaskOutcomeContract::new(RuntimeCheckedType::Result {
            ok: Box::new(RuntimeCheckedType::String),
            error: Box::new(RuntimeCheckedType::String),
        }))
    }
}
