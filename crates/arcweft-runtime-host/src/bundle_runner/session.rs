use super::{
    BundleRunnerError, BundleRunnerExecutor, BundleRunnerOptions, BundleRunnerPhase,
    BundleRunnerReport, BundleRunnerStepMode, BundleRunnerStepSummary, MaterializedBundleWorkspace,
    RuntimeExecutorInstance, bundle_host_policy, bundle_runner_bytecode, bundle_runner_entry,
    run_bundle_runner_phase, step_options, validate_bundle_image_assets, validate_bundle_kind,
};
use crate::native_task::{NativeTaskBridge, standard_cli_registry_builder};
use arcweft_bundle::ArcweftBundle;
use arcweft_core::{
    engine::{FlowFiberStatus, FlowStatusLabelStyle},
    step::RuntimeStepInput,
    task::TaskEvent,
    value::RuntimeBinding,
};
use arcweft_host_adapter::{HostAdapterError, HostAdapterRegistryBuilder};
use arcweft_interaction_model::audio::{AudioCommandEnvelope, AudioEvent};
use std::{path::Path, time::Instant};

/// One incremental bundle-runtime step executed by an embedding event loop.
#[derive(Clone, Debug)]
pub struct BundleRunnerSessionStep {
    pub summary: BundleRunnerStepSummary,
    pub audio_commands: Vec<AudioCommandEnvelope>,
    pub finished: bool,
}

/// Stateful bundle runner for hosts that must keep their UI event loop alive.
///
/// Unlike `run_bundle_with_native_adapters`, this type executes at most one
/// runtime step per call. `pump_main_thread` may also be called independently
/// while presentation is paused for user input. This is the boundary required
/// by native window-system APIs whose handles and mutations belong to the
/// event-loop thread.
pub struct BundleRunnerSession {
    _workspace: MaterializedBundleWorkspace,
    source: String,
    bytecode_instructions: usize,
    adapter_manifests: usize,
    phases: Vec<BundleRunnerPhase>,
    executor_kind: BundleRunnerExecutor,
    executor: RuntimeExecutorInstance,
    host: NativeTaskBridge,
    values: Vec<RuntimeBinding>,
    task_events: Vec<TaskEvent>,
    audio_events: Vec<AudioEvent>,
    mode: BundleRunnerStepMode,
    max_ops: usize,
    max_steps: usize,
    steps: Vec<BundleRunnerStepSummary>,
    run_started: Instant,
    finished: bool,
}

impl BundleRunnerSession {
    /// Creates a stateful bundle runner and lets the embedding host install
    /// adapters that capture event-loop-owned state.
    ///
    /// The installer is consumed synchronously. Captured handles therefore do
    /// not escape into a second setup phase, and the resulting registry remains
    /// the sole owner of the installed adapter set for the session lifetime.
    pub fn with_adapter_installer<F>(
        bundle: &ArcweftBundle,
        options: &BundleRunnerOptions,
        install: F,
    ) -> Result<Self, BundleRunnerError>
    where
        F: FnOnce(
            &Path,
            HostAdapterRegistryBuilder,
        ) -> Result<HostAdapterRegistryBuilder, HostAdapterError>,
    {
        let mut phases = Vec::new();
        run_bundle_runner_phase(&mut phases, "validate_bundle_kind", || {
            validate_bundle_kind(bundle)
        })?;
        run_bundle_runner_phase(&mut phases, "validate_image_assets", || {
            validate_bundle_image_assets(bundle)
        })?;
        let workspace = run_bundle_runner_phase(&mut phases, "materialize_bundle", || {
            MaterializedBundleWorkspace::create(bundle)
        })?;
        let selected_bytecode = run_bundle_runner_phase(&mut phases, "bytecode_decode", || {
            bundle_runner_bytecode(bundle, options)
        })?;
        let direct_bytecode =
            bundle_runner_entry(bundle, options).is_none() && options.flow.is_none();
        let bytecode = if direct_bytecode {
            bundle.bytecode.program.clone()
        } else {
            selected_bytecode
        };

        let policy = bundle_host_policy(bundle);
        let run_started = Instant::now();
        let builder = standard_cli_registry_builder(workspace.source_path())
            .map_err(BundleRunnerError::NativeAdapter)?;
        let registry = install(workspace.source_path(), builder)
            .map(HostAdapterRegistryBuilder::build)
            .map_err(BundleRunnerError::NativeAdapter)?;
        let host = NativeTaskBridge::try_with_registry(policy, registry)
            .map_err(BundleRunnerError::NativeAdapter)?;
        let executor = RuntimeExecutorInstance::from_bytecode(
            bytecode,
            options.executor,
            options.pure_config,
        )?;

        Ok(Self {
            _workspace: workspace,
            source: bundle.manifest.source_label.clone(),
            bytecode_instructions: bundle.manifest.runtime.bytecode_instructions,
            adapter_manifests: bundle.adapter_manifests.len(),
            phases,
            executor_kind: options.executor,
            executor,
            host,
            values: options.values.clone(),
            task_events: Vec::new(),
            audio_events: Vec::new(),
            mode: options.mode,
            max_ops: options.max_ops,
            max_steps: options.steps,
            steps: Vec::new(),
            run_started,
            finished: options.steps == 0,
        })
    }

