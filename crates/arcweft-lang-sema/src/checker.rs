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
pub mod helpers;
pub mod line_plan;
pub mod module;
pub mod presentation;
pub mod source;
pub mod stmt;

use helpers::{
    array_len_matches, array_repeat_len_label, await_branch_pattern_type, choice_output_type,
    collection_index_type, default_presentation_slot_family, entity_kind, entity_kind_for_decl,
    expr_path_label, first_arg_type, ident_pattern_name, is_character_entity_literal,
    is_dialogue_callee_type, is_drop_callee, is_drop_name, is_local_ident, iter_item_type,
    let_else_bindings, literal_type, merge_line_output, pattern_bindings_with_fallback,
    result_ok_type, source_return_types, stmts_diverge, stream_return_types, type_ref_kind,
    typed_pattern_binding, unify_loop_break_types, well_known_capacity_method_type,
    well_known_field_type, well_known_runtime_method_type,
};

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
