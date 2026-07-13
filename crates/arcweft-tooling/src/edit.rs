use crate::model::{TextEdit, ToolingEditReport, ToolingError};
use std::ops::Range;

#[derive(Clone, Debug, Eq, PartialEq)]
struct OverlayEdit {
    edit: TextEdit,
    consumed: bool,
}

/// Source-derived edits that can be composed into larger replacements.
///
/// An overlay edit is consumed only after its exact source range has been
/// rewritten successfully. Partially overlapping edits remain independent so
/// final edit validation still reports the overlap.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SourceEditOverlay {
    edits: Vec<OverlayEdit>,
}

impl SourceEditOverlay {
    pub(crate) fn new(edits: Vec<TextEdit>) -> Self {
        Self {
            edits: edits
                .into_iter()
                .map(|edit| OverlayEdit {
                    edit,
                    consumed: false,
                })
                .collect(),
        }
    }

    pub(crate) fn rewrite_range(
        &mut self,
        source: &str,
        range: Range<usize>,
    ) -> Result<String, ToolingError> {
        if range.start > range.end || range.end > source.len() {
            return Err(ToolingError::RangeOutOfBounds {
                start: range.start,
                end: range.end,
                len: source.len(),
            });
        }
        let authored = source
            .get(range.clone())
            .ok_or(ToolingError::InvalidCharBoundary {
                start: range.start,
                end: range.end,
            })?;
        let selected = self
            .edits
            .iter()
            .enumerate()
            .filter(|(_, overlay)| {
                range.start <= overlay.edit.start && overlay.edit.end <= range.end
            })
            .map(|(index, overlay)| {
                (
                    index,
                    TextEdit {
                        start: overlay.edit.start - range.start,
                        end: overlay.edit.end - range.start,
                        replacement: overlay.edit.replacement.clone(),
                    },
                )
            })
            .collect::<Vec<_>>();
        let local_edits = selected
            .iter()
            .map(|(_, edit)| edit.clone())
            .collect::<Vec<_>>();
        let rewritten = apply_text_edits(authored, &local_edits)?;
        for (index, _) in selected {
            self.edits[index].consumed = true;
        }
        Ok(rewritten)
    }

    pub(crate) fn into_unconsumed_edits(self) -> Vec<TextEdit> {
        self.edits
            .into_iter()
            .filter_map(|overlay| (!overlay.consumed).then_some(overlay.edit))
            .collect()
    }
}

/// Applies edits to source. Edits may be unsorted, but must not overlap.
pub fn apply_text_edits(source: &str, edits: &[TextEdit]) -> Result<String, ToolingError> {
    let mut sorted = edits.to_vec();
    sorted.sort_by_key(|edit| (edit.start, edit.end));
    let mut previous_end = 0;
    for edit in &sorted {
        if edit.start > edit.end || edit.end > source.len() {
            return Err(ToolingError::RangeOutOfBounds {
                start: edit.start,
                end: edit.end,
                len: source.len(),
            });
        }
        if !source.is_char_boundary(edit.start) || !source.is_char_boundary(edit.end) {
            return Err(ToolingError::InvalidCharBoundary {
                start: edit.start,
                end: edit.end,
            });
        }
        if edit.start < previous_end {
            return Err(ToolingError::OverlappingEdit {
                start: edit.start,
                end: edit.end,
            });
        }
        previous_end = edit.end;
    }
    let mut output = source.to_owned();
    for edit in sorted.iter().rev() {
        output.replace_range(edit.start..edit.end, &edit.replacement);
    }
    Ok(output)
}

pub(crate) fn report_from_edits(
    source: &str,
    mut edits: Vec<TextEdit>,
) -> Result<ToolingEditReport, ToolingError> {
    dedupe_edits(&mut edits);
    let output = apply_text_edits(source, &edits)?;
    Ok(ToolingEditReport {
        status: "ok".to_owned(),
        changed: output != source,
        edits,
        output,
        diagnostics: Vec::new(),
    })
}

pub(crate) fn edits_overlap(lhs: &TextEdit, rhs: &TextEdit) -> bool {
    lhs.start < rhs.end && rhs.start < lhs.end
}

pub(crate) fn dedupe_edits(edits: &mut Vec<TextEdit>) {
    edits.sort_by_key(|edit| (edit.start, edit.end, edit.replacement.clone()));
    edits.dedup();
}
