use crate::native_task::{NativeAdapterRegistrar, NativeTaskBridge, NativeTaskStats};
use crate::stats::{RuntimeExecutorStats, runtime_executor_stats};
use arcweft_bundle::{ArcweftBundle, BundleAdapterManifest, BundleVirtualFile};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::engine::{FlowExit, FlowFiber, FlowFiberStatus};
use arcweft_core::executor::{AotExecutor, BytecodeVmExecutor, RuntimeExecutor};
use arcweft_core::plan::{FlowRuntimeId, RuntimeEntryTarget, RuntimePlan};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepResult,
};
use arcweft_core::value::RuntimeBinding;
use arcweft_host_adapter::HostCallPolicy;
use arcweft_runtime_accelerator::{RuntimePureAccelerator, RuntimePureAcceleratorConfig};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Executes a decoded Arcweft bundle with native adapters supplied by the host.
pub fn run_bundle_with_native_adapters(
    bundle: &ArcweftBundle,
    options: &BundleRunnerOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<BundleRunnerReport, BundleRunnerError> {
    let mut phases = Vec::new();
    execute_bundle_with_native_adapters(bundle, options, adapter_registrars, &mut phases)
}

/// Reads, decodes, and executes an `.awfb` bundle with native adapters supplied by the host.
pub fn run_bundle_file_with_native_adapters(
    path: impl AsRef<Path>,
    options: &BundleRunnerOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<BundleRunnerReport, BundleRunnerError> {
    let path = path.as_ref();
    let mut phases = Vec::new();
    let bytes = run_bundle_runner_phase(&mut phases, "read_bundle", || {
        fs::read(path).map_err(|source| BundleRunnerError::ReadBundle {
            path: path.to_path_buf(),
            source,
        })
    })?;
    let bundle = run_bundle_runner_phase(&mut phases, "decode_bundle", || {
        ArcweftBundle::from_json_slice(&bytes).map_err(BundleRunnerError::DecodeBundle)
    })?;
    execute_bundle_with_native_adapters(&bundle, options, adapter_registrars, &mut phases)
}

/// Bundle execution options for embedding hosts.
#[derive(Clone, Debug)]
pub struct BundleRunnerOptions {
    pub entry: Option<String>,
    pub flow: Option<String>,
    pub executor: BundleRunnerExecutor,
    pub steps: usize,
    pub mode: BundleRunnerStepMode,
    pub max_ops: usize,
    pub values: Vec<RuntimeBinding>,
    pub pure_config: RuntimePureAcceleratorConfig,
}

impl Default for BundleRunnerOptions {
    fn default() -> Self {
        Self {
            entry: None,
            flow: None,
            executor: BundleRunnerExecutor::BytecodeVm,
            steps: 8,
            mode: BundleRunnerStepMode::Drain,
            max_ops: 32,
            values: Vec::new(),
            pure_config: RuntimePureAcceleratorConfig::default(),
        }
    }
}

/// Runtime execution tier selected by an embedding bundle runner.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleRunnerExecutor {
    BytecodeVm,
    Aot,
}

/// Step scheduling mode selected by an embedding bundle runner.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleRunnerStepMode {
    OneOp,
    Drain,
    Game,
    Server,
}

/// Result returned to embedding hosts after executing a bundle.
#[derive(Clone, Debug, serde::Serialize)]
pub struct BundleRunnerReport {
    pub source: String,
    pub bytecode_instructions: usize,
    pub adapter_manifests: usize,
    pub phases: Vec<BundleRunnerPhase>,
    pub executor: BundleRunnerExecutor,
    pub executor_stats: RuntimeExecutorStats,
    pub native_io: NativeTaskStats,
    pub steps: Vec<BundleRunnerStepSummary>,
    pub final_status: String,
}

/// One measured phase in bundle loading and execution.
#[derive(Clone, Debug, serde::Serialize)]
pub struct BundleRunnerPhase {
    pub name: &'static str,
    pub elapsed_ns: u128,
}

/// Public step summary for embedding bundle runners.
#[derive(Clone, Debug, serde::Serialize)]
pub struct BundleRunnerStepSummary {
    pub index: usize,
    pub stop_reason: String,
    pub fiber_status: String,
    pub executed_ops: usize,
    pub task_requests: usize,
    pub diagnostics: Vec<String>,
    pub line_effects: Vec<String>,
}

