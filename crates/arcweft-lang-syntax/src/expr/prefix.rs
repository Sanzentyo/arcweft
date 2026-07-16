use super::{
    DottedPath, Expr, ExprOp, ExprParseError, ExprParser, Name, Placeholder, Token, UnaryOp,
};
use crate::ast::common::TextRange;
use crate::reference::{BorrowExpr, BorrowKind, DerefExpr};

impl ExprParser {
    pub(super) fn parse_prefix(&mut self) -> Result<Expr, ExprParseError> {
        let prefix = self.bump_lexed();
        let prefix_range = self.absolute_range(&prefix);
        match prefix.token {
            Token::Amp => self.parse_borrow_prefix(prefix_range),
            Token::Star => self.parse_deref_prefix(prefix_range),
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
            Token::Bang => Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_prefix_operand(prefix_range)?),
            }),
            Token::Op(ExprOp::NegOrSub) => Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(self.parse_prefix_operand(prefix_range)?),
            }),
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
            Token::Invalid(message) => Err(ExprParseError::new(&message)),
            Token::Entity(entity) => Ok(Expr::EntityRef(entity.with_authored_range(prefix_range))),
            Token::LifetimePath { key, optional } => Ok(Expr::LifetimePath { key, optional }),
            Token::Ident(path) => {
                if self.peek() == &Token::LBrace {
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

    fn parse_borrow_prefix(&mut self, ampersand: TextRange) -> Result<Expr, ExprParseError> {
        let mut operator_end = ampersand.end();
        let kind = if self.peek_ident("mut") {
            let mut_end = self.bump_lexed().end;
            operator_end = self.absolute_offset(mut_end);
            BorrowKind::Mutable
        } else {
            BorrowKind::Shared
        };
        let operand = self.parse_prefix_operand(ampersand)?;
        Ok(Expr::Borrow(BorrowExpr::new(
            kind,
            Box::new(operand),
            TextRange::new(ampersand.start(), operator_end),
        )))
    }

    fn parse_deref_prefix(&mut self, operator: TextRange) -> Result<Expr, ExprParseError> {
        let operand = self.parse_prefix_operand(operator)?;
        Ok(Expr::Deref(DerefExpr::new(Box::new(operand), operator)))
    }

    fn parse_prefix_operand(&mut self, operator_range: TextRange) -> Result<Expr, ExprParseError> {
        if self.prefix_depth >= 64 {
            return Err(ExprParseError::at(
                "syntax.expr.prefix_depth_limit",
                "expression prefix nesting exceeds the inclusive limit of 64",
                operator_range,
            ));
        }
        if matches!(
            self.peek(),
            Token::Eof
                | Token::Comma
                | Token::Semicolon
                | Token::RParen
                | Token::RBracket
                | Token::RBrace
        ) {
            let insertion = self.absolute_offset(self.peek_lexed().start);
            return Err(ExprParseError::at(
                "syntax.expr.missing_prefix_operand",
                "prefix operator requires an operand",
                TextRange::new(insertion, insertion),
            ));
        }
        self.prefix_depth += 1;
        let operand = self.parse_expr_bp(90);
        self.prefix_depth -= 1;
        operand
    }
}
