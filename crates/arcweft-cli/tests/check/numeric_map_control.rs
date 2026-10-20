#[test]
fn closed_source_map_kernel_preserves_every_formal_and_one_op_continuation() {
    for (label, ty) in [("array", "Array<i64, 128>"), ("vec", "Vec<i64>")] {
        let values = if label == "array" {
            "[2i64; 128]".to_owned()
        } else {
            format!(
                "[{}]",
                std::iter::repeat_n("2i64", 128)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let source = format!(
            r#"
entry cli @entry.run {{ goto @flow.map_control }}
#[pure]
fn score(base: i64, bonus: i64, unused: i64) -> i64 {{ return base * (bonus + 2i64) }}
flow map_control() -> String {{
    let values: {ty} = {values}
    let scores: {ty} = values.map(|item| score(item, 2i64, 91i64))
    let total: i64 = scores.sum()
    if total != 1024i64 {{ return "wrong" }}
    return "done"
}}
"#
        );
        let path = temp_arcw(&format!("map-control-{label}"), &source);
        for (mode, maximum_ops) in [("drain", 64), ("drain", 2), ("one-op", 1)] {
            assert_closed_source_map_step_budget(&path, label, mode, maximum_ops);
        }
    }
}

fn assert_closed_source_map_step_budget(
    path: &std::path::Path,
    label: &str,
    mode: &str,
    maximum_ops: u64,
) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(path)
        .args([
            "--entry",
            "entry.run",
            "--mode",
            mode,
            "--steps",
            "4096",
            "--max-ops",
        ])
        .arg(maximum_ops.to_string())
        .args([
            "--pure-backend",
            "jit",
            "--pure-batch-min-len",
            "64",
            "--json",
        ])
        .output()
        .expect("run the original closed callable Map");
    assert!(
        output.status.success(),
        "{label}/{mode}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["final_status"], "done Return(\"done\")",
        "{label}/{mode}: {json}"
    );
    assert_eq!(
        sum_step_pure_counter(&json, "pure_calls"),
        128,
        "{label}/{mode}: {json}"
    );
    for step in json["steps"].as_array().unwrap() {
        assert!(step["stats"]["executed_ops"].as_u64().unwrap() <= maximum_ops);
    }
    if mode == "drain" && maximum_ops == 64 {
        for (counter, expected) in [
            ("batch_calls", 1),
            ("batch_items", 128),
            ("jit_calls", 128),
            ("arg_bytes_borrowed", 24),
        ] {
            assert_eq!(
                sum_step_pure_counter(&json, counter),
                expected,
                "{label}/{counter}: {json}"
            );
        }
        for counter in [
            "vm_calls",
            "aot_calls",
            "fallbacks",
            "arg_vec_allocations",
            "arg_bytes_copied",
            "result_bytes_copied",
            "thread_pool_jobs",
        ] {
            assert_eq!(
                sum_step_pure_counter(&json, counter),
                0,
                "{label}/{counter}: {json}"
            );
        }
    } else if mode == "one-op" {
        for counter in ["batch_calls", "batch_items", "jit_calls", "aot_calls"] {
            assert_eq!(
                sum_step_pure_counter(&json, counter),
                0,
                "{label}/{counter}: {json}"
            );
        }
        assert_eq!(sum_step_pure_counter(&json, "vm_calls"), 128);
    } else {
        // The Map plus its source alias and Sum cannot fit two ops.
        // Individual callbacks still execute through the ordinary body.
        for counter in ["batch_calls", "batch_items"] {
            assert_eq!(
                sum_step_pure_counter(&json, counter),
                0,
                "{label}/{mode}/{maximum_ops}/{counter}: {json}"
            );
        }
    }
}

#[test]
fn closed_source_map_log_does_not_hide_a_later_array_observer() {
    let path = temp_arcw(
        "map-control-log-observer",
        r#"
entry cli @entry.run { goto @flow.map_control }
#[pure]
fn score(base: i64, bonus: i64) -> i64 { return base * (bonus + 2i64) }
flow map_control() -> String {
    let values: Array<i64, 128> = [2i64; 128]
    let scores: Array<i64, 128> = values.map(|item| score(item, 2i64))
    let total: i64 = scores.sum()
    log.info(total)
    let again: i64 = scores.sum()
    if total != 1024i64 { return "wrong first sum" }
    if again != 1024i64 { return "wrong second sum" }
    return "done"
}

"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .args([
            "--entry",
            "entry.run",
            "--mode",
            "drain",
            "--steps",
            "64",
            "--max-ops",
            "64",
            "--pure-backend",
            "jit",
            "--pure-batch-min-len",
            "64",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["final_status"], "done Return(\"done\")", "{json}");
    for (counter, expected) in [
        ("pure_calls", 128),
        ("batch_calls", 1),
        ("batch_items", 128),
        ("jit_calls", 128),
        ("arg_bytes_borrowed", 2048),
        ("result_bytes_copied", 1024),
    ] {
        assert_eq!(
            sum_step_pure_counter(&json, counter),
            expected,
            "{counter}: {json}"
        );
    }
    for step in json["steps"].as_array().unwrap() {
        assert!(step["stats"]["executed_ops"].as_u64().unwrap() <= 64);
    }
}

#[test]
fn source_map_dense_u32_program_keeps_typed_callbacks_under_the_default_budget() {
    use arcweft_core::engine::{Engine, FlowFiberStatus};
    use arcweft_core::pure::RuntimePureCallBackend;
    use arcweft_lang_sema::final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource};
    use arcweft_runtime_accelerator::{
        RuntimePureAccelerator, RuntimePureAcceleratorConfig, RuntimePureBackendMode,
        RuntimePureWorkerCount,
    };
    use std::sync::Arc;
    let compiled = arcweft_compiler::source::compile_source(
        r#"
#[pure]
fn increment(value: u32) -> u32 { return value + 1u32 }
pub fn root(values: Seq<u32>) -> i64 { values.map(increment).sum() }
flow main() -> String { return "unused" }
"#,
    )
    .unwrap();
    let declaration = compiled
        .analysis
        .final_analysis()
        .hir_topology()
        .modules()
        .iter()
        .flat_map(|module| module.entries())
        .filter_map(|entry| entry.body())
        .find(|body| body.declaration().name() == "root")
        .unwrap()
        .declaration()
        .clone();
    let selected = compiled
        .analysis
        .compile_deterministic_program(
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                declaration,
                role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
            }),
            None,
            &Default::default(),
        )
        .unwrap();
    let mut engine = Engine::for_program_invocation(
        Arc::clone(selected.plan()),
        selected.program(),
        vec![arcweft_core::value::runtime_sequence_dense_u32(
            (0..128).collect(),
        )],
    )
    .unwrap();
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        selected.plan(),
    );
    let options = arcweft_core::step::RuntimeStepOptions::default();
    let mut steps = 0;
    for _ in 0..4096 {
        let step = engine.step_with_pure_backend(Default::default(), options, &mut accelerator);
        steps += 1;
        assert!(step.stats.executed_ops <= options.budget.max_ops);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        assert!(!matches!(step.fiber_status, FlowFiberStatus::Failed(_)));
        if matches!(step.fiber_status, FlowFiberStatus::Done(_)) {
            break;
        }
    }
    assert!(
        steps > 1,
        "default budget must preserve the ordinary Map continuation"
    );
    assert_eq!(
        engine.take_program_result().unwrap().unwrap().1,
        arcweft_core::value::RuntimeValue::i64(8256)
    );
    assert_eq!(accelerator.stats().pure_calls, 128);
}

