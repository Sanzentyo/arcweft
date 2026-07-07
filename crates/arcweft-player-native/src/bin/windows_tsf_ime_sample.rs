//! Windows TSF real IME sample for seq06.4b.2.
//!
//! Run on Windows with:
//!
//! ```text
//! cargo run -p arcweft-player-native --bin windows-tsf-ime-sample -- --trace-out fixtures/windows-tsf-real-ime/microsoft-japanese-ime-hiragana.real.json
//! ```

#[cfg(not(target_os = "windows"))]
fn main() -> std::process::ExitCode {
    eprintln!(
        "windows-tsf-ime-sample is Windows-only; use this binary on a Windows host with Microsoft Japanese IME enabled"
    );
    std::process::ExitCode::FAILURE
}

#[cfg(target_os = "windows")]
fn main() -> std::process::ExitCode {
    windows_sample::run_main()
}

#[cfg(target_os = "windows")]
mod windows_sample {
    use arcweft_desktop_native::text_input::windows_tsf::real_ime::{
        WindowsTsfImeBridge, WindowsTsfImeError,
    };
    use arcweft_id::PublicId;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::input::{InputEpoch, InteractionTarget};
    use arcweft_presentation::text_editor::{
        TextEditorLayout, TextEditorLocalClipboard, TextEditorState,
    };
    use arcweft_presentation::text_input::{
        TextInputBlurPolicy, TextInputFocusGeneration, TextInputKeyDisposition, TextInputOptions,
        TextInputPrivacy, TextInputSessionId,
    };
    use arcweft_runtime_host::text_input_dispatch::TextInputDispatchState;
    use clap::Parser;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use serde::Serialize;
    use std::ffi::c_void;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use thiserror::Error;
    use winit::application::ApplicationHandler;
    use winit::event::{ElementState, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::keyboard::{Key, NamedKey};
    use winit::window::{Window, WindowAttributes, WindowId};

    #[derive(Debug, Parser)]
    #[command(name = "windows-tsf-ime-sample")]
    struct Args {
        /// Write a redacted Windows TSF IME trace when the sample window closes.
        #[arg(long)]
        trace_out: Option<PathBuf>,
    }

    #[derive(Debug, Error)]
    enum SampleError {
        #[error("winit event loop failed: {0}")]
        EventLoop(String),
        #[error("winit window failed: {0}")]
        Window(String),
        #[error(transparent)]
        Tsf(#[from] WindowsTsfImeError),
        #[error("text editor failed: {0}")]
        Editor(String),
        #[error("runtime dispatch failed: {0}")]
        Dispatch(String),
        #[error("trace write failed: {0}")]
        Trace(String),
    }

    #[derive(Clone, Debug, Serialize)]
    struct TraceFile {
        metadata: TraceMetadata,
        events: Vec<TraceEvent>,
    }

    #[derive(Clone, Debug, Serialize)]
    struct TraceMetadata {
        adapter: &'static str,
        os: &'static str,
        ime: &'static str,
        arcweft_sequence: &'static str,
        secure_redaction: bool,
    }

    #[derive(Clone, Debug, Serialize)]
    struct TraceEvent {
        index: usize,
        control: &'static str,
        kind: String,
        session: u64,
        generation: u64,
        serial: Option<u64>,
        revision: u64,
        redacted: bool,
        text_len: usize,
    }

    struct SampleApp {
        args: Args,
        state: Option<SampleState>,
        error: Arc<Mutex<Option<String>>>,
    }

    struct SampleState {
        window: Arc<dyn Window>,
        bridge: WindowsTsfImeBridge,
        dispatch: TextInputDispatchState,
        controls: Vec<SampleControl>,
        active: usize,
        generation: TextInputFocusGeneration,
        trace: Vec<TraceEvent>,
        clipboard: TextEditorLocalClipboard,
    }

    struct SampleControl {
        name: &'static str,
        editor: TextEditorState,
        layout: TextEditorLayout,
        secure: bool,
    }

    pub fn run_main() -> std::process::ExitCode {
        match run(Args::parse()) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                std::process::ExitCode::FAILURE
            }
        }
    }

    fn run(args: Args) -> Result<(), SampleError> {
        let event_loop =
            EventLoop::new().map_err(|error| SampleError::EventLoop(error.to_string()))?;
        let error = Arc::new(Mutex::new(None));
        event_loop
            .run_app(SampleApp {
                args,
                state: None,
                error: Arc::clone(&error),
            })
            .map_err(|error| SampleError::EventLoop(error.to_string()))?;
        let error = error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(error) = error {
            Err(SampleError::Window(error))
        } else {
            Ok(())
        }
    }

