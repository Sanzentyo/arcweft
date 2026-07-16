//! Private typed-resource declaration grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::declaration::{emit_outer_prefixes, emit_visibility};
use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, find_top_level_boundary, first_significant, token_count, token_text,
    trimmed_end,
};
use super::type_ref::emit_type;
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
    parser.start(SyntaxKind::ResourceDeclarationItem, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();

    if parser.at("res") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_explicit_public_id(&mut parser);
    parser.bump_trivia();
    emit_resource_name(&mut parser);
    parser.bump_trivia();
    emit_colon(&mut parser);
    parser.bump_trivia();
    emit_resource_type(&mut parser);
    parser.bump_trivia();
    emit_resource_body(&mut parser);

    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_explicit_public_id(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.current_kind() != Some(SyntaxKind::EntityReferenceToken) {
        return;
    }
    let token = parser.current().expect("checked current resource ID token");
    if parser.text_of(token).starts_with("@.") {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.resource.relative_declaration_id",
            token.range(),
            "resource declaration IDs must be absolute",
        )));
    }
    parser.bump();
}

fn emit_resource_name(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
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
        "syntax.resource.missing_name",
        SourceRange::new(at, at),
        "resource declaration requires an ordinary local name",
    )));
}

fn emit_colon(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at(":") {
        parser.bump();
        return;
    }

    let at = parser.current_offset();
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::PunctuationToken),
        at,
    });
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.resource.missing_colon",
        SourceRange::new(at, at),
        "resource declaration requires `:` before its nominal type",
    )));
}

fn emit_resource_type(parser: &mut ShadowDocumentParser<'_, '_>) {
    let body = find_top_level_boundary(parser, parser.cursor(), &["{"]);
    let end = trimmed_end(parser, parser.cursor(), body);
    if parser.cursor() >= end {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingType, SyntaxRole::Type);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::IdentifierToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.resource.missing_type",
            SourceRange::new(at, at),
            "resource declaration requires a nominal resource type",
        )));
        return;
    }

    if !resource_type_head_is_path(parser, parser.cursor(), end) {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.resource.invalid_type_head",
            token_range(parser, parser.cursor(), end),
            "resource declaration type must be a nominal type path",
        )));
    }
    emit_type(parser, end, SyntaxRole::Type);
    bump_until(parser, end);
}

fn resource_type_head_is_path(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> bool {
    let significant = (start..end)
        .filter_map(|index| {
            let token = parser.token_at(index)?;
            (!matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken
                    | SyntaxKind::NewlineToken
                    | SyntaxKind::CommentToken
                    | SyntaxKind::DocCommentToken
            ))
            .then_some((token, parser.text_of(token)))
        })
        .collect::<Vec<_>>();
    let Some((first, _)) = significant.first() else {
        return false;
    };
    if !matches!(
        first.kind(),
        SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
    ) {
        return false;
    }

    let mut saw_segment = false;
    let mut expects_segment = false;
    let mut generic_depth = 0_usize;
    let mut generic_closed = false;
    for (token, text) in significant {
        if generic_depth != 0 {
            match text {
                "<" => generic_depth += 1,
                ">" => {
                    generic_depth -= 1;
                    generic_closed = generic_depth == 0;
                }
                _ => {}
            }
            continue;
        }
        if generic_closed {
            return false;
        }
        if expects_segment {
            if !matches!(
                token.kind(),
                SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
            ) {
                return false;
            }
            saw_segment = true;
            expects_segment = false;
            continue;
        }
        match text {
            "." | "::" if saw_segment => expects_segment = true,
            "<" if saw_segment => generic_depth = 1,
            _ if matches!(
                token.kind(),
                SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
            ) && !saw_segment =>
            {
                saw_segment = true;
            }
            _ => return false,
        }
    }
    saw_segment && !expects_segment && generic_depth == 0
}

fn emit_resource_body(parser: &mut ShadowDocumentParser<'_, '_>) {
    if !parser.at("{") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.resource.missing_body",
            SourceRange::new(at, at),
            "resource declaration requires a braced field body",
        )));
        return;
    }

    parser.start(SyntaxKind::ResourceBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = token_count(parser);
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(end);
    parser.start(SyntaxKind::FieldList, SyntaxRole::Element(0));
    emit_resource_fields(parser, close);
    bump_until(parser, close);
    parser.finish();
    if parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.resource.missing_body",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.resource.missing_body",
            SourceRange::new(at, at),
            "resource declaration body requires a closing `}`",
        )));
    }
    parser.finish();
}

fn emit_resource_fields(parser: &mut ShadowDocumentParser<'_, '_>, close: usize) {
    let mut ordinal = 0_u16;
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        if parser.at(",") {
            parser.bump();
            continue;
        }

        let end = resource_field_boundary(parser, parser.cursor(), close);
        emit_resource_field(parser, end, ordinal);
        bump_until(parser, end);
        if parser.at(",") {
            parser.bump();
        }
        ordinal = ordinal.saturating_add(1);
    }
}

fn emit_resource_field(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u16) {
    let field_start = parser.cursor();
    parser.start(
        SyntaxKind::ResourceFieldInitializer,
        SyntaxRole::Field(ordinal),
    );
    let significant_end = trimmed_end(parser, parser.cursor(), end);
    let Some(name) = first_significant(parser, parser.cursor(), significant_end) else {
        emit_malformed_field(parser, parser.cursor(), significant_end);
        parser.finish();
        return;
    };
    bump_until(parser, name);
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
    } else {
        emit_malformed_field(parser, field_start, significant_end);
        bump_until(parser, significant_end);
        parser.finish();
        return;
    }
    parser.bump_trivia();

    let equals = find_top_level_boundary(parser, parser.cursor(), &["="]).min(significant_end);
    if equals == significant_end || token_text(parser, equals) != Some("=") {
        emit_malformed_field(parser, field_start, significant_end);
        bump_until(parser, significant_end);
        parser.finish();
        return;
    }
    if parser.cursor() < equals {
        let recovery_start = parser.cursor();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        bump_until(parser, equals);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.resource.malformed_field",
            token_range(parser, recovery_start, equals),
            "resource field name must be followed directly by `=`",
        )));
    }
    parser.bump();
    parser.bump_trivia();
    if parser.cursor() >= significant_end {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingExpression, SyntaxRole::Initializer);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::TextToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.resource.missing_initializer",
            SourceRange::new(at, at),
            "resource field requires an initializer expression",
        )));
    } else {
        emit_expression(parser, significant_end, SyntaxRole::Initializer);
        bump_until(parser, significant_end);
    }
    parser.finish();
}

fn emit_malformed_field(parser: &mut ShadowDocumentParser<'_, '_>, start: usize, end: usize) {
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.resource.malformed_field",
        token_range(parser, start, end),
        "resource field must have the form `name = expression`",
    )));
}

fn resource_field_boundary(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> usize {
    let mut depth = 0_usize;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return index;
        };
        let text = parser.text_of(token);
        if depth == 0 && (text == "," || token.kind() == SyntaxKind::NewlineToken) {
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

fn token_range(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> SourceRange {
    let start_offset = parser
        .token_at(start)
        .map_or_else(|| parser.current_offset(), |token| token.range().start());
    let end_offset = (start..end)
        .rev()
        .find_map(|index| parser.token_at(index).map(|token| token.range().end()))
        .unwrap_or(start_offset);
    SourceRange::new(start_offset, end_offset)
}
