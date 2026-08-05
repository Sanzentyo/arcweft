//! Attached `source` declaration grammar over the shared document cursor.

use arcweft_id::DeclarationIdentityFamily;
use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use super::declaration::{emit_contract_clause_until, emit_outer_prefixes, emit_visibility};
use super::expression::emit_expression_node;
use super::lexer::{LexToken, typed_entity_reference};
use super::pattern::emit_pattern;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, find_statement_terminator, find_top_level_boundary, first_significant,
    token_count, token_text, trimmed_end,
};
use super::statement::{emit_braced_statement_block_until, emit_statement_fragment};
use super::type_ref::emit_type;
use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, PendingExpressionProjection,
    SyntaxCallArgumentListTerminator, SyntaxCallArgumentPart, SyntaxCallArgumentProjection,
    SyntaxCallProjection,
};
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::source_declaration_projection::{
    PendingSourceBackpressurePolicy, PendingSourceBodyProjection, PendingSourceBoundedArgument,
    PendingSourceChildState, PendingSourceDeclarationProjection, PendingSourceHandlerBody,
    PendingSourceHandlerEvent, PendingSourceId, PendingSourceMemberProjection, PendingSourceName,
    PendingSourceNamedPolicy, PendingSourceOverflowPolicy, PendingSourcePunctuation,
    PendingSourceTypeState, SourceContractSyntaxKind, SourcePrivacySyntaxKind,
    SourceReplaySyntaxKind,
};
use crate::id_ref::SyntaxIdRefIssue;
use crate::name::SyntaxName;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceIdProblem {
    WrongFamily,
    Malformed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceIdEmission {
    requires_name: bool,
    consumed_type_colon: bool,
    projection: PendingSourceId,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SourceBodyLedger {
    source_ordinal: u32,
    statement_ordinal: u32,
    contract_ordinal: u16,
    requires: u16,
    ensures: u16,
    saw_ensures: bool,
    saw_from: bool,
    saw_backpressure: bool,
    saw_replay: bool,
    saw_privacy: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceHandlerHead {
    Item,
    Error,
    Progress,
    Disconnected,
    PermissionRevoked,
    End,
    Unknown,
}

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = ShadowDocumentParser::new(source, tokens, events, budget);
    let owner = parser.start_projected_owner(SyntaxKind::SourceItem, role);
    parser.start(SyntaxKind::DeclarationHeader, SyntaxRole::Element(0));
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();

    if parser.at("source") {
        parser.bump();
    }
    parser.bump_trivia();
    let public_id = emit_public_id(&mut parser);
    parser.bump_trivia();
    let name = match public_id.as_ref() {
        None => emit_required_name(&mut parser),
        Some(emission) if emission.requires_name && emission.consumed_type_colon => {
            emit_missing_name(&mut parser)
        }
        Some(emission) if emission.requires_name => emit_required_name(&mut parser),
        Some(emission) if !emission.consumed_type_colon => emit_optional_name(&mut parser),
        Some(_) => PendingSourceName::Absent,
    };
    parser.bump_trivia();
    let (source_type, missing_type_colon) = emit_source_type(
        &mut parser,
        public_id
            .as_ref()
            .is_some_and(|emission| emission.consumed_type_colon),
    );
    parser.finish();
    let body = emit_source_body(&mut parser);
    while parser.bump().is_some() {}
    parser.set_source_declaration_projection(
        owner,
        PendingSourceDeclarationProjection::new(
            public_id.map_or(PendingSourceId::Absent, |emission| emission.projection),
            name,
            source_type,
            missing_type_colon,
            body,
        ),
    );
    parser.finish();
}

fn emit_public_id(parser: &mut ShadowDocumentParser<'_, '_>) -> Option<SourceIdEmission> {
    if parser.current_kind() != Some(SyntaxKind::EntityReferenceToken) {
        return None;
    }

    let token = parser
        .current()
        .expect("checked source declaration ID token");
    let spelling = parser.text_of(token);
    let trailing_type_colon = spelling.ends_with(':');
    let id_range = if trailing_type_colon {
        SourceRange::new(token.range().start(), token.range().end().saturating_sub(1))
    } else {
        token.range()
    };
    let id_spelling = if trailing_type_colon {
        &spelling[..spelling.len().saturating_sub(1)]
    } else {
        spelling
    };
    let typed_projection = typed_entity_reference(
        LexToken {
            kind: SyntaxKind::EntityReferenceToken,
            range: id_range,
        },
        id_spelling,
    );
    let marker_family = typed_projection.empty_marker_family().cloned();
    let typed = typed_projection.into_syntax();
    let malformed_delimited_absolute = id_spelling.starts_with("@<") && !id_spelling.ends_with('>');
    let source_family = SyntaxName::try_new(DeclarationIdentityFamily::Source.prefix())
        .expect("fixed Source family is an identifier");
    let canonical_source_family = !malformed_delimited_absolute
        && match typed.value() {
            Ok(_) => typed.normalized_for_family(&source_family).1,
            Err(SyntaxIdRefIssue::MissingSuffix) => marker_family.as_ref().is_some_and(|family| {
                family.as_ref().is_none_or(|family| {
                    family.as_str() == DeclarationIdentityFamily::Source.prefix()
                })
            }),
            Err(_) => false,
        };
    let problem = if malformed_delimited_absolute {
        Some(SourceIdProblem::Malformed)
    } else {
        match typed.value() {
            Err(SyntaxIdRefIssue::MissingSuffix) if marker_family.is_some() => {
                (!canonical_source_family).then_some(SourceIdProblem::WrongFamily)
            }
            Err(_) => Some(SourceIdProblem::Malformed),
            Ok(_) if !canonical_source_family => Some(SourceIdProblem::WrongFamily),
            Ok(_) => None,
        }
    };
    let requires_name = marker_family.is_some() || id_spelling == "@super.";

    parser.start(SyntaxKind::DeclarationPublicId, SyntaxRole::PublicId);
    if let Some(problem) = problem {
        parser.start(
            match problem {
                SourceIdProblem::WrongFamily => SyntaxKind::WrongFamilyReference,
                SourceIdProblem::Malformed => SyntaxKind::ErrorNode,
            },
            match problem {
                SourceIdProblem::WrongFamily => SyntaxRole::Reference(0),
                SourceIdProblem::Malformed => SyntaxRole::Recovery(0),
            },
        );
    }
    if trailing_type_colon {
        parser.take_for_partition();
        parser.push(SyntaxEvent::token(
            SyntaxKind::EntityReferenceToken,
            id_range,
        ));
    } else {
        parser.bump();
    }
    if problem.is_some() {
        parser.finish();
    }
    parser.finish();
    if let Some(problem) = problem {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            match problem {
                SourceIdProblem::WrongFamily => "syntax.source.wrong_family_id",
                SourceIdProblem::Malformed => "syntax.source.malformed_id",
            },
            id_range,
            match problem {
                SourceIdProblem::WrongFamily => {
                    "source declaration ID must belong to the `source` family"
                }
                SourceIdProblem::Malformed => "source declaration ID is malformed",
            },
        )));
    }
    if trailing_type_colon {
        parser.push(SyntaxEvent::token(
            SyntaxKind::PunctuationToken,
            SourceRange::new(id_range.end(), token.range().end()),
        ));
    }
    Some(SourceIdEmission {
        requires_name,
        consumed_type_colon: trailing_type_colon,
        projection: PendingSourceId::Authored {
            value: typed,
            source: id_range,
            canonical_source_family,
            requires_name,
        },
    })
}

