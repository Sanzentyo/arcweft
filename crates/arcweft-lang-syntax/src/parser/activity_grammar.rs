//! Private retained Activity interface declaration grammar.

use std::collections::BTreeMap;

use arcweft_id::RetainedIdentityFamily;
use arcweft_source::SourceRange;

use super::declaration::emit_retained_declaration_header;
use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, expected, find_matching_close,
    find_statement_terminator, find_top_level_boundary, token_count, trimmed_end,
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
    parser.start(SyntaxKind::ActivityDeclarationItem, role);
    emit_retained_declaration_header(&mut parser, RetainedIdentityFamily::Activity, |_| {});
    parser.bump_trivia();
    reject_unexpected_header(&mut parser);
    parser.bump_trivia();
    emit_activity_body(&mut parser);
    emit_trailing_recovery(&mut parser);
    parser.finish();
}

fn reject_unexpected_header(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("{") || parser.is_at_end() {
        return;
    }
    let body = find_top_level_boundary(parser, parser.cursor(), &["{"]);
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, body);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.declaration.unexpected_header",
        SourceRange::new(start, parser.current_offset()),
        "Activity accepts no generics, origin clauses, where clause, or contracts in its header",
    )));
}

#[derive(Default)]
struct ActivitySectionLedger {
    first_ranges: [Option<SourceRange>; 5],
    highest_rank: Option<usize>,
}

impl ActivitySectionLedger {
    fn record(
        &mut self,
        parser: &mut ShadowDocumentParser<'_, '_>,
        rank: usize,
        section: &'static str,
        range: SourceRange,
    ) {
        if let Some(first) = self.first_ranges[rank] {
            parser.push(SyntaxEvent::Diagnostic(
                PendingSyntaxDiagnostic::new(
                    "syntax.activity.duplicate_member",
                    range,
                    format!("Activity `{section}` section may appear only once"),
                )
                .with_related_range(first),
            ));
        } else {
            self.first_ranges[rank] = Some(range);
        }
        if self.highest_rank.is_some_and(|highest| rank < highest) {
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.activity.out_of_order_member",
                range,
                "Activity sections must be ordered as mode, lifecycle, input, output, contract",
            )));
        }
        self.highest_rank = Some(self.highest_rank.map_or(rank, |highest| highest.max(rank)));
    }
}

fn emit_activity_body(parser: &mut ShadowDocumentParser<'_, '_>) {
    if !parser.at("{") {
        emit_missing_body(parser);
        return;
    }

    parser.start(SyntaxKind::ActivityBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{");
    let body_end = close.unwrap_or_else(|| token_count(parser));
    emit_activity_sections(parser, body_end);
    bump_until(parser, body_end);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.activity.missing_body_close",
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
        "syntax.activity.missing_body",
        SourceRange::new(at, at),
        "Activity declaration requires a braced abstract interface",
    )));
}

fn emit_activity_sections(parser: &mut ShadowDocumentParser<'_, '_>, body_end: usize) {
    let mut ledger = ActivitySectionLedger::default();
    let mut port_names = BTreeMap::<String, SourceRange>::new();
    let mut ordinal = 0_u16;
    while parser.cursor() < body_end {
        parser.bump_trivia();
        if parser.cursor() >= body_end {
            break;
        }
        let section_end = find_statement_terminator(parser, parser.cursor(), body_end)
            .map_or(body_end, |(end, _)| end);
        let keyword = parser.current().expect("Activity section begins in body");
        match parser.text_of(keyword) {
            "mode" => emit_policy_member(
                parser,
                section_end,
                ordinal,
                &mut ledger,
                ActivityPolicy::Mode,
            ),
            "lifecycle" => emit_policy_member(
                parser,
                section_end,
                ordinal,
                &mut ledger,
                ActivityPolicy::Lifecycle,
            ),
            "input" => emit_port_block(
                parser,
                section_end,
                ordinal,
                &mut ledger,
                &mut port_names,
                PortDirection::Input,
            ),
            "output" => emit_port_block(
                parser,
                section_end,
                ordinal,
                &mut ledger,
                &mut port_names,
                PortDirection::Output,
            ),
            "contract" => {
                emit_contract_block(parser, section_end, ordinal, &mut ledger);
            }
            _ => emit_unknown_section(parser, section_end, ordinal, keyword.range()),
        }
        bump_until(parser, section_end);
        if parser.at(";") || parser.current_kind() == Some(SyntaxKind::NewlineToken) {
            parser.bump();
        }
        if parser.budget_failed() {
            bump_until(parser, body_end);
            break;
        }
        ordinal = ordinal
            .checked_add(1)
            .expect("Activity member budget is below the role index range");
    }
}

