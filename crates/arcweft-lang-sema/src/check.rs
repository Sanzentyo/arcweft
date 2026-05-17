use crate::symbols::{SymbolUseKind, collect_symbol_uses};
use arcweft_lang_hir::{HirFlowItem, HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::TypeRef;
use arcweft_lang_syntax::{
    AwaitBranchKind, ContractClause, DialogueToken, EntityDeclKind, EntityRef, EntityRefSyntax,
    FlowKind, IdRef, LinePlanItem, Pattern, Stmt, TriggerPattern,
};
use arcweft_lang_syntax::{
    BinaryOp, Expr, LifetimeAccessMode, LifetimeKey, LifetimeScopeKind, Literal,
};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Entity family inferred from an Arcweft public id prefix.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum EntityKind {
    Flow,
    Fragment,
    Choice,
    ChoiceOption,
    Character,
    Component,
    Activity,
    Textbox,
    DialogueLine,
    Text,
    Asset,
    Animation,
    Capture,
    Hook,
    Signal,
    Scene,
    Source,
    Test,
    Bench,
    Layer,
    Voice,
    Se,
    Bgm,
    AudioBus,
    MixerSnapshot,
    Ducking,
    Motion,
    Rig,
    Slot,
    Target,
    Other(String),
}

/// Minimal semantic type used by parser/HIR contract tests.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeKind {
    Bool,
    Int,
    Float,
    String,
    Char,
    TextCluster,
    Duration,
    Range,
    DisplayText,
    Ref(EntityKind),
    List(Box<TypeKind>),
    Slice(Box<TypeKind>),
    Seq(Box<TypeKind>),
    Map {
        key: Box<TypeKind>,
        value: Box<TypeKind>,
    },
    BorrowRef {
        lifetime: Option<LifetimeScopeKind>,
        inner: Box<TypeKind>,
    },
    Need {
        ready: Box<TypeKind>,
        error: Box<TypeKind>,
    },
    Result {
        ok: Box<TypeKind>,
        error: Box<TypeKind>,
    },
    Option(Box<TypeKind>),
    Handle {
        name: String,
        lifetime: LifetimeScopeKind,
        state: HandleState,
        must_drop: bool,
    },
    ThreadHandle(Box<TypeKind>),
    Shared(Box<TypeKind>),
    Function {
        return_type: Box<TypeKind>,
    },
    Speaker(EntityKind),
    SpeakerPreset(EntityKind),
    CharacterPatch(EntityKind),
    FocusPatch,
    Named(String),
    Tuple(Vec<TypeKind>),
    Unit,
    Never,
}

/// Minimal typestate for scoped handles tracked by the syntax checker.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HandleState {
    Live,
    Dropped,
    Detached,
    MovedOut,
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
    capabilities: HashSet<String>,
}

/// Semantic type-checking diagnostic.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckError {
    message: String,
}

/// Syntax-to-HIR readiness error for the future type checker.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
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
        line_label_stack: Vec::new(),
        line_cancel_depth: 0,
        active_presentation_defaults: HashMap::new(),
        line_mark_stack: Vec::new(),
        lifetime_guarantees: HashSet::new(),
        dropped_lifetime_keys: HashSet::new(),
        available_lifetimes: Vec::new(),
        effect_capabilities: env.capabilities.clone(),
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
    line_label_stack: Vec<Option<String>>,
    line_cancel_depth: usize,
    active_presentation_defaults: HashMap<String, String>,
    line_mark_stack: Vec<HashSet<String>>,
    lifetime_guarantees: HashSet<LifetimeKey>,
    dropped_lifetime_keys: HashSet<LifetimeKey>,
    available_lifetimes: Vec<LifetimeScopeKind>,
    effect_capabilities: HashSet<String>,
}

#[derive(Clone, Debug)]
struct TypeCheckerScopeSnapshot {
    active_borrows: Vec<String>,
    locals: HashMap<String, TypeKind>,
    active_presentation_defaults: HashMap<String, String>,
    lifetime_guarantees: HashSet<LifetimeKey>,
    dropped_lifetime_keys: HashSet<LifetimeKey>,
    available_lifetimes: Vec<LifetimeScopeKind>,
}

