use crate::clock::LogicalClockQuantizer;
use crate::edit_context::WebEditContextFeatureDetection;
use crate::host::BrowserTaskBroker;
use crate::report::{WebFrameObservationReport, WebObservationReport};
use crate::runtime_text_input::{
    WebPlayerTextInputBridgeHandle, WebRuntimeTextInputFocusReason, WebTextInputClientTransform,
    register_runtime_bridge,
};
use arcweft_bundle::{ArcweftBundle, BundleFormat};
use arcweft_layout::ScalePolicy;
use arcweft_player_scene::fonts::PlayerFontSet;
use arcweft_player_scene::frame::{
    PlayerFrameError, PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest,
};
use arcweft_player_scene::images::{BundleImageCatalog, BundleImageCatalogError};
use arcweft_player_scene::input::{InputController, InputOutcome};
use arcweft_presentation::clipboard::TextClipboardRequest;
use arcweft_presentation::input::{KeyPhase, PointerId, ViewportPoint};
use arcweft_presentation::text_input::TextInputKeyDisposition;
use arcweft_render_web::web::{WebGpuCanvasHost, WebGpuCanvasHostError};
use arcweft_render_wgpu::geometry::{RenderPreferences, RenderViewport};
use arcweft_render_wgpu::renderer::SharedRenderer;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions, BundleStepInput};
use js_sys::{Array, Uint8Array};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use thiserror::Error;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{CustomEvent, CustomEventInit, HtmlCanvasElement};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ButtonSource, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::platform::web::WindowAttributesWeb;
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(Debug, Error)]
enum WebPlayerError {
    #[error("browser window/document is unavailable")]
    MissingDocument,
    #[error("canvas `{0}` was not found")]
    MissingCanvas(String),
    #[error("element `{0}` is not a canvas")]
    NotCanvas(String),
    #[error("Arcweft bundle decode failed: {0}")]
    BundleDecode(String),
    #[error("Arcweft bundle session failed: {0}")]
    Session(String),
    #[error("browser task broker failed: {0}")]
    TaskBroker(String),
    #[error("winit event loop failed: {0}")]
    EventLoop(String),
    #[error("winit window failed: {0}")]
    Window(String),
    #[error("WebGPU initialization/rendering failed: {0}")]
    WebGpu(String),
    #[error("font registration failed: {0}")]
    Font(String),
    #[error("image catalog failed: {0}")]
    Image(String),
    #[error("player frame failed: {0}")]
    PlayerFrame(#[from] PlayerFrameError),
    #[error("diagnostic serialization failed: {0}")]
    Report(String),
    #[error("Web runtime text-input bridge failed: {0}")]
    TextInput(String),
    #[error("player text editor failed: {0}")]
    TextEditor(String),
}

struct ReadyGpu {
    host: WebGpuCanvasHost,
    renderer: SharedRenderer,
}

enum GpuState {
    Uninitialized,
    Loading,
    Ready(ReadyGpu),
    Failed,
}

struct PlayerState {
    canvas: HtmlCanvasElement,
    window: Option<Arc<dyn Window>>,
    gpu: GpuState,
    session: BundleSession,
    broker: BrowserTaskBroker,
    images: BundleImageCatalog,
    input: InputController,
    frame_planner: PlayerFramePlannerState,
    text_input: WebPlayerTextInputBridgeHandle,
    keyboard_modifiers: ModifiersState,
    frame_fit: PlayerFrameFit,
    clock: LogicalClockQuantizer,
    font_set: Option<PlayerFontSet>,
    prepared: Option<arcweft_render_wgpu::geometry::PreparedFrame>,
    dialogue_visual_clock: DialogueVisualClock,
    fatal: Option<String>,
}

struct BrowserViewport {
    render: RenderViewport,
    physical_size: PhysicalSize<u32>,
}

struct BrowserApp {
    state: Rc<RefCell<PlayerState>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebPlayerOptions {
    frame_fit: PlayerFrameFit,
    additional_font_bytes: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DialogueVisualClock {
    line: Option<arcweft_core::plan::RuntimeLineId>,
    started_at_millis: u64,
}

impl Default for WebPlayerOptions {
    fn default() -> Self {
        Self {
            frame_fit: PlayerFrameFit::design_1280x720(ScalePolicy::Contain),
            additional_font_bytes: Vec::new(),
        }
    }
}

impl PlayerState {
    fn browser_viewport(&self, window: &Arc<dyn Window>) -> BrowserViewport {
        let scale_factor = window.scale_factor().max(f64::EPSILON);
        let logical_width = self.canvas.client_width().max(1) as f32;
        let logical_height = self.canvas.client_height().max(1) as f32;
        let physical_width = ((f64::from(logical_width) * scale_factor).round() as u32).max(1);
        let physical_height = ((f64::from(logical_height) * scale_factor).round() as u32).max(1);
        if self.canvas.width() != physical_width {
            self.canvas.set_width(physical_width);
        }
        if self.canvas.height() != physical_height {
            self.canvas.set_height(physical_height);
        }

        BrowserViewport {
            render: RenderViewport {
                logical_width,
                logical_height,
                physical_width,
                physical_height,
                scale_factor,
            },
            physical_size: PhysicalSize::new(physical_width, physical_height),
        }
    }
}

/// Starts the WebGPU-first browser player using already-fetched bundle/font bytes.
/// JavaScript remains a bootstrap only; it does not render game View.
#[wasm_bindgen]
pub fn start_arcweft_player(
    canvas_id: String,
    bundle_bytes: Vec<u8>,
    font_bytes: Vec<u8>,
) -> Result<(), JsValue> {
    start(
        canvas_id,
        bundle_bytes,
        font_bytes,
        WebPlayerOptions::default(),
    )
    .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn start_arcweft_player_with_options(
    canvas_id: String,
    bundle_bytes: Vec<u8>,
    font_bytes: Vec<u8>,
    options: JsValue,
) -> Result<(), JsValue> {
    let options = web_player_options_from_js(&options)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    start(canvas_id, bundle_bytes, font_bytes, options)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn start(
    canvas_id: String,
    bundle_bytes: Vec<u8>,
    font_bytes: Vec<u8>,
    options: WebPlayerOptions,
) -> Result<(), WebPlayerError> {
    let WebPlayerOptions {
        frame_fit,
        additional_font_bytes,
    } = options;
    let mut font_resources = Vec::with_capacity(additional_font_bytes.len().saturating_add(1));
    font_resources.push(font_bytes);
    font_resources.extend(additional_font_bytes);
    let font_set = PlayerFontSet::from_font_resource_bytes(font_resources)
        .map_err(|error| WebPlayerError::Font(error.to_string()))?;

    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or(WebPlayerError::MissingDocument)?;
    let element = document
        .get_element_by_id(&canvas_id)
        .ok_or_else(|| WebPlayerError::MissingCanvas(canvas_id.clone()))?;
    let canvas = element
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| WebPlayerError::NotCanvas(canvas_id.clone()))?;
    let detection = WebEditContextFeatureDetection::detect_for_element(canvas.unchecked_ref());
    let text_input = register_runtime_bridge(canvas_id.clone(), detection);
    let bundle = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bundle_bytes)
        .map_err(|error| WebPlayerError::BundleDecode(error.to_string()))?;
    let session = BundleSession::new(&bundle, BundleSessionOptions::default())
        .map_err(|error| WebPlayerError::Session(error.to_string()))?;
    let broker = BrowserTaskBroker::from_bundle(&bundle)
        .map_err(|error| WebPlayerError::TaskBroker(error.to_string()))?;
    let images = BundleImageCatalog::from_bundle(&bundle)
        .map_err(|error| WebPlayerError::Image(error.to_string()))?;
    let clock = LogicalClockQuantizer::new(16, 4)
        .map_err(|error| WebPlayerError::Session(error.to_string()))?;
    let state = Rc::new(RefCell::new(PlayerState {
        canvas,
        window: None,
        gpu: GpuState::Uninitialized,
        session,
        broker,
        images,
        input: InputController::default(),
        frame_planner: PlayerFramePlannerState::new(),
        text_input,
        keyboard_modifiers: ModifiersState::default(),
        frame_fit,
        clock,
        font_set: Some(font_set),
        prepared: None,
        dialogue_visual_clock: DialogueVisualClock::default(),
        fatal: None,
    }));
    let event_loop =
        EventLoop::new().map_err(|error| WebPlayerError::EventLoop(error.to_string()))?;
    event_loop
        .run_app(BrowserApp { state })
        .map_err(|error| WebPlayerError::EventLoop(error.to_string()))
}

fn web_player_options_from_js(options: &JsValue) -> Result<WebPlayerOptions, WebPlayerError> {
    let mut parsed = WebPlayerOptions::default();
    if let Some(frame_fit) = js_property(options, "frameFit") {
        let fit = js_string_property(&frame_fit, "fit").unwrap_or_else(|| "contain".to_owned());
        let design_width = js_u32_property(&frame_fit, "designWidth")
            .or_else(|| js_u32_property(&frame_fit, "design_width"))
            .unwrap_or(1280);
        let design_height = js_u32_property(&frame_fit, "designHeight")
            .or_else(|| js_u32_property(&frame_fit, "design_height"))
            .unwrap_or(720);
        parsed.frame_fit = match fit.as_str() {
            "raw" | "none" => PlayerFrameFit::raw(),
            "cover" => PlayerFrameFit::design(design_width, design_height, ScalePolicy::Cover),
            "stretch" => PlayerFrameFit::design(design_width, design_height, ScalePolicy::Stretch),
            _ => PlayerFrameFit::design(design_width, design_height, ScalePolicy::Contain),
        };
    }
    parsed.additional_font_bytes = js_u8_array_list_property(options, "additionalFontBytes")?;
    Ok(parsed)
}

fn js_property(parent: &JsValue, key: &str) -> Option<JsValue> {
    let value = js_sys::Reflect::get(parent, &JsValue::from_str(key)).ok()?;
    (!value.is_undefined() && !value.is_null()).then_some(value)
}

fn js_string_property(parent: &JsValue, key: &str) -> Option<String> {
    js_property(parent, key)?.as_string()
}

fn js_u32_property(parent: &JsValue, key: &str) -> Option<u32> {
    let number = js_property(parent, key)?.as_f64()?.round();
    if !number.is_finite() || number < 1.0 {
        return None;
    }
    Some(number.min(f64::from(u32::MAX)) as u32)
}

fn js_u8_array_list_property(parent: &JsValue, key: &str) -> Result<Vec<Vec<u8>>, WebPlayerError> {
    let Some(value) = js_property(parent, key) else {
        return Ok(Vec::new());
    };
    if !Array::is_array(&value) {
        return Err(WebPlayerError::Font(format!(
            "`{key}` must be an array of Uint8Array font resources"
        )));
    }
    let array = Array::from(&value);
    array
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let typed = value.dyn_ref::<Uint8Array>().ok_or_else(|| {
                WebPlayerError::Font(format!("`{key}[{index}]` must be a Uint8Array"))
            })?;
            let mut bytes = vec![0; typed.length() as usize];
            typed.copy_to(&mut bytes);
            Ok(bytes)
        })
        .collect()
}

impl ApplicationHandler for BrowserApp {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let mut state = self.state.borrow_mut();
        if state.window.is_some() {
            return;
        }
        let web_attributes = WindowAttributesWeb::default()
            .with_canvas(Some(state.canvas.clone()))
            .with_append(false)
            .with_focusable(true);
        let attributes = WindowAttributes::default()
            .with_title("Arcweft WebGPU Player")
            .with_platform_attributes(Box::new(web_attributes));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => window,
            Err(error) => {
                set_fatal(&mut state, WebPlayerError::Window(error.to_string()));
                return;
            }
        };
        let window = Arc::<dyn Window>::from(window);
        state.window = Some(Arc::clone(&window));
        state.gpu = GpuState::Loading;
        let shared = Rc::clone(&self.state);
        spawn_local(async move {
            let result = initialize_gpu(Arc::clone(&window), &shared).await;
            if let Err(error) = result {
                let mut state = shared.borrow_mut();
                state.gpu = GpuState::Failed;
                set_fatal(&mut state, error);
            } else {
                emit_event("arcweft-player-ready", "{}".to_owned());
                window.request_redraw();
            }
        });
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let mut state = self.state.borrow_mut();
        let Some(window) = state.window.clone() else {
            return;
        };
        if window.id() != window_id || state.fatal.is_some() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::SurfaceResized(size) => {
                if let GpuState::Ready(gpu) = &mut state.gpu {
                    gpu.host.resize(size);
                }
                if let Err(error) = update_text_input_client_transform(&mut state) {
                    set_fatal(&mut state, error);
                    return;
                }
                window.request_redraw();
            }
            WindowEvent::Focused(focused) => {
                let outcome = state.input.focus_changed(focused);
                let clipboard_requests = apply_outcome(&mut state, outcome);
                schedule_clipboard_requests(Rc::clone(&self.state), clipboard_requests);
                if !focused && let Err(error) = state.text_input.blur_active() {
                    set_fatal(&mut state, WebPlayerError::TextInput(error.to_string()));
                    return;
                }
                window.request_redraw();
            }
            WindowEvent::PointerMoved { position, .. } => {
                let logical = logical_position(position, window.scale_factor());
                if let Some(frame) = state.prepared.clone() {
                    let outcome = state.input.pointer_move(&frame, PointerId(0), logical);
                    let clipboard_requests = apply_outcome(&mut state, outcome);
                    schedule_clipboard_requests(Rc::clone(&self.state), clipboard_requests);
                }
                window.request_redraw();
            }
            WindowEvent::PointerButton {
                state: element_state,
                button,
                position,
                ..
            } if button.clone().mouse_button() == Some(MouseButton::Left)
                || button.clone().mouse_button() == Some(MouseButton::Right)
                || matches!(button, ButtonSource::Touch { .. }) =>
            {
                if let Some(frame) = state.prepared.clone() {
                    let pointer = pointer_id(&button);
                    let position = logical_position(position, window.scale_factor());
                    let modifiers = arcweft_player_scene::input::InputPointerModifiers::new(
                        state.keyboard_modifiers.shift_key(),
                    );
                    let outcome = match element_state {
                        ElementState::Pressed
                            if button.mouse_button() == Some(MouseButton::Right) =>
                        {
                            state
                                .input
                                .pointer_context_menu(&frame, pointer, position, modifiers)
                        }
                        ElementState::Pressed => {
                            let _ = state.canvas.focus();
                            state
                                .input
                                .pointer_down(&frame, pointer, position, modifiers)
                        }
                        ElementState::Released => {
                            state.input.pointer_up(&frame, pointer, position, modifiers)
                        }
                    };
                    let clipboard_requests = apply_outcome(&mut state, outcome);
                    schedule_clipboard_requests(Rc::clone(&self.state), clipboard_requests);
                }
                window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 32.0,
                    MouseScrollDelta::PixelDelta(position) => {
                        (position.y / window.scale_factor()) as f32
                    }
                };
                if let Some(frame) = state.prepared.clone() {
                    let outcome = state.input.wheel(&frame, delta_y);
                    let clipboard_requests = apply_outcome(&mut state, outcome);
                    schedule_clipboard_requests(Rc::clone(&self.state), clipboard_requests);
                }
                window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                state.keyboard_modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let key = key_label(&event.logical_key);
                let phase = match event.state {
                    ElementState::Pressed => KeyPhase::Down,
                    ElementState::Released => KeyPhase::Up,
                };
                if let Some(frame) = state.prepared.clone() {
                    let mut clipboard_requests = Vec::new();
                    let text_input_changed = match drain_text_input_edits(&mut state, &frame) {
                        Ok((changed, requests)) => {
                            clipboard_requests.extend(requests);
                            changed
                        }
                        Err(error) => {
                            set_fatal(&mut state, error);
                            return;
                        }
                    };
                    if state.fatal.is_some() {
                        return;
                    }
                    if phase == KeyPhase::Down && text_input_changed {
                        schedule_clipboard_requests(Rc::clone(&self.state), clipboard_requests);
                        window.request_redraw();
                        return;
                    }
                    let disposition = state
                        .text_input
                        .key_disposition()
                        .unwrap_or(TextInputKeyDisposition::ShortcutCandidate);
                    let shift_pressed = state.keyboard_modifiers.shift_key();
                    let outcome = state.input.keyboard_with_modifiers_and_ime(
                        &frame,
                        &key,
                        phase,
                        shift_pressed,
                        disposition,
                    );
                    clipboard_requests.extend(apply_outcome(&mut state, outcome));
                    schedule_clipboard_requests(Rc::clone(&self.state), clipboard_requests);
                }
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => match redraw(&mut state, &window) {
                Ok(clipboard_requests) => {
                    schedule_clipboard_requests(Rc::clone(&self.state), clipboard_requests);
                }
                Err(error) => {
                    set_fatal(&mut state, error);
                }
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &dyn ActiveEventLoop) {
        if let Some(window) = self.state.borrow().window.as_ref() {
            window.request_redraw();
        }
    }
}

