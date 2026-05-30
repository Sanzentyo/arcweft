//! Lossless CST layer for Arcweft source.
//!
//! This module owns the rowan language binding and the token stream used to
//! build a syntax tree that survives malformed input. Typed AST/HIR lowering can
//! be rebuilt from this tree without losing comments, whitespace, or source
//! offsets.

use rowan::{GreenNodeBuilder, Language};
use std::ops::Index;

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

/// Lossless source line projected from CST line nodes.
///
/// This is the typed parser's temporary event input while the full grammar is
/// migrated onto rowan events. It is derived from CST ranges instead of a
/// separate raw-source line splitter, so source offsets stay tied to the
/// lossless tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CstLine {
    pub(crate) text: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
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
    pub dot_normalization_owned: usize,
    pub dialogue_rescue_expr_parse_attempts: usize,
    pub numeric_seq_summaries: usize,
}

/// Per-line punctuation depth summary computed once while projecting CST lines.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CstLinePunctuationSummary {
    brace_delta: i32,
    paren_delta: i32,
    bracket_delta: i32,
    has_top_level_brace_open: bool,
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
    Callable,
    State,
    Trait,
    Impl,
    Enum,
    Struct,
    TypeAlias,
    EntityDecl,
    Entry,
    ExternCapability,
    ExternMod,
    Hook,
    DialogueDefaults,
    MemoFn,
    Proof,
    TrustedAxiom,
    Test,
    Bench,
    Parser,
    Source,
    FlowBodyItemOrRaw,
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
    Borrow,
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
    MemoBlock,
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
pub(crate) struct CstBlockEvent {
    pub(crate) head: String,
    pub(crate) body: String,
    pub(crate) end: usize,
    pub(crate) ok: bool,
    pub(crate) next_index: usize,
}

/// Ordered line-event stream projected from the lossless CST.
///
/// The typed parser consumes this newtype instead of an unlabelled `Vec`, which
/// keeps the current line-event bridge explicit while later parser work moves
/// toward grammar-level rowan events.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CstLineEvents {
    lines: Vec<CstLine>,
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

