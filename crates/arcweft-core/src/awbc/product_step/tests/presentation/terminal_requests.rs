use super::cancellation_resume::{
    awaiting_many, fanout_fixture, install_bool_resource, install_resource, step_child_at,
    worker_index,
};
use super::trap_lifecycle::roundtrip;
use super::*;
use crate::awbc::fiber::{FiberAwaitTarget, FiberSuspensionReason, FiberTerminalValue};
use crate::awbc::schema::*;
use crate::presentation::RuntimePresentationError;
use crate::step::{
    RuntimeHostCallError, RuntimeHostCallErrorKind, RuntimeHostCallResult, RuntimeStepOutput,
};

fn one_op() -> RuntimeStepOptions {
    RuntimeStepOptions {
        budget: crate::step::RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    }
}

fn control_fixture(
    kind: AwbcEffectKind,
    blocked: bool,
) -> (AwbcProductStepExecutor, Vec<RuntimePresentationHandleToken>) {
    let mut program = super::traps::nested_program();
    let effect = push_effect_plan(&mut program, kind);
    program.instructions.push(AwbcInstruction::EmitEffect {
        effect,
        args: vec![],
    });
    program.blocks[2].instructions = AwbcTableRange::new(0, 2);
    program.canonicalize_string_table();
    super::traps::nested_fixture_with_program(program, blocked)
}

#[test]
fn control_failures_retire_actual_nested_scopes_and_roundtrip_the_public_owner() {
    for kind in [
        AwbcEffectKind::Panic,
        AwbcEffectKind::Fail,
        AwbcEffectKind::Bail,
        AwbcEffectKind::Goto,
    ] {
        let (mut executor, tokens) = control_fixture(kind, false);
        let output = executor.step(Default::default(), one_op());
        assert_eq!(
            executor.fiber.status,
            FiberStatus::Trapped,
            "{kind:?}: {:?}",
            output.output.diagnostics
        );
        let Some(FiberTerminalValue::Trapped(trap)) = &executor.fiber.terminal else {
            unreachable!()
        };
        assert_eq!(
            trap.code,
            if kind == AwbcEffectKind::Goto {
                AwbcTrapCode::MissingDynamicTarget
            } else {
                AwbcTrapCode::ExplicitPanic
            }
        );
        assert_eq!(
            executor
                .presentation
                .as_ref()
                .unwrap()
                .retirements()
                .iter()
                .map(|item| item.token.clone())
                .collect::<Vec<_>>(),
            tokens
        );
        roundtrip(executor);
    }
}

#[test]
fn control_failure_capacity_refusal_restores_the_producing_instruction_before_effect_publication() {
    for kind in [
        AwbcEffectKind::Panic,
        AwbcEffectKind::Fail,
        AwbcEffectKind::Bail,
        AwbcEffectKind::Goto,
    ] {
        let (mut executor, tokens) = control_fixture(kind, true);
        let before = executor.snapshot().unwrap();
        for _ in 0..2 {
            let output = executor.step(Default::default(), one_op());
            assert_eq!(output.output.diagnostics.len(), 1, "{kind:?}");
            assert!(
                output.output.diagnostics[0]
                    .message
                    .contains(&RuntimePresentationError::RetirementCapacity.to_string())
            );
            assert!(output.output.effects.line.is_empty());
            assert!(output.output.flow_events.is_empty());
            assert!(output.output.requests.line_commands.is_empty());
            assert_eq!(executor.snapshot().unwrap(), before, "{kind:?}");
        }
        executor
            .presentation
            .as_mut()
            .unwrap()
            .retirements_mut()
            .clear();
        executor.step(Default::default(), one_op());
        assert_eq!(executor.fiber.status, FiberStatus::Trapped);
        assert_eq!(
            executor
                .presentation
                .as_ref()
                .unwrap()
                .retirements()
                .iter()
                .map(|item| item.token.clone())
                .collect::<Vec<_>>(),
            tokens
        );
    }
}

fn selected_step(
    executor: &mut AwbcProductStepExecutor,
    input: &mut RuntimeStepInput,
    needs: &mut Vec<RuntimeNeedState>,
) -> (bool, RuntimeStepOutput) {
    let mut output = RuntimeStepOutput::default();
    let progressed = executor.step_next_child(
        &mut output,
        &mut crate::pure::VmRuntimePureCallBackend::default(),
        input,
        needs,
    );
    (progressed, output)
}

fn assert_refused(output: &RuntimeStepOutput) {
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains(&RuntimePresentationError::RetirementCapacity.to_string())
    );
    assert!(output.flow_events.is_empty());
    assert!(output.requests.line_commands.is_empty());
    assert!(output.requests.host_calls.is_empty());
    assert!(output.requests.tasks.is_empty());
}

