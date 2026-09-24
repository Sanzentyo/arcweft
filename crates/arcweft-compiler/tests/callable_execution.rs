use arcweft_character::id::CharacterId;
use arcweft_compiler::source::compile_source;
use arcweft_core::{
    pattern::RuntimeBuiltinVariantCaseIdentity,
    task::RuntimeProgramOwner,
    value::{RuntimeEntityReference, RuntimeValue},
};
use arcweft_dialogue::{CharacterDialogueRuntimeSchema, CharacterDialogueType};
use arcweft_interaction_model::dialogue::CharacterDialogueOperation;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::sync::Arc;

#[path = "support/execution.rs"]
mod execution;
use execution::{
    assert_awbc_return, assert_native_return, bind_character_dialogue_schema,
    execute_decoded_awbc_character_dialogue_calls, execute_native_character_dialogue_calls,
};

#[test]
fn function_value_calls_do_not_merge_argument_groups() {
    const SOURCE: &str = r#"
entry cli @entry.main { goto @flow.main }
fn increment(value: i64) -> i64 { value + 1i64 }
flow main() -> i64 {
    let factory = |offset: i64| { |value: i64| increment(value) + offset }
    let handler = factory(1i64)
    return handler(40i64)
}
"#;
    compile_source(SOURCE).expect("separate applications of the two function groups compile");
    let merged = SOURCE.replace(
        "let handler = factory(1i64)\n    return handler(40i64)",
        "return factory(1i64, 40i64)",
    );
    let error = compile_source(&merged).expect_err("surplus arguments cannot enter the next group");
    let [diagnostic] = error.project().diagnostics() else {
        panic!("one source-backed call diagnostic: {error:?}");
    };
    assert_eq!(
        diagnostic.stage(),
        arcweft_compiler::project::ProjectCompileStage::TypeCheck
    );
    assert_eq!(
        diagnostic
            .diagnostic()
            .code()
            .map(arcweft_source::DiagnosticCode::as_str),
        Some("sema.call.no_viable_signature")
    );
    assert!(diagnostic.syntax_diagnostic().is_none());
    assert!(diagnostic.source().is_some());
    let span = diagnostic.diagnostic().span().unwrap();
    assert_eq!(&merged[span.range().as_range()], "factory(1i64, 40i64)");
}

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
    curried_terminal_effect,
    r#"
