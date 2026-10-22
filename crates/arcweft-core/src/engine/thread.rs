//! Thread producer admission against the original program, before frame copies.

use super::{Engine, FlowFiber};
use crate::entry::RuntimeSchemaLimits;
use crate::plan::task_source::{RuntimeTaskSourceError, RuntimeTaskSourceProgram};
use crate::plan::{FlowOp, RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealLimits};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::task::{
    NeedProducerInstance, RuntimeTaskTemplateArguments, TaskIdentityError, TaskRequest, TaskSpec,
};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeEvalError, RuntimeRetainedValueAdmission,
};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub(super) enum ThreadPreparationError {
    #[error(transparent)]
    Admission(#[from] AwbcRuntimeValueSnapshotError),
    #[error(transparent)]
    Source(#[from] RuntimeTaskSourceError),
    #[error(transparent)]
    Producer(#[from] crate::task::RuntimeTaskProducerSourceError),
    #[error(transparent)]
    Identity(#[from] TaskIdentityError),
    #[error(transparent)]
    Frame(#[from] Box<RuntimeEvalError>),
    #[error("unknown Thread capture {0:?}")]
    UnknownCapture(RuntimeLocalDeclarationId),
}

pub(super) struct PreparedThread {
    pub(super) spec: TaskSpec,
    pub(super) child: FlowFiber,
    pub(super) next_fiber_id: u64,
}

impl Engine {
    pub(super) fn prepare_thread(
        &self,
        coordinate: &RuntimeTaskPlanBuildCoordinate,
    ) -> Result<PreparedThread, ThreadPreparationError> {
        let program = RuntimeTaskSourceProgram::native(Arc::clone(&self.plan));
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            program.program_owner(),
            RuntimeSchemaLimits::engine_default(),
        );
        let limits = RuntimeTaskPlanSealLimits::default();
        let mut meter = admission.task_source_meter(limits)?;
        let source = program
            .original()
            .task_source(coordinate, limits, &mut meter)?;
        let Some(FlowOp::Thread {
            producer,
            captures,
            body,
            ..
        }) = source.request_endpoint().operation()
        else {
            return Err(crate::task::RuntimeTaskProducerSourceError::RequestKind.into());
        };
        admission.validate_metadata_collection(captures.len())?;
        let values = captures
            .iter()
            .map(|local| {
                self.fiber
                    .frame
                    .env
                    .get(*local)
                    .ok_or(ThreadPreparationError::UnknownCapture(*local))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let checked = source.template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::Thread { captures: &values },
            self.fiber.frame.env.function_instantiation(),
            &mut admission,
        )?;
        admission.validate_metadata_string(&producer.debug_label)?;
        admission.validate_metadata(&checked.source().assertion())?;
        let instance = NeedProducerInstance::try_from(&checked)?;
        let (child, next_fiber_id) = self.prepare_child_fiber(body, captures).map_err(Box::new)?;
        let spec = TaskSpec {
            generation: self.generation,
            producer: instance,
            class: producer.class.clone(),
            priority: producer.priority,
            cancel_scope: producer.cancel_scope.clone(),
            policy: producer.policy,
            outcome: producer.outcome(),
            request: TaskRequest::Program(checked.source().clone()),
            debug_label: producer.debug_label.clone(),
        };
        Ok(PreparedThread {
            spec,
            child,
            next_fiber_id,
        })
    }
}
