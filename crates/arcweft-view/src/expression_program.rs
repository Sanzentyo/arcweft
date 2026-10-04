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
