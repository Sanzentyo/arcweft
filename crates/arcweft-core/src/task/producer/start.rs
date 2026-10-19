//! A selected launch keeps its genuine Source through registry publication.

use super::source::PreparedRuntimeSelectedProducer;
use super::{
    HostRestartPolicy, NeedProducerInstance, NeedProducerRuntimeArgument, NeedProducerSpec,
    RuntimeTaskProducerSourceError, TaskSpec,
};
use crate::plan::task_source::{RuntimeTaskSource, RuntimeTaskSourceProgram, RuntimeTaskSourceRef};
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimePayload,
    RuntimeRetainedValueAdmission, RuntimeStoredValue, RuntimeValue,
};

/// Source-admitted input shared by live execution and inert rollback storage.
/// Private fields keep its exact arguments, request, key and invocation binding
/// together. Neither role has a metadata-plan or caller-digest constructor.
#[derive(Debug)]
pub struct RuntimeNeedProducerStart<V = RuntimeValue, P = RuntimePayload> {
    pub(super) producer: NeedProducerSpec<RuntimeTaskSource>,
    pub(super) arguments: Vec<NeedProducerRuntimeArgument<V>>,
    pub(super) task_spec: TaskSpec<P>,
    pub(super) restart: HostRestartPolicy,
}

impl<V: PartialEq, P: PartialEq> PartialEq for RuntimeNeedProducerStart<V, P> {
    fn eq(&self, other: &Self) -> bool {
        self.producer.same_instance_contract(&other.producer)
            && self.arguments == other.arguments
            && self.task_spec == other.task_spec
            && self.restart == other.restart
    }
}

impl<V, P> RuntimeNeedProducerStart<V, P> {
    pub fn task_spec(&self) -> &TaskSpec<P> {
        &self.task_spec
    }
    pub fn producer(&self) -> &NeedProducerSpec<RuntimeTaskSource> {
        &self.producer
    }
    pub fn arguments(&self) -> &[NeedProducerRuntimeArgument<V>] {
        &self.arguments
    }
    pub fn source(&self) -> &RuntimeTaskSource {
        self.producer.source()
    }
    pub fn restart(&self) -> HostRestartPolicy {
        self.restart
    }
    pub(crate) fn binding(&self) -> Option<&RuntimeFunctionEffectInstantiation> {
        self.producer.binding()
    }
}

impl RuntimeTaskSourceRef<'_> {
    pub(crate) fn selected_start(
        &self,
        program: &RuntimeTaskSourceProgram,
        generation: super::GenerationId,
        arguments: Vec<NeedProducerRuntimeArgument>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<RuntimeNeedProducerStart, RuntimeTaskProducerSourceError> {
        self.prepare_selected_producer(program, admission)?
            .start(generation, arguments, binding)
    }
}

