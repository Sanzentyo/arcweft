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
        for (mode, maximum_ops) in [("drain", 64), ("one-op", 1)] {
            let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
                .arg("run")
                .arg(&path)
                .args([
                    "--entry",
                    "entry.run",
                    "--mode",
                    mode,
                    "--steps",
                    "4096",
                    "--max-ops",
                    "64",
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
            if mode == "drain" {
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
            } else {
                for counter in ["batch_calls", "batch_items", "jit_calls", "aot_calls"] {
                    assert_eq!(
                        sum_step_pure_counter(&json, counter),
                        0,
                        "{label}/{counter}: {json}"
                    );
                }
                assert_eq!(sum_step_pure_counter(&json, "vm_calls"), 128);
            }
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
