use crate::ast::{ContractClause, DialogueToken, EntityRef, FlowKind, LinePlanItem, Pattern, Stmt};
use crate::expr::{BinaryOp, Expr, Literal};
use crate::lower::{HirFlowItem, HirModule};
use crate::symbols::{SymbolUseKind, collect_symbol_uses};
use crate::types::TypeRef;
use core::fmt;
use std::collections::HashMap;

/// Entity family inferred from an Arcweft public id prefix.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum EntityKind {
    Flow,
    Fragment,
    Choice,
    ChoiceOption,
    Character,
    Textbox,
    DialogueLine,
    Text,
    Asset,
    Animation,
    Hook,
    Signal,
    Scene,
    Other(String),
}

/// Minimal semantic type used by parser/HIR contract tests.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeKind {
    Bool,
    Int,
    Float,
    String,
    Duration,
    Range,
    DisplayText,
    Ref(EntityKind),
    Need {
        ready: Box<TypeKind>,
        error: Box<TypeKind>,
    },
    Result {
        ok: Box<TypeKind>,
        error: Box<TypeKind>,
    },
    Named(String),
    Tuple(Vec<TypeKind>),
    Unit,
}

/// Method signature known to the parser-side semantic checker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodSignature {
    return_type: TypeKind,
}

/// Small, explicit environment used to validate that HIR can feed type checking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeCheckEnv {
    symbols: HashMap<String, TypeKind>,
    functions: HashMap<String, TypeKind>,
    methods: HashMap<(TypeKind, String), MethodSignature>,
    indexes: HashMap<TypeKind, TypeKind>,
}

/// Semantic type-checking diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeCheckError {
    message: String,
}

/// Syntax-to-HIR readiness error for the future type checker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeCheckReadinessError {
    message: String,
}

/// Verifies that lowered HIR no longer contains raw expression fragments.
///
/// This is not the type checker. It is the parser/HIR contract check that keeps
/// later name resolution and type checking from silently reparsing source text.
pub fn validate_typecheck_ready(module: &HirModule) -> Result<(), Vec<TypeCheckReadinessError>> {
    let errors = collect_symbol_uses(module)
        .into_iter()
        .filter(|symbol| symbol.kind() == SymbolUseKind::RawExpr)
        .map(|symbol| {
            TypeCheckReadinessError::new(format!(
                "raw expression is not ready for type checking: {}",
                symbol.name()
            ))
        })
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Type-checks the lowered HIR with an explicit symbol/method environment.
///
/// This is deliberately small but real: it verifies entity reference families,
/// dialogue callees, awaited `Need<T, E>` values, timed cue durations, and
/// expression symbols without reparsing source text.
pub fn typecheck_hir(module: &HirModule, env: &TypeCheckEnv) -> Result<(), Vec<TypeCheckError>> {
    let mut checker = TypeChecker {
        env,
        errors: Vec::new(),
        active_borrows: Vec::new(),
        locals: HashMap::new(),
        loop_stack: Vec::new(),
    };
    checker.check_module(module);
    if checker.errors.is_empty() {
        Ok(())
    } else {
        Err(checker.errors)
    }
}

struct TypeChecker<'a> {
    env: &'a TypeCheckEnv,
    errors: Vec<TypeCheckError>,
    active_borrows: Vec<String>,
    locals: HashMap<String, TypeKind>,
    loop_stack: Vec<LoopContext>,
}

#[derive(Clone, Debug, Default)]
struct LoopContext {
    allows_value_break: bool,
    break_types: Vec<TypeKind>,
}

