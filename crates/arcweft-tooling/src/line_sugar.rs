use arcweft_lang_syntax::{
    ast::{common::TextRange, dialogue::SpeakerLine},
    cst::CstLineEvents,
};
use std::collections::BTreeSet;

use crate::dialogue_sugar::{
    DialogueSugarContext, DialogueSugarMode, dialogue_text_canonical_edits,
};
use crate::edit::{SourceEditOverlay, apply_text_edits};
use crate::model::{TextEdit, ToolingError};
use crate::util::is_identifier;

pub(crate) fn await_question_edit(
    source: &str,
    line: &str,
    base: usize,
    overlay: &mut SourceEditOverlay,
) -> Result<Option<TextEdit>, ToolingError> {
    let leading = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    if trimmed.strip_prefix("await? ").is_none() {
        return Ok(None);
    }
    let rest_start = base + leading + "await? ".len();
    let rest = overlay.rewrite_range(source, rest_start..base + line.len())?;
    Ok(Some(TextEdit {
        start: base + leading,
        end: base + line.len(),
        replacement: format!("try await {rest}"),
    }))
}

pub(crate) fn speaker_line_edit(
    source: &str,
    line: &SpeakerLine,
    speaker_presets: &BTreeSet<String>,
    character_aliases: &BTreeSet<String>,
    overlay: &mut SourceEditOverlay,
) -> Result<Option<TextEdit>, ToolingError> {
    let surface = line.surface();
    let Some(content_range) = surface.inline_content_range() else {
        return Ok(None);
    };
    let base_name = line.speaker();
    if !is_identifier(base_name) {
        return Ok(None);
    }
    let text = source_text_for_typed_range(source, content_range)?;
    let text = canonical_dialogue_text_for_speaker_line(text, content_range)?;
    let args = surface
        .arguments_range()
        .map(|range| overlay.rewrite_range(source, range.as_range()))
        .transpose()?;
    let callee = if speaker_presets.contains(base_name) {
        args.map_or_else(
            || base_name.to_owned(),
            |args| format!("{base_name}({args})"),
        )
    } else if args.is_some() || character_aliases.contains(base_name) {
        args.map_or_else(
            || format!("{base_name}.say()"),
            |args| format!("{base_name}.say({args})"),
        )
    } else {
        format!("{base_name}.say()")
    };
    let line_range = surface.source_line_range();
    Ok(Some(TextEdit {
        start: surface.head_range().start(),
        end: line_range.end(),
        replacement: format!("{callee}[{text}]"),
    }))
}

fn canonical_dialogue_text_for_speaker_line(
    text: &str,
    source_range: TextRange,
) -> Result<String, ToolingError> {
    let edits = dialogue_text_canonical_edits(
        text,
        DialogueSugarMode::All,
        &DialogueSugarContext::default(),
    );
    apply_text_edits(text, &edits).map_err(|source| ToolingError::DialogueCanonicalization {
        start: source_range.start(),
        end: source_range.end(),
        source: Box::new(source),
    })
}

fn source_text_for_typed_range(source: &str, range: TextRange) -> Result<&str, ToolingError> {
    if range.start() > range.end() || range.end() > source.len() {
        return Err(ToolingError::RangeOutOfBounds {
            start: range.start(),
            end: range.end(),
            len: source.len(),
        });
    }
    source
        .get(range.as_range())
        .ok_or(ToolingError::InvalidCharBoundary {
            start: range.start(),
            end: range.end(),
        })
}

pub(crate) fn closing_brace_insert(lines: &CstLineEvents, with_start: usize) -> Option<TextEdit> {
    let index = lines.iter().position(|line| line.start() == with_start)?;
    let line = lines.get(index)?;
    let indent = leading_whitespace(line.text());
    let mut last_body = line;
    for candidate in lines.iter().skip(index + 1) {
        if candidate.trimmed().is_empty() {
            last_body = candidate;
            continue;
        }
        if leading_whitespace(candidate.text()).len() <= indent.len() {
            break;
        }
        last_body = candidate;
    }
    let insert_at = last_body.end();
    Some(TextEdit {
        start: insert_at,
        end: insert_at,
        replacement: format!("\n{indent}}}"),
    })
}

fn leading_whitespace(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}
