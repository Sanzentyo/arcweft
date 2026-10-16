//! One `AwaitMany` progress grammar with borrowed original plan admission.

use super::input::FrameValueContext;
use super::{
    AwbcPatternId, AwbcTaskPlanId, FiberStateError, InertFiberNeed, InertRuntimeValueStorage,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::RuntimeProgramValueType;
use crate::task::{RuntimeNeedHandle, RuntimeNeedHandleSaveSnapshot, RuntimeProgramOwner};
use crate::value::{
    AwbcRuntimeValueSnapshot, RuntimeNeedView, RuntimeScalarView, RuntimeTupleView, RuntimeValue,
    RuntimeValueView,
};
use serde::{Deserialize, Serialize};

/// All launched, pending and completed item ownership for one occurrence.
/// Only inert carriers have a save codec.
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberAwaitManyState;
/// let _: FiberAwaitManyState = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberAwaitManyState;
/// fn encode_live(state: &FiberAwaitManyState) { let _ = serde_json::to_vec(state); }
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: InertRuntimeValueStorage, N: InertFiberNeed",
        deserialize = "V: InertRuntimeValueStorage, N: InertFiberNeed"
    )
)]
pub struct FiberAwaitManyState<V = RuntimeValue, N = RuntimeNeedHandle> {
    pub plan: AwbcTaskPlanId,
    pub binding: Option<AwbcPatternId>,
    pub base: Option<N>,
    pub captured: Vec<V>,
    pub items: Vec<V>,
    pub next_index: u32,
    pub in_flight: Vec<FiberAwaitManyInFlight<N>>,
    pub results: Vec<Option<V>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(serialize = "N: InertFiberNeed", deserialize = "N: InertFiberNeed")
)]
pub struct FiberAwaitManyInFlight<N = RuntimeNeedHandle> {
    pub index: u32,
    pub handle: N,
}

