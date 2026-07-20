#[test]
fn dialogue_identity_raw_fields_and_serde_are_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/dialogue_identity_private.rs");
    cases.compile_fail("tests/ui/dialogue_text_key_private.rs");
    cases.compile_fail("tests/ui/dialogue_identity_not_serde.rs");
}
