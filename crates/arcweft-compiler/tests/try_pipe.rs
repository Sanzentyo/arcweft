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
fn a_root_try_delivers_success_and_residual_to_the_closure_caller() {
    for callable in [
        "|value| try value",
        "try _",
        "|value| try keep(value)",
        "try { let _ = keep(()); _ }",
        "try keep(_)",
    ] {
        let definitions = format!(
            r#"
fn keep<T>(value: T) -> T {{ value }}
fn relay_result(input: Result<i64, String>) -> Result<i64, String> {{
    let apply: (Result<i64, String> -> Result<i64, String> effects {{}}) = {callable}
    apply(input)
}}
fn relay_option(input: Option<i64>) -> Option<i64> {{
    let apply: (Option<i64> -> Option<i64> effects {{}}) = {callable}
    apply(input)
}}
"#,
        );
        assert_case(
            &definitions,
            "flow main() -> i64 { return if let Ok(value) = relay_result(Ok(42i64)) { value } else { -1i64 } }",
            RuntimeValue::i64(42),
            "42",
        );
        assert_case(
            &definitions,
            "flow main() -> String { return if let Err(reason) = relay_result(Err(\"stopped\")) { reason } else { \"unexpected\" } }",
            RuntimeValue::String("stopped".to_owned()),
            "stopped",
        );
        assert_case(
            &definitions,
            "flow main() -> i64 { return if let Some(value) = relay_option(Some(42i64)) { value } else { -1i64 } }",
            RuntimeValue::i64(42),
            "42",
        );
        assert_case(
            &definitions,
            "flow main() -> i64 { return if let Some(value) = relay_option(None) { value } else { -1i64 } }",
            RuntimeValue::i64(-1),
            "-1",
        );
    }
}

#[test]
fn a_root_try_rejects_a_mismatched_wrapped_success_payload() {
    for callable in ["|value| try value", "try _"] {
        let source = format!(
            "fn relay(input: Result<i64, String>) -> Result<bool, String> {{
                let apply: (Result<i64, String> -> Result<bool, String> effects {{}}) = {callable};
                apply(input)
            }}"
        );
        let error = compile_source(&source).expect_err("the returned carrier expects Bool");
        assert_eq!(error.project().stage(), "type-check");
    }
}

#[test]
fn implicit_call_arguments_keep_their_selected_callable_owner() {
    let definitions = r#"
fn keep<T>(value: T) -> T { value }
fn add(left: i64, right: i64) -> i64 { left + right }
fn apply(f: (i64 -> i64 effects {}), value: i64) -> i64 { f(value) }
fn retain(f: (i64 -> i64 effects {}), ignored: i64) -> (i64 -> i64 effects {}) { f }
fn add_to(offset: i64) -> (i64 -> i64 effects {}) {
    let f: (i64 -> i64 effects {}) = add(right = offset + 1i64, left = _)
    f
}
fn with_identity(offset: i64) -> (i64 -> i64 effects {}) {
    let f: (i64 -> i64 effects {}) = { let bonus = apply(_, offset); add(_, bonus) }
    f
}
fn with_nested_identity(offset: i64) -> (i64 -> i64 effects {}) {
    let f: (i64 -> i64 effects {}) = { let bonus = keep(apply(_, offset)); add(_, bonus) }
    f
}
"#;
    for flow in [
        "flow main() -> i64 { let f: (i64 -> i64 effects {}) = keep(_); return f(42i64) }",
        "flow main() -> i64 { let f: (i64 -> i64 effects {}) = keep(_) + 1i64; return f(41i64) }",
        "flow main() -> i64 { let f: (i64 -> i64 effects {}) = add(_, _); return f(21i64) }",
        "flow main() -> i64 { let f = add_to(2i64); return f(39i64) }",
        "flow main() -> i64 { let f = with_identity(2i64); return f(40i64) }",
        "flow main() -> i64 { let f = with_nested_identity(2i64); return f(40i64) }",
        "flow main() -> i64 { return apply(_, 42i64) }",
        "flow main() -> i64 { let f: (i64 -> i64 effects {}) = retain(_, 0i64); return f(42i64) }",
        "flow main() -> i64 { return apply(keep(_), 42i64) }",
        "flow main() -> i64 { return apply(_ + 1i64, 41i64) }",
    ] {
        assert_case(definitions, flow, RuntimeValue::i64(42), "42");
    }
}

#[test]
fn an_implicit_parameter_can_supply_a_returned_closure_callee() {
    assert_case(
        r#"
fn handoff(callback: (i64 -> i64 effects {})) -> (i64 -> i64 effects {}) {
    let relay: ((i64 -> i64 effects {}) -> (i64 -> i64 effects {}) effects {}) = {
        let forwarded = _;
        |n: i64| -> i64 { forwarded(n) }
    };
    relay(callback)
}
"#,
        "flow main() -> i64 { let callback = handoff(|n: i64| n + 1i64); return callback(41i64) }",
        RuntimeValue::i64(42),
        "42",
    );
}

