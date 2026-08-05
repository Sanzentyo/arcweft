//! Private external-capability grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use super::declaration::{
    FixedParameterGrammar, emit_fixed_parameters, emit_generic_parameters,
    emit_missing_parameter_group, emit_name, emit_outer_prefixes, emit_visibility,
};
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    emit_required_punctuation, find_matching_close, find_matching_close_before,
    find_top_level_boundary, first_significant, token_count, token_text, trimmed_end,
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
    parser.start(SyntaxKind::ExternCapabilityItem, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();

    if parser.at("extern") {
        parser.bump();
    }
    parser.bump_trivia();
    if parser.at("capability") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_name(&mut parser, "capability");
    parser.bump_trivia();
    recover_header_tail(&mut parser);
    emit_body(&mut parser, source);

    while parser.bump().is_some() {}
    parser.finish();
}

fn recover_header_tail(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("{") || parser.is_at_end() {
        return;
    }

    let start = parser.current_offset();
    let end = find_top_level_boundary(parser, parser.cursor(), &["{"]);
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.capability.invalid_header",
        SourceRange::new(start, parser.current_offset()),
        "unexpected token between the capability name and body",
    )));
    parser.bump_trivia();
}

fn emit_body(parser: &mut ShadowDocumentParser<'_, '_>, source: &str) {
    if !parser.at("{") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.capability.missing_body",
            SourceRange::new(at, at),
            "external capability declaration requires a braced body",
        )));
        return;
    }

    parser.start(SyntaxKind::DelimitedGroup, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = token_count(parser);
    let close = find_matching_close(parser, parser.cursor(), "{")
        .or_else(|| recovered_outer_close(parser, source, parser.cursor(), end))
        .unwrap_or(end);
    parser.start(SyntaxKind::ItemList, SyntaxRole::Element(0));
    emit_members(parser, close);
    bump_until(parser, close);
    parser.finish();
    if parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.capability.missing_body_close",
        );
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.capability.missing_body_close",
            SourceRange::new(at, at),
            "missing closing `}` for external capability declaration",
        )));
    }
    parser.finish();
}

fn emit_members(parser: &mut ShadowDocumentParser<'_, '_>, close: usize) {
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        bump_member_separators(parser);
        if parser.cursor() >= close {
            break;
        }
        let end = member_boundary(parser, parser.cursor(), close);
        match member_head(parser, parser.cursor(), end) {
            Some("type") => emit_type_member(parser, end, ordinal),
            Some("fn") => emit_function_member(parser, end, ordinal),
            _ => emit_error_member(parser, end, ordinal),
        }
        bump_until(parser, end);
        ordinal = ordinal.saturating_add(1);
    }
}

fn bump_member_separators(parser: &mut ShadowDocumentParser<'_, '_>) {
    while parser.current_kind().is_some_and(|kind| {
        matches!(
            kind,
            SyntaxKind::WhitespaceToken | SyntaxKind::NewlineToken | SyntaxKind::CommentToken
        )
    }) {
        parser.bump();
    }
}

fn emit_type_member(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let content_end = member_content_end(parser, parser.cursor(), end);
    parser.start(SyntaxKind::TypeAliasItem, SyntaxRole::Element(ordinal));
    emit_outer_prefixes(parser);
    parser.bump_trivia();
    emit_visibility(parser);
    parser.bump_trivia();
    if parser.at("type") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_name(parser, "type");
    parser.bump_trivia();
    if parser.at("<") {
        emit_generic_parameters(parser);
        parser.bump_trivia();
    }
    if parser.at("=") {
        parser.bump();
        parser.bump_trivia();
        emit_type(parser, content_end, SyntaxRole::Type);
    } else if parser.cursor() < content_end {
        emit_member_tail_error(parser, content_end, "capability type");
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_function_member(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let content_end = member_content_end(parser, parser.cursor(), end);
    parser.start(SyntaxKind::FunctionItem, SyntaxRole::Element(ordinal));
    emit_outer_prefixes(parser);
    parser.bump_trivia();
    emit_visibility(parser);
    parser.bump_trivia();
    if parser.at("fn") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_name(parser, "fn");
    parser.bump_trivia();
    if parser.at("<") {
        emit_generic_parameters(parser);
        parser.bump_trivia();
    }

    let mut groups = 0_u16;
    while parser.at("(") && parser.cursor() < content_end {
        emit_fixed_parameters(
            parser,
            FixedParameterGrammar::TypedPattern,
            "capability function parameters require an authored type",
            "syntax.capability.unclosed_parameters",
        );
        groups = groups.saturating_add(1);
        parser.bump_trivia();
    }
    if groups == 0 {
        emit_missing_parameter_group(parser, "fn", "at least one parameter group");
        parser.bump_trivia();
    }

    if parser.at("->") {
        emit_return_type(parser, content_end);
        parser.bump_trivia();
    }
    if parser.at("effects") {
        emit_effect_clause(parser, content_end);
        parser.bump_trivia();
    }
    if parser.cursor() < content_end {
        emit_member_tail_error(parser, content_end, "capability function");
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_return_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.start(SyntaxKind::ReturnType, SyntaxRole::ReturnType);
    emit_required_punctuation(
        parser,
        SyntaxKind::ThinArrowNode,
        SyntaxRole::Token,
        "->",
        "syntax.return.missing_arrow",
        "authored return type requires `->`",
    );
    parser.bump_trivia();
    let type_end = find_top_level_boundary(parser, parser.cursor(), &["effects"]).min(end);
    emit_type(parser, type_end, SyntaxRole::Type);
    bump_until(parser, type_end);
    parser.finish();
}

fn emit_effect_clause(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.start(SyntaxKind::DelimitedGroup, SyntaxRole::Element(0));
    parser.bump();
    parser.bump_trivia();
    if !parser.at("{") {
        let at = parser.current_offset();
        emit_missing_delimiter(parser, SyntaxKind::OpenBraceNode, SyntaxRole::OpenDelimiter);
        parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
        parser.finish();
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.capability.effects_requires_braces",
            SourceRange::new(at, at),
            "capability effects require a braced expression list",
        )));
        parser.finish();
        return;
    }

    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close_before(parser, parser.cursor(), end, "{").unwrap_or(end);
    parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let expression_end = find_top_level_boundary(parser, parser.cursor(), &[","]).min(close);
        emit_expression(parser, expression_end, SyntaxRole::Element(ordinal));
        bump_until(parser, expression_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        }
    }
    parser.finish();
    if parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.capability.missing_effects_close",
        );
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.capability.missing_effects_close",
            SourceRange::new(at, at),
            "missing closing `}` for capability effects",
        )));
    }
    parser.finish();
}

