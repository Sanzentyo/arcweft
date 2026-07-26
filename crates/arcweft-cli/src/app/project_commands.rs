//! Cargo-like project commands and the rustc-like single-source compiler route.

use super::bundle::{
    build_patch_bundle_artifact_from_awfb_bytes, compile_bundle_for_selection,
    write_bundle_artifact, write_patch_bundle_artifact,
};
use super::diagnostics::emit_diagnostics;
use super::progress::{CliProgress, CliProgressStatus};
use super::project::{
    ProfileOptions, SourceSelection, load_and_check_selection, print_project_compile_error,
    project_compilation_context, resolve_source_selection, runtime_plan_options_for_selection,
    semantic_context_for_selection,
};
use super::runtime::run::watch_inputs;
use super::shared::print_json;
use arcweft_bundle::{
    BundleFormat, BundleVirtualFileSpace,
    container::{BundleSectionKind, BundleView, ReadBudget, SectionDescriptor},
    patch::BundlePatchArtifact,
    patch::encode_patch_bundle,
};
use arcweft_compiler::{
    incremental::{BuildSnapshotRequest, snapshot_compiled_project},
    persistent::{
        ActualBytecodeUnitFactsInput, ActualLinkPlanFactsInput, BytecodeUnitFactsInput,
        HirBodyFactsInput, InterfaceSummaryFactsInput, LinkPlanFactsInput, ParsedSyntaxFactsInput,
        PersistentFactsError, TypecheckGateFactsInput, actual_bytecode_unit_payload,
        actual_link_plan_payload, bytecode_unit_payload, hir_body_object, hir_body_payload,
        interface_summary_object, interface_summary_payload, link_plan_payload,
        parsed_syntax_payload, typecheck_gate_object, typecheck_gate_payload,
    },
    project::{
        CompiledProject, CompiledProjectModule, InMemoryProjectCompileCache, NoProjectCompileCache,
        ProjectCompileCache, ProjectCompileCacheStatus, compile_project_with_cache,
    },
};
use arcweft_lang_sema::project_index::{ProgramHash, project_semantic_index_from_checked_project};
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_lang_syntax::source::ParsedSource;
use arcweft_project::{
    artifact::{ArtifactKey, ArtifactKeyInput, ArtifactKind},
    fingerprint::{BuildDigest, NamedDigest},
    incremental::{BuildSnapshot, CacheRecordStatus, InvalidationReason, QueryKind, QuerySnapshot},
    persistent_object::{
        AwboEnvelope, BytecodeLinkConservativeReason, BytecodeLinkIdentityOwner,
        BytecodeLinkProducerFamily, BytecodeUnitObject, CompilerBuildIdentity, CompilerObjectKey,
        CompilerObjectKind, CompilerObjectPayload, HirBodyObject, InterfaceSummaryObject,
        LinkDescriptorObject,
    },
    sources::ProjectSourceFile,
};
use arcweft_project_loader::cache::{
    persistent_query::{
        PersistentQueryReadOutcome, PersistentQueryReadRequest, PersistentQueryWriteRequest,
    },
    store::FilesystemCacheStore,
};
use arcweft_project_loader::project::{LoadedProject, ProjectLoadError};
use arcweft_source::SourceDocument;
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
    sync::Arc,
    thread,
    time::Duration,
};

/// Project-wide `arcw check` options.
#[derive(Args, Clone, Debug)]
pub(in crate::app) struct ProjectCheckOptions {
    #[command(flatten)]
    profile: ProfileOptions,
    path: Option<PathBuf>,
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
    source_document: Arc<SourceDocument>,
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
    snapshot: BuildSnapshot,
}

#[derive(Clone, Debug, Serialize)]
struct ProjectBuildCacheRecordReport {
    query: QueryKind,
    artifact_kind: ArtifactKind,
    logical_item: String,
    status: &'static str,
    key: String,
    object_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reuse_evidence: Option<ProjectBuildPersistentReuseEvidence>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum ProjectBuildPersistentReuseEvidence {
    ActualReusable {
        producer_family: BytecodeLinkProducerFamily,
        identity_owner: BytecodeLinkIdentityOwner,
        identity: String,
    },
    Conservative {
        producer_family: BytecodeLinkProducerFamily,
        identity_owner: BytecodeLinkIdentityOwner,
        reason: BytecodeLinkConservativeReason,
        missing_identity: &'static str,
        consumer_boundary: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        follow_up_sequence: Option<&'static str>,
    },
}

impl ProjectBuildPersistentReuseEvidence {
    const fn is_actual_reusable(&self) -> bool {
        matches!(self, Self::ActualReusable { .. })
    }
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

struct PersistentQueryWriteThroughResult {
    queries: Vec<QuerySnapshot>,
    reports: Vec<ProjectBuildCacheRecordReport>,
}

struct PersistentQueryWriteContext<'a> {
    state: &'a ProjectCommandState,
    snapshot: &'a BuildSnapshot,
    cache_root: &'a Path,
    store: &'a FilesystemCacheStore,
    incremental: bool,
    persistent_artifacts: &'a FullBuildPersistentArtifactContext,
}

struct ProjectBuildBundleOutput {
    bundle_bytes: Vec<u8>,
    bundle_cache_hit: bool,
    persistent_artifacts: FullBuildPersistentArtifactContext,
}

#[derive(Clone, Debug)]
struct FullBuildPersistentArtifactContext {
    bytecode_units: Vec<FullBuildBytecodeUnitArtifact>,
    link_plan: Option<FullBuildLinkPlanArtifact>,
    conservative_reason: Option<FullBuildConservativeReason>,
}

#[derive(Clone, Debug)]
struct FullBuildBytecodeUnitArtifact {
    module: String,
    runtime_plan_unit_digest: BuildDigest,
    canonical_awbc_bytes: Vec<u8>,
    awbc_schema_digest: BuildDigest,
    verifier_policy_digest: BuildDigest,
    codegen_policy_digest: BuildDigest,
    relocation_import_table_digest: BuildDigest,
}

#[derive(Clone, Debug)]
struct FullBuildLinkPlanArtifact {
    module: String,
    package: String,
    ordered_unit_identities: Vec<NamedDigest>,
    entrypoint_digest: BuildDigest,
    resource_section_digest: BuildDigest,
    adapter_requirements_digest: BuildDigest,
    patch_compatibility_digest: BuildDigest,
    product_build_options_digest: BuildDigest,
}

#[derive(Clone, Debug)]
struct FullBuildConservativeReason {
    bytecode: BytecodeLinkConservativeReason,
    link: BytecodeLinkConservativeReason,
}

struct PersistentQueryWriteItem {
    query: QueryKind,
    artifact_key: ArtifactKey,
    object_key: CompilerObjectKey,
    logical_item: String,
    payload: CompilerObjectPayload,
    object_digest: BuildDigest,
    expected_link_descriptor: Option<LinkDescriptorObject>,
    reuse_evidence: Option<ProjectBuildPersistentReuseEvidence>,
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
        let failed = state.verification.has_blocking_runtime_safety_gaps();
        let sources = state.loaded.sources();
        Self {
            status: if failed { "failed" } else { "ok" },
            package: sources.package().id.as_str().to_owned(),
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
                    source_hash: source.source_revision().to_hex(),
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
            flows: state
                .compiled
                .hir_project()
                .modules()
                .map(|(_, module)| module.flows().len())
                .sum(),
            line_task_groups: state.compiled.line_task_groups().len(),
            verifier_diagnostics: state.verification.diagnostics.len(),
            obligations: state.verification.obligations.len(),
            unsafe_audits: state.verification.unsafe_audit_count(),
        }
    }
}

