//! Sans I/O source-edit helpers for Arcweft tooling.
//!
//! This crate produces deterministic text edits and lightweight tooling data.
//! It does not read files, write files, watch paths, or run an LSP transport.

use arcweft_lang_syntax::{CstLine, IdRef, Item, ParsedSource, cst_lines, parse_source};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
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
    collect_dialogue_line_id_edits(source, &mut edits);
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
    collect_dialogue_id_hints(source, &mut hints);
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

fn collect_dialogue_line_id_edits(source: &str, edits: &mut Vec<TextEdit>) {
    let parsed = parse_source(source);
    let mut flow_slug = None;
    let mut scopes: Vec<ScopedLine> = Vec::new();
    let mut line_counters = BTreeMap::<String, usize>::new();
    for line in cst_lines(parsed.syntax()).iter() {
        let text = line.text();
        let trimmed = line.trimmed();
        let indent = text.len() - text.trim_start().len();
        if update_dialogue_scan_context(
            trimmed,
            indent,
            &mut scopes,
            &mut flow_slug,
            &mut line_counters,
        ) {
            continue;
        }
        let Some(flow) = flow_slug.as_deref() else {
            continue;
        };
        let Some(dialogue_head) = dialogue_head(trimmed) else {
            continue;
        };
        let speaker = speaker_slug(dialogue_head.callee);
        let scope_names = scopes
            .iter()
            .map(|scope| scope.name.as_str())
            .collect::<Vec<_>>();
        let mut normalized_id = dialogue_head.option("id").and_then(|option| {
            normalized_line_option_id(option.value, "say", flow, &speaker, &scope_names)
        });
        for option in &dialogue_head.options {
            let Some((relative, family)) = parse_relative_materialization(option.value) else {
                continue;
            };
            let expected_family = if option.name == "id" { "say" } else { "text" };
            if family.is_some_and(|family| family != expected_family) {
                continue;
            }
            let Some(scope_prefix) = relative_scope_prefix(&scope_names, relative.parent_depth)
            else {
                continue;
            };
            let normalized = if option.name == "id" {
                scoped_id("say", flow, &speaker, &scope_prefix, relative.suffix)
            } else {
                scoped_id("text", flow, &speaker, &scope_prefix, relative.suffix)
            };
            let Some(options_start) = dialogue_head.options_start else {
                continue;
            };
            let start = line.start() + text.len() - text.trim_start().len()
                + options_start
                + option.relative_start;
            edits.push(TextEdit {
                start,
                end: start + option.value.len(),
                replacement: format!("@{normalized}"),
            });
            if option.name == "id" {
                normalized_id = Some(normalized);
            }
        }
        if normalized_id.is_none() {
            normalized_id = Some(next_generated_line_id(
                flow,
                &speaker,
                &scope_names,
                &mut line_counters,
            ));
        }
        let Some(line_id) = normalized_id else {
            continue;
        };
        let text_key = dialogue_head.option("text_key").and_then(|option| {
            normalized_line_option_id(option.value, "text", flow, &speaker, &scope_names)
        });
        let missing = missing_line_options(&dialogue_head, &line_id, text_key.as_deref());
        if !missing.is_empty() {
            edits.push(insert_missing_line_options(line, &dialogue_head, &missing));
        }
    }
}