#[derive(Debug, Error)]
pub enum BundleRunnerError {
    #[error("failed to read bundle `{}`: {source}", path.display())]
    ReadBundle {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to decode bundle: {0}")]
    DecodeBundle(arcweft_bundle::BundleCodecError),
    #[error("failed to decode bundle bytecode: {0}")]
    DecodeBytecode(arcweft_core::plan::RuntimePlanError),
    #[error("failed to create bundle workspace: {0}")]
    CreateWorkspace(std::io::Error),
    #[error("failed to create bundle source directory: {0}")]
    CreateSourceDirectory(std::io::Error),
    #[error("failed to materialize bundle source: {0}")]
    MaterializeSource(std::io::Error),
    #[error("failed to create bundle virtual file directory: {0}")]
    CreateVirtualFileDirectory(std::io::Error),
    #[error("failed to materialize bundle virtual file: {0}")]
    MaterializeVirtualFile(std::io::Error),
    #[error("bundle virtual file path must be relative and normalized")]
    InvalidVirtualFilePath,
    #[error("entry and flow are mutually exclusive")]
    ConflictingEntrySelection,
    #[error("unknown flow `{flow}`")]
    UnknownFlow { flow: String },
    #[error("unknown entry `{entry}`")]
    UnknownEntry { entry: String },
    #[error("entry `{entry}` does not select a single runnable flow")]
    NonFlowEntry { entry: String },
    #[error("native adapter registration failed: {0}")]
    NativeAdapter(arcweft_host_adapter::HostAdapterError),
}

fn execute_bundle_with_native_adapters(
    bundle: &ArcweftBundle,
    options: &BundleRunnerOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
    phases: &mut Vec<BundleRunnerPhase>,
) -> Result<BundleRunnerReport, BundleRunnerError> {
    let workspace = run_bundle_runner_phase(phases, "materialize_bundle", || {
        MaterializedBundleWorkspace::create(bundle)
    })?;
    let bytecode = run_bundle_runner_phase(phases, "bytecode_decode", || {
        bundle_runner_bytecode(bundle, options)
    })?;
    let entry = bundle_runner_entry(bundle, options);
    let direct_bytecode = entry.is_none() && options.flow.is_none();
    let host_policy = bundle_host_policy(bundle);
    let trace = run_bundle_runner_phase(phases, "run", || {
        run_bytecode_runtime_steps(
            if direct_bytecode {
                bundle.bytecode.program.clone()
            } else {
                bytecode
            },
            Some(workspace.source_path()),
            RuntimeStepRunConfig {
                steps: options.steps,
                mode: options.mode,
                max_ops: options.max_ops,
                executor: options.executor,
                pure_config: options.pure_config,
            },
            &host_policy,
            adapter_registrars,
            &options.values,
        )
    })?;
    Ok(BundleRunnerReport {
        source: bundle.manifest.source_label.clone(),
        bytecode_instructions: bundle.manifest.runtime.bytecode_instructions,
        adapter_manifests: bundle.adapter_manifests.len(),
        phases: std::mem::take(phases),
        executor: options.executor,
        executor_stats: trace.executor_stats,
        native_io: trace.native_io,
        steps: trace.steps,
        final_status: flow_status_label(&trace.final_status),
    })
}

fn run_bundle_runner_phase<T>(
    phases: &mut Vec<BundleRunnerPhase>,
    name: &'static str,
    run: impl FnOnce() -> Result<T, BundleRunnerError>,
) -> Result<T, BundleRunnerError> {
    let started = Instant::now();
    let result = run();
    phases.push(BundleRunnerPhase {
        name,
        elapsed_ns: started.elapsed().as_nanos(),
    });
    result
}

fn bundle_runner_bytecode(
    bundle: &ArcweftBundle,
    options: &BundleRunnerOptions,
) -> Result<BytecodeProgram, BundleRunnerError> {
    let mut plan = bundle
        .bytecode
        .program
        .clone()
        .into_runtime_plan()
        .map_err(BundleRunnerError::DecodeBytecode)?;
    apply_bundle_runner_entry_selection(
        &mut plan,
        bundle_runner_entry(bundle, options),
        options.flow.as_deref(),
    )?;
    Ok(BytecodeProgram::from_runtime_plan(plan))
}

fn bundle_runner_entry<'a>(
    bundle: &'a ArcweftBundle,
    options: &'a BundleRunnerOptions,
) -> Option<&'a str> {
    options.entry.as_deref().or_else(|| {
        options
            .flow
            .is_none()
            .then_some(bundle.manifest.entry.as_deref())
            .flatten()
    })
}

