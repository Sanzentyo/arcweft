use arcweft_lang_hir::item::HirFlowItem;

fn removed(flow: &HirFlowItem) {
    let _ = flow.body().tail();
}

fn main() {}
