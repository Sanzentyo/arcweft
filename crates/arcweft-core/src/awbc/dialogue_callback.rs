//! Static callback admission borrows the actual program's typed invocation.

use super::schema::{
    AwbcFunctionId, AwbcFunctionKind, AwbcProgram, AwbcRuntimeTypeShape, AwbcSignature, AwbcTypeId,
};
use crate::plan::{RuntimeCallableRetainedInput, RuntimeCallableZeroArgProjection};
use crate::runtime_id::RuntimeCallableStateId;

#[derive(Debug, thiserror::Error)]
pub(crate) enum AwbcDialogueCallbackError {
    #[error("dialogue callback state {state} is absent")]
    State { state: RuntimeCallableStateId },
    #[error("dialogue callback is not an unattached zero-argument invocation")]
    Shape,
    #[error("dialogue callback must return its declared Unit type")]
    Result,
    #[error("dialogue callback body is not an ordinary function")]
    Body,
    #[error("dialogue callback signature disagrees with its projected inputs")]
    Signature,
}

pub(crate) struct AwbcDialogueCallbackAbi<'a> {
    retained: &'a [RuntimeCallableRetainedInput<AwbcTypeId>],
    projection: RuntimeCallableZeroArgProjection<'a, AwbcFunctionId>,
    signature: &'a AwbcSignature,
}
impl AwbcDialogueCallbackAbi<'_> {
    pub(crate) fn function(&self) -> AwbcFunctionId {
        *self.projection.function()
    }
    pub(crate) const fn retained(&self) -> &[RuntimeCallableRetainedInput<AwbcTypeId>] {
        self.retained
    }
    pub(crate) const fn signature(&self) -> &AwbcSignature {
        self.signature
    }
    pub(crate) fn positions(&self) -> impl Iterator<Item = usize> + '_ {
        self.projection.retained_positions()
    }
    pub(crate) fn input_count(&self) -> usize {
        self.projection.input_count()
    }
}
impl AwbcProgram {
    pub(crate) fn inspect_dialogue_callback(
        &self,
        id: RuntimeCallableStateId,
    ) -> Result<AwbcDialogueCallbackAbi<'_>, AwbcDialogueCallbackError> {
        use AwbcDialogueCallbackError as Error;
        let state = self
            .callable_states
            .get(id.index())
            .ok_or(Error::State { state: id })?;
        let projection = state.zero_argument_invocation().ok_or(Error::Shape)?;
        if !matches!(
            self.runtime_types
                .get(state.result.index())
                .map(|ty| ty.shape()),
            Some(AwbcRuntimeTypeShape::Unit)
        ) {
            return Err(Error::Result);
        }
        let body = self
            .functions
            .get(projection.function().index())
            .ok_or(Error::Body)?;
        if body.kind != AwbcFunctionKind::Ordinary {
            return Err(Error::Body);
        }
        let signature = self
            .signatures
            .get(body.signature.index())
            .ok_or(Error::Signature)?;
        if signature.result != Some(state.result)
            || signature.params.len() != projection.input_count()
            || projection
                .retained_positions()
                .zip(&signature.params)
                .any(|(index, ty)| state.retained[index].ty != *ty)
        {
            return Err(Error::Signature);
        }
        Ok(AwbcDialogueCallbackAbi {
            retained: &state.retained,
            projection,
            signature,
        })
    }
}