#[derive(Clone, Debug, Default)]
struct LoopContext {
    label: Option<String>,
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
            self.active_presentation_defaults.clear();
            self.line_mark_stack.clear();
            self.lifetime_guarantees.clear();
            self.dropped_lifetime_keys.clear();
            if let Some(signature) = flow.signature() {
                for group in signature.param_groups() {
                    for param in group.params() {
                        self.bind_function_param(param.pattern(), &type_ref_kind(param.ty()));
                    }
                }
            }
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
        for function in module.functions() {
            self.active_borrows.clear();
            self.locals.clear();
            self.loop_stack.clear();
            self.active_presentation_defaults.clear();
            for group in function.signature().param_groups() {
                for param in group.params() {
                    self.bind_function_param(param.pattern(), &type_ref_kind(param.ty()));
                }
            }
            for contract in function.contracts() {
                self.check_contract_clause(contract);
            }
            let actual = self.check_block_expr(function.statements(), function.value());
            if let (Some(expected), Some(actual)) = (
                function.signature().return_type().map(type_ref_kind),
                actual,
            ) {
                if actual != expected {
                    self.errors.push(TypeCheckError::new(format!(
                        "function `{}` returns {expected:?}, but body has {actual:?}",
                        function.name()
                    )));
                }
            }
        }
        for declaration in module.declarations() {
            self.check_top_level_decl(declaration);
        }
        self.check_flow_items(module.top_level_items());
    }

    fn check_top_level_decl(&mut self, declaration: &HirTopLevelDecl) {
        match declaration {
            HirTopLevelDecl::Attribute(_)
            | HirTopLevelDecl::DialogueDefaults(_)
            | HirTopLevelDecl::Enum(_)
            | HirTopLevelDecl::ExternMod(_)
            | HirTopLevelDecl::Impl(_)
            | HirTopLevelDecl::Proof(_)
            | HirTopLevelDecl::Struct(_)
            | HirTopLevelDecl::Trait(_)
            | HirTopLevelDecl::TrustedAxiom(_) => {}
            HirTopLevelDecl::Test(item) => {
                if let Some(id) = item.id().as_absolute() {
                    self.expect_entity_kind(id, &EntityKind::Test, "test id");
                }
            }
            HirTopLevelDecl::Bench(item) => {
                if let Some(id) = item.id().as_absolute() {
                    self.expect_entity_kind(id, &EntityKind::Bench, "bench id");
                }
            }
            HirTopLevelDecl::EntityDecl(item) => {
                self.expect_entity_kind(
                    item.id(),
                    &entity_kind_for_decl(item.kind()),
                    "entity declaration id",
                );
            }
            HirTopLevelDecl::Callable(item) => {
                self.active_borrows.clear();
                self.locals.clear();
                self.loop_stack.clear();
                for contract in item.contracts() {
                    self.check_contract_clause(contract);
                }
            }
            HirTopLevelDecl::State(item) => {
                self.active_borrows.clear();
                self.locals.clear();
                self.loop_stack.clear();
                for field in item.fields() {
                    self.check_expr(field.default());
                }
            }
            HirTopLevelDecl::TypeAlias(item) => {
                for clause in item.where_clauses() {
                    self.check_expr(clause);
                }
            }
            HirTopLevelDecl::Hook(item) => {
                self.expect_entity_kind(item.id(), &EntityKind::Hook, "hook id");
                self.check_block_expr(item.body_statements(), None);
            }
            HirTopLevelDecl::MemoFn(item) => {
                self.active_borrows.clear();
                self.locals.clear();
                self.loop_stack.clear();
                self.check_block_expr(item.body_statements(), item.body_value());
            }
            HirTopLevelDecl::Parser(item) => {
                self.active_borrows.clear();
                self.locals.clear();
                self.loop_stack.clear();
                self.check_block_expr(item.body_statements(), item.body_value());
            }
            HirTopLevelDecl::Source(item) => {
                self.active_borrows.clear();
                self.locals.clear();
                self.loop_stack.clear();
                if let Some(id) = item.id() {
                    self.expect_entity_kind(id, &EntityKind::Source, "source id");
                }
                self.check_block_expr(item.body_statements(), None);
            }
        }
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
                self.check_choice_binding(pattern, choice);
            }
            HirFlowItem::LetScope { pattern, scope } => {
                self.check_scope_expr_binding(pattern, scope);
            }
            HirFlowItem::LetLoop { pattern, block } => {
                self.check_loop_binding(pattern, block);
            }
            HirFlowItem::LetAwait {
                pattern,
                await_with,
            } => {
                self.check_await_binding(pattern, await_with);
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
                self.check_scoped_flow_items(block.body());
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

    fn check_scoped_flow_items(&mut self, items: &[HirFlowItem]) {
        let outer_locals = self.locals.clone();
        let outer_presentation_defaults = self.active_presentation_defaults.clone();
        self.check_flow_items(items);
        self.locals = outer_locals;
        self.active_presentation_defaults = outer_presentation_defaults;
    }

    fn check_choice_binding(&mut self, pattern: &Pattern, choice: &arcweft_lang_hir::HirChoice) {
        self.check_choice(choice);
        if let Some(name) = ident_pattern_name(pattern)
            && let Some(ty) = choice_output_type(choice)
        {
            self.locals.insert(name.to_owned(), ty);
        }
    }

    fn check_loop_binding(&mut self, pattern: &Pattern, block: &arcweft_lang_hir::HirLoop) {
        let ty = self.check_loop_block(block, true);
        if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), ty) {
            self.locals.insert(name.to_owned(), ty);
        }
    }

    fn check_await_binding(&mut self, pattern: &Pattern, await_with: &arcweft_lang_hir::HirAwait) {
        let ty = self.check_await_item(await_with);
        if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), ty) {
            self.locals.insert(name.to_owned(), ty);
        }
    }

    fn check_dialogue_item(&mut self, dialogue: &arcweft_lang_hir::HirDialogue) {
        if !self.is_dialogue_callee(dialogue.callee()) {
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
        if let Some(look) = dialogue.look() {
            self.check_expr(look);
        }
        if let Some(stage) = dialogue.stage() {
            self.check_expr(stage);
        }
        if let Some(portrait) = dialogue.portrait() {
            self.check_expr(portrait);
        }
        let marks = self.check_dialogue_content(dialogue.content().tokens());
        self.with_line_runtime_scope(|checker| {
            if let Some(focus) = dialogue.focus() {
                checker.check_expr(focus);
                checker.lifetime_guarantees.insert(LifetimeKey::new(
                    LifetimeScopeKind::Line,
                    vec!["focus".to_owned()],
                ));
            }
            if let Some(cleanup) = dialogue.cleanup() {
                checker.check_expr(cleanup);
            }
            if let Some(plan) = dialogue.plan() {
                checker
                    .line_label_stack
                    .push(plan.label().map(str::to_owned));
                checker.line_mark_stack.push(marks);
                for item in plan.items() {
                    checker.check_line_plan_item(item);
                }
                checker.line_mark_stack.pop();
                checker.line_label_stack.pop();
            }
        });
    }

    fn bind_function_param(&mut self, pattern: &Pattern, ty: &TypeKind) {
        for (name, binding_ty) in pattern_bindings_with_fallback(pattern, ty) {
            self.locals.insert(name, binding_ty);
        }
    }

    fn is_dialogue_callee(&self, callee: &str) -> bool {
        if is_dialogue_callee_type(self.env.symbol_type(callee)) {
            return true;
        }
        callee.strip_suffix(".say").is_some_and(|receiver| {
            is_dialogue_callee_type(self.env.symbol_type(receiver))
                || is_character_entity_literal(receiver)
        })
    }

    fn check_await_item(&mut self, await_with: &arcweft_lang_hir::HirAwait) -> Option<TypeKind> {
        self.reject_active_borrows("await suspension boundary");
        let ty = self.check_expr(await_with.expr());
        let Some(TypeKind::Need { ready, error }) = ty else {
            self.errors.push(TypeCheckError::new(
                "await expression must have Need<T, E> type".to_owned(),
            ));
            return None;
        };
        if await_with.branches().is_empty() {
            self.errors.push(TypeCheckError::new(
                "await with must define at least one wait-view branch".to_owned(),
            ));
        }
        for branch in await_with.branches() {
            let outer_locals = self.locals.clone();
            let branch_type = await_branch_pattern_type(branch.kind(), &ready, &error);
            for (name, ty) in let_else_bindings(branch.pattern(), Some(&branch_type)) {
                self.locals.insert(name, ty);
            }
            self.check_flow_items(branch.body());
            self.locals = outer_locals;
        }

        if await_with.applies_try() {
            Some(*ready)
        } else {
            Some(TypeKind::Result { ok: ready, error })
        }
    }

    fn check_loop_block(
        &mut self,
        block: &arcweft_lang_hir::HirLoop,
        allows_value_break: bool,
    ) -> Option<TypeKind> {
        self.loop_stack.push(LoopContext {
            label: block.label().map(str::to_owned),
            allows_value_break,
            break_types: Vec::new(),
        });
        self.check_flow_items(block.body());
        let context = self.loop_stack.pop()?;
        unify_loop_break_types(&context.break_types)
    }

    fn check_while_block(&mut self, block: &arcweft_lang_hir::HirWhile) {
        self.expect_expr_type(block.condition(), &TypeKind::Bool, "while condition");
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
    }

    fn check_if_let_block(&mut self, block: &arcweft_lang_hir::HirIfLet) {
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

    fn check_while_let_block(&mut self, block: &arcweft_lang_hir::HirWhileLet) {
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

    fn check_for_block(&mut self, block: &arcweft_lang_hir::HirFor) {
        self.check_expr(block.source());
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
    }

    fn with_statement_loop(&mut self, check_body: impl FnOnce(&mut Self)) {
        self.loop_stack.push(LoopContext {
            label: None,
            allows_value_break: false,
            break_types: Vec::new(),
        });
        check_body(self);
        self.loop_stack.pop();
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { pattern, ty, expr } => self.check_let_stmt(pattern, ty.as_ref(), expr),
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
            Stmt::Return(expr)
            | Stmt::Close(expr)
            | Stmt::Expr(expr)
            | Stmt::Panic(expr)
            | Stmt::Fail(expr)
            | Stmt::Bail(expr)
            | Stmt::Select(expr) => {
                self.check_expr(expr);
            }
            Stmt::Out { label, expr } => {
                self.check_out_stmt(label.as_deref(), expr);
            }
            Stmt::Ensure { condition, message } => self.check_ensure_stmt(condition, message),
            Stmt::Goto(expr) => {
                self.expect_expr_type(expr, &TypeKind::Ref(EntityKind::Flow), "goto destination");
            }
            Stmt::Thread(thread) => {
                self.reject_active_borrows("thread suspension boundary");
                for stmt in thread.body() {
                    self.check_stmt(stmt);
                }
            }
            Stmt::DeferBlock { statements, .. } => {
                self.reject_active_borrows("defer cleanup boundary");
                for stmt in statements {
                    self.check_stmt(stmt);
                }
            }
            Stmt::Defer { expr, .. } | Stmt::Yield(expr) => {
                self.reject_active_borrows("suspension boundary");
                self.check_expr(expr);
            }
            Stmt::Signal { target, value } => self.check_two_exprs(target, value),
            Stmt::LifetimeSet { target, expr } => self.check_lifetime_set_stmt(target, expr),
            Stmt::Wait(target) => self.check_wait_stmt(target),
            Stmt::On { body, .. } => self.check_on_stmt(stmt, body),
            Stmt::UnsafeLifetime {
                reason,
                has_safety_doc,
                body,
                ..
            } => self.check_unsafe_lifetime_stmt(reason.as_ref(), *has_safety_doc, body),
            Stmt::Command(command) => self.check_command_stmt(command),
            Stmt::If { condition, body } => self.check_if_stmt(condition, body),
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
                "raw statement is not type-checkable: {raw}"
            ))),
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

    fn check_two_exprs(&mut self, first: &Expr, second: &Expr) {
        self.check_expr(first);
        self.check_expr(second);
    }

    fn check_on_stmt(&mut self, stmt: &Stmt, body: &[Stmt]) {
        let outer_locals = self.locals.clone();
        self.bind_on_head_locals(stmt);
        for stmt in body {
            self.check_stmt(stmt);
        }
        self.locals = outer_locals;
    }

    fn check_unsafe_lifetime_stmt(
        &mut self,
        reason: Option<&Expr>,
        has_safety_doc: bool,
        body: &[Stmt],
    ) {
        if let Some(reason) = reason {
            self.check_expr(reason);
        } else {
            self.errors.push(TypeCheckError::new(
                "unsafe lifetime block requires a reason".to_owned(),
            ));
        }
        if !has_safety_doc {
            self.errors.push(TypeCheckError::new(
                "unsafe lifetime block requires a SAFETY doc comment".to_owned(),
            ));
        }
        for stmt in body {
            self.check_stmt(stmt);
        }
    }

    fn check_command_stmt(&mut self, command: &arcweft_lang_syntax::ScenarioCommand) {
        for arg in command.args() {
            self.check_expr(arg);
        }
    }

    fn check_stmt_while(&mut self, condition: &Expr, body: &[Stmt]) {
        self.expect_expr_type(condition, &TypeKind::Bool, "while condition");
        self.with_statement_loop(|this| {
            for stmt in body {
                this.check_stmt(stmt);
            }
        });
    }

    fn check_let_stmt(&mut self, pattern: &Pattern, annotation: Option<&TypeRef>, expr: &Expr) {
        let ty = self
            .check_expr(expr)
            .or_else(|| annotation.map(type_ref_kind));
        if let (Some(annotation), Some(actual)) = (annotation, ty.as_ref()) {
            let expected = type_ref_kind(annotation);
            if &expected != actual {
                self.errors.push(TypeCheckError::new(format!(
                    "let annotation expects {expected:?}, but expression has {actual:?}"
                )));
            }
        }
        if let Some(ty) = ty {
            if let Some(name) = ident_pattern_name(pattern) {
                if let Some(slot_family) = default_presentation_slot_family(expr) {
                    if let Some(previous) = self
                        .active_presentation_defaults
                        .insert(slot_family.to_owned(), name.to_owned())
                    {
                        self.errors.push(TypeCheckError::new(format!(
                            "presentation `{slot_family}` default slot already has live handle `{previous}`; use an explicit `slot = @slot.{slot_family}.name` for simultaneous values"
                        )));
                    }
                }
            }
            for (name, binding_ty) in pattern_bindings_with_fallback(pattern, &ty) {
                self.locals.insert(name, binding_ty);
            }
        }
        collect_borrow_lifetimes(pattern, &mut self.active_borrows);
        if let Some(annotation) = annotation {
            collect_type_lifetimes(annotation, &mut self.active_borrows);
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
                self.locals.insert(name, ty);
            }
            if let Pattern::Ident(name) = pattern
                && is_local_ident(name)
            {
                self.locals.entry(name.to_owned()).or_insert(TypeKind::Unit);
            }
        }
    }

    fn check_let_else_stmt(
        &mut self,
        pattern: &Pattern,
        annotation: Option<&TypeRef>,
        expr: &Expr,
        else_body: &[Stmt],
    ) {
        let expr_type = self
            .check_expr(expr)
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
            self.locals.insert(name, ty);
        }
        collect_borrow_lifetimes(pattern, &mut self.active_borrows);
        if let Some(annotation) = annotation {
            collect_type_lifetimes(annotation, &mut self.active_borrows);
        }
    }

    fn check_if_stmt(&mut self, condition: &Expr, body: &[Stmt]) {
        self.expect_expr_type(condition, &TypeKind::Bool, "if condition");
        let outer_locals = self.locals.clone();
        for stmt in body {
            self.check_stmt(stmt);
        }
        self.locals = outer_locals;
    }

    fn check_stmt_loop(&mut self, body: &[Stmt]) {
        self.loop_stack.push(LoopContext {
            label: None,
            allows_value_break: true,
            break_types: Vec::new(),
        });
        for stmt in body {
            self.check_stmt(stmt);
        }
        self.loop_stack.pop();
    }

    fn check_stmt_while_let(
        &mut self,
        pattern: &Pattern,
        expr: &Expr,
        guard: Option<&Expr>,
        body: &[Stmt],
    ) {
        let expr_type = self.check_expr(expr);
        let outer_locals = self.locals.clone();
        for (name, ty) in let_else_bindings(pattern, expr_type.as_ref()) {
            self.locals.insert(name, ty);
        }
        if let Some(guard) = guard {
            self.expect_expr_type(guard, &TypeKind::Bool, "while-let guard");
        }
        self.with_statement_loop(|this| {
            for stmt in body {
                this.check_stmt(stmt);
            }
        });
        self.locals = outer_locals;
    }

    fn check_stmt_for(&mut self, pattern: &Pattern, source: &Expr, body: &[Stmt]) {
        self.check_expr(source);
        let outer_locals = self.locals.clone();
        if let Some(name) = ident_pattern_name(pattern) {
            self.locals
                .insert(name.to_owned(), TypeKind::Named("IteratorItem".to_owned()));
        }
        self.with_statement_loop(|this| {
            for stmt in body {
                this.check_stmt(stmt);
            }
        });
        self.locals = outer_locals;
    }

    fn check_match_stmt(&mut self, expr: &Expr, arms: &[arcweft_lang_syntax::StmtMatchArm]) {
        let expr_type = self.check_expr(expr);
        for arm in arms {
            let outer_locals = self.locals.clone();
            for (name, ty) in let_else_bindings(arm.pattern(), expr_type.as_ref()) {
                self.locals.insert(name, ty);
            }
            if let Some(guard) = arm.guard() {
                self.expect_expr_type(guard, &TypeKind::Bool, "match guard");
            }
            for stmt in arm.body() {
                self.check_stmt(stmt);
            }
            self.locals = outer_locals;
        }
    }

    fn check_break_stmt(&mut self, label: Option<&str>, expr: Option<&Expr>) {
        let Some(index) = self.resolve_loop_label(label) else {
            self.errors.push(TypeCheckError::new(label.map_or_else(
                || "break is only allowed inside loop, while, or for".to_owned(),
                |label| format!("break label `'{label}` does not name an active loop"),
            )));
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

    fn check_out_stmt(&mut self, label: Option<&str>, expr: &Expr) -> Option<TypeKind> {
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
        self.check_expr(expr)
    }

    fn check_ensure_stmt(&mut self, condition: &Expr, message: &Expr) {
        self.expect_expr_type(condition, &TypeKind::Bool, "ensure condition");
        self.check_expr(message);
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

    fn check_choice(&mut self, choice: &arcweft_lang_hir::HirChoice) {
        if let Some(id) = choice.id() {
            self.expect_entity_kind(id, &EntityKind::Choice, "choice id");
        }
        self.check_choice_items(choice.items());
        for option in choice.options() {
            if let Some(id) = option.id() {
                self.expect_entity_kind(id, &EntityKind::ChoiceOption, "choice option id");
            }
            if let Some(target) = option.target() {
                self.expect_entity_kind(target, &EntityKind::Flow, "choice target");
            }
        }
        if let Some(plan) = choice.plan() {
            for item in plan.items() {
                self.check_choice_plan_item(item);
            }
        }
    }

    fn check_choice_items(&mut self, items: &[arcweft_lang_syntax::ChoiceItem]) {
        for item in items {
            self.check_choice_item(item);
        }
    }

    fn check_choice_item(&mut self, item: &arcweft_lang_syntax::ChoiceItem) {
        match item {
            arcweft_lang_syntax::ChoiceItem::Let { pattern, expr } => {
                let value_type = self.check_expr(expr);
                if let (Some(name), Some(ty)) = (ident_pattern_name(pattern), value_type) {
                    self.locals.insert(name.to_owned(), ty);
                }
                if let Pattern::Raw(raw) = pattern {
                    self.errors.push(TypeCheckError::new(format!(
                        "raw choice let pattern is not type-checkable: {raw}"
                    )));
                }
            }
            arcweft_lang_syntax::ChoiceItem::If { condition, items } => {
                self.expect_expr_type(condition, &TypeKind::Bool, "choice if condition");
                let outer_locals = self.locals.clone();
                self.check_choice_items(items);
                self.locals = outer_locals;
            }
            arcweft_lang_syntax::ChoiceItem::For {
                pattern,
                source,
                items,
            } => {
                let source_type = self.check_expr(source);
                let outer_locals = self.locals.clone();
                if let Some(name) = ident_pattern_name(pattern) {
                    self.locals
                        .insert(name.to_owned(), iter_item_type(source_type.as_ref()));
                } else if let Pattern::Raw(raw) = pattern {
                    self.errors.push(TypeCheckError::new(format!(
                        "raw choice for pattern is not type-checkable: {raw}"
                    )));
                }
                self.check_choice_items(items);
                self.locals = outer_locals;
            }
            arcweft_lang_syntax::ChoiceItem::Match { expr, arms } => {
                self.check_expr(expr);
                for arm in arms {
                    let outer_locals = self.locals.clone();
                    if let Pattern::Raw(raw) = arm.pattern() {
                        self.errors.push(TypeCheckError::new(format!(
                            "raw choice match pattern is not type-checkable: {raw}"
                        )));
                    }
                    if let Some(guard) = arm.guard() {
                        self.expect_expr_type(guard, &TypeKind::Bool, "choice match guard");
                    }
                    self.check_choice_items(arm.items());
                    self.locals = outer_locals;
                }
            }
            arcweft_lang_syntax::ChoiceItem::Option(option) => self.check_choice_option(option),
            arcweft_lang_syntax::ChoiceItem::Raw(raw) => self.errors.push(TypeCheckError::new(
                format!("raw choice item is not type-checkable: {raw}"),
            )),
        }
    }

    fn check_choice_option(&mut self, option: &arcweft_lang_syntax::ChoiceOption) {
        if let Some(IdRef::Absolute(id)) = option.id() {
            self.expect_entity_kind(id, &EntityKind::ChoiceOption, "choice option id");
        }
        if let Some(id_expr) = option.id_expr() {
            self.check_expr(id_expr);
        }
        if let Some(IdRef::Absolute(text_key)) = option.label_text_key() {
            self.expect_entity_kind(text_key, &EntityKind::Text, "choice label text key");
        }
        if let Some(value) = option.value() {
            self.check_expr(value);
        }
        if let Some(enabled) = option.enabled() {
            self.expect_expr_type(enabled, &TypeKind::Bool, "choice enabled");
        }
        if let Some(visible) = option.visible() {
            self.expect_expr_type(visible, &TypeKind::Bool, "choice visible");
        }
        if let Some(order) = option.order() {
            self.expect_expr_type(order, &TypeKind::Int, "choice order");
        }
        if let Some(hotkey) = option.hotkey() {
            self.check_expr(hotkey);
        }
        for field in option.ui_fields() {
            self.check_expr(field.value());
        }
        self.check_choice_action(option.action());
    }

    fn check_choice_action(&mut self, action: &arcweft_lang_syntax::ChoiceAction) {
        match action {
            arcweft_lang_syntax::ChoiceAction::Out(expr) => {
                self.check_expr(expr);
            }
            arcweft_lang_syntax::ChoiceAction::SelectBlock(statements) => {
                let outer_locals = self.locals.clone();
                for stmt in statements {
                    self.check_stmt(stmt);
                }
                self.locals = outer_locals;
            }
            arcweft_lang_syntax::ChoiceAction::Goto(target) => {
                if let EntityRefSyntax::Absolute(target) = target {
                    self.expect_entity_kind(target, &EntityKind::Flow, "choice target");
                }
            }
            arcweft_lang_syntax::ChoiceAction::None => {}
        }
    }

    fn check_choice_plan_item(&mut self, item: &arcweft_lang_syntax::ChoicePlanItem) {
        match item {
            arcweft_lang_syntax::ChoicePlanItem::Option { value, .. } => {
                self.check_expr(value);
            }
            arcweft_lang_syntax::ChoicePlanItem::Timeout { duration, body } => {
                self.expect_expr_type(duration, &TypeKind::Duration, "choice timeout duration");
                for stmt in body {
                    self.check_stmt(stmt);
                }
            }
            arcweft_lang_syntax::ChoicePlanItem::Cancel { body, .. } => {
                for stmt in body {
                    self.check_stmt(stmt);
                }
            }
            arcweft_lang_syntax::ChoicePlanItem::OnSelect { pattern, body } => {
                let outer_locals = self.locals.clone();
                if let Some(name) = ident_pattern_name(pattern) {
                    self.locals
                        .insert(name.to_owned(), TypeKind::Ref(EntityKind::ChoiceOption));
                }
                for stmt in body {
                    self.check_stmt(stmt);
                }
                self.locals = outer_locals;
            }
            arcweft_lang_syntax::ChoicePlanItem::Raw(raw) => self.errors.push(TypeCheckError::new(
                format!("raw choice-plan item is not type-checkable: {raw}"),
            )),
        }
    }

    fn check_scope_expr_binding(
        &mut self,
        pattern: &Pattern,
        scope: &arcweft_lang_hir::HirScopeExpr,
    ) {
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

    fn check_select_block(&mut self, block: &arcweft_lang_hir::HirSelect) {
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

    fn check_borrow_block(&mut self, block: &arcweft_lang_hir::HirBorrow) {
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

    fn check_select_head(&mut self, head: &arcweft_lang_syntax::SelectBranchHead) {
        match head {
            arcweft_lang_syntax::SelectBranchHead::Bind { source, .. } => {
                self.check_expr(source);
            }
            arcweft_lang_syntax::SelectBranchHead::Frame(pattern)
            | arcweft_lang_syntax::SelectBranchHead::Event(pattern) => {
                if let Pattern::Raw(raw) = pattern {
                    self.errors.push(TypeCheckError::new(format!(
                        "raw select branch pattern is not type-checkable: {raw}"
                    )));
                }
            }
            arcweft_lang_syntax::SelectBranchHead::Raw(raw) => {
                self.errors.push(TypeCheckError::new(format!(
                    "raw select branch head is not type-checkable: {raw}"
                )));
            }
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
                    self.check_contract_selector(item);
                }
            }
        }
    }

    fn check_contract_selector(&mut self, expr: &Expr) {
        // Contract selectors name capabilities or resources. They are not
        // executable expressions, so dotted selectors such as `signal.write`
        // must not resolve `signal` as a local value.
        if expr_path_label(expr).is_some() || matches!(expr, Expr::EntityRef(_)) {
            return;
        }
        self.check_expr(expr);
    }

    fn reject_active_borrows(&mut self, boundary: &str) {
        if !self.active_borrows.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "borrowed values with lifetimes {:?} cannot cross {boundary}",
                self.active_borrows
            )));
        }
    }

    fn snapshot_runtime_scope(&self) -> TypeCheckerScopeSnapshot {
        TypeCheckerScopeSnapshot {
            active_borrows: self.active_borrows.clone(),
            locals: self.locals.clone(),
            active_presentation_defaults: self.active_presentation_defaults.clone(),
            lifetime_guarantees: self.lifetime_guarantees.clone(),
            dropped_lifetime_keys: self.dropped_lifetime_keys.clone(),
            available_lifetimes: self.available_lifetimes.clone(),
        }
    }

    fn restore_runtime_scope(&mut self, snapshot: TypeCheckerScopeSnapshot) {
        self.active_borrows = snapshot.active_borrows;
        self.locals = snapshot.locals;
        self.active_presentation_defaults = snapshot.active_presentation_defaults;
        self.lifetime_guarantees = snapshot.lifetime_guarantees;
        self.dropped_lifetime_keys = snapshot.dropped_lifetime_keys;
        self.available_lifetimes = snapshot.available_lifetimes;
    }

    fn with_line_runtime_scope<R>(&mut self, check: impl FnOnce(&mut Self) -> R) -> R {
        let snapshot = self.snapshot_runtime_scope();
        self.available_lifetimes.push(LifetimeScopeKind::Line);
        let output = check(self);
        self.restore_runtime_scope(snapshot);
        output
    }

    fn with_child_task_scope<R>(
        &mut self,
        restrict_line_and_cue_lifetimes: bool,
        check: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let snapshot = self.snapshot_runtime_scope();
        if restrict_line_and_cue_lifetimes {
            self.available_lifetimes
                .retain(|scope| !matches!(scope, LifetimeScopeKind::Line | LifetimeScopeKind::Cue));
        }
        let output = check(self);
        self.restore_runtime_scope(snapshot);
        output
    }

    fn check_line_plan_output_type(
        &mut self,
        plan: &arcweft_lang_syntax::LinePlan,
    ) -> Option<TypeKind> {
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

    fn check_line_plan_item(&mut self, item: &LinePlanItem) -> Option<TypeKind> {
        match item {
            LinePlanItem::Init(statements)
            | LinePlanItem::Stmt(Stmt::DeferBlock { statements, .. }) => self
                .with_child_task_scope(false, |checker| {
                    checker.check_line_plan_statements(statements)
                }),
            LinePlanItem::Thread(thread) => self.check_thread_body(thread.body()),
            LinePlanItem::Stmt(stmt) => {
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
            LinePlanItem::CancelRule(rule) => {
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
                        if let Some(stmt_output) = stmt_output {
                            output = Some(match output {
                                Some(current) => {
                                    merge_line_output(current, &stmt_output, &mut checker.errors)
                                }
                                None => stmt_output,
                            });
                        }
                    }
                    output
                });
                self.line_cancel_depth -= 1;
                output
            }
            LinePlanItem::Assert { expr, .. } => {
                self.expect_expr_type(expr, &TypeKind::Bool, "line-plan assertion");
                None
            }
            LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => self
                .with_child_task_scope(false, |checker| {
                    let mut output = None;
                    for item in items {
                        if let Some(item_output) = checker.check_line_plan_item(item) {
                            output = Some(match output {
                                Some(current) => {
                                    merge_line_output(current, &item_output, &mut checker.errors)
                                }
                                None => item_output,
                            });
                        }
                    }
                    output
                }),
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

    fn check_line_plan_statements(&mut self, statements: &[Stmt]) -> Option<TypeKind> {
        for stmt in statements {
            self.check_stmt(stmt);
        }
        None
    }

    fn check_thread_body(&mut self, statements: &[Stmt]) -> Option<TypeKind> {
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
    fn check_dialogue_content(&mut self, tokens: &[DialogueToken]) -> HashSet<String> {
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

    fn check_lifetime_set_stmt(&mut self, target: &Expr, expr: &Expr) {
        self.check_expr(expr);
        let Some(key) = lifetime_key(target) else {
            self.errors.push(TypeCheckError::new(
                "lifetime registry assignment target must be `'scope.key`".to_owned(),
            ));
            self.check_expr(target);
            return;
        };
        self.check_lifetime_access(&key, LifetimeAccessMode::Write);
        self.lifetime_guarantees.insert(key);
    }

    fn check_wait_stmt(&mut self, target: &arcweft_lang_syntax::WaitTarget) {
        match target {
            arcweft_lang_syntax::WaitTarget::Duration(expr) => {
                self.expect_expr_type(expr, &TypeKind::Duration, "wait duration");
            }
            arcweft_lang_syntax::WaitTarget::Mark(name) => {
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
            arcweft_lang_syntax::WaitTarget::Expr(expr) => {
                self.check_expr(expr);
            }
        }
    }

    fn check_lifetime_path_expr(&mut self, key: &LifetimeKey, optional: bool) -> Option<TypeKind> {
        self.check_lifetime_access(key, LifetimeAccessMode::Read);
        if self.dropped_lifetime_keys.contains(key) {
            self.errors.push(TypeCheckError::new(format!(
                "lifetime registry key `{}` was already dropped",
                key.as_dotted()
            )));
            return None;
        }
        let value = lifetime_value_type(key);
        if optional || self.lifetime_guarantees.contains(key) {
            return Some(if optional {
                TypeKind::Option(Box::new(value))
            } else {
                value
            });
        }
        self.errors.push(TypeCheckError::new(format!(
            "lifetime registry key `{}` is not statically guaranteed; use `{}?` or initialize it first",
            key.as_dotted(),
            key.as_dotted()
        )));
        Some(TypeKind::Option(Box::new(value)))
    }

    fn check_lifetime_pipe(&mut self, lhs: &Expr, rhs: &Expr) -> Option<()> {
        let key = lifetime_key(lhs)?;
        match rhs {
            Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop") => {
                self.drop_lifetime_key(&key);
                Some(())
            }
            Expr::Call { callee, .. } if matches!(callee.as_ref(), Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop")) =>
            {
                self.drop_lifetime_key(&key);
                Some(())
            }
            _ => None,
        }
    }

    fn drop_lifetime_key(&mut self, key: &LifetimeKey) {
        self.check_lifetime_access(key, LifetimeAccessMode::Drop);
        if !self.dropped_lifetime_keys.insert(key.clone()) {
            self.errors.push(TypeCheckError::new(format!(
                "lifetime registry key `{}` was dropped more than once",
                key.as_dotted()
            )));
        }
        self.lifetime_guarantees.remove(key);
    }

    fn check_lifetime_access(&mut self, key: &LifetimeKey, mode: LifetimeAccessMode) {
        if !self.lifetime_available(key.scope()) {
            self.errors.push(TypeCheckError::new(format!(
                "lifetime `{}` is not available in this scope",
                key.scope().as_str()
            )));
        }
        if matches!(mode, LifetimeAccessMode::Write)
            && !matches!(key.scope(), LifetimeScopeKind::Line)
            && !self
                .effect_capabilities
                .contains(&format!("state.write({})", key.scope().as_str()))
        {
            self.errors.push(TypeCheckError::new(format!(
                "writing `{}` requires effect capability `state.write({})`",
                key.as_dotted(),
                key.scope().as_str()
            )));
        }
    }

    fn lifetime_available(&self, scope: &LifetimeScopeKind) -> bool {
        !matches!(scope, LifetimeScopeKind::Line | LifetimeScopeKind::Cue)
            || self.available_lifetimes.contains(scope)
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
            Expr::EntityRef(entity) => self.check_entity_ref_expr(entity),
            Expr::LifetimePath { key, optional } => self.check_lifetime_path_expr(key, *optional),
            Expr::Path(path) => self.check_path_expr(path),
            Expr::Placeholder(_) => None,
            Expr::Tuple(items) => Some(self.check_tuple_expr(items)),
            Expr::List(items) => Some(self.check_list_expr(items)),
            Expr::Call { callee, args } => self.check_call_expr(callee, args),
            Expr::NamedArg { value, .. } => self.check_expr(value),
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => self.check_method_call_expr(receiver, method, args),
            Expr::Field { target, field } => self.check_field_expr(expr, target, field),
            Expr::DialogueCall { callee, plan, .. } => {
                Some(self.check_dialogue_call_expr(callee, plan.as_ref()))
            }
            Expr::Index { target, index } => self.check_index_expr(target, index),
            Expr::Pipe { lhs, rhs } => self.check_pipe_expr(lhs, rhs),
            Expr::Try { expr } => self.check_try_expr(expr),
            Expr::Await { expr, applies_try } => self.check_await_expr(expr, *applies_try),
            Expr::Thread { block } => {
                self.check_thread_body(block.body());
                Some(TypeKind::ThreadHandle(Box::new(TypeKind::Unit)))
            }
            Expr::Range { start, end, .. } => {
                Some(self.check_range_expr(start.as_deref(), end.as_deref()))
            }
            Expr::Record { path, fields } => Some(self.check_record_expr(path, fields)),
            Expr::RecordLiteral(fields) => Some(self.check_record_literal_expr(fields)),
            Expr::Binary { lhs, op, rhs } => self.check_binary_expr(lhs, *op, rhs),
            Expr::Closure { body, .. } => {
                self.check_expr(body);
                None
            }
            Expr::Unary { op, expr } => Some(self.check_unary_expr(*op, expr)),
            Expr::Block { statements, value } => {
                self.check_block_expr(statements, value.as_deref())
            }
            Expr::ComputationBlock {
                statements, value, ..
            }
            | Expr::NamedBlock {
                statements, value, ..
            } => self.check_block_expr(statements, value.as_deref()),
            Expr::MemoBlock {
                options,
                statements,
                value,
            } => self.check_memo_block_expr(options, statements, value.as_deref()),
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

    fn check_entity_ref_expr(&mut self, entity: &EntityRefSyntax) -> Option<TypeKind> {
        entity
            .as_absolute()
            .and_then(entity_kind)
            .map(TypeKind::Ref)
            .or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown entity reference kind: {}",
                    entity.body()
                )));
                None
            })
    }

    fn check_path_expr(&mut self, path: &str) -> Option<TypeKind> {
        self.locals.get(path).cloned().or_else(|| {
            self.env.symbol_type(path).cloned().or_else(|| {
                self.check_dotted_path_target(path).or_else(|| {
                    // Short enum-variant expressions such as `.Instant` rely
                    // on expected type resolution in the full checker. The
                    // Phase 1 checker preserves unknown short variants as
                    // variant values after registered symbols and patch names
                    // had a chance to resolve.
                    if path.starts_with('.') {
                        return Some(TypeKind::Named("Variant".to_owned()));
                    }
                    self.errors
                        .push(TypeCheckError::new(format!("unknown symbol `{path}`")));
                    None
                })
            })
        })
    }

    fn check_pipe_expr(&mut self, lhs: &Expr, rhs: &Expr) -> Option<TypeKind> {
        if self.check_lifetime_pipe(lhs, rhs).is_some() {
            return Some(TypeKind::Unit);
        }
        self.check_expr(lhs);
        self.check_expr(rhs)
    }

    fn check_record_expr(&mut self, path: &str, fields: &[(String, Expr)]) -> TypeKind {
        self.check_record_fields(fields);
        TypeKind::Named(path.to_owned())
    }

    fn check_record_literal_expr(&mut self, fields: &[(String, Expr)]) -> TypeKind {
        self.check_record_fields(fields);
        TypeKind::Named("Record".to_owned())
    }

    fn check_dialogue_call_expr(
        &mut self,
        callee: &Expr,
        plan: Option<&arcweft_lang_syntax::LinePlan>,
    ) -> TypeKind {
        self.check_expr(callee);
        if let Some(plan) = plan {
            self.available_lifetimes.push(LifetimeScopeKind::Line);
            let output = self.check_line_plan_output_type(plan);
            self.available_lifetimes.pop();
            output.unwrap_or(TypeKind::Unit)
        } else {
            TypeKind::Unit
        }
    }

    fn check_tuple_expr(&mut self, items: &[Expr]) -> TypeKind {
        if items.is_empty() {
            return TypeKind::Unit;
        }
        TypeKind::Tuple(
            items
                .iter()
                .filter_map(|item| self.check_expr(item))
                .collect(),
        )
    }

    fn check_list_expr(&mut self, items: &[Expr]) -> TypeKind {
        let mut item_type = None;
        for item in items {
            let next_type = self.check_expr(item).unwrap_or(TypeKind::Unit);
            match &item_type {
                Some(existing) if existing != &next_type => {
                    self.errors.push(TypeCheckError::new(format!(
                        "list items must have the same type, found {existing:?} and {next_type:?}"
                    )));
                }
                Some(_) => {}
                None => item_type = Some(next_type),
            }
        }
        TypeKind::List(Box::new(item_type.unwrap_or(TypeKind::Unit)))
    }

    fn check_memo_block_expr(
        &mut self,
        options: &[(String, Expr)],
        statements: &[Stmt],
        value: Option<&Expr>,
    ) -> Option<TypeKind> {
        for (_, option) in options {
            self.check_expr(option);
        }
        self.check_block_expr(statements, value)
    }

    fn check_record_fields(&mut self, fields: &[(String, Expr)]) {
        for (_, value) in fields {
            self.check_expr(value);
        }
    }

    fn check_range_expr(&mut self, start: Option<&Expr>, end: Option<&Expr>) -> TypeKind {
        if let Some(start) = start {
            self.check_expr(start);
        }
        if let Some(end) = end {
            self.check_expr(end);
        }
        TypeKind::Range
    }

    fn check_call_expr(&mut self, callee: &Expr, args: &[Expr]) -> Option<TypeKind> {
        if let Some(name) = expr_path_label(callee)
            && let Some(ty) = self
                .env
                .function_type(&name)
                .cloned()
                .or_else(|| well_known_runtime_method_type(&name))
        {
            for arg in args {
                self.check_expr(arg);
            }
            return Some(ty);
        }
        if let Expr::Path(name) = callee {
            if let Some(ty) = self.check_presentation_call(name, args) {
                return Some(ty);
            }
            if matches!(name.as_str(), "promote" | "promote_unchecked") {
                for arg in args
                    .iter()
                    .filter(|arg| matches!(arg, Expr::NamedArg { .. }))
                {
                    self.check_expr(arg);
                }
                return Some(TypeKind::Named("Promoted".to_owned()));
            }
            if name == "assume" {
                return Some(TypeKind::Unit);
            }
            let arg_types = args
                .iter()
                .map(|arg| self.check_expr(arg))
                .collect::<Vec<_>>();
            if name == "Ok" {
                return Some(TypeKind::Result {
                    ok: Box::new(first_arg_type(&arg_types)),
                    error: Box::new(TypeKind::Named("_".to_owned())),
                });
            }
            if name == "Err" {
                return Some(TypeKind::Result {
                    ok: Box::new(TypeKind::Named("_".to_owned())),
                    error: Box::new(first_arg_type(&arg_types)),
                });
            }
            if self.env.symbol_type(name) == Some(&TypeKind::Ref(EntityKind::Character)) {
                return Some(TypeKind::SpeakerPreset(EntityKind::Character));
            }
            return self.env.function_type(name).cloned().or_else(|| {
                self.errors
                    .push(TypeCheckError::new(format!("unknown function `{name}`")));
                None
            });
        }
        for arg in args {
            self.check_expr(arg);
        }
        match self.check_expr(callee) {
            Some(TypeKind::Speaker(entity) | TypeKind::SpeakerPreset(entity)) => {
                Some(TypeKind::SpeakerPreset(entity))
            }
            other => other,
        }
    }

    fn check_presentation_call(&mut self, name: &str, args: &[Expr]) -> Option<TypeKind> {
        match name {
            "bg" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Asset, "bg asset");
                self.check_presentation_named_args(args, "background");
                Some(TypeKind::Named(
                    "PresentationHandle<BackgroundSurface>".to_owned(),
                ))
            }
            "show" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Character, "show character");
                self.check_presentation_named_args(args, "character");
                Some(TypeKind::Named(
                    "PresentationHandle<CharacterSurface>".to_owned(),
                ))
            }
            "ref.bg" => {
                self.check_presentation_named_args(args, "background");
                Some(TypeKind::Named("SlotRef<BackgroundSurface>".to_owned()))
            }
            "ref.show" => {
                self.check_positional_entity_arg(
                    args,
                    0,
                    &EntityKind::Character,
                    "ref show character",
                );
                self.check_presentation_named_args(args, "character");
                Some(TypeKind::Named("SlotRef<CharacterSurface>".to_owned()))
            }
            "clear.bg" => {
                self.check_presentation_named_args(args, "background");
                self.active_presentation_defaults.remove("background");
                Some(TypeKind::Named("Option<BackgroundSurface>".to_owned()))
            }
            "hide" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Character, "hide character");
                self.check_presentation_named_args(args, "character");
                self.active_presentation_defaults.remove("character");
                Some(TypeKind::Named("Option<CharacterSurface>".to_owned()))
            }
            _ => None,
        }
    }

    fn check_positional_entity_arg(
        &mut self,
        args: &[Expr],
        index: usize,
        expected: &EntityKind,
        context: &str,
    ) {
        let Some(arg) = args
            .iter()
            .filter(|arg| !matches!(arg, Expr::NamedArg { .. }))
            .nth(index)
        else {
            self.errors.push(TypeCheckError::new(format!(
                "{context} argument is required"
            )));
            return;
        };
        match arg {
            Expr::EntityRef(entity) => match entity.as_absolute().and_then(entity_kind) {
                Some(kind) if &kind == expected => {}
                actual => self.errors.push(TypeCheckError::new(format!(
                    "{context} must be a {expected:?} reference, found {actual:?}"
                ))),
            },
            Expr::Path(path) if self.locals.get(path) == Some(&TypeKind::Ref(expected.clone())) => {
            }
            Expr::Path(path)
                if self.env.symbol_type(path) == Some(&TypeKind::Ref(expected.clone())) => {}
            other => {
                self.check_expr(other);
                self.errors.push(TypeCheckError::new(format!(
                    "{context} must be a {expected:?} reference"
                )));
            }
        }
    }

    fn check_presentation_named_args(&mut self, args: &[Expr], slot_family: &str) {
        for arg in args {
            let Expr::NamedArg { name, value } = arg else {
                continue;
            };
            match name.as_str() {
                "target" => self.expect_entity_expr_kind(value, &EntityKind::Target, "target"),
                "slot" => self.expect_slot_family(value, slot_family),
                "scope" => self.expect_entity_expr_kind(
                    value,
                    &EntityKind::Other("scope".to_owned()),
                    "scope",
                ),
                _ => {
                    self.check_expr(value);
                }
            }
        }
    }

    fn expect_entity_expr_kind(&mut self, expr: &Expr, expected: &EntityKind, context: &str) {
        match expr {
            Expr::EntityRef(entity) => match entity.as_absolute().and_then(entity_kind) {
                Some(kind) if &kind == expected => {}
                actual => self.errors.push(TypeCheckError::new(format!(
                    "presentation {context} must be a {expected:?} reference, found {actual:?}"
                ))),
            },
            other => {
                self.check_expr(other);
                self.errors.push(TypeCheckError::new(format!(
                    "presentation {context} must be an entity reference"
                )));
            }
        }
    }

    fn expect_slot_family(&mut self, expr: &Expr, slot_family: &str) {
        match expr {
            Expr::EntityRef(entity) => {
                let Some(entity) = entity.as_absolute() else {
                    self.errors.push(TypeCheckError::new(
                        "presentation slot must be an absolute slot reference".to_owned(),
                    ));
                    return;
                };
                if entity_kind(entity) != Some(EntityKind::Slot)
                    || !entity.body().starts_with(&format!("slot.{slot_family}."))
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "presentation slot `{}` must be in `@slot.{slot_family}.*`",
                        entity.body()
                    )));
                }
            }
            other => {
                self.check_expr(other);
                self.errors.push(TypeCheckError::new(
                    "presentation slot must be an entity reference".to_owned(),
                ));
            }
        }
    }

    fn check_unary_expr(&mut self, op: arcweft_lang_syntax::UnaryOp, expr: &Expr) -> TypeKind {
        match op {
            arcweft_lang_syntax::UnaryOp::Not => {
                self.expect_expr_type(expr, &TypeKind::Bool, "not operand");
                TypeKind::Bool
            }
            arcweft_lang_syntax::UnaryOp::Neg => match self.check_expr(expr) {
                Some(TypeKind::Int) => TypeKind::Int,
                Some(TypeKind::Float) => TypeKind::Float,
                Some(TypeKind::Duration) => TypeKind::Duration,
                other => {
                    self.errors.push(TypeCheckError::new(format!(
                        "negation operand must be numeric or Duration, found {other:?}"
                    )));
                    TypeKind::Named("_".to_owned())
                }
            },
        }
    }

    fn check_method_call_expr(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
    ) -> Option<TypeKind> {
        if let Expr::Path(receiver_path) = receiver {
            let dotted = format!("{receiver_path}.{method}");
            if let Some(ty) = self
                .env
                .function_type(&dotted)
                .cloned()
                .or_else(|| well_known_runtime_method_type(&dotted))
            {
                for arg in args {
                    self.check_expr(arg);
                }
                return Some(ty);
            }
        }
        let receiver_type = self.check_expr(receiver);
        for arg in args {
            self.check_expr(arg);
        }
        receiver_type.and_then(|receiver_type| {
            self.env
                .method_type(&receiver_type, method)
                .cloned()
                .or_else(|| well_known_capacity_method_type(&receiver_type, method, args.len()))
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
            collection_index_type(&target_type)
                .or_else(|| self.env.index_type(&target_type).cloned())
                .or_else(|| {
                    self.errors.push(TypeCheckError::new(format!(
                        "type {target_type:?} is not indexable"
                    )));
                    None
                })
        })
    }

    fn check_dotted_path_target(&mut self, path: &str) -> Option<TypeKind> {
        let (target, field) = path.rsplit_once('.')?;
        if let Some(field_type) = well_known_field_type(field) {
            return Some(field_type);
        }
        self.locals
            .get(target)
            .cloned()
            .or_else(|| self.env.symbol_type(target).cloned())
    }

    fn check_field_expr(&mut self, expr: &Expr, target: &Expr, field: &str) -> Option<TypeKind> {
        if let Some(path) = expr_path_label(expr) {
            if let Some(ty) = self.locals.get(&path).cloned() {
                return Some(ty);
            }
            if let Some(ty) = self.env.symbol_type(&path).cloned() {
                return Some(ty);
            }
        }
        if let Some(field_type) = well_known_field_type(field) {
            self.check_expr(target);
            return Some(field_type);
        }
        self.check_expr(target);
        None
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

    fn check_await_expr(&mut self, expr: &Expr, applies_try: bool) -> Option<TypeKind> {
        self.reject_active_borrows("await suspension boundary");
        match self.check_expr(expr) {
            Some(TypeKind::Need { ready, .. }) if applies_try => Some(*ready),
            Some(TypeKind::Need { ready, error }) => Some(TypeKind::Result { ok: ready, error }),
            Some(other) => {
                self.errors.push(TypeCheckError::new(format!(
                    "await expression must have Need<T, E> type, found {other:?}"
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
        arms: &[arcweft_lang_syntax::MatchExprArm],
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
            BinaryOp::Merge => match (lhs_type, rhs_type) {
                (Some(TypeKind::CharacterPatch(lhs)), Some(TypeKind::CharacterPatch(rhs)))
                    if lhs == rhs =>
                {
                    Some(TypeKind::CharacterPatch(lhs))
                }
                (Some(TypeKind::FocusPatch), Some(TypeKind::FocusPatch)) => {
                    Some(TypeKind::FocusPatch)
                }
                (lhs, rhs) => {
                    self.errors.push(TypeCheckError::new(format!(
                        "merge operator `&` requires compatible patch operands, found {lhs:?} and {rhs:?}"
                    )));
                    None
                }
            },
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
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
        "ui" => EntityKind::Component,
        "activity" => EntityKind::Activity,
        "textbox" => EntityKind::Textbox,
        "say" => EntityKind::DialogueLine,
        "text" => EntityKind::Text,
        "item" => EntityKind::Other("item".to_owned()),
        "asset" => EntityKind::Asset,
        "anim" => EntityKind::Animation,
        "capture" => EntityKind::Capture,
        "hook" => EntityKind::Hook,
        "signal" => EntityKind::Signal,
        "scene" => EntityKind::Scene,
        "source" => EntityKind::Source,
        "test" => EntityKind::Test,
        "bench" => EntityKind::Bench,
        "layer" => EntityKind::Layer,
        "voice" => EntityKind::Voice,
        "se" => EntityKind::Se,
        "bgm" => EntityKind::Bgm,
        "bus" => EntityKind::AudioBus,
        "mix" => EntityKind::MixerSnapshot,
        "duck" => EntityKind::Ducking,
        "motion" => EntityKind::Motion,
        "rig" => EntityKind::Rig,
        "slot" => EntityKind::Slot,
        "target" => EntityKind::Target,
        "scope" => EntityKind::Other("scope".to_owned()),
        "ent" => EntityKind::Other("ent".to_owned()),
        _ => return None,
    })
}

fn expr_path_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => Some(path.clone()),
        Expr::Field { target, field } => Some(format!("{}.{}", expr_path_label(target)?, field)),
        _ => None,
    }
}

fn lifetime_key(expr: &Expr) -> Option<LifetimeKey> {
    match expr {
        Expr::LifetimePath { key, .. } => Some(key.clone()),
        _ => None,
    }
}

fn lifetime_value_type(key: &LifetimeKey) -> TypeKind {
    if key.scope() == &LifetimeScopeKind::Line
        && key.path().first().is_some_and(|part| part == "focus")
    {
        TypeKind::Handle {
            name: "FocusHandle".to_owned(),
            lifetime: key.scope().clone(),
            state: HandleState::Live,
            must_drop: true,
        }
    } else {
        TypeKind::Named("LifetimeValue".to_owned())
    }
}

fn entity_kind_for_decl(kind: EntityDeclKind) -> EntityKind {
    match kind {
        EntityDeclKind::Character => EntityKind::Character,
        EntityDeclKind::Component => EntityKind::Component,
        EntityDeclKind::Activity => EntityKind::Activity,
        EntityDeclKind::Signal => EntityKind::Signal,
        EntityDeclKind::Layer => EntityKind::Layer,
        EntityDeclKind::Textbox => EntityKind::Textbox,
        EntityDeclKind::Voice => EntityKind::Voice,
        EntityDeclKind::Se => EntityKind::Se,
        EntityDeclKind::Bgm => EntityKind::Bgm,
        EntityDeclKind::AudioBus => EntityKind::AudioBus,
        EntityDeclKind::MixerSnapshot => EntityKind::MixerSnapshot,
        EntityDeclKind::Ducking => EntityKind::Ducking,
        EntityDeclKind::Motion => EntityKind::Motion,
        EntityDeclKind::Rig => EntityKind::Rig,
    }
}

fn literal_type(literal: &Literal) -> TypeKind {
    match literal {
        Literal::String(_) => TypeKind::String,
        Literal::Char { .. } => TypeKind::Char,
        Literal::Int(_) => TypeKind::Int,
        Literal::Float(_) => TypeKind::Float,
        Literal::Bool(_) => TypeKind::Bool,
        Literal::Duration { .. } => TypeKind::Duration,
    }
}

fn is_dialogue_callee_type(ty: Option<&TypeKind>) -> bool {
    matches!(ty, Some(TypeKind::Ref(EntityKind::Character)))
        || matches!(ty, Some(TypeKind::Speaker(_)))
        || matches!(ty, Some(TypeKind::SpeakerPreset(_)))
        || matches!(ty, Some(TypeKind::Named(name)) if name == "SpeakerPreset")
}

fn is_character_entity_literal(source: &str) -> bool {
    let trimmed = source.trim();
    trimmed
        .strip_prefix("@<")
        .and_then(|inner| inner.strip_suffix('>'))
        .map_or_else(
            || trimmed.strip_prefix("@character.").is_some(),
            |inner| inner.starts_with("character."),
        )
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
        | Pattern::Variant { .. }
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

fn iter_item_type(source_type: Option<&TypeKind>) -> TypeKind {
    match source_type {
        Some(TypeKind::List(item) | TypeKind::Seq(item) | TypeKind::Slice(item)) => {
            item.as_ref().clone()
        }
        Some(TypeKind::Named(name)) if name.starts_with("List<") && name.ends_with('>') => {
            TypeKind::Named(name[5..name.len() - 1].to_owned())
        }
        Some(TypeKind::Named(name)) if name.starts_with("Seq<") && name.ends_with('>') => {
            TypeKind::Named(name[4..name.len() - 1].to_owned())
        }
        Some(TypeKind::Named(name)) if name.starts_with("Vec<") && name.ends_with('>') => {
            TypeKind::Named(name[4..name.len() - 1].to_owned())
        }
        _ => TypeKind::Named("ChoiceOptionSource".to_owned()),
    }
}

fn well_known_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "choice_id" | "id" => TypeKind::Ref(EntityKind::ChoiceOption),
        "target" => TypeKind::Ref(EntityKind::Flow),
        "enabled" | "visible" | "ready" => TypeKind::Bool,
        "order" | "count" | "index" => TypeKind::Int,
        "ratio" => TypeKind::Float,
        "label" | "disabled_reason" | "badge" | "hotkey" | "text" => TypeKind::String,
        _ => return None,
    })
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
        Pattern::Variant { payload, .. } => payload
            .iter()
            .flat_map(variant_payload_bindings)
            .filter_map(|name| variant_payload_type(expr_type).map(|ty| (name, ty)))
            .collect(),
        Pattern::Tuple(items) => items
            .iter()
            .flat_map(|item| let_else_bindings(item, None))
            .collect(),
        Pattern::List { items, rest } => {
            let mut bindings = items
                .iter()
                .flat_map(|item| let_else_bindings(item, None))
                .collect::<Vec<_>>();
            if let Some(rest) = rest.as_ref().filter(|name| is_local_ident(name)) {
                bindings.push((rest.to_owned(), TypeKind::Unit));
            }
            bindings
        }
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

fn pattern_bindings_with_fallback(
    pattern: &Pattern,
    fallback: &TypeKind,
) -> Vec<(String, TypeKind)> {
    let mut bindings = let_else_bindings(pattern, Some(fallback));
    for name in collect_pattern_binding_names(pattern) {
        if !bindings.iter().any(|(bound, _)| bound == &name) {
            bindings.push((name, TypeKind::Unit));
        }
    }
    bindings
}

fn collect_pattern_binding_names(pattern: &Pattern) -> Vec<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) if is_local_ident(name) => {
            vec![name.to_owned()]
        }
        Pattern::Tuple(items) => items
            .iter()
            .flat_map(collect_pattern_binding_names)
            .collect(),
        Pattern::List { items, rest } => {
            let mut names = items
                .iter()
                .flat_map(collect_pattern_binding_names)
                .collect::<Vec<_>>();
            if let Some(rest) = rest.as_ref().filter(|name| is_local_ident(name)) {
                names.push(rest.to_owned());
            }
            names
        }
        Pattern::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| collect_pattern_binding_names(field.pattern()))
            .collect(),
        Pattern::Variant { payload, .. } => payload
            .iter()
            .flat_map(|payload| match payload {
                arcweft_lang_syntax::VariantPatternPayload::Tuple(items) => items
                    .iter()
                    .flat_map(collect_pattern_binding_names)
                    .collect::<Vec<_>>(),
                arcweft_lang_syntax::VariantPatternPayload::Record { fields, .. } => fields
                    .iter()
                    .flat_map(|field| collect_pattern_binding_names(field.pattern()))
                    .collect(),
            })
            .collect(),
        Pattern::Whole { name, pattern } => {
            let mut names = is_local_ident(name)
                .then(|| name.to_owned())
                .into_iter()
                .collect::<Vec<_>>();
            names.extend(collect_pattern_binding_names(pattern));
            names
        }
        Pattern::Typed { name, .. } if is_local_ident(name) => vec![name.to_owned()],
        Pattern::Literal(_)
        | Pattern::Entity(_)
        | Pattern::Discard
        | Pattern::Raw(_)
        | Pattern::Typed { .. }
        | Pattern::Ident(_)
        | Pattern::MutIdent(_) => Vec::new(),
    }
}