impl TypeChecker<'_> {
    fn check_module(&mut self, module: &HirModule) {
        if let Err(errors) = validate_typecheck_ready(module) {
            self.errors.extend(
                errors
                    .into_iter()
                    .map(|error| TypeCheckError::new(error.message().to_owned())),
            );
        }

        for flow in module.flows() {
            self.active_borrows.clear();
            self.locals.clear();
            self.loop_stack.clear();
            if let Some(id) = flow.id() {
                match flow.kind() {
                    FlowKind::Flow => self.expect_entity_kind(id, &EntityKind::Flow, "flow id"),
                    FlowKind::Fragment => {
                        self.expect_entity_kind(id, &EntityKind::Fragment, "fragment id");
                    }
                }
            }
            for contract in flow.contracts() {
                self.check_contract_clause(contract);
            }
            self.check_flow_items(flow.body());
        }
        self.check_flow_items(module.top_level_items());
    }

    fn check_flow_items(&mut self, items: &[HirFlowItem]) {
        for item in items {
            self.check_flow_item(item);
        }
    }

    fn check_flow_item(&mut self, item: &HirFlowItem) {
        match item {
            HirFlowItem::Stmt(stmt) => self.check_stmt(stmt),
            HirFlowItem::Dialogue(dialogue) => {
                self.check_dialogue_item(dialogue);
            }
            HirFlowItem::Choice(choice) => {
                self.check_choice(choice);
            }
            HirFlowItem::LetChoice { pattern, choice } => {
                self.check_choice(choice);
                if let Some(name) = ident_pattern_name(pattern) {
                    if let Some(ty) = choice_output_type(choice) {
                        self.locals.insert(name.to_owned(), ty);
                    }
                }
            }
            HirFlowItem::LetScope { pattern, scope } => {
                self.check_scope_expr_binding(pattern, scope);
            }
            HirFlowItem::LetLoop { pattern, block } => {
                let ty = self.check_loop_block(block, true);
                if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), ty) {
                    self.locals.insert(name.to_owned(), ty);
                }
            }
            HirFlowItem::If(block) => {
                self.expect_expr_type(block.condition(), &TypeKind::Bool, "if condition");
                self.check_flow_items(block.body());
            }
            HirFlowItem::IfLet(block) => {
                self.check_if_let_block(block);
            }
            HirFlowItem::Match(block) => {
                let expr_type = self.check_expr(block.expr());
                for arm in block.arms() {
                    let outer_locals = self.locals.clone();
                    for (name, ty) in let_else_bindings(arm.pattern(), expr_type.as_ref()) {
                        self.locals.insert(name, ty);
                    }
                    if let Some(guard) = arm.guard() {
                        self.expect_expr_type(guard, &TypeKind::Bool, "match arm guard");
                    }
                    self.check_flow_items(arm.body());
                    self.locals = outer_locals;
                }
            }
            HirFlowItem::Loop(block) => {
                self.check_loop_block(block, true);
            }
            HirFlowItem::While(block) => {
                self.check_while_block(block);
            }
            HirFlowItem::WhileLet(block) => {
                self.check_while_let_block(block);
            }
            HirFlowItem::For(block) => {
                self.check_for_block(block);
            }
            HirFlowItem::Select(block) => {
                self.check_select_block(block);
            }
            HirFlowItem::Borrow(block) => {
                self.check_borrow_block(block);
            }
            HirFlowItem::SourceLocale(block) => {
                self.check_flow_items(block.body());
            }
            HirFlowItem::Scope(block) => {
                self.check_flow_items(block.body());
            }
            HirFlowItem::Include(entity) => {
                let kind = entity_kind(entity);
                if !matches!(kind, Some(EntityKind::Fragment | EntityKind::Flow)) {
                    self.errors.push(TypeCheckError::new(format!(
                        "include target `{}` must be a flow or fragment reference",
                        entity.body()
                    )));
                }
            }
            HirFlowItem::Await(await_with) => {
                self.check_await_item(await_with);
            }
            HirFlowItem::Scenario { args, .. } => {
                for arg in args {
                    self.check_expr(arg);
                }
            }
        }
    }

    fn check_dialogue_item(&mut self, dialogue: &crate::lower::HirDialogue) {
        let callee_type = self.env.symbol_type(dialogue.callee());
        if !is_dialogue_callee_type(callee_type) {
            self.errors.push(TypeCheckError::new(format!(
                "dialogue callee `{}` must resolve to Ref<Character> or SpeakerPreset",
                dialogue.callee()
            )));
        }
        if let Some(id) = dialogue.id() {
            self.expect_entity_kind(id, &EntityKind::DialogueLine, "dialogue line id");
        }
        if let Some(text_key) = dialogue.text_key() {
            self.expect_entity_kind(text_key, &EntityKind::Text, "dialogue text key");
        }
        self.check_dialogue_content(dialogue.content().tokens());
        if let Some(plan) = dialogue.plan() {
            for item in plan.items() {
                self.check_line_plan_item(item);
            }
        }
    }

    fn check_await_item(&mut self, await_with: &crate::lower::HirAwait) {
        self.reject_active_borrows("await suspension boundary");
        let ty = self.check_expr(await_with.expr());
        if !matches!(ty, Some(TypeKind::Need { .. })) {
            self.errors.push(TypeCheckError::new(
                "await expression must have Need<T, E> type".to_owned(),
            ));
        }
        if await_with.branches().is_empty() {
            self.errors.push(TypeCheckError::new(
                "await with must define at least one wait-view branch".to_owned(),
            ));
        }
        for branch in await_with.branches() {
            self.check_flow_items(branch.body());
        }
    }

    fn check_loop_block(
        &mut self,
        block: &crate::lower::HirLoop,
        allows_value_break: bool,
    ) -> Option<TypeKind> {
        self.loop_stack.push(LoopContext {
            allows_value_break,
            break_types: Vec::new(),
        });
        self.check_flow_items(block.body());
        let context = self.loop_stack.pop()?;
        unify_loop_break_types(&context.break_types)
    }

    fn check_while_block(&mut self, block: &crate::lower::HirWhile) {
        self.expect_expr_type(block.condition(), &TypeKind::Bool, "while condition");
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
    }

    fn check_if_let_block(&mut self, block: &crate::lower::HirIfLet) {
        let expr_type = self.check_expr(block.expr());
        if let Some(guard) = block.guard() {
            self.expect_expr_type(guard, &TypeKind::Bool, "if-let guard");
        }
        let outer_locals = self.locals.clone();
        for (name, ty) in let_else_bindings(block.pattern(), expr_type.as_ref()) {
            self.locals.insert(name, ty);
        }
        self.check_flow_items(block.body());
        self.locals = outer_locals;
    }

    fn check_while_let_block(&mut self, block: &crate::lower::HirWhileLet) {
        let expr_type = self.check_expr(block.expr());
        if let Some(guard) = block.guard() {
            self.expect_expr_type(guard, &TypeKind::Bool, "while-let guard");
        }
        let outer_locals = self.locals.clone();
        for (name, ty) in let_else_bindings(block.pattern(), expr_type.as_ref()) {
            self.locals.insert(name, ty);
        }
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
        self.locals = outer_locals;
    }

    fn check_for_block(&mut self, block: &crate::lower::HirFor) {
        self.check_expr(block.source());
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
    }

    fn with_statement_loop(&mut self, check_body: impl FnOnce(&mut Self)) {
        self.loop_stack.push(LoopContext {
            allows_value_break: false,
            break_types: Vec::new(),
        });
        check_body(self);
        self.loop_stack.pop();
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { pattern, expr } => {
                let ty = self.check_expr(expr);
                if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), ty) {
                    self.locals.insert(name.to_owned(), ty);
                }
                collect_borrow_lifetimes(pattern, &mut self.active_borrows);
            }
            Stmt::LetElse {
                pattern,
                expr,
                else_body,
            } => {
                let expr_type = self.check_expr(expr);
                for stmt in else_body {
                    self.check_stmt(stmt);
                }
                if !stmts_diverge(else_body) {
                    self.errors.push(TypeCheckError::new(
                        "let-else else block must leave the current continuation".to_owned(),
                    ));
                }
                for (name, ty) in let_else_bindings(pattern, expr_type.as_ref()) {
                    self.locals.insert(name, ty);
                }
                collect_borrow_lifetimes(pattern, &mut self.active_borrows);
            }
            Stmt::LetChoice { .. } => {
                self.errors.push(TypeCheckError::new(
                    "choice expression binding must be lowered before type checking".to_owned(),
                ));
            }
            Stmt::LetScope { .. } => {
                self.errors.push(TypeCheckError::new(
                    "scope expression binding must be lowered before type checking".to_owned(),
                ));
            }
            Stmt::LetLoop { .. } => {
                self.errors.push(TypeCheckError::new(
                    "loop expression binding must be lowered before type checking".to_owned(),
                ));
            }
            Stmt::Return(expr)
            | Stmt::Out(expr)
            | Stmt::Close(expr)
            | Stmt::Expr(expr)
            | Stmt::Panic(expr)
            | Stmt::Fail(expr) => {
                self.check_expr(expr);
            }
            Stmt::Goto(expr) => {
                self.expect_expr_type(expr, &TypeKind::Ref(EntityKind::Flow), "goto destination");
            }
            Stmt::Spawn(expr) | Stmt::Defer(expr) | Stmt::Yield(expr) => {
                self.reject_active_borrows("suspension boundary");
                self.check_expr(expr);
            }
            Stmt::Signal { target, value } => {
                self.check_expr(target);
                self.check_expr(value);
            }
            Stmt::Break(expr) => self.check_break_stmt(expr.as_ref()),
            Stmt::Continue => self.check_continue_stmt(),
            Stmt::Raw(raw) => self.errors.push(TypeCheckError::new(format!(
                "raw statement is not type-checkable: {raw}"
            ))),
        }
    }

    fn check_break_stmt(&mut self, expr: Option<&Expr>) {
        let Some(index) = self.loop_stack.len().checked_sub(1) else {
            self.errors.push(TypeCheckError::new(
                "break is only allowed inside loop, while, or for".to_owned(),
            ));
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

    fn check_continue_stmt(&mut self) {
        if self.loop_stack.is_empty() {
            self.errors.push(TypeCheckError::new(
                "continue is only allowed inside loop, while, or for".to_owned(),
            ));
        }
    }

    fn check_choice(&mut self, choice: &crate::lower::HirChoice) {
        if let Some(id) = choice.id() {
            self.expect_entity_kind(id, &EntityKind::Choice, "choice id");
        }
        for option in choice.options() {
            if let Some(id) = option.id() {
                self.expect_entity_kind(id, &EntityKind::ChoiceOption, "choice option id");
            }
            if let Some(condition) = option.condition() {
                self.expect_expr_type(condition, &TypeKind::Bool, "choice condition");
            }
            if let Some(target) = option.target() {
                self.expect_entity_kind(target, &EntityKind::Flow, "choice target");
            }
            if let crate::ast::ChoiceAction::Out(expr) = option.action() {
                self.check_expr(expr);
            }
        }
    }

    fn check_scope_expr_binding(&mut self, pattern: &Pattern, scope: &crate::lower::HirScopeExpr) {
        let outer_locals = self.locals.clone();
        for stmt in scope.statements() {
            self.check_stmt(stmt);
        }
        let value_type = scope.value().and_then(|value| self.check_expr(value));
        self.locals = outer_locals;
        if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), value_type) {
            self.locals.insert(name.to_owned(), ty);
        }
    }

    fn check_select_block(&mut self, block: &crate::lower::HirSelect) {
        if block.branches().is_empty() {
            self.errors.push(TypeCheckError::new(
                "select block must define at least one branch".to_owned(),
            ));
        }
        for branch in block.branches() {
            self.check_select_head(branch.head());
            for item in branch.body() {
                self.check_flow_item(item);
            }
        }
    }

    fn check_borrow_block(&mut self, block: &crate::lower::HirBorrow) {
        self.check_expr(block.source());
        let borrow_start = self.active_borrows.len();
        let locals_start = self.locals.clone();
        collect_borrow_lifetimes(block.binding(), &mut self.active_borrows);
        if let Some((name, ty)) = typed_pattern_binding(block.binding()) {
            self.locals.insert(name.to_owned(), type_ref_kind(ty));
        }
        for item in block.body() {
            self.check_flow_item(item);
        }
        self.active_borrows.truncate(borrow_start);
        self.locals = locals_start;
    }

    fn check_select_head(&mut self, head: &crate::ast::SelectBranchHead) {
        match head {
            crate::ast::SelectBranchHead::Bind { source, .. } => {
                self.check_expr(source);
            }
            crate::ast::SelectBranchHead::Frame(pattern)
            | crate::ast::SelectBranchHead::Event(pattern) => {
                if let Pattern::Raw(raw) = pattern {
                    self.errors.push(TypeCheckError::new(format!(
                        "raw select branch pattern is not type-checkable: {raw}"
                    )));
                }
            }
            crate::ast::SelectBranchHead::Raw(raw) => self.errors.push(TypeCheckError::new(
                format!("raw select branch head is not type-checkable: {raw}"),
            )),
        }
    }

    fn check_contract_clause(&mut self, contract: &ContractClause) {
        match contract {
            ContractClause::Requires { expr, .. }
            | ContractClause::Ensures { expr, .. }
            | ContractClause::Invariant { expr, .. }
            | ContractClause::Assume { expr } => {
                self.expect_expr_type(expr, &TypeKind::Bool, "contract expression");
            }
            ContractClause::NoEffect(expr) | ContractClause::Decreases(expr) => {
                self.check_expr(expr);
            }
            ContractClause::Reads(items)
            | ContractClause::Effects(items)
            | ContractClause::Modifies(items) => {
                for item in items {
                    self.check_expr(item);
                }
            }
        }
    }

    fn reject_active_borrows(&mut self, boundary: &str) {
        if !self.active_borrows.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "borrowed values with lifetimes {:?} cannot cross {boundary}",
                self.active_borrows
            )));
        }
    }

    fn check_line_plan_item(&mut self, item: &LinePlanItem) {
        match item {
            LinePlanItem::Option { value, .. }
            | LinePlanItem::Let { expr: value, .. }
            | LinePlanItem::Out(value) => {
                self.check_expr(value);
            }
            LinePlanItem::TimedCue { anchor, body } => {
                self.expect_expr_type(anchor, &TypeKind::Duration, "timeline anchor");
                self.check_expr(body);
            }
            LinePlanItem::CancelRule(_)
            | LinePlanItem::StartGroup(_)
            | LinePlanItem::TogetherGroup(_)
            | LinePlanItem::Memo(_)
            | LinePlanItem::Assert(_) => {}
            LinePlanItem::Raw(raw) => self.errors.push(TypeCheckError::new(format!(
                "raw line-plan item is not type-checkable: {raw}"
            ))),
        }
    }

    fn check_dialogue_content(&mut self, tokens: &[DialogueToken]) {
        for token in tokens {
            if let DialogueToken::Expr(expr) = token {
                self.check_expr(expr);
            }
        }
    }

    fn expect_entity_kind(&mut self, entity: &EntityRef, expected: &EntityKind, context: &str) {
        let actual = entity_kind(entity);
        if actual.as_ref() == Some(expected)
            || (expected == &EntityKind::ChoiceOption && actual == Some(EntityKind::Choice))
        {
            return;
        }
        self.errors.push(TypeCheckError::new(format!(
            "{context} `{}` must be a {expected:?} reference",
            entity.body()
        )));
    }

    fn expect_expr_type(&mut self, expr: &Expr, expected: &TypeKind, context: &str) {
        let actual = self.check_expr(expr);
        if actual.as_ref() != Some(expected) {
            self.errors.push(TypeCheckError::new(format!(
                "{context} must have type {expected:?}, found {actual:?}"
            )));
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Option<TypeKind> {
        match expr {
            Expr::Literal(literal) => Some(literal_type(literal)),
            Expr::EntityRef(entity) => entity_kind(entity).map(TypeKind::Ref).or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown entity reference kind: {}",
                    entity.body()
                )));
                None
            }),
            Expr::Path(path) => self.locals.get(path).cloned().or_else(|| {
                self.env.symbol_type(path).cloned().or_else(|| {
                    self.errors
                        .push(TypeCheckError::new(format!("unknown symbol `{path}`")));
                    None
                })
            }),
            Expr::Placeholder(_) => None,
            Expr::Tuple(items) => Some(TypeKind::Tuple(
                items
                    .iter()
                    .filter_map(|item| self.check_expr(item))
                    .collect(),
            )),
            Expr::Call { callee, args } => self.check_call_expr(callee, args),
            Expr::NamedArg { value, .. } => self.check_expr(value),
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => self.check_method_call_expr(receiver, method, args),
            Expr::DialogueCall { callee, .. } => {
                self.check_expr(callee);
                Some(TypeKind::Named("DialogueLine".to_owned()))
            }
            Expr::Index { target, index } => self.check_index_expr(target, index),
            Expr::Pipe { lhs, rhs } => {
                self.check_expr(lhs);
                self.check_expr(rhs)
            }
            Expr::Try { expr } => self.check_try_expr(expr),
            Expr::Range { start, end, .. } => {
                if let Some(start) = start {
                    self.check_expr(start);
                }
                if let Some(end) = end {
                    self.check_expr(end);
                }
                Some(TypeKind::Range)
            }
            Expr::Binary { lhs, op, rhs } => self.check_binary_expr(lhs, *op, rhs),
            Expr::Block { statements, value } => {
                self.check_block_expr(statements, value.as_deref())
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => self.check_if_expr(condition, then_branch, else_branch.as_deref()),
            Expr::IfLet {
                pattern,
                expr,
                guard,
                then_branch,
                else_branch,
            } => self.check_if_let_expr(
                pattern,
                expr,
                guard.as_deref(),
                then_branch,
                else_branch.as_deref(),
            ),
            Expr::Match { scrutinee, arms } => self.check_match_expr(scrutinee, arms),
            Expr::Raw(raw) => {
                self.errors.push(TypeCheckError::new(format!(
                    "raw expression is not type-checkable: {raw}"
                )));
                None
            }
        }
    }

    fn check_call_expr(&mut self, callee: &Expr, args: &[Expr]) -> Option<TypeKind> {
        for arg in args {
            self.check_expr(arg);
        }
        if let Expr::Path(name) = callee {
            return self.env.function_type(name).cloned().or_else(|| {
                self.errors
                    .push(TypeCheckError::new(format!("unknown function `{name}`")));
                None
            });
        }
        self.check_expr(callee)
    }

    fn check_method_call_expr(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
    ) -> Option<TypeKind> {
        let receiver_type = self.check_expr(receiver);
        for arg in args {
            self.check_expr(arg);
        }
        receiver_type.and_then(|receiver_type| {
            self.env
                .method_type(&receiver_type, method)
                .cloned()
                .or_else(|| {
                    self.errors.push(TypeCheckError::new(format!(
                        "unknown method `{method}` on {receiver_type:?}"
                    )));
                    None
                })
        })
    }

    fn check_index_expr(&mut self, target: &Expr, index: &Expr) -> Option<TypeKind> {
        let target_type = self.check_expr(target);
        self.check_expr(index);
        target_type.and_then(|target_type| {
            self.env.index_type(&target_type).cloned().or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "type {target_type:?} is not indexable"
                )));
                None
            })
        })
    }

    fn check_try_expr(&mut self, expr: &Expr) -> Option<TypeKind> {
        match self.check_expr(expr) {
            Some(TypeKind::Result { ok, .. }) => Some(*ok),
            Some(TypeKind::Named(name)) => result_ok_type(&name).or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "`?` requires Result<T, E> or Option<T>, found Named({name:?})"
                )));
                None
            }),
            Some(other) => {
                self.errors.push(TypeCheckError::new(format!(
                    "`?` requires Result<T, E> or Option<T>, found {other:?}"
                )));
                None
            }
            None => None,
        }
    }

    fn check_block_expr(&mut self, statements: &[Stmt], value: Option<&Expr>) -> Option<TypeKind> {
        let outer_locals = self.locals.clone();
        for stmt in statements {
            self.check_stmt(stmt);
        }
        let ty = value.map_or(Some(TypeKind::Unit), |value| self.check_expr(value));
        self.locals = outer_locals;
        ty
    }

    fn check_if_expr(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Option<TypeKind> {
        self.expect_expr_type(condition, &TypeKind::Bool, "if expression condition");
        let then_type = self.check_expr(then_branch);
        let else_type = else_branch.and_then(|branch| self.check_expr(branch));
        match (then_type, else_type) {
            (Some(then_type), Some(else_type)) if then_type == else_type => Some(then_type),
            (Some(then_type), Some(else_type)) => {
                self.errors.push(TypeCheckError::new(format!(
                    "if expression branches must have the same type, found {then_type:?} and {else_type:?}"
                )));
                None
            }
            _ => None,
        }
    }

    fn check_match_expr(
        &mut self,
        scrutinee: &Expr,
        arms: &[crate::expr::MatchExprArm],
    ) -> Option<TypeKind> {
        let scrutinee_type = self.check_expr(scrutinee);
        if arms.is_empty() {
            self.errors.push(TypeCheckError::new(
                "match expression must have at least one arm".to_owned(),
            ));
            return None;
        }

        let mut inferred = None;
        for arm in arms {
            let outer_locals = self.locals.clone();
            for (name, ty) in let_else_bindings(arm.pattern(), scrutinee_type.as_ref()) {
                self.locals.insert(name, ty);
            }
            if let Some(guard) = arm.guard() {
                self.expect_expr_type(guard, &TypeKind::Bool, "match arm guard");
            }
            let arm_type = self.check_expr(arm.value());
            self.locals = outer_locals;
            match (&inferred, arm_type) {
                (None, Some(ty)) => inferred = Some(ty),
                (Some(existing), Some(ty)) if existing == &ty => {}
                (Some(existing), Some(ty)) => {
                    self.errors.push(TypeCheckError::new(format!(
                        "match expression arms must have the same type, found {existing:?} and {ty:?}"
                    )));
                    return None;
                }
                (_, None) => return None,
            }
        }
        inferred
    }

    fn check_if_let_expr(
        &mut self,
        pattern: &Pattern,
        expr: &Expr,
        guard: Option<&Expr>,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Option<TypeKind> {
        let expr_type = self.check_expr(expr);
        if let Some(guard) = guard {
            self.expect_expr_type(guard, &TypeKind::Bool, "if-let expression guard");
        }

        let outer_locals = self.locals.clone();
        for (name, ty) in let_else_bindings(pattern, expr_type.as_ref()) {
            self.locals.insert(name, ty);
        }
        let then_type = self.check_expr(then_branch);
        self.locals = outer_locals;

        let else_type = else_branch.and_then(|branch| self.check_expr(branch));
        match (then_type, else_type) {
            (Some(then_type), Some(else_type)) if then_type == else_type => Some(then_type),
            (Some(then_type), Some(else_type)) => {
                self.errors.push(TypeCheckError::new(format!(
                    "if-let expression branches must have the same type, found {then_type:?} and {else_type:?}"
                )));
                None
            }
            _ => None,
        }
    }

    fn check_binary_expr(&mut self, lhs: &Expr, op: BinaryOp, rhs: &Expr) -> Option<TypeKind> {
        let lhs_type = self.check_expr(lhs);
        let rhs_type = self.check_expr(rhs);
        match op {
            BinaryOp::In => {
                if rhs_type != Some(TypeKind::Range) {
                    self.errors.push(TypeCheckError::new(format!(
                        "`in` expression requires a range on the right, found {rhs_type:?}"
                    )));
                    return None;
                }
                Some(TypeKind::Bool)
            }
            BinaryOp::Implies | BinaryOp::Or | BinaryOp::And => {
                if lhs_type != Some(TypeKind::Bool) || rhs_type != Some(TypeKind::Bool) {
                    self.errors.push(TypeCheckError::new(format!(
                        "logical contract expression must use Bool operands, found {lhs_type:?} and {rhs_type:?}"
                    )));
                    return None;
                }
                Some(TypeKind::Bool)
            }
            BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Gte
            | BinaryOp::Lte
            | BinaryOp::Gt
            | BinaryOp::Lt => Some(TypeKind::Bool),
            BinaryOp::Add | BinaryOp::Sub => {
                if lhs_type == Some(TypeKind::Duration) && rhs_type == Some(TypeKind::Duration) {
                    Some(TypeKind::Duration)
                } else if lhs_type == Some(TypeKind::Int) && rhs_type == Some(TypeKind::Int) {
                    Some(TypeKind::Int)
                } else if lhs_type == Some(TypeKind::Float) && rhs_type == Some(TypeKind::Float) {
                    Some(TypeKind::Float)
                } else {
                    self.errors.push(TypeCheckError::new(format!(
                        "arithmetic expression operands must have a supported numeric or Duration type, found {lhs_type:?} and {rhs_type:?}"
                    )));
                    None
                }
            }
        }
    }
}

fn entity_kind(entity: &EntityRef) -> Option<EntityKind> {
    let head = entity.body().split(['.', '@', ':']).next()?;
    Some(match head {
        "flow" => EntityKind::Flow,
        "frag" | "fragment" => EntityKind::Fragment,
        "choice" => EntityKind::Choice,
        "character" => EntityKind::Character,
        "textbox" => EntityKind::Textbox,
        "say" => EntityKind::DialogueLine,
        "text" => EntityKind::Text,
        "item" => EntityKind::Other("item".to_owned()),
        "asset" => EntityKind::Asset,
        "anim" => EntityKind::Animation,
        "hook" => EntityKind::Hook,
        "signal" => EntityKind::Signal,
        "scene" => EntityKind::Scene,
        "ent" => EntityKind::Other("ent".to_owned()),
        _ => return None,
    })
}

fn literal_type(literal: &Literal) -> TypeKind {
    match literal {
        Literal::String(_) => TypeKind::String,
        Literal::Int(_) => TypeKind::Int,
        Literal::Float(_) => TypeKind::Float,
        Literal::Bool(_) => TypeKind::Bool,
        Literal::Duration { .. } => TypeKind::Duration,
    }
}

fn is_dialogue_callee_type(ty: Option<&TypeKind>) -> bool {
    matches!(ty, Some(TypeKind::Ref(EntityKind::Character)))
        || matches!(ty, Some(TypeKind::Named(name)) if name == "SpeakerPreset")
}

fn collect_borrow_lifetimes(pattern: &Pattern, lifetimes: &mut Vec<String>) {
    match pattern {
        Pattern::Typed { ty, .. } => collect_type_lifetimes(ty, lifetimes),
        Pattern::Tuple(items) | Pattern::List { items, .. } => {
            for item in items {
                collect_borrow_lifetimes(item, lifetimes);
            }
        }
        Pattern::Record { fields, .. } => {
            for field in fields {
                collect_borrow_lifetimes(field.pattern(), lifetimes);
            }
        }
        Pattern::Whole { pattern, .. } => collect_borrow_lifetimes(pattern, lifetimes),
        Pattern::Ident(_)
        | Pattern::MutIdent(_)
        | Pattern::Literal(_)
        | Pattern::Entity(_)
        | Pattern::Variant(_)
        | Pattern::Discard
        | Pattern::Raw(_) => {}
    }
}

fn typed_pattern_binding(pattern: &Pattern) -> Option<(&str, &TypeRef)> {
    match pattern {
        Pattern::Typed { name, ty } => Some((name, ty)),
        _ => None,
    }
}

fn ident_pattern_name(pattern: &Pattern) -> Option<&str> {
    match pattern {
        Pattern::Ident(name) => Some(name),
        _ => None,
    }
}

fn let_else_bindings(pattern: &Pattern, expr_type: Option<&TypeKind>) -> Vec<(String, TypeKind)> {
    match pattern {
        Pattern::Ident(name) => expr_type
            .cloned()
            .map(|ty| vec![(name.to_owned(), ty)])
            .unwrap_or_default(),
        Pattern::MutIdent(name) => expr_type
            .cloned()
            .map(|ty| vec![(name.to_owned(), ty)])
            .unwrap_or_default(),
        Pattern::Variant(raw) => variant_payload_binding(raw)
            .into_iter()
            .filter_map(|name| option_payload_type(expr_type).map(|ty| (name, ty)))
            .collect(),
        Pattern::Tuple(items) => items
            .iter()
            .flat_map(|item| let_else_bindings(item, None))
            .collect(),
        Pattern::List { items, .. } => items
            .iter()
            .flat_map(|item| let_else_bindings(item, None))
            .collect(),
        Pattern::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| let_else_bindings(field.pattern(), None))
            .collect(),
        Pattern::Whole { name, pattern } => {
            let mut bindings = expr_type
                .cloned()
                .map(|ty| vec![(name.to_owned(), ty)])
                .unwrap_or_default();
            bindings.extend(let_else_bindings(pattern, expr_type));
            bindings
        }
        Pattern::Typed { name, ty } => vec![(name.to_owned(), type_ref_kind(ty))],
        Pattern::Literal(_) | Pattern::Entity(_) | Pattern::Discard | Pattern::Raw(_) => Vec::new(),
    }
}

