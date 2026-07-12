use super::repl_cli_command::{
    AgentReplParsedInput, CliReplCommandJsonOptions, CliReplCommandResult,
    cli_repl_command_result_json, cli_repl_protocol_unavailable_result, parse_agent_repl_input,
};
use super::*;
use arcweft_agent_repl::command::{
    ReplCommandEvidence, ReplCommandId, ReplCommandJsonOptions, ReplCommandResult,
    ReplCommandStatus, ReplTracePolicy, repl_command_result_json,
};

pub(super) fn agent_mcp_command(
    options: &AgentMcpOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut state = AgentMcpState {
        content_policy_mode: options.content_policy_mode,
        ..AgentMcpState::default()
    };
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| {
            eprintln!("error: failed to read MCP request: {error}");
            ExitCode::FAILURE
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<AgentMcpJsonRpcRequest>(&line) {
            Ok(request) => agent_mcp_handle_request(request, &mut state, adapter_registrars),
            Err(error) => Some(agent_mcp_error_response(
                None,
                -32700,
                &format!("parse error: {error}"),
            )),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut stdout, &response).map_err(|error| {
                eprintln!("error: failed to write MCP response: {error}");
                ExitCode::FAILURE
            })?;
            stdout.write_all(b"\n").map_err(|error| {
                eprintln!("error: failed to write MCP response newline: {error}");
                ExitCode::FAILURE
            })?;
            stdout.flush().map_err(|error| {
                eprintln!("error: failed to flush MCP response: {error}");
                ExitCode::FAILURE
            })?;
        }
    }
    Ok(())
}

#[derive(Default)]
pub(super) struct AgentMcpState {
    pub(super) content_policy_mode: AgentContentPolicyMode,
    pub(super) published_resources: AgentPublishedResourceCache,
    pub(super) report: Option<AgentObservationReport>,
    pub(super) image_output: Option<AgentImageOutput>,
    pub(super) image_frames: AgentImageFrameStore,
    pub(super) capture_resources: Vec<AgentResource>,
    pub(super) trace_resources: Vec<AgentResource>,
    pub(super) rag_context_packs: Vec<RagContextPack>,
    pub(super) project_context: Option<AgentMcpProjectContext>,
    pub(super) runtime: Option<NativeAgentRuntimeState>,
    pub(super) observe_options: Option<AgentObserveOptions>,
    pub(super) repl_session: Option<arcweft_agent_repl::ReplSession>,
    pub(super) repl_tier_handler: arcweft_agent_repl::ReplTierCommandHandler,
}

#[derive(Clone, Debug, Default)]
pub(super) struct AgentPublishedResourceCache {
    by_public_uri: BTreeMap<String, arcweft_agent_policy::PublishedAgentResource>,
    public_uri_by_source_uri: BTreeMap<String, String>,
}

impl AgentPublishedResourceCache {
    pub(super) fn clear(&mut self) {
        self.by_public_uri.clear();
        self.public_uri_by_source_uri.clear();
    }

    pub(super) fn get(&self, uri: &str) -> Option<&arcweft_agent_policy::PublishedAgentResource> {
        self.by_public_uri.get(uri)
    }

    pub(super) fn store(
        &mut self,
        source_uri: &str,
        published: arcweft_agent_policy::PublishedAgentResource,
    ) -> arcweft_agent_policy::PublishedAgentResource {
        self.remove_source_uri(source_uri);
        let public_uri = published.resource().uri.clone();
        for key in agent_published_resource_source_keys(source_uri) {
            self.public_uri_by_source_uri
                .insert(key, public_uri.clone());
        }
        self.by_public_uri.insert(public_uri, published.clone());
        published
    }

    pub(super) fn remove_source_uri(&mut self, source_uri: &str) {
        for key in agent_published_resource_source_keys(source_uri) {
            if let Some(public_uri) = self.public_uri_by_source_uri.remove(&key) {
                let still_referenced = self
                    .public_uri_by_source_uri
                    .values()
                    .any(|candidate| candidate == &public_uri);
                if !still_referenced {
                    self.by_public_uri.remove(&public_uri);
                }
            }
        }
    }
}

pub(super) fn agent_publish_resource_for_state(
    state: &mut AgentMcpState,
    resource: AgentResource,
) -> Result<arcweft_agent_policy::PublishedAgentResource, String> {
    let source_uri = resource.uri.clone();
    let published = agent_publish_resource_with_mode(state.content_policy_mode, resource)?;
    Ok(state.published_resources.store(&source_uri, published))
}

pub(super) fn agent_publish_resources_for_state(
    state: &mut AgentMcpState,
    resources: Vec<AgentResource>,
) -> Result<Vec<arcweft_agent_policy::PublishedAgentResource>, String> {
    resources
        .into_iter()
        .map(|resource| agent_publish_resource_for_state(state, resource))
        .collect()
}

pub(super) fn agent_mcp_cached_published_resource(
    state: &AgentMcpState,
    uri: &str,
) -> Option<arcweft_agent_policy::PublishedAgentResource> {
    state.published_resources.get(uri).cloned()
}

fn agent_published_resource_source_keys(source_uri: &str) -> Vec<String> {
    let mut keys = vec![source_uri.to_owned()];
    if let Some((base, _)) = source_uri.split_once('?')
        && base != source_uri
    {
        keys.push(base.to_owned());
    }
    keys
}

pub(super) struct AgentObservationState {
    pub(super) report: AgentObservationReport,
    pub(super) image_frames: AgentImageFrameStore,
}

pub(super) struct NativeAgentObservedSnapshot {
    pub(super) report: AgentObservationReport,
    pub(super) image_frames: AgentImageFrameStore,
}

pub(super) struct NativeAgentRuntimeState {
    pub(super) session: arcweft_runtime_driver::session::BundleSession,
    pub(super) images: arcweft_player_scene::images::BundleImageCatalog,
    pub(super) input: arcweft_player_scene::input::InputController,
    pub(super) prepared_frame: Option<arcweft_player_scene::frame::PlayerPreparedFrame>,
    pub(super) source_path: PathBuf,
    pub(super) project_context: AgentMcpProjectContext,
    pub(super) shared_capture: arcweft_render_wgpu::offscreen::SharedOffscreenCapture,
    pub(super) host: Option<NativeTaskBridge>,
    pub(super) task_events: Vec<arcweft_core::task::TaskEvent>,
    pub(super) next_clock_millis: u64,
}

#[derive(Clone, Debug)]
pub(super) struct AgentMcpProjectContext {
    pub(super) project_entities: serde_json::Value,
    pub(super) project_graph: serde_json::Value,
}

