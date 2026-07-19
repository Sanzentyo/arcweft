use super::{
    DottedPath, Expr, ExprOp, ExprParseError, ExprParser, LexedToken, Name, Placeholder, Token,
    UnaryOp,
};
use crate::ast::common::TextRange;
use crate::reference::{BorrowExpr, BorrowKind, DerefExpr};

enum ParsedPrefixOperator {
    Borrow { kind: BorrowKind, range: TextRange },
    Deref { range: TextRange },
    Unary(UnaryOp),
}

impl ExprParser {
    pub(super) fn parse_prefix(&mut self) -> Result<Expr, ExprParseError> {
        let prefix = self.bump_lexed();
        if matches!(
            &prefix.token,
            Token::Amp | Token::Star | Token::Bang | Token::Op(ExprOp::NegOrSub)
        ) {
            return self.parse_prefix_chain(prefix);
        }
        let prefix_range = self.absolute_range(&prefix)?;
        match prefix.token {
            Token::Ident(keyword) if keyword == "try" && self.peek_ident("await") => {
                self.bump();
                Ok(Expr::Await {
                    expr: Box::new(self.parse_expr_bp(90)?),
                    applies_try: true,
                })
            }
            Token::Ident(keyword) if keyword == "await" => {
                let applies_try = if self.peek() == &Token::Question {
                    self.bump();
                    true
                } else {
                    false
                };
                Ok(Expr::Await {
                    expr: Box::new(self.parse_expr_bp(90)?),
                    applies_try,
                })
            }
            Token::Ident(keyword) if keyword == "try" => Ok(Expr::Try {
                expr: Box::new(self.parse_expr_bp(90)?),
            }),
            Token::Ident(keyword) if keyword == "thread" => self.parse_thread_expr(),
            Token::Ident(keyword) if keyword == "if" => self.parse_if_expr_after_keyword(),
            Token::Ident(keyword) if keyword == "match" => self.parse_match_expr_after_keyword(),
            Token::Op(ExprOp::Or) => self.parse_zero_arg_closure(),
            Token::Op(ExprOp::ClosurePipe) => self.parse_closure_after_open_pipe(),
            Token::Op(ExprOp::Range | ExprOp::RangeInclusive) => {
                let inclusive = matches!(self.previous(), Some(Token::Op(ExprOp::RangeInclusive)));
                let end = if matches!(
                    self.peek(),
                    Token::Eof | Token::Comma | Token::RParen | Token::RBracket | Token::RBrace
                ) {
                    None
                } else {
                    Some(Box::new(self.parse_expr_bp(5)?))
                };
                Ok(Expr::Range {
                    start: None,
                    end,
                    inclusive,
                })
            }
            Token::Literal(literal) => Ok(Expr::Literal(literal)),
            Token::Invalid(message) => Err(ExprParseError::at(
                "syntax.expr.parse",
                &message,
                prefix_range,
            )),
            Token::Entity(entity) => Ok(Expr::EntityRef(entity.with_authored_range(prefix_range))),
            Token::LifetimePath { key, optional } => Ok(Expr::LifetimePath { key, optional }),
            Token::Ident(path) => {
                if self.peek() == &Token::LBrace && !self.control_body_brace_is_boundary {
                    self.bump();
                    return Ok(Expr::Record {
                        path,
                        fields: self.parse_record_fields()?,
                    });
                }
                Ok(Expr::Path(DottedPath::parse_dotted(path)))
            }
            Token::RelativePath(path) => Ok(Expr::ShortVariant(Name::new(
                path.trim_start_matches('.').to_owned(),
            ))),
            Token::Underscore => Ok(Expr::Placeholder(Placeholder::Partial)),
            Token::Caret => Ok(Expr::Placeholder(Placeholder::PipeLeft)),
            Token::LParen => self.parse_tuple_or_group(),
            Token::LBracket => self.parse_bracket_seq(),
            Token::LBrace => Ok(Expr::RecordLiteral(self.parse_record_fields()?)),
            token => Err(ExprParseError::new(&format!(
                "expected expression, found {token:?}"
            ))),
        }
    }

