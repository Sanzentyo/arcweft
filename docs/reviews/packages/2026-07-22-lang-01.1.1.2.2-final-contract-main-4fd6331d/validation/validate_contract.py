#!/usr/bin/env python3
"""Validate the independent Lang-01.1.1.2.2 final-contract package."""

from __future__ import annotations

import csv
import hashlib
import json
import re
import sys
from pathlib import Path

BASE_SHA = "4fd6331dc342d30a7f4ac7774852b60801866ef7"
REQUIRED_FILES = {
    "README.md",
    "REQUEST.md",
    "FINAL-CONTRACT.md",
    "API-SHAPES.md",
    "CONSTRUCTION-ORDER.md",
    "ERROR-ROLLBACK.md",
    "SCHEMA-TOOLING-PERSISTENCE.md",
    "IMPLEMENTATION-MAP.md",
    "TEST-MATRIX.csv",
    "TRACEABILITY.md",
    "NON-GOALS.md",
    "REPOSITORY-VALIDATION.md",
    "COMMANDS-RUN.md",
    "PRODUCTION-CODE-CHANGES.txt",
    "contract/DECISIONS.json",
    "evidence/BASELINE.json",
    "evidence/INSPECTED-FILES.tsv",
    "validation/validate_contract.py",
    "validation/VALIDATION-RESULT.txt",
    "MANIFEST.sha256",
}

REQUIRED_DECISION_TEXT = {
    "FINAL-CONTRACT.md": [
        "AcceptedNominalOwnerId::RustPackage(RustPackageId)",
        "AdapterTypeKind::to_sema_type_kind()` is deleted",
        "AcceptedNominalRecord::try_instantiate",
        "AcceptedRustTypeMetadataCatalog",
        "AcceptedNominalWorldStamp",
        "single transaction",
        "schema constants remain `1`",
    ],
    "API-SHAPES.md": [
        "pub struct ArcweftRustPackageId",
        "pub struct ArcweftRustTypePath",
        "pub enum AdapterNominalOwner",
        "pub struct EnvironmentTypeProjectionNode",
        "pub fn try_project_environment_publication",
        "GenericTypeOwnerId::AcceptedNominal",
    ],
    "CONSTRUCTION-ORDER.md": [
        "construct `AcceptedNominalWorld` exactly once",
        "project Rust ADT metadata",
        "project environment callables",
        "There is no `arcweft-lang-sema → arcweft-adapter-context` dependency",
    ],
    "ERROR-ROLLBACK.md": [
        "UnknownPath",
        "InaccessibleExport",
        "OwnerMismatch",
        "WrongArity",
        "LimitExceeded",
        "No phase mutates the previously accepted world",
    ],
    "SCHEMA-TOOLING-PERSISTENCE.md": [
        "TypeKind::semantic_identity_digest()",
        "CallableSignatureSchema::semantic_digest()",
        "CompilerObjectKey.environment_digest",
        "ReceiverMethodKey",
        "source label is never an identity",
    ],
}

REQUIRED_TEST_IDS = {
    "CALL-001", "CALL-002", "CALL-003", "CALL-004", "CALL-005",
    "CALL-006", "CALL-007", "CALL-008", "CALL-009", "CALL-010",
    "CALL-011", "CALL-012", "NOM-004", "NOM-005", "NOM-006",
    "NOM-019", "NOM-020", "NOM-021", "NOM-022", "NOM-023",
    "NOM-024", "NOM-025", "META-001", "META-006", "META-011",
    "REG-006", "REG-007", "REG-008", "REG-010", "TOOL-001",
    "TOOL-005", "TOOL-008", "PERSIST-001", "PERSIST-005",
    "ADP-011", "ABI-004", "ERR-007", "CUT-001", "CUT-005",
}

FORBIDDEN_PLACEHOLDERS = re.compile(
    r"\b(?:TBD|TODO|FIXME|TO\s+BE\s+DECIDED|DECISION\s+PENDING)\b",
    re.IGNORECASE,
)
FORBIDDEN_ALTERNATIVE_HEADING = re.compile(
    r"^#{1,6}\s+(?:Alternatives?|Options?)\b", re.IGNORECASE | re.MULTILINE
)


