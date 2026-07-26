use crate::ast::common::{DocBlock, ModuleDecl, TextRange, UseItem};
use crate::ast::dialogue::{
    ContentCall, DialogueContent, DialogueContentSourceMap, DialogueContentSourceSegment,
    SpeakerLine, SpeakerLineSurface,
};
use crate::ast::flow::{
    AuthoredExpr, Flow, FlowInit, FlowItem, ForBlock, IfBlock, IfLetBlock, LoopBlock, MatchArm,
    MatchBlock, ScopeBlock, ScopeExprBlock, SelectBlock, SelectBranch, SelectBranchHead, Stmt,
    WaitTarget, WhileBlock, WhileLetBlock,
};
use crate::ast::ids::{IdRef, RelativeId, RelativeIdSpelling};
use crate::ast::items::{Attribute, Item, RawSyntax, TypedSyntaxTree};
use crate::ast::line_plan::{BlockStyle, DeferOutcome, LinePlan};
use crate::cst::text::parse_flat_fence;
use crate::cst::{
    CstBlockEvent, CstBlockOpenRule, CstFlowItemKind, CstLetFlowItemKind, CstLine, CstLineEvents,
    CstPunctuationDeltas, CstStmtKind, CstStructuredFlowBlockKind, CstTopLevelItemKind,
    CstTopLevelLineKind, SyntaxNode, SyntaxParseStats, classify_stmt, cst_lines_for_source,
    find_matching_punctuation, find_top_level_punctuation, source_line_iter, split_leading_ident,
    split_top_level_keyword_once,
};
use crate::expr::Expr;
use crate::pattern::parse_pattern;
use crate::source::ParsedSource;
use crate::text::parse_dialogue_text;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRevision};
use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

mod action_grammar;
#[cfg(test)]
mod action_grammar_tests;
mod activity_grammar;
#[cfg(test)]
mod activity_grammar_tests;
pub mod assertion;
pub mod await_;
mod character_grammar;
#[cfg(test)]
mod character_grammar_tests;
pub mod choice;
pub mod control_flow;
mod declaration;
pub mod dialogue;
#[cfg(test)]
mod dialogue_expression_tests;
mod document;
pub(crate) use document::{ShadowFragmentKind, parse_shadow_document, parse_shadow_fragment};
mod entry_grammar;
#[cfg(test)]
mod entry_grammar_tests;
mod expression;
mod extern_capability_grammar;
#[cfg(test)]
mod extern_capability_grammar_tests;
pub mod flow;
pub mod fragment;
mod function_grammar;
#[cfg(test)]
mod function_grammar_tests;
pub mod headers;
pub mod helpers;
mod item;
#[cfg(test)]
mod item_tests;
pub mod items;
mod layer_grammar;
#[cfg(test)]
mod layer_grammar_tests;
mod lexer;
pub mod line_plan;
mod metric_grammar;
#[cfg(test)]
mod metric_grammar_tests;
mod module_use_grammar;
#[cfg(test)]
mod module_use_grammar_tests;
mod path;
mod pattern;
mod predicate_proof;
#[cfg(test)]
mod predicate_proof_tests;
pub mod proof;
pub mod recovery;
mod resource_grammar;
#[cfg(test)]
mod resource_grammar_tests;
#[cfg(test)]
mod retained_grammar_tests;
#[cfg(test)]
mod retained_header_tests;
mod rich_text_grammar;
mod shadow_flow;
#[cfg(test)]
mod shadow_flow_tests;
mod shadow_recovery;
mod signal_grammar;
#[cfg(test)]
mod signal_grammar_tests;
pub mod source;
mod statement;
pub mod statements;
pub mod style;
mod style_grammar;
#[cfg(test)]
mod style_grammar_tests;
mod test_bench_grammar;
#[cfg(test)]
mod test_bench_grammar_tests;
pub mod top_level;
mod trait_impl_grammar;
#[cfg(test)]
mod trait_impl_grammar_tests;
mod type_declaration_grammar;
#[cfg(test)]
mod type_declaration_grammar_tests;
mod type_ref;
pub mod view;
mod view_grammar;
#[cfg(test)]
mod view_grammar_tests;
use await_::{is_await_with_head, parse_await_with};
use control_flow::{
    parse_block_expr, parse_named_block_expr, parse_scope_authored_expr_body,
    parse_scope_authored_expr_body_recovering_with_base, parse_scope_expr_body, parse_stmt_lines,
    split_pattern_guard,
};
pub use fragment::{
    ExpectedToken, FragmentKind, ParseCompletion, ParseOptions, ParsedFragment, ParsedFragmentKind,
    parse_document_with_source, parse_fragment,
};
use helpers::{
    PendingDocLines, attach_plan_to_dialogue_expr, collect_logical_block_items,
    collect_logical_block_items_with_base, collect_wiki_links, contains_dialogue_expr,
    find_content_bracket, flat_block_head, indentation, is_expression_statement_call,
    is_typed_stmt, is_with_brace_head, parse_binding_pattern, parse_computation_block_kind,
    parse_dialogue_call_expr_source, parse_expr_lossy, parse_expr_lossy_with_stats,
    parse_expr_with_inline_line_plan_with_stats, parse_inline_with_colon_plan, parse_line_options,
    parse_line_plan_attachment, parse_line_plan_attachment_with_body_base, parse_outer_attribute,
    parse_owned_expr_recovering, parse_type_ref_or_error, parse_with_brace_label,
    parse_with_indent_label, retain_expr_recovery_diagnostic, source_take, split_brace_item,
    split_brace_item_with_scan, split_call_head, split_comma_args, split_optional_block_label,
    split_speaker_line, split_top_level_binding, validate_let_type_ascriptions,
};
use line_plan::{
    parse_defer_outcome, parse_thread_block, parse_thread_block_items, parse_trigger_pattern,
};
use recovery::{ParseError, ParseErrorKind, RecoverySuggestion};
use statements::{
    binding_value_start_in_line, braced_expr_source, parse_scope_head, parse_stmt,
    parse_stmt_recovering_with_base, parse_stmt_with_base, parse_unsafe_lifetime_block,
    parse_value_scope_stmt_with_stats_and_base, raw_stmt,
};

