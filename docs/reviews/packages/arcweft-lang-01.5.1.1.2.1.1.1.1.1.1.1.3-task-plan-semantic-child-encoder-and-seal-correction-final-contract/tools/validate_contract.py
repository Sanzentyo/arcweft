#!/usr/bin/env python3
"""Read-only validator for the task-plan semantic seal final contract.

It validates an extracted package directory or the returned ZIP. Repository
mode additionally checks the pinned Git commit and structured Cargo metadata;
it does not modify the repository.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

ARCHIVE = (
    "arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-"
    "task-plan-semantic-child-encoder-and-seal-correction-final-contract.zip"
)
ROOT_NAME = ARCHIVE.removesuffix(".zip")
COMMIT = "515bb071437c3af053f1560c3119906dc8002efc"
MANIFEST_EXCLUDED = {"MANIFEST.json", "MANIFEST.sha256", "CHECKSUMS.sha256"}
REQUIRED = {
    "README.md",
    "FINAL_CONTRACT.md",
    "RUST_SCHEMAS.md",
    "TRANSCRIPTS.md",
    "EXECUTABLE_TRANSCRIPT.md",
    "CYCLE_PROOF.md",
    "PRIVATE_CODEC_AND_EXPECTED_KEYS.md",
    "SEAL_STATE_MACHINES.md",
    "ERROR_PRECEDENCE_AND_LIMITS.md",
    "OWNER_CONSUMER_MATRIX.md",
    "DEPENDENCY_MATRIX.md",
    "COMPILE_CLEAN_SEQUENCE.md",
    "DECISION_REGISTER.md",
    "TEST_MATRIX.md",
    "SOURCE_INVENTORY.md",
    "VALIDATION_REPORT.md",
    "FINAL_STATUS",
    "OPEN_QUESTIONS",
    "schemas/final_contract.rs",
    "machine/contract.json",
    "machine/dependencies.json",
    "machine/limits.json",
    "machine/tests.json",
    "tables/transcript_rows.csv",
    "tables/dependencies.csv",
    "tables/owner_consumer.csv",
    "tables/error_precedence.csv",
    "tables/tests.csv",
    "tools/validate_contract.py",
    "tools/negative_self_tests.py",
    "tools/run_validation.sh",
    "inputs/CURRENT_REQUEST.md",
    "inputs/RUST_SKILL.md",
    "inputs/PROJECT_PREMISE.txt",
    "MANIFEST.json",
    "MANIFEST.sha256",
    "CHECKSUMS.sha256",
}


class ValidationError(RuntimeError):
    pass


def fail(message: str) -> None:
    raise ValidationError(message)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def safe_member(name: str) -> PurePosixPath:
    p = PurePosixPath(name)
    if not name or name.endswith("/"):
        return p
    if p.is_absolute() or ".." in p.parts:
        fail(f"unsafe ZIP member path: {name!r}")
    if re.match(r"^[A-Za-z]:", name):
        fail(f"drive-qualified ZIP member path: {name!r}")
    if "\\" in name:
        fail(f"backslash ZIP member path: {name!r}")
    return p


def inspect_zip(path: Path) -> tuple[tempfile.TemporaryDirectory[str], Path]:
    if path.name != ARCHIVE:
        fail(f"ZIP filename must be exactly {ARCHIVE!r}, got {path.name!r}")
    tmp = tempfile.TemporaryDirectory(prefix="arcweft-task-plan-contract-")
    tmp_path = Path(tmp.name)
    with zipfile.ZipFile(path) as zf:
        infos = zf.infolist()
        names = [info.filename for info in infos if not info.is_dir()]
        if len(names) != len(set(names)):
            fail("ZIP contains duplicate file member names")
        folded: dict[str, str] = {}
        for name in names:
            safe_member(name)
            key = name.casefold()
            if key in folded and folded[key] != name:
                fail(f"ZIP contains case-fold collision: {folded[key]!r}, {name!r}")
            folded[key] = name
            parts = PurePosixPath(name).parts
            if not parts or parts[0] != ROOT_NAME:
                fail(f"ZIP member is outside exact top-level wrapper {ROOT_NAME!r}: {name!r}")
        zf.extractall(tmp_path)
    return tmp, tmp_path / ROOT_NAME


def list_payload(root: Path) -> set[str]:
    return {
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_file()
    }


def load_json(path: Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        fail(f"invalid JSON {path}: {exc}")
    if not isinstance(value, dict):
        fail(f"JSON root must be object: {path}")
    return value


def validate_manifest(root: Path) -> None:
    manifest_path = root / "MANIFEST.json"
    manifest_bytes = manifest_path.read_bytes()
    expected_manifest_hash = (root / "MANIFEST.sha256").read_text(encoding="utf-8").strip()
    actual_manifest_hash = sha256_bytes(manifest_bytes)
    if expected_manifest_hash != actual_manifest_hash:
        fail("MANIFEST.sha256 does not match MANIFEST.json")
    manifest = load_json(manifest_path)
    if manifest.get("schema_version") != 1:
        fail("manifest schema_version must be exactly 1")
    if manifest.get("root") != ROOT_NAME:
        fail("manifest root mismatch")
    rows = manifest.get("files")
    if not isinstance(rows, list):
        fail("manifest files must be a list")
    seen: set[str] = set()
    for row in rows:
        if not isinstance(row, dict):
            fail("manifest file row must be object")
        rel = row.get("path")
        if not isinstance(rel, str) or rel in seen or rel in MANIFEST_EXCLUDED:
            fail(f"invalid/duplicate manifest path: {rel!r}")
        safe_member(rel)
        seen.add(rel)
        path = root / rel
        if not path.is_file():
            fail(f"manifest file missing: {rel}")
        if row.get("bytes") != path.stat().st_size:
            fail(f"manifest size mismatch: {rel}")
        if row.get("sha256") != sha256_file(path):
            fail(f"manifest hash mismatch: {rel}")
    payload = list_payload(root)
    expected_rows = payload - MANIFEST_EXCLUDED
    if seen != expected_rows:
        fail(
            "manifest file set mismatch: missing="
            f"{sorted(expected_rows - seen)}, extra={sorted(seen - expected_rows)}"
        )

    checksums_path = root / "CHECKSUMS.sha256"
    checksum_rows: dict[str, str] = {}
    for line in checksums_path.read_text(encoding="utf-8").splitlines():
        if not line:
            continue
        try:
            digest, rel = line.split("  ", 1)
        except ValueError:
            fail(f"malformed CHECKSUMS.sha256 line: {line!r}")
        if rel in checksum_rows:
            fail(f"duplicate checksum path: {rel}")
        checksum_rows[rel] = digest
    expected_checksum_files = payload - {"CHECKSUMS.sha256"}
    if set(checksum_rows) != expected_checksum_files:
        fail("CHECKSUMS.sha256 file set mismatch")
    for rel, digest in checksum_rows.items():
        if digest != sha256_file(root / rel):
            fail(f"CHECKSUMS.sha256 mismatch: {rel}")


def extract_struct(text: str, name: str) -> str:
    match = re.search(rf"pub struct {re.escape(name)}(?:<'a>)?\s*\{{", text)
    if not match:
        fail(f"missing struct {name}")
    start = match.end()
    depth = 1
    i = start
    while i < len(text) and depth:
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
        i += 1
    if depth:
        fail(f"unclosed struct {name}")
    return text[start : i - 1]


def validate_schema(root: Path) -> None:
    schema = (root / "schemas/final_contract.rs").read_text(encoding="utf-8")
    row = extract_struct(schema, "RuntimeTaskPlan")
    expected = [
        "producer_function:", "family:", "class:", "request_template:",
        "control_effect:", "binding:"
    ]
    for token in expected:
        if token not in row:
            fail(f"RuntimeTaskPlan missing field {token}")
    forbidden = [
        "semantic_digest:", "expected_digest:", "producer_contract:",
        "producer_site:", "payload_type:", "arguments:", "generation:",
        "policy:", "launch_ordinal:", "priority:", "cancel_scope:",
        "debug_label:", "view_program:", "view_site:", "view_admission:",
    ]
    for token in forbidden:
        if token in row:
            fail(f"RuntimeTaskPlan contains forbidden field {token}")

    for required in [
        "pub enum RuntimeTaskSemanticBinding",
        "Ordinary,",
        "View,",
        "AwaitManyBase,",
        "AwaitManyChild,",
        "Timeout {",
        "Line {",
        "pub struct RuntimeTaskPlanDigestBase<'a>",
        "pub struct ViewTaskPlanDigestRequest<'a>",
        "pub trait ViewTaskPlanAuthority",
        "fn task_plan_semantic_digest(",
        "finish_authority_transcript(",
        "pub struct ValidatedViewTaskPlanBinding",
        "pub struct ValidatedViewProgramResource",
        "fn validate_runtime_task_binding(",
        "fn semantic_digest(",
        "ExpectedTaskPlanKey([u8; 32])",
    ]:
        if required not in schema:
            fail(f"schema missing required token: {required}")

    base_start = schema.index("pub struct RuntimeTaskPlanDigestBase<'a>")
    prefix = schema[max(0, base_start - 180):base_start]
    if "derive(Clone" in prefix or "derive(Copy" in prefix:
        fail("RuntimeTaskPlanDigestBase must not derive Clone/Copy")
    request_start = schema.index("pub struct ViewTaskPlanDigestRequest<'a>")
    request_prefix = schema[max(0, request_start - 180):request_start]
    if "derive(Clone" in request_prefix or "derive(Copy" in request_prefix:
        fail("ViewTaskPlanDigestRequest must not derive Clone/Copy")

    if re.search(r"pub\s+(?:const\s+)?fn\s+(?:try_)?from_bytes\s*\(", schema):
        fail("public raw digest constructor is forbidden")
    if re.search(r"impl\s+(?:Try)?From\s*<\s*\[u8;\s*32\]\s*>", schema):
        fail("raw digest From/TryFrom implementation is forbidden")
    if "pub struct ViewProgramIdProjection" in schema or "pub struct ViewMatchSiteIdProjection" in schema:
        fail("raw core View projection is forbidden")
    if re.search(r"pub\s+fn\s+\w*\s*\([^)]*(?:dyn\s+std::io::Write|&mut\s+blake3::Hasher|byte_sink|caller_sink)", schema, re.S):
        fail("caller/general byte sink API is forbidden")
    if re.search(r"pub struct ExpectedTaskPlanKey", schema):
        fail("expected task-plan key must remain private")
    if "unsafe {" in schema or "Box::leak" in schema or "std::mem::forget" in schema:
        fail("unsafe/leak/forget is forbidden")


def validate_transcripts(root: Path) -> None:
    text = (root / "TRANSCRIPTS.md").read_text(encoding="utf-8")
    required = [
        'domain = "arcweft.task.plan-semantic.v1\\0"',
        'domain = "arcweft.runtime-plan.executable-semantic.v1\\0"',
        'domain = "arcweft.runtime-plan.producer-function-semantic.v1\\0"',
        'domain = "arcweft.task.request-template.v1\\0"',
        'domain = "arcweft.task.control-effect-contract.v1\\0"',
        "table_count:u8 = 15",
        "0 Ordinary:", "1 View:", "2 AwaitManyBase:",
        "3 AwaitManyChild:", "4 Timeout:", "5 Line:",
        "AcceptedViewProgramRevision` is checked but not written",
    ]
    for token in required:
        if token not in text:
            fail(f"transcript missing authoritative token: {token}")
    for tag in range(15):
        if not re.search(rf"\|\s*{tag}\s*\|", (root / "EXECUTABLE_TRANSCRIPT.md").read_text(encoding="utf-8")):
            fail(f"executable transcript lacks table tag {tag}")


def validate_machine(root: Path) -> None:
    contract = load_json(root / "machine/contract.json")
    if contract.get("schema_version") != 1:
        fail("contract schema_version must be exactly 1")
    if contract.get("final_status") != "READY_FOR_IMPLEMENTATION":
        fail("machine final_status mismatch")
    if contract.get("open_questions") != "none":
        fail("machine open_questions must be none")
    if contract.get("repository", {}).get("commit") != COMMIT:
        fail("machine repository commit mismatch")
    markers = contract.get("version_markers")
    if not isinstance(markers, dict) or not markers or any(value != 1 for value in markers.values()):
        fail("every Arcweft-owned version marker must be exactly 1")
    domains = contract.get("domains", {})
    if not domains or any(".v1\\0" not in value for value in domains.values()):
        fail("every semantic domain must be explicit version one with NUL")
    api = contract.get("digest_public_api", {})
    if api.get("from_bytes") or api.get("try_from_bytes") or api.get("serde"):
        fail("machine API permits raw/serde digest construction")
    expected_fields = [
        "producer_function", "family", "class", "request_template",
        "control_effect", "binding"
    ]
    if contract.get("runtime_task_plan", {}).get("fields") != expected_fields:
        fail("machine RuntimeTaskPlan field set mismatch")
    if contract.get("core_view_copy") is not False:
        fail("machine contract permits core View copy")
    authority = contract.get("view_authority", {})
    if authority.get("general_byte_sink") or authority.get("caller_digest_fields"):
        fail("machine contract permits caller sink/digest fields")
    if authority.get("validation_only_fields") != ["AcceptedViewProgramRevision"]:
        fail("accepted View revision role mismatch")
    if authority.get("hashed_fields") != [
        "ViewProgramId", "ViewMatchSiteId", "CheckedViewMatchAdmissionDigest"
    ]:
        fail("View binding hash field set mismatch")
    tables = contract.get("executable_tables")
    if not isinstance(tables, list) or [row.get("tag") for row in tables] != list(range(15)):
        fail("executable table tags must be exact dense 0..14")
    limits = contract.get("limits", {})
    expected_limits = {
        "max_task_plan_rows": 65536,
        "max_executable_rows": 1048576,
        "max_children_per_row": 65536,
        "max_function_roles": 65536,
        "max_request_roles": 65536,
        "max_control_effect_rows": 65536,
        "max_view_bindings": 65536,
        "max_transcript_bytes": 67108864,
        "max_semantic_work": 4194304,
    }
    if limits != expected_limits:
        fail("machine limit table mismatch")
    if contract.get("cut_publication") != 5:
        fail("final row/table must publish only in Cut 5")
    if contract.get("open_decisions") != []:
        fail("machine contract contains open decisions")

    dep = load_json(root / "machine/dependencies.json")
    if dep.get("schema_version") != 1:
        fail("dependency schema version must be one")
    edges = dep.get("edges")
    if not isinstance(edges, list):
        fail("dependency edges missing")
    edge_map = {(r.get("from"), r.get("to")): r.get("allowed") for r in edges}
    if edge_map.get(("arcweft-core", "arcweft-view")) is not False:
        fail("core->View must be forbidden")
    if edge_map.get(("arcweft-core", "arcweft-bundle")) is not False:
        fail("core->bundle must be forbidden")
    if edge_map.get(("arcweft-bundle", "arcweft-core")) is not True:
        fail("bundle->core must be allowed")
    if edge_map.get(("arcweft-bundle", "arcweft-view")) is not True:
        fail("bundle->View must be allowed")


def validate_status_and_required(root: Path) -> None:
    if not root.is_dir():
        fail(f"package root is not a directory: {root}")
    payload = list_payload(root)
    missing = REQUIRED - payload
    if missing:
        fail(f"required files missing: {sorted(missing)}")
    if (root / "FINAL_STATUS").read_text(encoding="utf-8") != "READY_FOR_IMPLEMENTATION\n":
        fail("FINAL_STATUS must be exactly READY_FOR_IMPLEMENTATION")
    if (root / "OPEN_QUESTIONS").read_text(encoding="utf-8") != "none\n":
        fail("OPEN_QUESTIONS must be exactly none")
    request = (root / "inputs/CURRENT_REQUEST.md").read_text(encoding="utf-8")
    if "task-plan semantic child encoder and seal correction" not in request:
        fail("current request input is not the requested child")
    if ARCHIVE not in request:
        fail("current request input lacks exact returned archive name")
    rust_skill = (root / "inputs/RUST_SKILL.md").read_text(encoding="utf-8")
    if "cargo clippy --all-targets" not in rust_skill or "Rust script" not in rust_skill:
        fail("Rust Skill input appears incomplete")


def cargo_package(metadata: dict, name: str) -> dict:
    for package in metadata.get("packages", []):
        if package.get("name") == name:
            return package
    fail(f"cargo metadata missing package {name}")


def dep_names(package: dict) -> set[str]:
    names = set()
    for dep in package.get("dependencies", []):
        names.add(dep.get("rename") or dep.get("name"))
        names.add(dep.get("name"))
    return {name for name in names if isinstance(name, str)}


def validate_repo(repo: Path) -> None:
    if not (repo / ".git").exists():
        fail(f"repository path has no .git: {repo}")
    result = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        check=True, capture_output=True, text=True
    )
    if result.stdout.strip() != COMMIT:
        fail(f"repository HEAD must equal inspected commit {COMMIT}")
    for rel in [
        "AGENTS.md", "crates/AGENTS.md", "docs/AGENTS.md", "docs/reviews/AGENTS.md",
        "crates/arcweft-core/src/plan.rs",
        "crates/arcweft-core/src/plan/construction.rs",
        "crates/arcweft-bundle/src/resource_codec/view/validated.rs",
        "crates/arcweft-view/src/view/identity.rs",
    ]:
        if not (repo / rel).is_file():
            fail(f"repository evidence path missing: {rel}")
    meta_result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=repo, check=True, capture_output=True, text=True
    )
    metadata = json.loads(meta_result.stdout)
    core = dep_names(cargo_package(metadata, "arcweft-core"))
    bundle = dep_names(cargo_package(metadata, "arcweft-bundle"))
    if "arcweft-view" in core or "arcweft-bundle" in core:
        fail("Cargo metadata contains forbidden core reverse dependency")
    if not {"arcweft-core", "arcweft-view"}.issubset(bundle):
        fail("bundle must depend directly on core and View")


def validate_root(root: Path, repo: Path | None = None) -> None:
    validate_status_and_required(root)
    validate_manifest(root)
    validate_machine(root)
    validate_schema(root)
    validate_transcripts(root)
    if repo is not None:
        validate_repo(repo.resolve())


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    parser.add_argument("--repo", type=Path)
    args = parser.parse_args(argv)

    temp: tempfile.TemporaryDirectory[str] | None = None
    try:
        package = args.package.resolve()
        if package.is_file():
            temp, root = inspect_zip(package)
        else:
            root = package
            if root.name != ROOT_NAME:
                fail(f"directory root must be named exactly {ROOT_NAME!r}")
        validate_root(root, args.repo)
    except (ValidationError, OSError, subprocess.CalledProcessError, zipfile.BadZipFile) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    finally:
        if temp is not None:
            temp.cleanup()
    print("PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
