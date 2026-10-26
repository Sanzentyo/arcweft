//! Borrowed values owned by suspended native waits and their status carrier.

use super::SuspendedValues;
use crate::engine::{AwaitManyState, AwaitState, FlowFiberStatus, WaitingManyStatus};
use crate::task::RuntimeNeedPublication;
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeNeedView, RuntimeRetainedValueAdmission};

#[cfg(test)]
mod many_tests;
#[cfg(test)]
mod tests;

fn invalid(message: &str) -> AwbcRuntimeValueSnapshotError {
    AwbcRuntimeValueSnapshotError::Message {
        message: message.into(),
    }
}

fn local_type(
    plan: &crate::plan::RuntimePlan,
    local: crate::runtime_id::RuntimeLocalDeclarationId,
    binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
) -> Result<crate::pattern::RuntimeSemanticTypeId, AwbcRuntimeValueSnapshotError> {
    let ty = plan
        .local_type_in_context(local, binding)
        .map_err(|error| invalid(&error.to_string()))?;
    plan.type_table()
        .get(ty)
        .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
        .ok_or_else(|| invalid("native AwaitMany local has an unknown original type"))
}

impl<H, P> SuspendedValues for AwaitState<H, RuntimeNeedPublication<P>>
where
    for<'a> RuntimeNeedView<'a>: From<&'a H>,
    P: crate::value::RuntimeStoredValue,
{
    fn admit_suspended_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        _binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.source_context
            .admit_retained(admission, P::admission_origin())?;
        admission.validate_metadata(&self.observed_through)?;
        admission.validate_metadata_collection(self.queued.len())?;
        let crate::engine::AwaitSource::Native(source) = &self.source else {
            return Err(invalid("native Await requires its original Plan operation"));
        };
        source.operation.admit_retained(admission)?;
        let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner().cloned()
        else {
            return Err(invalid(
                "native Await requires its original Plan executable",
            ));
        };
        let Some(crate::plan::FlowOp::Await { target, .. }) = source.operation.operation(&plan)
        else {
            return Err(invalid(
                "native Await source is not its original Await operation",
            ));
        };
        if !matches!(plan.type_table().get(target.source().ty()).map(crate::plan::RuntimePlanTypeDeclaration::projection),
            Some(crate::plan::RuntimePlanTypeProjection::Need(item)
                | crate::plan::RuntimePlanTypeProjection::ThreadHandle(item)) if *item == source.item_type)
        {
            return Err(invalid(
                "native Await item type differs from its original source",
            ));
        }
        let handle = RuntimeNeedView::from(&self.handle);
        handle.admit_retained(admission)?;
        let item = source.item_type;
        admission.validate_metadata(&item.get())?;
        let semantic = match admission.program_owner() {
            Some(crate::task::RuntimeProgramOwner::Plan(plan)) => plan
                .type_table()
                .get(item)
                .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity),
            _ => None,
        }
        .ok_or_else(|| invalid("native Await item type is absent from its original Plan"))?;
        if semantic != handle.outcome().payload_semantic_identity() {
            return Err(invalid(
                "native Await item type differs from its awaited outcome",
            ));
        }
        let mut previous = None;
        for publication in &self.queued {
            let cursor = publication.cursor();
            if previous.is_some_and(|previous: crate::task::TaskPublicationCursor| {
                previous.compare_same_source(cursor) != Some(std::cmp::Ordering::Less)
            }) {
                return Err(invalid("native Await queued publications are unordered"));
            }
            publication.admit_for_await(handle, admission)?;
            previous = Some(cursor);
        }
        if let (Some(queued), Some(observed)) = (previous, self.observed_through)
            && !matches!(
                queued.compare_same_source(observed),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            )
        {
            return Err(invalid("native Await queue exceeds its observed cursor"));
        }
        Ok(())
    }
}

