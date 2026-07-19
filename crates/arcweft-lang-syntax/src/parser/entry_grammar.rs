//! Private source-entry grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::declaration::{emit_outer_prefixes, emit_visibility};
use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::path::emit_path;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, find_top_level_boundary, first_significant, token_count, token_text,
    trimmed_end,
};
use super::type_ref::emit_type;
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntryRole {
    State,
    Initializer,
    Event,
    Reducer,
    Controller,
}

impl EntryRole {
    const fn from_spelling(spelling: &str) -> Option<Self> {
        match spelling.as_bytes() {
            b"state" => Some(Self::State),
            b"initializer" => Some(Self::Initializer),
            b"event" => Some(Self::Event),
            b"reducer" => Some(Self::Reducer),
            b"controller" => Some(Self::Controller),
            _ => None,
        }
    }

    const fn expects_type(self) -> bool {
        matches!(self, Self::State | Self::Event)
    }
}

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = ShadowDocumentParser::new(source, tokens, events, budget);
    parser.start(SyntaxKind::EntryDeclarationItem, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();

    if parser.at("entry") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_entry_kind(&mut parser);
    parser.bump_trivia();
    emit_entry_id(&mut parser);
    parser.bump_trivia();
    recover_header_tail(&mut parser);
    emit_entry_body(&mut parser, source);

    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_entry_kind(parser: &mut ShadowDocumentParser<'_, '_>) {
    if matches!(
        parser.current_kind(),
        Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
    ) {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Type);
        parser.bump();
        parser.finish();
        return;
    }

    let at = parser.current_offset();
    let range = parser
        .current()
        .filter(|token| token.kind() == SyntaxKind::EntityReferenceToken)
        .map_or(SourceRange::new(at, at), LexToken::range);
    parser.start(SyntaxKind::MissingName, SyntaxRole::Type);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.entry.missing_kind",
        range,
        "entry declaration requires an explicit entry kind",
    )));
}

fn emit_entry_id(parser: &mut ShadowDocumentParser<'_, '_>) {
    let Some(token) = parser
        .current()
        .filter(|token| token.kind() == SyntaxKind::EntityReferenceToken)
    else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::EntityReferenceToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.missing_id",
            SourceRange::new(at, at),
            "entry declaration requires an explicit canonical `@entry.*` ID",
        )));
        return;
    };

    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    let valid_family = parser
        .text_of(token)
        .strip_prefix("@entry.")
        .is_some_and(|suffix| !suffix.is_empty());
    parser.bump();
    parser.finish();
    if !valid_family {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.id_family",
            token.range(),
            "entry declaration IDs must use the `entry` family",
        )));
    }
}

fn recover_header_tail(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("{") || parser.is_at_end() {
        return;
    }

    let start = parser.current_offset();
    let end = find_top_level_boundary(parser, parser.cursor(), &["{"]);
    let end = trimmed_end(parser, parser.cursor(), end);
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.entry.trailing_head",
        SourceRange::new(start, parser.current_offset()),
        "unexpected text after the entry ID",
    )));
    parser.bump_trivia();
}

fn emit_entry_body(parser: &mut ShadowDocumentParser<'_, '_>, source: &str) {
    if !parser.at("{") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.missing_body",
            SourceRange::new(at, at),
            "entry declaration requires a braced body",
        )));
        return;
    }

    parser.start(SyntaxKind::EntryBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = token_count(parser);
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(end);
    parser.start(SyntaxKind::ItemList, SyntaxRole::Element(0));
    emit_entry_members(parser, source, close);
    bump_until(parser, close);
    parser.finish();
    if parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.entry.missing_body_close",
        );
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.missing_body_close",
            SourceRange::new(at, at),
            "missing closing `}` for entry declaration",
        )));
    }
    parser.finish();
}

fn emit_entry_members(parser: &mut ShadowDocumentParser<'_, '_>, source: &str, close: usize) {
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        bump_member_separators(parser, close);
        if parser.cursor() >= close {
            break;
        }

        let start = parser.cursor();
        let end = entry_member_boundary(parser, source, start, close);
        let spelling = parser.current_text();
        if let Some(role) = spelling.and_then(EntryRole::from_spelling) {
            emit_role_binding(parser, end, ordinal, role);
        } else {
            match spelling {
                Some("goto") => emit_goto(parser, end, ordinal),
                Some("route") => emit_route(parser, end, ordinal),
                _ if entry_option_equals(parser, start, end).is_some() => {
                    emit_option(parser, end, ordinal);
                }
                _ => emit_invalid_member(parser, end, ordinal),
            }
        }
        bump_until(parser, end);
        if parser.cursor() == start {
            parser.bump();
        }
        ordinal = ordinal.saturating_add(1);
    }
}

