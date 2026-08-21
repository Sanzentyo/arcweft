from __future__ import annotations

import csv
import hashlib
import json
import os
import re
import stat
import sys
import tempfile
import zipfile
from pathlib import Path, PurePosixPath

STEM = 'arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract'
BASELINE = 'c49099fb154d9e3dbb587e1bcd7ee243214da0c4'
REQUEST_NAME = '2026-08-21-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction.md'
REQUEST_SHA256 = '8bf22dbee57a94ee178e25d0004be7a18694a8b801ef79189da3f9e1a3741299'
REQUEST_BYTES = 27962
REQUEST_BLOB = 'a1411adcf7f2c9651f250d9db3302d3ab61ddfa7'
EXPECTED_OPCODES = [(0, 'Nop', 'instruction', 'implemented'), (1, 'LoadConst', 'instruction', 'implemented'), (2, 'Move', 'instruction', 'implemented'), (3, 'Clear', 'instruction', 'implemented'), (4, 'EnterScope', 'instruction', 'implemented'), (5, 'ExitScope', 'instruction', 'implemented'), (6, 'BindPattern', 'instruction', 'implemented'), (7, 'TestPattern', 'instruction', 'implemented'), (8, 'MakeTuple', 'instruction', 'implemented'), (9, 'MakeSequence', 'instruction', 'implemented'), (10, 'RepeatSequence', 'instruction', 'implemented'), (11, 'SequenceLen', 'instruction', 'implemented'), (12, 'SequenceGet', 'instruction', 'implemented'), (13, 'SequenceSlice', 'instruction', 'implemented'), (14, 'SequencePush', 'instruction', 'implemented'), (15, 'MakeRecord', 'instruction', 'implemented'), (16, 'MakeVariant', 'instruction', 'implemented'), (17, 'ProjectTuple', 'instruction', 'implemented'), (18, 'ProjectRecord', 'instruction', 'implemented'), (19, 'ProjectField', 'instruction', 'implemented'), (20, 'Unary', 'instruction', 'implemented'), (21, 'Binary', 'instruction', 'implemented'), (22, 'CallPureHelper', 'instruction', 'implemented'), (23, 'CallIntrinsic', 'instruction', 'implemented'), (24, 'EnsureContent', 'instruction', 'implemented'), (25, 'EmitEffect', 'instruction', 'implemented'), (26, 'StartTask', 'instruction', 'implemented'), (27, 'SpawnFiber', 'instruction', 'implemented'), (28, 'StreamYield', 'instruction', 'implemented'), (29, 'StreamClose', 'instruction', 'implemented'), (30, 'NeedTimeout', 'instruction', 'allocated_pending_feature_cut'), (31, 'Drop', 'instruction', 'implemented'), (32, 'CommitDialogueResult', 'instruction', 'allocated_pending_feature_cut'), (33, 'AssignRecordField', 'instruction', 'implemented'), (34, 'CallTraitMethod', 'instruction', 'implemented'), (35, 'RegisterCleanup', 'instruction', 'implemented'), (36, 'CancelCleanup', 'instruction', 'implemented'), (37, 'MakeFunction', 'instruction', 'implemented'), (38, 'ApplyFunction', 'instruction', 'implemented'), (39, 'MakeAgent', 'instruction', 'implemented'), (40, 'MakeReductionUnchanged', 'instruction', 'implemented'), (41, 'MakeNeedHandle', 'instruction', 'allocated_pending_feature_cut'), (42, 'CopyValue', 'instruction', 'allocated_pending_feature_cut'), (43, 'ExecuteLineOperation', 'instruction', 'allocated_pending_feature_cut'), (44, 'OpenStream', 'instruction', 'allocated_pending_feature_cut'), (45, 'FinishStream', 'instruction', 'allocated_pending_feature_cut'), (46, 'ApplyExternalStreamGroup', 'instruction', 'allocated_pending_feature_cut'), (128, 'Jump', 'terminator', 'implemented'), (129, 'Branch', 'terminator', 'implemented'), (130, 'Match', 'terminator', 'implemented'), (131, 'CallFunction', 'terminator', 'implemented'), (132, 'GotoStatic', 'terminator', 'implemented'), (133, 'GotoDynamic', 'terminator', 'implemented'), (134, 'Dialogue', 'terminator', 'implemented'), (135, 'Choice', 'terminator', 'implemented'), (136, 'Await', 'terminator', 'implemented'), (137, 'AwaitMany', 'terminator', 'implemented'), (138, 'HostCall', 'terminator', 'implemented'), (139, 'Return', 'terminator', 'implemented'), (140, 'Trap', 'terminator', 'implemented'), (141, 'BudgetYield', 'terminator', 'implemented'), (142, 'Unreachable', 'terminator', 'implemented'), (143, 'NextStream', 'terminator', 'allocated_pending_feature_cut'), (144, 'YieldStream', 'terminator', 'allocated_pending_feature_cut')]
EXPECTED_KINDS = [(0, 'Flow'), (1, 'PureHelper'), (2, 'TraitMethod'), (3, 'StreamTransform'), (6, 'LineTask'), (7, 'Synthetic'), (8, 'Ordinary'), (9, 'GeneratorProducer'), (10, 'LineActivation')]
EXPECTED_FLAGS = [(0, 'MaySuspend', 1), (1, 'MayAllocate', 2), (2, 'Deterministic', 4), (3, 'HasDynamicTarget', 8), (4, 'OwnsStreamProducer', 16), (5, 'NeedProducer', 32)]
EXPECTED_TYPE_KINDS = ['Bool', 'I8', 'I16', 'I32', 'I64', 'I128', 'ISize', 'U8', 'U16', 'U32', 'U64', 'U128', 'USize', 'F32', 'F64', 'String', 'Char', 'Bytes', 'TextCluster', 'Duration', 'Progress', 'StageApi', 'LineContext', 'StageActorHandle', 'CueHandle', 'VoiceHandle', 'Range', 'IteratorState', 'DisplayText', 'DebugStatePath', 'ObservationFieldPath', 'Ref', 'Probe', 'Predicate', 'Observation', 'ObservedObject', 'AgentBBox', 'ActionName', 'ActionTarget', 'ActionResult', 'AgentValue', 'DataFormat', 'DataShape', 'AgentEntityMetadata', 'AgentSourceAnchor', 'AgentProjectGraphNeighborhood', 'AgentProjectGraphSymbol', 'AgentProjectGraphEdge', 'CaptureTarget', 'CaptureRef', 'AgentResource', 'AgentResourceBody', 'RagContextPack', 'AgentBuiltin', 'Vec', 'Array', 'Slice', 'Seq', 'Map', 'BorrowRef', 'Need', 'Stream', 'Result', 'Option', 'Handle', 'ThreadHandle', 'Shared', 'Function', 'GenericParam', 'ProjectNominal', 'AcceptedNominal', 'OpenNominal', 'Error', 'Projection', 'CharacterPatch', 'FocusPatch', 'CharacterDialogue', 'DialogueLine', 'ViewValue', 'CharacterNominal', 'Named', 'Tuple', 'Choice', 'Unit', 'Never']
EXPECTED_COVERAGE = ['discard', 'binding', 'whole_binding', 'typed_binding', 'literal', 'entity_reference', 'closed_variant', 'tuple', 'record', 'array', 'vec_slice_seq_exact', 'vec_slice_seq_rest', 'or', 'open_opaque_future', 'poisoned']
REQUIRED_TEST_IDS = ['AWBC-001', 'AWBC-002', 'AWBC-003', 'AWBC-004', 'AWBC-005', 'VAR-0', 'VAR-1', 'VAR-127', 'VAR-128', 'VAR-4294967295', 'VAR-NC-1', 'VAR-OV-1', 'VAR-OV-2', 'VAR-TR-1', 'TENSOR-001', 'WIRE-001', 'WIRE-002', 'WIRE-003', 'WIRE-004', 'WIRE-005', 'FLAG-001', 'FLAG-002', 'FLAG-003', 'FLAG-004', 'FLAG-005', 'KIND-001', 'KIND-002', 'PARITY-001', 'VERSION-001', 'NEED-005', 'NEED-006', 'NEED-009', 'NEED-011', 'NEED-012', 'NEED-016', 'NEED-017', 'COV-001', 'COV-004', 'COV-005', 'COV-006', 'COV-007', 'COV-008', 'COV-011', 'COV-013', 'COV-014', 'COV-015', 'COV-021', 'COV-022', 'OWN-001', 'OWN-002', 'OWN-004', 'OWN-006', 'OWN-007', 'DIG-001', 'DIG-004', 'BUNDLE-001', 'SAVE-001', 'REPL-001', 'STRUCT-001', 'STRUCT-002', 'STRUCT-003', 'STRUCT-004']