fn variant_payload_binding(raw: &str) -> Option<String> {
    raw.strip_prefix(".Some(")
        .and_then(|rest| rest.strip_suffix(')'))
        .map(str::trim)
        .filter(|name| is_local_ident(name))
        .map(str::to_owned)
}

fn option_payload_type(expr_type: Option<&TypeKind>) -> Option<TypeKind> {
    match expr_type {
        Some(TypeKind::Named(name)) if name == "Option<Ref<Flow>>" => {
            Some(TypeKind::Ref(EntityKind::Flow))
        }
        Some(TypeKind::Named(name)) if name == "Option<Bool>" => Some(TypeKind::Bool),
        Some(TypeKind::Named(name)) if name == "Option<Int>" => Some(TypeKind::Int),
        Some(TypeKind::Named(name)) if name == "Option<String>" => Some(TypeKind::String),
        _ => None,
    }
}

fn result_ok_type(name: &str) -> Option<TypeKind> {
    let inner = name
        .strip_prefix("Result<")
        .and_then(|value| value.strip_suffix('>'))?;
    let ok = inner.split_once(',').map_or(inner, |(ok, _)| ok).trim();
    Some(named_type_label(ok))
}

fn named_type_label(name: &str) -> TypeKind {
    match name {
        "Bool" => TypeKind::Bool,
        "Int" => TypeKind::Int,
        "Float" => TypeKind::Float,
        "String" => TypeKind::String,
        "Duration" => TypeKind::Duration,
        "Unit" => TypeKind::Unit,
        other => TypeKind::Named(other.to_owned()),
    }
}

