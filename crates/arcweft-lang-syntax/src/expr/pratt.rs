use super::{
    BinaryOp, CallArg, DottedPath, Expr, ExprOp, ExprParseError, ExprParseStats, FlowItem,
    LexedToken, Lexer, Literal, ParsedExpr, Stmt, ThreadBlock, ThreadModifier, Token,
    flat_literal_bracket_seq_expr, literal_exprs_from_tokens, nonempty_joined_name, parse_expr,
    token_source,
};

pub(super) struct ExprParser {
    pub(super) source: String,
    pub(super) base: usize,
    pub(super) tokens: Vec<LexedToken>,
    pub(super) cursor: usize,
    stats: ExprParseStats,
    pub(super) prefix_depth: u8,
}

impl ExprParser {
    pub(super) fn new_at(source: &str, base: usize) -> Self {
        Self {
            source: source.to_owned(),
            base,
            tokens: Lexer::new(source).tokenize(),
            cursor: 0,
            stats: ExprParseStats::default(),
            prefix_depth: 0,
        }
    }

    pub(super) fn parse(mut self) -> Result<ParsedExpr, ExprParseError> {
        let expr = self.parse_expr_bp(0)?;
        if self.peek() != &Token::Eof {
            let unexpected = self.peek_lexed();
            return Err(ExprParseError::at(
                "syntax.expr.unexpected_token",
                &format!("unexpected token after expression: {:?}", unexpected.token),
                self.absolute_range(unexpected),
            ));
        }
        Ok(ParsedExpr {
            expr,
            stats: self.stats,
        })
    }

    pub(super) fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ExprParseError> {
        let mut lhs = self.parse_prefix()?;
        loop {
            lhs = match self.peek() {
                Token::Question if min_bp <= 100 => {
                    self.bump();
                    Expr::Try {
                        expr: Box::new(lhs),
                    }
                }
                Token::LParen if min_bp <= 100 => {
                    let args = self.parse_call_args()?;
                    Expr::call(lhs, args)
                }
                Token::LBracket if min_bp <= 100 => {
                    self.bump();
                    let index = if self.peek() == &Token::RBracket {
                        Expr::Tuple(Vec::new())
                    } else {
                        self.parse_expr_bp(0)?
                    };
                    self.expect(&Token::RBracket)?;
                    Expr::Index {
                        target: Box::new(lhs),
                        index: Box::new(index),
                    }
                }
                Token::Dot if min_bp <= 100 => {
                    self.bump();
                    let member = self.take_ident("expected selector name after `.`")?;
                    self.skip_selector_turbofish_before_call();
                    let selected = Expr::select(lhs, member);
                    if self.peek() == &Token::LParen {
                        let args = self.parse_call_args()?;
                        Expr::call(selected, args)
                    } else if self.peek() == &Token::LBrace {
                        Expr::call(
                            selected,
                            vec![CallArg::Positional(self.parse_callback_block_closure()?)],
                        )
                    } else {
                        selected
                    }
                }
                Token::Op(ExprOp::Range | ExprOp::RangeInclusive) if min_bp <= 5 => {
                    let inclusive = matches!(self.bump(), Token::Op(ExprOp::RangeInclusive));
                    let end = if matches!(
                        self.peek(),
                        Token::Eof | Token::Comma | Token::RParen | Token::RBracket | Token::RBrace
                    ) {
                        None
                    } else {
                        Some(Box::new(self.parse_expr_bp(5)?))
                    };
                    Expr::Range {
                        start: Some(Box::new(lhs)),
                        end,
                        inclusive,
                    }
                }
                Token::Amp | Token::Star | Token::Op(_) => {
                    let op = match self.peek() {
                        Token::Amp => ExprOp::Merge,
                        Token::Star => ExprOp::Mul,
                        Token::Op(op) => *op,
                        _ => unreachable!("guarded by the enclosing token pattern"),
                    };
                    let Some((left_bp, right_bp, binary)) = infix_binding_power(op) else {
                        break;
                    };
                    if left_bp < min_bp {
                        break;
                    }
                    self.bump();
                    let rhs = self.parse_expr_bp(right_bp)?;
                    if op == ExprOp::Pipe {
                        Expr::Pipe {
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        }
                    } else {
                        Expr::Binary {
                            lhs: Box::new(lhs),
                            op: binary,
                            rhs: Box::new(rhs),
                        }
                    }
                }
                _ => break,
            };
        }
        Ok(lhs)
    }

