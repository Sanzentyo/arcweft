#!/usr/bin/env python3
"""Verify the final-contract package using only Python's standard library."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
import tomllib
from collections import Counter
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REQUEST_NAME = (
    "2026-07-22-lang-01.3.1.2.2-curried-external-stream-runtime-argument-"
    "projection-correction.md"
)
REQUIRED_FILES = {
    "STATUS.md",
    "README.md",
    "FINAL_CONTRACT.md",
    "RUST_TYPES_AND_OWNERS.md",
    "AWBC_AND_WIRE.md",
    "HOST_JSON.md",
    "EVALUATION_EFFECT_SNAPSHOT.md",
    "FINGERPRINT_AND_HOT_RELOAD.md",
    "DELTA_FROM_LANG-01.3.1.2.1.md",
    "IMPLEMENTATION_PLAN.md",
    "TEST_MATRIX.md",
    "TEST_MATRIX.json",
    "REPOSITORY_EVIDENCE.md",
    "VALIDATION.md",
    "contract.json",
    f"request/{REQUEST_NAME}",
    "inputs/Rust Skill.txt",
    "inputs/前提(Sanzentyo-arcweft).txt",
    "model/Cargo.toml",
    "model/src/lib.rs",
    "host/validate_host_fixtures.py",
}
EXPECTED_PREFIX_COUNTS = {
    "P": 44,
    "N": 45,
    "T": 30,
    "J": 16,
    "S": 14,
    "H": 10,
    "A": 9,
}


class ContractValidationError(ValueError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_blob_sha1(path: Path) -> str:
    data = path.read_bytes()
    return hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()  # noqa: S324


def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ContractValidationError(f"duplicate JSON field {key!r}")
        value[key] = item
    return value


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicates)


def validate_contract_index() -> dict[str, Any]:
    contract = load_json(ROOT / "contract.json")
    expected_scalars = {
        "contract_id": "Lang-01.3.1.2.2",
        "contract_status": "FINAL",
        "open_questions": 0,
        "fallback": False,
        "production_code_changed": False,
        "currying_policy": "supported",
        "wire_strategy": (
            "nested_signature_groups_and_canonical_coordinate_table_plus_parallel_"
            "value_vector"
        ),
        "function_value_owner": "RuntimeFunctionValue::ExternalStreamPartial",
    }
    for key, expected in expected_scalars.items():
        if contract.get(key) != expected:
            raise ContractValidationError(
                f"contract.json {key}: {contract.get(key)!r} != {expected!r}"
            )

    repository = contract["repository"]
    if repository != {
        "name": "Sanzentyo/arcweft",
        "ref": "main",
        "commit": "5821a3ca479b5b89ca6ede997b9cf4f42f6280a6",
        "agents_blob": "e91f99213dde67953beda6aa078c370a8dc4541d",
        "request_blob": "6d24910f7961c56faaffddea5cfa6775b48578a1",
    }:
        raise ContractValidationError("contract.json repository baseline mismatch")
    if contract["versions"] != {
        "awbc_abi": 2,
        "awbc_codec": 8,
        "bundle_session_save_schema": 2,
    }:
        raise ContractValidationError("contract.json version allocation mismatch")
    if contract["limits"] != {
        "groups_per_callable": 16,
        "parameters_per_callable": 128,
        "coordinates_per_product": 128,
        "nesting_depth": 64,
        "encoded_bytes_default": 268435456,
    }:
        raise ContractValidationError("contract.json limits mismatch")
    if contract["tags"] != {
        "runtime_type_stream_handle": 21,
        "runtime_type_external_stream_callable": 22,
        "constant_external_stream_callable": 18,
        "opcode_apply_external_stream_group": 39,
        "opcode_open_stream": 40,
        "function_value_closure": 0,
        "function_value_external_stream_partial": 1,
        "argument_explicit": 0,
        "argument_defaulted": 1,
        "argument_omitted_optional": 2,
        "argument_rest_positional": 3,
        "argument_rest_named": 4,
    }:
        raise ContractValidationError("contract.json tag allocation mismatch")
    return contract


def validate_files(contract: dict[str, Any]) -> tuple[int, int]:
    for relative in REQUIRED_FILES:
        path = ROOT / relative
        if not path.is_file():
            raise ContractValidationError(f"required file missing: {relative}")
    for path in ROOT.rglob("*"):
        if path.is_symlink():
            raise ContractValidationError(f"symlink not allowed: {path.relative_to(ROOT)}")
        if path.is_file() and "__pycache__" in path.parts:
            raise ContractValidationError(f"Python cache not allowed: {path.relative_to(ROOT)}")

    input_hashes = contract["input_hashes"]
    request = ROOT / "request" / REQUEST_NAME
    rust_skill = ROOT / "inputs" / "Rust Skill.txt"
    premise = ROOT / "inputs" / "前提(Sanzentyo-arcweft).txt"
    if sha256(request) != input_hashes["request_sha256"]:
        raise ContractValidationError("request SHA-256 mismatch")
    if sha256(rust_skill) != input_hashes["rust_skill_sha256"]:
        raise ContractValidationError("Rust Skill SHA-256 mismatch")
    if sha256(premise) != input_hashes["premise_sha256"]:
        raise ContractValidationError("premise SHA-256 mismatch")
    if git_blob_sha1(request) != contract["repository"]["request_blob"]:
        raise ContractValidationError("request Git blob mismatch")

    text_files = 0
    total_bytes = 0
    for path in ROOT.rglob("*"):
        if not path.is_file():
            continue
        total_bytes += path.stat().st_size
        if path.suffix in {".md", ".rs", ".py", ".json", ".toml", ".txt"}:
            text = path.read_text(encoding="utf-8")
            if "\x00" in text:
                raise ContractValidationError(f"NUL in text file: {path.relative_to(ROOT)}")
            relative = path.relative_to(ROOT)
            exact_input = relative.parts[0] in {"inputs", "request"}
            if text and not text.endswith("\n") and not exact_input:
                raise ContractValidationError(
                    f"text file lacks final newline: {relative}"
                )
            text_files += 1
    return text_files, total_bytes


def parse_markdown_matrix() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    pattern = re.compile(r"(?:P|N|T|J|S|H|A)-\d{3}\Z")
    for line in (ROOT / "TEST_MATRIX.md").read_text(encoding="utf-8").splitlines():
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 4 or pattern.fullmatch(cells[0]) is None:
            continue
        rows.append(
            {
                "id": cells[0],
                "layer": cells[1],
                "scenario": cells[2],
                "expected": cells[3],
            }
        )
    return rows


def validate_test_matrix() -> tuple[int, Counter[str]]:
    matrix = load_json(ROOT / "TEST_MATRIX.json")
    if matrix.get("contract_id") != "Lang-01.3.1.2.2":
        raise ContractValidationError("TEST_MATRIX.json contract ID mismatch")
    rows = matrix.get("tests")
    if not isinstance(rows, list):
        raise ContractValidationError("TEST_MATRIX.json tests must be an array")
    identifiers = [row.get("id") for row in rows if isinstance(row, dict)]
    if len(identifiers) != len(rows) or len(set(identifiers)) != len(identifiers):
        raise ContractValidationError("test matrix IDs are missing or duplicated")
    counts = Counter(identifier[0] for identifier in identifiers)
    if dict(counts) != EXPECTED_PREFIX_COUNTS:
        raise ContractValidationError(
            f"test category counts mismatch: {dict(counts)} != {EXPECTED_PREFIX_COUNTS}"
        )
    if rows != parse_markdown_matrix():
        raise ContractValidationError("Markdown and JSON test matrices differ")
    return len(rows), counts


def validate_model_manifest() -> tuple[int, int]:
    cargo = tomllib.loads((ROOT / "model" / "Cargo.toml").read_text(encoding="utf-8"))
    package = cargo.get("package", {})
    if package.get("name") != "arcweft-lang-01-3-1-2-2-contract-model":
        raise ContractValidationError("reference model package name mismatch")
    if package.get("edition") != "2024" or package.get("publish") is not False:
        raise ContractValidationError("reference model edition/publish policy mismatch")
    if cargo.get("dependencies"):
        raise ContractValidationError("reference model must have no dependencies")
    if cargo.get("lints", {}).get("rust", {}).get("unsafe_code") != "forbid":
        raise ContractValidationError("reference model must forbid unsafe code")
    if cargo.get("lints", {}).get("clippy", {}).get("all") != "deny":
        raise ContractValidationError("reference model must deny clippy::all")

    rust_files = sorted((ROOT / "model" / "src").glob("*.rs"))
    if not rust_files:
        raise ContractValidationError("reference model has no Rust modules")
    maximum_lines = 0
    for path in rust_files:
        lines = len(path.read_text(encoding="utf-8").splitlines())
        maximum_lines = max(maximum_lines, lines)
        limit = 250 if path.name == "lib.rs" else 800
        if lines > limit:
            raise ContractValidationError(
                f"reference model module too large: {path.name} {lines} > {limit}"
            )
    return len(rust_files), maximum_lines


def validate_host() -> str:
    completed = subprocess.run(
        [sys.executable, str(ROOT / "host" / "validate_host_fixtures.py")],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        env={"PYTHONDONTWRITEBYTECODE": "1"},
    )
    if completed.returncode != 0:
        raise ContractValidationError(
            f"host fixture validation failed:\n{completed.stdout}{completed.stderr}"
        )
    return completed.stdout.strip().replace("\n", "; ")


def validate_manifest_if_present() -> int:
    manifest = ROOT / "MANIFEST.sha256"
    if not manifest.exists():
        return 0
    entries = 0
    for line in manifest.read_text(encoding="utf-8").splitlines():
        digest, separator, relative = line.partition("  ")
        if separator != "  " or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
            raise ContractValidationError(f"invalid manifest line: {line!r}")
        path = Path(relative)
        if path.is_absolute() or ".." in path.parts or relative == "MANIFEST.sha256":
            raise ContractValidationError(f"invalid manifest path: {relative}")
        target = ROOT / path
        if not target.is_file() or sha256(target) != digest:
            raise ContractValidationError(f"manifest mismatch: {relative}")
        entries += 1
    actual = {
        path.relative_to(ROOT).as_posix()
        for path in ROOT.rglob("*")
        if path.is_file() and path.name != "MANIFEST.sha256"
    }
    listed = {
        line.partition("  ")[2]
        for line in manifest.read_text(encoding="utf-8").splitlines()
    }
    if actual != listed:
        raise ContractValidationError(
            f"manifest file set mismatch: missing={sorted(actual-listed)}, extra={sorted(listed-actual)}"
        )
    return entries


def main() -> int:
    contract = validate_contract_index()
    text_files, total_bytes = validate_files(contract)
    tests, counts = validate_test_matrix()
    rust_modules, max_rust_lines = validate_model_manifest()
    host = validate_host()
    manifest_entries = validate_manifest_if_present()
    print("contract-package: PASS")
    print(f"baseline: {contract['repository']['commit']}")
    print(f"text-files: {text_files}")
    print(f"package-bytes-before-zip: {total_bytes}")
    print(f"test-matrix: {tests} {dict(counts)}")
    print(f"rust-modules: {rust_modules}; max-module-lines: {max_rust_lines}")
    print(f"host: {host}")
    print(f"manifest-entries: {manifest_entries}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ContractValidationError as error:
        print(f"contract-package: FAIL: {error}", file=sys.stderr)
        raise SystemExit(1) from error
