//! Source Action effects retain typed message custody in their real sessions.

use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{
        product_step::{AwbcProductExecutorSaveSnapshot, AwbcProductStepExecutor},
        schema::{AwbcEntryId, AwbcProgram},
    },
    engine::{Engine, FlowExit, FlowFiberStatus},
    step::{RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

fn options() -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        ..Default::default()
    }
}

fn compile(body: &str) -> (arcweft_compiler::types::CompiledSource, AwbcProgram) {
    let source = format!(
        "pub action @action:.feedback.submit submit(value: i64)\n{body}\nentry cli @entry.main {{ goto @flow.main }}\n"
    );
    let compiled = compile_source(&source).unwrap_or_else(|error| panic!("{error:?}"));
    let program = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "action_channel.arcw",
    )
    .lower()
    .unwrap()
    .program;
    (compiled, program)
}

fn assert_source_emission(body: &str) {
    let (compiled, program) = compile(body);
    let encoded = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, Default::default()).unwrap();
    let [flow] = compiled.plan.flows() else {
        panic!("one source Flow");
    };
    let flow = flow.id.clone();
    let mut native = Engine::for_flow(compiled.plan, &flow).unwrap();
    for _ in 0..64 {
        let result = native.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        assert!(result.output.effects.line.is_empty());
        assert!(result.output.requests.tasks.is_empty());
        if matches!(native.fiber().frame.status, FlowFiberStatus::Done(_)) {
            break;
        }
    }
    assert_eq!(
        native.fiber().frame.status,
        FlowFiberStatus::Done(FlowExit::Return("42".into()))
    );

    let mut product = AwbcProductStepExecutor::for_entry(decoded, AwbcEntryId(0), 64).unwrap();
    let mut finished = false;
    let mut messages = 0;
    for _ in 0..64 {
        let result = product.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        assert!(result.output.effects.line.is_empty());
        assert!(result.output.requests.tasks.is_empty());
        let image = product.snapshot().unwrap();
        let saved =
            AwbcProductExecutorSaveSnapshot::from_live(&image, product.admitted_program()).unwrap();
        assert!(saved.actions.len() >= messages);
        messages = saved.actions.len();
        assert!(messages <= 1, "one authored emission must not replay");
        for (key, declaration, payload) in &saved.actions {
            let arcweft_core::value::AwbcRuntimeValueSnapshot::NominalRecord(record) = payload
            else {
                panic!("original nominal payload");
            };
            assert_eq!(*key, record.semantic_identity);
            let program = product.program();
            let row = &program.runtime_types[program.semantic_type_id(*key).unwrap().index()];
            assert_eq!(Some(declaration), row.nominal_declaration());
            assert_eq!(
                row.nominal_declaration()
                    .unwrap()
                    .action()
                    .unwrap()
                    .as_str(),
                "action.feedback.submit"
            );
            assert_eq!(record.fields.len(), 1);
            assert_eq!(
                record.fields[0].value,
                arcweft_core::value::AwbcRuntimeValueSnapshot::Int(
                    arcweft_core::value::RuntimeInt::I64(42)
                )
            );
        }
        let saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        finished = matches!(result.fiber_status, FlowFiberStatus::Done(_));
        product = product
            .restore_inert_snapshot_owned(saved)
            .unwrap_or_else(|(_, error)| panic!("Action save restore: {error}"));
        if finished {
            assert_eq!(
                result.fiber_status,
                FlowFiberStatus::Done(FlowExit::Return("42".into()))
            );
            break;
        }
    }
    assert!(finished, "bounded Product execution finishes");
    assert_eq!(
        messages, 1,
        "the emitted payload remains in the actual session save"
    );
}

#[test]
fn source_action_emission_reaches_native_and_decoded_product_without_host_effects() {
    assert_source_emission("flow main() -> i64 { emit(submit(value = 42i64)); return 42i64 }");
}

