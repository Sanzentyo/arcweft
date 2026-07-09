use super::diagnostics::{DiagnosticEmitter, DiagnosticSource};
use super::runtime::options::{
    CliRuntimeMathBackend, CliRuntimePureBackend, CliRuntimePureWorkers,
};
use super::runtime::parse::parse_runtime_pure_workers;
use super::runtime::profile::run_profile_phase;
use super::shared::is_arcw_path;
use crate::output::RuntimeProfilePhase;
use arcweft_adapter_context::{manifest::AdapterManifest, standard};
use arcweft_character::catalog::CharacterCatalog;
use arcweft_compiler::{
    hir, parse,
    project::{ProjectCompileDiagnostic, ProjectCompileError, compile_project_with_env},
};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_lang_sema::{check::TypeCheckReport, env::TypeCheckEnv};
use arcweft_lang_syntax::{lint::SyntaxLint, source::ParsedSource};
use arcweft_launch::{
    LaunchKind, LaunchMathBackend, LaunchProfileManifest, LaunchPureBackend, ResolvedLaunchProfile,
};
use arcweft_project::manifest::AuthoredResourceRoots;
use arcweft_runtime_accelerator::{
    RuntimePureAcceleratorConfig, RuntimePureBackendMode, RuntimePureWorkerCount,
    math::RuntimeMathBackend,
};
use arcweft_runtime_host::{NativeFileRoots, NativeTaskBridge};
use arcweft_runtime_plan::{flow::RuntimePlanLowerOptions, line_task::LoweredLineTaskGroup};
use arcweft_rust_abi::ArcweftRustManifest;
use arcweft_source::{Diagnostic, DiagnosticSeverity, SourceName};
use clap::Args;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Args, Clone, Debug, Default)]
pub(in crate::app) struct ProfileOptions {
    #[arg(long)]
    pub(in crate::app) profile: Option<String>,
    #[arg(
        long = "manifest-path",
        alias = "manifest",
        default_value = "arcw.toml"
    )]
    pub(in crate::app) manifest: PathBuf,
}

#[derive(Clone, Debug)]
pub(in crate::app) enum SourceSelection {
    Direct {
        path: PathBuf,
    },
    Project {
        manifest: PathBuf,
        path: PathBuf,
    },
    Profile {
        manifest: PathBuf,
        profile: Box<ResolvedLaunchProfile>,
    },
}

impl SourceSelection {
    pub(in crate::app) fn path(&self) -> &Path {
        match self {
            Self::Direct { path } | Self::Project { path, .. } => path,
            Self::Profile { profile, .. } => profile.source(),
        }
    }

    pub(in crate::app) fn manifest(&self) -> Option<&Path> {
        match self {
            Self::Project { manifest, .. } => Some(manifest),
            Self::Direct { .. } | Self::Profile { .. } => None,
        }
    }

    pub(in crate::app) fn resource_manifest(&self) -> Option<&Path> {
        match self {
            Self::Project { manifest, .. } | Self::Profile { manifest, .. } => Some(manifest),
            Self::Direct { .. } => None,
        }
    }

    pub(in crate::app) fn profile(&self) -> Option<&ResolvedLaunchProfile> {
        match self {
            Self::Direct { .. } | Self::Project { .. } => None,
            Self::Profile { profile, .. } => Some(profile),
        }
    }

    pub(in crate::app) fn authored_resource_roots(
        &self,
    ) -> Result<AuthoredResourceRoots, ExitCode> {
        if let Some(manifest) = self.resource_manifest() {
            return arcweft_project_loader::project::load_authored_resource_roots(manifest)
                .map_err(|error| {
                    eprintln!("error: {error}");
                    ExitCode::FAILURE
                });
        }

        let source_dir = self.path().parent().unwrap_or_else(|| Path::new("."));
        Ok(AuthoredResourceRoots::new(
            source_dir.join("assets"),
            source_dir.join("content"),
        ))
    }