fn variant_payload_bindings(payload: &arcweft_lang_syntax::VariantPatternPayload) -> Vec<String> {
    match payload {
        arcweft_lang_syntax::VariantPatternPayload::Tuple(items) => items
            .iter()
            .filter_map(|pattern| match pattern {
                Pattern::Ident(name) if is_local_ident(name) => Some(name.to_owned()),
                _ => None,
            })
            .collect(),
        arcweft_lang_syntax::VariantPatternPayload::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| {
                let names = let_else_bindings(field.pattern(), None);
                if names.is_empty() {
                    vec![(field.name().to_owned(), TypeKind::Unit)]
                } else {
                    names
                }
            })
            .map(|(name, _)| name)
            .collect(),
    }
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

fn variant_payload_type(expr_type: Option<&TypeKind>) -> Option<TypeKind> {
    option_payload_type(expr_type).or_else(|| expr_type.cloned())
}

fn result_ok_type(name: &str) -> Option<TypeKind> {
    let inner = name
        .strip_prefix("Result<")
        .and_then(|value| value.strip_suffix('>'))?;
    let ok = inner.split_once(',').map_or(inner, |(ok, _)| ok).trim();
    Some(named_type_label(ok))
}

fn well_known_runtime_method_type(name: &str) -> Option<TypeKind> {
    if let Some(ty) = well_known_static_capacity_method_type(name) {
        return Some(ty);
    }
    (name.starts_with("log.")
        || matches!(
            name,
            "signal.set"
                | "metric.set"
                | "event.emit"
                | "scene.show"
                | "scene.clear"
                | "progress.set"
                | "meter.show"
                | "text.show"
                | "text.flush"
                | "voice.stop"
                | "cues.stop"
        ))
    .then_some(TypeKind::Unit)
}

