
use super::*;

pub(super) fn run_agent_observation(
    executor: &mut RuntimeExecutorInstance,
    catalog: &LineDisplayCatalog,
    mut context: AgentObservationRunContext<'_>,
) -> Result<AgentObservationRunOutput, arcweft_host_adapter::HostAdapterError> {
    let viewport = AgentViewport {
        width: context.options.viewport_width,
        height: context.options.viewport_height,
        scale: 1.0,
    };
    let mut host = context
        .host_config
        .source_path
        .map(|path| {
            NativeTaskBridge::try_new(
                path,
                context.host_config.policy.clone(),
                context.host_config.adapter_registrars,
            )
        })
        .transpose()?;
    let mut objects: Vec<AgentObservedObject> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut task_request_count = 0usize;
    let mut tick = 0usize;
    let effective_steps = agent_observe_effective_steps(context.options);
    let force_capture_step = context.options.capture_step.is_some();
    let mut native_session = context.native_session;
    for step_index in 0..effective_steps {
        tick = context.tick_offset.saturating_add(step_index);
        let result = executor.step_with_root_bindings(
            RuntimeStepInput {
                input_events: std::mem::take(&mut context.input_events),
                task_events: std::mem::take(&mut *context.task_events),
                ..RuntimeStepInput::default()
            },
            &context.options.values,
            step_options(context.options.mode, context.options.max_ops),
        );
        let RuntimeStepResult { mut output, .. } = result;
        diagnostics.extend(output.diagnostics.iter().map(|diagnostic| AgentDiagnostic {
            step: step_index,
            severity: AgentDiagnosticSeverity::Error,
            source: Some("runtime".to_owned()),
            code: None,
            effect_id: None,
            message: diagnostic.message.clone(),
        }));
        for event in &output.flow_events {
            let textbox_index = objects
                .iter()
                .filter(|object| agent_is_dialogue_textbox(object))
                .count();
            match agent_observed_objects_for_flow_event(
                step_index,
                textbox_index,
                catalog,
                event,
                &viewport,
                context.options,
                native_session.as_deref_mut(),
            ) {
                Ok(event_objects) => objects.extend(event_objects),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
        let task_requests = std::mem::take(&mut output.requests.tasks);
        task_request_count += task_requests.len();
        let done = matches!(
            executor.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        if done && !force_capture_step {
            break;
        }
        if let Some(host) = host.as_mut() {
            *context.task_events = host.complete_tasks(task_requests);
        }
    }
    Ok(AgentObservationRunOutput {
        report: finish_agent_observation_report(
            executor,
            context.source_path,
            AgentObservationTrace {
                viewport,
                objects,
                diagnostics,
                task_request_count,
                tick,
            },
            context.options,
        ),
        image_frames: AgentImageFrameStore::default(),
    })
}

pub(super) fn agent_observed_objects_for_flow_event(
    step: usize,
    textbox_index: usize,
    catalog: &LineDisplayCatalog,
    event: &FlowEvent,
    viewport: &AgentViewport,
    options: &AgentObserveOptions,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Vec<AgentObservedObject>, AgentDiagnostic> {
    let capture_time_seconds = agent_observe_capture_time_seconds(options);
    let FlowEvent::DialogueLine { line, bindings } = event else {
        return Ok(Vec::new());
    };
    let Some(spec) = catalog.find(line) else {
        return Err(AgentDiagnostic {
            step,
            severity: AgentDiagnosticSeverity::Warning,
            source: Some("runtime_plan".to_owned()),
            code: Some("missing_display_catalog_entry".to_owned()),
            effect_id: None,
            message: format!("missing display catalog entry for line {}", line.0),
        });
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::new(bindings.clone()))
        .map_err(|error| AgentDiagnostic {
            step,
            severity: AgentDiagnosticSeverity::Error,
            source: Some("render_text".to_owned()),
            code: Some("line_display_resolve_failed".to_owned()),
            effect_id: None,
            message: error.to_string(),
        })?;
    let mut textbox = agent_textbox_object(step, textbox_index, frame, viewport, options);
    let mut native_session = native_session;
    if let Some(capture_bbox) = agent_native_textbox_capture_bbox_for_page(
        &textbox,
        viewport,
        0,
        capture_time_seconds,
        native_session.as_deref_mut(),
    ) {
        textbox.capture_refs =
            agent_object_capture_refs_for_page("cli", step, &textbox.id, &capture_bbox, 0);
    }
    let native_bounds = agent_native_rich_text_element_bboxes(
        &textbox,
        viewport,
        capture_time_seconds,
        native_session.as_deref_mut(),
    );
    let children = agent_rich_text_child_objects(
        step,
        textbox_index,
        &textbox,
        viewport,
        capture_time_seconds,
        &native_bounds,
        native_session,
    );
    let mut objects = Vec::with_capacity(1 + children.len());
    objects.push(textbox);
    objects.extend(children);
    Ok(objects)
}

pub(super) fn finish_agent_observation_report(
    executor: &RuntimeExecutorInstance,
    source_path: &Path,
    trace: AgentObservationTrace,
    options: &AgentObserveOptions,
) -> AgentObservationReport {
    let AgentObservationTrace {
        viewport,
        objects,
        diagnostics,
        task_request_count,
        tick,
    } = trace;
    let object_refs = objects.iter().collect::<Vec<_>>();
    let overlay_svg = agent_overlay_svg(&viewport, &object_refs);
    let render_hash = hash_hex(overlay_svg.as_bytes());
    let observations = &executor.fiber().observations;
    let signals = observations
        .signals
        .iter()
        .map(|(name, value)| AgentAssignment {
            name: name.clone(),
            value: value.clone(),
        })
        .collect::<Vec<_>>();
    let metrics = observations
        .metrics
        .iter()
        .map(|(name, value)| AgentAssignment {
            name: name.clone(),
            value: value.clone(),
        })
        .collect::<Vec<_>>();
    let mut actions = agent_action_targets(&objects);
    actions.extend(agent_action_targets_for_runtime_status(
        &executor.fiber().status,
    ));
    let layers = agent_observed_layers("cli", tick, &objects);
    let presentation_tree = AgentPresentationTree::from_layers_and_objects(&layers, &objects);
    let status = flow_status_label(&executor.fiber().status);
    let state_hash = hash_hex(
        format!(
            "{}:{}:{}:{}:{}",
            status,
            tick,
            objects.len(),
            diagnostics.len(),
            task_request_count
        )
        .as_bytes(),
    );
    AgentObservationReport {
        status: if matches!(executor.fiber().status, FlowFiberStatus::Failed(_)) {
            "failed".to_owned()
        } else {
            "ok".to_owned()
        },
        session_id: "cli".to_owned(),
        tick,
        frame_id: format!("frame.{tick}"),
        state_hash,
        render_hash: render_hash.clone(),
        source: report_path(source_path),
        viewport,
        images: Vec::new(),
        layers,
        objects,
        presentation_tree,
        actions,
        ui_tree: AgentUiTree {
            root: "ui.root".to_owned(),
            children: vec!["dialogue.layer".to_owned()],
        },
        scene_graph: Vec::new(),
        audio_state: AgentAudioState {
            active_voices: Vec::new(),
            pending_events: Vec::new(),
        },
        logs: observations.logs.clone(),
        signals,
        metrics,
        events: observations.events.clone(),
        diagnostics,
        steps: tick + 1,
        capture_time_millis: agent_observe_report_capture_time_millis(options),
        task_requests: task_request_count,
        final_status: status,
        overlay_svg: None,
    }
}

pub(super) fn agent_action_targets_for_runtime_status(
    status: &FlowFiberStatus,
) -> Vec<AgentActionTarget> {
    let FlowFiberStatus::Choice(state) = status else {
        return Vec::new();
    };
    state
        .options
        .iter()
        .map(|option| {
            let target = option.id.as_deref().unwrap_or(option.label.as_str());
            AgentActionTarget {
                id: format!("action.select_choice.{target}"),
                target: target.to_owned(),
                action: AgentActionKind::SelectChoice,
                kind: AgentActionDispatch::Semantic,
                enabled: true,
            }
        })
        .collect()
}

pub(super) fn agent_runtime_presentation_image_observation(
    source_path: &Path,
    step: usize,
    viewport: &AgentViewport,
    calls: &[RuntimeCall],
    capture_time_seconds: f32,
) -> (AgentUiImageObservation, Vec<AgentDiagnostic>) {
    let mut inputs = Vec::new();
    let mut background_input = None;
    let mut diagnostics = Vec::new();
    let mut image_decode_cache = AgentSourceImageDecodeCache::default();
    let image_declarations =
        agent_load_declared_image_objects_or_diagnostic(source_path, step, &mut diagnostics);
    for (call_index, call) in calls.iter().enumerate() {
        let image_call =
            match agent_runtime_image_call(call, call_index, viewport, &image_declarations) {
                Ok(Some(image_call)) => image_call,
                Ok(None) => continue,
                Err(message) => {
                    diagnostics.push(AgentDiagnostic {
                        step,
                        severity: AgentDiagnosticSeverity::Warning,
                        source: Some("agent.presentation_image".to_owned()),
                        code: Some("image_call_invalid".to_owned()),
                        effect_id: Some(call.callee.clone()),
                        message,
                    });
                    continue;
                }
            };
        match image_decode_cache.decode_source_image_asset(source_path, image_call.asset.as_str()) {
            Ok(image) => {
                let input = agent_image_presentation_input(&image_call, image);
                if image_call.background_slot {
                    background_input = Some(input);
                } else {
                    inputs.push(input);
                }
            }
            Err(message) => diagnostics.push(AgentDiagnostic {
                step,
                severity: AgentDiagnosticSeverity::Warning,
                source: Some("agent.presentation_image".to_owned()),
                code: Some("image_asset_unavailable".to_owned()),
                effect_id: Some(call.callee.clone()),
                message,
            }),
        }
    }
    if let Some(background_input) = background_input {
        inputs.insert(0, background_input);
    }
    if inputs.is_empty() {
        return (AgentUiImageObservation::default(), diagnostics);
    }
    let frame = match arcweft_ui::UiImagePresentationFrame::from_inputs(inputs) {
        Ok(frame) => frame,
        Err(error) => {
            diagnostics.push(AgentDiagnostic {
                step,
                severity: AgentDiagnosticSeverity::Warning,
                source: Some("agent.presentation_image".to_owned()),
                code: Some("image_ui_lower_failed".to_owned()),
                effect_id: None,
                message: error.to_string(),
            });
            return (AgentUiImageObservation::default(), diagnostics);
        }
    };
    let (outputs, image_sources) = frame.into_parts();
    let layer_tree = agent_layer_tree_for_ui_outputs(&outputs);
    let mut builder = arcweft_runtime_host::UiFrameCommitBuilder::new(&layer_tree);
    for (layer, output) in outputs {
        if let Err(error) = builder.push_layer(layer, output) {
            diagnostics.push(AgentDiagnostic {
                step,
                severity: AgentDiagnosticSeverity::Warning,
                source: Some("agent.presentation_image".to_owned()),
                code: Some("image_ui_commit_failed".to_owned()),
                effect_id: None,
                message: error.to_string(),
            });
        }
    }
    let visual_time_millis = u64::from(agent_capture_time_millis(capture_time_seconds));
    (
        agent_image_observation_from_ui_frame(
            "cli",
            step,
            viewport,
            &builder.finish(),
            &image_sources,
            visual_time_millis,
        ),
        diagnostics,
    )
}

pub(super) fn agent_load_declared_image_objects_or_diagnostic(
    source_path: &Path,
    step: usize,
    diagnostics: &mut Vec<AgentDiagnostic>,
) -> BTreeMap<String, DeclaredImageObject> {
    match load_declared_image_objects(source_path) {
        Ok(declarations) => declarations,
        Err(message) => {
            diagnostics.push(AgentDiagnostic {
                step,
                severity: AgentDiagnosticSeverity::Warning,
                source: Some("agent.presentation_image".to_owned()),
                code: Some("image_declaration_unavailable".to_owned()),
                effect_id: None,
                message,
            });
            BTreeMap::new()
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct AgentRuntimeImageCall {
    pub(super) asset: arcweft_id::PublicId,
    pub(super) object: arcweft_id::PublicId,
    pub(super) target: arcweft_id::PublicId,
    pub(super) layer: arcweft_id::PublicId,
    pub(super) bounds: arcweft_presentation::hit::HitRect,
    pub(super) fit: arcweft_presentation::image::ImageObjectFit,
    pub(super) alignment: ImageObjectAlignment,
    pub(super) opacity_milli: u16,
    pub(super) playback: ImageObjectPlayback,
    pub(super) transform: ImageObjectTransform,
    pub(super) depth_milli: i32,
    pub(super) actions: Vec<arcweft_id::PublicId>,
    pub(super) params: BTreeMap<arcweft_id::PublicId, ImageObjectParam>,
    pub(super) proxies: Vec<ImageObjectProxy>,
    pub(super) background_slot: bool,
    pub(super) enabled: bool,
    pub(super) visible: bool,
}

#[derive(Debug, Default)]
pub(super) struct AgentSourceImageDecodeCache {
    pub(super) images: BTreeMap<String, arcweft_image::DecodedImage>,
    pub(super) hits: usize,
    pub(super) misses: usize,
}

impl AgentSourceImageDecodeCache {
    pub(super) fn decode_source_image_asset(
        &mut self,
        source_path: &Path,
        asset_id: &str,
    ) -> Result<arcweft_image::DecodedImage, String> {
        if let Some(image) = self.images.get(asset_id) {
            self.hits = self.hits.saturating_add(1);
            return Ok(image.clone());
        }
        let image = agent_decode_source_image_asset(source_path, asset_id)?;
        self.misses = self.misses.saturating_add(1);
        self.images.insert(asset_id.to_owned(), image.clone());
        Ok(image)
    }

    #[cfg(test)]
    pub(super) fn hits(&self) -> usize {
        self.hits
    }

    #[cfg(test)]
    pub(super) fn misses(&self) -> usize {
        self.misses
    }
}

pub(super) fn agent_runtime_image_call(
    call: &RuntimeCall,
    call_index: usize,
    viewport: &AgentViewport,
    image_declarations: &BTreeMap<String, DeclaredImageObject>,
) -> Result<Option<AgentRuntimeImageCall>, String> {
    match call.callee.as_str() {
        "bg" => agent_background_runtime_image_call(call, viewport).map(Some),
        "image" | "image.show" => {
            agent_object_runtime_image_call(call, call_index, image_declarations).map(Some)
        }
        _ => Ok(None),
    }
}

pub(super) fn agent_background_runtime_image_call(
    call: &RuntimeCall,
    viewport: &AgentViewport,
) -> Result<AgentRuntimeImageCall, String> {
    let asset = agent_image_call_asset(call)
        .ok_or_else(|| "`bg(...)` requires an `asset` argument".to_owned())?;
    Ok(AgentRuntimeImageCall {
        asset,
        object: arcweft_id::PublicId::try_new("image.background.default")
            .expect("static image object id is valid"),
        target: arcweft_id::PublicId::try_new("target.background.default")
            .expect("static target id is valid"),
        layer: arcweft_id::PublicId::try_new("layer.background").expect("static layer id is valid"),
        bounds: arcweft_presentation::hit::HitRect::new(
            0.0,
            0.0,
            viewport.width.to_string().parse().unwrap_or(0.0),
            viewport.height.to_string().parse().unwrap_or(0.0),
        ),
        fit: agent_call_named_value(call, "fit")
            .and_then(agent_image_fit_from_call_arg)
            .unwrap_or(arcweft_presentation::image::ImageObjectFit::Cover),
        alignment: agent_image_call_alignment(call),
        opacity_milli: agent_call_named_value(call, "opacity")
            .and_then(agent_image_call_opacity_milli)
            .unwrap_or(1_000),
        playback: agent_image_call_playback(call),
        transform: ImageObjectTransform::identity(),
        depth_milli: 0,
        actions: Vec::new(),
        params: BTreeMap::new(),
        proxies: Vec::new(),
        background_slot: true,
        enabled: true,
        visible: true,
    })
}

pub(super) fn agent_object_runtime_image_call(
    call: &RuntimeCall,
    call_index: usize,
    image_declarations: &BTreeMap<String, DeclaredImageObject>,
) -> Result<AgentRuntimeImageCall, String> {
    if let Some(declaration_id) = agent_declared_image_object_id(call) {
        let declaration = image_declarations
            .get(declaration_id.as_str())
            .ok_or_else(|| {
                format!(
                    "declared image object `{}` was not found",
                    declaration_id.as_str()
                )
            })?;
        let expanded_call = RuntimeCall {
            callee: call.callee.clone(),
            args: merge_declared_image_args(declaration, agent_image_call_override_args(call)),
        };
        return agent_object_runtime_image_call(&expanded_call, call_index, image_declarations);
    }
    let asset = agent_image_call_asset(call)
        .ok_or_else(|| "`image(...)` requires an `asset` argument".to_owned())?;
    let x = agent_required_image_call_length(call, "x", 1)?;
    let y = agent_required_image_call_length(call, "y", 2)?;
    let width = agent_required_image_call_length(call, "width", 3)?;
    let height = agent_required_image_call_length(call, "height", 4)?;
    let object = agent_call_named_value(call, "id")
        .and_then(agent_public_id_from_call_arg)
        .unwrap_or_else(|| {
            let stem = asset
                .as_str()
                .strip_prefix("asset.")
                .unwrap_or(asset.as_str());
            arcweft_id::PublicId::try_new(format!("image.{stem}.{call_index}"))
                .expect("generated image object id is valid")
        });
    let target = agent_call_named_value(call, "target")
        .and_then(agent_public_id_from_call_arg)
        .unwrap_or_else(|| {
            arcweft_id::PublicId::try_new(format!("target.{}", object.as_str()))
                .expect("generated image target id is valid")
        });
    let layer = agent_call_named_value(call, "layer")
        .and_then(agent_public_id_from_call_arg)
        .unwrap_or_else(|| {
            arcweft_id::PublicId::try_new("layer.game_ui").expect("static layer id is valid")
        });
    let fit = agent_call_named_value(call, "fit")
        .and_then(agent_image_fit_from_call_arg)
        .unwrap_or_default();
    let alignment = agent_image_call_alignment(call);
    let depth_milli = agent_call_named_value(call, "depth")
        .and_then(agent_image_call_milli)
        .unwrap_or_default();
    let opacity_milli = agent_call_named_value(call, "opacity")
        .and_then(agent_image_call_opacity_milli)
        .unwrap_or(1_000);
    let playback = agent_image_call_playback(call);
    let transform = agent_image_call_transform(call);
    let enabled = agent_call_named_value(call, "enabled")
        .and_then(agent_image_call_bool)
        .unwrap_or(true);
    let visible = agent_call_named_value(call, "visible")
        .and_then(agent_image_call_bool)
        .unwrap_or(true);
    Ok(AgentRuntimeImageCall {
        asset,
        object,
        target,
        layer,
        bounds: arcweft_presentation::hit::HitRect::new(x, y, width, height),
        fit,
        alignment,
        opacity_milli,
        playback,
        transform,
        depth_milli,
        actions: agent_image_call_actions(call),
        params: agent_image_call_params(call),
        proxies: agent_image_call_proxies(call),
        background_slot: false,
        enabled,
        visible,
    })
}

pub(super) fn agent_declared_image_object_id(call: &RuntimeCall) -> Option<arcweft_id::PublicId> {
    let id = agent_call_positional_value(call, 0).and_then(public_image_ref_arg)?;
    arcweft_id::PublicId::try_new(id).ok()
}

pub(super) fn agent_image_call_override_args(call: &RuntimeCall) -> Vec<String> {
    let mut skipped_decl_ref = false;
    call.args
        .iter()
        .filter_map(|arg| {
            if runtime_arg_name(arg).is_none()
                && !skipped_decl_ref
                && public_image_ref_arg(arg).is_some()
            {
                skipped_decl_ref = true;
                None
            } else {
                Some(arg.clone())
            }
        })
        .collect()
}

pub(super) fn agent_image_call_asset(call: &RuntimeCall) -> Option<arcweft_id::PublicId> {
    agent_call_named_value(call, "asset")
        .or_else(|| agent_call_positional_value(call, 0))
        .and_then(agent_asset_id_from_call_arg)
}

pub(super) fn agent_asset_id_from_call_arg(arg: &str) -> Option<arcweft_id::PublicId> {
    let id = agent_public_id_from_call_arg(arg)?;
    id.as_str().starts_with("asset.").then_some(id)
}

pub(super) fn agent_public_id_from_call_arg(arg: &str) -> Option<arcweft_id::PublicId> {
    let value = arg.trim().trim_matches('"').trim_matches('\'');
    let value = value.strip_prefix('@').unwrap_or(value);
    arcweft_id::PublicId::try_new(value).ok()
}

pub(super) fn agent_call_named_value<'a>(call: &'a RuntimeCall, name: &str) -> Option<&'a str> {
    call.args.iter().find_map(|arg| {
        let (arg_name, value) = arg.split_once(" = ")?;
        (arg_name.trim() == name).then_some(value.trim())
    })
}

pub(super) fn agent_call_positional_value(call: &RuntimeCall, index: usize) -> Option<&str> {
    call.args
        .iter()
        .filter(|arg| !arg.contains(" = "))
        .nth(index)
        .map(String::as_str)
}

pub(super) fn agent_required_image_call_length(
    call: &RuntimeCall,
    name: &str,
    positional_index: usize,
) -> Result<f32, String> {
    let value = agent_call_named_value(call, name)
        .or_else(|| agent_call_positional_value(call, positional_index))
        .ok_or_else(|| format!("`image(...)` requires `{name}`"))?;
    agent_image_call_length(value)
        .ok_or_else(|| format!("`image(...)` argument `{name}` must be a finite px length"))
}

pub(super) fn agent_image_call_length(value: &str) -> Option<f32> {
    let value = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .strip_suffix("px")
        .unwrap_or_else(|| value.trim().trim_matches('"').trim_matches('\''))
        .trim();
    let parsed = value.parse::<f32>().ok()?;
    parsed.is_finite().then_some(parsed)
}

pub(super) fn agent_image_fit_from_call_arg(
    value: &str,
) -> Option<arcweft_presentation::image::ImageObjectFit> {
    match value.trim().trim_matches('"').trim_matches('\'') {
        "contain" => Some(arcweft_presentation::image::ImageObjectFit::Contain),
        "cover" => Some(arcweft_presentation::image::ImageObjectFit::Cover),
        "stretch" => Some(arcweft_presentation::image::ImageObjectFit::Stretch),
        "intrinsic" => Some(arcweft_presentation::image::ImageObjectFit::Intrinsic),
        _ => None,
    }
}

pub(super) fn agent_image_call_alignment(call: &RuntimeCall) -> ImageObjectAlignment {
    ImageObjectAlignment::new(
        agent_call_named_value(call, "alignment.x")
            .or_else(|| agent_call_named_value(call, "align.x"))
            .and_then(|value| agent_image_alignment_component_milli(value, "x"))
            .unwrap_or_else(|| ImageObjectAlignment::default().x_milli()),
        agent_call_named_value(call, "alignment.y")
            .or_else(|| agent_call_named_value(call, "align.y"))
            .and_then(|value| agent_image_alignment_component_milli(value, "y"))
            .unwrap_or_else(|| ImageObjectAlignment::default().y_milli()),
    )
}

pub(super) fn agent_image_alignment_component_milli(value: &str, axis: &str) -> Option<i32> {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    match (axis, value) {
        ("x", "left" | "start") | ("y", "top" | "start") => return Some(0),
        ("x" | "y", "center" | "middle") => return Some(500),
        ("x", "right" | "end") | ("y", "bottom" | "end") => return Some(1_000),
        _ => {}
    }
    if let Ok(integer) = value.parse::<i32>() {
        return Some(if (0..=1).contains(&integer) {
            integer.saturating_mul(1_000)
        } else {
            integer.clamp(0, 1_000)
        });
    }
    let decimal = value.parse::<f64>().ok()?;
    if !decimal.is_finite() {
        return None;
    }
    format!("{:.0}", (decimal * 1_000.0).round().clamp(0.0, 1_000.0))
        .parse()
        .ok()
}

pub(super) fn agent_image_call_actions(call: &RuntimeCall) -> Vec<arcweft_id::PublicId> {
    call.args
        .iter()
        .filter_map(|arg| {
            let (name, value) = arg.split_once(" = ")?;
            matches!(name.trim(), "action" | "actions").then_some(value)
        })
        .flat_map(|value| value.split(','))
        .filter_map(agent_public_id_from_call_arg)
        .collect()
}

pub(super) fn agent_image_call_params(
    call: &RuntimeCall,
) -> BTreeMap<arcweft_id::PublicId, ImageObjectParam> {
    call.args
        .iter()
        .filter_map(|arg| {
            let (name, value) = arg.split_once(" = ")?;
            let key = name.trim().strip_prefix("param.")?;
            let key = arcweft_id::PublicId::try_new(format!("param.{key}")).ok()?;
            Some((key, agent_image_call_param(value.trim())))
        })
        .collect()
}

pub(super) fn agent_image_call_proxies(call: &RuntimeCall) -> Vec<ImageObjectProxy> {
    let Some(id) = agent_call_named_value(call, "proxy.id").and_then(agent_public_id_from_call_arg)
    else {
        return Vec::new();
    };
    let mut proxy = ImageObjectProxy::new(id);
    if let Some(value) = agent_call_named_value(call, "proxy.type") {
        proxy = proxy.with_type_name(value.trim().trim_matches('"').trim_matches('\''));
    }
    if let Some(value) = agent_call_named_value(call, "proxy.role") {
        proxy = proxy.with_role(value.trim().trim_matches('"').trim_matches('\''));
    }
    if let Some(layer) =
        agent_call_named_value(call, "proxy.layer").and_then(agent_public_id_from_call_arg)
    {
        proxy = proxy.with_layer(layer);
    }
    if let Some(depth) =
        agent_call_named_value(call, "proxy.depth").and_then(agent_image_call_milli)
    {
        proxy = proxy.with_depth_milli(depth);
    }
    if let Some(hit_test) =
        agent_call_named_value(call, "proxy.hit_test").and_then(agent_image_call_bool)
    {
        proxy = proxy.with_hit_test(hit_test);
    }
    for (key, value) in agent_image_call_proxy_params(call) {
        proxy = proxy.with_param(key, value);
    }
    vec![proxy]
}

pub(super) fn agent_image_call_proxy_params(
    call: &RuntimeCall,
) -> BTreeMap<arcweft_id::PublicId, ImageObjectParam> {
    call.args
        .iter()
        .filter_map(|arg| {
            let (name, value) = arg.split_once(" = ")?;
            let key = name.trim().strip_prefix("proxy.param.")?;
            let key = arcweft_id::PublicId::try_new(format!("param.{key}")).ok()?;
            Some((key, agent_image_call_param(value.trim())))
        })
        .collect()
}

pub(super) fn agent_image_call_param(value: &str) -> ImageObjectParam {
    let trimmed = value.trim();
    if let Some(id) = agent_public_id_from_call_arg(trimmed).filter(|_| trimmed.starts_with('@')) {
        return ImageObjectParam::Id(id);
    }
    let unquoted = trimmed.trim_matches('"').trim_matches('\'');
    match unquoted {
        "true" => ImageObjectParam::Bool(true),
        "false" => ImageObjectParam::Bool(false),
        _ => agent_image_call_milli(unquoted).map_or_else(
            || ImageObjectParam::Text(unquoted.to_owned()),
            ImageObjectParam::Milli,
        ),
    }
}

pub(super) fn agent_image_call_milli(value: &str) -> Option<i32> {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    if let Ok(integer) = value.parse::<i32>() {
        return Some(integer);
    }
    let decimal = value.parse::<f64>().ok()?;
    if !decimal.is_finite() {
        return None;
    }
    (decimal * 1_000.0)
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX))
        .to_string()
        .parse()
        .ok()
}

pub(super) fn agent_image_call_opacity_milli(value: &str) -> Option<u16> {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    if let Ok(integer) = value.parse::<u16>() {
        if integer <= 1 {
            return Some(integer.saturating_mul(1_000));
        }
        return Some(integer.min(1_000));
    }
    let decimal = value.parse::<f64>().ok()?;
    if !decimal.is_finite() {
        return None;
    }
    (decimal * 1_000.0)
        .round()
        .clamp(0.0, 1_000.0)
        .to_string()
        .parse()
        .ok()
}

pub(super) fn agent_image_call_playback(call: &RuntimeCall) -> ImageObjectPlayback {
    let start_time_millis = agent_call_named_value(call, "playback.start")
        .or_else(|| agent_call_named_value(call, "playback.start_time"))
        .and_then(agent_image_call_time_millis)
        .unwrap_or_default();
    let mut playback = ImageObjectPlayback::new(start_time_millis);
    if let Some(rate_milli) =
        agent_call_named_value(call, "playback.rate").and_then(agent_image_call_rate_milli)
    {
        playback = playback.with_rate_milli(rate_milli);
    }
    if let Some(paused_at_millis) = agent_call_named_value(call, "playback.paused_at")
        .or_else(|| agent_call_named_value(call, "playback.pause_at"))
        .and_then(agent_image_call_time_millis)
    {
        playback = playback.paused_at(paused_at_millis);
    }
    if let Some(local_time_millis) = agent_call_named_value(call, "playback.local_time")
        .or_else(|| agent_call_named_value(call, "playback.pinned_local_time"))
        .and_then(agent_image_call_time_millis)
    {
        playback = playback.pinned_local_time(local_time_millis);
    }
    playback
}

pub(super) fn agent_image_call_time_millis(value: &str) -> Option<u64> {
    let value = value.trim().trim_matches('"').trim_matches('\'').trim();
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number.trim(), 1.0)
    } else if let Some(number) = value.strip_suffix('s') {
        (number.trim(), 1_000.0)
    } else {
        (value, 1_000.0)
    };
    let millis = number.parse::<f64>().ok()? * multiplier;
    if !millis.is_finite() || millis < 0.0 {
        return None;
    }
    format!("{:.0}", millis.round()).parse().ok()
}

pub(super) fn agent_image_call_rate_milli(value: &str) -> Option<u32> {
    let value = value.trim().trim_matches('"').trim_matches('\'').trim();
    let parsed = value
        .strip_suffix('x')
        .unwrap_or(value)
        .parse::<f64>()
        .ok()?;
    let milli = if (0.0..=1.0).contains(&parsed) {
        parsed * 1_000.0
    } else {
        parsed
    };
    if !milli.is_finite() || milli < 0.0 {
        return None;
    }
    format!("{:.0}", milli.round()).parse().ok()
}

pub(super) fn agent_image_call_bool(value: &str) -> Option<bool> {
    match value.trim().trim_matches('"').trim_matches('\'') {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub(super) fn agent_image_call_transform(call: &RuntimeCall) -> ImageObjectTransform {
    let mut transform = ImageObjectTransform::identity();
    if let Some(value) = agent_call_named_value(call, "transform.m11")
        .and_then(agent_image_call_transform_component_milli)
    {
        transform.m11_milli = value;
    }
    if let Some(value) = agent_call_named_value(call, "transform.m12")
        .and_then(agent_image_call_transform_component_milli)
    {
        transform.m12_milli = value;
    }
    if let Some(value) = agent_call_named_value(call, "transform.m21")
        .and_then(agent_image_call_transform_component_milli)
    {
        transform.m21_milli = value;
    }
    if let Some(value) = agent_call_named_value(call, "transform.m22")
        .and_then(agent_image_call_transform_component_milli)
    {
        transform.m22_milli = value;
    }
    if let Some(value) =
        agent_call_named_value(call, "transform.tx").and_then(agent_image_call_length_milli)
    {
        transform.tx_milli = value;
    }
    if let Some(value) =
        agent_call_named_value(call, "transform.ty").and_then(agent_image_call_length_milli)
    {
        transform.ty_milli = value;
    }
    transform
}

pub(super) fn agent_image_call_transform_component_milli(value: &str) -> Option<i32> {
    agent_image_call_milli(value)
}

pub(super) fn agent_image_call_length_milli(value: &str) -> Option<i32> {
    let pixels = agent_image_call_length(value)?;
    let milli = f64::from(pixels) * 1_000.0;
    milli
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX))
        .to_string()
        .parse()
        .ok()
}

pub(super) fn agent_decode_source_image_asset(
    source_path: &Path,
    asset_id: &str,
) -> Result<arcweft_image::DecodedImage, String> {
    let Some(asset_stem) = asset_id.strip_prefix("asset.") else {
        return Err(format!(
            "image asset id `{asset_id}` must start with `asset.`"
        ));
    };
    let asset_relative = asset_stem.replace('.', "/");
    let asset_root = source_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".arcweft")
        .join("asset");
    for (extension, format) in [
        ("png", arcweft_image::ImageFormat::Png),
        ("jpg", arcweft_image::ImageFormat::Jpeg),
        ("jpeg", arcweft_image::ImageFormat::Jpeg),
        ("gif", arcweft_image::ImageFormat::Gif),
        ("webp", arcweft_image::ImageFormat::WebP),
    ] {
        let path = asset_root.join(format!("{asset_relative}.{extension}"));
        if !path.exists() {
            continue;
        }
        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read image asset {}: {error}", path.display()))?;
        return arcweft_image::decode_image_bytes(
            format,
            &bytes,
            arcweft_image::ImageDecodeOptions::default(),
        )
        .map_err(|error| format!("failed to decode image asset {}: {error}", path.display()));
    }
    Err(format!(
        "image asset `{asset_id}` was not found under {}",
        asset_root.display()
    ))
}

