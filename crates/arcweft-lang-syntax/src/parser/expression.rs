//! Private Pratt expression grammar over the shared document cursor.

mod composite;
mod control;

use super::document::ShadowDocumentParser;
use super::path::emit_path;
use super::rich_text_grammar::emit_dialogue_rich_text;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    find_top_level_boundary, trimmed_end,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CompletedNode {
    pub(super) start_event: usize,
}

pub(super) fn emit_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    let end = trimmed_end(parser, parser.cursor(), end);
    if parser.cursor() >= end {
        parser.start(SyntaxKind::MissingExpression, role);
        parser.finish();
        return;
    }

    let completed = parse_binding_power(parser, end, 0, role);
    if parser.cursor() < end {
        parser.insert_start(completed.start_event, SyntaxKind::ErrorExpression, role);
        parser.set_start_role(completed.start_event + 1, SyntaxRole::Operand);
        bump_until(parser, end);
        parser.finish();
    }
}

/// Emits an expression at a Flow position where bracketed dialogue content is
/// part of the surface grammar rather than an ordinary index expression.
///
/// The decision is made from the already lexed token stream. It never reparses
/// a source substring, and ordinary indexed values continue through the Pratt
/// parser unchanged.
pub(super) fn emit_dialogue_context_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    let end = trimmed_end(parser, parser.cursor(), end);
    let Some(surface) = dialogue_surface(parser, end) else {
        emit_expression(parser, end, role);
        return;
    };

    parse_binding_power_with_dialogue(parser, end, 0, role, Some(surface));
}

/// Emits one owner-provided named plan section through the shared expression
/// block grammar without teaching ordinary expression dispatch owner names.
pub(super) fn emit_named_plan_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    composite::emit_named_block(parser, end, role);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DialogueSurface {
    open: usize,
    close: Option<usize>,
}

