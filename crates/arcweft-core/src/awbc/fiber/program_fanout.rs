//! Owned input frontier of an original Program fanout kernel.

use super::{
    AwbcPatternId, AwbcSaveResult, AwbcTaskPlanId, FiberAwaitManyInFlight, InertFiberNeed,
    InertRuntimeValueStorage,
};
use crate::task::{RuntimeNeedHandle, RuntimeNeedHandleSaveSnapshot, RuntimeProgramOwner};
use crate::value::{AwbcRuntimeValueSnapshot, RuntimeValue};
use serde::{Deserialize, Serialize};

impl<V, N> FiberProgramFanoutState<V, N> {
    pub(super) fn validate_inputs(
        &self,
        frame: &super::FrameValueContext<'_, V>,
        cursor: super::FiberCursor,
        resume: super::FiberResumeTarget,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
        view: impl Fn(&V) -> crate::value::RuntimeValueView<'_> + Copy,
        need: impl Fn(&N) -> crate::value::RuntimeNeedView<'_> + Copy,
    ) -> Result<(), super::FiberStateError> {
        use super::FiberStateError;
        use crate::awbc::schema::{
            AwbcProgramFanoutInput, AwbcProgramTaskKernel, AwbcTaskPlanExecution,
        };
        use crate::value::{RuntimeScalarView, RuntimeUInt, RuntimeValueView};
        frame.validate_await_many_site(self.plan, self.binding, cursor, resume, resources)?;
        for count in [self.items.len(), self.in_flight.len(), self.results.len()] {
            resources
                .work()
                .collection(count)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        }
        let program = frame.program;
        let row = program
            .task_plans
            .get(self.plan.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let AwbcTaskPlanExecution::Program(body) = &row.execution else {
            return Err(FiberStateError::InvalidFrame);
        };
        let AwbcProgramTaskKernel::Fanout {
            worker,
            source,
            limit,
            inputs,
        } = &body.body
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        let function = program
            .functions
            .get(body.kernel.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let params = &program
            .signatures
            .get(function.signature.index())
            .ok_or(FiberStateError::InvalidFrame)?
            .params;
        if body.kernel != frame.function
            || body.capture_modes.len() != params.len()
            || !matches!(frame.registers.get(*source as usize), Some(value) if value.as_ref().is_none())
            || !matches!(frame.registers.get(*limit as usize).and_then(|value| value.as_ref()).map(view),
                Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U32(value)))) if value == self.limit)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        crate::task::RuntimeFanoutProgress {
            total: self.items.len(),
            next_index: self.next_index as usize,
            limit: self.limit,
            initialized: true,
        }
        .validate(
            self.in_flight.iter().map(|flight| flight.index as usize),
            self.results.iter().map(Option::is_some),
        )
        .map_err(|_| FiberStateError::InvalidFrame)?;
        if self
            .items
            .iter()
            .enumerate()
            .any(|(index, value)| value.is_some() != (index >= self.next_index as usize))
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let mut free = std::collections::BTreeSet::new();
        resources
            .work()
            .collection(inputs.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        for input in inputs {
            if let AwbcProgramFanoutInput::Capture { position } = input {
                free.insert(*position as usize);
            }
        }
        let all_started = !self.items.is_empty() && self.next_index as usize == self.items.len();
        for position in free {
            let value = frame
                .registers
                .get(position)
                .ok_or(FiberStateError::InvalidFrame)?
                .as_ref();
            if value.is_some() == all_started {
                return Err(FiberStateError::InvalidFrame);
            }
            if let Some(value) = value {
                if self.items.len() > 1 {
                    view(value).admit_ownership(
                        crate::value::ownership::RuntimeValueOwnershipRequirement::Unrestricted,
                        resources,
                        0,
                    )?;
                }
            }
        }
        let item = match program
            .runtime_types
            .get(params[*source as usize].index())
            .map(|ty| ty.shape())
        {
            Some(
                crate::awbc::schema::AwbcRuntimeTypeShape::Sequence { item, .. }
                | crate::awbc::schema::AwbcRuntimeTypeShape::Array { item, .. },
            ) => *item,
            _ => return Err(FiberStateError::InvalidFrame),
        };
        for value in self.items.iter().flatten() {
            let value = view(value);
            value.admit_ownership(
                crate::value::ownership::RuntimeValueOwnershipRequirement::Owned,
                resources,
                0,
            )?;
            crate::program_types::RuntimeProgramValueType::Awbc { program, ty: item }
                .validate_admitted_view(value, frame.binding, resources)
                .map_err(|error| {
                    FiberStateError::Snapshot(
                        crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(error)),
                    )
                })?;
        }
        let child = program
            .task_plans
            .get(worker.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        for value in self.results.iter().flatten() {
            crate::program_types::RuntimeProgramValueType::Awbc {
                program,
                ty: child.payload_type(),
            }
            .validate_view(view(value), frame.binding, resources)
            .map_err(|error| {
                FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(
                    Box::new(error),
                ))
            })?;
        }
        for flight in &self.in_flight {
            resources
                .validate_view(RuntimeValueView::Need(need(&flight.handle)), 0, None)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        }
        Ok(())
    }

    pub(super) fn admit_original_source(
        &self,
        original: &crate::plan::task_source::RuntimeTaskSourceProgram,
        need: impl Fn(&N) -> crate::value::RuntimeNeedView<'_> + Copy,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> AwbcSaveResult<()> {
        let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
        let source = original
            .awbc_program_source(self.plan, limits, &mut admission.task_source_meter(limits)?)
            .map_err(
                |error| crate::value::AwbcRuntimeValueSnapshotError::Message {
                    message: error.to_string(),
                },
            )?;
        let Some(template) = source.request_endpoint().program_template() else {
            return Err(super::FiberStateError::InvalidFrame.into());
        };
        let crate::plan::RuntimeProgramTaskKernelRef::Fanout(fanout) = template.kernel() else {
            return Err(super::FiberStateError::InvalidFrame.into());
        };
        let RuntimeProgramOwner::Awbc(program) = original.program_owner() else {
            return Err(super::FiberStateError::InvalidFrame.into());
        };
        let crate::awbc::schema::AwbcTaskPlanExecution::Program(body) =
            &program.task_plans[self.plan.index()].execution
        else {
            return Err(super::FiberStateError::InvalidFrame.into());
        };
        let crate::awbc::schema::AwbcProgramTaskKernel::Fanout { worker, .. } = body.body else {
            return Err(super::FiberStateError::InvalidFrame.into());
        };
        let worker = original
            .awbc_program_source(worker, limits, &mut admission.task_source_meter(limits)?)
            .map_err(
                |error| crate::value::AwbcRuntimeValueSnapshotError::Message {
                    message: error.to_string(),
                },
            )?;
        let mut correlations = std::collections::BTreeSet::new();
        for flight in &self.in_flight {
            let handle = need(&flight.handle);
            if !correlations.insert(handle.correlation())
                || handle.producer().plan() != worker.task_plan().key()
                || handle.outcome() != &fanout.worker.producer.outcome()
            {
                return Err(super::FiberStateError::InvalidFrame.into());
            }
        }
        Ok(())
    }
}

