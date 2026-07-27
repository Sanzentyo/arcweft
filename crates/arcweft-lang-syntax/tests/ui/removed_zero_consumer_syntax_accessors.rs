use arcweft_lang_syntax::{
    ast::{
        dialogue::DialogueTagKind,
        module_path::{CanonicalModulePath, ModulePathRoot},
        view::ViewBody,
    },
    expr::Expr,
};

fn removed_module_path_accessors(root: ModulePathRoot, path: &CanonicalModulePath) {
    let _ = root.is_crate_rooted();
    let _ = root.super_levels();
    let _ = path.ancestors_inclusive();
}

fn removed_typed_accessors(tag: DialogueTagKind, expr: &Expr, view: &ViewBody) {
    let _ = tag.is_point();
    let _ = expr.as_select();
    let _ = view.view_calls();
}

fn main() {}
