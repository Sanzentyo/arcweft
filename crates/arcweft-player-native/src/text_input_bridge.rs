//! Player-owned bridge for native window text input.
//!
//! The native scene/window owner integrates with this module once.  The normal
//! player route uses winit window IME and keyboard-text events as its source,
//! then normalizes those events into player-owned `TextInput` batches before
//! anything is routed back into the scene.

mod backend;
mod trace;

pub use trace::NativeTextInputTraceOptions;

use self::backend::NativeTextInputBackendIdentity;
use self::trace::NativeTextInputTraceWriter;
#[cfg(test)]
use arcweft_presentation::input::InteractionTarget;
#[cfg(test)]
use arcweft_presentation::text_input::TextInputSessionId;
use arcweft_presentation::text_input::{
    TextControlWriteBack, TextInput, TextInputBlurPolicy, TextInputCapabilities,
    TextInputClientSnapshot, TextInputGeometrySnapshot, TextInputKeyDisposition,
    TextInputSecurityPolicy,
};
use arcweft_runtime_host::{
    PlayerTextInputBridgeCore, PlayerTextInputFocusedControl, PlayerTextInputSyncPhase,
    TextInputDispatchError,
};
use thiserror::Error;

/// Native text-input bridge options supplied by the normal player run path.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeTextInputBridgeOptions {
    trace: NativeTextInputTraceOptions,
    blur_policy: TextInputBlurPolicy,
}

/// Focus reason recorded in player-owned native text-input traces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTextInputFocusReason {
    Pointer,
    RedrawRefresh,
    #[cfg(test)]
    Fixture,
}

/// Focused Arcweft text control snapshot plus renderer-backed geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeTextInputFocusedControl {
    focused: PlayerTextInputFocusedControl,
    reason: NativeTextInputFocusReason,
}

/// Cross-platform bridge owned by one native player window.
#[derive(Debug)]
pub(crate) struct NativeTextInputBridge {
    core: PlayerTextInputBridgeCore,
    backend: NativeTextInputBackendIdentity,
    trace: NativeTextInputTraceWriter,
}

/// Bridge-level error.  Native backend errors are contained in the player/native
/// crate and do not cross into Sans I/O crates.
#[derive(Debug, Error)]
pub enum NativeTextInputBridgeError {
    #[error("text-input dispatch failed: {0}")]
    Dispatch(#[from] TextInputDispatchError),
}

impl NativeTextInputBridgeOptions {
    #[must_use]
    pub fn with_trace(mut self, trace: NativeTextInputTraceOptions) -> Self {
        self.trace = trace;
        self
    }

    #[must_use]
    pub fn with_blur_policy(mut self, blur_policy: TextInputBlurPolicy) -> Self {
        self.blur_policy = blur_policy;
        self
    }

    pub const fn trace(&self) -> &NativeTextInputTraceOptions {
        &self.trace
    }

    pub const fn blur_policy(&self) -> TextInputBlurPolicy {
        self.blur_policy
    }
}

impl NativeTextInputFocusedControl {
    pub fn new(
        snapshot: TextInputClientSnapshot,
        geometry: TextInputGeometrySnapshot,
        reason: NativeTextInputFocusReason,
    ) -> Self {
        Self {
            focused: PlayerTextInputFocusedControl::new(
                snapshot,
                geometry,
                TextInputCapabilities::all_supported(),
            ),
            reason,
        }
    }

    pub const fn snapshot(&self) -> &TextInputClientSnapshot {
        self.focused.snapshot()
    }

    pub const fn geometry(&self) -> &TextInputGeometrySnapshot {
        self.focused.geometry()
    }

    fn with_capabilities(self, capabilities: TextInputCapabilities) -> Self {
        Self {
            focused: PlayerTextInputFocusedControl::new(
                self.focused.snapshot().clone(),
                self.focused.geometry().clone(),
                capabilities,
            ),
            reason: self.reason,
        }
    }

    const fn focused(&self) -> &PlayerTextInputFocusedControl {
        &self.focused
    }

    pub const fn reason(&self) -> NativeTextInputFocusReason {
        self.reason
    }
}

impl NativeTextInputBridge {
    pub(crate) fn new(options: NativeTextInputBridgeOptions) -> Self {
        let NativeTextInputBridgeOptions { trace, blur_policy } = options;
        let backend = NativeTextInputBackendIdentity::winit_window_ime();
        let mut trace = NativeTextInputTraceWriter::new(trace);
        trace.record_backend_selected(backend);
        trace.record_capabilities(backend, backend.capabilities());
        if let Some(reason) = backend.unavailable_reason() {
            trace.record_backend_unavailable(backend, reason);
        }
        Self {
            core: PlayerTextInputBridgeCore::default().with_blur_policy(blur_policy),
            backend,
            trace,
        }
    }

    pub(crate) fn backend_key_disposition(&mut self, key: &str) -> TextInputKeyDisposition {
        let disposition = self.backend.key_disposition();
        self.trace
            .record_key_disposition(self.backend, key, disposition);
        disposition
    }

