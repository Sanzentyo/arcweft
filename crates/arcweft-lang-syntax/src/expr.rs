use crate::ast::common::TextRange;
use crate::ast::flow::{Stmt, ThreadBlock, ThreadModifier};
use crate::ast::ids::{
    EntityRef, EntityRefSyntax, FamilyRelativeEntityRef, RelativeId, RelativeIdSpelling,
};
use crate::ast::line_plan::LinePlan;
use crate::ast::pattern::Pattern;
use crate::cst::{
    find_last_top_level_punctuation, find_top_level_punctuation, split_leading_entity_ref_parts,
    split_leading_relative_entity_ref, split_top_level_punctuation_once,
};
use arcweft_source::{SourceAnchor, SourceName};
use thiserror::Error;

/// Expression syntax preserved for type checking and HIR lowering.
///
/// This parser records expression shape without name resolution, generic
/// instantiation, or overload decisions. Those later compiler phases should be
/// able to consume this AST while keeping source-level diagnostics precise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    Literal(Literal),
    EntityRef(EntityRefSyntax),
    LifetimePath {
        key: LifetimeKey,
        optional: bool,
    },
    Path(String),
    Placeholder(Placeholder),
    Tuple(Vec<Expr>),
    /// Surface `[a, b, c]` sequence literal before expected-type resolution.
    BracketSeq(Vec<Expr>),
    ArrayRepeat {
        value: Box<Expr>,
        len: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    NamedArg {
        name: String,
        value: Box<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    Field {
        target: Box<Expr>,
        field: String,
    },
    DialogueCall {
        callee: Box<Expr>,
        content: String,
        plan: Option<LinePlan>,
    },
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },
    Pipe {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Try {
        expr: Box<Expr>,
    },
    Await {
        expr: Box<Expr>,
        applies_try: bool,
    },
    Thread {
        block: Box<ThreadBlock>,
    },
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },
    Record {
        path: String,
        fields: Vec<(String, Expr)>,
    },
    RecordLiteral(Vec<(String, Expr)>),
    Binary {
        lhs: Box<Expr>,
        op: BinaryOp,
        rhs: Box<Expr>,
    },
    Closure {
        params: Vec<String>,
        body: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Block {
        statements: Vec<Stmt>,
        value: Option<Box<Expr>>,
    },
    ComputationBlock {
        kind: ComputationBlockKind,
        statements: Vec<Stmt>,
        value: Option<Box<Expr>>,
    },
    MemoBlock {
        options: Vec<(String, Expr)>,
        statements: Vec<Stmt>,
        value: Option<Box<Expr>>,
    },
    NamedBlock {
        name: String,
        statements: Vec<Stmt>,
        value: Option<Box<Expr>>,
    },
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    IfLet {
        pattern: Box<Pattern>,
        expr: Box<Expr>,
        guard: Option<Box<Expr>>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchExprArm>,
    },
    Raw(String),
}

/// One value-producing `match` arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchExprArm {
    pattern: Box<Pattern>,
    guard: Option<Box<Expr>>,
    value: Box<Expr>,
}

/// Literal expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Literal {
    String(String),
    Char { raw: String, value: char },
    Int(i64),
    Float(String),
    Bool(bool),
    Duration { amount: String, unit: DurationUnit },
}

/// Duration suffix recognized by the syntax parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurationUnit {
    Millis,
    Seconds,
}

/// Placeholder expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placeholder {
    Partial,
    PipeLeft,
}

/// Built-in lifetime registry scopes used by script-visible lifetime paths.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LifetimeScopeKind {
    Frame,
    Tick,
    Cue,
    Line,
    Scene,
    Flow,
    Session,
    Global,
    Persistent,
    Named(String),
}

/// Structured key for a lifetime registry access such as `'line.focus?`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LifetimeKey {
    scope: LifetimeScopeKind,
    path: Vec<String>,
}

/// Semantic access mode used by the checker when validating registry paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifetimeAccessMode {
    Read,
    Write,
    MoveOut,
    Drop,
    Expose,
}