pub(super) fn agent_image_presentation_input(
    call: &AgentRuntimeImageCall,
    image: arcweft_image::DecodedImage,
) -> arcweft_ui::UiImagePresentationInput {
    let object = arcweft_presentation::image::ImagePresentationObject::new(
        arcweft_presentation::image::ImageObjectId::new(call.object.clone()),
        arcweft_presentation::image::ImageAssetRef::new(call.asset.clone()),
        arcweft_presentation::layer::LayerId::new(call.layer.clone()),
        arcweft_presentation::input::InteractionTarget::new(call.target.clone()),
        call.bounds,
    )
    .with_fit(call.fit)
    .with_alignment(call.alignment)
    .with_opacity_milli(call.opacity_milli)
    .with_playback(call.playback)
    .with_transform(call.transform)
    .with_depth_milli(call.depth_milli)
    .with_enabled(call.enabled)
    .with_visible(call.visible);
    let object = call.actions.iter().cloned().fold(
        object,
        arcweft_presentation::image::ImagePresentationObject::with_action,
    );
    let object = call.proxies.iter().cloned().fold(
        object,
        arcweft_presentation::image::ImagePresentationObject::with_proxy,
    );
    let object = call
        .params
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .fold(object, |object, (key, value)| object.with_param(key, value));
    arcweft_ui::UiImagePresentationInput::new(object, image)
}

