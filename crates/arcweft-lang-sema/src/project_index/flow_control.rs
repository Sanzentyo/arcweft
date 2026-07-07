use super::{
    CallArg, ChoiceAction, Expr, HirFlowItem, MatchExprArm, ProjectFlowControlSummary, Stmt,
    StmtMatchArm,
};

pub(super) fn summarize_flow_control_items(items: &[HirFlowItem]) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    for item in items {
        summary.merge(summarize_flow_control_item(item));
    }
    summary
}

fn summarize_flow_control_item(item: &HirFlowItem) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    match item {
        HirFlowItem::Dialogue(dialogue) => {
            for arg in dialogue.args() {
                summary.merge(summarize_expr_control(arg.value()));
            }
            if let Some(rich_text) = dialogue.rich_text() {
                summary.merge(summarize_expr_control(rich_text));
            }
        }
        HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
            summary.record_branch();
            for option in choice.options() {
                if let Some(condition) = option.condition() {
                    summary.merge(summarize_expr_control(condition));
                }
                if let Some(value) = option.value() {
                    summary.merge(summarize_expr_control(value));
                }
                summary.merge(match option.action() {
                    ChoiceAction::Out(expr) => summarize_expr_control(expr),
                    ChoiceAction::SelectBlock(statements) => {
                        summarize_stmt_body_control(statements)
                    }
                    ChoiceAction::Goto(_) | ChoiceAction::None => {
                        ProjectFlowControlSummary::default()
                    }
                });
            }
        }
        HirFlowItem::If(block) => {
            summary.record_branch();
            summary.merge(summarize_flow_control_items(block.body()));
            summary.merge(summarize_flow_control_items(block.else_body()));
        }
        HirFlowItem::IfLet(block) => {
            summary.record_branch();
            summary.merge(summarize_flow_control_items(block.body()));
            summary.merge(summarize_flow_control_items(block.else_body()));
        }
        HirFlowItem::Match(block) => {
            summary.record_branch();
            for arm in block.arms() {
                summary.merge(summarize_flow_control_items(arm.body()));
            }
        }
        HirFlowItem::Loop(block) | HirFlowItem::LetLoop { block, .. } => {
            summary.record_loop();
            summary.merge(summarize_flow_control_items(block.body()));
        }
        HirFlowItem::While(block) => {
            summary.record_loop();
            summary.merge(summarize_flow_control_items(block.body()));
        }
        HirFlowItem::WhileLet(block) => {
            summary.record_loop();
            summary.merge(summarize_flow_control_items(block.body()));
        }
        HirFlowItem::For(block) => {
            summary.record_loop();
            summary.merge(summarize_flow_control_items(block.body()));
        }
        HirFlowItem::Borrow(block) => {
            summary.merge(summarize_flow_control_items(block.body()));
        }
        HirFlowItem::SourceLocale(block) => {
            summary.merge(summarize_flow_control_items(block.body()));
        }
        HirFlowItem::Scope(block) => {
            summary.merge(summarize_flow_control_items(block.body()));
        }
        HirFlowItem::Select(block) => {
            summary.record_branch();
            summary.add_select_branches(block.branches().len());
            for branch in block.branches() {
                summary.merge(summarize_flow_control_items(branch.body()));
            }
        }
        HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
            summary.record_await();
            for branch in await_with.branches() {
                summary.merge(summarize_flow_control_items(branch.body()));
            }
        }
        HirFlowItem::Thread(thread) => {
            summary.record_thread();
            summary.merge(summarize_flow_control_items(thread.body()));
        }
        HirFlowItem::LetScope { scope, .. } => {
            summary.merge(summarize_stmt_body_control(scope.statements()));
            if let Some(value) = scope.value() {
                summary.merge(summarize_expr_control(value));
            }
        }
        HirFlowItem::Stmt(stmt) => {
            summary.merge(summarize_stmt_control(stmt));
        }
        HirFlowItem::Include(_) => {}
    }
    summary
}

fn summarize_stmt_body_control(statements: &[Stmt]) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    for statement in statements {
        summary.merge(summarize_stmt_control(statement));
    }
    summary
}

