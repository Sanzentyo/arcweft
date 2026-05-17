//! Sans I/O source-edit helpers for Arcweft tooling.
//!
//! This crate produces deterministic text edits and lightweight tooling data.
//! It does not read files, write files, watch paths, or run an LSP transport.

use arcweft_lang_syntax::{IdRef, Item, ParsedSource, cst_lines, parse_source};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Formatting and source normalization options.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FormatOptions {
    /// Rewrite script-friendly sugar into canonical block/call forms.
    pub expand_sugar: bool,
}

/// A half-open source edit over UTF-8 byte offsets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

/// One diagnostic produced while computing tooling edits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolingDiagnostic {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

/// Inlay hint data independent from any concrete LSP transport.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InlayHint {
    pub position: usize,
    pub label: String,
}

/// Tooling code action data independent from any concrete LSP transport.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolingCodeAction {
    pub id: String,
    pub label: String,
    pub edit: Option<TextEdit>,
}

/// A complete source-edit report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolingEditReport {
    pub status: String,
    pub changed: bool,
    pub edits: Vec<TextEdit>,
    pub output: String,
    pub diagnostics: Vec<ToolingDiagnostic>,
}

/// Error returned when edit application would corrupt source coordinates.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ToolingError {
    #[error("text edit range {start}..{end} is outside source length {len}")]
    RangeOutOfBounds {
        start: usize,
        end: usize,
        len: usize,
    },
    #[error("text edit range {start}..{end} overlaps a later edit")]
    OverlappingEdit { start: usize, end: usize },
}

/// Formats source while preserving authoring sugar by default.
pub fn format_source(
    source: &str,
    options: FormatOptions,
) -> Result<ToolingEditReport, ToolingError> {
    let edits = if options.expand_sugar {
        sugar_expansion_edits(source)
    } else {
        Vec::new()
    };
    report_from_edits(source, edits)
}

/// Rewrites ID-context relative IDs to normalized explicit IDs.
pub fn materialize_ids(source: &str) -> Result<ToolingEditReport, ToolingError> {
    let mut edits = Vec::new();
    collect_declaration_line_id_edits(source, &mut edits);
    collect_choice_line_id_edits(source, &mut edits);
    report_from_edits(source, edits)
}

/// Computes inferred-ID inlay hints for relative ID positions.
pub fn inferred_id_hints(source: &str) -> Vec<InlayHint> {
    let parsed = parse_source(source);
    let mut hints = Vec::new();
    for item in parsed.typed_tree().items() {
        collect_item_id_hints(item, &mut hints);
    }
    hints
}

/// Returns source-level code actions that are safe to expose through LSP.
pub fn source_code_actions(source: &str) -> Vec<ToolingCodeAction> {
    let mut actions = Vec::new();
    for edit in sugar_expansion_edits(source) {
        actions.push(ToolingCodeAction {
            id: "arcweft.expandSugar".to_owned(),
            label: "Expand Arcweft sugar".to_owned(),
            edit: Some(edit),
        });
    }
    if let Ok(report) = materialize_ids(source) {
        actions.extend(report.edits.into_iter().map(|edit| ToolingCodeAction {
            id: "arcweft.materializeId".to_owned(),
            label: "Materialize inferred Arcweft ID".to_owned(),
            edit: Some(edit),
        }));
    }
    actions
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

fn report_from_edits(
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

fn sugar_expansion_edits(source: &str) -> Vec<TextEdit> {
    let parsed = parse_source(source);
    let lines = cst_lines(parsed.syntax());
    let speaker_presets = collect_speaker_preset_locals(source);
    let character_aliases = collect_character_aliases(&parsed);
    let mut edits = Vec::new();

    for line in lines.iter() {
        if line.kind() == arcweft_lang_syntax::CstLineKind::Comment {
            continue;
        }
        edits.extend(parent_path_edits(line.text(), line.start()));
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
        if let Some(edit) = speaker_line_edit(
            line.text(),
            line.start(),
            &speaker_presets,
            &character_aliases,
        ) {
            edits.push(edit);
        }
        if let Some(edit) = await_question_edit(line.text(), line.start()) {
            edits.push(edit);
        }
    }
    edits
}

fn collect_character_aliases(parsed: &ParsedSource) -> BTreeSet<String> {
    parsed
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::EntityDecl(entity)
                if entity.kind() == arcweft_lang_syntax::EntityDeclKind::Character =>
            {
                entity.surface_alias().map(str::to_owned)
            }
            _ => None,
        })
        .collect()
}

fn collect_speaker_preset_locals(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed.strip_prefix("let ")?;
            let (name, rhs) = rest.split_once('=')?;
            rhs.contains('(')
                .then(|| name.trim())
                .filter(|name| is_identifier(name))
                .map(str::to_owned)
        })
        .collect()
}

