use std::ops::Range;

use super::raw::find_top_level_raw_punctuation;

pub(crate) struct DialogueStyleBlock<'a> {
    pub(crate) source: &'a str,
    pub(crate) absolute_start: Option<usize>,
}

pub(crate) fn named_style_block<'a>(
    body: &'a str,
    body_range: Option<&arcweft_lang_hir::syntax::ast::common::TextRange>,
    name: &str,
) -> Option<DialogueStyleBlock<'a>> {
    let start = body.find(name)?;
    let open = body[start..].find('{')? + start;
    let close = matching_brace(body, open)?;
    let raw_block = &body[open + 1..close];
    let leading = raw_block.len() - raw_block.trim_start().len();
    let source = raw_block.trim();
    Some(DialogueStyleBlock {
        source,
        absolute_start: body_range.map(|range| range.start() + open + 1 + leading),
    })
}

fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source[open..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) struct StyleBlockAssignment<'a> {
    pub(crate) name: String,
    pub(crate) value: &'a str,
    pub(crate) value_range: Range<usize>,
}

struct LogicalStyleItem<'a> {
    source: &'a str,
    range: Range<usize>,
}

pub(crate) fn style_block_assignments<'a>(
    body: &'a str,
    path_prefix: Option<&str>,
) -> Vec<StyleBlockAssignment<'a>> {
    nested_style_block_assignments(body, path_prefix)
}

fn nested_style_block_assignments<'a>(
    body: &'a str,
    path_prefix: Option<&str>,
) -> Vec<StyleBlockAssignment<'a>> {
    logical_style_items(body)
        .iter()
        .flat_map(|item| style_item_assignments(body, item, path_prefix))
        .collect()
}

fn style_item_assignments<'a>(
    body: &'a str,
    item: &LogicalStyleItem<'a>,
    path_prefix: Option<&str>,
) -> Vec<StyleBlockAssignment<'a>> {
    if let Some(assignment) = split_assignment(item, path_prefix) {
        return vec![assignment];
    }
    let Some((head, nested_body, nested_start)) = split_nested_style_block(body, item) else {
        return Vec::new();
    };
    let next_prefix =
        path_prefix.map_or_else(|| head.to_owned(), |prefix| format!("{prefix}.{head}"));
    nested_style_block_assignments(nested_body, Some(&next_prefix))
        .into_iter()
        .map(|assignment| StyleBlockAssignment {
            name: assignment.name,
            value: assignment.value,
            value_range: assignment.value_range.start + nested_start
                ..assignment.value_range.end + nested_start,
        })
        .collect()
}

fn logical_style_items(body: &str) -> Vec<LogicalStyleItem<'_>> {
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in body.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            '\n' if !in_string && depth == 0 => {
                let item = trim_logical_style_item(body, start, offset);
                if !item.source.is_empty() && !item.source.starts_with("//") {
                    items.push(item);
                }
                start = offset + '\n'.len_utf8();
            }
            _ => {}
        }
    }
    let tail = trim_logical_style_item(body, start, body.len());
    if !tail.source.is_empty() && !tail.source.starts_with("//") {
        items.push(tail);
    }
    items
}

fn trim_logical_style_item(body: &str, start: usize, end: usize) -> LogicalStyleItem<'_> {
    let raw = &body[start..end];
    let leading = raw.len() - raw.trim_start().len();
    let source = raw.trim();
    LogicalStyleItem {
        source,
        range: start + leading..start + leading + source.len(),
    }
}

fn split_assignment<'a>(
    item: &LogicalStyleItem<'a>,
    path_prefix: Option<&str>,
) -> Option<StyleBlockAssignment<'a>> {
    let equals = find_top_level_raw_punctuation(item.source, '=')?;
    let name = item.source[..equals].trim();
    let value_source = &item.source[equals + '='.len_utf8()..];
    let value_trimmed_start = value_source.trim_start();
    let leading = value_source.len() - value_trimmed_start.len();
    let value = value_trimmed_start.trim_end_matches(',').trim_end();
    let value_start = item.range.start + equals + '='.len_utf8() + leading;
    (!name.is_empty() && !value.is_empty()).then_some(StyleBlockAssignment {
        name: path_prefix.map_or_else(|| name.to_owned(), |prefix| format!("{prefix}.{name}")),
        value,
        value_range: value_start..value_start + value.len(),
    })
}

fn split_nested_style_block<'a>(
    body: &'a str,
    item: &LogicalStyleItem<'a>,
) -> Option<(&'a str, &'a str, usize)> {
    let open_in_item = item.source.find('{')?;
    let close_in_item = matching_brace(item.source, open_in_item)?;
    if !item.source[close_in_item + '}'.len_utf8()..]
        .trim()
        .is_empty()
    {
        return None;
    }
    let head = item.source[..open_in_item].trim();
    if head.is_empty() || head.contains(char::is_whitespace) {
        return None;
    }
    let inner_start = item.range.start + open_in_item + '{'.len_utf8();
    let inner_end = item.range.start + close_in_item;
    Some((head, &body[inner_start..inner_end], inner_start))
}
