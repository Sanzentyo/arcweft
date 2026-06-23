//! Cargo-like project commands and the rustc-like single-source compiler route.

use super::bundle::{
    build_patch_bundle_artifact_from_awfb_bytes, compile_bundle_for_selection,
    write_bundle_artifact, write_patch_bundle_artifact,
};
use super::project::{
    ProfileOptions, SourceSelection, load_and_check_selection, print_project_compile_error,
    resolve_source_selection, runtime_plan_options_for_selection, typecheck_env_for_selection,
};
use super::runtime::run::watch_inputs;
use super::shared::print_json;
use arcweft_bundle::{
    BundleFormat, BundleVirtualFileSpace,
    container::{BundleView, ReadBudget},
    patch::BundlePatchArtifact,
    patch::encode_patch_bundle,
};
use arcweft_compiler::{
    incremental::{BuildSnapshotRequest, snapshot_compiled_project},
    lower::lower_source_runtime_plan_with_options,
    project::{
        CompiledProject, InMemoryProjectCompileCache, NoProjectCompileCache, ProjectCompileCache,
        compile_project_with_cache,
    },
};
use arcweft_lang_sema::project_index::{ProgramHash, project_semantic_index_from_hir};
use arcweft_project::{
    artifact::{ArtifactKey, ArtifactKeyInput, ArtifactKind},
    fingerprint::{BuildDigest, NamedDigest},
    incremental::{BuildSnapshot, QueryKind},
};
use arcweft_project_loader::cache::store::FilesystemCacheStore;
use arcweft_project_loader::project::{LoadedProject, ProjectLoadError};
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::SourceName;
use arcweft_verify::{
    BackendKind, Severity, VerificationDiagnostic, VerificationMode, VerificationPolicy,
    VerificationReport, verify_module_with_env,
};
use clap::{Args, ValueEnum};
use serde::Serialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
    thread,
    time::Duration,
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
    watch: bool,
    #[arg(long, default_value_t = 250)]
    watch_poll_ms: u64,
    #[arg(long, hide = true, default_value_t = 0)]
    watch_iterations: usize,
    #[arg(long)]
    patch_base: Option<PathBuf>,
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
    snapshot: BuildSnapshot,
}

struct ProjectBuildArtifacts {
    bundle_path: PathBuf,
    metadata_path: PathBuf,
    plan_path: PathBuf,
    snapshot_path: PathBuf,
    cache_root: PathBuf,
    cache_records: Vec<ProjectBuildCacheRecordReport>,
    bundle_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
struct ProjectBuildCacheRecordReport {
    query: QueryKind,
    artifact_kind: ArtifactKind,
    logical_item: String,
    status: &'static str,
    key: String,
    object_digest: String,
}

struct ProjectBuildCacheInputs<'a> {
    state: &'a ProjectCommandState,
    report: &'a ProjectCommandReport,
    snapshot: &'a BuildSnapshot,
    cache_root: &'a Path,
    build_input_digest: BuildDigest,
    plan_bytes: &'a [u8],
    bundle_bytes: &'a [u8],
    hit_bundle_key: Option<ArtifactKey>,
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
    if options.watch && options.json {
        eprintln!("error: `arcw build --watch` cannot be combined with `--json`");
        return Err(ExitCode::from(2));
    }
    let mode = ProjectBuildMode::from_release(options.release);
    let mut compile_cache = InMemoryProjectCompileCache::default();
    let state = if options.watch {
        compile_project_command_with_cache(
            &options.profile,
            mode.verification_mode(),
            &mut compile_cache,
        )?
    } else {
        compile_project_command(&options.profile, mode.verification_mode())?
    };
    let report = ProjectCommandReport::from_state(&state);
    if report.status != "ok" {
        print_verification_diagnostics(&state.verification);
        if options.json {
            print_json(&report)?;
        }
        return Err(ExitCode::FAILURE);
    }

    let target_root = project_build_target_root(options, &state, mode);
    let artifacts = write_project_build_artifacts(&state, &report, &target_root)?;
    let mut patch_artifacts = Vec::new();
    if let Some(base) = options.patch_base.as_ref() {
        patch_artifacts.push(write_project_build_patch_from_base(
            base,
            &artifacts.bundle_bytes,
            &target_root,
            state.loaded.sources().manifest().package().name().as_str(),
        )?);
    }

