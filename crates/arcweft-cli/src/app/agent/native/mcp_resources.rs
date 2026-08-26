use super::*;

pub(super) fn agent_mcp_observe_if_requested(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), String> {
    if !agent_mcp_arguments_request_observe(arguments) {
        return Ok(());
    }
    let observed = agent_mcp_run_observation(arguments, adapter_registrars)?;
    agent_mcp_store_observation(state, observed);
    Ok(())
}

pub(super) fn agent_mcp_json_tool_result(
    value: &serde_json::Value,
    context: &str,
) -> Result<McpCallToolResult, String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize Agent {context}: {error}"))?;
    Ok(McpCallToolResult {
        content: vec![McpContentBlock::Text { text }],
        is_error: false,
    })
}

pub(super) fn agent_mcp_json_tool_error(
    value: &serde_json::Value,
    context: &str,
) -> Result<McpCallToolResult, String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize Agent {context}: {error}"))?;
    Ok(McpCallToolResult {
        content: vec![McpContentBlock::Text { text }],
        is_error: true,
    })
}

pub(super) fn agent_mcp_observation_state_summary(
    report: &AgentObservationReport,
) -> serde_json::Value {
    serde_json::json!({
        "status": report.status,
        "final_status": report.final_status,
        "tick": report.tick,
        "frame_id": report.frame_id,
        "state_hash": report.state_hash,
        "render_hash": report.render_hash,
        "source": report.source,
        "steps": report.steps,
        "task_requests": report.task_requests,
        "capture_time_millis": report.capture_time_millis,
    })
}

pub(super) fn agent_mcp_wait_report_value(
    report: &AgentObservationReport,
    matched: bool,
    stable_seen_before: u32,
    polls: u64,
) -> serde_json::Value {
    serde_json::json!({
        "matched": matched,
        "stable_seen_before": stable_seen_before,
        "polls": polls,
        "tick": report.tick,
        "frame_id": report.frame_id,
        "state_hash": report.state_hash,
        "render_hash": report.render_hash,
        "final_status": report.final_status,
        "signals": report.signals,
        "metrics": report.metrics,
    })
}

pub(super) fn agent_mcp_predicate_matches(
    predicate: &Predicate,
    report: &AgentObservationReport,
) -> bool {
    match predicate {
        Predicate::Compare { probe, op, value } => agent_mcp_probe_value(probe, report)
            .is_some_and(|actual| agent_mcp_compare_values(&actual, *op, value)),
        Predicate::Exists { probe } => agent_mcp_probe_value(probe, report).is_some(),
        Predicate::ActionEnabled { target } => report
            .actions
            .iter()
            .any(|action| action.enabled && action.target == target.as_str()),
        Predicate::DiagnosticsHasError => report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == AgentDiagnosticSeverity::Error),
        Predicate::All { predicates } => predicates
            .iter()
            .all(|predicate| agent_mcp_predicate_matches(predicate, report)),
        Predicate::Any { predicates } => predicates
            .iter()
            .any(|predicate| agent_mcp_predicate_matches(predicate, report)),
        Predicate::Not { predicate } => !agent_mcp_predicate_matches(predicate, report),
    }
}

pub(super) fn agent_mcp_probe_value(
    probe: &Probe,
    report: &AgentObservationReport,
) -> Option<AgentValue> {
    match probe {
        Probe::Signal { target } => agent_mcp_assignment_value(&report.signals, target.as_str()),
        Probe::Metric { target } => agent_mcp_assignment_value(&report.metrics, target.as_str()),
        Probe::StatePath { path } => serde_json::to_value(report)
            .ok()
            .and_then(|value| agent_json_path(&value, path.as_str()).cloned())
            .and_then(|value| agent_mcp_agent_value(&value).ok()),
        Probe::ObservationField { path } if path.as_str() == "tick" => {
            u64::try_from(report.tick).ok().map(AgentValue::U64)
        }
        Probe::ObservationField { path } if path.as_str() == "frame_id" => {
            Some(AgentValue::String(report.frame_id.clone()))
        }
        Probe::ObservationField { path } if path.as_str() == "state_hash" => {
            Some(AgentValue::String(report.state_hash.clone()))
        }
        Probe::ObservationField { path } if path.as_str() == "render_hash" => {
            Some(AgentValue::String(report.render_hash.clone()))
        }
        Probe::ObservationField { path } => path
            .as_str()
            .strip_prefix("signals.")
            .and_then(|signal| agent_mcp_assignment_value(&report.signals, signal))
            .or_else(|| {
                path.as_str()
                    .strip_prefix("metrics.")
                    .and_then(|metric| agent_mcp_assignment_value(&report.metrics, metric))
            })
            .or_else(|| {
                serde_json::to_value(report)
                    .ok()
                    .and_then(|value| agent_json_path(&value, path.as_str()).cloned())
                    .and_then(|value| agent_mcp_agent_value(&value).ok())
            }),
    }
}

