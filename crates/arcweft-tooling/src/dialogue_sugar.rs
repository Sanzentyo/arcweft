use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        items::{Attribute, Item},
    },
    source::ParsedSource,
    text::{RichTextTagFamily, find_dialogue_tag_boundary, inferred_rich_text_tag_family},
};
use std::collections::BTreeSet;

use crate::{dialogue_content::visit_dialogue_contents, model::TextEdit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DialogueSugarMode {
    All,
    RichTextOnly,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DialogueSugarContext {
    text_proxy_types: BTreeSet<String>,
}

impl DialogueSugarContext {
    pub(crate) fn from_parsed(parsed: &ParsedSource) -> Self {
        Self {
            text_proxy_types: collect_text_proxy_type_names(parsed),
        }
    }
}

pub(crate) fn dialogue_text_sugar_edits(
    parsed: &ParsedSource,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    visit_dialogue_contents(parsed, |site| {
        edits.extend(
            dialogue_text_canonical_edits(site.raw(), mode, context)
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

pub(crate) fn dialogue_text_canonical_edits(
    raw: &str,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) -> Vec<TextEdit> {
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
            '｜' if mode == DialogueSugarMode::All => {
                if let Some((end, replacement)) = natural_ruby_edit(raw, cursor) {
                    edits.push(TextEdit {
                        start: cursor,
                        end,
                        replacement,
                    });
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            '|' if mode == DialogueSugarMode::All => {
                if let Some((end, replacement)) = compact_ruby_edit(raw, cursor) {
                    edits.push(TextEdit {
                        start: cursor,
                        end,
                        replacement,
                    });
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            '$' if mode == DialogueSugarMode::All => {
                if let Some((end, replacement)) = dollar_expr_edit(raw, cursor) {
                    edits.push(TextEdit {
                        start: cursor,
                        end,
                        replacement,
                    });
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            '[' => {
                if let Some((end, replacement)) =
                    bracket_dialogue_edit(raw, cursor, &mut inferred_span_stack, mode, context)
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

fn natural_ruby_edit(raw: &str, start: usize) -> Option<(usize, String)> {
    let after_marker = start + '｜'.len_utf8();
    let tail = raw.get(after_marker..)?;
    let open = tail.find('《')?;
    let base_text = &tail[..open];
    let ruby_start = after_marker + open + '《'.len_utf8();
    let ruby_tail = raw.get(ruby_start..)?;
    let close = ruby_tail.find('》')?;
    let ruby = &ruby_tail[..close];
    if base_text.is_empty() || ruby.is_empty() {
        return None;
    }
    Some((
        ruby_start + close + '》'.len_utf8(),
        format!("|[{base_text}]({ruby})"),
    ))
}

fn compact_ruby_edit(raw: &str, start: usize) -> Option<(usize, String)> {
    let after_marker = start + '|'.len_utf8();
    let tail = raw.get(after_marker..)?;
    if tail.starts_with('[') {
        return None;
    }
    let open = tail.find('{')?;
    let base_text = &tail[..open];
    if base_text.is_empty()
        || base_text
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '[' | ']' | '{' | '}' | '#' | '|'))
    {
        return None;
    }
    let ruby_start = after_marker + open + '{'.len_utf8();
    let ruby_tail = raw.get(ruby_start..)?;
    let close = ruby_tail.find('}')?;
    let ruby = &ruby_tail[..close];
    if ruby.is_empty() {
        return None;
    }
    Some((
        ruby_start + close + '}'.len_utf8(),
        format!("|[{base_text}]({ruby})"),
    ))
}

fn dollar_expr_edit(raw: &str, start: usize) -> Option<(usize, String)> {
    let expr_start = start + "$(".len();
    let end = balanced_close(raw, expr_start, '(', ')')?;
    let expr = raw.get(expr_start..end - ')'.len_utf8())?;
    Some((end, format!("#[{expr}]")))
}

fn bracket_dialogue_edit(
    raw: &str,
    start: usize,
    inferred_span_stack: &mut Vec<Option<&'static str>>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) -> Option<(usize, String)> {
    if mode == DialogueSugarMode::All
        && let Some(body) = raw.get(start..)?.strip_prefix("[raw:")
    {
        let close_relative = raw_colon_close(body)?;
        let raw_body = body[..close_relative].trim_start();
        return Some((
            start + "[raw:".len() + close_relative + ']'.len_utf8(),
            format!("[raw]{raw_body}[/raw]"),
        ));
    }
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
    if mode == DialogueSugarMode::All && inside == "page" {
        return Some((end, "[p]".to_owned()));
    }
    if mode == DialogueSugarMode::All && inside == "wait" {
        return Some((end, "[l]".to_owned()));
    }
    if mode == DialogueSugarMode::All && inside == "nl" {
        return Some((end, "[r]".to_owned()));
    }
    if mode == DialogueSugarMode::All
        && let Some(rest) = inside.strip_prefix("! ")
    {
        return Some((end, format!("[call {rest}]")));
    }
    if inside.starts_with('.') && inside.len() > 1 {
        let (selector, attrs) = split_dialogue_tag_head(inside);
        let selector_name = selector.trim_start_matches('.');
        let family = inferred_rich_text_family(selector_name, attrs, context);
        if let Some(family) = family {
            inferred_span_stack.push(Some(family));
            let attrs = canonical_object_attrs(selector_name, attrs, context)
                .filter(|_| family == "object")
                .unwrap_or_else(|| attrs.to_owned());
            let replacement = if attrs.is_empty() {
                format!("[{family} {selector}]")
            } else {
                format!("[{family} {selector} {attrs}]")
            };
            return Some((end, replacement));
        }
        inferred_span_stack.push(None);
        return Some((end, format!("[mark {selector}]")));
    }
    if mode == DialogueSugarMode::All
        && let Some(time) = inside.strip_prefix("w ")
        && !time.contains('=')
    {
        return Some((end, format!("[w time={}]", time.trim())));
    }
    if mode == DialogueSugarMode::All
        && let Some((tag, body)) = inside.split_once(':')
    {
        let body = body.trim_start();
        if tag == "em" || tag == "strong" {
            return Some((end, format!("[{tag}]{body}[/{tag}]")));
        }
        if let Some(color) = tag.strip_prefix("color ") {
            return Some((
                end,
                format!("[color value=\"{}\"]{body}[/color]", color.trim()),
            ));
        }
    }
    if mode == DialogueSugarMode::All {
        rb_tag_edit(raw, start, inside, end)
    } else {
        None
    }
}

fn raw_colon_close(body: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut cursor = 0usize;
    while cursor < body.len() {
        let ch = body[cursor..].chars().next()?;
        match ch {
            '\\' => {
                cursor += ch.len_utf8();
                if let Some(escaped) = body[cursor..].chars().next() {
                    cursor += escaped.len_utf8();
                }
            }
            '[' => {
                depth += 1;
                cursor += ch.len_utf8();
            }
            ']' if depth == 0 => return Some(cursor),
            ']' => {
                depth = depth.saturating_sub(1);
                cursor += ch.len_utf8();
            }
            _ => cursor += ch.len_utf8(),
        }
    }
    None
}

fn split_dialogue_tag_head(source: &str) -> (&str, &str) {
    let mut parts = source.splitn(2, char::is_whitespace);
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default().trim(),
    )
}

fn inferred_rich_text_family(
    selector: &str,
    attrs: &str,
    context: &DialogueSugarContext,
) -> Option<&'static str> {
    if inferred_text_proxy_type(selector, attrs, context).is_some() {
        return Some("object");
    }
    match inferred_rich_text_tag_family(selector, attrs) {
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
    context: &'a DialogueSugarContext,
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
    context: &DialogueSugarContext,
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

fn rb_tag_edit(raw: &str, _start: usize, inside: &str, open_end: usize) -> Option<(usize, String)> {
    let attrs = inside.strip_prefix("rb")?.trim();
    let ruby = ruby_attr_value(attrs)?;
    let tail = raw.get(open_end..)?;
    let close = tail.find("[/rb]")?;
    let body = raw.get(open_end..open_end + close)?;
    Some((
        open_end + close + "[/rb]".len(),
        format!("[ruby rt=\"{ruby}\"]{body}[/ruby]"),
    ))
}

fn ruby_attr_value(attrs: &str) -> Option<&str> {
    let value = attrs.trim().strip_prefix("rt")?.trim_start();
    let value = value.strip_prefix('=')?.trim_start();
    if let Some(quoted) = value.strip_prefix('"') {
        return quoted.find('"').map(|end| &quoted[..end]);
    }
    let end = value.find(char::is_whitespace).unwrap_or(value.len());
    (end > 0).then_some(&value[..end])
}

fn raw_span_end(raw: &str, start: usize) -> Option<usize> {
    let body_start = start + "[raw]".len();
    raw.get(start..)?.starts_with("[raw]").then_some(())?;
    let close = raw.get(body_start..)?.find("[/raw]")?;
    Some(body_start + close + "[/raw]".len())
}

fn balanced_close(raw: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 1_u32;
    for (relative, ch) in raw.get(start..)?.char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(start + relative + close.len_utf8());
            }
        }
    }
    None
}
