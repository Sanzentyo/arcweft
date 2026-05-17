//! Surface parser for `.awft` source files.
//!
//! This crate owns syntax-level parsing only. It keeps enough structure for
//! formatter, diagnostics, and later HIR lowering, while deliberately avoiding
//! type resolution or runtime semantics.

mod ast;
mod cst;
mod expr;
mod lint;
mod parser;
mod pattern;
mod source;
mod text;
mod types;

pub use ast::{
    Attribute, AwaitBranch, AwaitBranchKind, AwaitWith, BenchItem, BlockStyle, BorrowBlock,
    CallableItem, CallableKind, CancelRuleSyntax, ChoiceAction, ChoiceBlock, ChoiceItem,
    ChoiceMatchArm, ChoiceOption, ChoicePlan, ChoicePlanItem, ContentCall, ContractClause,
    DeferOutcome, DialogueContent, DialogueDefaultOption, DialogueDefaultsItem, DialogueToken,
    DocBlock, EntityDeclItem, EntityDeclKind, EntityRef, EntityRefSyntax, EnumItem, EnumVariant,
    ExternModItem, Flow, FlowItem, FlowKind, ForBlock, FunctionItem, FunctionKind, HookItem, IdRef,
    IfBlock, IfLetBlock, ImplItem, ImplMember, Item, LineArg, LineMark, LineOptions, LinePlan,
    LinePlanItem, LoopBlock, MatchArm, MatchBlock, MemoFn, ModuleDecl, ParserItem, Pattern,
    ProofClause, ProofItem, RecordPatternField, RelativeId, RelativeIdSpelling, ScenarioCommand,
    ScopeBlock, ScopeExprBlock, SelectBlock, SelectBranch, SelectBranchHead,
    SourceBackpressurePolicy, SourceEventPattern, SourceHandler, SourceHeader, SourceItem,
    SourceLocaleBlock, SourceOverflowPolicy, SourcePrivacyPolicy, SourceReplayPolicy, SpeakerLine,
    StateField, StateItem, Stmt, StmtMatchArm, StructField, StructItem, TestItem, TestKind,
    TextRange, ThreadBlock, ThreadModifier, TraitItem, TraitMember, TriggerPattern,
    TrustedAxiomItem, TypeAliasItem, TypedSyntaxTree, UseItem, VariantPatternPayload, Visibility,
    WaitTarget, WhileBlock, WhileLetBlock, WikiLink,
};
pub use cst::{
    ArcweftLanguage, CstLine, CstLineEvents, CstLineKind, RowanTextRange, SyntaxElement,
    SyntaxKind, SyntaxNode, SyntaxToken, TextSize, cst_lines,
};
pub use expr::{
    BinaryOp, ComputationBlockKind, DurationUnit, Expr, LifetimeAccessMode, LifetimeKey,
    LifetimeScopeKind, Literal, MatchExprArm, Placeholder, UnaryOp, parse_expr,
};
pub use lint::{SyntaxLint, SyntaxLintCode, lint_id_policy};
pub use parser::{ParseError, RecoverySuggestion, parse_source};
pub use source::{LineIndex, ParsedSource, SourceHash};
pub use text::parse_dialogue_tokens;
pub use types::{
    FnParamGroup, FnSignature, GenericParam, LifetimeName, TypeParseError, TypeRef, WhereClause,
    parse_fn_signature, parse_type_ref,
};

#[cfg(test)]
mod tests;
