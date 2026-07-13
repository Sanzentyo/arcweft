use arcweft_lang_syntax::cst::{
    SyntaxNode,
    path::{CstPathRootKind, cst_path_roots},
};
use std::ops::Range;

use crate::{edit::SourceEditOverlay, model::TextEdit};

pub(crate) fn parent_path_alias_overlay(
    syntax: &SyntaxNode,
    dialogue_content_ranges: &[Range<usize>],
) -> SourceEditOverlay {
    let edits = cst_path_roots(syntax)
        .into_iter()
        .filter(|root| root.kind() == CstPathRootKind::ParentAlias)
        .filter_map(|root| {
            let range = root.name_range();
            let range = usize::from(range.start())..usize::from(range.end());
            (!dialogue_content_ranges
                .iter()
                .any(|content| content.start <= range.start && range.end <= content.end))
            .then(|| TextEdit {
                start: range.start,
                end: range.end,
                replacement: "super".to_owned(),
            })
        })
        .collect();
    SourceEditOverlay::new(edits)
}
