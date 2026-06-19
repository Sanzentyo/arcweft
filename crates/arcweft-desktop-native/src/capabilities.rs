use crate::{GlobalPointerPolicy, NativeDesktopOptions};
use arcweft_desktop_contract::{
    DesktopCapabilities, DesktopFeature, FeatureSupport, PermissionKind, PlatformKind,
};

pub(crate) fn native_capabilities(
    platform: PlatformKind,
    options: &NativeDesktopOptions,
    has_owned_driver: bool,
    owned_absolute_position: bool,
    has_external_control_driver: bool,
) -> DesktopCapabilities {
    DesktopCapabilities::new(
        platform,
        [
            owned_observe(has_owned_driver),
            owned_control(has_owned_driver),
            owned_absolute(platform, has_owned_driver, owned_absolute_position),
            owned_cursor(has_owned_driver),
            file_dialog(platform),
            known_directories(options),
            granted_file_io(options),
            FeatureSupport::unsupported(
                DesktopFeature::PersistentFileGrant,
                "the reference native backend keeps grants session-scoped; install a sealed-token persistence provider before enabling this feature",
            ),
            external_observe(platform, options),
            external_control(platform, has_external_control_driver),
            global_pointer_observe(platform, options.global_pointer),
            global_pointer_control(platform, options.global_pointer),
        ],
    )
}

fn owned_observe(has_driver: bool) -> FeatureSupport {
    if has_driver {
        FeatureSupport::supported(
            DesktopFeature::OwnedWindowObserve,
            "provided by the native player's event-loop window driver",
        )
    } else {
        FeatureSupport::unsupported(
            DesktopFeature::OwnedWindowObserve,
            "no owned-window driver is installed",
        )
    }
}

fn owned_control(has_driver: bool) -> FeatureSupport {
    if has_driver {
        FeatureSupport::with_consent(
            DesktopFeature::OwnedWindowControl,
            [PermissionKind::HostMainThread],
            "serialized through the native player's event-loop thread",
        )
    } else {
        FeatureSupport::unsupported(
            DesktopFeature::OwnedWindowControl,
            "no owned-window driver is installed",
        )
    }
}

fn owned_absolute(
    platform: PlatformKind,
    has_driver: bool,
    driver_supports_absolute: bool,
) -> FeatureSupport {
    if !has_driver || !driver_supports_absolute {
        return FeatureSupport::unsupported(
            DesktopFeature::OwnedWindowAbsolutePosition,
            "the installed owned-window driver does not expose absolute positioning",
        );
    }
    match platform {
        PlatformKind::LinuxWayland | PlatformKind::Web => FeatureSupport::unsupported(
            DesktopFeature::OwnedWindowAbsolutePosition,
            "the compositor/browser owns top-level placement",
        ),
        PlatformKind::Windows | PlatformKind::MacOs | PlatformKind::LinuxX11 => {
            FeatureSupport::best_effort(
                DesktopFeature::OwnedWindowAbsolutePosition,
                [PermissionKind::HostMainThread],
                "the window manager may adjust or reject requested bounds",
            )
        }
        PlatformKind::Other => FeatureSupport::unsupported(
            DesktopFeature::OwnedWindowAbsolutePosition,
            "unknown native window system",
        ),
    }
}

fn owned_cursor(has_driver: bool) -> FeatureSupport {
    if has_driver {
        FeatureSupport::with_consent(
            DesktopFeature::OwnedCursorControl,
            [PermissionKind::HostMainThread],
            "window-local cursor state is owned by the event-loop driver",
        )
    } else {
        FeatureSupport::unsupported(
            DesktopFeature::OwnedCursorControl,
            "no owned-window driver is installed",
        )
    }
}

fn file_dialog(platform: PlatformKind) -> FeatureSupport {
    if !cfg!(feature = "file-dialog") {
        return FeatureSupport::unsupported(
            DesktopFeature::UserFileDialog,
            "crate feature `file-dialog` is disabled",
        );
    }
    match platform {
        PlatformKind::Windows | PlatformKind::MacOs => FeatureSupport::with_consent(
            DesktopFeature::UserFileDialog,
            [
                PermissionKind::UserFileSelection,
                PermissionKind::HostMainThread,
            ],
            "native file chooser returns a session-scoped opaque grant",
        ),
        PlatformKind::LinuxX11 | PlatformKind::LinuxWayland => FeatureSupport::with_consent(
            DesktopFeature::UserFileDialog,
            [
                PermissionKind::UserFileSelection,
                PermissionKind::DesktopPortal,
                PermissionKind::HostMainThread,
            ],
            "RFD uses the desktop portal by default and falls back according to its runtime backend",
        ),
        PlatformKind::Web | PlatformKind::Other => FeatureSupport::unsupported(
            DesktopFeature::UserFileDialog,
            "the native backend is unavailable on this platform",
        ),
    }
}

fn known_directories(options: &NativeDesktopOptions) -> FeatureSupport {
    if !cfg!(feature = "known-directories") {
        return FeatureSupport::unsupported(
            DesktopFeature::KnownDirectoryGrant,
            "crate feature `known-directories` is disabled",
        );
    }
    if options.allowed_known_directories.is_empty() {
        FeatureSupport::unsupported(
            DesktopFeature::KnownDirectoryGrant,
            "the host policy allowlist is empty",
        )
    } else {
        FeatureSupport::with_consent(
            DesktopFeature::KnownDirectoryGrant,
            [PermissionKind::KnownDirectoryAccess],
            "only manifest-approved directory families can be granted",
        )
    }
}