fn parent_path_edits(line: &str, base: usize) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let mut search = 0;
    while let Some(offset) = line[search..].find("parent::") {
        let start = search + offset;
        edits.push(TextEdit {
            start: base + start,
            end: base + start + "parent".len(),
            replacement: "super".to_owned(),
        });
        search = start + "parent::".len();
    }
    edits
}

fn await_question_edit(line: &str, base: usize) -> Option<TextEdit> {
    let leading = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("await? ")?;
    Some(TextEdit {
        start: base + leading,
        end: base + line.len(),
        replacement: format!("try await {rest}"),
    })
}

fn speaker_line_edit(
    line: &str,
    base: usize,
    speaker_presets: &BTreeSet<String>,
    character_aliases: &BTreeSet<String>,
) -> Option<TextEdit> {
    let leading = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    if trimmed.starts_with("//")
        || trimmed.starts_with("///")
        || trimmed.starts_with("with:")
        || trimmed.starts_with("case ")
    {
        return None;
    }
    let (head, text) = trimmed.split_once(':')?;
    if head.contains(' ') || text.trim().is_empty() || head.starts_with('@') {
        return None;
    }
    let (base_name, args) = split_call_head(head.trim());
    if !is_identifier(base_name) {
        return None;
    }
    let text = text.trim_start();
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
    Some(TextEdit {
        start: base + leading,
        end: base + line.len(),
        replacement: format!("{callee}[{text}]"),
    })
}

fn split_call_head(head: &str) -> (&str, Option<&str>) {
    let Some(open) = head.find('(') else {
        return (head, None);
    };
    if !head.ends_with(')') {
        return (head, None);
    }
    (&head[..open], Some(&head[open + 1..head.len() - 1]))
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn closing_brace_insert(
    lines: &arcweft_lang_syntax::CstLineEvents,
    with_start: usize,
) -> Option<TextEdit> {
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

fn collect_item_id_hints(item: &Item, hints: &mut Vec<InlayHint>) {
    match item {
        Item::Flow(flow) => collect_optional_id_hint(flow.id(), "flow", hints),
        Item::Test(test) => collect_id_hint(test.id(), "test", hints),
        Item::Bench(bench) => collect_id_hint(bench.id(), "bench", hints),
        _ => {}
    }
}

fn collect_optional_id_hint(id: Option<&IdRef>, family: &str, hints: &mut Vec<InlayHint>) {
    if let Some(id) = id {
        collect_id_hint(id, family, hints);
    }
}

fn collect_id_hint(id: &IdRef, family: &str, hints: &mut Vec<InlayHint>) {
    if let Some(normalized) = normalized_id_ref(id, family, None) {
        hints.push(InlayHint {
            position: id.range().end(),
            label: format!("@{normalized}"),
        });
    }
}

fn normalized_id_ref(id: &IdRef, family: &str, flow_slug: Option<&str>) -> Option<String> {
    match id {
        IdRef::Absolute(_) => None,
        IdRef::Relative(relative) => Some(match flow_slug {
            Some(flow) if family == "choice" => format!("{family}.{flow}.{}", relative.suffix()),
            _ => format!("{family}.{}", relative.suffix()),
        }),
        IdRef::FamilyRelative(relative) => Some(match flow_slug {
            Some(flow) if relative.family() == "choice" => {
                format!(
                    "{}.{}.{}",
                    relative.family(),
                    flow,
                    relative.relative().suffix()
                )
            }
            _ => format!("{}.{}", relative.family(), relative.relative().suffix()),
        }),
    }
}

fn collect_declaration_line_id_edits(source: &str, edits: &mut Vec<TextEdit>) {
    let parsed = parse_source(source);
    for line in cst_lines(parsed.syntax()).iter() {
        let trimmed = line.trimmed();
        for family in ["flow", "test", "bench"] {
            if let Some((relative_start, raw_id)) = declaration_id_token(trimmed, family)
                && let Some(normalized) = normalize_raw_id(raw_id, family, None)
            {
                let start = line.start() + line.text().len() - line.text().trim_start().len()
                    + relative_start;
                edits.push(TextEdit {
                    start,
                    end: start + raw_id.len(),
                    replacement: format!("@{normalized}"),
                });
            }
        }
    }
}

fn collect_choice_line_id_edits(source: &str, edits: &mut Vec<TextEdit>) {
    let parsed = parse_source(source);
    let mut flow_slug = None;
    let mut choice_id = None;
    for line in cst_lines(parsed.syntax()).iter() {
        let trimmed = line.trimmed();
        if let Some(slug) = flow_slug_from_line(trimmed) {
            flow_slug = Some(slug);
            choice_id = None;
        }
        if let Some((relative_start, raw_id)) = keyword_id_token(trimmed, "choice")
            && let Some(normalized) = normalize_raw_id(raw_id, "choice", flow_slug.as_deref())
        {
            let start =
                line.start() + line.text().len() - line.text().trim_start().len() + relative_start;
            edits.push(TextEdit {
                start,
                end: start + raw_id.len(),
                replacement: format!("@{normalized}"),
            });
            choice_id = Some(normalized);
            continue;
        }
        if let Some(choice) = choice_id.as_deref()
            && let Some((relative_start, raw_id)) = choice_option_id_token(trimmed)
            && let Some(suffix) = raw_relative_suffix(raw_id)
        {
            let start =
                line.start() + line.text().len() - line.text().trim_start().len() + relative_start;
            edits.push(TextEdit {
                start,
                end: start + raw_id.len(),
                replacement: format!("@{choice}.{suffix}"),
            });
        }
    }
}

fn declaration_id_token<'a>(trimmed: &'a str, keyword: &str) -> Option<(usize, &'a str)> {
    if let Some(token) = keyword_id_token(trimmed, keyword) {
        return Some(token);
    }
    let pub_prefix = "pub ";
    let rest = trimmed.strip_prefix(pub_prefix)?;
    keyword_id_token(rest, keyword).map(|(start, id)| (pub_prefix.len() + start, id))
}

fn flow_slug_from_line(trimmed: &str) -> Option<String> {
    let rest = trimmed
        .strip_prefix("flow ")
        .or_else(|| trimmed.strip_prefix("pub flow "))?;
    let mut parts = rest.split_whitespace();
    let first = parts.next()?;
    if first.starts_with('@') {
        normalize_raw_id(first, "flow", None)
            .map(|id| id.trim_start_matches("flow.").to_owned())
            .or_else(|| {
                parts
                    .next()
                    .map(|name| name.trim_end_matches('{').to_owned())
            })
    } else {
        Some(first.trim_end_matches('{').to_owned())
    }
}

fn keyword_id_token<'a>(trimmed: &'a str, keyword: &str) -> Option<(usize, &'a str)> {
    let rest = trimmed.strip_prefix(keyword)?;
    let trimmed_rest = rest.trim_start();
    let id = trimmed_rest.split_whitespace().next()?;
    id.starts_with('@')
        .then_some((keyword.len() + rest.len() - trimmed_rest.len(), id))
}

