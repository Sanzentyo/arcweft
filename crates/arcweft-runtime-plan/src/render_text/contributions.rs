use std::ops::Range;

use arcweft_lang_hir::model::HirDialogue;
use arcweft_lang_hir::syntax::expr::Expr;
use arcweft_render_text::{
    RichTextAssignOp, RichTextCascadeLayer, RichTextSettingSource, RichTextSourceRange,
    RichTextStyle, RichTextStyleContribution,
};

use super::attrs::trim_quotes;
use super::raw::{source_with_relative_range, style_assignment_paths, style_assignments_from_raw};
use super::tag::{InferredTagFamily, inferred_tag_family, split_selector_attrs};

pub(crate) struct LineOptionContribution<'a> {
    pub(crate) path: &'a str,
    pub(crate) expr: &'a Expr,
    pub(crate) raw: Option<&'a str>,
    pub(crate) styles: &'a [RichTextStyle],
    pub(crate) has_policy: bool,
    pub(crate) source: RichTextSettingSource,
    pub(crate) layer: RichTextCascadeLayer,
}

pub(crate) fn append_line_option_contributions(
    target: &mut Vec<RichTextStyleContribution>,
    base_offset: &mut usize,
    input: &LineOptionContribution<'_>,
) {
    let style_index = (!input.styles.is_empty()).then_some(*base_offset);
    let active = input.has_policy || !input.styles.is_empty();
    if let Some(assignments) = input
        .raw
        .map(|raw| style_assignments_from_raw(input.path, raw))
        .filter(|assignments| !assignments.is_empty())
    {
        target.extend(
            assignments
                .into_iter()
                .map(|assignment| RichTextStyleContribution {
                    path: assignment.path,
                    layer: input.layer,
                    source: source_with_relative_range(&input.source, assignment.value_range),
                    op: RichTextAssignOp::Replace,
                    value: assignment.value,
                    style_index,
                    active,
                    shadowed_by: None,
                }),
        );
    } else {
        target.extend(
            style_assignment_paths(input.path, input.expr)
                .into_iter()
                .map(|(path, value)| RichTextStyleContribution {
                    path,
                    layer: input.layer,
                    source: input.source.clone(),
                    op: RichTextAssignOp::Replace,
                    value,
                    style_index,
                    active,
                    shadowed_by: None,
                }),
        );
    }
    *base_offset += input.styles.len();
}

pub(crate) fn append_inline_span_contributions(
    target: &mut Vec<RichTextStyleContribution>,
    dialogue: &HirDialogue,
) {
    target.extend(
        inline_style_assignments(dialogue.content().raw(), dialogue.content().range().start())
            .into_iter()
            .map(|assignment| RichTextStyleContribution {
                path: assignment.path,
                layer: RichTextCascadeLayer::InlineSpan,
                source: RichTextSettingSource::SourceFile {
                    item_id: dialogue.id().map(|id| id.body().to_owned()),
                    public_id: dialogue.text_key().map(|id| id.body().to_owned()),
                    range: Some(RichTextSourceRange {
                        start: assignment.value_range.start,
                        end: assignment.value_range.end,
                    }),
                },
                op: RichTextAssignOp::Replace,
                value: assignment.value,
                style_index: None,
                active: true,
                shadowed_by: None,
            }),
    );
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineStyleAssignment {
    path: String,
    value: String,
    value_range: Range<usize>,
}

fn inline_style_assignments(raw: &str, absolute_start: usize) -> Vec<InlineStyleAssignment> {
    inline_tag_ranges(raw)
        .into_iter()
        .flat_map(|tag_range| {
            let inside = &raw[tag_range.start + '['.len_utf8()..tag_range.end - ']'.len_utf8()];
            inline_assignments_from_tag(inside, absolute_start + tag_range.start + '['.len_utf8())
        })
        .collect()
}

fn inline_tag_ranges(raw: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_relative) = raw[cursor..].find('[') {
        let open = cursor + open_relative;
        let Some(close_relative) = raw[open + '['.len_utf8()..].find(']') else {
            break;
        };
        let close = open + '['.len_utf8() + close_relative + ']'.len_utf8();
        ranges.push(open..close);
        cursor = close;
    }
    ranges
}

