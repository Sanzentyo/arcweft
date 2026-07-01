use super::{
    AgentImageFrameStore, AgentObservationState, AgentObserveImageKind, AgentObserveOptions,
    ExitCode, NativeAdapterRegistrar, NativeAgentRuntimeState, NativeTaskBridge,
    agent_action_targets, agent_action_targets_for_runtime_status, agent_capture_time_millis,
    agent_mcp_project_context_from_hir, agent_native_capture_session_for_hir,
    agent_object_capture_refs_for_page, agent_observe_capture_time_seconds,
    agent_observe_effective_steps, agent_observe_layout_scene_graph,
    agent_observe_report_capture_time_millis, agent_observed_layers, agent_overlay_svg,
    agent_textbox_object, hash_hex, load_and_check_selection, native_host_policy_for_selection,
    report_path, resolve_source_selection,
};
use crate::app::bundle::compile_bundle_for_selection;
use arcweft_agent_protocol::{
    diagnostic::{AgentDiagnostic, AgentDiagnosticSeverity},
    geometry::{AgentBBox, AgentCoordinateSpace, AgentViewport},
    object::{AgentObservedObject, AgentObservedObjectContent},
    observation::AgentObservationReport,
    presentation::AgentPresentationTree,
    session::{AgentAssignment, AgentAudioState},
    ui::AgentUiTree,
};
use arcweft_bundle::BundleVirtualFileSpace;
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::task::TaskEvent;
use arcweft_interaction_model::input::RoutedInputEvent;
use arcweft_player_scene::frame::{PlayerFramePlanner, PlayerFrameRequest, PlayerPreparedFrame};
use arcweft_player_scene::{images::BundleImageCatalog, input::InputController};
use arcweft_presentation::{hit::HitRect, semantic::SemanticRole};
use arcweft_render_wgpu::{
    geometry::{PreparedFrame, RenderPreferences, RenderTextInputControl, RenderViewport},
    offscreen::SharedOffscreenCapture,
};
use arcweft_runtime_driver::{
    clock::RuntimeClockStep,
    display::BundlePresentationSnapshot,
    session::{BundleSession, BundleSessionOptions, BundleSessionStep, BundleStepInput},
    task::HostTaskDispatch,
};
use std::path::Path;

