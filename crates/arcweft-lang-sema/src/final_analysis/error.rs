//! Publication and semantic-analysis failures.

use std::sync::Arc;

use arcweft_interaction_model::dialogue::CharacterDialogueCustomFieldId;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceSpan};
use thiserror::Error;

use super::analyzer::{CallAnalysisFailure, CallFrameInvariant};
use super::{
    AssertionContext, AssertionMode, CharacterDialogueFieldCoordinate,
    CheckedCaptureAuthorityViolation, CheckedRichTextReport, EffectSet, ExprId, ItemId, LocalId,
    PatternId, StmtId, TypeId, TypeKind,
};
use crate::callable::{
    CallConstraintInvariant, CheckedCallSite, CheckedCallableId, CheckedCallableJoinError,
    UnknownCallKind,
};
use crate::entry::CheckedEntryDiagnostic;
use crate::semantic_coordinate::AcceptedSemanticRootCatalogError;

/// One typed call edge participating in a forbidden Predicate/Proof SCC.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RecursiveCallableContractEdge {
    caller: CheckedCallableId,
    callee: CheckedCallableId,
    expression: ExprId,
}

impl RecursiveCallableContractEdge {
    pub(crate) fn new(
        source_callable: CheckedCallableId,
        target_callable: CheckedCallableId,
        expression: ExprId,
    ) -> Self {
        Self {
            caller: source_callable,
            callee: target_callable,
            expression,
        }
    }

    pub const fn caller(&self) -> &CheckedCallableId {
        &self.caller
    }

    pub const fn callee(&self) -> &CheckedCallableId {
        &self.callee
    }

    pub const fn expression(&self) -> ExprId {
        self.expression
    }
}

/// Fact family used for duplicate and completeness diagnostics.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticFactFamily {
    Type,
    Local,
    Capture,
    Expression,
    Pattern,
    Statement,
    Item,
    Call,
}

/// Opaque public carrier for one terminal call-constraint failure.
///
/// The outer expression owner and the exact analyzer failure remain paired in
/// one immutable value.  Its payload is intentionally private: public users
/// can report the failure without manufacturing or reclassifying lower
/// constraint evidence, while the internal analyzer retains the full typed
/// disposition and nested source payload.
#[derive(Clone, Eq, PartialEq)]
pub struct FinalCallConstraintFailure {
    owner: ExprId,
    failure: Arc<CallAnalysisFailure>,
}

impl std::fmt::Debug for FinalCallConstraintFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FinalCallConstraintFailure")
            .field("owner", &self.owner)
            .field("failure", &self.failure)
            .finish()
    }
}

impl FinalCallConstraintFailure {
    pub(crate) fn new(owner: ExprId, failure: CallAnalysisFailure) -> Self {
        Self {
            owner,
            failure: Arc::new(failure),
        }
    }

    pub const fn owner(&self) -> ExprId {
        self.owner
    }
}

impl std::fmt::Display for FinalCallConstraintFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "call constraint failure for expression {:?}",
            self.owner
        )
    }
}

/// Exact coordinate at which final callable-graph sealing failed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FinalCallSealLocation {
    /// Failure while consuming one exact checked call graph node.
    Site(CheckedCallSite),
    /// Failure in the affine graph-wide detach or completion boundary.
    Graph,
}

/// Opaque public carrier for a terminal C-seal invariant.
///
/// The location and private typed callable invariant remain paired instead of
/// being reclassified as a checked-catalog lookup failure. Public consumers
/// can report the exact phase and site but cannot manufacture or branch on the
/// internal invariant vocabulary.
#[derive(Clone, Eq, PartialEq)]
pub struct FinalCallSealFailure {
    location: FinalCallSealLocation,
    failure: Arc<CallConstraintInvariant>,
}

impl FinalCallSealFailure {
    pub(crate) fn new(location: FinalCallSealLocation, failure: CallConstraintInvariant) -> Self {
        Self {
            location,
            failure: Arc::new(failure),
        }
    }

    pub const fn location(&self) -> FinalCallSealLocation {
        self.location
    }

    #[cfg(test)]
    pub(crate) fn typed_failure_for_test(&self) -> &CallConstraintInvariant {
        &self.failure
    }
}

impl std::fmt::Debug for FinalCallSealFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FinalCallSealFailure")
            .field("location", &self.location)
            .field("failure", &self.failure)
            .finish()
    }
}

