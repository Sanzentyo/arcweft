//! Public type-checking facade.
//!
//! The implementation lives in `checker` so this module stays small and stable
//! as borrow, lifetime, and language-family checkers are split out.

pub use crate::checker::{
    TypeCheckReport, TypeCheckStats, TypeJudgment, TypeJudgmentId, TypeJudgmentRule,
    TypeJudgmentSubject, analyze_types, typecheck_hir, validate_typecheck_ready,
};
