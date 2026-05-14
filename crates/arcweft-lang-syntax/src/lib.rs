//! Surface parser for `.awft` source files.
//!
//! This crate owns syntax-level parsing only. It keeps enough structure for
//! formatter, diagnostics, and later HIR lowering, while deliberately avoiding
//! type resolution or runtime semantics.

mod ast;
mod check;
mod expr;
mod lower;
mod parser;
mod pattern;
mod resolve;
mod symbols;
mod text;
mod types;

pub use ast::{
    Attribute, AwaitBranch, AwaitBranchKind, BorrowBlock, CallableItem, CallableKind, ChoiceAction,
    ChoiceBlock, ChoiceItem, ChoiceMatchArm, ChoiceOption, ChoicePlan, ChoicePlanItem, ContentCall,
    ContractClause, DialogueToken, EntityDeclItem, EntityDeclKind, EntityRef, EnumItem,
    EnumVariant, ExternModItem, Flow, FlowItem, FlowKind, ForBlock, FunctionItem, FunctionKind,
    HookItem, IfBlock, IfLetBlock, ImplItem, ImplMember, Item, LineOptions, LinePlan, LinePlanItem,
    LoopBlock, MatchArm, MatchBlock, MemoFn, ModuleDecl, ParserItem, Pattern, RecordPatternField,
    ScenarioCommand, ScopeBlock, ScopeExprBlock, SelectBlock, SelectBranch, SelectBranchHead,
    SourceItem, SourceLocaleBlock, SpeakerLine, StateField, StateItem, Stmt, StructField,
    StructItem, SyntaxTree, TextRange, TraitItem, TraitMember, TypeAliasItem, UseItem,
    VariantPatternPayload, Visibility, WhileBlock, WhileLetBlock, WikiLink,
};
pub use check::{
    EntityKind, TypeCheckEnv, TypeCheckError, TypeCheckReadinessError, TypeKind, typecheck_hir,
    validate_typecheck_ready,
};
pub use expr::{BinaryOp, ComputationBlockKind, Expr, Literal, Placeholder, UnaryOp, parse_expr};
pub use lower::{
    HirAwait, HirAwaitBranch, HirBorrow, HirChoice, HirChoiceOption, HirDialogue, HirFlow,
    HirFlowItem, HirFor, HirIf, HirIfLet, HirLoop, HirLowerError, HirMatch, HirMatchArm, HirModule,
    HirScope, HirSelect, HirSelectBranch, HirTopLevelDecl, HirWhile, HirWhileLet, lower_to_hir,
};
pub use parser::{ParseError, RecoverySuggestion, parse_source, parse_stub};
pub use resolve::{NameRegistry, NameResolutionError, registry_from_hir, validate_hir_references};
pub use symbols::{SymbolUse, SymbolUseKind, collect_symbol_uses};
pub use text::parse_dialogue_tokens;
pub use types::{
    FnSignature, LifetimeName, TypeParseError, TypeRef, parse_fn_signature, parse_type_ref,
};

#[cfg(test)]
mod tests;
