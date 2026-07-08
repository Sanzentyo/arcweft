use super::control_flow::parse_final_block_expr;
use super::headers::parse_required_id_ref;
use super::{
    AuthoredExpr, CstStmtKind, DeferOutcome, Expr, IdRef, ParseError, Parser, RawSyntax,
    RelativeId, RelativeIdSpelling, ScopeExprBlock, Stmt, TextRange, UnsafeAuditInsertion,
    WaitTarget, classify_stmt, parse_binding_pattern, parse_braced_while_let_stmt,
    parse_defer_outcome, parse_expr_lossy, parse_expr_lossy_with_stats,
    parse_expr_with_inline_line_plan_with_stats, parse_memo_block_options, parse_named_block_expr,
    parse_pattern, parse_scope_expr_body, parse_stmt_lines, parse_stmt_match_arms,
    parse_thread_block, parse_trigger_pattern, split_top_level_binding,
    split_top_level_keyword_once,
};
use crate::cst::{
    ArcweftPunctuation, CstBlockEvent, CstPunctuationScan, SyntaxParseStats,
    split_top_level_arcweft_punctuation_once,
};
use crate::{ast::pattern::Pattern, types::TypeRef};

impl Parser<'_> {
    pub(super) fn parse_let_scope(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing scope expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the scope expression block"],
            );
            return None;
        }

        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, scope_head) = split_top_level_binding(rest)?;
        let name = parse_scope_head(scope_head.trim())?;
        let (statements, value) = parse_scope_expr_body(&body);

        Some(Stmt::LetScope {
            pattern: parse_pattern(pattern.trim()),
            scope: ScopeExprBlock::new(
                name.as_option().map(str::to_owned),
                statements,
                value,
                TextRange::new(start_line.start, end),
            ),
        })
    }

    pub(super) fn parse_let_block(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing block expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the block expression"],
            );
            return None;
        }
        let head = &block.head;
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, block_head) = split_top_level_binding(rest)?;
        if !block_head.trim().is_empty() {
            return None;
        }

        let (pattern, ty) = parse_binding_pattern(pattern);
        let (expr_source, expr_range) = braced_expr_source(&block, block_expr_start(&block)?, "");
        Some(Stmt::Let {
            pattern,
            ty,
            expr: super::parse_block_expr(&block.body),
            expr_source,
            expr_range,
        })
    }

    pub(super) fn parse_let_computation_block(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing computation block expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the computation block expression"],
            );
            return None;
        }
        let head = &block.head;
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, block_head) = split_top_level_binding(rest)?;
        let block_head = block_head.trim();
        let kind = super::parse_computation_block_kind(block_head)?;
        let (statements, value) = parse_scope_expr_body(&block.body);

        let (pattern, ty) = parse_binding_pattern(pattern);
        let (expr_source, expr_range) = braced_expr_source(
            &block,
            binding_value_start_in_line(&start_line.text, start_line.start, block_head)?,
            block_head,
        );
        Some(Stmt::Let {
            pattern,
            ty,
            expr: Expr::ComputationBlock {
                kind,
                statements,
                value: value.map(Box::new),
            },
            expr_source,
            expr_range,
        })
    }

    pub(super) fn parse_let_memo_block(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing memo expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the memo expression block"],
            );
            return None;
        }
        let head = &block.head;
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, block_head) = split_top_level_binding(rest)?;
        let block_head = block_head.trim();
        let options = parse_memo_block_options(block_head)?;
        let (statements, value) = parse_scope_expr_body(&block.body);

        let (pattern, ty) = parse_binding_pattern(pattern);
        let (expr_source, expr_range) = braced_expr_source(
            &block,
            binding_value_start_in_line(&start_line.text, start_line.start, block_head)?,
            block_head,
        );
        Some(Stmt::Let {
            pattern,
            ty,
            expr: Expr::MemoBlock {
                options,
                statements,
                value: value.map(Box::new),
            },
            expr_source,
            expr_range,
        })
    }
}

pub(super) fn parse_stmt(trimmed: &str) -> Stmt {
    parse_stmt_inner(trimmed, None, None)
}

pub(super) fn parse_stmt_with_base(trimmed: &str, base: usize) -> Stmt {
    parse_stmt_inner(trimmed, None, Some(base))
}