/// Builds a lossless CST from source text.
#[must_use]
pub fn parse_cst(source: &str) -> SyntaxNode {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::Root.into_raw());

    let tokens = lex_cst(source);
    let mut line_open = false;
    for token in tokens {
        if !line_open && token.kind != SyntaxKind::Newline {
            builder.start_node(SyntaxKind::Line.into_raw());
            line_open = true;
        }

        builder.token(token.kind.into_raw(), token.text());

        if token.kind == SyntaxKind::Newline && line_open {
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
pub fn cst_lines(root: &SyntaxNode) -> CstLineEvents {
    CstLineEvents::from(root)
}

impl From<&SyntaxNode> for CstLineEvents {
    fn from(root: &SyntaxNode) -> Self {
        let lines = root
            .children()
            .filter(|node| node.kind() == SyntaxKind::Line)
            .map(|node| CstLine::from_node(&node))
            .collect::<Vec<_>>();
        let line_owned_bytes = lines.iter().map(|line| line.text.len()).sum();
        let punctuation_scan_bytes = line_owned_bytes;
        Self {
            stats: SyntaxParseStats {
                cst_lex_passes: 1,
                punctuation_scans: lines.len(),
                punctuation_scan_bytes,
                line_owned_bytes,
                ..SyntaxParseStats::default()
            },
            lines,
        }
    }
}

impl CstLineEvents {
    /// Number of projected line events.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Returns true when the source has no non-empty CST line events.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Iterates over projected CST line events.
    pub fn iter(&self) -> impl Iterator<Item = &CstLine> {
        self.lines.iter()
    }

    /// Returns a line event by index.
    pub fn get(&self, index: usize) -> Option<&CstLine> {
        self.lines.get(index)
    }

    /// Path-free counters collected while projecting CST lines.
    pub const fn stats(&self) -> SyntaxParseStats {
        self.stats
    }

    /// Collects a balanced brace block beginning at a line-event index.
    pub(crate) fn collect_brace_block(
        &self,
        start: usize,
        rule: CstBlockOpenRule,
    ) -> CstBlockEvent {
        let Some(first) = self.get(start) else {
            return CstBlockEvent::new(String::new(), String::new(), 0, false, start);
        };
        let mut text = String::new();
        let mut end = first.end;
        let mut depth = 0_i32;
        let mut seen_open = false;
        let mut seen_body_open = false;
        let mut index = start;

        while let Some(line) = self.get(index) {
            let trimmed = line.trimmed();
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&line.text);
            end = line.end;
            if matches!(rule, CstBlockOpenRule::FunctionBody)
                && (trimmed == "{"
                    || line.has_unclosed_top_level_brace_open()
                    || (looks_like_function_item(trimmed) && line.has_top_level_brace_open()))
            {
                seen_body_open = true;
            }
            if line.has_top_level_brace_open() {
                seen_open = true;
            }
            depth += line.brace_delta();
            index += 1;
            if block_event_is_complete(rule, seen_open, seen_body_open, depth) {
                break;
            }
        }

        let open = match rule {
            CstBlockOpenRule::FirstTopLevel => find_top_level_punctuation(&text, '{'),
            CstBlockOpenRule::FlowBody => find_last_top_level_punctuation(&text, '{'),
            CstBlockOpenRule::FunctionBody => find_body_open(&text),
        };
        let Some(open) = open else {
            return CstBlockEvent::new(text, String::new(), end, false, start + 1);
        };
        let Some(close) = find_last_punctuation(&text, '}') else {
            return CstBlockEvent::new(text, String::new(), end, false, index);
        };
        if depth != 0 {
            return CstBlockEvent::new(text, String::new(), end, false, index);
        }
        CstBlockEvent::new(
            text[..open].trim().to_owned(),
            text[open + 1..close].to_owned(),
            end,
            true,
            index,
        )
    }

    /// Collects a flow-like header prelude followed by a balanced brace body.
    pub(crate) fn collect_flow_block(&self, start: usize) -> CstBlockEvent {
        let Some(first) = self.get(start) else {
            return CstBlockEvent::new(String::new(), String::new(), 0, false, start);
        };
        let mut header = String::new();
        let mut end = first.end;
        let mut index = start;

        while let Some(line) = self.get(index) {
            if flow_line_starts_body(line, index == start) {
                break;
            }
            if !header.is_empty() {
                header.push('\n');
            }
            header.push_str(&line.text);
            end = line.end;
            index += 1;
        }

        if index >= self.len() {
            return CstBlockEvent::new(header, String::new(), end, false, index);
        }

        let mut body = self.collect_brace_block(index, CstBlockOpenRule::FlowBody);
        if !body.head.is_empty() {
            if !header.is_empty() {
                header.push('\n');
            }
            header.push_str(&body.head);
        }
        body.head = header;
        body
    }
}

impl Index<usize> for CstLineEvents {
    type Output = CstLine;

    fn index(&self, index: usize) -> &Self::Output {
        &self.lines[index]
    }
}

impl CstLine {
    fn from_node(node: &SyntaxNode) -> Self {
        let start = usize::from(node.text_range().start());
        let mut end = usize::from(node.text_range().end());
        let mut text = node.text().to_string();
        if text.ends_with("\r\n") {
            text.truncate(text.len() - 2);
            end -= 2;
        } else if text.ends_with('\n') || text.ends_with('\r') {
            text.truncate(text.len() - 1);
            end -= 1;
        }
        let kind = classify_line(&text);
        let punctuation = CstLinePunctuationSummary::from_node(node);
        Self {
            text,
            start,
            end,
            punctuation,
            kind,
        }
    }

    /// Line text without a trailing newline.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Coarse line-event kind.
    pub const fn kind(&self) -> CstLineKind {
        self.kind
    }

    /// Trimmed line text.
    pub fn trimmed(&self) -> &str {
        self.text.trim()
    }

    /// Line text with leading whitespace removed.
    pub fn trim_start(&self) -> &str {
        self.text.trim_start()
    }

    /// Returns true when the line should be skipped as trivia by grammar parsing.
    pub const fn is_trivia(&self) -> bool {
        matches!(self.kind, CstLineKind::Blank | CstLineKind::Comment)
    }

    /// Extracts a documentation-comment payload from a doc-comment line.
    pub fn doc_comment_text(&self) -> Option<&str> {
        let text = self.trim_start().strip_prefix("///")?;
        Some(text.strip_prefix(' ').unwrap_or(text))
    }

    /// Classifies a top-level line before declaration-specific parsing.
    pub(crate) fn top_level_line_kind(&self) -> CstTopLevelLineKind {
        classify_top_level_line(self.trimmed())
    }

    /// Classifies a top-level declaration line before AST construction.
    pub(crate) fn top_level_item_kind(&self) -> CstTopLevelItemKind {
        classify_top_level_item(self.trimmed())
    }

    /// Classifies a flow-body line before AST construction.
    pub(crate) fn flow_item_kind(&self) -> CstFlowItemKind {
        classify_flow_item(self.trimmed())
    }

    /// Start byte offset in the original source.
    pub const fn start(&self) -> usize {
        self.start
    }

    /// End byte offset before the line terminator.
    pub const fn end(&self) -> usize {
        self.end
    }

    pub(crate) const fn brace_delta(&self) -> i32 {
        self.punctuation.brace_delta
    }

    pub(crate) const fn has_top_level_brace_open(&self) -> bool {
        self.punctuation.has_top_level_brace_open
    }

    pub(crate) const fn has_unclosed_top_level_brace_open(&self) -> bool {
        self.punctuation.has_top_level_brace_open && self.punctuation.brace_delta > 0
    }
}

impl CstLinePunctuationSummary {
    fn from_node(node: &SyntaxNode) -> Self {
        let mut summary = Self::default();
        let mut paren = 0usize;
        let mut square = 0usize;
        let mut brace = 0usize;
        let mut angle = 0usize;

        for token in node
            .children_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
        {
            if token.kind() != SyntaxKind::Punctuation {
                continue;
            }
            if token.text() == "{" && paren == 0 && square == 0 && brace == 0 && angle == 0 {
                summary.has_top_level_brace_open = true;
            }
            match token.text() {
                "{" => {
                    summary.brace_delta += 1;
                    brace += 1;
                }
                "}" => {
                    summary.brace_delta -= 1;
                    brace = brace.saturating_sub(1);
                }
                "(" => {
                    summary.paren_delta += 1;
                    paren += 1;
                }
                ")" => {
                    summary.paren_delta -= 1;
                    paren = paren.saturating_sub(1);
                }
                "[" => {
                    summary.bracket_delta += 1;
                    square += 1;
                }
                "]" => {
                    summary.bracket_delta -= 1;
                    square = square.saturating_sub(1);
                }
                "<" => angle += 1,
                ">" => angle = angle.saturating_sub(1),
                _ => {}
            }
        }
        summary
    }
}

pub(crate) fn classify_stmt(trimmed: &str) -> CstStmtKind {
    if looks_like_lifetime_set(trimmed) {
        CstStmtKind::LifetimeSet
    } else if trimmed.starts_with("wait(") {
        CstStmtKind::Wait
    } else if trimmed.starts_with("let ") {
        CstStmtKind::Let
    } else if trimmed.starts_with("defer ") && trimmed.contains('{') {
        CstStmtKind::DeferBlock
    } else if trimmed.starts_with("defer ") {
        CstStmtKind::Defer
    } else if looks_like_control_transfer(trimmed) {
        CstStmtKind::ControlTransfer
    } else if trimmed.starts_with("on ") {
        CstStmtKind::On
    } else if trimmed.starts_with("unsafe lifetime ") && trimmed.contains('{') {
        CstStmtKind::UnsafeLifetime
    } else if looks_like_braced_stmt(trimmed) {
        CstStmtKind::Braced
    } else if matches!(trimmed.split_whitespace().next(), Some("match" | "if")) {
        CstStmtKind::AmbiguousBlockHead
    } else {
        CstStmtKind::Expr
    }
}

fn looks_like_lifetime_set(trimmed: &str) -> bool {
    let Some((target, _)) = split_top_level_punctuation_sequence_once(trimmed, &["<", "-"]) else {
        return false;
    };
    target.trim_start().starts_with('\'')
}

fn looks_like_control_transfer(trimmed: &str) -> bool {
    trimmed == "break"
        || trimmed == "continue"
        || trimmed.starts_with("continue ")
        || trimmed.starts_with("out ")
        || trimmed.starts_with("break ")
        || ["return ", "goto ", "yield ", "close ", "select "]
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
}

fn looks_like_braced_stmt(trimmed: &str) -> bool {
    find_top_level_punctuation(trimmed, '{').is_some()
}

fn classify_line(text: &str) -> CstLineKind {
    let trimmed = text.trim_start();
    if trimmed.trim_end().is_empty() {
        CstLineKind::Blank
    } else if trimmed.starts_with("///") {
        CstLineKind::DocComment
    } else if trimmed.starts_with("//") {
        CstLineKind::Comment
    } else {
        CstLineKind::Code
    }
}

fn classify_top_level_line(trimmed: &str) -> CstTopLevelLineKind {
    if trimmed.starts_with("#[") {
        CstTopLevelLineKind::Attribute
    } else if trimmed.starts_with("mod ") {
        CstTopLevelLineKind::Module
    } else if looks_like_use_line(trimmed) {
        CstTopLevelLineKind::Use
    } else {
        CstTopLevelLineKind::Item
    }
}

fn classify_top_level_item(trimmed: &str) -> CstTopLevelItemKind {
    if looks_like_flow(trimmed) {
        CstTopLevelItemKind::Flow
    } else if looks_like_function_item(trimmed) {
        CstTopLevelItemKind::Function
    } else if looks_like_callable_item(trimmed) {
        CstTopLevelItemKind::Callable
    } else if looks_like_state_item(trimmed) {
        CstTopLevelItemKind::State
    } else if looks_like_trait_item(trimmed) {
        CstTopLevelItemKind::Trait
    } else if looks_like_impl_item(trimmed) {
        CstTopLevelItemKind::Impl
    } else if looks_like_enum_item(trimmed) {
        CstTopLevelItemKind::Enum
    } else if looks_like_struct_item(trimmed) {
        CstTopLevelItemKind::Struct
    } else if looks_like_type_alias(trimmed) {
        CstTopLevelItemKind::TypeAlias
    } else if looks_like_entity_decl_item(trimmed) {
        CstTopLevelItemKind::EntityDecl
    } else if looks_like_entry_item(trimmed) {
        CstTopLevelItemKind::Entry
    } else if looks_like_extern_capability_item(trimmed) {
        CstTopLevelItemKind::ExternCapability
    } else if looks_like_extern_mod_item(trimmed) {
        CstTopLevelItemKind::ExternMod
    } else if looks_like_hook(trimmed) {
        CstTopLevelItemKind::Hook
    } else if looks_like_dialogue_defaults(trimmed) {
        CstTopLevelItemKind::DialogueDefaults
    } else if looks_like_memo_fn(trimmed) {
        CstTopLevelItemKind::MemoFn
    } else if looks_like_proof_item(trimmed) {
        CstTopLevelItemKind::Proof
    } else if looks_like_trusted_axiom_item(trimmed) {
        CstTopLevelItemKind::TrustedAxiom
    } else if looks_like_test_item(trimmed) {
        CstTopLevelItemKind::Test
    } else if looks_like_bench_item(trimmed) {
        CstTopLevelItemKind::Bench
    } else if looks_like_parser_item(trimmed) {
        CstTopLevelItemKind::Parser
    } else if looks_like_source_item(trimmed) {
        CstTopLevelItemKind::Source
    } else {
        CstTopLevelItemKind::FlowBodyItemOrRaw
    }
}

fn visible_tail(input: &str) -> &str {
    let trimmed = input.trim_start();
    if let Some(rest) = trimmed.strip_prefix("pub(crate)") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("pub(super)") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("pub ") {
        rest
    } else {
        input
    }
}

fn visible_head(input: &str) -> &str {
    visible_tail(input).trim_start()
}

fn looks_like_use_line(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    let rest = rest.strip_prefix("surface ").unwrap_or(rest);
    rest.starts_with("use ") || rest.starts_with("lazy use ") || rest.starts_with("eager use ")
}

fn looks_like_flow(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    rest.starts_with("flow ") || rest.starts_with("fragment ")
}

fn looks_like_function_item(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    rest.starts_with("fn ")
        || rest.starts_with("task fn ")
        || rest.starts_with("dialogue fn ")
        || rest.starts_with("stream fn ")
}

fn looks_like_callable_item(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    rest.starts_with("reducer ") || rest.starts_with("view ")
}

fn looks_like_state_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("state ")
}

