//! Private retained Layer declaration grammar.

use std::collections::BTreeMap;

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
    parser.start(SyntaxKind::LayerDeclarationItem, role);
    emit_retained_declaration_header(&mut parser, RetainedIdentityFamily::Layer, emit_layer_kind);
    parser.bump_trivia();
    emit_layer_body(&mut parser);
    emit_trailing_recovery(&mut parser);
    parser.finish();
}

fn emit_layer_kind(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at(":") {
        parser.bump();
    } else {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.layer.missing_colon",
            SourceRange::new(at, at),
            "Layer declaration requires `: LayerKind`",
        )));
    }
    parser.bump_trivia();
    parser.start(SyntaxKind::LayerKindNode, SyntaxRole::Kind);
    let Some(token) = parser.current() else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingMemberValue, SyntaxRole::Recovery(0));
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.layer.missing_kind",
            SourceRange::new(at, at),
            "Layer declaration requires a kind",
        )));
        parser.finish();
        return;
    };
    let kind = parser.text_of(token);
    if kind == "root" || !is_layer_kind(kind) {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.layer.unknown_kind",
            token.range(),
            if kind == "root" {
                "Layer kind `root` is engine-owned and cannot be authored"
            } else {
                "unknown Layer kind"
            },
        )));
    }
    parser.bump();
    parser.finish();
}

fn is_layer_kind(kind: &str) -> bool {
    matches!(
        kind,
        "background"
            | "world_2d"
            | "character"
            | "effects"
            | "dialogue"
            | "game_view"
            | "html_view"
            | "activity"
            | "modal"
            | "overlay"
            | "debug"
            | "agent"
            | "offscreen"
            | "custom"
    )
}

fn emit_layer_body(parser: &mut ShadowDocumentParser<'_, '_>) {
    if !parser.at("{") {
        emit_missing_body(parser);
        return;
    }

    parser.start(SyntaxKind::LayerBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{");
    let body_end = close.unwrap_or_else(|| token_count(parser));
    emit_layer_members(parser, body_end);
    bump_until(parser, body_end);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.layer.missing_body_close",
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
        "syntax.layer.missing_body",
        SourceRange::new(at, at),
        "Layer declaration requires a braced policy body",
    )));
}

fn emit_layer_members(parser: &mut ShadowDocumentParser<'_, '_>, body_end: usize) {
    let mut first_members = BTreeMap::<String, SourceRange>::new();
    let mut ordinal = 0_u16;
    while parser.cursor() < body_end {
        parser.bump_trivia();
        if parser.cursor() >= body_end {
            break;
        }
        let entry_end = find_statement_terminator(parser, parser.cursor(), body_end)
            .map_or(body_end, |(end, _)| end);
        let keyword = parser.current().expect("Layer member begins in body");
        let member = parser.text_of(keyword);
        if is_layer_member(member) {
            emit_layer_member(
                parser,
                entry_end,
                ordinal,
                member,
                keyword.range(),
                &mut first_members,
            );
        } else {
            emit_unknown_member(parser, entry_end, ordinal, keyword.range());
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
            .expect("Layer member budget is below the role index range");
    }
}

fn is_layer_member(member: &str) -> bool {
    matches!(
        member,
        "parent"
            | "phase"
            | "z"
            | "visible"
            | "transform"
            | "input"
            | "hit_test"
            | "capture"
            | "accessibility"
            | "view"
            | "activity"
    )
}

fn emit_layer_member(
    parser: &mut ShadowDocumentParser<'_, '_>,
    entry_end: usize,
    ordinal: u16,
    member: &str,
    keyword_range: SourceRange,
    first_members: &mut BTreeMap<String, SourceRange>,
) {
    parser.start(SyntaxKind::LayerMember, SyntaxRole::Member(ordinal));
    if let Some(first) = first_members.get(member).copied() {
        parser.push(SyntaxEvent::Diagnostic(
            PendingSyntaxDiagnostic::new(
                "syntax.layer.duplicate_member",
                keyword_range,
                format!("Layer `{member}` member may appear only once"),
            )
            .with_related_range(first),
        ));
    } else {
        first_members.insert(member.to_owned(), keyword_range);
    }
    parser.bump();
    parser.bump_trivia();
    emit_assignment(parser);
    parser.bump_trivia();
    match member {
        "parent" | "view" | "activity" => {
            emit_reference_value(
                parser,
                entry_end,
                reference_role(member),
                reference_family(member),
            );
        }
        "phase" | "input" | "hit_test" | "capture" | "accessibility" => {
            emit_policy_value(parser, entry_end, member);
        }
        "z" | "visible" | "transform" => emit_expression_value(parser, entry_end),
        _ => unreachable!("recognized Layer member has a value grammar"),
    }
    bump_until(parser, entry_end);
    parser.finish();
}

fn reference_role(member: &str) -> SyntaxRole {
    match member {
        "parent" => SyntaxRole::Reference(0),
        "view" => SyntaxRole::Reference(1),
        "activity" => SyntaxRole::Reference(2),
        _ => unreachable!(),
    }
}

fn reference_family(member: &str) -> RetainedIdentityFamily {
    match member {
        "parent" => RetainedIdentityFamily::Layer,
        "view" => RetainedIdentityFamily::View,
        "activity" => RetainedIdentityFamily::Activity,
        _ => unreachable!(),
    }
}

fn emit_reference_value(
    parser: &mut ShadowDocumentParser<'_, '_>,
    entry_end: usize,
    role: SyntaxRole,
    expected_family: RetainedIdentityFamily,
) {
    if parser.current_kind() != Some(SyntaxKind::EntityReferenceToken) {
        emit_missing_value(
            parser,
            "syntax.layer.missing_reference",
            "Layer reference member requires an entity reference",
        );
        return;
    }
    let token = parser.current().expect("checked Layer reference");
    let actual_family = retained_reference_family(parser.text_of(token));
    parser.start(
        if actual_family == Some(expected_family) {
            SyntaxKind::RetainedReference
        } else {
            SyntaxKind::WrongFamilyReference
        },
        role,
    );
    parser.bump();
    parser.finish();
    if actual_family != Some(expected_family) {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.layer.wrong_reference_family",
            token.range(),
            format!(
                "Layer member requires an @{} reference",
                expected_family.prefix()
            ),
        )));
    }
    reject_extra_member_value(parser, entry_end);
}