impl std::fmt::Display for FinalCallSealFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "call seal failure at {:?}: {}",
            self.location, self.failure
        )
    }
}

/// Opaque public carrier for a poisoned or otherwise invalid call-frame
/// transaction.  The analyzer retains the exact internal violation while the
/// public error keeps the generation-bound owner and a stable error boundary.
#[derive(Clone, Eq, PartialEq)]
pub struct FinalCallFrameInvariant {
    owner: ExprId,
    violation: CallFrameInvariant,
}

impl FinalCallFrameInvariant {
    pub(crate) fn new(owner: ExprId, violation: CallFrameInvariant) -> Self {
        Self { owner, violation }
    }
}

impl std::fmt::Debug for FinalCallFrameInvariant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FinalCallFrameInvariant")
            .field("owner", &self.owner)
            .finish()
    }
}

impl std::fmt::Display for FinalCallFrameInvariant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "call-frame invariant for expression {:?}",
            self.owner
        )
    }
}

/// Internal affine-token violation at the semantic fact transaction boundary.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CandidateFactTransactionViolation {
    #[error("candidate fact checkpoint sequence is exhausted")]
    SequenceExhausted,
    #[error("candidate fact checkpoint belongs to another issuer")]
    ForeignCheckpoint,
    #[error("candidate fact checkpoint is stale")]
    StaleCheckpoint,
    #[error("candidate fact checkpoint is not the active LIFO checkpoint")]
    NonLifoCheckpoint,
    #[error("candidate fact checkpoint journal cursor is inconsistent")]
    JournalCursorMismatch,
    #[error("candidate semantic projection belongs to another issuer")]
    ForeignProjection,
    #[error("candidate semantic projection requires one active transaction")]
    ProjectionOutsideTransaction,
    #[error("candidate semantic projection was not retained for publication")]
    ProjectionUnavailable,
    #[error("candidate fact ledger is poisoned after an unrecoverable close")]
    Poisoned,
    #[error("candidate fact ledger could not recover to its oldest active frame")]
    UnrecoverableLedger,
    #[error(
        "implicit capture use {callable:?}/{expression:?} resolved to conflicting locals {existing:?} and {proposed:?}"
    )]
    ImplicitCaptureUseConflict {
        callable: ExprId,
        expression: ExprId,
        existing: LocalId,
        proposed: LocalId,
    },
    #[error("candidate semantic projection authority is stale or mismatched")]
    ProjectionAuthorityMismatch,
    #[error("physical call attempt is not rooted in the active call-frame issuer")]
    PhysicalCallAttemptRootMismatch,
    #[error("physical call attempt is not the active LIFO attempt")]
    PhysicalCallAttemptOrder,
    #[error("physical candidate evaluation does not belong to the active call attempt")]
    PhysicalCallAttemptMismatch,
    #[error("prepared call graph transaction violation: {0}")]
    PreparedCallGraph(crate::callable::PreparedCallGraphInvariant),
}