    impl SampleApp {
        fn fail(&self, event_loop: &dyn ActiveEventLoop, error: String) {
            *self
                .error
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error);
            event_loop.exit();
        }
    }

    impl ApplicationHandler for SampleApp {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            if self.state.is_some() {
                return;
            }
            let window = match event_loop.create_window(
                WindowAttributes::default().with_title("Arcweft Windows TSF IME sample"),
            ) {
                Ok(window) => Arc::<dyn Window>::from(window),
                Err(error) => {
                    self.fail(event_loop, error.to_string());
                    return;
                }
            };
            match SampleState::new(window).and_then(|mut state| {
                state.focus_active()?;
                Ok(state)
            }) {
                Ok(state) => self.state = Some(state),
                Err(error) => {
                    self.fail(event_loop, error.to_string());
                }
            }
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
                    let result = self
                        .args
                        .trace_out
                        .as_ref()
                        .map_or(Ok(()), |path| state.write_trace(path));
                    if result.is_ok() {
                        event_loop.exit();
                    }
                    result
                }
                WindowEvent::Focused(true) => state.focus_active(),
                WindowEvent::Focused(false) => state
                    .bridge
                    .blur(TextInputBlurPolicy::PlatformDefault)
                    .map_err(Into::into),
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed
                        && event.logical_key == Key::Named(NamedKey::Tab) =>
                {
                    state.next_focus()
                }
                WindowEvent::RedrawRequested => state.poll_tsf_events(),
                _ => Ok(()),
            };
            if let Err(error) = result {
                self.fail(event_loop, error.to_string());
            }
        }
    }

    impl SampleState {
        fn new(window: Arc<dyn Window>) -> Result<Self, SampleError> {
            let handle = window
                .window_handle()
                .map_err(|error| SampleError::Window(error.to_string()))?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err(SampleError::Window(
                    "Windows TSF sample expected a Win32 HWND".to_owned(),
                ));
            };
            let hwnd = windows::Win32::Foundation::HWND(handle.hwnd.get() as *mut c_void);
            let bridge = WindowsTsfImeBridge::new_for_window(hwnd)?;
            Ok(Self {
                window,
                bridge,
                dispatch: TextInputDispatchState::default(),
                controls: vec![
                    SampleControl::new(
                        "TextField",
                        "textfield.windows.real",
                        false,
                        false,
                        HitRect::new(24.0, 32.0, 520.0, 28.0),
                    )?,
                    SampleControl::new(
                        "TextArea",
                        "textarea.windows.real",
                        true,
                        false,
                        HitRect::new(24.0, 84.0, 520.0, 88.0),
                    )?,
                    SampleControl::new(
                        "SecureField",
                        "secure.windows.real",
                        false,
                        true,
                        HitRect::new(24.0, 196.0, 520.0, 28.0),
                    )?,
                ],
                active: 0,
                generation: TextInputFocusGeneration::default(),
                trace: Vec::new(),
                clipboard: TextEditorLocalClipboard::default(),
            })
        }

        fn focus_active(&mut self) -> Result<(), SampleError> {
            self.generation = self.generation.next();
            let control = &self.controls[self.active];
            let snapshots = control
                .editor
                .snapshots(&control.layout)
                .map_err(|error| SampleError::Editor(error.to_string()))?;
            let activation = self.bridge.focus_text_input(
                snapshots.client(),
                self.generation,
                Some(snapshots.geometry()),
            )?;
            let transaction = self.dispatch.activate_with_capabilities(
                snapshots.client(),
                activation
                    .activation()
                    .capabilities()
                    .to_text_input_capabilities(),
            );
            debug_assert!(!transaction.commands().is_empty());
            self.trace_event("activate", None);
            Ok(())
        }

        fn next_focus(&mut self) -> Result<(), SampleError> {
            self.bridge.blur(TextInputBlurPolicy::PlatformDefault)?;
            self.active = (self.active + 1) % self.controls.len();
            self.focus_active()
        }

        fn poll_tsf_events(&mut self) -> Result<(), SampleError> {
            let events = self.bridge.drain_platform_events();
            for event in events {
                let serial = Some(event.context().serial().0);
                let privacy = if self.controls[self.active].secure {
                    TextInputPrivacy::Sensitive
                } else {
                    TextInputPrivacy::Plain
                };
                let input = event.clone().into_text_input(privacy);
                let _outputs = self.controls[self.active]
                    .editor
                    .apply_text_input(&input, &mut self.clipboard)
                    .map_err(|error| SampleError::Editor(error.to_string()))?;
                self.dispatch
                    .dispatch_platform_event(
                        InputEpoch::default(),
                        event,
                        TextInputKeyDisposition::ImeConsumed,
                    )
                    .map_err(|error| SampleError::Dispatch(error.to_string()))?;
                self.trace_event("platform_text_input", serial);
            }
            Ok(())
        }

        fn trace_event(&mut self, kind: &str, serial: Option<u64>) {
            let control = &self.controls[self.active];
            self.trace.push(TraceEvent {
                index: self.trace.len(),
                control: control.name,
                kind: kind.to_owned(),
                session: control.editor.session().0,
                generation: self.generation.0,
                serial,
                revision: control.editor.revision().0,
                redacted: control.secure,
                text_len: if control.secure {
                    0
                } else {
                    control.editor.text().len()
                },
            });
        }

        fn write_trace(&self, path: &PathBuf) -> Result<(), SampleError> {
            let trace = TraceFile {
                metadata: TraceMetadata {
                    adapter: "windows_tsf",
                    os: "windows",
                    ime: "microsoft_japanese_ime",
                    arcweft_sequence: "seq06.4b.2",
                    secure_redaction: true,
                },
                events: self.trace.clone(),
            };
            let bytes = serde_json::to_vec_pretty(&trace)
                .map_err(|error| SampleError::Trace(error.to_string()))?;
            fs::write(path, bytes).map_err(|error| SampleError::Trace(error.to_string()))
        }
    }

    impl SampleControl {
        fn new(
            name: &'static str,
            target: &str,
            multiline: bool,
            secure: bool,
            bounds: HitRect,
        ) -> Result<Self, SampleError> {
            let options = TextInputOptions::default()
                .multiline(multiline)
                .secure(secure);
            let session = TextInputSessionId(if secure {
                3
            } else if multiline {
                2
            } else {
                1
            });
            let editor = TextEditorState::new(
                session,
                InteractionTarget::new(
                    PublicId::try_new(target)
                        .map_err(|error| SampleError::Editor(error.to_string()))?,
                ),
                "",
                options,
            )
            .map_err(|error| SampleError::Editor(error.to_string()))?;
            let layout = TextEditorLayout::monospaced_fixture(bounds);
            Ok(Self {
                name,
                editor,
                layout,
                secure,
            })
        }
    }
}