    pub(crate) fn sync_focus(
        &mut self,
        focused: Option<NativeTextInputFocusedControl>,
    ) -> Result<(), NativeTextInputBridgeError> {
        let Some(focused) = focused else {
            self.blur_active();
            return Ok(());
        };
        let focused = focused.with_capabilities(self.backend.capabilities());
        let snapshot = focused.snapshot();
        let sync = if focused.reason() == NativeTextInputFocusReason::Pointer {
            self.core
                .sync_focus_for_user_activation(Some(focused.focused()))?
        } else {
            self.core.sync_focus(Some(focused.focused()))?
        };
        match sync.phase() {
            PlayerTextInputSyncPhase::Activated => {
                self.trace.record_focus(
                    self.backend,
                    focused.reason(),
                    snapshot,
                    sync.generation(),
                    sync.security(),
                );
            }
            PlayerTextInputSyncPhase::Updated => {
                self.trace.record_snapshot(
                    self.backend,
                    "update",
                    snapshot,
                    sync.generation(),
                    sync.security(),
                );
            }
            PlayerTextInputSyncPhase::Idle | PlayerTextInputSyncPhase::Blurred => {}
        }
        if matches!(
            sync.phase(),
            PlayerTextInputSyncPhase::Activated | PlayerTextInputSyncPhase::Updated
        ) {
            self.trace
                .record_geometry(self.backend, focused.geometry(), sync.security());
        }
        Ok(())
    }

    pub(crate) fn blur_active(&mut self) {
        let ended_session = self.core.active_session();
        let sync = self.core.blur_active();
        if sync.phase() == PlayerTextInputSyncPhase::Idle {
            return;
        }
        self.trace.record_blur(self.backend, ended_session);
    }

    pub(crate) fn shortcuts_allowed(&self, disposition: TextInputKeyDisposition) -> bool {
        self.core.shortcuts_allowed(disposition)
    }

    pub(crate) fn record_runtime_write_backs<'a>(
        &mut self,
        write_backs: impl IntoIterator<Item = &'a TextControlWriteBack>,
    ) {
        for write_back in write_backs {
            self.trace
                .record_runtime_write_back(self.backend, write_back);
        }
    }

    pub(crate) fn record_window_ime_text_input(
        &mut self,
        input: &TextInput,
        disposition: TextInputKeyDisposition,
    ) {
        let security = self
            .core
            .active_security()
            .unwrap_or(TextInputSecurityPolicy::Plain);
        self.trace
            .record_routed_text_input(self.backend, input, disposition, security);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::text_input::{
        TextByteOffset, TextGeometryTransform, TextInputGeometrySnapshotParts, TextInputOptions,
        TextRange, TextRevision, TextWritingMode,
    };

    fn target(name: &str) -> InteractionTarget {
        InteractionTarget::new(PublicId::try_new(format!("target.{name}")).unwrap())
    }

    fn snapshot(session: u64, target: InteractionTarget, secure: bool) -> TextInputClientSnapshot {
        TextInputClientSnapshot::new(
            TextInputSessionId(session),
            target,
            TextRevision(0),
            "かな",
            TextByteOffset(0),
            TextRange::new(TextByteOffset(0), TextByteOffset(6)),
            HitRect::new(10.0, 20.0, 200.0, 28.0),
            HitRect::new(18.0, 22.0, 1.0, 20.0),
            TextInputOptions::default().secure(secure),
        )
    }

    fn geometry(session: u64) -> TextInputGeometrySnapshot {
        TextInputGeometrySnapshot::new(TextInputGeometrySnapshotParts {
            session: TextInputSessionId(session),
            revision: TextRevision(0),
            writing_mode: TextWritingMode::HorizontalTb,
            text_local_control_rect: HitRect::new(0.0, 0.0, 200.0, 28.0),
            text_local_caret_rect: HitRect::new(8.0, 2.0, 1.0, 20.0),
            text_local_character_bounds: Vec::new(),
            text_local_selection_rects: Vec::new(),
            text_local_composition_rects: Vec::new(),
            text_local_to_viewport: TextGeometryTransform::translation(10.0, 20.0),
            viewport_to_screen: TextGeometryTransform::translation(100.0, 200.0),
        })
    }

    #[test]
    fn winit_backend_reports_capabilities_without_native_identity() {
        let mut bridge = NativeTextInputBridge::new(NativeTextInputBridgeOptions::default());
        let focus = NativeTextInputFocusedControl::new(
            snapshot(1, target("plain"), false),
            geometry(1),
            NativeTextInputFocusReason::Fixture,
        );

        bridge.sync_focus(Some(focus)).unwrap();
        let trace = bridge.trace.records_for_tests();

        assert!(
            trace
                .iter()
                .any(|record| record.contains("winit_window_ime"))
        );
        assert!(!trace.iter().any(|record| record.contains("windows_tsf")));
    }

    #[test]
    fn secure_focus_trace_is_redacted_before_backend_publication() {
        let mut bridge = NativeTextInputBridge::new(NativeTextInputBridgeOptions::default());
        let focus = NativeTextInputFocusedControl::new(
            snapshot(7, target("secure"), true),
            geometry(7),
            NativeTextInputFocusReason::Fixture,
        );

        bridge.sync_focus(Some(focus)).unwrap();
        let trace = bridge.trace.records_for_tests();

        assert!(
            trace
                .iter()
                .any(|record| record.contains("secure_redacted"))
        );
        assert!(!trace.iter().any(|record| record.contains("かな")));
    }
}