impl AgentMcpProjectContext {
    pub(super) fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "project_entities": self.project_entities,
            "project_graph": self.project_graph,
        })
    }
}

pub(super) fn agent_mcp_project_context_from_hir(
    hir: &arcweft_lang_hir::model::HirModule,
    source_path: &Path,
) -> Result<AgentMcpProjectContext, String> {
    let project = project_semantic_index_from_hir(
        hir,
        ProgramHash::new(format!("native-source:{}", source_path.display())),
        &SourceName::path(source_path.display().to_string()),
    )
    .map_err(|error| error.to_string())?;
    let project_entities =
        arcweft_compiler::agent_project::agent_required_entities_from_project(&project)
            .map_err(|error| error.to_string())?;
    let project_graph = arcweft_compiler::agent_project::agent_project_graph_from_project(&project)
        .map_err(|error| error.to_string())?;
    Ok(AgentMcpProjectContext {
        project_entities: agent_script_project_entities_metadata(&project_entities),
        project_graph: agent_script_project_graph_metadata(&project_graph),
    })
}

pub(super) struct AgentMcpObservation {
    pub(super) report: AgentObservationReport,
    pub(super) image_output: Option<AgentImageOutput>,
    pub(super) image_frames: AgentImageFrameStore,
    pub(super) runtime: NativeAgentRuntimeState,
    pub(super) options: AgentObserveOptions,
}

pub(super) struct AgentMcpFrame {
    pub(super) report: AgentObservationReport,
    pub(super) image_output: Option<AgentImageOutput>,
    pub(super) image_frames: AgentImageFrameStore,
    pub(super) resources: Vec<AgentResource>,
}

#[derive(serde::Deserialize)]
pub(super) struct AgentMcpJsonRpcRequest {
    #[serde(default)]
    pub(super) id: Option<serde_json::Value>,
    pub(super) method: String,
    #[serde(default)]
    pub(super) params: serde_json::Value,
}