fn granted_file_io(options: &NativeDesktopOptions) -> FeatureSupport {
    if cfg!(feature = "file-dialog")
        || (cfg!(feature = "known-directories") && !options.allowed_known_directories.is_empty())
    {
        FeatureSupport::supported(
            DesktopFeature::GrantedFileIo,
            "I/O is restricted to opaque file grants and validated descendants",
        )
    } else {
        FeatureSupport::unsupported(
            DesktopFeature::GrantedFileIo,
            "no grant-producing feature is enabled",
        )
    }
}

fn external_observe(platform: PlatformKind, options: &NativeDesktopOptions) -> FeatureSupport {
    if !cfg!(feature = "external-window-observe") {
        return FeatureSupport::unsupported(
            DesktopFeature::ExternalWindowObserve,
            "crate feature `external-window-observe` is disabled",
        );
    }
    if !options.external_window_observe {
        return FeatureSupport::unsupported(
            DesktopFeature::ExternalWindowObserve,
            "disabled by host policy",
        );
    }
    match platform {
        PlatformKind::Windows => FeatureSupport::best_effort(
            DesktopFeature::ExternalWindowObserve,
            [],
            "top-level desktop application windows are enumerated; protected or isolated desktops remain inaccessible",
        ),
        PlatformKind::MacOs => FeatureSupport::best_effort(
            DesktopFeature::ExternalWindowObserve,
            [PermissionKind::ScreenRecording],
            "window metadata availability depends on macOS privacy authorization",
        ),
        PlatformKind::LinuxX11 => FeatureSupport::best_effort(
            DesktopFeature::ExternalWindowObserve,
            [],
            "EWMH-compliant top-level windows are observed on the active X11 desktop",
        ),
        PlatformKind::LinuxWayland => FeatureSupport::unsupported(
            DesktopFeature::ExternalWindowObserve,
            "generic foreign-toplevel enumeration is compositor-specific or privileged on Wayland",
        ),
        PlatformKind::Web | PlatformKind::Other => FeatureSupport::unsupported(
            DesktopFeature::ExternalWindowObserve,
            "external desktop windows are unavailable",
        ),
    }
}

fn external_control(platform: PlatformKind, has_driver: bool) -> FeatureSupport {
    if !has_driver {
        return FeatureSupport::unsupported(
            DesktopFeature::ExternalWindowControl,
            "no privileged external-window control driver is installed",
        );
    }
    match platform {
        PlatformKind::LinuxWayland | PlatformKind::Web | PlatformKind::Other => {
            FeatureSupport::unsupported(
                DesktopFeature::ExternalWindowControl,
                "portable external-window control is unavailable on this window system",
            )
        }
        PlatformKind::Windows | PlatformKind::MacOs | PlatformKind::LinuxX11 => {
            FeatureSupport::best_effort(
                DesktopFeature::ExternalWindowControl,
                [
                    PermissionKind::Accessibility,
                    PermissionKind::HostMainThread,
                ],
                "a separately installed privileged driver submits user-like requests that the OS/window manager may reject",
            )
        }
    }
}

fn global_pointer_observe(platform: PlatformKind, policy: GlobalPointerPolicy) -> FeatureSupport {
    if !cfg!(feature = "global-pointer") {
        return FeatureSupport::unsupported(
            DesktopFeature::GlobalPointerObserve,
            "crate feature `global-pointer` is disabled",
        );
    }
    if !policy.allows_observe() {
        return FeatureSupport::unsupported(
            DesktopFeature::GlobalPointerObserve,
            "disabled by host policy",
        );
    }
    pointer_platform_support(platform, DesktopFeature::GlobalPointerObserve, false)
}

fn global_pointer_control(platform: PlatformKind, policy: GlobalPointerPolicy) -> FeatureSupport {
    if !cfg!(feature = "global-pointer") {
        return FeatureSupport::unsupported(
            DesktopFeature::GlobalPointerControl,
            "crate feature `global-pointer` is disabled",
        );
    }
    if !policy.allows_control() {
        return FeatureSupport::unsupported(
            DesktopFeature::GlobalPointerControl,
            "host policy permits observation at most",
        );
    }
    pointer_platform_support(platform, DesktopFeature::GlobalPointerControl, true)
}

fn pointer_platform_support(
    platform: PlatformKind,
    feature: DesktopFeature,
    controls_input: bool,
) -> FeatureSupport {
    match platform {
        PlatformKind::Windows => FeatureSupport::best_effort(
            feature,
            controls_input.then_some(PermissionKind::InputControl),
            "the operation is limited to the active input desktop and OS integrity boundaries",
        ),
        PlatformKind::MacOs => FeatureSupport::with_consent(
            feature,
            [PermissionKind::Accessibility, PermissionKind::InputControl],
            "macOS privacy and foreground-state rules apply",
        ),
        PlatformKind::LinuxX11 => FeatureSupport::best_effort(
            feature,
            controls_input.then_some(PermissionKind::InputControl),
            "global X11 coordinates are available while the active X server permits access",
        ),
        PlatformKind::LinuxWayland => FeatureSupport::unsupported(
            feature,
            "generic global pointer observation and warping are not portable Wayland capabilities",
        ),
        PlatformKind::Web | PlatformKind::Other => {
            FeatureSupport::unsupported(feature, "global desktop pointer access is unavailable")
        }
    }
}