pub(super) fn project_check_command(options: &ProjectCheckOptions) -> Result<(), ExitCode> {
    if let Some(path) = &options.path {
        if options.profile.profile.is_some() {
            eprintln!("error: source path and --profile cannot be used together");
            return Err(ExitCode::from(2));
        }
        return compile_command(&CompileOptions {
            input: path.clone(),
            emit: CompileEmit::Check,
            output: None,
            json: options.json,
        });
    }

    let progress = CliProgress::new(!options.json);
    let state = progress.run(CliProgressStatus::Checking, "project", || {
        compile_project_command(&options.profile, VerificationMode::Dev)
    })?;
    let report = ProjectCommandReport::from_state(&state);
    if options.json {
        print_json(&report)?;
    } else {
        emit_verification_diagnostics(&state.source_document, &state.verification);
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
    let progress = CliProgress::new(!options.json);
    let state = compile_project_build_state(options, mode, &mut compile_cache, progress)?;
    let report = ProjectCommandReport::from_state(&state);
    if report.status != "ok" {
        emit_verification_diagnostics(&state.source_document, &state.verification);
        if options.json {
            print_json(&report)?;
        }
        return Err(ExitCode::FAILURE);
    }

    let target_root = project_build_target_root(options, &state, mode);
    let artifacts = progress.run(
        CliProgressStatus::Writing,
        format!("{} artifacts", mode.directory()),
        || write_project_build_artifacts(&state, &report, &target_root),
    )?;
    let mut patch_artifacts = Vec::new();
    if let Some(base) = options.patch_base.as_ref() {
        patch_artifacts.push(
            progress.run(CliProgressStatus::Writing, "patch artifact", || {
                write_project_build_patch_from_base(
                    base,
                    &artifacts.bundle_bytes,
                    &target_root,
                    state.loaded.sources().package().id.as_str(),
                )
            })?,
        );
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
            artifacts.snapshot.clone(),
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

fn compile_project_build_state(
    options: &ProjectBuildOptions,
    mode: ProjectBuildMode,
    compile_cache: &mut InMemoryProjectCompileCache,
    progress: CliProgress,
) -> Result<ProjectCommandState, ExitCode> {
    progress.run(
        CliProgressStatus::Building,
        format!("{} project", mode.directory()),
        || {
            if options.watch {
                compile_project_command_with_cache(
                    &options.profile,
                    mode.verification_mode(),
                    compile_cache,
                )
            } else {
                compile_project_command(&options.profile, mode.verification_mode())
            }
        },
    )
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
    let package = state.loaded.sources().package().id.as_str();
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
    let ProjectBuildBundleOutput {
        bundle_bytes,
        bundle_cache_hit,
        persistent_artifacts,
    } = read_cached_project_bundle(&cache_root, bundle_key).map_or_else(
        || compile_project_bundle_output(state, &plan_bytes),
        |bytes| project_build_bundle_output_from_bytes(state, &plan_bytes, bytes, true),
    )?;
    let mut phases = Vec::new();
    write_bundle_artifact(&bundle_path, bundle_bytes.clone(), &mut phases)?;
    let content_root = awfb_content_root_digest(&bundle_bytes)?;
    let mut snapshot = state.snapshot.clone().with_content_root(content_root);
    let persistent_write_through =
        store_persistent_query_write_through(state, &snapshot, &cache_root, &persistent_artifacts)?;
    snapshot = snapshot.with_additional_queries(persistent_write_through.queries);
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
    let mut cache_records = persistent_write_through.reports;
    cache_records.extend(store_project_build_cache_artifacts(&cache_inputs)?);
    Ok(ProjectBuildArtifacts {
        bundle_path,
        metadata_path,
        plan_path,
        snapshot_path,
        cache_root,
        cache_records,
        bundle_bytes,
        snapshot,
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

fn compile_project_bundle_output(
    state: &ProjectCommandState,
    plan_bytes: &[u8],
) -> Result<ProjectBuildBundleOutput, ExitCode> {
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
    project_build_bundle_output_from_bytes(state, plan_bytes, bundle_bytes, false)
}

fn project_build_bundle_output_from_bytes(
    state: &ProjectCommandState,
    plan_bytes: &[u8],
    bundle_bytes: Vec<u8>,
    bundle_cache_hit: bool,
) -> Result<ProjectBuildBundleOutput, ExitCode> {
    let persistent_artifacts =
        FullBuildPersistentArtifactContext::from_build_output(state, plan_bytes, &bundle_bytes)?;
    Ok(ProjectBuildBundleOutput {
        bundle_bytes,
        bundle_cache_hit,
        persistent_artifacts,
    })
}

impl FullBuildPersistentArtifactContext {
    fn from_build_output(
        state: &ProjectCommandState,
        plan_bytes: &[u8],
        bundle_bytes: &[u8],
    ) -> Result<Self, ExitCode> {
        let view = BundleView::parse(bundle_bytes, ReadBudget::default()).map_err(|error| {
            eprintln!("error: failed to inspect generated AWFB for persistent artifacts: {error}");
            ExitCode::FAILURE
        })?;
        let canonical_awbc_bytes = project_product_awbc_bytes_from_view(&view)?;
        let product_awbc_descriptor =
            required_bundle_section_descriptor(&view, BundleSectionKind::ProgramBytecode)?;
        let adapter_requirements_descriptor =
            required_bundle_section_descriptor(&view, BundleSectionKind::AdapterRequirements)?;
        let modules = state.compiled.modules();
        let single_project_unit = modules.len() == 1
            && state.compiled.compile_units().len() == 1
            && state
                .compiled
                .compile_units()
                .first()
                .is_some_and(|unit| unit.modules().len() == 1);
        if !single_project_unit {
            return Ok(Self {
                bytecode_units: Vec::new(),
                link_plan: None,
                conservative_reason: Some(FullBuildConservativeReason {
                    bytecode:
                        BytecodeLinkConservativeReason::FullBuildMultiModuleProductAwbcNotNarrowed,
                    link:
                        BytecodeLinkConservativeReason::FullBuildMultiModuleProductAwbcNotNarrowed,
                }),
            });
        }

        let module = modules
            .first()
            .expect("single_project_unit has one module")
            .module()
            .to_string();
        let package = state.loaded.sources().package().id.as_str().to_owned();
        let bytecode_unit = FullBuildBytecodeUnitArtifact {
            module: module.clone(),
            runtime_plan_unit_digest: BuildDigest::of(plan_bytes),
            canonical_awbc_bytes,
            awbc_schema_digest: bundle_section_schema_digest(product_awbc_descriptor),
            verifier_policy_digest: full_build_bytecode_verifier_policy_digest(),
            codegen_policy_digest: full_build_codegen_policy_digest(product_awbc_descriptor),
            relocation_import_table_digest: bundle_section_identity_digest(
                adapter_requirements_descriptor,
            ),
        };
        let unit_identity = bytecode_unit.unit_identity_digest();
        let link_plan = FullBuildLinkPlanArtifact {
            module: module.clone(),
            package,
            ordered_unit_identities: vec![NamedDigest::new(module, unit_identity)],
            entrypoint_digest: bundle_section_identity_digest(required_bundle_section_descriptor(
                &view,
                BundleSectionKind::Entrypoints,
            )?),
            resource_section_digest: full_build_resource_section_digest(&view),
            adapter_requirements_digest: bundle_section_identity_digest(
                adapter_requirements_descriptor,
            ),
            patch_compatibility_digest: full_build_patch_compatibility_digest(&view),
            product_build_options_digest: full_build_product_options_digest(state),
        };
        Ok(Self {
            bytecode_units: vec![bytecode_unit],
            link_plan: Some(link_plan),
            conservative_reason: None,
        })
    }

    fn bytecode_unit(&self, module: &str) -> Option<&FullBuildBytecodeUnitArtifact> {
        self.bytecode_units
            .iter()
            .find(|unit| unit.module == module)
    }

    fn link_plan(&self, module: &str) -> Option<&FullBuildLinkPlanArtifact> {
        self.link_plan.as_ref().filter(|plan| plan.module == module)
    }

    fn expected_link_descriptor(
        &self,
        kind: CompilerObjectKind,
        module: &str,
        object_key: &CompilerObjectKey,
    ) -> Option<LinkDescriptorObject> {
        let plan = (kind == CompilerObjectKind::LinkPlan)
            .then(|| self.link_plan(module))
            .flatten()?;
        Some(plan.descriptor(object_key.stage_inputs().dependency_body_digest_root()))
    }

    fn reuse_evidence(
        &self,
        kind: CompilerObjectKind,
        module: &str,
    ) -> Option<ProjectBuildPersistentReuseEvidence> {
        match kind {
            CompilerObjectKind::BytecodeUnit => self.bytecode_unit(module).map_or_else(
                || {
                    self.conservative_reason.as_ref().map(|reason| {
                        ProjectBuildPersistentReuseEvidence::Conservative {
                            producer_family: BytecodeLinkProducerFamily::FullBuild,
                            identity_owner:
                                BytecodeLinkIdentityOwner::FullBuildPersistentArtifactContext,
                            reason: reason.bytecode,
                            missing_identity: reason.bytecode.missing_identity(),
                            consumer_boundary: reason.bytecode.consumer_boundary(),
                            follow_up_sequence: reason.bytecode.follow_up_sequence(),
                        }
                    })
                },
                |unit| {
                    Some(ProjectBuildPersistentReuseEvidence::ActualReusable {
                        producer_family: BytecodeLinkProducerFamily::FullBuild,
                        identity_owner: BytecodeLinkIdentityOwner::FullBuildBytecodeUnitArtifact,
                        identity: unit.unit_identity_digest().to_hex(),
                    })
                },
            ),
            CompilerObjectKind::LinkPlan => self.link_plan(module).map_or_else(
                || {
                    self.conservative_reason.as_ref().map(|reason| {
                        ProjectBuildPersistentReuseEvidence::Conservative {
                            producer_family: BytecodeLinkProducerFamily::FullBuild,
                            identity_owner:
                                BytecodeLinkIdentityOwner::FullBuildPersistentArtifactContext,
                            reason: reason.link,
                            missing_identity: reason.link.missing_identity(),
                            consumer_boundary: reason.link.consumer_boundary(),
                            follow_up_sequence: reason.link.follow_up_sequence(),
                        }
                    })
                },
                |plan| {
                    Some(ProjectBuildPersistentReuseEvidence::ActualReusable {
                        producer_family: BytecodeLinkProducerFamily::FullBuild,
                        identity_owner: BytecodeLinkIdentityOwner::FullBuildLinkPlanArtifact,
                        identity: plan.link_identity_digest().to_hex(),
                    })
                },
            ),
            CompilerObjectKind::ParsedSyntax
            | CompilerObjectKind::InterfaceSummary
            | CompilerObjectKind::HirBody
            | CompilerObjectKind::TypecheckGate
            | CompilerObjectKind::LineTaskEvidence
            | CompilerObjectKind::RuntimePlanUnit => None,
        }
    }
}

impl FullBuildBytecodeUnitArtifact {
    fn unit_identity_digest(&self) -> BuildDigest {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"arcweft-full-build-bytecode-unit-identity-v1\0");
        extend_build_digest(&mut bytes, self.runtime_plan_unit_digest);
        extend_build_digest(&mut bytes, BuildDigest::of(&self.canonical_awbc_bytes));
        extend_build_digest(&mut bytes, self.awbc_schema_digest);
        extend_build_digest(&mut bytes, self.verifier_policy_digest);
        extend_build_digest(&mut bytes, self.codegen_policy_digest);
        extend_build_digest(&mut bytes, self.relocation_import_table_digest);
        BuildDigest::of(&bytes)
    }
}

impl FullBuildLinkPlanArtifact {
    fn descriptor(&self, dependency_body_digest_root: BuildDigest) -> LinkDescriptorObject {
        LinkDescriptorObject {
            ordered_unit_identities: self.ordered_unit_identities.clone(),
            entrypoint_digest: self.entrypoint_digest,
            resource_section_digest: self.resource_section_digest,
            adapter_requirements_digest: self.adapter_requirements_digest,
            patch_compatibility_digest: self.patch_compatibility_digest,
            product_build_options_digest: self.product_build_options_digest,
            dependency_body_digest_root,
        }
    }

    fn link_identity_digest(&self) -> BuildDigest {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"arcweft-full-build-link-plan-identity-v1\0");
        for unit in &self.ordered_unit_identities {
            bytes.extend_from_slice(unit.name().as_bytes());
            bytes.push(0);
            extend_build_digest(&mut bytes, unit.digest());
        }
        extend_build_digest(&mut bytes, self.entrypoint_digest);
        extend_build_digest(&mut bytes, self.resource_section_digest);
        extend_build_digest(&mut bytes, self.adapter_requirements_digest);
        extend_build_digest(&mut bytes, self.patch_compatibility_digest);
        extend_build_digest(&mut bytes, self.product_build_options_digest);
        BuildDigest::of(&bytes)
    }
}

fn project_product_awbc_bytes_from_view(view: &BundleView<'_>) -> Result<Vec<u8>, ExitCode> {
    let descriptor = required_bundle_section_descriptor(view, BundleSectionKind::ProgramBytecode)?;
    view.decoded_section(descriptor.id())
        .map_err(|error| {
            eprintln!("error: failed to decode generated AWFB ProgramBytecode section: {error}");
            ExitCode::FAILURE
        })?
        .ok_or_else(|| {
            eprintln!("error: generated AWFB ProgramBytecode section is not embedded");
            ExitCode::FAILURE
        })
}

fn required_bundle_section_descriptor<'a>(
    view: &'a BundleView<'_>,
    kind: BundleSectionKind,
) -> Result<&'a SectionDescriptor, ExitCode> {
    let mut matches = view
        .sections()
        .iter()
        .filter(|descriptor| descriptor.known_kind() == Some(kind));
    let Some(descriptor) = matches.next() else {
        eprintln!("error: generated AWFB is missing required {kind:?} section");
        return Err(ExitCode::FAILURE);
    };
    if matches.next().is_some() {
        eprintln!("error: generated AWFB contains multiple {kind:?} sections");
        return Err(ExitCode::FAILURE);
    }
    Ok(descriptor)
}

fn bundle_section_schema_digest(descriptor: &SectionDescriptor) -> BuildDigest {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"arcweft-awfb-product-awbc-section-schema-v1\0");
    if let Some(kind) = descriptor.known_kind() {
        bytes.extend_from_slice(&kind.encoded().to_le_bytes());
    }
    bytes.extend_from_slice(&descriptor.schema_version().to_le_bytes());
    BuildDigest::of(&bytes)
}

fn bundle_section_identity_digest(descriptor: &SectionDescriptor) -> BuildDigest {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"arcweft-awfb-section-identity-v1\0");
    bytes.extend_from_slice(&descriptor.kind_code().encoded().to_le_bytes());
    bytes.extend_from_slice(&descriptor.schema_version().to_le_bytes());
    bytes.extend_from_slice(&descriptor.decoded_size().to_le_bytes());
    bytes.extend_from_slice(&descriptor.content_digest().as_bytes());
    bytes.push(u8::from(descriptor.required()));
    bytes.push(descriptor.residency().encoded());
    BuildDigest::of(&bytes)
}

