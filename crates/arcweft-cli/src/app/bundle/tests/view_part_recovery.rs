use super::*;
use arcweft_lang_syntax::{
    attachment::item::TypedItemNode, incremental::SyntaxDatabase, parser::ParseOptions,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId};
use std::sync::Arc;

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
    let mut syntax = SyntaxDatabase::try_new().expect("fixture syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .expect("attached recovered View source");

    assert!(
        !parsed.diagnostics().is_empty(),
        "non-current View input must use ordinary parser recovery"
    );
    let view = parsed
        .items()
        .expect("attached item projection")
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::View(view) => Some(view),
            _ => None,
        })
        .expect("recovered attached View");
    let view = view.semantics().expect("attached View semantics");
    assert_eq!(view.exports().count(), 0);
    assert!(view.has_recovery());

    assert!(
        collect_bundle_dsl_view_resources(&document).is_err(),
        "parser-recovered View syntax cannot enter an accepted runtime product"
    );
}