impl LifetimeScopeKind {
    pub fn parse(source: &str) -> Self {
        match source {
            "frame" => Self::Frame,
            "tick" => Self::Tick,
            "cue" => Self::Cue,
            "line" => Self::Line,
            "scene" => Self::Scene,
            "flow" => Self::Flow,
            "session" => Self::Session,
            "global" => Self::Global,
            "persistent" => Self::Persistent,
            name => Self::Named(name.to_owned()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Frame => "frame",
            Self::Tick => "tick",
            Self::Cue => "cue",
            Self::Line => "line",
            Self::Scene => "scene",
            Self::Flow => "flow",
            Self::Session => "session",
            Self::Global => "global",
            Self::Persistent => "persistent",
            Self::Named(name) => name.as_str(),
        }
    }
}

impl LifetimeKey {
    pub fn new(scope: LifetimeScopeKind, path: Vec<String>) -> Self {
        Self { scope, path }
    }

    pub const fn scope(&self) -> &LifetimeScopeKind {
        &self.scope
    }

    pub fn path(&self) -> &[String] {
        &self.path
    }

    pub fn as_dotted(&self) -> String {
        self.path
            .iter()
            .fold(self.scope.as_str().to_owned(), |mut key, part| {
                key.push('.');
                key.push_str(part);
                key
            })
    }
}

/// Binary operator syntax used in conditions and partial application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Implies,
    Or,
    And,
    In,
    Eq,
    NotEq,
    Gte,
    Lte,
    Gt,
    Lt,
    Merge,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

/// Unary operator syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
}

/// Named computation block syntax such as `result { ... }`, `task { ... }`, `seq { ... }`, or `stream { ... }`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputationBlockKind {
    Result,
    Task,
    Seq,
    Stream,
}

/// Expression parse error.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct ExprParseError {
    message: String,
    anchor: SourceAnchor,
}

/// Parses a single expression.
pub fn parse_expr(source: &str) -> Result<Expr, ExprParseError> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(ExprParseError::new("expected expression"));
    }
    if let Some((params, body)) = split_closure(trimmed) {
        return Ok(Expr::Closure {
            params,
            body: Box::new(parse_expr(body)?),
        });
    }
    if let Some((name, value)) = split_named_arg(trimmed) {
        return Ok(Expr::NamedArg {
            name: name.to_owned(),
            value: Box::new(parse_expr(value)?),
        });
    }
    if let Some((target, index)) = split_bracket_postfix(trimmed) {
        return Ok(Expr::Index {
            target: Box::new(parse_expr(target)?),
            index: Box::new(parse_expr(index).unwrap_or_else(|_| Expr::Raw(index.to_owned()))),
        });
    }
    ExprParser::new(trimmed).parse()
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Ident(String),
    RelativePath(String),
    Entity(EntityRefSyntax),
    LifetimePath { key: LifetimeKey, optional: bool },
    Literal(Literal),
    Invalid(String),
    Underscore,
    Caret,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Dot,
    Colon,
    Semicolon,
    Question,
    Bang,
    Op(&'static str),
    Eof,
}

struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, cursor: 0 }
    }

    fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.bump_char();
                continue;
            }
            tokens.push(match ch {
                '"' => self.lex_string_or_char(),
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
                ':' => self.single(Token::Colon),
                ';' => self.single(Token::Semicolon),
                '?' => self.single(Token::Question),
                '!' if self.starts_with("!=") => self.fixed_op("!=", 2),
                '!' => self.single(Token::Bang),
                '-' => self.fixed_op("-", 1),
                '.' if self.starts_with("..=") => self.fixed_op("..=", 3),
                '.' if self.starts_with("..") => self.fixed_op("..", 2),
                '.' if self.dot_starts_relative_path() => self.lex_relative_path(),
                '.' => self.single(Token::Dot),
                '=' if self.starts_with("=>") => self.fixed_op("=>", 2),
                '=' if self.starts_with("==") => self.fixed_op("==", 2),
                '=' => self.fixed_op("=", 1),
                '>' if self.starts_with(">=") => self.fixed_op(">=", 2),
                '<' if self.starts_with("<=") => self.fixed_op("<=", 2),
                '|' if self.starts_with("|>") => self.fixed_op("|>", 2),
                '|' if self.starts_with("||") => self.fixed_op("||", 2),
                '|' => self.fixed_op("|", 1),
                '&' if self.starts_with("&&") => self.fixed_op("&&", 2),
                '&' => self.fixed_op("&", 1),
                '+' => self.fixed_op("+", 1),
                '*' => self.fixed_op("*", 1),
                '/' => self.fixed_op("/", 1),
                '%' => self.fixed_op("%", 1),
                '>' => self.fixed_op(">", 1),
                '<' => self.fixed_op("<", 1),
                _ if is_ident_start(ch) => self.lex_ident(),
                _ => {
                    self.bump_char();
                    Token::Ident(ch.to_string())
                }
            });
        }
        tokens.push(Token::Eof);
        tokens
    }

    fn single(&mut self, token: Token) -> Token {
        self.bump_char();
        token
    }

    fn fixed_op(&mut self, op: &'static str, len: usize) -> Token {
        self.cursor += len;
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
                        .is_none_or(char_literal_suffix_boundary)
                {
                    self.bump_char();
                    let raw = self.source[literal_start..self.cursor].to_owned();
                    return match decode_char_literal(&value) {
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
            return parse_entity_expr(raw)
                .map_or_else(|| Token::Ident(raw.to_owned()), Token::Entity);
        }
        self.bump_char();
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() || matches!(ch, ')' | ']' | '}' | ',' | '{' | '[' | '(') {
                break;
            }
            self.bump_char();
        }
        let raw = &self.source[start..self.cursor];
        parse_entity_expr(raw).map_or_else(|| Token::Ident(raw.to_owned()), Token::Entity)
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
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.bump_char();
            } else {
                break;
            }
        }
        if self.peek_char() == Some('.')
            && !self.starts_with("..")
            && self
                .source
                .get(self.cursor + 1..)
                .and_then(|tail| tail.chars().next())
                .is_some_and(|ch| ch.is_ascii_digit())
        {
            self.bump_char();
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_digit() {
                    self.bump_char();
                } else {
                    break;
                }
            }
        }
        if self.starts_with("ms") {
            self.cursor += 2;
        } else if self.starts_with("s") {
            self.cursor += 1;
        } else {
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    self.bump_char();
                } else {
                    break;
                }
            }
        }
        let raw = &self.source[start..self.cursor];
        let (number, suffix) = split_number_suffix(raw);
        if let Some(duration) = parse_duration(raw) {
            Token::Literal(duration)
        } else if number.contains('.') || matches!(suffix, "f32" | "f64" | "pt" | "rad") {
            Token::Literal(Literal::Float(number.to_owned()))
        } else {
            Token::Literal(Literal::Int(number.parse().unwrap_or(0)))
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
            } else if self.starts_with("::") {
                self.cursor += 2;
            } else {
                break;
            }
        }
        if self.peek_char() == Some('<') {
            self.consume_angle_suffix();
        }
        let value = &self.source[start..self.cursor];
        match value {
            "true" => Token::Literal(Literal::Bool(true)),
            "false" => Token::Literal(Literal::Bool(false)),
            "in" => Token::Op("in"),
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

    fn dot_starts_relative_path(&self) -> bool {
        let at_expr_start = self.cursor == 0
            || self.source[..self.cursor]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_whitespace() || matches!(ch, '(' | '[' | '{' | ','));
        at_expr_start
            && self
                .source
                .get(self.cursor + 1..)
                .and_then(|tail| tail.chars().next())
                .is_some_and(is_ident_start)
    }

    fn consume_angle_suffix(&mut self) {
        let mut depth = 0_i32;
        while let Some(ch) = self.peek_char() {
            match ch {
                '<' => depth += 1,
                '>' => {
                    depth -= 1;
                    self.bump_char();
                    if depth == 0 {
                        break;
                    }
                    continue;
                }
                _ => {}
            }
            self.bump_char();
        }
    }
}