fn well_known_static_capacity_method_type(name: &str) -> Option<TypeKind> {
    match name {
        "List.with_capacity" => Some(TypeKind::List(Box::new(TypeKind::Named("_".to_owned())))),
        "String.with_capacity" => Some(TypeKind::String),
        "Bytes.with_capacity" => Some(TypeKind::Named("Bytes".to_owned())),
        _ => None,
    }
}

fn well_known_capacity_method_type(
    receiver: &TypeKind,
    method: &str,
    arg_count: usize,
) -> Option<TypeKind> {
    if !is_reservable_type(receiver) {
        return None;
    }
    match (method, arg_count) {
        ("reserve" | "shrink_to", 1) | ("shrink", 0) => Some(TypeKind::Unit),
        _ => None,
    }
}

fn is_reservable_type(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::List(_) | TypeKind::String)
        || matches!(ty, TypeKind::Named(name) if name == "Bytes")
}

fn collection_index_type(target_type: &TypeKind) -> Option<TypeKind> {
    match target_type {
        TypeKind::List(item) | TypeKind::Slice(item) => Some(item.as_ref().clone()),
        TypeKind::Map { value, .. } => Some(value.as_ref().clone()),
        TypeKind::String => Some(TypeKind::TextCluster),
        _ => None,
    }
}

