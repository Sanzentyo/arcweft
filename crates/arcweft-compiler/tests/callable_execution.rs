use arcweft_compiler::source::compile_source;
use arcweft_core::value::RuntimeValue;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[path = "support/execution.rs"]
mod execution;
use execution::{assert_awbc_return, assert_native_return};

#[test]
fn ordinary_closure_parameters_reject_trailing_input() {
    for pattern in [
        "_ trailing",
        "[value] trailing",
        "(value,) trailing",
        "42 trailing",
    ] {
        let source = format!(
            "flow main() -> Unit {{ let invalid = |{pattern}| 0; }}\nentry cli @entry.main {{ goto @flow.main }}\n",
        );
        let error = compile_source(&source).expect_err("malformed closure pattern cannot compile");
        let diagnostic = error
            .project()
            .diagnostics()
            .iter()
            .filter_map(|diagnostic| diagnostic.syntax_diagnostic())
            .find(|diagnostic| diagnostic.code() == "syntax.pattern.unexpected_trailing_input")
            .expect("compiler retains the exact Pattern grammar diagnostic");
        let range = diagnostic.primary().range();
        assert_eq!(&source[range.start()..range.end()], "trailing", "{pattern}");
    }
}

macro_rules! callable_case {
    ($name:ident, $source:literal, $expected:expr, $label:literal) => {
        mod $name {
            use super::*;

            const SOURCE: &str = concat!("entry cli @entry.main { goto @flow.main }\n", $source,);

            #[test]
            fn native() {
                assert_native_return(SOURCE, $label);
            }

            #[test]
            fn awbc() {
                assert_awbc_return(SOURCE, $expected);
            }
        }
    };
}

