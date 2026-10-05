use super::{
    AudioCommandEnvelope, AudioDispatchId, AwbcAwaitObserverResume, AwbcEffectPlanId,
    AwbcFunctionId, AwbcHostCallId, AwbcHostCallMode, AwbcProductStepExecutor, AwbcProgram,
    AwbcResumePointId, AwbcTrapCode, FiberAwaitManyInFlight, FiberAwaitTarget, FiberState,
    FiberSuspensionReason, FlowEvent, MappedEffect, NeedId, PendingHostCall, ProductStepError,
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeHostCallMode,
    RuntimeHostCallRequest, RuntimeNeedState, RuntimePayload, RuntimeStepOutput,
    RuntimeStreamEvent, RuntimeValue, TaskEvent, TaskEventKind, TaskId, TaskSequence,
    VmObservation, content_request, resolved_runtime_need_state, runtime_sequence_values,
    runtime_value_label, stream_id_for,
};
use crate::awbc::vm::cancel_fiber;
use crate::stream::StreamEventKind;
use crate::task::{NamedHostArg, NeedProducerRuntimeArgument};
use crate::value::runtime_value_into_sequence_values;
use arcweft_need::Need;
use std::cmp::Ordering;

fn observed_ready_payload(value: &RuntimePayload) -> Option<RuntimePayload> {
    value
        .value()
        .ownership()
        .permits_copy()
        .then(|| value.clone())
}

enum AwaitNeedPublicationKind {
    InfrastructureFailure(crate::task::RuntimeTaskFailure),
    NotStarted,
    Pending(arcweft_need::Progress),
    LocalReady,
    ExternalReady,
    ReadyTransferred,
    Cancelled,
}

enum DeferredSuspensionDispatch {
    BudgetYield,
    AwaitNeed {
        correlation: crate::task::TaskCorrelation,
        item_type: crate::awbc::schema::AwbcTypeId,
        binding: Option<crate::awbc::schema::AwbcPatternId>,
        observer: Option<AwbcAwaitObserverResume>,
    },
    AwaitMany,
    HostCall {
        call: AwbcHostCallId,
        destination: Option<crate::awbc::schema::AwbcRegisterId>,
    },
    Unsupported,
}

impl DeferredSuspensionDispatch {
    fn from_reason(reason: &FiberSuspensionReason) -> Self {
        match reason {
            FiberSuspensionReason::BudgetYield => Self::BudgetYield,
            FiberSuspensionReason::Await {
                target:
                    FiberAwaitTarget::Need {
                        need, item_type, ..
                    },
                binding,
                observer,
            } => Self::AwaitNeed {
                correlation: need.correlation(),
                item_type: *item_type,
                binding: *binding,
                observer: *observer,
            },
            FiberSuspensionReason::AwaitMany(_) => Self::AwaitMany,
            FiberSuspensionReason::HostCall {
                call, destination, ..
            } => Self::HostCall {
                call: *call,
                destination: *destination,
            },
            FiberSuspensionReason::Dialogue { .. } | FiberSuspensionReason::Choice { .. } => {
                Self::Unsupported
            }
        }
    }
}

impl AwbcProductStepExecutor {
    pub(super) fn emit_pending_need_reensure(&mut self, output: &mut RuntimeStepOutput) -> bool {
        let pending = self
            .need_producers
            .pending_reensure()
            .map(|launch| launch.need().clone())
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return true;
        }