fn emit_optional_name(parser: &mut ShadowDocumentParser<'_, '_>) -> PendingSourceName {
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        let token = parser.current().expect("checked Source name token");
        let value = SyntaxName::try_new(parser.text_of(token));
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
        PendingSourceName::Authored {
            value,
            source: token.range(),
        }
    } else {
        PendingSourceName::Absent
    }
}

fn emit_required_name(parser: &mut ShadowDocumentParser<'_, '_>) -> PendingSourceName {
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        emit_optional_name(parser)
    } else {
        emit_missing_name(parser)
    }
}

fn emit_missing_name(parser: &mut ShadowDocumentParser<'_, '_>) -> PendingSourceName {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.source.missing_name",
        SourceRange::new(at, at),
        "source declaration requires a public ID or local name",
    )));
    PendingSourceName::Missing {
        insertion: SourceRange::new(at, at),
    }
}

fn emit_source_type(
    parser: &mut ShadowDocumentParser<'_, '_>,
    consumed_colon: bool,
) -> (PendingSourceTypeState, bool) {
    let mut missing_colon = false;
    if !consumed_colon {
        if parser.at(":") {
            parser.bump();
        } else {
            missing_colon = true;
            let at = parser.current_offset();
            parser.push(SyntaxEvent::MissingToken {
                expected: expected(SyntaxKind::PunctuationToken),
                at,
            });
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.source.missing_colon",
                SourceRange::new(at, at),
                "source declaration requires `:` before its source type",
            )));
        }
    }
    parser.bump_trivia();

    let body = (parser.cursor()..token_count(parser))
        .find(|index| token_text(parser, *index) == Some("{"))
        .unwrap_or_else(|| token_count(parser));
    let end = trimmed_end(parser, parser.cursor(), body);
    if parser.cursor() == end {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::IdentifierToken),
            at,
        });
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source.missing_type",
            SourceRange::new(at, at),
            "source declaration requires a source type",
        )));
        emit_type(parser, end, SyntaxRole::Type);
        return (PendingSourceTypeState::Missing, missing_colon);
    }
    emit_type(parser, end, SyntaxRole::Type);
    bump_until(parser, end);
    (PendingSourceTypeState::Authored, missing_colon)
}

