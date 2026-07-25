//! Typed ID context collected from parsed Arcweft source.
//!
//! This module is the shared syntax-to-HIR bridge for ID materialization used
//! by formatter tooling, CLI commands, and LSP code actions. It intentionally
//! returns typed entries instead of source-specific text edits.

use arcweft_lang_syntax::{
    ast::common::TextRange, cst::cst_lines, parser::parse_source, source::ParsedSource,
};

/// ID family attached to a materialized source position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdContextKind {
    /// A top-level declaration ID such as `flow`, `test`, or `bench`.
    Declaration { family: &'static str },
    /// A `choice` block ID.
    Choice,
    /// A choice option ID scoped below the current choice.
    ChoiceOption,
}

/// Source operation represented by one ID-context entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdContextMaterialization {
    /// Replace the relative ID at `range` with `@{normalized}`.
    Replace {
        range: TextRange,
        normalized: String,
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

fn leading_len(source: &str) -> usize {
    source.len() - source.trim_start().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_only_declaration_and_choice_materialization() {
        let source = "flow @.opening opening {\n    choice @.first {\n        @.listen \"Listen\" -> @flow.next\n    }\n    alice:\n        Hi[p]\n}\ntest @.smoke scenario {}\n";
        let report = collect_id_context(source);
        assert_eq!(report.entries().len(), 4);
        assert!(matches!(
            report.entries()[0].kind(),
            IdContextKind::Declaration { family: "flow" }
        ));
        assert!(matches!(
            report.entries()[1].kind(),
            IdContextKind::Declaration { family: "test" }
        ));
        assert_eq!(report.entries()[2].kind(), IdContextKind::Choice);
        assert_eq!(report.entries()[3].kind(), IdContextKind::ChoiceOption);
    }
}