pub(super) fn agent_mcp_assignment_value(
    assignments: &[AgentAssignment],
    name: &str,
) -> Option<AgentValue> {
    assignments
        .iter()
        .find(|assignment| assignment.name.trim_start_matches('@') == name.trim_start_matches('@'))
        .map(agent_assignment_value)
}

pub(super) fn agent_mcp_compare_values(
    actual: &AgentValue,
    op: CompareOp,
    expected: &AgentValue,
) -> bool {
    match op {
        CompareOp::Eq => agent_mcp_values_equal(actual, expected),
        CompareOp::NotEq => !agent_mcp_values_equal(actual, expected),
        CompareOp::Greater => {
            agent_mcp_compare_numeric_values(actual, expected).is_some_and(i32::is_positive)
        }
        CompareOp::GreaterOrEqual => {
            agent_mcp_compare_numeric_values(actual, expected).is_some_and(|order| order >= 0)
        }
        CompareOp::Less => {
            agent_mcp_compare_numeric_values(actual, expected).is_some_and(i32::is_negative)
        }
        CompareOp::LessOrEqual => {
            agent_mcp_compare_numeric_values(actual, expected).is_some_and(|order| order <= 0)
        }
    }
}

pub(super) fn agent_mcp_values_equal(left: &AgentValue, right: &AgentValue) -> bool {
    match (left, right) {
        (AgentValue::Entity(left), AgentValue::String(right))
        | (AgentValue::String(right), AgentValue::Entity(left)) => left.as_str() == right,
        _ => left == right,
    }
}

pub(super) fn agent_mcp_compare_numeric_values(
    left: &AgentValue,
    right: &AgentValue,
) -> Option<i32> {
    Some(match (left, right) {
        (AgentValue::I64(left), AgentValue::I64(right)) => agent_mcp_compare_order(left.cmp(right)),
        (AgentValue::U64(left), AgentValue::U64(right)) => agent_mcp_compare_order(left.cmp(right)),
        (AgentValue::I64(left), AgentValue::U64(right)) => {
            if *left < 0 {
                -1
            } else {
                agent_mcp_compare_order(u64::try_from(*left).ok()?.cmp(right))
            }
        }
        (AgentValue::U64(left), AgentValue::I64(right)) => {
            if *right < 0 {
                1
            } else {
                agent_mcp_compare_order(left.cmp(&u64::try_from(*right).ok()?))
            }
        }
        (AgentValue::F64(left), AgentValue::F64(right)) => {
            agent_mcp_compare_order(left.partial_cmp(right)?)
        }
        _ => return None,
    })
}

pub(super) fn agent_mcp_compare_order(order: std::cmp::Ordering) -> i32 {
    match order {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

pub(super) fn agent_json_path<'a>(
    value: &'a serde_json::Value,
    path: &str,
) -> Option<&'a serde_json::Value> {
    if path.is_empty() {
        return Some(value);
    }
    path.split('.').try_fold(value, |current, segment| {
        current.as_object().and_then(|object| object.get(segment))
    })
}

pub(super) fn agent_mcp_call_trace_read(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
) -> Result<McpCallToolResult, String> {
    let path = arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "arcweft.trace.read requires arguments.path".to_owned())?;
    let records = super::read_and_validate_agent_trace_records(Path::new(path))?;
    let resource = trace_resource(&records)
        .map_err(|error| format!("failed to construct Agent trace resource: {error}"))?;
    state
        .trace_resources
        .retain(|cached| cached.uri != resource.uri);
    state.trace_resources.push(resource.clone());
    let published = agent_publish_resource_for_state(state, resource)?;
    tool_result_for_resource(&published)
        .map_err(|error| format!("failed to serialize MCP trace resource: {error}"))
}

pub(super) fn agent_mcp_arguments_request_observe(arguments: &serde_json::Value) -> bool {
    arguments.get("source").is_some() || arguments.get("profile").is_some()
}

