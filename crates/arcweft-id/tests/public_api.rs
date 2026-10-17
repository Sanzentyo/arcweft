#[test]
fn dialogue_identity_raw_fields_and_serde_are_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/dialogue_identity_private.rs");
    cases.compile_fail("tests/ui/dialogue_text_key_private.rs");
    cases.compile_fail("tests/ui/dialogue_identity_not_serde.rs");
}

#[test]
fn presentation_occurrence_identity_cannot_be_forged() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/presentation_identity_private.rs");
}
