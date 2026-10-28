//! Logical Duration waits execute without host clocks or repeated operand evaluation.

use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{
        product_step::{AwbcProductExecutorSaveSnapshot, AwbcProductProgram},
        schema::AwbcProgram,
    },
    engine::{Engine, FlowExit, FlowFiberStatus},
    executor::{ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot, RuntimeExecutor},
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    time::LogicalDuration,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::sync::Arc;

fn native(body: &str) -> Engine {
    let source = format!("{body}\nentry cli @entry.main {{ goto @flow.main }}\n");
    let compiled = compile_source(&source).unwrap_or_else(|error| panic!("{source}: {error:?}"));
    let flow = compiled
        .plan
        .flows()
        .iter()
        .find(|flow| flow.id.public_label().as_str() == "flow.main")
        .unwrap()
        .id
        .clone();
    Engine::for_flow(compiled.plan, &flow).unwrap()
}

fn step(engine: &mut Engine, nanos: u64) -> arcweft_core::step::RuntimeStepResult {
    let result = engine.step(
        RuntimeStepInput {
            dt: LogicalDuration::from_nanos(nanos),
            ..Default::default()
        },
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 64 },
            ..Default::default()
        },
    );
    assert!(
        result.output.diagnostics.is_empty(),
        "{:?}",
        result.output.diagnostics
    );
    assert!(result.stats.executed_ops <= 64);
    result
}

fn remaining(engine: &Engine) -> u64 {
    let FlowFiberStatus::DurationWaiting(wait) = &engine.fiber().frame.status else {
        panic!(
            "expected a suspended Duration wait: {:?}",
            engine.fiber().frame.status
        )
    };
    wait.remaining().as_nanos()
}

fn finish(engine: &mut Engine) {
    for _ in 0..128 {
        if matches!(engine.fiber().frame.status, FlowFiberStatus::Done(_)) {
            assert_eq!(
                engine.fiber().frame.status,
                FlowFiberStatus::Done(FlowExit::Return("42".into()))
            );
            return;
        }
        step(engine, 0);
    }
    panic!(
        "execution did not finish: {:?}",
        engine.fiber().frame.status
    );
}

#[test]
fn native_computed_wait_consumes_only_later_step_durations() {
    for expression in ["0.1s", "delay()", "if true { delay() } else { 0.35s }"] {
        let mut engine = native(&format!(
            "fn delay() -> Duration {{ 0.1s }}\nflow main() -> i64 {{ wait({expression}); return 42i64 }}"
        ));
        step(&mut engine, u64::MAX);
        assert_eq!(remaining(&engine), 100_000_000);
        step(&mut engine, 0);
        assert_eq!(remaining(&engine), 100_000_000);
        step(&mut engine, 99_999_999);
        assert_eq!(remaining(&engine), 1);
        step(&mut engine, 1);
        finish(&mut engine);
    }
}

#[test]
fn native_wait_overshoot_does_not_age_a_later_wait() {
    let mut engine = native("flow main() -> i64 { wait(0.1s); wait(0.35s); return 42i64 }");
    step(&mut engine, 0);
    assert_eq!(remaining(&engine), 100_000_000);
    step(&mut engine, u64::MAX);
    assert_eq!(remaining(&engine), 350_000_000);
    step(&mut engine, 349_999_999);
    assert_eq!(remaining(&engine), 1);
    step(&mut engine, 2);
    finish(&mut engine);
}

#[test]
fn native_zero_wait_never_requires_elapsed_time() {
    let mut engine = native("flow main() -> i64 { wait(0s); return 42i64 }");
    finish(&mut engine);
}

