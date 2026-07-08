use super::SourceDialect;
use super::helpers::LogicalBlockItem;
use super::{
    AuthoredExpr, BorrowBlock, CstBlockEvent, FlowItem, ForBlock, IfBlock, IfLetBlock, LoopBlock,
    MatchArm, MatchBlock, ParseError, Parser, SelectBlock, SelectBranch, SelectBranchHead, Stmt,
    StmtMatchArm, TextRange, WhileBlock, WhileLetBlock, binding_value_start_in_line,
    braced_expr_source, collect_logical_block_items, collect_logical_block_items_with_base,
    indentation, is_typed_stmt, parse_binding_pattern, parse_expr_lossy, parse_pattern, parse_stmt,
    parse_stmt_for_dialect_with_stats_and_base, parse_stmt_with_base, raw_stmt, split_brace_item,
    split_optional_block_label, split_top_level_binding, split_top_level_keyword_once,
    split_top_level_punctuation_once,
};
use crate::cst::{
    ArcweftPunctuation, CstPunctuationScan, split_top_level_arcweft_punctuation_once,
    strip_suffix_arcweft_punctuation,
};
use std::ops::Range;

impl Parser<'_> {
    pub(super) fn parse_let_loop(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing loop expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the loop expression block"],
            );
            return None;
        }

        let head = &block.head;
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
                self.parse_flow_body_from_block(&block, start_line.start + head.len()),
                TextRange::new(start_line.start, block.end),
            ),
        })
    }

    pub(super) fn parse_let_if(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if expression block"],
            );
            return None;
        }
        let head = &block.head;
        let body = &block.body;
        let (then_body, else_body) = split_embedded_else_body(body).map_or_else(
            || {
                self.take_optional_else_block(start_line.start)
                    .map(|else_body| (body.to_string(), else_body))
            },
            Some,
        )?;
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, if_head) = split_top_level_binding(rest)?;
        let condition = if_head.trim().strip_prefix("if")?.trim();

        let (pattern, ty) = parse_binding_pattern(pattern);
        let if_head = if_head.trim();
        let (expr_source, expr_range) = braced_expr_source(
            &block,
            binding_value_start_in_line(&start_line.text, start_line.start, if_head)?,
            if_head,
        );
        Some(Stmt::Let {
            pattern,
            ty,
            expr: crate::expr::Expr::If {
                condition: Box::new(parse_expr_lossy(condition)),
                then_branch: Box::new(parse_block_expr(&then_body)),
                else_branch: Some(Box::new(parse_block_expr(&else_body))),
            },
            expr_source,
            expr_range,
        })
    }

    pub(super) fn parse_let_if_let(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if-let expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if-let expression block"],
            );
            return None;
        }
        let head = &block.head;
        let body = &block.body;
        let (then_body, else_body) = split_embedded_else_body(body).map_or_else(
            || {
                self.take_optional_else_block(start_line.start)
                    .map(|else_body| (body.to_string(), else_body))
            },
            Some,
        )?;
        let rest = head.trim().strip_prefix("let")?.trim();
        let (target_pattern, if_head) = split_top_level_binding(rest)?;
        let if_let_head = if_head.trim().strip_prefix("if let")?.trim();
        let (binding_pattern, value_and_guard) = split_top_level_binding(if_let_head)?;
        let (value, guard) = split_if_let_guard(value_and_guard);

        let (target_pattern, ty) = parse_binding_pattern(target_pattern);
        let if_head = if_head.trim();
        let (expr_source, expr_range) = braced_expr_source(
            &block,
            binding_value_start_in_line(&start_line.text, start_line.start, if_head)?,
            if_head,
        );
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
            expr_source,
            expr_range,
        })
    }

    pub(super) fn parse_let_match(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing match expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the match expression block"],
            );
            return None;
        }
        let head = &block.head;
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, match_head) = split_top_level_binding(rest)?;
        let match_head = match_head.trim();
        let scrutinee = match_head.strip_prefix("match")?.trim();

        let (pattern, ty) = parse_binding_pattern(pattern);
        let (expr_source, expr_range) = braced_expr_source(
            &block,
            binding_value_start_in_line(&start_line.text, start_line.start, match_head)?,
            match_head,
        );
        Some(Stmt::Let {
            pattern,
            ty,
            expr: crate::expr::Expr::Match {
                scrutinee: Box::new(parse_expr_lossy(scrutinee)),
                arms: parse_match_expr_arms(&block.body),
            },
            expr_source,
            expr_range,
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
            Some(body.into_owned())
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
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if body"],
            );
            return None;
        }
        let head = &block.head;
        let head_base = trimmed_line_base(&start_line.text, start_line.start);
        let condition = head.strip_prefix("if")?.trim();
        let body_base = head_base + head.len();
        let (body_items, else_items) =
            if let Some((then_body, else_body)) = split_embedded_else_body(&block.body) {
                (
                    self.parse_flow_body(&then_body, body_base),
                    self.parse_flow_body(&else_body, body_base),
                )
            } else {
                let else_items = self
                    .take_optional_statement_else_items()
                    .unwrap_or_default();
                (
                    self.parse_flow_body_from_block(&block, body_base),
                    else_items,
                )
            };
        Some(IfBlock::new(
            authored_expr_in_source(head, condition, head_base),
            body_items,
            else_items,
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_if_let_block(&mut self) -> Option<IfLetBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if-let",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if-let body"],
            );
            return None;
        }
        let head = &block.head;
        let head_base = trimmed_line_base(&start_line.text, start_line.start);
        let rest = head.trim().strip_prefix("if let")?.trim();
        let (pattern, expr_and_guard) = split_top_level_binding(rest)?;
        let (expr, guard) = split_top_level_keyword_once(expr_and_guard, "when");
        let expr = expr.trim();
        let guard = guard.map(str::trim);
        let body_base = head_base + head.len();
        let (body_items, else_items) =
            if let Some((then_body, else_body)) = split_embedded_else_body(&block.body) {
                (
                    self.parse_flow_body(&then_body, body_base),
                    self.parse_flow_body(&else_body, body_base),
                )
            } else {
                let else_items = self
                    .take_optional_statement_else_items()
                    .unwrap_or_default();
                (
                    self.parse_flow_body_from_block(&block, body_base),
                    else_items,
                )
            };
        Some(IfLetBlock::new(
            parse_pattern(pattern.trim()),
            authored_expr_in_source(head, expr, head_base),
            guard.map(|guard| authored_expr_in_source(head, guard, head_base)),
            body_items,
            else_items,
            TextRange::new(start_line.start, block.end),
        ))
    }

    fn take_optional_statement_else_items(&mut self) -> Option<Vec<FlowItem>> {
        self.skip_blank_and_comments();
        if self.index >= self.events.len() {
            return None;
        }
        let line = self.current().clone();
        let trimmed = line.text.trim_start();
        if !trimmed.starts_with("else") && !trimmed.starts_with("} else") {
            return None;
        }
        if trimmed.starts_with("else if let ") || trimmed.starts_with("} else if let ") {
            return self
                .parse_else_if_let_block()
                .map(|block| vec![FlowItem::IfLet(block)]);
        }
        if trimmed.starts_with("else if ") || trimmed.starts_with("} else if ") {
            return self
                .parse_else_if_block()
                .map(|block| vec![FlowItem::If(block)]);
        }
        let (_, body, _, ok) = self.take_brace_block();
        ok.then(|| self.parse_flow_body(&body, line.start))
    }

    fn parse_else_if_block(&mut self) -> Option<IfBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing else-if",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the else-if body"],
            );
            return None;
        }
        let condition = block
            .head
            .trim()
            .strip_prefix('}')
            .unwrap_or(block.head.trim())
            .trim_start()
            .strip_prefix("else")?
            .trim_start()
            .strip_prefix("if")?
            .trim();
        let head_base = trimmed_line_base(&start_line.text, start_line.start);
        let body_base = head_base + block.head.len();
        let else_body = self
            .take_optional_statement_else_items()
            .unwrap_or_default();
        Some(IfBlock::new(
            authored_expr_in_source(&block.head, condition, head_base),
            self.parse_flow_body_from_block(&block, body_base),
            else_body,
            TextRange::new(start_line.start, block.end),
        ))
    }

    fn parse_else_if_let_block(&mut self) -> Option<IfLetBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing else-if-let",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the else-if-let body"],
            );
            return None;
        }
        let rest = block
            .head
            .trim()
            .strip_prefix('}')
            .unwrap_or(block.head.trim())
            .trim_start()
            .strip_prefix("else")?
            .trim_start()
            .strip_prefix("if let")?
            .trim();
        let (pattern, expr_and_guard) = split_top_level_binding(rest)?;
        let (expr, guard) = split_top_level_keyword_once(expr_and_guard, "when");
        let expr = expr.trim();
        let guard = guard.map(str::trim);
        let head_base = trimmed_line_base(&start_line.text, start_line.start);
        let body_base = head_base + block.head.len();
        let else_body = self
            .take_optional_statement_else_items()
            .unwrap_or_default();
        Some(IfLetBlock::new(
            parse_pattern(pattern.trim()),
            authored_expr_in_source(&block.head, expr, head_base),
            guard.map(|guard| authored_expr_in_source(&block.head, guard, head_base)),
            self.parse_flow_body_from_block(&block, body_base),
            else_body,
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_borrow_block(&mut self) -> Option<BorrowBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing borrow",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the borrow block"],
            );
            return None;
        }
        let head = &block.head;
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
        let body_items = self.parse_flow_body_from_block(&block, start_line.start + head.len());

        Some(BorrowBlock::new(
            parse_expr_lossy(source.trim()),
            binding,
            body_items,
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_match_block(&mut self) -> Option<MatchBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing match",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the match body"],
            );
            return None;
        }
        let head = block.head.as_ref();
        let expr = head.strip_prefix("match")?.trim();
        let head_base = trimmed_line_base(&start_line.text, start_line.start);
        let body_base = block
            .body_range
            .as_ref()
            .map_or(head_base + head.len() + 1, |range| range.start);
        Some(MatchBlock::new(
            authored_expr_in_source(head, expr, head_base),
            parse_match_arms(&block.body, body_base, &mut self.errors),
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_loop_block(&mut self) -> Option<LoopBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing loop",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the loop body"],
            );
            return None;
        }
        let head = &block.head;
        let body_base = start_line.start + head.len();
        let (label, head) = split_optional_block_label(head.trim());
        if head != "loop" {
            return None;
        }
        Some(LoopBlock::new(
            label,
            self.parse_flow_body_from_block(&block, body_base),
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_for_block(&mut self) -> Option<ForBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing for",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the for body"],
            );
            return None;
        }
        let head = &block.head;
        let rest = head.trim().strip_prefix("for")?.trim();
        let (pattern, Some(source)) = split_top_level_keyword_once(rest, "in") else {
            return None;
        };
        let head_base = trimmed_line_base(&start_line.text, start_line.start);
        let body_items = self.parse_flow_body_from_block(&block, head_base + head.len());
        let source = source.trim();
        Some(ForBlock::new(
            parse_pattern(pattern.trim()),
            authored_expr_in_source(head, source, head_base),
            body_items,
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_while_block(&mut self) -> Option<WhileBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing while",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the while body"],
            );
            return None;
        }
        let head = &block.head;
        let head_base = trimmed_line_base(&start_line.text, start_line.start);
        let condition = head.trim().strip_prefix("while")?.trim();
        Some(WhileBlock::new(
            authored_expr_in_source(head, condition, head_base),
            self.parse_flow_body_from_block(&block, head_base + head.len()),
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_while_let_block(&mut self) -> Option<WhileLetBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing while-let",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the while-let body"],
            );
            return None;
        }
        let head = &block.head;
        let head_base = trimmed_line_base(&start_line.text, start_line.start);
        let rest = head.trim().strip_prefix("while let")?.trim();
        let (pattern, expr_and_guard) = split_top_level_binding(rest)?;
        let (expr, guard) = split_top_level_keyword_once(expr_and_guard, "when");
        let expr = expr.trim();
        let guard = guard.map(str::trim);
        Some(WhileLetBlock::new(
            parse_pattern(pattern.trim()),
            authored_expr_in_source(head, expr, head_base),
            guard.map(|guard| authored_expr_in_source(head, guard, head_base)),
            self.parse_flow_body_from_block(&block, head_base + head.len()),
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_select_block(&mut self) -> Option<SelectBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing select",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the select body"],
            );
            return None;
        }
        let head = &block.head;
        if !head.trim().starts_with("select") {
            return None;
        }
        Some(SelectBlock::new(
            self.parse_select_branches_from_block(&block, start_line.start),
            TextRange::new(start_line.start, block.end),
        ))
    }

    fn parse_select_branches_from_block(
        &mut self,
        block: &CstBlockEvent<'_>,
        base: usize,
    ) -> Vec<SelectBranch> {
        if let Some(range) = block.body_line_range.clone() {
            self.parse_select_branches_from_line_range(range, base)
        } else {
            parse_select_branches(&block.body, base, &mut self.errors)
        }
    }

    fn parse_select_branches_from_line_range(
        &mut self,
        range: Range<usize>,
        base: usize,
    ) -> Vec<SelectBranch> {
        let mut branches = Vec::new();
        let mut index = range.start;
        while index < range.end {
            let Some(line) = self.events.get(index).cloned() else {
                break;
            };
            let trimmed = line.trimmed();
            if trimmed.is_empty() {
                index += 1;
                continue;
            }
            let Some(head) = strip_select_branch_suffix(trimmed) else {
                index += 1;
                continue;
            };
            let branch_indent = indentation(line.text());
            index += 1;
            let body_start = index;
            while index < range.end {
                let Some(child) = self.events.get(index) else {
                    break;
                };
                if child.trimmed() == "}" && indentation(child.text()) <= branch_indent {
                    break;
                }
                index += 1;
            }
            let body_end = index;
            if index < range.end {
                index += 1;
            }
            let parsed = if let Some(parsed) =
                self.parse_flow_body_from_line_range(body_start..body_end, base)
            {
                parsed
            } else {
                let body_source = self.collect_line_range_source(body_start..body_end);
                self.parse_flow_body(&body_source, base)
            };
            branches.push(SelectBranch::new(
                parse_select_branch_head(head.trim()),
                parsed,
            ));
        }
        branches
    }
}

