#!/usr/bin/env python3
"""Read-only validator for the runtime-task/Match-substrate final contract.

The validator uses only the Python standard library. Positive validation reads
the package. Negative self-tests mutate deep-copied in-memory machine data and
never write into the package.
"""

from __future__ import annotations

import argparse
import copy
import csv
import hashlib
import json
import os
from pathlib import Path
import sys
from typing import Any, Callable, Iterable

PACKAGE_NAME = 'arcweft-lang-01.5.1.1.2.1.1.1.1.1.1-runtime-task-persistence-and-match-substrate-correction-final-contract'
EXPECTED_MAIN = '3670625a02b9e7e8578b57fc7b148a1758a17dba'
EXPECTED_REQUEST_BLOB = '6b3d614e7813fa6552e84f15610175633470227d'
EXPECTED_INPUT_HASHES = {'inputs/CURRENT_REQUEST.md': '804f68c052640fe3964e70bfe011cad2c4429873a70b790c3a0526b5f46c7e6e', 'inputs/RUST_SKILL.txt': '1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665', 'inputs/PROJECT_PREMISE.txt': 'cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1'}
EXPECTED_TYPE_KINDS = ['Bool', 'I8', 'I16', 'I32', 'I64', 'I128', 'ISize', 'U8', 'U16', 'U32', 'U64', 'U128', 'USize', 'F32', 'F64', 'String', 'Char', 'Bytes', 'TextCluster', 'Duration', 'Progress', 'StageApi', 'LineContext', 'StageActorHandle', 'CueHandle', 'VoiceHandle', 'Range', 'IteratorState', 'DisplayText', 'DebugStatePath', 'ObservationFieldPath', 'Ref', 'Probe', 'Predicate', 'Observation', 'ObservedObject', 'AgentBBox', 'ActionName', 'ActionTarget', 'ActionResult', 'AgentValue', 'DataFormat', 'DataShape', 'AgentEntityMetadata', 'AgentSourceAnchor', 'AgentProjectGraphNeighborhood', 'AgentProjectGraphSymbol', 'AgentProjectGraphEdge', 'CaptureTarget', 'CaptureRef', 'AgentResource', 'AgentResourceBody', 'RagContextPack', 'AgentBuiltin', 'Vec', 'Array', 'Slice', 'Seq', 'Map', 'BorrowRef', 'Need', 'Stream', 'Result', 'Option', 'Handle', 'ThreadHandle', 'Shared', 'Function', 'GenericParam', 'ProjectNominal', 'AcceptedNominal', 'OpenNominal', 'Error', 'Projection', 'CharacterPatch', 'FocusPatch', 'CharacterDialogue', 'DialogueLine', 'ViewValue', 'CharacterNominal', 'Named', 'Tuple', 'Choice', 'Unit', 'Never']
EXPECTED_SCHEMA_NAMES = ['RuntimeTaskSchedulerSnapshotV1', 'RuntimeTaskJournalSnapshotV1', 'RuntimeTaskGenerationSnapshotV1', 'AlwaysStartOrdinalCounterSnapshotV1', 'TaskGroupSnapshotV1', 'TaskLaunchMappingSnapshotV1', 'TaskLaunchSnapshotV1', 'TaskSpecSnapshotV1', 'NeedProducerSpecSnapshotV1', 'TaskCorrelationSnapshotV1', 'TaskLifecycleSnapshotV1', 'TaskOutcomeContractSnapshotV1', 'TaskDebugMetadataSnapshotV1', 'TaskExecutionSnapshotV1', 'HostTaskRequestSnapshotV1', 'HeaderSnapshotV1', 'EnvPairSnapshotV1', 'NamedRuntimePayloadSnapshotV1', 'HostTaskStateSnapshotV1', 'HostTaskPhaseSnapshotV1', 'HostTaskRestorePolicySnapshotV1', 'RuntimeTaskRequestSnapshotV1', 'RuntimeAwaitManyAggregateRequestSnapshotV1', 'RuntimeTimeoutRequestSnapshotV1', 'RuntimeTaskStateSnapshotV1', 'RuntimeTaskRowSnapshotV1', 'RuntimeTaskRequestStateSnapshotV1', 'RuntimeAwaitManyAggregateTaskSnapshotV1', 'RuntimeAwaitManyChildSnapshotV1', 'RuntimeAwaitManyChildStatusSnapshotV1', 'RuntimeAwaitManyTerminalSnapshotV1', 'RuntimeTimeoutNeedSnapshotV1', 'RuntimeTimeoutPhaseSnapshotV1', 'RuntimeTimeoutTerminalSnapshotV1', 'NeedCellSnapshotV1', 'NeedStateSnapshotV1', 'RuntimeNeedOutcomeSnapshotV1', 'RuntimeTaskFailureSnapshotV1', 'TaskObserverSnapshotV1', 'TaskObserverKindSnapshotV1', 'RuntimeNeedHandleSnapshotV1', 'NeedHandleOriginSnapshotV1', 'TaskPublicationCursorSnapshotV1', 'TaskEventSnapshotV1', 'TaskEventKindSnapshotV1', 'TaskReplayStateSnapshotV1', 'TaskReplayCursorSnapshotV1', 'TaskReplayDigestSnapshotV1', 'TaskReplayEnvelopeV1', 'ReplacementStateSnapshotV1', 'ReplacementPlanSnapshotV1', 'ReplacementViewMappingSnapshotV1', 'ReplacementTaskMappingSnapshotV1', 'RuntimePayloadSnapshotV1', 'RuntimeValueSnapshotV1', 'RuntimeCheckedTypeSnapshotV1', 'ProgressSnapshotV1', 'LogicalDurationSnapshotV1', 'RuntimeIntSnapshotV1', 'RuntimeUIntSnapshotV1', 'RuntimeMatrixSnapshotV1', 'RuntimeTensorSnapshotV1', 'RuntimeRangeSnapshotV1', 'RuntimeIteratorSnapshotV1', 'RuntimeSeqSnapshotV1', 'RuntimeFieldSnapshotV1', 'RuntimeNominalRecordSnapshotV1', 'RuntimeOpaqueValueSnapshotV1', 'RuntimeReductionSnapshotV1', 'RuntimeAgentSnapshotV1', 'RuntimeFunctionSnapshotV1', 'RuntimeVariantSnapshotV1']
EXPECTED_EXPRESSION_RESOLUTIONS = ['Structural', 'Literal', 'Value', 'Select', 'Nominal', 'Variant', 'StageLook', 'Effect', 'Call', 'Await', 'Choice', 'Try', 'ImplicitCallable', 'ImplicitParameter', 'Pipe', 'PipeLeft', 'ViewCall', 'ViewCallee', 'StyleValue', 'StyleCallee', 'DialogueLineReference', 'DialogueLineCoordinate', 'DialogueTextKeyCoordinate', 'CharacterDialogueFactory', 'CharacterDialogueReconfigure', 'DialogueApplication', 'PostfixBracket']
EXPECTED_VALUE_RESOLUTIONS = ['Local', 'LineContext', 'CharacterField', 'ProjectCallable', 'ProjectItem', 'Entry', 'Registered', 'Constant']
EXPECTED_SELECT_RESOLUTIONS = ['Method', 'DialogueView', 'AgentField', 'ProgressField', 'Field', 'TupleElement', 'RecordElement']
EXPECTED_PATTERN_RESOLUTIONS = ['Structural', 'Literal', 'Entity', 'Nominal', 'Variant']
EXPECTED_HIR_PATTERN_FAMILIES = ['Binding', 'MutableBinding', 'Literal', 'EntityReference', 'Variant', 'Discard', 'Tuple', 'Record', 'BracketSequence', 'WholeBinding', 'Or', 'TypedBinding', 'Error']
EXPECTED_LITERAL_FAMILIES = ['String', 'Character', 'Integer', 'Float', 'UnitNumber', 'Boolean', 'Duration']
EXPECTED_FAMILY_MATRIX = {'StructuredTaskPlan': {'allowed_execution': ['Host'], 'allowed_policies': ['JoinSameKey', 'AlwaysStart']}, 'AwbcTaskPlan': {'allowed_execution': ['Host'], 'allowed_policies': ['JoinSameKey', 'AlwaysStart']}, 'ViewMatchSubscription': {'allowed_execution': ['Host'], 'allowed_policies': ['JoinSameKey']}, 'AwaitManyBase': {'allowed_execution': ['RuntimeAwaitManyAggregate'], 'allowed_policies': ['JoinSameKey']}, 'AwaitManyChild': {'allowed_execution': ['Host', 'RuntimeAwaitManyAggregate', 'RuntimeTimeout'], 'allowed_policies': ['JoinSameKey', 'AlwaysStart']}, 'Timeout': {'allowed_execution': ['RuntimeTimeout'], 'allowed_policies': ['JoinSameKey']}, 'LineTask': {'allowed_execution': ['Host'], 'allowed_policies': ['AlwaysStart']}, 'HostAdapterTask': {'allowed_execution': ['Host'], 'allowed_policies': ['JoinSameKey', 'AlwaysStart']}, 'MakeNeedHandle': {'allowed_execution': ['Host'], 'allowed_policies': ['JoinSameKey']}}