fn apply_bundle_runner_entry_selection(
    plan: &mut RuntimePlan,
    entry: Option<&str>,
    flow: Option<&str>,
) -> Result<(), BundleRunnerError> {
    if entry.is_some() && flow.is_some() {
        return Err(BundleRunnerError::ConflictingEntrySelection);
    }
    if let Some(flow) = flow {
        let flow = FlowRuntimeId(normalize_flow_id(flow));
        if !plan.flows.iter().any(|candidate| candidate.id == flow) {
            return Err(BundleRunnerError::UnknownFlow { flow: flow.0 });
        }
        plan.entry_flow = Some(flow);
        return Ok(());
    }
    if let Some(entry) = entry {
        let entry = normalize_entry_id(entry);
        let Some(spec) = plan
            .entries
            .iter()
            .find(|candidate| candidate.id.0 == entry)
        else {
            return Err(BundleRunnerError::UnknownEntry { entry });
        };
        let RuntimeEntryTarget::Flow(flow) = &spec.target else {
            return Err(BundleRunnerError::NonFlowEntry { entry });
        };
        plan.entry_flow = Some(flow.clone());
    }
    Ok(())
}

fn run_bytecode_runtime_steps(
    bytecode: BytecodeProgram,
    source_path: Option<&Path>,
    config: RuntimeStepRunConfig,
    host_policy: &HostCallPolicy,
    adapter_registrars: &[NativeAdapterRegistrar],
    values: &[RuntimeBinding],
) -> Result<RuntimeRunTrace, BundleRunnerError> {
    let mut executor =
        RuntimeExecutorInstance::from_bytecode(bytecode, config.executor, config.pure_config)
            .map_err(BundleRunnerError::DecodeBytecode)?;
    run_runtime_steps_with_executor(
        &mut executor,
        NativeRunHost {
            source_path,
            policy: host_policy,
            adapter_registrars,
        },
        config.steps,
        config.mode,
        config.max_ops,
        values,
    )
    .map_err(BundleRunnerError::NativeAdapter)
}

fn run_runtime_steps_with_executor(
    executor: &mut RuntimeExecutorInstance,
    host_config: NativeRunHost<'_>,
    steps: usize,
    mode: BundleRunnerStepMode,
    max_ops: usize,
    values: &[RuntimeBinding],
) -> Result<RuntimeRunTrace, arcweft_host_adapter::HostAdapterError> {
    let mut host = host_config
        .source_path
        .map(|path| {
            NativeTaskBridge::try_new(
                path,
                host_config.policy.clone(),
                host_config.adapter_registrars,
            )
        })
        .transpose()?;
    let mut task_events = Vec::new();
    let mut summaries = Vec::new();
    for step_index in 0..steps {
        let result = executor.step_with_root_bindings(
            RuntimeStepInput {
                task_events: std::mem::take(&mut task_events),
                ..RuntimeStepInput::default()
            },
            values,
            step_options(mode, max_ops),
        );
        let (summary, task_requests) = BundleRunnerStepSummary::from_result(step_index, result);
        let done = matches!(
            executor.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        summaries.push(summary);
        if done {
            break;
        }
        if let Some(host) = host.as_mut() {
            task_events = host.complete_tasks(task_requests);
        }
    }
    Ok(RuntimeRunTrace {
        steps: summaries,
        final_status: executor.fiber().status.clone(),
        executor_stats: executor.executor_stats(),
        native_io: host
            .as_ref()
            .map_or_else(NativeTaskStats::default, NativeTaskBridge::stats),
    })
}

#[derive(Clone, Copy, Debug)]
struct RuntimeStepRunConfig {
    steps: usize,
    mode: BundleRunnerStepMode,
    max_ops: usize,
    executor: BundleRunnerExecutor,
    pure_config: RuntimePureAcceleratorConfig,
}

struct RuntimeRunTrace {
    steps: Vec<BundleRunnerStepSummary>,
    final_status: FlowFiberStatus,
    executor_stats: RuntimeExecutorStats,
    native_io: NativeTaskStats,
}

#[derive(Clone, Copy)]
struct NativeRunHost<'a> {
    source_path: Option<&'a Path>,
    policy: &'a HostCallPolicy,
    adapter_registrars: &'a [NativeAdapterRegistrar],
}

