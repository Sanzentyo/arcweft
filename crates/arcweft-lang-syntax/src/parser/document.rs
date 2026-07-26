//! One-pass lexer and root event stream for the staged document grammar.

#![allow(
    dead_code,
    reason = "the shadow document parser remains private until the atomic syntax switch"
)]

use arcweft_source::{SourceDocument, SourceRange};

use crate::grammar::budget::GrammarBudget;
use crate::grammar::build::{GrammarBuild, GrammarBuildError, build_grammar};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

use super::expression::emit_expression;
use super::item::{classify_top_level_item, is_declaration_item_kind};
use super::lexer::{DocumentLexer, LexToken};
use super::pattern::emit_pattern;
use super::statement::emit_statement_fragment;
use super::type_ref::emit_type;

/// Shared cursor and event sink for every private shadow grammar parser.
pub(super) struct ShadowDocumentParser<'source, 'events> {
    source: &'source str,
    tokens: &'source [LexToken],
    cursor: usize,
    empty_offset: usize,
    events: &'events mut Vec<SyntaxEvent>,
    budget: &'events mut GrammarBudget,
}

impl<'source, 'events> ShadowDocumentParser<'source, 'events> {
    pub(super) fn new(
        source: &'source str,
        tokens: &'source [LexToken],
        events: &'events mut Vec<SyntaxEvent>,
        budget: &'events mut GrammarBudget,
    ) -> Self {
        Self {
            source,
            tokens,
            cursor: 0,
            empty_offset: 0,
            events,
            budget,
        }
    }

    fn for_fragment(
        source: &'source str,
        tokens: &'source [LexToken],
        empty_offset: usize,
        events: &'events mut Vec<SyntaxEvent>,
        budget: &'events mut GrammarBudget,
    ) -> Self {
        Self {
            source,
            tokens,
            cursor: 0,
            empty_offset,
            events,
            budget,
        }
    }

    pub(super) fn is_at_end(&self) -> bool {
        self.cursor == self.tokens.len()
    }

    pub(super) fn current(&self) -> Option<LexToken> {
        self.tokens.get(self.cursor).copied()
    }

    pub(super) fn current_kind(&self) -> Option<SyntaxKind> {
        self.current().map(LexToken::kind)
    }

