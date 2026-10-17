//! One selected source Map and its ordinary callable continuation.

use std::sync::Arc;

use super::FlowOp;
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::value::RuntimeStandardMapFamily;

/// Completion of the original mapping schedule in this same control owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeStandardMapCompletion {
    Collection,
    Sum { map_operations: usize },
}

/// The preparation operations bind both operands once in source order. This
/// owner references those bindings and the result declaration in its body.
/// The body is the complete ordinary schedule, including the original callable;
/// a physical executor must prove an equivalent kernel before eliding it.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeStandardMapControl {
    family: RuntimeStandardMapFamily,
    source: RuntimeLocalDeclarationId,
    mapping: RuntimeLocalDeclarationId,
    result: RuntimeLocalDeclarationId,
    body: Arc<Vec<FlowOp>>,
    completion: RuntimeStandardMapCompletion,
}

impl RuntimeStandardMapControl {
    pub(crate) fn new(
        family: RuntimeStandardMapFamily,
        source: RuntimeLocalDeclarationId,
        mapping: RuntimeLocalDeclarationId,
        result: RuntimeLocalDeclarationId,
        body: Vec<FlowOp>,
    ) -> Self {
        Self {
            family,
            source,
            mapping,
            result,
            body: Arc::new(body),
            completion: RuntimeStandardMapCompletion::Collection,
        }
    }

    pub const fn family(&self) -> RuntimeStandardMapFamily {
        self.family
    }
    pub const fn source(&self) -> RuntimeLocalDeclarationId {
        self.source
    }
    pub const fn mapping(&self) -> RuntimeLocalDeclarationId {
        self.mapping
    }
    pub const fn result(&self) -> RuntimeLocalDeclarationId {
        self.result
    }
    pub fn body(&self) -> &[FlowOp] {
        &self.body
    }

    pub const fn completion(&self) -> RuntimeStandardMapCompletion {
        self.completion
    }

    pub fn mapping_body(&self) -> &[FlowOp] {
        match self.completion {
            RuntimeStandardMapCompletion::Collection => self.body(),
            RuntimeStandardMapCompletion::Sum { map_operations } => &self.body[..map_operations],
        }
    }

    pub fn consuming_body(&self) -> Option<&[FlowOp]> {
        match self.completion {
            RuntimeStandardMapCompletion::Collection => None,
            RuntimeStandardMapCompletion::Sum { map_operations } => {
                Some(&self.body[map_operations..])
            }
        }
    }

    /// Original outer-operation cost when mapping and consumption complete in
    /// one admitted kernel. Ordinary callbacks keep their own step budgets.
    pub fn completion_operation_cost(&self) -> usize {
        1 + self.consuming_body().map_or(0, <[FlowOp]>::len)
    }

    pub(in crate::plan) fn append_sum(&mut self, continuation: Vec<FlowOp>) {
        let map_operations = self.body.len();
        Arc::make_mut(&mut self.body).extend(continuation);
        self.completion = RuntimeStandardMapCompletion::Sum { map_operations };
    }

    /// Admit the complete mapping before any callback copy, independently of
    /// the function carrier's static affine classification.
    pub(crate) fn admit_mapping(
        &self,
        value: &crate::value::RuntimeValue,
    ) -> Result<(), crate::value::RuntimeEvalError> {
        if !matches!(value, crate::value::RuntimeValue::Callable(_)) {
            return Err(crate::value::RuntimeEvalError::ExpectedFunction(
                crate::value::runtime_value_label(value),
            ));
        }
        if !crate::value::ownership::RuntimeValueOwnershipRequirement::Unrestricted
            .accepts(value.ownership())
        {
            return Err(crate::value::RuntimeEvalError::AffineLocalCopy(
                self.mapping,
            ));
        }
        Ok(())
    }
}
