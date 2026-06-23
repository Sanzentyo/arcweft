use super::entry::apply_runtime_entry_selection;
use super::options::{
    CliRuntimeExecutorTier, CliRuntimeRunner, CliRuntimeStepMode, RuntimeRunOptions,
    ScriptBenchOptions,
};
use super::script_bench::script_bench_selection;
use super::script_test::script_test_selection;
use super::serve::{RuntimeServeSelectionConfig, runtime_serve_selection};
use super::steps::{run_runtime_steps, runtime_step_run_config_from_run_options};
use crate::app::bundle::{compile_bundle_for_selection, write_bundle_artifact};
use crate::app::project::ProfileOptions;
use crate::app::project::{
    SourceSelection, load_and_check_selection, native_host_policy_for_selection,
    resolve_source_selection_or_default_profile, runtime_plan_options_for_selection,
    runtime_pure_config_for_selection,
};
use crate::app::shared::print_json;
use crate::output::{RuntimeExecutorTier, RuntimeRunReport};
use arcweft_bundle::{ArcweftBundle, BundleFormat, BundleVirtualFileSpace};
use arcweft_compiler::lower::lower_source_runtime_plan_with_options;
use arcweft_core::engine::FlowStatusLabelStyle;
use arcweft_core::plan::RuntimeEntryKind;
use arcweft_launch::{LaunchKind, ResolvedLaunchProfile};
use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
use arcweft_runtime_host::{NativeAdapterRegistrar, host_system_info};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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
    if should_try_bundle_run(options, &selection) {
        let pure_config = runtime_pure_config_for_selection(
            &selection,
            options.pure_backend,
            options.pure_workers,
            options.pure_batch_min_len,
            options.pure_object_artifacts,
            options.math_backend,
            options.math_wgpu_min_elements,
        )?;
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
    )?;
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
            LaunchKind::Game | LaunchKind::Cli => {}
        }
    }

    let checked = load_and_check_selection(selection, None)?;
    let host_policy = native_host_policy_for_selection(selection)?;
    let runtime_options = runtime_plan_options_for_selection(selection);
    let mut plan = lower_source_runtime_plan_with_options(&checked.hir, &runtime_options).map_err(
        |errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        },
    )?;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut plan, entry, options.flow.as_deref())?;
    let trace = run_runtime_steps(
        plan,
        Some(selection.path()),
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
    if options.json {
        print_json(&report)
    } else {
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
        || options.flow.is_some()
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

fn run_game_target(
    options: &RuntimeRunOptions,
    selection: &SourceSelection,
    _pure_config: RuntimePureAcceleratorConfig,
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
    let compiled =
        compile_bundle_for_selection(selection, vec![BundleVirtualFileSpace::Asset], &mut phases)?;
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
    let mut bundle = compiled.bundle;
    if let Some(entry) = options.entry.as_ref() {
        bundle.manifest.entry = Some(entry.clone());
    }
    match runner {
        CliRuntimeRunner::Native => {
            let output = run_bundle_output_path(selection, RUN_BUNDLE_DIR);
            write_run_bundle(&output, &bundle, &mut phases)?;
            println!("Built {}", output.display());
            run_native_bundle(bundle, options.steps)?;
            Ok(RunTargetOutcome::Handled)
        }
        CliRuntimeRunner::Web => {
            let output = run_bundle_output_path(selection, WEB_LOCAL_BUNDLE_DIR);
            write_run_bundle(&output, &bundle, &mut phases)?;
            println!("Built {}", output.display());
            println!(
                "Open web/index.html?bundle=./local/{} after building web/pkg.",
                output
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .unwrap_or("game.awfb")
            );
            Ok(RunTargetOutcome::Handled)
        }
        CliRuntimeRunner::Auto | CliRuntimeRunner::Headless => Ok(RunTargetOutcome::UseHeadless),
    }
}

fn write_run_bundle(
    output: &Path,
    bundle: &ArcweftBundle,
    phases: &mut Vec<crate::output::RuntimeProfilePhase>,
) -> Result<(), ExitCode> {
    let bytes = bundle
        .to_format_bytes(BundleFormat::Json)
        .map_err(|error| {
            eprintln!("error: failed to encode run bundle: {error}");
            ExitCode::FAILURE
        })?;
    write_bundle_artifact(output, bytes, phases)
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
        source_path_bundle_stem(profile.source())
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
fn run_native_bundle(bundle: ArcweftBundle, steps: usize) -> Result<(), ExitCode> {
    arcweft_player_native::run_bundle_windowed(bundle, steps).map_err(|error| {
        eprintln!("error: native player failed: {error}");
        ExitCode::FAILURE
    })
}

#[cfg(not(feature = "native-player"))]
fn run_native_bundle(_bundle: ArcweftBundle, _steps: usize) -> Result<(), ExitCode> {
    eprintln!("error: native player support is not enabled for this arcw build");
    Err(ExitCode::from(2))
}