fn full_build_resource_section_digest(view: &BundleView<'_>) -> BuildDigest {
    let mut descriptors = view
        .sections()
        .iter()
        .filter(|descriptor| {
            !matches!(
                descriptor.known_kind(),
                Some(
                    BundleSectionKind::ProgramBytecode
                        | BundleSectionKind::Entrypoints
                        | BundleSectionKind::AdapterRequirements
                )
            )
        })
        .collect::<Vec<_>>();
    descriptors.sort_by_key(|descriptor| descriptor.id());
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"arcweft-full-build-resource-sections-v1\0");
    for descriptor in descriptors {
        bytes.extend_from_slice(&descriptor.id().as_bytes());
        bytes.extend_from_slice(&bundle_section_identity_digest(descriptor).as_bytes());
    }
    BuildDigest::of(&bytes)
}

fn full_build_patch_compatibility_digest(view: &BundleView<'_>) -> BuildDigest {
    let mut descriptors = view.sections().iter().collect::<Vec<_>>();
    descriptors.sort_by_key(|descriptor| descriptor.id());
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"arcweft-full-build-patch-compatibility-v1\0");
    for descriptor in descriptors {
        bytes.extend_from_slice(&descriptor.id().as_bytes());
        bytes.extend_from_slice(&descriptor.kind_code().encoded().to_le_bytes());
        if let Some(kind) = descriptor.known_kind() {
            bytes.extend_from_slice(format!("{:?}", kind.patch_default_compatibility()).as_bytes());
        }
        bytes.push(0);
        bytes.extend_from_slice(&descriptor.content_digest().as_bytes());
    }
    BuildDigest::of(&bytes)
}