class ValidationFailure(Exception):
    pass


def fail(message: str) -> None:
    raise ValidationFailure(message)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def git_blob(data: bytes) -> str:
    return hashlib.sha1(f"blob {len(data)}\0".encode("ascii") + data).hexdigest()


def load_json(root: Path, name: str):
    try:
        return json.loads((root / name).read_text(encoding="utf-8"))
    except Exception as error:
        fail(f"cannot load {name}: {error}")


def load_csv(root: Path, name: str):
    try:
        with (root / name).open("r", encoding="utf-8", newline="") as handle:
            return list(csv.DictReader(handle))
    except Exception as error:
        fail(f"cannot load {name}: {error}")


def validate_zip_shape(path: Path) -> None:
    with zipfile.ZipFile(path) as archive:
        names = [info.filename for info in archive.infolist()]
        if len(names) != len(set(names)):
            fail("ZIP contains duplicate entry names")
        roots = set()
        for info in archive.infolist():
            pure = PurePosixPath(info.filename)
            if pure.is_absolute() or ".." in pure.parts or not pure.parts:
                fail(f"unsafe ZIP path: {info.filename}")
            roots.add(pure.parts[0])
            unix_mode = (info.external_attr >> 16) & 0xFFFF
            if stat.S_ISLNK(unix_mode):
                fail(f"ZIP symlink is forbidden: {info.filename}")
        if roots != {STEM}:
            fail(f"ZIP must contain exactly root {STEM}, found {sorted(roots)}")
        if not any(name == STEM + "/README.md" for name in names):
            fail("ZIP root lacks README.md")
        bad = archive.testzip()
        if bad is not None:
            fail(f"ZIP CRC failure: {bad}")