fn looks_like_trait_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("trait ")
}

fn looks_like_impl_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("impl")
}

fn looks_like_enum_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("enum ")
}

fn looks_like_struct_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("struct ")
}

fn looks_like_type_alias(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("type ")
}

fn looks_like_entity_decl_item(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    let rest = rest.strip_prefix("surface ").unwrap_or(rest);
    [
        "audio bus",
        "mixer snapshot",
        "voice profile",
        "character",
        "component",
        "activity",
        "metric counter",
        "metric gauge",
        "metric",
        "signal",
        "layer",
        "textbox",
        "voice",
        "se",
        "bgm",
        "ducking",
        "motion",
        "rig",
    ]
    .into_iter()
    .any(|keyword| {
        rest.strip_prefix(keyword)
            .is_some_and(|tail| tail.starts_with(char::is_whitespace))
    })
}

fn looks_like_extern_mod_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("extern ")
}

fn looks_like_entry_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("entry ")
}

fn looks_like_extern_capability_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("extern capability ")
}

fn looks_like_hook(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("hook ")
}

fn looks_like_dialogue_defaults(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("dialogue defaults")
}

fn looks_like_memo_fn(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("memo fn ")
}

fn looks_like_proof_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("proof ")
}

fn looks_like_trusted_axiom_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("trusted axiom ")
}

fn looks_like_test_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("test ")
}

fn looks_like_bench_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("bench ")
}