fn full_build_bytecode_verifier_policy_digest() -> BuildDigest {
    BuildDigest::of(
        b"arcweft-full-build-bytecode-verifier-policy-v1:awbc-read-through:require_entrypoint=false",
    )
}

fn full_build_codegen_policy_digest(descriptor: &SectionDescriptor) -> BuildDigest {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"arcweft-full-build-codegen-policy-v1\0");
    bytes.extend_from_slice(b"product-awbc-lowerer-default\0");
    bytes.extend_from_slice(&bundle_section_schema_digest(descriptor).as_bytes());
    BuildDigest::of(&bytes)
}

fn full_build_product_options_digest(state: &ProjectCommandState) -> BuildDigest {
    let project = state.snapshot.project();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"arcweft-full-build-product-options-v1\0");
    bytes.extend_from_slice(project.package().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(project.profile().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(project.target_triple().as_bytes());
    bytes.push(0);
    for feature in project.target_features() {
        bytes.extend_from_slice(feature.as_bytes());
        bytes.push(0);
    }
    bytes.extend_from_slice(b"bundle-format=awfb\0include-space=asset\0");
    extend_build_digest(&mut bytes, project.adapter_environment_digest());
    extend_build_digest(&mut bytes, project.launch_profile_digest());
    extend_build_digest(&mut bytes, project.declared_environment_digest());
    if let Some(profile) = state.selection.profile() {
        bytes.extend_from_slice(b"selection=profile\0");
        bytes.extend_from_slice(profile.id().as_str().as_bytes());
    } else {
        bytes.extend_from_slice(b"selection=project-default\0");
    }
    BuildDigest::of(&bytes)
}

fn persistent_feature_set_digest(features: &[String]) -> BuildDigest {
    let mut features = features.to_vec();
    features.sort();
    features.dedup();
    BuildDigest::of(features.join("\0").as_bytes())
}

fn extend_build_digest(bytes: &mut Vec<u8>, digest: BuildDigest) {
    bytes.extend_from_slice(&digest.as_bytes());
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
    let package = inputs.state.loaded.sources().package().id.as_str();
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
            reuse_evidence: None,
        })
    })
    .collect()
}