pub(super) fn agent_layer_tree_for_ui_outputs(
    outputs: &[(
        arcweft_presentation::layer::LayerId,
        arcweft_ui::UiLayerOutput,
    )],
) -> arcweft_presentation::layer::LayerTree {
    use arcweft_presentation::layer::{LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase};
    let root = arcweft_presentation::layer::LayerId::new(
        arcweft_id::PublicId::try_new("layer.root").expect("static root layer id is valid"),
    );
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        LayerOrder {
            phase: RenderPhase::Background,
            z: 0,
            stable_index: 0,
        },
    ));
    for (index, (layer, _)) in outputs.iter().enumerate() {
        let kind = if layer.public_id().as_str() == "layer.background" {
            LayerKind::Background
        } else {
            LayerKind::GameUi
        };
        let phase = match kind {
            LayerKind::Background => RenderPhase::Background,
            _ => RenderPhase::GameUi,
        };
        tree.insert(
            LayerNode::new(
                layer.clone(),
                kind,
                LayerOrder {
                    phase,
                    z: 0,
                    stable_index: u32::try_from(index).unwrap_or(u32::MAX),
                },
            )
            .with_parent(root.clone()),
        )
        .expect("generated image presentation layer tree is valid");
    }
    tree
}

pub(super) fn agent_refresh_observation_object_indexes(report: &mut AgentObservationReport) {
    let object_refs = report.objects.iter().collect::<Vec<_>>();
    let overlay_svg = agent_overlay_svg(&report.viewport, &object_refs);
    report.render_hash = hash_hex(overlay_svg.as_bytes());
    report.layers = agent_observed_layers("cli", report.tick, &report.objects);
    report.presentation_tree =
        AgentPresentationTree::from_layers_and_objects(&report.layers, &report.objects);
    report.actions = agent_action_targets(&report.objects);
}

