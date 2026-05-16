//! Surface parser for `.awft` source files.
//!
//! This crate owns syntax-level parsing only. It keeps enough structure for
//! formatter, diagnostics, and later HIR lowering, while deliberately avoiding
//! type resolution or runtime semantics.

mod ast;
mod check;
mod cst;
mod expr;
mod lint;
mod lower;
mod parser;
mod pattern;
mod resolve;
mod runtime_plan;
mod source;
mod symbols;
mod text;
mod types;

pub use ast::{
    Attribute, AwaitBranch, AwaitBranchKind, AwaitWith, BenchItem, BlockStyle, BorrowBlock,
    CallableItem, CallableKind, ChoiceAction, ChoiceBlock, ChoiceItem, ChoiceMatchArm,
    ChoiceOption, ChoicePlan, ChoicePlanItem, ContentCall, ContractClause, DeferOutcome,
    DialogueDefaultOption, DialogueDefaultsItem, DialogueToken, DocBlock, EntityDeclItem,
    EntityDeclKind, EntityRef, EntityRefSyntax, EnumItem, EnumVariant, ExternModItem, Flow,
    FlowItem, FlowKind, ForBlock, FunctionItem, FunctionKind, HookItem, IdRef, IfBlock, IfLetBlock,
    ImplItem, ImplMember, Item, LineArg, LineOptions, LinePlan, LinePlanItem, LoopBlock, MatchArm,
    MatchBlock, MemoFn, ModuleDecl, ParserItem, Pattern, ProofItem, RecordPatternField,
    ScenarioCommand, ScopeBlock, ScopeExprBlock, SelectBlock, SelectBranch, SelectBranchHead,
    SourceItem, SourceLocaleBlock, SpeakerLine, StateField, StateItem, Stmt, StructField,
    StructItem, TestItem, TestKind, TextRange, ThreadBlock, ThreadModifier, TraitItem, TraitMember,
    TriggerPattern, TrustedAxiomItem, TypeAliasItem, TypedSyntaxTree, UseItem,
    VariantPatternPayload, Visibility, WaitTarget, WhileBlock, WhileLetBlock, WikiLink,
};
pub use check::{
    EntityKind, HandleState, TypeCheckEnv, TypeCheckError, TypeCheckReadinessError, TypeKind,
    typecheck_hir, validate_typecheck_ready,
};
pub use cst::{
    ArcweftLanguage, CstLine, CstLineEvents, CstLineKind, RowanTextRange, SyntaxElement,
    SyntaxKind, SyntaxNode, SyntaxToken, TextSize, cst_lines,
};
pub use expr::{
    BinaryOp, ComputationBlockKind, Expr, LifetimeAccessMode, LifetimeKey, LifetimeScopeKind,
    Literal, Placeholder, UnaryOp, parse_expr,
};
pub use lint::{SyntaxLint, SyntaxLintCode, lint_id_policy};
pub use lower::{
    HirAwait, HirAwaitBranch, HirBorrow, HirChoice, HirChoiceOption, HirDialogue, HirFlow,
    HirFlowItem, HirFor, HirFunction, HirIf, HirIfLet, HirLoop, HirLowerError, HirMatch,
    HirMatchArm, HirModule, HirScope, HirScopeExpr, HirSelect, HirSelectBranch, HirSourceLocale,
    HirTopLevelDecl, HirWhile, HirWhileLet, lower_to_hir,
};
pub use parser::{ParseError, RecoverySuggestion, parse_source};
pub use resolve::{NameRegistry, NameResolutionError, registry_from_hir, validate_hir_references};
pub use runtime_plan::{LinePlanLowerError, LoweredLineTaskGroup, lower_line_task_groups};
pub use source::{LineIndex, ParsedSource, SourceHash};
pub use symbols::{SymbolUse, SymbolUseKind, collect_symbol_uses};
pub use text::parse_dialogue_tokens;
pub use types::{
    FnParamGroup, FnSignature, GenericParam, LifetimeName, TypeParseError, TypeRef, WhereClause,
    parse_fn_signature, parse_type_ref,
};

#[cfg(test)]
mod tests;