#[test]
fn an_implicit_call_body_can_diverge_under_a_contextual_return_type() {
    compile_source(
        r#"
entry cli @entry.main { goto @flow.main }
fn diverge(value: i64) -> Never { loop {} }
fn relay(input: i64) -> i64 {
    let f: (i64 -> i64 effects {}) = diverge(_)
    f(input)
}
flow main() -> i64 { return relay(42i64) }
"#,
    )
    .expect("a diverging implicit call body retains its contextual function result");
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
fn a_carrier_residual_can_finish_before_a_nonlocal_return() {
    for (ty, block, residual, success) in [
        ("Option<i64>", "option", "None", "Some(23i64)"),
        ("Result<i64, i64>", "result", "Err(7i64)", "Ok(23i64)"),
    ] {
        let body = format!("{block} {{ let value = try input; return value; 0i64 }}; return 42i64");
        let function = format!("fn classify(input: {ty}) -> i64 {{ {body} }}");
        for (input, expected, label) in [(residual, 42, "42"), (success, 23, "23")] {
            let flow = format!("flow main() -> i64 {{ let input: {ty} = {input}; {body} }}");
            assert_case("", &flow, RuntimeValue::i64(expected), label);
            let call = format!("flow main() -> i64 {{ return classify({input}) }}");
            assert_case(&function, &call, RuntimeValue::i64(expected), label);
        }
    }
}

#[test]
fn a_dead_try_does_not_revive_a_terminal_carrier() {
    for (ty, block, residual) in [
        ("Option<i64>", "option", "None"),
        ("Result<i64, i64>", "result", "Err(7i64)"),
    ] {
        let source = format!(
            "flow main() -> i64 {{ let input: {ty} = {residual}; {block} {{ return 23i64; try input }}; return 42i64 }}"
        );
        assert_case("", &source, RuntimeValue::i64(23), "23");
    }
    assert_case(
        "fn keep(first: i64, second: i64) -> i64 { first }",
        r#"
flow main() -> i64 {
    let input: Option<i64> = None
    option { keep(scope { return 23i64; 0i64 }, try input) }
    return 42i64
}
"#,
        RuntimeValue::i64(23),
        "23",
    );
}

#[test]
fn a_pure_carrier_without_a_delivery_has_type_never() {
    assert_case(
        r#"
pub fn impossible(value: Never) -> Never {
    option { value }
}
pub fn impossible_after_prefix(value: Never, input: Option<i64>) -> Never {
    option {
        let _ = value
        try input
    }
}
pub fn impossible_in_scope(value: Never, input: Option<i64>) -> Never {
    option {
        scope {
            let _ = value
            try input
        }
    }
}
"#,
        "flow main() -> i64 { let direct = impossible_after_prefix; let nested = impossible_in_scope; return 42i64 }",
        RuntimeValue::i64(42),
        "42",
    );
}

#[test]
fn a_residual_only_carrier_uses_the_expected_success_type() {
    assert_case(
        "",
        r#"
flow main() -> i64 {
    let input: Option<i64> = None
    let outcome: Option<i64> = option {
        let value = try input
        scope { return value; 0i64 }
    }
    return if let Some(value) = outcome { value } else { 42i64 }
}
"#,
        RuntimeValue::i64(42),
        "42",
    );
    assert_case(
        "",
        r#"
flow main() -> i64 {
    let input: Result<i64, i64> = Err(42i64)
    let outcome: Result<i64, i64> = result {
        let value = try input
        scope { return value; 0i64 }
    }
    return if let Err(error) = outcome { error } else { 0i64 }
}
"#,
        RuntimeValue::i64(42),
        "42",
    );
    assert_case(
        r#"
fn direct(value: Never, input: Option<i64>) -> Option<i64> {
    option { let seen = try input; value }
}
fn nested(value: Never, input: Result<i64, i64>) -> Result<i64, i64> {
    result {
        scope { let seen = try input; let _ = value; 0i64 }
    }
}
"#,
        "flow main() -> i64 { let option_case = direct; let result_case = nested; return 42i64 }",
        RuntimeValue::i64(42),
        "42",
    );
}

#[test]
fn result_residual_can_finish_before_a_nonlocal_loop_break() {
    for (input, expected, label) in [("Err(7i64)", 42, "42"), ("Ok(23i64)", 23, "23")] {
        assert_case(
            "",
            &format!(
                "flow main() -> i64 {{ let input: Result<i64, i64> = {input}; let answer = loop {{ result {{ let value = try input; break value; 0i64 }}; break 42i64 }}; return answer }}"
            ),
            RuntimeValue::i64(expected),
            label,
        );
    }
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
