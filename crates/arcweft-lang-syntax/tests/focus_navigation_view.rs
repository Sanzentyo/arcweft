use arcweft_lang_syntax::parser::parse_source;

#[test]
fn parser_accepts_focus_navigation_view_syntax() {
    let source = r#"
pub component SettingsPanel() {
  Column(nav: .vertical, group: @group:.settings, wrap: false, initial: @button:.apply, trap: .modal) {
    Button("Back", id: @button:.back)
      .nav(right: @button:.apply, down: auto)

    Button("Apply", id: @button:.apply)
      .nav(left: @button:.back, down: none, next: boundary)
  }
}
"#;
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:#?}", parsed.errors());
}