fn looks_like_parser_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("parser ")
}

fn looks_like_source_item(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    rest.starts_with("source ") && !rest.starts_with("source locale ")
}

fn classify_flow_item(trimmed: &str) -> CstFlowItemKind {
    if let Some(kind) = classify_structured_flow_block(trimmed) {
        CstFlowItemKind::StructuredBlock(kind)
    } else if trimmed.starts_with("include ") {
        CstFlowItemKind::Include
    } else if is_await_with_head(trimmed) {
        CstFlowItemKind::AwaitWith
    } else if let Some(kind) = classify_let_flow_item(trimmed) {
        CstFlowItemKind::Let(kind)
    } else if is_typed_stmt(trimmed) {
        CstFlowItemKind::TypedStmt
    } else {
        CstFlowItemKind::Other
    }
}

fn classify_structured_flow_block(trimmed: &str) -> Option<CstStructuredFlowBlockKind> {
    if trimmed.starts_with("choice ") {
        Some(CstStructuredFlowBlockKind::Choice)
    } else if trimmed.starts_with("if let ") {
        Some(CstStructuredFlowBlockKind::IfLet)
    } else if trimmed.starts_with("if ") {
        Some(CstStructuredFlowBlockKind::If)
    } else if trimmed.starts_with("match ") {
        Some(CstStructuredFlowBlockKind::Match)
    } else if is_loop_head(trimmed) {
        Some(CstStructuredFlowBlockKind::Loop)
    } else if trimmed.starts_with("while let ") {
        Some(CstStructuredFlowBlockKind::WhileLet)
    } else if trimmed.starts_with("while ") {
        Some(CstStructuredFlowBlockKind::While)
    } else if trimmed.starts_with("for ") {
        Some(CstStructuredFlowBlockKind::For)
    } else if trimmed.starts_with("select") {
        Some(CstStructuredFlowBlockKind::Select)
    } else if trimmed.starts_with("thread ") || matches!(trimmed, "thread" | "thread:") {
        Some(CstStructuredFlowBlockKind::Thread)
    } else if trimmed.starts_with("defer ") || matches!(trimmed, "defer" | "defer:") {
        Some(CstStructuredFlowBlockKind::Defer)
    } else if trimmed.starts_with("borrow ") {
        Some(CstStructuredFlowBlockKind::Borrow)
    } else if trimmed.starts_with("unsafe lifetime ") {
        Some(CstStructuredFlowBlockKind::UnsafeLifetime)
    } else if trimmed.starts_with("source locale ") {
        Some(CstStructuredFlowBlockKind::SourceLocale)
    } else if trimmed.starts_with('{') {
        Some(CstStructuredFlowBlockKind::BareScope)
    } else if trimmed.starts_with("scope ") || matches!(trimmed, "scope" | "scope:") {
        Some(CstStructuredFlowBlockKind::Scope)
    } else {
        None
    }
}

fn classify_let_flow_item(trimmed: &str) -> Option<CstLetFlowItemKind> {
    let value = let_binding_value(trimmed)?;
    Some(if value.trim_start().starts_with("choice ") {
        CstLetFlowItemKind::Choice
    } else if is_dialogue_call_value(value) {
        CstLetFlowItemKind::DialogueCall
    } else if parse_scope_head(value.trim_start()) {
        CstLetFlowItemKind::Scope
    } else if matches!(value.trim(), "result {" | "task {" | "seq {" | "stream {") {
        CstLetFlowItemKind::ComputationBlock
    } else if value.trim_start().starts_with("memo(") {
        CstLetFlowItemKind::MemoBlock
    } else if value.trim().starts_with('{') {
        CstLetFlowItemKind::Block
    } else if is_loop_head(value.trim_start()) {
        CstLetFlowItemKind::Loop
    } else if is_await_with_head(value.trim())
        || value.trim().starts_with("(await ") && value.contains(" with")
    {
        CstLetFlowItemKind::AwaitWith
    } else if is_await_start_head(value.trim()) {
        CstLetFlowItemKind::AwaitStart
    } else if value.trim_start().starts_with("if let ") {
        CstLetFlowItemKind::IfLet
    } else if value.trim_start().starts_with("if ") {
        CstLetFlowItemKind::If
    } else if value.trim_start().starts_with("match ") {
        CstLetFlowItemKind::Match
    } else if trimmed.starts_with("let ") && trimmed.contains(" else") && trimmed.contains('{') {
        CstLetFlowItemKind::LetElse
    } else {
        CstLetFlowItemKind::Plain
    })
}

fn let_binding_value(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("let ")?;
    split_top_level_punctuation_once(rest, '=').map(|(_, value)| value)
}

fn is_dialogue_call_value(value: &str) -> bool {
    let value = value.trim_start();
    let Some(open) = find_content_bracket(value) else {
        return false;
    };
    if value.starts_with('[') {
        return false;
    }
    let target = value[..open].trim();
    let Some(close) = find_matching_punctuation(value, open, '[', ']') else {
        // Multiline dialogue result bindings start as `let x = speaker.say()[`
        // on the first CST line. Classify those as dialogue so the AST parser
        // can collect the remaining content lines and the following `with:`.
        return target.contains('(');
    };
    let content = value[open + '['.len_utf8()..close].trim();
    target.contains('(') || crate::expr::parse_expr(content).is_err()
}

fn parse_scope_head(source: &str) -> bool {
    let Some(rest) = source.strip_prefix("scope") else {
        return false;
    };
    if rest
        .chars()
        .next()
        .is_some_and(|ch| !(ch.is_whitespace() || ch == '{'))
    {
        return false;
    }
    true
}

fn is_loop_head(head: &str) -> bool {
    head == "loop"
        || head.starts_with("loop ")
        || labeled_head_tail(head).is_some_and(|tail| tail == "loop" || tail.starts_with("loop "))
}

fn labeled_head_tail(head: &str) -> Option<&str> {
    let rest = head.trim_start().strip_prefix('\'')?;
    let (_, tail) = split_top_level_punctuation_once(rest, ':')?;
    Some(tail.trim_start())
}

