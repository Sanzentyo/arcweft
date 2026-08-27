//! Typed Sans-I/O commands for dialogue-owned stage and voice resources.

use crate::effect::RuntimeDropPolicy;
use crate::line_task::RuntimeLineHandleScope;
use crate::runtime_id::{DialogueActivationId, RuntimeLineHandleToken};
use crate::time::LogicalDuration;
use arcweft_character::id::{CharacterId, CharacterLookId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Globally exact identity of one dialogue-owned presentation command.
///
/// The activation is part of the identity, rather than a sibling command
/// field, so a stale outcome cannot match a reused per-activation sequence.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeLineCommandId {
    activation: DialogueActivationId,
    sequence: u64,
}

impl RuntimeLineCommandId {
    #[must_use]
    pub const fn new(activation: DialogueActivationId, sequence: u64) -> Self {
        Self {
            activation,
            sequence,
        }
    }

    #[must_use]
    pub const fn activation(&self) -> &DialogueActivationId {
        &self.activation
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeStageCommand {
    AcquireActor {
        command: RuntimeLineCommandId,
        actor: RuntimeLineHandleToken,
        character: CharacterId,
        scope: RuntimeLineHandleScope,
    },
    SetCharacterLook {
        command: RuntimeLineCommandId,
        cue: RuntimeLineHandleToken,
        actor: RuntimeLineHandleToken,
        character: CharacterId,
        look: CharacterLookId,
        crossfade: LogicalDuration,
    },
    ReleaseActor {
        command: RuntimeLineCommandId,
        actor: RuntimeLineHandleToken,
    },
    CancelCue {
        command: RuntimeLineCommandId,
        cue: RuntimeLineHandleToken,
    },
}

impl RuntimeStageCommand {
    #[must_use]
    pub const fn command(&self) -> &RuntimeLineCommandId {
        match self {
            Self::AcquireActor { command, .. }
            | Self::SetCharacterLook { command, .. }
            | Self::ReleaseActor { command, .. }
            | Self::CancelCue { command, .. } => command,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStageRejectCode {
    Unsupported,
    InvalidCharacter,
    InvalidLook,
    Busy,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RuntimeStageCommandOutcome {
    Acquired {
        command: RuntimeLineCommandId,
        actor: RuntimeLineHandleToken,
    },
    Accepted {
        command: RuntimeLineCommandId,
        cue: RuntimeLineHandleToken,
    },
    Completed {
        command: RuntimeLineCommandId,
        cue: RuntimeLineHandleToken,
    },
    Cancelled {
        command: RuntimeLineCommandId,
        cue: RuntimeLineHandleToken,
    },
    ReleasedActor {
        command: RuntimeLineCommandId,
        actor: RuntimeLineHandleToken,
    },
    Rejected {
        command: RuntimeLineCommandId,
        code: RuntimeStageRejectCode,
    },
}

impl RuntimeStageCommandOutcome {
    #[must_use]
    pub const fn command(&self) -> &RuntimeLineCommandId {
        match self {
            Self::Acquired { command, .. }
            | Self::Accepted { command, .. }
            | Self::Completed { command, .. }
            | Self::Cancelled { command, .. }
            | Self::ReleasedActor { command, .. }
            | Self::Rejected { command, .. } => command,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeVoiceStartTicket(String);

impl RuntimeVoiceStartTicket {
    pub fn try_new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.is_empty()).then_some(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeVoiceSessionId(String);

impl RuntimeVoiceSessionId {
    pub fn try_new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.is_empty()).then_some(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeVoiceFailure {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeDialogueVoiceState {
    Absent,
    Lazy(RuntimeVoiceStartTicket),
    Ready(RuntimeVoiceSessionId),
    Failed(RuntimeVoiceFailure),
    Completed(RuntimeVoiceSessionId),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeVoiceCommand {
    StartDialogueVoice {
        command: RuntimeLineCommandId,
        ticket: RuntimeVoiceStartTicket,
    },
    ReleaseDialogueVoice {
        command: RuntimeLineCommandId,
        handle: RuntimeLineHandleToken,
        policy: RuntimeDropPolicy,
    },
}

impl RuntimeVoiceCommand {
    #[must_use]
    pub const fn command(&self) -> &RuntimeLineCommandId {
        match self {
            Self::StartDialogueVoice { command, .. }
            | Self::ReleaseDialogueVoice { command, .. } => command,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RuntimeVoiceCommandOutcome {
    Started {
        command: RuntimeLineCommandId,
        session: RuntimeVoiceSessionId,
    },
    Released {
        command: RuntimeLineCommandId,
        handle: RuntimeLineHandleToken,
    },
    Rejected {
        command: RuntimeLineCommandId,
        failure: RuntimeVoiceFailure,
    },
}

impl RuntimeVoiceCommandOutcome {
    #[must_use]
    pub const fn command(&self) -> &RuntimeLineCommandId {
        match self {
            Self::Started { command, .. }
            | Self::Released { command, .. }
            | Self::Rejected { command, .. } => command,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "family", content = "command", rename_all = "snake_case")]
pub enum RuntimeLineHostCommand {
    Stage(RuntimeStageCommand),
    Voice(RuntimeVoiceCommand),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "family", content = "outcome", rename_all = "snake_case")]
pub enum RuntimeLineHostOutcome {
    Stage(RuntimeStageCommandOutcome),
    Voice(RuntimeVoiceCommandOutcome),
}

impl RuntimeLineHostCommand {
    #[must_use]
    pub const fn command(&self) -> &RuntimeLineCommandId {
        match self {
            Self::Stage(command) => command.command(),
            Self::Voice(command) => command.command(),
        }
    }
}

impl RuntimeLineHostOutcome {
    #[must_use]
    pub const fn command(&self) -> &RuntimeLineCommandId {
        match self {
            Self::Stage(outcome) => outcome.command(),
            Self::Voice(outcome) => outcome.command(),
        }
    }
}

/// Activation-scoped deterministic command queue. The dialogue state copies
/// its next sequence back after each transactional operation.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeCommandQueue {
    activation: DialogueActivationId,
    start_sequence: u64,
    next_sequence: u64,
    commands: Vec<RuntimeLineHostCommand>,
}

impl RuntimeCommandQueue {
    #[must_use]
    pub const fn new(activation: DialogueActivationId, next_sequence: u64) -> Self {
        Self {
            activation,
            start_sequence: next_sequence,
            next_sequence,
            commands: Vec::new(),
        }
    }

    fn allocate_sequence(&mut self) -> Result<u64, RuntimeCommandQueueError> {
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(RuntimeCommandQueueError::SequenceOverflow)?;
        Ok(sequence)
    }

    pub fn push_acquire_actor(
        &mut self,
        actor: RuntimeLineHandleToken,
        character: CharacterId,
        scope: RuntimeLineHandleScope,
    ) -> Result<RuntimeLineCommandId, RuntimeCommandQueueError> {
        let command = RuntimeLineCommandId::new(self.activation.clone(), self.allocate_sequence()?);
        self.commands.push(RuntimeLineHostCommand::Stage(
            RuntimeStageCommand::AcquireActor {
                command: command.clone(),
                actor,
                character,
                scope,
            },
        ));
        Ok(command)
    }

    pub fn push_set_character_look(
        &mut self,
        cue: RuntimeLineHandleToken,
        actor: RuntimeLineHandleToken,
        character: CharacterId,
        look: CharacterLookId,
        crossfade: LogicalDuration,
    ) -> Result<RuntimeLineCommandId, RuntimeCommandQueueError> {
        let command = RuntimeLineCommandId::new(self.activation.clone(), self.allocate_sequence()?);
        self.commands.push(RuntimeLineHostCommand::Stage(
            RuntimeStageCommand::SetCharacterLook {
                command: command.clone(),
                cue,
                actor,
                character,
                look,
                crossfade,
            },
        ));
        Ok(command)
    }

    pub fn push_start_voice(
        &mut self,
        ticket: RuntimeVoiceStartTicket,
    ) -> Result<RuntimeLineCommandId, RuntimeCommandQueueError> {
        let command = RuntimeLineCommandId::new(self.activation.clone(), self.allocate_sequence()?);
        self.commands.push(RuntimeLineHostCommand::Voice(
            RuntimeVoiceCommand::StartDialogueVoice {
                command: command.clone(),
                ticket,
            },
        ));
        Ok(command)
    }

    pub fn push_release_actor(
        &mut self,
        actor: RuntimeLineHandleToken,
    ) -> Result<RuntimeLineCommandId, RuntimeCommandQueueError> {
        let command = RuntimeLineCommandId::new(self.activation.clone(), self.allocate_sequence()?);
        self.commands.push(RuntimeLineHostCommand::Stage(
            RuntimeStageCommand::ReleaseActor {
                command: command.clone(),
                actor,
            },
        ));
        Ok(command)
    }

    pub fn push_cancel_cue(
        &mut self,
        cue: RuntimeLineHandleToken,
    ) -> Result<RuntimeLineCommandId, RuntimeCommandQueueError> {
        let command = RuntimeLineCommandId::new(self.activation.clone(), self.allocate_sequence()?);
        self.commands.push(RuntimeLineHostCommand::Stage(
            RuntimeStageCommand::CancelCue {
                command: command.clone(),
                cue,
            },
        ));
        Ok(command)
    }

    pub fn push_release_voice(
        &mut self,
        handle: RuntimeLineHandleToken,
        policy: RuntimeDropPolicy,
    ) -> Result<RuntimeLineCommandId, RuntimeCommandQueueError> {
        let command = RuntimeLineCommandId::new(self.activation.clone(), self.allocate_sequence()?);
        self.commands.push(RuntimeLineHostCommand::Voice(
            RuntimeVoiceCommand::ReleaseDialogueVoice {
                command: command.clone(),
                handle,
                policy,
            },
        ));
        Ok(command)
    }

    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    #[must_use]
    pub const fn activation(&self) -> &DialogueActivationId {
        &self.activation
    }

    #[must_use]
    pub const fn start_sequence(&self) -> u64 {
        self.start_sequence
    }

    #[must_use]
    pub fn into_commands(self) -> Vec<RuntimeLineHostCommand> {
        self.commands
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCommandQueueError {
    #[error("dialogue command sequence overflowed")]
    SequenceOverflow,
}