enum RuntimeExecutorInstance {
    BytecodeVm {
        executor: BytecodeVmExecutor,
        pure: RuntimePureAccelerator,
    },
    Aot {
        executor: AotExecutor,
        pure: RuntimePureAccelerator,
    },
}

impl RuntimeExecutorInstance {
    fn from_bytecode(
        bytecode: BytecodeProgram,
        tier: BundleRunnerExecutor,
        pure_config: RuntimePureAcceleratorConfig,
    ) -> Result<Self, arcweft_core::plan::RuntimePlanError> {
        let plan = bytecode.clone().into_runtime_plan()?;
        let pure = RuntimePureAccelerator::with_config(pure_config, &plan.pure_helpers);
        Ok(match tier {
            BundleRunnerExecutor::BytecodeVm => Self::BytecodeVm {
                executor: BytecodeVmExecutor::from_parts(bytecode, plan),
                pure,
            },
            BundleRunnerExecutor::Aot => Self::Aot {
                executor: AotExecutor::new(plan),
                pure,
            },
        })
    }

    fn step_with_root_bindings(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult {
        match self {
            Self::BytecodeVm { executor, pure } => executor
                .step_with_root_bindings_and_pure_backend(input, root_bindings, options, pure),
            Self::Aot { executor, pure } => executor.step_with_root_bindings_and_pure_backend(
                input,
                root_bindings,
                options,
                pure,
            ),
        }
    }

    fn fiber(&self) -> &FlowFiber {
        match self {
            Self::BytecodeVm { executor, .. } => executor.fiber(),
            Self::Aot { executor, .. } => executor.fiber(),
        }
    }

    fn executor_stats(&self) -> RuntimeExecutorStats {
        match self {
            Self::BytecodeVm { pure, .. } => runtime_executor_stats(0, pure),
            Self::Aot { executor, pure } => runtime_executor_stats(executor.fast_path_ops(), pure),
        }
    }
}

impl BundleRunnerStepSummary {
    fn from_result(
        index: usize,
        result: RuntimeStepResult,
    ) -> (Self, Vec<arcweft_core::task::TaskSpec>) {
        let RuntimeStepResult {
            mut output,
            fiber_status,
            stop_reason,
            stats,
        } = result;
        let task_requests = std::mem::take(&mut output.requests.tasks);
        (
            Self {
                index,
                stop_reason: format!("{stop_reason:?}"),
                fiber_status: flow_status_label(&fiber_status),
                executed_ops: stats.executed_ops,
                task_requests: task_requests.len(),
                diagnostics: output
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.message)
                    .collect(),
                line_effects: output.effects.line.iter().map(effect_label).collect(),
            },
            task_requests,
        )
    }
}

fn bundle_host_policy(bundle: &ArcweftBundle) -> HostCallPolicy {
    HostCallPolicy::from_host_call_ids(
        bundle
            .adapter_manifests
            .iter()
            .flat_map(BundleAdapterManifest::host_call_ids),
    )
}

fn step_options(mode: BundleRunnerStepMode, max_ops: usize) -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: match mode {
            BundleRunnerStepMode::OneOp => RuntimeStepMode::OneOp,
            BundleRunnerStepMode::Drain => RuntimeStepMode::Drain,
            BundleRunnerStepMode::Game => RuntimeStepMode::Game,
            BundleRunnerStepMode::Server => RuntimeStepMode::Server,
        },
        budget: RuntimeStepBudget { max_ops },
    }
}

struct MaterializedBundleWorkspace {
    root: PathBuf,
    source_path: PathBuf,
}

impl MaterializedBundleWorkspace {
    fn create(bundle: &ArcweftBundle) -> Result<Self, BundleRunnerError> {
        let root = std::env::temp_dir().join(format!(
            "arcweft-bundle-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        ));
        fs::create_dir_all(&root).map_err(BundleRunnerError::CreateWorkspace)?;
        let source_name = bundle_source_file_name(&bundle.source.label);
        let source_path = root.join(source_name);
        if let Some(parent) = source_path.parent() {
            fs::create_dir_all(parent).map_err(BundleRunnerError::CreateSourceDirectory)?;
        }
        fs::write(&source_path, &bundle.source.text)
            .map_err(BundleRunnerError::MaterializeSource)?;
        materialize_bundle_virtual_files(&root, &bundle.virtual_files)?;
        Ok(Self { root, source_path })
    }

    fn source_path(&self) -> &Path {
        &self.source_path
    }
}

impl Drop for MaterializedBundleWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn bundle_source_file_name(label: &str) -> String {
    let path = Path::new(label);
    path.file_name()
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("arcw"))
        })
        .map_or_else(
            || "bundle.arcw".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
}

