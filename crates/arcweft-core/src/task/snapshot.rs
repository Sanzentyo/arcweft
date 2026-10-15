//! Inert task persistence through the common specification/request grammar.

use super::{RuntimeProgramOwner, TaskSpec, TaskSubmission};
use crate::value::{AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimePayload};

pub type TaskSubmissionSaveSnapshot = TaskSubmission<AwbcRuntimeValueSnapshot>;

impl TaskSubmissionSaveSnapshot {
    pub(crate) fn request_values(&self) -> impl Iterator<Item = &AwbcRuntimeValueSnapshot> {
        self.spec.request.payloads()
    }
}

impl TaskSpec<AwbcRuntimeValueSnapshot> {
    pub(crate) fn from_admitted_live(
        spec: &TaskSpec,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Self::from_admitted_storage(spec, Some(owner))
    }

    pub(crate) fn from_admitted_storage(
        spec: &TaskSpec,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Ok(Self {
            generation: spec.generation,
            producer: spec.producer.clone(),
            class: spec.class.clone(),
            priority: spec.priority,
            cancel_scope: spec.cancel_scope.clone(),
            policy: spec.policy,
            outcome: spec.outcome.clone(),
            debug_label: spec.debug_label.clone(),
            request: spec.request.map_admitted(|value| {
                AwbcRuntimeValueSnapshot::from_admitted_storage(value.value(), owner)
            })?,
        })
    }

    pub(crate) fn into_admitted_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<TaskSpec, AwbcRuntimeValueSnapshotError> {
        Ok(TaskSpec {
            generation: self.generation,
            producer: self.producer,
            class: self.class,
            priority: self.priority,
            cancel_scope: self.cancel_scope,
            policy: self.policy,
            outcome: self.outcome,
            debug_label: self.debug_label,
            request: self.request.into_mapped_admitted(|value| {
                value.into_admitted_runtime_value(owner).map(RuntimePayload)
            })?,
        })
    }
    #[cfg(test)]
    pub(crate) fn from_live(
        spec: &TaskSpec,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let limits = crate::entry::RuntimeSchemaLimits::engine_default();
        let mut admission = match owner {
            Some(owner) => {
                crate::value::RuntimeRetainedValueAdmission::for_program(owner.clone(), limits)
            }
            None => crate::value::RuntimeRetainedValueAdmission::standalone(limits),
        };
        spec.admit_retained(&mut admission)?;
        Self::from_admitted_storage(spec, owner)
    }

    #[cfg(test)]
    pub(crate) fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<TaskSpec, AwbcRuntimeValueSnapshotError> {
        self.admit_retained(
            &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            ),
        )?;
        self.into_admitted_live(owner)
    }
}