fn emit_source_body(parser: &mut ShadowDocumentParser<'_, '_>) -> PendingSourceBodyProjection {
    parser.bump_trivia();
    if !parser.at("{") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source.missing_body",
            SourceRange::new(at, at),
            "source declaration requires a body",
        )));
        return PendingSourceBodyProjection::Missing;
    }

    parser.start(SyntaxKind::Block, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(token_count(parser));
    parser.start(SyntaxKind::StatementList, SyntaxRole::Element(0));
    let mut ledger = SourceBodyLedger::default();
    let mut members = Vec::new();
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }

        let start = parser.cursor();
        let terminator = find_source_entry_terminator(parser, start, close);
        let segment_end = terminator.map_or(close, |(index, _)| index);
        let significant_end = trimmed_end(parser, start, segment_end);
        if significant_end == start {
            bump_until(parser, segment_end.saturating_add(1));
            continue;
        }
        let end = if terminator.is_some_and(|(_, semicolon)| semicolon) {
            segment_end.saturating_add(1)
        } else {
            significant_end
        };
        if !parser.charge_source_member() {
            bump_until(parser, end);
            break;
        }
        members.push(emit_source_body_entry(parser, end, &mut ledger));
        bump_until(parser, end);
    }
    parser.finish();
    parser.start(SyntaxKind::OmittedBlockTail, SyntaxRole::Tail);
    parser.finish();

    let closed = parser.cursor() == close && parser.at("}");
    if closed {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.source.missing_block_close",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source.missing_block_close",
            SourceRange::new(at, at),
            "missing closing `}` for source body",
        )));
    }
    parser.finish();
    PendingSourceBodyProjection::Braced {
        members: members.into_boxed_slice(),
        closed,
    }
}

