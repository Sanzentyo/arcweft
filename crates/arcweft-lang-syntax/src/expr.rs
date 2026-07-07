use crate::ast::common::TextRange;
use crate::ast::flow::{FlowItem, Stmt, ThreadBlock, ThreadModifier};
use crate::ast::ids::{
    EntityRef, EntityRefSyntax, FamilyRelativeEntityRef, RelativeId, RelativeIdSpelling,
};
use crate::ast::line_plan::LinePlan;
use crate::ast::pattern::Pattern;
use crate::cst::{
    find_last_top_level_punctuation, split_leading_entity_ref_parts,
    split_leading_relative_entity_ref, split_top_level_punctuation,
    split_top_level_punctuation_once,
};
use crate::pattern::parse_pattern;
use crate::types::{TypeRef, parse_type_ref};
use arcweft_source::{SourceAnchor, SourceName};
use std::{
    fmt,
    ops::{Add, Deref},
};
use thiserror::Error;

mod char_literal;
mod closure_source;

use closure_source::ClosureBodySource;

/// Identifier segment used by expression paths and shorthand selectors.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Name(String);

impl Name {
    /// Creates a name segment from parser-owned source text.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the source spelling for this name segment.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Name {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<str> for Name {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Name {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<Name> for str {
    fn eq(&self, other: &Name) -> bool {
        self == other.as_str()
    }
}

impl From<Name> for String {
    fn from(value: Name) -> Self {
        value.0
    }
}

/// Dotted expression path before semantic resolution.
///
/// Parser code keeps these paths structured so later phases can distinguish
/// namespace, type, value, and capability selectors without reparsing strings.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DottedPath {
    segments: Vec<Name>,
    label: String,
}

impl DottedPath {
    /// Builds a path from one or more already-split segments.
    pub fn new(segments: Vec<Name>) -> Self {
        let label = segments
            .iter()
            .map(Name::as_str)
            .collect::<Vec<_>>()
            .join(".");
        Self { segments, label }
    }

    /// Builds a single-segment path from parser-owned source text.
    pub fn single(value: impl Into<String>) -> Self {
        Self::new(vec![Name::new(value)])
    }

    /// Splits canonical dotted surface syntax into path segments.
    pub fn parse_dotted(value: impl Into<String>) -> Self {
        let value = value.into();
        let segments = value
            .split('.')
            .filter(|segment| !segment.is_empty())
            .map(|segment| Name::new(segment.to_owned()))
            .collect::<Vec<_>>();
        Self::new(segments)
    }

    /// Returns a copy with one member appended.
    #[must_use]
    pub fn with_member(&self, member: impl Into<String>) -> Self {
        let mut segments = self.segments.clone();
        segments.push(Name::new(member));
        Self::new(segments)
    }

    /// Returns the canonical dotted source spelling.
    pub fn as_label(&self) -> &str {
        &self.label
    }

    /// Returns the canonical dotted source spelling.
    pub fn as_str(&self) -> &str {
        self.as_label()
    }

    /// Returns the path segments in source order.
    pub fn segments(&self) -> &[Name] {
        &self.segments
    }

    /// Returns true when the path contains exactly one segment with this name.
    pub fn is_single(&self, value: &str) -> bool {
        matches!(self.segments.as_slice(), [segment] if segment == value)
    }

    /// Returns true when the path segments exactly match `segments`.
    pub fn matches_segments(&self, segments: &[&str]) -> bool {
        self.segments.len() == segments.len()
            && self
                .segments
                .iter()
                .zip(segments.iter())
                .all(|(actual, expected)| actual.as_str() == *expected)
    }
}

impl AsRef<str> for DottedPath {
    fn as_ref(&self) -> &str {
        self.as_label()
    }
}

impl Deref for DottedPath {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_label()
    }
}

impl fmt::Display for DottedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_label())
    }
}

impl PartialEq<str> for DottedPath {
    fn eq(&self, other: &str) -> bool {
        self.as_label() == other
    }
}

impl PartialEq<&str> for DottedPath {
    fn eq(&self, other: &&str) -> bool {
        self.as_label() == *other
    }
}

impl PartialEq<DottedPath> for str {
    fn eq(&self, other: &DottedPath) -> bool {
        self == other.as_label()
    }
}

impl From<DottedPath> for String {
    fn from(value: DottedPath) -> Self {
        value.label
    }
}

impl From<String> for DottedPath {
    fn from(value: String) -> Self {
        Self::parse_dotted(value)
    }
}

