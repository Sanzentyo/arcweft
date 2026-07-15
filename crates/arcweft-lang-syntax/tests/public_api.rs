#[test]
fn removed_and_session_identity_public_api_contract() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/removed_borrow_block.rs");
    cases.compile_fail("tests/ui/removed_line_plan_assert.rs");
    cases.compile_fail("tests/ui/session_identity_raw_constructor.rs");
    cases.compile_fail("tests/ui/session_identity_serde.rs");
}