#[test]
fn native_wait_ages_at_ingress_without_spending_a_zero_operation_budget() {
    let mut engine = native("flow main() -> i64 { wait(0.1s); return 42i64 }");
    for _ in 0..128 {
        if matches!(
            engine.fiber().frame.status,
            FlowFiberStatus::DurationWaiting(_)
        ) {
            break;
        }
        let result = engine.step(Default::default(), Default::default());
        assert!(result.output.diagnostics.is_empty());
        assert!(result.stats.executed_ops <= 1);
    }
    assert_eq!(remaining(&engine), 100_000_000);
    let result = engine.step(
        RuntimeStepInput {
            dt: LogicalDuration::from_nanos(99_999_999),
            ..Default::default()
        },
        RuntimeStepOptions {
            budget: RuntimeStepBudget { max_ops: 0 },
            ..Default::default()
        },
    );
    assert_eq!(result.stats.executed_ops, 0);
    assert_eq!(remaining(&engine), 1);
    let result = engine.step(
        RuntimeStepInput {
            dt: LogicalDuration::from_nanos(1),
            ..Default::default()
        },
        RuntimeStepOptions {
            budget: RuntimeStepBudget { max_ops: 0 },
            ..Default::default()
        },
    );
    assert_eq!(result.stats.executed_ops, 0);
    assert_eq!(engine.fiber().frame.status, FlowFiberStatus::Running);
    finish(&mut engine);
}

#[test]
fn native_child_wait_progresses_while_its_parent_has_a_longer_wait() {
    let mut engine = native(
        "flow main() -> i64 {\n thread worker { wait(0.035s); log.info(\"child\") }\n wait(0.1s)\n return 42i64\n}",
    );
    step(&mut engine, 0);
    assert_eq!(remaining(&engine), 100_000_000);
    let result = step(&mut engine, 35_000_000);
    assert_eq!(remaining(&engine), 65_000_000);
    assert_eq!(result.output.effects.line.iter().filter(|effect| matches!(effect, arcweft_core::effect::LineEffectRequest::Log(log) if log.message == "child")).count(), 1);
    step(&mut engine, 65_000_000);
    finish(&mut engine);
}

fn product(body: &str, selected: bool) -> ArcweftRuntimeExecutor {
    let source = format!("{body}\nentry cli @entry.main {{ goto @flow.main }}\n");
    let compiled = compile_source(&source).unwrap_or_else(|error| panic!("{source}: {error:?}"));
    let [entry] = compiled.plan.entries() else {
        panic!("one entry")
    };
    let arcweft_core::plan::RuntimeEntryTarget::Flow(flow) = &entry.target else {
        panic!("Flow entry")
    };
    let flow = flow.clone();
    let lowerer = if selected {
        AwbcLowerer::for_entry(
            &compiled.plan,
            &compiled.dialogue_content,
            "wait.arcw",
            &entry.id,
        )
    } else {
        AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "wait.arcw")
    };
    let program = lowerer.lower().unwrap().program;
    let invocation = compiled.plan.seal_flow_invocation(flow, []).unwrap();
    let bytes = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, Default::default()).unwrap();
    let admitted = AwbcProductProgram::admit(
        Arc::new(decoded),
        Default::default(),
        Default::default(),
        Default::default(),
        None,
    )
    .unwrap();
    ArcweftRuntimeExecutor::from_admitted_awbc_product_flow_invocation(admitted, invocation)
        .unwrap()
}

fn saved(runtime: &ArcweftRuntimeExecutor) -> AwbcProductExecutorSaveSnapshot {
    let ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot) = runtime.snapshot().unwrap();
    let saved = AwbcProductExecutorSaveSnapshot::from_live(
        &snapshot,
        runtime.admitted_product_awbc_program().unwrap(),
    )
    .unwrap();
    let bytes = serde_json::to_vec(&saved).unwrap();
    let decoded = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(saved, decoded);
    decoded
}

fn product_step(
    mut runtime: ArcweftRuntimeExecutor,
    nanos: u64,
) -> (
    ArcweftRuntimeExecutor,
    arcweft_core::step::RuntimeStepResult,
) {
    let result = runtime.step(
        RuntimeStepInput {
            dt: LogicalDuration::from_nanos(nanos),
            ..Default::default()
        },
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 64 },
            ..Default::default()
        },
    );
    assert!(
        result.output.diagnostics.is_empty(),
        "{:?}",
        result.output.diagnostics
    );
    assert!(result.stats.executed_ops <= 64);
    let snapshot = saved(&runtime);
    let bound = runtime.bind_product_save_snapshot(snapshot).unwrap();
    (
        runtime
            .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
            .unwrap(),
        result,
    )
}

