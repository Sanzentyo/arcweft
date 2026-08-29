use arcweft_core::time::LogicalDuration;

use super::{CallableEvaluatedEffect, CallableLogLevel, DropCallableId, OpenArgumentId, TypeKind};

/// Checked operand retained by one evaluated-effect application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedEvaluatedEffectOperand {
    source: crate::callable::CheckedCallExecutionSource,
    ty: TypeKind,
}

impl CheckedEvaluatedEffectOperand {
    pub(crate) const fn new(
        source: crate::callable::CheckedCallExecutionSource,
        ty: TypeKind,
    ) -> Self {
        Self { source, ty }
    }

    pub const fn source(&self) -> &crate::callable::CheckedCallExecutionSource {
        &self.source
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedEffectField {
    open_argument: OpenArgumentId,
    operand: CheckedEvaluatedEffectOperand,
}

impl CheckedEffectField {
    pub(crate) const fn new(
        open_argument: OpenArgumentId,
        operand: CheckedEvaluatedEffectOperand,
    ) -> Self {
        Self {
            open_argument,
            operand,
        }
    }

    pub const fn open_argument(&self) -> &OpenArgumentId {
        &self.open_argument
    }

    pub const fn operand(&self) -> &CheckedEvaluatedEffectOperand {
        &self.operand
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDropFade {
    Constant(LogicalDuration),
    Operand(CheckedDropFadeOperand),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDropPolicySource(CheckedEvaluatedEffectOperand);

impl CheckedDropPolicySource {
    pub(crate) fn try_new(operand: CheckedEvaluatedEffectOperand) -> Option<Self> {
        matches!(
            operand.source().raw(),
            crate::callable::CheckedCallArgumentSlotSource::Expression(_)
        )
        .then_some(Self(operand))
    }

    pub const fn operand(&self) -> &CheckedEvaluatedEffectOperand {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDropFadeOperand(CheckedEvaluatedEffectOperand);

impl CheckedDropFadeOperand {
    pub(crate) fn try_new(operand: CheckedEvaluatedEffectOperand) -> Option<Self> {
        (operand.ty() == &TypeKind::Duration).then_some(Self(operand))
    }

    pub const fn operand(&self) -> &CheckedEvaluatedEffectOperand {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExplicitDropPolicy {
    Cancel,
    Stop { fade: CheckedDropFade },
    Finish,
    Release,
    Detach,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDropInvocation {
    Drop,
    DropOptional,
    DropWithPolicy {
        source: CheckedDropPolicySource,
        policy: CheckedExplicitDropPolicy,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedEvaluatedEffectOperation {
    Log {
        level: CallableLogLevel,
        message: CheckedEvaluatedEffectOperand,
        fields: Box<[CheckedEffectField]>,
    },
    SignalWrite {
        target: CheckedEvaluatedEffectOperand,
        value: CheckedEvaluatedEffectOperand,
    },
    MetricWrite {
        target: CheckedEvaluatedEffectOperand,
        value: CheckedEvaluatedEffectOperand,
    },
    EmitEvent {
        event: CheckedEvaluatedEffectOperand,
        fields: Box<[CheckedEffectField]>,
    },
    Panic {
        message: CheckedEvaluatedEffectOperand,
    },
    Fail {
        message: CheckedEvaluatedEffectOperand,
    },
    Bail {
        message: CheckedEvaluatedEffectOperand,
    },
    Ensure {
        condition: CheckedEvaluatedEffectOperand,
        message: CheckedEvaluatedEffectOperand,
    },
    Drop {
        target: CheckedEvaluatedEffectOperand,
        invocation: CheckedDropInvocation,
    },
}

/// Complete checked meaning of one evaluated-effect call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedEvaluatedEffect {
    application: crate::callable::CheckedCallApplicationSite,
    operation: CheckedEvaluatedEffectOperation,
}

impl CheckedEvaluatedEffect {
    pub(crate) const fn new(
        application: crate::callable::CheckedCallApplicationSite,
        operation: CheckedEvaluatedEffectOperation,
    ) -> Self {
        Self {
            application,
            operation,
        }
    }

    pub const fn application(&self) -> &crate::callable::CheckedCallApplicationSite {
        &self.application
    }

    pub const fn operation(&self) -> &CheckedEvaluatedEffectOperation {
        &self.operation
    }

    pub const fn disposition(&self) -> CallableEvaluatedEffect {
        match &self.operation {
            CheckedEvaluatedEffectOperation::Log { level, .. } => {
                CallableEvaluatedEffect::Log(*level)
            }
            CheckedEvaluatedEffectOperation::SignalWrite { .. } => {
                CallableEvaluatedEffect::SignalWrite
            }
            CheckedEvaluatedEffectOperation::MetricWrite { .. } => {
                CallableEvaluatedEffect::MetricWrite
            }
            CheckedEvaluatedEffectOperation::EmitEvent { .. } => CallableEvaluatedEffect::EmitEvent,
            CheckedEvaluatedEffectOperation::Panic { .. } => CallableEvaluatedEffect::Panic,
            CheckedEvaluatedEffectOperation::Fail { .. } => CallableEvaluatedEffect::Fail,
            CheckedEvaluatedEffectOperation::Bail { .. } => CallableEvaluatedEffect::Bail,
            CheckedEvaluatedEffectOperation::Ensure { .. } => CallableEvaluatedEffect::Ensure,
            CheckedEvaluatedEffectOperation::Drop { invocation, .. } => {
                CallableEvaluatedEffect::Drop(match invocation {
                    CheckedDropInvocation::Drop => DropCallableId::Drop,
                    CheckedDropInvocation::DropOptional => DropCallableId::DropOptional,
                    CheckedDropInvocation::DropWithPolicy { .. } => DropCallableId::DropWithPolicy,
                })
            }
        }
    }
}
