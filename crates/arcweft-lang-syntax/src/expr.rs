use crate::ast::{EntityRef, TextRange};
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
    EntityRef(EntityRef),
    Path(String),
    Placeholder(Placeholder),
    Tuple(Vec<Expr>),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    Pipe {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Binary {
        lhs: Box<Expr>,
        op: BinaryOp,
        rhs: Box<Expr>,
    },
    Raw(String),
}

/// Literal expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Literal {
    String(String),
    Int(i64),
    Bool(bool),
    Duration { value: u64, unit: DurationUnit },
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
    Eq,
    NotEq,
    Gte,
    Lte,
    Gt,
    Lt,
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
    Ok(parse_pipe(trimmed))
}

fn parse_pipe(source: &str) -> Expr {
    if let Some((lhs, rhs)) = split_top_level(source, "|>") {
        return Expr::Pipe {
            lhs: Box::new(parse_pipe(lhs)),
            rhs: Box::new(parse_pipe(rhs)),
        };
    }
    parse_binary(source)
}

fn parse_binary(source: &str) -> Expr {
    if source.starts_with("#<") && source.ends_with('>') {
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
    parse_postfix(source)
}

fn parse_postfix(source: &str) -> Expr {
    let source = source.trim();
    if let Some((receiver, method, args)) = split_method_call(source) {
        return Expr::MethodCall {
            receiver: Box::new(parse_postfix(receiver)),
            method: method.to_owned(),
            args: parse_arg_list(args),
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
    if let Some(entity) = parse_entity_expr(source) {
        return Expr::EntityRef(entity);
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

fn parse_string(source: &str) -> Option<String> {
    source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_owned)
}

fn parse_duration(source: &str) -> Option<Literal> {
    if let Some(value) = source.strip_suffix("ms") {
        return value.parse::<u64>().ok().map(|value| Literal::Duration {
            value,
            unit: DurationUnit::Millis,
        });
    }
    source
        .strip_suffix('s')?
        .parse::<u64>()
        .ok()
        .map(|value| Literal::Duration {
            value,
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

fn parse_arg_list(source: &str) -> Vec<Expr> {
    split_args(source).into_iter().map(parse_pipe).collect()
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
        .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | ':' | '.'))
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