pub(super) fn agent_mcp_run_observation(
    arguments: &serde_json::Value,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentMcpObservation, String> {
    let options = agent_mcp_observe_options(arguments)?;
    validate_agent_observe_options(&options).map_err(|_| "invalid observe options".to_owned())?;
    let mut runtime = native_player_runtime_state_for_options(&options, adapter_registrars)
        .map_err(|_| "failed to initialize player-backed MCP observe runtime".to_owned())?;
    let mut observed =
        observe_native_player_runtime(&mut runtime, &options, BundleStepInput::default())
            .map_err(|_| "failed to run player-backed MCP observe runtime".to_owned())?;
    let image_output =
        agent_observe_image_output(&mut observed.report, &options, &observed.image_frames)
            .map_err(|_| "failed to build MCP observe image output".to_owned())?;
    Ok(AgentMcpObservation {
        report: observed.report,
        image_output,
        image_frames: observed.image_frames,
        runtime,
        options,
    })
}

pub(super) fn agent_mcp_observe_runtime(
    runtime: &mut NativeAgentRuntimeState,
    options: &AgentObserveOptions,
    step_input: BundleStepInput,
    _adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentMcpFrame, String> {
    let mut observed = observe_native_player_runtime(runtime, options, step_input)
        .map_err(|_| "failed to run player-backed MCP observe runtime".to_owned())?;
    let image_output =
        agent_observe_image_output(&mut observed.report, options, &observed.image_frames)
            .map_err(|_| "failed to build MCP action image output".to_owned())?;
    let resources = agent_observe_list_resources(&observed.report, image_output.as_ref())
        .map_err(|_| "failed to build MCP action resources".to_owned())?;
    Ok(AgentMcpFrame {
        report: observed.report,
        image_output,
        image_frames: observed.image_frames,
        resources,
    })
}

pub(super) fn agent_mcp_call_resource_read(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
) -> Result<McpCallToolResult, String> {
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.resource.read")?;
    let uri = arguments
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "arcweft.resource.read requires arguments.uri".to_owned())?;
    let audit_path = agent_mcp_optional_debug_store_path(arguments);
    if let Some(published) = agent_mcp_cached_published_resource(state, uri) {
        if let Some(error) = agent_mcp_resource_read_privacy_error(
            published.resource(),
            max_privacy,
            "arcweft.resource.read",
        ) {
            agent_mcp_audit_resource_read(
                audit_path,
                uri,
                published.resource(),
                max_privacy,
                "blocked",
            )?;
            return agent_mcp_json_tool_error(&error, "resource privacy");
        }
        agent_mcp_audit_resource_read(
            audit_path,
            uri,
            published.resource(),
            max_privacy,
            "allowed",
        )?;
        return tool_result_for_resource(&published)
            .map_err(|error| format!("failed to serialize cached MCP resource: {error}"));
    }
    if let Some(resource) = agent_mcp_session_context_resource_for_uri(state, uri)
        .map_err(|error| format!("failed to build Agent session context: {error}"))?
    {
        if let Some(error) =
            agent_mcp_resource_read_privacy_error(&resource, max_privacy, "arcweft.resource.read")
        {
            agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "blocked")?;
            return agent_mcp_json_tool_error(&error, "resource privacy");
        }
        agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "allowed")?;
        let published = agent_publish_resource_for_state(state, resource)?;
        return tool_result_for_resource(&published)
            .map_err(|error| format!("failed to serialize MCP session context: {error}"));
    }
    if let Some(resource) = agent_mcp_cached_trace_resource(state, uri) {
        if let Some(error) =
            agent_mcp_resource_read_privacy_error(&resource, max_privacy, "arcweft.resource.read")
        {
            agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "blocked")?;
            return agent_mcp_json_tool_error(&error, "resource privacy");
        }
        agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "allowed")?;
        let published = agent_publish_resource_for_state(state, resource)?;
        return tool_result_for_resource(&published)
            .map_err(|error| format!("failed to serialize MCP trace resource: {error}"));
    }
    let Some(report) = state.report.clone() else {
        return Err("arcweft.resource.read requires a prior arcweft.observe call or arcweft.trace.read call".to_owned());
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
        agent_mcp_resource_read_privacy_error(&resource, max_privacy, "arcweft.resource.read")
    {
        agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "blocked")?;
        return agent_mcp_json_tool_error(&error, "resource privacy");
    }
    agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "allowed")?;
    let published = agent_publish_resource_for_state(state, resource)?;
    tool_result_for_resource(&published)
        .map_err(|error| format!("failed to serialize MCP tool resource: {error}"))
}

