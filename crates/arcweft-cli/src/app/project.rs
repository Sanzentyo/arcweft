use super::diagnostics::{DiagnosticEmitter, DiagnosticSource};
use super::runtime::options::{
    CliRuntimeMathBackend, CliRuntimePureBackend, CliRuntimePureWorkers,
};
use super::runtime::profile::run_profile_phase;
use super::shared::is_arcw_path;
use crate::output::RuntimeProfilePhase;
use arcweft_adapter_context::{
    manifest::AdapterManifest, publication::AdapterManifestSource, standard,
};
use arcweft_compiler::project::{
    AcceptedLaunchProfileInput, CompiledProject, ProjectCompilationContext,
    ProjectCompileDiagnostic, ProjectCompileError, ProjectEntrySelection,
    ProjectEntrySelectionKind, compile_project,
};
use arcweft_core::entry::{RootExecutionLimits, RuntimeCommandPolicy};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_id::PublicId;
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{
    callable::{EnvironmentCallablePublication, PRODUCTION_CALLABLE_LIMITS},
    check::TypeCheckReport,
    env::TypeCheckEnv,
    registration::ProjectRegistrationFacts,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_launch::{
    EntrySelectionId, LaunchKind, LaunchMathBackend, LaunchProfileSelection, LaunchPureBackend,
    diagnostic::ManifestDiagnosticCode, manifest::LaunchPureWorkers,
    resolve::ResolvedLaunchProfile,
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::{
    layout::AuthoredResourceRoots,
    sources::{ProjectSourceFile, ProjectSources},
};
use arcweft_project_loader::{
    environment::{
        ProfileRegistrationLoadRequest, ProjectLoadRequest, load_profile_registration,
        load_project_registration,
    },
    project::LoadedProject,
    topology::{
        LoadedProfileTopology, ProfileTopologyLoadRequest, ProfileTopologyOwnerId,
        load_profile_topology,
    },
};
use arcweft_runtime_accelerator::{
    RuntimePureAcceleratorConfig, RuntimePureBackendMode, RuntimePureWorkerCount,
    math::RuntimeMathBackend,
};
use arcweft_runtime_host::{NativeFileRoots, NativeTaskBridge};
use arcweft_runtime_plan::{flow::RuntimePlanLowerOptions, line_task::LoweredLineTaskGroup};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

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

pub(in crate::app) struct SelectionSemanticContext {
    base: TypeCheckEnv,
    adapter_manifests: Vec<AdapterManifest>,
    profile_registration_supplements: Vec<AdapterManifest>,
    profile_topology: Option<Arc<LoadedProfileTopology>>,
}

pub(in crate::app) struct DirectProjectCompilationInput {
    sources: ProjectSources,
    context: ProjectCompilationContext,
}

impl DirectProjectCompilationInput {
    pub(in crate::app) const fn sources(&self) -> &ProjectSources {
        &self.sources
    }

    pub(in crate::app) const fn context(&self) -> &ProjectCompilationContext {
        &self.context
    }
}

impl SelectionSemanticContext {
    pub(in crate::app) const fn base(&self) -> &TypeCheckEnv {
        &self.base
    }

    pub(in crate::app) fn adapter_manifests(&self) -> &[AdapterManifest] {
        &self.adapter_manifests
    }

    pub(in crate::app) fn profile_registration_supplements(&self) -> &[AdapterManifest] {
        &self.profile_registration_supplements
    }

    pub(in crate::app) fn profile_topology(&self) -> Option<&LoadedProfileTopology> {
        self.profile_topology.as_deref()
    }
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
        topology: Arc<LoadedProfileTopology>,
    },
}

enum CliTopologyLoadError {
    NoProfiles,
    Failed,
}

impl SourceSelection {
    pub(in crate::app) fn path(&self) -> &Path {
        match self {
            Self::Direct { path } | Self::Project { path, .. } => path,
            Self::Profile { topology } => topology.loaded_project().sources().root_module().path(),
        }
    }

    /// Returns the manifest only when this selection owns a project source
    /// graph. Launch profiles retain their manifest for package/resources but
    /// compile the explicitly selected source.
    pub(in crate::app) fn project_manifest(&self) -> Option<&Path> {
        match self {
            Self::Project { manifest, .. } => Some(manifest),
            Self::Direct { .. } | Self::Profile { .. } => None,
        }
    }

    pub(in crate::app) fn resource_manifest(&self) -> Option<&Path> {
        match self {
            Self::Project { manifest, .. } => Some(manifest),
            Self::Profile { topology } => Some(topology.loaded_project().sources().manifest_path()),
            Self::Direct { .. } => None,
        }
    }

    pub(in crate::app) fn profile(&self) -> Option<&ResolvedLaunchProfile> {
        match self {
            Self::Direct { .. } | Self::Project { .. } => None,
            Self::Profile { topology } => Some(topology.selected_profile()),
        }
    }

    pub(in crate::app) fn profile_topology(&self) -> Option<&LoadedProfileTopology> {
        match self {
            Self::Profile { topology } => Some(topology),
            Self::Direct { .. } | Self::Project { .. } => None,
        }
    }

    pub(in crate::app) fn authored_resource_roots(&self) -> AuthoredResourceRoots {
        if let Some(topology) = self.profile_topology() {
            return AuthoredResourceRoots::new(
                topology.layout().asset_root().as_path(),
                topology.layout().content_root().as_path(),
            );
        }
        if let Self::Project { manifest, .. } = self {
            let root = manifest.parent().unwrap_or_else(|| Path::new("."));
            return AuthoredResourceRoots::new(root.join("assets"), root.join("content"));
        }
        if let Some(root) = self.direct_containing_project_root() {
            return AuthoredResourceRoots::new(root.join("assets"), root.join("content"));
        }

        let source_dir = self.path().parent().unwrap_or_else(|| Path::new("."));
        AuthoredResourceRoots::new(source_dir.join("assets"), source_dir.join("content"))
    }

    pub(in crate::app) fn local_state_root(&self) -> PathBuf {
        self.resource_manifest()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| self.direct_containing_project_root())
            .or_else(|| self.path().parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".arcweft")
    }

    pub(in crate::app) fn native_file_roots(&self) -> NativeFileRoots {
        let authored = self.authored_resource_roots();
        NativeFileRoots::new(authored.asset(), self.local_state_root())
    }

    fn direct_containing_project_root(&self) -> Option<PathBuf> {
        let Self::Direct { path } = self else {
            return None;
        };
        arcweft_project_loader::project::discover_manifest(path)
            .ok()
            .and_then(|manifest| manifest.parent().map(Path::to_path_buf))
    }

    pub(in crate::app) fn entry(&self) -> Option<&str> {
        self.profile()
            .and_then(ResolvedLaunchProfile::entry)
            .and_then(|entry| entry.as_str().strip_prefix('@'))
    }

    pub(in crate::app) fn command_entry<'selection>(
        &'selection self,
        entry_override: Option<&'selection str>,
    ) -> Result<&'selection str, ExitCode> {
        match (self.profile(), self.entry(), entry_override) {
            (Some(_), Some(entry), None) => Ok(entry),
            (Some(profile), Some(entry), Some(_)) => {
                eprintln!(
                    "error: launch profile `{}` already selects `{}`; --entry cannot override a profile",
                    profile.id().as_str(),
                    entry
                );
                Err(ExitCode::from(2))
            }
            (_, None, Some(entry)) => {
                EntrySelectionId::new(entry).map_err(|error| {
                    eprintln!("error: --entry must be a canonical entry.* ID: {error}");
                    ExitCode::from(2)
                })?;
                Ok(entry)
            }
            (Some(profile), None, None) => {
                eprintln!(
                    "error: launch profile `{}` does not select an entry; pass --entry entry.*",
                    profile.id().as_str()
                );
                Err(ExitCode::from(2))
            }
            (None, Some(_), _) => unreachable!("direct selections have no profile entry"),
            (None, None, None) => {
                eprintln!("error: source launch requires --entry entry.*");
                Err(ExitCode::from(2))
            }
        }
    }

    pub(in crate::app) fn adapter(&self) -> Option<&str> {
        self.profile().map(|profile| profile.adapter().as_str())
    }

    pub(in crate::app) fn package_identity(&self) -> Result<String, ExitCode> {
        if let Some(topology) = self.profile_topology() {
            return Ok(topology
                .loaded_project()
                .sources()
                .package()
                .id
                .as_str()
                .to_owned());
        }
        if let Self::Project { manifest, .. } = self {
            return arcweft_project_loader::project::load(manifest)
                .map(|project| project.sources().package().id.as_str().to_owned())
                .map_err(|error| {
                    eprintln!("error: failed to resolve package identity: {error}");
                    ExitCode::FAILURE
                });
        }
        let package_name = self
            .path()
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
            .ok_or_else(|| {
                eprintln!("error: direct source has no package identity");
                ExitCode::FAILURE
            })?;
        direct_package_id(package_name)
            .map(|package| package.as_str().to_owned())
            .map_err(|error| {
                eprintln!(
                    "error: direct source has an invalid package identity `{package_name}`: {error}"
                );
                ExitCode::FAILURE
            })
    }
}

