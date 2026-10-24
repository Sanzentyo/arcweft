//! Indexed stream custody admitted against its original executable table.

use super::{
    AwbcFiberRoot, AwbcProgram, AwbcSaveResult, AwbcStreamPlanId, FiberStateError,
    RuntimeProgramOwner,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::RuntimeProgramValueType;
use crate::stream::RuntimeStreamYieldCopyProof;
use crate::value::{AwbcRuntimeValueSnapshot, RuntimeValue, RuntimeValueView};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// One stream queue grammar. Only the inert carrier has a persistence codec.
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberStreamState;
/// let _: FiberStreamState = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberStreamState;
/// fn encode_live(stream: &FiberStreamState) { let _ = serde_json::to_vec(stream); }
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: super::InertRuntimeValueStorage",
        deserialize = "V: super::InertRuntimeValueStorage"
    )
)]
pub struct FiberStreamState<V = RuntimeValue> {
    pub plan: AwbcStreamPlanId,
    pub queue: Vec<V>,
    pub closed: bool,
    pub emitted_count: u64,
}

impl<V> FiberStreamState<V> {
    pub(super) fn validate_inventory(
        program: &AwbcProgram,
        root: AwbcFiberRoot,
        streams: &[Self],
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_> + Copy,
    ) -> Result<(), FiberStateError> {
        Self::validate_headers(program, root, streams, resources)?;
        for stream in streams {
            for value in &stream.queue {
                Self::validate_item(program, stream.plan, view(value), resources)?;
            }
        }
        Ok(())
    }

    fn validate_headers(
        program: &AwbcProgram,
        root: AwbcFiberRoot,
        streams: &[Self],
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .collection(streams.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let expected = if root == AwbcFiberRoot::Empty {
            0
        } else {
            program.stream_plans.len()
        };
        if streams.len() != expected {
            return Err(FiberStateError::InvalidFrame);
        }
        // Authenticate every header before examining any queue descendant.
        for (index, stream) in streams.iter().enumerate() {
            resources
                .work()
                .charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            if stream.plan.index() != index
                || u64::try_from(stream.queue.len()).map_or(true, |len| len > stream.emitted_count)
            {
                return Err(FiberStateError::InvalidFrame);
            }
            resources
                .work()
                .collection(stream.queue.len())
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        }
        Ok(())
    }

    fn validate_item(
        program: &AwbcProgram,
        plan: AwbcStreamPlanId,
        value: RuntimeValueView<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let plan = program
            .stream_plans
            .get(plan.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        RuntimeStreamYieldCopyProof::validate_inputs(value, resources)?;
        RuntimeProgramValueType::Awbc {
            program,
            ty: plan.item_type,
        }
        .validate_admitted_view(value, None, resources)
        .map_err(|error| {
            FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(
                Box::new(error),
            ))
        })
    }
}

impl FiberStreamState {
    /// All fallible checks precede the queue/observation copy and register move.
    pub(in crate::awbc) fn prepare_yield(
        &self,
        program: &AwbcProgram,
        value: &RuntimeValue,
    ) -> Result<u64, FiberStateError> {
        if self.closed
            || u64::try_from(self.queue.len()).map_or(true, |len| len > self.emitted_count)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let next = self
            .emitted_count
            .checked_add(1)
            .ok_or(FiberStateError::InvalidFrame)?;
        let count = self
            .queue
            .len()
            .checked_add(1)
            .ok_or(FiberStateError::InvalidFrame)?;
        let mut resources =
            ValueResourceAdmission::live(crate::entry::RuntimeSchemaLimits::engine_default());
        resources
            .work()
            .collection(count)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        for retained in &self.queue {
            Self::validate_item(program, self.plan, retained.view(), &mut resources)?;
        }
        Self::validate_item(program, self.plan, value.view(), &mut resources)?;
        Ok(next)
    }

    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberStreamState<AwbcRuntimeValueSnapshot>> {
        Ok(FiberStreamState {
            plan: self.plan,
            queue: self
                .queue
                .iter()
                .map(|value| AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner))
                .collect::<Result<_, _>>()?,
            closed: self.closed,
            emitted_count: self.emitted_count,
        })
    }
}

impl FiberStreamState<AwbcRuntimeValueSnapshot> {
    pub(super) fn into_live_admitted<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> AwbcSaveResult<FiberStreamState> {
        let owner = owner.into();
        Ok(FiberStreamState {
            plan: self.plan,
            queue: self
                .queue
                .into_iter()
                .map(|value| value.into_admitted_runtime_value(owner))
                .collect::<Result<_, _>>()?,
            closed: self.closed,
            emitted_count: self.emitted_count,
        })
    }
}

impl super::FiberState {
    pub(in crate::awbc) fn stream_state(
        &self,
        program: &AwbcProgram,
        plan: AwbcStreamPlanId,
    ) -> Result<&FiberStreamState, FiberStateError> {
        FiberStreamState::validate_headers(
            program,
            self.root,
            &self.streams,
            &mut ValueResourceAdmission::live(crate::entry::RuntimeSchemaLimits::engine_default()),
        )?;
        self.streams
            .get(plan.index())
            .filter(|state| state.plan == plan)
            .ok_or(FiberStateError::InvalidFrame)
    }
}