callable_case!(
    direct_call,
    r#"
fn increment(value: i64) -> i64 { value + 1i64 }
flow main() -> i64 { return increment(41i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    callback_with_project_call_body,
    r#"
fn increment(value: i64) -> i64 { value + 1i64 }
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
flow main() -> i64 {
    let handler = |value: i64| increment(value)
    return apply(handler, 41i64)
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    callback_with_inferred_effects,
    r#"
fn increment(value: i64) -> i64 { value + 1i64 }
fn apply(handler: i64 -> i64, value: i64) -> i64 { handler(value) }
flow main() -> i64 {
    let handler = |value: i64| increment(value)
    return apply(handler, 41i64)
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    uninvoked_callback_with_inferred_effects,
    r#"
fn writer(value: i64) -> i64 effects { fs.write } { value }
fn ignore(unused: i64 -> i64, value: i64) -> i64 { value }
flow main() -> i64 { return ignore(|value: i64| writer(value), 42i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    contextual_closure_in_a_block_argument,
    r#"
fn apply(callback: i64 -> i64 effects {}) -> i64 { callback(41i64) }
flow main() -> i64 {
    let answer = apply({ let offset = 1i64
        |value| value + offset })
    return answer
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    curried_prefix_as_callback,
    r#"
fn add(first: i64)(second: i64) -> i64 { first + second }
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
flow main() -> i64 {
    let prefix = add(1i64)
    return apply(prefix, 41i64)
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    nested_generic_calls,
    r#"
fn identity<T>(value: T) -> T { value }
fn twice<T>(value: T) -> T { identity(identity(value)) }
flow main() -> i64 { return twice(42i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    character_factory_branches_keep_both_selected_calls,
    r#"
pub character alice {}
pub character bob {}
fn create_dialogues(condition: bool) -> i64 {
    let first = if condition { alice() } else { bob() }
    let second = if !condition { alice() } else { bob() }
    42i64
}
flow main() -> i64 { return create_dialogues(true) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    project_variant_keeps_unselected_payload_types,
    r#"
enum Event { Empty, Text String }
fn create() -> Event { .Empty }
flow main() -> i64 { let event = create(); return 42i64 }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    project_variant_constructor_keeps_unselected_payload_types,
    r#"
enum Event { Empty, Text String }
fn create() -> Event { .Empty() }
flow main() -> i64 { let event = create(); return 42i64 }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    direct_variant_constructor_keeps_unselected_payload_types,
    r#"
enum Event { Empty, Text String }
flow main() -> i64 { let event: Event = .Empty(); return 42i64 }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    nested_closure_variant_keeps_unselected_payload_types,
    r#"
enum Event { Empty, Text String }
fn create() -> Event {
    let build = || -> Event { .Empty }
    build()
}
flow main() -> i64 { let event = create(); return 42i64 }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    root_closure_variant_keeps_unselected_payload_types,
    r#"
enum Event { Empty, Text String }
flow main() -> i64 {
    let create = || -> Event { .Empty }
    let event = create()
    return 42i64
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    mutually_recursive_functions,
    r#"
fn is_even(value: i64) -> bool {
    if value == 0i64 { true } else { is_odd(value - 1i64) }
}
fn is_odd(value: i64) -> bool {
    if value == 0i64 { false } else { is_even(value - 1i64) }
}
flow main() -> bool { return is_even(4i64) }
"#,
    RuntimeValue::Bool(true),
    "true"
);

callable_case!(
    and_skips_the_right_call,
    r#"
fn recurse() -> bool { recurse() }
flow main() -> bool { return false && recurse() }
"#,
    RuntimeValue::Bool(false),
    "false"
);

callable_case!(
    or_skips_the_right_call,
    r#"
fn recurse() -> bool { recurse() }
flow main() -> bool { return true || recurse() }
"#,
    RuntimeValue::Bool(true),
    "true"
);

callable_case!(
    short_circuit_evaluates_the_selected_call,
    r#"
fn identity(value: bool) -> bool { value }
flow main() -> bool { return (true && identity(false)) || identity(true) }
"#,
    RuntimeValue::Bool(true),
    "true"
);

callable_case!(
    match_skips_unselected_arm_calls,
    r#"
fn recurse() -> i64 { recurse() }
fn identity(value: i64) -> i64 { value }
fn choose(value: i64) -> i64 {
    match identity(value) {
        0i64 => identity(42i64),
        _ => recurse(),
    }
}
flow main() -> i64 { return choose(0i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    if_let_skips_the_unselected_branch_call,
    r#"
fn recurse() -> i64 { recurse() }
fn choose(input: Option<i64>) -> i64 {
    if let Some(value) = input { value } else { recurse() }
}
flow main() -> i64 { return choose(Some(42i64)) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    generic_option_payload_keeps_the_instantiated_owner,
    r#"
fn choose<T>(input: Option<T>, fallback: T) -> T {
    if let Some(value) = input { value } else { fallback }
}
flow main() -> i64 { return choose(Some(42i64), 0i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    later_argument_closes_an_earlier_contextual_variant,
    r#"
fn choose<T>(input: Option<T>, fallback: T) -> T {
    if let Some(value) = input { value } else { fallback }
}
flow main() -> i64 { return choose(None, 42i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    recursive_generic,
    r#"
fn repeat<T>(value: T, count: i64) -> T {
    if count == 0i64 { value } else { repeat(value, count - 1i64) }
}
flow main() -> i64 { return repeat(42i64, 3i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    shared_prefix_with_distinct_later_types,
    r#"
fn choose<A, B>(first: A)(second: B) -> B { second }
flow main() -> i64 {
    let prefix = choose(1i64)
    let text = prefix("text")
    return prefix(42i64)
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    generic_prefix_as_monomorphic_callback,
    r#"
fn choose<A, B>(first: A)(second: B) -> B { second }
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
flow main() -> i64 {
    let prefix = choose("first")
    let text = prefix("text")
    return apply(prefix, 42i64)
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    later_argument_closes_a_project_enum_owner,
    r#"
enum Slot<T> { Empty, Full T }
fn fallback<T>(input: Slot<T>, value: T) -> T { value }
flow main() -> i64 { return fallback(.Empty, 42i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    contextual_project_variant_payload_closes_its_owner,
    r#"
enum Slot<T> { Empty, Full T }
fn fallback<T>(input: Slot<T>, value: T) -> T {
    if let .Full(item) = input { item } else { value }
}
flow main() -> i64 { return fallback(.Full(42i64), 0i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    contextual_project_unit_constructor_closes_with_a_later_argument,
    r#"
enum Slot<T> { Empty, Full T }
fn fallback<T>(input: Slot<T>, value: T) -> T { value }
flow main() -> i64 { return fallback(.Empty(), 42i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    correlated_ordinary_call_closes_from_a_later_parent_argument,
    r#"
fn empty<T>() -> Option<T> { None }
fn fallback<T>(input: Option<T>, value: T) -> T { value }
flow main() -> i64 { return fallback(empty(), 42i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    correlated_ordinary_calls_combine_complementary_parent_evidence,
    r#"
enum Either<A, B> { Left A, Right B }
fn left<A, B>(value: A) -> Either<A, B> { .Left(value) }
fn right<A, B>(value: B) -> Either<A, B> { .Right(value) }
fn combine<A, B>(left: Either<A, B>, right: Either<A, B>) -> i64 { 42i64 }
flow main() -> i64 {
    let first = combine(left(1i64), right("two"))
    let second = combine(right("two"), left(1i64))
    return first + second
}
"#,
    RuntimeValue::i64(84),
    "84"
);

callable_case!(
    contextual_project_constructor_infers_an_unselected_case_parameter,
    r#"
enum Either<A, B> { Left A, Right B }
fn fallback<A, B>(input: Either<A, B>, value: A, other: B) -> A {
    if let .Left(item) = input { item } else { value }
}
flow main() -> i64 { return fallback(.Left(42i64), 0i64, "other") }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    contextual_constructor_sources_combine_complementary_type_evidence,
    r#"
enum Either<A, B> { Left A, Right B }
fn combine<A, B>(left: Either<A, B>, right: Either<A, B>) -> i64 { 42i64 }
flow main() -> i64 { return combine(.Left(1i64), .Right("two")) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    contextual_project_constructor_retains_the_enclosing_parameter,
    r#"
enum Slot<T> { Empty, Full T }
fn pack<T>(value: T) -> Slot<T> { .Full(value) }
fn fallback<T>(input: Slot<T>, value: T) -> T {
    if let .Full(item) = input { item } else { value }
}
flow main() -> i64 { return fallback(pack(42i64), 0i64) }
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    root_closure_captures_its_enclosing_local,
    r#"
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
flow main() -> i64 {
    let offset = 1i64
    let handler = |value: i64| value + offset
    return apply(handler, 41i64)
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    root_closure_capture_and_project_call_share_one_frame,
    r#"
fn increment(value: i64) -> i64 { value + 1i64 }
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
flow main() -> i64 {
    let offset = 1i64
    let handler = |value: i64| increment(value) + offset
    return apply(handler, 40i64)
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    nested_root_closure_returns_an_executable_capture,
    r#"
fn increment(value: i64) -> i64 { value + 1i64 }
flow main() -> i64 {
    let factory = |offset: i64| { |value: i64| increment(value) + offset }
    let handler = factory(1i64)
    return handler(40i64)
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    function_callee_is_captured_before_a_later_call_argument,
    r#"
struct State { value: i64 }
fn identity(value: i64) -> i64 { value }
flow main() -> i64 {
    let state = State { value = 1i64 }
    return (|value: i64| value + state.value)({
        let input = 41i64
        state.value = 2i64
        identity(input)
    })
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    nominal_binary_read_precedes_later_operand_evaluation,
    r#"
struct State { value: i64 }
fn identity(value: i64) -> i64 { value }
flow main() -> i64 {
    let state = State { value = 1i64 }
    return state.value + {
        let input = 41i64
        state.value = 2i64
        identity(input)
    }
}
"#,
    RuntimeValue::i64(42),
    "42"
);

#[test]
fn nominal_field_awbc_roundtrip_rejects_wrong_ordinals_and_replacement_types() {
    use arcweft_core::awbc::codec::AwbcDecodeBudget;
    use arcweft_core::awbc::schema::{AwbcInstruction, AwbcProgram};
    use arcweft_core::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError};

    let compiled = compile_source(
        r#"
entry cli @entry.main { goto @flow.main }
struct State { value: i64 }
flow main() -> i64 {
    let state = State { value = 1i64 }
    state.value = 42i64
    return state.value
}
"#,
    )
    .expect("nominal field program compiles");
    let report = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "nominal_field.arcw",
    )
    .lower()
    .expect("nominal field program verifies");
    let bytes = report
        .program
        .encode_canonical()
        .expect("canonical field instructions encode");
    let program = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default())
        .expect("canonical field instructions decode");
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("decoded nominal field program verifies");

    let mut wrong_write = program.clone();
    let write = wrong_write
        .instructions
        .iter_mut()
        .find(|instruction| matches!(instruction, AwbcInstruction::AssignRecordField { .. }))
        .expect("assignment instruction");
    let AwbcInstruction::AssignRecordField { field, .. } = write else {
        unreachable!()
    };
    *field = 1;
    assert!(matches!(
        wrong_write.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::InvalidInvariant { .. })
    ));

    let mut wrong_value = program.clone();
    let write = wrong_value
        .instructions
        .iter_mut()
        .find(|instruction| matches!(instruction, AwbcInstruction::AssignRecordField { .. }))
        .expect("assignment instruction");
    let AwbcInstruction::AssignRecordField { target, value, .. } = write else {
        unreachable!()
    };
    *value = *target;
    assert!(matches!(
        wrong_value.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::TypeMismatch { .. })
    ));

    let mut wrong_read = program;
    let read = wrong_read
        .instructions
        .iter_mut()
        .find(|instruction| matches!(instruction, AwbcInstruction::ProjectRecord { .. }))
        .expect("ordinal projection instruction");
    let AwbcInstruction::ProjectRecord { ordinal, .. } = read else {
        unreachable!()
    };
    *ordinal = 1;
    assert!(matches!(
        wrong_read.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::IndexOutOfBounds { .. })
    ));
}
