//! Runtime-host validation and dispatch for platform IME adapters.
//!
//! This module remains Sans I/O: platform-specific TSF/AppKit/Wayland/
//! Android/iOS/Web objects are represented only by typed adapter events from
//! `arcweft-presentation::text_input`.

use arcweft_presentation::input::{InputEpoch, InteractionTarget, RawInputEvent, RawInputKind};
#[cfg(test)]
use arcweft_presentation::text_input::TextCommit;
use arcweft_presentation::text_input::{
    PlatformTextInputContext, PlatformTextInputEvent, TextEditCommand, TextInput,
    TextInputBlurPolicy, TextInputCapabilities, TextInputClientSnapshot, TextInputFocusGeneration,
    TextInputHostCommand, TextInputKeyDisposition, TextInputOperation, TextInputOptions,
    TextInputSecurityPolicy, TextInputSerial, TextInputSessionId, TextRevision,
    WebTextInputApiSupport,
};
use thiserror::Error;

/// Active text-input focus transaction tracked by the runtime host.
#[derive(Clone, Debug, PartialEq)]
pub struct FocusedTextInputSession {
    session: TextInputSessionId,
    target: InteractionTarget,
    generation: TextInputFocusGeneration,
    revision: TextRevision,
    options: TextInputOptions,
    capabilities: TextInputCapabilities,
    security: TextInputSecurityPolicy,
    last_serial: Option<TextInputSerial>,
    composition_active: bool,
}

/// Runtime-host state for one presentation focus owner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextInputDispatchState {
    active: Option<FocusedTextInputSession>,
    focus_generation: TextInputFocusGeneration,
}

/// Host commands emitted while opening or closing a text-input focus transaction.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextInputFocusTransaction {
    generation: TextInputFocusGeneration,
    ended_session: Option<TextInputSessionId>,
    commands: Vec<TextInputHostCommand>,
}

/// Validated routed raw input plus key-disposition metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct TextInputDispatchOutput {
    raw: RawInputEvent,
    key_disposition: TextInputKeyDisposition,
}

/// Runtime-host rejection reason for platform IME callbacks.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TextInputDispatchError {
    #[error("no active text-input session")]
    NoActiveSession,
    #[error("text-input session mismatch: active {active:?}, incoming {incoming:?}")]
    SessionMismatch {
        active: TextInputSessionId,
        incoming: TextInputSessionId,
    },
    #[error("text-input focus generation mismatch: active {active:?}, incoming {incoming:?}")]
    FocusGenerationMismatch {
        active: TextInputFocusGeneration,
        incoming: TextInputFocusGeneration,
    },
    #[error("text-input target mismatch: active {active:?}, incoming {incoming:?}")]
    TargetMismatch {
        active: InteractionTarget,
        incoming: InteractionTarget,
    },
    #[error("stale text-input serial: last {last:?}, incoming {incoming:?}")]
    StaleSerial {
        last: TextInputSerial,
        incoming: TextInputSerial,
    },
    #[error("secure text input forbids clipboard command {0:?}")]
    SecureClipboardCommand(TextEditCommand),
    #[error("Web text input requires EditContext; hidden textarea fallback is prohibited")]
    WebEditContextUnavailable,
}

impl TextInputDispatchState {
    pub const fn active(&self) -> Option<&FocusedTextInputSession> {
        self.active.as_ref()
    }

    pub const fn focus_generation(&self) -> TextInputFocusGeneration {
        self.focus_generation
    }

    /// Activates a platform text-input session from a `TextField` snapshot.
    ///
    /// Secure fields are redacted before the host command leaves runtime-host
    /// dispatch, while the active state still remembers that incoming batches
    /// must be hashed/replayed as sensitive text.
    pub fn activate(&mut self, snapshot: &TextInputClientSnapshot) -> TextInputFocusTransaction {
        self.activate_with_capabilities(snapshot, TextInputCapabilities::all_supported())
    }

    /// Activates a platform text-input session with adapter-reported
    /// capabilities.
    pub fn activate_with_capabilities(
        &mut self,
        snapshot: &TextInputClientSnapshot,
        capabilities: TextInputCapabilities,
    ) -> TextInputFocusTransaction {
        self.focus_generation = self.focus_generation.next();
        let generation = self.focus_generation;
        let security = TextInputSecurityPolicy::from_options(snapshot.options());
        let capabilities = capabilities.narrow_for_security(security);
        let session = snapshot.session();
        let target = snapshot.target().clone();
        let revision = snapshot.revision();
        let options = snapshot.options().clone();
        let command_snapshot = security.redact_snapshot(snapshot);
        self.active = Some(FocusedTextInputSession {
            session,
            target: target.clone(),
            generation,
            revision,
            options,
            capabilities,
            security,
            last_serial: None,
            composition_active: false,
        });
        TextInputFocusTransaction::new(generation).with_command(TextInputHostCommand::Activate {
            session,
            target,
            capabilities,
            snapshot: Box::new(command_snapshot),
        })
    }

