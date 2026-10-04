//! Retained expression programs use the ordinary executable value ABI.

use arcweft_id::{RuntimeSemanticTypeId, runtime_program::RuntimePureProgramId};
use serde::{Deserialize, Serialize};

use crate::{ViewParameterCoordinate, ViewParameterInput};

/// One output of an accepted binding program, separate from parameter ordinals.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewLocalCoordinate {
    pub program: RuntimePureProgramId,
    pub output: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewExecutionInputSource {
    Parameter(ViewParameterCoordinate),
    Local(ViewLocalCoordinate),
}

/// Canonical free input address and exact semantic type.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewExecutionInput {
    pub source: ViewExecutionInputSource,
    pub value_type: RuntimeSemanticTypeId,
}

impl ViewExecutionInput {
    pub const fn value_type(self) -> RuntimeSemanticTypeId {
        self.value_type
    }
    pub const fn parameter(self) -> Option<ViewParameterCoordinate> {
        match self.source {
            ViewExecutionInputSource::Parameter(parameter) => Some(parameter),
            ViewExecutionInputSource::Local(_) => None,
        }
    }
}

impl From<ViewParameterInput> for ViewExecutionInput {
    fn from(input: ViewParameterInput) -> Self {
        Self {
            source: ViewExecutionInputSource::Parameter(input.parameter()),
            value_type: input.value_type(),
        }
    }
}

/// Owned binding outputs are transported by Core in this exact order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewBindingProgram {
    pub execution: ViewExpressionProgram,
    pub outputs: Box<[ViewLocalOutput]>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewLocalOutput {
    pub coordinate: ViewLocalCoordinate,
    pub value_type: RuntimeSemanticTypeId,
}

/// One expression's exact program identity, canonical free-input order and result type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewExpressionProgram {
    pub program: RuntimePureProgramId,
    pub inputs: Box<[ViewExecutionInput]>,
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
