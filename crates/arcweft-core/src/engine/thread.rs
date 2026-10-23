//! Thread producer admission against the original program, before frame copies.

use super::{Engine, FlowFiber};
use crate::entry::RuntimeSchemaLimits;
use crate::plan::task_source::{RuntimeTaskSourceError, RuntimeTaskSourceProgram};
use crate::plan::{FlowOp, RuntimeFlowBodyRole, RuntimeTaskPlanSealLimits};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::task::{
    NeedProducerInstance, RuntimeTaskTemplateArguments, TaskIdentityError, TaskRequest, TaskSpec,
};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeEvalError, RuntimeRetainedValueAdmission,
};
use std::sync::Arc;

#[cfg(test)]
mod tests;

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
    #[error(transparent)]
    Task(#[from] crate::task::TaskEnsureError),
    #[error(transparent)]
    Handle(#[from] crate::task::RuntimeNeedHandleError),
    #[error(transparent)]
    Binding(#[from] crate::pattern::RuntimePatternMatchError),
    #[error("Thread handle did not match its checked result binding")]
    BindingMismatch,
    #[error("unknown Thread capture {0:?}")]
    UnknownCapture(RuntimeLocalDeclarationId),
}

pub(super) struct PreparedThread {
    pub(super) spec: TaskSpec,
    pub(super) child: FlowFiber,
    pub(super) next_fiber_id: u64,
}

impl Engine {
    pub(super) fn complete_program_task_return(
        &mut self,
        value: crate::value::RuntimeValue,
        explicit: bool,
        output: &mut crate::step::RuntimeStepOutput,
        backend: &mut impl super::RuntimeCallBackend,
    ) {
        let super::FlowFiberLaunch::Program(launch) = &self.fiber.launch else {
            unreachable!("task return is dispatched only with an accepted Program launch")
        };
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            launch.source().program_owner(),
            RuntimeSchemaLimits::engine_default(),
        );
        if let Err(error) = launch.admit_result(&value, &mut admission) {
            self.fail_eval(error, output);
            return;
        }
        let launch = launch.clone();
        self.fiber.frame.pending_ops.clear();
        self.fiber.frame.await_observer = None;
        self.fiber.frame.invocation_result =
            Some((super::NativeInvocationRoot::Task(launch), value));
        self.unwind_control_stack(output, backend);
        if matches!(self.fiber.frame.status, super::FlowFiberStatus::Failed(_)) {
            return;
        }
        self.fiber.frame.status = if explicit {
            let value = &self
                .fiber
                .frame
                .invocation_result
                .as_ref()
                .expect("the returning task retains its result")
                .1;
            let label = crate::value::runtime_value_label(value);
            output.flow_events.push(super::FlowEvent::Return {
                value: label.clone(),
            });
            super::FlowFiberStatus::Done(super::FlowExit::Return(label))
        } else {
            output.flow_events.push(super::FlowEvent::Done);
            super::FlowFiberStatus::Done(super::FlowExit::Done)
        };
    }

    pub(super) fn prepare_thread(
        &self,
        operation: &super::code_cursor::NativeSourceOperation,
    ) -> Result<PreparedThread, ThreadPreparationError> {
        let Some(FlowOp::Thread { producer, .. }) = operation.operation(&self.plan) else {
            return Err(crate::task::RuntimeTaskProducerSourceError::RequestKind.into());
        };
        let program = RuntimeTaskSourceProgram::native(Arc::clone(&self.plan));
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            program.program_owner(),
            RuntimeSchemaLimits::engine_default(),
        );
        let limits = RuntimeTaskPlanSealLimits::default();
        let mut meter = admission.task_source_meter(limits)?;
        let source = program
            .original()
            .task_source(&producer.plan, limits, &mut meter)?;
        let Some(FlowOp::Thread {
            producer, captures, ..
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
        let body = self
            .original_body_range(operation, RuntimeFlowBodyRole::Body)
            .map_err(Box::new)?;
        let pending_ops = self.prepare_task_root_work(body, false).map_err(Box::new)?;
        let (child, next_fiber_id) = self
            .prepare_child_fiber(pending_ops, captures)
            .map_err(Box::new)?;
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
