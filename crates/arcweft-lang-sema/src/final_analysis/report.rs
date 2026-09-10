//! Immutable accepted semantic report and publication transaction.

mod variants;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    item::{HirItemKind, HirVisibility},
    project::{
        HirProjectEvaluationTopology, HirRuntimeExecutableOwner, HirRuntimeReachabilityIdentity,
        HirRuntimeSemanticReachability,
    },
    scope::HirScopeKind,
    source_index::{
        HirDeclarationSourceRole, HirExprSourceRole, HirItemSourceRole, HirSourcePresence,
        HirSourceQuery, HirSourceSite,
    },
};
use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceSpan};
use thiserror::Error;

#[cfg(test)]
use std::sync::atomic::AtomicBool;

use super::PhysicalCandidateArgumentEvaluation;
use super::match_edges;
use super::{
    CallTargetFacts, CaptureId, CheckedBinding, CheckedCallableCatalog, CheckedExpression,
    CheckedExpressionCallCallee, CheckedExpressionExecutionPlan, CheckedExpressionResolution,
    CheckedImplicitCallable, CheckedImplicitCallableBody, CheckedImplicitCapture,
    CheckedImplicitParameterOccurrence, CheckedItem, CheckedPattern, CheckedPipe,
    CheckedPipeLeftOccurrence, CheckedRuntimeValueDisposition, CheckedStatement, CheckedTry,
    CheckedTryBoundary, CheckedTryCarrier, ExprId, FinalSemanticAnalysisControl,
    FinalSemanticAnalysisError, FinalSemanticAnalysisInput, FinalSemanticAnalysisWork,
    FinalSemanticProjectError, HirAnalysisProjectView, HirModule, HirModuleId, ItemId, LocalId,
    PatternId, ProjectSymbolTable, SemanticFactFamily, StmtId, TypeId, TypeKind,
    TypeResolutionReport,
    validation::{
        SemanticFactInventory, accepted_type_owners, collect_unique, collect_work,
        validate_bindings, validate_calls, validate_complete_inventory, validate_expressions,
        validate_items, validate_patterns, validate_physical_candidate_argument_evaluations,
        validate_statements, validate_types,
    },
};
use crate::callable::CheckedCallSite;
use crate::entry::CheckedEntryCatalog;
use crate::semantic_coordinate::AcceptedSemanticRootCatalog;

use super::nominal_schema::RuntimeNominalProjectionCatalog;
use super::nominal_semantic::{ProjectNominalSemanticCatalog, ProjectNominalSemanticDefinition};
use super::semantic_shapes::AcceptedSemanticShapeCatalog;

/// Immutable semantic analysis bound to one exact accepted HIR generation.
#[derive(Clone, Debug)]
pub struct FinalSemanticAnalysis {
    checked_callables: Arc<CheckedCallableCatalog>,
    accepted_roots: Arc<AcceptedSemanticRootCatalog>,
    checked_entries: CheckedEntryCatalog,
    project_nominals: ProjectNominalSemanticCatalog,
    checked_text_proxies: crate::checked_text_proxy::CheckedTextProxyCatalog,
    checked_fx_definitions: super::CheckedFxDefinitionCatalog,
    semantic_shapes: AcceptedSemanticShapeCatalog,
    runtime_nominals: RuntimeNominalProjectionCatalog,
    dialogue_lines: arcweft_lang_hir::project::AcceptedDialogueLineInventory,
    types: BTreeMap<TypeId, TypeKind>,
    type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
    locals: BTreeMap<LocalId, CheckedBinding>,
    captures: BTreeMap<CaptureId, CheckedBinding>,
    expressions: BTreeMap<ExprId, CheckedExpression>,
    patterns: BTreeMap<PatternId, CheckedPattern>,
    statements: BTreeMap<StmtId, CheckedStatement>,
    items: BTreeMap<ItemId, CheckedItem>,
    calls: BTreeMap<ExprId, CallTargetFacts>,
    pub(super) edge_facts: BTreeMap<
        ExprId,
        Result<super::CheckedExpressionEdgeFact, super::CheckedExpressionEdgeError>,
    >,
    diagnostics: Arc<[Diagnostic]>,
    #[cfg(test)]
    physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
    work: FinalSemanticAnalysisWork,
}

/// Final execution projection for one checked expression.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExpressionExecution {
    Structural {
        value: CheckedRuntimeValueDisposition,
    },
    Call {
        result: CheckedRuntimeValueDisposition,
        callee: CheckedCallExecutionCallee,
    },
}

/// Callee handling selected by one sealed ordinary call application.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCallExecutionCallee {
    Static,
    RuntimeReceiver,
}

/// Exact checked execution row for one explicit closure producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalAnalysisClosureExecution {
    id: crate::callable::CheckedClosureId,
    effects: crate::effect_row::EffectRow,
    suspension: super::CheckedSuspensionRole,
    control: super::CheckedExecutableControlRole,
}

impl FinalAnalysisClosureExecution {
    pub const fn id(&self) -> &crate::callable::CheckedClosureId {
        &self.id
    }

    pub const fn effects(&self) -> &crate::effect_row::EffectRow {
        &self.effects
    }

    pub const fn suspension(&self) -> super::CheckedSuspensionRole {
        self.suspension
    }

    pub const fn control(&self) -> super::CheckedExecutableControlRole {
        self.control
    }
}

/// Failure to project one checked expression into execution.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FinalAnalysisExecutionProjectionError {
    #[error("checked expression {owner:?} is absent from the final analysis")]
    MissingExpression { owner: ExprId },
    #[error("checked pattern {owner:?} is absent from the final analysis")]
    MissingPattern { owner: PatternId },
    #[error("checked statement {owner:?} is absent from the final analysis")]
    MissingStatement { owner: StmtId },
    #[error("checked HirCall expression {owner:?} has no call fact")]
    MissingCallFacts { owner: ExprId },
    #[error("checked HirCall expression {owner:?} has no selected call application")]
    UnselectedCall { owner: ExprId },
    #[error(
        "checked HirCall expression {owner:?} has a mismatched call fact site: expected {expected:?}, actual {actual:?}"
    )]
    CallSiteMismatch {
        owner: ExprId,
        expected: CheckedCallSite,
        actual: CheckedCallSite,
    },
    #[error("checked expression {owner:?} does not have the requested execution view")]
    WrongExpressionKind { owner: ExprId },
    #[error("runtime reachability has no executable-owner partition for {executable:?}")]
    MissingExecutablePartition {
        executable: HirRuntimeExecutableOwner,
    },
    #[error("checked closure expression {owner:?} has no exact source/execution authority")]
    MissingClosureExecution { owner: ExprId },
    #[error("checked constructor call {owner:?} has no exact instantiated variant authority")]
    InvalidVariantConstructor { owner: ExprId },
}

/// Borrowed final-analysis authority for execution.
///
/// This is the only cross-layer source for expression execution decisions and
/// the typed implicit-callable/pipe views. It borrows the immutable report so
/// consumers cannot rebuild execution facts by rescanning HIR.
pub struct FinalAnalysisExecutionProjection<'analysis> {
    analysis: &'analysis FinalSemanticAnalysis,
}

/// Runtime semantic-payload family selected for one expression in an exact
/// executable-owner partition.
///
/// This is a family tag rather than a second semantic payload. The final
/// checked expression remains the payload authority; the tag seals which
/// runtime-fact algebra member a downstream projection must publish.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExecutableRuntimeExpressionFactFamily {
    Structural,
    Consumed,
    Literal,
    Value,
    Select,
    NominalRecord,
    Variant,
    Call,
    PostfixCandidate,
    Await,
    Choice,
    Try,
    ImplicitCallable,
    Pipe,
    DialogueApplication,
    ContentApplication,
    Closure,
}

/// One final-sema-sealed expression row in an executable partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExecutableRuntimeExpressionFactOwner {
    owner: ExprId,
    family: CheckedExecutableRuntimeExpressionFactFamily,
    has_runtime_type: bool,
    children: Box<[ExprId]>,
}

impl CheckedExecutableRuntimeExpressionFactOwner {
    pub const fn owner(&self) -> ExprId {
        self.owner
    }

    pub const fn family(&self) -> CheckedExecutableRuntimeExpressionFactFamily {
        self.family
    }

    pub const fn has_runtime_type(&self) -> bool {
        self.has_runtime_type
    }

    pub const fn children(&self) -> &[ExprId] {
        &self.children
    }
}

/// Runtime semantic-payload family selected for one pattern.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExecutableRuntimePatternFactFamily {
    Structural,
    Literal,
    Entity,
    NominalRecord,
    Variant,
    TypedBinding,
}

/// One final-sema-sealed pattern row in an executable partition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedExecutableRuntimePatternFactOwner {
    owner: PatternId,
    family: CheckedExecutableRuntimePatternFactFamily,
}

impl CheckedExecutableRuntimePatternFactOwner {
    pub const fn owner(self) -> PatternId {
        self.owner
    }

    pub const fn family(self) -> CheckedExecutableRuntimePatternFactFamily {
        self.family
    }
}

/// Runtime semantic-payload family selected for one statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExecutableRuntimeStatementFactFamily {
    Structural,
    Assignment,
    Assertion,
    Defer,
    EvaluatedEffect,
    Iteration,
    ControlTransfer,
    Trigger,
    UnsafeAudit,
    Select,
    SourceLocale,
    Scope,
    Include,
    Suspension,
    Yield,
}

