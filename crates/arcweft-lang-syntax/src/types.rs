use thiserror::Error;

use crate::ast::common::{DocBlock, TextRange};
use crate::ast::pattern::Pattern;
use crate::cst::{
    find_matching_angle_group, find_matching_punctuation, find_top_level_matching_punctuation,
    find_top_level_punctuation, split_leading_ident, split_leading_lifetime,
    split_top_level_keyword_once, split_top_level_punctuation, split_top_level_punctuation_once,
    take_doc_comment_prefix,
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
    Choice(Vec<TypeRef>),
    Generic {
        base: String,
        args: Vec<TypeRef>,
    },
    TraitBound(TraitBound),
    Projection {
        subject: Box<TypeRef>,
        assoc: String,
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
    Type(GenericTypeParam),
}

/// Generic type parameter with inline trait bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericTypeParam {
    name: String,
    bounds: Vec<TypeRef>,
}

/// Associated type equality inside a trait bound, such as `Iterator<Item = T>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssocTypeBinding {
    name: String,
    value: TypeRef,
}

/// Trait bound syntax preserving associated type equality constraints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitBound {
    path: String,
    args: Vec<TypeRef>,
    assoc_bindings: Vec<AssocTypeBinding>,
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
    kind: FnParamKind,
    default: Option<Expr>,
}

/// Function parameter arity role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FnParamKind {
    /// A normal fixed parameter.
    Fixed,
    /// A positional rest parameter declared as `name: ...T`.
    Rest,
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
    parse_type_choice(source)
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
    validate_rest_parameters(&groups)?;
    Ok((groups, rest))
}

