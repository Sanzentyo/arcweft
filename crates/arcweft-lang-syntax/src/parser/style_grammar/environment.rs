//! Native Style environment condition and nested-body grammar.

use super::{
    PendingStyleBodyProjection, PendingStyleEnvironmentClause, PendingStyleEnvironmentComparison,
    PendingStyleEnvironmentCondition, PendingStyleEnvironmentConditionRecovery,
    PendingStyleEnvironmentField, PendingStyleEnvironmentProjection, PendingStyleName,
    PendingStylePunctuation, PendingSyntaxDiagnostic, ShadowDocumentParser, SourceRange,
    StyleEnvironmentComparison, StyleEnvironmentConditionIssue, StyleEnvironmentField,
    StyleSyntaxName, SyntaxEvent, SyntaxKind, SyntaxRole, bump_trivia_before, bump_until,
    emit_close_delimiter, emit_expression, emit_missing_delimiter, emit_missing_name,
    emit_open_delimiter, emit_style_members, emit_style_name, environment_clause_end, expected,
    find_matching_close, find_top_level_boundary, next_nontrivia, pending_name_range,
};

pub(super) fn emit_environment_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    enclosing_close: usize,
    source_ordinal: u32,
) -> PendingStyleEnvironmentProjection {
    parser.start(
        SyntaxKind::StyleEnvironmentBlock,
        SyntaxRole::Element(source_ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, enclosing_close);
    let intrinsic = if parser.at("environment") {
        let token = parser.current().expect("environment intrinsic");
        let source = token.range();
        let value = StyleSyntaxName::try_new(parser.text_of(token));
        parser.start(SyntaxKind::NameReference, SyntaxRole::Target);
        parser.bump();
        parser.finish();
        PendingStyleName::Authored {
            value,
            dotted_component_count: 1,
            source,
        }
    } else {
        emit_missing_name(
            parser,
            SyntaxRole::Target,
            "syntax.style.environment_name",
            "style environment block requires `environment`",
        )
    };
    bump_trivia_before(parser, enclosing_close);
    let condition = emit_environment_condition(parser, enclosing_close);
    bump_trivia_before(parser, enclosing_close);
    let body = if parser.at("{") {
        emit_environment_body(parser, enclosing_close)
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.environment_body",
            SourceRange::new(at, at),
            "style environment block requires a braced body",
        )));
        PendingStyleBodyProjection::Missing
    };
    parser.finish();
    PendingStyleEnvironmentProjection {
        source_ordinal,
        intrinsic,
        condition,
        body: Box::new(body),
    }
}

fn emit_environment_condition(
    parser: &mut ShadowDocumentParser<'_, '_>,
    enclosing_close: usize,
) -> PendingStyleEnvironmentCondition {
    parser.start(SyntaxKind::StyleEnvironmentCondition, SyntaxRole::Condition);
    let open = if parser.at("(") {
        let source = parser
            .current()
            .expect("environment condition open")
            .range();
        emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
        PendingStylePunctuation::Authored(source)
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(parser, SyntaxKind::OpenParenNode, SyntaxRole::OpenDelimiter);
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.environment_condition",
            SourceRange::new(at, at),
            "style environment condition requires `(`",
        )));
        PendingStylePunctuation::Missing(SourceRange::new(at, at))
    };

    let end = if matches!(open, PendingStylePunctuation::Authored(_)) {
        find_matching_close(parser, parser.cursor(), "(").unwrap_or_else(|| {
            find_top_level_boundary(parser, parser.cursor(), &["{"]).min(enclosing_close)
        })
    } else {
        find_top_level_boundary(parser, parser.cursor(), &[",", ")", "{"]).max(parser.cursor())
    }
    .min(enclosing_close);

    let (clauses, recoveries) = emit_environment_clause_list(parser, end);
    bump_until(parser, end);

    let close = if parser.at(")") {
        let source = parser
            .current()
            .expect("environment condition close")
            .range();
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseParenNode,
            ")",
            "syntax.style.environment_condition_close",
        );
        PendingStylePunctuation::Authored(source)
    } else {
        let at = parser.current_offset();
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseParenNode,
            ")",
            "syntax.style.environment_condition_close",
        );
        PendingStylePunctuation::Missing(SourceRange::new(at, at))
    };
    parser.finish();
    PendingStyleEnvironmentCondition {
        open,
        clauses: clauses.into_boxed_slice(),
        recoveries: recoveries.into_boxed_slice(),
        close,
    }
}

