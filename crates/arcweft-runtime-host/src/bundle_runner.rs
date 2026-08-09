use crate::native_task::{
    NativeAdapterRegistrar, NativeFileRoots, NativeTaskBridge, NativeTaskStats,
};
use crate::stats::{RuntimeExecutorStats, runtime_executor_stats};
use arcweft_bundle::{
    ArcweftBundle, BundleAdapterManifest, BundleFormat, BundleImageAnimation, BundleImageAsset,
    BundleImageDimensions, BundleImageFormat, BundleKind, BundleVirtualFile,
};
use arcweft_core::awbc::{
    product_step::AwbcProductStepBuildError,
    schema::{AwbcEntryId, AwbcProgram},
};
use arcweft_core::bytecode::{
    BytecodeProgram, BytecodeVerificationBudget, BytecodeVerificationError,
};
use arcweft_core::effect::{LineEffectRequest, RuntimeAssertionFailure};
use arcweft_core::engine::{EngineStartError, FlowFiber, FlowFiberStatus, FlowStatusLabelStyle};
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::plan::{EntryRuntimeId, FlowEvent, RuntimeEntryTarget, RuntimePlan};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepResult,
};
use arcweft_core::value::RuntimeBinding;
use arcweft_host_adapter::HostCallPolicy;
use arcweft_interaction_model::audio::AudioCommandEnvelope;
use arcweft_runtime_accelerator::{RuntimePureAccelerator, RuntimePureAcceleratorConfig};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

mod session;
pub use session::{BundleRunnerSession, BundleRunnerSessionStep};

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
    if path.extension().and_then(std::ffi::OsStr::to_str) != Some("awfb") {
        return Err(BundleRunnerError::ExpectedAwfbProduct {
            path: path.to_path_buf(),
        });
    }
    let mut phases = Vec::new();
    let bytes = run_bundle_runner_phase(&mut phases, "read_bundle", || {
        fs::read(path).map_err(|source| BundleRunnerError::ReadBundle {
            path: path.to_path_buf(),
            source,
        })
    })?;
    let bundle = run_bundle_runner_phase(&mut phases, "decode_bundle", || {
        ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
            .map_err(BundleRunnerError::DecodeBundle)
    })?;
    execute_bundle_with_native_adapters(&bundle, options, adapter_registrars, &mut phases)
}

/// Bundle execution options for embedding hosts.
#[derive(Clone, Debug)]
pub struct BundleRunnerOptions {
    pub entry: Option<EntryRuntimeId>,
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
            executor: BundleRunnerExecutor::AwbcProduct,
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
    AwbcProduct,
    BytecodeVm,
    Aot,
}

impl From<BundleRunnerExecutor> for ArcweftExecutionTier {
    fn from(value: BundleRunnerExecutor) -> Self {
        match value {
            BundleRunnerExecutor::AwbcProduct => Self::AwbcProduct,
            BundleRunnerExecutor::BytecodeVm => Self::StructuredVm,
            BundleRunnerExecutor::Aot => Self::StructuredAot,
        }
    }
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
    pub audio_commands: usize,
    pub diagnostics: Vec<String>,
    /// Typed failures produced from emitted assertion requests. The host does
    /// not parse materialized condition or message strings to create these.
    pub assertion_failures: Vec<RuntimeAssertionFailure>,
    pub line_effects: Vec<String>,
    #[serde(skip)]
    pub flow_events: Vec<FlowEvent>,
}

