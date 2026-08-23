// Shared in-memory validation for the structural nominal accepted design.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const EXPECTED_HEAD: &str = "9a5d30d25620541c3f2975d31e04e04e3bc9514c";
pub const SEQUENCE: &str = "Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1";
pub const REQUEST_SHA256: &str = "90ca32e38481fdf152b9ff5aaf145b4514b15ece7a92e989588adaa9b9481fbf";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    pub code: &'static str,
    pub detail: String,
}

#[derive(Clone)]
pub struct Candidate {
    pub root: PathBuf,
    pub files: BTreeMap<String, Vec<u8>>,
    pub contract: Value,
}

impl Candidate {
    pub fn load(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|error| format!("canonicalize design root: {error}"))?;
        let contract_bytes = fs::read(root.join("machine/final_contract.json"))
            .map_err(|error| format!("read machine contract: {error}"))?;
        let contract: Value = serde_json::from_slice(&contract_bytes)
            .map_err(|error| format!("parse machine contract: {error}"))?;
        let required = required_files(&contract)?;
        let mut files = BTreeMap::new();
        for path in required {
            let bytes = fs::read(root.join(&path))
                .map_err(|error| format!("read required file {path}: {error}"))?;
            files.insert(path, bytes);
        }
        Ok(Self {
            root,
            files,
            contract,
        })
    }
}

pub fn validate_artifacts(candidate: &Candidate) -> Vec<Finding> {
    let mut findings = Vec::new();
    check_required_members(candidate, &mut findings);
    check_contract(candidate, &mut findings);
    check_status(candidate, &mut findings);
    check_request(candidate, &mut findings);
    check_schema(candidate, &mut findings);
    check_decisions_and_cuts(candidate, &mut findings);
    check_wire(candidate, &mut findings);
    check_dependencies(candidate, &mut findings);
    check_manifest(candidate, &mut findings);
    check_report(candidate, &mut findings);
    findings
}

fn check_required_members(candidate: &Candidate, findings: &mut Vec<Finding>) {
    let required = candidate
        .contract
        .pointer("/required_files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    for path in required {
        if !candidate.files.contains_key(path) {
            findings.push(finding("ART001", format!("required file missing: {path}")));
        }
    }
}

fn check_contract(candidate: &Candidate, findings: &mut Vec<Finding>) {
    exact_u64(
        &candidate.contract,
        "/schema_version",
        1,
        "VER001",
        findings,
    );
    exact_str(
        &candidate.contract,
        "/sequence",
        SEQUENCE,
        "SEQ001",
        findings,
    );
    exact_str(
        &candidate.contract,
        "/status",
        "READY_FOR_IMPLEMENTATION",
        "STATUS001",
        findings,
    );
    exact_u64(
        &candidate.contract,
        "/open_questions",
        0,
        "OPEN001",
        findings,
    );
    exact_str(
        &candidate.contract,
        "/production_head",
        EXPECTED_HEAD,
        "GIT001",
        findings,
    );
    exact_str(
        &candidate.contract,
        "/request/sha256",
        REQUEST_SHA256,
        "REQ001",
        findings,
    );
    exact_bool(
        &candidate.contract,
        "/selected_model/accepted_runtime_carrier",
        false,
        "MODEL001",
        findings,
    );
    exact_bool(
        &candidate.contract,
        "/selected_model/schema_graph_persisted_in_runtime_plan",
        false,
        "MODEL002",
        findings,
    );
    exact_bool(
        &candidate.contract,
        "/selected_model/source_reconstruction",
        false,
        "MODEL003",
        findings,
    );
    exact_u64(&candidate.contract, "/wire/version", 1, "VER002", findings);
    exact_u64(
        &candidate.contract,
        "/wire/new_runtime_type_tags",
        0,
        "TAG001",
        findings,
    );
    exact_u64(
        &candidate.contract,
        "/wire/new_constant_tags",
        0,
        "TAG002",
        findings,
    );
    exact_bool(
        &candidate.contract,
        "/wire/old_reader",
        false,
        "VER003",
        findings,
    );
    exact_bool(
        &candidate.contract,
        "/wire/migration",
        false,
        "VER004",
        findings,
    );
    exact_bool(
        &candidate.contract,
        "/wire/per_value_version",
        false,
        "VER005",
        findings,
    );

    let blobs = candidate
        .contract
        .pointer("/source_blobs")
        .and_then(Value::as_object);
    if blobs.is_none_or(|rows| rows.len() < 35) {
        findings.push(finding("SRC001", "source blob inventory is incomplete"));
    }
    let ids = string_array(&candidate.contract, "/decision_ids");
    if ids != (1..=8).map(|n| format!("D{n}")).collect::<Vec<_>>() {
        findings.push(finding("DEC001", "decision IDs are not exactly D1..D8"));
    }
    let cuts = string_array(&candidate.contract, "/cuts");
    if cuts != (1..=6).map(|n| format!("C{n}")).collect::<Vec<_>>() {
        findings.push(finding("CUT001", "cuts are not exactly C1..C6"));
    }
}