def locate_root(argument: Path, temporary: tempfile.TemporaryDirectory[str] | None):
    if argument.is_dir():
        root = argument
        if root.name != STEM and (root / STEM).is_dir():
            root = root / STEM
        return root, temporary
    if argument.is_file() and argument.suffix.lower() == ".zip":
        validate_zip_shape(argument)
        temporary = tempfile.TemporaryDirectory(prefix="arcweft-contract-")
        with zipfile.ZipFile(argument) as archive:
            archive.extractall(temporary.name)
        return Path(temporary.name) / STEM, temporary
    fail(f"expected package directory or ZIP: {argument}")


def validate_required_files(root: Path) -> None:
    required = {
        "README.md", "FINAL_CONTRACT.md", "DECISION_REGISTER.md", "RUST_SCHEMAS.md",
        "OWNER_API_MAP.md", "DEPENDENCY_GRAPH.md", "AWBC_ALLOCATION_AND_WIRE.md",
        "AWBC_OPCODE_TABLE.csv", "NEED_TASK_IDENTITY.md", "NEED_IDENTITY_TABLE.csv",
        "NEED_CONSUMER_DELETION_MATRIX.md", "NEED_CONSUMER_DELETION_MATRIX.csv",
        "CHECKED_MATCH_COVERAGE.md", "COVERAGE_PATTERN_MATRIX.csv",
        "OWNERSHIP_AND_PERSISTENCE.md", "OWNERSHIP_TYPE_TABLE.csv",
        "CHECKED_MATCH_DIGEST.md", "PERSISTENCE_REPLAY_REPLACEMENT.md",
        "FAILURE_PRECEDENCE_AND_ATOMICITY.md", "COMPILE_CLEAN_SEQUENCE.md",
        "SOURCE_EVIDENCE.md", "SOURCE_EVIDENCE.csv", "REQUIREMENT_TRACEABILITY.md",
        "REQUIREMENT_TRACEABILITY.csv", "TEST_MATRIX.md", "TEST_MATRIX.csv",
        "STRUCTURAL_ABSENCE.md", "VERIFICATION_SCOPE.md", "VALIDATION.md",
        "VALIDATION.json", "VALIDATION_OUTPUT.txt", "OPEN_QUESTIONS.md",
        "MANIFEST.json", "MANIFEST.sha256", "SHA256SUMS",
        "inputs/CURRENT_REQUEST.md", "tools/validate_package.py",
        "machine/awbc-allocation.json", "machine/function-kinds.json",
        "machine/function-flags.json", "machine/version-markers.json",
        "machine/wire-primitives.json", "machine/need-identity.json",
        "machine/coverage-pattern-families.json", "machine/ownership-disposition.json",
        "machine/digest-grammar.json", "machine/request-chain.json",
        "machine/structural-absence.json",
    }
    actual = {str(path.relative_to(root)).replace(os.sep, "/") for path in root.rglob("*") if path.is_file()}
    missing = sorted(required - actual)
    if missing:
        fail(f"missing required files: {missing}")


