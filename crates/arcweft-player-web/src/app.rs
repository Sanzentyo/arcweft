use self::environment::WebEnvironmentError;
use self::registry::WebPlayerControl;
use crate::clock::LogicalClockQuantizer;
use crate::host::BrowserTaskBroker;
use crate::report::{WebFrameObservationReport, WebObservationReport};
use crate::runtime_text_input::{
    WebPlayerTextInputBridgeHandle, WebRuntimeTextInputFocusReason, WebTextInputClientTransform,
};
use arcweft_player_scene::dialogue::DialogueVisualClock;
use arcweft_player_scene::fonts::PlayerFontSet;
use arcweft_player_scene::frame::{
    PlayerFrameError, PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest,
    PlayerPreparedFrame, PlayerPreparedFrameCandidate, ViewGeometryConsumer,
    ViewGeometryConversionError, ViewGeometryConversionField, ViewGeometryPlatform,
    ViewGeometryRuntimeError,
};
use arcweft_player_scene::images::{BundleImageCatalog, BundleImageCatalogError};
use arcweft_player_scene::input::wheel::{
    WheelDelta, WheelNormalizationError, WheelNormalizationPolicy,
};
use arcweft_player_scene::input::{DialogueProgress, InputController, InputOutcome};
use arcweft_presentation::clipboard::TextClipboardRequest;
use arcweft_presentation::input::{KeyPhase, PointerId, ViewportPoint};
use arcweft_presentation::text_input::TextInputKeyDisposition;
use arcweft_render_web::web::{
    WebGpuCanvasFrameCandidate, WebGpuCanvasHost, WebGpuCanvasHostError,
};
use arcweft_render_wgpu::geometry::view_final::PreparedViewRenderCandidate;
use arcweft_render_wgpu::geometry::{RenderPreferences, RenderViewport};
use arcweft_render_wgpu::renderer::SharedRenderer;
use arcweft_runtime_driver::session::{BundleSession, BundleStepInput};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use thiserror::Error;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{CustomEvent, CustomEventInit, HtmlCanvasElement};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ButtonSource, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::platform::web::WindowAttributesWeb;
use winit::window::{Window, WindowAttributes, WindowId};

mod environment;
mod event_loop;
mod handle;
mod registry;

pub use handle::{
    ArcweftWebPlayerHandle, arcweft_player_handle, create_arcweft_player,
    create_arcweft_player_with_options, start_arcweft_player, start_arcweft_player_with_options,
    stop_arcweft_player,
};

