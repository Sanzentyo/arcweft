use super::headers::parse_required_id_ref;
use super::{
    CstStmtKind, DeferOutcome, Expr, IdRef, ParseError, Parser, RawSyntax, RelativeId,
    RelativeIdSpelling, ScopeExprBlock, Stmt, TextRange, WaitTarget, classify_stmt,
    parse_binding_pattern, parse_braced_while_let_stmt, parse_defer_outcome, parse_expr_lossy,
    parse_expr_with_inline_line_plan, parse_memo_block_options, parse_named_block_expr,
    parse_pattern, parse_scope_expr_body, parse_stmt_lines, parse_stmt_match_arms,
    parse_thread_block, parse_trigger_pattern, parse_word_scenario_command,
    split_top_level_binding, split_top_level_keyword_once, split_top_level_punctuation_once,
    split_top_level_punctuation_sequence_once,
};

impl Parser {
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
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing block expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the block expression"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, block_head) = split_top_level_binding(rest)?;
        if !block_head.trim().is_empty() {
            return None;
        }

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let {
            pattern,
            ty,
            expr: super::parse_block_expr(&body),
        })
    }

    pub(super) fn parse_let_computation_block(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing computation block expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the computation block expression"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, block_head) = split_top_level_binding(rest)?;
        let kind = super::parse_computation_block_kind(block_head.trim())?;
        let (statements, value) = parse_scope_expr_body(&body);

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let {
            pattern,
            ty,
            expr: Expr::ComputationBlock {
                kind,
                statements,
                value: value.map(Box::new),
            },
        })
    }

    pub(super) fn parse_let_memo_block(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing memo expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the memo expression block"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, block_head) = split_top_level_binding(rest)?;
        let options = parse_memo_block_options(block_head.trim())?;
        let (statements, value) = parse_scope_expr_body(&body);

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let {
            pattern,
            ty,
            expr: Expr::MemoBlock {
                options,
                statements,
                value: value.map(Box::new),
            },
        })
    }
}

