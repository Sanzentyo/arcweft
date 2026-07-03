//! Statement-level type checking.

use super::helpers::let_else_bindings;
use super::{
    BorrowStateDelta, EntityKind, Expr, LoopContext, Pattern, Stmt, TriggerPattern, TypeCheckError,
    TypeChecker, TypeJudgmentRule, TypeJudgmentSubject, TypeKind, YieldContext,
    default_presentation_slot_family, ident_pattern_name, is_local_ident,
    pattern_bindings_with_fallback, stmts_diverge, type_ref_kind,
};
use arcweft_lang_syntax::{ast::flow::StmtMatchArm, types::TypeRef};

impl TypeChecker<'_> {
    pub(super) fn check_tail_return_block_expr_with_expected(
        &mut self,
        statements: &[Stmt],
        expr: &Expr,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let ty = self.with_local_mutation_scope(|this| {
            for stmt in statements {
                this.check_stmt(stmt);
            }
            this.stats.statements += 1;
            this.check_expr_with_expected(expr, expected)
        });
        if let Some(ty) = ty.as_ref() {
            self.record_type_judgment(
                TypeJudgmentSubject::Return {
                    context: "tail block expression".to_owned(),
                },
                TypeJudgmentRule::Return,
                ty.clone(),
                expected,
            );
        }
        self.reject_borrow_escape(ty.as_ref(), "function return");
        ty
    }

    pub(super) fn check_stmt(&mut self, stmt: &Stmt) {
        self.stats.statements += 1;
        self.check_seq_stmt_policy(stmt);
        match stmt {
            Stmt::Let {
                pattern, ty, expr, ..
            } => self.check_let_stmt(pattern, ty.as_ref(), expr),
            Stmt::Assign { target, expr } => self.check_assign_stmt(target, expr),
            Stmt::LetElse {
                pattern,
                ty,
                expr,
                else_body,
            } => self.check_let_else_stmt(pattern, ty.as_ref(), expr, else_body),
            Stmt::LetChoice { .. }
            | Stmt::LetScope { .. }
            | Stmt::LetLoop { .. }
            | Stmt::LetAwait { .. } => self.reject_unlowered_stmt_binding(stmt),
            Stmt::LetTextSubmit { pattern, target } => {
                self.check_text_submit_binding(pattern, target);
            }
            Stmt::Return(expr) | Stmt::Close(expr) => self.check_return_stmt(expr),
            Stmt::Expr(expr) | Stmt::Select(expr) => {
                self.check_expr(expr);
                self.release_direct_drop_expr(expr);
            }
            Stmt::Out { label, expr } => {
                if self.line_out_depth == 0 {
                    self.errors.push(TypeCheckError::new(
                        "`out` can only be used in a dialogue line plan, cue block, or content scope"
                            .to_owned(),
                    ));
                }
                let ty = self.check_out_stmt(label.as_deref(), expr);
                self.reject_borrow_escape(ty.as_ref(), "line-plan out value");
            }
            Stmt::Goto(expr) => {
                self.expect_expr_type(
                    expr,
                    &TypeKind::entity_ref(EntityKind::Flow),
                    "goto destination",
                );
            }
            Stmt::Thread(thread) => {
                self.check_thread_body(thread.body());
            }
            Stmt::DeferBlock { statements, .. } => {
                self.reject_active_borrows("defer cleanup boundary");
                for stmt in statements {
                    self.check_stmt(stmt);
                }
            }
            Stmt::Defer { expr, .. } => {
                self.reject_active_borrows("suspension boundary");
                self.check_expr(expr);
            }
            Stmt::Yield(expr) => self.check_yield_stmt(expr),
            Stmt::Signal { target, value } => self.check_two_exprs(target, value),
            Stmt::LifetimeSet { target, expr } => self.check_lifetime_set_stmt(target, expr),
            Stmt::Wait(target) => self.check_wait_stmt(target),
            Stmt::On { body, .. } => self.check_on_stmt(stmt, body),
            Stmt::UnsafeLifetime { reason, body, .. } => {
                self.check_unsafe_lifetime_stmt(reason.as_ref(), body);
            }
            Stmt::If {
                condition,
                body,
                else_body,
            } => self.check_if_stmt(condition, body, else_body),
            Stmt::Loop { body } => self.check_stmt_loop(body),
            Stmt::While { condition, body } => self.check_stmt_while(condition, body),
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => self.check_stmt_while_let(pattern, expr, guard.as_ref(), body),
            Stmt::For {
                pattern,
                source,
                body,
            } => self.check_stmt_for(pattern, source, body),
            Stmt::Match { expr, arms } => self.check_match_stmt(expr, arms),
            Stmt::Break { label, expr } => self.check_break_stmt(label.as_deref(), expr.as_ref()),
            Stmt::Continue { label } => self.check_continue_stmt(label.as_deref()),
            Stmt::Raw(raw) => self.errors.push(TypeCheckError::new(format!(
                "raw {:?} recovery node is not type-checkable: {}",
                raw.family(),
                raw.source()
            ))),
        }
    }

    fn check_return_stmt(&mut self, expr: &Expr) {
        let expected = self.expected_returns.last().cloned();
        let ty = self.check_expr_with_expected(expr, expected.as_ref());
        if let Some(ty) = ty.as_ref() {
            self.record_type_judgment(
                TypeJudgmentSubject::Return {
                    context: "return statement".to_owned(),
                },
                TypeJudgmentRule::Return,
                ty.clone(),
                expected.as_ref(),
            );
        }
        self.reject_borrow_escape(ty.as_ref(), "function or flow return");
    }

    fn check_seq_stmt_policy(&mut self, stmt: &Stmt) {
        if !self
            .yield_stack
            .last()
            .is_some_and(|context| matches!(context, YieldContext::Seq { .. }))
        {
            return;
        }
        if matches!(
            stmt,
            Stmt::Thread(_)
                | Stmt::DeferBlock { .. }
                | Stmt::Defer { .. }
                | Stmt::Signal { .. }
                | Stmt::LifetimeSet { .. }
                | Stmt::Wait(_)
                | Stmt::LetTextSubmit { .. }
                | Stmt::On { .. }
                | Stmt::Select(_)
        ) {
            self.errors.push(TypeCheckError::new(
                "`seq` blocks are pure and cannot perform runtime effects".to_owned(),
            ));
        }
    }

    fn reject_unlowered_stmt_binding(&mut self, stmt: &Stmt) {
        let kind = match stmt {
            Stmt::LetChoice { .. } => "choice",
            Stmt::LetScope { .. } => "scope",
            Stmt::LetLoop { .. } => "loop",
            Stmt::LetAwait { .. } => "await",
            _ => return,
        };
        self.errors.push(TypeCheckError::new(format!(
            "{kind} expression binding must be lowered before type checking"
        )));
    }

    fn check_text_submit_binding(&mut self, pattern: &Pattern, target: &Expr) {
        self.expect_expr_type(
            target,
            &TypeKind::entity_ref(EntityKind::Input),
            "text submit target",
        );
        if let Some(name) = ident_pattern_name(pattern) {
            self.bind_local(name.to_owned(), TypeKind::String);
        }
    }

    fn check_two_exprs(&mut self, first: &Expr, second: &Expr) {
        self.check_expr(first);
        self.check_expr(second);
    }

    fn check_assign_stmt(&mut self, target: &Expr, expr: &Expr) {
        let target_ty = self.check_expr(target);
        let expr_ty = self.check_expr_with_expected(expr, target_ty.as_ref());
        if let (Some(target_ty), Some(expr_ty)) = (target_ty, expr_ty)
            && !self.types_compatible(&target_ty, &expr_ty)
        {
            self.errors.push(TypeCheckError::new(format!(
                "assignment expects {target_ty:?}, but expression has {expr_ty:?}"
            )));
        }
    }

    fn check_on_stmt(&mut self, stmt: &Stmt, body: &[Stmt]) {
        self.with_local_mutation_scope(|this| {
            this.bind_on_head_locals(stmt);
            for stmt in body {
                this.check_stmt(stmt);
            }
        });
    }

    fn check_unsafe_lifetime_stmt(&mut self, reason: Option<&Expr>, body: &[Stmt]) {
        if let Some(reason) = reason {
            self.check_expr(reason);
        }
        for stmt in body {
            self.check_stmt(stmt);
        }
    }

    fn check_stmt_while(&mut self, condition: &Expr, body: &[Stmt]) {
        self.expect_expr_type(condition, &TypeKind::Bool, "while condition");
        self.with_statement_loop(|this| {
            for stmt in body {
                this.check_stmt(stmt);
            }
        });
    }

    fn check_let_stmt(&mut self, pattern: &Pattern, annotation: Option<&TypeRef>, expr: &Expr) {
        let annotated_ty = annotation.map(type_ref_kind);
        let ty = self
            .check_expr_with_expected(expr, annotated_ty.as_ref())
            .or_else(|| annotated_ty.clone());
        if let (Some(annotation), Some(actual)) = (annotation, ty.as_ref()) {
            let expected = annotated_ty
                .clone()
                .unwrap_or_else(|| type_ref_kind(annotation));
            if !self.types_compatible(&expected, actual) {
                self.errors.push(TypeCheckError::new(format!(
                    "let annotation expects {expected:?}, but expression has {actual:?}"
                )));
            }
        }
        let binding_ty = annotated_ty
            .as_ref()
            .filter(|expected| {
                ty.as_ref()
                    .is_some_and(|actual| self.types_compatible(expected, actual))
            })
            .cloned()
            .or_else(|| ty.clone());
        if let Some(ty) = binding_ty.as_ref() {
            self.record_type_judgment(
                TypeJudgmentSubject::LetBinding {
                    pattern: format!("{pattern:?}"),
                },
                TypeJudgmentRule::LetBinding,
                ty.clone(),
                annotated_ty.as_ref(),
            );
            if let Some(name) = ident_pattern_name(pattern)
                && name == "RuntimeFrame"
            {
                self.errors.push(TypeCheckError::new(
                    "`RuntimeFrame` is not a valid runtime API name; use `RuntimeStep` terminology"
                        .to_owned(),
                ));
            }
            if let Some(name) = ident_pattern_name(pattern)
                && let Some(slot_family) = default_presentation_slot_family(expr)
                && let Some(previous) = self
                    .active_presentation_defaults
                    .insert(slot_family.to_owned(), name.to_owned())
            {
                self.errors.push(TypeCheckError::new(format!(
                    "presentation `{slot_family}` default slot already has live handle `{previous}`; use an explicit `slot = @slot.{slot_family}.name` for simultaneous values"
                )));
            }
            for (name, binding_ty) in pattern_bindings_with_fallback(pattern, ty) {
                self.bind_local(name, binding_ty);
            }
        }
        if let Some(borrow_ty) = annotated_ty.as_ref().or(ty.as_ref()) {
            self.register_borrow_bindings(pattern, borrow_ty);
        }
    }

    fn bind_on_head_locals(&mut self, stmt: &Stmt) {
        let Stmt::On { trigger, .. } = stmt else {
            return;
        };
        let pattern = match trigger {
            TriggerPattern::Input(pattern)
            | TriggerPattern::Event(pattern)
            | TriggerPattern::Mark(pattern)
            | TriggerPattern::Select(pattern)
            | TriggerPattern::Task(pattern)
            | TriggerPattern::Scope(pattern) => Some(pattern),
            TriggerPattern::Signal { value, .. } => value.as_ref(),
            TriggerPattern::Timeout(_) | TriggerPattern::Expr(_) => None,
        };
        if let Some(pattern) = pattern {
            for (name, ty) in let_else_bindings(pattern, None) {
                self.bind_local(name, ty);
            }
            if let Pattern::Ident(name) = pattern
                && is_local_ident(name)
                && !self.locals.contains_key(name)
            {
                self.bind_local(name.to_owned(), TypeKind::Unit);
            }
        }
    }

    fn check_let_else_stmt(
        &mut self,
        pattern: &Pattern,
        annotation: Option<&TypeRef>,
        expr: &Expr,
        else_body: &[Stmt],
    ) {
        let expr_type = self
            .check_expr(expr)
            .or_else(|| annotation.map(type_ref_kind));
        for stmt in else_body {
            self.check_stmt(stmt);
        }
        if !stmts_diverge(else_body) {
            self.errors.push(TypeCheckError::new(
                "let-else else block must leave the current continuation".to_owned(),
            ));
        }
        for (name, ty) in let_else_bindings(pattern, expr_type.as_ref()) {
            self.bind_local(name, ty);
        }
        let annotated_ty = annotation.map(type_ref_kind);
        if let Some(borrow_ty) = annotated_ty.as_ref().or(expr_type.as_ref()) {
            self.register_borrow_bindings(pattern, borrow_ty);
        }
    }

    fn check_if_stmt(&mut self, condition: &Expr, body: &[Stmt], else_body: &[Stmt]) {
        self.expect_expr_type(condition, &TypeKind::Bool, "if condition");
        let borrow_checkpoint = self.checkpoint_borrow_state();
        self.with_local_mutation_scope(|this| {
            for stmt in body {
                this.check_stmt(stmt);
            }
        });
        let then_state = self.capture_borrow_state_delta(borrow_checkpoint);
        self.restore_borrow_state(borrow_checkpoint);
        self.with_local_mutation_scope(|this| {
            for stmt in else_body {
                this.check_stmt(stmt);
            }
        });
        let else_state = self.capture_borrow_state_delta(borrow_checkpoint);
        let unchanged_state = BorrowStateDelta::default();
        let else_state = if else_body.is_empty() {
            &unchanged_state
        } else {
            &else_state
        };
        self.merge_borrow_state_from_deltas(borrow_checkpoint, &[&then_state, else_state]);
    }

    fn check_stmt_loop(&mut self, body: &[Stmt]) {
        let borrow_checkpoint = self.checkpoint_borrow_state();
        self.loop_stack.push(LoopContext {
            label: None,
            allows_value_break: true,
            break_types: Vec::new(),
        });
        for stmt in body {
            self.check_stmt(stmt);
        }
        self.loop_stack.pop();
        self.restore_borrow_state(borrow_checkpoint);
    }

    fn check_stmt_while_let(
        &mut self,
        pattern: &Pattern,
        expr: &Expr,
        guard: Option<&Expr>,
        body: &[Stmt],
    ) {
        let expr_type = self.check_expr(expr);
        let borrow_checkpoint = self.checkpoint_borrow_state();
        let local_snapshot =
            self.insert_scoped_locals(let_else_bindings(pattern, expr_type.as_ref()));
        if let Some(guard) = guard {
            self.expect_expr_type(guard, &TypeKind::Bool, "while-let guard");
        }
        self.with_statement_loop(|this| {
            for stmt in body {
                this.check_stmt(stmt);
            }
        });
        self.restore_borrow_state(borrow_checkpoint);
        self.restore_scoped_locals(local_snapshot);
    }

    fn check_stmt_for(&mut self, pattern: &Pattern, source: &Expr, body: &[Stmt]) {
        let source_ty = self.check_expr(source);
        let borrow_checkpoint = self.checkpoint_borrow_state();
        let item_ty = self
            .check_for_iteration_source(source_ty.as_ref())
            .map_or(TypeKind::Unit, |typing| typing.item_ty);
        let local_snapshot =
            self.insert_scoped_locals(pattern_bindings_with_fallback(pattern, &item_ty));
        self.register_borrow_bindings(pattern, &item_ty);
        self.with_statement_loop(|this| {
            for stmt in body {
                this.check_stmt(stmt);
            }
        });
        self.restore_borrow_state(borrow_checkpoint);
        self.restore_scoped_locals(local_snapshot);
    }

    fn check_match_stmt(&mut self, expr: &Expr, arms: &[StmtMatchArm]) {
        let expr_type = self.check_expr(expr);
        let base_borrow_checkpoint = self.checkpoint_borrow_state();
        let mut arm_states = Vec::new();
        for arm in arms {
            self.restore_borrow_state(base_borrow_checkpoint);
            let local_snapshot =
                self.insert_scoped_locals(let_else_bindings(arm.pattern(), expr_type.as_ref()));
            if let Some(guard) = arm.guard() {
                self.expect_expr_type(guard, &TypeKind::Bool, "match guard");
            }
            for stmt in arm.body() {
                self.check_stmt(stmt);
            }
            arm_states.push(self.capture_borrow_state_delta(base_borrow_checkpoint));
            self.restore_scoped_locals(local_snapshot);
        }
        self.check_choice_match_exhaustive(
            expr_type.as_ref(),
            arms.iter()
                .map(arcweft_lang_syntax::ast::flow::StmtMatchArm::pattern),
        );
        if arm_states.is_empty() {
            self.restore_borrow_state(base_borrow_checkpoint);
        } else {
            let arm_state_refs = arm_states.iter().collect::<Vec<_>>();
            self.merge_borrow_state_from_deltas(base_borrow_checkpoint, &arm_state_refs);
        }
    }

    fn check_break_stmt(&mut self, label: Option<&str>, expr: Option<&Expr>) {
        let Some(index) = self.resolve_loop_label(label) else {
            self.errors.push(TypeCheckError::new(label.map_or_else(
                || "break is only allowed inside loop, while, or for".to_owned(),
                |label| format!("break label `'{label}` does not name an active loop"),
            )));
            if let Some(expr) = expr {
                self.check_expr(expr);
            }
            return;
        };
        let allows_value_break = self.loop_stack[index].allows_value_break;
        match expr {
            Some(expr) if !allows_value_break => {
                self.errors.push(TypeCheckError::new(
                    "break expr is allowed only in loop blocks".to_owned(),
                ));
                self.check_expr(expr);
            }
            Some(expr) => {
                if let Some(ty) = self.check_expr(expr) {
                    self.loop_stack[index].break_types.push(ty);
                }
            }
            None if allows_value_break => {
                self.loop_stack[index].break_types.push(TypeKind::Unit);
            }
            None => {}
        }
    }

    fn check_continue_stmt(&mut self, label: Option<&str>) {
        let resolves_to_loop = self.resolve_loop_label(label).is_some();
        if !resolves_to_loop && (self.line_cancel_depth == 0 || label.is_some()) {
            self.errors.push(TypeCheckError::new(label.map_or_else(
                || {
                    "continue is only allowed inside loop, while, for, or line cancellation"
                        .to_owned()
                },
                |label| format!("continue label `'{label}` does not name an active loop"),
            )));
        }
    }

    pub(super) fn check_out_stmt(&mut self, label: Option<&str>, expr: &Expr) -> Option<TypeKind> {
        if let Some(label) = label
            && !self
                .line_label_stack
                .iter()
                .rev()
                .any(|active| active.as_deref() == Some(label))
        {
            self.errors.push(TypeCheckError::new(format!(
                "out label `'{label}` does not name an active line-plan scope"
            )));
        }
        self.check_expr(expr)
    }

    fn resolve_loop_label(&self, label: Option<&str>) -> Option<usize> {
        match label {
            Some(label) => self
                .loop_stack
                .iter()
                .rposition(|context| context.label.as_deref() == Some(label)),
            None => self.loop_stack.len().checked_sub(1),
        }
    }
}
