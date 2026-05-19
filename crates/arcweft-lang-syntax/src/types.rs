use thiserror::Error;

use crate::ast::{DocBlock, Pattern, TextRange};
use crate::cst::{
    find_matching_angle_group, find_matching_punctuation, find_top_level_punctuation,
    split_leading_ident, split_leading_lifetime, split_top_level_keyword_once,
    split_top_level_punctuation, split_top_level_punctuation_once, take_doc_comment_prefix,
};
use crate::expr::{Expr, parse_expr};
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
    ConstInt(usize),
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
    default: Option<Expr>,
}

/// One `where` clause predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhereClause {
    subject: TypeRef,
    bounds: Vec<TypeRef>,
}

/// Type syntax parse failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
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
    let (name, mut rest) = split_leading_ident(after_fn)
        .ok_or_else(|| TypeParseError::new("expected function name"))?;
    let name = name.to_owned();
    let generic_params = if let Some((params, tail)) = take_angle_group(rest) {
        rest = tail.trim_start();
        parse_generic_params(params)?
    } else {
        Vec::new()
    };
    let (param_groups, mut rest) = parse_fn_param_groups(rest)?;
    let (before_where, where_part) = split_top_level_keyword_once(rest.trim_start(), "where");
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
    while rest.starts_with('(') {
        let close = find_matching_punctuation(rest, 0, '(', ')')
            .ok_or_else(|| TypeParseError::new("unclosed parameter list"))?;
        let params = split_top_level_punctuation(&rest[1..close], ',')
            .into_iter()
            .filter(|param| !param.is_empty())
            .map(parse_fn_param)
            .collect::<Result<Vec<_>, _>>()?;
        groups.push(FnParamGroup { params });
        rest = rest[close + 1..].trim_start();
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
            default: None,
        });
    }
    let (doc, source) = take_param_doc(source);
    let (pattern, ty) = split_top_level_punctuation_once(source, ':')
        .ok_or_else(|| TypeParseError::new("expected `pattern: Type` parameter"))?;
    let (ty, default) = split_top_level_punctuation_once(ty, '=')
        .map_or((ty.trim(), None), |(ty, default)| {
            (ty.trim(), parse_expr(default.trim()).ok())
        });
    Ok(FnParam {
        doc,
        pattern: parse_pattern(pattern),
        ty: parse_type_ref(ty)?,
        default,
    })
}

fn take_param_doc(source: &str) -> (Option<DocBlock>, &str) {
    let Some(prefix) = take_doc_comment_prefix(source) else {
        return (None, source.trim());
    };
    let consumed = prefix.consumed();
    let rest = source.get(consumed..).unwrap_or_default().trim();
    (
        Some(DocBlock::new(
            prefix.lines().join("\n"),
            TextRange::new(0, consumed.saturating_sub(1)),
        )),
        rest,
    )
}

fn parse_type_atom(source: &str) -> TypeRef {
    if let Ok(value) = source.parse::<usize>() {
        return TypeRef::ConstInt(value);
    }
    if matches!(source, "!" | "Never") {
        return TypeRef::Never;
    }
    if let Some(rest) = source.strip_prefix('&') {
        let rest = rest.trim_start();
        let (lifetime, inner) = if let Some((lifetime, inner)) = split_leading_lifetime(rest) {
            (Some(parse_lifetime_name(lifetime)), inner)
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
    let open = find_top_level_punctuation(source, '<')?;
    let close = find_matching_angle_group(source, open)?;
    (close == source.len().saturating_sub(1))
        .then_some((source[..open].trim(), &source[open + 1..close]))
}

fn split_type_args(source: &str) -> Vec<&str> {
    split_top_level_punctuation(source, ',')
}

fn take_angle_group(source: &str) -> Option<(&str, &str)> {
    source.strip_prefix('<')?;
    let close = find_matching_angle_group(source, 0)?;
    Some((&source[1..close], &source[close + 1..]))
}

fn parse_generic_params(source: &str) -> Result<Vec<GenericParam>, TypeParseError> {
    split_top_level_punctuation(source, ',')
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
    split_top_level_punctuation(source, ',')
        .into_iter()
        .map(|clause| {
            let (subject, bounds) = split_top_level_punctuation_once(clause, ':')
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

fn type_ref_has_whitespace_path(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Never | TypeRef::ConstInt(_) => false,
        TypeRef::Path(path) => path.chars().any(char::is_whitespace),
        TypeRef::Generic { base, args } => {
            base.chars().any(char::is_whitespace) || args.iter().any(type_ref_has_whitespace_path)
        }
        TypeRef::Ref { inner, .. } | TypeRef::Slice(inner) => type_ref_has_whitespace_path(inner),
    }
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

    pub const fn default(&self) -> Option<&Expr> {
        self.default.as_ref()
    }
}

impl TypeParseError {
    fn new(message: &str) -> Self {
        Self {
            message: message.to_owned(),
        }
    }
}
