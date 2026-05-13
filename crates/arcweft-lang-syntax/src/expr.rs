use crate::ast::{EntityRef, Pattern, Stmt, TextRange};
use arcweft_source::{SourceAnchor, SourceName};
use core::fmt;

/// Expression syntax preserved for type checking and HIR lowering.
///
/// This parser records expression shape without name resolution, generic
/// instantiation, or overload decisions. Those later compiler phases should be
/// able to consume this AST while keeping source-level diagnostics precise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    Literal(Literal),
    EntityRef(EntityRef),
    Path(String),
    Placeholder(Placeholder),
    Tuple(Vec<Expr>),
    List(Vec<Expr>),
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
    Add,
    Sub,
}

/// Unary operator syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Not,
}

/// Named computation block syntax such as `result { ... }`, `task { ... }`, or `seq { ... }`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputationBlockKind {
    Result,
    Task,
    Seq,
}

/// Expression parse error.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    Ok(parse_pipe(trimmed))
}

fn parse_pipe(source: &str) -> Expr {
    if let Some(rest) = source.strip_prefix("try ") {
        return Expr::Try {
            expr: Box::new(parse_pipe(rest.trim())),
        };
    }
    if let Some((lhs, rhs)) = split_top_level(source, "|>") {
        return Expr::Pipe {
            lhs: Box::new(parse_pipe(lhs)),
            rhs: Box::new(parse_pipe(rhs)),
        };
    }
    if let Some((params, body)) = split_closure(source) {
        return Expr::Closure {
            params,
            body: Box::new(parse_pipe(body)),
        };
    }
    if let Some((name, value)) = split_named_arg(source) {
        return Expr::NamedArg {
            name: name.to_owned(),
            value: Box::new(parse_pipe(value)),
        };
    }
    parse_binary(source)
}

fn parse_binary(source: &str) -> Expr {
    if source.starts_with("#<") && source.ends_with('>') {
        return parse_postfix(source);
    }
    for (needle, op) in [
        ("=>", BinaryOp::Implies),
        ("||", BinaryOp::Or),
        ("&&", BinaryOp::And),
        (" in ", BinaryOp::In),
    ] {
        if let Some((lhs, rhs)) = split_top_level(source, needle) {
            return Expr::Binary {
                lhs: Box::new(parse_binary(lhs)),
                op,
                rhs: Box::new(parse_binary(rhs)),
            };
        }
    }
    if let Some(range) = parse_range_expr(source) {
        return range;
    }
    if split_call(source).is_some() {
        return parse_postfix(source);
    }
    for (needle, op) in [
        ("==", BinaryOp::Eq),
        ("!=", BinaryOp::NotEq),
        (">=", BinaryOp::Gte),
        ("<=", BinaryOp::Lte),
        (">", BinaryOp::Gt),
        ("<", BinaryOp::Lt),
    ] {
        if let Some((lhs, rhs)) = split_top_level(source, needle) {
            return Expr::Binary {
                lhs: Box::new(parse_postfix(lhs)),
                op,
                rhs: Box::new(parse_postfix(rhs)),
            };
        }
    }
    for (needle, op) in [("+", BinaryOp::Add), ("-", BinaryOp::Sub)] {
        if let Some((lhs, rhs)) = split_top_level_arithmetic(source, needle) {
            return Expr::Binary {
                lhs: Box::new(parse_postfix(lhs)),
                op,
                rhs: Box::new(parse_postfix(rhs)),
            };
        }
    }
    parse_postfix(source)
}

fn parse_postfix(source: &str) -> Expr {
    let source = source.trim();
    if let Some(inner) = source.strip_prefix('!') {
        if !inner.trim().is_empty() {
            return Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(parse_postfix(inner.trim())),
            };
        }
    }
    if let Some(inner) = source.strip_suffix('?') {
        if !inner.trim().is_empty() {
            return Expr::Try {
                expr: Box::new(parse_postfix(inner.trim())),
            };
        }
    }
    if let Some((target, content)) = split_bracket_postfix(source) {
        if looks_like_index_expr(content) {
            return Expr::Index {
                target: Box::new(parse_postfix(target)),
                index: Box::new(parse_pipe(content)),
            };
        }
        return Expr::DialogueCall {
            callee: Box::new(parse_postfix(target)),
            content: content.to_owned(),
        };
    }
    if let Some((receiver, method, args)) = split_method_call(source) {
        return Expr::MethodCall {
            receiver: Box::new(parse_postfix(receiver)),
            method: method.to_owned(),
            args: parse_arg_list(args),
        };
    }
    if let Some((target, field)) = split_field_access(source) {
        return Expr::Field {
            target: Box::new(parse_postfix(target)),
            field: field.to_owned(),
        };
    }
    if let Some((callee, args)) = split_call(source) {
        return Expr::Call {
            callee: Box::new(parse_postfix(callee)),
            args: parse_arg_list(args),
        };
    }
    parse_atom(source)
}