pub(super) fn parse_stmt_for_dialect(trimmed: &str, dialect: super::SourceDialect) -> Stmt {
    if dialect == super::SourceDialect::Agent && matches!(classify_stmt(trimmed), CstStmtKind::Wait)
    {
        return expr_stmt(parse_expr_lossy(trimmed), None, None);
    }
    parse_stmt(trimmed)
}

pub(super) fn parse_stmt_with_stats_and_base(
    trimmed: &str,
    stats: &mut SyntaxParseStats,
    base: usize,
) -> Stmt {
    parse_stmt_inner(trimmed, Some(stats), Some(base))
}

pub(super) fn parse_stmt_for_dialect_with_stats_and_base(
    trimmed: &str,
    dialect: super::SourceDialect,
    stats: &mut SyntaxParseStats,
    base: usize,
) -> Stmt {
    if dialect == super::SourceDialect::Agent && matches!(classify_stmt(trimmed), CstStmtKind::Wait)
    {
        return expr_stmt(
            parse_expr_lossy_with_stats(trimmed, Some(stats)),
            Some(trimmed.to_owned()),
            Some(TextRange::new(base, base + trimmed.len())),
        );
    }
    parse_stmt_inner(trimmed, Some(stats), Some(base))
}

fn parse_stmt_inner(
    trimmed: &str,
    mut stats: Option<&mut SyntaxParseStats>,
    base: Option<usize>,
) -> Stmt {
    match classify_stmt(trimmed) {
        CstStmtKind::LifetimeSet => {
            let Some((target, expr)) =
                split_top_level_arcweft_punctuation_once(trimmed, ArcweftPunctuation::LeftArrow)
            else {
                return raw_stmt(trimmed);
            };
            let target = target.trim();
            let expr = expr.trim();
            Stmt::LifetimeSet {
                target: authored_expr_in_stmt(trimmed, target, base, stats.as_deref_mut()),
                expr: authored_expr_in_stmt(trimmed, expr, base, stats.as_deref_mut()),
            }
        }
        CstStmtKind::Wait => wait_stmt_source(trimmed).map_or_else(
            || raw_stmt(trimmed),
            |(source, offset)| parse_wait_stmt(source, base.map(|base| base + offset), stats),
        ),
        CstStmtKind::Let => parse_let_stmt(trimmed, stats, base),
        CstStmtKind::DeferBlock | CstStmtKind::Braced | CstStmtKind::UnsafeLifetime => {
            parse_braced_stmt(trimmed, stats, base).unwrap_or_else(|| raw_stmt(trimmed))
        }
        CstStmtKind::Defer => trimmed.strip_prefix("defer ").map_or_else(
            || raw_stmt(trimmed),
            |rest| {
                let source = rest.trim();
                Stmt::Defer {
                    outcome: DeferOutcome::Always,
                    expr: authored_expr_in_stmt(trimmed, source, base, stats.as_deref_mut()),
                }
            },
        ),
        CstStmtKind::ControlTransfer => {
            parse_control_transfer_stmt(trimmed, base).unwrap_or_else(|| raw_stmt(trimmed))
        }
        CstStmtKind::On => parse_on_stmt(trimmed),
        CstStmtKind::AmbiguousBlockHead => raw_stmt(trimmed),
        CstStmtKind::Expr => {
            parse_assign_stmt(trimmed, stats.as_deref_mut(), base).unwrap_or_else(|| {
                expr_stmt(
                    parse_expr_lossy_with_stats(trimmed, stats),
                    Some(trimmed.to_owned()),
                    base.map(|base| TextRange::new(base, base + trimmed.len())),
                )
            })
        }
    }
}

pub(super) fn parse_label_ref(input: &str) -> Option<(String, &str)> {
    let (label, rest) = crate::cst::split_leading_lifetime(input)?;
    Some((label.trim_start_matches('\'').to_owned(), rest))
}

pub(super) fn raw_stmt(source: &str) -> Stmt {
    Stmt::Raw(RawSyntax::stmt(
        source,
        Some(TextRange::new(0, source.len())),
    ))
}