#[test]
fn genuine_program_await_many_fault_keeps_the_event_and_complete_checkpoint_until_unwind_is_admitted()
 {
    for blocked in [false, true] {
        let mut executor = awaiting_many(true);
        let instance = executor.child_fibers[0].fiber.instance;
        let ProductChildFiberOwner::Program { launch, .. } = &executor.child_fibers[0].owner else {
            unreachable!()
        };
        let producer = launch.handle().correlation;
        let token = install_resource(&mut executor, Some(0), blocked);
        let Some(FiberSuspensionReason::AwaitMany(state)) = executor.child_fibers[0]
            .fiber
            .suspension
            .as_ref()
            .map(|s| &s.reason)
        else {
            unreachable!()
        };
        let correlation = state.in_flight[0].handle.correlation();
        let failure = crate::task::RuntimeTaskFailure::new(
            crate::task::RuntimeTaskFailureKind::WorkerFailure,
            "worker fault",
        );
        let mut output = RuntimeStepOutput::default();
        executor.latch_task_events(
            vec![producer_task_event(
                correlation.generation,
                correlation,
                0,
                TaskEventKind::InfrastructureFailure(failure),
            )],
            &mut output,
        );
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let before = executor.snapshot().unwrap();
        if blocked {
            for _ in 0..2 {
                let (progressed, output) = selected_step(
                    &mut executor,
                    &mut RuntimeStepInput::default(),
                    &mut Vec::new(),
                );
                assert!(!progressed);
                assert_refused(&output);
                assert_eq!(executor.snapshot().unwrap(), before);
                assert_eq!(executor.queued_task_events.len(), 1);
            }
            executor
                .presentation
                .as_mut()
                .unwrap()
                .retirements_mut()
                .clear();
        }
        let (_, completed) = selected_step(
            &mut executor,
            &mut RuntimeStepInput::default(),
            &mut Vec::new(),
        );
        assert!(executor.queued_task_events.is_empty());
        assert_eq!(
            executor.presentation.as_ref().unwrap().retirements()[0].token,
            token
        );
        assert!(
            !executor
                .child_fibers
                .iter()
                .any(|child| child.fiber.instance == instance)
        );
        let fault = executor
            .need_producers
            .launch_for_correlation(&producer)
            .unwrap()
            .task_fault()
            .expect("ordinary Program retirement publishes its actual failure");
        assert_eq!(
            fault.kind,
            crate::task::RuntimeTaskFailureKind::WorkerFailure
        );
        assert!(fault.to_string().contains("worker fault"));
        if !blocked {
            let report = completed
                .requests
                .program_task_completions
                .into_iter()
                .find(|report| report.handle().correlation == producer)
                .expect("actual HostMetadata retirement emitted its authenticated completion");
            let task = executor
                .need_producers
                .task_admissions()
                .find(|task| task.handle().correlation == producer)
                .expect("the issuing journal retains the actual accepted Program receipt");
            let acknowledgement = report
                .bind_publication(
                    &task,
                    crate::task::TaskDispatchIdentity::new(
                        producer,
                        LogicalEpoch(7),
                        TaskSequence(0),
                    ),
                    crate::task::TaskPublicationRevision::FIRST,
                )
                .unwrap()
                .into_event();
            let mut acknowledged = RuntimeStepOutput::default();
            executor.latch_task_events(vec![acknowledgement], &mut acknowledged);
            assert!(
                acknowledged.diagnostics.is_empty(),
                "{:?}",
                acknowledged.diagnostics
            );
            roundtrip(executor);
        }
    }
}

#[test]
fn genuine_nested_need_rejects_a_foreign_publication_without_losing_its_resource_owner() {
    for blocked in [false, true] {
        let (mut executor, _, flights) = fanout_fixture(true);
        let mut awaited = None;
        for _ in 0..64 {
            let index = worker_index(&executor, flights[0]);
            if let Some(FiberSuspensionReason::Await {
                target: FiberAwaitTarget::Need { need, .. },
                ..
            }) = executor.child_fibers[index]
                .fiber
                .suspension
                .as_ref()
                .map(|s| &s.reason)
            {
                awaited = Some(need.correlation());
                break;
            }
            step_child_at(&mut executor, index);
        }
        let correlation = awaited.expect("real Source worker awaits its nested Program Need");
        let index = worker_index(&executor, flights[0]);
        let token = install_bool_resource(&mut executor, index, blocked);
        let child = executor.child_fibers.remove(index).unwrap();
        let instance = child.fiber.instance;
        executor.child_fibers.push_front(child);
        let before = executor.snapshot().unwrap();
        let mut needs = vec![RuntimeNeedState::new(correlation, None, Need::NotStarted)];
        if blocked {
            for _ in 0..2 {
                let (progressed, output) =
                    selected_step(&mut executor, &mut RuntimeStepInput::default(), &mut needs);
                assert!(!progressed);
                assert_refused(&output);
                assert_eq!(executor.snapshot().unwrap(), before);
                assert_eq!(needs.len(), 1);
            }
            executor
                .presentation
                .as_mut()
                .unwrap()
                .retirements_mut()
                .clear();
        }
        selected_step(&mut executor, &mut RuntimeStepInput::default(), &mut needs);
        let child = executor
            .child_fibers
            .iter()
            .find(|child| child.fiber.instance == instance)
            .unwrap();
        assert_eq!(child.fiber.status, FiberStatus::Trapped);
        assert_eq!(
            executor.presentation.as_ref().unwrap().retirements()[0].token,
            token
        );
        if !blocked {
            roundtrip(executor);
        }
    }
}