fn first_arg_type(types: &[Option<TypeKind>]) -> TypeKind {
    types
        .first()
        .and_then(Clone::clone)
        .unwrap_or(TypeKind::Unit)
}

fn merge_line_output(
    current: TypeKind,
    next: &TypeKind,
    errors: &mut Vec<TypeCheckError>,
) -> TypeKind {
    if &current == next {
        return current;
    }
    if let Some(merged) = merge_result_types(&current, next) {
        return merged;
    }
    errors.push(TypeCheckError::new(format!(
        "line-plan out expressions must have the same type, found {current:?} and {next:?}"
    )));
    current
}

fn merge_result_types(left: &TypeKind, right: &TypeKind) -> Option<TypeKind> {
    let (
        TypeKind::Result {
            ok: left_ok,
            error: left_error,
        },
        TypeKind::Result {
            ok: right_ok,
            error: right_error,
        },
    ) = (left, right)
    else {
        return None;
    };

    let ok = merge_placeholder_type(left_ok, right_ok)?;
    let error = merge_placeholder_type(left_error, right_error)?;
    Some(TypeKind::Result {
        ok: Box::new(ok),
        error: Box::new(error),
    })
}

fn merge_placeholder_type(left: &TypeKind, right: &TypeKind) -> Option<TypeKind> {
    if left == right {
        return Some(left.clone());
    }
    if is_placeholder_type(left) {
        return Some(right.clone());
    }
    if is_placeholder_type(right) {
        return Some(left.clone());
    }
    None
}

