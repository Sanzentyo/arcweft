//! Original Program task admission. The VM leaves captures and its cursor in
//! the parent until Source, child inputs and the actual receipt all admit.

mod retirement;
#[cfg(test)]
mod tests;

use super::{AwbcProductStepExecutor, ProductChildFiber, ProductChildFiberOwner, ProductStepError};
use crate::awbc::fiber::{
    FiberCursor, FiberStateError, PreparedProgramTaskFiber, PreparedYieldedRegisterWrite,
    validate_program_task_argument_refs,
};
use crate::awbc::schema::{AwbcRegisterId, AwbcTaskPlanId};
use crate::entry::RuntimeSchemaLimits;
use crate::plan::RuntimeTaskPlanSealLimits;
use crate::runtime_id::{RuntimeFiberInstanceId, RuntimePersistentFiberId};
use crate::step::RuntimeStepOutput;
use crate::task::{NeedProducerInstance, RuntimeTaskTemplateArguments, TaskRequest, TaskSpec};
use crate::value::{RuntimeRetainedValueAdmission, RuntimeStoredValue, RuntimeValue};

#[derive(Debug, thiserror::Error)]
pub(in crate::awbc) enum ProgramTaskError {
    #[error(transparent)]
    Source(#[from] crate::plan::task_source::RuntimeAwbcProgramSourceError),
    #[error(transparent)]
    Producer(#[from] crate::task::RuntimeTaskProducerSourceError),
    #[error(transparent)]
    Admission(#[from] crate::value::AwbcRuntimeValueSnapshotError),
    #[error(transparent)]
    Frame(#[from] FiberStateError),
    #[error(transparent)]
    Identity(#[from] crate::task::TaskIdentityError),
    #[error(transparent)]
    RuntimeIdentity(#[from] crate::runtime_id::RuntimeIdExhausted),
    #[error(transparent)]
    Ensure(#[from] crate::task::TaskEnsureError),
    #[error(transparent)]
    Handle(#[from] crate::task::RuntimeNeedHandleError),
    #[error("Program start has no admitted original Source")]
    MissingSource,
    #[error("Program start's parent fiber or destination is absent")]
    Parent,
    #[error("Program start exceeds its child identity or collection capacity")]
    Capacity,
}

enum ParentSlot {
    Main,
    Child(usize),
}

struct PreparedProgramStart {
    parent: ParentSlot,
    write: PreparedYieldedRegisterWrite,
    binding: Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
    destination: crate::pattern::RuntimeSemanticTypeId,
    admission: RuntimeRetainedValueAdmission,
    spec: TaskSpec,
    child: PreparedProgramTaskFiber,
    parent_id: RuntimeFiberInstanceId,
    generation: crate::task::GenerationId,
    next_generation: u64,
    next_fiber_instance: crate::runtime_id::RuntimeIdCursor,
}

impl AwbcProductStepExecutor {
    pub(super) fn commit_program_task_started(
        &mut self,
        cursor: FiberCursor,
        observed: RuntimePersistentFiberId,
        destination: AwbcRegisterId,
        plan: AwbcTaskPlanId,
        captures: &[AwbcRegisterId],
        output: &mut RuntimeStepOutput,
    ) {
        if self.remaining_new_task_requests == 0 {
            self.fail_with_budget_error(
                "Program start exceeds this step's task request quota".into(),
                output,
            );
            return;
        }
        let prepared = (|| {
            self.child_fibers
                .try_reserve(1)
                .map_err(|_| ProgramTaskError::Capacity)?;
            output
                .requests
                .tasks
                .try_reserve(1)
                .map_err(|_| ProgramTaskError::Capacity)?;
            self.prepare_program_task_start(cursor, observed, destination, plan, captures)
        })();
        let mut prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_with_error(error.into(), output);
                return;
            }
        };
        let accepted =
            self.need_producers
                .ensure_program_task_with_binding(prepared.spec, |receipt| {
                    let value = RuntimeValue::NeedHandle(crate::task::RuntimeNeedHandle::try_from(
                        receipt.clone(),
                    )?);
                    prepared
                        .admission
                        .validate_owned_typed_borrowed_in_invocation(
                            value.borrowed_value(),
                            prepared.destination,
                            prepared.binding.as_deref(),
                        )?;
                    Ok::<_, ProgramTaskError>(value)
                });
        let (receipt, launch, value) = match accepted {
            Ok(accepted) => accepted,
            Err(error) => {
                self.fail_with_error(error.into(), output);
                return;
            }
        };
        let parent = match prepared.parent {
            ParentSlot::Main => &mut self.fiber,
            ParentSlot::Child(index) => &mut self.child_fibers[index].fiber,
        };
        assert_eq!(parent.instance, prepared.parent_id);
        let child = prepared.child.commit(parent);
        let deliver = launch.delivery() == crate::task::ProgramTaskDelivery::HostMetadata;
        parent.commit_yielded_register_write_prepared(prepared.write, value);
        self.child_fibers.push_back(ProductChildFiber {
            owner: ProductChildFiberOwner::Program {
                launch,
                parent: prepared.parent_id,
            },
            fiber: child,
            runtime_generation: prepared.generation,
            pending_host_call: None,
        });
        self.next_generation = prepared.next_generation;
        self.next_fiber_instance = prepared.next_fiber_instance;
        self.remaining_new_task_requests -= 1;
        if deliver {
            output.requests.tasks.push(receipt);
        }
    }

    fn prepare_program_task_start(
        &self,
        cursor: FiberCursor,
        observed: RuntimePersistentFiberId,
        destination: AwbcRegisterId,
        plan: AwbcTaskPlanId,
        captures: &[AwbcRegisterId],
    ) -> Result<PreparedProgramStart, ProgramTaskError> {
        let (slot, parent, generation) = if self.fiber.instance.get().get() == observed.get() {
            (ParentSlot::Main, &self.fiber, self.runtime_generation)
        } else {
            let (index, child) = self
                .child_fibers
                .iter()
                .enumerate()
                .find(|(_, child)| child.fiber.instance.get().get() == observed.get())
                .ok_or(ProgramTaskError::Parent)?;
            (
                ParentSlot::Child(index),
                &child.fiber,
                child.runtime_generation,
            )
        };
        let write = parent.validate_yielded_register_write(cursor, destination)?;
        let program = self
            .program
            .task_source
            .as_ref()
            .ok_or(ProgramTaskError::MissingSource)?;
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            program.program_owner(),
            RuntimeSchemaLimits::engine_default(),
        );
        let count = self
            .child_fibers
            .len()
            .checked_add(1)
            .ok_or(ProgramTaskError::Capacity)?;
        admission.validate_metadata_collection(count)?;
        admission.validate_metadata_collection(captures.len())?;
        let limits = RuntimeTaskPlanSealLimits::default();
        let mut meter = admission.task_source_meter(limits)?;
        let source = program.awbc_program_source(plan, limits, &mut meter)?;
        let Some(crate::plan::FlowOp::ProgramTask { producer, .. }) =
            source.request_endpoint().operation()
        else {
            return Err(ProgramTaskError::MissingSource);
        };
        let frame = parent.active_frame()?;
        let values = captures
            .iter()
            .map(|register| frame.register(*register))
            .collect::<Result<Vec<_>, _>>()?;
        let checked = source.template_producer_spec(
            program,
            RuntimeTaskTemplateArguments::ProgramTask { captures: &values },
            frame.type_instantiation.as_deref(),
            &mut admission,
        )?;
        admission.validate_metadata_string(&producer.debug_label)?;
        admission.validate_metadata(&checked.source().assertion())?;
        let inputs = validate_program_task_argument_refs(
            &self.program,
            plan,
            &values,
            frame.type_instantiation.as_deref(),
            &admission.work(),
        )?;
        let destination = self
            .program
            .frame_layouts
            .get(frame.layout.index())
            .and_then(|layout| layout.slots.get(destination.index()))
            .and_then(|slot| self.program.runtime_types.get(slot.ty.index()))
            .ok_or(ProgramTaskError::Parent)?
            .semantic_identity();
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(ProgramTaskError::Capacity)?;
        let mut next_fiber_instance = self.next_fiber_instance;
        let instance = RuntimeFiberInstanceId::from_allocated(
            next_fiber_instance.take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)?,
        );
        let spec = TaskSpec {
            generation,
            producer: NeedProducerInstance::try_from(&checked)?,
            class: producer.class.clone(),
            priority: producer.priority,
            cancel_scope: producer.cancel_scope.clone(),
            policy: producer.policy,
            outcome: producer.outcome(),
            request: TaskRequest::Program(checked.source().clone()),
            debug_label: producer.debug_label.clone(),
        };
        let child = parent.prepare_program_task_fiber(
            &self.program,
            plan,
            captures,
            inputs,
            instance,
            self.next_generation,
        )?;
        Ok(PreparedProgramStart {
            parent: slot,
            write,
            binding: frame.type_instantiation.clone(),
            destination,
            admission,
            spec,
            child,
            parent_id: parent.instance,
            generation,
            next_generation,
            next_fiber_instance,
        })
    }
}