#[derive(Debug, Error)]
enum WebPlayerError {
    #[error(transparent)]
    Environment(#[from] WebEnvironmentError),
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
    #[error("Web viewport geometry conversion failed: {0}")]
    GeometryConversion(#[from] ViewGeometryConversionError),
    #[error("font registration failed: {0}")]
    Font(String),
    #[error("image catalog failed: {0}")]
    Image(String),
    #[error("player frame failed after registering {registered_font_bytes} font bytes: {source}")]
    PlayerFrame {
        registered_font_bytes: usize,
        #[source]
        source: PlayerFrameError,
    },
    #[error("diagnostic serialization failed: {0}")]
    Report(String),
    #[error("Web runtime text-input bridge failed: {0}")]
    TextInput(String),
    #[error("player text editor failed: {0}")]
    TextEditor(String),
    #[error("platform wheel input normalization failed: {0}")]
    WheelNormalization(#[from] WheelNormalizationError),
}

impl WebPlayerError {
    fn into_js_value(self) -> JsValue {
        match self {
            Self::Environment(error) => error.into_js_value(),
            error => JsValue::from_str(&error.to_string()),
        }
    }
}

struct ReadyGpu {
    host: WebGpuCanvasHost,
    renderer: SharedRenderer,
}

struct WebFramePublicationCandidate {
    view_render: PreparedViewRenderCandidate,
    canvas: WebGpuCanvasFrameCandidate,
}

impl WebFramePublicationCandidate {
    fn prepare(
        gpu: &mut ReadyGpu,
        candidate: &PlayerPreparedFrameCandidate,
        view_render: PreparedViewRenderCandidate,
    ) -> Result<Self, WebGpuCanvasHostError> {
        let canvas = gpu
            .host
            .prepare_frame(&mut gpu.renderer, candidate.prepared())?;
        Ok(Self {
            view_render,
            canvas,
        })
    }

    fn commit(self, gpu: &ReadyGpu, frame: &PlayerPreparedFrame) {
        debug_assert_eq!(
            self.view_render.generation(),
            frame.view_geometry().generation().value()
        );
        gpu.host.commit_frame(self.canvas);
    }
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

pub(super) struct BrowserApp;

impl PlayerState {
    fn browser_viewport(
        &self,
        window: &Arc<dyn Window>,
    ) -> Result<BrowserViewport, WebPlayerError> {
        let scale_factor = window.scale_factor();
        ViewGeometryConversionError::scale_factor(ViewGeometryPlatform::Web, scale_factor)?;
        let logical = ViewGeometryConversionError::viewport_input(
            ViewGeometryPlatform::Web,
            f64::from(self.canvas.client_width()),
            f64::from(self.canvas.client_height()),
        )?;
        let logical_width = ViewGeometryConversionError::exact_f32(
            None,
            ViewGeometryPlatform::Web,
            ViewGeometryConsumer::Layout,
            ViewGeometryConversionField::ViewportWidth,
            i64::from(logical.rect.right_milli),
        )?;
        let logical_height = ViewGeometryConversionError::exact_f32(
            None,
            ViewGeometryPlatform::Web,
            ViewGeometryConsumer::Layout,
            ViewGeometryConversionField::ViewportHeight,
            i64::from(logical.rect.bottom_milli),
        )?;
        let physical = ViewGeometryConversionError::viewport_input(
            ViewGeometryPlatform::Web,
            (f64::from(logical.rect.right_milli) / 1_000.0 * scale_factor).round(),
            (f64::from(logical.rect.bottom_milli) / 1_000.0 * scale_factor).round(),
        )?;
        let physical_width = web_surface_extent(
            physical.rect.right_milli,
            ViewGeometryConversionField::ViewportWidth,
        )?;
        let physical_height = web_surface_extent(
            physical.rect.bottom_milli,
            ViewGeometryConversionField::ViewportHeight,
        )?;
        if self.canvas.width() != physical_width {
            self.canvas.set_width(physical_width);
        }
        if self.canvas.height() != physical_height {
            self.canvas.set_height(physical_height);
        }

        Ok(BrowserViewport {
            render: RenderViewport {
                logical_width,
                logical_height,
                physical_width,
                physical_height,
                scale_factor,
            },
            physical_size: PhysicalSize::new(physical_width, physical_height),
        })
    }
}

fn web_surface_extent(
    value_milli: i32,
    field: ViewGeometryConversionField,
) -> Result<u32, ViewGeometryConversionError> {
    let value_milli = i64::from(value_milli);
    if value_milli < 0 {
        return Err(ViewGeometryConversionError::NegativeExtent {
            node: None,
            platform: ViewGeometryPlatform::Web,
            consumer: ViewGeometryConsumer::Layout,
            field,
            value_milli,
        });
    }
    debug_assert_eq!(value_milli % 1_000, 0);
    let value = u64::try_from(value_milli / 1_000).map_err(|_| {
        ViewGeometryConversionError::NegativeExtent {
            node: None,
            platform: ViewGeometryPlatform::Web,
            consumer: ViewGeometryConsumer::Layout,
            field,
            value_milli,
        }
    })?;
    u32::try_from(value).map_err(|_| ViewGeometryConversionError::IndexRange {
        node: None,
        platform: ViewGeometryPlatform::Web,
        consumer: ViewGeometryConsumer::Layout,
        field,
        value,
        max: u64::from(u32::MAX),
    })
}

fn create_pending_player_surfaces(event_loop: &dyn ActiveEventLoop) {
    for control in registry::active_controls() {
        create_player_surface(event_loop, control);
    }
}

fn create_player_surface(event_loop: &dyn ActiveEventLoop, control: Rc<WebPlayerControl>) {
    let Ok(mut player) = control.player.try_borrow_mut() else {
        return;
    };
    let Some(state) = player.as_mut() else {
        return;
    };
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
            set_fatal(state, WebPlayerError::Window(error.to_string()));
            return;
        }
    };
    let window = Arc::<dyn Window>::from(window);
    state.window = Some(Arc::clone(&window));
    state.gpu = GpuState::Loading;
    let weak_control = Rc::downgrade(&control);
    drop(player);
    spawn_local(async move {
        let result = initialize_gpu(Arc::clone(&window), &weak_control).await;
        if let Err(error) = result {
            if let Some(control) = weak_control.upgrade()
                && let Ok(mut player) = control.player.try_borrow_mut()
                && let Some(state) = player.as_mut()
            {
                state.gpu = GpuState::Failed;
                set_fatal(state, error);
            }
        } else {
            emit_event("arcweft-player-ready", "{}".to_owned());
            window.request_redraw();
        }
    });
}

fn control_for_window(window_id: WindowId) -> Option<Rc<WebPlayerControl>> {
    registry::active_controls().into_iter().find(|control| {
        let Ok(player) = control.player.try_borrow() else {
            return false;
        };
        player
            .as_ref()
            .and_then(|state| state.window.as_ref())
            .is_some_and(|window| window.id() == window_id)
    })
}

impl ApplicationHandler for BrowserApp {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        create_pending_player_surfaces(event_loop);
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        create_pending_player_surfaces(event_loop);
    }

    fn window_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(control) = control_for_window(window_id) else {
            return;
        };
        let Ok(mut player) = control.player.try_borrow_mut() else {
            return;
        };
        let Some(state) = player.as_mut() else {
            return;
        };
        let Some(window) = state.window.clone() else {
            return;
        };
        if window.id() != window_id || state.fatal.is_some() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                drop(player);
                let _ = registry::shutdown(&control);
                return;
            }
            WindowEvent::SurfaceResized(size) => {
                if let GpuState::Ready(gpu) = &mut state.gpu {
                    gpu.host.resize(size);
                }
                if let Err(error) = update_text_input_client_transform(state) {
                    set_fatal(state, error);
                    return;
                }
                window.request_redraw();
            }
            WindowEvent::Focused(focused) => {
                let outcome = state.input.focus_changed(focused);
                let clipboard_requests = apply_outcome(state, outcome);
                schedule_clipboard_requests(Rc::downgrade(&control), clipboard_requests);
                if !focused && let Err(error) = state.text_input.blur_active() {
                    set_fatal(state, WebPlayerError::TextInput(error.to_string()));
                    return;
                }
                window.request_redraw();
            }
            WindowEvent::PointerMoved { position, .. } => {
                let logical = match logical_position(position, window.scale_factor()) {
                    Ok(logical) => logical,
                    Err(error) => {
                        set_fatal(state, error);
                        return;
                    }
                };
                if let Some(frame) = state.prepared.clone() {
                    let outcome = state.input.pointer_move(&frame, PointerId(0), logical);
                    let clipboard_requests = apply_outcome(state, outcome);
                    schedule_clipboard_requests(Rc::downgrade(&control), clipboard_requests);
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
                    let position = match logical_position(position, window.scale_factor()) {
                        Ok(position) => position,
                        Err(error) => {
                            set_fatal(state, error);
                            return;
                        }
                    };
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
                    let clipboard_requests = apply_outcome(state, outcome);
                    schedule_clipboard_requests(Rc::downgrade(&control), clipboard_requests);
                }
                window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        Ok(WheelDelta::lines(f64::from(x), f64::from(y)))
                    }
                    MouseScrollDelta::PixelDelta(position) => WheelDelta::from_physical_pixels(
                        position.x,
                        position.y,
                        window.scale_factor(),
                    ),
                }
                .and_then(|delta| WheelNormalizationPolicy::default().normalize(delta));
                let delta = match delta {
                    Ok(delta) => delta,
                    Err(error) => {
                        set_fatal(state, error.into());
                        return;
                    }
                };
                if let Some(frame) = state.prepared.clone() {
                    let outcome =
                        state
                            .input
                            .precision_scroll(&frame, delta.horizontal(), delta.vertical());
                    let clipboard_requests = apply_outcome(state, outcome);
                    schedule_clipboard_requests(Rc::downgrade(&control), clipboard_requests);
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
                    let text_input_changed = match drain_text_input_edits(state, &frame) {
                        Ok((changed, requests)) => {
                            clipboard_requests.extend(requests);
                            changed
                        }
                        Err(error) => {
                            set_fatal(state, error);
                            return;
                        }
                    };
                    if state.fatal.is_some() {
                        return;
                    }
                    if phase == KeyPhase::Down && text_input_changed {
                        schedule_clipboard_requests(Rc::downgrade(&control), clipboard_requests);
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
                    clipboard_requests.extend(apply_outcome(state, outcome));
                    schedule_clipboard_requests(Rc::downgrade(&control), clipboard_requests);
                }
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => match redraw(state, &window) {
                Ok(clipboard_requests) => {
                    schedule_clipboard_requests(Rc::downgrade(&control), clipboard_requests);
                }
                Err(error) => {
                    set_fatal(state, error);
                }
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &dyn ActiveEventLoop) {
        for control in registry::active_controls() {
            let Ok(player) = control.player.try_borrow() else {
                continue;
            };
            if let Some(window) = player.as_ref().and_then(|state| state.window.as_ref()) {
                window.request_redraw();
            }
        }
    }
}