    if options.watch {
        println!(
            "watch: built {} (metadata={}, plan={}, snapshot={}, cache={} record(s) at {})",
            artifacts.bundle_path.display(),
            artifacts.metadata_path.display(),
            artifacts.plan_path.display(),
            artifacts.snapshot_path.display(),
            artifacts.cache_records.len(),
            artifacts.cache_root.display(),
        );
        project_build_watch_loop(
            options,
            mode,
            &target_root,
            state,
            artifacts.bundle_bytes,
            &mut compile_cache,
        )?;
        return Ok(());
    }

    let artifact_paths = project_build_artifact_paths(&artifacts, &patch_artifacts);
    if options.json {
        print_json(&serde_json::json!({
            "report": report,
            "artifacts": artifact_paths,
            "cache": {
                "root": artifacts.cache_root,
                "records": artifacts.cache_records,
            },
        }))?;
    } else {
        println!(
            "Finished `{}` profile: {} module(s), {} compile unit(s)",
            mode.directory(),
            report.modules.len(),
            report.compile_units.len(),
        );
        for artifact in artifact_paths {
            println!("  {artifact}");
        }
        println!(
            "  cache: {} record(s) at {}",
            artifacts.cache_records.len(),
            artifacts.cache_root.display()
        );
    }
    Ok(())
}

fn project_build_target_root(
    options: &ProjectBuildOptions,
    state: &ProjectCommandState,
    mode: ProjectBuildMode,
) -> PathBuf {
    options
        .target_dir
        .clone()
        .unwrap_or_else(|| state.loaded.sources().target_root())
        .join(mode.directory())
}

fn write_project_build_artifacts(
    state: &ProjectCommandState,
    report: &ProjectCommandReport,
    target_root: &Path,
) -> Result<ProjectBuildArtifacts, ExitCode> {
    fs::create_dir_all(target_root).map_err(|error| {
        eprintln!(
            "error: failed to create build directory {}: {error}",
            target_root.display()
        );
        ExitCode::FAILURE
    })?;
    let package = state.loaded.sources().manifest().package().name().as_str();
    let metadata_path = target_root.join(format!("{package}.project.json"));
    let plan_path = target_root.join(format!("{package}.plan"));
    let snapshot_path = target_root.join(format!("{package}.snapshot.json"));
    let bundle_path = target_root.join(format!("{package}.awfb"));
    write_json_file(&metadata_path, &report)?;
    let plan_bytes = format!("{:#?}\n", state.compiled.runtime_plan().plan).into_bytes();
    fs::write(&plan_path, &plan_bytes).map_err(|error| {
        eprintln!("error: failed to write {}: {error}", plan_path.display());
        ExitCode::FAILURE
    })?;

    let cache_root = project_cache_root(target_root);
    let build_input_digest = project_build_input_digest(state)?;
    let bundle_key = project_build_artifact_key(
        state,
        QueryKind::BundleIndex,
        ArtifactKind::BundleIndex,
        "program-awfb",
        build_input_digest,
    );
    let (bundle_bytes, bundle_cache_hit) = read_cached_project_bundle(&cache_root, bundle_key)
        .map_or_else(
            || compile_project_bundle_bytes(state),
            |bytes| Ok((bytes, true)),
        )?;
    let mut phases = Vec::new();
    write_bundle_artifact(&bundle_path, bundle_bytes.clone(), &mut phases)?;
    let content_root = awfb_content_root_digest(&bundle_bytes)?;
    let snapshot = state.snapshot.clone().with_content_root(content_root);
    write_json_file(&snapshot_path, &snapshot)?;
    let cache_inputs = ProjectBuildCacheInputs {
        state,
        report,
        snapshot: &snapshot,
        cache_root: &cache_root,
        build_input_digest,
        plan_bytes: &plan_bytes,
        bundle_bytes: &bundle_bytes,
        hit_bundle_key: bundle_cache_hit.then_some(bundle_key),
    };
    let cache_records = store_project_build_cache_artifacts(&cache_inputs)?;
    Ok(ProjectBuildArtifacts {
        bundle_path,
        metadata_path,
        plan_path,
        snapshot_path,
        cache_root,
        cache_records,
        bundle_bytes,
    })
}

