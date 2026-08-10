use super::{
    AGENT_DEBUG_RUNTIME_STALE_AFTER_MILLIS, AGENT_DEBUG_RUNTIME_STALE_REASON, ActionResult,
    AgentAction, AgentControllerRunConfig, AgentControllerRunReport, AgentHostResponse,
    AgentProjectGraph, AgentResource, AgentResourceBody, AgentResourceKind, AgentResourceUri,
    AgentRunId, AgentRunner, AgentRunnerConfig, AgentScriptBuildOptions, AgentScriptCheckOptions,
    AgentScriptCommand, AgentScriptReplayOptions, AgentScriptRunOptions, AgentScriptSignalArg,
    AgentScriptStateArg, AgentScriptTraceOptions, AgentSession, AgentSessionInfo, AgentTraceKind,
    AgentTraceRecord, AgentValue, ArcweftBundle, BTreeMap, BTreeSet, BundleKind, CaptureFormat,
    CaptureRequest, CaptureResult, DebugEvent, DebugEventKind, DebugEventSink, DebugScriptRun,
    DebugScriptRunFinish, DebugScriptRunOutcome, DebugSession, DebugSessionStatus, DebugStore,
    EntityKind, EntitySymbol, EntityType, ExitCode, Infallible, NativeAdapterRegistrar,
    NoopRagService, ObservationEnvelope, ObserveRequest, Path, PathBuf, ProjectSemanticIndex,
    RequiredEntity, RuntimeAgentCapability, RuntimeAgentPolicy, SemaPublicId, SemanticHash,
    SessionId, SourceAnchor, StableHash, SystemTime, TypeKind, UNIX_EPOCH, agent, agent_project,
    fs, print_json,
};
use arcweft_compiler::{
    incremental::{BuildSnapshotRequest, runtime_plan_artifact_key, snapshot_compiled_project},
    project::{
        ProjectCompilationContext, ProjectCompilationSession, ProjectCompileError,
        ProjectEntrySelection, ProjectEntrySelectionKind, compile_project,
    },
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{
    env::TypeCheckEnv, project_index::ProjectEntityId, registration::ProjectRegistrationFacts,
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase, parser::ParseOptions,
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId};
use arcweft_tooling::runtime_diagnostic::{
    project_persisted_assertion_failure, project_runtime_assertion_fault,
};
use std::sync::Arc;

#[cfg(feature = "native-capture")]
use super::{load_and_check_selection, native, resolve_source_selection};

pub(super) fn agent_script_command(
    command: AgentScriptCommand,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    match command {
        AgentScriptCommand::Build(options) => agent_script_build_command(&options),
        AgentScriptCommand::Check(options) => agent_script_check_command(&options),
        AgentScriptCommand::Replay(options) => agent_script_replay_command(&options),
        AgentScriptCommand::Run(options) => agent_script_run_command(&options, adapter_registrars),
        AgentScriptCommand::Trace(options) => agent_script_trace_command(&options),
    }
}

#[derive(serde::Serialize)]
pub(super) struct AgentScriptCheckReport {
    pub(super) path: String,
    pub(super) ok: bool,
    pub(super) agent_entries: usize,
    pub(super) error: Option<String>,
}

#[derive(serde::Serialize)]
pub(super) struct AgentScriptBuildReport {
    pub(super) path: String,
    pub(super) output: String,
    pub(super) ok: bool,
    pub(super) agent_entries: usize,
    pub(super) entry_id: Option<String>,
    pub(super) controller_id: Option<String>,
    pub(super) bundle_kind: Option<String>,
    pub(super) bytecode_instructions: usize,
    pub(super) bytes: usize,
    pub(super) error: Option<String>,
}

#[derive(Clone)]
pub(super) struct AgentScriptCompileTarget {
    typecheck_environment: Arc<TypeCheckEnv>,
    target_entities: Vec<EntitySymbol>,
    accepted_index: Option<Arc<ProjectSemanticIndex>>,
}

pub(super) struct CompiledAgentScriptSource {
    pub(super) artifact: arcweft_compiler::types::CompiledAgentBundle,
    semantic_index: ProjectSemanticIndex,
}

#[expect(
    clippy::too_many_lines,
    reason = "Agent script compilation publishes one atomic typed project and artifact transaction"
)]
pub(super) fn compile_agent_script_source(
    source_name: &Path,
    source: String,
    selected_entry: &str,
    target: &AgentScriptCompileTarget,
) -> Result<CompiledAgentScriptSource, String> {
    const PACKAGE_NAME: &str = "org.arcweft.tool.agent-script";

    let selected_entry =
        SemaPublicId::try_new(selected_entry.to_owned()).map_err(|error| error.to_string())?;
    let source_path = PathBuf::from("src/controller.arcw");
    let source_key = blake3::hash(source_name.to_string_lossy().as_bytes()).to_hex();
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-agent-script://{source_key}"))
                .map_err(|error| error.to_string())?,
            SourceName::path(source_name.display().to_string()),
            source,
        )
        .map_err(|error| error.to_string())?,
    );
    let mut syntax = SyntaxDatabase::try_new().map_err(|error| error.to_string())?;
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .map_err(|error| error.to_string())?;
    let parsed_sources = BTreeMap::from([(CanonicalModulePath::crate_root(), parsed)]);
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new(PACKAGE_NAME).map_err(|error| error.to_string())?,
            version: PackageVersion::new("0.0.0").map_err(|error| error.to_string())?,
        },
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-agent-script://manifest")
                    .map_err(|error| error.to_string())?,
                SourceName::Memory,
                "",
            )
            .map_err(|error| error.to_string())?,
        ),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            source_path,
            Arc::clone(&document),
            [],
        )],
    )
    .map_err(|error| error.to_string())?;
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(PACKAGE_NAME).map_err(|error| error.to_string())?,
        document.identity().id().clone(),
        format!("agent-script:{}", selected_entry.as_str()),
    )
    .map_err(|error| error.to_string())?;
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![document],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .map_err(|report| {
        report
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.diagnostic().message().to_owned())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let context = ProjectCompilationContext::new(
        Arc::clone(&target.typecheck_environment),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        Some(ProjectEntrySelection::new(
            selected_entry.clone(),
            ProjectEntrySelectionKind::Agent,
        )),
    );
    let mut compilation_session =
        ProjectCompilationSession::try_new().map_err(|error| error.to_string())?;
    let compiled = compile_project(
        &mut compilation_session,
        &project,
        &parsed_sources,
        &context,
    )
    .map_err(|error| project_compile_message(&error))?;
    let semantic_index = target.target_entities.iter().cloned().fold(
        compiled.semantic_index().as_ref().clone(),
        ProjectSemanticIndex::with_entity,
    );
    let snapshot = snapshot_compiled_project(
        &project,
        &compiled,
        BuildSnapshotRequest {
            build_id: compiled.program_hash().as_str().to_owned(),
            compiler_build_id: env!("CARGO_PKG_VERSION").to_owned(),
            target_triple: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            target_features: Vec::new(),
            profile: "agent-script".to_owned(),
            selected_entries: vec![selected_entry.as_str().to_owned()],
        },
    );
    let runtime_plan_artifact_key = runtime_plan_artifact_key(&snapshot, &compiled);
    let artifact = agent::compile_agent_project_bundle(
        &compiled,
        &selected_entry,
        &semantic_index,
        runtime_plan_artifact_key,
    )
    .map_err(|error| error.to_string())?;
    Ok(CompiledAgentScriptSource {
        artifact,
        semantic_index,
    })
}

