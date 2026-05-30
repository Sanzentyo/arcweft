use arcweft_lang_hir::model::{HirFlowItem, HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::{
    ast::{
        choice::{ChoiceAction, ChoiceBlock, ChoiceItem, ChoiceOption, ChoicePlanItem},
        dialogue::DialogueToken,
        flow::{
            AwaitWith, ContractClause, FlowItem, SelectBranchHead, Stmt, StmtMatchArm, WaitTarget,
        },
        ids::{EntityRef, EntityRefSyntax, IdRef},
        items::{ImplMember, RawSyntax, TraitMember},
        line_plan::{LinePlan, LinePlanItem, TriggerPattern},
        pattern::{Pattern, VariantPatternPayload},
    },
    expr::{Expr, MatchExprArm},
};

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
    for function in module.functions() {
        for contract in function.contracts() {
            collect_contract_clause(contract, &mut uses);
        }
        for stmt in function.statements() {
            collect_stmt(stmt, &mut uses);
        }
        if let Some(value) = function.value() {
            collect_expr(value, &mut uses);
        }
    }
    for declaration in module.declarations() {
        collect_top_level_decl(declaration, &mut uses);
    }
    for item in module.top_level_items() {
        collect_flow_item(item, &mut uses);
    }
    uses
}

fn collect_top_level_decl(declaration: &HirTopLevelDecl, uses: &mut Vec<SymbolUse>) {
    match declaration {
        HirTopLevelDecl::Attribute(_)
        | HirTopLevelDecl::DialogueDefaults(_)
        | HirTopLevelDecl::Enum(_)
        | HirTopLevelDecl::ExternCapability(_)
        | HirTopLevelDecl::ExternMod(_)
        | HirTopLevelDecl::Proof(_)
        | HirTopLevelDecl::Struct(_)
        | HirTopLevelDecl::TrustedAxiom(_) => {}
        HirTopLevelDecl::Entry(item) => collect_entry_decl(item, uses),
        HirTopLevelDecl::Test(item) => push_id_ref(uses, item.id()),
        HirTopLevelDecl::Bench(item) => push_id_ref(uses, item.id()),
        HirTopLevelDecl::Impl(item) => {
            for member in item.members() {
                match member {
                    ImplMember::Function {
                        body_statements,
                        body_value,
                        ..
                    } => {
                        for stmt in body_statements {
                            collect_stmt(stmt, uses);
                        }
                        if let Some(value) = body_value {
                            collect_expr(value, uses);
                        }
                    }
                    ImplMember::Raw(raw) => {
                        uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone()));
                    }
                    ImplMember::AssociatedType { .. } => {}
                }
            }
        }
        HirTopLevelDecl::EntityDecl(item) => push_entity(uses, item.id()),
        HirTopLevelDecl::Callable(item) => {
            for contract in item.contracts() {
                collect_contract_clause(contract, uses);
            }
        }
        HirTopLevelDecl::State(item) => {
            for field in item.fields() {
                collect_expr(field.default(), uses);
            }
        }
        HirTopLevelDecl::Trait(item) => {
            for member in item.members() {
                if let TraitMember::Raw(raw) = member {
                    uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone()));
                }
            }
        }
        HirTopLevelDecl::TypeAlias(item) => {
            for clause in item.where_clauses() {
                collect_expr(clause, uses);
            }
        }
        HirTopLevelDecl::Hook(item) => {
            push_entity(uses, item.id());
            for stmt in item.body_statements() {
                collect_stmt(stmt, uses);
            }
        }
        HirTopLevelDecl::MemoFn(item) => {
            for stmt in item.body_statements() {
                collect_stmt(stmt, uses);
            }
            if let Some(value) = item.body_value() {
                collect_expr(value, uses);
            }
        }
        HirTopLevelDecl::Parser(item) => {
            for stmt in item.body_statements() {
                collect_stmt(stmt, uses);
            }
            if let Some(value) = item.body_value() {
                collect_expr(value, uses);
            }
        }
        HirTopLevelDecl::Source(item) => {
            if let Some(id) = item.id() {
                push_entity(uses, id);
            }
            for stmt in item.body_statements() {
                collect_stmt(stmt, uses);
            }
        }
    }
}

