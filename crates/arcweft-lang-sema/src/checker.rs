use crate::borrow::{BorrowLocalState, BorrowStateSnapshot, merge_borrow_local_states};
use crate::diagnostics::{TypeCheckError, TypeCheckReadinessError, TypeCheckWarning};
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
    types::{FnParam, FnParamKind, FnSignature, TypeRef},
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
    is_drop_callee, is_local_ident, iter_item_type, merge_line_output, normalize_choice_type,
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

/// Deterministic counters collected while type checking lowered HIR.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeCheckStats {
    pub flows: usize,
    pub functions: usize,
    pub declarations: usize,
    pub top_level_items: usize,
    pub statements: usize,
    pub expressions: usize,
    pub judgments: usize,
    pub borrow_binding_groups: usize,
    pub borrow_bindings: usize,
    pub borrow_state_snapshots: usize,
    pub borrow_state_restores: usize,
    pub borrow_state_merges: usize,
    pub borrow_boundary_checks: usize,
    pub borrow_escape_checks: usize,
    pub max_active_borrows: usize,
}

/// Stable identifier for a recorded type-check judgment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeJudgmentId(usize);

impl TypeJudgmentId {
    /// Creates an identifier from a zero-based judgment index.
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based index of this judgment in its report.
    pub const fn index(self) -> usize {
        self.0
    }
}

/// HIR subject proven by a type-check judgment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeJudgmentSubject {
    /// An expression was assigned a type.
    Expr { kind: String },
    /// A let binding pattern was assigned a type.
    LetBinding { pattern: String },
    /// A function or flow return expression was assigned a type.
    Return { context: String },
    /// A contextual expected-type check was applied.
    Expected { context: String },
}

/// Rule family used to derive a type-check judgment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeJudgmentRule {
    /// General expression inference or checking.
    Expr,
    /// Expression checked against an expected type.
    Expected,
    /// Let binding annotation and expression reconciliation.
    LetBinding,
    /// Return context checking.
    Return,
}

/// Machine-readable evidence for one successful type-check decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeJudgment {
    pub id: TypeJudgmentId,
    pub subject: TypeJudgmentSubject,
    pub ty: TypeKind,
    pub rule: TypeJudgmentRule,
    pub expected: Option<TypeKind>,
}

/// Machine-readable type-check result used by tooling and profiling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeCheckReport {
    pub diagnostics: Vec<TypeCheckError>,
    pub warnings: Vec<TypeCheckWarning>,
    pub stats: TypeCheckStats,
    pub judgments: Vec<TypeJudgment>,
}

impl TypeCheckReport {
    pub fn into_result(self) -> Result<(), Vec<TypeCheckError>> {
        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(self.diagnostics)
        }
    }
}

/// Analyzes lowered HIR with an explicit symbol/method environment.
pub fn analyze_types(module: &HirModule, env: &TypeCheckEnv) -> TypeCheckReport {
    let mut checker = TypeChecker::new(env);
    checker.check_module(module);
    TypeCheckReport {
        diagnostics: checker.errors,
        warnings: checker.warnings,
        stats: checker.stats,
        judgments: checker.judgments,
    }
}

/// Type-checks the lowered HIR with an explicit symbol/method environment.
///
/// This is deliberately small but real: it verifies entity reference families,
/// dialogue callees, awaited `Need<T, E>` values, timed cue durations, and
/// expression symbols without reparsing source text.
pub fn typecheck_hir(module: &HirModule, env: &TypeCheckEnv) -> Result<(), Vec<TypeCheckError>> {
    analyze_types(module, env).into_result()
}

