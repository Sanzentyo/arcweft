#[test]
fn raw_text_document_analysis_entry_is_not_public() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/document_analysis_raw_text.rs");
    cases.compile_fail("tests/ui/removed_zero_consumer_lsp_facades.rs");
}