/// Failure to publish final semantic facts.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FinalSemanticAnalysisError {
    #[error("semantic analysis publication was cancelled")]
    Cancelled,
    #[error("semantic analysis work accounting overflowed")]
    AccountingOverflow,
    #[error("project symbol authority does not match the executable HIR generation")]
    SymbolGenerationMismatch,
    #[error("semantic analysis contains duplicate {family:?} fact")]
    DuplicateFact { family: SemanticFactFamily },
    #[error("semantic analysis is missing a {family:?} fact")]
    MissingFact { family: SemanticFactFamily },
    #[error("semantic expression fact {owner:?} is outside the selected expression graph")]
    UnexpectedExpressionFact { owner: ExprId },
    #[error("semantic fact references a foreign or missing HIR owner")]
    InvalidOwner,
    #[error("semantic fact does not match its final-HIR payload family")]
    WrongPayloadFamily,
    #[error("checked terminal capture authority violation: {violation}")]
    CaptureAuthority {
        violation: CheckedCaptureAuthorityViolation,
    },
    #[error("semantic fact references a recovered HIR payload")]
    RecoveredOwner,
    #[error("semantic type contains a poison carrier and cannot enter an executable report")]
    PoisonedType,
    #[error("semantic fact references an invalid project nominal owner")]
    InvalidNominalOwner,
    #[error("projection-dependent semantic owner reached publication before the C2 seal")]
    UnsealedPreparedC2Owner,
    #[error("runtime nominal projection failed: {0}")]
    NominalSchemaProjection(#[from] super::NominalSchemaProjectionError),
    #[error("semantic fact references an invalid project callable owner")]
    InvalidCallableOwner,
    #[error("call diagnostic source does not belong to the accepted project generation")]
    DiagnosticSourceMismatch,
    #[error("semantic effect row is not closed")]
    OpenEffectRow,
    #[error("semantic candidate fact transaction violation: {violation}")]
    CandidateFactTransaction {
        violation: CandidateFactTransactionViolation,
    },
    #[error(
        "assertion statement {owner:?} mode {mode:?} is not admitted in semantic context {context:?}"
    )]
    AssertionModeNotAllowed {
        owner: StmtId,
        mode: AssertionMode,
        context: AssertionContext,
    },
    #[error(
        "assertion statement {owner:?} condition {index} ({condition:?}) must have type Bool, found {actual:?}"
    )]
    AssertionConditionNotBool {
        owner: StmtId,
        condition: ExprId,
        index: usize,
        actual: Box<TypeKind>,
    },
    #[error(
        "assertion statement {owner:?} condition {index} ({condition:?}) must be pure and deterministic, found effects {effects}"
    )]
    AssertionConditionNotPure {
        owner: StmtId,
        condition: ExprId,
        index: usize,
        effects: EffectSet,
    },
    #[error("call fact is not a clean selected call")]
    UnacceptedCall,
    #[error("call result/effects disagree with the expression fact")]
    CallFactMismatch,
    #[error("semantic analysis belongs to a different HIR generation")]
    GenerationMismatch,
    #[error("semantic catalogs do not belong to the supplied project symbol authority")]
    CatalogGenerationMismatch,
    #[error("checked callable catalog construction or validation failed")]
    CheckedCallableCatalog,
    #[error("accepted semantic root catalog construction or validation failed: {0}")]
    AcceptedSemanticRootCatalog(#[from] AcceptedSemanticRootCatalogError),
    #[error("checked callable join enrichment failed: {0}")]
    CheckedCallableJoin(Box<CheckedCallableJoinError>),
    #[error("failed to construct the accepted nominal-resolution input for {owner:?}")]
    TypeResolutionInput { owner: TypeId },
    #[error("nominal type resolution did not produce one complete type for {owner:?}")]
    TypeResolutionFailed { owner: TypeId },
    #[error("nominal-resolution evidence disagrees with final type fact {owner:?}")]
    TypeResolutionReportMismatch { owner: TypeId },
    #[error("failed to construct the lexical generic scope for {owner:?}")]
    GenericScope { owner: TypeId },
    #[error("semantic expression dependency cycle contains {owner:?}")]
    ExpressionCycle { owner: ExprId },
    #[error("predicate/proof recursive callable contract contains call edges {edges:?}")]
    RecursiveCallableContract {
        edges: Box<[RecursiveCallableContractEdge]>,
    },
    #[error("semantic expression {owner:?} has no admissible final type")]
    ExpressionTypeUnavailable { owner: ExprId },
    #[error("postfix-bracket expression {owner:?} has more than one admissible interpretation")]
    AmbiguousPostfixBracket { owner: ExprId },
    #[error("postfix-bracket expression {owner:?} has no admissible interpretation")]
    UnresolvedPostfixBracket { owner: ExprId },
    #[error("dialogue content {owner:?} has invalid typed RichText attributes")]
    InvalidRichTextAttributes {
        owner: ExprId,
        report: Box<CheckedRichTextReport>,
    },
    #[error("dialogue content {owner:?} has inconsistent final-HIR source-role evidence")]
    RichTextSourceQuery { owner: ExprId },
    #[error("semantic local {owner:?} has no admissible final type")]
    LocalTypeUnavailable { owner: LocalId },
    #[error("semantic pattern {owner:?} has no admissible final type")]
    PatternTypeUnavailable { owner: PatternId },
    #[error("break expr is allowed only in loop blocks")]
    BreakValueRequiresLoop {
        owner: StmtId,
        value: ExprId,
        target: StmtId,
        value_source: SourceSpan,
        target_source: SourceSpan,
    },
    #[error("semantic value resolution failed for expression {owner:?}")]
    ValueResolutionFailed { owner: ExprId },
    #[error("shared callable resolution failed for expression {owner:?}")]
    CallResolutionFailed { owner: ExprId },
    #[error("typed call constraint failure: {0}")]
    CallConstraintFailure(FinalCallConstraintFailure),
    #[error("typed final call seal failure: {0}")]
    CallSeal(FinalCallSealFailure),
    #[error("typed call-frame invariant: {0}")]
    CallFrameInvariant(FinalCallFrameInvariant),
    #[error("CharacterDialogue patch on expression {owner:?} has an invalid argument shape")]
    InvalidCharacterDialoguePatch { owner: ExprId },
    #[error("CharacterDialogue patch uses unknown field `{name}` in module {scope:?}")]
    UnknownCharacterDialogueField {
        name: String,
        field_span: SourceSpan,
        scope: CanonicalModulePath,
    },
    #[error("CharacterDialogue patch repeats field coordinate {coordinate:?}")]
    DuplicateCharacterDialogueField {
        coordinate: CharacterDialogueFieldCoordinate,
        first_span: SourceSpan,
        duplicate_span: SourceSpan,
    },
    #[error("CharacterDialogue field `{field}` expects {declared:?}, found {actual:?}")]
    CharacterDialogueCustomFieldTypeMismatch {
        field: CharacterDialogueCustomFieldId,
        declared: Box<TypeKind>,
        actual: Box<TypeKind>,
        value_span: SourceSpan,
        declaration_span: SourceSpan,
    },
    #[error("CharacterDialogue field `{field}` is only valid on an immediate content application")]
    CharacterDialogueApplicationOnlyField {
        field: String,
        field_span: SourceSpan,
    },
    #[error("CharacterDialogue patch field on expression {owner:?} has an incompatible type")]
    CharacterDialogueFieldType { owner: ExprId },
    #[error("CharacterDialogue custom field `{field}` is not clearable")]
    CharacterDialogueFieldNotClearable {
        field: CharacterDialogueCustomFieldId,
        field_span: SourceSpan,
        declaration_span: SourceSpan,
    },
    #[error("DialogueLine execution operation cannot escape into a runtime value")]
    DialogueLineEscape { escape_span: SourceSpan },
    #[error("unknown function `{name}`")]
    UnknownCallTarget {
        owner: ExprId,
        kind: UnknownCallKind,
        name: String,
        call_source: SourceSpan,
    },
    #[error(
        "try propagates error type {operand_error:?}, but the enclosing return boundary requires {return_error:?}"
    )]
    PropagationErrorMismatch {
        owner: ExprId,
        operand_error: Box<TypeKind>,
        return_error: Box<TypeKind>,
        operator_source: SourceSpan,
        return_source: SourceSpan,
    },
    #[error("callable `{callable}` performs effects {missing} outside its declared upper bound")]
    EffectUpperBoundExceeded {
        owner: ItemId,
        callable: String,
        missing: EffectSet,
        contract_source: SourceSpan,
        trace_notes: Box<[String]>,
    },
    #[error("ordinary function {owner:?} has an invalid execution role")]
    InvalidFunctionExecution { owner: ItemId },
    #[error("callable body support remains deferred for item {owner:?}")]
    UnsupportedCallableBody { owner: ItemId },
}

