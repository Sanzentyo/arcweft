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
