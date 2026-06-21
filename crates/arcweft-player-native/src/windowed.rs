use crate::window_driver::{WindowCloseSignal, WinitOwnedWindowDriver};
use crate::{NativePlayerError, append_display_frames};
use arcweft_adapter_desktop::DesktopAdapterSet;
use arcweft_bundle::ArcweftBundle;
use arcweft_desktop_native::NativeDesktopBackend;
use arcweft_render_native::{
    NativeWindowLoopControl, NativeWindowLoopDriver, NativeWindowLoopInput,
    run_driven_frames_window,
};
use arcweft_render_text::{LineDisplayCatalog, LineDisplayFrame};
use arcweft_runtime_host::{
    BundleRunnerExecutor, BundleRunnerOptions, BundleRunnerSession, BundleRunnerStepMode,
};
use std::collections::VecDeque;
use std::sync::Arc;
use winit::window::Window;

/// Runs a bundle inside the native event loop and installs owned-window/cursor
/// support only after the renderer has created its primary window.
pub fn run_bundle_windowed(
    bundle: ArcweftBundle,
    max_steps: usize,
) -> Result<(), NativePlayerError> {
    run_driven_frames_window("Arcweft Player", BundleWindowDriver::new(bundle, max_steps))?;
    Ok(())
}

struct BundleWindowDriver {
    bundle: Option<ArcweftBundle>,
    display: LineDisplayCatalog,
    max_steps: usize,
    session: Option<BundleRunnerSession>,
    pending_frames: VecDeque<LineDisplayFrame>,
    diagnostics: Vec<String>,
    waiting_for_advance: bool,
    exit_requested: bool,
    close_signal: WindowCloseSignal,
}

impl BundleWindowDriver {
    fn new(bundle: ArcweftBundle, max_steps: usize) -> Self {
        Self {
            display: bundle.display.clone(),
            bundle: Some(bundle),
            max_steps,
            session: None,
            pending_frames: VecDeque::new(),
            diagnostics: Vec::new(),
            waiting_for_advance: false,
            exit_requested: false,
            close_signal: WindowCloseSignal::default(),
        }
    }

    fn session(&self) -> Result<&BundleRunnerSession, String> {
        self.session
            .as_ref()
            .ok_or_else(|| "native runtime session has not been attached to a window".to_owned())
    }

    fn session_mut(&mut self) -> Result<&mut BundleRunnerSession, String> {
        self.session
            .as_mut()
            .ok_or_else(|| "native runtime session has not been attached to a window".to_owned())
    }

    fn present_pending(&mut self) -> Option<NativeWindowLoopControl> {
        self.pending_frames.pop_front().map(|frame| {
            self.waiting_for_advance = true;
            NativeWindowLoopControl::Present(Box::new(frame))
        })
    }

    fn runtime_finished(&self) -> Result<bool, String> {
        self.session().map(BundleRunnerSession::is_finished)
    }
}

impl NativeWindowLoopDriver for BundleWindowDriver {
    fn attach_window(&mut self, window: Arc<dyn Window>) -> Result<(), String> {
        let bundle = self
            .bundle
            .take()
            .ok_or_else(|| "native bundle window was attached twice".to_owned())?;
        let owned_window = Arc::new(WinitOwnedWindowDriver::try_new(
            window,
            "Arcweft Player",
            self.close_signal.clone(),
        )?);
        let options = BundleRunnerOptions {
            steps: self.max_steps,
            mode: BundleRunnerStepMode::Game,
            max_ops: 64,
            executor: BundleRunnerExecutor::BytecodeVm,
            ..BundleRunnerOptions::default()
        };
        let session = BundleRunnerSession::with_adapter_installer(
            &bundle,
            &options,
            move |_source_path, builder| {
                let backend = NativeDesktopBackend::builder()
                    .with_owned_window_driver(owned_window)
                    .build();
                let (builder, _coordinator) =
                    DesktopAdapterSet::bind_current_thread(backend).register(builder)?;
                Ok(builder)
            },
        )
        .map_err(|error| error.to_string())?;
        self.session = Some(session);
        Ok(())
    }

    fn input(&mut self, input: NativeWindowLoopInput) -> Result<(), String> {
        match input {
            NativeWindowLoopInput::Advance => {
                self.waiting_for_advance = false;
                if self.pending_frames.is_empty() && self.runtime_finished()? {
                    self.exit_requested = true;
                }
            }
            NativeWindowLoopInput::CloseRequested => {
                self.exit_requested = true;
            }
        }
        Ok(())
    }

    fn event_loop_turn(&mut self) -> Result<NativeWindowLoopControl, String> {
        if self.exit_requested || self.close_signal.take() {
            return Ok(NativeWindowLoopControl::Exit);
        }

        self.session_mut()?
            .pump_main_thread()
            .map_err(|error| error.to_string())?;
        if self.close_signal.take() {
            return Ok(NativeWindowLoopControl::Exit);
        }
        if self.waiting_for_advance {
            return Ok(NativeWindowLoopControl::Continue);
        }
        if let Some(control) = self.present_pending() {
            return Ok(control);
        }
        if self.runtime_finished()? {
            return Ok(NativeWindowLoopControl::Exit);
        }

        if let Some(step) = self
            .session_mut()?
            .step()
            .map_err(|error| error.to_string())?
        {
            self.diagnostics
                .extend(step.summary.diagnostics.iter().cloned());
            let mut frames = Vec::new();
            append_display_frames(
                &self.display,
                &step.summary.flow_events,
                &mut frames,
                &mut self.diagnostics,
            );
            self.pending_frames.extend(frames);
        }

        if let Some(control) = self.present_pending() {
            return Ok(control);
        }
        if self.runtime_finished()? {
            Ok(NativeWindowLoopControl::Exit)
        } else {
            Ok(NativeWindowLoopControl::Continue)
        }
    }
}
