//! One effect payload determines its wire identity and complete static data.

use super::{AwbcAudioCommandId, AwbcConstantId, AwbcEffectKind, AwbcTypeId};
use arcweft_id::LifetimeRegistryKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AwbcEffectPayload {
    Wait(AwbcWaitDuration),
    Audio(AwbcAudioCommandId),
    Call(Vec<AwbcConstantId>),
    Log(Vec<AwbcConstantId>),
    SignalWrite(Vec<AwbcConstantId>),
    MetricWrite(Vec<AwbcConstantId>),
    EmitEvent(Vec<AwbcConstantId>),
    Out(Vec<AwbcConstantId>),
    Return(Vec<AwbcConstantId>),
    Goto(Vec<AwbcConstantId>),
    Panic(Vec<AwbcConstantId>),
    Fail(Vec<AwbcConstantId>),
    Bail(Vec<AwbcConstantId>),
    Ensure(Vec<AwbcConstantId>),
    Assert(Vec<AwbcConstantId>),
    Close(Vec<AwbcConstantId>),
    Select(Vec<AwbcConstantId>),
    Break(Vec<AwbcConstantId>),
    Continue(Vec<AwbcConstantId>),
    RegistryWrite {
        key: LifetimeRegistryKey,
        value_type: AwbcTypeId,
    },
}

/// One typed Duration source. Static packets (including Choice effects) have
/// no register context; executable effects consume their sole Duration input.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AwbcWaitDuration {
    Constant(AwbcConstantId),
    Input,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum AwbcWaitDurationError {
    #[error("AWBC wait effect is missing its Duration target")]
    Operands,
    #[error("AWBC wait target must evaluate to Duration")]
    Target,
}