fn store_persistent_query_write_through(
    state: &ProjectCommandState,
    snapshot: &BuildSnapshot,
    cache_root: &Path,
    persistent_artifacts: &FullBuildPersistentArtifactContext,
) -> Result<PersistentQueryWriteThroughResult, ExitCode> {
    let sources = state.loaded.sources();
    let package = sources.package().id.as_str();
    let store = FilesystemCacheStore::new(cache_root);
    let _lock = store.lock_package(package).map_err(|error| {
        eprintln!(
            "error: failed to acquire persistent query cache lock under {}: {error}",
            cache_root.display()
        );
        ExitCode::FAILURE
    })?;
    let incremental = sources.build().incremental;
    let mut result = PersistentQueryWriteThroughResult {
        queries: Vec::new(),
        reports: Vec::new(),
    };
    let context = PersistentQueryWriteContext {
        state,
        snapshot,
        cache_root,
        store: &store,
        incremental,
        persistent_artifacts,
    };

    for source in sources.modules() {
        write_persistent_query_source(&context, source, &mut result)?;
    }

    Ok(result)
}

fn write_persistent_query_source(
    context: &PersistentQueryWriteContext<'_>,
    source: &ProjectSourceFile,
    result: &mut PersistentQueryWriteThroughResult,
) -> Result<(), ExitCode> {
    let compiled = context
        .state
        .compiled
        .modules()
        .iter()
        .find(|module| module.module() == source.module())
        .expect("compiled project contains every loaded source module");
    let parsed = parse_document_with_source(Arc::clone(source.document()), ParseOptions::default());
    if !parsed.errors().is_empty() {
        eprintln!(
            "error: cannot persist parse facts for {} after a successful build: parser returned {} error(s)",
            source.path().display(),
            parsed.errors().len()
        );
        return Err(ExitCode::FAILURE);
    }
    let unit_cache_status = module_compile_cache_status(context.state, source);
    for kind in [
        CompilerObjectKind::ParsedSyntax,
        CompilerObjectKind::InterfaceSummary,
        CompilerObjectKind::HirBody,
        CompilerObjectKind::TypecheckGate,
        CompilerObjectKind::BytecodeUnit,
        CompilerObjectKind::LinkPlan,
    ] {
        if let Some(item) = persistent_query_write_item(
            context.state,
            context.snapshot,
            source,
            compiled,
            &parsed,
            context.persistent_artifacts,
            kind,
        )? {
            result.queries.push(commit_persistent_query_item(
                context.store,
                context.cache_root,
                context.incremental,
                unit_cache_status,
                item,
                &mut result.reports,
            )?);
        }
    }
    Ok(())
}

fn persistent_query_write_item(
    state: &ProjectCommandState,
    snapshot: &BuildSnapshot,
    source: &ProjectSourceFile,
    compiled: &CompiledProjectModule,
    parsed: &ParsedSource,
    persistent_artifacts: &FullBuildPersistentArtifactContext,
    kind: CompilerObjectKind,
) -> Result<Option<PersistentQueryWriteItem>, ExitCode> {
    let Some(query) = kind.safe_read_through_query_kind() else {
        return Ok(None);
    };
    let logical_item = persistent_query_logical_item(kind, source);
    let dependency_interface_digests = if matches!(
        kind,
        CompilerObjectKind::InterfaceSummary
            | CompilerObjectKind::HirBody
            | CompilerObjectKind::TypecheckGate
            | CompilerObjectKind::BytecodeUnit
            | CompilerObjectKind::LinkPlan
    ) {
        dependency_interface_digests(state, source)
    } else {
        Vec::new()
    };
    let dependency_body_digests = if matches!(
        kind,
        CompilerObjectKind::BytecodeUnit | CompilerObjectKind::LinkPlan
    ) {
        dependency_body_digests(state, source)
    } else {
        Vec::new()
    };
    let query_options_digest = persistent_query_options_digest(query);
    let object_key = CompilerObjectKey {
        kind,
        compiler: persistent_compiler_identity(snapshot),
        source_digest: BuildDigest::from(source.source_revision()),
        query_options_digest,
        dependency_interface_digests: dependency_interface_digests.clone(),
        dependency_body_digests: dependency_body_digests.clone(),
        environment_digest: snapshot.project().adapter_environment_digest(),
    }
    .canonicalized();
    let artifact_key = persistent_query_artifact_key(
        snapshot,
        query,
        &logical_item,
        object_key.source_digest,
        dependency_interface_digests,
        dependency_body_digests,
        query_options_digest,
    );
    let payload = persistent_query_payload(
        kind,
        &object_key,
        source,
        compiled,
        parsed,
        persistent_artifacts,
    )?;
    let object_digest = persistent_query_object_digest(&object_key, payload.clone())?;
    let module_label = source.module().to_string();
    let expected_link_descriptor =
        persistent_artifacts.expected_link_descriptor(kind, &module_label, &object_key);
    let reuse_evidence = persistent_artifacts.reuse_evidence(kind, &module_label);
    Ok(Some(PersistentQueryWriteItem {
        query,
        artifact_key,
        object_key,
        logical_item,
        payload,
        object_digest,
        expected_link_descriptor,
        reuse_evidence,
    }))
}

