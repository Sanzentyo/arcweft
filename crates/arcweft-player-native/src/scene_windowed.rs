use crate::NativePlayerError;
use crate::clipboard::NativeClipboardAdapter;
use crate::native_audio::{NativeAudioRuntime, NativePlayerAudioError};
use crate::text_input_bridge::{
    NativeTextInputBridge, NativeTextInputBridgeError, NativeTextInputBridgeOptions,
    NativeTextInputFocusReason, NativeTextInputFocusedControl,
};
use crate::window_driver::{WindowCloseSignal, WinitOwnedWindowDriver};
use crate::windowed_environment_ingress::{
    WindowedEnvironmentIngress, WindowedEnvironmentIngressCommand,
    WindowedEnvironmentIngressCompletion, WindowedEnvironmentIngressConfig,
    WindowedEnvironmentIngressEnvelope, WindowedEnvironmentIngressReceiver,
    WindowedEnvironmentUpdateError,
};
use crate::windowed_ingress::{
    WindowedPatchIngress, WindowedPatchIngressCompletion, WindowedPatchIngressConfig,
    WindowedPatchIngressMessage, WindowedPatchIngressReceiver,
};
use crate::windowed_patch::FrameBoundary;
use crate::windowed_player_ingress::WindowedPlayerIngress;
use crate::windowed_runtime::{
    WindowedRuntimeOutcome, WindowedRuntimeOwner, WindowedRuntimeOwnerError,
};
use arcweft_bundle::ArcweftBundle;
use arcweft_desktop_native::NativeDesktopBackend;
use arcweft_layout::ScalePolicy;
use arcweft_player_scene::dialogue::{DialogueVisualClock, DialogueVisualClockSnapshot};
use arcweft_player_scene::fonts::PlayerFontSet;
use arcweft_player_scene::frame::{
    PlayerFrameError, PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest,
    PlayerPreparedFrame, PlayerPreparedFrameCandidate, ViewGeometryConsumer,
    ViewGeometryConversionError, ViewGeometryConversionField, ViewGeometryPlatform,
    ViewGeometryRuntimeError,
};
use arcweft_player_scene::input::wheel::{
    WheelDelta, WheelNormalizationError, WheelNormalizationPolicy,
};
use arcweft_player_scene::input::{
    DialogueProgress, InputController, InputControllerSnapshot, InputControllerSnapshotError,
    InputOutcome,
};
use arcweft_presentation::input::{KeyPhase, PointerId, ViewportPoint};
use arcweft_presentation::text_input::{
    Capitalization, CompositionEndReason, TextAssistPolicy, TextByteOffset, TextCommit,
    TextCompositionUpdate, TextDeleteUnit, TextEditCommand, TextInput, TextInputClientSnapshot,
    TextInputGeometrySnapshot, TextInputKeyDisposition, TextInputOperation, TextInputOptions,
    TextInputPrivacy, TextInputPurpose, TextInputSerial, TextRange,
};
use arcweft_render_wgpu::geometry::{
    PreparedFrame, PreparedTextInputTarget, RenderPreferences, RenderViewport,
};
use arcweft_render_wgpu::renderer::{
    PreparedSharedRenderSubmission, SharedRenderer, SharedRendererError,
};
use arcweft_runtime_driver::clock::{RuntimeClockError, RuntimeClockStep};
use arcweft_runtime_driver::session::{BundleSessionError, BundleSessionOptions, BundleStepInput};
use arcweft_runtime_driver::session_save::BundleSessionSaveError;
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

mod frame_cycle;
mod input_cycle;

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
    configure_ingress: impl FnOnce(WindowedPlayerIngress),
) -> Result<(), NativePlayerError> {
    run_shared_scene_window_with_ingress("Arcweft Player", bundle, configure_ingress)
        .map_err(|error| NativePlayerError::SceneWindow(error.to_string()))
}