fn bump_member_separators(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    while parser.cursor() < end
        && parser.current().is_some_and(|token| {
            matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken
                    | SyntaxKind::NewlineToken
                    | SyntaxKind::CommentToken
                    | SyntaxKind::DocCommentToken
            ) || parser.text_of(token) == ";"
        })
    {
        parser.bump();
    }
}

fn entry_member_boundary(
    parser: &ShadowDocumentParser<'_, '_>,
    source: &str,
    start: usize,
    end: usize,
) -> usize {
    let mut depth = 0_usize;
    let member_indent = parser
        .token_at(start)
        .map_or(0, |token| line_indent(source, token.range().start()));
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return end;
        };
        let text = parser.text_of(token);
        if token.kind() == SyntaxKind::NewlineToken
            && (depth == 0
                || following_line_starts_entry_member(
                    parser,
                    source,
                    member_indent,
                    index + 1,
                    end,
                ))
        {
            return index;
        }
        if depth == 0 && text == ";" {
            return index;
        }
        match text {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    end
}

fn following_line_starts_entry_member(
    parser: &ShadowDocumentParser<'_, '_>,
    source: &str,
    member_indent: usize,
    start: usize,
    end: usize,
) -> bool {
    let Some(head) = first_significant(parser, start, end) else {
        return false;
    };
    let Some(spelling) = token_text(parser, head) else {
        return false;
    };
    let Some(token) = parser.token_at(head) else {
        return false;
    };
    // A same-or-lower-indented current-grammar member is a recovery boundary
    // for an unclosed nested group. Deeper lines remain expression/type
    // continuations even when they begin with a role-like identifier.
    if line_indent(source, token.range().start()) > member_indent {
        return false;
    }
    if EntryRole::from_spelling(spelling).is_some() || matches!(spelling, "goto" | "route") {
        return true;
    }
    matches!(
        Some(token.kind()),
        Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
    ) && first_significant(parser, head + 1, end).and_then(|index| token_text(parser, index))
        == Some("=")
}

fn line_indent(source: &str, offset: usize) -> usize {
    let line_start = source[..offset]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    source[line_start..offset]
        .char_indices()
        .find(|(_, character)| !matches!(character, ' ' | '\t'))
        .map_or(offset - line_start, |(indent, _)| indent)
}

fn emit_role_binding(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    ordinal: u32,
    role: EntryRole,
) {
    parser.start(SyntaxKind::EntryRoleBinding, SyntaxRole::Element(ordinal));
    emit_current_name(parser, SyntaxRole::Name);
    bump_trivia_before(parser, end);
    if parser.at("=") {
        parser.bump();
    } else {
        emit_missing_punctuation(parser, SyntaxRole::Recovery(0));
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.role_binding",
            SourceRange::new(parser.current_offset(), parser.current_offset()),
            "entry role binding requires `=` before its value",
        )));
    }
    bump_trivia_before(parser, end);

    if role.expects_type() {
        if parser.cursor() >= end {
            emit_missing_type(parser);
        } else {
            emit_type(parser, end, SyntaxRole::Type);
        }
    } else {
        emit_required_path(parser, end);
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_missing_type(parser: &mut ShadowDocumentParser<'_, '_>) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingType, SyntaxRole::Type);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.entry.role_value",
        SourceRange::new(at, at),
        "entry role requires a value",
    )));
}

fn emit_required_path(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    if parser.cursor() >= end {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, SyntaxRole::Initializer);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::IdentifierToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.role_value",
            SourceRange::new(at, at),
            "entry role requires a symbol path",
        )));
        return;
    }

    if !matches!(
        parser.current_kind(),
        Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
    ) {
        let range = token_range(parser, parser.cursor(), end);
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Initializer);
        bump_until(parser, end);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.role_path",
            range,
            "entry callable role requires a dotted symbol path",
        )));
        return;
    }

    emit_path(parser, end, SyntaxRole::Initializer);
    let Some(remainder) = first_significant(parser, parser.cursor(), end) else {
        bump_until(parser, end);
        return;
    };
    let range = token_range(parser, remainder, end);
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(1));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.entry.role_path",
        range,
        "entry callable role requires a dotted symbol path",
    )));
}