pub(in crate::app) fn runtime_plan_options_for_selection(
    selection: &SourceSelection,
) -> Result<RuntimePlanLowerOptions, ExitCode> {
    let options = RuntimePlanLowerOptions::default()
        .with_package_identity(selection.package_identity()?)
        .with_command_policy(RuntimeCommandPolicy::deny_all(
            RootExecutionLimits::engine_default(),
        ));
    Ok(options)
}

pub(in crate::app) fn runtime_pure_config_for_selection(
    selection: &SourceSelection,
    backend: Option<CliRuntimePureBackend>,
    workers: Option<CliRuntimePureWorkers>,
    batch_min_len: Option<usize>,
    object_artifacts: bool,
    math_backend: Option<CliRuntimeMathBackend>,
    math_wgpu_min_elements: Option<usize>,
) -> RuntimePureAcceleratorConfig {
    let mut config = RuntimePureAcceleratorConfig::default();
    if let Some(profile) = selection.profile().and_then(ResolvedLaunchProfile::pure) {
        if let Some(backend) = profile.backend() {
            config.backend = launch_pure_backend_mode(backend);
        }
        if let Some(backend) = profile.math_backend() {
            config.math.backend = launch_math_backend_mode(backend);
        }
        if let Some(min_elements) = profile.math_wgpu_min_elements() {
            config.math.wgpu_min_elements = min_elements.get() as usize;
        }
        if let Some(workers) = profile.workers() {
            config.workers = match workers {
                LaunchPureWorkers::Auto => RuntimePureWorkerCount::Auto,
                LaunchPureWorkers::Count(count) => {
                    RuntimePureWorkerCount::Fixed(count.get() as usize)
                }
            };
        }
        if let Some(batch_min_len) = profile.batch_min_len() {
            config.batch_min_len = batch_min_len.get() as usize;
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
    config
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
        (Some(path), None) => resolve_direct_or_containing_default_profile(path, preferred_kind),
        (None, Some(profile_id)) => resolve_profile_source_selection(profile, profile_id),
        (None, None) => {
            let manifest_path = resolve_manifest_path(&profile.manifest)?;
            match load_profile_topology_at(
                &manifest_path,
                LaunchProfileSelection::Automatic { previous: None },
            ) {
                Ok(topology) => {
                    if topology.selected_profile().kind() != preferred_kind {
                        eprintln!(
                            "error: default launch profile `{}` has kind {}; use --profile to select a {} profile",
                            topology.selected_profile().id(),
                            topology.selected_profile().kind().as_str(),
                            preferred_kind.as_str()
                        );
                        return Err(ExitCode::from(2));
                    }
                    Ok(SourceSelection::Profile { topology })
                }
                Err(CliTopologyLoadError::NoProfiles) => {
                    resolve_project_root_source_selection(&manifest_path)
                }
                Err(CliTopologyLoadError::Failed) => Err(ExitCode::FAILURE),
            }
        }
    }
}

fn resolve_direct_or_containing_default_profile(
    path: &Path,
    preferred_kind: LaunchKind,
) -> Result<SourceSelection, ExitCode> {
    let direct = || SourceSelection::Direct {
        path: path.to_path_buf(),
    };
    let Ok(manifest) = arcweft_project_loader::project::discover_manifest(path) else {
        return Ok(direct());
    };
    let topology = match load_profile_topology_at(
        &manifest,
        LaunchProfileSelection::Automatic { previous: None },
    ) {
        Ok(topology) => topology,
        Err(CliTopologyLoadError::NoProfiles) => return Ok(direct()),
        Err(CliTopologyLoadError::Failed) => return Err(ExitCode::FAILURE),
    };
    if topology.selected_profile().kind() != preferred_kind {
        return Ok(direct());
    }
    let selected_source = fs::canonicalize(
        topology.loaded_project().sources().root_module().path(),
    )
    .map_err(|error| {
        eprintln!(
            "error: failed to resolve selected profile source {}: {error}",
            topology
                .loaded_project()
                .sources()
                .root_module()
                .path()
                .display()
        );
        ExitCode::FAILURE
    })?;
    let requested_source = fs::canonicalize(path).map_err(|error| {
        eprintln!(
            "error: failed to resolve source path {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })?;
    if selected_source == requested_source {
        Ok(SourceSelection::Profile { topology })
    } else {
        Ok(direct())
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
    load_profile_topology_at(manifest_path, LaunchProfileSelection::Explicit(profile_id))
        .map(|topology| SourceSelection::Profile { topology })
        .map_err(|_| ExitCode::FAILURE)
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

fn load_profile_topology_at(
    manifest_path: &Path,
    selection: LaunchProfileSelection<'_>,
) -> Result<Arc<LoadedProfileTopology>, CliTopologyLoadError> {
    let manifest = fs::canonicalize(manifest_path).map_err(|error| {
        eprintln!(
            "error: failed to resolve profile manifest {}: {error}",
            manifest_path.display()
        );
        CliTopologyLoadError::Failed
    })?;
    let project_root = manifest.parent().ok_or_else(|| {
        eprintln!("error: profile manifest has no project root");
        CliTopologyLoadError::Failed
    })?;
    let owner = ProfileTopologyOwnerId::workspace(
        file_uri_identity(project_root),
        file_uri_identity(&manifest),
    )
    .map_err(|error| {
        eprintln!("error: invalid profile topology owner: {error}");
        CliTopologyLoadError::Failed
    })?;
    load_profile_topology(ProfileTopologyLoadRequest::new(
        &manifest,
        owner,
        selection,
        &[],
        standard::standard_registry(),
    ))
    .map(Arc::new)
    .map_err(|error| {
        if matches!(
            &error,
            arcweft_project_loader::topology::ProfileTopologyLoadError::ProfileSelection {
                source
            } if source
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.code() == ManifestDiagnosticCode::ProfileNone)
        ) {
            return CliTopologyLoadError::NoProfiles;
        }
        eprintln!("error: failed to load profile topology: {error}");
        CliTopologyLoadError::Failed
    })
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
    let semantic = semantic_context_for_selection(selection, adapter_override)?;
    let runtime_options = runtime_plan_options_for_selection(selection)?;
    if let Some(topology) = semantic.profile_topology() {
        let context = profile_project_compilation_context(topology, &semantic)?;
        return load_and_check_loaded_project(
            topology.loaded_project(),
            &context,
            &semantic,
            &runtime_options,
            phases,
        );
    }
    if let Some(manifest) = selection.project_manifest() {
        return load_and_check_project_with_env(
            manifest,
            selection,
            &semantic,
            &runtime_options,
            phases,
        );
    }
    let direct = direct_project_compilation_input(selection, &semantic, &mut phases)?;
    load_and_check_project_sources(
        direct.sources(),
        direct.context(),
        semantic.base(),
        &runtime_options,
        phases,
    )
}

fn load_and_check_project_with_env(
    manifest: &Path,
    selection: &SourceSelection,
    semantic: &SelectionSemanticContext,
    runtime_options: &RuntimePlanLowerOptions,
    mut phases: Vec<RuntimeProfilePhase>,
) -> Result<CheckedModule, ExitCode> {
    let loaded = run_profile_phase(&mut phases, "load_project", || {
        arcweft_project_loader::project::load(manifest).map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        })
    })?;
    let context = project_compilation_context(&loaded, selection, semantic)?;
    load_and_check_loaded_project(&loaded, &context, semantic, runtime_options, phases)
}

fn load_and_check_loaded_project(
    loaded: &LoadedProject,
    context: &ProjectCompilationContext,
    semantic: &SelectionSemanticContext,
    runtime_options: &RuntimePlanLowerOptions,
    phases: Vec<RuntimeProfilePhase>,
) -> Result<CheckedModule, ExitCode> {
    load_and_check_project_sources(
        loaded.sources(),
        context,
        semantic.base(),
        runtime_options,
        phases,
    )
}

fn load_and_check_project_sources(
    sources: &ProjectSources,
    context: &ProjectCompilationContext,
    env: &TypeCheckEnv,
    runtime_options: &RuntimePlanLowerOptions,
    mut phases: Vec<RuntimeProfilePhase>,
) -> Result<CheckedModule, ExitCode> {
    let source_document = Arc::clone(sources.root_module().document());
    let compiled = run_profile_phase(&mut phases, "project_compile", || {
        compile_project(sources, context, runtime_options).map_err(|error| {
            print_project_compile_error(&error);
            ExitCode::FAILURE
        })
    })?;
    let emitter = DiagnosticEmitter::stderr();
    for compiled_module in compiled.modules() {
        let source = sources
            .module(compiled_module.module())
            .expect("compiled project modules originate from the accepted project sources");
        let diagnostic_source = DiagnosticSource::new(source.document());
        for lint in compiled_module.syntax_lints() {
            emitter.emit(&lint.diagnostic(source.document()), &diagnostic_source);
        }
    }
    let compiled = Arc::new(compiled);
    Ok(CheckedModule {
        hir: compiled.linked_hir().clone(),
        env: env.clone(),
        source_document,
        syntax_warnings: compiled.syntax_warnings(),
        line_task_groups: compiled.line_task_groups().to_vec(),
        typecheck_report: compiled.typecheck_report().clone(),
        compiled,
        phases,
    })
}

pub(in crate::app) fn direct_project_compilation_input(
    selection: &SourceSelection,
    semantic: &SelectionSemanticContext,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<DirectProjectCompilationInput, ExitCode> {
    let package_identity = selection.package_identity()?;
    let package_id = PackageId::new(package_identity.clone()).map_err(|error| {
        eprintln!(
            "error: direct source has an invalid package identity `{package_identity}`: {error}"
        );
        ExitCode::FAILURE
    })?;
    direct_project_compilation_input_with_env(
        selection.path(),
        &package_id,
        semantic.base(),
        semantic.adapter_manifests(),
        phases,
    )
}

fn direct_project_compilation_input_with_env(
    path: &Path,
    package_id: &PackageId,
    env: &TypeCheckEnv,
    adapter_manifests: &[AdapterManifest],
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<DirectProjectCompilationInput, ExitCode> {
    if !is_arcw_path(path) {
        eprintln!("error: {} is not an .arcw source file", path.display());
        return Err(ExitCode::from(2));
    }
    let source = run_profile_phase(phases, "read_source", || {
        fs::read_to_string(path).map_err(|error| {
            eprintln!("error: failed to read {}: {error}", path.display());
            ExitCode::FAILURE
        })
    })?;
    let document = Arc::new(source_document_for_path(path, source)?);
    let sources = direct_project_sources(path, package_id, &document)?;
    let facts = direct_registration_facts(package_id, &document, adapter_manifests)?;
    let context = ProjectCompilationContext::new(
        Arc::new(env.clone()),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        None,
        callable_publications(adapter_manifests)?,
    );
    Ok(DirectProjectCompilationInput { sources, context })
}

fn direct_project_sources(
    path: &Path,
    package_id: &PackageId,
    document: &Arc<SourceDocument>,
) -> Result<ProjectSources, ExitCode> {
    let manifest_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-direct-manifest://{}",
                blake3::hash(path.to_string_lossy().as_bytes()).to_hex()
            ))
            .map_err(|error| {
                eprintln!("error: direct source manifest identity is invalid: {error}");
                ExitCode::FAILURE
            })?,
            SourceName::Memory,
            "",
        )
        .map_err(|error| {
            eprintln!("error: direct source manifest document is invalid: {error}");
            ExitCode::FAILURE
        })?,
    );
    ProjectSources::new(
        path.with_file_name("arcw.toml"),
        path.parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        PackageSpec {
            id: package_id.clone(),
            version: PackageVersion::new("0.0.0").expect("static direct version is valid"),
        },
        BuildSpec::default(),
        manifest_document,
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            path.to_path_buf(),
            Arc::clone(document),
            [],
        )],
    )
    .map_err(|error| {
        eprintln!("error: failed to construct direct-source project: {error}");
        ExitCode::FAILURE
    })
}

