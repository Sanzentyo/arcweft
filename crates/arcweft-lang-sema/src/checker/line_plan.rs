//! Dialogue line-plan type checking.

use super::helpers::let_else_bindings;
use super::{
    CallArg, CancelRuleSyntax, DialogueToken, Expr, LifetimeAccessMode, LifetimeScopeKind,
    LinePlanItem, Pattern, Stmt, TriggerPattern, TypeCheckError, TypeChecker, TypeKind,
    lifetime_key, merge_line_output,
};
use arcweft_lang_syntax::ast::{
    flow::{FlowItem, WaitTarget},
    line_plan::LinePlan,
};
use std::collections::HashSet;

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
            LinePlanItem::Assert { expr, .. } => {
                self.expect_expr_type(expr, &TypeKind::Bool, "line-plan assertion");
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
        self.reject_active_borrows("thread boundary");
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
        tokens: &[DialogueToken],
        has_default_inline_failure_policy: bool,
    ) -> HashSet<String> {
        let mut marks = HashSet::new();
        for token in tokens {
            match token {
                DialogueToken::Expr(expr) => {
                    self.check_expr(expr);
                    reject_inline_calls_without_failure_policy(
                        expr,
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
                DialogueToken::InferredTag(tag) if inferred_tag_is_mark(tag.name()) => {
                    if !marks.insert(tag.name().to_owned()) {
                        self.errors.push(TypeCheckError::new(format!(
                            "duplicate dialogue mark `{}` in line content",
                            tag.name()
                        )));
                    }
                }
                DialogueToken::Tag(tag) if tag.name() == "hook" => {
                    self.errors.push(TypeCheckError::new(
                        "local dialogue `[hook ...]` syntax was removed; use `[mark .name]` with `with: on mark(.name):`".to_owned(),
                    ));
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
            WaitTarget::Expr(expr) => {
                if let Some(name) = wait_mark_name(expr) {
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
                self.check_expr(expr);
            }
        }
    }
}

fn inferred_tag_is_mark(name: &str) -> bool {
    !matches!(
        name.trim_start_matches('.'),
        "italic"
            | "oblique"
            | "horizontal_tb"
            | "vertical_rl"
            | "vertical_lr"
            | "dir"
            | "ruby_over"
            | "ruby_under"
            | "ruby_inter_character"
            | "offset"
            | "pos"
            | "rotate"
            | "scale"
            | "skew"
            | "wave"
            | "shake"
            | "arc"
            | "typewriter"
            | "jitter"
            | "shader"
            | "host"
    )
}

fn reject_inline_calls_without_failure_policy(
    expr: &Expr,
    has_default_inline_failure_policy: bool,
    errors: &mut Vec<TypeCheckError>,
) {
    match expr {
        Expr::Call { callee, args } => validate_inline_call_failure_policy(
            inline_callable_label(callee),
            args,
            has_default_inline_failure_policy,
            errors,
        ),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => validate_inline_call_failure_policy(
            format!("{}.{method}", inline_callable_label(receiver)),
            args,
            has_default_inline_failure_policy,
            errors,
        ),
        _ => {}
    }

    for child in inline_expr_children(expr) {
        reject_inline_calls_without_failure_policy(
            child,
            has_default_inline_failure_policy,
            errors,
        );
    }
}

fn inline_expr_children(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::Call { callee, args } => {
            let mut children = Vec::with_capacity(args.len() + 1);
            children.push(callee.as_ref());
            children.extend(args.iter().filter_map(call_arg_inline_child));
            children
        }
        Expr::MethodCall { receiver, args, .. } => {
            let mut children = Vec::with_capacity(args.len() + 1);
            children.push(receiver.as_ref());
            children.extend(args.iter().filter_map(call_arg_inline_child));
            children
        }
        Expr::Field { target, .. }
        | Expr::Try { expr: target }
        | Expr::Await {
            expr: target,
            applies_try: _,
        }
        | Expr::Unary {
            op: _,
            expr: target,
        }
        | Expr::Closure { body: target, .. }
        | Expr::DialogueCall { callee: target, .. } => vec![target.as_ref()],
        Expr::Index { target, index } => vec![target.as_ref(), index.as_ref()],
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, op: _, rhs } => {
            vec![lhs.as_ref(), rhs.as_ref()]
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => items.iter().collect(),
        Expr::ArrayRepeat { value, len } => vec![value.as_ref(), len.as_ref()],
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            fields.iter().map(|(_, value)| value).collect()
        }
        Expr::Block { value, .. }
        | Expr::ComputationBlock { value, .. }
        | Expr::MemoBlock { value, .. }
        | Expr::NamedBlock { value, .. } => value.iter().map(Box::as_ref).collect(),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => optional_tail(
            vec![condition.as_ref(), then_branch.as_ref()],
            else_branch.as_deref(),
        ),
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => optional_tail(
            optional_tail(vec![expr.as_ref()], guard.as_deref())
                .into_iter()
                .chain([then_branch.as_ref()])
                .collect(),
            else_branch.as_deref(),
        ),
        Expr::Match { scrutinee, arms } => {
            let mut children = Vec::with_capacity(arms.len() + 1);
            children.push(scrutinee.as_ref());
            children.extend(
                arms.iter()
                    .map(arcweft_lang_syntax::expr::MatchExprArm::value),
            );
            children
        }
        Expr::Range { start, end, .. } => [start.as_deref(), end.as_deref()]
            .into_iter()
            .flatten()
            .collect(),
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::Thread { .. }
        | Expr::Raw(_) => Vec::new(),
    }
}

fn optional_tail<'a>(mut values: Vec<&'a Expr>, tail: Option<&'a Expr>) -> Vec<&'a Expr> {
    if let Some(tail) = tail {
        values.push(tail);
    }
    values
}

fn call_arg_expr(arg: &CallArg) -> &Expr {
    match arg {
        CallArg::Positional(value) => value,
        CallArg::Named { value, .. } | CallArg::Spread { value } => value,
    }
}

fn call_arg_inline_child(arg: &CallArg) -> Option<&Expr> {
    match arg {
        CallArg::Named { name, .. }
            if matches!(name.as_str(), "on_error" | "fallback" | "discard_error") =>
        {
            None
        }
        _ => Some(call_arg_expr(arg)),
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
        Expr::Field { target, field } => match target.as_ref() {
            Expr::Path(namespace) => unknown_inline_failure_field(namespace, field),
            _ => None,
        },
        Expr::Call { callee, args } => unknown_inline_failure_constructor(callee, args),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => unknown_inline_failure_method_constructor(receiver, method, args),
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

fn unknown_inline_failure_method_constructor(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Option<String> {
    if !matches!(receiver, Expr::Path(namespace) if namespace == "InlineFailure") {
        return None;
    }
    if method != "fallback" {
        return Some(format!("InlineFailure.{method}"));
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
        Expr::Field { target, field } if matches!(target.as_ref(), Expr::Path(namespace) if namespace == "InlineFailure") => {
            Some(field)
        }
        _ => None,
    }
}

fn unknown_inline_fallback_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => unknown_inline_fallback_atom(path),
        Expr::Field { target, field } => match target.as_ref() {
            Expr::Path(namespace) => unknown_inline_fallback_field(namespace, field),
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

fn inline_callable_label(expr: &Expr) -> String {
    match expr {
        Expr::Path(path) => path.clone(),
        Expr::Field { target, field } => format!("{}.{field}", inline_callable_label(target)),
        Expr::MethodCall {
            receiver, method, ..
        } => format!("{}.{method}", inline_callable_label(receiver)),
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
        Expr::Path(path) => Some(path.clone()),
        Expr::EntityRef(entity) => Some(entity.body().to_owned()),
        _ => None,
    }
}