impl AwbcWaitDuration {
    pub(crate) fn resolve(
        self,
        program: &super::AwbcProgram,
        inputs: &[crate::value::RuntimeValue],
    ) -> Result<crate::time::LogicalDuration, AwbcWaitDurationError> {
        match self {
            Self::Constant(id) if inputs.is_empty() => {
                let Some(super::AwbcConstant::DurationNanos(nanos)) =
                    program.constants.get(id.index())
                else {
                    return Err(AwbcWaitDurationError::Target);
                };
                Ok(crate::time::LogicalDuration::from_nanos(*nanos))
            }
            Self::Input if inputs.len() == 1 => {
                let crate::effect::RuntimeWaitTarget::Duration(duration) =
                    crate::effect::RuntimeWaitTarget::try_from(&inputs[0])
                        .map_err(|_| AwbcWaitDurationError::Target)?;
                Ok(duration)
            }
            Self::Constant(_) | Self::Input => Err(AwbcWaitDurationError::Operands),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("AWBC effect {kind:?} requires its typed payload")]
pub struct AwbcEffectPayloadError {
    pub kind: AwbcEffectKind,
}

impl TryFrom<(AwbcEffectKind, Vec<AwbcConstantId>)> for AwbcEffectPayload {
    type Error = AwbcEffectPayloadError;

    fn try_from((kind, args): (AwbcEffectKind, Vec<AwbcConstantId>)) -> Result<Self, Self::Error> {
        Ok(match kind {
            AwbcEffectKind::Wait => {
                let [constant] = args.as_slice() else {
                    return Err(AwbcEffectPayloadError { kind });
                };
                Self::Wait(AwbcWaitDuration::Constant(*constant))
            }
            AwbcEffectKind::Call => Self::Call(args),
            AwbcEffectKind::Log => Self::Log(args),
            AwbcEffectKind::SignalWrite => Self::SignalWrite(args),
            AwbcEffectKind::MetricWrite => Self::MetricWrite(args),
            AwbcEffectKind::EmitEvent => Self::EmitEvent(args),
            AwbcEffectKind::Out => Self::Out(args),
            AwbcEffectKind::Return => Self::Return(args),
            AwbcEffectKind::Goto => Self::Goto(args),
            AwbcEffectKind::Panic => Self::Panic(args),
            AwbcEffectKind::Fail => Self::Fail(args),
            AwbcEffectKind::Bail => Self::Bail(args),
            AwbcEffectKind::Ensure => Self::Ensure(args),
            AwbcEffectKind::Assert => Self::Assert(args),
            AwbcEffectKind::Close => Self::Close(args),
            AwbcEffectKind::Select => Self::Select(args),
            AwbcEffectKind::Break => Self::Break(args),
            AwbcEffectKind::Continue => Self::Continue(args),
            AwbcEffectKind::Audio | AwbcEffectKind::RegistryWrite => {
                return Err(AwbcEffectPayloadError { kind });
            }
        })
    }
}

impl AwbcEffectPayload {
    /// Static cleanup packets and context-free effect dispatch cannot retain
    /// the active execution owner required by a registry operation.
    pub(crate) fn require_context_free(
        &self,
    ) -> Result<(), crate::effect::RuntimeEffectMaterializeError> {
        match self {
            Self::RegistryWrite { .. } => {
                Err(crate::effect::RuntimeEffectMaterializeError::RegistryContextRequired)
            }
            Self::Wait(_)
            | Self::Audio(_)
            | Self::Call(_)
            | Self::Log(_)
            | Self::SignalWrite(_)
            | Self::MetricWrite(_)
            | Self::EmitEvent(_)
            | Self::Out(_)
            | Self::Return(_)
            | Self::Goto(_)
            | Self::Panic(_)
            | Self::Fail(_)
            | Self::Bail(_)
            | Self::Ensure(_)
            | Self::Assert(_)
            | Self::Close(_)
            | Self::Select(_)
            | Self::Break(_)
            | Self::Continue(_) => Ok(()),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> AwbcEffectKind {
        match self {
            Self::Wait(_) => AwbcEffectKind::Wait,
            Self::Audio(_) => AwbcEffectKind::Audio,
            Self::Call(_) => AwbcEffectKind::Call,
            Self::Log(_) => AwbcEffectKind::Log,
            Self::SignalWrite(_) => AwbcEffectKind::SignalWrite,
            Self::MetricWrite(_) => AwbcEffectKind::MetricWrite,
            Self::EmitEvent(_) => AwbcEffectKind::EmitEvent,
            Self::Out(_) => AwbcEffectKind::Out,
            Self::Return(_) => AwbcEffectKind::Return,
            Self::Goto(_) => AwbcEffectKind::Goto,
            Self::Panic(_) => AwbcEffectKind::Panic,
            Self::Fail(_) => AwbcEffectKind::Fail,
            Self::Bail(_) => AwbcEffectKind::Bail,
            Self::Ensure(_) => AwbcEffectKind::Ensure,
            Self::Assert(_) => AwbcEffectKind::Assert,
            Self::Close(_) => AwbcEffectKind::Close,
            Self::Select(_) => AwbcEffectKind::Select,
            Self::Break(_) => AwbcEffectKind::Break,
            Self::Continue(_) => AwbcEffectKind::Continue,
            Self::RegistryWrite { .. } => AwbcEffectKind::RegistryWrite,
        }
    }

    #[must_use]
    pub fn static_args(&self) -> &[AwbcConstantId] {
        match self {
            Self::Wait(AwbcWaitDuration::Constant(constant)) => std::slice::from_ref(constant),
            Self::Call(args)
            | Self::Log(args)
            | Self::SignalWrite(args)
            | Self::MetricWrite(args)
            | Self::EmitEvent(args)
            | Self::Out(args)
            | Self::Return(args)
            | Self::Goto(args)
            | Self::Panic(args)
            | Self::Fail(args)
            | Self::Bail(args)
            | Self::Ensure(args)
            | Self::Assert(args)
            | Self::Close(args)
            | Self::Select(args)
            | Self::Break(args)
            | Self::Continue(args) => args,
            Self::Wait(AwbcWaitDuration::Input) | Self::Audio(_) | Self::RegistryWrite { .. } => {
                &[]
            }
        }
    }
}
