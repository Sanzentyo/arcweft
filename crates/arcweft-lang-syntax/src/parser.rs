use crate::ast::common::{DocBlock, TextRange};
use crate::ast::dialogue::{ContentCall, DialogueContent, SpeakerLine};
use crate::ast::flow::{
    BorrowBlock, Flow, FlowInit, FlowItem, ForBlock, IfBlock, IfLetBlock, LoopBlock, MatchArm,
    MatchBlock, ScopeBlock, ScopeExprBlock, SelectBlock, SelectBranch, SelectBranchHead, Stmt,
    StmtMatchArm, WaitTarget, WhileBlock, WhileLetBlock,
};
use crate::ast::ids::{IdRef, RelativeId, RelativeIdSpelling};
use crate::ast::items::{RawSyntax, TypedSyntaxTree};
use crate::ast::line_plan::{BlockStyle, DeferOutcome, LinePlan};
use crate::cst::{
    CstBlockEvent, CstBlockOpenRule, CstFlowItemKind, CstLetFlowItemKind, CstLine, CstLineEvents,
    CstPunctuationDeltas, CstStmtKind, CstStructuredFlowBlockKind, CstTopLevelItemKind,
    CstTopLevelLineKind, SyntaxNode, SyntaxParseStats, classify_stmt, cst_lines_for_source,
    find_matching_punctuation, find_top_level_punctuation, parse_flat_fence, source_line_iter,
    split_leading_ident, split_top_level_keyword_once, split_top_level_punctuation_once,
    split_top_level_punctuation_sequence_once,
};
use crate::expr::Expr;
use crate::pattern::parse_pattern;
use crate::source::ParsedSource;
use crate::text::parse_dialogue_text;
use arcweft_source::{SourceAnchor, SourceName};
use std::borrow::Cow;
use std::ops::Range;

pub mod await_;
pub mod choice;
pub mod control_flow;
pub mod dialogue;
pub mod flow;
pub mod headers;
pub mod helpers;
pub mod hooks;
pub mod items;
pub mod line_plan;
pub mod proof;
pub mod recovery;
pub mod source;
pub mod statements;
pub mod top_level;
use await_::{is_await_with_head, parse_await_with};
use control_flow::{
    parse_block_expr, parse_braced_while_let_stmt, parse_named_block_expr, parse_scope_expr_body,
    parse_stmt_lines, parse_stmt_match_arms, split_pattern_guard,
};
use helpers::{
    PendingDocLines, attach_plan_to_dialogue_expr, collect_logical_block_items, collect_wiki_links,
    contains_dialogue_expr, find_content_bracket, flat_block_head, indentation,
    is_expression_statement_call, is_typed_stmt, is_with_brace_head, parse_binding_pattern,
    parse_computation_block_kind, parse_dialogue_call_expr_source, parse_expr_lossy,
    parse_expr_lossy_with_stats, parse_expr_with_inline_line_plan_with_stats,
    parse_inline_with_colon_plan, parse_line_options, parse_line_plan_attachment,
    parse_memo_block_options, parse_with_brace_label, parse_with_indent_label, source_take,
    split_brace_item, split_brace_item_with_scan, split_call_head, split_comma_args,
    split_optional_block_label, split_speaker_line, split_top_level_binding,
};
use line_plan::{
    parse_defer_outcome, parse_thread_block, parse_thread_block_items, parse_trigger_pattern,
};
use recovery::{ParseError, RecoverySuggestion};
use statements::{
    parse_scope_head, parse_stmt, parse_stmt_with_stats, parse_unsafe_lifetime_block, raw_stmt,
};

/// Parses an Arcweft source string.
#[must_use]
pub fn parse_source(source: impl Into<String>) -> ParsedSource {
    let source = source.into();
    let syntax = crate::cst::parse_cst(&source);
    let mut parser = Parser::from_syntax(&source, &syntax);
    let (tree, errors, syntax_stats) = parser.parse();
    ParsedSource::new(source, syntax, tree, errors, syntax_stats)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TopLevelDispatch {
    line: CstTopLevelLineKind,
    item: CstTopLevelItemKind,
}

impl From<&CstLine<'_>> for TopLevelDispatch {
    fn from(line: &CstLine<'_>) -> Self {
        Self {
            line: line.top_level_line_kind(),
            item: line.top_level_item_kind(),
        }
    }
}

type ContentCallParse = (
    String,
    Option<String>,
    DialogueContent,
    usize,
    Option<LinePlan>,
    Option<ScopeBlock>,
);