#[derive(Debug, Error)]
pub enum BundleRunnerError {
    #[error("failed to read bundle `{}`: {source}", path.display())]
    ReadBundle {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("bundle runner expects an .awfb product bundle: {}", path.display())]
    ExpectedAwfbProduct { path: PathBuf },
    #[error("failed to decode bundle: {0}")]
    DecodeBundle(arcweft_bundle::BundleCodecError),
    #[error("invalid bundle image asset: {0}")]
    InvalidImageAsset(#[source] arcweft_bundle::BundleCodecError),
    #[error("unsupported bundle kind `{kind}` for the game bundle runner")]
    UnsupportedBundleKind { kind: BundleKind },
    #[error("failed to decode bundle image asset `{asset_id}` ({path}): {source}")]
    DecodeImageAsset {
        asset_id: String,
        path: String,
        #[source]
        source: arcweft_image::ImageError,
    },
    #[error(
        "bundle image asset `{asset_id}` metadata mismatch for {field}: expected {expected}, actual {actual}"
    )]
    ImageAssetMetadataMismatch {
        asset_id: String,
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error("failed to decode bundle bytecode: {0}")]
    DecodeBytecode(arcweft_core::plan::RuntimePlanError),
    #[error(transparent)]
    ProductAwbcRuntime(#[from] AwbcProductStepBuildError),
    #[error("failed to verify bundle bytecode: {0}")]
    VerifyBytecode(BytecodeVerificationError),
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
    #[error("an exact entry selection is required to run a bundle")]
    MissingEntrySelection,
    #[error("invalid canonical entry selection `{entry}`: {message}")]
    InvalidEntrySelection { entry: String, message: String },
    #[error("unknown entry `{entry}`")]
    UnknownEntry { entry: String },
    #[error("entry `{entry}` does not select a single runnable flow")]
    NonFlowEntry { entry: String },
    #[error("failed to start exact entry: {0}")]
    StartEntry(EngineStartError),
    #[error("native adapter registration failed: {0}")]
    NativeAdapter(arcweft_host_adapter::HostAdapterError),
}