/// One final-sema-sealed statement row in an executable partition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedExecutableRuntimeStatementFactOwner {
    owner: StmtId,
    family: CheckedExecutableRuntimeStatementFactFamily,
}

impl CheckedExecutableRuntimeStatementFactOwner {
    pub const fn owner(self) -> StmtId {
        self.owner
    }

    pub const fn family(self) -> CheckedExecutableRuntimeStatementFactFamily {
        self.family
    }
}

/// Sealed final-sema inventory of every semantic owner belonging to one exact
/// HIR executable partition.
///
/// Nested closure bodies belong to independent rows. Private fields prevent a
/// downstream compiler or runtime-fact producer from substituting an
/// arbitrary same-module owner row or silently omitting a semantic family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExecutableRuntimeFactPartition {
    reachability: HirRuntimeReachabilityIdentity,
    executable: HirRuntimeExecutableOwner,
    expressions: Box<[CheckedExecutableRuntimeExpressionFactOwner]>,
    patterns: Box<[CheckedExecutableRuntimePatternFactOwner]>,
    statements: Box<[CheckedExecutableRuntimeStatementFactOwner]>,
    locals: Box<[LocalId]>,
    types: Box<[TypeId]>,
    captures: Box<[CaptureId]>,
}

impl CheckedExecutableRuntimeFactPartition {
    pub const fn reachability(&self) -> &HirRuntimeReachabilityIdentity {
        &self.reachability
    }

    pub const fn executable(&self) -> &HirRuntimeExecutableOwner {
        &self.executable
    }

    pub const fn expressions(&self) -> &[CheckedExecutableRuntimeExpressionFactOwner] {
        &self.expressions
    }

    pub const fn patterns(&self) -> &[CheckedExecutableRuntimePatternFactOwner] {
        &self.patterns
    }

    pub const fn statements(&self) -> &[CheckedExecutableRuntimeStatementFactOwner] {
        &self.statements
    }

    pub const fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    pub const fn types(&self) -> &[TypeId] {
        &self.types
    }

    pub const fn captures(&self) -> &[CaptureId] {
        &self.captures
    }
}

impl FinalAnalysisExecutionProjection<'_> {
    /// Projects one closure through its typed source-bound callable identity
    /// and completed effect/suspension/control row.
    pub fn closure_execution(
        &self,
        reachability: &HirRuntimeSemanticReachability<'_>,
        owner: ExprId,
    ) -> Result<FinalAnalysisClosureExecution, FinalAnalysisExecutionProjectionError> {
        let expression = self
            .analysis
            .expression(owner)
            .ok_or(FinalAnalysisExecutionProjectionError::MissingExpression { owner })?;
        if !matches!(
            expression.resolution(),
            CheckedExpressionResolution::Closure(_)
        ) {
            return Err(FinalAnalysisExecutionProjectionError::WrongExpressionKind { owner });
        }
        let module = reachability
            .project()
            .modules()
            .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module))
            .ok_or(FinalAnalysisExecutionProjectionError::MissingClosureExecution { owner })?;
        let source = module
            .source_site(
                module.provenance().source_identity(),
                HirSourceQuery::Expr {
                    owner,
                    role: HirExprSourceRole::Whole,
                },
            )
            .ok()
            .and_then(|lookup| match lookup.presence() {
                HirSourcePresence::Present(HirSourceSite::Span(span)) => Some(span),
                HirSourcePresence::Present(HirSourceSite::Insertion(_))
                | HirSourcePresence::AbsentOptional => None,
            })
            .ok_or(FinalAnalysisExecutionProjectionError::MissingClosureExecution { owner })?;
        let (id, execution) = self
            .analysis
            .checked_callables()
            .closure_identity_and_execution_at_source(source)
            .map_err(
                |_| FinalAnalysisExecutionProjectionError::MissingClosureExecution { owner },
            )?;
        Ok(FinalAnalysisClosureExecution {
            id: id.clone(),
            effects: execution.effects().clone(),
            suspension: execution.suspension(),
            control: execution.control(),
        })
    }

    /// Seals every checked semantic owner for one exact executable owner.
    /// Calls suppressed by their checked execution plan are tagged Consumed;
    /// nested closures retain only their value expression here while their
    /// bodies belong to the closure's independent HIR partition.
    pub fn runtime_fact_partition(
        &self,
        reachability: &HirRuntimeSemanticReachability<'_>,
        executable: &HirRuntimeExecutableOwner,
    ) -> Result<CheckedExecutableRuntimeFactPartition, FinalAnalysisExecutionProjectionError> {
        let owners = reachability.executable_owners(executable).ok_or_else(|| {
            FinalAnalysisExecutionProjectionError::MissingExecutablePartition {
                executable: executable.clone(),
            }
        })?;
        let module = |owner: ExprId| {
            reachability
                .project()
                .modules()
                .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module))
        };
        let mut expressions = Vec::new();
        let runtime_type_owners = owners.expression_type_owners().collect::<BTreeSet<_>>();
        for owner in owners.expressions() {
            let expression = self
                .analysis
                .expression(owner)
                .ok_or(FinalAnalysisExecutionProjectionError::MissingExpression { owner })?;
            let hir = module(owner)
                .and_then(|module| module.resolve_expr(owner).ok())
                .ok_or(FinalAnalysisExecutionProjectionError::MissingExpression { owner })?;
            let family = if expression.execution_plan().executes_as_runtime_call() {
                if !self.analysis.calls.contains_key(&owner) {
                    return Err(FinalAnalysisExecutionProjectionError::MissingCallFacts { owner });
                }
                CheckedExecutableRuntimeExpressionFactFamily::Call
            } else {
                match expression.resolution() {
                    CheckedExpressionResolution::Structural
                        if matches!(
                            hir.kind(),
                            arcweft_lang_hir::expr::HirExprKind::NumericBracketSequence(_)
                        ) =>
                    {
                        CheckedExecutableRuntimeExpressionFactFamily::Literal
                    }
                    CheckedExpressionResolution::Structural => {
                        CheckedExecutableRuntimeExpressionFactFamily::Structural
                    }
                    CheckedExpressionResolution::Literal(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Literal
                    }
                    CheckedExpressionResolution::Value(value)
                        if matches!(
                            value,
                            super::CheckedValueResolution::Local(_)
                                | super::CheckedValueResolution::Registered(_)
                                | super::CheckedValueResolution::Constant(_)
                        ) || matches!(
                            (value, hir.kind()),
                            (
                                super::CheckedValueResolution::ProjectItem(_),
                                arcweft_lang_hir::expr::HirExprKind::EntityReference(_)
                            )
                        ) =>
                    {
                        CheckedExecutableRuntimeExpressionFactFamily::Value
                    }
                    CheckedExpressionResolution::DialogueLineReference(_)
                    | CheckedExpressionResolution::StageLook(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Value
                    }
                    CheckedExpressionResolution::Value(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Consumed
                    }
                    CheckedExpressionResolution::Select(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Select
                    }
                    CheckedExpressionResolution::Nominal(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::NominalRecord
                    }
                    CheckedExpressionResolution::Variant(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Variant
                    }
                    CheckedExpressionResolution::Await(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Await
                    }
                    CheckedExpressionResolution::Choice(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Choice
                    }
                    CheckedExpressionResolution::Try(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Try
                    }
                    CheckedExpressionResolution::ImplicitCallable(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::ImplicitCallable
                    }
                    CheckedExpressionResolution::Closure(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Closure
                    }
                    CheckedExpressionResolution::Pipe(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Pipe
                    }
                    CheckedExpressionResolution::DialogueApplication { .. } => {
                        CheckedExecutableRuntimeExpressionFactFamily::DialogueApplication
                    }
                    CheckedExpressionResolution::ContentApplication(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::ContentApplication
                    }
                    CheckedExpressionResolution::PostfixBracket(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::PostfixCandidate
                    }
                    CheckedExpressionResolution::CompileTimeEnum(_)
                    | CheckedExpressionResolution::Effect(_)
                    | CheckedExpressionResolution::Call
                    | CheckedExpressionResolution::ImplicitParameter(_)
                    | CheckedExpressionResolution::PipeLeft(_)
                    | CheckedExpressionResolution::ViewCall(_)
                    | CheckedExpressionResolution::ViewFxApplication(_)
                    | CheckedExpressionResolution::StyleValue(_)
                    | CheckedExpressionResolution::CompileTimeCallee(_)
                    | CheckedExpressionResolution::TypeValue(_)
                    | CheckedExpressionResolution::CompileTimeScalar(_)
                    | CheckedExpressionResolution::DialogueLineCoordinate(_)
                    | CheckedExpressionResolution::DialogueTextKeyCoordinate(_)
                    | CheckedExpressionResolution::CharacterDialogueFactory(_)
                    | CheckedExpressionResolution::CharacterDialogueReconfigure(_) => {
                        CheckedExecutableRuntimeExpressionFactFamily::Consumed
                    }
                }
            };
            expressions.push(CheckedExecutableRuntimeExpressionFactOwner {
                owner,
                family,
                has_runtime_type: runtime_type_owners.contains(&owner),
                children: owners.expression_children(owner).into(),
            });
        }

        let patterns = owners
            .patterns()
            .map(|owner| {
                let pattern = self
                    .analysis
                    .pattern(owner)
                    .ok_or(FinalAnalysisExecutionProjectionError::MissingPattern { owner })?;
                let family = match pattern.resolution() {
                    super::CheckedPatternResolution::Structural => {
                        CheckedExecutableRuntimePatternFactFamily::Structural
                    }
                    super::CheckedPatternResolution::Literal(_) => {
                        CheckedExecutableRuntimePatternFactFamily::Literal
                    }
                    super::CheckedPatternResolution::Entity(_) => {
                        CheckedExecutableRuntimePatternFactFamily::Entity
                    }
                    super::CheckedPatternResolution::Record(_) => {
                        CheckedExecutableRuntimePatternFactFamily::NominalRecord
                    }
                    super::CheckedPatternResolution::Variant(_) => {
                        CheckedExecutableRuntimePatternFactFamily::Variant
                    }
                    super::CheckedPatternResolution::TypedBinding(_) => {
                        CheckedExecutableRuntimePatternFactFamily::TypedBinding
                    }
                };
                Ok(CheckedExecutableRuntimePatternFactOwner { owner, family })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let statements = owners
            .statements()
            .map(|owner| {
                let statement = self
                    .analysis
                    .statement(owner)
                    .ok_or(FinalAnalysisExecutionProjectionError::MissingStatement { owner })?;
                let family = match statement.payload() {
                    super::CheckedStatementPayload::Structural => {
                        CheckedExecutableRuntimeStatementFactFamily::Structural
                    }
                    super::CheckedStatementPayload::Assignment(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::Assignment
                    }
                    super::CheckedStatementPayload::Assertion(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::Assertion
                    }
                    super::CheckedStatementPayload::Defer(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::Defer
                    }
                    super::CheckedStatementPayload::EvaluatedEffect(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::EvaluatedEffect
                    }
                    super::CheckedStatementPayload::Iteration(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::Iteration
                    }
                    super::CheckedStatementPayload::ControlTransfer(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::ControlTransfer
                    }
                    super::CheckedStatementPayload::Trigger(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::Trigger
                    }
                    super::CheckedStatementPayload::UnsafeAudit(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::UnsafeAudit
                    }
                    super::CheckedStatementPayload::Select(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::Select
                    }
                    super::CheckedStatementPayload::SourceLocale(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::SourceLocale
                    }
                    super::CheckedStatementPayload::Scope(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::Scope
                    }
                    super::CheckedStatementPayload::Include(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::Include
                    }
                    super::CheckedStatementPayload::Suspension(_) => {
                        CheckedExecutableRuntimeStatementFactFamily::Suspension
                    }
                    super::CheckedStatementPayload::Yield => {
                        CheckedExecutableRuntimeStatementFactFamily::Yield
                    }
                };
                Ok(CheckedExecutableRuntimeStatementFactOwner { owner, family })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CheckedExecutableRuntimeFactPartition {
            reachability: reachability.identity().clone(),
            executable: executable.clone(),
            expressions: expressions.into_boxed_slice(),
            patterns: patterns.into_boxed_slice(),
            statements: statements.into_boxed_slice(),
            locals: owners.locals().collect(),
            types: owners.types().collect(),
            captures: owners.captures().collect(),
        })
    }

    /// Returns the exact execution projection of one checked expression.
    pub fn expression(
        &self,
        owner: ExprId,
    ) -> Result<CheckedExpressionExecution, FinalAnalysisExecutionProjectionError> {
        let expression = self
            .analysis
            .expression(owner)
            .ok_or(FinalAnalysisExecutionProjectionError::MissingExpression { owner })?;
        Ok(match expression.execution_plan() {
            CheckedExpressionExecutionPlan::Structural { value, .. } => {
                CheckedExpressionExecution::Structural { value: *value }
            }
            CheckedExpressionExecutionPlan::Call { result, callee, .. } => {
                CheckedExpressionExecution::Call {
                    result: *result,
                    callee: match callee {
                        CheckedExpressionCallCallee::Static => CheckedCallExecutionCallee::Static,
                        CheckedExpressionCallCallee::RuntimeReceiver => {
                            CheckedCallExecutionCallee::RuntimeReceiver
                        }
                    },
                }
            }
        })
    }

    /// Borrows the accepted implicit-callable execution view for `owner`.
    pub fn implicit_callable(
        &self,
        owner: ExprId,
    ) -> Result<FinalAnalysisImplicitCallableView<'_>, FinalAnalysisExecutionProjectionError> {
        let expression = self
            .analysis
            .expression(owner)
            .ok_or(FinalAnalysisExecutionProjectionError::MissingExpression { owner })?;
        let CheckedExpressionResolution::ImplicitCallable(callable) = expression.resolution()
        else {
            return Err(FinalAnalysisExecutionProjectionError::WrongExpressionKind { owner });
        };
        Ok(FinalAnalysisImplicitCallableView { callable })
    }

    /// Borrows the accepted once-only pipe execution view for `owner`.
    pub fn pipe(
        &self,
        owner: ExprId,
    ) -> Result<FinalAnalysisPipeView<'_>, FinalAnalysisExecutionProjectionError> {
        let expression = self
            .analysis
            .expression(owner)
            .ok_or(FinalAnalysisExecutionProjectionError::MissingExpression { owner })?;
        let CheckedExpressionResolution::Pipe(pipe) = expression.resolution() else {
            return Err(FinalAnalysisExecutionProjectionError::WrongExpressionKind { owner });
        };
        Ok(FinalAnalysisPipeView { pipe })
    }

    /// Borrows the exact checked Try execution view for `owner`.
    pub fn try_expression(
        &self,
        owner: ExprId,
    ) -> Result<FinalAnalysisTryView<'_>, FinalAnalysisExecutionProjectionError> {
        let expression = self
            .analysis
            .expression(owner)
            .ok_or(FinalAnalysisExecutionProjectionError::MissingExpression { owner })?;
        let CheckedExpressionResolution::Try(tried) = expression.resolution() else {
            return Err(FinalAnalysisExecutionProjectionError::WrongExpressionKind { owner });
        };
        Ok(FinalAnalysisTryView { tried })
    }
}

