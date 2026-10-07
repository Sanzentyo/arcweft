use crate::bundle_asset::BundleAssetAdapter;
use crate::native_task::{
    NativeAdapterRegistrar, NativeFileRoots, NativeTaskBridge, NativeTaskStats,
    standard_cli_registry_builder,
};
use crate::stats::{RuntimeExecutorStats, runtime_executor_stats};
use arcweft_bundle::container::{BundleView, ReadBudget};
use arcweft_bundle::{
    ArcweftBundle, BundleAdapterManifest, BundleArtifactIdentity, BundleImageAnimation,
    BundleImageAsset, BundleImageDimensions, BundleImageFormat, BundleKind, BundleVirtualFile,
};
use arcweft_core::awbc::{
    product_step::AwbcProductStepBuildError,
    schema::{AwbcEntryId, AwbcProgram},
};
use arcweft_core::effect::{LineEffectRequest, RuntimeAssertionFailure};
use arcweft_core::engine::{EngineStartError, FlowFiber, FlowFiberStatus, FlowStatusLabelStyle};
use arcweft_core::executor::{ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::plan::{EntryRuntimeId, FlowEvent, RuntimePlanBuilder};
use arcweft_core::step::{
    RuntimeHostCallRequest, RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode,
    RuntimeStepOptions, RuntimeStepResult,
};
use arcweft_core::task::{GenerationId, LogicalEpoch};
use arcweft_core::value::{
    RuntimeBundleAssetArtifactDigest, RuntimeBundleAssetContext, RuntimeBundleAssetValueError,
    RuntimeFormatContext,
};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_id::LocaleTag;
use arcweft_interaction_model::audio::AudioCommandEnvelope;
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_accelerator::{RuntimePureAccelerator, RuntimePureAcceleratorConfig};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
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
    let artifact_identity = logical_bundle_artifact_identity(bundle)?;
    execute_bundle_with_native_adapters(
        bundle,
        artifact_identity,
        options,
        adapter_registrars,
        &mut phases,
    )
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
    let artifact_identity = run_bundle_runner_phase(&mut phases, "artifact_identity", || {
        BundleView::parse(&bytes, ReadBudget::default())
            .map(|view| BundleArtifactIdentity::AwfbContainer {
                identity: view.artifact_identity(),
            })
            .map_err(BundleRunnerError::ContainerArtifactIdentity)
    })?;
    let bundle = run_bundle_runner_phase(&mut phases, "decode_bundle", || {
        ArcweftBundle::from_awfb_slice_with_resource_types(
            &bytes,
            options.engine_resource_types.as_ref(),
        )
        .map_err(BundleRunnerError::DecodeBundle)
    })?;
    execute_bundle_with_native_adapters(
        &bundle,
        artifact_identity,
        options,
        adapter_registrars,
        &mut phases,
    )
}

/// Bundle execution options for embedding hosts.
#[derive(Clone, Debug)]
pub struct BundleRunnerOptions {
    pub entry: Option<EntryRuntimeId>,
    /// Host-selected initial locale; otherwise the bundle project default.
    pub active_locale: Option<LocaleTag>,
    pub steps: usize,
    pub mode: BundleRunnerStepMode,
    pub max_ops: usize,
    pub engine_resource_types: std::sync::Arc<ResourceTypeRegistry>,
}