fn parse_fn_param(source: &str) -> Result<FnParam, TypeParseError> {
    if matches!(source.trim(), "self" | "&self" | "&mut self" | "mut self") {
        return Ok(FnParam {
            doc: None,
            pattern: Pattern::Ident("self".to_owned()),
            ty: TypeRef::Path("Self".to_owned()),
            kind: FnParamKind::Fixed,
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
    let (kind, ty) = ty
        .strip_prefix("...")
        .map_or((FnParamKind::Fixed, ty), |rest_ty| {
            (FnParamKind::Rest, rest_ty.trim_start())
        });
    if ty.is_empty() {
        return Err(TypeParseError::new(
            "expected rest parameter type after `...`",
        ));
    }
    if kind == FnParamKind::Rest && default.is_some() {
        return Err(TypeParseError::new(
            "rest parameter cannot declare a default value",
        ));
    }
    Ok(FnParam {
        doc,
        pattern: parse_pattern(pattern),
        ty: parse_type_ref(ty)?,
        kind,
        default,
    })
}

fn validate_rest_parameters(groups: &[FnParamGroup]) -> Result<(), TypeParseError> {
    let mut rest_count = 0;
    let final_group_index = groups.len().saturating_sub(1);
    for (group_index, group) in groups.iter().enumerate() {
        let final_param_index = group.params.len().saturating_sub(1);
        for (param_index, param) in group.params.iter().enumerate() {
            if param.kind != FnParamKind::Rest {
                continue;
            }
            rest_count += 1;
            if rest_count > 1 {
                return Err(TypeParseError::new(
                    "signature can declare at most one rest parameter",
                ));
            }
            if group_index != final_group_index || param_index != final_param_index {
                return Err(TypeParseError::new(
                    "rest parameter must be the last parameter of the final group",
                ));
            }
        }
    }
    Ok(())
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

fn parse_type_choice(source: &str) -> Result<TypeRef, TypeParseError> {
    let alternatives = split_top_level_punctuation(source, '|');
    if alternatives.len() <= 1 {
        return parse_type_atom(source);
    }
    let mut labels = Vec::new();
    let mut parsed = Vec::new();
    for alternative in alternatives {
        let alternative = alternative.trim();
        if alternative.is_empty() {
            return Err(TypeParseError::new(
                "anonymous sum alternative cannot be empty",
            ));
        }
        reject_variant_row_type(alternative)?;
        let ty = parse_type_atom(alternative)?;
        let label = type_ref_parse_label(&ty);
        if labels.iter().any(|existing| existing == &label) {
            return Err(TypeParseError::new(&format!(
                "duplicate alternative `{label}` in anonymous sum"
            )));
        }
        labels.push(label);
        parsed.push(ty);
    }
    Ok(TypeRef::Choice(parsed))
}

fn parse_type_atom(source: &str) -> Result<TypeRef, TypeParseError> {
    if let Some(inner) = parenthesized_type(source) {
        return parse_type_ref(inner);
    }
    if let Ok(value) = source.parse::<usize>() {
        return Ok(TypeRef::ConstInt(value));
    }
    if matches!(source, "!" | "Never") {
        return Ok(TypeRef::Never);
    }
    if let Some(rest) = source.strip_prefix('&') {
        let rest = rest.trim_start();
        let (lifetime, inner) = if let Some((lifetime, inner)) = split_leading_lifetime(rest) {
            (Some(parse_lifetime_name(lifetime)), inner)
        } else {
            (None, rest)
        };
        return Ok(TypeRef::Ref {
            lifetime,
            inner: Box::new(parse_type_ref(inner)?),
        });
    }
    if let Some(inner) = source
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return Ok(TypeRef::Slice(Box::new(parse_type_ref(inner.trim())?)));
    }
    if let Some((base, args)) = split_generic_type(source) {
        let parsed_args = split_type_args(args)
            .into_iter()
            .map(parse_type_arg)
            .collect::<Result<Vec<_>, _>>()?;
        let mut type_args = Vec::new();
        let mut assoc_bindings = Vec::new();
        for arg in parsed_args {
            match arg {
                TypeArg::Type(ty) => type_args.push(ty),
                TypeArg::Assoc(binding) => assoc_bindings.push(binding),
            }
        }
        if !assoc_bindings.is_empty() {
            return Ok(TypeRef::TraitBound(TraitBound {
                path: base.to_owned(),
                args: type_args,
                assoc_bindings,
            }));
        }
        return Ok(TypeRef::Generic {
            base: base.to_owned(),
            args: type_args,
        });
    }
    if let Some((subject, assoc)) = split_type_projection(source) {
        let assoc = assoc.trim();
        if !assoc.is_empty() && assoc.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
            return Ok(TypeRef::Projection {
                subject: Box::new(parse_type_ref(subject.trim())?),
                assoc: assoc.to_owned(),
            });
        }
    }
    Ok(TypeRef::Path(source.to_owned()))
}

enum TypeArg {
    Type(TypeRef),
    Assoc(AssocTypeBinding),
}

fn parenthesized_type(source: &str) -> Option<&str> {
    source.strip_prefix('(')?;
    let close = find_matching_punctuation(source, 0, '(', ')')?;
    (close == source.len().saturating_sub(1)).then(|| source[1..close].trim())
}

fn reject_variant_row_type(source: &str) -> Result<(), TypeParseError> {
    let Some((open, close)) = find_top_level_matching_punctuation(source, '(', ')') else {
        return Ok(());
    };
    if close != source.len().saturating_sub(1) {
        return Ok(());
    }
    let head = source[..open].trim();
    if head.chars().next().is_some_and(char::is_uppercase)
        && head.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
    {
        return Err(TypeParseError::new(
            "anonymous sum alternatives are types, not variant rows; use `A | B` or a nominal enum",
        ));
    }
    Ok(())
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

fn parse_type_arg(source: &str) -> Result<TypeArg, TypeParseError> {
    if let Some((name, value)) = split_top_level_punctuation_once(source, '=') {
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
            return Err(TypeParseError::new(
                "expected associated type name before `=`",
            ));
        }
        return Ok(TypeArg::Assoc(AssocTypeBinding {
            name: name.to_owned(),
            value: parse_type_ref(value.trim())?,
        }));
    }
    Ok(TypeArg::Type(parse_type_ref(source.trim())?))
}

fn split_type_projection(source: &str) -> Option<(&str, &str)> {
    let bytes = source.as_bytes();
    let mut angle = 0usize;
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut split = None;
    let mut index = 0usize;
    while index + 1 < bytes.len() {
        match bytes[index] as char {
            '<' if paren == 0 && bracket == 0 => angle += 1,
            '>' if angle > 0 && paren == 0 && bracket == 0 => angle -= 1,
            '(' if angle == 0 && bracket == 0 => paren += 1,
            ')' if paren > 0 && angle == 0 && bracket == 0 => paren -= 1,
            '[' if angle == 0 && paren == 0 => bracket += 1,
            ']' if bracket > 0 && angle == 0 && paren == 0 => bracket -= 1,
            ':' if bytes[index + 1] == b':' && angle == 0 && paren == 0 && bracket == 0 => {
                split = Some(index);
                index += 1;
            }
            _ => {}
        }
        index += 1;
    }
    let split = split?;
    Some((&source[..split], &source[split + 2..]))
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
                let (name, bounds) =
                    split_top_level_punctuation_once(param, ':').map_or((param, ""), |parts| parts);
                let bounds = if bounds.trim().is_empty() {
                    Vec::new()
                } else {
                    split_top_level_punctuation(bounds, '+')
                        .into_iter()
                        .map(str::trim)
                        .filter(|bound| !bound.is_empty())
                        .map(parse_type_ref)
                        .collect::<Result<Vec<_>, _>>()?
                };
                Ok(GenericParam::Type(GenericTypeParam {
                    name: name.trim().to_owned(),
                    bounds,
                }))
            }
        })
        .collect()
}

