use arcweft_lang_hir::syntax::expr::{Expr, Literal};

use crate::labels::{expr_label, literal_label};

pub(crate) fn style_call_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(path) => Some(path.as_str()),
        Expr::Field { field, .. } => Some(field.as_str()),
        _ => None,
    }
}

pub(crate) fn entity_ref_label(expr: &Expr) -> String {
    match expr {
        Expr::EntityRef(entity) => entity.body().to_owned(),
        _ => expr_style_value(expr).trim_start_matches('@').to_owned(),
    }
}

pub(crate) fn expr_style_value(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Literal::String(value)) | Expr::Path(value) => value.clone(),
        Expr::Literal(literal) => literal_label(literal),
        Expr::EntityRef(entity) => format!("@{}", entity.body()),
        _ => expr_label(expr),
    }
}