/// Terminal result boundary for one complete final-project semantic seal.
/// Entry admission remains distinct from semantic invariant failure so its
/// complete source-backed diagnostic set is never flattened.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FinalSemanticProjectError {
    #[error(transparent)]
    Semantic(#[from] FinalSemanticAnalysisError),
    #[error("checked Entry seal rejected one or more declarations")]
    Entry(Box<[CheckedEntryDiagnostic]>),
}

impl FinalSemanticProjectError {
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::Semantic(error) => error.diagnostic_code(),
            Self::Entry(diagnostics) => match diagnostics.first() {
                Some(diagnostic) => diagnostic.code(),
                None => "sema.entry.invalid_catalog",
            },
        }
    }

    pub fn source_diagnostic(&self) -> Option<Diagnostic> {
        match self {
            Self::Semantic(error) => error.source_diagnostic(),
            Self::Entry(diagnostics) => {
                let diagnostic = diagnostics.first()?;
                let mut projected =
                    Diagnostic::new(DiagnosticSeverity::Error, diagnostic.message().to_owned())
                        .with_code(diagnostic.code())
                        .with_label(DiagnosticLabel::primary(diagnostic.primary().clone(), None));
                for related in diagnostic.related() {
                    projected =
                        projected.with_label(DiagnosticLabel::secondary(related.clone(), None));
                }
                Some(projected)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn into_semantic_fixture_error(self) -> FinalSemanticAnalysisError {
        match self {
            Self::Semantic(error) => error,
            Self::Entry(diagnostics) => panic!(
                "call-free semantic fixture unexpectedly reached Entry rejection: {diagnostics:?}"
            ),
        }
    }
}

impl From<super::NominalSchemaProjectionError> for FinalSemanticProjectError {
    fn from(error: super::NominalSchemaProjectionError) -> Self {
        FinalSemanticAnalysisError::from(error).into()
    }
}

impl From<AcceptedSemanticRootCatalogError> for FinalSemanticProjectError {
    fn from(error: AcceptedSemanticRootCatalogError) -> Self {
        FinalSemanticAnalysisError::from(error).into()
    }
}

impl From<CandidateFactTransactionViolation> for FinalSemanticAnalysisError {
    fn from(violation: CandidateFactTransactionViolation) -> Self {
        Self::CandidateFactTransaction { violation }
    }
}

impl From<CheckedCaptureAuthorityViolation> for FinalSemanticAnalysisError {
    fn from(violation: CheckedCaptureAuthorityViolation) -> Self {
        Self::CaptureAuthority { violation }
    }
}

impl FinalSemanticAnalysisError {
    /// Stable compiler/LSP diagnostic code owned by the final semantic authority.
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::AssertionModeNotAllowed { .. } => "sema.assert.context",
            Self::AssertionConditionNotBool { .. } => "sema.assert.condition_not_bool",
            Self::AssertionConditionNotPure { .. } => "sema.assert.condition_not_pure",
            Self::RecursiveCallableContract { .. } => "sema.callable.recursive_contract",
            Self::BreakValueRequiresLoop { .. } => "sema.break.value_target",
            Self::UnknownCallTarget { .. } => "sema.call.unknown_target",
            Self::PropagationErrorMismatch { .. } => "sema.try.error_mismatch",
            Self::EffectUpperBoundExceeded { .. } => "AWF-EFX-001",
            Self::DuplicateCharacterDialogueField { .. } => "AW-CD-005",
            Self::CharacterDialogueApplicationOnlyField { .. } => "AW-CD-007",
            Self::UnknownCharacterDialogueField { .. } => "AW-CD-014",
            Self::CharacterDialogueCustomFieldTypeMismatch { .. } => "AW-CD-015",
            Self::CharacterDialogueFieldNotClearable { .. } => "AW-CD-016",
            Self::DialogueLineEscape { .. } => "AW-CD-017",
            _ => "sema.final_analysis",
        }
    }

    /// Source-backed semantic rejection retained by the final analyzer.
    pub fn source_diagnostic(&self) -> Option<Diagnostic> {
        match self {
            Self::UnknownCallTarget {
                kind,
                name,
                call_source,
                ..
            } => Some(unknown_call_diagnostic(
                *kind,
                name,
                call_source,
                self.diagnostic_code(),
            )),
            Self::PropagationErrorMismatch {
                operand_error,
                return_error,
                operator_source,
                return_source,
                ..
            } => Some(propagation_diagnostic(
                operand_error,
                return_error,
                operator_source,
                return_source,
                self.diagnostic_code(),
            )),
            Self::EffectUpperBoundExceeded {
                callable,
                missing,
                contract_source,
                trace_notes,
                ..
            } => Some(effect_upper_bound_diagnostic(
                callable,
                missing,
                contract_source,
                trace_notes,
                self.diagnostic_code(),
            )),
            Self::DialogueLineEscape { escape_span } => Some(dialogue_line_escape_diagnostic(
                escape_span,
                self.diagnostic_code(),
            )),
            Self::BreakValueRequiresLoop {
                value_source,
                target_source,
                ..
            } => Some(break_value_requires_loop_diagnostic(
                value_source,
                target_source,
                self.diagnostic_code(),
            )),
            _ => character_dialogue_diagnostic(self),
        }
    }
}