#[derive(Clone, Copy)]
enum ActivityPolicy {
    Mode,
    Lifecycle,
}

impl ActivityPolicy {
    const fn rank(self) -> usize {
        match self {
            Self::Mode => 0,
            Self::Lifecycle => 1,
        }
    }

    const fn keyword(self) -> &'static str {
        match self {
            Self::Mode => "mode",
            Self::Lifecycle => "lifecycle",
        }
    }

    const fn node(self) -> SyntaxKind {
        match self {
            Self::Mode => SyntaxKind::ActivityModeMember,
            Self::Lifecycle => SyntaxKind::ActivityLifecycleMember,
        }
    }

    fn accepts(self, value: &str) -> bool {
        match self {
            Self::Mode => matches!(
                value,
                "deterministic" | "checkpointed_realtime" | "external_realtime"
            ),
            Self::Lifecycle => matches!(value, "stateless" | "snapshot"),
        }
    }
}

fn emit_policy_member(
    parser: &mut ShadowDocumentParser<'_, '_>,
    section_end: usize,
    ordinal: u16,
    ledger: &mut ActivitySectionLedger,
    policy: ActivityPolicy,
) {
    let keyword = parser.current().expect("Activity policy keyword");
    ledger.record(parser, policy.rank(), policy.keyword(), keyword.range());
    parser.start(policy.node(), SyntaxRole::Member(ordinal));
    parser.bump();
    parser.bump_trivia();
    emit_assignment(parser, "syntax.activity.missing_policy_assignment");
    parser.bump_trivia();

    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        let value = parser.current().expect("checked Activity policy value");
        if !policy.accepts(parser.text_of(value)) {
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.activity.unknown_policy",
                value.range(),
                format!("unknown Activity {} policy", policy.keyword()),
            )));
        }
        parser.bump();
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingMemberValue, SyntaxRole::Recovery(0));
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.activity.missing_policy_value",
            SourceRange::new(at, at),
            format!("Activity `{}` requires a policy value", policy.keyword()),
        )));
    }
    bump_until(parser, section_end);
    parser.finish();
}

#[derive(Clone, Copy)]
enum PortDirection {
    Input,
    Output,
}

impl PortDirection {
    const fn rank(self) -> usize {
        match self {
            Self::Input => 2,
            Self::Output => 3,
        }
    }

    const fn keyword(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }

    const fn node(self) -> SyntaxKind {
        match self {
            Self::Input => SyntaxKind::ActivityInputBlock,
            Self::Output => SyntaxKind::ActivityOutputBlock,
        }
    }

    const fn role(self, ordinal: u16) -> SyntaxRole {
        match self {
            Self::Input => SyntaxRole::InputPort(ordinal),
            Self::Output => SyntaxRole::OutputPort(ordinal),
        }
    }
}