fn emit_goto(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    parser.start(SyntaxKind::EntryGoto, SyntaxRole::Element(ordinal));
    parser.bump();
    bump_trivia_before(parser, end);
    let target_start = parser.cursor();
    let valid = parser.current_kind() == Some(SyntaxKind::EntityReferenceToken)
        && first_significant(parser, target_start + 1, end).is_none();
    if target_start >= end {
        emit_missing_entity_reference(parser, SyntaxRole::Target, "syntax.entry.goto_target");
    } else {
        let range = token_range(parser, target_start, end);
        emit_expression(parser, end, SyntaxRole::Target);
        if !valid {
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.entry.goto_target",
                range,
                "entry `goto` requires one entity reference target",
            )));
        }
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_route(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    parser.start(SyntaxKind::EntryRoute, SyntaxRole::Element(ordinal));
    parser.bump();
    bump_trivia_before(parser, end);
    emit_route_method(parser, end);
    bump_trivia_before(parser, end);
    emit_route_path(parser, end);
    bump_trivia_before(parser, end);
    emit_route_arrow(parser);
    bump_trivia_before(parser, end);
    emit_route_target(parser, end);
    bump_trivia_before(parser, end);
    if parser.at("(") {
        emit_route_bindings(parser, end);
        bump_trivia_before(parser, end);
    }
    if parser.cursor() < end {
        let range = token_range(parser, parser.cursor(), end);
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        bump_until(parser, end);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.route_tail",
            range,
            "unexpected syntax after the entry route target",
        )));
    }
    parser.finish();
}

fn emit_route_method(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        )
    {
        emit_current_name(parser, SyntaxRole::Name);
        return;
    }

    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.entry.route_method",
        SourceRange::new(at, at),
        "entry route requires an HTTP method",
    )));
}

fn emit_route_path(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::StringToken | SyntaxKind::RawStringToken)
        )
    {
        parser.start(SyntaxKind::LiteralExpression, SyntaxRole::Operand);
        parser.bump();
        parser.finish();
        return;
    }

    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingExpression, SyntaxRole::Operand);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::StringToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.entry.route_path",
        SourceRange::new(at, at),
        "entry route requires a string path",
    )));
}

fn emit_route_arrow(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("->") {
        parser.bump();
        return;
    }

    let at = parser.current_offset();
    emit_missing_punctuation(parser, SyntaxRole::Recovery(0));
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.entry.route_arrow",
        SourceRange::new(at, at),
        "entry route requires `->` before its flow target",
    )));
}

fn emit_route_target(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    if parser.cursor() < end && parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        parser.start(SyntaxKind::EntityReferenceExpression, SyntaxRole::Target);
        parser.bump();
        parser.finish();
        return;
    }
    emit_missing_entity_reference(parser, SyntaxRole::Target, "syntax.entry.route_target");
}

fn emit_route_bindings(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.start(SyntaxKind::DelimitedGroup, SyntaxRole::Argument(0));
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    let close = find_matching_close(parser, parser.cursor(), "(")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::ArgumentList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    while parser.cursor() < close {
        bump_trivia_before(parser, close);
        if parser.cursor() >= close {
            break;
        }
        let binding_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(close);
        emit_route_binding(parser, binding_end, ordinal);
        bump_until(parser, binding_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    if parser.at(")") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseParenNode,
            ")",
            "syntax.entry.route_binding_close",
        );
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseParenNode,
            SyntaxRole::CloseDelimiter,
        );
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.route_binding_close",
            SourceRange::new(at, at),
            "missing closing `)` for entry route bindings",
        )));
    }
    parser.finish();
}

