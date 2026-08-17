use super::diagnostics::emit_diagnostics;
use super::progress::{CliProgress, CliProgressStatus};
use super::project::{
    ProfileOptions, SourceSelection, adapter_manifest_for_selection, resolve_source_selection,
    semantic_context_for_selection, verify_compiled_project,
};
use super::runtime::options::CliRuntimeStepMode;
use super::runtime::parse::parse_runtime_binding_arg;
use super::runtime::profile::report_path;
use super::runtime::profile::{
    ProfileCompiledRuntimePlan, compile_profile_runtime_plan, run_profile_phase,
};
use super::runtime::reports::{BundleCommandReport, BundleRunReport};
use super::shared::print_json;
use crate::output::{RuntimeExecutorTier, RuntimeProfilePhase};
use arcweft_adapter_context::{manifest::AdapterManifest, standard};
use arcweft_adapter_desktop::{
    DESKTOP_CAPABILITIES_CALL, DESKTOP_EXTERNAL_CONTROL_ADAPTER_ID, DESKTOP_EXTERNAL_CONTROL_CALL,
    DESKTOP_EXTERNAL_OBSERVE_ADAPTER_ID, DESKTOP_EXTERNAL_OBSERVE_CALL,
    DESKTOP_FILES_READ_ADAPTER_ID, DESKTOP_FILES_READ_CALL, DESKTOP_FILES_WRITE_ADAPTER_ID,
    DESKTOP_FILES_WRITE_CALL, DESKTOP_GLOBAL_POINTER_CONTROL_ADAPTER_ID,
    DESKTOP_GLOBAL_POINTER_CONTROL_CALL, DESKTOP_GLOBAL_POINTER_OBSERVE_ADAPTER_ID,
    DESKTOP_GLOBAL_POINTER_OBSERVE_CALL, DESKTOP_KNOWN_READ_ADAPTER_ID, DESKTOP_KNOWN_READ_CALL,
    DESKTOP_KNOWN_WRITE_ADAPTER_ID, DESKTOP_KNOWN_WRITE_CALL, DESKTOP_OWNED_WINDOW_ADAPTER_ID,
    DESKTOP_PLATFORM_ADAPTER_ID, desktop_external_control_manifest,
    desktop_external_observe_manifest, desktop_files_read_manifest, desktop_files_write_manifest,
    desktop_known_directory_read_manifest, desktop_known_directory_write_manifest,
    desktop_owned_window_manifest, desktop_platform_manifest,
    desktop_pointer_global_control_manifest, desktop_pointer_global_observe_manifest,
    is_desktop_owned_window_host_call,
};
use arcweft_bundle::{
    ArcweftBundle, BundleAdapterHostCall, BundleAdapterManifest, BundleFormat,
    BundleImageAnimation, BundleImageAsset, BundleImageDimensions, BundleImageFormat,
    BundleLaunchKind, BundleManifest, BundleRuntimeSummary, BundleVirtualFile,
    BundleVirtualFileRef, BundleVirtualFileSpace,
    container::{BundleDigest, BundleView, ReadBudget},
    fx_definitions::FxDefinitions,
    patch::{
        BundlePatchArtifact, PatchCompatibility, apply_patch_bundle_bytes, encode_patch_bundle,
    },
    resource_codec::{ViewLocalizedTextResource, ViewTextResource},
};
use arcweft_compiler::view::CompiledViewProduct;
use arcweft_core::{
    effect::{LineEffectRequest, RuntimeCall},
    line_task::{LineTaskGroup, LineTaskNode, ScopeExit},
    plan::{EntryRuntimeId, FlowOp, RuntimeEntryKind, RuntimeEntryTarget, RuntimePlan},
    value::{RuntimeBinding, RuntimeExpr, RuntimeExprKind, RuntimeValue},
};
use arcweft_id::{AssetId, AssetVirtualPath, DeclarationIdentityFamily, PublicId};
use arcweft_lang_sema::project_index::ProjectSemanticIndex;
use arcweft_launch::LaunchKind;
use arcweft_project::layout::AuthoredResourceRoots;
use arcweft_runtime_host::{
    BundleRunnerError, BundleRunnerOptions, INTERNAL_SCHEDULER_ADAPTER_ID, NativeAdapterRegistrar,
    internal_scheduler_manifest, run_bundle_file_with_native_adapters,
    run_bundle_with_native_adapters,
};
use arcweft_source::SourceDocument;
use arcweft_verify::{VerificationPolicy, VerificationReport};
use clap::Args;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct BundleOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    virtual_files: BundleVirtualFileOptions,
    #[arg(long, value_parser = parse_bundle_format_arg, default_value = "awfb")]
    format: BundleFormat,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct BundleVirtualFileOptions {
    #[arg(long)]
    include_save: bool,
    #[arg(long)]
    include_temp: bool,
    #[arg(long)]
    include_export: bool,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct RunBundleOptions {
    bundle: PathBuf,
    #[arg(long)]
    patch: Option<PathBuf>,
    #[arg(long)]
    entry: Option<String>,
    #[arg(long, default_value_t = 8)]
    steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 32)]
    max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct PatchBundleOptions {
    #[arg(long)]
    base: PathBuf,
    #[arg(long)]
    next: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(serde::Serialize)]
struct PatchBundleCommandReport {
    patch: String,
    base: String,
    next: String,
    base_content_root: String,
    target_content_root: String,
    operations: usize,
    changed_sections: usize,
    compatibility: PatchCompatibility,
}

impl BundleOptions {
    fn include_spaces(&self) -> Vec<BundleVirtualFileSpace> {
        let mut spaces = vec![BundleVirtualFileSpace::Asset];
        if self.virtual_files.include_save {
            spaces.push(BundleVirtualFileSpace::Save);
        }
        if self.virtual_files.include_temp {
            spaces.push(BundleVirtualFileSpace::Temp);
        }
        if self.virtual_files.include_export {
            spaces.push(BundleVirtualFileSpace::Export);
        }
        spaces
    }
}

