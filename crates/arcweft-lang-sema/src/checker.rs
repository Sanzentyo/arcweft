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
    expr::{Expr, LifetimeAccessMode, LifetimeKey, LifetimeScopeKind, Literal},
    types::TypeRef,
};
use std::collections::{HashMap, HashSet};

pub mod borrow_state;
pub mod choice;
pub mod effects;
pub mod expr;
pub mod flow;
pub mod helpers;
pub mod lifetime_access;
pub mod line_plan;
pub mod module;
pub mod presentation;
pub mod source;
pub mod stmt;
pub mod suspension;

use helpers::{
    await_branch_pattern_type, choice_output_type, default_presentation_slot_family, entity_kind,
    entity_kind_for_decl, ident_pattern_name, is_character_entity_literal, is_dialogue_callee_type,
    is_drop_callee, is_local_ident, iter_item_type, merge_line_output,
    pattern_bindings_with_fallback, source_return_types, stmts_diverge, stream_return_types,
    type_ref_kind, typed_pattern_binding, unify_loop_break_types,
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
        global_symbols: HashMap::new(),
        global_functions: HashMap::new(),
        locals: HashMap::new(),
        loop_stack: Vec::new(),
        line_label_stack: Vec::new(),
        line_cancel_depth: 0,
        line_out_depth: 0,
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
    global_symbols: HashMap<String, TypeKind>,
    global_functions: HashMap<String, TypeKind>,
    locals: HashMap<String, TypeKind>,
    loop_stack: Vec<LoopContext>,
    line_label_stack: Vec<Option<String>>,
    line_cancel_depth: usize,
    line_out_depth: usize,
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
    fn symbol_type(&self, name: &str) -> Option<&TypeKind> {
        self.locals
            .get(name)
            .or_else(|| self.global_symbols.get(name))
            .or_else(|| self.env.symbol_type(name))
    }

    fn function_type(&self, name: &str) -> Option<&TypeKind> {
        self.global_functions
            .get(name)
            .or_else(|| self.env.function_type(name))
    }

    fn is_dialogue_callee(&self, callee: &str) -> bool {
        if is_dialogue_callee_type(self.symbol_type(callee)) {
            return true;
        }
        callee.strip_suffix(".say").is_some_and(|receiver| {
            is_dialogue_callee_type(self.symbol_type(receiver))
                || is_character_entity_literal(receiver)
        })
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
}
