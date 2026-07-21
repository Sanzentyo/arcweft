use super::entry::select_runtime_entry;
use super::options::{
    CliRuntimeExecutorTier, CliRuntimeRunner, CliRuntimeStepMode, RuntimeRunOptions,
    ScriptBenchOptions,
};
use super::profile::report_path;
use super::script_bench::script_bench_selection;
use super::script_test::script_test_selection;
use super::serve::{RuntimeServeSelectionConfig, runtime_serve_selection};
use super::steps::{NativeRunSource, run_runtime_steps, runtime_step_run_config_from_run_options};
use crate::app::bundle::{
    build_patch_bundle_artifact_from_awfb_bytes, compile_bundle_for_selection,
    write_bundle_artifact, write_patch_bundle_artifact,
};
use crate::app::diagnostics::emit_diagnostics;
use crate::app::progress::{CliProgress, CliProgressStatus};
use crate::app::project::ProfileOptions;
use crate::app::project::{
    CheckedModule, SourceSelection, load_and_check_selection, native_host_policy_for_selection,
    resolve_source_selection_or_default_profile, runtime_pure_config_for_selection,
};
use crate::app::shared::print_json;
use crate::output::{RuntimeExecutorTier, RuntimeRunReport};
use arcweft_bundle::{
    ArcweftBundle, BundleFormat, BundleVirtualFileSpace,
    patch::{PatchCompatibility, encode_patch_bundle},
};
use arcweft_core::engine::FlowStatusLabelStyle;
use arcweft_core::plan::RuntimeEntryKind;
use arcweft_launch::{LaunchKind, LaunchPlayerViewportFit, resolve::ResolvedLaunchProfile};
#[cfg(feature = "native-player")]
use arcweft_layout::ScalePolicy;
use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
use arcweft_runtime_host::{NativeAdapterRegistrar, host_system_info};
use arcweft_verify::{
    BackendKind, VerificationMode, VerificationPolicy, VerificationReport, verify_module_with_env,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, SystemTime};

const RUN_BUNDLE_DIR: &str = "target/arcweft/run";
const WEB_LOCAL_BUNDLE_DIR: &str = "web/local";

pub(in crate::app) fn runtime_run_command(
    options: &RuntimeRunOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection_or_default_profile(
        options.path.as_ref(),
        &options.profile,
        LaunchKind::Game,
    )?;
    if options.watch && matches!(options.runner, CliRuntimeRunner::Headless) {
        eprintln!("error: `arcw run --watch` requires --runner auto, native, or web");
        return Err(ExitCode::from(2));
    }
    if options.watch && has_headless_debug_options_for_watch(options) {
        eprintln!(
            "error: `arcw run --watch` cannot be combined with headless runtime/debug options"
        );
        return Err(ExitCode::from(2));
    }
    if options.text_input_trace_out.is_some() && !matches!(options.runner, CliRuntimeRunner::Native)
    {
        eprintln!("error: --text-input-trace-out requires --runner native");
        return Err(ExitCode::from(2));
    }
    if has_session_save_options(options) && !matches!(options.runner, CliRuntimeRunner::Native) {
        eprintln!("error: --session-load and --session-save-out require --runner native");
        return Err(ExitCode::from(2));
    }
    if has_session_save_options(options) && options.watch {
        eprintln!("error: --session-load and --session-save-out cannot be combined with --watch");
        return Err(ExitCode::from(2));
    }
    if should_try_bundle_run(options, &selection) {
        let pure_config = runtime_pure_config_for_selection(
            &selection,
            options.pure_backend,
            options.pure_workers,
            options.pure_batch_min_len,
            options.pure_object_artifacts,
            options.math_backend,
            options.math_wgpu_min_elements,
        );
        match run_game_target(options, &selection, pure_config)? {
            RunTargetOutcome::Handled => return Ok(()),
            RunTargetOutcome::UseHeadless => {}
        }
    }
    runtime_run_headless_command(options, adapter_registrars, &selection)
}

fn runtime_run_headless_command(
    options: &RuntimeRunOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
    selection: &SourceSelection,
) -> Result<(), ExitCode> {
    let pure_config = runtime_pure_config_for_selection(
        selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    );
    if let Some(profile) = selection.profile() {
        match profile.kind() {
            LaunchKind::Server => {
                return runtime_serve_selection(
                    selection,
                    options.entry.as_deref(),
                    None,
                    RuntimeServeSelectionConfig {
                        listen: None,
                        once: false,
                        max_ops: options.max_ops,
                        pure_config,
                        json: options.json,
                    },
                    adapter_registrars,
                );
            }
            LaunchKind::Test => {
                return script_test_selection(
                    selection,
                    runtime_step_run_config_from_run_options(options, pure_config),
                    adapter_registrars,
                    &options.values,
                    options.json,
                );
            }
            LaunchKind::Bench => {
                return runtime_run_bench_selection(selection, options, adapter_registrars);
            }
            LaunchKind::Game | LaunchKind::Editor | LaunchKind::Agent | LaunchKind::Cli => {}
        }
    }

    let checked = load_and_check_selection(selection, None)?;
    require_runtime_verification_safety(&checked)?;
    let host_policy = native_host_policy_for_selection(selection)?;
    let plan = checked.runtime_plan().plan.clone();
    let entry = selection.command_entry(options.entry.as_deref())?;
    let entry = select_runtime_entry(&plan, entry)?;
    let file_roots = selection.native_file_roots();
    let trace = run_runtime_steps(
        plan,
        &entry,
        Some(NativeRunSource::new(selection.path(), &file_roots)),
        runtime_step_run_config_from_run_options(options, pure_config),
        &host_policy,
        adapter_registrars,
        &options.values,
    )?;
    let report = RuntimeRunReport {
        host_system: host_system_info(),
        executor: RuntimeExecutorTier::from(options.executor),
        executor_stats: trace.executor_stats,
        native_io: trace.native_io,
        steps: trace.steps,
        final_status: trace.final_status.status_label(FlowStatusLabelStyle::Debug),
    };
    print_runtime_run_report(selection, &report, options.json)
}