    pub(super) fn current_text(&self) -> Option<&'source str> {
        self.current()
            .map(|token| &self.source[token.range().as_range()])
    }

    pub(super) const fn source(&self) -> &'source str {
        self.source
    }

    pub(super) fn current_offset(&self) -> usize {
        self.current().map_or_else(
            || {
                self.tokens
                    .last()
                    .map_or(self.empty_offset, |token| token.range().end())
            },
            |token| token.range().start(),
        )
    }

    pub(super) fn at(&self, spelling: &str) -> bool {
        self.current_text() == Some(spelling)
    }

    pub(super) fn bump(&mut self) -> Option<LexToken> {
        let token = self.current()?;
        let event = SyntaxEvent::token(token.kind(), token.range());
        if self.budget.event(&event) {
            self.events.push(event);
        }
        self.cursor += 1;
        Some(token)
    }

    /// Advances one already-lexed token without emitting it.
    ///
    /// `RichText` uses this only when the same token is partitioned into exact
    /// quote/content ranges in the current event transaction. The caller must
    /// emit lossless replacement token events before building the tree.
    pub(super) fn take_for_partition(&mut self) -> Option<LexToken> {
        let token = self.current()?;
        self.cursor += 1;
        Some(token)
    }

    pub(super) fn start(&mut self, kind: SyntaxKind, role: SyntaxRole) {
        if self.budget.start(kind, role) {
            self.events.push(SyntaxEvent::start(kind, role));
        }
    }

    pub(super) fn event_position(&self) -> usize {
        self.events.len()
    }

    pub(super) fn started_kind_since(&self, position: usize, kind: SyntaxKind) -> bool {
        self.events[position..].iter().any(
            |event| matches!(event, SyntaxEvent::StartNode { kind: actual, .. } if *actual == kind),
        )
    }

    pub(super) fn insert_start(&mut self, position: usize, kind: SyntaxKind, role: SyntaxRole) {
        if self.budget.start(kind, role) {
            self.events.insert(position, SyntaxEvent::start(kind, role));
        }
    }

    pub(super) fn set_start_role(&mut self, position: usize, role: SyntaxRole) {
        if self.budget.failure().is_some() {
            return;
        }
        let Some(SyntaxEvent::StartNode {
            role: current_role, ..
        }) = self.events.get_mut(position)
        else {
            panic!("completed grammar marker must point to a node start event");
        };
        *current_role = role;
    }

    pub(super) fn finish(&mut self) {
        if self.budget.finish() {
            self.events.push(SyntaxEvent::FinishNode);
        }
    }

    pub(super) fn push(&mut self, event: SyntaxEvent) {
        if self.budget.event(&event) {
            self.events.push(event);
        }
    }

    pub(super) fn charge_assertion_condition(&mut self) {
        self.budget.assertion_condition();
    }

    pub(super) fn enter_prefix_expression(&mut self) -> bool {
        self.budget.enter_prefix_expression()
    }

    pub(super) fn leave_prefix_expression(&mut self) {
        self.budget.leave_prefix_expression();
    }

    pub(super) const fn budget_failed(&self) -> bool {
        self.budget.failure().is_some()
    }

    pub(super) fn bump_trivia(&mut self) {
        while self.current_kind().is_some_and(is_trivia_kind) {
            self.bump();
        }
    }

    pub(super) fn next_significant(&self) -> Option<(usize, LexToken, &'source str)> {
        self.tokens[self.cursor..]
            .iter()
            .copied()
            .enumerate()
            .find(|(_, token)| !is_trivia_kind(token.kind()))
            .map(|(relative, token)| {
                (
                    self.cursor + relative,
                    token,
                    &self.source[token.range().as_range()],
                )
            })
    }

    pub(super) fn bump_through(&mut self, inclusive_index: usize) {
        while self.cursor <= inclusive_index && !self.is_at_end() {
            self.bump();
        }
    }

    pub(super) const fn cursor(&self) -> usize {
        self.cursor
    }

    pub(super) fn token_at(&self, index: usize) -> Option<LexToken> {
        self.tokens.get(index).copied()
    }

    pub(super) fn offset_at_token_boundary(&self, index: usize) -> Option<usize> {
        if let Some(token) = self.tokens.get(index) {
            return Some(token.range().start());
        }
        (index == self.tokens.len()).then(|| {
            self.tokens
                .last()
                .map_or(self.empty_offset, |token| token.range().end())
        })
    }

    pub(super) fn token_boundary_index(&self, offset: usize) -> Option<usize> {
        if offset
            == self
                .tokens
                .last()
                .map_or(self.empty_offset, |token| token.range().end())
        {
            return Some(self.tokens.len());
        }
        self.tokens[self.cursor..]
            .iter()
            .position(|token| token.range().start() == offset)
            .map(|relative| self.cursor + relative)
    }

    pub(super) fn text_of(&self, token: LexToken) -> &'source str {
        &self.source[token.range().as_range()]
    }
}

/// Builds the private lossless root tree without allocating syntax identity.
pub(crate) fn parse_shadow_document(
    document: &SourceDocument,
) -> Result<GrammarBuild, GrammarBuildError> {
    let tokens = DocumentLexer::new(document.text()).lex();
    build_shadow_root(document, &tokens, |tokens, events, budget| {
        start_event(events, budget, SyntaxKind::ItemList, SyntaxRole::Element(0));
        emit_logical_lines(document.text(), tokens, events, budget)?;
        finish_event(events, budget);
        Ok(())
    })
}

/// Typed fragment family accepted by the private bound-fragment transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShadowFragmentKind {
    Expression,
    Type,
    Pattern,
    Statement,
}

