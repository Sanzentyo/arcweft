use super::runtime::{
    CliRuntimeMathBackend, CliRuntimePureBackend, CliRuntimePureWorkers,
    parse_runtime_pure_workers, run_profile_phase,
};
use super::shared::is_arcw_path;
use crate::output::RuntimeProfilePhase;
use arcweft_adapter_context::{codec::AdapterManifestFile, manifest::AdapterManifest, standard};
use arcweft_compiler::{LoweredLineTaskGroup, RuntimePlanLowerOptions};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_lang_sema::{check::TypeCheckReport, env::TypeCheckEnv};
use arcweft_launch::{
    LaunchKind, LaunchMathBackend, LaunchProfileManifest, LaunchPureBackend, ResolvedLaunchProfile,
};
use arcweft_runtime_accelerator::{
    RuntimePureAcceleratorConfig, RuntimePureBackendMode, RuntimePureWorkerCount,
    math::RuntimeMathBackend,
};
use arcweft_runtime_host::NativeTaskBridge;
use arcweft_rust_abi::ArcweftRustManifest;
use clap::Args;
use std::fs;
use std::net::SocketAddr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Args, Clone, Debug, Default)]
pub(in crate::app) struct ProfileOptions {
    #[arg(long)]
    pub(in crate::app) profile: Option<String>,
    #[arg(long, default_value = "arcw.toml")]
    pub(in crate::app) manifest: PathBuf,
}

#[derive(Clone, Debug)]
pub(in crate::app) enum SourceSelection {
    Direct { path: PathBuf },
    Profile(Box<ResolvedLaunchProfile>),
}

impl SourceSelection {
    pub(in crate::app) fn path(&self) -> &Path {
        match self {
            Self::Direct { path } => path,
            Self::Profile(profile) => profile.source(),
        }
    }

    pub(in crate::app) fn profile(&self) -> Option<&ResolvedLaunchProfile> {
        match self {
            Self::Direct { .. } => None,
            Self::Profile(profile) => Some(profile),
        }
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
        (None, Some(profile_id)) => {
            let source = fs::read_to_string(&profile.manifest).map_err(|error| {
                eprintln!(
                    "error: failed to read launch manifest {}: {error}",
                    profile.manifest.display()
                );
                ExitCode::FAILURE
            })?;
            let manifest = LaunchProfileManifest::parse_toml(&source).map_err(|error| {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            })?;
            let manifest_dir = profile.manifest.parent().unwrap_or_else(|| Path::new("."));
            let adapter_registry = standard::standard_registry();
            let adapter_ids = adapter_registry.adapter_ids();
            let resolved = manifest
                .resolve_profile_with_adapters(profile_id, manifest_dir, &adapter_ids)
                .map_err(|error| {
                    eprintln!("error: {error}");
                    ExitCode::FAILURE
                })?;
            Ok(SourceSelection::Profile(Box::new(resolved)))
        }
        (None, None) => {
            eprintln!("error: expected .arcw source path or --profile");
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
        "error: launch profile `{}` has kind {:?}; use an `{command}` profile for `arcw {command}`",
        profile.id().as_str(),
        profile.kind()
    );
    Err(ExitCode::from(2))
}

pub(in crate::app) fn profile_listen_addr(
    selection: &SourceSelection,
) -> Result<Option<SocketAddr>, ExitCode> {
    let Some(raw) = selection.profile().and_then(ResolvedLaunchProfile::listen) else {
        return Ok(None);
    };
    raw.parse::<SocketAddr>().map(Some).map_err(|error| {
        eprintln!("error: invalid launch profile listen address `{raw}`: {error}");
        ExitCode::from(2)
    })
}

pub(in crate::app) fn load_and_check_selection(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
) -> Result<CheckedModule, ExitCode> {
    let mut phases = Vec::new();
    let env = typecheck_env_for_selection(selection, adapter_override, &mut phases)?;
    load_and_check_with_env(selection.path(), &env, phases)
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
    Ok(manifest.apply_to_env(TypeCheckEnv::standard()))
}

pub(in crate::app) fn adapter_manifest_for_selection(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
) -> Result<AdapterManifest, ExitCode> {
    let adapter_id = adapter_override.or(selection.adapter());
    let registry = adapter_registry_for_selection(selection)?;
    adapter_manifest_from_registry(&registry, adapter_id)
}

fn adapter_manifest_from_registry(
    registry: &arcweft_adapter_context::manifest::AdapterRegistry,
    adapter: Option<&str>,
) -> Result<AdapterManifest, ExitCode> {
    let adapter_id = adapter.unwrap_or(standard::SANS_IO_ADAPTER_ID);
    if let Some(manifest) = registry.get(adapter_id) {
        return Ok(manifest.clone());
    }
    eprintln!("error: unknown adapter `{adapter_id}`");
    Err(ExitCode::from(2))
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
            read_adapter_manifest(path).map(|manifest| registry.with_manifest(manifest))
        })
}

