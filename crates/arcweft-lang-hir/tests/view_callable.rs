use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

#[test]
fn view_lowering_keeps_one_typed_view_owner() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://lang-hir/view/callable.arcw")
                .expect("View callable fixture source ID"),
            SourceName::path("lang-hir/view/callable.arcw"),
            "pub character alice { display_name = \"Alice\" }\npub view Card() {\n    Panel()\n}\n",
        )
        .expect("View callable fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("View lowers");

    let views = hir.view_declarations().collect::<Vec<_>>();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].local_binding_name(), Some("Card"));
    assert!(views[0].view_body().is_some());
}
