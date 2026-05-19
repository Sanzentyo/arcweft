use super::{
    BorrowBlock, FlowItem, ForBlock, IfBlock, IfLetBlock, LoopBlock, MatchArm, MatchBlock,
    ParseError, Parser, SelectBlock, SelectBranch, SelectBranchHead, Stmt, StmtMatchArm, TextRange,
    WhileBlock, WhileLetBlock, collect_logical_block_items, indentation, is_typed_stmt,
    parse_binding_pattern, parse_expr_lossy, parse_pattern, parse_stmt, raw_stmt, split_brace_item,
    split_optional_block_label, split_top_level_binding, split_top_level_keyword_once,
    split_top_level_punctuation_once, split_top_level_punctuation_sequence_once,
};

impl Parser {
    pub(super) fn parse_let_loop(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing loop expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the loop expression block"],
            );
            return None;
        }

        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, loop_head) = split_top_level_binding(rest)?;
        let (label, loop_head) = split_optional_block_label(loop_head.trim());
        if loop_head != "loop" {
            return None;
        }

        Some(Stmt::LetLoop {
            pattern: parse_pattern(pattern.trim()),
            block: LoopBlock::new(
                label,
                self.parse_flow_body(&body, start_line.start + head.len()),
                TextRange::new(start_line.start, end),
            ),
        })
    }

    pub(super) fn parse_let_if(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if expression block"],
            );
            return None;
        }
        let (then_body, else_body) = split_embedded_else_body(&body).map_or_else(
            || {
                self.take_optional_else_block(start_line.start)
                    .map(|else_body| (body, else_body))
            },
            Some,
        )?;
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, if_head) = split_top_level_binding(rest)?;
        let condition = if_head.trim().strip_prefix("if")?.trim();

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let {
            pattern,
            ty,
            expr: crate::expr::Expr::If {
                condition: Box::new(parse_expr_lossy(condition)),
                then_branch: Box::new(parse_block_expr(&then_body)),
                else_branch: Some(Box::new(parse_block_expr(&else_body))),
            },
        })
    }

    pub(super) fn parse_let_if_let(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if-let expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if-let expression block"],
            );
            return None;
        }
        let (then_body, else_body) = split_embedded_else_body(&body).map_or_else(
            || {
                self.take_optional_else_block(start_line.start)
                    .map(|else_body| (body, else_body))
            },
            Some,
        )?;
        let rest = head.trim().strip_prefix("let")?.trim();
        let (target_pattern, if_head) = split_top_level_binding(rest)?;
        let if_let_head = if_head.trim().strip_prefix("if let")?.trim();
        let (binding_pattern, value_and_guard) = split_top_level_binding(if_let_head)?;
        let (value, guard) = split_if_let_guard(value_and_guard);

        let (target_pattern, ty) = parse_binding_pattern(target_pattern);
        Some(Stmt::Let {
            pattern: target_pattern,
            ty,
            expr: crate::expr::Expr::IfLet {
                pattern: Box::new(parse_pattern(binding_pattern.trim())),
                expr: Box::new(parse_expr_lossy(value.trim())),
                guard: guard.map(|guard| Box::new(parse_expr_lossy(guard.trim()))),
                then_branch: Box::new(parse_block_expr(&then_body)),
                else_branch: Some(Box::new(parse_block_expr(&else_body))),
            },
        })
    }

    pub(super) fn parse_let_match(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing match expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the match expression block"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, match_head) = split_top_level_binding(rest)?;
        let scrutinee = match_head.trim().strip_prefix("match")?.trim();

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let {
            pattern,
            ty,
            expr: crate::expr::Expr::Match {
                scrutinee: Box::new(parse_expr_lossy(scrutinee)),
                arms: parse_match_expr_arms(&body),
            },
        })
    }

    fn take_optional_else_block(&mut self, base: usize) -> Option<String> {
        self.skip_blank_and_comments();
        if self.index >= self.events.len() {
            self.push_error(
                TextRange::new(base, self.previous_end()),
                "value-producing if expression requires else",
                ["else { ... }"],
                None,
                ["add an else block or use statement-style if"],
            );
            return None;
        }
        let line = self.current().clone();
        if !line.text.trim_start().starts_with("else") {
            self.push_error(
                TextRange::new(line.start, line.end),
                "value-producing if expression requires else",
                ["else { ... }"],
                Some(line.text.trim()),
                ["add an else block before the next statement"],
            );
            return None;
        }
        let (_, body, _, ok) = self.take_brace_block();
        if ok {
            Some(body)
        } else {
            self.push_error(
                TextRange::new(line.start, line.end),
                "unclosed else block while parsing if expression",
                ["}"],
                Some(line.text.trim()),
                ["insert a closing `}` for the else block"],
            );
            None
        }
    }

    pub(super) fn parse_let_else(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing let-else",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the let-else block"],
            );
            return None;
        }

        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, rhs) = split_top_level_binding(rest)?;
        let expr = rhs.trim().strip_suffix("else")?.trim();
        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::LetElse {
            pattern,
            ty,
            expr: parse_expr_lossy(expr),
            else_body: parse_stmt_lines(&body),
        })
    }

    pub(super) fn parse_if_block(&mut self) -> Option<IfBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if body"],
            );
            return None;
        }
        let condition = head.strip_prefix("if")?.trim();
        let body_items = self.parse_flow_body(&body, start_line.start + head.len());
        Some(IfBlock::new(
            parse_expr_lossy(condition),
            body_items,
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_if_let_block(&mut self) -> Option<IfLetBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if-let",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if-let body"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("if let")?.trim();
        let (pattern, expr_and_guard) = split_top_level_binding(rest)?;
        let (expr, guard) = split_top_level_keyword_once(expr_and_guard, "when");
        let guard = guard.map(|guard| parse_expr_lossy(guard.trim()));
        Some(IfLetBlock::new(
            parse_pattern(pattern.trim()),
            parse_expr_lossy(expr),
            guard,
            self.parse_flow_body(&body, start_line.start + head.len()),
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_borrow_block(&mut self) -> Option<BorrowBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing borrow",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the borrow block"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("borrow")?.trim();
        let (source, Some(binding)) = split_top_level_keyword_once(rest, "as") else {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "borrow block must bind a typed alias",
                ["borrow expr as name: Type { ... }"],
                Some(head.trim()),
                ["write the borrow block as `borrow source as name: Type { ... }`"],
            );
            return None;
        };
        let Some((name, ty)) = split_top_level_punctuation_once(binding, ':') else {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "borrow binding must declare a type",
                ["name: Type"],
                Some(binding.trim()),
                ["add the borrowed reference type after the alias name"],
            );
            return None;
        };
        let binding = parse_pattern(&format!("{}: {}", name.trim(), ty.trim()));
        let body_items = self.parse_flow_body(&body, start_line.start + head.len());

        Some(BorrowBlock::new(
            parse_expr_lossy(source.trim()),
            binding,
            body_items,
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_match_block(&mut self) -> Option<MatchBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing match",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the match body"],
            );
            return None;
        }
        let expr = head.strip_prefix("match")?.trim();
        Some(MatchBlock::new(
            parse_expr_lossy(expr),
            parse_match_arms(&body, start_line.start, &mut self.errors),
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_loop_block(&mut self) -> Option<LoopBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing loop",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the loop body"],
            );
            return None;
        }
        let body_base = start_line.start + head.len();
        let (label, head) = split_optional_block_label(head.trim());
        if head != "loop" {
            return None;
        }
        Some(LoopBlock::new(
            label,
            self.parse_flow_body(&body, body_base),
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_for_block(&mut self) -> Option<ForBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing for",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the for body"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("for")?.trim();
        let (pattern, Some(source)) = split_top_level_keyword_once(rest, "in") else {
            return None;
        };
        let body_items = self.parse_flow_body(&body, start_line.start + head.len());
        Some(ForBlock::new(
            parse_pattern(pattern.trim()),
            parse_expr_lossy(source.trim()),
            body_items,
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_while_block(&mut self) -> Option<WhileBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing while",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the while body"],
            );
            return None;
        }
        let condition = head.trim().strip_prefix("while")?.trim();
        Some(WhileBlock::new(
            parse_expr_lossy(condition),
            self.parse_flow_body(&body, start_line.start + head.len()),
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_while_let_block(&mut self) -> Option<WhileLetBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing while-let",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the while-let body"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("while let")?.trim();
        let (pattern, expr_and_guard) = split_top_level_binding(rest)?;
        let (expr, guard) = split_top_level_keyword_once(expr_and_guard, "when");
        let guard = guard.map(|guard| parse_expr_lossy(guard.trim()));
        Some(WhileLetBlock::new(
            parse_pattern(pattern.trim()),
            parse_expr_lossy(expr),
            guard,
            self.parse_flow_body(&body, start_line.start + head.len()),
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_select_block(&mut self) -> Option<SelectBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing select",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the select body"],
            );
            return None;
        }
        if !head.trim().starts_with("select") {
            return None;
        }
        Some(SelectBlock::new(
            parse_select_branches(&body, start_line.start, &mut self.errors),
            TextRange::new(start_line.start, end),
        ))
    }
}