impl Default for BundleRunnerOptions {
    fn default() -> Self {
        Self {
            entry: None,
            active_locale: None,
            steps: 8,
            mode: BundleRunnerStepMode::Drain,
            max_ops: 32,
            engine_resource_types: std::sync::Arc::new(ResourceTypeRegistry::empty()),
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
    #[error("failed to inspect the AWFB artifact identity: {0}")]
    ContainerArtifactIdentity(#[source] arcweft_bundle::container::ContainerError),
    #[error("failed to compute the logical bundle artifact identity: {0}")]
    LogicalArtifactIdentity(#[source] arcweft_bundle::BundleCodecError),
    #[error("failed to construct the bundle asset generation context: {0}")]
    BundleAssetContext(#[source] RuntimeBundleAssetValueError),
    #[error("failed to bind a bundle asset adapter: {0}")]
    BundleAssetAdapter(#[source] crate::bundle_asset::BundleAssetAdapterError),
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
    #[error(transparent)]
    ProductAwbcRuntime(#[from] AwbcProductStepBuildError),
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
    #[error("bundle runner exceeded the logical task epoch range")]
    LogicalEpochExhausted,
    #[error(transparent)]
    NativeTask(#[from] crate::native_task::NativeTaskBridgeError),
}

fn execute_bundle_with_native_adapters(
    bundle: &ArcweftBundle,
    artifact_identity: BundleArtifactIdentity,
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
    let host_policy = bundle_host_policy(bundle);
    let trace = run_bundle_runner_phase(phases, "run", || {
        run_product_runtime_steps(
            runtime_program,
            Some(workspace.source_path()),
            bundle,
            artifact_identity,
            options
                .active_locale
                .as_ref()
                .unwrap_or(bundle.manifest.locale.default_locale())
                .clone(),
            RuntimeStepRunConfig {
                steps: options.steps,
                mode: options.mode,
                max_ops: options.max_ops,
            },
            &host_policy,
            adapter_registrars,
        )
    })?;
    Ok(BundleRunnerReport {
        source: bundle.source_display_name().to_owned(),
        bytecode_instructions: bundle.manifest.runtime.bytecode_instructions,
        adapter_manifests: bundle.adapter_manifests.len(),
        phases: std::mem::take(phases),
        executor_stats: trace.executor_stats,
        native_io: trace.native_io,
        steps: trace.steps,
        final_status: trace.final_status,
    })
}

fn logical_bundle_artifact_identity(
    bundle: &ArcweftBundle,
) -> Result<BundleArtifactIdentity, BundleRunnerError> {
    bundle
        .logical_identity()
        .map(|identity| BundleArtifactIdentity::LogicalBundle { identity })
        .map_err(BundleRunnerError::LogicalArtifactIdentity)
}

fn bundle_asset_context(
    generation: GenerationId,
    identity: BundleArtifactIdentity,
) -> Result<RuntimeBundleAssetContext, BundleRunnerError> {
    let artifact =
        RuntimeBundleAssetArtifactDigest::try_from_bytes(identity.binding_digest().as_bytes())
            .map_err(BundleRunnerError::BundleAssetContext)?;
    Ok(RuntimeBundleAssetContext::new(generation, artifact))
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

pub(crate) fn validate_bundle_image_asset_metadata(
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

struct BundleRunnerRuntimeProgram {
    program: Box<AwbcProgram>,
    entry: AwbcEntryId,
}

fn bundle_runner_runtime_program(
    bundle: &ArcweftBundle,
    options: &BundleRunnerOptions,
) -> Result<BundleRunnerRuntimeProgram, BundleRunnerError> {
    let program = bundle.product_awbc().program().clone();
    let entry = selected_awbc_entry(&program, bundle, options)?;
    Ok(BundleRunnerRuntimeProgram {
        program: Box::new(program),
        entry,
    })
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

fn run_product_runtime_steps(
    program: BundleRunnerRuntimeProgram,
    source_path: Option<&Path>,
    bundle: &ArcweftBundle,
    artifact_identity: BundleArtifactIdentity,
    active_locale: LocaleTag,
    config: RuntimeStepRunConfig,
    host_policy: &HostCallPolicy,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<RuntimeRunTrace, BundleRunnerError> {
    let mut executor = RuntimeExecutorInstance::from_awbc_product(program, active_locale)?;
    run_runtime_steps_with_executor(
        &mut executor,
        NativeRunHost {
            source_path,
            policy: host_policy,
            adapter_registrars,
            bundle,
            artifact_identity,
        },
        config.steps,
        config.mode,
        config.max_ops,
    )
}

fn run_runtime_steps_with_executor(
    executor: &mut RuntimeExecutorInstance,
    host_config: NativeRunHost<'_>,
    steps: usize,
    mode: BundleRunnerStepMode,
    max_ops: usize,
) -> Result<RuntimeRunTrace, BundleRunnerError> {
    let asset_context = bundle_asset_context(executor.generation(), host_config.artifact_identity)?;
    let mut host = host_config
        .source_path
        .map(|path| {
            let mut builder =
                standard_cli_registry_builder(NativeFileRoots::for_bundle_workspace(path), &[])
                    .map_err(BundleRunnerError::NativeAdapter)?;
            let asset_adapter = BundleAssetAdapter::try_new(
                asset_context,
                host_config.artifact_identity,
                Arc::new(host_config.bundle.clone()),
            )
            .map_err(BundleRunnerError::BundleAssetAdapter)?;
            builder = builder
                .register(asset_adapter)
                .map_err(BundleRunnerError::NativeAdapter)?;
            let builder = host_config
                .adapter_registrars
                .iter()
                .try_fold(builder, |builder, register| register(path, builder))
                .map_err(BundleRunnerError::NativeAdapter)?;
            NativeTaskBridge::try_with_registry(host_config.policy.clone(), builder.build())
                .map_err(BundleRunnerError::NativeAdapter)
        })
        .transpose()?;
    let mut task_events = Vec::new();
    let mut host_call_results = Vec::new();
    let mut summaries = Vec::new();
    for step_index in 0..steps {
        if let Some(host) = host.as_mut() {
            host.pump_main_thread()
                .map_err(BundleRunnerError::NativeAdapter)?;
            task_events.extend(host.poll_completions()?);
            host_call_results.extend(host.take_host_call_results());
        }
        let result = executor.step(
            RuntimeStepInput {
                task_events: std::mem::take(&mut task_events),
                host_call_results: std::mem::take(&mut host_call_results),
                ..RuntimeStepInput::default()
            },
            step_options(mode, max_ops),
        );
        let (summary, task_requests, host_call_requests, _audio_commands) =
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
            task_events.extend(
                host.complete_tasks_with_generation_and_bundle_asset_context(
                    executor.program_owner(),
                    executor.generation(),
                    logical_epoch(step_index)?,
                    task_requests,
                    Some(asset_context),
                )?,
            );
            host_call_results.extend(host.complete_host_calls(
                executor.program_owner(),
                logical_epoch(step_index)?,
                host_call_requests,
            ));
        }
    }
    Ok(RuntimeRunTrace {
        steps: summaries,
        final_status: executor
            .fiber()
            .status
            .status_label(FlowStatusLabelStyle::Runtime),
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
}

struct RuntimeRunTrace {
    steps: Vec<BundleRunnerStepSummary>,
    final_status: String,
    executor_stats: RuntimeExecutorStats,
    native_io: NativeTaskStats,
}

#[derive(Clone, Copy)]
struct NativeRunHost<'a> {
    source_path: Option<&'a Path>,
    policy: &'a HostCallPolicy,
    adapter_registrars: &'a [NativeAdapterRegistrar],
    bundle: &'a ArcweftBundle,
    artifact_identity: BundleArtifactIdentity,
}

struct RuntimeExecutorInstance {
    executor: ArcweftRuntimeExecutor,
    pure: RuntimePureAccelerator,
}

impl RuntimeExecutorInstance {
    fn program_owner(&self) -> arcweft_core::task::RuntimeProgramOwner {
        self.executor.program_owner()
    }

    fn generation(&self) -> GenerationId {
        self.executor.generation()
    }

    fn from_awbc_product(
        program: BundleRunnerRuntimeProgram,
        active_locale: LocaleTag,
    ) -> Result<Self, BundleRunnerError> {
        let pure_plan = Arc::new(
            RuntimePlanBuilder::new()
                .finish()
                .expect("empty runtime plan is valid"),
        );
        let mut executor =
            ArcweftRuntimeExecutor::from_awbc_product(*program.program, program.entry)?;
        let format_context = RuntimeFormatContext::new(active_locale);
        executor.set_format_context(format_context.clone());
        let mut pure = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig::default(),
            &pure_plan,
        );
        pure.set_format_context(format_context);
        Ok(Self { executor, pure })
    }

    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult {
        self.executor
            .step_with_pure_backend(input, options, &mut self.pure)
    }

    fn fiber(&self) -> &FlowFiber {
        self.executor.fiber()
    }

    fn executor_stats(&self) -> RuntimeExecutorStats {
        runtime_executor_stats(self.executor.fast_path_ops(), &self.pure)
    }
}

fn logical_epoch(index: usize) -> Result<LogicalEpoch, BundleRunnerError> {
    u64::try_from(index)
        .map(LogicalEpoch)
        .map_err(|_| BundleRunnerError::LogicalEpochExhausted)
}

impl BundleRunnerStepSummary {
    fn from_result(
        index: usize,
        result: RuntimeStepResult,
    ) -> (
        Self,
        Vec<arcweft_core::task::TaskSubmission>,
        Vec<RuntimeHostCallRequest>,
        Vec<AudioCommandEnvelope>,
    ) {
        let RuntimeStepResult {
            mut output,
            fiber_status,
            stop_reason,
            stats,
        } = result;
        let task_requests = std::mem::take(&mut output.requests.tasks);
        let host_call_requests = std::mem::take(&mut output.requests.host_calls);
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
            host_call_requests,
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
        ..RuntimeStepOptions::default()
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
    use arcweft_audio_core::graph::{
        AudioAsset, AudioBusDef, AudioDecodeStrategy, AudioFormat, AudioGraph,
    };
    use arcweft_bundle::resource_codec::SourceMapSection;
    use arcweft_bundle::{
        ArcweftBundle, BundleImageAnimation, BundleImageAsset, BundleImageDimensions,
        BundleImageFormat, BundleManifest, BundleRuntimeSummary, BundleVirtualFile,
        BundleVirtualFileRef, BundleVirtualFileSpace,
    };
    use arcweft_bundle_assets::BundleAssetValidationError;
    use arcweft_character::{
        id::CharacterId,
        presentation_name::{
            CharacterPresentationCatalogGeneration, CharacterPresentationCatalogRevision,
            CharacterPresentationSemanticDigest,
        },
    };
    use arcweft_core::effect::{
        RuntimeAssertion, RuntimeAssertionGuardId, RuntimeAssertionProfile,
    };
    use arcweft_core::entry::{
        EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
        RuntimeFlowSchema, RuntimeSchemaLimits,
    };
    use arcweft_core::pattern::{RuntimeCheckedType, runtime_standard_opaque_type};
    use arcweft_core::plan::{
        FlowRuntimeId, RuntimeDialogueContentPlanSeed, RuntimeEntryKind, RuntimeEntrySpec,
        RuntimeEntryTarget, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed,
        RuntimeFlowSeed, RuntimeLineId, RuntimePlanBuilder,
    };
    use arcweft_core::task::{
        AssetLoadKind, AssetRequest, BoundTaskSpec, CancelScopeId, HostTaskRequest, TaskClass,
        TaskId, TaskKey, TaskOutcomeContract, TaskPolicy, TaskPriority, TaskPublicationRevision,
        TaskSpec,
    };
    use arcweft_core::value::{
        RuntimeAssetContentDigest, RuntimeAssetErrorValue, RuntimeAudioHandleValue,
        RuntimeBundleAssetBinding, RuntimeBundleAssetContext, RuntimeBundleAssetFailureReason,
        RuntimeBundleAssetResourceId, RuntimeImageHandleValue, RuntimeValue,
        RuntimeVoiceErrorValue,
    };
    use arcweft_dialogue::{
        CharacterDialogueCharacterDeclaration, CharacterDialogueConfig,
        CharacterDialogueGenerationDeclaration, CharacterDialoguePresentationContract,
        CharacterDialogueRolePayloadCodec, CharacterDialogueRuntimeCustomFieldCatalog,
        CharacterDialogueRuntimeDefault, CharacterDialogueRuntimeRole as DialogueRole,
        CharacterDialogueRuntimeRoleBody, CharacterDialogueRuntimeRoleType,
        CharacterDialogueRuntimeRoleTypes, CharacterDialogueType, CharacterDialogueVisualType,
        DialoguePresentationProfile, DialogueProfileRevision,
        character_presentation::{
            CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan,
        },
    };
    use arcweft_host_adapter::{
        HostAdapter, HostTaskCompletion, HostTaskSubmission, HostTaskSubmissionContext,
    };
    use arcweft_id::TextKey;
    use arcweft_interaction_model::audio::{
        AudioBusId, AudioLoopMode, AudioResourceId, GainDbMilli,
    };
    use arcweft_resource_model::registry::ResourceTypeRegistry;
    use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
    use arcweft_text_model::{
        DialogueContentCatalog, DialogueContentFragmentTemplate, DialogueContentSpec,
        RichTextDocument, RichTextNode,
    };
    use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId, ViewRegistry};

    fn fixture_runtime_artifact_fingerprint() -> arcweft_core::effect::RuntimeArtifactFingerprint {
        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x6a; 32])
            .expect("fixture runtime artifact fingerprint is non-zero")
    }

    fn test_character_plan() -> CheckedCharacterPresentationPlan {
        CheckedCharacterPresentationPlan::try_new(
            CharacterPresentationTargetEvidence::Exact(
                CharacterId::try_new("character.fixture").unwrap(),
            ),
            CharacterPresentationCatalogGeneration::new(
                CharacterPresentationCatalogRevision::INITIAL,
                CharacterPresentationSemanticDigest::from_bytes([1; 32]),
            ),
        )
        .unwrap()
    }

    fn test_dialogue_profile_revision() -> DialogueProfileRevision {
        let source = SourceDocument::try_new(
            SourceDocumentId::try_new("runtime-host-dialogue-profile-fixture").unwrap(),
            SourceName::Memory,
            "schema = 1\n",
        )
        .unwrap();
        let sources = SourceSetRevision::try_for_identities([source.identity()]).unwrap();
        DialogueProfileRevision::from_admitted_parts(
            source.identity().clone(),
            sources,
            sources,
            ViewProgramId::try_new("view_program.runtime_host_dialogue").unwrap(),
            AcceptedViewProgramRevision::try_from_bytes([0x44; 32]).unwrap(),
            ResourceTypeRegistry::empty().digest(),
        )
    }

    fn dialogue_runtime_roles() -> CharacterDialogueRuntimeRoleTypes {
        CharacterDialogueRuntimeRoleTypes::new(
            DialogueRole::AUTHORED_BASE.map(|role| {
                let body = if role == DialogueRole::RichText {
                    CharacterDialogueRuntimeRoleBody::bound(
                        CharacterDialogueRolePayloadCodec::RichTextProperties
                            .payload_schema()
                            .expect("RichText payload schema")
                            .root(),
                        CharacterDialogueRolePayloadCodec::RichTextProperties,
                    )
                } else {
                    CharacterDialogueRuntimeRoleBody::Unbound
                };
                CharacterDialogueRuntimeRoleType::new(
                    arcweft_core::pattern::RuntimeSemanticTypeId::from_bytes(
                        [20 + role.canonical_tag(); 32],
                    ),
                    body,
                )
            }),
            arcweft_core::pattern::RuntimeSemanticTypeId::from_bytes([50; 32]),
        )
    }

    fn with_dialogue_generation(bundle: ArcweftBundle) -> ArcweftBundle {
        let character = CharacterId::try_new("character.fixture").expect("fixture character ID");
        let product = arcweft_bundle::resource_codec::view::ValidatedViewProduct::try_new(
            Some(bundle.source_map.clone()),
            bundle.view_program.clone(),
            bundle.view_style.clone(),
            arcweft_bundle::resource_codec::view::ViewProductValidationLimits::default(),
        )
        .expect("fixture View product validates");
        let mut views = ViewRegistry::default();
        product
            .program()
            .expect("fixture includes standard dialogue View")
            .register_runtime_views(&mut views)
            .expect("fixture View registry accepts standard dialogue View");
        let view_fingerprint = arcweft_core::entry::RuntimeValueDigest::from_bytes(
            *views
                .runtime_digest_v1()
                .expect("fixture View registry digest")
                .as_bytes(),
        );
        let style_fingerprint = bundle.view_style.as_ref().map(|style| {
            arcweft_core::entry::RuntimeValueDigest::from_bytes(
                style
                    .canonical_digest()
                    .expect("fixture style digest")
                    .as_bytes(),
            )
        });
        let profile = DialoguePresentationProfile::engine_default();
        let roles = dialogue_runtime_roles();
        let config = CharacterDialogueConfig::try_from_presentation_profile(&profile, &roles)
            .expect("fixture CharacterDialogue defaults");
        let exact = CharacterDialogueType::exact(character.clone());
        let declaration = CharacterDialogueGenerationDeclaration::try_new(
            [(
                character.clone(),
                CharacterDialogueCharacterDeclaration::new(
                    exact.runtime_semantic_identity(),
                    CharacterDialogueVisualType::Absent,
                    CharacterDialogueRuntimeDefault::new(character, config),
                ),
            )],
            CharacterDialogueType::any().runtime_semantic_identity(),
            arcweft_core::pattern::RuntimeSemanticTypeId::from_bytes([56; 32]),
            roles,
            CharacterDialogueRuntimeCustomFieldCatalog::try_new([])
                .expect("empty fixture custom field catalog"),
            CharacterDialoguePresentationContract::try_new(
                profile,
                test_dialogue_profile_revision(),
                view_fingerprint,
                style_fingerprint,
            )
            .expect("fixture presentation contract"),
        )
        .expect("fixture generation declaration");
        bundle.with_character_dialogue_generation(declaration)
    }

    #[test]
    fn bundle_asset_adapter_publishes_typed_results_for_validated_image_and_voice_bytes() {
        let audio_bytes = tiny_wav_bytes();
        let (bundle, image_bytes) = asset_test_bundle(audio_bytes.clone());
        let artifact =
            logical_bundle_artifact_identity(&bundle).expect("logical artifact identity");
        let context = bundle_asset_context(GenerationId::new(7), artifact)
            .expect("generation-bound asset context");
        let adapter = BundleAssetAdapter::try_new(context, artifact, Arc::new(bundle.clone()))
            .expect("generation asset catalog");

        let (image_task, image_bound) = asset_task(AssetLoadKind::Image, "asset.bg.room");
        let image_result = submit_asset(&adapter, &image_task, &image_bound, context);
        let (case, Some(image_value)) = image_result
            .clone()
            .try_into_builtin_variant_case()
            .expect("image result carrier")
        else {
            panic!("image load returns Result::Ok")
        };
        assert_eq!(
            case,
            arcweft_core::pattern::RuntimeBuiltinVariantCaseIdentity::ResultOk
        );
        let image_handle = RuntimeImageHandleValue::try_from_runtime_value(&image_value)
            .expect("typed image handle");
        assert_eq!(image_handle.binding().context(), context);
        assert_eq!(image_handle.binding().id().as_str(), "asset.bg.room");
        assert_eq!(
            image_handle.binding().content_digest(),
            RuntimeAssetContentDigest::try_for_bytes(&image_bytes).expect("image digest")
        );
        assert_eq!(
            adapter
                .resolver()
                .resolve_image_handle(&image_handle)
                .expect("validated image bytes")
                .as_ref(),
            image_bytes.as_slice()
        );
        adapter
            .validate_image_result(&image_result)
            .expect("saved image result validates against exact catalog");

        let (voice_task, voice_bound) = asset_task(AssetLoadKind::Voice, "asset.voice.opening");
        let voice_result = submit_asset(&adapter, &voice_task, &voice_bound, context);
        let (case, Some(voice_value)) = voice_result
            .clone()
            .try_into_builtin_variant_case()
            .expect("voice result carrier")
        else {
            panic!("voice load returns Result::Ok")
        };
        assert_eq!(
            case,
            arcweft_core::pattern::RuntimeBuiltinVariantCaseIdentity::ResultOk
        );
        let voice_handle = RuntimeAudioHandleValue::try_from_runtime_value(&voice_value)
            .expect("typed audio handle");
        assert_eq!(voice_handle.binding().context(), context);
        assert_eq!(voice_handle.binding().id().as_str(), "asset.voice.opening");
        assert_eq!(
            voice_handle.binding().content_digest(),
            RuntimeAssetContentDigest::try_for_bytes(&audio_bytes).expect("audio digest")
        );
        assert_eq!(
            adapter
                .resolver()
                .resolve_voice_handle(&voice_handle)
                .expect("validated voice bytes")
                .as_ref(),
            audio_bytes.as_slice()
        );
        adapter
            .validate_voice_result(&voice_result)
            .expect("saved voice result validates against exact catalog");

        let (missing_task, missing_bound) = asset_task(AssetLoadKind::Image, "asset.bg.missing");
        let missing_result = submit_asset(&adapter, &missing_task, &missing_bound, context);
        let (case, Some(error_value)) = missing_result
            .clone()
            .try_into_builtin_variant_case()
            .expect("missing image result carrier")
        else {
            panic!("missing image returns Result::Err")
        };
        assert_eq!(
            case,
            arcweft_core::pattern::RuntimeBuiltinVariantCaseIdentity::ResultErr
        );
        let missing =
            RuntimeAssetErrorValue::try_from_runtime_value(&error_value).expect("typed AssetError");
        assert_eq!(
            missing.failure().reason(),
            RuntimeBundleAssetFailureReason::Missing
        );
        adapter
            .validate_image_result(&missing_result)
            .expect("saved missing result matches the bundle catalog");

        let (bad_bundle, _) = asset_test_bundle(b"not a wav".to_vec());
        let bad_artifact =
            logical_bundle_artifact_identity(&bad_bundle).expect("bad-byte logical identity");
        let bad_context = bundle_asset_context(GenerationId::new(8), bad_artifact)
            .expect("bad-byte generation context");
        adapter
            .bind_generation(bad_context, bad_artifact, Arc::new(bad_bundle))
            .expect("second generation catalog is retained");
        let (bad_voice_task, bad_voice_bound) =
            asset_task(AssetLoadKind::Voice, "asset.voice.opening");
        let bad_voice_result =
            submit_asset(&adapter, &bad_voice_task, &bad_voice_bound, bad_context);
        let (case, Some(error_value)) = bad_voice_result
            .clone()
            .try_into_builtin_variant_case()
            .expect("decode failure result carrier")
        else {
            panic!("invalid audio returns Result::Err")
        };
        assert_eq!(
            case,
            arcweft_core::pattern::RuntimeBuiltinVariantCaseIdentity::ResultErr
        );
        let decode =
            RuntimeVoiceErrorValue::try_from_runtime_value(&error_value).expect("typed VoiceError");
        assert_eq!(decode.failure().context(), bad_context);
        assert_eq!(decode.failure().id().as_str(), "asset.voice.opening");
        assert_eq!(
            decode.failure().reason(),
            RuntimeBundleAssetFailureReason::Decode
        );
        assert_eq!(
            decode.failure().content_digest(),
            Some(RuntimeAssetContentDigest::try_for_bytes(b"not a wav").expect("bad bytes digest"))
        );
        adapter
            .validate_voice_result(&bad_voice_result)
            .expect("saved decode failure matches exact bad-byte generation");
        assert!(
            adapter
                .resolver()
                .validate_owned_value(&voice_value)
                .expect("standalone handle validation")
        );

        let forged_binding = RuntimeBundleAssetBinding::try_new(
            context,
            RuntimeBundleAssetResourceId::try_new("asset.bg.room")
                .expect("canonical forged handle resource ID"),
            RuntimeAssetContentDigest::try_for_bytes(b"forged content")
                .expect("forged content digest"),
        )
        .expect("structurally valid forged handle");
        let forged_value = RuntimeImageHandleValue::from_binding(forged_binding)
            .into_runtime_value()
            .expect("typed image handle carrier");
        assert_eq!(
            adapter.resolver().validate_owned_value(&forged_value),
            Err(BundleAssetValidationError::ContentDigestMismatch)
        );
    }

    fn asset_task(kind: AssetLoadKind, id: &str) -> (TaskSpec, BoundTaskSpec) {
        let (ok, error) = match kind {
            AssetLoadKind::Image => (
                standard_opaque_checked_type(&["ImageHandle"]),
                standard_opaque_checked_type(&["AssetError"]),
            ),
            AssetLoadKind::Voice => (
                standard_opaque_checked_type(&["AudioHandle"]),
                standard_opaque_checked_type(&["VoiceError"]),
            ),
        };
        use arcweft_core::task::{
            GenerationId, NeedProducerContractDigest, NeedProducerFamily, NeedProducerInstance,
            NeedProducerSiteDigest, NeedProducerSpec, RuntimeTypeSemanticDigest,
            TaskAdmissionJournal, TaskPlanSemanticDigest,
        };
        let outcome = TaskOutcomeContract::new(RuntimeCheckedType::Result {
            ok: Box::new(ok),
            error: Box::new(error),
        });
        let producer = NeedProducerSpec::new(
            NeedProducerFamily::HostAdapterTask,
            NeedProducerContractDigest::from_bytes([1; 32]),
            TaskPlanSemanticDigest::from_bytes([2; 32]),
            NeedProducerSiteDigest::from_bytes([3; 32]),
            RuntimeTypeSemanticDigest::from_bytes(*outcome.payload_semantic_identity().as_bytes()),
            RuntimeValue::Tuple(vec![
                RuntimeValue::String(kind.as_str().into()),
                RuntimeValue::String(id.into()),
            ])
            .try_digest(1024)
            .unwrap(),
        );
        let task = TaskSpec {
            generation: GenerationId::new(1),
            producer: NeedProducerInstance::try_from(&producer).unwrap(),
            class: TaskClass::Cpu,
            priority: TaskPriority(0),
            cancel_scope: CancelScopeId("bundle-asset-test".into()),
            policy: TaskPolicy::JoinSameKey,
            outcome,
            request: HostTaskRequest::AssetLoad(AssetRequest {
                id: id.into(),
                kind: kind.as_str().into(),
            }),
            debug_label: id.into(),
        };
        let mut journal = TaskAdmissionJournal::default();
        let handle = journal.ensure_task(task.clone()).unwrap();
        let bound = BoundTaskSpec::bind(
            journal.submission(handle).unwrap(),
            None,
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
        (task, bound)
    }

    fn standard_opaque_checked_type(path: &[&str]) -> RuntimeCheckedType {
        RuntimeCheckedType::Opaque {
            owner: runtime_standard_opaque_type(path)
                .and_then(|spec| spec.monomorphic_owner())
                .expect("standard asset opaque owner"),
        }
    }

    fn submit_asset(
        adapter: &BundleAssetAdapter,
        task: &TaskSpec,
        bound: &BoundTaskSpec,
        context: RuntimeBundleAssetContext,
    ) -> RuntimeValue {
        let HostTaskSubmission::Completed(outcome) = HostAdapter::submit(
            adapter,
            bound,
            HostTaskSubmissionContext::new(
                arcweft_core::task::TaskDispatchIdentity::new(
                    bound.handle().correlation,
                    arcweft_core::task::LogicalEpoch(1),
                    arcweft_core::task::TaskSequence(1),
                ),
                TaskPublicationRevision::FIRST,
            )
            .with_bundle_asset_context(context),
        )
        .expect("asset adapter handles task") else {
            panic!("bundle asset result is synchronous")
        };
        let HostTaskCompletion::Ready(value) = outcome.completion else {
            panic!("typed missing/decode outcomes are Result values")
        };
        value.value().clone()
    }

    fn asset_test_bundle(audio_bytes: Vec<u8>) -> (ArcweftBundle, Vec<u8>) {
        let image_bytes = sample_image_asset_bytes("bg/room.png");
        let image_file = BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "bg/room.png".to_owned(),
            bytes: image_bytes.clone(),
        };
        let audio_file = BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "audio/opening.wav".to_owned(),
            bytes: audio_bytes,
        };
        let master_bus = AudioBusId::new("bus.master").expect("master bus");
        let bundle = ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: None,
                adapter: None,
                locale: arcweft_manifest_model::ProjectLocaleSpec::default(),
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                    entry_flow: None,
                    flows: 0,
                    bytecode_instructions: 0,
                    line_task_groups: 0,
                    stream_plans: 0,
                },
            },
            source_map("bundle-assets-test.arcw", ""),
            AwbcProgram::default(),
            DialogueContentCatalog::new(),
        )
        .expect("minimal bundle base")
        .with_virtual_files([image_file.clone(), audio_file])
        .with_image_assets([BundleImageAsset {
            id: "asset.bg.room".to_owned(),
            file: image_file.file_ref(),
            format: BundleImageFormat::Png,
            animation: BundleImageAnimation::Static,
            dimensions: None,
        }])
        .with_audio_graph(AudioGraph {
            master_bus: master_bus.clone(),
            assets: vec![AudioAsset {
                id: AudioResourceId::new("asset.voice.opening").expect("voice asset"),
                path: "audio/opening.wav".to_owned(),
                format: AudioFormat::Wav,
                strategy: AudioDecodeStrategy::Preload,
                default_loop: AudioLoopMode::None,
            }],
            buses: vec![AudioBusDef {
                id: master_bus,
                parent: None,
                gain: GainDbMilli::UNITY,
                muted: false,
                effects: Vec::new(),
            }],
            snapshots: Vec::new(),
        });
        (bundle, image_bytes)
    }

    fn tiny_wav_bytes() -> Vec<u8> {
        let samples = [0_i16; 32];
        let data_len = u32::try_from(samples.len() * 2).expect("small WAV data length");
        let mut bytes = Vec::with_capacity(44 + usize::try_from(data_len).expect("WAV length"));
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36_u32 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&8_000_u32.to_le_bytes());
        bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
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

        let (summary, tasks, host_calls, audio) = BundleRunnerStepSummary::from_result(0, result);

        assert_eq!(summary.assertion_failures, vec![expected]);
        assert_eq!(summary.line_effects, vec!["assert"]);
        assert!(tasks.is_empty());
        assert!(host_calls.is_empty());
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
        let bundle = dialogue_bundle().with_agent_manifest(
            serde_json::from_value(serde_json::json!({
                "schema_version": 1,
                "bundle_kind": "agent_controller",
                "entry_id": "entry.agent.fixture",
                "controller_id": "game::crate.fixture",
                "entry_binding_hash": "blake3:entry",
                "controller_contract_hash": "blake3:contract",
                "policy_hash": "blake3:policy",
                "source_hash": "blake3:source",
                "compiler_version": "arcweft-test",
                "project_binding": {
                    "program_hash": "blake3:program",
                    "mode": "strict",
                    "required_entities": []
                },
                "declared_effects": [],
                "verified_effects": {
                    "analysis_version": 1,
                    "declared": [],
                    "inferred": [],
                    "digest": "blake3:effects"
                },
                "budget": {
                    "logical_timeout_millis": 30000,
                    "max_vm_steps": 100000,
                    "max_host_calls": 256,
                    "max_observations": 256,
                    "max_captures": 16,
                    "max_capture_bytes": 67108864,
                    "max_rag_queries": 8,
                    "max_context_bytes": 1048576
                },
                "debug_map_hash": null
            }))
            .expect("fixture Agent manifest is schema-valid"),
        );

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
        let line = line_id("line.opening");
        let flow = flow_id("flow.main");
        let template = DialogueContentFragmentTemplate::try_new_canonical(
            arcweft_core::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                .expect("template identity"),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            RichTextDocument::new(vec![RichTextNode::Text {
                text: "Opening".to_owned(),
            }]),
        )
        .expect("dialogue template");
        let mut builder = RuntimePlanBuilder::new();
        let unit_result = arcweft_core::plan::RuntimeDialogueResultTargetSeed::discard(
            arcweft_core::pattern::RuntimeCheckedType::Unit.semantic_identity_digest(),
        );
        let dialogue_target_type = arcweft_dialogue::CharacterDialogueType::exact(
            CharacterId::try_new("character.fixture").expect("fixture character ID"),
        );
        let dialogue_target_owner = dialogue_target_type.runtime_opaque_owner();
        let dialogue_target_value = dialogue_target_owner
            .try_wrap(RuntimeValue::Unit)
            .expect("fixture CharacterDialogue value wraps");
        builder
            .admit_type_batch(
                [
                    arcweft_core::plan::RuntimePlanTypeSeed::new(
                        unit_result.ty(),
                        arcweft_core::plan::RuntimePlanTypeProjection::Unit,
                    ),
                    arcweft_core::plan::RuntimePlanTypeSeed::new(
                        dialogue_target_type.runtime_semantic_identity(),
                        arcweft_core::plan::RuntimePlanTypeProjection::Opaque {
                            producer: dialogue_target_owner.producer().clone(),
                            admission: dialogue_target_owner.admission(),
                            value_class: dialogue_target_owner.value_class(),
                            persistence: dialogue_target_owner.persistence(),
                            arguments: Box::default(),
                        },
                    ),
                ],
                [],
            )
            .expect("dialogue target and unit result types admit");
        let content = builder
            .push_dialogue_content_seed(RuntimeDialogueContentPlanSeed {
                line: line.clone(),
                template: arcweft_core::plan::RuntimeDialogueContentTemplateManifestSeed {
                    id: template.id(),
                    digest: template.digest(),
                    slots: Box::default(),
                    effects: Box::default(),
                },
                values: Box::default(),
                effect_sites: Box::default(),
                marks: Box::default(),
                effect_site_count: Default::default(),
            })
            .expect("dialogue content admits");
        let line_task_group = builder
            .push_line_task_group_seed(arcweft_core::plan::RuntimeLineTaskGroupSeed {
                definition:
                    arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [71; 32],
                    ),
                activation_ops: vec![RuntimeFlowOpSeed::CommitDialogueResult {
                    value: RuntimeExprSeed::new(
                        unit_result.ty(),
                        RuntimeExprSeedKind::Value(RuntimeValue::Unit),
                    ),
                }],
                result_type: unit_result.ty(),
                handle_sites: Box::default(),
                root: arcweft_core::plan::RuntimeLineTaskNodeSeed::Action(Vec::new()),
                cancel_rules: Box::default(),
                cleanup_completed: Vec::new(),
                cleanup_cancelled: Vec::new(),
                cleanup_failed: Vec::new(),
                cleanup_policy: Default::default(),
            })
            .expect("line-task group admits");
        builder
            .attach_line_task_group_seed(&content, &line_task_group)
            .expect("line-task group attaches to dialogue content");
        builder
            .admit_type_batch(
                [arcweft_core::plan::RuntimePlanTypeSeed::new(
                    arcweft_core::pattern::RuntimeCheckedType::String.semantic_identity_digest(),
                    arcweft_core::plan::RuntimePlanTypeProjection::String,
                )],
                [],
            )
            .expect("Flow result type admits");
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                flow.clone(),
                arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                    arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [61; 32],
                    ),
                    None,
                    Box::new([]),
                    arcweft_core::pattern::RuntimeCheckedType::String.semantic_identity_digest(),
                    arcweft_core::plan::RuntimeEffectSet::empty(),
                ),
                arcweft_core::plan::RuntimeExecutableBodySeed {
                    effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                    ops: (vec![
                        RuntimeFlowOpSeed::Dialogue {
                            target: RuntimeExprSeed::new(
                                dialogue_target_type.runtime_semantic_identity(),
                                RuntimeExprSeedKind::Value(dialogue_target_value),
                            ),
                            content,
                            result: unit_result,
                        },
                        RuntimeFlowOpSeed::Return("done".to_owned()),
                    ])
                    .into_boxed_slice(),
                },
            ))
            .expect("flow admits");
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: flow.clone(),
                parameters: Vec::new(),
            })
            .expect("flow schema admits");
        builder
            .push_flow_executable(RuntimeFlowExecutable {
                flow: flow.clone(),
                contract: FlowContractHash::from_bytes([0x7b; 32]),
                controller: None,
            })
            .expect("flow executable admits");
        builder
            .push_entry(RuntimeEntrySpec {
                id: EntryRuntimeId::from_source_entity_body("entry.main")
                    .expect("test entry ID is valid"),
                kind: RuntimeEntryKind::Cli,
                binding: EntryBindingIdentity::from_bytes([1; 32]),
                target: RuntimeEntryTarget::Flow(flow),
                roles: RuntimeEntryRoles::None,
            })
            .expect("entry admits");
        let plan = builder.finish().expect("runtime plan is valid");
        let source_map = source_map("dialogue-bundle.arcw", "flow main { dialogue }");
        let spec = DialogueContentSpec::try_new(
            line,
            TextKey::try_new("text.opening").expect("text key"),
            &template,
            test_character_plan(),
            arcweft_text_model::DialoguePresentationSnapshot::new(
                DialoguePresentationProfile::engine_default(),
                test_dialogue_profile_revision(),
            ),
            Vec::new(),
            source_map
                .primary_document()
                .expect("fixture source map retains its source")
                .product_source_ref(),
        )
        .expect("dialogue spec");
        let dialogue_content =
            DialogueContentCatalog::try_from_records_and_templates(vec![spec], vec![template])
                .expect("final dialogue content catalog");
        let product_awbc = AwbcLowerer::new(&plan, &dialogue_content, "dialogue-bundle.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        let bundle = ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("entry.main".to_owned()),
                adapter: None,
                locale: arcweft_manifest_model::ProjectLocaleSpec::default(),
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 2,
                    line_task_groups: 1,
                    stream_plans: 0,
                },
            },
            source_map,
            product_awbc,
            dialogue_content,
        )
        .expect("standard dialogue source joins source map");
        with_dialogue_generation(bundle)
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
