use arcweft_desktop_contract::{
    CursorGrabMode, CursorIcon, DesktopError, DesktopFeature, OwnedCursorRequest,
    OwnedWindowRequest, OwnedWindowResponse, PhysicalPosition, PhysicalRect, PhysicalSize,
    PlatformKind, ScaleFactor, WindowId, WindowMode, WindowScope, WindowSnapshot, WindowTarget,
};
use arcweft_desktop_native::{OwnedWindowDriver, native_platform_kind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use winit::{
    cursor::{Cursor, CursorIcon as WinitCursorIcon},
    dpi::{PhysicalPosition as WinitPhysicalPosition, PhysicalSize as WinitPhysicalSize},
    monitor::Fullscreen,
    window::{CursorGrabMode as WinitCursorGrabMode, Window},
};

#[derive(Clone, Debug, Default)]
pub(crate) struct WindowCloseSignal(Arc<AtomicBool>);

impl WindowCloseSignal {
    pub(crate) fn request(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(crate) fn take(&self) -> bool {
        self.0.swap(false, Ordering::AcqRel)
    }
}

#[derive(Debug)]
struct DriverState {
    title: String,
    visible: bool,
    mode: WindowMode,
    cursor_visible: bool,
    cursor_hidden_by_icon: bool,
}

/// Owned-window adapter backed by the winit window created by the native
/// renderer's event loop.
pub(crate) struct WinitOwnedWindowDriver {
    port: Arc<dyn WindowPort>,
    id: WindowId,
    platform: PlatformKind,
    state: Mutex<DriverState>,
    close_signal: WindowCloseSignal,
}

impl WinitOwnedWindowDriver {
    pub(crate) fn try_new(
        window: Arc<dyn Window>,
        title: impl Into<String>,
        close_signal: WindowCloseSignal,
    ) -> Result<Self, String> {
        Self::from_port(
            Arc::new(WinitWindowPort { window }),
            title.into(),
            close_signal,
            native_platform_kind(),
        )
    }

    fn from_port(
        port: Arc<dyn WindowPort>,
        title: String,
        close_signal: WindowCloseSignal,
        platform: PlatformKind,
    ) -> Result<Self, String> {
        let id = WindowId::try_new("owned:primary")
            .map_err(|error| format!("failed to create owned-window id: {error}"))?;
        let visible = port.is_visible().unwrap_or(true);
        let mode = current_mode(port.as_ref(), WindowMode::Normal);
        Ok(Self {
            port,
            id,
            platform,
            state: Mutex::new(DriverState {
                title,
                visible,
                mode,
                cursor_visible: true,
                cursor_hidden_by_icon: false,
            }),
            close_signal,
        })
    }

    fn state(&self) -> MutexGuard<'_, DriverState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn ensure_target(&self, target: &WindowTarget) -> Result<(), DesktopError> {
        match target {
            WindowTarget::PrimaryOwned => Ok(()),
            WindowTarget::Owned(id) if id == &self.id => Ok(()),
            WindowTarget::Owned(id) => Err(DesktopError::StaleHandle {
                handle: id.to_string(),
            }),
            WindowTarget::External(id) => Err(DesktopError::InvalidArgument {
                field: "target".to_owned(),
                detail: format!(
                    "external window `{id}` cannot be used with an owned-window request"
                ),
            }),
        }
    }

    fn snapshot(&self) -> WindowSnapshot {
        let (title, visible, requested_mode) = {
            let state = self.state();
            (state.title.clone(), state.visible, state.mode)
        };
        let outer_size = self.port.outer_size();
        let bounds = self
            .port
            .outer_position()
            .ok()
            .map(|position| PhysicalRect {
                position: PhysicalPosition {
                    x: position.x,
                    y: position.y,
                },
                size: PhysicalSize {
                    width: outer_size.width,
                    height: outer_size.height,
                },
            });
        WindowSnapshot {
            id: self.id.clone(),
            scope: WindowScope::Owned,
            title: Some(window_title(self.port.as_ref(), title)),
            application_name: application_name(),
            process_id: Some(std::process::id()),
            bounds,
            scale_factor: ScaleFactor::from_f64(self.port.scale_factor()).ok(),
            mode: current_mode(self.port.as_ref(), requested_mode),
            visible: self.port.is_visible().or(Some(visible)),
            focused: Some(self.port.has_focus()),
        }
    }

    fn set_mode(&self, mode: WindowMode) -> Result<(), DesktopError> {
        match mode {
            WindowMode::Normal => {
                self.port
                    .set_fullscreen(None)
                    .map_err(|error| Self::platform_error("owned_window_fullscreen", error))?;
                self.port.set_minimized(false);
                self.port.set_maximized(false);
            }
            WindowMode::Minimized => {
                self.port
                    .set_fullscreen(None)
                    .map_err(|error| Self::platform_error("owned_window_fullscreen", error))?;
                self.port.set_maximized(false);
                self.port.set_minimized(true);
            }
            WindowMode::Maximized => {
                self.port
                    .set_fullscreen(None)
                    .map_err(|error| Self::platform_error("owned_window_fullscreen", error))?;
                self.port.set_minimized(false);
                self.port.set_maximized(true);
            }
            WindowMode::BorderlessFullscreen => {
                self.port.set_minimized(false);
                self.port.set_maximized(false);
                self.port
                    .set_fullscreen(Some(NativeFullscreenMode::Borderless))
                    .map_err(|error| Self::platform_error("owned_window_fullscreen", error))?;
            }
            WindowMode::Fullscreen => {
                self.port.set_minimized(false);
                self.port.set_maximized(false);
                self.port
                    .set_fullscreen(Some(NativeFullscreenMode::Exclusive))
                    .map_err(|error| Self::platform_error("owned_window_fullscreen", error))?;
            }
        }
        self.state().mode = mode;
        Ok(())
    }

    fn set_bounds(&self, bounds: PhysicalRect) -> Result<(), DesktopError> {
        if bounds.size.width == 0 || bounds.size.height == 0 {
            return Err(DesktopError::InvalidArgument {
                field: "bounds.size".to_owned(),
                detail: "owned window dimensions must be greater than zero".to_owned(),
            });
        }
        if !self.supports_absolute_position() {
            return Err(DesktopError::Unsupported {
                feature: DesktopFeature::OwnedWindowAbsolutePosition,
                platform: self.platform,
                detail: "the active window system does not expose absolute placement".to_owned(),
            });
        }
        self.port.set_outer_position(WinitPhysicalPosition::new(
            bounds.position.x,
            bounds.position.y,
        ));

        let outer = self.port.outer_size();
        let surface = self.port.surface_size();
        let decoration_width = outer.width.saturating_sub(surface.width);
        let decoration_height = outer.height.saturating_sub(surface.height);
        let requested_surface = WinitPhysicalSize::new(
            bounds.size.width.saturating_sub(decoration_width).max(1),
            bounds.size.height.saturating_sub(decoration_height).max(1),
        );
        self.port.request_surface_size(requested_surface);
        Ok(())
    }

    fn platform_error(operation: &'static str, detail: String) -> DesktopError {
        DesktopError::Platform {
            operation: operation.to_owned(),
            code: None,
            detail,
        }
    }
}

impl OwnedWindowDriver for WinitOwnedWindowDriver {
    fn execute_window(
        &self,
        request: OwnedWindowRequest,
    ) -> Result<OwnedWindowResponse, DesktopError> {
        match request {
            OwnedWindowRequest::List => Ok(OwnedWindowResponse::Windows(vec![self.snapshot()])),
            OwnedWindowRequest::Get { target } => {
                self.ensure_target(&target)?;
                Ok(OwnedWindowResponse::Window(self.snapshot()))
            }
            OwnedWindowRequest::SetTitle { target, title } => {
                self.ensure_target(&target)?;
                self.port.set_title(&title);
                self.state().title = title;
                Ok(OwnedWindowResponse::Applied)
            }
            OwnedWindowRequest::SetVisible { target, visible } => {
                self.ensure_target(&target)?;
                self.port.set_visible(visible);
                self.state().visible = visible;
                Ok(OwnedWindowResponse::Applied)
            }
            OwnedWindowRequest::SetMode { target, mode } => {
                self.ensure_target(&target)?;
                self.set_mode(mode)?;
                Ok(OwnedWindowResponse::Applied)
            }
            OwnedWindowRequest::SetBounds { target, bounds } => {
                self.ensure_target(&target)?;
                self.set_bounds(bounds)?;
                Ok(OwnedWindowResponse::Applied)
            }
            OwnedWindowRequest::RequestFocus { target } => {
                self.ensure_target(&target)?;
                self.port.focus_window();
                Ok(OwnedWindowResponse::Applied)
            }
            OwnedWindowRequest::RequestClose { target } => {
                self.ensure_target(&target)?;
                self.close_signal.request();
                Ok(OwnedWindowResponse::Applied)
            }
        }
    }

    fn execute_cursor(&self, request: OwnedCursorRequest) -> Result<(), DesktopError> {
        match request {
            OwnedCursorRequest::SetIcon { target, icon } => {
                self.ensure_target(&target)?;
                let effective_visible = {
                    let mut state = self.state();
                    state.cursor_hidden_by_icon = icon == CursorIcon::Hidden;
                    state.cursor_visible && !state.cursor_hidden_by_icon
                };
                if let Some(icon) = winit_cursor_icon(icon) {
                    self.port.set_cursor_icon(icon);
                }
                self.port.set_cursor_visible(effective_visible);
                Ok(())
            }
            OwnedCursorRequest::SetVisible { target, visible } => {
                self.ensure_target(&target)?;
                let effective_visible = {
                    let mut state = self.state();
                    state.cursor_visible = visible;
                    state.cursor_visible && !state.cursor_hidden_by_icon
                };
                self.port.set_cursor_visible(effective_visible);
                Ok(())
            }
            OwnedCursorRequest::SetGrab { target, mode } => {
                self.ensure_target(&target)?;
                self.port
                    .set_cursor_grab(winit_grab_mode(mode))
                    .map_err(|error| Self::platform_error("owned_cursor_grab", error))
            }
            OwnedCursorRequest::SetPosition { target, position } => {
                self.ensure_target(&target)?;
                self.port
                    .set_cursor_position(WinitPhysicalPosition::new(position.x, position.y))
                    .map_err(|error| Self::platform_error("owned_cursor_position", error))
            }
        }
    }

    fn supports_absolute_position(&self) -> bool {
        platform_supports_absolute_position(self.platform)
    }
}

const fn platform_supports_absolute_position(platform: PlatformKind) -> bool {
    matches!(
        platform,
        PlatformKind::Windows | PlatformKind::MacOs | PlatformKind::LinuxX11
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeFullscreenMode {
    Borderless,
    Exclusive,
}

trait WindowPort: Send + Sync {
    fn title(&self) -> String;
    fn scale_factor(&self) -> f64;
    fn outer_position(&self) -> Result<WinitPhysicalPosition<i32>, String>;
    fn outer_size(&self) -> WinitPhysicalSize<u32>;
    fn surface_size(&self) -> WinitPhysicalSize<u32>;
    fn set_outer_position(&self, position: WinitPhysicalPosition<i32>);
    fn request_surface_size(&self, size: WinitPhysicalSize<u32>);
    fn set_title(&self, title: &str);
    fn set_visible(&self, visible: bool);
    fn is_visible(&self) -> Option<bool>;
    fn set_minimized(&self, minimized: bool);
    fn is_minimized(&self) -> Option<bool>;
    fn set_maximized(&self, maximized: bool);
    fn is_maximized(&self) -> bool;
    fn set_fullscreen(&self, mode: Option<NativeFullscreenMode>) -> Result<(), String>;
    fn fullscreen_mode(&self) -> Option<NativeFullscreenMode>;
    fn focus_window(&self);
    fn has_focus(&self) -> bool;
    fn set_cursor_icon(&self, icon: WinitCursorIcon);
    fn set_cursor_position(&self, position: WinitPhysicalPosition<i32>) -> Result<(), String>;
    fn set_cursor_grab(&self, mode: WinitCursorGrabMode) -> Result<(), String>;
    fn set_cursor_visible(&self, visible: bool);
}

struct WinitWindowPort {
    window: Arc<dyn Window>,
}

impl WindowPort for WinitWindowPort {
    fn title(&self) -> String {
        self.window.title()
    }

    fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    fn outer_position(&self) -> Result<WinitPhysicalPosition<i32>, String> {
        self.window
            .outer_position()
            .map_err(|error| error.to_string())
    }

    fn outer_size(&self) -> WinitPhysicalSize<u32> {
        self.window.outer_size()
    }

    fn surface_size(&self) -> WinitPhysicalSize<u32> {
        self.window.surface_size()
    }

    fn set_outer_position(&self, position: WinitPhysicalPosition<i32>) {
        self.window.set_outer_position(position.into());
    }

    fn request_surface_size(&self, size: WinitPhysicalSize<u32>) {
        let _ = self.window.request_surface_size(size.into());
    }

    fn set_title(&self, title: &str) {
        self.window.set_title(title);
    }

    fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible);
    }

    fn is_visible(&self) -> Option<bool> {
        self.window.is_visible()
    }

    fn set_minimized(&self, minimized: bool) {
        self.window.set_minimized(minimized);
    }

    fn is_minimized(&self) -> Option<bool> {
        self.window.is_minimized()
    }

    fn set_maximized(&self, maximized: bool) {
        self.window.set_maximized(maximized);
    }

    fn is_maximized(&self) -> bool {
        self.window.is_maximized()
    }

    fn set_fullscreen(&self, mode: Option<NativeFullscreenMode>) -> Result<(), String> {
        let fullscreen = match mode {
            Some(NativeFullscreenMode::Borderless) => Some(Fullscreen::Borderless(None)),
            Some(NativeFullscreenMode::Exclusive) => {
                Some(exclusive_fullscreen(self.window.as_ref())?)
            }
            None => None,
        };
        self.window.set_fullscreen(fullscreen);
        Ok(())
    }

    fn fullscreen_mode(&self) -> Option<NativeFullscreenMode> {
        self.window.fullscreen().map(|fullscreen| match fullscreen {
            Fullscreen::Borderless(_) => NativeFullscreenMode::Borderless,
            Fullscreen::Exclusive(_, _) => NativeFullscreenMode::Exclusive,
        })
    }

    fn focus_window(&self) {
        self.window.focus_window();
    }

    fn has_focus(&self) -> bool {
        self.window.has_focus()
    }

    fn set_cursor_icon(&self, icon: WinitCursorIcon) {
        self.window.set_cursor(Cursor::Icon(icon));
    }

    fn set_cursor_position(&self, position: WinitPhysicalPosition<i32>) -> Result<(), String> {
        self.window
            .set_cursor_position(position.into())
            .map_err(|error| error.to_string())
    }

    fn set_cursor_grab(&self, mode: WinitCursorGrabMode) -> Result<(), String> {
        self.window
            .set_cursor_grab(mode)
            .map_err(|error| error.to_string())
    }

    fn set_cursor_visible(&self, visible: bool) {
        self.window.set_cursor_visible(visible);
    }
}

fn window_title(window: &dyn WindowPort, fallback: String) -> String {
    let title = window.title();
    if title.is_empty() { fallback } else { title }
}

fn application_name() -> Option<String> {
    std::env::current_exe()
        .ok()?
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
}

fn current_mode(window: &dyn WindowPort, fallback: WindowMode) -> WindowMode {
    let minimized = window.is_minimized();
    if let Some(fullscreen) = window.fullscreen_mode() {
        match fullscreen {
            NativeFullscreenMode::Borderless => WindowMode::BorderlessFullscreen,
            NativeFullscreenMode::Exclusive => WindowMode::Fullscreen,
        }
    } else if minimized == Some(true) {
        WindowMode::Minimized
    } else if window.is_maximized() {
        WindowMode::Maximized
    } else if minimized.is_none() {
        fallback
    } else {
        WindowMode::Normal
    }
}

fn exclusive_fullscreen(window: &dyn Window) -> Result<Fullscreen, String> {
    let monitor = window
        .current_monitor()
        .ok_or_else(|| "exclusive fullscreen requires an available current monitor".to_owned())?;
    let video_mode = monitor
        .video_modes()
        .max_by_key(|mode| {
            let size = mode.size();
            (
                u64::from(size.width) * u64::from(size.height),
                mode.refresh_rate_millihertz()
                    .map_or(0, std::num::NonZero::get),
                mode.bit_depth().map_or(0, std::num::NonZero::get),
            )
        })
        .ok_or_else(|| {
            "exclusive fullscreen requires at least one monitor video mode".to_owned()
        })?;
    Ok(Fullscreen::Exclusive(monitor, video_mode))
}

fn winit_grab_mode(mode: CursorGrabMode) -> WinitCursorGrabMode {
    match mode {
        CursorGrabMode::None => WinitCursorGrabMode::None,
        CursorGrabMode::Confined => WinitCursorGrabMode::Confined,
        CursorGrabMode::Locked => WinitCursorGrabMode::Locked,
    }
}

fn winit_cursor_icon(icon: CursorIcon) -> Option<WinitCursorIcon> {
    match icon {
        CursorIcon::Default => Some(WinitCursorIcon::Default),
        CursorIcon::Pointer => Some(WinitCursorIcon::Pointer),
        CursorIcon::Text => Some(WinitCursorIcon::Text),
        CursorIcon::Crosshair => Some(WinitCursorIcon::Crosshair),
        CursorIcon::Move => Some(WinitCursorIcon::Move),
        CursorIcon::NotAllowed => Some(WinitCursorIcon::NotAllowed),
        CursorIcon::Wait => Some(WinitCursorIcon::Wait),
        CursorIcon::Progress => Some(WinitCursorIcon::Progress),
        CursorIcon::Help => Some(WinitCursorIcon::Help),
        CursorIcon::ZoomIn => Some(WinitCursorIcon::ZoomIn),
        CursorIcon::ZoomOut => Some(WinitCursorIcon::ZoomOut),
        CursorIcon::Grab => Some(WinitCursorIcon::Grab),
        CursorIcon::Grabbing => Some(WinitCursorIcon::Grabbing),
        CursorIcon::ResizeHorizontal => Some(WinitCursorIcon::EwResize),
        CursorIcon::ResizeVertical => Some(WinitCursorIcon::NsResize),
        CursorIcon::ResizeDiagonalNorthEastSouthWest => Some(WinitCursorIcon::NeswResize),
        CursorIcon::ResizeDiagonalNorthWestSouthEast => Some(WinitCursorIcon::NwseResize),
        CursorIcon::Hidden => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestPort {
        state: Mutex<TestWindowState>,
    }

    #[derive(Debug)]
    struct TestWindowState {
        title: String,
        visible: Option<bool>,
        minimized: Option<bool>,
        mode: TestModeState,
        focused: bool,
        outer_position: WinitPhysicalPosition<i32>,
        outer_size: WinitPhysicalSize<u32>,
        surface_size: WinitPhysicalSize<u32>,
        requested_position: Option<WinitPhysicalPosition<i32>>,
        requested_size: Option<WinitPhysicalSize<u32>>,
        cursor: TestCursorState,
        errors: TestPortErrors,
    }

    #[derive(Debug, Default)]
    struct TestModeState {
        maximized: bool,
        fullscreen: Option<NativeFullscreenMode>,
    }

    #[derive(Debug)]
    struct TestCursorState {
        icon: WinitCursorIcon,
        visible: bool,
        position: Option<WinitPhysicalPosition<i32>>,
        grab: WinitCursorGrabMode,
    }

    #[derive(Debug, Default)]
    struct TestPortErrors {
        cursor_position: Option<String>,
        cursor_grab: Option<String>,
    }

    impl Default for TestPort {
        fn default() -> Self {
            Self {
                state: Mutex::new(TestWindowState {
                    title: "Initial".to_owned(),
                    visible: Some(true),
                    minimized: Some(false),
                    mode: TestModeState::default(),
                    focused: false,
                    outer_position: WinitPhysicalPosition::new(10, 20),
                    outer_size: WinitPhysicalSize::new(960, 540),
                    surface_size: WinitPhysicalSize::new(940, 520),
                    requested_position: None,
                    requested_size: None,
                    cursor: TestCursorState {
                        icon: WinitCursorIcon::Default,
                        visible: true,
                        position: None,
                        grab: WinitCursorGrabMode::None,
                    },
                    errors: TestPortErrors::default(),
                }),
            }
        }
    }

    impl WindowPort for TestPort {
        fn title(&self) -> String {
            self.state.lock().expect("state lock").title.clone()
        }

        fn scale_factor(&self) -> f64 {
            1.5
        }

        fn outer_position(&self) -> Result<WinitPhysicalPosition<i32>, String> {
            Ok(self.state.lock().expect("state lock").outer_position)
        }

        fn outer_size(&self) -> WinitPhysicalSize<u32> {
            self.state.lock().expect("state lock").outer_size
        }

        fn surface_size(&self) -> WinitPhysicalSize<u32> {
            self.state.lock().expect("state lock").surface_size
        }

        fn set_outer_position(&self, position: WinitPhysicalPosition<i32>) {
            self.state.lock().expect("state lock").requested_position = Some(position);
        }

        fn request_surface_size(&self, size: WinitPhysicalSize<u32>) {
            self.state.lock().expect("state lock").requested_size = Some(size);
        }

        fn set_title(&self, title: &str) {
            self.state.lock().expect("state lock").title = title.to_owned();
        }

        fn set_visible(&self, visible: bool) {
            self.state.lock().expect("state lock").visible = Some(visible);
        }

        fn is_visible(&self) -> Option<bool> {
            self.state.lock().expect("state lock").visible
        }

        fn set_minimized(&self, minimized: bool) {
            self.state.lock().expect("state lock").minimized = Some(minimized);
        }

        fn is_minimized(&self) -> Option<bool> {
            self.state.lock().expect("state lock").minimized
        }

        fn set_maximized(&self, maximized: bool) {
            self.state.lock().expect("state lock").mode.maximized = maximized;
        }

        fn is_maximized(&self) -> bool {
            self.state.lock().expect("state lock").mode.maximized
        }

        fn set_fullscreen(&self, mode: Option<NativeFullscreenMode>) -> Result<(), String> {
            self.state.lock().expect("state lock").mode.fullscreen = mode;
            Ok(())
        }

        fn fullscreen_mode(&self) -> Option<NativeFullscreenMode> {
            self.state.lock().expect("state lock").mode.fullscreen
        }

        fn focus_window(&self) {
            self.state.lock().expect("state lock").focused = true;
        }

        fn has_focus(&self) -> bool {
            self.state.lock().expect("state lock").focused
        }

        fn set_cursor_icon(&self, icon: WinitCursorIcon) {
            self.state.lock().expect("state lock").cursor.icon = icon;
        }

        fn set_cursor_position(&self, position: WinitPhysicalPosition<i32>) -> Result<(), String> {
            let mut state = self.state.lock().expect("state lock");
            if let Some(error) = state.errors.cursor_position.clone() {
                return Err(error);
            }
            state.cursor.position = Some(position);
            Ok(())
        }

        fn set_cursor_grab(&self, mode: WinitCursorGrabMode) -> Result<(), String> {
            let mut state = self.state.lock().expect("state lock");
            if let Some(error) = state.errors.cursor_grab.clone() {
                return Err(error);
            }
            state.cursor.grab = mode;
            Ok(())
        }

        fn set_cursor_visible(&self, visible: bool) {
            self.state.lock().expect("state lock").cursor.visible = visible;
        }
    }

    fn test_driver(platform: PlatformKind) -> (Arc<TestPort>, WinitOwnedWindowDriver) {
        let port = Arc::new(TestPort::default());
        let close_signal = WindowCloseSignal::default();
        let driver = WinitOwnedWindowDriver::from_port(
            port.clone(),
            "Initial".to_owned(),
            close_signal,
            platform,
        )
        .expect("test driver initializes");
        (port, driver)
    }

    #[test]
    fn lists_the_primary_owned_window_without_native_handles() {
        let (_port, driver) = test_driver(PlatformKind::Windows);
        let OwnedWindowResponse::Windows(windows) = driver
            .execute_window(OwnedWindowRequest::List)
            .expect("list succeeds")
        else {
            panic!("list must return windows");
        };

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].id.as_str(), "owned:primary");
        assert_eq!(windows[0].scope, WindowScope::Owned);
        assert_eq!(windows[0].title.as_deref(), Some("Initial"));
        assert_eq!(windows[0].application_name, application_name());
        assert_eq!(windows[0].bounds.expect("bounds").size.width, 960);
        assert_eq!(windows[0].scale_factor, ScaleFactor::from_f64(1.5).ok());
    }

    #[test]
    fn rejects_wrong_owned_and_external_window_targets() {
        let (_port, driver) = test_driver(PlatformKind::Windows);
        let stale = WindowId::try_new("owned:stale").expect("id");
        let external = WindowId::try_new("external:target").expect("id");

        let stale_error = driver
            .execute_window(OwnedWindowRequest::Get {
                target: WindowTarget::Owned(stale.clone()),
            })
            .expect_err("stale owned id fails");
        assert!(matches!(
            stale_error,
            DesktopError::StaleHandle { handle } if handle == stale.to_string()
        ));

        let external_error = driver
            .execute_window(OwnedWindowRequest::RequestFocus {
                target: WindowTarget::External(external),
            })
            .expect_err("external target fails");
        assert!(matches!(
            external_error,
            DesktopError::InvalidArgument { field, .. } if field == "target"
        ));
    }

    #[test]
    fn applies_owned_window_requests_and_close_signal() {
        let (port, driver) = test_driver(PlatformKind::Windows);

        driver
            .execute_window(OwnedWindowRequest::SetTitle {
                target: WindowTarget::PrimaryOwned,
                title: "Changed".to_owned(),
            })
            .expect("title applies");
        driver
            .execute_window(OwnedWindowRequest::SetVisible {
                target: WindowTarget::PrimaryOwned,
                visible: false,
            })
            .expect("visibility applies");
        driver
            .execute_window(OwnedWindowRequest::SetMode {
                target: WindowTarget::PrimaryOwned,
                mode: WindowMode::Maximized,
            })
            .expect("mode applies");
        driver
            .execute_window(OwnedWindowRequest::SetMode {
                target: WindowTarget::PrimaryOwned,
                mode: WindowMode::BorderlessFullscreen,
            })
            .expect("borderless fullscreen mode applies");
        driver
            .execute_window(OwnedWindowRequest::SetMode {
                target: WindowTarget::PrimaryOwned,
                mode: WindowMode::Fullscreen,
            })
            .expect("exclusive fullscreen mode applies");
        driver
            .execute_window(OwnedWindowRequest::SetBounds {
                target: WindowTarget::PrimaryOwned,
                bounds: PhysicalRect {
                    position: PhysicalPosition { x: 40, y: 50 },
                    size: PhysicalSize {
                        width: 800,
                        height: 450,
                    },
                },
            })
            .expect("bounds apply");
        driver
            .execute_window(OwnedWindowRequest::RequestFocus {
                target: WindowTarget::PrimaryOwned,
            })
            .expect("focus applies");
        driver
            .execute_window(OwnedWindowRequest::RequestClose {
                target: WindowTarget::PrimaryOwned,
            })
            .expect("close applies");

        let state = port.state.lock().expect("state lock");
        assert_eq!(state.title, "Changed");
        assert_eq!(state.visible, Some(false));
        assert!(!state.mode.maximized);
        assert_eq!(state.mode.fullscreen, Some(NativeFullscreenMode::Exclusive));
        assert_eq!(
            state.requested_position,
            Some(WinitPhysicalPosition::new(40, 50))
        );
        assert_eq!(state.requested_size, Some(WinitPhysicalSize::new(780, 430)));
        assert!(state.focused);
        assert!(driver.close_signal.take());
    }

    #[test]
    fn reports_borderless_and_exclusive_fullscreen_modes_distinctly() {
        let (_port, driver) = test_driver(PlatformKind::Windows);

        driver
            .execute_window(OwnedWindowRequest::SetMode {
                target: WindowTarget::PrimaryOwned,
                mode: WindowMode::BorderlessFullscreen,
            })
            .expect("borderless fullscreen mode applies");
        let OwnedWindowResponse::Window(borderless) = driver
            .execute_window(OwnedWindowRequest::Get {
                target: WindowTarget::PrimaryOwned,
            })
            .expect("snapshot succeeds")
        else {
            panic!("get must return one window");
        };
        assert_eq!(borderless.mode, WindowMode::BorderlessFullscreen);

        driver
            .execute_window(OwnedWindowRequest::SetMode {
                target: WindowTarget::PrimaryOwned,
                mode: WindowMode::Fullscreen,
            })
            .expect("exclusive fullscreen mode applies");
        let OwnedWindowResponse::Window(exclusive) = driver
            .execute_window(OwnedWindowRequest::Get {
                target: WindowTarget::PrimaryOwned,
            })
            .expect("snapshot succeeds")
        else {
            panic!("get must return one window");
        };
        assert_eq!(exclusive.mode, WindowMode::Fullscreen);
    }

    #[test]
    fn wayland_rejects_absolute_bounds_without_requesting_placement_or_resize() {
        let (port, driver) = test_driver(PlatformKind::LinuxWayland);

        let error = driver
            .execute_window(OwnedWindowRequest::SetBounds {
                target: WindowTarget::PrimaryOwned,
                bounds: PhysicalRect {
                    position: PhysicalPosition { x: 40, y: 50 },
                    size: PhysicalSize {
                        width: 800,
                        height: 450,
                    },
                },
            })
            .expect_err("Wayland absolute bounds are unsupported");

        assert!(matches!(
            error,
            DesktopError::Unsupported {
                feature: DesktopFeature::OwnedWindowAbsolutePosition,
                platform: PlatformKind::LinuxWayland,
                ..
            }
        ));
        let state = port.state.lock().expect("state lock");
        assert_eq!(state.requested_position, None);
        assert_eq!(state.requested_size, None);
    }

    #[test]
    fn hidden_cursor_composes_with_explicit_visibility() {
        let (port, driver) = test_driver(PlatformKind::Windows);

        driver
            .execute_cursor(OwnedCursorRequest::SetVisible {
                target: WindowTarget::PrimaryOwned,
                visible: false,
            })
            .expect("visibility applies");
        driver
            .execute_cursor(OwnedCursorRequest::SetIcon {
                target: WindowTarget::PrimaryOwned,
                icon: CursorIcon::Pointer,
            })
            .expect("icon applies");
        assert!(!port.state.lock().expect("state lock").cursor.visible);

        driver
            .execute_cursor(OwnedCursorRequest::SetVisible {
                target: WindowTarget::PrimaryOwned,
                visible: true,
            })
            .expect("visibility applies");
        assert!(port.state.lock().expect("state lock").cursor.visible);

        driver
            .execute_cursor(OwnedCursorRequest::SetIcon {
                target: WindowTarget::PrimaryOwned,
                icon: CursorIcon::Hidden,
            })
            .expect("hidden icon applies");
        assert!(!port.state.lock().expect("state lock").cursor.visible);
    }

    #[test]
    fn applies_cursor_position_and_grab_requests() {
        let (port, driver) = test_driver(PlatformKind::Windows);
        driver
            .execute_cursor(OwnedCursorRequest::SetPosition {
                target: WindowTarget::PrimaryOwned,
                position: PhysicalPosition { x: 12, y: 34 },
            })
            .expect("cursor position applies");
        driver
            .execute_cursor(OwnedCursorRequest::SetGrab {
                target: WindowTarget::PrimaryOwned,
                mode: CursorGrabMode::Confined,
            })
            .expect("cursor grab applies");

        let state = port.state.lock().expect("state lock");
        assert_eq!(
            state.cursor.position,
            Some(WinitPhysicalPosition::new(12, 34))
        );
        assert_eq!(state.cursor.grab, WinitCursorGrabMode::Confined);
    }

    #[test]
    fn cursor_platform_failures_cross_as_structured_errors() {
        let (port, driver) = test_driver(PlatformKind::Windows);
        port.state
            .lock()
            .expect("state lock")
            .errors
            .cursor_position = Some("cursor warp rejected".to_owned());

        let error = driver
            .execute_cursor(OwnedCursorRequest::SetPosition {
                target: WindowTarget::PrimaryOwned,
                position: PhysicalPosition { x: 2, y: 3 },
            })
            .expect_err("platform failure propagates");

        assert!(matches!(
            error,
            DesktopError::Platform {
                operation,
                detail,
                ..
            } if operation == "owned_cursor_position" && detail == "cursor warp rejected"
        ));
    }

    #[test]
    fn cursor_icon_mapping_covers_every_visible_contract_icon() {
        let visible = [
            CursorIcon::Default,
            CursorIcon::Pointer,
            CursorIcon::Text,
            CursorIcon::Crosshair,
            CursorIcon::Move,
            CursorIcon::NotAllowed,
            CursorIcon::Wait,
            CursorIcon::Progress,
            CursorIcon::Help,
            CursorIcon::ZoomIn,
            CursorIcon::ZoomOut,
            CursorIcon::Grab,
            CursorIcon::Grabbing,
            CursorIcon::ResizeHorizontal,
            CursorIcon::ResizeVertical,
            CursorIcon::ResizeDiagonalNorthEastSouthWest,
            CursorIcon::ResizeDiagonalNorthWestSouthEast,
        ];
        assert!(
            visible
                .into_iter()
                .all(|icon| winit_cursor_icon(icon).is_some())
        );
        assert_eq!(winit_cursor_icon(CursorIcon::Hidden), None);
    }

    #[test]
    fn grab_mode_mapping_is_total() {
        assert_eq!(
            winit_grab_mode(CursorGrabMode::None),
            WinitCursorGrabMode::None
        );
        assert_eq!(
            winit_grab_mode(CursorGrabMode::Confined),
            WinitCursorGrabMode::Confined
        );
        assert_eq!(
            winit_grab_mode(CursorGrabMode::Locked),
            WinitCursorGrabMode::Locked
        );
    }

    #[test]
    fn absolute_position_excludes_wayland_and_non_desktop_hosts() {
        assert!(platform_supports_absolute_position(PlatformKind::Windows));
        assert!(platform_supports_absolute_position(PlatformKind::MacOs));
        assert!(platform_supports_absolute_position(PlatformKind::LinuxX11));
        assert!(!platform_supports_absolute_position(
            PlatformKind::LinuxWayland
        ));
        assert!(!platform_supports_absolute_position(PlatformKind::Web));
        assert!(!platform_supports_absolute_position(PlatformKind::Other));
    }

    #[test]
    fn close_signal_is_edge_triggered() {
        let signal = WindowCloseSignal::default();
        assert!(!signal.take());
        signal.request();
        assert!(signal.take());
        assert!(!signal.take());
    }
}