fn collect_entry_decl(
    item: &arcweft_lang_syntax::ast::items::EntryDeclItem,
    uses: &mut Vec<SymbolUse>,
) {
    push_entity(uses, item.id());
    for item in item.items() {
        match item {
            arcweft_lang_syntax::ast::items::EntryItem::Start(target)
            | arcweft_lang_syntax::ast::items::EntryItem::Run(target)
            | arcweft_lang_syntax::ast::items::EntryItem::Route { target, .. } => {
                push_entity(uses, target);
            }
            arcweft_lang_syntax::ast::items::EntryItem::Option { value, .. } => {
                collect_expr(value, uses);
            }
            arcweft_lang_syntax::ast::items::EntryItem::Raw(raw) => {
                uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone()));
            }
        }
    }
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
            for item in block.else_body() {
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
            for item in block.else_body() {
                collect_flow_item(item, uses);
            }
        }
        HirFlowItem::Match(block) => {
            collect_match_block(block, uses);
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
        HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
            collect_expr(await_with.expr(), uses);
            for branch in await_with.branches() {
                collect_pattern(branch.pattern(), uses);
                for item in branch.body() {
                    collect_flow_item(item, uses);
                }
            }
        }
        HirFlowItem::Thread(thread) => {
            for item in thread.body() {
                collect_flow_item(item, uses);
            }
        }
    }
}

fn collect_flow_items(items: &[HirFlowItem], uses: &mut Vec<SymbolUse>) {
    for item in items {
        collect_flow_item(item, uses);
    }
}

