//! Platform backend boundary for the native-player text-input bridge.

use super::platform::NativeTextInputWindowContext;
use arcweft_presentation::text_input::{
    PlatformTextInputEvent, TextInputCapabilities, TextInputCapabilitySupport,
    TextInputClientSnapshot, TextInputFocusGeneration, TextInputGeometrySnapshot,
    TextInputKeyDisposition, TextInputSessionId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(target_os = "windows")]
use arcweft_desktop_native::text_input::windows_tsf::real_ime::{
    WindowsTsfImeBridge, WindowsTsfImeError,
};

/// Trace-safe backend identity.  It intentionally contains no native handle or
/// object identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeTextInputBackendIdentity {
    WindowsTsf,
    MacosAppKit,
    WaylandTextInputV3Unavailable,
    AndroidInputConnectionUnavailable,
    IosUiTextInputUnavailable,
    Unavailable,
}

#[derive(Debug)]
pub(crate) enum NativeTextInputBackend {
    Unavailable(UnavailableTextInputBackend),
    #[cfg(target_os = "windows")]
    WindowsTsf(Box<WindowsTsfNativePlayerBackend>),
    #[cfg(target_os = "macos")]
    MacosAppKit(MacosAppKitNativePlayerBackend),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UnavailableTextInputBackend {
    identity: NativeTextInputBackendIdentity,
    reason: String,
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub(crate) struct WindowsTsfNativePlayerBackend {
    inner: WindowsTsfImeBridge,
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub(crate) struct MacosAppKitNativePlayerBackend {
    unavailable: UnavailableTextInputBackend,
}

#[derive(Debug, Error)]
pub enum NativeTextInputBackendError {
    #[cfg(target_os = "windows")]
    #[error("Windows TSF backend failed: {0}")]
    WindowsTsf(#[from] WindowsTsfImeError),
}

impl NativeTextInputBackend {
    pub(crate) fn for_window(window: NativeTextInputWindowContext) -> Self {
        #[cfg(target_os = "windows")]
        {
            if let Some(hwnd) = window.hwnd() {
                match WindowsTsfImeBridge::new_for_window(hwnd) {
                    Ok(inner) => {
                        Self::WindowsTsf(Box::new(WindowsTsfNativePlayerBackend { inner }))
                    }
                    Err(error) => Self::Unavailable(UnavailableTextInputBackend::new(
                        NativeTextInputBackendIdentity::WindowsTsf,
                        format!("Windows TSF activation failed: {error}"),
                    )),
                }
            } else {
                Self::Unavailable(UnavailableTextInputBackend::new(
                    NativeTextInputBackendIdentity::WindowsTsf,
                    "winit window did not expose a Win32 HWND".to_owned(),
                ))
            }
        }

        #[cfg(target_os = "macos")]
        {
            let _ = window;
            Self::MacosAppKit(MacosAppKitNativePlayerBackend {
                unavailable: UnavailableTextInputBackend::new(
                    NativeTextInputBackendIdentity::MacosAppKit,
                    "AppKit backend slot is present; current main exposes AppKit through helper-process validation and still needs in-window attachment".to_owned(),
                ),
            })
        }

        #[cfg(target_os = "linux")]
        {
            let _ = window;
            Self::Unavailable(UnavailableTextInputBackend::new(
                NativeTextInputBackendIdentity::WaylandTextInputV3Unavailable,
                "Wayland text-input-v3 host boundary is not connected in this build".to_owned(),
            ))
        }

        #[cfg(target_os = "android")]
        {
            let _ = window;
            Self::Unavailable(UnavailableTextInputBackend::new(
                NativeTextInputBackendIdentity::AndroidInputConnectionUnavailable,
                "Android InputConnection host boundary is not connected in this build".to_owned(),
            ))
        }

        #[cfg(target_os = "ios")]
        {
            let _ = window;
            Self::Unavailable(UnavailableTextInputBackend::new(
                NativeTextInputBackendIdentity::IosUiTextInputUnavailable,
                "iOS UITextInput host boundary is not connected in this build".to_owned(),
            ))
        }

        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux",
            target_os = "android",
            target_os = "ios"
        )))]
        {
            let _ = window;
            Self::Unavailable(UnavailableTextInputBackend::new(
                NativeTextInputBackendIdentity::Unavailable,
                "no native platform text-input backend is implemented for this target".to_owned(),
            ))
        }
    }

    pub(crate) fn identity(&self) -> NativeTextInputBackendIdentity {
        match self {
            Self::Unavailable(backend) => backend.identity,
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(_) => NativeTextInputBackendIdentity::WindowsTsf,
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(_) => NativeTextInputBackendIdentity::MacosAppKit,
        }
    }

    pub(crate) fn capabilities(&self) -> TextInputCapabilities {
        match self {
            Self::Unavailable(_) => unsupported_capabilities(),
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(_) => TextInputCapabilities::for_platform_adapter(
                arcweft_presentation::text_input::TextInputAdapterKind::WindowsTsf,
            ),
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(_) => unsupported_capabilities(),
        }
    }