fn find_source_entry_terminator(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    close: usize,
) -> Option<(usize, bool)> {
    let ordinary = find_statement_terminator(parser, start, close);
    if token_text(parser, start) != Some("on") {
        return ordinary;
    }

    let arrow = find_top_level_boundary(parser, start, &["=>"]).min(close);
    if arrow >= close || token_text(parser, arrow) != Some("=>") {
        return ordinary;
    }
    let Some(body_open) = first_significant(parser, arrow.saturating_add(1), close) else {
        return ordinary;
    };
    if token_text(parser, body_open) != Some("{") {
        return ordinary;
    }

    let mut brace_depth = 1_usize;
    for index in body_open.saturating_add(1)..close {
        match token_text(parser, index) {
            Some("{") => brace_depth = brace_depth.saturating_add(1),
            Some("}") => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
        if parser
            .token_at(index)
            .is_some_and(|token| token.kind() == SyntaxKind::NewlineToken)
            && brace_depth == 1
            && first_significant(parser, index.saturating_add(1), close)
                .and_then(|next| token_text(parser, next))
                .is_some_and(is_source_body_head)
        {
            let recovered = (index, false);
            return ordinary
                .filter(|(ordinary_index, _)| *ordinary_index <= index)
                .or(Some(recovered));
        }
    }
    ordinary
}

fn is_source_body_head(spelling: &str) -> bool {
    matches!(
        spelling,
        "from" | "backpressure" | "replay" | "privacy" | "on" | "requires" | "ensures"
    )
}

fn emit_source_body_entry(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    ledger: &mut SourceBodyLedger,
) -> PendingSourceMemberProjection {
    let source_ordinal = ledger.source_ordinal;
    ledger.source_ordinal = ledger.source_ordinal.saturating_add(1);
    match parser.current_text() {
        Some("from") => {
            let statement_ordinal = take_statement_ordinal(ledger);
            let duplicate = std::mem::replace(&mut ledger.saw_from, true);
            emit_source_from(parser, end, source_ordinal, statement_ordinal, duplicate)
        }
        Some("backpressure") => {
            let statement_ordinal = take_statement_ordinal(ledger);
            let duplicate = std::mem::replace(&mut ledger.saw_backpressure, true);
            let (assignment, evidence) =
                emit_source_policy_assignment(parser, end, statement_ordinal);
            PendingSourceMemberProjection::Backpressure {
                source_ordinal,
                statement_ordinal,
                assignment,
                policy: source_backpressure_policy(parser, &evidence),
                duplicate,
            }
        }
        Some("replay") => {
            let statement_ordinal = take_statement_ordinal(ledger);
            let duplicate = std::mem::replace(&mut ledger.saw_replay, true);
            let (assignment, evidence) =
                emit_source_policy_assignment(parser, end, statement_ordinal);
            PendingSourceMemberProjection::Replay {
                source_ordinal,
                statement_ordinal,
                assignment,
                policy: source_replay_policy(&evidence),
                duplicate,
            }
        }
        Some("privacy") => {
            let statement_ordinal = take_statement_ordinal(ledger);
            let duplicate = std::mem::replace(&mut ledger.saw_privacy, true);
            let (assignment, evidence) =
                emit_source_policy_assignment(parser, end, statement_ordinal);
            PendingSourceMemberProjection::Privacy {
                source_ordinal,
                statement_ordinal,
                assignment,
                policy: source_privacy_policy(&evidence),
                duplicate,
            }
        }
        Some("on") => {
            let statement_ordinal = take_statement_ordinal(ledger);
            emit_source_handler(parser, end, source_ordinal, statement_ordinal)
        }
        Some("requires") => {
            let clause_start = parser.current_offset();
            let contract_ordinal = ledger.contract_ordinal;
            let condition = emit_contract_clause_until(
                parser,
                end,
                SyntaxKind::RequiresClause,
                SyntaxRole::ContractClause(contract_ordinal),
            );
            let condition = completed_expression_state(parser, condition.start_event);
            if ledger.saw_ensures {
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.contract.invalid_clause_order",
                    SourceRange::new(clause_start, parser.current_offset()),
                    "`requires` clauses must precede every `ensures` clause",
                )));
            }
            let family_ordinal = ledger.requires;
            ledger.requires = ledger.requires.saturating_add(1);
            ledger.contract_ordinal = ledger.contract_ordinal.saturating_add(1);
            PendingSourceMemberProjection::UnsupportedContract {
                source_ordinal,
                contract_ordinal,
                family: SourceContractSyntaxKind::Requires,
                family_ordinal,
                condition,
                out_of_order: ledger.saw_ensures,
            }
        }
        Some("ensures") => {
            let contract_ordinal = ledger.contract_ordinal;
            let condition = emit_contract_clause_until(
                parser,
                end,
                SyntaxKind::EnsuresClause,
                SyntaxRole::ContractClause(contract_ordinal),
            );
            let condition = completed_expression_state(parser, condition.start_event);
            let family_ordinal = ledger.ensures;
            ledger.ensures = ledger.ensures.saturating_add(1);
            ledger.contract_ordinal = ledger.contract_ordinal.saturating_add(1);
            ledger.saw_ensures = true;
            PendingSourceMemberProjection::UnsupportedContract {
                source_ordinal,
                contract_ordinal,
                family: SourceContractSyntaxKind::Ensures,
                family_ordinal,
                condition,
                out_of_order: false,
            }
        }
        _ => {
            let statement_ordinal = take_statement_ordinal(ledger);
            emit_statement_fragment(parser, end, SyntaxRole::Statement(statement_ordinal));
            PendingSourceMemberProjection::Recovery {
                source_ordinal,
                statement_ordinal,
            }
        }
    }
}

