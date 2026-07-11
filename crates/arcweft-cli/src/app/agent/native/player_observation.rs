use super::image_mapping::{
    agent_image_geometry_from_native_quad, agent_object_capture_refs_with_source,
};
use super::{
    AgentImageFrameStore, AgentObservationState, AgentObserveImageKind, AgentObserveOptions,
    AgentStoredImagePlacement, ExitCode, NativeAdapterRegistrar, NativeAgentRuntimeState,
    NativeTaskBridge, agent_action_targets, agent_action_targets_for_runtime_status,
    agent_action_targets_for_scroll_regions, agent_action_targets_for_semantics,
    agent_capture_time_millis, agent_mcp_project_context_from_hir,
    agent_native_capture_session_for_hir, agent_object_capture_refs_for_page,
    agent_observe_capture_time_seconds, agent_observe_effective_steps,
    agent_observe_layout_scene_graph, agent_observe_report_capture_time_millis,
    agent_observed_layers, agent_observed_scroll_regions, agent_observed_views,
    agent_observed_virtual_lists, agent_overlay_svg, agent_textbox_object,
    dedupe_agent_action_targets, hash_hex, load_and_check_selection,
    native_host_policy_for_selection, report_path, resolve_source_selection,
};
use crate::app::bundle::compile_bundle_for_selection;
use arcweft_agent_protocol::{
    diagnostic::{AgentDiagnostic, AgentDiagnosticSeverity},
    geometry::{AgentBBox, AgentCoordinateSpace, AgentViewport},
    image::{
        AgentCaptureSourceIdentity, AgentImageAlignment, AgentImageFit, AgentImageObjectParam,
        AgentImageTransform,
    },
    object::{
        AgentObservedImageContent, AgentObservedLayer, AgentObservedObject,
        AgentObservedObjectContent, AgentObservedView,
    },
    observation::AgentObservationReport,
    presentation::AgentPresentationTree,
    proxy::AgentPresentationObjectProxyRef,
    session::{AgentAssignment, AgentAudioState},
    view::{AgentObservedScrollRegion, AgentObservedVirtualList, AgentViewTree},
};
use arcweft_bundle::BundleVirtualFileSpace;
use arcweft_bundle::{BundleImageObject, BundleImageObjectParam, BundleImageObjectProxy};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::task::TaskEvent;
use arcweft_player_scene::fonts::PlayerFontSet;
use arcweft_player_scene::frame::{
    PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest, PlayerPreparedFrame,
};
use arcweft_player_scene::{images::BundleImageCatalog, input::InputController};
use arcweft_presentation::{
    fx::{FxDiagnostic, FxDiagnosticSeverity},
    hit::HitRect,
    image::ImageObjectFit,
    semantic::SemanticRole,
};
use arcweft_render_text::{Milli, RichTextParam};
use arcweft_render_wgpu::{
    geometry::{
        PreparedFrame, RenderImage, RenderPreferences, RenderTextInputControl, RenderViewport,
    },
    offscreen::SharedOffscreenCapture,
};
use arcweft_runtime_driver::{
    clock::RuntimeClockStep,
    display::BundlePresentationSnapshot,
    session::{BundleSession, BundleSessionOptions, BundleSessionStep, BundleStepInput},
    task::HostTaskDispatch,
};
use std::collections::BTreeMap;
use std::path::Path;

