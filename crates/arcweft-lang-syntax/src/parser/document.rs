//! One-pass lexer and root event stream for the staged document grammar.

#![allow(
    dead_code,
    reason = "the shadow document parser remains private until the atomic syntax switch"
)]

use arcweft_source::{SourceDocument, SourceRange};

use crate::grammar::budget::GrammarBudget;
use crate::grammar::build::{GrammarBuild, GrammarBuildError, build_grammar_text};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

use super::cursor::{ShadowDocumentParser, is_trivia_kind};
use super::expression::emit_expression;
use super::fragment::ParseOptions;
use super::item::{classify_top_level_item, is_declaration_item_kind};
use super::lexer::{DocumentLexer, LexToken};
use super::pattern::emit_pattern;
use super::statement::emit_statement_fragment;
use super::type_ref::emit_type;

/// Builds the private lossless root tree without allocating syntax identity.
pub(crate) fn parse_shadow_document(
    document: &SourceDocument,
    options: ParseOptions,
) -> Result<GrammarBuild, GrammarBuildError> {
    // The accepted option type is currently fieldless. Destructuring it at
    // the canonical grammar entry makes a future option an explicit parser
    // migration instead of letting the transaction silently discard it.
    let ParseOptions {} = options;
    let tokens = DocumentLexer::new(document.text()).lex();
    build_shadow_root(document, &tokens, |tokens, events, budget| {
        start_event(events, budget, SyntaxKind::ItemList, SyntaxRole::Element(0));
        emit_logical_lines(document.text(), tokens, events, budget)?;
        finish_event(events, budget);
        Ok(())
    })
}

/// Grammar family accepted by standalone fragment parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FragmentGrammar {
    Expression,
    Type,
    Pattern,
    Statement,
}

/// Parses one source-free fragment through the shared lexer and grammar.
pub(super) fn parse_unbound_fragment(
    source: &str,
    grammar: FragmentGrammar,
) -> Result<GrammarBuild, GrammarBuildError> {
    let tokens = DocumentLexer::new(source).lex();
    build_shadow_root_text(source, &tokens, |tokens, events, budget| {
        let mut parser = ShadowDocumentParser::for_fragment(source, tokens, 0, events, budget);
        parser.bump_trivia();
        match grammar {
            FragmentGrammar::Expression => {
                emit_expression(&mut parser, tokens.len(), SyntaxRole::Element(0));
            }
            FragmentGrammar::Type => {
                emit_type(&mut parser, tokens.len(), SyntaxRole::Element(0));
            }
            FragmentGrammar::Pattern => {
                emit_pattern(&mut parser, tokens.len(), SyntaxRole::Element(0));
            }
            FragmentGrammar::Statement => {
                emit_statement_fragment(&mut parser, tokens.len(), SyntaxRole::Element(0));
            }
        }
        while parser.bump().is_some() {}
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
    build_shadow_root_text(document.text(), tokens, emit_body)
}

fn build_shadow_root_text(
    source: &str,
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
            SourceRange::new(source.len(), source.len()),
        ),
    );
    finish_event(&mut events, &mut budget);
    budget_failure(&budget)?;
    build_grammar_text(source, &events, tokens.len())
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
    let mut root_state = SourceRootState::default();
    while line < lines.len() {
        if let Some((declaration_line, kind)) =
            structured_declaration_after_outer_prefixes(source, tokens, &lines, line)
        {
            let last = declaration_group_end(source, tokens, &lines, declaration_line, kind);
            let grouped = &tokens[lines[line].start..lines[last].end];
            let item = root_state.classify(kind)?;
            debug_assert!(item.recovery.is_none());
            emit_declaration_item(source, grouped, item.kind, item.role, events, budget)?;
            line = last + 1;
        } else {
            let range = lines[line];
            let line_tokens = &tokens[range.start..range.end];
            let item = classify_top_level_item(source, line_tokens);
            let item = item.map(|kind| root_state.classify(kind)).transpose()?;
            emit_logical_line(source, line_tokens, item, ordinal, events, budget)?;
            line += 1;
        }
        ordinal = ordinal
            .checked_add(1)
            .ok_or(GrammarBuildError::ChildIndexExhausted)?;
    }
    Ok(())
}