#[test]
fn wide_source_map_sum_uses_complete_input_bounds_before_materialization() {
    for (ty, bytes, maximum) in [
        ("u64", 8, u128::from(u64::MAX)),
        ("i128", 16, i128::MAX as u128),
        ("u128", 16, u128::MAX),
        ("usize", 8, u128::from(u64::MAX)),
    ] {
        for (label, middle, body, expected) in [
            (
                "fits",
                i64::MAX as u128,
                format!(
                    "let marker: Unit = (); let _ = marker; let alias = value; if alias > 0{ty} {{ return alias }} else {{ return 0{ty} }}"
                ),
                Some(i64::MAX.wrapping_add(4)),
            ),
            (
                "outside",
                i64::MAX as u128 + 1,
                format!(
                    "let alias = value; if alias > 0{ty} {{ return alias }} else {{ return 0{ty} }}"
                ),
                None,
            ),
            (
                "annihilated",
                maximum,
                format!("return value * 0{ty}"),
                Some(0),
            ),
        ] {
            let source = format!(
                r#"
entry cli @entry.run {{ goto @flow.main }}
#[pure]
fn keep(value: {ty}, unused: {ty}) -> {ty} {{ {body} }}
flow main() -> String {{
    let values: Vec<{ty}> = [1{ty}, {middle}{ty}, 3{ty}]
    let mapped = values.map(|item| keep(item, 7{ty}))
    let total = mapped.sum()
    log.info(total)
    return "done"
}}
"#
            );
            let path = temp_arcw(&format!("wide-map-sum-{ty}-{label}"), &source);
            assert_wide_map_sum_completion(&path, bytes, expected);
        }
    }
}

