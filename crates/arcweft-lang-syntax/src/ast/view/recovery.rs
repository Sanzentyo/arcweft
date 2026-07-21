//! Recursive recovery-state inspection for retained View syntax.

use super::{ViewAction, ViewArg, ViewBody, ViewButtonLabel, ViewExpr, ViewModifier};
use crate::ast::{
    choice::{ChoiceAction, ChoiceBlock, ChoiceItem, ChoicePlanItem},
    dialogue::{DialogueContent, DialogueToken},
    flow::{FlowItem, SelectBranchHead, Stmt, WaitTarget},
    line_plan::{LinePlan, LinePlanItem, TriggerPattern},
    pattern::{Pattern, VariantPatternPayload},
};
use crate::expr::Expr;

pub(super) fn contains_recovered_syntax(body: &ViewBody) -> bool {
    body.locals()
        .iter()
        .any(|local| expr_contains_recovery(local.initial()))
        || view_expr_contains_recovery(body.value())
}

fn view_expr_contains_recovery(view: &ViewExpr) -> bool {
    match view {
        ViewExpr::Raw(_) => true,
        ViewExpr::Fragment(children) => children.iter().any(view_expr_contains_recovery),
        ViewExpr::Element(element) => {
            args_contain_recovery(element.args())
                || modifiers_contain_recovery(element.modifiers())
                || element.children().iter().any(view_expr_contains_recovery)
        }
        ViewExpr::ViewCall(call) => {
            expr_contains_recovery(call.view())
                || args_contain_recovery(call.args())
                || modifiers_contain_recovery(call.modifiers())
        }
        ViewExpr::Text(text) => {
            expr_contains_recovery(text.source()) || modifiers_contain_recovery(text.modifiers())
        }
        ViewExpr::Image(image) => {
            expr_contains_recovery(image.source()) || modifiers_contain_recovery(image.modifiers())
        }
        ViewExpr::TextField(field) => {
            expr_contains_recovery(field.value())
                || args_contain_recovery(field.args())
                || modifiers_contain_recovery(field.modifiers())
                || field
                    .submit_action()
                    .is_some_and(view_action_contains_recovery)
        }
        ViewExpr::Button(button) => {
            matches!(button.label(), ViewButtonLabel::Expr(expr) if expr_contains_recovery(expr))
                || args_contain_recovery(button.args())
                || button.enabled().is_some_and(expr_contains_recovery)
                || modifiers_contain_recovery(button.modifiers())
                || button
                    .activation()
                    .is_some_and(view_action_contains_recovery)
        }
        ViewExpr::Let(view_let) => {
            pattern_contains_recovery(view_let.pattern())
                || expr_contains_recovery(view_let.value())
        }
        ViewExpr::If(view_if) => {
            expr_contains_recovery(view_if.condition())
                || view_expr_contains_recovery(view_if.then_branch())
                || view_if
                    .else_branch()
                    .is_some_and(view_expr_contains_recovery)
        }
        ViewExpr::Match(view_match) => {
            expr_contains_recovery(view_match.scrutinee())
                || view_match.arms().iter().any(|arm| {
                    pattern_contains_recovery(arm.pattern())
                        || arm.guard().is_some_and(expr_contains_recovery)
                        || view_expr_contains_recovery(arm.value())
                })
        }
        ViewExpr::ForEach(view_for_each) => {
            pattern_contains_recovery(view_for_each.pattern())
                || expr_contains_recovery(view_for_each.source())
                || view_for_each.key().is_some_and(expr_contains_recovery)
                || view_expr_contains_recovery(view_for_each.body())
        }
        ViewExpr::Await(view_await) => {
            expr_contains_recovery(view_await.source())
                || view_await.branches().iter().any(|branch| {
                    pattern_contains_recovery(branch.pattern())
                        || view_expr_contains_recovery(branch.value())
                })
        }
        ViewExpr::Expr(expr) => expr_contains_recovery(expr),
    }
}

fn args_contain_recovery(args: &[ViewArg]) -> bool {
    args.iter().any(|arg| match arg {
        ViewArg::Positional(value) | ViewArg::Named { value, .. } => expr_contains_recovery(value),
    })
}