fn is_await_with_head(trimmed: &str) -> bool {
    (trimmed.starts_with("await ")
        || trimmed.starts_with("try await ")
        || trimmed.starts_with("await? "))
        && (trimmed.contains(" with ") || trimmed.ends_with("with:"))
}

fn is_await_start_head(trimmed: &str) -> bool {
    trimmed.starts_with("await ")
        || trimmed.starts_with("try await ")
        || trimmed.starts_with("await? ")
        || trimmed.starts_with("(await ")
}

fn find_content_bracket(text: &str) -> Option<usize> {
    let open = find_top_level_punctuation(text, '[')?;
    (!text[..open].trim_end().ends_with('#')).then_some(open)
}

fn is_typed_stmt(trimmed: &str) -> bool {
    if trimmed.starts_with('\'') && (trimmed.contains("<-") || trimmed.contains("|>")) {
        return true;
    }
    matches!(
        trimmed.split_whitespace().next(),
        Some(
            "let"
                | "match"
                | "if"
                | "for"
                | "return"
                | "out"
                | "goto"
                | "thread"
                | "defer"
                | "yield"
                | "unsafe"
                | "signal"
                | "close"
                | "break"
                | "continue"
        )
    )
}

impl CstBlockEvent {
    fn new(head: String, body: String, end: usize, ok: bool, next_index: usize) -> Self {
        Self {
            head,
            body,
            end,
            ok,
            next_index,
        }
    }
}

fn block_event_is_complete(
    rule: CstBlockOpenRule,
    seen_open: bool,
    seen_body_open: bool,
    depth: i32,
) -> bool {
    match rule {
        CstBlockOpenRule::FirstTopLevel | CstBlockOpenRule::FlowBody => seen_open && depth == 0,
        CstBlockOpenRule::FunctionBody => seen_open && seen_body_open && depth == 0,
    }
}

fn flow_line_starts_body(line: &CstLine, is_first_line: bool) -> bool {
    let trimmed = line.trimmed();
    trimmed == "{"
        || (is_first_line && line.has_top_level_brace_open() && !trimmed.starts_with("effects"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CstToken<'a> {
    kind: SyntaxKind,
    text: &'a str,
    start: usize,
    end: usize,
}

pub(crate) fn lex_cst(source: &str) -> Vec<CstToken<'_>> {
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while cursor < source.len() {
        let rest = &source[cursor..];
        let (kind, len) = next_token(rest);
        tokens.push(CstToken {
            kind,
            text: &source[cursor..cursor + len],
            start: cursor,
            end: cursor + len,
        });
        cursor += len;
    }

    tokens
}

impl<'a> CstToken<'a> {
    pub(crate) const fn kind(&self) -> SyntaxKind {
        self.kind
    }

    pub(crate) const fn text(&self) -> &'a str {
        self.text
    }

    pub(crate) const fn start(&self) -> usize {
        self.start
    }

    pub(crate) const fn end(&self) -> usize {
        self.end
    }

    pub(crate) fn text_starts_with(&self, value: char) -> bool {
        token_text_is(self.text, value)
    }
}

fn token_text_is(text: &str, value: char) -> bool {
    let mut chars = text.chars();
    chars.next() == Some(value) && chars.next().is_none()
}

/// Finds the close punctuation matching an opening punctuation token.
///
/// The scan is token-based, so quoted strings and comments are never inspected
/// as nested syntax. This is the interim CST event utility used while the
/// grammar parser is being migrated away from local string splitters.
pub(crate) fn find_matching_punctuation(
    source: &str,
    open_offset: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }
        if token.start() < open_offset {
            continue;
        }
        if token.text_starts_with(open) {
            depth += 1;
        } else if token.text_starts_with(close) {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(token.start());
            }
        }
    }
    None
}

/// Computes open-minus-close punctuation depth from CST tokens.
///
/// This is intentionally token-based instead of `chars()` based: brackets in
/// dialogue text strings, comments, and doc comments are source text, not block
/// structure. Callers use this for multiline recovery while the full parser is
/// still consuming line events.
pub(crate) fn punctuation_delta(source: &str, open: char, close: char) -> i32 {
    lex_cst(source)
        .into_iter()
        .filter(|token| token.kind() == SyntaxKind::Punctuation)
        .fold(0, |depth, token| match token.text() {
            text if token_text_is(text, open) => depth + 1,
            text if token_text_is(text, close) => depth - 1,
            _ => depth,
        })
}

/// Computes all bracket-family depth deltas in one token scan.
pub(crate) fn punctuation_deltas(source: &str) -> CstPunctuationDeltas {
    lex_cst(source)
        .into_iter()
        .filter(|token| token.kind() == SyntaxKind::Punctuation)
        .fold(CstPunctuationDeltas::default(), |mut deltas, token| {
            match token.text() {
                "{" => deltas.brace += 1,
                "}" => deltas.brace -= 1,
                "(" => deltas.paren += 1,
                ")" => deltas.paren -= 1,
                "[" => deltas.bracket += 1,
                "]" => deltas.bracket -= 1,
                _ => {}
            }
            deltas
        })
}

