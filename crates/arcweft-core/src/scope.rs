//! Accepted lexical namespace carried by executable scopes.

use arcweft_id::DeclarationName;
use serde::{Deserialize, Serialize};

/// Namespace contribution of one lexical scope. Static executable coordinates,
/// rather than this optional name, distinguish separate scopes with the same name.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeScopeIdentity {
    #[default]
    Anonymous,
    Named(DeclarationName),
}

impl RuntimeScopeIdentity {
    pub const fn name(&self) -> Option<&DeclarationName> {
        match self {
            Self::Anonymous => None,
            Self::Named(name) => Some(name),
        }
    }
}

/// Executable scopes authored into the admitted body and scopes scheduled by
/// control scaffolding are different owners. Namespace names do not select exits.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
pub enum RuntimeScopeFrameKind {
    EmittedLexical = 0,
    Control = 1,
}

/// A typed exit from the current body's lexical frame or one exact scheduled
/// frame. Consumers supply their actual contiguous active scope frames
/// innermost first; function/loop boundaries remain owned by those consumers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeScopeExitTarget<T> {
    EmittedLexical,
    Frame(T),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeScopeExit<T> {
    targets: Box<[T]>,
}
impl<T> RuntimeScopeExit<T> {
    pub fn targets(&self) -> &[T] {
        &self.targets
    }
    pub fn target(&self) -> &T {
        self.targets
            .last()
            .expect("an admitted scope exit always ends one exact target")
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeScopeExitError {
    #[error("scope exit has no active emitted lexical frame in the current body")]
    MissingEmittedLexical,
    #[error("scope exit does not name the active exact scope frame")]
    ScheduledTargetMismatch,
    #[error("native scheduled scope identity capacity exhausted")]
    IdentityCapacityExhausted,
}
impl<T: Copy + Eq> RuntimeScopeExitTarget<T> {
    pub fn resolve(
        self,
        frames: impl IntoIterator<Item = (T, RuntimeScopeFrameKind)>,
    ) -> Result<RuntimeScopeExit<T>, RuntimeScopeExitError> {
        let mut frames = frames.into_iter();
        match self {
            Self::EmittedLexical => {
                let mut targets = Vec::new();
                for (target, kind) in frames {
                    targets.push(target);
                    if kind == RuntimeScopeFrameKind::EmittedLexical {
                        return Ok(RuntimeScopeExit {
                            targets: targets.into_boxed_slice(),
                        });
                    }
                }
                Err(RuntimeScopeExitError::MissingEmittedLexical)
            }
            Self::Frame(expected) => {
                let Some((actual, _)) = frames.next() else {
                    return Err(RuntimeScopeExitError::ScheduledTargetMismatch);
                };
                if actual != expected {
                    return Err(RuntimeScopeExitError::ScheduledTargetMismatch);
                }
                Ok(RuntimeScopeExit {
                    targets: Box::new([actual]),
                })
            }
        }
    }
}

/// Exact native queue/frame coupling. Only the native driver allocates a token with its immutable frame role;
/// it never grants a source execution capability or crosses its issuing fiber.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeScheduledScopeToken {
    execution: crate::runtime_id::ExecutionInstanceId,
    fiber: crate::runtime_id::RuntimePersistentFiberId,
    ordinal: std::num::NonZeroU64,
    kind: RuntimeScopeFrameKind,
}
impl RuntimeScheduledScopeToken {
    pub(crate) const fn from_runtime(
        execution: crate::runtime_id::ExecutionInstanceId,
        fiber: crate::runtime_id::RuntimePersistentFiberId,
        ordinal: std::num::NonZeroU64,
        kind: RuntimeScopeFrameKind,
    ) -> Self {
        Self {
            execution,
            fiber,
            ordinal,
            kind,
        }
    }
    pub(crate) const fn belongs_to(
        self,
        execution: crate::runtime_id::ExecutionInstanceId,
        fiber: crate::runtime_id::RuntimePersistentFiberId,
    ) -> bool {
        self.execution.get().get() == execution.get().get() && self.fiber.get() == fiber.get()
    }
    pub(crate) const fn ordinal(self) -> std::num::NonZeroU64 {
        self.ordinal
    }
    pub const fn kind(self) -> RuntimeScopeFrameKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeScopeFrameOrigin {
    EmittedLexical,
    Scheduled(RuntimeScheduledScopeToken),
}
impl RuntimeScopeFrameOrigin {
    pub(crate) const fn token(self) -> Option<RuntimeScheduledScopeToken> {
        match self {
            Self::EmittedLexical => None,
            Self::Scheduled(token) => Some(token),
        }
    }
    pub(crate) const fn kind(self) -> RuntimeScopeFrameKind {
        match self {
            Self::EmittedLexical => RuntimeScopeFrameKind::EmittedLexical,
            Self::Scheduled(token) => token.kind(),
        }
    }
}