pub fn run_bundle_windowed_with_ingress_and_text_input_options(
    bundle: ArcweftBundle,
    _max_steps: usize,
    text_input_options: NativeTextInputBridgeOptions,
    configure_ingress: impl FnOnce(WindowedPlayerIngress),
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
    configure_ingress: impl FnOnce(WindowedPlayerIngress),
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
    #[error("failed to encode native player session save: {message}")]
    NativePlayerSessionSaveEncode { message: String },
    #[error("failed to decode native player session save: {message}")]
    NativePlayerSessionSaveDecode { message: String },
    #[error("player input snapshot restore failed: {0}")]
    InputSnapshot(#[from] InputControllerSnapshotError),
    #[error("platform wheel input normalization failed: {0}")]
    WheelNormalization(#[from] WheelNormalizationError),
    #[error("windowed runtime owner failed: {0}")]
    RuntimeOwner(#[from] WindowedRuntimeOwnerError),
    #[error("runtime clock failed: {0}")]
    Clock(#[from] RuntimeClockError),
    #[error("player frame failed: {0}")]
    PlayerFrame(Box<PlayerFrameError>),
    #[error("native viewport geometry conversion failed: {0}")]
    GeometryConversion(#[from] ViewGeometryConversionError),
    #[error("native render surface extent must be nonzero, got {width}x{height}")]
    InvalidSurfaceExtent { width: u32, height: u32 },
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

impl From<PlayerFrameError> for NativeSceneWindowError {
    fn from(error: PlayerFrameError) -> Self {
        Self::PlayerFrame(Box::new(error))
    }
}

struct NativeSceneApp {
    title: String,
    bundle: Option<ArcweftBundle>,
    options: NativePlayerOptions,
    state: Option<NativeSceneState>,
    ingress: WindowedPatchIngressReceiver,
    ingress_completion: WindowedPatchIngressCompletion,
    pending_ingress: VecDeque<WindowedPatchIngressMessage>,
    environment_ingress: WindowedEnvironmentIngressReceiver,
    environment_completion: WindowedEnvironmentIngressCompletion,
    pending_environment: VecDeque<WindowedEnvironmentIngressEnvelope>,
    error: Arc<Mutex<Option<String>>>,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "window visibility, surface configuration, IME state, and save completion are independent host lifecycle facts"
)]
struct NativeSceneState {
    window: Arc<dyn Window>,
    close_signal: WindowCloseSignal,
    surface: wgpu::Surface<'static>,
    surface_configured: bool,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: SharedRenderer,
    frame_planner: PlayerFramePlannerState,
    runtime: WindowedRuntimeOwner,
    audio: Option<NativeAudioRuntime>,
    ingress_completion: WindowedPatchIngressCompletion,
    environment_completion: WindowedEnvironmentIngressCompletion,
    input: InputController,
    clipboard: NativeClipboardAdapter,
    keyboard_modifiers: ModifiersState,
    text_input: NativeTextInputBridge,
    window_ime_supported: bool,
    window_ime_enabled: bool,
    next_window_ime_serial: u64,
    frame_fit: PlayerFrameFit,
    session_save_out: Option<PathBuf>,
    session_save_on_exit_completed: bool,
    prepared: Option<arcweft_render_wgpu::geometry::PreparedFrame>,
    pending_environment: VecDeque<WindowedEnvironmentIngressEnvelope>,
    dialogue_visual_clock: DialogueVisualClock,
    started_at: Instant,
    next_tick: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct NativePlayerSessionSave {
    runtime_session: Vec<u8>,
    input: InputControllerSnapshot,
    dialogue_visual_clock: DialogueVisualClockSnapshot,
}

impl NativePlayerSessionSave {
    fn decode(input: &[u8]) -> Result<Self, NativeSceneWindowError> {
        arcweft_save::decode_strict_typed_json_save(
            input,
            &arcweft_save::SaveSchemaId::new(NATIVE_PLAYER_SESSION_SAVE_SCHEMA_ID),
            NATIVE_PLAYER_SESSION_SAVE_SCHEMA_VERSION,
            &arcweft_save::SaveDecodeOptions::default(),
        )
        .map_err(
            |error| NativeSceneWindowError::NativePlayerSessionSaveDecode {
                message: error.to_string(),
            },
        )
    }
}

struct RestoredNativePlayerState {
    input: InputControllerSnapshot,
    dialogue_visual_clock: DialogueVisualClockSnapshot,
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
    configure_ingress: impl FnOnce(WindowedPlayerIngress),
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
    configure_ingress: impl FnOnce(WindowedPlayerIngress),
) -> Result<(), NativeSceneWindowError> {
    let event_loop =
        EventLoop::new().map_err(|error| NativeSceneWindowError::EventLoop(error.to_string()))?;
    let (ingress, ingress_rx) = WindowedPatchIngress::channel(
        event_loop.create_proxy(),
        WindowedPatchIngressConfig::default(),
    );
    let (environment_ingress, environment_ingress_rx) = WindowedEnvironmentIngress::channel(
        event_loop.create_proxy(),
        WindowedEnvironmentIngressConfig::default(),
    );
    let ingress_completion = ingress.completion();
    let environment_completion = environment_ingress.completion();
    configure_ingress(WindowedPlayerIngress::new(ingress, environment_ingress));
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
            environment_ingress: environment_ingress_rx,
            environment_completion: environment_completion.clone(),
            pending_environment: VecDeque::new(),
            error: Arc::clone(&error),
        })
        .map_err(|error| NativeSceneWindowError::EventLoop(error.to_string()))?;
    ingress_completion.close("native player event loop exited");
    environment_completion.close();
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
    fn fail(&mut self, event_loop: &dyn ActiveEventLoop, error: String) {
        self.ingress_completion
            .close(format!("native scene failed: {error}"));
        self.close_environment_ingress();
        *self
            .error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error);
        event_loop.exit();
    }

    fn close_environment_ingress(&mut self) {
        self.environment_completion.close();
        for envelope in self.environment_ingress.drain() {
            envelope.close();
        }
        for envelope in self.pending_environment.drain(..) {
            envelope.close();
        }
        if let Some(state) = self.state.as_mut() {
            for envelope in state.pending_environment.drain(..) {
                envelope.close();
            }
        }
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

    fn drain_environment_messages(&mut self) {
        let messages = self.environment_ingress.drain();
        for message in messages {
            self.environment_completion
                .accepted_by_event_loop(message.sequence(), message.command());
            self.apply_environment_message(message);
        }
    }

    fn drain_pending_environment(&mut self) {
        let pending = std::mem::take(&mut self.pending_environment);
        for message in pending {
            self.apply_environment_message(message);
        }
    }

    fn apply_environment_message(&mut self, message: WindowedEnvironmentIngressEnvelope) {
        if let Some(state) = self.state.as_mut() {
            state.pending_environment.push_back(message);
        } else {
            self.pending_environment.push_back(message);
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
            self.environment_completion.clone(),
            self.options.clone(),
        )) {
            Ok(state) => {
                self.state = Some(state);
                self.drain_pending_ingress();
                self.drain_ingress_messages();
                self.drain_pending_environment();
                self.drain_environment_messages();
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
                    self.environment_completion.close();
                    event_loop.exit();
                    Ok(())
                }
                Err(error) => Err(error),
            },
            WindowEvent::SurfaceResized(size) => {
                state.resize(size).map(|()| state.window.request_redraw())
            }
            WindowEvent::Focused(focused) => {
                state.window.request_redraw();
                state.focus_changed(focused)
            }
            WindowEvent::PointerMoved { position, .. } => state.pointer_move(position),
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
        self.drain_environment_messages();
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.drain_ingress_messages();
        self.drain_environment_messages();
        if let Some(state) = self.state.as_mut() {
            if state.take_close_requested() {
                if let Err(error) = state.save_session_on_exit() {
                    self.fail(event_loop, error.to_string());
                    return;
                }
                self.ingress_completion
                    .close("native player requested close from owned-window adapter");
                self.environment_completion.close();
                event_loop.exit();
                return;
            }
            state.window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + EVENT_LOOP_TICK));
    }
}

fn frame_fit_surface_size(frame_fit: PlayerFrameFit) -> LogicalSize<f64> {
    match frame_fit.scale_policy {
        ScalePolicy::Raw => LogicalSize::new(1280.0, 720.0),
        ScalePolicy::Contain | ScalePolicy::Cover | ScalePolicy::Stretch => LogicalSize::new(
            f64::from(frame_fit.design_width),
            f64::from(frame_fit.design_height),
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

fn scene_aspect_size(
    size: PhysicalSize<u32>,
    frame_fit: PlayerFrameFit,
) -> Result<PhysicalSize<u32>, ViewGeometryConversionError> {
    if size.width == 0 || size.height == 0 {
        return Ok(size);
    }
    let Some((aspect_width, aspect_height)) = frame_fit_aspect(frame_fit) else {
        return Ok(size);
    };
    if u64::from(size.width) * u64::from(aspect_height)
        == u64::from(size.height) * u64::from(aspect_width)
    {
        return Ok(size);
    }
    let width_for_height = (u64::from(size.height) * u64::from(aspect_width)
        + u64::from(aspect_height / 2))
        / u64::from(aspect_height);
    let height_for_width = (u64::from(size.width) * u64::from(aspect_height)
        + u64::from(aspect_width / 2))
        / u64::from(aspect_width);
    let width_for_height =
        u32::try_from(width_for_height).map_err(|_| ViewGeometryConversionError::IndexRange {
            node: None,
            platform: ViewGeometryPlatform::Native,
            consumer: ViewGeometryConsumer::Layout,
            field: ViewGeometryConversionField::ViewportWidth,
            value: width_for_height,
            max: u64::from(u32::MAX),
        })?;
    let height_for_width =
        u32::try_from(height_for_width).map_err(|_| ViewGeometryConversionError::IndexRange {
            node: None,
            platform: ViewGeometryPlatform::Native,
            consumer: ViewGeometryConsumer::Layout,
            field: ViewGeometryConversionField::ViewportHeight,
            value: height_for_width,
            max: u64::from(u32::MAX),
        })?;

    Ok(
        if height_for_width.abs_diff(size.height) <= width_for_height.abs_diff(size.width) {
            PhysicalSize::new(size.width, height_for_width)
        } else {
            PhysicalSize::new(width_for_height, size.height)
        },
    )
}

fn frame_fit_aspect(frame_fit: PlayerFrameFit) -> Option<(u32, u32)> {
    match frame_fit.scale_policy {
        ScalePolicy::Contain | ScalePolicy::Cover => {
            let width = frame_fit.design_width;
            let height = frame_fit.design_height;
            let divisor = gcd(width, height);
            debug_assert_ne!(divisor, 0, "designed frame fits have non-zero extents");
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
) -> Result<Option<RestoredNativePlayerState>, NativeSceneWindowError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let bytes = fs::read(path).map_err(|source| NativeSceneWindowError::SessionSaveIo {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;
    let save = NativePlayerSessionSave::decode(&bytes)?;
    runtime.session_mut().import_session_save_bytes(
        &save.runtime_session,
        &arcweft_save::SaveDecodeOptions::default(),
    )?;
    Ok(Some(RestoredNativePlayerState {
        input: save.input,
        dialogue_visual_clock: save.dialogue_visual_clock,
    }))
}

fn restored_windowed_runtime_and_input(
    bundle: &ArcweftBundle,
    backend: NativeDesktopBackend,
    session_load: Option<&Path>,
) -> Result<(WindowedRuntimeOwner, InputController, DialogueVisualClock), NativeSceneWindowError> {
    let mut runtime = WindowedRuntimeOwner::from_bundle_with_desktop_backend(
        bundle,
        BundleSessionOptions::default(),
        backend,
    )?;
    let restored = load_native_player_session_save(&mut runtime, session_load)?;
    let mut input = InputController::default();
    let mut dialogue_visual_clock = DialogueVisualClock::default();
    if let Some(restored) = restored {
        input.restore_snapshot(restored.input)?;
        dialogue_visual_clock.restore(restored.dialogue_visual_clock, 0);
    }
    Ok((runtime, input, dialogue_visual_clock))
}

fn save_native_player_session(
    path: &Path,
    runtime: &WindowedRuntimeOwner,
    input: &InputController,
    dialogue_visual_clock: &DialogueVisualClock,
    elapsed_millis: u64,
) -> Result<(), NativeSceneWindowError> {
    let save = NativePlayerSessionSave {
        runtime_session: runtime.session().export_session_save_bytes()?,
        input: input.snapshot(),
        dialogue_visual_clock: dialogue_visual_clock.snapshot(elapsed_millis),
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

#[cfg(test)]
mod tests {
    use super::{
        DialogueVisualClockSnapshot, InputControllerSnapshot, NATIVE_PLAYER_SESSION_SAVE_SCHEMA_ID,
        NATIVE_PLAYER_SESSION_SAVE_SCHEMA_VERSION, NativePlayerSessionSave,
        surrounding_excerpt_range, text_input_commit_from_key_text,
        window_ime_composition_selection,
    };
    use arcweft_presentation::text_input::{TextByteOffset, TextInputOperation, TextRange};

    #[test]
    fn native_player_session_save_decode_rejects_unknown_payload_fields() {
        let save = NativePlayerSessionSave {
            runtime_session: Vec::new(),
            input: InputControllerSnapshot::default(),
            dialogue_visual_clock: DialogueVisualClockSnapshot::default(),
        };
        let mut payload = serde_json::to_value(save).unwrap();
        payload
            .as_object_mut()
            .unwrap()
            .insert("predecessor".to_owned(), serde_json::Value::Bool(true));
        let bytes = arcweft_save::SaveEnvelope::new(
            arcweft_save::SaveSchemaId::new(NATIVE_PLAYER_SESSION_SAVE_SCHEMA_ID),
            NATIVE_PLAYER_SESSION_SAVE_SCHEMA_VERSION,
            arcweft_save::TYPED_JSON_CODEC_ID,
            serde_json::to_vec(&payload).unwrap(),
        )
        .encode_bytes()
        .unwrap();

        let error = NativePlayerSessionSave::decode(&bytes).unwrap_err();

        assert!(error.to_string().contains("predecessor"));
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
