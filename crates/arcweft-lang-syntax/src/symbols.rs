use crate::ast::{ContractClause, DialogueToken, EntityRef, LinePlanItem, Stmt};
use crate::expr::Expr;
use crate::lower::{HirFlowItem, HirModule};

/// Kind of symbol-like syntax discovered in lowered HIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolUseKind {
    DialogueCallee,
    EntityRef,
    Path,
    Call,
    Method,
    RawExpr,
}

/// A symbol-like use that later name resolution and type checking must handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolUse {
    kind: SymbolUseKind,
    name: String,
}

/// Collects all symbol-shaped uses from HIR without reparsing source snippets.
pub fn collect_symbol_uses(module: &HirModule) -> Vec<SymbolUse> {
    let mut uses = Vec::new();
    for flow in module.flows() {
        if let Some(id) = flow.id() {
            push_entity(&mut uses, id);
        }
        for contract in flow.contracts() {
            collect_contract_clause(contract, &mut uses);
        }
        for item in flow.body() {
            collect_flow_item(item, &mut uses);
        }
    }
    for item in module.top_level_items() {
        collect_flow_item(item, &mut uses);
    }
    uses
}

fn collect_flow_item(item: &HirFlowItem, uses: &mut Vec<SymbolUse>) {
    match item {
        HirFlowItem::Stmt(stmt) => collect_stmt(stmt, uses),
        HirFlowItem::Dialogue(dialogue) => collect_dialogue(dialogue, uses),
        HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
            collect_choice(choice, uses);
        }
        HirFlowItem::LetScope { scope, .. } => {
            for stmt in scope.statements() {
                collect_stmt(stmt, uses);
            }
            if let Some(value) = scope.value() {
                collect_expr(value, uses);
            }
        }
        HirFlowItem::LetLoop { block, .. } | HirFlowItem::Loop(block) => {
            for item in block.body() {
                collect_flow_item(item, uses);
            }
        }
        HirFlowItem::If(block) => {
            collect_expr(block.condition(), uses);
            for item in block.body() {
                collect_flow_item(item, uses);
            }
        }
        HirFlowItem::IfLet(block) => {
            collect_expr(block.expr(), uses);
            if let Some(guard) = block.guard() {
                collect_expr(guard, uses);
            }
            for item in block.body() {
                collect_flow_item(item, uses);
            }
        }
        HirFlowItem::Match(block) => {
            collect_expr(block.expr(), uses);
            for arm in block.arms() {
                for item in arm.body() {
                    collect_flow_item(item, uses);
                }
            }
        }
        HirFlowItem::While(block) => {
            collect_expr(block.condition(), uses);
            for item in block.body() {
                collect_flow_item(item, uses);
            }
        }
        HirFlowItem::WhileLet(block) => {
            collect_expr(block.expr(), uses);
            if let Some(guard) = block.guard() {
                collect_expr(guard, uses);
            }
            for item in block.body() {
                collect_flow_item(item, uses);
            }
        }
        HirFlowItem::For(block) => {
            collect_expr(block.source(), uses);
            for item in block.body() {
                collect_flow_item(item, uses);
            }
        }
        HirFlowItem::Select(block) => {
            for branch in block.branches() {
                collect_select_head(branch.head(), uses);
                for item in branch.body() {
                    collect_flow_item(item, uses);
                }
            }
        }
        HirFlowItem::Borrow(block) => {
            collect_expr(block.source(), uses);
            for item in block.body() {
                collect_flow_item(item, uses);
            }
        }
        HirFlowItem::SourceLocale(block) => {
            for item in block.body() {
                collect_flow_item(item, uses);
            }
        }
        HirFlowItem::Scope(block) => collect_flow_items(block.body(), uses),
        HirFlowItem::Include(entity) => push_entity(uses, entity),
        HirFlowItem::Await(await_with) => {
            collect_expr(await_with.expr(), uses);
            for branch in await_with.branches() {
                for item in branch.body() {
                    collect_flow_item(item, uses);
                }
            }
        }
        HirFlowItem::Scenario { args, .. } => {
            for arg in args {
                collect_expr(arg, uses);
            }
        }
    }
}

