#[test]
fn old_environment_taking_compile_entry_point_is_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/compile_project_with_env_removed.rs");
}