fn collect_dialogue(dialogue: &arcweft_lang_hir::model::HirDialogue, uses: &mut Vec<SymbolUse>) {
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

fn collect_choice(choice: &arcweft_lang_hir::model::HirChoice, uses: &mut Vec<SymbolUse>) {
    if let Some(id) = choice.id() {
        push_entity(uses, id);
    }
    for item in choice.items() {
        collect_choice_item(item, uses);
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
        match option.action() {
            ChoiceAction::Out(expr) => collect_expr(expr, uses),
            ChoiceAction::SelectBlock(statements) => {
                collect_stmt_block(statements, uses);
            }
            ChoiceAction::Goto(_) | ChoiceAction::None => {}
        }
    }
    if let Some(plan) = choice.plan() {
        for item in plan.items() {
            collect_choice_plan_item(item, uses);
        }
    }
}

fn collect_choice_item(item: &ChoiceItem, uses: &mut Vec<SymbolUse>) {
    match item {
        ChoiceItem::Let { pattern, expr } => {
            collect_pattern(pattern, uses);
            collect_expr(expr, uses);
        }
        ChoiceItem::If { condition, items } => {
            collect_expr(condition, uses);
            for item in items {
                collect_choice_item(item, uses);
            }
        }
        ChoiceItem::For {
            pattern,
            source,
            items,
        } => {
            collect_pattern(pattern, uses);
            collect_expr(source, uses);
            for item in items {
                collect_choice_item(item, uses);
            }
        }
        ChoiceItem::Match { expr, arms } => {
            collect_expr(expr, uses);
            for arm in arms {
                collect_pattern(arm.pattern(), uses);
                if let Some(guard) = arm.guard() {
                    collect_expr(guard, uses);
                }
                for item in arm.items() {
                    collect_choice_item(item, uses);
                }
            }
        }
        ChoiceItem::Option(option) => {
            collect_choice_option(option, uses);
        }
        ChoiceItem::Raw(raw) => {
            uses.push(SymbolUse::new(
                SymbolUseKind::RawExpr,
                raw.source().to_owned(),
            ));
        }
    }
}

fn collect_choice_option(option: &ChoiceOption, uses: &mut Vec<SymbolUse>) {
    if let Some(id) = option.id() {
        push_id_ref(uses, id);
    }
    if let Some(condition) = option.condition() {
        collect_expr(condition, uses);
    }
    if let Some(value) = option.value() {
        collect_expr(value, uses);
    }
    if let Some(text_key) = option.label_text_key() {
        push_id_ref(uses, text_key);
    }
    if let Some(target) = option.target() {
        push_entity_syntax(uses, target);
    }
    match option.action() {
        ChoiceAction::Out(expr) => collect_expr(expr, uses),
        ChoiceAction::SelectBlock(statements) => {
            collect_stmt_block(statements, uses);
        }
        ChoiceAction::Goto(_) | ChoiceAction::None => {}
    }
}

fn collect_choice_plan_item(item: &ChoicePlanItem, uses: &mut Vec<SymbolUse>) {
    match item {
        ChoicePlanItem::Option { value, .. } => collect_expr(value, uses),
        ChoicePlanItem::Timeout { duration, body } => {
            collect_expr(duration, uses);
            collect_stmt_block(body, uses);
        }
        ChoicePlanItem::Cancel { body, .. } => collect_stmt_block(body, uses),
        ChoicePlanItem::OnSelect { pattern, body } => {
            collect_pattern(pattern, uses);
            collect_stmt_block(body, uses);
        }
        ChoicePlanItem::Raw(raw) => {
            uses.push(SymbolUse::new(
                SymbolUseKind::RawExpr,
                raw.source().to_owned(),
            ));
        }
    }
}

fn collect_stmt_block(statements: &[Stmt], uses: &mut Vec<SymbolUse>) {
    for stmt in statements {
        collect_stmt(stmt, uses);
    }
}

fn collect_syntax_flow_block(items: &[FlowItem], uses: &mut Vec<SymbolUse>) {
    for item in items {
        match item {
            FlowItem::Stmt(stmt) => collect_stmt(stmt, uses),
            FlowItem::AwaitWith(await_with) => collect_unlowered_await_binding(await_with, uses),
            FlowItem::Choice(choice) => collect_choice_stmt(choice, uses),
            FlowItem::If(block) => {
                collect_expr(block.condition(), uses);
                collect_syntax_flow_block(block.body(), uses);
                collect_syntax_flow_block(block.else_body(), uses);
            }
            FlowItem::IfLet(block) => {
                collect_pattern(block.pattern(), uses);
                collect_expr(block.expr(), uses);
                if let Some(guard) = block.guard() {
                    collect_expr(guard, uses);
                }
                collect_syntax_flow_block(block.body(), uses);
                collect_syntax_flow_block(block.else_body(), uses);
            }
            FlowItem::Match(block) => {
                collect_expr(block.expr(), uses);
                for arm in block.arms() {
                    collect_pattern(arm.pattern(), uses);
                    if let Some(guard) = arm.guard() {
                        collect_expr(guard, uses);
                    }
                    collect_syntax_flow_block(arm.body(), uses);
                }
            }
            FlowItem::Loop(block) => collect_syntax_flow_block(block.body(), uses),
            FlowItem::While(block) => {
                collect_expr(block.condition(), uses);
                collect_syntax_flow_block(block.body(), uses);
            }
            FlowItem::WhileLet(block) => {
                collect_pattern(block.pattern(), uses);
                collect_expr(block.expr(), uses);
                if let Some(guard) = block.guard() {
                    collect_expr(guard, uses);
                }
                collect_syntax_flow_block(block.body(), uses);
            }
            FlowItem::For(block) => {
                collect_pattern(block.pattern(), uses);
                collect_expr(block.source(), uses);
                collect_syntax_flow_block(block.body(), uses);
            }
            FlowItem::Select(block) => {
                for branch in block.branches() {
                    collect_select_head(branch.head(), uses);
                    collect_syntax_flow_block(branch.body(), uses);
                }
            }
            FlowItem::BorrowBlock(block) => {
                collect_expr(block.source(), uses);
                collect_pattern(block.binding(), uses);
                collect_syntax_flow_block(block.body(), uses);
            }
            FlowItem::SourceLocale(block) => collect_syntax_flow_block(block.body(), uses),
            FlowItem::Scope(block) => collect_syntax_flow_block(block.body(), uses),
            FlowItem::Include(entity) => uses.push(SymbolUse::new(
                SymbolUseKind::EntityRef,
                entity.body().to_owned(),
            )),
            FlowItem::SpeakerLine(_) | FlowItem::ContentCall(_) | FlowItem::Raw(_) => {}
        }
    }
}

fn collect_match_block(block: &arcweft_lang_hir::model::HirMatch, uses: &mut Vec<SymbolUse>) {
    collect_expr(block.expr(), uses);
    for arm in block.arms() {
        collect_pattern(arm.pattern(), uses);
        if let Some(guard) = arm.guard() {
            collect_expr(guard, uses);
        }
        for item in arm.body() {
            collect_flow_item(item, uses);
        }
    }
}

fn collect_select_head(head: &SelectBranchHead, uses: &mut Vec<SymbolUse>) {
    match head {
        SelectBranchHead::Bind { source, .. } => collect_expr(source, uses),
        SelectBranchHead::Frame(pattern) | SelectBranchHead::Event(pattern) => {
            collect_pattern(pattern, uses);
        }
        SelectBranchHead::Raw(raw) => {
            uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone()));
        }
    }
}