    /// Runs queued `HostMainThread` adapter work and records deterministic task
    /// completion events without advancing the Arcweft runtime.
    pub fn pump_main_thread(&mut self) -> Result<usize, BundleRunnerError> {
        self.host
            .pump_main_thread()
            .map_err(BundleRunnerError::NativeAdapter)?;
        let completions = self.host.poll_completions();
        let completion_count = completions.len();
        self.task_events.extend(completions);
        Ok(completion_count)
    }

    /// Executes at most one Arcweft runtime step.
    pub fn step(&mut self) -> Result<Option<BundleRunnerSessionStep>, BundleRunnerError> {
        if self.finished {
            return Ok(None);
        }

        self.pump_main_thread()?;
        let index = self.steps.len();
        let result = self.executor.step_with_root_bindings(
            RuntimeStepInput {
                task_events: std::mem::take(&mut self.task_events),
                audio_events: std::mem::take(&mut self.audio_events),
                ..RuntimeStepInput::default()
            },
            &self.values,
            step_options(self.mode, self.max_ops),
        );
        let (summary, task_requests, audio_commands) =
            BundleRunnerStepSummary::from_result(index, result);
        let runtime_finished = matches!(
            self.executor.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        if !runtime_finished {
            self.task_events
                .extend(self.host.complete_tasks(task_requests));
        }

        self.steps.push(summary.clone());
        self.finished = runtime_finished || self.steps.len() >= self.max_steps;
        Ok(Some(BundleRunnerSessionStep {
            summary,
            audio_commands,
            finished: self.finished,
        }))
    }

    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn push_audio_events(&mut self, events: impl IntoIterator<Item = AudioEvent>) {
        self.audio_events.extend(events);
    }

    pub fn steps(&self) -> &[BundleRunnerStepSummary] {
        &self.steps
    }

    pub fn final_status(&self) -> String {
        self.executor
            .fiber()
            .status
            .status_label(FlowStatusLabelStyle::Runtime)
    }

    pub fn into_report(mut self) -> BundleRunnerReport {
        self.phases.push(BundleRunnerPhase {
            name: "run",
            elapsed_ns: self.run_started.elapsed().as_nanos(),
        });
        BundleRunnerReport {
            source: self.source,
            bytecode_instructions: self.bytecode_instructions,
            adapter_manifests: self.adapter_manifests,
            phases: self.phases,
            executor: self.executor_kind,
            executor_stats: self.executor.executor_stats(),
            native_io: self.host.stats(),
            steps: self.steps,
            final_status: self
                .executor
                .fiber()
                .status
                .status_label(FlowStatusLabelStyle::Runtime),
        }
    }
}
