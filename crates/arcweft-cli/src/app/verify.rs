use super::diagnostics::emit_diagnostics;
use super::project::{
    CheckedModule, ProfileOptions, SourceSelection, load_and_check_selection,
    native_host_policy_for_selection, resolve_source_selection, runtime_pure_config_for_selection,
};
use super::runtime::entry::select_runtime_entry;
use super::runtime::executor::RuntimeExecutorInstance;
use super::runtime::options::{
    CliRuntimeExecutorTier, CliRuntimeMathBackend, CliRuntimePureBackend, CliRuntimePureWorkers,
    CliRuntimeStepMode,
};
use super::runtime::parse::{parse_runtime_binding_arg, parse_runtime_pure_workers};
use super::runtime::profile::report_path;
use super::runtime::profile::run_profile_phase;
use super::runtime::steps::{NativeRunHost, NativeRunSource, run_runtime_steps_with_executor};
use super::shared::print_json;
use crate::output::{
    BorrowCheckProfileStats, RuntimeExecutorTier, RuntimeTypeValidationProfileStats,
    RuntimeTypeValidationReportSummary, TypeCheckProfileStats, VerifyTypesReport,
    VerifyTypesRuntimeSelfCheck, VerifyTypesVerifierSummary,
};
use arcweft_core::{
    engine::{FlowFiberStatus, FlowStatusLabelStyle},
    plan::{EntryRuntimeId, RuntimePlan},
    value::RuntimeBinding,
};
use arcweft_runtime_host::NativeAdapterRegistrar;
use arcweft_verify::{
    BackendKind, VerificationMode, VerificationPolicy, VerificationReport,
    smt::{SmtBackend, SmtEmission},
    validate_runtime_plan_types, verify_module_with_env,
};
use arcweft_verify_oxiz::OxizBackend;
use arcweft_verify_z3::ExternalZ3Backend;
use clap::Args;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs};

const Z3_COMMAND_ENV: &str = "ARCWEFT_Z3_COMMAND";
const Z3_BIN_ENV: &str = "ARCWEFT_Z3_BIN";

pub(super) fn verify_command(options: &VerifyOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let checked = load_and_check_selection(&selection, None)?;
    let mut report = verify_module_with_env(
        &checked.hir,
        &checked.env,
        VerificationPolicy {
            mode: options.mode,
            backend: options.backend,
            allow_trusted_proofs: options.mode != VerificationMode::Release,
        },
    );

    if let Some(path) = options.emit_obligations.as_ref() {
        write_json(path, &report.obligations)?;
    }
    if let Some(path) = options.emit_smt.as_ref() {
        emit_smt(path, &report)?;
    }
    if matches!(options.backend, BackendKind::Oxiz | BackendKind::Z3) {
        let z3_command = resolve_z3_command(options);
        solve_report(&mut report, options.backend, z3_command.as_deref());
    }
    if options.json {
        print_json(&report)?;
    } else {
        emit_verifier_diagnostics(&checked.source_document, &report);
        let status = if report.has_errors() || report.has_solver_failures() {
            "failed"
        } else {
            "ok"
        };
        println!(
            "{status}: {} ({} obligation(s), {} unsafe audit(s))",
            selection.path().display(),
            report.obligations.len(),
            report.unsafe_audit_count()
        );
    }
    if report.has_errors() || report.has_solver_failures() {
        Err(ExitCode::FAILURE)
    } else {
        Ok(())
    }
}