fn read_adapter_manifest(path: &Path) -> Result<AdapterManifest, ExitCode> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!(
            "error: failed to read adapter manifest {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })?;
    let file = match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => AdapterManifestFile::from_json(&source),
        _ => AdapterManifestFile::from_toml(&source),
    }
    .map_err(|error| {
        eprintln!(
            "error: failed to parse adapter manifest {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })?;
    Ok(file.into_manifest())
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
            let source = fs::read_to_string(path).map_err(|error| {
                eprintln!(
                    "error: failed to read Rust ABI metadata {}: {error}",
                    path.display()
                );
                ExitCode::FAILURE
            })?;
            ArcweftRustManifest::from_json(&source).map_err(|error| {
                eprintln!(
                    "error: failed to parse Rust ABI metadata {}: {error}",
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
    pub(crate) syntax_stats: arcweft_lang_syntax::cst::SyntaxParseStats,
    pub(crate) line_task_groups: Vec<LoweredLineTaskGroup>,
    pub(crate) typecheck_report: TypeCheckReport,
    pub(crate) phases: Vec<RuntimeProfilePhase>,
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
        catch_unwind(AssertUnwindSafe(|| {
            arcweft_compiler::parse_source_text(source)
        }))
        .map_err(|_| {
            eprintln!("error: parser panicked while checking {}", path.display());
            ExitCode::FAILURE
        })
    })?;
    if !parsed.errors().is_empty() {
        for error in parsed.errors() {
            eprintln!("error: {}", error.message());
        }
        return Err(ExitCode::FAILURE);
    }

    let syntax_stats = parsed.syntax_stats();
    let tree = parsed.into_typed_tree();
    let lints = run_profile_phase(&mut phases, "lint", || {
        Ok::<Vec<arcweft_lang_syntax::lint::SyntaxLint>, ExitCode>(
            arcweft_compiler::lint_source_tree(&tree),
        )
    })?;
    for lint in &lints {
        eprintln!(
            "{}[{} {}]: {}",
            lint.severity().label(),
            lint.code().stable_code(),
            lint.code().domain_name(),
            lint.message()
        );
    }
    if arcweft_compiler::has_error_lints(&lints) {
        return Err(ExitCode::FAILURE);
    }

    let hir = run_profile_phase(&mut phases, "lower_hir", || {
        arcweft_compiler::lower_source_tree(&tree).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;

    run_profile_phase(&mut phases, "resolve", || {
        arcweft_compiler::resolve_hir_references(&hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {error}");
            }
            ExitCode::FAILURE
        })
    })?;
    run_profile_phase(&mut phases, "readiness", || {
        arcweft_compiler::validate_hir_typecheck_ready(&hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;
    let typecheck_report = run_profile_phase(&mut phases, "typecheck", || {
        arcweft_compiler::typecheck_hir_with_env(&hir, env).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;

    let line_task_groups = run_profile_phase(&mut phases, "line_task_lower", || {
        arcweft_compiler::lower_source_line_tasks(&hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;

    Ok(CheckedModule {
        hir,
        env: env.clone(),
        syntax_warnings: arcweft_compiler::count_warning_lints(&lints),
        syntax_stats,
        line_task_groups,
        typecheck_report,
        phases,
    })
}
