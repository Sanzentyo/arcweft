use arcweft_lang_syntax::{
    cst::{CstLineKind, cst_lines},
    parser::parse_source,
};
use std::ops::Range;

use crate::decl_identity::declaration_identity_edits;
use crate::dialogue_content::{collect_dialogue_content_ranges, collect_speaker_lines};
use crate::dialogue_defaults::dialogue_defaults_nested_assignment_edits;
use crate::dialogue_sugar::{DialogueSugarContext, DialogueSugarMode, dialogue_text_sugar_edits};
use crate::edit::edits_overlap;
use crate::line_sugar::{await_question_edit, closing_brace_insert, speaker_line_edit};
use crate::model::{TextEdit, ToolingError};
use crate::path_sugar::parent_path_alias_overlay;
use crate::speaker_presets::{
    collect_character_aliases, collect_speaker_preset_locals_from_typed_tree,
};

pub(crate) fn sugar_expansion_edits(source: &str) -> Result<Vec<TextEdit>, ToolingError> {
    let parsed = parse_source(source);
    let lines = cst_lines(parsed.syntax());
    let character_aliases = collect_character_aliases(&parsed);
    let speaker_presets =
        collect_speaker_preset_locals_from_typed_tree(&parsed, &character_aliases);
    let dialogue_content_ranges = collect_dialogue_content_ranges(&parsed);
    let mut path_aliases = parent_path_alias_overlay(parsed.syntax(), &dialogue_content_ranges);
    let mut edits = Vec::new();
    edits.extend(declaration_identity_edits(source, &parsed));
    edits.extend(dialogue_defaults_nested_assignment_edits(
        source,
        &parsed,
        &mut path_aliases,
    )?);

    for line in collect_speaker_lines(&parsed) {
        if let Some(edit) = speaker_line_edit(
            source,
            line,
            &speaker_presets,
            &character_aliases,
            &mut path_aliases,
        )? {
            edits.push(edit);
        }
    }

    for line in lines.iter() {
        if line.kind() == CstLineKind::Comment {
            continue;
        }
        if line_starts_inside_any_dialogue_content(
            line.start(),
            line.text(),
            &dialogue_content_ranges,
        ) {
            continue;
        }
        if line.trimmed() == "with:" {
            edits.push(TextEdit {
                start: line.end() - 1,
                end: line.end(),
                replacement: " {".to_owned(),
            });
            if let Some(close) = closing_brace_insert(&lines, line.start()) {
                edits.push(close);
            }
            continue;
        }
        if let Some(edit) =
            await_question_edit(source, line.text(), line.start(), &mut path_aliases)?
        {
            edits.push(edit);
        }
    }
    edits.extend(path_aliases.into_unconsumed_edits());
    let context = DialogueSugarContext::from_parsed(&parsed);
    for edit in dialogue_text_sugar_edits(&parsed, DialogueSugarMode::All, &context) {
        if !edits.iter().any(|existing| edits_overlap(existing, &edit)) {
            edits.push(edit);
        }
    }
    Ok(edits)
}

fn line_starts_inside_any_dialogue_content(
    line_start: usize,
    line_text: &str,
    ranges: &[Range<usize>],
) -> bool {
    let trimmed_start = line_start + line_text.len() - line_text.trim_start().len();
    ranges
        .iter()
        .any(|range| range.contains(&line_start) || range.contains(&trimmed_start))
}
