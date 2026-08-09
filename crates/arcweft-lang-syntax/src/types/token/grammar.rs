//! Semantic type construction over one typed token transaction.

use super::scan::{
    Delimiter, first_top_level, index_u16, last_top_level, matching_close, require_nonempty,
    split_top_level, split_top_level_with_separators, token_range,
};
use super::{TypeToken, TypeTokenKind};
use crate::ast::{common::TextRange, module_path::ModuleSegment};
use crate::reference::{BorrowKind, ReferenceType, RegionSyntax};
use crate::types::source::{ParsedTypePath, TypePathComponent};
use crate::types::{
    AssociatedTypeBinding, AuthoredTypeRef, ParsedTypeRef, TraitBound, TypeEffectRow,
    TypeParseError, TypePath, TypeRef, TypeRefAssociatedBindingPart, TypeRefComponentRole,
    TypeRefComponentSource, TypeRefHeadKind, TypeRefHeadSource, TypeRefLexemeKind,
    TypeRefLexemeSource, TypeRefNodePath, TypeRefNodeSource, TypeRefNodeStep, TypeRefRegionPart,
    parse_lifetime_name, validate_type_ref_limits,
};

struct ParsedGenericArguments {
    is_trait: bool,
    type_args: Vec<TypeRef>,
    associated: Vec<AssociatedTypeBinding>,
    nodes: Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
    lexemes: Vec<TypeRefLexemeSource<TextRange>>,
}

struct UnaryGenericLayer {
    start: usize,
    end: usize,
    open: usize,
    close: usize,
    turbofish: Option<usize>,
    base_end: usize,
    argument: (usize, usize, Option<usize>),
    path: TypeRefNodePath,
}

#[cfg(test)]
#[path = "grammar_tests.rs"]
mod tests;

pub(super) fn parse_authored(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
) -> Result<AuthoredTypeRef, TypeParseError> {
    let mut parsed = parse_type(tokens, start, end, &TypeRefNodePath::root())?;
    validate_type_ref_limits(&parsed.value)?;
    parsed
        .lexemes
        .sort_by_key(|lexeme| (lexeme.range().start(), lexeme.range().end()));
    let components =
        collect_type_components(tokens, &parsed.value, &parsed.nodes, &parsed.lexemes)?;
    AuthoredTypeRef::try_new(parsed.value, parsed.nodes, parsed.lexemes, components).map_err(
        |error| {
            TypeParseError::new_owned(format!("invalid parser-owned type source map: {error:?}"))
        },
    )
}