fn print_runtime_run_report(
    selection: &SourceSelection,
    report: &RuntimeRunReport,
    json: bool,
) -> Result<(), ExitCode> {
    if json {
        return print_json(report);
    }

    for step in &report.steps {
        println!(
            "step {}: {} flow event(s), {} effect(s), {} task request(s), {} diagnostic(s)",
            step.index,
            step.flow_events.len(),
            step.line_effects.len(),
            step.task_requests.len(),
            step.diagnostics.len()
        );
        for event in &step.flow_events {
            println!("  event {event}");
        }
        for effect in &step.line_effects {
            println!("  effect {effect}");
        }
    }
    println!(
        "ok: {} ({} step(s), final_status={})",
        selection.path().display(),
        report.steps.len(),
        report.final_status
    );
    Ok(())
}

fn require_runtime_verification_safety(checked: &CheckedModule) -> Result<(), ExitCode> {
    let verification = verify_module_with_env(
        &checked.hir,
        &checked.env,
        VerificationPolicy {
            mode: VerificationMode::Dev,
            backend: BackendKind::Emit,
            allow_trusted_proofs: true,
        },
    );
    if verification.has_blocking_runtime_safety_gaps() {
        emit_runtime_verification_diagnostics(&checked.source_document, &verification);
        return Err(ExitCode::FAILURE);
    }
    Ok(())
}

fn emit_runtime_verification_diagnostics(
    document: &arcweft_source::SourceDocument,
    report: &VerificationReport,
) {
    let diagnostics = report.source_diagnostics(document);
    emit_diagnostics(document, &diagnostics);
}

fn runtime_run_bench_selection(
    selection: &SourceSelection,
    options: &RuntimeRunOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let bench_options = ScriptBenchOptions {
        path: None,
        profile: ProfileOptions::default(),
        steps: options.steps,
        mode: options.mode,
        max_ops: options.max_ops,
        iterations: 1,
        warmup: 0,
        samples: 5,
        input_seed: 0,
        executor: options.executor,
        pure_backend: options.pure_backend,
        pure_workers: options.pure_workers,
        pure_batch_min_len: options.pure_batch_min_len,
        pure_object_artifacts: options.pure_object_artifacts,
        math_backend: options.math_backend,
        math_wgpu_min_elements: options.math_wgpu_min_elements,
        values: options.values.clone(),
        json: options.json,
    };
    script_bench_selection(selection, &bench_options, adapter_registrars)
}

enum RunTargetOutcome {
    Handled,
    UseHeadless,
}

fn should_try_bundle_run(options: &RuntimeRunOptions, selection: &SourceSelection) -> bool {
    match options.runner {
        CliRuntimeRunner::Native | CliRuntimeRunner::Web => true,
        CliRuntimeRunner::Headless => false,
        CliRuntimeRunner::Auto => {
            !has_headless_debug_options(options)
                && selection.profile().is_none_or(|profile| {
                    matches!(profile.kind(), LaunchKind::Game | LaunchKind::Cli)
                })
        }
    }
}

fn has_headless_debug_options(options: &RuntimeRunOptions) -> bool {
    options.json
        || options.executor != CliRuntimeExecutorTier::BytecodeVm
        || options.mode != CliRuntimeStepMode::OneOp
        || options.max_ops != 1
        || !options.values.is_empty()
        || options.pure_backend.is_some()
        || options.pure_workers.is_some()
        || options.pure_batch_min_len.is_some()
        || options.pure_object_artifacts
        || options.math_backend.is_some()
        || options.math_wgpu_min_elements.is_some()
}

fn has_session_save_options(options: &RuntimeRunOptions) -> bool {
    options.session_load.is_some() || options.session_save_out.is_some()
}