pub(super) fn agent_mcp_audit_resource_read(
    path: Option<&str>,
    requested_uri: &str,
    resource: &AgentResource,
    max_privacy: PrivacyClass,
    outcome: &str,
) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    let mut store = DebugStore::open(path).map_err(|error| {
        format!("arcweft.resource.read failed to open audit store `{path}`: {error}")
    })?;
    let session_id = SessionId::new("session.mcp.resource_read")
        .map_err(|error| format!("invalid MCP resource audit session id: {error}"))?;
    store
        .upsert_session(&DebugSession {
            session_id: session_id.clone(),
            program_hash: None,
            profile: "mcp".to_owned(),
            transport: "stdio".to_owned(),
            started_unix_ms: 0,
            ended_unix_ms: None,
            status: DebugSessionStatus::Running,
            metadata: BTreeMap::from([(
                "surface".to_owned(),
                serde_json::Value::String("arcweft.resource.read".to_owned()),
            )]),
        })
        .map_err(|error| {
            format!("arcweft.resource.read failed to upsert audit session in `{path}`: {error}")
        })?;
    let privacy = agent_mcp_resource_privacy(resource);
    let sequence = store.next_event_sequence(&session_id).map_err(|error| {
        format!("arcweft.resource.read failed to allocate audit sequence in `{path}`: {error}")
    })?;
    store
        .append(&DebugEvent {
            schema_version: 1,
            session_id,
            run_id: None,
            sequence,
            tick: None,
            kind: DebugEventKind::ResourceRead,
            payload: serde_json::json!({
                "surface": "arcweft.resource.read",
                "requested_uri": requested_uri,
                "resolved_uri": resource.uri,
                "kind": resource.kind,
                "mime_type": resource.mime_type,
                "hash": resource.hash,
                "privacy": privacy.as_str(),
                "max_privacy": max_privacy.as_str(),
                "outcome": outcome,
            }),
            created_unix_ms: 0,
        })
        .map_err(|error| {
            format!("arcweft.resource.read failed to write audit event to `{path}`: {error}")
        })?;
    Ok(())
}

pub(super) fn agent_mcp_resource_read_privacy_error(
    resource: &AgentResource,
    max_privacy: PrivacyClass,
    surface: &str,
) -> Option<serde_json::Value> {
    let privacy = agent_mcp_resource_privacy(resource);
    (!privacy.is_allowed_by(max_privacy)).then(|| {
        serde_json::json!({
            "status": "blocked",
            "error": format!(
                "{} resource {} is {} and exceeds max_privacy {}",
                surface,
                resource.uri,
                privacy.as_str(),
                max_privacy.as_str(),
            ),
            "resource": resource.uri,
            "privacy": privacy.as_str(),
            "max_privacy": max_privacy.as_str(),
        })
    })
}

pub(super) fn agent_mcp_resource_read_privacy_message(value: &serde_json::Value) -> String {
    value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| value.to_string(), str::to_owned)
}

pub(super) fn agent_mcp_resource_privacy(resource: &AgentResource) -> PrivacyClass {
    if resource.kind == AgentResourceKind::Image
        || resource.image.is_some()
        || matches!(resource.body, AgentResourceBody::BytesBase64(_))
    {
        return PrivacyClass::Sensitive;
    }
    match &resource.body {
        AgentResourceBody::Json(value) => agent_mcp_resource_json_privacy(value),
        AgentResourceBody::Text(_) => PrivacyClass::Project,
        AgentResourceBody::BytesBase64(_) => PrivacyClass::Sensitive,
    }
}

pub(super) fn agent_mcp_resource_json_privacy(value: &serde_json::Value) -> PrivacyClass {
    value.as_array().map_or_else(
        || agent_mcp_json_privacy(value),
        |items| {
            items
                .iter()
                .map(agent_mcp_json_privacy)
                .max()
                .unwrap_or(PrivacyClass::Project)
        },
    )
}