fn direct_registration_facts(
    package_id: &PackageId,
    document: &Arc<SourceDocument>,
    adapter_manifests: &[AdapterManifest],
) -> Result<ProjectRegistrationFacts, ExitCode> {
    let mut documents = vec![Arc::clone(document)];
    let mut external_facts = Vec::new();
    for (index, manifest) in adapter_manifests.iter().enumerate() {
        let ordinal = u64::try_from(index).map_err(|_| {
            eprintln!("error: direct-source adapter ordinal exceeds u64::MAX");
            ExitCode::FAILURE
        })?;
        let registration = manifest
            .source_backed_registration_facts(ordinal)
            .map_err(|error| {
                eprintln!(
                    "error: failed to publish direct-source adapter registration facts: {error}"
                );
                ExitCode::FAILURE
            })?;
        let (adapter_document, facts) = registration.into_parts();
        documents.push(adapter_document);
        external_facts.extend(facts);
    }

    let package = CallablePackageId::try_new(package_id.as_str()).map_err(|error| {
        eprintln!("error: invalid direct-source callable package identity: {error}");
        ExitCode::FAILURE
    })?;
    let world = ProjectSymbolWorldId::try_new(package, document.identity().id().clone(), "direct")
        .map_err(|error| {
            eprintln!("error: invalid direct-source semantic world: {error}");
            ExitCode::FAILURE
        })?;
    ProjectRegistrationFacts::try_new(world, documents, external_facts, Vec::new()).map_err(
        |report| {
            for diagnostic in report.diagnostics() {
                let diagnostic = diagnostic.diagnostic();
                eprintln!(
                    "error[{}]: {}",
                    diagnostic
                        .code()
                        .map_or("registration", arcweft_source::DiagnosticCode::as_str),
                    diagnostic.message()
                );
            }
            if report.omitted_diagnostics() > 0 {
                eprintln!(
                    "error: {} direct-source registration diagnostic(s) omitted",
                    report.omitted_diagnostics()
                );
            }
            ExitCode::FAILURE
        },
    )
}

