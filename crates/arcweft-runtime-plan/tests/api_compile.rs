#[test]
fn removed_runtime_plan_apis_are_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/removed_zero_consumer_runtime_plan_facades.rs");
}
