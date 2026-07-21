//! Lossless CST layer for Arcweft source.
//!
//! This module owns the rowan language binding and the token stream used to
//! build a syntax tree that survives malformed input. Typed AST/HIR lowering can
//! be rebuilt from this tree without losing comments, whitespace, or source
//! offsets.

use rowan::{GreenNodeBuilder, Language};
use std::borrow::Cow;
use std::ops::Range;

pub mod classify;
pub mod entity_ref;
pub mod lexer;
pub mod line;
pub mod path;
pub mod punctuation;
pub mod text;

/// Rowan language marker for Arcweft syntax nodes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArcweftLanguage {}

impl Language for ArcweftLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SyntaxKind::from_raw(raw)
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into_raw()
    }
}

/// Lossless syntax node.
pub type SyntaxNode = rowan::SyntaxNode<ArcweftLanguage>;

/// Lossless syntax token.
pub type SyntaxToken = rowan::SyntaxToken<ArcweftLanguage>;

/// Lossless syntax element.
pub type SyntaxElement = rowan::SyntaxElement<ArcweftLanguage>;

/// Rowan text range type used by CST nodes and tokens.
pub type RowanTextRange = rowan::TextRange;

/// Rowan text size type used by CST nodes and tokens.
pub type TextSize = rowan::TextSize;

/// Returns whether `value` is exactly one canonical Arcweft identifier token.
///
/// This shares the lossless lexer's Unicode-letter/underscore start rule and
/// ASCII-digit continuation rule so semantic and lowering layers do not grow
/// subtly different identifier predicates.
pub fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(lexer::is_ident_start) && chars.all(lexer::is_ident_continue)
}

/// Lossless source line projected from CST line nodes.
///
/// This is the typed parser's temporary event input while the full grammar is
/// migrated onto rowan events. It is derived from CST ranges instead of a
/// separate raw-source line splitter, so source offsets stay tied to the
/// lossless tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CstLine<'a> {
    pub(crate) text: Cow<'a, str>,
    pub(crate) start: usize,
    pub(crate) end: usize,
    trim_start: usize,
    trim_end: usize,
    leading_trim_start: usize,
    punctuation: CstLinePunctuationSummary,
    kind: CstLineKind,
}

/// Path-free syntax parser counters used by profiling and benchmarks.
///
/// These fields are stable and always present. Default parsing updates only
/// counters that are available as by-products of normal parser work; fields
/// that would require timing, tracing, or extra scans remain zero until a
/// detailed instrumentation mode is added.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SyntaxParseStats {
    pub cst_lex_passes: usize,
    pub punctuation_scans: usize,
    pub punctuation_scan_bytes: usize,
    pub line_owned_bytes: usize,
    pub block_owned_bytes: usize,
    pub raw_owned_bytes: usize,
    pub wiki_scan_performed: usize,
    pub dialogue_rescue_expr_parse_attempts: usize,
    pub numeric_seq_summaries: usize,
    pub prefix_depth_limit_failures: usize,
}

impl SyntaxParseStats {
    pub(crate) fn checked_add_prefix_depth_limit_failures(&mut self, additional: usize) -> bool {
        let Some(total) = self.prefix_depth_limit_failures.checked_add(additional) else {
            self.prefix_depth_limit_failures = usize::MAX;
            return false;
        };
        self.prefix_depth_limit_failures = total;
        true
    }
}

/// Per-line punctuation depth summary computed once while projecting CST lines.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CstLinePunctuationSummary {
    brace_delta: i32,
    paren_delta: i32,
    bracket_delta: i32,
    first_top_level_brace_open: Option<usize>,
    last_top_level_brace_open: Option<usize>,
    last_brace_close: Option<usize>,
}

/// Open-minus-close depth deltas for all bracket families in one scan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CstPunctuationDeltas {
    pub(crate) brace: i32,
    pub(crate) paren: i32,
    pub(crate) bracket: i32,
}

