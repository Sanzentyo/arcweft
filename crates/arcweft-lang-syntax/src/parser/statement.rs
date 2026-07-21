//! Private predicate/proof block and statement grammar over the shared cursor.

use arcweft_source::SourceRange;

use super::document::ShadowDocumentParser;
use super::expression::{emit_dialogue_context_expression, emit_expression, expression_is_call};
use super::pattern::emit_pattern;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    find_matching_close, find_statement_terminator, find_top_level_boundary, first_significant,
    token_count, token_text, trimmed_end,
};
use super::type_ref::emit_type;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_block_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    item_kind: SyntaxKind,
    body_kind: SyntaxKind,
    keyword: &str,
) {
    let block_kind = if item_kind == SyntaxKind::PredicateItem {
        SyntaxKind::PredicateBlock
    } else {
        SyntaxKind::ProofBlock
    };
    parser.start(body_kind, SyntaxRole::Body);
    emit_braced_block(
        parser,
        item_kind,
        block_kind,
        SyntaxRole::Body,
        if keyword == "predicate" {
            "syntax.predicate.missing_block_close"
        } else {
            "syntax.proof.missing_block_close"
        },
    );
    parser.finish();
}

pub(super) fn emit_braced_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    item_kind: SyntaxKind,
    block_kind: SyntaxKind,
    role: SyntaxRole,
    missing_close_code: &'static str,
) {
    parser.start(block_kind, role);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(token_count(parser));
    parser.start(SyntaxKind::StatementList, SyntaxRole::Element(0));
    let mut statement = 0_u32;
    let mut has_tail = false;

    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let start = parser.cursor();
        let terminator = find_statement_terminator(parser, start, close);
        let segment_end = terminator.map_or(close, |(index, _)| index);
        let significant_end = trimmed_end(parser, start, segment_end);
        let first = first_significant(parser, start, significant_end)
            .and_then(|index| token_text(parser, index));
        let semicolon = terminator.is_some_and(|(_, semicolon)| semicolon);
        let later = terminator
            .is_some_and(|(index, _)| first_significant(parser, index + 1, close).is_some());
        let unterminated_value_head = !semicolon
            && !later
            && first.is_some_and(|spelling| matches!(spelling, "if" | "loop" | "match" | "thread"));
        let statement_shaped = first.is_some_and(is_statement_head) && !unterminated_value_head;
        if semicolon || later || statement_shaped {
            let end = if semicolon {
                terminator.map_or(segment_end, |(index, _)| index + 1)
            } else {
                significant_end
            };
            emit_statement(parser, end, item_kind, statement);
            statement = statement.saturating_add(1);
            bump_until(parser, segment_end);
            continue;
        }

        parser.finish();
        emit_item_expression(parser, significant_end, SyntaxRole::Tail, item_kind);
        bump_until(parser, close);
        has_tail = true;
        break;
    }

    if !has_tail {
        parser.finish();
        parser.start(SyntaxKind::OmittedBlockTail, SyntaxRole::Tail);
        parser.finish();
    }

    if parser.cursor() == close && parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.block.missing_close",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            missing_close_code,
            SourceRange::new(at, at),
            "missing closing `}` for block",
        )));
    }
    parser.finish();
}

fn emit_statement(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    emit_statement_with_role(parser, end, item_kind, SyntaxRole::Statement(ordinal));
}

/// Emits one ordinary statement fragment without inventing a declaration
/// owner. Proof/predicate restrictions remain owned by their document item.
pub(super) fn emit_statement_fragment(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    let end = trimmed_end(parser, parser.cursor(), end);
    emit_statement_with_role(parser, end, SyntaxKind::FunctionItem, role);
}

