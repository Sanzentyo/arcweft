use thiserror::Error;

use crate::ast::pattern::Pattern;
use crate::ast::{
    common::{DocBlock, TextRange},
    module_path::ModuleSegment,
};
use crate::cst::{
    ArcweftPunctuation, find_matching_angle_group, find_matching_punctuation,
    find_top_level_matching_punctuation, find_top_level_punctuation, split_leading_ident,
    split_top_level_arcweft_punctuation_once, split_top_level_keyword_once,
    split_top_level_punctuation, split_top_level_punctuation_once,
    strip_prefix_arcweft_punctuation, take_doc_comment_prefix,
};
use crate::expr::{Expr, parse_expr_at};
use crate::pattern::parse_pattern_at;
use crate::reference::{BorrowKind, ReferenceType};

mod expression_path;
mod reference;
mod source;

use self::reference::parse_reference_type;
pub use self::source::{
    AuthoredTypeRef, TypePath, TypeRecoveryId, TypeRefHeadKind, TypeRefHeadSource, TypeRefNodePath,
    TypeRefNodeSource, TypeRefNodeStep, TypeRefSourceMap, TypeRefSourceMapError,
};

/// Lifetime name used in Arcweft type syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifetimeName {
    name: String,
    range: TextRange,
}

/// Type syntax preserved for later borrow and suspension-boundary checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeRef {
    Never,
    ConstInt(usize),
    Path(TypePath),
    Tuple(Vec<TypeRef>),
    Function {
        params: Vec<TypeRef>,
        return_type: Box<TypeRef>,
        effects: Option<TypeEffectRow>,
    },
    Choice(Vec<TypeRef>),
    Generic {
        base: TypePath,
        args: Vec<TypeRef>,
    },
    TraitBound(TraitBound),
    Projection {
        subject: Box<TypeRef>,
        assoc: ModuleSegment,
    },
    Reference(ReferenceType),
    Slice(Box<TypeRef>),
    Recovery(TypeRecoveryId),
}

/// Closed effect row attached to a function type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeEffectRow {
    effects: Vec<String>,
}

/// Function signature shape preserved before semantic generic resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FnSignature {
    name: String,
    generic_params: Vec<GenericParam>,
    param_groups: Vec<FnParamGroup>,
    return_type: Option<AuthoredTypeRef>,
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
    name: ModuleSegment,
    bounds: Vec<AuthoredTypeRef>,
    name_range: TextRange,
    range: TextRange,
}

/// Associated type equality inside a trait bound, such as `Iterator<Item = T>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociatedTypeBinding {
    name: ModuleSegment,
    value: TypeRef,
}

/// Trait bound syntax preserving associated type equality constraints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitBound {
    path: TypePath,
    args: Vec<TypeRef>,
    associated: Vec<AssociatedTypeBinding>,
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
    ty: Option<AuthoredTypeRef>,
    kind: FnParamKind,
    default: Option<Expr>,
    receiver_kind: Option<FnReceiverKind>,
}

/// Function parameter arity role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FnParamKind {
    /// A normal fixed parameter.
    Fixed,
    /// A positional rest parameter declared as `name: ...T`.
    Rest,
}

/// Receiver ownership mode preserved from source syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FnReceiverKind {
    Owned,
    SharedRef,
    MutRef,
}

impl FnReceiverKind {
    /// Returns the shared or mutable borrow permission for reference receivers.
    pub const fn borrow_kind(self) -> Option<BorrowKind> {
        match self {
            Self::Owned => None,
            Self::SharedRef => Some(BorrowKind::Shared),
            Self::MutRef => Some(BorrowKind::Mutable),
        }
    }
}

/// One `where` clause predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhereClause {
    subject: AuthoredTypeRef,
    bounds: Vec<AuthoredTypeRef>,
    range: TextRange,
}

/// Type syntax parse failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeParseError {
    code: &'static str,
    range: Option<TextRange>,
    message: String,
}

/// Parses an Arcweft type expression together with exact source evidence.
pub fn parse_type_ref(source: &str) -> Result<AuthoredTypeRef, TypeParseError> {
    let parsed = parse_type_ref_value(source)?;
    validate_type_ref_limits(&parsed)?;
    let source_map = source::build_type_source_map(source, &parsed)?;
    AuthoredTypeRef::try_new(parsed, source_map).map_err(|error| {
        TypeParseError::new_owned(format!("invalid parser-owned type source map: {error:?}"))
    })
}