fn emit_port_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    section_end: usize,
    ordinal: u16,
    ledger: &mut ActivitySectionLedger,
    names: &mut BTreeMap<String, SourceRange>,
    direction: PortDirection,
) {
    let keyword = parser.current().expect("Activity port-block keyword");
    ledger.record(
        parser,
        direction.rank(),
        direction.keyword(),
        keyword.range(),
    );
    parser.start(direction.node(), SyntaxRole::Member(ordinal));
    parser.bump();
    parser.bump_trivia();
    if !parser.at("{") {
        emit_missing_section_body(parser, direction.keyword());
        bump_until(parser, section_end);
        parser.finish();
        return;
    }
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{")
        .map_or(section_end, |index| index.min(section_end));
    emit_ports(parser, close, direction, names);
    bump_until(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.activity.missing_port_block_close",
    );
    bump_until(parser, section_end);
    parser.finish();
}

fn emit_ports(
    parser: &mut ShadowDocumentParser<'_, '_>,
    block_end: usize,
    direction: PortDirection,
    names: &mut BTreeMap<String, SourceRange>,
) {
    let mut ordinal = 0_u16;
    while parser.cursor() < block_end {
        parser.bump_trivia();
        if parser.cursor() >= block_end {
            break;
        }
        let entry_end = find_statement_terminator(parser, parser.cursor(), block_end)
            .map_or(block_end, |(end, _)| end);
        emit_port(parser, entry_end, direction.role(ordinal), names);
        bump_until(parser, entry_end);
        if parser.at(";") || parser.current_kind() == Some(SyntaxKind::NewlineToken) {
            parser.bump();
        }
        if parser.budget_failed() {
            bump_until(parser, block_end);
            break;
        }
        ordinal = ordinal
            .checked_add(1)
            .expect("Activity port budget is below the role index range");
    }
}

fn emit_port(
    parser: &mut ShadowDocumentParser<'_, '_>,
    entry_end: usize,
    role: SyntaxRole,
    names: &mut BTreeMap<String, SourceRange>,
) {
    parser.start(SyntaxKind::ActivityPort, role);
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        let token = parser.current().expect("checked Activity port name");
        let name = parser.text_of(token).to_owned();
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
        if let Some(first) = names.get(&name).copied() {
            parser.push(SyntaxEvent::Diagnostic(
                PendingSyntaxDiagnostic::new(
                    "syntax.activity.duplicate_port",
                    token.range(),
                    "Activity port names must be unique across input and output",
                )
                .with_related_range(first),
            ));
        } else {
            names.insert(name, token.range());
        }
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.activity.missing_port_name",
            SourceRange::new(at, at),
            "Activity port requires an ordinary name",
        )));
    }
    parser.bump_trivia();
    emit_port_type(parser, entry_end);
    bump_until(parser, entry_end);
    parser.finish();
}

fn emit_port_type(parser: &mut ShadowDocumentParser<'_, '_>, entry_end: usize) {
    if !parser.at(":") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingType, SyntaxRole::Type);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.activity.missing_port_type",
            SourceRange::new(at, at),
            "Activity port requires `: Type`",
        )));
        return;
    }
    parser.bump();
    parser.bump_trivia();
    let default = find_top_level_boundary(parser, parser.cursor(), &["="]).min(entry_end);
    emit_type(parser, default, SyntaxRole::Type);
    bump_until(parser, default);
    if parser.at("=") {
        let start = parser.current_offset();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        bump_until(parser, entry_end);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.activity.port_initializer_not_allowed",
            SourceRange::new(start, parser.current_offset()),
            "Activity ports do not accept initializers",
        )));
    }
}

