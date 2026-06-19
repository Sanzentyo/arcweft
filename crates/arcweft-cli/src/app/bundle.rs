use super::project::{
    ProfileOptions, SourceSelection, adapter_manifest_for_selection, resolve_source_selection,
    typecheck_env_for_selection,
};
use super::runtime::{
    BundleCommandReport, BundleRunReport, ProfileCompiledRuntimePlan, compile_profile_runtime_plan,
    report_path, run_profile_phase,
};
use super::runtime::{CliRuntimeExecutorTier, CliRuntimeStepMode, parse_runtime_binding_arg};
use super::shared::print_json;
use crate::output::{RuntimeExecutorTier, RuntimeProfilePhase};
use arcweft_adapter_context::{manifest::AdapterManifest, standard};
use arcweft_bundle::{
    ArcweftBundle, BundleAdapterHostCall, BundleAdapterManifest, BundleImageAnimation,
    BundleImageAsset, BundleImageFormat, BundleLaunchKind, BundleManifest, BundleRuntimeSummary,
    BundleSource, BundleVirtualFile, BundleVirtualFileRef, BundleVirtualFileSpace,
};
use arcweft_core::{
    effect::{LineEffectRequest, RuntimeCall},
    line_task::{LineChildTask, LineTaskGroup, LineTaskNode, LineTaskScope},
    plan::{FlowOp, RuntimePlan},
    value::{RuntimeBinding, RuntimeExpr, RuntimeValue},
};
use arcweft_launch::LaunchKind;
use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
use arcweft_runtime_host::{
    BundleRunnerError, BundleRunnerOptions, INTERNAL_SCHEDULER_ADAPTER_ID, NativeAdapterRegistrar,
    internal_scheduler_manifest, run_bundle_file_with_native_adapters,
};
use clap::Args;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct BundleOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    virtual_files: BundleVirtualFileOptions,
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
    let bundle = compile_bundle_artifact(&selection, options, &mut phases)?;
    let bytes = run_profile_phase(&mut phases, "encode_bundle", || {
        bundle.to_json_bytes().map_err(|error| {
            eprintln!("error: failed to encode bundle: {error}");
            ExitCode::FAILURE
        })
    })?;
    write_bundle_artifact(&options.output, bytes, &mut phases)?;
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
    let env = typecheck_env_for_selection(selection, None, phases)?;
    let compiled = compile_profile_runtime_plan(selection, &env, phases)?;
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
    );
    let virtual_files = collect_bundle_virtual_files(selection.path(), options.include_spaces())?;
    let image_assets = collect_bundle_image_assets(&virtual_files);
    validate_referenced_bundle_image_assets(&compiled.plan, &image_assets)?;
    Ok(ArcweftBundle::new(
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
    .with_adapter_manifests(adapter_manifests)
    .with_virtual_files(virtual_files)
    .with_image_assets(image_assets))
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
            entry_flow: compiled.plan.entry_flow.as_ref().map(|flow| flow.0.clone()),
            flows: compiled.bytecode_stats.flows,
            bytecode_instructions: compiled.bytecode_stats.instructions,
            line_task_groups: compiled.bytecode_stats.line_task_groups,
            stream_plans: compiled.bytecode_stats.stream_plans,
            source_plans: compiled.bytecode_stats.source_plans,
        },
    }
}

