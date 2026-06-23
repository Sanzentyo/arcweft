//! Cargo-like project commands and the rustc-like single-source compiler route.

use super::project::{
    ProfileOptions, SourceSelection, load_and_check_selection, print_project_compile_error,
    resolve_source_selection, runtime_plan_options_for_selection, typecheck_env_for_selection,
};
use super::shared::print_json;
use arcweft_compiler::{
    lower::lower_source_runtime_plan_with_options,
    project::{CompiledProject, compile_project_with_env},
};
use arcweft_project_loader::project::{LoadedProject, ProjectLoadError};
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_verify::{
    BackendKind, VerificationMode, VerificationPolicy, VerificationReport, verify_module_with_env,
};
use clap::{Args, ValueEnum};
use serde::Serialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

/// Project-wide `arcw check` options.
#[derive(Args, Clone, Debug)]
pub(in crate::app) struct ProjectCheckOptions {
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long)]
    json: bool,
}

/// Project-wide `arcw build` options.
#[derive(Args, Clone, Debug)]
pub(in crate::app) struct ProjectBuildOptions {
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long)]
    release: bool,
    #[arg(long)]
    target_dir: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

/// Direct, rustc-like single-source compilation options.
#[derive(Args, Clone, Debug)]
pub(in crate::app) struct CompileOptions {
    input: PathBuf,
    #[arg(long, value_enum, default_value = "check")]
    emit: CompileEmit,
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

/// Direct compiler output selected by `arcw compile --emit`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(in crate::app) enum CompileEmit {
    Check,
    Hir,
    Plan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectBuildMode {
    Dev,
    Release,
}

struct ProjectCommandState {
    loaded: LoadedProject,
    selection: SourceSelection,
    compiled: CompiledProject,
    verification: VerificationReport,
}

#[derive(Serialize)]
struct ProjectCommandReport {
    status: &'static str,
    package: String,
    manifest: String,
    selected_profile: Option<String>,
    selected_source: String,
    modules: Vec<ProjectModuleReport>,
    compile_units: Vec<ProjectCompileUnitReport>,
    syntax_warnings: usize,
    flows: usize,
    line_task_groups: usize,
    verifier_diagnostics: usize,
    obligations: usize,
    unsafe_audits: usize,
}

#[derive(Serialize)]
struct ProjectModuleReport {
    module: String,
    source: String,
    source_hash: String,
}

#[derive(Serialize)]
struct ProjectCompileUnitReport {
    id: usize,
    modules: Vec<String>,
    fingerprint: String,
    cache: &'static str,
}

#[derive(Serialize)]
struct CompileReport {
    status: &'static str,
    input: String,
    emit: &'static str,
    output: Option<String>,
    syntax_warnings: usize,
    flows: usize,
    line_task_groups: usize,
    verifier_diagnostics: usize,
    obligations: usize,
}

impl CompileEmit {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Hir => "hir",
            Self::Plan => "plan",
        }
    }

    fn output_path(
        self,
        input: &Path,
        explicit: Option<&Path>,
    ) -> Result<Option<PathBuf>, &'static str> {
        match (self, explicit) {
            (Self::Check, None) => Ok(None),
            (Self::Check, Some(_)) => Err("--output cannot be used with --emit check"),
            (Self::Hir | Self::Plan, Some(path)) => Ok(Some(path.to_path_buf())),
            (Self::Hir, None) => Ok(Some(input.with_extension("hir"))),
            (Self::Plan, None) => Ok(Some(input.with_extension("plan"))),
        }
    }
}

impl ProjectBuildMode {
    const fn from_release(release: bool) -> Self {
        if release { Self::Release } else { Self::Dev }
    }

    const fn directory(self) -> &'static str {
        match self {
            Self::Dev => "debug",
            Self::Release => "release",
        }
    }

    const fn verification_mode(self) -> VerificationMode {
        match self {
            Self::Dev => VerificationMode::Dev,
            Self::Release => VerificationMode::Release,
        }
    }
}

impl ProjectCommandReport {
    fn from_state(state: &ProjectCommandState) -> Self {
        let failed = state.verification.has_errors();
        let sources = state.loaded.sources();
        Self {
            status: if failed { "failed" } else { "ok" },
            package: sources.manifest().package().name().as_str().to_owned(),
            manifest: sources.manifest_path().display().to_string(),
            selected_profile: state
                .selection
                .profile()
                .map(|profile| profile.id().as_str().to_owned()),
            selected_source: state.selection.path().display().to_string(),
            modules: sources
                .modules()
                .map(|source| ProjectModuleReport {
                    module: source.module().to_string(),
                    source: source.path().display().to_string(),
                    source_hash: source.source_hash().to_hex(),
                })
                .collect(),
            compile_units: state
                .compiled
                .compile_units()
                .iter()
                .map(|unit| ProjectCompileUnitReport {
                    id: unit.id().index(),
                    modules: unit.modules().iter().map(ToString::to_string).collect(),
                    fingerprint: unit.fingerprint().to_hex(),
                    cache: unit.cache_status().as_str(),
                })
                .collect(),
            syntax_warnings: state.compiled.syntax_warnings(),
            flows: state.compiled.linked_hir().flows().len(),
            line_task_groups: state.compiled.line_task_groups().len(),
            verifier_diagnostics: state.verification.diagnostics.len(),
            obligations: state.verification.obligations.len(),
            unsafe_audits: state.verification.unsafe_audit_count(),
        }
    }
}