/// Typed execution view of one checked implicit callable.
pub struct FinalAnalysisImplicitCallableView<'analysis> {
    callable: &'analysis CheckedImplicitCallable,
}

impl FinalAnalysisImplicitCallableView<'_> {
    pub fn parameter(&self) -> &TypeKind {
        self.callable.parameter()
    }

    pub fn result(&self) -> &TypeKind {
        self.callable.result()
    }

    pub fn placeholders(&self) -> impl ExactSizeIterator<Item = ExprId> + '_ {
        self.callable
            .parameter_occurrences()
            .iter()
            .map(CheckedImplicitParameterOccurrence::lookup_expression)
    }

    pub fn captures(&self) -> impl ExactSizeIterator<Item = LocalId> + '_ {
        self.callable
            .captures()
            .iter()
            .map(CheckedImplicitCapture::lookup_local)
    }

    pub fn body(&self) -> FinalAnalysisImplicitCallableBody<'_> {
        match self.callable.body() {
            CheckedImplicitCallableBody::Plain(resolution) => {
                FinalAnalysisImplicitCallableBody::Plain(resolution)
            }
            CheckedImplicitCallableBody::Try(tried) => {
                FinalAnalysisImplicitCallableBody::Try(FinalAnalysisTryView { tried })
            }
            CheckedImplicitCallableBody::Pipe(pipe) => {
                FinalAnalysisImplicitCallableBody::Pipe(FinalAnalysisPipeView { pipe })
            }
        }
    }
}

/// Exhaustive execution projection of an implicit callable body.
#[derive(Clone, Copy)]
pub enum FinalAnalysisImplicitCallableBody<'analysis> {
    Plain(&'analysis CheckedExpressionResolution),
    Try(FinalAnalysisTryView<'analysis>),
    Pipe(FinalAnalysisPipeView<'analysis>),
}

/// Typed execution view of one checked Try expression.
#[derive(Clone, Copy)]
pub struct FinalAnalysisTryView<'analysis> {
    tried: &'analysis CheckedTry,
}

impl FinalAnalysisTryView<'_> {
    pub const fn operand(&self) -> ExprId {
        self.tried.operand().lookup_owner()
    }

    pub const fn operand_type(&self) -> &TypeKind {
        self.tried.operand().value_type()
    }

    pub const fn carrier(&self) -> &CheckedTryCarrier {
        self.tried.carrier()
    }

    pub const fn boundary(&self) -> &CheckedTryBoundary {
        self.tried.boundary()
    }
}

/// Typed execution view of one checked once-only pipe.
#[derive(Clone, Copy)]
pub struct FinalAnalysisPipeView<'analysis> {
    pipe: &'analysis CheckedPipe,
}

impl FinalAnalysisPipeView<'_> {
    pub const fn left(&self) -> ExprId {
        self.pipe.lookup_left()
    }

    pub const fn right(&self) -> ExprId {
        self.pipe.lookup_right()
    }

    pub fn placeholders(&self) -> impl ExactSizeIterator<Item = ExprId> + '_ {
        self.pipe
            .occurrences()
            .iter()
            .map(CheckedPipeLeftOccurrence::lookup_expression)
    }
}

