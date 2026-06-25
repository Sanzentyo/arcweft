use super::schema::{AwbcBinaryOp, AwbcTrapCode, AwbcUnaryOp};
use crate::value::{RuntimeBinaryOp, RuntimeUnaryOp};

impl From<RuntimeUnaryOp> for AwbcUnaryOp {
    fn from(value: RuntimeUnaryOp) -> Self {
        match value {
            RuntimeUnaryOp::Not => Self::Not,
            RuntimeUnaryOp::Neg => Self::Neg,
        }
    }
}

impl From<RuntimeBinaryOp> for AwbcBinaryOp {
    fn from(value: RuntimeBinaryOp) -> Self {
        match value {
            RuntimeBinaryOp::Eq => Self::Eq,
            RuntimeBinaryOp::Ne => Self::Ne,
            RuntimeBinaryOp::Lt => Self::Lt,
            RuntimeBinaryOp::Le => Self::Le,
            RuntimeBinaryOp::Gt => Self::Gt,
            RuntimeBinaryOp::Ge => Self::Ge,
            RuntimeBinaryOp::Add => Self::Add,
            RuntimeBinaryOp::Sub => Self::Sub,
            RuntimeBinaryOp::Mul => Self::Mul,
            RuntimeBinaryOp::Div => Self::Div,
            RuntimeBinaryOp::And => Self::And,
            RuntimeBinaryOp::Or => Self::Or,
        }
    }
}

impl AwbcTrapCode {
    pub(crate) const fn stable_name(self) -> &'static str {
        match self {
            Self::TypeMismatch => "type_mismatch",
            Self::UninitializedRegister => "uninitialized_register",
            Self::InvalidIndex => "invalid_index",
            Self::DivisionByZero => "division_by_zero",
            Self::PatternMismatch => "pattern_mismatch",
            Self::MissingDynamicTarget => "missing_dynamic_target",
            Self::HostAbiMismatch => "host_abi_mismatch",
            Self::CapabilityDenied => "capability_denied",
            Self::ExplicitPanic => "explicit_panic",
            Self::InternalInvariant => "internal_invariant",
        }
    }
}