fn emit_statement_with_role(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    role: SyntaxRole,
) {
    let child_end = if end > parser.cursor() && token_text(parser, end - 1) == Some(";") {
        end - 1
    } else {
        end
    };
    let kind = classify_statement(parser, child_end, item_kind);
    parser.start(kind, role);
    match kind {
        SyntaxKind::LetStatement
        | SyntaxKind::LetElseStatement
        | SyntaxKind::LetChoiceStatement
        | SyntaxKind::LetScopeStatement
        | SyntaxKind::LetLoopStatement
        | SyntaxKind::LetAwaitStatement
        | SyntaxKind::LetActionReceiveStatement => {
            emit_let_children(parser, child_end, kind, item_kind);
        }
        SyntaxKind::AssertionStatement => emit_assertion_children(parser, child_end),
        SyntaxKind::AssignmentStatement | SyntaxKind::LifetimeSetStatement => {
            emit_assignment_children(parser, child_end, item_kind);
        }
        SyntaxKind::WaitStatement => emit_wait_children(parser, child_end),
        SyntaxKind::OnStatement => emit_on_children(parser, child_end, item_kind),
        SyntaxKind::ThreadStatement
        | SyntaxKind::DeferBlockStatement
        | SyntaxKind::UnsafeLifetimeStatement
        | SyntaxKind::IfStatement
        | SyntaxKind::LoopStatement
        | SyntaxKind::WhileStatement
        | SyntaxKind::WhileLetStatement
        | SyntaxKind::ForStatement
        | SyntaxKind::MatchStatement => {
            emit_control_children(parser, child_end, item_kind, kind);
        }
        SyntaxKind::ReturnStatement
        | SyntaxKind::OutStatement
        | SyntaxKind::GotoStatement
        | SyntaxKind::DeferStatement
        | SyntaxKind::YieldStatement
        | SyntaxKind::SignalStatement
        | SyntaxKind::CloseStatement
        | SyntaxKind::SelectStatement
        | SyntaxKind::BreakStatement
        | SyntaxKind::ContinueStatement => emit_keyword_value(parser, child_end, item_kind),
        SyntaxKind::ProofCallStatement => {
            emit_expression(parser, child_end, SyntaxRole::Callee);
        }
        SyntaxKind::ExpressionStatement => {
            emit_item_expression(parser, child_end, SyntaxRole::Operand, item_kind);
        }
        _ => bump_until(parser, child_end),
    }
    bump_until(parser, end);
    parser.finish();
}

fn classify_statement(
    parser: &ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) -> SyntaxKind {
    let start = parser.cursor();
    if matches!(item_kind, SyntaxKind::PredicateItem | SyntaxKind::ProofItem) {
        return match parser.current_text() {
            Some("let") => SyntaxKind::LetStatement,
            Some("assert") => SyntaxKind::AssertionStatement,
            _ if item_kind == SyntaxKind::ProofItem && expression_is_call(parser, start, end) => {
                SyntaxKind::ProofCallStatement
            }
            _ => SyntaxKind::ErrorStatement,
        };
    }
    match parser.current_text() {
        Some("assert") => SyntaxKind::AssertionStatement,
        Some("let") => classify_let_statement(parser, end),
        Some("return") => SyntaxKind::ReturnStatement,
        Some("out") => SyntaxKind::OutStatement,
        Some("goto") => SyntaxKind::GotoStatement,
        Some("thread") => SyntaxKind::ThreadStatement,
        Some("defer") if find_statement_open_brace(parser, start, end).is_some() => {
            SyntaxKind::DeferBlockStatement
        }
        Some("defer") => SyntaxKind::DeferStatement,
        Some("yield") => SyntaxKind::YieldStatement,
        Some("signal") => SyntaxKind::SignalStatement,
        Some("wait") => SyntaxKind::WaitStatement,
        Some("on") => SyntaxKind::OnStatement,
        Some("unsafe") => SyntaxKind::UnsafeLifetimeStatement,
        Some("if") => SyntaxKind::IfStatement,
        Some("loop") => SyntaxKind::LoopStatement,
        Some("while") if next_significant_text(parser, start + 1, end) == Some("let") => {
            SyntaxKind::WhileLetStatement
        }
        Some("while") => SyntaxKind::WhileStatement,
        Some("for") => SyntaxKind::ForStatement,
        Some("match") => SyntaxKind::MatchStatement,
        Some("close") => SyntaxKind::CloseStatement,
        Some("select") => SyntaxKind::SelectStatement,
        Some("break") => SyntaxKind::BreakStatement,
        Some("continue") => SyntaxKind::ContinueStatement,
        _ if top_level_operator(parser, start, end, "<-").is_some() => {
            SyntaxKind::LifetimeSetStatement
        }
        _ if top_level_operator(parser, start, end, "=").is_some() => {
            SyntaxKind::AssignmentStatement
        }
        _ if item_kind == SyntaxKind::ProofItem && expression_is_call(parser, start, end) => {
            SyntaxKind::ProofCallStatement
        }
        _ if expression_statement_start(parser) => SyntaxKind::ExpressionStatement,
        _ => SyntaxKind::ErrorStatement,
    }
}

