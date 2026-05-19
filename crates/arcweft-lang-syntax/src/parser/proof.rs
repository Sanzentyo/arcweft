use crate::ast::common::TextRange;
use crate::ast::proof::{BenchItem, ProofClause, ProofItem, TestItem, TestKind, TrustedAxiomItem};
use crate::cst::split_leading_ident;

use super::headers::{parse_required_id_ref, simple_error};
use super::{Parser, recovery::ParseError};

impl Parser {
    pub(super) fn parse_proof_item(&mut self) -> Option<ProofItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing proof item",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the proof body"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("proof")?.trim();
        let (id, rest) = parse_required_id_ref(rest, start_line.start, &mut self.errors)?;
        if !rest.trim().is_empty() {
            self.push_error(
                TextRange::new(start_line.start, start_line.start + head.len()),
                "unexpected text after proof id",
                ["{"],
                Some(rest.trim()),
                ["move proof clauses into the proof body"],
            );
        }
        let clauses = parse_proof_clauses(&body);
        Some(ProofItem::new(
            id,
            body,
            clauses,
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_trusted_axiom_item(&mut self) -> Option<TrustedAxiomItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing trusted axiom item",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the trusted axiom body"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("trusted axiom")?.trim();
        let (id, rest) = parse_required_id_ref(rest, start_line.start, &mut self.errors)?;
        if !rest.trim().is_empty() {
            self.push_error(
                TextRange::new(start_line.start, start_line.start + head.len()),
                "unexpected text after trusted axiom id",
                ["{"],
                Some(rest.trim()),
                ["move axiom metadata into the trusted axiom body"],
            );
        }
        Some(TrustedAxiomItem::new(
            id,
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_test_item(&mut self) -> Option<TestItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing test item",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the test body"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("test")?.trim();
        let (id, rest) = parse_required_id_ref(rest, start_line.start, &mut self.errors)?;
        let kind = parse_test_kind(rest.trim(), start_line.start, head.len(), &mut self.errors)?;
        Some(TestItem::new(
            id,
            kind,
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_bench_item(&mut self) -> Option<BenchItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing bench item",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the bench body"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("bench")?.trim();
        let (id, rest) = parse_required_id_ref(rest, start_line.start, &mut self.errors)?;
        if !rest.trim().is_empty() {
            self.push_error(
                TextRange::new(start_line.start, start_line.start + head.len()),
                "unexpected text after bench id",
                ["{"],
                Some(rest.trim()),
                ["move bench configuration into the bench body"],
            );
        }
        Some(BenchItem::new(
            id,
            body,
            TextRange::new(start_line.start, end),
        ))
    }
}

pub(super) fn parse_proof_clauses(body: &str) -> Vec<ProofClause> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| {
            !line.starts_with("//") && !line.starts_with("///") && !line.starts_with('#')
        })
        .map(parse_proof_clause)
        .collect()
}

fn parse_proof_clause(line: &str) -> ProofClause {
    if let Some(source) = line.strip_prefix("requires ") {
        return ProofClause::Requires {
            source: source.trim().to_owned(),
            lifetime_targets: collect_lifetime_targets(source),
        };
    }
    if let Some(source) = line.strip_prefix("ensures ") {
        return ProofClause::Ensures {
            source: source.trim().to_owned(),
            lifetime_targets: collect_lifetime_targets(source),
        };
    }
    if let Some(source) = line.strip_prefix("check ") {
        return ProofClause::Check {
            source: source.trim().to_owned(),
            lifetime_targets: collect_lifetime_targets(source),
        };
    }
    if let Some(source) = line.strip_prefix("assume ") {
        return ProofClause::Assume {
            source: source.trim().to_owned(),
            reason: named_clause_value(source, "reason"),
            axiom: named_clause_value(source, "axiom").or_else(|| find_axiom_ref(source)),
        };
    }
    if let Some(source) = line.strip_prefix("use ")
        && let Some(id) = find_axiom_ref(source)
    {
        return ProofClause::UseAxiom { id };
    }
    ProofClause::Raw {
        source: line.to_owned(),
    }
}

fn named_clause_value(source: &str, name: &str) -> Option<String> {
    let (_, value) = source.split_once(&format!("{name} ="))?;
    let value = value
        .split([',', '}'])
        .next()
        .unwrap_or(value)
        .trim()
        .trim_matches('"')
        .trim_start_matches('@')
        .to_owned();
    Some(value).filter(|value| !value.is_empty())
}

fn find_axiom_ref(source: &str) -> Option<String> {
    let start = source.find("@axiom.")?;
    let rest = &source[start + 1..];
    let end = rest
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | ':' | '-')))
        .unwrap_or(rest.len());
    Some(rest[..end].to_owned()).filter(|id| id != "axiom.")
}

fn collect_lifetime_targets(source: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\'' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'.' | b':'))
        {
            index += 1;
        }
        if index > start + 1 {
            let target = source[start..index].to_owned();
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    targets
}

pub(super) fn parse_test_kind(
    rest: &str,
    base: usize,
    head_len: usize,
    errors: &mut Vec<ParseError>,
) -> Option<TestKind> {
    let Some((kind, trailing)) = split_leading_ident(rest) else {
        errors.push(simple_error(
            base,
            head_len,
            "test item is missing a test kind",
            "test @test.id scenario { ... }",
        ));
        return None;
    };
    if !trailing.trim().is_empty() {
        errors.push(simple_error(
            base,
            head_len,
            "unexpected text after test kind",
            "test @test.id scenario { ... }",
        ));
        return None;
    }
    Some(match kind {
        "scenario" => TestKind::Scenario,
        "visual" => TestKind::Visual,
        "audio" => TestKind::Audio,
        "fixture" => TestKind::Fixture,
        custom => TestKind::Custom(custom.to_owned()),
    })
}
