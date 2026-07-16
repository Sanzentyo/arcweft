//! One-pass lexer and root event stream for the staged document grammar.

#![allow(
    dead_code,
    reason = "the shadow document parser remains private until the atomic syntax switch"
)]

use arcweft_source::{SourceDocument, SourceRange};

use crate::grammar::build::{GrammarBuild, GrammarBuildError, build_grammar};
use crate::grammar::event::SyntaxEvent;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

use super::lexer::{DocumentLexer, LexToken};

/// Shared cursor and event sink for every private shadow grammar parser.
pub(super) struct ShadowDocumentParser<'source, 'events> {
    source: &'source str,
    tokens: &'source [LexToken],
    cursor: usize,
    events: &'events mut Vec<SyntaxEvent>,
}

impl<'source, 'events> ShadowDocumentParser<'source, 'events> {
    pub(super) fn new(
        source: &'source str,
        tokens: &'source [LexToken],
        events: &'events mut Vec<SyntaxEvent>,
    ) -> Self {
        Self {
            source,
            tokens,
            cursor: 0,
            events,
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

    pub(super) fn current_offset(&self) -> usize {
        self.current().map_or_else(
            || self.tokens.last().map_or(0, |token| token.range().end()),
            |token| token.range().start(),
        )
    }

    pub(super) fn at(&self, spelling: &str) -> bool {
        self.current_text() == Some(spelling)
    }

    pub(super) fn bump(&mut self) -> Option<LexToken> {
        let token = self.current()?;
        self.events
            .push(SyntaxEvent::token(token.kind(), token.range()));
        self.cursor += 1;
        Some(token)
    }

    pub(super) fn start(&mut self, kind: SyntaxKind, role: SyntaxRole) {
        self.events.push(SyntaxEvent::start(kind, role));
    }

    pub(super) fn event_position(&self) -> usize {
        self.events.len()
    }

    pub(super) fn insert_start(&mut self, position: usize, kind: SyntaxKind, role: SyntaxRole) {
        self.events.insert(position, SyntaxEvent::start(kind, role));
    }

    pub(super) fn set_start_role(&mut self, position: usize, role: SyntaxRole) {
        let Some(SyntaxEvent::StartNode {
            role: current_role, ..
        }) = self.events.get_mut(position)
        else {
            panic!("completed grammar marker must point to a node start event");
        };
        *current_role = role;
    }

    pub(super) fn finish(&mut self) {
        self.events.push(SyntaxEvent::FinishNode);
    }

    pub(super) fn push(&mut self, event: SyntaxEvent) {
        self.events.push(event);
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

    pub(super) fn text_of(&self, token: LexToken) -> &'source str {
        &self.source[token.range().as_range()]
    }
}

/// Builds the private lossless root tree without allocating syntax identity.
pub(super) fn parse_shadow_document(
    document: &SourceDocument,
) -> Result<GrammarBuild, GrammarBuildError> {
    let tokens = DocumentLexer::new(document.text()).lex();
    let mut events = Vec::with_capacity(tokens.len() + 8);
    events.push(SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root));
    events.push(SyntaxEvent::start(
        SyntaxKind::ItemList,
        SyntaxRole::Element(0),
    ));
    emit_logical_lines(document.text(), &tokens, &mut events)?;
    events.push(SyntaxEvent::token(
        SyntaxKind::EofToken,
        SourceRange::new(document.text().len(), document.text().len()),
    ));
    events.push(SyntaxEvent::FinishNode);
    events.push(SyntaxEvent::FinishNode);
    build_grammar(document, &events)
}

fn emit_logical_lines(
    source: &str,
    tokens: &[LexToken],
    events: &mut Vec<SyntaxEvent>,
) -> Result<(), GrammarBuildError> {
    let lines = logical_token_ranges(source, tokens);
    let mut line = 0_usize;
    let mut ordinal = 0_u32;
    while line < lines.len() {
        if let Some((declaration_line, kind)) =
            predicate_or_proof_after_outer_prefixes(source, tokens, &lines, line)
        {
            let last = declaration_group_end(source, tokens, &lines, declaration_line);
            let grouped = &tokens[lines[line].start..lines[last].end];
            emit_declaration_item(source, grouped, kind, ordinal, events);
            line = last + 1;
        } else {
            let range = lines[line];
            let line_tokens = &tokens[range.start..range.end];
            emit_logical_line(source, line_tokens, ordinal, events);
            line += 1;
        }
        ordinal = ordinal
            .checked_add(1)
            .ok_or(GrammarBuildError::ChildIndexExhausted)?;
    }
    Ok(())
}

fn predicate_or_proof_after_outer_prefixes(
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
    matches!(kind, SyntaxKind::PredicateItem | SyntaxKind::ProofItem).then_some((declaration, kind))
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
        if token.kind == SyntaxKind::NewlineToken && delimiter_depth == 0 {
            ranges.push(LogicalTokenRange {
                start,
                end: index + 1,
            });
            start = index + 1;
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

fn declaration_group_end(
    source: &str,
    tokens: &[LexToken],
    lines: &[LogicalTokenRange],
    first: usize,
) -> usize {
    let mut last = first;
    loop {
        let grouped = &tokens[lines[first].start..lines[last].end];
        if declaration_has_body(source, grouped) {
            return last;
        }
        let Some(next) = lines.get(last + 1).copied() else {
            return last;
        };
        let next_tokens = &tokens[next.start..next.end];
        if declaration_header_angle_is_open(source, grouped)
            || declaration_continuation_line(source, next_tokens)
        {
            last += 1;
        } else {
            return last;
        }
    }
}

fn declaration_has_body(source: &str, tokens: &[LexToken]) -> bool {
    let mut depth = 0_usize;
    for token in tokens {
        if token.kind != SyntaxKind::PunctuationToken {
            continue;
        }
        let text = &source[token.range.as_range()];
        if depth == 0 && matches!(text, "=" | "{") {
            return true;
        }
        match text {
            "(" | "[" | "<" => depth += 1,
            ")" | "]" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

fn declaration_header_angle_is_open(source: &str, tokens: &[LexToken]) -> bool {
    let mut angle = 0_usize;
    for token in tokens {
        if token.kind != SyntaxKind::PunctuationToken {
            continue;
        }
        match &source[token.range.as_range()] {
            "(" if angle == 0 => return false,
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
                "where" | "requires" | "ensures" | "=" | "{" | "->"
            )
        })
}

fn emit_declaration_item(
    source: &str,
    tokens: &[LexToken],
    kind: SyntaxKind,
    ordinal: u32,
    events: &mut Vec<SyntaxEvent>,
) {
    let item_start = events.len();
    super::predicate_proof::emit_declaration(
        source,
        tokens,
        kind,
        SyntaxRole::Element(ordinal),
        events,
    );
    wrap_declaration_logical_lines(source, item_start, events);
}

fn wrap_declaration_logical_lines(source: &str, item_start: usize, events: &mut Vec<SyntaxEvent>) {
    let finish = events.pop().expect("declaration finish event");
    debug_assert_eq!(finish, SyntaxEvent::FinishNode);
    let inner = events.split_off(item_start + 1);
    let mut line_open = false;
    let mut line_ordinal = 0_u32;
    let mut nested_depth = 0_usize;
    let mut delimiter_depth = 0_usize;
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
                if *kind == SyntaxKind::PunctuationToken {
                    delimiter_depth = delimiter_depth_after(source, *kind, *range, delimiter_depth);
                }
                if *kind == SyntaxKind::NewlineToken && delimiter_depth == 0 {
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
) {
    events.push(SyntaxEvent::start(
        SyntaxKind::LogicalLine,
        SyntaxRole::Element(ordinal),
    ));
    let item = classify_top_level_item(source, tokens);
    if let Some(kind) = item {
        events.push(SyntaxEvent::start(kind, SyntaxRole::Element(ordinal)));
    }
    events.extend(
        tokens
            .iter()
            .map(|token| SyntaxEvent::token(token.kind, token.range)),
    );
    if item.is_some() {
        events.push(SyntaxEvent::FinishNode);
    }
    events.push(SyntaxEvent::FinishNode);
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

fn classify_top_level_item(source: &str, tokens: &[LexToken]) -> Option<SyntaxKind> {
    let significant = tokens
        .iter()
        .filter(|token| !is_trivia_kind(token.kind))
        .collect::<Vec<_>>();
    let spellings = significant
        .iter()
        .copied()
        .filter(|token| token.kind == SyntaxKind::KeywordToken)
        .map(|token| &source[token.range.as_range()])
        .collect::<Vec<_>>();
    let first = *significant.first()?;
    let first_text = &source[first.range.as_range()];
    if first_text == "#" {
        return Some(
            significant
                .get(1)
                .copied()
                .filter(|token| &source[token.range.as_range()] == "!")
                .map_or(SyntaxKind::OuterAttribute, |_| SyntaxKind::InnerAttribute),
        );
    }
    if let Some(kind) = declaration_kind(&spellings) {
        if matches!(kind, SyntaxKind::PredicateItem | SyntaxKind::ProofItem)
            && declaration_name_is_entity_reference(source, &significant)
        {
            return Some(SyntaxKind::ErrorItem);
        }
        return Some(kind);
    }
    Some(
        if is_flow_statement_head(first_text)
            || matches!(
                first.kind,
                SyntaxKind::IdentifierToken | SyntaxKind::EntityReferenceToken
            )
        {
            SyntaxKind::TopLevelFlowItem
        } else {
            SyntaxKind::ErrorItem
        },
    )
}

fn declaration_name_is_entity_reference(source: &str, tokens: &[&LexToken]) -> bool {
    tokens
        .iter()
        .position(|token| matches!(&source[token.range.as_range()], "predicate" | "proof"))
        .and_then(|keyword| tokens.get(keyword + 1))
        .is_some_and(|token| token.kind == SyntaxKind::EntityReferenceToken)
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

fn declaration_kind(keywords: &[&str]) -> Option<SyntaxKind> {
    let keyword = keywords
        .iter()
        .copied()
        .find(|keyword| !matches!(*keyword, "pub" | "crate" | "super"))?;
    Some(match keyword {
        "mod" => SyntaxKind::ModuleDeclaration,
        "use" => SyntaxKind::UseDeclaration,
        "flow" => SyntaxKind::FlowItem,
        "fn" => SyntaxKind::FunctionItem,
        "predicate" => SyntaxKind::PredicateItem,
        "proof" => SyntaxKind::ProofItem,
        "agent" => SyntaxKind::AgentItem,
        "callable" => SyntaxKind::CallableItem,
        "state" => SyntaxKind::StateItem,
        "trait" => SyntaxKind::TraitItem,
        "impl" => SyntaxKind::ImplItem,
        "enum" => SyntaxKind::EnumItem,
        "struct" => SyntaxKind::StructItem,
        "type" => SyntaxKind::TypeAliasItem,
        "entity" => SyntaxKind::EntityDeclarationItem,
        "entry" => SyntaxKind::EntryDeclarationItem,
        "extern" if keywords.contains(&"capability") => SyntaxKind::ExternCapabilityItem,
        "extern" if keywords.contains(&"mod") => SyntaxKind::ExternModuleItem,
        "hook" => SyntaxKind::HookItem,
        "dialogue" if keywords.contains(&"defaults") => SyntaxKind::DialogueDefaultsItem,
        "memo" if keywords.contains(&"fn") => SyntaxKind::MemoFunctionItem,
        "test" => SyntaxKind::TestItem,
        "bench" => SyntaxKind::BenchItem,
        "parser" => SyntaxKind::ParserItem,
        "source" => SyntaxKind::SourceItem,
        "style" => SyntaxKind::StyleItem,
        _ => return None,
    })
}

fn is_flow_statement_head(spelling: &str) -> bool {
    matches!(
        spelling,
        "assert"
            | "await"
            | "break"
            | "choice"
            | "close"
            | "continue"
            | "defer"
            | "for"
            | "goto"
            | "if"
            | "let"
            | "loop"
            | "match"
            | "on"
            | "out"
            | "return"
            | "select"
            | "signal"
            | "thread"
            | "unsafe"
            | "wait"
            | "while"
            | "yield"
    )
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