fn strip_select_branch_suffix(trimmed: &str) -> Option<&str> {
    let head = trimmed.strip_suffix('{').map_or(trimmed, str::trim_end);
    strip_suffix_arcweft_punctuation(head, ArcweftPunctuation::FatArrow).map(str::trim_end)
}

fn authored_expr_in_source(source: &str, expr_source: &str, base: usize) -> AuthoredExpr {
    let range = source.find(expr_source).map(|start| {
        let absolute_start = base + start;
        TextRange::new(absolute_start, absolute_start + expr_source.len())
    });
    AuthoredExpr::with_source(parse_expr_lossy(expr_source), expr_source.to_owned(), range)
}

fn trimmed_line_base(line: &str, line_base: usize) -> usize {
    line_base + line.len() - line.trim_start().len()
}

fn parse_match_arms(body: &str, body_base: usize, errors: &mut Vec<ParseError>) -> Vec<MatchArm> {
    collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .filter_map(|line| {
            let line_source = line.source.trim();
            if line_source.is_empty() {
                return None;
            }
            let (head, item) = split_top_level_arcweft_punctuation_once(
                line_source,
                ArcweftPunctuation::FatArrow,
            )?;
            let (pattern, guard) = split_pattern_guard(head);
            let item = item.trim();
            let item_base = line
                .source
                .find(item)
                .map_or(line.base, |offset| line.base + offset);
            let parsed = if is_typed_stmt(item) || item.starts_with("let ") {
                vec![FlowItem::Stmt(parse_stmt_with_base(item, item_base))]
            } else {
                let mut nested = Parser::new(item);
                let parsed = nested.parse_flow_item_until_indent(0).map_or_else(
                    || vec![FlowItem::Stmt(parse_stmt_with_base(item, item_base))],
                    |item| vec![item],
                );
                errors.extend(nested.errors.into_iter().map(|err| err.rebased(item_base)));
                parsed
            };
            Some(MatchArm::new(
                parse_pattern(pattern.trim()),
                guard.map(|guard| authored_expr_in_source(head, guard.trim(), line.base)),
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
            let (head, value) =
                split_top_level_arcweft_punctuation_once(line, ArcweftPunctuation::FatArrow)?;
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
        let Some(head) = strip_select_branch_suffix(trimmed) else {
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
        let body_source = body_lines.join("\n");
        let mut nested = Parser::new(&body_source);
        let parsed = nested.parse_flow_body(&body_source, base);
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
    let (statements, value) =
        parse_scope_authored_expr_body_with_base_for_dialect(body, 0, SourceDialect::Game);
    (statements, value.map(|value| value.expr().clone()))
}

pub(super) fn parse_scope_authored_expr_body(body: &str) -> (Vec<Stmt>, Option<AuthoredExpr>) {
    parse_scope_authored_expr_body_with_base_for_dialect(body, 0, SourceDialect::Game)
}

pub(super) fn parse_scope_authored_expr_body_for_dialect(
    body: &str,
    dialect: SourceDialect,
) -> (Vec<Stmt>, Option<AuthoredExpr>) {
    parse_scope_authored_expr_body_with_base_for_dialect(body, 0, dialect)
}

pub(super) fn parse_scope_authored_expr_body_with_base(
    body: &str,
    body_base: usize,
) -> (Vec<Stmt>, Option<AuthoredExpr>) {
    parse_scope_authored_expr_body_with_base_for_dialect(body, body_base, SourceDialect::Game)
}

pub(super) fn parse_scope_authored_expr_body_with_base_for_dialect(
    body: &str,
    body_base: usize,
    dialect: SourceDialect,
) -> (Vec<Stmt>, Option<AuthoredExpr>) {
    let lines = collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .collect::<Vec<_>>();
    let Some((last, statements)) = lines.split_last() else {
        return (Vec::new(), None);
    };
    let mut stats = crate::cst::SyntaxParseStats::default();
    let parsed_statements = statements
        .iter()
        .map(|line| {
            parse_stmt_for_dialect_with_stats_and_base(
                line.source.as_ref(),
                dialect,
                &mut stats,
                line.base,
            )
        })
        .collect::<Vec<_>>();
    if let Some(value) = parse_final_block_expr(last.source.as_ref()) {
        return (parsed_statements, Some(authored_block_value(last, value)));
    }
    if is_typed_stmt(last.source.as_ref()) {
        let mut parsed_statements = parsed_statements;
        parsed_statements.push(parse_stmt_for_dialect_with_stats_and_base(
            last.source.as_ref(),
            dialect,
            &mut stats,
            last.base,
        ));
        (parsed_statements, None)
    } else {
        (
            parsed_statements,
            Some(authored_block_value(
                last,
                parse_expr_lossy(last.source.as_ref()),
            )),
        )
    }
}

fn authored_block_value(item: &LogicalBlockItem<'_>, expr: crate::expr::Expr) -> AuthoredExpr {
    AuthoredExpr::with_source(
        expr,
        item.source.as_ref().to_owned(),
        Some(TextRange::new(item.base, item.base + item.source.len())),
    )
}

pub(super) fn parse_final_block_expr(source: &str) -> Option<crate::expr::Expr> {
    if source.trim_start().starts_with("if ")
        && let Some(expr) = parse_if_expr_source(source)
    {
        return Some(expr);
    }
    if let Some((condition, then_body, else_body)) = split_inline_if_else_expr(source) {
        return Some(crate::expr::Expr::If {
            condition: Box::new(parse_expr_lossy(condition)),
            then_branch: Box::new(parse_block_expr(then_body)),
            else_branch: Some(Box::new(parse_block_expr(else_body))),
        });
    }
    let (head, body) = split_brace_item(source)?;
    if let Some(scrutinee) = head.strip_prefix("match ").map(str::trim) {
        return Some(crate::expr::Expr::Match {
            scrutinee: Box::new(parse_expr_lossy(scrutinee)),
            arms: parse_match_expr_arms(body),
        });
    }
    if let Some(condition) = head.strip_prefix("if ").map(str::trim) {
        let (then_body, else_body) = split_embedded_else_body(body)?;
        return Some(crate::expr::Expr::If {
            condition: Box::new(parse_expr_lossy(condition)),
            then_branch: Box::new(parse_block_expr(&then_body)),
            else_branch: Some(Box::new(parse_block_expr(&else_body))),
        });
    }
    None
}

fn parse_if_expr_source(source: &str) -> Option<crate::expr::Expr> {
    let (head, body, trailing) = split_braced_source_with_trailing(source)?;
    let condition = head.strip_prefix("if ")?;
    Some(crate::expr::Expr::If {
        condition: Box::new(parse_expr_lossy(condition.trim())),
        then_branch: Box::new(parse_block_expr(body)),
        else_branch: Some(Box::new(parse_else_expr_tail(trailing)?)),
    })
}

fn parse_else_expr_tail(trailing: &str) -> Option<crate::expr::Expr> {
    let rest = trailing.trim().strip_prefix("else")?.trim_start();
    if rest.starts_with("if ") {
        return parse_if_expr_source(rest);
    }
    let (head, body, trailing) = split_braced_source_with_trailing(rest)?;
    (head.is_empty() && trailing.trim().is_empty()).then(|| parse_block_expr(body))
}

fn split_braced_source_with_trailing(source: &str) -> Option<(&str, &str, &str)> {
    let punctuation = CstPunctuationScan::new(source);
    let open = punctuation.find_top_level_punctuation('{')?;
    let close = punctuation.find_matching_punctuation(open, '{', '}')?;
    Some((
        source[..open].trim(),
        source[open + '{'.len_utf8()..close].trim(),
        source[close + '}'.len_utf8()..].trim(),
    ))
}

fn split_inline_if_else_expr(source: &str) -> Option<(&str, &str, &str)> {
    let source = source.trim();
    let condition_start = source.strip_prefix("if ")?;
    let open = condition_start.find('{')?;
    let condition = condition_start[..open].trim();
    let body_start = "if ".len() + open + '{'.len_utf8();
    let marker = "\n    } else {";
    let else_marker = source
        .find(marker)
        .or_else(|| source.find("\n} else {"))
        .or_else(|| source.find("} else {"))?;
    let else_body_start = else_marker + source[else_marker..].find('{')? + '{'.len_utf8();
    let else_body_end = source.rfind('}')?;
    (else_body_start <= else_body_end).then_some((
        condition,
        source[body_start..else_marker].trim(),
        source[else_body_start..else_body_end].trim(),
    ))
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
        expr: AuthoredExpr::new(parse_expr_lossy(expr.trim())),
        guard: guard.map(|guard| AuthoredExpr::new(parse_expr_lossy(guard.trim()))),
        body: parse_stmt_lines(body),
    })
}

pub(super) fn parse_stmt_match_arms(body: &str) -> Vec<StmtMatchArm> {
    collect_logical_block_items(body)
        .into_iter()
        .filter_map(|line| {
            let (head, value) = split_top_level_arcweft_punctuation_once(
                line.trim(),
                ArcweftPunctuation::FatArrow,
            )?;
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
