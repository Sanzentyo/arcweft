use arcweft_lang_syntax::{
    ast::items::{EntityDeclKind, Item},
    parser::parse_source,
};

#[test]
fn typed_view_declaration_is_the_only_view_callable_owner() {
    let parsed = parse_source("pub view Card() {\n    Panel()\n}\n");
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let view = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) if item.kind() == EntityDeclKind::View => Some(item),
            _ => None,
        })
        .expect("typed View declaration");

    assert_eq!(view.local_binding_name(), Some("Card"));
    assert_eq!(view.signature_tail(), "()");
    assert!(view.view_body().and_then(|body| body.view()).is_some());
}