fn emit_environment_clause_list(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
) -> (
    Vec<PendingStyleEnvironmentClause>,
    Vec<PendingStyleEnvironmentConditionRecovery>,
) {
    parser.start(SyntaxKind::FieldList, SyntaxRole::Element(0));
    let mut clauses = Vec::new();
    let mut recoveries = Vec::new();
    bump_trivia_before(parser, end);
    if parser.cursor() >= end {
        let source = SourceRange::new(parser.current_offset(), parser.current_offset());
        emit_environment_condition_recovery(
            parser,
            &mut recoveries,
            StyleEnvironmentConditionIssue::EmptyCondition,
            source,
            false,
        );
    }
    while parser.cursor() < end {
        bump_trivia_before(parser, end);
        if parser.cursor() >= end {
            break;
        }
        if parser.at(",") {
            emit_empty_environment_clause(parser, end, &mut recoveries);
            continue;
        }
        let clause_end = environment_clause_end(parser, parser.cursor(), end);
        let source_ordinal = u16::try_from(clauses.len()).unwrap_or(u16::MAX);
        clauses.push(emit_environment_clause(parser, clause_end, source_ordinal));
        bump_until(parser, clause_end);
        emit_environment_clause_separator(parser, end, &mut recoveries);
    }
    parser.finish();
    (clauses, recoveries)
}

fn emit_empty_environment_clause(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    recoveries: &mut Vec<PendingStyleEnvironmentConditionRecovery>,
) {
    let source = parser
        .current()
        .expect("empty environment clause comma")
        .range();
    let issue = if next_nontrivia(parser, parser.cursor() + 1, end).is_none() {
        StyleEnvironmentConditionIssue::TrailingComma
    } else {
        StyleEnvironmentConditionIssue::EmptyClause
    };
    emit_environment_condition_recovery(parser, recoveries, issue, source, true);
}

fn emit_environment_clause_separator(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    recoveries: &mut Vec<PendingStyleEnvironmentConditionRecovery>,
) {
    if !parser.at(",") {
        return;
    }
    let source = parser.current().expect("environment clause comma").range();
    if next_nontrivia(parser, parser.cursor() + 1, end).is_none() {
        emit_environment_condition_recovery(
            parser,
            recoveries,
            StyleEnvironmentConditionIssue::TrailingComma,
            source,
            true,
        );
    } else {
        parser.bump();
    }
}

fn emit_environment_condition_recovery(
    parser: &mut ShadowDocumentParser<'_, '_>,
    recoveries: &mut Vec<PendingStyleEnvironmentConditionRecovery>,
    issue: StyleEnvironmentConditionIssue,
    source: SourceRange,
    consume: bool,
) {
    let source_ordinal = u32::try_from(recoveries.len()).unwrap_or(u32::MAX);
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(source_ordinal));
    if consume {
        parser.bump();
    } else {
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::IdentifierToken),
            at: source.start(),
        });
    }
    parser.finish();
    let (code, message) = match issue {
        StyleEnvironmentConditionIssue::EmptyCondition => (
            "syntax.style.environment_empty_condition",
            "style environment condition requires at least one clause",
        ),
        StyleEnvironmentConditionIssue::EmptyClause => (
            "syntax.style.environment_empty_clause",
            "style environment condition contains an empty clause",
        ),
        StyleEnvironmentConditionIssue::TrailingComma => (
            "syntax.style.environment_trailing_comma",
            "style environment condition does not allow a trailing comma",
        ),
    };
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code, source, message,
    )));
    recoveries.push(PendingStyleEnvironmentConditionRecovery {
        source_ordinal,
        issue,
        source,
    });
}

