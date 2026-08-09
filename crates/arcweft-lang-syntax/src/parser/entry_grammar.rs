//! Private source-entry grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::declaration::{emit_outer_prefixes, emit_visibility};
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
use crate::grammar::entry_projection::{
    EntryRoleSyntaxKind, KnownEntryHttpMethod, KnownEntryKind, PendingEntryBodyProjection,
    PendingEntryDeclarationProjection, PendingEntryHttpMethod, PendingEntryId, PendingEntryKind,
    PendingEntryMemberProjection, PendingEntryName, PendingEntryPunctuation,
    PendingEntryRouteBinding, PendingEntryRouteBindings, PendingEntryValueState,
};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::id_ref::AuthoredIdRoot;
use crate::name::SyntaxName;

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = DocumentParser::new(source, tokens, events, budget);
    let owner = parser.start_projected_owner(SyntaxKind::EntryDeclarationItem, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();

    if parser.at("entry") {
        parser.bump();
    }
    parser.bump_trivia();
    let kind = emit_entry_kind(&mut parser);
    parser.bump_trivia();
    let id = emit_entry_id(&mut parser);
    parser.bump_trivia();
    let trailing_header_recovery = recover_header_tail(&mut parser);
    let body = emit_entry_body(&mut parser, source);

    parser.set_entry_projection(
        owner,
        PendingEntryDeclarationProjection {
            kind,
            id,
            trailing_header_recovery,
            body,
        },
    );

    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_entry_kind(parser: &mut DocumentParser<'_, '_>) -> PendingEntryKind {
    if matches!(
        parser.current_kind(),
        Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
    ) {
        let token = parser
            .current()
            .expect("entry kind dispatch retains one token");
        let source = token.range();
        let spelling = parser.text_of(token);
        let projected = KnownEntryKind::from_source_name(spelling).map_or_else(
            || PendingEntryKind::Custom {
                value: SyntaxName::try_new(spelling)
                    .expect("entry kind token is an identifier or keyword"),
                source,
            },
            |value| PendingEntryKind::Known { value, source },
        );
        parser.start(SyntaxKind::NameReference, SyntaxRole::Type);
        parser.bump();
        parser.finish();
        return projected;
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
    PendingEntryKind::Missing {
        insertion: SourceRange::new(at, at),
    }
}

fn emit_entry_id(parser: &mut DocumentParser<'_, '_>) -> PendingEntryId {
    let Some(token) = parser
        .current()
        .filter(|token| token.kind() == SyntaxKind::EntityReferenceToken)
    else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingExpression, SyntaxRole::Reference(0));
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
        return PendingEntryId::Missing {
            insertion: SourceRange::new(at, at),
        };
    };

    let range = token.range();
    let (_, reference) = super::expression::emit_entity_reference(parser, SyntaxRole::Reference(0));
    let valid_family = reference.value().is_ok_and(|reference| {
        matches!(
            reference.root(),
            AuthoredIdRoot::Absolute { delimited: false }
        ) && matches!(reference.segments(), [family, ..] if family.as_str() == "entry")
    });
    if !valid_family {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.id_family",
            range,
            "entry declaration IDs must use the `entry` family",
        )));
    }
    PendingEntryId::Authored {
        source: range,
        canonical_entry_family: valid_family,
    }
}

fn recover_header_tail(parser: &mut DocumentParser<'_, '_>) -> bool {
    if parser.at("{") || parser.is_at_end() {
        return false;
    }

    let start = parser.current_offset();
    let end = find_top_level_boundary(parser, parser.cursor(), token_count(parser), &["{"]);
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
    true
}