#[test]
fn real_deferred_host_failure_consumes_its_receipt_only_after_resource_unwind_is_admitted() {
    for kind in [
        RuntimeHostCallErrorKind::UnsupportedCapability,
        RuntimeHostCallErrorKind::Rejected,
        RuntimeHostCallErrorKind::Failed,
    ] {
        for blocked in [false, true] {
            let (mut executor, activation, _) =
                registered_root_defer_executor(defer_host_call_program());
            assert!(prepare_root_defer_child(&mut executor, &activation).0);
            let (_, output) = selected_step(
                &mut executor,
                &mut RuntimeStepInput::default(),
                &mut Vec::new(),
            );
            let [request] = output.requests.host_calls.as_slice() else {
                panic!("real deferred child emits its Host call")
            };
            let mut input = RuntimeStepInput {
                host_call_results: vec![RuntimeHostCallResult {
                    id: request.id.clone(),
                    outcome: Err(RuntimeHostCallError {
                        kind,
                        message: "host fault".into(),
                    }),
                }],
                ..Default::default()
            };
            let token = install_resource(&mut executor, Some(0), blocked);
            let before = executor.snapshot().unwrap();
            if blocked {
                for _ in 0..2 {
                    let (progressed, output) =
                        selected_step(&mut executor, &mut input, &mut Vec::new());
                    assert!(!progressed);
                    assert_refused(&output);
                    assert_eq!(executor.snapshot().unwrap(), before);
                    assert_eq!(input.host_call_results.len(), 1);
                    assert!(
                        matches!(&input.host_call_results[0].outcome, Err(error) if error.kind == kind && error.message == "host fault")
                    );
                }
                executor
                    .presentation
                    .as_mut()
                    .unwrap()
                    .retirements_mut()
                    .clear();
            }
            selected_step(&mut executor, &mut input, &mut Vec::new());
            assert!(input.host_call_results.is_empty());
            assert_eq!(
                executor.presentation.as_ref().unwrap().retirements()[0].token,
                token
            );
            executor.snapshot().unwrap();
            if !blocked {
                roundtrip(executor);
            }
        }
    }
}

fn activation_host_fixture(
    blocked: bool,
) -> (AwbcProductStepExecutor, RuntimePresentationHandleToken) {
    super::trap_lifecycle::dialogue_fixture_with_program(blocked, |program, entry| {
        let signature = AwbcSignatureId(program.signatures.len().try_into().unwrap());
        program.signatures.push(AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        });
        program.host_calls.push(AwbcHostCall {
            producer: crate::tests::host_producer(),
            public_id: AwbcStringId(0),
            capability: AwbcStringId(0),
            operation: AwbcStringId(0),
            contract: None,
            signature,
            mode: AwbcHostCallMode::Suspend,
            deterministic: true,
            arguments: Vec::new(),
        });
        let resume_block = AwbcBlockId((entry + 1).try_into().unwrap());
        program.blocks.insert(
            entry + 1,
            AwbcBlock {
                owner: AwbcFunctionId(1),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::HostCall,
                source_map: None,
            },
        );
        program.functions[1].blocks.len += 1;
        for function in program.functions.iter_mut().skip(2) {
            function.blocks.start += 1;
            function.entry_block.0 += 1;
        }
        let resume = AwbcResumePointId(program.resume_points.len().try_into().unwrap());
        program.resume_points.push(AwbcResumePoint {
            function: AwbcFunctionId(1),
            block: resume_block,
            frame_layout: program.functions[1].frame_layout,
            kind: AwbcSafePointKind::HostCall,
        });
        program.blocks[entry].terminator = AwbcTerminator::HostCall {
            call: AwbcHostCallId(0),
            args: Vec::new(),
            dst: None,
            resume,
        };
    })
}