class ValidationError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def validate(root: Path) -> tuple[int, int]:
    actual_files = {
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_file()
    }
    missing = sorted(REQUIRED_FILES - actual_files)
    require(not missing, f"missing required files: {missing}")

    decisions = json.loads((root / "contract/DECISIONS.json").read_text("utf-8"))
    baseline = json.loads((root / "evidence/BASELINE.json").read_text("utf-8"))

    require(decisions["status"] == "final", "decision status is not final")
    require(decisions["baseline"]["sha"] == BASE_SHA, "decision baseline mismatch")
    require(baseline["sha"] == BASE_SHA, "evidence baseline mismatch")
    require(decisions["production_code_modified"] is False, "production code flag is not false")
    require(baseline["production_repository_writes"] == 0, "repository writes were recorded")
    require(
        decisions["rust_nominal_owner"]
        == "AcceptedNominalOwnerId::RustPackage(RustPackageId)",
        "Rust nominal owner is not fixed to RustPackage",
    )
    require(
        decisions["context_free_conversion"].startswith(
            "AdapterTypeKind::to_sema_type_kind deleted"
        ),
        "context-free conversion decision is missing",
    )
    require(decisions["compatibility_layers"] == [], "compatibility layers are present")
    require(decisions["schema_versions"]["arcweft_rust_abi"] == 1, "Rust ABI schema changed")
    require(decisions["schema_versions"]["adapter_manifest"] == 1, "adapter schema changed")

    for relative, required_fragments in REQUIRED_DECISION_TEXT.items():
        text = (root / relative).read_text("utf-8")
        require(not FORBIDDEN_PLACEHOLDERS.search(text), f"placeholder in {relative}")
        require(
            not FORBIDDEN_ALTERNATIVE_HEADING.search(text),
            f"alternative/options heading in {relative}",
        )
        for fragment in required_fragments:
            require(fragment in text, f"{relative} missing required fragment: {fragment}")

    for relative in [
        "README.md",
        "TRACEABILITY.md",
        "NON-GOALS.md",
        "REPOSITORY-VALIDATION.md",
        "COMMANDS-RUN.md",
    ]:
        text = (root / relative).read_text("utf-8")
        require(not FORBIDDEN_PLACEHOLDERS.search(text), f"placeholder in {relative}")

    with (root / "TEST-MATRIX.csv").open("r", encoding="utf-8", newline="") as stream:
        test_rows = list(csv.DictReader(stream))
    require(len(test_rows) >= 150, f"test matrix too small: {len(test_rows)}")
    ids = [row["ID"] for row in test_rows]
    require(len(ids) == len(set(ids)), "duplicate test IDs")
    missing_ids = sorted(REQUIRED_TEST_IDS - set(ids))
    require(not missing_ids, f"missing required test IDs: {missing_ids}")
    for row in test_rows:
        for field in [
            "ID", "Requirement", "Crate", "Layer", "TestKind",
            "Setup", "Assertion", "RollbackOrFailure", "TypedAPI",
        ]:
            require(row.get(field, "").strip() != "", f"{row.get('ID')} empty {field}")

    inspected = (root / "evidence/INSPECTED-FILES.tsv").read_text("utf-8").splitlines()
    require(len(inspected) >= 45, f"inspected file inventory too small: {len(inspected)-1}")
    require(all(BASE_SHA in line for line in inspected[1:]), "inspected file baseline mismatch")

    request_hash = sha256(root / "REQUEST.md")
    expected_request_hash = baseline["inputs"][
        "2026-07-22-lang-01.1.1.2.2-adapter-callable-nominal-publication-projection-correction.md"
    ]
    require(request_hash == expected_request_hash, "request copy hash mismatch")

    manifest_path = root / "MANIFEST.sha256"
    manifest_lines = [
        line for line in manifest_path.read_text("utf-8").splitlines() if line.strip()
    ]
    manifest = {}
    for line in manifest_lines:
        digest, relative = line.split("  ", 1)
        require(re.fullmatch(r"[0-9a-f]{64}", digest) is not None, f"bad digest: {line}")
        require(relative not in manifest, f"duplicate manifest entry: {relative}")
        manifest[relative] = digest

    expected_manifest_files = actual_files - {"MANIFEST.sha256"}
    require(set(manifest) == expected_manifest_files, "manifest file set mismatch")
    for relative, digest in manifest.items():
        require(sha256(root / relative) == digest, f"manifest hash mismatch: {relative}")

    result_text = (root / "validation/VALIDATION-RESULT.txt").read_text("utf-8")
    require("PASS" in result_text, "validation result is not PASS")
    require(str(len(test_rows)) in result_text, "validation result test count mismatch")

    return len(test_rows), len(actual_files)


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    try:
        test_count, file_count = validate(root)
    except (OSError, ValueError, KeyError, ValidationError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print(
        f"PASS: final-contract package validated "
        f"({test_count} typed-API tests, {file_count} package files)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
