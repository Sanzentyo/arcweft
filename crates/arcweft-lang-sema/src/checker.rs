use crate::borrow::{BorrowLocalState, BorrowStateSnapshot, merge_borrow_local_states};
use crate::diagnostics::{TypeCheckError, TypeCheckReadinessError};
use crate::env::TypeCheckEnv;
use crate::fact_layer::{EffectScope, capability_from_expr};
use crate::lifetime::{
    collect_type_kind_lifetimes, lifetime_key, lifetime_value_type, type_contains_borrow_ref,
};
use crate::symbols::{SymbolUseKind, collect_symbol_uses};
use crate::types::{EntityKind, MapKind, TypeKind};
use arcweft_lang_hir::model::{HirFlowItem, HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::{
    ast::{
        choice::ChoiceAction,
        dialogue::DialogueToken,
        flow::{AwaitBranchKind, ContractClause, FlowKind, SelectBranchHead, Stmt},
        ids::{EntityRef, EntityRefSyntax, IdRef},
        items::{EntityDeclKind, FunctionKind},
        line_plan::{CancelRuleSyntax, LinePlanItem, TriggerPattern},
        pattern::{Pattern, VariantPatternPayload},
        source::{
            SourceBackpressurePolicy, SourceEventPattern, SourceHeader, SourcePrivacyPolicy,
            SourceReplayPolicy,
        },
    },
    expr::{
        BinaryOp, ComputationBlockKind, Expr, LifetimeAccessMode, LifetimeKey, LifetimeScopeKind,
        Literal, MatchExprArm, UnaryOp,
    },
    types::TypeRef,
};
use std::collections::{HashMap, HashSet};

pub mod borrow_state;
pub mod choice;
pub mod effects;
pub mod expr;
pub mod flow;
pub mod line_plan;
pub mod module;
pub mod source;
pub mod stmt;

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
        borrow_local_lifetimes: HashMap::new(),
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
        yield_stack: Vec::new(),
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
    borrow_local_lifetimes: HashMap<String, BorrowLocalState>,
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
    yield_stack: Vec<YieldContext>,
}

