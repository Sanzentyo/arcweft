use crate::ast::common::TextRange;
use crate::ast::ids::EntityRef;
use crate::ast::items::Attribute;
use crate::ast::proof::{BenchItem, ProofClause, ProofItem, ProofTrust, TestItem, TestKind};
use crate::cst::{
    split_leading_ident, split_top_level_punctuation, split_top_level_punctuation_once,
};
use crate::expr::{DecodedStringLiteral, Expr, Literal, parse_expr};

use super::headers::{parse_decl_identity_and_name, parse_required_id_ref, simple_error};
use super::{
    Parser,
    recovery::{ParseError, ParseErrorKind, RecoverySuggestion},
};

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
        let head = head.trim();
        let rest = head.strip_prefix("proof")?.trim_start();
        let explicit_id = rest.starts_with('@');
        let head_start = start_line.start + start_line.text.as_ref().find(head).unwrap_or(0);
        let name_start = head_start + head.find(rest).unwrap_or(head.len());
        let (entity, name, rest) =
            parse_decl_identity_and_name(rest, "proof", name_start, &mut self.errors)?;
        let entity = entity.unwrap_or_else(|| {
            EntityRef::module_scoped_declaration(
                "proof",
                name,
                None,
                TextRange::new(name_start, name_start + name.len()),
            )
        });
        let id = crate::ast::ids::IdRef::absolute(entity);
        if !rest.trim().is_empty() {
            self.push_error(
                TextRange::new(start_line.start, start_line.start + head.len()),
                "unexpected text after proof id",
                ["{"],
                Some(rest.trim()),
                ["move proof clauses into the proof body"],
            );
        }
        let trust = parse_proof_trust(&attrs, &mut self.errors)?;
        let clauses = parse_proof_clauses(&body);
        Some(ProofItem::new(
            id,
            name.to_owned(),
            explicit_id,
            attrs,
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

fn parse_proof_trust(attrs: &[Attribute], errors: &mut Vec<ParseError>) -> Option<ProofTrust> {
    if attrs.is_empty() {
        return Some(ProofTrust::Verified);
    }

    let trusted_attrs = attrs
        .iter()
        .filter(|attr| attr.name() == "verify.trusted")
        .collect::<Vec<_>>();
    let unsupported = report_unsupported_proof_attributes(attrs, errors);
    let duplicated = report_duplicate_trusted_attributes(&trusted_attrs, errors);
    let Some(attr) = trusted_attrs.first().copied() else {
        return (!unsupported).then_some(ProofTrust::Verified);
    };
    let reason = parse_trusted_reason(attr, errors);

    (!unsupported && !duplicated)
        .then_some(reason)
        .flatten()
        .map(|reason| ProofTrust::Trusted {
            reason,
            attribute_range: *attr.range(),
        })
}

fn report_unsupported_proof_attributes(attrs: &[Attribute], errors: &mut Vec<ParseError>) -> bool {
    let mut unsupported = false;
    for attr in attrs {
        if !matches!(attr.name(), "verify.trusted" | "allow" | "generated") {
            errors.push(simple_error(
                attr.range().start(),
                attr.range().end() - attr.range().start(),
                "proof attributes support `verify.trusted`, `allow`, and `generated`",
                "#[verify.trusted(reason = \"external review\")]",
            ));
            unsupported = true;
        }
    }
    unsupported
}

fn report_duplicate_trusted_attributes(
    trusted_attrs: &[&Attribute],
    errors: &mut Vec<ParseError>,
) -> bool {
    let Some(first) = trusted_attrs.first() else {
        return false;
    };
    for duplicate in &trusted_attrs[1..] {
        errors.push(
            proof_trust_error(
                ParseErrorKind::ProofTrustedDuplicate,
                duplicate,
                "a proof can carry only one `verify.trusted` attribute",
                Some(duplicate.name()),
            )
            .with_related(
                *first.range(),
                Some("the first `verify.trusted` attribute is here".to_owned()),
            ),
        );
    }
    trusted_attrs.len() > 1
}

fn parse_trusted_reason(attr: &Attribute, errors: &mut Vec<ParseError>) -> Option<String> {
    let Some(args) = attr.args() else {
        errors.push(proof_trust_error(
            ParseErrorKind::ProofTrustedReasonMissing,
            attr,
            "trusted proof requires a `reason` argument",
            None,
        ));
        return None;
    };

    let mut valid = true;
    let parts = split_top_level_punctuation(args, ',');
    let mut reason = None;
    let mut reason_seen = false;
    for part in parts {
        let part = part.trim();
        let Some((name, value)) = split_top_level_punctuation_once(part, '=') else {
            errors.push(proof_trust_error(
                ParseErrorKind::ProofTrustedPositionalArgument,
                attr,
                "trusted proof arguments must be named",
                (!part.is_empty()).then_some(part),
            ));
            valid = false;
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            errors.push(proof_trust_error(
                ParseErrorKind::ProofTrustedPositionalArgument,
                attr,
                "trusted proof arguments must be named",
                Some(part),
            ));
            valid = false;
            continue;
        }
        if name != "reason" {
            errors.push(proof_trust_error(
                ParseErrorKind::ProofTrustedUnknownArgument,
                attr,
                "trusted proof accepts only the `reason` argument",
                Some(name),
            ));
            valid = false;
            continue;
        }
        if reason_seen {
            errors.push(proof_trust_error(
                ParseErrorKind::ProofTrustedReasonDuplicate,
                attr,
                "trusted proof declares `reason` more than once",
                Some(name),
            ));
            valid = false;
            continue;
        }
        reason_seen = true;

        let Ok(Expr::Literal(Literal::String(value))) = parse_expr(value.trim()) else {
            errors.push(proof_trust_error(
                ParseErrorKind::ProofTrustedReasonNotString,
                attr,
                "trusted proof reason must be a string literal",
                Some(value.trim()),
            ));
            valid = false;
            continue;
        };
        let value = DecodedStringLiteral::from_raw_body(&value);
        if value.as_str().trim().is_empty() {
            errors.push(proof_trust_error(
                ParseErrorKind::ProofTrustedReasonEmpty,
                attr,
                "trusted proof reason must contain non-whitespace text",
                Some(value.as_str()),
            ));
            valid = false;
            continue;
        }
        reason = Some(value.into_string());
    }

    valid.then_some(reason).flatten()
}

fn proof_trust_error(
    kind: ParseErrorKind,
    attr: &Attribute,
    message: &str,
    found: Option<&str>,
) -> ParseError {
    ParseError::new_with_kind(
        kind,
        *attr.range(),
        vec!["#[verify.trusted(reason = \"external review\")]".to_owned()],
        found.map(str::to_owned),
        message.to_owned(),
        vec![RecoverySuggestion::new(
            "use one proof attribute with exactly one nonempty string reason",
        )],
    )
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
