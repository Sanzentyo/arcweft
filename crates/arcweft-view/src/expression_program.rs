//! Retained expression programs use the ordinary executable value ABI.

use arcweft_id::{RuntimeSemanticTypeId, runtime_program::RuntimePureProgramId};
use serde::{Deserialize, Serialize};

use crate::ViewParameterInput;

/// One expression's exact program identity, canonical free-input order and result type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewExpressionProgram {
    pub program: RuntimePureProgramId,
    pub inputs: Box<[ViewParameterInput]>,
    pub result_type: RuntimeSemanticTypeId,
}

/// A declared constant or an ordinary pure expression producing a property.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ViewExpressionValue<T> {
    Constant { value: T },
    Program { program: ViewExpressionProgram },
}

impl<T> ViewExpressionValue<T> {
    pub const fn constant(value: T) -> Self {
        Self::Constant { value }
    }
    pub const fn program(&self) -> Option<&ViewExpressionProgram> {
        match self {
            Self::Constant { .. } => None,
            Self::Program { program } => Some(program),
        }
    }
}
