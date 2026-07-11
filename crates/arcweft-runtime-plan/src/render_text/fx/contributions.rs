//! Cascade provenance emitted for typed `RichText` Fx applications.

use arcweft_lang_hir::{model::HirDialogue, syntax::ast::common::TextRange};
use arcweft_render_text::{
    RichTextAssignOp, RichTextCascadeLayer, RichTextSettingSource, RichTextSourceRange,
    RichTextStyleContribution,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FxInlineAssignment {
    definition: String,
    source_range: TextRange,
}

impl FxInlineAssignment {
    pub(super) fn new(definition: String, source_range: TextRange) -> Self {
        Self {
            definition,
            source_range,
        }
    }
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
            path: format!("rich_text.fx.{}", assignment.definition),
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
            value: assignment.definition.clone(),
            style_index: None,
            active: true,
            shadowed_by: None,
        })
    }));
}