pub(super) fn agent_action_targets(objects: &[AgentObservedObject]) -> Vec<AgentActionTarget> {
    objects
        .iter()
        .flat_map(agent_action_targets_for_object)
        .collect()
}

pub(super) fn agent_action_targets_for_object(
    object: &AgentObservedObject,
) -> Vec<AgentActionTarget> {
    match &object.content {
        AgentObservedObjectContent::RichText { .. } if agent_is_dialogue_textbox(object) => {
            vec![AgentActionTarget {
                id: format!("action.advance_text.{}", object.id),
                target: object.id.clone(),
                action: AgentActionKind::AdvanceText,
                kind: AgentActionDispatch::Semantic,
                enabled: object.visible && object.enabled,
            }]
        }
        AgentObservedObjectContent::Image(content) => content
            .actions
            .iter()
            .map(|action| AgentActionTarget {
                id: action.clone(),
                target: content
                    .target
                    .clone()
                    .or_else(|| content.object.clone())
                    .unwrap_or_else(|| object.id.clone()),
                action: AgentActionKind::Invoke,
                kind: AgentActionDispatch::Semantic,
                enabled: object.visible && object.enabled,
            })
            .collect(),
        AgentObservedObjectContent::RichText { .. } | AgentObservedObjectContent::Custom { .. } => {
            Vec::new()
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct AgentImageOutput {
    pub(super) uri: String,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(super) struct AgentRasterCapture {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) crop_origin: Option<AgentImageCropOrigin>,
    pub(super) composition: AgentImageComposition,
    pub(super) background: [u8; 4],
    pub(super) rgba: Vec<u8>,
    pub(super) diagnostics: Vec<arcweft_render_native::NativeVisualDiagnostic>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct AgentRasterContentStats {
    pub(super) bbox: Option<AgentImageContentBBox>,
    pub(super) content_pixels: u64,
}

impl AgentRasterCapture {
    pub(super) fn new(
        width: u32,
        height: u32,
        color: [u8; 4],
        composition: AgentImageComposition,
    ) -> Self {
        let pixel_count = usize::try_from(width)
            .unwrap_or(0)
            .saturating_mul(usize::try_from(height).unwrap_or(0));
        let mut rgba = Vec::with_capacity(pixel_count.saturating_mul(4));
        for _ in 0..pixel_count {
            rgba.extend_from_slice(&color);
        }
        Self {
            width,
            height,
            crop_origin: None,
            composition,
            background: color,
            rgba,
            diagnostics: Vec::new(),
        }
    }

    pub(super) fn content_stats(&self) -> AgentRasterContentStats {
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut count = 0_u64;
        for y in 0..self.height {
            for x in 0..self.width {
                let index = usize::try_from(y)
                    .unwrap_or(0)
                    .saturating_mul(usize::try_from(self.width).unwrap_or(0))
                    .saturating_add(usize::try_from(x).unwrap_or(0))
                    .saturating_mul(4)
                    .saturating_add(3);
                let Some(pixel) = self
                    .rgba
                    .get(index.saturating_sub(3)..index.saturating_add(1))
                else {
                    continue;
                };
                if pixel == self.background {
                    continue;
                }
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                count = count.saturating_add(1);
            }
        }
        AgentRasterContentStats {
            bbox: (count > 0).then_some(AgentImageContentBBox {
                x: min_x,
                y: min_y,
                width: max_x.saturating_sub(min_x).saturating_add(1),
                height: max_y.saturating_sub(min_y).saturating_add(1),
            }),
            content_pixels: count,
        }
    }
}

pub(super) fn agent_observe_image_output(
    report: &mut AgentObservationReport,
    options: &AgentObserveOptions,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
    image_frames: &AgentImageFrameStore,
) -> Result<Option<AgentImageOutput>, ExitCode> {
    let Some(image) = options.image else {
        return Ok(None);
    };
    match image {
        AgentObserveImageKind::Overlay => {
            let overlay_svg = {
                let selected = select_agent_capture_objects(&report.objects, options)?;
                agent_overlay_svg(&report.viewport, &selected)
            };
            let hash = hash_hex(overlay_svg.as_bytes());
            report.render_hash.clone_from(&hash);
            let uri = agent_capture_uri(report, "overlay", "svg", options);
            let scope = agent_capture_scope_for_options(options);
            report.images = vec![AgentImageResource {
                kind: AgentImageKind::OverlaySvg,
                renderer: AgentImageRenderer::Native,
                scope: agent_image_scope_for_capture_scope(&scope),
                composition: AgentImageComposition::OverlayVector,
                page: 0,
                capture_step: report.steps,
                capture_time_millis: agent_capture_time_millis(agent_observe_capture_time_seconds(
                    options,
                )),
                uri: uri.clone(),
                mime_type: "image/svg+xml".to_owned(),
                width: report.viewport.width,
                height: report.viewport.height,
                hash,
                crop_origin: None,
                content_bbox: None,
                content_viewport_bbox: None,
                content_pixels: None,
                object: agent_image_object_for_capture_scope(report, &scope),
                diagnostics: Vec::new(),
                written: options.out.as_deref().map(report_path),
            }];
            report.overlay_svg = Some(overlay_svg.clone());
            Ok(Some(AgentImageOutput {
                uri,
                bytes: overlay_svg.into_bytes(),
            }))
        }
        AgentObserveImageKind::RawRgba | AgentObserveImageKind::Png => {
            let request = agent_capture_request_for_options(report, image, options);
            let capture_result = match native_session {
                Some(native_session) => agent_native_capture_image_with_frame_store(
                    report,
                    &request,
                    native_session,
                    image_frames,
                )?,
                None => agent_native_capture_image(report, &request)?,
            };
            report
                .diagnostics
                .extend(capture_result.image.diagnostics.clone());
            let (mut image, bytes) = (capture_result.image, capture_result.bytes);
            image.written = options.out.as_deref().map(report_path);
            report.render_hash.clone_from(&image.hash);
            let uri = image.uri.clone();
            report.images = vec![image];
            Ok(Some(AgentImageOutput { uri, bytes }))
        }
    }
}

pub(super) fn agent_native_visual_diagnostics(
    step: usize,
    diagnostics: &[arcweft_render_native::NativeVisualDiagnostic],
) -> Vec<AgentDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| AgentDiagnostic {
            step,
            severity: match diagnostic.severity {
                arcweft_render_native::NativeVisualDiagnosticSeverity::Error => {
                    AgentDiagnosticSeverity::Error
                }
                arcweft_render_native::NativeVisualDiagnosticSeverity::Warning => {
                    AgentDiagnosticSeverity::Warning
                }
                arcweft_render_native::NativeVisualDiagnosticSeverity::Info => {
                    AgentDiagnosticSeverity::Info
                }
            },
            source: Some("native_rich_text".to_owned()),
            code: Some(diagnostic.code.clone()),
            effect_id: diagnostic.effect_id.clone(),
            message: format!(
                "native rich-text {}: {}",
                diagnostic.code, diagnostic.message
            ),
        })
        .collect()
}

pub(super) fn agent_capture_request_for_options(
    report: &AgentObservationReport,
    image_kind: AgentObserveImageKind,
    options: &AgentObserveOptions,
) -> AgentCaptureReadRequest {
    let capture_kind = agent_capture_kind(options);
    let extension = match image_kind {
        AgentObserveImageKind::Png => "png",
        AgentObserveImageKind::RawRgba => "rgba",
        AgentObserveImageKind::Overlay => "svg",
    };
    AgentCaptureReadRequest {
        uri: agent_capture_uri(report, capture_kind.resource_name(), extension, options),
        image_kind,
        capture_kind,
        scope: agent_capture_scope_for_options(options),
        page: options.page.unwrap_or(0),
        capture_step: report.steps,
        capture_time_seconds: agent_observe_capture_time_seconds(options),
    }
}

pub(super) fn agent_capture_scope_for_options(options: &AgentObserveOptions) -> AgentCaptureScope {
    if let Some(object_id) = &options.object {
        AgentCaptureScope::Object(object_id.clone())
    } else if let Some(layer) = &options.layer {
        AgentCaptureScope::Layer(layer.clone())
    } else {
        AgentCaptureScope::Viewport
    }
}

pub(super) fn agent_image_scope_for_capture_scope(scope: &AgentCaptureScope) -> AgentImageScope {
    match scope {
        AgentCaptureScope::Viewport => AgentImageScope::Viewport,
        AgentCaptureScope::Layer(id) => AgentImageScope::Layer { id: id.clone() },
        AgentCaptureScope::Object(id) => AgentImageScope::Object { id: id.clone() },
    }
}

pub(super) fn select_agent_capture_objects<'a>(
    objects: &'a [AgentObservedObject],
    options: &AgentObserveOptions,
) -> Result<Vec<&'a AgentObservedObject>, ExitCode> {
    if let Some(object_id) = &options.object {
        let Some(object) = objects.iter().find(|object| object.id == *object_id) else {
            eprintln!("error: no observed object matches --object {object_id}");
            return Err(ExitCode::from(2));
        };
        return Ok(vec![object]);
    }
    if let Some(layer) = &options.layer {
        let selected = objects
            .iter()
            .filter(|object| agent_object_matches_layer(object, layer))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            eprintln!("error: no observed object matches --layer {layer}");
            return Err(ExitCode::from(2));
        }
        return Ok(selected);
    }
    Ok(objects.iter().collect())
}

pub(super) fn agent_capture_kind(options: &AgentObserveOptions) -> AgentObserveCaptureKind {
    options.capture.unwrap_or(AgentObserveCaptureKind::Color)
}

pub(super) fn agent_image_kind(capture: AgentObserveCaptureKind) -> AgentImageKind {
    match capture {
        AgentObserveCaptureKind::Color => AgentImageKind::Color,
        AgentObserveCaptureKind::ObjectId => AgentImageKind::ObjectId,
        AgentObserveCaptureKind::Mask => AgentImageKind::Mask,
    }
}