fn collect_dialogue_id_hints(source: &str, hints: &mut Vec<InlayHint>) {
    let parsed = parse_source(source);
    let mut flow_slug = None;
    let mut scopes: Vec<ScopedLine> = Vec::new();
    let mut line_counters = BTreeMap::<String, usize>::new();
    for line in cst_lines(parsed.syntax()).iter() {
        let text = line.text();
        let trimmed = line.trimmed();
        let leading = text.len() - text.trim_start().len();
        let indent = leading;
        if update_dialogue_scan_context(
            trimmed,
            indent,
            &mut scopes,
            &mut flow_slug,
            &mut line_counters,
        ) {
            continue;
        }
        let Some(flow) = flow_slug.as_deref() else {
            continue;
        };
        let Some(dialogue_head) = dialogue_head(trimmed) else {
            continue;
        };
        let speaker = speaker_slug(dialogue_head.callee);
        let scope_names = scopes
            .iter()
            .map(|scope| scope.name.as_str())
            .collect::<Vec<_>>();
        let normalized_id = dialogue_head
            .option("id")
            .and_then(|option| {
                normalized_line_option_id(option.value, "say", flow, &speaker, &scope_names)
            })
            .unwrap_or_else(|| {
                next_generated_line_id(flow, &speaker, &scope_names, &mut line_counters)
            });
        for option in &dialogue_head.options {
            let family = if option.name == "id" { "say" } else { "text" };
            if let Some(normalized) =
                normalized_line_option_id(option.value, family, flow, &speaker, &scope_names)
            {
                let Some(options_start) = dialogue_head.options_start else {
                    continue;
                };
                hints.push(InlayHint {
                    position: line.start()
                        + leading
                        + options_start
                        + option.relative_start
                        + option.value.len(),
                    label: format!("@{normalized}"),
                });
            }
        }
        let text_key = dialogue_head
            .option("text_key")
            .and_then(|option| {
                normalized_line_option_id(option.value, "text", flow, &speaker, &scope_names)
            })
            .unwrap_or_else(|| line_id_to_text_key(&normalized_id));
        let missing = missing_line_options(&dialogue_head, &normalized_id, Some(&text_key));
        if !missing.is_empty() {
            hints.push(InlayHint {
                position: line.start() + leading + dialogue_head.missing_options_insert,
                label: missing.join(", "),
            });
        }
    }
}

fn update_dialogue_scan_context(
    trimmed: &str,
    indent: usize,
    scopes: &mut Vec<ScopedLine>,
    flow_slug: &mut Option<String>,
    line_counters: &mut BTreeMap<String, usize>,
) -> bool {
    while scopes
        .last()
        .and_then(|scope| scope.indent)
        .is_some_and(|scope_indent| indent <= scope_indent)
    {
        scopes.pop();
    }
    if let Some(fence) = flat_fence(trimmed) {
        if fence.close && fence.kind == "scope" {
            scopes.pop();
            return true;
        }
        if !fence.close && fence.kind == "scope" {
            if let Some(name) = nonempty_identifier(fence.head) {
                scopes.push(ScopedLine { indent: None, name });
            }
            return true;
        }
    }
    if let Some(slug) = flow_slug_from_line(trimmed) {
        *flow_slug = Some(slug);
        scopes.clear();
        line_counters.clear();
    }
    if let Some(name) = scope_name_from_line(trimmed) {
        scopes.push(ScopedLine {
            indent: Some(indent),
            name,
        });
        return true;
    }
    false
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

fn scope_name_from_line(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("scope ")?;
    let name = rest
        .split_whitespace()
        .next()?
        .trim_end_matches('{')
        .trim_end_matches(':');
    is_identifier(name).then(|| name.to_owned())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScopedLine {
    indent: Option<usize>,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DialogueHead<'a> {
    callee: &'a str,
    options_start: Option<usize>,
    options_has_any: bool,
    missing_options_insert: usize,
    options: Vec<LineIdOption<'a>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LineIdOption<'a> {
    name: &'a str,
    value: &'a str,
    relative_start: usize,
}

fn dialogue_head(trimmed: &str) -> Option<DialogueHead<'_>> {
    if let Some(fence) = flat_fence(trimmed)
        && !fence.close
        && fence.kind == "line"
    {
        let head_start = flat_fence_head_start(trimmed)?;
        return dialogue_head_from_call_head(fence.head, head_start);
    }
    let boundary = dialogue_head_boundary(trimmed)?;
    dialogue_head_from_call_head(trimmed[..boundary].trim_end(), 0)
}

fn dialogue_head_from_call_head(head: &str, base: usize) -> Option<DialogueHead<'_>> {
    let open = head.find('(');
    let close = head.rfind(')');
    let (callee, options_start, options_has_any, missing_options_insert, options) =
        match (open, close) {
            (Some(open), Some(close)) if close >= open => {
                let callee = head[..open].trim();
                let options_source = &head[open + 1..close];
                (
                    callee,
                    Some(base + open + 1),
                    !options_source.trim().is_empty(),
                    base + close,
                    line_id_options(options_source),
                )
            }
            _ => (
                head.trim(),
                None,
                false,
                base + head.trim_end().len(),
                Vec::new(),
            ),
        };
    if callee.is_empty()
        || callee.starts_with('@')
        || callee.contains('=')
        || callee.split_whitespace().nth(1).is_some()
        || is_control_head(callee)
    {
        return None;
    }
    Some(DialogueHead {
        callee,
        options_start,
        options_has_any,
        missing_options_insert,
        options,
    })
}

impl<'a> DialogueHead<'a> {
    fn option(&self, name: &str) -> Option<&LineIdOption<'a>> {
        self.options.iter().find(|option| option.name == name)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FlatFence<'a> {
    kind: &'a str,
    head: &'a str,
    close: bool,
}

fn flat_fence(source: &str) -> Option<FlatFence<'_>> {
    let inner = source
        .trim()
        .strip_prefix("===")?
        .strip_suffix("===")?
        .trim();
    if let Some(close) = inner.strip_prefix('/') {
        let kind = close.split_whitespace().next().unwrap_or_default();
        return Some(FlatFence {
            kind,
            head: close.trim(),
            close: true,
        });
    }
    let (kind, head) = split_leading_word(inner).unwrap_or((inner, ""));
    Some(FlatFence {
        kind,
        head: head.trim(),
        close: false,
    })
}

