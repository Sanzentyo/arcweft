#!/usr/bin/env python3
"""Mechanical validator for the Lang-01.3.1.2.3.2 design package."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import sys
import zipfile

EXPECTED_REQUEST_SHA256 = "dc9d39578e4706b7b518bc2cfdd37fda33d6be38352007c957e2360704afcf76"
EXPECTED_ARCHIVE_NAME = (
    "arcweft-lang-01.3.1.2.3.2-"
    "generic-ownership-identity-and-slot-reconciliation-correction-final-contract.zip"
)
REQUIRED_FILES = {
    "README.md",
    "FINAL_CONTRACT.md",
    "RUST_OWNERS_AND_APIS.md",
    "IDENTITY_AND_CODEC_CONTRACT.md",
    "TRANSACTION_AND_COMMIT_CONTRACT.md",
    "VALUE_PATH_AND_PRECEDENCE.md",
    "SNAPSHOT_ACTIVATION_AND_RESTORE.md",
    "PRODUCER_CONSUMER_DELETION_INVENTORY.md",
    "SUPERSESSION_DELTA.md",
    "IMPLEMENTATION_ORDER.md",
    "TEST_MATRIX.csv",
    "NEGATIVE_AND_TAMPER_MATRIX.md",
    "REQUIREMENTS_TRACEABILITY.md",
    "REPOSITORY_EVIDENCE.md",
    "CODEC_GOLDENS.md",
    "CODEC_GOLDENS.json",
    "SYMBOL_CLOSURE.json",
    "DECISION_REGISTER.md",
    "VALIDATION.md",
    "FINAL_STATUS.md",
    "OPEN_QUESTIONS.md",
    "SOURCE_REQUEST.md",
    "PACKAGE_VALIDATION.json",
    "MANIFEST.txt",
    "validation/model_checks.py",
    "validation/validate_package.py",
}
FORBIDDEN_BASENAMES = {
    "Cargo.toml",
    "Cargo.lock",
    ".git",
    ".jj",
}
FORBIDDEN_SUFFIXES = {
    ".rs",
    ".rlib",
    ".rmeta",
    ".o",
    ".obj",
    ".so",
    ".dll",
    ".dylib",
    ".wasm",
    ".patch",
    ".diff",
    ".rej",
    ".orig",
    ".pem",
    ".key",
}
REQUIRED_SYMBOLS = {
    "ExecutionInstanceId",
    "RuntimeFreshExecution",
    "RuntimeRecordFieldId",
    "RuntimeLocalSlotId",
    "RuntimeOwnedSlotId",
    "RuntimeOwnershipTransactionId",
    "RuntimeSlotRevision",
    "RuntimeMovedValueEvidence",
    "RuntimeDroppedValueEvidence",
    "RuntimePreparedCopy",
    "RuntimePreparedMove",
    "RuntimeTransferCommitError",
    "RuntimeOwnershipLimits",
    "RuntimeOwnershipTransaction",
    "RuntimeValuePath",
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def safe_name(name: str) -> bool:
    p = PurePosixPath(name)
    return (
        name
        and not name.startswith("/")
        and "\\" not in name
        and ".." not in p.parts
        and p.parts[0] not in {"", "."}
    )


def read_manifest(package: Path) -> dict[str, str]:
    result: dict[str, str] = {}
    lines = (package / "MANIFEST.txt").read_text(encoding="utf-8").splitlines()
    for index, line in enumerate(lines, 1):
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        assert match, f"invalid MANIFEST line {index}: {line!r}"
        digest, name = match.groups()
        assert name not in result, f"duplicate MANIFEST name: {name}"
        assert safe_name(name), f"unsafe MANIFEST path: {name}"
        result[name] = digest
    return result


def validate_package(package: Path) -> dict[str, object]:
    assert package.is_dir(), f"package directory missing: {package}"
    files = sorted(
        p.relative_to(package).as_posix()
        for p in package.rglob("*")
        if p.is_file()
    )
    assert REQUIRED_FILES <= set(files), sorted(REQUIRED_FILES - set(files))
    assert all(safe_name(name) for name in files)
    for name in files:
        p = PurePosixPath(name)
        assert p.name not in FORBIDDEN_BASENAMES, f"forbidden basename: {name}"
        assert p.suffix not in FORBIDDEN_SUFFIXES, f"forbidden suffix: {name}"
        assert "__pycache__" not in p.parts
        assert not p.name.startswith("."), f"hidden file: {name}"

    assert (package / "OPEN_QUESTIONS.md").read_bytes() == b"none\n"
    assert sha256((package / "SOURCE_REQUEST.md").read_bytes()) == EXPECTED_REQUEST_SHA256

    final = (package / "FINAL_CONTRACT.md").read_text(encoding="utf-8")
    rust = (package / "RUST_OWNERS_AND_APIS.md").read_text(encoding="utf-8")
    status = (package / "FINAL_STATUS.md").read_text(encoding="utf-8")
    for needle in [
        "STATUS=READY_FOR_IMPLEMENTATION",
        "OPEN_QUESTIONS=0",
        "IMPLEMENTATION_PERFORMED=NO",
    ]:
        assert needle in final or needle in status, f"missing status marker {needle}"
    assert "NOT_READY" not in status
    assert "todo!()" not in rust
    assert "unimplemented!()" not in rust
    assert "TBD" not in rust
    for symbol in REQUIRED_SYMBOLS:
        assert re.search(rf"\b{re.escape(symbol)}\b", rust), f"missing Rust symbol {symbol}"

    closure = json.loads((package / "SYMBOL_CLOSURE.json").read_text(encoding="utf-8"))
    names = [item["symbol"] for item in closure["symbols"]]
    assert len(names) == len(set(names)) == closure["symbol_count"]
    assert REQUIRED_SYMBOLS <= set(names)
    assert closure["placeholder_policy"]

    goldens = json.loads((package / "CODEC_GOLDENS.json").read_text(encoding="utf-8"))
    assert len(goldens["goldens"]) >= 20
    for item in goldens["goldens"]:
        raw = bytes.fromhex(item["binary_hex"])
        assert len(raw) == item["binary_length"], item["name"]
    assert len(goldens["invalid_binary"]) >= 15
    assert len(goldens["invalid_json"]) >= 10

    with (package / "TEST_MATRIX.csv").open(
        encoding="utf-8", newline=""
    ) as handle:
        rows = list(csv.DictReader(handle))
    assert len(rows) >= 400, len(rows)
    ids = [row["id"] for row in rows]
    assert len(ids) == len(set(ids))
    prefixes = {value.split("-", 1)[0] for value in ids}
    assert {
        "EXE", "COD", "REC", "LOC", "SLT", "TXN", "LIM", "ALC",
        "CMT", "PAR", "PTH", "SNP", "API", "FUL"
    } <= prefixes
    kinds = {row["kind"] for row in rows}
    assert {
        "positive", "negative", "tamper", "boundary", "golden",
        "parity", "structural", "compile_fail", "full_gate"
    } <= kinds
    assert all(all(row.get(column, "") for column in [
        "id", "area", "owner", "cut", "kind", "setup",
        "action", "expected", "evidence"
    ]) for row in rows)

    manifest = read_manifest(package)
    assert set(manifest) == set(files)
    for name in files:
        expected = "0" * 64 if name == "MANIFEST.txt" else sha256((package / name).read_bytes())
        assert manifest[name] == expected, f"MANIFEST mismatch: {name}"

    return {
        "file_count": len(files),
        "test_row_count": len(rows),
        "test_kinds": sorted(kinds),
        "symbol_count": len(names),
        "golden_count": len(goldens["goldens"]),
        "invalid_binary_count": len(goldens["invalid_binary"]),
        "invalid_json_count": len(goldens["invalid_json"]),
        "manifest_entries": len(manifest),
        "source_request_sha256": EXPECTED_REQUEST_SHA256,
    }


def validate_zip(archive: Path, package: Path) -> dict[str, object]:
    assert archive.name == EXPECTED_ARCHIVE_NAME
    assert archive.is_file()
    package_files = sorted(
        p.relative_to(package).as_posix()
        for p in package.rglob("*")
        if p.is_file()
    )
    with zipfile.ZipFile(archive) as zf:
        bad = zf.testzip()
        assert bad is None, f"CRC failure: {bad}"
        infos = zf.infolist()
        names = [info.filename for info in infos]
        assert names == sorted(names), "members are not lexical"
        assert names == package_files, "ZIP/package member mismatch"
        assert len(names) == len(set(names))
        for info in infos:
            assert safe_name(info.filename)
            assert not info.is_dir()
            assert info.date_time == (1980, 1, 1, 0, 0, 0)
            mode = (info.external_attr >> 16) & 0o7777
            assert mode == 0o644, (info.filename, oct(mode))
            assert zf.read(info.filename) == (package / info.filename).read_bytes()
    return {
        "archive_name": archive.name,
        "archive_sha256": sha256(archive.read_bytes()),
        "archive_size": archive.stat().st_size,
        "zip_member_count": len(package_files),
        "zip_crc": "pass",
        "zip_path_safety": "pass",
        "zip_deterministic_metadata": "pass",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    parser.add_argument("--zip", dest="archive", type=Path)
    args = parser.parse_args()

    report = {"package": validate_package(args.package)}
    if args.archive:
        report["zip"] = validate_zip(args.archive, args.package)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