fn take_statement_ordinal(ledger: &mut SourceBodyLedger) -> u32 {
    let ordinal = ledger.statement_ordinal;
    ledger.statement_ordinal = ledger.statement_ordinal.saturating_add(1);
    ordinal
}

fn emit_source_from(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    source_ordinal: u32,
    statement_ordinal: u32,
    duplicate: bool,
) -> PendingSourceMemberProjection {
    parser.start(
        SyntaxKind::ExpressionStatement,
        SyntaxRole::Statement(statement_ordinal),
    );
    parser.bump();
    parser.bump_trivia();
    let child_end = statement_child_end(parser, end);
    let value = emit_expression_node(parser, child_end, SyntaxRole::Initializer);
    bump_until(parser, end);
    parser.finish();
    PendingSourceMemberProjection::From {
        source_ordinal,
        statement_ordinal,
        value: completed_expression_state(parser, value.start_event),
        duplicate,
    }
}

fn emit_source_handler(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    source_ordinal: u32,
    statement_ordinal: u32,
) -> PendingSourceMemberProjection {
    parser.start(
        SyntaxKind::OnStatement,
        SyntaxRole::Statement(statement_ordinal),
    );
    parser.bump();
    parser.bump_trivia();
    let arrow = find_top_level_boundary(parser, parser.cursor(), &["=>"]).min(end);
    let has_arrow = arrow < end && token_text(parser, arrow) == Some("=>");
    let event_start = first_significant(parser, parser.cursor(), arrow);
    let head = match event_start.and_then(|index| token_text(parser, index)) {
        Some("item") => SourceHandlerHead::Item,
        Some("error") => SourceHandlerHead::Error,
        Some("progress") => SourceHandlerHead::Progress,
        Some("disconnected") => SourceHandlerHead::Disconnected,
        Some("permission_revoked") => SourceHandlerHead::PermissionRevoked,
        Some("end") => SourceHandlerHead::End,
        _ => SourceHandlerHead::Unknown,
    };
    let unknown_value = event_start.and_then(|start| single_name_in_interval(parser, start, arrow));
    let event = match head {
        SourceHandlerHead::Item | SourceHandlerHead::Error | SourceHandlerHead::Progress => {
            parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
            parser.bump();
            parser.finish();
            parser.bump_trivia();
            let pattern_start = parser.event_position();
            emit_pattern(parser, arrow, SyntaxRole::Pattern);
            let state = emitted_pattern_state(parser, pattern_start);
            match head {
                SourceHandlerHead::Item => PendingSourceHandlerEvent::Item(state),
                SourceHandlerHead::Error => PendingSourceHandlerEvent::Error(state),
                SourceHandlerHead::Progress => PendingSourceHandlerEvent::Progress(state),
                _ => unreachable!("matched closed payload event family"),
            }
        }
        SourceHandlerHead::Disconnected
        | SourceHandlerHead::PermissionRevoked
        | SourceHandlerHead::End => {
            let condition = emit_expression_node(parser, arrow, SyntaxRole::Condition);
            let state = completed_expression_state(parser, condition.start_event);
            match head {
                SourceHandlerHead::Disconnected => PendingSourceHandlerEvent::Disconnected(state),
                SourceHandlerHead::PermissionRevoked => {
                    PendingSourceHandlerEvent::PermissionRevoked(state)
                }
                SourceHandlerHead::End => PendingSourceHandlerEvent::End(state),
                _ => unreachable!("matched closed condition event family"),
            }
        }
        SourceHandlerHead::Unknown => {
            let condition = emit_expression_node(parser, arrow, SyntaxRole::Condition);
            PendingSourceHandlerEvent::Unknown {
                value: unknown_value,
                condition: completed_expression_state(parser, condition.start_event),
            }
        }
    };
    bump_until(parser, arrow);

    let arrow_state = if !has_arrow {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source.missing_handler_arrow",
            SourceRange::new(at, at),
            "source handler requires `=>` before its body",
        )));
        PendingSourcePunctuation::Missing(SourceRange::new(at, at))
    } else {
        let range = parser
            .current()
            .expect("checked Source handler arrow")
            .range();
        parser.bump();
        PendingSourcePunctuation::Authored(range)
    };

    parser.bump_trivia();
    let body_end = trimmed_end(parser, parser.cursor(), end);
    let body = if parser.cursor() >= body_end || !has_arrow {
        let at = parser.current_offset();
        if has_arrow {
            parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
            parser.finish();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.source.missing_handler_body",
                SourceRange::new(at, at),
                "source handler requires a statement or block body",
            )));
        }
        PendingSourceHandlerBody::Missing
    } else if parser.at("{") {
        let closed = emit_braced_statement_block_until(
            parser,
            end,
            SyntaxKind::SourceItem,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.source.missing_handler_close",
        );
        PendingSourceHandlerBody::Block { closed }
    } else {
        emit_statement_fragment(parser, end, SyntaxRole::Body);
        PendingSourceHandlerBody::Statement
    };
    bump_until(parser, end);
    parser.finish();
    PendingSourceMemberProjection::Handler {
        source_ordinal,
        statement_ordinal,
        event,
        arrow: arrow_state,
        body,
    }
}

