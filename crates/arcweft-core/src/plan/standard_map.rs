//! One selected source Map and its ordinary callable continuation.

use std::sync::Arc;

use super::FlowOp;
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::value::RuntimeStandardMapFamily;

#[cfg(test)]
mod tests;

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
pub struct RuntimeStandardMapControl<Local = RuntimeLocalDeclarationId, Body = Arc<Vec<FlowOp>>> {
    family: RuntimeStandardMapFamily,
    source: Local,
    mapping: Local,
    result: Local,
    body: Body,
    completion: RuntimeStandardMapCompletion,
}

impl<Local, Body> RuntimeStandardMapControl<Local, Body> {
    pub const fn family(&self) -> RuntimeStandardMapFamily {
        self.family
    }

    pub const fn completion(&self) -> RuntimeStandardMapCompletion {
        self.completion
    }
}

/// Unsealed construction phase of the same complete mapping schedule. Source
/// callbacks retain their real builder handles, and both bodies remain owned
/// until the existing operation factory admits them in function context.
pub type RuntimeStandardMapControlSeed =
    RuntimeStandardMapControl<super::RuntimeLocalSeedId, Vec<super::RuntimeFlowOpSeed>>;

impl RuntimeStandardMapControlSeed {
    pub fn collection(
        family: RuntimeStandardMapFamily,
        source: super::RuntimeLocalSeedId,
        mapping: super::RuntimeLocalSeedId,
        result: super::RuntimeLocalSeedId,
        body: Vec<super::RuntimeFlowOpSeed>,
    ) -> Self {
        Self {
            family,
            source,
            mapping,
            result,
            body,
            completion: RuntimeStandardMapCompletion::Collection,
        }
    }

    /// Supplies original definition data, without granting a fused execution
    /// proof. The builder rechecks the complete consumption against its real
    /// result declaration and the common sequence-sum grammar.
    pub fn sum(
        family: RuntimeStandardMapFamily,
        source: super::RuntimeLocalSeedId,
        mapping: super::RuntimeLocalSeedId,
        result: super::RuntimeLocalSeedId,
        mut mapping_body: Vec<super::RuntimeFlowOpSeed>,
        consumption: Vec<super::RuntimeFlowOpSeed>,
    ) -> Self {
        let map_operations = mapping_body.len();
        mapping_body.extend(consumption);
        Self {
            family,
            source,
            mapping,
            result,
            body: mapping_body,
            completion: RuntimeStandardMapCompletion::Sum { map_operations },
        }
    }

    pub const fn source(&self) -> &super::RuntimeLocalSeedId {
        &self.source
    }
    pub const fn mapping(&self) -> &super::RuntimeLocalSeedId {
        &self.mapping
    }
    pub const fn result(&self) -> &super::RuntimeLocalSeedId {
        &self.result
    }

    pub fn mapping_body(&self) -> &[super::RuntimeFlowOpSeed] {
        match self.completion {
            RuntimeStandardMapCompletion::Collection => &self.body,
            RuntimeStandardMapCompletion::Sum { map_operations } => &self.body[..map_operations],
        }
    }

    pub fn consuming_body(&self) -> Option<&[super::RuntimeFlowOpSeed]> {
        match self.completion {
            RuntimeStandardMapCompletion::Collection => None,
            RuntimeStandardMapCompletion::Sum { map_operations } => {
                Some(&self.body[map_operations..])
            }
        }
    }

    fn split_body_parts(
        mut body: Vec<super::RuntimeFlowOpSeed>,
        completion: RuntimeStandardMapCompletion,
    ) -> (
        Vec<super::RuntimeFlowOpSeed>,
        Option<Vec<super::RuntimeFlowOpSeed>>,
    ) {
        let consumption = match completion {
            RuntimeStandardMapCompletion::Collection => None,
            RuntimeStandardMapCompletion::Sum { map_operations } => {
                Some(body.split_off(map_operations))
            }
        };
        (body, consumption)
    }

    pub(in crate::plan) fn into_body_parts(
        self,
    ) -> (
        Vec<super::RuntimeFlowOpSeed>,
        Option<Vec<super::RuntimeFlowOpSeed>>,
    ) {
        Self::split_body_parts(self.body, self.completion)
    }

    /// Maps each complete source body separately, retaining its completion
    /// boundary even when a transformation changes operation counts.
    pub fn try_map_bodies<E>(
        self,
        mut map: impl FnMut(Vec<super::RuntimeFlowOpSeed>) -> Result<Vec<super::RuntimeFlowOpSeed>, E>,
    ) -> Result<Self, E> {
        let Self {
            family,
            source,
            mapping,
            result,
            body,
            completion,
        } = self;
        let (body, consumption) = Self::split_body_parts(body, completion);
        let body = map(body)?;
        Ok(match consumption {
            None => Self::collection(family, source, mapping, result, body),
            Some(consumption) => {
                Self::sum(family, source, mapping, result, body, map(consumption)?)
            }
        })
    }
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
