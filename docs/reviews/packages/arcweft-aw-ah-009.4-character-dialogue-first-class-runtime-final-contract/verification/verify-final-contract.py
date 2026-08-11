#!/usr/bin/env python3
"""Verify an extracted AW-AH-009.4 final-contract package.

This verifier inspects only the extracted artifact. It does not inspect an
Arcweft checkout and is not a repository source gate.
"""

from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

EXPECTED_REQUEST_SHA256 = "250a1dc175c5281d79b391cdc0873d75c1ef4b7517f63458bf4b5816a3e23b63"
EXPECTED_REVISION = "f56ed157f8d9070d9d1c607f739d9bd0baa1675d"
EXPECTED_DECISIONS = [f"D{index:03d}" for index in range(1, 35)]
EXPECTED_TEST_ROWS = 260
TEXT_SUFFIXES = {".md", ".txt", ".tsv", ".py", ".sh", ".log"}
REQUIRED_MEMBERS = {
    "README.md",
    "SOURCE_REQUEST.md",
    "FINAL_CONTRACT.md",
    "TYPE_AND_MERGE_TABLE.md",
    "GRAMMAR_HIR_SEMA.md",
    "RUNTIME_WIRE_PERSISTENCE.md",
    "TOOLING_DIAGNOSTICS_LIMITS.md",
    "IMPLEMENTATION_ORDER.md",
    "TEST_MATRIX.md",
    "DELETION_MATRIX.md",
    "REPOSITORY_EVIDENCE.md",
    "REQUIREMENTS_TRACEABILITY.md",
    "DECISION_INDEX.tsv",
    "FINAL_STATUS.md",
    "OPEN_QUESTIONS.md",
    "MANIFEST.txt",
    "verification/verify-final-contract.py",
    "verification/IMPLEMENTATION_VALIDATION.md",
    "verification/PACKAGE_VERIFICATION.log",
}


class VerificationError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise VerificationError(message)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def package_root() -> Path:
    if len(sys.argv) > 2:
        raise VerificationError("usage: verify-final-contract.py [extracted-package-root]")
    root = Path(sys.argv[1]) if len(sys.argv) == 2 else Path(__file__).resolve().parents[1]
    root = root.resolve()
    require(root.is_dir(), f"package root is not a directory: {root}")
    return root


def relative_files(root: Path) -> list[str]:
    files: list[str] = []
    for path in root.rglob("*"):
        require(not path.is_symlink(), f"symlink is forbidden in package: {path}")
        if path.is_file():
            files.append(path.relative_to(root).as_posix())
    return sorted(files)


def parse_manifest(root: Path) -> dict[str, tuple[str, int]]:
    manifest_path = root / "MANIFEST.txt"
    require(manifest_path.is_file(), "MANIFEST.txt is missing")
    lines = manifest_path.read_text(encoding="utf-8").splitlines()
    require(lines, "MANIFEST.txt is empty")
    parsed: dict[str, tuple[str, int]] = {}
    previous = ""
    for line_number, line in enumerate(lines, start=1):
        parts = line.split("\t")
        require(len(parts) == 3, f"manifest line {line_number} must have 3 tab fields")
        digest, size_text, relative = parts
        require(re.fullmatch(r"[0-9a-f]{64}", digest) is not None,
                f"manifest line {line_number} has invalid SHA-256")
        require(size_text.isdecimal(), f"manifest line {line_number} has invalid size")
        require(relative and not relative.startswith("/") and "\\" not in relative,
                f"manifest line {line_number} has invalid relative path")
        require(".." not in Path(relative).parts,
                f"manifest line {line_number} escapes the package root")
        require(relative not in parsed, f"manifest duplicate path: {relative}")
        require(previous < relative if previous else True,
                "manifest paths are not in strict lexical order")
        previous = relative
        parsed[relative] = (digest, int(size_text))
    return parsed