def validate_open_questions(root: Path) -> None:
    if (root / "OPEN_QUESTIONS.md").read_bytes() != b"none\n":
        fail("OPEN_QUESTIONS.md must contain exactly 'none\\n'")


def validate_request(root: Path) -> None:
    data = (root / "inputs/CURRENT_REQUEST.md").read_bytes()
    if len(data) != REQUEST_BYTES or digest(data) != REQUEST_SHA256 or git_blob(data) != REQUEST_BLOB:
        fail("current request copy hash/blob/length mismatch")
    chain = load_json(root, "machine/request-chain.json")
    if chain.get("baseline") != BASELINE:
        fail("request chain baseline mismatch")
    rows = chain.get("chain", [])
    if [row.get("sequence") for row in rows] != [1, 2, 3, 4]:
        fail("request chain sequence mismatch")
    expected_chain = [
        ("37bbf3d5c799a3b4dbae6ade9fe14bda737aa476", "993802106745fc9adb57829af67b1bb4379a6999286edaa4f110e3039c181304", 7719),
        ("b98cc4a22ed07fa4373d9e9112bf0c005f12d9a3", "fb1b21bf038346c5f742ef1a25ee1bf4b59b1573b489dad054ff051b0b970607", 10995),
        ("6d2acd931c430c5787a05fd4661009266903db69", "097611701bfb13d5a43317b65302fcb209d62b7bce9c47afe28b29ff934285c3", 14785),
        (REQUEST_BLOB, REQUEST_SHA256, REQUEST_BYTES),
    ]
    actual_chain = [
        (row.get("git_blob_sha1"), row.get("expected_sha256"), row.get("expected_bytes"))
        for row in rows
    ]
    if actual_chain != expected_chain:
        fail("exact repository request chain hash/blob/length metadata mismatch")
    archive = chain.get("retained_archive", {})
    if archive.get("expected_sha256") != "96f4f84be1b7b2bbec9d2ba564418f00f453870f4ea331566a1f51258cc1ef8d":
        fail("retained archive hash mismatch")


def validate_allocation(root: Path) -> None:
    data = load_json(root, "machine/awbc-allocation.json")
    rows = data.get("opcodes", [])
    actual = [(row.get("byte"), row.get("name"), row.get("class"), row.get("status")) for row in rows]
    if actual != EXPECTED_OPCODES:
        fail("AWBC opcode allocation differs from exact table")
    bytes_ = [row[0] for row in actual]
    if len(bytes_) != 64 or len(set(bytes_)) != 64:
        fail("AWBC allocation must contain 64 unique bytes")
    if data.get("repr") != "u8" or data.get("decode_table") != {"allocation_free": True, "length": 256, "source": "AwbcOpcode::ALL"}:
        fail("AwbcOpcode authority/decode-table contract mismatch")
    if data.get("reserved") != [
        {"action": "reject", "class": "unknown", "end": 127, "start": 47},
        {"action": "reject", "class": "unknown", "end": 255, "start": 145},
    ]:
        fail("reserved opcode ranges mismatch")
    csv_rows = load_csv(root, "AWBC_OPCODE_TABLE.csv")
    csv_actual = [(int(row["byte"]), row["name"], row["class"], row["status"]) for row in csv_rows]
    if csv_actual != EXPECTED_OPCODES:
        fail("AWBC CSV differs from machine allocation")


