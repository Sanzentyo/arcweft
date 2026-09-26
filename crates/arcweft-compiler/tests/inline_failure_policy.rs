use std::sync::Arc;

use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{codec::AwbcDecodeBudget, schema::AwbcProgram},
    task::RuntimeProgramOwner,
    value::RuntimeValue,
};
use arcweft_dialogue::{FallbackStylePolicy, InlineFailurePolicy, InlineFallback};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[path = "support/execution.rs"]
mod execution;

use execution::{
    bind_character_dialogue_schema, execute_decoded_awbc_character_dialogue_calls,
    execute_native_character_dialogue_calls,
};

#[test]
fn one_string_fallback_can_be_used_by_multiple_character_dialogues() {
    let compiled = compile_source(
        r#"
character alice { display = "Alice" }

flow main() -> Unit {
    let fallback: String = "unavailable"
    let policy = InlineFailure.fallback(fallback)
    let first = alice(inline_error=policy)
    let second = alice(inline_error=policy)
    let first_localized = first(source_locale="ja-JP")
    let second_localized = second(source_locale="en-US")
}

entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("the same String fallback can initialize more than one dialogue policy");

    let native = execute_native_character_dialogue_calls(&compiled);
    assert_eq!(
        native.len(),
        4,
        "both configured dialogues reach their producer"
    );

    let awbc = execute_decoded_awbc_character_dialogue_calls(&compiled);
    assert_eq!(
        native, awbc,
        "native and decoded AWBC preserve both policies"
    );

    let native_schema = bind_character_dialogue_schema(
        &compiled,
        RuntimeProgramOwner::Plan(Arc::new(compiled.plan.clone())),
    );
    let awbc_program = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "inline_failure_policy.arcw",
    )
    .lower()
    .expect("the policy values lower to verified AWBC")
    .program;
    let awbc_bytes = awbc_program
        .encode_canonical()
        .expect("the policy program encodes canonically");
    let decoded = AwbcProgram::decode_canonical(&awbc_bytes, AwbcDecodeBudget::default())
        .expect("the policy program decodes canonically");
    assert_eq!(decoded, awbc_program);
    let awbc_schema =
        bind_character_dialogue_schema(&compiled, RuntimeProgramOwner::Awbc(Arc::new(decoded)));

    for ((native_operation, native_value), (awbc_operation, awbc_value)) in native.iter().zip(&awbc)
    {
        assert_eq!(native_operation, awbc_operation);
        let RuntimeValue::Opaque(native_opaque) = native_value else {
            panic!("CharacterDialogue producer returns its opaque value");
        };
        let RuntimeValue::Opaque(awbc_opaque) = awbc_value else {
            panic!("CharacterDialogue producer returns its opaque value");
        };
        let native_dialogue = native_schema
            .try_decode_opaque(native_opaque)
            .expect("native policy belongs to its generation");
        let awbc_dialogue = awbc_schema
            .try_decode_opaque(awbc_opaque)
            .expect("AWBC policy belongs to its generation");
        let expected = InlineFailurePolicy::Fallback {
            fallback: InlineFallback::Text {
                text: "unavailable".to_owned(),
                style: FallbackStylePolicy::Plain,
            },
        };
        assert_eq!(
            native_dialogue.dialogue().config().inline_failure(),
            &expected
        );
        assert_eq!(
            awbc_dialogue.dialogue().config().inline_failure(),
            &expected
        );
    }
}
