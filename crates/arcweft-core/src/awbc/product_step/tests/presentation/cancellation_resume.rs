use super::super::*;
use crate::awbc::fiber::FiberSuspensionReason;
use crate::awbc::product_step::DeferredChildResumeJournal;
use crate::presentation::resources::{
    PresentationBanks, PresentationBody, PresentationLedgerLimits, PresentationRegistrationSite,
};
use crate::runtime_id::RuntimePresentationHandleToken;
use crate::step::RuntimeStepOutput;
use arcweft_interaction_model::presentation::PresentationResourceKind;

fn install_resource(
    executor: &mut AwbcProductStepExecutor,
    child: Option<usize>,
    blocked: bool,
) -> RuntimePresentationHandleToken {
    let expected = executor
        .program
        .runtime_types
        .iter()
        .find(|ty| matches!(ty.shape(), AwbcRuntimeTypeShape::String))
        .unwrap()
        .semantic_identity();
    install_resource_values(
        executor,
        child,
        blocked,
        expected,
        RuntimeValue::String("current".into()),
        RuntimeValue::String("prior".into()),
    )
}

fn install_resource_values(
    executor: &mut AwbcProductStepExecutor,
    child: Option<usize>,
    blocked: bool,
    expected: RuntimeSemanticTypeId,
    current: RuntimeValue,
    prior: RuntimeValue,
) -> RuntimePresentationHandleToken {
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let mut banks = PresentationBanks::new(
        owner.clone(),
        executor.artifact_fingerprint,
        if blocked {
            PresentationLedgerLimits {
                handles: 1,
                scopes: 8,
            }
        } else {
            PresentationLedgerLimits::for_schema(crate::entry::RuntimeSchemaLimits::engine_default())
        },
    );
    let fiber = child.map_or(&mut executor.fiber, |index| {
        &mut executor.child_fibers[index].fiber
    });
    let site = || PresentationRegistrationSite::Independent(PresentationResourceKind::Image);
    let body = |value| PresentationBody {
        expected,
        binding: None,
        value: RuntimePayload(value),
    };
    if blocked {
        let ledger = banks.ledger_mut(fiber.presentation_bank()).unwrap();
        let scope = fiber.bind_presentation_scope(&owner, ledger).unwrap();
        ledger.register(&owner, scope, site(), body(prior)).unwrap();
        fiber.retire_presentation_scope(&owner, &mut banks).unwrap();
    }
    let ledger = banks.ledger_mut(fiber.presentation_bank()).unwrap();
    let scope = fiber.bind_presentation_scope(&owner, ledger).unwrap();
    let token = ledger
        .register(&owner, scope, site(), body(current))
        .unwrap()
        .0;
    executor.presentation = Some(banks);
    executor.snapshot().unwrap();
    token
}

fn assert_retired(executor: &AwbcProductStepExecutor, token: &RuntimePresentationHandleToken) {
    let retired = executor.presentation.as_ref().unwrap().retirements();
    assert_eq!(retired.len(), 1);
    assert_eq!(&retired[0].token, token);
    assert_eq!(
        retired[0].body.value.0,
        RuntimeValue::String("current".into())
    );
}

fn roundtrip(executor: AwbcProductStepExecutor) -> AwbcProductStepExecutor {
    let before = executor.snapshot().unwrap();
    let saved = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let wire = serde_json::to_vec(&saved).unwrap();
    let executor = executor
        .restore_inert_snapshot_owned(serde_json::from_slice(&wire).unwrap())
        .unwrap();
    assert_eq!(executor.snapshot().unwrap(), before);
    executor
}

fn need_fixture(
    blocked: bool,
) -> (
    AwbcProductStepExecutor,
    RuntimeStepInput,
    RuntimePresentationHandleToken,
) {
    let (mut executor, input) =
        typed_direct_need_executor_and_input(vec![runtime_need_state(0, Need::Cancelled)]);
    let token = install_resource(&mut executor, None, blocked);
    let output = executor.step(Default::default(), RuntimeStepOptions::default());
    assert!(
        output.output.diagnostics.is_empty(),
        "{:?}",
        output.output.diagnostics
    );
    assert!(matches!(
        executor
            .fiber
            .suspension
            .as_ref()
            .map(|suspension| &suspension.reason),
        Some(FiberSuspensionReason::Await {
            target: crate::awbc::fiber::FiberAwaitTarget::Need { .. },
            ..
        })
    ));
    (executor, input, token)
}