fn execute_bundle_with_native_adapters(
    bundle: &ArcweftBundle,
    options: &BundleRunnerOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
    phases: &mut Vec<BundleRunnerPhase>,
) -> Result<BundleRunnerReport, BundleRunnerError> {
    run_bundle_runner_phase(phases, "validate_bundle_kind", || {
        validate_bundle_kind(bundle)
    })?;
    run_bundle_runner_phase(phases, "validate_image_assets", || {
        validate_bundle_image_assets(bundle)
    })?;
    let workspace = run_bundle_runner_phase(phases, "materialize_bundle", || {
        MaterializedBundleWorkspace::create(bundle)
    })?;
    let runtime_program = run_bundle_runner_phase(phases, "runtime_decode", || {
        bundle_runner_runtime_program(bundle, options)
    })?;
    let actual_executor = runtime_program.executor_kind();
    let host_policy = bundle_host_policy(bundle);
    let trace = run_bundle_runner_phase(phases, "run", || {
        run_product_runtime_steps(
            runtime_program,
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
        source: bundle.source_display_name().to_owned(),
        bytecode_instructions: bundle.manifest.runtime.bytecode_instructions,
        adapter_manifests: bundle.adapter_manifests.len(),
        phases: std::mem::take(phases),
        executor: actual_executor,
        executor_stats: trace.executor_stats,
        native_io: trace.native_io,
        steps: trace.steps,
        final_status: trace
            .final_status
            .status_label(FlowStatusLabelStyle::Runtime),
    })
}

fn validate_bundle_kind(bundle: &ArcweftBundle) -> Result<(), BundleRunnerError> {
    match bundle.bundle_kind {
        BundleKind::Game => Ok(()),
        BundleKind::AgentController => Err(BundleRunnerError::UnsupportedBundleKind {
            kind: bundle.bundle_kind,
        }),
    }
}

fn validate_bundle_image_assets(bundle: &ArcweftBundle) -> Result<(), BundleRunnerError> {
    for asset in &bundle.image_assets {
        let Some(bytes) = bundle
            .image_asset_bytes(&asset.id)
            .map_err(BundleRunnerError::InvalidImageAsset)?
        else {
            continue;
        };
        validate_bundle_image_asset_metadata(asset, bytes)?;
    }
    Ok(())
}

fn validate_bundle_image_asset_metadata(
    asset: &BundleImageAsset,
    bytes: &[u8],
) -> Result<(), BundleRunnerError> {
    let decoded = arcweft_image::decode_image_bytes(
        bundle_image_decode_format(asset.format),
        bytes,
        arcweft_image::ImageDecodeOptions::default(),
    )
    .map_err(|source| BundleRunnerError::DecodeImageAsset {
        asset_id: asset.id.clone(),
        path: asset.file.path.clone(),
        source,
    })?;
    let actual_animation = bundle_image_animation_from_decoded(&decoded);
    if asset.animation != actual_animation {
        return Err(BundleRunnerError::ImageAssetMetadataMismatch {
            asset_id: asset.id.clone(),
            field: "animation",
            expected: format!("{:?}", asset.animation),
            actual: format!("{actual_animation:?}"),
        });
    }
    if let Some(expected_dimensions) = asset.dimensions {
        let actual_dimensions = bundle_image_dimensions_from_decoded(&decoded);
        if expected_dimensions != actual_dimensions {
            return Err(BundleRunnerError::ImageAssetMetadataMismatch {
                asset_id: asset.id.clone(),
                field: "dimensions",
                expected: format!(
                    "{}x{}",
                    expected_dimensions.width, expected_dimensions.height
                ),
                actual: format!("{}x{}", actual_dimensions.width, actual_dimensions.height),
            });
        }
    }
    Ok(())
}

const fn bundle_image_decode_format(format: BundleImageFormat) -> arcweft_image::ImageFormat {
    match format {
        BundleImageFormat::Png => arcweft_image::ImageFormat::Png,
        BundleImageFormat::Jpeg => arcweft_image::ImageFormat::Jpeg,
        BundleImageFormat::Gif => arcweft_image::ImageFormat::Gif,
        BundleImageFormat::WebP => arcweft_image::ImageFormat::WebP,
    }
}

fn bundle_image_animation_from_decoded(
    image: &arcweft_image::DecodedImage,
) -> BundleImageAnimation {
    if image.is_animated() {
        BundleImageAnimation::Animated
    } else {
        BundleImageAnimation::Static
    }
}

fn bundle_image_dimensions_from_decoded(
    image: &arcweft_image::DecodedImage,
) -> BundleImageDimensions {
    let dimensions = image.dimensions();
    BundleImageDimensions::new(dimensions.width(), dimensions.height())
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
) -> Result<(BytecodeProgram, EntryRuntimeId), BundleRunnerError> {
    let structured_program = &bundle.bytecode.program;
    let program = structured_program.clone();
    program
        .verify(BytecodeVerificationBudget::default())
        .map_err(BundleRunnerError::VerifyBytecode)?;
    let plan = program
        .into_runtime_plan()
        .map_err(BundleRunnerError::DecodeBytecode)?;
    let entry = selected_structured_entry(&plan, bundle, options)?;
    Ok((BytecodeProgram::from_runtime_plan(plan), entry))
}

enum BundleRunnerRuntimeProgram {
    Awbc {
        program: Box<AwbcProgram>,
        entry: AwbcEntryId,
    },
    Structured {
        program: Box<BytecodeProgram>,
        entry: EntryRuntimeId,
        executor: BundleRunnerExecutor,
    },
}

impl BundleRunnerRuntimeProgram {
    const fn executor_kind(&self) -> BundleRunnerExecutor {
        match self {
            Self::Awbc { .. } => BundleRunnerExecutor::AwbcProduct,
            Self::Structured { executor, .. } => *executor,
        }
    }
}

fn bundle_runner_runtime_program(
    bundle: &ArcweftBundle,
    options: &BundleRunnerOptions,
) -> Result<BundleRunnerRuntimeProgram, BundleRunnerError> {
    if let Some(product_awbc) = bundle.product_awbc() {
        let program = product_awbc.program().clone();
        let entry = selected_awbc_entry(&program, bundle, options)?;
        return Ok(BundleRunnerRuntimeProgram::Awbc {
            program: Box::new(program),
            entry,
        });
    }
    match options.executor {
        BundleRunnerExecutor::AwbcProduct => {
            let program = bundle
                .product_awbc_program()
                .map_err(BundleRunnerError::DecodeBundle)?
                .clone();
            let entry = selected_awbc_entry(&program, bundle, options)?;
            Ok(BundleRunnerRuntimeProgram::Awbc {
                program: Box::new(program),
                entry,
            })
        }
        BundleRunnerExecutor::BytecodeVm | BundleRunnerExecutor::Aot => {
            bundle_runner_bytecode(bundle, options).map(|(program, entry)| {
                BundleRunnerRuntimeProgram::Structured {
                    program: Box::new(program),
                    entry,
                    executor: options.executor,
                }
            })
        }
    }
}

fn selected_awbc_entry(
    program: &AwbcProgram,
    bundle: &ArcweftBundle,
    options: &BundleRunnerOptions,
) -> Result<AwbcEntryId, BundleRunnerError> {
    let Some(entry) = bundle_runner_entry(bundle, options)? else {
        return Err(BundleRunnerError::MissingEntrySelection);
    };
    program
        .entries
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| {
            (candidate.runtime_id == entry).then(|| {
                AwbcEntryId(
                    u32::try_from(index)
                        .expect("verified AWBC entry table indices fit the u32 wire contract"),
                )
            })
        })
        .ok_or(BundleRunnerError::UnknownEntry {
            entry: entry.public_label().into_string(),
        })
}