    pub(in crate::app) fn local_state_root(&self) -> PathBuf {
        self.resource_manifest()
            .and_then(Path::parent)
            .or_else(|| self.path().parent())
            .unwrap_or_else(|| Path::new("."))
            .join(".arcweft")
    }

    pub(in crate::app) fn native_file_roots(&self) -> Result<NativeFileRoots, ExitCode> {
        let authored = self.authored_resource_roots()?;
        Ok(NativeFileRoots::new(
            authored.asset(),
            self.local_state_root(),
        ))
    }

    pub(in crate::app) fn entry(&self) -> Option<&str> {
        self.profile().and_then(ResolvedLaunchProfile::entry)
    }

    pub(in crate::app) fn adapter(&self) -> Option<&str> {
        self.profile().and_then(ResolvedLaunchProfile::adapter)
    }
}

pub(in crate::app) fn runtime_plan_options_for_selection(
    selection: &SourceSelection,
) -> RuntimePlanLowerOptions {
    selection
        .profile()
        .and_then(ResolvedLaunchProfile::dialogue_defaults)
        .map_or_else(RuntimePlanLowerOptions::default, |id| {
            RuntimePlanLowerOptions::default().with_dialogue_defaults(id)
        })
}

pub(in crate::app) fn runtime_pure_config_for_selection(
    selection: &SourceSelection,
    backend: Option<CliRuntimePureBackend>,
    workers: Option<CliRuntimePureWorkers>,
    batch_min_len: Option<usize>,
    object_artifacts: bool,
    math_backend: Option<CliRuntimeMathBackend>,
    math_wgpu_min_elements: Option<usize>,
) -> Result<RuntimePureAcceleratorConfig, ExitCode> {
    let mut config = RuntimePureAcceleratorConfig::default();
    if let Some(profile) = selection.profile().and_then(ResolvedLaunchProfile::pure) {
        if let Some(backend) = profile.backend() {
            config.backend = launch_pure_backend_mode(backend);
        }
        if let Some(backend) = profile.math_backend() {
            config.math.backend = launch_math_backend_mode(backend);
        }
        if let Some(min_elements) = profile.math_wgpu_min_elements() {
            config.math.wgpu_min_elements = min_elements;
        }
        if let Some(workers) = profile.workers() {
            config.workers = parse_runtime_pure_workers(workers)
                .map(RuntimePureWorkerCount::from)
                .map_err(|message| {
                    eprintln!("error: invalid launch profile pure.workers: {message}");
                    ExitCode::from(2)
                })?;
        }
        if let Some(batch_min_len) = profile.batch_min_len() {
            config.batch_min_len = batch_min_len;
        }
        if let Some(object_artifacts) = profile.object_artifacts() {
            config.emit_object_artifacts = object_artifacts;
        }
    }
    if let Some(backend) = backend {
        config.backend = backend.into();
    }
    if let Some(workers) = workers {
        config.workers = workers.into();
    }
    if let Some(batch_min_len) = batch_min_len {
        config.batch_min_len = batch_min_len;
    }
    if object_artifacts {
        config.emit_object_artifacts = true;
    }
    if let Some(backend) = math_backend {
        config.math.backend = backend.into();
    }
    if let Some(min_elements) = math_wgpu_min_elements {
        config.math.wgpu_min_elements = min_elements;
    }
    Ok(config)
}

fn launch_pure_backend_mode(value: LaunchPureBackend) -> RuntimePureBackendMode {
    match value {
        LaunchPureBackend::Auto => RuntimePureBackendMode::Auto,
        LaunchPureBackend::Vm => RuntimePureBackendMode::Vm,
        LaunchPureBackend::Aot => RuntimePureBackendMode::Aot,
        LaunchPureBackend::Jit => RuntimePureBackendMode::Jit,
    }
}

