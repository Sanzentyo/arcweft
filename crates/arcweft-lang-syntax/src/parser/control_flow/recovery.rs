use crate::ast::common::TextRange;
use crate::expr::{
    CallRecoveryBoundarySyntax, CallRecoveryTokenKind, Expr, ExprParseError, ExprParseStats,
    ExprRecoveryDiagnostic, MAX_EXPR_DIAGNOSTICS, ParsedExpr, expression_semicolon_ranges_at,
    parse_expr_fragment_recovering_at, parse_expr_fragment_recovering_with_owner_at,
};
use crate::parser::{
    AuthoredExpr, CstStmtKind, ParseError, Stmt, classify_stmt,
    collect_logical_block_items_with_base,
    helpers::{LogicalBlockItem, retain_expr_recovery_diagnostic},
    raw_stmt,
    statements::parse_value_scope_stmt_recovering_with_base,
};

pub(in crate::parser) fn parse_block_expr_recovering_with_base(
    body: &str,
    base: usize,
) -> Result<ParsedExpr, ExprParseError> {
    let body_end = base.checked_add(body.len()).ok_or_else(|| {
        ExprParseError::at(
            "syntax.expr.offset_overflow",
            "callback body range overflowed",
            TextRange::new(base, base),
        )
    })?;
    let lines = collect_logical_block_items_with_base(body, base)
        .into_iter()
        .collect::<Vec<_>>();
    let Some((last, statements)) = lines.split_last() else {
        return Ok(ParsedExpr {
            expr: Expr::Block {
                statements: Vec::new(),
                value: None,
            },
            range: TextRange::new(base, body_end),
            diagnostics: Vec::new(),
            stats: ExprParseStats::default(),
        });
    };
    let mut syntax_stats = crate::cst::SyntaxParseStats::default();
    let mut recovery = CallbackStatementRecovery::default();
    let mut parsed_statements = Vec::with_capacity(statements.len());
    for line in statements {
        let parsed = parse_value_scope_stmt_recovering_with_base(
            line.source.as_ref(),
            &mut syntax_stats,
            line.base,
        )?;
        recovery.absorb(
            parsed.diagnostics,
            parsed.stats,
            statement_range(line.source.as_ref(), line.base)?,
        )?;
        parsed_statements.push(parsed.stmt);
    }
    let (value, diagnostics, stats) = if let Some(parsed) =
        parse_statement_ambiguous_tail(last.source.as_ref(), last.base)
    {
        recovery.absorb(parsed.diagnostics, parsed.stats, parsed.range)?;
        (
            Some(Box::new(parsed.expr)),
            recovery.diagnostics,
            recovery.stats,
        )
    } else {
        let parsed = parse_value_scope_stmt_recovering_with_base(
            last.source.as_ref(),
            &mut syntax_stats,
            last.base,
        )?;
        recovery.absorb(
            parsed.diagnostics,
            parsed.stats,
            statement_range(last.source.as_ref(), last.base)?,
        )?;
        match parsed.stmt {
            Stmt::Expr { expr, .. } => (Some(Box::new(expr)), recovery.diagnostics, recovery.stats),
            statement => {
                parsed_statements.push(statement);
                (None, recovery.diagnostics, recovery.stats)
            }
        }
    };
    Ok(ParsedExpr {
        expr: Expr::Block {
            statements: parsed_statements,
            value,
        },
        range: TextRange::new(base, body_end),
        diagnostics,
        stats,
    })
}

#[derive(Default)]
struct CallbackStatementRecovery {
    diagnostics: Vec<ExprParseError>,
    stats: ExprParseStats,
}

impl CallbackStatementRecovery {
    fn absorb(
        &mut self,
        diagnostics: Vec<ExprParseError>,
        stats: ExprParseStats,
        range: TextRange,
    ) -> Result<(), ExprParseError> {
        let next_diagnostics = self
            .diagnostics
            .len()
            .checked_add(diagnostics.len())
            .ok_or_else(|| {
                ExprParseError::at(
                    "syntax.expr.offset_overflow",
                    "callback diagnostic count overflowed",
                    range,
                )
            })?;
        if next_diagnostics > MAX_EXPR_DIAGNOSTICS {
            return Err(ExprParseError::at(
                "syntax.expr.diagnostic_limit",
                "expression diagnostics exceed the inclusive limit of 128",
                range,
            ));
        }
        self.stats = self.stats.checked_add(stats).ok_or_else(|| {
            ExprParseError::at(
                "syntax.expr.offset_overflow",
                "callback parse statistics overflowed",
                range,
            )
        })?;
        self.diagnostics.extend(diagnostics);
        Ok(())
    }
}

