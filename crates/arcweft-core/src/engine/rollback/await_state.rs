//! Borrowed values owned by suspended native waits and their status carrier.

use super::SuspendedValues;
use crate::engine::{AwaitManyState, AwaitState, FlowFiberStatus, WaitingManyStatus};
use crate::plan::{RuntimeTaskPlanSealLimits, task_source::RuntimeTaskSourceProgram};
use crate::task::{RuntimeNeedPublication, RuntimeProgramOwner, RuntimeTaskTemplateArguments};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeNeedView, RuntimeRetainedValueAdmission,
    RuntimeValueSource,
};

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
    fn admit_frame_context<V: crate::value::RuntimeStoredValue, G>(
        &self,
        env: &crate::value::RuntimeEnv<V, G>,
        _admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if &self.source_context != env.source_function_context_ref() {
            return Err(invalid("native Await source differs from its owning frame"));
        }
        Ok(())
    }

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
        if let crate::plan::code::RuntimeCodeOwner::Function(site) = source.operation.owner()
            && !matches!(&self.source_context, crate::value::RuntimeScopeFunctionContext::Function { site: retained, .. } if *retained == site)
        {
            return Err(invalid(
                "native Await source differs from its invocation context",
            ));
        }
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
        if previous.is_some_and(|queued| {
            !self.observed_through.is_some_and(|observed| {
                matches!(
                    queued.compare_same_source(observed),
                    Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                )
            })
        }) {
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
    fn admit_frame_context<W: crate::value::RuntimeStoredValue, G>(
        &self,
        env: &crate::value::RuntimeEnv<W, G>,
        _admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if &self.source_context != env.source_function_context_ref()
            || matches!(self.source.owner(), crate::plan::code::RuntimeCodeOwner::Function(site)
                if env.function_contexts().next_back().map(|(active, _)| active) != Some(site))
        {
            return Err(invalid(
                "native AwaitMany source differs from its owning frame",
            ));
        }
        Ok(())
    }

    fn admit_suspended_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        _binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.source_context
            .admit_retained(admission, V::admission_origin())?;
        self.source.admit_retained(admission)?;
        let Some(RuntimeProgramOwner::Plan(plan)) = admission.program_owner().cloned() else {
            return Err(invalid(
                "native AwaitMany requires its original Plan executable",
            ));
        };
        let Some((_, target, _)) = self.source.await_many(&plan) else {
            return Err(invalid(
                "native AwaitMany source is not its original AwaitMany operation",
            ));
        };
        let binding = self.source_context.instantiation();
        admission.validate_metadata(&self.next_index)?;
        admission.validate_metadata_collection(self.captured.len())?;
        admission.validate_metadata_collection(self.items.len())?;
        admission.validate_metadata_collection(self.in_flight.len())?;
        admission.validate_metadata_collection(self.results.len())?;
        admission.validate_metadata_collection(target.base.request.captures().len())?;
        admission.validate_metadata_collection(target.child.request.captures().len())?;
        if self.captured.len() != target.captures().count() {
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
            limit: target.limit,
            initialized: true,
        }
        .validate(
            self.in_flight.iter().map(|flight| flight.index),
            self.results.iter().map(Option::is_some),
        )
        .map_err(|error| invalid(&error.to_string()))?;
        let base = RuntimeNeedView::from(&self.base);
        base.admit_retained(admission)?;
        if !base.requires_producer_custody() {
            return Err(invalid("native AwaitMany base requires an accepted launch"));
        }
        let item = local_type(&plan, target.item_binding, binding)?;
        for value in &self.items {
            admission.validate_typed_borrowed_in_invocation(
                value.borrowed_value(),
                item,
                binding,
            )?;
        }
        for (local, value) in target.captures().zip(&self.captured) {
            let expected = local_type(&plan, local, binding)?;
            admission.validate_typed_borrowed_in_invocation(
                value.borrowed_value(),
                expected,
                binding,
            )?;
        }
        let limits = RuntimeTaskPlanSealLimits::default();
        let mut meter = admission.task_source_meter(limits)?;
        let program = RuntimeTaskSourceProgram::native(plan.clone());
        let base_source = plan
            .task_source(&target.base.definition.plan, limits, &mut meter)
            .map_err(|error| invalid(&error.to_string()))?;
        let checked_base = base_source
            .template_producer_spec_after_value_admission(
                &program,
                RuntimeTaskTemplateArguments::AwaitManyBaseItems {
                    captures: &self.captured,
                    items: &self.items,
                },
                binding,
                admission,
            )
            .map_err(|error| invalid(&error.to_string()))?;
        base_source
            .validate_template_handle(&checked_base, base, admission)
            .map_err(|error| invalid(&error.to_string()))?;
        let child_source = plan
            .task_source(&target.child.definition.plan, limits, &mut meter)
            .map_err(|error| invalid(&error.to_string()))?;
        let mut correlations = std::collections::BTreeSet::new();
        for flight in &self.in_flight {
            let handle = RuntimeNeedView::from(&flight.handle);
            handle.admit_retained(admission)?;
            if !correlations.insert(handle.correlation())
                || handle.correlation().generation != base.correlation().generation
                || !handle.requires_producer_custody()
            {
                return Err(invalid(
                    "native AwaitMany in-flight child custody is inconsistent",
                ));
            }
            let index = u32::try_from(flight.index).map_err(|_| {
                invalid("native AwaitMany in-flight index exceeds its task argument domain")
            })?;
            let item = self.items[flight.index].borrowed_view();
            let checked = child_source
                .template_producer_spec_after_value_admission(
                    &program,
                    RuntimeTaskTemplateArguments::AwaitManyChild {
                        captures: &self.captured,
                        index,
                        item: RuntimeValueSource::Borrowed {
                            value: &item,
                            origin: V::admission_origin(),
                        },
                    },
                    binding,
                    admission,
                )
                .map_err(|error| invalid(&error.to_string()))?;
            child_source
                .validate_template_handle(&checked, handle, admission)
                .map_err(|error| invalid(&error.to_string()))?;
        }
        for result in &self.results {
            if let Some(value) = result {
                admission.validate_owned_borrowed(value.borrowed_value())?;
                admission.admit_composite(P::admission_origin(), |owner, resources| {
                    target.child.outcome().validate_admitted_in_invocation(
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

impl<H, V, P> SuspendedValues for crate::engine::ProgramFanoutState<H, V, P>
where
    for<'a> RuntimeNeedView<'a>: From<&'a H>,
    V: crate::value::RuntimeStoredValue,
    P: crate::value::RuntimeStoredValue,
{
    fn admit_frame_context<W: crate::value::RuntimeStoredValue, G>(
        &self,
        env: &crate::value::RuntimeEnv<W, G>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if &self.context != env.source_function_context_ref() {
            return Err(invalid(
                "Program fanout source differs from its owning frame",
            ));
        }
        let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
            return Err(invalid("Program fanout has no original Plan frame"));
        };
        let Some(crate::plan::FlowOp::ProgramTask { template, .. }) = self.source.operation(plan)
        else {
            return Err(invalid("Program fanout has no original task operation"));
        };
        let crate::plan::RuntimeProgramTaskBody::Fanout(fanout) = &template.body else {
            return Err(invalid("Program fanout has no original kernel"));
        };
        if !matches!(env.get_stored(fanout.limit).map(W::borrowed_view),
            Some(crate::value::RuntimeValueView::Scalar(crate::value::RuntimeScalarView::UInt(
                crate::value::RuntimeUInt::U32(limit)))) if limit == self.limit)
            || env.get_stored(fanout.source).is_some()
        {
            return Err(invalid(
                "Program fanout control inputs differ from its original frame",
            ));
        }
        let all_started = !self.items.is_empty() && self.next_index == self.items.len();
        for capture in &fanout.worker.captures {
            if capture.local != fanout.item
                && env.get_stored(capture.local).is_some() == all_started
            {
                return Err(invalid(
                    "Program fanout capture custody differs from its original frame",
                ));
            }
        }
        Ok(())
    }

    fn admit_suspended_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        _binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.context
            .admit_retained(admission, V::admission_origin())?;
        self.source.admit_retained(admission)?;
        admission.validate_metadata(&(self.limit, self.next_index))?;
        for count in [self.items.len(), self.results.len(), self.in_flight.len()] {
            admission.validate_metadata_collection(count)?;
        }
        crate::task::RuntimeFanoutProgress {
            total: self.items.len(),
            next_index: self.next_index,
            limit: self.limit,
            initialized: true,
        }
        .validate(
            self.in_flight.iter().map(|flight| flight.index),
            self.results.iter().map(Option::is_some),
        )
        .map_err(|error| invalid(&error.to_string()))?;
        if self
            .items
            .iter()
            .enumerate()
            .any(|(index, item)| item.is_some() != (index >= self.next_index))
        {
            return Err(invalid(
                "Program fanout item custody differs from its started frontier",
            ));
        }
        let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner().cloned()
        else {
            return Err(invalid(
                "Program fanout requires its original Plan executable",
            ));
        };
        let Some(crate::plan::FlowOp::ProgramTask { template, .. }) = self.source.operation(&plan)
        else {
            return Err(invalid("Program fanout lost its original task operation"));
        };
        let crate::plan::RuntimeProgramTaskBody::Fanout(fanout) = &template.body else {
            return Err(invalid("Program fanout lost its original Fanout kernel"));
        };
        let base = RuntimeNeedView::from(&self.base);
        base.admit_retained(admission)?;
        if !base.requires_producer_custody()
            || base.outcome() != &template.producer.outcome()
            || plan.task_plan(&template.producer.plan).map(|row| row.key())
                != Some(base.producer().plan())
        {
            return Err(invalid(
                "Program fanout base differs from its original task",
            ));
        }
        let binding = self.context.instantiation();
        let item = local_type(&plan, fanout.item, binding)?;
        for value in self.items.iter().flatten() {
            admission.validate_owned_typed_borrowed_in_invocation(
                value.borrowed_value(),
                item,
                binding,
            )?;
        }
        let mut correlations = std::collections::BTreeSet::new();
        for flight in &self.in_flight {
            admission.validate_metadata(&(flight.index, flight.observed_through))?;
            let handle = RuntimeNeedView::from(&flight.handle);
            handle.admit_retained(admission)?;
            if !correlations.insert(handle.correlation())
                || handle.correlation().generation != base.correlation().generation
                || !handle.requires_producer_custody()
                || flight.observed_through.is_some()
                || handle.outcome() != &fanout.worker.producer.outcome()
                || plan
                    .task_plan(&fanout.worker.producer.plan)
                    .map(|row| row.key())
                    != Some(handle.producer().plan())
            {
                return Err(invalid(
                    "Program fanout child differs from its original worker",
                ));
            }
        }
        for value in self.results.iter().flatten() {
            admission.validate_owned_borrowed(value.borrowed_value())?;
            admission.admit_composite(P::admission_origin(), |owner, resources| {
                fanout
                    .worker
                    .producer
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
        Ok(())
    }
}

impl SuspendedValues for crate::engine::ChoiceState {
    fn admit_suspended_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        _binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.admit_native(admission)
    }

    fn admit_frame_context<V: crate::value::RuntimeStoredValue, G>(
        &self,
        env: &crate::value::RuntimeEnv<V, G>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.continuation()
            .ok_or_else(|| invalid("observed Choice has no Native continuation"))?
            .admit_frame_context(env, admission)
    }
}

impl SuspendedValues for crate::engine::choice::NativeChoiceContinuation {
    fn admit_suspended_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        _binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.admit_retained(admission)
    }

    fn admit_frame_context<V: crate::value::RuntimeStoredValue, G>(
        &self,
        env: &crate::value::RuntimeEnv<V, G>,
        _admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.operation.admit_invocation(env)
    }
}

impl SuspendedValues for crate::engine::HostCallState {
    fn admit_suspended_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.continuation()
            .ok_or_else(crate::engine::host_call::invalid_host_call)?
            .admit_suspended_values(admission, binding)
    }

    fn admit_frame_context<V: crate::value::RuntimeStoredValue, G>(
        &self,
        env: &crate::value::RuntimeEnv<V, G>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.continuation()
            .ok_or_else(crate::engine::host_call::invalid_host_call)?
            .admit_frame_context(env, admission)
    }
}

impl SuspendedValues for crate::engine::host_call::NativeHostCallContinuation {
    fn admit_suspended_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        _binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.admit_retained(admission)
    }

    fn admit_frame_context<V: crate::value::RuntimeStoredValue, G>(
        &self,
        env: &crate::value::RuntimeEnv<V, G>,
        _admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.operation.admit_invocation(env)
    }
}

impl<A, M, P, H, C> SuspendedValues for FlowFiberStatus<A, WaitingManyStatus<M, P>, H, C>
where
    A: SuspendedValues,
    M: SuspendedValues,
    P: SuspendedValues,
    H: SuspendedValues,
    C: SuspendedValues,
{
    fn admit_frame_context<V: crate::value::RuntimeStoredValue, G>(
        &self,
        env: &crate::value::RuntimeEnv<V, G>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        match self {
            Self::Choice(state) => state.admit_frame_context(env, admission)?,
            Self::HostCall(state) => state.admit_frame_context(env, admission)?,
            Self::NeedWaiting(state) => state.admit_frame_context(env, admission)?,
            Self::WaitingMany(WaitingManyStatus::Program(state)) => {
                state.admit_frame_context(env, admission)?
            }
            Self::WaitingMany(WaitingManyStatus::Native(state)) => {
                state.admit_frame_context(env, admission)?
            }
            _ => {}
        }
        if let Self::ActionWaiting(state) = self {
            let crate::engine::ActionWaitingStatus::Native(state) = state else {
                return Err(invalid(
                    "native Action wait requires its original Plan operation",
                ));
            };
            let payload = crate::engine::NativeActionReceiveState::admit_frame(
                &state.source,
                env,
                admission,
            )?;
            if payload != state.payload {
                return Err(invalid(
                    "Action receive source differs from its retained original channel",
                ));
            }
        }
        Ok(())
    }

    fn admit_suspended_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        match self {
            Self::Choice(state) => state.admit_suspended_values(admission, binding),
            Self::HostCall(state) => state.admit_suspended_values(admission, binding),
            Self::Dialogue(activation) => admission.validate_metadata(activation),
            Self::DurationWaiting(wait) => admission.validate_metadata(wait),
            Self::ActionWaiting(state) => {
                let crate::engine::ActionWaitingStatus::Native(state) = state else {
                    return Err(invalid(
                        "native Action wait requires its original Plan operation",
                    ));
                };
                state.source.admit_retained(admission)?;
                admission.validate_metadata(&state.payload)
            }
            Self::NeedWaiting(state) => state.admit_suspended_values(admission, binding),
            Self::WaitingMany(WaitingManyStatus::Native(state)) => {
                state.admit_suspended_values(admission, binding)
            }
            Self::WaitingMany(WaitingManyStatus::Program(state)) => {
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