fn choice_option_id_token(trimmed: &str) -> Option<(usize, &str)> {
    let id = trimmed.split_whitespace().next()?;
    id.starts_with("@.")
        .then_some((trimmed.find(id).unwrap_or(0), id))
}

fn normalize_raw_id(raw: &str, family: &str, flow_slug: Option<&str>) -> Option<String> {
    if raw.starts_with(&format!("@{family}.")) {
        return None;
    }
    if let Some(suffix) = raw.strip_prefix("@.") {
        return Some(match flow_slug {
            Some(flow) if family == "choice" => format!("{family}.{flow}.{suffix}"),
            _ => format!("{family}.{suffix}"),
        });
    }
    let family_prefix = format!("@{family}:.");
    raw.strip_prefix(&family_prefix)
        .map(|suffix| match flow_slug {
            Some(flow) if family == "choice" => format!("{family}.{flow}.{suffix}"),
            _ => format!("{family}.{suffix}"),
        })
}

fn raw_relative_suffix(raw: &str) -> Option<&str> {
    raw.strip_prefix("@.")
}

fn dedupe_edits(edits: &mut Vec<TextEdit>) {
    edits.sort_by_key(|edit| (edit.start, edit.end, edit.replacement.clone()));
    edits.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_format_preserves_sugar() {
        let source = "flow @flow.opening opening {\n    alice: hi[p]\n}\n";
        let report = format_source(source, FormatOptions::default()).expect("format report");
        assert!(!report.changed);
        assert_eq!(report.output, source);
    }

    #[test]
    fn expands_speaker_with_and_parent_sugar() {
        let source = "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    alice: hi[p]\n    with:\n        log.info(\"x\")\n    goto parent::next\n}\n";
        let report =
            format_source(source, FormatOptions { expand_sugar: true }).expect("format report");
        assert!(report.output.contains("alice.say()[hi[p]]"));
        assert!(report.output.contains("with {"));
        assert!(report.output.contains("    }"));
        assert!(report.output.contains("goto super::next"));
    }

    #[test]
    fn materializes_top_level_and_choice_ids() {
        let source = "flow @flow.opening opening {\n    choice @.first {\n        @.listen \"Listen\" -> @flow.next\n    }\n}\ntest @.smoke scenario {}\n";
        let report = materialize_ids(source).expect("materialize report");
        assert!(report.output.contains("choice @choice.opening.first"));
        assert!(report.output.contains("@choice.opening.first.listen"));
        assert!(report.output.contains("test @test.smoke scenario"));
    }
}