fn launch_math_backend_mode(value: LaunchMathBackend) -> RuntimeMathBackend {
    match value {
        LaunchMathBackend::Auto => RuntimeMathBackend::Auto,
        LaunchMathBackend::Scalar => RuntimeMathBackend::Scalar,
        LaunchMathBackend::Glam => RuntimeMathBackend::Glam,
        LaunchMathBackend::Ndarray => RuntimeMathBackend::Ndarray,
        LaunchMathBackend::Wgpu => RuntimeMathBackend::Wgpu,
    }
}

pub(in crate::app) fn resolve_source_selection(
    path: Option<&PathBuf>,
    profile: &ProfileOptions,
) -> Result<SourceSelection, ExitCode> {
    match (path, profile.profile.as_deref()) {
        (Some(_), Some(_)) => {
            eprintln!("error: source path and --profile cannot be used together");
            Err(ExitCode::from(2))
        }
        (Some(path), None) => Ok(SourceSelection::Direct { path: path.clone() }),
        (None, Some(profile_id)) => resolve_profile_source_selection(profile, profile_id),
        (None, None) => {
            eprintln!("error: expected .arcw source path or --profile");
            Err(ExitCode::from(2))
        }
    }
}

pub(in crate::app) fn resolve_source_selection_or_default_profile(
    path: Option<&PathBuf>,
    profile: &ProfileOptions,
    preferred_kind: LaunchKind,
) -> Result<SourceSelection, ExitCode> {
    match (path, profile.profile.as_deref()) {
        (Some(_), Some(_)) => {
            eprintln!("error: source path and --profile cannot be used together");
            Err(ExitCode::from(2))
        }
        (Some(path), None) => Ok(SourceSelection::Direct { path: path.clone() }),
        (None, Some(profile_id)) => resolve_profile_source_selection(profile, profile_id),
        (None, None) => {
            let manifest_path = resolve_manifest_path(&profile.manifest)?;
            let manifest = read_launch_manifest(&manifest_path)?;
            match default_profile_id(&manifest, &manifest_path, preferred_kind)? {
                Some(profile_id) => {
                    resolve_profile_source_selection_from_manifest(&manifest_path, &profile_id)
                }
                None => resolve_project_root_source_selection(&manifest_path),
            }
        }
    }
}

fn resolve_profile_source_selection(
    profile: &ProfileOptions,
    profile_id: &str,
) -> Result<SourceSelection, ExitCode> {
    let manifest_path = resolve_manifest_path(&profile.manifest)?;
    resolve_profile_source_selection_from_manifest(&manifest_path, profile_id)
}

fn resolve_profile_source_selection_from_manifest(
    manifest_path: &Path,
    profile_id: &str,
) -> Result<SourceSelection, ExitCode> {
    let manifest = read_launch_manifest(manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let adapter_registry = standard::standard_registry();
    let adapter_ids = adapter_registry.adapter_ids();
    let resolved = manifest
        .resolve_profile_with_adapters(profile_id, manifest_dir, &adapter_ids)
        .map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        })?;
    Ok(SourceSelection::Profile {
        manifest: manifest_path.to_path_buf(),
        profile: Box::new(resolved),
    })
}

fn resolve_project_root_source_selection(
    manifest_path: &Path,
) -> Result<SourceSelection, ExitCode> {
    let project = arcweft_project_loader::project::load(manifest_path).map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })?;
    Ok(SourceSelection::Project {
        manifest: project.sources().manifest_path().to_path_buf(),
        path: project.sources().root_module().path().to_path_buf(),
    })
}

fn resolve_manifest_path(path: &Path) -> Result<PathBuf, ExitCode> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    if path == Path::new(arcweft_project_loader::project::PROJECT_MANIFEST_FILE) {
        let current = std::env::current_dir().map_err(|error| {
            eprintln!("error: failed to resolve current directory: {error}");
            ExitCode::FAILURE
        })?;
        return arcweft_project_loader::project::discover_manifest(&current).map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        });
    }
    Ok(path.to_path_buf())
}

fn read_launch_manifest(path: &Path) -> Result<LaunchProfileManifest, ExitCode> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!(
            "error: failed to read launch manifest {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })?;
    LaunchProfileManifest::parse_toml(&source).map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })
}

