//! Typed ID context collected from parsed Arcweft source.
//!
//! This module is the shared syntax-to-HIR bridge for ID materialization used
//! by formatter tooling, CLI commands, and LSP code actions. It intentionally
//! returns typed entries instead of source-specific text edits so higher layers
//! do not keep their own dialogue scanners.

use crate::dialogue_identity::{DialogueIdFamily, DialogueLineId, DialogueSpeakerSlug};
use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        flow::FlowItem,
        items::{Item, TypedSyntaxTree},
    },
    cst::{CstLine, CstLineKind, cst_lines, text::parse_flat_fence},
    parser::parse_source,
    source::ParsedSource,
};
use std::collections::{BTreeMap, BTreeSet};

/// ID family attached to a materialized source position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdContextKind {
    /// A top-level declaration ID such as `flow`, `test`, or `bench`.
    Declaration { family: &'static str },
    /// A `choice` block ID.
    Choice,
    /// A choice option ID scoped below the current choice.
    ChoiceOption,
    /// A dialogue line `id` option.
    DialogueLineId,
    /// A dialogue line `text_key` option.
    DialogueTextKey,
    /// A pair of missing dialogue options inferred from flow/speaker/scope.
    DialogueMissingOptions,
}

/// One named line option to insert into a dialogue call head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdContextOption {
    name: String,
    normalized: String,
}

impl IdContextOption {
    /// Creates an inferred option value without the leading `@`.
    pub fn new(name: impl Into<String>, normalized: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            normalized: normalized.into(),
        }
    }

    /// Option name such as `id` or `text_key`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Fully normalized public ID without the leading `@`.
    pub fn normalized(&self) -> &str {
        &self.normalized
    }

    /// Arcweft line-option spelling, for example `id=@say.opening.alice.001`.
    pub fn as_assignment(&self) -> String {
        format!("{}=@{}", self.name, self.normalized)
    }
}

/// Source operation represented by one ID-context entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdContextMaterialization {
    /// Replace the relative ID at `range` with `@{normalized}`.
    Replace {
        range: TextRange,
        normalized: String,
    },
    /// Insert missing dialogue options at `insert`.
    InsertDialogueOptions {
        insert: TextRange,
        call_has_options: bool,
        options_has_any: bool,
        options: Vec<IdContextOption>,
    },
}

/// One typed ID-context entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdContextEntry {
    kind: IdContextKind,
    materialization: IdContextMaterialization,
}

impl IdContextEntry {
    /// Builds a replacement entry for a normalized ID.
    pub fn replace(kind: IdContextKind, range: TextRange, normalized: impl Into<String>) -> Self {
        Self {
            kind,
            materialization: IdContextMaterialization::Replace {
                range,
                normalized: normalized.into(),
            },
        }
    }

    /// Builds an insertion entry for missing dialogue line options.
    pub fn insert_dialogue_options(
        insert: TextRange,
        call_has_options: bool,
        options_has_any: bool,
        options: Vec<IdContextOption>,
    ) -> Self {
        Self {
            kind: IdContextKind::DialogueMissingOptions,
            materialization: IdContextMaterialization::InsertDialogueOptions {
                insert,
                call_has_options,
                options_has_any,
                options,
            },
        }
    }

    /// Entry kind.
    pub fn kind(&self) -> IdContextKind {
        self.kind
    }

    /// Typed source operation needed by tooling, LSP, or CLI.
    pub fn materialization(&self) -> &IdContextMaterialization {
        &self.materialization
    }
}

/// Complete source-level ID context.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IdContextReport {
    entries: Vec<IdContextEntry>,
}

impl IdContextReport {
    /// Typed entries in source order.
    pub fn entries(&self) -> &[IdContextEntry] {
        &self.entries
    }

    fn push(&mut self, entry: IdContextEntry) {
        self.entries.push(entry);
    }
}

