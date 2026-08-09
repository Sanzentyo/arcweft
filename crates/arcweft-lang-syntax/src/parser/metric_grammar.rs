//! Private retained Metric declaration grammar.

use std::collections::BTreeMap;

use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::declaration::emit_metric_declaration_header;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, emit_required_punctuation, expected,
    find_matching_close, find_statement_terminator, find_top_level_boundary, token_count,
    trimmed_end,
};
use super::type_ref::emit_type;
use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, PendingExpressionComponent,
    PendingExpressionProjection,
};
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{MetricKindSyntaxValue, SyntaxKind, SyntaxRole};

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = DocumentParser::new(source, tokens, events, budget);
    parser.start(SyntaxKind::MetricDeclarationItem, role);
    emit_metric_declaration_header(&mut parser, emit_metric_kind, emit_metric_value_type);
    parser.bump_trivia();
    emit_metric_body(&mut parser);
    emit_trailing_recovery(&mut parser);
    parser.finish();
}

fn emit_metric_kind(parser: &mut DocumentParser<'_, '_>) {
    let role = match parser.current_text() {
        Some("counter") => SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Counter),
        Some("gauge") => SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Gauge),
        Some("histogram") => SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Histogram),
        _ => SyntaxRole::Kind,
    };
    parser.start(SyntaxKind::MetricKind, role);
    match parser.current_text() {
        Some("counter" | "gauge" | "histogram") => {
            parser.bump();
        }
        Some(_) if parser.current_kind() == Some(SyntaxKind::IdentifierToken) => {
            let range = parser.current().expect("checked metric kind").range();
            parser.bump();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.metric.unknown_kind",
                range,
                "Metric kind must be `counter`, `gauge`, or `histogram`",
            )));
        }
        _ => {
            let at = parser.current_offset();
            parser.push(SyntaxEvent::MissingToken {
                expected: expected(SyntaxKind::IdentifierToken),
                at,
            });
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.metric.missing_kind",
                SourceRange::new(at, at),
                "Metric declaration requires `counter`, `gauge`, or `histogram`",
            )));
        }
    }
    parser.finish();
}

fn emit_metric_value_type(parser: &mut DocumentParser<'_, '_>) {
    emit_required_punctuation(
        parser,
        SyntaxKind::ColonNode,
        SyntaxRole::Colon,
        ":",
        "syntax.metric.missing_type_separator",
        "Metric declaration requires `: ValueType`",
    );
    parser.bump_trivia();
    let end = find_top_level_boundary(parser, parser.cursor(), token_count(parser), &["{"]);
    emit_type(parser, end, SyntaxRole::Type);
    bump_until(parser, end);
}

#[derive(Default)]
struct MetricMemberLedger {
    first_ranges: [Option<SourceRange>; 3],
    highest_rank: Option<usize>,
}

impl MetricMemberLedger {
    fn record(
        &mut self,
        parser: &mut DocumentParser<'_, '_>,
        rank: usize,
        member: &'static str,
        range: SourceRange,
    ) {
        if let Some(first) = self.first_ranges[rank] {
            parser.push(SyntaxEvent::Diagnostic(
                PendingSyntaxDiagnostic::new(
                    "syntax.metric.duplicate_member",
                    range,
                    format!("Metric `{member}` section may appear only once"),
                )
                .with_related_range(first),
            ));
        } else {
            self.first_ranges[rank] = Some(range);
        }
        if self.highest_rank.is_some_and(|highest| rank < highest) {
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.metric.member_order",
                range,
                "Metric members must be ordered as `unit`, `labels`, then `buckets`",
            )));
        }
        self.highest_rank = Some(self.highest_rank.map_or(rank, |highest| highest.max(rank)));
    }
}

fn emit_metric_body(parser: &mut DocumentParser<'_, '_>) {
    if !parser.at("{") {
        emit_missing_body(parser);
        return;
    }

    parser.start(SyntaxKind::MetricBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{");
    let body_end = close.unwrap_or_else(|| token_count(parser));
    emit_metric_members(parser, body_end);
    bump_until(parser, body_end);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.metric.missing_body_close",
    );
    parser.finish();
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
        "syntax.metric.missing_body",
        SourceRange::new(at, at),
        "Metric declaration requires a braced policy body",
    )));
}