pub(super) fn agent_mcp_current_resources(
    state: &AgentMcpState,
) -> Result<Vec<AgentResource>, ExitCode> {
    let mut resources = if let Some(report) = &state.report {
        agent_observe_list_resources(report, state.image_output.as_ref())?
    } else {
        Vec::new()
    };
    if let Some(context) = agent_mcp_session_context_resource(state).map_err(|error| {
        eprintln!("error: failed to build Agent session context: {error}");
        ExitCode::FAILURE
    })? {
        resources.retain(|candidate| candidate.uri != context.uri);
        resources.insert(0, context);
    }
    for resource in state
        .capture_resources
        .iter()
        .chain(state.trace_resources.iter())
    {
        resources.retain(|candidate| candidate.uri != resource.uri);
        resources.push(resource.clone());
    }
    Ok(resources)
}

pub(super) fn agent_mcp_session_context_resource_for_uri(
    state: &AgentMcpState,
    uri: &str,
) -> Result<Option<AgentResource>, serde_json::Error> {
    agent_mcp_session_context_resource(state).map(|resource| {
        resource.filter(|resource| {
            resource.uri == uri
                || agent_uri_without_query(&resource.uri).is_some_and(|base| base == uri)
        })
    })
}

pub(super) fn agent_mcp_session_context_resource(
    state: &AgentMcpState,
) -> Result<Option<AgentResource>, serde_json::Error> {
    if state.report.is_none()
        && state.capture_resources.is_empty()
        && state.trace_resources.is_empty()
        && state.rag_context_packs.is_empty()
        && state.runtime.is_none()
    {
        return Ok(None);
    }
    let session_id = state
        .report
        .as_ref()
        .map_or("mcp", |report| report.session_id.as_str());
    let latest_observation = state.report.as_ref().map(|report| {
        serde_json::json!({
            "tick": report.tick,
            "frame_id": report.frame_id,
            "state_hash": report.state_hash,
            "render_hash": report.render_hash,
            "final_status": report.final_status,
            "capture_time_millis": report.capture_time_millis,
            "viewport": report.viewport,
            "counts": {
                "images": report.images.len(),
                "layers": report.layers.len(),
                "objects": report.objects.len(),
                "actions": report.actions.len(),
                "logs": report.logs.len(),
                "signals": report.signals.len(),
                "metrics": report.metrics.len(),
                "events": report.events.len(),
                "diagnostics": report.diagnostics.len(),
                "task_requests": report.task_requests,
            },
            "source_present": !report.source.is_empty(),
        })
    });
    let trace_resources = state
        .trace_resources
        .iter()
        .map(|resource| {
            serde_json::json!({
                "uri": resource.uri,
                "mime_type": resource.mime_type,
                "hash": resource.hash,
            })
        })
        .collect::<Vec<_>>();
    let rag_queries = state
        .rag_context_packs
        .iter()
        .map(|pack| {
            serde_json::json!({
                "query_id": pack.query.query_id,
                "text": pack.query.text,
                "item_count": pack.items.len(),
                "truncated": pack.truncated,
            })
        })
        .collect::<Vec<_>>();
    let body = serde_json::json!({
        "schema_version": 1,
        "kind": "session_context",
        "privacy_class": "project",
        "session_id": session_id,
        "observed": state.report.is_some(),
        "latest_observation": latest_observation,
        "resources": {
            "capture_resource_count": state.capture_resources.len(),
            "trace_resource_count": state.trace_resources.len(),
            "rag_query_count": state.rag_context_packs.len(),
            "cached_capture_uris": state.capture_resources.iter().map(|resource| &resource.uri).collect::<Vec<_>>(),
            "trace_resources": trace_resources,
            "rag_queries": rag_queries,
        },
        "runtime": {
            "native_runtime_active": state.runtime.is_some(),
            "shared_capture_session_active": state.runtime.is_some(),
        },
        "project": state.project_context.as_ref().map(AgentMcpProjectContext::to_json),
    });
    let bytes = serde_json::to_vec(&body)?;
    Ok(Some(AgentResource {
        uri: AgentResourceUri::new(format!("arcweft://session/{session_id}/context.json"))
            .expect("generated session context URI is nonempty"),
        kind: AgentResourceKind::SessionContext,
        mime_type: "application/json".to_owned(),
        hash: agent_mcp_content_hash(bytes),
        image: None,
        body: AgentResourceBody::Json(body),
    }))
}

pub(super) fn agent_mcp_cached_trace_resource(
    state: &AgentMcpState,
    uri: &str,
) -> Option<AgentResource> {
    state
        .trace_resources
        .iter()
        .rev()
        .find(|resource| resource.uri == uri)
        .cloned()
}

