#[test]
fn rejects_unknown_reflection_attributes() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/unknown_reflection_attribute.rs");
}