fn flat_fence_head_start(source: &str) -> Option<usize> {
    let open = source.find("===")? + "===".len();
    let after_open = &source[open..];
    let inner_leading = after_open.len() - after_open.trim_start().len();
    let inner_start = open + inner_leading;
    let inner = &source[inner_start..source.rfind("===")?];
    let (_, head) = split_leading_word(inner.trim())?;
    source[inner_start..]
        .find(head)
        .map(|offset| inner_start + offset)
}

fn split_leading_word(source: &str) -> Option<(&str, &str)> {
    let trimmed = source.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let end = trimmed
        .char_indices()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
        .unwrap_or(trimmed.len());
    Some((&trimmed[..end], trimmed[end..].trim_start()))
}

fn nonempty_identifier(source: &str) -> Option<String> {
    let name = source.trim();
    (!name.is_empty() && is_identifier(name)).then(|| name.to_owned())
}

fn normalized_line_option_id(
    raw: &str,
    family: &str,
    flow: &str,
    speaker: &str,
    scopes: &[&str],
) -> Option<String> {
    let absolute_prefix = format!("@{family}.");
    if let Some(body) = raw.strip_prefix(&absolute_prefix) {
        return Some(format!("{family}.{body}"));
    }
    let (relative, explicit_family) = parse_relative_materialization(raw)?;
    if explicit_family.is_some_and(|explicit| explicit != family) {
        return None;
    }
    let scope_prefix = relative_scope_prefix(scopes, relative.parent_depth)?;
    Some(scoped_id(
        family,
        flow,
        speaker,
        &scope_prefix,
        relative.suffix,
    ))
}

fn next_generated_line_id(
    flow: &str,
    speaker: &str,
    scopes: &[&str],
    counters: &mut BTreeMap<String, usize>,
) -> String {
    let mut parts = vec!["say", flow, speaker];
    parts.extend(scopes.iter().copied());
    let prefix = parts.join(".");
    let next = counters.entry(prefix.clone()).or_insert(0);
    *next += 1;
    format!("{prefix}.{next:03}")
}

fn missing_line_options(
    head: &DialogueHead<'_>,
    line_id: &str,
    text_key: Option<&str>,
) -> Vec<String> {
    let mut missing = Vec::new();
    if head.option("id").is_none() {
        missing.push(format!("id=@{line_id}"));
    }
    if head.option("text_key").is_none() {
        missing.push(format!(
            "text_key=@{}",
            text_key.map_or_else(|| line_id_to_text_key(line_id), str::to_owned)
        ));
    }
    missing
}

