use core::fmt;

use crate::ast::{DocBlock, Pattern, TextRange};
use crate::pattern::parse_pattern;

/// Lifetime name used in Arcweft type syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifetimeName {
    name: String,
}

/// Type syntax preserved for later borrow and suspension-boundary checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeRef {
    Never,
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

/// Function signature shape preserved before semantic generic resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FnSignature {
    name: String,
    generic_params: Vec<GenericParam>,
    param_groups: Vec<FnParamGroup>,
    return_type: Option<TypeRef>,
    where_clauses: Vec<WhereClause>,
}

/// Generic parameter declared by a function signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenericParam {
    Lifetime(LifetimeName),
    Type(String),
}

/// One parenthesized parameter group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FnParamGroup {
    params: Vec<FnParam>,
}

/// One function parameter pattern and type annotation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FnParam {
    doc: Option<DocBlock>,
    pattern: Pattern,
    ty: TypeRef,
}

/// One `where` clause predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhereClause {
    subject: TypeRef,
    bounds: Vec<TypeRef>,
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

/// Parses the head of a function signature, including generics and curried parameter groups.
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
    let generic_params = if let Some((params, tail)) = take_angle_group(rest) {
        rest = tail.trim_start();
        parse_generic_params(params)?
    } else {
        Vec::new()
    };
    let (param_groups, mut rest) = parse_fn_param_groups(rest)?;
    let (before_where, where_part) = split_keyword_top_level(rest.trim_start(), "where");
    rest = before_where.trim_start();
    let return_type = if let Some(tail) = rest.strip_prefix("->") {
        let ty = tail.trim();
        if ty.is_empty() {
            return Err(TypeParseError::new("expected return type after `->`"));
        }
        let ty = parse_type_ref(ty)?;
        if type_ref_has_whitespace_path(&ty) {
            return Err(TypeParseError::new("unexpected tokens after return type"));
        }
        Some(ty)
    } else if rest.is_empty() {
        None
    } else {
        return Err(TypeParseError::new(
            "unexpected tokens after parameter list",
        ));
    };
    let where_clauses = where_part.map_or_else(|| Ok(Vec::new()), parse_where_clauses)?;
    Ok(FnSignature {
        name,
        generic_params,
        param_groups,
        return_type,
        where_clauses,
    })
}

fn parse_fn_param_groups(source: &str) -> Result<(Vec<FnParamGroup>, &str), TypeParseError> {
    let mut rest = source.trim_start();
    let mut groups = Vec::new();
    while let Some(inner) = rest.strip_prefix('(') {
        let close = find_matching_paren(inner)
            .ok_or_else(|| TypeParseError::new("unclosed parameter list"))?;
        let params = split_top_level(&inner[..close], ',')
            .into_iter()
            .filter(|param| !param.is_empty())
            .map(parse_fn_param)
            .collect::<Result<Vec<_>, _>>()?;
        groups.push(FnParamGroup { params });
        rest = inner[close + 1..].trim_start();
    }
    if groups.is_empty() {
        return Err(TypeParseError::new("expected parameter list"));
    }
    Ok((groups, rest))
}

fn parse_fn_param(source: &str) -> Result<FnParam, TypeParseError> {
    if matches!(source.trim(), "self" | "&self" | "&mut self" | "mut self") {
        return Ok(FnParam {
            doc: None,
            pattern: Pattern::Ident("self".to_owned()),
            ty: TypeRef::Path("Self".to_owned()),
        });
    }
    let (doc, source) = take_param_doc(source);
    let (pattern, ty) = split_top_level_once(source, ':')
        .ok_or_else(|| TypeParseError::new("expected `pattern: Type` parameter"))?;
    Ok(FnParam {
        doc,
        pattern: parse_pattern(pattern),
        ty: parse_type_ref(ty.trim())?,
    })
}

fn take_param_doc(source: &str) -> (Option<DocBlock>, &str) {
    let mut docs = Vec::new();
    let mut consumed = 0;
    for line in source.lines() {
        let trimmed = line.trim();
        let Some(text) = trimmed.strip_prefix("///") else {
            break;
        };
        docs.push(text.strip_prefix(' ').unwrap_or(text).to_owned());
        consumed += line.len() + 1;
    }
    if docs.is_empty() {
        return (None, source.trim());
    }
    let rest = source.get(consumed..).unwrap_or_default().trim();
    (
        Some(DocBlock::new(
            docs.join("\n"),
            TextRange::new(0, consumed.saturating_sub(1)),
        )),
        rest,
    )
}

