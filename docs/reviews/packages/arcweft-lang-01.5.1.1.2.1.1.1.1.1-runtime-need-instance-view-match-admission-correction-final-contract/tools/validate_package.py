#!/usr/bin/env python3
"""Read-only validator for the Lang-01.5.1.1.2.1.1.1.1.1 design archive."""

from __future__ import annotations

import argparse
import copy
import csv
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import sys
import tempfile
import zipfile

PACKAGE = "arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract"
INSPECTED_SHA = "17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc"
REQUEST_SHA256 = "0152f1dd5f6fd315722f729700d3b94d1b0daa596a59445313e7796bddde8322"
REQUEST_GIT_BLOB = "7ed008dec6eddb820e228ea0803bf97a1ead2c36"
RUST_SHA256 = "1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665"
PREMISE_SHA256 = "cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1"

EXPECTED_DOMAINS = {
    "producer_instance": "arcweft.need.producer-instance.v1\0",
    "need": "arcweft.need.id.v1\0",
    "task_key": "arcweft.task.key.v1\0",
    "task_id": "arcweft.task.id.v1\0",
    "producer_contract": "arcweft.need.producer-contract.v1\0",
    "task_plan": "arcweft.task.plan-semantic.v1\0",
    "checked_match": "arcweft.lang.checked-match-semantic.v1\0",
    "ownership_evidence": "arcweft.lang.ownership-evidence.v1\0",
    "producer_admission": "arcweft.lang.need-producer-admission.v1\0",
    "view_admission": "arcweft.view.checked-match-admission.v1\0",
    "view_site": "arcweft.view.match-site.v1\0",
    "task_event": "arcweft.task.event.v1\0",
}

EXPECTED_FAMILIES = [
    "StructuredTaskPlan",
    "AwbcTaskPlan",
    "ViewMatchSubscription",
    "AwaitManyBase",
    "AwaitManyChild",
    "Timeout",
    "LineTask",
    "HostAdapterTask",
    "MakeNeedHandle",
]

EXPECTED_TYPEKINDS = {
    "Bool","I8","I16","I32","I64","I128","ISize",
    "U8","U16","U32","U64","U128","USize","F32","F64","String","Char",
    "Bytes","TextCluster","Duration","Progress","StageApi","LineContext",
    "StageActorHandle","CueHandle","VoiceHandle","Range","IteratorState",
    "DisplayText","DebugStatePath","ObservationFieldPath","Ref","Probe",
    "Predicate","Observation","ObservedObject","AgentBBox","ActionName",
    "ActionTarget","ActionResult","AgentValue","DataFormat","DataShape",
    "AgentEntityMetadata","AgentSourceAnchor","AgentProjectGraphNeighborhood",
    "AgentProjectGraphSymbol","AgentProjectGraphEdge","CaptureTarget","CaptureRef",
    "AgentResource","AgentResourceBody","RagContextPack","AgentBuiltin","Vec",
    "Array","Slice","Seq","Map","BorrowRef","Need","Stream","Result","Option",
    "Handle","ThreadHandle","Shared","Function","GenericParam","ProjectNominal",
    "AcceptedNominal","OpenNominal","Error","Projection","CharacterPatch",
    "FocusPatch","CharacterDialogue","DialogueLine","ViewValue",
    "CharacterNominal","Named","Tuple","Choice","Unit","Never",
}

REQUIRED_FILES = {
    "README.md",
    "OPEN_QUESTIONS.md",
    "FINAL_CONTRACT.md",
    "DECISION_REGISTER.md",
    "RUST_SCHEMAS.md",
    "OWNER_API_MAP.md",
    "DEPENDENCY_GRAPH.md",
    "IDENTITY_AND_DIGESTS.md",
    "TASK_LIFECYCLE_AND_PERSISTENCE.md",
    "CHECKED_MATCH_AND_VIEW_ADMISSION.md",
    "OWNERSHIP_EVIDENCE.md",
    "FAILURE_PRECEDENCE_AND_ATOMICITY.md",
    "COMPILE_CLEAN_SEQUENCE.md",
    "DELETION_MATRIX.md",
    "SOURCE_EVIDENCE.md",
    "REQUIREMENT_TRACEABILITY.md",
    "TEST_MATRIX.md",
    "STRUCTURAL_ABSENCE.md",
    "VALIDATION_SCOPE.md",
    "VALIDATION.md",
    "VALIDATION_OUTPUT.txt",
    "FINAL_STATUS.md",
    "inputs/CURRENT_REQUEST.md",
    "inputs/RUST_SKILL.txt",
    "inputs/PROJECT_PREMISE.txt",
    "machine/contract.json",
    "machine/limits.json",
    "machine/decisions.json",
    "machine/tests.json",
    "machine/traceability.json",
    "machine/source_evidence.json",
    "machine/request_hashes.json",
    "machine/ownership_matrix.json",
    "machine/deletion_matrix.json",
    "machine/task_policy.json",
    "machine/producer_families.json",
    "machine/validation.json",
    "tables/task_policy_truth_table.csv",
    "tables/ownership_matrix.csv",
    "tables/tests.csv",
    "tables/deletion_matrix.csv",
    "tools/validate_package.py",
    "MANIFEST.json",
    "MANIFEST.sha256",
}