/// Iterates source lines without treating line splitting as parser grammar.
///
/// This helper is intentionally small and text-level. Parser modules use it
/// while they are still being migrated to rowan events so line handling is
/// centralized in the CST/text utility layer instead of open-coded at each
/// grammar site.
pub(crate) fn source_line_iter(source: &str) -> impl Iterator<Item = &str> {
    source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

/// Returns non-empty trimmed source lines for interim line-oriented parsing.
pub(crate) fn nonempty_trimmed_source_lines(source: &str) -> Vec<&str> {
    source_line_iter(source)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

/// Counts source lines using the same text policy as [`source_line_iter`].
pub(crate) fn source_line_count(source: &str) -> usize {
    source_line_iter(source).count()
}

/// Documentation prefix extracted from a text fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CstDocPrefix {
    lines: Vec<String>,
    consumed: usize,
}

impl CstDocPrefix {
    pub(crate) fn lines(&self) -> &[String] {
        &self.lines
    }

    pub(crate) const fn consumed(&self) -> usize {
        self.consumed
    }
}

/// Takes leading `///` lines from a parameter fragment.
///
/// Function parameters are parsed from signature fragments rather than full
/// rowan line nodes. Keeping the scan here preserves one source of truth for
/// doc-comment stripping until signatures are fully event-backed.
pub(crate) fn take_doc_comment_prefix(source: &str) -> Option<CstDocPrefix> {
    let mut lines = Vec::new();
    let mut consumed = 0;

    for segment in source.split_inclusive('\n') {
        let line = segment.trim_end_matches('\n').trim_end_matches('\r');
        let trimmed = line.trim();
        let Some(text) = trimmed.strip_prefix("///") else {
            break;
        };
        lines.push(text.strip_prefix(' ').unwrap_or(text).to_owned());
        consumed += segment.len();
    }

    (!lines.is_empty()).then_some(CstDocPrefix { lines, consumed })
}

/// Finds a top-level punctuation token while ignoring strings and comments.
pub(crate) fn find_top_level_punctuation(source: &str, punctuation: char) -> Option<usize> {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }

        if token.text_starts_with(punctuation)
            && paren == 0
            && square == 0
            && brace == 0
            && angle == 0
        {
            return Some(token.start());
        }

        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            _ => {}
        }
    }
    None
}

/// Finds the last punctuation token with the requested text.
pub(crate) fn find_last_punctuation(source: &str, punctuation: char) -> Option<usize> {
    lex_cst(source)
        .into_iter()
        .filter(|token| {
            token.kind() == SyntaxKind::Punctuation && token.text_starts_with(punctuation)
        })
        .map(|token| token.start())
        .next_back()
}

/// Finds the last top-level punctuation token while ignoring strings and comments.
pub(crate) fn find_last_top_level_punctuation(source: &str, punctuation: char) -> Option<usize> {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;
    let mut found = None;

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }

        if token.text_starts_with(punctuation)
            && paren == 0
            && square == 0
            && brace == 0
            && angle == 0
        {
            found = Some(token.start());
        }

        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            _ => {}
        }
    }
    found
}

/// Finds the last opening punctuation that starts while the matching delimiter depth is zero.
pub(crate) fn find_last_depth_zero_open_punctuation(
    source: &str,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut found = None;

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }

        match token.text() {
            text if token_text_is(text, open) => {
                if depth == 0 {
                    found = Some(token.start());
                }
                depth += 1;
            }
            text if token_text_is(text, close) => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    found
}

fn find_body_open(source: &str) -> Option<usize> {
    find_last_depth_zero_open_punctuation(source, '{', '}')
}

/// Splits a leading identifier token from the rest of a source fragment.
pub(crate) fn split_leading_ident(source: &str) -> Option<(&str, &str)> {
    let token = lex_cst(source).into_iter().next()?;
    (token.kind() == SyntaxKind::Ident && token.start() == 0)
        .then(|| (token.text(), source[token.end()..].trim_start()))
}

/// Parses a flat fence line while preserving the byte offset of the fence head.
pub fn parse_flat_fence(source: &str) -> Option<FlatFence<'_>> {
    let trimmed_offset = leading_byte_len(source);
    let trimmed = source.trim();
    let inner_source = trimmed.strip_prefix("===")?.strip_suffix("===")?;
    let inner_leading = leading_byte_len(inner_source);
    let inner = inner_source.trim();
    let inner_start = trimmed_offset + "===".len() + inner_leading;
    if inner.is_empty() {
        return Some(FlatFence {
            kind: "",
            head: "",
            close: false,
            head_start: inner_start,
        });
    }
    if let Some(close) = inner.strip_prefix('/') {
        let close_leading = leading_byte_len(close);
        let close = close.trim_start();
        let kind = close.split_whitespace().next().unwrap_or_default();
        return Some(FlatFence {
            kind,
            head: close.trim(),
            close: true,
            head_start: inner_start + '/'.len_utf8() + close_leading,
        });
    }
    let (kind, head) = split_leading_ident(inner).unwrap_or((inner, ""));
    let head_leading = leading_byte_len(head);
    Some(FlatFence {
        kind,
        head: head.trim(),
        close: false,
        head_start: inner_start + (inner.len() - head.len()) + head_leading,
    })
}

fn leading_byte_len(source: &str) -> usize {
    source.len() - source.trim_start().len()
}

/// Splits a leading lifetime name, including the leading apostrophe.
pub(crate) fn split_leading_lifetime(source: &str) -> Option<(&str, &str)> {
    let rest = source.strip_prefix('\'')?;
    let len = take_while(rest, is_ident_continue);
    (len > 0).then(|| (&source[..'\''.len_utf8() + len], rest[len..].trim_start()))
}

/// Splits a leading entity reference and exposes its marker-normalized parts.
pub(crate) fn split_leading_entity_ref_parts(source: &str) -> Option<CstEntityRef<'_>> {
    let token = lex_cst(source).into_iter().next()?;
    if token.kind() != SyntaxKind::EntityRef || token.start() != 0 {
        return None;
    }
    let raw = token.text();
    if starts_family_relative_entity_ref(raw) || raw.starts_with("@.") || raw.starts_with("@super.")
    {
        return None;
    }
    let rest = &source[token.end()..];
    let delimited = raw.starts_with("@<");
    let body = if delimited {
        raw.strip_prefix("@<")
            .map_or("", |inner| inner.strip_suffix('>').unwrap_or(inner))
    } else {
        &raw[1..]
    };
    Some(CstEntityRef {
        raw,
        body,
        delimited,
        closed: !delimited || raw.ends_with('>'),
        rest,
    })
}

/// Returns true when a fragment begins with an entity reference token.
pub(crate) fn starts_leading_entity_ref(source: &str) -> bool {
    split_leading_entity_ref_parts(source).is_some()
}