/// Parsed `=== ... ===` fence used by flat dialogue and scope sugar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatFence<'a> {
    pub kind: &'a str,
    pub head: &'a str,
    pub close: bool,
    pub head_start: usize,
}

/// Relative ID token split from an ID-bearing context.
///
/// `body` excludes the relative marker. `parent_depth` is zero for the current
/// ID scope, one for the parent scope, and so on. The split keeps the marker
/// length so parser ranges can cover the exact source spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CstRelativeId<'a> {
    pub(crate) body: &'a str,
    pub(crate) parent_depth: usize,
    pub(crate) spelling: CstRelativeIdSpelling,
    pub(crate) marker_len: usize,
    pub(crate) rest: &'a str,
}

/// Source spelling used by a relative ID token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CstRelativeIdSpelling {
    DotRun,
    SuperChain,
}

/// Family-qualified relative entity reference split from a source fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CstRelativeEntityRef<'a> {
    pub(crate) raw: &'a str,
    pub(crate) family: &'a str,
    pub(crate) relative: CstRelativeId<'a>,
    pub(crate) rest: &'a str,
}

/// Entity reference token split from a source fragment.
///
/// The CST layer owns marker handling for canonical `@` refs so the typed
/// parser does not duplicate sigil and delimited-body slicing rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CstEntityRef<'a> {
    pub(crate) raw: &'a str,
    pub(crate) body: &'a str,
    pub(crate) delimited: bool,
    pub(crate) closed: bool,
    pub(crate) rest: &'a str,
}

/// Coarse line-event kind projected from CST tokens.
///
/// This is intentionally line-level rather than grammar-level. It removes
/// comment/doc/blank classification from the typed parser while the parser is
/// migrating toward richer rowan events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CstLineKind {
    Blank,
    Comment,
    DocComment,
    Code,
}

/// Top-level line event classification used before typed item construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CstTopLevelLineKind {
    Attribute,
    Module,
    Use,
    Item,
}

/// Top-level item classification projected from a code line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CstTopLevelItemKind {
    Flow,
    Function,
    Trait,
    Impl,
    Enum,
    Struct,
    TypeAlias,
    EntityDecl,
    Entry,
    ExternCapability,
    ExternMod,
    Proof,
    Test,
    Bench,
    Source,
    Style,
    Unrecognized,
}

/// Flow-body line classification before typed AST construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CstFlowItemKind {
    StructuredBlock(CstStructuredFlowBlockKind),
    Include,
    AwaitWith,
    Let(CstLetFlowItemKind),
    TypedStmt,
    Other,
}

/// Structured flow block head classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CstStructuredFlowBlockKind {
    Choice,
    IfLet,
    If,
    Match,
    Loop,
    WhileLet,
    While,
    For,
    Select,
    Thread,
    Defer,
    UnsafeLifetime,
    SourceLocale,
    BareScope,
    Scope,
}

/// `let ... = ...` flow statement classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CstLetFlowItemKind {
    Choice,
    DialogueCall,
    Scope,
    ComputationBlock,
    Block,
    Loop,
    AwaitWith,
    AwaitStart,
    IfLet,
    If,
    Match,
    LetElse,
    Plain,
}

/// Typed statement head classification used before AST construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CstStmtKind {
    LifetimeSet,
    Wait,
    Let,
    DeferBlock,
    Defer,
    ControlTransfer,
    On,
    UnsafeLifetime,
    Braced,
    AmbiguousBlockHead,
    Expr,
}

/// Rule used when collecting a balanced brace block from line events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CstBlockOpenRule {
    FirstTopLevel,
    FlowBody,
    FunctionBody,
}

/// Balanced brace block projected from CST line events.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CstBlockEvent<'a> {
    pub(crate) head: Cow<'a, str>,
    pub(crate) head_range: Option<Range<usize>>,
    pub(crate) body: Cow<'a, str>,
    pub(crate) body_range: Option<Range<usize>>,
    pub(crate) end: usize,
    pub(crate) ok: bool,
    pub(crate) next_index: usize,
    pub(crate) body_line_range: Option<Range<usize>>,
}