#[cfg(test)]
pub(super) fn parse_test_statement_block(
    document: &arcweft_source::SourceDocument,
) -> Result<crate::grammar::build::GrammarBuild, crate::grammar::build::GrammarBuildError> {
    let tokens = super::lexer::DocumentLexer::new(document.text()).lex();
    let mut events = Vec::with_capacity(tokens.len() + 8);
    let mut budget = crate::grammar::budget::GrammarBudget::default();
    assert!(budget.start(SyntaxKind::SourceFile, SyntaxRole::Root));
    events.push(SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root));
    {
        let mut parser =
            ShadowDocumentParser::new(document.text(), &tokens, &mut events, &mut budget);
        emit_braced_block(
            &mut parser,
            SyntaxKind::FunctionItem,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.statement.missing_block_close",
        );
        while parser.bump().is_some() {}
    }
    let eof = SyntaxEvent::token(
        SyntaxKind::EofToken,
        SourceRange::new(document.text().len(), document.text().len()),
    );
    assert!(budget.event(&eof));
    events.push(eof);
    assert!(budget.finish());
    events.push(SyntaxEvent::FinishNode);
    crate::grammar::build::build_grammar(document, &events)
}

fn classify_let_statement(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> SyntaxKind {
    let Some(equals) = top_level_operator(parser, parser.cursor(), end, "=") else {
        return SyntaxKind::LetStatement;
    };
    let initializer =
        first_significant(parser, equals + 1, end).and_then(|index| token_text(parser, index));
    if initializer != Some("if") && top_level_operator(parser, equals + 1, end, "else").is_some() {
        return SyntaxKind::LetElseStatement;
    }
    match initializer {
        Some("choice") => SyntaxKind::LetChoiceStatement,
        Some("scope") => SyntaxKind::LetScopeStatement,
        Some("loop") => SyntaxKind::LetLoopStatement,
        Some("await" | "try") => SyntaxKind::LetAwaitStatement,
        Some("receive") => SyntaxKind::LetActionReceiveStatement,
        _ => SyntaxKind::LetStatement,
    }
}

fn emit_let_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    item_kind: SyntaxKind,
) {
    parser.bump();
    parser.bump_trivia();
    let equals = find_top_level_boundary(parser, parser.cursor(), &["="]).min(end);
    let colon = find_top_level_boundary(parser, parser.cursor(), &[":"]).min(equals);
    let pattern_end = colon.min(equals);
    emit_pattern(parser, pattern_end, SyntaxRole::Pattern);
    bump_until(parser, pattern_end);
    if colon < equals && parser.cursor() == colon {
        parser.bump();
        parser.bump_trivia();
        emit_type(parser, equals, SyntaxRole::Type);
        bump_until(parser, equals);
    }
    if equals < end && parser.cursor() == equals {
        parser.bump();
        parser.bump_trivia();
        let initializer_end = if kind == SyntaxKind::LetElseStatement {
            top_level_operator(parser, parser.cursor(), end, "else").unwrap_or(end)
        } else {
            end
        };
        emit_item_expression(parser, initializer_end, SyntaxRole::Initializer, item_kind);
        bump_until(parser, initializer_end);
        if parser.at("else") {
            parser.bump();
            parser.bump_trivia();
            if parser.at("{") {
                emit_braced_block(
                    parser,
                    item_kind,
                    SyntaxKind::Block,
                    SyntaxRole::ElseBranch,
                    "syntax.statement.missing_let_else_close",
                );
            }
        }
    }
}

fn emit_assertion_children(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let open = find_top_level_boundary(parser, parser.cursor(), &["("]).min(end);
    bump_until(parser, open.saturating_add(1).min(end));
    let close = find_matching_close(parser, parser.cursor(), "(")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let condition_end =
            find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(close);
        parser.charge_assertion_condition();
        emit_expression(parser, condition_end, SyntaxRole::Condition);
        bump_until(parser, condition_end);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    bump_until(parser, end);
}

fn emit_assignment_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) {
    let operator = top_level_operator(parser, parser.cursor(), end, "<-")
        .or_else(|| top_level_operator(parser, parser.cursor(), end, "="))
        .unwrap_or(end);
    emit_expression(parser, operator, SyntaxRole::Target);
    bump_until(parser, operator);
    if parser.cursor() < end {
        parser.bump();
        parser.bump_trivia();
        emit_item_expression(parser, end, SyntaxRole::Initializer, item_kind);
    }
}

