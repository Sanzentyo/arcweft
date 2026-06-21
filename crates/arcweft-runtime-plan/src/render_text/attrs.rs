use std::collections::BTreeMap;

use arcweft_render_text::{
    Milli, RichTextAngle, RichTextParam, parse_decimal_milli, parse_milli_token,
};

pub(crate) fn param_from_value(value: &str) -> RichTextParam {
    let value = trim_quotes(value);
    if value == "true" {
        return RichTextParam::Bool { value: true };
    }
    if value == "false" {
        return RichTextParam::Bool { value: false };
    }
    if let Some(selector) = value.strip_prefix('.') {
        return RichTextParam::Selector {
            value: format!(".{selector}"),
        };
    }
    if let Ok(parsed) = value.parse::<i64>() {
        return RichTextParam::Int { value: parsed };
    }
    if let Some(milli) = parse_param_milli(value) {
        return RichTextParam::Milli { value: milli };
    }
    RichTextParam::Raw {
        value: value.to_owned(),
    }
}

pub(crate) fn parse_param_milli(value: &str) -> Option<Milli> {
    let trimmed = value.trim();
    let numeric = trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("deg"))
        .or_else(|| trimmed.strip_suffix("ch"))
        .unwrap_or(trimmed)
        .trim();
    parse_decimal_milli(numeric)
}

pub(crate) fn angle_from_attrs(attrs: &str, name: &str) -> Option<RichTextAngle> {
    angle_from_attrs_map(&parse_attrs(attrs), name)
}

pub(crate) fn transform_angle_attr(
    attrs: &BTreeMap<String, String>,
    raw_attrs: &str,
) -> Option<RichTextAngle> {
    angle_from_attrs_map(attrs, "angle")
        .or_else(|| angle_from_attrs_map(attrs, "deg"))
        .or_else(|| positional_angle_attr(raw_attrs))
}

pub(crate) fn angle_from_attrs_map(
    attrs: &BTreeMap<String, String>,
    name: &str,
) -> Option<RichTextAngle> {
    attrs.get(name).map(|value| RichTextAngle {
        degrees: parse_milli_token(value),
    })
}

pub(crate) fn positional_angle_attr(raw_attrs: &str) -> Option<RichTextAngle> {
    raw_attrs
        .split_whitespace()
        .find(|item| !item.contains('='))
        .map(|value| RichTextAngle {
            degrees: parse_milli_token(value),
        })
}

pub(crate) fn milli_attr(attrs: &BTreeMap<String, String>, name: &str) -> Option<Milli> {
    attrs.get(name).map(|value| parse_milli_token(value))
}

pub(crate) fn parse_attrs(source: &str) -> BTreeMap<String, String> {
    source
        .split_whitespace()
        .filter_map(|item| {
            let (key, value) = item.split_once('=')?;
            Some((key.to_owned(), trim_quotes(value).to_owned()))
        })
        .collect()
}

pub(crate) fn parse_attr_args(source: &str) -> BTreeMap<String, String> {
    split_attr_items(source)
        .into_iter()
        .filter_map(|item| {
            let (key, value) = item.as_str().split_once('=')?;
            Some((key.to_owned(), trim_quotes(value).to_owned()))
        })
        .collect()
}

fn split_attr_items(source: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in source.chars() {
        match (quote, ch) {
            (Some(active), next) if next == active => {
                quote = None;
                current.push(ch);
            }
            (None, '"' | '\'') => {
                quote = Some(ch);
                current.push(ch);
            }
            (None, ',' | ';') => {
                push_attr_item(&mut items, &mut current);
            }
            (None, next) if next.is_whitespace() => {
                push_attr_item(&mut items, &mut current);
            }
            _ => current.push(ch),
        }
    }
    push_attr_item(&mut items, &mut current);
    items
}

fn push_attr_item(items: &mut Vec<String>, current: &mut String) {
    let item = current.trim();
    if !item.is_empty() {
        items.push(item.to_owned());
    }
    current.clear();
}

pub(crate) fn truthy_attr(value: &str) -> bool {
    matches!(trim_quotes(value), "true" | "yes" | "1" | "on")
}

pub(crate) fn trim_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}