fn unify_loop_break_types(types: &[TypeKind]) -> Option<TypeKind> {
    let first = types.first()?.clone();
    if types.iter().all(|ty| ty == &first) {
        Some(first)
    } else {
        None
    }
}

fn stmts_diverge(stmts: &[Stmt]) -> bool {
    stmts.last().is_some_and(stmt_diverges)
}

fn stmt_diverges(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_)
        | Stmt::Goto(_)
        | Stmt::Break(_)
        | Stmt::Continue
        | Stmt::Panic(_)
        | Stmt::Fail(_) => true,
        Stmt::Raw(raw) => {
            raw.starts_with("break") || raw.starts_with("panic ") || raw.starts_with("fail ")
        }
        _ => false,
    }
}

fn is_local_ident(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn choice_output_type(choice: &crate::lower::HirChoice) -> Option<TypeKind> {
    let mut inferred = None;
    for option in choice.options() {
        let crate::ast::ChoiceAction::Out(expr) = option.action() else {
            return None;
        };
        let ty = simple_expr_type(expr)?;
        match &inferred {
            Some(existing) if existing != &ty => return None,
            Some(_) => {}
            None => inferred = Some(ty),
        }
    }
    inferred
}

fn simple_expr_type(expr: &Expr) -> Option<TypeKind> {
    match expr {
        Expr::EntityRef(entity) => entity_kind(entity).map(TypeKind::Ref),
        Expr::Literal(literal) => Some(literal_type(literal)),
        Expr::Tuple(items) => items
            .iter()
            .map(simple_expr_type)
            .collect::<Option<Vec<_>>>()
            .map(TypeKind::Tuple),
        _ => None,
    }
}

fn collect_type_lifetimes(ty: &TypeRef, lifetimes: &mut Vec<String>) {
    match ty {
        TypeRef::Ref { lifetime, inner } => {
            if let Some(lifetime) = lifetime {
                let name = lifetime.name();
                if name != "static" {
                    lifetimes.push(name.to_owned());
                }
            }
            collect_type_lifetimes(inner, lifetimes);
        }
        TypeRef::Generic { args, .. } => {
            for arg in args {
                collect_type_lifetimes(arg, lifetimes);
            }
        }
        TypeRef::Slice(inner) => collect_type_lifetimes(inner, lifetimes),
        TypeRef::Path(_) => {}
    }
}

fn type_ref_kind(ty: &TypeRef) -> TypeKind {
    TypeKind::Named(type_ref_label(ty))
}

fn type_ref_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Path(path) => path.clone(),
        TypeRef::Generic { base, args } => format!(
            "{base}<{}>",
            args.iter()
                .map(type_ref_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::Ref { lifetime, inner } => {
            let lifetime = lifetime
                .as_ref()
                .map(|lifetime| format!("'{} ", lifetime.name()))
                .unwrap_or_default();
            format!("&{lifetime}{}", type_ref_label(inner))
        }
        TypeRef::Slice(inner) => format!("[{}]", type_ref_label(inner)),
    }
}

impl TypeCheckReadinessError {
    fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable readiness failure.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for TypeCheckReadinessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TypeCheckReadinessError {}

impl TypeCheckEnv {
    /// Creates an empty type-checking environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a variable, constant, or resolved path.
    #[must_use]
    pub fn with_symbol(mut self, name: impl Into<String>, ty: TypeKind) -> Self {
        self.symbols.insert(name.into(), ty);
        self
    }

    /// Registers a free function return type.
    #[must_use]
    pub fn with_function(mut self, name: impl Into<String>, return_type: TypeKind) -> Self {
        self.functions.insert(name.into(), return_type);
        self
    }

    /// Registers a method return type for a receiver type.
    #[must_use]
    pub fn with_method(
        mut self,
        receiver: TypeKind,
        method: impl Into<String>,
        return_type: TypeKind,
    ) -> Self {
        self.methods
            .insert((receiver, method.into()), MethodSignature { return_type });
        self
    }

    /// Registers index result type for a collection-like type.
    #[must_use]
    pub fn with_index(mut self, target: TypeKind, return_type: TypeKind) -> Self {
        self.indexes.insert(target, return_type);
        self
    }

    fn symbol_type(&self, name: &str) -> Option<&TypeKind> {
        self.symbols.get(name)
    }

    fn function_type(&self, name: &str) -> Option<&TypeKind> {
        self.functions.get(name)
    }

    fn method_type(&self, receiver: &TypeKind, method: &str) -> Option<&TypeKind> {
        self.methods
            .get(&(receiver.clone(), method.to_owned()))
            .map(|signature| &signature.return_type)
    }

    fn index_type(&self, target: &TypeKind) -> Option<&TypeKind> {
        self.indexes.get(target)
    }
}

impl TypeCheckError {
    fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable type-checking failure.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for TypeCheckError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TypeCheckError {}
