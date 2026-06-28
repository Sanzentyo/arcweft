//! Safe owner for the real Windows TSF IME bridge.
//!
//! This module is the public Windows-only API. The COM implementation and raw
//! pointer work are confined to `unsafe_com`.

use crate::text_input::windows_tsf::capabilities::{
    WindowsTsfDisplayAttributeState, WindowsTsfLayoutState, WindowsTsfReconversionState,
    WindowsTsfRuntimeFacts,
};
use crate::text_input::windows_tsf::edit_session::WindowsTsfEditAccess;
use crate::text_input::windows_tsf::{WindowsTsfActivation, WindowsTsfAdapter};
use arcweft_presentation::text_input::{
    PlatformTextInputEvent, TextInputBlurPolicy, TextInputClientSnapshot, TextInputFocusGeneration,
    TextInputGeometrySnapshot, TextInputSecurityPolicy, TextInputSerial,
};
use thiserror::Error;
use windows::Win32::Foundation::HWND;

use super::unsafe_com::{TsfComError, TsfDocumentUpdate, WindowsTsfThreadContext};

/// Safe owner for one HWND's TSF thread-manager/document-manager lifetime.
#[derive(Debug)]
pub struct WindowsTsfImeBridge {
    thread: WindowsTsfThreadContext,
    adapter: WindowsTsfAdapter,
    activation: WindowsTsfActivation,
    active: Option<WindowsTsfImeFocus>,
}

/// TSF activation report surfaced to the native player/window owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsTsfImeActivation {
    generation: TextInputFocusGeneration,
    activation: WindowsTsfActivation,
}

/// Current Arcweft focus identity mirrored into the TSF text store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsTsfImeFocus {
    generation: TextInputFocusGeneration,
    security: TextInputSecurityPolicy,
}

/// Safe error type for the real IME bridge.
#[derive(Debug, Error)]
pub enum WindowsTsfImeError {
    #[error("Windows TSF COM bridge failed: {0}")]
    Com(#[from] TsfComError),
    #[error("no active Windows TSF text input focus")]
    NoActiveFocus,
    #[error("stale Windows TSF focus generation: active {active:?}, incoming {incoming:?}")]
    StaleFocusGeneration {
        active: TextInputFocusGeneration,
        incoming: TextInputFocusGeneration,
    },
}

impl WindowsTsfImeBridge {
    /// Creates a TSF bridge for a native window.
    pub fn new_for_window(hwnd: HWND) -> Result<Self, WindowsTsfImeError> {
        let thread = WindowsTsfThreadContext::activate(hwnd)?;
        let facts = thread_runtime_facts(&thread, TextInputSecurityPolicy::Plain);
        let (adapter, activation) = WindowsTsfAdapter::activate(facts);
        Ok(Self {
            thread,
            adapter,
            activation,
            active: None,
        })
    }

    /// Returns the most recent capability activation report.
    pub const fn activation(&self) -> &WindowsTsfActivation {
        &self.activation
    }

    /// Installs or replaces the active Arcweft text input focus in TSF.
    pub fn focus_text_input(
        &mut self,
        snapshot: &TextInputClientSnapshot,
        generation: TextInputFocusGeneration,
        geometry: Option<&TextInputGeometrySnapshot>,
    ) -> Result<WindowsTsfImeActivation, WindowsTsfImeError> {
        let security = TextInputSecurityPolicy::from_options(snapshot.options());
        let facts = thread_runtime_facts(&self.thread, security);
        let (adapter, activation) =
            WindowsTsfAdapter::activate(facts).with_first_serial_pair(self.adapter_next_serial());
        self.adapter = adapter;
        self.activation = activation.clone();

        let update = TsfDocumentUpdate::new(snapshot, generation, security).with_geometry(geometry);
        self.thread.focus_text_input(update)?;
        self.active = Some(WindowsTsfImeFocus {
            generation,
            security,
        });
        Ok(WindowsTsfImeActivation {
            generation,
            activation,
        })
    }

    /// Updates text snapshot after shared editor mutation.
    pub fn update_snapshot(
        &mut self,
        snapshot: &TextInputClientSnapshot,
    ) -> Result<(), WindowsTsfImeError> {
        let active = self
            .active
            .as_ref()
            .ok_or(WindowsTsfImeError::NoActiveFocus)?;
        let update = TsfDocumentUpdate::new(snapshot, active.generation, active.security);
        self.thread.update_document(update)?;
        Ok(())
    }

    /// Updates renderer-backed geometry for candidate windows.
    pub fn update_geometry(
        &mut self,
        geometry: &TextInputGeometrySnapshot,
    ) -> Result<(), WindowsTsfImeError> {
        self.thread.update_geometry(geometry)?;
        Ok(())
    }

    /// Ends Arcweft focus and deactivates TSF document focus.
    pub fn blur(&mut self, _policy: TextInputBlurPolicy) -> Result<(), WindowsTsfImeError> {
        self.active = None;
        self.thread.blur()?;
        Ok(())
    }

    /// Drains platform events created during TSF write callbacks.
    pub fn drain_platform_events(&mut self) -> Vec<PlatformTextInputEvent> {
        self.thread
            .drain_operations()
            .into_iter()
            .filter_map(|batch| {
                let snapshot = batch.snapshot();
                let mut builder = self.adapter.begin_edit_session(
                    snapshot,
                    batch.generation(),
                    WindowsTsfEditAccess::ReadWrite,
                );
                for operation in batch.into_operations() {
                    builder.push_operation(operation);
                }
                builder.finish()
            })
            .collect()
    }

    fn adapter_next_serial(&self) -> TextInputSerial {
        self.thread.next_arcweft_serial_hint()
    }
}

impl WindowsTsfImeActivation {
    pub const fn generation(&self) -> TextInputFocusGeneration {
        self.generation
    }

    pub const fn activation(&self) -> &WindowsTsfActivation {
        &self.activation
    }
}

trait WindowsTsfAdapterSerialReset {
    fn with_first_serial_pair(
        self,
        first: TextInputSerial,
    ) -> (WindowsTsfAdapter, WindowsTsfActivation);
}

impl WindowsTsfAdapterSerialReset for (WindowsTsfAdapter, WindowsTsfActivation) {
    fn with_first_serial_pair(
        self,
        first: TextInputSerial,
    ) -> (WindowsTsfAdapter, WindowsTsfActivation) {
        (self.0.with_first_serial(first), self.1)
    }
}

fn thread_runtime_facts(
    thread: &WindowsTsfThreadContext,
    security: TextInputSecurityPolicy,
) -> WindowsTsfRuntimeFacts {
    let mut facts = WindowsTsfRuntimeFacts::default()
        .with_runtime_ready()
        .with_layout_state(if thread.has_layout() {
            WindowsTsfLayoutState::Available
        } else {
            WindowsTsfLayoutState::Unavailable
        })
        .with_display_attribute_state(WindowsTsfDisplayAttributeState::MappedWithFixtureCoverage)
        .with_security(security);
    if thread.reconversion_available() {
        facts = facts.with_reconversion_state(WindowsTsfReconversionState::FunctionAvailable);
    }
    facts
}
