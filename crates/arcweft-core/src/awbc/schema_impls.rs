use super::schema::{AwbcBinaryOp, AwbcSignedIntKind, AwbcUnaryOp, AwbcUnsignedIntKind};
use crate::value::{
    RuntimeBinaryOp, RuntimeSignedIntWidth, RuntimeUnaryOp, RuntimeUnsignedIntWidth,
};

impl From<AwbcSignedIntKind> for RuntimeSignedIntWidth {
    fn from(kind: AwbcSignedIntKind) -> Self {
        match kind {
            AwbcSignedIntKind::I8 => Self::I8,
            AwbcSignedIntKind::I16 => Self::I16,
            AwbcSignedIntKind::I32 => Self::I32,
            AwbcSignedIntKind::I64 => Self::I64,
            AwbcSignedIntKind::I128 => Self::I128,
            AwbcSignedIntKind::ISize => Self::ISize,
        }
    }
}

impl From<AwbcUnsignedIntKind> for RuntimeUnsignedIntWidth {
    fn from(kind: AwbcUnsignedIntKind) -> Self {
        match kind {
            AwbcUnsignedIntKind::U8 => Self::U8,
            AwbcUnsignedIntKind::U16 => Self::U16,
            AwbcUnsignedIntKind::U32 => Self::U32,
            AwbcUnsignedIntKind::U64 => Self::U64,
            AwbcUnsignedIntKind::U128 => Self::U128,
            AwbcUnsignedIntKind::USize => Self::USize,
        }
    }
}

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
