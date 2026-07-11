//! Cascade provenance emitted by expanded `RichText` Fx layers.

use arcweft_lang_hir::{model::HirDialogue, syntax::ast::common::TextRange};
use arcweft_render_text::{
    RichTextAssignOp, RichTextCascadeLayer, RichTextSettingSource, RichTextSourceRange,
    RichTextStyleContribution,
};

use super::{ExpandedFxArgument, ExpandedFxLayer, FxLayerKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FxInlineAssignment {
    path: String,
    value: String,
    source_range: TextRange,
}

pub(super) fn inline_assignments(
    layers: &[ExpandedFxLayer],
    invocation_range: TextRange,
) -> Vec<FxInlineAssignment> {
    layers
        .iter()
        .flat_map(|layer| layer_assignments(layer, invocation_range))
        .collect()
}

pub(crate) fn append_fx_inline_contributions(
    target: &mut Vec<RichTextStyleContribution>,
    dialogue: &HirDialogue,
    assignments: &[FxInlineAssignment],
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
    layer: &ExpandedFxLayer,
    invocation_range: TextRange,
) -> Vec<FxInlineAssignment> {
    match layer.kind {
        FxLayerKind::Em | FxLayerKind::Strong => vec![assignment(
            "rich_text.text.style",
            if layer.kind == FxLayerKind::Em {
                "em"
            } else {
                "strong"
            },
            invocation_range,
        )],
        FxLayerKind::Color | FxLayerKind::Font | FxLayerKind::Size => layer
            .arguments
            .first()
            .map(|argument| {
                let path = match layer.kind {
                    FxLayerKind::Color => "rich_text.text.color",
                    FxLayerKind::Font => "rich_text.text.font",
                    FxLayerKind::Size => "rich_text.text.size",
                    _ => unreachable!(),
                };
                argument_assignment(path, argument, invocation_range)
            })
            .into_iter()
            .collect(),
        FxLayerKind::Style => vec![assignment(
            "rich_text.text.style",
            layer.selector.as_deref().unwrap_or(&layer.attrs),
            invocation_range,
        )],
        FxLayerKind::Transform => {
            effect_assignments(layer, "rich_text.transform", invocation_range)
        }
        FxLayerKind::Effect => effect_assignments(layer, "rich_text.effect", invocation_range),
        FxLayerKind::Shader => effect_assignments(layer, "rich_text.shader", invocation_range),
    }
}

fn effect_assignments(
    layer: &ExpandedFxLayer,
    prefix: &str,
    invocation_range: TextRange,
) -> Vec<FxInlineAssignment> {
    let selector = layer.selector.as_deref().unwrap_or("fx");
    let mut assignments = vec![assignment(prefix, selector, invocation_range)];
    assignments.extend(layer.arguments.iter().map(|argument| {
        argument_assignment(
            &format!("{prefix}.{selector}.{}", argument.name),
            argument,
            invocation_range,
        )
    }));
    assignments
}

fn argument_assignment(
    path: &str,
    argument: &ExpandedFxArgument,
    fallback: TextRange,
) -> FxInlineAssignment {
    assignment(
        path,
        &argument.value,
        argument.invocation_range.unwrap_or(fallback),
    )
}

fn assignment(path: &str, value: &str, source_range: TextRange) -> FxInlineAssignment {
    FxInlineAssignment {
        path: path.to_owned(),
        value: value.to_owned(),
        source_range,
    }
}
