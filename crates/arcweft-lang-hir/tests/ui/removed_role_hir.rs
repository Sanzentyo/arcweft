use arcweft_lang_hir::model::{HirAgent, HirModule, HirTopLevelDecl};

fn removed_access(module: &HirModule, declaration: &HirTopLevelDecl) {
    let _: Option<HirAgent> = None;
    let _ = module.agents();
    match declaration {
        HirTopLevelDecl::State(_) | HirTopLevelDecl::Agent(_) => {}
        _ => {}
    }
}

fn main() {}
