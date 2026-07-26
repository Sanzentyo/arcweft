use std::collections::BTreeSet;

use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        items::{Attribute, Item},
    },
    source::ParsedSource,
    text::find_dialogue_tag_boundary,
};
use arcweft_presentation::rich_text::{RichTextTagFamily, inferred_tag_family};

use crate::{dialogue_content::visit_dialogue_contents, model::TextEdit};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RichTextCanonicalizationContext {
    text_proxy_types: BTreeSet<String>,
}

impl RichTextCanonicalizationContext {
    pub(crate) fn from_parsed(parsed: &ParsedSource) -> Self {
        Self {
            text_proxy_types: collect_text_proxy_type_names(parsed),
        }
    }
}

pub(crate) fn rich_text_canonical_edits(
    parsed: &ParsedSource,
    context: &RichTextCanonicalizationContext,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    visit_dialogue_contents(parsed, |site| {
        edits.extend(
            canonical_content_edits(site.raw(), context)
                .into_iter()
                .filter_map(|edit| {
                    let source_range = site.source_range(TextRange::new(edit.start, edit.end))?;
                    Some(TextEdit {
                        start: source_range.start(),
                        end: source_range.end(),
                        replacement: edit.replacement,
                    })
                }),
        );
    });
    edits
}

fn canonical_content_edits(raw: &str, context: &RichTextCanonicalizationContext) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let mut cursor = 0;
    let mut inferred_span_stack = Vec::new();
    while cursor < raw.len() {
        let Some(ch) = raw[cursor..].chars().next() else {
            break;
        };
        match ch {
            '\\' => {
                cursor += ch.len_utf8();
                if let Some(escaped) = raw[cursor..].chars().next() {
                    cursor += escaped.len_utf8();
                }
            }
            '[' => {
                if let Some((end, replacement)) =
                    inferred_tag_edit(raw, cursor, &mut inferred_span_stack, context)
                {
                    edits.push(TextEdit {
                        start: cursor,
                        end,
                        replacement,
                    });
                    cursor = end;
                } else if let Some(end) = raw_span_end(raw, cursor) {
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            _ => cursor += ch.len_utf8(),
        }
    }
    edits
}

fn inferred_tag_edit(
    raw: &str,
    start: usize,
    inferred_span_stack: &mut Vec<Option<&'static str>>,
    context: &RichTextCanonicalizationContext,
) -> Option<(usize, String)> {
    let boundary = find_dialogue_tag_boundary(raw, start)?;
    let close = boundary.close();
    let inside = raw.get(start + '['.len_utf8()..close)?.trim();
    let end = boundary.end();
    if inside == "/" {
        return inferred_span_stack
            .pop()
            .map(|family| family.map_or_else(String::new, |family| format!("[/{family}]")))
            .map(|replacement| (end, replacement));
    }
    if !inside.starts_with('.') || inside.len() <= 1 {
        return None;
    }
    let (selector, attrs) = split_tag_head(inside);
    let selector_name = selector.trim_start_matches('.');
    let Some(family) = inferred_rich_text_family(selector_name, attrs, context) else {
        inferred_span_stack.push(None);
        return Some((end, format!("[mark {selector}]")));
    };
    inferred_span_stack.push(Some(family));
    let attrs = canonical_object_attrs(selector_name, attrs, context)
        .filter(|_| family == "object")
        .unwrap_or_else(|| attrs.to_owned());
    let replacement = if attrs.is_empty() {
        format!("[{family} {selector}]")
    } else {
        format!("[{family} {selector} {attrs}]")
    };
    Some((end, replacement))
}

fn split_tag_head(source: &str) -> (&str, &str) {
    let mut parts = source.splitn(2, char::is_whitespace);
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default().trim(),
    )
}

fn inferred_rich_text_family(
    selector: &str,
    attrs: &str,
    context: &RichTextCanonicalizationContext,
) -> Option<&'static str> {
    if inferred_text_proxy_type(selector, attrs, context).is_some() {
        return Some("object");
    }
    match inferred_tag_family(selector, attrs) {
        Some(RichTextTagFamily::Style) => Some("style"),
        Some(RichTextTagFamily::Layout) => Some("layout"),
        Some(RichTextTagFamily::Transform) => Some("transform"),
        Some(RichTextTagFamily::Effect) => Some("effect"),
        Some(RichTextTagFamily::Marker) | None => None,
    }
}

fn inferred_text_proxy_type<'a>(
    selector: &'a str,
    attrs: &'a str,
    context: &'a RichTextCanonicalizationContext,
) -> Option<&'a str> {
    text_proxy_type_attr(attrs)
        .filter(|name| context.text_proxy_types.contains(*name))
        .or_else(|| {
            context
                .text_proxy_types
                .contains(selector)
                .then_some(selector)
        })
}

fn canonical_object_attrs(
    selector: &str,
    attrs: &str,
    context: &RichTextCanonicalizationContext,
) -> Option<String> {
    let proxy_type = inferred_text_proxy_type(selector, attrs, context)?;
    if text_proxy_type_attr(attrs).is_some() {
        Some(attrs.to_owned())
    } else if attrs.trim().is_empty() {
        Some(format!("type={proxy_type}"))
    } else {
        Some(format!("type={proxy_type} {}", attrs.trim()))
    }
}

fn text_proxy_type_attr(attrs: &str) -> Option<&str> {
    find_tag_attr(attrs, "type")
        .or_else(|| find_tag_attr(attrs, "struct"))
        .or_else(|| find_tag_attr(attrs, "proxy"))
}

fn find_tag_attr<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    attrs.split_whitespace().find_map(|part| {
        let (name, value) = part.split_once('=')?;
        (name == key).then(|| value.trim_matches('"'))
    })
}

fn collect_text_proxy_type_names(parsed: &ParsedSource) -> BTreeSet<String> {
    parsed
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item)
                if item.attrs().iter().any(is_text_proxy_attribute) && !item.name().is_empty() =>
            {
                Some(item.name().to_owned())
            }
            _ => None,
        })
        .collect()
}

fn is_text_proxy_attribute(attr: &Attribute) -> bool {
    matches!(attr.name(), "text_proxy" | "rich_text_proxy")
}

fn raw_span_end(raw: &str, start: usize) -> Option<usize> {
    let body_start = start + "[raw]".len();
    raw.get(start..)?.starts_with("[raw]").then_some(())?;
    let close = raw.get(body_start..)?.find("[/raw]")?;
    Some(body_start + close + "[/raw]".len())
}