#[test]
fn actual_activation_host_failure_retains_receipt_and_custody_until_unwind_is_admitted() {
    for kind in [
        RuntimeHostCallErrorKind::UnsupportedCapability,
        RuntimeHostCallErrorKind::Rejected,
        RuntimeHostCallErrorKind::Failed,
    ] {
        for blocked in [true, false] {
            let (mut executor, token) = activation_host_fixture(blocked);
            let issued = executor.step(Default::default(), one_op());
            assert!(
                issued.output.diagnostics.is_empty(),
                "{:?}",
                issued.output.diagnostics
            );
            let [request] = issued.output.requests.host_calls.as_slice() else {
                panic!("actual Dialogue initializer issues its suspended Host call")
            };
            let mut host_results = vec![RuntimeHostCallResult {
                id: request.id.clone(),
                outcome: Err(RuntimeHostCallError {
                    kind,
                    message: "activation host fault".into(),
                }),
            }];
            let before = executor.snapshot().unwrap();
            if blocked {
                for _ in 0..2 {
                    let mut transaction = executor.dialogues.begin_active_transaction().unwrap();
                    let result = executor.step_dialogue_activation_with_host_results(
                        &mut transaction,
                        &mut host_results,
                        &mut crate::pure::VmRuntimePureCallBackend::default(),
                        &mut crate::awbc::product_step::ProductStepBudgets::default(),
                    );
                    assert!(matches!(result, Err(ref error) if error.is_execution_refusal()));
                    executor.dialogues.restore_transaction(transaction).unwrap();
                    assert_eq!(executor.snapshot().unwrap(), before);
                    assert_eq!(host_results.len(), 1);
                    assert!(
                        matches!(&host_results[0].outcome, Err(error) if error.kind == kind && error.message == "activation host fault")
                    );
                }
                executor
                    .presentation
                    .as_mut()
                    .unwrap()
                    .retirements_mut()
                    .clear();
            }
            let mut transaction = executor.dialogues.begin_active_transaction().unwrap();
            let mut progress = executor
                .step_dialogue_activation_with_host_results(
                    &mut transaction,
                    &mut host_results,
                    &mut crate::pure::VmRuntimePureCallBackend::default(),
                    &mut crate::awbc::product_step::ProductStepBudgets::default(),
                )
                .unwrap();
            assert_eq!(progress.executed_ops, 1);
            assert!(
                progress.trap.is_none(),
                "receipt commit retains the existing one-operation timing"
            );
            let ticket = progress
                .host_result_take
                .take()
                .expect("actual failed Host receipt is staged");
            let mut batch = match progress.execution.take() {
                Some(batch) => batch,
                None => executor
                    .prepare_line_task_commands(&mut transaction, progress.reducer)
                    .unwrap(),
            };
            let prepared = executor
                .preflight_line_task_commands(
                    &mut transaction,
                    &batch,
                    crate::awbc::product_step::line::ProductLineTaskCommit::Commands,
                )
                .unwrap();
            let proof = executor.dialogues.inspect_commit(&transaction).unwrap();
            executor.commit_activation_host_result(&mut transaction, &mut host_results, ticket);
            executor.realize_line_task_commands(&mut transaction, &mut batch, prepared);
            executor.dialogues.commit_prepared(transaction, proof);
            executor.commit_line_task_commands(batch, &mut RuntimeStepOutput::default());
            assert!(host_results.is_empty());
            let retired = executor.presentation.as_ref().unwrap().retirements();
            assert_eq!(retired.len(), 1);
            assert_eq!(retired[0].token, token);
            let frame = executor.dialogues.active_frame().unwrap();
            assert!(frame.pending_activation_host_call.is_none());
            let ProductDialoguePhase::Activating { fiber, .. } = &frame.phase else {
                unreachable!()
            };
            let Some(FiberTerminalValue::Trapped(trap)) = fiber.terminal.as_ref() else {
                unreachable!()
            };
            assert_eq!(trap.message.as_deref(), Some("activation host fault"));
            assert_eq!(
                trap.code,
                if kind == RuntimeHostCallErrorKind::UnsupportedCapability {
                    AwbcTrapCode::CapabilityDenied
                } else {
                    AwbcTrapCode::HostAbiMismatch
                }
            );
            executor.snapshot().unwrap();
            if !blocked {
                executor = roundtrip(executor);
            }
            let closed = executor.step(Default::default(), one_op());
            assert!(closed.output.requests.host_calls.is_empty());
            assert_eq!(
                executor.presentation.as_ref().unwrap().retirements().len(),
                1
            );
        }
    }
}
