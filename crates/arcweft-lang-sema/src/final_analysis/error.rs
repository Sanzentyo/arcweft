//! Publication and semantic-analysis failures.

use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceSpan};
use thiserror::Error;

use super::{
    AssertionContext, AssertionMode, CheckedRichTextReport, EffectSet, ExprId, ItemId, LocalId,
    PatternId, StmtId, TypeId, TypeKind,
};
use crate::callable::{CheckedCallableId, UnknownCallKind};

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PropagationOperator {
    Try,
    Await,
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
    #[error("semantic fact references a foreign or missing HIR owner")]
    InvalidOwner,
    #[error("semantic fact does not match its final-HIR payload family")]
    WrongPayloadFamily,
    #[error("semantic fact references a recovered HIR payload")]
    RecoveredOwner,
    #[error("semantic type contains a poison carrier and cannot enter an executable report")]
    PoisonedType,
    #[error("semantic fact references an invalid project nominal owner")]
    InvalidNominalOwner,
    #[error("semantic fact references an invalid project callable owner")]
    InvalidCallableOwner,
    #[error("call diagnostic source does not belong to the accepted project generation")]
    DiagnosticSourceMismatch,
    #[error("semantic effect row is not closed")]
    OpenEffectRow,
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
    #[error("semantic value resolution failed for expression {owner:?}")]
    ValueResolutionFailed { owner: ExprId },
    #[error("shared callable resolution failed for expression {owner:?}")]
    CallResolutionFailed { owner: ExprId },
    #[error("unknown function `{name}`")]
    UnknownCallTarget {
        owner: ExprId,
        kind: UnknownCallKind,
        name: String,
        call_source: SourceSpan,
    },
    #[error(
        "{operator:?} propagates error type {operand_error:?}, but the enclosing return boundary requires {return_error:?}"
    )]
    PropagationErrorMismatch {
        owner: ExprId,
        operator: PropagationOperator,
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

impl FinalSemanticAnalysisError {
    /// Stable compiler/LSP diagnostic code owned by the final semantic authority.
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::AssertionModeNotAllowed { .. } => "sema.assert.context",
            Self::AssertionConditionNotBool { .. } => "sema.assert.condition_not_bool",
            Self::AssertionConditionNotPure { .. } => "sema.assert.condition_not_pure",
            Self::RecursiveCallableContract { .. } => "sema.callable.recursive_contract",
            Self::UnknownCallTarget { .. } => "sema.call.unknown_target",
            Self::PropagationErrorMismatch {
                operator: PropagationOperator::Try,
                ..
            } => "sema.try.error_mismatch",
            Self::PropagationErrorMismatch {
                operator: PropagationOperator::Await,
                ..
            } => "sema.await.error_mismatch",
            Self::EffectUpperBoundExceeded { .. } => "AWF-EFX-001",
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
            } => Some(
                Diagnostic::new(
                    DiagnosticSeverity::Error,
                    match kind {
                        UnknownCallKind::Free => format!("unknown function `{name}`"),
                        UnknownCallKind::Method => format!("unknown method `{name}`"),
                        UnknownCallKind::AssociatedType => {
                            format!("unknown associated function `{name}`")
                        }
                    },
                )
                .with_code(self.diagnostic_code())
                .with_label(DiagnosticLabel::primary(
                    call_source.clone(),
                    Some("no registered callable candidate matches this target".to_owned()),
                )),
            ),
            Self::PropagationErrorMismatch {
                operator,
                operand_error,
                return_error,
                operator_source,
                return_source,
                ..
            } => Some(
                Diagnostic::new(
                    DiagnosticSeverity::Error,
                    format!(
                        "{} propagates error type {}, but the enclosing return boundary requires {}",
                        match operator {
                            PropagationOperator::Try => "try",
                            PropagationOperator::Await => "await",
                        },
                        operand_error.source_label(),
                        return_error.source_label(),
                    ),
                )
                .with_code(self.diagnostic_code())
                .with_label(DiagnosticLabel::primary(
                    operator_source.clone(),
                    Some("propagated error type does not match this boundary".to_owned()),
                ))
                .with_label(DiagnosticLabel::secondary(
                    return_source.clone(),
                    Some("enclosing return error is declared here".to_owned()),
                )),
            ),
            Self::EffectUpperBoundExceeded {
                callable,
                missing,
                contract_source,
                trace_notes,
                ..
            } => {
                let mut diagnostic = Diagnostic::new(
                    DiagnosticSeverity::Error,
                    format!(
                        "callable `{callable}` performs effects {missing} outside its declared upper bound"
                    ),
                )
                .with_code(self.diagnostic_code())
                .with_label(DiagnosticLabel::primary(
                    contract_source.clone(),
                    Some("declared effect upper bound is exceeded".to_owned()),
                ));
                for note in trace_notes {
                    diagnostic = diagnostic.with_note(note.clone());
                }
                Some(diagnostic)
            }
            _ => None,
        }
    }
}