pub(super) fn parse_stmt(trimmed: &str) -> Stmt {
    match classify_stmt(trimmed) {
        CstStmtKind::LifetimeSet => {
            let Some((target, expr)) =
                split_top_level_punctuation_sequence_once(trimmed, &["<", "-"])
            else {
                return raw_stmt(trimmed);
            };
            Stmt::LifetimeSet {
                target: parse_expr_lossy(target.trim()),
                expr: parse_expr_lossy(expr.trim()),
            }
        }
        CstStmtKind::Wait => trimmed
            .strip_prefix("wait ")
            .map(str::trim)
            .map_or_else(|| raw_stmt(trimmed), parse_wait_stmt),
        CstStmtKind::Let => parse_let_stmt(trimmed),
        CstStmtKind::DeferBlock | CstStmtKind::Braced | CstStmtKind::UnsafeLifetime => {
            parse_braced_stmt(trimmed).unwrap_or_else(|| raw_stmt(trimmed))
        }
        CstStmtKind::Defer => trimmed.strip_prefix("defer ").map_or_else(
            || raw_stmt(trimmed),
            |rest| Stmt::Defer {
                outcome: DeferOutcome::Always,
                expr: parse_expr_lossy(rest.trim()),
            },
        ),
        CstStmtKind::ControlTransfer => {
            parse_control_transfer_stmt(trimmed).unwrap_or_else(|| raw_stmt(trimmed))
        }
        CstStmtKind::Ensure => parse_ensure_stmt(trimmed),
        CstStmtKind::On => parse_on_stmt(trimmed),
        CstStmtKind::PresentationCall => {
            parse_presentation_special_call(trimmed).map_or_else(|| raw_stmt(trimmed), Stmt::Expr)
        }
        CstStmtKind::ScenarioCommand => {
            parse_word_scenario_command(trimmed, TextRange::new(0, trimmed.len()))
                .map_or_else(|| raw_stmt(trimmed), Stmt::Command)
        }
        CstStmtKind::AmbiguousBlockHead => raw_stmt(trimmed),
        CstStmtKind::Expr => Stmt::Expr(parse_expr_lossy(trimmed)),
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

fn parse_let_stmt(trimmed: &str) -> Stmt {
    let Some(rest) = trimmed.strip_prefix("let ") else {
        return raw_stmt(trimmed);
    };
    if let Some((pattern, expr)) = split_top_level_binding(rest) {
        let (pattern, ty) = parse_binding_pattern(pattern);
        Stmt::Let {
            pattern,
            ty,
            expr: parse_expr_with_inline_line_plan(expr.trim()),
        }
    } else {
        raw_stmt(trimmed)
    }
}

fn parse_ensure_stmt(trimmed: &str) -> Stmt {
    let Some(rest) = trimmed.strip_prefix("ensure ") else {
        return raw_stmt(trimmed);
    };
    if let Some((condition, message)) = split_top_level_punctuation_once(rest, ',') {
        Stmt::Ensure {
            condition: parse_expr_lossy(condition.trim()),
            message: parse_expr_lossy(message.trim()),
        }
    } else {
        raw_stmt(trimmed)
    }
}

fn parse_on_stmt(trimmed: &str) -> Stmt {
    let Some(rest) = trimmed.strip_prefix("on ") else {
        return raw_stmt(trimmed);
    };
    if let Some((head, action)) = split_top_level_punctuation_sequence_once(rest, &["=", ">"]) {
        Stmt::On {
            trigger: parse_trigger_pattern(head.trim()),
            body: vec![parse_stmt(action.trim())],
        }
    } else {
        raw_stmt(trimmed)
    }
}

fn parse_wait_stmt(rest: &str) -> Stmt {
    if let Some(name) = rest.strip_prefix("mark ") {
        return Stmt::Wait(WaitTarget::Mark(name.trim().to_owned()));
    }
    let expr = parse_expr_lossy(rest);
    match expr {
        Expr::Literal(crate::expr::Literal::Duration { .. }) => {
            Stmt::Wait(WaitTarget::Duration(expr))
        }
        _ => Stmt::Wait(WaitTarget::Expr(expr)),
    }
}

fn parse_braced_stmt(trimmed: &str) -> Option<Stmt> {
    let (head, body) = super::split_brace_item(trimmed)?;
    if head.starts_with("unsafe lifetime ") {
        let mut errors = Vec::new();
        return Some(parse_unsafe_lifetime_block(head, body, 0, &mut errors));
    }
    if head.starts_with("thread") {
        return Some(Stmt::Thread(parse_thread_block(head, body)));
    }
    if let Some(outcome) = parse_defer_outcome(head) {
        return Some(Stmt::DeferBlock {
            outcome,
            statements: parse_stmt_lines(body),
        });
    }
    if head.starts_with("scope") {
        return Some(Stmt::Expr(parse_named_block_expr(head, body)));
    }
    if let Some(condition) = head.strip_prefix("if ") {
        return Some(Stmt::If {
            condition: parse_expr_lossy(condition.trim()),
            body: parse_stmt_lines(body),
        });
    }
    if head == "loop" {
        return Some(Stmt::Loop {
            body: parse_stmt_lines(body),
        });
    }
    if let Some(stmt) = parse_braced_while_let_stmt(head, body) {
        return Some(stmt);
    }
    if let Some(condition) = head.strip_prefix("while ") {
        return Some(Stmt::While {
            condition: parse_expr_lossy(condition.trim()),
            body: parse_stmt_lines(body),
        });
    }
    if let Some(rest) = head.strip_prefix("for ") {
        let (pattern, Some(source)) = split_top_level_keyword_once(rest, "in") else {
            return Some(raw_stmt(trimmed));
        };
        return Some(Stmt::For {
            pattern: parse_pattern(pattern.trim()),
            source: parse_expr_lossy(source.trim()),
            body: parse_stmt_lines(body),
        });
    }
    head.strip_prefix("match ").map(|expr| Stmt::Match {
        expr: parse_expr_lossy(expr.trim()),
        arms: parse_stmt_match_arms(body),
    })
}

pub(super) fn parse_unsafe_lifetime_block(
    head: &str,
    body: &str,
    base: usize,
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
        body: parse_stmt_lines(&executable_body),
    }
}

fn parse_control_transfer_stmt(trimmed: &str) -> Option<Stmt> {
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
        return Some(Stmt::Out {
            label,
            expr: parse_expr_lossy(expr.trim()),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("break ") {
        let (label, expr) = split_optional_label_ref(rest.trim());
        return Some(Stmt::Break {
            label,
            expr: (!expr.trim().is_empty()).then(|| parse_expr_lossy(expr.trim())),
        });
    }
    [
        ("return ", Stmt::Return as fn(Expr) -> Stmt),
        ("goto ", Stmt::Goto),
        ("yield ", Stmt::Yield),
        ("panic ", Stmt::Panic),
        ("fail ", Stmt::Fail),
        ("bail ", Stmt::Bail),
        ("close ", Stmt::Close),
        ("select ", Stmt::Select),
    ]
    .into_iter()
    .find_map(|(prefix, build)| {
        trimmed
            .strip_prefix(prefix)
            .map(str::trim)
            .map(parse_expr_lossy)
            .map(build)
    })
}

fn split_optional_label_ref(input: &str) -> (Option<String>, &str) {
    parse_label_ref(input).map_or((None, input), |(label, tail)| (Some(label), tail))
}

fn parse_presentation_special_call(source: &str) -> Option<crate::expr::Expr> {
    let trimmed = source.trim();
    let (callee, call_source) = if let Some(rest) = trimmed.strip_prefix("ref bg") {
        ("ref.bg", rest)
    } else if let Some(rest) = trimmed.strip_prefix("ref show") {
        ("ref.show", rest)
    } else if let Some(rest) = trimmed.strip_prefix("clear bg") {
        ("clear.bg", rest)
    } else {
        return None;
    };
    if !call_source.trim_start().starts_with('(') {
        return None;
    }
    let crate::expr::Expr::Call { args, .. } = parse_expr_lossy(&format!("_{call_source}")) else {
        return None;
    };
    Some(crate::expr::Expr::Call {
        callee: Box::new(crate::expr::Expr::Path(callee.to_owned())),
        args,
    })
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