fn modifiers_contain_recovery(modifiers: &[ViewModifier]) -> bool {
    modifiers.iter().any(|modifier| match modifier {
        ViewModifier::Raw(_) => true,
        ViewModifier::Fx(application) => {
            expr_contains_recovery(application.call())
                || application.key().is_some_and(expr_contains_recovery)
        }
        ViewModifier::Label(expr)
        | ViewModifier::Placeholder(expr)
        | ViewModifier::Purpose(expr)
        | ViewModifier::EnterKey(expr)
        | ViewModifier::Enabled(expr) => expr_contains_recovery(expr),
        ViewModifier::Property { value, .. } | ViewModifier::OnEvent { body: value, .. } => {
            expr_contains_recovery(value)
        }
        ViewModifier::Environment(args) => args_contain_recovery(args),
        ViewModifier::Style(_)
        | ViewModifier::Part(_)
        | ViewModifier::AgentTarget(_)
        | ViewModifier::Focusable(_)
        | ViewModifier::Focus(_)
        | ViewModifier::Navigation(_) => false,
    })
}

fn view_action_contains_recovery(action: &ViewAction) -> bool {
    matches!(action, ViewAction::Projection(expr) if expr_contains_recovery(expr))
}

fn pattern_contains_recovery(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Raw(_) => true,
        Pattern::Literal(expr) => expr_contains_recovery(expr),
        Pattern::Variant {
            payload: Some(payload),
            ..
        } => match payload {
            VariantPatternPayload::Tuple(items) => items.iter().any(pattern_contains_recovery),
            VariantPatternPayload::Record { fields, .. } => fields
                .iter()
                .any(|field| pattern_contains_recovery(field.pattern())),
        },
        Pattern::Tuple(items) | Pattern::BracketSeq { items, .. } => {
            items.iter().any(pattern_contains_recovery)
        }
        Pattern::Record { fields, .. } => fields
            .iter()
            .any(|field| pattern_contains_recovery(field.pattern())),
        Pattern::Whole { pattern, .. } => pattern_contains_recovery(pattern),
        Pattern::Ident(_)
        | Pattern::MutIdent(_)
        | Pattern::Entity(_)
        | Pattern::Variant { payload: None, .. }
        | Pattern::Discard
        | Pattern::Typed { .. } => false,
    }
}

fn expr_contains_recovery(expr: &Expr) -> bool {
    match expr {
        Expr::Raw(_) => true,
        Expr::Tuple(items) | Expr::BracketSeq(items) => items.iter().any(expr_contains_recovery),
        Expr::ArrayRepeat { value, len } => {
            expr_contains_recovery(value) || expr_contains_recovery(len)
        }
        Expr::Call(call) => {
            expr_contains_recovery(call.callee())
                || call
                    .args()
                    .iter()
                    .any(|arg| expr_contains_recovery(arg.value()))
        }
        Expr::Select(select) => expr_contains_recovery(select.target()),
        Expr::DialogueCall {
            callee,
            content,
            plan,
        } => {
            expr_contains_recovery(callee)
                || dialogue_content_contains_recovery(content)
                || plan.as_ref().is_some_and(line_plan_contains_recovery)
        }
        Expr::Index { target, index } => {
            expr_contains_recovery(target) || expr_contains_recovery(index)
        }
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
            expr_contains_recovery(lhs) || expr_contains_recovery(rhs)
        }
        Expr::Try { expr } | Expr::Unary { expr, .. } => expr_contains_recovery(expr),
        Expr::Await(awaited) => expr_contains_recovery(awaited.operand()),
        Expr::Borrow(borrow) => expr_contains_recovery(borrow.operand()),
        Expr::Deref(deref) => expr_contains_recovery(deref.operand()),
        Expr::Range { start, end, .. } => {
            start.as_deref().is_some_and(expr_contains_recovery)
                || end.as_deref().is_some_and(expr_contains_recovery)
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .any(|(_, value)| expr_contains_recovery(value)),
        Expr::Closure { params, body, .. } => {
            params
                .iter()
                .any(|param| pattern_contains_recovery(param.pattern()))
                || expr_contains_recovery(body)
        }
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => {
            statements.iter().any(stmt_contains_recovery)
                || value.as_deref().is_some_and(expr_contains_recovery)
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains_recovery(condition)
                || expr_contains_recovery(then_branch)
                || else_branch.as_deref().is_some_and(expr_contains_recovery)
        }
        Expr::IfLet {
            pattern,
            expr,
            guard,
            then_branch,
            else_branch,
        } => {
            pattern_contains_recovery(pattern)
                || expr_contains_recovery(expr)
                || guard.as_deref().is_some_and(expr_contains_recovery)
                || expr_contains_recovery(then_branch)
                || else_branch.as_deref().is_some_and(expr_contains_recovery)
        }
        Expr::Match { scrutinee, arms } => {
            expr_contains_recovery(scrutinee)
                || arms.iter().any(|arm| {
                    pattern_contains_recovery(arm.pattern())
                        || arm.guard().is_some_and(expr_contains_recovery)
                        || expr_contains_recovery(arm.value())
                })
        }
        Expr::Thread { block } => flow_items_contain_recovery(block.body()),
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_) => false,
    }
}