    pub(super) fn parse_tuple_or_group(&mut self) -> Result<Expr, ExprParseError> {
        if self.peek() == &Token::RParen {
            self.bump();
            return Ok(Expr::Tuple(Vec::new()));
        }
        let mut items = Vec::new();
        loop {
            items.push(self.parse_expr_bp(0)?);
            match self.peek() {
                Token::Comma => {
                    self.bump();
                    if self.peek() == &Token::RParen {
                        self.bump();
                        return Ok(Expr::Tuple(items));
                    }
                }
                Token::RParen => {
                    self.bump();
                    return if items.len() == 1 {
                        Ok(items.remove(0))
                    } else {
                        Ok(Expr::Tuple(items))
                    };
                }
                _ => return Err(ExprParseError::new("expected `)` or `,` in tuple")),
            }
        }
    }

    pub(super) fn parse_bracket_seq(&mut self) -> Result<Expr, ExprParseError> {
        let mut items = Vec::new();
        if self.peek() == &Token::RBracket {
            self.bump();
            return Ok(Expr::BracketSeq(items));
        }
        if let Some(expr) = self.parse_flat_literal_bracket_seq() {
            return Ok(expr);
        }
        loop {
            items.push(self.parse_expr_bp(0)?);
            match self.peek() {
                Token::Semicolon => {
                    self.bump();
                    if items.len() != 1 {
                        return Err(ExprParseError::new(
                            "array repeat literal expects one value before `;`",
                        ));
                    }
                    let len = self.parse_expr_bp(0)?;
                    self.expect(&Token::RBracket)?;
                    let value = items.remove(0);
                    return Ok(Expr::ArrayRepeat {
                        value: Box::new(value),
                        len: Box::new(len),
                    });
                }
                Token::Comma => {
                    self.bump();
                    if self.peek() == &Token::RBracket {
                        self.bump();
                        return Ok(Expr::BracketSeq(items));
                    }
                }
                Token::RBracket => {
                    self.bump();
                    return Ok(Expr::BracketSeq(items));
                }
                _ => {
                    return Err(ExprParseError::new(
                        "expected `]` or `,` in bracket sequence literal",
                    ));
                }
            }
        }
    }

    pub(super) fn parse_flat_literal_bracket_seq(&mut self) -> Option<Expr> {
        let start = self.cursor;
        let mut fallback_items = None;
        let mut int_literals = Vec::new();
        let mut int_suffix = None;
        let mut int_suffix_seen = false;
        let mut all_int = true;
        loop {
            let Token::Literal(literal) = self.peek() else {
                self.cursor = start;
                return None;
            };
            match literal {
                Literal::Int(literal) if all_int => {
                    if int_suffix_seen && int_suffix != literal.suffix() {
                        all_int = false;
                    } else if !int_suffix_seen {
                        int_suffix = literal.suffix();
                        int_suffix_seen = true;
                    }
                    int_literals.push(literal.clone());
                }
                _ => all_int = false,
            }
            if !all_int {
                fallback_items
                    .get_or_insert_with(|| {
                        literal_exprs_from_tokens(&self.tokens[start..self.cursor])
                    })
                    .push(Expr::Literal(literal.clone()));
            }
            self.bump();
            match self.peek() {
                Token::Comma => {
                    self.bump();
                    if self.peek() == &Token::RBracket {
                        self.bump();
                        let expr =
                            flat_literal_bracket_seq_expr(all_int, int_literals, fallback_items);
                        if matches!(expr, Expr::NumericBracketSeq(_)) {
                            self.stats.numeric_seq_summaries += 1;
                        }
                        return Some(expr);
                    }
                }
                Token::RBracket => {
                    self.bump();
                    let expr = flat_literal_bracket_seq_expr(all_int, int_literals, fallback_items);
                    if matches!(expr, Expr::NumericBracketSeq(_)) {
                        self.stats.numeric_seq_summaries += 1;
                    }
                    return Some(expr);
                }
                _ => {
                    self.cursor = start;
                    return None;
                }
            }
        }
    }

