use arcweft_lang_syntax::{
    ast::items::{CallableItem, CallableKind, EntityDeclKind, Item},
    parser::parse_source,
};

#[test]
fn typed_view_declaration_preserves_the_view_callable_projection() {
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

    let callable = CallableItem::from_view_declaration(view).expect("View callable projection");
    assert_eq!(callable.kind(), CallableKind::View);
    assert_eq!(callable.name(), "Card");
    assert_eq!(callable.signature_tail(), "()");
    assert!(callable.body().is_empty());
}