impl<V, N> FiberAwaitManyState<V, N> {
    pub(super) fn validate_site(
        &self,
        frame: &FrameValueContext<'_, V>,
        cursor: super::FiberCursor,
        resume: super::FiberResumeTarget,
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let super::FiberResumeTarget::Declared(resume) = resume else {
            return Err(FiberStateError::InvalidFrame);
        };
        let point = frame
            .program
            .resume_points
            .get(resume.index())
            .ok_or(FiberStateError::UnknownResumePoint(resume.0))?;
        let function = frame
            .program
            .functions
            .get(frame.function.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        if point.function != frame.function
            || point.frame_layout != function.frame_layout
            || cursor.function != frame.function
        {
            return Err(FiberStateError::InvalidFrame);
        }
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let block = frame
            .program
            .blocks
            .get(cursor.block.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let crate::awbc::schema::AwbcTerminator::AwaitMany {
            plan,
            binding,
            resume: declared_resume,
            ..
        } = block.terminator
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        if block.owner != frame.function
            || cursor.instruction_offset != block.instructions.len
            || plan != self.plan
            || binding != self.binding
            || declared_resume != resume
        {
            return Err(FiberStateError::InvalidFrame);
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one progress admission checks index partition, original captures and request/result types, and base/child producer derivation before any value projection"
    )]
    pub(super) fn validate_inputs(
        &self,
        frame: &FrameValueContext<'_, V>,
        cursor: super::FiberCursor,
        resume: super::FiberResumeTarget,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_> + Copy,
        need: impl Fn(&N) -> RuntimeNeedView<'_> + Copy,
    ) -> Result<(), FiberStateError> {
        use crate::awbc::schema::AwbcTaskPlanKind;
        self.validate_site(frame, cursor, resume, resources)?;
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        for count in [
            self.captured.len(),
            self.items.len(),
            self.in_flight.len(),
            self.results.len(),
        ] {
            resources
                .work()
                .collection(count)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        }
        let program = frame.program;
        let plan = program
            .task_plans
            .get(self.plan.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let AwbcTaskPlanKind::AwaitMany {
            child,
            limit,
            captures,
            request_function,
            ..
        } = &plan.kind
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        let child_plan = program
            .task_plans
            .get(child.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let AwbcTaskPlanKind::Template {
            request_function: child_request,
            ..
        } = child_plan.kind
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        let request = program
            .functions
            .get(request_function.index())
            .and_then(|f| program.signatures.get(f.signature.index()))
            .ok_or(FiberStateError::InvalidFrame)?;
        let child_inputs = program
            .functions
            .get(child_request.index())
            .and_then(|f| program.signatures.get(f.signature.index()))
            .ok_or(FiberStateError::InvalidFrame)?;
        let item_type = child_inputs
            .params
            .last()
            .copied()
            .ok_or(FiberStateError::InvalidFrame)?;
        if *limit == 0
            || u32::try_from(self.items.len()).is_err()
            || self.captured.len() != captures.len()
            || request.params.len() != captures.len()
            || child_inputs.params.len()
                != captures
                    .len()
                    .checked_add(1)
                    .ok_or(FiberStateError::InvalidFrame)?
            || self.results.len() != self.items.len()
            || self.next_index as usize > self.items.len()
            || self.in_flight.len() > *limit as usize
            || self
                .binding
                .is_some_and(|p| program.patterns.get(p.index()).is_none())
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let mut pending = std::collections::BTreeSet::new();
        for child in &self.in_flight {
            resources
                .work()
                .charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            if child.index >= self.next_index || !pending.insert(child.index as usize) {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        if self.base.is_none()
            && (self.next_index != 0
                || !self.in_flight.is_empty()
                || self.results.iter().any(Option::is_some))
        {
            return Err(FiberStateError::InvalidFrame);
        }
        for (index, result) in self.results.iter().enumerate() {
            resources
                .work()
                .charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            if self.base.is_some()
                && !(if index < self.next_index as usize {
                    result.is_some() != pending.contains(&index)
                } else {
                    result.is_none() && !pending.contains(&index)
                })
            {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        for (ordinal, ((value, register), expected)) in self
            .captured
            .iter()
            .zip(captures)
            .zip(&request.params)
            .enumerate()
        {
            let value = view(value);
            value.admit_ownership(
                crate::value::ownership::RuntimeValueOwnershipRequirement::Unrestricted,
                resources,
                0,
            )?;
            for ty in [
                frame
                    .layout
                    .slots
                    .get(register.index())
                    .ok_or(FiberStateError::InvalidFrame)?
                    .ty,
                *expected,
                child_inputs.params[ordinal],
            ] {
                RuntimeProgramValueType::Awbc { program, ty }
                    .validate_admitted_view(value, frame.binding, resources)
                    .map_err(|error| {
                        FiberStateError::Snapshot(
                            crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(
                                error,
                            )),
                        )
                    })?;
            }
        }
        for value in &self.items {
            let value = view(value);
            value.admit_ownership(
                crate::value::ownership::RuntimeValueOwnershipRequirement::Unrestricted,
                resources,
                0,
            )?;
            RuntimeProgramValueType::Awbc {
                program,
                ty: item_type,
            }
            .validate_admitted_view(value, frame.binding, resources)
            .map_err(|error| {
                FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(
                    Box::new(error),
                ))
            })?;
        }
        for value in self.results.iter().flatten() {
            RuntimeProgramValueType::Awbc {
                program,
                ty: child_plan.payload_type,
            }
            .validate_view(view(value), frame.binding, resources)
            .map_err(|error| {
                FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(
                    Box::new(error),
                ))
            })?;
        }
        // Views compose the original template arguments; no live aggregate,
        // request clone or speculative TaskSpec is constructed by validation.
        let captured = self.captured.iter().map(view).collect::<Vec<_>>();
        let items = self.items.iter().map(view).collect::<Vec<_>>();
        // Template arguments use the existing producer key transcript limit;
        // its virtual tuple headers are separate from retained-value bytes.
        let max_bytes = 16 * 1024 * 1024;
        if let Some(base) = &self.base {
            let base = need(base);
            resources
                .validate_view(RuntimeValueView::Need(base), 0, None)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            let fields = [
                RuntimeValueView::Tuple(RuntimeTupleView::Views(&captured)),
                RuntimeValueView::Tuple(RuntimeTupleView::Views(&items)),
            ];
            plan.validate_template_handle(
                program,
                RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields)),
                base,
                max_bytes,
            )
            .map_err(|_| FiberStateError::InvalidFrame)?;
            for child in &self.in_flight {
                let handle = need(&child.handle);
                resources
                    .validate_view(RuntimeValueView::Need(handle), 0, None)
                    .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                let fields = [
                    RuntimeValueView::Tuple(RuntimeTupleView::Views(&captured)),
                    RuntimeValueView::Scalar(RuntimeScalarView::UInt(
                        crate::value::RuntimeUInt::U32(child.index),
                    )),
                    items[child.index as usize],
                ];
                child_plan
                    .validate_template_handle(
                        program,
                        RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields)),
                        handle,
                        max_bytes,
                    )
                    .map_err(|_| FiberStateError::InvalidFrame)?;
            }
        }
        Ok(())
    }
}

impl FiberAwaitManyState {
    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> super::AwbcSaveResult<
        FiberAwaitManyState<AwbcRuntimeValueSnapshot, RuntimeNeedHandleSaveSnapshot>,
    > {
        let value = |value: &RuntimeValue| {
            AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)
        };
        Ok(FiberAwaitManyState {
            plan: self.plan,
            binding: self.binding,
            base: self
                .base
                .as_ref()
                .map(|h| RuntimeNeedHandleSaveSnapshot::from_live(h, Some(owner)))
                .transpose()?,
            captured: self.captured.iter().map(value).collect::<Result<_, _>>()?,
            items: self.items.iter().map(value).collect::<Result<_, _>>()?,
            next_index: self.next_index,
            in_flight: self
                .in_flight
                .iter()
                .map(|c| {
                    Ok(FiberAwaitManyInFlight {
                        index: c.index,
                        handle: RuntimeNeedHandleSaveSnapshot::from_live(&c.handle, Some(owner))?,
                    })
                })
                .collect::<super::AwbcSaveResult<_>>()?,
            results: self
                .results
                .iter()
                .map(|v| v.as_ref().map(value).transpose())
                .collect::<Result<_, _>>()?,
        })
    }
}

impl FiberAwaitManyState<AwbcRuntimeValueSnapshot, RuntimeNeedHandleSaveSnapshot> {
    pub(super) fn into_live_admitted(
        self,
        owner: &RuntimeProgramOwner,
    ) -> super::AwbcSaveResult<FiberAwaitManyState> {
        let value = |value: AwbcRuntimeValueSnapshot| value.into_admitted_runtime_value(owner);
        Ok(FiberAwaitManyState {
            plan: self.plan,
            binding: self.binding,
            base: self.base.map(|h| h.into_live(owner)).transpose()?,
            captured: self
                .captured
                .into_iter()
                .map(value)
                .collect::<Result<_, _>>()?,
            items: self
                .items
                .into_iter()
                .map(value)
                .collect::<Result<_, _>>()?,
            next_index: self.next_index,
            in_flight: self
                .in_flight
                .into_iter()
                .map(|c| {
                    Ok(FiberAwaitManyInFlight {
                        index: c.index,
                        handle: c.handle.into_live(owner)?,
                    })
                })
                .collect::<super::AwbcSaveResult<_>>()?,
            results: self
                .results
                .into_iter()
                .map(|v| v.map(value).transpose())
                .collect::<Result<_, _>>()?,
        })
    }
}
