use std::fs;
use std::path::Path;
use std::sync::Arc;

use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_lang_syntax::{
    ast::{items::Item, style::StyleBodyItem},
    expr::Expr,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

#[test]
fn native_style_parity_sample_authors_observable_and_view_styles_in_dsl() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(root.join("samples/native-style-parity/src/main.arcw"))
        .expect("native Style parity sample source");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(
                "arcweft-project://samples/native-style-parity/src/main.arcw",
            )
            .expect("sample document ID"),
            SourceName::path("samples/native-style-parity/src/main.arcw"),
            source.as_str(),
        )
        .expect("sample source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());

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
                Expr::Call(call)
                    if call.callee().dotted_selector_label().as_deref() == Some("rgba")
            )
    }));
    assert!(style_body_has_predicate(sheet.body(), "hover"));
    assert!(style_body_has_predicate(sheet.body(), "active"));
    assert!(style_body_has_predicate(sheet.body(), "focus-visible"));
    assert!(style_body_has_predicate(sheet.body(), "composing"));
}

fn style_body_has_predicate(body: &[StyleBodyItem], expected: &str) -> bool {
    body.iter().any(|item| match item {
        StyleBodyItem::Rule(rule) => style_rule_has_predicate(rule, expected),
        StyleBodyItem::Environment(environment) => {
            style_body_has_predicate(environment.body(), expected)
        }
    })
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
