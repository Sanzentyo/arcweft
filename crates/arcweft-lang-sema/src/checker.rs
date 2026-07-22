use crate::borrow::{
    BorrowLocalState, BorrowStateCheckpoint, BorrowStateDelta, BorrowStateDeltaEntry,
    BorrowStateJournalEntry, merge_borrow_local_states,
};
use crate::callable::{
    CallTargetFactMode, CallTargetFacts, CallTargetFactsInput, CallableDiagnostic,
    CallableDiagnosticCode, CallableDiagnosticRelated, CallableDiagnosticSeverity,
    CallableDiagnosticSubject, CheckedCallTarget, PRODUCTION_CALLABLE_LIMITS, ResolvedCallable,
};
use crate::canonicalization::{
    CanonicalizationSourceSet, CheckedCanonicalizationInventory, CheckedSpeakerLine,
    SemanticScopeId, SemanticSymbolIdentity,
};
use crate::diagnostics::{
    TraitDiagnostic, TypeCheckError, TypeCheckReadinessError, TypeCheckWarning,
};
use crate::dialogue_view::DialogueViewModelRegistry;
use crate::effect_analysis::EffectAnalysisReport;
use crate::effect_collector::EffectCollector;
use crate::effect_model::{CallableId, CallableKind, EffectContract, EffectSite, Visibility};
use crate::effect_row::EffectRow;
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
use crate::nominal::{GenericTypeScope, NominalResolutionIndex, SelfTypeScope};
use crate::symbols::{SymbolUseKind, collect_symbol_uses};
use crate::traits::{
    ProjectionError, TraitCatalog, TraitPredicate, collect_trait_catalog,
    trait_predicate_inputs_for_signature,
};
use crate::types::{ArrayLength, EntityKind, MapKind, TypeKind};
use arcweft_lang_hir::{
    model::{HirFlowItem, HirModule, HirTopLevelDecl},
    symbol::{CallableDeclarationId, ProjectSymbolTable},
};
use arcweft_lang_syntax::{
    ast::{
        choice::ChoiceAction,
        common::TextRange,
        dialogue::{DialogueContent, DialogueToken},
        flow::{AwaitBranchKind, ContractClause, SelectBranchHead, Stmt},
        ids::{EntityRef, EntityRefSyntax, IdRef},
        items::{EntityDeclKind, FunctionKind},
        line_plan::{CancelRuleSyntax, LinePlanItem, TriggerPattern},
        module_path::CanonicalModulePath,
        pattern::{Pattern, RecordPatternField, VariantPatternPayload},
        source::{
            SourceBackpressurePolicy, SourceEventPattern, SourceHeader, SourcePrivacyPolicy,
            SourceReplayPolicy,
        },
    },
    expr::{CallArg, Expr, LifetimeAccessMode, LifetimeKey, LifetimeScopeKind, Literal},
    types::{FnParam, FnSignature, TypeRef},
};
use arcweft_source::SourceSpan;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

mod assertion;
pub mod borrow_state;
mod call_target_facts;
mod canonicalization;
pub mod choice;
pub mod effects;
pub mod expr;
pub mod flow;
pub mod fx;
pub mod helpers;
pub mod iterator;
pub mod lifetime_access;
pub mod line_plan;
pub mod module;
mod nominal_resolution;
pub mod presentation;
mod registered_candidate_transaction;
#[cfg(test)]
mod registered_candidate_transaction_tests;
pub(crate) mod signature;
pub mod source;
pub mod source_ranges;
pub mod stmt;
pub mod suspension;

pub use module::{
    analyze_project_types_for_canonicalization, analyze_registered_project_types,
    analyze_registered_project_types_for_canonicalization,
    analyze_registered_project_types_for_focused_call, analyze_types,
};

pub(crate) use call_target_facts::FocusedCallSite;
use call_target_facts::{
    CallResolverControl, CallTargetFactRecorder, CallTargetFactReport, CallableWorkOperation,
};
use fx::FxCatalog;
use helpers::{
    await_branch_pattern_type, builtin_path_type, choice_output_type,
    default_presentation_slot_family, entity_kind, entity_kind_for_decl, entity_syntax_kind,
    expr_path_label, ident_pattern_name, is_character_entity_literal, is_drop_callee,
    is_local_ident, iter_item_type, merge_line_output, pattern_bindings_with_fallback,
    pattern_bindings_with_nominal_types, stmts_diverge, unify_loop_break_types,
    variant_payload_type_for_name,
};
use signature::{
    available_effect_set, enum_variant_payload_type_for_name, function_param_higher_order_bindings,
    function_signature_from_resolved, selected_higher_order_argument,
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

fn function_callable_id(function_name: &str) -> CallableId {
    CallableId::source_function(function_name)
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
    /// Exact accepted source identity and range when this judgment came from a
    /// source-bound project module.
    pub source: Option<SourceSpan>,
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
    /// Exact project function and function-local expression identity when the
    /// evidence was produced while checking an ordinary callable body.
    pub owner: Option<TypedLoweringEvidenceOwner>,
    pub kind: TypedLoweringEvidenceKind,
}

impl TypedLoweringEvidence {
    fn new(expression_id: TypeExpressionId, kind: TypedLoweringEvidenceKind) -> Self {
        Self {
            expression_id,
            owner: None,
            kind,
        }
    }
}

/// Exact owner-relative identity for lowering evidence inside a project
/// function body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedLoweringEvidenceOwner {
    pub declaration: CallableDeclarationId,
    pub expression_id: TypeExpressionId,
}