async fn initialize_gpu(
    window: Arc<dyn Window>,
    control: &Weak<WebPlayerControl>,
) -> Result<(), WebPlayerError> {
    let host = WebGpuCanvasHost::new(window)
        .await
        .map_err(|error| WebPlayerError::WebGpu(error.to_string()))?;
    let mut renderer = SharedRenderer::new(host.device(), host.queue(), host.format());
    let control = control
        .upgrade()
        .ok_or_else(|| WebPlayerError::EventLoop("player closed during GPU startup".to_owned()))?;
    let mut player = control
        .player
        .try_borrow_mut()
        .map_err(|_| WebPlayerError::EventLoop("reentrant GPU startup".to_owned()))?;
    let state = player
        .as_mut()
        .ok_or_else(|| WebPlayerError::EventLoop("player closed during GPU startup".to_owned()))?;
    let font_set = state
        .font_set
        .take()
        .ok_or_else(|| WebPlayerError::Font("font set was already consumed".to_owned()))?;
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
    if !matches!(state.gpu, GpuState::Ready(_)) {
        return Ok(clipboard_requests);
    }
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
        let queued = state.broker.queue_dispatches(step.requested_tasks.clone());
        let report = WebObservationReport::from_step(&step, queued);
        let json = serde_json::to_string(&report)
            .map_err(|error| WebPlayerError::Report(error.to_string()))?;
        emit_event("arcweft-runtime-observation", json);
    }

    let browser_viewport = state.browser_viewport(window)?;
    let viewport = browser_viewport.render;
    let host_millis_u64 = host_millis.max(0.0) as u64;
    let mut candidate = prepare_web_player_frame(state, viewport, host_millis_u64)?;
    update_text_input_client_transform(state)?;
    state
        .text_input
        .sync_prepared_frame(
            candidate.prepared(),
            WebRuntimeTextInputFocusReason::RedrawRefresh,
        )
        .map_err(|error| WebPlayerError::TextInput(error.to_string()))?;
    let (changed, requests) = drain_text_input_edits(state, candidate.prepared())?;
    clipboard_requests.extend(requests);
    if changed {
        candidate = prepare_web_player_frame(state, viewport, host_millis_u64)?;
        state
            .text_input
            .sync_prepared_frame(
                candidate.prepared(),
                WebRuntimeTextInputFocusReason::RedrawRefresh,
            )
            .map_err(|error| WebPlayerError::TextInput(error.to_string()))?;
    }

    let frame_report = WebFrameObservationReport::from_prepared_frame(candidate.prepared());
    let frame_json = serde_json::to_string(&frame_report)
        .map_err(|error| WebPlayerError::Report(error.to_string()))?;
    let registered_font_bytes = state.frame_planner.stats().registered_font_bytes;

    if browser_viewport.physical_size.width == 0 || browser_viewport.physical_size.height == 0 {
        let (published, ()) = state
            .frame_planner
            .publication_guard()
            .publish_with(candidate, &mut state.input, |_| ())
            .map_err(|source| WebPlayerError::PlayerFrame {
                registered_font_bytes,
                source,
            })?;
        emit_event("arcweft-frame-observation", frame_json);
        state.prepared = Some(published.frame);
        return Ok(clipboard_requests);
    }

    let GpuState::Ready(gpu) = &mut state.gpu else {
        let (published, ()) = state
            .frame_planner
            .publication_guard()
            .publish_with(candidate, &mut state.input, |_| ())
            .map_err(|source| WebPlayerError::PlayerFrame {
                registered_font_bytes,
                source,
            })?;
        emit_event("arcweft-frame-observation", frame_json);
        state.prepared = Some(published.frame);
        return Ok(clipboard_requests);
    };

    gpu.host.resize(browser_viewport.physical_size);
    let health = gpu.host.health();
    if let Some(error) = health.device_lost.or(health.uncaptured_error) {
        return Err(WebPlayerError::WebGpu(error));
    }
    let view_render = PreparedViewRenderCandidate::prepare(
        candidate.view_geometry().generation().value(),
        candidate
            .view_geometry()
            .final_nodes()
            .map(|(_, geometry)| geometry),
    )
    .map_err(ViewGeometryRuntimeError::from)
    .map_err(PlayerFrameError::from)
    .map_err(|source| WebPlayerError::PlayerFrame {
        registered_font_bytes,
        source,
    })?;
    let web_candidate = match WebFramePublicationCandidate::prepare(gpu, &candidate, view_render) {
        Ok(candidate) => candidate,
        Err(WebGpuCanvasHostError::SurfaceLost | WebGpuCanvasHostError::SurfaceOutdated) => {
            gpu.host.reconfigure();
            window.request_redraw();
            return Ok(clipboard_requests);
        }
        Err(error) => return Err(WebPlayerError::WebGpu(error.to_string())),
    };
    let (published, ()) = state
        .frame_planner
        .publication_guard()
        .publish_with(candidate, &mut state.input, |frame| {
            web_candidate.commit(gpu, frame)
        })
        .map_err(|source| WebPlayerError::PlayerFrame {
            registered_font_bytes,
            source,
        })?;
    emit_event("arcweft-frame-observation", frame_json);
    state.prepared = Some(published.frame);
    Ok(clipboard_requests)
}