fn direct_package_id(package_name: &str) -> Result<PackageId, &'static str> {
    let mut suffix = package_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while suffix.contains("--") {
        suffix = suffix.replace("--", "-");
    }
    let suffix = suffix.trim_matches('-');
    if suffix.is_empty() {
        return Err("package name has no portable identity characters");
    }
    PackageId::new(format!("local.arcweft.{suffix}"))
        .map_err(|_| "package name cannot be represented as a package ID")
}

pub(in crate::app) fn project_compilation_context(
    loaded: &LoadedProject,
    selection: &SourceSelection,
    semantic: &SelectionSemanticContext,
) -> Result<ProjectCompilationContext, ExitCode> {
    let request = ProjectLoadRequest::new(loaded, Vec::new(), Vec::new())
        .with_adapter_manifests(semantic.adapter_manifests().iter().cloned());
    let registration = load_project_registration(&request).map_err(|error| {
        eprintln!("error: failed to load project registration facts: {error}");
        ExitCode::FAILURE
    })?;
    let (facts, _) = registration.into_parts();
    compilation_context_from_facts(facts, selection.profile(), semantic)
}

pub(in crate::app) fn profile_project_compilation_context(
    topology: &LoadedProfileTopology,
    semantic: &SelectionSemanticContext,
) -> Result<ProjectCompilationContext, ExitCode> {
    let request = ProfileRegistrationLoadRequest::new(topology)
        .with_adapter_manifests(semantic.profile_registration_supplements());
    let registration = load_profile_registration(&request).map_err(|error| {
        eprintln!("error: failed to load profile registration facts: {error}");
        ExitCode::FAILURE
    })?;
    let (facts, _) = registration.into_parts();
    let context =
        compilation_context_from_facts(facts, Some(topology.selected_profile()), semantic)?;
    let accepted_profile = AcceptedLaunchProfileInput::new(
        Arc::clone(topology.manifest()),
        topology.selected_profile().id().clone(),
        topology.selected_profile().clone(),
        topology.source_documents_revision(),
        Arc::clone(context.resource_types()),
    );
    Ok(context.with_accepted_launch_profile(accepted_profile))
}

