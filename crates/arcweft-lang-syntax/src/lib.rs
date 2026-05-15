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
mod source;
mod symbols;
mod text;
mod types;

pub use ast::{
    Attribute, AwaitBranch, AwaitBranchKind, BlockStyle, BorrowBlock, CallableItem, CallableKind,
    ChoiceAction, ChoiceBlock, ChoiceItem, ChoiceMatchArm, ChoiceOption, ChoicePlan,
    ChoicePlanItem, ContentCall, ContractClause, DialogueDefaultOption, DialogueDefaultsItem,
    DialogueToken, DocBlock, EntityDeclItem, EntityDeclKind, EntityRef, EnumItem, EnumVariant,
    ExternModItem, Flow, FlowItem, FlowKind, ForBlock, FunctionItem, FunctionKind, HookItem,
    IfBlock, IfLetBlock, ImplItem, ImplMember, Item, LineArg, LineOptions, LinePlan, LinePlanItem,
    LoopBlock, MatchArm, MatchBlock, MemoFn, ModuleDecl, ParserItem, Pattern, RecordPatternField,
    ScenarioCommand, ScopeBlock, ScopeExprBlock, SelectBlock, SelectBranch, SelectBranchHead,
    SourceItem, SourceLocaleBlock, SpeakerLine, StateField, StateItem, Stmt, StructField,
    StructItem, TextRange, ThreadBlock, ThreadModifier, TraitItem, TraitMember, TypeAliasItem,
    TypedSyntaxTree, UseItem, VariantPatternPayload, Visibility, WhileBlock, WhileLetBlock,
    WikiLink,
};
pub use check::{
    EntityKind, TypeCheckEnv, TypeCheckError, TypeCheckReadinessError, TypeKind, typecheck_hir,
    validate_typecheck_ready,
};
pub use cst::{
    ArcweftLanguage, CstLine, CstLineEvents, CstLineKind, RowanTextRange, SyntaxElement,
    SyntaxKind, SyntaxNode, SyntaxToken, TextSize, cst_lines,
};
pub use expr::{BinaryOp, ComputationBlockKind, Expr, Literal, Placeholder, UnaryOp, parse_expr};
pub use lint::{SyntaxLint, SyntaxLintCode, lint_id_policy};
pub use lower::{
    HirAwait, HirAwaitBranch, HirBorrow, HirChoice, HirChoiceOption, HirDialogue, HirFlow,
    HirFlowItem, HirFor, HirIf, HirIfLet, HirLoop, HirLowerError, HirMatch, HirMatchArm, HirModule,
    HirScope, HirSelect, HirSelectBranch, HirTopLevelDecl, HirWhile, HirWhileLet, lower_to_hir,
};
pub use parser::{ParseError, RecoverySuggestion, parse_source};
pub use resolve::{NameRegistry, NameResolutionError, registry_from_hir, validate_hir_references};
pub use source::{LineIndex, ParsedSource, SourceHash};
pub use symbols::{SymbolUse, SymbolUseKind, collect_symbol_uses};
pub use text::parse_dialogue_tokens;
pub use types::{
    FnParamGroup, FnSignature, GenericParam, LifetimeName, TypeParseError, TypeRef, WhereClause,
    parse_fn_signature, parse_type_ref,
};

#[cfg(test)]
mod tests;
