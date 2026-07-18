use super::{ParseErrorKind, TextRange, parse_native_sheet};

#[test]
fn environment_delimiter_eof_branches_retain_exact_typed_ranges() {
    let base = 11;
    let unclosed_body = "when environment(color-scheme == dark) {";
    let mut body_errors = Vec::new();
    parse_native_sheet(
        unclosed_body,
        TextRange::new(base, base + unclosed_body.len()),
        &mut body_errors,
    );
    let [body_error] = body_errors.as_slice() else {
        panic!("expected one unclosed environment body diagnostic, got {body_errors:?}");
    };
    assert_eq!(
        body_error.kind(),
        ParseErrorKind::StyleEnvironmentExpectedOpenBrace
    );
    assert_eq!(
        body_error.range(),
        &TextRange::new(base + unclosed_body.len() - 1, base + unclosed_body.len())
    );
    assert_eq!(body_error.message(), "unclosed environment style body");

    let unterminated_condition = "when environment(color-scheme == dark";
    let mut condition_errors = Vec::new();
    parse_native_sheet(
        unterminated_condition,
        TextRange::new(base, base + unterminated_condition.len()),
        &mut condition_errors,
    );
    let [condition_error] = condition_errors.as_slice() else {
        panic!(
            "expected one unterminated environment condition diagnostic, got {condition_errors:?}"
        );
    };
    assert_eq!(
        condition_error.kind(),
        ParseErrorKind::StyleEnvironmentUnterminatedCondition
    );
    assert_eq!(
        condition_error.range(),
        &TextRange::new(
            base + unterminated_condition.find('(').expect("condition open"),
            base + unterminated_condition.len(),
        )
    );
    assert_eq!(
        condition_error.message(),
        "unterminated environment condition"
    );
}
