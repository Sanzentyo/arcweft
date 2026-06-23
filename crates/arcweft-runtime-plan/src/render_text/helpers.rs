use arcweft_lang_hir::syntax::expr::{Expr, Literal};

use crate::labels::{entity_ref_label as syntax_entity_ref_label, expr_label, literal_label};

pub(crate) fn style_call_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(path) => Some(path.as_str()),
        Expr::Field { field, .. } => Some(field.as_str()),
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
        Expr::Literal(Literal::String(value)) | Expr::Path(value) => value.clone(),
        Expr::Literal(literal) => literal_label(literal),
        Expr::EntityRef(entity) => format!("@{}", syntax_entity_ref_label(entity)),
        _ => expr_label(expr),
    }
}