fn bundle_runner_entry(
    bundle: &ArcweftBundle,
    options: &BundleRunnerOptions,
) -> Result<Option<EntryRuntimeId>, BundleRunnerError> {
    if let Some(entry) = &options.entry {
        return Ok(Some(entry.clone()));
    }
    bundle
        .manifest
        .entry
        .as_deref()
        .map(|entry| {
            EntryRuntimeId::from_source_entity_body(entry).map_err(|error| {
                BundleRunnerError::InvalidEntrySelection {
                    entry: entry.to_owned(),
                    message: error.to_string(),
                }
            })
        })
        .transpose()
}

fn selected_structured_entry(
    plan: &RuntimePlan,
    bundle: &ArcweftBundle,
    options: &BundleRunnerOptions,
) -> Result<EntryRuntimeId, BundleRunnerError> {
    let Some(entry) = bundle_runner_entry(bundle, options)? else {
        return Err(BundleRunnerError::MissingEntrySelection);
    };
    let label = entry.public_label().into_string();
    let Some(spec) = plan.entries.iter().find(|candidate| candidate.id == entry) else {
        return Err(BundleRunnerError::UnknownEntry { entry: label });
    };
    if !matches!(spec.target, RuntimeEntryTarget::Flow(_)) {
        return Err(BundleRunnerError::NonFlowEntry { entry: label });
    }
    Ok(entry)
}