def verify_manifest(root: Path) -> tuple[int, int]:
    entries = parse_manifest(root)
    actual = relative_files(root)
    require(set(entries) == set(actual),
            f"manifest membership mismatch: missing={sorted(set(actual)-set(entries))}, "
            f"extra={sorted(set(entries)-set(actual))}")
    require(REQUIRED_MEMBERS <= set(actual),
            f"required package members missing: {sorted(REQUIRED_MEMBERS-set(actual))}")
    total = 0
    for relative in actual:
        path = root / relative
        data = path.read_bytes()
        total += len(data)
        digest, size = entries[relative]
        if relative == "MANIFEST.txt":
            require(digest == "0" * 64 and size == 0,
                    "MANIFEST.txt self-entry must use zero digest and zero size")
            continue
        require(size == len(data), f"size mismatch for {relative}: {size} != {len(data)}")
        require(digest == sha256_bytes(data), f"SHA-256 mismatch for {relative}")
    return len(actual), total


def verify_text(root: Path) -> int:
    count = 0
    for relative in relative_files(root):
        path = root / relative
        if path.suffix not in TEXT_SUFFIXES:
            continue
        data = path.read_bytes()
        require(b"\x00" not in data, f"NUL byte in text file {relative}")
        require(b"\r" not in data, f"CR byte in LF-only text file {relative}")
        require(data.endswith(b"\n"), f"text file lacks final newline: {relative}")
        try:
            data.decode("utf-8")
        except UnicodeDecodeError as error:
            raise VerificationError(f"invalid UTF-8 in {relative}: {error}") from error
        count += 1
    return count


def parse_status(root: Path) -> dict[str, str]:
    status: dict[str, str] = {}
    for line in (root / "FINAL_STATUS.md").read_text(encoding="utf-8").splitlines():
        require("=" in line, f"invalid FINAL_STATUS line: {line}")
        key, value = line.split("=", 1)
        require(key and key not in status, f"duplicate/empty FINAL_STATUS key: {key}")
        status[key] = value
    expected = {
        "STATUS": "READY_FOR_IMPLEMENTATION",
        "OPEN_QUESTIONS": "0",
        "IMPLEMENTATION_PERFORMED": "NO",
        "SELECTED_RUNTIME_MODEL": "RUNTIME_VALUE",
        "REJECTED_RUNTIME_MODEL": "PROVEN_STATIC_ELIMINATION",
        "LINE_ID_FAMILY": "RETAIN_SAY_AS_LINE_ENTITY_ONLY",
        "REPOSITORY": "Sanzentyo/arcweft",
        "REPOSITORY_REVISION": EXPECTED_REVISION,
        "REQUEST_SHA256": EXPECTED_REQUEST_SHA256,
        "PRODUCTION_PATCH_INCLUDED": "NO",
        "COMPATIBILITY_SHIM": "PROHIBITED",
        "DUAL_READER": "PROHIBITED",
        "SOURCE_GATE": "PROHIBITED",
        "CSS_TAKUMI_PATH": "PROHIBITED",
        "VIEW_PROJECTION_FOLLOWUP": "AW-AH-009.4.1",
        "ARTIFACT_VERIFICATION": "PASS",
    }
    require(status == expected, f"FINAL_STATUS mismatch: {status}")
    return status


def verify_decisions(root: Path) -> int:
    lines = (root / "DECISION_INDEX.tsv").read_text(encoding="utf-8").splitlines()
    require(lines and lines[0] == "decision_id\tstatus\tdecision",
            "DECISION_INDEX.tsv header mismatch")
    ids: list[str] = []
    for line_number, line in enumerate(lines[1:], start=2):
        parts = line.split("\t")
        require(len(parts) == 3, f"decision line {line_number} must have 3 fields")
        decision_id, status, decision = parts
        require(status.startswith("FROZEN:"), f"decision {decision_id} is not frozen")
        require(bool(decision.strip()), f"decision {decision_id} has no text")
        ids.append(decision_id)
    require(ids == EXPECTED_DECISIONS, f"decision IDs mismatch: {ids}")
    return len(ids)


