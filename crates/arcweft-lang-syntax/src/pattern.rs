use crate::ast::{EntityRef, Pattern, RecordPatternField, TextRange, VariantPatternPayload};
use crate::expr::{Expr, parse_expr};
use crate::types::parse_type_ref;

/// Parses the shared Arcweft pattern language.
///
/// The same parser is used for `let`, `match`, `if let`, `while let`,
/// line-plan outputs, and function parameters so later binding and lowering
/// passes do not need a separate parameter-only pattern model.
pub(crate) fn parse_pattern(source: &str) -> Pattern {
    let source = source.trim();
    if source == "_" {
        return Pattern::Discard;
    }
    if let Some(name) = source
        .strip_prefix("mut ")
        .map(str::trim)
        .filter(|name| is_pattern_ident(name))
    {
        return Pattern::MutIdent(name.to_owned());
    }
    if let Some((name, ty)) = source.split_once(':') {
        let name = name.trim();
        if is_pattern_ident(name)
            && let Ok(ty) = parse_type_ref(ty.trim())
        {
            return Pattern::Typed {
                name: name.to_owned(),
                ty,
            };
        }
    }
    if let Some(pattern) = parse_variant_pattern(source) {
        return pattern;
    }
    if let Ok(Expr::EntityRef(entity)) = parse_expr(source) {
        return Pattern::Entity(entity);
    }
    if let Some(entity) = parse_entity_pattern(source) {
        return Pattern::Entity(entity);
    }
    if let Ok(expr @ Expr::Literal(_)) = parse_expr(source) {
        return Pattern::Literal(expr);
    }
    if let Some(inner) = source
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    {
        return Pattern::Tuple(
            split_pattern_items(inner)
                .into_iter()
                .map(parse_pattern)
                .collect(),
        );
    }
    if let Some(inner) = source
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return parse_list_pattern(inner);
    }
    if let Some(pattern) = parse_record_pattern(source) {
        return pattern;
    }
    if let Some((name, rest)) = split_whole_pattern(source) {
        return Pattern::Whole {
            name: name.to_owned(),
            pattern: Box::new(parse_pattern(rest)),
        };
    }
    if is_pattern_ident(source) {
        return Pattern::Ident(source.to_owned());
    }
    Pattern::Raw(source.to_owned())
}

fn parse_entity_pattern(source: &str) -> Option<EntityRef> {
    let body = if let Some(body) = source
        .strip_prefix("#<")
        .and_then(|value| value.strip_suffix('>'))
    {
        body
    } else {
        source.strip_prefix('#')?
    };
    (!body.trim().is_empty()).then(|| {
        EntityRef::new(
            body.trim().to_owned(),
            source.starts_with("#<"),
            TextRange::new(0, source.len()),
        )
    })
}

fn split_whole_pattern(source: &str) -> Option<(&str, &str)> {
    let (name, rest) = source.split_once(' ')?;
    let name = name.trim();
    let rest = rest.trim();
    (is_pattern_ident(name)
        && !matches!(name, "mut")
        && !rest.is_empty()
        && !is_pattern_ident(rest))
    .then_some((name, rest))
}

fn parse_list_pattern(inner: &str) -> Pattern {
    let mut rest = None;
    let items = split_pattern_items(inner)
        .into_iter()
        .filter_map(|item| {
            if item == ".." {
                rest = Some(String::new());
                None
            } else if let Some(name) = item.strip_prefix("..") {
                rest = Some(name.trim().to_owned());
                None
            } else {
                Some(parse_pattern(item))
            }
        })
        .collect();
    Pattern::List { items, rest }
}

fn parse_variant_pattern(source: &str) -> Option<Pattern> {
    let (head, payload) = split_variant_payload(source);
    let (path, name) = if let Some(name) = head.strip_prefix('.') {
        (None, name.trim())
    } else if let Some((path, name)) = head.rsplit_once("::") {
        (Some(path.trim().to_owned()), name.trim())
    } else {
        return None;
    };
    if !is_pattern_ident(name) {
        return None;
    }
    Some(Pattern::Variant {
        path,
        name: name.to_owned(),
        payload,
    })
}

fn split_variant_payload(source: &str) -> (&str, Option<VariantPatternPayload>) {
    if let Some(inner) = source.find('(').and_then(|open| {
        source
            .strip_suffix(')')
            .map(|_| (open, &source[open + 1..source.len() - 1]))
    }) {
        let (open, inner) = inner;
        return (
            source[..open].trim(),
            Some(VariantPatternPayload::Tuple(
                split_pattern_items(inner)
                    .into_iter()
                    .map(parse_pattern)
                    .collect(),
            )),
        );
    }
    if let Some((head, body)) = split_brace_item(source) {
        let mut rest = false;
        let fields = split_pattern_items(body)
            .into_iter()
            .filter_map(|field| {
                if field == ".." {
                    rest = true;
                    return None;
                }
                let (name, pattern) = field
                    .split_once(':')
                    .map_or((field.trim(), field.trim()), |(name, pattern)| {
                        (name.trim(), pattern.trim())
                    });
                is_pattern_ident(name)
                    .then(|| RecordPatternField::new(name, parse_pattern(pattern)))
            })
            .collect();
        return (
            head.trim(),
            Some(VariantPatternPayload::Record { fields, rest }),
        );
    }
    (source, None)
}

fn parse_record_pattern(source: &str) -> Option<Pattern> {
    let (head, body) = split_brace_item(source)?;
    if head.split_whitespace().count() > 1 {
        return None;
    }
    if head.trim().is_empty() && !body.contains(':') {
        return None;
    }
    let mut rest = false;
    let fields = split_pattern_items(body)
        .into_iter()
        .filter_map(|field| {
            if field == ".." {
                rest = true;
                return None;
            }
            let (name, pattern) = field
                .split_once(':')
                .map_or((field.trim(), field.trim()), |(name, pattern)| {
                    (name.trim(), pattern.trim())
                });
            is_pattern_ident(name).then(|| RecordPatternField::new(name, parse_pattern(pattern)))
        })
        .collect();
    Some(Pattern::Record {
        path: (!head.trim().is_empty()).then(|| head.trim().to_owned()),
        fields,
        rest,
    })
}

fn is_pattern_ident(source: &str) -> bool {
    source
        .chars()
        .all(|ch| ch.is_alphanumeric() || matches!(ch, '_'))
        && source
            .chars()
            .next()
            .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
}

fn split_pattern_items(source: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                items.push(source[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
}

fn split_brace_item(source: &str) -> Option<(&str, &str)> {
    let open = source.find('{')?;
    let close = source.rfind('}')?;
    (open < close).then(|| (source[..open].trim(), source[open + 1..close].trim()))
}