fn run_product_runtime_steps(
    program: BundleRunnerRuntimeProgram,
    source_path: Option<&Path>,
    config: RuntimeStepRunConfig,
    host_policy: &HostCallPolicy,
    adapter_registrars: &[NativeAdapterRegistrar],
    values: &[RuntimeBinding],
) -> Result<RuntimeRunTrace, BundleRunnerError> {
    let mut executor = RuntimeExecutorInstance::from_product_program(
        program,
        config.executor,
        config.pure_config,
    )?;
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
                NativeFileRoots::for_bundle_workspace(path),
                host_config.policy.clone(),
                host_config.adapter_registrars,
            )
        })
        .transpose()?;
    let mut task_events = Vec::new();
    let mut summaries = Vec::new();
    for step_index in 0..steps {
        if let Some(host) = host.as_mut() {
            host.pump_main_thread()?;
            task_events.extend(host.poll_completions());
        }
        let result = executor.step_with_root_bindings(
            RuntimeStepInput {
                task_events: std::mem::take(&mut task_events),
                ..RuntimeStepInput::default()
            },
            values,
            step_options(mode, max_ops),
        );
        let (summary, task_requests, _audio_commands) =
            BundleRunnerStepSummary::from_result(step_index, result);
        let done = matches!(
            executor.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        summaries.push(summary);
        if done {
            break;
        }
        if let Some(host) = host.as_mut() {
            task_events.extend(host.complete_tasks(task_requests));
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

struct RuntimeExecutorInstance {
    executor: ArcweftRuntimeExecutor,
    pure: RuntimePureAccelerator,
}

impl RuntimeExecutorInstance {
    fn from_product_program(
        program: BundleRunnerRuntimeProgram,
        tier: BundleRunnerExecutor,
        pure_config: RuntimePureAcceleratorConfig,
    ) -> Result<Self, BundleRunnerError> {
        match program {
            BundleRunnerRuntimeProgram::Awbc { program, entry } => Ok(Self {
                executor: ArcweftRuntimeExecutor::from_awbc_product(*program, entry)?,
                pure: RuntimePureAccelerator::with_config(
                    RuntimePureAcceleratorConfig::default(),
                    &[],
                ),
            }),
            BundleRunnerRuntimeProgram::Structured { program, entry, .. } => {
                Self::from_bytecode(*program, &entry, tier, pure_config)
            }
        }
    }

    fn from_bytecode(
        bytecode: BytecodeProgram,
        entry: &EntryRuntimeId,
        tier: BundleRunnerExecutor,
        pure_config: RuntimePureAcceleratorConfig,
    ) -> Result<Self, BundleRunnerError> {
        bytecode
            .verify(BytecodeVerificationBudget::default())
            .map_err(BundleRunnerError::VerifyBytecode)?;
        let plan = bytecode
            .clone()
            .into_runtime_plan()
            .map_err(BundleRunnerError::DecodeBytecode)?;
        let pure = RuntimePureAccelerator::with_config(pure_config, &plan.pure_helpers);
        let mut executor = ArcweftRuntimeExecutor::from_bytecode(bytecode, tier.into())
            .map_err(BundleRunnerError::DecodeBytecode)?;
        executor
            .start_structured_entry(entry)
            .map_err(BundleRunnerError::StartEntry)?;
        Ok(Self { executor, pure })
    }

    fn step_with_root_bindings(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult {
        self.executor.step_with_root_bindings_and_pure_backend(
            input,
            root_bindings,
            options,
            &mut self.pure,
        )
    }

    fn fiber(&self) -> &FlowFiber {
        self.executor.fiber()
    }

    fn executor_stats(&self) -> RuntimeExecutorStats {
        runtime_executor_stats(self.executor.fast_path_ops(), &self.pure)
    }
}

impl BundleRunnerStepSummary {
    fn from_result(
        index: usize,
        result: RuntimeStepResult,
    ) -> (
        Self,
        Vec<arcweft_core::task::TaskSpec>,
        Vec<AudioCommandEnvelope>,
    ) {
        let RuntimeStepResult {
            mut output,
            fiber_status,
            stop_reason,
            stats,
        } = result;
        let task_requests = std::mem::take(&mut output.requests.tasks);
        let audio_commands = output.requests.audio;
        let flow_events = std::mem::take(&mut output.flow_events);
        let assertion_failures = output
            .effects
            .line
            .iter()
            .filter_map(|effect| match effect {
                LineEffectRequest::Assert(assertion) => {
                    Some(RuntimeAssertionFailure::new(assertion.clone()))
                }
                _ => None,
            })
            .collect();
        (
            Self {
                index,
                stop_reason: format!("{stop_reason:?}"),
                fiber_status: fiber_status.status_label(FlowStatusLabelStyle::Runtime),
                executed_ops: stats.executed_ops,
                task_requests: task_requests.len(),
                audio_commands: audio_commands.len(),
                diagnostics: output
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.message)
                    .collect(),
                assertion_failures,
                line_effects: output.effects.line.iter().map(effect_label).collect(),
                flow_events,
            },
            task_requests,
            audio_commands,
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
        let source = bundle.primary_source_document();
        let source_name = bundle_source_file_name(
            source.map_or("bundle.arcw", |source| source.display_name().display_name()),
        );
        let source_path = root.join(source_name);
        if let Some(parent) = source_path.parent() {
            fs::create_dir_all(parent).map_err(BundleRunnerError::CreateSourceDirectory)?;
        }
        fs::write(&source_path, source.map_or("", |source| source.text()))
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

fn effect_label(effect: &LineEffectRequest) -> String {
    match effect {
        LineEffectRequest::RegisterHandle { key, .. } => format!("register {key}"),
        LineEffectRequest::DropHandle { key } => format!("drop {key}"),
        LineEffectRequest::Wait(target) => format!("wait {target:?}"),
        LineEffectRequest::Call(call) => format!("call {}", call.callee),
        LineEffectRequest::Audio(command) => format!("audio.{}", command.operation_name()),
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
        LineEffectRequest::Assert(assertion) => match assertion.profile() {
            arcweft_core::effect::RuntimeAssertionProfile::Always => "assert".to_owned(),
            arcweft_core::effect::RuntimeAssertionProfile::DebugOnly => "debug_assert".to_owned(),
        },
        LineEffectRequest::Close(_) => "close".to_owned(),
        LineEffectRequest::Select(_) => "select".to_owned(),
        LineEffectRequest::Break { .. } => "break".to_owned(),
        LineEffectRequest::Continue { .. } => "continue".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::resource_codec::SourceMapSection;
    use arcweft_bundle::{
        ArcweftBundle, BundleImageAnimation, BundleImageAsset, BundleImageDimensions,
        BundleImageFormat, BundleManifest, BundleRuntimeSummary, BundleVirtualFile,
        BundleVirtualFileRef, BundleVirtualFileSpace,
    };
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_core::effect::{
        RuntimeAssertion, RuntimeAssertionGuardId, RuntimeAssertionProfile,
    };
    use arcweft_core::entry::{EntryBindingIdentity, RuntimeEntryRoles};
    use arcweft_core::line_task::LineTaskGroup;
    use arcweft_core::plan::{
        FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeFlow, RuntimeLineId,
    };
    use arcweft_id::TextKey;
    use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use arcweft_text_model::{
        DialogueContentCatalog, DialogueContentSpec, RichTextDocument, RichTextNode,
    };

    fn fixture_runtime_artifact_fingerprint() -> arcweft_core::effect::RuntimeArtifactFingerprint {
        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x6a; 32])
            .expect("fixture runtime artifact fingerprint is non-zero")
    }

    #[test]
    fn bundle_runner_wraps_emitted_assertion_without_condition_parsing() {
        let assertion = RuntimeAssertion::new(
            RuntimeAssertionGuardId::try_from_bytes([7; 16]).expect("fixture guard"),
            "opaque-condition-label".to_owned(),
            "must be ready".to_owned(),
            RuntimeAssertionProfile::Always,
        );
        let expected = RuntimeAssertionFailure::new(assertion.clone());
        let result = RuntimeStepResult {
            output: arcweft_core::step::RuntimeStepOutput {
                effects: arcweft_core::step::RuntimeEffectBatch {
                    line: vec![LineEffectRequest::Assert(assertion)],
                    ..arcweft_core::step::RuntimeEffectBatch::default()
                },
                ..arcweft_core::step::RuntimeStepOutput::default()
            },
            fiber_status: FlowFiberStatus::Done(arcweft_core::engine::FlowExit::Done),
            stop_reason: arcweft_core::step::RuntimeStepStopReason::Done,
            stats: arcweft_core::step::RuntimeStepStats::default(),
        };

        let (summary, tasks, audio) = BundleRunnerStepSummary::from_result(0, result);

        assert_eq!(summary.assertion_failures, vec![expected]);
        assert_eq!(summary.line_effects, vec!["assert"]);
        assert!(tasks.is_empty());
        assert!(audio.is_empty());
    }

    #[test]
    fn bundle_runner_session_captures_per_run_host_state_and_steps_incrementally() {
        let configured = std::cell::Cell::new(false);
        let bundle = dialogue_bundle();
        let mut session = BundleRunnerSession::with_adapter_installer(
            &bundle,
            &BundleRunnerOptions {
                steps: 4,
                mode: BundleRunnerStepMode::Game,
                max_ops: 64,
                ..BundleRunnerOptions::default()
            },
            |_source_path, builder| {
                configured.set(true);
                Ok(builder)
            },
        )
        .expect("session starts with capturing adapter installer");

        assert!(configured.get());
        assert!(!session.is_finished());
        let first = session
            .step()
            .expect("first step succeeds")
            .expect("first step runs");

        assert_eq!(first.summary.index, 0);
        assert_eq!(session.steps().len(), 1);
    }

    #[test]
    fn bundle_runner_preserves_typed_flow_events_for_embedding_hosts() {
        let bundle = dialogue_bundle();
        let report = run_bundle_with_native_adapters(
            &bundle,
            &BundleRunnerOptions {
                steps: 4,
                mode: BundleRunnerStepMode::Game,
                max_ops: 64,
                ..BundleRunnerOptions::default()
            },
            &[],
        )
        .expect("bundle runs");

        assert!(report.steps.iter().any(|step| {
            step.flow_events.iter().any(|event| {
                matches!(
                    event,
                    FlowEvent::DialogueLine { line, .. }
                        if line.public_label().as_str() == "say.opening"
                )
            })
        }));
        let json = serde_json::to_value(&report).expect("report serializes");
        assert!(
            json["steps"]
                .as_array()
                .expect("steps are serialized")
                .iter()
                .all(|step| step.get("flow_events").is_none())
        );
    }

    #[test]
    fn bundle_runner_rejects_missing_image_asset_virtual_file() {
        let bundle = dialogue_bundle().with_image_assets([BundleImageAsset {
            id: "asset.bg.room".to_owned(),
            file: BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Asset,
                path: "bg/room.png".to_owned(),
            },
            format: BundleImageFormat::Png,
            animation: BundleImageAnimation::Static,
            dimensions: None,
        }]);

        let error = run_bundle_with_native_adapters(
            &bundle,
            &BundleRunnerOptions {
                steps: 1,
                ..BundleRunnerOptions::default()
            },
            &[],
        )
        .expect_err("missing image file is rejected before execution");

        assert!(matches!(
            error,
            BundleRunnerError::InvalidImageAsset(
                arcweft_bundle::BundleCodecError::MissingImageFile {
                    asset_id,
                    space: BundleVirtualFileSpace::Asset,
                    path,
                }
            ) if asset_id == "asset.bg.room" && path == "bg/room.png"
        ));
    }

    #[test]
    fn bundle_runner_rejects_agent_controller_bundle_kind() {
        let mut bundle = dialogue_bundle();
        bundle.bundle_kind = BundleKind::AgentController;

        let error = run_bundle_with_native_adapters(
            &bundle,
            &BundleRunnerOptions {
                steps: 1,
                ..BundleRunnerOptions::default()
            },
            &[],
        )
        .expect_err("game runner must not execute agent controller bundles");

        assert!(matches!(
            error,
            BundleRunnerError::UnsupportedBundleKind {
                kind: BundleKind::AgentController
            }
        ));
    }

    #[test]
    fn bundle_runner_rejects_corrupt_image_asset_bytes_before_execution() {
        let image_file = BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "bg/room.png".to_owned(),
            bytes: b"not a png".to_vec(),
        };
        let bundle = dialogue_bundle()
            .with_virtual_files([image_file.clone()])
            .with_image_assets([BundleImageAsset {
                id: "asset.bg.room".to_owned(),
                file: image_file.file_ref(),
                format: BundleImageFormat::Png,
                animation: BundleImageAnimation::Static,
                dimensions: Some(BundleImageDimensions::new(2, 1)),
            }]);

        let error = run_bundle_with_native_adapters(
            &bundle,
            &BundleRunnerOptions {
                steps: 1,
                ..BundleRunnerOptions::default()
            },
            &[],
        )
        .expect_err("corrupt image bytes are rejected before execution");

        assert!(matches!(
            error,
            BundleRunnerError::DecodeImageAsset { asset_id, path, .. }
                if asset_id == "asset.bg.room" && path == "bg/room.png"
        ));
    }

    #[test]
    fn bundle_runner_rejects_image_asset_metadata_mismatch_before_execution() {
        let image_file = BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "bg/poster.webp".to_owned(),
            bytes: sample_image_asset_bytes("bg/poster.webp"),
        };
        let bundle = dialogue_bundle()
            .with_virtual_files([image_file.clone()])
            .with_image_assets([BundleImageAsset {
                id: "asset.bg.poster".to_owned(),
                file: image_file.file_ref(),
                format: BundleImageFormat::WebP,
                animation: BundleImageAnimation::Animated,
                dimensions: Some(BundleImageDimensions::new(2, 1)),
            }]);

        let error = run_bundle_with_native_adapters(
            &bundle,
            &BundleRunnerOptions {
                steps: 1,
                ..BundleRunnerOptions::default()
            },
            &[],
        )
        .expect_err("static webp cannot be declared animated");

        assert!(matches!(
            error,
            BundleRunnerError::ImageAssetMetadataMismatch { asset_id, field: "animation", .. }
                if asset_id == "asset.bg.poster"
        ));
    }

    fn sample_image_asset_bytes(path: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("samples")
                .join("assets")
                .join(path),
        )
        .expect("sample image asset is readable")
    }

    fn dialogue_bundle() -> ArcweftBundle {
        let plan = RuntimePlan::new(
            vec![RuntimeFlow {
                id: flow_id("flow.main"),
                ops: vec![
                    FlowOp::Dialogue {
                        line: line_id("line.opening"),
                        task_group: 0,
                    },
                    FlowOp::Return("done".to_owned()),
                ],
            }],
            vec![LineTaskGroup::default()],
        )
        .expect("runtime plan is valid")
        .with_entries(vec![RuntimeEntrySpec {
            id: EntryRuntimeId::from_source_entity_body("entry.main")
                .expect("test entry ID is valid"),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([1; 32]),
            target: RuntimeEntryTarget::Flow(flow_id("flow.main")),
            roles: RuntimeEntryRoles::None,
        }]);
        let source_map = source_map("dialogue-bundle.arcw", "flow main { dialogue }");
        let dialogue_content =
            DialogueContentCatalog::try_from_records(vec![DialogueContentSpec::new(
                line_id("line.opening"),
                TextKey::try_new("text.opening").expect("text key"),
                RichTextDocument::new(vec![RichTextNode::Text {
                    text: "Opening".to_owned(),
                }]),
                Vec::new(),
                source_map
                    .primary_document()
                    .expect("fixture source map retains its source")
                    .product_source_ref(),
            )])
            .expect("final dialogue content catalog");
        let product_awbc = AwbcLowerer::new(&plan, &dialogue_content, "dialogue-bundle.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        let bytecode = BytecodeProgram::from_runtime_plan(plan);
        ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("entry.main".to_owned()),
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 2,
                    line_task_groups: 1,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            source_map,
            bytecode,
            dialogue_content,
        )
        .expect("standard dialogue source joins source map")
        .with_product_awbc(product_awbc)
    }

    fn source_map(label: &str, text: &str) -> SourceMapSection {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new(label).expect("source ID"),
            SourceName::path(label),
            text,
        )
        .expect("source document");
        SourceMapSection::try_from_documents(&[&document]).expect("source map")
    }

    fn flow_id(value: &str) -> FlowRuntimeId {
        FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
    }

    fn line_id(value: &str) -> RuntimeLineId {
        RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
    }
}
