#[test]
fn rejects_unsupported_abi_shapes() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