/// Complete, unpublished semantic generation awaiting the consuming Entry and
/// runtime-nominal seal. This owner is deliberately non-Clone.
pub(crate) struct FinalSemanticAnalysisDraft {
    pub(super) checked_callables: Arc<CheckedCallableCatalog>,
    pub(super) accepted_roots: Arc<AcceptedSemanticRootCatalog>,
    pub(super) types: BTreeMap<TypeId, TypeKind>,
    pub(super) type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
    pub(super) locals: BTreeMap<LocalId, CheckedBinding>,
    pub(super) captures: BTreeMap<CaptureId, CheckedBinding>,
    pub(super) expressions: BTreeMap<ExprId, super::PreparedExpressionFact>,
    pub(super) patterns: BTreeMap<PatternId, super::PreparedPatternFact>,
    pub(super) statements: BTreeMap<StmtId, super::PreparedStatementPayload>,
    pub(super) items: BTreeMap<ItemId, CheckedItem>,
    pub(super) calls: BTreeMap<ExprId, CallTargetFacts>,
    pub(super) callable_joins: super::match_edges::PreparedCallableJoins,
    pub(super) selected_expressions: super::match_edges::CheckedSelectedExpressionGraph,
    pub(super) structural_edges: super::match_edges::CheckedStructuralEdgeDraft,
    pub(super) ingress: super::PreparedExecutableIngressSeal,
    pub(super) text_proxies: crate::checked_text_proxy::PreparedCheckedTextProxyCatalog,
    pub(super) fx_definitions: super::CheckedFxDefinitionCatalog,
    pub(super) physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
    pub(super) executable_suspensions:
        BTreeMap<ExprId, super::statement_effects::PreparedExecutableSuspensionRow>,
}

/// Disjoint moved draft state used while the nominal context borrows only the
/// accepted type map.
pub(crate) struct FinalSemanticAnalysisDraftParts {
    pub(super) checked_callables: Arc<CheckedCallableCatalog>,
    pub(super) accepted_roots: Arc<AcceptedSemanticRootCatalog>,
    pub(super) types: BTreeMap<TypeId, TypeKind>,
    pub(super) type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
    pub(super) locals: BTreeMap<LocalId, CheckedBinding>,
    pub(super) captures: BTreeMap<CaptureId, CheckedBinding>,
    pub(super) expressions: BTreeMap<ExprId, super::PreparedExpressionFact>,
    pub(super) patterns: BTreeMap<PatternId, super::PreparedPatternFact>,
    pub(super) statements: BTreeMap<StmtId, super::PreparedStatementPayload>,
    pub(super) items: BTreeMap<ItemId, CheckedItem>,
    pub(super) calls: BTreeMap<ExprId, CallTargetFacts>,
    pub(super) callable_joins: super::match_edges::PreparedCallableJoins,
    pub(super) selected_expressions: super::match_edges::CheckedSelectedExpressionGraph,
    pub(super) structural_edges: super::match_edges::CheckedStructuralEdgeDraft,
    pub(super) ingress: super::PreparedExecutableIngressSeal,
    pub(super) text_proxies: crate::checked_text_proxy::PreparedCheckedTextProxyCatalog,
    pub(super) fx_definitions: super::CheckedFxDefinitionCatalog,
    pub(super) physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
    pub(super) executable_suspensions:
        BTreeMap<ExprId, super::statement_effects::PreparedExecutableSuspensionRow>,
}

impl FinalSemanticAnalysisDraft {
    pub(crate) fn into_parts(self) -> FinalSemanticAnalysisDraftParts {
        let Self {
            checked_callables,
            accepted_roots,
            types,
            type_resolutions,
            locals,
            captures,
            expressions,
            patterns,
            statements,
            items,
            calls,
            callable_joins,
            selected_expressions,
            structural_edges,
            ingress,
            text_proxies,
            fx_definitions,
            physical_candidate_argument_evaluations,
            executable_suspensions,
        } = self;
        FinalSemanticAnalysisDraftParts {
            checked_callables,
            accepted_roots,
            types,
            type_resolutions,
            locals,
            captures,
            expressions,
            patterns,
            statements,
            items,
            calls,
            callable_joins,
            selected_expressions,
            structural_edges,
            ingress,
            text_proxies,
            fx_definitions,
            physical_candidate_argument_evaluations,
            executable_suspensions,
        }
    }
}

/// Post-Entry unpublished state. The full executable ingress worklist has
/// already been split and consumed; only the affine statement half remains.
pub(crate) struct FinalSemanticAnalysisPostEntryDraft {
    pub(super) checked_callables: Arc<CheckedCallableCatalog>,
    pub(super) accepted_roots: Arc<AcceptedSemanticRootCatalog>,
    pub(super) types: BTreeMap<TypeId, TypeKind>,
    pub(super) type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
    pub(super) locals: BTreeMap<LocalId, CheckedBinding>,
    pub(super) captures: BTreeMap<CaptureId, CheckedBinding>,
    pub(super) expressions: BTreeMap<ExprId, super::PreparedExpressionFact>,
    pub(super) patterns: BTreeMap<PatternId, super::PreparedPatternFact>,
    pub(super) statements: BTreeMap<StmtId, super::PreparedStatementPayload>,
    pub(super) items: BTreeMap<ItemId, CheckedItem>,
    pub(super) calls: BTreeMap<ExprId, CallTargetFacts>,
    pub(super) callable_joins: super::match_edges::PreparedCallableJoins,
    pub(super) selected_expressions: super::match_edges::CheckedSelectedExpressionGraph,
    pub(super) structural_edges: super::match_edges::CheckedStructuralEdgeDraft,
    pub(super) statement_ingress: super::PreparedStatementIngressSeal,
    pub(super) text_proxies: crate::checked_text_proxy::PreparedCheckedTextProxyCatalog,
    pub(super) fx_definitions: super::CheckedFxDefinitionCatalog,
    pub(super) physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
    pub(super) executable_suspensions:
        BTreeMap<ExprId, super::statement_effects::PreparedExecutableSuspensionRow>,
}

impl FinalSemanticAnalysisDraftParts {
    pub(crate) fn into_post_entry(
        self,
    ) -> (
        super::PreparedEntryIngressSeal,
        FinalSemanticAnalysisPostEntryDraft,
    ) {
        let FinalSemanticAnalysisDraftParts {
            checked_callables,
            accepted_roots,
            types,
            type_resolutions,
            locals,
            captures,
            expressions,
            patterns,
            statements,
            items,
            calls,
            callable_joins,
            selected_expressions,
            structural_edges,
            ingress,
            text_proxies,
            fx_definitions,
            physical_candidate_argument_evaluations,
            executable_suspensions,
        } = self;
        let (entry_ingress, statement_ingress) = ingress.into_phase_seals();
        (
            entry_ingress,
            FinalSemanticAnalysisPostEntryDraft {
                checked_callables,
                accepted_roots,
                types,
                type_resolutions,
                locals,
                captures,
                expressions,
                patterns,
                statements,
                items,
                calls,
                callable_joins,
                selected_expressions,
                structural_edges,
                statement_ingress,
                text_proxies,
                fx_definitions,
                physical_candidate_argument_evaluations,
                executable_suspensions,
            },
        )
    }
}

impl FinalSemanticAnalysisPostEntryDraft {
    pub(crate) fn seal(
        self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        checked_entries: CheckedEntryCatalog,
        project_nominals: ProjectNominalSemanticCatalog,
        semantic_shapes: AcceptedSemanticShapeCatalog,
        runtime_nominals: RuntimeNominalProjectionCatalog,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
        let Self {
            checked_callables,
            accepted_roots,
            types,
            type_resolutions,
            locals,
            captures,
            expressions: prepared_expressions,
            patterns: prepared_patterns,
            statements: prepared_statements,
            items,
            calls,
            callable_joins,
            selected_expressions,
            structural_edges,
            statement_ingress,
            text_proxies,
            fx_definitions,
            physical_candidate_argument_evaluations,
            executable_suspensions,
        } = self;
        control.check()?;
        let expressions = collect_sealed_expressions(prepared_expressions)?;
        fx_definitions
            .validate_applications(&expressions)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let checked_fx_definitions = fx_definitions;
        let patterns = collect_sealed_patterns(prepared_patterns)?;
        let evaluation_topology = Arc::clone(accepted_roots.topology());
        let modules = project_generation_modules(project);
        let dialogue_lines = selected_expressions.dialogue_lines().clone();
        let completed = {
            let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
                accepted_roots.as_ref(),
                &structural_edges,
            );
            let payloads = super::statement_seal::CheckedStatementSeal::new(
                prepared_statements,
                statement_ingress,
                &locals,
                checked_callables.as_ref(),
                &coordinates,
                project,
            );
            super::statement_effects::seal_statement_effects(
                super::statement_effects::StatementEffectSealInput {
                    modules: &modules,
                    topology: evaluation_topology.as_ref(),
                    selected: &selected_expressions,
                    calls: &calls,
                    callables: checked_callables.as_ref(),
                    expressions,
                    payloads,
                    control,
                },
            )?
        };
        let (expressions, statements) = completed.into_parts();
        for expression in expressions.values() {
            if let super::CheckedExpressionResolution::ImplicitCallable(callable) =
                expression.resolution()
            {
                callable
                    .validate_execution_uses(&expressions)
                    .map_err(|violation| FinalSemanticAnalysisError::CaptureAuthority {
                        violation,
                    })?;
            }
        }
        validate_checked_entry_references(&expressions, &checked_entries)?;

