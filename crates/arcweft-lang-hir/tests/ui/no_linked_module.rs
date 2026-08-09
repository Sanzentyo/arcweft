use arcweft_lang_hir::project::HirProject;

fn flatten(project: &HirProject) {
    let _ = project.linked_module();
}

fn main() {}
