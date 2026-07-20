//! Private retained Character declaration grammar.

use arcweft_id::RetainedIdentityFamily;
use arcweft_source::SourceRange;

use super::declaration::emit_retained_declaration_header;
use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, expected, find_matching_close,
    find_statement_terminator, token_count, trimmed_end,
};
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = ShadowDocumentParser::new(source, tokens, events, budget);
    parser.start(SyntaxKind::CharacterDeclarationItem, role);
    emit_retained_declaration_header(
        &mut parser,
        RetainedIdentityFamily::Character,
        emit_surface_alias,
    );
    parser.bump_trivia();
    recover_unexpected_header(&mut parser);
    parser.bump_trivia();
    emit_character_body(&mut parser);
    parser.bump_trivia();
    if !parser.is_at_end() {
        let start = parser.current_offset();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        while parser.bump().is_some() {}
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.declaration.trailing_syntax",
            SourceRange::new(start, parser.current_offset()),
            "unexpected syntax after Character declaration body",
        )));
    }
    parser.finish();
}

fn emit_surface_alias(parser: &mut ShadowDocumentParser<'_, '_>) {
    if !parser.at("as") {
        return;
    }
    parser.start(SyntaxKind::SurfaceAlias, SyntaxRole::Alias);
    parser.bump();
    parser.bump_trivia();
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::IdentifierToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.character.missing_alias",
            SourceRange::new(at, at),
            "Character `as` requires one surface-alias identifier",
        )));
    }
    parser.finish();
}

fn recover_unexpected_header(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("{") || parser.is_at_end() {
        return;
    }
    let start = parser.current_offset();
    let body = (parser.cursor()..token_count(parser))
        .find(|index| {
            parser
                .token_at(*index)
                .is_some_and(|token| parser.text_of(token) == "{")
        })
        .unwrap_or_else(|| token_count(parser));
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, body);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.declaration.unexpected_header",
        SourceRange::new(start, parser.current_offset()),
        "Character declaration accepts only an optional `as` alias after its name",
    )));
}

fn emit_character_body(parser: &mut ShadowDocumentParser<'_, '_>) {
    if !parser.at("{") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.declaration.missing_body",
            SourceRange::new(at, at),
            "Character declaration requires a braced body",
        )));
        return;
    }

    parser.start(SyntaxKind::CharacterBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{");
    let body_end = close.unwrap_or_else(|| token_count(parser));
    emit_character_members(parser, body_end);
    bump_until(parser, body_end);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.declaration.missing_close",
    );
    parser.finish();
}

fn emit_character_members(parser: &mut ShadowDocumentParser<'_, '_>, body_end: usize) {
    let mut ordinal = 0_u16;
    let mut display_name = None;
    while parser.cursor() < body_end {
        parser.bump_trivia();
        if parser.cursor() >= body_end {
            break;
        }
        let start_index = parser.cursor();
        let (line_end, has_terminator) = find_statement_terminator(parser, start_index, body_end)
            .map_or((body_end, false), |(end, _)| (end, true));
        let name = parser.current().expect("member start is in the body");
        if parser.text_of(name) == "display_name" {
            emit_display_name_member(parser, line_end, ordinal, &mut display_name, name.range());
        } else {
            emit_unknown_member(parser, line_end, ordinal, name.range());
        }
        bump_until(parser, line_end);
        if has_terminator {
            parser.bump();
        }
        ordinal = ordinal.saturating_add(1);
    }
}

fn emit_display_name_member(
    parser: &mut ShadowDocumentParser<'_, '_>,
    line_end: usize,
    ordinal: u16,
    first_display_name: &mut Option<SourceRange>,
    keyword_range: SourceRange,
) {
    parser.start(
        SyntaxKind::CharacterDisplayNameMember,
        SyntaxRole::Member(ordinal),
    );
    if let Some(first) = *first_display_name {
        parser.start(SyntaxKind::ErrorDeclarationMember, SyntaxRole::Recovery(0));
        parser.bump();
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(
            PendingSyntaxDiagnostic::new(
                "syntax.character.duplicate_member",
                keyword_range,
                "Character `display_name` may appear only once",
            )
            .with_related_range(first),
        ));
    } else {
        *first_display_name = Some(keyword_range);
        parser.bump();
    }
    parser.bump_trivia();
    if parser.at("=") {
        parser.bump();
    } else {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.character.missing_assignment",
            SourceRange::new(at, at),
            "Character `display_name` requires `=`",
        )));
    }
    parser.bump_trivia();
    let expression_end = trimmed_end(parser, parser.cursor(), line_end);
    if parser.cursor() == expression_end {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingMemberValue, SyntaxRole::Initializer);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.character.missing_display_name",
            SourceRange::new(at, at),
            "Character `display_name` requires a constant String expression",
        )));
    } else {
        emit_expression(parser, expression_end, SyntaxRole::Initializer);
    }
    bump_until(parser, line_end);
    parser.finish();
}

fn emit_unknown_member(
    parser: &mut ShadowDocumentParser<'_, '_>,
    line_end: usize,
    ordinal: u16,
    name_range: SourceRange,
) {
    parser.start(
        SyntaxKind::ErrorDeclarationMember,
        SyntaxRole::Member(ordinal),
    );
    bump_until(parser, line_end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.character.unknown_member",
        name_range,
        "Character body accepts only `display_name`",
    )));
}
