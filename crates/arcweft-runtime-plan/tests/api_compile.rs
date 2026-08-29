#[test]
fn removed_runtime_plan_apis_are_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/removed_zero_consumer_runtime_plan_facades.rs");
    cases.compile_fail("tests/ui/runtime_fault_has_no_public_constructor.rs");
    cases.compile_fail("tests/ui/runtime_session_identity_is_not_serde.rs");
}

#[test]
fn runtime_trigger_admission_has_no_external_constructor() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/runtime_trigger_admission_has_no_public_constructor.rs");
}

#[test]
fn removed_runtime_mark_handler_authority_is_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/removed_runtime_mark_handler_authority.rs");
}