/// Lowering-sensitive semantic facts proven during type checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypedLoweringEvidenceKind {
    /// Numeric literal or compact integer sequence after suffix, expected-type,
    /// and fallback resolution selected one concrete primitive representation.
    ResolvedNumericType { target: TypeKind },
    /// A call expression's callee evaluated to a function value.
    FunctionValueCall {
        callee: Option<String>,
        callee_ty: TypeKind,
        result_ty: TypeKind,
        arg_count: usize,
        /// Whether this call supplied fewer arguments than the current call group.
        partial: bool,
    },
    /// An expression was checked in a function-typed context.
    ExpectedFunctionValue {
        expected_ty: TypeKind,
        actual_ty: TypeKind,
        arity: usize,
    },
    /// A top-level function path was referenced as a first-class function
    /// value rather than called directly.
    FunctionValueReference { callee: String, ty: TypeKind },
    /// A direct named function signature call returned a partial function.
    SignaturePartialCall {
        callee: String,
        result_ty: TypeKind,
        arg_count: usize,
    },
    /// A function-valued expression owns a callable used by effect-row reports.
    FunctionEffectCallable { callable: CallableId },
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
    /// Source method-call argument in the fallback callable's first stage.
    CallArg { index: usize },
    /// The method-call receiver applied as one separate final call group.
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

/// Numeric literal family whose type was selected by the stable fallback rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericFallbackKind {
    IntegerLiteral,
    FloatLiteral,
    IntegerSequence,
}

/// Machine-readable fallback evidence for lint and editor policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NumericFallback {
    pub expression_id: TypeExpressionId,
    pub kind: NumericFallbackKind,
    pub fallback: TypeKind,
    pub inferred_contract: bool,
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
    pub numeric_fallbacks: Vec<NumericFallback>,
    /// Accepted source-backed nominal facts for this exact project transaction.
    pub nominal_resolutions: NominalResolutionIndex,
    /// Checked dialogue View projection models produced from semantic field types.
    pub dialogue_view_models: DialogueViewModelRegistry,
    /// Invocation behavior derived from checked return types and callable-owned body facts.
    ///
    /// Reports produced without a linked project symbol table leave this empty because a
    /// checked execution fact is never published without its canonical declaration identity.
    pub callable_executions: Vec<CheckedCallableExecution>,
    pub effects: EffectAnalysisReport,
    pub for_iteration_evidence: Vec<ForIterationEvidence>,
    pub trait_catalog: TraitCatalog,
    pub style_catalog: crate::style::CheckedViewStyleCatalog,
    pub view_part_catalog: crate::view_part::CheckedViewPartCatalog,
    pub view_part_diagnostics: Vec<crate::view_part::ViewPartDiagnostic>,
    pub canonicalization_inventories: Vec<CheckedCanonicalizationInventory>,
    /// Exact source ranges of calls resolved to ordinary project functions.
    pub project_callable_references: Vec<ProjectCallableReference>,
    /// Exact source ranges of authored absolute entity references.
    pub project_entity_references: Vec<ProjectEntityReference>,
    call_target_fact_report: CallTargetFactReport,
}

/// Checked invocation behavior for one canonical project callable declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableExecution {
    declaration: CallableDeclarationId,
    mode: CallableExecutionMode,
}

impl CheckedCallableExecution {
    fn new(declaration: CallableDeclarationId, mode: CallableExecutionMode) -> Self {
        Self { declaration, mode }
    }

    /// Returns the declaration identity shared with the project callable catalog.
    pub const fn declaration(&self) -> &CallableDeclarationId {
        &self.declaration
    }

    /// Returns the checked invocation behavior without projecting a source role.
    pub const fn mode(&self) -> &CallableExecutionMode {
        &self.mode
    }
}

/// Runtime-relevant invocation behavior derived after semantic checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableExecutionMode {
    /// The call enters an ordinary frame immediately, including suspending functions and
    /// functions that return an already-existing Stream value.
    DirectFrame,
    /// The call constructs a lazy Stream producer owned by this declaration.
    StreamFactory {
        item: TypeKind,
        error: TypeKind,
        generator: StreamGeneratorFacts,
    },
}

/// Callable-owned syntax facts used to classify one Stream generator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamGeneratorFacts {
    own_scope_yield_count: usize,
}

impl StreamGeneratorFacts {
    fn new(own_scope_yield_count: usize) -> Self {
        debug_assert!(own_scope_yield_count > 0);
        Self {
            own_scope_yield_count,
        }
    }

    /// Returns the number of syntactic yields owned by the declaration's generator scope.
    pub const fn own_scope_yield_count(self) -> usize {
        self.own_scope_yield_count
    }
}

/// One typed ordinary-call reference selected by the shared callable resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCallableReference {
    module: CanonicalModulePath,
    declaration: CallableDeclarationId,
    range: TextRange,
}

