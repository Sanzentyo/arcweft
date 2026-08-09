//! Private retained Layer declaration grammar.

use std::collections::BTreeMap;

use arcweft_id::DeclarationIdentityFamily;
use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::declaration::emit_retained_declaration_header;
use super::expression::emit_expression;
use super::lexer::{LexToken, typed_entity_reference};
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, expected, find_matching_close,
    find_statement_terminator, token_count, trimmed_end,
};
use crate::grammar::budget::GrammarBudget;
use crate::grammar::declaration_projection::{
    PendingLayerAssignment, PendingLayerBodyProjection, PendingLayerColon,
    PendingLayerDeclarationProjection, PendingLayerKind, PendingLayerMemberProjection,
    PendingLayerMemberValue, PendingLayerPolicy, PendingLayerReference,
};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{
    LayerKindSyntaxValue, LayerMemberSyntaxKind, LayerPolicySyntaxValue, SyntaxKind, SyntaxRole,
};
use crate::id_ref::{AuthoredIdRoot, SyntaxIdRefSyntax};

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = DocumentParser::new(source, tokens, events, budget);
    let owner = parser.start_projected_owner(SyntaxKind::LayerDeclarationItem, role);
    let (colon, kind) = emit_retained_declaration_header(
        &mut parser,
        DeclarationIdentityFamily::Layer,
        emit_layer_kind,
    );
    parser.bump_trivia();
    let body = emit_layer_body(&mut parser);
    let trailing_syntax = emit_trailing_recovery(&mut parser);
    parser.set_layer_projection(
        owner,
        PendingLayerDeclarationProjection::new(colon, kind, body, trailing_syntax),
    );
    parser.finish();
}

fn emit_layer_kind(parser: &mut DocumentParser<'_, '_>) -> (PendingLayerColon, PendingLayerKind) {
    parser.start(SyntaxKind::ColonNode, SyntaxRole::Colon);
    let colon = if parser.at(":") {
        let range = parser
            .current()
            .expect("checked Layer header colon")
            .range();
        parser.bump();
        PendingLayerColon::Authored(range)
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
        PendingLayerColon::Missing(SourceRange::new(at, at))
    };
    parser.finish();
    parser.bump_trivia();

    parser.start(SyntaxKind::LayerKindNode, SyntaxRole::Kind);
    let kind = match parser.current_text() {
        Some("background") => Some(LayerKindSyntaxValue::Background),
        Some("world_2d") => Some(LayerKindSyntaxValue::World2d),
        Some("character") => Some(LayerKindSyntaxValue::Character),
        Some("effects") => Some(LayerKindSyntaxValue::Effects),
        Some("dialogue") => Some(LayerKindSyntaxValue::Dialogue),
        Some("game_view") => Some(LayerKindSyntaxValue::GameView),
        Some("html_view") => Some(LayerKindSyntaxValue::HtmlView),
        Some("activity") => Some(LayerKindSyntaxValue::Activity),
        Some("modal") => Some(LayerKindSyntaxValue::Modal),
        Some("overlay") => Some(LayerKindSyntaxValue::Overlay),
        Some("debug") => Some(LayerKindSyntaxValue::Debug),
        Some("agent") => Some(LayerKindSyntaxValue::Agent),
        Some("offscreen") => Some(LayerKindSyntaxValue::Offscreen),
        Some("custom") => Some(LayerKindSyntaxValue::Custom),
        _ => None,
    };
    let state = if let Some(kind) = kind {
        parser.start(SyntaxKind::NameReference, SyntaxRole::LayerKindValue(kind));
        parser.bump();
        parser.finish();
        PendingLayerKind::Authored(kind)
    } else if parser
        .current_kind()
        .is_some_and(|kind| matches!(kind, SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken))
    {
        let token = parser.current().expect("checked unknown Layer kind");
        let is_root = parser.text_of(token) == "root";
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        parser.bump();
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.layer.unknown_kind",
            token.range(),
            if is_root {
                "Layer kind `root` is engine-owned and cannot be authored"
            } else {
                "unknown Layer kind"
            },
        )));
        PendingLayerKind::Unknown
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingMemberValue, SyntaxRole::Recovery(0));
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.layer.missing_kind",
            SourceRange::new(at, at),
            "Layer declaration requires a kind",
        )));
        PendingLayerKind::Missing
    };
    parser.finish();
    (colon, state)
}

