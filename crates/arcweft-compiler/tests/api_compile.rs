#[test]
fn removed_compiler_apis_are_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/compile_project_with_env_removed.rs");
    cases.compile_fail("tests/ui/agent_effects_module_removed.rs");
    cases.compile_fail("tests/ui/removed_zero_consumer_compiler_facades.rs");
    cases.compile_fail("tests/ui/lower_source_tree_removed.rs");
    cases.compile_fail("tests/ui/lower_source_document_removed.rs");
    cases.compile_fail("tests/ui/parse_source_text_removed.rs");
    cases.compile_fail("tests/ui/runtime_capability_policy_absent.rs");
}