fn insert_missing_line_options(
    line: &CstLine,
    head: &DialogueHead<'_>,
    missing: &[String],
) -> TextEdit {
    let leading = line.text().len() - line.text().trim_start().len();
    let start = line.start() + leading + head.missing_options_insert;
    let joined = missing.join(", ");
    let replacement = if head.options_start.is_some() {
        if head.options_has_any {
            format!(", {joined}")
        } else {
            joined
        }
    } else {
        format!("({joined})")
    };
    TextEdit {
        start,
        end: start,
        replacement,
    }
}

fn line_id_to_text_key(line_id: &str) -> String {
    line_id
        .strip_prefix("say.")
        .map_or_else(|| format!("text.{line_id}"), |tail| format!("text.{tail}"))
}

fn dialogue_head_boundary(trimmed: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut string_delim = None;
    let mut escaped = false;
    for (index, ch) in trimmed.char_indices() {
        if let Some(delim) = string_delim {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == delim {
                string_delim = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => string_delim = Some(ch),
            '(' | '{' => depth += 1,
            ')' | '}' => depth = depth.saturating_sub(1),
            ':' | '[' if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn is_control_head(callee: &str) -> bool {
    matches!(
        callee,
        "if" | "while" | "for" | "match" | "choice" | "scope" | "flow"
    )
}

fn line_id_options(source: &str) -> Vec<LineIdOption<'_>> {
    split_top_level_options(source)
        .into_iter()
        .filter_map(|part| {
            let item = &source[part.clone()];
            let eq = item.find('=')?;
            let name = item[..eq].trim();
            if !matches!(name, "id" | "text_key") {
                return None;
            }
            let value_source = &item[eq + 1..];
            let value = value_source.trim();
            let leading = value_source.len() - value_source.trim_start().len();
            Some(LineIdOption {
                name,
                value,
                relative_start: part.start + eq + 1 + leading,
            })
        })
        .collect()
}

fn split_top_level_options(source: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut string_delim = None;
    let mut escaped = false;
    for (index, ch) in source.char_indices() {
        if let Some(delim) = string_delim {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == delim {
                string_delim = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => string_delim = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                push_trimmed_range(source, start..index, &mut ranges);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    push_trimmed_range(source, start..source.len(), &mut ranges);
    ranges
}

fn push_trimmed_range(
    source: &str,
    range: std::ops::Range<usize>,
    ranges: &mut Vec<std::ops::Range<usize>>,
) {
    let item = &source[range.clone()];
    let leading = item.len() - item.trim_start().len();
    let trailing = item.len() - item.trim_end().len();
    let start = range.start + leading;
    let end = range.end.saturating_sub(trailing);
    if start < end {
        ranges.push(start..end);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelativeMaterialization<'a> {
    suffix: &'a str,
    parent_depth: usize,
}

fn parse_relative_materialization(
    raw: &str,
) -> Option<(RelativeMaterialization<'_>, Option<&str>)> {
    if !raw.starts_with('@') || raw.starts_with("@<") {
        return None;
    }
    if let Some((family, rest)) = raw[1..].split_once(":.") {
        return (!rest.is_empty()).then_some((
            RelativeMaterialization {
                suffix: rest,
                parent_depth: 0,
            },
            Some(family),
        ));
    }
    raw.strip_prefix('@')
        .and_then(|rest| relative_entity(rest).map(|relative| (relative, None)))
}

fn relative_entity(rest: &str) -> Option<RelativeMaterialization<'_>> {
    relative_dot_run(rest, 0).or_else(|| relative_super_chain(rest))
}

fn relative_dot_run(rest: &str, extra_parent_depth: usize) -> Option<RelativeMaterialization<'_>> {
    let dots = rest.chars().take_while(|ch| *ch == '.').count();
    if dots == 0 {
        return None;
    }
    let suffix = &rest[dots..];
    (!suffix.is_empty()).then_some(RelativeMaterialization {
        suffix,
        parent_depth: dots.saturating_sub(1) + extra_parent_depth,
    })
}

fn relative_super_chain(rest: &str) -> Option<RelativeMaterialization<'_>> {
    let mut depth = 0usize;
    let mut tail = rest;
    while let Some(next) = tail.strip_prefix("super.") {
        depth += 1;
        tail = next;
    }
    (!tail.is_empty() && depth > 0).then_some(RelativeMaterialization {
        suffix: tail,
        parent_depth: depth,
    })
}

fn relative_scope_prefix<'a>(scopes: &'a [&str], parent_depth: usize) -> Option<Vec<&'a str>> {
    let take = scopes.len().checked_sub(parent_depth)?;
    Some(scopes.iter().copied().take(take).collect())
}

fn scoped_id(family: &str, flow: &str, speaker: &str, scopes: &[&str], suffix: &str) -> String {
    let mut parts = vec![family, flow, speaker];
    parts.extend(scopes.iter().copied());
    parts.push(suffix);
    parts.join(".")
}

fn speaker_slug(callee: &str) -> String {
    let base = callee
        .trim()
        .trim_end_matches(".say")
        .strip_prefix("@<")
        .and_then(|inner| inner.strip_suffix('>'))
        .or_else(|| callee.trim().strip_prefix('@'))
        .unwrap_or_else(|| callee.trim().trim_end_matches(".say"));
    match base {
        "地の文" | "地文" | "ナレーター" | "ナレータ" | "ナレーション" | "語り" | "語り手"
        | "narrator" | "Narrator" | "NARRATOR" | "VO" | "V.O." | "O.S." | "Offscreen"
        | "Script" | "StageDirection" | "ト書き" | "脚本" => "narrator".to_owned(),
        other => other
            .rsplit('.')
            .next()
            .unwrap_or(other)
            .trim()
            .to_ascii_lowercase(),
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

    #[test]
    fn materializes_dialogue_line_option_ids() {
        let source = "flow @flow.opening opening {\n    scope outer {\n        scope rain {\n            地の文(id=@say:.sound):\n                雨の音。[p]\n            alice(id=@.comment, text_key=@.comment_text):\n                Good morning.[p]\n            alice.say(id=@...shared, text_key=@super.inner_text)[\n                Shared.[p]\n            ]\n        }\n    }\n}\n";
        let report = materialize_ids(source).expect("materialize report");

        assert!(report.output.contains(
            "地の文(id=@say.opening.narrator.outer.rain.sound, text_key=@text.opening.narrator.outer.rain.sound):"
        ));
        assert!(report.output.contains(
            "alice(id=@say.opening.alice.outer.rain.comment, text_key=@text.opening.alice.outer.rain.comment_text):"
        ));
        assert!(report.output.contains(
            "alice.say(id=@say.opening.alice.shared, text_key=@text.opening.alice.outer.inner_text)["
        ));
    }

    #[test]
    fn materializes_omitted_dialogue_ids_in_colon_call_and_flat_fences() {
        let source = "flow @flow.opening opening {\n    alice:\n        Hi[p]\n    alice.say()[\n        Again[p]\n    ]\n=== scope rain ===\n=== line 地の文 ===\n雨。[p]\n=== with ===\nwait mark .done\n=== /with ===\n=== /line ===\n=== /scope ===\n}\n";
        let report = materialize_ids(source).expect("materialize report");

        assert!(
            report
                .output
                .contains("alice(id=@say.opening.alice.001, text_key=@text.opening.alice.001):")
        );
        assert!(
            report.output.contains(
                "alice.say(id=@say.opening.alice.002, text_key=@text.opening.alice.002)["
            )
        );
        assert!(report.output.contains(
            "=== line 地の文(id=@say.opening.narrator.rain.001, text_key=@text.opening.narrator.rain.001) ==="
        ));
        assert!(report.output.contains("=== with ==="));
    }
}