fn product_remaining(runtime: &ArcweftRuntimeExecutor) -> u64 {
    let FlowFiberStatus::DurationWaiting(wait) = &runtime.fiber().frame.status else {
        panic!("expected Duration wait: {:?}", runtime.fiber().frame.status)
    };
    wait.remaining().as_nanos()
}

fn finish_product(runtime: ArcweftRuntimeExecutor) {
    finish_product_return(runtime, "42");
}

fn finish_product_return(mut runtime: ArcweftRuntimeExecutor, expected: &str) {
    for _ in 0..128 {
        if matches!(runtime.fiber().frame.status, FlowFiberStatus::Done(_)) {
            assert_eq!(
                runtime.fiber().frame.status,
                FlowFiberStatus::Done(FlowExit::Return(expected.into()))
            );
            return;
        }
        (runtime, _) = product_step(runtime, 0);
    }
    panic!(
        "Product wait did not resume: {:?}",
        runtime.fiber().frame.status
    );
}

#[test]
fn decoded_awbc_wait_preserves_remaining_time_through_each_step_public_save() {
    for selected in [false, true] {
        for expression in ["0.1s", "delay()", "if true { delay() } else { 0.35s }"] {
            let body = format!(
                "fn delay() -> Duration {{ 0.1s }}\nflow main() -> i64 {{ wait({expression}); return 42i64 }}"
            );
            let (mut runtime, _) = product_step(product(&body, selected), u64::MAX);
            assert_eq!(product_remaining(&runtime), 100_000_000);
            for (elapsed, expected) in [(0, 100_000_000), (99_999_999, 1)] {
                (runtime, _) = product_step(runtime, elapsed);
                assert_eq!(product_remaining(&runtime), expected);
            }
            (runtime, _) = product_step(runtime, 1);
            finish_product(runtime);
        }
    }
}

#[test]
fn decoded_awbc_zero_and_sequential_waits_keep_the_original_time_boundary() {
    for selected in [false, true] {
        finish_product(product(
            "flow main() -> i64 { wait(0s); return 42i64 }",
            selected,
        ));
        let (mut runtime, _) = product_step(
            product(
                "flow main() -> i64 { wait(0.1s); wait(0.35s); return 42i64 }",
                selected,
            ),
            0,
        );
        assert_eq!(product_remaining(&runtime), 100_000_000);
        (runtime, _) = product_step(runtime, u64::MAX);
        assert_eq!(product_remaining(&runtime), 350_000_000);
        (runtime, _) = product_step(runtime, 349_999_999);
        assert_eq!(product_remaining(&runtime), 1);
        (runtime, _) = product_step(runtime, 2);
        finish_product(runtime);
    }
}

#[test]
fn decoded_awbc_child_wait_can_finish_while_parent_waits_and_is_saved() {
    let body = "flow main() -> i64 {\n thread worker { wait(0.035s); log.info(\"child\") }\n wait(0.1s)\n return 42i64\n}";
    for selected in [false, true] {
        let (mut runtime, _) = product_step(product(body, selected), 0);
        assert_eq!(product_remaining(&runtime), 100_000_000);
        let result;
        (runtime, result) = product_step(runtime, 35_000_000);
        assert_eq!(product_remaining(&runtime), 65_000_000);
        assert_eq!(result.output.effects.line.iter().filter(|effect| matches!(effect, arcweft_core::effect::LineEffectRequest::Log(log) if log.message == "child")).count(), 1);
        (runtime, _) = product_step(runtime, 65_000_000);
        finish_product(runtime);
    }
}