fn emit_entry_body(
    parser: &mut DocumentParser<'_, '_>,
    source: &str,
) -> PendingEntryBodyProjection {
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
        return PendingEntryBodyProjection::Missing;
    }

    parser.start(SyntaxKind::EntryBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = token_count(parser);
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(end);
    parser.start(SyntaxKind::ItemList, SyntaxRole::Element(0));
    let members = emit_entry_members(parser, source, close);
    bump_until(parser, close);
    parser.finish();
    let closed = parser.at("}");
    if closed {
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
    PendingEntryBodyProjection::Braced {
        members: members.into_boxed_slice(),
        closed,
    }
}

fn emit_entry_members(
    parser: &mut DocumentParser<'_, '_>,
    source: &str,
    close: usize,
) -> Vec<PendingEntryMemberProjection> {
    let mut ordinal = 0_u32;
    let mut members = Vec::new();
    while parser.cursor() < close {
        bump_member_separators(parser, close);
        if parser.cursor() >= close {
            break;
        }

        let start = parser.cursor();
        let end = entry_member_boundary(parser, source, start, close);
        let spelling = parser.current_text();
        let member = if let Some(role) = spelling.and_then(EntryRoleSyntaxKind::from_source_name) {
            emit_role_binding(parser, end, ordinal, role)
        } else {
            match spelling {
                Some("goto") => emit_goto(parser, end, ordinal),
                Some("route") => emit_route(parser, end, ordinal),
                _ if entry_option_equals(parser, start, end).is_some() => {
                    emit_option(parser, end, ordinal)
                }
                _ => emit_invalid_member(parser, end, ordinal),
            }
        };
        members.push(member);
        bump_until(parser, end);
        if parser.cursor() == start {
            parser.bump();
        }
        ordinal = ordinal.saturating_add(1);
    }
    members
}

fn bump_member_separators(parser: &mut DocumentParser<'_, '_>, end: usize) {
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
    parser: &DocumentParser<'_, '_>,
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
    parser: &DocumentParser<'_, '_>,
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
    if EntryRoleSyntaxKind::from_source_name(spelling).is_some()
        || matches!(spelling, "goto" | "route")
    {
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
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u32,
    role: EntryRoleSyntaxKind,
) -> PendingEntryMemberProjection {
    parser.start(SyntaxKind::EntryRoleBinding, SyntaxRole::Element(ordinal));
    emit_current_name(parser, SyntaxRole::Name);
    bump_trivia_before(parser, end);
    let assignment = if parser.at("=") {
        let range = parser
            .current()
            .expect("assignment token is present")
            .range();
        parser.bump();
        PendingEntryPunctuation::Authored(range)
    } else {
        let at = parser.current_offset();
        emit_missing_punctuation(parser, SyntaxRole::Recovery(0));
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.role_binding",
            SourceRange::new(parser.current_offset(), parser.current_offset()),
            "entry role binding requires `=` before its value",
        )));
        PendingEntryPunctuation::Missing(SourceRange::new(at, at))
    };
    bump_trivia_before(parser, end);

    let (value, trailing_recovery) = if role.expects_type() {
        let missing = parser.cursor() >= end;
        let projection = emit_type(parser, end, SyntaxRole::Type);
        if missing {
            let at = parser.current_offset();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.entry.role_value",
                SourceRange::new(at, at),
                "entry role requires a value",
            )));
        }
        (
            if missing {
                PendingEntryValueState::Missing
            } else if matches!(
                projection.authored().value_at(projection.path()),
                Some(crate::types::TypeRef::Recovery(_))
            ) {
                PendingEntryValueState::Invalid
            } else {
                PendingEntryValueState::Authored
            },
            false,
        )
    } else {
        emit_required_path(parser, end)
    };
    bump_until(parser, end);
    parser.finish();
    PendingEntryMemberProjection::Role {
        source_ordinal: ordinal,
        role,
        assignment,
        value,
        trailing_recovery,
    }
}

fn emit_required_path(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
) -> (PendingEntryValueState, bool) {
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
        return (PendingEntryValueState::Missing, false);
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
        return (PendingEntryValueState::Invalid, false);
    }

    emit_path(
        parser,
        end,
        SyntaxRole::Initializer,
        super::path::PathSeparatorGrammar::DottedOrQualified,
    );
    let Some(remainder) = first_significant(parser, parser.cursor(), end) else {
        bump_until(parser, end);
        return (PendingEntryValueState::Authored, false);
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
    (PendingEntryValueState::Authored, true)
}

fn emit_goto(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u32,
) -> PendingEntryMemberProjection {
    parser.start(SyntaxKind::EntryGoto, SyntaxRole::Element(ordinal));
    parser.bump();
    bump_trivia_before(parser, end);
    let target_start = parser.cursor();
    let valid = parser.current_kind() == Some(SyntaxKind::EntityReferenceToken)
        && first_significant(parser, target_start + 1, end).is_none();
    let target = if target_start >= end {
        emit_missing_entity_reference(parser, SyntaxRole::Target, "syntax.entry.goto_target");
        PendingEntryValueState::Missing
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
        if valid {
            PendingEntryValueState::Authored
        } else {
            PendingEntryValueState::Invalid
        }
    };
    bump_until(parser, end);
    parser.finish();
    PendingEntryMemberProjection::Goto {
        source_ordinal: ordinal,
        target,
        trailing_recovery: false,
    }
}