/// Started items have moved into their worker; future items remain here.
/// Free parent captures move into the last worker, after earlier checked copies.
/// The aggregate itself is the enclosing Program task, not another Host request.
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberProgramFanoutState;
/// fn duplicate(state: &FiberProgramFanoutState) {
///     let _: FiberProgramFanoutState = state.clone();
/// }
/// ```
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: InertRuntimeValueStorage, N: InertFiberNeed",
        deserialize = "V: InertRuntimeValueStorage, N: InertFiberNeed"
    )
)]
pub struct FiberProgramFanoutState<V = RuntimeValue, N = RuntimeNeedHandle> {
    pub plan: AwbcTaskPlanId,
    pub binding: Option<AwbcPatternId>,
    pub limit: u32,
    pub items: Vec<Option<V>>,
    pub next_index: u32,
    pub in_flight: Vec<FiberAwaitManyInFlight<N>>,
    pub results: Vec<Option<V>>,
}

impl<V: InertRuntimeValueStorage + Clone, N: InertFiberNeed + Clone> Clone
    for FiberProgramFanoutState<V, N>
{
    fn clone(&self) -> Self {
        Self {
            plan: self.plan,
            binding: self.binding,
            limit: self.limit,
            items: self.items.clone(),
            next_index: self.next_index,
            in_flight: self.in_flight.clone(),
            results: self.results.clone(),
        }
    }
}

impl FiberProgramFanoutState {
    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<
        FiberProgramFanoutState<AwbcRuntimeValueSnapshot, RuntimeNeedHandleSaveSnapshot>,
    > {
        let value = |value: &RuntimeValue| {
            AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)
        };
        Ok(FiberProgramFanoutState {
            plan: self.plan,
            binding: self.binding,
            limit: self.limit,
            items: self
                .items
                .iter()
                .map(|v| v.as_ref().map(value).transpose())
                .collect::<Result<_, _>>()?,
            next_index: self.next_index,
            in_flight: self
                .in_flight
                .iter()
                .map(|child| {
                    Ok(FiberAwaitManyInFlight {
                        index: child.index,
                        handle: RuntimeNeedHandleSaveSnapshot::from_live(
                            &child.handle,
                            Some(owner),
                        )?,
                    })
                })
                .collect::<AwbcSaveResult<_>>()?,
            results: self
                .results
                .iter()
                .map(|v| v.as_ref().map(value).transpose())
                .collect::<Result<_, _>>()?,
        })
    }
}

impl FiberProgramFanoutState<AwbcRuntimeValueSnapshot, RuntimeNeedHandleSaveSnapshot> {
    pub(super) fn into_live_admitted<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> AwbcSaveResult<FiberProgramFanoutState> {
        let owner = owner.into();
        let value = |value: AwbcRuntimeValueSnapshot| value.into_admitted_runtime_value(owner);
        Ok(FiberProgramFanoutState {
            plan: self.plan,
            binding: self.binding,
            limit: self.limit,
            items: self
                .items
                .into_iter()
                .map(|v| v.map(value).transpose())
                .collect::<Result<_, _>>()?,
            next_index: self.next_index,
            in_flight: self
                .in_flight
                .into_iter()
                .map(|child| {
                    Ok(FiberAwaitManyInFlight {
                        index: child.index,
                        handle: child.handle.into_live_admitted(owner)?,
                    })
                })
                .collect::<AwbcSaveResult<_>>()?,
            results: self
                .results
                .into_iter()
                .map(|v| v.map(value).transpose())
                .collect::<Result<_, _>>()?,
        })
    }
}