fn parse_let_stmt(
    trimmed: &str,
    mut stats: Option<&mut SyntaxParseStats>,
    base: Option<usize>,
) -> Stmt {
    let Some(rest) = trimmed.strip_prefix("let ") else {
        return raw_stmt(trimmed);
    };
    if let Some((pattern, expr)) = split_top_level_binding(rest) {
        let (pattern, ty) = parse_binding_pattern(pattern);
        let expr = expr.trim();
        let expr_start = trimmed
            .len()
            .checked_sub(expr.len())
            .and_then(|start| base.map(|base| base + start));
        if let Some(value_expr) = parse_final_block_expr(expr) {
            return Stmt::Let {
                pattern,
                ty,
                expr: value_expr,
                expr_source: Some(expr.to_owned()),
                expr_range: expr_start.map(|start| TextRange::new(start, start + expr.len())),
            };
        }
        if let Some(stmt) = parse_inline_let_else_stmt(
            trimmed,
            pattern.clone(),
            ty.clone(),
            expr,
            stats.as_deref_mut(),
            base,
        ) {
            return stmt;
        }
        if ty.is_none()
            && let Some(action) = receive_action_target(expr)
        {
            let action_start = base.and_then(|base| trimmed.find(action).map(|start| base + start));
            return Stmt::LetActionReceive {
                pattern,
                action: AuthoredExpr::with_source(
                    parse_expr_lossy_with_stats(action, stats.as_deref_mut()),
                    action.to_owned(),
                    action_start.map(|start| TextRange::new(start, start + action.len())),
                ),
            };
        }
        Stmt::Let {
            pattern,
            ty,
            expr: parse_expr_with_inline_line_plan_with_stats(expr, stats),
            expr_source: Some(expr.to_owned()),
            expr_range: expr_start.map(|start| TextRange::new(start, start + expr.len())),
        }
    } else {
        raw_stmt(trimmed)
    }
}

fn parse_inline_let_else_stmt(
    stmt_source: &str,
    pattern: Pattern,
    ty: Option<TypeRef>,
    expr: &str,
    mut stats: Option<&mut SyntaxParseStats>,
    base: Option<usize>,
) -> Option<Stmt> {
    let (expr_source, else_tail) = split_top_level_keyword_once(expr, "else");
    let else_tail = else_tail?.trim();
    let expr_source = expr_source.trim();
    if expr_source.is_empty() || else_tail.is_empty() {
        return None;
    }
    let else_tail_start = statement_value_start(stmt_source, else_tail, base);
    let (head, body, body_base) = split_brace_item_with_body_base(else_tail, else_tail_start)?;
    if !head.is_empty() {
        return None;
    }
    let expr_start = statement_value_start(stmt_source, expr_source, base);
    Some(Stmt::LetElse {
        pattern,
        ty,
        expr: AuthoredExpr::with_source(
            parse_expr_lossy_with_stats(expr_source, stats.as_deref_mut()),
            expr_source.to_owned(),
            expr_start.map(|start| TextRange::new(start, start + expr_source.len())),
        ),
        else_body: parse_stmt_lines_with_optional_base(body, stats, body_base),
    })
}

fn receive_action_target(expr: &str) -> Option<&str> {
    expr.trim()
        .strip_prefix("receive action")?
        .trim_start()
        .strip_prefix('(')?
        .trim_end()
        .strip_suffix(')')
        .map(str::trim)
        .filter(|target| !target.is_empty())
}

pub(super) fn binding_value_start_in_line(
    line: &str,
    line_start: usize,
    value_head: &str,
) -> Option<usize> {
    let trimmed = line.trim_start();
    let trimmed_start = line_start + line.len() - trimmed.len();
    let rest = trimmed.strip_prefix("let")?;
    let rest_start = trimmed_start + "let".len();
    let binding = CstPunctuationScan::new(rest).find_top_level_punctuation('=')?;
    let value_with_ws = &rest[binding + '='.len_utf8()..];
    let value = value_with_ws.trim_start();
    let leading = value_with_ws.len() - value.len();
    let value_start = rest_start + binding + '='.len_utf8() + leading;
    value.starts_with(value_head).then_some(value_start)
}

pub(super) fn block_expr_start(block: &CstBlockEvent<'_>) -> Option<usize> {
    block
        .body_range
        .as_ref()
        .and_then(|body_range| body_range.start.checked_sub('{'.len_utf8()))
}