pub(super) fn agent_mcp_cached_capture_resource(
    state: &AgentMcpState,
    uri: &str,
) -> Option<AgentResource> {
    state
        .capture_resources
        .iter()
        .rev()
        .find(|resource| resource.uri == uri)
        .or_else(|| {
            if uri.contains('?') {
                return None;
            }
            state.capture_resources.iter().rev().find(|resource| {
                agent_uri_without_query(&resource.uri)
                    .is_some_and(|resource_uri| resource_uri == uri)
            })
        })
        .cloned()
}

pub(super) fn agent_mcp_latest_capture_resource(state: &AgentMcpState) -> Option<&AgentResource> {
    state.capture_resources.last()
}

pub(super) fn agent_mcp_uncached_resource_by_uri(
    report: &AgentObservationReport,
    uri: &str,
    state: &mut AgentMcpState,
) -> Result<AgentResource, ExitCode> {
    agent_observe_resource_by_uri_with_page_and_time_and_frame_store(
        report,
        uri,
        None,
        agent_report_capture_time_seconds(report),
        &state.image_frames,
    )
}

pub(super) fn agent_uri_without_query(uri: &str) -> Option<&str> {
    uri.split_once('?').map(|(base, _)| base)
}

pub(super) fn agent_mcp_call_capture(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    if arguments.get("source").is_some() || arguments.get("profile").is_some() {
        let observed = agent_mcp_run_observation(
            &agent_mcp_capture_observe_arguments(arguments),
            adapter_registrars,
        )?;
        agent_mcp_store_observation(state, observed);
    }
    let Some(report) = state.report.clone() else {
        return Err(
            "arcweft.capture requires a prior arcweft.observe call, arguments.source, or arguments.profile".to_owned(),
        );
    };
    let request = agent_mcp_capture_request(arguments, &report)?;
    let resource = agent_mcp_capture_resource(&report, &request, state)
        .map_err(|_| format!("failed to capture Agent image `{}`", request.uri))?;
    state
        .capture_resources
        .retain(|cached| cached.uri != resource.uri);
    state.capture_resources.push(resource.clone());
    let published = agent_publish_resource_for_state(state, resource)?;
    tool_result_for_resource(&published)
        .map_err(|error| format!("failed to serialize MCP capture resource: {error}"))
}

pub(super) fn agent_mcp_capture_resource(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    state: &mut AgentMcpState,
) -> Result<AgentResource, ExitCode> {
    agent_capture_resource(report, request, &state.image_frames)
}

pub(super) fn agent_mcp_capture_observe_arguments(
    arguments: &serde_json::Value,
) -> serde_json::Value {
    let mut observe_arguments = arguments.clone();
    if let Some(object) = observe_arguments.as_object_mut() {
        object.remove("format");
        object.remove("capture");
        object.remove("image");
        object.remove("uri");
        object.remove("page");
    }
    observe_arguments
}