/// Parses one standalone fragment through the shared lexer, grammar emitters,
/// event budget, recovery, and lossless root transaction.
pub(crate) fn parse_shadow_fragment(
    document: &SourceDocument,
    span: SourceRange,
    kind: ShadowFragmentKind,
) -> Result<GrammarBuild, GrammarBuildError> {
    document
        .span(span)
        .map_err(|_| GrammarBuildError::InvalidFragmentRange {
            start: span.start(),
            end: span.end(),
            source_len: document.text().len(),
        })?;
    let tokens = DocumentLexer::for_range(document.text(), span).lex();
    build_shadow_root(document, &tokens, |tokens, events, budget| {
        if span.start() > 0 {
            push_event(
                events,
                budget,
                SyntaxEvent::token(SyntaxKind::TextToken, SourceRange::new(0, span.start())),
            );
        }
        let mut parser = ShadowDocumentParser::for_fragment(
            document.text(),
            tokens,
            span.start(),
            events,
            budget,
        );
        parser.bump_trivia();
        match kind {
            ShadowFragmentKind::Expression => {
                emit_expression(&mut parser, tokens.len(), SyntaxRole::Element(0));
            }
            ShadowFragmentKind::Type => {
                emit_type(&mut parser, tokens.len(), SyntaxRole::Element(0));
            }
            ShadowFragmentKind::Pattern => {
                emit_pattern(&mut parser, tokens.len(), SyntaxRole::Element(0));
            }
            ShadowFragmentKind::Statement => {
                emit_statement_fragment(&mut parser, tokens.len(), SyntaxRole::Element(0));
            }
        }
        while parser.bump().is_some() {}
        if span.end() < document.text().len() {
            push_event(
                events,
                budget,
                SyntaxEvent::token(
                    SyntaxKind::TextToken,
                    SourceRange::new(span.end(), document.text().len()),
                ),
            );
        }
        Ok(())
    })
}

fn build_shadow_root(
    document: &SourceDocument,
    tokens: &[LexToken],
    emit_body: impl FnOnce(
        &[LexToken],
        &mut Vec<SyntaxEvent>,
        &mut GrammarBudget,
    ) -> Result<(), GrammarBuildError>,
) -> Result<GrammarBuild, GrammarBuildError> {
    let mut events = Vec::with_capacity(tokens.len() + 8);
    let mut budget = GrammarBudget::default();
    start_event(
        &mut events,
        &mut budget,
        SyntaxKind::SourceFile,
        SyntaxRole::Root,
    );
    emit_body(tokens, &mut events, &mut budget)?;
    push_event(
        &mut events,
        &mut budget,
        SyntaxEvent::token(
            SyntaxKind::EofToken,
            SourceRange::new(document.text().len(), document.text().len()),
        ),
    );
    finish_event(&mut events, &mut budget);
    budget_failure(&budget)?;
    build_grammar(document, &events)
}

fn emit_logical_lines(
    source: &str,
    tokens: &[LexToken],
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) -> Result<(), GrammarBuildError> {
    let lines = logical_token_ranges(source, tokens);
    let mut line = 0_usize;
    let mut ordinal = 0_u32;
    while line < lines.len() {
        if let Some((declaration_line, kind)) =
            structured_declaration_after_outer_prefixes(source, tokens, &lines, line)
        {
            let last = declaration_group_end(source, tokens, &lines, declaration_line, kind);
            let grouped = &tokens[lines[line].start..lines[last].end];
            emit_declaration_item(source, grouped, kind, ordinal, events, budget)?;
            line = last + 1;
        } else {
            let range = lines[line];
            let line_tokens = &tokens[range.start..range.end];
            emit_logical_line(source, line_tokens, ordinal, events, budget)?;
            line += 1;
        }
        ordinal = ordinal
            .checked_add(1)
            .ok_or(GrammarBuildError::ChildIndexExhausted)?;
    }
    Ok(())
}

fn structured_declaration_after_outer_prefixes(
    source: &str,
    tokens: &[LexToken],
    lines: &[LogicalTokenRange],
    first: usize,
) -> Option<(usize, SyntaxKind)> {
    let mut declaration = first;
    while let Some(range) = lines.get(declaration).copied() {
        let line_tokens = &tokens[range.start..range.end];
        if !is_outer_prefix_line(source, line_tokens) {
            break;
        }
        declaration += 1;
    }

    let range = lines.get(declaration).copied()?;
    let kind = classify_top_level_item(source, &tokens[range.start..range.end])?;
    matches!(
        kind,
        SyntaxKind::FlowItem
            | SyntaxKind::FunctionItem
            | SyntaxKind::PredicateItem
            | SyntaxKind::ProofItem
            | SyntaxKind::EnumItem
            | SyntaxKind::StructItem
            | SyntaxKind::TypeAliasItem
            | SyntaxKind::TraitItem
            | SyntaxKind::ImplItem
            | SyntaxKind::ResourceDeclarationItem
            | SyntaxKind::CharacterDeclarationItem
            | SyntaxKind::ViewDeclarationItem
            | SyntaxKind::ActionDeclarationItem
            | SyntaxKind::ActivityDeclarationItem
            | SyntaxKind::SignalDeclarationItem
            | SyntaxKind::MetricDeclarationItem
            | SyntaxKind::LayerDeclarationItem
            | SyntaxKind::EntryDeclarationItem
            | SyntaxKind::ExternCapabilityItem
            | SyntaxKind::TestItem
            | SyntaxKind::BenchItem
            | SyntaxKind::SourceItem
            | SyntaxKind::StyleItem
    )
    .then_some((declaration, kind))
}

