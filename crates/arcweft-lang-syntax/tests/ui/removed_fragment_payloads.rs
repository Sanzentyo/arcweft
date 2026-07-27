use arcweft_lang_syntax::{
    ast::items::Item,
    expr::Expr,
    parser::ParsedFragmentKind,
};

fn retain_expression(expr: Box<Expr>) -> ParsedFragmentKind {
    ParsedFragmentKind::Expression(expr)
}

fn retain_items(items: Vec<Item>) -> ParsedFragmentKind {
    ParsedFragmentKind::Items(items)
}

fn main() {}