fn dialogue_surface(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> Option<DialogueSurface> {
    let start = parser.cursor();
    let first = super::shadow_recovery::first_significant(parser, start, end)?;
    let mut depth = 0_usize;
    let mut saw_call = false;
    let mut open = None;
    for index in first..end {
        let text = super::shadow_recovery::token_text(parser, index)?;
        if depth == 0 && text == "[" {
            open = Some(index);
            break;
        }
        match text {
            "(" if depth == 0 => {
                saw_call = true;
                depth += 1;
            }
            "(" | "{" | "<" => depth += 1,
            ")" | "}" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    let open = open?;
    super::shadow_recovery::first_significant(parser, first, open)?;

    let close = super::shadow_recovery::find_matching_close(parser, open + 1, "[")
        .filter(|close| *close < end);
    if close.is_some_and(|close| {
        super::shadow_recovery::first_significant(parser, close + 1, end).is_some()
    }) {
        return None;
    }

    let content_end = close.unwrap_or(end);
    let first_content = super::shadow_recovery::first_significant(parser, open + 1, content_end);
    let begins_non_ascii_text = first_content
        .and_then(|index| super::shadow_recovery::token_text(parser, index))
        .and_then(|text| text.chars().next())
        .is_some_and(|character| !character.is_ascii());
    let contains_raw_text = (open + 1..content_end).any(|index| {
        parser
            .token_at(index)
            .is_some_and(|token| token.kind() == SyntaxKind::TextToken)
    });

    (saw_call || begins_non_ascii_text || contains_raw_text)
        .then_some(DialogueSurface { open, close })
}

fn emit_dialogue_call(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    surface: DialogueSurface,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::DialogueCallExpression, role);
    emit_expression(parser, surface.open, SyntaxRole::Callee);
    bump_until(parser, surface.open);
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    let content_end = surface.close.unwrap_or(end);
    emit_dialogue_rich_text(parser, content_end);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.expression.missing_dialogue_close",
    );
    bump_until(parser, end);
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn expression_is_call(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> bool {
    let mut depth = 0_usize;
    let mut saw_callee = false;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        let text = parser.text_of(token);
        if token.kind() == SyntaxKind::WhitespaceToken || token.kind() == SyntaxKind::CommentToken {
            continue;
        }
        if !saw_callee {
            saw_callee = matches!(
                token.kind(),
                SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
            );
            if !saw_callee {
                return false;
            }
            continue;
        }
        match text {
            "(" if depth == 0 => return true,
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "." | "?." | "::" => {}
            _ if token.kind() == SyntaxKind::IdentifierToken => {}
            _ => return false,
        }
    }
    false
}

fn parse_binding_power(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    minimum: u8,
    role: SyntaxRole,
) -> CompletedNode {
    parse_binding_power_with_dialogue(parser, end, minimum, role, None)
}

fn parse_binding_power_with_dialogue(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    minimum: u8,
    role: SyntaxRole,
    dialogue: Option<DialogueSurface>,
) -> CompletedNode {
    let mut left = parse_prefix_with_dialogue(parser, end, role, dialogue);

    while let Some((operator_index, _, operator)) = parser.next_significant() {
        if operator_index >= end {
            break;
        }

        if is_postfix_operator(operator) {
            bump_until(parser, operator_index);
            left = emit_postfix(parser, end, left, role, operator);
            continue;
        }

        let Some((left_power, right_power, kind)) = binary_binding_power(operator) else {
            break;
        };
        if left_power < minimum {
            break;
        }

        bump_until(parser, operator_index);
        parser.insert_start(left.start_event, kind, role);
        parser.set_start_role(left.start_event + 1, SyntaxRole::LeftOperand);
        parser.bump();
        parser.bump_trivia();
        if parser.cursor() < end {
            parse_binding_power(parser, end, right_power, SyntaxRole::RightOperand);
        } else {
            parser.start(SyntaxKind::MissingExpression, SyntaxRole::RightOperand);
            parser.finish();
        }
        parser.finish();
        left = CompletedNode {
            start_event: left.start_event,
        };
    }

    left
}

fn parse_prefix_with_dialogue(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    dialogue: Option<DialogueSurface>,
) -> CompletedNode {
    let start_event = parser.event_position();
    let Some(token) = parser.current() else {
        parser.start(SyntaxKind::MissingExpression, role);
        parser.finish();
        return CompletedNode { start_event };
    };
    let text = parser.text_of(token);

    if let Some(surface) = dialogue
        && parser.cursor() < surface.open
        && !matches!(
            text,
            "try" | "await" | "thread" | "if" | "match" | "&" | "*" | "!" | "-" | "+"
        )
    {
        return emit_dialogue_call(parser, end, role, surface);
    }

    match text {
        "&" => emit_prefix_operand(parser, end, SyntaxKind::BorrowExpression, role, true),
        "*" => emit_prefix_operand(parser, end, SyntaxKind::DereferenceExpression, role, false),
        "!" | "-" => emit_prefix_operand(parser, end, SyntaxKind::UnaryExpression, role, false),
        "try" if propagating_await_spelling(parser, end) == Some(PropagatingAwait::TryAwait) => {
            emit_propagating_await(parser, end, role, dialogue, PropagatingAwait::TryAwait)
        }
        "try" => emit_prefix_operand_with_dialogue(
            parser,
            end,
            SyntaxKind::TryExpression,
            role,
            false,
            dialogue,
        ),
        "await"
            if propagating_await_spelling(parser, end) == Some(PropagatingAwait::AwaitQuestion) =>
        {
            emit_propagating_await(parser, end, role, dialogue, PropagatingAwait::AwaitQuestion)
        }
        "await" => emit_prefix_operand_with_dialogue(
            parser,
            end,
            SyntaxKind::AwaitExpression,
            role,
            false,
            dialogue,
        ),
        "thread" if composite::has_braced_body(parser, end) => {
            composite::emit_thread_expression(parser, end, role)
        }
        "result" | "task" | "seq" | "stream" if composite::has_braced_body(parser, end) => {
            composite::emit_computation_block(parser, end, role)
        }
        "scope" if composite::has_braced_body(parser, end) => {
            composite::emit_named_block(parser, end, role)
        }
        "(" => composite::emit_parenthesized(parser, end, role),
        "[" => composite::emit_bracket_sequence(parser, end, role),
        "." => emit_short_variant(parser, end, role),
        "{" => composite::emit_braced_expression(parser, end, role),
        "if" => control::emit_if_expression(parser, end, role),
        "match" => control::emit_match_expression(parser, end, role),
        "|" | "||" => composite::emit_closure(parser, end, role),
        "_" => emit_single(parser, SyntaxKind::PlaceholderExpression, role),
        "true" | "false" => emit_single(parser, SyntaxKind::LiteralExpression, role),
        _ if token.kind() == SyntaxKind::EntityReferenceToken => {
            emit_single(parser, SyntaxKind::EntityReferenceExpression, role)
        }
        _ if token.kind() == SyntaxKind::LifetimeToken => {
            emit_path_like(parser, end, SyntaxKind::LifetimePathExpression, role)
        }
        _ if is_literal(token.kind()) => emit_single(parser, SyntaxKind::LiteralExpression, role),
        _ if matches!(
            token.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
        ) && composite::is_nominal_record_head(parser, end) =>
        {
            composite::emit_record_expression(parser, end, role)
        }
        _ if matches!(
            token.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
        ) =>
        {
            emit_path_like(parser, end, SyntaxKind::PathExpression, role)
        }
        _ => emit_single(parser, SyntaxKind::ErrorExpression, role),
    }
}

fn emit_prefix_operand(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
    accepts_mutability: bool,
) -> CompletedNode {
    emit_prefix_operand_with_dialogue(parser, end, kind, role, accepts_mutability, None)
}

fn emit_prefix_operand_with_dialogue(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
    accepts_mutability: bool,
    dialogue: Option<DialogueSurface>,
) -> CompletedNode {
    let start_event = parser.event_position();
    if !parser.enter_prefix_expression() {
        bump_until(parser, end);
        return CompletedNode { start_event };
    }
    parser.start(kind, role);
    parser.bump();
    parser.bump_trivia();
    if accepts_mutability && parser.at("mut") {
        parser.bump();
        parser.bump_trivia();
    }
    if parser.cursor() < end {
        parse_binding_power_with_dialogue(parser, end, 90, SyntaxRole::Operand, dialogue);
    } else {
        parser.start(SyntaxKind::MissingExpression, SyntaxRole::Operand);
        parser.finish();
    }
    parser.finish();
    parser.leave_prefix_expression();
    CompletedNode { start_event }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PropagatingAwait {
    TryAwait,
    AwaitQuestion,
}

fn propagating_await_spelling(
    parser: &ShadowDocumentParser<'_, '_>,
    end: usize,
) -> Option<PropagatingAwait> {
    match parser.current_text()? {
        "try" => super::shadow_recovery::first_significant(parser, parser.cursor() + 1, end)
            .filter(|index| super::shadow_recovery::token_text(parser, *index) == Some("await"))
            .map(|_| PropagatingAwait::TryAwait),
        "await" => {
            let question = parser.token_at(parser.cursor() + 1)?;
            (parser.text_of(question) == "?"
                && parser.current()?.range().end() == question.range().start())
            .then_some(PropagatingAwait::AwaitQuestion)
        }
        _ => None,
    }
}

fn emit_propagating_await(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    dialogue: Option<DialogueSurface>,
    spelling: PropagatingAwait,
) -> CompletedNode {
    let start_event = parser.event_position();
    if !parser.enter_prefix_expression() {
        bump_until(parser, end);
        return CompletedNode { start_event };
    }
    parser.start(SyntaxKind::AwaitExpression, role);
    parser.bump();
    parser.bump_trivia();
    match spelling {
        PropagatingAwait::TryAwait => debug_assert!(parser.at("await")),
        PropagatingAwait::AwaitQuestion => debug_assert!(parser.at("?")),
    }
    parser.bump();
    parser.bump_trivia();
    if parser.cursor() < end {
        parse_binding_power_with_dialogue(parser, end, 90, SyntaxRole::Operand, dialogue);
    } else {
        parser.start(SyntaxKind::MissingExpression, SyntaxRole::Operand);
        parser.finish();
    }
    parser.finish();
    parser.leave_prefix_expression();
    CompletedNode { start_event }
}

fn emit_short_variant(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::ShortVariantExpression, role);
    parser.bump();
    parser.bump_trivia();
    if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        )
    {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Target);
        parser.bump();
        parser.finish();
    } else {
        parser.start(SyntaxKind::MissingName, SyntaxRole::Target);
        parser.finish();
    }
    parser.finish();
    CompletedNode { start_event }
}