pub(super) fn agent_player_observation_for_options(
    options: &AgentObserveOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentObservationState, ExitCode> {
    let mut runtime = native_player_runtime_state_for_options(options, adapter_registrars)?;
    let observed = observe_native_player_runtime(&mut runtime, options, Vec::new())?;
    Ok(AgentObservationState {
        report: observed.report,
        image_frames: observed.image_frames,
        native_session: runtime.native_session,
    })
}

pub(super) struct NativePlayerObservedFrame {
    pub(super) report: AgentObservationReport,
    pub(super) image_frames: AgentImageFrameStore,
}

pub(super) fn native_player_runtime_state_for_options(
    options: &AgentObserveOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<NativeAgentRuntimeState, ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let checked = load_and_check_selection(&selection, None)?;
    let project_context = agent_mcp_project_context_from_hir(&checked.hir, selection.path())
        .map_err(|error| {
            eprintln!("error: failed to build native project context: {error}");
            ExitCode::FAILURE
        })?;
    let native_session = agent_native_capture_session_for_hir(&checked.hir)?;
    let mut phases = Vec::new();
    let compiled =
        compile_bundle_for_selection(&selection, vec![BundleVirtualFileSpace::Asset], &mut phases)?;
    let images = BundleImageCatalog::from_bundle(&compiled.bundle).map_err(|error| {
        eprintln!("error: player-backed observe image catalog failed: {error}");
        ExitCode::FAILURE
    })?;
    let session = BundleSession::new(
        &compiled.bundle,
        BundleSessionOptions {
            entry: options.entry.clone(),
            flow: options.flow.clone(),
            mode: options.mode.into(),
            max_ops: options.max_ops,
            root_bindings: options.values.clone(),
        },
    )
    .map_err(|error| {
        eprintln!("error: player-backed observe session failed: {error}");
        ExitCode::FAILURE
    })?;
    let host_policy = native_host_policy_for_selection(&selection)?;
    let host = NativeTaskBridge::try_new(selection.path(), host_policy, adapter_registrars)
        .map(Some)
        .map_err(|error| {
            eprintln!("error: failed to create native task bridge: {error}");
            ExitCode::FAILURE
        })?;
    Ok(NativeAgentRuntimeState {
        session,
        images,
        input: InputController::default(),
        source_path: selection.path().to_owned(),
        project_context,
        native_session,
        host,
        task_events: Vec::new(),
        next_clock_millis: 1,
    })
}

pub(super) fn observe_native_player_runtime(
    runtime: &mut NativeAgentRuntimeState,
    options: &AgentObserveOptions,
    input_events: Vec<RoutedInputEvent>,
) -> Result<NativePlayerObservedFrame, ExitCode> {
    let effective_steps = agent_observe_effective_steps(options);
    let force_capture_step = options.capture_step.is_some();
    let mut pending_input_events = input_events;
    let mut diagnostics = Vec::new();
    let mut task_request_count = 0usize;
    let mut last_step = None;
    for _ in 0..effective_steps {
        let clock =
            RuntimeClockStep::from_millis(runtime.next_clock_millis, 16).map_err(|error| {
                eprintln!("error: player-backed observe clock failed: {error}");
                ExitCode::FAILURE
            })?;
        runtime.next_clock_millis = runtime.next_clock_millis.saturating_add(1);
        let step = runtime.session.step_with_clock(
            clock,
            BundleStepInput {
                input_events: std::mem::take(&mut pending_input_events),
                task_events: std::mem::take(&mut runtime.task_events),
                ..BundleStepInput::default()
            },
        );
        let finished = step.finished;
        diagnostics.extend(
            step.diagnostics
                .iter()
                .cloned()
                .map(|message| AgentDiagnostic {
                    step: step.index,
                    severity: AgentDiagnosticSeverity::Error,
                    source: Some("runtime".to_owned()),
                    code: None,
                    effect_id: None,
                    message,
                }),
        );
        task_request_count = task_request_count.saturating_add(step.requested_tasks.len());
        runtime.task_events = if finished {
            Vec::new()
        } else {
            complete_player_runtime_tasks(runtime.host.as_mut(), &step.requested_tasks)
        };
        last_step = Some(step);
        if finished && !force_capture_step {
            break;
        }
    }
    let step = last_step.ok_or_else(|| {
        eprintln!("error: player-backed observe requires at least one runtime step");
        ExitCode::from(2)
    })?;
    let prepared = prepare_player_runtime_frame(runtime, &step.presentation, options)?;
    let objects = player_observed_objects(&prepared, &step.presentation, step.index, options);
    let mut image_frames = AgentImageFrameStore::default();
    if player_observe_requires_shared_capture(options) {
        let capture = player_observe_capture_frame(&prepared.frame)?;
        image_frames.set_full_frame(capture.width, capture.height, capture.rgba);
    }
    Ok(NativePlayerObservedFrame {
        report: player_observation_report(
            &runtime.source_path,
            &prepared,
            &step,
            objects,
            diagnostics,
            task_request_count,
            options,
        ),
        image_frames,
    })
}

fn complete_player_runtime_tasks(
    host: Option<&mut NativeTaskBridge>,
    requested_tasks: &[HostTaskDispatch],
) -> Vec<TaskEvent> {
    let Some(host) = host else {
        return Vec::new();
    };
    let tasks = requested_tasks
        .iter()
        .map(|dispatch| dispatch.task.clone())
        .collect::<Vec<_>>();
    host.complete_tasks(tasks)
        .into_iter()
        .map(|event| align_player_task_event(event, requested_tasks))
        .collect()
}

fn align_player_task_event(
    mut event: TaskEvent,
    requested_tasks: &[HostTaskDispatch],
) -> TaskEvent {
    if let Some(dispatch) = requested_tasks
        .iter()
        .find(|dispatch| dispatch.task.id == event.task_id)
    {
        event.logical_epoch = dispatch.logical_epoch;
        event.sequence = dispatch.sequence;
    }
    event
}

fn prepare_player_runtime_frame(
    runtime: &mut NativeAgentRuntimeState,
    presentation: &BundlePresentationSnapshot,
    options: &AgentObserveOptions,
) -> Result<PlayerPreparedFrame, ExitCode> {
    let visual_time_millis = u64::from(agent_capture_time_millis(
        agent_observe_capture_time_seconds(options),
    ));
    PlayerFramePlanner::prepare(
        &mut runtime.input,
        PlayerFrameRequest {
            presentation,
            images: &runtime.images,
            viewport: player_observe_viewport(options),
            image_time_millis: visual_time_millis,
            visual_time_millis,
            preferences: RenderPreferences::default(),
        },
    )
    .map_err(|error| {
        eprintln!("error: player-backed observe frame planning failed: {error}");
        ExitCode::FAILURE
    })
}

fn player_observe_capture_frame(
    prepared: &PreparedFrame,
) -> Result<arcweft_render_wgpu::offscreen::SharedFrameCapture, ExitCode> {
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ))
    .map_err(|error| {
        eprintln!("error: player-backed observe shared renderer setup failed: {error}");
        ExitCode::FAILURE
    })?;
    capture.capture_frame(prepared).map_err(|error| {
        eprintln!("error: player-backed observe shared renderer capture failed: {error}");
        ExitCode::FAILURE
    })
}