fn project_compile_message(error: &ProjectCompileError) -> String {
    let stage = error
        .diagnostics()
        .first()
        .map_or(error.stage(), |diagnostic| diagnostic.stage().as_str());
    let details = error
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            let diagnostic = diagnostic.diagnostic();
            match diagnostic.code() {
                Some(code) => format!("{}: {}", code.as_str(), diagnostic.message()),
                None => diagnostic.message().to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("{stage}: {details}")
}

pub(super) fn agent_script_build_command(
    options: &AgentScriptBuildOptions,
) -> Result<(), ExitCode> {
    if !is_awfagent_path(&options.path) {
        eprintln!(
            "error: {} is not an .awfagent source file",
            options.path.display()
        );
        return Err(ExitCode::from(2));
    }
    if !is_awfb_path(&options.output) {
        eprintln!(
            "error: {} is not an .awfb bundle output path",
            options.output.display()
        );
        return Err(ExitCode::from(2));
    }
    let report =
        agent_script_build_report(options).unwrap_or_else(|error| AgentScriptBuildReport {
            path: options.path.display().to_string(),
            output: options.output.display().to_string(),
            ok: false,
            agent_entries: 0,
            entry_id: None,
            controller_id: None,
            bundle_kind: None,
            bytecode_instructions: 0,
            bytes: 0,
            error: Some(error),
        });
    if options.json {
        print_json(&report)?;
    } else if report.ok {
        println!(
            "{}: wrote {} ({} bytecode instruction(s))",
            report.path, report.output, report.bytecode_instructions
        );
    } else if let Some(error) = &report.error {
        eprintln!("{}: {error}", report.path);
    }
    if report.ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

pub(super) fn agent_script_build_report(
    options: &AgentScriptBuildOptions,
) -> Result<AgentScriptBuildReport, String> {
    let source = fs::read_to_string(&options.path)
        .map_err(|error| format!("failed to read {}: {error}", options.path.display()))?;
    let target = agent_script_standalone_compile_target(&options.signals)?;
    let compiled =
        compile_agent_script_source(&options.path, source, &options.controller_entry, &target)?;
    let bytes = compiled
        .artifact
        .bundle
        .to_json_bytes()
        .map_err(|error| error.to_string())?;
    write_agent_bundle(&options.output, &bytes)?;
    Ok(AgentScriptBuildReport {
        path: options.path.display().to_string(),
        output: options.output.display().to_string(),
        ok: true,
        agent_entries: 1,
        entry_id: Some(compiled.artifact.manifest.entry_id.as_str().to_owned()),
        controller_id: Some(compiled.artifact.manifest.controller_id.as_str().to_owned()),
        bundle_kind: Some(compiled.artifact.bundle.bundle_kind.to_string()),
        bytecode_instructions: compiled
            .artifact
            .bundle
            .manifest
            .runtime
            .bytecode_instructions,
        bytes: bytes.len(),
        error: None,
    })
}

fn agent_script_compile_target(
    options: &AgentScriptRunOptions,
) -> Result<AgentScriptCompileTarget, String> {
    #[cfg(feature = "native-capture")]
    if agent_script_run_uses_native_session(options) {
        let selection =
            resolve_source_selection(options.native_source.as_ref(), &options.native_profile)
                .map_err(|code| {
                    format!("failed to resolve native source for Agent Script: {code:?}")
                })?;
        let checked = load_and_check_selection(&selection, None)
            .map_err(|code| format!("failed to check native source for Agent Script: {code:?}"))?;
        let typecheck_environment = Arc::new(
            checked
                .compiled
                .registered_environment()
                .typecheck_env()
                .clone(),
        );
        let mut project = checked.compiled.semantic_index().as_ref().clone();
        for signal in &options.signals {
            let id = SemaPublicId::try_new(signal.id.clone()).map_err(|error| error.to_string())?;
            let identity = ProjectEntityId::public(id.clone());
            if project.entity(&identity).is_none() {
                project = project.with_entity(agent_script_signal_symbol(signal, id));
            }
        }
        let target_entities = project.entities().values().cloned().collect();
        return Ok(AgentScriptCompileTarget {
            typecheck_environment,
            target_entities,
            accepted_index: Some(Arc::new(project)),
        });
    }
    agent_script_standalone_compile_target(&options.signals)
}

pub(super) fn agent_script_standalone_compile_target(
    signals: &[AgentScriptSignalArg],
) -> Result<AgentScriptCompileTarget, String> {
    Ok(AgentScriptCompileTarget {
        typecheck_environment: Arc::new(TypeCheckEnv::standard()),
        target_entities: agent_script_signal_symbols(signals)?,
        accepted_index: None,
    })
}

pub(super) fn write_agent_bundle(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(super) fn agent_script_check_command(
    options: &AgentScriptCheckOptions,
) -> Result<(), ExitCode> {
    if !is_awfagent_path(&options.path) {
        eprintln!(
            "error: {} is not an .awfagent source file",
            options.path.display()
        );
        return Err(ExitCode::from(2));
    }
    let source = fs::read_to_string(&options.path).map_err(|error| {
        eprintln!("error: failed to read {}: {error}", options.path.display());
        ExitCode::FAILURE
    })?;
    let target = agent_script_standalone_compile_target(&[]).map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })?;
    let report = match compile_agent_script_source(
        &options.path,
        source,
        &options.controller_entry,
        &target,
    ) {
        Ok(_) => AgentScriptCheckReport {
            path: options.path.display().to_string(),
            ok: true,
            agent_entries: 1,
            error: None,
        },
        Err(error) => AgentScriptCheckReport {
            path: options.path.display().to_string(),
            ok: false,
            agent_entries: 0,
            error: Some(error.clone()),
        },
    };
    if options.json {
        print_json(&report)?;
    } else if report.ok {
        println!(
            "{}: ok ({} selected Agent entry)",
            report.path, report.agent_entries
        );
    } else if let Some(error) = &report.error {
        eprintln!("{}: {error}", report.path);
    }
    if report.ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

#[derive(serde::Serialize)]
pub(super) struct AgentScriptRunReport {
    pub(super) path: String,
    pub(super) ok: bool,
    pub(super) agent_entries: usize,
    pub(super) steps: usize,
    pub(super) host_calls: usize,
    pub(super) events_emitted: u64,
    pub(super) final_status: Option<String>,
    pub(super) trace_path: Option<String>,
    pub(super) trace_records: usize,
    pub(super) blob_dir: Option<String>,
    pub(super) debug_db: Option<String>,
    pub(super) blobs_written: usize,
    pub(super) blob_bytes: u64,
    pub(super) responses: Vec<AgentHostResponse>,
    pub(super) assertion_diagnostics: Vec<AgentScriptRuntimeDiagnostic>,
    pub(super) error: Option<String>,
}

#[derive(serde::Serialize)]
pub(super) struct AgentScriptRuntimeDiagnostic {
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) identity: &'static str,
}

pub(in crate::app) struct AgentScriptRunInput {
    pub(super) path: String,
    pub(super) agent_entries: usize,
    pub(super) program_hash: String,
    pub(super) project_entities: Vec<RequiredEntity>,
    pub(super) project_graph: AgentProjectGraph,
    pub(super) bundle: ArcweftBundle,
    /// Present only when this process compiled the exact bundle generation.
    /// Decoded bundles intentionally retain persisted-only diagnostics.
    pub(super) execution_diagnostics:
        Option<Arc<arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext>>,
}

pub(super) struct AgentScriptDebugRunFinishContext<'a> {
    pub(super) session_id: &'a SessionId,
    pub(super) options: &'a AgentScriptRunOptions,
    pub(super) input: &'a AgentScriptRunInput,
    pub(super) run_id: &'a AgentRunId,
    pub(super) base_sequence: u64,
}