fn retained_reference_family(reference: &str) -> Option<RetainedIdentityFamily> {
    let value = reference.strip_prefix('@')?;
    let prefix_end = value.find(['.', ':']).unwrap_or(value.len());
    match &value[..prefix_end] {
        "layer" => Some(RetainedIdentityFamily::Layer),
        "view" => Some(RetainedIdentityFamily::View),
        "activity" => Some(RetainedIdentityFamily::Activity),
        _ => None,
    }
}

fn emit_policy_value(parser: &mut ShadowDocumentParser<'_, '_>, entry_end: usize, member: &str) {
    if parser
        .current_kind()
        .is_none_or(|kind| !matches!(kind, SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken))
    {
        emit_missing_value(
            parser,
            "syntax.layer.missing_policy_value",
            "Layer policy member requires one policy value",
        );
        return;
    }
    let token = parser.current().expect("checked Layer policy value");
    let value = parser.text_of(token);
    parser.start(SyntaxKind::LayerPolicyValue, SyntaxRole::Policy(0));
    parser.bump();
    parser.finish();
    if !policy_value_is_valid(member, value) {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.layer.unknown_policy",
            token.range(),
            format!("unknown `{member}` policy value"),
        )));
    }
    reject_extra_member_value(parser, entry_end);
}

fn policy_value_is_valid(member: &str, value: &str) -> bool {
    match member {
        "input" => matches!(
            value,
            "ignore" | "pass_through" | "hit_test" | "modal" | "capture"
        ),
        "hit_test" => matches!(value, "none" | "bounds" | "view_tree" | "object_id_mask"),
        "capture" => matches!(value, "none" | "color" | "object_id" | "mask" | "all"),
        "accessibility" => matches!(value, "hidden" | "exposed" | "container"),
        "phase" => true,
        _ => unreachable!("policy member"),
    }
}

fn emit_expression_value(parser: &mut ShadowDocumentParser<'_, '_>, entry_end: usize) {
    let expression_end = trimmed_end(parser, parser.cursor(), entry_end);
    if parser.cursor() == expression_end {
        emit_missing_value(
            parser,
            "syntax.layer.missing_expression",
            "Layer member requires an expression value",
        );
    } else {
        emit_expression(parser, expression_end, SyntaxRole::Initializer);
    }
}

fn reject_extra_member_value(parser: &mut ShadowDocumentParser<'_, '_>, entry_end: usize) {
    parser.bump_trivia();
    if parser.cursor() >= trimmed_end(parser, parser.cursor(), entry_end) {
        return;
    }
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, entry_end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.layer.trailing_member_value",
        SourceRange::new(start, parser.current_offset()),
        "Layer reference and policy members accept one value",
    )));
}

fn emit_unknown_member(
    parser: &mut ShadowDocumentParser<'_, '_>,
    entry_end: usize,
    ordinal: u16,
    range: SourceRange,
) {
    parser.start(
        SyntaxKind::ErrorDeclarationMember,
        SyntaxRole::Member(ordinal),
    );
    bump_until(parser, entry_end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.layer.unknown_member",
        range,
        "unknown Layer member",
    )));
}

fn emit_assignment(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("=") {
        parser.bump();
        return;
    }
    let at = parser.current_offset();
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::PunctuationToken),
        at,
    });
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.layer.missing_assignment",
        SourceRange::new(at, at),
        "Layer member requires `=`",
    )));
}

fn emit_missing_value(
    parser: &mut ShadowDocumentParser<'_, '_>,
    code: &'static str,
    message: &'static str,
) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingMemberValue, SyntaxRole::Initializer);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(at, at),
        message,
    )));
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
        "unexpected syntax after Layer declaration body",
    )));
}