fn check_status(candidate: &Candidate, findings: &mut Vec<Finding>) {
    if text(candidate, "FINAL_STATUS.md") != Some("READY_FOR_IMPLEMENTATION\n") {
        findings.push(finding("STATUS002", "FINAL_STATUS is not exact"));
    }
    if text(candidate, "OPEN_QUESTIONS.md") != Some("none\n") {
        findings.push(finding("OPEN002", "OPEN_QUESTIONS is not exact none"));
    }
}

fn check_request(candidate: &Candidate, findings: &mut Vec<Finding>) {
    match candidate.files.get("REQUEST.md") {
        Some(bytes) if sha256_hex(bytes) == REQUEST_SHA256 => {}
        Some(_) => findings.push(finding("REQ002", "REQUEST.md hash differs")),
        None => findings.push(finding("REQ002", "REQUEST.md is absent")),
    }
}

fn check_schema(candidate: &Candidate, findings: &mut Vec<Finding>) {
    let Some(schema) = text(candidate, "SCHEMAS.md") else {
        findings.push(finding("SCH001", "SCHEMAS.md is absent or non-UTF8"));
        return;
    };
    let tokens = candidate
        .contract
        .pointer("/schema_tokens")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    for token in tokens {
        if !schema.contains(token) {
            findings.push(finding("SCH002", format!("missing schema token {token}")));
        }
    }
    for token in [
        "RuntimeValue::NominalRecord",
        "RuntimeValue::Variant",
        "one-field tuple is never flattened",
    ] {
        if !all_design_text(candidate).contains(token) {
            findings.push(finding(
                "SCH003",
                format!("missing design invariant {token}"),
            ));
        }
    }
}

fn check_decisions_and_cuts(candidate: &Candidate, findings: &mut Vec<Finding>) {
    let decisions = text(candidate, "DECISION_REGISTER.md").unwrap_or_default();
    for id in 1..=8 {
        let token = format!("| D{id} ");
        if decisions.matches(&token).count() != 1 {
            findings.push(finding("DEC002", format!("{token} must occur once")));
        }
    }
    let cuts = text(candidate, "CUTS_TESTS_AND_DELETION.md").unwrap_or_default();
    for id in 1..=6 {
        let token = format!("## C{id} -");
        if cuts.matches(&token).count() != 1 {
            findings.push(finding("CUT002", format!("{token} must occur once")));
        }
    }
}

fn check_wire(candidate: &Candidate, findings: &mut Vec<Finding>) {
    let wire = text(candidate, "WIRE_AND_RESTORE.md").unwrap_or_default();
    for golden in string_array(&candidate.contract, "/wire/goldens") {
        if !wire.contains(&golden) {
            findings.push(finding("WIRE001", format!("missing golden {golden}")));
        }
    }
    for (pointer, expected) in [
        ("/wire/runtime_type_tags/Tuple", 10),
        ("/wire/runtime_type_tags/Record", 12),
        ("/wire/runtime_type_tags/Variant", 13),
        ("/wire/runtime_type_tags/Nominal", 22),
        ("/wire/runtime_type_tags/Opaque", 23),
        ("/wire/runtime_type_tags/NominalRecord", 24),
        ("/wire/constant_tags/Record", 12),
        ("/wire/constant_tags/Variant", 13),
        ("/wire/schema_transcript_tags/Tuple", 26),
        ("/wire/schema_transcript_tags/Result", 27),
        ("/wire/schema_transcript_tags/RecordValue", 28),
        ("/wire/schema_transcript_tags/ExactOpaque", 29),
        ("/wire/schema_transcript_tags/NominalRef", 30),
        ("/wire/checked_type_transcript_tags/Record", 22),
        ("/wire/record_shape_tags/Unit", 0),
        ("/wire/record_shape_tags/Tuple", 1),
        ("/wire/record_shape_tags/Record", 2),
        ("/wire/record_shape_tags/Newtype", 3),
        ("/wire/canonical_value_tags/Tuple", 11),
        ("/wire/canonical_value_tags/Record", 13),
        ("/wire/canonical_value_tags/Variant", 14),
        ("/wire/canonical_value_tags/NominalRecord", 15),
    ] {
        exact_u64(&candidate.contract, pointer, expected, "TAG003", findings);
    }
}

