//! Structured HIR for Arcweft language tooling.
//!
//! HIR lowering depends on surface syntax, but semantic analysis, verifier
//! passes, runtime-plan lowering, CLI, and LSP tooling should import HIR through
//! this crate instead of reaching into parser internals.

mod lower;

pub use arcweft_lang_syntax::{
    Attribute, AwaitBranchKind, AwaitWith, BenchItem, BorrowBlock, CallableItem, CallableKind,
    ChoiceAction, ChoiceBlock, ChoicePlan, ChoicePlanItem, ContractClause, DeferOutcome,
    DialogueDefaultsItem, DialogueToken, EntityDeclItem, EntityDeclKind, EntityRef,
    EntityRefSyntax, EnumItem, Expr, ExternModItem, FlowItem, FlowKind, FunctionKind, HookItem,
    IdRef, ImplItem, LifetimeKey, LifetimeScopeKind, LinePlan, LinePlanItem, MatchArm, MemoFn,
    ParserItem, Pattern, ProofItem, RelativeId, ScopeExprBlock, SourceItem, SourceLocaleBlock,
    StateItem, Stmt, TestItem, TestKind, TextRange, ThreadBlock, TraitItem, TriggerPattern,
    TrustedAxiomItem, TypeAliasItem, TypeRef, WaitTarget,
};
pub use lower::{
    HirAwait, HirAwaitBranch, HirBorrow, HirChoice, HirChoiceOption, HirDialogue, HirFlow,
    HirFlowItem, HirFor, HirFunction, HirIf, HirIfLet, HirLoop, HirLowerError, HirMatch,
    HirMatchArm, HirModule, HirScope, HirScopeExpr, HirSelect, HirSelectBranch, HirSourceLocale,
    HirTopLevelDecl, HirWhile, HirWhileLet, lower_to_hir,
};
