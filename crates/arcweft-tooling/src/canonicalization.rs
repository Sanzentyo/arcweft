//! Semantic sugar canonicalization from parser ranges plus checked sema records.

use std::ops::Range;

use arcweft_lang_sema::{
    canonicalization::{CheckedCanonicalizationInventory, CheckedSpeakerLine, SpeakerLineOutcome},
    types::SpeakerLineType,
};
use arcweft_lang_syntax::{
    ast::dialogue::SpeakerLine,
    cst::{CstLineKind, cst_lines},
    parser::parse_source,
};

use crate::decl_identity::declaration_identity_edits;
use crate::dialogue_content::{collect_dialogue_content_ranges, collect_speaker_lines};
use crate::dialogue_sugar::{DialogueSugarContext, DialogueSugarMode, dialogue_text_sugar_edits};
use crate::edit::{edits_overlap, report_from_edits};
use crate::line_sugar::{await_question_edit, closing_brace_insert, speaker_line_edit};
use crate::model::{
    CanonicalizationInput, TextEdit, ToolingDiagnostic, ToolingDiagnosticKind, ToolingEditReport,
    ToolingError,
};
use crate::path_sugar::parent_path_alias_overlay;

/// Canonicalizes semantic sugar. No overload exists without checked semantic input.
pub fn canonicalize_source(
    source: &str,
    input: CanonicalizationInput<'_>,
) -> Result<ToolingEditReport, ToolingError> {
    let inventory = checked_inventory(input)?;
    verify_inventory_revision(source, inventory)?;

    let parsed = parse_source(source);
    let lines = cst_lines(parsed.syntax());
    let dialogue_content_ranges = collect_dialogue_content_ranges(&parsed);
    let mut path_aliases = parent_path_alias_overlay(parsed.syntax(), &dialogue_content_ranges);
    let mut edits = Vec::new();
    let mut diagnostics = Vec::new();

    edits.extend(declaration_identity_edits(source, &parsed));

    for line in collect_speaker_lines(&parsed) {
        match exact_speaker_record(source, inventory, line) {
            ExactSpeakerRecord::Resolved(record, classification) => {
                if let Some(edit) =
                    speaker_line_edit(source, line, &classification, &mut path_aliases)?
                {
                    edits.push(edit);
                }
                debug_assert_eq!(
                    Some(classification),
                    record
                        .resolved_type()
                        .and_then(arcweft_lang_sema::types::TypeKind::speaker_line_classification)
                );
            }
            ExactSpeakerRecord::Diagnostic(diagnostic) => diagnostics.push(diagnostic),
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

    diagnostics.sort_by(|left, right| {
        (
            left.start,
            left.end,
            left.code.as_str(),
            left.message.as_str(),
        )
            .cmp(&(
                right.start,
                right.end,
                right.code.as_str(),
                right.message.as_str(),
            ))
    });
    let mut report = report_from_edits(source, edits)?;
    if !diagnostics.is_empty() {
        report.status = String::from("partial");
        report.diagnostics = diagnostics;
    }
    Ok(report)
}

fn checked_inventory(
    input: CanonicalizationInput<'_>,
) -> Result<&CheckedCanonicalizationInventory, ToolingError> {
    match input {
        CanonicalizationInput::Checked(inventory) => Ok(inventory),
        CanonicalizationInput::Unavailable(unavailable) => {
            Err(ToolingError::SemanticDataUnavailable {
                document: unavailable.document().as_str().to_owned(),
                reason: unavailable.reason().to_owned(),
            })
        }
    }
}

fn verify_inventory_revision(
    source: &str,
    inventory: &CheckedCanonicalizationInventory,
) -> Result<(), ToolingError> {
    let expected = inventory.source();
    let actual = arcweft_source::SourceRevision::for_utf8(source);
    let actual_len = u64::try_from(source.len()).expect("an in-memory source length fits u64");
    if expected.revision() == actual && expected.source_len() == actual_len {
        return Ok(());
    }
    Err(ToolingError::StaleSemanticInventory {
        document: expected.id().as_str().to_owned(),
        expected_revision: revision_hex(expected.revision()),
        actual_revision: revision_hex(actual),
        expected_len: usize::try_from(expected.source_len())
            .expect("an accepted source document length fits usize"),
        actual_len: source.len(),
    })
}

fn revision_hex(revision: arcweft_source::SourceRevision) -> String {
    revision
        .as_bytes()
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            use core::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
            output
        })
}

enum ExactSpeakerRecord<'a> {
    Resolved(&'a CheckedSpeakerLine, SpeakerLineType),
    Diagnostic(ToolingDiagnostic),
}