def verify_request_and_closure(root: Path) -> None:
    request = (root / "SOURCE_REQUEST.md").read_bytes()
    require(sha256_bytes(request) == EXPECTED_REQUEST_SHA256,
            "SOURCE_REQUEST.md is not byte-identical to the governing request")
    require((root / "OPEN_QUESTIONS.md").read_bytes() == b"OPEN_QUESTIONS=0\n",
            "OPEN_QUESTIONS.md must be exactly OPEN_QUESTIONS=0 plus LF")

    normative = [
        "FINAL_CONTRACT.md",
        "TYPE_AND_MERGE_TABLE.md",
        "GRAMMAR_HIR_SEMA.md",
        "RUNTIME_WIRE_PERSISTENCE.md",
        "TOOLING_DIAGNOSTICS_LIMITS.md",
        "IMPLEMENTATION_ORDER.md",
        "DELETION_MATRIX.md",
        "REQUIREMENTS_TRACEABILITY.md",
    ]
    forbidden = re.compile(r"\b(TODO|TBD|OPEN QUESTION|OPEN_QUESTION|UNDECIDED)\b", re.IGNORECASE)
    for relative in normative:
        text = (root / relative).read_text(encoding="utf-8")
        require(forbidden.search(text) is None,
                f"unclosed decision marker in normative file {relative}")


def verify_test_matrix(root: Path) -> int:
    text = (root / "TEST_MATRIX.md").read_text(encoding="utf-8")
    rows = []
    ids: list[str] = []
    for line in text.splitlines():
        if not line.startswith("| ") or line.startswith("| Category ") or line.startswith("|---"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        require(len(cells) == 7, f"test row does not have seven columns: {line}")
        match = re.fullmatch(r"`([A-Z]+-[0-9]{3})`", cells[1])
        require(match is not None, f"invalid test ID cell: {cells[1]}")
        ids.append(match.group(1))
        rows.append(cells)
    require(len(rows) == EXPECTED_TEST_ROWS,
            f"test row count mismatch: {len(rows)} != {EXPECTED_TEST_ROWS}")
    require(len(ids) == len(set(ids)), "test IDs are not unique")
    require(f"**Normative rows:** {EXPECTED_TEST_ROWS}" in text,
            "TEST_MATRIX.md normative row declaration mismatch")
    return len(rows)


def verify_deletion_coverage(root: Path) -> None:
    text = (root / "DELETION_MATRIX.md").read_text(encoding="utf-8")
    required = [
        "Character.say(...)",
        "SpeakerPreset.say(...)",
        "SpeakerPreset.call(...)",
        "SpeakerRef",
        "SpeakerPreset",
        "DialogueSpeakerPreset",
        "SayOptions",
        "DialogueLineBuilder::say()",
        "TypeKind::Speaker",
        "TypeKind::SpeakerPreset",
        "DialogueCalleeIdentity::Speaker",
        "DialogueCalleeIdentity::SpeakerPreset",
        "DialogueCallableId::SpeakerLine",
        "speaker_preset_chain",
        "all `.say` suffix stripping or reconstruction",
    ]
    missing = [item for item in required if item not in text]
    require(not missing, f"deletion matrix missing required items: {missing}")


def main() -> int:
    try:
        root = package_root()
        members, total_bytes = verify_manifest(root)
        text_files = verify_text(root)
        verify_request_and_closure(root)
        status = parse_status(root)
        decisions = verify_decisions(root)
        tests = verify_test_matrix(root)
        verify_deletion_coverage(root)
    except VerificationError as error:
        print(f"VERIFY_RESULT=FAIL\nERROR={error}", file=sys.stderr)
        return 1

    print("VERIFY_RESULT=PASS")
    print(f"PACKAGE_ROOT={root}")
    print(f"MEMBERS={members}")
    print(f"TOTAL_EXTRACTED_BYTES={total_bytes}")
    print(f"UTF8_LF_TEXT_FILES={text_files}")
    print(f"FROZEN_DECISIONS={decisions}")
    print(f"NORMATIVE_TEST_ROWS={tests}")
    print(f"STATUS={status['STATUS']}")
    print(f"OPEN_QUESTIONS={status['OPEN_QUESTIONS']}")
    print(f"REPOSITORY_REVISION={status['REPOSITORY_REVISION']}")
    print(f"REQUEST_SHA256={status['REQUEST_SHA256']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