    pub(super) fn parse_call_args(&mut self) -> Result<Vec<CallArg>, ExprParseError> {
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        if self.peek() == &Token::RParen {
            self.bump();
            return Ok(args);
        }
        loop {
            args.push(self.parse_arg_expr()?);
            match self.peek() {
                Token::Comma => {
                    self.bump();
                    if self.peek() == &Token::RParen {
                        self.bump();
                        return Ok(args);
                    }
                }
                Token::RParen => {
                    self.bump();
                    return Ok(args);
                }
                _ => return Err(ExprParseError::new("expected `)` or `,` in argument list")),
            }
        }
    }

    pub(super) fn parse_thread_expr(&mut self) -> Result<Expr, ExprParseError> {
        let mut modifiers = Vec::new();
        let mut name_parts = Vec::new();
        while self.peek() != &Token::LBrace {
            match self.bump() {
                Token::Ident(value) if value == "detached" && name_parts.is_empty() => {
                    modifiers.push(ThreadModifier::Detached);
                }
                Token::Ident(value) => name_parts.push(value),
                Token::Eof => return Err(ExprParseError::new("expected `{` in thread expression")),
                token => {
                    return Err(ExprParseError::new(&format!(
                        "expected thread name or `{{`, found {token:?}"
                    )));
                }
            }
        }
        self.expect(&Token::LBrace)?;
        let mut depth = 1usize;
        let mut body_tokens = Vec::new();
        while depth > 0 {
            match self.bump() {
                Token::LBrace => {
                    depth += 1;
                    body_tokens.push("{".to_owned());
                }
                Token::RBrace => {
                    depth -= 1;
                    if depth > 0 {
                        body_tokens.push("}".to_owned());
                    }
                }
                Token::Eof => return Err(ExprParseError::new("unclosed thread expression block")),
                token => body_tokens.push(token_source(&token)),
            }
        }
        let body_source = body_tokens.join(" ");
        let body = if body_source.trim().is_empty() {
            Vec::new()
        } else {
            vec![FlowItem::Stmt(Stmt::Expr {
                expr: parse_expr(body_source.trim())?,
                expr_source: None,
                expr_range: None,
            })]
        };
        Ok(Expr::Thread {
            block: Box::new(ThreadBlock::new(
                modifiers,
                nonempty_joined_name(&name_parts),
                body,
            )),
        })
    }

    pub(super) fn parse_arg_expr(&mut self) -> Result<CallArg, ExprParseError> {
        if let Some(name) = self.parse_named_arg_name() {
            return Ok(CallArg::Named {
                name,
                value: Box::new(self.parse_expr_bp(0)?),
            });
        }
        if self.peek() == &Token::Op(ExprOp::Or) {
            self.bump();
            return self.parse_zero_arg_closure().map(CallArg::Positional);
        }
        if self.peek() == &Token::Op(ExprOp::ClosurePipe) {
            return self.parse_closure_arg().map(CallArg::Positional);
        }
        let expr = self.parse_expr_bp(0)?;
        if self.peek() == &Token::Op(ExprOp::Spread) {
            self.bump();
            return Ok(CallArg::Spread {
                value: Box::new(expr),
            });
        }
        Ok(CallArg::Positional(expr))
    }

    pub(super) fn parse_named_arg_name(&mut self) -> Option<String> {
        let mut cursor = self.cursor;
        let Token::Ident(first) = self.token_at(cursor) else {
            return None;
        };
        let mut parts = vec![first.clone()];
        cursor += 1;
        while matches!(self.token_at(cursor), Token::Dot) {
            let Token::Ident(part) = self.token_at(cursor + 1) else {
                return None;
            };
            parts.push(part.clone());
            cursor += 2;
        }
        if self.token_at(cursor) != &Token::Op(ExprOp::Assign) {
            return None;
        }
        self.cursor = cursor + 1;
        Some(parts.join("."))
    }

    pub(super) fn parse_record_fields(&mut self) -> Result<Vec<(String, Expr)>, ExprParseError> {
        let mut fields = Vec::new();
        if self.peek() == &Token::RBrace {
            self.bump();
            return Ok(fields);
        }
        loop {
            let name = self.take_ident("expected record field name")?;
            let value = if matches!(self.peek(), Token::Colon | Token::Op(ExprOp::Assign)) {
                self.bump();
                self.parse_expr_bp(0)?
            } else {
                Expr::Path(DottedPath::single(name.clone()))
            };
            fields.push((name, value));
            match self.peek() {
                Token::Comma => {
                    self.bump();
                    if self.peek() == &Token::RBrace {
                        self.bump();
                        return Ok(fields);
                    }
                }
                Token::RBrace => {
                    self.bump();
                    return Ok(fields);
                }
                _ => return Err(ExprParseError::new("expected `}` or `,` in record literal")),
            }
        }
    }