fn run_game_target(
    options: &RuntimeRunOptions,
    selection: &SourceSelection,
    pure_config: RuntimePureAcceleratorConfig,
) -> Result<RunTargetOutcome, ExitCode> {
    if matches!(
        options.runner,
        CliRuntimeRunner::Native | CliRuntimeRunner::Web
    ) && has_headless_debug_options(options)
    {
        eprintln!(
            "error: --runner {} cannot be combined with headless runtime/debug options; use --runner headless",
            options.runner.label()
        );
        return Err(ExitCode::from(2));
    }
    let mut phases = Vec::new();
    let progress = CliProgress::new(!options.json);
    let compiled = progress.run(
        CliProgressStatus::Compiling,
        format!("bundle {}", report_path(selection.path())),
        || {
            compile_bundle_for_selection(
                selection,
                vec![BundleVirtualFileSpace::Asset],
                &mut phases,
            )
        },
    )?;
    let is_game = source_selection_is_game(selection)
        || compiled
            .entry_kinds
            .iter()
            .any(|kind| matches!(kind, RuntimeEntryKind::Game));
    let runner = match options.runner {
        CliRuntimeRunner::Auto if is_game => CliRuntimeRunner::Native,
        CliRuntimeRunner::Auto => return Ok(RunTargetOutcome::UseHeadless),
        runner => runner,
    };
    if matches!(runner, CliRuntimeRunner::Native | CliRuntimeRunner::Web) && !is_game {
        eprintln!(
            "error: `arcw run --runner {}` requires a game launch profile or `entry game`",
            runner.label()
        );
        return Err(ExitCode::from(2));
    }
    if options.watch {
        run_watch_target(
            options,
            selection,
            runner,
            pure_config,
            compiled.bundle,
            phases,
        )?;
        return Ok(RunTargetOutcome::Handled);
    }
    let mut bundle = compiled.bundle;
    if let Some(entry) = options.entry.as_ref() {
        bundle.manifest.entry = Some(entry.clone());
    }
    match runner {
        CliRuntimeRunner::Native => {
            let output = run_bundle_output_path(selection, RUN_BUNDLE_DIR);
            write_run_bundle_with_progress(progress, &output, &bundle, &mut phases)?;
            progress.run(CliProgressStatus::Running, "native player", || {
                run_native_bundle(bundle, options.steps, options, selection)
            })?;
            Ok(RunTargetOutcome::Handled)
        }
        CliRuntimeRunner::Web => {
            let output = run_bundle_output_path(selection, WEB_LOCAL_BUNDLE_DIR);
            write_run_bundle_with_progress(progress, &output, &bundle, &mut phases)?;
            progress.emit_status(
                "Open",
                format_args!(
                    "web/index.html?bundle=./local/{}{} after building web/pkg.",
                    output
                        .file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .unwrap_or("game.awfb"),
                    web_player_frame_fit_query(selection)
                ),
            );
            Ok(RunTargetOutcome::Handled)
        }
        CliRuntimeRunner::Auto | CliRuntimeRunner::Headless => Ok(RunTargetOutcome::UseHeadless),
    }
}

fn write_run_bundle_with_progress(
    progress: CliProgress,
    output: &Path,
    bundle: &ArcweftBundle,
    phases: &mut Vec<crate::output::RuntimeProfilePhase>,
) -> Result<(), ExitCode> {
    let bytes = progress.run(CliProgressStatus::Encoding, "run bundle", || {
        bundle.to_format_bytes(BundleFormat::Awfb).map_err(|error| {
            eprintln!("error: failed to encode run bundle: {error}");
            ExitCode::FAILURE
        })
    })?;
    progress.run(CliProgressStatus::Writing, output.display(), || {
        write_bundle_artifact(output, bytes, phases)
    })
}

fn run_watch_target(
    options: &RuntimeRunOptions,
    selection: &SourceSelection,
    runner: CliRuntimeRunner,
    _pure_config: RuntimePureAcceleratorConfig,
    mut initial_bundle: ArcweftBundle,
    mut phases: Vec<crate::output::RuntimeProfilePhase>,
) -> Result<(), ExitCode> {
    if matches!(runner, CliRuntimeRunner::Native) {
        return run_native_windowed_watch_target(
            options.clone(),
            selection.clone(),
            initial_bundle,
            phases,
        );
    }

    let output_dir = match runner {
        CliRuntimeRunner::Web => WEB_LOCAL_BUNDLE_DIR,
        CliRuntimeRunner::Auto | CliRuntimeRunner::Headless => {
            unreachable!("watch runner resolved")
        }
        CliRuntimeRunner::Native => unreachable!("native watch returned above"),
    };
    let output = run_bundle_output_path(selection, output_dir);
    apply_watch_entry_override(options, &mut initial_bundle);
    let mut base_bytes = write_watch_bundle(&output, &initial_bundle, &mut phases)?;
    let mut inputs = watch_inputs(selection)?;
    println!(
        "watch: built {} ({} input(s), runner={})",
        output.display(),
        inputs.len(),
        runner.label()
    );
    if matches!(runner, CliRuntimeRunner::Web) {
        println!(
            "watch: open web/index.html?bundle=./local/{}{} after building web/pkg.",
            output
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("game.awfb"),
            web_player_frame_fit_query(selection)
        );
    }

    let max_iterations = (options.watch_iterations > 0).then_some(options.watch_iterations);
    let mut iterations = 0_usize;
    loop {
        if max_iterations.is_some_and(|max| iterations >= max) {
            return Ok(());
        }
        iterations += 1;
        thread::sleep(Duration::from_millis(options.watch_poll_ms));
        let next_inputs = watch_inputs(selection)?;
        if next_inputs == inputs {
            continue;
        }
        let mut next_phases = Vec::new();
        match compile_watch_bundle(options, selection, &mut next_phases) {
            Ok(next_bundle) => {
                let next_bytes =
                    next_bundle
                        .to_format_bytes(BundleFormat::Awfb)
                        .map_err(|error| {
                            eprintln!("error: failed to encode watched bundle: {error}");
                            ExitCode::FAILURE
                        })?;
                let artifact =
                    build_patch_bundle_artifact_from_awfb_bytes(&base_bytes, &next_bytes)?;
                let patch_bytes = encode_patch_bundle(&artifact).map_err(|error| {
                    eprintln!("error: failed to encode watched patch bundle: {error}");
                    ExitCode::FAILURE
                })?;
                let patch_output = watch_patch_output_path(selection, &artifact);
                write_patch_bundle_artifact(&patch_output, patch_bytes)?;
                write_bundle_artifact(&output, next_bytes.clone(), &mut next_phases)?;
                let transport_output = write_watch_patch_transport_envelope(
                    selection,
                    runner,
                    &output,
                    &patch_output,
                    &artifact,
                )?;
                println!(
                    "watch: patch {} ({} operation(s), compatibility={}, transport={}, action={})",
                    patch_output.display(),
                    artifact.plan.operations.len(),
                    artifact.manifest.compatibility.label(),
                    transport_output.display(),
                    watch_patch_transport_action(artifact.manifest.compatibility).label()
                );
                base_bytes = next_bytes;
                inputs = next_inputs;
            }
            Err(code) => {
                eprintln!("watch: rebuild failed; keeping previous bundle active");
                if max_iterations.is_some() {
                    return Err(code);
                }
            }
        }
    }
}