fn resume_need(
    executor: &mut AwbcProductStepExecutor,
    input: &mut RuntimeStepInput,
    output: &mut RuntimeStepOutput,
) -> bool {
    let correlation = input.need_states[0].correlation;
    executor.resume_need(
        &correlation,
        AwbcTypeId(2),
        Some(AwbcPatternId(0)),
        None,
        AwbcResumePointId(0),
        &mut input.need_states,
        output,
    )
}

#[test]
fn need_cancellation_retires_the_actual_scope_and_roundtrips_the_public_owner() {
    let (mut executor, mut input, token) = need_fixture(false);
    assert!(resume_need(
        &mut executor,
        &mut input,
        &mut RuntimeStepOutput::default()
    ));
    assert_eq!(executor.fiber.status, FiberStatus::Cancelled);
    assert_eq!(executor.need_publications.len(), 1);
    assert_retired(&executor, &token);
    roundtrip(executor);
}

#[test]
fn need_cancellation_capacity_refusal_preserves_the_publication_and_complete_snapshot_until_retry()
{
    let (mut executor, mut input, token) = need_fixture(true);
    let before = executor.snapshot().unwrap();
    let mut output = RuntimeStepOutput::default();
    assert!(!resume_need(&mut executor, &mut input, &mut output));
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(executor.snapshot().unwrap(), before);
    assert!(executor.need_publications.is_empty());
    executor
        .presentation
        .as_mut()
        .unwrap()
        .retirements_mut()
        .clear();
    let mut output = RuntimeStepOutput::default();
    assert!(resume_need(&mut executor, &mut input, &mut output));
    assert!(output.diagnostics.is_empty());
    assert_eq!(executor.fiber.status, FiberStatus::Cancelled);
    assert_eq!(executor.need_publications.len(), 1);
    assert_retired(&executor, &token);
    executor.snapshot().unwrap();
}