pub(super) fn verify_types_command(
    options: &VerifyTypesOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    if options.run && options.steps == 0 {
        eprintln!("error: --steps must be greater than zero when --run is used");
        return Err(ExitCode::from(2));
    }
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let mut checked = load_and_check_selection(&selection, None)?;
    let (runtime_plan, entry) = verify_types_runtime_plan(&mut checked, &selection, options)?;
    let runtime_type_validation =
        verify_types_runtime_type_validation(&mut checked, &runtime_plan)?;
    let verification = verify_types_semantics(&mut checked, options.mode)?;
    let runtime = verify_types_runtime_self_check(
        runtime_plan,
        &entry,
        &selection,
        options,
        &mut checked,
        adapter_registrars,
    )?;
    let runtime_failed = runtime
        .as_ref()
        .is_some_and(|runtime| runtime.failed || runtime.diagnostics > 0);
    let verification_failed = verification.has_blocking_runtime_safety_gaps();
    let status = if runtime_type_validation.has_errors() || verification_failed || runtime_failed {
        "failed"
    } else {
        "ok"
    };
    let report = VerifyTypesReport {
        status: status.to_owned(),
        source: report_path(selection.path()),
        syntax_warnings: checked.syntax_warnings,
        line_task_groups: checked.line_task_groups.len(),
        phases: checked.phases.clone(),
        typecheck: TypeCheckProfileStats::from(&checked.typecheck_report),
        borrow_check: BorrowCheckProfileStats::from(&checked.typecheck_report.stats),
        runtime_type_validation: RuntimeTypeValidationReportSummary {
            diagnostics: runtime_type_validation.diagnostics.len(),
            errors: runtime_type_validation
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == arcweft_verify::Severity::Error)
                .count(),
            stats: RuntimeTypeValidationProfileStats::from(&runtime_type_validation.stats),
        },
        verifier: VerifyTypesVerifierSummary {
            diagnostics: verification.diagnostics.len(),
            obligations: verification.obligations.len(),
            unsafe_audits: verification.unsafe_audit_count(),
        },
        runtime,
    };
    if options.json {
        print_json(&report)?;
    } else {
        println!(
            "{}: {} (type_judgments={}, runtime_type_errors={}, obligations={})",
            report.status,
            report.source,
            report.typecheck.judgments,
            report.runtime_type_validation.errors,
            report.verifier.obligations
        );
    }
    if status == "ok" {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

fn verify_types_runtime_plan(
    checked: &mut CheckedModule,
    selection: &SourceSelection,
    options: &VerifyTypesOptions,
) -> Result<(RuntimePlan, EntryRuntimeId), ExitCode> {
    let runtime_plan = checked.runtime_plan().plan.clone();
    let entry = selection.command_entry(options.entry.as_deref())?;
    let entry = select_runtime_entry(&runtime_plan, entry)?;
    Ok((runtime_plan, entry))
}

fn verify_types_runtime_type_validation(
    checked: &mut CheckedModule,
    runtime_plan: &RuntimePlan,
) -> Result<arcweft_verify::RuntimeTypeValidationReport, ExitCode> {
    run_profile_phase(&mut checked.phases, "runtime_type_validate", || {
        Ok(validate_runtime_plan_types(
            runtime_plan,
            &checked.typecheck_report,
        ))
    })
}

fn verify_types_semantics(
    checked: &mut CheckedModule,
    mode: VerificationMode,
) -> Result<arcweft_verify::VerificationReport, ExitCode> {
    run_profile_phase(&mut checked.phases, "verify", || {
        Ok(verify_module_with_env(
            &checked.hir,
            &checked.env,
            VerificationPolicy {
                mode,
                backend: BackendKind::Emit,
                allow_trusted_proofs: mode != VerificationMode::Release,
            },
        ))
    })
}

fn verify_types_runtime_self_check(
    runtime_plan: RuntimePlan,
    entry: &EntryRuntimeId,
    selection: &SourceSelection,
    options: &VerifyTypesOptions,
    checked: &mut CheckedModule,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<Option<VerifyTypesRuntimeSelfCheck>, ExitCode> {
    if !options.run {
        return Ok(None);
    }
    let pure_config = runtime_pure_config_for_selection(
        selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    );
    let mut executor = run_profile_phase(&mut checked.phases, "executor_prepare", || {
        RuntimeExecutorInstance::new(runtime_plan, entry, options.executor, pure_config).map_err(
            |error| {
                eprintln!(
                    "error: failed to start entry `{}`: {error}",
                    entry.public_label()
                );
                ExitCode::FAILURE
            },
        )
    })?;
    let host_policy = native_host_policy_for_selection(selection)?;
    let file_roots = selection.native_file_roots();
    let trace = run_profile_phase(&mut checked.phases, "run", || {
        run_runtime_steps_with_executor(
            &mut executor,
            NativeRunHost {
                source: Some(NativeRunSource::new(selection.path(), &file_roots)),
                policy: &host_policy,
                adapter_registrars,
            },
            options.steps,
            options.runtime_mode,
            options.max_ops,
            &options.values,
        )
    })?;
    Ok(Some(VerifyTypesRuntimeSelfCheck {
        executor: RuntimeExecutorTier::from(options.executor),
        executor_stats: trace.executor_stats,
        native_io: trace.native_io,
        steps_run: trace.steps.len(),
        final_status: trace.final_status.status_label(FlowStatusLabelStyle::Debug),
        diagnostics: trace.steps.iter().map(|step| step.diagnostics.len()).sum(),
        failed: matches!(trace.final_status, FlowFiberStatus::Failed(_)),
        steps: trace.steps,
    }))
}

pub(super) fn unsafe_command(options: &UnsafeOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let checked = load_and_check_selection(&selection, None)?;
    let report = verify_module_with_env(
        &checked.hir,
        &checked.env,
        VerificationPolicy {
            mode: options.mode,
            backend: BackendKind::Emit,
            allow_trusted_proofs: options.mode != VerificationMode::Release,
        },
    );
    if options.json {
        print_json(&report.unsafe_audits)?;
    } else {
        for audit in &report.unsafe_audits {
            println!(
                "{} reason={} safety_doc={}",
                audit.id, audit.has_reason, audit.has_safety_doc
            );
        }
    }
    Ok(())
}

fn parse_verification_mode(value: &str) -> Result<VerificationMode, String> {
    match value {
        "dev" => Ok(VerificationMode::Dev),
        "test" => Ok(VerificationMode::Test),
        "release" => Ok(VerificationMode::Release),
        other => Err(format!("unknown verification mode `{other}`")),
    }
}

fn parse_backend_kind(value: &str) -> Result<BackendKind, String> {
    match value {
        "emit" => Ok(BackendKind::Emit),
        "oxiz" => Ok(BackendKind::Oxiz),
        "z3" => Ok(BackendKind::Z3),
        other => Err(format!("unknown verifier backend `{other}`")),
    }
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct VerifyOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long, value_parser = parse_verification_mode, default_value = "test")]
    mode: VerificationMode,
    #[arg(long, alias = "solver", value_parser = parse_backend_kind, default_value = "emit")]
    backend: BackendKind,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    emit_obligations: Option<PathBuf>,
    #[arg(long)]
    emit_smt: Option<PathBuf>,
    #[arg(long, alias = "solver-command")]
    z3_command: Option<String>,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct VerifyTypesOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long)]
    entry: Option<String>,
    #[arg(long, value_parser = parse_verification_mode, default_value = "test")]
    mode: VerificationMode,
    #[arg(long)]
    run: bool,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    runtime_mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 1)]
    steps: usize,
    #[arg(long, default_value_t = 64)]
    max_ops: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    math_wgpu_min_elements: Option<usize>,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct UnsafeOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long, value_parser = parse_verification_mode, default_value = "dev")]
    mode: VerificationMode,
    #[arg(long)]
    json: bool,
}

