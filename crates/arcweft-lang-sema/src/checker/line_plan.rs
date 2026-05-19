//! Dialogue line-plan type checking.

use super::helpers::let_else_bindings;
use super::{
    CancelRuleSyntax, DialogueToken, Expr, LifetimeAccessMode, LifetimeScopeKind, LinePlanItem,
    Pattern, Stmt, TriggerPattern, TypeCheckError, TypeChecker, TypeKind, lifetime_key,
    merge_line_output,
};
use arcweft_lang_syntax::ast::{flow::WaitTarget, line_plan::LinePlan};
use std::collections::HashSet;

impl TypeChecker<'_> {
    pub(super) fn check_line_plan_output_type(&mut self, plan: &LinePlan) -> Option<TypeKind> {
        self.with_line_runtime_scope(|checker| {
            checker
                .line_label_stack
                .push(plan.label().map(str::to_owned));
            let mut output = None;
            for item in plan.items() {
                if let Some(item_output) = checker.check_line_plan_item(item) {
                    output = Some(match output {
                        Some(current) => {
                            merge_line_output(current, &item_output, &mut checker.errors)
                        }
                        None => item_output,
                    });
                }
            }
            checker.line_label_stack.pop();
            output
        })
    }

    pub(super) fn check_line_plan_item(&mut self, item: &LinePlanItem) -> Option<TypeKind> {
        match item {
            LinePlanItem::Init(statements)
            | LinePlanItem::Stmt(Stmt::DeferBlock { statements, .. }) => self
                .with_child_task_scope(false, |checker| {
                    checker.check_line_plan_statements(statements)
                }),
            LinePlanItem::Thread(thread) => self.check_thread_body(thread.body()),
            LinePlanItem::Stmt(stmt) => {
                if matches!(stmt, Stmt::Yield(_)) {
                    self.errors.push(TypeCheckError::new(
                        "`yield` cannot be used in a dialogue line plan; use `out` for line results"
                            .to_owned(),
                    ));
                    return None;
                }
                self.check_stmt(stmt);
                None
            }
            LinePlanItem::On { trigger, body } => {
                self.check_line_on_trigger(trigger);
                self.with_child_task_scope(false, |checker| {
                    for stmt in body {
                        checker.check_stmt(stmt);
                    }
                });
                None
            }
            LinePlanItem::Option { value, .. } => {
                self.check_expr(value);
                None
            }
            LinePlanItem::Let { pattern, expr } => {
                let ty = self.check_expr(expr);
                for (name, ty) in let_else_bindings(pattern, ty.as_ref()) {
                    self.locals.insert(name, ty);
                }
                None
            }
            LinePlanItem::Out(value) => self.check_expr(value),
            LinePlanItem::TimedCue { anchor, body } => {
                self.expect_expr_type(anchor, &TypeKind::Duration, "timeline anchor");
                self.with_child_task_scope(false, |checker| {
                    checker.check_expr(body);
                });
                None
            }
            LinePlanItem::CancelRule(rule) => self.check_line_plan_cancel_rule(rule),
            LinePlanItem::Assert { expr, .. } => {
                self.expect_expr_type(expr, &TypeKind::Bool, "line-plan assertion");
                None
            }
            LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
                self.with_child_task_scope(false, |checker| checker.check_line_plan_group(items))
            }
            LinePlanItem::Memo { options, .. } => {
                for (_, value) in options {
                    self.check_expr(value);
                }
                None
            }
            LinePlanItem::Expr(expr) => {
                self.check_expr(expr);
                None
            }
            LinePlanItem::Raw(raw) => {
                self.errors.push(TypeCheckError::new(format!(
                    "raw line-plan item is not type-checkable: {raw}"
                )));
                None
            }
        }
    }

    fn check_line_plan_cancel_rule(&mut self, rule: &CancelRuleSyntax) -> Option<TypeKind> {
        self.line_cancel_depth += 1;
        let output = self.with_child_task_scope(false, |checker| {
            let mut output = None;
            for stmt in rule.action() {
                let stmt_output = if let Stmt::Out { label, expr } = stmt {
                    checker.check_out_stmt(label.as_deref(), expr)
                } else {
                    checker.check_stmt(stmt);
                    None
                };
                checker.merge_optional_line_output(&mut output, stmt_output);
            }
            output
        });
        self.line_cancel_depth -= 1;
        output
    }

    fn check_line_plan_group(&mut self, items: &[LinePlanItem]) -> Option<TypeKind> {
        let mut output = None;
        for item in items {
            let item_output = self.check_line_plan_item(item);
            self.merge_optional_line_output(&mut output, item_output);
        }
        output
    }

    fn merge_optional_line_output(
        &mut self,
        output: &mut Option<TypeKind>,
        next: Option<TypeKind>,
    ) {
        if let Some(next) = next {
            *output = Some(match output.take() {
                Some(current) => merge_line_output(current, &next, &mut self.errors),
                None => next,
            });
        }
    }

    fn check_line_plan_statements(&mut self, statements: &[Stmt]) -> Option<TypeKind> {
        for stmt in statements {
            self.check_stmt(stmt);
        }
        None
    }

    pub(super) fn check_thread_body(&mut self, statements: &[Stmt]) -> Option<TypeKind> {
        self.reject_active_borrows("thread boundary");
        self.with_child_task_scope(true, |checker| {
            checker.check_line_plan_statements(statements)
        })
    }

    fn check_line_on_trigger(&mut self, trigger: &TriggerPattern) {
        match trigger {
            TriggerPattern::Mark(pattern) => {
                if let Pattern::Variant { name, .. } | Pattern::Ident(name) = pattern {
                    let mark = if name.starts_with('.') {
                        name.clone()
                    } else {
                        format!(".{name}")
                    };
                    if !self
                        .line_mark_stack
                        .last()
                        .is_some_and(|marks| marks.contains(&mark))
                    {
                        self.errors.push(TypeCheckError::new(format!(
                            "line-local handler trigger `{mark}` does not name a `[mark {mark}]` in this dialogue line"
                        )));
                    }
                }
            }
            TriggerPattern::Signal { target, .. }
            | TriggerPattern::Timeout(target)
            | TriggerPattern::Expr(target) => {
                self.check_expr(target);
            }
            TriggerPattern::Input(_)
            | TriggerPattern::Event(_)
            | TriggerPattern::Select(_)
            | TriggerPattern::Task(_)
            | TriggerPattern::Scope(_) => {}
        }
    }

    pub(super) fn check_dialogue_content(&mut self, tokens: &[DialogueToken]) -> HashSet<String> {
        let mut marks = HashSet::new();
        for token in tokens {
            match token {
                DialogueToken::Expr(expr) => {
                    self.check_expr(expr);
                }
                DialogueToken::Mark(mark) => {
                    if !marks.insert(mark.name().to_owned()) {
                        self.errors.push(TypeCheckError::new(format!(
                            "duplicate dialogue mark `{}` in line content",
                            mark.name()
                        )));
                    }
                }
                DialogueToken::Tag(tag) if tag.name() == "hook" => {
                    self.errors.push(TypeCheckError::new(
                        "local dialogue `[hook ...]` syntax was removed; use `[mark .name]` with `with: on .name:`".to_owned(),
                    ));
                }
                DialogueToken::Tag(_)
                | DialogueToken::Text(_)
                | DialogueToken::Raw(_)
                | DialogueToken::EndTag(_)
                | DialogueToken::Ruby { .. }
                | DialogueToken::Escape(_) => {}
            }
        }
        marks
    }

    pub(super) fn check_lifetime_set_stmt(&mut self, target: &Expr, expr: &Expr) {
        let value_ty = self.check_expr(expr);
        let Some(key) = lifetime_key(target) else {
            self.errors.push(TypeCheckError::new(
                "lifetime registry assignment target must be `'scope.key`".to_owned(),
            ));
            self.check_expr(target);
            return;
        };
        self.check_lifetime_access(&key, LifetimeAccessMode::Write);
        if !matches!(
            key.scope(),
            LifetimeScopeKind::Line | LifetimeScopeKind::Cue
        ) {
            self.reject_borrow_escape(value_ty.as_ref(), "upper lifetime registry write");
        }
        self.lifetime_guarantees.insert(key);
    }

    pub(super) fn check_wait_stmt(&mut self, target: &WaitTarget) {
        match target {
            WaitTarget::Duration(expr) => {
                self.expect_expr_type(expr, &TypeKind::Duration, "wait duration");
            }
            WaitTarget::Mark(name) => {
                if !self
                    .line_mark_stack
                    .last()
                    .is_some_and(|marks| marks.contains(name))
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "wait mark `{name}` does not name a mark in this dialogue line"
                    )));
                }
            }
            WaitTarget::Expr(expr) => {
                self.check_expr(expr);
            }
        }
    }
}