#[test]
fn external_typed_action_ingress_uses_the_original_fifo_in_every_executor_tier() {
    use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
    use arcweft_core::value::{RuntimeInt, RuntimeValue};
    let (compiled, program) = compile(
        "flow main() -> i64 { let a = await receive(submit); let b = await receive(submit); return a.value * 1000i64 + b.value }",
    );
    let row = program
        .runtime_types
        .iter()
        .find(|row| {
            row.nominal_declaration()
                .and_then(|declaration| declaration.action())
                .is_some()
        })
        .unwrap();
    let payload_type = row.semantic_identity();
    let plan = std::sync::Arc::new(compiled.plan);
    let mut executors = [
        ArcweftRuntimeExecutor::from_runtime_plan(
            plan.clone(),
            ArcweftExecutionTier::RuntimePlanVm,
        )
        .unwrap(),
        ArcweftRuntimeExecutor::from_runtime_plan(plan, ArcweftExecutionTier::StructuredAot)
            .unwrap(),
        ArcweftRuntimeExecutor::from_awbc_product(program.clone(), AwbcEntryId(0)).unwrap(),
    ];
    for executor in &mut executors {
        if executor.tier() != ArcweftExecutionTier::AwbcProduct {
            executor
                .start_structured_entry(
                    &arcweft_core::plan::EntryRuntimeId::from_source_entity_body("entry.main")
                        .unwrap(),
                )
                .unwrap();
        }
        let payload = |value| {
            executor
                .program_owner()
                .types()
                .try_record_value(
                    payload_type,
                    vec![RuntimeValue::Int(RuntimeInt::I64(value))],
                    arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
                )
                .unwrap()
        };
        let first = payload(1);
        let second = payload(2);
        executor.emit_action(payload_type, first).unwrap();
        assert!(
            executor
                .emit_action(payload_type, RuntimeValue::Bool(false))
                .is_err()
        );
        executor.emit_action(payload_type, second).unwrap();
        let mut finished = false;
        for _ in 0..64 {
            let step = executor.step(RuntimeStepInput::default(), options());
            assert!(
                step.output.diagnostics.is_empty(),
                "{:?}",
                step.output.diagnostics
            );
            assert!(step.output.requests.host_calls.is_empty());
            if matches!(step.fiber_status, FlowFiberStatus::Done(_)) {
                assert_eq!(
                    step.fiber_status,
                    FlowFiberStatus::Done(FlowExit::Return("1002".into()))
                );
                finished = true;
                break;
            }
        }
        assert!(finished, "{:?}", executor.tier());
    }
}

#[test]
fn source_action_emission_in_an_effectful_function_keeps_its_original_payload() {
    assert_source_emission(
        "fn send(value: submit) -> Unit effects { action.emit } { emit(value) }\nflow main() -> i64 { send(submit(value = 42i64)); return 42i64 }",
    );
}

