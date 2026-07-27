use arcweft_lang_hir::{model::HirModule, project::HirProjectModule};

fn removed(module: HirModule, project_module: HirProjectModule) {
    let _ = module.source_len();
    let _ = module.top_level_ranges();
    let _ = project_module.into_parts();
}

fn main() {}