    fn parse_prefix_chain(&mut self, first: LexedToken) -> Result<Expr, ExprParseError> {
        let mut operators = Vec::new();
        let mut current = first;
        loop {
            let operator_range = self.absolute_range(&current)?;
            let Some(next_depth) = self.prefix_depth.checked_add(operators.len()) else {
                return Err(ExprParseError::at(
                    "syntax.expr.prefix_depth_limit",
                    "expression prefix nesting exceeds the inclusive limit of 64",
                    operator_range,
                ));
            };
            if next_depth >= 64 {
                return Err(ExprParseError::at(
                    "syntax.expr.prefix_depth_limit",
                    "expression prefix nesting exceeds the inclusive limit of 64",
                    operator_range,
                ));
            }
            let operator = self.parse_prefix_operator(&current, operator_range)?;
            operators.push(operator);

            self.require_prefix_operand()?;
            if matches!(
                self.peek(),
                Token::Amp | Token::Star | Token::Bang | Token::Op(ExprOp::NegOrSub)
            ) {
                current = self.bump_lexed();
                continue;
            }
            break;
        }

        let prior_depth = self.prefix_depth;
        self.prefix_depth = self
            .prefix_depth
            .checked_add(operators.len())
            .ok_or_else(|| {
                ExprParseError::at(
                    "syntax.expr.prefix_depth_limit",
                    "expression prefix nesting exceeds the inclusive limit of 64",
                    TextRange::new(self.base, self.base),
                )
            })?;
        let operand = self.parse_expr_bp(90);
        self.prefix_depth = prior_depth;
        let mut expr = operand?;
        for operator in operators.into_iter().rev() {
            expr = match operator {
                ParsedPrefixOperator::Borrow { kind, range } => {
                    Expr::Borrow(BorrowExpr::new(kind, Box::new(expr), range))
                }
                ParsedPrefixOperator::Deref { range } => {
                    Expr::Deref(DerefExpr::new(Box::new(expr), range))
                }
                ParsedPrefixOperator::Unary(op) => Expr::Unary {
                    op,
                    expr: Box::new(expr),
                },
            };
        }
        Ok(expr)
    }

    fn parse_prefix_operator(
        &mut self,
        token: &LexedToken,
        range: TextRange,
    ) -> Result<ParsedPrefixOperator, ExprParseError> {
        match &token.token {
            Token::Amp => {
                let mut operator_end = range.end();
                let kind = if self.peek_ident("mut") {
                    let mut_end = self.bump_lexed().end;
                    operator_end = self.absolute_offset(mut_end)?;
                    BorrowKind::Mutable
                } else {
                    BorrowKind::Shared
                };
                Ok(ParsedPrefixOperator::Borrow {
                    kind,
                    range: TextRange::new(range.start(), operator_end),
                })
            }
            Token::Star => Ok(ParsedPrefixOperator::Deref { range }),
            Token::Bang => Ok(ParsedPrefixOperator::Unary(UnaryOp::Not)),
            Token::Op(ExprOp::NegOrSub) => Ok(ParsedPrefixOperator::Unary(UnaryOp::Neg)),
            _ => Err(ExprParseError::at(
                "syntax.expr.parse",
                "expected prefix operator",
                range,
            )),
        }
    }

    fn require_prefix_operand(&self) -> Result<(), ExprParseError> {
        if !matches!(
            self.peek(),
            Token::Eof
                | Token::Comma
                | Token::Semicolon
                | Token::RParen
                | Token::RBracket
                | Token::RBrace
        ) {
            return Ok(());
        }
        let insertion = self.absolute_offset(self.peek_lexed().start)?;
        Err(ExprParseError::at(
            "syntax.expr.missing_prefix_operand",
            "prefix operator requires an operand",
            TextRange::new(insertion, insertion),
        ))
    }
}
