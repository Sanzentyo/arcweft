#!/usr/bin/env python3
"""Mechanical validator for the Lang-01.3.1.2.3 final-contract package."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import unittest

sys.dont_write_bytecode = True


PACKAGE_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_NAME = "MANIFEST.txt"
ZERO_SHA256 = "0" * 64

REQUIRED_FILES = {
    "README.md",
    "FINAL_STATUS.md",
    "OPEN_QUESTIONS.md",
    "SOURCE_REQUEST.md",
    "AGENTS_AND_RUST_POLICY.md",
    "INPUT_IDENTITIES.md",
    "FINAL_CONTRACT.md",
    "RUST_OWNERS_AND_APIS.md",
    "STRUCTURED_RUNTIME_TRANSFER_SEMANTICS.md",
    "AWBC_ABI2_OWNERSHIP_CONTRACT.md",
    "SNAPSHOT_SAVE_RESTORE_CONTRACT.md",
    "PLAN_HOST_REPLAY_PERSISTENCE.md",
    "SUPERSESSION_DELTA.md",
    "CONSUMER_AND_DELETION_INVENTORY.md",
    "IMPLEMENTATION_ORDER.md",
    "TEST_MATRIX.md",
    "TEST_MATRIX.json",
    "TEST_MATRIX.csv",
    "PARENT_TEST_MATRIX_INDEX.json",
    "REQUIREMENTS_TRACEABILITY.md",
    "REPOSITORY_EVIDENCE_AND_VERIFICATION_SCOPE.md",
    "contract.json",
    "reference_model/ownership_model.py",
    "reference_model/test_ownership_model.py",
    "validation/verify_contract.py",
    "validation/build_zip.py",
    "validation/VALIDATION_REPORT.md",
    "validation/reference-model-test-output.txt",
    MANIFEST_NAME,
}

FORBIDDEN_PARTS = {
    ".git",
    ".jj",
    "target",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".DS_Store",
}

EXPECTED_PARENT_COUNTS = {
    "Lang-01.3.1.2.1": 530,
    "Lang-01.3.1.2.2": 168,
    "Lang-01.3.1.2.2.1": 105,
}

EXPECTED_PARENT_ZIPS = {
    "Lang-01.3.1.2.1": "66809a1280a507f69bb78d9df3bf7af227a91cd68b86cf8771cbf9ee20aa856a",
    "Lang-01.3.1.2.2": "d1bd7fb5301509ca88be7c9d3662942ca88472d11143499c0c3067d626df9418",
    "Lang-01.3.1.2.2.1": "8aded7b1cb5d92f9d820c2cc82121ac6d070f3cf26d1618dc23ff144081090ad",
}

EXPECTED_PREFIX_COUNTS = {
    "AWBC": 53,
    "BOUND": 36,
    "CAP": 30,
    "DROP": 10,
    "DUP": 28,
    "FULL": 16,
    "OPS": 80,
    "OWN": 30,
    "PLAN": 20,
    "SNAP": 60,
    "STREAM": 20,
    "XFER": 12,
}


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def iter_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if any(part in FORBIDDEN_PARTS for part in relative.parts):
            raise AssertionError(f"forbidden path in package: {relative.as_posix()}")
        if path.is_symlink():
            raise AssertionError(f"symlink is forbidden: {relative.as_posix()}")
        if path.is_file():
            files.append(path)
    return sorted(files, key=lambda path: path.relative_to(root).as_posix())


def manifest_rows(root: Path) -> list[tuple[str, str]]:
    rows: list[tuple[str, str]] = []
    for path in iter_files(root):
        relative = path.relative_to(root).as_posix()
        digest = ZERO_SHA256 if relative == MANIFEST_NAME else sha256_bytes(path.read_bytes())
        rows.append((digest, relative))
    return rows


def write_manifest(root: Path) -> None:
    # MANIFEST must exist so its own 64-zero row is included.
    manifest = root / MANIFEST_NAME
    if not manifest.exists():
        manifest.write_text("", encoding="utf-8", newline="\n")
    content = "".join(f"{digest}  {relative}\n" for digest, relative in manifest_rows(root))
    manifest.write_text(content, encoding="utf-8", newline="\n")


def validate_manifest(root: Path) -> None:
    manifest = root / MANIFEST_NAME
    lines = manifest.read_text(encoding="utf-8").splitlines()
    parsed: list[tuple[str, str]] = []
    for line_number, line in enumerate(lines, start=1):
        if len(line) < 67 or line[64:66] != "  ":
            raise AssertionError(f"malformed manifest row {line_number}: {line!r}")
        digest, relative = line[:64], line[66:]
        if any(char not in "0123456789abcdef" for char in digest):
            raise AssertionError(f"non-lowercase SHA-256 at manifest row {line_number}")
        if not relative or relative.startswith("/") or ".." in Path(relative).parts:
            raise AssertionError(f"unsafe manifest path at row {line_number}: {relative!r}")
        parsed.append((digest, relative))

    expected = manifest_rows(root)
    if parsed != expected:
        parsed_paths = {path: digest for digest, path in parsed}
        expected_paths = {path: digest for digest, path in expected}
        missing = sorted(set(expected_paths) - set(parsed_paths))
        extra = sorted(set(parsed_paths) - set(expected_paths))
        mismatched = sorted(
            path
            for path in set(parsed_paths) & set(expected_paths)
            if parsed_paths[path] != expected_paths[path]
        )
        raise AssertionError(
            f"manifest mismatch: missing={missing}, extra={extra}, mismatched={mismatched}"
        )


def validate_utf8_lf(root: Path) -> None:
    for path in iter_files(root):
        data = path.read_bytes()
        relative = path.relative_to(root).as_posix()
        if b"\x00" in data:
            raise AssertionError(f"NUL byte in text package member: {relative}")
        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError as error:
            raise AssertionError(f"non-UTF-8 package member {relative}: {error}") from error
        if "\r" in text:
            raise AssertionError(f"non-LF line ending in {relative}")
        if data and not data.endswith(b"\n"):
            raise AssertionError(f"text member lacks final LF: {relative}")


def validate_required_members(root: Path) -> None:
    found = {path.relative_to(root).as_posix() for path in iter_files(root)}
    missing = sorted(REQUIRED_FILES - found)
    if missing:
        raise AssertionError(f"missing required package members: {missing}")


def validate_status(root: Path) -> None:
    if (root / "OPEN_QUESTIONS.md").read_bytes() != b"none\n":
        raise AssertionError('OPEN_QUESTIONS.md must be exactly "none\\n"')
    status = (root / "FINAL_STATUS.md").read_text(encoding="utf-8")
    required = {
        "STATUS=READY_FOR_IMPLEMENTATION",
        "OPEN_RESULT_CHANGING_DECISIONS=0",
        "OPEN_QUESTIONS=0",
        "BASELINE_GIT=177ba1e61e43fb2da2149869ce35e165d1e93b66",
        "PRODUCTION_CHANGES=0",
        "PRODUCTION_BUILD_VALIDATION=NOT_RUN",
        "CURRENT_MAIN_REPIN_REQUIRED=YES",
    }
    absent = sorted(value for value in required if value not in status)
    if absent:
        raise AssertionError(f"FINAL_STATUS.md lacks rows: {absent}")


def validate_contract_json(root: Path) -> None:
    contract = json.loads((root / "contract.json").read_text(encoding="utf-8"))
    assert contract["contract_id"] == "Lang-01.3.1.2.3"
    assert contract["status"] == "READY_FOR_IMPLEMENTATION"
    assert contract["open_questions"] == 0
    assert contract["open_result_changing_decisions"] == 0
    assert contract["baseline_git"] == "177ba1e61e43fb2da2149869ce35e165d1e93b66"
    assert contract["production_patch_included"] is False
    assert contract["current_main_repin_required"] is True
    assert contract["ownership"]["variants"] == ["Unrestricted", "Affine"]
    assert contract["runtime_value_api"]["runtime_value_clone"] is False
    assert contract["runtime_value_api"]["runtime_value_copy"] is False
    assert contract["capture"]["whole_environment_fallback"] is False
    assert contract["capture"]["source_reconstruction"] is False
    assert contract["sequence"]["slice"].startswith("source-preserving")
    assert contract["sequence"]["owner"].startswith("existing arcweft_core::value::RuntimeSeq")
    assert contract["sequence"]["parallel_wrapper"] is False
    assert contract["awbc"]["abi"] == 2
    assert contract["awbc"]["codec"] == 8
    assert contract["awbc"]["copy_value"]["opcode"] == "0x2a"
    assert contract["awbc"]["unknown_opcode_range_after_copy"] == "0x2b..=0x7f"
    assert contract["snapshot"]["alongside_install"] is False
    assert contract["runtime_payload"]["current_tuple_wrapper_replaced_in_place"] is True
    assert contract["runtime_payload"]["contains_runtime_value"] is False
    assert contract["runtime_payload"]["from_runtime_value"] is False
    assert contract["pattern"]["owner"] == "existing arcweft_core::pattern::RuntimePattern"
    assert contract["pattern"]["global_or_copied_registry"] is False
    assert contract["plan"]["removed_variants"] == [
        "RuntimeExpr::Value(RuntimeValue)",
        "RuntimePattern::Literal(RuntimeValue)",
    ]
    assert contract["plan"]["expression_replacement"] == "RuntimeExpr::Constant(RuntimeConstantId)"
    assert contract["plan"]["pattern_replacement"] == "RuntimePattern::Literal(RuntimeConstantId)"
    assert contract["plan"]["constant_entry"] == "RuntimePlanConstant(RuntimePayload)"
    assert contract["plan"]["direct_runtime_plan_serde"] is False
    assert contract["plan"]["runtime_plan_clone"] is False
    assert contract["plan"]["cache_owner"] == "Arc<RuntimePlan>"
    assert contract["plan"]["engine_plan_owner"] == "Arc<RuntimePlan>"
    assert contract["plan"]["pending_op_values"] is False
    assert contract["plan"]["engine_clone"] is False
    assert contract["plan"]["fiber_clone"] is False
    assert contract["plan"]["optional_binding_plan_invariant"] == "binding.is_some() == binding_plan.is_some()"
    assert "owner FlowCursor" in contract["plan"]["suspension_plan_reference"]
    assert "JoiningChildren" in contract["plan"]["child_join_reference"]
    assert contract["plan"]["removed_runtime_flow_variants"] == [
        "FlowOp::Bind(Vec<RuntimeBinding>)",
        "FlowOp::LoopNext",
        "FlowOp::WhileNext",
        "FlowOp::WhileLetNext",
        "FlowOp::ForNext",
    ]
    assert contract["tests"]["new_case_count"] == 395
    assert contract["tests"]["retained_parent_case_count"] == 803
    assert contract["tests"]["reference_model_case_count"] == 20

    parents = {row["contract_id"]: row for row in contract["parents"]}
    assert set(parents) == set(EXPECTED_PARENT_COUNTS)
    for contract_id, count in EXPECTED_PARENT_COUNTS.items():
        assert parents[contract_id]["retained_test_cases"] == count
        assert parents[contract_id]["zip_sha256"] == EXPECTED_PARENT_ZIPS[contract_id]


def validate_new_test_matrix(root: Path) -> None:
    matrix = json.loads((root / "TEST_MATRIX.json").read_text(encoding="utf-8"))
    cases = matrix["cases"]
    assert matrix["contract_id"] == "Lang-01.3.1.2.3"
    assert matrix["case_count"] == 395 == len(cases)
    ids = [row["id"] for row in cases]
    if len(ids) != len(set(ids)):
        raise AssertionError("duplicate IDs in TEST_MATRIX.json")
    expected_fields = {
        "id",
        "requirement",
        "area",
        "kind",
        "owner",
        "stage",
        "setup",
        "action",
        "expected",
        "evidence",
    }
    for row in cases:
        if set(row) != expected_fields:
            raise AssertionError(f"wrong fields for {row.get('id')}: {sorted(row)}")
        if not all(isinstance(row[field], str) and row[field] for field in expected_fields):
            raise AssertionError(f"empty/non-string test field in {row.get('id')}")

    prefix_counts: dict[str, int] = {}
    for test_id in ids:
        prefix = test_id.split("-", 1)[0]
        prefix_counts[prefix] = prefix_counts.get(prefix, 0) + 1
    if prefix_counts != EXPECTED_PREFIX_COUNTS:
        raise AssertionError(f"test prefix counts changed: {prefix_counts}")

    requirements = {row["requirement"] for row in cases}
    assert requirements == {f"R{index}" for index in range(1, 11)}

    by_id = {row["id"]: row for row in cases}
    assert "RuntimeSeq" in by_id["OWN-022"]["setup"]
    assert "RuntimeSequenceValue" not in by_id["OWN-022"]["setup"]
    assert "closed" in by_id["PLAN-011"]["expected"]
    assert "RuntimePattern::Literal(RuntimeValue)" in by_id["PLAN-018"]["setup"]
    assert "borrow" in by_id["PLAN-019"]["expected"].lower()
    assert "arc<runtimeplan>" in by_id["PLAN-010"]["expected"].lower()
    assert "flowcontrolstackentrykind::for" in by_id["PLAN-010"]["expected"].lower()
    assert "all five runtime-only flowop variants" in by_id["PLAN-018"]["expected"].lower()
    assert all("arcweft-core::value::RuntimeEnv" in row["owner"] for row in cases if row["id"].startswith("CAP-"))

    with (root / "TEST_MATRIX.csv").open(encoding="utf-8", newline="") as stream:
        csv_rows = list(csv.DictReader(stream))
    if csv_rows != cases:
        raise AssertionError("TEST_MATRIX.csv rows differ from TEST_MATRIX.json")


def validate_parent_test_matrix(root: Path) -> None:
    parent_index = json.loads(
        (root / "PARENT_TEST_MATRIX_INDEX.json").read_text(encoding="utf-8")
    )
    assert parent_index["retained_case_count"] == 803
    rows = parent_index["parents"]
    assert len(rows) == 3
    observed_total = 0
    all_qualified_ids: set[tuple[str, str]] = set()
    for parent in rows:
        contract_id = parent["contract_id"]
        assert contract_id in EXPECTED_PARENT_COUNTS
        assert parent["source_zip_sha256"] == EXPECTED_PARENT_ZIPS[contract_id]
        assert parent["case_count"] == EXPECTED_PARENT_COUNTS[contract_id]
        assert len(parent["cases"]) == parent["case_count"]
        observed_total += parent["case_count"]
        for index, row in enumerate(parent["cases"]):
            test_id = row.get("id") or row.get("test_id") or f"index:{index}"
            qualified = (contract_id, str(test_id))
            if qualified in all_qualified_ids:
                raise AssertionError(f"duplicate qualified parent test ID: {qualified}")
            all_qualified_ids.add(qualified)
    assert observed_total == 803


def validate_traceability(root: Path) -> None:
    text = (root / "REQUIREMENTS_TRACEABILITY.md").read_text(encoding="utf-8")
    for index in range(1, 11):
        if f"R{index}." not in text:
            raise AssertionError(f"traceability lacks R{index}")
    required_members = [
        "RUST_OWNERS_AND_APIS.md",
        "STRUCTURED_RUNTIME_TRANSFER_SEMANTICS.md",
        "AWBC_ABI2_OWNERSHIP_CONTRACT.md",
        "SNAPSHOT_SAVE_RESTORE_CONTRACT.md",
        "SUPERSESSION_DELTA.md",
        "CONSUMER_AND_DELETION_INVENTORY.md",
        "IMPLEMENTATION_ORDER.md",
        "PARENT_TEST_MATRIX_INDEX.json",
    ]
    for member in required_members:
        if member not in text:
            raise AssertionError(f"traceability lacks member reference: {member}")


def validate_normative_markers(root: Path) -> None:
    final_contract = (root / "FINAL_CONTRACT.md").read_text(encoding="utf-8")
    rust_api = (root / "RUST_OWNERS_AND_APIS.md").read_text(encoding="utf-8")
    awbc = (root / "AWBC_ABI2_OWNERSHIP_CONTRACT.md").read_text(encoding="utf-8")
    snapshot = (root / "SNAPSHOT_SAVE_RESTORE_CONTRACT.md").read_text(encoding="utf-8")
    implementation = (root / "IMPLEMENTATION_ORDER.md").read_text(encoding="utf-8")

    markers = {
        "FINAL_CONTRACT.md": [
            "RuntimeValueOwnership::{Unrestricted, Affine}",
            "try_duplicate_unrestricted",
            "Repeat, indexing, slicing, push",
            "CopyValue",
            "dormant",
        ],
        "RUST_OWNERS_AND_APIS.md": [
            "pub enum RuntimeValueOwnership",
            "pub fn try_duplicate_unrestricted",
            "pub enum RuntimeCaptureMode",
            "`RuntimeExpr::Value(RuntimeValue)` is deleted",
            "`RuntimePattern::Literal(RuntimeValue)` is simultaneously replaced",
            "The current `RuntimePayload(pub RuntimeValue)` wrapper cannot remain",
            "No `RuntimeSequenceValue` wrapper is introduced",
            "FlowOp::Bind(Vec<RuntimeBinding>)",
            "FlowFiber.pending_ops: VecDeque<FlowOp>",
            "pub fn new(plan: Arc<RuntimePlan>) -> Self",
        ],
        "AWBC_ABI2_OWNERSHIP_CONTRACT.md": [
            "opcode 0x2a",
            "0x2b..=0x7f",
            "existing `Move`",
            "Drop=0x1f",
        ],
        "SNAPSHOT_SAVE_RESTORE_CONTRACT.md": [
            "Exact tamper rejection order",
            "try_restore_empty",
            "frozen",
            "dormant",
        ],
        "IMPLEMENTATION_ORDER.md": [
            "G3.4",
            "P4 + C1",
            "P6 + C4",
            "P8 + C6",
        ],
    }
    documents = {
        "FINAL_CONTRACT.md": final_contract,
        "RUST_OWNERS_AND_APIS.md": rust_api,
        "AWBC_ABI2_OWNERSHIP_CONTRACT.md": awbc,
        "SNAPSHOT_SAVE_RESTORE_CONTRACT.md": snapshot,
        "IMPLEMENTATION_ORDER.md": implementation,
    }
    for name, required in markers.items():
        for marker in required:
            if marker not in documents[name]:
                raise AssertionError(f"{name} lacks normative marker {marker!r}")


def validate_reference_model(root: Path, *, run_tests: bool) -> int:
    model_dir = root / "reference_model"
    sys.path.insert(0, str(model_dir))
    try:
        suite = unittest.defaultTestLoader.discover(str(model_dir), pattern="test_*.py")
        count = suite.countTestCases()
    finally:
        sys.path.pop(0)
    if count != 20:
        raise AssertionError(f"expected 20 reference-model tests, found {count}")

    if run_tests:
        completed = subprocess.run(
            [sys.executable, "-B", "-m", "unittest", "discover", "-s", str(model_dir), "-p", "test_*.py", "-v"],
            cwd=root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        if completed.returncode != 0:
            raise AssertionError(f"reference-model tests failed:\n{completed.stdout}")
    return count


def validate(root: Path, *, run_reference_tests: bool = True) -> dict[str, int]:
    validate_required_members(root)
    validate_utf8_lf(root)
    validate_status(root)
    validate_contract_json(root)
    validate_new_test_matrix(root)
    validate_parent_test_matrix(root)
    validate_traceability(root)
    validate_normative_markers(root)
    reference_tests = validate_reference_model(root, run_tests=run_reference_tests)
    validate_manifest(root)
    return {
        "files": len(iter_files(root)),
        "new_tests": 395,
        "parent_tests": 803,
        "reference_tests": reference_tests,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root",
        type=Path,
        default=PACKAGE_ROOT,
        help="package root (defaults to the parent of validation/)",
    )
    parser.add_argument(
        "--write-manifest",
        action="store_true",
        help="write deterministic MANIFEST.txt before validating",
    )
    parser.add_argument(
        "--skip-reference-tests",
        action="store_true",
        help="validate test discovery/count without executing the tests",
    )
    args = parser.parse_args()
    root = args.root.resolve()
    if args.write_manifest:
        write_manifest(root)
    result = validate(root, run_reference_tests=not args.skip_reference_tests)
    print(
        "PASS "
        + " ".join(f"{key}={value}" for key, value in sorted(result.items()))
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