struct ExprParser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl ExprParser {
    fn new(source: &str) -> Self {
        Self {
            tokens: Lexer::new(source).tokenize(),
            cursor: 0,
        }
    }

    fn parse(mut self) -> Result<Expr, ExprParseError> {
        let expr = self.parse_expr_bp(0)?;
        if self.peek() != &Token::Eof {
            return Err(ExprParseError::new(&format!(
                "unexpected token after expression: {:?}",
                self.peek()
            )));
        }
        Ok(expr)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ExprParseError> {
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
                    Expr::Call {
                        callee: Box::new(lhs),
                        args,
                    }
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
                    let field = self.take_ident("expected field name after `.`")?;
                    self.skip_method_turbofish()?;
                    if self.peek() == &Token::LParen {
                        let args = self.parse_call_args()?;
                        Expr::MethodCall {
                            receiver: Box::new(lhs),
                            method: field,
                            args,
                        }
                    } else {
                        Expr::Field {
                            target: Box::new(lhs),
                            field,
                        }
                    }
                }
                Token::Op(".." | "..=") if min_bp <= 5 => {
                    let inclusive = matches!(self.bump(), Token::Op("..="));
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
                Token::Op(op) => {
                    let op = *op;
                    let Some((left_bp, right_bp, binary)) = infix_binding_power(op) else {
                        break;
                    };
                    if left_bp < min_bp {
                        break;
                    }
                    self.bump();
                    let rhs = self.parse_expr_bp(right_bp)?;
                    if op == "|>" {
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

    fn parse_prefix(&mut self) -> Result<Expr, ExprParseError> {
        match self.bump() {
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
            Token::Bang => Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_expr_bp(90)?),
            }),
            Token::Op("-") => Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(self.parse_expr_bp(90)?),
            }),
            Token::Op(".." | "..=") => {
                let inclusive = matches!(self.previous(), Some(Token::Op("..=")));
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
            Token::Entity(entity) => Ok(Expr::EntityRef(entity)),
            Token::LifetimePath { key, optional } => Ok(Expr::LifetimePath { key, optional }),
            Token::Ident(path) => {
                if self.peek() == &Token::LBrace {
                    self.bump();
                    return Ok(Expr::Record {
                        path,
                        fields: self.parse_record_fields()?,
                    });
                }
                Ok(Expr::Path(path))
            }
            Token::RelativePath(path) => Ok(Expr::Path(path)),
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

    fn parse_tuple_or_group(&mut self) -> Result<Expr, ExprParseError> {
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

    fn parse_bracket_seq(&mut self) -> Result<Expr, ExprParseError> {
        let mut items = Vec::new();
        if self.peek() == &Token::RBracket {
            self.bump();
            return Ok(Expr::BracketSeq(items));
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

    fn parse_call_args(&mut self) -> Result<Vec<Expr>, ExprParseError> {
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

    fn parse_thread_expr(&mut self) -> Result<Expr, ExprParseError> {
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
            vec![Stmt::Expr(parse_expr(body_source.trim())?)]
        };
        Ok(Expr::Thread {
            block: Box::new(ThreadBlock::new(
                modifiers,
                nonempty_joined_name(&name_parts),
                body,
            )),
        })
    }

    fn parse_arg_expr(&mut self) -> Result<Expr, ExprParseError> {
        if let (Token::Ident(name), Some(Token::Op("="))) =
            (self.peek(), self.tokens.get(self.cursor + 1))
        {
            let name = name.clone();
            self.bump();
            self.bump();
            return Ok(Expr::NamedArg {
                name,
                value: Box::new(self.parse_expr_bp(0)?),
            });
        }
        if self.peek() == &Token::Op("||") {
            self.bump();
            return Ok(Expr::Closure {
                params: Vec::new(),
                body: Box::new(self.parse_expr_bp(0)?),
            });
        }
        if self.peek() == &Token::Op("|") {
            return self.parse_closure_arg();
        }
        self.parse_expr_bp(0)
    }

    fn parse_closure_arg(&mut self) -> Result<Expr, ExprParseError> {
        self.expect(&Token::Op("|"))?;
        let mut params = Vec::new();
        loop {
            match self.bump() {
                Token::Ident(name) | Token::RelativePath(name) => params.push(name),
                Token::Comma => {}
                Token::Op("|") => break,
                Token::Eof => return Err(ExprParseError::new("unclosed closure parameter list")),
                _ => return Err(ExprParseError::new("expected closure parameter or `|`")),
            }
        }
        Ok(Expr::Closure {
            params,
            body: Box::new(self.parse_expr_bp(0)?),
        })
    }

    fn parse_record_fields(&mut self) -> Result<Vec<(String, Expr)>, ExprParseError> {
        let mut fields = Vec::new();
        if self.peek() == &Token::RBrace {
            self.bump();
            return Ok(fields);
        }
        loop {
            let name = self.take_ident("expected record field name")?;
            let value = if matches!(self.peek(), Token::Colon | Token::Op("=")) {
                self.bump();
                self.parse_expr_bp(0)?
            } else {
                Expr::Path(name.clone())
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

    fn take_ident(&mut self, message: &str) -> Result<String, ExprParseError> {
        match self.bump() {
            Token::Ident(name) | Token::RelativePath(name) => Ok(name),
            _ => Err(ExprParseError::new(message)),
        }
    }

    fn skip_method_turbofish(&mut self) -> Result<(), ExprParseError> {
        if self.peek() != &Token::Op("<") {
            return Ok(());
        }
        let mut depth = 0_i32;
        loop {
            match self.bump() {
                Token::Op("<") => depth += 1,
                Token::Op(">") => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Token::Eof => {
                    return Err(ExprParseError::new(
                        "unclosed generic argument list in method call",
                    ));
                }
                _ => {}
            }
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<(), ExprParseError> {
        let found = self.bump();
        if &found == expected {
            Ok(())
        } else {
            Err(ExprParseError::new(&format!(
                "expected {expected:?}, found {found:?}"
            )))
        }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.cursor).unwrap_or(&Token::Eof)
    }

    fn peek_ident(&self, expected: &str) -> bool {
        matches!(self.peek(), Token::Ident(value) if value == expected)
    }

    fn previous(&self) -> Option<&Token> {
        self.cursor
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
    }

    fn bump(&mut self) -> Token {
        let token = self.peek().clone();
        if !matches!(token, Token::Eof) {
            self.cursor += 1;
        }
        token
    }
}

fn infix_binding_power(op: &str) -> Option<(u8, u8, BinaryOp)> {
    Some(match op {
        "=>" => (10, 10, BinaryOp::Implies),
        "|>" => (15, 16, BinaryOp::Implies),
        "||" => (20, 21, BinaryOp::Or),
        "&&" => (30, 31, BinaryOp::And),
        "in" => (40, 5, BinaryOp::In),
        "==" => (45, 46, BinaryOp::Eq),
        "!=" => (45, 46, BinaryOp::NotEq),
        ">=" => (45, 46, BinaryOp::Gte),
        "<=" => (45, 46, BinaryOp::Lte),
        ">" => (45, 46, BinaryOp::Gt),
        "<" => (45, 46, BinaryOp::Lt),
        "&" => (48, 49, BinaryOp::Merge),
        "+" => (50, 51, BinaryOp::Add),
        "-" => (50, 51, BinaryOp::Sub),
        "*" => (60, 61, BinaryOp::Mul),
        "/" => (60, 61, BinaryOp::Div),
        "%" => (60, 61, BinaryOp::Rem),
        _ => return None,
    })
}

fn token_source(token: &Token) -> String {
    match token {
        Token::Ident(value)
        | Token::RelativePath(value)
        | Token::Literal(Literal::Float(value)) => value.clone(),
        Token::Entity(entity) => format!("@{}", entity.body()),
        Token::LifetimePath { key, optional } => {
            format!("'{}{}", key.as_dotted(), if *optional { "?" } else { "" })
        }
        Token::Literal(Literal::String(value)) => format!("\"{value}\""),
        Token::Literal(Literal::Char { raw, .. }) => raw.clone(),
        Token::Literal(Literal::Int(value)) => value.to_string(),
        Token::Literal(Literal::Bool(value)) => value.to_string(),
        Token::Literal(Literal::Duration { amount, unit }) => {
            format!("{amount}{}", duration_unit_suffix(*unit))
        }
        Token::Invalid(message) => message.clone(),
        Token::Underscore => "_".to_owned(),
        Token::Caret => "^".to_owned(),
        Token::LParen => "(".to_owned(),
        Token::RParen => ")".to_owned(),
        Token::LBracket => "[".to_owned(),
        Token::RBracket => "]".to_owned(),
        Token::LBrace => "{".to_owned(),
        Token::RBrace => "}".to_owned(),
        Token::Comma => ",".to_owned(),
        Token::Dot => ".".to_owned(),
        Token::Colon => ":".to_owned(),
        Token::Semicolon => ";".to_owned(),
        Token::Question => "?".to_owned(),
        Token::Bang => "!".to_owned(),
        Token::Op(op) => (*op).to_owned(),
        Token::Eof => String::new(),
    }
}

const fn duration_unit_suffix(unit: DurationUnit) -> &'static str {
    match unit {
        DurationUnit::Millis => "ms",
        DurationUnit::Seconds => "s",
    }
}

fn nonempty_joined_name(parts: &[String]) -> Option<String> {
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn parse_duration(source: &str) -> Option<Literal> {
    if let Some(value) = source.strip_suffix("ms") {
        if is_numeric_duration(value) {
            return Some(Literal::Duration {
                amount: value.to_owned(),
                unit: DurationUnit::Millis,
            });
        }
    }
    let value = source.strip_suffix('s')?;
    is_numeric_duration(value).then(|| Literal::Duration {
        amount: value.to_owned(),
        unit: DurationUnit::Seconds,
    })
}

fn split_number_suffix(source: &str) -> (&str, &str) {
    let split = source
        .char_indices()
        .find(|(_, ch)| ch.is_ascii_alphabetic() || *ch == '_')
        .map_or(source.len(), |(index, _)| index);
    (&source[..split], &source[split..])
}

fn parse_entity_expr(source: &str) -> Option<EntityRefSyntax> {
    if let Some(relative_ref) = split_leading_relative_entity_ref(source) {
        if relative_ref.raw.len() != source.len() {
            return None;
        }
        let spelling = match relative_ref.relative.spelling {
            crate::cst::CstRelativeIdSpelling::DotRun => RelativeIdSpelling::DotRun,
            crate::cst::CstRelativeIdSpelling::SuperChain => RelativeIdSpelling::SuperChain,
        };
        let relative = RelativeId::new(
            relative_ref.relative.body.to_owned(),
            relative_ref.relative.parent_depth,
            spelling,
            TextRange::new(
                '@'.len_utf8() + relative_ref.family.len() + ':'.len_utf8(),
                source.len(),
            ),
        );
        return Some(EntityRefSyntax::family_relative(
            FamilyRelativeEntityRef::new(
                relative_ref.family.to_owned(),
                relative,
                TextRange::new(0, source.len()),
            ),
        ));
    }
    let entity_ref = split_leading_entity_ref_parts(source)?;
    if entity_ref.body.is_empty() && !entity_ref.delimited {
        return None;
    }
    if entity_ref.delimited && !entity_ref.closed {
        return None;
    }
    (entity_ref.raw.len() == source.len()).then_some(EntityRefSyntax::absolute(EntityRef::new(
        entity_ref.body.to_owned(),
        entity_ref.delimited,
        TextRange::new(0, source.len()),
    )))
}

fn split_bracket_postfix(source: &str) -> Option<(&str, &str)> {
    let close = source.strip_suffix(']')?;
    let open = find_last_top_level_open_bracket(close)?;
    let target = close[..open].trim();
    if target.is_empty() {
        return None;
    }
    Some((target, &close[open + 1..]))
}

fn split_closure(source: &str) -> Option<(Vec<String>, &str)> {
    let rest = source.strip_prefix('|')?;
    let close = find_top_level_punctuation(rest, '|')?;
    let params = rest[..close]
        .split(',')
        .map(str::trim)
        .filter(|param| !param.is_empty())
        .map(str::to_owned)
        .collect();
    let body = rest[close + 1..].trim();
    (!body.is_empty()).then_some((params, body))
}

fn split_named_arg(source: &str) -> Option<(&str, &str)> {
    let (name, value) = split_top_level_punctuation_once(source, '=')?;
    if name.is_empty()
        || value.is_empty()
        || source.contains("==")
        || source.contains("!=")
        || source.contains(">=")
        || source.contains("<=")
        || !is_identifier(name)
    {
        return None;
    }
    Some((name, value))
}

fn find_last_top_level_open_bracket(source: &str) -> Option<usize> {
    find_last_top_level_punctuation(source, '[')
}

fn is_identifier(source: &str) -> bool {
    source
        .chars()
        .next()
        .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
        && source.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
}

fn is_numeric_duration(source: &str) -> bool {
    !source.is_empty()
        && source.chars().filter(|ch| *ch == '.').count() <= 1
        && source.chars().any(|ch| ch.is_ascii_digit())
        && source.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
}

fn char_literal_suffix_boundary(tail: &str) -> bool {
    tail.chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || matches!(ch, ')' | ']' | '}' | ',' | ';'))
}

fn decode_char_literal(source: &str) -> Result<char, String> {
    let mut chars = source.chars();
    let value = match chars.next() {
        Some('\\') => decode_char_escape(&mut chars)?,
        Some(value) => value,
        None => return Err("char literal must contain exactly one Unicode scalar value".to_owned()),
    };
    if chars.next().is_some() {
        return Err("char literal must contain exactly one Unicode scalar value".to_owned());
    }
    Ok(value)
}

fn decode_char_escape(chars: &mut core::str::Chars<'_>) -> Result<char, String> {
    match chars.next() {
        Some('n') => Ok('\n'),
        Some('r') => Ok('\r'),
        Some('t') => Ok('\t'),
        Some('0') => Ok('\0'),
        Some('\\') => Ok('\\'),
        Some('"') => Ok('"'),
        Some('u') => decode_unicode_escape(chars),
        Some(other) => Err(format!("unsupported char escape `\\{other}`")),
        None => Err("unterminated char escape".to_owned()),
    }
}

fn decode_unicode_escape(chars: &mut core::str::Chars<'_>) -> Result<char, String> {
    if chars.next() != Some('{') {
        return Err("unicode char escape must use `\\u{...}`".to_owned());
    }
    let mut digits = String::new();
    for ch in chars.by_ref() {
        if ch == '}' {
            let value = u32::from_str_radix(&digits, 16)
                .map_err(|_| "invalid unicode char escape".to_owned())?;
            return char::from_u32(value)
                .ok_or_else(|| "unicode char escape is not a valid scalar value".to_owned());
        }
        digits.push(ch);
    }
    Err("unterminated unicode char escape".to_owned())
}

impl ExprParseError {
    fn new(message: &str) -> Self {
        Self {
            message: message.to_owned(),
            anchor: SourceAnchor::new(SourceName::path("<expr>"), 0..0),
        }
    }

    /// Source anchor for the expression parse failure.
    pub const fn anchor(&self) -> &SourceAnchor {
        &self.anchor
    }
}

impl MatchExprArm {
    /// Builds a value-producing match arm.
    pub fn new(pattern: Pattern, guard: Option<Box<Expr>>, value: Box<Expr>) -> Self {
        Self {
            pattern: Box::new(pattern),
            guard,
            value,
        }
    }

    /// Pattern that must match before this arm can run.
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    /// Optional `when` guard.
    pub fn guard(&self) -> Option<&Expr> {
        self.guard.as_deref()
    }

    /// Value produced by the arm.
    pub fn value(&self) -> &Expr {
        &self.value
    }
}