fn stmt_contains_recovery(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Raw(_) => true,
        Stmt::Let { pattern, expr, .. } => {
            pattern_contains_recovery(pattern) || expr_contains_recovery(expr)
        }
        Stmt::Assign { target, expr } => {
            expr_contains_recovery(target.expr()) || expr_contains_recovery(expr.expr())
        }
        Stmt::LetElse {
            pattern,
            expr,
            else_body,
            ..
        } => {
            pattern_contains_recovery(pattern)
                || expr_contains_recovery(expr.expr())
                || else_body.iter().any(stmt_contains_recovery)
        }
        Stmt::LetActionReceive { pattern, action } => {
            pattern_contains_recovery(pattern) || expr_contains_recovery(action.expr())
        }
        Stmt::Return { expr, .. } | Stmt::Expr { expr, .. } => expr_contains_recovery(expr),
        Stmt::Out { expr, .. }
        | Stmt::Goto(expr)
        | Stmt::Defer { expr, .. }
        | Stmt::Yield(expr)
        | Stmt::Close(expr)
        | Stmt::Select(expr) => expr_contains_recovery(expr.expr()),
        Stmt::Signal { target, value }
        | Stmt::LifetimeSet {
            target,
            expr: value,
        } => expr_contains_recovery(target.expr()) || expr_contains_recovery(value.expr()),
        Stmt::Thread(thread) => flow_items_contain_recovery(thread.body()),
        Stmt::DeferBlock { statements, .. } | Stmt::Loop { body: statements } => {
            statements.iter().any(stmt_contains_recovery)
        }
        Stmt::On { trigger, body } => {
            trigger_contains_recovery(trigger) || body.iter().any(stmt_contains_recovery)
        }
        Stmt::UnsafeLifetime { reason, body, .. } => {
            reason.as_ref().is_some_and(expr_contains_recovery)
                || body.iter().any(stmt_contains_recovery)
        }
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            expr_contains_recovery(condition.expr())
                || body.iter().any(stmt_contains_recovery)
                || else_body.iter().any(stmt_contains_recovery)
        }
        Stmt::While { condition, body } => {
            expr_contains_recovery(condition.expr()) || body.iter().any(stmt_contains_recovery)
        }
        Stmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
        } => {
            pattern_contains_recovery(pattern)
                || expr_contains_recovery(expr.expr())
                || guard
                    .as_ref()
                    .is_some_and(|guard| expr_contains_recovery(guard.expr()))
                || body.iter().any(stmt_contains_recovery)
        }
        Stmt::For {
            pattern,
            source,
            body,
        } => {
            pattern_contains_recovery(pattern)
                || expr_contains_recovery(source.expr())
                || body.iter().any(stmt_contains_recovery)
        }
        Stmt::Match { expr, arms } => {
            expr_contains_recovery(expr.expr())
                || arms.iter().any(|arm| {
                    pattern_contains_recovery(arm.pattern())
                        || arm.guard().is_some_and(expr_contains_recovery)
                        || arm.body().iter().any(stmt_contains_recovery)
                })
        }
        Stmt::Break { expr, .. } => expr
            .as_ref()
            .is_some_and(|expr| expr_contains_recovery(expr.expr())),
        Stmt::Assertion(_)
        | Stmt::LetChoice { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Wait(_)
        | Stmt::Continue { .. } => stmt_nested_value_contains_recovery(stmt),
    }
}

fn stmt_nested_value_contains_recovery(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Assertion(assertion) => assertion.conditions().iter().any(expr_contains_recovery),
        Stmt::LetChoice { pattern, choice } => {
            pattern_contains_recovery(pattern) || choice_contains_recovery(choice)
        }
        Stmt::LetScope { pattern, scope } => {
            pattern_contains_recovery(pattern)
                || scope.statements().iter().any(stmt_contains_recovery)
                || scope.value().is_some_and(expr_contains_recovery)
        }
        Stmt::LetLoop { pattern, block } => {
            pattern_contains_recovery(pattern) || flow_items_contain_recovery(block.body())
        }
        Stmt::LetAwait {
            pattern,
            await_with,
        } => {
            pattern_contains_recovery(pattern)
                || expr_contains_recovery(await_with.expr())
                || await_with.branches().iter().any(|branch| {
                    pattern_contains_recovery(branch.pattern())
                        || flow_items_contain_recovery(branch.body())
                })
        }
        Stmt::Wait(WaitTarget::Duration(expr) | WaitTarget::Expr(expr)) => {
            expr_contains_recovery(expr.expr())
        }
        Stmt::Continue { .. } => false,
        _ => unreachable!("caller routes only nested-value statement variants"),
    }
}