fn materialize_bundle_virtual_files(
    root: &Path,
    files: &[BundleVirtualFile],
) -> Result<(), BundleRunnerError> {
    for file in files {
        let relative = Path::new(&file.path);
        validate_relative_virtual_path(relative)?;
        let path = root
            .join(".arcweft")
            .join(file.space.as_str())
            .join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(BundleRunnerError::CreateVirtualFileDirectory)?;
        }
        fs::write(&path, &file.bytes).map_err(BundleRunnerError::MaterializeVirtualFile)?;
    }
    Ok(())
}

fn validate_relative_virtual_path(path: &Path) -> Result<(), BundleRunnerError> {
    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
        .then_some(())
        .ok_or(BundleRunnerError::InvalidVirtualFilePath)
}

fn normalize_flow_id(value: &str) -> String {
    normalize_entity_selector(value, "flow")
}

fn normalize_entry_id(value: &str) -> String {
    normalize_entity_selector(value, "entry")
}

fn normalize_entity_selector(value: &str, family: &str) -> String {
    let value = value.trim().trim_start_matches('@');
    if value.contains('.') {
        value.to_owned()
    } else {
        format!("{family}.{value}")
    }
}

fn flow_status_label(status: &FlowFiberStatus) -> String {
    match status {
        FlowFiberStatus::Running => "running".to_owned(),
        FlowFiberStatus::Waiting(state) => format!("waiting {}", state.target.task.0),
        FlowFiberStatus::WaitingMany(state) => format!(
            "waiting_many {} {}/{}",
            state.target.task.0,
            state.results.iter().filter(|value| value.is_some()).count(),
            state.results.len()
        ),
        FlowFiberStatus::Choice(state) => {
            format!("choice {}", state.id.as_deref().unwrap_or("-"))
        }
        FlowFiberStatus::Done(FlowExit::Done) => "done".to_owned(),
        FlowFiberStatus::Done(FlowExit::Return(value)) => format!("done return {value}"),
        FlowFiberStatus::Failed(message) => format!("failed {message}"),
    }
}

fn effect_label(effect: &LineEffectRequest) -> String {
    match effect {
        LineEffectRequest::RegisterHandle { key, .. } => format!("register {key}"),
        LineEffectRequest::DropHandle { key } => format!("drop {key}"),
        LineEffectRequest::Wait(target) => format!("wait {target:?}"),
        LineEffectRequest::Call(call) => format!("call {}", call.callee),
        LineEffectRequest::Log(log) => format!("log.{}", log.level),
        LineEffectRequest::SignalWrite(write) => format!("signal.set {}", write.target),
        LineEffectRequest::MetricWrite(write) => format!("metric.set {}", write.target),
        LineEffectRequest::EmitEvent(event) => format!("event.emit {}", event.event),
        LineEffectRequest::Out(_) => "out".to_owned(),
        LineEffectRequest::Return(value) => format!("return {value}"),
        LineEffectRequest::Goto(_) => "goto".to_owned(),
        LineEffectRequest::Panic(_) => "panic".to_owned(),
        LineEffectRequest::Fail(_) => "fail".to_owned(),
        LineEffectRequest::Bail(_) => "bail".to_owned(),
        LineEffectRequest::Ensure { .. } => "ensure".to_owned(),
        LineEffectRequest::Assert(assertion) => match assertion.profile {
            arcweft_core::effect::RuntimeAssertionProfile::Always => "assert".to_owned(),
            arcweft_core::effect::RuntimeAssertionProfile::DebugOnly => "debug_assert".to_owned(),
        },
        LineEffectRequest::Close(_) => "close".to_owned(),
        LineEffectRequest::Select(_) => "select".to_owned(),
        LineEffectRequest::Break { .. } => "break".to_owned(),
        LineEffectRequest::Continue { .. } => "continue".to_owned(),
    }
}