#[cfg(feature = "native-player")]
fn run_native_windowed_watch_target(
    options: RuntimeRunOptions,
    selection: SourceSelection,
    mut initial_bundle: ArcweftBundle,
    mut phases: Vec<crate::output::RuntimeProfilePhase>,
) -> Result<(), ExitCode> {
    let output = run_bundle_output_path(&selection, RUN_BUNDLE_DIR);
    apply_watch_entry_override(&options, &mut initial_bundle);
    let base_bytes = write_watch_bundle(&output, &initial_bundle, &mut phases)?;
    let inputs = watch_inputs(&selection)?;
    println!(
        "watch: built {} ({} input(s), runner=native, ingress=windowed-event-loop)",
        output.display(),
        inputs.len(),
    );
    let native_options = native_player_options(&options, &selection);
    run_native_bundle_with_ingress(
        initial_bundle,
        options.steps,
        native_options,
        move |ingress| {
            thread::spawn(move || {
                if let Err(code) = native_windowed_watch_producer_loop(
                    &options,
                    &selection,
                    &output,
                    base_bytes,
                    inputs,
                    ingress.patches(),
                ) {
                    eprintln!("watch: native windowed producer stopped with {code:?}");
                }
            });
        },
    )
}

#[cfg(not(feature = "native-player"))]
fn run_native_windowed_watch_target(
    _options: RuntimeRunOptions,
    _selection: SourceSelection,
    _initial_bundle: ArcweftBundle,
    _phases: Vec<crate::output::RuntimeProfilePhase>,
) -> Result<(), ExitCode> {
    eprintln!("error: native player support is not enabled for this arcw build");
    Err(ExitCode::from(2))
}

#[cfg(feature = "native-player")]
fn native_windowed_watch_producer_loop(
    options: &RuntimeRunOptions,
    selection: &SourceSelection,
    output: &Path,
    mut base_bytes: Vec<u8>,
    mut inputs: BTreeMap<PathBuf, WatchFileState>,
    ingress: &arcweft_player_native::WindowedPatchIngress,
) -> Result<(), ExitCode> {
    let max_iterations = (options.watch_iterations > 0).then_some(options.watch_iterations);
    let mut iterations = 0_usize;
    loop {
        if max_iterations.is_some_and(|max| iterations >= max) {
            return Ok(());
        }
        iterations = iterations.saturating_add(1);
        thread::sleep(Duration::from_millis(options.watch_poll_ms));
        let next_inputs = watch_inputs(selection)?;
        if next_inputs == inputs {
            continue;
        }
        let mut next_phases = Vec::new();
        match compile_watch_bundle(options, selection, &mut next_phases) {
            Ok(next_bundle) => {
                let next_bytes =
                    next_bundle
                        .to_format_bytes(BundleFormat::Awfb)
                        .map_err(|error| {
                            eprintln!("error: failed to encode watched bundle: {error}");
                            ExitCode::FAILURE
                        })?;
                let artifact =
                    build_patch_bundle_artifact_from_awfb_bytes(&base_bytes, &next_bytes)?;
                let patch_bytes = encode_patch_bundle(&artifact).map_err(|error| {
                    eprintln!("error: failed to encode watched patch bundle: {error}");
                    ExitCode::FAILURE
                })?;
                let patch_output = watch_patch_output_path(selection, &artifact);
                write_patch_bundle_artifact(&patch_output, patch_bytes.clone())?;
                write_bundle_artifact(output, next_bytes.clone(), &mut next_phases)?;
                let transport_output = write_watch_patch_transport_envelope(
                    selection,
                    CliRuntimeRunner::Native,
                    output,
                    &patch_output,
                    &artifact,
                )?;
                let accepted = ingress
                    .push_patch_bundle_bytes(
                        patch_bytes,
                        arcweft_player_native::windowed_patch::PatchEventSource::WatchChannel,
                    )
                    .map_err(|error| {
                        eprintln!("error: native windowed patch ingress rejected patch: {error}");
                        ExitCode::FAILURE
                    })?;
                println!(
                    "watch: patch {} ({} operation(s), compatibility={}, transport={}, action={}, ingress_sequence={})",
                    patch_output.display(),
                    artifact.plan.operations.len(),
                    artifact.manifest.compatibility.label(),
                    transport_output.display(),
                    watch_patch_transport_action(artifact.manifest.compatibility).label(),
                    accepted.sequence,
                );
                base_bytes = next_bytes;
                inputs = next_inputs;
            }
            Err(code) => {
                eprintln!("watch: rebuild failed; keeping previous bundle active");
                if max_iterations.is_some() {
                    return Err(code);
                }
            }
        }
    }
}

fn has_headless_debug_options_for_watch(options: &RuntimeRunOptions) -> bool {
    options.json
        || options.executor != CliRuntimeExecutorTier::BytecodeVm
        || options.mode != CliRuntimeStepMode::OneOp
        || options.max_ops != 1
        || !options.values.is_empty()
        || options.pure_backend.is_some()
        || options.pure_workers.is_some()
        || options.pure_batch_min_len.is_some()
        || options.pure_object_artifacts
        || options.math_backend.is_some()
        || options.math_wgpu_min_elements.is_some()
}

