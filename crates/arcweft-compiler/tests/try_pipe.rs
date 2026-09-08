use arcweft_compiler::source::compile_source;
use arcweft_core::value::RuntimeValue;

#[path = "support/execution.rs"]
mod execution;

fn assert_case(definitions: &str, flow: &str, expected: RuntimeValue, label: &str) {
    let source = format!("entry cli @entry.main {{ goto @flow.main }}\n{definitions}\n{flow}");
    execution::assert_native_return(&source, label);
    execution::assert_awbc_return(&source, expected);
}

#[test]
fn pipe_lowers_the_left_value_once_through_the_admitted_local() {
    assert_case(
        r#"
fn increment(value: i64) -> i64 { value + 1i64 }
fn piped(value: i64) -> i64 { value |> increment(^) }
"#,
        "flow main() -> i64 { return piped(41i64) }",
        RuntimeValue::i64(42),
        "42",
    );
    assert_case(
        "struct State { value: i64 }",
        r#"
flow main() -> i64 {
    let state = State { value = 0i64 }
    let doubled = {
        let value = state.value + 1i64
        state.value = value
        value
    } |> ^ + ^
    return doubled + state.value
}
"#,
        RuntimeValue::i64(3),
        "3",
    );
}

#[test]
fn nested_try_lifts_the_surrounding_expression_into_the_carrier() {
    let definitions = r#"
fn retain(first: Result<i64, String>, second: Result<i64, String>) -> Result<i64, String> {
    let left = try first
    let right = try second
    Ok(left + right)
}
"#;
    assert_case(
        definitions,
        "flow main() -> i64 { return if let Ok(value) = retain(Ok(20i64), Ok(21i64)) { value } else { -1i64 } }",
        RuntimeValue::i64(41),
        "41",
    );
    assert_case(
        definitions,
        "flow main() -> String { return if let Err(message) = retain(Err(\"first\"), Ok(21i64)) { message } else { \"unexpected\" } }",
        RuntimeValue::String("first".to_owned()),
        "first",
    );
    assert_case(
        definitions,
        "flow main() -> String { return if let Err(message) = retain(Ok(20i64), Err(\"second\")) { message } else { \"unexpected\" } }",
        RuntimeValue::String("second".to_owned()),
        "second",
    );
}

#[test]
fn ordinary_terminal_try_is_not_implicitly_wrapped_for_the_return_type() {
    compile_source(
        r#"
fn invalid(value: Result<i64, String>) -> Result<i64, String> { try value }
"#,
    )
    .expect_err("ordinary Try returns its success type and cannot fabricate the outer carrier");
}

#[test]
fn carrier_block_catches_try_before_the_function_boundary() {
    assert_case(
        r#"
fn catch_result(value: Result<i64, String>) -> i64 {
    let kept = result { let unwrapped = try value
        unwrapped }
    if let Ok(item) = kept { item } else { 7i64 }
}
fn catch_option(value: Option<i64>) -> i64 {
    let kept = option { let unwrapped = try value
        unwrapped }
    if let Some(item) = kept { item } else { 3i64 }
}
"#,
        r#"
flow main() -> i64 {
    return catch_result(Err("stop")) + catch_option(None) + catch_result(Ok(30i64)) + catch_option(Some(2i64))
}
"#,
        RuntimeValue::i64(42),
        "42",
    );
}

#[test]
fn try_inside_an_ordinary_block_keeps_the_outer_continuation() {
    let definitions = r#"
fn retain(value: Result<i64, String>) -> Result<i64, String> {
    Ok({ let unwrapped = try value
        unwrapped + 1i64 })
}
"#;
    assert_case(
        definitions,
        "flow main() -> i64 { return if let Ok(value) = retain(Ok(40i64)) { value } else { -1i64 } }",
        RuntimeValue::i64(41),
        "41",
    );
    assert_case(
        definitions,
        "flow main() -> String { return if let Err(message) = retain(Err(\"stop\")) { message } else { \"unexpected\" } }",
        RuntimeValue::String("stop".to_owned()),
        "stop",
    );
}

#[test]
fn pipe_left_binding_precedes_try_in_the_pipe_body() {
    assert_case(
        r#"
struct State { value: i64 }
fn retain() -> Result<i64, String> {
    let state = State { value = 1i64 }
    Ok({ let left = state.value
        state.value = 2i64
        left } |> ^ + try {
            let right: Result<i64, String> = Ok(40i64)
            state.value = 3i64
            right
        })
}
"#,
        "flow main() -> i64 { return if let Ok(value) = retain() { value } else { -1i64 } }",
        RuntimeValue::i64(41),
        "41",
    );
}

#[test]
fn try_remains_inside_its_if_branch() {
    let definitions = r#"
fn choose(flag: bool, left: Result<i64, String>, right: Result<i64, String>) -> Result<i64, String> {
    if flag { Ok(try left) } else { Ok(try right) }
}
"#;
    assert_case(
        definitions,
        "flow main() -> i64 { return if let Ok(value) = choose(true, Ok(41i64), Err(\"unselected\")) { value } else { -1i64 } }",
        RuntimeValue::i64(41),
        "41",
    );
    assert_case(
        definitions,
        "flow main() -> i64 { return if let Ok(value) = choose(false, Err(\"unselected\"), Ok(42i64)) { value } else { -1i64 } }",
        RuntimeValue::i64(42),
        "42",
    );
}

#[test]
fn try_remains_inside_its_match_arm() {
    let definitions = r#"
fn choose(flag: bool, value: Result<i64, String>) -> Result<i64, String> {
    match flag {
        true => Ok(try value)
        false => Ok(0i64)
    }
}
"#;
    assert_case(
        definitions,
        "flow main() -> i64 { return if let Ok(value) = choose(true, Ok(41i64)) { value } else { -1i64 } }",
        RuntimeValue::i64(41),
        "41",
    );
    assert_case(
        definitions,
        "flow main() -> i64 { return if let Ok(value) = choose(false, Err(\"unselected\")) { value } else { -1i64 } }",
        RuntimeValue::i64(0),
        "0",
    );
    assert_case(
        definitions,
        "flow main() -> String { return if let Err(message) = choose(true, Err(\"selected\")) { message } else { \"unexpected\" } }",
        RuntimeValue::String("selected".to_owned()),
        "selected",
    );
}