fn staged(first: i64)(second: i64)(third: i64) -> i64 effects { fs.read } {
    first + second + third
}
flow main() -> i64 {
    let first = staged(1i64)
    let second = first(2i64)
    return second(39i64)
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
    named_curried_function_value,
    r#"
fn add(first: i64)(second: i64) -> i64 { first + second }
flow main() -> i64 {
    let factory = add
    let prefix = factory(1i64)
    return prefix(41i64)
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    callable_origins_remain_distinct_across_a_branch,
    r#"
fn add(first: i64)(second: i64) -> i64 { first + second }
fn subtract(first: i64)(second: i64) -> i64 { second - first }
flow main() -> i64 {
    let left = if true { add } else { subtract }
    let right = if false { add } else { subtract }
    return left(1i64)(20i64) + right(1i64)(22i64)
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    closure_retains_a_project_prefix,
    r#"
fn add(first: i64)(second: i64) -> i64 { first + second }
flow main() -> i64 {
    let prefix = add(1i64)
    let invoke = || prefix(41i64)
    return invoke()
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
    callback_returns_a_nonterminal_prefix,
    r#"
fn sum(first: i64)(second: i64)(third: i64) -> i64 { first + second + third }
fn advance(handler: i64 -> (i64 -> i64 effects {}) effects {}, value: i64) -> (i64 -> i64 effects {}) {
    handler(value)
}
flow main() -> i64 {
    let prefix = sum(1i64)
    let left = advance(prefix, 20i64)
    let right = advance(prefix, 30i64)
    return left(21i64) + right(11i64)
}
"#,
    RuntimeValue::i64(84),
    "84"
);

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one native/AWBC fixture checks branch selection, exact payload defaults, and patches together"
)]
fn character_factory_branches_preserve_values_through_native_and_decoded_awbc() {
    const SOURCE: &str = r#"
pub character alice {}
pub character bob {}
fn select_dialogue(condition: bool) -> CharacterDialogue {
    if condition { alice() } else { bob() }
}
flow main() -> Unit {
    let from_alice = select_dialogue(true)
    let from_bob = select_dialogue(false)
    let localized_alice = from_alice(source_locale = "ja-JP")
    let localized_bob = from_bob(source_locale = "en-US")
}
entry cli @entry.main { goto @flow.main }
"#;

    let compiled = compile_source(SOURCE).expect("both Character branches compile");
    let native = execute_native_character_dialogue_calls(&compiled);
    let awbc = execute_decoded_awbc_character_dialogue_calls(&compiled);
    assert_eq!(native, awbc, "native and decoded AWBC produce equal values");

    let alice = CharacterId::try_new("character.alice").unwrap();
    let bob = CharacterId::try_new("character.bob").unwrap();
    let expected = [
        (CharacterDialogueOperation::Factory, alice.clone(), None),
        (CharacterDialogueOperation::Factory, bob.clone(), None),
        (
            CharacterDialogueOperation::Reconfigure,
            alice,
            Some("ja-JP"),
        ),
        (CharacterDialogueOperation::Reconfigure, bob, Some("en-US")),
    ];
    assert_eq!(
        native
            .iter()
            .map(|(operation, _)| *operation)
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|(operation, _, _)| *operation)
            .collect::<Vec<_>>(),
        "both branch factories return to the caller and reach reconfiguration"
    );

    let owner = RuntimeProgramOwner::Plan(Arc::new(compiled.plan.clone()));
    let schema = bind_character_dialogue_schema(&compiled, owner);
    let generation = compiled
        .character_dialogue_generation
        .as_ref()
        .expect("the fixture publishes accepted CharacterDialogue generation");
    for ((operation, value), (expected_operation, character, locale)) in native.iter().zip(expected)
    {
        assert_eq!(*operation, expected_operation);
        let RuntimeValue::Opaque(opaque) = value else {
            panic!("CharacterDialogue producer returns its opaque runtime value")
        };
        assert_eq!(
            opaque.producer(),
            &CharacterDialogueRuntimeSchema::opaque_type_producer()
        );
        assert_eq!(
            opaque.semantic_identity(),
            CharacterDialogueType::exact(character.clone()).runtime_semantic_identity()
        );
        let RuntimeValue::Tuple(payload) = opaque.payload() else {
            panic!("the opaque CharacterDialogue payload is its closed tuple")
        };
        assert_eq!(payload.len(), 18, "the admitted payload retains every slot");
        let RuntimeValue::EntityRef(RuntimeEntityReference::Project { family, public_id }) =
            &payload[0]
        else {
            panic!("payload slot zero retains the selected Character reference")
        };
        assert_eq!(*family, arcweft_id::DeclarationIdentityFamily::Character);
        assert_eq!(public_id.as_str(), character.as_str());
        assert_eq!(
            payload[1].builtin_variant_case().map(|(case, _)| case),
            Some(RuntimeBuiltinVariantCaseIdentity::OptionNone),
            "logical Character declarations do not invent a visual manifest"
        );

        let decoded = schema
            .try_decode_opaque(opaque)
            .expect("the exact compiler generation admits its produced opaque value");
        let dialogue = decoded.dialogue();
        assert_eq!(dialogue.character(), &character);
        let config = dialogue.config();
        let defaults = generation
            .characters()
            .get(&character)
            .expect("selected Character belongs to the accepted generation")
            .defaults()
            .config();
        assert_eq!(config.voice(), defaults.voice());
        assert_eq!(config.look(), defaults.look());
        assert_eq!(config.stage(), defaults.stage());
        assert_eq!(config.portrait(), defaults.portrait());
        assert_eq!(config.focus(), defaults.focus());
        assert_eq!(config.cleanup(), defaults.cleanup());
        assert_eq!(config.view(), defaults.view());
        assert_eq!(config.hooks(), defaults.hooks());
        assert_eq!(config.style(), defaults.style());
        assert_eq!(config.rich_text(), defaults.rich_text());
        assert_eq!(config.inline_failure(), defaults.inline_failure());
        assert_eq!(config.custom(), defaults.custom());
        assert_eq!(
            config
                .source_locale()
                .map(arcweft_dialogue::DialogueLocaleId::as_str),
            locale,
            "the caller's explicit reconfigure patch is retained"
        );

        let RuntimeValue::EntityRef(RuntimeEntityReference::Project {
            family: view_family,
            public_id: view_id,
        }) = &payload[11]
        else {
            panic!("payload slot eleven retains the accepted View default")
        };
        assert_eq!(*view_family, arcweft_id::DeclarationIdentityFamily::View);
        assert_eq!(view_id.as_str(), defaults.view().as_str());
        let (locale_case, locale_payload) = payload[12]
            .builtin_variant_case()
            .expect("payload slot twelve is the source-locale option");
        match locale {
            Some(locale) => {
                assert_eq!(locale_case, RuntimeBuiltinVariantCaseIdentity::OptionSome);
                assert!(
                    matches!(locale_payload, Some(RuntimeValue::String(value)) if value.as_str() == locale)
                );
            }
            None => {
                assert_eq!(locale_case, RuntimeBuiltinVariantCaseIdentity::OptionNone);
                assert!(locale_payload.is_none());
            }
        }
    }
}

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
    mutually_recursive_generics_execute_for_distinct_types,
    r#"
