//! Private retained Action channel declaration grammar.

use arcweft_id::RetainedIdentityFamily;
use arcweft_source::SourceRange;

use super::declaration::emit_retained_declaration_header;
use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::pattern::emit_pattern;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, find_top_level_boundary, token_count, trimmed_end,
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
    parser.start(SyntaxKind::ActionDeclarationItem, role);
    emit_retained_declaration_header(
        &mut parser,
        RetainedIdentityFamily::Action,
        emit_action_signature,
    );
    emit_action_terminator_and_recovery(&mut parser);
    parser.finish();
}

fn emit_action_signature(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.start(SyntaxKind::ActionSignature, SyntaxRole::ParameterGroup);
    if !parser.at("(") {
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
            "syntax.action.missing_parameters",
            SourceRange::new(at, at),
            "Action declaration requires one fixed parameter group",
        )));
        parser.finish();
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
        let end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]);
        emit_action_parameter(parser, end, ordinal);
        bump_until(parser, end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.action.missing_parameter_close",
    );
    parser.finish();
    parser.finish();
}

fn emit_action_parameter(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u16) {
    parser.start(SyntaxKind::Parameter, SyntaxRole::Parameter(ordinal));
    let colon = find_top_level_boundary(parser, parser.cursor(), &[":"]).min(end);
    let colon = (colon < end
        && parser
            .token_at(colon)
            .is_some_and(|token| parser.text_of(token) == ":"))
    .then_some(colon);
    let pattern_end = colon.unwrap_or(end);
    let pattern_start = parser.cursor();
    let valid_binding = action_parameter_is_binding(parser, pattern_start, pattern_end);
    emit_pattern(parser, pattern_end, SyntaxRole::ParameterPattern);
    if !valid_binding {
        let range = token_range(parser, pattern_start, pattern_end);
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.action.invalid_parameter",
            range,
            "Action parameters must be ordinary binding names",
        )));
    }
    bump_until(parser, pattern_end);

    let Some(colon) = colon else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingType, SyntaxRole::ParameterType);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.parameter.missing_type",
            SourceRange::new(at, at),
            "Action parameter requires `: Type`",
        )));
        parser.finish();
        return;
    };

    debug_assert_eq!(parser.cursor(), colon);
    parser.bump();
    parser.bump_trivia();
    let default = find_top_level_boundary(parser, parser.cursor(), &["="]).min(end);
    let type_end = trimmed_end(parser, parser.cursor(), default);
    emit_type(parser, type_end, SyntaxRole::ParameterType);
    bump_until(parser, default);
    if default < end && parser.at("=") {
        let default_start = parser.current_offset();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        parser.bump();
        parser.bump_trivia();
        let expression_end = trimmed_end(parser, parser.cursor(), end);
        emit_expression(parser, expression_end, SyntaxRole::Initializer);
        bump_until(parser, end);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.action.default_not_allowed",
            SourceRange::new(default_start, parser.current_offset()),
            "Action channel parameters do not accept defaults",
        )));
    }
    parser.finish();
}

fn action_parameter_is_binding(
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
            .then_some(token)
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

fn emit_action_terminator_and_recovery(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.bump_trivia();
    if parser.at(";") {
        parser.bump();
        parser.bump_trivia();
    }
    if parser.is_at_end() {
        return;
    }

    let start = parser.current_offset();
    if parser.at("{") {
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        parser.bump();
        let close = find_matching_close(parser, parser.cursor(), "{");
        bump_until(
            parser,
            close.map_or_else(|| token_count(parser), |index| index + 1),
        );
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.action.body_not_allowed",
            SourceRange::new(start, parser.current_offset()),
            "Action is a bodyless typed channel",
        )));
    } else {
        let return_type = parser.at("->");
        let end = trimmed_end(parser, parser.cursor(), token_count(parser));
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        bump_until(parser, end);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            if return_type {
                "syntax.action.return_not_allowed"
            } else {
                "syntax.declaration.trailing_syntax"
            },
            SourceRange::new(start, parser.current_offset()),
            if return_type {
                "Action channels do not declare return types"
            } else {
                "unexpected syntax after Action signature"
            },
        )));
    }
    while parser.bump().is_some() {}
}

fn token_range(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> SourceRange {
    let start = parser
        .token_at(start)
        .map_or(parser.current_offset(), |token| token.range().start());
    let end = end
        .checked_sub(1)
        .and_then(|index| parser.token_at(index))
        .map_or(start, |token| token.range().end());
    SourceRange::new(start, end)
}
