use super::code_cursor::NativeSourceOperation;
use crate::pattern::{RuntimePattern, RuntimeSemanticTypeId};
use crate::plan::{FlowOp, RuntimeHostCallTarget, RuntimePlan};
use crate::step::{RuntimeHostCallId, RuntimeStepInput};
use crate::task::{
    HostTaskRequest, RuntimeProgramOwner, TaskCorrelation, TaskOutcomeContract, TaskSpec,
};
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission,
};

#[cfg(test)]
mod tests;

/// Suspended direct host call awaiting a typed host result.
#[derive(Clone, Debug, PartialEq)]
pub struct HostCallState {
    pub id: RuntimeHostCallId,
    continuation: Option<NativeHostCallContinuation>,
}

/// Native retains the authored operation and the journal identity of the issued request.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeHostCallContinuation {
    pub(super) operation: NativeSourceOperation,
    pub(super) sequence: u64,
    pub(super) correlation: TaskCorrelation,
}

impl HostCallState {
    pub(crate) fn observed(id: RuntimeHostCallId) -> Self {
        Self {
            id,
            continuation: None,
        }
    }

    pub(super) fn native(id: RuntimeHostCallId, continuation: NativeHostCallContinuation) -> Self {
        Self {
            id,
            continuation: Some(continuation),
        }
    }

    pub(super) fn continuation(&self) -> Option<&NativeHostCallContinuation> {
        self.continuation.as_ref()
    }

    pub(super) fn result_index(&self, input: &RuntimeStepInput) -> Option<usize> {
        input
            .host_call_results
            .iter()
            .position(|result| result.id == self.id)
    }
}

impl NativeHostCallContinuation {
    pub(super) fn render_id(public_id: &str, sequence: u64) -> RuntimeHostCallId {
        RuntimeHostCallId(if sequence == 0 {
            public_id.to_owned()
        } else {
            format!("{public_id}.{sequence}")
        })
    }

    pub(super) fn declaration<'a>(
        &self,
        plan: &'a RuntimePlan,
    ) -> Option<(Option<&'a RuntimePattern>, &'a RuntimeHostCallTarget)> {
        match self.operation.operation(plan)? {
            FlowOp::HostCall { binding, target } => Some((binding.as_ref(), target)),
            _ => None,
        }
    }

    fn declaration_with_result<'a>(
        &self,
        plan: &'a RuntimePlan,
    ) -> Option<(&'a RuntimeHostCallTarget, RuntimeSemanticTypeId)> {
        let (_, target) = self.declaration(plan)?;
        Some((
            target,
            plan.type_table().get(target.result)?.semantic_identity(),
        ))
    }

    pub(super) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.operation.admit_retained(admission)?;
        admission.validate_metadata(&(self.sequence, self.correlation))?;
        let Some(RuntimeProgramOwner::Plan(plan)) = admission.program_owner().cloned() else {
            return Err(invalid_host_call());
        };
        let (target, _) = self
            .declaration_with_result(&plan)
            .ok_or_else(invalid_host_call)?;
        admission.validate_metadata_string(&Self::render_id(&target.public_id, self.sequence).0)
    }

    pub(super) fn matches_receipt(
        &self,
        plan: &RuntimePlan,
        spec: &TaskSpec<AwbcRuntimeValueSnapshot>,
    ) -> bool {
        let Some((target, result)) = self.declaration_with_result(plan) else {
            return false;
        };
        let Some(HostTaskRequest::Custom {
            capability,
            operation,
            manifest_contract,
            ..
        }) = spec.request.as_host()
        else {
            return false;
        };
        capability.0 == target.capability
            && *operation == target.operation
            && *manifest_contract == target.contract
            && spec.outcome == TaskOutcomeContract::program(result)
            && target
                .producer
                .sealed_definition()
                .and_then(|producer| producer.producer_for(result, spec.producer.arguments()))
                .is_ok_and(|expected| expected == spec.producer)
    }

    pub(super) fn into_admitted_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<HostCallState, String> {
        let RuntimeProgramOwner::Plan(plan) = owner else {
            return Err(invalid_host_call().to_string());
        };
        let (_, target) = self
            .declaration(plan)
            .ok_or_else(|| invalid_host_call().to_string())?;
        Ok(HostCallState::native(
            Self::render_id(&target.public_id, self.sequence),
            self,
        ))
    }
}

pub(super) fn invalid_host_call() -> AwbcRuntimeValueSnapshotError {
    AwbcRuntimeValueSnapshotError::Message {
        message: "Native HostCall requires its original Plan operation and result type".into(),
    }
}
