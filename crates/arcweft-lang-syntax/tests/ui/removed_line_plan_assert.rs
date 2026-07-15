use arcweft_lang_syntax::ast::line_plan::LinePlanItem;

fn reject_removed_variant(item: LinePlanItem) {
    if let LinePlanItem::Assert { .. } = item {}
}

fn main() {}