        let type_owners = if type_resolutions.is_empty() {
            None
        } else {
            Some(accepted_type_owners(
                &modules,
                &expressions,
                &calls,
                &selected_expressions,
            )?)
        };
        validate_type_resolution_reports(
            &modules,
            type_owners.as_ref(),
            &types,
            &type_resolutions,
        )?;
        control.check()?;

        let inventory = SemanticFactInventory {
            types: &types,
            locals: &locals,
            captures: &captures,
            expressions: &expressions,
            patterns: &patterns,
            statements: &statements,
            items: &items,
            calls: &calls,
        };
        validate_complete_inventory(
            evaluation_topology.as_ref(),
            &modules,
            &selected_expressions,
            inventory,
            &type_resolutions,
        )?;
        for expression in expressions.values() {
            if let super::CheckedExpressionResolution::Closure(closure) = expression.resolution() {
                closure.validate_selection(&selected_expressions)?;
            }
        }
        control.check()?;
        validate_types(&modules, &types)?;
        control.check()?;
        validate_bindings(&modules, &locals, &captures)?;
        control.check()?;
        let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
            accepted_roots.as_ref(),
            &structural_edges,
        );
        validate_expressions(
            symbols,
            &evaluation_topology,
            &modules,
            &dialogue_lines,
            &expressions,
            &calls,
            &structural_edges,
            &coordinates,
        )?;
        control.check()?;
        validate_patterns(symbols, &modules, &types, &patterns)?;
        control.check()?;
        validate_statements(&modules, &locals, &statements, &calls)?;
        control.check()?;
        validate_items(&modules, &items)?;
        control.check()?;
        validate_calls(symbols, &modules, &expressions, &calls)?;
        validate_physical_candidate_argument_evaluations(
            &modules,
            &physical_candidate_argument_evaluations,
        )?;
        let work = collect_work(inventory)?;
        let (checked_text_proxies, text_proxy_diagnostics) = text_proxies
            .seal(crate::checked_text_proxy::TextProxyFinalSealAuthority {
                project,
                symbols,
                project_nominals: &project_nominals,
                types: &types,
                type_resolutions: &type_resolutions,
                expressions: &expressions,
                control,
            })?
            .into_parts();
        let mut diagnostics = collect_final_diagnostics(&modules, &types, &expressions, &items)?;
        diagnostics.extend(text_proxy_diagnostics);
        let (edge_facts, unconsumed_callable_joins) =
            structural_edges.into_final_facts(&calls, callable_joins);
        if !unconsumed_callable_joins.is_empty() {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        control.check()?;
        let mut analysis = FinalSemanticAnalysis {
            checked_callables,
            accepted_roots,
            checked_entries,
            project_nominals,
            checked_text_proxies,
            checked_fx_definitions,
            semantic_shapes,
            runtime_nominals,
            dialogue_lines,
            types,
            type_resolutions,
            locals,
            captures,
            expressions,
            patterns,
            statements,
            items,
            calls,
            edge_facts,
            diagnostics: diagnostics.into(),
            #[cfg(test)]
            physical_candidate_argument_evaluations,
            work,
        };
        seal_checked_callable_interfaces(
            &mut analysis,
            project,
            symbols,
            executable_suspensions,
            control,
        )?;
        Ok(analysis)
    }
}

fn seal_checked_callable_interfaces(
    analysis: &mut FinalSemanticAnalysis,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    executable_suspensions: BTreeMap<
        ExprId,
        super::statement_effects::PreparedExecutableSuspensionRow,
    >,
    control: FinalSemanticAnalysisControl<'_>,
) -> Result<(), FinalSemanticAnalysisError> {
    use crate::{
        callable::{
            CallableAttachedContentExecution, CallableAttachedContentPolicy, CallableCandidateId,
            CallableParameterPresence, CheckedAttachedContentDefault,
            CheckedCallableAttachedContentParameter,
        },
        effect_row::EffectRow,
        semantic_coordinate::{SemanticCoordinateIndex, StableCheckedValueCoordinate},
    };
    use arcweft_lang_hir::item::{HirAttachedContentPresence, HirAttachedContentRole};

    let coordinates = SemanticCoordinateIndex::new(analysis.accepted_root_catalog(), analysis);
    let mut rows = BTreeMap::new();
    for facts in analysis.checked_callables().records() {
        control.check()?;
        let id = facts.id().clone();
        let row = match (facts.record().id(), facts.signature().attached_content()) {
            (CallableCandidateId::Project(declaration), Some(parameter))
                if parameter.execution() == CallableAttachedContentExecution::RuntimeContent =>
            {
                let symbol = symbols
                    .callable(declaration)
                    .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
                let module = project
                    .modules()
                    .find_map(|(_, module)| {
                        (module.module_id() == symbol.source_item().module())
                            .then_some(module.as_ref())
                    })
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                let hir = crate::callable::project_callable_attached_content(module, symbol)
                    .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let CallableAttachedContentPolicy::Declared(admission) = parameter.policy() else {
                    return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                };
                let hir_admission = match hir.role() {
                    HirAttachedContentRole::Inline => crate::callable::CheckedContentRole::Inline,
                    HirAttachedContentRole::Rich => crate::callable::CheckedContentRole::Rich,
                    HirAttachedContentRole::Dialogue => {
                        crate::callable::CheckedContentRole::Dialogue
                    }
                };
                let hir_presence = match hir.presence() {
                    HirAttachedContentPresence::Required => CallableParameterPresence::Required,
                    HirAttachedContentPresence::Optional => CallableParameterPresence::Optional,
                    HirAttachedContentPresence::Defaulted { .. } => {
                        CallableParameterPresence::Defaulted
                    }
                };
                if hir_admission != admission || hir_presence != parameter.presence() {
                    return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                }
                let result = facts
                    .signature()
                    .value_type()
                    .cloned()
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let binding_type = match parameter.presence() {
                    CallableParameterPresence::Optional => {
                        TypeKind::Option(Box::new(result.clone()))
                    }
                    CallableParameterPresence::Required | CallableParameterPresence::Defaulted => {
                        result.clone()
                    }
                };
                let abi_type = match parameter.presence() {
                    CallableParameterPresence::Required => result.clone(),
                    CallableParameterPresence::Optional | CallableParameterPresence::Defaulted => {
                        TypeKind::Option(Box::new(result.clone()))
                    }
                };
                let binding = analysis.local(hir.binding()).ok_or(
                    FinalSemanticAnalysisError::LocalTypeUnavailable {
                        owner: hir.binding(),
                    },
                )?;
                if binding.ty() != &binding_type {
                    return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                }
                let binding_coordinate = coordinates
                    .binding(hir.binding())
                    .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let abi_position = u32::try_from(
                    facts
                        .signature()
                        .group(parameter.group())
                        .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
                        .parameters()
                        .len(),
                )
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                let default = match hir.presence() {
                    HirAttachedContentPresence::Defaulted { value } => {
                        let checked = analysis.expression(value).ok_or(
                            FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: value },
                        )?;
                        if checked.value_type() != Some(&result) {
                            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner: value,
                            });
                        }
                        let coordinate = StableCheckedValueCoordinate::Expression(
                            coordinates
                                .expression(value)
                                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?,
                        );
                        let expression = super::semantic_transcript::checked_attached_content_default_expression_digest(
                            analysis,
                            project,
                            value,
                            control,
                        )
                        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                        let execution = executable_suspensions
                            .get(&value)
                            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                        let captures = checked_attached_content_default_captures(
                            analysis,
                            module,
                            symbol,
                            value,
                            execution.expressions(),
                            &coordinates,
                        )?;
                        Some(CheckedAttachedContentDefault::new(
                            value,
                            coordinate,
                            result.semantic_identity_digest()?,
                            EffectRow::closed(checked.effects().clone()),
                            execution.suspension(),
                            execution.control(),
                            expression,
                            captures,
                        ))
                    }
                    HirAttachedContentPresence::Required | HirAttachedContentPresence::Optional => {
                        None
                    }
                };
                Some(CheckedCallableAttachedContentParameter::new(
                    parameter.group(),
                    hir.binding(),
                    binding_coordinate,
                    admission,
                    parameter.presence(),
                    abi_position,
                    binding_type,
                    abi_type,
                    default,
                ))
            }
            (_, Some(parameter))
                if parameter.execution() == CallableAttachedContentExecution::RuntimeContent =>
            {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
            _ => None,
        };
        if rows.insert(id, row).is_some() {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
    }
    let catalog = Arc::get_mut(&mut analysis.checked_callables)
        .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
    catalog
        .seal_interfaces(rows)
        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)
}

