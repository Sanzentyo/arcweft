use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_syntax::parser::parse_source;

#[test]
fn view_lowering_keeps_one_typed_view_owner() {
    let parsed =
        parse_source("pub character @character.alice\npub view Card() {\n    Panel()\n}\n");
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("View lowers");

    let views = hir.view_declarations().collect::<Vec<_>>();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].local_binding_name(), Some("Card"));
    assert!(views[0].view_body().is_some());
}
