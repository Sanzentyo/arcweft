use arcweft_compiler::{source::compile_source, types::CompiledSource};
use arcweft_core::{
    awbc::{product_step::AwbcProductExecutorSaveSnapshot, schema::AwbcProgram},
    effect::LineEffectRequest,
    engine::{Engine, FlowExit, FlowFiberStatus},
    executor::{ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot},
    pure::VmRuntimePureCallBackend,
    step::{
        RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
        RuntimeStepResult, RuntimeStepStopReason,
    },
    time::LogicalDuration,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

const SOURCE: &str = r#"
flow main() effects { log.write } {
    let message = "first"
    defer { log.info(message) }
    defer on completed { log.info("completed") }
    scope inner {
        defer { log.info("inner") }
        log.info("body")
    }
    log.info(message)
}
entry cli @entry.main { goto @flow.main }
"#;

fn run(mut step: impl FnMut() -> RuntimeStepResult) -> Vec<String> {
    let mut logs = Vec::new();
    for _ in 0..160 {
        let result = step();
        assert!(result.stats.executed_ops <= 1);
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        logs.extend(
            result
                .output
                .effects
                .line
                .into_iter()
                .filter_map(|effect| match effect {
                    LineEffectRequest::Log(row) => Some(row.message),
                    _ => None,
                }),
        );
        match result.fiber_status {
            FlowFiberStatus::Running => {}
            FlowFiberStatus::Done(FlowExit::Done) => return logs,
            status => panic!("unexpected defer status: {status:?}"),
        }
    }
    panic!("scope cleanup did not finish: {logs:?}");
}

fn awbc(compiled: &CompiledSource) -> ArcweftRuntimeExecutor {
    let report = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "scoped_defer.arcw",
    )
    .lower()
    .expect("ordinary scoped defer lowers to verified AWBC");
    let encoded = report.program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, Default::default()).unwrap();
    ArcweftRuntimeExecutor::from_awbc_product(decoded, arcweft_core::awbc::schema::AwbcEntryId(0))
        .unwrap()
}

#[test]
fn source_flow_defer_runs_original_captures_and_lexical_cleanup_in_native() {
    assert_eq!(
        native(SOURCE),
        ["body", "inner", "first", "completed", "first"]
    );
}

fn native(source: &str) -> Vec<String> {
    let compiled = compile_source(source).expect("ordinary scoped defer source compiles");
    let mut native = Engine::for_flow(compiled.plan.clone(), &compiled.plan.flows()[0].id).unwrap();
    let logs = run(|| native.step(RuntimeStepInput::default(), RuntimeStepOptions::default()));
    logs
}

#[test]
fn source_flow_defer_runs_original_captures_and_lexical_cleanup_in_decoded_awbc() {
    assert_eq!(
        saved_awbc(SOURCE),
        ["body", "inner", "first", "completed", "first"]
    );
}

fn saved_awbc(source: &str) -> Vec<String> {
    run(saved_awbc_steps(source))
}

fn saved_awbc_steps(source: &str) -> impl FnMut() -> RuntimeStepResult {
    let mut step = saved_awbc_input_steps(source);
    move || step(Default::default(), Default::default())
}

fn saved_awbc_input_steps(
    source: &str,
) -> impl FnMut(RuntimeStepInput, RuntimeStepOptions) -> RuntimeStepResult {
    let compiled = compile_source(source).expect("ordinary scoped defer source compiles");
    let mut executor = Some(awbc(&compiled));
    let mut backend = VmRuntimePureCallBackend::default();
    move |input, options| {
        let mut current = executor.take().unwrap();
        let result = current.step_with_pure_backend(input, options, &mut backend);
        let ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot) = current.snapshot().unwrap();
        let saved = AwbcProductExecutorSaveSnapshot::from_live(
            &snapshot,
            current.admitted_product_awbc_program().unwrap(),
        )
        .unwrap();
        let bytes = serde_json::to_vec(&saved).unwrap();
        let saved = serde_json::from_slice(&bytes).unwrap();
        let bound = current.bind_product_save_snapshot(saved).unwrap();
        executor = Some(
            current
                .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
                .unwrap(),
        );
        result
    }
}

