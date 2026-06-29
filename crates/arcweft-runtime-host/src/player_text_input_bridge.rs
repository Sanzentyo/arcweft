//! Shared player-level text-input bridge core.
//!
//! Platform players own host objects and event listeners. This module owns the
//! common Sans I/O lifecycle around the Arcweft text-input contract: focus
//! activation, snapshot and geometry publication, blur policy, dispatch-state
//! validation, key-shortcut admission, and validated edit extraction.

use crate::{TextInputDispatchError, TextInputDispatchState};
#[cfg(test)]
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::input::{InputEpoch, RawInputKind};
use arcweft_presentation::text_input::{
    PlatformTextInputEvent, TextInput, TextInputBlurPolicy, TextInputCapabilities,
    TextInputClientSnapshot, TextInputFocusGeneration, TextInputGeometrySnapshot,
    TextInputHostCommand, TextInputKeyDisposition, TextInputSecurityPolicy, TextInputSessionId,
};

/// Focused Arcweft text control prepared by the renderer/player boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerTextInputFocusedControl {
    snapshot: TextInputClientSnapshot,
    geometry: TextInputGeometrySnapshot,
    capabilities: TextInputCapabilities,
}

/// Player-level text-input core shared by native and Web shells.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerTextInputBridgeCore {
    dispatch: TextInputDispatchState,
    blur_policy: TextInputBlurPolicy,
    next_epoch: u64,
}

/// Phase emitted by one focus synchronization pass.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlayerTextInputSyncPhase {
    #[default]
    Idle,
    Activated,
    Updated,
    Blurred,
}

/// Result of synchronizing player focus with a platform host command sink.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerTextInputSync {
    phase: PlayerTextInputSyncPhase,
    generation: TextInputFocusGeneration,
    security: TextInputSecurityPolicy,
    commands: Vec<TextInputHostCommand>,
}

/// Validated platform edit ready for `InputController::text_input`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerTextInputEdit {
    input: TextInput,
    key_disposition: TextInputKeyDisposition,
}

/// Value-only host command sink implemented by native/Web shells.
///
/// Implementations may hold browser, TSF, `AppKit`, winit, or future platform
/// object identity, but those identities never cross this trait boundary.
pub trait PlayerTextInputHostCommandSink {
    type Error;

    fn apply_text_input_command(
        &mut self,
        command: &TextInputHostCommand,
        generation: TextInputFocusGeneration,
        geometry: Option<&TextInputGeometrySnapshot>,
    ) -> Result<(), Self::Error>;
}

impl PlayerTextInputFocusedControl {
    pub fn new(
        snapshot: TextInputClientSnapshot,
        geometry: TextInputGeometrySnapshot,
        capabilities: TextInputCapabilities,
    ) -> Self {
        Self {
            snapshot,
            geometry,
            capabilities,
        }
    }

    pub const fn snapshot(&self) -> &TextInputClientSnapshot {
        &self.snapshot
    }

    pub const fn geometry(&self) -> &TextInputGeometrySnapshot {
        &self.geometry
    }

    pub const fn capabilities(&self) -> TextInputCapabilities {
        self.capabilities
    }
}

impl Default for PlayerTextInputBridgeCore {
    fn default() -> Self {
        Self {
            dispatch: TextInputDispatchState::default(),
            blur_policy: TextInputBlurPolicy::PlatformDefault,
            next_epoch: 1,
        }
    }
}

impl PlayerTextInputBridgeCore {
    #[must_use]
    pub const fn with_blur_policy(mut self, blur_policy: TextInputBlurPolicy) -> Self {
        self.blur_policy = blur_policy;
        self
    }

    pub const fn blur_policy(&self) -> TextInputBlurPolicy {
        self.blur_policy
    }

    pub const fn active_session(&self) -> Option<TextInputSessionId> {
        match self.dispatch.active() {
            Some(active) => Some(active.session()),
            None => None,
        }
    }