        let mut proofs = Vec::with_capacity(pending.len());
        for need in &pending {
            match self.need_producers.inspect_task_ensured(need) {
                Ok(proof) => proofs.push(proof),
                Err(error) => {
                    self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                    return false;
                }
            }
        }
        if proofs.iter().all(|proof| proof.task_spec().is_none()) {
            self.fail_with_error(
                ProductStepError::Internal(
                    "Restartable Need re-registration produced no task specification".to_owned(),
                ),
                output,
            );
            return false;
        }
        let requests = proofs
            .into_iter()
            .filter_map(|proof| self.need_producers.mark_task_ensured_prepared(proof))
            .collect::<Vec<_>>();
        output.requests.tasks.extend(requests);
        true
    }

    pub(super) fn latch_task_events(
        &mut self,
        events: Vec<TaskEvent>,
        output: &mut RuntimeStepOutput,
    ) {
        for event in events {
            if let Err(error) = event.inspect_host_ready_ownership() {
                self.fail_with_trap(
                    AwbcTrapCode::HostAbiMismatch,
                    error.to_string(),
                    None,
                    output,
                );
                // A rejected host packet never becomes a runtime owner.
                continue;
            }
            let local_launch = self
                .need_producers
                .launches()
                .find(|launch| launch.task() == &event.correlation.task_id);
            if let Some(launch) = local_launch {
                if launch.generation() != event.correlation.generation {
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        "task event generation differs from the accepted Need producer launch"
                            .to_owned(),
                        None,
                        output,
                    );
                    continue;
                }
                if let TaskEventKind::Ready(value) = &event.kind {
                    let item_type =
                        self.awbc_type_id_for_semantic_identity(launch.plan().payload_type());
                    if item_type.is_none_or(|item_type| {
                        !crate::awbc::fiber::runtime_value_matches_type(
                            &self.program,
                            value.value(),
                            item_type,
                            0,
                        )
                    }) {
                        self.fail_with_trap(
                            AwbcTrapCode::HostAbiMismatch,
                            "Need producer Ready payload is outside its checked item type"
                                .to_owned(),
                            None,
                            output,
                        );
                        continue;
                    }
                }
            }
            let event = match self.need_producers.publish_task_event_owned(event) {
                Ok(crate::task::NeedProducerOwnedTaskEventDisposition::Published)
                | Ok(crate::task::NeedProducerOwnedTaskEventDisposition::Duplicate(_)) => {
                    continue;
                }
                Ok(crate::task::NeedProducerOwnedTaskEventDisposition::NotLocal(event)) => event,
                Err(error) => {
                    let (reason, rejected_event) = error.into_parts();
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        reason.to_string(),
                        None,
                        output,
                    );
                    // An invalid host packet never enters the runtime owner
                    // graph; its sole inbound carrier is rejected here.
                    drop(rejected_event);
                    continue;
                }
            };
            let cursor = crate::task::TaskPublicationCursor::from_event(&event);
            if let Some(observed) = self.task_publications.get(&event.correlation.task_id) {
                match cursor.compare_same_source(*observed) {
                    Some(Ordering::Greater) => {}
                    Some(Ordering::Equal | Ordering::Less) => {
                        if matches!(
                            &event.kind,
                            TaskEventKind::Ready(value)
                                if !value.value().ownership().permits_copy()
                        ) {
                            self.fail_with_trap(
                                AwbcTrapCode::HostAbiMismatch,
                                "duplicate affine task Ready publication is not admissible"
                                    .to_owned(),
                                None,
                                output,
                            );
                        }
                        continue;
                    }
                    None => {
                        self.fail_with_trap(
                            AwbcTrapCode::InternalInvariant,
                            "task publication cursor changed source for one task".to_owned(),
                            None,
                            output,
                        );
                        continue;
                    }
                }
            }
            self.task_publications
                .insert(event.correlation.task_id.clone(), cursor);
            self.queued_task_events.push_back(event);
        }
    }

    pub(super) fn resume_need(
        &mut self,
        correlation: &crate::task::TaskCorrelation,
        item_type: crate::awbc::schema::AwbcTypeId,
        binding: Option<crate::awbc::schema::AwbcPatternId>,
        observer: Option<AwbcAwaitObserverResume>,
        resume: AwbcResumePointId,
        states: &mut Vec<RuntimeNeedState>,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let need = &correlation.need;
        let local_launch = self.need_producers.launch_for_correlation(correlation);
        let (cursor, state) = if let Some(launch) = local_launch {
            if states.iter().any(|state| &state.correlation == correlation) {
                self.fail_with_trap(
                    AwbcTrapCode::InternalInvariant,
                    "external Need state attempts to publish a Product-owned Need".to_owned(),
                    None,
                    output,
                );
                return true;
            }
            if let Some(fault) = launch.task_fault() {
                self.fail_with_trap(
                    AwbcTrapCode::HostAbiMismatch,
                    format!("Need producer task failed: {fault}"),
                    None,
                    output,
                );
                return true;
            }
            let Some(cursor) = launch.publication() else {
                return false;
            };
            let state = match launch.state() {
                crate::task::RuntimeNeedProducerState::NotStarted => {
                    AwaitNeedPublicationKind::NotStarted
                }
                crate::task::RuntimeNeedProducerState::Pending(progress) => {
                    AwaitNeedPublicationKind::Pending(progress.clone())
                }
                crate::task::RuntimeNeedProducerState::Ready(_) => {
                    AwaitNeedPublicationKind::LocalReady
                }
                crate::task::RuntimeNeedProducerState::InfrastructureFailure(failure) => {
                    AwaitNeedPublicationKind::InfrastructureFailure(failure.clone())
                }
                crate::task::RuntimeNeedProducerState::ReadyTransferred => {
                    AwaitNeedPublicationKind::ReadyTransferred
                }
                crate::task::RuntimeNeedProducerState::Cancelled => {
                    AwaitNeedPublicationKind::Cancelled
                }
            };
            (cursor, state)
        } else {
            let Some(state) = resolved_runtime_need_state(states, correlation) else {
                return false;
            };
            let Some(cursor) = state.cursor else {
                return false;
            };
            let kind = match state.state() {
                Need::NotStarted => AwaitNeedPublicationKind::NotStarted,
                Need::Pending(progress) => AwaitNeedPublicationKind::Pending(progress.clone()),
                Need::Ready(crate::task::RuntimeNeedOutcome::Value(_)) => {
                    AwaitNeedPublicationKind::ExternalReady
                }
                Need::Ready(crate::task::RuntimeNeedOutcome::InfrastructureFailure(failure)) => {
                    AwaitNeedPublicationKind::InfrastructureFailure(failure.clone())
                }
                Need::Cancelled => AwaitNeedPublicationKind::Cancelled,
            };
            (cursor, kind)
        };
        let waiter = crate::runtime_id::RuntimePersistentFiberId::from_allocated(
            self.fiber.instance.get().get(),
        );
        let publication_key = (waiter, *correlation);
        if let Some(observed) = self.need_publications.get(&publication_key) {
            match cursor.compare_same_source(*observed) {
                Some(Ordering::Greater) => {}
                Some(Ordering::Equal | Ordering::Less) => return false,
                None => {
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        "Need publication cursor changed source for one Await".to_owned(),
                        None,
                        output,
                    );
                    return true;
                }
            }
        }
        self.need_publications.insert(publication_key, cursor);
        match state {
            AwaitNeedPublicationKind::InfrastructureFailure(failure) => {
                self.fail_with_trap(
                    AwbcTrapCode::HostAbiMismatch,
                    failure.to_string(),
                    None,
                    output,
                );
                true
            }
            AwaitNeedPublicationKind::NotStarted => false,
            AwaitNeedPublicationKind::Pending(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need: need.clone(),
                    progress: progress.clone(),
                });
                observer.is_some_and(|observer| {
                    self.resume_await_progress(observer, progress.clone(), output)
                })
            }
            kind @ (AwaitNeedPublicationKind::LocalReady
            | AwaitNeedPublicationKind::ExternalReady) => {
                let local = matches!(kind, AwaitNeedPublicationKind::LocalReady);
                let selected = if local {
                    self.need_producers
                        .ready_for_correlation(correlation)
                        .map(|value| (value, None))
                } else {
                    resolved_runtime_need_state(states, correlation).and_then(|state| {
                        let index = states
                            .iter()
                            .position(|candidate| std::ptr::eq(candidate, state))?;
                        match state.state() {
                            Need::Ready(crate::task::RuntimeNeedOutcome::Value(value)) => {
                                Some((value, state.cursor.map(|cursor| (index, cursor))))
                            }
                            _ => None,
                        }
                    })
                };
                let Some((value, _external)) = selected else {
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        "Need Ready publication lost its selected payload owner".to_owned(),
                        None,
                        output,
                    );
                    return true;
                };
                if !crate::awbc::fiber::runtime_value_matches_type(
                    &self.program,
                    value.value(),
                    item_type,
                    0,
                ) {
                    self.fail_with_trap(
                        AwbcTrapCode::HostAbiMismatch,
                        format!(
                            "Need {} published a Ready payload outside its checked item type",
                            need
                        ),
                        None,
                        output,
                    );
                    return true;
                }
                self.resume_need_ready_owned(correlation, binding, resume, states, local, output)
            }
            AwaitNeedPublicationKind::ReadyTransferred => {
                self.fail_with_trap(
                    AwbcTrapCode::HostAbiMismatch,
                    "Need Ready payload has already transferred to its unique waiter".to_owned(),
                    None,
                    output,
                );
                true
            }
            AwaitNeedPublicationKind::Cancelled => {
                let cancellation = cancel_fiber(&mut self.fiber);
                self.consume_observations(cancellation.observations, output);
                true
            }
        }
    }

    fn resume_await_progress(
        &mut self,
        observer: AwbcAwaitObserverResume,
        progress: arcweft_need::Progress,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let value = RuntimeValue::Progress(progress);
        if let Ok(frame) = self.fiber.active_frame_mut()
            && let Err(error) = frame.set_register(observer.destination, value)
        {
            self.fail_with_trap(AwbcTrapCode::TypeMismatch, error.to_string(), None, output);
            return true;
        }
        match self
            .fiber
            .resume_await_observer_at(&self.program, observer.resume)
        {
            Ok(()) => true,
            Err(error) => {
                self.fail_with_trap(
                    AwbcTrapCode::InternalInvariant,
                    error.to_string(),
                    None,
                    output,
                );
                false
            }
        }
    }

    fn resume_need_ready_owned(
        &mut self,
        correlation: &crate::task::TaskCorrelation,
        binding: Option<crate::awbc::schema::AwbcPatternId>,
        resume: AwbcResumePointId,
        states: &mut Vec<RuntimeNeedState>,
        local: bool,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let need = &correlation.need;
        let selected = if local {
            self.need_producers
                .ready_for_correlation(correlation)
                .map(|value| (value, None))
        } else {
            resolved_runtime_need_state(states, correlation).and_then(|state| match state.state() {
                Need::Ready(crate::task::RuntimeNeedOutcome::Value(value)) => {
                    Some((value, state.cursor.map(|cursor| cursor)))
                }
                _ => None,
            })
        };
        let Some((value, sequence)) = selected else {
            self.fail_with_trap(
                AwbcTrapCode::InternalInvariant,
                "selected Need Ready payload is unavailable".to_owned(),
                None,
                output,
            );
            return true;
        };
        let local_take = if local {
            match self
                .need_producers
                .inspect_ready_take_for_correlation(correlation)
            {
                Ok(proof) => Some(proof),
                Err(error) => {
                    self.fail_with_trap(
                        AwbcTrapCode::InternalInvariant,
                        error.to_string(),
                        None,
                        output,
                    );
                    return true;
                }
            }
        } else {
            None
        };
        let observed = observed_ready_payload(value);
        let copyable = value.value().ownership().permits_copy();
        let prepared_resume = match self.fiber.validate_resume_at(&self.program, resume) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_with_trap(
                    AwbcTrapCode::InternalInvariant,
                    error.to_string(),
                    None,
                    output,
                );
                return true;
            }
        };
        let prepared_binding = if let Some(pattern) = binding {
            let prepared = match crate::awbc::vm::prepare_pattern_binding(
                &self.program,
                &self.fiber,
                pattern,
                value.value(),
            ) {
                Ok(prepared) => prepared,
                Err(error) => {
                    self.fail_with_trap(
                        AwbcTrapCode::PatternMismatch,
                        error.to_string(),
                        None,
                        output,
                    );
                    return true;
                }
            };
            let vacant = self.fiber.active_frame().ok().is_some_and(|frame| {
                prepared.registers().iter().all(|register| {
                    frame
                        .registers
                        .get(register.index())
                        .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
                })
            });
            if !vacant {
                self.fail_with_trap(
                    AwbcTrapCode::TypeMismatch,
                    "Need Ready binding destination is occupied".to_owned(),
                    None,
                    output,
                );
                return true;
            }
            Some(prepared)
        } else {
            None
        };
        let payload = if let Some(proof) = local_take {
            Ok(self.need_producers.take_ready_for_need_prepared(proof))
        } else if copyable {
            Ok(value.clone())
        } else {
            sequence
                .and_then(|sequence| super::take_runtime_need_state(states, correlation, sequence))
                .and_then(|(_, state)| match state.into_parts().2 {
                    Need::Ready(crate::task::RuntimeNeedOutcome::Value(value)) => Some(value),
                    _ => None,
                })
                .ok_or_else(|| "selected external Need Ready owner is unavailable".to_owned())
        };
        let payload = match payload {
            Ok(payload) => payload,
            Err(error) => {
                self.fail_with_trap(AwbcTrapCode::InternalInvariant, error, None, output);
                return true;
            }
        };
        if let Some(binding) = prepared_binding {
            crate::awbc::vm::bind_pattern_owned_prepared(
                &self.program,
                &mut self.fiber,
                binding,
                payload.into_value(),
            );
        }
        self.fiber.resume_at_prepared(prepared_resume);
        output.flow_events.push(FlowEvent::AwaitReady {
            need: need.clone(),
            value: observed,
        });
        true
    }

    fn task_payload_accepts(
        &self,
        plan: crate::awbc::schema::AwbcTaskPlanId,
        value: &RuntimeValue,
    ) -> bool {
        self.program
            .task_plans
            .get(plan.index())
            .is_some_and(|record| {
                self.program
                    .validate_live_value(
                        record.payload_type,
                        value,
                        crate::entry::RuntimeSchemaLimits::engine_default(),
                    )
                    .is_ok()
            })
    }

    fn await_many_child_plan(
        &self,
        plan: crate::awbc::schema::AwbcTaskPlanId,
    ) -> crate::awbc::schema::AwbcTaskPlanId {
        match self.program.task_plans[plan.index()].kind {
            crate::awbc::schema::AwbcTaskPlanKind::AwaitMany { child, .. } => child,
            _ => unreachable!("verified AwaitMany state owns an aggregate plan"),
        }
    }

    pub(super) fn fill_await_many(
        &mut self,
        output: &mut RuntimeStepOutput,
        backend: &mut impl crate::pure::RuntimeCallBackend,
    ) {
        let Some(suspension) = self.fiber.suspension.as_mut() else {
            return;
        };
        let reason = std::mem::replace(&mut suspension.reason, FiberSuspensionReason::BudgetYield);
        let FiberSuspensionReason::AwaitMany(mut state) = reason else {
            suspension.reason = reason;
            return;
        };
        let result = self.fill_await_many_state(&mut state, output, backend);
        if let Some(suspension) = self.fiber.suspension.as_mut() {
            suspension.reason = FiberSuspensionReason::AwaitMany(state);
        }
        if let Err(error) = result {
            self.fail_with_error(error, output);
        }
    }

    fn fill_await_many_state(
        &mut self,
        state: &mut crate::awbc::fiber::FiberAwaitManyState,
        output: &mut RuntimeStepOutput,
        backend: &mut impl crate::pure::RuntimeCallBackend,
    ) -> Result<(), ProductStepError> {
        use crate::awbc::schema::AwbcTaskPlanKind;
        use crate::value::{RuntimeTupleView, RuntimeValueView};
        let record = self
            .program
            .task_plans
            .get(state.plan.index())
            .ok_or_else(|| ProductStepError::Internal("AwaitMany plan is absent".into()))?;
        let AwbcTaskPlanKind::AwaitMany {
            child,
            limit,
            request_function,
            ..
        } = record.kind
        else {
            return Err(ProductStepError::Internal(
                "AwaitMany suspension has the wrong plan kind".into(),
            ));
        };
        let child = child;
        let limit = limit as usize;
        if limit == 0
            || state.results.len() != state.items.len()
            || u32::try_from(state.items.len()).is_err()
        {
            return Err(ProductStepError::Input(
                "AwaitMany source count, limit or result partition is invalid".into(),
            ));
        }
        if state.base.is_none() {
            let args = self.evaluate_task_request(
                state.plan,
                request_function,
                &state.captured,
                None,
                backend,
            )?;
            let request = super::mapping::task_request(&self.program, state.plan, args)?;
            let fields = [
                RuntimeValueView::Tuple(RuntimeTupleView::Values(&state.captured)),
                RuntimeValueView::Tuple(RuntimeTupleView::Values(&state.items)),
            ];
            let spec = self.program.task_plans[state.plan.index()]
                .instantiate_template(
                    &self.program,
                    self.runtime_generation,
                    RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields)),
                    request,
                    16 * 1024 * 1024,
                )
                .map_err(ProductStepError::Input)?;
            let accepted = self
                .need_producers
                .ensure_task(spec)
                .map_err(|error| ProductStepError::Input(error.to_string()))?;
            let (spec, handle) = accepted.into_parts();
            state.base = Some(
                crate::task::RuntimeNeedHandle::try_from_accepted_launch(spec, handle)
                    .map_err(|error| ProductStepError::Input(error.to_string()))?,
            );
        }
        let child_record = self.program.task_plans.get(child.index()).ok_or_else(|| {
            ProductStepError::Internal("AwaitMany child template is absent".into())
        })?;
        let AwbcTaskPlanKind::Template {
            request_function, ..
        } = child_record.kind
        else {
            return Err(ProductStepError::Internal(
                "AwaitMany child has the wrong template kind".into(),
            ));
        };
        while state.in_flight.len() < limit && (state.next_index as usize) < state.items.len() {
            if self.remaining_new_task_requests == 0 {
                break;
            }
            let index = state.next_index;
            let next_index = index
                .checked_add(1)
                .ok_or_else(|| ProductStepError::Input("AwaitMany index exceeds u32".into()))?;
            let item = &state.items[index as usize];
            let args = self.evaluate_task_request(
                child,
                request_function,
                &state.captured,
                Some(item),
                backend,
            )?;
            let request = super::mapping::task_request(&self.program, child, args)?;
            let index_value = RuntimeValue::u32(index);
            let fields = [
                RuntimeValueView::Tuple(RuntimeTupleView::Values(&state.captured)),
                index_value.view(),
                item.view(),
            ];
            let spec = self.program.task_plans[child.index()]
                .instantiate_template(
                    &self.program,
                    self.runtime_generation,
                    RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields)),
                    request,
                    16 * 1024 * 1024,
                )
                .map_err(ProductStepError::Input)?;
            let accepted = self
                .need_producers
                .ensure_task(spec)
                .map_err(|error| ProductStepError::Input(error.to_string()))?;
            let handle = crate::task::RuntimeNeedHandle::try_from_accepted_launch(
                accepted.spec().clone(),
                accepted.handle(),
            )
            .map_err(|error| ProductStepError::Input(error.to_string()))?;
            output.flow_events.push(FlowEvent::AwaitStarted {
                need: handle.need_id(),
                task: Some(handle.correlation().task_id),
            });
            output.requests.tasks.push(accepted);
            state
                .in_flight
                .push(FiberAwaitManyInFlight { index, handle });
            state.next_index = next_index;
            self.remaining_new_task_requests -= 1;
        }
        if state.in_flight.is_empty() && (state.next_index as usize) < state.items.len() {
            return Err(ProductStepError::Input(
                "AwaitMany fan-out exceeds this step's task request quota".into(),
            ));
        }
        Ok(())
    }

    fn evaluate_task_request(
        &mut self,
        plan: crate::awbc::schema::AwbcTaskPlanId,
        function: AwbcFunctionId,
        captured: &[RuntimeValue],
        item: Option<&RuntimeValue>,
        backend: &mut impl crate::pure::RuntimeCallBackend,
    ) -> Result<Vec<RuntimeValue>, ProductStepError> {
        if captured
            .iter()
            .chain(item)
            .any(|value| !value.ownership().permits_copy())
        {
            return Err(ProductStepError::Type(
                "task request kernel inputs require deep Copy ownership".into(),
            ));
        }
        let mut inputs = captured.to_vec();
        if let Some(item) = item {
            inputs.push(item.clone());
        }
        let value = super::run_function(
            &self.program,
            function,
            inputs,
            backend,
            &mut self.compact_pure_stats,
        )
        .map_err(ProductStepError::Vm)?;
        let RuntimeValue::Tuple(args) = value else {
            return Err(ProductStepError::Type(
                "task request kernel must return its argument tuple".into(),
            ));
        };
        let record = &self.program.task_plans[plan.index()];
        let signature = self
            .program
            .signatures
            .get(record.signature.index())
            .ok_or_else(|| ProductStepError::Internal("task signature is absent".into()))?;
        if signature.params.len() != args.len() {
            return Err(ProductStepError::Type(
                "task request kernel arity mismatch".into(),
            ));
        }
        for (ty, value) in signature.params.iter().zip(&args) {
            self.program
                .validate_live_value(
                    *ty,
                    value,
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                )
                .map_err(|error| ProductStepError::Type(error.to_string()))?;
        }
        Ok(args)
    }
    pub(super) fn resume_await_many(
        &mut self,
        mut state: crate::awbc::fiber::FiberAwaitManyState,
        resume: AwbcResumePointId,
        before_handles: std::collections::BTreeMap<
            crate::runtime_id::RuntimeLineHandleToken,
            crate::value::ownership::RuntimeOwnedSlotId,
        >,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl crate::pure::RuntimeCallBackend,
    ) -> bool {
        if state.results.len() != state.items.len() {
            if let Some(suspension) = self.fiber.suspension.as_mut() {
                suspension.reason = FiberSuspensionReason::AwaitMany(state);
            }
            self.record_error(
                ProductStepError::Internal(
                    "AwaitMany result slots disagree with the admitted item count".to_owned(),
                ),
                output,
            );
            return false;
        }
        let in_flight_tasks = state
            .in_flight
            .iter()
            .map(|in_flight| in_flight.handle.correlation().task_id)
            .collect::<std::collections::BTreeSet<_>>();
        let events = self.take_await_many_task_events(&in_flight_tasks);
        let mut progressed = false;
        let mut accepted_ready = false;
        for event in events {
            let Some(position) = state.in_flight.iter().position(|in_flight| {
                in_flight.handle.correlation().task_id == event.correlation.task_id
            }) else {
                continue;
            };
            match event.kind {
                TaskEventKind::Ready(value) => {
                    if !self
                        .task_payload_accepts(self.await_many_child_plan(state.plan), value.value())
                    {
                        if !self.discard_extracted_await_many(
                            state,
                            &before_handles,
                            crate::effect::RuntimeDropPolicy::Default,
                            output,
                        ) {
                            return false;
                        }
                        self.fail_with_trap(
                            AwbcTrapCode::HostAbiMismatch,
                            format!(
                                "await task {} published a payload outside its checked outcome contract",
                                event.correlation.task_id
                            ),
                            None,
                            output,
                        );
                        return true;
                    }
                    let in_flight = state.in_flight.remove(position);
                    let observation = observed_ready_payload(&value);
                    state.results[in_flight.index as usize] = Some(value.into_value());
                    output.flow_events.push(FlowEvent::AwaitReady {
                        need: in_flight.handle.need_id(),
                        value: observation,
                    });
                    progressed = true;
                    accepted_ready = true;
                }
                TaskEventKind::Progress(progress) => {
                    output.flow_events.push(FlowEvent::AwaitProgress {
                        need: state.in_flight[position].handle.need_id(),
                        progress,
                    });
                    progressed = true;
                }
                TaskEventKind::InfrastructureFailure(error) => {
                    let message = format!(
                        "await task {} at index {} failed: {}",
                        event.correlation.task_id, state.in_flight[position].index, error
                    );
                    if !self.discard_extracted_await_many(
                        state,
                        &before_handles,
                        crate::effect::RuntimeDropPolicy::Default,
                        output,
                    ) {
                        return false;
                    }
                    self.fail_with_trap(AwbcTrapCode::HostAbiMismatch, message, None, output);
                    return true;
                }
                TaskEventKind::Cancelled => {
                    if !self.discard_extracted_await_many(
                        state,
                        &before_handles,
                        crate::effect::RuntimeDropPolicy::Cancel,
                        output,
                    ) {
                        return false;
                    }
                    let cancellation = cancel_fiber(&mut self.fiber);
                    self.consume_observations(cancellation.observations, output);
                    return true;
                }
            }
        }
        if state.in_flight.is_empty()
            && state.next_index as usize >= state.items.len()
            && state.results.iter().all(Option::is_some)
        {
            if accepted_ready {
                if let Some(suspension) = self.fiber.suspension.as_mut() {
                    suspension.reason = FiberSuspensionReason::AwaitMany(state);
                }
                return true;
            }
            let values = state
                .results
                .iter_mut()
                .map(|slot| {
                    slot.take()
                        .expect("completed AwaitMany result remains present")
                })
                .collect::<Vec<_>>();
            let value = runtime_sequence_values(values);
            let prepared = (|| -> Result<_, ProductStepError> {
                let need = state
                    .base
                    .as_ref()
                    .ok_or_else(|| {
                        ProductStepError::Internal("AwaitMany has no accepted base".into())
                    })?
                    .need_id();
                let resume = self
                    .fiber
                    .validate_resume_at(&self.program, resume)
                    .map_err(ProductStepError::Fiber)?;
                let frame = self.fiber.active_frame().map_err(ProductStepError::Fiber)?;
                let binding = if let Some(pattern) = state.binding {
                    let prepared = crate::awbc::vm::prepare_pattern_binding(
                        &self.program,
                        &self.fiber,
                        pattern,
                        &value,
                    )
                    .map_err(|error| ProductStepError::Internal(error.to_string()))?;
                    if prepared.registers().iter().any(|register| {
                        !frame
                            .registers
                            .get(register.index())
                            .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
                    }) {
                        return Err(ProductStepError::Internal(
                            "AwaitMany binding destination register is occupied".to_owned(),
                        ));
                    }
                    Some(prepared)
                } else {
                    None
                };
                let mut after = super::line::without_await_many_slots(
                    &before_handles,
                    self.fiber.instance,
                    state.plan,
                );
                if let Some(pattern) = state.binding {
                    for handle in super::line::unique_line_handles(&value)? {
                        let token = handle.token().clone();
                        let destinations = crate::awbc::vm::pattern_handle_destinations(
                            &self.program,
                            &self.fiber,
                            pattern,
                            &value,
                            &token,
                        )
                        .map_err(|error| ProductStepError::Internal(error.to_string()))?;
                        if let Some(register) = destinations.first().copied() {
                            after.insert(
                                token,
                                crate::value::ownership::RuntimeOwnedSlotId::AwbcRegister {
                                    execution: self.facade_fiber.execution,
                                    fiber: self.fiber.instance,
                                    frame: frame.instance,
                                    register,
                                },
                            );
                        }
                    }
                }
                let reconciliation = self.dialogues.inspect_parent_fiber_reconciliation(
                    self.facade_fiber.execution,
                    &before_handles,
                    &after,
                    &crate::line_task::RuntimeHandleDropAuthorization::at_boundary(Some(
                        crate::effect::RuntimeDropPolicy::Default,
                    )),
                )?;
                Ok((need, resume, binding, reconciliation))
            })();
            let (need, prepared_resume, binding, reconciliation) = match prepared {
                Ok(prepared) => prepared,
                Err(error) => {
                    let dropped = super::line::without_await_many_slots(
                        &before_handles,
                        self.fiber.instance,
                        state.plan,
                    );
                    if let Ok(reconciliation) = self.dialogues.inspect_parent_fiber_reconciliation(
                        self.facade_fiber.execution,
                        &before_handles,
                        &dropped,
                        &crate::line_task::RuntimeHandleDropAuthorization::at_boundary(Some(
                            crate::effect::RuntimeDropPolicy::Default,
                        )),
                    ) {
                        let receipt = self
                            .dialogues
                            .commit_parent_fiber_reconciliation(reconciliation);
                        output
                            .requests
                            .line_commands
                            .extend(receipt.into_commands());
                        self.fail_with_error(error, output);
                        return true;
                    }
                    state.results = runtime_value_into_sequence_values(value)
                        .expect("owned AwaitMany aggregate remains a sequence")
                        .into_iter()
                        .map(Some)
                        .collect();
                    if let Some(suspension) = self.fiber.suspension.as_mut() {
                        suspension.reason = FiberSuspensionReason::AwaitMany(state);
                    }
                    self.record_error(error, output);
                    return false;
                }
            };
            let observation = value
                .ownership()
                .permits_copy()
                .then(|| RuntimePayload::from(value.clone()));
            if let Some(binding) = binding {
                crate::awbc::vm::bind_pattern_owned_prepared(
                    &self.program,
                    &mut self.fiber,
                    binding,
                    value,
                );
            }
            self.fiber.resume_at_prepared(prepared_resume);
            let receipt = self
                .dialogues
                .commit_parent_fiber_reconciliation(reconciliation);
            output
                .requests
                .line_commands
                .extend(receipt.into_commands());
            output.flow_events.push(FlowEvent::AwaitReady {
                need,
                value: observation,
            });
            return true;
        }
        if let Some(suspension) = self.fiber.suspension.as_mut() {
            suspension.reason = FiberSuspensionReason::AwaitMany(state);
        }
        self.fill_await_many(output, pure_backend);
        progressed || !output.requests.tasks.is_empty()
    }

    /// An AwaitMany source may have been moved out of the suspension while
    /// host events are inspected. Reconcile its typed slots before a trap or
    /// cancellation clears the runtime owner graph.
    fn discard_extracted_await_many(
        &mut self,
        state: crate::awbc::fiber::FiberAwaitManyState,
        before: &std::collections::BTreeMap<
            crate::runtime_id::RuntimeLineHandleToken,
            crate::value::ownership::RuntimeOwnedSlotId,
        >,
        policy: crate::effect::RuntimeDropPolicy,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let after = super::line::without_await_many_slots(before, self.fiber.instance, state.plan);
        let prepared = match self.dialogues.inspect_parent_fiber_reconciliation(
            self.facade_fiber.execution,
            before,
            &after,
            &crate::line_task::RuntimeHandleDropAuthorization::at_boundary(Some(policy)),
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                if let Some(suspension) = self.fiber.suspension.as_mut() {
                    suspension.reason = FiberSuspensionReason::AwaitMany(state);
                }
                self.record_error(error.into(), output);
                return false;
            }
        };
        let receipt = self.dialogues.commit_parent_fiber_reconciliation(prepared);
        output
            .requests
            .line_commands
            .extend(receipt.into_commands());
        true
    }

    /// The ordinary trap path can still own an installed AwaitMany packet.
    /// Its item/result slots must be released through the parent ledger before
    /// FiberState clears the suspension.
    pub(super) fn release_installed_await_many_for_trap(
        &mut self,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        let Some(FiberSuspensionReason::AwaitMany(state)) = self
            .fiber
            .suspension
            .as_ref()
            .map(|suspension| &suspension.reason)
        else {
            return true;
        };
        let before = match super::line::product_fiber_handle_owners(
            self.facade_fiber.execution,
            &self.fiber,
        ) {
            Ok(before) => before,
            Err(error) => {
                self.record_error(error, output);
                return false;
            }
        };
        let after = super::line::without_await_many_slots(&before, self.fiber.instance, state.plan);
        let prepared = match self.dialogues.inspect_parent_fiber_reconciliation(
            self.facade_fiber.execution,
            &before,
            &after,
            &crate::line_task::RuntimeHandleDropAuthorization::at_boundary(Some(
                crate::effect::RuntimeDropPolicy::Default,
            )),
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.record_error(error.into(), output);
                return false;
            }
        };
        let receipt = self.dialogues.commit_parent_fiber_reconciliation(prepared);
        output
            .requests
            .line_commands
            .extend(receipt.into_commands());
        true
    }

    pub(super) fn emit_host_call(
        &mut self,
        call: AwbcHostCallId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        if args.iter().any(|value| !value.ownership().permits_copy()) {
            self.record_error(
                ProductStepError::Type(
                    "external host-call arguments require deep Copy carriers".to_owned(),
                ),
                output,
            );
            return;
        }
        if self
            .pending_host_call
            .as_ref()
            .is_some_and(|pending| pending.call == call)
        {
            return;
        }
        let Some(record) = self.program.host_calls.get(call.index()) else {
            self.record_error(
                ProductStepError::Internal(format!("missing AWBC host call {}", call.0)),
                output,
            );
            return;
        };
        let Some(signature) = self.program.signatures.get(record.signature.index()) else {
            self.record_error(
                ProductStepError::Internal(format!("AWBC host call {} has no signature", call.0)),
                output,
            );
            return;
        };
        let result =
            match signature.result {
                Some(result) => self.program.runtime_types.get(result.index()),
                None => self.program.runtime_types.iter().find(|ty| {
                    matches!(ty.shape(), crate::awbc::schema::AwbcRuntimeTypeShape::Unit)
                }),
            };
        let Some(result) = result else {
            self.record_error(
                ProductStepError::Internal(format!(
                    "AWBC host call {} result type is absent from the selected program",
                    call.0
                )),
                output,
            );
            return;
        };
        let public_id = self
            .program
            .strings
            .get(record.public_id.index())
            .cloned()
            .unwrap_or_else(|| format!("awbc.host_call.{}", call.0));
        let sequence = self.next_host_call_sequence;
        self.next_host_call_sequence = self.next_host_call_sequence.saturating_add(1);
        let id = RuntimeHostCallId(if sequence == 0 {
            public_id.clone()
        } else {
            format!("{public_id}.{sequence}")
        });
        let mut positional = Vec::new();
        let mut named_args = Vec::new();
        for (descriptor, value) in record.arguments.iter().zip(args) {
            if descriptor.spread {
                let Ok(values) = runtime_value_into_sequence_values(value.clone()) else {
                    self.record_error(
                        ProductStepError::Host(format!(
                            "spread host argument requires a tuple or bracket sequence, found {}",
                            runtime_value_label(value)
                        )),
                        output,
                    );
                    return;
                };
                positional.extend(values.into_iter().map(RuntimePayload::from));
            } else if let Some(name) = descriptor.name {
                let name = self
                    .program
                    .strings
                    .get(name.index())
                    .cloned()
                    .unwrap_or_else(|| format!("argument.{}", name.0));
                named_args.push(NamedHostArg {
                    name,
                    value: RuntimePayload::from(value.clone()),
                });
            } else {
                positional.push(RuntimePayload::from(value.clone()));
            }
        }
        let request = crate::task::HostTaskRequest::Custom {
            capability: crate::task::HostCapabilityId(
                self.program.strings[record.capability.index()].clone(),
            ),
            operation: self.program.strings[record.operation.index()].clone(),
            manifest_contract: record.contract,
            args: positional,
            named_args,
        };
        let request = match RuntimeHostCallRequest::admit(
            id.clone(),
            record.producer,
            self.runtime_generation,
            request,
            result.semantic_identity(),
            match record.mode {
                AwbcHostCallMode::Immediate => RuntimeHostCallMode::Immediate,
                AwbcHostCallMode::Suspend => RuntimeHostCallMode::Suspend,
            },
            record.deterministic,
            &mut self.need_producers,
        ) {
            Ok(request) => request,
            Err(error) => {
                self.record_error(ProductStepError::Host(error.to_string()), output);
                return;
            }
        };
        output.requests.host_calls.push(request);
        self.pending_host_call = Some(PendingHostCall { call, id });
    }

    pub(super) fn resume_host_call(
        &mut self,
        call: AwbcHostCallId,
        destination: Option<crate::awbc::schema::AwbcRegisterId>,
        resume: AwbcResumePointId,
        results: &mut Vec<crate::step::RuntimeHostCallResult>,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if self.pending_host_call.is_none() {
            let Some(args) = self.fiber.suspension.as_mut().and_then(|suspension| {
                match &mut suspension.reason {
                    FiberSuspensionReason::HostCall { args, .. } => Some(std::mem::take(args)),
                    _ => None,
                }
            }) else {
                self.fail_with_trap(
                    AwbcTrapCode::InternalInvariant,
                    "host-call suspension lost its argument owner".to_owned(),
                    None,
                    output,
                );
                return true;
            };
            self.emit_host_call(call, &args, output);
            if let Some(FiberSuspensionReason::HostCall { args: retained, .. }) = self
                .fiber
                .suspension
                .as_mut()
                .map(|suspension| &mut suspension.reason)
            {
                *retained = args;
            }
        }
        let Some(pending) = self.pending_host_call.clone() else {
            return false;
        };
        let Some(index) = results.iter().position(|result| result.id == pending.id) else {
            return false;
        };
        if pending.call != call {
            self.pending_host_call = None;
            self.fail_with_trap(
                AwbcTrapCode::HostAbiMismatch,
                "pending host call does not match the selected continuation".to_owned(),
                None,
                output,
            );
            return true;
        }
        match &results[index].outcome {
            Ok(value) => {
                let checked = self
                    .program
                    .host_calls
                    .get(call.index())
                    .and_then(|record| self.program.signatures.get(record.signature.index()))
                    .is_some_and(|signature| match signature.result {
                        Some(ty) => self
                            .program
                            .validate_live_value(
                                ty,
                                value.value(),
                                crate::entry::RuntimeSchemaLimits::engine_default(),
                            )
                            .is_ok(),
                        None => value.value() == &crate::value::RuntimeValue::Unit,
                    });
                if !checked {
                    self.pending_host_call = None;
                    self.fail_with_trap(
                        AwbcTrapCode::HostAbiMismatch,
                        "host-call result does not satisfy the selected program type".to_owned(),
                        None,
                        output,
                    );
                    return true;
                }
                let prepared_resume = match self.fiber.validate_resume_at(&self.program, resume) {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                        return true;
                    }
                };
                if let Some(destination) = destination {
                    let vacant = self.fiber.active_frame().ok().is_some_and(|frame| {
                        frame
                            .registers
                            .get(destination.index())
                            .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
                    });
                    if !vacant {
                        self.fail_with_trap(
                            AwbcTrapCode::HostAbiMismatch,
                            "host-call destination register is unavailable".to_owned(),
                            None,
                            output,
                        );
                        return true;
                    }
                }
                let result = results.remove(index);
                let value = result
                    .outcome
                    .expect("validated host-call result remains successful")
                    .into_value();
                if let Some(destination) = destination {
                    self.fiber
                        .active_frame_mut()
                        .expect("validated host-call frame remains active")
                        .set_register(destination, value)
                        .expect("validated host-call destination remains available");
                }
                self.pending_host_call = None;
                self.fiber.resume_at_prepared(prepared_resume);
                true
            }
            Err(error) => {
                let error = error.clone();
                results.remove(index);
                self.pending_host_call = None;
                self.fail_with_trap(
                    match error.kind {
                        crate::step::RuntimeHostCallErrorKind::UnsupportedCapability => {
                            AwbcTrapCode::CapabilityDenied
                        }
                        crate::step::RuntimeHostCallErrorKind::Rejected
                        | crate::step::RuntimeHostCallErrorKind::Failed => {
                            AwbcTrapCode::HostAbiMismatch
                        }
                    },
                    error.message.clone(),
                    None,
                    output,
                );
                true
            }
        }
    }

    pub(super) fn initialize_deferred_child_suspension(
        &mut self,
        child: &mut super::ProductChildFiber,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl crate::pure::RuntimeCallBackend,
    ) -> Result<(), ProductStepError> {
        let Some(suspension) = child.fiber.suspension.as_ref() else {
            return Ok(());
        };
        let dispatch = DeferredSuspensionDispatch::from_reason(&suspension.reason);
        if let DeferredSuspensionDispatch::AwaitNeed {
            correlation: id, ..
        } = &dispatch
        {
            let task = self
                .need_producers
                .launch_for_correlation(id)
                .map(|launch| launch.task().clone());
            output.flow_events.push(FlowEvent::AwaitStarted {
                need: id.need,
                task,
            });
        }
        match dispatch {
            DeferredSuspensionDispatch::AwaitNeed { .. }
            | DeferredSuspensionDispatch::BudgetYield => {}
            DeferredSuspensionDispatch::AwaitMany => {
                let diagnostics = output.diagnostics.len();
                self.fill_await_many_for_fiber(&mut child.fiber, output, pure_backend);
                if output.diagnostics.len() > diagnostics {
                    return Err(ProductStepError::Internal(
                        output.diagnostics.last().map_or_else(
                            || "deferred await-many request failed".to_owned(),
                            |d| d.message.clone(),
                        ),
                    ));
                }
            }
            DeferredSuspensionDispatch::HostCall { call, .. } => {
                if child.pending_host_call.is_none() {
                    let Some(args) = child.fiber.suspension.as_ref().and_then(|suspension| {
                        match &suspension.reason {
                            FiberSuspensionReason::HostCall { args, .. } => Some(args.as_slice()),
                            _ => None,
                        }
                    }) else {
                        return Err(ProductStepError::Internal(
                            "deferred child host-call suspension lost its argument owner"
                                .to_owned(),
                        ));
                    };
                    if args.iter().any(|value| !value.ownership().permits_copy()) {
                        return Err(ProductStepError::Type(
                            "external host-call arguments require deep Copy carriers".to_owned(),
                        ));
                    }
                    let parent_pending = self.pending_host_call.take();
                    self.emit_host_call(call, args, output);
                    child.pending_host_call = self.pending_host_call.take();
                    self.pending_host_call = parent_pending;
                    if child.pending_host_call.is_none() {
                        return Err(ProductStepError::Internal(format!(
                            "deferred child host call {} did not produce a pending request",
                            call.0
                        )));
                    }
                }
            }
            DeferredSuspensionDispatch::Unsupported => {
                return Err(ProductStepError::Internal(
                    "deferred child suspended on a product-owned dialogue or choice boundary"
                        .to_owned(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn resume_deferred_child_suspension(
        &mut self,
        child: &mut super::ProductChildFiber,
        need_states: &mut Vec<RuntimeNeedState>,
        host_results: &mut Vec<crate::step::RuntimeHostCallResult>,
        output: &mut RuntimeStepOutput,
        journal: &mut super::DeferredChildResumeJournal,
        pure_backend: &mut impl crate::pure::RuntimeCallBackend,
    ) -> Result<bool, ProductStepError> {
        let Some(suspension) = child.fiber.suspension.as_ref() else {
            return Ok(false);
        };
        let resume = suspension.declared_resume();
        let dispatch = DeferredSuspensionDispatch::from_reason(&suspension.reason);
        let Some(resume) = resume else {
            return match dispatch {
                DeferredSuspensionDispatch::BudgetYield => {
                    child
                        .fiber
                        .resume_budget_yield(&self.program)
                        .map_err(|error| ProductStepError::Internal(error.to_string()))?;
                    child.fiber.replenish_budget();
                    Ok(true)
                }
                _ => Err(ProductStepError::Internal(
                    "deferred child suspension has no declared resume point".to_owned(),
                )),
            };
        };
        match dispatch {
            DeferredSuspensionDispatch::AwaitNeed {
                correlation,
                item_type,
                binding,
                observer,
            } => Ok(self.resume_deferred_await_need(
                &mut child.fiber,
                &correlation,
                item_type,
                binding,
                observer,
                resume,
                need_states,
                output,
                journal,
            )),
            DeferredSuspensionDispatch::AwaitMany => {
                let reason = &mut child
                    .fiber
                    .suspension
                    .as_mut()
                    .expect("checked deferred child suspension remains present")
                    .reason;
                let FiberSuspensionReason::AwaitMany(state) =
                    std::mem::replace(reason, FiberSuspensionReason::BudgetYield)
                else {
                    unreachable!("checked deferred AwaitMany dispatch retains its sole owner")
                };
                Ok(self.resume_deferred_await_many(
                    &mut child.fiber,
                    state,
                    resume,
                    output,
                    journal,
                    pure_backend,
                ))
            }
            DeferredSuspensionDispatch::HostCall { call, destination } => Ok(self
                .resume_deferred_host_call(
                    child,
                    call,
                    destination,
                    resume,
                    host_results,
                    journal,
                    output,
                )),
            DeferredSuspensionDispatch::Unsupported => Err(ProductStepError::Internal(
                "deferred child suspended on a product-owned dialogue or choice boundary"
                    .to_owned(),
            )),
            DeferredSuspensionDispatch::BudgetYield => Ok(false),
        }
    }

    fn resume_deferred_await_need(
        &mut self,
        fiber: &mut FiberState,
        correlation: &crate::task::TaskCorrelation,
        item_type: crate::awbc::schema::AwbcTypeId,
        binding: Option<crate::awbc::schema::AwbcPatternId>,
        observer: Option<AwbcAwaitObserverResume>,
        resume: AwbcResumePointId,
        states: &mut Vec<RuntimeNeedState>,
        output: &mut RuntimeStepOutput,
        journal: &mut super::DeferredChildResumeJournal,
    ) -> bool {
        let need = &correlation.need;
        let local_launch = self.need_producers.launch_for_correlation(correlation);
        let (cursor, state) = if let Some(launch) = local_launch {
            if states.iter().any(|state| &state.correlation == correlation) {
                mark_child_trapped(
                    fiber,
                    AwbcTrapCode::InternalInvariant,
                    "external Need state attempts to publish a Product-owned Need".to_owned(),
                );
                return true;
            }
            if let Some(fault) = launch.task_fault() {
                mark_child_trapped(
                    fiber,
                    AwbcTrapCode::HostAbiMismatch,
                    format!("Need producer task failed: {fault}"),
                );
                return true;
            }
            let Some(cursor) = launch.publication() else {
                return false;
            };
            let state = match launch.state() {
                crate::task::RuntimeNeedProducerState::NotStarted => {
                    AwaitNeedPublicationKind::NotStarted
                }
                crate::task::RuntimeNeedProducerState::Pending(progress) => {
                    AwaitNeedPublicationKind::Pending(progress.clone())
                }
                crate::task::RuntimeNeedProducerState::Ready(_) => {
                    AwaitNeedPublicationKind::LocalReady
                }
                crate::task::RuntimeNeedProducerState::InfrastructureFailure(failure) => {
                    AwaitNeedPublicationKind::InfrastructureFailure(failure.clone())
                }
                crate::task::RuntimeNeedProducerState::ReadyTransferred => {
                    AwaitNeedPublicationKind::ReadyTransferred
                }
                crate::task::RuntimeNeedProducerState::Cancelled => {
                    AwaitNeedPublicationKind::Cancelled
                }
            };
            (cursor, state)
        } else {
            let Some(state) = resolved_runtime_need_state(states, correlation) else {
                return false;
            };
            let Some(cursor) = state.cursor else {
                return false;
            };
            let kind = match state.state() {
                Need::NotStarted => AwaitNeedPublicationKind::NotStarted,
                Need::Pending(progress) => AwaitNeedPublicationKind::Pending(progress.clone()),
                Need::Ready(crate::task::RuntimeNeedOutcome::Value(_)) => {
                    AwaitNeedPublicationKind::ExternalReady
                }
                Need::Ready(crate::task::RuntimeNeedOutcome::InfrastructureFailure(failure)) => {
                    AwaitNeedPublicationKind::InfrastructureFailure(failure.clone())
                }
                Need::Cancelled => AwaitNeedPublicationKind::Cancelled,
            };
            (cursor, kind)
        };
        let waiter =
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(fiber.instance.get().get());
        let publication_key = (waiter, *correlation);
        if let Some(observed) = self.need_publications.get(&publication_key) {
            match cursor.compare_same_source(*observed) {
                Some(Ordering::Greater) => {}
                Some(Ordering::Equal | Ordering::Less) => return false,
                None => {
                    mark_child_trapped(
                        fiber,
                        AwbcTrapCode::InternalInvariant,
                        "Need publication cursor changed source for one Await".to_owned(),
                    );
                    return true;
                }
            }
        }
        self.need_publications.insert(publication_key, cursor);
        match state {
            AwaitNeedPublicationKind::InfrastructureFailure(failure) => {
                journal.record_default_drop_policy();
                mark_child_trapped(fiber, AwbcTrapCode::HostAbiMismatch, failure.to_string());
                true
            }
            AwaitNeedPublicationKind::NotStarted => false,
            AwaitNeedPublicationKind::Pending(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need: need.clone(),
                    progress: progress.clone(),
                });
                observer.is_some_and(|observer| {
                    resume_deferred_await_progress(
                        &self.program,
                        fiber,
                        observer,
                        progress.clone(),
                        output,
                    )
                })
            }
            kind @ (AwaitNeedPublicationKind::LocalReady
            | AwaitNeedPublicationKind::ExternalReady) => {
                let local = matches!(kind, AwaitNeedPublicationKind::LocalReady);
                let selected = if local {
                    self.need_producers
                        .ready_for_correlation(correlation)
                        .map(|value| (value, None))
                } else {
                    resolved_runtime_need_state(states, correlation).and_then(|state| {
                        let index = states
                            .iter()
                            .position(|candidate| std::ptr::eq(candidate, state))?;
                        match state.state() {
                            Need::Ready(crate::task::RuntimeNeedOutcome::Value(value)) => {
                                Some((value, state.cursor.map(|cursor| (index, cursor))))
                            }
                            _ => None,
                        }
                    })
                };
                let Some((value, external)) = selected else {
                    mark_child_trapped(
                        fiber,
                        AwbcTrapCode::InternalInvariant,
                        "selected deferred Need Ready payload is unavailable".to_owned(),
                    );
                    return true;
                };
                if !crate::awbc::fiber::runtime_value_matches_type(
                    &self.program,
                    value.value(),
                    item_type,
                    0,
                ) {
                    mark_child_trapped(
                        fiber,
                        AwbcTrapCode::HostAbiMismatch,
                        format!(
                            "Need {} published a Ready payload outside its checked item type",
                            need
                        ),
                    );
                    return true;
                }
                let resume_proof = match fiber.validate_resume_at(&self.program, resume) {
                    Ok(proof) => proof,
                    Err(error) => {
                        mark_child_trapped(
                            fiber,
                            AwbcTrapCode::InternalInvariant,
                            error.to_string(),
                        );
                        return true;
                    }
                };
                let binding_proof = if let Some(pattern) = binding {
                    let proof = match crate::awbc::vm::prepare_pattern_binding(
                        &self.program,
                        fiber,
                        pattern,
                        value.value(),
                    ) {
                        Ok(proof) => proof,
                        Err(error) => {
                            mark_child_trapped(
                                fiber,
                                AwbcTrapCode::PatternMismatch,
                                error.to_string(),
                            );
                            return true;
                        }
                    };
                    let vacant = fiber.active_frame().ok().is_some_and(|frame| {
                        proof.registers().iter().all(|register| {
                            frame
                                .registers
                                .get(register.index())
                                .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
                        })
                    });
                    if !vacant {
                        mark_child_trapped(
                            fiber,
                            AwbcTrapCode::TypeMismatch,
                            "deferred Need Ready binding destination is occupied".to_owned(),
                        );
                        return true;
                    }
                    Some(proof)
                } else {
                    None
                };
                let observation = observed_ready_payload(value);
                let source = if let Some((index, cursor)) = external {
                    super::DeferredNeedReadySource::External { index, cursor }
                } else {
                    let proof = match self
                        .need_producers
                        .inspect_ready_take_for_correlation(correlation)
                    {
                        Ok(proof) => proof,
                        Err(error) => {
                            mark_child_trapped(
                                fiber,
                                AwbcTrapCode::InternalInvariant,
                                error.to_string(),
                            );
                            return true;
                        }
                    };
                    super::DeferredNeedReadySource::Local(proof)
                };
                if let Err(error) = journal.stage_need_ready(super::DeferredNeedReadyStage {
                    correlation: *correlation,
                    source,
                    resume: resume_proof,
                    binding: binding_proof,
                }) {
                    mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
                    return true;
                }
                output.flow_events.push(FlowEvent::AwaitReady {
                    need: need.clone(),
                    value: observation,
                });
                true
            }
            AwaitNeedPublicationKind::ReadyTransferred => {
                mark_child_trapped(
                    fiber,
                    AwbcTrapCode::HostAbiMismatch,
                    "Need Ready payload has already transferred to its unique waiter".to_owned(),
                );
                true
            }
            AwaitNeedPublicationKind::Cancelled => {
                journal.record_drop_policy(crate::effect::RuntimeDropPolicy::Cancel);
                let cancellation = cancel_fiber(fiber);
                journal.record_deferred_observations(cancellation.observations);
                true
            }
        }
    }

    fn resume_deferred_host_call(
        &mut self,
        child: &mut super::ProductChildFiber,
        call: AwbcHostCallId,
        destination: Option<crate::awbc::schema::AwbcRegisterId>,
        resume: AwbcResumePointId,
        results: &mut Vec<crate::step::RuntimeHostCallResult>,
        journal: &mut super::DeferredChildResumeJournal,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        if child.pending_host_call.is_none() {
            let Some(args) =
                child
                    .fiber
                    .suspension
                    .as_ref()
                    .and_then(|suspension| match &suspension.reason {
                        FiberSuspensionReason::HostCall { args, .. } => Some(args.as_slice()),
                        _ => None,
                    })
            else {
                mark_child_trapped(
                    &mut child.fiber,
                    AwbcTrapCode::InternalInvariant,
                    "deferred child host-call suspension lost its argument owner".to_owned(),
                );
                return true;
            };
            if args.iter().any(|value| !value.ownership().permits_copy()) {
                mark_child_trapped(
                    &mut child.fiber,
                    AwbcTrapCode::HostAbiMismatch,
                    "external host-call arguments require deep Copy carriers".to_owned(),
                );
                return true;
            }
            let parent_pending = self.pending_host_call.take();
            self.emit_host_call(call, args, output);
            child.pending_host_call = self.pending_host_call.take();
            self.pending_host_call = parent_pending;
        }
        let Some(pending) = child.pending_host_call.clone() else {
            return false;
        };
        let Some(index) = results.iter().position(|result| result.id == pending.id) else {
            return false;
        };
        if pending.call != call {
            child.pending_host_call = None;
            mark_child_trapped(
                &mut child.fiber,
                AwbcTrapCode::HostAbiMismatch,
                "pending deferred child host call does not match its continuation".to_owned(),
            );
            return true;
        }
        match &results[index].outcome {
            Ok(value) => {
                let checked = self
                    .program
                    .host_calls
                    .get(call.index())
                    .and_then(|record| self.program.signatures.get(record.signature.index()))
                    .is_some_and(|signature| match signature.result {
                        Some(ty) => self
                            .program
                            .validate_live_value(
                                ty,
                                value.value(),
                                crate::entry::RuntimeSchemaLimits::engine_default(),
                            )
                            .is_ok(),
                        None => value.value() == &RuntimeValue::Unit,
                    });
                if !checked {
                    child.pending_host_call = None;
                    mark_child_trapped(
                        &mut child.fiber,
                        AwbcTrapCode::HostAbiMismatch,
                        "deferred child host-call result violates its checked type".to_owned(),
                    );
                    return true;
                }
                let prepared_resume = match child.fiber.validate_resume_at(&self.program, resume) {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        mark_child_trapped(
                            &mut child.fiber,
                            AwbcTrapCode::InternalInvariant,
                            error.to_string(),
                        );
                        return true;
                    }
                };
                if let Some(destination) = destination {
                    let vacant = child.fiber.active_frame().ok().is_some_and(|frame| {
                        frame
                            .registers
                            .get(destination.index())
                            .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
                    });
                    if !vacant {
                        mark_child_trapped(
                            &mut child.fiber,
                            AwbcTrapCode::TypeMismatch,
                            "deferred host-call destination register is unavailable".to_owned(),
                        );
                        return true;
                    }
                }
                let stage = super::PreparedHostResultTake {
                    result_id: pending.id,
                    result_index: index,
                    target: super::PreparedHostResultTarget::Deferred {
                        child: child.fiber.instance,
                        call,
                        outcome: super::PreparedHostResultOutcome::Ready {
                            destination,
                            resume: prepared_resume,
                        },
                    },
                };
                if let Err(error) = journal.stage_host_call(stage) {
                    mark_child_trapped(
                        &mut child.fiber,
                        AwbcTrapCode::InternalInvariant,
                        error.to_string(),
                    );
                }
                true
            }
            Err(error) => {
                let stage = super::PreparedHostResultTake {
                    result_id: pending.id,
                    result_index: index,
                    target: super::PreparedHostResultTarget::Deferred {
                        child: child.fiber.instance,
                        call,
                        outcome: super::PreparedHostResultOutcome::Failed {
                            kind: error.kind,
                            message: error.message.clone(),
                        },
                    },
                };
                if let Err(error) = journal.stage_host_call(stage) {
                    mark_child_trapped(
                        &mut child.fiber,
                        AwbcTrapCode::InternalInvariant,
                        error.to_string(),
                    );
                }
                true
            }
        }
    }

    fn fill_await_many_for_fiber(
        &mut self,
        fiber: &mut FiberState,
        output: &mut RuntimeStepOutput,
        backend: &mut impl crate::pure::RuntimeCallBackend,
    ) {
        let Some(suspension) = fiber.suspension.as_mut() else {
            return;
        };
        let reason = std::mem::replace(&mut suspension.reason, FiberSuspensionReason::BudgetYield);
        let FiberSuspensionReason::AwaitMany(mut state) = reason else {
            suspension.reason = reason;
            return;
        };
        let result = self.fill_await_many_state(&mut state, output, backend);
        if let Some(suspension) = fiber.suspension.as_mut() {
            suspension.reason = FiberSuspensionReason::AwaitMany(state);
        }
        if let Err(error) = result {
            mark_child_trapped(fiber, error.trap_code(), error.to_string());
        }
    }
    fn resume_deferred_await_many(
        &mut self,
        fiber: &mut FiberState,
        mut state: crate::awbc::fiber::FiberAwaitManyState,
        resume: AwbcResumePointId,
        output: &mut RuntimeStepOutput,
        journal: &mut super::DeferredChildResumeJournal,
        pure_backend: &mut impl crate::pure::RuntimeCallBackend,
    ) -> bool {
        if state.results.len() != state.items.len() {
            journal.record_default_drop_policy();
            mark_child_trapped(
                fiber,
                AwbcTrapCode::InternalInvariant,
                "deferred AwaitMany result slots disagree with the admitted item count".to_owned(),
            );
            return true;
        }
        let in_flight_tasks = state
            .in_flight
            .iter()
            .map(|in_flight| in_flight.handle.correlation().task_id)
            .collect::<std::collections::BTreeSet<_>>();
        let events = self.take_await_many_task_events_for_deferred(&in_flight_tasks, journal);
        let mut progressed = false;
        for event_index in events {
            let event = journal
                .task_event(event_index)
                .expect("recorded deferred task event remains present");
            let Some(position) = state.in_flight.iter().position(|in_flight| {
                in_flight.handle.correlation().task_id == event.correlation.task_id
            }) else {
                continue;
            };
            match &event.kind {
                TaskEventKind::Ready(value) => {
                    if !self
                        .task_payload_accepts(self.await_many_child_plan(state.plan), value.value())
                    {
                        let task_id = event.correlation.task_id;
                        journal.record_default_drop_policy();
                        mark_child_trapped(
                            fiber,
                            AwbcTrapCode::HostAbiMismatch,
                            format!(
                                "await task {} published a payload outside its checked outcome contract",
                                task_id
                            ),
                        );
                        return true;
                    }
                    let in_flight = state.in_flight.remove(position);
                    let observation = observed_ready_payload(value);
                    let value = match journal.take_ready_value(
                        event_index,
                        state.plan,
                        in_flight.index as usize,
                    ) {
                        Ok(value) => value,
                        Err(error) => {
                            journal.record_default_drop_policy();
                            mark_child_trapped(
                                fiber,
                                AwbcTrapCode::InternalInvariant,
                                error.to_string(),
                            );
                            return true;
                        }
                    };
                    state.results[in_flight.index as usize] = Some(value);
                    output.flow_events.push(FlowEvent::AwaitReady {
                        need: in_flight.handle.need_id(),
                        value: observation,
                    });
                    if let Some(suspension) = fiber.suspension.as_mut() {
                        suspension.reason = FiberSuspensionReason::AwaitMany(state);
                    }
                    return true;
                }
                TaskEventKind::Progress(progress) => {
                    output.flow_events.push(FlowEvent::AwaitProgress {
                        need: state.in_flight[position].handle.need_id(),
                        progress: progress.clone(),
                    });
                    progressed = true;
                }
                TaskEventKind::InfrastructureFailure(error) => {
                    let message = format!(
                        "await task {} at index {} failed: {}",
                        event.correlation.task_id, state.in_flight[position].index, error
                    );
                    journal.record_default_drop_policy();
                    mark_child_trapped(fiber, AwbcTrapCode::HostAbiMismatch, message);
                    return true;
                }
                TaskEventKind::Cancelled => {
                    journal.record_drop_policy(crate::effect::RuntimeDropPolicy::Cancel);
                    let cancellation = cancel_fiber(fiber);
                    journal.record_deferred_observations(cancellation.observations);
                    return true;
                }
            }
        }
        if state.in_flight.is_empty()
            && state.next_index as usize >= state.items.len()
            && state.results.iter().all(Option::is_some)
        {
            let values = state
                .results
                .iter_mut()
                .map(|slot| {
                    slot.take()
                        .expect("completed deferred AwaitMany result remains present")
                })
                .collect::<Vec<_>>();
            let value = runtime_sequence_values(values);
            let Some(base) = &state.base else {
                journal.record_default_drop_policy();
                mark_child_trapped(
                    fiber,
                    AwbcTrapCode::InternalInvariant,
                    "AwaitMany has no accepted base".into(),
                );
                return true;
            };
            let need = base.need_id();
            let prepared_resume = match fiber.validate_resume_at(&self.program, resume) {
                Ok(prepared) => prepared,
                Err(error) => {
                    journal.record_default_drop_policy();
                    mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
                    return true;
                }
            };
            let binding = if let Some(pattern) = state.binding {
                let prepared = match crate::awbc::vm::prepare_pattern_binding(
                    &self.program,
                    fiber,
                    pattern,
                    &value,
                ) {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        journal.record_default_drop_policy();
                        mark_child_trapped(fiber, AwbcTrapCode::PatternMismatch, error.to_string());
                        return true;
                    }
                };
                let vacant = fiber.active_frame().ok().is_some_and(|frame| {
                    prepared.registers().iter().all(|register| {
                        frame
                            .registers
                            .get(register.index())
                            .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
                    })
                });
                if !vacant {
                    journal.record_default_drop_policy();
                    mark_child_trapped(
                        fiber,
                        AwbcTrapCode::TypeMismatch,
                        "deferred AwaitMany binding destination is occupied".to_owned(),
                    );
                    return true;
                }
                Some(prepared)
            } else {
                None
            };
            let observation = value
                .ownership()
                .permits_copy()
                .then(|| RuntimePayload::from(value.clone()));
            if let Some(binding) = binding {
                crate::awbc::vm::bind_pattern_owned_prepared(&self.program, fiber, binding, value);
            }
            fiber.resume_at_prepared(prepared_resume);
            journal.record_default_drop_policy();
            output.flow_events.push(FlowEvent::AwaitReady {
                need,
                value: observation,
            });
            return true;
        }
        if let Some(suspension) = fiber.suspension.as_mut() {
            suspension.reason = FiberSuspensionReason::AwaitMany(state);
        }
        let request_count = output.requests.tasks.len();
        let diagnostic_count = output.diagnostics.len();
        self.fill_await_many_for_fiber(fiber, output, pure_backend);
        if fiber.status == crate::awbc::fiber::FiberStatus::Trapped {
            journal.record_default_drop_policy();
        }
        if output.diagnostics.len() > diagnostic_count {
            journal.record_default_drop_policy();
            mark_child_trapped(
                fiber,
                AwbcTrapCode::InternalInvariant,
                output.diagnostics.last().map_or_else(
                    || "deferred await-many request failed".to_owned(),
                    |d| d.message.clone(),
                ),
            );
            return true;
        }
        progressed || output.requests.tasks.len() > request_count
    }

    fn take_await_many_task_events(
        &mut self,
        in_flight_tasks: &std::collections::BTreeSet<TaskId>,
    ) -> Vec<TaskEvent> {
        let mut events = Vec::new();
        let mut index = 0;
        while index < self.queued_task_events.len() {
            if in_flight_tasks.contains(&self.queued_task_events[index].correlation.task_id) {
                if let Some(event) = self.queued_task_events.remove(index) {
                    events.push(event);
                }
                break;
            } else {
                index += 1;
            }
        }
        events
    }

    fn take_await_many_task_events_for_deferred(
        &mut self,
        in_flight_tasks: &std::collections::BTreeSet<TaskId>,
        journal: &mut super::DeferredChildResumeJournal,
    ) -> std::ops::Range<usize> {
        let start = journal.consumed_task_events.len();
        let mut index = 0;
        while index < self.queued_task_events.len() {
            if in_flight_tasks.contains(&self.queued_task_events[index].correlation.task_id) {
                let event = self
                    .queued_task_events
                    .remove(index)
                    .expect("checked queued task event remains present");
                let mut original_index = index;
                let mut prior_positions = journal
                    .consumed_task_events
                    .iter()
                    .map(|entry| entry.queue_index)
                    .collect::<Vec<_>>();
                prior_positions.sort_unstable();
                for position in prior_positions {
                    if position <= original_index {
                        original_index += 1;
                    }
                }
                journal.record_consumed_task_event(original_index, event);
                break;
            } else {
                index += 1;
            }
        }
        start..journal.consumed_task_events.len()
    }

    pub(super) fn consume_observations(
        &mut self,
        observations: Vec<VmObservation>,
        output: &mut RuntimeStepOutput,
    ) {
        for observation in observations {
            match observation {
                VmObservation::Instruction { .. } => {}
                VmObservation::Effect { effect, args } => self.emit_effect(effect, &args, output),
                VmObservation::EnsureContent(content) => {
                    if self.emitted_content.insert(content) {
                        match content_request(&self.program, content) {
                            Ok(request) => output.requests.ensure_content.push(request),
                            Err(error) => self.record_error(error, output),
                        }
                    }
                }
                VmObservation::NeedProducerStarted {
                    cursor,
                    fiber,
                    dst,
                    plan,
                    args,
                } => {
                    if self.fiber.instance.get().get() == fiber.get() {
                        self.commit_need_producer_started(cursor, fiber, dst, plan, args, output);
                    } else {
                        self.commit_child_need_producer_started(
                            cursor, fiber, dst, plan, args, output,
                        );
                    }
                }
                VmObservation::Goto(target) => match self.flow_identity_for_function(target) {
                    Ok(target) => output.flow_events.push(FlowEvent::Goto { target }),
                    Err(error) => self.record_error(error, output),
                },
                VmObservation::FiberSpawned { function, args, .. } => {
                    self.spawn_child(function, &args, output);
                }
                VmObservation::StreamYield { stream, value } => {
                    let event_value =
                        match crate::stream::RuntimeStreamYieldCopyProof::inspect(&value) {
                            Ok(proof) => proof.copy(),
                            Err(error) => {
                                self.fail_with_error(
                                    super::ProductStepError::Internal(error.to_string()),
                                    output,
                                );
                                continue;
                            }
                        };
                    let stream_id = stream_id_for(&self.program, stream);
                    let sequence = self.stream_sequences.entry(stream).or_default();
                    output.effects.stream_events.push(RuntimeStreamEvent {
                        stream: stream_id.clone(),
                        sequence: TaskSequence(*sequence),
                        kind: StreamEventKind::Item(RuntimePayload::from(event_value)),
                    });
                    *sequence = sequence.saturating_add(1);
                    if let Some(state) = self.facade_fiber.stream_states.get_mut(&stream_id) {
                        state.push_item(RuntimePayload::from(value));
                    }
                }
                VmObservation::StreamClose(stream) => {
                    let stream_id = stream_id_for(&self.program, stream);
                    if let Some(state) = self.facade_fiber.stream_states.get_mut(&stream_id)
                        && let Some(sequence) = state.close_with_sequence()
                    {
                        output.effects.stream_events.push(RuntimeStreamEvent {
                            stream: stream_id,
                            sequence,
                            kind: StreamEventKind::End,
                        });
                    }
                }
                VmObservation::LineOperation { .. }
                | VmObservation::DialogueResult { .. }
                | VmObservation::LineDeferRegistration { .. }
                | VmObservation::ScopedDeferRegistration { .. }
                | VmObservation::ScopedDeferUnwind { .. }
                | VmObservation::ScopedDeferFailure(_)
                | VmObservation::Drop { .. }
                | VmObservation::DiscardedValue(_) => self.fail_with_error(
                    crate::line_task::LineRuntimeError::InvalidActivationOperation.into(),
                    output,
                ),
                VmObservation::Trap(trap) => self.record_trap(&trap, output),
            }
        }
    }

    pub(super) fn emit_effect(
        &mut self,
        effect: AwbcEffectPlanId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        let Some(plan) = self.program.effect_plans.get(effect.index()) else {
            self.record_error(
                ProductStepError::Internal(format!("missing AWBC effect plan {}", effect.0)),
                output,
            );
            return;
        };
        match plan.kind.map_product_effect(&self.program, effect, args) {
            MappedEffect::Omitted => {}
            MappedEffect::Line(effect) => output.effects.line.push(effect),
            MappedEffect::Audio(command) => output.requests.audio.push(AudioCommandEnvelope::new(
                self.next_audio_dispatch(),
                command,
            )),
            MappedEffect::Unsupported(diagnostic) => output.diagnostics.push(diagnostic),
        }
    }

    pub(super) fn next_audio_dispatch(&mut self) -> AudioDispatchId {
        let dispatch = AudioDispatchId::new(0, self.next_audio_sequence);
        self.next_audio_sequence = self.next_audio_sequence.saturating_add(1);
        dispatch
    }

    pub(super) fn spawn_child(
        &mut self,
        function: AwbcFunctionId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        self.spawn_owned_child(
            super::ProductChildFiberOwner::Independent,
            function,
            args,
            output,
        );
    }

    pub(super) fn spawn_owned_child(
        &mut self,
        owner: super::ProductChildFiberOwner,
        function: AwbcFunctionId,
        args: &[RuntimeValue],
        output: &mut RuntimeStepOutput,
    ) {
        let Some(next_generation) = self.next_generation.checked_add(1) else {
            self.record_error(ProductStepError::ChildGenerationOverflow, output);
            return;
        };
        let mut next_fiber_instance = self.next_fiber_instance;
        let fiber_instance = match next_fiber_instance
            .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
        {
            Ok(instance) => crate::runtime_id::RuntimeFiberInstanceId::from_allocated(instance),
            Err(error) => {
                self.record_error(ProductStepError::RuntimeIdentity(error), output);
                return;
            }
        };
        match FiberState::for_function_with_instance(
            &self.program,
            crate::awbc::fiber::AwbcFiberRoot::Function(function),
            function,
            fiber_instance,
            self.next_generation,
            self.fiber.budget.quantum.max(1),
        ) {
            Ok(mut child) => {
                match child
                    .active_frame_mut()
                    .and_then(|frame| frame.bind_positional_arguments(&self.program, args))
                {
                    Ok(()) => {
                        self.next_generation = next_generation;
                        self.next_fiber_instance = next_fiber_instance;
                        self.child_fibers.push_back(super::ProductChildFiber {
                            owner,
                            fiber: child,
                            runtime_generation: self.runtime_generation,
                            pending_host_call: None,
                        });
                    }
                    Err(error) => {
                        self.record_error(ProductStepError::Type(error.to_string()), output);
                    }
                }
            }
            Err(error) => self.record_error(ProductStepError::Internal(error.to_string()), output),
        }
    }

    fn commit_need_producer_started(
        &mut self,
        cursor: crate::awbc::fiber::FiberCursor,
        fiber: crate::runtime_id::RuntimePersistentFiberId,
        dst: crate::awbc::schema::AwbcRegisterId,
        plan_id: crate::awbc::schema::AwbcTaskPlanId,
        values: Vec<RuntimeValue>,
        output: &mut RuntimeStepOutput,
    ) {
        let started = self.stage_need_producer_started(
            &self.fiber,
            &self.need_producers,
            self.runtime_generation,
            cursor,
            fiber,
            plan_id,
            values,
        );
        let started = match started {
            Ok(started) => started,
            Err(error) => {
                self.fail_with_error(error, output);
                return;
            }
        };
        let ensure =
            started.admission().disposition() == crate::task::NeedProducerTaskDisposition::Ensure;
        if ensure && self.remaining_new_task_requests == 0 {
            self.fail_with_budget_error(
                "Need producer start exceeds this step's task request quota".to_owned(),
                output,
            );
            return;
        }
        let checkpoint = match self.fiber.checkpoint() {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                self.fail_with_error(ProductStepError::Fiber(error), output);
                return;
            }
        };
        let committed = self
            .need_producers
            .commit_start_visit_with(started, |handle| {
                self.fiber
                    .active_frame_mut()
                    .and_then(|frame| frame.set_register(dst, RuntimeValue::NeedHandle(handle)))
                    .and_then(|()| self.fiber.commit_yielded_instruction(cursor))
            });
        let admission = match committed {
            Ok((admission, ())) => admission,
            Err(error) => {
                let owner =
                    crate::task::RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&self.program));
                if let Err(restore_error) = self.fiber.restore(checkpoint, &owner) {
                    self.fail_with_error(ProductStepError::Fiber(restore_error), output);
                } else {
                    self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                }
                return;
            }
        };
        if ensure {
            self.remaining_new_task_requests -= 1;
            output
                .requests
                .tasks
                .push(self.need_producers.submission_for_admission(&admission));
        }
    }

    fn commit_child_need_producer_started(
        &mut self,
        cursor: crate::awbc::fiber::FiberCursor,
        persistent_fiber: crate::runtime_id::RuntimePersistentFiberId,
        dst: crate::awbc::schema::AwbcRegisterId,
        plan_id: crate::awbc::schema::AwbcTaskPlanId,
        values: Vec<RuntimeValue>,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(index) = self
            .child_fibers
            .iter()
            .position(|child| child.fiber.instance.get().get() == persistent_fiber.get())
        else {
            self.fail_with_error(
                ProductStepError::Internal(
                    "Need producer observation references an absent Product child fiber".to_owned(),
                ),
                output,
            );
            return;
        };
        let started = self.stage_need_producer_started(
            &self.child_fibers[index].fiber,
            &self.need_producers,
            self.runtime_generation,
            cursor,
            persistent_fiber,
            plan_id,
            values,
        );
        let started = match started {
            Ok(started) => started,
            Err(error) => {
                self.fail_with_error(error, output);
                return;
            }
        };
        let ensure =
            started.admission().disposition() == crate::task::NeedProducerTaskDisposition::Ensure;
        if ensure && self.remaining_new_task_requests == 0 {
            self.fail_with_budget_error(
                "Need producer start exceeds this step's task request quota".to_owned(),
                output,
            );
            return;
        }
        let checkpoint = match self.child_fibers[index].fiber.checkpoint() {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                self.fail_with_error(ProductStepError::Fiber(error), output);
                return;
            }
        };
        let committed = self
            .need_producers
            .commit_start_visit_with(started, |handle| {
                self.child_fibers[index]
                    .fiber
                    .active_frame_mut()
                    .and_then(|frame| frame.set_register(dst, RuntimeValue::NeedHandle(handle)))
                    .and_then(|()| {
                        self.child_fibers[index]
                            .fiber
                            .commit_yielded_instruction(cursor)
                    })
            });
        let admission = match committed {
            Ok((admission, ())) => admission,
            Err(error) => {
                let owner =
                    crate::task::RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&self.program));
                if let Err(restore_error) =
                    self.child_fibers[index].fiber.restore(checkpoint, &owner)
                {
                    self.fail_with_error(ProductStepError::Fiber(restore_error), output);
                } else {
                    self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                }
                return;
            }
        };
        if ensure {
            self.remaining_new_task_requests -= 1;
            output
                .requests
                .tasks
                .push(self.need_producers.submission_for_admission(&admission));
        }
    }

    fn fail_with_budget_error(&mut self, message: String, output: &mut RuntimeStepOutput) {
        output.diagnostics.push(RuntimeDiagnostic::categorized(
            RuntimeDiagnosticCategory::Budget,
            message.clone(),
        ));
        self.fail_with_trap(AwbcTrapCode::InternalInvariant, message, None, output);
    }

    fn stage_need_producer_started(
        &self,
        fiber: &FiberState,
        registry: &crate::task::NeedProducerRegistry,
        generation: crate::task::GenerationId,
        cursor: crate::awbc::fiber::FiberCursor,
        persistent_fiber: crate::runtime_id::RuntimePersistentFiberId,
        plan_id: crate::awbc::schema::AwbcTaskPlanId,
        values: Vec<RuntimeValue>,
    ) -> Result<crate::task::NeedProducerStartProof, ProductStepError> {
        if fiber.cursor != cursor || fiber.instance.get().get() != persistent_fiber.get() {
            return Err(ProductStepError::Internal(
                "Need producer observation does not belong to the selected Product fiber cursor"
                    .to_owned(),
            ));
        }
        let row = self
            .program
            .task_plans
            .get(plan_id.index())
            .ok_or_else(|| {
                ProductStepError::Internal(format!(
                    "Need producer observation references absent task plan {}",
                    plan_id.0
                ))
            })?;
        let plan = row
            .need_producer_plan(&self.program)
            .map_err(ProductStepError::Internal)?;
        let signature = self
            .program
            .signatures
            .get(row.signature.index())
            .ok_or_else(|| {
                ProductStepError::Internal("Need producer signature is absent".to_owned())
            })?;
        if values.len() != row.arguments.len() || values.len() != signature.params.len() {
            return Err(ProductStepError::Internal(
                "Need producer runtime arguments disagree with its verified signature".to_owned(),
            ));
        }
        let mut arguments = Vec::with_capacity(values.len());
        for ((descriptor, value), expected) in row
            .arguments
            .iter()
            .zip(values)
            .zip(signature.params.iter().copied())
        {
            if descriptor.spread
                || !crate::awbc::fiber::runtime_value_matches_type(
                    &self.program,
                    &value,
                    expected,
                    0,
                )
            {
                return Err(ProductStepError::Type(
                    "Need producer argument violates its checked source type".to_owned(),
                ));
            }
            let name = descriptor
                .name
                .map(|name| {
                    self.program
                        .strings
                        .get(name.index())
                        .cloned()
                        .ok_or_else(|| {
                            ProductStepError::Internal(
                                "Need producer argument name is absent".to_owned(),
                            )
                        })
                })
                .transpose()?;
            arguments.push(NeedProducerRuntimeArgument { name, value });
        }
        registry
            .inspect_start_visit(generation, persistent_fiber, plan, arguments)
            .map_err(|error| ProductStepError::Host(error.to_string()))
    }
}

fn mark_child_trapped(fiber: &mut FiberState, code: AwbcTrapCode, message: String) {
    fiber.mark_trapped(crate::awbc::fiber::FiberTrap {
        code,
        message: Some(message),
        source_map: None,
    });
}

fn resume_deferred_await_progress(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    observer: AwbcAwaitObserverResume,
    progress: arcweft_need::Progress,
    _output: &mut RuntimeStepOutput,
) -> bool {
    let value = RuntimeValue::Progress(progress);
    match fiber.active_frame_mut() {
        Ok(frame) => {
            if let Err(error) = frame.set_register(observer.destination, value) {
                mark_child_trapped(fiber, AwbcTrapCode::TypeMismatch, error.to_string());
                return true;
            }
        }
        Err(error) => {
            mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
            return true;
        }
    }
    if let Err(error) = fiber.resume_await_observer_at(program, observer.resume) {
        mark_child_trapped(fiber, AwbcTrapCode::InternalInvariant, error.to_string());
    }
    true
}