fn collect_pattern(pattern: &Pattern, uses: &mut Vec<SymbolUse>) {
    match pattern {
        Pattern::Raw(raw) => {
            uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone()));
        }
        Pattern::Literal(expr) => collect_expr(expr, uses),
        Pattern::Entity(entity) => push_entity(uses, entity),
        Pattern::Tuple(items) | Pattern::BracketSeq { items, .. } => {
            for item in items {
                collect_pattern(item, uses);
            }
        }
        Pattern::Record { fields, .. } => {
            for field in fields {
                collect_pattern(field.pattern(), uses);
            }
        }
        Pattern::Whole { pattern, .. } => collect_pattern(pattern, uses),
        Pattern::Variant {
            payload: Some(payload),
            ..
        } => collect_variant_payload(payload, uses),
        Pattern::Ident(_)
        | Pattern::MutIdent(_)
        | Pattern::Variant { payload: None, .. }
        | Pattern::Discard
        | Pattern::Typed { .. } => {}
    }
}

fn collect_variant_payload(payload: &VariantPatternPayload, uses: &mut Vec<SymbolUse>) {
    match payload {
        VariantPatternPayload::Tuple(items) => {
            for item in items {
                collect_pattern(item, uses);
            }
        }
        VariantPatternPayload::Record { fields, .. } => {
            for field in fields {
                collect_pattern(field.pattern(), uses);
            }
        }
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
        | Stmt::Goto(expr)
        | Stmt::Yield(expr)
        | Stmt::Close(expr)
        | Stmt::Defer { expr, .. }
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
        Stmt::LetChoice { choice, .. } => collect_choice_stmt(choice, uses),
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
        Stmt::LetAwait { await_with, .. } => collect_unlowered_await_binding(await_with, uses),
        Stmt::Thread(thread) => collect_syntax_flow_block(thread.body(), uses),
        Stmt::DeferBlock { statements, .. } => collect_stmt_block(statements, uses),
        Stmt::Break {
            expr: Some(expr), ..
        }
        | Stmt::Select(expr)
        | Stmt::Out { expr, .. } => collect_expr(expr, uses),
        Stmt::LifetimeSet { target, expr } => {
            collect_expr(target, uses);
            collect_expr(expr, uses);
        }
        Stmt::Wait(target) => collect_wait_target(target, uses),
        Stmt::On { body, .. } | Stmt::Loop { body } => collect_stmt_block(body, uses),
        Stmt::UnsafeLifetime { reason, body, .. } => {
            if let Some(reason) = reason {
                collect_expr(reason, uses);
            }
            collect_stmt_block(body, uses);
        }
        Stmt::If { condition, body } | Stmt::While { condition, body } => {
            collect_expr(condition, uses);
            collect_stmt_block(body, uses);
        }
        Stmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
        } => {
            collect_pattern(pattern, uses);
            collect_expr(expr, uses);
            if let Some(guard) = guard {
                collect_expr(guard, uses);
            }
            collect_stmt_block(body, uses);
        }
        Stmt::For {
            pattern,
            source,
            body,
        } => {
            collect_pattern(pattern, uses);
            collect_expr(source, uses);
            collect_stmt_block(body, uses);
        }
        Stmt::Match { expr, arms } => collect_stmt_match(expr, arms, uses),
        Stmt::Break { expr: None, .. } | Stmt::Continue { .. } => {}
        Stmt::Raw(raw) => collect_raw_stmt(raw, uses),
    }
}

fn collect_raw_stmt(raw: &RawSyntax, uses: &mut Vec<SymbolUse>) {
    uses.push(SymbolUse::new(
        SymbolUseKind::RawExpr,
        raw.source().to_owned(),
    ));
}

fn collect_unlowered_await_binding(await_with: &AwaitWith, uses: &mut Vec<SymbolUse>) {
    uses.push(SymbolUse::new(
        SymbolUseKind::RawExpr,
        format!(
            "await expression binding with {} wait-view branches",
            await_with.branches().len()
        ),
    ));
}

