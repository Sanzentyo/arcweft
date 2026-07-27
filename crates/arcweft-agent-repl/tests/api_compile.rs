#[test]
fn internal_repl_source_module_is_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/source_module_private.rs");
}