fn break_value_requires_loop_diagnostic(
    value_source: &SourceSpan,
    target_source: &SourceSpan,
    code: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "break expr is allowed only in loop blocks",
    )
    .with_code(code)
    .with_label(DiagnosticLabel::primary(
        value_source.clone(),
        Some("this break carries a value".to_owned()),
    ))
    .with_label(DiagnosticLabel::secondary(
        target_source.clone(),
        Some("this target loop does not yield a value".to_owned()),
    ))
}

fn unknown_call_diagnostic(
    kind: UnknownCallKind,
    name: &str,
    call_source: &SourceSpan,
    code: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        match kind {
            UnknownCallKind::Free => format!("unknown function `{name}`"),
            UnknownCallKind::Method => format!("unknown method `{name}`"),
            UnknownCallKind::AssociatedType => format!("unknown associated function `{name}`"),
        },
    )
    .with_code(code)
    .with_label(DiagnosticLabel::primary(
        call_source.clone(),
        Some("no registered callable candidate matches this target".to_owned()),
    ))
}

fn propagation_diagnostic(
    operand_error: &TypeKind,
    return_error: &TypeKind,
    operator_source: &SourceSpan,
    return_source: &SourceSpan,
    code: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        format!(
            "try propagates error type {}, but the enclosing return boundary requires {}",
            operand_error.source_label(),
            return_error.source_label(),
        ),
    )
    .with_code(code)
    .with_label(DiagnosticLabel::primary(
        operator_source.clone(),
        Some("propagated error type does not match this boundary".to_owned()),
    ))
    .with_label(DiagnosticLabel::secondary(
        return_source.clone(),
        Some("enclosing return error is declared here".to_owned()),
    ))
}