fn project_build_artifact_paths(
    artifacts: &ProjectBuildArtifacts,
    patch_artifacts: &[PathBuf],
) -> Vec<String> {
    [
        artifacts.bundle_path.display().to_string(),
        artifacts.metadata_path.display().to_string(),
        artifacts.plan_path.display().to_string(),
        artifacts.snapshot_path.display().to_string(),
    ]
    .into_iter()
    .chain(
        patch_artifacts
            .iter()
            .map(|path| path.display().to_string()),
    )
    .collect()
}

fn project_cache_root(target_root: &Path) -> PathBuf {
    target_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("cache")
        .join("v1")
}

fn awfb_content_root_digest(bundle_bytes: &[u8]) -> Result<BuildDigest, ExitCode> {
    let view = BundleView::parse(bundle_bytes, ReadBudget::default()).map_err(|error| {
        eprintln!("error: failed to inspect generated AWFB content root: {error}");
        ExitCode::FAILURE
    })?;
    Ok(BuildDigest::from_bytes(view.content_root().as_bytes()))
}

fn compile_project_bundle_bytes(state: &ProjectCommandState) -> Result<(Vec<u8>, bool), ExitCode> {
    let mut phases = Vec::new();
    let bundle = compile_bundle_for_selection(
        &state.selection,
        vec![BundleVirtualFileSpace::Asset],
        &mut phases,
    )?
    .bundle;
    let bundle_bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .map_err(|error| {
            eprintln!("error: failed to encode project bundle: {error}");
            ExitCode::FAILURE
        })?;
    Ok((bundle_bytes, false))
}

fn read_cached_project_bundle(cache_root: &Path, key: ArtifactKey) -> Option<Vec<u8>> {
    let store = FilesystemCacheStore::new(cache_root);
    let bytes = match store.read_artifact(QueryKind::BundleIndex, key) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!(
                "warning: ignoring invalid cached project bundle under {}: {error}",
                cache_root.display()
            );
            return None;
        }
    };
    let bytes = bytes?;
    if let Err(error) = BundleView::parse(&bytes, ReadBudget::default()) {
        eprintln!(
            "warning: ignoring cached project bundle with invalid AWFB bytes under {}: {error}",
            cache_root.display()
        );
        return None;
    }
    Some(bytes)
}

fn project_build_input_digest(state: &ProjectCommandState) -> Result<BuildDigest, ExitCode> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&state.snapshot.project().digest().as_bytes());
    for path in watch_inputs(&state.selection)?.keys() {
        bytes.extend_from_slice(path.to_string_lossy().as_bytes());
        bytes.push(0);
        let input = fs::read(path).map_err(|error| {
            eprintln!(
                "error: failed to read build input {} for cache key: {error}",
                path.display()
            );
            ExitCode::FAILURE
        })?;
        bytes.extend_from_slice(&BuildDigest::of(&input).as_bytes());
    }
    Ok(BuildDigest::of(&bytes))
}

