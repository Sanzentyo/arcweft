//! Public type-checking facade.
//!
//! The implementation lives in `checker` so this module stays small and stable
//! as borrow, lifetime, and language-family checkers are split out.

pub use crate::{
    checker::{
        CallableExecutionMode, CheckedCallableExecution, ClosureCapture, ClosureCaptureInventory,
        DataLastMethodFallbackArg, ForIterationEvidence, ForIterationEvidenceFamily,
        NumericFallback, NumericFallbackKind, ProjectCallableReference, ProjectEntityReference,
        StandardIteratorFamily, StreamGeneratorFacts, TypeCheckReport, TypeCheckStats,
        TypeExpressionId, TypeJudgment, TypeJudgmentExpected, TypeJudgmentId, TypeJudgmentRule,
        TypeJudgmentSubject, TypedLoweringEvidence, TypedLoweringEvidenceKind,
        analyze_registered_project_types, analyze_types, typecheck_hir, validate_typecheck_ready,
    },
    diagnostics::TypeCheckWarning,
    effect_analysis::{EffectTraceReport, EffectTraceSummary},
    effect_row::{
        ClosedEffectRowReport, ClosedEffectRowSummary, EffectRow, EffectRowCloseError,
        EffectRowError, EffectRowReport, EffectRowSummary, EffectRowTail, EffectSubstitution,
        EffectVar, EffectVarSupply,
    },
    propagation::{
        CheckedReturnType, PropagationBarrierEvidence, PropagationBoundaryEvidence,
        PropagationBoundaryKind, PropagationTargetEvidence, TryPropagationOperand,
    },
};