fn assert_source_receive(body: &str, expected: &str, require_wait: bool) {
    use arcweft_core::awbc::fiber::FiberSuspensionReason;
    let (compiled, program) = compile(body);
    let decoded =
        AwbcProgram::decode_canonical(&program.encode_canonical().unwrap(), Default::default())
            .unwrap();
    let flow = compiled.plan.flows()[0].id.clone();
    let mut native = Engine::for_flow(compiled.plan, &flow).unwrap();
    let mut native_program_launches = 0;
    for _ in 0..512 {
        let result = native.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "Native: {:?}",
            result.output.diagnostics
        );
        native_program_launches += result.output.requests.tasks.len();
        assert!(result.output.requests.tasks.iter().all(|task| matches!(
            task.spec().request,
            arcweft_core::task::TaskRequest::Program(_)
        )));
        assert!(result.output.effects.line.is_empty());
        if matches!(native.fiber().frame.status, FlowFiberStatus::Done(_)) {
            break;
        }
    }
    assert_eq!(
        native.fiber().frame.status,
        FlowFiberStatus::Done(FlowExit::Return(expected.into()))
    );
    assert_eq!(native_program_launches, usize::from(require_wait));
    let mut product = AwbcProductStepExecutor::for_entry(decoded, AwbcEntryId(0), 64).unwrap();
    let mut waited = false;
    let mut finished = false;
    let mut product_program_launches = 0;
    for _ in 0..512 {
        let result = product.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "Product: {:?}",
            result.output.diagnostics
        );
        product_program_launches += result.output.requests.tasks.len();
        assert!(result.output.requests.tasks.iter().all(|task| matches!(
            task.spec().request,
            arcweft_core::task::TaskRequest::Program(_)
        )));
        assert!(result.output.effects.line.is_empty());
        let image = product.snapshot().unwrap();
        waited |= image.child_fibers.iter().any(|child| {
            child.fiber.suspension.as_ref().is_some_and(|suspension| {
                matches!(
                    suspension.reason,
                    FiberSuspensionReason::ActionReceive { .. }
                )
            })
        });
        let saved =
            AwbcProductExecutorSaveSnapshot::from_live(&image, product.admitted_program()).unwrap();
        let saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        finished = matches!(result.fiber_status, FlowFiberStatus::Done(_));
        product = product
            .restore_inert_snapshot_owned(saved)
            .unwrap_or_else(|(_, error)| panic!("receive cold restore: {error}"));
        if finished {
            assert_eq!(
                result.fiber_status,
                FlowFiberStatus::Done(FlowExit::Return(expected.into()))
            );
            break;
        }
    }
    assert!(finished, "bounded Product receive finishes");
    assert_eq!(product_program_launches, usize::from(require_wait));
    assert!(
        !require_wait || waited,
        "an actual receiver must park before the sender publishes"
    );
}

#[test]
fn source_action_receive_consumes_the_exact_emitted_payload() {
    assert_source_receive(
        "flow main() -> i64 { emit(submit(value = 42i64)); let received = await receive(submit); return received.value }",
        "42",
        false,
    );
}

#[test]
fn source_action_receive_is_fifo_across_consecutive_needs() {
    assert_source_receive(
        "flow main() -> i64 { emit(submit(value = 20i64)); emit(submit(value = 22i64)); let a = await receive(submit); let b = await receive(submit); return a.value * 100i64 + b.value }",
        "2022",
        false,
    );
}

#[test]
fn source_action_receiver_waits_for_a_thread_sender_and_survives_cold_restore() {
    assert_source_receive(
        "flow main() -> i64 { let pending = receive(submit); thread sender { let a = 1i64; let b = 2i64; let c = a + b; emit(submit(value = 42i64)) }; let received = await pending; return received.value }",
        "42",
        true,
    );
}

#[test]
fn source_action_receive_captures_an_aliased_channel_in_a_function() {
    assert_source_receive(
        "fn next() -> Need<submit> effects { action.receive } { let channel = submit; receive(channel) }\nflow main() -> i64 { emit(submit(value = 42i64)); let received = await next(); return received.value }",
        "42",
        false,
    );
}

#[test]
fn source_action_receive_keeps_equal_shaped_channels_separate() {
    assert_source_receive(
        "pub action @action:.feedback.other other(value: i64)\nflow main() -> i64 { emit(other(value = 99i64)); emit(submit(value = 42i64)); let received = await receive(submit); let unrelated = await receive(other); return received.value * 100i64 + unrelated.value }",
        "4299",
        false,
    );
}

fn parked_receiver() -> AwbcProductStepExecutor {
    use arcweft_core::awbc::fiber::FiberSuspensionReason;
    let (_, program) = compile(
        "flow main() -> i64 { let received = await receive(submit); return received.value }",
    );
    let program =
        AwbcProgram::decode_canonical(&program.encode_canonical().unwrap(), Default::default())
            .unwrap();
    let mut product = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
    for _ in 0..64 {
        let result = product.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        let image = product.snapshot().unwrap();
        if matches!(result.fiber_status, FlowFiberStatus::NeedWaiting(_))
            && image.child_fibers.iter().any(|child| {
                child.fiber.suspension.as_ref().is_some_and(|suspension| {
                    matches!(
                        suspension.reason,
                        FiberSuspensionReason::ActionReceive { .. }
                    )
                })
            })
        {
            return product;
        }
    }
    panic!("the original receiver must park");
}