/// Parses an Arcweft source string.
#[must_use]
pub fn parse_source(source: impl Into<String>) -> ParsedSource {
    parse_source_with_options(source, ParseOptions::default())
}

pub(crate) fn parse_callback_block_expr_body_recovering_at(
    body: &str,
    base: usize,
) -> Result<crate::expr::ParsedExpr, crate::expr::ExprParseError> {
    control_flow::parse_block_expr_recovering_with_base(body, base)
}

/// Parses dialogue text content outside a full source document.
///
/// This preserves the same token model used by speaker-line and content-call
/// parsing, including recoverable text diagnostics with content-relative
/// ranges. An owning expression or document parser supplies source projection.
#[must_use]
pub fn parse_dialogue_content(raw: impl Into<String>) -> DialogueContent {
    let raw = raw.into();
    let parsed = parse_dialogue_text(&raw);
    let source_map = DialogueContentSourceMap::identity(raw.len(), 0);
    let (tokens, diagnostics) = parsed.into_parts();
    DialogueContent::new(raw, tokens, diagnostics, source_map)
}

fn parse_source_with_options(source: impl Into<String>, options: ParseOptions) -> ParsedSource {
    let source = source.into();
    let id = SourceDocumentId::try_new(format!(
        "memory:{}",
        SourceRevision::for_utf8(&source).to_hex()
    ))
    .expect("content-addressed memory source ID is valid");
    let document = SourceDocument::try_new(id, SourceName::Memory, source)
        .expect("Rust String length fits the source identity");
    parse_source_document_with_options(Arc::new(document), options)
}

fn parse_source_document_with_options(
    document: Arc<SourceDocument>,
    _options: ParseOptions,
) -> ParsedSource {
    let source = document.text();
    let syntax = crate::cst::parse_cst(source);
    let (tree, mut errors, syntax_stats) = {
        let mut parser = Parser::from_document(&document, &syntax);
        parser.parse()
    };
    errors.extend(validate_let_type_ascriptions(source));
    ParsedSource::new(document, syntax, tree, errors, syntax_stats)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TopLevelDispatch {
    line: CstTopLevelLineKind,
    item: CstTopLevelItemKind,
}

struct TopLevelSinks<'a> {
    attrs: &'a mut Vec<Attribute>,
    source_attrs_open: &'a mut bool,
    module: &'a mut Option<ModuleDecl>,
    uses: &'a mut Vec<UseItem>,
    items: &'a mut Vec<Item>,
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
    Option<(String, usize)>,
    DialogueContent,
    usize,
    Option<LinePlan>,
    Option<ScopeBlock>,
);

struct Parser<'a> {
    document: Option<&'a SourceDocument>,
    source: &'a str,
    events: CstLineEvents<'a>,
    index: usize,
    errors: Vec<ParseError>,
    pending_flow_items: Vec<FlowItem>,
    pending_doc: Option<DocBlock>,
    pending_attrs: Vec<Attribute>,
    syntax_stats: SyntaxParseStats,
    current_module_path: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct MappedDialogueSource {
    raw: String,
    source_map: DialogueContentSourceMap,
}

