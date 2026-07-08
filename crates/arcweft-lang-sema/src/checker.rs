use crate::borrow::{
    BorrowLocalState, BorrowStateCheckpoint, BorrowStateDelta, BorrowStateDeltaEntry,
    BorrowStateJournalEntry, merge_borrow_local_states,
};
use crate::diagnostics::{
    TraitDiagnostic, TypeCheckError, TypeCheckReadinessError, TypeCheckWarning,
};
use crate::effect_analysis::EffectAnalysisReport;
use crate::effect_collector::EffectCollector;
use crate::effect_model::{CallableId, CallableKind, EffectContract, EffectSite, Visibility};
use crate::effects::{EffectId, EffectSet};
use crate::env::{
    AgentActionEnvParam, DebugPathKind, EffectCapability, EnumVariantPayload, FunctionParam,
    FunctionParamHigherOrderBinding, FunctionParamSelector, FunctionParamSelectorSegment,
    FunctionSignature, TypeCheckEnv,
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
        common::TextRange,
        dialogue::DialogueToken,
        flow::{AwaitBranchKind, ContractClause, SelectBranchHead, Stmt},
        ids::{EntityRef, EntityRefSyntax, IdRef},
        items::{EntityDeclKind, FunctionKind},
        line_plan::{CancelRuleSyntax, LinePlanItem, TriggerPattern},
        pattern::{Pattern, RecordPatternField, VariantPatternPayload},
        source::{
            SourceBackpressurePolicy, SourceEventPattern, SourceHeader, SourcePrivacyPolicy,
            SourceReplayPolicy,
        },
    },
    expr::{CallArg, Expr, LifetimeAccessMode, LifetimeKey, LifetimeScopeKind, Literal},
    types::{FnParam, FnSignature, TypeRef},
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

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
pub mod source_ranges;
pub mod stmt;
pub mod suspension;

use helpers::{
    await_branch_pattern_type, choice_output_type, default_presentation_slot_family, entity_kind,
    entity_kind_for_decl, entity_syntax_kind, expr_path_label, ident_pattern_name,
    is_character_entity_literal, is_dialogue_callee_type, is_drop_callee, is_local_ident,
    iter_item_type, merge_line_output, normalize_choice_type, pattern_bindings_with_fallback,
    pattern_bindings_with_nominal_types, source_return_types, stmts_diverge, stream_return_types,
    type_ref_kind, typed_pattern_binding, unify_loop_break_types, variant_payload_type_for_name,
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
    pub source_backed_expr_judgments: usize,
    pub source_missing_expr_judgments: usize,
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ExprNodeKey(usize);

impl ExprNodeKey {
    fn from_expr(expr: &Expr) -> Self {
        Self(std::ptr::from_ref::<Expr>(expr) as usize)
    }
}

fn closure_effect_callable_id(expression_id: TypeExpressionId) -> CallableId {
    CallableId::new(format!("closure.expr.{}", expression_id.index()))
}

fn function_return_effect_callable_id(function_name: &str) -> CallableId {
    CallableId::new(format!("fn.{function_name}.return"))
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
    pub source_range: Option<TextRange>,
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
    /// A direct named function signature call returned a partial function.
    SignaturePartialCall {
        callee: String,
        result_ty: TypeKind,
        arg_count: usize,
    },
    /// A method-call expression resolved through data-last callable fallback.
    DataLastMethodFallback {
        method: String,
        arg_count: usize,
        arg_order: Vec<DataLastMethodFallbackArg>,
    },
}

/// One runtime argument selected for a data-last method fallback call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataLastMethodFallbackArg {
    /// Source method-call argument at the given index.
    CallArg { index: usize },
    /// The method-call receiver appended as the callable's data-last argument.
    Receiver,
}

/// One local binding captured by a closure expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureCapture {
    pub name: String,
    pub ty: TypeKind,
}

/// Machine-readable capture inventory for one closure expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureCaptureInventory {
    pub expression_id: TypeExpressionId,
    pub captures: Vec<ClosureCapture>,
}

/// Machine-readable type-check result used by tooling and profiling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeCheckReport {
    pub diagnostics: Vec<TypeCheckError>,
    pub warnings: Vec<TypeCheckWarning>,
    pub stats: TypeCheckStats,
    pub judgments: Vec<TypeJudgment>,
    pub typed_lowering_evidence: Vec<TypedLoweringEvidence>,
    pub closure_captures: Vec<ClosureCaptureInventory>,
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
    checker.apply_pending_higher_order_effect_calls();
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
        closure_captures: checker.closure_captures,
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
    nominal_variant_payloads: HashMap<String, HashMap<String, EnumVariantPayload>>,
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
    expected_returns: Vec<Option<TypeKind>>,
    partial_placeholder_stack: Vec<TypeKind>,
    allow_inferred_signature_partial_calls: bool,
    yield_stack: Vec<YieldContext>,
    stats: TypeCheckStats,
    judgments: Vec<TypeJudgment>,
    typed_lowering_evidence: Vec<TypedLoweringEvidence>,
    expression_source_ranges: HashMap<ExprNodeKey, TextRange>,
    closure_capture_stack: Vec<ClosureCaptureFrame>,
    closure_inference_stack: Vec<ClosureInferenceContext>,
    closure_captures: Vec<ClosureCaptureInventory>,
    local_function_effects: HashMap<String, CallableId>,
    closure_effect_callables_by_expr: HashMap<ExprNodeKey, CallableId>,
    last_checked_closure_effect_callable: Option<CallableId>,
    function_return_effect_callables: HashMap<String, CallableId>,
    local_curried_signature_calls: HashMap<String, CurriedSignatureCallValue>,
    last_checked_curried_signature_call: Option<CurriedSignatureCallValue>,
    local_higher_order_param_aliases: HashMap<String, String>,
    higher_order_param_scope_stack: Vec<HigherOrderParamScope>,
    higher_order_param_invocations: BTreeMap<String, BTreeSet<String>>,
    higher_order_param_closure_invocations:
        BTreeMap<String, BTreeMap<String, BTreeSet<CallableId>>>,
    pending_higher_order_effect_calls: Vec<PendingHigherOrderEffectCall>,
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