fn persistent_query_payload(
    kind: CompilerObjectKind,
    object_key: &CompilerObjectKey,
    source: &ProjectSourceFile,
    compiled: &CompiledProjectModule,
    parsed: &ParsedSource,
    persistent_artifacts: &FullBuildPersistentArtifactContext,
) -> Result<CompilerObjectPayload, ExitCode> {
    let source_label = source.path().display().to_string();
    let module_label = source.module().to_string();
    match kind {
        CompilerObjectKind::ParsedSyntax => parsed_syntax_payload(&ParsedSyntaxFactsInput {
            key: object_key,
            source_label: &source_label,
            parsed,
        }),
        CompilerObjectKind::HirBody => hir_body_payload(&HirBodyFactsInput {
            key: object_key,
            module: &module_label,
            parsed,
            hir: compiled.hir(),
        }),
        CompilerObjectKind::InterfaceSummary => {
            interface_summary_payload(&InterfaceSummaryFactsInput {
                key: object_key,
                module: &module_label,
                parsed,
                hir: compiled.hir(),
            })
        }
        CompilerObjectKind::TypecheckGate => {
            typecheck_gate_payload_for_query(object_key, &module_label, compiled, parsed)
        }
        CompilerObjectKind::BytecodeUnit => bytecode_unit_payload_for_query(
            object_key,
            &module_label,
            compiled,
            parsed,
            persistent_artifacts,
        ),
        CompilerObjectKind::LinkPlan => {
            link_plan_payload_for_query(object_key, &module_label, parsed, persistent_artifacts)
        }
        CompilerObjectKind::LineTaskEvidence | CompilerObjectKind::RuntimePlanUnit => {
            unreachable!("safe query kind list is exhaustive")
        }
    }
    .map_err(|error| {
        eprintln!(
            "error: failed to build persistent query payload `{}`: {error}",
            persistent_query_logical_item(kind, source)
        );
        ExitCode::FAILURE
    })
}

fn typecheck_gate_payload_for_query(
    object_key: &CompilerObjectKey,
    module_label: &str,
    compiled: &CompiledProjectModule,
    parsed: &ParsedSource,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    let (interface_summary, hir_body) =
        interface_and_hir_sibling_objects(object_key, module_label, compiled, parsed)?;
    typecheck_gate_payload(&TypecheckGateFactsInput {
        key: object_key,
        module: module_label,
        parsed,
        interface_summary: &interface_summary,
        hir_body: &hir_body,
    })
}

fn bytecode_unit_payload_for_query(
    object_key: &CompilerObjectKey,
    module_label: &str,
    compiled: &CompiledProjectModule,
    parsed: &ParsedSource,
    persistent_artifacts: &FullBuildPersistentArtifactContext,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    let (interface_summary, hir_body) =
        interface_and_hir_sibling_objects(object_key, module_label, compiled, parsed)?;
    let typecheck_key = sibling_persistent_object_key(
        object_key,
        CompilerObjectKind::TypecheckGate,
        QueryKind::TypeCheck,
    );
    let typecheck_gate = typecheck_gate_object(&TypecheckGateFactsInput {
        key: &typecheck_key,
        module: module_label,
        parsed,
        interface_summary: &interface_summary,
        hir_body: &hir_body,
    })?;
    if let Some(actual) = persistent_artifacts.bytecode_unit(module_label) {
        return actual_bytecode_unit_payload(&ActualBytecodeUnitFactsInput {
            key: object_key,
            module: module_label,
            parsed,
            hir_body: &hir_body,
            typecheck_gate: &typecheck_gate,
            runtime_plan_unit_digest: actual.runtime_plan_unit_digest,
            canonical_awbc_bytes: &actual.canonical_awbc_bytes,
            awbc_schema_digest: actual.awbc_schema_digest,
            verifier_policy_digest: actual.verifier_policy_digest,
            codegen_policy_digest: actual.codegen_policy_digest,
            target_profile_digest: object_key.query_options_digest,
            feature_set_digest: persistent_feature_set_digest(
                &object_key.compiler.enabled_features,
            ),
            relocation_import_table_digest: actual.relocation_import_table_digest,
        });
    }
    bytecode_unit_payload(&BytecodeUnitFactsInput {
        key: object_key,
        module: module_label,
        parsed,
        hir_body: &hir_body,
        typecheck_gate: &typecheck_gate,
    })
}

fn link_plan_payload_for_query(
    object_key: &CompilerObjectKey,
    module_label: &str,
    parsed: &ParsedSource,
    persistent_artifacts: &FullBuildPersistentArtifactContext,
) -> Result<CompilerObjectPayload, PersistentFactsError> {
    if let Some(actual) = persistent_artifacts.link_plan(module_label) {
        return actual_link_plan_payload(&ActualLinkPlanFactsInput {
            key: object_key,
            package: &actual.package,
            parsed,
            ordered_unit_identities: actual.ordered_unit_identities.clone(),
            entrypoint_digest: actual.entrypoint_digest,
            resource_section_digest: actual.resource_section_digest,
            adapter_requirements_digest: actual.adapter_requirements_digest,
            patch_compatibility_digest: actual.patch_compatibility_digest,
            product_build_options_digest: actual.product_build_options_digest,
        });
    }
    link_plan_payload(&LinkPlanFactsInput {
        key: object_key,
        package: module_label,
        parsed,
        ordered_unit_digests: vec![NamedDigest::new(
            module_label.to_owned(),
            BytecodeUnitObject::conservative_canonical_bytecode_digest(),
        )],
        product_build_options_digest: persistent_query_options_digest(QueryKind::LinkPlan),
    })
}

fn interface_and_hir_sibling_objects(
    object_key: &CompilerObjectKey,
    module_label: &str,
    compiled: &CompiledProjectModule,
    parsed: &ParsedSource,
) -> Result<(InterfaceSummaryObject, HirBodyObject), PersistentFactsError> {
    let interface_key = sibling_persistent_object_key(
        object_key,
        CompilerObjectKind::InterfaceSummary,
        QueryKind::Interface,
    );
    let hir_key =
        sibling_persistent_object_key(object_key, CompilerObjectKind::HirBody, QueryKind::HirBody);
    let interface_summary = interface_summary_object(&InterfaceSummaryFactsInput {
        key: &interface_key,
        module: module_label,
        parsed,
        hir: compiled.hir(),
    })?;
    let hir_body = hir_body_object(&HirBodyFactsInput {
        key: &hir_key,
        module: module_label,
        parsed,
        hir: compiled.hir(),
    })?;
    Ok((interface_summary, hir_body))
}

fn sibling_persistent_object_key(
    source: &CompilerObjectKey,
    kind: CompilerObjectKind,
    query: QueryKind,
) -> CompilerObjectKey {
    CompilerObjectKey {
        kind,
        compiler: source.compiler.clone(),
        source_digest: source.source_digest,
        query_options_digest: persistent_query_options_digest(query),
        dependency_interface_digests: source.dependency_interface_digests.clone(),
        dependency_body_digests: source.dependency_body_digests.clone(),
        environment_digest: source.environment_digest,
    }
    .canonicalized()
}

