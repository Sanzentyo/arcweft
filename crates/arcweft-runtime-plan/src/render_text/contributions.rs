use std::ops::Range;

use arcweft_lang_hir::model::HirDialogue;
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_lang_syntax::ast::dialogue::{
    DialogueContent, DialogueTag, DialogueTagArg, DialogueToken,
};
use arcweft_lang_syntax::expr::Expr;
use arcweft_presentation::rich_text::{RichTextTagFamily, inferred_tag_family};
use arcweft_render_text::{
    RichTextAssignOp, RichTextCascadeLayer, RichTextSettingSource, RichTextSourceRange,
    RichTextStyle, RichTextStyleContribution,
};

use super::raw::{source_with_relative_range, style_assignment_paths, style_assignments_from_raw};
use super::tag::split_selector_attrs;

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
    let content = dialogue.content();
    target.extend(
        inline_style_assignments(content)
            .into_iter()
            .filter_map(|assignment| {
                let source_range = content.source_range(TextRange::new(
                    assignment.value_range.start,
                    assignment.value_range.end,
                ))?;
                Some(RichTextStyleContribution {
                    path: assignment.path,
                    layer: RichTextCascadeLayer::InlineSpan,
                    source: RichTextSettingSource::SourceFile {
                        item_id: dialogue.id().map(|id| id.body().to_owned()),
                        public_id: dialogue.text_key().map(|id| id.body().to_owned()),
                        range: Some(RichTextSourceRange {
                            start: source_range.start(),
                            end: source_range.end(),
                        }),
                    },
                    op: RichTextAssignOp::Replace,
                    value: assignment.value,
                    style_index: None,
                    active: true,
                    shadowed_by: None,
                })
            }),
    );
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineStyleAssignment {
    path: String,
    value: String,
    value_range: Range<usize>,
}

type InlineSelectorAssignmentBuilder =
    fn(&str, &str, Range<usize>, Range<usize>, &[DialogueTagArg]) -> Vec<InlineStyleAssignment>;

fn inline_style_assignments(content: &DialogueContent) -> Vec<InlineStyleAssignment> {
    content
        .tokens()
        .iter()
        .flat_map(|token| match token {
            DialogueToken::Tag(tag) => inline_assignments_from_tag(tag, false),
            DialogueToken::InferredTag(tag) => inline_assignments_from_tag(tag, true),
            _ => Vec::new(),
        })
        .collect()
}

fn inline_assignments_from_tag(tag: &DialogueTag, inferred: bool) -> Vec<InlineStyleAssignment> {
    if inferred {
        let Some((selector, selector_range)) = dot_selector(tag.name(), tag.name_range()) else {
            return Vec::new();
        };
        return inferred_inline_assignments(
            selector,
            tag.attrs(),
            selector_range,
            tag.attrs_range().as_range(),
            tag.arguments(),
        );
    }

    match tag.name() {
        "style" => selector_inline_assignments(tag, style_selector_inline_assignments),
        "layout" => selector_inline_assignments(tag, layout_selector_inline_assignments),
        "transform" => selector_inline_assignments(tag, transform_selector_inline_assignments),
        "effect" | "fx" => selector_inline_assignments(tag, effect_selector_inline_assignments),
        "color" | "font" | "size" | "em" | "strong" | "i" | "italic" | "oblique" | "slant" => {
            direct_inline_assignments(tag)
        }
        _ => Vec::new(),
    }
}

fn selector_inline_assignments(
    tag: &DialogueTag,
    build: InlineSelectorAssignmentBuilder,
) -> Vec<InlineStyleAssignment> {
    let (selector_source, selector_attrs) = split_selector_attrs(tag.attrs());
    let selector = selector_source.trim_start_matches('.');
    let selector_range = tag
        .arguments()
        .first()
        .and_then(DialogueTagArg::value)
        .and_then(|value| dot_selector(value.value(), value.range()))
        .map_or_else(|| tag.attrs_range().as_range(), |(_, range)| range);
    let attrs_range = argument_span(
        tag.arguments()
            .iter()
            .filter(|argument| argument.name().is_some()),
    )
    .unwrap_or_else(|| selector_range.clone());
    build(
        selector,
        selector_attrs,
        selector_range,
        attrs_range,
        tag.arguments(),
    )
}

fn inferred_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_range: Range<usize>,
    attrs_range: Range<usize>,
    arguments: &[DialogueTagArg],
) -> Vec<InlineStyleAssignment> {
    match inferred_tag_family(selector, attrs) {
        Some(RichTextTagFamily::Style) => style_selector_inline_assignments(
            selector,
            attrs,
            selector_range,
            attrs_range,
            arguments,
        ),
        Some(RichTextTagFamily::Layout) => layout_selector_inline_assignments(
            selector,
            attrs,
            selector_range,
            attrs_range,
            arguments,
        ),
        Some(RichTextTagFamily::Transform) => transform_selector_inline_assignments(
            selector,
            attrs,
            selector_range,
            attrs_range,
            arguments,
        ),
        Some(RichTextTagFamily::Effect) => effect_selector_inline_assignments(
            selector,
            attrs,
            selector_range,
            attrs_range,
            arguments,
        ),
        Some(RichTextTagFamily::Marker) | None => Vec::new(),
    }
}