    pub const fn active_generation(&self) -> Option<TextInputFocusGeneration> {
        match self.dispatch.active() {
            Some(active) => Some(active.generation()),
            None => None,
        }
    }

    pub const fn active_security(&self) -> Option<TextInputSecurityPolicy> {
        match self.dispatch.active() {
            Some(active) => Some(active.security()),
            None => None,
        }
    }

    pub const fn active_capabilities(&self) -> Option<TextInputCapabilities> {
        match self.dispatch.active() {
            Some(active) => Some(active.capabilities()),
            None => None,
        }
    }

    pub fn sync_focus(
        &mut self,
        focused: Option<&PlayerTextInputFocusedControl>,
    ) -> Result<PlayerTextInputSync, TextInputDispatchError> {
        let Some(focused) = focused else {
            return Ok(self.blur_active());
        };
        let snapshot = focused.snapshot();
        let security = TextInputSecurityPolicy::from_options(snapshot.options());
        let capabilities = focused.capabilities().narrow_for_security(security);
        let focus_changed = self.dispatch.active().is_none_or(|active| {
            active.session() != snapshot.session()
                || active.target() != snapshot.target()
                || active.security() != security
                || active.capabilities() != capabilities
        });

        if focus_changed {
            let mut commands = self.blur_active().into_commands();
            let transaction = self
                .dispatch
                .activate_with_capabilities(snapshot, capabilities);
            let generation = transaction.generation();
            commands.extend(transaction.into_commands());
            commands.push(self.dispatch.update_geometry(focused.geometry())?);
            return Ok(PlayerTextInputSync {
                phase: PlayerTextInputSyncPhase::Activated,
                generation,
                security,
                commands,
            });
        }

        let generation = self
            .dispatch
            .active()
            .map_or(self.dispatch.focus_generation(), |active| {
                active.generation()
            });
        Ok(PlayerTextInputSync {
            phase: PlayerTextInputSyncPhase::Updated,
            generation,
            security,
            commands: vec![
                self.dispatch.update_snapshot(snapshot)?,
                self.dispatch.update_geometry(focused.geometry())?,
            ],
        })
    }

    #[must_use]
    pub fn blur_active(&mut self) -> PlayerTextInputSync {
        let transaction = self.dispatch.blur(self.blur_policy);
        let phase = if transaction.commands().is_empty() {
            PlayerTextInputSyncPhase::Idle
        } else {
            PlayerTextInputSyncPhase::Blurred
        };
        PlayerTextInputSync {
            phase,
            generation: transaction.generation(),
            security: TextInputSecurityPolicy::Plain,
            commands: transaction.into_commands(),
        }
    }

    pub fn dispatch_platform_event(
        &mut self,
        event: PlatformTextInputEvent,
        key_disposition: TextInputKeyDisposition,
    ) -> Result<PlayerTextInputEdit, TextInputDispatchError> {
        let epoch = self.next_epoch();
        let output = self
            .dispatch
            .dispatch_platform_event(epoch, event, key_disposition)?;
        let key_disposition = output.key_disposition();
        let raw = output.into_raw();
        let RawInputKind::Text(input) = raw.kind() else {
            unreachable!("TextInputDispatchState emits text raw events for platform text input")
        };
        Ok(PlayerTextInputEdit {
            input: input.clone(),
            key_disposition,
        })
    }

    pub fn shortcuts_allowed(&self, disposition: TextInputKeyDisposition) -> bool {
        self.dispatch.shortcuts_allowed(disposition)
    }

    fn next_epoch(&mut self) -> InputEpoch {
        let epoch = InputEpoch(self.next_epoch);
        self.next_epoch = self.next_epoch.saturating_add(1);
        epoch
    }
}

impl PlayerTextInputSync {
    pub const fn phase(&self) -> PlayerTextInputSyncPhase {
        self.phase
    }

    pub const fn generation(&self) -> TextInputFocusGeneration {
        self.generation
    }

