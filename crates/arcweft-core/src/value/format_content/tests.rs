use super::*;
use crate::pattern::RuntimeCheckedType;
use crate::value::{RuntimeSignedIntWidth, RuntimeUnsignedIntWidth};

fn text(
    context: &RuntimeFormatContext,
    primary_type: RuntimeCheckedType,
    value: RuntimeValue,
    style: Option<&str>,
    locale: Option<&str>,
    currency: Option<&str>,
) -> RuntimeDialogueFormattedOutcome {
    let mut operands = vec![(RuntimeFmtParameterId::Value, Some(value))];
    for (parameter, value) in [
        (RuntimeFmtParameterId::Style, style),
        (RuntimeFmtParameterId::Locale, locale),
        (RuntimeFmtParameterId::Currency, currency),
    ] {
        if let Some(value) = value {
            operands.push((parameter, Some(RuntimeValue::String(value.to_owned()))));
        }
    }
    finish_format_content_attempt(
        context,
        RuntimeFormatPrimaryKind::Scalar(primary_type.semantic_identity_digest()),
        &operands,
        None,
    )
    .unwrap()
    .outcome()
    .clone()
}

fn success_text(outcome: RuntimeDialogueFormattedOutcome) -> String {
    match outcome {
        RuntimeDialogueFormattedOutcome::Success {
            value: RuntimeDialogueFormattedSuccess::Text(text),
            ..
        } => text,
        other => panic!("expected formatted text, got {other:?}"),
    }
}

#[test]
fn invalid_dynamic_locale_retains_plain_evaluated_value() {
    let formatted = finish_format_content_attempt(
        &RuntimeFormatContext::default(),
        RuntimeFormatPrimaryKind::Scalar(
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest(),
        ),
        &[
            (RuntimeFmtParameterId::Value, Some(RuntimeValue::i64(42))),
            (
                RuntimeFmtParameterId::Style,
                Some(RuntimeValue::String("number".to_owned())),
            ),
            (
                RuntimeFmtParameterId::Locale,
                Some(RuntimeValue::String("invalid_locale".to_owned())),
            ),
        ],
        None,
    )
    .expect("formatter failure remains a value");
    assert!(matches!(formatted.outcome(),
            RuntimeDialogueFormattedOutcome::Failure { reason, value_plain }
                if reason.starts_with("invalid fmt locale") && value_plain.as_deref() == Some("42")));
}

#[test]
fn recoverable_option_failure_retains_plain_evaluated_primary() {
    let primary_kind = RuntimeFormatPrimaryKind::Scalar(
        RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest(),
    );
    let formatted = finish_format_content_attempt(
        &RuntimeFormatContext::default(),
        primary_kind,
        &[
            (RuntimeFmtParameterId::Value, Some(RuntimeValue::i64(42))),
            (RuntimeFmtParameterId::Style, None),
        ],
        Some("style evaluation failed"),
    )
    .expect("recoverable formatter failure remains a value");
    assert_eq!(
        formatted.outcome(),
        &RuntimeDialogueFormattedOutcome::Failure {
            reason: "style evaluation failed".to_owned(),
            value_plain: Some("42".to_owned()),
        }
    );

    let missing_primary = finish_format_content_attempt(
        &RuntimeFormatContext::default(),
        primary_kind,
        &[(RuntimeFmtParameterId::Value, None)],
        Some("primary evaluation failed"),
    )
    .expect("failed primary remains a formatted failure");
    assert_eq!(
        missing_primary.outcome(),
        &RuntimeDialogueFormattedOutcome::Failure {
            reason: "primary evaluation failed".to_owned(),
            value_plain: None,
        }
    );
}