fn forward<Left>(value: Left, remaining: i64) -> Left {
    if remaining == 0i64 { value } else { backward(value, remaining - 1i64) }
}
fn backward<Right>(value: Right, remaining: i64) -> Right {
    if remaining == 0i64 { value } else { forward(value, remaining - 1i64) }
}
flow main() -> bool {
    let number = forward(42i64, 4i64)
    let text = backward("kept", 3i64)
    return number == 42i64 && text == "kept"
}
"#,
    RuntimeValue::Bool(true),
    "true"
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

const GENERIC_PREFIX_ORIGIN_BRANCH_SOURCE: &str = r#"
entry cli @entry.main { goto @flow.main }
fn left<A, B>(first: A)(second: B) -> i64 { 1i64 }
fn right<A, B>(first: A)(second: B) -> i64 { 42i64 }
fn identity<T>(value: T) -> T { value }
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
fn selected(pick: bool) -> i64 {
    let prefix = if pick { left("left") } else { right("right") }
    return apply(identity(prefix), 9i64)
}
flow main() -> bool {
    let left_result = selected(identity(true))
    let right_result = selected(identity(false))
    return left_result == 1i64 && right_result == 42i64
}
"#;

mod generic_prefix_origin_branch {
    use super::*;

    #[test]
    fn native() {
        assert_native_return(GENERIC_PREFIX_ORIGIN_BRANCH_SOURCE, "true");
    }

    #[test]
    fn awbc() {
        assert_awbc_return(
            GENERIC_PREFIX_ORIGIN_BRANCH_SOURCE,
            RuntimeValue::Bool(true),
        );
    }
}

#[test]
fn generic_prefix_origin_branch_has_deterministic_canonical_awbc_bytes() {
    fn lower_twice_source() -> Vec<u8> {
        let compiled = compile_source(GENERIC_PREFIX_ORIGIN_BRANCH_SOURCE)
            .expect("generic prefix origin branch source compiles");
        AwbcLowerer::new(
            &compiled.plan,
            &compiled.dialogue_content,
            "generic_prefix_origin_branch.arcw",
        )
        .lower()
        .expect("generic prefix origin branch lowers to verified AWBC")
        .program
        .encode_canonical()
        .expect("generic prefix origin branch encodes canonically")
    }

    assert_eq!(lower_twice_source(), lower_twice_source());
}

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