/// Collects typed ID materialization context from Arcweft source.
pub fn collect_id_context(source: &str) -> IdContextReport {
    let parsed = parse_source(source);
    let mut report = IdContextReport::default();
    collect_declaration_ids(&parsed, &mut report);
    collect_choice_ids(&parsed, &mut report);
    collect_dialogue_ids(&parsed, &mut report);
    report
}

fn collect_declaration_ids(parsed: &ParsedSource, report: &mut IdContextReport) {
    for line in cst_lines(parsed.syntax()).iter() {
        let trimmed = line.trimmed();
        for family in ["flow", "test", "bench"] {
            if let Some((relative_start, raw_id)) = declaration_id_token(trimmed, family)
                && let Some(normalized) = normalize_raw_id(raw_id, family, None)
            {
                let start = line.start() + leading_len(line.text()) + relative_start;
                report.push(IdContextEntry::replace(
                    IdContextKind::Declaration { family },
                    TextRange::new(start, start + raw_id.len()),
                    normalized,
                ));
            }
        }
    }
}

fn collect_choice_ids(parsed: &ParsedSource, report: &mut IdContextReport) {
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
            let start = line.start() + leading_len(line.text()) + relative_start;
            report.push(IdContextEntry::replace(
                IdContextKind::Choice,
                TextRange::new(start, start + raw_id.len()),
                normalized.clone(),
            ));
            choice_id = Some(normalized);
            continue;
        }
        if let Some(choice) = choice_id.as_deref()
            && let Some((relative_start, raw_id)) = choice_option_id_token(trimmed)
            && let Some(suffix) = raw_relative_suffix(raw_id)
        {
            let start = line.start() + leading_len(line.text()) + relative_start;
            report.push(IdContextEntry::replace(
                IdContextKind::ChoiceOption,
                TextRange::new(start, start + raw_id.len()),
                format!("{choice}.{suffix}"),
            ));
        }
    }
}