async fn initialize_gpu(
    window: Arc<dyn Window>,
    state: &Rc<RefCell<PlayerState>>,
) -> Result<(), WebPlayerError> {
    let host = WebGpuCanvasHost::new(window)
        .await
        .map_err(|error| WebPlayerError::WebGpu(error.to_string()))?;
    let mut renderer = SharedRenderer::new(host.device(), host.queue(), host.format());
    let font_set = state
        .borrow_mut()
        .font_set
        .take()
        .ok_or_else(|| WebPlayerError::Font("font set was already consumed".to_owned()))?;
    let mut state = state.borrow_mut();
    font_set
        .register_with_renderer_and_planner(&mut renderer, &mut state.frame_planner)
        .map_err(|error| WebPlayerError::Font(error.to_string()))?;
    state.gpu = GpuState::Ready(ReadyGpu { host, renderer });
    Ok(())
}

fn redraw(
    state: &mut PlayerState,
    window: &Arc<dyn Window>,
) -> Result<Vec<TextClipboardRequest>, WebPlayerError> {
    let mut clipboard_requests = Vec::new();
    let host_millis = now_millis();
    for clock in state
        .clock
        .advance(host_millis)
        .map_err(|error| WebPlayerError::Session(error.to_string()))?
    {
        let task_events = state.broker.drain_queued_task_events();
        let step = state.session.step_with_clock(
            clock,
            BundleStepInput {
                task_events,
                ..BundleStepInput::default()
            },
        );
        state.broker.cancel_scopes(step.cancel_scopes.clone());
        state.broker.close_sources(step.source_close.clone());
        let queued = state.broker.queue_dispatches(step.requested_tasks.clone());
        let report = WebObservationReport::from_step(&step, queued);
        let json = serde_json::to_string(&report)
            .map_err(|error| WebPlayerError::Report(error.to_string()))?;
        emit_event("arcweft-runtime-observation", json);
    }

    let browser_viewport = state.browser_viewport(window);
    let viewport = browser_viewport.render;
    let host_millis_u64 = host_millis.max(0.0) as u64;
    let mut prepared = prepare_web_player_frame(state, viewport, host_millis_u64)?;
    update_text_input_client_transform(state)?;
    state
        .text_input
        .sync_prepared_frame(&prepared, WebRuntimeTextInputFocusReason::RedrawRefresh)
        .map_err(|error| WebPlayerError::TextInput(error.to_string()))?;
    let (changed, requests) = drain_text_input_edits(state, &prepared)?;
    clipboard_requests.extend(requests);
    if changed {
        prepared = prepare_web_player_frame(state, viewport, host_millis_u64)?;
        state
            .text_input
            .sync_prepared_frame(&prepared, WebRuntimeTextInputFocusReason::RedrawRefresh)
            .map_err(|error| WebPlayerError::TextInput(error.to_string()))?;
    }

    let GpuState::Ready(gpu) = &mut state.gpu else {
        return Ok(clipboard_requests);
    };
    let paragraph_evidence = gpu
        .renderer
        .frame_styled_paragraph_layout_evidence(&prepared);
    let frame_report =
        WebFrameObservationReport::from_prepared_frame(&prepared, &paragraph_evidence)
            .map_err(|error| WebPlayerError::Report(error.to_string()))?;
    let frame_json = serde_json::to_string(&frame_report)
        .map_err(|error| WebPlayerError::Report(error.to_string()))?;
    emit_event("arcweft-frame-observation", frame_json);

    gpu.host.resize(browser_viewport.physical_size);
    let health = gpu.host.health();
    if let Some(error) = health.device_lost.or(health.uncaptured_error) {
        return Err(WebPlayerError::WebGpu(error));
    }
    match gpu.host.render_and_present(&mut gpu.renderer, &prepared) {
        Ok(()) => {}
        Err(WebGpuCanvasHostError::SurfaceLost | WebGpuCanvasHostError::SurfaceOutdated) => {
            gpu.host.reconfigure();
            window.request_redraw();
        }
        Err(error) => return Err(WebPlayerError::WebGpu(error.to_string())),
    }
    state.prepared = Some(prepared);
    Ok(clipboard_requests)
}

