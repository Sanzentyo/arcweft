use super::{
    AgentImageFrameStore, AgentObserveImageKind, AgentObserveOptions, AgentUiImageObservation,
    ExitCode, agent_capture_time_millis, agent_object_capture_refs_for_page,
    agent_observe_capture_time_seconds, agent_observe_effective_steps, agent_textbox_object,
    resolve_source_selection,
};
use crate::app::bundle::compile_bundle_for_selection;
use arcweft_agent_protocol::{
    geometry::{AgentBBox, AgentCoordinateSpace, AgentViewport},
    object::{AgentObservedObject, AgentObservedObjectContent},
};
use arcweft_bundle::{ArcweftBundle, BundleVirtualFileSpace};
use arcweft_player_scene::{
    images::BundleImageCatalog, input::InputController, text_controls::RuntimeTextControlLowerer,
};
use arcweft_presentation::{hit::HitRect, semantic::SemanticRole};
use arcweft_render_wgpu::{
    geometry::{
        PreparedFrame, RenderChoiceItem, RenderDialogue, RenderPreferences, RenderScene,
        RenderTextInputControl, RenderViewport, SharedFramePlanner,
    },
    offscreen::SharedOffscreenCapture,
};
use arcweft_runtime_driver::{
    clock::RuntimeClockStep,
    display::BundlePresentationSnapshot,
    session::{BundleSession, BundleSessionOptions, BundleSessionStep, BundleStepInput},
};
use std::path::Path;

pub(super) fn agent_player_visual_observation_for_options(
    options: &AgentObserveOptions,
) -> Result<AgentUiImageObservation, ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let mut phases = Vec::new();
    let compiled =
        compile_bundle_for_selection(&selection, vec![BundleVirtualFileSpace::Asset], &mut phases)?;
    let bundle = compiled.bundle;
    let step = run_player_bundle_observation(&bundle, options)?;
    let viewport = player_observe_viewport(options);
    let visual_time_millis = u64::from(agent_capture_time_millis(
        agent_observe_capture_time_seconds(options),
    ));
    let images = BundleImageCatalog::from_bundle(&bundle).map_err(|error| {
        eprintln!("error: player-backed observe image catalog failed: {error}");
        ExitCode::FAILURE
    })?;
    let mut input = InputController::default();
    let scene = player_observe_render_scene(
        &images,
        &step.presentation,
        viewport,
        visual_time_millis,
        &mut input,
    )?;
    let prepared = SharedFramePlanner::prepare(&scene).map_err(|error| {
        eprintln!("error: player-backed observe frame planning failed: {error}");
        ExitCode::FAILURE
    })?;
    input.ensure_choice_focus(&prepared);
    let scene = player_observe_render_scene(
        &images,
        &step.presentation,
        viewport,
        visual_time_millis,
        &mut input,
    )?;
    let prepared = SharedFramePlanner::prepare(&RenderScene {
        interaction: input.visual_state(),
        choice_scroll: input.choice_scroll(),
        ..scene.clone()
    })
    .map_err(|error| {
        eprintln!("error: player-backed observe frame planning failed: {error}");
        ExitCode::FAILURE
    })?;
    let objects =
        player_observed_objects(&scene, &prepared, &step.presentation, step.index, options);
    let mut image_frames = AgentImageFrameStore::default();
    if player_observe_requires_shared_capture(options) {
        let capture = player_observe_capture_frame(&prepared)?;
        image_frames.set_full_frame(capture.width, capture.height, capture.rgba);
    }
    Ok(AgentUiImageObservation {
        objects,
        image_frames,
    })
}

fn run_player_bundle_observation(
    bundle: &ArcweftBundle,
    options: &AgentObserveOptions,
) -> Result<BundleSessionStep, ExitCode> {
    let mut session = BundleSession::new(
        bundle,
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
    let effective_steps = agent_observe_effective_steps(options);
    let force_capture_step = options.capture_step.is_some();
    let mut last_step = None;
    for step_index in 0..effective_steps {
        let clock = RuntimeClockStep::from_millis(
            u64::try_from(step_index.saturating_add(1)).unwrap_or(u64::MAX),
            16,
        )
        .map_err(|error| {
            eprintln!("error: player-backed observe clock failed: {error}");
            ExitCode::FAILURE
        })?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let finished = step.finished;
        last_step = Some(step);
        if finished && !force_capture_step {
            break;
        }
    }
    last_step.ok_or_else(|| {
        eprintln!("error: player-backed observe requires at least one runtime step");
        ExitCode::from(2)
    })
}

fn player_observe_render_scene(
    images: &BundleImageCatalog,
    presentation: &BundlePresentationSnapshot,
    viewport: RenderViewport,
    visual_time_millis: u64,
    input: &mut InputController,
) -> Result<RenderScene, ExitCode> {
    let text_inputs = RuntimeTextControlLowerer::lower_for_frame(input, &presentation.text_inputs)
        .map_err(|error| {
            eprintln!("error: player-backed observe text-control lowering failed: {error}");
            ExitCode::FAILURE
        })?;
    let images = images
        .render_images(&presentation.images, visual_time_millis)
        .map_err(|error| {
            eprintln!("error: player-backed observe image rendering failed: {error}");
            ExitCode::FAILURE
        })?;
    Ok(RenderScene {
        dialogue: presentation
            .dialogue
            .as_ref()
            .map(RenderDialogue::from_display_frame),
        choices: presentation
            .choices
            .iter()
            .map(|choice| RenderChoiceItem {
                id: choice.id.clone(),
                label: choice.label.clone(),
            })
            .collect(),
        text_inputs,
        images,
        viewport,
        visual_time_millis,
        preferences: RenderPreferences::default(),
        interaction: input.visual_state(),
        choice_scroll: input.choice_scroll(),
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

fn player_observed_objects(
    scene: &RenderScene,
    prepared: &PreparedFrame,
    presentation: &BundlePresentationSnapshot,
    step: usize,
    options: &AgentObserveOptions,
) -> Vec<AgentObservedObject> {
    let viewport = AgentViewport {
        width: scene.viewport.physical_width,
        height: scene.viewport.physical_height,
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
    objects.extend(prepared.semantics.as_slice().iter().filter_map(|node| {
        let control = scene
            .text_inputs
            .iter()
            .find(|control| control.target == *node.target());
        player_semantic_object(step, node, control)
    }));
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
