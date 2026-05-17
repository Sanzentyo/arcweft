//! Semantic analysis for Arcweft HIR.
//!
//! This crate owns name resolution, symbol collection, and the current minimal
//! type-checking pass. It depends on parsed syntax and HIR, but parser/runtime
//! crates do not depend on it.

mod check;
mod fact_layer;
mod resolve;
mod semantic;
mod symbols;

pub use check::{
    EntityKind, HandleState, TypeCheckEnv, TypeCheckError, TypeCheckReadinessError, TypeKind,
    typecheck_hir, validate_typecheck_ready,
};
pub use resolve::{NameRegistry, NameResolutionError, registry_from_hir, validate_hir_references};
pub use semantic::{
    SemanticDiagnostic, SemanticDischarge, SemanticMode, SemanticObligation,
    SemanticObligationKind, SemanticPolicy, SemanticProofSummary, SemanticReport, SemanticSeverity,
    SemanticSourceSpan, SemanticTrustedAxiomSummary, SemanticUnsafeAuditSummary, analyze_semantics,
};
pub use symbols::{SymbolUse, SymbolUseKind, collect_symbol_uses};

#[cfg(test)]
mod tests;