fn emit_layer_body(parser: &mut DocumentParser<'_, '_>) -> PendingLayerBodyProjection {
    if !parser.at("{") {
        emit_missing_body(parser);
        return PendingLayerBodyProjection::Missing;
    }

    parser.start(SyntaxKind::LayerBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{");
    let body_end = close.unwrap_or_else(|| token_count(parser));
    let members = emit_layer_members(parser, body_end);
    bump_until(parser, body_end);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.layer.missing_body_close",
    );
    parser.finish();
    PendingLayerBodyProjection::Braced {
        closed: close.is_some(),
        members: members.into_boxed_slice(),
    }
}

fn emit_missing_body(parser: &mut DocumentParser<'_, '_>) {
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

fn emit_layer_members(
    parser: &mut DocumentParser<'_, '_>,
    body_end: usize,
) -> Vec<PendingLayerMemberProjection> {
    let mut first_members = BTreeMap::<LayerMemberSyntaxKind, SourceRange>::new();
    let mut members = Vec::new();
    let mut ordinal = 0_u16;
    while parser.cursor() < body_end {
        parser.bump_trivia();
        if parser.cursor() >= body_end {
            break;
        }
        let entry_end = find_statement_terminator(parser, parser.cursor(), body_end)
            .map_or(body_end, |(end, _)| end);
        let keyword = parser.current().expect("Layer member begins in body");
        let kind = match parser.text_of(keyword) {
            "parent" => Some(LayerMemberSyntaxKind::Parent),
            "phase" => Some(LayerMemberSyntaxKind::Phase),
            "z" => Some(LayerMemberSyntaxKind::Z),
            "visible" => Some(LayerMemberSyntaxKind::Visible),
            "transform" => Some(LayerMemberSyntaxKind::Transform),
            "input" => Some(LayerMemberSyntaxKind::Input),
            "hit_test" => Some(LayerMemberSyntaxKind::HitTest),
            "capture" => Some(LayerMemberSyntaxKind::Capture),
            "accessibility" => Some(LayerMemberSyntaxKind::Accessibility),
            "view" => Some(LayerMemberSyntaxKind::View),
            "activity" => Some(LayerMemberSyntaxKind::Activity),
            _ => None,
        };
        let member = if let Some(kind) = kind {
            emit_layer_member(
                parser,
                entry_end,
                ordinal,
                kind,
                keyword.range(),
                &mut first_members,
            )
        } else {
            emit_unknown_member(parser, entry_end, ordinal, keyword.range())
        };
        members.push(member);
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
    members
}

fn emit_layer_member(
    parser: &mut DocumentParser<'_, '_>,
    entry_end: usize,
    ordinal: u16,
    kind: LayerMemberSyntaxKind,
    keyword_range: SourceRange,
    first_members: &mut BTreeMap<LayerMemberSyntaxKind, SourceRange>,
) -> PendingLayerMemberProjection {
    parser.start(SyntaxKind::LayerMember, SyntaxRole::Member(ordinal));
    let duplicate = if let Some(first) = first_members.get(&kind).copied() {
        parser.push(SyntaxEvent::Diagnostic(
            PendingSyntaxDiagnostic::new(
                "syntax.layer.duplicate_member",
                keyword_range,
                format!("Layer `{}` member may appear only once", kind.spelling()),
            )
            .with_related_range(first),
        ));
        true
    } else {
        first_members.insert(kind, keyword_range);
        false
    };
    parser.start(SyntaxKind::NameReference, SyntaxRole::LayerMemberName(kind));
    parser.bump();
    parser.finish();
    parser.bump_trivia();
    let assignment = emit_assignment(parser);
    parser.bump_trivia();
    let (value, trailing_recovery) = match kind {
        LayerMemberSyntaxKind::Parent => emit_reference_value(
            parser,
            entry_end,
            SyntaxRole::Reference(0),
            DeclarationIdentityFamily::Layer,
        ),
        LayerMemberSyntaxKind::View => emit_reference_value(
            parser,
            entry_end,
            SyntaxRole::Reference(1),
            DeclarationIdentityFamily::View,
        ),
        LayerMemberSyntaxKind::Activity => emit_reference_value(
            parser,
            entry_end,
            SyntaxRole::Reference(2),
            DeclarationIdentityFamily::Activity,
        ),
        LayerMemberSyntaxKind::Phase
        | LayerMemberSyntaxKind::Input
        | LayerMemberSyntaxKind::HitTest
        | LayerMemberSyntaxKind::Capture
        | LayerMemberSyntaxKind::Accessibility => emit_policy_value(parser, entry_end, kind),
        LayerMemberSyntaxKind::Z
        | LayerMemberSyntaxKind::Visible
        | LayerMemberSyntaxKind::Transform => (emit_expression_value(parser, entry_end), false),
    };
    bump_until(parser, entry_end);
    parser.finish();
    PendingLayerMemberProjection::Member {
        source_ordinal: ordinal,
        kind,
        duplicate,
        assignment,
        value,
        trailing_recovery,
    }
}

fn emit_reference_value(
    parser: &mut DocumentParser<'_, '_>,
    entry_end: usize,
    role: SyntaxRole,
    expected_family: DeclarationIdentityFamily,
) -> (PendingLayerMemberValue, bool) {
    if parser.current_kind() != Some(SyntaxKind::EntityReferenceToken) {
        emit_missing_value(
            parser,
            "syntax.layer.missing_reference",
            "Layer reference member requires an entity reference",
        );
        return (PendingLayerMemberValue::Missing, false);
    }
    let token = parser.current().expect("checked Layer reference");
    let reference = typed_entity_reference(token, parser.text_of(token)).into_syntax();
    let wrong_absolute_family = absolute_reference_conflicts_with(&reference, expected_family);
    parser.start(
        if wrong_absolute_family {
            SyntaxKind::WrongFamilyReference
        } else {
            SyntaxKind::RetainedReference
        },
        role,
    );
    parser.bump();
    parser.finish();
    if wrong_absolute_family {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.layer.wrong_reference_family",
            token.range(),
            format!(
                "Layer member requires an @{} reference",
                expected_family.prefix()
            ),
        )));
    }
    let trailing_recovery = reject_extra_member_value(parser, entry_end);
    (
        PendingLayerMemberValue::Reference(PendingLayerReference::new(
            reference,
            wrong_absolute_family,
        )),
        trailing_recovery,
    )
}

