//! Private module and import grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::declaration::emit_visibility;
use super::document::ShadowDocumentParser;
use super::lexer::LexToken;
use super::path::emit_path;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, first_significant, token_count, token_text, trimmed_end,
};
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    kind: SyntaxKind,
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    debug_assert!(matches!(
        kind,
        SyntaxKind::ModuleDeclaration | SyntaxKind::UseDeclaration
    ));
    let mut parser = ShadowDocumentParser::new(source, tokens, events, budget);
    parser.start(kind, role);
    parser.bump_trivia();
    if kind == SyntaxKind::ModuleDeclaration {
        emit_module(&mut parser);
    } else {
        emit_use(&mut parser);
    }
    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_module(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("pub") {
        let start = parser.current_offset();
        emit_visibility(parser);
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.module.visibility_not_allowed",
            SourceRange::new(start, parser.current_offset()),
            "a source module declaration does not accept visibility",
        )));
        parser.bump_trivia();
    }
    if parser.at("mod") {
        parser.bump();
    }
    parser.bump_trivia();
    let end = trimmed_end(parser, parser.cursor(), token_count(parser));
    emit_required_path(
        parser,
        end,
        SyntaxRole::Target,
        "syntax.module.missing_path",
        "module declaration requires a module path",
    );
    emit_unexpected_tail(
        parser,
        end,
        "syntax.module.invalid_path",
        "unexpected token after module path",
    );
}

fn emit_use(parser: &mut ShadowDocumentParser<'_, '_>) {
    emit_visibility(parser);
    parser.bump_trivia();
    if parser.at("use") {
        parser.bump();
    }
    parser.bump_trivia();
    let end = trimmed_end(parser, parser.cursor(), token_count(parser));
    if parser.cursor() == end {
        emit_missing_path(
            parser,
            SyntaxRole::Target,
            "syntax.use.missing_tree",
            "use declaration requires an import tree",
        );
        return;
    }

    if let Some(open) = top_level_token(parser, parser.cursor(), end, "{") {
        emit_grouped_use(parser, open, end);
    } else {
        emit_path_or_glob_use(parser, end);
    }
}

fn emit_grouped_use(parser: &mut ShadowDocumentParser<'_, '_>, open: usize, end: usize) {
    let path_end = preceding_separator(parser, parser.cursor(), open).unwrap_or(open);
    emit_required_path(
        parser,
        path_end,
        SyntaxRole::Target,
        "syntax.use.missing_tree",
        "grouped use declaration requires a module path",
    );
    bump_until(parser, open);

    parser.start(SyntaxKind::DelimitedGroup, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(end);
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let member_end = top_level_token(parser, parser.cursor(), close, ",").unwrap_or(close);
        emit_group_member(parser, member_end, ordinal);
        bump_until(parser, member_end);
        if parser.at(",") {
            parser.bump();
        }
        ordinal = ordinal.saturating_add(1);
    }
    if parser.cursor() == close && parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.use.missing_group_close",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.use.missing_group_close",
            SourceRange::new(at, at),
            "missing closing `}` for grouped use declaration",
        )));
    }
    parser.finish();
    emit_unexpected_tail(
        parser,
        end,
        "syntax.use.invalid_tree",
        "unexpected token after grouped use declaration",
    );
}

fn emit_group_member(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let significant_end = trimmed_end(parser, parser.cursor(), end);
    let Some(name) = first_significant(parser, parser.cursor(), significant_end) else {
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Element(ordinal));
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.use.invalid_group_member",
            SourceRange::new(parser.current_offset(), parser.current_offset()),
            "grouped use declaration contains an empty member",
        )));
        return;
    };
    bump_until(parser, name);
    if !is_path_segment(parser, name) {
        let start = parser.current_offset();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Element(ordinal));
        bump_until(parser, significant_end);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.use.invalid_group_member",
            SourceRange::new(start, parser.current_offset()),
            "grouped use member must be one name with an optional alias",
        )));
        return;
    }

    parser.start(SyntaxKind::NameReference, SyntaxRole::Element(ordinal));
    parser.bump();
    parser.finish();
    parser.bump_trivia();
    if parser.at("as") {
        emit_alias(parser, significant_end);
    }
    emit_unexpected_tail(
        parser,
        significant_end,
        "syntax.use.invalid_group_member",
        "grouped use member must be one name with an optional alias",
    );
}

fn emit_path_or_glob_use(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let alias = top_level_token(parser, parser.cursor(), end, "as");
    let path_or_glob_end = alias.unwrap_or(end);
    let star = last_significant(parser, parser.cursor(), path_or_glob_end)
        .filter(|index| token_text(parser, *index) == Some("*"));
    let path_end = star
        .and_then(|star| preceding_separator(parser, parser.cursor(), star))
        .unwrap_or(path_or_glob_end);
    emit_required_path(
        parser,
        path_end,
        SyntaxRole::Target,
        "syntax.use.missing_tree",
        "use declaration requires an import path",
    );
    bump_until(parser, path_or_glob_end);
    if let Some(alias) = alias {
        bump_until(parser, alias);
        emit_alias(parser, end);
    }
    emit_unexpected_tail(
        parser,
        end,
        "syntax.use.invalid_tree",
        "unexpected token after use tree",
    );
}

fn emit_alias(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    debug_assert!(parser.at("as"));
    parser.bump();
    parser.bump_trivia();
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
        "syntax.use.missing_alias",
        SourceRange::new(at, at),
        "`as` requires an ordinary alias name",
    )));
    bump_until(parser, end);
}

fn emit_required_path(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    diagnostic: &'static str,
    message: &'static str,
) {
    let Some(first) = first_significant(parser, parser.cursor(), end) else {
        emit_missing_path(parser, role, diagnostic, message);
        return;
    };
    bump_until(parser, first);
    if is_path_segment(parser, first) {
        emit_path(parser, end, role);
    } else {
        emit_missing_path(parser, role, diagnostic, message);
    }
}

fn emit_missing_path(
    parser: &mut ShadowDocumentParser<'_, '_>,
    role: SyntaxRole,
    diagnostic: &'static str,
    message: &'static str,
) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::Path, role);
    parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        diagnostic,
        SourceRange::new(at, at),
        message,
    )));
}

fn emit_unexpected_tail(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    diagnostic: &'static str,
    message: &'static str,
) {
    parser.bump_trivia();
    if parser.cursor() >= end {
        return;
    }
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        diagnostic,
        SourceRange::new(start, parser.current_offset()),
        message,
    )));
}

fn top_level_token(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    spelling: &str,
) -> Option<usize> {
    let mut depth = 0_usize;
    for index in start..end {
        let text = token_text(parser, index)?;
        if depth == 0 && text == spelling {
            return Some(index);
        }
        match text {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn preceding_separator(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    before: usize,
) -> Option<usize> {
    (start..before)
        .rev()
        .find(|index| token_text(parser, *index).is_some_and(|text| matches!(text, "." | "::")))
}

fn last_significant(
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

fn is_path_segment(parser: &ShadowDocumentParser<'_, '_>, index: usize) -> bool {
    parser.token_at(index).is_some_and(|token| {
        matches!(
            token.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken | SyntaxKind::LifetimeToken
        )
    })
}