fn summarize_stmt_control(stmt: &Stmt) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    match stmt {
        Stmt::Goto(Expr::EntityRef(target)) if target.as_absolute().is_some() => {
            summary.record_static_goto();
        }
        Stmt::Goto(expr) => {
            summary.record_dynamic_goto();
            summary.merge(summarize_expr_control(expr));
        }
        Stmt::Let { expr, .. }
        | Stmt::Return(expr)
        | Stmt::Out { expr, .. }
        | Stmt::Defer { expr, .. }
        | Stmt::Yield(expr)
        | Stmt::LifetimeSet { expr, .. }
        | Stmt::Close(expr)
        | Stmt::Select(expr)
        | Stmt::Expr(expr) => {
            summary.merge(summarize_expr_control(expr));
        }
        Stmt::Assign { target, expr } => {
            summary.merge(summarize_expr_control(target));
            summary.merge(summarize_expr_control(expr));
        }
        Stmt::Signal { target, value } => {
            summary.merge(summarize_expr_control(target));
            summary.merge(summarize_expr_control(value));
        }
        Stmt::LetElse {
            expr, else_body, ..
        } => {
            summary.record_branch();
            summary.merge(summarize_expr_control(expr));
            summary.merge(summarize_stmt_body_control(else_body));
        }
        Stmt::LetActionReceive { action, .. } => {
            summary.merge(summarize_await_expr_control(action));
        }
        Stmt::DeferBlock { statements, .. } => {
            summary.merge(summarize_stmt_body_control(statements));
        }
        Stmt::On { body, .. } | Stmt::UnsafeLifetime { body, .. } => {
            summary.merge(summarize_stmt_body_control(body));
        }
        Stmt::Loop { body } => {
            summary.record_loop();
            summary.merge(summarize_stmt_body_control(body));
        }
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            summary.record_branch();
            summary.merge(summarize_expr_control(condition));
            summary.merge(summarize_stmt_body_control(body));
            summary.merge(summarize_stmt_body_control(else_body));
        }
        Stmt::While { condition, body } => {
            summary.record_loop();
            summary.merge(summarize_expr_control(condition));
            summary.merge(summarize_stmt_body_control(body));
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            summary.record_loop();
            summary.merge(summarize_expr_control(expr));
            if let Some(guard) = guard {
                summary.merge(summarize_expr_control(guard));
            }
            summary.merge(summarize_stmt_body_control(body));
        }
        Stmt::For { source, body, .. } => {
            summary.record_loop();
            summary.merge(summarize_expr_control(source));
            summary.merge(summarize_stmt_body_control(body));
        }
        Stmt::Match { arms, .. } => {
            summary.record_branch();
            summary.merge(summarize_stmt_match_control(arms));
        }
        Stmt::Thread(_) => {
            summary.record_thread();
        }
        Stmt::Wait(_) => {
            summary.record_await();
        }
        Stmt::LetChoice { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => {}
    }
    summary
}

fn summarize_await_expr_control(expr: &Expr) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    summary.record_await();
    summary.merge(summarize_expr_control(expr));
    summary
}

fn summarize_stmt_match_control(arms: &[StmtMatchArm]) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    for arm in arms {
        if let Some(guard) = arm.guard() {
            summary.merge(summarize_expr_control(guard));
        }
        summary.merge(summarize_stmt_body_control(arm.body()));
    }
    summary
}

fn summarize_expr_control(expr: &Expr) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    match expr {
        Expr::Call { callee, args } => {
            summary.merge(summarize_expr_call_control(callee, args));
        }
        Expr::Select(select) => summary.merge(summarize_expr_control(select.target())),
        Expr::Try { expr: target }
        | Expr::Await { expr: target, .. }
        | Expr::Unary { expr: target, .. }
        | Expr::DialogueCall { callee: target, .. }
        | Expr::Closure { body: target, .. } => summary.merge(summarize_expr_control(target)),
        Expr::Index {
            target,
            index: item,
        }
        | Expr::Pipe {
            lhs: target,
            rhs: item,
        }
        | Expr::Binary {
            lhs: target,
            rhs: item,
            ..
        }
        | Expr::ArrayRepeat {
            value: target,
            len: item,
        } => {
            summary.merge(summarize_expr_pair_control(target, item));
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            summary.merge(summarize_expr_items_control(items));
        }
        Expr::Record { fields: _, .. } | Expr::RecordLiteral(_) => {
            summary.merge(summarize_expr_record_control(expr));
        }
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => {
            summary.merge(summarize_expr_block_control(statements, value.as_deref()));
        }
        Expr::MemoBlock {
            options,
            statements,
            value,
        } => {
            summary.merge(summarize_expr_memo_block_control(
                options,
                statements,
                value.as_deref(),
            ));
        }
        Expr::If { .. } | Expr::IfLet { .. } | Expr::Match { .. } => {
            summary.merge(summarize_expr_branch_control(expr));
        }
        Expr::Range { start, end, .. } => {
            summary.merge(summarize_expr_range_control(
                start.as_deref(),
                end.as_deref(),
            ));
        }
        Expr::Thread { .. } => summary.record_thread(),
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::Raw(_) => {}
    }
    summary
}