fn collect_type_components(
    tokens: &[TypeToken<'_>],
    value: &TypeRef,
    nodes: &[(TypeRefNodePath, TypeRefNodeSource<TextRange>)],
    lexemes: &[TypeRefLexemeSource<TextRange>],
) -> Result<Vec<TypeRefComponentSource<TextRange>>, TypeParseError> {
    let sources = nodes
        .iter()
        .map(|(path, source)| (path.clone(), *source.whole()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut output = Vec::new();
    let mut pending = vec![(TypeRefNodePath::root(), value)];
    while let Some((path, value)) = pending.pop() {
        collect_node_components(
            tokens,
            value,
            &path,
            &sources,
            lexemes,
            &mut output,
            &mut pending,
        )?;
    }
    Ok(output)
}

#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive semantic type-family source projection is one closed match"
)]
fn collect_node_components<'a>(
    tokens: &[TypeToken<'_>],
    value: &'a TypeRef,
    path: &TypeRefNodePath,
    sources: &std::collections::BTreeMap<TypeRefNodePath, TextRange>,
    lexemes: &[TypeRefLexemeSource<TextRange>],
    output: &mut Vec<TypeRefComponentSource<TextRange>>,
    pending: &mut Vec<(TypeRefNodePath, &'a TypeRef)>,
) -> Result<(), TypeParseError> {
    let whole = *sources
        .get(path)
        .ok_or_else(|| TypeParseError::new("semantic type node has no source owner"))?;
    component(output, path, TypeRefComponentRole::Whole, whole);

    match value {
        TypeRef::Never => component(output, path, TypeRefComponentRole::NeverMarker, whole),
        TypeRef::ConstInt(_) => {
            component(output, path, TypeRefComponentRole::ConstInteger, whole);
        }
        TypeRef::Path(_) => collect_path_components(path, lexemes, output),
        TypeRef::Tuple(items) => {
            punctuation_component(
                tokens,
                whole,
                TypeTokenKind::OpenParen,
                path,
                TypeRefComponentRole::TupleOpen,
                output,
            )?;
            punctuation_component(
                tokens,
                whole,
                TypeTokenKind::CloseParen,
                path,
                TypeRefComponentRole::TupleClose,
                output,
            )?;
            collect_indexed_children(
                tokens,
                items,
                path,
                sources,
                output,
                TypeRefNodeStep::TupleItem,
                |ordinal| TypeRefComponentRole::TupleElement { ordinal },
                |ordinal| TypeRefComponentRole::TupleSeparator { ordinal },
                pending,
            )?;
        }
        TypeRef::Function {
            params,
            return_type,
            effects,
        } => {
            let return_path = path.child(TypeRefNodeStep::FunctionReturn);
            let return_range = *sources
                .get(&return_path)
                .ok_or_else(|| TypeParseError::new("function return type has no source owner"))?;
            let (node_start, _) = token_bounds(tokens, whole)
                .ok_or_else(|| TypeParseError::new("function type has no token source"))?;
            // Parenthesized grouping is erased from the semantic TypeRef, so
            // the owner's arrow is not necessarily top-level within `whole`.
            // The final arrow before the exact return child is nevertheless
            // unique, including for right-associative and nested functions.
            let (arrow_index, arrow_token) = tokens
                .iter()
                .enumerate()
                .rev()
                .find(|(_, token)| {
                    token.range.start() >= whole.start()
                        && token.range.end() <= return_range.start()
                        && matches!(token.kind, TypeTokenKind::ThinArrow)
                })
                .ok_or_else(|| TypeParseError::new("function type has no arrow source"))?;
            let arrow = arrow_token.range;
            component(output, path, TypeRefComponentRole::FunctionArrow, arrow);
            component(
                output,
                path,
                TypeRefComponentRole::FunctionReturn,
                return_range,
            );
            let last_parameter = params
                .len()
                .checked_sub(1)
                .ok_or_else(|| TypeParseError::new("function type has no parameter source"))?;
            let last_parameter = u16::try_from(last_parameter)
                .map_err(|_| TypeParseError::resource_limit("type node ordinal overflow"))?;
            let last_parameter_range =
                sources[&path.child(TypeRefNodeStep::FunctionParameter(last_parameter))];
            let parameter_close = tokens
                .iter()
                .enumerate()
                .take(arrow_index)
                .rev()
                .find(|(_, token)| {
                    token.range.start() >= last_parameter_range.end()
                        && matches!(token.kind, TypeTokenKind::CloseParen)
                })
                .map(|(index, _)| index);
            if let Some(close_index) = parameter_close {
                let open_index = (node_start..close_index).rev().find(|candidate| {
                    matches!(tokens[*candidate].kind, TypeTokenKind::OpenParen)
                        && matching_close(tokens, *candidate, close_index + 1, Delimiter::Paren)
                            == Some(close_index)
                });
                let open_index = open_index.ok_or_else(|| {
                    TypeParseError::new("function parameter group has no open source")
                })?;
                component(
                    output,
                    path,
                    TypeRefComponentRole::FunctionOpen,
                    tokens[open_index].range,
                );
                component(
                    output,
                    path,
                    TypeRefComponentRole::FunctionClose,
                    tokens[close_index].range,
                );
            }
            for (index, param) in params.iter().enumerate() {
                let index = index_u32(index)?;
                let child_path = path.child(TypeRefNodeStep::FunctionParameter(
                    index_u16_from_u32(index)?,
                ));
                let child_range = *sources
                    .get(&child_path)
                    .ok_or_else(|| TypeParseError::new("function parameter has no source owner"))?;
                component(
                    output,
                    path,
                    TypeRefComponentRole::FunctionParameter { ordinal: index },
                    child_range,
                );
                pending.push((child_path.clone(), param));
                if index > 0 {
                    let previous = path.child(TypeRefNodeStep::FunctionParameter(
                        index_u16_from_u32(index - 1)?,
                    ));
                    let separator =
                        comma_between(tokens, sources[&previous].end(), child_range.start())?;
                    component(
                        output,
                        path,
                        TypeRefComponentRole::FunctionSeparator { ordinal: index - 1 },
                        separator,
                    );
                }
            }
            pending.push((return_path, return_type));
            if effects.is_some() {
                collect_effect_components(tokens, path, return_range.end(), whole.end(), output)?;
            }
        }
        TypeRef::Choice(items) => collect_indexed_children(
            tokens,
            items,
            path,
            sources,
            output,
            TypeRefNodeStep::ChoiceAlternative,
            |ordinal| TypeRefComponentRole::ChoiceAlternative { ordinal },
            |ordinal| TypeRefComponentRole::ChoiceSeparator { ordinal },
            pending,
        )?,
        TypeRef::Generic { args, .. } => {
            collect_path_components(path, lexemes, output);
            collect_generic_components(
                tokens,
                args,
                &[],
                path,
                sources,
                lexemes,
                output,
                false,
                pending,
            )?;
        }
        TypeRef::TraitBound(bound) => {
            collect_path_components(path, lexemes, output);
            collect_generic_components(
                tokens,
                bound.args(),
                bound.associated(),
                path,
                sources,
                lexemes,
                output,
                true,
                pending,
            )?;
        }
        TypeRef::Projection { subject, .. } => {
            let subject_path = path.child(TypeRefNodeStep::ProjectionSubject);
            let subject_range = sources[&subject_path];
            component(
                output,
                path,
                TypeRefComponentRole::ProjectionSubject,
                subject_range,
            );
            let separator = find_token_between(tokens, subject_range.end(), whole.end(), |kind| {
                matches!(kind, TypeTokenKind::PathSeparator)
            })
            .ok_or_else(|| TypeParseError::new("projection has no separator source"))?;
            component(
                output,
                path,
                TypeRefComponentRole::ProjectionSeparator,
                separator,
            );
            let name = find_token_between(tokens, separator.end(), whole.end(), |kind| {
                matches!(kind, TypeTokenKind::Identifier(_))
            })
            .ok_or_else(|| TypeParseError::new("projection has no name source"))?;
            component(output, path, TypeRefComponentRole::ProjectionName, name);
            pending.push((subject_path, subject));
        }
        TypeRef::Reference(reference) => {
            component(
                output,
                path,
                TypeRefComponentRole::ReferenceAmpersand,
                reference.amp_range(),
            );
            match reference.region().name() {
                Some(_) => {
                    let range = reference.region().range();
                    component(
                        output,
                        path,
                        TypeRefComponentRole::Region(TypeRefRegionPart::Whole),
                        range,
                    );
                    component(
                        output,
                        path,
                        TypeRefComponentRole::Region(TypeRefRegionPart::NamedApostrophe),
                        TextRange::new(range.start(), range.start() + 1),
                    );
                    component(
                        output,
                        path,
                        TypeRefComponentRole::Region(TypeRefRegionPart::NamedName),
                        TextRange::new(range.start() + 1, range.end()),
                    );
                }
                None => component(
                    output,
                    path,
                    TypeRefComponentRole::Region(TypeRefRegionPart::ElisionInsertion),
                    reference.region().range(),
                ),
            }
            if let Some(range) = reference.mut_range() {
                component(
                    output,
                    path,
                    TypeRefComponentRole::ReferenceMutKeyword,
                    range,
                );
            }
            let referent_path = path.child(TypeRefNodeStep::ReferenceReferent);
            let referent_range = sources[&referent_path];
            component(
                output,
                path,
                TypeRefComponentRole::ReferenceReferent,
                referent_range,
            );
            pending.push((referent_path, reference.referent()));
        }
        TypeRef::Slice(item) => {
            punctuation_component(
                tokens,
                whole,
                TypeTokenKind::OpenBracket,
                path,
                TypeRefComponentRole::SliceOpen,
                output,
            )?;
            punctuation_component(
                tokens,
                whole,
                TypeTokenKind::CloseBracket,
                path,
                TypeRefComponentRole::SliceClose,
                output,
            )?;
            let item_path = path.child(TypeRefNodeStep::SliceItem);
            component(
                output,
                path,
                TypeRefComponentRole::SliceElement,
                sources[&item_path],
            );
            pending.push((item_path, item));
        }
        TypeRef::Recovery(_) => component(output, path, TypeRefComponentRole::Recovery, whole),
    }
    Ok(())
}

fn component(
    output: &mut Vec<TypeRefComponentSource<TextRange>>,
    owner: &TypeRefNodePath,
    role: TypeRefComponentRole,
    range: TextRange,
) {
    output.push(TypeRefComponentSource::new(owner.clone(), role, range));
}

fn collect_path_components(
    path: &TypeRefNodePath,
    lexemes: &[TypeRefLexemeSource<TextRange>],
    output: &mut Vec<TypeRefComponentSource<TextRange>>,
) {
    for lexeme in lexemes.iter().filter(|lexeme| lexeme.owner() == path) {
        match lexeme.kind() {
            TypeRefLexemeKind::PathRoot => component(
                output,
                path,
                TypeRefComponentRole::PathRoot,
                *lexeme.range(),
            ),
            TypeRefLexemeKind::PathSegment { ordinal } => component(
                output,
                path,
                TypeRefComponentRole::PathSegment {
                    ordinal: u32::from(*ordinal),
                },
                *lexeme.range(),
            ),
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_indexed_children<'a>(
    tokens: &[TypeToken<'_>],
    items: &'a [TypeRef],
    path: &TypeRefNodePath,
    sources: &std::collections::BTreeMap<TypeRefNodePath, TextRange>,
    output: &mut Vec<TypeRefComponentSource<TextRange>>,
    step: fn(u16) -> TypeRefNodeStep,
    item_role: impl Fn(u32) -> TypeRefComponentRole,
    separator_role: impl Fn(u32) -> TypeRefComponentRole,
    pending: &mut Vec<(TypeRefNodePath, &'a TypeRef)>,
) -> Result<(), TypeParseError> {
    let mut previous_end = None;
    for (index, item) in items.iter().enumerate() {
        let ordinal = index_u32(index)?;
        let child_path = path.child(step(index_u16_from_u32(ordinal)?));
        let child_range = sources[&child_path];
        component(output, path, item_role(ordinal), child_range);
        if let Some(previous_end) = previous_end {
            let separator = find_token_between(tokens, previous_end, child_range.start(), |kind| {
                matches!(kind, TypeTokenKind::Comma | TypeTokenKind::Pipe)
            })
            .ok_or_else(|| TypeParseError::new("type sequence has no separator source"))?;
            component(output, path, separator_role(ordinal - 1), separator);
        }
        pending.push((child_path, item));
        previous_end = Some(child_range.end());
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "generic and trait roles share one canonical token/source-map transaction"
)]
fn collect_generic_components<'a>(
    tokens: &[TypeToken<'_>],
    args: &'a [TypeRef],
    associated: &'a [AssociatedTypeBinding],
    path: &TypeRefNodePath,
    sources: &std::collections::BTreeMap<TypeRefNodePath, TextRange>,
    lexemes: &[TypeRefLexemeSource<TextRange>],
    output: &mut Vec<TypeRefComponentSource<TextRange>>,
    is_trait: bool,
    pending: &mut Vec<(TypeRefNodePath, &'a TypeRef)>,
) -> Result<(), TypeParseError> {
    let whole = sources[path];
    let open = find_token(tokens, whole, |kind| {
        matches!(kind, TypeTokenKind::OpenAngle)
    })
    .ok_or_else(|| TypeParseError::new("generic type has no open delimiter source"))?;
    let close = tokens
        .iter()
        .rev()
        .find(|token| {
            matches!(token.kind, TypeTokenKind::CloseAngle)
                && token.range.end() <= whole.end()
                && token.range.start() >= open.end()
        })
        .map(|token| token.range)
        .ok_or_else(|| TypeParseError::new("generic type has no close delimiter source"))?;
    let base_end = lexemes
        .iter()
        .find(|lexeme| {
            lexeme.owner() == path && matches!(lexeme.kind(), TypeRefLexemeKind::TurbofishSeparator)
        })
        .map_or(open.start(), |lexeme| lexeme.range().start());
    let base = TextRange::new(whole.start(), base_end);
    component(
        output,
        path,
        if is_trait {
            TypeRefComponentRole::TraitBase
        } else {
            TypeRefComponentRole::GenericBase
        },
        base,
    );
    component(
        output,
        path,
        if is_trait {
            TypeRefComponentRole::TraitOpen
        } else {
            TypeRefComponentRole::GenericOpen
        },
        open,
    );
    component(
        output,
        path,
        if is_trait {
            TypeRefComponentRole::TraitClose
        } else {
            TypeRefComponentRole::GenericClose
        },
        close,
    );

    for (index, arg) in args.iter().enumerate() {
        let ordinal = index_u32(index)?;
        let child_path = path.child(if is_trait {
            TypeRefNodeStep::TraitArgument(index_u16_from_u32(ordinal)?)
        } else {
            TypeRefNodeStep::GenericArgument(index_u16_from_u32(ordinal)?)
        });
        component(
            output,
            path,
            if is_trait {
                TypeRefComponentRole::TraitArgument { ordinal }
            } else {
                TypeRefComponentRole::GenericArgument { ordinal }
            },
            sources[&child_path],
        );
        pending.push((child_path, arg));
    }
    for (index, binding) in associated.iter().enumerate() {
        let ordinal = index_u32(index)?;
        let child_path = path.child(TypeRefNodeStep::AssociatedBinding(index_u16_from_u32(
            ordinal,
        )?));
        let value_range = sources[&child_path];
        let equals = tokens
            .iter()
            .rev()
            .find(|token| {
                matches!(token.kind, TypeTokenKind::Equals)
                    && token.range.end() <= value_range.start()
                    && token.range.start() > open.end()
            })
            .map(|token| token.range)
            .ok_or_else(|| TypeParseError::new("associated binding has no equals source"))?;
        let name = tokens
            .iter()
            .rev()
            .find(|token| {
                matches!(token.kind, TypeTokenKind::Identifier(_))
                    && token.range.end() <= equals.start()
                    && token.range.start() >= open.end()
            })
            .map(|token| token.range)
            .ok_or_else(|| TypeParseError::new("associated binding has no name source"))?;
        let whole_binding = TextRange::new(name.start(), value_range.end());
        for (part, range) in [
            (TypeRefAssociatedBindingPart::Whole, whole_binding),
            (TypeRefAssociatedBindingPart::Name, name),
            (TypeRefAssociatedBindingPart::Equals, equals),
            (TypeRefAssociatedBindingPart::Value, value_range),
        ] {
            component(
                output,
                path,
                TypeRefComponentRole::AssociatedBinding { ordinal, part },
                range,
            );
        }
        pending.push((child_path, binding.value()));
    }
    let separators = lexemes.iter().filter(|lexeme| {
        lexeme.owner() == path
            && matches!(
                lexeme.kind(),
                TypeRefLexemeKind::ArgumentSeparator { .. }
                    | TypeRefLexemeKind::TrailingArgumentSeparator
            )
    });
    for (index, separator) in separators.enumerate() {
        let ordinal = index_u32(index)?;
        component(
            output,
            path,
            if is_trait {
                TypeRefComponentRole::TraitSeparator { ordinal }
            } else {
                TypeRefComponentRole::GenericSeparator { ordinal }
            },
            *separator.range(),
        );
    }
    Ok(())
}

fn collect_effect_components(
    tokens: &[TypeToken<'_>],
    path: &TypeRefNodePath,
    start: usize,
    end: usize,
    output: &mut Vec<TypeRefComponentSource<TextRange>>,
) -> Result<(), TypeParseError> {
    let open_index = tokens
        .iter()
        .position(|token| {
            token.range.start() >= start
                && token.range.end() <= end
                && matches!(token.kind, TypeTokenKind::OpenBrace)
        })
        .ok_or_else(|| TypeParseError::new("function effect row has no open source"))?;
    let close_index = tokens
        .iter()
        .rposition(|token| {
            token.range.start() >= start
                && token.range.end() <= end
                && matches!(token.kind, TypeTokenKind::CloseBrace)
        })
        .ok_or_else(|| TypeParseError::new("function effect row has no close source"))?;
    component(
        output,
        path,
        TypeRefComponentRole::FunctionEffectOpen,
        tokens[open_index].range,
    );
    component(
        output,
        path,
        TypeRefComponentRole::FunctionEffectClose,
        tokens[close_index].range,
    );
    for (index, (label_start, label_end)) in
        split_top_level(tokens, open_index + 1, close_index, |kind| {
            matches!(kind, TypeTokenKind::Comma)
        })
        .into_iter()
        .filter(|(start, end)| start != end)
        .enumerate()
    {
        component(
            output,
            path,
            TypeRefComponentRole::FunctionEffect {
                ordinal: index_u32(index)?,
            },
            token_range(tokens, label_start, label_end)?,
        );
    }
    Ok(())
}

fn punctuation_component(
    tokens: &[TypeToken<'_>],
    whole: TextRange,
    expected: TypeTokenKind<'_>,
    path: &TypeRefNodePath,
    role: TypeRefComponentRole,
    output: &mut Vec<TypeRefComponentSource<TextRange>>,
) -> Result<(), TypeParseError> {
    let range = find_token(tokens, whole, |kind| {
        core::mem::discriminant(kind) == core::mem::discriminant(&expected)
    })
    .ok_or_else(|| TypeParseError::new("type component has no punctuation source"))?;
    component(output, path, role, range);
    Ok(())
}

fn find_token(
    tokens: &[TypeToken<'_>],
    whole: TextRange,
    predicate: impl Fn(&TypeTokenKind<'_>) -> bool,
) -> Option<TextRange> {
    tokens
        .iter()
        .find(|token| {
            token.range.start() >= whole.start()
                && token.range.end() <= whole.end()
                && predicate(&token.kind)
        })
        .map(|token| token.range)
}

fn token_bounds(tokens: &[TypeToken<'_>], whole: TextRange) -> Option<(usize, usize)> {
    let start = tokens
        .iter()
        .position(|token| token.range.start() >= whole.start())?;
    let end = tokens
        .iter()
        .rposition(|token| token.range.end() <= whole.end())?
        + 1;
    Some((start, end))
}

fn find_token_between(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    predicate: impl Fn(&TypeTokenKind<'_>) -> bool,
) -> Option<TextRange> {
    tokens
        .iter()
        .find(|token| {
            token.range.start() >= start && token.range.end() <= end && predicate(&token.kind)
        })
        .map(|token| token.range)
}

fn comma_between(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
) -> Result<TextRange, TypeParseError> {
    find_token_between(tokens, start, end, |kind| {
        matches!(kind, TypeTokenKind::Comma)
    })
    .ok_or_else(|| TypeParseError::new("type sequence has no comma source"))
}

fn index_u32(index: usize) -> Result<u32, TypeParseError> {
    u32::try_from(index)
        .map_err(|_| TypeParseError::resource_limit("type component ordinal overflow"))
}

fn index_u16_from_u32(index: u32) -> Result<u16, TypeParseError> {
    u16::try_from(index).map_err(|_| TypeParseError::resource_limit("type node ordinal overflow"))
}

fn parse_type(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    path: &TypeRefNodePath,
) -> Result<ParsedTypeRef, TypeParseError> {
    require_nonempty(tokens, start, end)?;
    let (function_end, effects) = split_effect_row(tokens, start, end)?;
    if let Some(arrow) = first_top_level(tokens, start, function_end, |kind| {
        matches!(kind, TypeTokenKind::ThinArrow)
    }) {
        let params = parse_function_parameters(tokens, start, arrow, path)?;
        if arrow + 1 == function_end {
            return Err(TypeParseError::new("expected return type after `->`"));
        }
        let return_type = parse_type(
            tokens,
            arrow + 1,
            function_end,
            &path.child(TypeRefNodeStep::FunctionReturn),
        )?;
        let mut values = Vec::with_capacity(params.len());
        let mut nodes = vec![(
            path.clone(),
            TypeRefNodeSource::new(token_range(tokens, start, end)?, None),
        )];
        let mut lexemes = Vec::new();
        for param in params {
            values.push(param.value);
            nodes.extend(param.nodes);
            lexemes.extend(param.lexemes);
        }
        nodes.extend(return_type.nodes);
        lexemes.extend(return_type.lexemes);
        return Ok(ParsedTypeRef {
            value: TypeRef::Function {
                params: values,
                return_type: Box::new(return_type.value),
                effects,
            },
            nodes,
            lexemes,
        });
    }

    let mut parsed = parse_choice(tokens, start, function_end, path)?;
    if let Some(effects) = effects {
        let TypeRef::Function {
            params,
            return_type,
            effects: current,
        } = parsed.value
        else {
            return Err(TypeParseError::new(
                "effect row can only annotate a function type",
            ));
        };
        if current.is_some() {
            return Err(TypeParseError::new(
                "function type cannot declare multiple effect rows",
            ));
        }
        parsed.value = TypeRef::Function {
            params,
            return_type,
            effects: Some(effects),
        };
        parsed.replace_node_whole(path, token_range(tokens, start, end)?);
    }
    Ok(parsed)
}

fn parse_function_parameters(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    path: &TypeRefNodePath,
) -> Result<Vec<ParsedTypeRef>, TypeParseError> {
    require_nonempty(tokens, start, end)?;
    let direct_group = matches!(tokens[start].kind, TypeTokenKind::OpenParen)
        && matching_close(tokens, start, end, Delimiter::Paren) == Some(end - 1);
    let mut params = Vec::new();
    if direct_group {
        let inner_start = start + 1;
        let inner_end = end - 1;
        require_nonempty(tokens, inner_start, inner_end)?;
        let parts = split_top_level(tokens, inner_start, inner_end, |kind| {
            matches!(kind, TypeTokenKind::Comma)
        });
        if parts.len() > 1 {
            for (index, (part_start, part_end)) in parts.into_iter().enumerate() {
                require_nonempty(tokens, part_start, part_end)?;
                params.push(parse_type(
                    tokens,
                    part_start,
                    part_end,
                    &path.child(TypeRefNodeStep::FunctionParameter(index_u16(index, path)?)),
                )?);
            }
        } else {
            let parsed = parse_type(
                tokens,
                inner_start,
                inner_end,
                &path.child(TypeRefNodeStep::FunctionParameter(0)),
            )?;
            if matches!(parsed.value, TypeRef::Tuple(_)) {
                return Err(TypeParseError::new(
                    "function parameter group cannot contain an anonymous tuple type; use `(A, B) -> C` for one call group",
                ));
            }
            params.push(parsed);
        }
    } else {
        params.push(parse_choice(
            tokens,
            start,
            end,
            &path.child(TypeRefNodeStep::FunctionParameter(0)),
        )?);
    }
    Ok(params)
}

fn parse_choice(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    path: &TypeRefNodePath,
) -> Result<ParsedTypeRef, TypeParseError> {
    let alternatives = split_top_level(tokens, start, end, |kind| {
        matches!(kind, TypeTokenKind::Pipe)
    });
    if alternatives.len() == 1 {
        return parse_atom(tokens, start, end, path);
    }
    let mut values = Vec::with_capacity(alternatives.len());
    let mut nodes = vec![(
        path.clone(),
        TypeRefNodeSource::new(token_range(tokens, start, end)?, None),
    )];
    let mut lexemes = Vec::new();
    for (index, (alternative_start, alternative_end)) in alternatives.into_iter().enumerate() {
        require_nonempty(tokens, alternative_start, alternative_end)?;
        let parsed = parse_atom(
            tokens,
            alternative_start,
            alternative_end,
            &path.child(TypeRefNodeStep::ChoiceAlternative(index_u16(index, path)?)),
        )?;
        values.push(parsed.value);
        nodes.extend(parsed.nodes);
        lexemes.extend(parsed.lexemes);
    }
    Ok(ParsedTypeRef {
        value: TypeRef::Choice(values),
        nodes,
        lexemes,
    })
}

fn parse_atom(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    path: &TypeRefNodePath,
) -> Result<ParsedTypeRef, TypeParseError> {
    require_nonempty(tokens, start, end)?;
    let whole = token_range(tokens, start, end)?;

    if matches!(tokens[start].kind, TypeTokenKind::OpenParen)
        && matching_close(tokens, start, end, Delimiter::Paren) == Some(end - 1)
    {
        return parse_parenthesized(tokens, start, end, path, whole);
    }

    if end == start + 1 {
        match tokens[start].kind {
            TypeTokenKind::Integer(raw) => {
                let value = raw
                    .parse::<usize>()
                    .map_err(|_| TypeParseError::new("invalid const integer type"))?;
                return Ok(ParsedTypeRef::node(
                    TypeRef::ConstInt(value),
                    path,
                    whole,
                    Some(TypeRefHeadSource::new(TypeRefHeadKind::ConstInt, whole)),
                    Vec::new(),
                ));
            }
            TypeTokenKind::Bang | TypeTokenKind::Identifier("Never") => {
                return Ok(ParsedTypeRef::node(
                    TypeRef::Never,
                    path,
                    whole,
                    Some(TypeRefHeadSource::new(TypeRefHeadKind::Never, whole)),
                    Vec::new(),
                ));
            }
            _ => {}
        }
    }

    if matches!(tokens[start].kind, TypeTokenKind::Ampersand) {
        return parse_reference(tokens, start, end, path);
    }
    if matches!(tokens[start].kind, TypeTokenKind::OpenBracket)
        && matching_close(tokens, start, end, Delimiter::Bracket) == Some(end - 1)
    {
        let parsed = parse_type(
            tokens,
            start + 1,
            end - 1,
            &path.child(TypeRefNodeStep::SliceItem),
        )?;
        return Ok(ParsedTypeRef {
            value: TypeRef::Slice(Box::new(parsed.value)),
            nodes: std::iter::once((path.clone(), TypeRefNodeSource::new(whole, None)))
                .chain(parsed.nodes)
                .collect(),
            lexemes: parsed.lexemes,
        });
    }
    if let Some(projection) = try_parse_projection(tokens, start, end, path, whole)? {
        return Ok(projection);
    }
    if let Some(open) = first_top_level(tokens, start, end, |kind| {
        matches!(kind, TypeTokenKind::OpenAngle)
    }) {
        return parse_generic(tokens, start, end, open, path);
    }
    parse_path_node(tokens, start, end, path, TypeRefHeadKind::Path)
}

fn parse_parenthesized(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    path: &TypeRefNodePath,
    whole: TextRange,
) -> Result<ParsedTypeRef, TypeParseError> {
    let inner_start = start + 1;
    let inner_end = end - 1;
    require_nonempty(tokens, inner_start, inner_end)?;
    let items = split_top_level(tokens, inner_start, inner_end, |kind| {
        matches!(kind, TypeTokenKind::Comma)
    });
    if items.len() == 1 {
        let mut parsed = parse_type(tokens, inner_start, inner_end, path)?;
        parsed.replace_node_whole(path, whole);
        return Ok(parsed);
    }

    let mut values = Vec::with_capacity(items.len());
    let mut nodes = vec![(path.clone(), TypeRefNodeSource::new(whole, None))];
    let mut lexemes = Vec::new();
    for (index, (item_start, item_end)) in items.into_iter().enumerate() {
        require_nonempty(tokens, item_start, item_end)?;
        let parsed = parse_type(
            tokens,
            item_start,
            item_end,
            &path.child(TypeRefNodeStep::TupleItem(index_u16(index, path)?)),
        )?;
        values.push(parsed.value);
        nodes.extend(parsed.nodes);
        lexemes.extend(parsed.lexemes);
    }
    Ok(ParsedTypeRef {
        value: TypeRef::Tuple(values),
        nodes,
        lexemes,
    })
}

fn try_parse_projection(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    path: &TypeRefNodePath,
    whole: TextRange,
) -> Result<Option<ParsedTypeRef>, TypeParseError> {
    let Some(separator) = last_top_level(tokens, start, end, |kind| {
        matches!(kind, TypeTokenKind::PathSeparator)
    }) else {
        return Ok(None);
    };
    if separator + 2 != end {
        return Ok(None);
    }
    let TypeTokenKind::Identifier(assoc) = tokens[separator + 1].kind else {
        return Ok(None);
    };
    let parsed = parse_type(
        tokens,
        start,
        separator,
        &path.child(TypeRefNodeStep::ProjectionSubject),
    )?;
    let assoc_range = tokens[separator + 1].range;
    let assoc = ModuleSegment::new(assoc.to_owned()).map_err(|error| {
        TypeParseError::new_owned(format!("invalid associated type name `{assoc}`: {error}"))
    })?;
    Ok(Some(ParsedTypeRef {
        value: TypeRef::Projection {
            subject: Box::new(parsed.value),
            assoc,
        },
        nodes: std::iter::once((
            path.clone(),
            TypeRefNodeSource::new(
                whole,
                Some(TypeRefHeadSource::with_terminal(
                    TypeRefHeadKind::ProjectionMember,
                    assoc_range,
                    assoc_range,
                )),
            ),
        ))
        .chain(parsed.nodes)
        .collect(),
        lexemes: parsed.lexemes,
    }))
}

fn parse_reference(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    path: &TypeRefNodePath,
) -> Result<ParsedTypeRef, TypeParseError> {
    let amp = tokens[start].range;
    let mut cursor = start + 1;
    let region = if cursor < end {
        if let TypeTokenKind::Lifetime(lifetime) = tokens[cursor].kind {
            let range = tokens[cursor].range;
            cursor += 1;
            RegionSyntax::Named {
                name: parse_lifetime_name(lifetime, range),
                range,
            }
        } else {
            RegionSyntax::Elided {
                anchor: TextRange::new(amp.end(), amp.end()),
            }
        }
    } else {
        RegionSyntax::Elided {
            anchor: TextRange::new(amp.end(), amp.end()),
        }
    };
    let (kind, mut_range) =
        if cursor < end && matches!(tokens[cursor].kind, TypeTokenKind::Identifier("mut")) {
            let range = tokens[cursor].range;
            cursor += 1;
            (BorrowKind::Mutable, Some(range))
        } else {
            (BorrowKind::Shared, None)
        };
    if kind.is_mutable()
        && cursor < end
        && matches!(tokens[cursor].kind, TypeTokenKind::Lifetime(_))
    {
        return Err(TypeParseError::at(
            "syntax.type.region_after_mut",
            "a reference lifetime must appear before `mut`",
            tokens[cursor].range,
        ));
    }
    if cursor == end {
        let insertion = tokens[end - 1].range.end();
        return Err(TypeParseError::at(
            "syntax.type.reference_missing_referent",
            "reference type requires a referent",
            TextRange::new(insertion, insertion),
        ));
    }
    let referent = parse_type(
        tokens,
        cursor,
        end,
        &path.child(TypeRefNodeStep::ReferenceReferent),
    )?;
    let whole = token_range(tokens, start, end)?;
    Ok(ParsedTypeRef {
        value: TypeRef::Reference(ReferenceType::new(
            kind,
            region,
            Box::new(referent.value),
            amp,
            mut_range,
            whole,
        )),
        nodes: std::iter::once((path.clone(), TypeRefNodeSource::new(whole, None)))
            .chain(referent.nodes)
            .collect(),
        lexemes: referent.lexemes,
    })
}

fn parse_generic(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    open: usize,
    path: &TypeRefNodePath,
) -> Result<ParsedTypeRef, TypeParseError> {
    let Some(close) = matching_close(tokens, open, end, Delimiter::Angle) else {
        return Err(TypeParseError::at(
            "syntax.type.invalid",
            "unclosed generic argument list",
            tokens[open].range,
        ));
    };
    if close + 1 != end {
        return Err(TypeParseError::new(
            "unexpected tokens after generic argument list",
        ));
    }
    if let Some(parsed) = parse_nested_unary_generics(tokens, start, end, path)? {
        return Ok(parsed);
    }
    let turbofish = (open > start && matches!(tokens[open - 1].kind, TypeTokenKind::PathSeparator))
        .then_some(open - 1);
    let base_end = turbofish.unwrap_or(open);
    require_nonempty(tokens, start, base_end)?;

    let raw_parts = split_top_level_with_separators(tokens, open + 1, close, |kind| {
        matches!(kind, TypeTokenKind::Comma)
    });
    if matches!(raw_parts.as_slice(), [(part_start, part_end, None)] if part_start == part_end) {
        return Err(TypeParseError::at(
            "syntax.type.invalid",
            "generic argument list requires at least one type",
            tokens[close].range,
        ));
    }
    let trailing = raw_parts
        .last()
        .is_some_and(|(part_start, part_end, separator)| {
            part_start == part_end && separator.is_some()
        });
    let part_count = raw_parts.len().saturating_sub(usize::from(trailing));
    let ParsedGenericArguments {
        is_trait,
        type_args,
        associated,
        mut nodes,
        mut lexemes,
    } = parse_generic_arguments(tokens, &raw_parts[..part_count], path)?;

    let head_kind = if is_trait {
        TypeRefHeadKind::Trait
    } else {
        TypeRefHeadKind::Constructor
    };
    let ParsedTypePath {
        value: base,
        head,
        lexemes: path_lexemes,
    } = parse_path(tokens, start, base_end, path, head_kind)?;
    lexemes.extend(path_lexemes);
    append_generic_boundary_lexemes(
        tokens,
        path,
        turbofish,
        open,
        close,
        &raw_parts,
        part_count,
        trailing,
        &mut lexemes,
    )?;
    nodes.push((
        path.clone(),
        TypeRefNodeSource::new(token_range(tokens, start, end)?, Some(head)),
    ));
    let value = if is_trait {
        TypeRef::TraitBound(TraitBound {
            path: base,
            args: type_args,
            associated,
        })
    } else {
        TypeRef::Generic {
            base,
            args: type_args,
        }
    };
    Ok(ParsedTypeRef {
        value,
        nodes,
        lexemes,
    })
}

fn parse_nested_unary_generics(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    path: &TypeRefNodePath,
) -> Result<Option<ParsedTypeRef>, TypeParseError> {
    let mut layers = Vec::new();
    let mut current_start = start;
    let mut current_end = end;
    let mut current_path = path.clone();

    while let Some(layer) =
        unary_generic_layer(tokens, current_start, current_end, current_path.clone())?
    {
        current_start = layer.argument.0;
        current_end = layer.argument.1;
        current_path = current_path.child(TypeRefNodeStep::GenericArgument(0));
        layers.push(layer);
    }

    if layers.len() < 2 {
        return Ok(None);
    }

    let mut parsed = parse_type(tokens, current_start, current_end, &current_path)?;
    for layer in layers.into_iter().rev() {
        let ParsedTypePath {
            value: base,
            head,
            lexemes: path_lexemes,
        } = parse_path(
            tokens,
            layer.start,
            layer.base_end,
            &layer.path,
            TypeRefHeadKind::Constructor,
        )?;
        parsed.lexemes.extend(path_lexemes);
        append_generic_boundary_lexemes(
            tokens,
            &layer.path,
            layer.turbofish,
            layer.open,
            layer.close,
            std::slice::from_ref(&layer.argument),
            1,
            false,
            &mut parsed.lexemes,
        )?;
        parsed.nodes.push((
            layer.path.clone(),
            TypeRefNodeSource::new(token_range(tokens, layer.start, layer.end)?, Some(head)),
        ));
        parsed.value = TypeRef::Generic {
            base,
            args: vec![parsed.value],
        };
    }
    Ok(Some(parsed))
}

fn unary_generic_layer(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    path: TypeRefNodePath,
) -> Result<Option<UnaryGenericLayer>, TypeParseError> {
    let Some(open) = first_top_level(tokens, start, end, |kind| {
        matches!(kind, TypeTokenKind::OpenAngle)
    }) else {
        return Ok(None);
    };
    let Some(close) = matching_close(tokens, open, end, Delimiter::Angle) else {
        return Ok(None);
    };
    if close + 1 != end {
        return Ok(None);
    }
    let raw_parts = split_top_level_with_separators(tokens, open + 1, close, |kind| {
        matches!(kind, TypeTokenKind::Comma)
    });
    let [argument] = raw_parts.as_slice() else {
        return Ok(None);
    };
    if argument.0 == argument.1
        || first_top_level(tokens, argument.0, argument.1, |kind| {
            matches!(kind, TypeTokenKind::Equals)
        })
        .is_some()
    {
        return Ok(None);
    }
    let turbofish = (open > start && matches!(tokens[open - 1].kind, TypeTokenKind::PathSeparator))
        .then_some(open - 1);
    let base_end = turbofish.unwrap_or(open);
    require_nonempty(tokens, start, base_end)?;
    Ok(Some(UnaryGenericLayer {
        start,
        end,
        open,
        close,
        turbofish,
        base_end,
        argument: *argument,
        path,
    }))
}

fn parse_generic_arguments(
    tokens: &[TypeToken<'_>],
    parts: &[(usize, usize, Option<usize>)],
    path: &TypeRefNodePath,
) -> Result<ParsedGenericArguments, TypeParseError> {
    let is_trait = parts.iter().any(|(part_start, part_end, _)| {
        first_top_level(tokens, *part_start, *part_end, |kind| {
            matches!(kind, TypeTokenKind::Equals)
        })
        .is_some()
    });
    let mut type_args = Vec::new();
    let mut associated = Vec::new();
    let mut nodes = Vec::new();
    let mut lexemes = Vec::new();
    for &(part_start, part_end, _) in parts {
        require_nonempty(tokens, part_start, part_end)?;
        let equals = first_top_level(tokens, part_start, part_end, |kind| {
            matches!(kind, TypeTokenKind::Equals)
        });
        if let Some(equals) = equals {
            if equals != part_start + 1 {
                return Err(TypeParseError::new(
                    "expected associated type name before `=`",
                ));
            }
            let TypeTokenKind::Identifier(name) = tokens[part_start].kind else {
                return Err(TypeParseError::new(
                    "expected associated type name before `=`",
                ));
            };
            require_nonempty(tokens, equals + 1, part_end)?;
            let parsed = parse_type(
                tokens,
                equals + 1,
                part_end,
                &path.child(TypeRefNodeStep::AssociatedBinding(index_u16(
                    associated.len(),
                    path,
                )?)),
            )?;
            associated.push(AssociatedTypeBinding {
                name: ModuleSegment::new(name.to_owned()).map_err(|error| {
                    TypeParseError::new_owned(format!(
                        "invalid associated type name `{name}`: {error}"
                    ))
                })?,
                value: parsed.value,
            });
            nodes.extend(parsed.nodes);
            lexemes.extend(parsed.lexemes);
            continue;
        }

        let step = if is_trait {
            TypeRefNodeStep::TraitArgument(index_u16(type_args.len(), path)?)
        } else {
            TypeRefNodeStep::GenericArgument(index_u16(type_args.len(), path)?)
        };
        let parsed = parse_type(tokens, part_start, part_end, &path.child(step))?;
        type_args.push(parsed.value);
        nodes.extend(parsed.nodes);
        lexemes.extend(parsed.lexemes);
    }
    Ok(ParsedGenericArguments {
        is_trait,
        type_args,
        associated,
        nodes,
        lexemes,
    })
}

#[allow(clippy::too_many_arguments)]
fn append_generic_boundary_lexemes(
    tokens: &[TypeToken<'_>],
    path: &TypeRefNodePath,
    turbofish: Option<usize>,
    open: usize,
    close: usize,
    raw_parts: &[(usize, usize, Option<usize>)],
    part_count: usize,
    trailing: bool,
    lexemes: &mut Vec<TypeRefLexemeSource<TextRange>>,
) -> Result<(), TypeParseError> {
    if let Some(separator) = turbofish {
        lexemes.push(TypeRefLexemeSource::new(
            path.clone(),
            TypeRefLexemeKind::TurbofishSeparator,
            tokens[separator].range,
        ));
    }
    lexemes.push(TypeRefLexemeSource::new(
        path.clone(),
        TypeRefLexemeKind::OpenAngle,
        tokens[open].range,
    ));
    for (before, (_, _, separator)) in raw_parts.iter().take(part_count).enumerate().skip(1) {
        let separator = separator.expect("non-first generic parts retain their preceding comma");
        lexemes.push(TypeRefLexemeSource::new(
            path.clone(),
            TypeRefLexemeKind::ArgumentSeparator {
                before: index_u16(before, path)?,
            },
            tokens[separator].range,
        ));
    }
    if trailing {
        let separator = raw_parts
            .last()
            .and_then(|(_, _, separator)| *separator)
            .expect("a trailing empty part is introduced by a comma");
        lexemes.push(TypeRefLexemeSource::new(
            path.clone(),
            TypeRefLexemeKind::TrailingArgumentSeparator,
            tokens[separator].range,
        ));
    }
    lexemes.push(TypeRefLexemeSource::new(
        path.clone(),
        TypeRefLexemeKind::CloseAngle,
        tokens[close].range,
    ));
    Ok(())
}

fn parse_path_node(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    path: &TypeRefNodePath,
    head_kind: TypeRefHeadKind,
) -> Result<ParsedTypeRef, TypeParseError> {
    let ParsedTypePath {
        value,
        head,
        lexemes,
    } = parse_path(tokens, start, end, path, head_kind)?;
    Ok(ParsedTypeRef::node(
        TypeRef::Path(value),
        path,
        token_range(tokens, start, end)?,
        Some(head),
        lexemes,
    ))
}

fn parse_path(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    owner: &TypeRefNodePath,
    head_kind: TypeRefHeadKind,
) -> Result<ParsedTypePath, TypeParseError> {
    require_nonempty(tokens, start, end)?;
    let mut components = Vec::new();
    let mut separators = Vec::new();
    let mut cursor = start;
    loop {
        let TypeTokenKind::Identifier(spelling) = tokens[cursor].kind else {
            return Err(TypeParseError::at(
                "syntax.type.invalid",
                "expected identifier in type path",
                tokens[cursor].range,
            ));
        };
        if spelling == "_" {
            return Err(TypeParseError::at(
                "syntax.type.infer_unsupported",
                "inferred type syntax is not part of the final semantic type vocabulary",
                tokens[cursor].range,
            ));
        }
        components.push(TypePathComponent {
            spelling,
            range: tokens[cursor].range,
        });
        cursor += 1;
        if cursor == end {
            break;
        }
        if !matches!(
            tokens[cursor].kind,
            TypeTokenKind::Dot | TypeTokenKind::PathSeparator
        ) {
            return Err(TypeParseError::at(
                "syntax.type.invalid",
                "unexpected token in type path",
                tokens[cursor].range,
            ));
        }
        separators.push(tokens[cursor].range);
        cursor += 1;
        if cursor == end {
            return Err(TypeParseError::new("type path cannot end with a separator"));
        }
    }
    TypePath::from_token_parts(&components, &separators, owner, head_kind)
        .map_err(|error| TypeParseError::new_owned(format!("invalid type path: {error}")))
}

fn split_effect_row(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
) -> Result<(usize, Option<TypeEffectRow>), TypeParseError> {
    let Some(effects) = first_top_level(tokens, start, end, |kind| {
        matches!(kind, TypeTokenKind::Identifier("effects"))
    }) else {
        return Ok((end, None));
    };
    if effects + 1 >= end || !matches!(tokens[effects + 1].kind, TypeTokenKind::OpenBrace) {
        return Ok((end, None));
    }
    let Some(close) = matching_close(tokens, effects + 1, end, Delimiter::Brace) else {
        return Err(TypeParseError::new(
            "expected `{ ... }` after function type `effects`",
        ));
    };
    if close + 1 != end {
        return Err(TypeParseError::new(
            "unexpected tokens after function type effect row",
        ));
    }
    let labels = split_top_level(tokens, effects + 2, close, |kind| {
        matches!(kind, TypeTokenKind::Comma)
    })
    .into_iter()
    .filter(|(label_start, label_end)| label_start != label_end)
    .map(|(label_start, label_end)| token_label(tokens, label_start, label_end))
    .collect::<Result<Vec<_>, _>>()?;
    Ok((effects, Some(TypeEffectRow::new(labels))))
}

fn token_label(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
) -> Result<String, TypeParseError> {
    let mut label = String::new();
    for token in &tokens[start..end] {
        match token.kind {
            TypeTokenKind::Identifier(value)
            | TypeTokenKind::Lifetime(value)
            | TypeTokenKind::Integer(value) => label.push_str(value),
            TypeTokenKind::Bang => label.push('!'),
            TypeTokenKind::Ampersand => label.push('&'),
            TypeTokenKind::OpenParen => label.push('('),
            TypeTokenKind::CloseParen => label.push(')'),
            TypeTokenKind::OpenBracket => label.push('['),
            TypeTokenKind::CloseBracket => label.push(']'),
            TypeTokenKind::OpenAngle => label.push('<'),
            TypeTokenKind::CloseAngle => label.push('>'),
            TypeTokenKind::Dot => label.push('.'),
            TypeTokenKind::PathSeparator => label.push_str("::"),
            TypeTokenKind::Colon => label.push(':'),
            TypeTokenKind::Equals => label.push('='),
            TypeTokenKind::Pipe => label.push('|'),
            TypeTokenKind::ThinArrow => label.push_str("->"),
            _ => {
                return Err(TypeParseError::at(
                    "syntax.type.invalid",
                    "invalid effect label token",
                    token.range,
                ));
            }
        }
    }
    Ok(label)
}
