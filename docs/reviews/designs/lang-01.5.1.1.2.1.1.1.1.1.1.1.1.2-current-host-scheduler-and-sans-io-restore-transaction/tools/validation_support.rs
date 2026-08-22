use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub const SEQUENCE: &str = "Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2";
pub const EXPECTED_HEAD: &str = "9168c8ac7285c6b44f29018626a0e7c1b0059796";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    pub code: &'static str,
    pub detail: String,
}

impl Finding {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

#[derive(Clone)]
pub struct Candidate {
    pub files: BTreeMap<String, Vec<u8>>,
    pub contract: Value,
}

impl Candidate {
    pub fn load(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|error| format!("canonicalize {}: {error}", root.display()))?;
        let mut files = BTreeMap::new();
        collect_files(&root, &root, &mut files)?;
        let contract_bytes = files
            .get("machine/final_contract.json")
            .ok_or_else(|| "missing machine/final_contract.json".to_owned())?;
        let contract = serde_json::from_slice(contract_bytes)
            .map_err(|error| format!("parse machine/final_contract.json: {error}"))?;
        Ok(Self { files, contract })
    }

    pub fn text(&self, path: &str) -> Option<&str> {
        std::str::from_utf8(self.files.get(path)?).ok()
    }

    pub fn set_text(&mut self, path: &str, text: String) {
        self.files.insert(path.to_owned(), text.into_bytes());
    }
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("read_dir {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read_dir entry {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("file_type {}: {error}", path.display()))?;
        if kind.is_symlink() {
            return Err(format!(
                "symlink is forbidden in design package: {}",
                path.display()
            ));
        }
        if kind.is_dir() {
            collect_files(root, &path, files)?;
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("strip_prefix {}: {error}", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes =
                fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
            files.insert(relative, bytes);
        }
    }
    Ok(())
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn string_at<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer)?.as_str()
}

fn bool_at(value: &Value, pointer: &str) -> Option<bool> {
    value.pointer(pointer)?.as_bool()
}

fn string_array<'a>(value: &'a Value, pointer: &str) -> Option<Vec<&'a str>> {
    value
        .pointer(pointer)?
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect()
}

fn normalized_line(text: &str) -> &str {
    text.trim_end_matches(['\r', '\n'])
}