fn checked_attached_content_default_captures(
    analysis: &FinalSemanticAnalysis,
    module: &arcweft_lang_hir::module::HirModule,
    symbol: &arcweft_lang_hir::symbol::CallableSymbol,
    root: ExprId,
    executed_expressions: &[ExprId],
    coordinates: &crate::semantic_coordinate::SemanticCoordinateIndex<'_, '_>,
) -> Result<Box<[crate::callable::CheckedAttachedContentDefaultCapture]>, FinalSemanticAnalysisError>
{
    use arcweft_lang_hir::item::HirItemKind;

    let item = module
        .resolve_item(symbol.source_item())
        .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?;
    let HirItemKind::Function(function) = item.kind() else {
        return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
    };
    let root_path = coordinates
        .expression_evidence(root)
        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
        .into_coordinate();
    if !executed_expressions.contains(&root)
        || executed_expressions
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
    }
    let mut parameters = BTreeMap::new();
    for (group_index, group) in function.parameter_groups().iter().enumerate() {
        let group_coordinate = crate::callable::CallableGroupIndex::try_from_usize(group_index)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        for (parameter_index, parameter) in group.parameters().iter().enumerate() {
            let parameter_coordinate =
                crate::callable::CallableParameterIndex::try_from_usize(parameter_index)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let coordinate = crate::callable::CallableParameterCoordinate::new(
                group_coordinate,
                parameter_coordinate,
            );
            for local in parameter.locals() {
                if parameters.insert(*local, (coordinate, parameter)).is_some() {
                    return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                }
            }
        }
    }

    let mut used = BTreeMap::<
        crate::callable::CallableParameterCoordinate,
        Vec<crate::callable::CheckedAttachedContentDefaultCaptureLocal>,
    >::new();
    let mut captured = BTreeSet::new();
    for &owner in executed_expressions {
        let checked = analysis
            .expression(owner)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        if let Some(local) = checked.execution_local_use() {
            let local_ty = analysis
                .local(local)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
            if checked.value_type() != Some(local_ty.ty()) {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
            let origin = coordinates
                .binding(local)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            if !origin.path().is_at_or_below(&root_path) && captured.insert(local) {
                let (parameter, _) = parameters
                    .get(&local)
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                used.entry(*parameter).or_default().push(
                    crate::callable::CheckedAttachedContentDefaultCaptureLocal::new(
                        local,
                        origin,
                        local_ty.ty().clone(),
                    ),
                );
            }
        }
    }

    let mut captures = Vec::new();
    for (group_index, group) in function.parameter_groups().iter().enumerate() {
        let group_coordinate = crate::callable::CallableGroupIndex::try_from_usize(group_index)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        for (parameter_index, parameter) in group.parameters().iter().enumerate() {
            let parameter_coordinate =
                crate::callable::CallableParameterIndex::try_from_usize(parameter_index)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let coordinate = crate::callable::CallableParameterCoordinate::new(
                group_coordinate,
                parameter_coordinate,
            );
            let Some(mut used_locals) = used.remove(&coordinate) else {
                continue;
            };
            used_locals.sort_by(|left, right| left.origin().cmp(right.origin()));
            let pattern = analysis
                .pattern(parameter.pattern())
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            let pattern_digest =
                super::semantic_transcript::checked_attached_content_default_pattern_digest(
                    analysis,
                    module,
                    parameter.pattern(),
                )
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            let binding_evidence = parameter
                .locals()
                .iter()
                .map(|local| {
                    let checked = analysis.local(*local).ok_or(
                        FinalSemanticAnalysisError::LocalTypeUnavailable { owner: *local },
                    )?;
                    let origin = coordinates
                        .binding(*local)
                        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                    Ok(
                        crate::callable::CheckedAttachedContentDefaultCaptureLocal::new(
                            *local,
                            origin,
                            checked.ty().clone(),
                        ),
                    )
                })
                .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?
                .into_boxed_slice();
            captures.push(crate::callable::CheckedAttachedContentDefaultCapture::new(
                coordinate,
                parameter.pattern(),
                pattern_digest,
                parameter.locals().to_vec().into_boxed_slice(),
                binding_evidence,
                used_locals.into_boxed_slice(),
                pattern.ty().clone(),
            ));
        }
    }
    if !used.is_empty() {
        return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
    }
    Ok(captures.into_boxed_slice())
}

fn validate_checked_entry_references(
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    entries: &CheckedEntryCatalog,
) -> Result<(), FinalSemanticAnalysisError> {
    for (owner, expression) in expressions {
        let super::CheckedExpressionResolution::Value(super::CheckedValueResolution::Entry(
            reference,
        )) = expression.resolution()
        else {
            continue;
        };
        let binding = entries
            .get_public(reference.diagnostic_public_id())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let checked_type = expression
            .value_type()
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: *owner })?;
        if binding.source_item() != reference.lookup_owner()
            || binding.binding_digest() != reference.binding()
            || checked_type.semantic_identity_digest()? != reference.value_type()
            || checked_type != &TypeKind::entity_ref(crate::types::EntityKind::Entry)
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    }
    Ok(())
}

impl FinalSemanticAnalysis {
    #[cfg(test)]
    pub(super) const fn accepted_types(&self) -> &BTreeMap<TypeId, TypeKind> {
        &self.types
    }

