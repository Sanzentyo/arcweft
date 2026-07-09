//! Public type-checking facade.
//!
//! The implementation lives in `checker` so this module stays small and stable
//! as borrow, lifetime, and language-family checkers are split out.

pub use crate::{
    checker::{
        ClosureCapture, ClosureCaptureInventory, DataLastMethodFallbackArg, ForIterationEvidence,
        ForIterationEvidenceFamily, StandardIteratorFamily, TypeCheckReport, TypeCheckStats,
        TypeExpressionId, TypeJudgment, TypeJudgmentExpected, TypeJudgmentId, TypeJudgmentRule,
        TypeJudgmentSubject, TypedLoweringEvidence, TypedLoweringEvidenceKind, analyze_types,
        typecheck_hir, validate_typecheck_ready,
    },
    diagnostics::TypeCheckWarning,
    effect_analysis::{EffectTraceReport, EffectTraceSummary},
    effect_row::{EffectRow, EffectRowReport, EffectRowSummary, EffectRowTail},
};
