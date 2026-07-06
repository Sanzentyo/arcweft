use arcweft_lang_syntax::{
    ast::{
        choice::{ChoiceAction, ChoiceItem},
        flow::{AwaitBranch, FlowItem, Stmt},
        items::{EntityDeclKind, Item},
        line_plan::LinePlanItem,
        pattern::Pattern,
    },
    expr::Expr,
    source::ParsedSource,
};
use std::collections::BTreeSet;

use crate::util::is_identifier;

pub(crate) fn collect_character_aliases(parsed: &ParsedSource) -> BTreeSet<String> {
    parsed
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::EntityDecl(entity) if entity.kind() == EntityDeclKind::Character => {
                entity.surface_alias().map(str::to_owned)
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn collect_speaker_preset_locals_from_typed_tree(
    parsed: &ParsedSource,
    character_aliases: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut presets = BTreeSet::new();
    for item in parsed.typed_tree().items() {
        collect_speaker_presets_from_item(item, character_aliases, &mut presets);
    }
    presets
}

fn collect_speaker_presets_from_item(
    item: &Item,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match item {
        Item::Flow(flow) => {
            collect_speaker_presets_from_flow_items(flow.body(), character_aliases, presets);
        }
        Item::Function(function) => {
            collect_speaker_presets_from_stmts(
                function.body_statements(),
                character_aliases,
                presets,
            );
            if let Some(value) = function.body_value() {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        Item::MemoFn(memo) => {
            collect_speaker_presets_from_stmts(memo.body_statements(), character_aliases, presets);
            if let Some(value) = memo.body_value() {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        Item::Parser(parser) => {
            collect_speaker_presets_from_stmts(
                parser.body_statements(),
                character_aliases,
                presets,
            );
            if let Some(value) = parser.body_value() {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        Item::Source(source) => {
            for handler in source.handlers() {
                collect_speaker_presets_from_stmts(handler.body(), character_aliases, presets);
            }
        }
        Item::FlowItem(item) => {
            collect_speaker_presets_from_flow_item(item, character_aliases, presets);
        }
        _ => {}
    }
}

fn collect_speaker_presets_from_flow_items(
    items: &[FlowItem],
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    for item in items {
        collect_speaker_presets_from_flow_item(item, character_aliases, presets);
    }
}

fn collect_speaker_presets_from_flow_item(
    item: &FlowItem,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match item {
        FlowItem::Stmt(stmt) => collect_speaker_presets_from_stmt(stmt, character_aliases, presets),
        FlowItem::If(block) => {
            collect_speaker_presets_from_expr(block.condition(), character_aliases, presets);
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::IfLet(block) => {
            collect_speaker_presets_from_expr(block.expr(), character_aliases, presets);
            if let Some(guard) = block.guard() {
                collect_speaker_presets_from_expr(guard, character_aliases, presets);
            }
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::Match(block) => {
            collect_speaker_presets_from_expr(block.expr(), character_aliases, presets);
            for arm in block.arms() {
                if let Some(guard) = arm.guard() {
                    collect_speaker_presets_from_expr(guard, character_aliases, presets);
                }
                collect_speaker_presets_from_flow_items(arm.body(), character_aliases, presets);
            }
        }
        FlowItem::Loop(block) => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::While(block) => {
            collect_speaker_presets_from_expr(block.condition(), character_aliases, presets);
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::WhileLet(block) => {
            collect_speaker_presets_from_expr(block.expr(), character_aliases, presets);
            if let Some(guard) = block.guard() {
                collect_speaker_presets_from_expr(guard, character_aliases, presets);
            }
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::For(block) => {
            collect_speaker_presets_from_expr(block.source(), character_aliases, presets);
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::Select(block) => {
            for branch in block.branches() {
                collect_speaker_presets_from_flow_items(branch.body(), character_aliases, presets);
            }
        }
        FlowItem::BorrowBlock(block) => {
            collect_speaker_presets_from_expr(block.source(), character_aliases, presets);
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::SourceLocale(block) => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::Scope(block) => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::AwaitWith(await_with) => {
            collect_speaker_presets_from_expr(await_with.expr(), character_aliases, presets);
            for branch in await_with.branches() {
                collect_speaker_presets_from_await_branch(branch, character_aliases, presets);
            }
        }
        FlowItem::Choice(choice) => {
            collect_speaker_presets_from_choice_items(choice.items(), character_aliases, presets);
            if let Some(plan) = choice.plan() {
                for item in plan.items() {
                    collect_speaker_presets_from_choice_plan_item(item, character_aliases, presets);
                }
            }
        }
        FlowItem::SpeakerLine(line) => {
            if let Some(plan) = line.plan() {
                for item in plan.items() {
                    collect_speaker_presets_from_line_plan_item(item, character_aliases, presets);
                }
            }
        }
        FlowItem::ContentCall(call) => {
            if let Some(plan) = call.plan() {
                for item in plan.items() {
                    collect_speaker_presets_from_line_plan_item(item, character_aliases, presets);
                }
            }
        }
        FlowItem::Include(_) | FlowItem::Raw(_) => {}
    }
}

fn collect_speaker_presets_from_await_branch(
    branch: &AwaitBranch,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    collect_speaker_presets_from_flow_items(branch.body(), character_aliases, presets);
}

fn collect_speaker_presets_from_choice_items(
    items: &[ChoiceItem],
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    for item in items {
        match item {
            ChoiceItem::Let { pattern, expr } => {
                collect_speaker_preset_binding(pattern, expr, character_aliases, presets);
            }
            ChoiceItem::If { condition, items } => {
                collect_speaker_presets_from_expr(condition, character_aliases, presets);
                collect_speaker_presets_from_choice_items(items, character_aliases, presets);
            }
            ChoiceItem::For { source, items, .. } => {
                collect_speaker_presets_from_expr(source, character_aliases, presets);
                collect_speaker_presets_from_choice_items(items, character_aliases, presets);
            }
            ChoiceItem::Match { expr, arms } => {
                collect_speaker_presets_from_expr(expr, character_aliases, presets);
                for arm in arms {
                    if let Some(guard) = arm.guard() {
                        collect_speaker_presets_from_expr(guard, character_aliases, presets);
                    }
                    collect_speaker_presets_from_choice_items(
                        arm.items(),
                        character_aliases,
                        presets,
                    );
                }
            }
            ChoiceItem::Option(option) => {
                if let Some(expr) = option.id_expr() {
                    collect_speaker_presets_from_expr(expr, character_aliases, presets);
                }
                if let Some(value) = option.value() {
                    collect_speaker_presets_from_expr(value, character_aliases, presets);
                }
                if let Some(condition) = option.condition() {
                    collect_speaker_presets_from_expr(condition, character_aliases, presets);
                }
                if let Some(visible) = option.visible() {
                    collect_speaker_presets_from_expr(visible, character_aliases, presets);
                }
                if let Some(order) = option.order() {
                    collect_speaker_presets_from_expr(order, character_aliases, presets);
                }
                if let Some(hotkey) = option.hotkey() {
                    collect_speaker_presets_from_expr(hotkey, character_aliases, presets);
                }
                for field in option.ui_fields() {
                    collect_speaker_presets_from_expr(field.value(), character_aliases, presets);
                }
                match option.action() {
                    ChoiceAction::Out(expr) => {
                        collect_speaker_presets_from_expr(expr, character_aliases, presets);
                    }
                    ChoiceAction::SelectBlock(stmts) => {
                        collect_speaker_presets_from_stmts(stmts, character_aliases, presets);
                    }
                    ChoiceAction::Goto(_) | ChoiceAction::None => {}
                }
            }
            ChoiceItem::Raw(_) => {}
        }
    }
}

fn collect_speaker_presets_from_choice_plan_item(
    item: &arcweft_lang_syntax::ast::choice::ChoicePlanItem,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match item {
        arcweft_lang_syntax::ast::choice::ChoicePlanItem::Option { value, .. } => {
            collect_speaker_presets_from_expr(value, character_aliases, presets);
        }
        arcweft_lang_syntax::ast::choice::ChoicePlanItem::Timeout { duration, body } => {
            collect_speaker_presets_from_expr(duration, character_aliases, presets);
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        arcweft_lang_syntax::ast::choice::ChoicePlanItem::Cancel { body, .. }
        | arcweft_lang_syntax::ast::choice::ChoicePlanItem::OnSelect { body, .. } => {
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        arcweft_lang_syntax::ast::choice::ChoicePlanItem::Raw(_) => {}
    }
}

fn collect_speaker_presets_from_line_plan_item(
    item: &LinePlanItem,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match item {
        LinePlanItem::Init(stmts) | LinePlanItem::On { body: stmts, .. } => {
            collect_speaker_presets_from_stmts(stmts, character_aliases, presets);
        }
        LinePlanItem::CancelRule(rule) => {
            collect_speaker_presets_from_stmts(rule.action(), character_aliases, presets);
        }
        LinePlanItem::Thread(block) => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        LinePlanItem::Option { value, .. }
        | LinePlanItem::Let { expr: value, .. }
        | LinePlanItem::Out(value)
        | LinePlanItem::TimedCue { anchor: value, .. }
        | LinePlanItem::Assert { expr: value, .. }
        | LinePlanItem::Expr(value) => {
            collect_speaker_presets_from_expr(value, character_aliases, presets);
        }
        LinePlanItem::Stmt(stmt) => {
            collect_speaker_presets_from_stmt(stmt, character_aliases, presets);
        }
        LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
            for item in items {
                collect_speaker_presets_from_line_plan_item(item, character_aliases, presets);
            }
        }
        LinePlanItem::Raw(_) => {}
    }
}

fn collect_speaker_presets_from_stmts(
    stmts: &[Stmt],
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    for stmt in stmts {
        collect_speaker_presets_from_stmt(stmt, character_aliases, presets);
    }
}

fn collect_speaker_presets_from_stmt(
    stmt: &Stmt,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match stmt {
        Stmt::Let { pattern, expr, .. } => {
            collect_speaker_preset_binding(pattern, expr, character_aliases, presets);
        }
        Stmt::LetElse {
            pattern,
            expr,
            else_body,
            ..
        } => {
            collect_speaker_preset_binding(pattern, expr, character_aliases, presets);
            collect_speaker_presets_from_stmts(else_body, character_aliases, presets);
        }
        Stmt::LetChoice { pattern: _, choice } => {
            collect_speaker_presets_from_choice_items(choice.items(), character_aliases, presets);
        }
        Stmt::LetScope { scope, .. } => {
            collect_speaker_presets_from_stmts(scope.statements(), character_aliases, presets);
            if let Some(value) = scope.value() {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        Stmt::LetLoop { block, .. } => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        Stmt::LetAwait { await_with, .. } => {
            collect_speaker_presets_from_expr(await_with.expr(), character_aliases, presets);
            for branch in await_with.branches() {
                collect_speaker_presets_from_await_branch(branch, character_aliases, presets);
            }
        }
        Stmt::Return(expr)
        | Stmt::Out { expr, .. }
        | Stmt::Goto(expr)
        | Stmt::Defer { expr, .. }
        | Stmt::Yield(expr)
        | Stmt::Close(expr)
        | Stmt::Select(expr)
        | Stmt::Expr(expr) => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
        }
        Stmt::Assign { target, expr } => {
            collect_speaker_presets_from_expr(target, character_aliases, presets);
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
        }
        Stmt::Thread(block) => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        Stmt::DeferBlock { statements, .. } => {
            collect_speaker_presets_from_stmts(statements, character_aliases, presets);
        }
        Stmt::Signal { target, value }
        | Stmt::LifetimeSet {
            target,
            expr: value,
        } => {
            collect_speaker_presets_from_expr(target, character_aliases, presets);
            collect_speaker_presets_from_expr(value, character_aliases, presets);
        }
        Stmt::On { body, .. } | Stmt::UnsafeLifetime { body, .. } | Stmt::Loop { body } => {
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::WhileLet { .. }
        | Stmt::For { .. }
        | Stmt::Match { .. }
        | Stmt::Break { .. } => {
            collect_speaker_presets_from_control_stmt(stmt, character_aliases, presets);
        }
        Stmt::LetTextSubmit { target, .. } => {
            collect_speaker_presets_from_expr(target, character_aliases, presets);
        }
        Stmt::LetActionReceive { action, .. } => {
            collect_speaker_presets_from_expr(action, character_aliases, presets);
        }
        Stmt::Wait(_) | Stmt::Continue { .. } | Stmt::Raw(_) => {}
    }
}

fn collect_speaker_presets_from_control_stmt(
    stmt: &Stmt,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match stmt {
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            collect_speaker_presets_from_expr(condition, character_aliases, presets);
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
            collect_speaker_presets_from_stmts(else_body, character_aliases, presets);
        }
        Stmt::While { condition, body } => {
            collect_speaker_presets_from_expr(condition, character_aliases, presets);
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
            if let Some(guard) = guard {
                collect_speaker_presets_from_expr(guard, character_aliases, presets);
            }
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        Stmt::For { source, body, .. } => {
            collect_speaker_presets_from_expr(source, character_aliases, presets);
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        Stmt::Match { expr, arms } => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    collect_speaker_presets_from_expr(guard, character_aliases, presets);
                }
                collect_speaker_presets_from_stmts(arm.body(), character_aliases, presets);
            }
        }
        Stmt::Break {
            expr: Some(expr), ..
        } => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
        }
        _ => {}
    }
}

fn collect_speaker_preset_binding(
    pattern: &Pattern,
    expr: &Expr,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    if let Some(name) = pattern_binding_name(pattern)
        && is_speaker_preset_expr(expr, character_aliases, presets)
    {
        presets.insert(name.to_owned());
    }
    collect_speaker_presets_from_expr(expr, character_aliases, presets);
}

fn pattern_binding_name(pattern: &Pattern) -> Option<&str> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Some(name.as_str())
        }
        _ => None,
    }
    .filter(|name| is_identifier(name))
}

fn collect_speaker_presets_from_expr(
    expr: &Expr,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match expr {
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            for item in items {
                collect_speaker_presets_from_expr(item, character_aliases, presets);
            }
        }
        Expr::ArrayRepeat { value, len } => {
            collect_speaker_presets_from_expr(value, character_aliases, presets);
            collect_speaker_presets_from_expr(len, character_aliases, presets);
        }
        Expr::Call { callee, args } => {
            collect_speaker_presets_from_expr(callee, character_aliases, presets);
            for arg in args {
                collect_speaker_presets_from_expr(arg.value(), character_aliases, presets);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_speaker_presets_from_expr(receiver, character_aliases, presets);
            for arg in args {
                collect_speaker_presets_from_expr(arg.value(), character_aliases, presets);
            }
        }
        Expr::Field { target, .. } | Expr::Try { expr: target } => {
            collect_speaker_presets_from_expr(target, character_aliases, presets);
        }
        Expr::DialogueCall { callee, plan, .. } => {
            collect_speaker_presets_from_expr(callee, character_aliases, presets);
            if let Some(plan) = plan {
                for item in plan.items() {
                    collect_speaker_presets_from_line_plan_item(item, character_aliases, presets);
                }
            }
        }
        Expr::Index { target, index } => {
            collect_speaker_presets_from_expr(target, character_aliases, presets);
            collect_speaker_presets_from_expr(index, character_aliases, presets);
        }
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
            collect_speaker_presets_from_expr(lhs, character_aliases, presets);
            collect_speaker_presets_from_expr(rhs, character_aliases, presets);
        }
        Expr::Await { expr, .. } | Expr::Unary { expr, .. } => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
        }
        Expr::Range { start, end, .. } => {
            if let Some(start) = start {
                collect_speaker_presets_from_expr(start, character_aliases, presets);
            }
            if let Some(end) = end {
                collect_speaker_presets_from_expr(end, character_aliases, presets);
            }
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            for (_, value) in fields {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        Expr::Closure { body, .. } => {
            collect_speaker_presets_from_expr(body, character_aliases, presets);
        }
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::MemoBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => {
            collect_speaker_presets_from_expr_block(
                statements,
                value.as_deref(),
                character_aliases,
                presets,
            );
        }
        Expr::If { .. } | Expr::IfLet { .. } | Expr::Match { .. } => {
            collect_speaker_presets_from_control_expr(expr, character_aliases, presets);
        }
        Expr::Thread { block } => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::Raw(_) => {}
    }
}

fn collect_speaker_presets_from_expr_block(
    statements: &[Stmt],
    value: Option<&Expr>,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    collect_speaker_presets_from_stmts(statements, character_aliases, presets);
    if let Some(value) = value {
        collect_speaker_presets_from_expr(value, character_aliases, presets);
    }
}

fn collect_speaker_presets_from_control_expr(
    expr: &Expr,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match expr {
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_speaker_presets_from_expr(condition, character_aliases, presets);
            collect_speaker_presets_from_expr(then_branch, character_aliases, presets);
            if let Some(else_branch) = else_branch {
                collect_speaker_presets_from_expr(else_branch, character_aliases, presets);
            }
        }
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
            if let Some(guard) = guard {
                collect_speaker_presets_from_expr(guard, character_aliases, presets);
            }
            collect_speaker_presets_from_expr(then_branch, character_aliases, presets);
            if let Some(else_branch) = else_branch {
                collect_speaker_presets_from_expr(else_branch, character_aliases, presets);
            }
        }
        Expr::Match { scrutinee, arms } => {
            collect_speaker_presets_from_expr(scrutinee, character_aliases, presets);
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    collect_speaker_presets_from_expr(guard, character_aliases, presets);
                }
                collect_speaker_presets_from_expr(arm.value(), character_aliases, presets);
            }
        }
        _ => {}
    }
}

fn is_speaker_preset_expr(
    expr: &Expr,
    character_aliases: &BTreeSet<String>,
    presets: &BTreeSet<String>,
) -> bool {
    match expr {
        Expr::Call { callee, .. } => speaker_preset_callee(callee, character_aliases, presets),
        Expr::MethodCall { receiver, .. } => {
            is_speaker_preset_expr(receiver, character_aliases, presets)
        }
        Expr::Block { value, .. }
        | Expr::ComputationBlock { value, .. }
        | Expr::MemoBlock { value, .. }
        | Expr::NamedBlock { value, .. } => value
            .as_deref()
            .is_some_and(|value| is_speaker_preset_expr(value, character_aliases, presets)),
        _ => false,
    }
}

fn speaker_preset_callee(
    callee: &Expr,
    character_aliases: &BTreeSet<String>,
    presets: &BTreeSet<String>,
) -> bool {
    match callee {
        Expr::Path(path)
            if character_aliases.contains(path.as_label()) || presets.contains(path.as_label()) =>
        {
            true
        }
        Expr::Field { target, field } if field == "new" => {
            matches!(target.as_ref(), Expr::Path(path) if path == "SpeakerPreset")
        }
        _ => false,
    }
}
