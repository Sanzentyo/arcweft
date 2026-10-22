//! Inert task persistence through the common specification/request grammar.

use super::{RuntimeProgramOwner, TaskSpec, TaskSubmission};
use crate::value::{AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimePayload};

pub type TaskSpecSaveSnapshot = TaskSpec<
    AwbcRuntimeValueSnapshot,
    super::TaskPlanSemanticDigest,
    crate::plan::RuntimeTaskSourceAssertion,
>;
pub type TaskSubmissionSaveSnapshot = TaskSubmission<
    AwbcRuntimeValueSnapshot,
    super::TaskPlanSemanticDigest,
    crate::plan::RuntimeTaskSourceAssertion,
>;

pub(crate) struct TaskSourceRestore<'a> {
    pub program: &'a crate::plan::task_source::RuntimeTaskSourceProgram,
    pub admission: &'a mut crate::value::RuntimeRetainedValueAdmission,
}

pub(crate) trait TaskSnapshotSource: super::TaskExecutionSource + Sized {
    fn snapshot_source(source: &crate::plan::RuntimeTaskSource) -> Self;
    fn restore_source(
        self,
        owner: &RuntimeProgramOwner,
        context: Option<TaskSourceRestore<'_>>,
    ) -> Result<crate::plan::RuntimeTaskSource, AwbcRuntimeValueSnapshotError>;
}

impl TaskSnapshotSource for crate::plan::RuntimeTaskSource {
    fn snapshot_source(source: &Self) -> Self {
        source.clone()
    }

    fn restore_source(
        self,
        owner: &RuntimeProgramOwner,
        _context: Option<TaskSourceRestore<'_>>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        if !owner.same_program(&self.program_owner()) {
            return Err(AwbcRuntimeValueSnapshotError::Message {
                message: "rollback task Source belongs to another executable".into(),
            });
        }
        Ok(self)
    }
}

impl TaskSnapshotSource for crate::plan::RuntimeTaskSourceAssertion {
    fn snapshot_source(source: &crate::plan::RuntimeTaskSource) -> Self {
        source.assertion()
    }

    fn restore_source(
        self,
        owner: &RuntimeProgramOwner,
        context: Option<TaskSourceRestore<'_>>,
    ) -> Result<crate::plan::RuntimeTaskSource, AwbcRuntimeValueSnapshotError> {
        let context = context.ok_or_else(|| AwbcRuntimeValueSnapshotError::Message {
            message: "program task restore requires its enclosing admitted Source".into(),
        })?;
        let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
        let mut meter = context.admission.task_source_meter(limits)?;
        let source = context
            .program
            .borrow_assertion(&self, limits, &mut meter)
            .map_err(|error| {
                context.admission.status().err().unwrap_or_else(|| {
                    AwbcRuntimeValueSnapshotError::Message {
                        message: error.to_string(),
                    }
                })
            })?;
        crate::plan::RuntimeTaskSource::from_borrowed(&source, context.program, owner).ok_or_else(
            || AwbcRuntimeValueSnapshotError::Message {
                message: "program task restore selected another executable".into(),
            },
        )
    }
}

impl TaskSubmissionSaveSnapshot {
    pub(crate) fn request_values(&self) -> impl Iterator<Item = &AwbcRuntimeValueSnapshot> {
        self.spec.request.payloads()
    }
}

impl<S: TaskSnapshotSource> TaskSpec<AwbcRuntimeValueSnapshot, super::TaskPlanSemanticDigest, S> {
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
            request: spec.request.map_admitted(
                |value| AwbcRuntimeValueSnapshot::from_admitted_storage(value.value(), owner),
                |source| Ok(S::snapshot_source(source)),
            )?,
        })
    }

    pub(crate) fn into_admitted_live(
        self,
        owner: &RuntimeProgramOwner,
        source_context: Option<TaskSourceRestore<'_>>,
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
            request: self.request.into_mapped_admitted(
                |value| value.into_admitted_runtime_value(owner).map(RuntimePayload),
                |source| source.restore_source(owner, source_context),
            )?,
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
        self.into_admitted_live(owner, None)
    }
}
