use super::{
    AdapterManifest, ArcweftBundle, BundleAdapterHostCall, BundleAdapterManifest,
    BundleCommandReport, BundleManifest, BundleOptions, BundleRunReport, BundleRunnerError,
    BundleRunnerOptions, BundleRuntimeSummary, BundleSource, BundleVirtualFile,
    BundleVirtualFileSpace, CliRuntimeExecutorTier, Component, ExitCode, FlowOp,
    INTERNAL_SCHEDULER_ADAPTER_ID, NativeAdapterRegistrar, Path, ProfileCompiledRuntimePlan,
    RunBundleOptions, RuntimeExecutorTier, RuntimePlan, RuntimeProfilePhase, SourceSelection,
    adapter_manifest_for_selection, bundle_launch_kind, compile_profile_runtime_plan, fs,
    internal_scheduler_manifest, print_json, report_path, resolve_source_selection,
    run_bundle_file_with_native_adapters, run_profile_phase, standard, typecheck_env_for_selection,
};

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
    .with_virtual_files(collect_bundle_virtual_files(
        selection.path(),
        options.include_spaces(),
    )?))
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