    pub const fn security(&self) -> TextInputSecurityPolicy {
        self.security
    }

    pub fn commands(&self) -> &[TextInputHostCommand] {
        &self.commands
    }

    pub fn into_commands(self) -> Vec<TextInputHostCommand> {
        self.commands
    }
}

impl PlayerTextInputEdit {
    pub const fn input(&self) -> &TextInput {
        &self.input
    }

    pub const fn key_disposition(&self) -> TextInputKeyDisposition {
        self.key_disposition
    }

    pub fn into_input(self) -> TextInput {
        self.input
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::text_input::{
        PlatformTextInputContext, TextByteOffset, TextCommit, TextGeometryTransform,
        TextInputAdapterKind, TextInputGeometrySnapshotParts, TextInputOperation, TextInputOptions,
        TextInputSerial, TextRange, TextRevision, TextWritingMode,
    };

    fn target(name: &str) -> InteractionTarget {
        InteractionTarget::new(PublicId::try_new(format!("target.{name}")).unwrap())
    }

    fn snapshot(
        session: u64,
        target: InteractionTarget,
        text: &str,
        secure: bool,
    ) -> TextInputClientSnapshot {
        let end = TextByteOffset(u32::try_from(text.len()).unwrap());
        TextInputClientSnapshot::new(
            TextInputSessionId(session),
            target,
            TextRevision(1),
            text,
            TextByteOffset(0),
            TextRange::new(end, end),
            HitRect::new(10.0, 20.0, 240.0, 32.0),
            HitRect::new(38.0, 24.0, 1.0, 24.0),
            TextInputOptions::default().secure(secure),
        )
    }

    fn geometry(session: u64, text: &str) -> TextInputGeometrySnapshot {
        let end = TextByteOffset(u32::try_from(text.len()).unwrap());
        TextInputGeometrySnapshot::new(TextInputGeometrySnapshotParts {
            session: TextInputSessionId(session),
            revision: TextRevision(1),
            writing_mode: TextWritingMode::HorizontalTb,
            text_local_control_rect: HitRect::new(0.0, 0.0, 240.0, 32.0),
            text_local_caret_rect: HitRect::new(28.0, 4.0, 1.0, 24.0),
            text_local_character_bounds: vec![
                arcweft_presentation::text_input::TextCharacterBounds::new(
                    TextRange::new(TextByteOffset(0), end),
                    HitRect::new(0.0, 0.0, 48.0, 24.0),
                ),
            ],
            text_local_selection_rects: Vec::new(),
            text_local_composition_rects: Vec::new(),
            text_local_to_viewport: TextGeometryTransform::translation(10.0, 20.0),
            viewport_to_screen: TextGeometryTransform::translation(100.0, 200.0),
        })
    }

    fn focused(
        session: u64,
        target: InteractionTarget,
        text: &str,
        secure: bool,
    ) -> PlayerTextInputFocusedControl {
        PlayerTextInputFocusedControl::new(
            snapshot(session, target, text, secure),
            geometry(session, text),
            TextInputCapabilities::all_supported(),
        )
    }

    fn context(
        core: &PlayerTextInputBridgeCore,
        target: InteractionTarget,
        serial: u64,
    ) -> PlatformTextInputContext {
        PlatformTextInputContext::new(
            TextInputAdapterKind::WebEditContext,
            core.active_session().unwrap(),
            core.active_generation().unwrap(),
            target,
            TextInputSerial(serial),
        )
    }

    #[test]
    fn focus_update_and_blur_emit_common_host_commands() {
        let target = target("plain");
        let mut core = PlayerTextInputBridgeCore::default()
            .with_blur_policy(TextInputBlurPolicy::CommitComposition);

        let activation = core
            .sync_focus(Some(&focused(1, target.clone(), "abc", false)))
            .unwrap();
        assert_eq!(activation.phase(), PlayerTextInputSyncPhase::Activated);
        assert!(matches!(
            activation.commands()[0],
            TextInputHostCommand::Activate { .. }
        ));
        assert!(matches!(
            activation.commands()[1],
            TextInputHostCommand::UpdateGeometry(_)
        ));

        let update = core
            .sync_focus(Some(&focused(1, target.clone(), "abcd", false)))
            .unwrap();
        assert_eq!(update.phase(), PlayerTextInputSyncPhase::Updated);
        assert!(matches!(
            update.commands()[0],
            TextInputHostCommand::Update(_)
        ));
        assert!(matches!(
            update.commands()[1],
            TextInputHostCommand::UpdateGeometry(_)
        ));

        let event = PlatformTextInputEvent::StartComposition(context(&core, target, 1));
        let _ = core
            .dispatch_platform_event(event, TextInputKeyDisposition::ImeConsumed)
            .unwrap();
        let blur = core.blur_active();
        assert_eq!(blur.phase(), PlayerTextInputSyncPhase::Blurred);
        assert!(
            blur.commands()
                .iter()
                .any(|command| matches!(command, TextInputHostCommand::CommitComposition { .. }))
        );
        assert!(
            blur.commands()
                .iter()
                .any(|command| matches!(command, TextInputHostCommand::Deactivate { .. }))
        );
    }

    #[test]
    fn platform_dispatch_routes_text_and_rejects_stale_serial() {
        let target = target("dispatch");
        let mut core = PlayerTextInputBridgeCore::default();
        core.sync_focus(Some(&focused(9, target.clone(), "", false)))
            .unwrap();

        let edit = core
            .dispatch_platform_event(
                PlatformTextInputEvent::Commit {
                    context: context(&core, target.clone(), 1),
                    commit: TextCommit::new("日"),
                },
                TextInputKeyDisposition::ImeConsumed,
            )
            .unwrap();
        assert_eq!(edit.key_disposition(), TextInputKeyDisposition::ImeConsumed);
        assert!(matches!(
            edit.input().operations(),
            [TextInputOperation::Commit(_)]
        ));

        let error = core
            .dispatch_platform_event(
                PlatformTextInputEvent::Commit {
                    context: context(&core, target, 1),
                    commit: TextCommit::new("本"),
                },
                TextInputKeyDisposition::ImeConsumed,
            )
            .expect_err("stale serial rejects");
        assert!(matches!(error, TextInputDispatchError::StaleSerial { .. }));
    }

    #[test]
    fn secure_focus_redacts_snapshot_and_geometry_commands() {
        let target = target("secure");
        let mut core = PlayerTextInputBridgeCore::default();
        let activation = core
            .sync_focus(Some(&focused(7, target, "secret", true)))
            .unwrap();

        let TextInputHostCommand::Activate { snapshot, .. } = &activation.commands()[0] else {
            panic!("activate command expected");
        };
        assert!(snapshot.surrounding_text().is_empty());
        assert!(snapshot.character_bounds().is_empty());
        let TextInputHostCommand::UpdateGeometry(geometry) = &activation.commands()[1] else {
            panic!("geometry command expected");
        };
        assert!(geometry.screen_character_bounds().is_empty());
    }

    #[test]
    fn shortcuts_follow_dispatch_composition_state() {
        let target = target("shortcuts");
        let mut core = PlayerTextInputBridgeCore::default();
        core.sync_focus(Some(&focused(3, target.clone(), "", false)))
            .unwrap();
        assert!(core.shortcuts_allowed(TextInputKeyDisposition::ShortcutCandidate));
        assert!(!core.shortcuts_allowed(TextInputKeyDisposition::ImeConsumed));

        let _ = core
            .dispatch_platform_event(
                PlatformTextInputEvent::StartComposition(context(&core, target, 1)),
                TextInputKeyDisposition::ImeConsumed,
            )
            .unwrap();
        assert!(!core.shortcuts_allowed(TextInputKeyDisposition::ShortcutCandidate));
    }
}