fn parse_match_arms(body: &str, base: usize, errors: &mut Vec<ParseError>) -> Vec<MatchArm> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let (head, item) = split_top_level_punctuation_sequence_once(line, &["=", ">"])?;
            let (pattern, guard) = split_pattern_guard(head);
            let mut nested = Parser::new(item.trim().to_owned());
            let parsed = nested.parse_flow_item_until_indent(0).map_or_else(
                || vec![FlowItem::Stmt(parse_stmt(item.trim()))],
                |item| vec![item],
            );
            errors.extend(nested.errors.into_iter().map(|err| err.rebased(base)));
            Some(MatchArm::new(
                parse_pattern(pattern.trim()),
                guard.map(|guard| parse_expr_lossy(guard.trim())),
                parsed,
            ))
        })
        .collect()
}

fn parse_match_expr_arms(body: &str) -> Vec<crate::expr::MatchExprArm> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let (head, value) = split_top_level_punctuation_sequence_once(line, &["=", ">"])?;
            let (pattern, guard) = split_pattern_guard(head);
            Some(crate::expr::MatchExprArm::new(
                parse_pattern(pattern.trim()),
                guard.map(|guard| Box::new(parse_expr_lossy(guard.trim()))),
                Box::new(parse_match_arm_value(value.trim())),
            ))
        })
        .collect()
}