fn collect_choice_stmt(choice: &ChoiceBlock, uses: &mut Vec<SymbolUse>) {
    if let Some(id) = choice.id() {
        push_id_ref(uses, id);
    }
    for option in choice.options() {
        if let Some(id) = option.id() {
            push_id_ref(uses, id);
        }
        if let Some(condition) = option.condition() {
            collect_expr(condition, uses);
        }
        if let Some(value) = option.value() {
            collect_expr(value, uses);
        }
        if let Some(text_key) = option.label_text_key() {
            push_id_ref(uses, text_key);
        }
        match option.action() {
            ChoiceAction::Out(expr) => collect_expr(expr, uses),
            ChoiceAction::SelectBlock(statements) => {
                collect_stmt_block(statements, uses);
            }
            ChoiceAction::Goto(target) => push_entity_syntax(uses, target),
            ChoiceAction::None => {}
        }
    }
}

fn collect_stmt_match(expr: &Expr, arms: &[StmtMatchArm], uses: &mut Vec<SymbolUse>) {
    collect_expr(expr, uses);
    for arm in arms {
        collect_pattern(arm.pattern(), uses);
        if let Some(guard) = arm.guard() {
            collect_expr(guard, uses);
        }
        collect_stmt_block(arm.body(), uses);
    }
}

fn collect_line_plan_item(item: &LinePlanItem, uses: &mut Vec<SymbolUse>) {
    match item {
        LinePlanItem::Init(statements) => {
            collect_stmt_block(statements, uses);
        }
        LinePlanItem::Thread(thread) => collect_syntax_flow_block(thread.body(), uses),
        LinePlanItem::On { trigger, body } => {
            collect_trigger_pattern(trigger, uses);
            collect_stmt_block(body, uses);
        }
        LinePlanItem::Stmt(stmt) => collect_stmt(stmt, uses),
        LinePlanItem::Option { value, .. }
        | LinePlanItem::Let { expr: value, .. }
        | LinePlanItem::Out(value) => collect_expr(value, uses),
        LinePlanItem::TimedCue { anchor, body } => {
            collect_expr(anchor, uses);
            collect_expr(body, uses);
        }
        LinePlanItem::CancelRule(rule) => collect_stmt_block(rule.action(), uses),
        LinePlanItem::Assert { expr, .. } | LinePlanItem::Expr(expr) => collect_expr(expr, uses),
        LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
            for item in items {
                collect_line_plan_item(item, uses);
            }
        }
        LinePlanItem::Raw(raw) => uses.push(SymbolUse::new(
            SymbolUseKind::RawExpr,
            raw.source().to_owned(),
        )),
    }
}

fn collect_trigger_pattern(trigger: &TriggerPattern, uses: &mut Vec<SymbolUse>) {
    match trigger {
        TriggerPattern::Input(pattern)
        | TriggerPattern::Event(pattern)
        | TriggerPattern::Mark(pattern)
        | TriggerPattern::Select(pattern)
        | TriggerPattern::Task(pattern)
        | TriggerPattern::Scope(pattern) => collect_pattern(pattern, uses),
        TriggerPattern::Signal { target, value } => {
            collect_expr(target, uses);
            if let Some(value) = value {
                collect_pattern(value, uses);
            }
        }
        TriggerPattern::Timeout(expr) | TriggerPattern::Expr(expr) => {
            collect_expr(expr, uses);
        }
    }
}

fn collect_dialogue_content(tokens: &[DialogueToken], uses: &mut Vec<SymbolUse>) {
    for token in tokens {
        if let DialogueToken::Expr(expr) = token {
            collect_expr(expr, uses);
        }
    }
}