    #[allow(
        dead_code,
        reason = "used only by the crate-private Cut 2 ownership classifier until Cut 5 publication"
    )]
    pub(crate) fn matches_symbol_lease(&self, symbols: &ProjectSymbolTable) -> bool {
        symbols.world() == self.hir_generation().symbol_world()
            && *symbols.revision() == self.hir_generation().symbol_revision()
    }

    /// Validates and publishes a complete semantic generation.
    #[cfg(test)]
    pub(crate) fn try_new(
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        topology: Arc<HirProjectEvaluationTopology>,
        checked_callables: Arc<CheckedCallableCatalog>,
        input: FinalSemanticAnalysisInput,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let cancellation = AtomicBool::new(false);
        Self::try_new_with_control(
            project,
            symbols,
            topology,
            checked_callables,
            input,
            FinalSemanticAnalysisControl::new(&cancellation),
        )
    }

    /// Validates and publishes a complete semantic generation while observing
    /// caller-owned cancellation at every publication phase boundary.
    #[cfg(test)]
    pub(crate) fn try_new_with_control(
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        topology: Arc<HirProjectEvaluationTopology>,
        checked_callables: Arc<CheckedCallableCatalog>,
        input: FinalSemanticAnalysisInput,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        Self::try_new_with_control_and_type_resolutions(
            project,
            symbols,
            topology,
            checked_callables,
            input,
            BTreeMap::new(),
            control,
        )
    }

    /// Test-only publication keeps a single topology lease without exposing a
    /// constructor that can mint accepted roots in production.
    #[cfg(test)]
    pub(crate) fn try_new_with_control_and_type_resolutions(
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        topology: Arc<HirProjectEvaluationTopology>,
        checked_callables: Arc<CheckedCallableCatalog>,
        mut input: FinalSemanticAnalysisInput,
        type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        control.check()?;
        let mut item_facts = BTreeMap::new();
        for (item, fact) in &input.items {
            if item_facts.insert(*item, fact).is_some() {
                return Err(FinalSemanticAnalysisError::DuplicateFact {
                    family: SemanticFactFamily::Item,
                });
            }
        }
        let accepted_roots = Arc::new(AcceptedSemanticRootCatalog::seal(
            Arc::clone(&topology),
            &checked_callables,
            &item_facts,
        )?);
        let modules = project_generation_modules(project);
        let prepared_expressions = collect_unique(
            input.expressions.iter().cloned(),
            SemanticFactFamily::Expression,
        )?;
        let expressions = prepared_expressions;
        let selected_expressions =
            match_edges::CheckedSelectedExpressionGraph::seal_call_free_fixture(
                project,
                Arc::clone(&topology),
                &expressions,
            )?;
        input.set_structural_edges(match_edges::CheckedStructuralEdgeDraft::seal(
            &selected_expressions,
            &modules,
            &expressions,
        ))?;
        input.set_selected_expressions(selected_expressions)?;
        Self::try_new_with_control_and_type_resolutions_and_catalog(
            project,
            symbols,
            checked_callables,
            input,
            type_resolutions,
            accepted_roots,
            AcceptedSemanticShapeCatalog::default(),
            crate::checked_text_proxy::PreparedCheckedTextProxyCatalog::default(),
            super::CheckedFxDefinitionCatalog::default(),
            control,
        )
        .map_err(FinalSemanticProjectError::into_semantic_fixture_error)
    }

    /// Publishes the semantic type products created by the sole production
    /// nominal resolver with the same accepted generation as their flattened
    /// type facts. Manual fact fixtures deliberately use the constructor above
    /// and therefore cannot fabricate nominal-reference evidence.
    pub(super) fn try_new_with_control_and_type_resolutions_and_catalog(
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        checked_callables: Arc<CheckedCallableCatalog>,
        mut input: FinalSemanticAnalysisInput,
        type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
        accepted_roots: Arc<AcceptedSemanticRootCatalog>,
        semantic_shapes: AcceptedSemanticShapeCatalog,
        text_proxies: crate::checked_text_proxy::PreparedCheckedTextProxyCatalog,
        fx_definitions: super::CheckedFxDefinitionCatalog,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<Self, FinalSemanticProjectError> {
        control.check()?;
        let observed = project
            .accept_symbol_generation(symbols)
            .map_err(|_| FinalSemanticAnalysisError::SymbolGenerationMismatch)?;
        if !accepted_roots
            .topology()
            .generation()
            .same_generation(observed.generation().as_ref())
        {
            return Err(FinalSemanticAnalysisError::GenerationMismatch.into());
        }
        let callable_generation = checked_callables
            .hir_generation()
            .ok_or(FinalSemanticAnalysisError::CatalogGenerationMismatch)?;
        if !Arc::ptr_eq(accepted_roots.topology().generation(), callable_generation) {
            return Err(FinalSemanticAnalysisError::CatalogGenerationMismatch.into());
        }
        let evaluation_topology = Arc::clone(accepted_roots.topology());
        let types = collect_unique(input.types, SemanticFactFamily::Type)?;
        let locals = collect_unique(input.locals, SemanticFactFamily::Local)?;
        control.check()?;
        let captures = collect_unique(input.captures, SemanticFactFamily::Capture)?;
        control.check()?;
        let prepared_expressions =
            collect_unique(input.expressions, SemanticFactFamily::Expression)?;
        let selected_expressions = input
            .selected_expressions
            .take()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if !Arc::ptr_eq(selected_expressions.topology(), &evaluation_topology) {
            return Err(FinalSemanticAnalysisError::GenerationMismatch.into());
        }
        let structural_edges = input
            .structural_edges
            .take()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let ingress = input
            .ingress_seal
            .take()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        control.check()?;
        let prepared_patterns = collect_unique(input.patterns, SemanticFactFamily::Pattern)?;
        control.check()?;
        let statements = collect_unique(input.statements, SemanticFactFamily::Statement)?;
        control.check()?;
        let items = collect_unique(input.items, SemanticFactFamily::Item)?;
        control.check()?;
        let calls = collect_unique(
            input
                .calls
                .into_iter()
                .map(|call| (call.expression(), call)),
            SemanticFactFamily::Call,
        )?;
        let callable_joins = input.callable_joins;
        match_edges::validate_callable_join_inventory(&calls, &callable_joins)
            .map_err(|error| FinalSemanticAnalysisError::CheckedCallableJoin(Box::new(error)))?;
        let physical_candidate_argument_evaluations = input.physical_candidate_argument_evaluations;
        control.check()?;
        let draft = FinalSemanticAnalysisDraft {
            checked_callables,
            accepted_roots,
            types,
            type_resolutions,
            locals,
            captures,
            expressions: prepared_expressions,
            patterns: prepared_patterns,
            statements,
            items,
            calls,
            callable_joins,
            selected_expressions,
            structural_edges,
            ingress,
            text_proxies,
            fx_definitions,
            physical_candidate_argument_evaluations,
            executable_suspensions: input.executable_suspensions,
        };
        super::nominal_schema::seal_nominal_draft(draft, project, symbols, semantic_shapes, control)
    }

    /// Rejects reuse with any missing, foreign, or stale module generation.
    pub fn validate_generation(
        &self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let observed = project
            .accept_symbol_generation(symbols)
            .map_err(|_| FinalSemanticAnalysisError::SymbolGenerationMismatch)?;
        self.hir_generation()
            .same_generation(observed.generation().as_ref())
            .then_some(())
            .ok_or(FinalSemanticAnalysisError::GenerationMismatch)
    }

    /// Validates one module-scoped query against this report's exact accepted
    /// snapshot and symbol generation. This does not publish or reconstruct a
    /// partial analysis; it is only a lease check for consumers that already
    /// hold the immutable project report.
    pub fn validate_module_generation(
        &self,
        module: &HirModule,
        symbols: &ProjectSymbolTable,
    ) -> Result<(), FinalSemanticAnalysisError> {
        self.hir_generation()
            .validate_module_lease(module, symbols)
            .map_err(|_| FinalSemanticAnalysisError::GenerationMismatch)
    }

    pub fn hir_generation(&self) -> &Arc<arcweft_lang_hir::project::AcceptedHirProjectGeneration> {
        self.accepted_roots.topology().generation()
    }

    pub fn validate_registered_callable_authority(
        &self,
        registered: &crate::callable::RegisteredCallableCatalog,
    ) -> Result<(), crate::callable::CheckedCallableLookupError> {
        self.checked_callables
            .validate_registered_authority(registered, self.hir_generation().as_ref())
    }

    pub fn hir_topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        self.accepted_roots.topology()
    }

    /// Dialogue-line identities accepted from this generation's selected HIR
    /// expression graph. The HIR project itself retains only provisional
    /// source-site evidence.
    pub const fn dialogue_lines(
        &self,
    ) -> &arcweft_lang_hir::project::AcceptedDialogueLineInventory {
        &self.dialogue_lines
    }

    /// Sole checked Entry catalog accepted by this semantic generation.
    pub const fn checked_entries(&self) -> &CheckedEntryCatalog {
        &self.checked_entries
    }

    /// Sole typed text-proxy catalog accepted by this semantic generation.
    pub const fn checked_text_proxies(
        &self,
    ) -> &crate::checked_text_proxy::CheckedTextProxyCatalog {
        &self.checked_text_proxies
    }

    /// Sole checked Fx definition catalog accepted by this semantic generation.
    pub const fn checked_fx_definitions(&self) -> &super::CheckedFxDefinitionCatalog {
        &self.checked_fx_definitions
    }

    pub(crate) const fn runtime_nominals(&self) -> &RuntimeNominalProjectionCatalog {
        &self.runtime_nominals
    }

    /// Layout-free accepted semantics for one exact project nominal type.
    pub(crate) fn project_nominal_semantic(
        &self,
        semantic_type: crate::types::SemanticTypeDigest,
    ) -> Option<&ProjectNominalSemanticDefinition> {
        self.project_nominals.get(semantic_type)
    }

    /// Complete layout-free checked variant owner for one project semantic type.
    pub fn project_variant_owner(
        &self,
        semantic_type: crate::types::SemanticTypeDigest,
    ) -> Result<Option<super::CheckedVariantOwner>, super::CheckedVariantOwnerError> {
        let Some(definition) = self.project_nominals.get(semantic_type) else {
            return Ok(None);
        };
        let Some(cases) = definition.cases() else {
            return Ok(None);
        };
        super::CheckedVariantOwner::try_project_shapes(
            definition.nominal().clone(),
            cases.iter().map(|case| {
                (
                    case.payload().clone(),
                    Some(case.diagnostic_name().to_owned()),
                )
            }),
        )
        .map(Some)
    }

    pub(crate) const fn semantic_shapes(&self) -> &AcceptedSemanticShapeCatalog {
        &self.semantic_shapes
    }

    pub fn ty(&self, owner: TypeId) -> Option<&TypeKind> {
        self.types.get(&owner)
    }

    /// Complete nominal-resolution product for one exact final-HIR type root.
    ///
    /// Production reports contain one row for every structural type root.
    /// Hand-built test fixtures intentionally return `None` instead of
    /// inventing source or alias evidence.
    pub fn type_resolution(&self, owner: TypeId) -> Option<&TypeResolutionReport> {
        self.type_resolutions.get(&owner)
    }

    pub fn type_resolutions(
        &self,
    ) -> impl ExactSizeIterator<Item = (TypeId, &TypeResolutionReport)> {
        self.type_resolutions
            .iter()
            .map(|(owner, report)| (*owner, report))
    }

    pub fn local(&self, owner: LocalId) -> Option<&CheckedBinding> {
        self.locals.get(&owner)
    }

    pub fn capture(&self, owner: CaptureId) -> Option<&CheckedBinding> {
        self.captures.get(&owner)
    }

    /// Selected capture identity, source local, and access from its closed
    /// producer. Type facts and this projection belong to the same generation.
    pub fn selected_capture(
        &self,
        owner: CaptureId,
    ) -> Option<&arcweft_lang_hir::project::HirSelectedCapture> {
        self.capture(owner)?;
        let row = self
            .accepted_root_catalog()
            .topology()
            .module(owner.module())?
            .captures()
            .capture(owner)?;
        let super::CheckedExpressionResolution::Closure(closure) =
            self.expression(row.closure())?.resolution()
        else {
            return None;
        };
        closure
            .captures()
            .iter()
            .find(|capture| capture.capture() == owner)
    }

    pub fn expression(&self, owner: ExprId) -> Option<&CheckedExpression> {
        self.expressions.get(&owner)
    }

    /// Borrows the final execution authority for this semantic generation.
    pub const fn execution_projection(&self) -> FinalAnalysisExecutionProjection<'_> {
        FinalAnalysisExecutionProjection { analysis: self }
    }

    pub fn pattern(&self, owner: PatternId) -> Option<&CheckedPattern> {
        self.patterns.get(&owner)
    }

    pub fn statement(&self, owner: StmtId) -> Option<&CheckedStatement> {
        self.statements.get(&owner)
    }

    pub fn item(&self, owner: ItemId) -> Option<&CheckedItem> {
        self.items.get(&owner)
    }

    pub fn call(&self, owner: ExprId) -> Option<&CallTargetFacts> {
        self.calls.get(&owner)
    }

    /// Sole immutable checked callable/effect authority accepted with this
    /// semantic generation.
    pub const fn checked_callables(&self) -> &Arc<CheckedCallableCatalog> {
        &self.checked_callables
    }

    /// Sole accepted-root authority retained by this immutable report.
    pub(crate) const fn accepted_root_catalog(&self) -> &Arc<AcceptedSemanticRootCatalog> {
        &self.accepted_roots
    }

    pub fn types(&self) -> impl ExactSizeIterator<Item = (TypeId, &TypeKind)> {
        self.types.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn locals(&self) -> impl ExactSizeIterator<Item = (LocalId, &CheckedBinding)> {
        self.locals.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn captures(&self) -> impl ExactSizeIterator<Item = (CaptureId, &CheckedBinding)> {
        self.captures.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn expressions(&self) -> impl ExactSizeIterator<Item = (ExprId, &CheckedExpression)> {
        self.expressions.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn patterns(&self) -> impl ExactSizeIterator<Item = (PatternId, &CheckedPattern)> {
        self.patterns.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn statements(&self) -> impl ExactSizeIterator<Item = (StmtId, &CheckedStatement)> {
        self.statements.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn items(&self) -> impl ExactSizeIterator<Item = (ItemId, &CheckedItem)> {
        self.items.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn calls(&self) -> impl ExactSizeIterator<Item = (ExprId, &CallTargetFacts)> {
        self.calls.iter().map(|(id, fact)| (*id, fact))
    }

    /// Ordered per-root operational trace used by the sema acceptance matrix.
    /// This remains crate-owned and is not projected into language tooling.
    #[cfg(test)]
    pub(crate) fn physical_candidate_argument_evaluations(
        &self,
    ) -> impl Iterator<Item = &PhysicalCandidateArgumentEvaluation> {
        self.physical_candidate_argument_evaluations
            .values()
            .flat_map(|evaluations| evaluations.iter())
    }

    pub const fn work(&self) -> FinalSemanticAnalysisWork {
        self.work
    }

    /// Diagnostics remain owned by their exact call facts; this projection
    /// does not copy them into a positional side table.
    pub fn call_diagnostics(&self) -> impl Iterator<Item = &crate::callable::CallableDiagnostic> {
        self.calls
            .values()
            .flat_map(|call| call.diagnostics().iter())
    }

    /// Source-backed warnings accepted with this exact semantic generation.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

fn collect_sealed_expressions(
    prepared: BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Result<BTreeMap<ExprId, CheckedExpression>, FinalSemanticAnalysisError> {
    prepared
        .into_iter()
        .map(|(owner, fact)| {
            let super::PreparedExpressionFact::Complete(fact) = fact else {
                return Err(FinalSemanticAnalysisError::UnsealedPreparedC2Owner);
            };
            Ok((owner, fact))
        })
        .collect()
}

fn collect_sealed_patterns(
    prepared: BTreeMap<PatternId, super::PreparedPatternFact>,
) -> Result<BTreeMap<PatternId, CheckedPattern>, FinalSemanticAnalysisError> {
    prepared
        .into_iter()
        .map(|(owner, fact)| {
            fact.into_complete()
                .map(|fact| (owner, fact))
                .map_err(|_| FinalSemanticAnalysisError::UnsealedPreparedC2Owner)
        })
        .collect()
}

fn project_generation_modules(
    project: HirAnalysisProjectView<'_>,
) -> BTreeMap<HirModuleId, &HirModule> {
    project
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect()
}

fn collect_final_diagnostics(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    types: &BTreeMap<TypeId, TypeKind>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    items: &BTreeMap<ItemId, CheckedItem>,
) -> Result<Vec<Diagnostic>, FinalSemanticAnalysisError> {
    let mut diagnostics = Vec::new();
    for (owner, checked) in expressions {
        if checked.type_selection() != Some(super::CheckedTypeSelection::DefaultNumericFallback) {
            continue;
        }
        let module = resolve_module(modules, owner.module())?;
        let expression = module
            .resolve_expr(*owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if !scope_is_nested_in_closure(module, expression.scope())? {
            continue;
        }
        let span = source_span(
            module,
            HirSourceQuery::Expr {
                owner: *owner,
                role: HirExprSourceRole::Whole,
            },
        )?;
        let checked_type = checked
            .value_type()
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: *owner })?;
        diagnostics.push(
            Diagnostic::new(
                DiagnosticSeverity::Warning,
                format!(
                    "unsuffixed numeric literal inside inferred closure body defaults to {}; add a suffix or closure return type to make the contract explicit",
                    checked_type.source_label()
                ),
            )
            .with_code("sema.numeric.fallback_in_inferred_closure")
            .with_label(DiagnosticLabel::primary(
                span,
                Some("default numeric type selected here".to_owned()),
            )),
        );
    }
    for (owner, checked) in items {
        if !matches!(checked.role(), super::CheckedItemRole::TypeAlias) {
            continue;
        }
        let module = resolve_module(modules, owner.module())?;
        let item = module
            .resolve_item(*owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirItemKind::TypeAlias(alias) = item.kind() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if item.prefix().visibility() != Some(HirVisibility::Public)
            || !matches!(types.get(&alias.target()), Some(TypeKind::Choice(_)))
        {
            continue;
        }
        let name = alias
            .name()
            .resolved()
            .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
        let span = source_span(
            module,
            HirSourceQuery::Item {
                owner: *owner,
                role: HirItemSourceRole::Declaration(HirDeclarationSourceRole::Name),
            },
        )?;
        let ty = types
            .get(&alias.target())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        diagnostics.push(
            Diagnostic::new(
                DiagnosticSeverity::Warning,
                format!(
                    "public type alias `{}` exposes anonymous sum `{}`; public ABI and save data are more stable with a nominal enum",
                    name.as_str(),
                    ty.source_label()
                ),
            )
            .with_code("sema.public_abi.anonymous_sum")
            .with_label(DiagnosticLabel::primary(
                span,
                Some("public anonymous sum type".to_owned()),
            )),
        );
    }
    Ok(diagnostics)
}

fn scope_is_nested_in_closure(
    module: &HirModule,
    mut scope: arcweft_lang_hir::identity::ScopeId,
) -> Result<bool, FinalSemanticAnalysisError> {
    loop {
        let current = module
            .resolve_scope(scope)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if current.kind() == HirScopeKind::Closure {
            return Ok(true);
        }
        let Some(parent) = current.parent() else {
            return Ok(false);
        };
        scope = parent;
    }
}

fn resolve_module<'a>(
    modules: &BTreeMap<HirModuleId, &'a HirModule>,
    owner: HirModuleId,
) -> Result<&'a HirModule, FinalSemanticAnalysisError> {
    modules
        .get(&owner)
        .copied()
        .ok_or(FinalSemanticAnalysisError::InvalidOwner)
}

fn source_span(
    module: &HirModule,
    query: HirSourceQuery,
) -> Result<SourceSpan, FinalSemanticAnalysisError> {
    module
        .source_anchor(query)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
        .ok_or(FinalSemanticAnalysisError::RecoveredOwner)
}

fn validate_type_resolution_reports(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    accepted_owners: Option<&BTreeSet<TypeId>>,
    types: &BTreeMap<TypeId, TypeKind>,
    reports: &BTreeMap<TypeId, TypeResolutionReport>,
) -> Result<(), FinalSemanticAnalysisError> {
    if reports.is_empty() {
        return Ok(());
    }
    let all_nodes = accepted_owners.cloned().unwrap_or_else(|| {
        modules
            .values()
            .flat_map(|module| module.types().map(|(owner, _)| owner))
            .collect::<BTreeSet<_>>()
    });
    let mut node_facts = BTreeMap::new();
    for (owner, report) in reports {
        let product = report.outcome().product();
        if product.root() != *owner {
            return Err(FinalSemanticAnalysisError::TypeResolutionReportMismatch { owner: *owner });
        }
        for node in product.nodes() {
            if node.is_contextual_alias_target() {
                continue;
            }
            if !all_nodes.contains(&node.node()) {
                return Err(FinalSemanticAnalysisError::TypeResolutionReportMismatch {
                    owner: node.node(),
                });
            }
            let recovered = node.recovered().cloned();
            merge_type_resolution_fact(&mut node_facts, node.node(), &recovered)?;
        }
    }
    let covered = node_facts.keys().copied().collect::<BTreeSet<_>>();
    let recovered = node_facts
        .into_iter()
        .filter_map(|(owner, ty)| ty.map(|ty| (owner, ty)))
        .collect::<BTreeMap<_, _>>();
    if covered != all_nodes || recovered != *types {
        let owner = all_nodes
            .iter()
            .find(|owner| !covered.contains(owner) || recovered.get(owner) != types.get(owner))
            .copied()
            .or_else(|| {
                types
                    .keys()
                    .find(|owner| !recovered.contains_key(owner))
                    .copied()
            })
            .unwrap_or_else(|| *reports.keys().next().expect("non-empty report inventory"));
        return Err(FinalSemanticAnalysisError::TypeResolutionReportMismatch { owner });
    }
    Ok(())
}

pub(super) fn merge_type_resolution_fact<T: Clone + Eq>(
    facts: &mut BTreeMap<TypeId, T>,
    owner: TypeId,
    fact: &T,
) -> Result<(), FinalSemanticAnalysisError> {
    match facts.entry(owner) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(fact.clone());
            Ok(())
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == fact => Ok(()),
        std::collections::btree_map::Entry::Occupied(_) => {
            Err(FinalSemanticAnalysisError::TypeResolutionReportMismatch { owner })
        }
    }
}