struct Parser<'a> {
    source: &'a str,
    events: CstLineEvents<'a>,
    index: usize,
    errors: Vec<ParseError>,
    pending_flow_items: Vec<FlowItem>,
    pending_doc: Option<DocBlock>,
    syntax_stats: SyntaxParseStats,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        let syntax = crate::cst::parse_cst(source);
        Self::from_syntax(source, &syntax)
    }

    fn from_syntax(source: &'a str, syntax: &SyntaxNode) -> Self {
        let events = cst_lines_for_source(syntax, source);
        let syntax_stats = events.stats();
        Self::from_line_events(source, events, syntax_stats)
    }

    fn from_line_events(
        source: &'a str,
        events: CstLineEvents<'a>,
        syntax_stats: SyntaxParseStats,
    ) -> Self {
        Self {
            source,
            events,
            index: 0,
            errors: Vec::new(),
            pending_flow_items: Vec::new(),
            pending_doc: None,
            syntax_stats,
        }
    }

    fn parse(&mut self) -> (TypedSyntaxTree, Vec<ParseError>, SyntaxParseStats) {
        let mut module = None;
        let mut uses = Vec::new();
        let mut items = Vec::new();
        let wiki_links = if self.source.contains("[[") {
            self.syntax_stats.wiki_scan_performed += 1;
            collect_wiki_links(self.source)
        } else {
            Vec::new()
        };

        while self.index < self.events.len() {
            self.skip_blank_and_comments();
            if self.index >= self.events.len() {
                break;
            }
            if let Some(doc) = self.take_doc_block() {
                if self.pending_doc.is_some() {
                    self.push_error(
                        *doc.range(),
                        "documentation comment is not attached to an item",
                        ["item declaration"],
                        Some(doc.text()),
                        ["move the `///` block directly before the item it documents"],
                    );
                }
                self.pending_doc = Some(doc);
                continue;
            }

            let line = self.current().clone();
            let trimmed = line.trimmed();
            let range = TextRange::new(line.start, line.end);
            let dispatch = TopLevelDispatch::from(&line);

            self.parse_top_level_line(dispatch, trimmed, range, &mut module, &mut uses, &mut items);
        }

        let tree = TypedSyntaxTree::new(source_take(self), module, uses, items, wiki_links);
        (tree, core::mem::take(&mut self.errors), self.syntax_stats)
    }

    fn take_flow_block(&mut self) -> (Cow<'a, str>, Cow<'a, str>, usize, bool) {
        let event = self.take_flow_block_event();
        (event.head, event.body, event.end, event.ok)
    }

    fn take_flow_block_event(&mut self) -> CstBlockEvent<'a> {
        let event = self.events.collect_flow_block(self.index);
        self.index = event.next_index;
        self.syntax_stats.block_owned_bytes += event.owned_bytes();
        event
    }

    fn take_function_block(&mut self) -> (Cow<'a, str>, Cow<'a, str>, usize, bool) {
        let event = self.take_block_event(CstBlockOpenRule::FunctionBody);
        (event.head, event.body, event.end, event.ok)
    }

    fn next_nonblank_line_is_brace(&self) -> bool {
        for line in self.events.iter().skip(self.index + 1) {
            if line.is_trivia() {
                continue;
            }
            let trimmed = line.trimmed();
            if !trimmed.starts_with('#') {
                return trimmed == "{";
            }
        }
        false
    }

    fn take_indented_line_range(&mut self, min_indent: usize) -> Range<usize> {
        let start = self.index;
        while self.index < self.events.len() {
            let line = self.current();
            if line.text.trim().is_empty() {
                self.index += 1;
                continue;
            }
            if indentation(&line.text) < min_indent {
                break;
            }
            self.index += 1;
        }
        start..self.index
    }

    fn collect_line_range_source(&self, range: Range<usize>) -> String {
        let mut source = String::new();
        for index in range {
            let Some(line) = self.events.get(index) else {
                break;
            };
            if !source.is_empty() {
                source.push('\n');
            }
            source.push_str(line.text());
        }
        source
    }

    fn parse_stmt_line_range(&self, range: Range<usize>) -> Vec<Stmt> {
        self.logical_stmt_line_ranges(range)
            .into_iter()
            .filter_map(|range| self.parse_stmt_line_group(range))
            .collect()
    }

    fn logical_stmt_line_ranges(&self, range: Range<usize>) -> Vec<Range<usize>> {
        let mut groups: Vec<Range<usize>> = Vec::new();
        let mut start = None;
        let mut end = range.start;
        let mut depth = CstPunctuationDeltas::default();

        for index in range {
            let Some(line) = self.events.get(index) else {
                break;
            };
            let trimmed = line.trimmed();
            if trimmed.is_empty() {
                continue;
            }
            if start.is_none()
                && trimmed.starts_with('.')
                && let Some(previous) = groups.pop()
            {
                start = Some(previous.start);
            }
            if start.is_none() {
                start = Some(index);
            }
            end = index + 1;

            let line_depth = line.punctuation_deltas();
            depth.brace += line_depth.brace;
            depth.paren += line_depth.paren;
            depth.bracket += line_depth.bracket;
            if depth.brace + depth.paren + depth.bracket <= 0 {
                if let Some(start) = start.take() {
                    groups.push(start..end);
                }
                depth = CstPunctuationDeltas::default();
            }
        }

        if let Some(start) = start
            && start < end
        {
            groups.push(start..end);
        }
        groups
    }

    fn parse_stmt_line_group(&self, range: Range<usize>) -> Option<Stmt> {
        if range.is_empty() {
            return None;
        }
        if range.end == range.start + 1 {
            return self
                .events
                .get(range.start)
                .map(|line| parse_stmt(line.trimmed()));
        }
        let source = self.collect_stmt_line_group_source(range);
        let trimmed = source.trim();
        (!trimmed.is_empty()).then(|| parse_stmt(trimmed))
    }

    fn collect_stmt_line_group_source(&self, range: Range<usize>) -> String {
        let mut source = String::new();
        for index in range {
            let Some(line) = self.events.get(index) else {
                break;
            };
            if line.trimmed().is_empty() {
                continue;
            }
            if !source.is_empty() {
                source.push('\n');
            }
            source.push_str(line.text());
        }
        source
    }

    fn take_brace_block(&mut self) -> (Cow<'a, str>, Cow<'a, str>, usize, bool) {
        let event = self.take_brace_block_event();
        (event.head, event.body, event.end, event.ok)
    }

    fn take_brace_block_event(&mut self) -> CstBlockEvent<'a> {
        self.take_block_event(CstBlockOpenRule::FirstTopLevel)
    }

    fn take_block_event(&mut self, rule: CstBlockOpenRule) -> CstBlockEvent<'a> {
        let event = self.events.collect_brace_block(self.index, rule);
        self.index = event.next_index;
        self.syntax_stats.block_owned_bytes += event.owned_bytes();
        event
    }

    fn current(&self) -> CstLine<'a> {
        self.events[self.index].clone()
    }

    fn previous_end(&self) -> usize {
        self.index
            .checked_sub(1)
            .and_then(|index| self.events.get(index))
            .map_or(0, |line| line.end)
    }

    fn skip_blank_and_comments(&mut self) {
        while self.index < self.events.len() {
            if self.current().is_trivia() {
                self.index += 1;
            } else {
                break;
            }
        }
    }

    fn take_doc_block(&mut self) -> Option<DocBlock> {
        let first = self.events.get(self.index)?;
        first.doc_comment_text()?;
        let start = first.start;
        let mut end = first.end;
        let mut lines = Vec::new();
        while self.index < self.events.len() {
            let line = self.current();
            let Some(text) = line.doc_comment_text() else {
                break;
            };
            lines.push(text.to_owned());
            end = line.end;
            self.index += 1;
        }
        Some(DocBlock::new(lines.join("\n"), TextRange::new(start, end)))
    }

    fn take_pending_doc(&mut self) -> Option<DocBlock> {
        self.pending_doc.take()
    }

    fn reject_pending_doc(&mut self, fallback_range: TextRange) {
        if let Some(doc) = self.pending_doc.take() {
            self.push_error(
                *doc.range(),
                "documentation comment is not attached to a documentable item",
                ["function or flow declaration"],
                Some(doc.text()),
                ["move the `///` block directly before a supported declaration"],
            );
        } else {
            let _ = fallback_range;
        }
    }

    fn push_error<const E: usize, const R: usize>(
        &mut self,
        range: TextRange,
        message: &str,
        expected: [&str; E],
        found: Option<&str>,
        recovery: [&str; R],
    ) {
        self.errors.push(ParseError::new(
            range,
            expected.into_iter().map(str::to_owned).collect(),
            found.map(str::to_owned),
            message.to_owned(),
            recovery
                .into_iter()
                .map(|message| RecoverySuggestion {
                    message: message.to_owned(),
                })
                .collect(),
            SourceAnchor::new(SourceName::path("<memory>"), 0..0),
        ));
    }

    fn dialogue_content(&mut self, raw: String, range: TextRange) -> DialogueContent {
        let parsed = parse_dialogue_text(&raw);
        for diagnostic in parsed.diagnostics() {
            let diagnostic_range = TextRange::new(
                range.start() + diagnostic.range().start(),
                range.start() + diagnostic.range().end(),
            );
            self.push_error(
                diagnostic_range,
                diagnostic.message(),
                ["valid dialogue text markup"],
                raw.get(diagnostic.range().start()..diagnostic.range().end()),
                [diagnostic.recovery()],
            );
        }
        DialogueContent::new(raw, parsed.into_tokens(), range)
    }
}