fn emit_metric_members(parser: &mut DocumentParser<'_, '_>, body_end: usize) {
    let mut ledger = MetricMemberLedger::default();
    let mut ordinal = 0_u16;
    while parser.cursor() < body_end {
        parser.bump_trivia();
        if parser.cursor() >= body_end {
            break;
        }
        let start = parser.cursor();
        let line_end =
            find_statement_terminator(parser, start, body_end).map_or(body_end, |(end, _)| end);
        let keyword = parser.current().expect("metric member starts inside body");
        match parser.text_of(keyword) {
            "unit" => emit_unit_member(parser, line_end, ordinal, &mut ledger),
            "labels" => emit_labels_member(parser, line_end, ordinal, &mut ledger),
            "buckets" => emit_buckets_member(parser, line_end, ordinal, &mut ledger),
            _ => emit_unknown_member(parser, line_end, ordinal, keyword.range()),
        }
        bump_until(parser, line_end);
        if parser.at(";") || parser.current_kind() == Some(SyntaxKind::NewlineToken) {
            parser.bump();
        }
        if parser.budget_failed() {
            bump_until(parser, body_end);
            break;
        }
        ordinal = ordinal
            .checked_add(1)
            .expect("Metric member budget is below the role index range");
    }
}

fn emit_unit_member(
    parser: &mut DocumentParser<'_, '_>,
    line_end: usize,
    ordinal: u16,
    ledger: &mut MetricMemberLedger,
) {
    let keyword = parser.current().expect("unit keyword");
    ledger.record(parser, 0, "unit", keyword.range());
    parser.start(SyntaxKind::MetricUnitMember, SyntaxRole::Member(ordinal));
    parser.bump();
    parser.bump_trivia();
    emit_assignment(parser, "syntax.metric.missing_unit_assignment");
    parser.bump_trivia();
    let value_start = parser.cursor();
    let value_end = trimmed_end(parser, value_start, line_end);
    if value_start == value_end {
        emit_missing_member_value(
            parser,
            "syntax.metric.missing_unit",
            "Metric `unit` requires a string literal",
        );
    } else {
        emit_expression(parser, value_end, SyntaxRole::Initializer);
        if !is_single_string_token(parser, value_start, value_end) {
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.metric.unit_not_string",
                token_range(parser, value_start, value_end),
                "Metric `unit` must be one string literal",
            )));
        }
    }
    bump_until(parser, line_end);
    parser.finish();
}

fn emit_labels_member(
    parser: &mut DocumentParser<'_, '_>,
    line_end: usize,
    ordinal: u16,
    ledger: &mut MetricMemberLedger,
) {
    let keyword = parser.current().expect("labels keyword");
    ledger.record(parser, 1, "labels", keyword.range());
    parser.start(SyntaxKind::MetricLabelsBlock, SyntaxRole::Member(ordinal));
    parser.bump();
    parser.bump_trivia();
    if !parser.at("{") {
        emit_missing_member_value(
            parser,
            "syntax.metric.missing_labels_body",
            "Metric `labels` requires a braced label schema",
        );
        bump_until(parser, line_end);
        parser.finish();
        return;
    }

    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{")
        .map_or(line_end, |index| index.min(line_end));
    emit_metric_labels(parser, close);
    bump_until(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.metric.missing_labels_close",
    );
    bump_until(parser, line_end);
    parser.finish();
}

fn emit_metric_labels(parser: &mut DocumentParser<'_, '_>, block_end: usize) {
    let mut names = BTreeMap::<String, SourceRange>::new();
    let mut ordinal = 0_u16;
    while parser.cursor() < block_end {
        parser.bump_trivia();
        if parser.cursor() >= block_end {
            break;
        }
        let entry_end = find_statement_terminator(parser, parser.cursor(), block_end)
            .map_or(block_end, |(end, _)| end);
        emit_metric_label(parser, entry_end, ordinal, &mut names);
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
            .expect("Metric label budget is below the role index range");
    }
}

fn emit_metric_label(
    parser: &mut DocumentParser<'_, '_>,
    label_end: usize,
    ordinal: u16,
    names: &mut BTreeMap<String, SourceRange>,
) {
    parser.start(SyntaxKind::MetricLabel, SyntaxRole::Label(ordinal));
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        let token = parser.current().expect("checked label name");
        let name = parser.text_of(token).to_owned();
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
        if let Some(first) = names.get(&name).copied() {
            parser.push(SyntaxEvent::Diagnostic(
                PendingSyntaxDiagnostic::new(
                    "syntax.metric.duplicate_label",
                    token.range(),
                    "Metric label names must be unique",
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
            "syntax.metric.missing_label_name",
            SourceRange::new(at, at),
            "Metric label requires an ordinary name",
        )));
    }
    parser.bump_trivia();
    let authored_colon = emit_required_punctuation(
        parser,
        SyntaxKind::ColonNode,
        SyntaxRole::Colon,
        ":",
        "syntax.metric.missing_label_type",
        "Metric label requires `: Type`",
    );
    parser.bump_trivia();
    let type_end = if authored_colon {
        label_end
    } else {
        parser.cursor()
    };
    emit_type(parser, type_end, SyntaxRole::Type);
    bump_until(parser, label_end);
    parser.finish();
}

fn emit_buckets_member(
    parser: &mut DocumentParser<'_, '_>,
    line_end: usize,
    ordinal: u16,
    ledger: &mut MetricMemberLedger,
) {
    let keyword = parser.current().expect("buckets keyword");
    ledger.record(parser, 2, "buckets", keyword.range());
    parser.start(SyntaxKind::MetricBucketsMember, SyntaxRole::Member(ordinal));
    parser.bump();
    parser.bump_trivia();
    emit_assignment(parser, "syntax.metric.missing_buckets_assignment");
    parser.bump_trivia();
    if parser.at("[") {
        emit_bucket_sequence(parser, line_end);
    } else if parser.cursor() == trimmed_end(parser, parser.cursor(), line_end) {
        emit_missing_member_value(
            parser,
            "syntax.metric.missing_buckets",
            "Metric `buckets` requires a non-empty bracket sequence",
        );
    } else {
        let value_start = parser.cursor();
        emit_expression(parser, line_end, SyntaxRole::Initializer);
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.metric.buckets_not_sequence",
            token_range(parser, value_start, line_end),
            "Metric `buckets` must be a bracket sequence",
        )));
    }
    bump_until(parser, line_end);
    parser.finish();
}