    pub(super) fn take_ident(&mut self, message: &str) -> Result<String, ExprParseError> {
        match self.bump() {
            Token::Ident(name) | Token::RelativePath(name) => Ok(name),
            _ => Err(ExprParseError::new(message)),
        }
    }

    pub(super) fn skip_selector_turbofish_before_call(&mut self) -> bool {
        if self.peek() != &Token::Op(ExprOp::Lt) {
            return false;
        }
        let start = self.cursor;
        let mut depth = 0_i32;
        loop {
            match self.bump() {
                Token::Op(ExprOp::Lt) => depth += 1,
                Token::Op(ExprOp::Gt) => {
                    depth -= 1;
                    if depth == 0 {
                        if self.peek() == &Token::LParen {
                            return true;
                        }
                        self.cursor = start;
                        return false;
                    }
                }
                Token::Eof => {
                    self.cursor = start;
                    return false;
                }
                _ => {}
            }
        }
    }

    pub(super) fn expect(&mut self, expected: &Token) -> Result<(), ExprParseError> {
        let found = self.bump();
        if &found == expected {
            Ok(())
        } else {
            Err(ExprParseError::new(&format!(
                "expected {expected:?}, found {found:?}"
            )))
        }
    }

    pub(super) fn peek(&self) -> &Token {
        self.token_at(self.cursor)
    }

    pub(super) fn peek_lexed(&self) -> &LexedToken {
        self.tokens.get(self.cursor).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("lexer always appends an EOF token")
        })
    }

    pub(super) fn token_at(&self, index: usize) -> &Token {
        self.tokens
            .get(index)
            .map_or(&Token::Eof, |lexed| &lexed.token)
    }

    pub(super) fn peek_ident(&self, expected: &str) -> bool {
        matches!(self.peek(), Token::Ident(value) if value == expected)
    }

    pub(super) fn previous(&self) -> Option<&Token> {
        self.cursor
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(|lexed| &lexed.token)
    }

    pub(super) fn bump(&mut self) -> Token {
        let token = self.peek().clone();
        if !matches!(token, Token::Eof) {
            self.cursor += 1;
        }
        token
    }

    pub(super) fn bump_lexed(&mut self) -> LexedToken {
        let lexed = self.tokens.get(self.cursor).cloned().unwrap_or_else(|| {
            let end = self.source.len();
            LexedToken {
                token: Token::Eof,
                start: end,
                end,
            }
        });
        if !matches!(lexed.token, Token::Eof) {
            self.cursor += 1;
        }
        lexed
    }

    pub(super) fn absolute_range(&self, token: &LexedToken) -> crate::ast::common::TextRange {
        crate::ast::common::TextRange::new(self.base + token.start, self.base + token.end)
    }

    pub(super) const fn absolute_offset(&self, offset: usize) -> usize {
        self.base + offset
    }
}

fn infix_binding_power(op: ExprOp) -> Option<(u8, u8, BinaryOp)> {
    Some(match op {
        ExprOp::FatArrow => (10, 10, BinaryOp::Implies),
        ExprOp::Pipe => (15, 16, BinaryOp::Implies),
        ExprOp::Or => (20, 21, BinaryOp::Or),
        ExprOp::And => (30, 31, BinaryOp::And),
        ExprOp::In => (40, 5, BinaryOp::In),
        ExprOp::Eq => (45, 46, BinaryOp::Eq),
        ExprOp::NotEq => (45, 46, BinaryOp::NotEq),
        ExprOp::Gte => (45, 46, BinaryOp::Gte),
        ExprOp::Lte => (45, 46, BinaryOp::Lte),
        ExprOp::Gt => (45, 46, BinaryOp::Gt),
        ExprOp::Lt => (45, 46, BinaryOp::Lt),
        ExprOp::Merge => (48, 49, BinaryOp::Merge),
        ExprOp::Add => (50, 51, BinaryOp::Add),
        ExprOp::NegOrSub => (50, 51, BinaryOp::Sub),
        ExprOp::Mul => (60, 61, BinaryOp::Mul),
        ExprOp::Div => (60, 61, BinaryOp::Div),
        ExprOp::Rem => (60, 61, BinaryOp::Rem),
        _ => return None,
    })
}
