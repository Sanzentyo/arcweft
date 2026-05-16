//! Structured HIR facade for Arcweft language tooling.
//!
//! The current parser crate still owns the concrete HIR data structures while
//! the compiler is being split. This crate is the stable import boundary for
//! semantic tooling, verifier passes, CLI, and LSP code so those consumers do
//! not reach back into parser internals directly.

pub use arcweft_lang_syntax::{
    BenchItem, ContractClause, EntityRef, HirAwait, HirAwaitBranch, HirBorrow, HirChoice,
    HirChoiceOption, HirDialogue, HirFlow, HirFlowItem, HirFor, HirFunction, HirIf, HirIfLet,
    HirLoop, HirLowerError, HirMatch, HirMatchArm, HirModule, HirScope, HirScopeExpr, HirSelect,
    HirSelectBranch, HirSourceLocale, HirTopLevelDecl, HirWhile, HirWhileLet, IdRef, LinePlan,
    LinePlanItem, Pattern, Stmt, TestItem, TestKind, TextRange, TrustedAxiomItem, TypeCheckEnv,
    TypeCheckError, TypeCheckReadinessError, TypeKind, lower_to_hir, typecheck_hir,
    validate_typecheck_ready,
};