fn store_project_build_cache_artifacts(
    inputs: &ProjectBuildCacheInputs<'_>,
) -> Result<Vec<ProjectBuildCacheRecordReport>, ExitCode> {
    let package = inputs
        .state
        .loaded
        .sources()
        .manifest()
        .package()
        .name()
        .as_str();
    let store = FilesystemCacheStore::new(inputs.cache_root);
    let _lock = store.lock_package(package).map_err(|error| {
        eprintln!(
            "error: failed to acquire build cache lock under {}: {error}",
            inputs.cache_root.display()
        );
        ExitCode::FAILURE
    })?;
    let metadata_bytes = serde_json::to_vec_pretty(inputs.report).map_err(|error| {
        eprintln!("error: failed to encode project metadata for cache: {error}");
        ExitCode::FAILURE
    })?;
    let snapshot_bytes = serde_json::to_vec_pretty(inputs.snapshot).map_err(|error| {
        eprintln!("error: failed to encode build snapshot for cache: {error}");
        ExitCode::FAILURE
    })?;
    [
        (
            QueryKind::TypeCheck,
            ArtifactKind::TypeCheckReport,
            "project-metadata",
            metadata_bytes.as_slice(),
        ),
        (
            QueryKind::RuntimePlan,
            ArtifactKind::RuntimePlan,
            "runtime-plan",
            inputs.plan_bytes,
        ),
        (
            QueryKind::LinkPlan,
            ArtifactKind::LinkPlan,
            "build-snapshot",
            snapshot_bytes.as_slice(),
        ),
        (
            QueryKind::BundleIndex,
            ArtifactKind::BundleIndex,
            "program-awfb",
            inputs.bundle_bytes,
        ),
    ]
    .into_iter()
    .map(|(query, artifact_kind, logical_item, bytes)| {
        let key = project_build_artifact_key(
            inputs.state,
            query,
            artifact_kind,
            logical_item,
            inputs.build_input_digest,
        );
        let record = store
            .store_artifact_with_logical_item(query, key, artifact_kind, Some(logical_item), bytes)
            .map_err(|error| {
                eprintln!(
                    "error: failed to store build cache artifact `{logical_item}` under {}: {error}",
                    inputs.cache_root.display()
                );
                ExitCode::FAILURE
            })?;
        Ok(ProjectBuildCacheRecordReport {
            query,
            artifact_kind,
            logical_item: logical_item.to_owned(),
            status: if inputs.hit_bundle_key == Some(key) {
                "hit"
            } else {
                "stored"
            },
            key: key.to_string(),
            object_digest: record.object_digest().to_string(),
        })
    })
    .collect()
}

fn project_build_artifact_key(
    state: &ProjectCommandState,
    query: QueryKind,
    artifact_kind: ArtifactKind,
    logical_item: &str,
    build_input_digest: BuildDigest,
) -> ArtifactKey {
    let project = state.snapshot.project();
    let modules = state.snapshot.modules();
    ArtifactKey::derive(&ArtifactKeyInput {
        compiler_build_id: project.compiler_build_id().to_owned(),
        query,
        artifact_kind,
        target_triple: project.target_triple().to_owned(),
        target_features: project.target_features().to_vec(),
        profile: project.profile().to_owned(),
        package: project.package().to_owned(),
        logical_item: logical_item.to_owned(),
        source_digest: build_input_digest,
        dependency_interface_digests: modules
            .iter()
            .map(|module| NamedDigest::new(module.module(), module.interface_digest()))
            .collect(),
        dependency_body_digests: modules
            .iter()
            .map(|module| NamedDigest::new(module.module(), module.body_digest()))
            .collect(),
        adapter_environment_digest: project.adapter_environment_digest(),
        launch_profile_digest: project.launch_profile_digest(),
        declared_environment_digest: project.declared_environment_digest(),
        format_options_digest: BuildDigest::of(query.cache_namespace().as_bytes()),
    })
}

fn write_project_build_patch_from_base(
    base: &Path,
    next_bytes: &[u8],
    target_root: &Path,
    package: &str,
) -> Result<PathBuf, ExitCode> {
    let base_bytes = fs::read(base).map_err(|error| {
        eprintln!(
            "error: failed to read patch base bundle {}: {error}",
            base.display()
        );
        ExitCode::FAILURE
    })?;
    let artifact = build_patch_bundle_artifact_from_awfb_bytes(&base_bytes, next_bytes)?;
    write_project_build_patch_artifact(target_root, package, &artifact)
}

fn write_project_build_patch_artifact(
    target_root: &Path,
    package: &str,
    artifact: &BundlePatchArtifact,
) -> Result<PathBuf, ExitCode> {
    let patch_output = project_build_patch_output_path(target_root, package, artifact);
    let patch_bytes = encode_patch_bundle(artifact).map_err(|error| {
        eprintln!("error: failed to encode build patch bundle: {error}");
        ExitCode::FAILURE
    })?;
    write_patch_bundle_artifact(&patch_output, patch_bytes)?;
    Ok(patch_output)
}

fn project_build_patch_output_path(
    target_root: &Path,
    package: &str,
    artifact: &BundlePatchArtifact,
) -> PathBuf {
    target_root.join("patches").join(format!(
        "{package}-{}-{}.awfb",
        artifact.manifest.base_content_root, artifact.manifest.target_content_root
    ))
}

