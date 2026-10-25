//! Program task admission against the original Source, before frame copies.

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
pub(super) enum ProgramTaskPreparationError {
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
    #[error("Program task handle did not match its checked result binding")]
    BindingMismatch,
    #[error("unknown Program task capture {0:?}")]
    UnknownCapture(RuntimeLocalDeclarationId),
}

pub(super) struct PreparedProgramTask {
    pub(super) spec: TaskSpec,
    pub(super) child: FlowFiber,
    pub(super) next_fiber_id: u64,
    captures: Vec<crate::plan::RuntimeProgramTaskCapture>,
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
        if let Err(error) = self.unwind_control_stack(output, backend) {
            self.fail_eval(error, output);
            return;
        }
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

    pub(super) fn prepare_program_task(
        &self,
        operation: &super::code_cursor::NativeSourceOperation,
    ) -> Result<PreparedProgramTask, ProgramTaskPreparationError> {
        let Some(FlowOp::ProgramTask { producer, .. }) = operation.operation(&self.plan) else {
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
        let Some(FlowOp::ProgramTask {
            producer, captures, ..
        }) = source.request_endpoint().operation()
        else {
            return Err(crate::task::RuntimeTaskProducerSourceError::RequestKind.into());
        };
        admission.validate_metadata_collection(captures.len())?;
        let values = captures
            .iter()
            .map(|capture| {
                self.fiber
                    .frame
                    .env
                    .get(capture.local)
                    .ok_or(ProgramTaskPreparationError::UnknownCapture(capture.local))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let checked = source.template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::ProgramTask { captures: &values },
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
            .prepare_child_fiber(pending_ops, &[])
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
        Ok(PreparedProgramTask {
            spec,
            child,
            next_fiber_id,
            captures: captures.clone(),
        })
    }

    pub(super) fn start_prepared_program_task(
        &mut self,
        prepared: PreparedProgramTask,
        binding: &crate::pattern::RuntimePattern,
    ) -> Result<crate::task::TaskSubmission, ProgramTaskPreparationError> {
        let PreparedProgramTask {
            spec,
            mut child,
            next_fiber_id,
            captures,
        } = prepared;
        let captured = self
            .fiber
            .frame
            .env
            .prepare_task_captures(&captures)
            .map_err(Box::new)?;
        let (accepted, launch, bindings) =
            self.need_producers
                .ensure_program_task_with_binding(spec, |receipt| {
                    let handle = crate::task::RuntimeNeedHandle::try_from(receipt.clone())?;
                    crate::pattern::match_runtime_pattern_owned(
                        &self.plan,
                        binding,
                        crate::value::RuntimeValue::NeedHandle(handle),
                        captured.function_instantiation(),
                    )?
                    .ok_or(ProgramTaskPreparationError::BindingMismatch)
                })?;
        // Every fallible Source/capture/receipt/binding check precedes this
        // exclusive Move commit and the parent/child scheduler mutation.
        child.frame.env = captured.commit();
        child.launch = super::FlowFiberLaunch::Program(launch);
        child.frame.status = super::FlowFiberStatus::Running;
        self.child_fibers.push_back(child);
        self.next_fiber_id = next_fiber_id;
        self.run_child_next = true;
        self.fiber.frame.env.bind_all(bindings);
        self.task_request_quota_remaining -= 1;
        Ok(accepted)
    }
}
