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
    BoundTaskOutcome, BoundTaskSpec, CancelScopeId, GenerationId, HostTaskRequest, LogicalEpoch,
    RuntimeProgramOwner, SchedulerBudget, TaskCompletionError, TaskDispatchIdentity,
    TaskDispatchStart, TaskEnsureError, TaskEvent, TaskEventKind, TaskId, TaskKey,
    TaskOutcomeBindingError, TaskOutcomeContract, TaskPolicy, TaskPriority,
    TaskPublicationRevision, TaskSequence, TaskSpec, normalize_task_events,
};
use arcweft_core::value::{
    RuntimeBundleAssetContext, RuntimePayload, RuntimeUnsignedIntWidth, RuntimeValue,
    RuntimeVirtualPath, RuntimeVirtualPathSpace, runtime_sequence_dense_bytes,
    runtime_sequence_values,
};
use arcweft_host_adapter::{
    HostAdapter, HostAdapterCompletion, HostAdapterError, HostAdapterRegistry,
    HostAdapterRegistryBuilder, HostCallArgs, HostCallPolicy, HostTaskCompletion, HostTaskMetrics,
    HostTaskOutcome, HostTaskSubmission, HostTaskSubmissionContext,
};
use arcweft_runtime_scheduler::{RuntimeScheduler, RuntimeSchedulerStats, TaskClassCounts};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;
use thiserror::Error;

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
    dispatches: BTreeMap<TaskId, TaskDispatchIdentity>,
    publication_frontiers: BTreeMap<TaskId, TaskPublicationRevision>,
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
    result: BoundTaskOutcome,
    publication_revision: Option<TaskPublicationRevision>,
}

#[derive(Debug, Error)]
pub enum NativeTaskBridgeError {
    #[error("failed to bind a task result to its selected program: {0}")]
    TaskBinding(#[from] TaskOutcomeBindingError),
    #[error("task submission rejected a conflicting specification: {0}")]
    TaskSubmission(#[from] TaskEnsureError),
    #[error("task completion rejected an unregistered or repeated event: {0}")]
    TaskCompletion(#[from] TaskCompletionError),
    #[error("adapter returned the same task completion twice in one batch: {task_id:?}")]
    DuplicateAdapterCompletion { task_id: TaskId },
    #[error("native host task dispatch identity sequence is exhausted")]
    DispatchSequenceExhausted,
    #[error("task {task_id:?} publication revision is exhausted")]
    PublicationRevisionExhausted { task_id: TaskId },
    #[error(
        "task {task_id:?} asset context generation {context_generation:?} differs from dispatch generation {dispatch_generation:?}"
    )]
    BundleAssetContextGenerationMismatch {
        task_id: TaskId,
        context_generation: GenerationId,
        dispatch_generation: GenerationId,
    },
}

/// One task paired with the exact dispatch start and its retained bundle asset
/// identity, when the request is a bundle-backed asset load.
#[derive(Clone, Debug)]
pub struct NativeTaskDispatch {
    start: TaskDispatchStart,
    task: TaskSpec,
    bundle_asset_context: Option<RuntimeBundleAssetContext>,
}

impl NativeTaskDispatch {
    #[must_use]
    pub const fn new(
        start: TaskDispatchStart,
        task: TaskSpec,
        bundle_asset_context: Option<RuntimeBundleAssetContext>,
    ) -> Self {
        Self {
            start,
            task,
            bundle_asset_context,
        }
    }

    #[must_use]
    pub const fn start(&self) -> &TaskDispatchStart {
        &self.start
    }

    #[must_use]
    pub const fn task(&self) -> &TaskSpec {
        &self.task
    }