impl From<&str> for DottedPath {
    fn from(value: &str) -> Self {
        Self::parse_dotted(value)
    }
}

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
    Path(DottedPath),
    ShortVariant(Name),
    Placeholder(Placeholder),
    Tuple(Vec<Expr>),
    /// Surface `[a, b, c]` sequence literal before expected-type resolution.
    BracketSeq(Vec<Expr>),
    /// Integer-only sequence literal summarized without per-item expression nodes.
    NumericBracketSeq(NumericBracketSeq),
    ArrayRepeat {
        value: Box<Expr>,
        len: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<CallArg>,
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
        params: Vec<ClosureParam>,
        return_type: Option<TypeRef>,
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

/// Compact representation for integer-only bracket sequence literals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NumericBracketSeq {
    values: Vec<i64>,
    suffix: Option<String>,
}

/// One argument in a call or method-call argument list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallArg {
    Positional(Expr),
    Named { name: String, value: Box<Expr> },
    Spread { value: Box<Expr> },
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
    Char {
        raw: String,
        value: char,
    },
    Int {
        raw: String,
        value: i64,
        suffix: Option<String>,
    },
    Float {
        raw: String,
        suffix: Option<FloatSuffix>,
    },
    UnitNumber {
        raw: String,
        suffix: UnitNumberSuffix,
    },
    Bool(bool),
    Duration {
        amount: String,
        unit: DurationUnit,
    },
}

/// Floating-point literal width suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatSuffix {
    F32,
    F64,
}

impl FloatSuffix {
    pub fn parse(source: &str) -> Option<Self> {
        match source {
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

impl fmt::Display for FloatSuffix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Numeric literal suffix that carries presentation or geometry units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitNumberSuffix {
    Percent,
    Px,
    Pt,
    Em,
    Rem,
    Vw,
    Vh,
    Deg,
    Rad,
    Turn,
    Db,
    Lufs,
    Bpm,
    Bars,
}

impl UnitNumberSuffix {
    pub fn parse(source: &str) -> Option<Self> {
        match source {
            "%" => Some(Self::Percent),
            "px" => Some(Self::Px),
            "pt" => Some(Self::Pt),
            "em" => Some(Self::Em),
            "rem" => Some(Self::Rem),
            "vw" => Some(Self::Vw),
            "vh" => Some(Self::Vh),
            "deg" => Some(Self::Deg),
            "rad" => Some(Self::Rad),
            "turn" => Some(Self::Turn),
            "db" => Some(Self::Db),
            "lufs" => Some(Self::Lufs),
            "bpm" => Some(Self::Bpm),
            "bars" => Some(Self::Bars),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Percent => "%",
            Self::Px => "px",
            Self::Pt => "pt",
            Self::Em => "em",
            Self::Rem => "rem",
            Self::Vw => "vw",
            Self::Vh => "vh",
            Self::Deg => "deg",
            Self::Rad => "rad",
            Self::Turn => "turn",
            Self::Db => "db",
            Self::Lufs => "lufs",
            Self::Bpm => "bpm",
            Self::Bars => "bars",
        }
    }
}

impl fmt::Display for UnitNumberSuffix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Duration suffix recognized by the syntax parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurationUnit {
    Nanos,
    Micros,
    Millis,
    Seconds,
    Minutes,
    Hours,
}

impl DurationUnit {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nanos => "ns",
            Self::Micros => "us",
            Self::Millis => "ms",
            Self::Seconds => "s",
            Self::Minutes => "min",
            Self::Hours => "h",
        }
    }
}

/// Placeholder expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placeholder {
    Partial,
    PipeLeft,
}

/// One closure parameter before semantic capture and lifetime analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureParam {
    pattern: Pattern,
    ty: Option<TypeRef>,
}

impl ClosureParam {
    /// Creates a parsed closure parameter from a pattern and optional type ascription.
    pub fn new(pattern: Pattern, ty: Option<TypeRef>) -> Self {
        Self { pattern, ty }
    }

    /// Source pattern bound by this parameter.
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    /// Optional type ascription written after the parameter pattern.
    pub const fn ty(&self) -> Option<&TypeRef> {
        self.ty.as_ref()
    }

    /// Returns the parameter name when the pattern is a simple local binding.
    pub fn simple_ident(&self) -> Option<&str> {
        match &self.pattern {
            Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
                Some(name)
            }
            _ => None,
        }
    }
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

impl NumericBracketSeq {
    pub fn new(values: Vec<i64>, suffix: Option<String>) -> Self {
        Self { values, suffix }
    }

    pub fn values(&self) -> &[i64] {
        &self.values
    }

