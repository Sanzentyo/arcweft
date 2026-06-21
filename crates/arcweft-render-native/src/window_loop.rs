use super::{
    NativeWindowError, WindowPage, WindowState, key_advances_page, key_closes_window, redraw,
};
use arcweft_render_text::LineDisplayFrame;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes},
};

const EVENT_LOOP_TICK: Duration = Duration::from_millis(16);

/// User input forwarded from the native renderer to an embedding runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWindowLoopInput {
    Advance,
    CloseRequested,
}

/// Result of one embedding-driver turn.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeWindowLoopControl {
    Continue,
    Present(Box<LineDisplayFrame>),
    Exit,
}

/// Event-loop integration point used by the native player.
///
/// Every method is invoked on the thread that owns the winit event loop. The
/// driver may therefore install a main-thread desktop adapter and pump it from
/// `event_loop_turn` without moving native window handles into Sans I/O state.
pub trait NativeWindowLoopDriver: 'static {
    fn attach_window(&mut self, window: Arc<dyn Window>) -> Result<(), String>;

    fn input(&mut self, input: NativeWindowLoopInput) -> Result<(), String>;

    fn event_loop_turn(&mut self) -> Result<NativeWindowLoopControl, String>;
}

/// Runs a native renderer whose pages are supplied incrementally by an
/// event-loop-owned driver.
pub fn run_driven_frames_window<D>(title: &str, driver: D) -> Result<(), NativeWindowError>
where
    D: NativeWindowLoopDriver,
{
    let event_loop =
        EventLoop::new().map_err(|error| NativeWindowError::EventLoop(error.to_string()))?;
    let driver_error = Arc::new(Mutex::new(None));
    let result = event_loop.run_app(DrivenApplication {
        title: title.to_owned(),
        driver,
        queued_pages: VecDeque::new(),
        window_state: None,
        driver_error: driver_error.clone(),
    });
    result.map_err(|error| NativeWindowError::EventLoop(error.to_string()))?;

    let error = driver_error
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    match error {
        Some(error) => Err(NativeWindowError::Driver(error)),
        None => Ok(()),
    }
}

struct DrivenApplication<D> {
    title: String,
    driver: D,
    queued_pages: VecDeque<WindowPage>,
    window_state: Option<WindowState>,
    driver_error: Arc<Mutex<Option<String>>>,
}

impl<D> DrivenApplication<D>
where
    D: NativeWindowLoopDriver,
{
    fn fail(&self, event_loop: &dyn ActiveEventLoop, error: String) {
        *self
            .driver_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error);
        event_loop.exit();
    }

    fn present(&mut self, frame: &LineDisplayFrame) {
        self.queued_pages.extend(WindowPage::from_frame(frame));
        self.present_next_page();
    }

    fn present_next_page(&mut self) -> bool {
        let Some(page) = self.queued_pages.pop_front() else {
            return false;
        };
        if let Some(state) = self.window_state.as_mut() {
            state.set_page(&page);
        }
        true
    }

    fn forward_input(&mut self, event_loop: &dyn ActiveEventLoop, input: NativeWindowLoopInput) {
        if let Err(error) = self.driver.input(input) {
            self.fail(event_loop, error);
        }
    }
}

impl<D> ApplicationHandler for DrivenApplication<D>
where
    D: NativeWindowLoopDriver,
{
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.window_state.is_some() {
            return;
        }
        let window = match event_loop.create_window(
            WindowAttributes::default()
                .with_surface_size(LogicalSize::new(960.0, 540.0))
                .with_title(self.title.clone()),
        ) {
            Ok(window) => window,
            Err(error) => {
                self.fail(
                    event_loop,
                    format!("failed to create native window: {error}"),
                );
                return;
            }
        };
        let window = Arc::<dyn Window>::from(window);
        if let Err(error) = self.driver.attach_window(window.clone()) {
            self.fail(event_loop, error);
            return;
        }
        let placeholder = WindowPage::plain("");
        self.window_state = Some(pollster::block_on(WindowState::new(
            window,
            event_loop,
            &placeholder,
        )));
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + EVENT_LOOP_TICK));
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::KeyboardInput {
            event,
            is_synthetic: false,
            ..
        } = &event
        {
            if key_closes_window(event) {
                self.forward_input(event_loop, NativeWindowLoopInput::CloseRequested);
                event_loop.exit();
                return;
            }
            if key_advances_page(event) {
                if !self.present_next_page() {
                    self.forward_input(event_loop, NativeWindowLoopInput::Advance);
                }
                return;
            }
        }

        if matches!(&event, WindowEvent::CloseRequested) {
            self.forward_input(event_loop, NativeWindowLoopInput::CloseRequested);
            event_loop.exit();
            return;
        }

        let Some(state) = self.window_state.as_mut() else {
            return;
        };
        match event {
            WindowEvent::SurfaceResized(size) => state.resize(size),
            WindowEvent::RedrawRequested => redraw(state),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.window_state.is_none() {
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + EVENT_LOOP_TICK));
            return;
        }

        match self.driver.event_loop_turn() {
            Ok(NativeWindowLoopControl::Continue) => {}
            Ok(NativeWindowLoopControl::Present(frame)) => self.present(frame.as_ref()),
            Ok(NativeWindowLoopControl::Exit) => {
                event_loop.exit();
                return;
            }
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        }

        if let Some(state) = self.window_state.as_ref()
            && state.has_timed_effects
        {
            state.window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + EVENT_LOOP_TICK));
    }
}
