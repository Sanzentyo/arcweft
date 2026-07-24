use super::{
    AwaitExpr, AwaitExprSource, AwaitPropagation, AwaitPropagationSource, DottedPath, Expr, ExprOp,
    ExprParseError, ExprParser, LexedToken, Name, Placeholder, Token, TryExpr, TryExprSource,
    TryOperatorSource, UnaryOp, parse_expr_at,
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
        if let Some(dialogue) = self.parse_dialogue_primary(&prefix)? {
            return Ok(dialogue);
        }
        match prefix.token {
            Token::Ident(keyword) if keyword == "try" && self.peek_ident("await") => {
                self.parse_try_await_expr(prefix_range)
            }
            Token::Ident(keyword) if keyword == "await" => self.parse_await_expr(prefix_range),
            Token::Ident(keyword) if keyword == "try" => self.parse_try_chain(prefix_range),
            Token::Ident(keyword) if keyword == "thread" => self.parse_thread_expr(),
            Token::Ident(keyword) if keyword == "if" => self.parse_if_expr_after_keyword(),
            Token::Ident(keyword) if keyword == "match" => self.parse_match_expr_after_keyword(),
            Token::Op(ExprOp::Or) => self.parse_zero_arg_closure(prefix_range),
            Token::Op(ExprOp::ClosurePipe) => self.parse_closure_after_open_pipe(prefix_range),
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
            Token::Ident(first) => {
                let mut segments = vec![Name::new(first)];
                while self.peek() == &Token::DoubleColon
                    && matches!(
                        self.tokens.get(self.cursor + 1).map(|token| &token.token),
                        Some(Token::Ident(_))
                    )
                {
                    self.bump();
                    let segment = self.bump_lexed();
                    let Token::Ident(segment) = segment.token else {
                        unreachable!("qualified path lookahead admitted an identifier")
                    };
                    segments.push(Name::new(segment));
                }
                let path = DottedPath::new(segments);
                if self.peek() == &Token::LBrace && !self.control_body_brace_is_boundary {
                    self.bump();
                    return Ok(Expr::Record {
                        path,
                        fields: self.parse_record_fields()?,
                    });
                }
                Ok(Expr::Path(path))
            }
            Token::RelativePath(path) => Ok(Expr::ShortVariant(Name::new(
                path.trim_start_matches('.').to_owned(),
            ))),
            Token::Underscore => Ok(Expr::Placeholder(Placeholder::Partial)),
            Token::Caret => Ok(Expr::Placeholder(Placeholder::PipeLeft)),
            Token::LParen => self.parse_tuple_or_group(),
            Token::LBracket => self.parse_bracket_seq(),
            Token::LBrace => Ok(Expr::RecordLiteral(self.parse_record_fields()?)),
            token => Err(ExprParseError::at(
                "syntax.expr.parse",
                &format!("expected expression, found {token:?}"),
                prefix_range,
            )),
        }
    }

    fn parse_dialogue_primary(
        &mut self,
        first: &LexedToken,
    ) -> Result<Option<Expr>, ExprParseError> {
        if matches!(
            &first.token,
            Token::Ident(keyword)
                if matches!(keyword.as_str(), "try" | "await" | "thread" | "if" | "match")
        ) {
            return Ok(None);
        }
        let Some(primary) = self.dialogue_primary.take() else {
            return Ok(None);
        };
        if first.start >= primary.open {
            self.dialogue_primary = Some(primary);
            return Ok(None);
        }

        let callee_source = self.source.get(first.start..primary.open).ok_or_else(|| {
            ExprParseError::at(
                "syntax.expr.invalid_scope",
                "dialogue callee range is outside the expression source",
                TextRange::new(self.base, self.base),
            )
        })?;
        let callee_base = self.absolute_offset(first.start)?;
        let callee = parse_expr_at(callee_source, callee_base)?;
        while self.peek() != &Token::Eof && self.peek_lexed().start < primary.end {
            self.bump();
        }
        Ok(Some(Expr::DialogueCall {
            callee: Box::new(callee),
            content: Box::new(primary.content),
            plan: primary.plan,
        }))
    }

    fn parse_try_await_expr(&mut self, try_keyword: TextRange) -> Result<Expr, ExprParseError> {
        let await_keyword = self.bump_lexed();
        let await_keyword = self.absolute_range(&await_keyword)?;
        let operand = self.parse_prefixed_operand(try_keyword)?;
        Ok(Expr::Await(AwaitExpr::new(
            Box::new(operand.expr),
            AwaitPropagation::PropagateError,
            AwaitExprSource::new(
                TextRange::new(try_keyword.start(), operand.range.end()),
                await_keyword,
                operand.range,
                Some(AwaitPropagationSource::PrefixTry { try_keyword }),
            ),
        )))
    }

    fn parse_await_expr(&mut self, await_keyword: TextRange) -> Result<Expr, ExprParseError> {
        let propagation_source = if self.peek() == &Token::Question {
            let question = self.bump_lexed();
            Some(AwaitPropagationSource::AttachedQuestion {
                question: self.absolute_range(&question)?,
            })
        } else {
            None
        };
        let operand = self.parse_prefixed_operand(await_keyword)?;
        Ok(Expr::Await(AwaitExpr::new(
            Box::new(operand.expr),
            if propagation_source.is_some() {
                AwaitPropagation::PropagateError
            } else {
                AwaitPropagation::PreserveResult
            },
            AwaitExprSource::new(
                TextRange::new(await_keyword.start(), operand.range.end()),
                await_keyword,
                operand.range,
                propagation_source,
            ),
        )))
    }

    fn parse_try_chain(&mut self, first: TextRange) -> Result<Expr, ExprParseError> {
        let mut operators = vec![first];
        loop {
            let operator_range = *operators
                .last()
                .expect("a try chain always retains its first operator");
            let Some(depth) = self.prefix_depth.checked_add(operators.len()) else {
                return Err(ExprParseError::prefix_depth_limit(operator_range));
            };
            if depth > 64 {
                return Err(ExprParseError::prefix_depth_limit(operator_range));
            }
            if !self.peek_ident("try")
                || matches!(
                    self.tokens.get(self.cursor + 1).map(|token| &token.token),
                    Some(Token::Ident(keyword)) if keyword == "await"
                )
            {
                break;
            }
            let operator = self.bump_lexed();
            operators.push(self.absolute_range(&operator)?);
        }

        let prior_depth = self.prefix_depth;
        self.prefix_depth = self
            .prefix_depth
            .checked_add(operators.len())
            .expect("the checked try-chain depth remains representable");
        let operand = self.parse_expr_bp_spanned(90);
        self.prefix_depth = prior_depth;
        let operand = operand?;
        let mut range = operand.range;
        let mut expr = operand.expr;
        for try_keyword in operators.into_iter().rev() {
            let whole = TextRange::new(try_keyword.start(), range.end());
            expr = Expr::Try(TryExpr::new(
                Box::new(expr),
                TryExprSource::new(whole, range, TryOperatorSource::PrefixTry { try_keyword }),
            ));
            range = whole;
        }
        Ok(expr)
    }

    fn parse_prefixed_operand(
        &mut self,
        operator_range: TextRange,
    ) -> Result<super::pratt::SpannedExpr, ExprParseError> {
        let depth = self
            .prefix_depth
            .checked_add(1)
            .ok_or_else(|| ExprParseError::prefix_depth_limit(operator_range))?;
        if depth > 64 {
            return Err(ExprParseError::prefix_depth_limit(operator_range));
        }
        let prior_depth = self.prefix_depth;
        self.prefix_depth = depth;
        let operand = self.parse_expr_bp_spanned(90);
        self.prefix_depth = prior_depth;
        operand
    }

    fn parse_prefix_chain(&mut self, first: LexedToken) -> Result<Expr, ExprParseError> {
        let mut operators = Vec::new();
        let mut current = first;
        loop {
            let operator_range = self.absolute_range(&current)?;
            let Some(next_depth) = self.prefix_depth.checked_add(operators.len()) else {
                return Err(ExprParseError::prefix_depth_limit(operator_range));
            };
            if next_depth >= 64 {
                return Err(ExprParseError::prefix_depth_limit(operator_range));
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
                ExprParseError::prefix_depth_limit(TextRange::new(self.base, self.base))
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
