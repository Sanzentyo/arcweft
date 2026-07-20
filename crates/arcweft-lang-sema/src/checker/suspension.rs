//! Suspension boundaries and runtime-scope helpers.

use super::helpers::{let_else_bindings, pattern_bindings_with_fallback, type_kind_label};
use super::{
    BorrowStateDelta, Expr, LifetimeScopeKind, LoopContext, SuspensionBoundary, TypeCheckError,
    TypeChecker, TypeCheckerScopeSnapshot, TypeKind, YieldContext, await_branch_pattern_type,
    type_contains_borrow_ref, unify_loop_break_types,
};
use arcweft_lang_syntax::{ast::flow::AuthoredExpr, expr::AwaitPropagation};

impl TypeChecker<'_> {
    pub(super) fn check_yield_stmt(&mut self, expr: &AuthoredExpr) {
        self.reject_active_borrows(SuspensionBoundary::Yield);
        let actual = self.check_authored_expr(expr);
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
                                "yielded item types do not match, found {} and {}",
                                type_kind_label(expected),
                                type_kind_label(&actual)
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
                        "yielded item must have type {}, found {}",
                        type_kind_label(item_ty),
                        type_kind_label(&actual)
                    )));
                }
            }
        }
    }

    pub(super) fn check_await_expr(
        &mut self,
        expr: &Expr,
        propagation: AwaitPropagation,
    ) -> Option<TypeKind> {
        self.reject_active_borrows(SuspensionBoundary::Await);
        match self.check_expr(expr) {
            Some(TypeKind::Need { ready, .. })
                if propagation == AwaitPropagation::PropagateError =>
            {
                Some(*ready)
            }
            Some(TypeKind::Need { ready, error }) => Some(TypeKind::Result { ok: ready, error }),
            Some(other) => {
                self.errors.push(TypeCheckError::new(format!(
                    "await expression must have Need<T, E> type, found {}",
                    type_kind_label(&other)
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
        self.reject_active_borrows(SuspensionBoundary::Await);
        let ty = self.check_authored_expr(await_with.expr_authored());
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
            let borrow_checkpoint = self.checkpoint_borrow_state();
            let branch_type = await_branch_pattern_type(branch.kind(), &ready, &error);
            let local_snapshot =
                self.insert_scoped_locals(let_else_bindings(branch.pattern(), Some(&branch_type)));
            self.check_flow_items(branch.body());
            self.restore_borrow_state(borrow_checkpoint);
            self.restore_scoped_locals(local_snapshot);
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
        let borrow_checkpoint = self.checkpoint_borrow_state();
        self.loop_stack.push(LoopContext {
            label: block.label().map(str::to_owned),
            allows_value_break,
            break_types: Vec::new(),
        });
        self.check_flow_items(block.body());
        let context = self.loop_stack.pop()?;
        self.restore_borrow_state(borrow_checkpoint);
        unify_loop_break_types(&context.break_types)
    }

    pub(super) fn check_while_block(&mut self, block: &arcweft_lang_hir::model::HirWhile) {
        self.expect_authored_expr_type(
            block.condition_authored(),
            &TypeKind::Bool,
            "while condition",
        );
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
    }

    pub(super) fn check_if_let_block(&mut self, block: &arcweft_lang_hir::model::HirIfLet) {
        let expr_type = self.check_authored_expr(block.expr_authored());
        let borrow_checkpoint = self.checkpoint_borrow_state();
        let local_snapshot =
            self.insert_scoped_locals(let_else_bindings(block.pattern(), expr_type.as_ref()));
        if let Some(guard) = block.guard_authored() {
            self.expect_authored_expr_type(guard, &TypeKind::Bool, "if-let guard");
        }
        self.check_flow_items(block.body());
        let then_state = self.capture_borrow_state_delta(borrow_checkpoint);
        self.restore_borrow_state(borrow_checkpoint);
        self.restore_scoped_locals(local_snapshot);
        self.check_flow_items(block.else_body());
        let else_state = self.capture_borrow_state_delta(borrow_checkpoint);
        let unchanged_state = BorrowStateDelta::default();
        self.merge_borrow_state_from_deltas(
            borrow_checkpoint,
            &[&unchanged_state, &then_state, &else_state],
        );
    }

    pub(super) fn check_while_let_block(&mut self, block: &arcweft_lang_hir::model::HirWhileLet) {
        let expr_type = self.check_authored_expr(block.expr_authored());
        let borrow_checkpoint = self.checkpoint_borrow_state();
        let local_snapshot =
            self.insert_scoped_locals(let_else_bindings(block.pattern(), expr_type.as_ref()));
        if let Some(guard) = block.guard_authored() {
            self.expect_authored_expr_type(guard, &TypeKind::Bool, "while-let guard");
        }
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
        self.restore_borrow_state(borrow_checkpoint);
        self.restore_scoped_locals(local_snapshot);
    }

    pub(super) fn check_for_block(&mut self, block: &arcweft_lang_hir::model::HirFor) {
        let source_ty = self.check_authored_expr(block.source_authored());
        let borrow_checkpoint = self.checkpoint_borrow_state();
        let item_ty = self
            .check_for_iteration_source(source_ty.as_ref())
            .map_or(TypeKind::Unit, |typing| typing.item_ty);
        let local_snapshot =
            self.insert_scoped_locals(pattern_bindings_with_fallback(block.pattern(), &item_ty));
        self.register_borrow_bindings(block.pattern(), &item_ty);
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
        self.restore_borrow_state(borrow_checkpoint);
        self.restore_scoped_locals(local_snapshot);
    }

    pub(super) fn with_statement_loop(&mut self, check_body: impl FnOnce(&mut Self)) {
        let borrow_checkpoint = self.checkpoint_borrow_state();
        self.loop_stack.push(LoopContext {
            label: None,
            allows_value_break: false,
            break_types: Vec::new(),
        });
        check_body(self);
        self.loop_stack.pop();
        self.restore_borrow_state(borrow_checkpoint);
    }

    pub(super) fn reject_active_borrows(&mut self, boundary: SuspensionBoundary) {
        self.stats.borrow_boundary_checks += 1;
        self.record_closure_suspension_boundary(boundary);
        if self.active_borrow_total > 0 {
            let labels = self.active_borrow_labels();
            self.errors.push(TypeCheckError::new(format!(
                "borrowed values with lifetimes {labels:?} cannot cross {}",
                boundary.label()
            )));
        }
    }

    pub(super) fn reject_borrow_escape(&mut self, ty: Option<&TypeKind>, destination: &str) {
        self.stats.borrow_escape_checks += 1;
        if ty.is_some_and(type_contains_borrow_ref) {
            self.errors.push(TypeCheckError::new(format!(
                "borrowed value cannot escape through {destination}"
            )));
        }
    }

    fn snapshot_runtime_scope(&mut self) -> TypeCheckerScopeSnapshot {
        let borrow_checkpoint = self.checkpoint_borrow_state();
        TypeCheckerScopeSnapshot {
            borrow_checkpoint,
            active_presentation_defaults: self.active_presentation_defaults.clone(),
            lifetime_guarantees: self.lifetime_guarantees.clone(),
            dropped_lifetime_keys: self.dropped_lifetime_keys.clone(),
            available_lifetimes: self.available_lifetimes.clone(),
        }
    }

    fn restore_runtime_scope(&mut self, snapshot: TypeCheckerScopeSnapshot) {
        self.restore_borrow_state(snapshot.borrow_checkpoint);
        self.active_presentation_defaults = snapshot.active_presentation_defaults;
        self.lifetime_guarantees = snapshot.lifetime_guarantees;
        self.dropped_lifetime_keys = snapshot.dropped_lifetime_keys;
        self.available_lifetimes = snapshot.available_lifetimes;
    }

    pub(super) fn with_line_runtime_scope<R>(&mut self, check: impl FnOnce(&mut Self) -> R) -> R {
        let snapshot = self.snapshot_runtime_scope();
        let output = self.with_local_mutation_scope(|checker| {
            checker.available_lifetimes.push(LifetimeScopeKind::Line);
            check(checker)
        });
        self.restore_runtime_scope(snapshot);
        output
    }

    pub(super) fn with_child_task_scope<R>(
        &mut self,
        restrict_line_and_cue_lifetimes: bool,
        check: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let snapshot = self.snapshot_runtime_scope();
        let output = self.with_local_mutation_scope(|checker| {
            if restrict_line_and_cue_lifetimes {
                checker.available_lifetimes.retain(|scope| {
                    !matches!(scope, LifetimeScopeKind::Line | LifetimeScopeKind::Cue)
                });
            }
            check(checker)
        });
        self.restore_runtime_scope(snapshot);
        output
    }
}