MANDATORY_TRACE = (
    [f"A{i}" for i in range(1, 8)]
    + [f"B{i}" for i in range(1, 6)]
    + [f"C{i}" for i in range(1, 7)]
    + [f"D{i}" for i in range(1, 7)]
    + [f"E{i}" for i in range(1, 5)]
    + [f"F{i}" for i in range(1, 7)]
    + [f"CUT{i}" for i in range(1, 6)]
    + [f"ART{i}" for i in range(1, 16)]
)

REQUIRED_TEST_IDS = {
    "TASK-001","TASK-002","TASK-003","TASK-004","TASK-005","TASK-007",
    "ID-004","ID-005","ID-007","ID-008","ID-009","ID-010","ID-011",
    "ID-012","ID-016","DIG-001","DIG-002","DIG-003","AWAIT-001",
    "AWAIT-006","AWAIT-007","AWAIT-009","AWAIT-010","AWAIT-015",
    "AWAIT-016","EVT-002","EVT-003","EVT-004","EVT-009","EVT-019",
    "MATCH-001","MATCH-004","MATCH-005","MATCH-006","MATCH-018",
    "VIEW-001","VIEW-007","VIEW-008","VIEW-009","VIEW-010","VIEW-011",
    "VIEW-013","VIEW-015","OWN-001","OWN-002","OWN-003","OWN-005",
    "OWN-009","OWN-010","OWN-012","OWN-015","STR-001","STR-002",
    "STR-003","STR-004","STR-006","STR-008","STR-011","STR-013",
    "STR-019",
}

REQUIRED_TEST_KINDS = {
    "positive","negative","property","tamper","differential","exact_limit",
    "one_over","rollback","structural","tier2",
}


class ValidationError(RuntimeError):
    pass