fn prepare_web_player_frame(
    state: &mut PlayerState,
    viewport: RenderViewport,
    host_millis: u64,
) -> Result<arcweft_render_wgpu::geometry::PreparedFrame, WebPlayerError> {
    let presentation = state.session.presentation();
    let visual_time_millis = dialogue_visual_time_millis(
        &mut state.dialogue_visual_clock,
        presentation.dialogue.as_ref(),
        host_millis,
    );
    Ok(state
        .frame_planner
        .prepare(
            &mut state.input,
            PlayerFrameRequest {
                presentation,
                images: &state.images,
                viewport,
                fit: state.frame_fit,
                image_time_millis: host_millis,
                visual_time_millis,
                preferences: RenderPreferences::default(),
            },
        )?
        .frame)
}

fn dialogue_visual_time_millis(
    clock: &mut DialogueVisualClock,
    dialogue: Option<&arcweft_render_text::LineDisplayFrame>,
    now_millis: u64,
) -> u64 {
    let Some(dialogue) = dialogue else {
        clock.line = None;
        clock.started_at_millis = now_millis;
        return 0;
    };
    if clock.line.as_ref() != Some(&dialogue.line) {
        clock.line = Some(dialogue.line.clone());
        clock.started_at_millis = now_millis;
    }
    dialogue_visual_time_override_millis()
        .unwrap_or_else(|| now_millis.saturating_sub(clock.started_at_millis))
}

