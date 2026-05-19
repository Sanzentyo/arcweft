use crate::ast::{EntityRef, Pattern, RecordPatternField, TextRange, VariantPatternPayload};
use crate::cst::{
    find_matching_punctuation, find_top_level_punctuation,
    split_last_top_level_punctuation_sequence_once, split_leading_ident,
    split_top_level_punctuation, split_top_level_punctuation_once,
};
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
    if let Some((name, ty)) = split_top_level_punctuation_once(source, ':') {
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
    if let Ok(Expr::EntityRef(crate::ast::EntityRefSyntax::Absolute(entity))) = parse_expr(source) {
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
        return parse_bracket_seq_pattern(inner);
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
        .strip_prefix("@<")
        .and_then(|value| value.strip_suffix('>'))
    {
        body
    } else {
        source.strip_prefix('@')?
    };
    (!body.trim().is_empty()).then(|| {
        EntityRef::new(
            body.trim().to_owned(),
            source.starts_with("@<"),
            TextRange::new(0, source.len()),
        )
    })
}

fn split_whole_pattern(source: &str) -> Option<(&str, &str)> {
    let (name, rest) = split_leading_ident(source)?;
    (is_pattern_ident(name)
        && !matches!(name, "mut")
        && !rest.is_empty()
        && !is_pattern_ident(rest))
    .then_some((name, rest))
}

fn parse_bracket_seq_pattern(inner: &str) -> Pattern {
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
    Pattern::BracketSeq { items, rest }
}

fn parse_variant_pattern(source: &str) -> Option<Pattern> {
    let (head, payload) = split_variant_payload(source);
    let (path, name) = if let Some(name) = head.strip_prefix('.') {
        (None, name.trim())
    } else if let Some((path, name)) =
        split_last_top_level_punctuation_sequence_once(head, &[":", ":"])
    {
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
    if let Some(open) = find_top_level_punctuation(source, '(')
        && let Some(close) = find_matching_punctuation(source, open, '(', ')')
        && source[close + ')'.len_utf8()..].trim().is_empty()
    {
        let inner = &source[open + '('.len_utf8()..close];
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
                let (name, pattern) = split_pattern_field(field);
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
            let (name, pattern) = split_pattern_field(field);
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
    split_top_level_punctuation(source, ',')
}

fn split_pattern_field(field: &str) -> (&str, &str) {
    split_top_level_punctuation_once(field, ':')
        .map_or((field.trim(), field.trim()), |(name, pattern)| {
            (name.trim(), pattern.trim())
        })
}

fn split_brace_item(source: &str) -> Option<(&str, &str)> {
    let open = find_top_level_punctuation(source, '{')?;
    let close = find_matching_punctuation(source, open, '{', '}')?;
    (source[close + '}'.len_utf8()..].trim().is_empty())
        .then(|| (source[..open].trim(), source[open + 1..close].trim()))
}
