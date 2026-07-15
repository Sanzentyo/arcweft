//! Dialogue line-plan type checking.

use super::{
    CallArg, CancelRuleSyntax, DialogueContent, DialogueToken, Expr, LifetimeAccessMode,
    LifetimeScopeKind, LinePlanItem, Pattern, Stmt, SuspensionBoundary, TriggerPattern,
    TypeCheckError, TypeChecker, TypeKind, lifetime_key, merge_line_output,
};
use super::{fx::FxSpanState, helpers::let_else_bindings};
use arcweft_lang_syntax::ast::{
    flow::{AuthoredExpr, FlowItem, WaitTarget},
    line_plan::LinePlan,
};
use arcweft_lang_syntax::expr::parse_expr;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DialogueContentRangeMode {
    /// Project inline-expression ranges through the content's document map.
    ContentSourceMap,
    /// The owning expression traversal already registered document ranges.
    PreRegisteredExpression,
}

impl TypeChecker<'_> {
    pub(super) fn check_line_plan_output_type(&mut self, plan: &LinePlan) -> Option<TypeKind> {
        self.with_line_runtime_scope(|checker| {
            checker.line_out_depth += 1;
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
            checker.line_out_depth -= 1;
            output
        })
    }

    pub(super) fn check_line_plan_item(&mut self, item: &LinePlanItem) -> Option<TypeKind> {
        match item {
            LinePlanItem::Init(statements) => self.with_child_task_scope(false, |checker| {
                checker.check_line_plan_statements(statements)
            }),
            LinePlanItem::Thread(thread) => self.check_thread_body(thread.body()),
            LinePlanItem::Stmt(stmt) => {
                if let Stmt::DeferBlock { statements, .. } = stmt.as_ref() {
                    return self.with_child_task_scope(false, |checker| {
                        checker.check_line_plan_statements(statements)
                    });
                }
                if matches!(stmt.as_ref(), Stmt::Yield(_)) {
                    self.errors.push(TypeCheckError::new(
                        "`yield` cannot be used in a dialogue line plan; use `out` for line results"
                            .to_owned(),
                    ));
                    return None;
                }
                self.check_stmt(stmt.as_ref());
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
                let ty = self.check_line_plan_let_expr(expr);
                for (name, ty) in let_else_bindings(pattern, ty.as_ref()) {
                    self.bind_local(name, ty);
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
            LinePlanItem::TimelineAssert(assertion) => {
                self.expect_expr_type(
                    assertion.condition(),
                    &TypeKind::Bool,
                    "line-plan assertion",
                );
                None
            }
            LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
                self.with_child_task_scope(false, |checker| checker.check_line_plan_group(items))
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

    fn check_line_plan_let_expr(&mut self, expr: &Expr) -> Option<TypeKind> {
        let Expr::NamedBlock {
            name,
            statements,
            value,
        } = expr
        else {
            return self.check_expr(expr);
        };
        let Some(anchor) = parse_timed_cue_block_anchor(name, &mut self.errors) else {
            return self.check_expr(expr);
        };
        self.expect_expr_type(&anchor, &TypeKind::Duration, "timeline anchor");
        self.with_child_task_scope(false, |checker| {
            checker.check_block_expr(statements, value.as_deref());
        });
        Some(TypeKind::Named("CueHandle".to_owned()))
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

    pub(super) fn check_thread_body(&mut self, statements: &[FlowItem]) -> Option<TypeKind> {
        self.reject_active_borrows(SuspensionBoundary::Thread);
        self.with_child_task_scope(true, |checker| {
            for item in statements {
                match item {
                    FlowItem::Stmt(stmt) => checker.check_stmt(stmt),
                    other => checker.errors.push(TypeCheckError::new(format!(
                        "thread body item is not valid in this statement context: {other:?}"
                    ))),
                }
            }
            None
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

    pub(super) fn check_dialogue_content(
        &mut self,
        content: &DialogueContent,
        has_default_inline_failure_policy: bool,
        range_mode: DialogueContentRangeMode,
    ) -> HashSet<String> {
        let mut marks = HashSet::new();
        let mut fx_spans = FxSpanState::default();
        if range_mode == DialogueContentRangeMode::PreRegisteredExpression {
            self.errors
                .extend(content.diagnostics().iter().map(|diagnostic| {
                    TypeCheckError::new(format!(
                        "invalid dialogue content: {}; {}",
                        diagnostic.message(),
                        diagnostic.recovery()
                    ))
                }));
        }
        for token in content.tokens() {
            fx_spans.observe(token, &self.fx, &mut self.errors);
            match token {
                DialogueToken::Expr(expr_token) => {
                    if range_mode == DialogueContentRangeMode::ContentSourceMap {
                        self.register_projected_expr_source_ranges(
                            expr_token.expr(),
                            Some(expr_token.source()),
                            Some(expr_token.range()),
                            |range| content.source_range(range),
                        );
                    }
                    let expr_ty = self.check_expr(expr_token.expr());
                    reject_fallible_inline_value_without_failure_policy(
                        expr_token.expr(),
                        expr_ty.as_ref(),
                        has_default_inline_failure_policy,
                        &mut self.errors,
                    );
                }
                DialogueToken::Mark(mark) => {
                    if !marks.insert(mark.name().to_owned()) {
                        self.errors.push(TypeCheckError::new(format!(
                            "duplicate dialogue mark `{}` in line content",
                            mark.name()
                        )));
                    }
                }
                DialogueToken::InferredTag(tag) if self.fx.inferred_tag_is_mark(tag) => {
                    if !marks.insert(tag.name().to_owned()) {
                        self.errors.push(TypeCheckError::new(format!(
                            "duplicate dialogue mark `{}` in line content",
                            tag.name()
                        )));
                    }
                }
                DialogueToken::Tag(tag) if tag.name() == "w" => {
                    if let Err(error) = tag.wait_duration() {
                        self.errors.push(TypeCheckError::new(error.to_string()));
                    }
                }
                DialogueToken::Tag(tag) if tag.name() == "speed" => {
                    if let Err(error) = tag.reveal_speed() {
                        self.errors.push(TypeCheckError::new(error.to_string()));
                    }
                }
                DialogueToken::Tag(tag)
                    if matches!(
                        tag.name(),
                        "p" | "l" | "r" | "clear" | "er" | "cm" | "reset"
                    ) && !tag.attrs().trim().is_empty() =>
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "dialogue control `[{}]` does not accept attributes",
                        tag.name()
                    )));
                }
                DialogueToken::Tag(_)
                | DialogueToken::Text(_)
                | DialogueToken::Raw(_)
                | DialogueToken::EndTag(_)
                | DialogueToken::InferredTag(_)
                | DialogueToken::InferredEndTag
                | DialogueToken::Ruby { .. }
                | DialogueToken::Escape(_) => {}
            }
        }
        fx_spans.finish(&mut self.errors);
        marks
    }

    pub(super) fn check_lifetime_set_stmt(&mut self, target: &AuthoredExpr, expr: &AuthoredExpr) {
        let value_ty = self.check_authored_expr(expr);
        let Some(key) = lifetime_key(target.expr()) else {
            self.errors.push(TypeCheckError::new(
                "lifetime registry assignment target must be `'scope.key`".to_owned(),
            ));
            self.check_authored_expr(target);
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
                self.expect_authored_expr_type(expr, &TypeKind::Duration, "wait duration");
            }
            WaitTarget::Expr(expr) => {
                if let Some(name) = wait_mark_name(expr.expr()) {
                    if !self
                        .line_mark_stack
                        .last()
                        .is_some_and(|marks| marks.contains(&name))
                    {
                        self.errors.push(TypeCheckError::new(format!(
                            "wait(mark({name})) does not name a mark in this dialogue line"
                        )));
                    }
                    return;
                }
                self.check_authored_expr(expr);
            }
        }
    }
}

fn reject_fallible_inline_value_without_failure_policy(
    expr: &Expr,
    expr_ty: Option<&TypeKind>,
    has_default_inline_failure_policy: bool,
    errors: &mut Vec<TypeCheckError>,
) {
    if !matches!(expr_ty, Some(TypeKind::DisplayText)) {
        return;
    }
    if let Expr::Call { callee, args } = expr {
        validate_inline_call_failure_policy(
            inline_callable_label(callee),
            args,
            has_default_inline_failure_policy,
            errors,
        );
    }
}

fn validate_inline_call_failure_policy(
    function: String,
    args: &[CallArg],
    has_default_inline_failure_policy: bool,
    errors: &mut Vec<TypeCheckError>,
) {
    let policy_args = args
        .iter()
        .filter_map(inline_failure_policy_arg)
        .collect::<Vec<_>>();
    if policy_args.is_empty() {
        if !has_default_inline_failure_policy {
            errors.push(TypeCheckError::inline_call_error_policy_missing(function));
        }
        return;
    }
    if policy_args.len() > 1 {
        errors.push(TypeCheckError::inline_failure_policy_conflict(
            function.clone(),
        ));
    }
    for policy_arg in policy_args {
        if let Some(policy) = unknown_inline_failure_policy(policy_arg) {
            errors.push(TypeCheckError::unknown_inline_failure_policy(
                function.clone(),
                policy,
            ));
        }
    }
}

fn inline_failure_policy_arg(arg: &CallArg) -> Option<&Expr> {
    match arg {
        CallArg::Named { name, value }
            if matches!(name.as_str(), "on_error" | "fallback" | "discard_error") =>
        {
            Some(value)
        }
        CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
    }
}

fn unknown_inline_failure_policy(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => unknown_inline_failure_atom(path),
        Expr::ShortVariant(name) => unknown_inline_failure_atom(&format!(".{name}")),
        Expr::Select(select) => match select.target() {
            Expr::Path(namespace) => {
                unknown_inline_failure_field(namespace.as_label(), select.member().as_str())
            }
            _ => None,
        },
        Expr::Call { callee, args } => unknown_inline_failure_constructor(callee, args),
        _ => None,
    }
}

fn unknown_inline_failure_constructor(callee: &Expr, args: &[CallArg]) -> Option<String> {
    let constructor = inline_failure_constructor_name(callee)?;
    if constructor != "fallback" {
        return Some(inline_callable_label(callee));
    }
    args.iter().find_map(|arg| match arg {
        CallArg::Positional(value) => unknown_inline_fallback_value(value),
        CallArg::Named { name, value } if name == "value" || name == "text" => {
            unknown_inline_fallback_value(value)
        }
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn inline_failure_constructor_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(path) if path == "fallback" => Some("fallback"),
        Expr::Select(select) if matches!(select.target(), Expr::Path(namespace) if namespace == "InlineFailure") => {
            Some(select.member().as_str())
        }
        _ => None,
    }
}

fn unknown_inline_fallback_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => unknown_inline_fallback_atom(path),
        Expr::ShortVariant(name) => unknown_inline_fallback_atom(&format!(".{name}")),
        Expr::Select(select) => match select.target() {
            Expr::Path(namespace) => {
                unknown_inline_fallback_field(namespace.as_label(), select.member().as_str())
            }
            _ => None,
        },
        _ => None,
    }
}

