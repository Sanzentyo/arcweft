#![cfg(feature = "derive")]

#[test]
fn derive_attribute_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/derive/pass/*.rs");
    cases.compile_fail("tests/derive/fail/*.rs");
}