#[derive(Clone, Debug)]
struct TypeCheckerScopeSnapshot {
    active_borrows: Vec<String>,
    borrow_local_lifetimes: HashMap<String, BorrowLocalState>,
    locals: HashMap<String, TypeKind>,
    active_presentation_defaults: HashMap<String, String>,
    lifetime_guarantees: HashSet<LifetimeKey>,
    dropped_lifetime_keys: HashSet<LifetimeKey>,
    available_lifetimes: Vec<LifetimeScopeKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum YieldContext {
    Seq {
        item_ty: Option<TypeKind>,
        yield_count: usize,
    },
    Stream {
        item_ty: TypeKind,
        error_ty: TypeKind,
        yield_count: usize,
    },
    Source {
        item_ty: TypeKind,
        error_ty: TypeKind,
        yield_count: usize,
    },
}

#[derive(Clone, Debug, Default)]
struct LoopContext {
    label: Option<String>,
    allows_value_break: bool,
    break_types: Vec<TypeKind>,
}

impl TypeChecker<'_> {
    pub(super) fn check_yield_stmt(&mut self, expr: &Expr) {
        self.reject_active_borrows("yield suspension boundary");
        let actual = self.check_expr(expr);
        let Some(context) = self.yield_stack.last_mut() else {
            self.errors.push(TypeCheckError::new(
                "`yield` is only valid in `seq`, `stream`, or `source` contexts".to_owned(),
            ));
            return;
        };
        match context {
            YieldContext::Seq {
                item_ty,
                yield_count,
            } => {
                *yield_count += 1;
                if let Some(actual) = actual {
                    match item_ty {
                        Some(expected) if expected != &actual => {
                            self.errors.push(TypeCheckError::new(format!(
                                "yielded item types do not match, found {expected:?} and {actual:?}"
                            )));
                        }
                        Some(_) => {}
                        None => *item_ty = Some(actual),
                    }
                }
            }
            YieldContext::Stream {
                item_ty,
                yield_count,
                ..
            }
            | YieldContext::Source {
                item_ty,
                yield_count,
                ..
            } => {
                *yield_count += 1;
                if let Some(actual) = actual
                    && &actual != item_ty
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "yielded item must have type {item_ty:?}, found {actual:?}"
                    )));
                }
            }
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

    fn check_await_item(
        &mut self,
        await_with: &arcweft_lang_hir::model::HirAwait,
    ) -> Option<TypeKind> {
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
            let borrow_snapshot = self.snapshot_borrow_state();
            let outer_locals = self.locals.clone();
            let branch_type = await_branch_pattern_type(branch.kind(), &ready, &error);
            for (name, ty) in let_else_bindings(branch.pattern(), Some(&branch_type)) {
                self.locals.insert(name, ty);
            }
            self.check_flow_items(branch.body());
            self.restore_borrow_state(borrow_snapshot);
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
        block: &arcweft_lang_hir::model::HirLoop,
        allows_value_break: bool,
    ) -> Option<TypeKind> {
        let borrow_snapshot = self.snapshot_borrow_state();
        self.loop_stack.push(LoopContext {
            label: block.label().map(str::to_owned),
            allows_value_break,
            break_types: Vec::new(),
        });
        self.check_flow_items(block.body());
        let context = self.loop_stack.pop()?;
        self.restore_borrow_state(borrow_snapshot);
        unify_loop_break_types(&context.break_types)
    }

    fn check_while_block(&mut self, block: &arcweft_lang_hir::model::HirWhile) {
        self.expect_expr_type(block.condition(), &TypeKind::Bool, "while condition");
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
    }

    fn check_if_let_block(&mut self, block: &arcweft_lang_hir::model::HirIfLet) {
        let expr_type = self.check_expr(block.expr());
        if let Some(guard) = block.guard() {
            self.expect_expr_type(guard, &TypeKind::Bool, "if-let guard");
        }
        let borrow_snapshot = self.snapshot_borrow_state();
        let outer_locals = self.locals.clone();
        for (name, ty) in let_else_bindings(block.pattern(), expr_type.as_ref()) {
            self.locals.insert(name, ty);
        }
        self.check_flow_items(block.body());
        let then_state = self.snapshot_borrow_state();
        self.merge_borrow_state_from_paths(
            &borrow_snapshot,
            &[borrow_snapshot.clone(), then_state],
        );
        self.locals = outer_locals;
    }

    fn check_while_let_block(&mut self, block: &arcweft_lang_hir::model::HirWhileLet) {
        let expr_type = self.check_expr(block.expr());
        if let Some(guard) = block.guard() {
            self.expect_expr_type(guard, &TypeKind::Bool, "while-let guard");
        }
        let borrow_snapshot = self.snapshot_borrow_state();
        let outer_locals = self.locals.clone();
        for (name, ty) in let_else_bindings(block.pattern(), expr_type.as_ref()) {
            self.locals.insert(name, ty);
        }
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
        self.restore_borrow_state(borrow_snapshot);
        self.locals = outer_locals;
    }

    fn check_for_block(&mut self, block: &arcweft_lang_hir::model::HirFor) {
        self.check_expr(block.source());
        let borrow_snapshot = self.snapshot_borrow_state();
        self.with_statement_loop(|this| this.check_flow_items(block.body()));
        self.restore_borrow_state(borrow_snapshot);
    }

    fn with_statement_loop(&mut self, check_body: impl FnOnce(&mut Self)) {
        let borrow_snapshot = self.snapshot_borrow_state();
        self.loop_stack.push(LoopContext {
            label: None,
            allows_value_break: false,
            break_types: Vec::new(),
        });
        check_body(self);
        self.loop_stack.pop();
        self.restore_borrow_state(borrow_snapshot);
    }

    fn reject_active_borrows(&mut self, boundary: &str) {
        if !self.active_borrows.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "borrowed values with lifetimes {:?} cannot cross {boundary}",
                self.active_borrows
            )));
        }
    }

    fn reject_borrow_escape(&mut self, ty: Option<&TypeKind>, destination: &str) {
        if ty.is_some_and(type_contains_borrow_ref) {
            self.errors.push(TypeCheckError::new(format!(
                "borrowed value cannot escape through {destination}"
            )));
        }
    }

    fn snapshot_runtime_scope(&self) -> TypeCheckerScopeSnapshot {
        TypeCheckerScopeSnapshot {
            active_borrows: self.active_borrows.clone(),
            borrow_local_lifetimes: self.borrow_local_lifetimes.clone(),
            locals: self.locals.clone(),
            active_presentation_defaults: self.active_presentation_defaults.clone(),
            lifetime_guarantees: self.lifetime_guarantees.clone(),
            dropped_lifetime_keys: self.dropped_lifetime_keys.clone(),
            available_lifetimes: self.available_lifetimes.clone(),
        }
    }

    fn restore_runtime_scope(&mut self, snapshot: TypeCheckerScopeSnapshot) {
        self.active_borrows = snapshot.active_borrows;
        self.borrow_local_lifetimes = snapshot.borrow_local_lifetimes;
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

    fn release_direct_drop_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call { callee, args } if is_drop_callee(callee) => {
                for arg in args {
                    if let Expr::Path(name) = arg {
                        self.release_borrow_local(name);
                    }
                }
            }
            Expr::MethodCall {
                receiver, method, ..
            } if is_drop_name(method) => {
                if let Expr::Path(name) = receiver.as_ref() {
                    self.release_borrow_local(name);
                }
            }
            Expr::Pipe { lhs, rhs } if is_drop_callee(rhs) => {
                if let Expr::Path(name) = lhs.as_ref() {
                    self.release_borrow_local(name);
                }
            }
            _ => {}
        }
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

    fn check_unary_expr(&mut self, op: UnaryOp, expr: &Expr) -> TypeKind {
        match op {
            UnaryOp::Not => {
                self.expect_expr_type(expr, &TypeKind::Bool, "not operand");
                TypeKind::Bool
            }
            UnaryOp::Neg => match self.check_expr(expr) {
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
        if is_drop_name(method) {
            return Some(TypeKind::Unit);
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
        self.reject_borrow_escape(ty.as_ref(), "block final value");
        self.locals = outer_locals;
        ty
    }

    fn check_computation_block(
        &mut self,
        kind: ComputationBlockKind,
        statements: &[Stmt],
        value: Option<&Expr>,
    ) -> Option<TypeKind> {
        match kind {
            ComputationBlockKind::Result | ComputationBlockKind::Task => {
                self.check_block_expr(statements, value)
            }
            ComputationBlockKind::Seq => {
                self.yield_stack.push(YieldContext::Seq {
                    item_ty: None,
                    yield_count: 0,
                });
                self.check_block_expr(statements, value);
                let Some(YieldContext::Seq { item_ty, .. }) = self.yield_stack.pop() else {
                    return None;
                };
                Some(TypeKind::Seq(Box::new(item_ty.unwrap_or(TypeKind::Unit))))
            }
            ComputationBlockKind::Stream => {
                self.yield_stack.push(YieldContext::Seq {
                    item_ty: None,
                    yield_count: 0,
                });
                self.check_block_expr(statements, value);
                let Some(YieldContext::Seq { item_ty, .. }) = self.yield_stack.pop() else {
                    return None;
                };
                Some(TypeKind::Stream {
                    item: Box::new(item_ty.unwrap_or(TypeKind::Unit)),
                    error: Box::new(TypeKind::Unit),
                })
            }
        }
    }

    fn check_if_expr(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Option<TypeKind> {
        self.expect_expr_type(condition, &TypeKind::Bool, "if expression condition");
        let base_borrow_snapshot = self.snapshot_borrow_state();
        let then_type = self.check_expr(then_branch);
        let then_borrow_state = self.snapshot_borrow_state();
        self.restore_borrow_state(base_borrow_snapshot.clone());
        let else_type = else_branch.and_then(|branch| self.check_expr(branch));
        let else_borrow_state = self.snapshot_borrow_state();
        if else_branch.is_some() {
            self.merge_borrow_state_from_paths(
                &base_borrow_snapshot,
                &[then_borrow_state, else_borrow_state],
            );
        } else {
            self.merge_borrow_state_from_paths(
                &base_borrow_snapshot,
                &[base_borrow_snapshot.clone(), then_borrow_state],
            );
        }
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

    fn check_match_expr(&mut self, scrutinee: &Expr, arms: &[MatchExprArm]) -> Option<TypeKind> {
        let scrutinee_type = self.check_expr(scrutinee);
        if arms.is_empty() {
            self.errors.push(TypeCheckError::new(
                "match expression must have at least one arm".to_owned(),
            ));
            return None;
        }

        let base_borrow_snapshot = self.snapshot_borrow_state();
        let mut arm_states = Vec::new();
        let mut inferred = None;
        for arm in arms {
            self.restore_borrow_state(base_borrow_snapshot.clone());
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
            arm_states.push(self.snapshot_borrow_state());
        }
        self.merge_borrow_state_from_paths(&base_borrow_snapshot, &arm_states);
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

        let base_borrow_snapshot = self.snapshot_borrow_state();
        let outer_locals = self.locals.clone();
        for (name, ty) in let_else_bindings(pattern, expr_type.as_ref()) {
            self.locals.insert(name, ty);
        }
        let then_type = self.check_expr(then_branch);
        let then_borrow_state = self.snapshot_borrow_state();
        self.restore_borrow_state(base_borrow_snapshot.clone());
        self.locals = outer_locals;

        let else_type = else_branch.and_then(|branch| self.check_expr(branch));
        let else_borrow_state = self.snapshot_borrow_state();
        if else_branch.is_some() {
            self.merge_borrow_state_from_paths(
                &base_borrow_snapshot,
                &[then_borrow_state, else_borrow_state],
            );
        } else {
            self.merge_borrow_state_from_paths(
                &base_borrow_snapshot,
                &[base_borrow_snapshot.clone(), then_borrow_state],
            );
        }
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
        "metric" => EntityKind::Metric,
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

fn entity_kind_for_decl(kind: EntityDeclKind) -> EntityKind {
    match kind {
        EntityDeclKind::Character => EntityKind::Character,
        EntityDeclKind::Component => EntityKind::Component,
        EntityDeclKind::Activity => EntityKind::Activity,
        EntityDeclKind::Signal => EntityKind::Signal,
        EntityDeclKind::Metric => EntityKind::Metric,
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
        Some(
            TypeKind::Vec(item)
            | TypeKind::Array { item, .. }
            | TypeKind::Seq(item)
            | TypeKind::Slice(item),
        ) => item.as_ref().clone(),
        Some(TypeKind::Named(name)) => named_iter_item_type(name).map_or_else(
            || TypeKind::Named("ChoiceOptionSource".to_owned()),
            TypeKind::Named,
        ),
        _ => TypeKind::Named("ChoiceOptionSource".to_owned()),
    }
}

pub(crate) fn named_iter_item_type(name: &str) -> Option<String> {
    if let Some(inner) = generic_named_type_arg(name, "Vec")
        .or_else(|| generic_named_type_arg(name, "Seq"))
        .or_else(|| generic_named_type_arg(name, "Slice"))
    {
        return Some(inner.to_owned());
    }
    let inner = generic_named_type_arg(name, "Array")?;
    Some(
        inner
            .split_once(',')
            .map_or(inner, |(item, _)| item)
            .trim()
            .to_owned(),
    )
}

fn generic_named_type_arg<'a>(name: &'a str, base: &str) -> Option<&'a str> {
    name.strip_prefix(base)?
        .strip_prefix('<')?
        .strip_suffix('>')
        .map(str::trim)
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
        Pattern::BracketSeq { items, rest } => {
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
        Pattern::BracketSeq { items, rest } => {
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
                VariantPatternPayload::Tuple(items) => items
                    .iter()
                    .flat_map(collect_pattern_binding_names)
                    .collect::<Vec<_>>(),
                VariantPatternPayload::Record { fields, .. } => fields
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

fn variant_payload_bindings(payload: &VariantPatternPayload) -> Vec<String> {
    match payload {
        VariantPatternPayload::Tuple(items) => items
            .iter()
            .filter_map(|pattern| match pattern {
                Pattern::Ident(name) if is_local_ident(name) => Some(name.to_owned()),
                _ => None,
            })
            .collect(),
        VariantPatternPayload::Record { fields, .. } => fields
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

fn is_drop_callee(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(name) if is_drop_name(name))
        || matches!(expr, Expr::Call { callee, .. } if is_drop_callee(callee))
}

fn is_drop_name(name: &str) -> bool {
    matches!(name, "drop" | "drop_optional" | "on_drop")
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
            "drop"
                | "drop_optional"
                | "on_drop"
                | "signal.set"
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
        "Vec.with_capacity" => Some(TypeKind::Vec(Box::new(TypeKind::Named("_".to_owned())))),
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
    matches!(ty, TypeKind::Vec(_) | TypeKind::String)
        || matches!(ty, TypeKind::Named(name) if name == "Bytes")
}

fn collection_index_type(target_type: &TypeKind) -> Option<TypeKind> {
    match target_type {
        TypeKind::Vec(item) | TypeKind::Array { item, .. } | TypeKind::Slice(item) => {
            Some(item.as_ref().clone())
        }
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
            raw.source().starts_with("break")
                || raw.source().starts_with("panic ")
                || raw.source().starts_with("fail ")
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

fn choice_output_type(choice: &arcweft_lang_hir::model::HirChoice) -> Option<TypeKind> {
    let mut inferred = None;
    for option in choice.options() {
        let ty = match option.action() {
            ChoiceAction::Out(expr) => simple_expr_type(expr)?,
            ChoiceAction::SelectBlock(statements) => {
                let [Stmt::Out { expr, .. }] = statements.as_slice() else {
                    return None;
                };
                simple_expr_type(expr)?
            }
            ChoiceAction::Goto(_) | ChoiceAction::None => return None,
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
        Expr::BracketSeq(items) => {
            let item = items
                .first()
                .and_then(simple_expr_type)
                .unwrap_or(TypeKind::Unit);
            Some(TypeKind::Vec(Box::new(item)))
        }
        Expr::ArrayRepeat { value, len } => {
            let item = simple_expr_type(value)?;
            Some(TypeKind::Array {
                item: Box::new(item),
                len: array_repeat_len_label(len)?,
            })
        }
        Expr::RecordLiteral(_) => Some(TypeKind::Named("Record".to_owned())),
        _ => None,
    }
}

fn array_repeat_len_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(Literal::Int(value)) if *value >= 0 => Some(value.to_string()),
        _ => None,
    }
}

fn array_len_matches(label: &str, actual: usize) -> bool {
    label
        .parse::<usize>()
        .ok()
        .or_else(|| label.strip_prefix('N')?.parse::<usize>().ok())
        .is_none_or(|expected| expected == actual)
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

fn is_map_type_name(name: &str) -> bool {
    matches!(name, "OrderedMap" | "SortedMap" | "BTreeMap")
}

fn map_kind_for_type_name(name: &str) -> MapKind {
    match name {
        "OrderedMap" => MapKind::Ordered,
        "SortedMap" => MapKind::Sorted,
        "BTreeMap" => MapKind::BTree,
        _ => unreachable!("map type names are filtered before kind selection"),
    }
}

pub(crate) fn type_ref_kind(ty: &TypeRef) -> TypeKind {
    match ty {
        TypeRef::Never => TypeKind::Never,
        TypeRef::ConstInt(value) => TypeKind::Named(value.to_string()),
        TypeRef::Path(path) => named_type_label(path),
        TypeRef::Generic { base, args } if base == "Vec" && args.len() == 1 => {
            TypeKind::Vec(Box::new(type_ref_kind(&args[0])))
        }
        TypeRef::Generic { base, args } if base == "Array" && args.len() == 2 => TypeKind::Array {
            item: Box::new(type_ref_kind(&args[0])),
            len: type_ref_label(&args[1]),
        },
        TypeRef::Generic { base, args } if base == "Seq" && args.len() == 1 => {
            TypeKind::Seq(Box::new(type_ref_kind(&args[0])))
        }
        TypeRef::Generic { base, args } if is_map_type_name(base) && args.len() == 2 => {
            TypeKind::Map {
                kind: map_kind_for_type_name(base),
                key: Box::new(type_ref_kind(&args[0])),
                value: Box::new(type_ref_kind(&args[1])),
            }
        }
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
        TypeRef::Generic { base, args } if base == "Stream" && args.len() == 2 => {
            TypeKind::Stream {
                item: Box::new(type_ref_kind(&args[0])),
                error: Box::new(type_ref_kind(&args[1])),
            }
        }
        TypeRef::Generic { base, args } if base == "Source" && args.len() == 2 => {
            TypeKind::Source {
                item: Box::new(type_ref_kind(&args[0])),
                error: Box::new(type_ref_kind(&args[1])),
            }
        }
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

fn stream_return_types(ty: &TypeRef) -> Option<(TypeKind, TypeKind)> {
    match ty {
        TypeRef::Generic { base, args } if base == "Stream" && args.len() == 2 => {
            Some((type_ref_kind(&args[0]), type_ref_kind(&args[1])))
        }
        TypeRef::Generic { base, .. } if base == "Source" => None,
        _ => None,
    }
}

fn source_return_types(ty: &TypeRef) -> Option<(TypeKind, TypeKind)> {
    match ty {
        TypeRef::Generic { base, args } if base == "Source" && args.len() == 2 => {
            Some((type_ref_kind(&args[0]), type_ref_kind(&args[1])))
        }
        _ => None,
    }
}

fn type_ref_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Never => "Never".to_owned(),
        TypeRef::ConstInt(value) => value.to_string(),
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