    pub fn suffix(&self) -> Option<&str> {
        self.suffix.as_deref()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
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

/// Path-free counters collected while parsing one expression.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExprParseStats {
    numeric_seq_summaries: usize,
}

impl ExprParseStats {
    /// Number of integer-only bracket sequences parsed as compact summaries.
    pub const fn numeric_seq_summaries(self) -> usize {
        self.numeric_seq_summaries
    }
}

impl Add for ExprParseStats {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            numeric_seq_summaries: self
                .numeric_seq_summaries
                .saturating_add(rhs.numeric_seq_summaries),
        }
    }
}

/// Parses a single expression.
pub fn parse_expr(source: &str) -> Result<Expr, ExprParseError> {
    parse_expr_with_stats(source).map(|parsed| parsed.expr)
}

/// Parses a single expression and returns path-free parser counters.
pub fn parse_expr_with_stats(source: &str) -> Result<ParsedExpr, ExprParseError> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(ExprParseError::new("expected expression"));
    }
    if let Some(closure) = closure_source::split(trimmed)? {
        let params = parse_closure_params(closure.params)?;
        let return_type = closure
            .return_type
            .map(parse_type_ref)
            .transpose()
            .map_err(|error| ExprParseError::new(&error.to_string()))?;
        let parsed_body = match closure.body {
            ClosureBodySource::Expr(body) => parse_expr_with_stats(body)?,
            ClosureBodySource::Block(body) => ParsedExpr {
                expr: crate::parser::parse_callback_block_expr_body(body),
                stats: ExprParseStats::default(),
            },
        };
        return Ok(ParsedExpr {
            expr: Expr::Closure {
                params,
                return_type,
                body: Box::new(parsed_body.expr),
            },
            stats: parsed_body.stats,
        });
    }
    if !trimmed.starts_with('[')
        && let Some((target, index)) = split_bracket_postfix(trimmed)
    {
        let parsed_target = parse_expr_with_stats(target)?;
        let parsed_index = parse_expr_with_stats(index).unwrap_or_else(|_| ParsedExpr {
            expr: Expr::Raw(index.to_owned()),
            stats: ExprParseStats::default(),
        });
        return Ok(ParsedExpr {
            expr: Expr::Index {
                target: Box::new(parsed_target.expr),
                index: Box::new(parsed_index.expr),
            },
            stats: parsed_target.stats + parsed_index.stats,
        });
    }
    ExprParser::new(trimmed).parse()
}

/// Expression parse result bundled with cheap parser counters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedExpr {
    pub expr: Expr,
    pub stats: ExprParseStats,
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
    Op(ExprOp),
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExprOp {
    NotEq,
    Arrow,
    NegOrSub,
    Spread,
    RangeInclusive,
    Range,
    FatArrow,
    Assign,
    Eq,
    Gte,
    Lte,
    Pipe,
    Or,
    ClosurePipe,
    And,
    Merge,
    Add,
    Mul,
    Div,
    Rem,
    Gt,
    Lt,
    In,
}