impl PreparedRuntimeSelectedProducer<'_, '_> {
    pub(crate) fn start(
        mut self,
        generation: super::GenerationId,
        arguments: Vec<NeedProducerRuntimeArgument>,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> Result<RuntimeNeedProducerStart, RuntimeTaskProducerSourceError> {
        let producer = self.producer_spec(&arguments, binding)?;
        let metadata = self.metadata;
        let request = metadata.project_request(&arguments)?;
        let task_spec = TaskSpec {
            generation,
            producer: NeedProducerInstance::try_from(&producer)?,
            class: metadata.class().clone(),
            priority: metadata.priority(),
            cancel_scope: metadata.cancel_scope().clone(),
            policy: metadata.policy(),
            outcome: metadata.outcome(),
            debug_label: request.debug_label(),
            request,
        };
        Ok(RuntimeNeedProducerStart {
            producer,
            arguments,
            task_spec,
            restart: metadata.restart(),
        })
    }

    pub(crate) fn restore_start<V: RuntimeStoredValue, P: RuntimeStoredValue>(
        mut self,
        arguments: Vec<NeedProducerRuntimeArgument<V>>,
        task_spec: TaskSpec<P>,
        binding: Option<RuntimeFunctionEffectInstantiation>,
    ) -> Result<RuntimeNeedProducerStart<V, P>, RuntimeTaskProducerSourceError> {
        let producer = self.producer_spec(&arguments, binding.as_ref())?;
        self.validate_specification(&producer, &arguments, &task_spec)?;
        Ok(RuntimeNeedProducerStart {
            producer,
            arguments,
            task_spec,
            restart: self.metadata.restart(),
        })
    }

    fn validate_specification<V: RuntimeStoredValue, P: RuntimeStoredValue>(
        &mut self,
        producer: &NeedProducerSpec<RuntimeTaskSource>,
        arguments: &[NeedProducerRuntimeArgument<V>],
        spec: &TaskSpec<P>,
    ) -> Result<(), RuntimeTaskProducerSourceError> {
        spec.admit_retained(self.admission)?;
        if spec.producer != NeedProducerInstance::try_from(producer)?
            || spec.class != *self.metadata.class()
            || spec.priority != self.metadata.priority()
            || spec.cancel_scope != *self.metadata.cancel_scope()
            || spec.policy != self.metadata.policy()
            || spec.outcome != self.metadata.outcome()
            || spec.debug_label != spec.request.debug_label()
            || !self.metadata.request_matches(arguments, &spec.request)?
        {
            return Err(RuntimeTaskProducerSourceError::Specification);
        }
        Ok(())
    }
}

impl<V: RuntimeStoredValue, P: RuntimeStoredValue> RuntimeNeedProducerStart<V, P> {
    pub(super) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let result = admission.with_program_owner(self.source().program_owner(), |admission| {
            let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
            let mut meter = admission.task_source_meter(limits)?;
            let source = self.source().borrow(limits, &mut meter).map_err(|error| {
                admission
                    .status()
                    .err()
                    .unwrap_or_else(|| AwbcRuntimeValueSnapshotError::Message {
                        message: error.to_string(),
                    })
            })?;
            let mut prepared = source
                .prepare_selected_producer(self.source().program(), admission)
                .map_err(AwbcRuntimeValueSnapshotError::from)?;
            let canonical = prepared
                .producer_spec(&self.arguments, self.binding())
                .map_err(AwbcRuntimeValueSnapshotError::from)?;
            if !canonical.same_instance_contract(&self.producer)
                || self.restart != prepared.metadata.restart()
            {
                return Err(AwbcRuntimeValueSnapshotError::Message {
                    message: "retained selected input differs from its original Source".into(),
                });
            }
            prepared
                .validate_specification(&canonical, &self.arguments, &self.task_spec)
                .map_err(AwbcRuntimeValueSnapshotError::from)
        });
        if let Err(error) = &result {
            admission.retain_failure(error.clone());
        }
        result
    }
}

type SavedStart = RuntimeNeedProducerStart<AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshot>;

impl Clone for SavedStart {
    fn clone(&self) -> Self {
        Self {
            producer: self.producer.clone(),
            arguments: self.arguments.clone(),
            task_spec: self.task_spec.clone(),
            restart: self.restart,
        }
    }
}

impl RuntimeNeedProducerStart {
    pub(super) fn snapshot_admitted(&self) -> Result<SavedStart, AwbcRuntimeValueSnapshotError> {
        let owner = self.source().program_owner();
        Ok(SavedStart {
            producer: self.producer.clone(),
            arguments: self
                .arguments
                .iter()
                .map(|argument| {
                    Ok(NeedProducerRuntimeArgument {
                        name: argument.name.clone(),
                        value: AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                            &argument.value,
                            &owner,
                        )?,
                    })
                })
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
            task_spec: TaskSpec::<AwbcRuntimeValueSnapshot>::from_admitted_live(
                &self.task_spec,
                &owner,
            )?,
            restart: self.restart,
        })
    }
}

impl SavedStart {
    pub(super) fn into_admitted_live(
        self,
    ) -> Result<RuntimeNeedProducerStart, AwbcRuntimeValueSnapshotError> {
        let owner = self.source().program_owner();
        Ok(RuntimeNeedProducerStart {
            producer: self.producer,
            arguments: self
                .arguments
                .into_iter()
                .map(|argument| {
                    Ok(NeedProducerRuntimeArgument {
                        name: argument.name,
                        value: argument.value.into_admitted_runtime_value(&owner)?,
                    })
                })
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
            task_spec: self.task_spec.into_admitted_live(&owner)?,
            restart: self.restart,
        })
    }
}