fn is_outer_prefix_line(source: &str, tokens: &[LexToken]) -> bool {
    is_documentation_line(tokens)
        || classify_top_level_item(source, tokens) == Some(SyntaxKind::OuterAttribute)
}

fn is_documentation_line(tokens: &[LexToken]) -> bool {
    let mut saw_documentation = false;
    for token in tokens {
        match token.kind {
            SyntaxKind::WhitespaceToken | SyntaxKind::NewlineToken => {}
            SyntaxKind::DocCommentToken => saw_documentation = true,
            _ => return false,
        }
    }
    saw_documentation
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LogicalTokenRange {
    start: usize,
    end: usize,
}

fn logical_token_ranges(source: &str, tokens: &[LexToken]) -> Vec<LogicalTokenRange> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut delimiter_depth = 0_usize;
    for (index, token) in tokens.iter().enumerate() {
        delimiter_depth = delimiter_depth_after(source, token.kind, token.range, delimiter_depth);
        let header_angle_open = token.kind == SyntaxKind::NewlineToken
            && delimiter_depth == 0
            && declaration_header_angle_is_open(source, &tokens[start..=index]);
        let nested_delimiter_open = delimiter_depth != 0 || header_angle_open;
        let recovery_sync = token.kind == SyntaxKind::NewlineToken
            && nested_delimiter_open
            && begins_unindented_declaration(source, tokens, index + 1);
        if token.kind == SyntaxKind::NewlineToken && (!nested_delimiter_open || recovery_sync) {
            ranges.push(LogicalTokenRange {
                start,
                end: index + 1,
            });
            start = index + 1;
            if recovery_sync {
                delimiter_depth = 0;
            }
        }
    }
    if start < tokens.len() {
        ranges.push(LogicalTokenRange {
            start,
            end: tokens.len(),
        });
    }
    ranges
}

fn begins_unindented_declaration(source: &str, tokens: &[LexToken], start: usize) -> bool {
    let mut line = start;
    loop {
        let Some(first) = tokens.get(line) else {
            return false;
        };
        if matches!(
            first.kind,
            SyntaxKind::WhitespaceToken | SyntaxKind::NewlineToken
        ) {
            return false;
        }
        let end = recovery_logical_line_end(source, tokens, line);
        let line_tokens = &tokens[line..end];
        if is_outer_prefix_line(source, line_tokens) {
            line = end;
            continue;
        }
        return classify_top_level_item(source, line_tokens).is_some_and(is_declaration_item_kind);
    }
}

fn recovery_logical_line_end(source: &str, tokens: &[LexToken], start: usize) -> usize {
    let mut delimiter_depth = 0_usize;
    for (relative, token) in tokens[start..].iter().enumerate() {
        delimiter_depth = delimiter_depth_after(source, token.kind, token.range, delimiter_depth);
        if token.kind == SyntaxKind::NewlineToken && delimiter_depth == 0 {
            return start + relative + 1;
        }
    }
    tokens.len()
}

fn declaration_group_end(
    source: &str,
    tokens: &[LexToken],
    lines: &[LogicalTokenRange],
    first: usize,
    kind: SyntaxKind,
) -> usize {
    let mut last = first;
    loop {
        let grouped = &tokens[lines[first].start..lines[last].end];
        if kind != SyntaxKind::TypeAliasItem && declaration_has_body(source, grouped, kind) {
            return last;
        }
        let Some(next) = lines.get(last + 1).copied() else {
            return last;
        };
        let next_tokens = &tokens[next.start..next.end];
        if kind == SyntaxKind::SourceItem
            && classify_top_level_item(source, next_tokens).is_some_and(is_declaration_item_kind)
        {
            return last;
        }
        if (kind == SyntaxKind::TypeAliasItem && line_starts_with(source, next_tokens, "where"))
            || declaration_header_angle_is_open(source, grouped)
            || declaration_continuation_line(source, next_tokens)
        {
            last += 1;
        } else {
            return last;
        }
    }
}