fn emit_verifier_diagnostics(
    document: &arcweft_source::SourceDocument,
    report: &VerificationReport,
) {
    let diagnostics = report.source_diagnostics(document);
    emit_diagnostics(document, &diagnostics);
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), ExitCode> {
    let json = serde_json::to_string_pretty(value).map_err(|error| {
        eprintln!("error: failed to encode JSON: {error}");
        ExitCode::FAILURE
    })?;
    fs::write(path, json).map_err(|error| {
        eprintln!("error: failed to write {}: {error}", path.display());
        ExitCode::FAILURE
    })
}

fn emit_smt(path: &Path, report: &VerificationReport) -> Result<(), ExitCode> {
    fs::create_dir_all(path).map_err(|error| {
        eprintln!("error: failed to create {}: {error}", path.display());
        ExitCode::FAILURE
    })?;
    for obligation in &report.obligations {
        let Some(problem) = &obligation.smt else {
            continue;
        };
        let file = path.join(format!("{}.smt2", obligation.id));
        let script = problem
            .emit_smt_lib(SmtEmission::CounterexampleValues)
            .map_err(|error| {
                eprintln!("error: failed to emit {}: {error}", file.display());
                ExitCode::FAILURE
            })?;
        fs::write(&file, script).map_err(|error| {
            eprintln!("error: failed to write {}: {error}", file.display());
            ExitCode::FAILURE
        })?;
    }
    Ok(())
}

