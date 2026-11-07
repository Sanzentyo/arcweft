use arcweft_compiler::{source::compile_source, types::CompiledSource};
use arcweft_core::{
    awbc::{product_step::AwbcProductExecutorSaveSnapshot, schema::AwbcProgram},
    effect::LineEffectRequest,
    engine::{Engine, FlowExit, FlowFiberStatus},
    executor::{ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot},
    pure::VmRuntimePureCallBackend,
    step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepResult},
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
    let compiled = compile_source(source).expect("ordinary scoped defer source compiles");
    let mut executor = Some(awbc(&compiled));
    let mut backend = VmRuntimePureCallBackend::default();
    move || {
        let mut current = executor.take().unwrap();
        let result = current.step_with_pure_backend(
            RuntimeStepInput::default(),
            RuntimeStepOptions::default(),
            &mut backend,
        );
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
