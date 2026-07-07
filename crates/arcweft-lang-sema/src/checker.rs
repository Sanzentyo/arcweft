use crate::borrow::{
    BorrowLocalState, BorrowStateCheckpoint, BorrowStateDelta, BorrowStateDeltaEntry,
    BorrowStateJournalEntry, merge_borrow_local_states,
};
use crate::diagnostics::{
    TraitDiagnostic, TypeCheckError, TypeCheckReadinessError, TypeCheckWarning,
};
use crate::effect_analysis::EffectAnalysisReport;
use crate::effect_collector::EffectCollector;
use crate::effects::{EffectId, EffectSet};
use crate::env::{
    AgentActionEnvParam, DebugPathKind, EffectCapability, FunctionParam, FunctionSignature,
    TypeCheckEnv,
};
use crate::fact_layer::{EffectScope, capability_from_expr};
use crate::lifetime::{
    collect_type_kind_lifetimes, lifetime_key, lifetime_value_type, type_contains_borrow_ref,
};
use crate::symbols::{SymbolUseKind, collect_symbol_uses};
use crate::traits::{
    ProjectionError, ProjectionResolution, TraitCatalog, TraitPredicate, collect_trait_catalog,
};
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
    expr::{CallArg, Expr, LifetimeAccessMode, LifetimeKey, LifetimeScopeKind, Literal},
    types::{FnParam, FnSignature, TypeRef},
};
use std::collections::{BTreeMap, HashMap, HashSet};

pub mod borrow_state;
pub mod choice;
pub mod effects;
pub mod expr;
pub mod flow;
pub mod helpers;
pub mod iterator;
pub mod lifetime_access;
pub mod line_plan;
pub mod module;
pub mod presentation;
pub mod source;
pub mod stmt;
pub mod suspension;

use helpers::{
    await_branch_pattern_type, choice_output_type, default_presentation_slot_family, entity_kind,
    entity_kind_for_decl, entity_syntax_kind, ident_pattern_name, is_character_entity_literal,
    is_dialogue_callee_type, is_drop_callee, is_local_ident, iter_item_type, merge_line_output,
    normalize_choice_type, pattern_bindings_with_fallback, source_return_types, stmts_diverge,
    stream_return_types, type_ref_kind, typed_pattern_binding, unify_loop_break_types,
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
    pub expr_judgments: usize,
    pub expected_judgments: usize,
    pub let_binding_judgments: usize,
    pub return_judgments: usize,
    pub type_compatibility_checks: usize,
    pub borrow_binding_groups: usize,
    pub borrow_bindings: usize,
    pub borrow_state_snapshots: usize,
    pub borrow_state_restores: usize,
    pub borrow_state_merges: usize,
    pub borrow_state_cloned_bindings: usize,
    pub borrow_state_delta_entries: usize,
    pub borrow_state_full_clones: usize,
    pub borrow_state_merge_keys: usize,
    pub borrow_boundary_checks: usize,
    pub borrow_escape_checks: usize,
    pub active_borrow_removes: usize,
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

/// Stable identifier for an expression visited by the type checker.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeExpressionId(usize);

impl TypeExpressionId {
    /// Creates an identifier from a zero-based expression traversal index.
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based expression traversal index in this report.
    pub const fn index(self) -> usize {
        self.0
    }
}

/// HIR subject proven by a type-check judgment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeJudgmentSubject {
    /// An expression was assigned a type.
    Expr {
        id: TypeExpressionId,
        kind: &'static str,
    },
    /// A let binding pattern was assigned a type.
    LetBinding { pattern: String },
    /// A function or flow return expression was assigned a type.
    Return { context: String },
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

/// Expected-type evidence attached to a type-check judgment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeJudgmentExpected {
    /// The expected type is identical to the judgment type.
    SameAsJudgment,
    /// The expected type differs from the judgment type but was still the check context.
    Other(TypeKind),
}

/// Machine-readable evidence for one successful type-check decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeJudgment {
    pub id: TypeJudgmentId,
    pub subject: TypeJudgmentSubject,
    pub ty: TypeKind,
    pub rule: TypeJudgmentRule,
    pub expected: Option<TypeJudgmentExpected>,
}

