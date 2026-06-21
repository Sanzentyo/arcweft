
use super::*;

pub(super) fn agent_observe_command(
    options: &AgentObserveOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    validate_agent_observe_options(options)?;
    let mut observed = agent_observation_for_options(options, adapter_registrars)?;
    let image_output = agent_observe_image_output(
        &mut observed.report,
        options,
        Some(&mut observed.native_session),
        &observed.image_frames,
    )?;
    if let Some(uri) = &options.read_uri {
        let resource = agent_observe_cached_image_resource(
            &observed.report,
            image_output.as_ref(),
            uri,
        )
        .map_or_else(
            || {
                agent_observe_resource_by_uri_with_page_and_time_and_session_and_frame_store(
                    &observed.report,
                    uri,
                    options.page,
                    agent_observe_capture_time_seconds(options),
                    Some(&mut observed.native_session),
                    &observed.image_frames,
                )
            },
            Ok,
        )?;
        if options.mcp {
            let resource = agent_observe_mcp_resource_output(
                AgentObserveResourceOutput::One(Box::new(resource)),
                options.mcp_format,
            )?;
            return print_json(&resource);
        }
        return print_json(&resource);
    }
    if let Some(out) = &options.out {
        let Some(image_output) = &image_output else {
            eprintln!("error: --out requires --image");
            return Err(ExitCode::from(2));
        };
        fs::write(out, &image_output.bytes).map_err(|error| {
            eprintln!("error: failed to write {}: {error}", out.display());
            ExitCode::FAILURE
        })?;
    }
    if let Some(resource) = options.resource {
        let resource = agent_observe_resource(&observed.report, image_output.as_ref(), resource)?;
        if options.mcp {
            let resource = agent_observe_mcp_resource_output(resource, options.mcp_format)?;
            print_json(&resource)
        } else {
            print_json(&resource)
        }
    } else if options.json {
        print_json(&observed.report)
    } else {
        println!(
            "ok: {} ({} object(s), {} diagnostic(s), render_hash={})",
            observed.report.source,
            observed.report.objects.len(),
            observed.report.diagnostics.len(),
            observed.report.render_hash
        );
        Ok(())
    }
}

pub(super) fn agent_observation_report_for_options(
    options: &AgentObserveOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentObservationReport, ExitCode> {
    agent_observation_for_options(options, adapter_registrars).map(|observed| observed.report)
}

pub(super) fn native_agent_runtime_state_for_options(
    options: &AgentObserveOptions,
) -> Result<NativeAgentRuntimeState, ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    )?;
    let checked = load_and_check_selection(&selection, None)?;
    let project_context = agent_mcp_project_context_from_hir(&checked.hir, selection.path())
        .map_err(|error| {
            eprintln!("error: failed to build native project context: {error}");
            ExitCode::FAILURE
        })?;
    let native_session = agent_native_capture_session_for_hir(&checked.hir)?;
    let host_policy = native_host_policy_for_selection(&selection)?;
    let runtime_options = runtime_plan_options_for_selection(&selection);
    let lowered = lower_source_runtime_plan_with_stats_and_options(&checked.hir, &runtime_options)
        .map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })?;
    let mut plan = lowered.plan;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut plan, entry, options.flow.as_deref())?;
    Ok(NativeAgentRuntimeState {
        executor: RuntimeExecutorInstance::new(plan, options.executor, pure_config),
        catalog: lowered.line_display_catalog,
        source_path: selection.path().to_owned(),
        host_policy,
        project_context,
        native_session,
        task_events: Vec::new(),
        next_tick: 0,
    })
}

pub(super) fn agent_observation_for_options(
    options: &AgentObserveOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentObservationState, ExitCode> {
    let mut runtime = native_agent_runtime_state_for_options(options)?;
    let mut observed = run_agent_observation(
        &mut runtime.executor,
        &runtime.catalog,
        AgentObservationRunContext {
            host_config: NativeRunHost {
                source_path: Some(&runtime.source_path),
                policy: &runtime.host_policy,
                adapter_registrars,
            },
            options,
            source_path: &runtime.source_path,
            native_session: Some(&mut runtime.native_session),
            tick_offset: 0,
            input_events: Vec::new(),
            task_events: &mut runtime.task_events,
        },
    )
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })?;
    extend_agent_observation_with_runtime_images(
        &mut observed,
        &runtime.source_path,
        &runtime.executor,
        options,
    );
    Ok(AgentObservationState {
        report: observed.report,
        image_frames: observed.image_frames,
        native_session: runtime.native_session,
    })
}