fn inline_assignments_from_tag(
    inside: &str,
    inside_absolute_start: usize,
) -> Vec<InlineStyleAssignment> {
    let leading = inside.len() - inside.trim_start().len();
    let trimmed = inside.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.starts_with('!') {
        return Vec::new();
    }
    let trimmed_start = inside_absolute_start + leading;
    if trimmed.starts_with('.') {
        let (selector, attrs) = split_tag_name_attrs_for_inline(trimmed);
        let attrs_start = inline_attrs_start(trimmed, selector, trimmed_start);
        return inferred_inline_assignments(
            selector.trim_start_matches('.'),
            attrs,
            trimmed_start,
            attrs_start,
        );
    }

    let (name, attrs) = split_tag_name_attrs_for_inline(trimmed);
    let attrs_start = inline_attrs_start(trimmed, name, trimmed_start);
    match name {
        "style" => {
            selector_inline_assignments(attrs, attrs_start, style_selector_inline_assignments)
        }
        "layout" => {
            selector_inline_assignments(attrs, attrs_start, layout_selector_inline_assignments)
        }
        "transform" => {
            selector_inline_assignments(attrs, attrs_start, transform_selector_inline_assignments)
        }
        "effect" | "fx" => {
            selector_inline_assignments(attrs, attrs_start, effect_selector_inline_assignments)
        }
        "color" | "font" | "size" | "em" | "strong" | "i" | "italic" | "oblique" | "slant" => {
            direct_inline_assignments(name, attrs, trimmed_start, attrs_start)
        }
        _ => Vec::new(),
    }
}

fn split_tag_name_attrs_for_inline(source: &str) -> (&str, &str) {
    let mut parts = source.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or_default();
    let attrs = parts.next().unwrap_or_default().trim();
    (name, attrs)
}

fn inline_attrs_start(trimmed: &str, name: &str, trimmed_start: usize) -> usize {
    trimmed_start + name.len() + trimmed[name.len()..].len()
        - trimmed[name.len()..].trim_start().len()
}

fn selector_inline_assignments(
    attrs: &str,
    attrs_start: usize,
    build: fn(&str, &str, usize, usize) -> Vec<InlineStyleAssignment>,
) -> Vec<InlineStyleAssignment> {
    let (selector, selector_attrs) = split_selector_attrs(attrs);
    let selector_offset = attrs.find(selector).unwrap_or(0);
    let selector_start = attrs_start + selector_offset;
    let selector_attrs_start =
        inline_attrs_start(&attrs[selector_offset..], selector, selector_start);
    build(
        selector.trim_start_matches('.'),
        selector_attrs,
        selector_start,
        selector_attrs_start,
    )
}

fn inferred_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    match inferred_tag_family(selector, attrs) {
        Some(InferredTagFamily::Style) => {
            style_selector_inline_assignments(selector, attrs, selector_start, attrs_start)
        }
        Some(InferredTagFamily::Layout) => {
            layout_selector_inline_assignments(selector, attrs, selector_start, attrs_start)
        }
        Some(InferredTagFamily::Transform) => {
            transform_selector_inline_assignments(selector, attrs, selector_start, attrs_start)
        }
        Some(InferredTagFamily::Effect) => {
            effect_selector_inline_assignments(selector, attrs, selector_start, attrs_start)
        }
        Some(InferredTagFamily::Marker) | None => Vec::new(),
    }
}

fn direct_inline_assignments(
    name: &str,
    attrs: &str,
    name_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    let value_range = if attrs.is_empty() {
        name_start..name_start + name.len()
    } else {
        attrs_start..attrs_start + attrs.len()
    };
    let value = if attrs.is_empty() { name } else { attrs }
        .trim()
        .to_owned();
    let path = match name {
        "color" => "rich_text.text.color",
        "font" => "rich_text.text.font",
        "size" => "rich_text.text.size",
        "em" | "strong" | "i" | "italic" | "oblique" | "slant" => "rich_text.text.style",
        _ => return Vec::new(),
    };
    vec![InlineStyleAssignment {
        path: path.to_owned(),
        value,
        value_range,
    }]
}