fn assert_wide_map_sum_completion(path: &std::path::Path, bytes: u64, expected: Option<i64>) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(path)
        .args([
            "--entry",
            "entry.run",
            "--mode",
            "drain",
            "--steps",
            "64",
            "--max-ops",
            "64",
            "--pure-backend",
            "jit",
            "--pure-batch-min-len",
            "1",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(i32::from(expected.is_none())),
        "unexpected completion status, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    for (counter, expected) in [
        ("pure_calls", 3),
        ("batch_calls", 1),
        ("batch_items", 3),
        ("jit_calls", 3),
        ("arg_bytes_borrowed", 6 * bytes),
    ] {
        assert_eq!(
            sum_step_pure_counter(&json, counter),
            expected,
            "{counter}: {json}"
        );
    }
    for counter in [
        "vm_calls",
        "aot_calls",
        "fallbacks",
        "arg_vec_allocations",
        "arg_bytes_copied",
    ] {
        assert_eq!(
            sum_step_pure_counter(&json, counter),
            0,
            "{counter}: {json}"
        );
    }
    for step in json["steps"].as_array().unwrap() {
        assert!(step["stats"]["executed_ops"].as_u64().unwrap() <= 64);
    }
    if let Some(value) = expected {
        assert_eq!(json["final_status"], "done Return(\"done\")", "{json}");
        assert_eq!(
            sum_step_pure_counter(&json, "result_bytes_copied"),
            0,
            "{json}"
        );
        let logs = json["steps"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|step| step["observations"]["logs"].as_array().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(logs.len(), 1, "{json}");
        assert_eq!(logs[0]["message"], value.to_string());
    } else {
        assert!(
            json["final_status"]
                .as_str()
                .unwrap()
                .starts_with("failed "),
            "{json}"
        );
        assert!(
            json["final_status"]
                .as_str()
                .unwrap()
                .contains("9223372036854775808"),
            "{json}"
        );
        assert_eq!(
            sum_step_pure_counter(&json, "result_bytes_copied"),
            3 * bytes,
            "{json}"
        );
    }
}

fn assert_dense_integer_fixture_typed_execution(path: &std::path::Path) {
    use arcweft_core::awbc::{
        codec::AwbcDecodeBudget,
        schema::{AwbcEntryId, AwbcProgram},
    };
    use arcweft_core::engine::Engine;
    use arcweft_core::executor::{ArcweftRuntimeExecutor, RuntimeExecutor};
    use arcweft_core::plan::{FlowOp, RuntimePlanSequenceKind, RuntimePlanTypeProjection};
    use arcweft_core::step::{
        RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
    };
    use arcweft_core::value::{RuntimeSignedIntWidth as S, RuntimeUnsignedIntWidth as U};
    use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

    let compiled =
        arcweft_compiler::source::compile_source(&std::fs::read_to_string(path).unwrap()).unwrap();
    let [flow] = compiled.plan.flows() else {
        panic!("one measured flow")
    };
    let types = flow
        .body()
        .ops()
        .iter()
        .filter_map(|operation| {
            let FlowOp::Let { pattern, expr } = operation else {
                return None;
            };
            let row = compiled.plan.type_table().get(pattern.ty()).unwrap();
            let RuntimePlanTypeProjection::Sequence {
                kind: RuntimePlanSequenceKind::Vec,
                item,
            } = row.projection()
            else {
                return None;
            };
            assert_eq!(expr.ty(), pattern.ty());
            Some(
                compiled
                    .plan
                    .type_table()
                    .get(*item)
                    .unwrap()
                    .projection()
                    .clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(types.len(), 7);
    for expected in [
        RuntimePlanTypeProjection::Signed(S::I8),
        RuntimePlanTypeProjection::Signed(S::I16),
        RuntimePlanTypeProjection::Signed(S::I32),
        RuntimePlanTypeProjection::Unsigned(U::U8),
        RuntimePlanTypeProjection::Unsigned(U::U16),
        RuntimePlanTypeProjection::Unsigned(U::U32),
        RuntimePlanTypeProjection::Unsigned(U::U64),
    ] {
        assert_eq!(
            types.iter().filter(|actual| *actual == &expected).count(),
            1,
            "{expected:?}"
        );
    }
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..RuntimeStepOptions::default()
    };
    let mut native = Engine::for_flow(compiled.plan.clone(), &flow.id).unwrap();
    assert_eq!(
        run_dense_integer_one_op(&mut || native.step(RuntimeStepInput::default(), options)),
        10
    );
    let program = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "dense-widths.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let decoded = AwbcProgram::decode_canonical(
        &program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    assert_eq!(decoded, program);
    let mut product = ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0)).unwrap();
    run_dense_integer_one_op(&mut || product.step(RuntimeStepInput::default(), options));
}

fn run_dense_integer_one_op(
    step: &mut dyn FnMut() -> arcweft_core::step::RuntimeStepResult,
) -> usize {
    use arcweft_core::effect::LineEffectRequest;
    use arcweft_core::engine::{FlowExit, FlowFiberStatus};
    let mut operations = 0;
    let mut messages = Vec::new();
    // The existing measurement budget is 64 steps * 64 ops. This additional
    // execution keeps that total allowance and tightens every step to OneOp/1.
    for _ in 0..(64 * 64) {
        let result = step();
        assert!(result.stats.executed_ops <= 1);
        operations += result.stats.executed_ops;
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        messages.extend(result.output.effects.line.into_iter().filter_map(|effect| {
            if let LineEffectRequest::Log(log) = effect {
                Some(log.message)
            } else {
                None
            }
        }));
        match result.fiber_status {
            FlowFiberStatus::Done(FlowExit::Return(value)) => {
                assert_eq!(value, "done");
                assert_eq!(messages, ["3696"]);
                return operations;
            }
            FlowFiberStatus::Failed(error) => panic!("typed integer fixture failed: {error:?}"),
            _ => {}
        }
    }
    panic!("typed integer fixture did not complete its original budget");
}

fn assert_source_map_one_op_prefix(path: &std::path::Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(path)
        .args([
            "--entry",
            "entry.run",
            "--mode",
            "one-op",
            "--steps",
            "32",
            "--pure-backend",
            "jit",
            "--json",
        ])
        .output()
        .expect("run the original bounded OneOp prefix");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["final_status"], "running");
    assert_eq!(json["steps"].as_array().unwrap().len(), 32);
    assert_source_map_step_output(&json, 1, &[]);
    assert_eq!(sum_step_pure_counter(&json, "pure_calls"), 2);
    assert_eq!(sum_step_pure_counter(&json, "vm_calls"), 2);
    for counter in ["batch_calls", "batch_items", "jit_calls", "aot_calls"] {
        assert_eq!(sum_step_pure_counter(&json, counter), 0, "{counter}");
    }
}

fn assert_source_map_step_output(json: &serde_json::Value, maximum_ops: u64, messages: &[&str]) {
    let mut observed = 0;
    for step in json["steps"].as_array().unwrap() {
        assert!(step["stats"]["executed_ops"].as_u64().unwrap() <= maximum_ops);
        assert!(step["diagnostics"].as_array().unwrap().is_empty());
        let actual = step["observations"]["logs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|log| log["message"].as_str().unwrap())
            .collect::<Vec<_>>();
        // Observations are cumulative snapshots. Every snapshot must retain
        // the exact ordered prefix, and the final snapshot must be complete.
        assert!(actual.len() >= observed);
        assert!(messages.starts_with(&actual), "{actual:?}");
        observed = actual.len();
    }
    assert_eq!(observed, messages.len());
}