#[derive(Clone, Copy)]
struct NominalTypeContext<'a> {
    fields: Option<&'a HashMap<String, HashMap<String, TypeKind>>>,
    variant_payloads: Option<&'a HashMap<String, HashMap<String, EnumVariantPayload>>>,
    env: Option<&'a TypeCheckEnv>,
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

impl<'a> NominalTypeContext<'a> {
    const fn empty() -> Self {
        Self {
            fields: None,
            variant_payloads: None,
            env: None,
        }
    }

    const fn new(
        fields: &'a HashMap<String, HashMap<String, TypeKind>>,
        variant_payloads: &'a HashMap<String, HashMap<String, EnumVariantPayload>>,
        env: &'a TypeCheckEnv,
    ) -> Self {
        Self {
            fields: Some(fields),
            variant_payloads: Some(variant_payloads),
            env: Some(env),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct LocalBindingSnapshot(Vec<LocalBindingSnapshotEntry>);

#[derive(Clone, Debug)]
struct LocalBindingSnapshotEntry {
    name: String,
    previous_ty: Option<TypeKind>,
    previous_function_effect: Option<CallableId>,
    previous_curried_signature_call: Option<CurriedSignatureCallValue>,
    previous_higher_order_param_alias: Option<String>,
}

#[derive(Clone, Debug)]
struct ClosureCaptureFrame {
    expression_id: TypeExpressionId,
    locals: HashSet<String>,
    captures: BTreeMap<String, TypeKind>,
    suspension_boundaries: BTreeSet<SuspensionBoundary>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClosureInferenceContext {
    inferred_return_type: bool,
}

#[derive(Clone, Debug)]
struct HigherOrderParamScope {
    function_name: String,
    callable: CallableId,
    param_names: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct PendingHigherOrderEffectCall {
    caller: CallableId,
    callee_function: String,
    param_name: String,
    effect_callable: CallableId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CurriedSignatureCallValue {
    function_name: String,
    remaining_group_index: usize,
    group_arg_offset: usize,
    pending_higher_order_args: Vec<PendingCurriedHigherOrderArg>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingCurriedHigherOrderArg {
    param_name: String,
    effect_callable: CallableId,
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SuspensionBoundary {
    Await,
    Defer,
    DeferCleanup,
    Thread,
    ThreadSuspension,
    Yield,
}

impl SuspensionBoundary {
    const fn label(self) -> &'static str {
        match self {
            Self::Await => "await suspension boundary",
            Self::Defer => "suspension boundary",
            Self::DeferCleanup => "defer cleanup boundary",
            Self::Thread => "thread boundary",
            Self::ThreadSuspension => "thread suspension boundary",
            Self::Yield => "yield suspension boundary",
        }
    }
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
            nominal_variant_payloads: HashMap::new(),
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
            allow_inferred_signature_partial_calls: true,
            yield_stack: Vec::new(),
            stats: TypeCheckStats::default(),
            judgments: Vec::new(),
            typed_lowering_evidence: Vec::new(),
            expression_source_ranges: HashMap::new(),
            closure_capture_stack: Vec::new(),
            closure_inference_stack: Vec::new(),
            closure_captures: Vec::new(),
            local_function_effects: HashMap::new(),
            closure_effect_callables_by_expr: HashMap::new(),
            last_checked_closure_effect_callable: None,
            function_return_effect_callables: HashMap::new(),
            local_curried_signature_calls: HashMap::new(),
            last_checked_curried_signature_call: None,
            local_higher_order_param_aliases: HashMap::new(),
            higher_order_param_scope_stack: Vec::new(),
            higher_order_param_invocations: BTreeMap::new(),
            higher_order_param_closure_invocations: BTreeMap::new(),
            pending_higher_order_effect_calls: Vec::new(),
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
                    let previous_function_effect = self.local_function_effects.get(&name).cloned();
                    let previous_curried_signature_call =
                        self.local_curried_signature_calls.get(&name).cloned();
                    let previous_higher_order_param_alias =
                        self.local_higher_order_param_aliases.get(&name).cloned();
                    let previous_ty = self.bind_local(name.clone(), ty);
                    LocalBindingSnapshotEntry {
                        name,
                        previous_ty,
                        previous_function_effect,
                        previous_curried_signature_call,
                        previous_higher_order_param_alias,
                    }
                })
                .collect(),
        )
    }

    fn bind_local(&mut self, name: String, ty: TypeKind) -> Option<TypeKind> {
        let previous = self.locals.insert(name.clone(), ty);
        let previous_function_effect = self.local_function_effects.remove(&name);
        let previous_curried_signature_call = self.local_curried_signature_calls.remove(&name);
        let previous_higher_order_param_alias = self.local_higher_order_param_aliases.remove(&name);
        if let Some(frame) = self.closure_capture_stack.last_mut() {
            frame.locals.insert(name.clone());
        }
        if let Some(scope) = self.local_scope_stack.last_mut() {
            scope.0.push(LocalBindingSnapshotEntry {
                name,
                previous_ty: previous.clone(),
                previous_function_effect,
                previous_curried_signature_call,
                previous_higher_order_param_alias,
            });
        }
        previous
    }

    fn bind_local_function_effect(&mut self, name: &str, callable: CallableId) {
        self.local_function_effects
            .insert(name.to_owned(), callable);
    }

    fn bind_local_curried_signature_call(&mut self, name: &str, value: CurriedSignatureCallValue) {
        self.local_curried_signature_calls
            .insert(name.to_owned(), value);
    }

    fn bind_local_higher_order_param_alias(&mut self, name: &str, param_name: &str) {
        self.local_higher_order_param_aliases
            .insert(name.to_owned(), param_name.to_owned());
    }

    fn restore_scoped_locals(&mut self, snapshot: LocalBindingSnapshot) {
        for entry in snapshot.0.into_iter().rev() {
            if let Some(ty) = entry.previous_ty {
                self.locals.insert(entry.name.clone(), ty);
            } else {
                self.locals.remove(&entry.name);
            }
            if let Some(callable) = entry.previous_function_effect {
                self.local_function_effects
                    .insert(entry.name.clone(), callable);
            } else {
                self.local_function_effects.remove(&entry.name);
            }
            if let Some(value) = entry.previous_curried_signature_call {
                self.local_curried_signature_calls
                    .insert(entry.name.clone(), value);
            } else {
                self.local_curried_signature_calls.remove(&entry.name);
            }
            if let Some(param_name) = entry.previous_higher_order_param_alias {
                self.local_higher_order_param_aliases
                    .insert(entry.name, param_name);
            } else {
                self.local_higher_order_param_aliases.remove(&entry.name);
            }
        }
    }

    fn with_inferred_signature_partial_calls<R>(
        &mut self,
        allowed: bool,
        check: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = self.allow_inferred_signature_partial_calls;
        self.allow_inferred_signature_partial_calls = allowed;
        let result = check(self);
        self.allow_inferred_signature_partial_calls = previous;
        result
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

    fn record_type_judgment_with_source_range(
        &mut self,
        subject: TypeJudgmentSubject,
        rule: TypeJudgmentRule,
        ty: TypeKind,
        expected: Option<&TypeKind>,
        source_range: Option<TextRange>,
    ) -> TypeJudgmentId {
        let id = TypeJudgmentId(self.judgments.len());
        let stored_expected = expected.map(|expected| {
            if expected == &ty {
                TypeJudgmentExpected::SameAsJudgment
            } else {
                TypeJudgmentExpected::Other(expected.clone())
            }
        });
        let is_expr_subject = matches!(subject, TypeJudgmentSubject::Expr { .. });
        self.judgments.push(TypeJudgment {
            id,
            subject,
            ty,
            rule,
            expected: stored_expected,
            source_range,
        });
        self.stats.judgments += 1;
        if is_expr_subject {
            if source_range.is_some() {
                self.stats.source_backed_expr_judgments += 1;
            } else {
                self.stats.source_missing_expr_judgments += 1;
            }
        }
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

    fn record_function_expr_effect_callable(&mut self, expr: &Expr, ty: &TypeKind) {
        if !matches!(ty, TypeKind::Function { .. }) {
            return;
        }
        let Some(callable) = self
            .last_checked_closure_effect_callable
            .clone()
            .or_else(|| self.closure_effect_callable_for_function_expr(expr, ty))
        else {
            return;
        };
        self.closure_effect_callables_by_expr
            .insert(ExprNodeKey::from_expr(expr), callable.clone());
        self.last_checked_closure_effect_callable = Some(callable);
    }

    fn push_closure_capture_frame(
        &mut self,
        expression_id: TypeExpressionId,
        locals: impl IntoIterator<Item = String>,
    ) {
        self.closure_capture_stack.push(ClosureCaptureFrame {
            expression_id,
            locals: locals.into_iter().collect(),
            captures: BTreeMap::new(),
            suspension_boundaries: BTreeSet::new(),
        });
    }

    fn enter_closure_effect_callable(
        &mut self,
        expression_id: TypeExpressionId,
    ) -> (CallableId, Option<CallableId>) {
        let id = closure_effect_callable_id(expression_id);
        let source_name = id.as_str().to_owned();
        if let Err(error) = self.effect_collector.register_callable(
            source_name,
            id.clone(),
            CallableKind::Function,
            Visibility::Private,
            EffectContract::inferred(),
        ) {
            self.errors.push(TypeCheckError::new(error.to_string()));
        }
        let previous = self.effect_collector.enter(id.clone());
        (id, previous)
    }

    fn restore_effect_callable(&mut self, previous: Option<CallableId>) {
        self.effect_collector.restore(previous);
    }

    fn pop_closure_capture_frame(&mut self) {
        let frame = self
            .closure_capture_stack
            .pop()
            .expect("closure capture frame stack must stay balanced");
        let ClosureCaptureFrame {
            expression_id,
            captures,
            suspension_boundaries,
            ..
        } = frame;
        let captures = captures
            .into_iter()
            .map(|(name, ty)| ClosureCapture { name, ty })
            .collect::<Vec<_>>();
        self.reject_borrowed_closure_captures(&captures, &suspension_boundaries);
        self.closure_captures.push(ClosureCaptureInventory {
            expression_id,
            captures,
        });
    }

    fn record_closure_suspension_boundary(&mut self, boundary: SuspensionBoundary) {
        if let Some(frame) = self.closure_capture_stack.last_mut() {
            frame.suspension_boundaries.insert(boundary);
        }
    }

    fn push_closure_inference_context(&mut self, inferred_return_type: bool) {
        self.closure_inference_stack.push(ClosureInferenceContext {
            inferred_return_type,
        });
    }

    fn pop_closure_inference_context(&mut self) {
        self.closure_inference_stack
            .pop()
            .expect("closure inference context stack must stay balanced");
    }

    fn record_numeric_fallback_in_inferred_closure(
        &mut self,
        literal_kind: &'static str,
        fallback: TypeKind,
    ) {
        if self
            .closure_inference_stack
            .last()
            .is_some_and(|context| context.inferred_return_type)
        {
            self.warnings
                .push(TypeCheckWarning::numeric_fallback_in_inferred_closure(
                    literal_kind,
                    fallback,
                ));
        }
    }

    fn reject_borrowed_closure_captures(
        &mut self,
        captures: &[ClosureCapture],
        boundaries: &BTreeSet<SuspensionBoundary>,
    ) {
        if boundaries.is_empty() {
            return;
        }
        for capture in captures {
            if !type_contains_borrow_ref(&capture.ty) {
                continue;
            }
            let mut lifetimes = Vec::new();
            collect_type_kind_lifetimes(&capture.ty, &mut lifetimes);
            lifetimes.sort();
            lifetimes.dedup();
            for boundary in boundaries {
                self.errors
                    .push(TypeCheckError::borrowed_closure_capture_crosses_boundary(
                        capture.name.clone(),
                        capture.ty.clone(),
                        lifetimes.clone(),
                        boundary.label(),
                    ));
            }
        }
    }

    fn record_closure_capture(&mut self, name: &str, ty: &TypeKind) {
        for frame in self.closure_capture_stack.iter_mut().rev() {
            if frame.locals.contains(name) {
                break;
            }
            frame
                .captures
                .entry(name.to_owned())
                .or_insert_with(|| ty.clone());
        }
    }

    fn record_function_value_effect_call(
        &mut self,
        callee: Option<&str>,
        effect_callable: Option<CallableId>,
        arg_count: usize,
        arity: usize,
    ) {
        let callable = effect_callable
            .or_else(|| callee.and_then(|callee| self.local_function_effects.get(callee).cloned()));
        if arg_count < arity {
            if let Some(callable) = callable {
                self.last_checked_closure_effect_callable = Some(callable);
            }
            return;
        }
        self.record_higher_order_param_invocation(callee);
        let Some(callable) = callable else {
            return;
        };
        let site = callee.map_or_else(
            || "function value call on closure expression".to_owned(),
            |callee| format!("function value call `{callee}`"),
        );
        self.effect_collector
            .record_local_call(callable, EffectSite::new(site));
    }

    fn record_higher_order_function_argument_effect_call(
        &mut self,
        expr: &Expr,
        ty: &TypeKind,
        site: impl Into<String>,
    ) {
        if let Some(callable) = self.closure_effect_callable_for_function_expr(expr, ty) {
            self.effect_collector
                .record_local_call(callable, EffectSite::new(site));
        }
        self.last_checked_closure_effect_callable = None;
    }

    fn record_pending_higher_order_signature_arg_effect_call(
        &mut self,
        function_name: &str,
        param: &FunctionParam,
        value: &Expr,
        actual: Option<&TypeKind>,
    ) {
        let args = self.higher_order_signature_arg_effect_calls(param, value, actual);
        let Some(caller) = self.effect_collector.current_callable() else {
            return;
        };
        self.pending_higher_order_effect_calls
            .extend(args.into_iter().map(|arg| PendingHigherOrderEffectCall {
                caller: caller.clone(),
                callee_function: function_name.to_owned(),
                param_name: arg.param_name,
                effect_callable: arg.effect_callable,
            }));
    }

    fn higher_order_signature_arg_effect_calls(
        &mut self,
        param: &FunctionParam,
        value: &Expr,
        actual: Option<&TypeKind>,
    ) -> Vec<PendingCurriedHigherOrderArg> {
        let Some(actual) = actual else {
            self.last_checked_closure_effect_callable = None;
            return Vec::new();
        };
        let args = param
            .higher_order_bindings()
            .iter()
            .filter_map(|binding| {
                let (value, actual) = selected_higher_order_argument(
                    binding.selector(),
                    value,
                    actual,
                    binding.ty(),
                )?;
                if !matches!(binding.ty(), TypeKind::Function { .. })
                    || !matches!(actual, TypeKind::Function { .. })
                {
                    return None;
                }
                self.closure_effect_callable_for_function_expr(value, actual)
                    .map(|effect_callable| PendingCurriedHigherOrderArg {
                        param_name: binding.name().to_owned(),
                        effect_callable,
                    })
            })
            .collect();
        self.last_checked_closure_effect_callable = None;
        args
    }

    fn record_curried_signature_result(
        &mut self,
        function_name: &str,
        next_group_index: usize,
        result_ty: &TypeKind,
        has_next_group_metadata: bool,
        exact_group_call: bool,
    ) {
        self.last_checked_curried_signature_call = (has_next_group_metadata
            && exact_group_call
            && matches!(result_ty, TypeKind::Function { .. }))
        .then(|| CurriedSignatureCallValue {
            function_name: function_name.to_owned(),
            remaining_group_index: next_group_index,
            group_arg_offset: 0,
            pending_higher_order_args: Vec::new(),
        });
    }

    fn register_function_return_effect_callable(&mut self, function_name: &str, ty: &TypeKind) {
        if !matches!(ty, TypeKind::Function { .. }) {
            return;
        }
        if self
            .function_return_effect_callables
            .contains_key(function_name)
        {
            return;
        }
        let id = function_return_effect_callable_id(function_name);
        if let Err(error) = self.effect_collector.register_callable(
            id.as_str().to_owned(),
            id.clone(),
            CallableKind::Function,
            Visibility::Private,
            EffectContract::inferred(),
        ) {
            self.errors.push(TypeCheckError::new(error.to_string()));
        }
        self.function_return_effect_callables
            .insert(function_name.to_owned(), id);
    }

    fn connect_function_return_effect_callable(
        &mut self,
        function_name: &str,
        ty: Option<&TypeKind>,
    ) {
        let Some(TypeKind::Function { .. }) = ty else {
            self.last_checked_closure_effect_callable = None;
            return;
        };
        let Some(return_callable) = self
            .function_return_effect_callables
            .get(function_name)
            .cloned()
        else {
            return;
        };
        let Some(body_callable) = self.last_checked_closure_effect_callable.clone() else {
            return;
        };
        if return_callable != body_callable {
            self.effect_collector.record_local_call_from(
                &return_callable,
                body_callable,
                EffectSite::new(format!("returned function value from `{function_name}`")),
            );
        }
    }

    fn record_function_return_effect_result(&mut self, function_name: &str, ty: &TypeKind) {
        if !matches!(ty, TypeKind::Function { .. }) {
            self.last_checked_closure_effect_callable = None;
            return;
        }
        if let Some(callable) = self
            .function_return_effect_callables
            .get(function_name)
            .cloned()
        {
            self.last_checked_closure_effect_callable = Some(callable);
        }
    }

    fn record_pending_curried_higher_order_arg_effect_calls(
        &mut self,
        function_name: &str,
        args: &[PendingCurriedHigherOrderArg],
    ) {
        let Some(caller) = self.effect_collector.current_callable() else {
            return;
        };
        self.pending_higher_order_effect_calls
            .extend(
                args.iter()
                    .cloned()
                    .map(|arg| PendingHigherOrderEffectCall {
                        caller: caller.clone(),
                        callee_function: function_name.to_owned(),
                        param_name: arg.param_name,
                        effect_callable: arg.effect_callable,
                    }),
            );
    }

    fn record_higher_order_param_invocation(&mut self, callee: Option<&str>) {
        let Some(callee) = callee else {
            return;
        };
        let Some(scope) = self.higher_order_param_scope_stack.last() else {
            return;
        };
        let param_name = self
            .local_higher_order_param_aliases
            .get(callee)
            .map_or(callee, String::as_str);
        if !scope.param_names.contains(param_name) {
            return;
        }
        let Some(current_callable) = self.effect_collector.current_callable() else {
            return;
        };
        if current_callable == scope.callable {
            self.higher_order_param_invocations
                .entry(scope.function_name.clone())
                .or_default()
                .insert(param_name.to_owned());
        } else {
            self.higher_order_param_closure_invocations
                .entry(scope.function_name.clone())
                .or_default()
                .entry(param_name.to_owned())
                .or_default()
                .insert(current_callable);
        }
    }

    fn apply_pending_higher_order_effect_calls(&mut self) {
        let pending = std::mem::take(&mut self.pending_higher_order_effect_calls);
        for call in pending {
            if let Some(invoked_params) = self
                .higher_order_param_invocations
                .get(&call.callee_function)
                && invoked_params.contains(&call.param_name)
            {
                self.effect_collector.record_local_call_from(
                    &call.caller,
                    call.effect_callable.clone(),
                    EffectSite::new(format!(
                        "higher-order argument `{}` for `{}`",
                        call.param_name, call.callee_function
                    )),
                );
            }
            if let Some(closure_invocations) = self
                .higher_order_param_closure_invocations
                .get(&call.callee_function)
                .and_then(|params| params.get(&call.param_name))
            {
                for closure_callable in closure_invocations {
                    self.effect_collector.record_local_call_from(
                        closure_callable,
                        call.effect_callable.clone(),
                        EffectSite::new(format!(
                            "higher-order argument `{}` captured by returned closure from `{}`",
                            call.param_name, call.callee_function
                        )),
                    );
                }
            }
        }
    }

    fn closure_effect_callable_for_function_expr(
        &self,
        expr: &Expr,
        ty: &TypeKind,
    ) -> Option<CallableId> {
        if !matches!(ty, TypeKind::Function { .. }) {
            return None;
        }
        if let Some(callable) = self
            .closure_effect_callables_by_expr
            .get(&ExprNodeKey::from_expr(expr))
            .cloned()
        {
            return Some(callable);
        }
        match expr {
            Expr::Closure { .. } => self.last_checked_closure_effect_callable.clone(),
            Expr::Path(path) => self.local_function_effects.get(path.as_label()).cloned(),
            Expr::Call { callee, args } => {
                if let Some(callable) = self.last_checked_closure_effect_callable.clone() {
                    return Some(callable);
                }
                let callee = expr_path_label(callee)?;
                if let Some(callable) = self.function_return_effect_callables.get(&callee).cloned()
                {
                    return Some(callable);
                }
                let callable = self.local_function_effects.get(&callee)?.clone();
                let arity = self.locals.get(&callee)?.function_arity()?;
                let positional_arg_count = args
                    .iter()
                    .filter(|arg| matches!(arg, CallArg::Positional(_)))
                    .count();
                let all_positional = args.iter().all(|arg| matches!(arg, CallArg::Positional(_)));
                (all_positional && positional_arg_count < arity).then_some(callable)
            }
            _ => None,
        }
    }

    fn curried_signature_call_for_function_expr(
        &self,
        expr: &Expr,
        ty: &TypeKind,
    ) -> Option<CurriedSignatureCallValue> {
        if !matches!(ty, TypeKind::Function { .. }) {
            return None;
        }
        match expr {
            Expr::Call { .. } => self.last_checked_curried_signature_call.clone(),
            Expr::Path(path) => self
                .local_curried_signature_calls
                .get(path.as_label())
                .cloned(),
            _ => None,
        }
    }

    fn higher_order_param_alias_for_function_expr(
        &self,
        expr: &Expr,
        ty: &TypeKind,
    ) -> Option<String> {
        if !matches!(ty, TypeKind::Function { .. }) {
            return None;
        }
        let Expr::Path(path) = expr else {
            return None;
        };
        let name = path.as_label();
        let scope = self.higher_order_param_scope_stack.last()?;
        if scope.param_names.contains(name) {
            return Some(name.to_owned());
        }
        self.local_higher_order_param_aliases.get(name).cloned()
    }

    fn local_symbol_type_with_capture(&mut self, name: &str) -> Option<TypeKind> {
        let ty = self.locals.get(name).cloned()?;
        self.record_closure_capture(name, &ty);
        Some(ty)
    }

    fn symbol_type_with_capture(&mut self, name: &str) -> Option<TypeKind> {
        self.local_symbol_type_with_capture(name)
            .or_else(|| self.global_symbols.get(name).cloned())
            .or_else(|| self.env.symbol_type(name).cloned())
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

    fn function_value_type(&self, name: &str) -> Option<TypeKind> {
        self.function_signature(name)
            .and_then(FunctionSignature::function_value_type)
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
    function_signature_type_with_nominal_types(signature, NominalTypeContext::empty())
}

fn function_signature_type_with_nominal_types(
    signature: &FnSignature,
    nominal_types: NominalTypeContext<'_>,
) -> FunctionSignature {
    let return_type = curried_signature_return_type(signature);
    let params = signature
        .param_groups()
        .first()
        .into_iter()
        .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        .map(|param| function_param_type(param, nominal_types))
        .collect::<Vec<_>>();
    let remaining_param_groups = signature
        .param_groups()
        .iter()
        .skip(1)
        .map(|group| {
            group
                .params()
                .iter()
                .map(|param| function_param_type(param, nominal_types))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    FunctionSignature::new(return_type, params).with_remaining_param_groups(remaining_param_groups)
}

fn curried_signature_return_type(signature: &FnSignature) -> TypeKind {
    let return_type = signature
        .return_type()
        .map_or(TypeKind::Unit, type_ref_kind);
    signature
        .param_groups()
        .iter()
        .skip(1)
        .rev()
        .fold(return_type, |return_type, group| TypeKind::Function {
            params: group
                .params()
                .iter()
                .map(|param| type_ref_kind(param.ty()))
                .collect(),
            return_type: Box::new(return_type),
        })
}

fn function_param_type(param: &FnParam, nominal_types: NominalTypeContext<'_>) -> FunctionParam {
    let ty = type_ref_kind(param.ty());
    FunctionParam::new(
        pattern_param_name(param.pattern()),
        ty.clone(),
        param.kind(),
        param.default().is_some(),
        function_param_higher_order_bindings(param.pattern(), &ty, nominal_types),
    )
}

fn function_param_higher_order_bindings(
    pattern: &Pattern,
    ty: &TypeKind,
    nominal_types: NominalTypeContext<'_>,
) -> Vec<FunctionParamHigherOrderBinding> {
    let mut bindings = Vec::new();
    collect_function_param_higher_order_bindings(
        pattern,
        ty,
        FunctionParamSelector::Root,
        nominal_types,
        &mut bindings,
    );
    bindings
}

fn collect_function_param_higher_order_bindings(
    pattern: &Pattern,
    ty: &TypeKind,
    selector: FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. }
            if is_local_ident(name) && matches!(ty, TypeKind::Function { .. }) =>
        {
            bindings.push(FunctionParamHigherOrderBinding::new(
                name.clone(),
                ty.clone(),
                selector,
            ));
        }
        Pattern::Tuple(items) => {
            let TypeKind::Tuple(item_types) = ty else {
                return;
            };
            collect_tuple_function_param_higher_order_bindings(
                items,
                item_types,
                &selector,
                nominal_types,
                bindings,
            );
        }
        Pattern::Whole { name, pattern } => {
            if is_local_ident(name) && matches!(ty, TypeKind::Function { .. }) {
                bindings.push(FunctionParamHigherOrderBinding::new(
                    name.clone(),
                    ty.clone(),
                    selector.clone(),
                ));
            }
            collect_function_param_higher_order_bindings(
                pattern,
                ty,
                selector,
                nominal_types,
                bindings,
            );
        }
        Pattern::Record { fields, .. } => {
            collect_record_function_param_higher_order_bindings(
                pattern,
                fields,
                ty,
                &selector,
                nominal_types,
                bindings,
            );
        }
        Pattern::Variant {
            payload: Some(VariantPatternPayload::Record { fields, rest: _ }),
            name,
            ..
        } => {
            collect_variant_record_function_param_higher_order_bindings(
                name,
                fields,
                pattern,
                ty,
                &selector,
                nominal_types,
                bindings,
            );
        }
        Pattern::BracketSeq { items, .. }
        | Pattern::Variant {
            payload: Some(VariantPatternPayload::Tuple(items)),
            name: _,
            ..
        } => {
            if let Pattern::Variant { name, .. } = pattern {
                collect_variant_tuple_function_param_higher_order_bindings(
                    name,
                    items,
                    ty,
                    &selector,
                    nominal_types,
                    bindings,
                );
            } else {
                collect_bracket_seq_function_param_higher_order_bindings(
                    items,
                    &selector,
                    nominal_types,
                    bindings,
                );
            }
        }
        Pattern::Ident(_)
        | Pattern::MutIdent(_)
        | Pattern::Typed { .. }
        | Pattern::Literal(_)
        | Pattern::Entity(_)
        | Pattern::Discard
        | Pattern::Raw(_)
        | Pattern::Variant { payload: None, .. } => {}
    }
}

fn collect_tuple_function_param_higher_order_bindings(
    items: &[Pattern],
    item_types: &[TypeKind],
    selector: &FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    for (index, (item, item_ty)) in items.iter().zip(item_types).enumerate() {
        collect_function_param_higher_order_bindings(
            item,
            item_ty,
            selector_with_tuple_index(selector, index),
            nominal_types,
            bindings,
        );
    }
}

fn collect_record_function_param_higher_order_bindings(
    pattern: &Pattern,
    fields: &[RecordPatternField],
    ty: &TypeKind,
    selector: &FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    for field in fields {
        let Some(field_ty) = pattern_type_hint(field.pattern())
            .or_else(|| record_pattern_field_type(pattern, ty, field.name(), nominal_types.fields))
        else {
            continue;
        };
        collect_function_param_higher_order_bindings(
            field.pattern(),
            &field_ty,
            selector_with_record_field(selector, field.name()),
            nominal_types,
            bindings,
        );
    }
}

fn collect_variant_record_function_param_higher_order_bindings(
    variant: &str,
    fields: &[RecordPatternField],
    pattern: &Pattern,
    ty: &TypeKind,
    selector: &FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    let payload_ty = variant_payload_type_for_name(variant, Some(ty));
    let nominal_payload = enum_variant_payload_type_for_name(
        variant,
        ty,
        nominal_types.variant_payloads,
        nominal_types.env,
    );
    let payload_selector = selector_with_variant_payload(selector, variant);
    for field in fields {
        let Some(field_ty) = pattern_type_hint(field.pattern()).or_else(|| {
            nominal_payload
                .as_ref()
                .and_then(|payload| payload.record_field_type(field.name()))
                .or_else(|| {
                    payload_ty.as_ref().and_then(|payload_ty| {
                        record_pattern_field_type(
                            pattern,
                            payload_ty,
                            field.name(),
                            nominal_types.fields,
                        )
                    })
                })
        }) else {
            continue;
        };
        collect_function_param_higher_order_bindings(
            field.pattern(),
            &field_ty,
            selector_with_record_field(&payload_selector, field.name()),
            nominal_types,
            bindings,
        );
    }
}

fn collect_variant_tuple_function_param_higher_order_bindings(
    variant: &str,
    items: &[Pattern],
    ty: &TypeKind,
    selector: &FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    let nominal_payload = enum_variant_payload_type_for_name(
        variant,
        ty,
        nominal_types.variant_payloads,
        nominal_types.env,
    );
    let Some(payload_ty) = nominal_payload
        .as_ref()
        .and_then(EnumVariantPayload::single_type)
        .or_else(|| {
            nominal_payload
                .is_none()
                .then(|| variant_payload_type_for_name(variant, Some(ty)))
                .flatten()
        })
    else {
        return;
    };
    let payload_selector = selector_with_variant_payload(selector, variant);
    if items.len() == 1 {
        collect_function_param_higher_order_bindings(
            &items[0],
            &payload_ty,
            payload_selector,
            nominal_types,
            bindings,
        );
        return;
    }
    let item_types = match payload_ty {
        TypeKind::Tuple(item_types) => item_types,
        _ => nominal_payload
            .as_ref()
            .and_then(EnumVariantPayload::tuple_items)
            .unwrap_or_default(),
    };
    if item_types.is_empty() {
        return;
    }
    collect_tuple_function_param_higher_order_bindings(
        items,
        &item_types,
        &payload_selector,
        nominal_types,
        bindings,
    );
}

fn collect_bracket_seq_function_param_higher_order_bindings(
    items: &[Pattern],
    selector: &FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    for item in items {
        collect_function_param_higher_order_bindings(
            item,
            &TypeKind::Unit,
            selector.clone(),
            nominal_types,
            bindings,
        );
    }
}

fn selector_with_tuple_index(
    selector: &FunctionParamSelector,
    index: usize,
) -> FunctionParamSelector {
    match selector {
        FunctionParamSelector::Root => FunctionParamSelector::TupleIndex(vec![index]),
        FunctionParamSelector::TupleIndex(path) => {
            let mut path = path.clone();
            path.push(index);
            FunctionParamSelector::TupleIndex(path)
        }
        FunctionParamSelector::Path(path) => {
            let mut path = path.clone();
            path.push(FunctionParamSelectorSegment::TupleIndex(index));
            FunctionParamSelector::Path(path)
        }
    }
}

fn selector_with_record_field(
    selector: &FunctionParamSelector,
    field: &str,
) -> FunctionParamSelector {
    let segment = FunctionParamSelectorSegment::RecordField(field.to_owned());
    match selector {
        FunctionParamSelector::Root => FunctionParamSelector::Path(vec![segment]),
        FunctionParamSelector::TupleIndex(path) => {
            let mut path = path
                .iter()
                .copied()
                .map(FunctionParamSelectorSegment::TupleIndex)
                .collect::<Vec<_>>();
            path.push(segment);
            FunctionParamSelector::Path(path)
        }
        FunctionParamSelector::Path(path) => {
            let mut path = path.clone();
            path.push(segment);
            FunctionParamSelector::Path(path)
        }
    }
}

fn selector_with_variant_payload(
    selector: &FunctionParamSelector,
    variant: &str,
) -> FunctionParamSelector {
    let segment = FunctionParamSelectorSegment::VariantPayload(normalize_variant_name(variant));
    match selector {
        FunctionParamSelector::Root => FunctionParamSelector::Path(vec![segment]),
        FunctionParamSelector::TupleIndex(path) => {
            let mut path = path
                .iter()
                .copied()
                .map(FunctionParamSelectorSegment::TupleIndex)
                .collect::<Vec<_>>();
            path.push(segment);
            FunctionParamSelector::Path(path)
        }
        FunctionParamSelector::Path(path) => {
            let mut path = path.clone();
            path.push(segment);
            FunctionParamSelector::Path(path)
        }
    }
}

fn selected_higher_order_argument<'a>(
    selector: &FunctionParamSelector,
    value: &'a Expr,
    actual: &'a TypeKind,
    fallback_ty: &'a TypeKind,
) -> Option<(&'a Expr, &'a TypeKind)> {
    match selector {
        FunctionParamSelector::Root => Some((value, actual)),
        FunctionParamSelector::TupleIndex(path) => {
            let mut value = value;
            let mut actual = actual;
            for index in path {
                let (Expr::Tuple(values), TypeKind::Tuple(types)) = (value, actual) else {
                    return None;
                };
                value = values.get(*index)?;
                actual = types.get(*index)?;
            }
            Some((value, actual))
        }
        FunctionParamSelector::Path(path) => {
            let mut value = value;
            let mut actual = Some(actual);
            for segment in path {
                match segment {
                    FunctionParamSelectorSegment::TupleIndex(index) => {
                        let Expr::Tuple(values) = value else {
                            return None;
                        };
                        value = values.get(*index)?;
                        actual = match actual {
                            Some(TypeKind::Tuple(types)) => types.get(*index),
                            _ => None,
                        };
                    }
                    FunctionParamSelectorSegment::RecordField(field) => {
                        let (Expr::Record { fields, .. } | Expr::RecordLiteral(fields)) = value
                        else {
                            return None;
                        };
                        value = fields
                            .iter()
                            .find_map(|(name, value)| (name == field).then_some(value))?;
                        actual = None;
                    }
                    FunctionParamSelectorSegment::VariantPayload(variant) => match value {
                        Expr::Call { callee, args } => {
                            let callee = expr_path_label(callee)?;
                            if !variant_constructor_matches(&callee, variant) {
                                return None;
                            }
                            let [CallArg::Positional(payload)] = args.as_slice() else {
                                return None;
                            };
                            value = payload;
                            actual = None;
                        }
                        Expr::Record { path, .. } if variant_constructor_matches(path, variant) => {
                            actual = None;
                        }
                        _ => return None,
                    },
                }
            }
            Some((value, actual.unwrap_or(fallback_ty)))
        }
    }
}

fn normalize_variant_name(name: &str) -> String {
    name.strip_prefix('.').unwrap_or(name).to_owned()
}

fn variant_constructor_matches(path: &str, variant: &str) -> bool {
    let path = normalize_variant_name(path);
    path == variant
        || path
            .rsplit_once('.')
            .is_some_and(|(_, name)| name == variant)
}

fn pattern_type_hint(pattern: &Pattern) -> Option<TypeKind> {
    match pattern {
        Pattern::Typed { ty, .. } => Some(type_ref_kind(ty)),
        Pattern::Tuple(items) => items
            .iter()
            .map(pattern_type_hint)
            .collect::<Option<Vec<_>>>()
            .map(TypeKind::Tuple),
        Pattern::Whole { pattern, .. } => pattern_type_hint(pattern),
        _ => None,
    }
}

fn record_pattern_field_type(
    pattern: &Pattern,
    ty: &TypeKind,
    field: &str,
    nominal_fields: Option<&HashMap<String, HashMap<String, TypeKind>>>,
) -> Option<TypeKind> {
    let Pattern::Record { path, .. } = pattern else {
        return None;
    };
    let record_name = path.as_deref().or_else(|| match ty {
        TypeKind::Named(name) => Some(name.as_str()),
        TypeKind::BorrowRef { inner, .. } | TypeKind::Shared(inner) => {
            nominal_record_type_name(inner)
        }
        _ => None,
    })?;
    nominal_fields?
        .get(record_name)
        .and_then(|fields| fields.get(field))
        .cloned()
}

fn enum_variant_payload_type_for_name(
    variant: &str,
    ty: &TypeKind,
    nominal_variant_payloads: Option<&HashMap<String, HashMap<String, EnumVariantPayload>>>,
    env: Option<&TypeCheckEnv>,
) -> Option<EnumVariantPayload> {
    let variant = normalize_variant_name(variant);
    let variant = variant
        .rsplit_once('.')
        .map_or(variant.as_str(), |(_, name)| name);
    nominal_record_type_name(ty)
        .and_then(|enum_name| {
            nominal_variant_payloads?
                .get(enum_name)?
                .get(variant)
                .cloned()
        })
        .or_else(|| env_variant_payload_type_for_name(ty, variant, env))
}

fn env_variant_payload_type_for_name(
    ty: &TypeKind,
    variant: &str,
    env: Option<&TypeCheckEnv>,
) -> Option<EnumVariantPayload> {
    match ty {
        TypeKind::BorrowRef { inner, .. } | TypeKind::Shared(inner) => {
            env_variant_payload_type_for_name(inner, variant, env)
        }
        ty => env?.enum_variant_payload(ty, variant).cloned(),
    }
}

fn nominal_record_type_name(ty: &TypeKind) -> Option<&str> {
    match ty {
        TypeKind::Named(name) => Some(name),
        TypeKind::BorrowRef { inner, .. } | TypeKind::Shared(inner) => {
            nominal_record_type_name(inner)
        }
        _ => None,
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