fn prepare_web_player_frame(
    state: &mut PlayerState,
    viewport: RenderViewport,
    host_millis: u64,
) -> Result<PlayerPreparedFrameCandidate, WebPlayerError> {
    let presentation = state.session.presentation();
    let fx_definitions = state.session.fx_definitions();
    let style_environment = state.session.presentation_environment();
    let dialogue_visual = state.dialogue_visual_clock.progress(
        presentation.dialogue.latest_active(),
        host_millis,
        dialogue_visual_time_override_millis(),
    );
    let registered_font_bytes = state.frame_planner.stats().registered_font_bytes;
    state
        .frame_planner
        .prepare_candidate(
            &state.input,
            PlayerFrameRequest {
                presentation,
                fx_definitions,
                images: &state.images,
                style_program: state.session.view_style_program(),
                style_environment: &style_environment,
                style_palettes: state.session.view_style_palettes(),
                viewport,
                fit: state.frame_fit,
                image_time_millis: host_millis,
                visual_time_millis: dialogue_visual.elapsed_millis(),
                dialogue_reveal_complete: dialogue_visual.is_complete(),
                preferences: RenderPreferences::default(),
            },
        )
        .map_err(|source| WebPlayerError::PlayerFrame {
            registered_font_bytes,
            source,
        })
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
        dialogue_progress,
        cancel: _,
        redraw: _,
    } = outcome;
    match dialogue_progress {
        DialogueProgress::None => {}
        DialogueProgress::Reveal => state.dialogue_visual_clock.complete_current_stage(),
        DialogueProgress::Advance { target } => state.session.queue_dialogue_advance(target),
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
    control: Weak<WebPlayerControl>,
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
                let Some(control) = control.upgrade() else {
                    return;
                };
                let Ok(mut player) = control.player.try_borrow_mut() else {
                    return;
                };
                let Some(state) = player.as_mut() else {
                    return;
                };
                if let Some(frame) = state.prepared.clone() {
                    match state.input.apply_clipboard_outcome(&frame, host_outcome) {
                        Ok(outcome) => apply_outcome(state, outcome),
                        Err(error) => {
                            set_fatal(state, WebPlayerError::TextEditor(error.to_string()));
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                }
            };
            pending.extend(nested);
            let Some(control) = control.upgrade() else {
                return;
            };
            let Ok(player) = control.player.try_borrow() else {
                return;
            };
            let Some(state) = player.as_ref() else {
                return;
            };
            if let Some(window) = state.window.as_ref() {
                window.request_redraw();
            }
            if state.fatal.is_some() {
                break;
            }
        }
    });
}