fn statement_range(source: &str, base: usize) -> Result<TextRange, ExprParseError> {
    let end = base.checked_add(source.len()).ok_or_else(|| {
        ExprParseError::at(
            "syntax.expr.offset_overflow",
            "callback statement range overflowed",
            TextRange::new(base, base),
        )
    })?;
    Ok(TextRange::new(base, end))
}

pub(in crate::parser) fn parse_scope_authored_expr_body_recovering_with_base(
    body: &str,
    body_base: usize,
    errors: &mut Vec<ParseError>,
) -> (Vec<Stmt>, Option<AuthoredExpr>) {
    match retain_semicolon_statement_prefix(body, body_base, errors) {
        Ok(Some(parsed)) => return parsed,
        Ok(None) => {}
        Err(error) => {
            retain_fatal_expression_error(&error, errors);
            return (Vec::new(), None);
        }
    }
    let lines = collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .collect::<Vec<_>>();
    let Some((last, statements)) = lines.split_last() else {
        return (Vec::new(), None);
    };
    let mut stats = crate::cst::SyntaxParseStats::default();
    let mut parsed_statements = Vec::with_capacity(statements.len());
    for statement in statements {
        parsed_statements.push(retain_recovering_statement(statement, &mut stats, errors));
    }
    if let Some(parsed) = parse_statement_ambiguous_tail(last.source.as_ref(), last.base) {
        let expr = retain_scope_tail_expr(parsed, &mut stats, errors);
        return (
            parsed_statements,
            Some(authored_block_value_checked(last, expr, errors)),
        );
    }
    match parse_value_scope_stmt_recovering_with_base(last.source.as_ref(), &mut stats, last.base) {
        Ok(parsed) => {
            for diagnostic in &parsed.diagnostics {
                retain_expr_recovery_diagnostic(diagnostic, errors);
            }
            match parsed.stmt {
                Stmt::Expr { expr, .. } => (
                    parsed_statements,
                    Some(authored_block_value_checked(last, expr, errors)),
                ),
                statement => {
                    parsed_statements.push(statement);
                    (parsed_statements, None)
                }
            }
        }
        Err(error) => {
            retain_fatal_expression_error(&error, errors);
            if classify_stmt(last.source.as_ref()) == CstStmtKind::Expr {
                return (
                    parsed_statements,
                    Some(authored_block_value_checked(
                        last,
                        Expr::Raw(last.source.trim().to_owned()),
                        errors,
                    )),
                );
            }
            parsed_statements.push(raw_stmt(last.source.as_ref()));
            (parsed_statements, None)
        }
    }
}

type RecoveredScopeBody = (Vec<Stmt>, Option<AuthoredExpr>);

fn retain_semicolon_statement_prefix(
    body: &str,
    body_base: usize,
    errors: &mut Vec<ParseError>,
) -> Result<Option<RecoveredScopeBody>, ExprParseError> {
    let Some(prefix) = recovered_semicolon_statement_prefix(body, body_base)? else {
        return Ok(None);
    };
    for diagnostic in &prefix.parsed.diagnostics {
        retain_expr_recovery_diagnostic(diagnostic, errors);
    }
    let statement_source = prefix
        .parsed
        .range
        .start()
        .checked_sub(body_base)
        .and_then(|start| {
            prefix
                .parsed
                .range
                .end()
                .checked_sub(body_base)
                .and_then(|end| body.get(start..end))
        })
        .ok_or_else(|| {
            ExprParseError::at(
                "syntax.expr.invalid_scope",
                "recovered statement range is outside its owner body",
                prefix.parsed.range,
            )
        })?
        .to_owned();
    let statement = Stmt::Expr {
        expr: prefix.parsed.expr,
        expr_source: Some(statement_source),
        expr_range: Some(prefix.parsed.range),
    };
    let (mut statements, value) = parse_scope_authored_expr_body_recovering_with_base(
        prefix.remainder,
        prefix.remainder_base,
        errors,
    );
    statements.insert(0, statement);
    Ok(Some((statements, value)))
}

fn retain_recovering_statement(
    line: &LogicalBlockItem<'_>,
    stats: &mut crate::cst::SyntaxParseStats,
    errors: &mut Vec<ParseError>,
) -> Stmt {
    match parse_value_scope_stmt_recovering_with_base(line.source.as_ref(), stats, line.base) {
        Ok(parsed) => {
            for diagnostic in &parsed.diagnostics {
                retain_expr_recovery_diagnostic(diagnostic, errors);
            }
            parsed.stmt
        }
        Err(error) => {
            retain_fatal_expression_error(&error, errors);
            raw_stmt(line.source.as_ref())
        }
    }
}