fn emit_member_tail_error(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    owner: &'static str,
) {
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.capability.invalid_member_tail",
        SourceRange::new(start, parser.current_offset()),
        format!("unexpected token in {owner} declaration"),
    )));
}

fn emit_error_member(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorItem, SyntaxRole::Element(ordinal));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.capability.invalid_member",
        SourceRange::new(start, parser.current_offset()),
        "external capability bodies accept type and function declarations",
    )));
}

fn member_head<'a>(
    parser: &'a ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<&'a str> {
    let index = member_payload_index(parser, start, end)?;
    token_text(parser, index)
}

fn member_payload_index(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    let mut index = next_non_trivia(parser, start, end)?;
    while token_text(parser, index) == Some("#") {
        index = next_non_trivia(parser, index + 1, end)?;
        if token_text(parser, index) != Some("[") {
            return None;
        }
        index = skip_balanced_group(parser, index, end, "[", "]")?;
        index = next_non_trivia(parser, index, end)?;
    }
    if token_text(parser, index) == Some("pub") {
        index = next_non_trivia(parser, index + 1, end)?;
        if token_text(parser, index) == Some("(") {
            index = skip_balanced_group(parser, index, end, "(", ")")?;
            index = next_non_trivia(parser, index, end)?;
        }
    }
    Some(index)
}

fn member_boundary(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> usize {
    let mut depth = 0_usize;
    let payload = member_payload_index(parser, start, end);
    let mut saw_braced_clause = false;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return index;
        };
        let text = parser.text_of(token);
        let saw_payload = payload.is_some_and(|payload| index >= payload);
        if depth == 0 && text == ";" && saw_payload {
            return index + 1;
        }
        if token.kind() == SyntaxKind::NewlineToken && saw_payload {
            let next =
                first_significant(parser, index + 1, end).and_then(|next| token_text(parser, next));
            let previous = previous_significant(parser, start, index)
                .and_then(|previous| token_text(parser, previous));
            if depth == 0 {
                if next.is_some_and(|next| matches!(next, "effects" | "(" | "<" | "->" | "="))
                    || previous.is_some_and(|previous| matches!(previous, "->" | "="))
                {
                    continue;
                }
                return index;
            }
            if next.is_some_and(|next| matches!(next, "type" | "fn")) {
                return index;
            }
        }
        match text {
            "(" | "[" | "<" => depth += 1,
            ")" | "]" | ">" => depth = depth.saturating_sub(1),
            "{" => {
                saw_braced_clause = true;
                depth += 1;
            }
            "}" if depth != 0 => {
                depth -= 1;
                if saw_braced_clause && depth == 0 {
                    return member_close_end(parser, index + 1, end);
                }
            }
            _ => {}
        }
    }
    end
}

fn member_close_end(
    parser: &ShadowDocumentParser<'_, '_>,
    after_close: usize,
    end: usize,
) -> usize {
    first_significant(parser, after_close, end)
        .filter(|next| token_text(parser, *next) == Some(";"))
        .map_or(after_close, |semicolon| semicolon + 1)
}

fn member_content_end(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> usize {
    let end = trimmed_end(parser, start, end);
    if end > start && token_text(parser, end - 1) == Some(";") {
        end - 1
    } else {
        end
    }
}

fn recovered_outer_close(
    parser: &ShadowDocumentParser<'_, '_>,
    source: &str,
    body_start: usize,
    end: usize,
) -> Option<usize> {
    let declaration_indent = (0..body_start)
        .rev()
        .find(|index| token_text(parser, *index) == Some("extern"))
        .and_then(|index| parser.token_at(index))
        .map(|token| line_indent(source, token.range().start()))?;
    (body_start..end).rev().find(|index| {
        token_text(parser, *index) == Some("}")
            && parser
                .token_at(*index)
                .map(|token| line_indent(source, token.range().start()))
                .is_some_and(|indent| indent <= declaration_indent)
    })
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

fn previous_significant(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    (start..end).rev().find(|index| {
        parser.token_at(*index).is_some_and(|token| {
            !matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken
                    | SyntaxKind::NewlineToken
                    | SyntaxKind::CommentToken
                    | SyntaxKind::DocCommentToken
            )
        })
    })
}

fn next_non_trivia(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    (start..end).find(|index| {
        parser.token_at(*index).is_some_and(|token| {
            !matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken
                    | SyntaxKind::NewlineToken
                    | SyntaxKind::CommentToken
                    | SyntaxKind::DocCommentToken
            )
        })
    })
}

fn skip_balanced_group(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    open: &str,
    close: &str,
) -> Option<usize> {
    let mut depth = 0_usize;
    for index in start..end {
        match token_text(parser, index)? {
            text if text == open => depth += 1,
            text if text == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }
    None
}