#[derive(Clone, Debug)]
struct SourcePolicyEvidence {
    state: PendingSourceChildState,
    name: Option<SyntaxName>,
    projection: Option<PendingExpressionProjection>,
}

fn emit_source_policy_assignment(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    statement_ordinal: u32,
) -> (PendingSourcePunctuation, SourcePolicyEvidence) {
    let child_end = statement_child_end(parser, end);
    let target_start = parser.cursor();
    let equals = find_top_level_boundary(parser, target_start, &["="]).min(child_end);
    let has_equals = equals < child_end && token_text(parser, equals) == Some("=");
    let target_end = if has_equals {
        equals
    } else {
        first_significant(parser, target_start.saturating_add(1), child_end).unwrap_or(child_end)
    };

    parser.start(
        SyntaxKind::AssignmentStatement,
        SyntaxRole::Statement(statement_ordinal),
    );
    let _ = emit_expression_node(parser, target_end, SyntaxRole::Target);
    bump_until(parser, target_end);
    let assignment = if has_equals {
        let range = parser
            .current()
            .expect("checked Source policy assignment")
            .range();
        parser.bump();
        PendingSourcePunctuation::Authored(range)
    } else {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source.missing_policy_assignment",
            SourceRange::new(at, at),
            "Source policy requires `=` before its value",
        )));
        PendingSourcePunctuation::Missing(SourceRange::new(at, at))
    };
    parser.bump_trivia();
    let value_start = parser.cursor();
    let value = emit_expression_node(parser, child_end, SyntaxRole::Initializer);
    let evidence = SourcePolicyEvidence {
        state: completed_expression_state(parser, value.start_event),
        name: single_name_in_interval(parser, value_start, child_end),
        projection: parser.expression_projection_at(value.start_event).cloned(),
    };
    bump_until(parser, end);
    parser.finish();
    (assignment, evidence)
}

fn source_backpressure_policy(
    parser: &ShadowDocumentParser<'_, '_>,
    evidence: &SourcePolicyEvidence,
) -> PendingSourceBackpressurePolicy {
    match evidence.state {
        PendingSourceChildState::Missing => PendingSourceBackpressurePolicy::Missing,
        PendingSourceChildState::Invalid => PendingSourceBackpressurePolicy::Invalid,
        PendingSourceChildState::Authored => match evidence.name.as_ref().map(SyntaxName::as_str) {
            Some("latest") => PendingSourceBackpressurePolicy::Latest,
            Some("blocking_not_allowed") => PendingSourceBackpressurePolicy::BlockingNotAllowed,
            _ => source_bounded_policy(parser, evidence)
                .unwrap_or_else(|| PendingSourceBackpressurePolicy::Unknown(evidence.name.clone())),
        },
    }
}