pub(super) fn agent_mcp_capture_request(
    arguments: &serde_json::Value,
    report: &AgentObservationReport,
) -> Result<AgentCaptureReadRequest, String> {
    if let Some(uri) = arguments.get("uri").and_then(serde_json::Value::as_str) {
        for key in ["format", "capture", "view", "layer", "object"] {
            if arguments.get(key).is_some() {
                return Err(
                    "arcweft.capture accepts arguments.uri or format/capture/view/layer/object selectors, not both"
                        .to_owned(),
                );
            }
        }
        let mut request = agent_capture_request_from_uri(report, uri)
            .ok_or_else(|| format!("unsupported Agent image capture URI `{uri}`"))?;
        if arguments.get("renderer").is_some() {
            return Err("arcweft.capture no longer accepts arguments.renderer".to_owned());
        }
        if arguments.get("page").is_some() {
            request.page = agent_mcp_capture_page(arguments)?;
        }
        request.capture_time_seconds =
            agent_mcp_capture_time_argument(arguments, "arcweft.capture")?
                .unwrap_or(request.capture_time_seconds);
        return Ok(request);
    }
    let page = agent_mcp_capture_page(arguments)?;
    let capture_time_seconds =
        agent_mcp_capture_time_seconds(arguments, report, "arcweft.capture")?;
    let image_kind = arguments
        .get("format")
        .and_then(serde_json::Value::as_str)
        .map(agent_mcp_capture_image_kind)
        .transpose()?
        .unwrap_or(AgentObserveImageKind::Png);
    let capture_kind = arguments
        .get("capture")
        .and_then(serde_json::Value::as_str)
        .map(agent_mcp_capture_kind)
        .transpose()?
        .unwrap_or(AgentObserveCaptureKind::Color);
    if arguments.get("renderer").is_some() {
        return Err("arcweft.capture no longer accepts arguments.renderer".to_owned());
    }
    let layer = arguments
        .get("layer")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    let view = arguments
        .get("view")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    let object = arguments
        .get("object")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    if [layer.is_some(), view.is_some(), object.is_some()]
        .into_iter()
        .filter(|selected| *selected)
        .count()
        > 1
    {
        return Err(
            "arcweft.capture accepts one of arguments.view, arguments.layer, or arguments.object"
                .to_owned(),
        );
    }
    let extension = match image_kind {
        AgentObserveImageKind::Png => "png",
        AgentObserveImageKind::RawRgba => "rgba",
        AgentObserveImageKind::Overlay => {
            return Err("arcweft.capture supports format png or raw-rgba".to_owned());
        }
    };
    let (scope, name) = if let Some(view) = view {
        let name = agent_scoped_capture_name("view", &view, capture_kind.resource_name());
        (AgentCaptureScope::View(view), name)
    } else if let Some(object) = object {
        let name = agent_scoped_capture_name("object", &object, capture_kind.resource_name());
        (AgentCaptureScope::Object(object), name)
    } else if let Some(layer) = layer {
        let name = agent_scoped_capture_name("layer", &layer, capture_kind.resource_name());
        (AgentCaptureScope::Layer(layer), name)
    } else {
        (
            AgentCaptureScope::Viewport,
            capture_kind.resource_name().to_owned(),
        )
    };
    let uri =
        agent_frame_capture_uri_for_page(&report.session_id, report.tick, &name, extension, page);
    Ok(AgentCaptureReadRequest {
        uri,
        image_kind,
        capture_kind,
        scope,
        page,
        capture_step: report.steps,
        capture_time_seconds,
    })
}

pub(super) fn agent_mcp_capture_page(arguments: &serde_json::Value) -> Result<usize, String> {
    agent_mcp_page_argument(arguments, "arcweft.capture")
}

pub(super) fn agent_mcp_capture_time_seconds(
    arguments: &serde_json::Value,
    report: &AgentObservationReport,
    tool: &str,
) -> Result<f32, String> {
    Ok(
        agent_mcp_capture_time_argument(arguments, tool)?.unwrap_or_else(|| {
            agent_mcp_usize_argument(arguments, "capture_step").map_or_else(
                || agent_report_capture_time_seconds(report),
                agent_capture_time_seconds_from_step,
            )
        }),
    )
}

pub(super) fn agent_mcp_page_argument(
    arguments: &serde_json::Value,
    tool: &str,
) -> Result<usize, String> {
    let Some(value) = arguments.get("page") else {
        return Ok(0);
    };
    let page = value
        .as_u64()
        .ok_or_else(|| format!("{tool} argument page must be a non-negative integer"))?;
    usize::try_from(page)
        .map_err(|_| format!("{tool} argument page is too large for this platform"))
}

pub(super) fn agent_mcp_capture_time_argument(
    arguments: &serde_json::Value,
    tool: &str,
) -> Result<Option<f32>, String> {
    let Some(value) = arguments.get("capture_time") else {
        return Ok(None);
    };
    let seconds = serde_json::from_value::<f32>(value.clone())
        .map_err(|_| format!("{tool} argument capture_time must be a number of seconds"))?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(format!(
            "{tool} argument capture_time must be a finite non-negative number of seconds"
        ));
    }
    Ok(Some(seconds))
}

pub(super) fn agent_mcp_capture_image_kind(value: &str) -> Result<AgentObserveImageKind, String> {
    match value {
        "png" => Ok(AgentObserveImageKind::Png),
        "raw-rgba" => Ok(AgentObserveImageKind::RawRgba),
        _ => Err(format!("unsupported capture format `{value}`")),
    }
}

