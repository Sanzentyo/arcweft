use crate::ast::common::TextRange;
use crate::ast::dialogue::DialogueContent;
use crate::ast::flow::{FlowItem, Stmt, ThreadBlock, ThreadModifier};
use crate::ast::ids::{
    EntityRef, EntityRefSyntax, FamilyRelativeEntityRef, RelativeId, RelativeIdSpelling,
};
use crate::ast::line_plan::LinePlan;
use crate::ast::pattern::Pattern;
use crate::cst::{
    find_last_top_level_punctuation, split_leading_entity_ref_parts,
    split_leading_relative_entity_ref,
};
use crate::reference::{BorrowExpr, DerefExpr};
use crate::types::{TypeRef, parse_type_ref};
use std::{
    fmt,
    ops::{Add, Deref},
};
use thiserror::Error;

mod char_literal;
mod closure_parse;
mod closure_source;
mod control_parse;
mod numeric;
mod pipe_scope;
mod source_ranges;

use closure_parse::parse_closure_params;
use closure_source::ClosureBodySource;
pub use numeric::{
    IntLiteral, IntLiteralValueError, IntRadix, IntSuffix, NumericBracketSeq,
    NumericBracketSeqError,
};
use numeric::{digit_matches_radix, split_number_suffix};
pub use source_ranges::{
    ExprSourceRange, collect_dialogue_call_content_ranges, collect_expr_source_ranges,
};

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

/// Dot selector in source syntax before name and type resolution.
///
/// `target.member` can later resolve to a module path, field access, associated
/// namespace item, enum constructor, trait/inherent method, environment method,
/// or callable value projection. The syntax layer does not call `target` a
/// receiver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectExpr {
    target: Box<Expr>,
    member: Name,
}

impl SelectExpr {
    /// Builds a source-level dot selector.
    pub fn new(target: Expr, member: Name) -> Self {
        Self {
            target: Box::new(target),
            member,
        }
    }

    /// Returns the selected target expression.
    pub const fn target(&self) -> &Expr {
        &self.target
    }

    /// Returns the selected member name.
    pub const fn member(&self) -> &Name {
        &self.member
    }

    /// Splits this selector into owned parts.
    pub fn into_parts(self) -> (Expr, Name) {
        (*self.target, self.member)
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
    Select(SelectExpr),
    DialogueCall {
        callee: Box<Expr>,
        content: Box<DialogueContent>,
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
    Borrow(BorrowExpr),
    Deref(DerefExpr),
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

impl Expr {
    /// Builds a source-level dot selector.
    pub fn select(target: Expr, member: impl Into<String>) -> Self {
        Self::Select(SelectExpr::new(target, Name::new(member.into())))
    }

    /// Builds a source-level call expression.
    pub fn call(callee: Expr, args: Vec<CallArg>) -> Self {
        Self::Call {
            callee: Box::new(callee),
            args,
        }
    }

    /// Builds a call whose callee is a source-level dot selector.
    pub fn selected_call(target: Expr, member: impl Into<String>, args: Vec<CallArg>) -> Self {
        Self::call(Self::select(target, member), args)
    }

    /// Returns the selector payload when this expression is a dot selector.
    pub fn as_select(&self) -> Option<&SelectExpr> {
        match self {
            Self::Select(select) => Some(select),
            _ => None,
        }
    }

    /// Returns a dotted syntax label when this expression is only path/select nodes.
    pub fn dotted_selector_label(&self) -> Option<String> {
        match self {
            Self::Path(path) => Some(path.as_label().to_owned()),
            Self::Select(select) => {
                let mut label = select.target().dotted_selector_label()?;
                label.push('.');
                label.push_str(select.member().as_str());
                Some(label)
            }
            _ => None,
        }
    }
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
    Int(IntLiteral),
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
    Milli,
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
            "milli" => Some(Self::Milli),
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
            Self::Milli => "milli",
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
    code: &'static str,
    range: TextRange,
    message: String,
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
    parse_expr_with_stats_at(source, 0)
}

pub(crate) fn parse_expr_at(source: &str, base: usize) -> Result<Expr, ExprParseError> {
    parse_expr_with_stats_at(source, base).map(|parsed| parsed.expr)
}

fn parse_expr_with_stats_at(source: &str, base: usize) -> Result<ParsedExpr, ExprParseError> {
    let trimmed = source.trim();
    let base = base + subslice_offset(source, trimmed);
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
            ClosureBodySource::Expr(body) => {
                parse_expr_with_stats_at(body, base + subslice_offset(trimmed, body))?
            }
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
        let parsed_target =
            parse_expr_with_stats_at(target, base + subslice_offset(trimmed, target))?;
        let parsed_index = parse_expr_with_stats_at(index, base + subslice_offset(trimmed, index))
            .unwrap_or_else(|_| ParsedExpr {
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
    let parsed = ExprParser::new_at(trimmed, base).parse()?;
    if let Some((kind, range)) = collect_expr_source_ranges(
        &parsed.expr,
        trimmed,
        TextRange::new(base, base + trimmed.len()),
    )
    .into_iter()
    .find_map(|entry| {
        crate::assertion::classify_expression_call(entry.expr()).map(|kind| (kind, entry.range()))
    }) {
        let (code, message) = match kind {
            crate::assertion::AssertionExpressionCall::Known(mode) => (
                "syntax.assert.statement_only",
                format!(
                    "assert.{} is a statement and cannot be used as an expression",
                    mode.keyword()
                ),
            ),
            crate::assertion::AssertionExpressionCall::UnknownMode => (
                "syntax.assert.unknown_mode",
                "unknown assertion mode".to_owned(),
            ),
        };
        return Err(ExprParseError::at(code, &message, range));
    }
    Ok(parsed)
}

fn subslice_offset(source: &str, fragment: &str) -> usize {
    (fragment.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
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
    Amp,
    Star,
    Op(ExprOp),
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExprOp {
    NotEq,
    ThinArrow,
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
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NotEq => "!=",
            Self::ThinArrow => "->",
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

mod lexer;
mod pratt;
mod prefix;

use lexer::Lexer;
use pratt::ExprParser;

fn flat_literal_bracket_seq_expr(
    all_int: bool,
    int_literals: Vec<IntLiteral>,
    fallback_items: Option<Vec<Expr>>,
) -> Expr {
    if all_int {
        Expr::NumericBracketSeq(
            NumericBracketSeq::new(int_literals)
                .expect("flat integer sequence parser checked the common suffix"),
        )
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
        Token::Literal(Literal::Char { raw, .. }) => raw.clone(),
        Token::Literal(Literal::Int(literal)) => literal.raw().to_owned(),
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
        Token::Amp => "&".to_owned(),
        Token::Star => "*".to_owned(),
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
        Self::at("syntax.expr.parse", message, TextRange::new(0, 0))
    }

    fn at(code: &'static str, message: &str, range: TextRange) -> Self {
        Self {
            code,
            range,
            message: message.to_owned(),
        }
    }

    /// Stable diagnostic code for this expression parse failure.
    pub fn code(&self) -> &str {
        self.code
    }

    /// Exact primary byte range in the parsed expression source.
    pub const fn range(&self) -> TextRange {
        self.range
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
mod tests;