fn source_bounded_policy(
    parser: &ShadowDocumentParser<'_, '_>,
    evidence: &SourcePolicyEvidence,
) -> Option<PendingSourceBackpressurePolicy> {
    let pending = evidence.projection.as_ref()?;
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call)) =
        pending.projection()
    else {
        return None;
    };
    let callee = pending
        .components()
        .iter()
        .find(|component| component.role() == ExpressionComponentRole::CallCallee)
        .and_then(|component| single_name_in_range(parser, component.range()));
    if callee.as_ref().map(SyntaxName::as_str) != Some("bounded") {
        return None;
    }

    let mut capacity = PendingSourceBoundedArgument::Missing;
    let mut overflow_argument = PendingSourceBoundedArgument::Missing;
    let mut capacity_seen = false;
    let mut overflow_seen = false;
    let mut unexpected_arguments = false;
    for (ordinal, argument) in call.arguments().iter().enumerate() {
        let checked_ordinal = u16::try_from(ordinal)
            .expect("Call argument budget fits the Source policy ordinal domain");
        let SyntaxCallArgumentProjection::Named { name, .. } = argument else {
            unexpected_arguments = true;
            continue;
        };
        let state = source_argument_state(parser, pending, argument, checked_ordinal);
        match name.as_ref().map(SyntaxName::as_str) {
            Ok("capacity") => {
                if capacity_seen {
                    unexpected_arguments = true;
                    if let PendingSourceBoundedArgument::Present { duplicate, .. } = &mut capacity {
                        *duplicate = true;
                    }
                } else {
                    capacity_seen = true;
                    capacity = PendingSourceBoundedArgument::Present {
                        ordinal: checked_ordinal,
                        value: state,
                        duplicate: false,
                    };
                }
            }
            Ok("overflow") => {
                if overflow_seen {
                    unexpected_arguments = true;
                    if let PendingSourceBoundedArgument::Present { duplicate, .. } =
                        &mut overflow_argument
                    {
                        *duplicate = true;
                    }
                } else {
                    overflow_seen = true;
                    overflow_argument = PendingSourceBoundedArgument::Present {
                        ordinal: checked_ordinal,
                        value: state,
                        duplicate: false,
                    };
                }
            }
            _ => unexpected_arguments = true,
        }
    }

    let overflow = if !overflow_seen {
        PendingSourceOverflowPolicy::Missing
    } else if overflow_argument.value_has_recovery() {
        PendingSourceOverflowPolicy::Invalid {
            argument: overflow_argument,
        }
    } else {
        match source_call_argument_name(parser, pending, overflow_argument) {
            Some(name) if name.as_str() == "drop_oldest" => {
                PendingSourceOverflowPolicy::DropOldest(overflow_argument)
            }
            Some(name) if name.as_str() == "drop_newest" => {
                PendingSourceOverflowPolicy::DropNewest(overflow_argument)
            }
            Some(name) if name.as_str() == "error" => {
                PendingSourceOverflowPolicy::Error(overflow_argument)
            }
            Some(name) if name.as_str() == "coalesce" => {
                PendingSourceOverflowPolicy::Coalesce(overflow_argument)
            }
            value => PendingSourceOverflowPolicy::Unknown {
                argument: overflow_argument,
                value,
            },
        }
    };

    Some(PendingSourceBackpressurePolicy::Bounded {
        capacity,
        overflow,
        unexpected_arguments,
        recovered_call: call.terminator() != SyntaxCallArgumentListTerminator::Closed,
    })
}

fn source_replay_policy(
    evidence: &SourcePolicyEvidence,
) -> PendingSourceNamedPolicy<SourceReplaySyntaxKind> {
    match evidence.state {
        PendingSourceChildState::Missing => PendingSourceNamedPolicy::Missing,
        PendingSourceChildState::Invalid => PendingSourceNamedPolicy::Invalid,
        PendingSourceChildState::Authored => match evidence.name.as_ref().map(SyntaxName::as_str) {
            Some("full") => PendingSourceNamedPolicy::Known(SourceReplaySyntaxKind::Full),
            Some("hash_only") => PendingSourceNamedPolicy::Known(SourceReplaySyntaxKind::HashOnly),
            Some("summary") => PendingSourceNamedPolicy::Known(SourceReplaySyntaxKind::Summary),
            Some("event_only") => {
                PendingSourceNamedPolicy::Known(SourceReplaySyntaxKind::EventOnly)
            }
            Some("none") => PendingSourceNamedPolicy::Known(SourceReplaySyntaxKind::None),
            _ => PendingSourceNamedPolicy::Unknown(evidence.name.clone()),
        },
    }
}

fn source_privacy_policy(
    evidence: &SourcePolicyEvidence,
) -> PendingSourceNamedPolicy<SourcePrivacySyntaxKind> {
    match evidence.state {
        PendingSourceChildState::Missing => PendingSourceNamedPolicy::Missing,
        PendingSourceChildState::Invalid => PendingSourceNamedPolicy::Invalid,
        PendingSourceChildState::Authored => match evidence.name.as_ref().map(SyntaxName::as_str) {
            Some("transient") => {
                PendingSourceNamedPolicy::Known(SourcePrivacySyntaxKind::Transient)
            }
            Some("redacted") => PendingSourceNamedPolicy::Known(SourcePrivacySyntaxKind::Redacted),
            Some("recordable") => {
                PendingSourceNamedPolicy::Known(SourcePrivacySyntaxKind::Recordable)
            }
            Some("private") => PendingSourceNamedPolicy::Known(SourcePrivacySyntaxKind::Private),
            _ => PendingSourceNamedPolicy::Unknown(evidence.name.clone()),
        },
    }
}

