#[test]
fn permissive_typed_json_decoder_is_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/permissive_typed_json_decoder_removed.rs");
}
