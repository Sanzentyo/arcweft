//! Private module and import grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use super::declaration::emit_visibility;
use super::lexer::LexToken;
use super::path::emit_path;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, first_significant, token_count, token_text, trimmed_end,
};
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::source_projection::{
    PendingPathProjection, PendingPathRoot, PendingPathSegmentKind, PendingUseAlias,
    PendingUseGroupMember, PendingUseProjection, PendingUseTreeKind,
};

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
    let use_owner = if kind == SyntaxKind::UseDeclaration {
        parser.start_projected_owner(kind, role)
    } else {
        parser.start(kind, role);
        None
    };
    parser.bump_trivia();
    if kind == SyntaxKind::ModuleDeclaration {
        emit_module(&mut parser);
    } else {
        let projection = emit_use(&mut parser);
        parser.set_use_projection(use_owner, projection);
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

fn emit_use(parser: &mut ShadowDocumentParser<'_, '_>) -> PendingUseProjection {
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
        return PendingUseProjection::new(PendingUseTreeKind::Path, Vec::new());
    }

    if let Some(open) = top_level_token(parser, parser.cursor(), end, "{") {
        emit_grouped_use(parser, open, end)
    } else {
        emit_path_or_glob_use(parser, end)
    }
}

fn emit_grouped_use(
    parser: &mut ShadowDocumentParser<'_, '_>,
    open: usize,
    end: usize,
) -> PendingUseProjection {
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
    let mut name_ordinal = 0_u32;
    let mut alias_ordinal = 0_u16;
    let mut recovery_ordinal = 0_u32;
    let mut aliases = Vec::new();
    let mut members = Vec::new();
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let member_end = top_level_token(parser, parser.cursor(), close, ",").unwrap_or(close);
        if !parser.charge_grouped_use_member() {
            bump_until(parser, close);
            break;
        }
        members.push(emit_group_member(
            parser,
            member_end,
            &mut name_ordinal,
            &mut alias_ordinal,
            &mut recovery_ordinal,
            &mut aliases,
        ));
        bump_until(parser, member_end);
        if parser.at(",") {
            parser.bump();
        }
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
    emit_recovery_tail(
        parser,
        end,
        recovery_ordinal,
        "syntax.use.invalid_tree",
        "unexpected token after grouped use declaration",
    );
    PendingUseProjection::new(
        PendingUseTreeKind::Group(members.into_boxed_slice()),
        aliases,
    )
}