fn bundle_runner_options(options: &RunBundleOptions) -> Result<BundleRunnerOptions, ExitCode> {
    let entry = options
        .entry
        .as_deref()
        .map(EntryRuntimeId::from_source_entity_body)
        .transpose()
        .map_err(|error| {
            eprintln!("error: --entry must be an exact canonical entry.* ID: {error}");
            ExitCode::from(2)
        })?;
    Ok(BundleRunnerOptions {
        entry,
        steps: options.steps,
        mode: options.mode.into(),
        max_ops: options.max_ops,
        values: options.values.clone(),
        engine_resource_types: std::sync::Arc::new(
            arcweft_resource_model::registry::ResourceTypeRegistry::empty(),
        ),
    })
}

fn parse_bundle_format_arg(value: &str) -> Result<BundleFormat, String> {
    let format = BundleFormat::parse(value).map_err(|error| error.to_string())?;
    if format.is_codec_enabled() {
        Ok(format)
    } else {
        let feature = format
            .required_feature()
            .expect("disabled bundle formats have feature gates");
        Err(format!(
            "bundle format `{format}` requires feature `{feature}`"
        ))
    }
}

fn bundle_launch_kind(kind: LaunchKind) -> BundleLaunchKind {
    match kind {
        LaunchKind::Game => BundleLaunchKind::Game,
        LaunchKind::Editor => BundleLaunchKind::Editor,
        LaunchKind::Cli => BundleLaunchKind::Cli,
        LaunchKind::Server => BundleLaunchKind::Server,
        LaunchKind::Test => BundleLaunchKind::Test,
        LaunchKind::Bench => BundleLaunchKind::Bench,
        LaunchKind::Agent => BundleLaunchKind::Agent,
    }
}

pub(super) fn bundle_command(options: &BundleOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let mut phases = Vec::new();
    let progress = CliProgress::new(!options.json);
    let bundle = progress.run(
        CliProgressStatus::Compiling,
        format!("bundle {}", report_path(selection.path())),
        || compile_bundle_artifact(&selection, options, &mut phases),
    )?;
    let bytes = progress.run(
        CliProgressStatus::Encoding,
        format!("{} bundle", options.format),
        || {
            run_profile_phase(&mut phases, "encode_bundle", || {
                bundle.to_format_bytes(options.format).map_err(|error| {
                    eprintln!("error: failed to encode bundle: {error}");
                    ExitCode::FAILURE
                })
            })
        },
    )?;
    progress.run(CliProgressStatus::Writing, options.output.display(), || {
        write_bundle_artifact(&options.output, bytes, &mut phases)
    })?;
    if options.json {
        print_json(&bundle_command_report(&options.output, &bundle, phases))
    } else {
        println!(
            "ok: {} (source={}, {} virtual file(s))",
            options.output.display(),
            bundle_source_display_name(&bundle),
            bundle.virtual_files.len()
        );
        Ok(())
    }
}