fn compilation_context_from_facts(
    facts: ProjectRegistrationFacts,
    profile: Option<&ResolvedLaunchProfile>,
    semantic: &SelectionSemanticContext,
) -> Result<ProjectCompilationContext, ExitCode> {
    let entry_selection = profile
        .and_then(|profile| profile.entry().map(|entry| (profile, entry)))
        .map(|(profile, entry)| {
            let entry = entry
                .as_str()
                .strip_prefix('@')
                .expect("resolved entry references use the @entry family");
            PublicId::try_new(entry)
                .map(|id| ProjectEntrySelection::new(id, project_entry_kind(profile.kind())))
                .map_err(|error| {
                    eprintln!("error: invalid launch-profile entry identity: {error}");
                    ExitCode::FAILURE
                })
        })
        .transpose()?;
    let callable_publications = callable_publications(semantic.adapter_manifests())?;
    Ok(ProjectCompilationContext::new(
        Arc::new(semantic.base().clone()),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        entry_selection,
        callable_publications,
    ))
}

fn callable_publications(
    adapter_manifests: &[AdapterManifest],
) -> Result<Vec<EnvironmentCallablePublication>, ExitCode> {
    let mut callable_publications = standard::callable_publications(&PRODUCTION_CALLABLE_LIMITS)
        .map_err(|error| {
            eprintln!("error: failed to publish standard callable catalog: {error}");
            ExitCode::FAILURE
        })?;
    for manifest in adapter_manifests {
        if let Some(source) = standard::manifest_source(manifest.id().as_str()) {
            if !manifest.rust_functions().is_empty() {
                callable_publications.push(
                    manifest
                        .try_rust_callable_publication(source, &PRODUCTION_CALLABLE_LIMITS)
                        .map_err(|error| {
                            eprintln!(
                                "error: failed to publish Rust ABI callable catalog: {error}"
                            );
                            ExitCode::FAILURE
                        })?,
                );
            }
            continue;
        }
        callable_publications.push(
            manifest
                .try_callable_publication(
                    AdapterManifestSource::SelectedAdapter,
                    &PRODUCTION_CALLABLE_LIMITS,
                )
                .map_err(|error| {
                    eprintln!("error: failed to publish adapter callable catalog: {error}");
                    ExitCode::FAILURE
                })?,
        );
    }
    Ok(callable_publications)
}