def fail(message: str) -> None:
    raise ValidationError(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load_json(root: Path, rel: str):
    try:
        return json.loads((root / rel).read_text(encoding="utf-8"))
    except Exception as exc:
        fail(f"{rel}: invalid JSON: {exc}")


def safe_zip_root(path: Path, temp: Path) -> Path:
    with zipfile.ZipFile(path) as zf:
        infos = zf.infolist()
        require(0 < len(infos) <= 256, "ZIP file count outside safe package bound")
        names: set[str] = set()
        roots: set[str] = set()
        total = 0
        for info in infos:
            name = info.filename
            require(name not in names, f"duplicate ZIP path: {name}")
            names.add(name)
            require(not (info.flag_bits & 0x1), f"encrypted ZIP entry: {name}")
            pure = PurePosixPath(name)
            require(not pure.is_absolute(), f"absolute ZIP path: {name}")
            require(".." not in pure.parts, f"traversal ZIP path: {name}")
            require("\\" not in name, f"backslash ZIP path: {name}")
            require(pure.parts, f"empty ZIP path: {name}")
            roots.add(pure.parts[0])
            mode = (info.external_attr >> 16) & 0xFFFF
            require(not stat.S_ISLNK(mode), f"symlink ZIP entry: {name}")
            require(info.file_size <= 8 * 1024 * 1024, f"oversized ZIP file: {name}")
            total += info.file_size
            require(total <= 32 * 1024 * 1024, "ZIP uncompressed size exceeds bound")
        require(roots == {PACKAGE}, f"ZIP must contain exactly root {PACKAGE}, got {sorted(roots)}")
        zf.extractall(temp)
    root = temp / PACKAGE
    require(root.is_dir(), "ZIP package root missing")
    return root


def resolve_root(path: Path, temp_stack: list[tempfile.TemporaryDirectory]) -> Path:
    if path.is_file():
        require(zipfile.is_zipfile(path), "input file is not a ZIP")
        td = tempfile.TemporaryDirectory(prefix="arcweft-contract-validate-")
        temp_stack.append(td)
        return safe_zip_root(path, Path(td.name))
    require(path.is_dir(), "input path is neither directory nor ZIP")
    if path.name == PACKAGE:
        return path
    candidate = path / PACKAGE
    require(candidate.is_dir(), f"directory must be package root or contain {PACKAGE}")
    return candidate


def package_files(root: Path) -> set[str]:
    result = set()
    for path in root.rglob("*"):
        require(not path.is_symlink(), f"symlink in extracted package: {path}")
        if path.is_file():
            result.add(path.relative_to(root).as_posix())
    return result


def validate_manifest(root: Path, files: set[str]) -> None:
    manifest = load_json(root, "MANIFEST.json")
    require(manifest.get("schema_version") == 1, "manifest schema version must be 1")
    require(manifest.get("package") == PACKAGE, "manifest package mismatch")
    require(manifest.get("inspected_sha") == INSPECTED_SHA, "manifest inspected SHA mismatch")
    rows = manifest.get("payloads")
    require(isinstance(rows, list), "manifest payloads missing")
    paths = [row.get("path") for row in rows]
    require(paths == sorted(paths), "manifest payloads not sorted")
    require(len(paths) == len(set(paths)), "manifest duplicate payload")
    expected = files - {"MANIFEST.json", "MANIFEST.sha256"}
    require(set(paths) == expected, "manifest does not cover exactly every nonmanifest payload")
    for row in rows:
        rel = row["path"]
        p = root / rel
        require(row.get("bytes") == p.stat().st_size, f"manifest byte mismatch: {rel}")
        require(row.get("sha256") == sha256_file(p), f"manifest SHA mismatch: {rel}")
    manifest_digest = sha256_file(root / "MANIFEST.json")
    recorded = (root / "MANIFEST.sha256").read_text(encoding="ascii").strip()
    require(re.fullmatch(r"[0-9a-f]{64}", recorded) is not None, "MANIFEST.sha256 format")
    require(recorded == manifest_digest, "MANIFEST.sha256 mismatch")


def validate_contract(root: Path) -> None:
    c = load_json(root, "machine/contract.json")
    require(c.get("schema_version") == 1, "contract schema version")
    require(c.get("package") == PACKAGE, "contract package")
    require(c.get("status") == "READY_FOR_IMPLEMENTATION_DESIGN_ONLY", "contract status")
    require(c.get("open_questions") == 0, "contract open questions")
    require(c.get("repository") == "Sanzentyo/arcweft", "contract repository")
    require(c.get("inspected_sha") == INSPECTED_SHA, "contract inspected SHA")
    require(c.get("arcweft_version_markers") == [1], "all Arcweft version markers must be exactly 1")
    require(c.get("numeric_awbc_allocation_emitted") is False, "numeric AWBC table emitted")
    require(c.get("production_modified") is False, "package claims production modification")
    require(c.get("identity_domains") == EXPECTED_DOMAINS, "identity domains differ")
    require(c.get("fixed_identity_zero_invalid") == [
        "NeedProducerInstanceKey","NeedId","TaskKey","TaskId"
    ], "fixed zero-invalid set")
    require(c.get("zero_valid") == ["GenerationId(0)","TaskLaunchOrdinal::JOIN(0)"], "zero-valid set")
    require(c.get("sole_runtime_value_digest") == "arcweft_core::entry::RuntimeValueDigest",
            "duplicate RuntimeValueDigest authority")
    require(c.get("empty_arguments") == "digest(RuntimeValue::Tuple([]))", "empty argument rule")
    require(c.get("runtime_value_need_handle_tag") == 20, "NeedHandle canonical tag")
    require(c.get("current_view_owners") == {
        "program": "ViewProgramId",
        "revision": "AcceptedViewProgramRevision([u8;32])",
    }, "current View owner mismatch")
    require(c.get("ownership_context") == ["ProjectSymbolTable","RegisteredSemanticWorld"],
            "ownership context mismatch")
    require(c.get("cuts") == [
        "generic_match","ownership_evidence","view_admission",
        "private_identity_preparation","atomic_task_need_carrier",
    ], "compile-clean cuts mismatch")
    require(c.get("cut5_indivisible") is True, "Cut 5 must be indivisible")
    flags = c.get("architecture_flags")
    require(isinstance(flags, dict) and flags, "architecture flags missing")
    for name, admitted in flags.items():
        require(admitted is False, f"forbidden architecture flag admitted: {name}")
    minima = c.get("validation_minima")
    require(isinstance(minima, dict), "validation minima missing")


def validate_inputs(root: Path) -> None:
    hashes = load_json(root, "machine/request_hashes.json")
    cur = hashes.get("current_request", {})
    rust = hashes.get("rust_skill", {})
    premise = hashes.get("project_premise", {})
    require(cur.get("sha256") == REQUEST_SHA256, "request hash model stale")
    require(cur.get("git_blob_sha1") == REQUEST_GIT_BLOB, "request Git blob stale")
    require(cur.get("bytes") == 26729, "request byte count")
    require(sha256_file(root / cur["path"]) == REQUEST_SHA256, "request copy SHA mismatch")
    require((root / cur["path"]).stat().st_size == 26729, "request copy size mismatch")
    require(rust.get("sha256") == RUST_SHA256 and rust.get("read_full") is True,
            "Rust skill identity/read-full evidence")
    require(sha256_file(root / rust["path"]) == RUST_SHA256, "Rust skill copy SHA mismatch")
    require(premise.get("sha256") == PREMISE_SHA256 and premise.get("read_full") is True,
            "project premise identity/read-full evidence")
    require(sha256_file(root / premise["path"]) == PREMISE_SHA256, "project premise copy SHA mismatch")


def validate_decisions(root: Path) -> None:
    model = load_json(root, "machine/decisions.json")
    rows = model.get("decisions")
    require(isinstance(rows, list) and len(rows) >= 50, "insufficient decision rows")
    ids = [r.get("id") for r in rows]
    require(len(ids) == len(set(ids)), "duplicate decision ID")
    for row in rows:
        require(row.get("status") == "CLOSED", f"unresolved decision: {row.get('id')}")
        require(
            isinstance(row.get("owner"), str) and len(row["owner"].strip()) >= 4,
            f"vague decision {row.get('id')} field owner",
        )
        for key in ("selected", "rejected"):
            require(isinstance(row.get(key), str) and len(row[key].strip()) >= 12,
                    f"vague decision {row.get('id')} field {key}")


def validate_traceability(root: Path) -> None:
    rows = load_json(root, "machine/traceability.json").get("rows")
    require(isinstance(rows, list) and len(rows) >= 50, "traceability rows missing")
    ids = [r.get("requirement_id") for r in rows]
    require(len(ids) == len(set(ids)), "duplicate traceability ID")
    for required in MANDATORY_TRACE:
        require(ids.count(required) == 1, f"mandatory traceability missing/duplicate: {required}")
    for row in rows:
        require(row.get("status") == "CLOSED", f"traceability not closed: {row.get('requirement_id')}")
        require(row.get("artifacts"), f"traceability artifacts missing: {row.get('requirement_id')}")
        require(row.get("tests"), f"traceability tests missing: {row.get('requirement_id')}")


def validate_policy_and_families(root: Path) -> None:
    policy = load_json(root, "machine/task_policy.json").get("rows")
    require(isinstance(policy, list) and len(policy) == 2, "policy rows")
    by_name = {r.get("policy"): r for r in policy}
    require(set(by_name) == {"JoinSameKey","AlwaysStart"}, "policy names")
    join = by_name["JoinSameKey"]
    always = by_name["AlwaysStart"]
    require(join.get("tag") == 0 and join.get("first_ordinal") == 0,
            "Join policy/ordinal conflation")
    require(join.get("reusable_prelaunch_handle") is True, "Join handle policy")
    require(always.get("tag") == 1 and always.get("first_ordinal") == 1,
            "Always policy/ordinal conflation")
    require(always.get("reusable_prelaunch_handle") is False, "Always reusable handle forbidden")
    require("ordinal" not in str(always.get("same_generation_instance_task_key","")).lower(),
            "TaskKey description contains launch ordinal")

    families = load_json(root, "machine/producer_families.json").get("rows")
    require(isinstance(families, list) and len(families) == 9, "producer family count")
    require([r.get("tag") for r in families] == list(range(9)), "producer family tags")
    require([r.get("name") for r in families] == EXPECTED_FAMILIES, "producer family names")


def validate_ownership(root: Path) -> None:
    rows = load_json(root, "machine/ownership_matrix.json").get("rows")
    require(isinstance(rows, list) and len(rows) >= 80, "ownership matrix too small")
    names = [r.get("variant") for r in rows]
    require(len(names) == len(set(names)), "duplicate TypeKind ownership row")
    require(set(names) == EXPECTED_TYPEKINDS,
            f"TypeKind ownership set mismatch: missing={sorted(EXPECTED_TYPEKINDS-set(names))}, extra={sorted(set(names)-EXPECTED_TYPEKINDS)}")
    by = {r["variant"]: r for r in rows}
    require(by["Need"]["disposition"] == "SnapshotClone", "Need ownership row")
    require(by["Ref"]["disposition"] == "SnapshotClone", "Ref ownership row")
    require(by["ViewValue"]["rejection"] == "MissingViewPersistenceEvidence", "ViewValue row")
    require(by["Function"]["disposition"] == "Reject at type level", "Function type row")
    require(by["Shared"]["disposition"] == "SnapshotClone", "Shared row")
    require(by["AgentResource"]["disposition"] == "SnapshotClone", "AgentResource row")
    require(by["AgentResourceBody"]["disposition"] == "SnapshotClone", "AgentResourceBody row")
    require(by["Stream"]["disposition"] == "Reject", "Stream row")


def validate_tests(root: Path) -> None:
    model = load_json(root, "machine/tests.json")
    rows = model.get("tests")
    require(isinstance(rows, list) and len(rows) >= 180, "insufficient test rows")
    require(model.get("count") == len(rows), "test count mismatch")
    ids = [r.get("id") for r in rows]
    require(len(ids) == len(set(ids)), "duplicate test IDs")
    require(REQUIRED_TEST_IDS <= set(ids), f"required test rows missing: {sorted(REQUIRED_TEST_IDS-set(ids))}")
    kinds = {r.get("kind") for r in rows}
    require(REQUIRED_TEST_KINDS <= kinds, f"required test kinds missing: {sorted(REQUIRED_TEST_KINDS-kinds)}")
    for index, family in enumerate(EXPECTED_FAMILIES, 1):
        require(f"ID-FAM-{index:02d}A" in ids and f"ID-FAM-{index:02d}B" in ids,
                f"family equality/difference tests missing: {family}")
    for row in rows:
        for key in ("category","kind","owner","input","expected","gate"):
            require(isinstance(row.get(key), str) and row[key].strip(),
                    f"vague test {row.get('id')} field {key}")


def validate_source_evidence(root: Path) -> None:
    model = load_json(root, "machine/source_evidence.json")
    require(model.get("repository") == "Sanzentyo/arcweft", "evidence repository")
    require(model.get("inspected_sha") == INSPECTED_SHA, "evidence SHA")
    rows = model.get("rows")
    require(isinstance(rows, list) and len(rows) >= 15, "insufficient source evidence")
    for row in rows:
        require(isinstance(row.get("path"), str) and "/" in row["path"],
                "vague evidence path")
        require(re.fullmatch(r"\d+-\d+", str(row.get("lines",""))) is not None,
                f"vague evidence range: {row.get('path')}")
        require(re.fullmatch(r"[0-9a-f]{40}", str(row.get("blob",""))) is not None,
                f"vague evidence blob: {row.get('path')}")
        require(len(str(row.get("observation","")).strip()) >= 40,
                f"vague evidence observation: {row.get('path')}")
        require(len(str(row.get("verification","")).strip()) >= 8,
                f"vague evidence verification: {row.get('path')}")


def validate_deletions(root: Path) -> None:
    rows = load_json(root, "machine/deletion_matrix.json").get("rows")
    require(isinstance(rows, list) and len(rows) >= 35, "deletion matrix incomplete")
    text = "\n".join(str(r) for r in rows)
    for token in (
        "String-backed NeedId","AwbcTaskPlan.need_id","plan_digest",
        "direct-Await surrogate","indexed NeedId suffix generation","old save/replay String readers",
        "generic Match ownership gate","ResourceTypeRegistry","ViewProgramSemanticDigest",
    ):
        require(token in text, f"deletion matrix missing: {token}")


def validate_human_contract(root: Path) -> None:
    docs = {
        rel: (root / rel).read_text(encoding="utf-8")
        for rel in (
            "README.md","FINAL_CONTRACT.md","RUST_SCHEMAS.md",
            "IDENTITY_AND_DIGESTS.md","TASK_LIFECYCLE_AND_PERSISTENCE.md",
            "CHECKED_MATCH_AND_VIEW_ADMISSION.md","OWNERSHIP_EVIDENCE.md",
            "COMPILE_CLEAN_SEQUENCE.md","SOURCE_EVIDENCE.md",
        )
    }
    require(INSPECTED_SHA in docs["README.md"], "README full SHA missing")
    require("READY_FOR_IMPLEMENTATION — DESIGN ONLY — OPEN_QUESTIONS=0" in docs["README.md"],
            "README final status")
    for domain in EXPECTED_DOMAINS.values():
        require(domain.replace("\0", "\\0") in docs["IDENTITY_AND_DIGESTS.md"],
                f"human digest domain missing: {domain!r}")
    rust = docs["RUST_SCHEMAS.md"]
    for field in (
        "pub generation: GenerationId",
        "pub producer: NeedProducerInstanceKey",
        "pub producer_contract: NeedProducerContractDigest",
        "pub need: NeedId",
        "pub task_key: TaskKey",
        "pub task_id: TaskId",
        "pub launch_ordinal: TaskLaunchOrdinal",
    ):
        require(field in rust, f"incomplete TaskCorrelation schema: {field}")
    require("pub struct TaskEvent" in rust and "pub correlation: TaskCorrelation" in rust
            and "pub cursor: TaskPublicationCursor" in rust, "incomplete event schema")
    require("TaskSpec" in rust and "pub id:" not in rust, "caller ID leaked into TaskSpec schema")
    require("This cut is intentionally indivisible" in docs["COMPILE_CLEAN_SEQUENCE.md"],
            "Cut 5 indivisibility statement")
    require("`ResourceTypeRegistry` is absent from this API." in rust, "ownership context statement")
    require(
        "ConstantFalse" in docs["CHECKED_MATCH_AND_VIEW_ADMISSION.md"]
        and "CheckedExpressionResolution::Literal(HirLiteral::Boolean(true))"
            in docs["CHECKED_MATCH_AND_VIEW_ADMISSION.md"]
        and "No source interpreter" in docs["CHECKED_MATCH_AND_VIEW_ADMISSION.md"],
        "guard contract missing",
    )
    require("No stronger binary verification is claimed" in docs["SOURCE_EVIDENCE.md"],
            "evidence honesty statement missing")


def validate_csvs(root: Path) -> None:
    for rel in (
        "tables/task_policy_truth_table.csv",
        "tables/ownership_matrix.csv",
        "tables/tests.csv",
        "tables/deletion_matrix.csv",
    ):
        with (root / rel).open("r", encoding="utf-8", newline="") as f:
            rows = list(csv.reader(f))
        require(len(rows) >= 2, f"CSV empty: {rel}")


def validate_root(root: Path, *, skip_manifest: bool = False) -> list[str]:
    files = package_files(root)
    missing = REQUIRED_FILES - files
    require(not missing, f"required files missing: {sorted(missing)}")
    require((root / "OPEN_QUESTIONS.md").read_bytes() == b"none",
            "OPEN_QUESTIONS.md must be exact four-byte 'none'")
    validate_contract(root)
    validate_inputs(root)
    validate_decisions(root)
    validate_traceability(root)
    validate_policy_and_families(root)
    validate_ownership(root)
    validate_tests(root)
    validate_source_evidence(root)
    validate_deletions(root)
    validate_human_contract(root)
    validate_csvs(root)
    if not skip_manifest:
        validate_manifest(root, files)
    return [
        f"package={PACKAGE}",
        f"inspected_sha={INSPECTED_SHA}",
        f"files={len(files)}",
        "open_questions=0",
        "schema_version=1",
        "status=PASS",
    ]


def expect_failure(path: Path, mutate, expected_label: str) -> None:
    with tempfile.TemporaryDirectory(prefix="arcweft-validator-negative-") as td:
        target = Path(td) / PACKAGE
        shutil.copytree(path, target)
        mutate(target)
        try:
            validate_root(target, skip_manifest=True)
        except ValidationError:
            return
        fail(f"negative self-test did not fail: {expected_label}")


def run_self_tests(root: Path) -> int:
    cases = []

    cases.append(("open questions", lambda r: (r/"OPEN_QUESTIONS.md").write_bytes(b"later")))
    cases.append(("stale request", lambda r: (r/"inputs/CURRENT_REQUEST.md").write_bytes(
        (r/"inputs/CURRENT_REQUEST.md").read_bytes() + b"\n")))
    cases.append(("version marker", lambda r: _mutate_json(r/"machine/contract.json",
        lambda d: d.__setitem__("arcweft_version_markers", [2]))))
    cases.append(("cut5 divisible", lambda r: _mutate_json(r/"machine/contract.json",
        lambda d: d.__setitem__("cut5_indivisible", False))))
    cases.append(("View owner mismatch", lambda r: _mutate_json(r/"machine/contract.json",
        lambda d: d["current_view_owners"].__setitem__("revision", "u32"))))
    cases.append(("generic/View conflation", lambda r: _mutate_json(r/"machine/contract.json",
        lambda d: d["architecture_flags"].__setitem__("generic_view_admission_conflation", True))))
    cases.append(("compatibility reader", lambda r: _mutate_json(r/"machine/contract.json",
        lambda d: d["architecture_flags"].__setitem__("compatibility_reader", True))))
    cases.append(("unresolved decision", lambda r: _mutate_json(r/"machine/decisions.json",
        lambda d: d["decisions"][0].__setitem__("status", "OPEN"))))
    cases.append(("missing traceability", lambda r: _mutate_json(r/"machine/traceability.json",
        lambda d: d.__setitem__("rows", [x for x in d["rows"] if x["requirement_id"] != "A1"]))))
    cases.append(("policy conflation", lambda r: _mutate_json(r/"machine/task_policy.json",
        lambda d: d["rows"][1].__setitem__("first_ordinal", 0))))
    cases.append(("vague evidence", lambda r: _mutate_json(r/"machine/source_evidence.json",
        lambda d: d["rows"][0].__setitem__("lines", "somewhere"))))
    cases.append(("incomplete event flag", lambda r: _mutate_json(r/"machine/contract.json",
        lambda d: d["architecture_flags"].__setitem__("incomplete_event_schema", True))))
    cases.append(("unconstructible evidence flag", lambda r: _mutate_json(r/"machine/contract.json",
        lambda d: d["architecture_flags"].__setitem__("unconstructible_opaque_evidence", True))))
    cases.append(("delayed persistence", lambda r: _mutate_json(r/"machine/contract.json",
        lambda d: d["architecture_flags"].__setitem__("delayed_persistence_switch", True))))

    for label, mutate in cases:
        expect_failure(root, mutate, label)

    # Manifest mismatch is checked with the normal manifest path.
    with tempfile.TemporaryDirectory(prefix="arcweft-validator-manifest-") as td:
        target = Path(td) / PACKAGE
        shutil.copytree(root, target)
        with (target/"README.md").open("ab") as f:
            f.write(b"\nchanged")
        try:
            validate_root(target, skip_manifest=False)
        except ValidationError:
            pass
        else:
            fail("negative self-test did not fail: manifest mismatch")

    # Unsafe ZIP shape.
    with tempfile.TemporaryDirectory(prefix="arcweft-validator-unsafe-") as td:
        bad = Path(td) / "bad.zip"
        with zipfile.ZipFile(bad, "w") as zf:
            zf.writestr(f"{PACKAGE}/README.md", "x")
            zf.writestr("../escape", "x")
        tds: list[tempfile.TemporaryDirectory] = []
        try:
            resolve_root(bad, tds)
        except ValidationError:
            pass
        else:
            fail("negative self-test did not fail: unsafe ZIP")
        finally:
            for item in tds:
                item.cleanup()

    return len(cases) + 2


def _mutate_json(path: Path, mutator) -> None:
    data = json.loads(path.read_text(encoding="utf-8"))
    mutator(data)
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", nargs="?", default=".")
    parser.add_argument("--self-test", action="store_true",
                        help="also run destructive negative tests in temporary copies")
    args = parser.parse_args()
    temp_stack: list[tempfile.TemporaryDirectory] = []
    try:
        root = resolve_root(Path(args.path).resolve(), temp_stack)
        lines = validate_root(root)
        for line in lines:
            print(line)
        if args.self_test:
            count = run_self_tests(root)
            print(f"negative_self_tests={count}")
        return 0
    except (ValidationError, OSError, zipfile.BadZipFile) as exc:
        print(f"status=FAIL: {exc}", file=sys.stderr)
        return 1
    finally:
        for td in temp_stack:
            td.cleanup()


if __name__ == "__main__":
    raise SystemExit(main())
