#[test]
fn runtime_assertion_identity_boundaries_are_compile_time_closed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/core_cannot_name_hir_ids.rs");
    cases.compile_fail("tests/ui/prove_is_not_runtime_assertion_mode.rs");
}