    /// Emits a redacted or plain platform snapshot update for the active session.
    pub fn update_snapshot(
        &self,
        snapshot: &TextInputClientSnapshot,
    ) -> Result<TextInputHostCommand, TextInputDispatchError> {
        let active = self
            .active
            .as_ref()
            .ok_or(TextInputDispatchError::NoActiveSession)?;
        if active.session != snapshot.session() {
            return Err(TextInputDispatchError::SessionMismatch {
                active: active.session,
                incoming: snapshot.session(),
            });
        }
        if &active.target != snapshot.target() {
            return Err(TextInputDispatchError::TargetMismatch {
                active: active.target.clone(),
                incoming: snapshot.target().clone(),
            });
        }
        Ok(TextInputHostCommand::Update(Box::new(
            active.security.redact_snapshot(snapshot),
        )))
    }

    /// Emits a redacted or plain geometry update for the active session.
    pub fn update_geometry(
        &self,
        geometry: &arcweft_presentation::text_input::TextInputGeometrySnapshot,
    ) -> Result<TextInputHostCommand, TextInputDispatchError> {
        let active = self
            .active
            .as_ref()
            .ok_or(TextInputDispatchError::NoActiveSession)?;
        if active.session != geometry.session() {
            return Err(TextInputDispatchError::SessionMismatch {
                active: active.session,
                incoming: geometry.session(),
            });
        }
        Ok(TextInputHostCommand::UpdateGeometry(Box::new(
            active.security.redact_geometry(geometry),
        )))
    }

    /// Validates one platform callback and converts it to routed raw text input.
    pub fn dispatch_platform_event(
        &mut self,
        epoch: InputEpoch,
        event: PlatformTextInputEvent,
        key_disposition: TextInputKeyDisposition,
    ) -> Result<TextInputDispatchOutput, TextInputDispatchError> {
        let context = event.context().clone();
        let active = self.validate_context(&context)?;
        let input = event.into_text_input(active.security.input_privacy());
        reject_secure_clipboard(active, &input)?;
        active.last_serial = Some(context.serial());
        active.composition_active = composition_active_after(active.composition_active, &input);
        Ok(TextInputDispatchOutput {
            raw: RawInputEvent::new(epoch, RawInputKind::Text(input)),
            key_disposition,
        })
    }

    /// Ends the active text-input session and emits host-side composition policy.
    pub fn blur(&mut self, policy: TextInputBlurPolicy) -> TextInputFocusTransaction {
        let Some(active) = self.active.take() else {
            return TextInputFocusTransaction::new(self.focus_generation);
        };
        let mut transaction =
            TextInputFocusTransaction::new(active.generation).ending_session(active.session);
        if active.composition_active {
            transaction = match policy {
                TextInputBlurPolicy::CommitComposition => {
                    transaction.with_command(TextInputHostCommand::CommitComposition {
                        session: active.session,
                    })
                }
                TextInputBlurPolicy::CancelComposition => {
                    transaction.with_command(TextInputHostCommand::CancelComposition {
                        session: active.session,
                    })
                }
                TextInputBlurPolicy::PlatformDefault => transaction,
            };
        }
        transaction = transaction.with_command(TextInputHostCommand::Deactivate {
            session: active.session,
        });
        self.focus_generation = self.focus_generation.next();
        transaction
    }

    /// Returns true when a key shortcut should continue through normal routing.
    pub fn shortcuts_allowed(&self, disposition: TextInputKeyDisposition) -> bool {
        !disposition.shortcuts_suppressed()
            && self.active.as_ref().is_none_or(|active| {
                !active.composition_active && active.options.shortcuts_enabled()
            })
    }