fn default_profile_id(
    manifest: &LaunchProfileManifest,
    manifest_path: &Path,
    preferred_kind: LaunchKind,
) -> Result<Option<String>, ExitCode> {
    if let Some(default_profile) = manifest.default_profile() {
        return Ok(Some(default_profile.to_owned()));
    }
    let profiles = manifest.profiles();
    let matching_kind = manifest.profile_ids_with_kind(preferred_kind);
    match (matching_kind.as_slice(), profiles.len()) {
        ([profile_id], _) => Ok(Some(profile_id.clone())),
        ([], 1) => Ok(Some(
            profiles
                .keys()
                .next()
                .expect("profile map has one entry")
                .clone(),
        )),
        ([], 0) => Ok(None),
        ([], _) => {
            eprintln!(
                "error: expected .arcw source path, --profile, or a default profile in {}",
                manifest_path.display()
            );
            Err(ExitCode::from(2))
        }
        _ => {
            eprintln!(
                "error: multiple {} launch profiles found; set `default = \"...\"` in arcw.toml or pass --profile",
                preferred_kind.as_str()
            );
            for profile_id in matching_kind {
                eprintln!("  {profile_id}");
            }
            Err(ExitCode::from(2))
        }
    }
}

pub(in crate::app) fn require_profile_kind(
    selection: &SourceSelection,
    expected: LaunchKind,
    command: &str,
) -> Result<(), ExitCode> {
    let Some(profile) = selection.profile() else {
        return Ok(());
    };
    if profile.kind() == expected {
        return Ok(());
    }
    eprintln!(
        "error: launch profile `{}` has kind {}; use an `{command}` profile for `arcw {command}`",
        profile.id().as_str(),
        profile.kind().as_str()
    );
    Err(ExitCode::from(2))
}

pub(in crate::app) fn load_and_check_selection(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
) -> Result<CheckedModule, ExitCode> {
    let mut phases = Vec::new();
    let env = typecheck_env_for_selection(selection, adapter_override, &mut phases)?;
    if let Some(manifest) = selection.manifest() {
        return load_and_check_project_with_env(manifest, &env, phases);
    }
    load_and_check_with_env(selection.path(), &env, phases)
}

fn load_and_check_project_with_env(
    manifest: &Path,
    env: &TypeCheckEnv,
    mut phases: Vec<RuntimeProfilePhase>,
) -> Result<CheckedModule, ExitCode> {
    let loaded = run_profile_phase(&mut phases, "load_project", || {
        arcweft_project_loader::project::load(manifest).map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        })
    })?;
    let runtime_options = RuntimePlanLowerOptions::default();
    let compiled = run_profile_phase(&mut phases, "project_compile", || {
        compile_project_with_env(loaded.sources(), env, &runtime_options).map_err(|error| {
            print_project_compile_error(&error);
            ExitCode::FAILURE
        })
    })?;
    Ok(CheckedModule {
        hir: compiled.linked_hir().clone(),
        env: env.clone(),
        syntax_warnings: compiled.syntax_warnings(),
        line_task_groups: compiled.line_task_groups().to_vec(),
        typecheck_report: compiled.typecheck_report().clone(),
        phases,
    })
}

pub(in crate::app) fn print_project_compile_error(error: &ProjectCompileError) {
    let emitter = DiagnosticEmitter::stderr();
    for diagnostic in error.diagnostics() {
        emit_project_compile_diagnostic(&emitter, diagnostic);
    }
}

fn emit_project_compile_diagnostic(
    emitter: &DiagnosticEmitter,
    diagnostic: &ProjectCompileDiagnostic,
) {
    if let Some(source) = diagnostic.source()
        && let Some(text) = source.text()
    {
        let diagnostic_source =
            DiagnosticSource::from_display_path(source.name().display_name().to_owned(), text);
        emitter.emit(diagnostic.diagnostic(), &diagnostic_source);
        return;
    }
    emitter.emit_without_source(diagnostic.diagnostic());
}