fn dialogue_visual_time_override_millis() -> Option<u64> {
    let window = web_sys::window()?;
    let value = js_sys::Reflect::get(
        &window,
        &JsValue::from_str("__arcweftDialogueVisualTimeMillis"),
    )
    .ok()?;
    let millis = if value.is_function() {
        js_sys::Function::from(value)
            .call0(&JsValue::NULL)
            .ok()?
            .as_f64()?
    } else {
        value.as_f64()?
    };
    millis.is_finite().then(|| millis.max(0.0) as u64)
}

impl From<BundleImageCatalogError> for WebPlayerError {
    fn from(error: BundleImageCatalogError) -> Self {
        Self::Image(error.to_string())
    }
}

fn apply_outcome(state: &mut PlayerState, outcome: InputOutcome) -> Vec<TextClipboardRequest> {
    let InputOutcome {
        actions,
        text_control_write_backs,
        clipboard_requests,
        diagnostics: _,
        dialogue_advance,
        cancel: _,
        redraw: _,
    } = outcome;
    if dialogue_advance {
        state.session.queue_dialogue_advance();
    }
    for action in actions {
        if let Err(error) = state.session.queue_semantic_action(&action) {
            set_fatal(state, WebPlayerError::Session(error.to_string()));
            break;
        }
    }
    if let Err(error) = state
        .session
        .queue_text_control_write_backs(text_control_write_backs)
    {
        set_fatal(state, WebPlayerError::Session(error.to_string()));
    }
    clipboard_requests
}