struct TypeChecker<'a> {
    env: &'a TypeCheckEnv,
    errors: Vec<TypeCheckError>,
    warnings: Vec<TypeCheckWarning>,
    active_borrows: Vec<String>,
    borrow_local_lifetimes: HashMap<String, BorrowLocalState>,
    global_symbols: HashMap<String, TypeKind>,
    global_functions: HashMap<String, TypeKind>,
    global_function_signatures: HashMap<String, FunctionSignatureType>,
    global_function_effects: HashMap<String, Vec<String>>,
    global_type_aliases: HashMap<String, TypeKind>,
    flow_params: HashMap<String, HashSet<String>>,
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
    expected_returns: Vec<TypeKind>,
    yield_stack: Vec<YieldContext>,
    stats: TypeCheckStats,
    judgments: Vec<TypeJudgment>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct FunctionSignatureType {
    params: Vec<FunctionParamType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FunctionParamType {
    name: Option<String>,
    ty: TypeKind,
    kind: FnParamKind,
    has_default: bool,
}

impl FunctionParamType {
    const fn is_rest(&self) -> bool {
        matches!(self.kind, FnParamKind::Rest)
    }
}

impl TypeChecker<'_> {
    fn new(env: &TypeCheckEnv) -> TypeChecker<'_> {
        TypeChecker {
            env,
            errors: Vec::new(),
            warnings: Vec::new(),
            active_borrows: Vec::new(),
            borrow_local_lifetimes: HashMap::new(),
            global_symbols: HashMap::new(),
            global_functions: HashMap::new(),
            global_function_signatures: HashMap::new(),
            global_function_effects: HashMap::new(),
            global_type_aliases: HashMap::new(),
            flow_params: HashMap::new(),
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
            expected_returns: Vec::new(),
            yield_stack: Vec::new(),
            stats: TypeCheckStats::default(),
            judgments: Vec::new(),
        }
    }

    fn record_type_judgment(
        &mut self,
        subject: TypeJudgmentSubject,
        rule: TypeJudgmentRule,
        ty: TypeKind,
        expected: Option<&TypeKind>,
    ) -> TypeJudgmentId {
        let id = TypeJudgmentId(self.judgments.len());
        self.judgments.push(TypeJudgment {
            id,
            subject,
            ty,
            rule,
            expected: expected.cloned(),
        });
        self.stats.judgments = self.judgments.len();
        id
    }

    fn record_active_borrow_depth(&mut self) {
        self.stats.max_active_borrows =
            self.stats.max_active_borrows.max(self.active_borrows.len());
    }

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

    fn function_signature(&self, name: &str) -> Option<&FunctionSignatureType> {
        self.global_function_signatures.get(name)
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

fn function_signature_type(signature: &FnSignature) -> FunctionSignatureType {
    let params = signature
        .param_groups()
        .iter()
        .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        .map(function_param_type)
        .collect();
    FunctionSignatureType { params }
}

fn function_param_type(param: &FnParam) -> FunctionParamType {
    FunctionParamType {
        name: pattern_param_name(param.pattern()),
        ty: type_ref_kind(param.ty()),
        kind: param.kind(),
        has_default: param.default().is_some(),
    }
}

fn function_param_local_type(param: &FnParam) -> TypeKind {
    let ty = type_ref_kind(param.ty());
    if param.is_rest() {
        TypeKind::Vec(Box::new(ty))
    } else {
        ty
    }
}

fn pattern_param_name(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Some(name.clone())
        }
        _ => None,
    }
}

fn types_compatible(expected: &TypeKind, actual: &TypeKind) -> bool {
    if expected == actual || matches!(expected, TypeKind::Named(name) if name == "_") {
        return true;
    }
    match (expected, actual) {
        (TypeKind::Choice(alternatives), TypeKind::Choice(actual_alternatives)) => {
            actual_alternatives
                .iter()
                .all(|actual| choice_injection_target(alternatives, actual).is_some())
        }
        (TypeKind::Choice(alternatives), actual) => {
            choice_injection_target(alternatives, actual).is_some()
        }
        (expected, TypeKind::Choice(alternatives)) => alternatives
            .iter()
            .all(|actual| types_compatible(expected, actual)),
        (
            TypeKind::Result {
                ok: expected_ok,
                error: expected_error,
            },
            TypeKind::Result {
                ok: actual_ok,
                error: actual_error,
            },
        ) => {
            types_compatible(expected_ok, actual_ok)
                && (types_compatible(expected_error, actual_error)
                    || matches!(actual_error.as_ref(), TypeKind::Named(name) if name == "_"))
        }
        (TypeKind::Option(expected), TypeKind::Option(actual)) => {
            types_compatible(expected, actual)
                || matches!(actual.as_ref(), TypeKind::Named(name) if name == "_")
        }
        _ => false,
    }
}

fn choice_injection_target<'a>(
    alternatives: &'a [TypeKind],
    actual: &TypeKind,
) -> Option<&'a TypeKind> {
    let mut compatible_alternatives = alternatives
        .iter()
        .filter(|alternative| types_compatible(alternative, actual));
    let selected = compatible_alternatives.next()?;
    compatible_alternatives.next().is_none().then_some(selected)
}