fn parse_atom(source: &str) -> Expr {
    let source = source.trim();
    if source == "_" {
        return Expr::Placeholder(Placeholder::Partial);
    }
    if source == "^" {
        return Expr::Placeholder(Placeholder::PipeLeft);
    }
    if source == "true" {
        return Expr::Literal(Literal::Bool(true));
    }
    if source == "false" {
        return Expr::Literal(Literal::Bool(false));
    }
    if let Some(value) = parse_string(source) {
        return Expr::Literal(Literal::String(value));
    }
    if let Some(value) = parse_duration(source) {
        return Expr::Literal(value);
    }
    if let Ok(value) = source.parse::<i64>() {
        return Expr::Literal(Literal::Int(value));
    }
    if is_float_literal(source) {
        return Expr::Literal(Literal::Float(source.to_owned()));
    }
    if let Some(entity) = parse_entity_expr(source) {
        return Expr::EntityRef(entity);
    }
    if let Some(items) = parse_list_expr(source) {
        return Expr::List(items);
    }
    if let Some((path, fields)) = parse_record_expr(source) {
        return Expr::Record { path, fields };
    }
    if let Some(fields) = parse_record_literal(source) {
        return Expr::RecordLiteral(fields);
    }
    if let Some(inner) = source
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    {
        let args = split_args(inner);
        if args.len() > 1 {
            return Expr::Tuple(args.into_iter().map(parse_pipe).collect());
        }
        return parse_pipe(inner);
    }
    if is_path_like(source) {
        return Expr::Path(source.to_owned());
    }
    Expr::Raw(source.to_owned())
}

fn parse_list_expr(source: &str) -> Option<Vec<Expr>> {
    let inner = source.strip_prefix('[')?.strip_suffix(']')?;
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    Some(split_args(inner).into_iter().map(parse_pipe).collect())
}

fn parse_record_expr(source: &str) -> Option<(String, Vec<(String, Expr)>)> {
    let open = find_top_level_char(source, '{')?;
    let close = source.rfind('}')?;
    if open >= close || close != source.len() - 1 {
        return None;
    }
    let path = source[..open].trim();
    if path.is_empty() || !is_path_like(path) {
        return None;
    }
    let fields = parse_record_fields(&source[open + 1..close])?;
    Some((path.to_owned(), fields))
}

fn parse_record_literal(source: &str) -> Option<Vec<(String, Expr)>> {
    let inner = source.strip_prefix('{')?.strip_suffix('}')?;
    parse_record_fields(inner)
}

fn parse_record_fields(source: &str) -> Option<Vec<(String, Expr)>> {
    if source.trim().is_empty() {
        return Some(Vec::new());
    }
    source
        .lines()
        .flat_map(|line| line.split(','))
        .map(|part| {
            let part = part.trim().trim_end_matches(',');
            if part.is_empty() {
                return Some(None);
            }
            let (name, value) = part
                .split_once('=')
                .or_else(|| part.split_once(':'))
                .map_or((part, part), |(name, value)| (name.trim(), value.trim()));
            is_identifier(name).then(|| Some((name.to_owned(), parse_pipe(value))))
        })
        .collect::<Option<Vec<_>>>()
        .map(|fields| fields.into_iter().flatten().collect())
}

fn parse_range_expr(source: &str) -> Option<Expr> {
    if let Some((start, end)) = split_top_level_range(source, "..=") {
        return Some(Expr::Range {
            start: (!start.is_empty()).then(|| Box::new(parse_pipe(start))),
            end: (!end.is_empty()).then(|| Box::new(parse_pipe(end))),
            inclusive: true,
        });
    }
    let (start, end) = split_top_level_range(source, "..")?;
    Some(Expr::Range {
        start: (!start.is_empty()).then(|| Box::new(parse_pipe(start))),
        end: (!end.is_empty()).then(|| Box::new(parse_pipe(end))),
        inclusive: false,
    })
}

fn parse_string(source: &str) -> Option<String> {
    source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_owned)
}

fn find_top_level_char(source: &str, needle: char) -> Option<usize> {
    let mut depth = 0_i32;
    let mut in_string = false;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' | '[' if !in_string => depth += 1,
            ')' | ']' if !in_string => depth -= 1,
            _ => {}
        }
        if depth == 0 && !in_string && ch == needle {
            return Some(index);
        }
    }
    None
}

fn split_top_level_range<'a>(source: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    let mut depth = 0_i32;
    let mut in_string = false;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth -= 1,
            _ => {}
        }
        if depth == 0 && !in_string && source[index..].starts_with(needle) {
            let lhs = source[..index].trim();
            let rhs = source[index + needle.len()..].trim();
            return Some((lhs, rhs));
        }
    }
    None
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