fn emit_route(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u32,
) -> PendingEntryMemberProjection {
    parser.start(SyntaxKind::EntryRoute, SyntaxRole::Element(ordinal));
    parser.bump();
    bump_trivia_before(parser, end);
    let method = emit_route_method(parser, end);
    bump_trivia_before(parser, end);
    let path = emit_route_path(parser, end);
    bump_trivia_before(parser, end);
    let arrow = emit_route_arrow(parser);
    bump_trivia_before(parser, end);
    let target = emit_route_target(parser, end);
    bump_trivia_before(parser, end);
    let bindings = if parser.at("(") {
        let bindings = emit_route_bindings(parser, end);
        bump_trivia_before(parser, end);
        bindings
    } else {
        PendingEntryRouteBindings::Absent
    };
    let trailing_recovery = parser.cursor() < end;
    if trailing_recovery {
        let range = token_range(parser, parser.cursor(), end);
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(1));
        bump_until(parser, end);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.route_tail",
            range,
            "unexpected syntax after the entry route target",
        )));
    }
    parser.finish();
    PendingEntryMemberProjection::Route {
        source_ordinal: ordinal,
        method,
        path,
        arrow,
        target,
        bindings,
        trailing_recovery,
    }
}

fn emit_route_method(parser: &mut DocumentParser<'_, '_>, end: usize) -> PendingEntryHttpMethod {
    if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        )
    {
        let token = parser
            .current()
            .expect("route method dispatch retains one token");
        let source = token.range();
        let spelling = parser.text_of(token);
        let projected = KnownEntryHttpMethod::from_source_name(spelling).map_or_else(
            || PendingEntryHttpMethod::Unsupported {
                value: SyntaxName::try_new(spelling),
                source,
            },
            |value| PendingEntryHttpMethod::Known { value, source },
        );
        emit_current_name(parser, SyntaxRole::Name);
        return projected;
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
    PendingEntryHttpMethod::Missing {
        insertion: SourceRange::new(at, at),
    }
}

fn emit_route_path(parser: &mut DocumentParser<'_, '_>, end: usize) -> PendingEntryValueState {
    if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::StringToken | SyntaxKind::RawStringToken)
        )
    {
        super::expression::emit_literal(parser, SyntaxRole::Operand);
        return PendingEntryValueState::Authored;
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
    if parser.cursor() < end {
        PendingEntryValueState::Invalid
    } else {
        PendingEntryValueState::Missing
    }
}

fn emit_route_arrow(parser: &mut DocumentParser<'_, '_>) -> PendingEntryPunctuation {
    if parser.at("->") {
        let range = parser.current().expect("route arrow is present").range();
        parser.bump();
        return PendingEntryPunctuation::Authored(range);
    }

    let at = parser.current_offset();
    emit_missing_punctuation(parser, SyntaxRole::Recovery(0));
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.entry.route_arrow",
        SourceRange::new(at, at),
        "entry route requires `->` before its flow target",
    )));
    PendingEntryPunctuation::Missing(SourceRange::new(at, at))
}

fn emit_route_target(parser: &mut DocumentParser<'_, '_>, end: usize) -> PendingEntryValueState {
    if parser.cursor() < end && parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        super::expression::emit_entity_reference(parser, SyntaxRole::Target);
        return PendingEntryValueState::Authored;
    }
    emit_missing_entity_reference(parser, SyntaxRole::Target, "syntax.entry.route_target");
    if parser.cursor() < end {
        PendingEntryValueState::Invalid
    } else {
        PendingEntryValueState::Missing
    }
}

