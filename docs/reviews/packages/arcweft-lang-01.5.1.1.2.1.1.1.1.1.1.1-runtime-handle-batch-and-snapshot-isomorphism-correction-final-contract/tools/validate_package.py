#!/usr/bin/env python3
"""Read-only validator for the runtime-handle/batch/snapshot-isomorphism package."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import sys
import zipfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable


PACKAGE_NAME = (
    "arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1-"
    "runtime-handle-batch-and-snapshot-isomorphism-correction-final-contract"
)

REQUIRED_FILES = {
    "README.md",
    "FINAL_STATUS.md",
    "OPEN_QUESTIONS.md",
    "INPUTS_AND_PRECEDENCE.md",
    "VERIFICATION_SCOPE.md",
    "FINAL_CONTRACT.md",
    "DECISION_REGISTER.md",
    "RUST_LIVE_SCHEMAS.md",
    "NEED_HANDLE_AND_AWAIT_MANY.md",
    "BATCH_OBSERVER_AND_CANCEL.md",
    "HOST_ADAPTER_AND_LAYERING.md",
    "SNAPSHOT_ISOMORPHISM.md",
    "PROJECTION_REGISTRY.md",
    "MATCH_ROLE_TAG_CALLABLE.md",
    "OWNERSHIP_PROJECTION_MATRIX.md",
    "EVENT_ORDER_AND_SNAPSHOT_POLICY.md",
    "SOURCE_EVIDENCE.md",
    "SOURCE_DELETION_AND_CUTS.md",
    "TEST_MATRIX.md",
    "REQUIREMENT_TRACEABILITY.md",
    "STRUCTURAL_ABSENCE.md",
    "VALIDATION_GUIDE.md",
    "VALIDATION_OUTPUT.txt",
    "schemas/final_contract.rs",
    "machine/contract.json",
    "machine/adapter_protocol.json",
    "machine/match_roles.json",
    "machine/live_snapshot_inventory.json",
    "machine/ownership_matrix.json",
    "machine/compile_cuts.json",
    "machine/blockers.json",
    "machine/source_inventory.json",
    "machine/test_matrix.json",
    "inputs/REQUEST.md",
    "inputs/RUST_SKILL.txt",
    "inputs/PROJECT_PREMISE.txt",
    "tools/validate_package.py",
    "tools/run_validator.sh",
    "MANIFEST.json",
    "MANIFEST.sha256",
}

EXPECTED_FINAL_RUNTIME_VARIANTS = [
    "Unit", "Bool", "Int", "UInt", "F32", "F64", "MatrixF32", "MatrixF64",
    "TensorF32", "TensorF64", "String", "Char", "Duration", "Progress",
    "Range", "Iterator", "EntityRef", "Tuple", "Seq", "Record",
    "NominalRecord", "Opaque", "Reduction", "Agent", "Function", "Variant",
    "NeedHandle",
]

EXPECTED_DENSE_SEQ_CASES = [
    "Units", "Bools", "I8", "I16", "I32", "I64", "I128", "ISize",
    "U8", "U16", "U32", "U64", "U128", "USize", "F32", "F64",
    "Strings", "Chars", "Durations", "EntityRefs", "Bytes",
]

EXPECTED_EXPR_FAMILIES = [
    "Unit", "Literal", "EntityReference", "LifetimePath", "Path",
    "ShortVariant", "Placeholder", "Tuple", "BracketSequence",
    "NumericBracketSequence", "ArrayRepeat", "Call", "Select", "Index",
    "Pipe", "Try", "Await", "Thread", "Choice", "Range", "Record",
    "RecordLiteral", "Binary", "Borrow", "Dereference", "Closure", "Unary",
    "Block", "ComputationBlock", "NamedBlock", "Loop", "If", "IfLet",
    "Match", "DialogueContentApplication", "PostfixBracket", "Error",
    "ForSynthetic",
]

EXPECTED_PATTERN_FAMILIES = [
    "Binding", "MutableBinding", "Literal", "EntityReference", "Variant",
    "Discard", "Tuple", "Record", "BracketSequence", "WholeBinding", "Or",
    "TypedBinding", "Error",
]

EXPECTED_TYPE_KINDS = [
    'Bool', 'I8', 'I16', 'I32', 'I64', 'I128',
    'ISize', 'U8', 'U16', 'U32', 'U64', 'U128',
    'USize', 'F32', 'F64', 'String', 'Char', 'Bytes',
    'TextCluster', 'Duration', 'Progress', 'StageApi', 'LineContext', 'StageActorHandle',
    'CueHandle', 'VoiceHandle', 'Range', 'IteratorState', 'DisplayText', 'DebugStatePath',
    'ObservationFieldPath', 'Ref', 'Probe', 'Predicate', 'Observation', 'ObservedObject',
    'AgentBBox', 'ActionName', 'ActionTarget', 'ActionResult', 'AgentValue', 'DataFormat',
    'DataShape', 'AgentEntityMetadata', 'AgentSourceAnchor', 'AgentProjectGraphNeighborhood', 'AgentProjectGraphSymbol', 'AgentProjectGraphEdge',
    'CaptureTarget', 'CaptureRef', 'AgentResource', 'AgentResourceBody', 'RagContextPack', 'AgentBuiltin',
    'Vec', 'Array', 'Slice', 'Seq', 'Map', 'BorrowRef',
    'Need', 'Stream', 'Result', 'Option', 'Handle', 'ThreadHandle',
    'Shared', 'Function', 'GenericParam', 'ProjectNominal', 'AcceptedNominal', 'OpenNominal',
    'Error', 'Projection', 'CharacterPatch', 'FocusPatch', 'CharacterDialogue', 'DialogueLine',
    'ViewValue', 'CharacterNominal', 'Named', 'Tuple', 'Choice', 'Unit',
    'Never'
]

EXPECTED_BLOCKER_CODES = [
    "E_HANDLE_REUSABLE_SPEC",
    "E_AWAIT_MANY_EVIDENCE",
    "E_BATCH_PER_CHILD_COMMIT",
    "E_OBSERVER_ALLOCATOR",
    "E_ADAPTER_CANCEL_METHOD",
    "E_SANS_IO_DEPENDENCY",
    "E_SNAPSHOT_LOSSY",
    "E_UNDEFINED_PROJECTION",
    "E_MATCH_CALLABLE_JOIN",
    "E_OWNERSHIP_AMBIGUOUS",
    "E_EVENT_ORDER",
    "E_SNAPSHOT_BLANKET_HOST_REJECT",
]


@dataclass(frozen=True)
class Finding:
    code: str
    message: str

    def render(self) -> str:
        return f"{self.code}: {self.message}"


class PackageReader:
    """Uniform read-only access to a directory or one-root ZIP."""

    def __init__(self, source: Path):
        self.source = source
        self._zip: zipfile.ZipFile | None = None
        self._prefix = ""
        if source.is_dir():
            self.kind = "directory"
            self.root = source
        elif source.is_file() and source.suffix.lower() == ".zip":
            self.kind = "zip"
            self.root = None
            self._zip = zipfile.ZipFile(source, "r")
            names = [
                PurePosixPath(i.filename)
                for i in self._zip.infolist()
                if not i.is_dir()
            ]
            if not names:
                raise ValueError("ZIP contains no files")
            roots = {p.parts[0] for p in names if p.parts}
            if len(roots) != 1:
                raise ValueError("ZIP must contain exactly one top-level package root")
            root = next(iter(roots))
            if root != PACKAGE_NAME:
                raise ValueError(
                    f"ZIP root is {root!r}, expected {PACKAGE_NAME!r}"
                )
            self._prefix = root + "/"
        else:
            raise ValueError("source must be an extracted package directory or .zip")

    def close(self) -> None:
        if self._zip is not None:
            self._zip.close()

    def files(self) -> list[str]:
        if self.kind == "directory":
            assert self.root is not None
            return sorted(
                p.relative_to(self.root).as_posix()
                for p in self.root.rglob("*")
                if p.is_file()
            )
        assert self._zip is not None
        return sorted(
            i.filename[len(self._prefix):]
            for i in self._zip.infolist()
            if not i.is_dir() and i.filename.startswith(self._prefix)
        )

    def read_bytes(self, relative: str) -> bytes:
        relative = PurePosixPath(relative).as_posix()
        if self.kind == "directory":
            assert self.root is not None
            path = (self.root / relative).resolve()
            root = self.root.resolve()
            if path != root and root not in path.parents:
                raise ValueError(f"path escapes package root: {relative}")
            return path.read_bytes()
        assert self._zip is not None
        return self._zip.read(self._prefix + relative)

    def read_text(self, relative: str) -> str:
        return self.read_bytes(relative).decode("utf-8")

    def read_json(self, relative: str) -> Any:
        return json.loads(self.read_text(relative))


def add(findings: list[Finding], code: str, message: str, condition: bool) -> None:
    if not condition:
        findings.append(Finding(code, message))


def unique(values: Iterable[Any]) -> bool:
    values = list(values)
    return len(values) == len(set(values))


def validate_manifest(reader: PackageReader) -> list[Finding]:
    findings: list[Finding] = []
    files = set(reader.files())
    add(findings, "E_PACKAGE_FILES", "required package files are missing",
        REQUIRED_FILES <= files)
    if "MANIFEST.json" not in files or "MANIFEST.sha256" not in files:
        return findings

    try:
        manifest_bytes = reader.read_bytes("MANIFEST.json")
        manifest = json.loads(manifest_bytes)
    except Exception as exc:  # noqa: BLE001 - validator reports malformed inputs
        return findings + [Finding("E_MANIFEST_PARSE", str(exc))]

    actual_manifest_sha = hashlib.sha256(manifest_bytes).hexdigest()
    recorded_manifest_sha = reader.read_text("MANIFEST.sha256").strip().split()[0]
    add(findings, "E_MANIFEST_SELF_HASH", "MANIFEST.sha256 does not match MANIFEST.json",
        recorded_manifest_sha.lower() == actual_manifest_sha.lower())

    add(findings, "E_MANIFEST_SCHEMA", "manifest package name mismatch",
        manifest.get("package") == PACKAGE_NAME)
    add(findings, "E_MANIFEST_SCHEMA", "manifest schema version must remain 1",
        manifest.get("version") == 1)

    entries = manifest.get("files")
    if not isinstance(entries, list):
        return findings + [Finding("E_MANIFEST_SCHEMA", "manifest files must be a list")]

    expected_files = files - {"MANIFEST.json", "MANIFEST.sha256"}
    recorded_paths = [entry.get("path") for entry in entries if isinstance(entry, dict)]
    add(findings, "E_MANIFEST_SET", "manifest file set differs from archive/directory",
        set(recorded_paths) == expected_files and unique(recorded_paths))

    for entry in entries:
        if not isinstance(entry, dict):
            findings.append(Finding("E_MANIFEST_SCHEMA", "non-object manifest entry"))
            continue
        path = entry.get("path")
        if not isinstance(path, str) or path not in expected_files:
            continue
        data = reader.read_bytes(path)
        add(findings, "E_MANIFEST_SIZE", f"size mismatch for {path}",
            entry.get("bytes") == len(data))
        add(findings, "E_MANIFEST_HASH", f"SHA-256 mismatch for {path}",
            str(entry.get("sha256", "")).lower() == hashlib.sha256(data).hexdigest())
    return findings


def validate_models(models: dict[str, Any], schema_text: str = "") -> list[Finding]:
    findings: list[Finding] = []
    contract = models["contract"]
    adapter = models["adapter"]
    snapshot = models["snapshot"]
    match = models["match"]
    ownership = models["ownership"]
    cuts = models["cuts"]
    blockers = models["blockers"]
    tests = models["tests"]

    package = contract.get("package", {})
    add(findings, "E_STATUS", "status must be READY_FOR_IMPLEMENTATION",
        package.get("status") == "READY_FOR_IMPLEMENTATION")
    add(findings, "E_OPEN_QUESTIONS", "open questions must be empty",
        package.get("open_questions") == [])
    add(findings, "E_DESIGN_ONLY", "package must be design-only",
        package.get("design_only") is True)
    versions = contract.get("versions", {})
    add(findings, "E_VERSION", "every Arcweft-owned marker must be exactly 1",
        bool(versions) and all(v == 1 for v in versions.values()))

    # Mandatory correction 1.
    handle = contract.get("need_handle", {})
    add(findings, "E_HANDLE_FIELDS", "RuntimeNeedHandle fields are not exact",
        handle.get("live_fields") == [
            "correlation: TaskCorrelation",
            "producer: NeedProducerSpec",
            "outcome: TaskOutcomeContract",
            "state: RuntimeNeedHandleState",
        ])
    states = {s.get("name"): s for s in handle.get("states", []) if isinstance(s, dict)}
    reusable = states.get("ReusableJoin", {})
    add(findings, "E_HANDLE_REUSABLE_SPEC",
        "ReusableJoin must retain exactly spec: Box<TaskSpec>",
        reusable.get("fields") == ["spec: Box<TaskSpec>"])
    add(findings, "E_HANDLE_STATE", "AcceptedLaunch must be a fieldless closed state",
        states.get("AcceptedLaunch", {}).get("fields") == [] and set(states) == {
            "ReusableJoin", "AcceptedLaunch"
        })
    constructors = handle.get("constructors", {})
    reusable_ctor = constructors.get("try_reusable_join", {})
    accepted_ctor = constructors.get("try_from_accepted_launch", {})
    add(findings, "E_HANDLE_CONSTRUCTOR", "reusable constructor may not mutate scheduler/adapter",
        reusable_ctor.get("mutates_scheduler") is False
        and reusable_ctor.get("mutates_adapter") is False)
    add(findings, "E_HANDLE_CONSTRUCTOR", "accepted constructor must be sealed and retain no spec",
        accepted_ctor.get("visibility") == "pub(crate)"
        and accepted_ctor.get("retains_spec") is False)
    add(findings, "E_HANDLE_IDENTITY", "handle identity must be NeedId-only",
        handle.get("identity") == "NeedId-only"
        and handle.get("always_start_state") == "AcceptedLaunch only")
    add(findings, "E_HANDLE_AWAIT", "both exact await state machines are required",
        set(handle.get("await", {})) == {"ReusableJoin", "AcceptedLaunch"}
        and any("sole scheduler" in step for step in handle["await"]["ReusableJoin"])
        and any("without correlation rederivation or task relaunch" in step
                for step in handle["await"]["AcceptedLaunch"]))

    # Mandatory correction 2.
    await_many = contract.get("await_many", {})
    exact_await_fields = [
        "captured: Box<[RuntimeValue]>",
        "source_items: Box<[RuntimeValue]>",
        "child: Box<NeedProducerTemplate>",
        "limit: NonZeroU32",
    ]
    add(findings, "E_AWAIT_MANY_EVIDENCE",
        "AwaitMany must retain captured/source/template/limit evidence",
        await_many.get("fields") == exact_await_fields
        and "Tuple(captured)" in await_many.get("child_argument_formula", "")
        and "source_items[i]" in await_many.get("child_argument_formula", "")
        and await_many.get("caller_supplies_child_digest") is False
        and "regenerate every child TaskSpec" in await_many.get("restore", ""))
    add(findings, "E_AWAIT_MANY_BASE", "aggregate base transcript must be source-order tuple",
        await_many.get("aggregate_base_argument_formula")
        == "RuntimeValue::Tuple(source_items in source order)")

    # Mandatory corrections 3 and 4.
    batch = contract.get("batch", {})
    required_batch_fields = {
        "journal: RuntimeJournalBatchDelta",
        "runtime: RuntimeTaskBatchDelta",
        "observers: RuntimeObserverBatchDelta",
        "prepared_host: Vec<PreparedLaunch>",
        "results: Vec<(u32, RuntimeNeedHandle, TaskObserverId)>",
    }
    add(findings, "E_BATCH_PLAN", "EnsureBatchPlan fields/whole-batch owner are incomplete",
        batch.get("whole_batch") is True
        and set(batch.get("plan_fields", [])) == required_batch_fields
        and batch.get("owner") == "RuntimeTaskScheduler::ensure_task_batch")
    add(findings, "E_BATCH_PER_CHILD_COMMIT",
        "aggregate batch may not call per-child committing ensure_task",
        batch.get("per_child_ensure_task_commits") is False)
    add(findings, "E_BATCH_ROLLBACK", "batch rollback and failure ID semantics are incomplete",
        batch.get("rollback_order") == "reverse preparation order"
        and batch.get("failure_consumes_task_ordinal") is False
        and batch.get("failure_consumes_observer_id") is False
        and batch.get("failure_launches_worker_visible_work") is False)

    observer = contract.get("observer_allocator")
    add(findings, "E_OBSERVER_ALLOCATOR",
        "each generation must persist a monotonic NonZeroU64 observer allocator",
        isinstance(observer, dict)
        and observer.get("present") is True
        and observer.get("owner") == "RuntimeGenerationJournal"
        and observer.get("field") == "next_observer_id: NonZeroU64"
        and observer.get("start") == 1
        and observer.get("domain") == "per generation"
        and "strictly greater" in observer.get("restore_rule", "")
        and observer.get("removal_rewinds") is False
        and observer.get("failed_registration_consumes") is False
        and observer.get("failed_batch_consumes") is False)

    # Mandatory corrections 5 and 6.
    required_methods = {
        "prepare_launch", "commit_launch", "rollback_launch",
        "prepare_restore", "commit_restore", "rollback_restore",
        "prepare_rebind", "commit_rebind", "rollback_rebind",
        "prepare_cancel", "commit_cancel", "rollback_cancel",
    }
    methods = set(adapter.get("methods", []))
    add(findings, "E_ADAPTER_CANCEL_METHOD",
        "prepare_cancel/commit_cancel/rollback_cancel are all required",
        {"prepare_cancel", "commit_cancel", "rollback_cancel"} <= methods)
    add(findings, "E_ADAPTER_METHOD", "complete launch/restore/rebind/cancel protocol missing",
        methods == required_methods)
    add(findings, "E_SANS_IO_DEPENDENCY",
        "scheduler must depend only on core, never host-adapter implementations",
        adapter.get("owner_crate") == "arcweft-core"
        and adapter.get("module") == "arcweft_core::task"
        and adapter.get("scheduler_crate") == "arcweft-runtime-scheduler"
        and adapter.get("scheduler_dependencies") == ["arcweft-core"]
        and not any("host-adapter" in dep or "runtime-host" in dep
                    for dep in adapter.get("scheduler_dependencies", [])))
    prepare = adapter.get("prepare_contract", {})
    add(findings, "E_ADAPTER_PREPARE_SIDE_EFFECT",
        "prepare may reserve only; it may not start workers or I/O",
        all(prepare.get(k) is False for k in [
            "worker_start", "network_io", "filesystem_io", "audio_io",
            "other_external_side_effect",
        ]))
    add(findings, "E_ADAPTER_COMMIT", "commit and rollback must be infallible",
        adapter.get("commit_contract", {}).get("fallible") is False
        and adapter.get("rollback_contract", {}).get("fallible") is False
        and adapter.get("cancel", {}).get("commit_fallible") is False)
    cancel = adapter.get("cancel", {})
    add(findings, "E_CANCEL_PROTOCOL", "typed complete idempotent cancel rows are required",
        cancel.get("batch") == "HostTaskCancelBatch { generation, rows }"
        and all(token in cancel.get("row", []) for token in [
            "command: HostCancelCommandId",
            "correlation: TaskCorrelation",
            "operation: HostOperationIdentityV1",
            "launch: HostLaunchCapability",
            "cancel: HostCancellationCapability",
            "reason: TaskCancelReason",
        ])
        and cancel.get("domain_error_payload") is False)
    add(findings, "E_ADAPTER_MIGRATION", "old immediate adapter timing must be deleted",
        adapter.get("migration", {}).get("wrapping_old_timing_is_accepted") is False)
    add(findings, "E_HOST_OPERATION_IDENTITY", "Host operation identity must be typed",
        adapter.get("operation_identity", {}).get("source_string_identity") is False
        and "Catalog { catalog_digest: HostOperationCatalogDigest, operation: HostOperationId }"
        in adapter.get("operation_identity", {}).get("variants", []))

    # Mandatory correction 7.
    add(findings, "E_SNAPSHOT_OWNER", "snapshot owner must evolve in place with no parallel reader",
        str(snapshot.get("owner", "")).endswith("AwbcRuntimeValueSnapshot")
        and "existing owner in place" in snapshot.get("replacement_policy", "")
        and "no second reader" in snapshot.get("replacement_policy", ""))
    final_variants = snapshot.get("final_live_variants", [])
    add(findings, "E_SNAPSHOT_INVENTORY", "final live RuntimeValue inventory is incomplete",
        final_variants == EXPECTED_FINAL_RUNTIME_VARIANTS
        and set(snapshot.get("live_variant_shapes", {})) == set(final_variants)
        and set(snapshot.get("snapshot_variant_shapes", {})) == set(final_variants)
        and set(snapshot.get("snapshot_tags", {})) == set(final_variants))
    role_fields = snapshot.get("snapshot_role_fields", {})
    lossy_text = json.dumps({
        "role_fields": role_fields,
        "shapes": snapshot.get("snapshot_variant_shapes", {}),
    }, sort_keys=True)
    add(findings, "E_SNAPSHOT_LOSSY",
        "snapshot contains or permits a lossy generic carrier",
        not any(form in lossy_text for form in [
            '"kind"', "kind: String", "items: Vec<RuntimeValueSummary>",
            "source: RuntimeValueSummary", "opaque bytes",
            "callable/captures summary",
        ])
        and role_fields.get("Seq") == [
            "Values recursive items",
            "Dense exact DenseSeq variant and storage",
            "TupleColumns len + recursive columns",
            "RecordColumns len + field identity/name/recursive values",
        ])
    add(findings, "E_DENSE_SEQ_INVENTORY", "all exact DenseSeq cases are required",
        snapshot.get("dense_seq_cases") == EXPECTED_DENSE_SEQ_CASES)
    definitions = snapshot.get("projection_definitions", {})
    references = set(snapshot.get("projection_references", []))
    missing_projections = sorted(ref for ref in references if ref not in definitions)
    add(findings, "E_UNDEFINED_PROJECTION",
        f"undefined projection(s): {', '.join(missing_projections)}",
        not missing_projections
        and "RuntimeCheckedTypeProjectionV1" in definitions
        and len(definitions.get("RuntimeCheckedTypeProjectionV1", [])) == 22
        and "RuntimeAgentValueProjectionV1" in definitions)
    function = snapshot.get("function_admission", {})
    add(findings, "E_FUNCTION_SNAPSHOT",
        "Structured functions must reject; AWBC functions require exact authority",
        function.get("Structured", {}).get("snapshot") == "reject"
        and function.get("Structured", {}).get("error")
        == "UnrebindableStructuredFunction"
        and function.get("Awbc", {}).get("snapshot")
        == "AwbcRuntimeFunctionSnapshot::Awbc"
        and "authority" in function.get("Awbc", {}))
    codec = snapshot.get("codec", {})
    add(findings, "E_COMPATIBILITY_READER", "compatibility/generic/string fallback is forbidden",
        codec.get("compatibility_reader") is False
        and codec.get("generic_serde_reader") is False
        and codec.get("string_fallback") is False
        and codec.get("opaque_bytes_summary") is False)

    # Mandatory correction 8.
    expr_rows = match.get("hir_expr_inventory", [])
    expr_names = [row.get("name") for row in expr_rows]
    expr_tags = [row.get("tag") for row in expr_rows]
    add(findings, "E_MATCH_EXPR_INVENTORY", "HirExprKind inventory/tags are not exact",
        match.get("hir_expr_inventory_count") == 38
        and expr_names == EXPECTED_EXPR_FAMILIES
        and unique(expr_names) and unique(expr_tags))
    role_rows = match.get("child_role_enum", [])
    role_names = [row.get("name") for row in role_rows]
    role_tags = [row.get("tag") for row in role_rows]
    add(findings, "E_MATCH_ROLE_ENUM", "CheckedExpressionChildRole is incomplete/unstable",
        len(role_rows) >= 62 and unique(role_names) and unique(role_tags)
        and all(isinstance(tag, int) and 0 <= tag <= 0xFFFF for tag in role_tags))
    pattern_rows = match.get("constructor_tables", {}).get("HirPatternKind", [])
    pattern_names = [row.get("name") for row in pattern_rows]
    pattern_tags = [row.get("tag") for row in pattern_rows]
    add(findings, "E_MATCH_PATTERN_INVENTORY", "HirPatternKind inventory/tags are not exact",
        pattern_names == EXPECTED_PATTERN_FAMILIES
        and unique(pattern_names) and unique(pattern_tags))
    callable_join = match.get("callable_join", {})
    add(findings, "E_MATCH_CALLABLE_JOIN",
        "Match transcript must require exact checked callable catalog joins",
        callable_join.get("required") is True
        and callable_join.get("source_catalog") == "CheckedCallableCatalogV1"
        and callable_join.get("source_key")
        == ["CheckedCallableId", "CheckedCallableDigest"]
        and callable_join.get("hir_name_is_identity") is False
        and callable_join.get("source_spelling_is_identity") is False
        and callable_join.get("arena_id_is_identity") is False)
    limits = match.get("work_limits", {})
    add(findings, "E_MATCH_LIMITS", "positive work limits and deterministic first error required",
        all(isinstance(v, int) and v > 0 for v in limits.values())
        and "first" in match.get("first_error", "").lower())
    add(findings, "E_MATCH_PERSISTENCE", "compiler-local IDs may not persist",
        "HirSnapshotId + ExprId" in match.get("lookup_key", "")
        and "compiler-local" in match.get("lookup_key", "")
        and match.get("persistent_compiler_ids") is False)

    # Mandatory correction 9.
    rows = ownership.get("rows", [])
    row_names = [row.get("type_kind") for row in rows]
    add(findings, "E_OWNERSHIP_INVENTORY", "ownership classifier must cover 85 current TypeKind rows once",
        ownership.get("type_kind_count") == 85
        and row_names == EXPECTED_TYPE_KINDS
        and unique(row_names))
    signed = {
        "I8": "RuntimeValue::Int(RuntimeInt::I8)",
        "I16": "RuntimeValue::Int(RuntimeInt::I16)",
        "I32": "RuntimeValue::Int(RuntimeInt::I32)",
        "I64": "RuntimeValue::Int(RuntimeInt::I64)",
        "I128": "RuntimeValue::Int(RuntimeInt::I128)",
        "ISize": "RuntimeValue::Int(RuntimeInt::ISize)",
    }
    unsigned = {
        "U8": "RuntimeValue::UInt(RuntimeUInt::U8)",
        "U16": "RuntimeValue::UInt(RuntimeUInt::U16)",
        "U32": "RuntimeValue::UInt(RuntimeUInt::U32)",
        "U64": "RuntimeValue::UInt(RuntimeUInt::U64)",
        "U128": "RuntimeValue::UInt(RuntimeUInt::U128)",
        "USize": "RuntimeValue::UInt(RuntimeUInt::USize)",
    }
    by_name = {row.get("type_kind"): row for row in rows}
    exact_numeric = (
        all(by_name.get(k, {}).get("live_carrier") == v
            and by_name.get(k, {}).get("snapshot_carrier")
            == "AwbcRuntimeValueSnapshot::Int"
            for k, v in signed.items())
        and all(by_name.get(k, {}).get("live_carrier") == v
                and by_name.get(k, {}).get("snapshot_carrier")
                == "AwbcRuntimeValueSnapshot::UInt"
                for k, v in unsigned.items())
    )
    serialized_rows = json.dumps(rows, sort_keys=True)
    add(findings, "E_OWNERSHIP_AMBIGUOUS",
        "signed/unsigned ownership carriers must be exact and IntOrUInt-free",
        exact_numeric and "IntOrUInt" not in serialized_rows)
    successful = [r for r in rows if r.get("disposition") != "Reject"]
    add(findings, "E_OWNERSHIP_SUCCESS_OWNER",
        "every successful ownership row needs exact projection/live/snapshot/identity",
        all(r.get("runtime_projection") and r.get("live_carrier")
                and r.get("snapshot_carrier") and r.get("canonical_identity")
                and r.get("rejection") is None for r in successful))
    rejected = [r for r in rows if r.get("disposition") == "Reject"]
    add(findings, "E_OWNERSHIP_REJECTION",
        "every rejected ownership row needs one typed rejection and no carrier",
        all(r.get("rejection") and r.get("live_carrier") is None
                and r.get("snapshot_carrier") is None for r in rejected))
    add(findings, "E_OWNERSHIP_PREDICATE_SHARED",
        "Predicate must be a leaf and Shared must reject before recursion",
        "TypeKind leaf" in by_name.get("Predicate", {}).get("recursion", "")
        and by_name.get("Shared", {}).get("rejection")
        == "MissingRuntimeSnapshotOwner"
        and by_name.get("Shared", {}).get("recursion")
        == "reject before child recursion")
    add(findings, "E_OWNERSHIP_CUT",
        "Need certificate must remain private until atomic Cut 5",
        by_name.get("Need", {}).get("publication_cut") == 5
        and by_name.get("Need", {}).get("visibility") == "private_until_cut_5")
    for family in ["Result", "Option"]:
        add(findings, "E_OWNERSHIP_VARIANT",
            f"{family} must have one exact Variant carrier",
            "RuntimeValue::Variant" in by_name.get(family, {}).get("live_carrier", "")
            and "AwbcRuntimeValueSnapshot::Variant"
            in by_name.get(family, {}).get("snapshot_carrier", ""))
    add(findings, "E_OWNERSHIP_CHOICE",
        "Choice must reject while its exact carrier remains ambiguous",
        by_name.get("Choice", {}).get("disposition") == "Reject"
        and by_name.get("Choice", {}).get("rejection")
        == "MissingRuntimeSnapshotOwner")

    # Mandatory correction 10.
    event = contract.get("event_order", {})
    add(findings, "E_EVENT_ORDER",
        "event order must place TaskId before sequence",
        event.get("single_generation")
        == ["logical_epoch", "task_id", "sequence"]
        and event.get("retained_generations")
        == ["generation", "logical_epoch", "task_id", "sequence"]
        and event.get("sequence_precedes_task_id") is False)
    policy = contract.get("snapshot_policy", {})
    add(findings, "E_SNAPSHOT_BLANKET_HOST_REJECT",
        "active Restartable Host rows must be persistable; blanket rejection forbidden",
        policy.get("blanket_active_host_rejection") is False
        and policy.get("restartable_active_host_persisted") is True
        and policy.get("prepared_adapter_transactions_block_snapshot") is True
        and policy.get("must_be_quiescent_active_host_blocks") is True
        and "prepare_restore" in policy.get("restartable_restore", ""))

    # Compile cuts.
    cut_rows = cuts.get("cuts", [])
    cut_numbers = [c.get("cut") for c in cut_rows]
    add(findings, "E_COMPILE_CUTS", "compile cuts must be exactly 1..5",
        cut_numbers == [1, 2, 3, 4, 5])
    for row in cut_rows:
        cut = row.get("cut")
        deps = row.get("depends_on_cuts", [])
        add(findings, "E_COMPILE_FORWARD_REFERENCE",
            f"Cut {cut} depends on same/later cut",
            isinstance(cut, int) and all(isinstance(d, int) and d < cut for d in deps))
    cut5 = next((c for c in cut_rows if c.get("cut") == 5), {})
    add(findings, "E_COMPILE_CUT5",
        "Cut 5 must atomically publish all final runtime owners and delete old routes",
        cut5.get("atomic") is True
        and cut5.get("task_types") is True
        and len(cut5.get("deletes", [])) >= 10
        and any("RuntimeNeedHandle" in p for p in cut5.get("publishes", []))
        and any("isomorphic" in p for p in cut5.get("publishes", [])))

    # Test and blocker inventories.
    blocker_rows = blockers if isinstance(blockers, list) else blockers.get("blockers", [])
    codes = [b.get("expected_error") for b in blocker_rows]
    add(findings, "E_BLOCKER_INVENTORY", "exactly twelve blocker self-tests are required",
        len(blocker_rows) == 12 and codes == EXPECTED_BLOCKER_CODES and unique(codes))
    test_rows = tests.get("tests", [])
    categories = {row.get("category") for row in test_rows}
    add(findings, "E_TEST_MATRIX", "100 concrete test rows are required",
        tests.get("version") == 1 and len(test_rows) == 100
        and tests.get("summary", {}).get("count") == 100
        and unique(row.get("id") for row in test_rows)
        and {"focused", "property", "differential", "tamper", "rollback",
             "restore", "negative"} <= categories)

    # Rust-shaped schema is checked for critical structural anchors, not compiled.
    if schema_text:
        required_anchors = [
            "pub struct RuntimeNeedHandle",
            "ReusableJoin { spec: Box<TaskSpec> }",
            "pub struct RuntimeAwaitManyAggregateRequest",
            "struct EnsureBatchPlan",
            "next_observer_id: NonZeroU64",
            "fn prepare_cancel",
            "pub enum RuntimeCheckedTypeProjectionV1",
            "pub enum CheckedExpressionChildRole",
            "pub enum AwbcRuntimeValueSnapshot",
        ]
        add(findings, "E_SCHEMA_ANCHOR", "Rust-shaped schema misses a critical owner",
            all(anchor in schema_text for anchor in required_anchors))
        add(findings, "E_SCHEMA_FORBIDDEN", "Rust-shaped schema contains IntOrUInt",
            "IntOrUInt" not in schema_text)

    return findings


def load_models(reader: PackageReader) -> dict[str, Any]:
    return {
        "contract": reader.read_json("machine/contract.json"),
        "adapter": reader.read_json("machine/adapter_protocol.json"),
        "snapshot": reader.read_json("machine/live_snapshot_inventory.json"),
        "match": reader.read_json("machine/match_roles.json"),
        "ownership": reader.read_json("machine/ownership_matrix.json"),
        "cuts": reader.read_json("machine/compile_cuts.json"),
        "blockers": reader.read_json("machine/blockers.json"),
        "tests": reader.read_json("machine/test_matrix.json"),
    }


def self_test(models: dict[str, Any], schema_text: str) -> list[Finding]:
    """Require every mandatory negative mutation to trigger its exact blocker."""
    results: list[Finding] = []

    mutations: list[tuple[str, str, Any]] = []

    def mutate_reusable(m: dict[str, Any]) -> None:
        for state in m["contract"]["need_handle"]["states"]:
            if state["name"] == "ReusableJoin":
                state["fields"] = []

    def mutate_await_many(m: dict[str, Any]) -> None:
        m["contract"]["await_many"]["fields"] = [
            f for f in m["contract"]["await_many"]["fields"]
            if not f.startswith("captured:") and not f.startswith("child:")
        ]

    def mutate_batch(m: dict[str, Any]) -> None:
        m["contract"]["batch"]["per_child_ensure_task_commits"] = True

    def mutate_observer(m: dict[str, Any]) -> None:
        m["contract"]["observer_allocator"] = None

    def mutate_cancel(m: dict[str, Any]) -> None:
        m["adapter"]["methods"].remove("prepare_cancel")

    def mutate_dependency(m: dict[str, Any]) -> None:
        m["adapter"]["scheduler_dependencies"].append("arcweft-host-adapter")

    def mutate_lossy(m: dict[str, Any]) -> None:
        m["snapshot"]["snapshot_role_fields"]["Seq"] = [
            "kind: String", "items: Vec<RuntimeValueSummary>"
        ]

    def mutate_projection(m: dict[str, Any]) -> None:
        del m["snapshot"]["projection_definitions"]["RuntimeCheckedTypeProjectionV1"]

    def mutate_callable(m: dict[str, Any]) -> None:
        m["match"]["callable_join"]["required"] = False

    def mutate_ownership(m: dict[str, Any]) -> None:
        for row in m["ownership"]["rows"]:
            if row["type_kind"] == "I8":
                row["live_carrier"] = "RuntimeValue::IntOrUInt"
                break

    def mutate_order(m: dict[str, Any]) -> None:
        m["contract"]["event_order"]["single_generation"] = [
            "logical_epoch", "sequence", "task_id"
        ]

    def mutate_host_policy(m: dict[str, Any]) -> None:
        m["contract"]["snapshot_policy"]["blanket_active_host_rejection"] = True

    mutations.extend([
        ("B01", "E_HANDLE_REUSABLE_SPEC", mutate_reusable),
        ("B02", "E_AWAIT_MANY_EVIDENCE", mutate_await_many),
        ("B03", "E_BATCH_PER_CHILD_COMMIT", mutate_batch),
        ("B04", "E_OBSERVER_ALLOCATOR", mutate_observer),
        ("B05", "E_ADAPTER_CANCEL_METHOD", mutate_cancel),
        ("B06", "E_SANS_IO_DEPENDENCY", mutate_dependency),
        ("B07", "E_SNAPSHOT_LOSSY", mutate_lossy),
        ("B08", "E_UNDEFINED_PROJECTION", mutate_projection),
        ("B09", "E_MATCH_CALLABLE_JOIN", mutate_callable),
        ("B10", "E_OWNERSHIP_AMBIGUOUS", mutate_ownership),
        ("B11", "E_EVENT_ORDER", mutate_order),
        ("B12", "E_SNAPSHOT_BLANKET_HOST_REJECT", mutate_host_policy),
    ])

    for blocker, expected, mutate in mutations:
        candidate = copy.deepcopy(models)
        mutate(candidate)
        codes = {f.code for f in validate_models(candidate, schema_text)}
        if expected not in codes:
            results.append(Finding(
                "E_SELF_TEST",
                f"{blocker} mutation did not trigger {expected}; got {sorted(codes)}",
            ))
    return results


def validate(reader: PackageReader, run_self_test: bool) -> tuple[list[Finding], int]:
    findings = validate_manifest(reader)
    try:
        models = load_models(reader)
        schema_text = reader.read_text("schemas/final_contract.rs")
    except Exception as exc:  # noqa: BLE001
        return findings + [Finding("E_PACKAGE_PARSE", str(exc))], 0

    findings.extend(validate_models(models, schema_text))
    self_test_count = 0
    if run_self_test:
        self_test_count = 12
        findings.extend(self_test(models, schema_text))
    return findings, self_test_count


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path,
                        help="extracted package directory or independently throwable ZIP")
    parser.add_argument("--self-test", action="store_true",
                        help="run all twelve mandatory negative in-memory mutations")
    args = parser.parse_args(argv)

    try:
        reader = PackageReader(args.source)
    except Exception as exc:  # noqa: BLE001
        print(f"FAIL E_OPEN: {exc}")
        return 2

    try:
        findings, self_tests = validate(reader, args.self_test)
        manifest = reader.read_json("MANIFEST.json")
        manifest_count = len(manifest.get("files", []))
        tests = reader.read_json("machine/test_matrix.json")
        test_count = len(tests.get("tests", []))
        blockers = reader.read_json("machine/blockers.json")
        blocker_count = len(blockers.get("blockers", blockers)) if isinstance(blockers, dict) else len(blockers)
    except Exception as exc:  # noqa: BLE001
        print(f"FAIL E_VALIDATE: {exc}")
        return 2
    finally:
        reader.close()

    if findings:
        print(f"FAIL findings={len(findings)}")
        for finding in findings:
            print(f"  {finding.render()}")
        return 1

    print(f"PASS package={PACKAGE_NAME}")
    print(f"source_kind={reader.kind}")
    print(f"manifest_entries={manifest_count}")
    print(f"normative_test_rows={test_count}")
    print(f"blocker_rows={blocker_count}")
    if args.self_test:
        print(f"negative_self_tests={self_tests}/12 PASS")
    print("status=READY_FOR_IMPLEMENTATION")
    print("open_questions=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