fn compile_watch_bundle(
    options: &RuntimeRunOptions,
    selection: &SourceSelection,
    phases: &mut Vec<crate::output::RuntimeProfilePhase>,
) -> Result<ArcweftBundle, ExitCode> {
    let mut bundle =
        compile_bundle_for_selection(selection, vec![BundleVirtualFileSpace::Asset], phases)?
            .bundle;
    apply_watch_entry_override(options, &mut bundle);
    Ok(bundle)
}

fn apply_watch_entry_override(options: &RuntimeRunOptions, bundle: &mut ArcweftBundle) {
    if let Some(entry) = options.entry.as_ref() {
        bundle.manifest.entry = Some(entry.clone());
    }
}

fn write_watch_bundle(
    output: &Path,
    bundle: &ArcweftBundle,
    phases: &mut Vec<crate::output::RuntimeProfilePhase>,
) -> Result<Vec<u8>, ExitCode> {
    let bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .map_err(|error| {
            eprintln!("error: failed to encode watched bundle: {error}");
            ExitCode::FAILURE
        })?;
    write_bundle_artifact(output, bytes.clone(), phases)?;
    Ok(bytes)
}

fn watch_patch_output_path(
    selection: &SourceSelection,
    artifact: &arcweft_bundle::patch::BundlePatchArtifact,
) -> PathBuf {
    Path::new(RUN_BUNDLE_DIR).join("patches").join(format!(
        "{}-{}-{}.awfb",
        run_bundle_stem(selection),
        artifact.manifest.base_content_root,
        artifact.manifest.target_content_root
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum WatchPatchTransportAction {
    ApplyPatch,
    RestartPlayer,
}

impl WatchPatchTransportAction {
    const fn label(self) -> &'static str {
        match self {
            Self::ApplyPatch => "apply-patch",
            Self::RestartPlayer => "restart-player",
        }
    }
}

#[derive(Debug, serde::Serialize)]
struct WatchPatchTransportEnvelope {
    schema_version: u32,
    runner: &'static str,
    source: String,
    target_bundle: String,
    patch_bundle: String,
    base_content_root: String,
    target_content_root: String,
    compatibility: &'static str,
    operation_count: usize,
    action: WatchPatchTransportAction,
}

fn watch_patch_transport_action(compatibility: PatchCompatibility) -> WatchPatchTransportAction {
    match compatibility {
        PatchCompatibility::ContentOnly | PatchCompatibility::CodeCompatible => {
            WatchPatchTransportAction::ApplyPatch
        }
        PatchCompatibility::CodeGenerational | PatchCompatibility::RestartRequired => {
            WatchPatchTransportAction::RestartPlayer
        }
    }
}

fn write_watch_patch_transport_envelope(
    selection: &SourceSelection,
    runner: CliRuntimeRunner,
    target_bundle: &Path,
    patch_bundle: &Path,
    artifact: &arcweft_bundle::patch::BundlePatchArtifact,
) -> Result<PathBuf, ExitCode> {
    let output = watch_patch_transport_output_path(patch_bundle);
    let envelope = WatchPatchTransportEnvelope {
        schema_version: 1,
        runner: runner.label(),
        source: selection.path().display().to_string(),
        target_bundle: target_bundle.display().to_string(),
        patch_bundle: patch_bundle.display().to_string(),
        base_content_root: artifact.manifest.base_content_root.to_string(),
        target_content_root: artifact.manifest.target_content_root.to_string(),
        compatibility: artifact.manifest.compatibility.label(),
        operation_count: artifact.plan.operations.len(),
        action: watch_patch_transport_action(artifact.manifest.compatibility),
    };
    let bytes = serde_json::to_vec_pretty(&envelope).map_err(|error| {
        eprintln!("error: failed to encode watch patch transport envelope: {error}");
        ExitCode::FAILURE
    })?;
    fs::write(&output, bytes).map_err(|error| {
        eprintln!(
            "error: failed to write watch patch transport envelope {}: {error}",
            output.display()
        );
        ExitCode::FAILURE
    })?;
    Ok(output)
}

fn watch_patch_transport_output_path(patch_bundle: &Path) -> PathBuf {
    let mut output = patch_bundle.to_path_buf();
    output.set_extension("transport.json");
    output
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct WatchFileState {
    len: u64,
    modified: Option<SystemTime>,
}

pub(in crate::app) fn watch_inputs(
    selection: &SourceSelection,
) -> Result<BTreeMap<PathBuf, WatchFileState>, ExitCode> {
    let mut paths = watch_input_paths(selection)?;
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .try_fold(BTreeMap::new(), |mut inputs, path| {
            inputs.insert(path.clone(), watch_file_state(&path)?);
            Ok(inputs)
        })
}

fn watch_input_paths(selection: &SourceSelection) -> Result<Vec<PathBuf>, ExitCode> {
    let mut paths = vec![selection.path().to_path_buf()];
    if let Some(manifest) = selection.project_manifest() {
        paths.push(manifest.to_path_buf());
        paths.extend(watch_project_source_paths(manifest)?);
    } else if let Some(topology) = selection.profile_topology() {
        paths.extend(
            topology
                .resources()
                .map(|resource| resource.path().to_path_buf()),
        );
    }
    paths.extend(watch_authored_resource_paths(selection)?);
    Ok(paths)
}

fn watch_project_source_paths(manifest: &Path) -> Result<Vec<PathBuf>, ExitCode> {
    let project = arcweft_project_loader::project::load(manifest).map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })?;
    Ok(project
        .sources()
        .modules()
        .map(|source| source.path().to_path_buf())
        .collect())
}

fn watch_authored_resource_paths(selection: &SourceSelection) -> Result<Vec<PathBuf>, ExitCode> {
    let roots = selection.authored_resource_roots();
    let mut paths = Vec::new();
    for dir in [roots.asset(), roots.content()] {
        if dir.exists() {
            collect_watch_input_files_from_dir(dir, &mut paths)?;
        }
    }
    Ok(paths)
}

fn collect_watch_input_files_from_dir(
    dir: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<(), ExitCode> {
    let mut entries = fs::read_dir(dir)
        .map_err(|error| {
            eprintln!(
                "error: failed to read watched input directory {}: {error}",
                dir.display()
            );
            ExitCode::FAILURE
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            eprintln!("error: failed to read watched input entry: {error}");
            ExitCode::FAILURE
        })?;
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        let file_type = entry.file_type().map_err(|error| {
            eprintln!(
                "error: failed to inspect watched input {}: {error}",
                entry.path().display()
            );
            ExitCode::FAILURE
        })?;
        if file_type.is_dir() {
            collect_watch_input_files_from_dir(&entry.path(), paths)?;
        } else if file_type.is_file() {
            paths.push(entry.path());
        }
    }
    Ok(())
}

fn watch_file_state(path: &Path) -> Result<WatchFileState, ExitCode> {
    let metadata = fs::metadata(path).map_err(|error| {
        eprintln!(
            "error: failed to stat watched input {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })?;
    Ok(WatchFileState {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn source_selection_is_game(selection: &SourceSelection) -> bool {
    selection
        .profile()
        .is_some_and(|profile| profile.kind() == LaunchKind::Game)
}

fn run_bundle_output_path(selection: &SourceSelection, directory: &str) -> PathBuf {
    Path::new(directory).join(format!("{}.awfb", run_bundle_stem(selection)))
}

fn run_bundle_stem(selection: &SourceSelection) -> String {
    selection
        .profile()
        .and_then(profile_bundle_stem)
        .or_else(|| source_path_bundle_stem(selection.path()))
        .map(sanitize_bundle_stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "arcweft-run".to_owned())
}

fn profile_bundle_stem(profile: &ResolvedLaunchProfile) -> Option<&str> {
    let id = profile.id().as_str();
    if id == "main"
        || id
            .rsplit_once('.')
            .is_some_and(|(_, segment)| segment == "main")
    {
        None
    } else {
        Some(id)
    }
}

fn source_path_bundle_stem(path: &Path) -> Option<&str> {
    let stem = path.file_stem().and_then(std::ffi::OsStr::to_str)?;
    if matches!(stem, "main" | "mod" | "lib") {
        path.parent()
            .and_then(Path::file_name)
            .and_then(std::ffi::OsStr::to_str)
            .or(Some(stem))
    } else {
        Some(stem)
    }
}

fn sanitize_bundle_stem(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(feature = "native-player")]
fn run_native_bundle(
    bundle: ArcweftBundle,
    steps: usize,
    options: &RuntimeRunOptions,
    selection: &SourceSelection,
) -> Result<(), ExitCode> {
    arcweft_player_native::run_bundle_windowed_with_options(
        bundle,
        steps,
        native_player_options(options, selection),
    )
    .map_err(|error| {
        eprintln!("error: native player failed: {error}");
        ExitCode::FAILURE
    })
}

#[cfg(feature = "native-player")]
fn run_native_bundle_with_ingress<F>(
    bundle: ArcweftBundle,
    steps: usize,
    options: arcweft_player_native::NativePlayerOptions,
    ingress_ready: F,
) -> Result<(), ExitCode>
where
    F: FnOnce(arcweft_player_native::WindowedPlayerIngress) + Send + 'static,
{
    arcweft_player_native::run_bundle_windowed_with_ingress_and_options(
        bundle,
        steps,
        options,
        ingress_ready,
    )
    .map_err(|error| {
        eprintln!("error: native player failed: {error}");
        ExitCode::FAILURE
    })
}

#[cfg(feature = "native-player")]
fn native_text_input_options(
    options: &RuntimeRunOptions,
) -> arcweft_player_native::NativeTextInputBridgeOptions {
    let mut bridge = arcweft_player_native::NativeTextInputBridgeOptions::default();
    if let Some(output) = options.text_input_trace_out.as_ref() {
        bridge = bridge.with_trace(
            arcweft_player_native::NativeTextInputTraceOptions::write_to(output.clone()),
        );
    }
    bridge
}

#[cfg(feature = "native-player")]
fn native_player_options(
    options: &RuntimeRunOptions,
    selection: &SourceSelection,
) -> arcweft_player_native::NativePlayerOptions {
    let mut native_options = arcweft_player_native::NativePlayerOptions::default()
        .with_text_input_options(native_text_input_options(options));
    if let Some(frame_fit) = frame_fit_for_selection(selection) {
        native_options = native_options.with_frame_fit(frame_fit);
    }
    if let Some(path) = options.session_load.as_ref() {
        native_options = native_options.with_session_load_path(path.clone());
    }
    if let Some(path) = options.session_save_out.as_ref() {
        native_options = native_options.with_session_save_out_path(path.clone());
    }
    native_options
}

#[cfg(feature = "native-player")]
fn frame_fit_for_selection(
    selection: &SourceSelection,
) -> Option<arcweft_player_scene::frame::PlayerFrameFit> {
    let viewport = selection.profile()?.player().viewport()?;
    let scale_policy = match viewport.fit() {
        LaunchPlayerViewportFit::Raw => ScalePolicy::Raw,
        LaunchPlayerViewportFit::Contain => ScalePolicy::Contain,
        LaunchPlayerViewportFit::Cover => ScalePolicy::Cover,
        LaunchPlayerViewportFit::Stretch => ScalePolicy::Stretch,
    };
    Some(if scale_policy == ScalePolicy::Raw {
        arcweft_player_scene::frame::PlayerFrameFit::raw()
    } else {
        arcweft_player_scene::frame::PlayerFrameFit::design(
            viewport
                .design_width()
                .expect("non-raw viewport has a validated design width")
                .get(),
            viewport
                .design_height()
                .expect("non-raw viewport has a validated design height")
                .get(),
            scale_policy,
        )
    })
}

#[cfg(not(feature = "native-player"))]
fn run_native_bundle(
    _bundle: ArcweftBundle,
    _steps: usize,
    _options: &RuntimeRunOptions,
    _selection: &SourceSelection,
) -> Result<(), ExitCode> {
    eprintln!("error: native player support is not enabled for this arcw build");
    Err(ExitCode::from(2))
}

fn web_player_frame_fit_query(selection: &SourceSelection) -> String {
    let Some(viewport) = selection
        .profile()
        .and_then(|profile| profile.player().viewport())
    else {
        return String::new();
    };
    let fit = match viewport.fit() {
        LaunchPlayerViewportFit::Raw => return "&fit=raw".to_owned(),
        LaunchPlayerViewportFit::Contain => "contain",
        LaunchPlayerViewportFit::Cover => "cover",
        LaunchPlayerViewportFit::Stretch => "stretch",
    };
    format!(
        "&fit={fit}&designWidth={}&designHeight={}",
        viewport
            .design_width()
            .expect("non-raw viewport has a validated design width")
            .get(),
        viewport
            .design_height()
            .expect("non-raw viewport has a validated design height")
            .get()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn watch_inputs_include_project_manifest_and_modules() {
        let root = temp_project_root("watch-inputs");
        let source_root = root.join("src");
        let nested = source_root.join("routes");
        fs::create_dir_all(&nested).expect("source dirs");
        fs::write(
            root.join("arcw.toml"),
            r#"
schema = 1

[package]
id = "org.arcweft.test.watch-test"
version = "0.1.0"
"#,
        )
        .expect("manifest writes");
        fs::write(
            source_root.join("main.arcw"),
            r#"flow @flow.main main { return "ok" }"#,
        )
        .expect("main source writes");
        fs::write(
            nested.join("opening.arcw"),
            r#"mod routes.opening
flow @flow.opening opening { return "ok" }
"#,
        )
        .expect("nested source writes");
        let manifest = root.join("arcw.toml");
        let selection = SourceSelection::Project {
            manifest: manifest.clone(),
            path: source_root.join("main.arcw"),
        };

        let inputs = watch_input_paths(&selection).expect("watch inputs resolve");

        assert!(inputs.contains(&manifest));
        assert!(inputs.contains(&source_root.join("main.arcw")));
        assert!(inputs.contains(&nested.join("opening.arcw")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn watch_inputs_include_authored_asset_and_content_files() {
        let root = temp_project_root("watch-virtual-inputs");
        let source_root = root.join("src");
        let asset_dir = root.join("assets").join("bg");
        let content_dir = root.join("content").join("chapter_two");
        fs::create_dir_all(&source_root).expect("source dir");
        fs::create_dir_all(&asset_dir).expect("asset dir");
        fs::create_dir_all(&content_dir).expect("content dir");
        fs::write(
            root.join("arcw.toml"),
            r#"
schema = 1

[package]
id = "org.arcweft.test.watch-virtual-test"
version = "0.1.0"
"#,
        )
        .expect("manifest writes");
        let source = source_root.join("main.arcw");
        fs::write(&source, r#"flow @flow.main main { return "ok" }"#).expect("main source writes");
        let asset = asset_dir.join("room.png");
        let content = content_dir.join("catalog.json");
        fs::write(&asset, [0_u8, 1, 2, 3]).expect("asset writes");
        fs::write(&content, br#"{"chapter":2}"#).expect("content writes");
        let selection = SourceSelection::Project {
            manifest: root.join("arcw.toml"),
            path: source,
        };

        let inputs = watch_input_paths(&selection).expect("watch inputs resolve");

        assert!(inputs.contains(&asset));
        assert!(inputs.contains(&content));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn watch_inputs_detect_authored_file_additions() {
        let root = temp_project_root("watch-virtual-additions");
        let source_root = root.join("src");
        fs::create_dir_all(&source_root).expect("source dir");
        fs::write(
            root.join("arcw.toml"),
            r#"
schema = 1

[package]
id = "org.arcweft.test.watch-virtual-addition-test"
version = "0.1.0"
"#,
        )
        .expect("manifest writes");
        let source = source_root.join("main.arcw");
        fs::write(&source, r#"flow @flow.main main { return "ok" }"#).expect("main source writes");
        let selection = SourceSelection::Project {
            manifest: root.join("arcw.toml"),
            path: source,
        };
        let before = watch_inputs(&selection).expect("initial watch inputs");
        let asset_dir = root.join("assets").join("view");
        fs::create_dir_all(&asset_dir).expect("asset dir");
        fs::write(asset_dir.join("logo.png"), [0_u8, 1, 2, 3]).expect("asset writes");

        let after = watch_inputs(&selection).expect("updated watch inputs");

        assert_ne!(before, after);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn watch_patch_transport_action_restarts_only_for_non_live_compatibility() {
        assert_eq!(
            watch_patch_transport_action(PatchCompatibility::ContentOnly),
            WatchPatchTransportAction::ApplyPatch
        );
        assert_eq!(
            watch_patch_transport_action(PatchCompatibility::CodeCompatible),
            WatchPatchTransportAction::ApplyPatch
        );
        assert_eq!(
            watch_patch_transport_action(PatchCompatibility::CodeGenerational),
            WatchPatchTransportAction::RestartPlayer
        );
        assert_eq!(
            watch_patch_transport_action(PatchCompatibility::RestartRequired),
            WatchPatchTransportAction::RestartPlayer
        );
    }

    #[test]
    fn watch_patch_transport_output_path_replaces_awfb_extension() {
        let output = watch_patch_transport_output_path(Path::new(
            "target/arcweft/run/patches/game-base-target.awfb",
        ));

        assert_eq!(
            output,
            PathBuf::from("target/arcweft/run/patches/game-base-target.transport.json")
        );
    }

    #[test]
    fn watch_patch_transport_envelope_writes_restart_fallback_action() {
        let root = temp_project_root("watch-transport-envelope");
        let patch_dir = root.join("patches");
        fs::create_dir_all(&patch_dir).expect("patch dir");
        let source = root.join("game.arcw");
        fs::write(&source, "flow main {}").expect("source writes");
        let selection = SourceSelection::Direct {
            path: source.clone(),
        };
        let patch_bundle = patch_dir.join("game-base-target.awfb");
        let target_bundle = root.join("game.awfb");
        let base_root = arcweft_bundle::container::BundleDigest::ZERO;
        let target_root = arcweft_bundle::container::BundleDigest::of(b"target");
        let section_id = arcweft_bundle::container::SectionId::from_bytes([7; 16]);
        let artifact = arcweft_bundle::patch::BundlePatchArtifact {
            manifest: arcweft_bundle::patch::BundlePatchManifest {
                schema_version: arcweft_bundle::patch::PATCH_PLAN_SCHEMA_VERSION,
                min_reader_schema_version: arcweft_bundle::patch::PATCH_PLAN_SCHEMA_VERSION,
                runtime_abi: arcweft_bundle::patch::RuntimeAbiRange::CURRENT,
                base_artifact: arcweft_bundle::container::ArtifactIdentity::for_current_container(
                    arcweft_bundle::container::BundleKind::Program,
                    base_root,
                    arcweft_bundle::container::BundleDigest::of(b"base-manifest"),
                ),
                target_artifact: arcweft_bundle::container::ArtifactIdentity::for_current_container(
                    arcweft_bundle::container::BundleKind::Program,
                    target_root,
                    arcweft_bundle::container::BundleDigest::of(b"target-manifest"),
                ),
                base_content_root: base_root,
                target_content_root: target_root,
                compatibility: PatchCompatibility::CodeGenerational,
                materialization: arcweft_bundle::patch::PatchMaterializationContract::default(),
                compatibility_fingerprints: vec![
                    arcweft_bundle::patch::SectionCompatibilityFingerprint {
                        id: section_id,
                        operation: arcweft_bundle::patch::SectionChangeOperation::Remove,
                        raw_kind_code: 0,
                        known_kind: None,
                        required: true,
                        compatibility: PatchCompatibility::CodeGenerational,
                        derivation:
                            arcweft_bundle::patch::SectionChangeDerivation::SectionKindDefault,
                        base_descriptor_fingerprint: None,
                        target_descriptor_fingerprint: None,
                        base_content_fingerprint: None,
                        target_content_fingerprint: None,
                    },
                ],
            },
            plan: arcweft_bundle::patch::BundlePatchPlan {
                base_content_root: base_root,
                target_content_root: target_root,
                operations: vec![arcweft_bundle::patch::SectionOperation::Remove {
                    id: section_id,
                    old: arcweft_bundle::container::BundleDigest::of(b"old-section"),
                }],
            },
            target_manifest_bytes: None,
            changed_sections: Vec::new(),
        };

        let output = write_watch_patch_transport_envelope(
            &selection,
            CliRuntimeRunner::Web,
            &target_bundle,
            &patch_bundle,
            &artifact,
        )
        .expect("transport envelope writes");
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(output).expect("transport envelope reads"))
                .expect("transport envelope parses");

        assert_eq!(json["action"], "restart_player");
        assert_eq!(json["compatibility"], "code-generational");
        assert_eq!(json["target_bundle"], target_bundle.display().to_string());
        assert_eq!(json["patch_bundle"], patch_bundle.display().to_string());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn watch_file_state_tracks_size_changes() {
        let root = temp_project_root("watch-state");
        fs::create_dir_all(&root).expect("temp dir");
        let path = root.join("input.arcw");
        fs::write(&path, "one").expect("initial input writes");
        let before = watch_file_state(&path).expect("initial state");

        fs::write(&path, "one two").expect("updated input writes");
        let after = watch_file_state(&path).expect("updated state");

        assert_ne!(before, after);

        let _ = fs::remove_dir_all(root);
    }

    fn temp_project_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after UNIX epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("arcweft-run-{label}-{}-{nanos}", process::id()))
    }
}