fn emit_environment_clause(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    source_ordinal: u16,
) -> PendingStyleEnvironmentClause {
    parser.start(
        SyntaxKind::StyleEnvironmentClause,
        SyntaxRole::Field(source_ordinal),
    );
    let name = emit_style_name(
        parser,
        end,
        SyntaxKind::NameReference,
        SyntaxRole::Name,
        false,
        "syntax.style.environment_field",
        "style environment clause requires a field",
    );
    let field = match &name {
        PendingStyleName::Authored {
            value: Ok(parsed), ..
        } => StyleEnvironmentField::from_source_name(parsed.as_str()).map_or_else(
            || {
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.style.environment_field",
                    pending_name_range(&name),
                    "unsupported style environment field",
                )));
                PendingStyleEnvironmentField::Unsupported(name.clone())
            },
            |value| PendingStyleEnvironmentField::Known {
                value,
                name: name.clone(),
            },
        ),
        PendingStyleName::Authored { .. } => {
            PendingStyleEnvironmentField::Unsupported(name.clone())
        }
        PendingStyleName::Missing { .. } => PendingStyleEnvironmentField::Missing(name.clone()),
    };
    bump_trivia_before(parser, end);

    let comparison = emit_environment_comparison(parser, end);
    bump_trivia_before(parser, end);
    emit_expression(parser, end, SyntaxRole::Value);
    bump_until(parser, end);
    parser.finish();
    PendingStyleEnvironmentClause {
        source_ordinal,
        field,
        comparison,
    }
}

fn emit_environment_comparison(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
) -> PendingStyleEnvironmentComparison {
    let Some(token) = parser.current().filter(|_| parser.cursor() < end) else {
        return emit_missing_environment_comparison(parser);
    };
    let source = token.range();
    let spelling = parser.text_of(token);
    if let Some(value) = StyleEnvironmentComparison::from_source_token(spelling) {
        parser.bump();
        return PendingStyleEnvironmentComparison::Known { value, source };
    }
    if matches!(spelling, "=" | "+=" | "-=") {
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        parser.bump();
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.environment_comparison",
            source,
            "style environment comparison requires `==`, `!=`, `<`, `<=`, `>`, or `>=`",
        )));
        return PendingStyleEnvironmentComparison::Unsupported { source };
    }
    emit_missing_environment_comparison(parser)
}

fn emit_missing_environment_comparison(
    parser: &mut ShadowDocumentParser<'_, '_>,
) -> PendingStyleEnvironmentComparison {
    let at = parser.current_offset();
    emit_missing_delimiter(
        parser,
        SyntaxKind::MissingTokenNode,
        SyntaxRole::Recovery(0),
    );
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.style.environment_comparison",
        SourceRange::new(at, at),
        "style environment clause requires a comparison",
    )));
    PendingStyleEnvironmentComparison::Missing {
        insertion: SourceRange::new(at, at),
    }
}

fn emit_environment_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    enclosing_close: usize,
) -> PendingStyleBodyProjection {
    parser.start(SyntaxKind::StyleBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = find_matching_close(parser, parser.cursor(), "{")
        .unwrap_or(enclosing_close)
        .min(enclosing_close);
    parser.start(SyntaxKind::ItemList, SyntaxRole::Element(0));
    let members = emit_style_members(parser, end, false);
    bump_until(parser, end);
    parser.finish();
    let closed = parser.at("}");
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.style.environment_body_close",
    );
    parser.finish();
    PendingStyleBodyProjection::Braced {
        members: members.into_boxed_slice(),
        closed,
    }
}