const fn project_entry_kind(kind: LaunchKind) -> ProjectEntrySelectionKind {
    match kind {
        LaunchKind::Game => ProjectEntrySelectionKind::Game,
        LaunchKind::Editor => ProjectEntrySelectionKind::Editor,
        LaunchKind::Server => ProjectEntrySelectionKind::Server,
        LaunchKind::Cli => ProjectEntrySelectionKind::Cli,
        LaunchKind::Test => ProjectEntrySelectionKind::Test,
        LaunchKind::Bench => ProjectEntrySelectionKind::Bench,
        LaunchKind::Agent => ProjectEntrySelectionKind::Agent,
    }
}

pub(in crate::app) fn source_document_for_path(
    path: &Path,
    text: impl Into<Arc<str>>,
) -> Result<SourceDocument, ExitCode> {
    let package = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| {
            eprintln!("error: source path has no valid package identity");
            ExitCode::FAILURE
        })?;
    let relative = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            eprintln!("error: source path has no valid project-relative name");
            ExitCode::FAILURE
        })?;
    let id = SourceDocumentId::try_new(format!("arcweft-project://{package}/{relative}")).map_err(
        |error| {
            eprintln!("error: invalid source document identity: {error}");
            ExitCode::FAILURE
        },
    )?;
    SourceDocument::try_new(id, SourceName::path(path.display().to_string()), text).map_err(
        |error| {
            eprintln!("error: failed to construct source document: {error}");
            ExitCode::FAILURE
        },
    )
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
    if let Some(source) = diagnostic.source() {
        let diagnostic_source = DiagnosticSource::new(source.document());
        emitter.emit(diagnostic.diagnostic(), &diagnostic_source);
        return;
    }
    emitter.emit_without_source(diagnostic.diagnostic());
}

