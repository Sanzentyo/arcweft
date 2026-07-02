//! Native-player text input source boundary.
//!
//! The normal winit-backed player uses winit window IME and keyboard-text
//! events as its single text source.  Platform-specific TSF/AppKit adapter
//! experiments stay in diagnostic or desktop-native boundaries so they do not
//! compete with winit for the same live window.

use arcweft_presentation::text_input::{
    TextInputCapabilities, TextInputCapabilitySupport, TextInputKeyDisposition,
};
use serde::{Deserialize, Serialize};

/// Trace-safe backend identity.  It intentionally contains no native handle or
/// object identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeTextInputBackendIdentity {
    WinitWindowIme,
}

impl NativeTextInputBackendIdentity {
    pub(crate) const fn winit_window_ime() -> Self {
        Self::WinitWindowIme
    }

    pub(crate) const fn capabilities(self) -> TextInputCapabilities {
        match self {
            Self::WinitWindowIme => TextInputCapabilities {
                surrounding_text: TextInputCapabilitySupport::HostDependent,
                delete_surrounding: TextInputCapabilitySupport::HostDependent,
                reconversion: TextInputCapabilitySupport::Unsupported,
                composition_segments: TextInputCapabilitySupport::Unsupported,
                character_bounds: TextInputCapabilitySupport::Limited,
                programmatic_commit: TextInputCapabilitySupport::Unsupported,
                programmatic_cancel: TextInputCapabilitySupport::Unsupported,
            },
        }
    }

    pub(crate) const fn unavailable_reason(self) -> Option<&'static str> {
        match self {
            Self::WinitWindowIme => None,
        }
    }

    pub(crate) const fn key_disposition(self) -> TextInputKeyDisposition {
        match self {
            Self::WinitWindowIme => TextInputKeyDisposition::ShortcutCandidate,
        }
    }
}