    fn validate_context(
        &mut self,
        context: &PlatformTextInputContext,
    ) -> Result<&mut FocusedTextInputSession, TextInputDispatchError> {
        let active = self
            .active
            .as_mut()
            .ok_or(TextInputDispatchError::NoActiveSession)?;
        if active.session != context.session() {
            return Err(TextInputDispatchError::SessionMismatch {
                active: active.session,
                incoming: context.session(),
            });
        }
        if active.generation != context.generation() {
            return Err(TextInputDispatchError::FocusGenerationMismatch {
                active: active.generation,
                incoming: context.generation(),
            });
        }
        if &active.target != context.target() {
            return Err(TextInputDispatchError::TargetMismatch {
                active: active.target.clone(),
                incoming: context.target().clone(),
            });
        }
        if let Some(last) = active.last_serial
            && context.serial().0 <= last.0
        {
            return Err(TextInputDispatchError::StaleSerial {
                last,
                incoming: context.serial(),
            });
        }
        Ok(active)
    }
}

impl FocusedTextInputSession {
    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn generation(&self) -> TextInputFocusGeneration {
        self.generation
    }

    pub const fn revision(&self) -> TextRevision {
        self.revision
    }

    pub const fn options(&self) -> &TextInputOptions {
        &self.options
    }

    pub const fn security(&self) -> TextInputSecurityPolicy {
        self.security
    }

    pub const fn capabilities(&self) -> TextInputCapabilities {
        self.capabilities
    }

    pub const fn composition_active(&self) -> bool {
        self.composition_active
    }
}

impl TextInputFocusTransaction {
    pub fn new(generation: TextInputFocusGeneration) -> Self {
        Self {
            generation,
            ended_session: None,
            commands: Vec::new(),
        }
    }

    #[must_use]
    pub fn ending_session(mut self, session: TextInputSessionId) -> Self {
        self.ended_session = Some(session);
        self
    }

    #[must_use]
    pub fn with_command(mut self, command: TextInputHostCommand) -> Self {
        self.commands.push(command);
        self
    }

    pub const fn generation(&self) -> TextInputFocusGeneration {
        self.generation
    }

    pub const fn ended_session(&self) -> Option<TextInputSessionId> {
        self.ended_session
    }

    pub fn commands(&self) -> &[TextInputHostCommand] {
        &self.commands
    }

    pub fn into_commands(self) -> Vec<TextInputHostCommand> {
        self.commands
    }
}

impl TextInputDispatchOutput {
    pub const fn raw(&self) -> &RawInputEvent {
        &self.raw
    }

    pub const fn key_disposition(&self) -> TextInputKeyDisposition {
        self.key_disposition
    }

    pub fn into_raw(self) -> RawInputEvent {
        self.raw
    }
}

/// Host-facing helper used by keyboard paths before routing shortcuts.
pub const fn dispatch_event_suppresses_shortcuts(disposition: TextInputKeyDisposition) -> bool {
    disposition.shortcuts_suppressed()
}

/// Web capability gate: unsupported browsers must report an explicit missing
/// `EditContext` path rather than falling back to hidden DOM textareas.
pub const fn web_edit_context_capabilities(
    support: WebTextInputApiSupport,
) -> Result<TextInputCapabilities, TextInputDispatchError> {
    match TextInputCapabilities::for_web_support(support) {
        Some(capabilities) => Ok(capabilities),
        None => Err(TextInputDispatchError::WebEditContextUnavailable),
    }
}

fn composition_active_after(previous: bool, input: &TextInput) -> bool {
    input
        .operations()
        .iter()
        .fold(previous, |active, operation| match operation {
            TextInputOperation::StartComposition | TextInputOperation::SetComposition(_) => true,
            TextInputOperation::Commit(_)
            | TextInputOperation::EndComposition { .. }
            | TextInputOperation::Command(TextEditCommand::Cancel | TextEditCommand::Submit) => {
                false
            }
            TextInputOperation::DeleteSurrounding { .. }
            | TextInputOperation::SetSelection(_)
            | TextInputOperation::Command(_) => active,
        })
}