fn commit_persistent_query_item(
    store: &FilesystemCacheStore,
    cache_root: &Path,
    incremental: bool,
    unit_cache_status: ProjectCompileCacheStatus,
    item: PersistentQueryWriteItem,
    reports: &mut Vec<ProjectBuildCacheRecordReport>,
) -> Result<QuerySnapshot, ExitCode> {
    let (status, written_digest) = if incremental {
        let mut read_request =
            PersistentQueryReadRequest::new(item.query, item.artifact_key, item.object_key.clone());
        if let Some(descriptor) = item.expected_link_descriptor.clone() {
            read_request = read_request.with_expected_link_descriptor(descriptor);
        }
        let read = store.read_persistent_query(&read_request);
        let status = persistent_query_status_after_read(
            &read,
            item.object_key.kind,
            unit_cache_status,
            item.reuse_evidence.as_ref(),
        );
        let receipt = store
            .write_persistent_query(&PersistentQueryWriteRequest::new(
                item.query,
                item.artifact_key,
                item.object_key,
                item.logical_item.clone(),
                item.payload,
            ))
            .map_err(|error| {
                eprintln!(
                    "error: failed to write persistent query object `{}` under {}: {error}",
                    item.logical_item,
                    cache_root.display()
                );
                ExitCode::FAILURE
            })?;
        reports.push(ProjectBuildCacheRecordReport {
            query: item.query,
            artifact_kind: receipt.artifact_kind,
            logical_item: item.logical_item,
            status: status.as_str(),
            key: item.artifact_key.to_string(),
            object_digest: receipt.object_digest.to_string(),
            reuse_evidence: item.reuse_evidence,
        });
        (status, receipt.object_digest)
    } else {
        (
            CacheRecordStatus::Rebuilt {
                reason: InvalidationReason::OptionsChanged,
            },
            item.object_digest,
        )
    };
    Ok(QuerySnapshot::new(
        item.query,
        item.artifact_key,
        written_digest,
        status,
    ))
}

fn persistent_query_status_after_read(
    read: &PersistentQueryReadOutcome,
    object_kind: CompilerObjectKind,
    unit_cache_status: ProjectCompileCacheStatus,
    reuse_evidence: Option<&ProjectBuildPersistentReuseEvidence>,
) -> CacheRecordStatus {
    let read_status = read.cache_record_status();
    if unit_cache_status.is_hit() && persistent_read_status_did_not_feed_build(&read_status) {
        return persistent_query_status_from_unit_cache_hit(object_kind, reuse_evidence);
    }

    match read_status {
        CacheRecordStatus::Hit
            if reuse_evidence
                .is_some_and(ProjectBuildPersistentReuseEvidence::is_actual_reusable) =>
        {
            CacheRecordStatus::Hit
        }
        CacheRecordStatus::Hit => {
            let policy = reuse_evidence
                .and_then(|evidence| match evidence {
                    ProjectBuildPersistentReuseEvidence::Conservative { reason, .. } => {
                        Some(reason.policy())
                    }
                    ProjectBuildPersistentReuseEvidence::ActualReusable { .. } => None,
                })
                .unwrap_or("full_build_shadow_validation_rebuilt_after_persistent_read_through");
            CacheRecordStatus::HitThenRebuilt {
                reason: InvalidationReason::ConservativeInvalidation {
                    policy: policy.to_owned(),
                },
            }
        }
        CacheRecordStatus::HitThenRebuilt { reason } => {
            CacheRecordStatus::HitThenRebuilt { reason }
        }
        CacheRecordStatus::Miss { reason } | CacheRecordStatus::Rebuilt { reason } => {
            CacheRecordStatus::Rebuilt { reason }
        }
        CacheRecordStatus::Stored => CacheRecordStatus::Stored,
    }
}

fn persistent_read_status_did_not_feed_build(status: &CacheRecordStatus) -> bool {
    matches!(
        status,
        CacheRecordStatus::Miss { .. } | CacheRecordStatus::Rebuilt { .. }
    )
}

fn persistent_query_status_from_unit_cache_hit(
    object_kind: CompilerObjectKind,
    reuse_evidence: Option<&ProjectBuildPersistentReuseEvidence>,
) -> CacheRecordStatus {
    if reuse_evidence.is_some_and(ProjectBuildPersistentReuseEvidence::is_actual_reusable) {
        return CacheRecordStatus::Hit;
    }

    let policy = reuse_evidence
        .and_then(|evidence| match evidence {
            ProjectBuildPersistentReuseEvidence::Conservative { reason, .. } => {
                Some(reason.policy())
            }
            ProjectBuildPersistentReuseEvidence::ActualReusable { .. } => None,
        })
        .or_else(|| object_kind.conservative_read_through_policy());

    if let Some(policy) = policy {
        CacheRecordStatus::HitThenRebuilt {
            reason: InvalidationReason::ConservativeInvalidation {
                policy: policy.to_owned(),
            },
        }
    } else {
        CacheRecordStatus::Hit
    }
}

fn persistent_query_object_digest(
    object_key: &CompilerObjectKey,
    payload: arcweft_project::persistent_object::CompilerObjectPayload,
) -> Result<BuildDigest, ExitCode> {
    let envelope = AwboEnvelope::new(object_key, payload).map_err(|error| {
        eprintln!("error: failed to build persistent query envelope: {error}");
        ExitCode::FAILURE
    })?;
    let bytes = envelope.encode().map_err(|error| {
        eprintln!("error: failed to encode persistent query envelope: {error}");
        ExitCode::FAILURE
    })?;
    Ok(BuildDigest::of(&bytes))
}

fn persistent_query_artifact_key(
    snapshot: &BuildSnapshot,
    query: QueryKind,
    logical_item: &str,
    source_digest: BuildDigest,
    dependency_interface_digests: Vec<NamedDigest>,
    dependency_body_digests: Vec<NamedDigest>,
    query_options_digest: BuildDigest,
) -> ArtifactKey {
    let project = snapshot.project();
    ArtifactKey::derive(&ArtifactKeyInput {
        compiler_build_id: project.compiler_build_id().to_owned(),
        query,
        artifact_kind: query.artifact_kind(),
        target_triple: project.target_triple().to_owned(),
        target_features: project.target_features().to_vec(),
        profile: project.profile().to_owned(),
        package: project.package().to_owned(),
        logical_item: logical_item.to_owned(),
        source_digest,
        dependency_interface_digests,
        dependency_body_digests,
        adapter_environment_digest: project.adapter_environment_digest(),
        launch_profile_digest: project.launch_profile_digest(),
        declared_environment_digest: project.declared_environment_digest(),
        format_options_digest: query_options_digest,
    })
}