impl ProjectCallableReference {
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn declaration(&self) -> &CallableDeclarationId {
        &self.declaration
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

/// One authored absolute entity-reference token retained for typed tooling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectEntityReference {
    module: CanonicalModulePath,
    name: String,
    range: TextRange,
}

impl ProjectEntityReference {
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
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

#[derive(Clone, Copy, Debug)]
struct SignatureWorkChargeState {
    candidate_work: bool,
}

#[derive(Clone)]
struct CallableDiagnosticDraft {
    code: CallableDiagnosticCode,
    severity: CallableDiagnosticSeverity,
    span: Option<arcweft_source::SourceSpan>,
    subject: CallableDiagnosticSubject,
    related: Vec<CallableDiagnosticRelated>,
}

impl CallableDiagnosticDraft {
    fn error(
        code: CallableDiagnosticCode,
        span: Option<arcweft_source::SourceSpan>,
        subject: CallableDiagnosticSubject,
    ) -> Self {
        Self {
            code,
            severity: CallableDiagnosticSeverity::Error,
            span,
            subject,
            related: Vec::new(),
        }
    }

    #[must_use]
    fn with_related(
        mut self,
        subject: CallableDiagnosticSubject,
        span: Option<arcweft_source::SourceSpan>,
    ) -> Self {
        self.related
            .push(CallableDiagnosticRelated::new(subject, span));
        self
    }
}

struct TypeChecker<'a> {
    env: &'a TypeCheckEnv,
    checked_module: &'a HirModule,
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
    ordinary_source_functions: HashSet<String>,
    action_signatures: HashMap<String, ActionSignature>,
    nominal_fields: HashMap<String, HashMap<String, TypeKind>>,
    project_nominal_shapes: crate::nominal::ProjectNominalShapeCatalog,
    dialogue_view_models: DialogueViewModelRegistry,
    nominal_variant_payloads: HashMap<String, HashMap<String, EnumVariantPayload>>,
    trait_catalog: TraitCatalog,
    trait_predicate_stack: Vec<Vec<TraitPredicate>>,
    flow_params: HashMap<String, HashSet<String>>,
    fx: FxCatalog,
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
    assertion_effect_conditions: BTreeMap<CallableId, usize>,
    next_assertion_effect_scope: u64,
    expected_returns: Vec<Option<TypeKind>>,
    partial_placeholder_stack: Vec<TypeKind>,
    pipe_left_stack: Vec<PipeLeftBinding>,
    allow_inferred_signature_partial_calls: bool,
    yield_stack: Vec<YieldContext>,
    stats: TypeCheckStats,
    judgments: Vec<TypeJudgment>,
    typed_lowering_evidence: Vec<TypedLoweringEvidence>,
    typed_lowering_owner: Option<TypedLoweringOwnerScope>,
    expression_source_ranges: HashMap<ExprNodeKey, TextRange>,
    closure_capture_stack: Vec<ClosureCaptureFrame>,
    closure_inference_stack: Vec<ClosureInferenceContext>,
    closure_captures: Vec<ClosureCaptureInventory>,
    numeric_fallbacks: Vec<NumericFallback>,
    nominal_resolution_cache: crate::nominal::CheckedTypeReferenceCache,
    nominal_resolutions: NominalResolutionIndex,
    checked_anonymous_choice_roots: HashSet<arcweft_source::SourceSpan>,
    active_generic_scope: crate::nominal::GenericTypeScope,
    active_self_scope: crate::nominal::SelfTypeScope,
    callable_executions: Vec<CheckedCallableExecution>,
    allow_signed_min_literal: bool,
    local_function_effects: HashMap<String, CallableId>,
    closure_effect_callables_by_expr: HashMap<ExprNodeKey, CallableId>,
    last_checked_closure_effect_callable: Option<CallableId>,
    function_return_effect_callables: HashMap<String, CallableId>,
    local_callable_signatures: HashMap<String, SourceCallableSignature>,
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
    canonicalization_sources: Option<&'a CanonicalizationSourceSet>,
    project_symbols: Option<&'a ProjectSymbolTable>,
    registered_environment: Option<&'a crate::registration::RegisteredTypeCheckEnv>,
    registered_world: Option<&'a crate::registration::RegisteredSemanticWorld>,
    project_functions: BTreeMap<CallableDeclarationId, TypeKind>,
    project_function_signatures: BTreeMap<CallableDeclarationId, FunctionSignature>,
    project_callable_references: Vec<ProjectCallableReference>,
    project_entity_references: Vec<ProjectEntityReference>,
    call_target_fact_recorder: CallTargetFactRecorder,
    call_resolver_control: CallResolverControl<'a>,
    signature_work_charge: SignatureWorkChargeState,
    focused_candidate_depth: usize,
    local_symbol_identities: HashMap<String, SemanticSymbolIdentity>,
    semantic_scope_stack: Vec<SemanticScopeId>,
    next_semantic_scope: u32,
    next_semantic_binding: u32,
    current_module: Option<arcweft_lang_syntax::ast::module_path::CanonicalModulePath>,
    checked_speaker_lines: Vec<CheckedSpeakerLine>,
    registered_candidate_transaction_depth: usize,
    registered_candidate_journal:
        Vec<registered_candidate_transaction::RegisteredCandidateMutation>,
}

/// Type and authored provenance of the value bound by one active pipe RHS.
///
/// `^` is a lexical read, but diagnostics and judgments still point at the
/// authored expression that produced the binding rather than at the read token.
#[derive(Clone, Debug)]
struct PipeLeftBinding {
    ty: TypeKind,
    source_range: Option<TextRange>,
}

#[derive(Clone, Debug)]
struct TypeCheckerScopeSnapshot {
    borrow_checkpoint: BorrowStateCheckpoint,
    active_presentation_defaults: HashMap<String, String>,
    lifetime_guarantees: HashSet<LifetimeKey>,
    dropped_lifetime_keys: HashSet<LifetimeKey>,
    available_lifetimes: Vec<LifetimeScopeKind>,
}

#[derive(Clone, Debug)]
struct TypedLoweringOwnerScope {
    declaration: CallableDeclarationId,
    expression_base: usize,
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
pub(crate) struct NominalTypeContext<'a> {
    fields: Option<&'a HashMap<String, HashMap<String, TypeKind>>>,
    variant_payloads: Option<&'a HashMap<String, HashMap<String, EnumVariantPayload>>>,
    project: Option<&'a crate::nominal::ProjectNominalShapeCatalog>,
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
    pub(crate) const fn empty() -> Self {
        Self {
            fields: None,
            variant_payloads: None,
            project: None,
            env: None,
        }
    }

