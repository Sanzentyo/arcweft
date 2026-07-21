//! Statement-level type checking.

use super::helpers::{let_else_bindings, type_kind_label};
use super::{
    BorrowStateDelta, EntityKind, Expr, LoopContext, Pattern, Stmt, SuspensionBoundary,
    TriggerPattern, TypeCheckError, TypeChecker, TypeJudgmentRule, TypeJudgmentSubject, TypeKind,
    YieldContext, default_presentation_slot_family, ident_pattern_name, is_local_ident,
    pattern_bindings_with_fallback, stmts_diverge, type_ref_kind,
};
use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        flow::{AuthoredExpr, StmtMatchArm},
    },
    reference::BorrowKind,
    types::TypeRef,
};

impl TypeChecker<'_> {
    pub(super) fn check_tail_return_block_expr_with_expected(
        &mut self,
        statements: &[Stmt],
        expr: &Expr,
        expr_source: Option<&str>,
        expr_range: Option<TextRange>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        self.register_expr_source_ranges(expr, expr_source, expr_range);
        let ty = self.with_local_mutation_scope(|this| {
            for stmt in statements {
                this.check_stmt(stmt);
            }
            this.stats.statements += 1;
            this.with_inferred_signature_partial_calls(true, |this| match expr_range {
                Some(range) => this.check_expr_with_expected_at_range(expr, expected, range),
                None => this.check_expr_with_expected(expr, expected),
            })
        });
        if let Some(ty) = ty.as_ref() {
            self.record_type_judgment_with_source_range(
                TypeJudgmentSubject::Return {
                    context: "tail block expression".to_owned(),
                },
                TypeJudgmentRule::Return,
                ty.clone(),
                expected,
                expr_range,
            );
        }
        self.reject_borrow_escape(ty.as_ref(), "function return");
        ty
    }

    pub(super) fn check_stmt(&mut self, stmt: &Stmt) {
        self.stats.statements += 1;
        self.check_seq_stmt_policy(stmt);
        match stmt {
            Stmt::Assertion(assertion) => self.check_assertion(assertion),
            Stmt::Let { .. } => self.check_let_stmt_node(stmt),
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
            Stmt::LetActionReceive { pattern, action } => {
                self.check_action_receive_binding(pattern, action);
            }
            Stmt::Return { .. } => self.check_return_stmt_node(stmt),
            Stmt::Close(expr) => self.check_return_stmt(expr.expr(), expr.source(), expr.range()),
            Stmt::Expr { .. } => self.check_expr_stmt_node(stmt),
            Stmt::Select(expr) => {
                self.check_expr_stmt(expr.expr(), expr.source(), expr.range());
                self.release_direct_drop_expr(expr.expr());
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
                self.expect_authored_expr_type(
                    expr,
                    &TypeKind::entity_ref(EntityKind::Flow),
                    "goto destination",
                );
            }
            Stmt::Thread(thread) => self.check_thread_stmt(thread),
            Stmt::DeferBlock { statements, .. } => self.check_defer_block_stmt(statements),
            Stmt::Defer { expr, .. } => {
                self.reject_active_borrows(SuspensionBoundary::Defer);
                self.check_authored_expr(expr);
            }
            Stmt::Yield(expr) => self.check_yield_stmt(expr),
            Stmt::Signal { target, value } => self.check_two_authored_exprs(target, value),
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

    fn check_let_stmt_node(&mut self, stmt: &Stmt) {
        let Stmt::Let {
            pattern,
            ty,
            expr,
            expr_source,
            expr_range,
        } = stmt
        else {
            return;
        };
        self.check_let_stmt(
            pattern,
            ty.as_ref(),
            expr,
            expr_source.as_deref(),
            *expr_range,
        );
    }

    fn check_defer_block_stmt(&mut self, statements: &[Stmt]) {
        self.reject_active_borrows(SuspensionBoundary::DeferCleanup);
        for stmt in statements {
            self.check_stmt(stmt);
        }
    }

    fn check_thread_stmt(&mut self, thread: &arcweft_lang_syntax::ast::flow::ThreadBlock) {
        self.check_thread_body(thread.body());
    }

    fn check_return_stmt(
        &mut self,
        expr: &Expr,
        expr_source: Option<&str>,
        expr_range: Option<TextRange>,
    ) {
        let expected = self.expected_returns.last().cloned().flatten();
        self.register_expr_source_ranges(expr, expr_source, expr_range);
        let ty = match expr_range {
            Some(range) => self.check_expr_with_expected_at_range(expr, expected.as_ref(), range),
            None => self.check_expr_with_expected(expr, expected.as_ref()),
        };
        if let Some(ty) = ty.as_ref() {
            self.record_type_judgment_with_source_range(
                TypeJudgmentSubject::Return {
                    context: "return statement".to_owned(),
                },
                TypeJudgmentRule::Return,
                ty.clone(),
                expected.as_ref(),
                expr_range,
            );
        }
        if let (Some(expected), Some(actual)) = (expected.as_ref(), ty.as_ref())
            && !self.types_compatible(expected, actual)
        {
            self.errors.push(TypeCheckError::new(format!(
                "return value must have type {}, found {}",
                type_kind_label(expected),
                type_kind_label(actual)
            )));
        }
        self.reject_borrow_escape(ty.as_ref(), "function or flow return");
    }

    fn check_return_stmt_node(&mut self, stmt: &Stmt) {
        let Stmt::Return {
            expr,
            expr_source,
            expr_range,
        } = stmt
        else {
            return;
        };
        self.check_return_stmt(expr, expr_source.as_deref(), *expr_range);
    }

    fn check_expr_stmt_node(&mut self, stmt: &Stmt) {
        let Stmt::Expr {
            expr,
            expr_source,
            expr_range,
        } = stmt
        else {
            return;
        };
        self.check_expr_stmt(expr, expr_source.as_deref(), *expr_range);
        self.release_direct_drop_expr(expr);
    }

    fn check_expr_stmt(
        &mut self,
        expr: &Expr,
        expr_source: Option<&str>,
        expr_range: Option<TextRange>,
    ) {
        self.register_expr_source_ranges(expr, expr_source, expr_range);
        self.with_inferred_signature_partial_calls(false, |this| match expr_range {
            Some(range) => {
                this.check_expr_with_expected_at_range(expr, None, range);
            }
            None => {
                this.check_expr(expr);
            }
        });
    }

    pub(super) fn check_authored_expr(&mut self, authored: &AuthoredExpr) -> Option<TypeKind> {
        self.check_authored_expr_with_expected(authored, None)
    }

    pub(super) fn check_authored_expr_with_expected(
        &mut self,
        authored: &AuthoredExpr,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        self.register_expr_source_ranges(authored.expr(), authored.source(), authored.range());
        match authored.range() {
            Some(range) => self.check_expr_with_expected_at_range(authored.expr(), expected, range),
            None => self.check_expr_with_expected(authored.expr(), expected),
        }
    }

    pub(super) fn expect_authored_expr_type(
        &mut self,
        authored: &AuthoredExpr,
        expected: &TypeKind,
        context: &str,
    ) {
        let actual = self.check_authored_expr_with_expected(authored, Some(expected));
        if !actual
            .as_ref()
            .is_some_and(|actual| self.types_compatible(expected, actual))
        {
            let actual = super::helpers::optional_type_kind_label(actual.as_ref());
            self.errors.push(TypeCheckError::new(format!(
                "{context} must have type {}, found {actual}",
                type_kind_label(expected)
            )));
        }
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
                | Stmt::LetActionReceive { .. }
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

    fn check_action_receive_binding(&mut self, pattern: &Pattern, action: &AuthoredExpr) {
        self.expect_authored_expr_type(
            action,
            &TypeKind::entity_ref(EntityKind::Action),
            "action receive target",
        );
        let action_event = TypeKind::action_event();
        for (name, binding_ty) in pattern_bindings_with_fallback(pattern, &action_event) {
            self.bind_local(name, binding_ty);
        }
    }

    fn check_two_authored_exprs(&mut self, first: &AuthoredExpr, second: &AuthoredExpr) {
        self.check_authored_expr(first);
        self.check_authored_expr(second);
    }

    fn check_assign_stmt(&mut self, target: &AuthoredExpr, expr: &AuthoredExpr) {
        self.register_expr_source_ranges(target.expr(), target.source(), target.range());
        self.register_expr_source_ranges(expr.expr(), expr.source(), expr.range());
        let target_ty = if let Expr::Deref(deref) = target.expr() {
            match self.check_expr(deref.operand()) {
                Some(TypeKind::BorrowRef {
                    kind: BorrowKind::Mutable,
                    inner,
                    ..
                }) => Some(*inner),
                Some(TypeKind::BorrowRef {
                    kind: BorrowKind::Shared,
                    ..
                }) => {
                    self.errors
                        .push(TypeCheckError::unsupported_assignment_target(
                            assignment_target_label(target.expr()),
                            "shared references cannot be written through",
                        ));
                    None
                }
                Some(actual) => {
                    self.errors
                        .push(TypeCheckError::unsupported_assignment_target(
                            assignment_target_label(target.expr()),
                            format!(
                                "dereference assignment requires a mutable reference, found {}",
                                type_kind_label(&actual)
                            ),
                        ));
                    None
                }
                None => None,
            }
        } else if let Some((receiver, field)) = direct_assignment_target(target.expr()) {
            let Some(receiver_ty) = self.symbol_type(receiver).cloned() else {
                self.errors.push(TypeCheckError::new(format!(
                    "assignment receiver `{receiver}` is not bound"
                )));
                self.check_authored_expr(expr);
                return;
            };
            if let Some(target_ty) = self.nominal_field_type(&receiver_ty, field) {
                Some(target_ty)
            } else {
                self.errors
                    .push(TypeCheckError::unsupported_assignment_target(
                        assignment_target_label(target.expr()),
                        format!("field `{field}` is not known on {receiver_ty:?}"),
                    ));
                None
            }
        } else {
            self.errors
                .push(TypeCheckError::unsupported_assignment_target(
                    assignment_target_label(target.expr()),
                    "only direct local record fields and mutable-reference dereferences are executable",
                ));
            None
        };
        let Some(target_ty) = target_ty else {
            self.check_authored_expr(expr);
            return;
        };
        let expr_ty = match expr.range() {
            Some(range) => {
                self.check_expr_with_expected_at_range(expr.expr(), Some(&target_ty), range)
            }
            None => self.check_expr_with_expected(expr.expr(), Some(&target_ty)),
        };
        if let Some(expr_ty) = expr_ty
            && !self.types_compatible(&target_ty, &expr_ty)
        {
            self.errors.push(TypeCheckError::new(format!(
                "assignment expects {}, but expression has {}",
                type_kind_label(&target_ty),
                type_kind_label(&expr_ty)
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

    fn check_stmt_while(&mut self, condition: &AuthoredExpr, body: &[Stmt]) {
        self.expect_authored_expr_type(condition, &TypeKind::Bool, "while condition");
        self.with_statement_loop(|this| {
            for stmt in body {
                this.check_stmt(stmt);
            }
        });
    }

    fn check_let_stmt(
        &mut self,
        pattern: &Pattern,
        annotation: Option<&TypeRef>,
        expr: &Expr,
        expr_source: Option<&str>,
        expr_range: Option<TextRange>,
    ) {
        self.last_checked_closure_effect_callable = None;
        self.last_checked_curried_signature_call = None;
        self.register_expr_source_ranges(expr, expr_source, expr_range);
        let annotated_ty = annotation.map(type_ref_kind);
        let ty = self
            .with_inferred_signature_partial_calls(true, |this| match expr_range {
                Some(range) => {
                    this.check_expr_with_expected_at_range(expr, annotated_ty.as_ref(), range)
                }
                None => this.check_expr_with_expected(expr, annotated_ty.as_ref()),
            })
            .or_else(|| annotated_ty.clone());
        if let (Some(annotation), Some(actual)) = (annotation, ty.as_ref()) {
            let expected = annotated_ty
                .clone()
                .unwrap_or_else(|| type_ref_kind(annotation));
            if !self.types_compatible(&expected, actual) {
                self.errors.push(TypeCheckError::new(format!(
                    "let annotation expects {}, but expression has {}",
                    type_kind_label(&expected),
                    type_kind_label(actual)
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
            self.record_type_judgment_with_source_range(
                TypeJudgmentSubject::LetBinding {
                    pattern: format!("{pattern:?}"),
                },
                TypeJudgmentRule::LetBinding,
                ty.clone(),
                annotated_ty.as_ref(),
                expr_range,
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
                && let Some(previous) =
                    self.set_active_presentation_default(slot_family, name.to_owned())
            {
                self.errors.push(TypeCheckError::new(format!(
                    "presentation `{slot_family}` default slot already has live handle `{previous}`; use an explicit `slot = @slot.{slot_family}.name` for simultaneous values"
                )));
            }
            for (name, binding_ty) in pattern_bindings_with_fallback(pattern, ty) {
                self.bind_local(name, binding_ty);
            }
            if let Some(name) = ident_pattern_name(pattern)
                && let Some(signature) = self.callable_signature_for_function_expr(expr, ty)
            {
                self.bind_local_callable_signature(name, signature);
            }
            if let Some(name) = ident_pattern_name(pattern)
                && let Some(callable) = self.closure_effect_callable_for_function_expr(expr, ty)
            {
                self.bind_local_function_effect(name, callable);
            }
            if let Some(name) = ident_pattern_name(pattern)
                && let Some(value) = self.curried_signature_call_for_function_expr(expr, ty)
            {
                self.bind_local_curried_signature_call(name, value);
            }
            if let Some(name) = ident_pattern_name(pattern)
                && let Some(param_name) = self.higher_order_param_alias_for_function_expr(expr, ty)
            {
                self.bind_local_higher_order_param_alias(name, &param_name);
            }
        }
        self.last_checked_closure_effect_callable = None;
        self.last_checked_curried_signature_call = None;
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
        expr: &AuthoredExpr,
        else_body: &[Stmt],
    ) {
        let expr_type = self
            .check_authored_expr(expr)
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

    fn check_if_stmt(&mut self, condition: &AuthoredExpr, body: &[Stmt], else_body: &[Stmt]) {
        self.expect_authored_expr_type(condition, &TypeKind::Bool, "if condition");
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
        expr: &AuthoredExpr,
        guard: Option<&AuthoredExpr>,
        body: &[Stmt],
    ) {
        let expr_type = self.check_authored_expr(expr);
        let borrow_checkpoint = self.checkpoint_borrow_state();
        let local_snapshot =
            self.insert_scoped_locals(let_else_bindings(pattern, expr_type.as_ref()));
        if let Some(guard) = guard {
            self.expect_authored_expr_type(guard, &TypeKind::Bool, "while-let guard");
        }
        self.with_statement_loop(|this| {
            for stmt in body {
                this.check_stmt(stmt);
            }
        });
        self.restore_borrow_state(borrow_checkpoint);
        self.restore_scoped_locals(local_snapshot);
    }

    fn check_stmt_for(&mut self, pattern: &Pattern, source: &AuthoredExpr, body: &[Stmt]) {
        let source_ty = self.check_authored_expr(source);
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

    fn check_match_stmt(&mut self, expr: &AuthoredExpr, arms: &[StmtMatchArm]) {
        let expr_type = self.check_authored_expr(expr);
        let base_borrow_checkpoint = self.checkpoint_borrow_state();
        let mut arm_states = Vec::new();
        for arm in arms {
            self.restore_borrow_state(base_borrow_checkpoint);
            let local_snapshot =
                self.insert_scoped_locals(let_else_bindings(arm.pattern(), expr_type.as_ref()));
            if let Some(guard) = arm.guard_authored() {
                self.expect_authored_expr_type(guard, &TypeKind::Bool, "match guard");
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

    fn check_break_stmt(&mut self, label: Option<&str>, expr: Option<&AuthoredExpr>) {
        let Some(index) = self.resolve_loop_label(label) else {
            self.errors.push(TypeCheckError::new(label.map_or_else(
                || "break is only allowed inside loop, while, or for".to_owned(),
                |label| format!("break label `'{label}` does not name an active loop"),
            )));
            if let Some(expr) = expr {
                self.check_authored_expr(expr);
            }
            return;
        };
        let allows_value_break = self.loop_stack[index].allows_value_break;
        match expr {
            Some(expr) if !allows_value_break => {
                self.errors.push(TypeCheckError::new(
                    "break expr is allowed only in loop blocks".to_owned(),
                ));
                self.check_authored_expr(expr);
            }
            Some(expr) => {
                if let Some(ty) = self.check_authored_expr(expr) {
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

    pub(super) fn check_out_stmt(
        &mut self,
        label: Option<&str>,
        expr: &AuthoredExpr,
    ) -> Option<TypeKind> {
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
        self.check_authored_expr(expr)
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

fn direct_assignment_target(target: &Expr) -> Option<(&str, &str)> {
    let Expr::Select(select) = target else {
        return None;
    };
    let Expr::Path(receiver) = select.target() else {
        return None;
    };
    Some((receiver.as_label(), select.member().as_str()))
}

fn assignment_target_label(target: &Expr) -> String {
    match target {
        Expr::Borrow(borrow) => format!(
            "&{}{}",
            borrow.kind().source_qualifier(),
            assignment_target_label(borrow.operand())
        ),
        Expr::Deref(deref) => format!("*{}", assignment_target_label(deref.operand())),
        Expr::Select(select) => format!(
            "{}.{}",
            assignment_target_label(select.target()),
            select.member().as_str()
        ),
        Expr::Path(path) => path.as_label().to_owned(),
        Expr::ShortVariant(name) => format!(".{name}"),
        Expr::Index { target, .. } => format!("{}[]", assignment_target_label(target)),
        Expr::Call(_) => "call(...)".to_owned(),
        _ => format!("{target:?}"),
    }
}