def validate_kind_flags_versions(root: Path) -> None:
    kinds = load_json(root, "machine/function-kinds.json")
    actual_kinds = [(row.get("tag"), row.get("name")) for row in kinds.get("rows", [])]
    if actual_kinds != EXPECTED_KINDS or kinds.get("tombstones") != [4, 5]:
        fail("function-kind table/tombstones mismatch")
    flags = load_json(root, "machine/function-flags.json")
    actual_flags = [(row.get("bit"), row.get("name"), row.get("mask")) for row in flags.get("rows", [])]
    if actual_flags != EXPECTED_FLAGS or flags.get("known_mask") != 0x3F:
        fail("function flag table or KNOWN_MASK mismatch")
    constraints = flags.get("constraints", {})
    if not constraints.get("forbid_both_producer_bits"):
        fail("producer bits 4+5 must reject")
    versions = load_json(root, "machine/version-markers.json")
    markers = versions.get("markers", {})
    if not markers or any(value != 1 for value in markers.values()):
        fail("every version marker must equal 1")
    if versions.get("compatibility_readers") != 0:
        fail("compatibility reader count must be zero")


def validate_wire(root: Path) -> None:
    wire = load_json(root, "machine/wire-primitives.json")
    u32 = wire.get("ordinary_u32", {})
    if u32.get("encoding") != "shortest unsigned base-128 varint" or u32.get("maximum_bytes") != 5:
        fail("ordinary u32 varint grammar mismatch")
    if set(u32.get("reject", [])) != {"overflow", "sixth byte", "unterminated", "redundant encoding"}:
        fail("varint rejection set incomplete")
    if wire.get("usize_on_wire") is not False:
        fail("usize must not enter wire")
    if wire.get("tensor_shape") != "Vec<u32>::Wire using ordinary varints on write and read":
        fail("tensor shape symmetry contract mismatch")
    single = wire.get("single_buffer", {})
    if single.get("enabled") is not True or single.get("reserve_envelope") != 20 or "truncate" not in single.get("rollback", ""):
        fail("single-buffer/rollback contract incomplete")
    grammars = wire.get("instruction_grammars", {})
    exact = {
        "MakeNeedHandle": "29 dst:varu32 plan:varu32 site:varu32 argc:varu32 args[argc]:varu32",
        "NeedTimeout": "1e dst:varu32 source:varu32 limit:varu32 producer_site:varu32",
        "ExecuteLineOperation": "2b dst:varu32 operation:varu32 argc:varu32 args[argc]:varu32",
    }
    for name, grammar in exact.items():
        if grammars.get(name) != grammar:
            fail(f"exact instruction grammar mismatch: {name}")


def validate_coverage_ownership(root: Path) -> None:
    coverage = load_json(root, "machine/coverage-pattern-families.json")
    if coverage.get("caller_supplied_coverage") is not False:
        fail("coverage must be non-forgeable")
    actual_families = [row.get("family") for row in coverage.get("families", [])]
    if actual_families != EXPECTED_COVERAGE:
        fail("coverage pattern-family matrix incomplete or reordered")
    limits = coverage.get("limits", {})
    expected_limits = {
        "max_arms": 4096, "max_pattern_nodes": 65536, "max_or_alternatives": 4096,
        "max_matrix_rows": 8192, "max_specializations": 32768,
        "max_sequence_partitions": 2048, "max_witness_nodes": 1024,
        "max_recursion_depth": 64, "max_unreachable_rows": 4096,
    }
    if limits != expected_limits:
        fail("coverage limits mismatch")
    ownership = load_json(root, "machine/ownership-disposition.json")
    actual_types = [row.get("type_kind") for row in ownership.get("type_kinds", [])]
    if actual_types != EXPECTED_TYPE_KINDS or len(actual_types) != len(set(actual_types)):
        fail("TypeKind ownership table is not exact and total")
    if ownership.get("inputs") != ["ProjectSymbolTable", "RegisteredSemanticWorld", "ResourceTypeRegistry", "CheckedOwnershipLimits"]:
        fail("ownership context inputs mismatch")
    if ownership.get("success") != ["Copy", "SnapshotClone"] or ownership.get("rejections_are_errors") is not True:
        fail("ownership disposition contract mismatch")
    csv_types = [row["type_kind"] for row in load_csv(root, "OWNERSHIP_TYPE_TABLE.csv")]
    if csv_types != EXPECTED_TYPE_KINDS:
        fail("ownership CSV differs from machine table")