fn exact_speaker_record<'a>(
    source: &str,
    inventory: &'a CheckedCanonicalizationInventory,
    line: &SpeakerLine,
) -> ExactSpeakerRecord<'a> {
    let surface = line.surface();
    let matches = inventory
        .speaker_lines()
        .iter()
        .filter(|record| {
            record.id().module() == inventory.module()
                && record.id().head_range() == surface.head_range()
        })
        .collect::<Vec<_>>();
    exact_speaker_record_from_matches(source, line, &matches)
}

fn exact_speaker_record_from_matches<'a>(
    source: &str,
    line: &SpeakerLine,
    matches: &[&'a CheckedSpeakerLine],
) -> ExactSpeakerRecord<'a> {
    let surface = line.surface();
    let [record] = matches else {
        return ExactSpeakerRecord::Diagnostic(ToolingDiagnostic::from_kind(
            &ToolingDiagnosticKind::SpeakerSurfaceInconsistent {
                reason: if matches.is_empty() {
                    "missing_record".to_owned()
                } else {
                    "duplicate_record".to_owned()
                },
            },
            surface.source_line_range().start(),
            surface.source_line_range().end(),
        ));
    };
    if !speaker_surface_is_valid(source, record.surface()) {
        return ExactSpeakerRecord::Diagnostic(ToolingDiagnostic::from_kind(
            &ToolingDiagnosticKind::SpeakerSurfaceInconsistent {
                reason: "range_invalid".to_owned(),
            },
            surface.source_line_range().start(),
            surface.source_line_range().end(),
        ));
    }
    if record.surface() != &surface {
        return ExactSpeakerRecord::Diagnostic(ToolingDiagnostic::from_kind(
            &ToolingDiagnosticKind::SpeakerSurfaceInconsistent {
                reason: "surface_mismatch".to_owned(),
            },
            surface.source_line_range().start(),
            surface.source_line_range().end(),
        ));
    }
    match record.outcome() {
        SpeakerLineOutcome::Preset { entity_kind } => {
            ExactSpeakerRecord::Resolved(record, SpeakerLineType::Preset(entity_kind.clone()))
        }
        SpeakerLineOutcome::Speaker { entity_kind } => {
            ExactSpeakerRecord::Resolved(record, SpeakerLineType::Speaker(entity_kind.clone()))
        }
        SpeakerLineOutcome::NonSpeaker => {
            ExactSpeakerRecord::Diagnostic(ToolingDiagnostic::from_kind(
                &ToolingDiagnosticKind::SpeakerExpressionNonSpeaker {
                    reference: record.reference().to_owned(),
                    resolved_type: record.resolved_type().map_or_else(
                        || "<missing>".to_owned(),
                        arcweft_lang_sema::types::TypeKind::source_label,
                    ),
                },
                surface.head_range().start(),
                surface.head_range().end(),
            ))
        }
        SpeakerLineOutcome::Unresolved | SpeakerLineOutcome::Erroneous => {
            ExactSpeakerRecord::Diagnostic(ToolingDiagnostic::from_kind(
                &ToolingDiagnosticKind::SpeakerExpressionUnresolved {
                    reference: record.reference().to_owned(),
                    state: match record.outcome() {
                        SpeakerLineOutcome::Unresolved => "unresolved".to_owned(),
                        SpeakerLineOutcome::Erroneous => "erroneous".to_owned(),
                        _ => unreachable!("matched an incomplete outcome"),
                    },
                },
                surface.head_range().start(),
                surface.head_range().end(),
            ))
        }
    }
}

#[cfg(test)]
pub(crate) fn speaker_record_diagnostic_for_matches(
    source: &str,
    line: &SpeakerLine,
    matches: &[&CheckedSpeakerLine],
) -> Option<ToolingDiagnostic> {
    match exact_speaker_record_from_matches(source, line, matches) {
        ExactSpeakerRecord::Resolved(_, _) => None,
        ExactSpeakerRecord::Diagnostic(diagnostic) => Some(diagnostic),
    }
}

fn speaker_surface_is_valid(
    source: &str,
    surface: &arcweft_lang_syntax::ast::dialogue::SpeakerLineSurface,
) -> bool {
    [
        Some(surface.source_line_range()),
        Some(surface.head_range()),
        surface.arguments_range(),
        surface.inline_content_range(),
    ]
    .into_iter()
    .flatten()
    .all(|range| {
        range.start() <= range.end()
            && range.end() <= source.len()
            && source.is_char_boundary(range.start())
            && source.is_char_boundary(range.end())
    })
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
