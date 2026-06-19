use arcweft_desktop_contract::PlatformKind;

/// Detects the concrete window-system family relevant to capability behavior.
#[cfg(target_arch = "wasm32")]
pub const fn native_platform_kind() -> PlatformKind {
    PlatformKind::Web
}

/// Detects the concrete window-system family relevant to capability behavior.
#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
pub const fn native_platform_kind() -> PlatformKind {
    PlatformKind::Windows
}

/// Detects the concrete window-system family relevant to capability behavior.
#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
pub const fn native_platform_kind() -> PlatformKind {
    PlatformKind::MacOs
}

/// Detects X11 versus Wayland at runtime on Linux.
#[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
pub fn native_platform_kind() -> PlatformKind {
    let session = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if session == "wayland" || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        PlatformKind::LinuxWayland
    } else if session == "x11" || std::env::var_os("DISPLAY").is_some() {
        PlatformKind::LinuxX11
    } else {
        PlatformKind::Other
    }
}

/// Falls back to an unknown native host on unsupported targets.
#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "windows"),
    not(target_os = "macos"),
    not(target_os = "linux")
))]
pub const fn native_platform_kind() -> PlatformKind {
    PlatformKind::Other
}