#[derive(Debug)]
pub(super) struct MappedDialogueSourceBuilder {
    raw: String,
    segments: Vec<DialogueContentSourceSegment>,
    source_anchor: usize,
    last_source_end: Option<usize>,
    lines: usize,
}

impl MappedDialogueSource {
    pub(super) fn slice(&self, range: TextRange) -> Option<Self> {
        let raw = self.raw.get(range.as_range())?.to_owned();
        let source_map = self.source_map.slice(range)?;
        Some(Self { raw, source_map })
    }

    pub(super) fn trim(&self) -> Option<Self> {
        let leading = self.raw.len() - self.raw.trim_start().len();
        let trimmed = self.raw.trim();
        self.slice(TextRange::new(leading, leading + trimmed.len()))
    }
}

impl MappedDialogueSourceBuilder {
    pub(super) const fn new(source_anchor: usize) -> Self {
        Self {
            raw: String::new(),
            segments: Vec::new(),
            source_anchor,
            last_source_end: None,
            lines: 0,
        }
    }

    pub(super) fn push_line(&mut self, text: &str, source_range: TextRange) {
        debug_assert_eq!(text.len(), source_range.end() - source_range.start());
        if self.lines > 0 {
            let boundary_start = self.last_source_end.unwrap_or(source_range.start());
            let boundary_end = source_range.start();
            debug_assert!(boundary_start <= boundary_end);
            let content_start = self.raw.len();
            self.raw.push('\n');
            self.segments
                .push(DialogueContentSourceSegment::normalized_newline(
                    TextRange::new(content_start, content_start + 1),
                    TextRange::new(boundary_start, boundary_end),
                ));
        }
        if !text.is_empty() {
            let content_start = self.raw.len();
            self.raw.push_str(text);
            self.segments.push(DialogueContentSourceSegment::copied(
                TextRange::new(content_start, self.raw.len()),
                source_range,
            ));
        }
        self.last_source_end = Some(source_range.end());
        self.lines += 1;
    }