/// Splits a leading family-qualified relative entity reference.
pub(crate) fn split_leading_relative_entity_ref(source: &str) -> Option<CstRelativeEntityRef<'_>> {
    let at = source.strip_prefix('@')?;
    let family_len = take_while(at, |ch| ch.is_ascii_alphanumeric() || ch == '_');
    if family_len == 0 || !at.get(family_len..)?.starts_with(":.") {
        return None;
    }
    let family = &at[..family_len];
    let relative_source = &at[family_len + ':'.len_utf8()..];
    let dots = take_while(relative_source, |ch| ch == '.');
    if dots == 0 {
        return None;
    }
    let body_source = &relative_source[dots..];
    let body_len = take_relative_id_body(body_source);
    if body_len == 0 {
        return None;
    }
    let relative = CstRelativeId {
        body: &body_source[..body_len],
        parent_depth: dots.saturating_sub(1),
        spelling: CstRelativeIdSpelling::DotRun,
        marker_len: dots,
        rest: &body_source[body_len..],
    };
    let raw_len =
        '@'.len_utf8() + family_len + ':'.len_utf8() + relative.marker_len + relative.body.len();
    Some(CstRelativeEntityRef {
        raw: &source[..raw_len],
        family,
        relative,
        rest: &source[raw_len..],
    })
}

fn starts_family_relative_entity_ref(raw: &str) -> bool {
    let Some(at) = raw.strip_prefix('@') else {
        return false;
    };
    let family_len = take_while(at, |ch| ch.is_ascii_alphanumeric() || ch == '_');
    family_len > 0
        && at
            .get(family_len..)
            .is_some_and(|tail| tail.starts_with(":."))
}

/// Returns true when a fragment begins with a family-qualified relative entity reference.
pub(crate) fn starts_leading_relative_entity_ref(source: &str) -> bool {
    split_leading_relative_entity_ref(source).is_some()
}

/// Returns true when a fragment begins with an ID-context relative ID marker.
pub(crate) fn starts_leading_relative_id(source: &str) -> bool {
    source.starts_with("@.") || source.starts_with("@super.")
}

/// Splits a leading relative ID marker in an ID-bearing context.
///
/// The current grammar uses `@.id`, parent-dot forms such as `@..id`, and
/// explicit `@super.id` forms.
pub(crate) fn split_leading_relative_id(source: &str) -> Option<CstRelativeId<'_>> {
    if let Some(relative) = split_dot_relative_id(source) {
        return Some(relative);
    }
    split_super_relative_id(source)
}

fn split_dot_relative_id(source: &str) -> Option<CstRelativeId<'_>> {
    let rest = source.strip_prefix('@')?;
    let dots = take_while(rest, |ch| ch == '.');
    let (dot_run, marker_len) = (dots, 1 + dots);
    if dot_run == 0 {
        return None;
    }
    let body_source = &source[marker_len..];
    let body_len = take_relative_id_body(body_source);
    (body_len > 0).then(|| CstRelativeId {
        body: &body_source[..body_len],
        parent_depth: dot_run.saturating_sub(1),
        spelling: CstRelativeIdSpelling::DotRun,
        marker_len,
        rest: &body_source[body_len..],
    })
}

fn split_super_relative_id(source: &str) -> Option<CstRelativeId<'_>> {
    if !source.starts_with('@') {
        return None;
    }
    let mut cursor = "@".len();
    let mut parents = 0usize;
    while cursor <= source.len() && source[cursor..].starts_with("super.") {
        parents += 1;
        cursor += "super.".len();
    }
    if parents == 0 {
        return None;
    }
    let body_len = take_relative_id_body(&source[cursor..]);
    (body_len > 0).then(|| CstRelativeId {
        body: &source[cursor..cursor + body_len],
        parent_depth: parents,
        spelling: CstRelativeIdSpelling::SuperChain,
        marker_len: cursor,
        rest: &source[cursor + body_len..],
    })
}

fn take_relative_id_body(source: &str) -> usize {
    let Some(first) = source.chars().next() else {
        return 0;
    };
    if !is_ident_start(first) {
        return 0;
    }
    take_while(source, |ch| {
        ch.is_alphanumeric() || matches!(ch, '.' | '_' | '-')
    })
}

/// Finds the closing `>` for an angle group that starts at `open_offset`.
pub(crate) fn find_matching_angle_group(source: &str, open_offset: usize) -> Option<usize> {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;
    let mut previous_text = "";

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            previous_text = token.text();
            continue;
        }
        if token.start() < open_offset {
            previous_text = token.text();
            continue;
        }

        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" if previous_text != "-" => {
                angle = angle.checked_sub(1)?;
                if paren == 0 && square == 0 && brace == 0 && angle == 0 {
                    return Some(token.start());
                }
            }
            _ => {}
        }
        previous_text = token.text();
    }
    None
}

/// Splits once at a top-level punctuation token.
pub(crate) fn split_top_level_punctuation_once(
    source: &str,
    delimiter: char,
) -> Option<(&str, &str)> {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }

        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            text if token_text_is(text, delimiter)
                && paren == 0
                && square == 0
                && brace == 0
                && angle == 0 =>
            {
                return Some((source[..token.start()].trim(), source[token.end()..].trim()));
            }
            _ => {}
        }
    }
    None
}

/// Splits at every top-level punctuation token.
pub(crate) fn split_top_level_punctuation(source: &str, delimiter: char) -> Vec<&str> {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;
    let mut parts = Vec::new();
    let mut start = 0usize;

    for token in lex_cst(source) {
        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }

        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            text if token_text_is(text, delimiter)
                && paren == 0
                && square == 0
                && brace == 0
                && angle == 0 =>
            {
                parts.push(source[start..token.start()].trim());
                start = token.end();
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

/// Returns the first complete string-literal body and the tail after the token.
pub(crate) fn split_first_string_literal(source: &str) -> Option<(&str, &str)> {
    lex_cst(source)
        .into_iter()
        .find(|token| token.kind() == SyntaxKind::String)
        .and_then(|token| {
            let text = token.text();
            (text.len() >= 2 && text.starts_with('"') && text.ends_with('"'))
                .then(|| (&text[1..text.len() - 1], &source[token.end()..]))
        })
}

/// Returns all `[[wiki link]]` marker ranges found in source text.
pub(crate) fn collect_wiki_link_ranges(source: &str) -> Vec<(&str, usize, usize)> {
    let mut links = Vec::new();
    let mut cursor = 0;
    while let Some(start_relative) = source[cursor..].find("[[") {
        let start = cursor + start_relative;
        let body_start = start + 2;
        let Some(end_relative) = source[body_start..].find("]]") else {
            break;
        };
        let end = body_start + end_relative;
        links.push((&source[body_start..end], start, end + 2));
        cursor = end + 2;
    }
    links
}

/// Splits once at a top-level contiguous punctuation token sequence.
///
/// Operators such as `=>`, `->`, and `<-` are lexed as individual punctuation
/// tokens. Keeping this sequence splitter in the CST layer prevents each parser
/// family from inventing its own string search for multi-token separators.
pub(crate) fn split_top_level_punctuation_sequence_once<'a>(
    source: &'a str,
    sequence: &[&str],
) -> Option<(&'a str, &'a str)> {
    let tokens = lex_cst(source);
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;

    for (index, token) in tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::Punctuation
            && paren == 0
            && square == 0
            && brace == 0
            && angle == 0
            && punctuation_sequence_matches(&tokens, index, sequence)
        {
            let end = tokens[index + sequence.len() - 1].end();
            return Some((source[..token.start()].trim(), source[end..].trim()));
        }

        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }
        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            _ => {}
        }
    }
    None
}

