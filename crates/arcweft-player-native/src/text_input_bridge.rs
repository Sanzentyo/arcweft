//! Player-owned cross-platform bridge for native platform text input.
//!
//! The native scene/window owner integrates with this module once.  Platform
//! implementations such as Windows TSF or macOS `AppKit` live behind the backend
//! enum and normalize callbacks into `PlatformTextInputEvent` before anything is
//! routed back into the scene.

mod backend;
mod platform;
mod trace;

pub(crate) use platform::NativeTextInputWindowContext;
pub use trace::NativeTextInputTraceOptions;

use self::backend::{NativeTextInputBackend, NativeTextInputBackendError};
use self::trace::NativeTextInputTraceWriter;
#[cfg(test)]
use arcweft_presentation::input::InteractionTarget;
#[cfg(test)]
use arcweft_presentation::text_input::TextInputSessionId;
use arcweft_presentation::text_input::{
    PlatformTextInputEvent, TextControlWriteBack, TextInput, TextInputBlurPolicy,
    TextInputCapabilities, TextInputClientSnapshot, TextInputFocusGeneration,
    TextInputGeometrySnapshot, TextInputHostCommand, TextInputKeyDisposition,
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

/// Validated edit drained from a platform backend and ready for
/// `InputController::text_input`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeTextInputPlayerEdit {
    input: TextInput,
}

/// Cross-platform bridge owned by one native player window.
#[derive(Debug)]
pub(crate) struct NativeTextInputBridge {
    core: PlayerTextInputBridgeCore,
    backend: NativeTextInputBackend,
    trace: NativeTextInputTraceWriter,
}

/// Bridge-level error.  Native backend errors are contained in the player/native
/// crate and do not cross into Sans I/O crates.
#[derive(Debug, Error)]
pub enum NativeTextInputBridgeError {
    #[error("native text-input backend failed: {0}")]
    Backend(#[from] NativeTextInputBackendError),
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

impl NativeTextInputPlayerEdit {
    pub fn into_input(self) -> TextInput {
        self.input
    }
}

impl NativeTextInputBridge {
    pub(crate) fn new(
        window: NativeTextInputWindowContext,
        options: NativeTextInputBridgeOptions,
    ) -> Self {
        let NativeTextInputBridgeOptions { trace, blur_policy } = options;
        let backend = NativeTextInputBackend::for_window(window);
        let mut trace = NativeTextInputTraceWriter::new(trace);
        trace.record_backend_selected(backend.identity());
        trace.record_capabilities(backend.identity(), backend.capabilities());
        if let Some(reason) = backend.unavailable_reason() {
            trace.record_backend_unavailable(backend.identity(), reason);
        }
        Self {
            core: PlayerTextInputBridgeCore::default().with_blur_policy(blur_policy),
            backend,
            trace,
        }
    }

    pub(crate) fn backend_key_disposition(&mut self, key: &str) -> TextInputKeyDisposition {
        let disposition = self.backend.filter_key(key);
        self.trace
            .record_key_disposition(self.backend.identity(), key, disposition);
        disposition
    }

    pub(crate) fn sync_focus(
        &mut self,
        focused: Option<NativeTextInputFocusedControl>,
    ) -> Result<(), NativeTextInputBridgeError> {
        let Some(focused) = focused else {
            return self.blur_active();
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
                    self.backend.identity(),
                    focused.reason(),
                    snapshot,
                    sync.generation(),
                    sync.security(),
                );
            }
            PlayerTextInputSyncPhase::Updated => {
                self.trace.record_snapshot(
                    self.backend.identity(),
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
            self.trace.record_geometry(
                self.backend.identity(),
                focused.geometry(),
                sync.security(),
            );
        }
        for command in sync.commands() {
            self.apply_host_command(command, sync.generation(), Some(focused.geometry()))?;
        }
        Ok(())
    }

    pub(crate) fn blur_active(&mut self) -> Result<(), NativeTextInputBridgeError> {
        let ended_session = self.core.active_session();
        let sync = self.core.blur_active();
        if sync.phase() == PlayerTextInputSyncPhase::Idle {
            return Ok(());
        }
        self.trace
            .record_blur(self.backend.identity(), ended_session);
        for command in sync.commands() {
            self.apply_host_command(command, sync.generation(), None)?;
        }
        Ok(())
    }

    pub(crate) fn drain_platform_edits(
        &mut self,
        disposition: TextInputKeyDisposition,
    ) -> Result<Vec<NativeTextInputPlayerEdit>, NativeTextInputBridgeError> {
        let security = self
            .core
            .active_security()
            .unwrap_or(TextInputSecurityPolicy::Plain);
        self.backend
            .drain_platform_events()
            .into_iter()
            .map(|event| self.dispatch_platform_event(event, disposition, security))
            .collect()
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
                .record_runtime_write_back(self.backend.identity(), write_back);
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
            .record_routed_text_input(self.backend.identity(), input, disposition, security);
    }

    fn dispatch_platform_event(
        &mut self,
        event: PlatformTextInputEvent,
        disposition: TextInputKeyDisposition,
        security: TextInputSecurityPolicy,
    ) -> Result<NativeTextInputPlayerEdit, NativeTextInputBridgeError> {
        self.trace
            .record_platform_event(self.backend.identity(), &event, security);
        let edit = match self.core.dispatch_platform_event(event, disposition) {
            Ok(edit) => edit,
            Err(error) => {
                self.trace
                    .record_dispatch_rejection(self.backend.identity(), &error);
                return Err(error.into());
            }
        };
        self.trace.record_routed_text_input(
            self.backend.identity(),
            edit.input(),
            edit.key_disposition(),
            security,
        );
        Ok(NativeTextInputPlayerEdit {
            input: edit.into_input(),
        })
    }

    fn apply_host_command(
        &mut self,
        command: &TextInputHostCommand,
        generation: TextInputFocusGeneration,
        geometry: Option<&TextInputGeometrySnapshot>,
    ) -> Result<(), NativeTextInputBridgeError> {
        match command {
            TextInputHostCommand::Activate { snapshot, .. } => {
                self.backend.activate(snapshot, generation, geometry)?;
            }
            TextInputHostCommand::Update(snapshot) => self.backend.update_snapshot(snapshot)?,
            TextInputHostCommand::UpdateGeometry(geometry) => {
                self.backend.update_geometry(geometry)?;
            }
            TextInputHostCommand::CommitComposition { session } => {
                self.backend.commit_composition(*session);
            }
            TextInputHostCommand::CancelComposition { session } => {
                self.backend.cancel_composition(*session);
            }
            TextInputHostCommand::Deactivate { .. } => {
                self.backend.blur(self.core.blur_policy())?;
            }
        }
        Ok(())
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
    fn unavailable_backend_reports_capabilities_without_platform_events() {
        let mut bridge = NativeTextInputBridge::new(
            NativeTextInputWindowContext::unavailable_for_tests(),
            NativeTextInputBridgeOptions::default(),
        );
        let focus = NativeTextInputFocusedControl::new(
            snapshot(1, target("plain"), false),
            geometry(1),
            NativeTextInputFocusReason::Fixture,
        );

        bridge.sync_focus(Some(focus)).unwrap();
        let edits = bridge
            .drain_platform_edits(TextInputKeyDisposition::ShortcutCandidate)
            .unwrap();

        assert!(edits.is_empty());
    }

    #[test]
    fn secure_focus_trace_is_redacted_before_backend_publication() {
        let mut bridge = NativeTextInputBridge::new(
            NativeTextInputWindowContext::unavailable_for_tests(),
            NativeTextInputBridgeOptions::default(),
        );
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