#[derive(serde::Serialize)]
pub(super) struct AgentScriptTraceReport {
    pub(super) path: String,
    pub(super) ok: bool,
    pub(super) records: usize,
    pub(super) run_id: Option<String>,
    pub(super) sessions: Vec<String>,
    pub(super) first_sequence: Option<u64>,
    pub(super) last_sequence: Option<u64>,
    pub(super) started: bool,
    pub(super) finished: bool,
    pub(super) blob_refs: usize,
    pub(super) blobs_validated: usize,
    pub(super) blob_bytes: u64,
    pub(super) kinds: BTreeMap<String, usize>,
    pub(super) error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct AgentBlobWriteReport {
    pub(super) dir: Option<String>,
    pub(super) count: usize,
    pub(super) bytes: u64,
}

#[derive(Clone, Debug)]
pub(super) struct AgentCaptureBlob {
    pub(super) content_hash: String,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AgentTraceBlobValidation {
    pub(super) count: usize,
    pub(super) bytes: u64,
}

#[derive(serde::Serialize)]
pub(super) struct AgentScriptReplayReport {
    pub(super) path: String,
    pub(super) ok: bool,
    pub(super) records: usize,
    pub(super) events: usize,
    pub(super) expected_path: Option<String>,
    pub(super) matched_expected: Option<bool>,
    pub(super) first_mismatch: Option<AgentScriptReplayMismatch>,
    pub(super) logical_sequence: Vec<AgentScriptReplayEvent>,
    pub(super) error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(super) struct AgentScriptReplayEvent {
    pub(super) sequence: u64,
    pub(super) kind: String,
    pub(super) tick: Option<u64>,
    pub(super) payload_hash: String,
    pub(super) blob_refs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(super) struct AgentScriptReplayMismatch {
    pub(super) index: usize,
    pub(super) actual: Option<AgentScriptReplayEvent>,
    pub(super) expected: Option<AgentScriptReplayEvent>,
}

pub(super) fn agent_script_run_command(
    options: &AgentScriptRunOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let report = match agent_script_run_input(options) {
        Ok(input) => agent_script_run_bundle(options, &input, adapter_registrars)?,
        Err(error) => AgentScriptRunReport {
            path: options.path.display().to_string(),
            ok: false,
            agent_entries: 0,
            steps: 0,
            host_calls: 0,
            events_emitted: 0,
            final_status: None,
            trace_path: None,
            trace_records: 0,
            blob_dir: options
                .blob_dir
                .as_ref()
                .map(|path| path.display().to_string()),
            debug_db: options
                .debug_db
                .as_ref()
                .map(|path| path.display().to_string()),
            blobs_written: 0,
            blob_bytes: 0,
            responses: Vec::new(),
            assertion_diagnostics: Vec::new(),
            error: Some(error),
        },
    };
    if options.json {
        print_json(&report)?;
    } else if report.ok {
        println!(
            "{}: ok ({} step(s), {} host call(s))",
            report.path, report.steps, report.host_calls
        );
        for diagnostic in &report.assertion_diagnostics {
            eprintln!("error[{}]: {}", diagnostic.code, diagnostic.message);
        }
    } else if let Some(error) = &report.error {
        eprintln!("{}: {error}", report.path);
    }
    if report.ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

pub(super) fn agent_script_run_input(
    options: &AgentScriptRunOptions,
) -> Result<AgentScriptRunInput, String> {
    if is_awfagent_path(&options.path) {
        return agent_script_run_source_input(options);
    }
    if is_awfb_path(&options.path) {
        return agent_script_run_bundle_input(options);
    }
    Err(format!(
        "{} is not an .awfagent source file or .awfb Agent bundle",
        options.path.display()
    ))
}

pub(super) fn agent_script_run_source_input(
    options: &AgentScriptRunOptions,
) -> Result<AgentScriptRunInput, String> {
    let source = fs::read_to_string(&options.path)
        .map_err(|error| format!("failed to read {}: {error}", options.path.display()))?;
    let target = agent_script_compile_target(options)?;
    let compiled =
        compile_agent_script_source(&options.path, source, &options.controller_entry, &target)?;
    let program_hash = compiled.semantic_index.program_hash().as_str().to_owned();
    let project_entities = agent_project_entities(&compiled.semantic_index)?;
    let project_graph = agent_project_graph(&compiled.semantic_index)?;
    Ok(AgentScriptRunInput {
        path: options.path.display().to_string(),
        agent_entries: 1,
        program_hash,
        project_entities,
        project_graph,
        execution_diagnostics: Some(Arc::clone(&compiled.artifact.execution_diagnostics)),
        bundle: compiled.artifact.bundle,
    })
}

pub(super) fn agent_script_run_bundle_input(
    options: &AgentScriptRunOptions,
) -> Result<AgentScriptRunInput, String> {
    let path = &options.path;
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let engine_resource_types = arcweft_resource_model::registry::ResourceTypeRegistry::empty();
    let bundle = ArcweftBundle::from_product_path_slice_with_resource_types(
        path,
        &bytes,
        &engine_resource_types,
    )
    .map_err(|error| format!("failed to decode {}: {error}", path.display()))?;
    if bundle.bundle_kind != BundleKind::AgentController {
        return Err(format!(
            "{} is a {} bundle, not an agent_controller bundle",
            path.display(),
            bundle.bundle_kind
        ));
    }
    let target = agent_script_compile_target(options)?;
    let manifest = bundle
        .agent
        .as_ref()
        .expect("agent_controller bundles retain an Agent artifact manifest");
    let (program_hash, project_entities, project_graph) =
        if let Some(project) = target.accepted_index.as_deref() {
            (
                project.program_hash().as_str().to_owned(),
                agent_project_entities(project)?,
                agent_project_graph(project)?,
            )
        } else {
            let entities =
                agent_project::agent_required_entities_from_symbols(target.target_entities.iter())
                    .map_err(|error| error.to_string())?;
            (
                manifest.project_binding.program_hash.as_str().to_owned(),
                if entities.is_empty() {
                    manifest.project_binding.required_entities.clone()
                } else {
                    entities
                },
                AgentProjectGraph::default(),
            )
        };
    let agent_entries = usize::from(bundle.agent.is_some());
    Ok(AgentScriptRunInput {
        path: path.display().to_string(),
        agent_entries,
        program_hash,
        project_entities,
        project_graph,
        execution_diagnostics: None,
        bundle,
    })
}

pub(super) fn agent_script_run_bundle(
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentScriptRunReport, ExitCode> {
    #[cfg(not(feature = "native-capture"))]
    let _ = adapter_registrars;
    #[cfg(feature = "native-capture")]
    if agent_script_run_uses_native_session(options) {
        return native::observe::agent_script_run_native_bundle(options, input, adapter_registrars);
    }
    let session = CliAgentSession::new(
        options.signals.clone(),
        options.states.clone(),
        input.program_hash.clone(),
        input.project_entities.clone(),
        input.project_graph.clone(),
    );
    let mut runner = AgentRunner::new(
        session,
        CollectingDebugSink::default(),
        NoopRagService,
        agent_script_runtime_policy(input),
        AgentRunnerConfig::new(agent_cli_session_id()),
    );
    let run_result = runner.run_controller_bundle(
        &input.bundle,
        AgentControllerRunConfig {
            max_steps: options.max_steps,
            max_ops_per_step: options.max_ops,
        },
    );
    let blob_result = write_agent_capture_blobs(
        options.blob_dir.as_deref(),
        runner.session_mut().capture_blobs(),
    );
    let debug_events = runner.debug_mut().events.clone();
    let run_id = AgentRunId::new(options.run_id.clone()).map_err(|error| {
        eprintln!("error: invalid run id: {error}");
        ExitCode::from(2)
    })?;
    agent_script_run_report_from_result(
        options,
        input,
        run_result,
        &run_id,
        &debug_events,
        blob_result,
    )
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })
}

pub(in crate::app) fn agent_script_runtime_policy(
    input: &AgentScriptRunInput,
) -> RuntimeAgentPolicy {
    agent_script_runtime_policy_for_bundle(&input.bundle)
}

pub(in crate::app) fn agent_script_runtime_policy_for_bundle(
    bundle: &ArcweftBundle,
) -> RuntimeAgentPolicy {
    let mut capabilities = vec![
        RuntimeAgentCapability::Observe,
        RuntimeAgentCapability::Act,
        RuntimeAgentCapability::Capture,
        RuntimeAgentCapability::ResourceRead,
        RuntimeAgentCapability::DebugRead,
        RuntimeAgentCapability::DebugRecord,
        RuntimeAgentCapability::Rag,
    ];
    if agent_bundle_verifies_effect(bundle, "agent.act.physical") {
        capabilities.push(RuntimeAgentCapability::ActPhysical);
    }
    RuntimeAgentPolicy::new(capabilities)
}

pub(super) fn agent_bundle_verifies_effect(bundle: &ArcweftBundle, effect: &str) -> bool {
    bundle.agent.as_ref().is_some_and(|manifest| {
        manifest
            .verified_effects
            .inferred
            .iter()
            .any(|verified| verified.as_str() == effect)
    })
}

#[cfg(feature = "native-capture")]
pub(super) fn agent_script_run_uses_native_session(options: &AgentScriptRunOptions) -> bool {
    options.native_source.is_some() || options.native_profile.profile.is_some()
}

pub(super) fn agent_script_run_report_from_result(
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
    run_result: Result<AgentControllerRunReport, impl std::fmt::Display>,
    run_id: &AgentRunId,
    debug_events: &[DebugEvent],
    blob_result: Result<AgentBlobWriteReport, String>,
) -> Result<AgentScriptRunReport, String> {
    let trace_records = agent_trace_records(run_id, &agent_cli_session_id(), debug_events);
    let trace_result = options
        .trace_out
        .as_ref()
        .map(|path| write_agent_trace(path, &trace_records).map(|()| path.display().to_string()))
        .transpose();
    let mut report = match (run_result, trace_result, blob_result) {
        (Ok(run), Ok(trace_path), Ok(blob_report)) => agent_script_run_success_report(
            &input.path,
            input.agent_entries,
            run,
            trace_path,
            trace_records.len(),
            blob_report,
            input.execution_diagnostics.as_deref(),
        )?,
        (Err(error), Ok(trace_path), Ok(blob_report)) => agent_script_run_error_report(
            &input.path,
            input.agent_entries,
            trace_path,
            trace_records.len(),
            blob_report,
            error.to_string(),
        ),
        (_, Err(error), blob_result) => agent_script_run_error_report(
            &input.path,
            input.agent_entries,
            options
                .trace_out
                .as_ref()
                .map(|path| path.display().to_string()),
            trace_records.len(),
            blob_result.unwrap_or_default(),
            error,
        ),
        (_, _, Err(error)) => agent_script_run_error_report(
            &input.path,
            input.agent_entries,
            options
                .trace_out
                .as_ref()
                .map(|path| path.display().to_string()),
            trace_records.len(),
            AgentBlobWriteReport::default(),
            error,
        ),
    };
    report.debug_db = options
        .debug_db
        .as_ref()
        .map(|path| path.display().to_string());
    agent_script_persist_debug_run(options, input, run_id, debug_events, &report)?;
    Ok(report)
}

pub(super) fn agent_script_persist_debug_run(
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
    run_id: &AgentRunId,
    debug_events: &[DebugEvent],
    report: &AgentScriptRunReport,
) -> Result<(), String> {
    let Some(mut store) = agent_script_debug_store(options)? else {
        return Ok(());
    };
    let (session_id, base_sequence) = agent_script_start_debug_run(&store, options, input, run_id)?;
    agent_script_append_debug_events(&mut store, run_id, base_sequence, debug_events)?;
    agent_script_finish_debug_run(
        &store,
        &AgentScriptDebugRunFinishContext {
            session_id: &session_id,
            options,
            input,
            run_id,
            base_sequence,
        },
        debug_events,
        report,
    )?;
    store
        .flush()
        .map_err(|error| format!("failed to flush Agent Script debug database: {error}"))
}

pub(super) fn agent_script_debug_store(
    options: &AgentScriptRunOptions,
) -> Result<Option<DebugStore>, String> {
    let Some(path) = &options.debug_db else {
        return Ok(None);
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create debug database directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let store = DebugStore::open(path).map_err(|error| {
        format!(
            "failed to open Agent Script debug database {}: {error}",
            path.display()
        )
    })?;
    Ok(Some(store))
}

pub(super) fn agent_script_start_debug_run(
    store: &DebugStore,
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
    run_id: &AgentRunId,
) -> Result<(SessionId, u64), String> {
    let program_hash = StableHash::new(input.program_hash.clone())
        .map_err(|error| format!("invalid Agent Script program hash: {error}"))?;
    store
        .upsert_program(&program_hash, None, None, 0)
        .map_err(|error| format!("failed to persist Agent Script debug program: {error}"))?;
    let session_id = agent_cli_session_id();
    let session_metadata = agent_script_debug_session_metadata(options, input);
    let transport = if agent_script_run_uses_native_session_for_metadata(options) {
        "native"
    } else {
        "cli"
    };
    agent_debug_start_runtime_session(
        store,
        session_id.clone(),
        Some(program_hash),
        "script",
        transport,
        session_metadata,
        "Agent Script debug session",
    )?;
    let base_sequence = store
        .next_session_sequence(&session_id)
        .map_err(|error| format!("failed to allocate Agent Script debug sequence: {error}"))?;
    let manifest = input.bundle.agent.as_ref();
    let project_binding_mode = manifest
        .map_or("unknown", |manifest| match manifest.project_binding.mode {
            arcweft_agent_protocol::artifact::ProjectBindingMode::Strict => "strict",
            arcweft_agent_protocol::artifact::ProjectBindingMode::Compatible => "compatible",
        })
        .to_owned();
    store
        .upsert_script_run(&DebugScriptRun {
            run_id: run_id.clone(),
            session_id: session_id.clone(),
            agent_id: manifest.map(|manifest| manifest.entry_id.clone()),
            artifact_hash: None,
            source_hash: manifest.map(|manifest| manifest.source_hash.clone()),
            project_binding_mode,
            started_sequence: base_sequence,
            finished_sequence: None,
            outcome: DebugScriptRunOutcome::Running,
            partially_effectful: false,
            trace_uri: None,
            error: None,
            metadata: agent_script_debug_run_metadata(input),
        })
        .map_err(|error| format!("failed to persist Agent Script run start: {error}"))?;
    Ok((session_id, base_sequence))
}

pub(super) fn agent_script_debug_session_metadata(
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
) -> BTreeMap<String, serde_json::Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("path".to_owned(), serde_json::json!(input.path));
    metadata.insert(
        "native".to_owned(),
        serde_json::json!(agent_script_run_uses_native_session_for_metadata(options)),
    );
    metadata.insert(
        "project_entities".to_owned(),
        agent_script_project_entities_metadata(&input.project_entities),
    );
    metadata.insert(
        "project_graph".to_owned(),
        agent_script_project_graph_metadata(&input.project_graph),
    );
    metadata
}

pub(super) fn agent_script_debug_run_metadata(
    input: &AgentScriptRunInput,
) -> BTreeMap<String, serde_json::Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "project_entities".to_owned(),
        agent_script_project_entities_metadata(&input.project_entities),
    );
    metadata.insert(
        "project_graph".to_owned(),
        agent_script_project_graph_metadata(&input.project_graph),
    );
    metadata
}

pub(in crate::app::agent) fn agent_script_project_entities_metadata(
    entities: &[RequiredEntity],
) -> serde_json::Value {
    let mut kind_counts = BTreeMap::<String, usize>::new();
    for entity in entities {
        *kind_counts.entry(entity.kind.clone()).or_insert(0) += 1;
    }
    serde_json::json!({
        "count": entities.len(),
        "kind_counts": kind_counts,
    })
}

pub(in crate::app::agent) fn agent_script_project_graph_metadata(
    graph: &AgentProjectGraph,
) -> serde_json::Value {
    let mut symbol_kind_counts = BTreeMap::<String, usize>::new();
    for symbol in &graph.symbols {
        *symbol_kind_counts.entry(symbol.kind.clone()).or_insert(0) += 1;
    }
    let mut edge_kind_counts = BTreeMap::<String, usize>::new();
    for edge in &graph.edges {
        *edge_kind_counts.entry(edge.edge_kind.clone()).or_insert(0) += 1;
    }
    let summary_symbol = graph
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "project_summary");
    let project_summary = summary_symbol.and_then(|symbol| symbol.project_summary);
    serde_json::json!({
        "symbol_count": graph.symbols.len(),
        "edge_count": graph.edges.len(),
        "summary_symbol_id": summary_symbol.map(|symbol| symbol.symbol_id.as_str()),
        "has_project_summary": project_summary.is_some(),
        "project_summary": project_summary,
        "symbol_kind_counts": symbol_kind_counts,
        "edge_kind_counts": edge_kind_counts,
    })
}

pub(super) fn agent_script_append_debug_events(
    store: &mut DebugStore,
    run_id: &AgentRunId,
    base_sequence: u64,
    debug_events: &[DebugEvent],
) -> Result<(), String> {
    for event in debug_events {
        let mut event = event.clone();
        event.run_id = Some(run_id.clone());
        event.sequence = event.sequence.saturating_add(base_sequence);
        store
            .append(&event)
            .map_err(|error| format!("failed to persist Agent Script debug event: {error}"))?;
    }
    Ok(())
}

pub(super) fn agent_script_finish_debug_run(
    store: &DebugStore,
    context: &AgentScriptDebugRunFinishContext<'_>,
    debug_events: &[DebugEvent],
    report: &AgentScriptRunReport,
) -> Result<(), String> {
    let finished_sequence =
        debug_events
            .last()
            .map_or(context.base_sequence.saturating_add(1), |event| {
                context
                    .base_sequence
                    .saturating_add(event.sequence)
                    .saturating_add(1)
            });
    let outcome = if report.ok {
        DebugScriptRunOutcome::Done
    } else {
        DebugScriptRunOutcome::Failed
    };
    let mut run_metadata = agent_script_debug_run_metadata(context.input);
    run_metadata.insert("steps".to_owned(), serde_json::json!(report.steps));
    run_metadata.insert(
        "host_calls".to_owned(),
        serde_json::json!(report.host_calls),
    );
    run_metadata.insert(
        "events_emitted".to_owned(),
        serde_json::json!(report.events_emitted),
    );
    run_metadata.insert(
        "assertion_diagnostics".to_owned(),
        serde_json::json!(&report.assertion_diagnostics),
    );
    run_metadata.insert(
        "trace_records".to_owned(),
        serde_json::json!(report.trace_records),
    );
    let error = report
        .error
        .as_ref()
        .map(|message| serde_json::json!({ "message": message }));
    store
        .finish_script_run(
            context.run_id,
            &DebugScriptRunFinish {
                outcome,
                finished_sequence,
                partially_effectful: report.host_calls > 0,
                trace_uri: report.trace_path.clone(),
                error,
                metadata: run_metadata,
            },
        )
        .map_err(|error| format!("failed to persist Agent Script run finish: {error}"))?;
    let mut session_finish_metadata =
        agent_script_debug_session_metadata(context.options, context.input);
    session_finish_metadata.insert("runs".to_owned(), serde_json::json!(1));
    session_finish_metadata.insert(
        "last_run_id".to_owned(),
        serde_json::json!(context.run_id.as_str()),
    );
    session_finish_metadata.insert("ok".to_owned(), serde_json::json!(report.ok));
    agent_debug_finish_runtime_session(
        store,
        context.session_id,
        if report.ok {
            DebugSessionStatus::Finished
        } else {
            DebugSessionStatus::Failed
        },
        &session_finish_metadata,
        "Agent Script debug session",
    )
}

pub(super) fn agent_debug_start_runtime_session(
    store: &DebugStore,
    session_id: SessionId,
    program_hash: Option<StableHash>,
    profile: &str,
    transport: &str,
    metadata: BTreeMap<String, serde_json::Value>,
    context: &str,
) -> Result<i64, String> {
    let started_unix_ms = agent_debug_current_unix_millis()
        .map_err(|error| format!("failed to read current time for {context}: {error}"))?;
    let mut session = DebugSession {
        session_id,
        program_hash,
        profile: profile.to_owned(),
        transport: transport.to_owned(),
        started_unix_ms,
        ended_unix_ms: None,
        status: DebugSessionStatus::Running,
        metadata,
    };
    store
        .upsert_session(&session)
        .map_err(|error| format!("failed to persist {context}: {error}"))?;
    let closed_sessions =
        agent_debug_close_stale_running_sessions_for_runtime(store, started_unix_ms, context)?;
    session.metadata.insert(
        "lifecycle_policy".to_owned(),
        agent_debug_runtime_lifecycle_policy_metadata(started_unix_ms, &closed_sessions),
    );
    store
        .upsert_session(&session)
        .map_err(|error| format!("failed to persist {context} lifecycle metadata: {error}"))?;
    Ok(started_unix_ms)
}

pub(super) fn agent_debug_finish_runtime_session(
    store: &DebugStore,
    session_id: &SessionId,
    status: DebugSessionStatus,
    metadata: &BTreeMap<String, serde_json::Value>,
    context: &str,
) -> Result<(), String> {
    let ended_unix_ms = agent_debug_current_unix_millis()
        .map_err(|error| format!("failed to read current time for {context}: {error}"))?;
    store
        .finish_session(session_id, status, ended_unix_ms, metadata)
        .map_err(|error| format!("failed to finish {context}: {error}"))
}

pub(super) fn agent_debug_close_stale_running_sessions_for_runtime(
    store: &DebugStore,
    now_unix_ms: i64,
    context: &str,
) -> Result<Vec<DebugSession>, String> {
    let cutoff_unix_ms = now_unix_ms.saturating_sub(AGENT_DEBUG_RUNTIME_STALE_AFTER_MILLIS);
    store
        .abandon_stale_running_sessions(
            cutoff_unix_ms,
            now_unix_ms,
            AGENT_DEBUG_RUNTIME_STALE_REASON,
        )
        .map_err(|error| format!("failed to apply {context} lifecycle policy: {error}"))
}

pub(super) fn agent_debug_runtime_lifecycle_policy_metadata(
    now_unix_ms: i64,
    closed_sessions: &[DebugSession],
) -> serde_json::Value {
    let cutoff_unix_ms = now_unix_ms.saturating_sub(AGENT_DEBUG_RUNTIME_STALE_AFTER_MILLIS);
    let closed_session_ids = closed_sessions
        .iter()
        .map(|session| session.session_id.as_str())
        .collect::<Vec<_>>();
    serde_json::json!({
        "operation": "runtime_session_start",
        "stale_after_millis": AGENT_DEBUG_RUNTIME_STALE_AFTER_MILLIS,
        "cutoff_unix_ms": cutoff_unix_ms,
        "closed_unix_ms": now_unix_ms,
        "reason": AGENT_DEBUG_RUNTIME_STALE_REASON,
        "closed_count": closed_session_ids.len(),
        "closed_sessions": closed_session_ids,
    })
}

pub(super) fn agent_debug_current_unix_millis() -> Result<i64, std::time::SystemTimeError> {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    Ok(i64::try_from(millis).unwrap_or(i64::MAX))
}

pub(super) fn agent_script_run_uses_native_session_for_metadata(
    options: &AgentScriptRunOptions,
) -> bool {
    #[cfg(feature = "native-capture")]
    {
        agent_script_run_uses_native_session(options)
    }
    #[cfg(not(feature = "native-capture"))]
    {
        let _ = options;
        false
    }
}

pub(super) fn agent_script_run_success_report(
    path: &str,
    agent_entries: usize,
    run: AgentControllerRunReport,
    trace_path: Option<String>,
    trace_records: usize,
    blob_report: AgentBlobWriteReport,
    execution_diagnostics: Option<
        &arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext,
    >,
) -> Result<AgentScriptRunReport, String> {
    let assertion_diagnostics = run
        .assertion_failures
        .iter()
        .map(|failure| {
            let diagnostic = if let Some(context) = execution_diagnostics {
                let fault =
                    context
                        .project_assertion_failure(failure.clone())
                        .map_err(|error| {
                            format!("fresh Agent assertion identity projection failed: {error}")
                        })?;
                project_runtime_assertion_fault(&fault)
            } else {
                project_persisted_assertion_failure(failure, None)
            };
            Ok(AgentScriptRuntimeDiagnostic {
                code: diagnostic.code(),
                message: diagnostic.message().to_owned(),
                identity: if execution_diagnostics.is_some() {
                    "session"
                } else {
                    "persisted_only"
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(AgentScriptRunReport {
        path: path.to_owned(),
        ok: true,
        agent_entries,
        steps: run.steps,
        host_calls: run.host_calls,
        events_emitted: run.events_emitted,
        final_status: run.final_status.map(|status| format!("{status:?}")),
        trace_path,
        trace_records,
        blob_dir: blob_report.dir,
        debug_db: None,
        blobs_written: blob_report.count,
        blob_bytes: blob_report.bytes,
        responses: run.responses,
        assertion_diagnostics,
        error: None,
    })
}

pub(super) fn agent_script_run_error_report(
    path: &str,
    agent_entries: usize,
    trace_path: Option<String>,
    trace_records: usize,
    blob_report: AgentBlobWriteReport,
    error: String,
) -> AgentScriptRunReport {
    AgentScriptRunReport {
        path: path.to_owned(),
        ok: false,
        agent_entries,
        steps: 0,
        host_calls: 0,
        events_emitted: 0,
        final_status: None,
        trace_path,
        trace_records,
        blob_dir: blob_report.dir,
        debug_db: None,
        blobs_written: blob_report.count,
        blob_bytes: blob_report.bytes,
        responses: Vec::new(),
        assertion_diagnostics: Vec::new(),
        error: Some(error),
    }
}

pub(super) fn agent_cli_session_id() -> SessionId {
    SessionId::new("session.cli").expect("static session id")
}

pub(super) fn write_agent_capture_blobs(
    blob_dir: Option<&Path>,
    blobs: &[AgentCaptureBlob],
) -> Result<AgentBlobWriteReport, String> {
    let Some(blob_dir) = blob_dir else {
        return Ok(AgentBlobWriteReport::default());
    };
    let mut report = AgentBlobWriteReport {
        dir: Some(blob_dir.display().to_string()),
        count: 0,
        bytes: 0,
    };
    for blob in blobs {
        let path = agent_blob_path(blob_dir, &blob.content_hash)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        fs::write(&path, &blob.bytes)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        report.count += 1;
        report.bytes = report
            .bytes
            .checked_add(u64::try_from(blob.bytes.len()).map_err(|_| {
                format!(
                    "capture blob {} is too large to count as u64 bytes",
                    blob.content_hash
                )
            })?)
            .ok_or_else(|| "capture blob byte count overflowed u64".to_owned())?;
    }
    Ok(report)
}

pub(super) fn agent_blob_path(root: &Path, content_hash: &str) -> Result<PathBuf, String> {
    let Some(hex) = content_hash.strip_prefix("blake3:") else {
        return Err(format!(
            "capture blob hash `{content_hash}` is not a blake3 content hash"
        ));
    };
    if hex.is_empty()
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!(
            "capture blob hash `{content_hash}` has an invalid blake3 digest"
        ));
    }
    Ok(root.join("blake3").join(hex))
}

pub(super) fn agent_script_replay_command(
    options: &AgentScriptReplayOptions,
) -> Result<(), ExitCode> {
    let report =
        agent_script_replay_report(options).unwrap_or_else(|error| AgentScriptReplayReport {
            path: options.path.display().to_string(),
            ok: false,
            records: 0,
            events: 0,
            expected_path: options
                .expect
                .as_ref()
                .map(|path| path.display().to_string()),
            matched_expected: None,
            first_mismatch: None,
            logical_sequence: Vec::new(),
            error: Some(error),
        });
    if options.json {
        print_json(&report)?;
    } else if report.ok {
        if let Some(expected) = &report.expected_path {
            println!(
                "{}: replay ok ({} event(s), matched {})",
                report.path, report.events, expected
            );
        } else {
            println!("{}: replay ok ({} event(s))", report.path, report.events);
        }
    } else if let Some(error) = &report.error {
        eprintln!("{}: {error}", report.path);
    }
    if report.ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

pub(super) fn agent_script_replay_report(
    options: &AgentScriptReplayOptions,
) -> Result<AgentScriptReplayReport, String> {
    let records = read_and_validate_agent_trace_records(&options.path)?;
    let sequence = agent_trace_replay_sequence(&records);
    let expected = options
        .expect
        .as_ref()
        .map(|path| read_and_validate_agent_trace_records(path).map(|records| (path, records)))
        .transpose()?;
    let comparison = expected.as_ref().map(|(_, records)| {
        compare_agent_trace_replay(&sequence, &agent_trace_replay_sequence(records))
    });
    let matched_expected = comparison.as_ref().map(Option::is_none);
    let first_mismatch = comparison.flatten();
    Ok(AgentScriptReplayReport {
        path: options.path.display().to_string(),
        ok: first_mismatch.is_none(),
        records: records.len(),
        events: sequence.len(),
        expected_path: expected.map(|(path, _)| path.display().to_string()),
        matched_expected,
        first_mismatch: first_mismatch.clone(),
        logical_sequence: sequence,
        error: first_mismatch.map(|mismatch| {
            format!(
                "trace logical sequence diverged at replay event {}",
                mismatch.index
            )
        }),
    })
}

pub(super) fn read_and_validate_agent_trace_records(
    path: &Path,
) -> Result<Vec<AgentTraceRecord>, String> {
    let records = read_agent_trace_records(path)?;
    validate_agent_trace(path, &records, None)?;
    Ok(records)
}

pub(super) fn agent_trace_replay_sequence(
    records: &[AgentTraceRecord],
) -> Vec<AgentScriptReplayEvent> {
    records
        .iter()
        .map(|record| AgentScriptReplayEvent {
            sequence: record.sequence,
            kind: agent_trace_kind_name(record.kind).to_owned(),
            tick: record.tick,
            payload_hash: record.payload_hash.as_str().to_owned(),
            blob_refs: record
                .blob_refs
                .iter()
                .map(|hash| hash.as_str().to_owned())
                .collect(),
        })
        .collect()
}

pub(super) fn compare_agent_trace_replay(
    actual: &[AgentScriptReplayEvent],
    expected: &[AgentScriptReplayEvent],
) -> Option<AgentScriptReplayMismatch> {
    actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| !agent_trace_replay_events_match(actual, expected))
        .or_else(|| (actual.len() != expected.len()).then_some(actual.len().min(expected.len())))
        .map(|index| AgentScriptReplayMismatch {
            index,
            actual: actual.get(index).cloned(),
            expected: expected.get(index).cloned(),
        })
}

pub(super) fn agent_trace_replay_events_match(
    actual: &AgentScriptReplayEvent,
    expected: &AgentScriptReplayEvent,
) -> bool {
    actual.kind == expected.kind
        && actual.tick == expected.tick
        && actual.payload_hash == expected.payload_hash
        && actual.blob_refs == expected.blob_refs
}

pub(super) fn agent_script_trace_command(
    options: &AgentScriptTraceOptions,
) -> Result<(), ExitCode> {
    let report = read_agent_trace_records(&options.path)
        .and_then(|records| {
            validate_agent_trace(&options.path, &records, options.blob_dir.as_deref())
        })
        .unwrap_or_else(|error| AgentScriptTraceReport {
            path: options.path.display().to_string(),
            ok: false,
            records: 0,
            run_id: None,
            sessions: Vec::new(),
            first_sequence: None,
            last_sequence: None,
            started: false,
            finished: false,
            blob_refs: 0,
            blobs_validated: 0,
            blob_bytes: 0,
            kinds: BTreeMap::new(),
            error: Some(error),
        });
    if options.json {
        print_json(&report)?;
    } else if report.ok {
        println!(
            "{}: ok ({} trace record(s), run {})",
            report.path,
            report.records,
            report.run_id.as_deref().unwrap_or("<unknown>")
        );
    } else if let Some(error) = &report.error {
        eprintln!("{}: {error}", report.path);
    }
    if report.ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

pub(super) fn read_agent_trace_records(path: &Path) -> Result<Vec<AgentTraceRecord>, String> {
    if !is_arcwx_path(path) {
        return Err(format!(
            "{} is not an .arcwx trace input path",
            path.display()
        ));
    }
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to decode {}: {error}", path.display()))
}

pub(super) fn validate_agent_trace(
    path: &Path,
    records: &[AgentTraceRecord],
    blob_dir: Option<&Path>,
) -> Result<AgentScriptTraceReport, String> {
    let run_id = records
        .first()
        .map(|record| record.run_id.clone())
        .ok_or_else(|| "trace must contain at least one record".to_owned())?;
    let first_sequence = records.first().map(|record| record.sequence);
    let last_sequence = records.last().map(|record| record.sequence);
    let blob_validation = validate_agent_trace_records(records, &run_id, blob_dir)?;
    Ok(AgentScriptTraceReport {
        path: path.display().to_string(),
        ok: true,
        records: records.len(),
        run_id: Some(run_id.as_str().to_owned()),
        sessions: agent_trace_sessions(records),
        first_sequence,
        last_sequence,
        started: records
            .first()
            .is_some_and(|record| record.kind == AgentTraceKind::RunStarted),
        finished: records
            .last()
            .is_some_and(|record| record.kind == AgentTraceKind::RunFinished),
        blob_refs: records.iter().map(|record| record.blob_refs.len()).sum(),
        blobs_validated: blob_validation.count,
        blob_bytes: blob_validation.bytes,
        kinds: agent_trace_kind_counts(records),
        error: None,
    })
}

pub(super) fn validate_agent_trace_records(
    records: &[AgentTraceRecord],
    run_id: &AgentRunId,
    blob_dir: Option<&Path>,
) -> Result<AgentTraceBlobValidation, String> {
    let first = records
        .first()
        .ok_or_else(|| "trace must contain at least one record".to_owned())?;
    if first.kind != AgentTraceKind::RunStarted {
        return Err("trace first record must be run_started".to_owned());
    }
    if !records
        .last()
        .is_some_and(|record| record.kind == AgentTraceKind::RunFinished)
    {
        return Err("trace last record must be run_finished".to_owned());
    }
    let mut previous = None;
    let mut blob_validation = AgentTraceBlobValidation::default();
    for record in records {
        if let Some(bytes) = validate_agent_trace_record(record, run_id, previous, blob_dir)? {
            blob_validation.count += 1;
            blob_validation.bytes = blob_validation
                .bytes
                .checked_add(bytes)
                .ok_or_else(|| "validated blob byte count overflowed u64".to_owned())?;
        }
        previous = Some(record.sequence);
    }
    Ok(blob_validation)
}

pub(super) fn validate_agent_trace_record(
    record: &AgentTraceRecord,
    run_id: &AgentRunId,
    previous_sequence: Option<u64>,
    blob_dir: Option<&Path>,
) -> Result<Option<u64>, String> {
    if record.schema_version != 1 {
        return Err(format!(
            "trace record {} has unsupported schema_version {}",
            record.sequence, record.schema_version
        ));
    }
    if &record.run_id != run_id {
        return Err(format!(
            "trace record {} changes run_id from {} to {}",
            record.sequence,
            run_id.as_str(),
            record.run_id.as_str()
        ));
    }
    if previous_sequence.is_some_and(|sequence| record.sequence <= sequence) {
        return Err(format!(
            "trace record sequence {} is not strictly increasing",
            record.sequence
        ));
    }
    let expected_hash = stable_payload_hash(&record.payload);
    if record.payload_hash != expected_hash {
        return Err(format!(
            "trace record {} payload_hash mismatch: expected {}, got {}",
            record.sequence,
            expected_hash.as_str(),
            record.payload_hash.as_str()
        ));
    }
    if record.kind == AgentTraceKind::CaptureStored {
        return validate_agent_trace_capture_blob_refs(record, blob_dir);
    }
    Ok(None)
}

pub(super) fn validate_agent_trace_capture_blob_refs(
    record: &AgentTraceRecord,
    blob_dir: Option<&Path>,
) -> Result<Option<u64>, String> {
    let content_hash = record
        .payload
        .get("content_hash")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "trace record {} capture payload is missing content_hash",
                record.sequence
            )
        })?;
    let content_hash = StableHash::new(content_hash.to_owned()).map_err(|error| {
        format!(
            "trace record {} capture content_hash is invalid: {error}",
            record.sequence
        )
    })?;
    if record.blob_refs.iter().any(|hash| hash == &content_hash) {
        return validate_agent_trace_capture_blob_bytes(record, &content_hash, blob_dir);
    }
    Err(format!(
        "trace record {} capture blob_refs does not include content_hash {}",
        record.sequence,
        content_hash.as_str()
    ))
}

pub(super) fn validate_agent_trace_capture_blob_bytes(
    record: &AgentTraceRecord,
    content_hash: &StableHash,
    blob_dir: Option<&Path>,
) -> Result<Option<u64>, String> {
    let Some(blob_dir) = blob_dir else {
        return Ok(None);
    };
    let expected_len = record
        .payload
        .get("byte_len")
        .and_then(serde_json::Value::as_u64);
    let path = agent_blob_path(blob_dir, content_hash.as_str())?;
    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let actual_len = u64::try_from(bytes.len()).map_err(|_| {
        format!(
            "trace record {} capture blob {} is too large to count as u64 bytes",
            record.sequence,
            path.display()
        )
    })?;
    if expected_len.is_some_and(|expected_len| expected_len != actual_len) {
        return Err(format!(
            "trace record {} capture blob byte_len mismatch for {}: expected {}, got {}",
            record.sequence,
            content_hash.as_str(),
            expected_len.unwrap_or_default(),
            actual_len
        ));
    }
    let actual_hash = StableHash::new(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
        .expect("generated blob hash is nonempty");
    if &actual_hash != content_hash {
        return Err(format!(
            "trace record {} capture blob hash mismatch for {}: got {}",
            record.sequence,
            content_hash.as_str(),
            actual_hash.as_str()
        ));
    }
    Ok(Some(actual_len))
}

pub(super) fn agent_trace_sessions(records: &[AgentTraceRecord]) -> Vec<String> {
    records
        .iter()
        .filter_map(|record| {
            record
                .session_id
                .as_ref()
                .map(|session_id| session_id.as_str().to_owned())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn agent_trace_kind_counts(records: &[AgentTraceRecord]) -> BTreeMap<String, usize> {
    records.iter().fold(BTreeMap::new(), |mut counts, record| {
        *counts
            .entry(agent_trace_kind_name(record.kind).to_owned())
            .or_default() += 1;
        counts
    })
}

pub(super) fn agent_trace_kind_name(kind: AgentTraceKind) -> &'static str {
    match kind {
        AgentTraceKind::RunStarted => "run_started",
        AgentTraceKind::VmStep => "vm_step",
        AgentTraceKind::HostCallRequested => "host_call_requested",
        AgentTraceKind::ObservationReceived => "observation_received",
        AgentTraceKind::ActionCompleted => "action_completed",
        AgentTraceKind::CaptureStored => "capture_stored",
        AgentTraceKind::ResourceReadCompleted => "resource_read_completed",
        AgentTraceKind::AssertionEvaluated => "assertion_evaluated",
        AgentTraceKind::RagQueryCompleted => "rag_query_completed",
        AgentTraceKind::DiagnosticEmitted => "diagnostic_emitted",
        AgentTraceKind::RunFinished => "run_finished",
    }
}

pub(super) fn is_awfagent_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "awfagent")
}

pub(super) fn is_awfb_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "awfb")
}

pub(super) fn is_arcwx_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "arcwx")
}

