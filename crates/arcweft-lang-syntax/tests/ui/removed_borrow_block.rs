use arcweft_lang_syntax::ast::flow::{BorrowBlock, FlowItem};

fn reject_removed_variant(item: FlowItem) {
    if let FlowItem::BorrowBlock(_borrow) = item {}
}

fn main() {
    let _: Option<BorrowBlock> = None;
}