fn emit_keyword_value(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) {
    parser.bump();
    parser.bump_trivia();
    if parser.current_kind() == Some(SyntaxKind::LifetimeToken) {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Target);
        parser.bump();
        parser.finish();
        parser.bump_trivia();
    }
    if parser.cursor() < end {
        emit_item_expression(parser, end, SyntaxRole::Operand, item_kind);
    }
}

fn emit_wait_children(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.bump();
    parser.bump_trivia();
    if parser.at("(") {
        emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
        let close = find_matching_close(parser, parser.cursor(), "(")
            .unwrap_or(end)
            .min(end);
        emit_expression(parser, close, SyntaxRole::Operand);
        bump_until(parser, close);
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseParenNode,
            ")",
            "syntax.statement.missing_wait_close",
        );
    } else if parser.cursor() < end {
        emit_expression(parser, end, SyntaxRole::Operand);
    }
}

fn emit_on_children(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, item_kind: SyntaxKind) {
    parser.bump();
    parser.bump_trivia();
    let arrow = top_level_operator(parser, parser.cursor(), end, "=>").unwrap_or(end);
    emit_expression(parser, arrow, SyntaxRole::Condition);
    bump_until(parser, arrow);
    if parser.cursor() < end {
        parser.bump();
        parser.bump_trivia();
        emit_statement(parser, end, item_kind, 0);
    }
}

fn emit_control_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    kind: SyntaxKind,
) {
    let Some(open) = find_statement_open_brace(parser, parser.cursor(), end) else {
        bump_until(parser, end);
        return;
    };

    match kind {
        SyntaxKind::IfStatement => {
            emit_if_children(parser, open, end, item_kind);
            return;
        }
        SyntaxKind::WhileLetStatement => {
            emit_pattern_condition_head(parser, open, "while");
        }
        SyntaxKind::WhileStatement => emit_expression_head(parser, open, "while"),
        SyntaxKind::ForStatement => emit_for_head(parser, open),
        SyntaxKind::MatchStatement => {
            emit_expression_head(parser, open, "match");
            bump_until(parser, open);
            emit_match_block(parser, end, item_kind);
            return;
        }
        _ => bump_until(parser, open),
    }
    bump_until(parser, open);
    emit_braced_block(
        parser,
        item_kind,
        SyntaxKind::Block,
        SyntaxRole::Body,
        "syntax.statement.missing_block_close",
    );
}

fn emit_if_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    open: usize,
    end: usize,
    item_kind: SyntaxKind,
) {
    if next_significant_text(parser, parser.cursor() + 1, open) == Some("let") {
        emit_pattern_condition_head(parser, open, "if");
    } else {
        emit_expression_head(parser, open, "if");
    }
    bump_until(parser, open);
    emit_braced_block(
        parser,
        item_kind,
        SyntaxKind::Block,
        SyntaxRole::ThenBranch,
        "syntax.statement.missing_if_close",
    );
    parser.bump_trivia();
    if parser.cursor() >= end || !parser.at("else") {
        return;
    }
    parser.bump();
    parser.bump_trivia();
    if parser.at("if") {
        parser.start(SyntaxKind::IfStatement, SyntaxRole::ElseBranch);
        if let Some(nested_open) = find_statement_open_brace(parser, parser.cursor(), end) {
            emit_if_children(parser, nested_open, end, item_kind);
        } else {
            bump_until(parser, end);
        }
        parser.finish();
    } else if parser.at("{") {
        emit_braced_block(
            parser,
            item_kind,
            SyntaxKind::Block,
            SyntaxRole::ElseBranch,
            "syntax.statement.missing_else_close",
        );
    } else {
        bump_until(parser, end);
    }
}

fn emit_expression_head(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, keyword: &str) {
    debug_assert!(parser.at(keyword));
    parser.bump();
    parser.bump_trivia();
    emit_expression(parser, end, SyntaxRole::Condition);
}