fn awaiting_many(threaded: bool) -> AwbcProductStepExecutor {
    let program = await_many_product_program_with_thread(
        if threaded {
            AwaitManyFixtureOutput::Result
        } else {
            AwaitManyFixtureOutput::Sequence
        },
        threaded,
    );
    let mut executor = if threaded {
        AwbcProductStepExecutor::for_function_invocation_with_generation(
            program,
            AwbcEntryId(0),
            AwbcFunctionId(3),
            [RuntimeFlowParameterBinding {
                parameter: crate::entry::FlowParameterCoordinate::from_position(0),
                value: await_many_items(),
            }],
            64,
            GenerationId::new(7),
        )
        .unwrap()
    } else {
        await_many_executor(&program, await_many_items(), GenerationId::new(7))
    };
    for _ in 0..64 {
        let result = executor.step(Default::default(), direct_need_step_options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        let fiber = if threaded {
            executor.child_fibers.front().map(|child| &child.fiber)
        } else {
            Some(&executor.fiber)
        };
        if fiber.is_some_and(|fiber| {
            matches!(fiber.suspension.as_ref().map(|suspension| &suspension.reason),
            Some(FiberSuspensionReason::AwaitMany(state)) if !state.in_flight.is_empty())
        }) {
            return executor;
        }
    }
    panic!("genuine Source-backed AwaitMany did not suspend");
}

fn queue_cancellation(executor: &mut AwbcProductStepExecutor, child: bool) {
    let fiber = if child {
        &executor.child_fibers[0].fiber
    } else {
        &executor.fiber
    };
    let Some(FiberSuspensionReason::AwaitMany(state)) = fiber
        .suspension
        .as_ref()
        .map(|suspension| &suspension.reason)
    else {
        unreachable!()
    };
    let correlation = state.in_flight[0].handle.correlation();
    let mut output = RuntimeStepOutput::default();
    executor.latch_task_events(
        vec![producer_task_event(
            correlation.generation,
            correlation,
            0,
            TaskEventKind::Cancelled,
        )],
        &mut output,
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(executor.queued_task_events.len(), 1);
}

fn resume_many(
    executor: &mut AwbcProductStepExecutor,
    child: bool,
    output: &mut RuntimeStepOutput,
) -> bool {
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    if child {
        let mut journal = DeferredChildResumeJournal::capture(executor);
        let mut child = executor.child_fibers.pop_front().unwrap();
        let mut staged = RuntimeStepOutput::default();
        let result = executor
            .resume_deferred_child_suspension(
                &mut child,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut staged,
                &mut journal,
                &mut backend,
            )
            .unwrap();
        if !result {
            assert!(journal.consumed_task_events.is_empty());
        }
        executor.child_fibers.push_front(child);
        journal.commit(executor, staged, output);
        result
    } else {
        let before = crate::awbc::product_step::line::product_fiber_handle_owners(
            executor.facade_fiber.execution,
            &executor.fiber,
        )
        .unwrap();
        let suspension = executor.fiber.suspension.as_mut().unwrap();
        let resume = suspension.declared_resume().unwrap();
        let FiberSuspensionReason::AwaitMany(state) =
            std::mem::replace(&mut suspension.reason, FiberSuspensionReason::BudgetYield)
        else {
            unreachable!()
        };
        executor.resume_await_many(state, resume, before, output, &mut backend)
    }
}

#[test]
fn main_await_many_cancellation_keeps_the_queued_event_and_extracted_state_on_capacity_refusal() {
    for blocked in [false, true] {
        let mut executor = awaiting_many(false);
        let token = install_resource(&mut executor, None, blocked);
        queue_cancellation(&mut executor, false);
        if blocked {
            let before = executor.snapshot().unwrap();
            let mut output = RuntimeStepOutput::default();
            assert!(!resume_many(&mut executor, false, &mut output));
            assert_eq!(output.diagnostics.len(), 1);
            assert_eq!(executor.snapshot().unwrap(), before);
            assert_eq!(executor.queued_task_events.len(), 1);
            executor
                .presentation
                .as_mut()
                .unwrap()
                .retirements_mut()
                .clear();
        }
        let mut output = RuntimeStepOutput::default();
        assert!(resume_many(&mut executor, false, &mut output));
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(executor.fiber.status, FiberStatus::Cancelled);
        assert!(executor.queued_task_events.is_empty());
        assert_retired(&executor, &token);
        executor.snapshot().unwrap();
        if !blocked {
            roundtrip(executor);
        }
    }
}

#[test]
fn genuine_program_child_await_many_cancellation_preserves_event_journal_and_bank_until_admitted() {
    for blocked in [false, true] {
        let mut executor = awaiting_many(true);
        assert!(matches!(
            executor.child_fibers[0].owner,
            ProductChildFiberOwner::Program { .. }
        ));
        let token = install_resource(&mut executor, Some(0), blocked);
        queue_cancellation(&mut executor, true);
        if blocked {
            let before = executor.snapshot().unwrap();
            let mut output = RuntimeStepOutput::default();
            assert!(!resume_many(&mut executor, true, &mut output));
            assert_eq!(output.diagnostics.len(), 1);
            assert_eq!(executor.snapshot().unwrap(), before);
            assert_eq!(executor.queued_task_events.len(), 1);
            executor
                .presentation
                .as_mut()
                .unwrap()
                .retirements_mut()
                .clear();
        }
        let mut output = RuntimeStepOutput::default();
        assert!(resume_many(&mut executor, true, &mut output));
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(
            executor.child_fibers[0].fiber.status,
            FiberStatus::Cancelled
        );
        assert!(executor.queued_task_events.is_empty());
        assert_retired(&executor, &token);
        executor.snapshot().unwrap();
        if !blocked {
            roundtrip(executor);
        }
    }
}

fn fanout_fixture(
    nested: bool,
) -> (
    AwbcProductStepExecutor,
    crate::runtime_id::RuntimeFiberInstanceId,
    [crate::task::TaskCorrelation; 2],
) {
    let bytes: &[u8] = if nested {
        include_bytes!("fanout-need-cancellation.awbc")
    } else {
        include_bytes!("fanout-cancellation.awbc")
    };
    let program = AwbcProgram::decode_canonical(bytes, Default::default()).unwrap();
    program
        .verify(Default::default(), Default::default())
        .unwrap();
    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
    for _ in 0..128 {
        let output = executor.step(Default::default(), RuntimeStepOptions::default());
        assert!(
            output.output.diagnostics.is_empty(),
            "{:?}",
            output.output.diagnostics
        );
        assert!(
            output.output.requests.tasks.is_empty(),
            "LocalOnly Source never requires Host execution"
        );
        if let Some((controller, flights)) = executor.child_fibers.iter().find_map(|child| {
            let FiberSuspensionReason::ProgramFanout(state) =
                &child.fiber.suspension.as_ref()?.reason
            else {
                return None;
            };
            (state.in_flight.len() == 2).then(|| {
                (
                    child.fiber.instance,
                    [
                        state.in_flight[0].handle.correlation(),
                        state.in_flight[1].handle.correlation(),
                    ],
                )
            })
        }) {
            return (executor, controller, flights);
        }
    }
    panic!("genuine decoded Source did not start both Fanout workers");
}

fn worker_index(
    executor: &AwbcProductStepExecutor,
    correlation: crate::task::TaskCorrelation,
) -> usize {
    executor.child_fibers.iter().position(|child| matches!(&child.owner,
        ProductChildFiberOwner::Program { launch, .. } if launch.handle().correlation == correlation)).unwrap()
}

fn step_child_at(executor: &mut AwbcProductStepExecutor, index: usize) {
    let child = executor.child_fibers.remove(index).unwrap();
    executor.child_fibers.push_front(child);
    let mut output = RuntimeStepOutput::default();
    assert!(executor.step_next_child(
        &mut output,
        &mut crate::pure::VmRuntimePureCallBackend::default(),
        &mut RuntimeStepInput::default(),
        &mut Vec::new()
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

fn cancel_worker(
    executor: &mut AwbcProductStepExecutor,
    correlation: crate::task::TaskCorrelation,
) {
    let index = worker_index(executor, correlation);
    let output = executor
        .program
        .cancel_fiber(
            &mut executor.child_fibers[index].fiber,
            &mut executor.presentation,
        )
        .unwrap();
    assert!(output.observations.is_empty());
    step_child_at(executor, index);
    assert!(matches!(
        executor
            .need_producers
            .launch_for_correlation(&correlation)
            .unwrap()
            .state(),
        crate::task::RuntimeNeedProducerState::Cancelled
    ));
}

fn install_bool_resource(
    executor: &mut AwbcProductStepExecutor,
    index: usize,
    blocked: bool,
) -> RuntimePresentationHandleToken {
    let expected = executor
        .program
        .runtime_types
        .iter()
        .find(|ty| matches!(ty.shape(), AwbcRuntimeTypeShape::Bool))
        .unwrap()
        .semantic_identity();
    install_resource_values(
        executor,
        Some(index),
        blocked,
        expected,
        RuntimeValue::Bool(true),
        RuntimeValue::Bool(false),
    )
}

fn assert_bool_retired(executor: &AwbcProductStepExecutor, token: &RuntimePresentationHandleToken) {
    let retired = executor.presentation.as_ref().unwrap().retirements();
    assert_eq!(retired.len(), 1);
    assert_eq!(&retired[0].token, token);
    assert_eq!(retired[0].body.value.0, RuntimeValue::Bool(true));
}

#[test]
fn fanout_cancellation_preflights_the_installed_owner_before_an_earlier_ready_value_moves() {
    for blocked in [false, true] {
        let (mut executor, controller, flights) = fanout_fixture(false);
        for _ in 0..64 {
            if matches!(
                executor
                    .need_producers
                    .launch_for_correlation(&flights[0])
                    .unwrap()
                    .state(),
                crate::task::RuntimeNeedProducerState::Ready(_)
            ) {
                break;
            }
            let index = worker_index(&executor, flights[0]);
            step_child_at(&mut executor, index);
        }
        assert!(matches!(
            executor
                .need_producers
                .launch_for_correlation(&flights[0])
                .unwrap()
                .state(),
            crate::task::RuntimeNeedProducerState::Ready(_)
        ));
        cancel_worker(&mut executor, flights[1]);
        let index = executor
            .child_fibers
            .iter()
            .position(|child| child.fiber.instance == controller)
            .unwrap();
        let token = install_bool_resource(&mut executor, index, blocked);
        let before = executor.snapshot().unwrap();
        let mut child = executor.child_fibers.remove(index).unwrap();
        let resume = child
            .fiber
            .suspension
            .as_ref()
            .unwrap()
            .declared_resume()
            .unwrap();
        let mut journal = DeferredChildResumeJournal::capture(&executor);
        let mut output = RuntimeStepOutput::default();
        if blocked {
            let error = executor
                .resume_program_fanout_for_child(&mut child, resume, &mut output, &mut journal)
                .unwrap_err();
            assert!(matches!(
                error,
                ProductStepError::Fiber(crate::awbc::fiber::FiberStateError::Snapshot(
                    crate::value::AwbcRuntimeValueSnapshotError::Presentation(
                        crate::presentation::RuntimePresentationError::RetirementCapacity
                    )
                ))
            ));
            assert!(!journal.owner_value_transferred);
            assert!(matches!(
                executor
                    .need_producers
                    .launch_for_correlation(&flights[0])
                    .unwrap()
                    .state(),
                crate::task::RuntimeNeedProducerState::Ready(_)
            ));
            executor.child_fibers.insert(index, child);
            assert_eq!(executor.snapshot().unwrap(), before);
            executor
                .presentation
                .as_mut()
                .unwrap()
                .retirements_mut()
                .clear();
            child = executor.child_fibers.remove(index).unwrap();
        }
        assert!(
            executor
                .resume_program_fanout_for_child(&mut child, resume, &mut output, &mut journal)
                .unwrap()
        );
        assert_eq!(child.fiber.status, FiberStatus::Cancelled);
        assert!(journal.owner_value_transferred);
        assert!(matches!(
            executor
                .need_producers
                .launch_for_correlation(&flights[0])
                .unwrap()
                .state(),
            crate::task::RuntimeNeedProducerState::ReadyTransferred
        ));
        executor.child_fibers.insert(index, child);
        journal.commit(&mut executor, output, &mut RuntimeStepOutput::default());
        assert_bool_retired(&executor, &token);
        executor.snapshot().unwrap();
        if !blocked {
            roundtrip(executor);
        }
    }
}

#[test]
fn genuine_nested_program_need_cancellation_preserves_its_publication_until_scope_retirement_is_admitted()
 {
    for blocked in [false, true] {
        let (mut executor, _, flights) = fanout_fixture(true);
        let mut awaited = None;
        for _ in 0..64 {
            let index = worker_index(&executor, flights[0]);
            if let Some(FiberSuspensionReason::Await {
                target: crate::awbc::fiber::FiberAwaitTarget::Need { need, .. },
                ..
            }) = executor.child_fibers[index]
                .fiber
                .suspension
                .as_ref()
                .map(|suspension| &suspension.reason)
            {
                awaited = Some(need.correlation());
                break;
            }
            step_child_at(&mut executor, index);
        }
        cancel_worker(
            &mut executor,
            awaited.expect("real Source worker awaits its actual nested Program"),
        );
        let index = worker_index(&executor, flights[0]);
        let token = install_bool_resource(&mut executor, index, blocked);
        let before = executor.snapshot().unwrap();
        let mut journal = DeferredChildResumeJournal::capture(&executor);
        let mut child = executor.child_fibers.remove(index).unwrap();
        let mut output = RuntimeStepOutput::default();
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        if blocked {
            assert!(
                !executor
                    .resume_deferred_child_suspension(
                        &mut child,
                        &mut Vec::new(),
                        &mut Vec::new(),
                        &mut output,
                        &mut journal,
                        &mut backend
                    )
                    .unwrap()
            );
            assert_eq!(output.diagnostics.len(), 1);
            assert!(!journal.owner_value_transferred);
            executor.child_fibers.insert(index, child);
            assert_eq!(executor.snapshot().unwrap(), before);
            executor
                .presentation
                .as_mut()
                .unwrap()
                .retirements_mut()
                .clear();
            child = executor.child_fibers.remove(index).unwrap();
            output = RuntimeStepOutput::default();
        }
        assert!(
            executor
                .resume_deferred_child_suspension(
                    &mut child,
                    &mut Vec::new(),
                    &mut Vec::new(),
                    &mut output,
                    &mut journal,
                    &mut backend
                )
                .unwrap()
        );
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(child.fiber.status, FiberStatus::Cancelled);
        executor.child_fibers.insert(index, child);
        journal.commit(&mut executor, output, &mut RuntimeStepOutput::default());
        assert_bool_retired(&executor, &token);
        executor.snapshot().unwrap();
        if !blocked {
            roundtrip(executor);
        }
    }
}