fn collect_flow_items(items: &[HirFlowItem], uses: &mut Vec<SymbolUse>) {
    for item in items {
        collect_flow_item(item, uses);
    }
}

fn collect_dialogue(dialogue: &crate::lower::HirDialogue, uses: &mut Vec<SymbolUse>) {
    uses.push(SymbolUse::new(
        SymbolUseKind::DialogueCallee,
        dialogue.callee().to_owned(),
    ));
    if let Some(id) = dialogue.id() {
        push_entity(uses, id);
    }
    if let Some(text_key) = dialogue.text_key() {
        push_entity(uses, text_key);
    }
    collect_dialogue_content(dialogue.content().tokens(), uses);
    if let Some(plan) = dialogue.plan() {
        for item in plan.items() {
            collect_line_plan_item(item, uses);
        }
    }
}

fn collect_choice(choice: &crate::lower::HirChoice, uses: &mut Vec<SymbolUse>) {
    if let Some(id) = choice.id() {
        push_entity(uses, id);
    }
    for option in choice.options() {
        if let Some(id) = option.id() {
            push_entity(uses, id);
        }
        if let Some(condition) = option.condition() {
            collect_expr(condition, uses);
        }
        if let Some(value) = option.value() {
            collect_expr(value, uses);
        }
        if let Some(text_key) = option.label_text_key() {
            push_entity(uses, text_key);
        }
        if let Some(target) = option.target() {
            push_entity(uses, target);
        }
        if let crate::ast::ChoiceAction::Out(expr) = option.action() {
            collect_expr(expr, uses);
        }
    }
}

fn collect_select_head(head: &crate::ast::SelectBranchHead, uses: &mut Vec<SymbolUse>) {
    match head {
        crate::ast::SelectBranchHead::Bind { source, .. } => collect_expr(source, uses),
        crate::ast::SelectBranchHead::Frame(pattern)
        | crate::ast::SelectBranchHead::Event(pattern) => {
            collect_pattern(pattern, uses);
        }
        crate::ast::SelectBranchHead::Raw(raw) => {
            uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone()));
        }
    }
}

fn collect_pattern(pattern: &crate::ast::Pattern, uses: &mut Vec<SymbolUse>) {
    if let crate::ast::Pattern::Raw(raw) = pattern {
        uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone()));
    }
}

fn collect_contract_clause(contract: &ContractClause, uses: &mut Vec<SymbolUse>) {
    match contract {
        ContractClause::Requires { expr, .. }
        | ContractClause::Ensures { expr, .. }
        | ContractClause::Invariant { expr, .. }
        | ContractClause::Assume { expr }
        | ContractClause::NoEffect(expr)
        | ContractClause::Decreases(expr) => collect_expr(expr, uses),
        ContractClause::Reads(items)
        | ContractClause::Effects(items)
        | ContractClause::Modifies(items) => {
            for item in items {
                collect_expr(item, uses);
            }
        }
    }
}

fn collect_stmt(stmt: &Stmt, uses: &mut Vec<SymbolUse>) {
    match stmt {
        Stmt::Let { expr, .. }
        | Stmt::Return(expr)
        | Stmt::Out(expr)
        | Stmt::Goto(expr)
        | Stmt::Spawn(expr)
        | Stmt::Defer(expr)
        | Stmt::Yield(expr)
        | Stmt::Close(expr)
        | Stmt::Expr(expr) => {
            collect_expr(expr, uses);
        }
        Stmt::Signal { target, value } => {
            collect_expr(target, uses);
            collect_expr(value, uses);
        }
        Stmt::LetElse {
            expr, else_body, ..
        } => {
            collect_expr(expr, uses);
            for stmt in else_body {
                collect_stmt(stmt, uses);
            }
        }
        Stmt::LetChoice { choice, .. } => {
            if let Some(id) = choice.id() {
                push_entity(uses, id);
            }
            for option in choice.options() {
                if let Some(id) = option.id() {
                    push_entity(uses, id);
                }
                if let Some(condition) = option.condition() {
                    collect_expr(condition, uses);
                }
                if let Some(value) = option.value() {
                    collect_expr(value, uses);
                }
                if let Some(text_key) = option.label_text_key() {
                    push_entity(uses, text_key);
                }
                if let crate::ast::ChoiceAction::Out(expr) = option.action() {
                    collect_expr(expr, uses);
                }
                if let Some(target) = match option.action() {
                    crate::ast::ChoiceAction::Goto(target) => Some(target),
                    _ => None,
                } {
                    push_entity(uses, target);
                }
            }
        }
        Stmt::LetScope { scope, .. } => {
            for stmt in scope.statements() {
                collect_stmt(stmt, uses);
            }
            if let Some(value) = scope.value() {
                collect_expr(value, uses);
            }
        }
        Stmt::LetLoop { block, .. } => {
            uses.push(SymbolUse::new(
                SymbolUseKind::RawExpr,
                format!("loop expression with {} body items", block.body().len()),
            ));
        }
        Stmt::Break(Some(expr)) => collect_expr(expr, uses),
        Stmt::Break(None) | Stmt::Continue => {}
        Stmt::Raw(raw) => uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone())),
    }
}

