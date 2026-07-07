use crate::NativePlayerError;
use crate::native_audio::{NativeAudioRuntime, NativePlayerAudioError};
use crate::text_input_bridge::{
    NativeTextInputBridge, NativeTextInputBridgeError, NativeTextInputBridgeOptions,
    NativeTextInputFocusReason, NativeTextInputFocusedControl,
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
use arcweft_layout::ScalePolicy;
use arcweft_player_scene::fonts::PlayerFontSet;
use arcweft_player_scene::frame::{
    PlayerFrameError, PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest,
};
use arcweft_player_scene::input::{
    InputController, InputControllerSnapshot, InputControllerSnapshotError, InputOutcome,
};
use arcweft_presentation::input::{KeyPhase, PointerId, ViewportPoint};
use arcweft_presentation::text_input::{
    Capitalization, CompositionEndReason, TextAssistPolicy, TextByteOffset, TextCommit,
    TextCompositionUpdate, TextDeleteUnit, TextEditCommand, TextInput, TextInputClientSnapshot,
    TextInputGeometrySnapshot, TextInputKeyDisposition, TextInputOperation, TextInputOptions,
    TextInputPrivacy, TextInputPurpose, TextInputSerial, TextRange,
};
use arcweft_render_text::LineDisplayFrame;
use arcweft_render_wgpu::geometry::{
    PreparedFrame, PreparedTextInputTarget, RenderPreferences, RenderViewport,
};
use arcweft_render_wgpu::renderer::{SharedRenderer, SharedRendererError};
use arcweft_runtime_driver::clock::{RuntimeClockError, RuntimeClockStep};
use arcweft_runtime_driver::session::{BundleSessionError, BundleSessionOptions, BundleStepInput};
use arcweft_runtime_driver::session_save::BundleSessionSaveError;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Size};
use winit::event::{
    ButtonSource, ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{
    ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose, ImeRequest, ImeRequestData,
    ImeRequestError, ImeSurroundingText, Window, WindowAttributes, WindowId,
};

const EVENT_LOOP_TICK: Duration = Duration::from_millis(16);
const NATIVE_PLAYER_SESSION_SAVE_SCHEMA_ID: &str = "arcweft.native_player_session";
const NATIVE_PLAYER_SESSION_SAVE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct NativePlayerOptions {
    text_input: NativeTextInputBridgeOptions,
    frame_fit: PlayerFrameFit,
    session_load: Option<PathBuf>,
    session_save_out: Option<PathBuf>,
}

impl Default for NativePlayerOptions {
    fn default() -> Self {
        Self {
            text_input: NativeTextInputBridgeOptions::default(),
            frame_fit: PlayerFrameFit::design_1280x720(ScalePolicy::Contain),
            session_load: None,
            session_save_out: None,
        }
    }
}

impl NativePlayerOptions {
    #[must_use]
    pub fn with_text_input_options(mut self, options: NativeTextInputBridgeOptions) -> Self {
        self.text_input = options;
        self
    }

    #[must_use]
    pub fn with_frame_fit(mut self, frame_fit: PlayerFrameFit) -> Self {
        self.frame_fit = frame_fit;
        self
    }

    #[must_use]
    pub fn with_session_load_path(mut self, path: PathBuf) -> Self {
        self.session_load = Some(path);
        self
    }

    #[must_use]
    pub fn with_session_save_out_path(mut self, path: PathBuf) -> Self {
        self.session_save_out = Some(path);
        self
    }
}

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
    run_shared_scene_window_with_options(
        "Arcweft Player",
        bundle,
        NativePlayerOptions::default().with_text_input_options(text_input_options),
        |_| {},
    )
    .map_err(|error| NativePlayerError::SceneWindow(error.to_string()))
}

pub fn run_bundle_windowed_with_options(
    bundle: ArcweftBundle,
    _max_steps: usize,
    options: NativePlayerOptions,
) -> Result<(), NativePlayerError> {
    run_shared_scene_window_with_options("Arcweft Player", bundle, options, |_| {})
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
        NativePlayerOptions::default().with_text_input_options(text_input_options),
        configure_ingress,
    )
    .map_err(|error| NativePlayerError::SceneWindow(error.to_string()))
}

