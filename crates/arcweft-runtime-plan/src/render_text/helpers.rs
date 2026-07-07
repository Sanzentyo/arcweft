use arcweft_lang_hir::syntax::expr::{Expr, Literal};

use crate::labels::{entity_ref_label as syntax_entity_ref_label, expr_label, literal_label};

pub(crate) fn style_call_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(path) => Some(path.as_str()),
        Expr::ShortVariant(name) => Some(name.as_str()),
        Expr::Select(select) => Some(select.member().as_str()),
        _ => None,
    }
}

pub(crate) fn entity_ref_label(expr: &Expr) -> String {
    match expr {
        Expr::EntityRef(entity) => syntax_entity_ref_label(entity),
        _ => expr_style_value(expr).trim_start_matches('@').to_owned(),
    }
}

pub(crate) fn expr_style_value(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Literal::String(value)) => value.clone(),
        Expr::Path(value) => value.as_label().to_owned(),
        Expr::ShortVariant(value) => format!(".{value}"),
        Expr::Literal(literal) => literal_label(literal),
        Expr::EntityRef(entity) => format!("@{}", syntax_entity_ref_label(entity)),
        _ => expr_label(expr),
    }
}