pub(in crate::app::agent) fn agent_script_run_native_bundle(
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentScriptRunReport, ExitCode> {
    let session = NativeAgentScriptSession::new(
        options,
        adapter_registrars,
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
    let blob_result = super::write_agent_capture_blobs(
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

#[derive(Debug, Error)]
pub(super) enum NativeAgentScriptSessionError {
    #[error("native Agent Script observation failed")]
    Observe,
    #[error("native Agent Script capture failed")]
    Capture,
    #[error("native Agent Script resource read failed")]
    ResourceRead,
    #[error("native Agent Script action is not currently selectable")]
    ActionUnavailable,
    #[error("native Agent Script action kind is not supported by the native semantic dispatcher")]
    UnsupportedAction,
}

pub(super) struct NativeAgentScriptSession<'a> {
    pub(super) options: AgentObserveOptions,
    pub(super) adapter_registrars: &'a [NativeAdapterRegistrar],
    pub(super) program_hash: String,
    pub(super) project_entities: Vec<RequiredEntity>,
    pub(super) project_graph: arcweft_agent_protocol::protocol::AgentProjectGraph,
    pub(super) runtime: Option<NativeAgentRuntimeState>,
    pub(super) observed: Option<NativeAgentObservedSnapshot>,
    pub(super) capture_blobs: Vec<AgentCaptureBlob>,
}

impl<'a> NativeAgentScriptSession<'a> {
    fn new(
        options: &AgentScriptRunOptions,
        adapter_registrars: &'a [NativeAdapterRegistrar],
        program_hash: String,
        project_entities: Vec<RequiredEntity>,
        project_graph: arcweft_agent_protocol::protocol::AgentProjectGraph,
    ) -> Self {
        Self {
            options: AgentObserveOptions {
                path: options.native_source.clone(),
                profile: options.native_profile.clone(),
                entry: options.entry.clone(),
                flow: options.flow.clone(),
                executor: options.executor,
                pure_backend: options.pure_backend,
                pure_workers: options.pure_workers,
                pure_batch_min_len: options.pure_batch_min_len,
                pure_object_artifacts: options.pure_object_artifacts,
                math_backend: options.math_backend,
                math_wgpu_min_elements: options.math_wgpu_min_elements,
                steps: options.native_steps,
                capture_step: None,
                mode: options.native_mode,
                max_ops: options.native_max_ops,
                values: options.values.clone(),
                viewport_width: options.viewport_width,
                viewport_height: options.viewport_height,
                textbox_height: options.textbox_height,
                image: None,
                capture: None,
                layer: None,
                object: None,
                page: None,
                capture_time_seconds: options.capture_time_seconds,
                resource: None,
                read_uri: None,
                mcp: false,
                mcp_format: AgentObserveMcpFormat::Read,
                out: None,
                json: true,
            },
            adapter_registrars,
            program_hash,
            project_entities,
            project_graph,
            runtime: None,
            observed: None,
            capture_blobs: Vec::new(),
        }
    }

    fn capture_blobs(&self) -> &[AgentCaptureBlob] {
        &self.capture_blobs
    }

    fn observe_report(&mut self) -> Result<&AgentObservationReport, NativeAgentScriptSessionError> {
        if self.observed.is_none() {
            self.refresh_observation(Vec::new())?;
        }
        self.observed
            .as_ref()
            .map(|observed| &observed.report)
            .ok_or(NativeAgentScriptSessionError::Observe)
    }

    fn runtime_state(
        &mut self,
    ) -> Result<&mut NativeAgentRuntimeState, NativeAgentScriptSessionError> {
        if self.runtime.is_none() {
            let runtime = native_agent_runtime_state_for_options(&self.options)
                .map_err(|_| NativeAgentScriptSessionError::Observe)?;
            self.runtime = Some(runtime);
        }
        self.runtime
            .as_mut()
            .ok_or(NativeAgentScriptSessionError::Observe)
    }

    fn refresh_observation(
        &mut self,
        input_events: Vec<RoutedInputEvent>,
    ) -> Result<&AgentObservationReport, NativeAgentScriptSessionError> {
        let options = self.options.clone();
        let adapter_registrars = self.adapter_registrars;
        let runtime = self.runtime_state()?;
        let tick_offset = runtime.next_tick;
        let mut observed = run_agent_observation(
            &mut runtime.executor,
            &runtime.catalog,
            AgentObservationRunContext {
                host_config: NativeRunHost {
                    source_path: Some(&runtime.source_path),
                    policy: &runtime.host_policy,
                    adapter_registrars,
                },
                options: &options,
                source_path: &runtime.source_path,
                native_session: Some(&mut runtime.native_session),
                tick_offset,
                input_events,
                task_events: &mut runtime.task_events,
            },
        )
        .map_err(|_| NativeAgentScriptSessionError::Observe)?;
        extend_agent_observation_with_runtime_images(
            &mut observed,
            &runtime.source_path,
            &runtime.executor,
            &options,
        );
        runtime.next_tick = observed.report.tick.saturating_add(1);
        self.observed = Some(NativeAgentObservedSnapshot {
            report: observed.report,
            image_frames: observed.image_frames,
        });
        self.observe_report()
    }

    fn resource_for_uri(
        &mut self,
        uri: &str,
    ) -> Result<AgentResource, NativeAgentScriptSessionError> {
        if self.observed.is_none() {
            self.refresh_observation(Vec::new())?;
        }
        let Some(mut runtime) = self.runtime.take() else {
            return Err(NativeAgentScriptSessionError::ResourceRead);
        };
        let result = {
            let Some(observed) = self.observed.as_ref() else {
                self.runtime = Some(runtime);
                return Err(NativeAgentScriptSessionError::ResourceRead);
            };
            agent_observe_resource_by_uri_with_page_and_time_and_session_and_frame_store(
                &observed.report,
                uri,
                None,
                agent_report_capture_time_seconds(&observed.report),
                Some(&mut runtime.native_session),
                &observed.image_frames,
            )
            .map_err(|_| NativeAgentScriptSessionError::ResourceRead)
        };
        self.runtime = Some(runtime);
        result
    }

    fn action_input_events(
        &mut self,
        action: AgentAction,
    ) -> Result<Vec<RoutedInputEvent>, NativeAgentScriptSessionError> {
        let report = self.observe_report()?;
        native_agent_action_input_events(report, action)
    }

    pub(super) fn action_result(
        before: &AgentObservationReport,
        after: &AgentObservationReport,
    ) -> ActionResult {
        ActionResult {
            accepted: before.state_hash != after.state_hash || before.tick != after.tick,
            before_tick: u64::try_from(before.tick).unwrap_or(u64::MAX),
            after_tick: u64::try_from(after.tick).unwrap_or(u64::MAX),
            before_state_hash: before.state_hash.clone(),
            after_state_hash: after.state_hash.clone(),
        }
    }
}

pub(super) fn native_agent_report_has_action(
    report: &AgentObservationReport,
    kind: AgentActionKind,
    target: &str,
) -> bool {
    report.actions.iter().any(|candidate| {
        candidate.enabled
            && candidate.kind == AgentActionDispatch::Semantic
            && candidate.action == kind
            && candidate.target == target
    })
}

pub(super) fn native_agent_action_input_events(
    report: &AgentObservationReport,
    action: AgentAction,
) -> Result<Vec<RoutedInputEvent>, NativeAgentScriptSessionError> {
    match action {
        AgentAction::SelectChoice { choice } => {
            native_agent_select_choice_input_events(report, &choice)
        }
        AgentAction::AdvanceText => native_agent_advance_text_input_events(report),
        AgentAction::Invoke(invoke) => {
            native_agent_invoke_input_events(report, &invoke.target, &invoke.action)
        }
        AgentAction::PointerClick { .. } => Err(NativeAgentScriptSessionError::UnsupportedAction),
    }
}

pub(super) fn native_agent_select_choice_input_events(
    report: &AgentObservationReport,
    choice: &PublicId,
) -> Result<Vec<RoutedInputEvent>, NativeAgentScriptSessionError> {
    let choice_id = choice.as_str();
    native_agent_report_has_action(report, AgentActionKind::SelectChoice, choice_id)
        .then(|| vec![native_runtime_input_event("choice", Some(choice_id))])
        .ok_or(NativeAgentScriptSessionError::ActionUnavailable)
}

pub(super) fn native_agent_advance_text_input_events(
    report: &AgentObservationReport,
) -> Result<Vec<RoutedInputEvent>, NativeAgentScriptSessionError> {
    report
        .actions
        .iter()
        .any(|candidate| {
            candidate.enabled
                && candidate.kind == AgentActionDispatch::Semantic
                && candidate.action == AgentActionKind::AdvanceText
        })
        .then(|| vec![native_runtime_input_event("advance", None)])
        .ok_or(NativeAgentScriptSessionError::ActionUnavailable)
}

pub(super) fn native_agent_invoke_input_events(
    report: &AgentObservationReport,
    target: &PublicId,
    action: &str,
) -> Result<Vec<RoutedInputEvent>, NativeAgentScriptSessionError> {
    let target = target.as_str();
    report
        .actions
        .iter()
        .any(|candidate| {
            candidate.enabled
                && candidate.kind == AgentActionDispatch::Semantic
                && candidate.action == AgentActionKind::Invoke
                && candidate.target == target
                && candidate.id == action
        })
        .then(|| vec![native_runtime_input_event("invoke", Some(action))])
        .ok_or(NativeAgentScriptSessionError::ActionUnavailable)
}

pub(super) fn native_runtime_input_event(kind: &str, payload: Option<&str>) -> RoutedInputEvent {
    let mut event = RoutedInputEvent::new(
        InputEpoch::default(),
        InputSequence::default(),
        InteractionTarget::new("runtime").expect("runtime target"),
        InputEventKind::Custom {
            name: Identifier::new(kind).expect("runtime input kind"),
        },
    );
    if let Some(payload) = payload {
        event = event.with_payload(InteractionPayload::Text(payload.to_owned()));
    }
    event
}

impl AgentSession for NativeAgentScriptSession<'_> {
    type Error = NativeAgentScriptSessionError;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(AgentSessionInfo {
            session_id: "session.native".to_owned(),
            program_hash: self.program_hash.clone(),
            project_entities: self.project_entities.clone(),
            project_graph: self.project_graph.clone(),
            profile: self.options.profile.profile.clone(),
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
        if self.observed.is_some()
            && self.runtime.as_ref().is_some_and(|runtime| {
                matches!(
                    runtime.executor.fiber().status,
                    FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
                )
            })
        {
            return self.observe_report().map(native_agent_observation_envelope);
        }
        let report = self.refresh_observation(Vec::new())?;
        Ok(native_agent_observation_envelope(report))
    }

    fn act(&mut self, action: AgentAction) -> Result<ActionResult, Self::Error> {
        let before = self.observe_report()?.clone();
        let input_events = self.action_input_events(action)?;
        let after = self.refresh_observation(input_events)?.clone();
        Ok(Self::action_result(&before, &after))
    }

    fn capture(&mut self, request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        let report = self.observe_report()?;
        let uri = native_agent_capture_uri(report, &request)?;
        let resource = self.resource_for_uri(&uri)?;
        let blob = agent_capture_blob_from_resource(&resource)
            .map_err(|_| NativeAgentScriptSessionError::Capture)?;
        let byte_len = u64::try_from(blob.bytes.len()).unwrap_or(u64::MAX);
        let result = CaptureResult {
            uri: AgentResourceUri::new(resource.uri)
                .map_err(|_| NativeAgentScriptSessionError::Capture)?,
            content_hash: blob.content_hash.clone(),
            media_type: resource.mime_type,
            byte_len,
        };
        self.capture_blobs.push(blob);
        Ok(result)
    }

    fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error> {
        self.resource_for_uri(uri)
    }

    fn step_frames(&mut self, count: u32) -> Result<ObservationEnvelope, Self::Error> {
        let additional = usize::try_from(count.max(1)).unwrap_or(usize::MAX);
        self.options.steps = self.options.steps.saturating_add(additional);
        let report = self.refresh_observation(Vec::new())?;
        Ok(native_agent_observation_envelope(report))
    }
}

pub(super) fn native_agent_observation_envelope(
    report: &AgentObservationReport,
) -> ObservationEnvelope {
    ObservationEnvelope {
        tick: u64::try_from(report.tick).unwrap_or(u64::MAX),
        frame_id: report.frame_id.clone(),
        state_hash: report.state_hash.clone(),
        render_hash: report.render_hash.clone(),
        actions: report.actions.clone(),
        signals: report
            .signals
            .iter()
            .map(|signal| {
                (
                    signal.name.trim_start_matches('@').to_owned(),
                    agent_assignment_value(signal),
                )
            })
            .collect(),
        payload: serde_json::to_value(report).unwrap_or(serde_json::Value::Null),
    }
}

pub(super) fn native_agent_capture_uri(
    report: &AgentObservationReport,
    request: &CaptureRequest,
) -> Result<String, NativeAgentScriptSessionError> {
    let image_kind = native_agent_capture_image_kind(request.format);
    let capture_kind = native_agent_capture_kind(&request.capture_kind);
    let extension = match image_kind {
        AgentObserveImageKind::Png => "png",
        AgentObserveImageKind::RawRgba => "rgba",
        AgentObserveImageKind::Overlay => return Err(NativeAgentScriptSessionError::Capture),
    };
    let name = match &request.target {
        CaptureTarget::Viewport => capture_kind.resource_name().to_owned(),
        CaptureTarget::Layer { id } => {
            agent_scoped_capture_name("layer", id.as_str(), capture_kind.resource_name())
        }
        CaptureTarget::Object { id } => {
            agent_scoped_capture_name("object", id, capture_kind.resource_name())
        }
    };
    Ok(agent_frame_capture_uri_for_page(
        &report.session_id,
        report.tick,
        &name,
        extension,
        0,
    ))
}

pub(super) fn native_agent_capture_image_kind(format: CaptureFormat) -> AgentObserveImageKind {
    match format {
        CaptureFormat::Png => AgentObserveImageKind::Png,
        CaptureFormat::RawRgba => AgentObserveImageKind::RawRgba,
    }
}

pub(super) fn native_agent_capture_kind(value: &str) -> AgentObserveCaptureKind {
    match value {
        "object-id" | "object_id" => AgentObserveCaptureKind::ObjectId,
        "mask" => AgentObserveCaptureKind::Mask,
        _ => AgentObserveCaptureKind::Color,
    }
}

pub(super) fn agent_assignment_value(
    signal: &AgentAssignment,
) -> arcweft_agent_protocol::value::AgentValue {
    match signal.value.as_str() {
        "true" => arcweft_agent_protocol::value::AgentValue::Bool(true),
        "false" => arcweft_agent_protocol::value::AgentValue::Bool(false),
        value if value.starts_with('@') => {
            arcweft_agent_protocol::ids::PublicId::new(value.trim_start_matches('@')).map_or_else(
                |_| arcweft_agent_protocol::value::AgentValue::String(value.to_owned()),
                arcweft_agent_protocol::value::AgentValue::Entity,
            )
        }
        value => value.parse::<i64>().map_or_else(
            |_| arcweft_agent_protocol::value::AgentValue::String(value.to_owned()),
            arcweft_agent_protocol::value::AgentValue::I64,
        ),
    }
}

pub(super) fn agent_capture_blob_from_resource(
    resource: &AgentResource,
) -> Result<AgentCaptureBlob, NativeAgentScriptSessionError> {
    let bytes = match &resource.body {
        AgentResourceBody::BytesBase64(body) => match body.encoding {
            AgentBinaryEncoding::Base64 => STANDARD
                .decode(&body.data)
                .map_err(|_| NativeAgentScriptSessionError::Capture)?,
        },
        AgentResourceBody::Text(text) if resource.mime_type == "image/svg+xml" => {
            text.as_bytes().to_vec()
        }
        AgentResourceBody::Json(_) | AgentResourceBody::Text(_) => {
            return Err(NativeAgentScriptSessionError::Capture);
        }
    };
    Ok(AgentCaptureBlob {
        content_hash: format!("blake3:{}", blake3::hash(&bytes).to_hex()),
        bytes,
    })
}

pub(super) fn agent_hit_test_command(
    options: &AgentHitTestOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    validate_agent_hit_test_options(options)?;
    let observe_options = agent_hit_test_observe_options(options);
    let report = agent_observation_report_for_options(&observe_options, adapter_registrars)?;
    let hit_test = agent_hit_test_report(&report, options.x, options.y);
    if options.json {
        print_json(&hit_test)
    } else if let Some(top) = &hit_test.top_object_id {
        println!(
            "ok: hit {} at {},{} ({} candidate(s))",
            top,
            options.x,
            options.y,
            hit_test.hits.len()
        );
        Ok(())
    } else {
        println!("ok: no hit at {},{}", options.x, options.y);
        Ok(())
    }
}

pub(super) fn validate_agent_hit_test_options(
    options: &AgentHitTestOptions,
) -> Result<(), ExitCode> {
    let observe_options = agent_hit_test_observe_options(options);
    validate_agent_observe_options(&observe_options)
}

pub(super) fn agent_hit_test_observe_options(options: &AgentHitTestOptions) -> AgentObserveOptions {
    AgentObserveOptions {
        path: options.path.clone(),
        profile: options.profile.clone(),
        entry: options.entry.clone(),
        flow: options.flow.clone(),
        executor: options.executor,
        pure_backend: options.pure_backend,
        pure_workers: options.pure_workers,
        pure_batch_min_len: options.pure_batch_min_len,
        pure_object_artifacts: options.pure_object_artifacts,
        math_backend: options.math_backend,
        math_wgpu_min_elements: options.math_wgpu_min_elements,
        steps: options.steps,
        capture_step: options.capture_step,
        mode: options.mode,
        max_ops: options.max_ops,
        values: options.values.clone(),
        viewport_width: options.viewport_width,
        viewport_height: options.viewport_height,
        textbox_height: options.textbox_height,
        image: None,
        capture: None,
        layer: None,
        object: None,
        page: None,
        capture_time_seconds: options.capture_time_seconds,
        resource: None,
        read_uri: None,
        mcp: false,
        mcp_format: AgentObserveMcpFormat::Read,
        out: None,
        json: true,
    }
}

pub(super) fn agent_hit_test_report(
    report: &AgentObservationReport,
    x: u32,
    y: u32,
) -> AgentHitTestReport {
    let mut hits = report
        .objects
        .iter()
        .filter(|object| object.visible)
        .flat_map(|object| agent_hit_test_object_hits(object, x, y))
        .collect::<Vec<_>>();
    hits.sort_by(agent_hit_test_hit_order);
    for (rank, hit) in hits.iter_mut().enumerate() {
        hit.rank = rank;
    }
    AgentHitTestReport {
        status: "ok".to_owned(),
        session_id: report.session_id.clone(),
        frame_id: report.frame_id.clone(),
        source: report.source.clone(),
        viewport: report.viewport,
        x,
        y,
        top_object_id: hits.first().map(|hit| hit.object_id.clone()),
        hits,
    }
}

pub(super) fn agent_hit_test_object_hits(
    object: &AgentObservedObject,
    x: u32,
    y: u32,
) -> Vec<AgentHitTestHit> {
    let Some(rich_text_ref) = &object.rich_text_ref else {
        return agent_image_or_generic_object_hits(object, x, y);
    };
    if !rich_text_ref.hit_test {
        return Vec::new();
    }
    rich_text_ref
        .hit_regions
        .iter()
        .filter(|region| agent_bbox_contains(&region.bbox, x, y))
        .map(|region| AgentHitTestHit {
            rank: 0,
            object_id: object.id.clone(),
            object: AgentImageObjectRef::from_observed(object),
            layer: agent_hit_test_layer(object, rich_text_ref, region),
            role: object.role.clone(),
            text: object.text.clone(),
            bbox: object.bbox.clone(),
            polygon: object.polygon.clone(),
            capture_refs: object.capture_refs.clone(),
            region: region.clone(),
            rich_text_ref: Some(rich_text_ref.clone()),
            depth: region
                .depth
                .or(object.resolved_object_depth())
                .or(rich_text_ref.object_depth),
        })
        .collect()
}

pub(super) fn agent_image_or_generic_object_hits(
    object: &AgentObservedObject,
    x: u32,
    y: u32,
) -> Vec<AgentHitTestHit> {
    if !agent_object_contains_point(object, x, y) {
        return Vec::new();
    }
    agent_image_or_generic_hit_regions(object)
        .into_iter()
        .map(|region| AgentHitTestHit {
            rank: 0,
            object_id: object.id.clone(),
            object: AgentImageObjectRef::from_observed(object),
            layer: region
                .proxy_layer
                .clone()
                .or_else(|| object.resolved_object_layer())
                .unwrap_or_else(|| object.layer.clone()),
            role: object.role.clone(),
            text: object.text.clone(),
            bbox: object.bbox.clone(),
            polygon: object.polygon.clone(),
            capture_refs: object.capture_refs.clone(),
            depth: region.depth.or(object.resolved_object_depth()),
            region,
            rich_text_ref: None,
        })
        .collect()
}

pub(super) fn agent_image_or_generic_hit_regions(
    object: &AgentObservedObject,
) -> Vec<AgentHitRegion> {
    let mut regions = vec![agent_generic_object_hit_region(object)];
    if let AgentObservedObjectContent::Image(content) = &object.content {
        regions.extend(
            content
                .proxies
                .iter()
                .filter(|proxy| proxy.hit_test)
                .map(|proxy| agent_image_proxy_hit_region(object, proxy)),
        );
    }
    regions
}

pub(super) fn agent_generic_object_hit_region(object: &AgentObservedObject) -> AgentHitRegion {
    AgentHitRegion {
        kind: AgentHitRegionKind::Object,
        bbox: object.bbox.clone(),
        range: RichTextRange::new(0, 0),
        proxy_id: None,
        proxy_type: None,
        proxy_declaration: None,
        proxy_role: None,
        proxy_layer: object.resolved_object_layer(),
        depth: object.resolved_object_depth(),
        proxy_params: BTreeMap::new(),
    }
}

pub(super) fn agent_image_proxy_hit_region(
    object: &AgentObservedObject,
    proxy: &AgentPresentationObjectProxyRef,
) -> AgentHitRegion {
    AgentHitRegion {
        kind: AgentHitRegionKind::ObjectProxy,
        bbox: object.bbox.clone(),
        range: RichTextRange::new(0, 0),
        proxy_id: Some(proxy.id.clone()),
        proxy_type: proxy.type_name.clone(),
        proxy_declaration: proxy.declaration.clone(),
        proxy_role: proxy.role.clone(),
        proxy_layer: proxy
            .layer
            .clone()
            .or_else(|| object.resolved_object_layer()),
        depth: proxy.depth.or_else(|| object.resolved_object_depth()),
        proxy_params: proxy.params.clone(),
    }
}

pub(super) fn agent_object_contains_point(object: &AgentObservedObject, x: u32, y: u32) -> bool {
    if !agent_bbox_contains(&object.bbox, x, y) {
        return false;
    }
    if object.polygon.len() >= 3 {
        return agent_polygon_contains(&object.polygon, x, y);
    }
    true
}

pub(super) fn agent_hit_test_layer(
    object: &AgentObservedObject,
    rich_text_ref: &AgentRichTextElementRef,
    region: &AgentHitRegion,
) -> String {
    region
        .proxy_layer
        .clone()
        .or_else(|| object.resolved_object_layer())
        .or_else(|| rich_text_ref.object_layer.clone())
        .unwrap_or_else(|| object.layer.clone())
}

pub(super) fn agent_hit_test_hit_order(
    left: &AgentHitTestHit,
    right: &AgentHitTestHit,
) -> std::cmp::Ordering {
    right
        .depth
        .unwrap_or(0)
        .cmp(&left.depth.unwrap_or(0))
        .then_with(|| {
            agent_hit_test_region_priority(left.region.kind)
                .cmp(&agent_hit_test_region_priority(right.region.kind))
        })
        .then_with(|| {
            agent_hit_test_role_priority(&left.role).cmp(&agent_hit_test_role_priority(&right.role))
        })
        .then_with(|| agent_bbox_area(&left.region.bbox).cmp(&agent_bbox_area(&right.region.bbox)))
        .then_with(|| left.object_id.cmp(&right.object_id))
}

pub(super) const fn agent_hit_test_region_priority(kind: AgentHitRegionKind) -> u8 {
    match kind {
        AgentHitRegionKind::TextObjectProxy => 0,
        AgentHitRegionKind::ObjectProxy => 5,
        AgentHitRegionKind::TextGlyph => 10,
        AgentHitRegionKind::GlyphCluster => 20,
        AgentHitRegionKind::RubyAnnotation => 30,
        AgentHitRegionKind::RubyBase => 40,
        AgentHitRegionKind::RubyObject => 50,
        AgentHitRegionKind::TextRun => 60,
        AgentHitRegionKind::TextLine => 70,
        AgentHitRegionKind::TextPage => 80,
        AgentHitRegionKind::Object => 90,
    }
}

pub(super) fn agent_hit_test_role_priority(role: &str) -> u8 {
    match role {
        "rich_text_proxy" => 0,
        "rich_text_glyph" => 10,
        "rich_text_cluster" => 20,
        "rich_text_ruby" => 30,
        "rich_text_run" => 40,
        "rich_text_line" => 50,
        "rich_text_page" => 60,
        _ => 100,
    }
}

pub(super) fn agent_bbox_contains(bbox: &AgentBBox, x: u32, y: u32) -> bool {
    x >= bbox.x
        && y >= bbox.y
        && x < bbox.x.saturating_add(bbox.width)
        && y < bbox.y.saturating_add(bbox.height)
}

pub(super) fn agent_bbox_area(bbox: &AgentBBox) -> u64 {
    u64::from(bbox.width) * u64::from(bbox.height)
}

pub(super) fn agent_polygon_contains(polygon: &[AgentPoint], x: u32, y: u32) -> bool {
    let x = f64::from(x);
    let y = f64::from(y);
    let mut inside = false;
    for index in 0..polygon.len() {
        let current = &polygon[index];
        let previous = &polygon[(index + polygon.len() - 1) % polygon.len()];
        let yi = f64::from(current.y);
        let yj = f64::from(previous.y);
        if (yi > y) == (yj > y) {
            continue;
        }
        let xi = f64::from(current.x);
        let xj = f64::from(previous.x);
        let intersection_x = (xj - xi) * (y - yi) / (yj - yi) + xi;
        if x < intersection_x {
            inside = !inside;
        }
    }
    inside
}

pub(super) fn validate_agent_observe_options(
    options: &AgentObserveOptions,
) -> Result<(), ExitCode> {
    if options.capture_step == Some(0) {
        eprintln!("error: --capture-step must be greater than zero");
        return Err(ExitCode::from(2));
    }
    if agent_observe_effective_steps(options) == 0 {
        eprintln!("error: --steps must be greater than zero");
        return Err(ExitCode::from(2));
    }
    if options.viewport_width == 0 || options.viewport_height == 0 {
        eprintln!("error: --viewport-width and --viewport-height must be greater than zero");
        return Err(ExitCode::from(2));
    }
    if options.textbox_height == Some(0) {
        eprintln!("error: --textbox-height must be greater than zero");
        return Err(ExitCode::from(2));
    }
    if options.layer.is_some() && options.object.is_some() {
        eprintln!("error: --layer and --object cannot be used together");
        return Err(ExitCode::from(2));
    }
    if options.out.is_some() && options.image.is_none() {
        eprintln!("error: --out requires --image");
        return Err(ExitCode::from(2));
    }
    if options.capture.is_some()
        && !matches!(
            options.image,
            Some(AgentObserveImageKind::Png | AgentObserveImageKind::RawRgba)
        )
    {
        eprintln!("error: --capture requires --image png or --image raw-rgba");
        return Err(ExitCode::from(2));
    }
    if matches!(options.resource, Some(AgentObserveResourceKind::Overlay))
        && !matches!(options.image, Some(AgentObserveImageKind::Overlay))
    {
        eprintln!("error: --resource overlay requires --image overlay");
        return Err(ExitCode::from(2));
    }
    if options.read_uri.is_some() && options.resource.is_some() {
        eprintln!("error: --read-uri and --resource cannot be used together");
        return Err(ExitCode::from(2));
    }
    if options.mcp && options.resource.is_none() && options.read_uri.is_none() {
        eprintln!("error: --mcp requires --resource or --read-uri");
        return Err(ExitCode::from(2));
    }
    if !options.mcp && options.mcp_format != AgentObserveMcpFormat::Read {
        eprintln!("error: --mcp-format requires --mcp");
        return Err(ExitCode::from(2));
    }
    if options
        .capture_time_seconds
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        eprintln!("error: --capture-time must be a finite non-negative number of seconds");
        return Err(ExitCode::from(2));
    }
    Ok(())
}

pub(super) fn agent_observe_effective_steps(options: &AgentObserveOptions) -> usize {
    options.capture_step.unwrap_or(options.steps)
}

pub(super) fn agent_observe_capture_time_seconds(options: &AgentObserveOptions) -> f32 {
    options
        .capture_time_seconds
        .unwrap_or_else(|| match options.capture_step {
            Some(step) => agent_capture_time_seconds_from_step(step),
            None => 60.0,
        })
}

pub(super) fn agent_observe_report_capture_time_millis(
    options: &AgentObserveOptions,
) -> Option<u32> {
    (options.capture_time_seconds.is_some() || options.capture_step.is_some())
        .then(|| agent_capture_time_millis(agent_observe_capture_time_seconds(options)))
}

pub(super) fn agent_report_capture_time_seconds(report: &AgentObservationReport) -> f32 {
    report.capture_time_millis.map_or(60.0, |millis| {
        (f64::from(millis) / 1000.0)
            .to_string()
            .parse()
            .unwrap_or(f32::MAX)
    })
}

pub(super) fn agent_capture_time_seconds_from_step(step: usize) -> f32 {
    f32::from(u16::try_from(step).unwrap_or(u16::MAX))
}

pub(super) fn agent_capture_time_millis(time_seconds: f32) -> u32 {
    if !time_seconds.is_finite() || time_seconds <= 0.0 {
        return 0;
    }
    let millis = f64::from(time_seconds) * 1000.0;
    if millis >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        millis.round().to_string().parse().unwrap_or(u32::MAX)
    }
}

pub(super) fn agent_observe_resource_by_uri(
    report: &AgentObservationReport,
    uri: &str,
) -> Result<AgentResource, ExitCode> {
    agent_observe_resource_by_uri_with_page_and_time(
        report,
        uri,
        None,
        agent_report_capture_time_seconds(report),
    )
}

pub(super) fn agent_observe_resource_by_uri_with_page_and_time(
    report: &AgentObservationReport,
    uri: &str,
    page_override: Option<usize>,
    capture_time_seconds: f32,
) -> Result<AgentResource, ExitCode> {
    agent_observe_resource_by_uri_with_page_and_time_and_session(
        report,
        uri,
        page_override,
        capture_time_seconds,
        None,
    )
}

pub(super) fn agent_observe_resource_by_uri_with_page_and_time_and_session(
    report: &AgentObservationReport,
    uri: &str,
    page_override: Option<usize>,
    capture_time_seconds: f32,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<AgentResource, ExitCode> {
    agent_observe_resource_by_uri_with_page_and_time_and_session_and_frame_store(
        report,
        uri,
        page_override,
        capture_time_seconds,
        native_session,
        &AgentImageFrameStore::default(),
    )
}

pub(super) fn agent_observe_resource_by_uri_with_page_and_time_and_session_and_frame_store(
    report: &AgentObservationReport,
    uri: &str,
    page_override: Option<usize>,
    capture_time_seconds: f32,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
    image_frames: &AgentImageFrameStore,
) -> Result<AgentResource, ExitCode> {
    if uri
        == format!(
            "arcweft://session/{}/observation/latest.json",
            report.session_id
        )
    {
        return report
            .observation_resource()
            .map_err(|error| agent_json_error(&error));
    }
    if uri
        == format!(
            "arcweft://session/{}/frame/{}/objects.json",
            report.session_id, report.tick
        )
    {
        return report
            .objects_resource()
            .map_err(|error| agent_json_error(&error));
    }
    if let Some(resource) = agent_presentation_tree_resource_from_uri(report, uri) {
        return resource;
    }
    if uri
        == format!(
            "arcweft://session/{}/frame/{}/overlay.svg",
            report.session_id, report.tick
        )
    {
        let selected = report.objects.iter().collect::<Vec<_>>();
        let overlay = agent_overlay_svg(&report.viewport, &selected);
        return Ok(AgentResource {
            uri: uri.to_owned(),
            kind: AgentResourceKind::OverlaySvg,
            mime_type: "image/svg+xml".to_owned(),
            hash: hash_hex(overlay.as_bytes()),
            image: None,
            body: AgentResourceBody::Text(overlay),
        });
    }
    if uri == format!("arcweft://session/{}/logs.ndjson", report.session_id) {
        return report
            .logs_resource()
            .map_err(|error| agent_json_error(&error));
    }
    if uri == format!("arcweft://session/{}/signals.json", report.session_id) {
        return report
            .signals_resource()
            .map_err(|error| agent_json_error(&error));
    }
    if uri == format!("arcweft://session/{}/audio.json", report.session_id) {
        return report
            .audio_resource()
            .map_err(|error| agent_json_error(&error));
    }
    let Some(request) = agent_capture_request_from_uri(report, uri) else {
        eprintln!("error: unsupported Agent resource URI: {uri}");
        return Err(ExitCode::from(2));
    };
    let request = AgentCaptureReadRequest {
        page: page_override.unwrap_or(request.page),
        capture_time_seconds,
        ..request
    };
    match native_session {
        Some(native_session) => agent_native_capture_resource_with_session_and_frame_store(
            report,
            &request,
            native_session,
            image_frames,
        ),
        None => agent_observe_capture_resource(report, &request),
    }
}

pub(super) fn agent_presentation_tree_resource_from_uri(
    report: &AgentObservationReport,
    uri: &str,
) -> Option<Result<AgentResource, ExitCode>> {
    let (base_uri, query_string) = uri.split_once('?').unwrap_or((uri, ""));
    if base_uri
        != format!(
            "arcweft://session/{}/frame/{}/presentation-tree.json",
            report.session_id, report.tick
        )
    {
        return None;
    }

    if query_string.is_empty() {
        return Some(
            report
                .presentation_tree_resource()
                .map_err(|error| agent_json_error(&error)),
        );
    }

    Some(
        agent_presentation_tree_query_from_uri(query_string).and_then(|query| {
            report
                .filtered_presentation_tree_resource(uri.to_owned(), &query)
                .map_err(|error| agent_json_error(&error))
        }),
    )
}

pub(super) fn agent_presentation_tree_query_from_uri(
    query_string: &str,
) -> Result<AgentPresentationTreeQuery, ExitCode> {
    let mut query = AgentPresentationTreeQuery::default();
    for part in query_string.split('&') {
        let Some((key, value)) = part.split_once('=') else {
            eprintln!("error: invalid presentation-tree query segment: {part}");
            return Err(ExitCode::from(2));
        };
        if value.is_empty() {
            eprintln!("error: presentation-tree query value for `{key}` must not be empty");
            return Err(ExitCode::from(2));
        }
        match key {
            "role" => query.role = Some(value.to_owned()),
            "rich_text_kind" => {
                query.rich_text_kind = Some(agent_rich_text_kind_from_query_value(value)?);
            }
            "object_layer" => query.object_layer = Some(value.to_owned()),
            "effect" | "effect_id" => query.effect_id = Some(value.to_owned()),
            "shader" | "shader_id" => query.shader_id = Some(value.to_owned()),
            "motion" | "motion_function" | "motion_function_id" => {
                query.motion_function_id = Some(value.to_owned());
            }
            "proxy" | "object_proxy" | "object_proxy_id" => {
                query.object_proxy_id = Some(value.to_owned());
            }
            "proxy_type" | "object_proxy_type" | "type" => {
                query.object_proxy_type = Some(value.to_owned());
            }
            "proxy_role" | "object_proxy_role" => {
                query.object_proxy_role = Some(value.to_owned());
            }
            "proxy_struct" | "object_proxy_struct" | "struct" => {
                query.object_proxy_struct = Some(value.to_owned());
            }
            "proxy_param" | "object_proxy_param" => {
                query.object_proxy_param = Some(agent_proxy_param_query_value(value));
            }
            "has_transform" => query.has_transform = Some(agent_bool_query_value(value)?),
            _ if key.starts_with("proxy_param.") => {
                query.object_proxy_param = Some(agent_proxy_param_query_key_value(
                    key.trim_start_matches("proxy_param."),
                    value,
                )?);
            }
            _ if key.starts_with("object_proxy_param.") => {
                query.object_proxy_param = Some(agent_proxy_param_query_key_value(
                    key.trim_start_matches("object_proxy_param."),
                    value,
                )?);
            }
            _ => {
                eprintln!("error: unsupported presentation-tree query key: {key}");
                return Err(ExitCode::from(2));
            }
        }
    }
    Ok(query)
}

pub(super) fn agent_proxy_param_query_value(value: &str) -> AgentPresentationObjectProxyParamQuery {
    value.split_once('=').map_or_else(
        || AgentPresentationObjectProxyParamQuery {
            key: value.to_owned(),
            value: None,
        },
        |(key, value)| AgentPresentationObjectProxyParamQuery {
            key: key.to_owned(),
            value: Some(value.to_owned()),
        },
    )
}

pub(super) fn agent_proxy_param_query_key_value(
    key: &str,
    value: &str,
) -> Result<AgentPresentationObjectProxyParamQuery, ExitCode> {
    if key.is_empty() {
        eprintln!("error: proxy parameter query key must not be empty");
        return Err(ExitCode::from(2));
    }
    Ok(AgentPresentationObjectProxyParamQuery {
        key: key.to_owned(),
        value: Some(value.to_owned()),
    })
}

pub(super) fn agent_rich_text_kind_from_query_value(
    value: &str,
) -> Result<AgentRichTextElementKind, ExitCode> {
    match value {
        "text_page" => Ok(AgentRichTextElementKind::TextPage),
        "text_line" => Ok(AgentRichTextElementKind::TextLine),
        "text_run" => Ok(AgentRichTextElementKind::TextRun),
        "text_glyph" => Ok(AgentRichTextElementKind::TextGlyph),
        "ruby" => Ok(AgentRichTextElementKind::Ruby),
        "glyph_cluster" => Ok(AgentRichTextElementKind::GlyphCluster),
        "text_object_proxy" => Ok(AgentRichTextElementKind::TextObjectProxy),
        _ => {
            eprintln!("error: unsupported rich_text_kind query value: {value}");
            Err(ExitCode::from(2))
        }
    }
}

pub(super) fn agent_bool_query_value(value: &str) -> Result<bool, ExitCode> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => {
            eprintln!("error: expected boolean query value, got: {value}");
            Err(ExitCode::from(2))
        }
    }
}