fn line_starts_with(source: &str, tokens: &[LexToken], spelling: &str) -> bool {
    tokens
        .iter()
        .find(|token| !is_trivia_kind(token.kind))
        .is_some_and(|token| &source[token.range.as_range()] == spelling)
}

fn declaration_has_body(source: &str, tokens: &[LexToken], kind: SyntaxKind) -> bool {
    let mut depth = 0_usize;
    let mut contract_list = false;
    for token in tokens {
        if token.kind == SyntaxKind::NewlineToken && depth == 0 {
            contract_list = false;
            continue;
        }
        let text = &source[token.range.as_range()];
        if kind == SyntaxKind::SourceItem && text == "{" {
            return true;
        }
        if depth == 0
            && matches!(kind, SyntaxKind::FlowItem | SyntaxKind::FunctionItem)
            && matches!(text, "reads" | "effects" | "modifies")
        {
            contract_list = true;
            continue;
        }
        if token.kind != SyntaxKind::PunctuationToken {
            continue;
        }
        if depth == 0 && matches!(text, "=" | "{") {
            if text == "{" && contract_list {
                contract_list = false;
                depth = 1;
                continue;
            }
            return true;
        }
        match text {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

fn declaration_header_angle_is_open(source: &str, tokens: &[LexToken]) -> bool {
    if !classify_top_level_item(source, tokens).is_some_and(is_declaration_item_kind) {
        return false;
    }

    let mut angle = 0_usize;
    for token in tokens {
        let text = &source[token.range.as_range()];
        if angle == 0 && matches!(text, "requires" | "ensures" | "=" | "{") {
            return false;
        }
        if token.kind != SyntaxKind::PunctuationToken {
            continue;
        }
        match text {
            "<" => angle += 1,
            ">" => angle = angle.saturating_sub(1),
            _ => {}
        }
    }
    angle != 0
}

fn declaration_continuation_line(source: &str, tokens: &[LexToken]) -> bool {
    tokens
        .iter()
        .find(|token| !is_trivia_kind(token.kind))
        .is_none_or(|token| {
            matches!(
                &source[token.range.as_range()],
                "(" | "where"
                    | "requires"
                    | "ensures"
                    | "invariant"
                    | "assume"
                    | "reads"
                    | "effects"
                    | "modifies"
                    | "decreases"
                    | "="
                    | "{"
                    | "->"
            )
        })
}

#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive top-level kind-to-grammar-owner dispatcher stays in one auditable table"
)]
fn emit_declaration_item(
    source: &str,
    tokens: &[LexToken],
    kind: SyntaxKind,
    ordinal: u32,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) -> Result<(), GrammarBuildError> {
    let item_start = events.len();
    match kind {
        SyntaxKind::FlowItem => super::shadow_flow::emit_declaration(
            source,
            tokens,
            SyntaxRole::Element(ordinal),
            events,
            budget,
        ),
        SyntaxKind::FunctionItem => super::function_grammar::emit_declaration(
            source,
            tokens,
            SyntaxRole::Element(ordinal),
            events,
            budget,
        ),
        SyntaxKind::PredicateItem | SyntaxKind::ProofItem => {
            super::predicate_proof::emit_declaration(
                source,
                tokens,
                kind,
                SyntaxRole::Element(ordinal),
                events,
                budget,
            );
        }
        SyntaxKind::EnumItem | SyntaxKind::StructItem | SyntaxKind::TypeAliasItem => {
            super::type_declaration_grammar::emit_declaration(
                source,
                tokens,
                kind,
                SyntaxRole::Element(ordinal),
                events,
                budget,
            );
        }
        SyntaxKind::TraitItem | SyntaxKind::ImplItem => {
            super::trait_impl_grammar::emit_declaration(
                source,
                tokens,
                kind,
                SyntaxRole::Element(ordinal),
                events,
                budget,
            );
        }
        SyntaxKind::ResourceDeclarationItem => super::resource_grammar::emit_declaration(
            source,
            tokens,
            SyntaxRole::Element(ordinal),
            events,
            budget,
        ),
        SyntaxKind::CharacterDeclarationItem
        | SyntaxKind::ViewDeclarationItem
        | SyntaxKind::ActionDeclarationItem
        | SyntaxKind::ActivityDeclarationItem
        | SyntaxKind::SignalDeclarationItem
        | SyntaxKind::MetricDeclarationItem
        | SyntaxKind::LayerDeclarationItem => {
            emit_retained_declaration_item(source, tokens, kind, ordinal, events, budget);
        }
        SyntaxKind::EntryDeclarationItem => super::entry_grammar::emit_declaration(
            source,
            tokens,
            SyntaxRole::Element(ordinal),
            events,
            budget,
        ),
        SyntaxKind::ExternCapabilityItem => super::extern_capability_grammar::emit_declaration(
            source,
            tokens,
            SyntaxRole::Element(ordinal),
            events,
            budget,
        ),
        SyntaxKind::TestItem | SyntaxKind::BenchItem => {
            super::test_bench_grammar::emit_declaration(
                source,
                tokens,
                kind,
                SyntaxRole::Element(ordinal),
                events,
                budget,
            );
        }
        SyntaxKind::SourceItem => super::source_grammar::emit_declaration(
            source,
            tokens,
            SyntaxRole::Element(ordinal),
            events,
            budget,
        ),
        SyntaxKind::StyleItem => super::style_grammar::emit_declaration(
            source,
            tokens,
            SyntaxRole::Element(ordinal),
            events,
            budget,
        ),
        _ => unreachable!("only structured declaration kinds are grouped"),
    }
    budget_failure(budget)?;
    wrap_declaration_logical_lines(source, item_start, events);
    Ok(())
}

fn emit_retained_declaration_item(
    source: &str,
    tokens: &[LexToken],
    kind: SyntaxKind,
    ordinal: u32,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let role = SyntaxRole::Element(ordinal);
    match kind {
        SyntaxKind::CharacterDeclarationItem => {
            super::character_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::ViewDeclarationItem => {
            super::view_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::ActionDeclarationItem => {
            super::action_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::ActivityDeclarationItem => {
            super::activity_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::SignalDeclarationItem => {
            super::signal_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::MetricDeclarationItem => {
            super::metric_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::LayerDeclarationItem => {
            super::layer_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        _ => unreachable!("retained declaration dispatcher receives a retained item kind"),
    }
}

fn wrap_declaration_logical_lines(source: &str, item_start: usize, events: &mut Vec<SyntaxEvent>) {
    let finish = events.pop().expect("declaration finish event");
    debug_assert_eq!(finish, SyntaxEvent::FinishNode);
    let inner = events.split_off(item_start + 1);
    let mut line_open = false;
    let mut line_ordinal = 0_u32;
    let mut nested_depth = 0_usize;
    let mut delimiter_depth = 0_usize;
    let mut header_angle_depth = 0_usize;
    let mut in_declaration_header = true;
    let mut pending_boundary = false;
    let mut prewrapped_depth = 0_usize;

    for event in inner {
        if prewrapped_depth != 0 {
            match &event {
                SyntaxEvent::StartNode { .. } => prewrapped_depth += 1,
                SyntaxEvent::FinishNode => prewrapped_depth -= 1,
                SyntaxEvent::Token { .. }
                | SyntaxEvent::MissingToken { .. }
                | SyntaxEvent::Diagnostic(_) => {}
            }
            events.push(event);
            continue;
        }
        if !line_open
            && matches!(
                &event,
                SyntaxEvent::StartNode {
                    kind: SyntaxKind::DocBlock | SyntaxKind::AttributeList,
                    ..
                }
            )
        {
            prewrapped_depth = 1;
            events.push(event);
            continue;
        }
        if !line_open && !matches!(event, SyntaxEvent::Diagnostic(_)) {
            events.push(SyntaxEvent::start(
                SyntaxKind::LogicalLine,
                SyntaxRole::Element(line_ordinal),
            ));
            line_open = true;
        }

        match &event {
            SyntaxEvent::StartNode { .. } => nested_depth += 1,
            SyntaxEvent::FinishNode => nested_depth = nested_depth.saturating_sub(1),
            SyntaxEvent::Token { kind, range } => {
                let text = &source[range.as_range()];
                if in_declaration_header {
                    if matches!(text, "requires" | "ensures" | "=" | "{") {
                        in_declaration_header = false;
                        header_angle_depth = 0;
                    } else if *kind == SyntaxKind::PunctuationToken {
                        match text {
                            "<" => header_angle_depth += 1,
                            ">" => header_angle_depth = header_angle_depth.saturating_sub(1),
                            _ => {}
                        }
                    }
                }
                if *kind == SyntaxKind::PunctuationToken {
                    delimiter_depth = delimiter_depth_after(source, *kind, *range, delimiter_depth);
                }
                if *kind == SyntaxKind::NewlineToken
                    && delimiter_depth == 0
                    && header_angle_depth == 0
                {
                    pending_boundary = true;
                }
            }
            SyntaxEvent::MissingToken { .. } | SyntaxEvent::Diagnostic(_) => {}
        }
        events.push(event);

        if pending_boundary && nested_depth == 0 && line_open {
            events.push(SyntaxEvent::FinishNode);
            line_open = false;
            line_ordinal = line_ordinal.saturating_add(1);
            pending_boundary = false;
        }
    }
    if line_open {
        events.push(SyntaxEvent::FinishNode);
    }
    events.push(finish);
}

fn emit_logical_line(
    source: &str,
    tokens: &[LexToken],
    ordinal: u32,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) -> Result<(), GrammarBuildError> {
    start_event(
        events,
        budget,
        SyntaxKind::LogicalLine,
        SyntaxRole::Element(ordinal),
    );
    let item = classify_top_level_item(source, tokens);
    match item {
        Some(kind @ (SyntaxKind::ModuleDeclaration | SyntaxKind::UseDeclaration)) => {
            super::module_use_grammar::emit_declaration(
                source,
                tokens,
                kind,
                SyntaxRole::Element(ordinal),
                events,
                budget,
            );
        }
        Some(SyntaxKind::FlowItem) => {
            super::shadow_flow::emit_declaration(
                source,
                tokens,
                SyntaxRole::Element(ordinal),
                events,
                budget,
            );
        }
        Some(kind) => {
            start_event(events, budget, kind, SyntaxRole::Element(ordinal));
            for token in tokens {
                push_event(events, budget, SyntaxEvent::token(token.kind, token.range));
            }
            if kind == SyntaxKind::ErrorItem {
                let first = tokens
                    .iter()
                    .find(|token| !is_trivia_kind(token.kind))
                    .expect("classified error item has a significant token");
                let last = tokens
                    .iter()
                    .rev()
                    .find(|token| !is_trivia_kind(token.kind))
                    .expect("classified error item has a significant token");
                push_event(
                    events,
                    budget,
                    SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                        "syntax.item.expected_declaration",
                        SourceRange::new(first.range.start(), last.range.end()),
                        "regular Arcweft source accepts declarations at the top level",
                    )),
                );
            }
            finish_event(events, budget);
        }
        None => {
            for token in tokens {
                push_event(events, budget, SyntaxEvent::token(token.kind, token.range));
            }
        }
    }
    finish_event(events, budget);
    budget_failure(budget)
}

fn start_event(
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
    kind: SyntaxKind,
    role: SyntaxRole,
) {
    if budget.start(kind, role) {
        events.push(SyntaxEvent::start(kind, role));
    }
}

fn finish_event(events: &mut Vec<SyntaxEvent>, budget: &mut GrammarBudget) {
    if budget.finish() {
        events.push(SyntaxEvent::FinishNode);
    }
}

fn push_event(events: &mut Vec<SyntaxEvent>, budget: &mut GrammarBudget, event: SyntaxEvent) {
    if budget.event(&event) {
        events.push(event);
    }
}

fn budget_failure(budget: &GrammarBudget) -> Result<(), GrammarBuildError> {
    budget
        .failure()
        .map_or(Ok(()), |limit| Err(GrammarBuildError::LimitExceeded(limit)))
}

fn delimiter_depth_after(
    source: &str,
    kind: SyntaxKind,
    range: SourceRange,
    depth: usize,
) -> usize {
    if kind != SyntaxKind::PunctuationToken {
        return depth;
    }
    match &source[range.as_range()] {
        "(" | "[" | "{" => depth + 1,
        ")" | "]" | "}" => depth.saturating_sub(1),
        _ => depth,
    }
}

const fn is_trivia_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