fn assert_deferred_wait(source: &str, before: &[&str], after: &[&str]) {
    let compiled = compile_source(source).unwrap();
    let mut native = Engine::for_flow(compiled.plan.clone(), &compiled.plan.flows()[0].id).unwrap();
    let mut native_step = |input, options| native.step(input, options);
    let mut awbc_step = saved_awbc_input_steps(source);
    for (backend, step) in [
        (
            "Native",
            &mut native_step
                as &mut dyn FnMut(RuntimeStepInput, RuntimeStepOptions) -> RuntimeStepResult,
        ),
        ("saved AWBC", &mut awbc_step),
    ] {
        let options = RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 128 },
            ..Default::default()
        };
        let logs = |result: &RuntimeStepResult| {
            result
                .output
                .effects
                .line
                .iter()
                .filter_map(|effect| match effect {
                    LineEffectRequest::Log(row) => Some(row.message.as_str()),
                    _ => None,
                })
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        let entered = step(Default::default(), options);
        assert!(
            entered.output.diagnostics.is_empty(),
            "{backend}: {:?}",
            entered.output.diagnostics
        );
        assert_eq!(logs(&entered), before, "{backend}");
        let paused = step(Default::default(), options);
        assert!(
            paused.output.diagnostics.is_empty(),
            "{backend}: {:?}",
            paused.output.diagnostics
        );
        assert_eq!(paused.stats.executed_ops, 0, "{backend}");
        assert!(logs(&paused).is_empty(), "{backend}");
        assert_eq!(
            paused.stop_reason,
            RuntimeStepStopReason::Blocked,
            "{backend}"
        );
        let partial = step(
            RuntimeStepInput {
                dt: LogicalDuration::from_nanos(40_000_000),
                ..Default::default()
            },
            options,
        );
        assert!(partial.output.diagnostics.is_empty(), "{backend}");
        assert!(logs(&partial).is_empty(), "{backend}");
        assert_eq!(partial.stats.executed_ops, 0, "{backend}");
        assert_eq!(
            partial.stop_reason,
            RuntimeStepStopReason::Blocked,
            "{backend}"
        );
        let resumed = step(
            RuntimeStepInput {
                dt: LogicalDuration::from_nanos(60_000_000),
                ..Default::default()
            },
            options,
        );
        assert!(
            resumed.output.diagnostics.is_empty(),
            "{backend}: {:?}",
            resumed.output.diagnostics
        );
        assert_eq!(logs(&resumed), after, "{backend}");
        assert_eq!(
            resumed.fiber_status,
            FlowFiberStatus::Done(FlowExit::Done),
            "{backend}"
        );
        let finished = step(Default::default(), options);
        assert!(
            logs(&finished).is_empty(),
            "{backend}: completed cleanup must not replay"
        );
    }
}

#[test]
fn registered_defer_does_not_make_a_waiting_flow_runnable() {
    assert_deferred_wait(
        r#"
flow main() {
    defer { log.info("cleanup") }
    wait(0.1s)
    log.info("after-wait")
}
entry cli @entry.main { goto @flow.main }
"#,
        &[],
        &["after-wait", "cleanup"],
    );
}

#[test]
fn deferred_callback_wait_blocks_until_its_original_deadline() {
    assert_deferred_wait(
        r#"
flow main() {
    scope region {
        defer { log.info("cleanup-enter"); wait(0.1s); log.info("cleanup-done") }
        log.info("body")
    }
    log.info("after-scope")
}
entry cli @entry.main { goto @flow.main }
"#,
        &["body", "cleanup-enter"],
        &["cleanup-done", "after-scope"],
    );
}

#[test]
fn invocation_cleanup_waits_before_publishing_completion() {
    assert_deferred_wait(
        r#"
flow main() {
    defer { log.info("cleanup-enter"); wait(0.1s); log.info("cleanup-done") }
    log.info("body")
}
entry cli @entry.main { goto @flow.main }
"#,
        &["body", "cleanup-enter"],
        &["cleanup-done"],
    );
}

#[test]
fn direct_function_cleanup_waits_before_delivering_return_value() {
    assert_deferred_wait(
        r#"
fn relay() -> i64 effects { log.write, control.suspend } {
    defer { log.info("cleanup-enter"); wait(0.1s); log.info("cleanup-done") }
    log.info("function-body")
    return 42i64
}
flow main() { let value = relay(); log.info(value) }
entry cli @entry.main { goto @flow.main }
"#,
        &["function-body", "cleanup-enter"],
        &["cleanup-done", "42"],
    );
}