fn compile_bundle_artifact(
    selection: &SourceSelection,
    options: &BundleOptions,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ArcweftBundle, ExitCode> {
    compile_bundle_for_selection(selection, options.include_spaces(), phases)
        .map(|compiled| compiled.bundle)
}

#[derive(Clone, Debug)]
pub(in crate::app) struct CompiledBundleArtifact {
    pub(in crate::app) bundle: ArcweftBundle,
    pub(in crate::app) entry_kinds: Vec<RuntimeEntryKind>,
    pub(in crate::app) semantic_index: Arc<ProjectSemanticIndex>,
    pub(in crate::app) execution_diagnostics:
        Arc<arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext>,
}

pub(in crate::app) fn compile_bundle_for_selection(
    selection: &SourceSelection,
    include_spaces: Vec<BundleVirtualFileSpace>,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<CompiledBundleArtifact, ExitCode> {
    let semantic = semantic_context_for_selection(selection, None)?;
    let compiled = compile_profile_runtime_plan(selection, &semantic, phases)?;
    compile_bundle_from_profile_runtime_plan(selection, compiled, include_spaces)
}

pub(in crate::app) fn compile_bundle_from_profile_runtime_plan(
    selection: &SourceSelection,
    compiled: ProfileCompiledRuntimePlan,
    include_spaces: Vec<BundleVirtualFileSpace>,
) -> Result<CompiledBundleArtifact, ExitCode> {
    let semantic_index = Arc::clone(compiled.compiled.semantic_index());
    let execution_diagnostics = Arc::clone(&compiled.execution_diagnostics);
    let verification = verify_compiled_project(&compiled.compiled, VerificationPolicy::default())?;
    if verification.has_blocking_runtime_safety_gaps() {
        emit_bundle_verification_diagnostics(&compiled.source_document, &verification);
        return Err(ExitCode::FAILURE);
    }
    let entry_kinds = compiled
        .plan
        .entries()
        .iter()
        .map(|entry| entry.kind.clone())
        .collect::<Vec<_>>();
    let required_host_calls = bundle_required_host_calls(&compiled.plan);
    let adapter_manifest = adapter_manifest_for_selection(selection, None)?;
    let adapter_manifest_ids = bundle_adapter_manifest_ids(
        adapter_manifest.id().as_str(),
        required_host_calls.iter().map(String::as_str),
    );
    let adapter_manifests = bundle_adapter_manifests(
        &adapter_manifest,
        required_host_calls.iter().map(String::as_str),
    )?;
    let authored_resources = selection.authored_resource_roots();
    let virtual_files = collect_bundle_virtual_files(
        &authored_resources,
        &selection.local_state_root(),
        include_spaces,
    )?;
    let image_assets = collect_bundle_image_assets(&virtual_files)?;
    let fx_definitions =
        FxDefinitions::try_new(compiled.fx_definitions.iter().cloned()).map_err(|error| {
            eprintln!("error: failed to build Fx definitions inventory: {error}");
            ExitCode::FAILURE
        })?;
    let view_product = compiled.view_product.clone();
    let image_objects = view_product.image_objects().to_vec();
    validate_referenced_bundle_image_assets(&compiled.plan, &image_assets)?;
    let mut bundle = ArcweftBundle::try_new(
        bundle_manifest(
            selection,
            &compiled,
            adapter_manifest_ids,
            required_host_calls,
        ),
        compiled.source_map,
        compiled.product_awbc,
        compiled.dialogue_content_catalog,
    )
    .map_err(|error| {
        eprintln!("error: failed to reserve the standard dialogue Style source: {error}");
        ExitCode::FAILURE
    })?
    .with_fx_definitions(fx_definitions)
    .with_adapter_manifests(adapter_manifests)
    .with_virtual_files(virtual_files)
    .with_image_assets(image_assets)
    .with_image_objects(image_objects);
    if let Some(topology) = selection.profile_topology() {
        bundle = bundle.with_resource_type_manifests(topology.resource_type_manifests().clone());
    }
    if let Some(catalog) = compiled.character_presentation_catalog {
        bundle = bundle.with_character_presentation_catalog(catalog.as_ref().clone());
    }
    let bundle = attach_compiled_view_product(bundle, &view_product)?;
    Ok(CompiledBundleArtifact {
        bundle,
        entry_kinds,
        semantic_index,
        execution_diagnostics,
    })
}

fn emit_bundle_verification_diagnostics(document: &SourceDocument, report: &VerificationReport) {
    let diagnostics = report.source_diagnostics(document);
    emit_diagnostics(document, &diagnostics);
}

fn attach_compiled_view_product(
    mut bundle: ArcweftBundle,
    compiled: &CompiledViewProduct,
) -> Result<ArcweftBundle, ExitCode> {
    bundle = bundle
        .try_with_validated_view_product(compiled.product())
        .map_err(|error| {
            eprintln!("error: failed to attach the accepted View product: {error}");
            ExitCode::FAILURE
        })?;
    if let Some(mut resource) = compiled.text().cloned() {
        hydrate_default_view_localization(&mut resource, &bundle.dialogue_content);
        bundle = bundle.with_view_text(resource);
    }
    if let Some(resource) = compiled.input().cloned() {
        bundle = bundle.with_view_input(resource);
    }
    Ok(bundle)
}

fn hydrate_default_view_localization(
    resource: &mut ViewTextResource,
    dialogue_content: &arcweft_text_model::DialogueContentCatalog,
) {
    let keys = resource
        .sources
        .iter()
        .filter_map(|source| match &source.kind {
            arcweft_bundle::resource_codec::view::ViewTextSourceKind::Localized {
                key,
                locale: None,
            } => Some(key.clone()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    for key in keys {
        if resource.localized_document(&key, None).is_some() {
            continue;
        }
        if let Some(spec) = dialogue_content
            .records()
            .iter()
            .find(|spec| spec.text_key().as_str() == key.as_str())
        {
            resource.localized.push(ViewLocalizedTextResource {
                key,
                locale: None,
                document: spec.content().clone(),
            });
        }
    }
}

fn bundle_required_host_calls(plan: &RuntimePlan) -> Vec<String> {
    let mut required_host_calls = plan
        .flows()
        .iter()
        .flat_map(|flow| flow.ops.iter())
        .flat_map(collect_flow_op_host_calls)
        .chain(
            plan.line_task_groups()
                .iter()
                .flat_map(collect_line_task_group_host_calls),
        )
        .collect::<Vec<_>>();
    required_host_calls.sort();
    required_host_calls.dedup();
    required_host_calls
}

fn bundle_manifest(
    selection: &SourceSelection,
    compiled: &ProfileCompiledRuntimePlan,
    adapter_manifest_ids: Vec<String>,
    required_host_calls: Vec<String>,
) -> BundleManifest {
    BundleManifest {
        profile_id: selection
            .profile()
            .map(|profile| profile.id().as_str().to_owned()),
        profile_kind: selection
            .profile()
            .map(|profile| bundle_launch_kind(profile.kind())),
        entry: selection.entry().map(str::to_owned),
        adapter: selection.adapter().map(str::to_owned),
        adapter_manifest_ids,
        required_host_calls,
        runtime: BundleRuntimeSummary {
            artifact_fingerprint: compiled.execution_diagnostics.artifact(),
            entry_flow: selection.entry().and_then(|selected| {
                compiled
                    .plan
                    .entries()
                    .iter()
                    .find(|entry| entry.id.public_label().as_str() == selected)
                    .and_then(|entry| match &entry.target {
                        RuntimeEntryTarget::Flow(flow) | RuntimeEntryTarget::Controller(flow) => {
                            Some(flow.public_label().into_string())
                        }
                        RuntimeEntryTarget::Routes(_) => None,
                    })
            }),
            flows: compiled.product_awbc.flow_executables.len(),
            bytecode_instructions: compiled.product_awbc.instructions.len(),
            line_task_groups: compiled.product_awbc.line_task_groups.len(),
            stream_plans: compiled.product_awbc.stream_plans.len(),
        },
    }
}

pub(in crate::app) fn write_bundle_artifact(
    output: &Path,
    bytes: Vec<u8>,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<(), ExitCode> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            eprintln!(
                "error: failed to create bundle output directory {}: {error}",
                parent.display()
            );
            ExitCode::FAILURE
        })?;
    }
    run_profile_phase(phases, "write_bundle", || {
        fs::write(output, bytes).map_err(|error| {
            eprintln!(
                "error: failed to write bundle {}: {error}",
                output.display()
            );
            ExitCode::FAILURE
        })
    })
}

fn bundle_command_report(
    output: &Path,
    bundle: &ArcweftBundle,
    phases: Vec<RuntimeProfilePhase>,
) -> BundleCommandReport {
    BundleCommandReport {
        bundle: report_path(output),
        source: bundle_source_display_name(bundle).to_owned(),
        required_host_calls: bundle.manifest.required_host_calls.clone(),
        adapter_manifests: bundle.adapter_manifests.len(),
        bytecode_instructions: bundle.manifest.runtime.bytecode_instructions,
        virtual_files: bundle.virtual_files.len(),
        image_assets: bundle.image_assets.len(),
        phases,
        runtime: bundle.manifest.runtime.clone(),
    }
}

fn bundle_source_display_name(bundle: &ArcweftBundle) -> &str {
    bundle.source_display_name()
}

pub(super) fn run_bundle_command(
    options: &RunBundleOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let runner_options = bundle_runner_options(options)?;
    let execution = if let Some(patch) = options.patch.as_ref() {
        run_patched_bundle_with_native_adapters(
            &options.bundle,
            patch,
            &runner_options,
            adapter_registrars,
        )?
    } else {
        run_bundle_file_with_native_adapters(&options.bundle, &runner_options, adapter_registrars)
            .map_err(|error| {
            eprintln!("error: {error}");
            bundle_runner_error_exit_code(&error)
        })?
    };
    let report = BundleRunReport {
        bundle: report_path(&options.bundle),
        patch: options.patch.as_deref().map(report_path),
        source: execution.source,
        bytecode_instructions: execution.bytecode_instructions,
        adapter_manifests: execution.adapter_manifests,
        phases: execution.phases,
        executor: RuntimeExecutorTier::AwbcProduct,
        executor_stats: execution.executor_stats,
        native_io: execution.native_io,
        steps: execution.steps,
        final_status: execution.final_status,
    };
    if options.json {
        print_json(&report)
    } else {
        println!(
            "ok: {} ({} step(s), final_status={})",
            options.bundle.display(),
            report.steps.len(),
            report.final_status
        );
        Ok(())
    }
}

pub(super) fn patch_bundle_command(options: &PatchBundleOptions) -> Result<(), ExitCode> {
    let base_bytes = read_patch_input("base", &options.base)?;
    let next_bytes = read_patch_input("next", &options.next)?;
    let artifact = build_patch_bundle_artifact_from_awfb_bytes(&base_bytes, &next_bytes)?;
    let patch_bytes = encode_patch_bundle(&artifact).map_err(|error| {
        eprintln!("error: failed to encode patch bundle: {error}");
        ExitCode::FAILURE
    })?;
    write_patch_bundle_artifact(&options.output, patch_bytes)?;
    let report = PatchBundleCommandReport {
        patch: report_path(&options.output),
        base: report_path(&options.base),
        next: report_path(&options.next),
        base_content_root: digest_report(artifact.plan.base_content_root),
        target_content_root: digest_report(artifact.plan.target_content_root),
        operations: artifact.plan.operations.len(),
        changed_sections: artifact.changed_sections.len(),
        compatibility: artifact.manifest.compatibility,
    };
    if options.json {
        print_json(&report)
    } else {
        println!(
            "ok: {} ({} operation(s), compatibility={})",
            options.output.display(),
            report.operations,
            report.compatibility.label()
        );
        Ok(())
    }
}

fn read_patch_input(label: &str, path: &Path) -> Result<Vec<u8>, ExitCode> {
    fs::read(path).map_err(|error| {
        eprintln!(
            "error: failed to read {label} bundle {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })
}

pub(in crate::app) fn build_patch_bundle_artifact_from_awfb_bytes(
    base_bytes: &[u8],
    next_bytes: &[u8],
) -> Result<BundlePatchArtifact, ExitCode> {
    let base = BundleView::parse(base_bytes, ReadBudget::default()).map_err(|error| {
        eprintln!("error: failed to decode base AWFB bundle: {error}");
        ExitCode::FAILURE
    })?;
    let next = BundleView::parse(next_bytes, ReadBudget::default()).map_err(|error| {
        eprintln!("error: failed to decode next AWFB bundle: {error}");
        ExitCode::FAILURE
    })?;
    BundlePatchArtifact::from_views(&base, &next).map_err(|error| {
        eprintln!("error: failed to build patch artifact: {error}");
        ExitCode::FAILURE
    })
}

pub(in crate::app) fn write_patch_bundle_artifact(
    output: &Path,
    bytes: Vec<u8>,
) -> Result<(), ExitCode> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            eprintln!(
                "error: failed to create patch output directory {}: {error}",
                parent.display()
            );
            ExitCode::FAILURE
        })?;
    }
    fs::write(output, bytes).map_err(|error| {
        eprintln!(
            "error: failed to write patch bundle {}: {error}",
            output.display()
        );
        ExitCode::FAILURE
    })
}

fn digest_report(digest: BundleDigest) -> String {
    digest.to_string()
}

fn run_patched_bundle_with_native_adapters(
    bundle: &Path,
    patch: &Path,
    runner_options: &BundleRunnerOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<arcweft_runtime_host::BundleRunnerReport, ExitCode> {
    let base_bytes = fs::read(bundle).map_err(|error| {
        eprintln!(
            "error: failed to read base bundle {}: {error}",
            bundle.display()
        );
        ExitCode::FAILURE
    })?;
    let patch_bytes = fs::read(patch).map_err(|error| {
        eprintln!(
            "error: failed to read patch bundle {}: {error}",
            patch.display()
        );
        ExitCode::FAILURE
    })?;
    let materialized = apply_patch_bundle_bytes(&base_bytes, &patch_bytes).map_err(|error| {
        eprintln!("error: failed to apply bundle patch: {error}");
        ExitCode::FAILURE
    })?;
    let target_bytes = materialized.into_bytes();
    let engine_resource_types = arcweft_resource_model::registry::ResourceTypeRegistry::empty();
    let target_bundle =
        ArcweftBundle::from_awfb_slice_with_resource_types(&target_bytes, &engine_resource_types)
            .map_err(|error| {
            eprintln!("error: failed to decode patched target bundle: {error}");
            ExitCode::FAILURE
        })?;
    run_bundle_with_native_adapters(&target_bundle, runner_options, adapter_registrars).map_err(
        |error| {
            eprintln!("error: {error}");
            bundle_runner_error_exit_code(&error)
        },
    )
}

fn bundle_runner_error_exit_code(error: &BundleRunnerError) -> ExitCode {
    match error {
        BundleRunnerError::MissingEntrySelection
        | BundleRunnerError::InvalidEntrySelection { .. }
        | BundleRunnerError::ExpectedAwfbProduct { .. } => ExitCode::from(2),
        BundleRunnerError::ReadBundle { .. }
        | BundleRunnerError::DecodeBundle(_)
        | BundleRunnerError::InvalidImageAsset(_)
        | BundleRunnerError::UnsupportedBundleKind { .. }
        | BundleRunnerError::DecodeImageAsset { .. }
        | BundleRunnerError::ImageAssetMetadataMismatch { .. }
        | BundleRunnerError::ProductAwbcRuntime(_)
        | BundleRunnerError::CreateWorkspace(_)
        | BundleRunnerError::CreateSourceDirectory(_)
        | BundleRunnerError::MaterializeSource(_)
        | BundleRunnerError::CreateVirtualFileDirectory(_)
        | BundleRunnerError::MaterializeVirtualFile(_)
        | BundleRunnerError::InvalidVirtualFilePath
        | BundleRunnerError::UnknownEntry { .. }
        | BundleRunnerError::NonFlowEntry { .. }
        | BundleRunnerError::StartEntry(_)
        | BundleRunnerError::NativeAdapter(_) => ExitCode::FAILURE,
    }
}

fn collect_flow_op_host_calls(op: &FlowOp) -> Vec<String> {
    match op {
        FlowOp::Await { target, .. } => vec![host_call_id_for_template(
            target.request.capability.0.as_str(),
            target.request.operation.as_str(),
        )],
        FlowOp::AwaitMany { target, .. } => vec![host_call_id_for_template(
            target.request.capability.0.as_str(),
            target.request.operation.as_str(),
        )],
        FlowOp::HostCall { target, .. } => {
            vec![target.public_id.clone()]
        }
        FlowOp::LetElse { else_ops, .. } => collect_flow_ops_host_calls(else_ops),
        FlowOp::If {
            then_ops, else_ops, ..
        }
        | FlowOp::IfLet {
            then_ops, else_ops, ..
        } => collect_flow_ops_host_calls(then_ops)
            .into_iter()
            .chain(collect_flow_ops_host_calls(else_ops))
            .collect(),
        FlowOp::Match { arms, .. } => arms
            .iter()
            .flat_map(|arm| collect_flow_ops_host_calls(&arm.ops))
            .collect(),
        FlowOp::Loop { body, .. }
        | FlowOp::While { body, .. }
        | FlowOp::WhileLet { body, .. }
        | FlowOp::For { body, .. }
        | FlowOp::Thread { body, .. } => {
            let mut calls = collect_flow_ops_host_calls(body);
            if matches!(op, FlowOp::Thread { .. }) {
                calls.push("flow_thread.run_child".to_owned());
            }
            calls
        }
        FlowOp::LoopNext { body }
        | FlowOp::WhileNext { body, .. }
        | FlowOp::WhileLetNext { body, .. }
        | FlowOp::ForNext { body, .. } => collect_flow_ops_host_calls(body.as_ref().iter()),
        FlowOp::Scope(ops) | FlowOp::LetScope { ops, .. } => collect_flow_ops_host_calls(ops),
        FlowOp::Bind(_)
        | FlowOp::Let { .. }
        | FlowOp::AssignNominalField { .. }
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Break(_)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::GotoExpr(_)
        | FlowOp::Return(_)
        | FlowOp::ReturnExpr(_)
        | FlowOp::Effect(_)
        | FlowOp::EvaluatedEffect(_)
        | FlowOp::RegisterCleanup { .. }
        | FlowOp::CancelCleanup { .. }
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::ExitScopeBind { .. }
        | FlowOp::Noop => Vec::new(),
    }
}

fn collect_line_task_group_host_calls(group: &LineTaskGroup) -> Vec<String> {
    group
        .nodes()
        .iter()
        .filter_map(|node| match node {
            LineTaskNode::Action(ops) => Some(ops.as_ref()),
            LineTaskNode::Sequence(_)
            | LineTaskNode::Start(_)
            | LineTaskNode::Parallel { .. }
            | LineTaskNode::Child { .. } => None,
        })
        .flat_map(collect_flow_ops_host_calls)
        .chain(
            group
                .cancel_rules()
                .iter()
                .flat_map(|rule| collect_flow_ops_host_calls(rule.action())),
        )
        .chain(
            [
                ScopeExit::Completed,
                ScopeExit::Cancelled,
                ScopeExit::Failed,
            ]
            .into_iter()
            .flat_map(|exit| collect_flow_ops_host_calls(group.cleanup().actions(exit))),
        )
        .collect()
}

fn collect_flow_ops_host_calls<'a>(ops: impl IntoIterator<Item = &'a FlowOp>) -> Vec<String> {
    ops.into_iter()
        .flat_map(collect_flow_op_host_calls)
        .collect()
}

fn validate_referenced_bundle_image_assets(
    plan: &RuntimePlan,
    image_assets: &[BundleImageAsset],
) -> Result<(), ExitCode> {
    let available = image_assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<Vec<_>>();
    let missing = static_image_asset_refs(plan)
        .into_iter()
        .filter(|id| !available.iter().any(|available_id| available_id == id))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    eprintln!(
        "error: bundle source references missing image asset(s): {}",
        missing.join(", ")
    );
    Err(ExitCode::from(2))
}

fn static_image_asset_refs(plan: &RuntimePlan) -> Vec<String> {
    let mut refs = plan
        .flows()
        .iter()
        .flat_map(|flow| flow.ops.iter())
        .flat_map(collect_flow_op_static_image_asset_refs)
        .chain(
            plan.line_task_groups()
                .iter()
                .flat_map(collect_line_task_group_static_image_asset_refs),
        )
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

fn collect_flow_op_static_image_asset_refs(op: &FlowOp) -> Vec<String> {
    match op {
        FlowOp::Await {
            target, pending, ..
        } => static_image_asset_ref_for_template(&target.request)
            .into_iter()
            .chain(collect_line_effects_static_image_asset_refs(pending))
            .collect(),
        FlowOp::AwaitMany {
            target, pending, ..
        } => static_image_asset_ref_for_template(&target.request)
            .into_iter()
            .chain(collect_line_effects_static_image_asset_refs(pending))
            .collect(),
        FlowOp::LetElse { else_ops, .. } => collect_flow_ops_static_image_asset_refs(else_ops),
        FlowOp::If {
            then_ops, else_ops, ..
        }
        | FlowOp::IfLet {
            then_ops, else_ops, ..
        } => collect_flow_ops_static_image_asset_refs(then_ops)
            .into_iter()
            .chain(collect_flow_ops_static_image_asset_refs(else_ops))
            .collect(),
        FlowOp::Match { arms, .. } => arms
            .iter()
            .flat_map(|arm| collect_flow_ops_static_image_asset_refs(&arm.ops))
            .collect(),
        FlowOp::Loop { body, .. }
        | FlowOp::While { body, .. }
        | FlowOp::WhileLet { body, .. }
        | FlowOp::For { body, .. }
        | FlowOp::Thread { body, .. } => collect_flow_ops_static_image_asset_refs(body),
        FlowOp::LoopNext { body }
        | FlowOp::WhileNext { body, .. }
        | FlowOp::WhileLetNext { body, .. }
        | FlowOp::ForNext { body, .. } => collect_flow_ops_static_image_asset_refs(body.iter()),
        FlowOp::Scope(ops) | FlowOp::LetScope { ops, .. } => {
            collect_flow_ops_static_image_asset_refs(ops)
        }
        FlowOp::Effect(effect) | FlowOp::RegisterCleanup { effect, .. } => {
            collect_line_effect_static_image_asset_refs(effect)
        }
        FlowOp::Bind(_)
        | FlowOp::Let { .. }
        | FlowOp::AssignNominalField { .. }
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::HostCall { .. }
        | FlowOp::Break(_)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::GotoExpr(_)
        | FlowOp::Return(_)
        | FlowOp::ReturnExpr(_)
        | FlowOp::EvaluatedEffect(_)
        | FlowOp::CancelCleanup { .. }
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::ExitScopeBind { .. }
        | FlowOp::Noop => Vec::new(),
    }
}

fn collect_flow_ops_static_image_asset_refs<'a>(
    ops: impl IntoIterator<Item = &'a FlowOp>,
) -> Vec<String> {
    ops.into_iter()
        .flat_map(collect_flow_op_static_image_asset_refs)
        .collect()
}

fn static_image_asset_ref_for_template(
    request: &arcweft_core::task::HostTaskRequestTemplate,
) -> Option<String> {
    if request.capability.0 != "asset" || request.operation != "image" {
        return None;
    }
    request
        .args
        .first()
        .and_then(|arg| static_image_asset_ref_expr(arg.value()))
}

fn static_image_asset_ref_expr(expr: &RuntimeExpr) -> Option<String> {
    match expr.kind() {
        RuntimeExprKind::EntityRef(id) => Some(id.runtime_label()),
        RuntimeExprKind::Value(RuntimeValue::EntityRef(id) | RuntimeValue::String(id)) => {
            Some(id.clone())
        }
        _ => None,
    }
}

fn collect_line_task_group_static_image_asset_refs(group: &LineTaskGroup) -> Vec<String> {
    group
        .nodes()
        .iter()
        .filter_map(|node| match node {
            LineTaskNode::Action(ops) => Some(ops.as_ref()),
            LineTaskNode::Sequence(_)
            | LineTaskNode::Start(_)
            | LineTaskNode::Parallel { .. }
            | LineTaskNode::Child { .. } => None,
        })
        .flat_map(collect_flow_ops_static_image_asset_refs)
        .chain(
            group
                .cancel_rules()
                .iter()
                .flat_map(|rule| collect_flow_ops_static_image_asset_refs(rule.action())),
        )
        .chain(
            [
                ScopeExit::Completed,
                ScopeExit::Cancelled,
                ScopeExit::Failed,
            ]
            .into_iter()
            .flat_map(|exit| {
                collect_flow_ops_static_image_asset_refs(group.cleanup().actions(exit))
            }),
        )
        .collect()
}

fn collect_line_effects_static_image_asset_refs<'a>(
    effects: impl IntoIterator<Item = &'a LineEffectRequest>,
) -> Vec<String> {
    effects
        .into_iter()
        .flat_map(collect_line_effect_static_image_asset_refs)
        .collect()
}

