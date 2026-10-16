#[test]
fn source_map_retains_zero_argument_target_calls_when_source_items_are_unused() {
    for (label, input, output) in [
        ("array", "Array<String, 3>", "Array<i64, 3>"),
        ("vec", "Vec<String>", "Vec<i64>"),
    ] {
        let source = format!(
            r#"
entry cli @entry.run {{ goto @flow.map_packet }}
#[pure]
fn score() -> i64 {{ return 7i64 }}
flow map_packet() -> String {{
    let values: {input} = ["a", "bb", "ccc"]
    let scores: {output} = values.map(|_| score())
    let total: i64 = scores.sum()
    if total != 21i64 {{ return "wrong" }}
    return "done"
}}
"#,
        );
        let path = temp_arcw(&format!("map-zero-argument-{label}"), &source);
        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("run")
            .arg(&path)
            .args([
                "--entry",
                "entry.run",
                "--mode",
                "drain",
                "--steps",
                "32",
                "--max-ops",
                "4096",
                "--pure-backend",
                "jit",
                "--json",
            ])
            .output()
            .expect("run the original source Map with an unused source item");
        assert!(
            output.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            json["final_status"], "done Return(\"done\")",
            "{label}: {json}"
        );
        assert_eq!(
            sum_step_pure_counter(&json, "pure_calls"),
            3,
            "{label}: {json}"
        );
        assert_eq!(
            sum_step_pure_counter(&json, "batch_calls"),
            1,
            "{label}: {json}"
        );
        assert_eq!(
            sum_step_pure_counter(&json, "batch_items"),
            3,
            "{label}: {json}"
        );
        assert_eq!(
            sum_step_pure_counter(&json, "jit_calls"),
            3,
            "{label}: {json}"
        );
        for counter in [
            "vm_calls",
            "aot_calls",
            "fallbacks",
            "arg_stack_packs",
            "arg_vec_allocations",
            "arg_bytes_borrowed",
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
    }
}