fn source_argument_state(
    parser: &ShadowDocumentParser<'_, '_>,
    pending: &PendingExpressionProjection,
    argument: &SyntaxCallArgumentProjection,
    ordinal: u16,
) -> PendingSourceChildState {
    if argument.value().is_missing() {
        PendingSourceChildState::Missing
    } else if argument.has_recovery()
        || source_call_argument_value_range(pending, ordinal)
            .and_then(|range| parser.expression_projection_for_range(range))
            .is_none_or(PendingExpressionProjection::has_recovery)
    {
        PendingSourceChildState::Invalid
    } else {
        PendingSourceChildState::Authored
    }
}

fn source_call_argument_value_range(
    pending: &PendingExpressionProjection,
    ordinal: u16,
) -> Option<SourceRange> {
    pending
        .components()
        .iter()
        .find(|component| {
            component.role()
                == ExpressionComponentRole::CallArgument {
                    argument: ordinal,
                    part: SyntaxCallArgumentPart::Value,
                }
        })
        .map(|component| component.range())
}

fn source_call_argument_name(
    parser: &ShadowDocumentParser<'_, '_>,
    pending: &PendingExpressionProjection,
    argument: PendingSourceBoundedArgument,
) -> Option<SyntaxName> {
    let PendingSourceBoundedArgument::Present { ordinal, .. } = argument else {
        return None;
    };
    pending
        .components()
        .iter()
        .find(|component| {
            component.role()
                == ExpressionComponentRole::CallArgument {
                    argument: ordinal,
                    part: SyntaxCallArgumentPart::Value,
                }
        })
        .and_then(|component| single_name_in_range(parser, component.range()))
}

fn completed_expression_state(
    parser: &ShadowDocumentParser<'_, '_>,
    start_event: usize,
) -> PendingSourceChildState {
    if parser.completed_kind(start_event) == Some(SyntaxKind::MissingExpression) {
        PendingSourceChildState::Missing
    } else if parser
        .expression_projection_at(start_event)
        .is_none_or(PendingExpressionProjection::has_recovery)
    {
        PendingSourceChildState::Invalid
    } else {
        PendingSourceChildState::Authored
    }
}

fn emitted_pattern_state(
    parser: &ShadowDocumentParser<'_, '_>,
    event_position: usize,
) -> PendingSourceChildState {
    if parser.started_kind_since(event_position, SyntaxKind::MissingPattern) {
        PendingSourceChildState::Missing
    } else if parser.started_kind_since(event_position, SyntaxKind::ErrorPattern) {
        PendingSourceChildState::Invalid
    } else {
        PendingSourceChildState::Authored
    }
}

fn single_name_in_interval(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<SyntaxName> {
    let first = first_significant(parser, start, end)?;
    let next = first_significant(parser, first.saturating_add(1), end);
    if next.is_some() {
        return None;
    }
    let token = parser.token_at(first)?;
    SyntaxName::try_new(parser.text_of(token)).ok()
}

fn single_name_in_range(
    parser: &ShadowDocumentParser<'_, '_>,
    range: SourceRange,
) -> Option<SyntaxName> {
    let mut found = None;
    for index in 0..token_count(parser) {
        let token = parser.token_at(index)?;
        if token.range().end() <= range.start() || token.range().start() >= range.end() {
            continue;
        }
        if matches!(
            token.kind(),
            SyntaxKind::WhitespaceToken
                | SyntaxKind::NewlineToken
                | SyntaxKind::CommentToken
                | SyntaxKind::DocCommentToken
        ) {
            continue;
        }
        if found.is_some() || token.range() != range {
            return None;
        }
        found = SyntaxName::try_new(parser.text_of(token)).ok();
    }
    found
}

fn statement_child_end(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> usize {
    if end > parser.cursor() && token_text(parser, end - 1) == Some(";") {
        end - 1
    } else {
        end
    }
}
