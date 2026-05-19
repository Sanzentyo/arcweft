//! Surface parser for `.awft` source files.
//!
//! This crate owns syntax-level parsing only. It keeps enough structure for
//! formatter, diagnostics, and later HIR lowering, while deliberately avoiding
//! type resolution or runtime semantics.

pub mod ast;
mod cst;
mod expr;
mod lint;
mod parser;
mod pattern;
mod source;
mod text;
mod types;

pub use ast::choice::{
    ChoiceAction, ChoiceBlock, ChoiceItem, ChoiceMatchArm, ChoiceOption, ChoicePlan, ChoicePlanItem,
};
pub use ast::common::{DocBlock, ModuleDecl, TextRange, UseItem, Visibility};
pub use ast::dialogue::{
    ContentCall, DialogueContent, DialogueDefaultOption, DialogueDefaultsItem, DialogueToken,
    LineArg, LineMark, LineOptions, ScenarioCommand, SpeakerLine,
};
pub use ast::flow::{
    AwaitBranch, AwaitBranchKind, AwaitWith, BorrowBlock, ContractClause, Flow, FlowItem, FlowKind,
    ForBlock, IfBlock, IfLetBlock, LoopBlock, MatchArm, MatchBlock, ScopeBlock, ScopeExprBlock,
    SelectBlock, SelectBranch, SelectBranchHead, SourceLocaleBlock, Stmt, StmtMatchArm,
    ThreadBlock, ThreadModifier, WaitTarget, WhileBlock, WhileLetBlock,
};
pub use ast::ids::{EntityRef, EntityRefSyntax, IdRef, RelativeId, RelativeIdSpelling, WikiLink};
pub use ast::items::{
    Attribute, CallableItem, CallableKind, EntityDeclItem, EntityDeclKind, EnumItem, EnumVariant,
    ExternModItem, FunctionItem, FunctionKind, HookItem, ImplItem, ImplMember, Item, MemoFn,
    ParserItem, RawSyntax, RawSyntaxFamily, StateField, StateItem, StructField, StructItem,
    TraitItem, TraitMember, TypeAliasItem, TypedSyntaxTree,
};
pub use ast::line_plan::{
    BlockStyle, CancelRuleSyntax, DeferOutcome, LinePlan, LinePlanItem, TriggerPattern,
};
pub use ast::pattern::{Pattern, RecordPatternField, VariantPatternPayload};
pub use ast::proof::{BenchItem, ProofClause, ProofItem, TestItem, TestKind, TrustedAxiomItem};
pub use ast::source::{
    SourceBackpressurePolicy, SourceEventPattern, SourceHandler, SourceHeader, SourceItem,
    SourceOverflowPolicy, SourcePrivacyPolicy, SourceReplayPolicy,
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