/// Splits once at the last top-level contiguous punctuation token sequence.
pub(crate) fn split_last_top_level_punctuation_sequence_once<'a>(
    source: &'a str,
    sequence: &[&str],
) -> Option<(&'a str, &'a str)> {
    let tokens = lex_cst(source);
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;
    let mut found = None;

    for (index, token) in tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::Punctuation
            && paren == 0
            && square == 0
            && brace == 0
            && angle == 0
            && punctuation_sequence_matches(&tokens, index, sequence)
        {
            let end = tokens[index + sequence.len() - 1].end();
            found = Some((token.start(), end));
        }

        if token.kind() != SyntaxKind::Punctuation {
            continue;
        }
        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            _ => {}
        }
    }

    found.map(|(start, end)| (source[..start].trim(), source[end..].trim()))
}

fn punctuation_sequence_matches(tokens: &[CstToken<'_>], index: usize, sequence: &[&str]) -> bool {
    if sequence.is_empty() || index + sequence.len() > tokens.len() {
        return false;
    }

    sequence.iter().enumerate().all(|(offset, expected)| {
        let token = &tokens[index + offset];
        token.kind() == SyntaxKind::Punctuation
            && token.text() == *expected
            && (offset == 0 || tokens[index + offset - 1].end() == token.start())
    })
}

/// Splits once before a top-level identifier keyword.
pub(crate) fn split_top_level_keyword_once<'a>(
    source: &'a str,
    keyword: &str,
) -> (&'a str, Option<&'a str>) {
    let mut paren = 0usize;
    let mut square = 0usize;
    let mut brace = 0usize;
    let mut angle = 0usize;

    for token in lex_cst(source) {
        match token.text() {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => square += 1,
            "]" => square = square.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            _ => {}
        }

        if token.kind() == SyntaxKind::Ident
            && token.text() == keyword
            && paren == 0
            && square == 0
            && brace == 0
            && angle == 0
        {
            return (&source[..token.start()], Some(source[token.end()..].trim()));
        }
    }
    (source, None)
}

fn next_token(source: &str) -> (SyntaxKind, usize) {
    if source.starts_with("\r\n") {
        return (SyntaxKind::Newline, 2);
    }

    let mut chars = source.char_indices();
    let Some((_, first)) = chars.next() else {
        return (SyntaxKind::Text, 0);
    };

    if first == '\n' || first == '\r' {
        return (SyntaxKind::Newline, first.len_utf8());
    }

    if first.is_whitespace() {
        return (
            SyntaxKind::Whitespace,
            take_while(source, |ch| ch.is_whitespace() && ch != '\n' && ch != '\r'),
        );
    }

    if source.starts_with("///") {
        return (SyntaxKind::DocComment, take_until_newline(source));
    }

    if source.starts_with("//") {
        return (SyntaxKind::Comment, take_until_newline(source));
    }

    if first == '"' {
        return (SyntaxKind::String, take_string(source));
    }

    if first == '@' && at_starts_entity_ref(source) {
        return (SyntaxKind::EntityRef, take_entity_ref(source));
    }

    if is_ident_start(first) {
        return (SyntaxKind::Ident, take_while(source, is_ident_continue));
    }

    if first.is_ascii_digit() {
        return (
            SyntaxKind::Number,
            take_while(source, |ch| ch.is_ascii_digit() || ch == '.'),
        );
    }

    if is_punctuation(first) {
        return (SyntaxKind::Punctuation, first.len_utf8());
    }

    (SyntaxKind::Text, first.len_utf8())
}

fn take_until_newline(source: &str) -> usize {
    source.find(['\r', '\n']).unwrap_or(source.len())
}

fn take_while(source: &str, predicate: impl Fn(char) -> bool) -> usize {
    source
        .char_indices()
        .take_while(|(_, ch)| predicate(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

fn take_string(source: &str) -> usize {
    let mut escaped = false;
    for (index, ch) in source.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return index + ch.len_utf8();
        }
        if ch == '\r' || ch == '\n' {
            return index;
        }
    }
    source.len()
}

fn take_entity_ref(source: &str) -> usize {
    if let Some(after_at) = source.strip_prefix("@<") {
        return after_at
            .find('>')
            .map_or(source.len(), |index| index + "@<>".len());
    }

    1 + source[1..]
        .char_indices()
        .take_while(|(_, ch)| is_entity_ref_char(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

fn at_starts_entity_ref(source: &str) -> bool {
    source.starts_with("@<")
        || source
            .chars()
            .nth(1)
            .is_some_and(|ch| is_ident_start(ch) && !source.starts_with("@super."))
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

fn is_entity_ref_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '@' | '/')
}

fn is_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '{' | '}'
            | '('
            | ')'
            | '['
            | ']'
            | ','
            | ':'
            | ';'
            | '.'
            | '='
            | '+'
            | '-'
            | '*'
            | '/'
            | '?'
            | '!'
            | '<'
            | '>'
            | '|'
            | '&'
            | '@'
    )
}
