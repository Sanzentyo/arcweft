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
    TypeParseError, TypePath, TypeRef, TypeRefHeadKind, TypeRefHeadSource, TypeRefLexemeKind,
    TypeRefLexemeSource, TypeRefNodePath, TypeRefNodeSource, TypeRefNodeStep, parse_lifetime_name,
    validate_type_ref_limits,
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
    AuthoredTypeRef::try_new(parsed.value, parsed.nodes, parsed.lexemes).map_err(|error| {
        TypeParseError::new_owned(format!("invalid parser-owned type source map: {error:?}"))
    })
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
    if let Some(open) = first_top_level(tokens, start, end, |kind| {
        matches!(kind, TypeTokenKind::OpenAngle)
    }) {
        return parse_generic(tokens, start, end, open, path);
    }
    if let Some(projection) = try_parse_projection(tokens, start, end, path, whole)? {
        return Ok(projection);
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
    let trailing = raw_parts
        .last()
        .is_some_and(|(part_start, part_end, _)| part_start == part_end);
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