#[derive(Default)]
struct SourceRootState {
    attributes: u16,
    uses: u16,
    items: u32,
    phase: SourceRootPhase,
    module_seen: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SourceRootPhase {
    #[default]
    Header,
    Uses,
    Items,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceRootRecovery {
    DuplicateModule,
    LateModule,
    LateUse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceRootItem {
    kind: SyntaxKind,
    role: SyntaxRole,
    recovery: Option<SourceRootRecovery>,
}

impl SourceRootState {
    fn classify(&mut self, kind: SyntaxKind) -> Result<SourceRootItem, GrammarBuildError> {
        let (kind, role, recovery) = match kind {
            SyntaxKind::InnerAttribute | SyntaxKind::OuterAttribute => {
                let ordinal = self.attributes;
                self.attributes = ordinal
                    .checked_add(1)
                    .ok_or(GrammarBuildError::ChildIndexExhausted)?;
                (kind, SyntaxRole::Attribute(ordinal), None)
            }
            SyntaxKind::ModuleDeclaration => {
                let recovery = if self.module_seen {
                    Some(SourceRootRecovery::DuplicateModule)
                } else if self.phase != SourceRootPhase::Header {
                    Some(SourceRootRecovery::LateModule)
                } else {
                    None
                };
                self.module_seen = true;
                if let Some(recovery) = recovery {
                    self.phase = SourceRootPhase::Items;
                    (
                        SyntaxKind::ErrorItem,
                        self.next_item_role()?,
                        Some(recovery),
                    )
                } else {
                    (kind, SyntaxRole::Target, None)
                }
            }
            SyntaxKind::UseDeclaration => {
                if self.phase == SourceRootPhase::Items {
                    (
                        SyntaxKind::ErrorItem,
                        self.next_item_role()?,
                        Some(SourceRootRecovery::LateUse),
                    )
                } else {
                    self.phase = SourceRootPhase::Uses;
                    let ordinal = self.uses;
                    self.uses = ordinal
                        .checked_add(1)
                        .ok_or(GrammarBuildError::ChildIndexExhausted)?;
                    (kind, SyntaxRole::Reference(ordinal), None)
                }
            }
            _ => {
                self.phase = SourceRootPhase::Items;
                (kind, self.next_item_role()?, None)
            }
        };
        Ok(SourceRootItem {
            kind,
            role,
            recovery,
        })
    }

    fn next_item_role(&mut self) -> Result<SyntaxRole, GrammarBuildError> {
        let ordinal = self.items;
        self.items = ordinal
            .checked_add(1)
            .ok_or(GrammarBuildError::ChildIndexExhausted)?;
        Ok(SyntaxRole::Element(ordinal))
    }
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
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) -> Result<(), GrammarBuildError> {
    let item_start = events.len();
    match kind {
        SyntaxKind::FlowItem => {
            super::shadow_flow::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::FunctionItem => {
            super::function_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::PredicateItem | SyntaxKind::ProofItem => {
            super::predicate_proof::emit_declaration(source, tokens, kind, role, events, budget);
        }
        SyntaxKind::EnumItem | SyntaxKind::StructItem | SyntaxKind::TypeAliasItem => {
            super::type_declaration_grammar::emit_declaration(
                source, tokens, kind, role, events, budget,
            );
        }
        SyntaxKind::TraitItem | SyntaxKind::ImplItem => {
            super::trait_impl_grammar::emit_declaration(source, tokens, kind, role, events, budget);
        }
        SyntaxKind::ResourceDeclarationItem => {
            super::resource_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::CharacterDeclarationItem
        | SyntaxKind::ViewDeclarationItem
        | SyntaxKind::ActionDeclarationItem
        | SyntaxKind::ActivityDeclarationItem
        | SyntaxKind::SignalDeclarationItem
        | SyntaxKind::MetricDeclarationItem
        | SyntaxKind::LayerDeclarationItem => {
            emit_retained_declaration_item(source, tokens, kind, role, events, budget);
        }
        SyntaxKind::EntryDeclarationItem => {
            super::entry_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::ExternCapabilityItem => {
            super::extern_capability_grammar::emit_declaration(
                source, tokens, role, events, budget,
            );
        }
        SyntaxKind::TestItem | SyntaxKind::BenchItem => {
            super::test_bench_grammar::emit_declaration(source, tokens, kind, role, events, budget);
        }
        SyntaxKind::SourceItem => {
            super::source_grammar::emit_declaration(source, tokens, role, events, budget);
        }
        SyntaxKind::StyleItem => {
            super::style_grammar::emit_declaration(source, tokens, role, events, budget);
        }
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
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
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
    item: Option<SourceRootItem>,
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
    match item {
        Some(SourceRootItem {
            kind: kind @ (SyntaxKind::ModuleDeclaration | SyntaxKind::UseDeclaration),
            role,
            recovery: None,
        }) => {
            super::module_use_grammar::emit_declaration(source, tokens, kind, role, events, budget);
        }
        Some(SourceRootItem {
            kind: SyntaxKind::FlowItem,
            role,
            recovery: None,
        }) => {
            super::shadow_flow::emit_declaration(source, tokens, role, events, budget);
        }
        Some(SourceRootItem {
            kind,
            role,
            recovery,
        }) => {
            start_event(events, budget, kind, role);
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
                let (code, message) = match recovery {
                    Some(SourceRootRecovery::DuplicateModule) => (
                        "syntax.source.duplicate_module_declaration",
                        "a source file accepts at most one leading module declaration",
                    ),
                    Some(SourceRootRecovery::LateModule) => (
                        "syntax.source.late_module_declaration",
                        "a module declaration must precede use declarations and ordinary items",
                    ),
                    Some(SourceRootRecovery::LateUse) => (
                        "syntax.source.late_use_declaration",
                        "a use declaration must precede ordinary items",
                    ),
                    None => (
                        "syntax.item.expected_declaration",
                        "regular Arcweft source accepts declarations at the top level",
                    ),
                };
                push_event(
                    events,
                    budget,
                    SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                        code,
                        SourceRange::new(first.range.start(), last.range.end()),
                        message,
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

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
