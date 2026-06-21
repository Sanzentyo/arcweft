use serde_json::Value;

#[must_use]
pub fn parse_json(stdout: &str) -> Value {
    serde_json::from_str(stdout)
        .unwrap_or_else(|error| panic!("stdout was not valid JSON: {error}\n{stdout}"))
}

pub fn assert_json_error_code(value: &Value, expected: &str) {
    assert_eq!(
        value.pointer("/error/code").and_then(Value::as_str),
        Some(expected),
        "unexpected JSON error envelope: {value}"
    );
}