pub fn parse_where_clause_list(source: &str) -> Result<Vec<WhereClause>, TypeParseError> {
    if source.trim().is_empty() {
        return Err(TypeParseError::new("expected where clause predicate"));
    }
    split_top_level_punctuation(source, ',')
        .into_iter()
        .map(|clause| {
            let (subject, bounds) = split_top_level_punctuation_once(clause, ':')
                .ok_or_else(|| TypeParseError::new("expected `Type: Bound` where predicate"))?;
            let bounds = split_top_level_punctuation(bounds, '+')
                .into_iter()
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

fn parse_where_clauses(source: &str) -> Result<Vec<WhereClause>, TypeParseError> {
    parse_where_clause_list(source)
}

fn type_ref_has_whitespace_path(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Never | TypeRef::ConstInt(_) => false,
        TypeRef::Path(path) => path.chars().any(char::is_whitespace),
        TypeRef::Choice(alternatives) => alternatives.iter().any(type_ref_has_whitespace_path),
        TypeRef::Generic { base, args } => {
            base.chars().any(char::is_whitespace) || args.iter().any(type_ref_has_whitespace_path)
        }
        TypeRef::TraitBound(bound) => {
            bound.path.chars().any(char::is_whitespace)
                || bound.args.iter().any(type_ref_has_whitespace_path)
                || bound
                    .assoc_bindings
                    .iter()
                    .any(|binding| type_ref_has_whitespace_path(&binding.value))
        }
        TypeRef::Projection { subject, assoc } => {
            assoc.chars().any(char::is_whitespace) || type_ref_has_whitespace_path(subject)
        }
        TypeRef::Ref { inner, .. } | TypeRef::Slice(inner) => type_ref_has_whitespace_path(inner),
    }
}

fn type_ref_parse_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Never => "Never".to_owned(),
        TypeRef::ConstInt(value) => value.to_string(),
        TypeRef::Path(path) => path.clone(),
        TypeRef::Choice(alternatives) => alternatives
            .iter()
            .map(type_ref_parse_label)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeRef::Generic { base, args } => format!(
            "{base}<{}>",
            args.iter()
                .map(type_ref_parse_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::TraitBound(bound) => {
            let mut args = bound
                .args
                .iter()
                .map(type_ref_parse_label)
                .collect::<Vec<_>>();
            args.extend(bound.assoc_bindings.iter().map(|binding| {
                format!(
                    "{} = {}",
                    binding.name,
                    type_ref_parse_label(&binding.value)
                )
            }));
            format!("{}<{}>", bound.path, args.join(", "))
        }
        TypeRef::Projection { subject, assoc } => {
            format!("{}::{assoc}", type_ref_parse_label(subject))
        }
        TypeRef::Ref { lifetime, inner } => {
            let lifetime = lifetime
                .as_ref()
                .map(|lifetime| format!("'{} ", lifetime.name()))
                .unwrap_or_default();
            format!("&{lifetime}{}", type_ref_parse_label(inner))
        }
        TypeRef::Slice(inner) => format!("[{}]", type_ref_parse_label(inner)),
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
            Self::Type(param) => Some(param.name()),
            Self::Lifetime(_) => None,
        }
    }

    /// Generic parameter as a full type-parameter node, when applicable.
    pub const fn as_type_param(&self) -> Option<&GenericTypeParam> {
        match self {
            Self::Type(param) => Some(param),
            Self::Lifetime(_) => None,
        }
    }
}

impl GenericTypeParam {
    /// Generic type parameter name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Inline bounds declared on the parameter.
    pub fn bounds(&self) -> &[TypeRef] {
        &self.bounds
    }
}

impl AssocTypeBinding {
    /// Associated type name constrained by this binding.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Required associated type value.
    pub const fn value(&self) -> &TypeRef {
        &self.value
    }
}

impl TraitBound {
    /// Trait path used by this bound.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Positional type arguments supplied to the trait.
    pub fn args(&self) -> &[TypeRef] {
        &self.args
    }

    /// Associated type equalities supplied to the trait.
    pub fn assoc_bindings(&self) -> &[AssocTypeBinding] {
        &self.assoc_bindings
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

    /// Parameter arity role.
    pub const fn kind(&self) -> FnParamKind {
        self.kind
    }

    /// Whether this parameter is a positional rest parameter.
    pub const fn is_rest(&self) -> bool {
        matches!(self.kind, FnParamKind::Rest)
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