#[test]
fn decoded_awbc_wait_cold_restore_uses_the_original_instruction_and_remainder() {
    let body = "flow main() -> i64 { wait(0.1s); return 42i64 }";
    for selected in [false, true] {
        let (mut runtime, _) = product_step(product(body, selected), 0);
        (runtime, _) = product_step(runtime, 40_000_000);
        let snapshot = saved(&runtime);
        drop(runtime);
        let fresh = product(body, selected);
        let bound = fresh.bind_product_save_snapshot(snapshot).unwrap();
        let restored = fresh
            .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
            .unwrap();
        assert_eq!(product_remaining(&restored), 60_000_000);
        let (runtime, _) = product_step(restored, 60_000_000);
        finish_product(runtime);
    }
}

#[test]
fn decoded_awbc_wait_rejects_forged_resume_status_and_source_without_mutation() {
    use arcweft_core::awbc::fiber::{FiberResumeTarget, FiberStatus, FiberSuspensionReason};
    let (mut runtime, _) = product_step(
        product("flow main() -> i64 { wait(0.1s); return 42i64 }", false),
        0,
    );
    let original = saved(&runtime);
    for case in 0..4 {
        let mut invalid = original.clone();
        match case {
            0 => {
                invalid.fiber.suspension.as_mut().unwrap().resume =
                    FiberResumeTarget::Exact(invalid.fiber.cursor)
            }
            1 => invalid.fiber.status = FiberStatus::Running,
            2 => {
                invalid.fiber.suspension.as_mut().unwrap().reason =
                    FiberSuspensionReason::BudgetYield
            }
            3 => invalid.fiber.cursor.instruction_offset += 1,
            _ => unreachable!(),
        }
        let bound = runtime.bind_product_save_snapshot(invalid).unwrap();
        let rejected = runtime
            .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
            .expect_err("forged Duration state must fail full restore admission");
        (runtime, _) = rejected.into_parts();
        assert_eq!(
            saved(&runtime),
            original,
            "rejection changed live state for forgery {case}"
        );
    }
    let (runtime, _) = product_step(runtime, 100_000_000);
    finish_product(runtime);
}

#[test]
fn decoded_awbc_wait_rejects_a_replaced_source_instruction_atomically() {
    use arcweft_core::awbc::schema::AwbcInstruction;
    let (mut runtime, _) = product_step(
        product("flow main() -> i64 { wait(0.1s); return 42i64 }", false),
        0,
    );
    let original = saved(&runtime);
    let mut replacement = runtime.product_awbc_program().unwrap().clone();
    let cursor = original.fiber.cursor;
    let instruction = (replacement.blocks[cursor.block.index()].instructions.start
        + cursor.instruction_offset) as usize;
    assert!(matches!(
        replacement.instructions[instruction],
        AwbcInstruction::EmitEffect { .. }
    ));
    replacement.instructions[instruction] = AwbcInstruction::Nop;
    replacement
        .verify(Default::default(), Default::default())
        .unwrap();
    assert!(runtime.replace_product_awbc_program(replacement).is_err());
    assert_eq!(saved(&runtime), original);
    let (runtime, _) = product_step(runtime, 100_000_000);
    finish_product(runtime);
}

#[test]
fn decoded_awbc_compatible_body_replacement_keeps_elapsed_wait_and_uses_new_tail() {
    for selected in [false, true] {
        let (mut runtime, _) = product_step(
            product("flow main() -> i64 { wait(0.1s); return 42i64 }", selected),
            0,
        );
        (runtime, _) = product_step(runtime, 40_000_000);
        let replacement = product("flow main() -> i64 { wait(0.2s); return 43i64 }", selected);
        runtime
            .replace_admitted_product_awbc_program(
                replacement.admitted_product_awbc_program().unwrap().clone(),
            )
            .unwrap();
        assert_eq!(product_remaining(&runtime), 60_000_000);
        (runtime, _) = product_step(runtime, 60_000_000);
        finish_product_return(runtime, "43");
    }
}

