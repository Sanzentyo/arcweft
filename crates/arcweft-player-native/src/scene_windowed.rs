use crate::NativePlayerError;
use crate::windowed_patch::FrameBoundary;
use crate::windowed_runtime::{
    WindowedRuntimeOutcome, WindowedRuntimeOwner, WindowedRuntimeOwnerError,
};
use arcweft_bundle::ArcweftBundle;
use arcweft_player_scene::images::BundleImageCatalogError;
use arcweft_player_scene::input::{InputController, InputOutcome};
use arcweft_presentation::input::{KeyPhase, PointerId, ViewportPoint};
use arcweft_render_text::LineDisplayFrame;
use arcweft_render_wgpu::geometry::{
    RenderChoiceItem, RenderDialogue, RenderPreferences, RenderScene, RenderViewport,
    SharedFramePlanner,
};
use arcweft_render_wgpu::renderer::{SharedRenderer, SharedRendererError};
use arcweft_runtime_driver::clock::{RuntimeClockError, RuntimeClockStep};
use arcweft_runtime_driver::session::{BundleSessionError, BundleSessionOptions, BundleStepInput};
use num_traits::ToPrimitive;
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
    state: Option<NativeSceneState>,
    error: Arc<Mutex<Option<String>>>,
}

struct NativeSceneState {
    window: Arc<dyn Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: SharedRenderer,
    runtime: WindowedRuntimeOwner,
    input: InputController,
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
    let event_loop =
        EventLoop::new().map_err(|error| NativeSceneWindowError::EventLoop(error.to_string()))?;
    let error = Arc::new(Mutex::new(None));
    event_loop
        .run_app(NativeSceneApp {
            title: title.to_owned(),
            bundle: Some(bundle),
            state: None,
            error: Arc::clone(&error),
        })
        .map_err(|error| NativeSceneWindowError::EventLoop(error.to_string()))?;
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
        *self
            .error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error);
        event_loop.exit();
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
        match pollster::block_on(NativeSceneState::new(window, bundle)) {
            Ok(state) => self.state = Some(state),
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
                event_loop.exit();
                Ok(())
            }
            WindowEvent::SurfaceResized(size) => {
                state.resize(size);
                state.window.request_redraw();
                Ok(())
            }
            WindowEvent::Focused(focused) => {
                state.input.focus_changed(focused);
                state.window.request_redraw();
                Ok(())
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
                state.keyboard(&event.logical_key, event.state);
                Ok(())
            }
            WindowEvent::RedrawRequested => state.redraw(),
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, error.to_string());
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(state) = self.state.as_ref() {
            state.window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + EVENT_LOOP_TICK));
    }
}

impl NativeSceneState {
    async fn new(
        window: Arc<dyn Window>,
        bundle: ArcweftBundle,
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
        let runtime = WindowedRuntimeOwner::from_bundle(&bundle, BundleSessionOptions::default())?;
        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            runtime,
            input: InputController::default(),
            prepared: None,
            dialogue_visual_clock: DialogueVisualClock::default(),
            started_at: Instant::now(),
            next_tick: 1,
        })
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
        self.step_runtime()?;
        let prepared = self.prepare_frame()?;
        self.input.ensure_choice_focus(&prepared);
        let prepared = self.prepare_frame_with_interaction()?;
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
        let clock = RuntimeClockStep::from_millis(self.next_tick, 16)?;
        self.next_tick = self.next_tick.saturating_add(1);
        let _step = self
            .runtime
            .session_mut()
            .step_with_clock(clock, BundleStepInput::default());
        Ok(())
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

    fn keyboard(&mut self, key: &Key, element_state: ElementState) {
        let Some(frame) = self.prepared.clone() else {
            return;
        };
        let phase = match element_state {
            ElementState::Pressed => KeyPhase::Down,
            ElementState::Released => KeyPhase::Up,
        };
        let outcome = self.input.keyboard(&frame, &key_label(key), phase);
        let _ = self.apply_outcome(outcome);
        self.window.request_redraw();
    }

    fn apply_outcome(&mut self, outcome: InputOutcome) -> Result<(), NativeSceneWindowError> {
        for action in outcome.actions {
            self.runtime.session_mut().queue_semantic_action(&action)?;
        }
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

fn key_label(key: &Key) -> String {
    match key {
        Key::Named(NamedKey::ArrowUp) => "ArrowUp".to_owned(),
        Key::Named(NamedKey::ArrowDown) => "ArrowDown".to_owned(),
        Key::Named(NamedKey::ArrowLeft) => "ArrowLeft".to_owned(),
        Key::Named(NamedKey::ArrowRight) => "ArrowRight".to_owned(),
        Key::Named(NamedKey::Enter) => "Enter".to_owned(),
        Key::Named(NamedKey::Home) => "Home".to_owned(),
        Key::Named(NamedKey::End) => "End".to_owned(),
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