fn flow_items_contain_recovery(items: &[FlowItem]) -> bool {
    items.iter().any(flow_item_contains_recovery)
}

fn flow_item_contains_recovery(item: &FlowItem) -> bool {
    match item {
        FlowItem::Raw(_) => true,
        FlowItem::Stmt(statement) => stmt_contains_recovery(statement),
        FlowItem::SpeakerLine(line) => {
            dialogue_content_contains_recovery(line.content())
                || line.plan().is_some_and(line_plan_contains_recovery)
        }
        FlowItem::ContentCall(call) => {
            dialogue_content_contains_recovery(call.content())
                || call.plan().is_some_and(line_plan_contains_recovery)
        }
        FlowItem::Choice(choice) => choice_contains_recovery(choice),
        FlowItem::If(block) => {
            expr_contains_recovery(block.condition())
                || flow_items_contain_recovery(block.body())
                || flow_items_contain_recovery(block.else_body())
        }
        FlowItem::IfLet(block) => {
            pattern_contains_recovery(block.pattern())
                || expr_contains_recovery(block.expr())
                || block.guard().is_some_and(expr_contains_recovery)
                || flow_items_contain_recovery(block.body())
                || flow_items_contain_recovery(block.else_body())
        }
        FlowItem::Match(block) => {
            expr_contains_recovery(block.expr())
                || block.arms().iter().any(|arm| {
                    pattern_contains_recovery(arm.pattern())
                        || arm.guard().is_some_and(expr_contains_recovery)
                        || flow_items_contain_recovery(arm.body())
                })
        }
        FlowItem::Loop(block) => flow_items_contain_recovery(block.body()),
        FlowItem::While(block) => {
            expr_contains_recovery(block.condition()) || flow_items_contain_recovery(block.body())
        }
        FlowItem::WhileLet(block) => {
            pattern_contains_recovery(block.pattern())
                || expr_contains_recovery(block.expr())
                || block.guard().is_some_and(expr_contains_recovery)
                || flow_items_contain_recovery(block.body())
        }
        FlowItem::For(block) => {
            pattern_contains_recovery(block.pattern())
                || expr_contains_recovery(block.source())
                || flow_items_contain_recovery(block.body())
        }
        FlowItem::Select(block) => block.branches().iter().any(|branch| {
            select_head_contains_recovery(branch.head())
                || flow_items_contain_recovery(branch.body())
        }),
        FlowItem::SourceLocale(block) => flow_items_contain_recovery(block.body()),
        FlowItem::Scope(block) => flow_items_contain_recovery(block.body()),
        FlowItem::AwaitWith(await_with) => {
            expr_contains_recovery(await_with.expr())
                || await_with.branches().iter().any(|branch| {
                    pattern_contains_recovery(branch.pattern())
                        || flow_items_contain_recovery(branch.body())
                })
        }
        FlowItem::Include(_) => false,
    }
}

fn select_head_contains_recovery(head: &SelectBranchHead) -> bool {
    match head {
        SelectBranchHead::Raw(_) => true,
        SelectBranchHead::Bind { source, .. } => expr_contains_recovery(source),
        SelectBranchHead::Frame(pattern) | SelectBranchHead::Event(pattern) => {
            pattern_contains_recovery(pattern)
        }
    }
}

fn choice_contains_recovery(choice: &ChoiceBlock) -> bool {
    choice.items().iter().any(choice_item_contains_recovery)
        || choice.plan().is_some_and(|plan| {
            plan.items().iter().any(|item| match item {
                ChoicePlanItem::Raw(_) => true,
                ChoicePlanItem::Option { value, .. } => expr_contains_recovery(value),
                ChoicePlanItem::Timeout { duration, body } => {
                    expr_contains_recovery(duration) || body.iter().any(stmt_contains_recovery)
                }
                ChoicePlanItem::Cancel { trigger, body } => {
                    trigger_contains_recovery(trigger) || body.iter().any(stmt_contains_recovery)
                }
                ChoicePlanItem::OnSelect { pattern, body } => {
                    pattern_contains_recovery(pattern) || body.iter().any(stmt_contains_recovery)
                }
            })
        })
}

