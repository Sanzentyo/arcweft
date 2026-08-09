use arcweft_lang_hir::model::HirFlowItem;

fn removed(item: HirFlowItem) {
    match item {
        HirFlowItem::Stmt(_) => {}
        _ => {}
    }
}

fn main() {}