fn resolve_z3_command(options: &VerifyOptions) -> Option<OsString> {
    options
        .z3_command
        .as_ref()
        .map(OsString::from)
        .or_else(|| non_empty_env_os(Z3_COMMAND_ENV))
        .or_else(|| non_empty_env_os(Z3_BIN_ENV).map(z3_command_from_bin))
}

fn non_empty_env_os(name: &str) -> Option<OsString> {
    env::var_os(name).filter(|value| !value.is_empty())
}

fn z3_command_from_bin(value: OsString) -> OsString {
    let path = PathBuf::from(&value);
    if path.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        name.eq_ignore_ascii_case("z3") || name.eq_ignore_ascii_case("z3.exe")
    }) {
        value
    } else {
        path.join(if cfg!(windows) { "z3.exe" } else { "z3" })
            .into_os_string()
    }
}

fn solve_report(report: &mut VerificationReport, backend: BackendKind, z3_command: Option<&OsStr>) {
    let checks = report
        .obligations
        .iter()
        .filter_map(|obligation| {
            obligation
                .smt
                .clone()
                .map(|problem| (obligation.id.clone(), problem))
        })
        .collect::<Vec<_>>();
    for (obligation, problem) in checks {
        let outcome = match backend {
            BackendKind::Emit => continue,
            BackendKind::Oxiz => OxizBackend.check(&problem),
            BackendKind::Z3 => {
                let backend = z3_command.map_or_else(ExternalZ3Backend::default, |command| {
                    ExternalZ3Backend::new(command.to_os_string())
                });
                backend.check(&problem)
            }
        };
        match &outcome {
            Ok(check) if check.model.is_empty() => {
                eprintln!("solver[{backend:?}] {obligation}: {:?}", check.outcome);
            }
            Ok(check) => eprintln!(
                "solver[{backend:?}] {obligation}: {:?} {:?}",
                check.outcome, check.model
            ),
            Err(error) => eprintln!("solver[{backend:?}] {obligation}: {error}"),
        }
        report.record_solver_check(&obligation, backend, outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::z3_command_from_bin;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn z3_bin_env_points_to_platform_executable() {
        let command = PathBuf::from(z3_command_from_bin(OsString::from("D:/tools/z3/bin")));
        assert!(command.ends_with(if cfg!(windows) { "z3.exe" } else { "z3" }));
    }

    #[test]
    fn z3_bin_env_accepts_direct_executable_path() {
        let command = z3_command_from_bin(OsString::from("D:/tools/z3/bin/z3.exe"));
        assert_eq!(
            PathBuf::from(command),
            PathBuf::from("D:/tools/z3/bin/z3.exe")
        );
    }
}