    #[must_use]
    pub const fn bundle_asset_context(&self) -> Option<RuntimeBundleAssetContext> {
        self.bundle_asset_context
    }
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
            dispatches: BTreeMap::new(),
            publication_frontiers: BTreeMap::new(),
            scheduler: RuntimeScheduler::default(),
            pending_host_calls: BTreeMap::new(),
            retired_host_call_tasks: BTreeSet::new(),
            seen_host_calls: BTreeSet::new(),
            ready_host_call_results: Vec::new(),
            stats: NativeTaskStats::default(),
        })
    }

    pub fn standard_policy() -> HostCallPolicy {
        HostCallPolicy::from_returning_manifests([
            standard::native_file_manifest(),
            standard::native_cli_manifest(),
            standard::system_info_manifest(),
            internal_scheduler_manifest(),
        ])
    }

    pub fn policy_from_manifest(manifest: &AdapterManifest) -> HostCallPolicy {
        HostCallPolicy::from_manifests([manifest.clone()])
    }

    /// Checks whether the existing registry can implement an additional
    /// generation's manifest-derived host-call policy.
    pub fn validate_policy_extension(
        &self,
        additional: HostCallPolicy,
    ) -> Result<(), HostAdapterError> {
        self.policy
            .clone()
            .union(additional)
            .ensure_implemented_by(&self.registry)
    }

    /// Admits additional manifest-derived calls without replacing the bridge's
    /// registry, scheduler, dispatch journal, or retained generations.
    pub fn try_allow_policy(&mut self, additional: HostCallPolicy) -> Result<(), HostAdapterError> {
        let expanded = self.policy.clone().union(additional);
        expanded.ensure_implemented_by(&self.registry)?;
        self.policy = expanded;
        Ok(())
    }

    /// Admits only the selected adapter and the engine's internal scheduling calls.
    pub fn selected_policy_for_manifest(manifest: &AdapterManifest) -> HostCallPolicy {
        Self::policy_from_manifest(manifest).union(HostCallPolicy::from_manifests([
            internal_scheduler_manifest(),
        ]))
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
    pub fn poll_completions(&mut self) -> Result<Vec<TaskEvent>, NativeTaskBridgeError> {
        let mut completions = self.registry.drain_completions();
        completions.sort_by(|left, right| {
            left.task_id
                .cmp(&right.task_id)
                .then_with(|| left.publication_revision.cmp(&right.publication_revision))
        });
        if let Some(pair) = completions.windows(2).find(|pair| {
            pair[0].task_id == pair[1].task_id
                && self.pending_host_calls.contains_key(&pair[0].task_id)
        }) {
            return Err(NativeTaskBridgeError::DuplicateAdapterCompletion {
                task_id: pair[0].task_id.clone(),
            });
        }
        let mut owner_ids = BTreeSet::new();
        let mut host_results = Vec::new();
        let mut host_call_progress_metrics = Vec::new();
        let mut retired_seen = Vec::new();
        let saved_stats = self.stats;
        let saved_sequence = self.sequence;
        let mut pending_host_calls_after = self.pending_host_calls.clone();
        let mut task_events = Vec::new();
        for HostAdapterCompletion {
            task_id,
            publication_revision,
            outcome,
        } in completions
        {
            if let Some(mut pending) = pending_host_calls_after.get(&task_id).cloned() {
                if pending
                    .publication_revision
                    .is_some_and(|previous| publication_revision <= previous)
                    || pending.publication_revision.is_none()
                        && publication_revision != TaskPublicationRevision::FIRST
                {
                    self.stats = saved_stats;
                    return Err(TaskCompletionError::StalePublication { task_id }.into());
                }
                pending.publication_revision = Some(publication_revision);
                if matches!(&outcome.completion, HostTaskCompletion::Progress(_)) {
                    host_call_progress_metrics.push(outcome.metrics);
                    pending_host_calls_after.insert(task_id, pending);
                } else {
                    pending_host_calls_after.insert(task_id.clone(), pending.clone());
                    host_results.push((task_id, pending, outcome));
                }
            } else if self.retired_host_call_tasks.contains(&task_id) {
                retired_seen.push(task_id);
                continue;
            } else {
                owner_ids.insert(task_id.clone());
                let event = match self.task_event(TaskCompletion {
                    task_id,
                    completion: outcome.completion,
                    publication_revision,
                    stats: outcome.metrics,
                }) {
                    Ok(event) => event,
                    Err(error) => {
                        self.stats = saved_stats;
                        return Err(error);
                    }
                };
                task_events.push(event);
            }
        }
        // Validate the host-owned frontier before the scheduler can observe
        // any completion. In particular, a freshly restored scheduler has no
        // prior publication state of its own; the bridge's saved dispatch
        // start remains the authority for rejecting a stale replay.
        if let Err(error) = self.prepare_task_event_frontiers(&task_events) {
            self.stats = saved_stats;
            return Err(error);
        }
        let mut scheduler_after = self.scheduler.clone();
        let events = match scheduler_after.complete(task_events) {
            Ok(events) => events,
            Err(error) => {
                self.stats = saved_stats;
                self.sequence = saved_sequence;
                return Err(error.into());
            }
        };
        self.record_scheduler_owner_outcomes(&owner_ids, &events);
        let events = match self.restamp_dispatch_events(events) {
            Ok(events) => normalize_task_events(events),
            Err(error) => {
                self.stats = saved_stats;
                self.sequence = saved_sequence;
                return Err(error);
            }
        };
        let frontiers = match self.prepare_task_event_frontiers(&events) {
            Ok(frontiers) => frontiers,
            Err(error) => {
                self.stats = saved_stats;
                self.sequence = saved_sequence;
                return Err(error);
            }
        };
        self.scheduler = scheduler_after;
        self.publication_frontiers = frontiers;
        self.retire_terminal_dispatches(&events);
        self.pending_host_calls = pending_host_calls_after;
        for metrics in host_call_progress_metrics {
            self.record_metrics(metrics);
        }
        for task_id in retired_seen {
            self.retired_host_call_tasks.remove(&task_id);
        }
        for (task_id, pending, outcome) in host_results {
            self.pending_host_calls.remove(&task_id);
            let result = self.host_call_result(pending.id, &pending.result, outcome);
            self.ready_host_call_results.push(result);
        }
        self.ready_host_call_results
            .sort_by(|left, right| left.id.cmp(&right.id));
        Ok(events)
    }

    /// Dispatches direct runtime host calls through the same manifest-owned
    /// adapter registry as temporal tasks. Synchronous results are returned in
    /// call-identity order; suspended completions are retrieved with
    /// [`Self::take_host_call_results`] after [`Self::poll_completions`].
    pub fn complete_host_calls(
        &mut self,
        program: RuntimeProgramOwner,
        requests: Vec<RuntimeHostCallRequest>,
    ) -> Vec<RuntimeHostCallResult> {
        let mut results = requests
            .into_iter()
            .filter_map(|request| self.complete_host_call(program.clone(), request))
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
        program: RuntimeProgramOwner,
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
        if self.scheduler.contains_task_id(&task_id) {
            return Some(host_call_error(
                runtime_id,
                RuntimeHostCallErrorKind::Rejected,
                "runtime host-call identity collides with a scheduler task",
            ));
        }
        let contract = request.contract;
        let named_args = request
            .named_args
            .into_iter()
            .map(|argument| (argument.name, argument.value))
            .collect::<Vec<_>>();
        let host_request = if let Some(contract) = contract {
            HostTaskRequest::custom_with_named_args_and_manifest_contract(
                request.capability,
                request.operation,
                request.args,
                named_args,
                contract,
            )
        } else {
            HostTaskRequest::custom_with_named_args(
                request.capability,
                request.operation,
                request.args,
                named_args,
            )
        };
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
            request.result,
        ) {
            return Some(host_call_error(
                runtime_id,
                RuntimeHostCallErrorKind::Rejected,
                "host-call result type does not match the registered adapter manifest",
            ));
        }
        let outcome_contract = TaskOutcomeContract::program(request.result);
        let bound = match outcome_contract.bind_program(
            program,
            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
        ) {
            Ok(bound) => bound,
            Err(error) => {
                return Some(host_call_error(
                    runtime_id,
                    RuntimeHostCallErrorKind::Rejected,
                    error.to_string(),
                ));
            }
        };
        let task = TaskSpec::new(
            task_id.clone(),
            TaskKey(task_id.0.clone()),
            host_request.task_class(),
            TaskPriority(0),
            CancelScopeId("runtime-host-call".to_owned()),
            TaskPolicy::AlwaysStart,
            host_request,
        )
        .with_outcome(outcome_contract);
        match self.registry.submit_runtime_host_call(
            &task,
            &bound,
            request.mode,
            HostTaskSubmissionContext::new(TaskPublicationRevision::FIRST),
        ) {
            Some(HostTaskSubmission::Completed(outcome)) => {
                Some(self.host_call_result(runtime_id, &bound, outcome))
            }
            Some(HostTaskSubmission::Pending) if request.mode == RuntimeHostCallMode::Suspend => {
                self.pending_host_calls.insert(
                    task_id,
                    PendingRuntimeHostCall {
                        id: runtime_id,
                        result: bound,
                        publication_revision: None,
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
        expected: &BoundTaskOutcome,
        outcome: HostTaskOutcome,
    ) -> RuntimeHostCallResult {
        self.record_metrics(outcome.metrics);
        match outcome.completion {
            HostTaskCompletion::Progress(_) => {
                self.stats.failed_tasks += 1;
                host_call_error(
                    id,
                    RuntimeHostCallErrorKind::Failed,
                    "adapter returned a progress publication as a terminal host-call result",
                )
            }
            HostTaskCompletion::Ready(value)
                if expected.try_payload(value.value().clone()).is_ok() =>
            {
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
                    "adapter result does not satisfy the selected program host-call result type",
                )
            }
            HostTaskCompletion::Failed(message) => {
                self.stats.failed_tasks += 1;
                host_call_error(id, RuntimeHostCallErrorKind::Failed, message)
            }
        }
    }

    fn record_metrics(&mut self, metrics: HostTaskMetrics) {
        self.stats.read_ops += metrics.read_ops;
        self.stats.write_ops += metrics.write_ops;
        self.stats.system_info_ops += metrics.system_info_ops;
        self.stats.bytes_read += metrics.bytes_read;
        self.stats.bytes_written += metrics.bytes_written;
    }

    /// Forwards task cancellation to the adapter owning pending host work.
    pub fn cancel_task(&self, task_id: &arcweft_core::task::TaskId) -> bool {
        self.registry.cancel(task_id)
    }

    pub fn complete_tasks(
        &mut self,
        program: RuntimeProgramOwner,
        tasks: Vec<TaskSpec>,
    ) -> Result<Vec<TaskEvent>, NativeTaskBridgeError> {
        self.complete_tasks_with_generation(program, GenerationId::new(0), LogicalEpoch(0), tasks)
    }

    /// Assigns exact dispatch identities before submission. Restarts of an
    /// already active task reuse its original tuple and publication frontier.
    pub fn complete_tasks_with_generation(
        &mut self,
        program: RuntimeProgramOwner,
        generation: GenerationId,
        request_epoch: LogicalEpoch,
        tasks: Vec<TaskSpec>,
    ) -> Result<Vec<TaskEvent>, NativeTaskBridgeError> {
        self.complete_tasks_with_generation_and_bundle_asset_context(
            program,
            generation,
            request_epoch,
            tasks,
            None,
        )
    }

    /// Assigns dispatch identities and attaches the exact bundle context only
    /// to asset-load requests in this single-generation batch.
    pub fn complete_tasks_with_generation_and_bundle_asset_context(
        &mut self,
        program: RuntimeProgramOwner,
        generation: GenerationId,
        request_epoch: LogicalEpoch,
        tasks: Vec<TaskSpec>,
        bundle_asset_context: Option<RuntimeBundleAssetContext>,
    ) -> Result<Vec<TaskEvent>, NativeTaskBridgeError> {
        let mut next_sequence = self.sequence;
        let mut dispatches = Vec::with_capacity(tasks.len());
        let mut seen = BTreeSet::new();
        for task in tasks {
            if !seen.insert(task.id.clone()) {
                return Err(
                    TaskEnsureError::TaskIdSpecificationConflict { task_id: task.id }.into(),
                );
            }
            let identity = if let Some(identity) = self.dispatches.get(&task.id) {
                identity.clone()
            } else {
                let identity = TaskDispatchIdentity::new(
                    generation,
                    request_epoch,
                    TaskSequence(next_sequence),
                    task.id.clone(),
                );
                next_sequence = next_sequence
                    .checked_add(1)
                    .ok_or(NativeTaskBridgeError::DispatchSequenceExhausted)?;
                identity
            };
            let last_publication_revision = self.publication_frontiers.get(&task.id).copied();
            let task_asset_context = bundle_asset_context
                .filter(|_| matches!(&task.request, HostTaskRequest::AssetLoad(_)));
            dispatches.push(NativeTaskDispatch::new(
                TaskDispatchStart::new(identity, last_publication_revision),
                task,
                task_asset_context,
            ));
        }
        self.complete_tasks_with_dispatches(program, dispatches)
    }

    /// Submits requests paired with their exact generation, epoch, sequence,
    /// task identity, and optional retained bundle asset identity. This is the
    /// bridge for re-ensure and mixed-generation batches where the caller owns
    /// the dispatch coordinates.
    pub fn complete_tasks_with_dispatches(
        &mut self,
        program: RuntimeProgramOwner,
        dispatches: Vec<NativeTaskDispatch>,
    ) -> Result<Vec<TaskEvent>, NativeTaskBridgeError> {
        let mut next_sequence = self.sequence;
        let mut candidate_dispatches = BTreeMap::new();
        let mut candidate_start_frontiers = BTreeMap::new();
        let mut submission_contexts = BTreeMap::new();
        let mut identities = BTreeSet::new();
        let mut task_ids = BTreeSet::new();
        let mut tasks = Vec::with_capacity(dispatches.len());
        for NativeTaskDispatch {
            start,
            task,
            bundle_asset_context,
        } in dispatches
        {
            if !task_ids.insert(task.id.clone()) {
                return Err(
                    TaskEnsureError::TaskIdSpecificationConflict { task_id: task.id }.into(),
                );
            }
            let identity = start.identity();
            if identity.task_id != task.id {
                return Err(TaskCompletionError::DispatchMismatch { task_id: task.id }.into());
            }
            if let Some(context) = bundle_asset_context
                && context.generation() != identity.generation
            {
                return Err(
                    NativeTaskBridgeError::BundleAssetContextGenerationMismatch {
                        task_id: task.id,
                        context_generation: context.generation(),
                        dispatch_generation: identity.generation,
                    },
                );
            }
            if let Some(existing) = self.dispatches.get(&task.id)
                && existing != identity
            {
                return Err(TaskCompletionError::DispatchMismatch { task_id: task.id }.into());
            }
            let last_publication_revision = start.last_publication_revision();
            if self.dispatches.contains_key(&task.id)
                && self.publication_frontiers.get(&task.id).copied() != last_publication_revision
            {
                return Err(TaskCompletionError::DispatchMismatch { task_id: task.id }.into());
            }
            if !self.dispatches.contains_key(&task.id)
                && self.publication_frontiers.contains_key(&task.id)
            {
                return Err(TaskCompletionError::DispatchMismatch { task_id: task.id }.into());
            }
            let next_publication_revision = start.next_publication_revision().ok_or_else(|| {
                NativeTaskBridgeError::PublicationRevisionExhausted {
                    task_id: task.id.clone(),
                }
            })?;
            let tuple = (
                identity.generation,
                identity.logical_epoch,
                identity.sequence,
            );
            if !identities.insert(tuple)
                || self.dispatches.iter().any(|(task_id, existing)| {
                    task_id != &task.id
                        && existing.generation == identity.generation
                        && existing.logical_epoch == identity.logical_epoch
                        && existing.sequence == identity.sequence
                })
            {
                return Err(TaskCompletionError::DispatchMismatch { task_id: task.id }.into());
            }
            let following = identity
                .sequence
                .0
                .checked_add(1)
                .ok_or(NativeTaskBridgeError::DispatchSequenceExhausted)?;
            next_sequence = next_sequence.max(following);
            candidate_dispatches.insert(task.id.clone(), identity.clone());
            candidate_start_frontiers.insert(task.id.clone(), last_publication_revision);
            let mut submission_context = HostTaskSubmissionContext::new(next_publication_revision);
            if let Some(context) = bundle_asset_context {
                submission_context = submission_context.with_bundle_asset_context(context);
            }
            submission_contexts.insert(task.id.clone(), submission_context);
            tasks.push(task);
        }
        for task in &tasks {
            if self.pending_host_calls.contains_key(&task.id)
                || self.retired_host_call_tasks.contains(&task.id)
            {
                return Err(TaskEnsureError::TaskIdSpecificationConflict {
                    task_id: task.id.clone(),
                }
                .into());
            }
        }
        let (unauthorized, tasks): (Vec<_>, Vec<_>) = tasks
            .into_iter()
            .partition(|task| !self.policy.allows(&task.request));
        let (unimplemented, tasks): (Vec<_>, Vec<_>) = tasks
            .into_iter()
            .partition(|task| !self.registry.contains(&task.request.host_call_id()));
        let scheduled_ids = tasks
            .iter()
            .map(|task| task.id.clone())
            .collect::<BTreeSet<_>>();
        let mut rejected_ids = BTreeSet::new();
        for task in unauthorized.iter().chain(&unimplemented) {
            if self.scheduler.contains_task_id(&task.id)
                || scheduled_ids.contains(&task.id)
                || !rejected_ids.insert(task.id.clone())
            {
                return Err(TaskEnsureError::TaskIdSpecificationConflict {
                    task_id: task.id.clone(),
                }
                .into());
            }
        }
        let bound_tasks = tasks
            .into_iter()
            .map(|task| {
                let owner = match &task.outcome {
                    TaskOutcomeContract::Standalone { .. } => None,
                    TaskOutcomeContract::Program { .. } => Some(program.clone()),
                };
                BoundTaskSpec::bind(
                    task,
                    owner,
                    arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let started = Instant::now();
        if let Err(error) = self.scheduler.submit(bound_tasks) {
            return Err(error.into());
        }
        for (task_id, identity) in candidate_dispatches {
            self.dispatches.entry(task_id).or_insert(identity);
        }
        for (task_id, frontier) in candidate_start_frontiers {
            if let Some(frontier) = frontier {
                self.publication_frontiers.insert(task_id, frontier);
            }
        }
        self.sequence = next_sequence;
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
        let completions =
            complete_dispatched_tasks(&self.registry, &dispatch.tasks, &submission_contexts);
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
                .filter(|task| is_io_task(&task.spec().request))
                .count();
            self.stats.parallel_system_info_tasks += dispatch
                .tasks
                .iter()
                .filter(|task| is_system_info_task(&task.spec().request))
                .count();
            self.stats.parallel_marker_tasks += dispatch
                .tasks
                .iter()
                .filter(|task| is_scheduler_marker_task(&task.spec().request))
                .count();
            self.stats.parallel_workers = self.stats.parallel_workers.max(
                rayon::current_num_threads()
                    .min(completions.items.len())
                    .max(1),
            );
        }

        let started = Instant::now();
        let owner_ids = completions
            .items
            .iter()
            .map(|completion| completion.task_id.clone())
            .collect::<BTreeSet<_>>();
        let mut events = completions
            .items
            .into_iter()
            .map(|completion| self.task_event(completion))
            .collect::<Result<Vec<_>, _>>()?;
        self.stats.event_build_elapsed_ns = self
            .stats
            .event_build_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());

        // Check adapter and synchronous outcomes against the exact dispatch
        // frontier before mutating scheduler state. The post-scheduler check
        // below also validates joined waiter publications.
        self.prepare_task_event_frontiers(&events)?;

        let started = Instant::now();
        let mut scheduler_after = self.scheduler.clone();
        let scheduler_events = match scheduler_after.complete(std::mem::take(&mut events)) {
            Ok(events) => events,
            Err(error) => return Err(error.into()),
        };
        self.record_scheduler_owner_outcomes(&owner_ids, &scheduler_events);
        events = self.restamp_dispatch_events(scheduler_events)?;
        events.extend(
            unauthorized
                .into_iter()
                .map(|task| {
                    let next = submission_contexts
                        .get(&task.id)
                        .expect("every submitted task has a publication context")
                        .next_publication_revision();
                    self.rejected_task_event(task, next)
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
        events.extend(
            unimplemented
                .into_iter()
                .map(|task| {
                    let next = submission_contexts
                        .get(&task.id)
                        .expect("every submitted task has a publication context")
                        .next_publication_revision();
                    self.unimplemented_task_event(task, next)
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
        events = self.restamp_dispatch_events(events)?;
        self.stats.scheduler_complete_elapsed_ns = self
            .stats
            .scheduler_complete_elapsed_ns
            .saturating_add(started.elapsed().as_nanos());
        let events = normalize_task_events(events);
        let frontiers = self.prepare_task_event_frontiers(&events)?;
        self.scheduler = scheduler_after;
        self.publication_frontiers = frontiers;
        self.retire_terminal_dispatches(&events);
        Ok(events)
    }

    fn rejected_task_event(
        &mut self,
        task: TaskSpec,
        publication_revision: TaskPublicationRevision,
    ) -> Result<TaskEvent, NativeTaskBridgeError> {
        self.stats.failed_tasks += 1;
        self.event_for_task(
            &task.id,
            publication_revision,
            TaskEventKind::Failed(format!(
                "host call `{}` is not provided by the active adapter manifest",
                task.request.host_call_id()
            )),
        )
    }

    fn unimplemented_task_event(
        &mut self,
        task: TaskSpec,
        publication_revision: TaskPublicationRevision,
    ) -> Result<TaskEvent, NativeTaskBridgeError> {
        self.stats.failed_tasks += 1;
        self.event_for_task(
            &task.id,
            publication_revision,
            TaskEventKind::Failed(format!(
                "host call `{}` is provided by the active adapter manifest but no native adapter implementation is registered",
                task.request.host_call_id()
            )),
        )
    }

    fn task_event(
        &mut self,
        completion: TaskCompletion,
    ) -> Result<TaskEvent, NativeTaskBridgeError> {
        self.record_metrics(completion.stats);
        let kind = match completion.completion {
            HostTaskCompletion::Progress(value) => TaskEventKind::Progress(value),
            HostTaskCompletion::Ready(value) => TaskEventKind::Ready(value),
            HostTaskCompletion::Failed(error) => TaskEventKind::Failed(error),
        };
        self.event_for_task(&completion.task_id, completion.publication_revision, kind)
    }

    fn event_for_task(
        &self,
        task_id: &TaskId,
        publication_revision: TaskPublicationRevision,
        kind: TaskEventKind,
    ) -> Result<TaskEvent, NativeTaskBridgeError> {
        let dispatch = self.dispatches.get(task_id).cloned().ok_or_else(|| {
            TaskCompletionError::DispatchMismatch {
                task_id: task_id.clone(),
            }
        })?;
        Ok(TaskEvent::from_dispatch(
            dispatch,
            publication_revision,
            kind,
        ))
    }

    /// Rebinds scheduler events, including joined waiter events, to the exact
    /// dispatch accepted for each task id. The scheduler owns publication
    /// revision ordering; this bridge only restores each waiter's own request
    /// coordinates before the events leave the native boundary.
    fn restamp_dispatch_events(
        &self,
        events: Vec<TaskEvent>,
    ) -> Result<Vec<TaskEvent>, NativeTaskBridgeError> {
        events
            .into_iter()
            .map(|event| {
                let identity = self
                    .dispatches
                    .get(&event.task_id)
                    .cloned()
                    .ok_or_else(|| TaskCompletionError::DispatchMismatch {
                        task_id: event.task_id.clone(),
                    })?;
                Ok(TaskEvent::from_dispatch(
                    identity,
                    event.publication_revision,
                    event.kind,
                ))
            })
            .collect()
    }

    fn retire_terminal_dispatches(&mut self, events: &[TaskEvent]) {
        for event in events {
            if matches!(
                event.kind,
                TaskEventKind::Ready(_) | TaskEventKind::Failed(_) | TaskEventKind::Cancelled
            ) {
                self.dispatches.remove(&event.task_id);
                self.publication_frontiers.remove(&event.task_id);
            }
        }
    }

    fn prepare_task_event_frontiers(
        &self,
        events: &[TaskEvent],
    ) -> Result<BTreeMap<TaskId, TaskPublicationRevision>, NativeTaskBridgeError> {
        let mut frontiers = self.publication_frontiers.clone();
        let mut terminal = BTreeSet::new();
        for event in events {
            if terminal.contains(&event.task_id) {
                return Err(TaskCompletionError::StalePublication {
                    task_id: event.task_id.clone(),
                }
                .into());
            }
            let identity = self.dispatches.get(&event.task_id).ok_or_else(|| {
                TaskCompletionError::DispatchMismatch {
                    task_id: event.task_id.clone(),
                }
            })?;
            if identity != &event.dispatch_identity() {
                return Err(TaskCompletionError::DispatchMismatch {
                    task_id: event.task_id.clone(),
                }
                .into());
            }
            let expected = frontiers
                .get(&event.task_id)
                .copied()
                .map_or(
                    Some(TaskPublicationRevision::FIRST),
                    TaskPublicationRevision::checked_next,
                )
                .ok_or_else(|| NativeTaskBridgeError::PublicationRevisionExhausted {
                    task_id: event.task_id.clone(),
                })?;
            if event.publication_revision != expected {
                return Err(TaskCompletionError::StalePublication {
                    task_id: event.task_id.clone(),
                }
                .into());
            }
            if matches!(event.kind, TaskEventKind::Progress(_))
                && event.publication_revision.checked_next().is_none()
            {
                return Err(TaskCompletionError::PublicationRevisionExhausted {
                    task_id: event.task_id.clone(),
                }
                .into());
            }
            frontiers.insert(event.task_id.clone(), event.publication_revision);
            if matches!(
                event.kind,
                TaskEventKind::Ready(_) | TaskEventKind::Failed(_) | TaskEventKind::Cancelled
            ) {
                terminal.insert(event.task_id.clone());
            }
        }
        Ok(frontiers)
    }

    fn record_scheduler_owner_outcomes(
        &mut self,
        owner_ids: &BTreeSet<TaskId>,
        events: &[TaskEvent],
    ) {
        for event in events {
            if !owner_ids.contains(&event.task_id) {
                continue;
            }
            match event.kind {
                TaskEventKind::Ready(_) => self.stats.completed_tasks += 1,
                TaskEventKind::Failed(_) => self.stats.failed_tasks += 1,
                TaskEventKind::Progress(_) | TaskEventKind::Cancelled => {}
            }
        }
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
    publication_revision: TaskPublicationRevision,
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

    fn complete(&self, task: &TaskSpec, bound: &BoundTaskOutcome) -> Option<HostTaskOutcome> {
        if let HostTaskRequest::Custom {
            capability,
            operation,
            args,
            named_args,
            ..
        } = &task.request
            && capability.0 == "path"
        {
            let space = RuntimeVirtualPathSpace::from_label(operation)?;
            let argument = HostCallArgs::new(args, named_args).single("path");
            let completion = match argument {
                Ok(RuntimeValue::String(path)) => bound
                    .try_payload(RuntimeVirtualPath::new(space, path.clone()).into_value())
                    .map_or_else(
                        |error| HostTaskCompletion::Failed(error.to_string()),
                        HostTaskCompletion::Ready,
                    ),
                _ => HostTaskCompletion::Failed(
                    "path constructor expects exactly one String path argument".to_owned(),
                ),
            };
            return Some(HostTaskOutcome {
                completion,
                metrics: HostTaskMetrics::default(),
            });
        }
        if matches!(
            &task.request,
            HostTaskRequest::Custom { capability, .. } if capability.0 == "fs"
        ) {
            return Some(complete_manifest_file_call(
                &self.manifest,
                &self.roots,
                task,
                bound,
            ));
        }
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
            completion: file_task_completion(bound, result),
            metrics,
        })
    }

    fn can_complete_in_parallel(&self, request: &HostTaskRequest) -> bool {
        is_file_read_request(request)
    }
}

impl HostAdapter for NativeCliAdapter {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn complete(&self, task: &TaskSpec, bound: &BoundTaskOutcome) -> Option<HostTaskOutcome> {
        let HostTaskRequest::Custom {
            capability,
            operation,
            args,
            named_args,
            ..
        } = &task.request
        else {
            return None;
        };
        if capability.0 != "cli" {
            return None;
        }
        let arguments = HostCallArgs::new(args, named_args);
        let completion = match operation.as_str() {
            "args" if args.is_empty() && named_args.is_empty() => bound
                .try_payload(runtime_sequence_values(
                    self.args
                        .iter()
                        .cloned()
                        .map(RuntimeValue::String)
                        .collect(),
                ))
                .map_or_else(
                    |error| HostTaskCompletion::Failed(error.to_string()),
                    HostTaskCompletion::Ready,
                ),
            "args" => HostTaskCompletion::Failed("cli.args expects no arguments".to_owned()),
            "stdout" | "stderr" => match arguments.single("text") {
                Ok(RuntimeValue::String(text)) => {
                    if operation == "stdout" {
                        Self::write_output(io::stdout().lock(), text, bound)
                    } else {
                        Self::write_output(io::stderr().lock(), text, bound)
                    }
                }
                _ => HostTaskCompletion::Failed(format!(
                    "cli.{operation} expects exactly one String text argument"
                )),
            },
            "exit" => match arguments.single("code").ok().and_then(|value| match value {
                RuntimeValue::Int(value) => value.exact_i32(),
                _ => None,
            }) {
                Some(code) => {
                    // The native process adapter owns termination. A Never call
                    // cannot publish a successful runtime payload or continue.
                    std::process::exit(code);
                }
                _ => HostTaskCompletion::Failed(
                    "cli.exit expects exactly one i32 code argument".to_owned(),
                ),
            },
            _ => return None,
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

impl NativeCliAdapter {
    fn write_output(
        mut writer: impl Write,
        text: &str,
        bound: &BoundTaskOutcome,
    ) -> HostTaskCompletion {
        match writer
            .write_all(text.as_bytes())
            .and_then(|()| writer.flush())
        {
            Ok(()) => bound.try_payload(RuntimeValue::Unit).map_or_else(
                |error| HostTaskCompletion::Failed(error.to_string()),
                HostTaskCompletion::Ready,
            ),
            Err(error) => HostTaskCompletion::Failed(error.to_string()),
        }
    }
}

fn file_task_completion(
    bound: &BoundTaskOutcome,
    result: Result<RuntimePayload, String>,
) -> HostTaskCompletion {
    match result {
        Ok(value) => bound.try_result_ok(value.value().clone()).map_or_else(
            |error| HostTaskCompletion::Failed(error.to_string()),
            HostTaskCompletion::Ready,
        ),
        Err(error) => {
            let Ok(Some(RuntimeCheckedType::Opaque { owner })) = bound.result_error_checked()
            else {
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
                Ok(value) => bound.try_result_err(value).map_or_else(
                    |error| HostTaskCompletion::Failed(error.to_string()),
                    HostTaskCompletion::Ready,
                ),
                Err(error) => HostTaskCompletion::Failed(format!(
                    "native file task could not materialize its domain error: {error}"
                )),
            }
        }
    }
}

fn complete_manifest_file_call(
    manifest: &AdapterManifest,
    roots: &NativeFileRoots,
    task: &TaskSpec,
    bound: &BoundTaskOutcome,
) -> HostTaskOutcome {
    let HostTaskRequest::Custom {
        capability,
        operation,
        manifest_contract,
        ..
    } = &task.request
    else {
        return failed_host_task("native file adapter expected a manifest-owned custom request");
    };
    if capability.0 != "fs" {
        return failed_host_task(format!(
            "native file adapter does not implement capability `{}`",
            capability.0
        ));
    }

    let host_call_id = task.request.host_call_id();
    let Some(host_call) = manifest
        .host_calls()
        .iter()
        .find(|call| call.id() == host_call_id)
    else {
        return failed_host_task(format!(
            "native file adapter manifest does not define host call `{host_call_id}`"
        ));
    };
    if *manifest_contract != Some(host_call.contract_digest()) {
        return failed_host_task(format!(
            "host call `{host_call_id}` does not carry its exact native file manifest contract"
        ));
    }

    let names = match manifest_argument_names(host_call) {
        Ok(names) => names,
        Err(message) => return failed_host_task(message),
    };
    let arguments = match manifest_custom_arguments(&task.request, &names) {
        Ok(arguments) => arguments,
        Err(message) => return failed_host_task(message),
    };

    let (result, metrics) = match operation.as_str() {
        "read_text" => match arguments.as_slice() {
            [path] => match custom_file_path(path) {
                Ok(path) => complete_read_text(roots, &path),
                Err(message) => return failed_host_task(message),
            },
            _ => return failed_host_task("fs.read_text manifest requires exactly one argument"),
        },
        "read_bytes" => match arguments.as_slice() {
            [path] => match custom_file_path(path) {
                Ok(path) => complete_read_bytes(roots, &path),
                Err(message) => return failed_host_task(message),
            },
            _ => return failed_host_task("fs.read_bytes manifest requires exactly one argument"),
        },
        "write_text" => match arguments.as_slice() {
            [path, body] => {
                let path = match custom_file_path(path) {
                    Ok(path) => path,
                    Err(message) => return failed_host_task(message),
                };
                let RuntimeValue::String(body) = body else {
                    return failed_host_task("fs.write_text body must be String");
                };
                complete_write_text(roots, &path, body)
            }
            _ => return failed_host_task("fs.write_text manifest requires exactly two arguments"),
        },
        "write_bytes" => match arguments.as_slice() {
            [path, body] => {
                let path = match custom_file_path(path) {
                    Ok(path) => path,
                    Err(message) => return failed_host_task(message),
                };
                let body = match custom_file_bytes(body) {
                    Ok(body) => body,
                    Err(message) => return failed_host_task(message),
                };
                complete_write_bytes(roots, &path, &body)
            }
            _ => {
                return failed_host_task("fs.write_bytes manifest requires exactly two arguments");
            }
        },
        _ => {
            return failed_host_task(format!(
                "native file adapter does not implement host call `{host_call_id}`"
            ));
        }
    };

    HostTaskOutcome {
        completion: file_task_completion(bound, result),
        metrics,
    }
}

fn manifest_argument_names(host_call: &AdapterHostCall) -> Result<Vec<String>, String> {
    let [group] = host_call.signature().groups() else {
        return Err(format!(
            "host call `{}` has an unsupported native file parameter-group contract",
            host_call.id()
        ));
    };
    group
        .parameters()
        .iter()
        .map(|parameter| {
            parameter
                .name()
                .map(|name| name.as_str().to_owned())
                .ok_or_else(|| {
                    format!(
                        "host call `{}` has an unnamed native file parameter",
                        host_call.id()
                    )
                })
        })
        .collect()
}

fn manifest_custom_arguments<'a>(
    request: &'a HostTaskRequest,
    parameter_names: &[String],
) -> Result<Vec<&'a RuntimeValue>, String> {
    let HostTaskRequest::Custom {
        args, named_args, ..
    } = request
    else {
        return Err("native file adapter expected a custom host request".to_owned());
    };
    if args.len() + named_args.len() != parameter_names.len() {
        return Err(format!(
            "custom file request has {} argument(s), manifest requires {}",
            args.len() + named_args.len(),
            parameter_names.len()
        ));
    }

    let mut values = vec![None; parameter_names.len()];
    for (index, argument) in args.iter().enumerate() {
        values[index] = Some(argument.value());
    }
    for argument in named_args {
        let Some(index) = parameter_names
            .iter()
            .position(|name| name == &argument.name)
        else {
            return Err(format!(
                "custom file request contains unknown argument `{}`",
                argument.name
            ));
        };
        if values[index].replace(argument.value.value()).is_some() {
            return Err(format!(
                "custom file request supplies argument `{}` more than once",
                argument.name
            ));
        }
    }

    values
        .into_iter()
        .zip(parameter_names)
        .map(|(value, name)| {
            value.ok_or_else(|| format!("custom file request is missing argument `{name}`"))
        })
        .collect()
}

fn custom_file_path(value: &RuntimeValue) -> Result<String, String> {
    RuntimeVirtualPath::try_from(value)
        .map(|path| path.runtime_label())
        .map_err(|error| format!("custom file request path is invalid: {error}"))
}

fn custom_file_bytes(value: &RuntimeValue) -> Result<Vec<u8>, String> {
    let RuntimeValue::Seq(sequence) = value else {
        return Err("fs.write_bytes body must be a byte sequence".to_owned());
    };
    if let Some(bytes) = sequence.as_bytes() {
        return Ok(bytes.to_vec());
    }
    let Some(values) = sequence.as_values() else {
        return Err("fs.write_bytes body must be a byte sequence".to_owned());
    };
    values
        .iter()
        .map(|value| match value {
            RuntimeValue::UInt(value) if value.width() == RuntimeUnsignedIntWidth::U8 => value
                .try_into_i64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| "fs.write_bytes body contains a value outside u8".to_owned()),
            _ => Err("fs.write_bytes body must contain only u8 values".to_owned()),
        })
        .collect()
}

fn failed_host_task(message: impl Into<String>) -> HostTaskOutcome {
    HostTaskOutcome {
        completion: HostTaskCompletion::Failed(message.into()),
        metrics: HostTaskMetrics::default(),
    }
}

impl HostAdapter for NativeSystemInfoAdapter {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn complete(&self, task: &TaskSpec, bound: &BoundTaskOutcome) -> Option<HostTaskOutcome> {
        let HostTaskRequest::SystemInfo(request) = &task.request else {
            return None;
        };
        Some(HostTaskOutcome {
            completion: bound
                .try_result_ok(RuntimeValue::String(
                    system_info_value(self.host_system, request.kind).to_string(),
                ))
                .map_or_else(
                    |error| HostTaskCompletion::Failed(error.to_string()),
                    HostTaskCompletion::Ready,
                ),
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

    fn complete(&self, task: &TaskSpec, bound: &BoundTaskOutcome) -> Option<HostTaskOutcome> {
        is_scheduler_marker_task(&task.request).then(|| HostTaskOutcome {
            completion: bound.try_payload(RuntimeValue::Unit).map_or_else(
                |error| HostTaskCompletion::Failed(error.to_string()),
                HostTaskCompletion::Ready,
            ),
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
    tasks: &[BoundTaskSpec],
    submission_contexts: &BTreeMap<TaskId, HostTaskSubmissionContext>,
) -> TaskCompletions {
    let parallel = should_complete_in_parallel(registry, tasks);
    let items = if parallel {
        tasks
            .par_iter()
            .filter_map(|task| {
                complete_task(registry, task, *submission_contexts.get(&task.spec().id)?)
            })
            .collect()
    } else {
        tasks
            .iter()
            .filter_map(|task| {
                complete_task(registry, task, *submission_contexts.get(&task.spec().id)?)
            })
            .collect()
    };
    TaskCompletions { parallel, items }
}

fn should_complete_in_parallel(registry: &HostAdapterRegistry, tasks: &[BoundTaskSpec]) -> bool {
    tasks.len() > 1
        && tasks
            .iter()
            .all(|task| registry.can_complete_in_parallel(&task.spec().request))
        && tasks
            .iter()
            .any(|task| is_parallel_host_work(&task.spec().request))
}

fn complete_task(
    registry: &HostAdapterRegistry,
    task: &BoundTaskSpec,
    context: HostTaskSubmissionContext,
) -> Option<TaskCompletion> {
    match registry.submit(task.spec(), task.outcome(), context) {
        Some(HostTaskSubmission::Completed(outcome)) => Some(TaskCompletion {
            task_id: task.spec().id.clone(),
            completion: match outcome.completion {
                HostTaskCompletion::Progress(_) => HostTaskCompletion::Failed(
                    "adapter returned progress from an immediate task submission".to_owned(),
                ),
                terminal => terminal,
            },
            publication_revision: context.next_publication_revision(),
            stats: outcome.metrics,
        }),
        Some(HostTaskSubmission::Pending) => None,
        None => Some(TaskCompletion {
            task_id: task.spec().id.clone(),
            completion: HostTaskCompletion::Failed(format!(
                "adapter rejected the registered host-call request `{}`",
                task.spec().request.host_call_id()
            )),
            publication_revision: context.next_publication_revision(),
            stats: HostTaskMetrics::default(),
        }),
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
    is_file_read_request(request)
}

fn is_file_read_request(request: &HostTaskRequest) -> bool {
    matches!(
        request,
        HostTaskRequest::FileReadText(_) | HostTaskRequest::FileReadBytes(_)
    ) || matches!(
        request,
        HostTaskRequest::Custom {
            capability,
            operation,
            ..
        } if capability.0 == "fs" && matches!(operation.as_str(), "read_text" | "read_bytes")
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
    use arcweft_core::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
    use arcweft_core::plan::{
        RuntimePlanBuilder, RuntimePlanSequenceKind, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    };
    use arcweft_core::task::{
        CancelScopeId, GenerationId, HostTaskRequest, LogicalEpoch, Progress, SystemInfoKind,
        SystemInfoRequest, TaskClass, TaskDispatchIdentity, TaskDispatchStart, TaskId, TaskKey,
        TaskOutcomeContract, TaskPolicy, TaskPriority, TaskPublicationRevision, TaskSequence,
    };
    use arcweft_core::value::runtime_sequence_dense_u8;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NATIVE_FILE_TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempNativeFileRoot(PathBuf);

    impl TempNativeFileRoot {
        fn new() -> Self {
            let index = NATIVE_FILE_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "arcweft-native-file-custom-{}-{index}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("native file test root is created");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempNativeFileRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn native_dispatch(start: TaskDispatchStart, task: TaskSpec) -> NativeTaskDispatch {
        NativeTaskDispatch::new(start, task, None)
    }

    #[derive(Debug)]
    struct PendingWrongValueAdapter {
        manifest: AdapterManifest,
        completions: Mutex<Vec<HostAdapterCompletion>>,
    }

    #[derive(Debug)]
    struct RevisionPublishingAdapter {
        manifest: AdapterManifest,
        completions: Mutex<Vec<HostAdapterCompletion>>,
    }

    #[derive(Debug)]
    struct RestoreRevisionAdapter {
        manifest: AdapterManifest,
        complete_on_submit: bool,
        completions: Mutex<Vec<HostAdapterCompletion>>,
    }

    #[derive(Debug)]
    struct DecliningAdapter {
        manifest: AdapterManifest,
    }

    impl HostAdapter for DecliningAdapter {
        fn manifest(&self) -> &AdapterManifest {
            &self.manifest
        }

        fn can_complete_in_parallel(&self, _request: &HostTaskRequest) -> bool {
            false
        }
    }

    impl HostAdapter for RevisionPublishingAdapter {
        fn manifest(&self) -> &AdapterManifest {
            &self.manifest
        }

        fn submit(
            &self,
            task: &TaskSpec,
            _outcome: &BoundTaskOutcome,
            context: HostTaskSubmissionContext,
        ) -> Option<HostTaskSubmission> {
            let first = context.next_publication_revision();
            let second = first.checked_next().expect("revision space remains");
            let third = second.checked_next().expect("revision space remains");
            let completion = |completion| HostTaskOutcome {
                completion,
                metrics: HostTaskMetrics::default(),
            };
            self.completions.lock().expect("completion queue").extend([
                HostAdapterCompletion {
                    task_id: task.id.clone(),
                    publication_revision: first,
                    outcome: completion(HostTaskCompletion::Progress(
                        Progress::new(0.25).expect("valid progress"),
                    )),
                },
                HostAdapterCompletion {
                    task_id: task.id.clone(),
                    publication_revision: second,
                    outcome: completion(HostTaskCompletion::Progress(
                        Progress::new(0.75).expect("valid progress"),
                    )),
                },
                HostAdapterCompletion {
                    task_id: task.id.clone(),
                    publication_revision: third,
                    outcome: completion(HostTaskCompletion::Ready(RuntimePayload::from("done"))),
                },
            ]);
            Some(HostTaskSubmission::Pending)
        }

        fn drain_completions(&self) -> Vec<HostAdapterCompletion> {
            std::mem::take(&mut *self.completions.lock().expect("completion queue"))
        }

        fn can_complete_in_parallel(&self, _request: &HostTaskRequest) -> bool {
            false
        }
    }

    impl HostAdapter for PendingWrongValueAdapter {
        fn manifest(&self) -> &AdapterManifest {
            &self.manifest
        }

        fn submit(
            &self,
            task: &TaskSpec,
            _outcome: &BoundTaskOutcome,
            context: HostTaskSubmissionContext,
        ) -> Option<HostTaskSubmission> {
            self.completions
                .lock()
                .expect("completion queue")
                .push(HostAdapterCompletion {
                    task_id: task.id.clone(),
                    publication_revision: context.next_publication_revision(),
                    outcome: HostTaskOutcome {
                        completion: HostTaskCompletion::Ready(RuntimePayload(RuntimeValue::Bool(
                            true,
                        ))),
                        metrics: HostTaskMetrics::default(),
                    },
                });
            Some(HostTaskSubmission::Pending)
        }

        fn drain_completions(&self) -> Vec<HostAdapterCompletion> {
            std::mem::take(&mut *self.completions.lock().expect("completion queue"))
        }

        fn can_complete_in_parallel(&self, _request: &HostTaskRequest) -> bool {
            false
        }
    }

    impl HostAdapter for RestoreRevisionAdapter {
        fn manifest(&self) -> &AdapterManifest {
            &self.manifest
        }

        fn submit(
            &self,
            task: &TaskSpec,
            _outcome: &BoundTaskOutcome,
            context: HostTaskSubmissionContext,
        ) -> Option<HostTaskSubmission> {
            if self.complete_on_submit {
                return Some(HostTaskSubmission::Completed(HostTaskOutcome {
                    completion: HostTaskCompletion::Ready(RuntimePayload::from("restored-done")),
                    metrics: HostTaskMetrics::default(),
                }));
            }
            let first = context.next_publication_revision();
            let second = first.checked_next().expect("revision space remains");
            self.completions.lock().expect("completion queue").extend(
                [(first, 0.25), (second, 0.75)].map(|(publication_revision, ratio)| {
                    HostAdapterCompletion {
                        task_id: task.id.clone(),
                        publication_revision,
                        outcome: HostTaskOutcome {
                            completion: HostTaskCompletion::Progress(
                                Progress::new(ratio).expect("valid progress"),
                            ),
                            metrics: HostTaskMetrics::default(),
                        },
                    }
                }),
            );
            Some(HostTaskSubmission::Pending)
        }

        fn drain_completions(&self) -> Vec<HostAdapterCompletion> {
            std::mem::take(&mut *self.completions.lock().expect("completion queue"))
        }

        fn can_complete_in_parallel(&self, _request: &HostTaskRequest) -> bool {
            false
        }
    }

    #[test]
    fn pending_task_completion_is_checked_against_its_retained_outcome() {
        let manifest = AdapterManifest::new("pending-wrong-value", "Pending Wrong Value")
            .with_host_call(AdapterHostCall::new("pending.echo", []));
        let registry = HostAdapterRegistry::builder()
            .register(PendingWrongValueAdapter {
                manifest: manifest.clone(),
                completions: Mutex::new(Vec::new()),
            })
            .expect("pending adapter")
            .build();
        let mut bridge = NativeTaskBridge::try_with_registry(
            NativeTaskBridge::policy_from_manifest(&manifest),
            registry,
        )
        .expect("implemented policy");
        let pending = TaskSpec::new(
            TaskId("pending-value".to_owned()),
            TaskKey("pending-value".to_owned()),
            TaskClass::Cpu,
            TaskPriority(0),
            CancelScopeId("test".to_owned()),
            TaskPolicy::JoinSameKey,
            HostTaskRequest::custom("pending", "echo", []),
        )
        .with_outcome(TaskOutcomeContract::new(RuntimeCheckedType::String));

        assert!(
            bridge
                .complete_tasks(standalone_test_program(), vec![pending])
                .expect("pending task submission")
                .is_empty()
        );
        let events = bridge.poll_completions().expect("pending completion");
        assert!(
            matches!(events.as_slice(), [TaskEvent { kind: TaskEventKind::Failed(message), .. }] if message.contains("standalone task outcome rejected"))
        );
        assert!(bridge.poll_completions().unwrap().is_empty());
    }

    #[test]
    fn registered_adapter_decline_publishes_a_terminal_failure() {
        let manifest = AdapterManifest::new("declining-adapter", "Declining Adapter")
            .with_host_call(AdapterHostCall::new("decline.echo", []));
        let registry = HostAdapterRegistry::builder()
            .register(DecliningAdapter {
                manifest: manifest.clone(),
            })
            .expect("declining adapter")
            .build();
        let mut bridge = NativeTaskBridge::try_with_registry(
            NativeTaskBridge::policy_from_manifest(&manifest),
            registry,
        )
        .expect("registered call implements selected policy");
        let request = TaskSpec::new(
            TaskId("declined-request".to_owned()),
            TaskKey("declined-request".to_owned()),
            TaskClass::Cpu,
            TaskPriority(0),
            CancelScopeId("test".to_owned()),
            TaskPolicy::AlwaysStart,
            HostTaskRequest::custom("decline", "echo", []),
        )
        .with_outcome(TaskOutcomeContract::new(RuntimeCheckedType::String));

        let events = bridge
            .complete_tasks(standalone_test_program(), vec![request])
            .expect("declined registered request becomes an event");

        assert!(matches!(
            events.as_slice(),
            [TaskEvent {
                kind: TaskEventKind::Failed(message),
                ..
            }] if message.contains("adapter rejected the registered host-call request `decline.echo`")
        ));
        assert_eq!(bridge.stats().failed_tasks, 1);
        assert_eq!(bridge.stats().scheduler.in_flight, 0);
    }

    #[test]
    fn progress_revisions_survive_bridge_and_joined_waiters_keep_their_dispatch() {
        let manifest = AdapterManifest::new("progress-adapter", "Progress Adapter")
            .with_host_call(AdapterHostCall::new("progress.echo", []));
        let registry = HostAdapterRegistry::builder()
            .register(RevisionPublishingAdapter {
                manifest: manifest.clone(),
                completions: Mutex::new(Vec::new()),
            })
            .expect("progress adapter")
            .build();
        let mut bridge = NativeTaskBridge::try_with_registry(
            NativeTaskBridge::policy_from_manifest(&manifest),
            registry,
        )
        .expect("implemented policy");
        let make_task = |id: &str| {
            TaskSpec::new(
                TaskId(id.to_owned()),
                TaskKey("shared-progress-task".to_owned()),
                TaskClass::Cpu,
                TaskPriority(0),
                CancelScopeId("test".to_owned()),
                TaskPolicy::JoinSameKey,
                HostTaskRequest::custom("progress", "echo", []),
            )
            .with_outcome(TaskOutcomeContract::new(RuntimeCheckedType::String))
        };
        let owner_identity = TaskDispatchIdentity::new(
            GenerationId::new(3),
            LogicalEpoch(5),
            TaskSequence(11),
            TaskId("progress-owner".to_owned()),
        );
        let waiter_identity = TaskDispatchIdentity::new(
            GenerationId::new(8),
            LogicalEpoch(9),
            TaskSequence(27),
            TaskId("progress-waiter".to_owned()),
        );

        assert!(
            bridge
                .complete_tasks_with_dispatches(
                    standalone_test_program(),
                    vec![native_dispatch(
                        TaskDispatchStart::new(owner_identity.clone(), None),
                        make_task("progress-owner"),
                    )],
                )
                .expect("owner starts")
                .is_empty()
        );
        assert!(
            bridge
                .complete_tasks_with_dispatches(
                    standalone_test_program(),
                    vec![native_dispatch(
                        TaskDispatchStart::new(waiter_identity.clone(), None),
                        make_task("progress-waiter"),
                    )],
                )
                .expect("waiter joins")
                .is_empty()
        );

        let events = bridge.poll_completions().expect("three host publications");
        assert_eq!(events.len(), 6);
        let expected = [
            (&owner_identity, 1),
            (&owner_identity, 2),
            (&owner_identity, 3),
            (&waiter_identity, 1),
            (&waiter_identity, 2),
            (&waiter_identity, 3),
        ];
        for (event, (identity, revision)) in events.iter().zip(expected) {
            assert_eq!(event.dispatch_identity(), identity.clone());
            assert_eq!(event.publication_revision.get(), revision);
        }
        assert!(matches!(
            events[0].kind,
            TaskEventKind::Progress(ref progress) if progress.ratio() == 0.25
        ));
        assert!(matches!(
            events[1].kind,
            TaskEventKind::Progress(ref progress) if progress.ratio() == 0.75
        ));
        assert!(matches!(
            events[2].kind,
            TaskEventKind::Ready(ref value) if value.value() == &RuntimeValue::String("done".to_owned())
        ));
        assert!(matches!(
            events[3].kind,
            TaskEventKind::Progress(ref progress) if progress.ratio() == 0.25
        ));
        assert!(matches!(
            events[4].kind,
            TaskEventKind::Progress(ref progress) if progress.ratio() == 0.75
        ));
        assert!(matches!(
            events[5].kind,
            TaskEventKind::Ready(ref value) if value.value() == &RuntimeValue::String("done".to_owned())
        ));
    }

    #[test]
    fn restored_host_bridge_continues_publication_revision_frontier() {
        let manifest = AdapterManifest::new("restore-adapter", "Restore Adapter")
            .with_host_call(AdapterHostCall::new("restore.echo", []));
        let task = TaskSpec::new(
            TaskId("restore-progress-task".to_owned()),
            TaskKey("restore-progress-task".to_owned()),
            TaskClass::Cpu,
            TaskPriority(0),
            CancelScopeId("test".to_owned()),
            TaskPolicy::AlwaysStart,
            HostTaskRequest::custom("restore", "echo", []),
        )
        .with_outcome(TaskOutcomeContract::new(RuntimeCheckedType::String));
        let identity = TaskDispatchIdentity::new(
            GenerationId::new(14),
            LogicalEpoch(23),
            TaskSequence(5),
            task.id.clone(),
        );

        let initial_registry = HostAdapterRegistry::builder()
            .register(RestoreRevisionAdapter {
                manifest: manifest.clone(),
                complete_on_submit: false,
                completions: Mutex::new(Vec::new()),
            })
            .expect("initial adapter")
            .build();
        let mut initial = NativeTaskBridge::try_with_registry(
            NativeTaskBridge::policy_from_manifest(&manifest),
            initial_registry,
        )
        .expect("implemented policy");
        assert!(
            initial
                .complete_tasks_with_dispatches(
                    standalone_test_program(),
                    vec![native_dispatch(
                        TaskDispatchStart::new(identity.clone(), None),
                        task.clone(),
                    )],
                )
                .expect("initial start")
                .is_empty()
        );
        let progress = initial.poll_completions().expect("two progress events");
        assert_eq!(progress.len(), 2);
        assert_eq!(progress[0].publication_revision.get(), 1);
        assert_eq!(progress[1].publication_revision.get(), 2);
        assert!(
            progress
                .iter()
                .all(|event| matches!(event.kind, TaskEventKind::Progress(_)))
        );

        // A restored bridge has no in-memory scheduler or adapter task. The
        // exact saved dispatch and publication frontier resume at revision 3.
        let restored_registry = HostAdapterRegistry::builder()
            .register(RestoreRevisionAdapter {
                manifest: manifest.clone(),
                complete_on_submit: true,
                completions: Mutex::new(Vec::new()),
            })
            .expect("restored adapter")
            .build();
        let mut restored = NativeTaskBridge::try_with_registry(
            NativeTaskBridge::policy_from_manifest(&manifest),
            restored_registry,
        )
        .expect("implemented policy");
        let ready = restored
            .complete_tasks_with_dispatches(
                standalone_test_program(),
                vec![native_dispatch(
                    TaskDispatchStart::new(
                        identity.clone(),
                        Some(TaskPublicationRevision::new(
                            std::num::NonZeroU64::new(2).expect("nonzero revision"),
                        )),
                    ),
                    task,
                )],
            )
            .expect("restored re-ensure");
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].dispatch_identity(), identity);
        assert_eq!(ready[0].publication_revision.get(), 3);
        assert!(matches!(
            &ready[0].kind,
            TaskEventKind::Ready(value)
                if value.value() == &RuntimeValue::String("restored-done".to_owned())
        ));
    }

    #[test]
    fn dispatch_bridge_rejects_misbound_and_duplicate_task_ids_atomically() {
        let registry = HostAdapterRegistry::builder().build();
        let mut bridge = NativeTaskBridge::try_with_registry(HostCallPolicy::default(), registry)
            .expect("empty policy needs no adapters");
        let task = TaskSpec::new(
            TaskId("bridge-duplicate".to_owned()),
            TaskKey("bridge-duplicate".to_owned()),
            TaskClass::Cpu,
            TaskPriority(0),
            CancelScopeId("test".to_owned()),
            TaskPolicy::AlwaysStart,
            HostTaskRequest::custom("bridge", "echo", []),
        )
        .with_outcome(TaskOutcomeContract::new(RuntimeCheckedType::String));
        let identity = TaskDispatchIdentity::new(
            GenerationId::new(0),
            LogicalEpoch(0),
            TaskSequence(1),
            task.id.clone(),
        );
        let mismatched = TaskDispatchIdentity::new(
            GenerationId::new(0),
            LogicalEpoch(0),
            TaskSequence(1),
            TaskId("different-id".to_owned()),
        );

        assert!(matches!(
            bridge.complete_tasks_with_dispatches(
                standalone_test_program(),
                vec![native_dispatch(
                    TaskDispatchStart::new(mismatched, None),
                    task.clone()
                )],
            ),
            Err(NativeTaskBridgeError::TaskCompletion(
                TaskCompletionError::DispatchMismatch { .. }
            ))
        ));
        assert!(matches!(
            bridge.complete_tasks_with_dispatches(
                standalone_test_program(),
                vec![
                    native_dispatch(TaskDispatchStart::new(identity.clone(), None), task.clone()),
                    native_dispatch(TaskDispatchStart::new(identity, None), task),
                ],
            ),
            Err(NativeTaskBridgeError::TaskSubmission(
                TaskEnsureError::TaskIdSpecificationConflict { .. }
            ))
        ));
        assert_eq!(bridge.stats().scheduler.submitted, 0);
    }

    #[test]
    fn conflicting_joined_task_result_does_not_replace_the_pending_owner() {
        let manifest = AdapterManifest::new("pending-join", "Pending Join")
            .with_host_call(AdapterHostCall::new("pending.echo", []));
        let registry = HostAdapterRegistry::builder()
            .register(PendingWrongValueAdapter {
                manifest: manifest.clone(),
                completions: Mutex::new(Vec::new()),
            })
            .unwrap()
            .build();
        let mut bridge = NativeTaskBridge::try_with_registry(
            NativeTaskBridge::policy_from_manifest(&manifest),
            registry,
        )
        .unwrap();
        let make_task = |id: &str, payload| {
            TaskSpec::new(
                TaskId(id.to_owned()),
                TaskKey("same-producer".to_owned()),
                TaskClass::Cpu,
                TaskPriority(0),
                CancelScopeId("test".to_owned()),
                TaskPolicy::JoinSameKey,
                HostTaskRequest::custom("pending", "echo", []),
            )
            .with_outcome(TaskOutcomeContract::new(payload))
        };

        assert!(
            bridge
                .complete_tasks(
                    standalone_test_program(),
                    vec![make_task("owner", RuntimeCheckedType::Bool)],
                )
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            bridge.complete_tasks(
                standalone_test_program(),
                vec![make_task("foreign-waiter", RuntimeCheckedType::String)],
            ),
            Err(NativeTaskBridgeError::TaskSubmission(
                TaskEnsureError::JoinSpecificationConflict { .. }
            ))
        ));
        let events = bridge.poll_completions().unwrap();
        assert!(
            matches!(events.as_slice(), [TaskEvent { task_id, kind: TaskEventKind::Ready(value), .. }] if task_id.0 == "owner" && value.value() == &RuntimeValue::Bool(true))
        );
    }

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
        let events = bridge
            .complete_tasks(
                standalone_test_program(),
                vec![task(
                    "missing",
                    HostTaskRequest::SystemInfo(SystemInfoRequest {
                        kind: SystemInfoKind::CoreCount,
                    }),
                )],
            )
            .expect("rejected host call event");

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
        let events = bridge
            .complete_tasks(
                standalone_test_program(),
                vec![task(
                    "system",
                    HostTaskRequest::SystemInfo(SystemInfoRequest {
                        kind: SystemInfoKind::AvailableParallelism,
                    }),
                )],
            )
            .expect("completed host call event");

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
            "cli.stdout",
            "cli.stderr",
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
        assert!(!policy.contains("cli.exit"));
        assert!(
            NativeTaskBridge::selected_policy_for_manifest(&standard::native_cli_manifest())
                .contains("cli.exit")
        );
    }

    #[test]
    fn native_cli_rejects_malformed_process_arguments_without_side_effects() {
        let adapter = NativeCliAdapter {
            manifest: standard::native_cli_manifest(),
            args: Box::new([]),
        };
        let exact_code =
            RuntimePayload::new(RuntimeValue::Int(arcweft_core::value::RuntimeInt::i32(7)));
        let wider_code =
            RuntimePayload::new(RuntimeValue::Int(arcweft_core::value::RuntimeInt::i64(7)));
        let never = TaskOutcomeContract::new(RuntimeCheckedType::Never)
            .bind_standalone()
            .unwrap();
        for request in [
            HostTaskRequest::custom_with_named_args("cli", "exit", [], []),
            HostTaskRequest::custom_with_named_args("cli", "exit", [wider_code], []),
            HostTaskRequest::custom_with_named_args("cli", "exit", [RuntimePayload::from("7")], []),
            HostTaskRequest::custom_with_named_args(
                "cli",
                "exit",
                [exact_code.clone(), exact_code.clone()],
                [],
            ),
            HostTaskRequest::custom_with_named_args(
                "cli",
                "exit",
                [],
                [("wrong".to_owned(), exact_code.clone())],
            ),
            HostTaskRequest::custom_with_named_args(
                "cli",
                "exit",
                [exact_code.clone()],
                [("code".to_owned(), exact_code)],
            ),
        ] {
            let outcome = adapter
                .complete(&task("bad-exit", request), &never)
                .unwrap();
            assert!(matches!(outcome.completion, HostTaskCompletion::Failed(_)));
        }
        let unit = TaskOutcomeContract::new(RuntimeCheckedType::Unit)
            .bind_standalone()
            .unwrap();
        for operation in ["stdout", "stderr"] {
            let request = HostTaskRequest::custom_with_named_args(
                "cli",
                operation,
                [RuntimePayload::new(RuntimeValue::Bool(true))],
                [],
            );
            let outcome = adapter
                .complete(&task("bad-output", request), &unit)
                .unwrap();
            assert!(matches!(outcome.completion, HostTaskCompletion::Failed(_)));
        }
    }

    #[test]
    fn native_cli_exit_requires_exact_manifest_and_never_result_before_dispatch() {
        let source_path = std::env::temp_dir().join("arcweft-native-cli-exit-contract.arcw");
        let manifest = standard::native_cli_manifest();
        let call = manifest
            .host_calls()
            .iter()
            .find(|call| call.id() == "cli.exit")
            .unwrap();
        let result = RuntimeCheckedType::Never.semantic_identity_digest();
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    result,
                    RuntimePlanTypeProjection::Never,
                )],
                [],
            )
            .unwrap();
        let program = RuntimeProgramOwner::Plan(std::sync::Arc::new(builder.finish().unwrap()));
        for (contract, requested_result) in [
            (None, result),
            (
                Some(arcweft_core::step::HostCallContractDigest::from_bytes(
                    [0xa5; 32],
                )),
                result,
            ),
            (
                Some(call.contract_digest()),
                RuntimeCheckedType::Unit.semantic_identity_digest(),
            ),
        ] {
            let mut bridge = NativeTaskBridge::try_new(
                &source_path,
                NativeFileRoots::for_source(&source_path),
                &[],
                NativeTaskBridge::selected_policy_for_manifest(&manifest),
                &[],
            )
            .unwrap();
            let results = bridge.complete_host_calls(
                program.clone(),
                vec![RuntimeHostCallRequest {
                    id: arcweft_core::step::RuntimeHostCallId("cli.exit.invalid".to_owned()),
                    public_id: "cli.exit".to_owned(),
                    capability: "cli".to_owned(),
                    operation: "exit".to_owned(),
                    contract,
                    args: vec![RuntimePayload::new(RuntimeValue::Int(
                        arcweft_core::value::RuntimeInt::i32(7),
                    ))],
                    named_args: Vec::new(),
                    result: requested_result,
                    mode: RuntimeHostCallMode::Immediate,
                    deterministic: true,
                }],
            );
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

        let result_type = bridge
            .registry
            .host_call_result_type("cli.args")
            .expect("registered result type");
        let results = bridge.complete_host_calls(
            cli_test_program(result_type),
            vec![RuntimeHostCallRequest {
                id: arcweft_core::step::RuntimeHostCallId("cli.args.0".to_owned()),
                public_id: "cli.args".to_owned(),
                capability: "cli".to_owned(),
                operation: "args".to_owned(),
                contract: Some(standard::native_cli_manifest().host_calls()[0].contract_digest()),
                args: Vec::new(),
                named_args: Vec::new(),
                result: result_type,
                mode: RuntimeHostCallMode::Immediate,
                deterministic: true,
            }],
        );

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

        let result_type = bridge
            .registry
            .host_call_result_type("cli.args")
            .expect("registered result type");
        let results = bridge.complete_host_calls(
            cli_test_program(result_type),
            vec![RuntimeHostCallRequest {
                id: arcweft_core::step::RuntimeHostCallId("cli.args.0".to_owned()),
                public_id: "cli.args".to_owned(),
                capability: "cli".to_owned(),
                operation: "args".to_owned(),
                contract: Some(arcweft_core::step::HostCallContractDigest::from_bytes(
                    [0xa5; 32],
                )),
                args: Vec::new(),
                named_args: Vec::new(),
                result: result_type,
                mode: RuntimeHostCallMode::Immediate,
                deterministic: true,
            }],
        );

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

        let result_type = bridge
            .registry
            .host_call_result_type("cli.args")
            .expect("registered result type");
        let results = bridge.complete_host_calls(
            cli_test_program(result_type),
            vec![RuntimeHostCallRequest {
                id: arcweft_core::step::RuntimeHostCallId("cli.args.0".to_owned()),
                public_id: "cli.args".to_owned(),
                capability: "cli".to_owned(),
                operation: "args".to_owned(),
                contract: Some(manifest.host_calls()[0].contract_digest()),
                args: Vec::new(),
                named_args: Vec::new(),
                result: RuntimeCheckedType::String.semantic_identity_digest(),
                mode: RuntimeHostCallMode::Immediate,
                deterministic: true,
            }],
        );

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
    fn selected_native_policy_does_not_grant_unselected_host_calls() {
        let empty = NativeTaskBridge::selected_policy_for_manifest(&standard::sans_io_manifest());
        assert!(!empty.contains("path.save"));
        assert!(!empty.contains("cli.args"));
        assert!(empty.contains("flow_thread.run_child"));
        let file =
            NativeTaskBridge::selected_policy_for_manifest(&standard::native_file_manifest());
        assert!(file.contains("path.save"));
        assert!(file.contains("fs.read_text"));
        assert!(!file.contains("cli.args"));
    }

    #[test]
    fn native_path_constructor_returns_the_exact_standard_opaque_value() {
        let adapter = NativeFileAdapter {
            manifest: standard::native_file_manifest(),
            roots: NativeFileRoots::new("assets", "state"),
        };
        let bound = TaskOutcomeContract::new(RuntimeCheckedType::Opaque {
            owner: RuntimeVirtualPath::exact_owner(),
        })
        .bind_standalone()
        .unwrap();
        for space in ["save", "asset", "temp", "export"] {
            let request = HostTaskRequest::custom_with_named_args(
                "path",
                space,
                [RuntimePayload::from("nested/profile.json")],
                [],
            );
            let result = adapter.complete(&task("path", request), &bound).unwrap();
            let HostTaskCompletion::Ready(value) = result.completion else {
                panic!("path constructor failed");
            };
            let path = RuntimeVirtualPath::try_from(value.value()).unwrap();
            assert_eq!(path.runtime_label(), format!("{space}:nested/profile.json"));
        }
    }

    #[test]
    fn native_file_adapter_completes_manifest_bound_custom_file_calls() {
        let temporary = TempNativeFileRoot::new();
        let assets = temporary.path().join("assets");
        let state = temporary.path().join("state");
        let save = state.join("save");
        fs::create_dir_all(&save).expect("save directory is created");
        fs::write(save.join("profile.txt"), "existing text").expect("text fixture is written");
        fs::write(save.join("profile.bin"), [1_u8, 2, 3]).expect("byte fixture is written");

        let manifest = standard::native_file_manifest();
        let adapter = NativeFileAdapter {
            manifest: manifest.clone(),
            roots: NativeFileRoots::new(&assets, &state),
        };
        let path = |relative: &str| {
            RuntimeVirtualPath::new(RuntimeVirtualPathSpace::Save, relative).into_value()
        };
        let string_result = || result_type(RuntimeCheckedType::String);
        let byte_sequence = || {
            RuntimeCheckedType::Sequence(Box::new(RuntimeCheckedType::Unsigned(
                RuntimeUnsignedIntWidth::U8,
            )))
        };

        let (read_text, read_text_bound) = manifest_file_task(
            &manifest,
            "read-text",
            "read_text",
            [("path", path("profile.txt"))],
            string_result(),
        );
        let read_text_outcome = adapter
            .complete(&read_text, &read_text_bound)
            .expect("manifest-bound custom read_text is implemented");
        assert_eq!(
            read_text_outcome.completion,
            HostTaskCompletion::Ready(RuntimePayload::new(RuntimeValue::result_ok(
                RuntimeValue::String("existing text".to_owned())
            )))
        );
        assert_eq!(read_text_outcome.metrics.read_ops, 1);

        let (read_bytes, read_bytes_bound) = manifest_file_task(
            &manifest,
            "read-bytes",
            "read_bytes",
            [("path", path("profile.bin"))],
            result_type(byte_sequence()),
        );
        let read_bytes_outcome = adapter
            .complete(&read_bytes, &read_bytes_bound)
            .expect("manifest-bound custom read_bytes is implemented");
        assert_eq!(
            read_bytes_outcome.completion,
            HostTaskCompletion::Ready(RuntimePayload::new(RuntimeValue::result_ok(
                runtime_sequence_dense_bytes(vec![1, 2, 3])
            )))
        );
        assert_eq!(read_bytes_outcome.metrics.bytes_read, 3);

        let (write_text, write_text_bound) = manifest_file_task(
            &manifest,
            "write-text",
            "write_text",
            [
                ("path", path("nested/output.txt")),
                ("body", RuntimeValue::String("written text".to_owned())),
            ],
            result_type(RuntimeCheckedType::Unit),
        );
        let write_text_outcome = adapter
            .complete(&write_text, &write_text_bound)
            .expect("manifest-bound custom write_text is implemented");
        assert_eq!(
            write_text_outcome.completion,
            HostTaskCompletion::Ready(RuntimePayload::new(RuntimeValue::result_ok(
                RuntimeValue::Unit
            )))
        );
        assert_eq!(
            fs::read_to_string(save.join("nested/output.txt")).expect("text output exists"),
            "written text"
        );

        let (write_bytes, write_bytes_bound) = manifest_file_task(
            &manifest,
            "write-bytes",
            "write_bytes",
            [
                ("path", path("nested/output.bin")),
                ("body", runtime_sequence_dense_u8(vec![4, 5, 6])),
            ],
            result_type(RuntimeCheckedType::Unit),
        );
        let write_bytes_outcome = adapter
            .complete(&write_bytes, &write_bytes_bound)
            .expect("manifest-bound custom write_bytes is implemented");
        assert_eq!(
            write_bytes_outcome.completion,
            HostTaskCompletion::Ready(RuntimePayload::new(RuntimeValue::result_ok(
                RuntimeValue::Unit
            )))
        );
        assert_eq!(
            fs::read(save.join("nested/output.bin")).expect("byte output exists"),
            vec![4, 5, 6]
        );

        for request in [&read_text, &read_bytes] {
            assert!(adapter.can_complete_in_parallel(&request.request));
            assert!(is_io_task(&request.request));
        }
        assert!(!adapter.can_complete_in_parallel(&write_text.request));
        assert!(!adapter.can_complete_in_parallel(&write_bytes.request));
    }

    #[test]
    fn native_file_custom_calls_reject_wrong_contract_and_argument_shape() {
        let temporary = TempNativeFileRoot::new();
        let manifest = standard::native_file_manifest();
        let adapter = NativeFileAdapter {
            manifest: manifest.clone(),
            roots: NativeFileRoots::new(temporary.path().join("assets"), temporary.path()),
        };
        let request_path =
            RuntimeVirtualPath::new(RuntimeVirtualPathSpace::Save, "profile.txt").into_value();
        let (request, bound) = manifest_file_task(
            &manifest,
            "wrong-contract",
            "read_text",
            [("path", request_path.clone())],
            result_type(RuntimeCheckedType::String),
        );
        let mut wrong_contract = request.clone();
        let HostTaskRequest::Custom {
            manifest_contract, ..
        } = &mut wrong_contract.request
        else {
            panic!("fixture request is custom");
        };
        *manifest_contract =
            Some(arcweft_adapter_context::manifest::HostCallContractDigest::from_bytes([0xff; 32]));
        assert!(matches!(
            adapter.complete(&wrong_contract, &bound).map(|outcome| outcome.completion),
            Some(HostTaskCompletion::Failed(message))
                if message.contains("exact native file manifest contract")
        ));

        let (wrong_arguments, wrong_arguments_bound) = manifest_file_task(
            &manifest,
            "wrong-arguments",
            "read_text",
            [("unexpected", request_path)],
            result_type(RuntimeCheckedType::String),
        );
        assert!(matches!(
            adapter
                .complete(&wrong_arguments, &wrong_arguments_bound)
                .map(|outcome| outcome.completion),
            Some(HostTaskCompletion::Failed(message))
                if message.contains("unknown argument `unexpected`")
        ));

        let (missing_arguments, missing_arguments_bound) = manifest_file_task(
            &manifest,
            "missing-arguments",
            "read_text",
            [],
            result_type(RuntimeCheckedType::String),
        );
        assert!(matches!(
            adapter
                .complete(&missing_arguments, &missing_arguments_bound)
                .map(|outcome| outcome.completion),
            Some(HostTaskCompletion::Failed(message))
                if message.contains("manifest requires 1")
        ));
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

    fn result_type(ok: RuntimeCheckedType) -> RuntimeCheckedType {
        RuntimeCheckedType::Result {
            ok: Box::new(ok),
            error: Box::new(RuntimeCheckedType::String),
        }
    }

    fn manifest_file_task(
        manifest: &AdapterManifest,
        id: &str,
        operation: &str,
        named_arguments: impl IntoIterator<Item = (&'static str, RuntimeValue)>,
        result_type: RuntimeCheckedType,
    ) -> (TaskSpec, BoundTaskOutcome) {
        let host_call_id = format!("fs.{operation}");
        let contract = manifest
            .host_calls()
            .iter()
            .find(|call| call.id() == host_call_id)
            .expect("file host call is in the selected manifest")
            .contract_digest();
        let outcome = TaskOutcomeContract::new(result_type);
        let bound = outcome
            .clone()
            .bind_standalone()
            .expect("standalone test outcome binds");
        let request = HostTaskRequest::custom_with_named_args_and_manifest_contract(
            "fs",
            operation,
            [],
            named_arguments
                .into_iter()
                .map(|(name, value)| (name.to_owned(), RuntimePayload::new(value))),
            contract,
        );
        let task = TaskSpec::new(
            TaskId(id.to_owned()),
            TaskKey(id.to_owned()),
            TaskClass::Io,
            TaskPriority(0),
            CancelScopeId("native-file-test".to_owned()),
            TaskPolicy::AlwaysStart,
            request,
        )
        .with_outcome(outcome);
        (task, bound)
    }

    fn standalone_test_program() -> RuntimeProgramOwner {
        RuntimeProgramOwner::Plan(std::sync::Arc::new(
            RuntimePlanBuilder::new().finish().expect("empty program"),
        ))
    }

    fn cli_test_program(result: RuntimeSemanticTypeId) -> RuntimeProgramOwner {
        let item = RuntimeCheckedType::String.semantic_identity_digest();
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(item, RuntimePlanTypeProjection::String),
                    RuntimePlanTypeSeed::new(
                        result,
                        RuntimePlanTypeProjection::Sequence {
                            kind: RuntimePlanSequenceKind::Vec,
                            item,
                        },
                    ),
                ],
                [],
            )
            .expect("CLI result graph");
        RuntimeProgramOwner::Plan(std::sync::Arc::new(builder.finish().expect("CLI program")))
    }
}