#[test]
fn nested_deferred_callback_wait_blocks_all_waiting_ancestors() {
    assert_deferred_wait(
        r#"
flow main() {
    scope region {
        defer {
            defer { log.info("inner-enter"); wait(0.1s); log.info("inner-done") }
            log.info("outer")
        }
        log.info("body")
    }
    log.info("after-scope")
}
entry cli @entry.main { goto @flow.main }
"#,
        &["body", "outer", "inner-enter"],
        &["inner-done", "after-scope"],
    );
}

#[test]
fn completed_scope_skips_failed_and_cancelled_callbacks_in_both_backends() {
    let source = r#"
flow main() effects { log.write } {
    defer on failed { log.info("wrong-failed") }
    defer on cancelled { log.info("wrong-cancelled") }
    defer on completed { log.info("completed") }
    defer { log.info("always") }
    log.info("body")
}
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["body", "always", "completed"]);
    assert_eq!(saved_awbc(source), ["body", "always", "completed"]);
}

#[test]
fn nested_function_defer_finishes_before_binding_original_return_value() {
    let source = r#"
fn child() -> String effects { log.write } {
    defer { log.info("child-root") }
    scope inner {
        defer { log.info("child-inner") }
        return "kept"
    }
}
flow main() effects { log.write } {
    defer { log.info("main-root") }
    log.info(child())
    log.info("after-call")
}
entry cli @entry.main { goto @flow.main }
"#;
    let expected = [
        "child-inner",
        "child-root",
        "kept",
        "after-call",
        "main-root",
    ];
    assert_eq!(native(source), expected);
    assert_eq!(saved_awbc(source), expected);
}

#[test]
fn saved_defer_rejects_stale_frontier_and_unpaired_callback_owner() {
    let compiled = compile_source(SOURCE).unwrap();
    let mut executor = awbc(&compiled);
    let mut backend = VmRuntimePureCallBackend::default();
    for _ in 0..160 {
        executor.step_with_pure_backend(Default::default(), Default::default(), &mut backend);
        let ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot) = executor.snapshot().unwrap();
        let saved = AwbcProductExecutorSaveSnapshot::from_live(
            &snapshot,
            executor.admitted_product_awbc_program().unwrap(),
        )
        .unwrap();
        let original = serde_json::to_value(saved).unwrap();
        let Some(index) = original["child_fibers"]
            .as_array()
            .unwrap()
            .iter()
            .position(|child| !child["owner"]["FlowDeferred"].is_null())
        else {
            continue;
        };
        let mut forged = original.clone();
        forged["flow_defer_frontier"] = serde_json::json!(1);
        executor = reject_save(executor, forged);
        let mut forged = original.clone();
        forged["child_fibers"][index]["owner"]["FlowDeferred"]["owner"]["frame"] =
            serde_json::json!(u64::MAX.to_string());
        executor = reject_save(executor, forged);
        let mut forged = original.clone();
        forged["child_fibers"].as_array_mut().unwrap().remove(index);
        executor = reject_save(executor, forged);
        let mut forged = original.clone();
        forged["child_fibers"][index]["owner"] = serde_json::json!("Independent");
        executor = reject_save(executor, forged);
        let ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot) = executor.snapshot().unwrap();
        let saved = AwbcProductExecutorSaveSnapshot::from_live(
            &snapshot,
            executor.admitted_product_awbc_program().unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(saved).unwrap(), original);
        return;
    }
    panic!("the source did not reach its deferred callback");
}

fn reject_save(
    executor: ArcweftRuntimeExecutor,
    forged: serde_json::Value,
) -> ArcweftRuntimeExecutor {
    let saved = serde_json::from_value(forged).unwrap();
    let Ok(bound) = executor.bind_product_save_snapshot(saved) else {
        return executor;
    };
    executor
        .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
        .expect_err("the owning restore must reject the forged retained contour")
        .into_parts()
        .0
}

#[test]
fn generic_defer_sites_keep_distinct_capture_abis() {
    let source = r#"
fn consume<T>(value: T) -> Unit {}
fn capture<T>(value: T, label: String) -> Unit effects { log.write } {
    defer { consume(value); log.info(label) }
}
flow main() effects { log.write } {
    capture("string", "first")
    capture(42i64, "second")
}
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["first", "second"]);
    assert_eq!(saved_awbc(source), ["first", "second"]);
}