fn emit_pattern_condition_head(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    keyword: &str,
) {
    debug_assert!(parser.at(keyword));
    parser.bump();
    parser.bump_trivia();
    if parser.at("let") {
        parser.bump();
        parser.bump_trivia();
    }
    let equals = top_level_operator(parser, parser.cursor(), end, "=").unwrap_or(end);
    emit_pattern(parser, equals, SyntaxRole::Pattern);
    bump_until(parser, equals);
    if parser.cursor() >= end {
        return;
    }
    parser.bump();
    parser.bump_trivia();
    let guard = top_level_operator(parser, parser.cursor(), end, "when").unwrap_or(end);
    emit_expression(parser, guard, SyntaxRole::Scrutinee);
    bump_until(parser, guard);
    if parser.at("when") {
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, end, SyntaxRole::Guard);
    }
}

fn emit_for_head(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.bump();
    parser.bump_trivia();
    let separator = top_level_operator(parser, parser.cursor(), end, "in").unwrap_or(end);
    emit_pattern(parser, separator, SyntaxRole::Pattern);
    bump_until(parser, separator);
    if parser.cursor() < end {
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, end, SyntaxRole::Scrutinee);
    }
}

fn emit_match_block(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, item_kind: SyntaxKind) {
    parser.start(SyntaxKind::Block, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::MatchArmList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let arm_end = find_match_arm_end(parser, parser.cursor(), close);
        emit_match_arm(parser, arm_end, item_kind, ordinal);
        bump_until(parser, arm_end);
        ordinal = ordinal.saturating_add(1);
        if matches!(parser.current_text(), Some("," | ";")) {
            parser.bump();
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.statement.missing_match_close",
    );
    parser.finish();
}

fn emit_match_arm(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u16,
) {
    parser.start(SyntaxKind::MatchArm, SyntaxRole::MatchArm(ordinal));
    let arrow = top_level_operator(parser, parser.cursor(), end, "=>").unwrap_or(end);
    let guard = top_level_operator(parser, parser.cursor(), arrow, "when").unwrap_or(arrow);
    emit_pattern(parser, guard, SyntaxRole::Pattern);
    bump_until(parser, guard);
    if parser.at("when") {
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, arrow, SyntaxRole::Guard);
        bump_until(parser, arrow);
    }
    if parser.at("=>") {
        parser.bump();
        parser.bump_trivia();
        if parser.at("{") {
            emit_braced_block(
                parser,
                item_kind,
                SyntaxKind::Block,
                SyntaxRole::Body,
                "syntax.statement.missing_match_arm_close",
            );
        } else {
            emit_item_expression(parser, end, SyntaxRole::Body, item_kind);
        }
    }
    parser.finish();
}

fn emit_item_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    item_kind: SyntaxKind,
) {
    if item_kind == SyntaxKind::FlowItem {
        emit_dialogue_context_expression(parser, end, role);
    } else {
        emit_expression(parser, end, role);
    }
}

fn find_match_arm_end(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> usize {
    let mut depth = 0_usize;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return end;
        };
        let text = parser.text_of(token);
        if depth == 0 && (matches!(text, "," | ";") || token.kind() == SyntaxKind::NewlineToken) {
            return index;
        }
        match text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    end
}

fn top_level_operator(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    spelling: &str,
) -> Option<usize> {
    let mut depth = 0_usize;
    for index in start..end {
        let token = parser.token_at(index)?;
        let text = parser.text_of(token);
        if depth == 0 && text == spelling {
            return Some(index);
        }
        match text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn next_significant_text<'a>(
    parser: &'a ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<&'a str> {
    first_significant(parser, start, end).and_then(|index| token_text(parser, index))
}

fn find_statement_open_brace(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    let mut paren = 0_usize;
    let mut bracket = 0_usize;
    for index in start..end {
        let text = token_text(parser, index)?;
        if text == "{" && paren == 0 && bracket == 0 {
            return Some(index);
        }
        match text {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => bracket += 1,
            "]" => bracket = bracket.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn is_statement_head(spelling: &str) -> bool {
    matches!(
        spelling,
        "assert"
            | "break"
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

fn expression_statement_start(parser: &ShadowDocumentParser<'_, '_>) -> bool {
    matches!(
        parser.current_kind(),
        Some(
            SyntaxKind::IdentifierToken
                | SyntaxKind::KeywordToken
                | SyntaxKind::LifetimeToken
                | SyntaxKind::NumberToken
                | SyntaxKind::StringToken
                | SyntaxKind::RawStringToken
                | SyntaxKind::CharacterToken
                | SyntaxKind::EntityReferenceToken
        )
    ) || matches!(
        parser.current_text(),
        Some("(" | "[" | "{" | "." | "_" | "&" | "*" | "!" | "-" | "+")
    )
}