fn absolute_reference_conflicts_with(
    reference: &SyntaxIdRefSyntax,
    expected: DeclarationIdentityFamily,
) -> bool {
    let Ok(value) = reference.value() else {
        return false;
    };
    let AuthoredIdRoot::Absolute { .. } = value.root() else {
        return false;
    };
    value
        .segments()
        .first()
        .and_then(|segment| DeclarationIdentityFamily::from_prefix(segment.as_str()))
        != Some(expected)
}

fn emit_policy_value(
    parser: &mut DocumentParser<'_, '_>,
    entry_end: usize,
    member: LayerMemberSyntaxKind,
) -> (PendingLayerMemberValue, bool) {
    if parser
        .current_kind()
        .is_none_or(|kind| !matches!(kind, SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken))
    {
        emit_missing_value(
            parser,
            "syntax.layer.missing_policy_value",
            "Layer policy member requires one policy value",
        );
        return (PendingLayerMemberValue::Missing, false);
    }
    let token = parser.current().expect("checked Layer policy value");
    let value = match (member, parser.text_of(token)) {
        (LayerMemberSyntaxKind::Phase, "background") => {
            Some(LayerPolicySyntaxValue::PhaseBackground)
        }
        (LayerMemberSyntaxKind::Phase, "world") => Some(LayerPolicySyntaxValue::PhaseWorld),
        (LayerMemberSyntaxKind::Phase, "characters") => {
            Some(LayerPolicySyntaxValue::PhaseCharacters)
        }
        (LayerMemberSyntaxKind::Phase, "effects") => Some(LayerPolicySyntaxValue::PhaseEffects),
        (LayerMemberSyntaxKind::Phase, "dialogue") => Some(LayerPolicySyntaxValue::PhaseDialogue),
        (LayerMemberSyntaxKind::Phase, "game_view") => Some(LayerPolicySyntaxValue::PhaseGameView),
        (LayerMemberSyntaxKind::Phase, "html_view") => Some(LayerPolicySyntaxValue::PhaseHtmlView),
        (LayerMemberSyntaxKind::Phase, "modal") => Some(LayerPolicySyntaxValue::PhaseModal),
        (LayerMemberSyntaxKind::Phase, "debug") => Some(LayerPolicySyntaxValue::PhaseDebug),
        (LayerMemberSyntaxKind::Phase, "agent_overlay") => {
            Some(LayerPolicySyntaxValue::PhaseAgentOverlay)
        }
        (LayerMemberSyntaxKind::Input, "ignore") => Some(LayerPolicySyntaxValue::InputIgnore),
        (LayerMemberSyntaxKind::Input, "pass_through") => {
            Some(LayerPolicySyntaxValue::InputPassThrough)
        }
        (LayerMemberSyntaxKind::Input, "hit_test") => Some(LayerPolicySyntaxValue::InputHitTest),
        (LayerMemberSyntaxKind::Input, "modal") => Some(LayerPolicySyntaxValue::InputModal),
        (LayerMemberSyntaxKind::Input, "capture") => Some(LayerPolicySyntaxValue::InputCapture),
        (LayerMemberSyntaxKind::HitTest, "none") => Some(LayerPolicySyntaxValue::HitTestNone),
        (LayerMemberSyntaxKind::HitTest, "bounds") => Some(LayerPolicySyntaxValue::HitTestBounds),
        (LayerMemberSyntaxKind::HitTest, "view_tree") => {
            Some(LayerPolicySyntaxValue::HitTestViewTree)
        }
        (LayerMemberSyntaxKind::HitTest, "object_id_mask") => {
            Some(LayerPolicySyntaxValue::HitTestObjectIdMask)
        }
        (LayerMemberSyntaxKind::Capture, "none") => Some(LayerPolicySyntaxValue::CaptureNone),
        (LayerMemberSyntaxKind::Capture, "color") => Some(LayerPolicySyntaxValue::CaptureColor),
        (LayerMemberSyntaxKind::Capture, "object_id") => {
            Some(LayerPolicySyntaxValue::CaptureObjectId)
        }
        (LayerMemberSyntaxKind::Capture, "mask") => Some(LayerPolicySyntaxValue::CaptureMask),
        (LayerMemberSyntaxKind::Capture, "all") => Some(LayerPolicySyntaxValue::CaptureAll),
        (LayerMemberSyntaxKind::Accessibility, "hidden") => {
            Some(LayerPolicySyntaxValue::AccessibilityHidden)
        }
        (LayerMemberSyntaxKind::Accessibility, "exposed") => {
            Some(LayerPolicySyntaxValue::AccessibilityExposed)
        }
        (LayerMemberSyntaxKind::Accessibility, "container") => {
            Some(LayerPolicySyntaxValue::AccessibilityContainer)
        }
        _ => None,
    };
    parser.start(SyntaxKind::LayerPolicyValue, SyntaxRole::Policy(0));
    let state = if let Some(value) = value {
        parser.start(
            SyntaxKind::NameReference,
            SyntaxRole::LayerPolicyValue(value),
        );
        parser.bump();
        parser.finish();
        PendingLayerPolicy::Authored(value)
    } else {
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        parser.bump();
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.layer.unknown_policy",
            token.range(),
            format!("unknown `{}` policy value", member.spelling()),
        )));
        PendingLayerPolicy::Unknown
    };
    parser.finish();
    let trailing_recovery = reject_extra_member_value(parser, entry_end);
    (PendingLayerMemberValue::Policy(state), trailing_recovery)
}