#[test]
fn number_uses_explicit_or_ambient_locale_and_exact_integer_digits() {
    let japanese = RuntimeFormatContext::default();
    let en_us = RuntimeFormatContext::new(LocaleTag::try_new("en-US").unwrap());
    assert_eq!(
        success_text(text(
            &japanese,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64),
            RuntimeValue::i64(12345),
            Some("number"),
            None,
            None,
        )),
        "12,345"
    );
    assert_eq!(
        success_text(text(
            &en_us,
            RuntimeCheckedType::F64,
            RuntimeValue::F64(12345.67),
            Some("number"),
            Some("de-de"),
            None,
        )),
        "12.345,67"
    );
    assert_eq!(
        success_text(text(
            &en_us,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U128),
            RuntimeValue::u128(u128::MAX),
            Some("number"),
            None,
            None,
        )),
        "340,282,366,920,938,463,463,374,607,431,768,211,455"
    );
    assert_eq!(
        success_text(text(
            &en_us,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I128),
            RuntimeValue::i128(i128::MIN),
            Some("number"),
            None,
            None,
        )),
        "-170,141,183,460,469,231,731,687,303,715,884,105,728"
    );
}

#[test]
fn floating_precision_and_negative_zero_follow_value_width() {
    let en_us = RuntimeFormatContext::new(LocaleTag::try_new("en-US").unwrap());
    for (primary_type, value) in [
        (RuntimeCheckedType::F32, RuntimeValue::F32(0.1)),
        (RuntimeCheckedType::F64, RuntimeValue::F64(0.1)),
    ] {
        assert_eq!(
            success_text(text(
                &en_us,
                primary_type,
                value,
                Some("number"),
                None,
                None
            )),
            "0.1"
        );
    }
    for (primary_type, value) in [
        (RuntimeCheckedType::F32, RuntimeValue::F32(-0.0)),
        (RuntimeCheckedType::F64, RuntimeValue::F64(-0.0)),
    ] {
        assert_eq!(
            success_text(text(
                &en_us,
                primary_type,
                value,
                Some("number"),
                None,
                None
            )),
            "-0"
        );
    }
    assert!(matches!(
        text(
            &en_us,
            RuntimeCheckedType::F64,
            RuntimeValue::F64(f64::INFINITY),
            Some("number"),
            None,
            None,
        ),
        RuntimeDialogueFormattedOutcome::Failure {
            value_plain: None,
            ..
        }
    ));
}

#[test]
fn currency_implies_numeric_format_and_applies_currency_fraction_rules() {
    let en_us = RuntimeFormatContext::new(LocaleTag::try_new("en-US").unwrap());
    assert_eq!(
        success_text(text(
            &en_us,
            RuntimeCheckedType::F64,
            RuntimeValue::F64(12345.67),
            None,
            None,
            Some("USD"),
        )),
        "$12,345.67"
    );
    assert_eq!(
        success_text(text(
            &en_us,
            RuntimeCheckedType::F64,
            RuntimeValue::F64(-0.0),
            None,
            None,
            Some("USD"),
        )),
        "-$0.00"
    );
    assert_eq!(
        success_text(text(
            &en_us,
            RuntimeCheckedType::F64,
            RuntimeValue::F64(12345.67),
            None,
            None,
            Some("JPY"),
        )),
        "¥12,346"
    );
}

#[test]
fn unsupported_styles_and_currency_codes_preserve_plain_value() {
    let en_us = RuntimeFormatContext::new(LocaleTag::try_new("en-US").unwrap());
    for (style, currency) in [(Some("percent"), None), (None, Some("US"))] {
        assert!(matches!(
            text(
                &en_us,
                RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64),
                RuntimeValue::i64(42),
                style,
                None,
                currency,
            ),
            RuntimeDialogueFormattedOutcome::Failure { value_plain: Some(value), .. }
                if value == "42"
        ));
    }
}