fn emit_contract_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    section_end: usize,
    ordinal: u16,
    ledger: &mut ActivitySectionLedger,
) {
    let keyword = parser.current().expect("contract keyword");
    ledger.record(parser, 4, "contract", keyword.range());
    parser.start(
        SyntaxKind::ActivityContractBlock,
        SyntaxRole::Member(ordinal),
    );
    parser.bump();
    parser.bump_trivia();
    if !parser.at("{") {
        emit_missing_section_body(parser, "contract");
        bump_until(parser, section_end);
        parser.finish();
        return;
    }
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{")
        .map_or(section_end, |index| index.min(section_end));
    emit_activity_contract_clauses(parser, close);
    bump_until(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.activity.missing_contract_close",
    );
    bump_until(parser, section_end);
    parser.finish();
}

fn emit_activity_contract_clauses(parser: &mut ShadowDocumentParser<'_, '_>, block_end: usize) {
    let mut requires = 0_u16;
    let mut ensures = 0_u16;
    let mut saw_ensures = false;
    while parser.cursor() < block_end {
        parser.bump_trivia();
        if parser.cursor() >= block_end {
            break;
        }
        let entry_end = find_statement_terminator(parser, parser.cursor(), block_end)
            .map_or(block_end, |(end, _)| end);
        match parser.current_text() {
            Some("requires") => {
                let range = parser.current().expect("requires token").range();
                emit_activity_contract_clause(
                    parser,
                    entry_end,
                    SyntaxKind::RequiresClause,
                    SyntaxRole::RequiresClause(requires),
                );
                if saw_ensures {
                    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                        "syntax.activity.contract_order",
                        range,
                        "`requires` clauses must precede `ensures` clauses",
                    )));
                }
                requires = requires
                    .checked_add(1)
                    .expect("contract clause budget is below the role index range");
            }
            Some("ensures") => {
                saw_ensures = true;
                emit_activity_contract_clause(
                    parser,
                    entry_end,
                    SyntaxKind::EnsuresClause,
                    SyntaxRole::EnsuresClause(ensures),
                );
                ensures = ensures
                    .checked_add(1)
                    .expect("contract clause budget is below the role index range");
            }
            _ => emit_contract_error(parser, entry_end),
        }
        bump_until(parser, entry_end);
        if parser.at(";") || parser.current_kind() == Some(SyntaxKind::NewlineToken) {
            parser.bump();
        }
        if parser.budget_failed() {
            bump_until(parser, block_end);
            break;
        }
    }
}

fn emit_activity_contract_clause(
    parser: &mut ShadowDocumentParser<'_, '_>,
    entry_end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
) {
    parser.start(kind, role);
    parser.bump();
    parser.bump_trivia();
    let expression_end = trimmed_end(parser, parser.cursor(), entry_end);
    if parser.cursor() == expression_end {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingExpression, SyntaxRole::Condition);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.activity.missing_contract_expression",
            SourceRange::new(at, at),
            "Activity contract clause requires an expression",
        )));
    } else {
        emit_expression(parser, expression_end, SyntaxRole::Condition);
    }
    bump_until(parser, entry_end);
    parser.finish();
}

fn emit_contract_error(parser: &mut ShadowDocumentParser<'_, '_>, entry_end: usize) {
    let range = parser.current().expect("invalid contract member").range();
    parser.start(SyntaxKind::ErrorDeclarationMember, SyntaxRole::Recovery(0));
    bump_until(parser, entry_end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.activity.unknown_contract_clause",
        range,
        "Activity contract accepts only `requires` and `ensures` clauses",
    )));
}

fn emit_unknown_section(
    parser: &mut ShadowDocumentParser<'_, '_>,
    section_end: usize,
    ordinal: u16,
    range: SourceRange,
) {
    parser.start(
        SyntaxKind::ErrorDeclarationMember,
        SyntaxRole::Member(ordinal),
    );
    bump_until(parser, section_end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.activity.unknown_section",
        range,
        "Activity body accepts only mode, lifecycle, input, output, and contract",
    )));
}

fn emit_assignment(parser: &mut ShadowDocumentParser<'_, '_>, code: &'static str) {
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
        code,
        SourceRange::new(at, at),
        "Activity policy requires `=`",
    )));
}

fn emit_missing_section_body(parser: &mut ShadowDocumentParser<'_, '_>, section: &str) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.activity.missing_section_body",
        SourceRange::new(at, at),
        format!("Activity `{section}` section requires a braced body"),
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
        "unexpected syntax after Activity declaration body",
    )));
}
