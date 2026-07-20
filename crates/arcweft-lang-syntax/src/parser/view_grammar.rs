//! Private retained View declaration grammar.

use arcweft_id::RetainedIdentityFamily;
use arcweft_source::SourceRange;

use super::declaration::emit_retained_declaration_header;
use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::path::emit_path;
use super::pattern::emit_pattern;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, find_statement_terminator, find_top_level_boundary, first_significant,
    token_count, trimmed_end,
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
    parser.start(SyntaxKind::ViewDeclarationItem, role);
    emit_retained_declaration_header(
        &mut parser,
        RetainedIdentityFamily::View,
        emit_view_signature,
    );
    parser.bump_trivia();
    reject_view_header_extensions(&mut parser);
    parser.bump_trivia();
    emit_view_body(&mut parser);
    emit_trailing_recovery(&mut parser);
    parser.finish();
}

fn emit_view_signature(parser: &mut ShadowDocumentParser<'_, '_>) {
    if !parser.at("(") {
        emit_missing_parameter_group(parser);
        return;
    }
    parser.start(SyntaxKind::FixedParameterGroup, SyntaxRole::ParameterGroup);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.is_at_end() || parser.at(")") {
            break;
        }
        let parameter_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]);
        emit_view_parameter(parser, parameter_end, ordinal);
        bump_until(parser, parameter_end);
        if parser.budget_failed() {
            break;
        }
        ordinal = ordinal
            .checked_add(1)
            .expect("View parameter budget is below the role index range");
        if parser.at(",") {
            parser.bump();
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.view.missing_parameter_close",
    );
    parser.finish();
}

fn emit_missing_parameter_group(parser: &mut ShadowDocumentParser<'_, '_>) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::FixedParameterGroup, SyntaxRole::ParameterGroup);
    emit_missing_delimiter(
        parser,
        SyntaxKind::MissingTokenNode,
        SyntaxRole::OpenDelimiter,
    );
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    parser.finish();
    emit_missing_delimiter(
        parser,
        SyntaxKind::MissingTokenNode,
        SyntaxRole::CloseDelimiter,
    );
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.view.missing_parameters",
        SourceRange::new(at, at),
        "View declaration requires one fixed parameter group",
    )));
}

fn emit_view_parameter(
    parser: &mut ShadowDocumentParser<'_, '_>,
    parameter_end: usize,
    ordinal: u16,
) {
    parser.start(SyntaxKind::Parameter, SyntaxRole::Parameter(ordinal));
    let colon = find_top_level_boundary(parser, parser.cursor(), &[":"]).min(parameter_end);
    let colon = (colon < parameter_end
        && parser
            .token_at(colon)
            .is_some_and(|token| parser.text_of(token) == ":"))
    .then_some(colon);
    let pattern_end = colon.unwrap_or(parameter_end);
    let pattern_start = parser.cursor();
    let valid_binding = parameter_is_binding(parser, pattern_start, pattern_end);
    emit_pattern(parser, pattern_end, SyntaxRole::ParameterPattern);
    if !valid_binding {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.view.invalid_parameter",
            token_range(parser, pattern_start, pattern_end),
            "View parameters must be ordinary binding names",
        )));
    }
    bump_until(parser, pattern_end);

    let Some(colon) = colon else {
        emit_missing_parameter_type(parser);
        parser.finish();
        return;
    };
    debug_assert_eq!(parser.cursor(), colon);
    parser.bump();
    parser.bump_trivia();
    let default = find_top_level_boundary(parser, parser.cursor(), &["="]).min(parameter_end);
    let type_end = trimmed_end(parser, parser.cursor(), default);
    emit_type(parser, type_end, SyntaxRole::ParameterType);
    bump_until(parser, default);
    if parser.at("=") {
        parser.bump();
        parser.bump_trivia();
        let expression_end = trimmed_end(parser, parser.cursor(), parameter_end);
        emit_expression(parser, expression_end, SyntaxRole::Initializer);
    }
    bump_until(parser, parameter_end);
    parser.finish();
}

fn emit_missing_parameter_type(parser: &mut ShadowDocumentParser<'_, '_>) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingType, SyntaxRole::ParameterType);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.parameter.missing_type",
        SourceRange::new(at, at),
        "View parameter requires `: Type`",
    )));
}

fn parameter_is_binding(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> bool {
    let significant = (start..end)
        .filter_map(|index| {
            let token = parser.token_at(index)?;
            (!is_trivia(token.kind())).then_some(token)
        })
        .collect::<Vec<_>>();
    matches!(
        significant.as_slice(),
        [token]
            if matches!(
                token.kind(),
                SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
            )
    )
}

fn reject_view_header_extensions(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("{") || parser.is_at_end() {
        return;
    }
    let return_type = parser.at("->");
    let body = find_top_level_boundary(parser, parser.cursor(), &["{"]);
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, body);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        if return_type {
            "syntax.view.return_not_allowed"
        } else {
            "syntax.declaration.unexpected_header"
        },
        SourceRange::new(start, parser.current_offset()),
        if return_type {
            "View declarations do not declare return types"
        } else {
            "View accepts no generics, where clause, or contracts"
        },
    )));
}

fn emit_view_body(parser: &mut ShadowDocumentParser<'_, '_>) {
    if !parser.at("{") {
        emit_missing_body(parser);
        return;
    }

    parser.start(SyntaxKind::ViewDeclarationBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{");
    let body_end = close.unwrap_or_else(|| token_count(parser));
    emit_leading_exports(parser, body_end);
    emit_view_fragment(parser, body_end);
    bump_until(parser, body_end);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.view.missing_body_close",
    );
    parser.finish();
}

fn emit_missing_body(parser: &mut ShadowDocumentParser<'_, '_>) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::PunctuationToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.view.missing_body",
        SourceRange::new(at, at),
        "View declaration requires a braced typed View body",
    )));
}

