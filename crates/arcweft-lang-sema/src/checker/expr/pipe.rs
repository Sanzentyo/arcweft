use super::{CallArg, Expr, Placeholder, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind};
use crate::checker::PipeLeftBinding;

impl TypeChecker<'_> {
    pub(super) fn check_placeholder_expr(&mut self, placeholder: Placeholder) -> Option<TypeKind> {
        match placeholder {
            Placeholder::PipeLeft => self
                .pipe_left_stack
                .last()
                .map(|binding| binding.ty.clone())
                .or_else(|| {
                    self.errors.push(TypeCheckError::new(
                        "`^` can only appear inside the right-hand side of a pipe expression"
                            .to_owned(),
                    ));
                    None
                }),
            Placeholder::Partial => self
                .current_partial_placeholder_type()
                .or_else(|| self.reject_partial_placeholder_without_expected_type()),
        }
    }

    pub(super) fn check_pipe_expr(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        if self.check_lifetime_pipe(lhs, rhs).is_some() {
            return Some(TypeKind::Unit);
        }
        if rhs.contains_pipe_left() {
            return self.check_pipe_placeholder_rhs(lhs, rhs);
        }
        self.check_data_last_pipe(lhs, rhs, expression_id)
    }

    fn check_pipe_placeholder_rhs(&mut self, lhs: &Expr, rhs: &Expr) -> Option<TypeKind> {
        let previous_closure_effect_callable = self.last_checked_closure_effect_callable.take();
        let previous_curried_signature_call = self.last_checked_curried_signature_call.take();
        let lhs_source_range = self.source_range_for_expr(lhs);
        let lhs_ty = self
            .check_expr(lhs)
            .unwrap_or_else(|| TypeKind::Named("_".to_owned()));

        // The left value is produced before the RHS. Function/effect side-channel
        // evidence for the whole pipe, however, belongs to the RHS result.
        self.last_checked_closure_effect_callable = previous_closure_effect_callable;
        self.last_checked_curried_signature_call = previous_curried_signature_call;
        self.pipe_left_stack.push(PipeLeftBinding {
            ty: lhs_ty,
            source_range: lhs_source_range,
        });
        let result = self.check_expr(rhs);
        self.pipe_left_stack
            .pop()
            .expect("pipe-left type scope must stay balanced");
        result
    }

    fn check_data_last_pipe(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        let previous_closure_effect_callable = self.last_checked_closure_effect_callable.take();
        let previous_curried_signature_call = self.last_checked_curried_signature_call.take();
        let rhs_ty = self.check_expr(rhs);
        let rhs_effect_callable = self.last_checked_closure_effect_callable.take();
        let rhs_curried_signature_call = self.last_checked_curried_signature_call.take();
        self.last_checked_closure_effect_callable = previous_closure_effect_callable;
        self.last_checked_curried_signature_call = previous_curried_signature_call;

        let Some(rhs_ty @ TypeKind::Function { .. }) = rhs_ty else {
            // Keep authored diagnostics complete without visiting the pipe LHS
            // twice. Runtime lowering uses the same RHS-then-LHS construction
            // order while the resulting lexical let still evaluates LHS first.
            self.check_expr(lhs);
            if let Some(rhs_ty) = rhs_ty {
                self.errors.push(TypeCheckError::new(format!(
                    "pipe right-hand side must be a function value, found {}",
                    rhs_ty.source_label()
                )));
            }
            return None;
        };

        Some(self.check_known_function_value_call(
            expression_id,
            None,
            rhs_effect_callable,
            rhs_curried_signature_call.as_ref(),
            None,
            &[CallArg::Positional(lhs.clone())],
            rhs_ty,
        ))
    }
}