fn unknown_inline_failure_atom(path: &str) -> Option<String> {
    let variant = path.strip_prefix('.')?;
    (!matches!(variant, "fail" | "discard" | "line_error")).then(|| path.to_owned())
}

fn unknown_inline_failure_field(namespace: &str, field: &str) -> Option<String> {
    (namespace == "InlineFailure" && !matches!(field, "fail" | "discard" | "line_error"))
        .then(|| format!("{namespace}.{field}"))
}

fn unknown_inline_fallback_atom(path: &str) -> Option<String> {
    let variant = path.strip_prefix('.')?;
    (!matches!(variant, "expr_source" | "call_source" | "value_plain")).then(|| path.to_owned())
}

fn unknown_inline_fallback_field(namespace: &str, field: &str) -> Option<String> {
    (namespace == "InlineFallback"
        && !matches!(field, "expr_source" | "call_source" | "value_plain"))
    .then(|| format!("{namespace}.{field}"))
}

fn parse_timed_cue_block_anchor(name: &str, errors: &mut Vec<TypeCheckError>) -> Option<Expr> {
    let anchor = name.strip_prefix("at(")?.strip_suffix(')')?.trim();
    if anchor.is_empty() {
        errors.push(TypeCheckError::new(
            "timed cue anchor cannot be empty".to_owned(),
        ));
        return None;
    }
    match parse_expr(anchor) {
        Ok(anchor) => Some(anchor),
        Err(error) => {
            errors.push(TypeCheckError::new(format!(
                "timed cue anchor is not a valid expression: {error}"
            )));
            None
        }
    }
}

fn inline_callable_label(expr: &Expr) -> String {
    match expr {
        Expr::Path(path) => path.as_label().to_owned(),
        Expr::ShortVariant(name) => format!(".{name}"),
        Expr::Select(select) => format!(
            "{}.{}",
            inline_callable_label(select.target()),
            select.member().as_str()
        ),
        _ => format!("{expr:?}"),
    }
}

fn wait_mark_name(expr: &Expr) -> Option<String> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    if !matches!(callee.as_ref(), Expr::Path(path) if path == "mark") || args.len() != 1 {
        return None;
    }
    match args[0].value() {
        Expr::Path(path) => Some(path.as_label().to_owned()),
        Expr::ShortVariant(name) => Some(format!(".{name}")),
        Expr::EntityRef(entity) => Some(entity.body().to_owned()),
        _ => None,
    }
}