fn reject_secure_clipboard(
    active: &FocusedTextInputSession,
    input: &TextInput,
) -> Result<(), TextInputDispatchError> {
    if active.security.allows_clipboard() {
        return Ok(());
    }
    input
        .operations()
        .iter()
        .find_map(|operation| match operation {
            TextInputOperation::Command(
                command @ (TextEditCommand::Copy | TextEditCommand::Cut | TextEditCommand::Paste),
            ) => Some(*command),
            TextInputOperation::StartComposition
            | TextInputOperation::SetComposition(_)
            | TextInputOperation::Commit(_)
            | TextInputOperation::EndComposition { .. }
            | TextInputOperation::DeleteSurrounding { .. }
            | TextInputOperation::SetSelection(_)
            | TextInputOperation::Command(_) => None,
        })
        .map_or(Ok(()), |command| {
            Err(TextInputDispatchError::SecureClipboardCommand(command))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::text_input::{
        PlatformTextSelection, TextByteOffset, TextCompositionUpdate, TextInputAdapterKind,
        TextInputOperation, TextInputPrivacy, TextRange, TextSelectionAffinity,
    };
    use arcweft_view::text_field::{TextEditState, TextFieldEditPolicy};

    fn target(name: &str) -> InteractionTarget {
        InteractionTarget::new(PublicId::try_new(format!("target.{name}")).unwrap())
    }

    fn snapshot(
        session: TextInputSessionId,
        target: InteractionTarget,
        options: TextInputOptions,
    ) -> TextInputClientSnapshot {
        TextInputClientSnapshot::new(
            session,
            target,
            TextRevision(0),
            "",
            TextByteOffset(0),
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            HitRect::new(0.0, 0.0, 200.0, 24.0),
            HitRect::new(0.0, 0.0, 1.0, 24.0),
            options,
        )
    }

    fn context(
        target: InteractionTarget,
        session: TextInputSessionId,
        generation: TextInputFocusGeneration,
        serial: u64,
    ) -> PlatformTextInputContext {
        PlatformTextInputContext::new(
            TextInputAdapterKind::MacosTextInputClient,
            session,
            generation,
            target,
            TextInputSerial(serial),
        )
    }

    #[test]
    fn japanese_composition_trace_keeps_preedit_visual_until_commit() {
        let target = target("textfield.nihongo");
        let session = TextInputSessionId(9);
        let mut dispatcher = TextInputDispatchState::default();
        let activation = dispatcher.activate(&snapshot(
            session,
            target.clone(),
            TextInputOptions::default(),
        ));
        let generation = activation.generation();
        let mut editor = TextEditState::new("");
        editor.bind_session(session);

        let update = PlatformTextInputEvent::SetComposition {
            context: context(target.clone(), session, generation, 1),
            update: TextCompositionUpdate::new(
                "にほんご",
                TextRange::new(TextByteOffset(0), TextByteOffset(12)),
            ),
        };
        let raw = dispatcher
            .dispatch_platform_event(InputEpoch(1), update, TextInputKeyDisposition::ImeConsumed)
            .unwrap()
            .into_raw();
        let RawInputKind::Text(input) = raw.kind() else {
            panic!("text input routed");
        };
        editor
            .apply_text_input_with_policy(input, TextFieldEditPolicy::default())
            .unwrap();
        assert_eq!(editor.document(), "");
        assert_eq!(
            editor.visual_source(),
            arcweft_view::ViewTextSource::plain("にほんご")
        );

        let commit = PlatformTextInputEvent::Commit {
            context: context(target, session, generation, 2),
            commit: TextCommit::new("日本語"),
        };
        let raw = dispatcher
            .dispatch_platform_event(InputEpoch(2), commit, TextInputKeyDisposition::ImeConsumed)
            .unwrap()
            .into_raw();
        let RawInputKind::Text(input) = raw.kind() else {
            panic!("text input routed");
        };
        let outcome = editor
            .apply_text_input_with_policy(input, TextFieldEditPolicy::default())
            .unwrap();

        assert!(outcome.changed());
        assert_eq!(editor.document(), "日本語");
    }

    #[test]
    fn stale_serial_and_focus_generation_are_rejected() {
        let target = target("textfield.stale");
        let session = TextInputSessionId(3);
        let mut dispatcher = TextInputDispatchState::default();
        let generation = dispatcher
            .activate(&snapshot(
                session,
                target.clone(),
                TextInputOptions::default(),
            ))
            .generation();
        let first = PlatformTextInputEvent::Commit {
            context: context(target.clone(), session, generation, 4),
            commit: TextCommit::new("a"),
        };
        dispatcher
            .dispatch_platform_event(InputEpoch(1), first, TextInputKeyDisposition::ImeConsumed)
            .unwrap();
        let stale_serial = PlatformTextInputEvent::Commit {
            context: context(target.clone(), session, generation, 4),
            commit: TextCommit::new("b"),
        };
        assert!(matches!(
            dispatcher.dispatch_platform_event(
                InputEpoch(2),
                stale_serial,
                TextInputKeyDisposition::ImeConsumed
            ),
            Err(TextInputDispatchError::StaleSerial { .. })
        ));

        let stale_generation = PlatformTextInputEvent::Commit {
            context: context(target, session, generation.next(), 5),
            commit: TextCommit::new("c"),
        };
        assert!(matches!(
            dispatcher.dispatch_platform_event(
                InputEpoch(3),
                stale_generation,
                TextInputKeyDisposition::ImeConsumed
            ),
            Err(TextInputDispatchError::FocusGenerationMismatch { .. })
        ));
    }

    #[test]
    fn secure_activation_redacts_snapshot_and_marks_batches_sensitive() {
        let target = target("secure.password");
        let session = TextInputSessionId(44);
        let mut dispatcher = TextInputDispatchState::default();
        let activation = dispatcher.activate(&snapshot(
            session,
            target.clone(),
            TextInputOptions::default().secure(true),
        ));
        let TextInputHostCommand::Activate { snapshot, .. } = &activation.commands()[0] else {
            panic!("activate command emitted");
        };
        assert!(snapshot.surrounding_text().is_empty());
        let event = PlatformTextInputEvent::Commit {
            context: context(target, session, activation.generation(), 1),
            commit: TextCommit::new("secret"),
        };
        let raw = dispatcher
            .dispatch_platform_event(InputEpoch(10), event, TextInputKeyDisposition::ImeConsumed)
            .unwrap()
            .into_raw();
        let RawInputKind::Text(input) = raw.kind() else {
            panic!("text input routed");
        };
        assert_eq!(input.privacy(), TextInputPrivacy::Sensitive);
    }

    #[test]
    fn secure_clipboard_and_ime_consumed_shortcuts_are_blocked() {
        let target = target("secure.clipboard");
        let session = TextInputSessionId(77);
        let mut dispatcher = TextInputDispatchState::default();
        let generation = dispatcher
            .activate(&snapshot(
                session,
                target.clone(),
                TextInputOptions::default().secure(true),
            ))
            .generation();
        let copy = PlatformTextInputEvent::Command {
            context: context(target, session, generation, 1),
            command: TextEditCommand::Copy,
        };
        assert_eq!(
            dispatcher.dispatch_platform_event(
                InputEpoch(1),
                copy,
                TextInputKeyDisposition::ImeConsumed
            ),
            Err(TextInputDispatchError::SecureClipboardCommand(
                TextEditCommand::Copy
            ))
        );
        assert!(!dispatcher.shortcuts_allowed(TextInputKeyDisposition::ImeConsumed));
    }

    #[test]
    fn blur_policy_emits_commit_cancel_or_platform_default_commands() {
        let target = target("textfield.blur");
        let session = TextInputSessionId(12);
        let mut dispatcher = TextInputDispatchState::default();
        let generation = dispatcher
            .activate(&snapshot(
                session,
                target.clone(),
                TextInputOptions::default(),
            ))
            .generation();
        dispatcher
            .dispatch_platform_event(
                InputEpoch(1),
                PlatformTextInputEvent::StartComposition(context(target, session, generation, 1)),
                TextInputKeyDisposition::ImeConsumed,
            )
            .unwrap();

        let transaction = dispatcher.blur(TextInputBlurPolicy::CommitComposition);

        assert!(matches!(
            transaction.commands(),
            [
                TextInputHostCommand::CommitComposition { .. },
                TextInputHostCommand::Deactivate { .. }
            ]
        ));
    }

    #[test]
    fn web_without_editcontext_reports_unsupported_without_hidden_textarea() {
        assert_eq!(
            web_edit_context_capabilities(WebTextInputApiSupport::UnsupportedNoFallback),
            Err(TextInputDispatchError::WebEditContextUnavailable)
        );
        assert!(web_edit_context_capabilities(WebTextInputApiSupport::EditContext).is_ok());
    }

    #[test]
    fn platform_fixture_events_map_to_expected_operations() {
        let target = target("textfield.fixtures");
        let session = TextInputSessionId(20);
        let generation = TextInputFocusGeneration(1);
        let contexts = [
            TextInputAdapterKind::WindowsTsf,
            TextInputAdapterKind::MacosTextInputClient,
            TextInputAdapterKind::WaylandTextInputV3,
            TextInputAdapterKind::AndroidInputConnection,
            TextInputAdapterKind::IosTextInput,
            TextInputAdapterKind::WebEditContext,
        ];
        for (index, adapter) in contexts.into_iter().enumerate() {
            let context = PlatformTextInputContext::new(
                adapter,
                session,
                generation,
                target.clone(),
                TextInputSerial(index as u64 + 1),
            );
            let input = PlatformTextInputEvent::SetSelection {
                context,
                selection: PlatformTextSelection::new(
                    TextRange::new(TextByteOffset(0), TextByteOffset(0)),
                    TextSelectionAffinity::Downstream,
                ),
            }
            .into_text_input(TextInputPrivacy::Plain);
            assert!(matches!(
                input.operations(),
                [TextInputOperation::SetSelection(_)]
            ));
        }
    }
}