fn direct_inline_assignments(tag: &DialogueTag) -> Vec<InlineStyleAssignment> {
    let scalar = matches!(tag.name(), "color" | "font" | "size");
    let (value, value_range) =
        if scalar && let Some(value) = tag.arguments().first().and_then(DialogueTagArg::value) {
            (value.value().to_owned(), value.range().as_range())
        } else {
            let value_range = if tag.attrs().is_empty() {
                tag.name_range().as_range()
            } else {
                tag.attrs_range().as_range()
            };
            let value = if tag.attrs().is_empty() {
                tag.name()
            } else {
                tag.attrs()
            }
            .trim()
            .to_owned();
            (value, value_range)
        };
    let path = match tag.name() {
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
    selector_range: Range<usize>,
    attrs_range: Range<usize>,
    _arguments: &[DialogueTagArg],
) -> Vec<InlineStyleAssignment> {
    let value = if attrs.is_empty() { selector } else { attrs }
        .trim()
        .to_owned();
    let value_range = if attrs.is_empty() {
        selector_range
    } else {
        attrs_range
    };
    vec![InlineStyleAssignment {
        path: "rich_text.text.style".to_owned(),
        value,
        value_range,
    }]
}

fn layout_selector_inline_assignments(
    selector: &str,
    _attrs: &str,
    selector_range: Range<usize>,
    _attrs_range: Range<usize>,
    arguments: &[DialogueTagArg],
) -> Vec<InlineStyleAssignment> {
    let mut assignments = Vec::new();
    match selector {
        "vertical_rl" | "vertical" | "vertical_lr" | "horizontal_tb" => {
            assignments.push(InlineStyleAssignment {
                path: "rich_text.layout.writing_mode".to_owned(),
                value: selector.to_owned(),
                value_range: selector_range.clone(),
            });
        }
        "ruby_over" | "ruby_under" | "ruby_inter_character" => {
            assignments.push(InlineStyleAssignment {
                path: "rich_text.ruby.position".to_owned(),
                value: selector.trim_start_matches("ruby_").to_owned(),
                value_range: selector_range,
            });
        }
        _ => {}
    }
    assignments.extend(
        inline_attr_assignments(arguments)
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
    _attrs: &str,
    selector_range: Range<usize>,
    _attrs_range: Range<usize>,
    arguments: &[DialogueTagArg],
) -> Vec<InlineStyleAssignment> {
    let mut assignments = vec![InlineStyleAssignment {
        path: "rich_text.transform.kind".to_owned(),
        value: selector.to_owned(),
        value_range: selector_range,
    }];
    assignments.extend(inline_attr_assignments(arguments).into_iter().map(|attr| {
        InlineStyleAssignment {
            path: format!("rich_text.transform.{}", attr.name),
            value: attr.value,
            value_range: attr.value_range,
        }
    }));
    assignments
}

fn effect_selector_inline_assignments(
    selector: &str,
    _attrs: &str,
    selector_range: Range<usize>,
    _attrs_range: Range<usize>,
    arguments: &[DialogueTagArg],
) -> Vec<InlineStyleAssignment> {
    let mut assignments = vec![InlineStyleAssignment {
        path: "rich_text.effect".to_owned(),
        value: selector.to_owned(),
        value_range: selector_range,
    }];
    assignments.extend(inline_attr_assignments(arguments).into_iter().map(|attr| {
        InlineStyleAssignment {
            path: format!("rich_text.effect.{selector}.{}", attr.name),
            value: attr.value,
            value_range: attr.value_range,
        }
    }));
    assignments
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineAttrAssignment {
    name: String,
    value: String,
    value_range: Range<usize>,
}

fn inline_attr_assignments(arguments: &[DialogueTagArg]) -> Vec<InlineAttrAssignment> {
    arguments
        .iter()
        .filter_map(|argument| {
            let value = argument.value()?;
            Some(InlineAttrAssignment {
                name: argument.name()?.to_owned(),
                value: value.value().to_owned(),
                value_range: value.range().as_range(),
            })
        })
        .collect()
}

fn dot_selector(source: &str, range: TextRange) -> Option<(&str, Range<usize>)> {
    let selector = source.strip_prefix('.')?;
    let start = range.end().checked_sub(selector.len())?;
    Some((selector, start..range.end()))
}

fn argument_span<'a>(
    mut arguments: impl Iterator<Item = &'a DialogueTagArg>,
) -> Option<Range<usize>> {
    let first = arguments.next()?.range();
    let end = arguments
        .last()
        .map_or(first.end(), |arg| arg.range().end());
    Some(first.start()..end)
}