fn collect_line_effect_static_image_asset_refs(effect: &LineEffectRequest) -> Vec<String> {
    match effect {
        LineEffectRequest::Call(call) => static_image_asset_ref_for_runtime_call(call)
            .into_iter()
            .collect(),
        LineEffectRequest::RegisterHandle { .. }
        | LineEffectRequest::DropHandle { .. }
        | LineEffectRequest::Wait(_)
        | LineEffectRequest::Log(_)
        | LineEffectRequest::SignalWrite(_)
        | LineEffectRequest::MetricWrite(_)
        | LineEffectRequest::EmitEvent(_)
        | LineEffectRequest::Out(_)
        | LineEffectRequest::Return(_)
        | LineEffectRequest::Goto(_)
        | LineEffectRequest::Panic(_)
        | LineEffectRequest::Fail(_)
        | LineEffectRequest::Bail(_)
        | LineEffectRequest::Ensure { .. }
        | LineEffectRequest::Assert(_)
        | LineEffectRequest::Close(_)
        | LineEffectRequest::Select(_)
        | LineEffectRequest::Break { .. }
        | LineEffectRequest::Continue { .. }
        | LineEffectRequest::Audio(_) => Vec::new(),
    }
}

fn static_image_asset_ref_for_runtime_call(call: &RuntimeCall) -> Option<String> {
    match call.callee.as_str() {
        "bg" | "image" => runtime_call_asset_arg(call, 0),
        _ => None,
    }
}