EXPECTED_TASK_SPEC_FIELDS = [
    "producer", "class", "priority", "cancel_scope", "policy", "outcome",
    "execution", "debug",
]
EXPECTED_BUNDLE_FIELDS = [
    "version", "program", "accepted_revision", "site", "checked_match",
    "view_admission", "need_admission", "ownership", "producer_contract",
    "payload_type", "plan", "arguments", "resource_dependency",
]
CUT4_TYPES_FORBIDDEN_IN_CUT3 = {
    "NeedProducerContractDigest",
    "TaskPlanSemanticDigest",
    "RuntimeValueDigest",
    "TaskSpec",
    "TaskExecution",
    "AcceptedViewMatchBundleRowV1",
    "RuntimeValue::NeedHandle",
}
FORBIDDEN_BUNDLE_FIELDS = {
    "CheckedMatchRef", "ExprId", "HirSnapshotId", "SourceSpan",
    "compiler_certificate",
}
FORBIDDEN_NONEXISTENT_CRATE_TARGETS = {
    "crates/arcweft-native-adapter",
    "crates/arcweft-web-adapter",
    "crates/arcweft-headless",
}
REQUIRED_FILES = [
    "README.md",
    "FINAL_STATUS.md",
    "OPEN_QUESTIONS.md",
    "FINAL_CONTRACT.md",
    "DECISION_REGISTER.md",
    "RUST_SCHEMAS.md",
    "CANONICAL_VALUE_AND_CONSTANT_ADMISSION.md",
    "EXECUTION_TRUTH_TABLE.md",
    "SCHEDULER_OWNER_AND_API.md",
    "STATE_MACHINES.md",
    "FAILURE_PRECEDENCE_AND_ATOMICITY.md",
    "PERSISTENCE_AND_REPLAY.md",
    "MATCH_SEMANTIC_TRANSCRIPTS.md",
    "VIEW_BUNDLE_PROJECTION.md",
    "OWNERSHIP_MATRIX.md",
    "IDENTITY_AND_USE.md",
    "OWNER_API_MAP.md",
    "DEPENDENCY_GRAPH.md",
    "SOURCE_EVIDENCE.md",
    "DELETION_MATRIX.md",
    "COMPILE_CLEAN_SEQUENCE.md",
    "TEST_MATRIX.md",
    "REQUIREMENT_TRACEABILITY.md",
    "STRUCTURAL_ABSENCE.md",
    "VALIDATION_SCOPE.md",
    "VALIDATION.md",
    "examples/ensure_task_transaction.md",
    "examples/snapshot_restore_transaction.md",
    "examples/match_coordinate.md",
    "machine/contract.json",
    "machine/producer_execution_truth_table.json",
    "machine/persistence_schemas.json",
    "machine/expression_transcripts.json",
    "machine/pattern_transcripts.json",
    "machine/ownership_matrix.json",
    "machine/compile_cuts.json",
    "machine/deletion_matrix.json",
    "machine/source_evidence.json",
    "machine/tests.json",
    "machine/traceability.json",
    "machine/owner_api_map.json",
    "tables/README.md",
    "tables/producer_execution_truth_table.csv",
    "tables/ownership_matrix.csv",
    "tables/persistence_schemas.csv",
    "tables/expression_transcripts.csv",
    "tables/literal_transcripts.csv",
    "tables/pattern_transcripts.csv",
    "tables/deletion_matrix.csv",
    "tables/tests.csv",
    "tables/source_evidence.csv",
    "tables/compile_cuts.csv",
    "tables/owner_api_map.csv",
    "tables/requirement_traceability.csv",
    "inputs/CURRENT_REQUEST.md",
    "inputs/RUST_SKILL.txt",
    "inputs/PROJECT_PREMISE.txt",
]

class PackageValidationFailure(Exception):
    """Raised only for command-line package-root failures."""


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        raise PackageValidationFailure(f"missing JSON file: {path}") from None
    except json.JSONDecodeError as error:
        raise PackageValidationFailure(
            f"invalid JSON {path}: line {error.lineno}, column {error.colno}: {error.msg}"
        ) from None


def load_machine_data(package_root: Path) -> dict[str, Any]:
    return {
        "contract": load_json(package_root / "machine/contract.json"),
        "families": load_json(
            package_root / "machine/producer_execution_truth_table.json"
        ),
        "schemas": load_json(
            package_root / "machine/persistence_schemas.json"
        )["schemas"],
        "expressions": load_json(
            package_root / "machine/expression_transcripts.json"
        ),
        "patterns": load_json(
            package_root / "machine/pattern_transcripts.json"
        ),
        "ownership": load_json(
            package_root / "machine/ownership_matrix.json"
        )["rows"],
        "cuts": load_json(package_root / "machine/compile_cuts.json")["cuts"],
        "deletions": load_json(
            package_root / "machine/deletion_matrix.json"
        )["rows"],
        "source": load_json(package_root / "machine/source_evidence.json"),
        "tests": load_json(package_root / "machine/tests.json")["tests"],
        "traceability": load_json(
            package_root / "machine/traceability.json"
        )["requirements"],
        "owner_api": load_json(
            package_root / "machine/owner_api_map.json"
        )["rows"],
    }