fn player_observe_requires_shared_capture(options: &AgentObserveOptions) -> bool {
    matches!(
        options.image,
        Some(AgentObserveImageKind::Png | AgentObserveImageKind::RawRgba)
    ) || options
        .read_uri
        .as_deref()
        .is_some_and(player_observe_uri_requests_raster)
}

fn player_observe_uri_requests_raster(uri: &str) -> bool {
    let base = uri.split_once('?').map_or(uri, |(base, _)| base);
    Path::new(base)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("png") || extension.eq_ignore_ascii_case("rgba")
        })
}

fn player_observe_viewport(options: &AgentObserveOptions) -> RenderViewport {
    RenderViewport {
        logical_width: u32_to_f32(options.viewport_width),
        logical_height: u32_to_f32(options.viewport_height),
        physical_width: options.viewport_width.max(1),
        physical_height: options.viewport_height.max(1),
        scale_factor: 1.0,
    }
}

fn player_observation_report(
    source_path: &Path,
    prepared: &PlayerPreparedFrame,
    step: &BundleSessionStep,
    objects: Vec<AgentObservedObject>,
    diagnostics: Vec<AgentDiagnostic>,
    task_request_count: usize,
    options: &AgentObserveOptions,
) -> AgentObservationReport {
    let viewport = AgentViewport {
        width: prepared.scene.viewport.physical_width,
        height: prepared.scene.viewport.physical_height,
        scale: 1.0,
    };
    let object_refs = objects.iter().collect::<Vec<_>>();
    let overlay_svg = agent_overlay_svg(&viewport, &object_refs);
    let render_hash = hash_hex(overlay_svg.as_bytes());
    let mut actions = agent_action_targets(&objects);
    actions.extend(agent_action_targets_for_runtime_status(&step.fiber_status));
    let layers = agent_observed_layers("cli", step.index, &objects);
    let presentation_tree = AgentPresentationTree::from_layers_and_objects(&layers, &objects);
    let state_hash = hash_hex(
        format!(
            "{}:{}:{}:{}:{}",
            step.status_label,
            step.index,
            objects.len(),
            step.diagnostics.len(),
            task_request_count
        )
        .as_bytes(),
    );
    AgentObservationReport {
        status: if matches!(step.fiber_status, FlowFiberStatus::Failed(_)) {
            "failed".to_owned()
        } else {
            "ok".to_owned()
        },
        session_id: "cli".to_owned(),
        tick: step.index,
        frame_id: format!("frame.{}", step.index),
        state_hash,
        render_hash,
        source: report_path(source_path),
        viewport,
        images: Vec::new(),
        layers: layers.clone(),
        objects,
        presentation_tree,
        actions,
        ui_tree: AgentUiTree {
            root: "ui.root".to_owned(),
            children: layers.iter().map(|layer| layer.id.clone()).collect(),
        },
        scene_graph: vec![agent_observe_layout_scene_graph(&viewport)],
        audio_state: AgentAudioState {
            active_voices: Vec::new(),
            pending_events: Vec::new(),
        },
        logs: step.observations.logs.clone(),
        signals: step
            .observations
            .signals
            .iter()
            .map(|(name, value)| AgentAssignment {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        metrics: step
            .observations
            .metrics
            .iter()
            .map(|(name, value)| AgentAssignment {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        events: step.observations.events.clone(),
        diagnostics,
        steps: step.index.saturating_add(1),
        capture_time_millis: agent_observe_report_capture_time_millis(options),
        task_requests: task_request_count,
        final_status: step.status_label.clone(),
        overlay_svg: None,
    }
}

fn player_observed_objects(
    prepared: &PlayerPreparedFrame,
    presentation: &BundlePresentationSnapshot,
    step: usize,
    options: &AgentObserveOptions,
) -> Vec<AgentObservedObject> {
    let viewport = AgentViewport {
        width: prepared.scene.viewport.physical_width,
        height: prepared.scene.viewport.physical_height,
        scale: 1.0,
    };
    let mut objects = Vec::new();
    if let Some(dialogue) = &presentation.dialogue {
        objects.push(agent_textbox_object(
            step,
            0,
            dialogue.clone(),
            &viewport,
            options,
        ));
    }
    objects.extend(
        prepared
            .frame
            .semantics
            .as_slice()
            .iter()
            .filter_map(|node| {
                let control = prepared
                    .scene
                    .text_inputs
                    .iter()
                    .find(|control| control.target == *node.target());
                player_semantic_object(step, node, control)
            }),
    );
    objects
}

fn player_semantic_object(
    step: usize,
    node: &arcweft_presentation::semantic::SemanticNode,
    control: Option<&RenderTextInputControl>,
) -> Option<AgentObservedObject> {
    if !node.visible() {
        return None;
    }
    let id = node.target().id().as_str().to_owned();
    let bbox = agent_bbox_from_hit_rect(node.bounds())?;
    let text = player_semantic_text(node.role(), node.label(), control);
    let mut object = AgentObservedObject {
        id: id.clone(),
        parent_id: None,
        entity: Some(id.clone()),
        layer: node.layer().public_id().as_str().to_owned(),
        role: player_semantic_role(node.role()).to_owned(),
        visible: node.visible(),
        enabled: node.enabled(),
        polygon: bbox.polygon(),
        capture_refs: agent_object_capture_refs_for_page("cli", step, &id, &bbox, 0),
        bbox,
        object_layer: None,
        object_depth: None,
        text,
        rich_text_ref: None,
        content: AgentObservedObjectContent::Custom {
            object_type: player_semantic_object_type(node.role()).to_owned(),
        },
    };
    if let Some(label) = node.label().filter(|label| !label.is_empty()) {
        object.text.get_or_insert_with(|| label.to_owned());
    }
    Some(object)
}

fn player_semantic_text(
    role: SemanticRole,
    label: Option<&str>,
    control: Option<&RenderTextInputControl>,
) -> Option<String> {
    match role {
        SemanticRole::SecureTextField => None,
        SemanticRole::TextField | SemanticRole::TextArea => control
            .map(|control| control.value.clone())
            .or_else(|| label.map(str::to_owned)),
        SemanticRole::Button
        | SemanticRole::TextBox
        | SemanticRole::Activity
        | SemanticRole::Image
        | SemanticRole::Debug
        | SemanticRole::Custom => label.map(str::to_owned),
    }
}

fn player_semantic_role(role: SemanticRole) -> &'static str {
    match role {
        SemanticRole::TextBox => "text_box",
        SemanticRole::Activity => "activity",
        SemanticRole::Button => "button",
        SemanticRole::TextField => "text_field",
        SemanticRole::TextArea => "text_area",
        SemanticRole::SecureTextField => "secure_text_field",
        SemanticRole::Image => "image",
        SemanticRole::Debug => "debug",
        SemanticRole::Custom => "custom",
    }
}

fn player_semantic_object_type(role: SemanticRole) -> &'static str {
    match role {
        SemanticRole::TextField | SemanticRole::TextArea | SemanticRole::SecureTextField => {
            "text_input"
        }
        SemanticRole::Button => "button",
        SemanticRole::TextBox => "text_box",
        SemanticRole::Activity => "activity",
        SemanticRole::Image => "image",
        SemanticRole::Debug => "debug",
        SemanticRole::Custom => "custom",
    }
}

fn agent_bbox_from_hit_rect(bounds: HitRect) -> Option<AgentBBox> {
    if !bounds.x.is_finite()
        || !bounds.y.is_finite()
        || !bounds.width.is_finite()
        || !bounds.height.is_finite()
    {
        return None;
    }
    let x = non_negative_f32_to_u32(bounds.x.floor());
    let y = non_negative_f32_to_u32(bounds.y.floor());
    let width = non_negative_f32_to_u32(bounds.width.ceil()).max(1);
    let height = non_negative_f32_to_u32(bounds.height.ceil()).max(1);
    Some(AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x,
        y,
        width,
        height,
    })
}

fn non_negative_f32_to_u32(value: f32) -> u32 {
    if value <= 0.0 {
        0
    } else {
        value.round().to_string().parse().unwrap_or(u32::MAX)
    }
}

fn u32_to_f32(value: u32) -> f32 {
    value.to_string().parse().unwrap_or(f32::MAX)
}