pub(in crate::app) fn typecheck_env_for_selection(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<TypeCheckEnv, ExitCode> {
    let mut manifest = adapter_manifest_for_selection(selection, adapter_override)?;
    if adapter_override.is_none() && selection.profile().is_some() {
        let manifests = run_profile_phase(phases, "rust_metadata", || {
            rust_metadata_for_selection(selection)
        })?;
        for rust_manifest in manifests {
            manifest = manifest.with_rust_manifest(&rust_manifest);
        }
    }
    let characters = if selection.profile().is_some() {
        run_profile_phase(phases, "character_manifests", || {
            character_catalog_for_selection(selection)
        })?
    } else {
        CharacterCatalog::new()
    };
    let env = if adapter_override.is_some() || selection.profile().is_some() {
        manifest.apply_to_target_env(TypeCheckEnv::standard())
    } else {
        manifest.apply_to_env(TypeCheckEnv::standard())
    };
    let env = characters.manifests().fold(
        env,
        arcweft_lang_sema::env::TypeCheckEnv::with_character_manifest,
    );
    Ok(arcweft_adapter_desktop::standard_desktop_manifests()
        .into_iter()
        .fold(env, |env, manifest| manifest.apply_to_env(env)))
}

pub(in crate::app) fn adapter_manifest_for_selection(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
) -> Result<AdapterManifest, ExitCode> {
    let adapter_id = adapter_override
        .or(selection.adapter())
        .unwrap_or(standard::SANS_IO_ADAPTER_ID);
    let registry = adapter_registry_for_selection(selection)?;
    let Some(manifest) = registry.get(adapter_id) else {
        eprintln!("error: unknown adapter `{adapter_id}`");
        return Err(ExitCode::from(2));
    };
    Ok(manifest.clone())
}

pub(in crate::app) fn native_host_policy_for_selection(
    selection: &SourceSelection,
) -> Result<HostCallPolicy, ExitCode> {
    native_host_policy_for_selection_with_adapter(selection, None)
}

pub(in crate::app) fn native_host_policy_for_selection_with_adapter(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
) -> Result<HostCallPolicy, ExitCode> {
    let selected = adapter_manifest_for_selection(selection, adapter_override)?;
    let desktop_policy =
        HostCallPolicy::from_manifests(arcweft_adapter_desktop::standard_desktop_manifests());
    Ok(NativeTaskBridge::standard_cli_policy_for_manifest(&selected).union(desktop_policy))
}

fn adapter_registry_for_selection(
    selection: &SourceSelection,
) -> Result<arcweft_adapter_context::manifest::AdapterRegistry, ExitCode> {
    let registry = standard::standard_registry();
    let Some(profile) = selection.profile() else {
        return Ok(registry);
    };
    profile
        .adapter_manifests()
        .iter()
        .try_fold(registry, |registry, path| {
            arcweft_project_loader::adapter_manifest::load(path)
                .map(|manifest| registry.with_manifest(manifest))
                .map_err(|error| {
                    eprintln!(
                        "error: failed to load adapter manifest {}: {error}",
                        path.display()
                    );
                    ExitCode::FAILURE
                })
        })
}

fn character_catalog_for_selection(
    selection: &SourceSelection,
) -> Result<CharacterCatalog, ExitCode> {
    let Some(profile) = selection.profile() else {
        return Ok(CharacterCatalog::new());
    };
    let mut catalog = CharacterCatalog::new();
    for path in profile.character_manifests() {
        let manifest = arcweft_project_loader::character_manifest::load(path).map_err(|error| {
            eprintln!(
                "error: failed to load character manifest {}: {error}",
                path.display()
            );
            ExitCode::FAILURE
        })?;
        catalog.insert(manifest).map_err(|error| {
            eprintln!(
                "error: failed to register character manifest {}: {error}",
                path.display()
            );
            ExitCode::FAILURE
        })?;
    }
    Ok(catalog)
}

fn rust_metadata_for_selection(
    selection: &SourceSelection,
) -> Result<Vec<ArcweftRustManifest>, ExitCode> {
    let Some(profile) = selection.profile() else {
        return Ok(Vec::new());
    };
    profile
        .rust_metadata()
        .iter()
        .map(|path| {
            arcweft_project_loader::rust_metadata::load(path).map_err(|error| {
                eprintln!(
                    "error: failed to load Rust ABI metadata {}: {error}",
                    path.display()
                );
                ExitCode::FAILURE
            })
        })
        .collect()
}

pub(crate) struct CheckedModule {
    pub(crate) hir: arcweft_lang_hir::model::HirModule,
    pub(crate) env: TypeCheckEnv,
    pub(crate) syntax_warnings: usize,
    pub(crate) line_task_groups: Vec<LoweredLineTaskGroup>,
    pub(crate) typecheck_report: TypeCheckReport,
    pub(crate) phases: Vec<RuntimeProfilePhase>,
}

fn emit_phase_error_diagnostics(
    emitter: &DiagnosticEmitter,
    diagnostic_source: &DiagnosticSource<'_>,
    code: &'static str,
    messages: impl IntoIterator<Item = String>,
) {
    let diagnostics = messages
        .into_iter()
        .map(|message| Diagnostic::new(DiagnosticSeverity::Error, message).with_code(code))
        .collect::<Vec<_>>();
    emitter.emit_all(&diagnostics, diagnostic_source);
}

fn emit_parse_error_diagnostics(
    parsed: &ParsedSource,
    source_name: &SourceName,
    emitter: &DiagnosticEmitter,
    diagnostic_source: &DiagnosticSource<'_>,
) {
    let diagnostics = parsed
        .errors()
        .iter()
        .map(|error| error.diagnostic(source_name))
        .collect::<Vec<_>>();
    emitter.emit_all(&diagnostics, diagnostic_source);
}

fn emit_syntax_lint_diagnostics(
    lints: &[SyntaxLint],
    source_name: &SourceName,
    emitter: &DiagnosticEmitter,
    diagnostic_source: &DiagnosticSource<'_>,
) {
    for lint in lints {
        emitter.emit(&lint.diagnostic(source_name), diagnostic_source);
    }
}

pub(in crate::app) fn load_and_check_with_env(
    path: &Path,
    env: &TypeCheckEnv,
    mut phases: Vec<RuntimeProfilePhase>,
) -> Result<CheckedModule, ExitCode> {
    if !is_arcw_path(path) {
        eprintln!("error: {} is not an .arcw source file", path.display());
        return Err(ExitCode::from(2));
    }
    let source = run_profile_phase(&mut phases, "read_source", || {
        fs::read_to_string(path).map_err(|error| {
            eprintln!("error: failed to read {}: {error}", path.display());
            ExitCode::FAILURE
        })
    })?;

    let parsed = run_profile_phase(&mut phases, "parse", || {
        catch_unwind(AssertUnwindSafe(|| parse::parse_source_text(source))).map_err(|_| {
            eprintln!("error: parser panicked while checking {}", path.display());
            ExitCode::FAILURE
        })
    })?;
    let source_text = parsed.source().to_owned();
    let source_name = SourceName::path(path.display().to_string());
    let diagnostic_source = DiagnosticSource::new(path, &source_text);
    let emitter = DiagnosticEmitter::stderr();
    if !parsed.errors().is_empty() {
        emit_parse_error_diagnostics(&parsed, &source_name, &emitter, &diagnostic_source);
        return Err(ExitCode::FAILURE);
    }

    let tree = parsed.into_typed_tree();
    let lints = run_profile_phase(&mut phases, "lint", || {
        Ok::<Vec<SyntaxLint>, ExitCode>(parse::lint_source_tree(&tree))
    })?;
    emit_syntax_lint_diagnostics(&lints, &source_name, &emitter, &diagnostic_source);
    if parse::has_error_lints(&lints) {
        return Err(ExitCode::FAILURE);
    }

    let hir = run_profile_phase(&mut phases, "lower_hir", || {
        hir::lower_source_tree(&tree).map_err(|errors| {
            emit_phase_error_diagnostics(
                &emitter,
                &diagnostic_source,
                "hir.lower",
                errors.into_iter().map(|error| error.message().to_owned()),
            );
            ExitCode::FAILURE
        })
    })?;

    run_profile_phase(&mut phases, "resolve", || {
        hir::resolve_hir_references_with_env(&hir, env).map_err(|errors| {
            emit_phase_error_diagnostics(
                &emitter,
                &diagnostic_source,
                "sema.resolve",
                errors.into_iter().map(|error| error.to_string()),
            );
            ExitCode::FAILURE
        })
    })?;
    run_profile_phase(&mut phases, "readiness", || {
        hir::validate_hir_typecheck_ready(&hir).map_err(|errors| {
            emit_phase_error_diagnostics(
                &emitter,
                &diagnostic_source,
                "sema.readiness",
                errors.into_iter().map(|error| error.message().to_owned()),
            );
            ExitCode::FAILURE
        })
    })?;
    let typecheck_report = run_profile_phase(&mut phases, "typecheck", || {
        hir::typecheck_hir_with_env(&hir, env).map_err(|errors| {
            emit_phase_error_diagnostics(
                &emitter,
                &diagnostic_source,
                "sema.typecheck",
                errors.into_iter().map(|error| error.message().to_owned()),
            );
            ExitCode::FAILURE
        })
    })?;

    let line_task_groups = run_profile_phase(&mut phases, "line_task_lower", || {
        arcweft_compiler::lower::lower_source_line_tasks(&hir).map_err(|errors| {
            emit_phase_error_diagnostics(
                &emitter,
                &diagnostic_source,
                "runtime.line_task.lower",
                errors.into_iter().map(|error| error.message().to_owned()),
            );
            ExitCode::FAILURE
        })
    })?;

    Ok(CheckedModule {
        hir,
        env: env.clone(),
        syntax_warnings: parse::count_warning_lints(&lints),
        line_task_groups,
        typecheck_report,
        phases,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_prefers_manifest_default() {
        let manifest = LaunchProfileManifest::parse_toml(
            r#"
default = "mobile"

[profiles.desktop]
kind = "game"
source = "main.arcw"

[profiles.mobile]
kind = "game"
source = "main.arcw"
"#,
        )
        .expect("manifest parses");

        assert_eq!(
            default_profile_id(&manifest, Path::new("arcw.toml"), LaunchKind::Game)
                .expect("default profile resolves"),
            Some("mobile".to_owned())
        );
    }

    #[test]
    fn default_profile_uses_single_matching_kind() {
        let manifest = LaunchProfileManifest::parse_toml(
            r#"
[profiles."game.main"]
kind = "game"
source = "game.arcw"

[profiles."server.dev"]
kind = "server"
source = "server.arcw"
"#,
        )
        .expect("manifest parses");

        assert_eq!(
            default_profile_id(&manifest, Path::new("arcw.toml"), LaunchKind::Game)
                .expect("default profile resolves"),
            Some("game.main".to_owned())
        );
    }

    #[test]
    fn default_profile_can_fall_back_to_project_root() {
        let manifest = LaunchProfileManifest::parse_toml(
            r#"
[package]
name = "smoke_project"
"#,
        )
        .expect("manifest parses");

        assert_eq!(
            default_profile_id(&manifest, Path::new("arcw.toml"), LaunchKind::Game)
                .expect("profile fallback resolves"),
            None
        );
    }

    #[test]
    fn default_profile_rejects_ambiguous_matching_kind() {
        let manifest = LaunchProfileManifest::parse_toml(
            r#"
[profiles.desktop]
kind = "game"
source = "main.arcw"

[profiles.mobile]
kind = "game"
source = "main.arcw"
"#,
        )
        .expect("manifest parses");

        assert!(default_profile_id(&manifest, Path::new("arcw.toml"), LaunchKind::Game).is_err());
    }
}