fn parse_type_ref_value(source: &str) -> Result<TypeRef, TypeParseError> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(TypeParseError::new("expected type"));
    }
    let mut parsed = match parse_single_argument_generic_chain(trimmed)? {
        Some(parsed) => parsed,
        None => parse_function_type(trimmed)?,
    };
    parsed.rebase_reference_ranges(subslice_offset(source, trimmed));
    Ok(parsed)
}

/// Parses the common unary-constructor chain without one Rust stack frame per
/// type layer. The production nominal resolver accepts a recursive type depth
/// of 256, so syntax parsing must be able to construct at least that depth
/// before the semantic limit can make the acceptance decision.
fn parse_single_argument_generic_chain(source: &str) -> Result<Option<TypeRef>, TypeParseError> {
    let mut fragment = source;
    let mut fragment_base = 0usize;
    let mut layers = Vec::new();

    while let Some((base, arguments)) = split_generic_type(fragment) {
        let parts = split_type_args(arguments);
        if parts.len() != 1 {
            break;
        }
        let argument = parts[0].trim();
        if argument.is_empty() || split_top_level_punctuation_once(argument, '=').is_some() {
            break;
        }
        layers.push(base);
        fragment_base = fragment_base
            .checked_add(subslice_offset(fragment, argument))
            .ok_or_else(|| TypeParseError::new("type source offset overflow"))?;
        fragment = argument;
    }

    if layers.is_empty() {
        return Ok(None);
    }

    let mut parsed = parse_function_type(fragment)?;
    parsed.rebase_reference_ranges(fragment_base);
    for base in layers.into_iter().rev() {
        parsed = TypeRef::Generic {
            base: TypePath::parse(base).map_err(|error| {
                TypeParseError::new_owned(format!("invalid type constructor `{base}`: {error}"))
            })?,
            args: vec![parsed],
        };
    }
    Ok(Some(parsed))
}

const MAX_TYPE_GENERIC_ARGUMENTS: usize = 256;
const MAX_TYPE_NODES: usize = 4_096;

fn validate_type_ref_limits(root: &TypeRef) -> Result<(), TypeParseError> {
    let mut pending = vec![root];
    let mut nodes = 0usize;
    while let Some(ty) = pending.pop() {
        nodes = nodes
            .checked_add(1)
            .ok_or_else(|| TypeParseError::new("type node count overflow"))?;
        if nodes > MAX_TYPE_NODES {
            return Err(TypeParseError::new("type exceeds the 4096 node limit"));
        }
        match ty {
            TypeRef::Tuple(items) | TypeRef::Choice(items) => pending.extend(items.iter().rev()),
            TypeRef::Function {
                params,
                return_type,
                ..
            } => {
                pending.push(return_type);
                pending.extend(params.iter().rev());
            }
            TypeRef::Generic { args, .. } => {
                if args.len() > MAX_TYPE_GENERIC_ARGUMENTS {
                    return Err(TypeParseError::new(
                        "type constructor exceeds the 256 argument limit",
                    ));
                }
                pending.extend(args.iter().rev());
            }
            TypeRef::TraitBound(bound) => {
                let argument_count = bound
                    .args
                    .len()
                    .checked_add(bound.associated.len())
                    .ok_or_else(|| TypeParseError::new("trait argument count overflow"))?;
                if argument_count > MAX_TYPE_GENERIC_ARGUMENTS {
                    return Err(TypeParseError::new(
                        "trait bound exceeds the 256 argument limit",
                    ));
                }
                pending.extend(bound.associated.iter().rev().map(|binding| &binding.value));
                pending.extend(bound.args.iter().rev());
            }
            TypeRef::Projection { subject, .. } | TypeRef::Slice(subject) => {
                pending.push(subject);
            }
            TypeRef::Reference(reference) => pending.push(reference.referent()),
            TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Path(_) | TypeRef::Recovery(_) => {}
        }
    }
    Ok(())
}

impl TypeRef {
    /// Deterministic Arcweft spelling used by typed semantic identities.
    pub fn canonical_label(&self) -> String {
        type_ref_parse_label(self)
    }