pub(in crate::app::agent) fn parse_agent_script_signal_arg(
    value: &str,
) -> Result<AgentScriptSignalArg, String> {
    let (id, raw_value) = value
        .split_once('=')
        .ok_or_else(|| "signal must be formatted as id=value".to_owned())?;
    let id = id.trim().trim_start_matches('@').to_owned();
    if id.is_empty() {
        return Err("signal id must not be empty".to_owned());
    }
    let (value, ty) = parse_agent_script_value(raw_value.trim());
    Ok(AgentScriptSignalArg { id, value, ty })
}

pub(in crate::app::agent) fn parse_agent_script_state_arg(
    value: &str,
) -> Result<AgentScriptStateArg, String> {
    let (path, raw_value) = value
        .split_once('=')
        .ok_or_else(|| "state must be formatted as path=value".to_owned())?;
    let path = path.trim().to_owned();
    if path.is_empty() {
        return Err("state path must not be empty".to_owned());
    }
    let (value, _) = parse_agent_script_value(raw_value.trim());
    Ok(AgentScriptStateArg { path, value })
}

pub(super) fn parse_agent_script_value(raw_value: &str) -> (AgentValue, TypeKind) {
    match raw_value {
        "true" => (AgentValue::Bool(true), TypeKind::Bool),
        "false" => (AgentValue::Bool(false), TypeKind::Bool),
        _ => raw_value.parse::<i64>().map_or_else(
            |_| {
                (
                    AgentValue::String(
                        raw_value
                            .strip_prefix('"')
                            .and_then(|value| value.strip_suffix('"'))
                            .unwrap_or(raw_value)
                            .to_owned(),
                    ),
                    TypeKind::String,
                )
            },
            |value| (AgentValue::I64(value), TypeKind::I64),
        ),
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct CollectingDebugSink {
    pub(super) events: Vec<DebugEvent>,
}

impl DebugEventSink for CollectingDebugSink {
    type Error = Infallible;

    fn append(&mut self, event: &DebugEvent) -> Result<(), Self::Error> {
        self.events.push(event.clone());
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(super) fn agent_trace_records(
    run_id: &AgentRunId,
    session_id: &SessionId,
    events: &[DebugEvent],
) -> Vec<AgentTraceRecord> {
    let mut records = Vec::with_capacity(events.len() + 2);
    records.push(agent_trace_record(
        run_id,
        Some(session_id),
        0,
        None,
        AgentTraceKind::RunStarted,
        serde_json::json!({ "source": "arcw agent script run" }),
    ));
    records.extend(events.iter().map(|event| {
        agent_trace_record(
            run_id,
            Some(&event.session_id),
            event.sequence,
            event.tick,
            agent_trace_kind(event.kind),
            event.payload.clone(),
        )
    }));
    records.push(agent_trace_record(
        run_id,
        Some(session_id),
        events
            .last()
            .map_or(1, |event| event.sequence.saturating_add(1)),
        None,
        AgentTraceKind::RunFinished,
        serde_json::json!({ "debug_events": events.len() }),
    ));
    records
}

pub(super) fn agent_trace_record(
    run_id: &AgentRunId,
    session_id: Option<&SessionId>,
    sequence: u64,
    tick: Option<u64>,
    kind: AgentTraceKind,
    payload: serde_json::Value,
) -> AgentTraceRecord {
    let blob_refs = agent_trace_blob_refs(kind, &payload);
    AgentTraceRecord {
        schema_version: 1,
        run_id: run_id.clone(),
        session_id: session_id.cloned(),
        sequence,
        tick,
        kind,
        payload_hash: stable_payload_hash(&payload),
        payload,
        blob_refs,
    }
}

pub(super) fn agent_trace_blob_refs(
    kind: AgentTraceKind,
    payload: &serde_json::Value,
) -> Vec<StableHash> {
    if kind != AgentTraceKind::CaptureStored {
        return Vec::new();
    }
    payload
        .get("content_hash")
        .and_then(serde_json::Value::as_str)
        .and_then(|hash| StableHash::new(hash.to_owned()).ok())
        .into_iter()
        .collect()
}

pub(super) fn agent_trace_kind(kind: DebugEventKind) -> AgentTraceKind {
    match kind {
        DebugEventKind::RunStarted | DebugEventKind::SessionStarted => AgentTraceKind::RunStarted,
        DebugEventKind::RunFinished | DebugEventKind::SessionFinished => {
            AgentTraceKind::RunFinished
        }
        DebugEventKind::StepStarted | DebugEventKind::StepFinished => AgentTraceKind::VmStep,
        DebugEventKind::Observation => AgentTraceKind::ObservationReceived,
        DebugEventKind::Action => AgentTraceKind::ActionCompleted,
        DebugEventKind::Capture => AgentTraceKind::CaptureStored,
        DebugEventKind::ResourceRead => AgentTraceKind::ResourceReadCompleted,
        DebugEventKind::Assertion => AgentTraceKind::AssertionEvaluated,
        DebugEventKind::Diagnostic | DebugEventKind::ReplCell => AgentTraceKind::DiagnosticEmitted,
        DebugEventKind::RagQuery => AgentTraceKind::RagQueryCompleted,
    }
}

pub(super) fn stable_payload_hash(payload: &serde_json::Value) -> StableHash {
    let bytes = serde_json::to_vec(payload).unwrap_or_default();
    StableHash::new(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
        .expect("generated trace payload hash is nonempty")
}

pub(super) fn write_agent_trace(path: &Path, records: &[AgentTraceRecord]) -> Result<(), String> {
    if !is_arcwx_path(path) {
        return Err(format!(
            "{} is not an .arcwx trace output path",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(records)
        .map_err(|error| format!("failed to encode trace: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(super) fn agent_project_entities(
    project: &ProjectSemanticIndex,
) -> Result<Vec<RequiredEntity>, String> {
    agent_project::agent_required_entities_from_project(project).map_err(|error| error.to_string())
}

pub(super) fn agent_project_graph(
    project: &ProjectSemanticIndex,
) -> Result<AgentProjectGraph, String> {
    agent_project::agent_project_graph_from_project(project).map_err(|error| error.to_string())
}

pub(super) fn agent_script_signal_symbols(
    signals: &[AgentScriptSignalArg],
) -> Result<Vec<EntitySymbol>, String> {
    signals
        .iter()
        .map(|signal| {
            let id = SemaPublicId::try_new(signal.id.clone()).map_err(|error| error.to_string())?;
            Ok(agent_script_signal_symbol(signal, id))
        })
        .collect()
}

pub(super) fn agent_script_signal_symbol(
    signal: &AgentScriptSignalArg,
    id: SemaPublicId,
) -> EntitySymbol {
    let document = arcweft_source::SourceDocument::try_new(
        arcweft_source::SourceDocumentId::try_new(format!(
            "arcweft-generated://cli-agent-signal/{}",
            signal.id
        ))
        .expect("validated signal ids form generated document ids"),
        arcweft_source::SourceName::Generated,
        "",
    )
    .expect("empty generated source fits a source document");
    let source = SourceAnchor::from_span(
        document
            .span(arcweft_source::SourceRange::new(0, 0))
            .expect("the empty range belongs to the generated document"),
    );
    EntitySymbol::new(
        ProjectEntityId::public(id),
        EntityType::new(EntityKind::Signal, Some(signal.ty.clone())),
        source,
        SemanticHash::new(format!("cli-signal:{}", signal.id)),
    )
}

#[derive(Debug)]
pub(super) struct CliAgentSession {
    pub(super) program_hash: String,
    pub(super) project_entities: Vec<RequiredEntity>,
    pub(super) project_graph: AgentProjectGraph,
    pub(super) tick: u64,
    pub(super) signals: BTreeMap<String, AgentValue>,
    pub(super) states: BTreeMap<String, AgentValue>,
    pub(super) captures: u64,
    pub(super) capture_blobs: Vec<AgentCaptureBlob>,
}

impl CliAgentSession {
    pub(super) fn new(
        signals: Vec<AgentScriptSignalArg>,
        states: Vec<AgentScriptStateArg>,
        program_hash: String,
        project_entities: Vec<RequiredEntity>,
        project_graph: AgentProjectGraph,
    ) -> Self {
        Self {
            program_hash,
            project_entities,
            project_graph,
            tick: 0,
            signals: signals
                .into_iter()
                .map(|signal| (signal.id, signal.value))
                .collect(),
            states: states
                .into_iter()
                .map(|state| (state.path, state.value))
                .collect(),
            captures: 0,
            capture_blobs: Vec::new(),
        }
    }

    pub(super) fn observation(&self) -> ObservationEnvelope {
        ObservationEnvelope {
            tick: self.tick,
            frame_id: format!("cli.frame.{}", self.tick),
            state_hash: format!("cli.state.{}", self.tick),
            render_hash: format!("cli.render.{}", self.tick),
            actions: Vec::new(),
            signals: self.signals.clone(),
            payload: serde_json::json!({
                "source": "arcw agent script run",
                "deterministic_cli_session": true,
                "state": agent_values_to_json(&self.states),
            }),
        }
    }

    pub(super) fn capture_blobs(&self) -> &[AgentCaptureBlob] {
        &self.capture_blobs
    }
}

impl AgentSession for CliAgentSession {
    type Error = Infallible;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(AgentSessionInfo {
            session_id: "session.cli".to_owned(),
            program_hash: self.program_hash.clone(),
            project_entities: self.project_entities.clone(),
            project_graph: self.project_graph.clone(),
            profile: Some("cli".to_owned()),
            capabilities: vec![
                "agent.observe".to_owned(),
                "agent.wait".to_owned(),
                "agent.capture".to_owned(),
                "agent.act.semantic".to_owned(),
                "agent.resource.read".to_owned(),
                "debug.read".to_owned(),
                "debug.record".to_owned(),
            ],
        })
    }

    fn observe(&mut self, _request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        Ok(self.observation())
    }

    fn act(&mut self, _action: AgentAction) -> Result<ActionResult, Self::Error> {
        let before_tick = self.tick;
        self.tick = self.tick.saturating_add(1);
        Ok(ActionResult {
            accepted: true,
            before_tick,
            after_tick: self.tick,
            before_state_hash: format!("cli.state.{before_tick}"),
            after_state_hash: format!("cli.state.{}", self.tick),
        })
    }

    fn capture(&mut self, request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        self.captures = self.captures.saturating_add(1);
        let (media_type, bytes) = cli_capture_blob_bytes(&request);
        let content_hash = format!("blake3:{}", blake3::hash(&bytes).to_hex());
        let byte_len = u64::try_from(bytes.len()).expect("capture blob length fits u64");
        self.capture_blobs.push(AgentCaptureBlob {
            content_hash: content_hash.clone(),
            bytes,
        });
        let uri = format!("agent://capture/cli/{}-{}", request.name, self.captures);
        Ok(CaptureResult {
            uri: AgentResourceUri::new(uri).expect("generated capture uri is nonempty"),
            content_hash,
            media_type,
            byte_len,
        })
    }

    fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error> {
        Ok(AgentResource {
            uri: AgentResourceUri::new(uri).expect("requested resource URI is nonempty"),
            kind: AgentResourceKind::ObservationLatest,
            mime_type: "application/json".to_owned(),
            hash: "cli-resource".to_owned(),
            image: None,
            body: AgentResourceBody::Json(serde_json::json!({
                "uri": uri,
                "source": "arcw agent script run"
            })),
        })
    }

    fn step_frames(&mut self, count: u32) -> Result<ObservationEnvelope, Self::Error> {
        self.tick = self.tick.saturating_add(u64::from(count.max(1)));
        Ok(self.observation())
    }
}

pub(super) fn cli_capture_blob_bytes(request: &CaptureRequest) -> (String, Vec<u8>) {
    match request.format {
        CaptureFormat::Png => ("image/png".to_owned(), CLI_TRANSPARENT_PNG.to_vec()),
        CaptureFormat::RawRgba => ("application/octet-stream".to_owned(), vec![0, 0, 0, 0]),
    }
}

pub(super) const CLI_TRANSPARENT_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

pub(super) fn agent_values_to_json(values: &BTreeMap<String, AgentValue>) -> serde_json::Value {
    serde_json::Value::Object(
        values
            .iter()
            .map(|(key, value)| (key.clone(), agent_value_to_json(value)))
            .collect(),
    )
}

pub(super) fn agent_value_to_json(value: &AgentValue) -> serde_json::Value {
    match value {
        AgentValue::Null => serde_json::Value::Null,
        AgentValue::Bool(value) => serde_json::Value::Bool(*value),
        AgentValue::I64(value) => serde_json::Value::Number((*value).into()),
        AgentValue::U64(value) => serde_json::Number::from(*value).into(),
        AgentValue::F64(value) => serde_json::Number::from_f64(*value)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        AgentValue::String(value) => serde_json::Value::String(value.clone()),
        AgentValue::Entity(value) => serde_json::Value::String(value.as_str().to_owned()),
        AgentValue::List(values) => {
            serde_json::Value::Array(values.iter().map(agent_value_to_json).collect())
        }
        AgentValue::Map(values) => agent_values_to_json(values),
    }
}

#[cfg(test)]
mod removed_role_tests {
    use super::*;
    use arcweft_core::{
        effect::{
            RuntimeAssertion, RuntimeAssertionFailure, RuntimeAssertionGuardId,
            RuntimeAssertionProfile,
        },
        engine::{FlowExit, FlowFiberStatus},
    };

    #[test]
    fn awfagent_compiler_consumer_rejects_removed_role_declarations() {
        let target = agent_script_standalone_compile_target(&[]).expect("standalone target");
        for source in [
            "state GameState {\n    value: i32\n}\n",
            "reducer update(state: GameState, event: GameEvent) -> GameState {\n    state\n}\n",
            "agent @agent.smoke smoke() {\n    Ok(())\n}\n",
        ] {
            let Err(error) = compile_agent_script_source(
                Path::new("removed.awfagent"),
                source.to_owned(),
                "entry.agent.main",
                &target,
            ) else {
                panic!("removed declaration must fail the .awfagent compiler consumer");
            };
            assert!(
                error.starts_with("parse:"),
                "removed declaration must fail at parse, got: {error}"
            );
        }
    }

    #[test]
    fn decoded_agent_bundle_projects_persisted_runtime_assertion_without_changing_status() {
        let failure = RuntimeAssertionFailure::new(RuntimeAssertion::new(
            RuntimeAssertionGuardId::try_from_bytes([0x73; 16]).expect("fixture assertion guard"),
            "ready".to_owned(),
            "runtime condition failed".to_owned(),
            RuntimeAssertionProfile::Always,
        ));
        let report = agent_script_run_success_report(
            "assertion.awfagent",
            1,
            AgentControllerRunReport {
                steps: 1,
                host_calls: 0,
                responses: Vec::new(),
                assertion_failures: vec![failure],
                events_emitted: 0,
                final_status: Some(FlowFiberStatus::Done(FlowExit::Return("done".to_owned()))),
            },
            None,
            0,
            AgentBlobWriteReport::default(),
            None,
        )
        .expect("persisted assertion diagnostic projects");

        assert!(report.ok);
        assert_eq!(report.assertion_diagnostics.len(), 1);
        assert_eq!(
            report.assertion_diagnostics[0].code,
            "runtime.assertion_failed"
        );
        assert_eq!(
            report.assertion_diagnostics[0].message,
            "runtime condition failed"
        );
        assert_eq!(report.assertion_diagnostics[0].identity, "persisted_only");
        assert_eq!(
            report.final_status.as_deref(),
            Some("Done(Return(\"done\"))")
        );
    }
}