fn collect_line_plan_item(item: &LinePlanItem, uses: &mut Vec<SymbolUse>) {
    match item {
        LinePlanItem::Option { value, .. }
        | LinePlanItem::Let { expr: value, .. }
        | LinePlanItem::Out(value) => collect_expr(value, uses),
        LinePlanItem::TimedCue { anchor, body } => {
            collect_expr(anchor, uses);
            collect_expr(body, uses);
        }
        LinePlanItem::CancelRule(_)
        | LinePlanItem::StartGroup(_)
        | LinePlanItem::TogetherGroup(_)
        | LinePlanItem::Memo(_)
        | LinePlanItem::Assert(_) => {}
        LinePlanItem::Raw(raw) => uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone())),
    }
}

fn collect_dialogue_content(tokens: &[DialogueToken], uses: &mut Vec<SymbolUse>) {
    for token in tokens {
        if let DialogueToken::Expr(expr) = token {
            collect_expr(expr, uses);
        }
    }
}

fn collect_expr(expr: &Expr, uses: &mut Vec<SymbolUse>) {
    match expr {
        Expr::Literal(_) | Expr::Placeholder(_) => {}
        Expr::EntityRef(entity) => push_entity(uses, entity),
        Expr::Path(path) => uses.push(SymbolUse::new(SymbolUseKind::Path, path.clone())),
        Expr::Tuple(items) => {
            for item in items {
                collect_expr(item, uses);
            }
        }
        Expr::Call { callee, args } => {
            if let Expr::Path(path) = callee.as_ref() {
                uses.push(SymbolUse::new(SymbolUseKind::Call, path.clone()));
            }
            collect_expr(callee, uses);
            for arg in args {
                collect_expr(arg, uses);
            }
        }
        Expr::NamedArg { value, .. } => collect_expr(value, uses),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            uses.push(SymbolUse::new(SymbolUseKind::Method, method.clone()));
            collect_expr(receiver, uses);
            for arg in args {
                collect_expr(arg, uses);
            }
        }
        Expr::DialogueCall { callee, .. } => collect_expr(callee, uses),
        Expr::Index { target, index } => {
            collect_expr(target, uses);
            collect_expr(index, uses);
        }
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
            collect_expr(lhs, uses);
            collect_expr(rhs, uses);
        }
        Expr::Range { start, end, .. } => {
            if let Some(start) = start {
                collect_expr(start, uses);
            }
            if let Some(end) = end {
                collect_expr(end, uses);
            }
        }
        Expr::Raw(raw) => uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone())),
    }
}

fn push_entity(uses: &mut Vec<SymbolUse>, entity: &EntityRef) {
    if entity.is_relative() {
        return;
    }
    uses.push(SymbolUse::new(
        SymbolUseKind::EntityRef,
        entity.body().to_owned(),
    ));
}

impl SymbolUse {
    fn new(kind: SymbolUseKind, name: String) -> Self {
        Self { kind, name }
    }

    pub const fn kind(&self) -> SymbolUseKind {
        self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