fn emit_path_like(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    expression_kind: SyntaxKind,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(expression_kind, role);
    emit_path(parser, end, SyntaxRole::Target);
    parser.finish();
    CompletedNode { start_event }
}

fn emit_single(
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start(kind, role);
    parser.bump();
    parser.finish();
    CompletedNode { start_event }
}

fn emit_postfix(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
    operator: &str,
) -> CompletedNode {
    match operator {
        "(" => emit_call(parser, end, left, role),
        "[" => emit_index(parser, end, left, role),
        "." | "?." => emit_select(parser, end, left, role),
        "?" => emit_try(parser, left, role),
        _ => left,
    }
}

fn emit_call(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    parser.insert_start(left.start_event, SyntaxKind::CallExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Callee);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.start(SyntaxKind::ArgumentList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end || parser.at(")") {
            break;
        }
        let argument_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(end);
        emit_call_argument(parser, argument_end, ordinal);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.expression.missing_call_close",
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

pub(super) fn emit_call_argument(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    ordinal: u16,
) {
    parser.start(SyntaxKind::CallArgument, SyntaxRole::Argument(ordinal));
    let assignment = find_top_level_boundary(parser, parser.cursor(), &["="]).min(end);
    if assignment < end {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
        bump_until(parser, trimmed_end(parser, parser.cursor(), assignment));
        parser.finish();
        bump_until(parser, assignment);
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, end, SyntaxRole::Operand);
    } else {
        let spread = find_top_level_boundary(parser, parser.cursor(), &["..."]).min(end);
        emit_expression(parser, spread, SyntaxRole::Operand);
        bump_until(parser, spread);
        if parser.at("...") {
            parser.bump();
        }
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_index(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    parser.insert_start(left.start_event, SyntaxKind::IndexExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Target);
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    parser.bump_trivia();
    let index_end = find_top_level_boundary(parser, parser.cursor(), &["]"]).min(end);
    emit_expression(parser, index_end, SyntaxRole::Argument(0));
    bump_until(parser, index_end);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.expression.missing_index_close",
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

fn emit_select(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    parser.insert_start(left.start_event, SyntaxKind::SelectExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Target);
    parser.bump();
    parser.bump_trivia();
    if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        )
    {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Field(0));
        parser.bump();
        parser.finish();
    } else {
        parser.start(SyntaxKind::MissingName, SyntaxRole::Field(0));
        parser.finish();
    }
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

fn emit_try(
    parser: &mut ShadowDocumentParser<'_, '_>,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    parser.insert_start(left.start_event, SyntaxKind::TryExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Operand);
    if parser.at("?") {
        parser.bump();
    } else {
        emit_missing_delimiter(parser, SyntaxKind::MissingTokenNode, SyntaxRole::Token);
    }
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

fn is_postfix_operator(operator: &str) -> bool {
    matches!(operator, "(" | "[" | "." | "?." | "?")
}

fn binary_binding_power(operator: &str) -> Option<(u8, u8, SyntaxKind)> {
    let (power, kind) = match operator {
        "|>" => (1, SyntaxKind::PipeExpression),
        "||" | "??" => (3, SyntaxKind::BinaryExpression),
        "&&" => (5, SyntaxKind::BinaryExpression),
        "==" | "!=" => (7, SyntaxKind::BinaryExpression),
        "<" | "<=" | ">" | ">=" | "in" => (9, SyntaxKind::BinaryExpression),
        ".." | "..=" => (11, SyntaxKind::RangeExpression),
        "+" | "-" => (13, SyntaxKind::BinaryExpression),
        "*" | "/" | "%" => (15, SyntaxKind::BinaryExpression),
        _ => return None,
    };
    Some((power, power + 1, kind))
}

const fn is_literal(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::NumberToken
            | SyntaxKind::StringToken
            | SyntaxKind::RawStringToken
            | SyntaxKind::CharacterToken
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::budget::GrammarBudget;
    use crate::grammar::event::SyntaxEvent;
    use crate::parser::lexer::DocumentLexer;

    #[test]
    fn selected_postfix_try_slot_recovers_a_missing_operator_at_operand_end() {
        let source = "value";
        let tokens = DocumentLexer::new(source).lex();
        let mut events = Vec::new();
        let mut budget = GrammarBudget::default();
        {
            let mut parser = ShadowDocumentParser::new(source, &tokens, &mut events, &mut budget);

            let operand =
                parse_prefix_with_dialogue(&mut parser, tokens.len(), SyntaxRole::Element(0), None);
            assert!(parser.is_at_end());
            emit_try(&mut parser, operand, SyntaxRole::Element(0));
        }

        assert!(events.iter().any(|event| matches!(
            event,
            SyntaxEvent::StartNode {
                kind: SyntaxKind::MissingTokenNode,
                role: SyntaxRole::Token,
            }
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            SyntaxEvent::MissingToken { at, .. } if *at == source.len()
        )));
    }
}