#[test]
fn empty_action_receive_stays_blocked_without_replaying_or_requesting_host_work() {
    use arcweft_core::step::RuntimeStepStopReason;
    let mut product = parked_receiver();
    for _ in 0..8 {
        let result = product.step(RuntimeStepInput::default(), options());
        assert_eq!(result.stop_reason, RuntimeStepStopReason::Blocked);
        assert_eq!(result.stats.executed_ops, 0);
        assert!(result.output.diagnostics.is_empty());
        assert!(result.output.requests.tasks.is_empty());
        let saved = AwbcProductExecutorSaveSnapshot::from_live(
            &product.snapshot().unwrap(),
            product.admitted_program(),
        )
        .unwrap();
        assert!(saved.actions.is_empty());
        let saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        product = product
            .restore_inert_snapshot_owned(saved)
            .unwrap_or_else(|(_, error)| panic!("parked receive restore: {error}"));
    }
}

#[test]
fn altered_action_wait_key_or_source_is_rejected_without_replacing_the_session() {
    use arcweft_core::awbc::fiber::FiberSuspensionReason;
    let mut product = parked_receiver();
    let saved = AwbcProductExecutorSaveSnapshot::from_live(
        &product.snapshot().unwrap(),
        product.admitted_program(),
    )
    .unwrap();
    for change_key in [true, false] {
        let mut forged = saved.clone();
        let child = forged
            .child_fibers
            .iter_mut()
            .find(|child| {
                child.fiber.suspension.as_ref().is_some_and(|suspension| {
                    matches!(
                        suspension.reason,
                        FiberSuspensionReason::ActionReceive { .. }
                    )
                })
            })
            .unwrap();
        if change_key {
            let FiberSuspensionReason::ActionReceive { payload } =
                &mut child.fiber.suspension.as_mut().unwrap().reason
            else {
                unreachable!()
            };
            *payload = arcweft_core::pattern::RuntimeSemanticTypeId::from_bytes([255; 32]);
        } else {
            child.fiber.cursor.instruction_offset += 1;
        }
        let (unchanged, _) = product
            .restore_inert_snapshot_owned(forged)
            .expect_err("the original instruction owns this wait");
        product = unchanged;
        let after = AwbcProductExecutorSaveSnapshot::from_live(
            &product.snapshot().unwrap(),
            product.admitted_program(),
        )
        .unwrap();
        assert_eq!(after, saved);
    }
}

#[test]
fn cancellation_of_a_source_parked_action_receiver_never_fabricates_a_payload() {
    use arcweft_core::{
        awbc::{
            fiber::{FiberStatus, FiberSuspensionReason, FiberTerminalValue},
            vm,
        },
        task::RuntimeProgramOwner,
    };
    let product = parked_receiver();
    let program = std::sync::Arc::new(product.program().clone());
    let saved = AwbcProductExecutorSaveSnapshot::from_live(
        &product.snapshot().unwrap(),
        product.admitted_program(),
    )
    .unwrap();
    drop(product);
    let child = saved
        .child_fibers
        .into_iter()
        .find(|child| {
            child.fiber.suspension.as_ref().is_some_and(|suspension| {
                matches!(
                    suspension.reason,
                    FiberSuspensionReason::ActionReceive { .. }
                )
            })
        })
        .unwrap();
    let mut fiber = child
        .fiber
        .into_live_for_program(&RuntimeProgramOwner::Awbc(std::sync::Arc::clone(&program)))
        .unwrap();
    let cancelled = vm::cancel_fiber(&program, &mut fiber).unwrap();
    assert_eq!(cancelled.executed, 0);
    assert!(cancelled.observations.is_empty());
    assert_eq!(fiber.status, FiberStatus::Cancelled);
    assert!(fiber.suspension.is_none());
    assert!(matches!(
        fiber.terminal,
        Some(FiberTerminalValue::Cancelled)
    ));
    assert!(
        vm::cancel_fiber(&program, &mut fiber)
            .unwrap()
            .observations
            .is_empty()
    );
}