fn persistent_compiler_identity(snapshot: &BuildSnapshot) -> CompilerBuildIdentity {
    CompilerBuildIdentity {
        package_version: env!("CARGO_PKG_VERSION").to_owned(),
        git_commit: option_env!("VERGEN_GIT_SHA")
            .or(option_env!("GIT_COMMIT_HASH"))
            .unwrap_or(snapshot.project().compiler_build_id())
            .to_owned(),
        rustc: option_env!("RUSTC_VERSION")
            .unwrap_or("rustc-unknown")
            .to_owned(),
        target: snapshot.project().target_triple().to_owned(),
        enabled_features: snapshot.project().target_features().to_vec(),
    }
    .canonicalized()
}

fn persistent_query_options_digest(query: QueryKind) -> BuildDigest {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"arcweft-persistent-query-options-v1\0");
    bytes.extend_from_slice(query.cache_namespace().as_bytes());
    BuildDigest::of(&bytes)
}

fn persistent_query_logical_item(kind: CompilerObjectKind, source: &ProjectSourceFile) -> String {
    format!("{}:{}", kind.cache_namespace(), source.module())
}

fn dependency_interface_digests(
    state: &ProjectCommandState,
    source: &ProjectSourceFile,
) -> Vec<NamedDigest> {
    source
        .dependencies()
        .iter()
        .map(|dependency| {
            let module = dependency.target();
            let digest = state
                .snapshot
                .modules()
                .iter()
                .find(|entry| entry.module() == module.to_string())
                .expect("snapshot contains dependency module fingerprint")
                .interface_digest();
            NamedDigest::new(module.to_string(), digest)
        })
        .collect()
}

fn dependency_body_digests(
    state: &ProjectCommandState,
    source: &ProjectSourceFile,
) -> Vec<NamedDigest> {
    source
        .dependencies()
        .iter()
        .map(|dependency| {
            let module = dependency.target();
            let digest = state
                .snapshot
                .modules()
                .iter()
                .find(|entry| entry.module() == module.to_string())
                .expect("snapshot contains dependency module fingerprint")
                .body_digest();
            NamedDigest::new(module.to_string(), digest)
        })
        .collect()
}

fn module_compile_cache_status(
    state: &ProjectCommandState,
    source: &ProjectSourceFile,
) -> ProjectCompileCacheStatus {
    let unit_id = state
        .loaded
        .sources()
        .graph()
        .unit_for_module(source.module())
        .expect("loaded module belongs to a compile unit");
    state
        .compiled
        .compile_units()
        .iter()
        .find(|unit| unit.id() == unit_id)
        .expect("compiled project records every compile unit")
        .cache_status()
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
    initial_snapshot: BuildSnapshot,
    mut base_bytes: Vec<u8>,
    compile_cache: &mut InMemoryProjectCompileCache,
) -> Result<(), ExitCode> {
    let mut selection = initial_state.selection;
    let mut base_snapshot = initial_snapshot;
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
                    emit_verification_diagnostics(
                        &next_state.source_document,
                        &next_state.verification,
                    );
                    eprintln!("watch: rebuild failed verification; keeping previous bundle active");
                    if max_iterations.is_some() {
                        return Err(ExitCode::FAILURE);
                    }
                    continue;
                }
                let artifacts = write_project_build_artifacts(&next_state, &report, target_root)?;
                let module_invalidations = artifacts
                    .snapshot
                    .module_invalidations_since(&base_snapshot);
                let query_invalidations = artifacts
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
                let package = next_state.loaded.sources().package().id.as_str();
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
                base_snapshot = artifacts.snapshot.clone();
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
    let progress = CliProgress::new(!options.json);
    let checked = progress.run(CliProgressStatus::Checking, options.input.display(), || {
        load_and_check_selection(&selection, None)
    })?;
    let verification = progress.run(
        CliProgressStatus::Verifying,
        options.input.display(),
        || {
            Ok(verify_module_with_env(
                &checked.hir,
                &checked.env,
                VerificationPolicy {
                    mode: VerificationMode::Dev,
                    backend: BackendKind::Emit,
                    allow_trusted_proofs: true,
                },
            ))
        },
    )?;
    if verification.has_blocking_runtime_safety_gaps() {
        emit_verification_diagnostics(&checked.source_document, &verification);
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
        CompileEmit::Hir => {
            let output_path = output.as_deref().expect("HIR emit has a default path");
            progress.run(CliProgressStatus::Writing, output_path.display(), || {
                write_text_artifact(output_path, &format!("{:#?}\n", checked.hir))
            })?;
        }
        CompileEmit::Plan => {
            let plan = progress.run(CliProgressStatus::Compiling, "runtime plan", || {
                Ok::<_, ExitCode>(checked.runtime_plan().plan.clone())
            })?;
            let output_path = output.as_deref().expect("plan emit has a default path");
            progress.run(CliProgressStatus::Writing, output_path.display(), || {
                write_text_artifact(output_path, &format!("{plan:#?}\n"))
            })?;
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
    let source_document = Arc::clone(
        loaded
            .module_document(loaded.sources().root_module().module())
            .expect("loaded projects retain their root source document"),
    );
    let semantic = semantic_context_for_selection(&selection, None)?;
    let runtime_options = runtime_plan_options_for_selection(&selection)?;
    let context = project_compilation_context(&loaded, &selection, &semantic)?;
    let compiled =
        compile_project_with_cache(loaded.sources(), &context, &runtime_options, compile_cache)
            .map_err(|error| {
                print_project_compile_error(&error);
                ExitCode::FAILURE
            })?;
    let mut verification = verify_module_with_env(
        compiled.linked_hir(),
        semantic.base(),
        VerificationPolicy {
            mode: verification_mode,
            backend: BackendKind::Emit,
            allow_trusted_proofs: verification_mode != VerificationMode::Release,
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
        source_document,
        compiled,
        verification,
        snapshot,
    })
}

fn project_build_id(loaded: &LoadedProject, compiled: &CompiledProject) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(loaded.sources().package().id.as_str().as_bytes());
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
    let Ok(index) = project_semantic_index_from_checked_project(
        compiled.hir_project(),
        compiled.registered_world().symbols(),
        compiled.typecheck_report(),
        ProgramHash::new("project.release"),
        compiled.checked_entries(),
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

fn emit_verification_diagnostics(document: &SourceDocument, report: &VerificationReport) {
    let diagnostics = report.source_diagnostics(document);
    emit_diagnostics(document, &diagnostics);
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
mod tests;