fn effect_upper_bound_diagnostic(
    callable: &str,
    missing: &EffectSet,
    contract_source: &SourceSpan,
    trace_notes: &[String],
    code: &'static str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(
        DiagnosticSeverity::Error,
        format!(
            "callable `{callable}` performs effects {missing} outside its declared upper bound"
        ),
    )
    .with_code(code)
    .with_label(DiagnosticLabel::primary(
        contract_source.clone(),
        Some("declared effect upper bound is exceeded".to_owned()),
    ));
    for note in trace_notes {
        diagnostic = diagnostic.with_note(note.clone());
    }
    diagnostic
}

fn dialogue_line_escape_diagnostic(escape_span: &SourceSpan, code: &'static str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "DialogueLine is a line-execution operation and cannot be stored, captured, returned, or passed as a runtime value",
    )
    .with_code(code)
    .with_label(DiagnosticLabel::primary(
        escape_span.clone(),
        Some("this operation escapes its line-execution boundary".to_owned()),
    ))
}

fn character_dialogue_diagnostic(error: &FinalSemanticAnalysisError) -> Option<Diagnostic> {
    let code = error.diagnostic_code();
    match error {
        FinalSemanticAnalysisError::UnknownCharacterDialogueField {
            name,
            field_span,
            scope,
        } => Some(unknown_character_dialogue_field_diagnostic(
            name, field_span, scope, code,
        )),
        FinalSemanticAnalysisError::DuplicateCharacterDialogueField {
            coordinate,
            first_span,
            duplicate_span,
        } => Some(duplicate_character_dialogue_field_diagnostic(
            coordinate,
            first_span,
            duplicate_span,
            code,
        )),
        FinalSemanticAnalysisError::CharacterDialogueCustomFieldTypeMismatch {
            field,
            declared,
            actual,
            value_span,
            declaration_span,
        } => Some(character_dialogue_type_mismatch_diagnostic(
            field,
            declared,
            actual,
            value_span,
            declaration_span,
            code,
        )),
        FinalSemanticAnalysisError::CharacterDialogueApplicationOnlyField { field, field_span } => {
            Some(character_dialogue_application_only_diagnostic(
                field, field_span, code,
            ))
        }
        FinalSemanticAnalysisError::CharacterDialogueFieldNotClearable {
            field,
            field_span,
            declaration_span,
        } => Some(character_dialogue_not_clearable_diagnostic(
            field,
            field_span,
            declaration_span,
            code,
        )),
        _ => None,
    }
}