fn is_placeholder_type(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::Named(name) if name == "_")
}

fn named_type_label(name: &str) -> TypeKind {
    match name {
        "bool" | "Bool" => TypeKind::Bool,
        "i32" | "i64" | "usize" | "Int" => TypeKind::Int,
        "f32" | "f64" | "Float" => TypeKind::Float,
        "String" => TypeKind::String,
        "char" | "Char" => TypeKind::Char,
        "TextCluster" => TypeKind::TextCluster,
        "Duration" => TypeKind::Duration,
        "()" | "Unit" => TypeKind::Unit,
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
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Panic(_)
        | Stmt::Fail(_)
        | Stmt::Bail(_) => true,
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

fn choice_output_type(choice: &arcweft_lang_hir::HirChoice) -> Option<TypeKind> {
    let mut inferred = None;
    for option in choice.options() {
        let ty = match option.action() {
            arcweft_lang_syntax::ChoiceAction::Out(expr) => simple_expr_type(expr)?,
            arcweft_lang_syntax::ChoiceAction::SelectBlock(statements) => {
                let [Stmt::Out { expr, .. }] = statements.as_slice() else {
                    return None;
                };
                simple_expr_type(expr)?
            }
            arcweft_lang_syntax::ChoiceAction::Goto(_)
            | arcweft_lang_syntax::ChoiceAction::None => return None,
        };
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
        Expr::EntityRef(entity) => entity
            .as_absolute()
            .and_then(entity_kind)
            .map(TypeKind::Ref),
        Expr::Literal(literal) => Some(literal_type(literal)),
        Expr::Tuple(items) => items
            .iter()
            .map(simple_expr_type)
            .collect::<Option<Vec<_>>>()
            .map(TypeKind::Tuple),
        Expr::List(items) => {
            let item = items
                .first()
                .and_then(simple_expr_type)
                .unwrap_or(TypeKind::Unit);
            Some(TypeKind::List(Box::new(item)))
        }
        Expr::RecordLiteral(_) => Some(TypeKind::Named("Record".to_owned())),
        _ => None,
    }
}

fn default_presentation_slot_family(expr: &Expr) -> Option<&'static str> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    if args
        .iter()
        .any(|arg| matches!(arg, Expr::NamedArg { name, .. } if name == "slot"))
    {
        return None;
    }
    match callee.as_ref() {
        Expr::Path(name) if name == "bg" => Some("background"),
        Expr::Path(name) if name == "show" => Some("character"),
        _ => None,
    }
}

fn await_branch_pattern_type(
    kind: AwaitBranchKind,
    ready: &TypeKind,
    error: &TypeKind,
) -> TypeKind {
    match kind {
        AwaitBranchKind::Pending => TypeKind::Named("Progress".to_owned()),
        AwaitBranchKind::Ready => ready.clone(),
        AwaitBranchKind::Error => error.clone(),
        AwaitBranchKind::Denied => TypeKind::Named("AwaitDenied".to_owned()),
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
        TypeRef::Never | TypeRef::Path(_) => {}
    }
}

fn type_ref_kind(ty: &TypeRef) -> TypeKind {
    match ty {
        TypeRef::Never => TypeKind::Never,
        TypeRef::Path(path) => named_type_label(path),
        TypeRef::Generic { base, args } if base == "List" && args.len() == 1 => {
            TypeKind::List(Box::new(type_ref_kind(&args[0])))
        }
        TypeRef::Generic { base, args } if base == "Seq" && args.len() == 1 => {
            TypeKind::Seq(Box::new(type_ref_kind(&args[0])))
        }
        TypeRef::Generic { base, args } if base == "Map" && args.len() == 2 => TypeKind::Map {
            key: Box::new(type_ref_kind(&args[0])),
            value: Box::new(type_ref_kind(&args[1])),
        },
        TypeRef::Generic { base, args } if base == "Result" && args.len() == 2 => {
            TypeKind::Result {
                ok: Box::new(type_ref_kind(&args[0])),
                error: Box::new(type_ref_kind(&args[1])),
            }
        }
        TypeRef::Generic { base, args } if base == "Need" && args.len() == 2 => TypeKind::Need {
            ready: Box::new(type_ref_kind(&args[0])),
            error: Box::new(type_ref_kind(&args[1])),
        },
        TypeRef::Ref { lifetime, inner } => TypeKind::BorrowRef {
            lifetime: lifetime
                .as_ref()
                .map(|lifetime| LifetimeScopeKind::parse(lifetime.name())),
            inner: Box::new(type_ref_kind(inner)),
        },
        TypeRef::Slice(inner) => TypeKind::Slice(Box::new(type_ref_kind(inner))),
        TypeRef::Generic { .. } => TypeKind::Named(type_ref_label(ty)),
    }
}

fn type_ref_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Never => "Never".to_owned(),
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

    /// Registers a checker capability such as `state.write(flow)`.
    #[must_use]
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.insert(capability.into());
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

    /// Returns whether the environment grants a named effect or state capability.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
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
