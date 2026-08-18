use arcweft_compiler::source::compile_source;
use arcweft_core::pure::VmPureFunctionScratch;
use arcweft_core::value::RuntimeExprKind;
use arcweft_core::value::RuntimeValue;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::sync::Arc;

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

#[test]
fn nested_pure_try_lifts_the_surrounding_expression_into_the_carrier() {
    let compiled = compile_source(
        r#"
entry cli @entry.main { goto @flow.main }

fn retain(
    first: Result<i64, String>,
    second: Result<i64, String>,
) -> Result<i64, String> {
    let left = try first
    let right = try second
    Ok(left + right)
}

flow main() -> Result<i64, String> {
    return retain(Ok(20i64), Ok(21i64))
}
"#,
    )
    .expect("nested pure Try compiles through the carrier continuation");

    assert_eq!(compiled.plan.pure_helpers().len(), 1);
    AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "try_pipe.arcw")
        .lower()
        .expect("nested pure Try lowers to verified product AWBC");
    let helper = compiled.plan.pure_helpers()[0].id;
    let plan = Arc::new(compiled.plan);
    let mut evaluator = VmPureFunctionScratch::default();
    assert_eq!(
        evaluator
            .evaluate_values(
                &plan,
                helper,
                &[
                    RuntimeValue::result_ok(RuntimeValue::i64(20)),
                    RuntimeValue::result_ok(RuntimeValue::i64(21)),
                ],
            )
            .expect("success path evaluates"),
        RuntimeValue::result_ok(RuntimeValue::i64(41)),
    );
    assert_eq!(
        evaluator
            .evaluate_values(
                &plan,
                helper,
                &[
                    RuntimeValue::result_err(RuntimeValue::String("first".to_owned())),
                    RuntimeValue::result_ok(RuntimeValue::i64(21)),
                ],
            )
            .expect("residual path evaluates"),
        RuntimeValue::result_err(RuntimeValue::String("first".to_owned())),
    );
}

#[test]
fn pure_carrier_block_catches_try_before_the_function_boundary() {
    let compiled = compile_source(
        r#"
fn retain_result(value: Result<i64, String>) -> Result<i64, String> {
    result {
        let unwrapped = try value
        unwrapped
    }
}

fn retain_option(value: Option<i64>) -> Option<i64> {
    option {
        let unwrapped = try value
        unwrapped
    }
}

fn exercise() -> String {
    let result = retain_result(Ok(41i64))
    let option = retain_option(Some(41i64))
    "ok"
}

flow main() -> String {
    return exercise()
}
"#,
    )
    .expect("pure carrier block catches its checked Try residual");

    assert_eq!(compiled.plan.pure_helpers().len(), 3);
}

#[test]
fn pure_try_inside_an_ordinary_block_keeps_the_outer_continuation() {
    let compiled = compile_source(
        r#"
fn retain(value: Result<i64, String>) -> Result<i64, String> {
    Ok({
        let unwrapped = try value
        unwrapped + 1i64
    })
}

flow main() -> Result<i64, String> {
    return retain(Ok(40i64))
}
"#,
    )
    .expect("pure Try in an ordinary block retains its strict outer continuation");

    assert_eq!(compiled.plan.pure_helpers().len(), 1);
}

#[test]
fn pipe_left_binding_precedes_try_in_the_pipe_body() {
    let compiled = compile_source(
        r#"
fn retain(left: i64, right: Result<i64, String>) -> Result<i64, String> {
    Ok(left |> ^ + try right)
}

flow main() -> Result<i64, String> {
    return retain(1i64, Ok(40i64))
}
"#,
    )
    .expect("pipe binds its left value before evaluating Try in the body");

    let helper = compiled
        .plan
        .pure_helpers()
        .iter()
        .find(|helper| helper.name.ends_with("retain"))
        .expect("retained helper");
    let RuntimeExprKind::Let { body, .. } = helper.expr.kind() else {
        panic!("pipe-left once-only binding must be the outer runtime expression");
    };
    assert!(matches!(body.kind(), RuntimeExprKind::Match { .. }));
    let helper = helper.id;
    let plan = Arc::new(compiled.plan);
    assert_eq!(
        VmPureFunctionScratch::default()
            .evaluate_values(
                &plan,
                helper,
                &[
                    RuntimeValue::i64(1),
                    RuntimeValue::result_ok(RuntimeValue::i64(40)),
                ],
            )
            .expect("pipe/Try helper evaluates"),
        RuntimeValue::result_ok(RuntimeValue::i64(41)),
    );
}

#[test]
fn pure_try_remains_inside_its_if_branch() {
    let compiled = compile_source(
        r#"
fn choose(
    flag: bool,
    left: Result<i64, String>,
    right: Result<i64, String>,
) -> Result<i64, String> {
    if flag {
        Ok(try left)
    } else {
        Ok(try right)
    }
}

flow main() -> Result<i64, String> {
    return choose(true, Ok(41i64), Ok(42i64))
}
"#,
    )
    .expect("pure Try remains branch-local during continuation lowering");

    assert_eq!(compiled.plan.pure_helpers().len(), 1);
}

#[test]
fn pure_try_remains_inside_its_match_arm() {
    let compiled = compile_source(
        r#"
fn choose(flag: bool, value: Result<i64, String>) -> Result<i64, String> {
    match flag {
        true => Ok(try value)
        false => Ok(0i64)
    }
}

flow main() -> Result<i64, String> {
    return choose(true, Ok(41i64))
}
"#,
    )
    .expect("pure Try remains arm-local during continuation lowering");

    assert_eq!(compiled.plan.pure_helpers().len(), 1);
}