impl<H, V, P> SuspendedValues for AwaitManyState<H, V, P>
where
    for<'a> RuntimeNeedView<'a>: From<&'a H>,
    V: crate::value::RuntimeStoredValue,
    P: crate::value::RuntimeStoredValue,
{
    fn admit_suspended_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        _binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.source_context
            .admit_retained(admission, V::admission_origin())?;
        let binding = self.source_context.instantiation();
        admission.validate_metadata(&(self.target.limit, self.next_index))?;
        admission.validate_metadata_collection(self.captured.len())?;
        admission.validate_metadata_collection(self.items.len())?;
        admission.validate_metadata_collection(self.in_flight.len())?;
        admission.validate_metadata_collection(self.results.len())?;
        admission.validate_metadata_collection(self.target.base.request.captures().len())?;
        admission.validate_metadata_collection(self.target.child.request.captures().len())?;
        admission.validate_metadata(&(self.target.base.outcome(), self.target.child.outcome()))?;
        if self.captured.len() != self.target.captures().count() {
            return Err(invalid(
                "native AwaitMany inventories disagree with its frontier",
            ));
        }
        for flight in &self.in_flight {
            admission.validate_metadata(&(flight.index, flight.observed_through))?;
        }
        crate::task::RuntimeFanoutProgress {
            total: self.items.len(),
            next_index: self.next_index,
            limit: self.target.limit,
            initialized: true,
        }
        .validate(
            self.in_flight.iter().map(|flight| flight.index),
            self.results.iter().map(Option::is_some),
        )
        .map_err(|error| invalid(&error.to_string()))?;
        let base = RuntimeNeedView::from(&self.base);
        base.admit_retained(admission)?;
        if base.outcome() != &self.target.base.outcome() {
            return Err(invalid(
                "native AwaitMany base outcome differs from its target",
            ));
        }
        let owner = admission
            .program_owner()
            .cloned()
            .ok_or_else(|| invalid("native AwaitMany requires its original Plan executable"))?;
        let crate::task::RuntimeProgramOwner::Plan(plan) = owner else {
            return Err(invalid(
                "native AwaitMany requires its original Plan executable",
            ));
        };
        let item = local_type(&plan, self.target.item_binding, binding)?;
        for value in &self.items {
            admission.validate_typed_borrowed_in_invocation(
                value.borrowed_value(),
                item,
                binding,
            )?;
        }
        for (local, value) in self.target.captures().zip(&self.captured) {
            let expected = local_type(&plan, local, binding)?;
            admission.validate_typed_borrowed_in_invocation(
                value.borrowed_value(),
                expected,
                binding,
            )?;
        }
        let mut correlations = std::collections::BTreeSet::new();
        for flight in &self.in_flight {
            let handle = RuntimeNeedView::from(&flight.handle);
            handle.admit_retained(admission)?;
            if !correlations.insert(handle.correlation())
                || handle.correlation().generation != base.correlation().generation
                || handle.outcome() != &self.target.child.outcome()
            {
                return Err(invalid(
                    "native AwaitMany in-flight child custody is inconsistent",
                ));
            }
        }
        for result in &self.results {
            if let Some(value) = result {
                admission.validate_owned_borrowed(value.borrowed_value())?;
                admission.admit_composite(P::admission_origin(), |owner, resources| {
                    self.target
                        .child
                        .outcome()
                        .validate_admitted_in_invocation(
                            value.borrowed_view(),
                            owner,
                            binding,
                            resources,
                        )?;
                    Ok(())
                })?;
            }
        }
        Ok(())
    }
}

impl<A, M> SuspendedValues for FlowFiberStatus<A, WaitingManyStatus<M>>
where
    A: SuspendedValues,
    M: SuspendedValues,
{
    fn admit_suspended_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        match self {
            Self::NeedWaiting(state) => state.admit_suspended_values(admission, binding),
            Self::WaitingMany(WaitingManyStatus::Native(state)) => {
                state.admit_suspended_values(admission, binding)
            }
            Self::WaitingMany(WaitingManyStatus::Observed(progress)) => {
                admission.validate_metadata(&(
                    progress.task,
                    progress.completed,
                    progress.total,
                ))?;
                if progress.completed > progress.total {
                    return Err(invalid("native AwaitMany progress exceeds its total"));
                }
                Ok(())
            }
            Self::Failed(message) => admission.validate_metadata_string(message),
            Self::Done(crate::engine::FlowExit::Return(value)) => {
                admission.validate_metadata_string(value)
            }
            _ => Ok(()),
        }
    }
}