fn schedule_clipboard_requests(
    state: Rc<RefCell<PlayerState>>,
    clipboard_requests: Vec<TextClipboardRequest>,
) {
    if clipboard_requests.is_empty() {
        return;
    }
    spawn_local(async move {
        let mut pending = VecDeque::from(clipboard_requests);
        while let Some(request) = pending.pop_front() {
            let host_outcome = crate::clipboard::apply_clipboard_request(request).await;
            let nested = {
                let mut state = state.borrow_mut();
                if let Some(frame) = state.prepared.clone() {
                    match state.input.apply_clipboard_outcome(&frame, host_outcome) {
                        Ok(outcome) => apply_outcome(&mut state, outcome),
                        Err(error) => {
                            set_fatal(&mut state, WebPlayerError::TextEditor(error.to_string()));
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                }
            };
            pending.extend(nested);
            if let Some(window) = state.borrow().window.as_ref() {
                window.request_redraw();
            }
            if state.borrow().fatal.is_some() {
                break;
            }
        }
    });
}

fn update_text_input_client_transform(state: &mut PlayerState) -> Result<(), WebPlayerError> {
    let rect = state.canvas.get_bounding_client_rect();
    state
        .text_input
        .set_client_transform(WebTextInputClientTransform::new(
            rect.left() as f32,
            rect.top() as f32,
        ))
        .map_err(|error| WebPlayerError::TextInput(error.to_string()))
}

fn drain_text_input_edits(
    state: &mut PlayerState,
    frame: &arcweft_render_wgpu::geometry::PreparedFrame,
) -> Result<(bool, Vec<TextClipboardRequest>), WebPlayerError> {
    let edits = state
        .text_input
        .drain_pending_edits()
        .map_err(|error| WebPlayerError::TextInput(error.to_string()))?;
    let changed = !edits.is_empty();
    let mut clipboard_requests = Vec::new();
    for edit in edits {
        let outcome = state
            .input
            .text_input(frame, edit.into_input())
            .map_err(|error| WebPlayerError::TextEditor(error.to_string()))?;
        clipboard_requests.extend(apply_outcome(state, outcome));
    }
    Ok((changed, clipboard_requests))
}

fn logical_position(position: PhysicalPosition<f64>, scale_factor: f64) -> ViewportPoint {
    ViewportPoint::new(
        (position.x / scale_factor) as f32,
        (position.y / scale_factor) as f32,
    )
}

fn pointer_id(button: &ButtonSource) -> PointerId {
    match button {
        ButtonSource::Touch { finger_id, .. } => PointerId(finger_id.into_raw() as u64 + 10),
        ButtonSource::Mouse(_) | ButtonSource::TabletTool { .. } | ButtonSource::Unknown(_) => {
            PointerId(0)
        }
    }
}

fn key_label(key: &Key) -> String {
    match key {
        Key::Named(NamedKey::ArrowUp) => "ArrowUp".to_owned(),
        Key::Named(NamedKey::ArrowDown) => "ArrowDown".to_owned(),
        Key::Named(NamedKey::ArrowLeft) => "ArrowLeft".to_owned(),
        Key::Named(NamedKey::ArrowRight) => "ArrowRight".to_owned(),
        Key::Named(NamedKey::Enter) => "Enter".to_owned(),
        Key::Named(NamedKey::Home) => "Home".to_owned(),
        Key::Named(NamedKey::End) => "End".to_owned(),
        Key::Named(NamedKey::Tab) => "Tab".to_owned(),
        Key::Character(value) if value == " " => "Space".to_owned(),
        Key::Character(value) => value.to_string(),
        _ => format!("{key:?}"),
    }
}

fn now_millis() -> f64 {
    let Some(window) = web_sys::window() else {
        return js_sys::Date::now();
    };
    if let Ok(now_hook) = js_sys::Reflect::get(&window, &JsValue::from_str("__arcweftNowMillis"))
        && now_hook.is_function()
        && let Ok(value) = js_sys::Function::from(now_hook).call0(&JsValue::NULL)
        && let Some(millis) = value.as_f64()
    {
        return millis;
    }
    window
        .performance()
        .map_or_else(js_sys::Date::now, |performance| performance.now())
}

fn emit_event(name: &str, detail: String) {
    let init = CustomEventInit::new();
    init.set_detail(&JsValue::from_str(&detail));
    if let Ok(event) = CustomEvent::new_with_event_init_dict(name, &init)
        && let Some(document) = web_sys::window().and_then(|window| window.document())
    {
        let _ = document.dispatch_event(&event);
    }
}

fn set_fatal(state: &mut PlayerState, error: WebPlayerError) {
    let message = error.to_string();
    state.fatal = Some(message.clone());
    emit_event("arcweft-player-fatal", message);
}