impl TypeJudgment {
    /// Returns the expected type context for this judgment, if one was present.
    pub const fn expected_type(&self) -> Option<&TypeKind> {
        match &self.expected {
            Some(TypeJudgmentExpected::SameAsJudgment) => Some(&self.ty),
            Some(TypeJudgmentExpected::Other(expected)) => Some(expected),
            None => None,
        }
    }
}

/// Machine-readable evidence that affects runtime-plan lowering choices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedLoweringEvidence {
    pub expression_id: TypeExpressionId,
    pub kind: TypedLoweringEvidenceKind,
}

/// Lowering-sensitive semantic facts proven during type checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypedLoweringEvidenceKind {
    /// A call expression's callee evaluated to a function value.
    FunctionValueCall {
        callee: Option<String>,
        callee_ty: TypeKind,
        result_ty: TypeKind,
        arg_count: usize,
    },
    /// An expression was checked in a function-typed context.
    ExpectedFunctionValue {
        expected_ty: TypeKind,
        actual_ty: TypeKind,
        arity: usize,
    },
    /// A method-call expression resolved through data-last callable fallback.
    DataLastMethodFallback { method: String, arg_count: usize },
}

/// Machine-readable type-check result used by tooling and profiling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeCheckReport {
    pub diagnostics: Vec<TypeCheckError>,
    pub warnings: Vec<TypeCheckWarning>,
    pub stats: TypeCheckStats,
    pub judgments: Vec<TypeJudgment>,
    pub typed_lowering_evidence: Vec<TypedLoweringEvidence>,
    pub effects: EffectAnalysisReport,
    pub for_iteration_evidence: Vec<ForIterationEvidence>,
    pub trait_catalog: TraitCatalog,
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
    let effects = std::mem::take(&mut checker.effect_collector).finish();
    checker
        .errors
        .extend(effects.errors().cloned().map(TypeCheckError::effect));
    checker
        .warnings
        .extend(effects.warnings().cloned().map(TypeCheckWarning::effect));
    TypeCheckReport {
        diagnostics: checker.errors,
        warnings: checker.warnings,
        stats: checker.stats,
        judgments: checker.judgments,
        typed_lowering_evidence: checker.typed_lowering_evidence,
        effects,
        for_iteration_evidence: checker.for_iteration_evidence,
        trait_catalog: checker.trait_catalog,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForIterationEvidence {
    pub family: ForIterationEvidenceFamily,
    pub item_ty: TypeKind,
    pub into_iter_ty: TypeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ForIterationEvidenceFamily {
    Builtin(StandardIteratorFamily),
    Witness {
        into_iterator: crate::traits::TraitWitnessId,
        iterator: crate::traits::TraitWitnessId,
    },
    IteratorWitness {
        iterator: crate::traits::TraitWitnessId,
    },
    WitnessUnsupported {
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardIteratorFamily {
    Range,
    Seq,
    Stream,
    Vec,
    Array,
    Slice,
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
    active_borrow_lifetimes: BTreeMap<String, usize>,
    active_borrow_total: usize,
    borrow_local_lifetimes: HashMap<String, BorrowLocalState>,
    borrow_state_journal: Vec<BorrowStateJournalEntry>,
    global_symbols: HashMap<String, TypeKind>,
    global_functions: HashMap<String, TypeKind>,
    global_function_signatures: HashMap<String, FunctionSignature>,
    global_function_effects: HashMap<String, Vec<String>>,
    global_type_aliases: HashMap<String, TypeKind>,
    action_signatures: HashMap<String, ActionSignature>,
    nominal_fields: HashMap<String, HashMap<String, TypeKind>>,
    trait_catalog: TraitCatalog,
    trait_predicate_stack: Vec<Vec<TraitPredicate>>,
    flow_params: HashMap<String, HashSet<String>>,
    locals: HashMap<String, TypeKind>,
    local_scope_stack: Vec<LocalBindingSnapshot>,
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
    effect_collector: EffectCollector,
    expected_returns: Vec<TypeKind>,
    partial_placeholder_stack: Vec<TypeKind>,
    yield_stack: Vec<YieldContext>,
    stats: TypeCheckStats,
    judgments: Vec<TypeJudgment>,
    typed_lowering_evidence: Vec<TypedLoweringEvidence>,
    for_iteration_evidence: Vec<ForIterationEvidence>,
    record_runtime_for_iteration_evidence: bool,
}

#[derive(Clone, Debug)]
struct TypeCheckerScopeSnapshot {
    borrow_checkpoint: BorrowStateCheckpoint,
    active_presentation_defaults: HashMap<String, String>,
    lifetime_guarantees: HashSet<LifetimeKey>,
    dropped_lifetime_keys: HashSet<LifetimeKey>,
    available_lifetimes: Vec<LifetimeScopeKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActionSignature {
    params: Vec<ActionParam>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActionParam {
    name: String,
    ty: TypeKind,
    has_default: bool,
}

impl ActionSignature {
    fn new(params: impl IntoIterator<Item = ActionParam>) -> Self {
        Self {
            params: params.into_iter().collect(),
        }
    }

    fn params(&self) -> &[ActionParam] {
        &self.params
    }

    fn param(&self, name: &str) -> Option<&ActionParam> {
        self.params.iter().find(|param| param.name() == name)
    }
}

impl ActionParam {
    fn new(name: impl Into<String>, ty: TypeKind, has_default: bool) -> Self {
        Self {
            name: name.into(),
            ty,
            has_default,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    const fn has_default(&self) -> bool {
        self.has_default
    }
}

#[derive(Clone, Debug, Default)]
struct LocalBindingSnapshot(Vec<(String, Option<TypeKind>)>);

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
    fn new(env: &TypeCheckEnv) -> TypeChecker<'_> {
        TypeChecker {
            env,
            errors: Vec::new(),
            warnings: Vec::new(),
            active_borrow_lifetimes: BTreeMap::new(),
            active_borrow_total: 0,
            borrow_local_lifetimes: HashMap::new(),
            borrow_state_journal: Vec::new(),
            global_symbols: HashMap::new(),
            global_functions: HashMap::new(),
            global_function_signatures: HashMap::new(),
            global_function_effects: HashMap::new(),
            global_type_aliases: HashMap::new(),
            action_signatures: HashMap::new(),
            nominal_fields: HashMap::new(),
            trait_catalog: TraitCatalog::default(),
            trait_predicate_stack: Vec::new(),
            flow_params: HashMap::new(),
            locals: HashMap::new(),
            local_scope_stack: Vec::new(),
            loop_stack: Vec::new(),
            line_label_stack: Vec::new(),
            line_cancel_depth: 0,
            line_out_depth: 0,
            active_presentation_defaults: HashMap::new(),
            line_mark_stack: Vec::new(),
            lifetime_guarantees: HashSet::new(),
            dropped_lifetime_keys: HashSet::new(),
            available_lifetimes: Vec::new(),
            effect_capabilities: env
                .capabilities
                .iter()
                .map(|capability| capability.as_str().to_owned())
                .collect(),
            effect_collector: EffectCollector::new(available_effect_set(env)),
            expected_returns: Vec::new(),
            partial_placeholder_stack: Vec::new(),
            yield_stack: Vec::new(),
            stats: TypeCheckStats::default(),
            judgments: Vec::new(),
            typed_lowering_evidence: Vec::new(),
            for_iteration_evidence: Vec::new(),
            record_runtime_for_iteration_evidence: false,
        }
    }

    fn insert_scoped_locals(
        &mut self,
        bindings: impl IntoIterator<Item = (String, TypeKind)>,
    ) -> LocalBindingSnapshot {
        LocalBindingSnapshot(
            bindings
                .into_iter()
                .map(|(name, ty)| {
                    let previous = self.bind_local(name.clone(), ty);
                    (name, previous)
                })
                .collect(),
        )
    }

    fn bind_local(&mut self, name: String, ty: TypeKind) -> Option<TypeKind> {
        let previous = self.locals.insert(name.clone(), ty);
        if let Some(scope) = self.local_scope_stack.last_mut() {
            scope.0.push((name, previous.clone()));
        }
        previous
    }

    fn restore_scoped_locals(&mut self, snapshot: LocalBindingSnapshot) {
        for (name, previous) in snapshot.0.into_iter().rev() {
            if let Some(ty) = previous {
                self.locals.insert(name, ty);
            } else {
                self.locals.remove(&name);
            }
        }
    }

    fn with_local_mutation_scope<R>(&mut self, check: impl FnOnce(&mut Self) -> R) -> R {
        self.local_scope_stack.push(LocalBindingSnapshot::default());
        let result = check(self);
        let snapshot = self
            .local_scope_stack
            .pop()
            .expect("local mutation scope stack must stay balanced");
        self.restore_scoped_locals(snapshot);
        result
    }

    fn record_type_judgment(
        &mut self,
        subject: TypeJudgmentSubject,
        rule: TypeJudgmentRule,
        ty: TypeKind,
        expected: Option<&TypeKind>,
    ) -> TypeJudgmentId {
        let id = TypeJudgmentId(self.judgments.len());
        let stored_expected = expected.map(|expected| {
            if expected == &ty {
                TypeJudgmentExpected::SameAsJudgment
            } else {
                TypeJudgmentExpected::Other(expected.clone())
            }
        });
        self.judgments.push(TypeJudgment {
            id,
            subject,
            ty,
            rule,
            expected: stored_expected,
        });
        self.stats.judgments += 1;
        match rule {
            TypeJudgmentRule::Expr => self.stats.expr_judgments += 1,
            TypeJudgmentRule::Expected => self.stats.expected_judgments += 1,
            TypeJudgmentRule::LetBinding => self.stats.let_binding_judgments += 1,
            TypeJudgmentRule::Return => self.stats.return_judgments += 1,
        }
        id
    }

    fn record_typed_lowering_evidence(&mut self, evidence: TypedLoweringEvidence) {
        self.typed_lowering_evidence.push(evidence);
    }

    fn record_active_borrow_depth(&mut self) {
        self.stats.max_active_borrows = self.stats.max_active_borrows.max(self.active_borrow_total);
    }

    fn types_compatible(&mut self, expected: &TypeKind, actual: &TypeKind) -> bool {
        self.stats.type_compatibility_checks += 1;
        types_compatible(expected, actual)
    }

    fn collect_and_store_trait_catalog(&mut self, module: &HirModule) {
        let (catalog, diagnostics) = collect_trait_catalog(module);
        self.trait_catalog = catalog;
        self.errors.extend(diagnostics);
    }

    fn active_trait_predicates(&self) -> Vec<TraitPredicate> {
        self.trait_predicate_stack
            .iter()
            .flat_map(|scope| scope.iter().cloned())
            .collect()
    }

    fn resolve_type_projection(&mut self, ty: TypeKind) -> TypeKind {
        match ty {
            TypeKind::Projection { subject, assoc, .. } => {
                match self.trait_catalog.resolve_projection(
                    &subject,
                    &assoc,
                    &self.active_trait_predicates(),
                ) {
                    Ok(ProjectionResolution::Resolved(ty) | ProjectionResolution::Deferred(ty)) => {
                        ty
                    }
                    Err(ProjectionError::UnknownAssociatedType { subject, assoc }) => {
                        self.errors.push(TypeCheckError::trait_diagnostic(
                            TraitDiagnostic::unknown_associated_type(format!("{subject:?}"), assoc),
                        ));
                        TypeKind::Named("_".to_owned())
                    }
                    Err(ProjectionError::Ambiguous { subject, assoc }) => {
                        self.errors.push(TypeCheckError::trait_diagnostic(
                            TraitDiagnostic::ambiguous_projection(format!("{subject:?}"), assoc),
                        ));
                        TypeKind::Named("_".to_owned())
                    }
                }
            }
            TypeKind::Vec(inner) => TypeKind::Vec(Box::new(self.resolve_type_projection(*inner))),
            TypeKind::Seq(inner) => TypeKind::Seq(Box::new(self.resolve_type_projection(*inner))),
            TypeKind::Range(inner) => {
                TypeKind::Range(Box::new(self.resolve_type_projection(*inner)))
            }
            TypeKind::Slice(inner) => {
                TypeKind::Slice(Box::new(self.resolve_type_projection(*inner)))
            }
            TypeKind::Option(inner) => {
                TypeKind::Option(Box::new(self.resolve_type_projection(*inner)))
            }
            TypeKind::Result { ok, error } => TypeKind::Result {
                ok: Box::new(self.resolve_type_projection(*ok)),
                error: Box::new(self.resolve_type_projection(*error)),
            },
            other => other,
        }
    }

    fn clear_active_borrows(&mut self) {
        self.active_borrow_lifetimes.clear();
        self.active_borrow_total = 0;
    }

    fn active_borrow_labels(&self) -> Vec<&str> {
        self.active_borrow_lifetimes
            .keys()
            .map(String::as_str)
            .collect()
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

    fn function_signature(&self, name: &str) -> Option<&FunctionSignature> {
        self.global_function_signatures
            .get(name)
            .or_else(|| self.env.function_signature(name))
    }

    fn nominal_field_type(&self, receiver: &TypeKind, field: &str) -> Option<TypeKind> {
        match receiver {
            TypeKind::Named(name) if name == TypeKind::ACTION_EVENT_TYPE_NAME => {
                TypeKind::action_event_field(field)
            }
            TypeKind::Named(name) => self
                .nominal_fields
                .get(name)
                .and_then(|fields| fields.get(field))
                .cloned(),
            TypeKind::BorrowRef { inner, .. } | TypeKind::Shared(inner) => {
                self.nominal_field_type(inner, field)
            }
            _ => None,
        }
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

fn available_effect_set(env: &TypeCheckEnv) -> Option<EffectSet> {
    env.available_effects().map(|available| {
        available
            .iter()
            .filter_map(|capability| EffectId::parse(capability.as_str()).ok())
            .collect::<EffectSet>()
    })
}

fn function_signature_type(signature: &FnSignature) -> FunctionSignature {
    let return_type = signature
        .return_type()
        .map_or(TypeKind::Unit, type_ref_kind);
    let params = signature
        .param_groups()
        .iter()
        .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        .map(function_param_type)
        .collect::<Vec<_>>();
    FunctionSignature::new(return_type, params)
}

fn function_param_type(param: &FnParam) -> FunctionParam {
    FunctionParam {
        name: pattern_param_name(param.pattern()),
        ty: type_ref_kind(param.ty()),
        kind: param.kind(),
        has_default: param.default().is_some(),
    }
}

fn function_param_local_type(param: &FnParam) -> TypeKind {
    function_param_local_type_with_generics(param, &HashSet::new())
}

fn function_param_local_type_with_generics(
    param: &FnParam,
    generic_names: &HashSet<String>,
) -> TypeKind {
    let ty = type_ref_kind_with_generics(param.ty(), generic_names);
    if param.is_rest() {
        TypeKind::Vec(Box::new(ty))
    } else {
        ty
    }
}

fn signature_generic_names(signature: &FnSignature) -> HashSet<String> {
    signature
        .generic_params()
        .iter()
        .filter_map(|param| param.as_type())
        .map(ToOwned::to_owned)
        .collect()
}

fn type_ref_kind_with_generics(ty: &TypeRef, generic_names: &HashSet<String>) -> TypeKind {
    match ty {
        TypeRef::Path(path) if generic_names.contains(path) => TypeKind::GenericParam(path.clone()),
        TypeRef::Projection { subject, assoc } => TypeKind::Projection {
            subject: Box::new(type_ref_kind_with_generics(subject, generic_names)),
            trait_name: None,
            assoc: assoc.clone(),
        },
        TypeRef::Generic { base, args } if base == "Vec" && args.len() == 1 => TypeKind::Vec(
            Box::new(type_ref_kind_with_generics(&args[0], generic_names)),
        ),
        TypeRef::Generic { base, args } if base == "Option" && args.len() == 1 => TypeKind::Option(
            Box::new(type_ref_kind_with_generics(&args[0], generic_names)),
        ),
        TypeRef::Generic { base, args } if base == "Result" && args.len() == 2 => {
            TypeKind::Result {
                ok: Box::new(type_ref_kind_with_generics(&args[0], generic_names)),
                error: Box::new(type_ref_kind_with_generics(&args[1], generic_names)),
            }
        }
        TypeRef::Generic { base, args } if base == "Need" && args.len() == 2 => TypeKind::Need {
            ready: Box::new(type_ref_kind_with_generics(&args[0], generic_names)),
            error: Box::new(type_ref_kind_with_generics(&args[1], generic_names)),
        },
        TypeRef::Ref { lifetime, inner } => TypeKind::BorrowRef {
            lifetime: lifetime
                .as_ref()
                .map(|lifetime| LifetimeScopeKind::parse(lifetime.name())),
            inner: Box::new(type_ref_kind_with_generics(inner, generic_names)),
        },
        TypeRef::Slice(inner) => {
            TypeKind::Slice(Box::new(type_ref_kind_with_generics(inner, generic_names)))
        }
        TypeRef::Choice(alternatives) => normalize_choice_type(
            alternatives
                .iter()
                .map(|alternative| type_ref_kind_with_generics(alternative, generic_names))
                .collect::<Vec<_>>(),
        ),
        _ => type_ref_kind(ty),
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
    if matches!(actual, TypeKind::Never) {
        return true;
    }
    match (expected, actual) {
        (TypeKind::Bytes, TypeKind::Vec(inner) | TypeKind::Slice(inner) | TypeKind::Seq(inner)) => {
            matches!(inner.as_ref(), TypeKind::U8)
        }
        (TypeKind::ActionName, TypeKind::String | TypeKind::Named(_)) => true,
        (TypeKind::AgentValue, actual) => is_agent_value_type(actual),
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
        (TypeKind::Vec(expected), TypeKind::Vec(actual))
        | (TypeKind::Seq(expected), TypeKind::Seq(actual))
        | (TypeKind::Slice(expected), TypeKind::Slice(actual)) => {
            types_compatible(expected, actual)
        }
        (
            TypeKind::Array {
                item: expected_item,
                len: expected_len,
            },
            TypeKind::Array {
                item: actual_item,
                len: actual_len,
            },
        ) => expected_len == actual_len && types_compatible(expected_item, actual_item),
        (TypeKind::Range(expected), TypeKind::Range(actual)) => types_compatible(expected, actual),
        (
            TypeKind::Function {
                params: expected_params,
                return_type: expected_return,
            },
            TypeKind::Function {
                params: actual_params,
                return_type: actual_return,
            },
        ) => {
            expected_params.len() == actual_params.len()
                && expected_params
                    .iter()
                    .zip(actual_params.iter())
                    .all(|(expected, actual)| types_compatible(expected, actual))
                && types_compatible(expected_return, actual_return)
        }
        _ => false,
    }
}

fn is_agent_value_type(ty: &TypeKind) -> bool {
    match ty {
        TypeKind::Bool
        | TypeKind::I8
        | TypeKind::I16
        | TypeKind::I32
        | TypeKind::I64
        | TypeKind::I128
        | TypeKind::ISize
        | TypeKind::U8
        | TypeKind::U16
        | TypeKind::U32
        | TypeKind::U64
        | TypeKind::U128
        | TypeKind::USize
        | TypeKind::F32
        | TypeKind::F64
        | TypeKind::String
        | TypeKind::Char
        | TypeKind::Bytes
        | TypeKind::Duration
        | TypeKind::DisplayText
        | TypeKind::ActionName
        | TypeKind::AgentValue
        | TypeKind::ObservedObject
        | TypeKind::AgentBBox
        | TypeKind::Ref(_)
        | TypeKind::CaptureRef
        | TypeKind::AgentResource
        | TypeKind::AgentResourceBody => true,
        TypeKind::Vec(inner)
        | TypeKind::Array { item: inner, .. }
        | TypeKind::Slice(inner)
        | TypeKind::Range(inner)
        | TypeKind::Option(inner) => is_agent_value_type(inner),
        TypeKind::Map { key, value, .. } => {
            types_compatible(&TypeKind::String, key) && is_agent_value_type(value)
        }
        TypeKind::Choice(alternatives) => alternatives.iter().all(is_agent_value_type),
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
