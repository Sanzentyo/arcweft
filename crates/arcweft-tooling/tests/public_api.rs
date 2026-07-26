#[test]
fn detached_text_tooling_entrypoints_are_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/format_source_removed.rs");
    cases.compile_fail("tests/ui/source_code_actions_requires_document.rs");
}