    pub(crate) fn rebase_reference_ranges(&mut self, base: usize) {
        match self {
            Self::Tuple(items) | Self::Choice(items) => {
                for item in items {
                    item.rebase_reference_ranges(base);
                }
            }
            Self::Function {
                params,
                return_type,
                ..
            } => {
                for param in params {
                    param.rebase_reference_ranges(base);
                }
                return_type.rebase_reference_ranges(base);
            }
            Self::Generic { args, .. } => {
                for arg in args {
                    arg.rebase_reference_ranges(base);
                }
            }
            Self::TraitBound(bound) => {
                for arg in &mut bound.args {
                    arg.rebase_reference_ranges(base);
                }
                for binding in &mut bound.associated {
                    binding.value.rebase_reference_ranges(base);
                }
            }
            Self::Projection { subject, .. } | Self::Slice(subject) => {
                subject.rebase_reference_ranges(base);
            }
            Self::Reference(reference) => reference.rebase(base),
            Self::Never | Self::ConstInt(_) | Self::Path(_) | Self::Recovery(_) => {}
        }
    }
}

fn subslice_offset(source: &str, fragment: &str) -> usize {
    (fragment.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
}

/// Parses the head of a function signature, including generics and curried parameter groups.
pub fn parse_fn_signature(source: &str) -> Result<FnSignature, TypeParseError> {
    parse_fn_signature_at(source, 0)
}

pub(crate) fn parse_fn_signature_at(
    source: &str,
    base: usize,
) -> Result<FnSignature, TypeParseError> {
    let trimmed = source.trim();
    let base = base + subslice_offset(source, trimmed);
    let source = trimmed;
    let after_fn = source
        .strip_prefix("fn ")
        .ok_or_else(|| TypeParseError::new("expected `fn` signature"))?;
    let (name, mut rest) = split_leading_ident(after_fn)
        .ok_or_else(|| TypeParseError::new("expected function name"))?;
    let name = name.to_owned();
    let generic_params = if let Some((params, tail)) = take_angle_group(rest) {
        rest = tail.trim_start();
        parse_generic_params_at(params, base + subslice_offset(source, params))?
    } else {
        Vec::new()
    };
    let (param_groups, mut rest) =
        parse_fn_param_groups(rest, base + subslice_offset(source, rest))?;
    let (before_where, where_part) = split_top_level_keyword_once(rest.trim_start(), "where");
    rest = before_where.trim_start();
    let return_type =
        if let Some(tail) = strip_prefix_arcweft_punctuation(rest, ArcweftPunctuation::ThinArrow) {
            let ty = tail.trim();
            if ty.is_empty() {
                return Err(TypeParseError::new("expected return type after `->`"));
            }
            let mut parsed = parse_type_ref(ty)?;
            if type_ref_has_whitespace_path(parsed.value()) {
                return Err(TypeParseError::new("unexpected tokens after return type"));
            }
            parsed.rebase(base + subslice_offset(source, ty));
            Some(parsed)
        } else if rest.is_empty() {
            None
        } else {
            return Err(TypeParseError::new(
                "unexpected tokens after parameter list",
            ));
        };
    let where_clauses = where_part.map_or_else(
        || Ok(Vec::new()),
        |where_source| {
            parse_where_clauses_at(where_source, base + subslice_offset(source, where_source))
        },
    )?;
    Ok(FnSignature {
        name,
        generic_params,
        param_groups,
        return_type,
        where_clauses,
    })
}

fn parse_fn_param_groups(
    source: &str,
    base: usize,
) -> Result<(Vec<FnParamGroup>, &str), TypeParseError> {
    let mut rest = source.trim_start();
    let mut groups = Vec::new();
    while rest.starts_with('(') {
        let close = find_matching_punctuation(rest, 0, '(', ')')
            .ok_or_else(|| TypeParseError::new("unclosed parameter list"))?;
        let param_source = &rest[1..close];
        let param_source_base = base + subslice_offset(source, param_source);
        let params = split_top_level_punctuation(param_source, ',')
            .into_iter()
            .filter(|param| !param.is_empty())
            .map(|param| {
                parse_fn_param(
                    param,
                    param_source_base + subslice_offset(param_source, param),
                )
            })
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

fn parse_fn_param(source: &str, base: usize) -> Result<FnParam, TypeParseError> {
    let trimmed = source.trim();
    let base = base + subslice_offset(source, trimmed);
    if let Some(receiver_kind) = receiver_kind(trimmed) {
        return Ok(FnParam {
            doc: None,
            pattern: Pattern::Ident("self".to_owned()),
            ty: None,
            kind: FnParamKind::Fixed,
            default: None,
            receiver_kind: Some(receiver_kind),
        });
    }
    let (doc, source) = take_param_doc(trimmed);
    let (pattern, ty) = split_top_level_punctuation_once(source, ':')
        .ok_or_else(|| TypeParseError::new("expected `pattern: Type` parameter"))?;
    let (ty, default) = if let Some((ty, default)) = split_top_level_punctuation_once(ty, '=') {
        let default = default.trim();
        if default.is_empty() {
            return Err(TypeParseError::new(
                "function parameter default requires an expression",
            ));
        }
        let default_base = base + subslice_offset(trimmed, default);
        let default = parse_expr_at(default, default_base).map_err(|error| {
            TypeParseError::new_owned(format!("invalid function parameter default: {error}"))
        })?;
        (ty.trim(), Some(default))
    } else {
        (ty.trim(), None)
    };
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
    let pattern_source = pattern.trim();
    let pattern = parse_pattern_at(
        pattern_source,
        base + subslice_offset(trimmed, pattern_source),
    );
    let type_source = ty;
    let mut ty = parse_type_ref(type_source)?;
    ty.rebase(base + subslice_offset(trimmed, type_source));
    Ok(FnParam {
        doc,
        pattern,
        ty: Some(ty),
        kind,
        default,
        receiver_kind: None,
    })
}

fn receiver_kind(source: &str) -> Option<FnReceiverKind> {
    Some(match source {
        "self" | "mut self" => FnReceiverKind::Owned,
        "&self" => FnReceiverKind::SharedRef,
        "&mut self" => FnReceiverKind::MutRef,
        _ => return None,
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

fn parse_function_type(source: &str) -> Result<TypeRef, TypeParseError> {
    let (function_source, effects) = split_type_effect_row_suffix(source)?;
    if let Some((params, return_type)) = split_top_level_arrow(function_source) {
        let params_source = params.trim();
        let mut params = parse_function_type_params(params_source)?;
        let params_base = subslice_offset(source, params_source);
        for param in &mut params {
            param.rebase_reference_ranges(params_base);
        }
        let return_source = return_type.trim();
        if return_source.is_empty() {
            return Err(TypeParseError::new("expected return type after `->`"));
        }
        let mut return_type = parse_function_type(return_source)?;
        return_type.rebase_reference_ranges(subslice_offset(source, return_source));
        return Ok(TypeRef::Function {
            params,
            return_type: Box::new(return_type),
            effects,
        });
    }
    if let Some(effects) = effects {
        let Some(inner) = parenthesized_type(function_source) else {
            return Err(TypeParseError::new(
                "effect row can only annotate a function type",
            ));
        };
        let mut inner_ty = parse_function_type(inner)?;
        inner_ty.rebase_reference_ranges(subslice_offset(source, inner));
        let TypeRef::Function {
            params,
            return_type,
            effects: inner_effects,
        } = inner_ty
        else {
            return Err(TypeParseError::new(
                "effect row can only annotate a function type",
            ));
        };
        if inner_effects.is_some() {
            return Err(TypeParseError::new(
                "function type cannot declare multiple effect rows",
            ));
        }
        return Ok(TypeRef::Function {
            params,
            return_type,
            effects: Some(effects),
        });
    }
    parse_type_choice(source)
}

fn parse_function_type_params(source: &str) -> Result<Vec<TypeRef>, TypeParseError> {
    let mut params = if let Some(inner) = parenthesized_type(source) {
        let parts = split_top_level_punctuation(inner, ',');
        if parts.len() > 1 {
            parts
                .into_iter()
                .map(|part| parse_nested_type(inner, part))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![parse_nested_type(inner, inner)?]
        }
    } else {
        vec![parse_type_choice(source)?]
    };
    if let Some(inner) = parenthesized_type(source) {
        let inner_base = subslice_offset(source, inner);
        for param in &mut params {
            param.rebase_reference_ranges(inner_base);
        }
    }
    if params
        .iter()
        .any(|param| matches!(param, TypeRef::Tuple(_)))
    {
        return Err(TypeParseError::new(
            "function parameter group cannot contain an anonymous tuple type; use `(A, B) -> C` for one call group",
        ));
    }
    Ok(params)
}

fn parse_type_choice(source: &str) -> Result<TypeRef, TypeParseError> {
    let alternatives = split_top_level_punctuation(source, '|');
    if alternatives.len() <= 1 {
        return parse_type_atom(source);
    }
    let mut parsed = Vec::new();
    for alternative in alternatives {
        let alternative = alternative.trim();
        if alternative.is_empty() {
            return Err(TypeParseError::new(
                "anonymous sum alternative cannot be empty",
            ));
        }
        reject_variant_row_type(alternative)?;
        let mut ty = parse_type_atom(alternative)?;
        ty.rebase_reference_ranges(subslice_offset(source, alternative));
        parsed.push(ty);
    }
    Ok(TypeRef::Choice(parsed))
}

fn parse_type_atom(source: &str) -> Result<TypeRef, TypeParseError> {
    if let Some(inner) = parenthesized_type(source) {
        let parts = split_top_level_punctuation(inner, ',');
        if parts.len() > 1 {
            let mut tuple = parts
                .into_iter()
                .map(|part| parse_nested_type(inner, part))
                .collect::<Result<Vec<_>, _>>()
                .map(TypeRef::Tuple)?;
            tuple.rebase_reference_ranges(subslice_offset(source, inner));
            return Ok(tuple);
        }
        return parse_nested_type(source, inner);
    }
    if let Ok(value) = source.parse::<usize>() {
        return Ok(TypeRef::ConstInt(value));
    }
    if matches!(source, "!" | "Never") {
        return Ok(TypeRef::Never);
    }
    if source.starts_with('&') {
        return parse_reference_type(source).map(TypeRef::Reference);
    }
    if let Some(inner) = source
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return Ok(TypeRef::Slice(Box::new(parse_nested_type(source, inner)?)));
    }
    if let Some((base, args)) = split_generic_type(source) {
        let parsed_args = split_type_args(args)
            .into_iter()
            .map(|arg| {
                let mut parsed = parse_type_arg(arg)?;
                parsed.rebase_reference_ranges(subslice_offset(args, arg));
                Ok(parsed)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut type_args = Vec::new();
        let mut associated = Vec::new();
        for arg in parsed_args {
            match arg {
                TypeArg::Type(ty) => type_args.push(ty),
                TypeArg::Assoc(binding) => associated.push(binding),
            }
        }
        if !associated.is_empty() {
            let mut result = TypeRef::TraitBound(TraitBound {
                path: TypePath::parse(base).map_err(|error| {
                    TypeParseError::new_owned(format!("invalid trait path `{base}`: {error}"))
                })?,
                args: type_args,
                associated,
            });
            result.rebase_reference_ranges(subslice_offset(source, args));
            return Ok(result);
        }
        let mut result = TypeRef::Generic {
            base: TypePath::parse(base).map_err(|error| {
                TypeParseError::new_owned(format!("invalid type constructor `{base}`: {error}"))
            })?,
            args: type_args,
        };
        result.rebase_reference_ranges(subslice_offset(source, args));
        return Ok(result);
    }
    if let Some((subject, assoc)) = split_type_projection(source) {
        let assoc = assoc.trim();
        if !assoc.is_empty() && assoc.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
            return Ok(TypeRef::Projection {
                subject: Box::new(parse_nested_type(source, subject)?),
                assoc: ModuleSegment::new(assoc.to_owned()).map_err(|error| {
                    TypeParseError::new_owned(format!(
                        "invalid associated type name `{assoc}`: {error}"
                    ))
                })?,
            });
        }
    }
    TypePath::parse(source).map(TypeRef::Path).map_err(|error| {
        TypeParseError::new_owned(format!("invalid type path `{source}`: {error}"))
    })
}

fn split_type_effect_row_suffix(
    source: &str,
) -> Result<(&str, Option<TypeEffectRow>), TypeParseError> {
    let (before_effects, effects) = split_top_level_keyword_once(source, "effects");
    let Some(effects) = effects else {
        return Ok((source, None));
    };
    let effects = effects.trim();
    if before_effects.trim().is_empty() && !effects.starts_with('{') {
        return Ok((source, None));
    }
    let Some(close) = effects
        .starts_with('{')
        .then(|| find_matching_punctuation(effects, 0, '{', '}'))
        .flatten()
    else {
        return Err(TypeParseError::new(
            "expected `{ ... }` after function type `effects`",
        ));
    };
    if !effects[close + 1..].trim().is_empty() {
        return Err(TypeParseError::new(
            "unexpected tokens after function type effect row",
        ));
    }
    let labels = split_top_level_punctuation(&effects[1..close], ',')
        .into_iter()
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    Ok((before_effects.trim_end(), Some(TypeEffectRow::new(labels))))
}

enum TypeArg {
    Type(TypeRef),
    Assoc(AssociatedTypeBinding),
}

impl TypeArg {
    fn rebase_reference_ranges(&mut self, base: usize) {
        match self {
            Self::Type(ty) => ty.rebase_reference_ranges(base),
            Self::Assoc(binding) => binding.value.rebase_reference_ranges(base),
        }
    }
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
        return Ok(TypeArg::Assoc(AssociatedTypeBinding {
            name: ModuleSegment::new(name.to_owned()).map_err(|error| {
                TypeParseError::new_owned(format!("invalid associated type name `{name}`: {error}"))
            })?,
            value: parse_nested_type(source, value)?,
        }));
    }
    Ok(TypeArg::Type(parse_nested_type(source, source)?))
}

fn parse_nested_type(parent: &str, fragment: &str) -> Result<TypeRef, TypeParseError> {
    let fragment = fragment.trim();
    let mut parsed = parse_type_ref_value(fragment)?;
    parsed.rebase_reference_ranges(subslice_offset(parent, fragment));
    Ok(parsed)
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

fn split_top_level_arrow(source: &str) -> Option<(&str, &str)> {
    split_top_level_arcweft_punctuation_once(source, ArcweftPunctuation::ThinArrow)
}

fn take_angle_group(source: &str) -> Option<(&str, &str)> {
    source.strip_prefix('<')?;
    let close = find_matching_angle_group(source, 0)?;
    Some((&source[1..close], &source[close + 1..]))
}

/// Parses the comma-separated contents of one generic parameter list.
///
/// The surrounding `<` and `>` are owned by the declaration parser. Keeping
/// this parser shared ensures nominal declarations and function signatures
/// retain the same typed generic-parameter model.
pub(crate) fn parse_generic_params_at(
    source: &str,
    base: usize,
) -> Result<Vec<GenericParam>, TypeParseError> {
    split_top_level_punctuation(source, ',')
        .into_iter()
        .map(|param| {
            let trimmed = param.trim();
            let param_base = base + subslice_offset(source, trimmed);
            let param = trimmed;
            if param.is_empty() {
                return Err(TypeParseError::new("empty generic parameter"));
            }
            if param.starts_with('\'') {
                Ok(GenericParam::Lifetime(parse_lifetime_name(
                    param,
                    TextRange::new(param_base, param_base + param.len()),
                )))
            } else {
                let (name, bounds) =
                    split_top_level_punctuation_once(param, ':').map_or((param, ""), |parts| parts);
                let name = name.trim();
                let name_range = TextRange::new(
                    param_base + subslice_offset(param, name),
                    param_base + subslice_offset(param, name) + name.len(),
                );
                let name = ModuleSegment::new(name.to_owned()).map_err(|error| {
                    TypeParseError::new_owned(format!(
                        "invalid generic type parameter name `{name}`: {error}"
                    ))
                })?;
                let bounds = if bounds.trim().is_empty() {
                    Vec::new()
                } else {
                    split_top_level_punctuation(bounds, '+')
                        .into_iter()
                        .map(str::trim)
                        .filter(|bound| !bound.is_empty())
                        .map(|bound| {
                            let mut parsed = parse_type_ref(bound)?;
                            parsed.rebase(param_base + subslice_offset(param, bound));
                            Ok(parsed)
                        })
                        .collect::<Result<Vec<_>, _>>()?
                };
                Ok(GenericParam::Type(GenericTypeParam {
                    name,
                    bounds,
                    name_range,
                    range: TextRange::new(param_base, param_base + param.len()),
                }))
            }
        })
        .collect()
}

pub fn parse_where_clause_list(source: &str) -> Result<Vec<WhereClause>, TypeParseError> {
    parse_where_clauses_at(source, 0)
}

pub(crate) fn parse_where_clauses_at(
    source: &str,
    base: usize,
) -> Result<Vec<WhereClause>, TypeParseError> {
    if source.trim().is_empty() {
        return Err(TypeParseError::new("expected where clause predicate"));
    }
    split_top_level_punctuation(source, ',')
        .into_iter()
        .map(|clause| {
            let clause = clause.trim();
            let clause_base = base + subslice_offset(source, clause);
            let (subject, bounds) = split_top_level_punctuation_once(clause, ':')
                .ok_or_else(|| TypeParseError::new("expected `Type: Bound` where predicate"))?;
            let subject_source = subject.trim();
            let mut subject = parse_type_ref(subject_source)?;
            subject.rebase(clause_base + subslice_offset(clause, subject_source));
            let bounds = split_top_level_punctuation(bounds, '+')
                .into_iter()
                .map(str::trim)
                .filter(|bound| !bound.is_empty())
                .map(|bound| {
                    let mut parsed = parse_type_ref(bound)?;
                    parsed.rebase(clause_base + subslice_offset(clause, bound));
                    Ok(parsed)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if bounds.is_empty() {
                return Err(TypeParseError::new("expected where clause bound"));
            }
            Ok(WhereClause {
                subject,
                bounds,
                range: TextRange::new(clause_base, clause_base + clause.len()),
            })
        })
        .collect()
}

fn type_ref_has_whitespace_path(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Never | TypeRef::ConstInt(_) | TypeRef::Recovery(_) | TypeRef::Path(_) => false,
        TypeRef::Tuple(items) => items.iter().any(type_ref_has_whitespace_path),
        TypeRef::Function {
            params,
            return_type,
            effects,
        } => {
            params.iter().any(type_ref_has_whitespace_path)
                || type_ref_has_whitespace_path(return_type)
                || effects.as_ref().is_some_and(|effects| {
                    effects
                        .effects()
                        .iter()
                        .any(|effect| effect.chars().any(char::is_whitespace))
                })
        }
        TypeRef::Choice(alternatives) => alternatives.iter().any(type_ref_has_whitespace_path),
        TypeRef::Generic { args, .. } => args.iter().any(type_ref_has_whitespace_path),
        TypeRef::TraitBound(bound) => {
            bound.args.iter().any(type_ref_has_whitespace_path)
                || bound
                    .associated
                    .iter()
                    .any(|binding| type_ref_has_whitespace_path(&binding.value))
        }
        TypeRef::Projection { subject, .. } => type_ref_has_whitespace_path(subject),
        TypeRef::Reference(reference) => type_ref_has_whitespace_path(reference.referent()),
        TypeRef::Slice(inner) => type_ref_has_whitespace_path(inner),
    }
}

fn type_ref_parse_label(ty: &TypeRef) -> String {
    type_ref_label_in(ty, TypeLabelContext::TopLevel)
}

#[derive(Clone, Copy)]
enum TypeLabelContext {
    TopLevel,
    FunctionParameter,
    FunctionReturn,
    ChoiceAlternative,
    ReferenceReferent,
    ProjectionSubject,
    Delimited,
}

fn type_ref_label_in(ty: &TypeRef, context: TypeLabelContext) -> String {
    let label = type_ref_unparenthesized_label(ty);
    if type_ref_label_needs_parentheses(ty, context) {
        format!("({label})")
    } else {
        label
    }
}

fn type_ref_label_needs_parentheses(ty: &TypeRef, context: TypeLabelContext) -> bool {
    match context {
        TypeLabelContext::TopLevel | TypeLabelContext::Delimited => false,
        TypeLabelContext::FunctionParameter => matches!(ty, TypeRef::Function { .. }),
        TypeLabelContext::FunctionReturn => {
            matches!(
                ty,
                TypeRef::Function {
                    effects: Some(_),
                    ..
                }
            )
        }
        TypeLabelContext::ChoiceAlternative
        | TypeLabelContext::ReferenceReferent
        | TypeLabelContext::ProjectionSubject => {
            matches!(ty, TypeRef::Function { .. } | TypeRef::Choice(_))
        }
    }
}

fn type_ref_unparenthesized_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Never => "Never".to_owned(),
        TypeRef::ConstInt(value) => value.to_string(),
        TypeRef::Path(path) => path.canonical_string(),
        TypeRef::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(|item| type_ref_label_in(item, TypeLabelContext::Delimited))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::Function {
            params,
            return_type,
            effects,
        } => {
            let params = if params.len() == 1 {
                type_ref_label_in(&params[0], TypeLabelContext::FunctionParameter)
            } else {
                format!(
                    "({})",
                    params
                        .iter()
                        .map(|param| type_ref_label_in(param, TypeLabelContext::Delimited))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let label = format!(
                "{params} -> {}",
                type_ref_label_in(return_type, TypeLabelContext::FunctionReturn)
            );
            type_effect_row_label(effects.as_ref()).map_or(label.clone(), |effects| {
                format!("{label} effects {effects}")
            })
        }
        TypeRef::Choice(alternatives) => alternatives
            .iter()
            .map(|alternative| type_ref_label_in(alternative, TypeLabelContext::ChoiceAlternative))
            .collect::<Vec<_>>()
            .join(" | "),
        TypeRef::Generic { base, args } => format!(
            "{base}<{}>",
            args.iter()
                .map(|arg| type_ref_label_in(arg, TypeLabelContext::Delimited))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::TraitBound(bound) => {
            let mut args = bound
                .args
                .iter()
                .map(|arg| type_ref_label_in(arg, TypeLabelContext::Delimited))
                .collect::<Vec<_>>();
            args.extend(bound.associated.iter().map(|binding| {
                format!(
                    "{} = {}",
                    binding.name,
                    type_ref_label_in(&binding.value, TypeLabelContext::Delimited)
                )
            }));
            format!("{}<{}>", bound.path, args.join(", "))
        }
        TypeRef::Projection { subject, assoc } => {
            format!(
                "{}::{assoc}",
                type_ref_label_in(subject, TypeLabelContext::ProjectionSubject)
            )
        }
        TypeRef::Reference(reference) => {
            let lifetime = reference
                .region()
                .name()
                .map(|lifetime| format!("'{} ", lifetime.name()))
                .unwrap_or_default();
            format!(
                "&{lifetime}{}{}",
                reference.kind().source_qualifier(),
                type_ref_label_in(reference.referent(), TypeLabelContext::ReferenceReferent)
            )
        }
        TypeRef::Slice(inner) => format!(
            "[{}]",
            type_ref_label_in(inner, TypeLabelContext::Delimited)
        ),
        TypeRef::Recovery(id) => format!("<recovered-type:{}>", id.index()),
    }
}

fn type_effect_row_label(effects: Option<&TypeEffectRow>) -> Option<String> {
    effects.map(|effects| {
        if effects.effects().is_empty() {
            "{ }".to_owned()
        } else {
            format!("{{ {} }}", effects.effects().join(", "))
        }
    })
}

fn parse_lifetime_name(source: &str, range: TextRange) -> LifetimeName {
    LifetimeName {
        name: source.trim_start_matches('\'').to_owned(),
        range,
    }
}

impl LifetimeName {
    /// Lifetime name without the leading apostrophe.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Exact source range including the leading apostrophe.
    pub const fn range(&self) -> TextRange {
        self.range
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
    pub const fn return_type(&self) -> Option<&AuthoredTypeRef> {
        self.return_type.as_ref()
    }

    /// Where-clause predicates.
    pub fn where_clauses(&self) -> &[WhereClause] {
        &self.where_clauses
    }
}

impl TypeEffectRow {
    fn new(effects: Vec<String>) -> Self {
        Self { effects }
    }

    /// Source labels declared in this closed effect row.
    pub fn effects(&self) -> &[String] {
        &self.effects
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
    pub const fn as_type(&self) -> Option<&ModuleSegment> {
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
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    /// Inline bounds declared on the parameter.
    pub fn bounds(&self) -> &[AuthoredTypeRef] {
        &self.bounds
    }

    /// Exact source range of the parameter name.
    pub const fn name_range(&self) -> TextRange {
        self.name_range
    }

    /// Exact source range of the complete generic parameter.
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl AssociatedTypeBinding {
    /// Associated type name constrained by this binding.
    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    /// Required associated type value.
    pub const fn value(&self) -> &TypeRef {
        &self.value
    }
}

impl TraitBound {
    /// Trait path used by this bound.
    pub const fn path(&self) -> &TypePath {
        &self.path
    }

    /// Positional type arguments supplied to the trait.
    pub fn args(&self) -> &[TypeRef] {
        &self.args
    }

    /// Associated type equalities supplied to the trait.
    pub fn associated(&self) -> &[AssociatedTypeBinding] {
        &self.associated
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
    pub const fn subject(&self) -> &AuthoredTypeRef {
        &self.subject
    }

    /// Bounds that the subject must satisfy.
    pub fn bounds(&self) -> &[AuthoredTypeRef] {
        &self.bounds
    }

    /// Exact source range of the complete predicate.
    pub const fn range(&self) -> TextRange {
        self.range
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
    pub const fn ty(&self) -> Option<&AuthoredTypeRef> {
        self.ty.as_ref()
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

    /// Receiver ownership mode when this parameter is source `self`.
    pub const fn receiver_kind(&self) -> Option<FnReceiverKind> {
        self.receiver_kind
    }
}

impl TypeParseError {
    fn new(message: &str) -> Self {
        Self {
            code: "syntax.type.invalid",
            range: None,
            message: message.to_owned(),
        }
    }

    fn new_owned(message: String) -> Self {
        Self {
            code: "syntax.type.invalid",
            range: None,
            message,
        }
    }

    pub(super) fn at(code: &'static str, message: &str, range: TextRange) -> Self {
        Self {
            code,
            range: Some(range),
            message: message.to_owned(),
        }
    }

    pub(super) fn rebased(mut self, base: usize) -> Self {
        if let Some(range) = self.range {
            self.range = Some(TextRange::new(range.start() + base, range.end() + base));
        }
        self
    }

    /// Stable parser diagnostic code.
    pub const fn code(&self) -> &'static str {
        self.code
    }

    /// Type-fragment-relative error range, when exact.
    pub const fn range(&self) -> Option<TextRange> {
        self.range
    }
}
