use crate::ast::common::TextRange;
use crate::ast::items::Attribute;
use crate::ast::proof::{BenchItem, ProofClause, ProofItem, ProofTrust, TestItem, TestKind};
use crate::cst::{
    split_leading_ident, split_top_level_punctuation, split_top_level_punctuation_once,
};
use crate::expr::{Expr, Literal, parse_expr};

use super::headers::{parse_required_id_ref, simple_error};
use super::{Parser, recovery::ParseError};

impl Parser<'_> {
    pub(super) fn parse_proof_item(&mut self) -> Option<ProofItem> {
        let attrs = self.take_pending_attrs();
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
        let trust = parse_proof_trust(attrs, &mut self.errors)?;
        let clauses = parse_proof_clauses(&body);
        Some(ProofItem::new(
            id,
            trust,
            body.into_owned(),
            clauses,
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
            body.into_owned(),
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
            body.into_owned(),
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
            proof: named_clause_value(source, "proof").or_else(|| find_proof_ref(source)),
        };
    }
    if let Some(source) = line.strip_prefix("use ")
        && let Some(id) = find_proof_ref(source)
    {
        return ProofClause::UseProof { id };
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

fn find_proof_ref(source: &str) -> Option<String> {
    let start = source.find("@proof.")?;
    let rest = &source[start + 1..];
    let end = rest
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | ':' | '-')))
        .unwrap_or(rest.len());
    Some(rest[..end].to_owned()).filter(|id| id != "proof.")
}

fn parse_proof_trust(attrs: Vec<Attribute>, errors: &mut Vec<ParseError>) -> Option<ProofTrust> {
    if attrs.is_empty() {
        return Some(ProofTrust::Verified);
    }
    if attrs.len() != 1 || attrs[0].name() != "verify.trusted" {
        for attr in attrs {
            errors.push(simple_error(
                attr.range().start(),
                attr.range().end() - attr.range().start(),
                "proof attributes only support `verify.trusted`",
                "#[verify.trusted(reason = \"external review\")]",
            ));
        }
        return None;
    }
    let attr = &attrs[0];
    let Some(args) = attr.args() else {
        return invalid_proof_trust(attr, errors, "trusted proof requires a reason");
    };
    let parts = split_top_level_punctuation(args, ',');
    let Some((name, value)) = parts
        .as_slice()
        .first()
        .filter(|_| parts.len() == 1)
        .and_then(|arg| split_top_level_punctuation_once(arg, '='))
    else {
        return invalid_proof_trust(attr, errors, "trusted proof accepts exactly one reason");
    };
    let Ok(Expr::Literal(Literal::String(reason))) = parse_expr(value.trim()) else {
        return invalid_proof_trust(
            attr,
            errors,
            "trusted proof reason must be a string literal",
        );
    };
    if name.trim() != "reason" || reason.trim().is_empty() {
        return invalid_proof_trust(attr, errors, "trusted proof requires a nonempty reason");
    }
    Some(ProofTrust::Trusted { reason })
}

fn invalid_proof_trust(
    attr: &Attribute,
    errors: &mut Vec<ParseError>,
    message: &str,
) -> Option<ProofTrust> {
    errors.push(simple_error(
        attr.range().start(),
        attr.range().end() - attr.range().start(),
        message,
        "#[verify.trusted(reason = \"external review\")]",
    ));
    None
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
