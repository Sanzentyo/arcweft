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
    CstBlockOpenRule, CstFlowItemKind, CstLetFlowItemKind, CstLine, CstLineEvents, CstStmtKind,
    CstStructuredFlowBlockKind, CstTopLevelItemKind, CstTopLevelLineKind, SyntaxNode,
    SyntaxParseStats, classify_stmt, cst_lines, find_matching_punctuation,
    find_top_level_punctuation, parse_flat_fence, punctuation_delta, source_lines,
    split_leading_ident, split_top_level_keyword_once, split_top_level_punctuation_once,
    split_top_level_punctuation_sequence_once,
};
use crate::expr::Expr;
use crate::pattern::parse_pattern;
use crate::source::ParsedSource;
use crate::text::parse_dialogue_text;
use arcweft_source::{SourceAnchor, SourceName};

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
    parse_expr_with_inline_line_plan, parse_inline_with_colon_plan, parse_line_options,
    parse_line_plan_attachment, parse_memo_block_options, parse_with_brace_label,
    parse_with_indent_label, source_take, split_brace_item, split_call_head, split_comma_args,
    split_optional_block_label, split_speaker_line, split_top_level_binding,
};
use line_plan::{
    parse_defer_outcome, parse_thread_block, parse_thread_block_items, parse_trigger_pattern,
};
use recovery::{ParseError, RecoverySuggestion};
use statements::{parse_scope_head, parse_stmt, parse_unsafe_lifetime_block, raw_stmt};

/// Parses an Arcweft source string.
#[must_use]
pub fn parse_source(source: impl Into<String>) -> ParsedSource {
    let source = source.into();
    let syntax = crate::cst::parse_cst(&source);
    let mut parser = Parser::from_syntax(source.clone(), &syntax);
    let (tree, errors, syntax_stats) = parser.parse();
    ParsedSource::new(source, syntax, tree, errors, syntax_stats)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TopLevelDispatch {
    line: CstTopLevelLineKind,
    item: CstTopLevelItemKind,
}

impl From<&CstLine> for TopLevelDispatch {
    fn from(line: &CstLine) -> Self {
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

struct Parser {
    source: String,
    events: CstLineEvents,
    index: usize,
    errors: Vec<ParseError>,
    pending_flow_items: Vec<FlowItem>,
    pending_doc: Option<DocBlock>,
    syntax_stats: SyntaxParseStats,
}

impl Parser {
    fn new(source: String) -> Self {
        let syntax = crate::cst::parse_cst(&source);
        Self::from_syntax(source, &syntax)
    }

    fn from_syntax(source: String, syntax: &SyntaxNode) -> Self {
        let events = cst_lines(syntax);
        let syntax_stats = events.stats();
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
        if self.source.contains("[[") {
            self.syntax_stats.wiki_scan_performed += 1;
        }
        let wiki_links = collect_wiki_links(&self.source);

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
            let trimmed = line.trimmed().to_owned();
            let range = TextRange::new(line.start, line.end);
            let dispatch = TopLevelDispatch::from(&line);

            self.parse_top_level_line(
                dispatch,
                &trimmed,
                range,
                &mut module,
                &mut uses,
                &mut items,
            );
        }

        let tree = TypedSyntaxTree::new(source_take(self), module, uses, items, wiki_links);
        (tree, core::mem::take(&mut self.errors), self.syntax_stats)
    }

    fn take_flow_block(&mut self) -> (String, String, usize, bool) {
        let event = self.events.collect_flow_block(self.index);
        self.index = event.next_index;
        self.syntax_stats.block_owned_bytes += event.head.len() + event.body.len();
        (event.head, event.body, event.end, event.ok)
    }

    fn take_function_block(&mut self) -> (String, String, usize, bool) {
        self.take_block_event(CstBlockOpenRule::FunctionBody)
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

    fn take_indented_await_body(&mut self, min_indent: usize) -> String {
        let mut raw = String::new();
        while self.index < self.events.len() {
            let line = self.current();
            if line.text.trim().is_empty() {
                raw.push('\n');
                self.index += 1;
                continue;
            }
            if indentation(&line.text) < min_indent {
                break;
            }
            if !raw.is_empty() {
                raw.push('\n');
            }
            raw.push_str(&line.text);
            self.index += 1;
        }
        raw
    }

    fn take_brace_block(&mut self) -> (String, String, usize, bool) {
        self.take_block_event(CstBlockOpenRule::FirstTopLevel)
    }

    fn take_block_event(&mut self, rule: CstBlockOpenRule) -> (String, String, usize, bool) {
        let event = self.events.collect_brace_block(self.index, rule);
        self.index = event.next_index;
        self.syntax_stats.block_owned_bytes += event.head.len() + event.body.len();
        (event.head, event.body, event.end, event.ok)
    }

    fn current(&self) -> &CstLine {
        &self.events[self.index]
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
