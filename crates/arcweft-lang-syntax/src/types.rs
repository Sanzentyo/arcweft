use core::fmt;

use crate::ast::Pattern;
use crate::pattern::parse_pattern;

/// Lifetime name used in Arcweft type syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifetimeName {
    name: String,
}

/// Type syntax preserved for later borrow and suspension-boundary checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeRef {
    Path(String),
    Generic {
        base: String,
        args: Vec<TypeRef>,
    },
    Ref {
        lifetime: Option<LifetimeName>,
        inner: Box<TypeRef>,
    },
    Slice(Box<TypeRef>),
}

/// Function signature shape needed to expose lifetime parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FnSignature {
    name: String,
    lifetimes: Vec<LifetimeName>,
    params: Vec<FnParam>,
    return_type: Option<TypeRef>,
}

/// One function parameter pattern and type annotation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FnParam {
    pattern: Pattern,
    ty: TypeRef,
}

/// Type syntax parse failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeParseError {
    message: String,
}

/// Parses an Arcweft type expression.
pub fn parse_type_ref(source: &str) -> Result<TypeRef, TypeParseError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(TypeParseError::new("expected type"));
    }
    Ok(parse_type_atom(source))
}

/// Parses the head of a function signature, including lifetime parameters.
pub fn parse_fn_signature(source: &str) -> Result<FnSignature, TypeParseError> {
    let source = source.trim();
    let after_fn = source
        .strip_prefix("fn ")
        .ok_or_else(|| TypeParseError::new("expected `fn` signature"))?;
    let name_end = after_fn
        .char_indices()
        .take_while(|(_, ch)| ch.is_alphanumeric() || *ch == '_')
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .ok_or_else(|| TypeParseError::new("expected function name"))?;
    let name = after_fn[..name_end].to_owned();
    let mut rest = after_fn[name_end..].trim_start();
    let lifetimes = rest
        .strip_prefix('<')
        .and_then(|value| value.split_once('>'))
        .map_or_else(Vec::new, |(params, tail)| {
            rest = tail.trim_start();
            params
                .split(',')
                .map(str::trim)
                .filter(|param| param.starts_with('\''))
                .map(parse_lifetime_name)
                .collect()
        });
    let (params, rest) = parse_fn_params(rest)?;
    let return_type = rest
        .trim_start()
        .strip_prefix("->")
        .map(str::trim)
        .filter(|tail| !tail.is_empty())
        .map(parse_type_ref)
        .transpose()?;
    Ok(FnSignature {
        name,
        lifetimes,
        params,
        return_type,
    })
}

fn parse_fn_params(source: &str) -> Result<(Vec<FnParam>, &str), TypeParseError> {
    let source = source.trim_start();
    let inner = source
        .strip_prefix('(')
        .ok_or_else(|| TypeParseError::new("expected parameter list"))?;
    let close =
        find_matching_paren(inner).ok_or_else(|| TypeParseError::new("unclosed parameter list"))?;
    let params = split_top_level(&inner[..close], ',')
        .into_iter()
        .filter(|param| !param.is_empty())
        .map(parse_fn_param)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((params, &inner[close + 1..]))
}

fn parse_fn_param(source: &str) -> Result<FnParam, TypeParseError> {
    if matches!(source.trim(), "self" | "&self" | "&mut self" | "mut self") {
        return Ok(FnParam {
            pattern: Pattern::Ident("self".to_owned()),
            ty: TypeRef::Path("Self".to_owned()),
        });
    }
    let (pattern, ty) = split_top_level_once(source, ':')
        .ok_or_else(|| TypeParseError::new("expected `pattern: Type` parameter"))?;
    Ok(FnParam {
        pattern: parse_pattern(pattern),
        ty: parse_type_ref(ty.trim())?,
    })
}

fn parse_type_atom(source: &str) -> TypeRef {
    if let Some(rest) = source.strip_prefix('&') {
        let rest = rest.trim_start();
        let (lifetime, inner) = if rest.starts_with('\'') {
            let len = rest
                .char_indices()
                .take_while(|(_, ch)| ch.is_alphanumeric() || matches!(*ch, '\'' | '_'))
                .map(|(index, ch)| index + ch.len_utf8())
                .last()
                .unwrap_or(0);
            (
                Some(parse_lifetime_name(&rest[..len])),
                rest[len..].trim_start(),
            )
        } else {
            (None, rest)
        };
        return TypeRef::Ref {
            lifetime,
            inner: Box::new(parse_type_atom(inner)),
        };
    }
    if let Some(inner) = source
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return TypeRef::Slice(Box::new(parse_type_atom(inner.trim())));
    }
    if let Some((base, args)) = split_generic_type(source) {
        return TypeRef::Generic {
            base: base.to_owned(),
            args: split_type_args(args)
                .into_iter()
                .map(parse_type_atom)
                .collect(),
        };
    }
    TypeRef::Path(source.to_owned())
}

fn split_generic_type(source: &str) -> Option<(&str, &str)> {
    let (base, args) = source.split_once('<')?;
    let args = args.strip_suffix('>')?;
    Some((base.trim(), args))
}

fn split_type_args(source: &str) -> Vec<&str> {
    split_top_level(source, ',')
}

fn find_matching_paren(source: &str) -> Option<usize> {
    let mut depth = 0_i32;
    let mut previous = None;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '<' | '[' | '{' => depth += 1,
            ')' if depth == 0 => return Some(index),
            '>' if previous != Some('-') => depth -= 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        previous = Some(ch);
    }
    None
}

fn split_top_level(source: &str, delimiter: char) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    let mut previous = None;
    for (index, ch) in source.char_indices() {
        match ch {
            '<' | '[' | '(' | '{' => depth += 1,
            '>' if previous != Some('-') => depth -= 1,
            ']' | ')' | '}' => depth -= 1,
            ch if ch == delimiter && depth == 0 => {
                args.push(source[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
        previous = Some(ch);
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        args.push(tail);
    }
    args
}

fn split_top_level_once(source: &str, delimiter: char) -> Option<(&str, &str)> {
    let mut depth = 0_i32;
    let mut previous = None;
    for (index, ch) in source.char_indices() {
        match ch {
            '<' | '[' | '(' | '{' => depth += 1,
            '>' if previous != Some('-') => depth -= 1,
            ']' | ')' | '}' => depth -= 1,
            ch if ch == delimiter && depth == 0 => {
                return Some((&source[..index], &source[index + ch.len_utf8()..]));
            }
            _ => {}
        }
        previous = Some(ch);
    }
    None
}

fn parse_lifetime_name(source: &str) -> LifetimeName {
    LifetimeName {
        name: source.trim_start_matches('\'').to_owned(),
    }
}

impl LifetimeName {
    /// Lifetime name without the leading apostrophe.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl FnSignature {
    /// Function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Declared lifetime parameters.
    pub fn lifetimes(&self) -> &[LifetimeName] {
        &self.lifetimes
    }

    /// Declared function parameters.
    pub fn params(&self) -> &[FnParam] {
        &self.params
    }

    /// Declared return type, if present.
    pub const fn return_type(&self) -> Option<&TypeRef> {
        self.return_type.as_ref()
    }
}

impl FnParam {
    /// Parameter binding pattern.
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    /// Parameter type annotation.
    pub const fn ty(&self) -> &TypeRef {
        &self.ty
    }
}

impl TypeParseError {
    fn new(message: &str) -> Self {
        Self {
            message: message.to_owned(),
        }
    }
}

impl fmt::Display for TypeParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TypeParseError {}
