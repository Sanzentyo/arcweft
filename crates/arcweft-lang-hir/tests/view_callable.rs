use arcweft_lang_hir::{lower::lower_to_hir, model::HirTopLevelDecl};
use arcweft_lang_syntax::{
    ast::items::{CallableKind, EntityDeclKind},
    parser::parse_source,
};

#[test]
fn view_lowering_keeps_typed_view_body_and_view_callable_owner() {
    let parsed = parse_source("pub view Card() {\n    Panel()\n}\n");
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_to_hir(parsed.typed_tree()).expect("View lowers");

    assert!(hir.declarations().iter().any(|declaration| {
        matches!(
            declaration,
            HirTopLevelDecl::Callable(item)
                if item.kind() == CallableKind::View && item.name() == "Card"
        )
    }));
    assert!(hir.declarations().iter().any(|declaration| {
        matches!(
            declaration,
            HirTopLevelDecl::EntityDecl(item)
                if item.kind() == EntityDeclKind::View && item.view_body().is_some()
        )
    }));
}