pub fn run_bundle_windowed_with_ingress_and_options(
    bundle: ArcweftBundle,
    _max_steps: usize,
    options: NativePlayerOptions,
    configure_ingress: impl FnOnce(WindowedPatchIngress),
) -> Result<(), NativePlayerError> {
    run_shared_scene_window_with_options("Arcweft Player", bundle, options, configure_ingress)
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
    #[error("bundle session save failed: {0}")]
    SessionSave(#[from] BundleSessionSaveError),
    #[error("failed to {operation} native player session save {path}: {source}")]
    SessionSaveIo {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("native player session save schema id `{actual}` does not match expected `{expected}`")]
    NativePlayerSessionSaveSchemaId { actual: String, expected: String },
    #[error(
        "native player session save schema version {actual} is not supported; expected {expected}"
    )]
    NativePlayerSessionSaveSchemaVersion { actual: u32, expected: u32 },
    #[error("failed to encode native player session save: {message}")]
    NativePlayerSessionSaveEncode { message: String },
    #[error("failed to decode native player session save: {message}")]
    NativePlayerSessionSaveDecode { message: String },
    #[error("player input snapshot restore failed: {0}")]
    InputSnapshot(#[from] InputControllerSnapshotError),
    #[error("windowed runtime owner failed: {0}")]
    RuntimeOwner(#[from] WindowedRuntimeOwnerError),
    #[error("runtime clock failed: {0}")]
    Clock(#[from] RuntimeClockError),
    #[error("player frame failed: {0}")]
    PlayerFrame(#[from] PlayerFrameError),
    #[error("native text-input bridge failed: {0}")]
    TextInputBridge(#[from] NativeTextInputBridgeError),
    #[error("native audio failed: {0}")]
    Audio(#[from] NativePlayerAudioError),
    #[error("player text editor failed: {0}")]
    TextEditor(#[from] arcweft_presentation::text_editor::TextEditorError),
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
    #[error("player font registration failed: {0}")]
    Font(String),
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
    options: NativePlayerOptions,
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
    frame_planner: PlayerFramePlannerState,
    runtime: WindowedRuntimeOwner,
    audio: Option<NativeAudioRuntime>,
    ingress_completion: WindowedPatchIngressCompletion,
    input: InputController,
    keyboard_modifiers: ModifiersState,
    text_input: NativeTextInputBridge,
    window_ime_supported: bool,
    window_ime_enabled: bool,
    next_window_ime_serial: u64,
    frame_fit: PlayerFrameFit,
    session_save_out: Option<PathBuf>,
    session_save_on_exit_completed: bool,
    prepared: Option<arcweft_render_wgpu::geometry::PreparedFrame>,
    dialogue_visual_clock: DialogueVisualClock,
    started_at: Instant,
    next_tick: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct NativePlayerSessionSaveSchema {
    id: String,
    version: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct NativePlayerSessionSave {
    schema: NativePlayerSessionSaveSchema,
    runtime_session: Vec<u8>,
    input: InputControllerSnapshot,
}

impl Default for NativePlayerSessionSaveSchema {
    fn default() -> Self {
        Self {
            id: NATIVE_PLAYER_SESSION_SAVE_SCHEMA_ID.to_owned(),
            version: NATIVE_PLAYER_SESSION_SAVE_SCHEMA_VERSION,
        }
    }
}

impl NativePlayerSessionSaveSchema {
    fn validate(&self) -> Result<(), NativeSceneWindowError> {
        if self.id != NATIVE_PLAYER_SESSION_SAVE_SCHEMA_ID {
            return Err(NativeSceneWindowError::NativePlayerSessionSaveSchemaId {
                actual: self.id.clone(),
                expected: NATIVE_PLAYER_SESSION_SAVE_SCHEMA_ID.to_owned(),
            });
        }
        if self.version != NATIVE_PLAYER_SESSION_SAVE_SCHEMA_VERSION {
            return Err(
                NativeSceneWindowError::NativePlayerSessionSaveSchemaVersion {
                    actual: self.version,
                    expected: NATIVE_PLAYER_SESSION_SAVE_SCHEMA_VERSION,
                },
            );
        }
        Ok(())
    }
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
        NativePlayerOptions::default(),
        configure_ingress,
    )
}

fn run_shared_scene_window_with_options(
    title: &str,
    bundle: ArcweftBundle,
    options: NativePlayerOptions,
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
            options,
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
        let mut attributes = WindowAttributes::default()
            .with_title(self.title.clone())
            .with_surface_size(frame_fit_surface_size(self.options.frame_fit))
            .with_min_surface_size(frame_fit_min_surface_size(self.options.frame_fit));
        if let Some(increments) = frame_fit_resize_increments(self.options.frame_fit) {
            attributes = attributes.with_surface_resize_increments(increments);
        }
        let window = match event_loop.create_window(attributes) {
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
            self.options.clone(),
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
            WindowEvent::CloseRequested => match state.save_session_on_exit() {
                Ok(()) => {
                    self.ingress_completion.close("native player closed");
                    event_loop.exit();
                    Ok(())
                }
                Err(error) => Err(error),
            },
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
                    button @ (ButtonSource::Mouse(MouseButton::Left | MouseButton::Right)
                    | ButtonSource::Touch { .. }),
                position,
                ..
            } => state.pointer_button(&button, element_state, position),
            WindowEvent::MouseWheel { delta, .. } => state.wheel(delta),
            WindowEvent::KeyboardInput { event, .. } => state.keyboard(&event),
            WindowEvent::ModifiersChanged(modifiers) => {
                state.keyboard_modifiers = modifiers.state();
                Ok(())
            }
            WindowEvent::Ime(event) => state.ime(event),
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
        if let Some(state) = self.state.as_mut() {
            if state.take_close_requested() {
                if let Err(error) = state.save_session_on_exit() {
                    self.fail(event_loop, error.to_string());
                    return;
                }
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
        options: NativePlayerOptions,
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
        let size = scene_aspect_size(window.surface_size(), options.frame_fit);
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
        let mut frame_planner = PlayerFramePlannerState::new();
        PlayerFontSet::bundled_default()
            .register_with_renderer_and_planner(&mut renderer, &mut frame_planner)
            .map_err(|error| NativeSceneWindowError::Font(error.to_string()))?;
        let close_signal = WindowCloseSignal::default();
        let owned_window = Arc::new(
            WinitOwnedWindowDriver::try_new(Arc::clone(&window), title, close_signal.clone())
                .map_err(NativeSceneWindowError::Window)?,
        );
        let backend = NativeDesktopBackend::builder()
            .with_owned_window_driver(owned_window)
            .build();
        let audio = NativeAudioRuntime::from_bundle(&bundle)?;
        let (runtime, input) =
            restored_windowed_runtime_and_input(&bundle, backend, options.session_load.as_deref())?;
        let text_input = NativeTextInputBridge::new(options.text_input.clone());
        Ok(Self {
            window,
            close_signal,
            surface,
            device,
            queue,
            config,
            renderer,
            frame_planner,
            runtime,
            audio,
            ingress_completion,
            input,
            keyboard_modifiers: ModifiersState::default(),
            text_input,
            window_ime_supported: true,
            window_ime_enabled: false,
            next_window_ime_serial: 1,
            frame_fit: options.frame_fit,
            session_save_out: options.session_save_out.clone(),
            session_save_on_exit_completed: false,
            prepared: None,
            dialogue_visual_clock: DialogueVisualClock::default(),
            started_at: Instant::now(),
            next_tick: 1,
        })
    }

    fn take_close_requested(&self) -> bool {
        self.close_signal.take()
    }

    fn save_session_on_exit(&mut self) -> Result<(), NativeSceneWindowError> {
        if self.session_save_on_exit_completed {
            return Ok(());
        }
        let Some(path) = self.session_save_out.clone() else {
            return Ok(());
        };
        save_native_player_session(&path, &self.runtime, &self.input)?;
        self.session_save_on_exit_completed = true;
        Ok(())
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        let requested = non_zero_size(size);
        let size = scene_aspect_size(requested, self.frame_fit);
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
        self.sync_text_input_bridge(&prepared.frame, NativeTextInputFocusReason::RedrawRefresh)?;
        self.sync_window_ime(&prepared.frame);
        self.render(&prepared.frame)?;
        let patch_outcomes = self.drain_patch_events_after_render_submitted()?;
        if patch_outcomes
            .iter()
            .any(WindowedRuntimeOutcome::invalidates_prepared_frame)
        {
            self.prepared = None;
        } else {
            self.prepared = Some(prepared.frame);
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
    ) -> Result<arcweft_player_scene::frame::PlayerPreparedFrame, NativeSceneWindowError> {
        let viewport = self.viewport();
        let elapsed = self.elapsed_millis();
        let presentation = self.runtime.session().presentation();
        let visual_time_millis = dialogue_visual_time_millis(
            &mut self.dialogue_visual_clock,
            presentation.dialogue.as_ref(),
            elapsed,
        );
        Ok(self.frame_planner.prepare(
            &mut self.input,
            PlayerFrameRequest {
                presentation,
                images: self.runtime.images(),
                viewport,
                fit: self.frame_fit,
                image_time_millis: elapsed,
                visual_time_millis,
                preferences: RenderPreferences::default(),
            },
        )?)
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
        let modifiers = arcweft_player_scene::input::InputPointerModifiers::new(
            self.keyboard_modifiers.shift_key(),
        );
        let outcome = match element_state {
            ElementState::Pressed if button.clone().mouse_button() == Some(MouseButton::Right) => {
                self.input
                    .pointer_context_menu(&frame, pointer, position, modifiers)
            }
            ElementState::Pressed => self
                .input
                .pointer_down(&frame, pointer, position, modifiers),
            ElementState::Released => self.input.pointer_up(&frame, pointer, position, modifiers),
        };
        self.apply_outcome(outcome)?;
        let prepared = self.prepare_frame()?;
        self.sync_text_input_bridge(&prepared.frame, NativeTextInputFocusReason::Pointer)?;
        self.sync_window_ime(&prepared.frame);
        self.prepared = Some(prepared.frame);
        self.window.request_redraw();
        Ok(())
    }

    fn wheel(&mut self, delta: MouseScrollDelta) -> Result<(), NativeSceneWindowError> {
        let Some(frame) = self.prepared.clone() else {
            return Ok(());
        };
        let delta_y = match delta {
            MouseScrollDelta::LineDelta(_, y) => y * 32.0,
            MouseScrollDelta::PixelDelta(position) => (position.y / self.window.scale_factor())
                .to_f32()
                .unwrap_or(0.0),
        };
        let outcome = self.input.wheel(&frame, delta_y);
        self.apply_outcome(outcome)?;
        let prepared = self.prepare_frame()?;
        self.sync_text_input_bridge(&prepared.frame, NativeTextInputFocusReason::RedrawRefresh)?;
        self.sync_window_ime(&prepared.frame);
        self.prepared = Some(prepared.frame);
        self.window.request_redraw();
        Ok(())
    }

    fn keyboard(&mut self, event: &KeyEvent) -> Result<(), NativeSceneWindowError> {
        let Some(frame) = self.prepared.clone() else {
            return Ok(());
        };
        let phase = match event.state {
            ElementState::Pressed => KeyPhase::Down,
            ElementState::Released => KeyPhase::Up,
        };
        if phase == KeyPhase::Down
            && let Some(operation) =
                self.text_input_operation_from_key_event(event, self.keyboard_modifiers)
        {
            self.apply_window_ime_operations(vec![operation])?;
            return Ok(());
        }
        let key = &event.logical_key;
        let label = key_label(key);
        let disposition = self.text_input.backend_key_disposition(&label);
        let player_disposition = if self.text_input.shortcuts_allowed(disposition) {
            disposition
        } else {
            arcweft_presentation::text_input::TextInputKeyDisposition::ImeConsumed
        };
        let outcome = self.input.keyboard_with_modifiers_and_ime(
            &frame,
            &label,
            phase,
            self.keyboard_modifiers.shift_key(),
            player_disposition,
        );
        self.apply_outcome(outcome)?;
        self.window.request_redraw();
        Ok(())
    }

    fn text_input_operation_from_key_event(
        &self,
        event: &KeyEvent,
        modifiers: ModifiersState,
    ) -> Option<TextInputOperation> {
        let editor = self.input.focused_text_editor()?;
        let selecting = modifiers.shift_key() && editor.options().selection_enabled();
        if editor.options().shortcuts_enabled()
            && let Some(command) = shortcut_command_from_key(&event.logical_key, modifiers)
        {
            return Some(TextInputOperation::Command(command));
        }
        match &event.logical_key {
            Key::Named(NamedKey::Backspace) if modifiers.control_key() || modifiers.alt_key() => {
                Some(TextInputOperation::Command(TextEditCommand::DeleteWordLeft))
            }
            Key::Named(NamedKey::Backspace) => {
                Some(TextInputOperation::Command(TextEditCommand::Backspace))
            }
            Key::Named(NamedKey::Delete) if modifiers.control_key() || modifiers.alt_key() => Some(
                TextInputOperation::Command(TextEditCommand::DeleteWordRight),
            ),
            Key::Named(NamedKey::Delete) => {
                Some(TextInputOperation::Command(TextEditCommand::Delete))
            }
            Key::Named(NamedKey::ArrowLeft) => Some(TextInputOperation::Command(
                left_arrow_text_command(modifiers, selecting),
            )),
            Key::Named(NamedKey::ArrowRight) => Some(TextInputOperation::Command(
                right_arrow_text_command(modifiers, selecting),
            )),
            Key::Named(NamedKey::ArrowUp) => {
                Some(TextInputOperation::Command(TextEditCommand::MoveUp {
                    selecting,
                }))
            }
            Key::Named(NamedKey::ArrowDown) => {
                Some(TextInputOperation::Command(TextEditCommand::MoveDown {
                    selecting,
                }))
            }
            Key::Named(NamedKey::PageUp) => {
                Some(TextInputOperation::Command(TextEditCommand::MovePageUp {
                    selecting,
                }))
            }
            Key::Named(NamedKey::PageDown) => {
                Some(TextInputOperation::Command(TextEditCommand::MovePageDown {
                    selecting,
                }))
            }
            Key::Named(NamedKey::Home) => {
                let command = if modifiers.control_key() || modifiers.meta_key() {
                    TextEditCommand::MoveDocumentStart { selecting }
                } else {
                    TextEditCommand::MoveLineStart { selecting }
                };
                Some(TextInputOperation::Command(command))
            }
            Key::Named(NamedKey::End) => {
                let command = if modifiers.control_key() || modifiers.meta_key() {
                    TextEditCommand::MoveDocumentEnd { selecting }
                } else {
                    TextEditCommand::MoveLineEnd { selecting }
                };
                Some(TextInputOperation::Command(command))
            }
            Key::Named(NamedKey::Tab) if editor.options().tab_inserts_text() => {
                Some(TextInputOperation::Commit(TextCommit::new("\t")))
            }
            Key::Named(NamedKey::Enter) => {
                if editor.options().is_multiline() {
                    Some(TextInputOperation::Commit(TextCommit::new("\n")))
                } else {
                    Some(TextInputOperation::Command(TextEditCommand::Submit))
                }
            }
            Key::Named(NamedKey::Escape) => {
                Some(TextInputOperation::Command(TextEditCommand::Cancel))
            }
            _ if shortcut_modifier_active(modifiers) => None,
            _ => event
                .text
                .as_ref()
                .and_then(|text| text_input_commit_from_key_text(text.as_str())),
        }
    }

    fn ime(&mut self, event: Ime) -> Result<(), NativeSceneWindowError> {
        match event {
            Ime::Enabled => {
                self.window_ime_supported = true;
                self.window_ime_enabled = true;
                if self.input.window_focused()
                    && let Some(frame) = self.prepared.clone()
                {
                    self.sync_window_ime(&frame);
                }
                Ok(())
            }
            Ime::Preedit(preedit, selection) => {
                if !self.input.window_focused() {
                    return Ok(());
                }
                let selection = window_ime_composition_selection(&preedit, selection);
                let update = TextCompositionUpdate::new(preedit, selection);
                self.apply_window_ime_operations(vec![TextInputOperation::SetComposition(update)])
            }
            Ime::Commit(text) => {
                if !self.input.window_focused() {
                    return Ok(());
                }
                self.apply_window_ime_operations(vec![TextInputOperation::Commit(TextCommit::new(
                    text,
                ))])
            }
            Ime::DeleteSurrounding {
                before_bytes,
                after_bytes,
            } => {
                if !self.input.window_focused() {
                    return Ok(());
                }
                self.apply_window_ime_operations(vec![TextInputOperation::DeleteSurrounding {
                    before: u32::try_from(before_bytes).unwrap_or(u32::MAX),
                    after: u32::try_from(after_bytes).unwrap_or(u32::MAX),
                    unit: TextDeleteUnit::Utf8Byte,
                }])
            }
            Ime::Disabled => {
                self.window_ime_enabled = false;
                if !self.input.window_focused() {
                    return Ok(());
                }
                self.apply_window_ime_operations(vec![TextInputOperation::EndComposition {
                    reason: CompositionEndReason::PlatformDisabled,
                }])
            }
        }
    }

    fn focus_changed(&mut self, focused: bool) -> Result<(), NativeSceneWindowError> {
        let outcome = self.input.focus_changed(focused);
        self.apply_outcome(outcome)?;
        if !focused {
            self.text_input.blur_active();
            self.disable_window_ime();
        }
        Ok(())
    }

    fn apply_window_ime_operations(
        &mut self,
        operations: Vec<TextInputOperation>,
    ) -> Result<(), NativeSceneWindowError> {
        if operations.is_empty() {
            return Ok(());
        }
        let Some(frame) = self.prepared.clone() else {
            return Ok(());
        };
        let Some(editor) = self.input.focused_text_editor() else {
            return Ok(());
        };
        let session = editor.session();
        let privacy = if editor.options().is_secure() {
            TextInputPrivacy::Sensitive
        } else {
            TextInputPrivacy::Plain
        };
        let input = TextInput::new(session, self.next_window_ime_serial(), operations)
            .with_privacy(privacy);
        self.text_input
            .record_window_ime_text_input(&input, TextInputKeyDisposition::ImeConsumed);
        let outcome = self.input.text_input(&frame, input)?;
        self.apply_outcome(outcome)?;
        let prepared = self.prepare_frame()?;
        self.sync_text_input_bridge(&prepared.frame, NativeTextInputFocusReason::RedrawRefresh)?;
        self.sync_window_ime(&prepared.frame);
        self.prepared = Some(prepared.frame);
        self.window.request_redraw();
        Ok(())
    }

    fn next_window_ime_serial(&mut self) -> TextInputSerial {
        let serial = TextInputSerial(self.next_window_ime_serial);
        self.next_window_ime_serial = self.next_window_ime_serial.saturating_add(1);
        serial
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

    fn sync_window_ime(&mut self, frame: &PreparedFrame) {
        if !self.window_ime_supported {
            return;
        }
        if !self.input.window_focused() {
            return;
        }
        let Some(PreparedTextInputTarget { snapshot, geometry }) =
            frame.focused_text_input_target()
        else {
            self.disable_window_ime();
            return;
        };
        let request = window_ime_request_data(&snapshot, &geometry);
        if self.window_ime_enabled {
            self.update_window_ime(request);
        } else {
            self.enable_window_ime(request);
        }
    }

    fn enable_window_ime(&mut self, request: ImeRequestData) {
        let capabilities = window_ime_capabilities_for_request(&request);
        let Some(enable) = ImeEnableRequest::new(capabilities, request.clone()) else {
            self.window_ime_supported = false;
            return;
        };
        match self.window.request_ime_update(ImeRequest::Enable(enable)) {
            Ok(()) | Err(ImeRequestError::AlreadyEnabled) => {
                self.window_ime_enabled = true;
                self.update_window_ime(request);
            }
            Err(ImeRequestError::NotEnabled) => {
                self.window_ime_enabled = false;
            }
            Err(_) => {
                self.mark_window_ime_unsupported();
            }
        }
    }

    fn update_window_ime(&mut self, request: ImeRequestData) {
        match self
            .window
            .request_ime_update(ImeRequest::Update(request.clone()))
        {
            Ok(()) | Err(ImeRequestError::AlreadyEnabled) => {
                self.window_ime_enabled = true;
            }
            Err(ImeRequestError::NotEnabled) => {
                self.window_ime_enabled = false;
                self.enable_window_ime(request);
            }
            Err(_) => {
                self.mark_window_ime_unsupported();
            }
        }
    }

    fn disable_window_ime(&mut self) {
        if self.window_ime_enabled {
            let _ = self.window.request_ime_update(ImeRequest::Disable);
        }
        self.window_ime_enabled = false;
    }

    fn mark_window_ime_unsupported(&mut self) {
        self.window_ime_supported = false;
        self.window_ime_enabled = false;
    }

    fn apply_outcome(&mut self, outcome: InputOutcome) -> Result<(), NativeSceneWindowError> {
        let InputOutcome {
            actions,
            text_control_write_backs,
            diagnostics: _,
            dialogue_advance,
            cancel: _,
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

fn frame_fit_surface_size(frame_fit: PlayerFrameFit) -> LogicalSize<f64> {
    match frame_fit.scale_policy {
        ScalePolicy::Raw => LogicalSize::new(1280.0, 720.0),
        ScalePolicy::Contain | ScalePolicy::Cover | ScalePolicy::Stretch => LogicalSize::new(
            f64::from(frame_fit.design_width.max(1)),
            f64::from(frame_fit.design_height.max(1)),
        ),
    }
}

fn frame_fit_min_surface_size(frame_fit: PlayerFrameFit) -> LogicalSize<f64> {
    let size = frame_fit_surface_size(frame_fit);
    LogicalSize::new(
        (size.width * 0.5).max(320.0),
        (size.height * 0.5).max(180.0),
    )
}

fn frame_fit_resize_increments(frame_fit: PlayerFrameFit) -> Option<LogicalSize<f64>> {
    let (width, height) = frame_fit_aspect(frame_fit)?;
    Some(LogicalSize::new(f64::from(width), f64::from(height)))
}

fn scene_aspect_size(size: PhysicalSize<u32>, frame_fit: PlayerFrameFit) -> PhysicalSize<u32> {
    let size = non_zero_size(size);
    let Some((aspect_width, aspect_height)) = frame_fit_aspect(frame_fit) else {
        return size;
    };
    if u64::from(size.width) * u64::from(aspect_height)
        == u64::from(size.height) * u64::from(aspect_width)
    {
        return size;
    }
    let max_u32 = u64::from(u32::MAX);
    let width_for_height = ((u64::from(size.height) * u64::from(aspect_width)
        + u64::from(aspect_height / 2))
        / u64::from(aspect_height))
    .clamp(1, max_u32);
    let height_for_width = ((u64::from(size.width) * u64::from(aspect_height)
        + u64::from(aspect_width / 2))
        / u64::from(aspect_width))
    .clamp(1, max_u32);
    let width_for_height = u32::try_from(width_for_height).unwrap_or(u32::MAX);
    let height_for_width = u32::try_from(height_for_width).unwrap_or(u32::MAX);

    if height_for_width.abs_diff(size.height) <= width_for_height.abs_diff(size.width) {
        PhysicalSize::new(size.width, height_for_width)
    } else {
        PhysicalSize::new(width_for_height, size.height)
    }
}

fn frame_fit_aspect(frame_fit: PlayerFrameFit) -> Option<(u32, u32)> {
    match frame_fit.scale_policy {
        ScalePolicy::Contain | ScalePolicy::Cover => {
            let width = frame_fit.design_width.max(1);
            let height = frame_fit.design_height.max(1);
            let divisor = gcd(width, height).max(1);
            Some((width / divisor, height / divisor))
        }
        ScalePolicy::Raw | ScalePolicy::Stretch => None,
    }
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    left
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

fn window_ime_request_data(
    snapshot: &TextInputClientSnapshot,
    geometry: &TextInputGeometrySnapshot,
) -> ImeRequestData {
    let mut request = ImeRequestData::default()
        .with_hint_and_purpose(
            window_ime_hint(snapshot.options()),
            window_ime_purpose(snapshot.options()),
        )
        .with_cursor_area(
            window_ime_cursor_position(geometry),
            window_ime_cursor_size(geometry),
        );
    if let Some(surrounding) = window_ime_surrounding_text(snapshot) {
        request = request.with_surrounding_text(surrounding);
    }
    request
}

fn window_ime_capabilities_for_request(request: &ImeRequestData) -> ImeCapabilities {
    let capabilities = ImeCapabilities::new()
        .with_hint_and_purpose()
        .with_cursor_area();
    if request.surrounding_text.is_some() {
        capabilities.with_surrounding_text()
    } else {
        capabilities
    }
}

fn window_ime_cursor_position(geometry: &TextInputGeometrySnapshot) -> winit::dpi::Position {
    let caret = geometry.viewport_caret_rect();
    LogicalPosition::new(f64::from(caret.x), f64::from(caret.y)).into()
}

fn window_ime_cursor_size(geometry: &TextInputGeometrySnapshot) -> Size {
    let caret = geometry.viewport_caret_rect();
    LogicalSize::new(
        f64::from(caret.width.max(1.0)),
        f64::from(caret.height.max(1.0)),
    )
    .into()
}

fn window_ime_surrounding_text(snapshot: &TextInputClientSnapshot) -> Option<ImeSurroundingText> {
    if snapshot.options().is_secure() {
        return None;
    }
    let text = snapshot.surrounding_text();
    let (cursor, anchor) = window_ime_selection_offsets(snapshot)?;
    let (excerpt_start, excerpt_end) = surrounding_excerpt_range(text, cursor, anchor)?;
    let excerpt = text.get(excerpt_start..excerpt_end)?.to_owned();
    ImeSurroundingText::new(excerpt, cursor - excerpt_start, anchor - excerpt_start).ok()
}

fn window_ime_selection_offsets(snapshot: &TextInputClientSnapshot) -> Option<(usize, usize)> {
    let base = snapshot.surrounding_start().get();
    let start = snapshot.selection().start().get().checked_sub(base)?;
    let end = snapshot.selection().end().get().checked_sub(base)?;
    let start = usize::try_from(start).ok()?;
    let end = usize::try_from(end).ok()?;
    let text = snapshot.surrounding_text();
    if start > text.len() || end > text.len() {
        return None;
    }
    if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return None;
    }
    Some((end, start))
}

fn surrounding_excerpt_range(text: &str, cursor: usize, anchor: usize) -> Option<(usize, usize)> {
    let first = cursor.min(anchor);
    let last = cursor.max(anchor);
    let max_len = ImeSurroundingText::MAX_TEXT_BYTES.saturating_sub(1);
    if last.saturating_sub(first) > max_len {
        return None;
    }
    let mut start = last.saturating_sub(max_len);
    while start < first && !text.is_char_boundary(start) {
        start = start.saturating_add(1);
    }
    if start > first {
        start = first;
    }
    let mut end = text.len().min(start.saturating_add(max_len));
    while end > last && !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    if end < last || start > end {
        return None;
    }
    Some((start, end))
}

fn window_ime_hint(options: &TextInputOptions) -> ImeHint {
    let mut hint = ImeHint::NONE;
    if matches!(options.autocorrect(), TextAssistPolicy::Enabled) {
        hint |= ImeHint::COMPLETION;
    }
    if matches!(options.spellcheck(), TextAssistPolicy::Enabled) {
        hint |= ImeHint::SPELLCHECK;
    }
    match options.capitalization() {
        Capitalization::None => {}
        Capitalization::Sentences => hint |= ImeHint::AUTO_CAPITALIZATION,
        Capitalization::Words => hint |= ImeHint::TITLECASE,
        Capitalization::Characters => hint |= ImeHint::UPPERCASE,
    }
    if options.is_secure() {
        hint |= ImeHint::HIDDEN_TEXT | ImeHint::SENSITIVE_DATA;
    }
    if options.is_multiline() {
        hint |= ImeHint::MULTILINE;
    }
    if matches!(options.purpose(), TextInputPurpose::Terminal) {
        hint |= ImeHint::LATIN;
    }
    hint
}

fn window_ime_purpose(options: &TextInputOptions) -> ImePurpose {
    if options.is_secure() {
        return ImePurpose::Password;
    }
    match options.purpose() {
        TextInputPurpose::Email => ImePurpose::Email,
        TextInputPurpose::Url => ImePurpose::Url,
        TextInputPurpose::Telephone => ImePurpose::Phone,
        TextInputPurpose::Number | TextInputPurpose::Decimal => ImePurpose::Number,
        TextInputPurpose::Password => ImePurpose::Password,
        TextInputPurpose::Pin => ImePurpose::Pin,
        TextInputPurpose::Terminal => ImePurpose::Terminal,
        TextInputPurpose::Text | TextInputPurpose::Search | TextInputPurpose::Name => {
            ImePurpose::Normal
        }
    }
}

fn window_ime_composition_selection(
    preedit: &str,
    selection: Option<(usize, usize)>,
) -> TextRange<TextByteOffset> {
    let fallback = preedit.len();
    let (start, end) = selection.unwrap_or((fallback, fallback));
    TextRange::new(
        TextByteOffset(window_ime_preedit_offset(preedit, start)),
        TextByteOffset(window_ime_preedit_offset(preedit, end)),
    )
}

fn window_ime_preedit_offset(preedit: &str, offset: usize) -> u32 {
    let offset = if offset <= preedit.len() && preedit.is_char_boundary(offset) {
        offset
    } else {
        preedit.len()
    };
    u32::try_from(offset).unwrap_or(u32::MAX)
}

fn text_input_commit_from_key_text(text: &str) -> Option<TextInputOperation> {
    if text.is_empty() || text.chars().all(char::is_control) {
        return None;
    }
    Some(TextInputOperation::Commit(TextCommit::new(text)))
}

fn shortcut_command_from_key(key: &Key, modifiers: ModifiersState) -> Option<TextEditCommand> {
    if !shortcut_modifier_active(modifiers) {
        return None;
    }
    let Key::Character(value) = key else {
        return None;
    };
    if value.eq_ignore_ascii_case("a") {
        Some(TextEditCommand::SelectAll)
    } else if value.eq_ignore_ascii_case("c") {
        Some(TextEditCommand::Copy)
    } else if value.eq_ignore_ascii_case("x") {
        Some(TextEditCommand::Cut)
    } else if value.eq_ignore_ascii_case("v") {
        Some(TextEditCommand::Paste)
    } else {
        None
    }
}

fn left_arrow_text_command(modifiers: ModifiersState, selecting: bool) -> TextEditCommand {
    if modifiers.meta_key() {
        TextEditCommand::MoveLineStart { selecting }
    } else if modifiers.control_key() || modifiers.alt_key() {
        TextEditCommand::MoveWordLeft { selecting }
    } else {
        TextEditCommand::MoveLeft { selecting }
    }
}

fn right_arrow_text_command(modifiers: ModifiersState, selecting: bool) -> TextEditCommand {
    if modifiers.meta_key() {
        TextEditCommand::MoveLineEnd { selecting }
    } else if modifiers.control_key() || modifiers.alt_key() {
        TextEditCommand::MoveWordRight { selecting }
    } else {
        TextEditCommand::MoveRight { selecting }
    }
}

fn shortcut_modifier_active(modifiers: ModifiersState) -> bool {
    modifiers.control_key() || modifiers.meta_key()
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

fn load_native_player_session_save(
    runtime: &mut WindowedRuntimeOwner,
    path: Option<&Path>,
) -> Result<Option<InputControllerSnapshot>, NativeSceneWindowError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let bytes = fs::read(path).map_err(|source| NativeSceneWindowError::SessionSaveIo {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;
    let save = arcweft_save::decode_typed_json_save::<NativePlayerSessionSave>(
        &bytes,
        &arcweft_save::SaveSchemaId::new(NATIVE_PLAYER_SESSION_SAVE_SCHEMA_ID),
        NATIVE_PLAYER_SESSION_SAVE_SCHEMA_VERSION,
        &arcweft_save::SaveDecodeOptions::default(),
    )
    .map_err(
        |error| NativeSceneWindowError::NativePlayerSessionSaveDecode {
            message: error.to_string(),
        },
    )?;
    save.schema.validate()?;
    runtime.session_mut().import_session_save_bytes(
        &save.runtime_session,
        &arcweft_save::SaveDecodeOptions::default(),
    )?;
    Ok(Some(save.input))
}

fn restored_windowed_runtime_and_input(
    bundle: &ArcweftBundle,
    backend: NativeDesktopBackend,
    session_load: Option<&Path>,
) -> Result<(WindowedRuntimeOwner, InputController), NativeSceneWindowError> {
    let mut runtime = WindowedRuntimeOwner::from_bundle_with_desktop_backend(
        bundle,
        BundleSessionOptions::default(),
        backend,
    )?;
    let input_snapshot = load_native_player_session_save(&mut runtime, session_load)?;
    let mut input = InputController::default();
    if let Some(snapshot) = input_snapshot {
        input.restore_snapshot(snapshot)?;
    }
    Ok((runtime, input))
}

fn save_native_player_session(
    path: &Path,
    runtime: &WindowedRuntimeOwner,
    input: &InputController,
) -> Result<(), NativeSceneWindowError> {
    let save = NativePlayerSessionSave {
        schema: NativePlayerSessionSaveSchema::default(),
        runtime_session: runtime.session().export_session_save_bytes()?,
        input: input.snapshot(),
    };
    let bytes = arcweft_save::encode_typed_json_save(
        &save,
        arcweft_save::SaveSchemaId::new(NATIVE_PLAYER_SESSION_SAVE_SCHEMA_ID),
        NATIVE_PLAYER_SESSION_SAVE_SCHEMA_VERSION,
    )
    .map_err(
        |error| NativeSceneWindowError::NativePlayerSessionSaveEncode {
            message: error.to_string(),
        },
    )?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| NativeSceneWindowError::SessionSaveIo {
            operation: "create parent directory for",
            path: path.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, bytes).map_err(|source| NativeSceneWindowError::SessionSaveIo {
        operation: "write",
        path: path.to_path_buf(),
        source,
    })
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
    use super::{
        surrounding_excerpt_range, text_input_commit_from_key_text,
        window_ime_composition_selection,
    };
    use arcweft_presentation::text_input::{TextByteOffset, TextInputOperation, TextRange};

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
            "self.render(&prepared.frame)?;\n        let patch_outcomes = self.drain_patch_events_after_render_submitted()?;"
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

    #[test]
    fn native_player_session_save_pairs_runtime_and_input_snapshots() {
        let source = include_str!("scene_windowed.rs");

        assert!(source.contains("arcweft.native_player_session"));
        assert!(source.contains("runtime_session: Vec<u8>"));
        assert!(source.contains("input: InputControllerSnapshot"));
        assert!(source.contains("import_session_save_bytes"));
        assert!(source.contains("export_session_save_bytes"));
        assert!(source.contains("input.restore_snapshot(snapshot)?"));
        assert!(source.contains("input.snapshot()"));
        assert!(source.contains("save_session_on_exit()?"));
    }

    #[test]
    fn winit_preedit_selection_uses_utf8_byte_offsets() {
        let selection = window_ime_composition_selection("あい", Some((3, 6)));

        assert_eq!(
            selection,
            TextRange::new(TextByteOffset(3), TextByteOffset(6))
        );
    }

    #[test]
    fn winit_preedit_selection_rejects_non_boundary_offsets_to_end() {
        let selection = window_ime_composition_selection("あい", Some((1, 2)));

        assert_eq!(
            selection,
            TextRange::new(TextByteOffset(6), TextByteOffset(6))
        );
    }

    #[test]
    fn winit_keyboard_text_commits_printable_text_only() {
        let Some(TextInputOperation::Commit(commit)) = text_input_commit_from_key_text("abc")
        else {
            panic!("printable keyboard text should become a commit");
        };
        assert_eq!(commit.text(), "abc");
        assert!(text_input_commit_from_key_text("\r").is_none());
        assert!(text_input_commit_from_key_text("").is_none());
    }

    #[test]
    fn surrounding_excerpt_keeps_cursor_and_anchor_inside_utf8_window() {
        let text = format!("{}東京", "a".repeat(5000));
        let cursor = 5006;
        let anchor = 5003;

        let (start, end) = surrounding_excerpt_range(&text, cursor, anchor).unwrap();

        assert!(start <= anchor);
        assert!(cursor <= end);
        assert!(text.is_char_boundary(start));
        assert!(text.is_char_boundary(end));
        assert!(end - start < 4000);
    }
}