fn write_bundle_artifact(
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
    let execution =
        run_bundle_file_with_native_adapters(&options.bundle, &runner_options, adapter_registrars)
            .map_err(|error| {
                eprintln!("error: {error}");
                bundle_runner_error_exit_code(&error)
            })?;
    let report = BundleRunReport {
        bundle: report_path(&options.bundle),
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

fn bundle_runner_error_exit_code(error: &BundleRunnerError) -> ExitCode {
    match error {
        BundleRunnerError::ConflictingEntrySelection => ExitCode::from(2),
        BundleRunnerError::ReadBundle { .. }
        | BundleRunnerError::DecodeBundle(_)
        | BundleRunnerError::InvalidImageAsset(_)
        | BundleRunnerError::DecodeBytecode(_)
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
        FlowOp::Effect(effect) => collect_line_effect_static_image_asset_refs(effect),
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
        | RuntimeExpr::Record(_)
        | RuntimeExpr::Variant { .. }
        | RuntimeExpr::Field { .. }
        | RuntimeExpr::ProjectTuple { .. }
        | RuntimeExpr::ProjectRecord { .. }
        | RuntimeExpr::Call { .. }
        | RuntimeExpr::PureCall { .. }
        | RuntimeExpr::SpreadArg(_)
        | RuntimeExpr::MethodCall { .. }
        | RuntimeExpr::Map { .. }
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
        | LineEffectRequest::Continue { .. } => Vec::new(),
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
    let value = arg.trim().trim_matches('"').trim_matches('\'');
    let value = value.strip_prefix('@').unwrap_or(value);
    value
        .starts_with("asset.")
        .then(|| value.to_owned())
        .filter(|value| value.chars().all(public_asset_ref_char))
}

fn public_asset_ref_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')
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
) -> Vec<BundleAdapterManifest> {
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
    manifests.sort_by(|left, right| left.id.cmp(&right.id));
    manifests.dedup_by(|left, right| left.id == right.id);
    manifests
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
    source_path: &Path,
    spaces: impl IntoIterator<Item = BundleVirtualFileSpace>,
) -> Result<Vec<BundleVirtualFile>, ExitCode> {
    let root = source_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".arcweft");
    spaces
        .into_iter()
        .map(|space| collect_bundle_virtual_files_for_space(&root, space))
        .collect::<Result<Vec<_>, _>>()
        .map(|groups| groups.into_iter().flatten().collect())
}

fn collect_bundle_image_assets(files: &[BundleVirtualFile]) -> Vec<BundleImageAsset> {
    let mut assets = files
        .iter()
        .filter(|file| file.space == BundleVirtualFileSpace::Asset)
        .filter_map(bundle_image_asset_from_virtual_file)
        .collect::<Vec<_>>();
    assets.sort_by(|left, right| left.id.cmp(&right.id));
    assets.dedup_by(|left, right| left.id == right.id);
    assets
}

fn bundle_image_asset_from_virtual_file(file: &BundleVirtualFile) -> Option<BundleImageAsset> {
    let format = bundle_image_format_from_path(&file.path)?;
    Some(BundleImageAsset {
        id: bundle_asset_id_from_virtual_path(&file.path)?,
        file: BundleVirtualFileRef {
            space: file.space,
            path: file.path.clone(),
        },
        format,
        animation: bundle_image_animation_for_format(format),
        dimensions: None,
    })
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

const fn bundle_image_animation_for_format(format: BundleImageFormat) -> BundleImageAnimation {
    match format {
        BundleImageFormat::Gif | BundleImageFormat::WebP => BundleImageAnimation::Animated,
        BundleImageFormat::Png | BundleImageFormat::Jpeg => BundleImageAnimation::Static,
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
    root: &Path,
    space: BundleVirtualFileSpace,
) -> Result<Vec<BundleVirtualFile>, ExitCode> {
    let dir = root.join(space.as_str());
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_bundle_virtual_files_from_dir(&dir, &dir, space, &mut files)?;
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
mod tests {
    use super::*;
    use arcweft_core::plan::{FlowRuntimeId, RuntimeFlow};
    use arcweft_core::task::{
        AwaitTarget, HostTaskArgTemplate, HostTaskRequestTemplate, NeedId, TaskId,
    };

    fn image_await(id: &str) -> FlowOp {
        FlowOp::Await {
            binding: None,
            target: AwaitTarget::new(
                NeedId(format!("need.{id}")),
                TaskId(format!("task.{id}")),
                HostTaskRequestTemplate::new(
                    "asset",
                    "image",
                    [HostTaskArgTemplate::positional(RuntimeExpr::EntityRef(
                        id.to_owned(),
                    ))],
                ),
            ),
            pending: Vec::new(),
        }
    }

    fn image_effect_call(callee: &str, arg: &str) -> FlowOp {
        FlowOp::Effect(LineEffectRequest::Call(RuntimeCall {
            callee: callee.to_owned(),
            args: vec![arg.to_owned()],
        }))
    }

    fn plan_with_ops(ops: Vec<FlowOp>) -> RuntimePlan {
        RuntimePlan {
            flows: vec![RuntimeFlow {
                id: FlowRuntimeId("flow.test".to_owned()),
                ops,
            }],
            ..RuntimePlan::default()
        }
    }

    fn plan_with_line_task(effect: LineEffectRequest) -> RuntimePlan {
        RuntimePlan {
            line_task_groups: vec![LineTaskGroup {
                root: LineTaskScope {
                    node: LineTaskNode::Effect(effect),
                    ..LineTaskScope::default()
                },
                ..LineTaskGroup::default()
            }],
            ..RuntimePlan::default()
        }
    }

    fn image_asset(id: &str) -> BundleImageAsset {
        BundleImageAsset {
            id: id.to_owned(),
            file: BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Asset,
                path: "bg/room.png".to_owned(),
            },
            format: BundleImageFormat::Png,
            animation: BundleImageAnimation::Static,
            dimensions: None,
        }
    }

    #[test]
    fn static_image_asset_refs_collects_nested_asset_image_entity_refs() {
        let plan = plan_with_ops(vec![FlowOp::If {
            condition: RuntimeExpr::Value(RuntimeValue::Bool(true)),
            then_ops: vec![image_await("asset.bg.room")],
            else_ops: vec![image_effect_call("image", "asset = @asset.ui.logo")],
        }]);

        assert_eq!(
            static_image_asset_refs(&plan),
            vec!["asset.bg.room".to_owned(), "asset.ui.logo".to_owned()]
        );
    }

    #[test]
    fn static_image_asset_refs_collects_runtime_presentation_image_calls() {
        let plan = plan_with_ops(vec![
            image_effect_call("bg", "@asset.bg.room"),
            image_effect_call("image.show", "asset = \"asset.ui.logo\""),
            FlowOp::Await {
                binding: None,
                target: AwaitTarget::new(
                    NeedId("need.unrelated".to_owned()),
                    TaskId("task.unrelated".to_owned()),
                    HostTaskRequestTemplate::new("system", "info", []),
                ),
                pending: vec![LineEffectRequest::Call(RuntimeCall {
                    callee: "image".to_owned(),
                    args: vec!["asset = @asset.bg.pulse".to_owned()],
                })],
            },
        ]);

        assert_eq!(
            static_image_asset_refs(&plan),
            vec![
                "asset.bg.pulse".to_owned(),
                "asset.bg.room".to_owned(),
                "asset.ui.logo".to_owned()
            ]
        );
    }

    #[test]
    fn static_image_asset_refs_collects_line_task_image_calls() {
        let plan = plan_with_line_task(LineEffectRequest::Call(RuntimeCall {
            callee: "bg".to_owned(),
            args: vec!["@asset.bg.room".to_owned()],
        }));

        assert_eq!(static_image_asset_refs(&plan), vec!["asset.bg.room"]);
    }

    #[test]
    fn validate_referenced_bundle_image_assets_rejects_missing_static_refs() {
        let plan = plan_with_ops(vec![
            image_await("asset.bg.room"),
            image_effect_call("image", "asset = @asset.ui.logo"),
        ]);

        assert!(validate_referenced_bundle_image_assets(&plan, &[]).is_err());
        assert!(
            validate_referenced_bundle_image_assets(
                &plan,
                &[image_asset("asset.bg.room"), image_asset("asset.ui.logo")]
            )
            .is_ok()
        );
    }
}