fn collect_wait_target(target: &WaitTarget, uses: &mut Vec<SymbolUse>) {
    match target {
        WaitTarget::Duration(expr) | WaitTarget::Expr(expr) => {
            collect_expr(expr, uses);
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "symbol collection mirrors the public Expr enum so new syntax variants stay auditable"
)]
fn collect_expr(expr: &Expr, uses: &mut Vec<SymbolUse>) {
    match expr {
        Expr::Literal(_)
        | Expr::Placeholder(_)
        | Expr::LifetimePath { .. }
        | Expr::NumericBracketSeq(_) => {}
        Expr::EntityRef(entity) => push_entity_syntax(uses, entity),
        Expr::Path(path) => uses.push(SymbolUse::new(SymbolUseKind::Path, path.clone())),
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            for item in items {
                collect_expr(item, uses);
            }
        }
        Expr::ArrayRepeat { value, len } => {
            collect_expr(value, uses);
            collect_expr(len, uses);
        }
        Expr::Call { callee, args } => {
            if let Expr::Path(path) = callee.as_ref() {
                uses.push(SymbolUse::new(SymbolUseKind::Call, path.clone()));
            }
            collect_expr(callee, uses);
            for arg in args {
                collect_expr(arg.value(), uses);
            }
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            uses.push(SymbolUse::new(SymbolUseKind::Method, method.clone()));
            collect_expr(receiver, uses);
            for arg in args {
                collect_expr(arg.value(), uses);
            }
        }
        Expr::Field { target, .. } => collect_expr(target, uses),
        Expr::DialogueCall { callee, plan, .. } => {
            collect_dialogue_call_expr(callee, plan.as_ref(), uses);
        }
        Expr::Index { target, index } => {
            collect_expr(target, uses);
            collect_expr(index, uses);
        }
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
            collect_expr(lhs, uses);
            collect_expr(rhs, uses);
        }
        Expr::Closure { body, .. } => collect_expr(body, uses),
        Expr::Unary { expr, .. } | Expr::Try { expr } | Expr::Await { expr, .. } => {
            collect_expr(expr, uses);
        }
        Expr::Thread { block } => collect_syntax_flow_block(block.body(), uses),
        Expr::Range { start, end, .. } => {
            if let Some(start) = start {
                collect_expr(start, uses);
            }
            if let Some(end) = end {
                collect_expr(end, uses);
            }
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            for (_, value) in fields {
                collect_expr(value, uses);
            }
        }
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => collect_block_expr(statements, value.as_deref(), uses),
        Expr::MemoBlock {
            options,
            statements,
            value,
        } => {
            for (_, option) in options {
                collect_expr(option, uses);
            }
            collect_block_expr(statements, value.as_deref(), uses);
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => collect_if_expr(condition, then_branch, else_branch.as_deref(), uses),
        Expr::IfLet {
            pattern,
            expr,
            guard,
            then_branch,
            else_branch,
        } => collect_if_let_expr(
            pattern,
            expr,
            guard.as_deref(),
            then_branch,
            else_branch.as_deref(),
            uses,
        ),
        Expr::Match { scrutinee, arms } => collect_match_expr(scrutinee, arms, uses),
        Expr::Raw(raw) => uses.push(SymbolUse::new(SymbolUseKind::RawExpr, raw.clone())),
    }
}

fn collect_dialogue_call_expr(callee: &Expr, plan: Option<&LinePlan>, uses: &mut Vec<SymbolUse>) {
    collect_expr(callee, uses);
    if let Some(plan) = plan {
        for item in plan.items() {
            collect_line_plan_item(item, uses);
        }
    }
}

fn collect_block_expr(statements: &[Stmt], value: Option<&Expr>, uses: &mut Vec<SymbolUse>) {
    for stmt in statements {
        collect_stmt(stmt, uses);
    }
    if let Some(value) = value {
        collect_expr(value, uses);
    }
}

fn collect_if_expr(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    uses: &mut Vec<SymbolUse>,
) {
    collect_expr(condition, uses);
    collect_expr(then_branch, uses);
    if let Some(else_branch) = else_branch {
        collect_expr(else_branch, uses);
    }
}

fn collect_if_let_expr(
    pattern: &Pattern,
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    uses: &mut Vec<SymbolUse>,
) {
    collect_pattern(pattern, uses);
    collect_expr(expr, uses);
    if let Some(guard) = guard {
        collect_expr(guard, uses);
    }
    collect_expr(then_branch, uses);
    if let Some(else_branch) = else_branch {
        collect_expr(else_branch, uses);
    }
}

fn collect_match_expr(scrutinee: &Expr, arms: &[MatchExprArm], uses: &mut Vec<SymbolUse>) {
    collect_expr(scrutinee, uses);
    for arm in arms {
        collect_pattern(arm.pattern(), uses);
        if let Some(guard) = arm.guard() {
            collect_expr(guard, uses);
        }
        collect_expr(arm.value(), uses);
    }
}

fn push_entity(uses: &mut Vec<SymbolUse>, entity: &EntityRef) {
    uses.push(SymbolUse::new(
        SymbolUseKind::EntityRef,
        entity.body().to_owned(),
    ));
}

fn push_id_ref(uses: &mut Vec<SymbolUse>, id: &IdRef) {
    if let IdRef::Absolute(entity) = id {
        push_entity(uses, entity);
    }
}

fn push_entity_syntax(uses: &mut Vec<SymbolUse>, entity: &EntityRefSyntax) {
    if let EntityRefSyntax::Absolute(entity) = entity {
        push_entity(uses, entity);
    }
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