def add(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def unique(values: Iterable[Any]) -> bool:
    values = list(values)
    try:
        return len(values) == len(set(values))
    except TypeError:
        return False


def row_by(rows: list[dict[str, Any]], key: str, value: Any) -> dict[str, Any] | None:
    return next((row for row in rows if row.get(key) == value), None)


def variant_names(rows: list[dict[str, Any]]) -> list[str]:
    return [str(row.get("variant", "")) for row in rows]


def validate_contract_data(data: dict[str, Any]) -> list[str]:
    """Validate closed machine contract data.

    This function does not touch the filesystem so the negative self-tests can
    exercise the exact same logic on deep-copied corruptions.
    """

    errors: list[str] = []
    contract = data["contract"]

    add(errors, contract.get("package") == PACKAGE_NAME, "package identity mismatch")
    add(
        errors,
        contract.get("status") == "READY_FOR_IMPLEMENTATION",
        "status must be READY_FOR_IMPLEMENTATION",
    )
    add(errors, contract.get("open_questions") == 0, "open questions must equal zero")
    add(
        errors,
        contract.get("repository") == "Sanzentyo/arcweft",
        "repository identity mismatch",
    )
    add(
        errors,
        contract.get("inspected_main") == EXPECTED_MAIN,
        "inspected main SHA mismatch",
    )
    add(
        errors,
        contract.get("request_blob_sha") == EXPECTED_REQUEST_BLOB,
        "request blob SHA mismatch",
    )
    add(
        errors,
        contract.get("all_arcweft_version_markers") == [1],
        "all Arcweft-owned version markers must be exactly [1]",
    )
    add(
        errors,
        contract.get("corrections_closed")
        == [f"mandatory_correction_{index}" for index in range(1, 11)],
        "mandatory correction inventory must be exactly 1 through 10",
    )

    frozen = contract.get("frozen_decisions", {})
    add(
        errors,
        frozen.get("producer_instance_fields")
        == [
            "family",
            "contract",
            "plan",
            "producer_site",
            "payload_type",
            "arguments",
        ],
        "producer instance transcript fields changed",
    )
    add(errors, frozen.get("join_ordinal") == 0, "Join ordinal must be zero")
    add(
        errors,
        frozen.get("always_start_first_ordinal") == 1,
        "AlwaysStart first ordinal must be one",
    )
    add(
        errors,
        frozen.get("task_key_includes_ordinal") is False,
        "TaskKey must exclude ordinal",
    )
    add(
        errors,
        frozen.get("task_id_includes_ordinal_count") == 1,
        "TaskId must include ordinal exactly once",
    )
    add(
        errors,
        frozen.get("reusable_handles") == ["JoinSameKey"],
        "only JoinSameKey may create reusable handles",
    )
    add(
        errors,
        set(frozen.get("fixed_ids_reject_zero", []))
        == {"NeedProducerInstanceKey", "NeedId", "TaskKey", "TaskId"},
        "fixed zero-reject identity set changed",
    )
    add(
        errors,
        frozen.get("semantic_digests_zero_is_valid") is True,
        "semantic digests must accept complete zero hash outputs",
    )
    add(
        errors,
        frozen.get("view_identity") == "ViewProgramId",
        "ViewProgramId role changed",
    )
    add(
        errors,
        frozen.get("view_revision")
        == "AcceptedViewProgramRevision([u8; 32])",
        "accepted View revision role changed",
    )

    canonical = contract.get("canonical_value", {})
    add(
        errors,
        canonical.get("sole_owner")
        == "arcweft_core::entry canonical RuntimeValue visitor",
        "canonical RuntimeValue identity must have one core owner",
    )
    add(
        errors,
        canonical.get("byte_and_blake3_sinks_share_visitor") is True,
        "byte and BLAKE3 sinks must share one exhaustive visitor",
    )
    add(
        errors,
        canonical.get("opaque_plain_persistence_admitted")
        == ["ConstantAndSnapshot", "SnapshotOnly"],
        "Plain opaque identity must admit exactly both persistence modes",
    )
    add(
        errors,
        canonical.get("opaque_affine_identity") == "reject",
        "affine opaque values must not receive RuntimeValueDigest",
    )
    add(
        errors,
        canonical.get("need_handle_tag") == 20,
        "NeedHandle canonical tag must remain 20",
    )
    add(
        errors,
        canonical.get("need_handle_payload") == ["NeedId"],
        "NeedHandle canonical payload must be exactly NeedId",
    )
    pair = canonical.get("plain_snapshot_only_pair", {})
    add(
        errors,
        pair
        == {
            "canonical_bytes": "success",
            "direct_digest": "success and equals BLAKE3(canonical_bytes)",
            "producer_admission": "success",
            "snapshot_roundtrip": "success",
            "constant_publication": "ConstantAdmissionError::SnapshotOnlyOpaque",
        },
        "Plain+SnapshotOnly paired evidence contract is incomplete",
    )

    task_spec = contract.get("task_spec", {})
    add(
        errors,
        task_spec.get("fields") == EXPECTED_TASK_SPEC_FIELDS,
        "TaskSpec fields must contain exactly one execution field and no request field",
    )
    forbidden_task_fields = set(task_spec.get("forbidden_fields", []))
    add(
        errors,
        forbidden_task_fields == {"request", "host_request", "runtime_request"},
        "TaskSpec forbidden-field set changed",
    )
    add(
        errors,
        not (set(task_spec.get("fields", [])) & forbidden_task_fields),
        "TaskSpec contains an unconditional or parallel request field",
    )
    add(
        errors,
        task_spec.get("execution_field") == "TaskExecution",
        "TaskSpec execution field type mismatch",
    )
    add(
        errors,
        task_spec.get("no_parallel_option_pair") is True,
        "parallel optional host/runtime requests are forbidden",
    )

    execution = contract.get("task_execution", {})
    add(
        errors,
        execution.get("variants")
        == {"Host": "HostTaskRequest", "Runtime": "RuntimeTaskRequest"},
        "TaskExecution variants changed",
    )
    add(
        errors,
        execution.get("runtime_variants")
        == {
            "AwaitManyAggregate": "RuntimeAwaitManyAggregateRequest",
            "Timeout": "RuntimeTimeoutRequest",
        },
        "RuntimeTaskRequest variants changed",
    )

    scheduler = contract.get("scheduler_owner", {})
    add(
        errors,
        scheduler.get("type")
        == "arcweft_runtime_scheduler::RuntimeTaskScheduler<A: TaskLaunchAdapter>",
        "sole scheduler/journal/adapter owner changed",
    )
    add(errors, scheduler.get("driver_role") == "consumer only", "driver must be consumer only")
    add(errors, scheduler.get("unsafe") is False, "scheduler design must not require unsafe")
    add(
        errors,
        scheduler.get("interior_global_state") is False,
        "scheduler design must not require interior global state",
    )
    add(
        errors,
        scheduler.get("fallible_after_irreversible_commit") is False,
        "fallible work after irreversible commit is forbidden",
    )

    adapter = contract.get("adapter", {})
    add(
        errors,
        adapter.get("prepare_launch_rows") == ["Host"],
        "adapter prepare must accept Host rows only",
    )
    add(
        errors,
        adapter.get("runtime_rows_reach_adapter") is False,
        "runtime-owned tasks must never reach the host adapter",
    )
    add(errors, adapter.get("commit_return") == "()", "adapter commit must be infallible")
    add(errors, adapter.get("rollback_return") == "()", "adapter rollback must be infallible")
    add(
        errors,
        "AdapterCommit" not in set(adapter.get("error_variants", [])),
        "AdapterCommit must not be a reachable error variant",
    )
    add(
        errors,
        adapter.get("forbidden_error_variants") == ["AdapterCommit"],
        "AdapterCommit structural-absence marker missing",
    )

    handle = contract.get("need_handle", {})
    add(errors, handle.get("semantic_key") == "NeedId", "NeedHandle semantic key must be NeedId")
    add(
        errors,
        handle.get("manual_eq_hash_ord") is True,
        "RuntimeNeedHandle requires manual Eq/Hash/Ord by NeedId",
    )
    add(
        errors,
        handle.get("canonical_identity_fields") == ["NeedId"],
        "NeedHandle value identity includes non-NeedId fields",
    )
    add(
        errors,
        handle.get("ordinary_use_requires_active_generation") is True,
        "ordinary NeedHandle use must validate active generation",
    )
    add(
        errors,
        handle.get("replacement_may_rebind_generation") is True,
        "only replacement may rebind generation",
    )

    persistence = contract.get("persistence", {})
    add(errors, persistence.get("version") == 1, "persistence version must be exactly 1")
    add(
        errors,
        persistence.get("compatibility_reader") is False,
        "compatibility snapshot reader is forbidden",
    )
    add(errors, persistence.get("string_fallback") is False, "String snapshot fallback is forbidden")
    for field in ("unknown_rejection", "duplicate_rejection", "trailing_rejection"):
        add(errors, persistence.get(field) is True, f"strict persistence {field} must be enabled")
    add(
        errors,
        persistence.get("prepared_adapter_tokens_persisted") is False,
        "prepared adapter tokens must not be persisted",
    )

    match = contract.get("match_substrate", {})
    add(
        errors,
        match.get("compiler_local_ref") == ["HirSnapshotId", "ExprId"],
        "CheckedMatchRef must use current HirSnapshotId plus ExprId",
    )
    add(
        errors,
        match.get("persistent_bundle_contains_compiler_local_ids") is False,
        "persistent bundle must exclude compiler-local IDs",
    )
    add(
        errors,
        match.get("expression_resolution_variant_count")
        == len(EXPECTED_EXPRESSION_RESOLUTIONS),
        "Match expression resolution count mismatch",
    )
    add(
        errors,
        match.get("pattern_resolution_variant_count")
        == len(EXPECTED_PATTERN_RESOLUTIONS),
        "Match pattern resolution count mismatch",
    )
    add(
        errors,
        match.get("hir_pattern_family_count")
        == len(EXPECTED_HIR_PATTERN_FAMILIES),
        "HIR pattern family count mismatch",
    )
    add(
        errors,
        match.get("literal_variant_count") == len(EXPECTED_LITERAL_FAMILIES),
        "literal family count mismatch",
    )

    own_contract = contract.get("ownership", {})
    add(
        errors,
        own_contract.get("type_kind_count") == len(EXPECTED_TYPE_KINDS),
        "TypeKind count mismatch",
    )
    add(
        errors,
        own_contract.get("predicate_recursion") == "none",
        "Predicate must be a TypeKind leaf",
    )
    add(
        errors,
        own_contract.get("shared")
        == {"disposition": "Reject", "reason": "MissingRuntimeSnapshotOwner"},
        "Shared must remain MissingRuntimeSnapshotOwner",
    )
    add(
        errors,
        own_contract.get("snapshot_clone_required_evidence")
        == [
            "runtime_projection",
            "live_carrier",
            "canonical_identity",
            "snapshot_codec",
        ],
        "SnapshotClone evidence columns changed",
    )

    bundle = contract.get("bundle", {})
    add(
        errors,
        bundle.get("type") == "AcceptedViewMatchBundleRowV1",
        "persistent View Match bundle owner mismatch",
    )
    add(
        errors,
        bundle.get("fields") == EXPECTED_BUNDLE_FIELDS,
        "AcceptedViewMatchBundleRowV1 field set/order mismatch",
    )
    add(
        errors,
        set(bundle.get("forbidden_fields", [])) == FORBIDDEN_BUNDLE_FIELDS,
        "bundle compiler-local forbidden-field set mismatch",
    )
    add(
        errors,
        not (set(bundle.get("fields", [])) & FORBIDDEN_BUNDLE_FIELDS),
        "compiler-local ID or certificate embedded in persistent bundle",
    )
    add(errors, contract.get("compile_cuts") == 5, "compile-cut count must equal five")

    # Nine-family execution truth table.
    families = data["families"]
    names = [row.get("family") for row in families]
    add(
        errors,
        names == list(EXPECTED_FAMILY_MATRIX),
        "producer family inventory/order must contain exactly the nine current families",
    )
    add(errors, unique(names), "producer family names must be unique")
    for row in families:
        name = row.get("family")
        expected = EXPECTED_FAMILY_MATRIX.get(name)
        if expected is None:
            errors.append(f"unknown producer family: {name!r}")
            continue
        add(
            errors,
            row.get("allowed_execution") == expected["allowed_execution"],
            f"producer family {name} execution routes changed",
        )
        add(
            errors,
            row.get("allowed_policies") == expected["allowed_policies"],
            f"producer family {name} policy set changed",
        )
        add(
            errors,
            bool(str(row.get("restriction", "")).strip()),
            f"producer family {name} lacks a policy restriction",
        )
    base = row_by(families, "family", "AwaitManyBase")
    timeout = row_by(families, "family", "Timeout")
    add(
        errors,
        base is not None and base.get("allowed_execution") == ["RuntimeAwaitManyAggregate"],
        "AwaitManyBase must be runtime-owned",
    )
    add(
        errors,
        timeout is not None and timeout.get("allowed_execution") == ["RuntimeTimeout"],
        "Timeout must be runtime-owned",
    )

    # Complete persistence graph.
    schemas = data["schemas"]
    schema_names = [row.get("name") for row in schemas]
    add(
        errors,
        schema_names == EXPECTED_SCHEMA_NAMES,
        "persistence schema inventory/order must contain exactly 72 rows",
    )
    add(errors, unique(schema_names), "persistence schema names must be unique")
    schema_set = set(schema_names)
    for row in schemas:
        name = str(row.get("name", ""))
        kind = row.get("kind")
        add(errors, kind in {"struct", "enum"}, f"{name}: schema kind must be struct or enum")
        if kind == "struct":
            fields = row.get("fields", [])
            add(errors, isinstance(fields, list) and bool(fields), f"{name}: struct fields are undefined")
            field_names = [field[0] for field in fields if isinstance(field, list) and len(field) == 2]
            add(
                errors,
                len(field_names) == len(fields) and unique(field_names),
                f"{name}: malformed or duplicate struct field",
            )
        if kind == "enum":
            variants = row.get("variants", [])
            add(errors, isinstance(variants, list) and bool(variants), f"{name}: enum variants are undefined")
            variant_ids = [
                variant[0] for variant in variants
                if isinstance(variant, list) and len(variant) == 2
            ]
            add(
                errors,
                len(variant_ids) == len(variants) and unique(variant_ids),
                f"{name}: malformed or duplicate enum variant",
            )
        if row.get("versioned"):
            add(errors, row.get("version") == 1, f"{name}: version marker must be exactly 1")
        else:
            add(errors, row.get("version") is None, f"{name}: unversioned row must not carry a version")
        for strict_key in ("unknown_fields", "duplicate_fields", "trailing_bytes"):
            add(errors, row.get(strict_key) == "reject", f"{name}: {strict_key} must reject")
        add(errors, bool(str(row.get("bound", "")).strip()), f"{name}: decoder bound owner missing")
        add(errors, bool(str(row.get("order", "")).strip()), f"{name}: key/field order missing")
        for reference in row.get("references", []):
            add(
                errors,
                reference in schema_set,
                f"{name}: undefined schema reference {reference}",
            )

    task_spec_snapshot = row_by(schemas, "name", "TaskSpecSnapshotV1")
    if task_spec_snapshot is None:
        errors.append("TaskSpecSnapshotV1 is missing")
    else:
        snapshot_fields = [field[0] for field in task_spec_snapshot.get("fields", [])]
        add(
            errors,
            snapshot_fields
            == [
                "version",
                "producer",
                "class",
                "priority",
                "cancel_scope",
                "policy",
                "outcome",
                "execution",
                "debug",
            ],
            "TaskSpecSnapshotV1 must contain exactly one execution row",
        )
        add(
            errors,
            not ({"request", "host_request", "runtime_request"} & set(snapshot_fields)),
            "TaskSpecSnapshotV1 contains unconditional/parallel request fields",
        )
    task_execution_snapshot = row_by(schemas, "name", "TaskExecutionSnapshotV1")
    if task_execution_snapshot is not None:
        add(
            errors,
            [row[0] for row in task_execution_snapshot.get("variants", [])]
            == ["Host", "Runtime"],
            "TaskExecutionSnapshotV1 variants changed",
        )
    runtime_request_snapshot = row_by(schemas, "name", "RuntimeTaskRequestSnapshotV1")
    if runtime_request_snapshot is not None:
        add(
            errors,
            [row[0] for row in runtime_request_snapshot.get("variants", [])]
            == ["AwaitManyAggregate", "Timeout"],
            "RuntimeTaskRequestSnapshotV1 variants changed",
        )

    # Exhaustive Match substrate.
    expressions = data["expressions"]
    patterns = data["patterns"]
    add(
        errors,
        variant_names(expressions.get("checked_expression_resolution", []))
        == EXPECTED_EXPRESSION_RESOLUTIONS,
        "CheckedExpressionResolution transcript inventory is not exhaustive",
    )
    add(
        errors,
        variant_names(expressions.get("checked_value_resolution", []))
        == EXPECTED_VALUE_RESOLUTIONS,
        "CheckedValueResolution transcript inventory is not exhaustive",
    )
    add(
        errors,
        variant_names(expressions.get("checked_select_resolution", []))
        == EXPECTED_SELECT_RESOLUTIONS,
        "CheckedSelectResolution transcript inventory is not exhaustive",
    )
    add(
        errors,
        variant_names(patterns.get("checked_pattern_resolution", []))
        == EXPECTED_PATTERN_RESOLUTIONS,
        "CheckedPatternResolution transcript inventory is not exhaustive",
    )
    add(
        errors,
        variant_names(patterns.get("hir_pattern_families", []))
        == EXPECTED_HIR_PATTERN_FAMILIES,
        "HirPatternKind transcript inventory is not exhaustive",
    )
    add(
        errors,
        variant_names(expressions.get("literals", []))
        == EXPECTED_LITERAL_FAMILIES,
        "HirLiteral transcript inventory is not exhaustive",
    )
    for inventory_name, rows in [
        ("CheckedExpressionResolution", expressions.get("checked_expression_resolution", [])),
        ("CheckedValueResolution", expressions.get("checked_value_resolution", [])),
        ("CheckedSelectResolution", expressions.get("checked_select_resolution", [])),
        ("CheckedPatternResolution", patterns.get("checked_pattern_resolution", [])),
        ("HirPatternKind", patterns.get("hir_pattern_families", [])),
    ]:
        for row in rows:
            add(
                errors,
                bool(str(row.get("transcript", "")).strip()),
                f"{inventory_name}::{row.get('variant')}: transcript is undefined",
            )
    for row in expressions.get("literals", []):
        for key in ("accepted_payload", "transcript", "excluded", "invalid"):
            add(
                errors,
                bool(str(row.get(key, "")).strip()),
                f"HirLiteral::{row.get('variant')}: {key} is undefined",
            )

    # Carrier-backed TypeKind matrix.
    ownership = data["ownership"]
    ownership_names = [row.get("variant") for row in ownership]
    add(
        errors,
        ownership_names == EXPECTED_TYPE_KINDS,
        "ownership matrix must contain exactly the 85 current TypeKind variants",
    )
    add(errors, unique(ownership_names), "ownership TypeKind rows must be unique")
    for row in ownership:
        variant = row.get("variant")
        disposition = row.get("disposition")
        add(
            errors,
            disposition
            in {"Copy", "SnapshotClone", "Reject", "Delegate", "RejectAtTypeLevel"},
            f"{variant}: unknown ownership disposition",
        )
        if disposition == "SnapshotClone":
            for column in (
                "runtime_projection",
                "live_carrier",
                "canonical_identity",
                "snapshot_codec",
            ):
                add(
                    errors,
                    bool(str(row.get(column, "")).strip()),
                    f"{variant}: SnapshotClone lacks {column}",
                )
        if disposition in {"Reject", "RejectAtTypeLevel"}:
            add(
                errors,
                bool(str(row.get("rejection", "")).strip()),
                f"{variant}: rejected row lacks a typed reason",
            )
    predicate = row_by(ownership, "variant", "Predicate")
    shared = row_by(ownership, "variant", "Shared")
    add(
        errors,
        predicate is not None and predicate.get("recursion") == "none",
        "Predicate TypeKind row must have no child recursion",
    )
    add(
        errors,
        shared is not None
        and shared.get("disposition") == "Reject"
        and shared.get("rejection") == "MissingRuntimeSnapshotOwner"
        and not shared.get("live_carrier")
        and not shared.get("canonical_identity")
        and not shared.get("snapshot_codec"),
        "Shared must reject as MissingRuntimeSnapshotOwner without invented carrier evidence",
    )

    # Exact five compile-clean cuts.
    cuts = data["cuts"]
    cut_numbers = [row.get("cut") for row in cuts]
    add(errors, cut_numbers == [1, 2, 3, 4, 5], "compile cuts must be exactly ordered 1 through 5")
    for row in cuts:
        cut = row.get("cut")
        add(errors, row.get("compile_clean") is True, f"cut {cut} must be compile-clean")
        add(errors, bool(row.get("crates")), f"cut {cut} must name exact crates")
        add(errors, bool(row.get("gates")), f"cut {cut} must name build/test gates")
        add(
            errors,
            all(int(dep) < int(cut) for dep in row.get("depends_on_cuts", [])),
            f"cut {cut} has a forward or self dependency",
        )
    cut3 = row_by(cuts, "cut", 3)
    cut4 = row_by(cuts, "cut", 4)
    if cut3 is None:
        errors.append("cut 3 is missing")
    else:
        bad = set(cut3.get("type_dependencies", [])) & CUT4_TYPES_FORBIDDEN_IN_CUT3
        add(
            errors,
            not bad,
            f"cut 3 depends on cut 4 task types: {sorted(bad)}",
        )
        add(
            errors,
            not (set(cut3.get("publishes", [])) & CUT4_TYPES_FORBIDDEN_IN_CUT3),
            "cut 3 publishes a cut 4 task type",
        )
    if cut4 is None:
        errors.append("cut 4 is missing")
    else:
        published = set(cut4.get("publishes", []))
        add(
            errors,
            "RuntimeValue::NeedHandle" not in published
            and "public RuntimeValue::NeedHandle" not in published,
            "cut 4 claims a private/public RuntimeValue::NeedHandle variant",
        )
        add(
            errors,
            "public RuntimeValue::NeedHandle" in set(cut4.get("forbidden", [])),
            "cut 4 must explicitly forbid public RuntimeValue::NeedHandle",
        )

    # Current path deletion matrix and source evidence.
    deletions = data["deletions"]
    add(errors, len(deletions) >= 40, "deletion matrix is not current-tree exhaustive")
    deletion_keys = [(row.get("path"), row.get("old_authority")) for row in deletions]
    add(errors, unique(deletion_keys), "deletion matrix contains duplicate authority rows")
    for row in deletions:
        path = str(row.get("path", ""))
        add(errors, path.startswith("crates/"), f"deletion path is not a real crate path: {path}")
        add(
            errors,
            not any(path.startswith(prefix) for prefix in FORBIDDEN_NONEXISTENT_CRATE_TARGETS),
            f"deletion matrix names nonexistent implementation target: {path}",
        )
        add(errors, str(row.get("cut")) in {"1", "2", "3", "4", "5", "4/5", "1/3", "2/5", "3/5"}, f"{path}: invalid deletion cut")
        add(errors, bool(str(row.get("proof", "")).strip()), f"{path}: deletion proof missing")

    source = data["source"]
    add(errors, source.get("repository") == "Sanzentyo/arcweft", "source evidence repository mismatch")
    add(errors, source.get("inspected_main") == EXPECTED_MAIN, "source evidence main SHA mismatch")
    source_rows = source.get("rows", [])
    add(errors, len(source_rows) >= 25, "source evidence is insufficient")
    add(
        errors,
        unique([row.get("path") for row in source_rows]),
        "source evidence paths must be unique",
    )
    required_source_paths = {
        "AGENTS.md",
        "crates/AGENTS.md",
        "docs/AGENTS.md",
        "docs/reviews/AGENTS.md",
        "docs/implementation/AGENTS.md",
        "crates/arcweft-core/src/task.rs",
        "crates/arcweft-core/src/value.rs",
        "crates/arcweft-core/src/entry/schema.rs",
        "crates/arcweft-runtime-scheduler/src/lib.rs",
        "crates/arcweft-runtime-driver/src/task.rs",
        "crates/arcweft-lang-sema/src/final_analysis/model.rs",
        "crates/arcweft-lang-sema/src/types.rs",
    }
    add(
        errors,
        required_source_paths <= {row.get("path") for row in source_rows},
        "source evidence omits a required current owner",
    )

    # Test and traceability closure.
    tests = data["tests"]
    test_ids = [row.get("id") for row in tests]
    add(errors, len(tests) >= 90, "test matrix must contain at least 90 focused/property/tamper rows")
    add(errors, unique(test_ids), "test IDs must be unique")
    allowed_categories = {
        "borrow-flow", "compile-clean", "dependency", "differential",
        "exhaustive", "focused", "migration", "negative", "property",
        "replacement", "replay", "rollback", "snapshot",
        "structural-absence", "tamper", "validator-negative",
    }
    for row in tests:
        add(
            errors,
            row.get("category") in allowed_categories,
            f"{row.get('id')}: unknown test category {row.get('category')!r}",
        )
        add(
            errors,
            row.get("kind") in {"implementation", "package"},
            f"{row.get('id')}: test kind must be implementation or package",
        )
        add(errors, bool(str(row.get("assertion", "")).strip()), f"{row.get('id')}: empty assertion")
    traceability = data["traceability"]
    add(errors, len(traceability) == 10, "traceability must map all ten mandatory corrections")
    all_test_ids = set(test_ids)
    for row in traceability:
        refs = row.get("tests", [])
        add(errors, bool(refs), f"{row.get('requirement')}: no test rows")
        add(
            errors,
            set(refs) <= all_test_ids,
            f"{row.get('requirement')}: traceability references unknown tests",
        )
        add(errors, bool(row.get("docs")), f"{row.get('requirement')}: no prose mapping")
        add(errors, bool(row.get("machine")), f"{row.get('requirement')}: no machine mapping")

    owner_api = data["owner_api"]
    add(errors, len(owner_api) >= 12, "owner/API map is incomplete")
    add(
        errors,
        unique([row.get("owner") for row in owner_api]),
        "owner/API map contains duplicate owners",
    )
    scheduler_api = row_by(
        owner_api,
        "owner",
        "arcweft_runtime_scheduler::RuntimeTaskScheduler<A>",
    )
    add(
        errors,
        scheduler_api is not None,
        "owner/API map omits RuntimeTaskScheduler<A>",
    )

    return sorted(set(errors))


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def git_blob_sha(path: Path) -> str:
    data = path.read_bytes()
    header = f"blob {len(data)}\0".encode("ascii")
    return hashlib.sha1(header + data).hexdigest()


def validate_required_files(package_root: Path) -> list[str]:
    errors: list[str] = []
    for relative in REQUIRED_FILES:
        path = package_root / relative
        add(errors, path.is_file(), f"required file missing: {relative}")
        if path.is_file():
            add(errors, path.stat().st_size > 0, f"required file is empty: {relative}")

    for path in package_root.rglob("*"):
        if path.is_symlink():
            errors.append(f"symlink is forbidden in throwable package: {path.relative_to(package_root)}")
        if path.is_file():
            relative = path.relative_to(package_root)
            add(errors, ".." not in relative.parts, f"unsafe package path: {relative}")

    final_status = package_root / "FINAL_STATUS.md"
    if final_status.is_file():
        text = final_status.read_text(encoding="utf-8")
        add(errors, "READY_FOR_IMPLEMENTATION" in text, "FINAL_STATUS lacks readiness marker")
        add(errors, "OPEN_QUESTIONS=0" in text, "FINAL_STATUS lacks OPEN_QUESTIONS=0")
    open_questions = package_root / "OPEN_QUESTIONS.md"
    if open_questions.is_file():
        text = open_questions.read_text(encoding="utf-8")
        add(errors, "OPEN_QUESTIONS=0" in text, "OPEN_QUESTIONS.md must close all choices")

    for relative, expected in EXPECTED_INPUT_HASHES.items():
        path = package_root / relative
        if path.is_file():
            add(
                errors,
                sha256_file(path) == expected,
                f"retained input hash mismatch: {relative}",
            )
    request_path = package_root / "inputs/CURRENT_REQUEST.md"
    if request_path.is_file():
        add(
            errors,
            git_blob_sha(request_path) == EXPECTED_REQUEST_BLOB,
            "retained request is not byte-equal to the inspected repository blob",
        )
    return sorted(set(errors))


def csv_cell(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, list):
        return json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    if isinstance(value, dict):
        return json.dumps(
            value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
        )
    return str(value)


def validate_csv(
    package_root: Path,
    relative: str,
    fieldnames: list[str],
    expected_rows: list[dict[str, Any]],
) -> list[str]:
    path = package_root / relative
    if not path.is_file():
        return [f"CSV missing: {relative}"]
    try:
        with path.open(newline="", encoding="utf-8") as source:
            reader = csv.DictReader(source)
            actual_header = reader.fieldnames or []
            actual_rows = list(reader)
    except (OSError, csv.Error) as error:
        return [f"CSV read failure {relative}: {error}"]

    errors: list[str] = []
    add(errors, actual_header == fieldnames, f"{relative}: CSV header mismatch")
    normalized = [
        {field: csv_cell(row.get(field, "")) for field in fieldnames}
        for row in expected_rows
    ]
    add(errors, actual_rows == normalized, f"{relative}: CSV rows differ from machine JSON")
    return errors


def validate_csv_equivalence(package_root: Path, data: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    errors += validate_csv(
        package_root,
        "tables/producer_execution_truth_table.csv",
        ["family", "allowed_execution", "allowed_policies", "restriction"],
        data["families"],
    )
    errors += validate_csv(
        package_root,
        "tables/ownership_matrix.csv",
        [
            "variant", "disposition", "recursion", "runtime_projection",
            "live_carrier", "canonical_identity", "snapshot_codec", "rejection",
            "notes",
        ],
        data["ownership"],
    )
    errors += validate_csv(
        package_root,
        "tables/persistence_schemas.csv",
        [
            "name", "kind", "versioned", "version", "fields", "variants", "key",
            "order", "bound", "invariants", "references", "unknown_fields",
            "duplicate_fields", "trailing_bytes",
        ],
        data["schemas"],
    )
    expression_rows: list[dict[str, Any]] = []
    for inventory, key in [
        ("CheckedExpressionResolution", "checked_expression_resolution"),
        ("CheckedValueResolution", "checked_value_resolution"),
        ("CheckedSelectResolution", "checked_select_resolution"),
    ]:
        expression_rows.extend(
            {"inventory": inventory, **row}
            for row in data["expressions"][key]
        )
    errors += validate_csv(
        package_root,
        "tables/expression_transcripts.csv",
        ["inventory", "variant", "transcript"],
        expression_rows,
    )
    literal_rows = [
        {"inventory": "HirLiteral", **row}
        for row in data["expressions"]["literals"]
    ]
    errors += validate_csv(
        package_root,
        "tables/literal_transcripts.csv",
        [
            "inventory", "variant", "accepted_payload", "transcript",
            "excluded", "invalid",
        ],
        literal_rows,
    )
    pattern_rows: list[dict[str, Any]] = []
    for inventory, key in [
        ("CheckedPatternResolution", "checked_pattern_resolution"),
        ("HirPatternKind", "hir_pattern_families"),
    ]:
        pattern_rows.extend(
            {"inventory": inventory, **row}
            for row in data["patterns"][key]
        )
    errors += validate_csv(
        package_root,
        "tables/pattern_transcripts.csv",
        ["inventory", "variant", "transcript"],
        pattern_rows,
    )
    errors += validate_csv(
        package_root,
        "tables/deletion_matrix.csv",
        ["path", "old_authority", "final_action", "cut", "proof"],
        data["deletions"],
    )
    errors += validate_csv(
        package_root,
        "tables/tests.csv",
        ["id", "category", "owner", "assertion", "kind", "cut"],
        data["tests"],
    )
    errors += validate_csv(
        package_root,
        "tables/source_evidence.csv",
        ["path", "blob", "owner", "observation", "verification"],
        data["source"]["rows"],
    )
    errors += validate_csv(
        package_root,
        "tables/compile_cuts.csv",
        [
            "cut", "name", "crates", "feature_gate", "publishes",
            "depends_on_cuts", "type_dependencies", "forbidden", "deletes",
            "gates", "compile_clean",
        ],
        data["cuts"],
    )
    errors += validate_csv(
        package_root,
        "tables/owner_api_map.csv",
        ["owner", "path", "api", "borrowing", "cut", "dependencies", "notes"],
        data["owner_api"],
    )
    errors += validate_csv(
        package_root,
        "tables/requirement_traceability.csv",
        ["requirement", "decision", "docs", "machine", "tests"],
        data["traceability"],
    )
    return sorted(set(errors))


def validate_normative_prose(package_root: Path) -> list[str]:
    errors: list[str] = []
    normative = [
        "FINAL_CONTRACT.md",
        "DECISION_REGISTER.md",
        "RUST_SCHEMAS.md",
        "SCHEDULER_OWNER_AND_API.md",
        "PERSISTENCE_AND_REPLAY.md",
        "MATCH_SEMANTIC_TRANSCRIPTS.md",
        "VIEW_BUNDLE_PROJECTION.md",
        "OWNERSHIP_MATRIX.md",
        "COMPILE_CLEAN_SEQUENCE.md",
        "STRUCTURAL_ABSENCE.md",
    ]
    for relative in normative:
        path = package_root / relative
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        add(errors, "\x00" not in text, f"{relative}: embedded NUL")
        for token in ("TODO", "TBD"):
            add(errors, token not in text, f"{relative}: unresolved {token}")
    schema_text = (package_root / "RUST_SCHEMAS.md").read_text(
        encoding="utf-8"
    ) if (package_root / "RUST_SCHEMAS.md").is_file() else ""
    add(
        errors,
        "pub execution: TaskExecution" in schema_text,
        "RUST_SCHEMAS.md lacks TaskSpec.execution",
    )
    task_spec_start = schema_text.find("pub struct TaskSpec {")
    task_spec_end = schema_text.find("\n}", task_spec_start)
    task_spec_block = (
        schema_text[task_spec_start:task_spec_end]
        if task_spec_start >= 0 and task_spec_end > task_spec_start
        else ""
    )
    add(
        errors,
        "HostTaskRequest" not in task_spec_block
        and "request:" not in task_spec_block,
        "RUST_SCHEMAS.md reintroduces unconditional TaskSpec HostTaskRequest",
    )
    return sorted(set(errors))


def validate_manifest(package_root: Path) -> list[str]:
    manifest_path = package_root / "MANIFEST.json"
    sidecar_path = package_root / "MANIFEST.sha256"
    if not manifest_path.is_file():
        return ["MANIFEST.json missing"]
    if not sidecar_path.is_file():
        return ["MANIFEST.sha256 missing"]
    errors: list[str] = []
    manifest = load_json(manifest_path)
    add(errors, manifest.get("manifest_version") == 1, "manifest version must be one")
    add(errors, manifest.get("package") == PACKAGE_NAME, "manifest package identity mismatch")
    add(errors, manifest.get("hash_algorithm") == "sha256", "manifest hash algorithm mismatch")
    add(errors, manifest.get("inspected_main") == EXPECTED_MAIN, "manifest inspected SHA mismatch")
    excluded = manifest.get("excluded_from_file_entries")
    add(
        errors,
        excluded == ["MANIFEST.json", "MANIFEST.sha256"],
        "manifest exclusion set mismatch",
    )
    entries = manifest.get("files", [])
    entry_paths = [entry.get("path") for entry in entries]
    add(errors, unique(entry_paths), "manifest paths must be unique")
    actual_files = sorted(
        str(path.relative_to(package_root)).replace(os.sep, "/")
        for path in package_root.rglob("*")
        if path.is_file()
        and str(path.relative_to(package_root)).replace(os.sep, "/")
        not in {"MANIFEST.json", "MANIFEST.sha256"}
    )
    add(errors, entry_paths == actual_files, "manifest file set does not match package content")
    add(errors, manifest.get("file_count") == len(entries), "manifest file_count mismatch")
    for entry in entries:
        relative = str(entry.get("path", ""))
        path = package_root / relative
        if not path.is_file():
            errors.append(f"manifest entry missing file: {relative}")
            continue
        add(errors, entry.get("bytes") == path.stat().st_size, f"manifest byte count mismatch: {relative}")
        add(errors, entry.get("sha256") == sha256_file(path), f"manifest SHA-256 mismatch: {relative}")
    expected_manifest_hash = sha256_file(manifest_path)
    sidecar = sidecar_path.read_text(encoding="ascii").strip().split()
    add(
        errors,
        len(sidecar) == 2
        and sidecar[0] == expected_manifest_hash
        and sidecar[1] == "MANIFEST.json",
        "MANIFEST.sha256 does not protect MANIFEST.json",
    )
    return sorted(set(errors))


def validate_package(package_root: Path, *, check_manifest: bool) -> tuple[dict[str, Any], list[str]]:
    data = load_machine_data(package_root)
    errors: list[str] = []
    errors += validate_required_files(package_root)
    errors += validate_contract_data(data)
    errors += validate_csv_equivalence(package_root, data)
    errors += validate_normative_prose(package_root)
    if check_manifest:
        errors += validate_manifest(package_root)
    return data, sorted(set(errors))


def negative_self_tests(base_data: dict[str, Any]) -> list[tuple[str, bool, str]]:
    cases: list[tuple[str, str, Callable[[dict[str, Any]], None]]] = []

    def register(
        name: str,
        expected_fragment: str,
        mutate: Callable[[dict[str, Any]], None],
    ) -> None:
        cases.append((name, expected_fragment, mutate))

    register(
        "adapter_commit",
        "AdapterCommit must not be a reachable error variant",
        lambda data: data["contract"]["adapter"]["error_variants"].append("AdapterCommit"),
    )
    register(
        "unconditional_host_request",
        "TaskSpec fields must contain exactly one execution field",
        lambda data: data["contract"]["task_spec"]["fields"].append("request"),
    )
    register(
        "undefined_snapshot",
        "undefined schema reference UndefinedSnapshotV1",
        lambda data: data["schemas"][0]["references"].append("UndefinedSnapshotV1"),
    )
    register(
        "compiler_local_bundle_id",
        "AcceptedViewMatchBundleRowV1 field set/order mismatch",
        lambda data: data["contract"]["bundle"]["fields"].append("ExprId"),
    )

    def shared_without_carrier(data: dict[str, Any]) -> None:
        shared = row_by(data["ownership"], "variant", "Shared")
        assert shared is not None
        shared["disposition"] = "SnapshotClone"
        shared["live_carrier"] = "InventedSharedCarrier"
        shared["canonical_identity"] = "InventedSharedIdentity"
        shared["snapshot_codec"] = "InventedSharedSnapshotV1"
        shared["rejection"] = ""

    register(
        "shared_without_carrier",
        "Shared must reject as MissingRuntimeSnapshotOwner",
        shared_without_carrier,
    )

    def predicate_recursion(data: dict[str, Any]) -> None:
        predicate = row_by(data["ownership"], "variant", "Predicate")
        assert predicate is not None
        predicate["recursion"] = "classify child"

    register(
        "predicate_recursion",
        "Predicate TypeKind row must have no child recursion",
        predicate_recursion,
    )

    def private_variant(data: dict[str, Any]) -> None:
        cut4 = row_by(data["cuts"], "cut", 4)
        assert cut4 is not None
        cut4["publishes"].append("RuntimeValue::NeedHandle")

    register(
        "private_runtime_value_variant",
        "cut 4 claims a private/public RuntimeValue::NeedHandle variant",
        private_variant,
    )

    def cut3_dependency(data: dict[str, Any]) -> None:
        cut3 = row_by(data["cuts"], "cut", 3)
        assert cut3 is not None
        cut3["type_dependencies"].append("NeedProducerContractDigest")

    register(
        "cut3_depends_on_cut4",
        "cut 3 depends on cut 4 task types",
        cut3_dependency,
    )
    register(
        "missing_producer_family",
        "producer family inventory/order must contain exactly the nine current families",
        lambda data: data["families"].pop(),
    )
    register(
        "version_two",
        "all Arcweft-owned version markers must be exactly [1]",
        lambda data: data["contract"].__setitem__("all_arcweft_version_markers", [1, 2]),
    )

    results: list[tuple[str, bool, str]] = []
    for name, expected_fragment, mutate in cases:
        candidate = copy.deepcopy(base_data)
        try:
            mutate(candidate)
        except Exception as error:  # mutation harness failure is visible
            results.append((name, False, f"mutation failed: {error}"))
            continue
        errors = validate_contract_data(candidate)
        match = next((error for error in errors if expected_fragment in error), None)
        if match is None:
            details = "; ".join(errors[:4]) if errors else "corruption was accepted"
            results.append((name, False, details))
        else:
            results.append((name, True, match))
    return results


def format_output(
    package_root: Path,
    errors: list[str],
    data: dict[str, Any] | None,
    self_tests: list[tuple[str, bool, str]],
    check_manifest: bool,
) -> str:
    lines = [
        f"PACKAGE={package_root.name}",
        f"MANIFEST_CHECK={'enabled' if check_manifest else 'skipped'}",
    ]
    if data is not None:
        lines.extend(
            [
                f"PRODUCER_FAMILIES={len(data['families'])}",
                f"PERSISTENCE_SCHEMAS={len(data['schemas'])}",
                f"TYPE_KIND_ROWS={len(data['ownership'])}",
                f"TEST_ROWS={len(data['tests'])}",
            ]
        )
    for name, passed, detail in self_tests:
        lines.append(f"NEGATIVE_SELF_TEST {name}={'PASS' if passed else 'FAIL'} :: {detail}")
    for error in errors:
        lines.append(f"ERROR :: {error}")
    all_self_pass = all(passed for _, passed, _ in self_tests)
    passed = not errors and all_self_pass
    lines.append(f"RESULT={'PASS' if passed else 'FAIL'}")
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--package-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="root directory of the extracted final-contract package",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run ten in-memory negative mutation tests",
    )
    parser.add_argument(
        "--skip-manifest",
        action="store_true",
        help="skip MANIFEST.json validation while assembling a package",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    package_root = args.package_root.resolve()
    if not package_root.is_dir():
        print(f"package root is not a directory: {package_root}", file=sys.stderr)
        return 2
    if package_root.name != PACKAGE_NAME:
        print(
            f"package root name mismatch: expected {PACKAGE_NAME}, got {package_root.name}",
            file=sys.stderr,
        )
        return 2

    try:
        data, errors = validate_package(
            package_root,
            check_manifest=not args.skip_manifest,
        )
    except PackageValidationFailure as error:
        print(f"ERROR :: {error}", file=sys.stderr)
        return 2

    self_tests = negative_self_tests(data) if args.self_test else []
    output = format_output(
        package_root,
        errors,
        data,
        self_tests,
        check_manifest=not args.skip_manifest,
    )
    sys.stdout.write(output)
    return 0 if not errors and all(passed for _, passed, _ in self_tests) else 1


if __name__ == "__main__":
    raise SystemExit(main())
