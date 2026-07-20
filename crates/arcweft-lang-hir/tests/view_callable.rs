use arcweft_lang_hir::{lower::lower_to_hir, model::HirTopLevelDecl};
use arcweft_lang_syntax::{ast::items::EntityDeclKind, parser::parse_source};

#[test]
fn view_lowering_keeps_one_typed_view_owner() {
    let parsed = parse_source("pub view Card() {\n    Panel()\n}\n");
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_to_hir(parsed.typed_tree()).expect("View lowers");

    let views = hir
        .declarations()
        .iter()
        .filter_map(|declaration| match declaration {
            HirTopLevelDecl::EntityDecl(item) if item.kind() == EntityDeclKind::View => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].local_binding_name(), Some("Card"));
    assert!(views[0].view_body().is_some());
}