fn runtime_call_asset_arg(call: &RuntimeCall, positional_index: usize) -> Option<String> {
    call.args
        .iter()
        .find_map(|arg| runtime_named_call_arg(arg, "asset"))
        .or_else(|| runtime_positional_call_arg(call, positional_index))
        .and_then(static_image_asset_ref_runtime_arg)
}

fn runtime_named_call_arg<'a>(arg: &'a str, name: &str) -> Option<&'a str> {
    let (arg_name, value) = arg.split_once(" = ")?;
    (arg_name.trim() == name).then_some(value.trim())
}

fn runtime_positional_call_arg(call: &RuntimeCall, index: usize) -> Option<&str> {
    call.args
        .iter()
        .filter(|arg| !arg.contains(" = "))
        .nth(index)
        .map(String::as_str)
}

fn static_image_asset_ref_runtime_arg(arg: &str) -> Option<String> {
    let value = arg.trim().trim_matches('"').trim_matches('\'');
    let value = value.strip_prefix('@').unwrap_or(value);
    let canonical = if let Some((family, suffix)) = value.split_once(":.") {
        (!family.is_empty() && !suffix.is_empty()).then(|| format!("{family}.{suffix}"))?
    } else {
        value.to_owned()
    };
    let id = PublicId::try_new(canonical).ok()?;
    DeclarationIdentityFamily::Asset
        .validate_public_id(&id)
        .ok()?;
    Some(id.as_str().to_owned())
}