pub fn validate_artifacts(candidate: &Candidate) -> Vec<Finding> {
    let mut findings = Vec::new();
    let contract = &candidate.contract;

    if contract.pointer("/schema_version").and_then(Value::as_u64) != Some(1) {
        findings.push(Finding::new("CON001", "schema_version must be 1"));
    }
    if string_at(contract, "/sequence") != Some(SEQUENCE) {
        findings.push(Finding::new("CON002", "sequence mismatch"));
    }
    if string_at(contract, "/status") != Some("READY_FOR_IMPLEMENTATION") {
        findings.push(Finding::new("CON003", "machine status mismatch"));
    }
    if contract.pointer("/open_questions").and_then(Value::as_u64) != Some(0) {
        findings.push(Finding::new(
            "CON004",
            "machine open_questions must be zero",
        ));
    }
    if string_at(contract, "/production_head") != Some(EXPECTED_HEAD) {
        findings.push(Finding::new("CON005", "production_head mismatch"));
    }

    match candidate.text("FINAL_STATUS.md") {
        Some(text) if normalized_line(text) == "READY_FOR_IMPLEMENTATION" => {}
        _ => findings.push(Finding::new(
            "STA001",
            "FINAL_STATUS.md must contain exactly READY_FOR_IMPLEMENTATION",
        )),
    }
    match candidate.text("OPEN_QUESTIONS.md") {
        Some(text) if normalized_line(text) == "none" => {}
        _ => findings.push(Finding::new(
            "OPEN001",
            "OPEN_QUESTIONS.md must contain exactly none",
        )),
    }

    let required_files = string_array(contract, "/required_files").unwrap_or_default();
    for path in required_files {
        if !candidate.files.contains_key(path) {
            findings.push(Finding::new(
                "FILE001",
                format!("missing required file {path}"),
            ));
        }
    }

    let request = candidate.files.get("REQUEST.md");
    let expected_request_hash = string_at(contract, "/request/sha256");
    match (request, expected_request_hash) {
        (Some(bytes), Some(expected)) if sha256(bytes) == expected => {}
        _ => findings.push(Finding::new("REQ001", "REQUEST.md hash mismatch")),
    }
    if string_at(contract, "/request/status") != Some("RESOLVED_BY_ACCEPTED_DESIGN")
        || !candidate
            .text("REQUEST.md")
            .is_some_and(|text| text.contains("Status: `RESOLVED_BY_ACCEPTED_DESIGN`"))
    {
        findings.push(Finding::new("REQ002", "request resolution status mismatch"));
    }

    let decisions = candidate.text("DECISION_REGISTER.md").unwrap_or_default();
    for id in string_array(contract, "/decision_ids").unwrap_or_default() {
        if decisions.matches(id).count() != 1 {
            findings.push(Finding::new(
                "DEC001",
                format!("decision {id} must occur exactly once"),
            ));
        }
    }

    let schemas = candidate.text("SCHEMAS.md").unwrap_or_default();
    for token in string_array(contract, "/schema_tokens").unwrap_or_default() {
        if !schemas.contains(token) {
            findings.push(Finding::new(
                "SCH001",
                format!("SCHEMAS.md lacks mirror token {token}"),
            ));
        }
    }

    if string_at(contract, "/task_host/sole_event_drain") != Some("TaskHost::poll_frame")
        || string_at(contract, "/task_host/event_queue_owner")
            != Some("RuntimeTaskScheduler::SchedulerRuntimeState::pending_events")
        || !candidate
            .text("FINAL_DESIGN.md")
            .is_some_and(|text| text.contains("sole event queue") && text.contains("poll_frame"))
    {
        findings.push(Finding::new("EVT001", "single event owner/drain mismatch"));
    }

    let expected_apply = [
        "RuntimeGenerationJournal::apply_after_image:last_fallible",
        "RuntimeTaskScheduler::apply_runtime_after_image:infallible",
        "TaskLaunchAdapter::commit_operation:infallible_canonical_order",
        "move_core_owned_applied_result:infallible",
    ];
    if string_array(contract, "/apply_order").as_deref() != Some(&expected_apply)
        || !candidate
            .text("TRANSACTION_AND_STATE_PROJECTION.md")
            .is_some_and(|text| {
                text.contains("apply_after_image` is the last `Result`")
                    && text.contains("commit_restore, canonical order")
                    && text.contains("return core-built AppliedRuntimeTaskRestore")
            })
    {
        findings.push(Finding::new("TXN001", "apply transcript mismatch"));
    }

    for pointer in [
        "/post_apply/allocation",
        "/post_apply/validation",
        "/post_apply/callback",
        "/post_apply/persistence_io",
        "/post_apply/result_edge",
        "/persistence/durable_prepared_record",
        "/persistence/durable_committed_record",
        "/persistence/wal",
        "/persistence/crash_publication_replay",
        "/wire/old_reader",
        "/wire/migration",
    ] {
        if bool_at(contract, pointer) != Some(false) {
            findings.push(Finding::new(
                "NEG001",
                format!("forbidden capability enabled at {pointer}"),
            ));
        }
    }
    if contract.pointer("/wire/version").and_then(Value::as_u64) != Some(1) {
        findings.push(Finding::new("VER001", "wire version must remain 1"));
    }

    let source = candidate.text("SOURCE_EVIDENCE.md").unwrap_or_default();
    if !source.contains(EXPECTED_HEAD) {
        findings.push(Finding::new(
            "SRC001",
            "source evidence lacks production head",
        ));
    }
    if let Some(blobs) = contract.pointer("/source_blobs").and_then(Value::as_object) {
        for (path, hash) in blobs {
            let Some(hash) = hash.as_str() else {
                findings.push(Finding::new(
                    "SRC002",
                    format!("non-string blob for {path}"),
                ));
                continue;
            };
            if !source.contains(path) || !source.contains(hash) {
                findings.push(Finding::new(
                    "SRC003",
                    format!("source evidence lacks {path} at {hash}"),
                ));
            }
        }
    }

    validate_manifest(candidate, &mut findings);
    findings
}