fn check_dependencies(candidate: &Candidate, findings: &mut Vec<Finding>) {
    let deps = text(candidate, "DEPENDENCIES.md").unwrap_or_default();
    for token in [
        "Core does not import sema",
        "graph is then dropped",
        "Restore reads only the",
    ] {
        if !deps.contains(token) {
            findings.push(finding(
                "DEP001",
                format!("missing dependency rule {token}"),
            ));
        }
    }
}

fn check_report(candidate: &Candidate, findings: &mut Vec<Finding>) {
    let report = text(candidate, "VALIDATION_REPORT.md").unwrap_or_default();
    for token in [
        "Overall: PASS",
        "positive validator: PASS",
        "negative self-tests: PASS",
    ] {
        if !report.contains(token) {
            findings.push(finding("VAL001", format!("report lacks {token}")));
        }
    }
}

fn check_manifest(candidate: &Candidate, findings: &mut Vec<Finding>) {
    let Some(manifest) = text(candidate, "MANIFEST.sha256") else {
        findings.push(finding("MAN001", "manifest missing/non-UTF8"));
        return;
    };
    let mut rows = BTreeMap::new();
    for (index, line) in manifest.lines().enumerate() {
        let Some((hash, path)) = line.split_once("  ") else {
            findings.push(finding(
                "MAN002",
                format!("bad manifest line {}", index + 1),
            ));
            continue;
        };
        if rows.insert(path.to_owned(), hash.to_owned()).is_some() {
            findings.push(finding("MAN003", format!("duplicate manifest path {path}")));
        }
    }
    let expected = candidate
        .files
        .keys()
        .filter(|path| path.as_str() != "MANIFEST.sha256")
        .cloned()
        .collect::<BTreeSet<_>>();
    if rows.keys().cloned().collect::<BTreeSet<_>>() != expected {
        findings.push(finding(
            "MAN004",
            "manifest membership differs from required files",
        ));
    }
    for path in expected {
        let Some(bytes) = candidate.files.get(&path) else {
            continue;
        };
        if rows
            .get(&path)
            .is_none_or(|hash| hash != &sha256_hex(bytes))
        {
            findings.push(finding("MAN005", format!("manifest hash mismatch {path}")));
        }
    }
}

fn required_files(contract: &Value) -> Result<Vec<String>, String> {
    contract
        .pointer("/required_files")
        .and_then(Value::as_array)
        .ok_or_else(|| "machine contract lacks required_files".to_owned())?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| "required_files contains non-string".to_owned())
        })
        .collect()
}

fn exact_str(
    value: &Value,
    pointer: &str,
    expected: &str,
    code: &'static str,
    findings: &mut Vec<Finding>,
) {
    if value.pointer(pointer).and_then(Value::as_str) != Some(expected) {
        findings.push(finding(code, format!("{pointer} differs from {expected}")));
    }
}

fn exact_u64(
    value: &Value,
    pointer: &str,
    expected: u64,
    code: &'static str,
    findings: &mut Vec<Finding>,
) {
    if value.pointer(pointer).and_then(Value::as_u64) != Some(expected) {
        findings.push(finding(code, format!("{pointer} differs from {expected}")));
    }
}

fn exact_bool(
    value: &Value,
    pointer: &str,
    expected: bool,
    code: &'static str,
    findings: &mut Vec<Finding>,
) {
    if value.pointer(pointer).and_then(Value::as_bool) != Some(expected) {
        findings.push(finding(code, format!("{pointer} differs from {expected}")));
    }
}

fn string_array(value: &Value, pointer: &str) -> Vec<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn text<'a>(candidate: &'a Candidate, path: &str) -> Option<&'a str> {
    candidate
        .files
        .get(path)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
}

fn all_design_text(candidate: &Candidate) -> String {
    candidate
        .files
        .iter()
        .filter(|(path, _)| path.ends_with(".md"))
        .filter_map(|(_, bytes)| std::str::from_utf8(bytes).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn finding(code: &'static str, detail: impl Into<String>) -> Finding {
    Finding {
        code,
        detail: detail.into(),
    }
}