pub(in crate::app) fn semantic_context_for_selection(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
) -> Result<SelectionSemanticContext, ExitCode> {
    let profile_topology = load_selection_profile_topology(selection);
    let manifest = match profile_topology.as_ref() {
        Some(topology) => adapter_manifest_from_topology(topology, adapter_override)?,
        None => adapter_manifest_for_selection(selection, adapter_override)?,
    };
    let env = if adapter_override.is_some() || selection.profile().is_some() {
        manifest.apply_to_target_env(TypeCheckEnv::standard())
    } else {
        manifest.apply_to_env(TypeCheckEnv::standard())
    };
    let desktop = arcweft_adapter_desktop::standard_desktop_manifests();
    let env = desktop
        .iter()
        .fold(env, |env, manifest| manifest.apply_to_env(env));
    let mut adapter_manifests = Vec::with_capacity(desktop.len() + 1);
    adapter_manifests.push(manifest);
    adapter_manifests.extend(desktop.iter().cloned());
    Ok(SelectionSemanticContext {
        base: env,
        adapter_manifests,
        profile_registration_supplements: profile_topology
            .as_ref()
            .map_or_else(Vec::new, |_| desktop),
        profile_topology,
    })
}

fn load_selection_profile_topology(
    selection: &SourceSelection,
) -> Option<Arc<LoadedProfileTopology>> {
    match selection {
        SourceSelection::Profile { topology } => Some(Arc::clone(topology)),
        SourceSelection::Direct { .. } | SourceSelection::Project { .. } => None,
    }
}

fn adapter_manifest_from_topology(
    topology: &LoadedProfileTopology,
    adapter_override: Option<&str>,
) -> Result<AdapterManifest, ExitCode> {
    let Some(adapter_id) = adapter_override else {
        return Ok(topology.adapter().clone());
    };
    if topology.adapter().id().as_str() == adapter_id {
        return Ok(topology.adapter().clone());
    }
    if let Some(manifest) = standard::standard_registry().get(adapter_id) {
        return Ok(manifest.clone());
    }
    eprintln!("error: unknown adapter `{adapter_id}`");
    Err(ExitCode::from(2))
}

fn file_uri_identity(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let body = if normalized
        .as_bytes()
        .get(1)
        .is_some_and(|byte| *byte == b':')
    {
        format!("/{normalized}")
    } else {
        normalized
    };
    format!("file://{body}")
}