pub(super) fn braced_expr_source(
    block: &CstBlockEvent<'_>,
    expr_start: usize,
    prefix: &str,
) -> (Option<String>, Option<TextRange>) {
    let Some(body_range) = block.body_range.as_ref() else {
        return (None, None);
    };
    let Some(open) = body_range.start.checked_sub('{'.len_utf8()) else {
        return (None, None);
    };
    let Some(end) = body_range.end.checked_add('}'.len_utf8()) else {
        return (None, None);
    };
    let Some(prefix_len) = open.checked_sub(expr_start) else {
        return (None, None);
    };

    let mut source = String::new();
    push_prefix_with_len(&mut source, prefix, prefix_len);
    source.push('{');
    source.push_str(&block.body);
    source.push('}');
    (Some(source), Some(TextRange::new(expr_start, end)))
}

fn push_prefix_with_len(source: &mut String, prefix: &str, target_len: usize) {
    if target_len == 0 {
        return;
    }
    if let Some(prefix) = prefix.get(..target_len) {
        source.push_str(prefix);
    } else {
        source.push_str(prefix);
    }
    while source.len() < target_len {
        source.push(' ');
    }
}

fn parse_assign_stmt(
    trimmed: &str,
    mut stats: Option<&mut SyntaxParseStats>,
    base: Option<usize>,
) -> Option<Stmt> {
    let (target, expr) = split_top_level_binding(trimmed)?;
    let target = target.trim();
    let expr = expr.trim();
    if target.is_empty() || expr.is_empty() {
        return None;
    }
    if target.ends_with(['!', '<', '>', '=']) || expr.starts_with('=') {
        return None;
    }
    let target_start = statement_value_start(trimmed, target, base);
    let expr_start = statement_value_start(trimmed, expr, base);
    Some(Stmt::Assign {
        target: AuthoredExpr::with_source(
            parse_expr_lossy_with_stats(target, stats.as_deref_mut()),
            target.to_owned(),
            target_start.map(|start| TextRange::new(start, start + target.len())),
        ),
        expr: AuthoredExpr::with_source(
            parse_expr_lossy_with_stats(expr, stats),
            expr.to_owned(),
            expr_start.map(|start| TextRange::new(start, start + expr.len())),
        ),
    })
}

fn parse_on_stmt(trimmed: &str) -> Stmt {
    let Some(rest) = trimmed.strip_prefix("on ") else {
        return raw_stmt(trimmed);
    };
    if let Some((head, action)) =
        split_top_level_arcweft_punctuation_once(rest, ArcweftPunctuation::FatArrow)
    {
        Stmt::On {
            trigger: parse_trigger_pattern(head.trim()),
            body: vec![parse_stmt(action.trim())],
        }
    } else {
        raw_stmt(trimmed)
    }
}

fn parse_wait_stmt(
    source: &str,
    start: Option<usize>,
    stats: Option<&mut SyntaxParseStats>,
) -> Stmt {
    let expr = parse_expr_lossy_with_stats(source, stats);
    let value = AuthoredExpr::with_source(
        expr,
        source.to_owned(),
        start.map(|start| TextRange::new(start, start + source.len())),
    );
    match value.expr() {
        Expr::Literal(crate::expr::Literal::Duration { .. }) => {
            Stmt::Wait(WaitTarget::Duration(value))
        }
        _ => Stmt::Wait(WaitTarget::Expr(value)),
    }
}

fn wait_stmt_source(trimmed: &str) -> Option<(&str, usize)> {
    let rest = trimmed.strip_prefix("wait(")?.strip_suffix(')')?;
    let start_trim = rest.len() - rest.trim_start().len();
    let source = rest.trim();
    Some((source, "wait(".len() + start_trim))
}

