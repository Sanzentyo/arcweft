use arcweft_desktop_contract::{
    DesktopError, DesktopFeature, ExternalWindowRequest, ExternalWindowResponse, PlatformKind,
};

#[cfg(feature = "external-window-observe")]
use arcweft_desktop_contract::{
    PhysicalPosition, PhysicalRect, PhysicalSize, WindowId, WindowMode, WindowScope, WindowSnapshot,
};

#[cfg(feature = "external-window-observe")]
pub(crate) fn observe_external_windows(
    platform: PlatformKind,
    request: ExternalWindowRequest,
) -> Result<ExternalWindowResponse, DesktopError> {
    if matches!(
        platform,
        PlatformKind::LinuxWayland | PlatformKind::Web | PlatformKind::Other
    ) {
        return Err(unsupported(platform));
    }
    match request {
        ExternalWindowRequest::List => list(platform).map(ExternalWindowResponse::Windows),
        ExternalWindowRequest::Get { id } => list(platform)?
            .into_iter()
            .find(|window| window.id == id)
            .map(ExternalWindowResponse::Window)
            .ok_or_else(|| DesktopError::StaleHandle {
                handle: id.to_string(),
            }),
        ExternalWindowRequest::Activate { .. }
        | ExternalWindowRequest::SetBounds { .. }
        | ExternalWindowRequest::RequestClose { .. } => Err(DesktopError::ResponseMismatch {
            request: "external-window observation request".to_owned(),
        }),
    }
}

#[cfg(not(feature = "external-window-observe"))]
pub(crate) fn observe_external_windows(
    platform: PlatformKind,
    _request: ExternalWindowRequest,
) -> Result<ExternalWindowResponse, DesktopError> {
    Err(unsupported(platform))
}

#[cfg(feature = "external-window-observe")]
fn list(platform: PlatformKind) -> Result<Vec<WindowSnapshot>, DesktopError> {
    let mut snapshots = xcap::Window::all()
        .map_err(|error| DesktopError::BackendUnavailable {
            backend: "xcap".to_owned(),
            detail: error.to_string(),
        })?
        .into_iter()
        .map(|window| snapshot(platform, &window))
        .collect::<Result<Vec<_>, _>>()?;
    snapshots.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(snapshots)
}

#[cfg(feature = "external-window-observe")]
fn snapshot(platform: PlatformKind, window: &xcap::Window) -> Result<WindowSnapshot, DesktopError> {
    let raw_id = window.id().map_err(xcap_error("external_window_id"))?;
    let minimized = window
        .is_minimized()
        .map_err(xcap_error("external_window_minimized"))?;
    let maximized = window
        .is_maximized()
        .map_err(xcap_error("external_window_maximized"))?;
    let mode = if minimized {
        WindowMode::Minimized
    } else if maximized {
        WindowMode::Maximized
    } else {
        WindowMode::Normal
    };
    let title = window
        .title()
        .map_err(xcap_error("external_window_title"))?;
    let app_name = window
        .app_name()
        .map_err(xcap_error("external_window_app_name"))?;
    Ok(WindowSnapshot {
        id: WindowId::new(format!("external:{}:{raw_id}", platform_label(platform)))
            .expect("generated window identifier is valid"),
        scope: WindowScope::External,
        title: (!title.is_empty()).then_some(title),
        application_name: (!app_name.is_empty()).then_some(app_name),
        process_id: Some(window.pid().map_err(xcap_error("external_window_pid"))?),
        bounds: Some(PhysicalRect {
            position: PhysicalPosition {
                x: window.x().map_err(xcap_error("external_window_x"))?,
                y: window.y().map_err(xcap_error("external_window_y"))?,
            },
            size: PhysicalSize {
                width: window
                    .width()
                    .map_err(xcap_error("external_window_width"))?,
                height: window
                    .height()
                    .map_err(xcap_error("external_window_height"))?,
            },
        }),
        scale_factor: None,
        mode,
        visible: None,
        focused: Some(
            window
                .is_focused()
                .map_err(xcap_error("external_window_focused"))?,
        ),
    })
}

#[cfg(feature = "external-window-observe")]
fn xcap_error<E>(operation: &'static str) -> impl FnOnce(E) -> DesktopError
where
    E: ToString,
{
    move |error| DesktopError::Platform {
        operation: operation.to_owned(),
        code: None,
        detail: error.to_string(),
    }
}

#[cfg(feature = "external-window-observe")]
fn platform_label(platform: PlatformKind) -> &'static str {
    match platform {
        PlatformKind::Windows => "windows",
        PlatformKind::MacOs => "macos",
        PlatformKind::LinuxX11 => "linux-x11",
        PlatformKind::LinuxWayland => "linux-wayland",
        PlatformKind::Web => "web",
        PlatformKind::Other => "other",
    }
}

fn unsupported(platform: PlatformKind) -> DesktopError {
    DesktopError::Unsupported {
        feature: DesktopFeature::ExternalWindowObserve,
        platform,
        detail: "external-window observation is disabled or unavailable".to_owned(),
    }
}
