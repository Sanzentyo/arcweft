//! Effective-cascade contributions emitted by expanded decoration layers.

use arcweft_lang_hir::{
    decoration::DecorationBuilderKind, model::HirDialogue, syntax::ast::common::TextRange,
};
use arcweft_render_text::{
    RichTextAssignOp, RichTextCascadeLayer, RichTextSettingSource, RichTextSourceRange,
    RichTextStyleContribution,
};

use super::{ExpandedDecorationArgument, ExpandedDecorationLayer};

/// One expanded inline assignment before content-relative provenance is
/// projected back to the authored document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecorationInlineAssignment {
    path: String,
    value: String,
    source_range: TextRange,
}

pub(super) fn inline_assignments(
    layers: &[ExpandedDecorationLayer],
    invocation_range: TextRange,
) -> Vec<DecorationInlineAssignment> {
    layers
        .iter()
        .flat_map(|layer| layer_assignments(layer, invocation_range))
        .collect()
}

pub(crate) fn append_decoration_inline_contributions(
    target: &mut Vec<RichTextStyleContribution>,
    dialogue: &HirDialogue,
    assignments: &[DecorationInlineAssignment],
) {
    let content = dialogue.content();
    target.extend(assignments.iter().filter_map(|assignment| {
        let source_range = content.source_range(assignment.source_range)?;
        Some(RichTextStyleContribution {
            path: assignment.path.clone(),
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
            value: assignment.value.clone(),
            style_index: None,
            active: true,
            shadowed_by: None,
        })
    }));
}

fn layer_assignments(
    layer: &ExpandedDecorationLayer,
    invocation_range: TextRange,
) -> Vec<DecorationInlineAssignment> {
    match layer.builder {
        DecorationBuilderKind::Em | DecorationBuilderKind::Strong => {
            vec![assignment(
                "rich_text.text.style",
                if layer.builder == DecorationBuilderKind::Em {
                    "em"
                } else {
                    "strong"
                },
                invocation_range,
            )]
        }
        DecorationBuilderKind::Color
        | DecorationBuilderKind::Font
        | DecorationBuilderKind::Size => scalar_assignment(layer, invocation_range)
            .into_iter()
            .collect(),
        DecorationBuilderKind::Style => style_assignments(layer, invocation_range),
        DecorationBuilderKind::Layout => layout_assignments(layer, invocation_range),
        DecorationBuilderKind::Transform => transform_assignments(layer, invocation_range),
        DecorationBuilderKind::Effect => effect_assignments(layer, invocation_range),
        DecorationBuilderKind::Decorate => Vec::new(),
    }
}

fn scalar_assignment(
    layer: &ExpandedDecorationLayer,
    invocation_range: TextRange,
) -> Option<DecorationInlineAssignment> {
    let argument = layer.arguments.first()?;
    let path = match layer.builder {
        DecorationBuilderKind::Color => "rich_text.text.color",
        DecorationBuilderKind::Font => "rich_text.text.font",
        DecorationBuilderKind::Size => "rich_text.text.size",
        _ => return None,
    };
    Some(assignment(
        path,
        &argument.value,
        argument.invocation_range.unwrap_or(invocation_range),
    ))
}

fn style_assignments(
    layer: &ExpandedDecorationLayer,
    invocation_range: TextRange,
) -> Vec<DecorationInlineAssignment> {
    let Some(selector) = layer.selector.as_deref() else {
        return Vec::new();
    };
    let value = if layer.attrs.is_empty() {
        selector
    } else {
        layer.attrs.as_str()
    };
    vec![assignment(
        "rich_text.text.style",
        value,
        first_argument_range(layer, invocation_range),
    )]
}

fn layout_assignments(
    layer: &ExpandedDecorationLayer,
    invocation_range: TextRange,
) -> Vec<DecorationInlineAssignment> {
    let Some(selector) = layer.selector.as_deref() else {
        return Vec::new();
    };
    let mut assignments = Vec::new();
    match selector {
        "vertical_rl" | "vertical" | "vertical_lr" | "horizontal_tb" => {
            assignments.push(assignment(
                "rich_text.layout.writing_mode",
                selector,
                invocation_range,
            ));
        }
        "ruby_over" | "ruby_under" | "ruby_inter_character" => {
            assignments.push(assignment(
                "rich_text.ruby.position",
                selector.trim_start_matches("ruby_"),
                invocation_range,
            ));
        }
        _ => {}
    }
    assignments.extend(layer.arguments.iter().filter_map(|argument| {
        let path = match argument.name.as_str() {
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
        Some(argument_assignment(path, argument, invocation_range))
    }));
    assignments
}

fn transform_assignments(
    layer: &ExpandedDecorationLayer,
    invocation_range: TextRange,
) -> Vec<DecorationInlineAssignment> {
    let Some(selector) = layer.selector.as_deref() else {
        return Vec::new();
    };
    let mut assignments = vec![assignment(
        "rich_text.transform.kind",
        selector,
        invocation_range,
    )];
    assignments.extend(layer.arguments.iter().map(|argument| {
        argument_assignment(
            &format!("rich_text.transform.{}", argument.name),
            argument,
            invocation_range,
        )
    }));
    assignments
}

fn effect_assignments(
    layer: &ExpandedDecorationLayer,
    invocation_range: TextRange,
) -> Vec<DecorationInlineAssignment> {
    let Some(selector) = layer.selector.as_deref() else {
        return Vec::new();
    };
    let mut assignments = vec![assignment("rich_text.effect", selector, invocation_range)];
    assignments.extend(layer.arguments.iter().map(|argument| {
        argument_assignment(
            &format!("rich_text.effect.{selector}.{}", argument.name),
            argument,
            invocation_range,
        )
    }));
    assignments
}

fn first_argument_range(layer: &ExpandedDecorationLayer, fallback: TextRange) -> TextRange {
    layer
        .arguments
        .iter()
        .find_map(|argument| argument.invocation_range)
        .unwrap_or(fallback)
}

fn argument_assignment(
    path: &str,
    argument: &ExpandedDecorationArgument,
    fallback: TextRange,
) -> DecorationInlineAssignment {
    assignment(
        path,
        &argument.value,
        argument.invocation_range.unwrap_or(fallback),
    )
}

fn assignment(path: &str, value: &str, source_range: TextRange) -> DecorationInlineAssignment {
    DecorationInlineAssignment {
        path: path.to_owned(),
        value: value.to_owned(),
        source_range,
    }
}