fn parse_statement_ambiguous_tail(source: &str, base: usize) -> Option<ParsedExpr> {
    let parsed = parse_expr_fragment_recovering_at(
        source,
        base,
        CallRecoveryBoundarySyntax::EndOfExpression,
    )
    .ok()?;
    parsed
        .expr
        .owns_statement_ambiguous_block_tail()
        .then_some(parsed)
}

fn retain_scope_tail_expr(
    parsed: ParsedExpr,
    syntax_stats: &mut crate::cst::SyntaxParseStats,
    errors: &mut Vec<ParseError>,
) -> Expr {
    let ParsedExpr {
        expr,
        range,
        diagnostics,
        stats,
    } = parsed;
    for diagnostic in &diagnostics {
        retain_expr_recovery_diagnostic(diagnostic, errors);
    }
    let Some(numeric_seq_summaries) = syntax_stats
        .numeric_seq_summaries
        .checked_add(stats.numeric_seq_summaries())
    else {
        retain_fatal_expression_error(
            &ExprParseError::at(
                "syntax.expr.offset_overflow",
                "syntax expression parse statistics overflowed",
                range,
            ),
            errors,
        );
        return expr;
    };
    syntax_stats.numeric_seq_summaries = numeric_seq_summaries;
    expr
}

struct RecoveredSemicolonStatement<'a> {
    parsed: ParsedExpr,
    remainder: &'a str,
    remainder_base: usize,
}

fn recovered_semicolon_statement_prefix(
    body: &str,
    body_base: usize,
) -> Result<Option<RecoveredSemicolonStatement<'_>>, ExprParseError> {
    for boundary_range in expression_semicolon_ranges_at(body, body_base)? {
        let boundary_start = boundary_range
            .start()
            .checked_sub(body_base)
            .ok_or_else(|| {
                ExprParseError::at(
                    "syntax.expr.invalid_scope",
                    "statement boundary starts before its owner body",
                    boundary_range,
                )
            })?;
        let boundary_end = boundary_range.end().checked_sub(body_base).ok_or_else(|| {
            ExprParseError::at(
                "syntax.expr.invalid_scope",
                "statement boundary ends before its owner body",
                boundary_range,
            )
        })?;
        let Some(fragment) = body.get(..boundary_start) else {
            return Err(ExprParseError::at(
                "syntax.expr.invalid_scope",
                "statement expression boundary is outside its owner body",
                boundary_range,
            ));
        };
        let Ok(parsed) = parse_expr_fragment_recovering_with_owner_at(
            fragment,
            body_base,
            body,
            body_base,
            CallRecoveryBoundarySyntax::Token {
                kind: CallRecoveryTokenKind::Semicolon,
                range: boundary_range,
            },
        ) else {
            continue;
        };
        let owns_boundary = parsed.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.recovery_diagnostic(),
                Some(ExprRecoveryDiagnostic::MissingCallClose { .. })
            ) && diagnostic.range()
                == TextRange::new(boundary_range.start(), boundary_range.start())
        });
        if !owns_boundary {
            continue;
        }
        let Some(remainder) = body.get(boundary_end..) else {
            return Err(ExprParseError::at(
                "syntax.expr.invalid_scope",
                "statement remainder starts outside its owner body",
                boundary_range,
            ));
        };
        return Ok(Some(RecoveredSemicolonStatement {
            parsed,
            remainder,
            remainder_base: boundary_range.end(),
        }));
    }
    Ok(None)
}

fn retain_fatal_expression_error(error: &ExprParseError, errors: &mut Vec<ParseError>) {
    let mut parsed = ParseError::new(
        error.range(),
        vec!["expression".to_owned()],
        None,
        error.to_string(),
        Vec::new(),
    );
    for related in error.related_ranges() {
        parsed = parsed.with_related(*related, Some("related expression syntax".to_owned()));
    }
    errors.push(parsed);
}

fn authored_block_value_checked(
    item: &LogicalBlockItem<'_>,
    expr: Expr,
    errors: &mut Vec<ParseError>,
) -> AuthoredExpr {
    let range = item
        .base
        .checked_add(item.source.len())
        .map(|end| TextRange::new(item.base, end));
    if range.is_none() {
        errors.push(ParseError::new(
            TextRange::new(item.base, item.base),
            Vec::new(),
            None,
            "authored block value range overflowed".to_owned(),
            Vec::new(),
        ));
    }
    AuthoredExpr::with_source(expr, item.source.as_ref().to_owned(), range)
}