fn emit_route_binding(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u16) {
    parser.start(SyntaxKind::EntryRouteBinding, SyntaxRole::Argument(ordinal));
    if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        )
    {
        emit_current_name(parser, SyntaxRole::Name);
    } else {
        emit_missing_name(parser, SyntaxRole::Name);
    }
    bump_trivia_before(parser, end);
    if parser.at("=") {
        parser.bump();
    } else {
        emit_missing_punctuation(parser, SyntaxRole::Recovery(0));
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.route_binding",
            SourceRange::new(parser.current_offset(), parser.current_offset()),
            "entry route binding requires `=`",
        )));
    }
    bump_trivia_before(parser, end);
    if parser.at(":") {
        parser.bump();
    } else {
        emit_missing_punctuation(parser, SyntaxRole::Recovery(1));
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.route_binding",
            SourceRange::new(parser.current_offset(), parser.current_offset()),
            "entry route binding values must name a `:path_param`",
        )));
    }
    bump_trivia_before(parser, end);
    if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        )
    {
        emit_current_name(parser, SyntaxRole::Initializer);
    } else {
        emit_missing_name(parser, SyntaxRole::Initializer);
    }
    bump_trivia_before(parser, end);
    if parser.cursor() < end {
        let range = token_range(parser, parser.cursor(), end);
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(2));
        bump_until(parser, end);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.route_binding",
            range,
            "unexpected syntax in entry route binding",
        )));
    }
    parser.finish();
}

fn emit_option(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let equals = entry_option_equals(parser, parser.cursor(), end)
        .expect("entry option dispatch requires a top-level equals token");
    parser.start(SyntaxKind::EntryOption, SyntaxRole::Element(ordinal));
    emit_current_name(parser, SyntaxRole::Name);
    bump_trivia_before(parser, equals);
    if parser.cursor() < equals {
        let range = token_range(parser, parser.cursor(), equals);
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        bump_until(parser, equals);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.option_name",
            range,
            "entry option names must be one identifier",
        )));
    }
    bump_until(parser, equals);
    parser.bump();
    bump_trivia_before(parser, end);
    if parser.cursor() >= end {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingExpression, SyntaxRole::Initializer);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::IdentifierToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.option_value",
            SourceRange::new(at, at),
            "entry option requires a value expression",
        )));
    } else {
        emit_expression(parser, end, SyntaxRole::Initializer);
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_invalid_member(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let start = parser.cursor();
    let range = token_range(parser, start, end);
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Element(ordinal));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.entry.invalid_member",
        range,
        "entry bodies accept typed role bindings, `goto`, routes, and option assignments",
    )));
}

fn entry_option_equals(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    let first = parser.token_at(start)?;
    if !matches!(
        first.kind(),
        SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
    ) {
        return None;
    }
    let equals = find_top_level_boundary(parser, start + 1, &["="]);
    (equals < end).then_some(equals)
}

fn emit_current_name(parser: &mut ShadowDocumentParser<'_, '_>, role: SyntaxRole) {
    parser.start(SyntaxKind::NameReference, role);
    parser.bump();
    parser.finish();
}

fn emit_missing_name(parser: &mut ShadowDocumentParser<'_, '_>, role: SyntaxRole) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingName, role);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
}

fn emit_missing_entity_reference(
    parser: &mut ShadowDocumentParser<'_, '_>,
    role: SyntaxRole,
    diagnostic: &'static str,
) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingExpression, role);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::EntityReferenceToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        diagnostic,
        SourceRange::new(at, at),
        "entry target requires an entity reference",
    )));
}

fn emit_missing_punctuation(parser: &mut ShadowDocumentParser<'_, '_>, role: SyntaxRole) {
    parser.start(SyntaxKind::MissingTokenNode, role);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::PunctuationToken),
        at: parser.current_offset(),
    });
    parser.finish();
}

fn bump_trivia_before(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    while parser.cursor() < end
        && parser.current_kind().is_some_and(|kind| {
            matches!(
                kind,
                SyntaxKind::WhitespaceToken
                    | SyntaxKind::NewlineToken
                    | SyntaxKind::CommentToken
                    | SyntaxKind::DocCommentToken
            )
        })
    {
        parser.bump();
    }
}

fn token_range(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> SourceRange {
    let start = first_significant(parser, start, end).unwrap_or(start);
    let end = trimmed_end(parser, start, end);
    let range_start = parser
        .token_at(start)
        .map_or_else(|| parser.current_offset(), |token| token.range().start());
    let range_end = end
        .checked_sub(1)
        .and_then(|index| parser.token_at(index))
        .map_or(range_start, |token| token.range().end());
    SourceRange::new(range_start, range_end)
}