def validate_identity_digest(root: Path) -> None:
    need = load_json(root, "machine/need-identity.json")
    if need.get("hash") != "BLAKE3-256" or need.get("legacy_string_fallback") is not False:
        fail("Need identity hash/fallback contract mismatch")
    if need.get("zero_value") != "reserved and rejected" or "never parsed" not in need.get("display", ""):
        fail("Need identity zero/display contract mismatch")
    required_domains = {
        "producer_contract", "task_plan", "host_task", "view_producer", "line_task",
        "await_many_base", "await_many_child", "timeout", "task_key", "task_id", "runtime_value",
    }
    if set(need.get("domains", {})) != required_domains:
        fail("Need identity domains incomplete")
    digest_spec = load_json(root, "machine/digest-grammar.json")
    persisted = set(digest_spec.get("persisted_fields", []))
    forbidden = set(digest_spec.get("forbidden_persisted_fields", []))
    if persisted & forbidden:
        fail("persisted digest fields contain HIR/session identity")
    required_forbidden = {"ExprId", "ScopeId", "PatternId", "LocalId", "HirSnapshotId", "usize"}
    if not required_forbidden <= forbidden:
        fail("digest forbidden field set incomplete")


def validate_trace_evidence_tests(root: Path) -> None:
    trace = load_csv(root, "REQUIREMENT_TRACEABILITY.csv")
    decisions = [int(row["decision"]) for row in trace]
    if decisions != list(range(1, 24)):
        fail("Required exact decisions 1..23 are not traced exactly")
    for row in trace:
        if not (root / row["artifact"]).is_file():
            fail(f"trace artifact does not exist: {row['artifact']}")
    evidence = load_csv(root, "SOURCE_EVIDENCE.csv")
    if len(evidence) < 30:
        fail("source evidence is incomplete")
    for row in evidence:
        if row.get("git_sha") != BASELINE:
            fail(f"evidence baseline mismatch: {row.get('id')}")
        try:
            start = int(row["start_line"])
            end = int(row["end_line"])
        except ValueError:
            fail(f"vague evidence range: {row.get('id')}")
        if start < 1 or end < start:
            fail(f"invalid evidence range: {row.get('id')}")
        if re.search(r"\bend\b|1-end", row["start_line"] + row["end_line"], re.I):
            fail(f"vague evidence label: {row.get('id')}")
    deletion = load_csv(root, "NEED_CONSUMER_DELETION_MATRIX.csv")
    if len(deletion) < 35:
        fail("Need/task consumer deletion matrix is incomplete")
    combined = "\n".join(row["legacy"] + " " + row["replacement"] for row in deletion)
    for token in ["AwbcTaskPlan.need_id", "format base.index", "String snapshot", "payloadless/string NeedHandle", "caller-supplied coverage", "duplicate numeric opcode"]:
        if token not in combined:
            fail(f"deletion matrix missing required consumer: {token}")
    tests = load_csv(root, "TEST_MATRIX.csv")
    ids = {row["id"] for row in tests}
    missing = sorted(set(REQUIRED_TEST_IDS) - ids)
    if missing:
        fail(f"test matrix missing required rows: {missing}")
    if len(tests) < 100:
        fail("test matrix must contain at least 100 concrete rows")
    kinds = {row["kind"] for row in tests}
    required_kinds = {"positive", "negative", "tamper", "property", "differential", "exact_limit", "one_over", "rollback", "structural", "fuzz", "golden", "exhaustive", "roundtrip"}
    if not required_kinds <= kinds:
        fail(f"test matrix missing test kinds: {sorted(required_kinds - kinds)}")