pub(super) fn agent_player_observation_for_options(
    options: &AgentObserveOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentObservationState, ExitCode> {
    let mut runtime = native_player_runtime_state_for_options(options, adapter_registrars)?;
    let observed =
        observe_native_player_runtime(&mut runtime, options, BundleStepInput::default())?;
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

struct PreparedPlayerRuntimeFrame {
    prepared: PlayerPreparedFrame,
    fonts: PlayerFontSet,
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
    let file_roots = selection.native_file_roots()?;
    let host = NativeTaskBridge::try_new(
        selection.path(),
        file_roots,
        host_policy,
        adapter_registrars,
    )
    .map(Some)
    .map_err(|error| {
        eprintln!("error: failed to create native task bridge: {error}");
        ExitCode::FAILURE
    })?;
    Ok(NativeAgentRuntimeState {
        session,
        images,
        input: InputController::default(),
        prepared_frame: None,
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
    step_input: BundleStepInput,
) -> Result<NativePlayerObservedFrame, ExitCode> {
    let effective_steps = agent_observe_effective_steps(options);
    let force_capture_step = options.capture_step.is_some();
    let mut pending_step_input = step_input;
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
        let mut step_input = std::mem::take(&mut pending_step_input);
        step_input.task_events = std::mem::take(&mut runtime.task_events);
        let step = runtime.session.step_with_clock(clock, step_input);
        let finished = step.finished;
        let fx_diagnostics = &step.presentation.fx_diagnostics;
        diagnostics.extend(
            step.diagnostics
                .iter()
                .filter(|message| {
                    !fx_diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.as_str() == message.as_str())
                })
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
        diagnostics.extend(
            fx_diagnostics
                .iter()
                .map(|diagnostic| agent_fx_diagnostic(step.index, diagnostic)),
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
    let prepared_runtime = prepare_player_runtime_frame(runtime, &step.presentation, options)?;
    let prepared = &prepared_runtime.prepared;
    append_frame_fx_diagnostics(&mut diagnostics, &prepared.frame, step.index);
    let viewport = player_observed_viewport(prepared);
    let mut objects =
        player_observed_objects(prepared, &step.presentation, step.index, &viewport, options);
    let mut image_frames = AgentImageFrameStore::default();
    objects.extend(player_observed_image_objects(
        prepared,
        &step.presentation,
        step.index,
        &viewport,
        &mut image_frames,
        u64::from(agent_capture_time_millis(
            agent_observe_capture_time_seconds(options),
        )),
    ));
    if player_observe_requires_shared_capture(options) {
        let capture = player_observe_capture_frame(&prepared.frame, &prepared_runtime.fonts)?;
        image_frames.set_full_frame(capture.width, capture.height, capture.rgba);
    }
    let report = player_observation_report(
        &runtime.source_path,
        prepared,
        objects,
        options,
        PlayerObservationEvidence {
            step: &step,
            diagnostics,
            task_request_count,
            virtual_ranges: runtime.session.view_virtualization().range_tables(),
        },
    );
    runtime.prepared_frame = Some(prepared_runtime.prepared);
    Ok(NativePlayerObservedFrame {
        report,
        image_frames,
    })
}

fn agent_fx_diagnostic(step: usize, diagnostic: &FxDiagnostic) -> AgentDiagnostic {
    AgentDiagnostic {
        step,
        severity: match diagnostic.severity {
            FxDiagnosticSeverity::Error => AgentDiagnosticSeverity::Error,
            FxDiagnosticSeverity::Warning => AgentDiagnosticSeverity::Warning,
        },
        source: Some("fx".to_owned()),
        code: Some(diagnostic.code.as_str().to_owned()),
        effect_id: diagnostic
            .context
            .definition
            .as_ref()
            .map(ToString::to_string),
        message: diagnostic.message.clone(),
    }
}

fn append_frame_fx_diagnostics(
    diagnostics: &mut Vec<AgentDiagnostic>,
    frame: &PreparedFrame,
    step: usize,
) {
    let additions = frame
        .fx_diagnostics
        .iter()
        .filter(|diagnostic| {
            !diagnostics.iter().any(|existing| {
                existing.code.as_deref() == Some(diagnostic.code.as_str())
                    && existing.effect_id
                        == diagnostic
                            .context
                            .definition
                            .as_ref()
                            .map(ToString::to_string)
            })
        })
        .map(|diagnostic| agent_fx_diagnostic(step, diagnostic))
        .collect::<Vec<_>>();
    diagnostics.extend(additions);
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
) -> Result<PreparedPlayerRuntimeFrame, ExitCode> {
    let visual_time_millis = u64::from(agent_capture_time_millis(
        agent_observe_capture_time_seconds(options),
    ));
    let fonts = PlayerFontSet::bundled_default();
    let mut planner = PlayerFramePlannerState::new();
    fonts.register_with_planner(&mut planner).map_err(|error| {
        eprintln!("error: player-backed observe font registration failed: {error}");
        ExitCode::FAILURE
    })?;
    let prepared = planner
        .prepare(
            &mut runtime.input,
            PlayerFrameRequest {
                presentation,
                fx_definitions: runtime.session.fx_definitions(),
                images: &runtime.images,
                viewport: player_observe_viewport(options),
                fit: PlayerFrameFit::raw(),
                image_time_millis: visual_time_millis,
                visual_time_millis,
                dialogue_reveal_complete: false,
                preferences: RenderPreferences::default(),
            },
        )
        .map_err(|error| {
            eprintln!("error: player-backed observe frame planning failed: {error}");
            ExitCode::FAILURE
        })?;
    Ok(PreparedPlayerRuntimeFrame { prepared, fonts })
}

fn player_observe_capture_frame(
    prepared: &PreparedFrame,
    fonts: &PlayerFontSet,
) -> Result<arcweft_render_wgpu::offscreen::SharedFrameCapture, ExitCode> {
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ))
    .map_err(|error| {
        eprintln!("error: player-backed observe shared renderer setup failed: {error}");
        ExitCode::FAILURE
    })?;
    fonts
        .register_with_offscreen_capture(&mut capture)
        .map_err(|error| {
            eprintln!("error: player-backed observe font registration failed: {error}");
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

struct PlayerObservationEvidence<'a> {
    step: &'a BundleSessionStep,
    diagnostics: Vec<AgentDiagnostic>,
    task_request_count: usize,
    virtual_ranges: Vec<arcweft_view::virtualization::ViewVirtualRangeTable>,
}

struct PlayerVisualEvidence {
    viewport: AgentViewport,
    render_hash: String,
    scroll_regions: Vec<AgentObservedScrollRegion>,
    virtual_lists: Vec<AgentObservedVirtualList>,
}

fn player_observation_report(
    source_path: &Path,
    prepared: &PlayerPreparedFrame,
    objects: Vec<AgentObservedObject>,
    options: &AgentObserveOptions,
    evidence: PlayerObservationEvidence<'_>,
) -> AgentObservationReport {
    let PlayerObservationEvidence {
        step,
        mut diagnostics,
        task_request_count,
        virtual_ranges,
    } = evidence;
    let PlayerVisualEvidence {
        viewport,
        render_hash,
        scroll_regions,
        virtual_lists,
    } = player_visual_evidence(prepared, &objects, &virtual_ranges);
    let mut actions = agent_action_targets(&objects);
    actions.extend(agent_action_targets_for_semantics(
        &prepared.frame.semantics,
    ));
    actions.extend(agent_action_targets_for_scroll_regions(&prepared.frame));
    actions.extend(agent_action_targets_for_runtime_status(&step.fiber_status));
    dedupe_agent_action_targets(&mut actions);
    let layers = agent_observed_layers("cli", step.index, &objects);
    let views = agent_observed_views("cli", step.index, &objects);
    push_missing_requested_scope_diagnostic(
        &mut diagnostics,
        step.index,
        options,
        &layers,
        &views,
        &objects,
    );
    let presentation_tree = AgentPresentationTree::from_layers_and_objects(&layers, &objects);
    let state_hash = hash_hex(
        format!(
            "{}:{}:{}:{}:{}:{}",
            step.status_label,
            step.index,
            objects.len(),
            step.diagnostics.len(),
            task_request_count,
            render_hash,
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
        views,
        objects,
        presentation_tree,
        actions,
        scroll_regions,
        virtual_lists,
        view_tree: AgentViewTree {
            root: "view.root".to_owned(),
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

fn player_visual_evidence(
    prepared: &PlayerPreparedFrame,
    objects: &[AgentObservedObject],
    virtual_ranges: &[arcweft_view::virtualization::ViewVirtualRangeTable],
) -> PlayerVisualEvidence {
    let viewport = player_observed_viewport(prepared);
    let object_refs = objects.iter().collect::<Vec<_>>();
    let overlay_svg = agent_overlay_svg(&viewport, &object_refs);
    let scroll_regions = agent_observed_scroll_regions(&prepared.frame);
    let virtual_lists = agent_observed_virtual_lists(virtual_ranges);
    let mut render_evidence = overlay_svg.into_bytes();
    render_evidence.extend(
        serde_json::to_vec(&scroll_regions).expect("finite typed scroll metadata serializes"),
    );
    render_evidence.extend(
        serde_json::to_vec(&virtual_lists).expect("finite typed virtual range metadata serializes"),
    );
    PlayerVisualEvidence {
        viewport,
        render_hash: hash_hex(&render_evidence),
        scroll_regions,
        virtual_lists,
    }
}

fn push_missing_requested_scope_diagnostic(
    diagnostics: &mut Vec<AgentDiagnostic>,
    step: usize,
    options: &AgentObserveOptions,
    layers: &[AgentObservedLayer],
    views: &[AgentObservedView],
    objects: &[AgentObservedObject],
) {
    push_missing_capture_scope_diagnostics(
        diagnostics,
        step,
        requested_capture_scopes(options),
        layers,
        views,
        objects,
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RequestedCaptureScope<'a> {
    kind: RequestedCaptureScopeKind,
    id: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestedCaptureScopeKind {
    View,
    Object,
    Layer,
}

impl RequestedCaptureScopeKind {
    fn label(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Object => "object",
            Self::Layer => "layer",
        }
    }
}

fn requested_capture_scopes(
    options: &AgentObserveOptions,
) -> [Option<RequestedCaptureScope<'_>>; 3] {
    [
        options.view.as_deref().map(|id| RequestedCaptureScope {
            kind: RequestedCaptureScopeKind::View,
            id,
        }),
        options.object.as_deref().map(|id| RequestedCaptureScope {
            kind: RequestedCaptureScopeKind::Object,
            id,
        }),
        options.layer.as_deref().map(|id| RequestedCaptureScope {
            kind: RequestedCaptureScopeKind::Layer,
            id,
        }),
    ]
}

fn push_missing_capture_scope_diagnostics(
    diagnostics: &mut Vec<AgentDiagnostic>,
    step: usize,
    requested_scopes: [Option<RequestedCaptureScope<'_>>; 3],
    layers: &[AgentObservedLayer],
    views: &[AgentObservedView],
    objects: &[AgentObservedObject],
) {
    for scope in requested_scopes
        .into_iter()
        .flatten()
        .filter(|scope| !capture_scope_is_observed(*scope, layers, views, objects))
    {
        diagnostics.push(AgentDiagnostic {
            step,
            severity: AgentDiagnosticSeverity::Error,
            source: Some("agent.observe".to_owned()),
            code: Some("AGENT_CAPTURE_MISSING_SCOPE".to_owned()),
            effect_id: None,
            message: format!(
                "no observed {} matches requested capture scope `{}` after presentation handle filtering",
                scope.kind.label(),
                scope.id
            ),
        });
    }
}

fn capture_scope_is_observed(
    scope: RequestedCaptureScope<'_>,
    layers: &[AgentObservedLayer],
    views: &[AgentObservedView],
    objects: &[AgentObservedObject],
) -> bool {
    match scope.kind {
        RequestedCaptureScopeKind::View => views.iter().any(|observed| observed.id == scope.id),
        RequestedCaptureScopeKind::Object => objects.iter().any(|observed| observed.id == scope.id),
        RequestedCaptureScopeKind::Layer => layers.iter().any(|observed| observed.id == scope.id),
    }
}

fn player_observed_viewport(prepared: &PlayerPreparedFrame) -> AgentViewport {
    AgentViewport {
        width: prepared.scene.viewport.physical_width,
        height: prepared.scene.viewport.physical_height,
        scale: 1.0,
    }
}

fn player_observed_objects(
    prepared: &PlayerPreparedFrame,
    presentation: &BundlePresentationSnapshot,
    step: usize,
    viewport: &AgentViewport,
    options: &AgentObserveOptions,
) -> Vec<AgentObservedObject> {
    let mut objects = Vec::new();
    let view_by_target = player_runtime_view_by_target(presentation);
    if let Some(dialogue) = &presentation.dialogue
        && let Some(stage) = dialogue.current_stage()
    {
        let mut textbox = agent_textbox_object(step, 0, stage.to_frame(), viewport, options);
        textbox.enabled = dialogue.is_waiting_for_advance();
        objects.push(textbox);
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
                player_semantic_object(step, node, control, &view_by_target)
            }),
    );
    objects
}

fn player_runtime_view_by_target(
    presentation: &BundlePresentationSnapshot,
) -> BTreeMap<String, String> {
    presentation
        .text_inputs
        .iter()
        .filter_map(|control| control.view.as_ref().map(|view| (control, view)))
        .flat_map(|(control, view)| {
            [
                (control.public_id.clone(), view.clone()),
                (control.target.clone(), view.clone()),
            ]
        })
        .chain(
            presentation
                .action_buttons
                .iter()
                .filter_map(|button| button.view.as_ref().map(|view| (button, view)))
                .flat_map(|(button, view)| {
                    [
                        (button.public_id.clone(), view.clone()),
                        (button.target.clone(), view.clone()),
                    ]
                }),
        )
        .collect()
}

fn player_observed_image_objects(
    prepared: &PlayerPreparedFrame,
    presentation: &BundlePresentationSnapshot,
    step: usize,
    viewport: &AgentViewport,
    image_frames: &mut AgentImageFrameStore,
    visual_time_millis: u64,
) -> Vec<AgentObservedObject> {
    prepared
        .frame
        .images
        .iter()
        .filter_map(|image| {
            let source = presentation
                .images
                .iter()
                .find(|object| object.id == image.id);
            player_observed_image_object(
                step,
                viewport,
                image,
                source,
                image_frames,
                visual_time_millis,
            )
        })
        .collect()
}

fn player_observed_image_object(
    step: usize,
    viewport: &AgentViewport,
    image: &RenderImage,
    source: Option<&BundleImageObject>,
    image_frames: &mut AgentImageFrameStore,
    visual_time_millis: u64,
) -> Option<AgentObservedObject> {
    if !source.is_none_or(|source| source.visible) {
        return None;
    }
    let object_id = format!("object.image.{}", image.id);
    let layer = source
        .and_then(|source| source.layer.clone())
        .unwrap_or_else(|| "image".to_owned());
    let target = source.and_then(|source| source.target.clone());
    let object_depth = source.map(|source| source.depth_milli);
    let native_quad = player_render_image_native_quad(image)?;
    let geometry = agent_image_geometry_from_native_quad(native_quad, viewport);
    let bbox = geometry.bbox;
    let polygon = geometry.polygon;
    image_frames.insert_with_placement(
        object_id.clone(),
        image.frame.width,
        image.frame.height,
        image.frame.rgba.clone(),
        Some(AgentStoredImagePlacement {
            dst: native_quad.dst,
            transform: native_quad.transform,
            opacity_milli: native_quad.opacity_milli,
        }),
    );
    Some(AgentObservedObject {
        id: object_id.clone(),
        parent_id: None,
        entity: Some(image.id.clone()),
        layer: layer.clone(),
        role: "image".to_owned(),
        visible: true,
        enabled: true,
        bbox: bbox.clone(),
        polygon,
        capture_refs: agent_object_capture_refs_with_source(
            "cli",
            step,
            &object_id,
            &bbox,
            0,
            AgentCaptureSourceIdentity::Object {
                id: object_id.clone(),
                parent_id: None,
                entity: Some(image.id.clone()),
                layer: layer.clone(),
                role: "image".to_owned(),
                object_layer: Some(layer.clone()),
                object_depth,
                rich_text: None,
            },
        ),
        object_layer: Some(layer),
        object_depth,
        text: None,
        rich_text_ref: None,
        content: AgentObservedObjectContent::Image(Box::new(AgentObservedImageContent {
            source: image.id.clone(),
            object: Some(image.id.clone()),
            target,
            asset: source.map(|source| source.asset.clone()),
            frame_index: image.frame.index,
            local_time_millis: source
                .map(|source| source.playback.local_time_millis(visual_time_millis)),
            opacity_milli: Some(image.opacity_milli),
            fit: Some(player_agent_image_fit(image.fit)),
            alignment: Some(AgentImageAlignment {
                x_milli: image.alignment.x_milli(),
                y_milli: image.alignment.y_milli(),
            }),
            transform: Some(AgentImageTransform {
                m11_milli: image.transform.m11_milli,
                m12_milli: image.transform.m12_milli,
                m21_milli: image.transform.m21_milli,
                m22_milli: image.transform.m22_milli,
                tx_milli: image.transform.tx_milli,
                ty_milli: image.transform.ty_milli,
            }),
            authored_placement: source.and_then(|source| source.placement),
            resolved_placement: image.placement.clone(),
            intrinsic_width: Some(image.frame.width),
            intrinsic_height: Some(image.frame.height),
            actions: source
                .map(|source| source.actions.clone())
                .unwrap_or_default(),
            params: source
                .map(|source| bundle_image_params(&source.params))
                .unwrap_or_default(),
            proxies: source
                .map(|source| bundle_image_proxies(&source.proxies))
                .unwrap_or_default(),
        })),
    })
}

fn bundle_image_params(
    params: &BTreeMap<String, BundleImageObjectParam>,
) -> BTreeMap<String, AgentImageObjectParam> {
    params
        .iter()
        .map(|(key, value)| (key.clone(), bundle_image_param(value)))
        .collect()
}

fn bundle_image_param(value: &BundleImageObjectParam) -> AgentImageObjectParam {
    match value {
        BundleImageObjectParam::Bool { value } => AgentImageObjectParam::Bool { value: *value },
        BundleImageObjectParam::Integer { value } => {
            AgentImageObjectParam::Integer { value: *value }
        }
        BundleImageObjectParam::Milli { value } => AgentImageObjectParam::Milli { value: *value },
        BundleImageObjectParam::Text { value } => AgentImageObjectParam::Text {
            value: value.clone(),
        },
        BundleImageObjectParam::Id { value } => AgentImageObjectParam::Id {
            value: value.clone(),
        },
    }
}

fn bundle_image_proxies(
    proxies: &[BundleImageObjectProxy],
) -> Vec<AgentPresentationObjectProxyRef> {
    proxies
        .iter()
        .map(|proxy| AgentPresentationObjectProxyRef {
            id: proxy.id.clone(),
            type_name: proxy.type_name.clone(),
            role: proxy.role.clone(),
            layer: proxy.layer.clone(),
            depth: proxy.depth_milli,
            declaration: None,
            hit_test: proxy.hit_test,
            params: proxy
                .params
                .iter()
                .map(|(key, value)| (key.clone(), bundle_image_proxy_param(value)))
                .collect(),
        })
        .collect()
}

fn bundle_image_proxy_param(value: &BundleImageObjectParam) -> RichTextParam {
    match value {
        BundleImageObjectParam::Bool { value } => RichTextParam::Bool { value: *value },
        BundleImageObjectParam::Integer { value } => RichTextParam::Int { value: *value },
        BundleImageObjectParam::Milli { value } => RichTextParam::Milli {
            value: Milli(*value),
        },
        BundleImageObjectParam::Text { value } => RichTextParam::Text {
            value: value.clone(),
        },
        BundleImageObjectParam::Id { value } => RichTextParam::Selector {
            value: value.clone(),
        },
    }
}

fn player_render_image_native_quad(
    image: &RenderImage,
) -> Option<arcweft_render_native::NativeImageQuad<'_>> {
    let quad = image.visible_quad()?;
    let transform = image.transform_matrix();
    Some(arcweft_render_native::NativeImageQuad {
        width: image.frame.width,
        height: image.frame.height,
        rgba: &image.frame.rgba,
        opacity_milli: image.opacity_milli,
        dst: arcweft_render_native::NativeImageRect {
            x: quad.rect.x,
            y: quad.rect.y,
            width: quad.rect.width,
            height: quad.rect.height,
        },
        transform: arcweft_render_native::NativeImageTransform {
            m11: transform.m11,
            m12: transform.m12,
            m21: transform.m21,
            m22: transform.m22,
            tx: transform.tx,
            ty: transform.ty,
        },
    })
}

fn player_agent_image_fit(fit: ImageObjectFit) -> AgentImageFit {
    match fit {
        ImageObjectFit::Contain => AgentImageFit::Contain,
        ImageObjectFit::Cover => AgentImageFit::Cover,
        ImageObjectFit::Stretch => AgentImageFit::Stretch,
        ImageObjectFit::Intrinsic => AgentImageFit::Intrinsic,
    }
}

fn player_semantic_object(
    step: usize,
    node: &arcweft_presentation::semantic::SemanticNode,
    control: Option<&RenderTextInputControl>,
    view_by_target: &BTreeMap<String, String>,
) -> Option<AgentObservedObject> {
    if !node.visible() {
        return None;
    }
    let id = node.target().id().as_str().to_owned();
    let bbox = agent_bbox_from_hit_rect(node.bounds())?;
    let text = player_semantic_text(node.role(), node.label(), control);
    let mut object = AgentObservedObject {
        id: id.clone(),
        parent_id: view_by_target.get(&id).cloned(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_agent_protocol::action::{AgentActionDispatch, AgentActionKind};
    use arcweft_bundle::resource_codec::view::{
        CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewInputKind,
        ViewInputPurpose, ViewSecureInputPolicy, ViewTextSelectionPolicy, ViewTextShortcutPolicy,
        ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
    };
    use arcweft_bundle::resource_codec::{
        ViewRuntimeActionButton, ViewRuntimeActionButtonAction, ViewRuntimeButtonBounds,
        ViewRuntimeControlStyle, ViewRuntimeTextControl, ViewRuntimeTextControlBounds,
        ViewRuntimeTextControlHandlers, ViewRuntimeTextControlOptions, ViewRuntimeTextSelection,
    };
    use arcweft_bundle::{
        BundleImageObjectBounds, BundleImageObjectFit, BundleImageObjectPlayback,
        BundleImageObjectTransform,
    };
    use arcweft_id::PublicId;
    use arcweft_presentation::image::{ImageObjectAlignment, ImageObjectTransform};
    use arcweft_presentation::input::InteractionTarget;
    use arcweft_presentation::layer::LayerId;
    use arcweft_presentation::semantic::{SemanticNode, SemanticTree};
    use arcweft_render_wgpu::geometry::RenderImageFrame;

    #[test]
    fn player_semantic_objects_preserve_runtime_view_parent() {
        let presentation = BundlePresentationSnapshot {
            text_inputs: vec![runtime_text_control("input.visitor_name")],
            action_buttons: vec![runtime_action_button("button.continue")],
            ..BundlePresentationSnapshot::default()
        };
        let view_by_target = player_runtime_view_by_target(&presentation);
        let input_target = interaction_target("input.visitor_name");
        let input_node = SemanticNode::new(
            layer_id("view.text_input"),
            input_target.clone(),
            SemanticRole::TextField,
            HitRect::new(48.0, 48.0, 420.0, 48.0),
        );
        let render_control = RenderTextInputControl::new(
            input_target,
            arcweft_presentation::text_input::TextInputSessionId(41),
            "Ada",
            arcweft_presentation::text_input::TextRange::new(
                arcweft_presentation::text_input::TextByteOffset(3),
                arcweft_presentation::text_input::TextByteOffset(3),
            ),
            arcweft_presentation::text_input::TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(48.0, 48.0, 420.0, 48.0),
        );
        let button_node = SemanticNode::new(
            layer_id("view.button"),
            interaction_target("button.continue"),
            SemanticRole::Button,
            HitRect::new(484.0, 48.0, 180.0, 48.0),
        );

        let input = player_semantic_object(7, &input_node, Some(&render_control), &view_by_target)
            .expect("input object");
        let button =
            player_semantic_object(7, &button_node, None, &view_by_target).expect("button object");

        assert_eq!(input.parent_id.as_deref(), Some("view.ModernFeedbackPanel"));
        assert_eq!(
            button.parent_id.as_deref(),
            Some("view.ModernFeedbackPanel")
        );
    }

    #[test]
    fn player_semantic_actions_become_agent_action_targets() {
        let mut semantics = SemanticTree::default();
        semantics.push(
            SemanticNode::new(
                layer_id("view.button"),
                interaction_target("button.continue"),
                SemanticRole::Button,
                HitRect::new(484.0, 48.0, 180.0, 48.0),
            )
            .with_action(PublicId::try_new("action.feedback.submit_name").unwrap()),
        );

        let actions = agent_action_targets_for_semantics(&semantics);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "action.feedback.submit_name");
        assert_eq!(actions[0].target, "button.continue");
        assert_eq!(actions[0].action, AgentActionKind::Invoke);
        assert_eq!(actions[0].kind, AgentActionDispatch::Semantic);
        assert!(actions[0].enabled);
    }

    #[test]
    fn missing_requested_capture_scopes_report_structured_diagnostics() {
        let mut diagnostics = Vec::new();

        push_missing_capture_scope_diagnostics(
            &mut diagnostics,
            9,
            [
                Some(RequestedCaptureScope {
                    kind: RequestedCaptureScopeKind::View,
                    id: "view.HiddenPanel",
                }),
                Some(RequestedCaptureScope {
                    kind: RequestedCaptureScopeKind::Object,
                    id: "button.hidden",
                }),
                None,
            ],
            &[],
            &[],
            &[],
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.step == 9
                && diagnostic.severity == AgentDiagnosticSeverity::Error
                && diagnostic.source.as_deref() == Some("agent.observe")
                && diagnostic.code.as_deref() == Some("AGENT_CAPTURE_MISSING_SCOPE")
        }));
        assert!(diagnostics[0].message.contains("view.HiddenPanel"));
        assert!(diagnostics[1].message.contains("button.hidden"));
    }

    #[test]
    fn player_image_object_observation_skips_hidden_source_and_frame() {
        let viewport = AgentViewport {
            width: 1280,
            height: 720,
            scale: 1.0,
        };
        let render_image = render_image("image.glass_bg");
        let visible_source = bundle_image_object("image.glass_bg", true);
        let hidden_source = bundle_image_object("image.glass_bg", false);
        let mut visible_frames = AgentImageFrameStore::default();
        let mut hidden_frames = AgentImageFrameStore::default();

        let visible = player_observed_image_object(
            3,
            &viewport,
            &render_image,
            Some(&visible_source),
            &mut visible_frames,
            125,
        )
        .expect("visible image object");
        let hidden = player_observed_image_object(
            3,
            &viewport,
            &render_image,
            Some(&hidden_source),
            &mut hidden_frames,
            125,
        );

        assert_eq!(visible.id, "object.image.image.glass_bg");
        assert!(visible_frames.get(&visible.id).is_some());
        assert!(hidden.is_none());
        assert!(hidden_frames.get("object.image.image.glass_bg").is_none());
    }

    #[test]
    fn player_image_object_observation_uses_scroll_clipped_visible_quad() {
        let viewport = AgentViewport {
            width: 1280,
            height: 720,
            scale: 1.0,
        };
        let mut render_image = render_image("image.glass_bg");
        render_image.fit = ImageObjectFit::Stretch;
        render_image.bounds = HitRect::new(100.0, 170.0, 200.0, 80.0);
        render_image.viewport_clip = Some(HitRect::new(100.0, 100.0, 160.0, 80.0));
        let source = bundle_image_object("image.glass_bg", true);
        let mut frames = AgentImageFrameStore::default();

        let object = player_observed_image_object(
            3,
            &viewport,
            &render_image,
            Some(&source),
            &mut frames,
            125,
        )
        .expect("visible image object");

        assert_eq!(object.bbox.x, 100);
        assert_eq!(object.bbox.y, 170);
        assert_eq!(object.bbox.width, 160);
        assert_eq!(object.bbox.height, 10);
        let frame = frames.get(&object.id).expect("image frame stored");
        let placement = frame.placement.as_ref().expect("placement stored");
        assert!((placement.dst.x - 100.0).abs() < f32::EPSILON);
        assert!((placement.dst.y - 170.0).abs() < f32::EPSILON);
        assert!((placement.dst.width - 160.0).abs() < f32::EPSILON);
        assert!((placement.dst.height - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn hidden_image_object_capture_scope_reports_missing_scope_diagnostic() {
        let viewport = AgentViewport {
            width: 1280,
            height: 720,
            scale: 1.0,
        };
        let render_image = render_image("image.glass_bg");
        let hidden_source = bundle_image_object("image.glass_bg", false);
        let mut hidden_frames = AgentImageFrameStore::default();

        let objects = player_observed_image_object(
            4,
            &viewport,
            &render_image,
            Some(&hidden_source),
            &mut hidden_frames,
            125,
        )
        .into_iter()
        .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        push_missing_capture_scope_diagnostics(
            &mut diagnostics,
            4,
            [
                None,
                Some(RequestedCaptureScope {
                    kind: RequestedCaptureScopeKind::Object,
                    id: "object.image.image.glass_bg",
                }),
                None,
            ],
            &[],
            &[],
            &objects,
        );

        assert!(objects.is_empty());
        assert!(hidden_frames.get("object.image.image.glass_bg").is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("AGENT_CAPTURE_MISSING_SCOPE")
        );
        assert!(
            diagnostics[0]
                .message
                .contains("object.image.image.glass_bg")
        );
    }

    #[test]
    fn released_image_object_capture_scope_reports_missing_scope_diagnostic() {
        let objects = Vec::new();
        let frames = AgentImageFrameStore::default();
        let mut diagnostics = Vec::new();

        push_missing_capture_scope_diagnostics(
            &mut diagnostics,
            5,
            [
                None,
                Some(RequestedCaptureScope {
                    kind: RequestedCaptureScopeKind::Object,
                    id: "object.image.image.glass_bg",
                }),
                None,
            ],
            &[],
            &[],
            &objects,
        );

        assert!(frames.get("object.image.image.glass_bg").is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].step, 5);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("AGENT_CAPTURE_MISSING_SCOPE")
        );
        assert!(
            diagnostics[0]
                .message
                .contains("object.image.image.glass_bg")
        );
    }

    fn runtime_text_control(public_id: &str) -> ViewRuntimeTextControl {
        ViewRuntimeTextControl {
            public_id: public_id.to_owned(),
            target: public_id.to_owned(),
            view: Some("view.ModernFeedbackPanel".to_owned()),
            containing_scroll_region: None,
            session: 41,
            value: String::new(),
            selection: ViewRuntimeTextSelection::new(0, 0),
            options: ViewRuntimeTextControlOptions {
                purpose: ViewInputPurpose::Text,
                autocorrect: TextAssistPolicy::PlatformDefault,
                spellcheck: TextAssistPolicy::PlatformDefault,
                capitalization: TextCapitalization::None,
                enter_key: EnterKeyHint::Default,
                multiline: false,
                selection_policy: ViewTextSelectionPolicy::Enabled,
                shortcut_policy: ViewTextShortcutPolicy::Enabled,
                tab_policy: ViewTextTabPolicy::FocusNavigation,
                vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
                secure_policy: ViewSecureInputPolicy::Plain,
                composition_on_blur: CompositionOnBlurPolicy::Commit,
            },
            kind: ViewInputKind::TextField,
            bounds: ViewRuntimeTextControlBounds::from_px(48, 48, 420, 48),
            label: None,
            handlers: ViewRuntimeTextControlHandlers::default(),
            style: ViewRuntimeControlStyle::default(),
        }
    }

    fn runtime_action_button(public_id: &str) -> ViewRuntimeActionButton {
        ViewRuntimeActionButton {
            public_id: public_id.to_owned(),
            target: public_id.to_owned(),
            view: Some("view.ModernFeedbackPanel".to_owned()),
            containing_scroll_region: None,
            label: "Continue".to_owned(),
            enabled: true,
            bounds: ViewRuntimeButtonBounds::new(484_000, 48_000, 180_000, 48_000),
            action: ViewRuntimeActionButtonAction::Noop,
            style: ViewRuntimeControlStyle::default(),
        }
    }

    fn interaction_target(public_id: &str) -> InteractionTarget {
        InteractionTarget::new(PublicId::try_new(public_id).expect("valid test target id"))
    }

    fn layer_id(public_id: &str) -> LayerId {
        LayerId::new(PublicId::try_new(public_id).expect("valid test layer id"))
    }

    fn render_image(id: &str) -> RenderImage {
        RenderImage {
            id: id.to_owned(),
            frame: RenderImageFrame {
                index: None,
                width: 2,
                height: 1,
                rgba: vec![10, 20, 30, 255, 40, 50, 60, 255],
            },
            bounds: HitRect::new(0.0, 0.0, 1280.0, 720.0),
            containing_scroll_region: None,
            viewport_clip: None,
            placement: None,
            fit: ImageObjectFit::Cover,
            alignment: ImageObjectAlignment::top_left(),
            transform: ImageObjectTransform::identity(),
            opacity_milli: 1_000,
        }
    }

    fn bundle_image_object(id: &str, visible: bool) -> BundleImageObject {
        BundleImageObject {
            id: id.to_owned(),
            asset: "asset.glass_bg".to_owned(),
            target: Some("target.glass_bg".to_owned()),
            layer: Some("layer.background".to_owned()),
            view: None,
            containing_scroll_region: None,
            bounds: BundleImageObjectBounds::from_px(0, 0, 1280, 720),
            placement: None,
            fit: BundleImageObjectFit::Cover,
            alignment: arcweft_bundle::BundleImageObjectAlignment::default(),
            playback: BundleImageObjectPlayback::default(),
            transform: BundleImageObjectTransform::default(),
            depth_milli: -10_000,
            opacity_milli: 1_000,
            actions: Vec::new(),
            params: BTreeMap::default(),
            proxies: Vec::new(),
            visible,
        }
    }
}