impl ExprOp {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NotEq => "!=",
            Self::Arrow => "->",
            Self::NegOrSub => "-",
            Self::Spread => "...",
            Self::RangeInclusive => "..=",
            Self::Range => "..",
            Self::FatArrow => "=>",
            Self::Assign => "=",
            Self::Eq => "==",
            Self::Gte => ">=",
            Self::Lte => "<=",
            Self::Pipe => "|>",
            Self::Or => "||",
            Self::ClosurePipe => "|",
            Self::And => "&&",
            Self::Merge => "&",
            Self::Add => "+",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
            Self::Gt => ">",
            Self::Lt => "<",
            Self::In => "in",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LexedToken {
    token: Token,
    start: usize,
    end: usize,
}

struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, cursor: 0 }
    }

    fn tokenize(mut self) -> Vec<LexedToken> {
        let mut tokens = Vec::with_capacity(self.source.len().saturating_div(3).saturating_add(1));
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.bump_char();
                continue;
            }
            let start = self.cursor;
            let token = match ch {
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
                '!' if self.starts_with("!=") => self.fixed_op(ExprOp::NotEq, 2),
                '!' => self.single(Token::Bang),
                '-' if self.starts_with("->") => self.fixed_op(ExprOp::Arrow, 2),
                '-' => self.fixed_op(ExprOp::NegOrSub, 1),
                '.' if self.starts_with("...") => self.fixed_op(ExprOp::Spread, 3),
                '.' if self.starts_with("..=") => self.fixed_op(ExprOp::RangeInclusive, 3),
                '.' if self.starts_with("..") => self.fixed_op(ExprOp::Range, 2),
                '.' if self.dot_starts_relative_path() => self.lex_relative_path(),
                '.' => self.single(Token::Dot),
                '=' if self.starts_with("=>") => self.fixed_op(ExprOp::FatArrow, 2),
                '=' if self.starts_with("==") => self.fixed_op(ExprOp::Eq, 2),
                '=' => self.fixed_op(ExprOp::Assign, 1),
                '>' if self.starts_with(">=") => self.fixed_op(ExprOp::Gte, 2),
                '<' if self.starts_with("<=") => self.fixed_op(ExprOp::Lte, 2),
                '|' if self.starts_with("|>") => self.fixed_op(ExprOp::Pipe, 2),
                '|' if self.starts_with("||") => self.fixed_op(ExprOp::Or, 2),
                '|' => self.fixed_op(ExprOp::ClosurePipe, 1),
                '&' if self.starts_with("&&") => self.fixed_op(ExprOp::And, 2),
                '&' => self.fixed_op(ExprOp::Merge, 1),
                '+' => self.fixed_op(ExprOp::Add, 1),
                '*' => self.fixed_op(ExprOp::Mul, 1),
                '/' => self.fixed_op(ExprOp::Div, 1),
                '%' => self.fixed_op(ExprOp::Rem, 1),
                '>' => self.fixed_op(ExprOp::Gt, 1),
                '<' => self.fixed_op(ExprOp::Lt, 1),
                _ if is_ident_start(ch) => self.lex_ident(),
                _ => {
                    self.bump_char();
                    Token::Ident(ch.to_string())
                }
            };
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

    fn single(&mut self, token: Token) -> Token {
        self.bump_char();
        token
    }

    fn fixed_op(&mut self, op: ExprOp, len: usize) -> Token {
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
        self.consume_number_body();
        self.consume_exponent();
        self.consume_number_suffix();
        let raw = &self.source[start..self.cursor];
        let (number, suffix) = split_number_suffix(raw);
        let suffix = (!suffix.is_empty()).then(|| suffix.trim_start_matches('_').to_owned());
        let float_suffix = suffix.as_deref().and_then(FloatSuffix::parse);
        let unit_suffix = suffix.as_deref().and_then(UnitNumberSuffix::parse);
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
                    suffix.as_deref().unwrap_or_default()
                ));
            }
            Token::Literal(Literal::Float {
                raw: raw.to_owned(),
                suffix: float_suffix,
            })
        } else {
            Token::Literal(Literal::Int {
                raw: raw.to_owned(),
                value: parse_int_literal_value(number).unwrap_or(0),
                suffix,
            })
        }
    }

    fn consume_number_body(&mut self) {
        self.bump_char();
        if self.source[self.cursor.saturating_sub(1)..].starts_with('0')
            && matches!(self.peek_char(), Some('x' | 'X'))
        {
            self.bump_char();
            self.consume_radix_digits_or_underscores(16);
            return;
        }
        if self.source[self.cursor.saturating_sub(1)..].starts_with('0')
            && matches!(self.peek_char(), Some('b' | 'B'))
        {
            self.bump_char();
            self.consume_radix_digits_or_underscores(2);
            return;
        }
        if self.source[self.cursor.saturating_sub(1)..].starts_with('0')
            && matches!(self.peek_char(), Some('o' | 'O'))
        {
            self.bump_char();
            self.consume_radix_digits_or_underscores(8);
            return;
        }
        self.consume_decimal_digits_or_underscores();
        if self.peek_char() == Some('.') && !self.starts_with("..") {
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
        if self.peek_char() == Some('<') {
            self.consume_angle_suffix();
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

    fn dot_starts_relative_path(&self) -> bool {
        let at_expr_start = self.cursor == 0
            || self.source[..self.cursor]
                .chars()
                .next_back()
                .is_some_and(|ch| {
                    ch.is_whitespace() || matches!(ch, '(' | '[' | '{' | ',' | '=' | ':')
                });
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
    source: String,
    tokens: Vec<LexedToken>,
    cursor: usize,
    stats: ExprParseStats,
}

#[derive(Default)]
struct ClosureReturnParse {
    return_type: Option<TypeRef>,
    block_body: Option<String>,
}

impl ExprParser {
    fn new(source: &str) -> Self {
        Self {
            source: source.to_owned(),
            tokens: Lexer::new(source).tokenize(),
            cursor: 0,
            stats: ExprParseStats::default(),
        }
    }

    fn parse(mut self) -> Result<ParsedExpr, ExprParseError> {
        let expr = self.parse_expr_bp(0)?;
        if self.peek() != &Token::Eof {
            return Err(ExprParseError::new(&format!(
                "unexpected token after expression: {:?}",
                self.peek()
            )));
        }
        Ok(ParsedExpr {
            expr,
            stats: self.stats,
        })
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
                    self.skip_method_turbofish_before_call();
                    if self.peek() == &Token::LParen {
                        let args = self.parse_call_args()?;
                        Expr::MethodCall {
                            receiver: Box::new(lhs),
                            method: field,
                            args,
                        }
                    } else if self.peek() == &Token::LBrace {
                        Expr::MethodCall {
                            receiver: Box::new(lhs),
                            method: field,
                            args: vec![CallArg::Positional(self.parse_callback_block_closure()?)],
                        }
                    } else {
                        Expr::Field {
                            target: Box::new(lhs),
                            field,
                        }
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
            Token::Op(ExprOp::NegOrSub) => Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(self.parse_expr_bp(90)?),
            }),
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

    fn parse_flat_literal_bracket_seq(&mut self) -> Option<Expr> {
        let start = self.cursor;
        let mut fallback_items = None;
        let mut int_values = Vec::new();
        let mut int_suffix = None;
        let mut int_suffix_seen = false;
        let mut all_int = true;
        loop {
            let Token::Literal(literal) = self.peek() else {
                self.cursor = start;
                return None;
            };
            match literal {
                Literal::Int { value, suffix, .. } if all_int => {
                    if int_suffix_seen && int_suffix.as_ref() != suffix.as_ref() {
                        all_int = false;
                    } else if !int_suffix_seen {
                        int_suffix.clone_from(suffix);
                        int_suffix_seen = true;
                    }
                    int_values.push(*value);
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
                        let expr = flat_literal_bracket_seq_expr(
                            all_int,
                            int_values,
                            int_suffix,
                            fallback_items,
                        );
                        if matches!(expr, Expr::NumericBracketSeq(_)) {
                            self.stats.numeric_seq_summaries += 1;
                        }
                        return Some(expr);
                    }
                }
                Token::RBracket => {
                    self.bump();
                    let expr = flat_literal_bracket_seq_expr(
                        all_int,
                        int_values,
                        int_suffix,
                        fallback_items,
                    );
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

    fn parse_call_args(&mut self) -> Result<Vec<CallArg>, ExprParseError> {
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
            vec![FlowItem::Stmt(Stmt::Expr(parse_expr(body_source.trim())?))]
        };
        Ok(Expr::Thread {
            block: Box::new(ThreadBlock::new(
                modifiers,
                nonempty_joined_name(&name_parts),
                body,
            )),
        })
    }

    fn parse_arg_expr(&mut self) -> Result<CallArg, ExprParseError> {
        if let Some(name) = self.parse_named_arg_name() {
            return Ok(CallArg::Named {
                name,
                value: Box::new(self.parse_expr_bp(0)?),
            });
        }
        if self.peek() == &Token::Op(ExprOp::Or) {
            self.bump();
            let closure_return = self.parse_closure_return_type()?;
            let body = self.parse_closure_body(closure_return.block_body)?;
            return Ok(CallArg::Positional(Expr::Closure {
                params: Vec::new(),
                return_type: closure_return.return_type,
                body: Box::new(body),
            }));
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

    fn parse_named_arg_name(&mut self) -> Option<String> {
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

    fn parse_closure_arg(&mut self) -> Result<Expr, ExprParseError> {
        self.expect(&Token::Op(ExprOp::ClosurePipe))?;
        let param_tokens = self.take_closure_param_tokens()?;
        let params_source = token_span_source(&param_tokens, &self.source).unwrap_or_default();
        let params = parse_closure_params(params_source)?;
        let closure_return = self.parse_closure_return_type()?;
        let body = self.parse_closure_body(closure_return.block_body)?;
        Ok(Expr::Closure {
            params,
            return_type: closure_return.return_type,
            body: Box::new(body),
        })
    }

    fn parse_callback_block_closure(&mut self) -> Result<Expr, ExprParseError> {
        let tokens = self.take_braced_tokens()?;
        let (params, body_tokens) = callback_block_parts(&tokens, &self.source)?;
        let body_source = token_span_source(body_tokens, &self.source)
            .unwrap_or_default()
            .trim();
        if body_source.trim().is_empty() {
            return Err(ExprParseError::new(
                "callback block requires a body expression",
            ));
        }
        Ok(Expr::Closure {
            params,
            return_type: None,
            body: Box::new(crate::parser::parse_callback_block_expr_body(body_source)),
        })
    }

    fn parse_closure_return_type(&mut self) -> Result<ClosureReturnParse, ExprParseError> {
        if self.peek() != &Token::Op(ExprOp::Arrow) {
            return Ok(ClosureReturnParse::default());
        }
        self.bump();
        let type_tokens = self.take_closure_return_type_tokens()?;
        let type_source = token_span_source(&type_tokens, &self.source).unwrap_or_default();
        let return_type =
            parse_type_ref(type_source).map_err(|error| ExprParseError::new(&error.to_string()))?;
        if self.peek() != &Token::LBrace {
            return Err(ExprParseError::new(
                "closure return type annotation requires a block body",
            ));
        }
        let body_tokens = self.take_braced_tokens()?;
        let body_source = token_span_source(&body_tokens, &self.source).unwrap_or_default();
        Ok(ClosureReturnParse {
            return_type: Some(return_type),
            block_body: Some(body_source.trim().to_owned()),
        })
    }

    fn parse_closure_body(&mut self, block_body: Option<String>) -> Result<Expr, ExprParseError> {
        block_body.map_or_else(
            || self.parse_expr_bp(0),
            |body| Ok(crate::parser::parse_callback_block_expr_body(&body)),
        )
    }

    fn take_closure_param_tokens(&mut self) -> Result<Vec<LexedToken>, ExprParseError> {
        let mut paren_depth = 0_u32;
        let mut bracket_depth = 0_u32;
        let mut brace_depth = 0_u32;
        let mut tokens = Vec::new();
        loop {
            let lexed = self.bump_lexed();
            match &lexed.token {
                Token::Op(ExprOp::ClosurePipe)
                    if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 =>
                {
                    return Ok(tokens);
                }
                Token::LParen => paren_depth = paren_depth.saturating_add(1),
                Token::RParen => paren_depth = paren_depth.saturating_sub(1),
                Token::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                Token::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                Token::LBrace => brace_depth = brace_depth.saturating_add(1),
                Token::RBrace => brace_depth = brace_depth.saturating_sub(1),
                Token::Eof => return Err(ExprParseError::new("unclosed closure parameter list")),
                _ => {}
            }
            tokens.push(lexed);
        }
    }

    fn take_closure_return_type_tokens(&mut self) -> Result<Vec<LexedToken>, ExprParseError> {
        let mut paren_depth = 0_u32;
        let mut bracket_depth = 0_u32;
        let mut tokens = Vec::new();
        loop {
            match self.peek() {
                Token::LBrace if paren_depth == 0 && bracket_depth == 0 => {
                    if tokens.is_empty() {
                        return Err(ExprParseError::new(
                            "expected closure return type after `->`",
                        ));
                    }
                    return Ok(tokens);
                }
                Token::Eof => {
                    return Err(ExprParseError::new(
                        "closure return type annotation requires a block body",
                    ));
                }
                Token::RParen if paren_depth == 0 => {
                    return Err(ExprParseError::new(
                        "closure return type annotation requires a block body",
                    ));
                }
                Token::RBracket if bracket_depth == 0 => {
                    return Err(ExprParseError::new(
                        "closure return type annotation requires a block body",
                    ));
                }
                Token::LParen => paren_depth = paren_depth.saturating_add(1),
                Token::RParen => paren_depth = paren_depth.saturating_sub(1),
                Token::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                Token::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
            tokens.push(self.bump_lexed());
        }
    }

    fn take_braced_tokens(&mut self) -> Result<Vec<LexedToken>, ExprParseError> {
        if self.peek() != &Token::LBrace {
            return Err(ExprParseError::new(&format!(
                "expected {:?}, found {:?}",
                Token::LBrace,
                self.peek()
            )));
        }
        self.cursor += 1;
        let mut depth = 1_u32;
        let mut tokens = Vec::new();
        loop {
            let lexed = self.bump_lexed();
            match lexed.token {
                Token::LBrace => {
                    depth = depth.saturating_add(1);
                    tokens.push(lexed);
                }
                Token::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Ok(tokens);
                    }
                    tokens.push(lexed);
                }
                Token::Eof => return Err(ExprParseError::new("unclosed callback block")),
                _ => tokens.push(lexed),
            }
        }
    }

    fn parse_record_fields(&mut self) -> Result<Vec<(String, Expr)>, ExprParseError> {
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

    fn take_ident(&mut self, message: &str) -> Result<String, ExprParseError> {
        match self.bump() {
            Token::Ident(name) | Token::RelativePath(name) => Ok(name),
            _ => Err(ExprParseError::new(message)),
        }
    }

    fn skip_method_turbofish_before_call(&mut self) -> bool {
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
        self.token_at(self.cursor)
    }

    fn token_at(&self, index: usize) -> &Token {
        self.tokens
            .get(index)
            .map_or(&Token::Eof, |lexed| &lexed.token)
    }

    fn peek_ident(&self, expected: &str) -> bool {
        matches!(self.peek(), Token::Ident(value) if value == expected)
    }

    fn previous(&self) -> Option<&Token> {
        self.cursor
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(|lexed| &lexed.token)
    }

    fn bump(&mut self) -> Token {
        let token = self.peek().clone();
        if !matches!(token, Token::Eof) {
            self.cursor += 1;
        }
        token
    }

    fn bump_lexed(&mut self) -> LexedToken {
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

fn flat_literal_bracket_seq_expr(
    all_int: bool,
    int_values: Vec<i64>,
    int_suffix: Option<String>,
    fallback_items: Option<Vec<Expr>>,
) -> Expr {
    if all_int {
        Expr::NumericBracketSeq(NumericBracketSeq::new(int_values, int_suffix))
    } else {
        Expr::BracketSeq(fallback_items.unwrap_or_default())
    }
}

fn literal_exprs_from_tokens(tokens: &[LexedToken]) -> Vec<Expr> {
    tokens
        .iter()
        .filter_map(|lexed| match &lexed.token {
            Token::Literal(literal) => Some(Expr::Literal(literal.clone())),
            _ => None,
        })
        .collect()
}

fn callback_block_parts<'a>(
    tokens: &'a [LexedToken],
    source: &str,
) -> Result<(Vec<ClosureParam>, &'a [LexedToken]), ExprParseError> {
    let Some(arrow) = top_level_callback_arrow(tokens) else {
        return Ok((Vec::new(), tokens));
    };
    let params = callback_block_params(&tokens[..arrow], source)?;
    Ok((params, &tokens[arrow + 1..]))
}

fn top_level_callback_arrow(tokens: &[LexedToken]) -> Option<usize> {
    let mut paren_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    let mut brace_depth = 0_u32;
    tokens.iter().enumerate().find_map(|(index, lexed)| {
        match &lexed.token {
            Token::LParen => paren_depth = paren_depth.saturating_add(1),
            Token::RParen => paren_depth = paren_depth.saturating_sub(1),
            Token::LBracket => bracket_depth = bracket_depth.saturating_add(1),
            Token::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
            Token::LBrace => brace_depth = brace_depth.saturating_add(1),
            Token::RBrace => brace_depth = brace_depth.saturating_sub(1),
            Token::Op(ExprOp::FatArrow)
                if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 =>
            {
                return Some(index);
            }
            _ => {}
        }
        None
    })
}

fn callback_block_params(
    tokens: &[LexedToken],
    source: &str,
) -> Result<Vec<ClosureParam>, ExprParseError> {
    let params_source = token_span_source(tokens, source).unwrap_or_default().trim();
    if params_source.is_empty() {
        return Err(ExprParseError::new(
            "callback block parameter list must appear before `=>`",
        ));
    }
    parse_closure_params(params_source)
}

fn parse_closure_params(source: &str) -> Result<Vec<ClosureParam>, ExprParseError> {
    let source = source.trim();
    if source.is_empty() {
        return Ok(Vec::new());
    }
    split_top_level_punctuation(source, ',')
        .into_iter()
        .map(parse_closure_param)
        .collect()
}

fn parse_closure_param(source: &str) -> Result<ClosureParam, ExprParseError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(ExprParseError::new("expected closure parameter"));
    }
    let (pattern, ty) = split_top_level_punctuation_once(source, ':')
        .map_or((source, None), |(pattern, ty)| {
            (pattern.trim(), Some(ty.trim()))
        });
    let ty = ty
        .filter(|ty| !ty.is_empty())
        .map(parse_type_ref)
        .transpose()
        .map_err(|error| {
            ExprParseError::new(&format!("invalid closure parameter type: {error}"))
        })?;
    Ok(ClosureParam::new(parse_pattern(pattern), ty))
}

fn token_span_source<'a>(tokens: &[LexedToken], source: &'a str) -> Option<&'a str> {
    let first = tokens.first()?;
    let last = tokens.last()?;
    source.get(first.start..last.end)
}

fn token_source(token: &Token) -> String {
    match token {
        Token::Ident(value)
        | Token::RelativePath(value)
        | Token::Literal(
            Literal::Float { raw: value, .. } | Literal::UnitNumber { raw: value, .. },
        ) => value.clone(),
        Token::Entity(entity) => format!("@{}", entity.body()),
        Token::LifetimePath { key, optional } => {
            format!("'{}{}", key.as_dotted(), if *optional { "?" } else { "" })
        }
        Token::Literal(Literal::String(value)) => format!("\"{value}\""),
        Token::Literal(Literal::Char { raw, .. } | Literal::Int { raw, .. }) => raw.clone(),
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
        Token::Op(op) => op.as_str().to_owned(),
        Token::Eof => String::new(),
    }
}

const fn duration_unit_suffix(unit: DurationUnit) -> &'static str {
    unit.as_str()
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
    [
        ("min", DurationUnit::Minutes),
        ("ns", DurationUnit::Nanos),
        ("us", DurationUnit::Micros),
        ("ms", DurationUnit::Millis),
        ("s", DurationUnit::Seconds),
        ("h", DurationUnit::Hours),
    ]
    .into_iter()
    .find_map(|(suffix, unit)| {
        source
            .strip_suffix(suffix)
            .filter(|amount| is_numeric_unit_amount(amount))
            .map(|amount| Literal::Duration {
                amount: amount.to_owned(),
                unit,
            })
    })
}

fn split_number_suffix(source: &str) -> (&str, &str) {
    let split = numeric_body_len(source);
    (&source[..split], &source[split..])
}

fn numeric_body_len(source: &str) -> usize {
    if let Some(rest) = source
        .strip_prefix("0x")
        .or_else(|| source.strip_prefix("0X"))
    {
        return "0x".len() + radix_digits_len(rest, 16);
    }
    if let Some(rest) = source
        .strip_prefix("0b")
        .or_else(|| source.strip_prefix("0B"))
    {
        return "0b".len() + radix_digits_len(rest, 2);
    }
    if let Some(rest) = source
        .strip_prefix("0o")
        .or_else(|| source.strip_prefix("0O"))
    {
        return "0o".len() + radix_digits_len(rest, 8);
    }
    let bytes = source.as_bytes();
    let mut index = decimal_digits_len(source);
    if bytes.get(index) == Some(&b'.') && !source[index..].starts_with("..") {
        index += 1;
        index += decimal_digits_len(&source[index..]);
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        let exponent_start = index;
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let digits_start = index;
        index += decimal_digits_len(&source[index..]);
        if source[digits_start..index]
            .chars()
            .filter(|ch| *ch != '_')
            .all(|ch| !ch.is_ascii_digit())
        {
            index = exponent_start;
        }
    }
    index
}

fn decimal_digits_len(source: &str) -> usize {
    source
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit() || *ch == '_')
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

fn radix_digits_len(source: &str, radix: u32) -> usize {
    source
        .char_indices()
        .take_while(|(_, ch)| *ch == '_' || digit_matches_radix(*ch, radix))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

fn parse_int_literal_value(number: &str) -> Option<i64> {
    let cleaned = number.replace('_', "");
    let (radix, digits) = if let Some(digits) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        (16, digits)
    } else if let Some(digits) = cleaned
        .strip_prefix("0b")
        .or_else(|| cleaned.strip_prefix("0B"))
    {
        (2, digits)
    } else if let Some(digits) = cleaned
        .strip_prefix("0o")
        .or_else(|| cleaned.strip_prefix("0O"))
    {
        (8, digits)
    } else {
        (10, cleaned.as_str())
    };
    i64::from_str_radix(digits, radix).ok()
}

fn digit_matches_radix(ch: char, radix: u32) -> bool {
    ch.is_digit(radix)
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

fn find_last_top_level_open_bracket(source: &str) -> Option<usize> {
    find_last_top_level_punctuation(source, '[')
}

fn is_numeric_unit_amount(source: &str) -> bool {
    if source.is_empty() || source.starts_with('_') || source.ends_with('_') {
        return false;
    }
    let cleaned = source.replace('_', "");
    cleaned.chars().any(|ch| ch.is_ascii_digit()) && cleaned.parse::<f64>().is_ok()
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

impl CallArg {
    /// Expression carried by this argument.
    pub fn value(&self) -> &Expr {
        match self {
            Self::Positional(value) => value,
            Self::Named { value, .. } | Self::Spread { value } => value.as_ref(),
        }
    }

    /// Name for a named argument.
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Named { name, .. } => Some(name),
            Self::Positional(_) | Self::Spread { .. } => None,
        }
    }

    /// Whether this is a positional spread argument.
    pub const fn is_spread(&self) -> bool {
        matches!(self, Self::Spread { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::{BinaryOp, Expr, parse_expr};

    #[test]
    fn parses_field_access_comparison() {
        let parsed = parse_expr("self.current < self.end")
            .expect("field access comparison parses as an expression");
        let Expr::Binary { lhs, op, rhs } = parsed else {
            panic!("expected binary expression");
        };
        assert_eq!(op, BinaryOp::Lt);
        assert!(matches!(*lhs, Expr::Field { .. }));
        assert!(matches!(*rhs, Expr::Field { .. }));
    }
}
