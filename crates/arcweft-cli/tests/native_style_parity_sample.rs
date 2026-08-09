use std::fs;
use std::path::Path;
use std::sync::Arc;

use arcweft_lang_syntax::{
    attachment::{AttachedStyleBody, AttachedStyleExpression, AttachedStyleMember, TypedItemNode},
    expressions::{ExpressionProjection, SyntaxCallProjection},
    incremental::SyntaxDatabase,
    parser::ParseOptions,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId};

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
    let mut syntax = SyntaxDatabase::try_new().expect("sample syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            document,
            ParseOptions::default(),
        )
        .expect("native-style-parity source attaches");
    let style = parsed
        .items()
        .expect("sample source-item inventory")
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Style(style) => {
                let style = style.semantics().expect("attached Style semantics");
                let is_sample_style = style
                    .id()
                    .reference()
                    .and_then(|reference| reference.value().ok())
                    .is_some_and(|reference| {
                        reference.segments().len() == 1
                            && reference.segments()[0].as_str() == "native_style_parity"
                    });
                is_sample_style.then_some(style)
            }
            _ => None,
        })
        .expect("native-style-parity style item");

    assert!(!style.has_recovery(), "sample Style remains fully attached");
    assert!(style.body().members().iter().any(|member| {
        let AttachedStyleMember::Token(token) = member else {
            return false;
        };
        token
            .name()
            .value()
            .is_ok_and(|name| name.as_str() == "color.accent")
            && matches!(
                token.value(),
                AttachedStyleExpression::Authored(value)
                    if matches!(
                        value.projection(),
                        ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(_))
                    ) && value.children().first().is_some_and(|callee| {
                        callee
                            .authored_semantic()
                            .ok()
                            .flatten()
                            .and_then(|callee| callee.path().cloned())
                            .is_some_and(|path| {
                                path.segments().len() == 1
                                    && path.segments()[0].source_text() == "rgba"
                            })
                    })
            )
    }));
    assert!(style_body_has_predicate(style.body(), "hover"));
    assert!(style_body_has_predicate(style.body(), "active"));
    assert!(style_body_has_predicate(style.body(), "focus-visible"));
    assert!(style_body_has_predicate(style.body(), "composing"));
}

fn style_body_has_predicate(body: &AttachedStyleBody, expected: &str) -> bool {
    body.members().iter().any(|member| match member {
        AttachedStyleMember::Rule(rule) => rule.selector().sequences().iter().any(|sequence| {
            sequence.predicates().iter().any(|predicate| {
                predicate
                    .name()
                    .value()
                    .is_ok_and(|name| name.as_str() == expected)
            })
        }),
        AttachedStyleMember::Environment(environment) => {
            style_body_has_predicate(environment.body(), expected)
        }
        AttachedStyleMember::Token(_) | AttachedStyleMember::Error { .. } => false,
    })
}
