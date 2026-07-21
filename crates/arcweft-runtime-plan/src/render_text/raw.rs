use std::collections::BTreeMap;
use std::ops::Range;

use arcweft_lang_hir::model::HirDialogue;
use arcweft_lang_hir::syntax::ast::common::TextRange;
use arcweft_lang_hir::syntax::expr::{CallArg, Expr};
use arcweft_render_text::{
    RichTextAssignOp, RichTextSettingSource, RichTextSourceRange, RichTextStyleContribution,
};

use crate::labels::expr_label;

use super::helpers::style_call_name;

pub(crate) fn style_assignment_paths(path: &str, expr: &Expr) -> Vec<(String, String)> {
    let nested = nested_style_assignment_paths(path, expr);
    if nested.is_empty() {
        vec![(path.to_owned(), expr_label(expr))]
    } else {
        nested
    }
}

pub(crate) struct RawStyleAssignment {
    pub(crate) path: String,
    pub(crate) value: String,
    pub(crate) value_range: Option<Range<usize>>,
}

pub(crate) fn style_assignments_from_raw(path: &str, raw: &str) -> Vec<RawStyleAssignment> {
    let raw = raw.trim();
    style_assignments_from_trimmed_raw(path, raw, 0)
}

fn style_assignments_from_trimmed_raw(
    path: &str,
    raw: &str,
    raw_offset: usize,
) -> Vec<RawStyleAssignment> {
    let Some((callee, _args)) = raw_call_parts(raw) else {
        return vec![RawStyleAssignment {
            path: path.to_owned(),
            value: raw.to_owned(),
            value_range: Some(raw_offset..raw_offset + raw.len()),
        }];
    };
    let Some(call_args_range) = raw_call_args_source_range(raw) else {
        return vec![RawStyleAssignment {
            path: path.to_owned(),
            value: raw.to_owned(),
            value_range: Some(raw_offset..raw_offset + raw.len()),
        }];
    };
    match callee.rsplit('.').next().unwrap_or(callee) {
        "text_style" | "dialogue_style" | "style" | "rich_text_style" => {
            raw_call_arg_ranges(&raw[call_args_range.clone()])
                .into_iter()
                .flat_map(|arg| {
                    let raw_arg_range =
                        call_args_range.start + arg.start..call_args_range.start + arg.end;
                    let trimmed_arg_range = trim_raw_range(raw, raw_arg_range);
                    if let Some((name, value)) =
                        split_named_raw_range(raw, trimmed_arg_range.clone())
                    {
                        let child = format!("{path}.{name}");
                        style_assignments_from_trimmed_raw(
                            &child,
                            &raw[value.clone()],
                            raw_offset + value.start,
                        )
                    } else {
                        style_assignments_from_trimmed_raw(
                            path,
                            &raw[trimmed_arg_range.clone()],
                            raw_offset + trimmed_arg_range.start,
                        )
                    }
                })
                .collect()
        }
        "ruby_style" | "layout_style" => raw_call_arg_ranges(&raw[call_args_range.clone()])
            .into_iter()
            .filter_map(|arg| {
                let raw_arg_range =
                    call_args_range.start + arg.start..call_args_range.start + arg.end;
                let trimmed_arg_range = trim_raw_range(raw, raw_arg_range);
                let (name, value) = split_named_raw_range(raw, trimmed_arg_range)?;
                Some(RawStyleAssignment {
                    path: format!("{path}.{name}"),
                    value: raw[value.clone()].trim().to_owned(),
                    value_range: Some(raw_offset + value.start..raw_offset + value.end),
                })
            })
            .collect(),
        _ => vec![RawStyleAssignment {
            path: path.to_owned(),
            value: raw.to_owned(),
            value_range: Some(raw_offset..raw_offset + raw.len()),
        }],
    }
}

fn trim_raw_range(source: &str, range: Range<usize>) -> Range<usize> {
    let raw = &source[range.clone()];
    let leading = raw.len() - raw.trim_start().len();
    let trailing = raw.len() - raw.trim_end().len();
    range.start + leading..range.end - trailing
}

fn split_named_raw_range(source: &str, range: Range<usize>) -> Option<(&str, Range<usize>)> {
    let raw = &source[range.clone()];
    let equals = find_top_level_raw_punctuation(raw, '=')?;
    let name = raw[..equals].trim();
    if name.is_empty() {
        return None;
    }
    let value = &raw[equals + '='.len_utf8()..];
    let value_leading = value.len() - value.trim_start().len();
    let value_trailing = value.len() - value.trim_end().len();
    Some((
        name,
        range.start + equals + '='.len_utf8() + value_leading..range.end - value_trailing,
    ))
}

pub(crate) fn source_with_relative_range(
    source: &RichTextSettingSource,
    value_range: Option<Range<usize>>,
) -> RichTextSettingSource {
    match (source, value_range) {
        (
            RichTextSettingSource::SourceFile {
                item_id,
                public_id,
                range: Some(source_range),
            },
            Some(value_range),
        ) => RichTextSettingSource::SourceFile {
            item_id: item_id.clone(),
            public_id: public_id.clone(),
            range: Some(RichTextSourceRange {
                start: source_range.start + value_range.start,
                end: source_range.start + value_range.end,
            }),
        },
        _ => source.clone(),
    }
}