#[test]
fn deferred_callback_drains_its_own_defer_before_resuming_parent() {
    let source = r#"
flow main() effects { log.write } {
    defer {
        defer { log.info("nested-cleanup") }
        log.info("cleanup")
    }
    log.info("body")
}
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["body", "cleanup", "nested-cleanup"]);
    assert_eq!(saved_awbc(source), ["body", "cleanup", "nested-cleanup"]);
}

#[test]
fn unsafe_audit_body_runs_its_lexical_cleanup_before_the_outer_continuation() {
    let source = r#"
flow main() effects { log.write } {
    defer { log.info("flow-cleanup") }
    unsafe lifetime @unsafe.body reason = "owned local stays in this body" {
        /// SAFETY: only the owned local is captured by this body's cleanup.
        let message = "audit-cleanup"
        defer { log.info(message) }
        log.info("audit-body")
    }
    log.info("after-audit")
}
entry cli @entry.main { goto @flow.main }
"#;
    let expected = ["audit-body", "audit-cleanup", "after-audit", "flow-cleanup"];
    assert_eq!(native(source), expected);
    assert_eq!(saved_awbc(source), expected);
}

#[test]
fn unsafe_audit_body_preserves_generic_return_and_cleanup_ownership() {
    let source = r#"
fn audited<T>(value: T) -> T effects { log.write } {
    defer { log.info("function-cleanup") }
    unsafe lifetime @unsafe.return reason = "return keeps the input's owned type" {
        /// SAFETY: no borrowed value escapes this invocation.
        defer { log.info("audit-cleanup") }
        return value
    }
}
flow main() effects { log.write } {
    log.info(audited("kept"))
    log.info(audited(42i64))
}
entry cli @entry.main { goto @flow.main }
"#;
    let expected = [
        "audit-cleanup",
        "function-cleanup",
        "kept",
        "audit-cleanup",
        "function-cleanup",
        "42",
    ];
    assert_eq!(native(source), expected);
    assert_eq!(saved_awbc(source), expected);
}

#[test]
fn unsafe_audit_body_without_effects_keeps_its_enclosing_value() {
    let source = r#"
fn audited(value: i64) -> i64 {
    unsafe lifetime @unsafe.local reason = "the block retains only an owned scalar" {
        /// SAFETY: the local does not escape its lexical scope.
        let value = 99i64
    }
    value
}
flow main() { log.info(audited(42i64)) }
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["42"]);
    assert_eq!(saved_awbc(source), ["42"]);
}

#[test]
fn unsafe_audit_body_drains_cleanup_before_breaking_its_enclosing_loop() {
    let source = r#"
flow main() {
    let value = loop {
        unsafe lifetime @unsafe.loop reason = "the loop result is owned" {
            /// SAFETY: cleanup finishes before the owned result leaves.
            defer { log.info("audit-cleanup") }
            break "kept"
        }
    }
    log.info(value)
}
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["audit-cleanup", "kept"]);
    assert_eq!(saved_awbc(source), ["audit-cleanup", "kept"]);
}

#[test]
fn unsafe_audit_body_waits_for_cleanup_before_resuming_its_caller() {
    assert_deferred_wait(
        r#"
flow main() {
    unsafe lifetime @unsafe.wait reason = "cleanup retains its owned input while suspended" {
        /// SAFETY: the callback and its input remain in this scope.
        defer { log.info("cleanup-enter"); wait(0.1s); log.info("cleanup-done") }
        log.info("audit-body")
    }
    log.info("after-audit")
}
entry cli @entry.main { goto @flow.main }
"#,
        &["audit-body", "cleanup-enter"],
        &["cleanup-done", "after-audit"],
    );
}

#[test]
fn scoped_defer_writes_the_reached_flow_registry_before_resuming_parent() {
    let source = r#"
flow main() {
    'flow.flag <- false
    scope inner {
        defer { 'flow.flag <- true; }
    }
    let flag: Option<bool> = 'flow.flag?
    if let .Some(value) = flag { log.info(value) } else { log.info("missing") }
}
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["true"]);
    assert_eq!(saved_awbc(source), ["true"]);
}