fn parse_braced_stmt(
    trimmed: &str,
    mut stats: Option<&mut SyntaxParseStats>,
    base: Option<usize>,
) -> Option<Stmt> {
    if trimmed.starts_with("if ") {
        return parse_if_stmt(trimmed, stats, base);
    }
    let (head, body, body_base) = split_brace_item_with_body_base(trimmed, base)?;
    if head.starts_with("unsafe lifetime ") {
        let mut errors = Vec::new();
        return Some(parse_unsafe_lifetime_block(
            head,
            body,
            0,
            None,
            &mut errors,
        ));
    }
    if head.starts_with("thread") {
        return Some(Stmt::Thread(parse_thread_block(head, body)));
    }
    if let Some(outcome) = parse_defer_outcome(head) {
        return Some(Stmt::DeferBlock {
            outcome,
            statements: parse_stmt_lines_with_optional_base(body, stats.as_deref_mut(), body_base),
        });
    }
    if head.starts_with("scope") {
        return Some(expr_stmt(parse_named_block_expr(head, body), None, None));
    }
    if head == "loop" {
        return Some(Stmt::Loop {
            body: parse_stmt_lines_with_optional_base(body, stats.as_deref_mut(), body_base),
        });
    }
    if let Some(stmt) = parse_braced_while_let_stmt(trimmed, head, body, base, body_base) {
        return Some(stmt);
    }
    if let Some(condition) = head.strip_prefix("while ") {
        let condition = condition.trim();
        return Some(Stmt::While {
            condition: authored_expr_in_stmt(trimmed, condition, base, stats.as_deref_mut()),
            body: parse_stmt_lines_with_optional_base(body, stats.as_deref_mut(), body_base),
        });
    }
    if let Some(rest) = head.strip_prefix("for ") {
        let (pattern, Some(source)) = split_top_level_keyword_once(rest, "in") else {
            return Some(raw_stmt(trimmed));
        };
        let source = source.trim();
        return Some(Stmt::For {
            pattern: parse_pattern(pattern.trim()),
            source: authored_expr_in_stmt(trimmed, source, base, stats.as_deref_mut()),
            body: parse_stmt_lines_with_optional_base(body, stats.as_deref_mut(), body_base),
        });
    }
    head.strip_prefix("match ").map(|expr| {
        let expr = expr.trim();
        Stmt::Match {
            expr: authored_expr_in_stmt(trimmed, expr, base, stats),
            arms: parse_stmt_match_arms(body, body_base),
        }
    })
}

fn parse_if_stmt(
    source: &str,
    mut stats: Option<&mut SyntaxParseStats>,
    base: Option<usize>,
) -> Option<Stmt> {
    let (head, body, body_base, trailing, trailing_base) =
        split_braced_stmt_with_trailing_base(source, base)?;
    let condition = head.strip_prefix("if ")?.trim();
    Some(Stmt::If {
        condition: authored_expr_in_stmt(source, condition, base, stats.as_deref_mut()),
        body: parse_stmt_lines_with_optional_base(body, stats.as_deref_mut(), body_base),
        else_body: parse_else_stmt_tail(trailing, stats, trailing_base)?,
    })
}

fn parse_else_stmt_tail(
    trailing: &str,
    stats: Option<&mut SyntaxParseStats>,
    base: Option<usize>,
) -> Option<Vec<Stmt>> {
    let (trailing, base) = trim_with_base(trailing, base);
    if trailing.is_empty() {
        return Some(Vec::new());
    }
    let rest = trailing.strip_prefix("else")?;
    let rest_base = base.map(|base| base + "else".len());
    let (rest, rest_base) = trim_start_with_base(rest, rest_base);
    if rest.starts_with("if ") {
        return parse_if_stmt(rest, stats, rest_base).map(|stmt| vec![stmt]);
    }
    let (head, body, body_base, trailing, _) =
        split_braced_stmt_with_trailing_base(rest, rest_base)?;
    (head.is_empty() && trailing.trim().is_empty())
        .then(|| parse_stmt_lines_with_optional_base(body, stats, body_base))
}

type BracedStmtParts<'a> = (&'a str, &'a str, Option<usize>, &'a str, Option<usize>);

fn split_brace_item_with_body_base(
    source: &str,
    base: Option<usize>,
) -> Option<(&str, &str, Option<usize>)> {
    let punctuation = CstPunctuationScan::new(source);
    let open = punctuation.find_top_level_punctuation('{')?;
    let close = punctuation.find_matching_punctuation(open, '{', '}')?;
    (source[close + '}'.len_utf8()..].trim().is_empty()).then(|| {
        let body = &source[open + '{'.len_utf8()..close];
        let (body, body_base) = trim_with_base(body, base.map(|base| base + open + 1));
        (source[..open].trim(), body, body_base)
    })
}

