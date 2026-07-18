#[test]
fn resolved_profiles_have_no_capability_policy_accessor() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/capability_policy_absent.rs");
}