pub(super) fn project_check_command(options: &ProjectCheckOptions) -> Result<(), ExitCode> {
    let state = compile_project_command(&options.profile, VerificationMode::Dev)?;
    let report = ProjectCommandReport::from_state(&state);
    if options.json {
        print_json(&report)?;
    } else {
        print_verification_diagnostics(&state.verification);
        println!(
            "{}: {} ({} module(s), {} compile unit(s), {} warning(s), {} obligation(s))",
            report.status,
            report.package,
            report.modules.len(),
            report.compile_units.len(),
            report.syntax_warnings,
            report.obligations,
        );
    }
    status_result(report.status)
}

pub(super) fn project_build_command(options: &ProjectBuildOptions) -> Result<(), ExitCode> {
    let mode = ProjectBuildMode::from_release(options.release);
    let state = compile_project_command(&options.profile, mode.verification_mode())?;
    let report = ProjectCommandReport::from_state(&state);
    if report.status != "ok" {
        print_verification_diagnostics(&state.verification);
        if options.json {
            print_json(&report)?;
        }
        return Err(ExitCode::FAILURE);
    }

    let target_root = options
        .target_dir
        .clone()
        .unwrap_or_else(|| state.loaded.sources().target_root())
        .join(mode.directory());
    fs::create_dir_all(&target_root).map_err(|error| {
        eprintln!(
            "error: failed to create build directory {}: {error}",
            target_root.display()
        );
        ExitCode::FAILURE
    })?;
    let package = state.loaded.sources().manifest().package().name().as_str();
    let metadata_path = target_root.join(format!("{package}.project.json"));
    let plan_path = target_root.join(format!("{package}.plan"));
    write_json_file(&metadata_path, &report)?;
    fs::write(
        &plan_path,
        format!("{:#?}\n", state.compiled.runtime_plan().plan),
    )
    .map_err(|error| {
        eprintln!("error: failed to write {}: {error}", plan_path.display());
        ExitCode::FAILURE
    })?;

    if options.json {
        print_json(&serde_json::json!({
            "report": report,
            "artifacts": [
                metadata_path.display().to_string(),
                plan_path.display().to_string(),
            ],
        }))?;
    } else {
        println!(
            "Finished `{}` profile: {} module(s), {} compile unit(s)",
            mode.directory(),
            report.modules.len(),
            report.compile_units.len(),
        );
        println!("  {}", metadata_path.display());
        println!("  {}", plan_path.display());
    }
    Ok(())
}

pub(super) fn compile_command(options: &CompileOptions) -> Result<(), ExitCode> {
    let selection = SourceSelection::Direct {
        path: options.input.clone(),
    };
    let checked = load_and_check_selection(&selection, None)?;
    let verification = verify_module_with_env(
        &checked.hir,
        &checked.env,
        VerificationPolicy {
            mode: VerificationMode::Dev,
            backend: BackendKind::Emit,
        },
    );
    if verification.has_errors() {
        print_verification_diagnostics(&verification);
        return Err(ExitCode::FAILURE);
    }

    let output = options
        .emit
        .output_path(&options.input, options.output.as_deref())
        .map_err(|message| {
            eprintln!("error: {message}");
            ExitCode::from(2)
        })?;
    match options.emit {
        CompileEmit::Check => {}
        CompileEmit::Hir => write_text_artifact(
            output.as_deref().expect("HIR emit has a default path"),
            &format!("{:#?}\n", checked.hir),
        )?,
        CompileEmit::Plan => {
            let plan = lower_source_runtime_plan_with_options(
                &checked.hir,
                &RuntimePlanLowerOptions::default(),
            )
            .map_err(|errors| {
                for error in errors {
                    eprintln!("error: {}", error.message());
                }
                ExitCode::FAILURE
            })?;
            write_text_artifact(
                output.as_deref().expect("plan emit has a default path"),
                &format!("{plan:#?}\n"),
            )?;
        }
    }

    let report = CompileReport {
        status: "ok",
        input: options.input.display().to_string(),
        emit: options.emit.as_str(),
        output: output.as_ref().map(|path| path.display().to_string()),
        syntax_warnings: checked.syntax_warnings,
        flows: checked.hir.flows().len(),
        line_task_groups: checked.line_task_groups.len(),
        verifier_diagnostics: verification.diagnostics.len(),
        obligations: verification.obligations.len(),
    };
    if options.json {
        print_json(&report)?;
    } else {
        println!(
            "ok: {} (emit={}, {} flow(s), {} warning(s), {} obligation(s))",
            options.input.display(),
            report.emit,
            report.flows,
            report.syntax_warnings,
            report.obligations,
        );
        if let Some(output) = output {
            println!("  {}", output.display());
        }
    }
    Ok(())
}