#[test]
fn decoded_awbc_wait_resumes_under_zero_budget_without_executing_its_tail() {
    for selected in [false, true] {
        let (mut runtime, _) = product_step(
            product("flow main() -> i64 { wait(0.1s); return 42i64 }", selected),
            0,
        );
        let result = runtime.step(
            RuntimeStepInput {
                dt: LogicalDuration::from_nanos(100_000_000),
                ..Default::default()
            },
            RuntimeStepOptions {
                budget: RuntimeStepBudget { max_ops: 0 },
                ..Default::default()
            },
        );
        assert!(result.output.diagnostics.is_empty());
        assert_eq!(result.stats.executed_ops, 0);
        assert_eq!(runtime.fiber().frame.status, FlowFiberStatus::Running);
        let bound = runtime.bind_product_save_snapshot(saved(&runtime)).unwrap();
        finish_product(
            runtime
                .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
                .unwrap(),
        );
    }
}

#[test]
fn effectful_duration_operand_is_evaluated_once_across_native_and_awbc_suspension() {
    let body = "fn delay() -> Duration effects { log.write } { log.info(\"operand\"); 0.1s }\nflow main() -> i64 { wait(delay()); return 42i64 }";
    let count = |result: &arcweft_core::step::RuntimeStepResult| {
        result.output.effects.line.iter().filter(|effect| matches!(effect, arcweft_core::effect::LineEffectRequest::Log(log) if log.message == "operand")).count()
    };
    let mut engine = native(body);
    assert_eq!(count(&step(&mut engine, 0)), 1);
    assert_eq!(count(&step(&mut engine, 50_000_000)), 0);
    assert_eq!(count(&step(&mut engine, 50_000_000)), 0);
    finish(&mut engine);
    for selected in [false, true] {
        let (mut runtime, result) = product_step(product(body, selected), 0);
        assert_eq!(count(&result), 1);
        for dt in [50_000_000, 50_000_000] {
            let result;
            (runtime, result) = product_step(runtime, dt);
            assert_eq!(count(&result), 0);
        }
        finish_product(runtime);
    }
}

#[test]
fn standalone_awbc_cancellation_retires_wait_without_executing_the_tail() {
    use arcweft_core::awbc::{
        fiber::{FiberState, FiberStatus, FiberSuspensionReason},
        schema::AwbcEntryId,
        vm::{self, VmExit, VmStepOptions},
    };
    let runtime = product(
        "flow main() -> i64 { wait(0.1s); log.info(\"tail\"); return 42i64 }",
        false,
    );
    let program = runtime.product_awbc_program().unwrap();
    let fingerprint = arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
        *blake3::hash(&program.encode_canonical().unwrap()).as_bytes(),
    )
    .unwrap();
    let context = vm::VmExecutionContext::for_program(
        fingerprint,
        runtime.admitted_product_awbc_program().unwrap().clone(),
    );
    let mut fiber = FiberState::for_entry(program, AwbcEntryId(0), 1, 65_536).unwrap();
    let mut host = vm::RejectingVmHost;
    for _ in 0..128 {
        if matches!(
            fiber
                .suspension
                .as_ref()
                .map(|suspension| &suspension.reason),
            Some(FiberSuspensionReason::DurationWait(_))
        ) {
            break;
        }
        let result = vm::step_with_host_context(
            program,
            &mut fiber,
            VmStepOptions::default(),
            &context,
            &mut host,
        )
        .unwrap();
        match result.exit {
            VmExit::Running => {}
            VmExit::Suspended(notice) => assert_eq!(
                notice.kind,
                arcweft_core::awbc::fiber::FiberSuspensionKind::DurationWait
            ),
            exit => panic!("unexpected pre-cancellation exit: {exit:?}"),
        }
    }
    assert!(matches!(
        fiber
            .suspension
            .as_ref()
            .map(|suspension| &suspension.reason),
        Some(FiberSuspensionReason::DurationWait(_))
    ));
    fiber
        .advance_duration_wait(program, LogicalDuration::from_nanos(40_000_000))
        .unwrap();
    let cancelled = vm::cancel_fiber(program, &mut fiber).unwrap();
    assert!(matches!(cancelled.exit, VmExit::Cancelled));
    assert_eq!(fiber.status, FiberStatus::Cancelled);
    assert!(fiber.suspension.is_none());
    assert!(cancelled.observations.is_empty());
    fiber
        .advance_duration_wait(program, LogicalDuration::from_nanos(u64::MAX))
        .unwrap();
    let result = vm::step_with_host_context(
        program,
        &mut fiber,
        VmStepOptions::default(),
        &context,
        &mut host,
    )
    .unwrap();
    assert!(matches!(result.exit, VmExit::Cancelled));
    assert!(result.observations.is_empty());
}

