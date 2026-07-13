use std::fs;
use std::path::Path;

use arcweft_lang_syntax::parser::parse_source;
use arcweft_lang_syntax::{ast::items::Item, expr::Expr};

#[test]
fn native_style_parity_sample_authors_observable_and_view_styles_in_dsl() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(root.join("samples/native-style-parity/main.arcw"))
        .expect("native Style parity sample source");
    let parsed = parse_source(source.clone());

    assert!(
        parsed.errors().is_empty(),
        "native-style-parity should parse cleanly: {:?}",
        parsed.errors()
    );
    let style = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Style(style) if style.id().body() == "style.native_style_parity" => Some(style),
            _ => None,
        })
        .expect("native-style-parity style item");

    let sheet = style.sheet();
    assert!(sheet.tokens().iter().any(|token| {
        token.public_id() == "color.accent"
            && matches!(
                token.value().expr(),
                Expr::Call { callee, .. }
                    if callee.dotted_selector_label().as_deref() == Some("rgba")
            )
    }));
    assert!(
        sheet
            .rules()
            .iter()
            .any(|rule| style_rule_has_predicate(rule, "hover"))
    );
    assert!(
        sheet
            .rules()
            .iter()
            .any(|rule| style_rule_has_predicate(rule, "active"))
    );
    assert!(
        sheet
            .rules()
            .iter()
            .any(|rule| style_rule_has_predicate(rule, "focus-visible"))
    );
    assert!(
        sheet
            .rules()
            .iter()
            .any(|rule| style_rule_has_predicate(rule, "composing"))
    );
}

fn style_rule_has_predicate(
    rule: &arcweft_lang_syntax::ast::style::StyleRuleDecl,
    expected: &str,
) -> bool {
    rule.selector().sequences().iter().any(|sequence| {
        sequence
            .predicates()
            .iter()
            .any(|predicate| predicate.name() == expected)
    })
}
