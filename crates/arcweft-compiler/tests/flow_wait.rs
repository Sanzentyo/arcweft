//! Logical Duration waits execute without host clocks or repeated operand evaluation.

use arcweft_compiler::source::compile_source;
use arcweft_core::{
    engine::{Engine, FlowExit, FlowFiberStatus},
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    time::LogicalDuration,
};

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