fn host_call_id_for_template(capability: &str, operation: &str) -> String {
    format!("{capability}.{operation}")
}

fn bundle_adapter_manifest_ids<'a>(
    selected_adapter_id: &str,
    required_host_calls: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut ids = std::iter::once(selected_adapter_id)
        .chain(required_host_calls.into_iter().filter_map(|host_call| {
            host_call
                .strip_prefix("fs.")
                .map(|_| standard::NATIVE_FILE_ADAPTER_ID)
                .or_else(|| {
                    host_call
                        .strip_prefix("system.")
                        .map(|_| standard::SYSTEM_INFO_ADAPTER_ID)
                })
                .or_else(|| {
                    matches!(host_call, "line_task.run_child" | "flow_thread.run_child")
                        .then_some(INTERNAL_SCHEDULER_ADAPTER_ID)
                })
                .or_else(|| desktop_manifest_id_for_host_call(host_call))
        }))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn bundle_adapter_manifests<'a>(
    selected: &AdapterManifest,
    required_host_calls: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<BundleAdapterManifest>, ExitCode> {
    let required = required_host_calls.into_iter().collect::<Vec<_>>();
    let mut manifests = vec![bundle_adapter_manifest_from_context(selected)];
    if required
        .iter()
        .any(|host_call| host_call.starts_with("fs."))
    {
        manifests.push(bundle_adapter_manifest_from_context(
            &standard::native_file_manifest(),
        ));
    }
    if required
        .iter()
        .any(|host_call| host_call.starts_with("system."))
    {
        manifests.push(bundle_adapter_manifest_from_context(
            &standard::system_info_manifest(),
        ));
    }
    if required
        .iter()
        .any(|host_call| matches!(*host_call, "line_task.run_child" | "flow_thread.run_child"))
    {
        manifests.push(bundle_adapter_manifest_from_context(
            &internal_scheduler_manifest(),
        ));
    }
    manifests.extend(
        required
            .iter()
            .filter_map(|host_call| desktop_manifest_for_host_call(host_call))
            .map(|manifest| bundle_adapter_manifest_from_context(&manifest)),
    );
    let mut by_id: BTreeMap<String, BundleAdapterManifest> = BTreeMap::new();
    for manifest in manifests {
        match by_id.entry(manifest.id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(manifest);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                if entry.get() != &manifest {
                    eprintln!(
                        "error: duplicate bundle adapter manifest id `{}` has conflicting bodies",
                        entry.key()
                    );
                    return Err(ExitCode::FAILURE);
                }
            }
        }
    }
    Ok(by_id.into_values().collect())
}