fn parse_type_atom(source: &str) -> TypeRef {
    if matches!(source, "!" | "Never") {
        return TypeRef::Never;
    }
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

fn take_angle_group(source: &str) -> Option<(&str, &str)> {
    let inner = source.strip_prefix('<')?;
    let close = find_matching_angle(inner)?;
    Some((&inner[..close], &inner[close + 1..]))
}

fn find_matching_angle(source: &str) -> Option<usize> {
    let mut depth = 0_i32;
    let mut previous = None;
    for (index, ch) in source.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' if depth == 0 && previous != Some('-') => return Some(index),
            '>' if previous != Some('-') => depth -= 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        previous = Some(ch);
    }
    None
}

fn parse_generic_params(source: &str) -> Result<Vec<GenericParam>, TypeParseError> {
    split_top_level(source, ',')
        .into_iter()
        .map(|param| {
            let param = param.trim();
            if param.is_empty() {
                return Err(TypeParseError::new("empty generic parameter"));
            }
            if param.starts_with('\'') {
                Ok(GenericParam::Lifetime(parse_lifetime_name(param)))
            } else {
                Ok(GenericParam::Type(param.to_owned()))
            }
        })
        .collect()
}

fn parse_where_clauses(source: &str) -> Result<Vec<WhereClause>, TypeParseError> {
    if source.trim().is_empty() {
        return Err(TypeParseError::new("expected where clause predicate"));
    }
    split_top_level(source, ',')
        .into_iter()
        .map(|clause| {
            let (subject, bounds) = split_top_level_once(clause, ':')
                .ok_or_else(|| TypeParseError::new("expected `Type: Bound` where predicate"))?;
            let bounds = bounds
                .split('+')
                .map(str::trim)
                .filter(|bound| !bound.is_empty())
                .map(parse_type_ref)
                .collect::<Result<Vec<_>, _>>()?;
            if bounds.is_empty() {
                return Err(TypeParseError::new("expected where clause bound"));
            }
            Ok(WhereClause {
                subject: parse_type_ref(subject.trim())?,
                bounds,
            })
        })
        .collect()
}

fn split_keyword_top_level<'a>(source: &'a str, keyword: &str) -> (&'a str, Option<&'a str>) {
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut previous = None;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '<' | '[' | '(' | '{' if !in_string => depth += 1,
            '>' if !in_string && previous != Some('-') => depth -= 1,
            ']' | ')' | '}' if !in_string => depth -= 1,
            _ => {}
        }
        if depth == 0
            && !in_string
            && source[index..].starts_with(keyword)
            && source[..index]
                .chars()
                .last()
                .is_none_or(char::is_whitespace)
            && source[index + keyword.len()..]
                .chars()
                .next()
                .is_none_or(char::is_whitespace)
        {
            return (
                &source[..index],
                Some(source[index + keyword.len()..].trim()),
            );
        }
        previous = Some(ch);
    }
    (source, None)
}

fn type_ref_has_whitespace_path(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Never => false,
        TypeRef::Path(path) => path.chars().any(char::is_whitespace),
        TypeRef::Generic { base, args } => {
            base.chars().any(char::is_whitespace) || args.iter().any(type_ref_has_whitespace_path)
        }
        TypeRef::Ref { inner, .. } | TypeRef::Slice(inner) => type_ref_has_whitespace_path(inner),
    }
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

    /// Declared generic parameters.
    pub fn generic_params(&self) -> &[GenericParam] {
        &self.generic_params
    }

    /// Parenthesized parameter groups in declaration order.
    pub fn param_groups(&self) -> &[FnParamGroup] {
        &self.param_groups
    }

    /// Declared return type, if present.
    pub const fn return_type(&self) -> Option<&TypeRef> {
        self.return_type.as_ref()
    }

    /// Where-clause predicates.
    pub fn where_clauses(&self) -> &[WhereClause] {
        &self.where_clauses
    }
}

impl GenericParam {
    /// Generic parameter as a lifetime, when applicable.
    pub const fn as_lifetime(&self) -> Option<&LifetimeName> {
        match self {
            Self::Lifetime(lifetime) => Some(lifetime),
            Self::Type(_) => None,
        }
    }

    /// Generic parameter as a type name, when applicable.
    pub fn as_type(&self) -> Option<&str> {
        match self {
            Self::Type(name) => Some(name),
            Self::Lifetime(_) => None,
        }
    }
}

impl FnParamGroup {
    /// Parameters in this parenthesized group.
    pub fn params(&self) -> &[FnParam] {
        &self.params
    }
}

impl WhereClause {
    /// Type constrained by this predicate.
    pub const fn subject(&self) -> &TypeRef {
        &self.subject
    }

    /// Bounds that the subject must satisfy.
    pub fn bounds(&self) -> &[TypeRef] {
        &self.bounds
    }
}

impl FnParam {
    /// Markdown documentation attached to this parameter.
    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

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