fn update_text_input_client_transform(state: &mut PlayerState) -> Result<(), WebPlayerError> {
    let rect = state.canvas.get_bounding_client_rect();
    let left = ViewGeometryConversionError::logical_pointer(
        ViewGeometryPlatform::Web,
        ViewGeometryConversionField::Left,
        rect.left(),
    )?;
    let top = ViewGeometryConversionError::logical_pointer(
        ViewGeometryPlatform::Web,
        ViewGeometryConversionField::Top,
        rect.top(),
    )?;
    state
        .text_input
        .set_client_transform(WebTextInputClientTransform::new(left, top))
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

fn logical_position(
    position: PhysicalPosition<f64>,
    scale_factor: f64,
) -> Result<ViewportPoint, WebPlayerError> {
    ViewGeometryConversionError::scale_factor(ViewGeometryPlatform::Web, scale_factor)?;
    Ok(ViewportPoint::new(
        ViewGeometryConversionError::logical_pointer(
            ViewGeometryPlatform::Web,
            ViewGeometryConversionField::Left,
            position.x / scale_factor,
        )?,
        ViewGeometryConversionError::logical_pointer(
            ViewGeometryPlatform::Web,
            ViewGeometryConversionField::Top,
            position.y / scale_factor,
        )?,
    ))
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
