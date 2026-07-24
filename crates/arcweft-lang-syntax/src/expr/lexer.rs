use super::{
    ExprOp, FloatSuffix, IntLiteral, IntRadix, IntSuffix, LexedToken, LifetimeKey,
    LifetimeScopeKind, Literal, Token, UnitNumberSuffix, char_literal, digit_matches_radix,
    is_ident_continue, is_ident_start, parse_duration, parse_entity_expr, split_number_suffix,
};

pub(super) struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
}

impl<'a> Lexer<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        Self { source, cursor: 0 }
    }

    pub(super) fn tokenize(mut self) -> Vec<LexedToken> {
        let mut tokens = Vec::new();
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.bump_char();
                continue;
            }
            if self.starts_with("//") {
                while self.peek_char().is_some_and(|next| next != '\n') {
                    self.bump_char();
                }
                continue;
            }
            if self.starts_with("/*") {
                let start = self.cursor;
                self.cursor += 2;
                if let Some(close) = self.source[self.cursor..].find("*/") {
                    self.cursor += close + 2;
                    continue;
                }
                self.cursor = self.source.len();
                tokens.push(LexedToken {
                    token: Token::Invalid("unclosed block comment in expression".to_owned()),
                    start,
                    end: self.cursor,
                });
                break;
            }
            let start = self.cursor;
            let token = self.lex_token(ch, tokens.last().map(|token| &token.token));
            tokens.push(LexedToken {
                token,
                start,
                end: self.cursor,
            });
        }
        tokens.push(LexedToken {
            token: Token::Eof,
            start: self.cursor,
            end: self.cursor,
        });
        tokens
    }

    fn lex_token(&mut self, ch: char, previous: Option<&Token>) -> Token {
        match ch {
            '"' => self.lex_string_or_char(),
            'r' if self.raw_string_prefix().is_some() => self.lex_raw_string(),
            '@' => self.lex_entity(),
            '\'' => self.lex_lifetime_path(),
            '0'..='9' => self.lex_number_or_duration(),
            '_' => {
                self.bump_char();
                Token::Underscore
            }
            '^' => {
                self.bump_char();
                Token::Caret
            }
            '(' => self.single(Token::LParen),
            ')' => self.single(Token::RParen),
            '[' => self.single(Token::LBracket),
            ']' => self.single(Token::RBracket),
            '{' => self.single(Token::LBrace),
            '}' => self.single(Token::RBrace),
            ',' => self.single(Token::Comma),
            ':' if self.starts_with("::") => {
                self.cursor += 2;
                Token::DoubleColon
            }
            ':' => self.single(Token::Colon),
            ';' => self.single(Token::Semicolon),
            '?' => self.single(Token::Question),
            '!' if self.starts_with_op(ExprOp::NotEq) => self.fixed_op(ExprOp::NotEq),
            '!' => self.single(Token::Bang),
            '-' if self.starts_with_op(ExprOp::ThinArrow) => self.fixed_op(ExprOp::ThinArrow),
            '-' => self.fixed_op(ExprOp::NegOrSub),
            '.' if self.starts_with_op(ExprOp::Spread) => self.fixed_op(ExprOp::Spread),
            '.' if self.starts_with_op(ExprOp::RangeInclusive) => {
                self.fixed_op(ExprOp::RangeInclusive)
            }
            '.' if self.starts_with_op(ExprOp::Range) => self.fixed_op(ExprOp::Range),
            '.' if self.dot_starts_relative_path(previous) => self.lex_relative_path(),
            '.' => self.single(Token::Dot),
            '=' if self.starts_with_op(ExprOp::FatArrow) => self.fixed_op(ExprOp::FatArrow),
            '=' if self.starts_with_op(ExprOp::Eq) => self.fixed_op(ExprOp::Eq),
            '=' => self.fixed_op(ExprOp::Assign),
            '>' if self.starts_with_op(ExprOp::Gte) => self.fixed_op(ExprOp::Gte),
            '<' if self.starts_with_op(ExprOp::Lte) => self.fixed_op(ExprOp::Lte),
            '|' if self.starts_with_op(ExprOp::Pipe) => self.fixed_op(ExprOp::Pipe),
            '|' if self.starts_with_op(ExprOp::Or) => self.fixed_op(ExprOp::Or),
            '|' => self.fixed_op(ExprOp::ClosurePipe),
            '&' if self.starts_with_op(ExprOp::And) => self.fixed_op(ExprOp::And),
            '&' => self.single(Token::Amp),
            '+' => self.fixed_op(ExprOp::Add),
            '*' => self.single(Token::Star),
            '/' => self.fixed_op(ExprOp::Div),
            '%' => self.fixed_op(ExprOp::Rem),
            '>' => self.fixed_op(ExprOp::Gt),
            '<' => self.fixed_op(ExprOp::Lt),
            _ if is_ident_start(ch) => self.lex_ident(),
            _ => {
                self.bump_char();
                Token::Invalid(format!("invalid expression token `{ch}`"))
            }
        }
    }

    fn single(&mut self, token: Token) -> Token {
        self.bump_char();
        token
    }

    fn fixed_op(&mut self, op: ExprOp) -> Token {
        self.cursor += op.as_str().len();
        Token::Op(op)
    }

    fn lex_string_or_char(&mut self) -> Token {
        let literal_start = self.cursor;
        self.bump_char();
        let start = self.cursor;
        let mut escaped = false;
        while let Some(ch) = self.peek_char() {
            if ch == '"' && !escaped {
                let value = self.source[start..self.cursor].to_owned();
                self.bump_char();
                if self.starts_with("c")
                    && self
                        .source
                        .get(self.cursor + 'c'.len_utf8()..)
                        .is_none_or(char_literal::suffix_boundary)
                {
                    self.bump_char();
                    let raw = self.source[literal_start..self.cursor].to_owned();
                    return match char_literal::decode(&value) {
                        Ok(value) => Token::Literal(Literal::Char { raw, value }),
                        Err(message) => Token::Invalid(message),
                    };
                }
                return Token::Literal(Literal::String(value));
            }
            escaped = ch == '\\' && !escaped;
            if ch != '\\' {
                escaped = false;
            }
            self.bump_char();
        }
        Token::Literal(Literal::String(self.source[start..].to_owned()))
    }

    fn raw_string_prefix(&self) -> Option<usize> {
        let tail = self.source.get(self.cursor..)?.strip_prefix('r')?;
        let hashes = tail.chars().take_while(|ch| *ch == '#').count();
        tail.get(hashes..)?.starts_with('"').then_some(hashes)
    }

    fn lex_raw_string(&mut self) -> Token {
        let hashes = self
            .raw_string_prefix()
            .expect("raw string lexer is called only for a validated prefix");
        self.cursor += 'r'.len_utf8() + hashes + '"'.len_utf8();
        let body_start = self.cursor;
        let terminator = format!("\"{}", "#".repeat(hashes));
        let Some(relative_end) = self.source[self.cursor..].find(&terminator) else {
            self.cursor = self.source.len();
            return Token::Invalid("unclosed raw string literal".to_owned());
        };
        let body_end = self.cursor + relative_end;
        self.cursor = body_end + terminator.len();
        Token::Literal(Literal::String(
            self.source[body_start..body_end].to_owned(),
        ))
    }

    fn lex_entity(&mut self) -> Token {
        let start = self.cursor;
        if self.starts_with("@<") {
            self.cursor += 2;
            while let Some(ch) = self.peek_char() {
                self.bump_char();
                if ch == '>' {
                    break;
                }
            }
            let raw = &self.source[start..self.cursor];
            return parse_entity_expr(raw).map_or_else(
                || Token::Invalid(format!("invalid entity reference `{raw}`")),
                Token::Entity,
            );
        }
        self.bump_char();
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() || matches!(ch, ')' | ']' | '}' | ',' | '{' | '[' | '(') {
                break;
            }
            self.bump_char();
        }
        let raw = &self.source[start..self.cursor];
        parse_entity_expr(raw).map_or_else(
            || Token::Invalid(format!("invalid entity reference `{raw}`")),
            Token::Entity,
        )
    }

    fn lex_lifetime_path(&mut self) -> Token {
        self.bump_char();
        let lifetime_start = self.cursor;
        while let Some(ch) = self.peek_char() {
            if is_ident_continue(ch) {
                self.bump_char();
            } else {
                break;
            }
        }
        let lifetime = self.source[lifetime_start..self.cursor].to_owned();
        let mut path = Vec::new();
        while self.peek_char() == Some('.') {
            self.bump_char();
            let part_start = self.cursor;
            while let Some(ch) = self.peek_char() {
                if is_ident_continue(ch) {
                    self.bump_char();
                } else {
                    break;
                }
            }
            if part_start == self.cursor {
                break;
            }
            path.push(self.source[part_start..self.cursor].to_owned());
        }
        let optional = if self.peek_char() == Some('?') {
            self.bump_char();
            true
        } else {
            false
        };
        if lifetime.is_empty() || path.is_empty() {
            Token::Ident(format!("'{lifetime}"))
        } else {
            Token::LifetimePath {
                key: LifetimeKey::new(LifetimeScopeKind::parse(&lifetime), path),
                optional,
            }
        }
    }

    fn lex_number_or_duration(&mut self) -> Token {
        let start = self.cursor;
        self.consume_number_body();
        self.consume_exponent();
        self.consume_number_suffix();
        let raw = &self.source[start..self.cursor];
        let (number, suffix) = split_number_suffix(raw);
        let suffix = (!suffix.is_empty()).then(|| suffix.trim_start_matches('_'));
        let float_suffix = suffix.and_then(FloatSuffix::parse);
        let unit_suffix = suffix.and_then(UnitNumberSuffix::parse);
        let has_float_body = number.contains('.') || number.contains('e') || number.contains('E');
        if let Some(duration) = parse_duration(raw) {
            Token::Literal(duration)
        } else if let Some(unit_suffix) = unit_suffix {
            Token::Literal(Literal::UnitNumber {
                raw: raw.to_owned(),
                suffix: unit_suffix,
            })
        } else if has_float_body || float_suffix.is_some() {
            if suffix.is_some() && float_suffix.is_none() {
                return Token::Invalid(format!(
                    "unknown float literal suffix `{}`",
                    suffix.unwrap_or_default()
                ));
            }
            Token::Literal(Literal::Float {
                raw: raw.to_owned(),
                suffix: float_suffix,
            })
        } else {
            let int_suffix = match suffix {
                Some(suffix) => match IntSuffix::parse(suffix) {
                    Some(suffix) => Some(suffix),
                    None => {
                        return Token::Invalid(format!(
                            "unknown integer literal suffix `{suffix}`"
                        ));
                    }
                },
                None => None,
            };
            Token::Literal(Literal::Int(IntLiteral::new(
                raw,
                IntRadix::from_number_source(number),
                int_suffix,
            )))
        }
    }

    fn consume_number_body(&mut self) {
        self.bump_char();
        let starts_with_zero = self
            .cursor
            .checked_sub('0'.len_utf8())
            .and_then(|start| self.source.get(start..self.cursor))
            == Some("0");
        if starts_with_zero && matches!(self.peek_char(), Some('x' | 'X')) {
            self.bump_char();
            self.consume_radix_digits_or_underscores(16);
            return;
        }
        if starts_with_zero && matches!(self.peek_char(), Some('b' | 'B')) {
            self.bump_char();
            self.consume_radix_digits_or_underscores(2);
            return;
        }
        if starts_with_zero && matches!(self.peek_char(), Some('o' | 'O')) {
            self.bump_char();
            self.consume_radix_digits_or_underscores(8);
            return;
        }
        self.consume_decimal_digits_or_underscores();
        if self.peek_char() == Some('.') && !self.starts_with_op(ExprOp::Range) {
            self.bump_char();
            self.consume_decimal_digits_or_underscores();
        }
    }

    fn consume_decimal_digits_or_underscores(&mut self) {
        while self
            .peek_char()
            .is_some_and(|ch| ch.is_ascii_digit() || ch == '_')
        {
            self.bump_char();
        }
    }

    fn consume_radix_digits_or_underscores(&mut self, radix: u32) {
        while self
            .peek_char()
            .is_some_and(|ch| ch == '_' || digit_matches_radix(ch, radix))
        {
            self.bump_char();
        }
    }

    fn consume_number_suffix(&mut self) {
        if self.peek_char() == Some('%') {
            self.bump_char();
            return;
        }
        while self
            .peek_char()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            self.bump_char();
        }
    }

    fn consume_exponent(&mut self) {
        if !matches!(self.peek_char(), Some('e' | 'E')) {
            return;
        }
        let exponent_start = self.cursor;
        self.bump_char();
        if matches!(self.peek_char(), Some('+' | '-')) {
            self.bump_char();
        }
        let digits_start = self.cursor;
        self.consume_decimal_digits_or_underscores();
        if self.source[digits_start..self.cursor]
            .chars()
            .filter(|ch| *ch != '_')
            .all(|ch| !ch.is_ascii_digit())
        {
            self.cursor = exponent_start;
        }
    }

    fn lex_relative_path(&mut self) -> Token {
        let start = self.cursor;
        self.bump_char();
        while let Some(ch) = self.peek_char() {
            if is_ident_continue(ch) {
                self.bump_char();
            } else {
                break;
            }
        }
        Token::RelativePath(self.source[start..self.cursor].to_owned())
    }

    fn lex_ident(&mut self) -> Token {
        let start = self.cursor;
        self.bump_char();
        while let Some(ch) = self.peek_char() {
            if is_ident_continue(ch) {
                self.bump_char();
            } else {
                break;
            }
        }
        let value = &self.source[start..self.cursor];
        match value {
            "true" => Token::Literal(Literal::Bool(true)),
            "false" => Token::Literal(Literal::Bool(false)),
            "in" => Token::Op(ExprOp::In),
            _ => Token::Ident(value.to_owned()),
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source.get(self.cursor..)?.chars().next()
    }

    fn bump_char(&mut self) {
        if let Some(ch) = self.peek_char() {
            self.cursor += ch.len_utf8();
        }
    }

    fn starts_with(&self, value: &str) -> bool {
        self.source[self.cursor..].starts_with(value)
    }

    fn starts_with_op(&self, op: ExprOp) -> bool {
        self.starts_with(op.as_str())
    }

    fn dot_starts_relative_path(&self, previous: Option<&Token>) -> bool {
        let at_expr_start = previous.is_none_or(|token| match token {
            // `>` is also the closing token of an authored generic receiver.
            // Keeping the following dot separate lets the speculative type
            // transaction decide that case without stealing comparison tokens.
            Token::Op(ExprOp::Gt) => false,
            Token::LParen
            | Token::LBracket
            | Token::LBrace
            | Token::Comma
            | Token::Colon
            | Token::Semicolon
            | Token::Amp
            | Token::Star
            | Token::Bang
            | Token::Op(_) => true,
            _ => false,
        });
        at_expr_start
            && self
                .source
                .get(self.cursor + 1..)
                .and_then(|tail| tail.chars().next())
                .is_some_and(is_ident_start)
    }
}