fn parse_entity_expr(source: &str) -> Option<EntityRef> {
    if let Some(body) = source
        .strip_prefix("#<")
        .and_then(|value| value.strip_suffix('>'))
    {
        return Some(EntityRef::new(
            body.to_owned(),
            true,
            TextRange::new(0, source.len()),
        ));
    }
    let body = source.strip_prefix('#')?;
    if body.is_empty() || body.chars().any(char::is_whitespace) {
        return None;
    }
    Some(EntityRef::new(
        body.to_owned(),
        false,
        TextRange::new(0, source.len()),
    ))
}

fn split_call(source: &str) -> Option<(&str, &str)> {
    let close = source.strip_suffix(')')?;
    let open = find_last_top_level_open_paren(close)?;
    let callee = close[..open].trim();
    if callee.is_empty() || callee.ends_with('.') {
        return None;
    }
    Some((callee, &close[open + 1..]))
}

fn split_method_call(source: &str) -> Option<(&str, &str, &str)> {
    let (callee, args) = split_call(source)?;
    let dot = find_last_top_level_dot(callee)?;
    let receiver = callee[..dot].trim();
    let method = callee[dot + 1..].trim();
    if receiver.is_empty() || method.is_empty() {
        return None;
    }
    Some((receiver, method, args))
}

fn split_field_access(source: &str) -> Option<(&str, &str)> {
    if source.starts_with('#') || is_float_literal(source) || parse_duration(source).is_some() {
        return None;
    }
    let dot = find_last_top_level_dot(source)?;
    let target = source[..dot].trim();
    let field = source[dot + 1..].trim();
    let target_can_have_field =
        matches!(target, "_" | "^") || target.ends_with([')', ']']) || target.contains(')');
    if !target_can_have_field || target.is_empty() || field.is_empty() || !is_identifier(field) {
        return None;
    }
    Some((target, field))
}

fn parse_arg_list(source: &str) -> Vec<Expr> {
    split_args(source).into_iter().map(parse_pipe).collect()
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

fn split_args(source: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    let mut in_string = false;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth -= 1,
            ',' if depth == 0 && !in_string => {
                let arg = source[start..index].trim();
                if !arg.is_empty() {
                    args.push(arg);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        args.push(tail);
    }
    args
}

fn split_top_level<'a>(source: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    let mut depth = 0_i32;
    let mut in_string = false;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth -= 1,
            _ => {}
        }
        if depth == 0 && !in_string && source[index..].starts_with(needle) {
            let lhs = source[..index].trim();
            let rhs = source[index + needle.len()..].trim();
            if !lhs.is_empty() && !rhs.is_empty() {
                return Some((lhs, rhs));
            }
        }
    }
    None
}

fn split_closure(source: &str) -> Option<(Vec<String>, &str)> {
    let rest = source.strip_prefix('|')?;
    let close = rest.find('|')?;
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
    let (name, value) = split_top_level(source, "=")?;
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

fn split_top_level_arithmetic<'a>(source: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    if source.starts_with('#') {
        return None;
    }
    let (lhs, rhs) = split_top_level(source, needle)?;
    if lhs.is_empty() || rhs.is_empty() || lhs.ends_with(['e', 'E']) {
        return None;
    }
    Some((lhs, rhs))
}

fn find_last_top_level_open_paren(source: &str) -> Option<usize> {
    let mut depth = 0_i32;
    let mut last = None;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' => {
                if depth == 0 {
                    last = Some(index);
                }
                depth += 1;
            }
            ')' => depth -= 1,
            _ => {}
        }
    }
    last
}

fn find_last_top_level_open_bracket(source: &str) -> Option<usize> {
    let mut paren_depth = 0_i32;
    let mut bracket_depth = 0_i32;
    let mut last = None;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '[' => {
                if paren_depth == 0 && bracket_depth == 0 {
                    last = Some(index);
                }
                bracket_depth += 1;
            }
            ']' => bracket_depth -= 1,
            _ => {}
        }
    }
    last
}

fn find_last_top_level_dot(source: &str) -> Option<usize> {
    let mut depth = 0_i32;
    let mut last = None;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '.' if depth == 0 => last = Some(index),
            _ => {}
        }
    }
    last
}

fn is_path_like(source: &str) -> bool {
    source
        .chars()
        .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | ':' | '.' | '<' | '>' | ','))
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

fn is_float_literal(source: &str) -> bool {
    !source.is_empty()
        && source.chars().filter(|ch| *ch == '.').count() == 1
        && source.chars().any(|ch| ch.is_ascii_digit())
        && source.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
}

fn looks_like_index_expr(source: &str) -> bool {
    let trimmed = source.trim();
    trimmed.starts_with('#')
        || trimmed.starts_with('"')
        || trimmed == "_"
        || trimmed == "^"
        || trimmed == "true"
        || trimmed == "false"
        || trimmed.parse::<i64>().is_ok()
        || is_path_like(trimmed)
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

impl fmt::Display for ExprParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ExprParseError {}