fn emit_route_bindings(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
) -> PendingEntryRouteBindings {
    parser.start(SyntaxKind::DelimitedGroup, SyntaxRole::Argument(0));
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    let close = find_matching_close(parser, parser.cursor(), "(")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::ArgumentList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    let mut bindings = Vec::new();
    while parser.cursor() < close {
        bump_trivia_before(parser, close);
        if parser.cursor() >= close {
            break;
        }
        let binding_end = find_top_level_boundary(parser, parser.cursor(), close, &[",", ")"]);
        bindings.push(emit_route_binding(parser, binding_end, ordinal));
        bump_until(parser, binding_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    let closed = parser.at(")");
    if closed {
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
    PendingEntryRouteBindings::Parenthesized {
        bindings: bindings.into_boxed_slice(),
        closed,
    }
}

fn emit_route_binding(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u16,
) -> PendingEntryRouteBinding {
    parser.start(SyntaxKind::EntryRouteBinding, SyntaxRole::Argument(ordinal));
    let parameter = if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        ) {
        emit_projected_current_name(parser, SyntaxRole::Name)
    } else {
        emit_projected_missing_name(parser, SyntaxRole::Name)
    };
    bump_trivia_before(parser, end);
    let equals = if parser.at("=") {
        let range = parser
            .current()
            .expect("route binding equals is present")
            .range();
        parser.bump();
        PendingEntryPunctuation::Authored(range)
    } else {
        let at = parser.current_offset();
        emit_missing_punctuation(parser, SyntaxRole::Recovery(0));
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.route_binding",
            SourceRange::new(parser.current_offset(), parser.current_offset()),
            "entry route binding requires `=`",
        )));
        PendingEntryPunctuation::Missing(SourceRange::new(at, at))
    };
    bump_trivia_before(parser, end);
    let colon = if parser.at(":") {
        let range = parser
            .current()
            .expect("route binding colon is present")
            .range();
        parser.bump();
        PendingEntryPunctuation::Authored(range)
    } else {
        let at = parser.current_offset();
        emit_missing_punctuation(parser, SyntaxRole::Recovery(1));
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.entry.route_binding",
            SourceRange::new(parser.current_offset(), parser.current_offset()),
            "entry route binding values must name a `:path_param`",
        )));
        PendingEntryPunctuation::Missing(SourceRange::new(at, at))
    };
    bump_trivia_before(parser, end);
    let capture = if parser.cursor() < end
        && matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        ) {
        emit_projected_current_name(parser, SyntaxRole::Initializer)
    } else {
        emit_projected_missing_name(parser, SyntaxRole::Initializer)
    };
    bump_trivia_before(parser, end);
    let trailing_recovery = parser.cursor() < end;
    if trailing_recovery {
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
    PendingEntryRouteBinding {
        source_ordinal: ordinal,
        parameter,
        equals,
        colon,
        capture,
        trailing_recovery,
    }
}

fn emit_option(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u32,
) -> PendingEntryMemberProjection {
    let equals = entry_option_equals(parser, parser.cursor(), end)
        .expect("entry option dispatch requires a top-level equals token");
    parser.start(SyntaxKind::EntryOption, SyntaxRole::Element(ordinal));
    let name = emit_projected_current_name(parser, SyntaxRole::Name);
    bump_trivia_before(parser, equals);
    let trailing_recovery = parser.cursor() < equals;
    if trailing_recovery {
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
    let assignment_range = parser
        .current()
        .expect("entry option equals is present")
        .range();
    parser.bump();
    bump_trivia_before(parser, end);
    let value = if parser.cursor() >= end {
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
        PendingEntryValueState::Missing
    } else {
        emit_expression(parser, end, SyntaxRole::Initializer);
        PendingEntryValueState::Authored
    };
    bump_until(parser, end);
    parser.finish();
    PendingEntryMemberProjection::Option {
        source_ordinal: ordinal,
        name,
        assignment: PendingEntryPunctuation::Authored(assignment_range),
        value,
        trailing_recovery,
    }
}

fn emit_invalid_member(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u32,
) -> PendingEntryMemberProjection {
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
    PendingEntryMemberProjection::Recovery {
        source_ordinal: ordinal,
    }
}

fn entry_option_equals(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> Option<usize> {
    let first = parser.token_at(start)?;
    if !matches!(
        first.kind(),
        SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
    ) {
        return None;
    }
    let equals = find_top_level_boundary(parser, start + 1, token_count(parser), &["="]);
    (equals < end).then_some(equals)
}

fn emit_current_name(parser: &mut DocumentParser<'_, '_>, role: SyntaxRole) {
    parser.start(SyntaxKind::NameReference, role);
    parser.bump();
    parser.finish();
}

fn emit_projected_current_name(
    parser: &mut DocumentParser<'_, '_>,
    role: SyntaxRole,
) -> PendingEntryName {
    let token = parser.current().expect("name dispatch retains one token");
    let source = token.range();
    let value = SyntaxName::try_new(parser.text_of(token));
    emit_current_name(parser, role);
    PendingEntryName::Authored { value, source }
}

fn emit_missing_name(parser: &mut DocumentParser<'_, '_>, role: SyntaxRole) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingName, role);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
}

fn emit_projected_missing_name(
    parser: &mut DocumentParser<'_, '_>,
    role: SyntaxRole,
) -> PendingEntryName {
    let at = parser.current_offset();
    emit_missing_name(parser, role);
    PendingEntryName::Missing {
        insertion: SourceRange::new(at, at),
    }
}

fn emit_missing_entity_reference(
    parser: &mut DocumentParser<'_, '_>,
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

fn emit_missing_punctuation(parser: &mut DocumentParser<'_, '_>, role: SyntaxRole) {
    parser.start(SyntaxKind::MissingTokenNode, role);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::PunctuationToken),
        at: parser.current_offset(),
    });
    parser.finish();
}

fn bump_trivia_before(parser: &mut DocumentParser<'_, '_>, end: usize) {
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

fn token_range(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> SourceRange {
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