fn desktop_manifest_id_for_host_call(host_call: &str) -> Option<&'static str> {
    match host_call {
        DESKTOP_CAPABILITIES_CALL => Some(DESKTOP_PLATFORM_ADAPTER_ID),
        host_call if is_desktop_owned_window_host_call(host_call) => {
            Some(DESKTOP_OWNED_WINDOW_ADAPTER_ID)
        }
        DESKTOP_FILES_READ_CALL => Some(DESKTOP_FILES_READ_ADAPTER_ID),
        DESKTOP_FILES_WRITE_CALL => Some(DESKTOP_FILES_WRITE_ADAPTER_ID),
        DESKTOP_KNOWN_READ_CALL => Some(DESKTOP_KNOWN_READ_ADAPTER_ID),
        DESKTOP_KNOWN_WRITE_CALL => Some(DESKTOP_KNOWN_WRITE_ADAPTER_ID),
        DESKTOP_GLOBAL_POINTER_OBSERVE_CALL => Some(DESKTOP_GLOBAL_POINTER_OBSERVE_ADAPTER_ID),
        DESKTOP_GLOBAL_POINTER_CONTROL_CALL => Some(DESKTOP_GLOBAL_POINTER_CONTROL_ADAPTER_ID),
        DESKTOP_EXTERNAL_OBSERVE_CALL => Some(DESKTOP_EXTERNAL_OBSERVE_ADAPTER_ID),
        DESKTOP_EXTERNAL_CONTROL_CALL => Some(DESKTOP_EXTERNAL_CONTROL_ADAPTER_ID),
        _ => None,
    }
}

fn desktop_manifest_for_host_call(host_call: &str) -> Option<AdapterManifest> {
    match host_call {
        DESKTOP_CAPABILITIES_CALL => Some(desktop_platform_manifest()),
        host_call if is_desktop_owned_window_host_call(host_call) => {
            Some(desktop_owned_window_manifest())
        }
        DESKTOP_FILES_READ_CALL => Some(desktop_files_read_manifest()),
        DESKTOP_FILES_WRITE_CALL => Some(desktop_files_write_manifest()),
        DESKTOP_KNOWN_READ_CALL => Some(desktop_known_directory_read_manifest()),
        DESKTOP_KNOWN_WRITE_CALL => Some(desktop_known_directory_write_manifest()),
        DESKTOP_GLOBAL_POINTER_OBSERVE_CALL => Some(desktop_pointer_global_observe_manifest()),
        DESKTOP_GLOBAL_POINTER_CONTROL_CALL => Some(desktop_pointer_global_control_manifest()),
        DESKTOP_EXTERNAL_OBSERVE_CALL => Some(desktop_external_observe_manifest()),
        DESKTOP_EXTERNAL_CONTROL_CALL => Some(desktop_external_control_manifest()),
        _ => None,
    }
}

fn bundle_adapter_manifest_from_context(manifest: &AdapterManifest) -> BundleAdapterManifest {
    BundleAdapterManifest {
        id: manifest.id().as_str().to_owned(),
        display_name: manifest.display_name().to_owned(),
        effects: manifest
            .effects()
            .iter()
            .map(|effect| effect.as_str().to_owned())
            .collect(),
        host_calls: manifest
            .host_calls()
            .iter()
            .map(|host_call| BundleAdapterHostCall {
                id: host_call.id().to_owned(),
                effects: host_call
                    .effects()
                    .iter()
                    .map(|effect| effect.as_str().to_owned())
                    .collect(),
            })
            .collect(),
    }
}

