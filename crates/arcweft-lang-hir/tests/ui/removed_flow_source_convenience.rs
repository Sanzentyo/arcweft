use arcweft_lang_hir::model::HirFlow;

fn removed(flow: &HirFlow) {
    let _ = flow.range();
}

fn main() {}