pub(in crate::app) fn adapter_manifest_for_selection(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
) -> Result<AdapterManifest, ExitCode> {
    let adapter_id = adapter_override
        .or(selection.adapter())
        .unwrap_or(standard::SANS_IO_ADAPTER_ID);
    let registry = adapter_registry_for_selection(selection);
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
) -> arcweft_adapter_context::manifest::AdapterRegistry {
    let Some(topology) = selection.profile_topology() else {
        return standard::standard_registry();
    };
    let selected = topology.adapter();
    arcweft_adapter_context::manifest::AdapterRegistry::from_manifests(
        standard::standard_registry()
            .manifests()
            .iter()
            .filter(|manifest| manifest.id() != selected.id())
            .cloned()
            .chain([selected.clone()]),
    )
}

pub(crate) struct CheckedModule {
    pub(crate) compiled: Arc<CompiledProject>,
    pub(crate) hir: arcweft_lang_hir::model::HirModule,
    pub(crate) env: TypeCheckEnv,
    pub(crate) source_document: Arc<SourceDocument>,
    pub(crate) syntax_warnings: usize,
    pub(crate) line_task_groups: Vec<LoweredLineTaskGroup>,
    pub(crate) typecheck_report: TypeCheckReport,
    pub(crate) phases: Vec<RuntimeProfilePhase>,
}

impl CheckedModule {
    pub(crate) fn runtime_plan(&self) -> &arcweft_runtime_plan::flow::RuntimePlanLowerReport {
        self.compiled.runtime_plan()
    }
}

pub(in crate::app) fn load_and_check_with_env(
    path: &Path,
    env: &TypeCheckEnv,
    mut phases: Vec<RuntimeProfilePhase>,
) -> Result<CheckedModule, ExitCode> {
    let package_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| {
            eprintln!("error: direct source has no package identity");
            ExitCode::FAILURE
        })?;
    let package_id = direct_package_id(package_name).map_err(|error| {
        eprintln!("error: direct source has an invalid package identity `{package_name}`: {error}");
        ExitCode::FAILURE
    })?;
    let direct =
        direct_project_compilation_input_with_env(path, &package_id, env, &[], &mut phases)?;
    let runtime_options = RuntimePlanLowerOptions::default()
        .with_package_identity(package_id.as_str())
        .with_command_policy(RuntimeCommandPolicy::deny_all(
            RootExecutionLimits::engine_default(),
        ));
    load_and_check_project_sources(
        direct.sources(),
        direct.context(),
        env,
        &runtime_options,
        phases,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_source_entry_is_required_and_canonical() {
        let selection = SourceSelection::Direct {
            path: PathBuf::from("main.arcw"),
        };

        assert!(selection.command_entry(None).is_err());
        assert!(selection.command_entry(Some("main")).is_err());
        assert!(selection.command_entry(Some("@entry.main")).is_err());
        assert_eq!(
            selection.command_entry(Some("entry.game.main")),
            Ok("entry.game.main")
        );
    }

    #[test]
    fn direct_source_inside_project_uses_manifest_owned_resource_roots() {
        let root = std::env::temp_dir().join(format!(
            "arcweft-direct-project-roots-{}",
            std::process::id()
        ));
        let source_root = root.join("src");
        fs::create_dir_all(&source_root).expect("test source root");
        fs::write(root.join("arcw.toml"), "").expect("test manifest");
        let source = source_root.join("main.arcw");
        fs::write(&source, "").expect("test source");
        let selection = SourceSelection::Direct { path: source };

        let authored = selection.authored_resource_roots();
        assert_eq!(authored.asset(), root.join("assets"));
        assert_eq!(authored.content(), root.join("content"));
        assert_eq!(selection.local_state_root(), root.join(".arcweft"));

        fs::remove_dir_all(root).expect("remove test project");
    }

    #[test]
    fn launch_kind_maps_to_the_compiler_entry_selection_without_string_dispatch() {
        for (launch, compiler) in [
            (LaunchKind::Game, ProjectEntrySelectionKind::Game),
            (LaunchKind::Editor, ProjectEntrySelectionKind::Editor),
            (LaunchKind::Server, ProjectEntrySelectionKind::Server),
            (LaunchKind::Cli, ProjectEntrySelectionKind::Cli),
            (LaunchKind::Test, ProjectEntrySelectionKind::Test),
            (LaunchKind::Bench, ProjectEntrySelectionKind::Bench),
            (LaunchKind::Agent, ProjectEntrySelectionKind::Agent),
        ] {
            assert_eq!(project_entry_kind(launch), compiler);
        }
    }
}
