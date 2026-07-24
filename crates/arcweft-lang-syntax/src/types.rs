use thiserror::Error;

use crate::ast::pattern::Pattern;
use crate::ast::{
    common::{DocBlock, TextRange},
    module_path::ModuleSegment,
};
use crate::cst::{
    ArcweftPunctuation, find_matching_angle_group, find_matching_punctuation, split_leading_ident,
    split_top_level_keyword_once, split_top_level_punctuation, split_top_level_punctuation_once,
    strip_prefix_arcweft_punctuation, take_doc_comment_prefix,
};
use crate::expr::{Expr, parse_expr_at};
use crate::pattern::parse_pattern_at;
use crate::reference::{BorrowKind, ReferenceType};

mod expression_path;
mod source;
mod token;

pub use self::source::{
    AuthoredTypeRef, TypePath, TypeRecoveryId, TypeRefHeadKind, TypeRefHeadSource,
    TypeRefLexemeKind, TypeRefLexemeSource, TypeRefNodePath, TypeRefNodeSource, TypeRefNodeStep,
    TypeRefSourceMap, TypeRefSourceMapError,
};
pub(crate) use self::token::{
    ParsedGenericCallee, ParsedTypeReceiver, TypeToken, TypeTokenCursor, TypeTokenKind,
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

struct ParsedTypeRef {
    value: TypeRef,
    nodes: Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
    lexemes: Vec<TypeRefLexemeSource<TextRange>>,
}

impl ParsedTypeRef {
    fn node(
        value: TypeRef,
        path: &TypeRefNodePath,
        whole: TextRange,
        head: Option<TypeRefHeadSource<TextRange>>,
        lexemes: Vec<TypeRefLexemeSource<TextRange>>,
    ) -> Self {
        Self {
            value,
            nodes: vec![(path.clone(), TypeRefNodeSource::new(whole, head))],
            lexemes,
        }
    }

    fn replace_node_whole(&mut self, path: &TypeRefNodePath, whole: TextRange) {
        let (_, source) = self
            .nodes
            .iter_mut()
            .find(|(candidate, _)| candidate == path)
            .expect("parsed wrapper retains its structural root");
        source.replace_whole(whole);
    }
}

/// Parses an Arcweft type expression together with exact source evidence.
pub fn parse_type_ref(source: &str) -> Result<AuthoredTypeRef, TypeParseError> {
    parse_authored_type_ref_at(source, 0)
}

fn parse_authored_type_ref_at(
    source: &str,
    base: usize,
) -> Result<AuthoredTypeRef, TypeParseError> {
    token::parse_source_at(source, base)
}

const MAX_TYPE_GENERIC_ARGUMENTS: usize = 256;
const MAX_TYPE_NODES: usize = 4_096;

fn validate_type_ref_limits(root: &TypeRef) -> Result<(), TypeParseError> {
    let mut pending = vec![root];
    let mut nodes = 0usize;
    while let Some(ty) = pending.pop() {
        nodes = nodes
            .checked_add(1)
            .ok_or_else(|| TypeParseError::resource_limit("type node count overflow"))?;
        if nodes > MAX_TYPE_NODES {
            return Err(TypeParseError::node_limit(
                "type exceeds the 4096 node limit",
            ));
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
                    return Err(TypeParseError::generic_argument_limit(
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
                    .ok_or_else(|| {
                        TypeParseError::resource_limit("trait argument count overflow")
                    })?;
                if argument_count > MAX_TYPE_GENERIC_ARGUMENTS {
                    return Err(TypeParseError::generic_argument_limit(
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

    /// Nominal path authored at this type's head, when this node has one.
    ///
    /// This is the structural resolver input for path, generic-constructor,
    /// and trait-bound nodes. It never reconstructs a display label.
    pub const fn nominal_path(&self) -> Option<&TypePath> {
        match self {
            Self::Path(path) | Self::Generic { base: path, .. } => Some(path),
            Self::TraitBound(bound) => Some(&bound.path),
            Self::Never
            | Self::ConstInt(_)
            | Self::Tuple(_)
            | Self::Function { .. }
            | Self::Choice(_)
            | Self::Projection { .. }
            | Self::Reference(_)
            | Self::Slice(_)
            | Self::Recovery(_) => None,
        }
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

    fn resource_limit(message: &str) -> Self {
        Self::without_range("syntax.type.resource_limit", message)
    }

    fn node_limit(message: &str) -> Self {
        Self::without_range("syntax.type.node_limit", message)
    }

    fn generic_argument_limit(message: &str) -> Self {
        Self::without_range("syntax.type.generic_argument_limit", message)
    }

    fn without_range(code: &'static str, message: &str) -> Self {
        Self {
            code,
            range: None,
            message: message.to_owned(),
        }
    }

    pub(super) fn at(code: &'static str, message: &str, range: TextRange) -> Self {
        Self {
            code,
            range: Some(range),
            message: message.to_owned(),
        }
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