fn summarize_expr_call_control(callee: &Expr, args: &[CallArg]) -> ProjectFlowControlSummary {
    let mut summary = summarize_expr_control(callee);
    summary.merge(summarize_call_args_control(args));
    summary
}

fn summarize_expr_pair_control(lhs: &Expr, rhs: &Expr) -> ProjectFlowControlSummary {
    let mut summary = summarize_expr_control(lhs);
    summary.merge(summarize_expr_control(rhs));
    summary
}

fn summarize_expr_items_control(items: &[Expr]) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    for item in items {
        summary.merge(summarize_expr_control(item));
    }
    summary
}

fn summarize_expr_record_control(expr: &Expr) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    match expr {
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            for (_, value) in fields {
                summary.merge(summarize_expr_control(value));
            }
        }
        _ => {}
    }
    summary
}

fn summarize_expr_block_control(
    statements: &[Stmt],
    value: Option<&Expr>,
) -> ProjectFlowControlSummary {
    let mut summary = summarize_stmt_body_control(statements);
    if let Some(value) = value {
        summary.merge(summarize_expr_control(value));
    }
    summary
}

fn summarize_expr_memo_block_control(
    options: &[(String, Expr)],
    statements: &[Stmt],
    value: Option<&Expr>,
) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    for (_, value) in options {
        summary.merge(summarize_expr_control(value));
    }
    summary.merge(summarize_stmt_body_control(statements));
    if let Some(value) = value {
        summary.merge(summarize_expr_control(value));
    }
    summary
}

fn summarize_expr_branch_control(expr: &Expr) -> ProjectFlowControlSummary {
    match expr {
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => summarize_expr_if_control(condition, then_branch, else_branch.as_deref()),
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => summarize_expr_if_let_control(
            expr,
            guard.as_deref(),
            then_branch,
            else_branch.as_deref(),
        ),
        Expr::Match { scrutinee, arms } => summarize_expr_match_control(scrutinee, arms),
        _ => ProjectFlowControlSummary::default(),
    }
}

fn summarize_expr_if_control(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    summary.record_branch();
    summary.merge(summarize_expr_control(condition));
    summary.merge(summarize_expr_control(then_branch));
    if let Some(else_branch) = else_branch {
        summary.merge(summarize_expr_control(else_branch));
    }
    summary
}

fn summarize_expr_if_let_control(
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    summary.record_branch();
    summary.merge(summarize_expr_control(expr));
    if let Some(guard) = guard {
        summary.merge(summarize_expr_control(guard));
    }
    summary.merge(summarize_expr_control(then_branch));
    if let Some(else_branch) = else_branch {
        summary.merge(summarize_expr_control(else_branch));
    }
    summary
}

fn summarize_expr_match_control(
    scrutinee: &Expr,
    arms: &[MatchExprArm],
) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    summary.record_branch();
    summary.merge(summarize_expr_control(scrutinee));
    for arm in arms {
        if let Some(guard) = arm.guard() {
            summary.merge(summarize_expr_control(guard));
        }
        summary.merge(summarize_expr_control(arm.value()));
    }
    summary
}

fn summarize_expr_range_control(
    start: Option<&Expr>,
    end: Option<&Expr>,
) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    if let Some(start) = start {
        summary.merge(summarize_expr_control(start));
    }
    if let Some(end) = end {
        summary.merge(summarize_expr_control(end));
    }
    summary
}

fn summarize_call_args_control(args: &[CallArg]) -> ProjectFlowControlSummary {
    let mut summary = ProjectFlowControlSummary::default();
    for arg in args {
        let value = match arg {
            CallArg::Positional(value) => value,
            CallArg::Named { value, .. } | CallArg::Spread { value } => value,
        };
        summary.merge(summarize_expr_control(value));
    }
    summary
}