def validate_structural_and_text(root: Path) -> None:
    absence = load_json(root, "machine/structural-absence.json")
    forbidden = absence.get("forbidden", [])
    required = [
        "caller-supplied CheckedMatchCoverage", "String NeedId/TaskId/TaskKey", "payloadless NeedHandle",
        "feature-local opcode numeric table", "fixed-le ordinary u32", "usize Wire",
        "legacy compatibility reader", "persisted HIR identity", "RuntimeSemanticFactInput", "AwbcVm",
        "extension trait for Arcweft-owned enum behavior",
    ]
    for item in required:
        if item not in forbidden:
            fail(f"structural absence set missing: {item}")
    for path in root.rglob("*.md"):
        if path.name == "CURRENT_REQUEST.md":
            continue
        text = path.read_text(encoding="utf-8")
        if re.search(r"\b(?:TODO|TBD|FIXME)\b", text):
            fail(f"placeholder token in {path.relative_to(root)}")
    rust = (root / "RUST_SCHEMAS.md").read_text(encoding="utf-8")
    if "RuntimePlanSemanticFactInput" not in rust or "pub fn step(" not in rust or "step_with_host" not in rust:
        fail("constructible runtime-plan/functional VM APIs are missing")
    if "coverage: CheckedMatchCoverage" in rust.split("impl CheckedMatch", 1)[-1].split("## 6", 1)[0]:
        fail("CheckedMatch constructor still accepts coverage")


def validate_manifest(root: Path) -> None:
    manifest_bytes = (root / "MANIFEST.json").read_bytes()
    expected_manifest_hash = (root / "MANIFEST.sha256").read_text(encoding="ascii").strip()
    if digest(manifest_bytes) != expected_manifest_hash:
        fail("MANIFEST.sha256 mismatch")
    manifest = json.loads(manifest_bytes)
    if manifest.get("package") != STEM or manifest.get("baseline") != BASELINE or manifest.get("algorithm") != "sha256" or manifest.get("schema_version") != 1:
        fail("manifest header mismatch")
    rows = manifest.get("files", [])
    row_paths = [row.get("path") for row in rows]
    if row_paths != sorted(row_paths) or len(row_paths) != len(set(row_paths)):
        fail("manifest paths must be sorted and unique")
    excluded = {"MANIFEST.json", "MANIFEST.sha256", "SHA256SUMS"}
    actual_payload = sorted(
        str(path.relative_to(root)).replace(os.sep, "/")
        for path in root.rglob("*") if path.is_file() and str(path.relative_to(root)).replace(os.sep, "/") not in excluded
    )
    if row_paths != actual_payload:
        fail("manifest does not cover exactly every payload file")
    for row in rows:
        data = (root / row["path"]).read_bytes()
        if len(data) != row.get("bytes") or digest(data) != row.get("sha256"):
            fail(f"manifest payload mismatch: {row['path']}")
    sum_rows = {}
    for line in (root / "SHA256SUMS").read_text(encoding="ascii").splitlines():
        parts = line.split("  ", 1)
        if len(parts) != 2:
            fail("malformed SHA256SUMS line")
        sum_rows[parts[1]] = parts[0]
    expected_sum_paths = sorted(
        str(path.relative_to(root)).replace(os.sep, "/")
        for path in root.rglob("*") if path.is_file() and path.name != "SHA256SUMS"
    )
    if sorted(sum_rows) != expected_sum_paths:
        fail("SHA256SUMS coverage mismatch")
    for name, expected in sum_rows.items():
        if digest((root / name).read_bytes()) != expected:
            fail(f"SHA256SUMS mismatch: {name}")


def validate(root: Path) -> None:
    if root.name != STEM:
        fail(f"package root must be named {STEM}")
    validate_required_files(root)
    validate_open_questions(root)
    validate_request(root)
    validate_allocation(root)
    validate_kind_flags_versions(root)
    validate_wire(root)
    validate_coverage_ownership(root)
    validate_identity_digest(root)
    validate_trace_evidence_tests(root)
    validate_structural_and_text(root)
    validate_manifest(root)


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: validate_package.py <package-directory-or-zip>", file=sys.stderr)
        return 2
    temporary = None
    try:
        root, temporary = locate_root(Path(sys.argv[1]).resolve(), temporary)
        validate(root)
        print(f"PASS {root.name}")
        return 0
    except ValidationFailure as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    finally:
        if temporary is not None:
            temporary.cleanup()


if __name__ == "__main__":
    raise SystemExit(main())
