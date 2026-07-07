//! Closure capture metadata threaded through checked runtime-plan lowering.

use crate::typed_evidence::RuntimeTypedExpressionId;

use super::RuntimePlanLowerOptions;

/// Runtime-plan-local closure capture metadata exported from semantic analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeClosureCaptureInventory {
    pub expression_id: RuntimeTypedExpressionId,
    pub captures: Vec<RuntimeClosureCapture>,
}

/// One local binding captured by a runtime closure expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeClosureCapture {
    pub name: String,
    pub type_label: String,
}

impl RuntimePlanLowerOptions {
    #[must_use]
    pub fn with_closure_capture_metadata(
        mut self,
        captures: impl IntoIterator<Item = RuntimeClosureCaptureInventory>,
    ) -> Self {
        self.closure_captures = captures.into_iter().collect();
        self
    }

    pub fn closure_captures(&self) -> &[RuntimeClosureCaptureInventory] {
        &self.closure_captures
    }
}
