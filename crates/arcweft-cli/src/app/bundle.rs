use super::bundle_view::view_sidecars;
use super::diagnostics::emit_diagnostics_for_path;
use super::image_declarations::{
    DeclaredImageObject, declaration_arg_value, declared_image_asset_refs,
    parse_declared_image_objects, public_asset_ref_arg, public_id_arg,
};
use super::progress::{CliProgress, CliProgressStatus};
use super::project::{
    ProfileOptions, SourceSelection, adapter_manifest_for_selection, resolve_source_selection,
    typecheck_env_for_selection,
};
use super::runtime::options::{CliRuntimeExecutorTier, CliRuntimeStepMode};
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
    BundleImageObject, BundleImageObjectAlignment, BundleImageObjectFit, BundleImageObjectParam,
    BundleImageObjectPlayback, BundleImageObjectProxy, BundleImageObjectTransform,
    BundleLaunchKind, BundleManifest, BundleRuntimeSummary, BundleSource, BundleVirtualFile,
    BundleVirtualFileRef, BundleVirtualFileSpace,
    container::{BundleDigest, BundleView, ReadBudget},
    fx_definitions::FxDefinitions,
    patch::{
        BundlePatchArtifact, PatchCompatibility, apply_patch_bundle_bytes, encode_patch_bundle,
    },
    resource_codec::{
        ViewInputResource, ViewLocalizedTextResource, ViewProgramResource, ViewStyleResource,
        ViewTextResource, ViewThemeResource,
        view::{
            RgbaColor, StyleAssignOp, StyleSourceIdentity, StyleSourceRef,
            StyleSyntax as ProductStyleSyntax, SystemColor, ViewElementKind, ViewElementState,
            ViewEnvironmentPredicate, ViewInteractionState, ViewStyleDeclaration, ViewStyleRule,
            ViewStyleSelector, ViewStyleSelectorPart, ViewStyleToken, ViewStyleValue,
        },
    },
};
use arcweft_core::{
    effect::{LineEffectRequest, RuntimeCall},
    line_task::{LineChildTask, LineTaskGroup, LineTaskNode, LineTaskScope},
    plan::{FlowOp, RuntimeEntryKind, RuntimePlan},
    value::{RuntimeBinding, RuntimeExpr, RuntimeValue},
};
use arcweft_lang_hir::model::{HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::ast::{
    items::{
        StyleItem, ViewStyleAssignOpDecl, ViewStyleEnvironmentPredicateDecl,
        ViewStyleSelectorPartDecl, ViewStyleValueDecl,
    },
    style::StyleSyntax,
};
use arcweft_launch::LaunchKind;
use arcweft_project::manifest::AuthoredResourceRoots;
use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
use arcweft_runtime_host::{
    BundleRunnerError, BundleRunnerOptions, INTERNAL_SCHEDULER_ADAPTER_ID, NativeAdapterRegistrar,
    internal_scheduler_manifest, run_bundle_file_with_native_adapters,
    run_bundle_with_native_adapters,
};
use arcweft_runtime_plan::fx::lower_fx_definitions_for_package;
use arcweft_source::SourceName;
use arcweft_verify::{
    BackendKind, VerificationMode, VerificationPolicy, VerificationReport, verify_module_with_env,
};
use clap::Args;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

mod stage_placement;
mod view_mounts;
use stage_placement::{image_design_bounds, image_stage_placement};
use view_mounts::{mounted_view_ids, mounted_view_matches};

mod view_merge;
use view_merge::merge_view_programs;

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
    #[arg(long, conflicts_with = "flow")]
    entry: Option<String>,
    #[arg(long, conflicts_with = "entry")]
    flow: Option<String>,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    executor: CliRuntimeExecutorTier,
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

impl From<&RunBundleOptions> for BundleRunnerOptions {
    fn from(options: &RunBundleOptions) -> Self {
        Self {
            entry: options.entry.clone(),
            flow: options.flow.clone(),
            executor: options.executor.into(),
            steps: options.steps,
            mode: options.mode.into(),
            max_ops: options.max_ops,
            values: options.values.clone(),
            pure_config: RuntimePureAcceleratorConfig::default(),
        }
    }
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
        LaunchKind::Cli => BundleLaunchKind::Cli,
        LaunchKind::Server => BundleLaunchKind::Server,
        LaunchKind::Test => BundleLaunchKind::Test,
        LaunchKind::Bench => BundleLaunchKind::Bench,
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
            bundle.manifest.source_label,
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
}

pub(in crate::app) fn compile_bundle_for_selection(
    selection: &SourceSelection,
    include_spaces: Vec<BundleVirtualFileSpace>,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<CompiledBundleArtifact, ExitCode> {
    let env = typecheck_env_for_selection(selection, None, phases)?;
    let compiled = compile_profile_runtime_plan(selection, &env, phases)?;
    let verification = verify_module_with_env(
        &compiled.hir,
        &env,
        VerificationPolicy {
            mode: VerificationMode::Dev,
            backend: BackendKind::Emit,
        },
    );
    if verification.has_blocking_runtime_safety_gaps() {
        emit_bundle_verification_diagnostics(selection, &verification);
        return Err(ExitCode::FAILURE);
    }
    let entry_kinds = compiled
        .plan
        .entries
        .iter()
        .map(|entry| entry.kind.clone())
        .collect::<Vec<_>>();
    let source = fs::read_to_string(selection.path()).map_err(|error| {
        eprintln!(
            "error: failed to read bundle source {}: {error}",
            selection.path().display()
        );
        ExitCode::FAILURE
    })?;
    let source_label = report_path(selection.path());
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
    let authored_resources = selection.authored_resource_roots()?;
    let virtual_files = collect_bundle_virtual_files(
        &authored_resources,
        &selection.local_state_root(),
        include_spaces,
    )?;
    let image_assets = collect_bundle_image_assets(&virtual_files)?;
    let image_declarations = parse_declared_image_objects(&source);
    let mut image_objects = bundle_image_objects(&image_declarations)?;
    let package = selection.package_identity()?;
    let fx_definitions = lower_fx_definitions_for_package(&compiled.hir, &package)
        .map_err(|error| {
            eprintln!("error: failed to compile Fx definitions: {error}");
            ExitCode::FAILURE
        })
        .and_then(|definitions| {
            FxDefinitions::try_new(definitions).map_err(|error| {
                eprintln!("error: failed to build Fx definitions inventory: {error}");
                ExitCode::FAILURE
            })
        })?;
    let view_sidecars = collect_bundle_view_sidecars(authored_resources.content())?.merged(
        collect_bundle_dsl_view_resources_for_package(&compiled.hir, &image_objects, &package)?,
    )?;
    image_objects.extend(view_sidecars.image_objects.iter().cloned());
    validate_referenced_bundle_image_assets(&compiled.plan, &image_declarations, &image_assets)?;
    let bundle = attach_bundle_view_sidecars(
        ArcweftBundle::new(
            bundle_manifest(
                selection,
                source_label.clone(),
                &compiled,
                adapter_manifest_ids,
                required_host_calls,
            ),
            BundleSource {
                label: source_label,
                text: source,
            },
            compiled.bytecode,
            compiled.line_display_catalog,
        )
        .with_product_awbc(compiled.product_awbc)
        .with_fx_definitions(fx_definitions)
        .with_adapter_manifests(adapter_manifests)
        .with_virtual_files(virtual_files)
        .with_image_assets(image_assets)
        .with_image_objects(image_objects),
        view_sidecars,
    );
    Ok(CompiledBundleArtifact {
        bundle,
        entry_kinds,
    })
}

fn emit_bundle_verification_diagnostics(selection: &SourceSelection, report: &VerificationReport) {
    let source_name = SourceName::path(selection.path().display().to_string());
    let diagnostics = report.source_diagnostics(&source_name);
    emit_diagnostics_for_path(selection.path(), &diagnostics);
}

#[derive(Clone, Debug, Default)]
struct BundleViewSidecars {
    program: Option<ViewProgramResource>,
    style: Option<ViewStyleResource>,
    text: Option<ViewTextResource>,
    input: Option<ViewInputResource>,
    theme: Option<ViewThemeResource>,
    image_objects: Vec<BundleImageObject>,
}

impl BundleViewSidecars {
    fn merged(mut self, other: Self) -> Result<Self, ExitCode> {
        self.program = match (self.program, other.program) {
            (Some(left), Some(right)) => {
                Some(merge_view_programs(left, right).map_err(|error| {
                    eprintln!("error: failed to merge View value inventories: {error}");
                    ExitCode::FAILURE
                })?)
            }
            (Some(program), None) | (None, Some(program)) => Some(program),
            (None, None) => None,
        };
        self.text = merge_optional(self.text, other.text, merge_view_text);
        self.input = merge_optional(self.input, other.input, merge_view_input);
        self.style = merge_optional(self.style, other.style, merge_view_style);
        self.theme = self.theme.or(other.theme);
        self.image_objects.extend(other.image_objects);
        Ok(self)
    }
}

fn merge_optional<T>(
    left: Option<T>,
    right: Option<T>,
    merge: impl FnOnce(T, T) -> T,
) -> Option<T> {
    match (left, right) {
        (Some(left), Some(right)) => Some(merge(left, right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn merge_view_text(mut left: ViewTextResource, right: ViewTextResource) -> ViewTextResource {
    left.sources.extend(right.sources);
    left.localized.extend(right.localized);
    left.rich_text_documents.extend(right.rich_text_documents);
    left.display_frames.extend(right.display_frames);
    left.source_ranges.extend(right.source_ranges);
    left.reveal_policies.extend(right.reveal_policies);
    left.cursor_policies.extend(right.cursor_policies);
    left.redactions.extend(right.redactions);
    left
}

fn merge_view_input(mut left: ViewInputResource, right: ViewInputResource) -> ViewInputResource {
    left.options.extend(right.options);
    left.adapter_requirements.extend(right.adapter_requirements);
    left
}

fn merge_view_style(mut left: ViewStyleResource, right: ViewStyleResource) -> ViewStyleResource {
    left.arcweft_sources.extend(right.arcweft_sources);
    left.css_sources.extend(right.css_sources);
    left.tokens.extend(right.tokens);
    left.rules.extend(right.rules);
    left.part_rules.extend(right.part_rules);
    left.environment_predicates
        .extend(right.environment_predicates);
    left.source_map_refs.extend(right.source_map_refs);
    left.external_css_descriptors
        .extend(right.external_css_descriptors);
    left.adapter_requirements.extend(right.adapter_requirements);
    left
}

fn collect_bundle_view_sidecars(root: &Path) -> Result<BundleViewSidecars, ExitCode> {
    if !root.exists() {
        return Ok(BundleViewSidecars::default());
    }

    Ok(BundleViewSidecars {
        program: read_optional_json_sidecar(&root.join("view.program.json"))?,
        style: read_optional_json_sidecar(&root.join("view.style.json"))?,
        text: read_optional_json_sidecar(&root.join("view.text.json"))?,
        input: read_optional_json_sidecar(&root.join("view.input.json"))?,
        theme: read_optional_json_sidecar(&root.join("view.theme.json"))?,
        image_objects: Vec::new(),
    })
}

fn read_optional_json_sidecar<T>(path: &Path) -> Result<Option<T>, ExitCode>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }

    let bytes = fs::read(path).map_err(|error| {
        eprintln!(
            "error: failed to read View resource sidecar {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })?;
    serde_json::from_slice(&bytes).map(Some).map_err(|error| {
        eprintln!(
            "error: failed to decode View resource sidecar {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })
}

#[cfg(test)]
fn collect_bundle_dsl_view_resources(
    module: &HirModule,
    image_objects: &[BundleImageObject],
) -> Result<BundleViewSidecars, ExitCode> {
    collect_bundle_dsl_view_resources_for_package(module, image_objects, "test-package")
}

fn collect_bundle_dsl_view_resources_for_package(
    module: &HirModule,
    image_objects: &[BundleImageObject],
    package: &str,
) -> Result<BundleViewSidecars, ExitCode> {
    let styles = module
        .declarations()
        .iter()
        .filter_map(|decl| match decl {
            HirTopLevelDecl::Style(item) => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mounted_views = mounted_view_ids(module);
    let views = module
        .declarations()
        .iter()
        .filter_map(|decl| match decl {
            HirTopLevelDecl::EntityDecl(item)
                if item.view_body().is_some()
                    && mounted_view_matches(&mounted_views, item.id().body()) =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let style = dsl_view_style_resource(&styles)?;
    let fx_definitions = lower_fx_definitions_for_package(module, package).map_err(|error| {
        eprintln!("error: failed to compile View-visible Fx definitions: {error}");
        ExitCode::FAILURE
    })?;
    let view_sidecars = view_sidecars(&views, style.as_ref(), image_objects, &fx_definitions)
        .map_err(|error| {
            eprintln!("{error}");
            ExitCode::FAILURE
        })?;
    let mut sidecars = BundleViewSidecars {
        style,
        ..BundleViewSidecars::default()
    };
    sidecars = sidecars.merged(BundleViewSidecars {
        program: view_sidecars.program,
        style: view_sidecars.style,
        text: view_sidecars.text,
        input: view_sidecars.input,
        theme: None,
        image_objects: view_sidecars.image_objects,
    })?;

    Ok(sidecars)
}

fn dsl_view_style_resource(styles: &[&StyleItem]) -> Result<Option<ViewStyleResource>, ExitCode> {
    let mut style_program_id = None;
    let mut arcweft_sources = Vec::new();
    let mut css_sources = Vec::new();
    let mut tokens = Vec::new();
    let mut rules = Vec::new();
    let mut environment_predicates = Vec::new();
    for style in styles {
        style_program_id.get_or_insert_with(|| style.id().body().to_owned());
        if let Some(source) = style.inline_source() {
            match style.syntax() {
                StyleSyntax::Arcweft => {
                    arcweft_sources.push(dsl_style_source_identity(style, source));
                }
                StyleSyntax::Css => css_sources.push(dsl_style_source_identity(style, source)),
            }
        }
        for token in style.tokens() {
            tokens.push(dsl_view_style_token(token)?);
        }
        for rule in style.rules() {
            rules.push(dsl_view_style_rule(rule)?);
        }
        environment_predicates.extend(
            style
                .environment_predicates()
                .iter()
                .map(dsl_view_style_environment_predicate),
        );
    }
    let Some(style_program_id) = style_program_id else {
        return Ok(None);
    };
    let resource = ViewStyleResource {
        style_program_id,
        arcweft_sources,
        css_sources,
        tokens,
        rules,
        part_rules: Vec::new(),
        environment_predicates,
        source_map_refs: Vec::new(),
        external_css_descriptors: Vec::new(),
        adapter_requirements: Vec::new(),
    };
    validate_view_interactive_overflow_style_rules(&resource)?;
    Ok(Some(resource))
}

fn validate_view_interactive_overflow_style_rules(
    style: &ViewStyleResource,
) -> Result<(), ExitCode> {
    for rule in &style.rules {
        let Some(element) = view_style_rule_element(rule) else {
            continue;
        };
        if element == ViewElementKind::Scroll {
            continue;
        }
        for declaration in &rule.declarations {
            if !view_style_overflow_property_is_interactive(
                style,
                &declaration.property,
                &declaration.value,
            ) {
                continue;
            }
            eprintln!(
                "error[AWF0617 view::interactive_overflow_requires_scroll]: style rule selector `{}` cannot use `{}: {}` as an interactive overflow container; wrap the content in `Scroll {{ ... }}` or move `{}` to the Scroll element",
                element.source_name(),
                normalize_view_style_property(&declaration.property),
                view_style_value_label(&declaration.value),
                normalize_view_style_property(&declaration.property)
            );
            return Err(ExitCode::FAILURE);
        }
    }
    Ok(())
}

fn view_style_rule_element(rule: &ViewStyleRule) -> Option<ViewElementKind> {
    rule.selector.parts.iter().find_map(|part| match part {
        ViewStyleSelectorPart::Element(element) => Some(*element),
        ViewStyleSelectorPart::Part(_)
        | ViewStyleSelectorPart::State(_)
        | ViewStyleSelectorPart::Interaction(_)
        | ViewStyleSelectorPart::Environment(_)
        | ViewStyleSelectorPart::Descendant
        | ViewStyleSelectorPart::Child => None,
    })
}

fn view_style_overflow_property_is_interactive(
    style: &ViewStyleResource,
    property: &str,
    value: &ViewStyleValue,
) -> bool {
    let property = normalize_view_style_property(property);
    if !matches!(property.as_str(), "overflow" | "overflow-x" | "overflow-y") {
        return false;
    }
    view_style_interactive_overflow_value(style, value, 0)
}

fn view_style_interactive_overflow_value(
    style: &ViewStyleResource,
    value: &ViewStyleValue,
    depth: u8,
) -> bool {
    if depth > 8 {
        return false;
    }
    match value {
        ViewStyleValue::Text(value) | ViewStyleValue::Resource(value) => {
            matches!(
                value.trim().trim_matches('"').trim_start_matches('.'),
                "auto" | "scroll"
            )
        }
        ViewStyleValue::Token(token) => style
            .tokens
            .iter()
            .find(|candidate| id_or_tail_matches(token, &candidate.public_id))
            .is_some_and(|token| {
                view_style_interactive_overflow_value(style, &token.value, depth + 1)
            }),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::Milli(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Digest(_) => false,
    }
}

fn view_style_value_label(value: &ViewStyleValue) -> String {
    match value {
        ViewStyleValue::Token(value)
        | ViewStyleValue::Text(value)
        | ViewStyleValue::Resource(value) => value.trim().trim_matches('"').to_owned(),
        ViewStyleValue::SystemColor(value) => format!("{value:?}"),
        ViewStyleValue::Rgba(color) => format!(
            "rgba({}, {}, {}, {})",
            color.red, color.green, color.blue, color.alpha
        ),
        ViewStyleValue::Milli(value) => value.to_string(),
        ViewStyleValue::List(values) => values
            .iter()
            .map(view_style_value_label)
            .collect::<Vec<_>>()
            .join(" "),
        ViewStyleValue::Digest(value) => value.to_string(),
    }
}

fn normalize_view_style_property(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}

fn id_or_tail_matches(candidate: &str, target: &str) -> bool {
    let candidate = candidate.trim().trim_start_matches('.');
    let target = target.trim().trim_start_matches('@');
    candidate == target
        || target
            .rsplit('.')
            .next()
            .is_some_and(|tail| candidate == tail)
}

fn dsl_style_source_identity(style: &StyleItem, source: &str) -> StyleSourceIdentity {
    let source_digest = BundleDigest::of(source.as_bytes());
    StyleSourceIdentity {
        public_id: format!("{}.source", style.id().body()),
        syntax: match style.syntax() {
            StyleSyntax::Arcweft => ProductStyleSyntax::Arcweft,
            StyleSyntax::Css => ProductStyleSyntax::Css,
        },
        identity: StyleSourceRef::Inline { source_digest },
        content_digest: Some(source_digest),
    }
}

fn dsl_view_style_token(
    token: &arcweft_lang_syntax::ast::items::ViewStyleTokenDecl,
) -> Result<ViewStyleToken, ExitCode> {
    Ok(ViewStyleToken {
        public_id: token.public_id().to_owned(),
        value: dsl_view_style_value(token.value())?,
    })
}

fn dsl_view_style_rule(
    rule: &arcweft_lang_syntax::ast::items::ViewStyleRuleDecl,
) -> Result<ViewStyleRule, ExitCode> {
    let parts = rule
        .selector()
        .iter()
        .map(dsl_view_style_selector_part)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ViewStyleRule {
        selector: ViewStyleSelector { parts },
        declarations: rule
            .declarations()
            .iter()
            .map(dsl_view_style_declaration)
            .collect::<Result<Vec<_>, _>>()?,
        source: None,
    })
}

fn dsl_view_style_selector_part(
    part: &ViewStyleSelectorPartDecl,
) -> Result<ViewStyleSelectorPart, ExitCode> {
    Ok(match part {
        ViewStyleSelectorPartDecl::Element(value) => {
            ViewStyleSelectorPart::Element(dsl_view_element_kind(value)?)
        }
        ViewStyleSelectorPartDecl::Part(value) => ViewStyleSelectorPart::Part(value.clone()),
        ViewStyleSelectorPartDecl::State(value) => {
            ViewStyleSelectorPart::State(dsl_view_element_state(value)?)
        }
        ViewStyleSelectorPartDecl::Interaction(value) => {
            ViewStyleSelectorPart::Interaction(dsl_view_interaction_state(value)?)
        }
        ViewStyleSelectorPartDecl::Descendant => ViewStyleSelectorPart::Descendant,
        ViewStyleSelectorPartDecl::Child => ViewStyleSelectorPart::Child,
    })
}

fn dsl_view_style_declaration(
    declaration: &arcweft_lang_syntax::ast::items::ViewStyleDeclarationDecl,
) -> Result<ViewStyleDeclaration, ExitCode> {
    Ok(ViewStyleDeclaration {
        property: declaration.property().to_owned(),
        value: dsl_view_style_value(declaration.value())?,
        op: dsl_view_style_assign_op(declaration.op()),
    })
}

fn dsl_view_style_assign_op(op: ViewStyleAssignOpDecl) -> StyleAssignOp {
    match op {
        ViewStyleAssignOpDecl::Replace => StyleAssignOp::Replace,
        ViewStyleAssignOpDecl::Append => StyleAssignOp::Append,
    }
}

fn dsl_view_style_value(value: &ViewStyleValueDecl) -> Result<ViewStyleValue, ExitCode> {
    Ok(match value {
        ViewStyleValueDecl::Token(value) => ViewStyleValue::Token(value.clone()),
        ViewStyleValueDecl::SystemColor(value) => {
            ViewStyleValue::SystemColor(dsl_view_system_color(value)?)
        }
        ViewStyleValueDecl::Rgba {
            red,
            green,
            blue,
            alpha,
        } => ViewStyleValue::Rgba(RgbaColor {
            red: *red,
            green: *green,
            blue: *blue,
            alpha: *alpha,
        }),
        ViewStyleValueDecl::Milli(value) => ViewStyleValue::Milli(*value),
        ViewStyleValueDecl::Text(value) => ViewStyleValue::Text(value.clone()),
        ViewStyleValueDecl::List(values) => ViewStyleValue::List(
            values
                .iter()
                .map(dsl_view_style_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        ViewStyleValueDecl::Resource(value) => ViewStyleValue::Resource(value.clone()),
    })
}

fn dsl_view_style_environment_predicate(
    predicate: &ViewStyleEnvironmentPredicateDecl,
) -> ViewEnvironmentPredicate {
    match predicate {
        ViewStyleEnvironmentPredicateDecl::TextScaleAtLeastMilli(value) => {
            ViewEnvironmentPredicate::TextScaleAtLeastMilli(*value)
        }
    }
}

fn dsl_view_element_kind(value: &str) -> Result<ViewElementKind, ExitCode> {
    if let Some(element) = ViewElementKind::from_runtime_label(value) {
        Ok(element)
    } else {
        eprintln!("error: unknown View style element selector `{value}`");
        Err(ExitCode::FAILURE)
    }
}

fn dsl_view_element_state(value: &str) -> Result<ViewElementState, ExitCode> {
    match value {
        "focus_visible" => Ok(ViewElementState::FocusVisible),
        "read_only" => Ok(ViewElementState::ReadOnly),
        "invalid" => Ok(ViewElementState::Invalid),
        "composing" => Ok(ViewElementState::Composing),
        "placeholder_shown" => Ok(ViewElementState::PlaceholderShown),
        other => {
            eprintln!("error: unknown View style element state `{other}`");
            Err(ExitCode::FAILURE)
        }
    }
}

fn dsl_view_interaction_state(value: &str) -> Result<ViewInteractionState, ExitCode> {
    match value {
        "hover" => Ok(ViewInteractionState::Hover),
        "active" => Ok(ViewInteractionState::Active),
        "disabled" => Ok(ViewInteractionState::Disabled),
        other => {
            eprintln!("error: unknown View style interaction state `{other}`");
            Err(ExitCode::FAILURE)
        }
    }
}

fn dsl_view_system_color(value: &str) -> Result<SystemColor, ExitCode> {
    match value {
        "canvas" => Ok(SystemColor::Canvas),
        "canvas_text" => Ok(SystemColor::CanvasText),
        "panel" => Ok(SystemColor::Panel),
        "panel_text" => Ok(SystemColor::PanelText),
        "raised_panel" => Ok(SystemColor::RaisedPanel),
        "muted_text" => Ok(SystemColor::MutedText),
        "border" => Ok(SystemColor::Border),
        "accent" => Ok(SystemColor::Accent),
        "accent_text" => Ok(SystemColor::AccentText),
        "focus_ring" => Ok(SystemColor::FocusRing),
        "selection" => Ok(SystemColor::Selection),
        "selection_text" => Ok(SystemColor::SelectionText),
        "danger" => Ok(SystemColor::Danger),
        "warning" => Ok(SystemColor::Warning),
        "success" => Ok(SystemColor::Success),
        other => {
            eprintln!("error: unknown View style system color `{other}`");
            Err(ExitCode::FAILURE)
        }
    }
}

fn attach_bundle_view_sidecars(
    mut bundle: ArcweftBundle,
    sidecars: BundleViewSidecars,
) -> ArcweftBundle {
    if let Some(resource) = sidecars.program {
        bundle = bundle.with_view_program(resource);
    }
    if let Some(resource) = sidecars.style {
        bundle = bundle.with_view_style(resource);
    }
    if let Some(mut resource) = sidecars.text {
        hydrate_default_view_localization(&mut resource, &bundle.display);
        bundle = bundle.with_view_text(resource);
    }
    if let Some(resource) = sidecars.input {
        bundle = bundle.with_view_input(resource);
    }
    if let Some(resource) = sidecars.theme {
        bundle = bundle.with_view_theme(resource);
    }
    bundle
}

fn hydrate_default_view_localization(
    resource: &mut ViewTextResource,
    display: &arcweft_render_text::LineDisplayCatalog,
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
        if let Some(spec) = display
            .lines()
            .iter()
            .find(|spec| spec.text_key.as_deref() == Some(key.as_str()))
        {
            resource.localized.push(ViewLocalizedTextResource {
                key,
                locale: None,
                document: spec.content.clone(),
            });
        }
    }
}

fn bundle_required_host_calls(plan: &RuntimePlan) -> Vec<String> {
    let mut required_host_calls = plan
        .flows
        .iter()
        .flat_map(|flow| flow.ops.iter())
        .flat_map(collect_flow_op_host_calls)
        .collect::<Vec<_>>();
    required_host_calls.sort();
    required_host_calls.dedup();
    required_host_calls
}

fn bundle_manifest(
    selection: &SourceSelection,
    source_label: String,
    compiled: &ProfileCompiledRuntimePlan,
    adapter_manifest_ids: Vec<String>,
    required_host_calls: Vec<String>,
) -> BundleManifest {
    BundleManifest {
        source_label,
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
            entry_flow: compiled
                .plan
                .entry_flow
                .as_ref()
                .map(|flow| flow.public_label().into_string()),
            flows: compiled.bytecode_stats.flows,
            bytecode_instructions: compiled.bytecode_stats.instructions,
            line_task_groups: compiled.bytecode_stats.line_task_groups,
            stream_plans: compiled.bytecode_stats.stream_plans,
            source_plans: compiled.bytecode_stats.source_plans,
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
        source: bundle.manifest.source_label.clone(),
        required_host_calls: bundle.manifest.required_host_calls.clone(),
        adapter_manifests: bundle.adapter_manifests.len(),
        bytecode_instructions: bundle.manifest.runtime.bytecode_instructions,
        virtual_files: bundle.virtual_files.len(),
        image_assets: bundle.image_assets.len(),
        phases,
        runtime: bundle.manifest.runtime.clone(),
    }
}

pub(super) fn run_bundle_command(
    options: &RunBundleOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let runner_options = BundleRunnerOptions::from(options);
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
        executor: RuntimeExecutorTier::from(CliRuntimeExecutorTier::from(execution.executor)),
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
    let target_bundle = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &target_bytes)
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
        BundleRunnerError::ConflictingEntrySelection
        | BundleRunnerError::ExpectedAwfbProduct { .. } => ExitCode::from(2),
        BundleRunnerError::ReadBundle { .. }
        | BundleRunnerError::DecodeBundle(_)
        | BundleRunnerError::InvalidImageAsset(_)
        | BundleRunnerError::UnsupportedBundleKind { .. }
        | BundleRunnerError::DecodeImageAsset { .. }
        | BundleRunnerError::ImageAssetMetadataMismatch { .. }
        | BundleRunnerError::DecodeBytecode(_)
        | BundleRunnerError::ProductAwbcRuntime(_)
        | BundleRunnerError::VerifyBytecode(_)
        | BundleRunnerError::CreateWorkspace(_)
        | BundleRunnerError::CreateSourceDirectory(_)
        | BundleRunnerError::MaterializeSource(_)
        | BundleRunnerError::CreateVirtualFileDirectory(_)
        | BundleRunnerError::MaterializeVirtualFile(_)
        | BundleRunnerError::InvalidVirtualFilePath
        | BundleRunnerError::UnknownFlow { .. }
        | BundleRunnerError::UnknownEntry { .. }
        | BundleRunnerError::NonFlowEntry { .. }
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
            vec![host_call_id_for_template(
                &target.capability,
                &target.operation,
            )]
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
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
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

fn collect_flow_ops_host_calls<'a>(ops: impl IntoIterator<Item = &'a FlowOp>) -> Vec<String> {
    ops.into_iter()
        .flat_map(collect_flow_op_host_calls)
        .collect()
}

fn validate_referenced_bundle_image_assets(
    plan: &RuntimePlan,
    image_declarations: &BTreeMap<String, DeclaredImageObject>,
    image_assets: &[BundleImageAsset],
) -> Result<(), ExitCode> {
    let available = image_assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<Vec<_>>();
    let missing = static_image_asset_refs(plan, image_declarations)
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

fn bundle_image_objects(
    image_declarations: &BTreeMap<String, DeclaredImageObject>,
) -> Result<Vec<BundleImageObject>, ExitCode> {
    image_declarations
        .values()
        .map(bundle_image_object)
        .collect::<Result<Vec<_>, ExitCode>>()
}

fn bundle_image_object(declaration: &DeclaredImageObject) -> Result<BundleImageObject, ExitCode> {
    let asset = declaration
        .args()
        .iter()
        .find_map(|arg| {
            let (name, value) = arg.split_once(" = ")?;
            (name.trim() == "asset")
                .then_some(value.trim())
                .and_then(public_asset_ref_arg)
        })
        .ok_or_else(|| {
            eprintln!(
                "error: image object `{}` is missing an asset reference",
                declaration.id()
            );
            ExitCode::from(2)
        })?;
    let placement = image_stage_placement(declaration)?;
    let bounds = image_design_bounds(&placement)?;
    Ok(BundleImageObject {
        id: declaration.id().to_owned(),
        asset,
        target: declaration_arg_value(declaration.args(), "target").and_then(public_id_arg),
        layer: declaration_arg_value(declaration.args(), "layer").and_then(public_id_arg),
        view: None,
        containing_scroll_region: None,
        bounds,
        placement: Some(placement),
        fit: image_fit_arg(declaration),
        alignment: image_alignment_arg(declaration),
        playback: image_playback_arg(declaration),
        transform: image_transform_arg(declaration),
        depth_milli: declaration_arg_value(declaration.args(), "depth")
            .and_then(parse_depth_arg)
            .unwrap_or_default(),
        opacity_milli: image_opacity_milli_arg(declaration)?,
        actions: image_actions_arg(declaration),
        params: image_params_arg(declaration),
        proxies: image_proxies_arg(declaration),
        visible: declaration_arg_value(declaration.args(), "visible")
            .and_then(parse_bool_arg)
            .unwrap_or(true),
    })
}

fn image_fit_arg(declaration: &DeclaredImageObject) -> BundleImageObjectFit {
    match declaration_arg_value(declaration.args(), "fit").map(unquote_arg) {
        Some("cover") => BundleImageObjectFit::Cover,
        Some("stretch") => BundleImageObjectFit::Stretch,
        Some("intrinsic") => BundleImageObjectFit::Intrinsic,
        _ => BundleImageObjectFit::Contain,
    }
}

fn image_alignment_arg(declaration: &DeclaredImageObject) -> BundleImageObjectAlignment {
    BundleImageObjectAlignment {
        x_milli: declaration_arg_value(declaration.args(), "alignment.x")
            .or_else(|| declaration_arg_value(declaration.args(), "align.x"))
            .and_then(|value| parse_alignment_component_milli(value, "x"))
            .unwrap_or(500),
        y_milli: declaration_arg_value(declaration.args(), "alignment.y")
            .or_else(|| declaration_arg_value(declaration.args(), "align.y"))
            .and_then(|value| parse_alignment_component_milli(value, "y"))
            .unwrap_or(500),
    }
}

fn parse_alignment_component_milli(value: &str, axis: &str) -> Option<i32> {
    match (axis, unquote_arg(value)) {
        ("x", "left" | "start") | ("y", "top" | "start") => return Some(0),
        ("x" | "y", "center" | "middle") => return Some(500),
        ("x", "right" | "end") | ("y", "bottom" | "end") => return Some(1_000),
        _ => {}
    }
    let integer = unquote_arg(value).parse::<i32>().ok()?;
    Some(if (0..=1).contains(&integer) {
        integer.saturating_mul(1_000)
    } else {
        integer.clamp(0, 1_000)
    })
}

fn image_playback_arg(declaration: &DeclaredImageObject) -> BundleImageObjectPlayback {
    BundleImageObjectPlayback {
        start_time_millis: declaration_arg_value(declaration.args(), "playback.start")
            .or_else(|| declaration_arg_value(declaration.args(), "playback.start_time"))
            .and_then(parse_duration_millis)
            .unwrap_or_default(),
        rate_milli: declaration_arg_value(declaration.args(), "playback.rate")
            .and_then(parse_rate_milli)
            .unwrap_or(1_000),
        paused_at_millis: declaration_arg_value(declaration.args(), "playback.paused_at")
            .and_then(parse_duration_millis),
        pinned_local_time_millis: declaration_arg_value(declaration.args(), "playback.local_time")
            .or_else(|| declaration_arg_value(declaration.args(), "playback.pinned_local_time"))
            .and_then(parse_duration_millis),
    }
}

fn image_transform_arg(declaration: &DeclaredImageObject) -> BundleImageObjectTransform {
    BundleImageObjectTransform {
        m11_milli: declaration_arg_value(declaration.args(), "transform.m11")
            .and_then(parse_milli_arg)
            .unwrap_or(1_000),
        m12_milli: declaration_arg_value(declaration.args(), "transform.m12")
            .and_then(parse_milli_arg)
            .unwrap_or_default(),
        m21_milli: declaration_arg_value(declaration.args(), "transform.m21")
            .and_then(parse_milli_arg)
            .unwrap_or_default(),
        m22_milli: declaration_arg_value(declaration.args(), "transform.m22")
            .and_then(parse_milli_arg)
            .unwrap_or(1_000),
        tx_milli: declaration_arg_value(declaration.args(), "transform.tx")
            .and_then(parse_px_milli)
            .unwrap_or_default(),
        ty_milli: declaration_arg_value(declaration.args(), "transform.ty")
            .and_then(parse_px_milli)
            .unwrap_or_default(),
    }
}

fn image_actions_arg(declaration: &DeclaredImageObject) -> Vec<String> {
    declaration
        .args()
        .iter()
        .filter_map(|arg| {
            let (name, value) = arg.split_once(" = ")?;
            match name.trim() {
                "action" => Some(vec![public_id_arg(value).unwrap_or_else(|| {
                    unquote_arg(value.trim().trim_start_matches('@')).to_owned()
                })]),
                "actions" => Some(
                    unquote_arg(value)
                        .trim_matches(['[', ']'])
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| {
                            public_id_arg(value).unwrap_or_else(|| {
                                unquote_arg(value.trim().trim_start_matches('@')).to_owned()
                            })
                        })
                        .collect(),
                ),
                _ => None,
            }
        })
        .flatten()
        .collect()
}

fn image_params_arg(declaration: &DeclaredImageObject) -> BTreeMap<String, BundleImageObjectParam> {
    declaration
        .args()
        .iter()
        .filter_map(|arg| {
            let (name, value) = arg.split_once(" = ")?;
            let name = name.trim();
            name.strip_prefix("param.").map(|_| {
                (
                    name.to_owned(),
                    bundle_image_object_param_from_arg(value.trim()),
                )
            })
        })
        .collect()
}

fn image_proxies_arg(declaration: &DeclaredImageObject) -> Vec<BundleImageObjectProxy> {
    let Some(id) = declaration_arg_value(declaration.args(), "proxy.id").and_then(public_id_arg)
    else {
        return Vec::new();
    };
    let params = declaration
        .args()
        .iter()
        .filter_map(|arg| {
            let (name, value) = arg.split_once(" = ")?;
            name.trim().strip_prefix("proxy.param.").map(|name| {
                (
                    name.to_owned(),
                    bundle_image_object_param_from_arg(value.trim()),
                )
            })
        })
        .collect();
    vec![BundleImageObjectProxy {
        id,
        type_name: declaration_arg_value(declaration.args(), "proxy.type")
            .map(unquote_arg)
            .map(str::to_owned),
        role: declaration_arg_value(declaration.args(), "proxy.role")
            .map(unquote_arg)
            .map(str::to_owned),
        layer: declaration_arg_value(declaration.args(), "proxy.layer").and_then(public_id_arg),
        depth_milli: declaration_arg_value(declaration.args(), "proxy.depth")
            .and_then(parse_depth_arg),
        hit_test: declaration_arg_value(declaration.args(), "proxy.hit_test")
            .and_then(parse_bool_arg)
            .unwrap_or_default(),
        params,
    }]
}

fn bundle_image_object_param_from_arg(value: &str) -> BundleImageObjectParam {
    let trimmed = value.trim();
    if let Some(value) = parse_bool_arg(trimmed) {
        return BundleImageObjectParam::Bool { value };
    }
    if let Ok(value) = unquote_arg(trimmed).parse::<i64>() {
        return BundleImageObjectParam::Integer { value };
    }
    if unquote_arg(trimmed).ends_with('%')
        && let Some(value) = parse_milli_arg(trimmed)
    {
        return BundleImageObjectParam::Milli { value };
    }
    if trimmed.trim_start().starts_with('@')
        && let Some(value) = public_id_arg(trimmed)
    {
        return BundleImageObjectParam::Id { value };
    }
    BundleImageObjectParam::Text {
        value: unquote_arg(trimmed).to_owned(),
    }
}

fn parse_bool_arg(value: &str) -> Option<bool> {
    match unquote_arg(value) {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_rate_milli(value: &str) -> Option<u32> {
    let milli = parse_milli_arg(value)?;
    u32::try_from(milli.max(0)).ok()
}

fn parse_depth_arg(value: &str) -> Option<i32> {
    rounded_i32(unquote_arg(value).parse::<f64>().ok()?)
}

fn parse_milli_arg(value: &str) -> Option<i32> {
    let value = unquote_arg(value);
    if let Some(percent) = value.strip_suffix('%') {
        let parsed = percent.trim().parse::<f64>().ok()?;
        return rounded_i32(parsed * 10.0);
    }
    let parsed = value.parse::<f64>().ok()?;
    rounded_i32(parsed * 1_000.0)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn parse_duration_millis(value: &str) -> Option<u64> {
    let value = unquote_arg(value);
    let millis = if let Some(ms) = value.strip_suffix("ms") {
        ms.trim().parse::<f64>().ok()?
    } else if let Some(seconds) = value.strip_suffix('s') {
        seconds.trim().parse::<f64>().ok()? * 1_000.0
    } else {
        value.parse::<f64>().ok()?
    };
    let millis = millis.round();
    millis
        .is_finite()
        .then_some(millis.clamp(0.0, u64::MAX as f64) as u64)
}

#[allow(clippy::cast_possible_truncation)]
fn rounded_i32(value: f64) -> Option<i32> {
    let rounded = value.round();
    rounded
        .is_finite()
        .then_some(rounded.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32)
}

fn unquote_arg(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('\'')
}

fn parse_px_milli(value: &str) -> Option<i32> {
    let pixels = unquote_arg(value).strip_suffix("px")?.trim();
    let (whole, fraction) = pixels.split_once('.').unwrap_or((pixels, ""));
    let sign = whole.starts_with('-');
    let whole_abs = whole.trim_start_matches('-');
    let whole_milli = whole_abs.parse::<i32>().ok()?.checked_mul(1_000)?;
    let fraction_milli = fraction
        .chars()
        .take(3)
        .try_fold((0_i32, 100_i32), |(value, scale), ch| {
            let digit = ch.to_digit(10)?;
            Some((value + i32::try_from(digit).ok()? * scale, scale / 10))
        })?
        .0;
    let milli = whole_milli.checked_add(fraction_milli)?;
    Some(if sign { -milli } else { milli })
}

fn image_opacity_milli_arg(declaration: &DeclaredImageObject) -> Result<u16, ExitCode> {
    let Some(value) = declaration_arg_value(declaration.args(), "opacity") else {
        return Ok(1_000);
    };
    let Some(milli) = parse_opacity_milli(value) else {
        eprintln!(
            "error: image object `{}` has invalid `opacity` value `{value}`",
            declaration.id()
        );
        return Err(ExitCode::from(2));
    };
    Ok(milli)
}

fn parse_opacity_milli(value: &str) -> Option<u16> {
    let value = value.trim();
    if let Some(milli) = value.strip_suffix("milli") {
        return milli
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|value| *value <= 1_000);
    }
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    let whole = whole.parse::<u16>().ok()?;
    let fraction_milli = fraction
        .chars()
        .take(3)
        .try_fold((0_u16, 100_u16), |(value, scale), ch| {
            let digit = ch.to_digit(10)?;
            Some((value + u16::try_from(digit).ok()? * scale, scale / 10))
        })?
        .0;
    let milli = whole.checked_mul(1_000)?.checked_add(fraction_milli)?;
    (milli <= 1_000).then_some(milli)
}

fn static_image_asset_refs(
    plan: &RuntimePlan,
    image_declarations: &BTreeMap<String, DeclaredImageObject>,
) -> Vec<String> {
    let mut refs = plan
        .flows
        .iter()
        .flat_map(|flow| flow.ops.iter())
        .flat_map(collect_flow_op_static_image_asset_refs)
        .chain(
            plan.line_task_groups
                .iter()
                .flat_map(collect_line_task_group_static_image_asset_refs),
        )
        .collect::<Vec<_>>();
    refs.extend(declared_image_asset_refs(image_declarations));
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
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
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
    match expr {
        RuntimeExpr::EntityRef(id) => Some(id.clone()),
        RuntimeExpr::Value(RuntimeValue::EntityRef(id) | RuntimeValue::String(id)) => {
            Some(id.clone())
        }
        RuntimeExpr::Value(_)
        | RuntimeExpr::Local(_)
        | RuntimeExpr::Let { .. }
        | RuntimeExpr::Tuple(_)
        | RuntimeExpr::BracketSeq(_)
        | RuntimeExpr::RepeatSeq { .. }
        | RuntimeExpr::Range { .. }
        | RuntimeExpr::Record(_)
        | RuntimeExpr::Variant { .. }
        | RuntimeExpr::Field { .. }
        | RuntimeExpr::ProjectTuple { .. }
        | RuntimeExpr::ProjectRecord { .. }
        | RuntimeExpr::AssignField { .. }
        | RuntimeExpr::Call { .. }
        | RuntimeExpr::Function { .. }
        | RuntimeExpr::Apply { .. }
        | RuntimeExpr::TraitCall { .. }
        | RuntimeExpr::PureCall { .. }
        | RuntimeExpr::SpreadArg(_)
        | RuntimeExpr::MethodCall { .. }
        | RuntimeExpr::Map { .. }
        | RuntimeExpr::Filter { .. }
        | RuntimeExpr::Sum { .. }
        | RuntimeExpr::Unary { .. }
        | RuntimeExpr::Binary { .. }
        | RuntimeExpr::If { .. }
        | RuntimeExpr::IfLet { .. }
        | RuntimeExpr::Match { .. } => None,
    }
}

fn collect_line_task_group_static_image_asset_refs(group: &LineTaskGroup) -> Vec<String> {
    collect_line_task_scope_static_image_asset_refs(&group.root)
}

fn collect_line_task_scope_static_image_asset_refs(scope: &LineTaskScope) -> Vec<String> {
    collect_line_task_node_static_image_asset_refs(&scope.node)
        .into_iter()
        .chain(collect_line_effects_static_image_asset_refs(
            scope.defer_stack.iter().flatten(),
        ))
        .chain(collect_line_effects_static_image_asset_refs(
            scope.completed_defer_stack.iter().flatten(),
        ))
        .chain(collect_line_effects_static_image_asset_refs(
            scope.cancelled_defer_stack.iter().flatten(),
        ))
        .chain(collect_line_effects_static_image_asset_refs(
            scope.failed_defer_stack.iter().flatten(),
        ))
        .collect()
}

fn collect_line_task_node_static_image_asset_refs(node: &LineTaskNode) -> Vec<String> {
    match node {
        LineTaskNode::Seq(nodes) | LineTaskNode::Start(nodes) => nodes
            .iter()
            .flat_map(collect_line_task_node_static_image_asset_refs)
            .collect(),
        LineTaskNode::Parallel { children, .. } => children
            .iter()
            .flat_map(collect_line_task_node_static_image_asset_refs)
            .collect(),
        LineTaskNode::Child(child) => collect_line_child_task_static_image_asset_refs(child),
        LineTaskNode::Effect(effect) => collect_line_effect_static_image_asset_refs(effect),
    }
}

fn collect_line_child_task_static_image_asset_refs(child: &LineChildTask) -> Vec<String> {
    collect_line_task_scope_static_image_asset_refs(&child.scope)
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
        "bg" | "image" | "image.show" => runtime_call_asset_arg(call, 0),
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
    public_asset_ref_arg(arg)
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
    Ok(assets)
}

fn bundle_image_asset_from_virtual_file(
    file: &BundleVirtualFile,
) -> Result<Option<BundleImageAsset>, ExitCode> {
    let Some(format) = bundle_image_format_from_path(&file.path) else {
        return Ok(None);
    };
    let Some(id) = bundle_asset_id_from_virtual_path(&file.path) else {
        return Ok(None);
    };
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
        id,
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

fn bundle_asset_id_from_virtual_path(path: &str) -> Option<String> {
    let without_extension = path.rsplit_once('.').map_or(path, |(stem, _)| stem);
    let parts = without_extension
        .split('/')
        .filter(|part| !part.is_empty())
        .map(bundle_asset_id_component)
        .collect::<Option<Vec<_>>>()?;
    (!parts.is_empty()).then(|| format!("asset.{}", parts.join(".")))
}

fn bundle_asset_id_component(value: &str) -> Option<String> {
    let component = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else if matches!(ch, '_' | '-') {
                '_'
            } else {
                '\0'
            }
        })
        .collect::<String>();
    (!component.is_empty()
        && component
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_lowercase() || ch.is_ascii_digit()))
    .then_some(component)
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
