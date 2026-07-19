use super::parse_value_scope_stmt_inner;
use crate::cst::SyntaxParseStats;
use crate::expr::{
    CallRecoveryBoundarySyntax, ExprParseError, ExprParseStats, MAX_EXPR_DIAGNOSTICS, ParsedExpr,
    parse_expr_fragment_recovering_at,
};
use crate::parser::control_flow::parse_final_block_expr;
use crate::parser::{
    Expr, Stmt, TextRange, parse_expr_lossy_with_stats,
    parse_expr_with_inline_line_plan_with_stats, parse_named_block_expr,
};

pub(super) enum StmtExprContext<'a> {
    Lossy {
        syntax_stats: Option<&'a mut SyntaxParseStats>,
    },
    Recovering(RecoveringStmtExprState<'a>),
}

pub(super) struct RecoveringStmtExprState<'a> {
    syntax_stats: &'a mut SyntaxParseStats,
    diagnostics: Vec<ExprParseError>,
    stats: ExprParseStats,
    fatal: Option<ExprParseError>,
}

impl<'a> StmtExprContext<'a> {
    pub(super) fn lossy(syntax_stats: Option<&'a mut SyntaxParseStats>) -> Self {
        Self::Lossy { syntax_stats }
    }

    pub(super) fn recovering(syntax_stats: &'a mut SyntaxParseStats) -> Self {
        Self::Recovering(RecoveringStmtExprState {
            syntax_stats,
            diagnostics: Vec::new(),
            stats: ExprParseStats::default(),
            fatal: None,
        })
    }

    pub(super) fn parse(&mut self, source: &str, base: Option<usize>) -> Expr {
        if let Some((head, body, body_base)) = super::split_brace_item_with_body_base(source, base)
            && crate::parser::helpers::is_plain_block_callee(head)
        {
            let kind = crate::parser::parse_computation_block_kind(head);
            let parsed = self.parse_named_block(head, body, body_base);
            if let Some(kind) = kind {
                return match parsed {
                    Expr::NamedBlock {
                        statements, value, ..
                    } => Expr::ComputationBlock {
                        kind,
                        statements,
                        value,
                    },
                    other => other,
                };
            }
            return parsed;
        }
        match self {
            Self::Lossy { syntax_stats } => {
                parse_expr_lossy_with_stats(source, syntax_stats.as_deref_mut())
            }
            Self::Recovering(state) => state.parse(source, base),
        }
    }

    pub(super) fn parse_with_inline_line_plan(
        &mut self,
        source: &str,
        base: Option<usize>,
    ) -> Expr {
        match self {
            Self::Lossy { syntax_stats } => {
                parse_expr_with_inline_line_plan_with_stats(source, syntax_stats.as_deref_mut())
            }
            Self::Recovering(state) => state.parse(source, base),
        }
    }

    pub(super) fn parse_stmt_value(&mut self, source: &str, base: Option<usize>) -> Expr {
        match self {
            Self::Lossy { .. } => {
                parse_final_block_expr(source).unwrap_or_else(|| self.parse(source, base))
            }
            Self::Recovering(_) => self.parse(source, base),
        }
    }

    pub(super) fn parse_final_block(&mut self, source: &str) -> Option<Expr> {
        match self {
            Self::Lossy { .. } => parse_final_block_expr(source),
            Self::Recovering(_) => None,
        }
    }

    pub(super) fn parse_named_block(
        &mut self,
        name: &str,
        body: &str,
        base: Option<usize>,
    ) -> Expr {
        let Self::Recovering(state) = self else {
            return parse_named_block_expr(name, body);
        };
        let Some(base) = base else {
            state.retain_fatal(ExprParseError::at(
                "syntax.expr.invalid_scope",
                "recovering named block requires an owner source range",
                TextRange::new(0, 0),
            ));
            return Expr::Raw(body.to_owned());
        };
        match crate::parser::parse_callback_block_expr_body_recovering_at(body, base) {
            Ok(parsed) => {
                if let Expr::Block { statements, value } = state.retain_parsed(parsed) {
                    Expr::NamedBlock {
                        name: name.to_owned(),
                        statements,
                        value,
                    }
                } else {
                    state.retain_fatal(ExprParseError::at(
                        "syntax.expr.call_invariant",
                        "recovering named-block body did not produce a block expression",
                        TextRange::new(base, base),
                    ));
                    Expr::Raw(body.to_owned())
                }
            }
            Err(error) => {
                state.retain_fatal(error);
                Expr::Raw(body.to_owned())
            }
        }
    }
}

