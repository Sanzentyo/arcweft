use super::{Expr, ExprOp, ExprParseError, ExprParser, MatchExprArm, Token, parse_expr};
use crate::pattern::parse_pattern;

impl ExprParser {
    pub(super) fn parse_if_expr_after_keyword(&mut self) -> Result<Expr, ExprParseError> {
        if self.peek_ident("let") {
            self.bump();
            return self.parse_if_let_expr_after_keywords();
        }
        let condition = self.parse_control_head_expr()?;
        let then_branch = self.parse_braced_value_expr()?;
        let else_branch = self.parse_optional_else_expr()?;
        Ok(Expr::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch,
        })
    }

    fn parse_if_let_expr_after_keywords(&mut self) -> Result<Expr, ExprParseError> {
        let pattern_start = self.cursor;
        while !matches!(self.peek(), Token::Op(ExprOp::Assign) | Token::Eof) {
            self.bump();
        }
        let pattern_end = self.cursor;
        self.expect(&Token::Op(ExprOp::Assign))?;
        let pattern_source = self.token_range_source(pattern_start, pattern_end);
        let expr = self.parse_control_head_expr()?;
        let guard = if self.peek_ident("when") {
            self.bump();
            Some(Box::new(self.parse_control_head_expr()?))
        } else {
            None
        };
        let then_branch = self.parse_braced_value_expr()?;
        let else_branch = self.parse_optional_else_expr()?;
        Ok(Expr::IfLet {
            pattern: Box::new(parse_pattern(pattern_source.trim())),
            expr: Box::new(expr),
            guard,
            then_branch: Box::new(then_branch),
            else_branch,
        })
    }

    fn parse_optional_else_expr(&mut self) -> Result<Option<Box<Expr>>, ExprParseError> {
        if !self.peek_ident("else") {
            return Ok(None);
        }
        self.bump();
        let else_branch = if self.peek_ident("if") || self.peek_ident("match") {
            self.parse_expr_bp(0)?
        } else {
            self.parse_braced_value_expr()?
        };
        Ok(Some(Box::new(else_branch)))
    }

    pub(super) fn parse_match_expr_after_keyword(&mut self) -> Result<Expr, ExprParseError> {
        let scrutinee = self.parse_control_head_expr()?;
        self.expect(&Token::LBrace)?;
        let mut arms = Vec::new();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            arms.push(self.parse_match_expr_arm()?);
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        })
    }

    fn parse_match_expr_arm(&mut self) -> Result<MatchExprArm, ExprParseError> {
        let head_start = self.cursor;
        let mut guard_start = None;
        let mut depth = 0usize;
        while !matches!(self.peek(), Token::Eof) {
            match self.peek() {
                Token::Op(ExprOp::FatArrow) if depth == 0 => break,
                Token::Ident(keyword) if keyword == "when" && depth == 0 => {
                    guard_start.get_or_insert(self.cursor);
                }
                Token::LParen | Token::LBracket | Token::LBrace => depth += 1,
                Token::RParen | Token::RBracket | Token::RBrace => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
            self.bump();
        }
        let head_end = self.cursor;
        self.expect(&Token::Op(ExprOp::FatArrow))?;
        let pattern_end = guard_start.unwrap_or(head_end);
        let pattern_source = self.token_range_source(head_start, pattern_end);
        let guard = guard_start.map(|guard_start| {
            Box::new(
                parse_expr(self.token_range_source(guard_start + 1, head_end).trim())
                    .unwrap_or_else(|_| {
                        Expr::Raw(self.token_range_source(guard_start + 1, head_end))
                    }),
            )
        });
        let value = if self.peek() == &Token::LBrace {
            self.parse_braced_value_expr()?
        } else {
            self.parse_expr_bp(0)?
        };
        Ok(MatchExprArm::new(
            parse_pattern(pattern_source.trim()),
            guard,
            Box::new(value),
        ))
    }

    fn parse_braced_value_expr(&mut self) -> Result<Expr, ExprParseError> {
        let open = self.peek_lexed().clone();
        let open_range = self.absolute_range(&open)?;
        self.expect(&Token::LBrace)?;
        let mut body_end = self.cursor;
        let mut depth = 1usize;
        while depth > 0 {
            let token_index = self.cursor;
            match self.bump_lexed().token {
                Token::LBrace => depth += 1,
                Token::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        body_end = token_index;
                    }
                }
                Token::Eof => return Err(ExprParseError::new("unclosed expression block")),
                _ => {}
            }
        }
        let close_start = self
            .tokens
            .get(body_end)
            .map(|token| token.start)
            .ok_or_else(|| {
                ExprParseError::at(
                    "syntax.expr.invalid_token_span",
                    "control block closing token is outside the expression token stream",
                    open_range,
                )
            })?;
        let body_source = self.source.get(open.end..close_start).ok_or_else(|| {
            ExprParseError::at(
                "syntax.expr.invalid_token_span",
                "control block body is outside the owning expression source",
                open_range,
            )
        })?;
        let body_base = self.absolute_offset(open.end)?;
        let parsed =
            crate::parser::parse_callback_block_expr_body_recovering_at(body_source, body_base)?;
        match self.retain_nested_parsed_expr(parsed)? {
            block @ Expr::Block { .. } => Ok(block),
            _ => Err(ExprParseError::at(
                "syntax.expr.call_invariant",
                "control block parser did not produce a block expression",
                open_range,
            )),
        }
    }

    fn token_range_source(&self, start: usize, end: usize) -> String {
        if start >= end {
            return String::new();
        }
        let Some(first) = self.tokens.get(start) else {
            return String::new();
        };
        let Some(last) = self.tokens.get(end.saturating_sub(1)) else {
            return String::new();
        };
        self.source[first.start..last.end].to_owned()
    }
}