pub(super) fn agent_mcp_handle_request(
    request: AgentMcpJsonRpcRequest,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Option<serde_json::Value> {
    let id = request.id;
    let result = match request.method.as_str() {
        "notifications/initialized" => return None,
        "initialize" => Ok(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "serverInfo": {
                "name": "arcweft-agent",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "tools/list" => Ok(serde_json::json!({
            "tools": agent_tool_descriptors()
        })),
        "resources/templates/list" => serde_json::to_value(list_resource_templates_result())
            .map_err(|error| format!("failed to serialize MCP resource templates: {error}")),
        "resources/list" => agent_mcp_resource_list(state),
        "resources/read" => agent_mcp_resource_read(&request.params, state),
        "tools/call" => agent_mcp_tool_call(&request.params, state, adapter_registrars),
        method => Err(format!("unsupported MCP method `{method}`")),
    };
    Some(match result {
        Ok(result) => agent_mcp_success_response(id.as_ref(), &result),
        Err(message) => agent_mcp_error_response(id.as_ref(), -32603, &message),
    })
}

pub(super) fn agent_mcp_resource_list(
    state: &mut AgentMcpState,
) -> Result<serde_json::Value, String> {
    let resources = agent_mcp_current_resources(state)
        .map_err(|_| "failed to build Agent resource list".to_owned())?;
    let published = agent_publish_resources_for_state(state, resources)?;
    serde_json::to_value(list_resources_result(&published))
        .map_err(|error| format!("failed to serialize MCP resource list: {error}"))
}

pub(super) fn agent_mcp_resource_read(
    params: &serde_json::Value,
    state: &mut AgentMcpState,
) -> Result<serde_json::Value, String> {
    let max_privacy = agent_mcp_max_privacy_argument(params, "resources/read")?;
    let uri = params
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "resources/read requires params.uri".to_owned())?;
    if let Some(published) = agent_mcp_cached_published_resource(state, uri) {
        if let Some(error) = agent_mcp_resource_read_privacy_error(
            published.resource(),
            max_privacy,
            "resources/read",
        ) {
            return Err(agent_mcp_resource_read_privacy_message(&error));
        }
        let read = read_resource_result(&published)
            .map_err(|error| format!("failed to serialize MCP resource: {error}"))?;
        return serde_json::to_value(read)
            .map_err(|error| format!("failed to serialize MCP read: {error}"));
    }
    if let Some(resource) = agent_mcp_session_context_resource_for_uri(state, uri)
        .map_err(|error| format!("failed to build Agent session context: {error}"))?
    {
        if let Some(error) =
            agent_mcp_resource_read_privacy_error(&resource, max_privacy, "resources/read")
        {
            return Err(agent_mcp_resource_read_privacy_message(&error));
        }
        let published = agent_publish_resource_for_state(state, resource)?;
        let read = read_resource_result(&published)
            .map_err(|error| format!("failed to serialize MCP session context: {error}"))?;
        return serde_json::to_value(read)
            .map_err(|error| format!("failed to serialize MCP read: {error}"));
    }
    if let Some(resource) = agent_mcp_cached_trace_resource(state, uri) {
        if let Some(error) =
            agent_mcp_resource_read_privacy_error(&resource, max_privacy, "resources/read")
        {
            return Err(agent_mcp_resource_read_privacy_message(&error));
        }
        let published = agent_publish_resource_for_state(state, resource)?;
        let read = read_resource_result(&published)
            .map_err(|error| format!("failed to serialize MCP resource: {error}"))?;
        return serde_json::to_value(read)
            .map_err(|error| format!("failed to serialize MCP read: {error}"));
    }
    let Some(report) = state.report.clone() else {
        return Err(
            "resources/read requires a prior arcweft.observe call or arcweft.trace.read call"
                .to_owned(),
        );
    };
    let image_output = state.image_output.clone();
    let resource = if let Some(resource) = agent_mcp_cached_capture_resource(state, uri)
        .or_else(|| agent_observe_cached_image_resource(&report, image_output.as_ref(), uri))
    {
        resource
    } else {
        agent_mcp_uncached_resource_by_uri(&report, uri, state)
            .map_err(|_| format!("failed to read Agent resource `{uri}`"))?
    };
    if let Some(error) =
        agent_mcp_resource_read_privacy_error(&resource, max_privacy, "resources/read")
    {
        return Err(agent_mcp_resource_read_privacy_message(&error));
    }
    let published = agent_publish_resource_for_state(state, resource)?;
    let read = read_resource_result(&published)
        .map_err(|error| format!("failed to serialize MCP resource: {error}"))?;
    serde_json::to_value(read).map_err(|error| format!("failed to serialize MCP read: {error}"))
}

pub(super) fn agent_mcp_tool_call(
    params: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<serde_json::Value, String> {
    let name = params
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "tools/call requires params.name".to_owned())?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    match name {
        "arcweft.observe" => agent_mcp_call_observe(&arguments, state, adapter_registrars),
        "arcweft.action" | "arcweft.act" => {
            let tool = agent_mcp_call_action(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP action result: {error}"))
        }
        "arcweft.session.step_frames" => {
            let tool = agent_mcp_call_step_frames(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP step result: {error}"))
        }
        "arcweft.wait" => {
            let tool = agent_mcp_call_wait(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP wait result: {error}"))
        }
        "arcweft.script.run" => {
            let tool = agent_mcp_call_script_run(&arguments, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP script run result: {error}"))
        }
        "arcweft.session.info" => {
            let tool = agent_mcp_call_session_info(state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP session info: {error}"))
        }
        MCP_REPL_COMMAND_TOOL => {
            let tool = agent_mcp_call_repl_command(&arguments, state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP REPL command result: {error}"))
        }
        "arcweft.resource.read" => {
            let tool = agent_mcp_call_resource_read(&arguments, state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP tool result: {error}"))
        }
        "arcweft.capture" => {
            let tool = agent_mcp_call_capture(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP capture result: {error}"))
        }
        "arcweft.hit_test" => {
            let tool = agent_mcp_call_hit_test(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP hit-test result: {error}"))
        }
        "arcweft.get_state" => {
            let tool = agent_mcp_call_get_state(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP state result: {error}"))
        }
        "arcweft.signal_get" => {
            let tool = agent_mcp_call_signal_get(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP signal result: {error}"))
        }
        "arcweft.log_query" => {
            let tool = agent_mcp_call_log_query(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP log result: {error}"))
        }
        tool if tool.starts_with("arcweft.debug.") => agent_mcp_debug_tool_call(tool, &arguments),
        "arcweft.rag.query" => {
            let tool = agent_mcp_call_rag_query(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP RAG result: {error}"))
        }
        "arcweft.rag.explain" => {
            let tool = agent_mcp_call_rag_explain(&arguments, state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP RAG explanation: {error}"))
        }
        "arcweft.rag.context.read" => {
            let tool = agent_mcp_call_rag_context_read(&arguments, state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP RAG context item: {error}"))
        }
        "arcweft.trace.read" => {
            let tool = agent_mcp_call_trace_read(&arguments, state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP trace result: {error}"))
        }
        tool => Err(format!("unsupported Arcweft MCP tool `{tool}`")),
    }
}

pub(super) fn agent_mcp_debug_tool_call(
    name: &str,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let (tool, label) = match name {
        "arcweft.debug.search" => (
            agent_mcp_call_debug_search(arguments)?,
            "MCP debug search result",
        ),
        "arcweft.debug.script.runs" => (
            agent_mcp_call_debug_script_runs(arguments)?,
            "MCP debug script runs",
        ),
        "arcweft.debug.sessions.close_stale" => (
            agent_mcp_call_debug_close_stale_sessions(arguments)?,
            "MCP debug close stale sessions",
        ),
        "arcweft.debug.session.timeline" => (
            agent_mcp_call_debug_session_timeline(arguments)?,
            "MCP debug timeline",
        ),
        "arcweft.debug.repl.cells" => (
            agent_mcp_call_debug_repl_cells(arguments)?,
            "MCP debug REPL cells",
        ),
        "arcweft.debug.source.files" => (
            agent_mcp_call_debug_source_files(arguments)?,
            "MCP debug source files",
        ),
        "arcweft.debug.graph.inventory" => (
            agent_mcp_call_debug_graph_inventory(arguments)?,
            "MCP debug graph inventory",
        ),
        tool => return Err(format!("unsupported Arcweft MCP tool `{tool}`")),
    };
    serde_json::to_value(tool).map_err(|error| format!("failed to serialize {label}: {error}"))
}

pub(super) fn agent_mcp_call_observe(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<serde_json::Value, String> {
    let observed = agent_mcp_run_observation(arguments, adapter_registrars)?;
    agent_mcp_store_observation(state, observed);
    let resources = agent_mcp_current_resources(state)
        .map_err(|_| "failed to build Agent session resource list".to_owned())?;
    let published = agent_publish_resources_for_state(state, resources)?;
    let tool = tool_result_for_resources(&published);
    serde_json::to_value(tool)
        .map_err(|error| format!("failed to serialize MCP tool result: {error}"))
}

pub(super) fn agent_mcp_store_observation(
    state: &mut AgentMcpState,
    observed: AgentMcpObservation,
) {
    state.report = Some(observed.report);
    state.image_output = observed.image_output;
    state.image_frames = observed.image_frames;
    state.project_context = Some(observed.runtime.project_context.clone());
    state.runtime = Some(observed.runtime);
    state.observe_options = Some(observed.options);
    state.capture_resources.clear();
    state.published_resources.clear();
}

pub(super) fn agent_mcp_call_session_info(
    state: &mut AgentMcpState,
) -> Result<McpCallToolResult, String> {
    let capabilities = agent_mcp_session_capabilities();
    let info = if let Some(report) = state.report.clone() {
        let resources = agent_mcp_current_resources(state)
            .map_err(|_| "failed to build Agent session resource list".to_owned())?;
        let published = agent_publish_resources_for_state(state, resources)?;
        let descriptors = list_resources_result(&published).resources;
        let latest_capture = agent_mcp_latest_capture_resource(state).cloned();
        let latest_capture_descriptor = latest_capture
            .clone()
            .and_then(|resource| agent_publish_resource_for_state(state, resource).ok())
            .as_ref()
            .map(resource_descriptor);
        serde_json::json!({
            "observed": true,
            "session_id": report.session_id,
            "program_hash": agent_mcp_program_hash_for_state(state),
            "profile": null,
            "capabilities": capabilities,
            "project_entities": [],
            "project_graph": {},
            "tick": report.tick,
            "frame_id": report.frame_id,
            "source": report.source,
            "final_status": report.final_status,
            "resource_count": descriptors.len(),
            "resources": descriptors,
            "resource_templates": list_resource_templates_result().resource_templates,
            "images": report.images,
            "layers": report.layers,
            "objects": report.objects,
            "capture_resource_count": state.capture_resources.len(),
            "shared_capture_session_active": state.runtime.is_some(),
            "project": state.project_context.as_ref().map(AgentMcpProjectContext::to_json),
            "latest_capture": latest_capture.as_ref().and_then(|resource| resource.image.as_ref()),
            "latest_capture_uri": latest_capture.as_ref().map(|resource| resource.uri.as_str()),
            "latest_capture_resource": latest_capture_descriptor,
            "trace_resource_count": state.trace_resources.len(),
        })
    } else {
        let published = agent_publish_resources_for_state(state, state.trace_resources.clone())?;
        let descriptors = list_resources_result(&published).resources;
        serde_json::json!({
            "observed": false,
            "session_id": "session.mcp.unobserved",
            "program_hash": "program.mcp.unobserved",
            "profile": null,
            "capabilities": capabilities,
            "project_entities": [],
            "project_graph": {},
            "resource_count": descriptors.len(),
            "resources": descriptors,
            "resource_templates": list_resource_templates_result().resource_templates,
            "images": [],
            "layers": [],
            "objects": [],
            "capture_resource_count": 0,
            "trace_resource_count": state.trace_resources.len(),
            "shared_capture_session_active": false,
            "project": state.project_context.as_ref().map(AgentMcpProjectContext::to_json),
            "latest_capture": null,
            "latest_capture_uri": null,
            "latest_capture_resource": null,
        })
    };
    let text = serde_json::to_string(&info)
        .map_err(|error| format!("failed to serialize Agent session info: {error}"))?;
    Ok(McpCallToolResult {
        content: vec![McpContentBlock::Text { text }],
        is_error: false,
    })
}

fn agent_mcp_session_capabilities() -> Vec<&'static str> {
    vec![
        "observe",
        "act",
        "capture",
        "resource_read",
        "step_frames",
        "repl_command",
        "hit_test",
        "rag",
    ]
}

pub(super) fn agent_mcp_call_repl_command(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
) -> Result<McpCallToolResult, String> {
    let request = McpReplCommandRequest::from_arguments(arguments)
        .map_err(|error| format!("invalid {MCP_REPL_COMMAND_TOOL} arguments: {error}"))?;
    let parsed = match parse_agent_repl_input(&request.input) {
        Ok(parsed) => parsed,
        Err(error) => {
            let result = ReplCommandResult::error(
                agent_mcp_repl_command_id(&request),
                ReplCommandEvidence::Empty,
                error.into_diagnostic(),
            );
            return Ok(agent_mcp_shared_repl_tool_result(&result, &request));
        }
    };
    match parsed {
        AgentReplParsedInput::Shared(_) => agent_mcp_call_shared_repl_command(&request, state),
        AgentReplParsedInput::Cli(command) => {
            let result = cli_repl_protocol_unavailable_result(
                agent_mcp_repl_command_id(&request),
                &command,
                ReplTracePolicy::from(request.trace_policy),
            );
            Ok(agent_mcp_cli_repl_tool_result(&result, &request))
        }
    }
}

fn agent_mcp_call_shared_repl_command(
    request: &McpReplCommandRequest,
    state: &mut AgentMcpState,
) -> Result<McpCallToolResult, String> {
    agent_mcp_ensure_repl_session(state)?;
    let Some(session) = state.repl_session.as_mut() else {
        return Err("failed to initialize MCP REPL command session".to_owned());
    };
    let mut endpoint = McpReplCommandEndpoint::new(session, &mut state.repl_tier_handler);
    Ok(endpoint.execute(request))
}

fn agent_mcp_repl_command_id(request: &McpReplCommandRequest) -> ReplCommandId {
    ReplCommandId::new(request.command_id.max(1))
}

fn agent_mcp_shared_repl_json_options(request: &McpReplCommandRequest) -> ReplCommandJsonOptions {
    ReplCommandJsonOptions {
        max_items: request.max_items,
        max_string_bytes: request.max_string_bytes,
        include_diagnostics: request.include_diagnostics,
    }
}

fn agent_mcp_cli_repl_json_options(request: &McpReplCommandRequest) -> CliReplCommandJsonOptions {
    CliReplCommandJsonOptions::new(request.include_diagnostics)
}

fn agent_mcp_shared_repl_tool_result(
    result: &ReplCommandResult,
    request: &McpReplCommandRequest,
) -> McpCallToolResult {
    agent_mcp_repl_tool_result_from_json(
        &repl_command_result_json(result, &agent_mcp_shared_repl_json_options(request)),
        result.status,
    )
}

fn agent_mcp_cli_repl_tool_result(
    result: &CliReplCommandResult,
    request: &McpReplCommandRequest,
) -> McpCallToolResult {
    agent_mcp_repl_tool_result_from_json(
        &cli_repl_command_result_json(result, agent_mcp_cli_repl_json_options(request)),
        result.status,
    )
}

fn agent_mcp_repl_tool_result_from_json(
    json: &serde_json::Value,
    status: ReplCommandStatus,
) -> McpCallToolResult {
    let text = serde_json::to_string(&json).unwrap_or_else(|error| {
        serde_json::json!({ "formatter_error": error.to_string() }).to_string()
    });
    McpCallToolResult {
        content: vec![McpContentBlock::Text { text }],
        is_error: matches!(
            status,
            ReplCommandStatus::Rejected | ReplCommandStatus::Error
        ),
    }
}

fn agent_mcp_ensure_repl_session(state: &mut AgentMcpState) -> Result<(), String> {
    if state.repl_session.is_some() {
        return Ok(());
    }
    let project = agent_script_project_index(&[])?;
    state.repl_session = Some(arcweft_agent_repl::ReplSession::new(
        arcweft_agent_repl::ReplBaseSnapshot::from_project("mcp-agent-repl", project),
        arcweft_agent_repl::ReplSessionOptions::default(),
    ));
    Ok(())
}

fn agent_mcp_program_hash_for_state(state: &AgentMcpState) -> String {
    state.runtime.as_ref().map_or_else(
        || "program.mcp.unobserved".to_owned(),
        |runtime| format!("native-source:{}", runtime.source_path.display()),
    )
}

pub(super) fn agent_mcp_call_hit_test(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    let x = agent_mcp_u32_argument(arguments, "x", "arcweft.hit_test")?
        .ok_or_else(|| "arcweft.hit_test requires arguments.x".to_owned())?;
    let y = agent_mcp_u32_argument(arguments, "y", "arcweft.hit_test")?
        .ok_or_else(|| "arcweft.hit_test requires arguments.y".to_owned())?;
    if agent_mcp_arguments_request_observe(arguments) {
        let observed = agent_mcp_run_observation(arguments, adapter_registrars)?;
        agent_mcp_store_observation(state, observed);
    }
    let Some(report) = &state.report else {
        return Err(
            "arcweft.hit_test requires a prior arcweft.observe call, arguments.source, or arguments.profile"
                .to_owned(),
        );
    };
    let hit_test = agent_hit_test_report(report, x, y);
    let text = serde_json::to_string_pretty(&hit_test)
        .map_err(|error| format!("failed to serialize hit-test result: {error}"))?;
    Ok(McpCallToolResult {
        content: vec![McpContentBlock::Text { text }],
        is_error: false,
    })
}

pub(super) fn agent_mcp_call_action(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let before = state.report.clone().ok_or_else(|| {
        "arcweft.action requires a prior arcweft.observe call, arguments.source, or arguments.profile"
            .to_owned()
    })?;
    let action = agent_mcp_action_argument(arguments, &before)?;
    let options = state
        .observe_options
        .clone()
        .ok_or_else(|| "arcweft.action requires an active native observation session".to_owned())?;
    let frame = {
        let runtime = state
            .runtime
            .as_mut()
            .ok_or_else(|| "arcweft.action requires an active native runtime session".to_owned())?;
        let step_input = match &action {
            AgentAction::Scroll(scroll) => {
                native_agent_scroll_region(runtime, &before, scroll)
                    .map_err(|error| error.to_string())?;
                BundleStepInput::default()
            }
            _ => native_agent_action_step_input(
                &before,
                runtime.session.presentation(),
                action.clone(),
            )
            .map_err(|error| error.to_string())?,
        };
        agent_mcp_observe_runtime(runtime, &options, step_input, adapter_registrars)?
    };
    let result = NativeAgentScriptSession::action_result(&before, &frame.report);
    let value = serde_json::json!({
        "accepted": result.accepted,
        "before_tick": result.before_tick,
        "after_tick": result.after_tick,
        "before_state_hash": result.before_state_hash,
        "after_state_hash": result.after_state_hash,
        "action": action,
        "after": {
            "tick": frame.report.tick,
            "frame_id": frame.report.frame_id,
            "state_hash": frame.report.state_hash,
            "render_hash": frame.report.render_hash,
            "final_status": frame.report.final_status,
            "actions": frame.report.actions,
            "scroll_regions": frame.report.scroll_regions,
        },
        "resource_count": frame.resources.len(),
    });
    agent_mcp_store_frame(state, frame);
    agent_mcp_json_tool_result(&value, "action")
}

pub(super) fn agent_mcp_call_step_frames(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let count = agent_mcp_u32_argument(arguments, "count", "arcweft.session.step_frames")?
        .unwrap_or(1)
        .max(1);
    let options = state
        .observe_options
        .clone()
        .ok_or_else(|| {
            "arcweft.session.step_frames requires an active native observation session".to_owned()
        })
        .map(|mut options| {
            options.steps = usize::try_from(count).unwrap_or(usize::MAX);
            options.capture_step = None;
            options
        })?;
    let frame = {
        let runtime = state.runtime.as_mut().ok_or_else(|| {
            "arcweft.session.step_frames requires an active native runtime session".to_owned()
        })?;
        agent_mcp_observe_runtime(
            runtime,
            &options,
            BundleStepInput::default(),
            adapter_registrars,
        )?
    };
    let value = serde_json::json!({
        "count": count,
        "tick": frame.report.tick,
        "frame_id": frame.report.frame_id,
        "state_hash": frame.report.state_hash,
        "render_hash": frame.report.render_hash,
        "final_status": frame.report.final_status,
        "actions": frame.report.actions,
        "resource_count": frame.resources.len(),
    });
    agent_mcp_store_frame(state, frame);
    agent_mcp_json_tool_result(&value, "step_frames")
}

pub(super) fn agent_mcp_call_wait(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let predicate = arguments
        .get("predicate")
        .ok_or_else(|| "arcweft.wait requires arguments.predicate".to_owned())
        .and_then(|value| {
            serde_json::from_value::<Predicate>(value.clone())
                .map_err(|error| format!("invalid arcweft.wait predicate: {error}"))
        })?;
    let timeout_millis = agent_mcp_u64_argument(arguments, "timeout_millis", "arcweft.wait")?
        .ok_or_else(|| "arcweft.wait requires arguments.timeout_millis".to_owned())?;
    let stable_frames =
        agent_mcp_u32_argument(arguments, "stable_frames", "arcweft.wait")?.unwrap_or(1);
    let poll_frames =
        agent_mcp_u32_argument(arguments, "poll_frames", "arcweft.wait")?.unwrap_or(1);
    let max_polls = (timeout_millis / u64::from(poll_frames.max(1))).max(1);
    let mut stable_seen = 0u32;
    let mut last_value = None;

    for poll_index in 0..max_polls {
        let options = state
            .observe_options
            .clone()
            .ok_or_else(|| "arcweft.wait requires an active native observation session".to_owned())
            .map(|mut options| {
                options.steps = usize::try_from(poll_frames.max(1)).unwrap_or(usize::MAX);
                options.capture_step = None;
                options
            })?;
        let frame = {
            let runtime = state.runtime.as_mut().ok_or_else(|| {
                "arcweft.wait requires an active native runtime session".to_owned()
            })?;
            agent_mcp_observe_runtime(
                runtime,
                &options,
                BundleStepInput::default(),
                adapter_registrars,
            )?
        };
        let matched = agent_mcp_predicate_matches(&predicate, &frame.report);
        let summary =
            agent_mcp_wait_report_value(&frame.report, matched, stable_seen, poll_index + 1);
        stable_seen = if matched {
            stable_seen.saturating_add(1)
        } else {
            0
        };
        let done = stable_seen >= stable_frames.max(1);
        let resources_len = frame.resources.len();
        agent_mcp_store_frame(state, frame);
        last_value = Some(serde_json::json!({
            "matched": matched,
            "stable_seen": stable_seen,
            "stable_required": stable_frames.max(1),
            "polls": poll_index + 1,
            "timeout_millis": timeout_millis,
            "poll_frames": poll_frames.max(1),
            "resource_count": resources_len,
            "observation": summary,
        }));
        if done {
            return agent_mcp_json_tool_result(
                last_value.as_ref().expect("wait result exists"),
                "wait result",
            );
        }
    }

    let value = last_value.unwrap_or_else(|| {
        serde_json::json!({
            "matched": false,
            "stable_seen": 0,
            "stable_required": stable_frames.max(1),
            "polls": 0,
            "timeout_millis": timeout_millis,
            "poll_frames": poll_frames.max(1),
        })
    });
    agent_mcp_json_tool_error(&value, "wait timeout")
}

pub(super) fn agent_mcp_call_script_run(
    arguments: &serde_json::Value,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    let options = agent_mcp_script_run_options(arguments)?;
    let report = match agent_script_run_input(&options) {
        Ok(input) => agent_script_run_bundle(&options, &input, adapter_registrars)
            .map_err(|code| format!("arcweft.script.run failed with exit code {code:?}"))?,
        Err(error) => AgentScriptRunReport {
            path: options.path.display().to_string(),
            ok: false,
            agents: 0,
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
            error: Some(error),
        },
    };
    let value = serde_json::to_value(&report)
        .map_err(|error| format!("failed to serialize Agent Script run report: {error}"))?;
    if report.ok {
        agent_mcp_json_tool_result(&value, "script run")
    } else {
        agent_mcp_json_tool_error(&value, "script run")
    }
}

pub(super) fn agent_mcp_script_run_options(
    arguments: &serde_json::Value,
) -> Result<AgentScriptRunOptions, String> {
    let path = arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "arcweft.script.run requires arguments.path".to_owned())?;
    let native_source = arguments
        .get("native_source")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from);
    let profile = arguments
        .get("profile")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    if native_source.is_some() && profile.is_some() {
        return Err(
            "arcweft.script.run arguments.native_source and arguments.profile are mutually exclusive"
                .to_owned(),
        );
    }
    Ok(AgentScriptRunOptions {
        path,
        json: true,
        native_source,
        native_profile: ProfileOptions {
            profile,
            manifest: arguments
                .get("manifest")
                .and_then(serde_json::Value::as_str)
                .map_or_else(|| PathBuf::from("arcw.toml"), PathBuf::from),
        },
        entry: arguments
            .get("entry")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        flow: arguments
            .get("flow")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        executor: agent_mcp_value_enum_argument(arguments, "executor", "arcweft.script.run")?
            .unwrap_or(CliRuntimeExecutorTier::BytecodeVm),
        pure_backend: agent_mcp_value_enum_argument(
            arguments,
            "pure_backend",
            "arcweft.script.run",
        )?,
        pure_workers: agent_mcp_pure_workers_argument(arguments, "arcweft.script.run")?,
        pure_batch_min_len: agent_mcp_usize_argument(arguments, "pure_batch_min_len"),
        pure_object_artifacts: agent_mcp_bool_argument(
            arguments,
            "pure_object_artifacts",
            "arcweft.script.run",
        )?
        .unwrap_or(false),
        math_backend: agent_mcp_value_enum_argument(
            arguments,
            "math_backend",
            "arcweft.script.run",
        )?,
        math_wgpu_min_elements: agent_mcp_usize_argument(arguments, "math_wgpu_min_elements"),
        native_steps: agent_mcp_usize_argument(arguments, "native_steps").unwrap_or(8),
        native_mode: agent_mcp_value_enum_argument(arguments, "native_mode", "arcweft.script.run")?
            .unwrap_or(CliRuntimeStepMode::Drain),
        native_max_ops: agent_mcp_usize_argument(arguments, "native_max_ops").unwrap_or(64),
        values: agent_mcp_runtime_bindings(arguments)?,
        viewport_width: agent_mcp_u32_argument(arguments, "viewport_width", "arcweft.script.run")?
            .unwrap_or(AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH),
        viewport_height: agent_mcp_u32_argument(
            arguments,
            "viewport_height",
            "arcweft.script.run",
        )?
        .unwrap_or(AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT),
        capture_time_seconds: agent_mcp_capture_time_argument(arguments, "arcweft.script.run")?,
        max_steps: agent_mcp_usize_argument(arguments, "max_steps").unwrap_or(256),
        max_ops: agent_mcp_usize_argument(arguments, "max_ops").unwrap_or(1024),
        signals: agent_mcp_script_signal_args(arguments)?,
        states: agent_mcp_script_state_args(arguments)?,
        trace_out: arguments
            .get("trace_out")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from),
        blob_dir: arguments
            .get("blob_dir")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from),
        debug_db: arguments
            .get("debug_db")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from),
        run_id: arguments
            .get("run_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("run.cli")
            .to_owned(),
    })
}

pub(super) fn agent_mcp_script_signal_args(
    arguments: &serde_json::Value,
) -> Result<Vec<super::AgentScriptSignalArg>, String> {
    agent_mcp_script_key_value_args(arguments, "signals", "arcweft.script.run signals").and_then(
        |values| {
            values
                .iter()
                .map(|value| parse_agent_script_signal_arg(value))
                .collect()
        },
    )
}

pub(super) fn agent_mcp_script_state_args(
    arguments: &serde_json::Value,
) -> Result<Vec<super::AgentScriptStateArg>, String> {
    agent_mcp_script_key_value_args(arguments, "state", "arcweft.script.run state").and_then(
        |values| {
            values
                .iter()
                .map(|value| parse_agent_script_state_arg(value))
                .collect()
        },
    )
}

pub(super) fn agent_mcp_script_key_value_args(
    arguments: &serde_json::Value,
    name: &str,
    context: &str,
) -> Result<Vec<String>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(Vec::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| format!("{context} must be a JSON object"))?;
    object
        .iter()
        .map(|(key, value)| {
            Ok(format!(
                "{key}={}",
                agent_mcp_script_scalar_arg(value, context)?
            ))
        })
        .collect()
}

pub(super) fn agent_mcp_script_scalar_arg(
    value: &serde_json::Value,
    context: &str,
) -> Result<String, String> {
    match value {
        serde_json::Value::Bool(value) => Ok(value.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::String(value) => serde_json::to_string(value)
            .map_err(|error| format!("failed to serialize {context} string: {error}")),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err(format!("{context} values must be bool, string, or number"))
        }
    }
}

pub(super) fn agent_mcp_runtime_bindings(
    arguments: &serde_json::Value,
) -> Result<Vec<arcweft_core::value::RuntimeBinding>, String> {
    let Some(value) = arguments.get("values") else {
        return Ok(Vec::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| "arcweft.script.run values must be a JSON object".to_owned())?;
    object
        .iter()
        .map(|(key, value)| {
            parse_runtime_binding_arg(&format!(
                "{key}={}",
                agent_mcp_runtime_value_arg(value, "arcweft.script.run values")?
            ))
        })
        .collect()
}

pub(super) fn agent_mcp_runtime_value_arg(
    value: &serde_json::Value,
    context: &str,
) -> Result<String, String> {
    match value {
        serde_json::Value::Bool(value) => Ok(value.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::String(value) => Ok(value.clone()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err(format!("{context} values must be bool, string, or number"))
        }
    }
}

pub(super) fn agent_mcp_value_enum_argument<T>(
    arguments: &serde_json::Value,
    name: &str,
    tool: &str,
) -> Result<Option<T>, String>
where
    T: ValueEnum,
{
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("{tool} argument {name} must be a string"))?;
    T::from_str(value, true)
        .map(Some)
        .map_err(|error| format!("{tool} argument {name} is invalid: {error}"))
}

pub(super) fn agent_mcp_pure_workers_argument(
    arguments: &serde_json::Value,
    tool: &str,
) -> Result<Option<CliRuntimePureWorkers>, String> {
    let Some(value) = arguments.get("pure_workers") else {
        return Ok(None);
    };
    let raw = match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Number(value) if value.is_u64() => value.to_string(),
        _ => {
            return Err(format!(
                "{tool} argument pure_workers must be `auto` or a positive integer"
            ));
        }
    };
    parse_runtime_pure_workers(&raw).map(Some)
}

pub(super) fn agent_mcp_bool_argument(
    arguments: &serde_json::Value,
    name: &str,
    tool: &str,
) -> Result<Option<bool>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    value
        .as_bool()
        .ok_or_else(|| format!("{tool} argument {name} must be a boolean"))
        .map(Some)
}

pub(super) fn agent_mcp_store_frame(state: &mut AgentMcpState, frame: AgentMcpFrame) {
    state.report = Some(frame.report);
    state.image_output = frame.image_output;
    state.image_frames = frame.image_frames;
    state.capture_resources.clear();
    state.published_resources.clear();
}

pub(super) fn agent_mcp_action_argument(
    arguments: &serde_json::Value,
    report: &AgentObservationReport,
) -> Result<AgentAction, String> {
    if let Some(action_id) = arguments
        .get("action_id")
        .and_then(serde_json::Value::as_str)
    {
        let target = report
            .actions
            .iter()
            .find(|candidate| candidate.id == action_id)
            .ok_or_else(|| format!("arcweft.action action_id `{action_id}` is not observed"))?;
        return agent_mcp_action_from_target(arguments, target);
    }
    let kind = arguments
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            "arcweft.action requires arguments.action_id or arguments.kind".to_owned()
        })?;
    match kind {
        "advance_text" => Ok(AgentAction::AdvanceText),
        "select_choice" => {
            let target = agent_mcp_public_id_argument(arguments, "target", "arcweft.action")?;
            Ok(AgentAction::SelectChoice { choice: target })
        }
        "invoke" => {
            let target = agent_mcp_public_id_argument(arguments, "target", "arcweft.action")?;
            let action = arguments
                .get("action")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "arcweft.action kind invoke requires arguments.action".to_owned())?
                .to_owned();
            let args = agent_mcp_action_args(arguments)?;
            Ok(AgentAction::Invoke(Box::new(AgentInvokeAction {
                target,
                action,
                args: Box::new(args),
            })))
        }
        "scroll" => {
            let region = arguments
                .get("region")
                .and_then(serde_json::Value::as_str)
                .filter(|region| !region.is_empty())
                .ok_or_else(|| "arcweft.action kind scroll requires arguments.region".to_owned())?;
            agent_mcp_scroll_action(arguments, region)
        }
        _ => Err(format!(
            "arcweft.action kind must be one of advance_text, select_choice, invoke, or scroll: `{kind}`"
        )),
    }
}

fn agent_mcp_scroll_action(
    arguments: &serde_json::Value,
    observed_region: &str,
) -> Result<AgentAction, String> {
    if let Some(region) = arguments.get("region") {
        let region = region
            .as_str()
            .filter(|region| !region.is_empty())
            .ok_or_else(|| {
                "arcweft.action arguments.region must be a non-empty string".to_owned()
            })?;
        if region != observed_region {
            return Err(format!(
                "arcweft.action region `{region}` does not match observed action target `{observed_region}`"
            ));
        }
    }
    let action = AgentScrollAction {
        region: observed_region.to_owned(),
        delta_x_milli: agent_mcp_required_i32_argument(arguments, "delta_x_milli")?,
        delta_y_milli: agent_mcp_required_i32_argument(arguments, "delta_y_milli")?,
    };
    if action.delta_x_milli == 0 && action.delta_y_milli == 0 {
        return Err("arcweft.action scroll delta must not be zero on both axes".to_owned());
    }
    Ok(AgentAction::Scroll(action))
}

fn agent_mcp_required_i32_argument(
    arguments: &serde_json::Value,
    name: &str,
) -> Result<i32, String> {
    let value = arguments
        .get(name)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| format!("arcweft.action scroll requires integer arguments.{name}"))?;
    i32::try_from(value).map_err(|_| {
        format!("arcweft.action arguments.{name} must be in the i32 milli-pixel range")
    })
}

pub(super) fn agent_mcp_action_args(
    arguments: &serde_json::Value,
) -> Result<BTreeMap<String, AgentValue>, String> {
    let Some(args) = arguments.get("args") else {
        return Ok(BTreeMap::new());
    };
    let values = args
        .as_object()
        .ok_or_else(|| "arcweft.action arguments.args must be an object".to_owned())?;
    values
        .iter()
        .map(|(key, value)| Ok((key.clone(), agent_mcp_agent_value(value)?)))
        .collect()
}

pub(super) fn agent_mcp_agent_value(value: &serde_json::Value) -> Result<AgentValue, String> {
    match value {
        serde_json::Value::Null => Ok(AgentValue::Null),
        serde_json::Value::Bool(value) => Ok(AgentValue::Bool(*value)),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(AgentValue::I64)
            .or_else(|| value.as_u64().map(AgentValue::U64))
            .or_else(|| value.as_f64().map(AgentValue::F64))
            .ok_or_else(|| format!("arcweft.action arguments.args number is not finite: {value}")),
        serde_json::Value::String(value) => Ok(AgentValue::String(value.clone())),
        serde_json::Value::Array(values) => values
            .iter()
            .map(agent_mcp_agent_value)
            .collect::<Result<Vec<_>, _>>()
            .map(AgentValue::List),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), agent_mcp_agent_value(value)?)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(AgentValue::Map),
    }
}

pub(super) fn agent_mcp_action_from_target(
    arguments: &serde_json::Value,
    target: &AgentActionTarget,
) -> Result<AgentAction, String> {
    match target.action {
        AgentActionKind::AdvanceText => {
            agent_mcp_reject_action_args_for_non_invoke(arguments)?;
            Ok(AgentAction::AdvanceText)
        }
        AgentActionKind::SelectChoice => {
            agent_mcp_reject_action_args_for_non_invoke(arguments)?;
            Ok(AgentAction::SelectChoice {
                choice: agent_mcp_public_id_from_str(&target.target)?,
            })
        }
        AgentActionKind::Invoke => Ok(AgentAction::Invoke(Box::new(AgentInvokeAction {
            target: agent_mcp_public_id_from_str(&target.target)?,
            action: target.id.clone(),
            args: Box::new(agent_mcp_action_args(arguments)?),
        }))),
        AgentActionKind::Scroll => agent_mcp_scroll_action(arguments, &target.target),
        AgentActionKind::PointerClick => {
            Err("arcweft.action does not synthesize physical pointer_click actions".to_owned())
        }
    }
}

pub(super) fn agent_mcp_reject_action_args_for_non_invoke(
    arguments: &serde_json::Value,
) -> Result<(), String> {
    if arguments.get("args").is_some() {
        return Err("arcweft.action arguments.args is only valid for invoke actions".to_owned());
    }
    Ok(())
}

pub(super) fn agent_mcp_public_id_argument(
    arguments: &serde_json::Value,
    name: &str,
    tool: &str,
) -> Result<PublicId, String> {
    let value = arguments
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{tool} requires arguments.{name}"))?;
    agent_mcp_public_id_from_str(value)
}

pub(super) fn agent_mcp_public_id_from_str(value: &str) -> Result<PublicId, String> {
    PublicId::new(value.trim_start_matches('@').to_owned()).map_err(|error| error.to_string())
}

pub(super) fn agent_mcp_call_get_state(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.get_state")?;
    if let Some(error) =
        agent_mcp_observation_debug_read_privacy_error("state", PrivacyClass::Project, max_privacy)
    {
        return agent_mcp_json_tool_error(&error, "state privacy");
    }
    let report = state.report.as_ref().ok_or_else(|| {
        "arcweft.get_state requires a prior arcweft.observe call, arguments.source, or arguments.profile"
            .to_owned()
    })?;
    let summary = agent_mcp_observation_state_summary(report);
    let value = if let Some(path) = arguments.get("path").and_then(serde_json::Value::as_str) {
        let selected = agent_json_path(&summary, path).cloned();
        serde_json::json!({
            "path": path,
            "found": selected.is_some(),
            "value": selected,
            "tick": report.tick,
            "state_hash": report.state_hash,
        })
    } else {
        summary
    };
    agent_mcp_json_tool_result(&value, "state")
}

pub(super) fn agent_mcp_call_signal_get(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.signal_get")?;
    if let Some(error) =
        agent_mcp_observation_debug_read_privacy_error("signal", PrivacyClass::Project, max_privacy)
    {
        return agent_mcp_json_tool_error(&error, "signal privacy");
    }
    let name = arguments
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "arcweft.signal_get requires arguments.name".to_owned())?;
    let report = state.report.as_ref().ok_or_else(|| {
        "arcweft.signal_get requires a prior arcweft.observe call, arguments.source, or arguments.profile"
            .to_owned()
    })?;
    let signal = report.signals.iter().find(|signal| signal.name == name);
    let value = serde_json::json!({
        "name": name,
        "found": signal.is_some(),
        "value": signal.map(|signal| signal.value.as_str()),
        "tick": report.tick,
        "state_hash": report.state_hash,
    });
    agent_mcp_json_tool_result(&value, "signal")
}

pub(super) fn agent_mcp_call_log_query(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.log_query")?;
    if let Some(error) =
        agent_mcp_observation_debug_read_privacy_error("logs", PrivacyClass::Project, max_privacy)
    {
        return agent_mcp_json_tool_error(&error, "logs privacy");
    }
    let report = state.report.as_ref().ok_or_else(|| {
        "arcweft.log_query requires a prior arcweft.observe call, arguments.source, or arguments.profile"
            .to_owned()
    })?;
    let level = arguments.get("level").and_then(serde_json::Value::as_str);
    let contains = arguments
        .get("contains")
        .and_then(serde_json::Value::as_str);
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(50);
    let logs = report
        .logs
        .iter()
        .filter(|log| level.is_none_or(|level| log.level == level))
        .filter(|log| contains.is_none_or(|needle| log.message.contains(needle)))
        .take(limit)
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "tick": report.tick,
        "state_hash": report.state_hash,
        "level": level,
        "contains": contains,
        "limit": limit,
        "count": logs.len(),
        "logs": logs,
    });
    agent_mcp_json_tool_result(&value, "logs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_repl_command_returns_cli_local_protocol_json() {
        let (json, is_error) = call_repl_command_json(&serde_json::json!({
            "input": ":trace",
            "command_id": 9,
            "trace_policy": "read_write",
        }));

        assert!(is_error);
        assert_eq!(json["command_id"], 9);
        assert_eq!(json["status"], "error");
        assert_eq!(json["command"], ":trace");
        assert_eq!(json["evidence"]["kind"], "empty");
        assert_eq!(json["diagnostics"][0]["code"], "unhandled_extension");
        assert!(json.get("formatted_text").is_none());
    }

    #[test]
    fn mcp_repl_command_omits_cli_local_diagnostics_when_requested() {
        let (json, is_error) = call_repl_command_json(&serde_json::json!({
            "input": ":trace",
            "command_id": 10,
            "include_diagnostics": false,
        }));

        assert!(is_error);
        assert_eq!(json["status"], "error");
        assert_eq!(json["diagnostics"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn mcp_repl_command_rejects_cli_local_mutation_in_read_only_trace() {
        let (json, is_error) = call_repl_command_json(&serde_json::json!({
            "input": ":save out.awfagent",
            "command_id": 11,
            "trace_policy": "read_only_trace",
        }));

        assert!(is_error);
        assert_eq!(json["status"], "rejected");
        assert_eq!(json["command"], ":save");
        assert_eq!(json["diagnostics"][0]["code"], "read_only_trace_rejected");
    }

    fn call_repl_command_json(arguments: &serde_json::Value) -> (serde_json::Value, bool) {
        let mut state = AgentMcpState::default();
        let result = agent_mcp_call_repl_command(arguments, &mut state)
            .expect("MCP REPL command returns tool result");
        let McpContentBlock::Text { text } = &result.content[0] else {
            panic!("MCP REPL command must return JSON text content");
        };
        (
            serde_json::from_str(text).expect("MCP REPL command JSON parses"),
            result.is_error,
        )
    }
}