fn emit_group_member(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    name_ordinal: &mut u32,
    alias_ordinal: &mut u16,
    recovery_ordinal: &mut u32,
    aliases: &mut Vec<PendingUseAlias>,
) -> PendingUseGroupMember {
    let source_start = parser.current_offset();
    let significant_end = trimmed_end(parser, parser.cursor(), end);
    let Some(name) = first_significant(parser, parser.cursor(), significant_end) else {
        let recovery = *recovery_ordinal;
        *recovery_ordinal = recovery
            .checked_add(1)
            .expect("grouped-use member budget bounds recovery ordinals");
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(recovery));
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.use.invalid_group_member",
            SourceRange::new(parser.current_offset(), parser.current_offset()),
            "grouped use declaration contains an empty member",
        )));
        return PendingUseGroupMember::Recovery {
            source: SourceRange::new(source_start, parser.current_offset()),
            recovery_ordinal: recovery,
        };
    };
    bump_until(parser, name);
    if !is_path_segment(parser, name) {
        let start = parser.current_offset();
        let recovery = *recovery_ordinal;
        *recovery_ordinal = recovery
            .checked_add(1)
            .expect("grouped-use member budget bounds recovery ordinals");
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(recovery));
        bump_until(parser, significant_end);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.use.invalid_group_member",
            SourceRange::new(start, parser.current_offset()),
            "grouped use member must be one name with an optional alias",
        )));
        return PendingUseGroupMember::Recovery {
            source: SourceRange::new(source_start, parser.current_offset()),
            recovery_ordinal: recovery,
        };
    }

    let member_name = *name_ordinal;
    let member_name_kind = match parser.current_kind() {
        Some(SyntaxKind::IdentifierToken) => PendingPathSegmentKind::Identifier,
        Some(SyntaxKind::KeywordToken) => PendingPathSegmentKind::Keyword,
        Some(SyntaxKind::LifetimeToken) => PendingPathSegmentKind::Lifetime,
        _ => unreachable!("group member path token was validated above"),
    };
    *name_ordinal = member_name
        .checked_add(1)
        .expect("grouped-use member budget bounds name ordinals");
    parser.start(SyntaxKind::NameReference, SyntaxRole::Element(member_name));
    parser.bump();
    parser.finish();
    parser.bump_trivia();
    let member_alias = parser.at("as").then(|| {
        let ordinal = *alias_ordinal;
        *alias_ordinal = ordinal
            .checked_add(1)
            .expect("grouped-use member budget bounds alias ordinals");
        aliases.push(emit_alias(
            parser,
            significant_end,
            SyntaxRole::Field(ordinal),
        ));
        ordinal
    });
    let member_recovery = emit_recovery_tail(
        parser,
        significant_end,
        *recovery_ordinal,
        "syntax.use.invalid_group_member",
        "grouped use member must be one name with an optional alias",
    )
    .then(|| {
        let ordinal = *recovery_ordinal;
        *recovery_ordinal = ordinal
            .checked_add(1)
            .expect("grouped-use member budget bounds recovery ordinals");
        ordinal
    });
    PendingUseGroupMember::Binding {
        source: SourceRange::new(source_start, parser.current_offset()),
        name_ordinal: member_name,
        name_kind: member_name_kind,
        alias_ordinal: member_alias,
        recovery_ordinal: member_recovery,
    }
}

fn emit_path_or_glob_use(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
) -> PendingUseProjection {
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
    let mut aliases = Vec::new();
    if let Some(alias) = alias {
        bump_until(parser, alias);
        aliases.push(emit_alias(parser, end, SyntaxRole::Name));
    }
    emit_unexpected_tail(
        parser,
        end,
        "syntax.use.invalid_tree",
        "unexpected token after use tree",
    );
    let kind = star.map_or(PendingUseTreeKind::Path, |star| PendingUseTreeKind::Glob {
        marker: parser.token_at(star).expect("located glob token").range(),
    });
    PendingUseProjection::new(kind, aliases)
}

fn emit_alias(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    name_role: SyntaxRole,
) -> PendingUseAlias {
    debug_assert!(parser.at("as"));
    let start = parser.current_offset();
    parser.bump();
    parser.bump_trivia();
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        parser.start(SyntaxKind::NameDefinition, name_role);
        parser.bump();
        parser.finish();
        return PendingUseAlias::new(SourceRange::new(start, parser.current_offset()));
    }

    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingName, name_role);
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
    PendingUseAlias::new(SourceRange::new(start, parser.current_offset()))
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
        emit_path(
            parser,
            end,
            role,
            super::path::PathSeparatorGrammar::DottedOrQualified,
        );
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
    let owner = parser.start_projected_owner(SyntaxKind::Path, role);
    parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.set_path_projection(
        owner,
        PendingPathProjection::new(PendingPathRoot::ImplicitCrate, Vec::new()),
    );
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
    emit_recovery_tail(parser, end, 0, diagnostic, message);
}

fn emit_recovery_tail(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    recovery_ordinal: u32,
    diagnostic: &'static str,
    message: &'static str,
) -> bool {
    parser.bump_trivia();
    if parser.cursor() >= end {
        return false;
    }
    let start = parser.current_offset();
    parser.start(
        SyntaxKind::ErrorNode,
        SyntaxRole::Recovery(recovery_ordinal),
    );
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        diagnostic,
        SourceRange::new(start, parser.current_offset()),
        message,
    )));
    true
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