fn collect_bundle_virtual_files(
    authored_resources: &AuthoredResourceRoots,
    local_state_root: &Path,
    spaces: impl IntoIterator<Item = BundleVirtualFileSpace>,
) -> Result<Vec<BundleVirtualFile>, ExitCode> {
    spaces
        .into_iter()
        .map(|space| {
            let root = match space {
                BundleVirtualFileSpace::Asset => authored_resources.asset().to_path_buf(),
                BundleVirtualFileSpace::Save
                | BundleVirtualFileSpace::Temp
                | BundleVirtualFileSpace::Export => local_state_root.join(space.as_str()),
            };
            collect_bundle_virtual_files_for_space(&root, space)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|groups| groups.into_iter().flatten().collect())
}

fn collect_bundle_image_assets(
    files: &[BundleVirtualFile],
) -> Result<Vec<BundleImageAsset>, ExitCode> {
    let mut assets = files
        .iter()
        .filter(|file| file.space == BundleVirtualFileSpace::Asset)
        .map(bundle_image_asset_from_virtual_file)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assets.sort_by(|left, right| left.id.cmp(&right.id));
    if let Some(collision) = assets.windows(2).find(|pair| pair[0].id == pair[1].id) {
        eprintln!(
            "error: bundled image asset paths {} and {} derive the same stable identity {}",
            collision[0].file.path, collision[1].file.path, collision[0].id
        );
        return Err(ExitCode::FAILURE);
    }
    Ok(assets)
}

fn bundle_image_asset_from_virtual_file(
    file: &BundleVirtualFile,
) -> Result<Option<BundleImageAsset>, ExitCode> {
    let Some(format) = bundle_image_format_from_path(&file.path) else {
        return Ok(None);
    };
    let virtual_path = AssetVirtualPath::try_new(file.path.clone()).map_err(|error| {
        eprintln!(
            "error: bundled image asset has an invalid virtual path {}: {error}",
            file.path
        );
        ExitCode::FAILURE
    })?;
    let id = AssetId::try_from(&virtual_path).map_err(|error| {
        eprintln!(
            "error: bundled image asset cannot form a stable asset identity {}: {error}",
            file.path
        );
        ExitCode::FAILURE
    })?;
    let decoded = arcweft_image::decode_image_bytes(
        bundle_image_decode_format(format),
        &file.bytes,
        arcweft_image::ImageDecodeOptions::default(),
    )
    .map_err(|error| {
        eprintln!(
            "error: failed to decode bundled image asset {}: {error}",
            file.path
        );
        ExitCode::FAILURE
    })?;
    let dimensions = decoded.dimensions();
    Ok(Some(BundleImageAsset {
        id: id.as_str().to_owned(),
        file: BundleVirtualFileRef {
            space: file.space,
            path: file.path.clone(),
        },
        format,
        animation: if decoded.is_animated() {
            BundleImageAnimation::Animated
        } else {
            BundleImageAnimation::Static
        },
        dimensions: Some(BundleImageDimensions {
            width: dimensions.width(),
            height: dimensions.height(),
        }),
    }))
}

fn bundle_image_format_from_path(path: &str) -> Option<BundleImageFormat> {
    match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "png" => Some(BundleImageFormat::Png),
        "jpg" | "jpeg" => Some(BundleImageFormat::Jpeg),
        "gif" => Some(BundleImageFormat::Gif),
        "webp" => Some(BundleImageFormat::WebP),
        _ => None,
    }
}

const fn bundle_image_decode_format(format: BundleImageFormat) -> arcweft_image::ImageFormat {
    match format {
        BundleImageFormat::Png => arcweft_image::ImageFormat::Png,
        BundleImageFormat::Jpeg => arcweft_image::ImageFormat::Jpeg,
        BundleImageFormat::Gif => arcweft_image::ImageFormat::Gif,
        BundleImageFormat::WebP => arcweft_image::ImageFormat::WebP,
    }
}

fn collect_bundle_virtual_files_for_space(
    dir: &Path,
    space: BundleVirtualFileSpace,
) -> Result<Vec<BundleVirtualFile>, ExitCode> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_bundle_virtual_files_from_dir(dir, dir, space, &mut files)?;
    Ok(files)
}

fn collect_bundle_virtual_files_from_dir(
    root: &Path,
    dir: &Path,
    space: BundleVirtualFileSpace,
    files: &mut Vec<BundleVirtualFile>,
) -> Result<(), ExitCode> {
    let entries = fs::read_dir(dir).map_err(|error| {
        eprintln!(
            "error: failed to read virtual file directory {}: {error}",
            dir.display()
        );
        ExitCode::FAILURE
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            eprintln!("error: failed to read virtual file entry: {error}");
            ExitCode::FAILURE
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_bundle_virtual_files_from_dir(root, &path, space, files)?;
        } else if path.is_file() {
            let relative = normalized_relative_path(root, &path)?;
            let bytes = fs::read(&path).map_err(|error| {
                eprintln!(
                    "error: failed to read virtual file {}: {error}",
                    path.display()
                );
                ExitCode::FAILURE
            })?;
            files.push(BundleVirtualFile {
                space,
                path: relative,
                bytes,
            });
        }
    }
    Ok(())
}

fn normalized_relative_path(root: &Path, path: &Path) -> Result<String, ExitCode> {
    let relative = path.strip_prefix(root).map_err(|error| {
        eprintln!(
            "error: virtual file {} is outside {}: {error}",
            path.display(),
            root.display()
        );
        ExitCode::FAILURE
    })?;
    validate_relative_virtual_path(relative)?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn validate_relative_virtual_path(path: &Path) -> Result<(), ExitCode> {
    let valid = path
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    if valid {
        Ok(())
    } else {
        eprintln!("error: bundle virtual file path must be relative and normalized");
        Err(ExitCode::FAILURE)
    }
}

#[cfg(test)]
mod tests;
