use super::*;
use arcweft_lang_sema::view_part::check_view_parts;
use arcweft_lang_syntax::ast::{
    items::Item,
    view::{ViewBody, ViewExpr},
};
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

fn first_view_body(parsed: &arcweft_lang_syntax::source::ParsedSource) -> &ViewBody {
    parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("recovered View body")
}

#[test]
fn ordinary_view_recovery_cannot_create_exported_part_facts() {
    let source = r#"
view Card() {
  UnknownContainer {
    Text("Title").part(title)
  }
}

flow test {
  view(@view.Card)
}
"#;
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("recovered-view.arcw").expect("source identity"),
            SourceName::path("recovered-view.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());

    assert!(
        !parsed.errors().is_empty(),
        "non-current View input must use ordinary parser recovery"
    );
    let body = first_view_body(&parsed);
    assert!(body.exports().is_empty());
    assert!(matches!(body.value(), ViewExpr::Raw(_)));

    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("ordinary recovery lowers without an exported-part node");
    assert!(hir.view_parts().is_empty());

    let (checked, _) = check_view_parts(&hir);
    assert!(
        checked
            .owners()
            .iter()
            .all(|owner| owner.exports().is_empty())
    );

    assert!(
        collect_bundle_dsl_view_resources(&hir).is_err(),
        "parser-recovered View syntax cannot enter an accepted runtime product"
    );
}