fn validate_manifest(candidate: &Candidate, findings: &mut Vec<Finding>) {
    let Some(manifest) = candidate.text("MANIFEST.sha256") else {
        findings.push(Finding::new("MAN001", "missing MANIFEST.sha256"));
        return;
    };
    let mut listed = BTreeSet::new();
    for (index, line) in manifest.lines().enumerate() {
        let Some((digest, path)) = line.split_once("  ") else {
            findings.push(Finding::new(
                "MAN002",
                format!("malformed manifest line {}", index + 1),
            ));
            continue;
        };
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || path.is_empty()
            || path.contains('\\')
            || !listed.insert(path.to_owned())
        {
            findings.push(Finding::new(
                "MAN002",
                format!("invalid manifest row for {path}"),
            ));
            continue;
        }
        match candidate.files.get(path) {
            Some(bytes) if sha256(bytes) == digest => {}
            _ => findings.push(Finding::new(
                "MAN003",
                format!("manifest digest mismatch for {path}"),
            )),
        }
    }
    for path in candidate.files.keys() {
        if path != "MANIFEST.sha256" && !listed.contains(path) {
            findings.push(Finding::new(
                "MAN004",
                format!("manifest omits package member {path}"),
            ));
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Mutation {
    FinalStatus,
    OpenQuestion,
    RequestByte,
    ContractStatus,
    DecisionMissing,
    SchemaOwnerMissing,
    EventSecondDrain,
    ApplyOrder,
    EnableWal,
    VersionBump,
    SourceHead,
    RequiredFileMissing,
    ManifestDigest,
}

impl Mutation {
    pub const fn cases() -> &'static [(&'static str, Mutation, &'static str)] {
        &[
            ("final-status", Mutation::FinalStatus, "STA001"),
            ("open-question", Mutation::OpenQuestion, "OPEN001"),
            ("request-byte", Mutation::RequestByte, "REQ001"),
            ("contract-status", Mutation::ContractStatus, "CON003"),
            ("decision-missing", Mutation::DecisionMissing, "DEC001"),
            ("schema-owner", Mutation::SchemaOwnerMissing, "SCH001"),
            ("second-event-drain", Mutation::EventSecondDrain, "EVT001"),
            ("apply-order", Mutation::ApplyOrder, "TXN001"),
            ("enable-wal", Mutation::EnableWal, "NEG001"),
            ("version-bump", Mutation::VersionBump, "VER001"),
            ("source-head", Mutation::SourceHead, "SRC001"),
            ("required-file", Mutation::RequiredFileMissing, "FILE001"),
            ("manifest-digest", Mutation::ManifestDigest, "MAN003"),
        ]
    }

    pub fn apply(self, candidate: &mut Candidate) {
        match self {
            Self::FinalStatus => candidate.set_text("FINAL_STATUS.md", "BLOCKED\n".to_owned()),
            Self::OpenQuestion => {
                candidate.set_text("OPEN_QUESTIONS.md", "which owner?\n".to_owned());
            }
            Self::RequestByte => {
                let mut text = candidate.text("REQUEST.md").unwrap_or_default().to_owned();
                text.push('x');
                candidate.set_text("REQUEST.md", text);
            }
            Self::ContractStatus => {
                candidate.contract["status"] = Value::String("BLOCKED".to_owned());
            }
            Self::DecisionMissing => {
                let text = candidate
                    .text("DECISION_REGISTER.md")
                    .unwrap_or_default()
                    .replace("HSRT-016", "HSRT-X16");
                candidate.set_text("DECISION_REGISTER.md", text);
            }
            Self::SchemaOwnerMissing => {
                let text = candidate
                    .text("SCHEMAS.md")
                    .unwrap_or_default()
                    .replace("pub trait TaskHost", "pub trait MutatedTaskHost");
                candidate.set_text("SCHEMAS.md", text);
            }
            Self::EventSecondDrain => {
                candidate.contract["task_host"]["sole_event_drain"] =
                    Value::String("TaskHostStepOutput + TaskHost::poll_frame".to_owned());
            }
            Self::ApplyOrder => {
                candidate.contract["apply_order"] = Value::Array(Vec::new());
            }
            Self::EnableWal => candidate.contract["persistence"]["wal"] = Value::Bool(true),
            Self::VersionBump => candidate.contract["wire"]["version"] = Value::from(2_u64),
            Self::SourceHead => {
                let text = candidate
                    .text("SOURCE_EVIDENCE.md")
                    .unwrap_or_default()
                    .replace(EXPECTED_HEAD, "0000000000000000000000000000000000000000");
                candidate.set_text("SOURCE_EVIDENCE.md", text);
            }
            Self::RequiredFileMissing => {
                candidate.files.remove("FINAL_DESIGN.md");
            }
            Self::ManifestDigest => {
                let mut bytes = candidate
                    .files
                    .get("MANIFEST.sha256")
                    .cloned()
                    .unwrap_or_default();
                if let Some(first) = bytes.first_mut() {
                    *first = if *first == b'0' { b'1' } else { b'0' };
                }
                let text = String::from_utf8(bytes).unwrap_or_default();
                candidate.set_text("MANIFEST.sha256", text);
            }
        }
    }
}
