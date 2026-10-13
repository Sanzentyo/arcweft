//! Inert task persistence through the common specification/request grammar.

use super::{RuntimeProgramOwner, TaskHandle, TaskSpec, TaskSubmission};
use crate::value::{AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimePayload};

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSubmissionSaveSnapshot {
    spec: TaskSpec<AwbcRuntimeValueSnapshot>,
    handle: TaskHandle,
}

impl TaskSubmissionSaveSnapshot {
    pub(crate) fn from_live(
        submission: &TaskSubmission,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Ok(Self {
            spec: TaskSpec::<AwbcRuntimeValueSnapshot>::from_live(submission.spec(), owner)?,
            handle: submission.handle(),
        })
    }
    pub(crate) fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<TaskSubmission, AwbcRuntimeValueSnapshotError> {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::InertSnapshot,
        );
        resources.validate_task_descriptor(&self.spec, 0)?;
        self.spec.validate_handle(self.handle).map_err(|error| {
            AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            }
        })?;
        let spec = self.spec.into_live(owner)?;
        TaskSubmission::try_from_accepted(spec, self.handle).map_err(|error| {
            AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            }
        })
    }
    pub(crate) fn request_values(&self) -> impl Iterator<Item = &AwbcRuntimeValueSnapshot> {
        self.spec.request.payloads()
    }
}

impl TaskSpec<AwbcRuntimeValueSnapshot> {
    pub(crate) fn from_live(
        spec: &TaskSpec,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
        );
        spec.validate_descriptor_inputs(&mut resources, 0, |payload| payload.value().view())?;
        spec.validate_outcome()
            .map_err(|error| AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            })?;
        spec.outcome.validate_storage_owner(owner)?;
        for payload in spec.request.payloads() {
            AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                payload.value().view(),
                owner,
                &resources,
            )?;
        }
        let value = |payload: &RuntimePayload| {
            AwbcRuntimeValueSnapshot::from_admitted_storage(payload.value(), owner)
        };
        Ok(Self {
            generation: spec.generation,
            producer: spec.producer.clone(),
            class: spec.class.clone(),
            priority: spec.priority,
            cancel_scope: spec.cancel_scope.clone(),
            policy: spec.policy,
            outcome: spec.outcome.clone(),
            request: spec.request.map_admitted(value)?,
            debug_label: spec.debug_label.clone(),
        })
    }
    pub(crate) fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<TaskSpec, AwbcRuntimeValueSnapshotError> {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::InertSnapshot,
        );
        self.validate_descriptor_inputs(&mut resources, 0, AwbcRuntimeValueSnapshot::view)?;
        self.validate_outcome()
            .map_err(|error| AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            })?;
        self.outcome.validate_storage_owner(Some(owner))?;
        for payload in self.request.payloads() {
            AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                payload.view(),
                Some(owner),
                &resources,
            )?;
        }
        Ok(TaskSpec {
            generation: self.generation,
            producer: self.producer,
            class: self.class,
            priority: self.priority,
            cancel_scope: self.cancel_scope,
            policy: self.policy,
            outcome: self.outcome,
            request: self.request.into_mapped_admitted(|value| {
                value.into_admitted_runtime_value(owner).map(RuntimePayload)
            })?,
            debug_label: self.debug_label,
        })
    }
}