impl RecoveringStmtExprState<'_> {
    fn parse(&mut self, source: &str, base: Option<usize>) -> Expr {
        let trimmed = source.trim();
        let Some(base) = base else {
            self.retain_fatal(ExprParseError::at(
                "syntax.expr.invalid_scope",
                "recovering statement expression requires an owner source range",
                TextRange::new(0, 0),
            ));
            return Expr::Raw(trimmed.to_owned());
        };
        match parse_expr_fragment_recovering_at(
            source,
            base,
            CallRecoveryBoundarySyntax::EndOfExpression,
        ) {
            Ok(parsed) => self.retain_parsed(parsed),
            Err(error) => {
                self.retain_fatal(error);
                Expr::Raw(trimmed.to_owned())
            }
        }
    }

    fn retain_parsed(&mut self, parsed: ParsedExpr) -> Expr {
        let ParsedExpr {
            expr,
            range,
            diagnostics,
            stats,
        } = parsed;
        let next_diagnostic_count = self
            .diagnostics
            .len()
            .checked_add(diagnostics.len())
            .filter(|count| *count <= MAX_EXPR_DIAGNOSTICS);
        if next_diagnostic_count.is_none() {
            self.retain_fatal(ExprParseError::at(
                "syntax.expr.diagnostic_limit",
                "expression diagnostics exceed the inclusive limit of 128",
                range,
            ));
            return expr;
        }
        let Some(next_stats) = self.stats.checked_add(stats) else {
            self.retain_fatal(ExprParseError::at(
                "syntax.expr.offset_overflow",
                "statement expression parse statistics overflowed",
                range,
            ));
            return expr;
        };
        let Some(next_numeric_summaries) = self
            .syntax_stats
            .numeric_seq_summaries
            .checked_add(stats.numeric_seq_summaries())
        else {
            self.retain_fatal(ExprParseError::at(
                "syntax.expr.offset_overflow",
                "syntax expression parse statistics overflowed",
                range,
            ));
            return expr;
        };
        self.diagnostics.extend(diagnostics);
        self.stats = next_stats;
        self.syntax_stats.numeric_seq_summaries = next_numeric_summaries;
        expr
    }

    fn retain_fatal(&mut self, error: ExprParseError) {
        if self.fatal.is_none() {
            self.fatal = Some(error);
        }
    }
}

pub(in crate::parser) struct ParsedRecoveringStmt {
    pub(in crate::parser) stmt: Stmt,
    pub(in crate::parser) diagnostics: Vec<ExprParseError>,
    pub(in crate::parser) stats: ExprParseStats,
}

pub(in crate::parser) fn parse_value_scope_stmt_recovering_with_base(
    trimmed: &str,
    stats: &mut SyntaxParseStats,
    base: usize,
) -> Result<ParsedRecoveringStmt, ExprParseError> {
    let mut expressions = StmtExprContext::recovering(stats);
    let stmt = parse_value_scope_stmt_inner(trimmed, &mut expressions, base);
    finish_recovering_stmt(expressions, stmt, base)
}

pub(in crate::parser) fn parse_stmt_recovering_with_base(
    trimmed: &str,
    stats: &mut SyntaxParseStats,
    base: usize,
) -> Result<ParsedRecoveringStmt, ExprParseError> {
    let mut expressions = StmtExprContext::recovering(stats);
    let stmt = super::parse_stmt_inner(trimmed, &mut expressions, Some(base));
    finish_recovering_stmt(expressions, stmt, base)
}

fn finish_recovering_stmt(
    expressions: StmtExprContext<'_>,
    stmt: Stmt,
    base: usize,
) -> Result<ParsedRecoveringStmt, ExprParseError> {
    let StmtExprContext::Recovering(RecoveringStmtExprState {
        diagnostics,
        stats,
        fatal,
        ..
    }) = expressions
    else {
        return Err(ExprParseError::at(
            "syntax.expr.invalid_scope",
            "recovering statement parser lost its parse mode",
            TextRange::new(base, base),
        ));
    };
    if let Some(error) = fatal {
        return Err(error);
    }
    Ok(ParsedRecoveringStmt {
        stmt,
        diagnostics,
        stats,
    })
}
