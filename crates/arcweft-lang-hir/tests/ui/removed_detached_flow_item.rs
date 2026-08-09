use arcweft_lang_syntax::ast::flow::FlowItem;

fn removed(item: FlowItem) {
    match item {
        FlowItem::Stmt(_) => {}
        _ => {}
    }
}

fn main() {}
