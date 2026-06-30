use crate::NativePlayerError;
use crate::native_audio::{NativeAudioRuntime, NativePlayerAudioError};
use crate::text_input_bridge::{
    NativeTextInputBridge, NativeTextInputBridgeError, NativeTextInputBridgeOptions,
    NativeTextInputFocusReason, NativeTextInputFocusedControl, NativeTextInputWindowContext,
};
use crate::window_driver::{WindowCloseSignal, WinitOwnedWindowDriver};
use crate::windowed_ingress::{
    WindowedPatchIngress, WindowedPatchIngressCompletion, WindowedPatchIngressConfig,
    WindowedPatchIngressMessage, WindowedPatchIngressReceiver,
};
use crate::windowed_patch::FrameBoundary;
use crate::windowed_runtime::{
    WindowedRuntimeOutcome, WindowedRuntimeOwner, WindowedRuntimeOwnerError,
};
use arcweft_bundle::ArcweftBundle;
use arcweft_desktop_native::NativeDesktopBackend;
use arcweft_player_scene::images::BundleImageCatalogError;
use arcweft_player_scene::input::{InputController, InputOutcome};
use arcweft_player_scene::text_controls::{
    RuntimeTextControlLowerer, RuntimeTextControlLoweringError,
};
use arcweft_presentation::input::{KeyPhase, PointerId, ViewportPoint};
use arcweft_render_text::LineDisplayFrame;
use arcweft_render_wgpu::geometry::{
    PreparedFrame, PreparedTextInputTarget, RenderChoiceItem, RenderDialogue, RenderPreferences,
    RenderScene, RenderViewport, SharedFramePlanner,
};
use arcweft_render_wgpu::renderer::{SharedRenderer, SharedRendererError};
use arcweft_runtime_driver::clock::{RuntimeClockError, RuntimeClockStep};
use arcweft_runtime_driver::session::{BundleSessionError, BundleSessionOptions, BundleStepInput};
use num_traits::ToPrimitive;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Size};
use winit::event::{ButtonSource, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const EVENT_LOOP_TICK: Duration = Duration::from_millis(16);
const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../../../web/assets/arcweft-demo.ttf");
const SCENE_ASPECT_WIDTH: u32 = 16;
const SCENE_ASPECT_HEIGHT: u32 = 9;

pub fn run_bundle_windowed(
    bundle: ArcweftBundle,
    _max_steps: usize,
) -> Result<(), NativePlayerError> {
    run_shared_scene_window("Arcweft Player", bundle)
        .map_err(|error| NativePlayerError::SceneWindow(error.to_string()))
}

pub fn run_bundle_windowed_with_text_input_options(
    bundle: ArcweftBundle,
    _max_steps: usize,
    text_input_options: NativeTextInputBridgeOptions,
) -> Result<(), NativePlayerError> {
    run_shared_scene_window_with_options("Arcweft Player", bundle, text_input_options, |_| {})
        .map_err(|error| NativePlayerError::SceneWindow(error.to_string()))
}

pub fn run_bundle_windowed_with_ingress(
    bundle: ArcweftBundle,
    _max_steps: usize,
    configure_ingress: impl FnOnce(WindowedPatchIngress),
) -> Result<(), NativePlayerError> {
    run_shared_scene_window_with_ingress("Arcweft Player", bundle, configure_ingress)
        .map_err(|error| NativePlayerError::SceneWindow(error.to_string()))
}

pub fn run_bundle_windowed_with_ingress_and_text_input_options(
    bundle: ArcweftBundle,
    _max_steps: usize,
    text_input_options: NativeTextInputBridgeOptions,
    configure_ingress: impl FnOnce(WindowedPatchIngress),
) -> Result<(), NativePlayerError> {
    run_shared_scene_window_with_options(
        "Arcweft Player",
        bundle,
        text_input_options,
        configure_ingress,
    )
    .map_err(|error| NativePlayerError::SceneWindow(error.to_string()))
}

#[derive(Debug, Error)]
enum NativeSceneWindowError {
    #[error("winit event loop failed: {0}")]
    EventLoop(String),
    #[error("native window failed: {0}")]
    Window(String),
    #[error("bundle session failed: {0}")]
    Session(#[from] BundleSessionError),
    #[error("windowed runtime owner failed: {0}")]
    RuntimeOwner(#[from] WindowedRuntimeOwnerError),
    #[error("runtime clock failed: {0}")]
    Clock(#[from] RuntimeClockError),
    #[error("bundle image catalog failed: {0}")]
    Images(#[from] BundleImageCatalogError),
    #[error("native text-input bridge failed: {0}")]
    TextInputBridge(#[from] NativeTextInputBridgeError),
    #[error("native audio failed: {0}")]
    Audio(#[from] NativePlayerAudioError),
    #[error("player text editor failed: {0}")]
    TextEditor(#[from] arcweft_presentation::text_editor::TextEditorError),
    #[error("runtime text-control lowering failed: {0}")]
    TextControlLowering(#[from] RuntimeTextControlLoweringError),
    #[error("WebGPU surface creation failed: {0}")]
    SurfaceCreation(String),
    #[error("no WebGPU adapter is available for the native surface")]
    AdapterUnavailable,
    #[error("WebGPU device acquisition failed: {0}")]
    DeviceRequest(String),
    #[error("the WebGPU surface reported no supported texture format")]
    NoSurfaceFormat,
    #[error("shared renderer failed: {0}")]
    Renderer(#[from] SharedRendererError),
    #[error("frame planning failed: {0}")]
    FramePlan(String),
    #[error("surface is outdated")]
    SurfaceOutdated,
    #[error("surface was lost")]
    SurfaceLost,
    #[error("surface is currently occluded")]
    SurfaceOccluded,
    #[error("surface acquisition timed out")]
    SurfaceTimeout,
    #[error("surface acquisition failed validation")]
    SurfaceValidation,
}

struct NativeSceneApp {
    title: String,
    bundle: Option<ArcweftBundle>,
    text_input_options: NativeTextInputBridgeOptions,
    state: Option<NativeSceneState>,
    ingress: WindowedPatchIngressReceiver,
    ingress_completion: WindowedPatchIngressCompletion,
    pending_ingress: VecDeque<WindowedPatchIngressMessage>,
    error: Arc<Mutex<Option<String>>>,
}

struct NativeSceneState {
    window: Arc<dyn Window>,
    close_signal: WindowCloseSignal,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: SharedRenderer,
    runtime: WindowedRuntimeOwner,
    audio: Option<NativeAudioRuntime>,
    ingress_completion: WindowedPatchIngressCompletion,
    input: InputController,
    text_input: NativeTextInputBridge,
    prepared: Option<arcweft_render_wgpu::geometry::PreparedFrame>,
    dialogue_visual_clock: DialogueVisualClock,
    started_at: Instant,
    next_tick: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DialogueVisualClock {
    line: Option<arcweft_core::plan::RuntimeLineId>,
    started_at_millis: u64,
}

fn run_shared_scene_window(
    title: &str,
    bundle: ArcweftBundle,
) -> Result<(), NativeSceneWindowError> {
    run_shared_scene_window_with_ingress(title, bundle, |_| {})
}

fn run_shared_scene_window_with_ingress(
    title: &str,
    bundle: ArcweftBundle,
    configure_ingress: impl FnOnce(WindowedPatchIngress),
) -> Result<(), NativeSceneWindowError> {
    run_shared_scene_window_with_options(
        title,
        bundle,
        NativeTextInputBridgeOptions::default(),
        configure_ingress,
    )
}

fn run_shared_scene_window_with_options(
    title: &str,
    bundle: ArcweftBundle,
    text_input_options: NativeTextInputBridgeOptions,
    configure_ingress: impl FnOnce(WindowedPatchIngress),
) -> Result<(), NativeSceneWindowError> {
    let event_loop =
        EventLoop::new().map_err(|error| NativeSceneWindowError::EventLoop(error.to_string()))?;
    let (ingress, ingress_rx) = WindowedPatchIngress::channel(
        event_loop.create_proxy(),
        WindowedPatchIngressConfig::default(),
    );
    let ingress_completion = ingress.completion();
    configure_ingress(ingress);
    let error = Arc::new(Mutex::new(None));
    event_loop
        .run_app(NativeSceneApp {
            title: title.to_owned(),
            bundle: Some(bundle),
            text_input_options,
            state: None,
            ingress: ingress_rx,
            ingress_completion: ingress_completion.clone(),
            pending_ingress: VecDeque::new(),
            error: Arc::clone(&error),
        })
        .map_err(|error| NativeSceneWindowError::EventLoop(error.to_string()))?;
    ingress_completion.close("native player event loop exited");
    let error = error
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    match error {
        Some(error) => Err(NativeSceneWindowError::Window(error)),
        None => Ok(()),
    }
}

impl NativeSceneApp {
    fn fail(&self, event_loop: &dyn ActiveEventLoop, error: String) {
        self.ingress_completion
            .close(format!("native scene failed: {error}"));
        *self
            .error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error);
        event_loop.exit();
    }

    fn drain_ingress_messages(&mut self) {
        let messages = self.ingress.drain();
        for message in messages {
            self.apply_ingress_message(message);
        }
    }

    fn drain_pending_ingress(&mut self) {
        let pending = std::mem::take(&mut self.pending_ingress);
        for message in pending {
            self.apply_ingress_message(message);
        }
    }

    fn apply_ingress_message(&mut self, message: WindowedPatchIngressMessage) {
        if let Some(state) = self.state.as_mut() {
            state.apply_ingress_message(message);
        } else {
            self.pending_ingress.push_back(message);
        }
    }
}

impl ApplicationHandler for NativeSceneApp {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let Some(bundle) = self.bundle.take() else {
            self.fail(
                event_loop,
                "native scene bundle was already consumed".to_owned(),
            );
            return;
        };
        let window = match event_loop.create_window(
            WindowAttributes::default()
                .with_title(self.title.clone())
                .with_surface_size(LogicalSize::new(1280.0, 720.0))
                .with_min_surface_size(LogicalSize::new(640.0, 360.0))
                .with_surface_resize_increments(LogicalSize::new(16.0, 9.0)),
        ) {
            Ok(window) => Arc::<dyn Window>::from(window),
            Err(error) => {
                self.fail(event_loop, error.to_string());
                return;
            }
        };
        match pollster::block_on(NativeSceneState::new(
            Arc::clone(&window),
            self.title.clone(),
            bundle,
            self.ingress_completion.clone(),
            NativeTextInputWindowContext::from_winit_window(window.as_ref()),
            self.text_input_options.clone(),
        )) {
            Ok(state) => {
                self.state = Some(state);
                self.drain_pending_ingress();
                self.drain_ingress_messages();
            }
            Err(error) => {
                self.fail(event_loop, error.to_string());
                return;
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + EVENT_LOOP_TICK));
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if state.window.id() != window_id {
            return;
        }
        let result = match event {
            WindowEvent::CloseRequested => {
                self.ingress_completion.close("native player closed");
                event_loop.exit();
                Ok(())
            }
            WindowEvent::SurfaceResized(size) => {
                state.resize(size);
                state.window.request_redraw();
                Ok(())
            }
            WindowEvent::Focused(focused) => {
                state.window.request_redraw();
                state.focus_changed(focused)
            }
            WindowEvent::PointerMoved { position, .. } => {
                state.pointer_move(position);
                Ok(())
            }
            WindowEvent::PointerButton {
                state: element_state,
                button:
                    button @ (ButtonSource::Mouse(MouseButton::Left) | ButtonSource::Touch { .. }),
                position,
                ..
            } => state.pointer_button(&button, element_state, position),
            WindowEvent::MouseWheel { delta, .. } => {
                state.wheel(delta);
                Ok(())
            }
            WindowEvent::KeyboardInput { event, .. } => {
                state.keyboard(&event.logical_key, event.state)
            }
            WindowEvent::RedrawRequested => state.redraw(),
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, error.to_string());
        }
    }

    fn proxy_wake_up(&mut self, _event_loop: &dyn ActiveEventLoop) {
        self.drain_ingress_messages();
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain_ingress_messages();
        if let Some(state) = self.state.as_ref() {
            if state.take_close_requested() {
                self.ingress_completion
                    .close("native player requested close from owned-window adapter");
                event_loop.exit();
                return;
            }
            state.window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + EVENT_LOOP_TICK));
    }
}

impl NativeSceneState {
    async fn new(
        window: Arc<dyn Window>,
        title: String,
        bundle: ArcweftBundle,
        ingress_completion: WindowedPatchIngressCompletion,
        text_input_window: NativeTextInputWindowContext,
        text_input_options: NativeTextInputBridgeOptions,
    ) -> Result<Self, NativeSceneWindowError> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(|error| NativeSceneWindowError::SurfaceCreation(error.to_string()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|_| NativeSceneWindowError::AdapterUnavailable)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("arcweft-native-scene-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .map_err(|error| NativeSceneWindowError::DeviceRequest(error.to_string()))?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or(NativeSceneWindowError::NoSurfaceFormat)?;
        let size = scene_aspect_size(window.surface_size());
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: capabilities
                .present_modes
                .iter()
                .copied()
                .find(|mode| *mode == wgpu::PresentMode::Fifo)
                .unwrap_or(wgpu::PresentMode::AutoVsync),
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: Vec::new(),
        };
        surface.configure(&device, &config);
        let mut renderer = SharedRenderer::new(&device, &queue, format);
        renderer.register_font_bytes(DEFAULT_FONT_BYTES.to_vec())?;
        let close_signal = WindowCloseSignal::default();
        let owned_window = Arc::new(
            WinitOwnedWindowDriver::try_new(Arc::clone(&window), title, close_signal.clone())
                .map_err(NativeSceneWindowError::Window)?,
        );
        let backend = NativeDesktopBackend::builder()
            .with_owned_window_driver(owned_window)
            .build();
        let audio = NativeAudioRuntime::from_bundle(&bundle)?;
        let runtime = WindowedRuntimeOwner::from_bundle_with_desktop_backend(
            &bundle,
            BundleSessionOptions::default(),
            backend,
        )?;
        let text_input = NativeTextInputBridge::new(text_input_window, text_input_options);
        Ok(Self {
            window,
            close_signal,
            surface,
            device,
            queue,
            config,
            renderer,
            runtime,
            audio,
            ingress_completion,
            input: InputController::default(),
            text_input,
            prepared: None,
            dialogue_visual_clock: DialogueVisualClock::default(),
            started_at: Instant::now(),
            next_tick: 1,
        })
    }

    fn take_close_requested(&self) -> bool {
        self.close_signal.take()
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        let requested = non_zero_size(size);
        let size = scene_aspect_size(requested);
        if size != requested {
            let _ = self.window.request_surface_size(Size::Physical(size));
        }
        if self.config.width == size.width && self.config.height == size.height {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }

    fn redraw(&mut self) -> Result<(), NativeSceneWindowError> {
        self.runtime.pump_main_thread()?;
        self.step_runtime()?;
        let prepared = self.prepare_frame()?;
        self.input.ensure_choice_focus(&prepared);
        let prepared = self.prepare_frame_with_interaction()?;
        self.sync_text_input_bridge(&prepared, NativeTextInputFocusReason::RedrawRefresh)?;
        self.render(&prepared)?;
        let patch_outcomes = self.drain_patch_events_after_render_submitted()?;
        if patch_outcomes
            .iter()
            .any(WindowedRuntimeOutcome::invalidates_prepared_frame)
        {
            self.prepared = None;
        } else {
            self.prepared = Some(prepared);
        }
        Ok(())
    }

    fn step_runtime(&mut self) -> Result<(), NativeSceneWindowError> {
        if self.runtime.session().is_finished() {
            return Ok(());
        }
        if let Some(audio) = &mut self.audio {
            let mut events = Vec::new();
            audio.drain_events(&mut events);
            self.runtime.push_audio_events(events);
        }
        let clock = RuntimeClockStep::from_millis(self.next_tick, 16)?;
        self.next_tick = self.next_tick.saturating_add(1);
        let step = self
            .runtime
            .step_with_clock(clock, BundleStepInput::default());
        if let Some(audio) = &mut self.audio {
            let mut command_events = Vec::new();
            audio.submit_commands(step.audio_commands, &mut command_events);
            self.runtime.push_audio_events(command_events);
        }
        Ok(())
    }

    fn apply_ingress_message(&mut self, message: WindowedPatchIngressMessage) {
        match message {
            WindowedPatchIngressMessage::Enqueue(envelope) => {
                let source = envelope.event.source();
                self.ingress_completion
                    .accepted_by_event_loop(envelope.sequence, source);
                self.runtime.push_patch_event(envelope.event);
            }
            WindowedPatchIngressMessage::RetainRejected { source, message } => {
                self.runtime.retain_patch_ingress_rejection(source, message);
            }
        }
        self.window.request_redraw();
    }

    fn prepare_frame(
        &mut self,
    ) -> Result<arcweft_render_wgpu::geometry::PreparedFrame, NativeSceneWindowError> {
        let scene = self.render_scene()?;
        SharedFramePlanner::prepare(&scene)
            .map_err(|error| NativeSceneWindowError::FramePlan(error.to_string()))
    }

    fn prepare_frame_with_interaction(
        &mut self,
    ) -> Result<arcweft_render_wgpu::geometry::PreparedFrame, NativeSceneWindowError> {
        let scene = self.render_scene()?;
        SharedFramePlanner::prepare(&RenderScene {
            interaction: self.input.visual_state(),
            choice_scroll: self.input.choice_scroll(),
            ..scene
        })
        .map_err(|error| NativeSceneWindowError::FramePlan(error.to_string()))
    }

    fn render_scene(&mut self) -> Result<RenderScene, NativeSceneWindowError> {
        let viewport = self.viewport();
        let elapsed = self.elapsed_millis();
        let presentation = self.runtime.session().presentation();
        let visual_time_millis = dialogue_visual_time_millis(
            &mut self.dialogue_visual_clock,
            presentation.dialogue.as_ref(),
            elapsed,
        );
        // Runtime text controls use the shared player-owned lowering path; IME
        // activation still comes only from PreparedFrame geometry.
        let text_inputs =
            RuntimeTextControlLowerer::lower_for_frame(&mut self.input, &presentation.text_inputs)?;
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
            images: self
                .runtime
                .images()
                .render_images(&presentation.images, elapsed)?,
            viewport,
            visual_time_millis,
            preferences: RenderPreferences::default(),
            interaction: self.input.visual_state(),
            choice_scroll: self.input.choice_scroll(),
        })
    }

    fn render(
        &mut self,
        prepared: &arcweft_render_wgpu::geometry::PreparedFrame,
    ) -> Result<(), NativeSceneWindowError> {
        let surface_frame = match surface_texture(self.surface.get_current_texture()) {
            Ok(texture) => texture,
            Err(NativeSceneWindowError::SurfaceLost | NativeSceneWindowError::SurfaceOutdated) => {
                self.surface.configure(&self.device, &self.config);
                self.window.request_redraw();
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        let view = surface_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer
            .render_to_view(&self.device, &self.queue, &view, prepared)?;
        surface_frame.present();
        Ok(())
    }

    fn drain_patch_events_after_render_submitted(
        &mut self,
    ) -> Result<Vec<WindowedRuntimeOutcome>, NativeSceneWindowError> {
        let outcomes = self
            .runtime
            .drain_patch_boundary(FrameBoundary::AfterRenderSubmitted)?;
        self.ingress_completion
            .completed_at_frame_boundary(outcomes.len());
        if !outcomes.is_empty() {
            self.window.request_redraw();
        }
        Ok(outcomes)
    }

    fn viewport(&self) -> RenderViewport {
        let size = PhysicalSize::new(self.config.width, self.config.height);
        let scale_factor = self.window.scale_factor().max(f64::EPSILON);
        let logical_width = (f64::from(size.width) / scale_factor)
            .to_f32()
            .unwrap_or(f32::MAX);
        let logical_height = (f64::from(size.height) / scale_factor)
            .to_f32()
            .unwrap_or(f32::MAX);
        RenderViewport {
            logical_width,
            logical_height,
            physical_width: size.width,
            physical_height: size.height,
            scale_factor,
        }
    }

    fn elapsed_millis(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn pointer_move(&mut self, position: PhysicalPosition<f64>) {
        if let Some(frame) = self.prepared.clone() {
            self.input
                .pointer_move(&frame, PointerId(0), self.logical_position(position));
            self.window.request_redraw();
        }
    }

    fn pointer_button(
        &mut self,
        button: &ButtonSource,
        element_state: ElementState,
        position: PhysicalPosition<f64>,
    ) -> Result<(), NativeSceneWindowError> {
        let Some(frame) = self.prepared.clone() else {
            return Ok(());
        };
        let pointer = pointer_id(button);
        let position = self.logical_position(position);
        let outcome = match element_state {
            ElementState::Pressed => self.input.pointer_down(&frame, pointer, position),
            ElementState::Released => self.input.pointer_up(&frame, pointer, position),
        };
        self.apply_outcome(outcome)?;
        self.sync_text_input_bridge(&frame, NativeTextInputFocusReason::Pointer)?;
        self.window.request_redraw();
        Ok(())
    }

    fn wheel(&mut self, delta: MouseScrollDelta) {
        let delta_y = match delta {
            MouseScrollDelta::LineDelta(_, y) => y * 32.0,
            MouseScrollDelta::PixelDelta(position) => (position.y / self.window.scale_factor())
                .to_f32()
                .unwrap_or(0.0),
        };
        self.input.wheel(delta_y);
        self.window.request_redraw();
    }

    fn keyboard(
        &mut self,
        key: &Key,
        element_state: ElementState,
    ) -> Result<(), NativeSceneWindowError> {
        let Some(frame) = self.prepared.clone() else {
            return Ok(());
        };
        let phase = match element_state {
            ElementState::Pressed => KeyPhase::Down,
            ElementState::Released => KeyPhase::Up,
        };
        let label = key_label(key);
        let disposition = self.text_input.backend_key_disposition(&label);
        let player_disposition = if self.text_input.shortcuts_allowed(disposition) {
            disposition
        } else {
            arcweft_presentation::text_input::TextInputKeyDisposition::ImeConsumed
        };
        let outcome = self
            .input
            .keyboard_with_ime(&frame, &label, phase, player_disposition);
        self.apply_outcome(outcome)?;
        for edit in self.text_input.drain_platform_edits(player_disposition)? {
            let outcome = self.input.text_input(&frame, edit.into_input())?;
            self.apply_outcome(outcome)?;
        }
        self.window.request_redraw();
        Ok(())
    }

    fn focus_changed(&mut self, focused: bool) -> Result<(), NativeSceneWindowError> {
        let outcome = self.input.focus_changed(focused);
        self.apply_outcome(outcome)?;
        if !focused {
            self.text_input.blur_active()?;
        }
        Ok(())
    }

    fn sync_text_input_bridge(
        &mut self,
        frame: &PreparedFrame,
        reason: NativeTextInputFocusReason,
    ) -> Result<(), NativeSceneWindowError> {
        self.text_input
            .sync_focus(focused_text_input_control(frame, reason))?;
        Ok(())
    }

    fn apply_outcome(&mut self, outcome: InputOutcome) -> Result<(), NativeSceneWindowError> {
        let InputOutcome {
            actions,
            text_control_write_backs,
            dialogue_advance,
            redraw: _,
        } = outcome;
        if dialogue_advance {
            self.runtime.session_mut().queue_dialogue_advance();
        }
        for action in actions {
            self.runtime.session_mut().queue_semantic_action(&action)?;
        }
        self.text_input
            .record_runtime_write_backs(&text_control_write_backs);
        self.runtime
            .session_mut()
            .queue_text_control_write_backs(text_control_write_backs)?;
        Ok(())
    }

    fn logical_position(&self, position: PhysicalPosition<f64>) -> ViewportPoint {
        ViewportPoint::new(
            (position.x / self.window.scale_factor())
                .to_f32()
                .unwrap_or(0.0),
            (position.y / self.window.scale_factor())
                .to_f32()
                .unwrap_or(0.0),
        )
    }
}

fn non_zero_size(size: PhysicalSize<u32>) -> PhysicalSize<u32> {
    PhysicalSize::new(size.width.max(1), size.height.max(1))
}

fn scene_aspect_size(size: PhysicalSize<u32>) -> PhysicalSize<u32> {
    let size = non_zero_size(size);
    if u64::from(size.width) * u64::from(SCENE_ASPECT_HEIGHT)
        == u64::from(size.height) * u64::from(SCENE_ASPECT_WIDTH)
    {
        return size;
    }
    let max_u32 = u64::from(u32::MAX);
    let width_for_height = ((u64::from(size.height) * u64::from(SCENE_ASPECT_WIDTH)
        + u64::from(SCENE_ASPECT_HEIGHT / 2))
        / u64::from(SCENE_ASPECT_HEIGHT))
    .clamp(1, max_u32);
    let height_for_width = ((u64::from(size.width) * u64::from(SCENE_ASPECT_HEIGHT)
        + u64::from(SCENE_ASPECT_WIDTH / 2))
        / u64::from(SCENE_ASPECT_WIDTH))
    .clamp(1, max_u32);
    let width_for_height = u32::try_from(width_for_height).unwrap_or(u32::MAX);
    let height_for_width = u32::try_from(height_for_width).unwrap_or(u32::MAX);

    if height_for_width.abs_diff(size.height) <= width_for_height.abs_diff(size.width) {
        PhysicalSize::new(size.width, height_for_width)
    } else {
        PhysicalSize::new(width_for_height, size.height)
    }
}

fn surface_texture(
    status: wgpu::CurrentSurfaceTexture,
) -> Result<wgpu::SurfaceTexture, NativeSceneWindowError> {
    match status {
        wgpu::CurrentSurfaceTexture::Success(texture)
        | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => Ok(texture),
        wgpu::CurrentSurfaceTexture::Lost => Err(NativeSceneWindowError::SurfaceLost),
        wgpu::CurrentSurfaceTexture::Outdated => Err(NativeSceneWindowError::SurfaceOutdated),
        wgpu::CurrentSurfaceTexture::Timeout => Err(NativeSceneWindowError::SurfaceTimeout),
        wgpu::CurrentSurfaceTexture::Occluded => Err(NativeSceneWindowError::SurfaceOccluded),
        wgpu::CurrentSurfaceTexture::Validation => Err(NativeSceneWindowError::SurfaceValidation),
    }
}

fn pointer_id(button: &ButtonSource) -> PointerId {
    match button {
        ButtonSource::Touch { finger_id, .. } => PointerId(
            u64::try_from(finger_id.into_raw())
                .unwrap_or(u64::MAX.saturating_sub(10))
                .saturating_add(10),
        ),
        ButtonSource::Mouse(_) | ButtonSource::TabletTool { .. } | ButtonSource::Unknown(_) => {
            PointerId(0)
        }
    }
}

fn focused_text_input_control(
    frame: &PreparedFrame,
    reason: NativeTextInputFocusReason,
) -> Option<NativeTextInputFocusedControl> {
    let PreparedTextInputTarget { snapshot, geometry } = frame.focused_text_input_target()?;
    Some(NativeTextInputFocusedControl::new(
        snapshot, geometry, reason,
    ))
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

fn dialogue_visual_time_millis(
    clock: &mut DialogueVisualClock,
    dialogue: Option<&LineDisplayFrame>,
    elapsed_millis: u64,
) -> u64 {
    let Some(dialogue) = dialogue else {
        clock.line = None;
        clock.started_at_millis = elapsed_millis;
        return 0;
    };
    if clock.line.as_ref() != Some(&dialogue.line) {
        clock.line = Some(dialogue.line.clone());
        clock.started_at_millis = elapsed_millis;
    }
    elapsed_millis.saturating_sub(clock.started_at_millis)
}

#[cfg(test)]
mod tests {
    fn native_scene_state_body(source: &str) -> &str {
        let struct_start = source
            .find("struct NativeSceneState {")
            .expect("NativeSceneState declaration exists");
        let source_after_start = &source[struct_start..];
        let body_start = source_after_start
            .find('{')
            .expect("NativeSceneState starts a body");
        let mut depth = 0usize;
        let mut start = None;
        for (offset, character) in source_after_start[body_start..].char_indices() {
            match character {
                '{' => {
                    depth = depth.saturating_add(1);
                    if start.is_none() {
                        start = Some(body_start + offset + character.len_utf8());
                    }
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        let start = start.expect("NativeSceneState body start was recorded");
                        return &source_after_start[start..body_start + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("NativeSceneState body closes");
    }

    #[test]
    fn native_scene_state_stores_runtime_owner_not_session_catalog_pair() {
        let source = include_str!("scene_windowed.rs");
        let body = native_scene_state_body(source);

        assert!(body.contains("runtime: WindowedRuntimeOwner,"));
        assert!(!body.contains("session:"));
        assert!(!body.contains("images:"));
    }

    #[test]
    fn after_render_submitted_boundary_is_after_surface_present_returns() {
        let source = include_str!("scene_windowed.rs");

        assert!(source.contains(
            "self.render(&prepared)?;\n        let patch_outcomes = self.drain_patch_events_after_render_submitted()?;"
        ));
        assert!(source.contains("FrameBoundary::AfterRenderSubmitted"));
    }

    #[test]
    fn windowed_ingress_is_accepted_and_completed_by_event_loop_owner() {
        let source = include_str!("scene_windowed.rs");

        assert!(source.contains("WindowedPatchIngress::channel("));
        assert!(source.contains("fn proxy_wake_up(&mut self, _event_loop: &dyn ActiveEventLoop)"));
        assert!(source.contains("accepted_by_event_loop(envelope.sequence, source)"));
        assert!(source.contains("self.runtime.push_patch_event(envelope.event)"));
        assert!(source.contains("completed_at_frame_boundary(outcomes.len())"));
        assert!(source.contains("ingress_completion.close(\"native player closed\")"));
    }
}