    pub(super) fn finish(self) -> MappedDialogueSource {
        let source_map =
            DialogueContentSourceMap::new(self.segments, self.raw.len(), self.source_anchor);
        MappedDialogueSource {
            raw: self.raw,
            source_map,
        }
    }
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        let syntax = crate::cst::parse_cst(source);
        Self::from_syntax(source, &syntax)
    }

    fn new_with_base_offset(source: &'a str, base_offset: usize) -> Self {
        let syntax = crate::cst::parse_cst(source);
        let events = cst_lines_for_source(&syntax, source)
            .with_absolute_offsets(base_offset)
            .unwrap_or_default();
        let syntax_stats = events.stats();
        Self::from_line_events(None, "", events, syntax_stats)
    }

    fn from_syntax(source: &'a str, syntax: &SyntaxNode) -> Self {
        let events = cst_lines_for_source(syntax, source);
        let syntax_stats = events.stats();
        Self::from_line_events(None, source, events, syntax_stats)
    }

    fn from_document(document: &'a SourceDocument, syntax: &SyntaxNode) -> Self {
        let source = document.text();
        let events = cst_lines_for_source(syntax, source);
        let syntax_stats = events.stats();
        Self::from_line_events(Some(document), source, events, syntax_stats)
    }

    fn from_line_events(
        document: Option<&'a SourceDocument>,
        source: &'a str,
        events: CstLineEvents<'a>,
        syntax_stats: SyntaxParseStats,
    ) -> Self {
        Self {
            document,
            source,
            events,
            index: 0,
            errors: Vec::new(),
            pending_flow_items: Vec::new(),
            pending_doc: None,
            pending_attrs: Vec::new(),
            syntax_stats,
            current_module_path: None,
        }
    }

    fn parse(&mut self) -> (TypedSyntaxTree, Vec<ParseError>, SyntaxParseStats) {
        let mut attrs = Vec::new();
        let mut module = None;
        let mut uses = Vec::new();
        let mut items = Vec::new();
        let mut source_attrs_open = true;
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
                source_attrs_open = false;
                continue;
            }

            let line = self.current().clone();
            let trimmed = line.trimmed();
            let range = TextRange::new(line.start, line.end);
            let dispatch = TopLevelDispatch::from(&line);

            let mut sinks = TopLevelSinks {
                attrs: &mut attrs,
                source_attrs_open: &mut source_attrs_open,
                module: &mut module,
                uses: &mut uses,
                items: &mut items,
            };
            self.parse_top_level_line(dispatch, trimmed, range, &mut sinks);
        }

        self.reject_pending_attrs(TextRange::new(self.previous_end(), self.previous_end()));
        let tree = TypedSyntaxTree::new(source_take(self), attrs, module, uses, items, wiki_links);
        (tree, core::mem::take(&mut self.errors), self.syntax_stats)
    }

    fn take_flow_block_event(&mut self) -> CstBlockEvent<'a> {
        let event = self.events.collect_flow_block(self.index);
        self.index = event.next_index;
        self.syntax_stats.block_owned_bytes += event.owned_bytes();
        event
    }

    fn take_function_block_event(&mut self) -> CstBlockEvent<'a> {
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

    fn take_multiline_outer_attribute(&mut self) -> Option<Attribute> {
        let first = self.events.get(self.index)?;
        if !first.trimmed().starts_with("#[") || first.trimmed().ends_with(']') {
            return None;
        }
        let start = first.start;
        let mut end = first.end;
        let mut source = String::new();
        let mut depth = CstPunctuationDeltas::default();
        while let Some(line) = self.events.get(self.index) {
            if !source.is_empty() {
                source.push('\n');
            }
            source.push_str(line.trimmed());
            end = line.end;
            let deltas = line.punctuation_deltas();
            depth.brace += deltas.brace;
            depth.paren += deltas.paren;
            depth.bracket += deltas.bracket;
            self.index += 1;
            if depth.brace + depth.paren + depth.bracket <= 0 && line.trimmed().ends_with(']') {
                break;
            }
        }
        parse_outer_attribute(source.trim(), TextRange::new(start, end))
    }

    fn take_pending_doc(&mut self) -> Option<DocBlock> {
        self.pending_doc.take()
    }

    fn push_pending_attr(&mut self, attr: Attribute) {
        self.pending_attrs.push(attr);
    }

    fn take_pending_attrs(&mut self) -> Vec<Attribute> {
        core::mem::take(&mut self.pending_attrs)
    }

    fn reject_pending_attrs(&mut self, fallback_range: TextRange) {
        for attr in self.take_pending_attrs() {
            if attr.name() == "verify.trusted" {
                self.reject_trusted_attr(&attr);
            } else {
                self.push_error(
                    *attr.range(),
                    "attribute is not attached to an attribute-aware item",
                    ["flow", "fn", "character", "source"],
                    Some(attr.name()),
                    ["move the attribute directly before a supported declaration"],
                );
            }
        }
        let _ = fallback_range;
    }

    fn reject_pending_trusted_attrs(&mut self) {
        let attrs = self.take_pending_attrs();
        for attr in attrs {
            if attr.name() == "verify.trusted" {
                self.reject_trusted_attr(&attr);
            } else {
                self.push_pending_attr(attr);
            }
        }
    }

    fn reject_trusted_attr(&mut self, attr: &Attribute) {
        self.push_error_with_kind(
            ParseErrorKind::ProofTrustedNotProof,
            *attr.range(),
            "`verify.trusted` can only be attached to a proof",
            ["proof declaration"],
            Some(attr.name()),
            ["move the attribute directly before a proof declaration"],
        );
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
            recovery.into_iter().map(RecoverySuggestion::new).collect(),
        ));
    }

    fn push_error_with_kind<const E: usize, const R: usize>(
        &mut self,
        kind: ParseErrorKind,
        range: TextRange,
        message: &str,
        expected: [&str; E],
        found: Option<&str>,
        recovery: [&str; R],
    ) {
        self.errors.push(ParseError::new_with_kind(
            kind,
            range,
            expected.into_iter().map(str::to_owned).collect(),
            found.map(str::to_owned),
            message.to_owned(),
            recovery.into_iter().map(RecoverySuggestion::new).collect(),
        ));
    }

    fn dialogue_content(&mut self, mapped: MappedDialogueSource) -> DialogueContent {
        let parsed = parse_dialogue_text(&mapped.raw);
        for diagnostic in parsed.diagnostics() {
            let diagnostic_range = mapped
                .source_map
                .source_range(*diagnostic.range())
                .unwrap_or_else(|| {
                    TextRange::new(
                        mapped.source_map.source_anchor(),
                        mapped.source_map.source_anchor(),
                    )
                });
            self.push_error(
                diagnostic_range,
                diagnostic.message(),
                ["valid dialogue text markup"],
                mapped.raw.get(diagnostic.range().as_range()),
                [diagnostic.recovery()],
            );
        }
        let (tokens, diagnostics) = parsed.into_parts();
        DialogueContent::new(mapped.raw, tokens, diagnostics, mapped.source_map)
    }
}