    pub(crate) fn unavailable_reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(backend) => Some(backend.reason()),
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(_) => None,
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(backend) => Some(backend.unavailable.reason()),
        }
    }

    pub(crate) fn activate(
        &mut self,
        snapshot: &TextInputClientSnapshot,
        generation: TextInputFocusGeneration,
        geometry: Option<&TextInputGeometrySnapshot>,
    ) -> Result<(), NativeTextInputBackendError> {
        match self {
            Self::Unavailable(_) => Ok(()),
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(backend) => backend.activate(snapshot, generation, geometry),
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(backend) => backend.activate(snapshot, generation, geometry),
        }
    }

    pub(crate) fn update_snapshot(
        &mut self,
        snapshot: &TextInputClientSnapshot,
    ) -> Result<(), NativeTextInputBackendError> {
        match self {
            Self::Unavailable(_) => Ok(()),
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(backend) => backend.update_snapshot(snapshot),
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(backend) => backend.update_snapshot(snapshot),
        }
    }

    pub(crate) fn update_geometry(
        &mut self,
        geometry: &TextInputGeometrySnapshot,
    ) -> Result<(), NativeTextInputBackendError> {
        match self {
            Self::Unavailable(_) => Ok(()),
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(backend) => backend.update_geometry(geometry),
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(backend) => backend.update_geometry(geometry),
        }
    }

    pub(crate) fn commit_composition(&mut self, _session: TextInputSessionId) {
        match self {
            Self::Unavailable(_) => {}
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(_) => {}
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(_) => {}
        }
    }

    pub(crate) fn cancel_composition(&mut self, _session: TextInputSessionId) {
        match self {
            Self::Unavailable(_) => {}
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(_) => {}
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(_) => {}
        }
    }

    pub(crate) fn blur(
        &mut self,
        policy: arcweft_presentation::text_input::TextInputBlurPolicy,
    ) -> Result<(), NativeTextInputBackendError> {
        match self {
            Self::Unavailable(_) => Ok(()),
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(backend) => backend.blur(policy),
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(backend) => backend.blur(policy),
        }
    }

    pub(crate) fn drain_platform_events(&mut self) -> Vec<PlatformTextInputEvent> {
        match self {
            Self::Unavailable(_) => Vec::new(),
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(backend) => backend.drain_platform_events(),
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(backend) => backend.drain_platform_events(),
        }
    }

    pub(crate) fn filter_key(&mut self, _key: &str) -> TextInputKeyDisposition {
        match self {
            Self::Unavailable(_) => TextInputKeyDisposition::ShortcutCandidate,
            #[cfg(target_os = "windows")]
            Self::WindowsTsf(_) => TextInputKeyDisposition::ShortcutCandidate,
            #[cfg(target_os = "macos")]
            Self::MacosAppKit(_) => TextInputKeyDisposition::ShortcutCandidate,
        }
    }
}

impl UnavailableTextInputBackend {
    pub(crate) fn new(identity: NativeTextInputBackendIdentity, reason: String) -> Self {
        Self { identity, reason }
    }

    fn reason(&self) -> &str {
        &self.reason
    }
}

#[cfg(target_os = "windows")]
impl WindowsTsfNativePlayerBackend {
    fn activate(
        &mut self,
        snapshot: &TextInputClientSnapshot,
        generation: TextInputFocusGeneration,
        geometry: Option<&TextInputGeometrySnapshot>,
    ) -> Result<(), NativeTextInputBackendError> {
        self.inner
            .focus_text_input(snapshot, generation, geometry)?;
        Ok(())
    }

    fn update_snapshot(
        &mut self,
        snapshot: &TextInputClientSnapshot,
    ) -> Result<(), NativeTextInputBackendError> {
        self.inner.update_snapshot(snapshot)?;
        Ok(())
    }

    fn update_geometry(
        &mut self,
        geometry: &TextInputGeometrySnapshot,
    ) -> Result<(), NativeTextInputBackendError> {
        self.inner.update_geometry(geometry)?;
        Ok(())
    }

    fn blur(
        &mut self,
        policy: arcweft_presentation::text_input::TextInputBlurPolicy,
    ) -> Result<(), NativeTextInputBackendError> {
        self.inner.blur(policy)?;
        Ok(())
    }

    fn drain_platform_events(&mut self) -> Vec<PlatformTextInputEvent> {
        self.inner.drain_platform_events()
    }
}

#[cfg(target_os = "macos")]
impl MacosAppKitNativePlayerBackend {
    fn activate(
        &mut self,
        _snapshot: &TextInputClientSnapshot,
        _generation: TextInputFocusGeneration,
        _geometry: Option<&TextInputGeometrySnapshot>,
    ) -> Result<(), NativeTextInputBackendError> {
        Ok(())
    }

    fn update_snapshot(
        &mut self,
        _snapshot: &TextInputClientSnapshot,
    ) -> Result<(), NativeTextInputBackendError> {
        Ok(())
    }

    fn update_geometry(
        &mut self,
        _geometry: &TextInputGeometrySnapshot,
    ) -> Result<(), NativeTextInputBackendError> {
        Ok(())
    }

    fn blur(
        &mut self,
        _policy: arcweft_presentation::text_input::TextInputBlurPolicy,
    ) -> Result<(), NativeTextInputBackendError> {
        Ok(())
    }

    fn drain_platform_events(&mut self) -> Vec<PlatformTextInputEvent> {
        let _ = &self.unavailable;
        Vec::new()
    }
}

const fn unsupported_capabilities() -> TextInputCapabilities {
    TextInputCapabilities {
        surrounding_text: TextInputCapabilitySupport::Unsupported,
        delete_surrounding: TextInputCapabilitySupport::Unsupported,
        reconversion: TextInputCapabilitySupport::Unsupported,
        composition_segments: TextInputCapabilitySupport::Unsupported,
        character_bounds: TextInputCapabilitySupport::Unsupported,
        programmatic_commit: TextInputCapabilitySupport::Unsupported,
        programmatic_cancel: TextInputCapabilitySupport::Unsupported,
    }
}