fn collect_dialogue_ids(parsed: &ParsedSource, report: &mut IdContextReport) {
    let dialogue_starts = dialogue_start_offsets(parsed.typed_tree());
    let mut flow_slug = None;
    let mut scopes: Vec<ScopedLine> = Vec::new();
    let mut line_counters = BTreeMap::<String, usize>::new();
    for line in cst_lines(parsed.syntax()).iter() {
        if line.kind() == CstLineKind::Comment {
            continue;
        }
        let text = line.text();
        let trimmed = line.trimmed();
        let indent = leading_len(text);
        if update_dialogue_context(
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
        if dialogue_starts
            .range(line.start()..line.end())
            .next()
            .is_none()
        {
            continue;
        }
        collect_dialogue_line(line, flow, &scopes, &mut line_counters, report);
    }
}

fn collect_dialogue_line(
    line: &CstLine<'_>,
    flow: &str,
    scopes: &[ScopedLine],
    line_counters: &mut BTreeMap<String, usize>,
    report: &mut IdContextReport,
) {
    let Some(dialogue_head) = dialogue_head(line.trimmed()) else {
        return;
    };
    let Some(speaker) = DialogueSpeakerSlug::from_callee(dialogue_head.callee) else {
        return;
    };
    let scope_names = scopes
        .iter()
        .map(|scope| scope.name.as_str())
        .collect::<Vec<_>>();
    let normalized_id = match dialogue_head.option("id") {
        Some(option) => normalized_line_option_id(
            option.value,
            DialogueIdFamily::Line,
            flow,
            &speaker,
            &scope_names,
        ),
        None => Some(next_generated_line_id(
            flow,
            &speaker,
            &scope_names,
            line_counters,
        )),
    };

    for option in &dialogue_head.options {
        let expected_family = if option.name == "id" {
            DialogueIdFamily::Line
        } else {
            DialogueIdFamily::Text
        };
        let Some(normalized) =
            normalized_line_option_id(option.value, expected_family, flow, &speaker, &scope_names)
        else {
            continue;
        };
        if let Some(options_start) = dialogue_head.options_start
            && parse_relative_materialization(option.value).is_some()
        {
            let start =
                line.start() + leading_len(line.text()) + options_start + option.relative_start;
            report.push(IdContextEntry::replace(
                if option.name == "id" {
                    IdContextKind::DialogueLineId
                } else {
                    IdContextKind::DialogueTextKey
                },
                TextRange::new(start, start + option.value.len()),
                normalized,
            ));
        }
    }

    let Some(line_id) = normalized_id else {
        // An authored ID from another family is a compiler error. Do not
        // materialize a text key from an unrelated generated identity.
        return;
    };
    let text_key = match dialogue_head.option("text_key") {
        Some(option) => normalized_line_option_id(
            option.value,
            DialogueIdFamily::Text,
            flow,
            &speaker,
            &scope_names,
        ),
        None => DialogueLineId::parse(&line_id).map(DialogueLineId::generated_text_key),
    };
    let options = missing_line_options(&dialogue_head, &line_id, text_key.as_deref());
    if !options.is_empty() {
        let start = line.start() + leading_len(line.text()) + dialogue_head.missing_options_insert;
        report.push(IdContextEntry::insert_dialogue_options(
            TextRange::new(start, start),
            dialogue_head.options_start.is_some(),
            dialogue_head.options_has_any,
            options,
        ));
    }
}

fn dialogue_start_offsets(tree: &TypedSyntaxTree) -> BTreeSet<usize> {
    let mut starts = BTreeSet::new();
    for item in tree.items() {
        match item {
            Item::Flow(flow) => collect_dialogue_starts(flow.body(), &mut starts),
            Item::FlowItem(item) => {
                collect_dialogue_starts(std::slice::from_ref(item.as_ref()), &mut starts);
            }
            _ => {}
        }
    }
    starts
}

fn collect_dialogue_starts(items: &[FlowItem], starts: &mut BTreeSet<usize>) {
    for item in items {
        match item {
            FlowItem::SpeakerLine(line) => {
                starts.insert(line.range().start());
            }
            FlowItem::ContentCall(call) => {
                starts.insert(call.range().start());
            }
            FlowItem::If(block) => {
                collect_dialogue_starts(block.body(), starts);
                collect_dialogue_starts(block.else_body(), starts);
            }
            FlowItem::IfLet(block) => {
                collect_dialogue_starts(block.body(), starts);
                collect_dialogue_starts(block.else_body(), starts);
            }
            FlowItem::Match(block) => {
                for arm in block.arms() {
                    collect_dialogue_starts(arm.body(), starts);
                }
            }
            FlowItem::Loop(block) => collect_dialogue_starts(block.body(), starts),
            FlowItem::While(block) => collect_dialogue_starts(block.body(), starts),
            FlowItem::WhileLet(block) => collect_dialogue_starts(block.body(), starts),
            FlowItem::For(block) => collect_dialogue_starts(block.body(), starts),
            FlowItem::Select(block) => {
                for branch in block.branches() {
                    collect_dialogue_starts(branch.body(), starts);
                }
            }
            FlowItem::BorrowBlock(block) => collect_dialogue_starts(block.body(), starts),
            FlowItem::SourceLocale(block) => collect_dialogue_starts(block.body(), starts),
            FlowItem::Scope(block) => collect_dialogue_starts(block.body(), starts),
            FlowItem::AwaitWith(await_with) => {
                for branch in await_with.branches() {
                    collect_dialogue_starts(branch.body(), starts);
                }
            }
            FlowItem::Stmt(_) | FlowItem::Choice(_) | FlowItem::Include(_) | FlowItem::Raw(_) => {}
        }
    }
}

fn update_dialogue_context(
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
    if let Some(fence) = parse_flat_fence(trimmed) {
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

impl<'a> DialogueHead<'a> {
    fn option(&self, name: &str) -> Option<&LineIdOption<'a>> {
        self.options.iter().find(|option| option.name == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LineIdOption<'a> {
    name: &'a str,
    value: &'a str,
    relative_start: usize,
}

fn dialogue_head(trimmed: &str) -> Option<DialogueHead<'_>> {
    if let Some(fence) = parse_flat_fence(trimmed)
        && !fence.close
        && fence.kind == "line"
    {
        return dialogue_head_from_call_head(fence.head, fence.head_start);
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
        || (callee.starts_with('@') && !callee.starts_with("@<"))
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

fn missing_line_options(
    head: &DialogueHead<'_>,
    line_id: &str,
    text_key: Option<&str>,
) -> Vec<IdContextOption> {
    let mut missing = Vec::new();
    if head.option("id").is_none() {
        missing.push(IdContextOption::new("id", line_id));
    }
    if head.option("text_key").is_none()
        && let Some(text_key) = text_key
    {
        missing.push(IdContextOption::new("text_key", text_key));
    }
    missing
}

fn declaration_id_token<'a>(trimmed: &'a str, keyword: &str) -> Option<(usize, &'a str)> {
    if let Some(token) = keyword_id_token(trimmed, keyword) {
        return Some(token);
    }
    let pub_prefix = "pub ";
    let rest = trimmed.strip_prefix(pub_prefix)?;
    leading_id_token(rest).map(|(start, id)| (pub_prefix.len() + start, id))
}

fn keyword_id_token<'a>(trimmed: &'a str, keyword: &str) -> Option<(usize, &'a str)> {
    let rest = trimmed.strip_prefix(keyword)?;
    leading_id_token(rest).map(|(start, id)| (keyword.len() + start, id))
}

fn leading_id_token(rest: &str) -> Option<(usize, &str)> {
    let trimmed_rest = rest.trim_start();
    let id = trimmed_rest.split_whitespace().next()?;
    id.starts_with('@')
        .then_some((rest.len() - trimmed_rest.len(), id))
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

fn normalized_line_option_id(
    raw: &str,
    family: DialogueIdFamily,
    flow: &str,
    speaker: &DialogueSpeakerSlug,
    scopes: &[&str],
) -> Option<String> {
    let absolute_prefix = format!("@{}.", family.prefix());
    if let Some(body) = raw.strip_prefix(&absolute_prefix) {
        return Some(format!("{}.{body}", family.prefix()));
    }
    let (relative, explicit_family) = parse_relative_materialization(raw)?;
    if explicit_family.is_some_and(|explicit| explicit != family.prefix()) {
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

fn scoped_id(
    family: DialogueIdFamily,
    flow: &str,
    speaker: &DialogueSpeakerSlug,
    scopes: &[&str],
    suffix: &str,
) -> String {
    let mut parts = vec![family.prefix(), flow, speaker.as_str()];
    parts.extend(scopes.iter().copied());
    parts.push(suffix);
    parts.join(".")
}

fn next_generated_line_id(
    flow: &str,
    speaker: &DialogueSpeakerSlug,
    scopes: &[&str],
    counters: &mut BTreeMap<String, usize>,
) -> String {
    let mut parts = vec![DialogueIdFamily::Line.prefix(), flow, speaker.as_str()];
    parts.extend(scopes.iter().copied());
    let prefix = parts.join(".");
    let next = counters.entry(prefix.clone()).or_insert(0);
    *next += 1;
    format!("{prefix}.{next:03}")
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
            let leading = leading_len(value_source);
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
    let leading = leading_len(item);
    let trailing = item.len() - item.trim_end().len();
    let start = range.start + leading;
    let end = range.end.saturating_sub(trailing);
    if start < end {
        ranges.push(start..end);
    }
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

fn nonempty_identifier(source: &str) -> Option<String> {
    let name = source.trim();
    (!name.is_empty() && is_identifier(name)).then(|| name.to_owned())
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn leading_len(source: &str) -> usize {
    source.len() - source.trim_start().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lower::lower_to_hir, model::HirFlowItem};

    #[test]
    fn collects_declaration_choice_and_dialogue_materialization() {
        let source = "flow @.opening opening {\n    choice @.first {\n        @.listen \"Listen\" -> @flow.next\n    }\n    alice:\n        Hi[p]\n}\ntest @.smoke scenario {}\n";
        let report = collect_id_context(source);
        let replacements = report
            .entries()
            .iter()
            .filter(|entry| {
                matches!(
                    entry.materialization(),
                    IdContextMaterialization::Replace { .. }
                )
            })
            .count();
        assert_eq!(replacements, 4);
        assert!(report.entries().iter().any(|entry| {
            matches!(
                entry.materialization(),
                IdContextMaterialization::InsertDialogueOptions { options, .. }
                    if options.iter().any(|option| option.normalized() == "say.opening.alice.001")
            )
        }));
    }

    #[test]
    fn flat_scope_changes_generated_dialogue_ids() {
        let source = "flow @flow.opening opening {\n=== scope rain ===\n=== line 地の文 ===\n雨。[p]\n=== /line ===\n=== /scope ===\n}\n";
        let report = collect_id_context(source);
        assert!(report.entries().iter().any(|entry| {
            matches!(
                entry.materialization(),
                IdContextMaterialization::InsertDialogueOptions { options, .. }
                    if options.iter().any(|option| option.normalized() == "say.opening.narrator.rain.001")
            )
        }));
    }

    #[test]
    fn source_materialization_matches_hir_dialogue_identity_normalization() {
        let source = r"
flow @flow.Opening Opening {
    Alice:
        First[p]
    Alice.say[
        Second[p]
    ]
    @<character.Alice>.say[
        Third[p]
    ]
    Narration:
        Fourth[p]
    地:
        Fifth[p]
}
";
        let parsed = parse_source(source);
        assert_eq!(parsed.errors(), &[]);
        let hir = lower_to_hir(parsed.typed_tree()).expect("dialogue source lowers");
        let lowered = hir.flows()[0]
            .body()
            .iter()
            .filter_map(|item| match item {
                HirFlowItem::Dialogue(dialogue) => Some((
                    dialogue.id().expect("generated line ID").body().to_owned(),
                    dialogue
                        .text_key()
                        .expect("generated text key")
                        .body()
                        .to_owned(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        let materialized = collect_id_context(source)
            .entries()
            .iter()
            .filter_map(|entry| match entry.materialization() {
                IdContextMaterialization::InsertDialogueOptions { options, .. } => {
                    let id = options.iter().find(|option| option.name() == "id")?;
                    let text_key = options.iter().find(|option| option.name() == "text_key")?;
                    Some((id.normalized().to_owned(), text_key.normalized().to_owned()))
                }
                IdContextMaterialization::Replace { .. } => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(materialized, lowered);
        assert_eq!(
            lowered,
            [
                ("say.Opening.Alice.001", "text.Opening.Alice.001"),
                ("say.Opening.Alice.002", "text.Opening.Alice.002"),
                ("say.Opening.Alice.003", "text.Opening.Alice.003"),
                ("say.Opening.narrator.001", "text.Opening.narrator.001"),
                ("say.Opening.narrator.002", "text.Opening.narrator.002"),
            ]
            .map(|(id, text_key)| (id.to_owned(), text_key.to_owned()))
        );
    }

    #[test]
    fn invalid_line_family_does_not_materialize_a_phantom_text_key() {
        let source = "flow @flow.opening opening {\n    alice(id=@text.not_a_line): Bad[p]\n}\n";
        let report = collect_id_context(source);
        assert!(!report.entries().iter().any(|entry| {
            matches!(
                entry.materialization(),
                IdContextMaterialization::InsertDialogueOptions { options, .. }
                    if options.iter().any(|option| option.name() == "text_key")
            )
        }));
    }
}
