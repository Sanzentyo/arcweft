use arcweft_lang_hir::module::HirModule;

fn append(module: &mut HirModule, other: HirModule) {
    module.append_module_body(other);
}

fn main() {}