fn emit_bucket_sequence(parser: &mut DocumentParser<'_, '_>, line_end: usize) {
    let close = find_matching_close(parser, parser.cursor() + 1, "[")
        .map_or(line_end, |index| index.min(line_end));
    let owner = parser.start_projected_owner(
        SyntaxKind::BracketSequenceExpression,
        SyntaxRole::Initializer,
    );
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
    let mut slots = Vec::new();
    let mut components = Vec::new();
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("]") {
            break;
        }
        let bucket_end = find_top_level_boundary(parser, parser.cursor(), close, &[",", "]"]);
        let (slot, range) = super::expression::expression_slot(parser, bucket_end);
        emit_expression(parser, bucket_end, SyntaxRole::Bucket(ordinal));
        bump_until(parser, bucket_end);
        if parser.budget_failed() {
            bump_until(parser, close);
            break;
        }
        slots.push(slot);
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::Element {
                ordinal: u32::from(ordinal),
            },
            range,
        ));
        ordinal = ordinal
            .checked_add(1)
            .expect("Metric bucket budget is below the role index range");
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    if ordinal == 0 && !parser.budget_failed() {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.metric.empty_buckets",
            SourceRange::new(at, at),
            "Histogram buckets must be non-empty",
        )));
    }
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.metric.missing_buckets_close",
    );
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::BracketSequence(slots.into_boxed_slice()),
            components,
        ),
    );
    parser.finish();
}

fn emit_unknown_member(
    parser: &mut DocumentParser<'_, '_>,
    line_end: usize,
    ordinal: u16,
    range: SourceRange,
) {
    parser.start(
        SyntaxKind::ErrorDeclarationMember,
        SyntaxRole::Member(ordinal),
    );
    bump_until(parser, line_end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.metric.unknown_member",
        range,
        "Metric body accepts only `unit`, `labels`, and `buckets`",
    )));
}

fn emit_assignment(parser: &mut DocumentParser<'_, '_>, code: &'static str) {
    parser.start(SyntaxKind::EqualsNode, SyntaxRole::Equals);
    if parser.at("=") {
        parser.bump();
        parser.finish();
        return;
    }
    let at = parser.current_offset();
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::PunctuationToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(at, at),
        "Metric member requires `=`",
    )));
}

fn emit_missing_member_value(
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

fn is_single_string_token(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> bool {
    let significant = (start..end)
        .filter_map(|index| {
            let token = parser.token_at(index)?;
            (!is_trivia(token.kind())).then_some(token)
        })
        .collect::<Vec<_>>();
    matches!(significant.as_slice(), [token] if token.kind() == SyntaxKind::StringToken)
}

fn token_range(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> SourceRange {
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

fn emit_trailing_recovery(parser: &mut DocumentParser<'_, '_>) {
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
        "unexpected syntax after Metric declaration body",
    )));
}
