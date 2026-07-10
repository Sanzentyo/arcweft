use super::{ExprNodeKey, TypeChecker};
use crate::types::TypeKind;
use arcweft_lang_syntax::{
    ast::common::TextRange,
    expr::{Expr, Placeholder, collect_expr_source_ranges},
};

impl TypeChecker<'_> {
    pub(super) fn check_expr_with_expected_at_range(
        &mut self,
        expr: &Expr,
        expected: Option<&TypeKind>,
        source_range: TextRange,
    ) -> Option<TypeKind> {
        let key = ExprNodeKey::from_expr(expr);
        let previous = self.expression_source_ranges.insert(key, source_range);
        let ty = self.check_expr_with_expected(expr, expected);
        if let Some(previous) = previous {
            self.expression_source_ranges.insert(key, previous);
        } else {
            self.expression_source_ranges.remove(&key);
        }
        ty
    }

    pub(super) fn register_expr_source_ranges(
        &mut self,
        expr: &Expr,
        expr_source: Option<&str>,
        expr_range: Option<TextRange>,
    ) {
        let (Some(expr_source), Some(expr_range)) = (expr_source, expr_range) else {
            return;
        };
        for source_range in collect_expr_source_ranges(expr, expr_source, expr_range) {
            self.expression_source_ranges.insert(
                ExprNodeKey::from_expr(source_range.expr()),
                source_range.range(),
            );
        }
    }

    pub(super) fn source_range_for_expr(&self, expr: &Expr) -> Option<TextRange> {
        if matches!(expr, Expr::Placeholder(Placeholder::PipeLeft))
            && let Some(binding) = self.pipe_left_stack.last()
        {
            return binding.source_range;
        }
        self.expression_source_ranges
            .get(&ExprNodeKey::from_expr(expr))
            .copied()
    }
}