/// Ordered line-event stream projected from the lossless CST.
///
/// The typed parser consumes this newtype instead of an unlabelled `Vec`, which
/// keeps the current line-event bridge explicit while later parser work moves
/// toward grammar-level rowan events.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CstLineEvents<'a> {
    lines: Vec<CstLine<'a>>,
    source: Option<&'a str>,
    stats: SyntaxParseStats,
}

/// Arcweft CST node and token kinds.
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxKind {
    Root,
    Line,
    Error,
    Whitespace,
    Newline,
    Comment,
    DocComment,
    Ident,
    Number,
    String,
    EntityRef,
    Punctuation,
    Text,
}

impl SyntaxKind {
    const fn from_raw(raw: rowan::SyntaxKind) -> Self {
        match raw.0 {
            0 => Self::Root,
            1 => Self::Line,
            3 => Self::Whitespace,
            4 => Self::Newline,
            5 => Self::Comment,
            6 => Self::DocComment,
            7 => Self::Ident,
            8 => Self::Number,
            9 => Self::String,
            10 => Self::EntityRef,
            11 => Self::Punctuation,
            12 => Self::Text,
            _ => Self::Error,
        }
    }

    const fn into_raw(self) -> rowan::SyntaxKind {
        rowan::SyntaxKind(self as u16)
    }
}

/// Documentation prefix extracted from a text fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CstDocPrefix {
    lines: Vec<String>,
    consumed: usize,
}

/// Builds a lossless CST from source text.
#[must_use]
pub fn parse_cst(source: &str) -> SyntaxNode {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::Root.into_raw());

    let tokens = lexer::lex_cst(source);
    let mut line_open = false;
    for token in tokens {
        if !line_open && token.kind() != SyntaxKind::Newline {
            builder.start_node(SyntaxKind::Line.into_raw());
            line_open = true;
        }

        builder.token(token.kind().into_raw(), token.text());

        if token.kind() == SyntaxKind::Newline && line_open {
            builder.finish_node();
            line_open = false;
        }
    }

    if line_open {
        builder.finish_node();
    }

    builder.finish_node();
    SyntaxNode::new_root(builder.finish())
}

/// Projects CST `Line` nodes into parser input events.
#[must_use]
pub fn cst_lines(root: &SyntaxNode) -> CstLineEvents<'static> {
    CstLineEvents::from(root)
}

/// Projects CST `Line` nodes using the original source as the text backing.
#[must_use]
pub fn cst_lines_for_source<'a>(root: &SyntaxNode, source: &'a str) -> CstLineEvents<'a> {
    CstLineEvents::from_root_and_source(root, source)
}

pub(crate) use classify::classify_stmt;
pub(crate) use entity_ref::{
    split_leading_entity_ref_parts, split_leading_relative_entity_ref, split_leading_relative_id,
    starts_leading_entity_ref, starts_leading_relative_entity_ref, starts_leading_relative_id,
};
#[cfg(test)]
pub(crate) use punctuation::split_top_level_punctuation_sequence_once;
pub(crate) use punctuation::{
    ArcweftPunctuation, CstPunctuationScan, collect_wiki_link_ranges, contains_arcweft_punctuation,
    find_matching_angle_group, find_matching_punctuation, find_top_level_matching_punctuation,
    find_top_level_punctuation, split_first_string_literal,
    split_last_top_level_punctuation_sequence_once, split_top_level_arcweft_punctuation_once,
    split_top_level_keyword_once, split_top_level_punctuation, split_top_level_punctuation_once,
    strip_prefix_arcweft_punctuation, strip_suffix_arcweft_punctuation,
};
#[cfg(test)]
pub(crate) use punctuation::{
    find_last_depth_zero_open_punctuation, find_last_top_level_punctuation,
};
pub(crate) use text::{
    nonempty_trimmed_source_lines, source_line_count, source_line_iter, split_leading_ident,
    split_leading_lifetime, take_doc_comment_prefix,
};