#[test]
fn generic_thread_wait_retains_its_duration_capture_and_return_value() {
    let body = "fn launch<T>(value: T, delay: Duration) -> ThreadHandle<T> effects { control.spawn } {\n thread { wait(delay); return value }\n}\nflow main() -> i64 { let child = launch(42i64, 0.1s); return await child }";
    let waits = |result: &arcweft_core::step::RuntimeStepResult| {
        result
            .output
            .effects
            .line
            .iter()
            .filter(|effect| matches!(effect, arcweft_core::effect::LineEffectRequest::Wait(_)))
            .count()
    };
    let mut engine = native(body);
    assert_eq!(waits(&step(&mut engine, 0)), 1);
    step(&mut engine, 99_999_999);
    assert!(!matches!(
        engine.fiber().frame.status,
        FlowFiberStatus::Done(_)
    ));
    step(&mut engine, 1);
    finish(&mut engine);
    for selected in [false, true] {
        let (mut runtime, result) = product_step(product(body, selected), 0);
        assert_eq!(waits(&result), 1);
        (runtime, _) = product_step(runtime, 99_999_999);
        assert!(!matches!(
            runtime.fiber().frame.status,
            FlowFiberStatus::Done(_)
        ));
        (runtime, _) = product_step(runtime, 1);
        finish_product(runtime);
    }
}

#[test]
fn loop_waits_preserve_the_owning_continuation_across_save_and_time_overshoot() {
    let body = "flow main() -> i64 { for delay in [0.1s, 0.2s] { wait(delay) }; return 42i64 }";
    let mut engine = native(body);
    step(&mut engine, 0);
    assert_eq!(remaining(&engine), 100_000_000);
    step(&mut engine, u64::MAX);
    assert_eq!(remaining(&engine), 200_000_000);
    step(&mut engine, 200_000_000);
    finish(&mut engine);
    for selected in [false, true] {
        let (mut runtime, _) = product_step(product(body, selected), 0);
        assert_eq!(product_remaining(&runtime), 100_000_000);
        (runtime, _) = product_step(runtime, u64::MAX);
        assert_eq!(product_remaining(&runtime), 200_000_000);
        (runtime, _) = product_step(runtime, 200_000_000);
        finish_product(runtime);
    }
}

#[test]
fn decoded_awbc_one_operation_steps_save_before_and_during_the_wait() {
    for selected in [false, true] {
        let mut runtime = product("flow main() -> i64 { wait(0.1s); return 42i64 }", selected);
        let mut wait_count = 0;
        for _ in 0..128 {
            if matches!(
                runtime.fiber().frame.status,
                FlowFiberStatus::DurationWaiting(_)
            ) {
                break;
            }
            let result = runtime.step(Default::default(), Default::default());
            assert!(result.output.diagnostics.is_empty());
            assert!(result.stats.executed_ops <= 1);
            wait_count += result
                .output
                .effects
                .line
                .iter()
                .filter(|effect| matches!(effect, arcweft_core::effect::LineEffectRequest::Wait(_)))
                .count();
            let bound = runtime.bind_product_save_snapshot(saved(&runtime)).unwrap();
            runtime = runtime
                .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
                .unwrap();
        }
        assert_eq!(wait_count, 1);
        assert_eq!(product_remaining(&runtime), 100_000_000);
        (runtime, _) = product_step(runtime, 100_000_000);
        finish_product(runtime);
    }
}
