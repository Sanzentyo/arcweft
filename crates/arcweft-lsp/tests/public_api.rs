#[test]
fn raw_text_document_analysis_entry_is_not_public() {
    trybuild::TestCases::new().compile_fail("tests/ui/document_analysis_raw_text.rs");
}