pub(super) fn split_pattern_guard(source: &str) -> (&str, Option<&str>) {
    split_top_level_keyword_once(source, "when")
}

fn parse_match_arm_value(source: &str) -> crate::expr::Expr {
    source
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .map_or_else(|| parse_expr_lossy(source), parse_block_expr)
}

fn parse_select_branches(
    body: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Vec<SelectBranch> {
    let lines = body.lines().collect::<Vec<_>>();
    let mut branches = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        let Some(head) = trimmed
            .strip_suffix("=> {")
            .or_else(|| trimmed.strip_suffix("=>"))
        else {
            index += 1;
            continue;
        };
        let branch_indent = indentation(line);
        index += 1;
        let mut body_lines = Vec::new();
        while index < lines.len() {
            let child = lines[index];
            let child_trimmed = child.trim();
            if child_trimmed == "}" && indentation(child) <= branch_indent {
                index += 1;
                break;
            }
            body_lines.push(child);
            index += 1;
        }
        let mut nested = Parser::new(body_lines.join("\n"));
        let parsed = nested.parse_flow_body(&body_lines.join("\n"), base);
        errors.extend(nested.errors.into_iter().map(|err| err.rebased(base)));
        branches.push(SelectBranch::new(
            parse_select_branch_head(head.trim()),
            parsed,
        ));
    }
    branches
}