#[test]
fn option_numeric_and_unstyled_scalar_keep_their_distinct_paths() {
    let en_us = RuntimeFormatContext::new(LocaleTag::try_new("en-US").unwrap());
    let int_type =
        RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest();
    let some = finish_format_content_attempt(
        &en_us,
        RuntimeFormatPrimaryKind::OptionScalar(int_type),
        &[
            (
                RuntimeFmtParameterId::Value,
                Some(RuntimeValue::option_some(RuntimeValue::i64(12345))),
            ),
            (
                RuntimeFmtParameterId::Style,
                Some(RuntimeValue::String("number".to_owned())),
            ),
        ],
        None,
    )
    .unwrap();
    assert_eq!(success_text(some.outcome().clone()), "12,345");
    let none = finish_format_content_attempt(
        &en_us,
        RuntimeFormatPrimaryKind::OptionScalar(int_type),
        &[
            (
                RuntimeFmtParameterId::Value,
                Some(RuntimeValue::option_none()),
            ),
            (
                RuntimeFmtParameterId::Style,
                Some(RuntimeValue::String("number".to_owned())),
            ),
            (
                RuntimeFmtParameterId::NoneValue,
                Some(RuntimeValue::String("--".to_owned())),
            ),
        ],
        None,
    )
    .unwrap();
    assert_eq!(success_text(none.outcome().clone()), "--");
    assert_eq!(
        success_text(text(
            &en_us,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64),
            RuntimeValue::i64(12345),
            None,
            None,
            None,
        )),
        "12345"
    );
    assert!(matches!(
        text(
            &en_us,
            RuntimeCheckedType::String,
            RuntimeValue::String("label".to_owned()),
            Some("number"),
            None,
            None,
        ),
        RuntimeDialogueFormattedOutcome::Failure { value_plain: Some(value), .. }
            if value == "label"
    ));
}

#[test]
fn numeric_none_text_obeys_the_shared_string_limit() {
    let maximum = crate::entry::RuntimeSchemaLimits::engine_default().max_string_bytes;
    let too_long = "x".repeat(usize::try_from(maximum).unwrap() + 1);
    let error = finish_format_content_attempt(
        &RuntimeFormatContext::default(),
        RuntimeFormatPrimaryKind::OptionScalar(
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest(),
        ),
        &[
            (
                RuntimeFmtParameterId::Value,
                Some(RuntimeValue::option_none()),
            ),
            (
                RuntimeFmtParameterId::Style,
                Some(RuntimeValue::String("number".to_owned())),
            ),
            (
                RuntimeFmtParameterId::NoneValue,
                Some(RuntimeValue::String(too_long)),
            ),
        ],
        None,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        RuntimeFormatAttemptError::Inline(RuntimeInlineTextValueError::StringLimit { .. })
    ));
}

#[test]
fn unstyled_content_passes_through_and_numeric_style_rejects_it() {
    let template = crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap();
    let digest = crate::entry::RuntimeDialogueContentTemplateDigest::from_bytes([0x22; 32]);
    let reference = crate::value::RuntimeDialoguePlainTextContextTemplateRef::from_encoded_identity(
        template, digest,
    );
    let proof = crate::value::RuntimeDialoguePlainTextContextTemplateProof::try_from_validated_ref(
        reference, digest,
    )
    .unwrap();
    let content = RuntimeDialogueContentValue::try_new_plain_text(
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x4a; 32]).unwrap(),
        proof,
        "hello",
    )
    .unwrap();
    let value = content.clone().into_runtime_value();
    let plain = finish_format_content_attempt(
        &RuntimeFormatContext::default(),
        RuntimeFormatPrimaryKind::Content,
        &[(RuntimeFmtParameterId::Value, Some(value.clone()))],
        None,
    )
    .unwrap();
    assert!(
        matches!(plain.outcome(), RuntimeDialogueFormattedOutcome::Success {
            value: RuntimeDialogueFormattedSuccess::Content(inner), ..
        } if **inner == content)
    );
    let styled = finish_format_content_attempt(
        &RuntimeFormatContext::default(),
        RuntimeFormatPrimaryKind::Content,
        &[
            (RuntimeFmtParameterId::Value, Some(value)),
            (
                RuntimeFmtParameterId::Style,
                Some(RuntimeValue::String("number".to_owned())),
            ),
        ],
        None,
    )
    .unwrap();
    assert!(matches!(
        styled.outcome(),
        RuntimeDialogueFormattedOutcome::Failure {
            value_plain: None,
            ..
        }
    ));
}