    const fn new(
        fields: &'a HashMap<String, HashMap<String, TypeKind>>,
        variant_payloads: &'a HashMap<String, HashMap<String, EnumVariantPayload>>,
        project: &'a crate::nominal::ProjectNominalShapeCatalog,
        env: &'a TypeCheckEnv,
    ) -> Self {
        Self {
            fields: Some(fields),
            variant_payloads: Some(variant_payloads),
            project: Some(project),
            env: Some(env),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct LocalBindingSnapshot {
    entries: Vec<LocalBindingSnapshotEntry>,
    entered_semantic_scope: Option<SemanticScopeId>,
}

#[derive(Clone, Debug)]
struct LocalBindingSnapshotEntry {
    name: String,
    previous_ty: Option<TypeKind>,
    previous_function_effect: Option<CallableId>,
    previous_callable_signature: Option<SourceCallableSignature>,
    previous_curried_signature_call: Option<CurriedSignatureCallValue>,
    previous_higher_order_param_alias: Option<String>,
    previous_symbol_identity: Option<SemanticSymbolIdentity>,
}

#[derive(Clone, Debug)]
struct SourceCallableSignature {
    source_name: String,
    signature: FunctionSignature,
}

impl SourceCallableSignature {
    fn new(source_name: impl Into<String>, signature: FunctionSignature) -> Self {
        Self {
            source_name: source_name.into(),
            signature,
        }
    }
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
    current_group_params: Option<Vec<FunctionParam>>,
    pending_higher_order_args: Vec<PendingCurriedHigherOrderArg>,
    resolved: Option<ResolvedCallable>,
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
    fn new<'a>(env: &'a TypeCheckEnv, module: &'a HirModule) -> TypeChecker<'a> {
        TypeChecker::new_with_project(
            env,
            module,
            None,
            None,
            None,
            CallTargetFactMode::Disabled,
            CallResolverControl::ordinary(),
        )
    }
}

impl<'a> TypeChecker<'a> {
    fn charge_signature_work(
        &mut self,
        kind: crate::callable::SignatureWorkKind,
        units: u64,
    ) -> bool {
        match self.call_resolver_control.charge_signature(kind, units) {
            Ok(()) => true,
            Err(reason) => {
                self.call_target_fact_recorder
                    .record_signature_accounting_error(reason);
                false
            }
        }
    }

    fn charge_callable_work(
        &mut self,
        call: &arcweft_lang_syntax::expr::CallExpr,
        focused: bool,
        operation: CallableWorkOperation,
    ) -> bool {
        let call_span = self.source_span_for_current_range(call.range());
        self.charge_callable_work_for_span(call_span.as_ref(), focused, operation)
    }

    fn charge_callable_work_for_span(
        &mut self,
        call_span: Option<&arcweft_source::SourceSpan>,
        focused: bool,
        operation: CallableWorkOperation,
    ) -> bool {
        match self
            .call_resolver_control
            .charge_callable_operation(focused, operation)
        {
            Ok(()) => true,
            Err(error) => {
                let error = crate::callable::ResolveCallError::Work(error);
                self.call_target_fact_recorder
                    .record_resolve_error(call_span, error.clone());
                self.errors.push(TypeCheckError::new(error.to_string()));
                false
            }
        }
    }

    fn new_with_project(
        env: &'a TypeCheckEnv,
        checked_module: &'a HirModule,
        canonicalization_sources: Option<&'a CanonicalizationSourceSet>,
        project_symbols: Option<&'a ProjectSymbolTable>,
        registered_world: Option<&'a crate::registration::RegisteredSemanticWorld>,
        call_target_fact_mode: CallTargetFactMode,
        call_resolver_control: CallResolverControl<'a>,
    ) -> Self {
        let (registered_environment, nominal_resolutions, nominal_diagnostics) =
            Self::initial_nominal_resolution_state(registered_world);
        TypeChecker {
            env,
            checked_module,
            errors: nominal_diagnostics,
            warnings: Vec::new(),
            active_borrow_lifetimes: BTreeMap::new(),
            active_borrow_total: 0,
            borrow_local_lifetimes: HashMap::new(),
            borrow_state_journal: Vec::new(),
            global_symbols: HashMap::new(),
            global_functions: HashMap::new(),
            global_function_signatures: HashMap::new(),
            global_function_effects: HashMap::new(),
            ordinary_source_functions: HashSet::new(),
            action_signatures: HashMap::new(),
            nominal_fields: env.nominal_records.clone(),
            project_nominal_shapes: crate::nominal::ProjectNominalShapeCatalog::default(),
            dialogue_view_models: env.dialogue_view_models.clone(),
            nominal_variant_payloads: HashMap::new(),
            trait_catalog: TraitCatalog::default(),
            trait_predicate_stack: Vec::new(),
            flow_params: HashMap::new(),
            fx: FxCatalog::default(),
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
            effect_capabilities: Self::initial_effect_capabilities(env),
            effect_collector: EffectCollector::new(available_effect_set(env)),
            assertion_effect_conditions: BTreeMap::new(),
            next_assertion_effect_scope: 0,
            expected_returns: Vec::new(),
            partial_placeholder_stack: Vec::new(),
            pipe_left_stack: Vec::new(),
            allow_inferred_signature_partial_calls: true,
            yield_stack: Vec::new(),
            stats: TypeCheckStats::default(),
            judgments: Vec::new(),
            typed_lowering_evidence: Vec::new(),
            typed_lowering_owner: None,
            expression_source_ranges: HashMap::new(),
            closure_capture_stack: Vec::new(),
            closure_inference_stack: Vec::new(),
            closure_captures: Vec::new(),
            numeric_fallbacks: Vec::new(),
            nominal_resolution_cache: crate::nominal::CheckedTypeReferenceCache::default(),
            nominal_resolutions,
            checked_anonymous_choice_roots: HashSet::new(),
            active_generic_scope: crate::nominal::GenericTypeScope::empty(),
            active_self_scope: crate::nominal::SelfTypeScope::Absent,
            callable_executions: Vec::new(),
            allow_signed_min_literal: false,
            local_function_effects: HashMap::new(),
            closure_effect_callables_by_expr: HashMap::new(),
            last_checked_closure_effect_callable: None,
            function_return_effect_callables: HashMap::new(),
            local_callable_signatures: HashMap::new(),
            local_curried_signature_calls: HashMap::new(),
            last_checked_curried_signature_call: None,
            local_higher_order_param_aliases: HashMap::new(),
            higher_order_param_scope_stack: Vec::new(),
            higher_order_param_invocations: BTreeMap::new(),
            higher_order_param_closure_invocations: BTreeMap::new(),
            pending_higher_order_effect_calls: Vec::new(),
            for_iteration_evidence: Vec::new(),
            record_runtime_for_iteration_evidence: false,
            canonicalization_sources,
            project_symbols,
            registered_environment,
            registered_world,
            project_functions: BTreeMap::new(),
            project_function_signatures: BTreeMap::new(),
            project_callable_references: Vec::new(),
            project_entity_references: Vec::new(),
            call_target_fact_recorder: CallTargetFactRecorder::new(call_target_fact_mode),
            call_resolver_control,
            signature_work_charge: SignatureWorkChargeState {
                candidate_work: false,
            },
            focused_candidate_depth: 0,
            local_symbol_identities: HashMap::new(),
            semantic_scope_stack: Vec::new(),
            next_semantic_scope: 0,
            next_semantic_binding: 0,
            current_module: None,
            checked_speaker_lines: Vec::new(),
            registered_candidate_transaction_depth: 0,
            registered_candidate_journal: Vec::new(),
        }
    }

    fn initial_nominal_resolution_state(
        registered_world: Option<&'a crate::registration::RegisteredSemanticWorld>,
    ) -> (
        Option<&'a crate::registration::RegisteredTypeCheckEnv>,
        NominalResolutionIndex,
        Vec<TypeCheckError>,
    ) {
        let registered_environment =
            registered_world.map(crate::registration::RegisteredSemanticWorld::environment);
        let resolutions = registered_environment
            .map_or_else(NominalResolutionIndex::production, |environment| {
                environment.callable_catalog().nominal_resolutions().clone()
            });
        let diagnostics = resolutions
            .diagnostics()
            .iter()
            .cloned()
            .map(TypeCheckError::nominal)
            .collect();
        (registered_environment, resolutions, diagnostics)
    }

    fn initial_effect_capabilities(env: &TypeCheckEnv) -> HashSet<String> {
        env.capabilities
            .iter()
            .map(|capability| capability.as_str().to_owned())
            .collect()
    }

    pub(super) fn records_call_target_facts(
        &self,
        call_span: Option<&arcweft_source::SourceSpan>,
    ) -> bool {
        self.call_target_fact_recorder.wants(call_span)
    }

    pub(super) fn uses_focused_callable_work(
        &self,
        call_span: Option<&arcweft_source::SourceSpan>,
    ) -> bool {
        self.focused_candidate_depth != 0 || self.call_target_fact_recorder.focuses(call_span)
    }

    pub(super) fn record_call_target_facts(
        &mut self,
        expression: TypeExpressionId,
        document: &arcweft_source::SourceDocumentIdentity,
        call_span: &arcweft_source::SourceSpan,
        checked: CheckedCallTarget,
        diagnostic_drafts: Vec<CallableDiagnosticDraft>,
    ) {
        if !self.records_call_target_facts(Some(call_span)) {
            return;
        }
        let active_parameter = self.call_target_fact_recorder.active_parameter(&checked);
        let mut diagnostics = Vec::with_capacity(diagnostic_drafts.len());
        for draft in diagnostic_drafts {
            if !self.charge_callable_work_for_span(
                Some(call_span),
                true,
                CallableWorkOperation::Resolver,
            ) {
                return;
            }
            match CallableDiagnostic::try_new(
                draft.code,
                draft.severity,
                draft.span,
                draft.subject,
                draft.related,
                Some(document),
                &PRODUCTION_CALLABLE_LIMITS,
            ) {
                Ok(diagnostic) => diagnostics.push(diagnostic),
                Err(reason) => {
                    self.call_target_fact_recorder
                        .record_unavailable(call_span, reason);
                    return;
                }
            }
        }
        match CallTargetFacts::try_new(
            CallTargetFactsInput {
                expression,
                document: document.clone(),
                call_span: call_span.clone(),
                enclosing_callable: self
                    .typed_lowering_owner
                    .as_ref()
                    .map(|owner| owner.declaration.clone()),
                checked,
                active_parameter,
                diagnostics,
            },
            &PRODUCTION_CALLABLE_LIMITS,
        ) {
            Ok(facts) => self.call_target_fact_recorder.record(facts),
            Err(reason) => self
                .call_target_fact_recorder
                .record_unavailable(call_span, reason),
        }
    }

    fn insert_scoped_locals(
        &mut self,
        bindings: impl IntoIterator<Item = (String, TypeKind)>,
    ) -> LocalBindingSnapshot {
        let entered_semantic_scope = self.push_semantic_scope();
        LocalBindingSnapshot {
            entries: bindings
                .into_iter()
                .map(|(name, ty)| {
                    let previous_function_effect = self.local_function_effects.get(&name).cloned();
                    let previous_callable_signature =
                        self.local_callable_signatures.get(&name).cloned();
                    let previous_curried_signature_call =
                        self.local_curried_signature_calls.get(&name).cloned();
                    let previous_higher_order_param_alias =
                        self.local_higher_order_param_aliases.get(&name).cloned();
                    let previous_symbol_identity = self.local_symbol_identities.get(&name).cloned();
                    let previous_ty = self.bind_local(name.clone(), ty);
                    LocalBindingSnapshotEntry {
                        name,
                        previous_ty,
                        previous_function_effect,
                        previous_callable_signature,
                        previous_curried_signature_call,
                        previous_higher_order_param_alias,
                        previous_symbol_identity,
                    }
                })
                .collect(),
            entered_semantic_scope: Some(entered_semantic_scope),
        }
    }

    fn bind_local(&mut self, name: String, ty: TypeKind) -> Option<TypeKind> {
        let previous = self.locals.insert(name.clone(), ty);
        let previous_function_effect = self.local_function_effects.remove(&name);
        let previous_callable_signature = self.local_callable_signatures.remove(&name);
        let previous_curried_signature_call = self.local_curried_signature_calls.remove(&name);
        let previous_higher_order_param_alias = self.local_higher_order_param_aliases.remove(&name);
        let scope = self.current_semantic_scope();
        let binding = self.allocate_semantic_binding();
        let previous_symbol_identity = self.local_symbol_identities.insert(
            name.clone(),
            SemanticSymbolIdentity::Local {
                scope,
                binding,
                name: name.clone(),
            },
        );
        if let Some(frame) = self.closure_capture_stack.len().checked_sub(1) {
            self.retain_closure_frame_local(frame, name.clone());
        }
        if let Some(scope) = self.local_scope_stack.last_mut() {
            scope.entries.push(LocalBindingSnapshotEntry {
                name,
                previous_ty: previous.clone(),
                previous_function_effect,
                previous_callable_signature,
                previous_curried_signature_call,
                previous_higher_order_param_alias,
                previous_symbol_identity,
            });
        }
        previous
    }

    fn bind_local_function_effect(&mut self, name: &str, callable: CallableId) {
        self.local_function_effects
            .insert(name.to_owned(), callable);
    }

    fn bind_local_callable_signature(&mut self, name: &str, signature: SourceCallableSignature) {
        self.local_callable_signatures
            .insert(name.to_owned(), signature);
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
        for entry in snapshot.entries.into_iter().rev() {
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
            if let Some(signature) = entry.previous_callable_signature {
                self.local_callable_signatures
                    .insert(entry.name.clone(), signature);
            } else {
                self.local_callable_signatures.remove(&entry.name);
            }
            if let Some(value) = entry.previous_curried_signature_call {
                self.local_curried_signature_calls
                    .insert(entry.name.clone(), value);
            } else {
                self.local_curried_signature_calls.remove(&entry.name);
            }
            if let Some(param_name) = entry.previous_higher_order_param_alias {
                self.local_higher_order_param_aliases
                    .insert(entry.name.clone(), param_name);
            } else {
                self.local_higher_order_param_aliases.remove(&entry.name);
            }
            if let Some(identity) = entry.previous_symbol_identity {
                self.local_symbol_identities.insert(entry.name, identity);
            } else {
                self.local_symbol_identities.remove(&entry.name);
            }
        }
        if snapshot.entered_semantic_scope.is_some() {
            self.pop_semantic_scope();
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
        let entered_semantic_scope = self.push_semantic_scope();
        self.local_scope_stack.push(LocalBindingSnapshot {
            entries: Vec::new(),
            entered_semantic_scope: Some(entered_semantic_scope),
        });
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
        let source = source_range.and_then(|range| self.source_span_for_current_range(range));
        self.judgments.push(TypeJudgment {
            id,
            subject,
            ty,
            rule,
            expected: stored_expected,
            source_range,
            source,
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

    fn record_typed_lowering_evidence(&mut self, mut evidence: TypedLoweringEvidence) {
        evidence.owner = self.typed_lowering_owner.as_ref().map(|owner| {
            let local_index = evidence
                .expression_id
                .index()
                .checked_sub(owner.expression_base)
                .expect("typed lowering evidence must follow its callable expression base");
            TypedLoweringEvidenceOwner {
                declaration: owner.declaration.clone(),
                expression_id: TypeExpressionId::from_index(local_index),
            }
        });
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
        self.retain_closure_effect_callable(ExprNodeKey::from_expr(expr), callable.clone());
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
        upper_bound: Option<EffectSet>,
    ) -> (CallableId, EffectRow, Option<CallableId>) {
        let id = closure_effect_callable_id(expression_id);
        let source_name = id.as_str().to_owned();
        let contract = upper_bound.map_or_else(EffectContract::inferred, EffectContract::bounded);
        if let Err(error) = self.effect_collector.register_callable(
            source_name,
            id.clone(),
            CallableKind::Function,
            Visibility::Private,
            contract,
        ) {
            self.errors.push(TypeCheckError::new(error.to_string()));
        }
        let inferred_row = self.effect_collector.ensure_inferred_effect_row(&id);
        let previous = self.effect_collector.enter(id.clone());
        (id, inferred_row, previous)
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
        self.retain_closure_suspension_boundary(boundary);
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

    fn record_numeric_fallback(
        &mut self,
        expression_id: TypeExpressionId,
        kind: NumericFallbackKind,
        literal_kind: &'static str,
        fallback: TypeKind,
    ) {
        let inferred_contract = self
            .closure_inference_stack
            .last()
            .is_some_and(|context| context.inferred_return_type);
        self.numeric_fallbacks.push(NumericFallback {
            expression_id,
            kind,
            fallback: fallback.clone(),
            inferred_contract,
        });
        if inferred_contract {
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
        let mut inserted = Vec::new();
        for (index, frame) in self.closure_capture_stack.iter_mut().enumerate().rev() {
            if frame.locals.contains(name) {
                break;
            }
            if let std::collections::btree_map::Entry::Vacant(entry) =
                frame.captures.entry(name.to_owned())
            {
                entry.insert(ty.clone());
                inserted.push(index);
            }
        }
        for frame in inserted {
            self.retain_closure_capture(frame, name.to_owned());
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
        pending_higher_order_args: Vec<PendingCurriedHigherOrderArg>,
    ) {
        self.last_checked_curried_signature_call = (has_next_group_metadata
            && exact_group_call
            && matches!(result_ty, TypeKind::Function { .. }))
        .then(|| CurriedSignatureCallValue {
            function_name: function_name.to_owned(),
            remaining_group_index: next_group_index,
            group_arg_offset: 0,
            current_group_params: None,
            pending_higher_order_args,
            resolved: None,
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
        self.effect_collector.ensure_inferred_effect_row(&id);
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
        self.last_checked_closure_effect_callable = self
            .function_return_effect_callables
            .get(function_name)
            .cloned();
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
            self.retain_higher_order_param_invocation(
                scope.function_name.clone(),
                param_name.to_owned(),
            );
        } else {
            self.retain_higher_order_param_closure_invocation(
                scope.function_name.clone(),
                param_name.to_owned(),
                current_callable,
            );
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
            Expr::Path(path) => self
                .local_function_effects
                .get(path.as_label())
                .cloned()
                .or_else(|| self.source_function_effect_callable(path.as_label())),
            Expr::Call(call) => {
                if let Some(callable) = self.last_checked_closure_effect_callable.clone() {
                    return Some(callable);
                }
                let callee = expr_path_label(call.callee())?;
                if let Some(callable) = self.function_return_effect_callables.get(&callee).cloned()
                {
                    return Some(callable);
                }
                let callable = self.local_function_effects.get(&callee)?.clone();
                let arity = self.locals.get(&callee)?.function_arity()?;
                let positional_arg_count = call
                    .args()
                    .iter()
                    .filter(|arg| matches!(arg, CallArg::Positional(_)))
                    .count();
                let all_positional = call
                    .args()
                    .iter()
                    .all(|arg| matches!(arg, CallArg::Positional(_)));
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
            Expr::Call(_) => self.last_checked_curried_signature_call.clone(),
            Expr::Path(path) => self
                .local_curried_signature_calls
                .get(path.as_label())
                .cloned()
                .or_else(|| {
                    let name = path.as_label();
                    let signature = self.function_signature(name)?;
                    (signature.remaining_call_groups() > 0).then(|| CurriedSignatureCallValue {
                        function_name: name.to_owned(),
                        remaining_group_index: 0,
                        group_arg_offset: 0,
                        current_group_params: None,
                        pending_higher_order_args: Vec::new(),
                        resolved: None,
                    })
                }),
            _ => None,
        }
    }

    fn callable_signature_for_function_expr(
        &self,
        expr: &Expr,
        ty: &TypeKind,
    ) -> Option<SourceCallableSignature> {
        if !matches!(ty, TypeKind::Function { .. }) {
            return None;
        }
        let Expr::Path(path) = expr else {
            return None;
        };
        let name = path.as_label();
        self.local_callable_signatures
            .get(name)
            .cloned()
            .or_else(|| {
                self.function_signature(name)
                    .cloned()
                    .map(|signature| SourceCallableSignature::new(name, signature))
            })
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
        expected.accepts(actual)
    }

    fn collect_and_store_trait_catalog(&mut self, module: &HirModule) {
        let (catalog, diagnostics) = collect_trait_catalog(
            module,
            &mut |declaration_module, authored, generics, self_scope| {
                self.resolve_authored_type_in_module(
                    declaration_module,
                    authored,
                    generics,
                    self_scope,
                )
            },
        );
        self.trait_catalog = catalog;
        self.errors.extend(diagnostics);
    }

    fn trait_predicates_for_signature(
        &mut self,
        signature: &FnSignature,
        generics: &GenericTypeScope,
        self_scope: &SelfTypeScope,
    ) -> Vec<TraitPredicate> {
        let inputs = trait_predicate_inputs_for_signature(signature, generics, |authored, node| {
            self.resolve_authored_type_node(authored, node, generics, self_scope.clone())
        });
        self.trait_catalog.predicates_from_inputs(inputs)
    }

    fn active_trait_predicates(&self) -> Vec<TraitPredicate> {
        self.trait_predicate_stack
            .iter()
            .flat_map(|scope| scope.iter().cloned())
            .collect()
    }

    fn resolve_type_projection(&mut self, ty: TypeKind) -> TypeKind {
        match self
            .trait_catalog
            .resolve_type_projections(ty, &self.active_trait_predicates())
        {
            Ok(ty) => ty,
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

    fn path_has_known_resolution(&self, path: &str) -> bool {
        self.symbol_type(path).is_some()
            || self.function_type(path).is_some()
            || self.function_signature(path).is_some()
            || self.check_dotted_path_target(path).is_some()
            || builtin_path_type(path).is_some()
    }

    fn function_type(&self, name: &str) -> Option<&TypeKind> {
        self.resolve_project_callable(name)
            .and_then(|declaration| self.project_functions.get(declaration))
            .or_else(|| {
                self.global_functions
                    .get(name)
                    .or_else(|| self.env.function_type(name))
            })
    }

    fn function_signature(&self, name: &str) -> Option<&FunctionSignature> {
        self.resolve_project_callable(name)
            .and_then(|declaration| self.project_function_signatures.get(declaration))
            .or_else(|| {
                self.global_function_signatures
                    .get(name)
                    .or_else(|| self.env.function_signature(name))
            })
    }

    fn function_value_type(&mut self, name: &str) -> Option<TypeKind> {
        let signature = self.function_signature(name)?.clone();
        if self.uses_final_group_effect_timing(name) {
            signature.function_value_type_with_effects(self.function_effect_row(name))
        } else {
            signature.function_value_type()
        }
    }

    fn function_effect_row(&mut self, name: &str) -> EffectRow {
        if let Some(callable) = self
            .resolve_project_callable(name)
            .map(CallableId::project_function)
            && let Some(row) = self.effect_collector.inferred_effect_row(&callable)
        {
            return row;
        }
        let effects = self
            .global_function_effects
            .get(name)
            .map(|effects| effects.iter().map(String::as_str).collect::<Vec<_>>())
            .or_else(|| {
                self.env.function_effects(name).map(|effects| {
                    effects
                        .iter()
                        .map(crate::env::EffectCapability::as_str)
                        .collect()
                })
            });
        if let Some(effects) = effects.and_then(|effects| EffectSet::from_labels(effects).ok()) {
            return EffectRow::closed(effects);
        }
        if let Some(row) = self
            .effect_collector
            .inferred_effect_row(&function_callable_id(name))
        {
            return row;
        }
        EffectRow::unknown()
    }

    fn source_function_effect_callable(&self, name: &str) -> Option<CallableId> {
        self.resolve_project_callable(name)
            .map(CallableId::project_function)
            .or_else(|| self.effect_collector.registered_callable(name).cloned())
    }

    fn uses_final_group_effect_timing(&self, function_name: &str) -> bool {
        !self.global_function_signatures.contains_key(function_name)
            || self.ordinary_source_functions.contains(function_name)
    }

    fn nominal_field_type(&self, receiver: &TypeKind, field: &str) -> Option<TypeKind> {
        if let Some(ty) = self.project_nominal_shapes.field_type(receiver, field) {
            return Some(ty);
        }
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
        if self
            .speaker_reference_type(callee)
            .as_ref()
            .and_then(TypeKind::speaker_line_classification)
            .is_some()
        {
            return true;
        }
        callee.strip_suffix(".say").is_some_and(|receiver| {
            self.speaker_reference_type(receiver)
                .as_ref()
                .and_then(TypeKind::speaker_line_classification)
                .is_some()
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
