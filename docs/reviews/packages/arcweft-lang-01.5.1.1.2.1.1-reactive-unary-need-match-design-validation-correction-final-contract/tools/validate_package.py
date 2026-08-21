#!/usr/bin/env python3
"""Read-only validator for the reactive unary-Need match design package."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Iterable


REQUIRED_FILES = {
    "README.md",
    "OPEN_QUESTIONS.md",
    "FINAL_STATUS.md",
    "FINAL_CONTRACT.md",
    "OWNERS_AND_APIS.md",
    "RUST_SCHEMAS.md",
    "PUBLICATION_SEMANTICS.md",
    "MATCH_EXECUTION.md",
    "PRODUCER_START_AND_CANCELLATION.md",
    "WIRE_CODEC_SAVE_REPLAY_REPLACEMENT.md",
    "VERSION_1_ALLOCATION_TABLE.md",
    "STATIC_CERTIFICATION.md",
    "FAILURE_PRECEDENCE_AND_ATOMICITY.md",
    "WORK_ACCOUNTING.md",
    "IMPLEMENTATION_SEQUENCE.md",
    "PARENT_ROW_SUPERSESSION.md",
    "DECISION_LOG.md",
    "NON_GOALS.md",
    "SOURCE_EVIDENCE.csv",
    "SOURCE_EVIDENCE.md",
    "CONSUMER_MATRIX.csv",
    "CONSUMER_MATRIX.md",
    "DELETION_MATRIX.csv",
    "DELETION_MATRIX.md",
    "REQUIREMENT_TRACEABILITY.csv",
    "REQUIREMENT_TRACEABILITY.md",
    "TEST_MATRIX.csv",
    "TEST_MATRIX.md",
    "VALIDATION_GATES.md",
    "VERIFICATION.md",
    "CONTRACT_MODEL.json",
    "SHA256SUMS",
    "inputs/CORRECTION_REQUEST.md",
    "inputs/PRIMARY_REQUEST.md",
    "inputs/PARENT_PRECEDENCE.md",
    "inputs/FAILED_RETURN_VALIDATION.md",
    "evidence/repository-state.json",
    "evidence/verification.json",
    "evidence/design-validation.json",
    "evidence/design-validation.status",
    "tools/validate_package.py",
}

MINIMA = {
    "source_evidence": 80,
    "rust_line_evidence": 65,
    "consumer_rows": 25,
    "deletion_rows": 35,
    "traceability_rows": 60,
    "test_rows": 350,
    "work_limits": 20,
}

CURRENT_SHA = "cec30b57fa734efb059d7b846b397ac7d2b0701a"
REQUIRED_PRIMARY_IDS = {f"P{i:02d}" for i in range(1, 13)}
REQUIRED_CORRECTION_IDS = {f"C{i:02d}" for i in range(1, 19)}
REQUIRED_DECISIONS = {f"D{i:02d}" for i in range(1, 16)}
REQUIRED_DELETIONS = {f"DEL{i:02d}" for i in range(1, 41)}
FORBIDDEN_TOP_LEVEL = {
    "production-overlay",
    "production_overlay",
    "patches",
    "src",
    "crates",
    ".git",
}
FORBIDDEN_PLACEHOLDERS = (
    "OPEN_QUESTION:",
    "[TBD]",
    "{{TODO}}",
    "<TBD>",
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def rows(path: Path) -> list[dict[str, str]]:
    with path.open("r", encoding="utf-8", newline="") as stream:
        return list(csv.DictReader(stream))


def payload_files(root: Path) -> set[str]:
    return {
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_file() and path.relative_to(root).as_posix() != "SHA256SUMS"
    }


def parse_manifest(path: Path) -> dict[str, str]:
    result: dict[str, str] = {}
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line:
            continue
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        if match is None:
            raise ValueError(f"invalid SHA256SUMS row {line_no}")
        digest, name = match.groups()
        if name in result:
            raise ValueError(f"duplicate SHA256SUMS row for {name}")
        result[name] = digest
    return result


def check_required(root: Path, issues: list[str]) -> None:
    actual = {
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_file()
    }
    for missing in sorted(REQUIRED_FILES - actual):
        issues.append(f"MISSING {missing}")
    for forbidden in sorted(FORBIDDEN_TOP_LEVEL):
        if (root / forbidden).exists():
            issues.append(f"FORBIDDEN_ARTIFACT_PATH {forbidden}")


def check_open_questions(root: Path, issues: list[str]) -> None:
    path = root / "OPEN_QUESTIONS.md"
    if path.exists() and path.read_bytes() != b"none":
        issues.append("OPEN_QUESTIONS_NOT_EXACT_NONE")


def check_matrices(root: Path, issues: list[str], counts: dict[str, int]) -> None:
    evidence = rows(root / "SOURCE_EVIDENCE.csv")
    consumers = rows(root / "CONSUMER_MATRIX.csv")
    deletions = rows(root / "DELETION_MATRIX.csv")
    trace = rows(root / "REQUIREMENT_TRACEABILITY.csv")
    tests = rows(root / "TEST_MATRIX.csv")

    rust = [row for row in evidence if row.get("kind") == "Rust"]
    counts.update(
        source_evidence=len(evidence),
        rust_line_evidence=len(rust),
        consumer_rows=len(consumers),
        deletion_rows=len(deletions),
        traceability_rows=len(trace),
        test_rows=len(tests),
    )

    for name in (
        "source_evidence",
        "rust_line_evidence",
        "consumer_rows",
        "deletion_rows",
        "traceability_rows",
        "test_rows",
    ):
        if counts[name] < MINIMA[name]:
            issues.append(f"{name.upper()}_TOO_SMALL {counts[name]}<{MINIMA[name]}")

    evidence_ids = [row.get("id", "") for row in evidence]
    if len(evidence_ids) != len(set(evidence_ids)):
        issues.append("DUPLICATE_SOURCE_EVIDENCE_ID")
    for row in evidence:
        if row.get("sha") != CURRENT_SHA:
            issues.append(f"SOURCE_EVIDENCE_WRONG_SHA {row.get('id')}")
        if not row.get("path"):
            issues.append(f"SOURCE_EVIDENCE_MISSING_PATH {row.get('id')}")
        if row.get("kind") == "Rust" and re.fullmatch(r"[1-9][0-9]*-[1-9][0-9]*", row.get("lines", "")) is None:
            issues.append(f"RUST_LINE_EVIDENCE_INVALID {row.get('id')}")
        if not row.get("owner_api") or not row.get("consumer_dependency"):
            issues.append(f"SOURCE_EVIDENCE_DIRECTION_MISSING {row.get('id')}")

    consumer_ids = [row.get("id", "") for row in consumers]
    if len(consumer_ids) != len(set(consumer_ids)):
        issues.append("DUPLICATE_CONSUMER_ID")

    deletion_ids = {row.get("id", "") for row in deletions}
    missing_deletions = REQUIRED_DELETIONS - deletion_ids
    if missing_deletions:
        issues.append("MISSING_DELETION_ROWS " + ",".join(sorted(missing_deletions)))
    for row in deletions:
        if not row.get("owner") or not row.get("old_surface") or not row.get("absence_proof"):
            issues.append(f"DELETION_ROW_INCOMPLETE {row.get('id')}")

    requirement_ids = {row.get("requirement_id", "") for row in trace}
    missing_primary = REQUIRED_PRIMARY_IDS - requirement_ids
    missing_correction = REQUIRED_CORRECTION_IDS - requirement_ids
    if missing_primary:
        issues.append("MISSING_PRIMARY_TRACE " + ",".join(sorted(missing_primary)))
    if missing_correction:
        issues.append("MISSING_CORRECTION_TRACE " + ",".join(sorted(missing_correction)))
    for row in trace:
        if not row.get("selected_decision") or not row.get("artifacts") or not row.get("test_ids"):
            issues.append(f"TRACE_ROW_INCOMPLETE {row.get('id')}")

    test_ids = [row.get("id", "") for row in tests]
    if len(test_ids) != len(set(test_ids)):
        issues.append("DUPLICATE_TEST_ID")
    required_test_classes = {"positive", "negative", "tamper", "limit", "structural", "gate"}
    actual_test_classes = {row.get("class", "") for row in tests}
    missing_classes = required_test_classes - actual_test_classes
    if missing_classes:
        issues.append("MISSING_TEST_CLASSES " + ",".join(sorted(missing_classes)))
    required_tiers = {"focused", "integration", "Tier-2"}
    actual_tiers = {row.get("tier", "") for row in tests}
    missing_tiers = required_tiers - actual_tiers
    if missing_tiers:
        issues.append("MISSING_TEST_TIERS " + ",".join(sorted(missing_tiers)))
    for row in tests:
        for field in ("owner", "inputs", "operation", "expected", "atomicity", "gate"):
            if not row.get(field):
                issues.append(f"TEST_ROW_INCOMPLETE {row.get('id')}:{field}")


def check_contract_model(root: Path, issues: list[str], counts: dict[str, int]) -> None:
    model = json.loads((root / "CONTRACT_MODEL.json").read_text(encoding="utf-8"))
    if model.get("status", {}).get("open_questions") != "none":
        issues.append("MODEL_OPEN_QUESTIONS_NOT_NONE")
    if model.get("status", {}).get("design_ready") is not True:
        issues.append("MODEL_DESIGN_NOT_READY")
    if model.get("status", {}).get("implementation_claimed") is not False:
        issues.append("MODEL_IMPLEMENTATION_OVERCLAIM")
    versions = model.get("versions", {})
    if not versions or any(value != 1 for value in versions.values()):
        issues.append("NON_V1_MARKER")
    if model.get("states", {}).get("need_error_branch") is not False:
        issues.append("NEED_ERROR_BRANCH_REINTRODUCED")
    if model.get("states", {}).get("need_denied_branch") is not False:
        issues.append("NEED_DENIED_BRANCH_REINTRODUCED")
    if model.get("policies", {}).get("old_await_reader") is not False:
        issues.append("OLD_AWAIT_READER_ENABLED")
    if model.get("policies", {}).get("old_await_alias") is not False:
        issues.append("OLD_AWAIT_ALIAS_ENABLED")
    work_limits = model.get("limits", {})
    counts["work_limits"] = len(work_limits)
    if counts["work_limits"] < MINIMA["work_limits"]:
        issues.append(
            f"WORK_LIMITS_TOO_SMALL {counts['work_limits']}<{MINIMA['work_limits']}"
        )
    for name, value in work_limits.items():
        if not isinstance(name, str) or not name or not isinstance(value, int) or value <= 0:
            issues.append(f"INVALID_WORK_LIMIT {name!r}={value!r}")


def check_decisions(root: Path, issues: list[str]) -> None:
    text = (root / "DECISION_LOG.md").read_text(encoding="utf-8")
    present = set(re.findall(r"\|\s*(D[0-9]{2})\s*\|", text))
    missing = REQUIRED_DECISIONS - present
    if missing:
        issues.append("MISSING_DECISIONS " + ",".join(sorted(missing)))

    combined = "\n".join(
        (root / name).read_text(encoding="utf-8")
        for name in (
            "FINAL_CONTRACT.md",
            "OWNERS_AND_APIS.md",
            "RUST_SCHEMAS.md",
            "PUBLICATION_SEMANTICS.md",
            "MATCH_EXECUTION.md",
            "PRODUCER_START_AND_CANCELLATION.md",
            "WIRE_CODEC_SAVE_REPLAY_REPLACEMENT.md",
            "STATIC_CERTIFICATION.md",
            "FAILURE_PRECEDENCE_AND_ATOMICITY.md",
            "WORK_ACCOUNTING.md",
            "IMPLEMENTATION_SEQUENCE.md",
            "DELETION_MATRIX.md",
        )
    )
    required_phrases = (
        "CheckedViewCatalog",
        "CheckedViewNeedMatch",
        "ViewNeedSubscriptionId",
        "ViewNeedSubscriptionSemanticId",
        "ViewNeedSubscriptionContractDigest",
        "RuntimeNeedState",
        "TaskPublicationCursor",
        "NotStarted",
        "Pending(Progress)",
        "Ready(T)",
        "Cancelled",
        "JoinSameKey",
        "ProducerOwned",
        "ViewInstruction::Match",
        "RuntimeValue",
        "OPEN_QUESTIONS.md",
        "LiveNeedSubscription",
        "strict",
        "atomic",
        "version",
    )
    for phrase in required_phrases:
        if phrase not in combined:
            issues.append(f"MISSING_NORMATIVE_PHRASE {phrase}")

    for placeholder in FORBIDDEN_PLACEHOLDERS:
        for path in root.rglob("*.md"):
            if placeholder in path.read_text(encoding="utf-8"):
                issues.append(
                    f"UNRESOLVED_PLACEHOLDER {path.relative_to(root).as_posix()}:{placeholder}"
                )


def check_validation_record(
    root: Path, issues: list[str], counts: dict[str, int]
) -> None:
    record = json.loads(
        (root / "evidence/design-validation.json").read_text(encoding="utf-8")
    )
    if record.get("pass") is not True:
        issues.append("DESIGN_VALIDATION_RECORD_NOT_PASS")
    if record.get("issues") != []:
        issues.append("DESIGN_VALIDATION_RECORD_HAS_ISSUES")
    if record.get("head") != CURRENT_SHA:
        issues.append("DESIGN_VALIDATION_WRONG_HEAD")
    declared = record.get("counts", {})
    for name, value in counts.items():
        if declared.get(name) != value:
            issues.append(
                f"DESIGN_VALIDATION_COUNT_MISMATCH {name}:{declared.get(name)}!={value}"
            )
    status = (root / "evidence/design-validation.status").read_bytes()
    if status != b"PASS\n":
        issues.append("DESIGN_VALIDATION_STATUS_NOT_PASS")


def check_manifest(root: Path, issues: list[str], counts: dict[str, int]) -> None:
    try:
        manifest = parse_manifest(root / "SHA256SUMS")
    except ValueError as error:
        issues.append(f"MANIFEST_PARSE {error}")
        return
    expected = payload_files(root)
    actual = set(manifest)
    if expected != actual:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        if missing:
            issues.append("MANIFEST_MISSING " + ",".join(missing))
        if extra:
            issues.append("MANIFEST_EXTRA " + ",".join(extra))
    for name, expected_digest in manifest.items():
        path = root / name
        if not path.is_file():
            continue
        actual_digest = sha256(path)
        if actual_digest != expected_digest:
            issues.append(f"MANIFEST_HASH_MISMATCH {name}")
    counts["manifest_rows"] = len(manifest)


def validate(root: Path) -> dict[str, object]:
    root = root.resolve()
    issues: list[str] = []
    counts: dict[str, int] = {}
    check_required(root, issues)
    # Stop cleanly if a required file needed by subsequent checks is absent.
    if issues:
        missing_only = [issue for issue in issues if issue.startswith("MISSING ")]
        if missing_only:
            return {
                "pass": False,
                "issues": sorted(set(issues)),
                "head": CURRENT_SHA,
                "counts": counts,
                "root": str(root),
            }

    check_open_questions(root, issues)
    check_matrices(root, issues, counts)
    check_contract_model(root, issues, counts)
    check_decisions(root, issues)
    check_validation_record(root, issues, counts)
    check_manifest(root, issues, counts)

    return {
        "pass": not issues,
        "issues": sorted(set(issues)),
        "head": CURRENT_SHA,
        "counts": counts,
        "root": str(root),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "root",
        nargs="?",
        default=str(Path(__file__).resolve().parents[1]),
        help="package root (defaults to the validator's parent package)",
    )
    args = parser.parse_args()
    result = validate(Path(args.root))
    print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