fn unknown_character_dialogue_field_diagnostic(
    name: &str,
    field_span: &SourceSpan,
    scope: &CanonicalModulePath,
    code: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        format!("unknown CharacterDialogue custom field `{name}` in module {scope:?}"),
    )
    .with_code(code)
    .with_label(DiagnosticLabel::primary(
        field_span.clone(),
        Some("no accepted custom-field binding matches this name".to_owned()),
    ))
}

fn duplicate_character_dialogue_field_diagnostic(
    coordinate: &CharacterDialogueFieldCoordinate,
    first_span: &SourceSpan,
    duplicate_span: &SourceSpan,
    code: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        format!("CharacterDialogue field {coordinate:?} is configured more than once"),
    )
    .with_code(code)
    .with_label(DiagnosticLabel::primary(
        duplicate_span.clone(),
        Some("this field repeats the same semantic coordinate".to_owned()),
    ))
    .with_label(DiagnosticLabel::secondary(
        first_span.clone(),
        Some("the coordinate was first configured here".to_owned()),
    ))
}

fn character_dialogue_type_mismatch_diagnostic(
    field: &CharacterDialogueCustomFieldId,
    declared: &TypeKind,
    actual: &TypeKind,
    value_span: &SourceSpan,
    declaration_span: &SourceSpan,
    code: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        format!(
            "CharacterDialogue custom field `{field}` expects {}, found {}",
            declared.source_label(),
            actual.source_label(),
        ),
    )
    .with_code(code)
    .with_label(DiagnosticLabel::primary(
        value_span.clone(),
        Some("this value does not match the accepted field type".to_owned()),
    ))
    .with_label(DiagnosticLabel::secondary(
        declaration_span.clone(),
        Some("the custom-field type is declared here".to_owned()),
    ))
}

fn character_dialogue_application_only_diagnostic(
    field: &str,
    field_span: &SourceSpan,
    code: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        format!(
            "CharacterDialogue field `{field}` is only valid on an immediate content application"
        ),
    )
    .with_code(code)
    .with_label(DiagnosticLabel::primary(
        field_span.clone(),
        Some("move this coordinate to the outer content application".to_owned()),
    ))
}

fn character_dialogue_not_clearable_diagnostic(
    field: &CharacterDialogueCustomFieldId,
    field_span: &SourceSpan,
    declaration_span: &SourceSpan,
    code: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        format!("CharacterDialogue custom field `{field}` cannot be cleared"),
    )
    .with_code(code)
    .with_label(DiagnosticLabel::primary(
        field_span.clone(),
        Some("`None` requests Clear for this non-clearable field".to_owned()),
    ))
    .with_label(DiagnosticLabel::secondary(
        declaration_span.clone(),
        Some("the accepted custom-field descriptor is declared here".to_owned()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_call_failure_carrier_retains_exact_abort_payload() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root module");
        let owner = module
            .expressions()
            .next()
            .map(|(owner, _)| owner)
            .expect("expression owner");
        let carrier = FinalCallConstraintFailure::new(
            owner,
            CallAnalysisFailure::Abort(crate::types::constraints::TypeConstraintAbort::Cancelled),
        );
        assert_eq!(carrier.owner(), owner);
        assert!(matches!(
            carrier.failure.as_ref(),
            CallAnalysisFailure::Abort(crate::types::constraints::TypeConstraintAbort::Cancelled)
        ));
        assert_eq!(carrier, carrier.clone());
        let debug = format!("{carrier:?}");
        assert!(debug.contains("FinalCallConstraintFailure"));
        assert!(debug.contains("Cancelled"));
        assert!(debug.contains("Abort"));
    }

    #[test]
    fn public_call_seal_carrier_retains_typed_site_and_invariant_provenance() {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root module");
        let owner = module
            .expressions()
            .next()
            .map(|(owner, _)| owner)
            .expect("expression owner");
        let location = FinalCallSealLocation::Site(CheckedCallSite::HirCall(owner));
        let carrier = FinalCallSealFailure::new(
            location,
            CallConstraintInvariant::PreparedFunctionTypeMismatch,
        );

        assert_eq!(carrier.location(), location);
        assert!(matches!(
            carrier.failure.as_ref(),
            CallConstraintInvariant::PreparedFunctionTypeMismatch
        ));
        assert_eq!(carrier, carrier.clone());
        assert!(format!("{carrier:?}").contains("PreparedFunctionTypeMismatch"));
        assert!(format!("{carrier}").contains("prepared callable function type"));
    }
}
