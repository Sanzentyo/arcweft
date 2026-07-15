use arcweft_lang_hir::model::{HirBorrow, HirFlowItem};

fn reject_removed_variant(item: HirFlowItem) {
    if let HirFlowItem::Borrow(_borrow) = item {}
}

fn main() {
    let _: Option<HirBorrow> = None;
}