fn raw_call_parts(raw: &str) -> Option<(&str, &str)> {
    let open = find_top_level_raw_punctuation(raw, '(')?;
    let close = raw.rfind(')')?;
    (close > open && raw[close + ')'.len_utf8()..].trim().is_empty())
        .then(|| (raw[..open].trim(), raw[open + '('.len_utf8()..close].trim()))
}

pub(crate) fn speaker_preset_arg_ranges(
    expr_source: Option<&str>,
    expr_range: Option<&TextRange>,
) -> BTreeMap<String, RichTextSourceRange> {
    let (Some(expr_source), Some(expr_range)) = (expr_source, expr_range) else {
        return BTreeMap::new();
    };
    let Some(args_range) = raw_call_args_source_range(expr_source) else {
        return BTreeMap::new();
    };
    let raw_args_source = &expr_source[args_range.clone()];
    raw_call_arg_ranges(raw_args_source)
        .into_iter()
        .filter_map(|arg_range| {
            let arg_text = &raw_args_source[arg_range.clone()];
            let leading = arg_text.len() - arg_text.trim_start().len();
            let trimmed = arg_text.trim();
            if trimmed.is_empty() {
                return None;
            }
            let equals = find_top_level_raw_punctuation(trimmed, '=')?;
            let name = trimmed[..equals].trim();
            let value = &trimmed[equals + '='.len_utf8()..];
            let value_leading = value.len() - value.trim_start().len();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            let value_start = args_range.start
                + arg_range.start
                + leading
                + equals
                + '='.len_utf8()
                + value_leading;
            Some((
                name.to_owned(),
                RichTextSourceRange {
                    start: expr_range.start() + value_start,
                    end: expr_range.start() + value_start + value.len(),
                },
            ))
        })
        .collect()
}

fn raw_call_args_source_range(raw: &str) -> Option<Range<usize>> {
    let open = find_top_level_raw_punctuation(raw, '(')?;
    let close = raw.rfind(')')?;
    (close > open && raw[close + ')'.len_utf8()..].trim().is_empty())
        .then(|| open + '('.len_utf8()..close)
}

fn raw_call_arg_ranges(source: &str) -> Vec<Range<usize>> {
    split_top_level_raw_ranges(source, ',')
        .into_iter()
        .filter(|range| !source[range.clone()].trim().is_empty())
        .collect()
}

fn split_top_level_raw_ranges(source: &str, delimiter: char) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            _ if ch == delimiter && !in_string && depth == 0 => {
                ranges.push(start..offset);
                start = offset + ch.len_utf8();
            }
            _ => {}
        }
    }
    ranges.push(start..source.len());
    ranges
}

pub(crate) fn find_top_level_raw_punctuation(source: &str, needle: char) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            _ if ch == needle && !in_string && depth == 0 => return Some(offset),
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn nested_style_assignment_paths(path: &str, expr: &Expr) -> Vec<(String, String)> {
    let Expr::Call(call) = expr else {
        return Vec::new();
    };
    match style_call_name(call.callee()) {
        Some("text_style" | "dialogue_style" | "style" | "rich_text_style") => call
            .args()
            .iter()
            .flat_map(|arg| match arg {
                CallArg::Named { name, value } => {
                    let child = format!("{path}.{name}");
                    style_assignment_paths(&child, value)
                }
                CallArg::Positional(value) => style_assignment_paths(path, value),
                CallArg::Spread { .. } => Vec::new(),
            })
            .collect(),
        Some("ruby_style" | "layout_style") => call
            .args()
            .iter()
            .filter_map(|arg| match arg {
                CallArg::Named { name, value } => {
                    Some((format!("{path}.{name}"), expr_label(value)))
                }
                CallArg::Positional(_) | CallArg::Spread { .. } => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn mark_shadowed_style_contributions(contributions: &mut [RichTextStyleContribution]) {
    let mut latest_by_path = BTreeMap::<String, usize>::new();
    for index in 0..contributions.len() {
        if !contributions[index].active || contributions[index].op != RichTextAssignOp::Replace {
            continue;
        }
        if let Some(previous) = latest_by_path.insert(contributions[index].path.clone(), index) {
            contributions[previous].active = false;
            contributions[previous].shadowed_by = Some(index);
        }
    }
}

pub(crate) fn dialogue_option_source(
    dialogue: &HirDialogue,
    range: Option<RichTextSourceRange>,
) -> RichTextSettingSource {
    RichTextSettingSource::SourceFile {
        item_id: dialogue.id().map(|id| id.body().to_owned()),
        public_id: dialogue.text_key().map(|id| id.body().to_owned()),
        range,
    }
}

pub(crate) fn source_file(
    item_id: Option<String>,
    range: Option<RichTextSourceRange>,
) -> RichTextSettingSource {
    RichTextSettingSource::SourceFile {
        public_id: item_id.clone(),
        item_id,
        range,
    }
}

pub(crate) fn style_assignment_source(
    item_id: Option<&str>,
    body_absolute_start: Option<usize>,
    body_relative_range: Range<usize>,
) -> RichTextSettingSource {
    let range = body_absolute_start.map(|start| RichTextSourceRange {
        start: start + body_relative_range.start,
        end: start + body_relative_range.end,
    });
    source_file(item_id.map(str::to_owned), range)
}

pub(crate) fn source_range(range: &TextRange) -> RichTextSourceRange {
    RichTextSourceRange {
        start: range.start(),
        end: range.end(),
    }
}
