use super::{Expr, TypeCheckError, TypeChecker, TypeKind};
use crate::checker::helpers::numeric_literal_suffix_type;
use arcweft_lang_syntax::expr::Literal;

impl TypeChecker<'_> {
    pub(super) fn check_range_expr(
        &mut self,
        start: Option<&Expr>,
        end: Option<&Expr>,
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        let item_type = range_expected_item(expected)
            .cloned()
            .or_else(|| start.and_then(|start| self.range_bound_type_hint(start)))
            .or_else(|| end.and_then(|end| self.range_bound_type_hint(end)))
            .unwrap_or(TypeKind::I32);
        if !(item_type.is_integer()
            || matches!(item_type, TypeKind::Named(ref name) if name == "_"))
        {
            self.errors.push(TypeCheckError::new(format!(
                "range endpoints must have an integer type, found {item_type:?}"
            )));
        }
        let start_type =
            start.and_then(|start| self.check_expr_with_expected(start, Some(&item_type)));
        self.check_range_bound_type("start", &item_type, start_type.as_ref());
        let end_type = end.and_then(|end| self.check_expr_with_expected(end, Some(&item_type)));
        self.check_range_bound_type("end", &item_type, end_type.as_ref());
        TypeKind::Range(Box::new(item_type))
    }

    fn range_bound_type_hint(&self, expr: &Expr) -> Option<TypeKind> {
        match expr {
            Expr::Path(path) => self.symbol_type(path).cloned(),
            Expr::Literal(Literal::Int {
                suffix: Some(suffix),
                ..
            }) => numeric_literal_suffix_type(Some(suffix.as_str())),
            _ => None,
        }
    }

    fn check_range_bound_type(
        &mut self,
        bound: &str,
        expected: &TypeKind,
        actual: Option<&TypeKind>,
    ) {
        let Some(actual) = actual else {
            return;
        };
        if !self.types_compatible(expected, actual) {
            self.errors.push(TypeCheckError::new(format!(
                "range {bound} bound must have type {expected:?}, found {actual:?}"
            )));
        }
    }
}

fn range_expected_item(expected: Option<&TypeKind>) -> Option<&TypeKind> {
    match expected {
        Some(TypeKind::Range(item)) => Some(item),
        _ => None,
    }
}