fn compile_project_command(
    profile: &ProfileOptions,
    verification_mode: VerificationMode,
) -> Result<ProjectCommandState, ExitCode> {
    let (loaded, resolved_profile) = load_project(profile)?;
    let selection = if resolved_profile.profile.is_some() {
        resolve_source_selection(None, &resolved_profile)?
    } else {
        SourceSelection::Project {
            manifest: loaded.sources().manifest_path().to_path_buf(),
            path: loaded.sources().root_module().path().to_path_buf(),
        }
    };
    let mut phases = Vec::new();
    let env = typecheck_env_for_selection(&selection, None, &mut phases)?;
    let runtime_options = runtime_plan_options_for_selection(&selection);
    let compiled =
        compile_project_with_env(loaded.sources(), &env, &runtime_options).map_err(|error| {
            print_project_compile_error(&error);
            ExitCode::FAILURE
        })?;
    let verification = verify_module_with_env(
        compiled.linked_hir(),
        &env,
        VerificationPolicy {
            mode: verification_mode,
            backend: BackendKind::Emit,
        },
    );
    Ok(ProjectCommandState {
        loaded,
        selection,
        compiled,
        verification,
    })
}

fn load_project(profile: &ProfileOptions) -> Result<(LoadedProject, ProfileOptions), ExitCode> {
    let explicit = profile.manifest.as_path();
    let loaded = if explicit.is_file() {
        arcweft_project_loader::project::load(explicit)
    } else if explicit == Path::new("arcw.toml") {
        let current = env::current_dir().map_err(|error| {
            eprintln!("error: failed to resolve current directory: {error}");
            ExitCode::FAILURE
        })?;
        arcweft_project_loader::project::load_discovered(&current)
    } else {
        arcweft_project_loader::project::load(explicit)
    }
    .map_err(|error| print_project_load_error(&error))?;
    let resolved = ProfileOptions {
        profile: profile.profile.clone(),
        manifest: loaded.sources().manifest_path().to_path_buf(),
    };
    Ok((loaded, resolved))
}

fn print_project_load_error(error: &ProjectLoadError) -> ExitCode {
    eprintln!("error: {error}");
    ExitCode::FAILURE
}

fn print_verification_diagnostics(report: &VerificationReport) {
    for diagnostic in &report.diagnostics {
        eprintln!("{:?}: {}", diagnostic.severity, diagnostic.message);
    }
}

fn status_result(status: &str) -> Result<(), ExitCode> {
    if status == "ok" {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), ExitCode> {
    let json = serde_json::to_vec_pretty(value).map_err(|error| {
        eprintln!("error: failed to encode {}: {error}", path.display());
        ExitCode::FAILURE
    })?;
    fs::write(path, json).map_err(|error| {
        eprintln!("error: failed to write {}: {error}", path.display());
        ExitCode::FAILURE
    })
}

fn write_text_artifact(path: &Path, contents: &str) -> Result<(), ExitCode> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            eprintln!("error: failed to create {}: {error}", parent.display());
            ExitCode::FAILURE
        })?;
    }
    fs::write(path, contents).map_err(|error| {
        eprintln!("error: failed to write {}: {error}", path.display());
        ExitCode::FAILURE
    })
}

#[cfg(test)]
mod tests {
    use super::CompileEmit;
    use std::path::Path;

    #[test]
    fn compile_emit_owns_output_path_policy() {
        assert_eq!(
            CompileEmit::Hir
                .output_path(Path::new("src/main.arcw"), None)
                .expect("HIR output resolves")
                .as_deref(),
            Some(Path::new("src/main.hir"))
        );
        assert_eq!(
            CompileEmit::Plan
                .output_path(
                    Path::new("src/main.arcw"),
                    Some(Path::new("target/main.plan")),
                )
                .expect("explicit plan output resolves")
                .as_deref(),
            Some(Path::new("target/main.plan"))
        );
        assert!(
            CompileEmit::Check
                .output_path(Path::new("src/main.arcw"), Some(Path::new("unused")))
                .is_err()
        );
    }
}
