//! Declaration-owned default programs use the ordinary executable value ABI.

use arcweft_id::{RuntimeSemanticTypeId, runtime_program::RuntimePureProgramId};
use serde::{Deserialize, Serialize};

use crate::ViewParameterInput;

/// One default's exact program identity and canonical free-input order.
/// The destination parameter remains the authority for its declared type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewParameterDefaultProgram {
    pub program: RuntimePureProgramId,
    pub inputs: Box<[ViewParameterInput]>,
    pub result_type: RuntimeSemanticTypeId,
}
