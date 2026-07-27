#[test]
fn removed_project_loader_apis_are_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/capability_policy_absent.rs");
    cases.compile_fail("tests/ui/removed_zero_consumer_project_facades.rs");
}
