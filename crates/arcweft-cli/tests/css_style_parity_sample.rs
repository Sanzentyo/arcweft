use std::fs;
use std::path::Path;

use arcweft_lang_syntax::ast::items::{Item, ViewStyleSelectorPartDecl, ViewStyleValueDecl};
use arcweft_lang_syntax::parser::parse_source;

#[test]
fn css_style_parity_sample_authors_observable_and_view_styles_in_dsl() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(root.join("samples/css-style-parity/main.arcw"))
        .expect("css style parity sample source");
    let parsed = parse_source(source.clone());

    assert!(
        parsed.errors().is_empty(),
        "css-style-parity should parse cleanly: {:?}",
        parsed.errors()
    );
    assert!(source.contains("[style .opacity 0.86]"));
    assert!(source.contains("[transform .offset x=6px y=-1px]"));
    assert!(source.contains("[effect .wave amp=2px dir=0,1 period=8 speed=1]"));

    let style = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Style(style) if style.id().body() == "style.css_style_parity" => Some(style),
            _ => None,
        })
        .expect("css-style-parity style item");

    assert!(
        style
            .tokens()
            .iter()
            .any(|token| token.public_id() == "color.accent"
                && matches!(token.value(), ViewStyleValueDecl::Rgba { .. }))
    );
    assert!(
        style
            .rules()
            .iter()
            .any(|rule| rule.selector().iter().any(|part| {
                matches!(part, ViewStyleSelectorPartDecl::Interaction(value) if value == "hover")
            }))
    );
    assert!(
        style
            .rules()
            .iter()
            .any(|rule| rule.selector().iter().any(|part| {
                matches!(part, ViewStyleSelectorPartDecl::Interaction(value) if value == "active")
            }))
    );
    assert!(style.rules().iter().any(|rule| rule.selector().iter().any(
        |part| matches!(part, ViewStyleSelectorPartDecl::State(value) if value == "focus_visible")
    )));
    assert!(style.rules().iter().any(|rule| rule.selector().iter().any(
        |part| matches!(part, ViewStyleSelectorPartDecl::State(value) if value == "composing")
    )));
}
