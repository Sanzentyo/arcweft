//! Suspension boundaries and runtime-scope helpers.

use super::helpers::let_else_bindings;
use super::{
    Expr, LifetimeScopeKind, LoopContext, TypeCheckError, TypeChecker, TypeCheckerScopeSnapshot,
    TypeKind, YieldContext, await_branch_pattern_type, type_contains_borrow_ref,
    unify_loop_break_types,
};

impl TypeChecker<'_> {
    pub(super) fn check_yield_stmt(&mut self, expr: &Expr) {
        self.reject_active_borrows("yield suspension boundary");
        let actual = self.check_expr(expr);
        let Some(context) = self.yield_stack.last_mut() else {
            self.errors.push(TypeCheckError::new(
                "`yield` is only valid in `seq`, `stream`, or `source` contexts".to_owned(),
            ));
            return;
        };
        match context {
            YieldContext::Seq {
                item_ty,
                yield_count,
            } => {
                *yield_count += 1;
                if let Some(actual) = actual {
                    match item_ty {
                        Some(expected) if expected != &actual => {
                            self.errors.push(TypeCheckError::new(format!(
                                "yielded item types do not match, found {expected:?} and {actual:?}"
                            )));
                        }
                        Some(_) => {}
                        None => *item_ty = Some(actual),
                    }
                }
            }
            YieldContext::Stream {
                item_ty,
                yield_count,
                ..
            }
            | YieldContext::Source {
                item_ty,
                yield_count,
                ..
            } => {
                *yield_count += 1;
                if let Some(actual) = actual
                    && &actual != item_ty
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "yielded item must have type {item_ty:?}, found {actual:?}"
                    )));
                }
            }
        }
    }

    pub(super) fn check_await_expr(&mut self, expr: &Expr, applies_try: bool) -> Option<TypeKind> {
        self.reject_active_borrows("await suspension boundary");
        match self.check_expr(expr) {
            Some(TypeKind::Need { ready, .. }) if applies_try => Some(*ready),
            Some(TypeKind::Need { ready, error }) => Some(TypeKind::Result { ok: ready, error }),
            Some(other) => {
                self.errors.push(TypeCheckError::new(format!(
                    "await expression must have Need<T, E> type, found {other:?}"
                )));
                None
            }
            None => None,
        }
    }

    pub(super) fn check_await_item(
        &mut self,
        await_with: &arcweft_lang_hir::model::HirAwait,
    ) -> Option<TypeKind> {
        self.reject_active_borrows("await suspension boundary");
        let ty = self.check_expr(await_with.expr());
        let Some(TypeKind::Need { ready, error }) = ty else {
            self.errors.push(TypeCheckError::new(
                "await expression must have Need<T, E> type".to_owned(),
            ));
            return None;
        };
        if await_with.branches().is_empty() {
            self.errors.push(TypeCheckError::new(
                "await with must define at least one wait-view branch".to_owned(),
            ));
        }
        for branch in await_with.branches() {
            let borrow_snapshot = self.snapshot_borrow_state();
            let outer_locals = self.locals.clone();
            let branch_type = await_branch_pattern_type(branch.kind(), &ready, &error);
            for (name, ty) in let_else_bindings(branch.pattern(), Some(&branch_type)) {
                self.locals.insert(name, ty);
            }
            self.check_flow_items(branch.body());
            self.restore_borrow_state(borrow_snapshot);
            self.locals = outer_locals;
        }

        if await_with.applies_try() {
            Some(*ready)
        } else {
            Some(TypeKind::Result { ok: ready, error })
        }
    }

    pub(super) fn check_loop_block(
        &mut self,
        block: &arcweft_lang_hir::model::HirLoop,
        allows_value_break: bool,
    ) -> Option<TypeKind> {
        let borrow_snapshot = self.snapshot_borrow_state();
        self.loop_stack.push(LoopContext {
            label: block.label().map(str::to_owned),
            allows_value_break,
            break_types: Vec::new(),
        });
        self.check_flow_items(block.body());
        let context = self.loop_stack.pop()?;
        self.restore_borrow_state(borrow_snapshot);
        unify_loop_break_types(&context.break_types)
    }

    pub(super) fn check_while_block(&mut self, block: &arcweft_lang_hir::model::HirWhile) {
        self.expect_expr_type(block.condition(), &TypeKind::Bool, "while condition");
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
    }

    pub(super) fn check_if_let_block(&mut self, block: &arcweft_lang_hir::model::HirIfLet) {
        let expr_type = self.check_expr(block.expr());
        if let Some(guard) = block.guard() {
            self.expect_expr_type(guard, &TypeKind::Bool, "if-let guard");
        }
        let borrow_snapshot = self.snapshot_borrow_state();
        let outer_locals = self.locals.clone();
        for (name, ty) in let_else_bindings(block.pattern(), expr_type.as_ref()) {
            self.locals.insert(name, ty);
        }
        self.check_flow_items(block.body());
        let then_state = self.snapshot_borrow_state();
        self.merge_borrow_state_from_paths(
            &borrow_snapshot,
            &[borrow_snapshot.clone(), then_state],
        );
        self.locals = outer_locals;
    }

    pub(super) fn check_while_let_block(&mut self, block: &arcweft_lang_hir::model::HirWhileLet) {
        let expr_type = self.check_expr(block.expr());
        if let Some(guard) = block.guard() {
            self.expect_expr_type(guard, &TypeKind::Bool, "while-let guard");
        }
        let borrow_snapshot = self.snapshot_borrow_state();
        let outer_locals = self.locals.clone();
        for (name, ty) in let_else_bindings(block.pattern(), expr_type.as_ref()) {
            self.locals.insert(name, ty);
        }
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
        self.restore_borrow_state(borrow_snapshot);
        self.locals = outer_locals;
    }

    pub(super) fn check_for_block(&mut self, block: &arcweft_lang_hir::model::HirFor) {
        self.check_expr(block.source());
        let borrow_snapshot = self.snapshot_borrow_state();
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
        self.restore_borrow_state(borrow_snapshot);
    }

    pub(super) fn with_statement_loop(&mut self, check_body: impl FnOnce(&mut Self)) {
        let borrow_snapshot = self.snapshot_borrow_state();
        self.loop_stack.push(LoopContext {
            label: None,
            allows_value_break: false,
            break_types: Vec::new(),
        });
        check_body(self);
        self.loop_stack.pop();
        self.restore_borrow_state(borrow_snapshot);
    }

    pub(super) fn reject_active_borrows(&mut self, boundary: &str) {
        if !self.active_borrows.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "borrowed values with lifetimes {:?} cannot cross {boundary}",
                self.active_borrows
            )));
        }
    }

    pub(super) fn reject_borrow_escape(&mut self, ty: Option<&TypeKind>, destination: &str) {
        if ty.is_some_and(type_contains_borrow_ref) {
            self.errors.push(TypeCheckError::new(format!(
                "borrowed value cannot escape through {destination}"
            )));
        }
    }

    fn snapshot_runtime_scope(&self) -> TypeCheckerScopeSnapshot {
        TypeCheckerScopeSnapshot {
            active_borrows: self.active_borrows.clone(),
            borrow_local_lifetimes: self.borrow_local_lifetimes.clone(),
            locals: self.locals.clone(),
            active_presentation_defaults: self.active_presentation_defaults.clone(),
            lifetime_guarantees: self.lifetime_guarantees.clone(),
            dropped_lifetime_keys: self.dropped_lifetime_keys.clone(),
            available_lifetimes: self.available_lifetimes.clone(),
        }
    }

    fn restore_runtime_scope(&mut self, snapshot: TypeCheckerScopeSnapshot) {
        self.active_borrows = snapshot.active_borrows;
        self.borrow_local_lifetimes = snapshot.borrow_local_lifetimes;
        self.locals = snapshot.locals;
        self.active_presentation_defaults = snapshot.active_presentation_defaults;
        self.lifetime_guarantees = snapshot.lifetime_guarantees;
        self.dropped_lifetime_keys = snapshot.dropped_lifetime_keys;
        self.available_lifetimes = snapshot.available_lifetimes;
    }

    pub(super) fn with_line_runtime_scope<R>(&mut self, check: impl FnOnce(&mut Self) -> R) -> R {
        let snapshot = self.snapshot_runtime_scope();
        self.available_lifetimes.push(LifetimeScopeKind::Line);
        let output = check(self);
        self.restore_runtime_scope(snapshot);
        output
    }

    pub(super) fn with_child_task_scope<R>(
        &mut self,
        restrict_line_and_cue_lifetimes: bool,
        check: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let snapshot = self.snapshot_runtime_scope();
        if restrict_line_and_cue_lifetimes {
            self.available_lifetimes
                .retain(|scope| !matches!(scope, LifetimeScopeKind::Line | LifetimeScopeKind::Cue));
        }
        let output = check(self);
        self.restore_runtime_scope(snapshot);
        output
    }
}