fn style_selector_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    let value = if attrs.is_empty() { selector } else { attrs }
        .trim()
        .to_owned();
    let value_range = if attrs.is_empty() {
        selector_start..selector_start + selector.len()
    } else {
        attrs_start..attrs_start + attrs.len()
    };
    vec![InlineStyleAssignment {
        path: "rich_text.text.style".to_owned(),
        value,
        value_range,
    }]
}

fn layout_selector_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    let mut assignments = Vec::new();
    match selector {
        "vertical_rl" | "vertical" | "vertical_lr" | "horizontal_tb" => {
            assignments.push(InlineStyleAssignment {
                path: "rich_text.layout.writing_mode".to_owned(),
                value: selector.to_owned(),
                value_range: selector_start..selector_start + selector.len(),
            });
        }
        "ruby_over" | "ruby_under" | "ruby_inter_character" => {
            assignments.push(InlineStyleAssignment {
                path: "rich_text.ruby.position".to_owned(),
                value: selector.trim_start_matches("ruby_").to_owned(),
                value_range: selector_start..selector_start + selector.len(),
            });
        }
        _ => {}
    }
    assignments.extend(
        inline_attr_assignments(attrs, attrs_start)
            .into_iter()
            .filter_map(|attr| {
                let path = match attr.name.as_str() {
                    "ruby_size" | "size" if selector.starts_with("ruby_") => "rich_text.ruby.size",
                    "ruby_gap" | "gap" if selector.starts_with("ruby_") => "rich_text.ruby.gap",
                    "ruby_overhang" | "overhang" => "rich_text.ruby.overhang",
                    "ruby_collision_gap" | "collision_gap" => "rich_text.ruby.collision_gap",
                    "jlreq" | "strictness" | "kinsoku" => "rich_text.layout.jlreq",
                    "latin" | "vertical_latin" => "rich_text.layout.vertical_latin",
                    "dir" | "direction" => "rich_text.layout.direction",
                    "column_gap" | "gap" => "rich_text.layout.column_gap",
                    _ => return None,
                };
                Some(InlineStyleAssignment {
                    path: path.to_owned(),
                    value: attr.value,
                    value_range: attr.value_range,
                })
            }),
    );
    assignments
}

fn transform_selector_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    let mut assignments = vec![InlineStyleAssignment {
        path: "rich_text.transform.kind".to_owned(),
        value: selector.to_owned(),
        value_range: selector_start..selector_start + selector.len(),
    }];
    assignments.extend(
        inline_attr_assignments(attrs, attrs_start)
            .into_iter()
            .map(|attr| InlineStyleAssignment {
                path: format!("rich_text.transform.{}", attr.name),
                value: attr.value,
                value_range: attr.value_range,
            }),
    );
    assignments
}

fn effect_selector_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    let mut assignments = vec![InlineStyleAssignment {
        path: "rich_text.effect".to_owned(),
        value: selector.to_owned(),
        value_range: selector_start..selector_start + selector.len(),
    }];
    assignments.extend(
        inline_attr_assignments(attrs, attrs_start)
            .into_iter()
            .map(|attr| InlineStyleAssignment {
                path: format!("rich_text.effect.{selector}.{}", attr.name),
                value: attr.value,
                value_range: attr.value_range,
            }),
    );
    assignments
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineAttrAssignment {
    name: String,
    value: String,
    value_range: Range<usize>,
}

fn inline_attr_assignments(attrs: &str, attrs_start: usize) -> Vec<InlineAttrAssignment> {
    let mut assignments = Vec::new();
    let mut cursor = 0usize;
    for part in attrs.split_whitespace() {
        let part_start = attrs[cursor..]
            .find(part)
            .map_or(cursor, |relative| cursor + relative);
        cursor = part_start + part.len();
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        let value_start = attrs_start + part_start + name.len() + '='.len_utf8();
        assignments.push(InlineAttrAssignment {
            name: name.to_owned(),
            value: trim_quotes(value).to_owned(),
            value_range: value_start..value_start + value.len(),
        });
    }
    assignments
}