fn split_braced_stmt_with_trailing_base(
    source: &str,
    base: Option<usize>,
) -> Option<BracedStmtParts<'_>> {
    let punctuation = CstPunctuationScan::new(source);
    let open = punctuation.find_top_level_punctuation('{')?;
    let close = punctuation.find_matching_punctuation(open, '{', '}')?;
    let body = &source[open + '{'.len_utf8()..close];
    let trailing_start = close + '}'.len_utf8();
    let trailing = &source[trailing_start..];
    let (body, body_base) = trim_with_base(body, base.map(|base| base + open + 1));
    let (trailing, trailing_base) =
        trim_with_base(trailing, base.map(|base| base + trailing_start));
    Some((
        source[..open].trim(),
        body,
        body_base,
        trailing,
        trailing_base,
    ))
}

fn parse_stmt_lines_with_optional_base(
    body: &str,
    mut stats: Option<&mut SyntaxParseStats>,
    body_base: Option<usize>,
) -> Vec<Stmt> {
    let Some(body_base) = body_base else {
        return parse_stmt_lines(body);
    };
    super::collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .filter_map(|line| {
            let source = line.source.trim();
            (!source.is_empty())
                .then(|| parse_stmt_inner(source, stats.as_deref_mut(), Some(line.base)))
        })
        .collect()
}

fn authored_expr_in_stmt(
    stmt_source: &str,
    expr_source: &str,
    base: Option<usize>,
    stats: Option<&mut SyntaxParseStats>,
) -> AuthoredExpr {
    let range = base.and_then(|base| {
        stmt_source.find(expr_source).map(|start| {
            let absolute_start = base + start;
            TextRange::new(absolute_start, absolute_start + expr_source.len())
        })
    });
    AuthoredExpr::with_source(
        parse_expr_lossy_with_stats(expr_source, stats),
        expr_source.to_owned(),
        range,
    )
}

fn trim_with_base(source: &str, base: Option<usize>) -> (&str, Option<usize>) {
    let (trimmed, base) = trim_start_with_base(source, base);
    let end = trimmed.trim_end().len();
    (&trimmed[..end], base)
}

fn trim_start_with_base(source: &str, base: Option<usize>) -> (&str, Option<usize>) {
    let trimmed = source.trim_start();
    let leading = source.len() - trimmed.len();
    (trimmed, base.map(|base| base + leading))
}

pub(super) fn parse_unsafe_lifetime_block(
    head: &str,
    body: &str,
    base: usize,
    audit_insertion: Option<UnsafeAuditInsertion>,
    errors: &mut Vec<ParseError>,
) -> Stmt {
    let mut lines = head.lines().map(str::trim).filter(|line| !line.is_empty());
    let first = lines.next().unwrap_or(head.trim());
    let rest = first
        .trim_start()
        .strip_prefix("unsafe lifetime")
        .unwrap_or_default()
        .trim();
    let (id, trailing) = parse_required_id_ref(rest, base, errors).unwrap_or_else(|| {
        (
            IdRef::relative(RelativeId::new(
                "missing".to_owned(),
                0,
                RelativeIdSpelling::DotRun,
                TextRange::new(base, base),
            )),
            "",
        )
    });
    let inline_reason = split_top_level_keyword_once(trailing.trim(), "reason")
        .1
        .and_then(|tail| split_top_level_binding(tail.trim()).map(|(_, expr)| expr.trim()));
    let reason = inline_reason
        .or_else(|| {
            lines.find_map(|line| {
                line.strip_prefix("reason").and_then(|tail| {
                    split_top_level_binding(tail.trim()).map(|(_, expr)| expr.trim())
                })
            })
        })
        .map(parse_expr_lossy);
    let has_safety_doc = body
        .lines()
        .any(|line| line.trim_start().starts_with("/// SAFETY"));
    let executable_body = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("///"))
        .collect::<Vec<_>>()
        .join("\n");
    Stmt::UnsafeLifetime {
        id,
        reason,
        has_safety_doc,
        audit_insertion,
        body: parse_stmt_lines(&executable_body),
    }
}