fn parse_select_branch_head(source: &str) -> SelectBranchHead {
    if let Some(rest) = source.strip_prefix("frame ") {
        return SelectBranchHead::Frame(parse_pattern(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("event ") {
        return SelectBranchHead::Event(parse_pattern(rest.trim()));
    }
    if let Some((name, source)) = split_top_level_binding(source) {
        let source = source.trim();
        let propagates_error = source.ends_with('?');
        return SelectBranchHead::Bind {
            name: name.trim().to_owned(),
            source: parse_expr_lossy(source.trim_end_matches('?').trim()),
            propagates_error,
        };
    }
    SelectBranchHead::Raw(source.to_owned())
}

fn split_if_let_guard(source: &str) -> (&str, Option<&str>) {
    split_top_level_keyword_once(source, "when")
}

pub(super) fn parse_scope_expr_body(body: &str) -> (Vec<Stmt>, Option<crate::expr::Expr>) {
    let lines = collect_logical_block_items(body)
        .into_iter()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let Some((last, statements)) = lines.split_last() else {
        return (Vec::new(), None);
    };
    let parsed_statements = statements
        .iter()
        .map(|line| parse_stmt(line.as_str()))
        .collect::<Vec<_>>();
    if let Some(value) = parse_final_block_expr(last.as_str()) {
        return (parsed_statements, Some(value));
    }
    if is_typed_stmt(last) {
        let mut parsed_statements = parsed_statements;
        parsed_statements.push(parse_stmt(last.as_str()));
        (parsed_statements, None)
    } else {
        (parsed_statements, Some(parse_expr_lossy(last.as_str())))
    }
}

fn parse_final_block_expr(source: &str) -> Option<crate::expr::Expr> {
    let (head, body) = split_brace_item(source)?;
    head.strip_prefix("match ")
        .map(str::trim)
        .map(|scrutinee| crate::expr::Expr::Match {
            scrutinee: Box::new(parse_expr_lossy(scrutinee)),
            arms: parse_match_expr_arms(body),
        })
}

pub(super) fn parse_block_expr(body: &str) -> crate::expr::Expr {
    let (statements, value) = parse_scope_expr_body(body);
    crate::expr::Expr::Block {
        statements,
        value: value.map(Box::new),
    }
}

pub(super) fn parse_named_block_expr(name: &str, body: &str) -> crate::expr::Expr {
    let (statements, value) = parse_scope_expr_body(body);
    crate::expr::Expr::NamedBlock {
        name: name.to_owned(),
        statements,
        value: value.map(Box::new),
    }
}

fn split_embedded_else_body(body: &str) -> Option<(String, String)> {
    let mut then_lines = Vec::new();
    let mut else_lines = Vec::new();
    let mut in_else = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if matches!(trimmed, "} else {" | "} else{") {
            in_else = true;
            continue;
        }
        if in_else {
            else_lines.push(line);
        } else {
            then_lines.push(line);
        }
    }
    in_else.then(|| (then_lines.join("\n"), else_lines.join("\n")))
}

pub(super) fn parse_stmt_lines(body: &str) -> Vec<Stmt> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .map(|line| parse_stmt(&line))
        .collect()
}

pub(super) fn parse_braced_while_let_stmt(head: &str, body: &str) -> Option<Stmt> {
    let rest = head.strip_prefix("while let ")?;
    let Some((pattern, expr_and_guard)) = split_top_level_binding(rest) else {
        return Some(raw_stmt(&format!("{head} {{ {body} }}")));
    };
    let (expr, guard) = split_pattern_guard(expr_and_guard.trim());
    Some(Stmt::WhileLet {
        pattern: parse_pattern(pattern.trim()),
        expr: parse_expr_lossy(expr.trim()),
        guard: guard.map(|guard| parse_expr_lossy(guard.trim())),
        body: parse_stmt_lines(body),
    })
}

pub(super) fn parse_stmt_match_arms(body: &str) -> Vec<StmtMatchArm> {
    collect_logical_block_items(body)
        .into_iter()
        .filter_map(|line| {
            let (head, value) =
                split_top_level_punctuation_sequence_once(line.trim(), &["=", ">"])?;
            let (pattern, guard) = split_pattern_guard(head.trim());
            let body = value
                .trim()
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
                .map_or_else(
                    || vec![parse_stmt(value.trim())],
                    |block| parse_stmt_lines(block.trim()),
                );
            Some(StmtMatchArm::new(
                parse_pattern(pattern.trim()),
                guard.map(|guard| parse_expr_lossy(guard.trim())),
                body,
            ))
        })
        .collect()
}