fn choice_item_contains_recovery(item: &ChoiceItem) -> bool {
    match item {
        ChoiceItem::Raw(_) => true,
        ChoiceItem::Let { pattern, expr } => {
            pattern_contains_recovery(pattern) || expr_contains_recovery(expr)
        }
        ChoiceItem::If { condition, items } => {
            expr_contains_recovery(condition) || items.iter().any(choice_item_contains_recovery)
        }
        ChoiceItem::For {
            pattern,
            source,
            items,
        } => {
            pattern_contains_recovery(pattern)
                || expr_contains_recovery(source)
                || items.iter().any(choice_item_contains_recovery)
        }
        ChoiceItem::Match { expr, arms } => {
            expr_contains_recovery(expr)
                || arms.iter().any(|arm| {
                    pattern_contains_recovery(arm.pattern())
                        || arm.guard().is_some_and(expr_contains_recovery)
                        || arm.items().iter().any(choice_item_contains_recovery)
                })
        }
        ChoiceItem::Option(option) => {
            option.id_expr().is_some_and(expr_contains_recovery)
                || option.value().is_some_and(expr_contains_recovery)
                || option.enabled().is_some_and(expr_contains_recovery)
                || option.visible().is_some_and(expr_contains_recovery)
                || option.order().is_some_and(expr_contains_recovery)
                || option.hotkey().is_some_and(expr_contains_recovery)
                || option
                    .view_fields()
                    .iter()
                    .any(|field| expr_contains_recovery(field.value()))
                || match option.action() {
                    ChoiceAction::Out(expr) => expr_contains_recovery(expr),
                    ChoiceAction::SelectBlock(body) => body.iter().any(stmt_contains_recovery),
                    ChoiceAction::Goto(_) | ChoiceAction::None => false,
                }
        }
    }
}

fn dialogue_content_contains_recovery(content: &DialogueContent) -> bool {
    !content.diagnostics().is_empty()
        || content.tokens().iter().any(|token| match token {
            DialogueToken::Raw(_) => true,
            DialogueToken::Expr(expr) => expr_contains_recovery(expr.expr()),
            DialogueToken::Text(_)
            | DialogueToken::Tag(_)
            | DialogueToken::InferredTag(_)
            | DialogueToken::Mark(_)
            | DialogueToken::EndTag(_)
            | DialogueToken::InferredEndTag
            | DialogueToken::Ruby { .. }
            | DialogueToken::Escape(_) => false,
        })
}

fn line_plan_contains_recovery(plan: &LinePlan) -> bool {
    plan.items().iter().any(line_plan_item_contains_recovery)
}

fn line_plan_item_contains_recovery(item: &LinePlanItem) -> bool {
    match item {
        LinePlanItem::Raw(_) => true,
        LinePlanItem::Init(body) => body.iter().any(stmt_contains_recovery),
        LinePlanItem::Thread(thread) => flow_items_contain_recovery(thread.body()),
        LinePlanItem::On { trigger, body } => {
            trigger_contains_recovery(trigger) || body.iter().any(stmt_contains_recovery)
        }
        LinePlanItem::Option { value, .. }
        | LinePlanItem::Out(value)
        | LinePlanItem::Expr(value) => expr_contains_recovery(value),
        LinePlanItem::Let { pattern, expr } => {
            pattern_contains_recovery(pattern) || expr_contains_recovery(expr)
        }
        LinePlanItem::Stmt(statement) => stmt_contains_recovery(statement),
        LinePlanItem::CancelRule(rule) => {
            trigger_contains_recovery(rule.trigger())
                || rule.action().iter().any(stmt_contains_recovery)
        }
        LinePlanItem::TimedCue { anchor, body } => {
            expr_contains_recovery(anchor) || expr_contains_recovery(body)
        }
        LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
            items.iter().any(line_plan_item_contains_recovery)
        }
        LinePlanItem::TimelineAssert(assertion) => expr_contains_recovery(assertion.condition()),
    }
}

fn trigger_contains_recovery(trigger: &TriggerPattern) -> bool {
    match trigger {
        TriggerPattern::Input(pattern)
        | TriggerPattern::Event(pattern)
        | TriggerPattern::Mark(pattern)
        | TriggerPattern::Select(pattern)
        | TriggerPattern::Task(pattern)
        | TriggerPattern::Scope(pattern) => pattern_contains_recovery(pattern),
        TriggerPattern::Signal { target, value } => {
            expr_contains_recovery(target) || value.as_ref().is_some_and(pattern_contains_recovery)
        }
        TriggerPattern::Timeout(expr) | TriggerPattern::Expr(expr) => expr_contains_recovery(expr),
    }
}
