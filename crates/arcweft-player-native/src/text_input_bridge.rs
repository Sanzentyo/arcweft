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
use arcweft_presentation::input::{InputEpoch, InteractionTarget, RawInputKind};
use arcweft_presentation::text_input::{
    PlatformTextInputEvent, TextInput, TextInputBlurPolicy, TextInputClientSnapshot,
    TextInputFocusGeneration, TextInputGeometrySnapshot, TextInputHostCommand,
    TextInputKeyDisposition, TextInputSecurityPolicy, TextInputSessionId,
};
use arcweft_runtime_host::{TextInputDispatchError, TextInputDispatchState};
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
    snapshot: TextInputClientSnapshot,
    geometry: TextInputGeometrySnapshot,
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
    dispatch: TextInputDispatchState,
    backend: NativeTextInputBackend,
    trace: NativeTextInputTraceWriter,
    active: Option<NativeTextInputActiveFocus>,
    next_epoch: u64,
    blur_policy: TextInputBlurPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeTextInputActiveFocus {
    session: TextInputSessionId,
    target: InteractionTarget,
    generation: TextInputFocusGeneration,
    security: TextInputSecurityPolicy,
}

/// Bridge-level error.  Native backend errors are contained in the player/native
/// crate and do not cross into Sans I/O crates.
#[derive(Debug, Error)]
pub enum NativeTextInputBridgeError {
    #[error("native text-input backend failed: {0}")]
    Backend(#[from] NativeTextInputBackendError),
    #[error("text-input dispatch failed: {0}")]
    Dispatch(#[from] TextInputDispatchError),
    #[error("backend command was requested without an active bridge focus")]
    NoActiveFocus,
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
            snapshot,
            geometry,
            reason,
        }
    }

    pub const fn snapshot(&self) -> &TextInputClientSnapshot {
        &self.snapshot
    }

    pub const fn geometry(&self) -> &TextInputGeometrySnapshot {
        &self.geometry
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
        trace.record_capabilities(backend.identity(), backend.capabilities());
        if let Some(reason) = backend.unavailable_reason() {
            trace.record_backend_unavailable(backend.identity(), reason);
        }
        Self {
            dispatch: TextInputDispatchState::default(),
            backend,
            trace,
            active: None,
            next_epoch: 1,
            blur_policy,
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
        let snapshot = focused.snapshot();
        let security = TextInputSecurityPolicy::from_options(snapshot.options());
        let target = snapshot.target().clone();
        let session = snapshot.session();
        let focus_changed = self.active.as_ref().is_none_or(|active| {
            active.session != session || active.target != target || active.security != security
        });
        if focus_changed {
            self.blur_active()?;
            let transaction = self
                .dispatch
                .activate_with_capabilities(snapshot, self.backend.capabilities());
            let generation = transaction.generation();
            self.active = Some(NativeTextInputActiveFocus {
                session,
                target,
                generation,
                security,
            });
            self.trace.record_focus(
                self.backend.identity(),
                focused.reason(),
                snapshot,
                generation,
                security,
            );
            for command in transaction.commands() {
                self.apply_host_command(command, Some(focused.geometry()))?;
            }
            self.update_geometry(focused.geometry(), security)?;
            return Ok(());
        }
        self.update_snapshot(snapshot, security)?;
        self.update_geometry(focused.geometry(), security)
    }

    pub(crate) fn blur_active(&mut self) -> Result<(), NativeTextInputBridgeError> {
        if self.active.is_none() {
            return Ok(());
        }
        let transaction = self.dispatch.blur(self.blur_policy);
        self.trace
            .record_blur(self.backend.identity(), transaction.ended_session());
        for command in transaction.commands() {
            self.apply_host_command(command, None)?;
        }
        self.active = None;
        Ok(())
    }

    pub(crate) fn drain_platform_edits(
        &mut self,
        disposition: TextInputKeyDisposition,
    ) -> Result<Vec<NativeTextInputPlayerEdit>, NativeTextInputBridgeError> {
        let security = self
            .active
            .as_ref()
            .map_or(TextInputSecurityPolicy::Plain, |active| active.security);
        self.backend
            .drain_platform_events()
            .into_iter()
            .map(|event| self.dispatch_platform_event(event, disposition, security))
            .collect()
    }

    fn update_snapshot(
        &mut self,
        snapshot: &TextInputClientSnapshot,
        security: TextInputSecurityPolicy,
    ) -> Result<(), NativeTextInputBridgeError> {
        let command = self.dispatch.update_snapshot(snapshot)?;
        self.trace.record_snapshot(
            self.backend.identity(),
            "update",
            snapshot,
            self.active_generation()?,
            security,
        );
        self.apply_host_command(&command, None)
    }

    fn update_geometry(
        &mut self,
        geometry: &TextInputGeometrySnapshot,
        security: TextInputSecurityPolicy,
    ) -> Result<(), NativeTextInputBridgeError> {
        let command = self.dispatch.update_geometry(geometry)?;
        self.trace
            .record_geometry(self.backend.identity(), geometry, security);
        self.apply_host_command(&command, Some(geometry))
    }

    fn dispatch_platform_event(
        &mut self,
        event: PlatformTextInputEvent,
        disposition: TextInputKeyDisposition,
        security: TextInputSecurityPolicy,
    ) -> Result<NativeTextInputPlayerEdit, NativeTextInputBridgeError> {
        self.trace
            .record_platform_event(self.backend.identity(), &event, security);
        let output = match self.dispatch.dispatch_platform_event(
            InputEpoch(self.next_epoch),
            event,
            disposition,
        ) {
            Ok(output) => output,
            Err(error) => {
                self.trace
                    .record_dispatch_rejection(self.backend.identity(), &error);
                return Err(error.into());
            }
        };
        self.next_epoch = self.next_epoch.saturating_add(1);
        let key_disposition = output.key_disposition();
        let raw = output.into_raw();
        let input = match raw.kind() {
            RawInputKind::Text(input) => input.clone(),
            _ => unreachable!(
                "TextInputDispatchState always emits RawInputKind::Text for platform events"
            ),
        };
        self.trace.record_routed_text_input(
            self.backend.identity(),
            &input,
            key_disposition,
            security,
        );
        Ok(NativeTextInputPlayerEdit { input })
    }

    fn apply_host_command(
        &mut self,
        command: &TextInputHostCommand,
        geometry: Option<&TextInputGeometrySnapshot>,
    ) -> Result<(), NativeTextInputBridgeError> {
        match command {
            TextInputHostCommand::Activate { snapshot, .. } => {
                self.backend
                    .activate(snapshot, self.active_generation()?, geometry)?;
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
            TextInputHostCommand::Deactivate { .. } => self.backend.blur(self.blur_policy)?,
        }
        Ok(())
    }

    fn active_generation(&self) -> Result<TextInputFocusGeneration, NativeTextInputBridgeError> {
        self.active
            .as_ref()
            .map(|active| active.generation)
            .ok_or(NativeTextInputBridgeError::NoActiveFocus)
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