fn project_build_watch_loop(
    options: &ProjectBuildOptions,
    mode: ProjectBuildMode,
    target_root: &Path,
    initial_state: ProjectCommandState,
    mut base_bytes: Vec<u8>,
    compile_cache: &mut InMemoryProjectCompileCache,
) -> Result<(), ExitCode> {
    let mut selection = initial_state.selection;
    let mut base_snapshot = initial_state.snapshot;
    let mut inputs = watch_inputs(&selection)?;
    println!(
        "watch: tracking {} input(s) under {}",
        inputs.len(),
        target_root.display()
    );
    let max_iterations = (options.watch_iterations > 0).then_some(options.watch_iterations);
    let mut iterations = 0_usize;
    loop {
        if max_iterations.is_some_and(|max| iterations >= max) {
            return Ok(());
        }
        iterations += 1;
        thread::sleep(Duration::from_millis(options.watch_poll_ms));
        let next_inputs = watch_inputs(&selection)?;
        if next_inputs == inputs {
            continue;
        }
        match compile_project_command_with_cache(
            &options.profile,
            mode.verification_mode(),
            compile_cache,
        ) {
            Ok(next_state) => {
                let report = ProjectCommandReport::from_state(&next_state);
                if report.status != "ok" {
                    print_verification_diagnostics(&next_state.verification);
                    eprintln!("watch: rebuild failed verification; keeping previous bundle active");
                    if max_iterations.is_some() {
                        return Err(ExitCode::FAILURE);
                    }
                    continue;
                }
                let artifacts = write_project_build_artifacts(&next_state, &report, target_root)?;
                let module_invalidations = next_state
                    .snapshot
                    .module_invalidations_since(&base_snapshot);
                let query_invalidations = next_state
                    .snapshot
                    .query_invalidations_since(&base_snapshot)
                    .into_iter()
                    .filter(|invalidation| !invalidation.reason().is_reusable())
                    .count();
                let compile_unit_hits = next_state
                    .compiled
                    .compile_units()
                    .iter()
                    .filter(|unit| unit.cache_status().is_hit())
                    .count();
                let patch_artifact = build_patch_bundle_artifact_from_awfb_bytes(
                    &base_bytes,
                    &artifacts.bundle_bytes,
                )?;
                let package = next_state
                    .loaded
                    .sources()
                    .manifest()
                    .package()
                    .name()
                    .as_str();
                let patch_output =
                    write_project_build_patch_artifact(target_root, package, &patch_artifact)?;
                println!(
                    "watch: patch {} ({} operation(s), compatibility={}, modules_changed={}, queries_invalidated={}, compile_unit_hits={}, cache_records={})",
                    patch_output.display(),
                    patch_artifact.plan.operations.len(),
                    patch_artifact.manifest.compatibility.label(),
                    module_invalidations.len(),
                    query_invalidations,
                    compile_unit_hits,
                    artifacts.cache_records.len()
                );
                base_bytes = artifacts.bundle_bytes;
                base_snapshot = next_state.snapshot.clone();
                selection = next_state.selection;
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
    let mut compile_cache = NoProjectCompileCache;
    compile_project_command_with_cache(profile, verification_mode, &mut compile_cache)
}

fn compile_project_command_with_cache<C>(
    profile: &ProfileOptions,
    verification_mode: VerificationMode,
    compile_cache: &mut C,
) -> Result<ProjectCommandState, ExitCode>
where
    C: ProjectCompileCache,
{
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
        compile_project_with_cache(loaded.sources(), &env, &runtime_options, compile_cache)
            .map_err(|error| {
                print_project_compile_error(&error);
                ExitCode::FAILURE
            })?;
    let mut verification = verify_module_with_env(
        compiled.linked_hir(),
        &env,
        VerificationPolicy {
            mode: verification_mode,
            backend: BackendKind::Emit,
        },
    );
    append_release_dynamic_goto_diagnostics(&mut verification, &compiled, verification_mode);
    let snapshot = snapshot_compiled_project(
        loaded.sources(),
        &compiled,
        BuildSnapshotRequest {
            build_id: project_build_id(&loaded, &compiled),
            compiler_build_id: env!("CARGO_PKG_VERSION").to_owned(),
            target_triple: format!("{}-{}", env::consts::ARCH, env::consts::OS),
            target_features: Vec::new(),
            profile: selection.profile().map_or_else(
                || "default".to_owned(),
                |profile| profile.id().as_str().to_owned(),
            ),
            selected_entries: selected_snapshot_entries(&selection),
        },
    );
    Ok(ProjectCommandState {
        loaded,
        selection,
        compiled,
        verification,
        snapshot,
    })
}

fn project_build_id(loaded: &LoadedProject, compiled: &CompiledProject) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
        loaded
            .sources()
            .manifest()
            .package()
            .name()
            .as_str()
            .as_bytes(),
    );
    for unit in compiled.compile_units() {
        bytes.extend_from_slice(&unit.fingerprint().as_bytes());
    }
    blake3::hash(&bytes).to_hex().to_string()
}

fn selected_snapshot_entries(selection: &SourceSelection) -> Vec<String> {
    selection.profile().map_or_else(
        || vec![selection.path().display().to_string()],
        |profile| vec![profile.id().as_str().to_owned()],
    )
}

fn append_release_dynamic_goto_diagnostics(
    verification: &mut VerificationReport,
    compiled: &CompiledProject,
    verification_mode: VerificationMode,
) {
    if verification_mode != VerificationMode::Release {
        return;
    }
    let Ok(index) = project_semantic_index_from_hir(
        compiled.linked_hir(),
        ProgramHash::new("project.release"),
        &SourceName::path("project.arcw"),
    ) else {
        verification.diagnostics.push(VerificationDiagnostic {
            id: "diagnostic.release.dynamic_control_index".to_owned(),
            severity: Severity::Error,
            message: "release build could not index project control-flow shape".to_owned(),
            source: None,
            obligation: None,
            related_ids: Vec::new(),
            actions: Vec::new(),
        });
        return;
    };
    verification.diagnostics.extend(
        index
            .flow_control_summaries()
            .iter()
            .filter(|(_, summary)| summary.dynamic_goto_count() > 0)
            .map(|(flow, summary)| VerificationDiagnostic {
                id: format!("diagnostic.release.dynamic_goto.{}", flow.as_str()),
                severity: Severity::Error,
                message: format!(
                    "release build rejects {} dynamic goto(s) in flow `{}`; use static flow references or a finite manifest-backed set",
                    summary.dynamic_goto_count(),
                    flow.as_str()
                ),
                source: None,
                obligation: None,
                related_ids: vec![flow.as_str().to_owned()],
                actions: Vec::new(),
            }),
    );
    verification
        .diagnostics
        .sort_by(|left, right| left.id.cmp(&right.id));
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
    use super::{CompileEmit, ProfileOptions, compile_project_command};
    use arcweft_verify::VerificationMode;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

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

    #[test]
    fn release_project_diagnostics_reject_dynamic_goto() {
        let root = temp_project_root("release-dynamic-goto");
        let source_root = root.join("src");
        fs::create_dir_all(&source_root).expect("source dir creates");
        fs::write(
            root.join("arcw.toml"),
            r#"
[package]
name = "release_dynamic_goto"
"#,
        )
        .expect("manifest writes");
        fs::write(
            source_root.join("main.arcw"),
            r#"
entry game {
    start(@flow.opening)
}

flow opening {
    let route = @flow.done
    goto route
}

flow done {
    return "done"
}
"#,
        )
        .expect("source writes");
        let profile = ProfileOptions {
            profile: None,
            manifest: root.join("arcw.toml"),
        };

        let dev =
            compile_project_command(&profile, VerificationMode::Dev).expect("dev project compiles");
        assert!(
            !dev.verification
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.id.starts_with("diagnostic.release.dynamic_goto") })
        );
        let release = compile_project_command(&profile, VerificationMode::Release)
            .expect("release project compiles before release diagnostics");

        assert!(release.verification.has_errors());
        assert!(release.verification.diagnostics.iter().any(|diagnostic| {
            diagnostic.id.starts_with("diagnostic.release.dynamic_goto")
                && diagnostic.message.contains("flow.opening")
        }));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_project_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "arcweft-project-command-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }
}