#[test]
fn function_defer_writes_its_callers_flow_registry() {
    let source = r#"
fn schedule() effects { state.write('flow) } {
    defer { 'flow.flag <- true; }
}
flow main() {
    'flow.flag <- false
    schedule()
    let flag: Option<bool> = 'flow.flag?
    if let .Some(value) = flag { log.info(value) } else { log.info("missing") }
}
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["true"]);
    assert_eq!(saved_awbc(source), ["true"]);
}

#[test]
fn scoped_defer_reads_and_drops_only_its_reached_flow_registry_key() {
    let source = r#"
flow main() {
    'flow.flag <- false
    'flow.sibling <- "kept"
    scope inner {
        defer {
            let before: Option<bool> = 'flow.flag?
            if let .Some(value) = before { log.info(value) } else { log.info("missing") }
            drop_optional('flow.flag?)
        }
    }
    let flag: Option<bool> = 'flow.flag?
    if let .None = flag { log.info('flow.sibling) } else { log.info("not-dropped") }
}
entry cli @entry.main { goto @flow.main }
"#;
    let compiled = compile_source(source).unwrap();
    let [site] = compiled.plan.defer_sites() else {
        panic!("one original cleanup callback");
    };
    let callback = compiled.plan.function_sites().get(*site).unwrap();
    let write = arcweft_id::EffectId::parse("state.write('flow.flag)").unwrap();
    assert!(
        callback
            .invocation_effects()
            .iter()
            .any(|effect| effect.covers(&write)),
        "the admitted cleanup must retain its registry drop permission"
    );
    assert_eq!(native(source), ["false", "kept"]);
    assert_eq!(saved_awbc(source), ["false", "kept"]);
}

#[test]
fn nested_defer_reaches_flow_registry_through_its_real_cleanup_parent() {
    let source = r#"
flow main() {
    'flow.flag <- false
    scope inner {
        defer {
            defer { 'flow.flag <- true; }
            log.info("outer")
        }
    }
    let flag: Option<bool> = 'flow.flag?
    if let .Some(value) = flag { log.info(value) } else { log.info("missing") }
}
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["outer", "true"]);
    assert_eq!(saved_awbc(source), ["outer", "true"]);
}

#[test]
fn generic_defer_keeps_registry_values_bound_to_each_original_capture_type() {
    let source = r#"
fn schedule<T>(value: T) effects { state.write('flow) } {
    defer { 'flow.value <- value; }
}
flow main() {
    schedule("kept")
    let first: Option<String> = 'flow.value?
    if let .Some(value) = first { log.info(value) } else { log.info("missing-string") }
    schedule(42i64)
    let second: Option<i64> = 'flow.value?
    if let .Some(value) = second { log.info(value) } else { log.info("missing-integer") }
}
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["kept", "42"]);
    assert_eq!(saved_awbc(source), ["kept", "42"]);
}

#[test]
fn trailing_registry_write_is_an_ordinary_statement_in_cleanup_value_blocks() {
    for body in [
        "{ 'flow.flag <- true }",
        "{\n    'flow.flag <- true\n}",
        "{\r\n    'flow.flag <- true\r\n}",
    ] {
        let source = format!(
            "flow main() {{\n 'flow.flag <- false\n scope inner {{ defer {body} }}\n let flag: Option<bool> = 'flow.flag?\n if let .Some(value) = flag {{ log.info(value) }} else {{ log.info(\"missing\") }}\n}}\nentry cli @entry.main {{ goto @flow.main }}\n"
        );
        assert_eq!(native(&source), ["true"], "{body:?}");
        assert_eq!(saved_awbc(&source), ["true"], "{body:?}");
    }
}

#[test]
fn static_goto_drains_invocation_and_nested_scope_cleanup_before_target() {
    let source = r#"
flow main() {
    defer { log.info("outer") }
    scope inner {
        defer { log.info("inner") }
        goto @flow.done
    }
}
flow done() { log.info("target") }
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["inner", "outer", "target"]);
    assert_eq!(saved_awbc(source), ["inner", "outer", "target"]);
}

#[test]
fn dynamic_goto_drains_invocation_and_nested_scope_cleanup_before_target() {
    let source = r#"
flow main() {
    defer { log.info("outer") }
    scope inner {
        defer { log.info("inner") }
        let target = @flow.done
        goto target
    }
}
flow done() { log.info("target") }
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["inner", "outer", "target"]);
    assert_eq!(saved_awbc(source), ["inner", "outer", "target"]);
}

#[test]
fn rejected_dynamic_goto_runs_failure_cleanup_without_completing_its_scope() {
    let source = r#"
flow main() effects { log.write } {
    defer on failed { log.info("outer-failed") }
    scope inner {
        defer on completed { log.info("wrong-completed") }
        defer on failed { log.info("inner-failed") }
        let target = @flow.needs_input
        goto target
    }
}
flow needs_input(value: i64) { log.info(value) }
entry cli @entry.main { goto @flow.main }
"#;
    let compiled = compile_source(source).unwrap();
    let mut native = Engine::for_flow(compiled.plan.clone(), &compiled.plan.flows()[0].id).unwrap();
    let mut native_step = || native.step(Default::default(), Default::default());
    let mut awbc_step = saved_awbc_steps(source);
    for (backend, step, diagnostic) in [
        (
            "Native",
            &mut native_step as &mut dyn FnMut() -> RuntimeStepResult,
            "expects 1 invocation bindings, received 0",
        ),
        (
            "saved AWBC",
            &mut awbc_step,
            "expects 1 arguments, received 0",
        ),
    ] {
        let mut logs = Vec::new();
        let failure =
            (0..160).find_map(|_| {
                let result = step();
                assert!(result.stats.executed_ops <= 1, "{backend}");
                logs.extend(result.output.effects.line.into_iter().filter_map(
                    |effect| match effect {
                        LineEffectRequest::Log(row) => Some(row.message),
                        _ => None,
                    },
                ));
                match result.fiber_status {
                    FlowFiberStatus::Running => None,
                    FlowFiberStatus::Failed(message) => Some(message),
                    status => panic!("{backend}: unexpected status {status:?}"),
                }
            });
        let failure = failure.unwrap_or_else(|| panic!("{backend}: expected ABI rejection"));
        assert!(failure.contains(diagnostic), "{backend}: {failure}");
        assert_eq!(logs, ["inner-failed", "outer-failed"], "{backend}");
    }
}

#[test]
fn function_goto_drains_its_own_and_callers_cleanup_without_resuming_caller_code() {
    let source = r#"
fn transfer() effects { log.write } {
    defer { log.info("function") }
    goto @flow.done
}
flow main() {
    defer { log.info("outer") }
    transfer()
    log.info("wrong-after-transfer")
}
flow done() { log.info("target") }
entry cli @entry.main { goto @flow.main }
"#;
    assert_eq!(native(source), ["function", "outer", "target"]);
    assert_eq!(saved_awbc(source), ["function", "outer", "target"]);
}

#[test]
fn goto_waits_for_invocation_cleanup_before_starting_its_original_target() {
    let source = r#"
flow main() {
    defer { wait(100ms); log.info("outer") }
    goto @flow.done
}
flow done() { log.info("target") }
entry cli @entry.main { goto @flow.main }
"#;
    assert_deferred_wait(source, &[], &["outer", "target"]);
}

#[test]
fn callback_failure_drains_remaining_cleanup_without_resuming_normal_code() {
    let source = r#"
flow main() effects { log.write } {
    defer on failed { log.info("outer-failed") }
    scope inner {
        defer on completed { log.info("inner-completed") }
        defer { let zero = 0i64; log.info(1i64 / zero) }
        log.info("body")
    }
    log.info("wrong-after-scope")
}

entry cli @entry.main { goto @flow.main }
"#;
    let compiled = compile_source(source).unwrap();
    let mut native = Engine::for_flow(compiled.plan.clone(), &compiled.plan.flows()[0].id).unwrap();
    let mut native_step = || native.step(Default::default(), Default::default());
    let mut awbc_step = saved_awbc_steps(source);
    for step in [
        &mut native_step as &mut dyn FnMut() -> RuntimeStepResult,
        &mut awbc_step,
    ] {
        let mut logs = Vec::new();
        let mut failed = false;
        for _ in 0..160 {
            let result = step();
            assert!(result.stats.executed_ops <= 1);
            logs.extend(
                result
                    .output
                    .effects
                    .line
                    .into_iter()
                    .filter_map(|effect| match effect {
                        LineEffectRequest::Log(row) => Some(row.message),
                        _ => None,
                    }),
            );
            if matches!(result.fiber_status, FlowFiberStatus::Failed(_)) {
                failed = true;
                break;
            }
        }
        assert!(
            failed,
            "cleanup failure must prevent normal completion: {logs:?}"
        );
        assert_eq!(logs, ["body", "inner-completed", "outer-failed"]);
    }
}

#[test]
fn selected_choice_drains_nested_and_invocation_cleanup_before_target() {
    use arcweft_interaction_model::{
        id::Identifier,
        input::{InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent},
        payload::InteractionPayload,
    };
    let source = r#"
flow main() {
    defer { log.info("outer") }
    scope inner {
        defer { log.info("inner") }
        choice @.first { @.next "Next" -> @flow.done }
    }
    log.info("wrong-after-choice")
}
flow done() { log.info("target") }
entry cli @entry.main { goto @flow.main }
"#;
    let compiled = compile_source(source).unwrap();
    let mut native = Engine::for_flow(compiled.plan.clone(), &compiled.plan.flows()[0].id).unwrap();
    let mut native_step = |input, options| native.step(input, options);
    let mut awbc_step = saved_awbc_input_steps(source);
    for step in [
        &mut native_step
            as &mut dyn FnMut(RuntimeStepInput, RuntimeStepOptions) -> RuntimeStepResult,
        &mut awbc_step,
    ] {
        let mut input = RuntimeStepInput::default();
        let mut logs = Vec::new();
        let mut selected = false;
        let mut done = false;
        for _ in 0..160 {
            let result = step(std::mem::take(&mut input), Default::default());
            assert!(result.stats.executed_ops <= 1);
            assert!(
                result.output.diagnostics.is_empty(),
                "{:?}",
                result.output.diagnostics
            );
            logs.extend(
                result
                    .output
                    .effects
                    .line
                    .iter()
                    .filter_map(|effect| match effect {
                        LineEffectRequest::Log(row) => Some(row.message.clone()),
                        _ => None,
                    }),
            );
            match result.fiber_status {
                FlowFiberStatus::Choice(choice) => {
                    assert!(!selected, "one selected option must suffice");
                    selected = true;
                    input.input_events.push(
                        RoutedInputEvent::new(
                            InputEpoch::new(0),
                            InputSequence::new(0),
                            InteractionTarget::new("choice").unwrap(),
                            InputEventKind::Custom {
                                name: Identifier::new(format!("choice:{}", choice.id().unwrap()))
                                    .unwrap(),
                            },
                        )
                        .with_payload(InteractionPayload::Text(
                            choice.options()[0].id.clone().unwrap(),
                        )),
                    );
                }
                FlowFiberStatus::Done(FlowExit::Done) => {
                    done = true;
                    break;
                }
                FlowFiberStatus::Running => {}
                status => panic!("unexpected Choice cleanup status: {status:?}"),
            }
        }
        assert!(selected && done);
        assert_eq!(logs, ["inner", "outer", "target"]);
    }
}

#[test]
fn failed_goto_cleanup_drains_callers_without_entering_original_target() {
    let source = r#"
fn transfer() effects { log.write } {
    defer on completed { log.info("inner-completed") }
    defer { let zero = 0i64; log.info(1i64 / zero) }
    goto @flow.done
}
flow main() {
    defer on failed { log.info("outer-failed") }
    transfer()
    log.info("wrong-after-call")
}
flow done() { log.info("wrong-target") }
entry cli @entry.main { goto @flow.main }
"#;
    let compiled = compile_source(source).unwrap();
    let mut native = Engine::for_flow(compiled.plan.clone(), &compiled.plan.flows()[0].id).unwrap();
    let mut native_step = || native.step(Default::default(), Default::default());
    let mut awbc_step = saved_awbc_steps(source);
    for step in [
        &mut native_step as &mut dyn FnMut() -> RuntimeStepResult,
        &mut awbc_step,
    ] {
        let mut logs = Vec::new();
        let mut failed = false;
        for _ in 0..160 {
            let result = step();
            assert!(result.stats.executed_ops <= 1);
            logs.extend(
                result
                    .output
                    .effects
                    .line
                    .iter()
                    .filter_map(|effect| match effect {
                        LineEffectRequest::Log(row) => Some(row.message.clone()),
                        _ => None,
                    }),
            );
            if matches!(result.fiber_status, FlowFiberStatus::Failed(_)) {
                failed = true;
                break;
            }
        }
        assert!(failed, "failed cleanup must prevent the target: {logs:?}");
        assert_eq!(logs, ["inner-completed", "outer-failed"]);
    }
}