pub(super) fn agent_mcp_observe_options(
    arguments: &serde_json::Value,
) -> Result<AgentObserveOptions, String> {
    if arguments.get("flow").is_some() {
        return Err(
            "arcweft.observe does not accept arguments.flow; select an exact entry.* ID".to_owned(),
        );
    }
    let source = arguments.get("source").and_then(serde_json::Value::as_str);
    let profile = arguments
        .get("profile")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    if source.is_some() && profile.is_some() {
        return Err(
            "arcweft.observe arguments.source and arguments.profile are mutually exclusive"
                .to_owned(),
        );
    }
    if source.is_none() && profile.is_none() {
        return Err("arcweft.observe requires arguments.source or arguments.profile".to_owned());
    }
    if source.is_some() && arguments.get("entry").is_none() {
        return Err("arcweft.observe with arguments.source requires arguments.entry".to_owned());
    }
    if arguments.get("renderer").is_some() {
        return Err("arcweft.observe no longer accepts arguments.renderer".to_owned());
    }
    Ok(AgentObserveOptions {
        path: source.map(PathBuf::from),
        profile: ProfileOptions {
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
        executor: CliRuntimeExecutorTier::BytecodeVm,
        pure_backend: None,
        pure_workers: None,
        pure_batch_min_len: None,
        pure_object_artifacts: false,
        math_backend: None,
        math_wgpu_min_elements: None,
        steps: agent_mcp_usize_argument(arguments, "steps").unwrap_or(8),
        capture_step: agent_mcp_usize_argument(arguments, "capture_step"),
        mode: CliRuntimeStepMode::Drain,
        max_ops: agent_mcp_usize_argument(arguments, "max_ops").unwrap_or(64),
        view_values: Vec::new(),
        viewport_width: agent_mcp_u32_argument(arguments, "viewport_width", "arcweft.observe")?
            .unwrap_or(AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH),
        viewport_height: agent_mcp_u32_argument(arguments, "viewport_height", "arcweft.observe")?
            .unwrap_or(AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT),
        image: arguments
            .get("image")
            .and_then(serde_json::Value::as_str)
            .map(agent_mcp_image_kind)
            .transpose()?,
        capture: arguments
            .get("capture")
            .and_then(serde_json::Value::as_str)
            .map(agent_mcp_capture_kind)
            .transpose()?,
        layer: arguments
            .get("layer")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        view: arguments
            .get("view")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        object: arguments
            .get("object")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        page: arguments
            .get("page")
            .map(|_| agent_mcp_page_argument(arguments, "arcweft.observe"))
            .transpose()?,
        capture_time_seconds: agent_mcp_capture_time_argument(arguments, "arcweft.observe")?,
        resource: None,
        read_uri: None,
        mcp: false,
        mcp_format: AgentObserveMcpFormat::Read,
        content_policy_mode: AgentContentPolicyMode::Strict,
        out: None,
        json: false,
    })
}

pub(super) fn agent_mcp_usize_argument(arguments: &serde_json::Value, name: &str) -> Option<usize> {
    arguments
        .get(name)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

pub(super) fn agent_mcp_u64_argument(
    arguments: &serde_json::Value,
    name: &str,
    tool: &str,
) -> Result<Option<u64>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    value
        .as_u64()
        .ok_or_else(|| format!("{tool} argument {name} must be a positive integer"))
        .map(Some)
}

pub(super) fn agent_mcp_u32_argument(
    arguments: &serde_json::Value,
    name: &str,
    tool: &str,
) -> Result<Option<u32>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    let value = value
        .as_u64()
        .ok_or_else(|| format!("{tool} argument {name} must be a positive integer"))?;
    u32::try_from(value)
        .map(Some)
        .map_err(|_| format!("{tool} argument {name} is too large"))
}

pub(super) fn agent_mcp_image_kind(value: &str) -> Result<AgentObserveImageKind, String> {
    match value {
        "overlay" => Ok(AgentObserveImageKind::Overlay),
        "png" => Ok(AgentObserveImageKind::Png),
        "raw-rgba" => Ok(AgentObserveImageKind::RawRgba),
        _ => Err(format!("unsupported image kind `{value}`")),
    }
}

pub(super) fn agent_mcp_capture_kind(value: &str) -> Result<AgentObserveCaptureKind, String> {
    match value {
        "color" => Ok(AgentObserveCaptureKind::Color),
        "object-id" => Ok(AgentObserveCaptureKind::ObjectId),
        "mask" => Ok(AgentObserveCaptureKind::Mask),
        _ => Err(format!("unsupported capture kind `{value}`")),
    }
}

pub(super) fn agent_mcp_success_response(
    id: Option<&serde_json::Value>,
    result: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

pub(super) fn agent_mcp_error_response(
    id: Option<&serde_json::Value>,
    code: i64,
    message: &str,
) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}
