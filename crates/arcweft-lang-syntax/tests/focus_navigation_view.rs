#[test]
fn parser_accepts_focus_navigation_view_syntax() {
    let source = r#"
pub view SettingsPanel() {
  Column(nav: .vertical, group: @group:.settings, wrap: false, initial: @button:.apply, trap: .modal) {
    Button("Back", id: @button:.back)
      .nav(right: @button:.apply, down: auto)

    Button("Apply", id: @button:.apply)
      .nav(left: @button:.back, down: none, next: boundary)
  }
}
"#;
    let parsed = parse_focus_navigation_fixture(source);
    assert!(parsed.errors().is_empty(), "{:#?}", parsed.errors());
}
fn parse_focus_navigation_fixture(
    source: impl Into<String>,
) -> arcweft_lang_syntax::source::ParsedSource {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new(
                "arcweft-test://syntax/focus-navigation-view",
            )
            .expect("fixed test document ID is valid"),
            arcweft_source::SourceName::path("focus-navigation-view.arcw"),
            source.into(),
        )
        .expect("test source document"),
    );
    arcweft_lang_syntax::parser::parse_document_with_source(
        document,
        arcweft_lang_syntax::parser::ParseOptions::default(),
    )
}
