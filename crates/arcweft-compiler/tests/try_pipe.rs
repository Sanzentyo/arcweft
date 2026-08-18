use arcweft_compiler::source::compile_source;

#[test]
fn pure_pipe_lowers_the_left_value_once_through_the_admitted_local() {
    let compiled = compile_source(
        r#"
fn increment(value: i64) -> i64 {
    value + 1i64
}

fn piped(value: i64) -> i64 {
    value |> increment(^)
}

flow main() -> i64 {
    return piped(41i64)
}
"#,
    )
    .expect("pure pipe compiles through the checked once-only binding");

    assert_eq!(compiled.plan.pure_helpers().len(), 2);
}