fn emit_expression_value(
    parser: &mut DocumentParser<'_, '_>,
    entry_end: usize,
) -> PendingLayerMemberValue {
    let expression_end = trimmed_end(parser, parser.cursor(), entry_end);
    if parser.cursor() == expression_end {
        emit_missing_value(
            parser,
            "syntax.layer.missing_expression",
            "Layer member requires an expression value",
        );
        PendingLayerMemberValue::Missing
    } else {
        emit_expression(parser, expression_end, SyntaxRole::Initializer);
        PendingLayerMemberValue::Expression
    }
}

fn reject_extra_member_value(parser: &mut DocumentParser<'_, '_>, entry_end: usize) -> bool {
    parser.bump_trivia();
    if parser.cursor() >= trimmed_end(parser, parser.cursor(), entry_end) {
        return false;
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
    true
}

fn emit_unknown_member(
    parser: &mut DocumentParser<'_, '_>,
    entry_end: usize,
    ordinal: u16,
    range: SourceRange,
) -> PendingLayerMemberProjection {
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
    PendingLayerMemberProjection::Recovery {
        source_ordinal: ordinal,
    }
}

fn emit_assignment(parser: &mut DocumentParser<'_, '_>) -> PendingLayerAssignment {
    parser.start(SyntaxKind::EqualsNode, SyntaxRole::Equals);
    let assignment = if parser.at("=") {
        let range = parser
            .current()
            .expect("checked Layer assignment token")
            .range();
        parser.bump();
        PendingLayerAssignment::Authored(range)
    } else {
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
        PendingLayerAssignment::Missing(SourceRange::new(at, at))
    };
    parser.finish();
    assignment
}

fn emit_missing_value(
    parser: &mut DocumentParser<'_, '_>,
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

fn emit_trailing_recovery(parser: &mut DocumentParser<'_, '_>) -> bool {
    parser.bump_trivia();
    if parser.is_at_end() {
        return false;
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
    true
}