#[test]
fn action_emission_without_session_authority_preserves_the_original_operand_register() {
    use arcweft_core::{
        awbc::{
            fiber::{AwbcFiberStateSnapshot, FiberState},
            product_step::AwbcProductProgram,
            vm::{self, VmExecutionContext, VmStepOptions},
        },
        effect::RuntimeArtifactFingerprint,
        entry::RuntimeSchemaLimits,
        task::RuntimeProgramOwner,
        value::RuntimeRetainedValueAdmission,
    };
    use std::sync::Arc;
    let (_, program) = compile("flow main() -> i64 { emit(submit(value = 42i64)); return 42i64 }");
    let artifact = RuntimeArtifactFingerprint::try_from_bytes(
        *blake3::hash(&program.encode_canonical().unwrap()).as_bytes(),
    )
    .unwrap();
    let program = Arc::new(program);
    let admitted = AwbcProductProgram::admit(
        Arc::clone(&program),
        Default::default(),
        Default::default(),
        Default::default(),
        None,
    )
    .unwrap();
    let context = VmExecutionContext::for_program(artifact, admitted);
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 65536).unwrap();
    let mut refused = false;
    for _ in 0..64 {
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            RuntimeProgramOwner::Awbc(Arc::clone(&program)),
            RuntimeSchemaLimits::engine_default(),
        );
        let before = AwbcFiberStateSnapshot::from_live(&fiber, &mut admission).unwrap();
        match vm::step_with_host_context(
            &program,
            &mut fiber,
            VmStepOptions {
                max_instructions: 1,
            },
            &context,
            &mut vm::RejectingVmHost,
        ) {
            Err(vm::VmError::ActionContextRequired) => {
                let mut admission = RuntimeRetainedValueAdmission::for_program(
                    RuntimeProgramOwner::Awbc(Arc::clone(&program)),
                    RuntimeSchemaLimits::engine_default(),
                );
                let after = AwbcFiberStateSnapshot::from_live(&fiber, &mut admission).unwrap();
                assert_eq!(before.cursor, after.cursor);
                assert_eq!(before.frames, after.frames);
                refused = true;
                break;
            }
            Err(error) => panic!("unexpected refusal: {error}"),
            Ok(_) => {}
        }
    }
    assert!(
        refused,
        "a program context alone cannot lend the session's mailbox"
    );
}

#[test]
fn changed_action_declaration_cannot_replace_a_session_holding_its_original_message() {
    let (_, program) = compile("flow main() -> i64 { emit(submit(value = 42i64)); return 42i64 }");
    let mut product = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
    let mut queued = false;
    for _ in 0..64 {
        let result = product.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        let image = product.snapshot().unwrap();
        let saved =
            AwbcProductExecutorSaveSnapshot::from_live(&image, product.admitted_program()).unwrap();
        if saved.actions.len() == 1 {
            queued = true;
            break;
        }
    }
    assert!(queued);
    let before = product.snapshot().unwrap();
    let mut changed = product.program().clone();
    let index = changed
        .runtime_types
        .iter()
        .position(|row| {
            row.nominal_declaration()
                .is_some_and(|declaration| declaration.action().is_some())
        })
        .unwrap();
    let declaration = changed.runtime_types[index].nominal_declaration().unwrap();
    let changed_declaration = arcweft_core::entry::RuntimeNominalDeclarationId::try_action(
        *declaration.as_bytes(),
        arcweft_id::PublicId::try_new_engine_owned("action.feedback.other").unwrap(),
    )
    .unwrap();
    // Keep the same semantic type, producer digest, field shape and layout;
    // the original program proof must still reject the substituted Action.
    changed.runtime_types[index] = changed.runtime_types[index]
        .clone()
        .with_nominal_declaration(changed_declaration);
    assert!(product.replace_program_preserving_state(changed).is_err());
    assert_eq!(product.snapshot().unwrap(), before);
}
