//! A selected launch keeps its genuine Source through registry publication.

use super::source::PreparedRuntimeSelectedProducer;
use super::{
    HostRestartPolicy, NeedProducerInstance, NeedProducerRuntimeArgument, NeedProducerSpec,
    RuntimeTaskProducerSourceError, TaskSpec,
};
use crate::plan::task_source::{RuntimeTaskSource, RuntimeTaskSourceProgram, RuntimeTaskSourceRef};
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::value::{RuntimeRetainedValueAdmission, RuntimeValue};

/// Complete admitted selected input. Its private fields bind the exact
/// arguments, request and completed key to the original immutable program.
/// No constructor accepts a copied request plan or caller Task digest.
#[derive(Debug)]
pub struct RuntimeNeedProducerStart {
    pub(super) producer: NeedProducerSpec<RuntimeTaskSource>,
    pub(super) arguments: Vec<NeedProducerRuntimeArgument>,
    pub(super) task_spec: TaskSpec,
    pub(super) restart: HostRestartPolicy,
}

impl RuntimeNeedProducerStart {
    pub fn task_spec(&self) -> &TaskSpec {
        &self.task_spec
    }
}

impl RuntimeTaskSourceRef<'_> {
    /// The existing bank admits every input before request projection copies
    /// a host-safe value. The result retains that same original/code Source.
    pub(crate) fn selected_start(
        &self,
        program: &RuntimeTaskSourceProgram,
        generation: super::GenerationId,
        arguments: Vec<NeedProducerRuntimeArgument<RuntimeValue>>,
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
        arguments: Vec<NeedProducerRuntimeArgument<RuntimeValue>>,
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
}
