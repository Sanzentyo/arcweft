//! One selected source Map and its ordinary callable continuation.

use std::sync::Arc;

use super::FlowOp;
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::value::RuntimeStandardMapFamily;

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
    body: Arc<[FlowOp]>,
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
            body: body.into(),
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
