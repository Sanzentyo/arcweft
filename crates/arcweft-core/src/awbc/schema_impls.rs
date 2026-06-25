use super::schema::{AwbcBinaryOp, AwbcUnaryOp};
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