fn parse_control_transfer_stmt(trimmed: &str, base: Option<usize>) -> Option<Stmt> {
    if trimmed == "break" {
        return Some(Stmt::Break {
            label: None,
            expr: None,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("continue") {
        if rest.trim().is_empty() {
            return Some(Stmt::Continue { label: None });
        }
        let rest = rest.trim();
        return parse_label_ref(rest).and_then(|(label, tail)| {
            tail.trim()
                .is_empty()
                .then_some(Stmt::Continue { label: Some(label) })
        });
    }
    if let Some(rest) = trimmed.strip_prefix("out ") {
        let (label, expr) = split_optional_label_ref(rest.trim());
        let source = expr.trim();
        let start = statement_value_start(trimmed, source, base);
        return Some(Stmt::Out {
            label,
            expr: AuthoredExpr::with_source(
                parse_stmt_value_expr(source),
                source.to_owned(),
                start.map(|start| TextRange::new(start, start + source.len())),
            ),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("break ") {
        let (label, expr) = split_optional_label_ref(rest.trim());
        let source = expr.trim();
        let start = statement_value_start(trimmed, source, base);
        return Some(Stmt::Break {
            label,
            expr: (!source.is_empty()).then(|| {
                AuthoredExpr::with_source(
                    parse_stmt_value_expr(source),
                    source.to_owned(),
                    start.map(|start| TextRange::new(start, start + source.len())),
                )
            }),
        });
    }
    if let Some(source) = trimmed.strip_prefix("return ").map(str::trim) {
        let start = statement_value_start(trimmed, source, base);
        return Some(Stmt::Return {
            expr: parse_stmt_value_expr(source),
            expr_source: Some(source.to_owned()),
            expr_range: start.map(|start| TextRange::new(start, start + source.len())),
        });
    }
    [
        ("goto ", ControlTransferKind::Goto),
        ("yield ", ControlTransferKind::Yield),
        ("close ", ControlTransferKind::Close),
        ("select ", ControlTransferKind::Select),
    ]
    .into_iter()
    .find_map(|(prefix, kind)| {
        let source = trimmed.strip_prefix(prefix).map(str::trim)?;
        let start = statement_value_start(trimmed, source, base);
        let value = AuthoredExpr::with_source(
            parse_stmt_value_expr(source),
            source.to_owned(),
            start.map(|start| TextRange::new(start, start + source.len())),
        );
        Some(kind.into_stmt(value))
    })
}

fn parse_stmt_value_expr(source: &str) -> Expr {
    parse_final_block_expr(source).unwrap_or_else(|| parse_expr_lossy(source))
}

#[derive(Clone, Copy)]
enum ControlTransferKind {
    Goto,
    Yield,
    Close,
    Select,
}

impl ControlTransferKind {
    fn into_stmt(self, value: AuthoredExpr) -> Stmt {
        match self {
            Self::Goto => Stmt::Goto(value),
            Self::Yield => Stmt::Yield(value),
            Self::Close => Stmt::Close(value),
            Self::Select => Stmt::Select(value),
        }
    }
}

fn expr_stmt(expr: Expr, expr_source: Option<String>, expr_range: Option<TextRange>) -> Stmt {
    Stmt::Expr {
        expr,
        expr_source,
        expr_range,
    }
}

fn statement_value_start(trimmed: &str, source: &str, base: Option<usize>) -> Option<usize> {
    trimmed
        .len()
        .checked_sub(source.len())
        .and_then(|offset| base.map(|base| base + offset))
}

fn split_optional_label_ref(input: &str) -> (Option<String>, &str) {
    parse_label_ref(input).map_or((None, input), |(label, tail)| (Some(label), tail))
}

pub(super) enum ParsedScopeName<'a> {
    Named(&'a str),
    Unnamed,
}

impl<'a> ParsedScopeName<'a> {
    pub(super) const fn as_option(&self) -> Option<&'a str> {
        match self {
            Self::Named(name) => Some(name),
            Self::Unnamed => None,
        }
    }
}

pub(super) fn parse_scope_head(source: &str) -> Option<ParsedScopeName<'_>> {
    let rest = source.strip_prefix("scope")?;
    if rest
        .chars()
        .next()
        .is_some_and(|ch| !(ch.is_whitespace() || ch == '{'))
    {
        return None;
    }

    let rest = rest.trim_start();
    if rest.is_empty() || rest.starts_with('{') {
        return Some(ParsedScopeName::Unnamed);
    }

    let name = rest.trim();
    (!name.is_empty()).then_some(ParsedScopeName::Named(name))
}