fn emit_leading_exports(parser: &mut ShadowDocumentParser<'_, '_>, body_end: usize) {
    parser.bump_trivia();
    if !parser.at("export") {
        return;
    }
    parser.start(SyntaxKind::ViewExportBlock, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    while parser.cursor() < body_end && parser.at("export") {
        let entry_end = find_statement_terminator(parser, parser.cursor(), body_end)
            .map_or(body_end, |(end, _)| end);
        emit_export(parser, entry_end, ordinal, false);
        bump_until(parser, entry_end);
        if parser.at(";") || parser.current_kind() == Some(SyntaxKind::NewlineToken) {
            parser.bump();
        }
        parser.bump_trivia();
        if parser.budget_failed() {
            bump_until(parser, body_end);
            break;
        }
        ordinal = ordinal
            .checked_add(1)
            .expect("View export budget is below the role index range");
    }
    parser.finish();
}

fn emit_export(
    parser: &mut ShadowDocumentParser<'_, '_>,
    entry_end: usize,
    ordinal: u16,
    misplaced: bool,
) {
    let keyword_range = parser.current().expect("export keyword").range();
    parser.start(
        SyntaxKind::ViewExportDeclaration,
        SyntaxRole::Export(ordinal),
    );
    parser.bump();
    parser.bump_trivia();
    if parser.at("part") {
        parser.bump();
    } else {
        emit_missing_token_diagnostic(
            parser,
            "syntax.view.export_missing_part",
            "View export requires `export part`",
        );
    }
    parser.bump_trivia();
    let alias = find_top_level_boundary(parser, parser.cursor(), &["as"]).min(entry_end);
    emit_required_path(
        parser,
        alias,
        SyntaxRole::Target,
        "syntax.view.export_missing_local",
    );
    bump_until(parser, alias);
    if parser.at("as") {
        parser.bump();
    } else {
        emit_missing_token_diagnostic(
            parser,
            "syntax.view.export_missing_as",
            "View export requires `as` before its public part name",
        );
    }
    parser.bump_trivia();
    emit_required_path(
        parser,
        entry_end,
        SyntaxRole::Name,
        "syntax.view.export_missing_public",
    );
    bump_until(parser, entry_end);
    parser.finish();
    if misplaced {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.view.misplaced_export",
            keyword_range,
            "View part exports must form one leading block before View values",
        )));
    }
}

fn emit_required_path(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    code: &'static str,
) {
    let end = trimmed_end(parser, parser.cursor(), end);
    if first_significant(parser, parser.cursor(), end).is_none() {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, role);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            code,
            SourceRange::new(at, at),
            "View export requires a dotted name",
        )));
    } else {
        emit_path(parser, end, role);
    }
}

fn emit_view_fragment(parser: &mut ShadowDocumentParser<'_, '_>, body_end: usize) {
    parser.start(SyntaxKind::ViewFragment, SyntaxRole::Tail);
    let mut ordinal = 0_u32;
    let mut misplaced_export_ordinal = 0_u16;
    while parser.cursor() < body_end {
        parser.bump_trivia();
        if parser.cursor() >= body_end {
            break;
        }
        let entry_end = view_value_end(parser, parser.cursor(), body_end);
        if parser.at("export") {
            emit_export(parser, entry_end, misplaced_export_ordinal, true);
            misplaced_export_ordinal = misplaced_export_ordinal
                .checked_add(1)
                .expect("View export budget is below the role index range");
        } else {
            emit_expression(parser, entry_end, SyntaxRole::Element(ordinal));
        }
        bump_until(parser, entry_end);
        if parser.at(";") || parser.current_kind() == Some(SyntaxKind::NewlineToken) {
            parser.bump();
        }
        if parser.budget_failed() {
            bump_until(parser, body_end);
            break;
        }
        ordinal = ordinal
            .checked_add(1)
            .expect("View fragment role index exhausted");
    }
    parser.finish();
}

fn view_value_end(parser: &ShadowDocumentParser<'_, '_>, start: usize, body_end: usize) -> usize {
    let mut cursor = start;
    loop {
        let end =
            find_statement_terminator(parser, cursor, body_end).map_or(body_end, |(end, _)| end);
        let next_start = end.checked_add(1).unwrap_or(body_end);
        let Some(next) = first_significant(parser, next_start, body_end) else {
            return end;
        };
        if parser
            .token_at(next)
            .is_some_and(|token| parser.text_of(token) == ".")
        {
            cursor = next;
            continue;
        }
        return end;
    }
}

fn emit_missing_token_diagnostic(
    parser: &mut ShadowDocumentParser<'_, '_>,
    code: &'static str,
    message: &'static str,
) {
    let at = parser.current_offset();
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(at, at),
        message,
    )));
}

fn token_range(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> SourceRange {
    let first = (start..end).find_map(|index| {
        parser
            .token_at(index)
            .filter(|token| !is_trivia(token.kind()))
    });
    let Some(first) = first else {
        let at = parser.current_offset();
        return SourceRange::new(at, at);
    };
    let last = (start..end)
        .rev()
        .find_map(|index| {
            parser
                .token_at(index)
                .filter(|token| !is_trivia(token.kind()))
        })
        .unwrap_or(first);
    SourceRange::new(first.range().start(), last.range().end())
}

const fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}

fn emit_trailing_recovery(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.bump_trivia();
    if parser.is_at_end() {
        return;
    }
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    while parser.bump().is_some() {}
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.declaration.trailing_syntax",
        SourceRange::new(start, parser.current_offset()),
        "unexpected syntax after View declaration body",
    )));
}
