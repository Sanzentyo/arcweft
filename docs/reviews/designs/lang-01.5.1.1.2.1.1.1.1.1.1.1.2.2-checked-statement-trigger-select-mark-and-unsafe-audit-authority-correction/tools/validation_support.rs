use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

pub const DESIGN_REL: &str = "docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2.2-checked-statement-trigger-select-mark-and-unsafe-audit-authority-correction";

const STATEMENTS: &[(&str, &str, &str)] = &[
    ("Assertion", "Assertion", "CheckedAssertionDisposition"),
    ("Let", "Structural", "typed children"),
    ("Assign", "Assignment", "CheckedAssignment"),
    ("LetElse", "Structural", "typed children and body"),
    ("LetChoice", "Structural", "typed children"),
    ("LetScope", "Structural", "typed children"),
    ("LetActionReceive", "Structural", "typed children"),
    ("Return", "Structural", "typed child and function contract"),
    ("Out", "ControlTransfer", "Output"),
    ("Goto", "Structural", "typed target fact"),
    ("DeferBlock", "Defer", "DeferOutcome"),
    ("Defer", "Defer", "DeferOutcome"),
    ("Yield", "Yield", "consumed StreamFactory item proof"),
    ("Signal", "Structural", "typed children"),
    ("LifetimeSet", "Structural", "typed children"),
    (
        "Wait",
        "Suspension",
        "CheckedSuspensionStatement; mark wait rejects",
    ),
    ("On", "Trigger", "CheckedTrigger"),
    ("UnsafeLifetime", "UnsafeAudit", "CheckedUnsafeAudit"),
    ("Choice", "Structural", "typed children and body"),
    ("If", "Structural", "typed condition and bodies"),
    ("IfLet", "Structural", "typed pattern and bodies"),
    ("Match", "Structural", "generic Match authority"),
    ("While", "Structural", "typed condition and body"),
    ("WhileLet", "Structural", "typed pattern and body"),
    ("For", "Iteration", "CheckedIteration"),
    ("Close", "Structural", "typed child"),
    ("Select", "Select", "CheckedSelectStatement"),
    ("SourceLocale", "SourceLocale", "LocaleTag"),
    ("Scope", "Scope", "CheckedScopeIdentity"),
    ("Include", "Include", "CallableDeclarationDigest"),
    ("Break", "ControlTransfer", "Loop"),
    ("Continue", "ControlTransfer", "Loop"),
    (
        "Expression",
        "EvaluatedEffect|Structural",
        "exact sealed effect or structural",
    ),
    ("ProofCall", "Structural", "typed call facts"),
    ("Error", "Reject", "sole rejection family"),
];

const TRIGGERS: &[&str] = &[
    "Input",
    "Event",
    "Signal",
    "Timeout",
    "Mark",
    "Select",
    "Task",
    "Scope",
    "Expression",
];
const SELECT_STATEMENTS: &[&str] = &["Operand", "Branches"];
const SELECT_HEADS: &[&str] = &["Bind", "Frame", "Event"];
const PAYLOADS: &[&str] = &[
    "Structural",
    "Assignment",
    "Assertion",
    "Defer",
    "EvaluatedEffect",
    "Iteration",
    "ControlTransfer",
    "Trigger",
    "UnsafeAudit",
    "Select",
    "SourceLocale",
    "Scope",
    "Include",
    "Suspension",
    "Yield",
];
const SCRUTINEE_ROLES: &[&str] = &[
    "TriggerInput",
    "TriggerEvent",
    "TriggerSignal",
    "TriggerSelect",
    "TriggerTask",
    "TriggerScope",
    "SelectFrame",
    "SelectEvent",
];
const DELETION_ORDER: &[&str] = &[
    "prerequisite authority freeze",
    "syntax selector and prefix-Try-only propagation",
    "HIR mark Trigger Select unsafe replacement",
    "registration preparation coordinates specialized checked rows",
    "complete checked statement payload replacement",
    "rich-text compiler runtime-plan verifier tooling migration",
    "mandatory obsolete authority deletion and repository validation",
    "statement body rich-text transcript and generic-Match closure",
];
const MANDATORY_GATES: &[&str] = &[
    "terminal_status",
    "open_questions",
    "request_mirror",
    "manifest",
    "version_one",
    "required_files",
    "precedence_inventory",
    "syntax_contract",
    "hir_contract",
    "ingress_contract",
    "scrutinee_contract",
    "mark_coordinate",
    "checked_tags",
    "statement_matrix",
    "transcript_contract",
    "wait_mark_policy",
    "deletion_order",
    "forbidden_authority",
    "source_inventory",
    "dependency_direction",
];

#[derive(Clone, Debug)]
pub struct Bundle {
    pub root: PathBuf,
    pub repo: PathBuf,
    pub files: BTreeMap<String, Vec<u8>>,
    pub contract: Value,
    pub inventory: Value,
    pub corpus: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateError {
    pub code: &'static str,
    pub detail: String,
}

impl GateError {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl Display for GateError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for GateError {}

type GateResult<T = ()> = Result<T, GateError>;

pub fn load_bundle(root: &Path) -> GateResult<Bundle> {
    let root = root
        .canonicalize()
        .map_err(|error| GateError::new("required_files", error.to_string()))?;
    let repo = root
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .map(Path::to_path_buf)
        .ok_or_else(|| GateError::new("required_files", "cannot locate .git ancestor"))?;
    let files = WalkDir::new(&root)
        .into_iter()
        .map(|entry| entry.map_err(|error| GateError::new("required_files", error.to_string())))
        .filter_map(|result| match result {
            Ok(entry) if entry.file_type().is_file() => Some(Ok(entry)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .map(|entry| {
            let entry = entry?;
            let relative = normalized_relative(&root, entry.path())?;
            let bytes = fs::read(entry.path())
                .map_err(|error| GateError::new("required_files", error.to_string()))?;
            Ok((relative, bytes))
        })
        .collect::<GateResult<BTreeMap<_, _>>>()?;
    let contract = parse_json_file(&files, "machine/final_contract.json")?;
    let inventory = parse_json_file(&files, "machine/source_inventory.json")?;
    let corpus = parse_json_file(&files, "machine/negative_corpus.json")?;
    Ok(Bundle {
        root,
        repo,
        files,
        contract,
        inventory,
        corpus,
    })
}

pub fn validate_semantic(bundle: &Bundle) -> GateResult {
    require_file_bytes(
        bundle,
        "FINAL_STATUS.md",
        b"READY_FOR_IMPLEMENTATION\n",
        "terminal_status",
    )?;
    require_str(
        &bundle.contract,
        "/status",
        "READY_FOR_IMPLEMENTATION",
        "terminal_status",
    )?;
    require_file_bytes(bundle, "OPEN_QUESTIONS.md", b"none\n", "open_questions")?;
    require_str(
        &bundle.contract,
        "/open_questions",
        "none",
        "open_questions",
    )?;
    require_u64(&bundle.contract, "/contract_version", 1, "version_one")?;
    require_u64(&bundle.inventory, "/inventory_version", 1, "version_one")?;
    require_u64(&bundle.corpus, "/corpus_version", 1, "version_one")?;
    validate_required_files(bundle)?;
    validate_request(bundle)?;
    validate_precedence(bundle)?;
    validate_syntax_hir(bundle)?;
    validate_ingress(bundle)?;
    validate_scrutinee(bundle)?;
    validate_mark(bundle)?;
    validate_checked_tags(bundle)?;
    validate_statement_matrix(bundle)?;
    validate_transcript(bundle)?;
    validate_wait_mark(bundle)?;
    require_string_array(
        &bundle.contract,
        "/deletion_order",
        DELETION_ORDER,
        "deletion_order",
    )?;
    validate_prohibitions(bundle)?;
    require_string_array(
        &bundle.corpus,
        "/mandatory_gates",
        MANDATORY_GATES,
        "required_files",
    )?;
    validate_doc_anchors(bundle)
}

fn validate_required_files(bundle: &Bundle) -> GateResult {
    let required = bundle
        .contract
        .pointer("/required_files")
        .and_then(Value::as_array)
        .ok_or_else(|| GateError::new("required_files", "required_files missing"))?;
    let names = required
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| GateError::new("required_files", "non-string required file"))
        })
        .collect::<GateResult<BTreeSet<_>>>()?;
    if names.len() != required.len() {
        return Err(GateError::new("required_files", "duplicate required file"));
    }
    names.into_iter().try_for_each(|name| {
        if bundle.files.contains_key(&name) {
            Ok(())
        } else {
            Err(GateError::new("required_files", format!("missing {name}")))
        }
    })
}

fn validate_request(bundle: &Bundle) -> GateResult {
    let source = require_value_str(&bundle.contract, "/request/source", "request_mirror")?;
    require_str(
        &bundle.contract,
        "/request/mirror",
        "REQUEST.md",
        "request_mirror",
    )?;
    let expected_bytes = require_value_u64(&bundle.contract, "/request/bytes", "request_mirror")?;
    let expected_hash = require_value_str(&bundle.contract, "/request/sha256", "request_mirror")?;
    let request = fs::read(bundle.repo.join(source))
        .map_err(|error| GateError::new("request_mirror", error.to_string()))?;
    let mirror = bundle
        .files
        .get("REQUEST.md")
        .ok_or_else(|| GateError::new("request_mirror", "REQUEST.md missing"))?;
    if request != *mirror {
        return Err(GateError::new(
            "request_mirror",
            "maintained request and mirror differ",
        ));
    }
    if u64::try_from(mirror.len()).ok() != Some(expected_bytes) {
        return Err(GateError::new(
            "request_mirror",
            "request byte count differs",
        ));
    }
    if sha256(mirror) != expected_hash {
        return Err(GateError::new("request_mirror", "request SHA-256 differs"));
    }
    Ok(())
}

fn validate_precedence(bundle: &Bundle) -> GateResult {
    require_str(
        &bundle.contract,
        "/precedence/normative_file",
        "FINAL_DESIGN.md",
        "precedence_inventory",
    )?;
    require_bool(
        &bundle.contract,
        "/precedence/amends_generic_statement_and_body_grammar_only",
        true,
        "precedence_inventory",
    )?;
    require_u64(
        &bundle.contract,
        "/precedence/checked_select_resolution_current_count",
        5,
        "precedence_inventory",
    )?;
    require_string_array(
        &bundle.contract,
        "/precedence/checked_select_resolution_current",
        &[
            "Method",
            "DialogueView",
            "AgentField",
            "ProgressField",
            "Field",
        ],
        "precedence_inventory",
    )?;
    require_u64(
        &bundle.contract,
        "/precedence/view_specified_value_current_count",
        26,
        "precedence_inventory",
    )?;
    for key in [
        "stale_tuple_record_select_tags_reserved",
        "stale_inventories_imported",
    ] {
        let expected = key != "stale_inventories_imported";
        require_bool(
            &bundle.contract,
            &format!("/precedence/{key}"),
            expected,
            "precedence_inventory",
        )?;
    }
    Ok(())
}

fn validate_syntax_hir(bundle: &Bundle) -> GateResult {
    require_str(
        &bundle.contract,
        "/syntax/mark_selector",
        "SyntaxDialogueMarkName",
        "syntax_contract",
    )?;
    require_string_array(
        &bundle.contract,
        "/syntax/accepted_forms",
        &["[mark .name]", "[.name]", "on mark(.name)"],
        "syntax_contract",
    )?;
    for key in ["leading_dot_required", "parse_once"] {
        require_bool(
            &bundle.contract,
            &format!("/syntax/{key}"),
            true,
            "syntax_contract",
        )?;
    }
    require_bool(
        &bundle.contract,
        "/syntax/select_suffix_question_accepted",
        false,
        "syntax_contract",
    )?;
    require_str(
        &bundle.contract,
        "/syntax/select_propagation_owner",
        "HirExprKind::Try",
        "syntax_contract",
    )?;
    require_string_array(
        &bundle.contract,
        "/hir/trigger_variants",
        &[
            "Input",
            "Event",
            "Signal",
            "Timeout",
            "Mark",
            "Select",
            "Task",
            "Scope",
            "Expression",
            "Recovered",
        ],
        "hir_contract",
    )?;
    require_bool(
        &bundle.contract,
        "/hir/mark_has_pattern_child",
        false,
        "hir_contract",
    )?;
    require_string_array(
        &bundle.contract,
        "/hir/select_bind_fields",
        &["binding", "source"],
        "hir_contract",
    )?;
    require_bool(
        &bundle.contract,
        "/hir/select_propagates_error_field",
        false,
        "hir_contract",
    )?;
    require_string_array(
        &bundle.contract,
        "/hir/unsafe_issues",
        &["Missing", "InvalidReference", "NonAbsolute", "WrongFamily"],
        "hir_contract",
    )?;
    require_str(
        &bundle.contract,
        "/hir/choice_context_accessor",
        "HirProjectEvaluationTopology::enclosing_choice_lifecycle",
        "hir_contract",
    )
}

fn validate_ingress(bundle: &Bundle) -> GateResult {
    for (pointer, expected) in [
        (
            "/standard_ingress/owner",
            "RegisteredTypeCheckEnv::statement_ingress",
        ),
        (
            "/standard_ingress/record",
            "RegisteredStatementIngressTypes",
        ),
        (
            "/standard_ingress/type_id",
            "StandardStatementIngressTypeId",
        ),
        ("/standard_ingress/role_id", "StatementIngressTypeRoleId"),
        (
            "/standard_ingress/input",
            "StatementIngressTypePublicationInput",
        ),
        (
            "/standard_ingress/type_kind_variant",
            "TypeKind::StatementIngress",
        ),
        (
            "/standard_ingress/input_type",
            "TypeKind::entity_ref(EntityKind::Input)",
        ),
    ] {
        require_str(&bundle.contract, pointer, expected, "ingress_contract")?;
    }
    require_u64(
        &bundle.contract,
        "/standard_ingress/type_kind_outer_tag",
        88,
        "ingress_contract",
    )?;
    require_str(
        &bundle.contract,
        "/standard_ingress/match_domain",
        "closed opaque atomic exact-ID",
        "ingress_contract",
    )?;
    let publications = bundle
        .contract
        .pointer("/standard_ingress/publications")
        .and_then(Value::as_array)
        .ok_or_else(|| GateError::new("ingress_contract", "publications missing"))?;
    let expected = [
        (
            "Task",
            "TaskEvent",
            "TypeKind::StatementIngress(TaskEvent)",
            0,
        ),
        (
            "Scope",
            "ScopeExit",
            "TypeKind::StatementIngress(ScopeExit)",
            1,
        ),
        (
            "Frame",
            "FrameBoundary",
            "TypeKind::StatementIngress(FrameBoundary)",
            2,
        ),
    ];
    if publications.len() != expected.len() {
        return Err(GateError::new(
            "ingress_contract",
            "publication count differs",
        ));
    }
    for (actual, (role, id, ty, tag)) in publications.iter().zip(expected) {
        if actual["role"].as_str() != Some(role)
            || actual["id"].as_str() != Some(id)
            || actual["type"].as_str() != Some(ty)
            || actual["inner_tag"].as_u64() != Some(tag)
        {
            return Err(GateError::new(
                "ingress_contract",
                format!("invalid publication {actual}"),
            ));
        }
    }
    require_bool(
        &bundle.contract,
        "/standard_ingress/source_backed_extension_allowed",
        false,
        "ingress_contract",
    )?;
    for key in [
        "included_in_registered_environment_digest",
        "missing_duplicate_conflict_reject",
    ] {
        require_bool(
            &bundle.contract,
            &format!("/standard_ingress/{key}"),
            true,
            "ingress_contract",
        )?;
    }
    Ok(())
}

fn validate_scrutinee(bundle: &Bundle) -> GateResult {
    require_str(
        &bundle.contract,
        "/scrutinee/authority",
        "StatementScrutineeTypeAuthority",
        "scrutinee_contract",
    )?;
    require_string_array(
        &bundle.contract,
        "/scrutinee/authority_fields",
        &["standard", "project", "topology", "entries"],
        "scrutinee_contract",
    )?;
    require_string_array(
        &bundle.contract,
        "/scrutinee/roles",
        SCRUTINEE_ROLES,
        "scrutinee_contract",
    )?;
    require_str(
        &bundle.contract,
        "/scrutinee/completion_algorithm",
        "deterministic Entry-seeded declaration worklist with completed-graph recheck",
        "scrutinee_contract",
    )?;
    require_string_array(
        &bundle.contract,
        "/scrutinee/limits",
        &[
            "max_declarations",
            "max_edges",
            "max_entry_contributors",
            "max_contextual_statements",
            "max_work",
        ],
        "scrutinee_contract",
    )?;
    for key in [
        "authority_owns_type_kind",
        "authority_clone",
        "published_context_map",
    ] {
        require_bool(
            &bundle.contract,
            &format!("/scrutinee/{key}"),
            false,
            "scrutinee_contract",
        )?;
    }
    for key in ["event_zero_reject", "event_mismatch_reject"] {
        require_bool(
            &bundle.contract,
            &format!("/scrutinee/{key}"),
            true,
            "scrutinee_contract",
        )?;
    }
    for (pointer, expected) in [
        (
            "/scrutinee/event_type_accessor",
            "PreparedEntrySemanticAuthority::ty(TypeId)",
        ),
        (
            "/scrutinee/event_final_seal",
            "CheckedStatefulEntry::event().semantic_type()",
        ),
        (
            "/scrutinee/choice_expected",
            "TypeKind::entity_ref(EntityKind::ChoiceOption)",
        ),
        ("/scrutinee/timeout_expected", "TypeKind::Duration"),
        ("/scrutinee/expression_expected", "TypeKind::Bool"),
    ] {
        require_str(&bundle.contract, pointer, expected, "scrutinee_contract")?;
    }
    require_string_array(
        &bundle.contract,
        "/scrutinee/event_reachability_edges",
        &[
            "stateful Entry goto",
            "selected project call",
            "prepared Include Flow",
        ],
        "scrutinee_contract",
    )
}

fn validate_mark(bundle: &Bundle) -> GateResult {
    require_string_array(
        &bundle.contract,
        "/mark_coordinate/bytes",
        &[
            "CheckedSemanticPath::canonical_bytes(application)",
            "u8(2)",
            "u8(0)",
            "u32_le(ordinal)",
        ],
        "mark_coordinate",
    )?;
    require_u64(
        &bundle.contract,
        "/mark_coordinate/path_suffix",
        2,
        "mark_coordinate",
    )?;
    require_u64(
        &bundle.contract,
        "/mark_coordinate/family_tag",
        0,
        "mark_coordinate",
    )?;
    for key in ["name_semantic", "tag_id_semantic"] {
        require_bool(
            &bundle.contract,
            &format!("/mark_coordinate/{key}"),
            false,
            "mark_coordinate",
        )?;
    }
    require_string_array(
        &bundle.contract,
        "/mark_coordinate/checked_line_plan_fields",
        &["effect_sites"],
        "mark_coordinate",
    )?;
    require_bool(
        &bundle.contract,
        "/mark_coordinate/compiler_projection_map_temporary",
        true,
        "mark_coordinate",
    )?;
    require_str(
        &bundle.contract,
        "/mark_coordinate/runtime_mark_type",
        "RuntimeDialogueMarkId",
        "mark_coordinate",
    )
}

fn validate_checked_tags(bundle: &Bundle) -> GateResult {
    require_tagged_array(
        &bundle.contract,
        "/checked_trigger",
        TRIGGERS,
        "checked_tags",
    )?;
    require_tagged_array(
        &bundle.contract,
        "/checked_select_statement",
        SELECT_STATEMENTS,
        "checked_tags",
    )?;
    require_tagged_array(
        &bundle.contract,
        "/checked_select_head",
        SELECT_HEADS,
        "checked_tags",
    )?;
    require_tagged_array(
        &bundle.contract,
        "/checked_statement_payload",
        PAYLOADS,
        "checked_tags",
    )?;
    require_string_array(
        &bundle.contract,
        "/checked_unsafe/fields",
        &["UnsafeAuditId", "has_safety_doc"],
        "checked_tags",
    )?;
    require_bool(
        &bundle.contract,
        "/checked_unsafe/verifier_rereads_hir",
        false,
        "checked_tags",
    )
}

fn validate_statement_matrix(bundle: &Bundle) -> GateResult {
    let rows = bundle
        .contract
        .pointer("/statement_matrix")
        .and_then(Value::as_array)
        .ok_or_else(|| GateError::new("statement_matrix", "matrix missing"))?;
    if rows.len() != STATEMENTS.len() {
        return Err(GateError::new(
            "statement_matrix",
            format!("expected {} rows, got {}", STATEMENTS.len(), rows.len()),
        ));
    }
    for (index, (row, (hir, payload, detail))) in rows.iter().zip(STATEMENTS).enumerate() {
        let tag = format!("0x{:04X}", 0x0700 + index);
        if row["index"].as_u64() != u64::try_from(index).ok()
            || row["hir_tag"].as_str() != Some(tag.as_str())
            || row["hir"].as_str() != Some(hir)
            || row["payload"].as_str() != Some(payload)
            || row["detail"].as_str() != Some(detail)
        {
            return Err(GateError::new(
                "statement_matrix",
                format!("row {index} differs: {row}"),
            ));
        }
    }
    Ok(())
}

fn validate_transcript(bundle: &Bundle) -> GateResult {
    require_u64(&bundle.contract, "/transcript/version", 1, "version_one")?;
    require_str(
        &bundle.contract,
        "/transcript/algorithm",
        "BLAKE3 purpose-built canonical grammar",
        "transcript_contract",
    )?;
    for (pointer, expected) in [
        (
            "/transcript/statement_domain",
            "arcweft.lang.sema.checked-statement.v1\\0",
        ),
        (
            "/transcript/body_domain",
            "arcweft.lang.sema.checked-statement-body.v1\\0",
        ),
        (
            "/transcript/rich_text_domain",
            "arcweft.lang.sema.checked-rich-text-action.v1\\0",
        ),
    ] {
        require_str(&bundle.contract, pointer, expected, "transcript_contract")?;
    }
    for key in [
        "hir_tag_first",
        "typed_child_roles",
        "typed_body_roles",
        "checked_u64_accounting",
    ] {
        require_bool(
            &bundle.contract,
            &format!("/transcript/{key}"),
            true,
            "transcript_contract",
        )?;
    }
    for (pointer, expected) in [
        (
            "/transcript/accepted_statement_coordinate",
            "StableCheckedStatementCoordinate::canonical_bytes()",
        ),
        (
            "/transcript/accepted_body_coordinate",
            "StableCheckedBodyCoordinate::canonical_bytes()",
        ),
    ] {
        require_str(&bundle.contract, pointer, expected, "transcript_contract")?;
    }
    for key in [
        "raw_ids",
        "source_spelling",
        "spans",
        "serde",
        "whole_catalog_digest",
        "other_success",
        "unsupported_identity_success",
    ] {
        require_bool(
            &bundle.contract,
            &format!("/transcript/{key}"),
            false,
            "transcript_contract",
        )?;
    }
    Ok(())
}

fn validate_wait_mark(bundle: &Bundle) -> GateResult {
    require_bool(
        &bundle.contract,
        "/wait_mark/surface_admitted_to_final_runtime_plan",
        false,
        "wait_mark_policy",
    )?;
    require_str(
        &bundle.contract,
        "/wait_mark/this_cut_policy",
        "executable rejection",
        "wait_mark_policy",
    )?;
    require_bool(
        &bundle.contract,
        "/wait_mark/legacy_string_fallback",
        false,
        "wait_mark_policy",
    )?;
    require_str(
        &bundle.contract,
        "/wait_mark/future_identity",
        "HirDialogueMarkId -> StableCheckedDialogueMarkCoordinate -> RuntimeDialogueMarkId",
        "wait_mark_policy",
    )
}

fn validate_prohibitions(bundle: &Bundle) -> GateResult {
    let object = bundle
        .contract
        .pointer("/prohibitions")
        .and_then(Value::as_object)
        .ok_or_else(|| GateError::new("forbidden_authority", "prohibitions missing"))?;
    let expected = [
        "any_semantic_fallback",
        "other_semantic_fallback",
        "named_ingress_fallback",
        "source_string_authority",
        "raw_hir_id_identity",
        "public_structural_expr_id",
        "copied_statement_ast",
        "copied_body_graph",
        "legacy_reader",
        "compatibility_alias",
        "version_other_than_one",
        "second_statement_model",
        "second_mark_model",
        "second_trigger_select_unsafe_model",
    ];
    if object.len() != expected.len() {
        return Err(GateError::new(
            "forbidden_authority",
            "prohibition count differs",
        ));
    }
    expected
        .into_iter()
        .try_for_each(|key| match object.get(key) {
            Some(Value::Bool(false)) => Ok(()),
            value => Err(GateError::new(
                "forbidden_authority",
                format!("{key} must be false, got {value:?}"),
            )),
        })
}

fn validate_doc_anchors(bundle: &Bundle) -> GateResult {
    require_anchors(
        bundle,
        "FINAL_DESIGN.md",
        &[
            "`CheckedStatementPayload`",
            "TypeKind::StatementIngress",
            "Entry-seeded declaration worklist",
            "wait(mark(.name))",
            "five `CheckedSelectResolution` variants",
        ],
        "required_files",
    )?;
    require_anchors(
        bundle,
        "HIR_AND_SEMA_SCHEMAS.md",
        &[
            "pub enum HirTrigger",
            "StatementScrutineeTypeAuthority",
            "pub enum CheckedStatementPayload",
            "RuntimeTriggerAdmission",
            "tag `88`",
        ],
        "required_files",
    )?;
    require_anchors(
        bundle,
        "SCRUTINEE_TYPE_SOURCES.md",
        &[
            "PreparedEntrySemanticAuthority::ty(TypeId)",
            "enclosing_choice_lifecycle",
            "Prepared Include resolution",
        ],
        "required_files",
    )?;
    require_anchors(
        bundle,
        "MARK_COORDINATE_AND_TRANSCRIPT.md",
        &[
            "|| u8(2)",
            "|| u8(0)",
            "|| u32_le(ordinal)",
            "0x0700..0x0722",
            "Marker action",
        ],
        "required_files",
    )?;
    require_anchors(
        bundle,
        "TEST_MATRIX.md",
        &["0x0700", "0x0722", "per-row all-35 payload mutation"],
        "required_files",
    )
}

pub fn validate_manifest(bundle: &Bundle) -> GateResult {
    let manifest = text_file(bundle, "MANIFEST.txt", "manifest")?;
    let mut actual = BTreeMap::new();
    for line in manifest.lines() {
        let Some((hash, path)) = line.split_once("  ") else {
            return Err(GateError::new("manifest", format!("invalid line {line:?}")));
        };
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(GateError::new(
                "manifest",
                format!("invalid hash for {path}"),
            ));
        }
        if path == "MANIFEST.txt"
            || path.starts_with('/')
            || path.contains("..")
            || path.contains('\\')
            || actual.insert(path.to_owned(), hash.to_owned()).is_some()
        {
            return Err(GateError::new("manifest", format!("invalid path {path}")));
        }
    }
    let expected = bundle
        .files
        .iter()
        .filter(|(path, _)| path.as_str() != "MANIFEST.txt")
        .map(|(path, bytes)| (path.clone(), sha256(bytes)))
        .collect::<BTreeMap<_, _>>();
    if actual == expected {
        Ok(())
    } else {
        Err(GateError::new(
            "manifest",
            format!(
                "manifest differs: expected {} entries, got {}",
                expected.len(),
                actual.len()
            ),
        ))
    }
}

#[allow(
    dead_code,
    reason = "the positive cargo-script uses manifest writing; the negative script does not"
)]
pub fn write_manifest(root: &Path) -> GateResult {
    let bundle = load_bundle(root)?;
    validate_semantic(&bundle)?;
    let contents = bundle
        .files
        .iter()
        .filter(|(path, _)| path.as_str() != "MANIFEST.txt")
        .map(|(path, bytes)| format!("{}  {path}\n", sha256(bytes)))
        .collect::<String>();
    fs::write(bundle.root.join("MANIFEST.txt"), contents)
        .map_err(|error| GateError::new("manifest", error.to_string()))
}

pub fn validate_repository(bundle: &Bundle) -> GateResult {
    let expected_head = require_value_str(&bundle.inventory, "/head", "source_inventory")?;
    let expected_origin = require_value_str(&bundle.inventory, "/origin_main", "source_inventory")?;
    if git(&bundle.repo, &["branch", "--show-current"])? != "main" {
        return Err(GateError::new("source_inventory", "branch is not main"));
    }
    if git(&bundle.repo, &["rev-parse", "HEAD"])? != expected_head {
        return Err(GateError::new("source_inventory", "HEAD differs"));
    }
    if git(&bundle.repo, &["rev-parse", "origin/main"])? != expected_origin {
        return Err(GateError::new("source_inventory", "origin/main differs"));
    }
    validate_source_files(bundle)?;
    validate_ast_inventories(bundle)?;
    let dependencies = cargo_dependency_map(&bundle.repo)?;
    validate_dependency_map(&dependencies)
}

fn validate_source_files(bundle: &Bundle) -> GateResult {
    let files = bundle
        .inventory
        .pointer("/files")
        .and_then(Value::as_array)
        .ok_or_else(|| GateError::new("source_inventory", "files missing"))?;
    for row in files {
        let path = row["path"]
            .as_str()
            .ok_or_else(|| GateError::new("source_inventory", "path missing"))?;
        let expected = row["blob"]
            .as_str()
            .ok_or_else(|| GateError::new("source_inventory", "blob missing"))?;
        let actual = git(&bundle.repo, &["hash-object", "--", path])?;
        if actual != expected {
            return Err(GateError::new(
                "source_inventory",
                format!("worktree blob differs for {path}: {actual}"),
            ));
        }
        match row["head_blob"].as_str() {
            Some(expected_head) => {
                let actual_head = git(&bundle.repo, &["rev-parse", &format!("HEAD:{path}")])?;
                if actual_head != expected_head {
                    return Err(GateError::new(
                        "source_inventory",
                        format!("HEAD blob differs for {path}"),
                    ));
                }
            }
            None => {
                if git_status(&bundle.repo, &["cat-file", "-e", &format!("HEAD:{path}")])? {
                    return Err(GateError::new(
                        "source_inventory",
                        format!("expected untracked source now exists at HEAD:{path}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_ast_inventories(bundle: &Bundle) -> GateResult {
    let inventories = bundle
        .inventory
        .pointer("/ast_inventories")
        .and_then(Value::as_array)
        .ok_or_else(|| GateError::new("source_inventory", "AST inventories missing"))?;
    for inventory in inventories {
        let path = inventory["path"]
            .as_str()
            .ok_or_else(|| GateError::new("source_inventory", "AST path missing"))?;
        let enum_name = inventory["enum"]
            .as_str()
            .ok_or_else(|| GateError::new("source_inventory", "AST enum missing"))?;
        let expected = inventory["variants"]
            .as_array()
            .ok_or_else(|| GateError::new("source_inventory", "AST variants missing"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| GateError::new("source_inventory", "non-string AST variant"))
            })
            .collect::<GateResult<Vec<_>>>()?;
        let source = fs::read_to_string(bundle.repo.join(path))
            .map_err(|error| GateError::new("source_inventory", error.to_string()))?;
        let syntax = syn::parse_file(&source)
            .map_err(|error| GateError::new("source_inventory", error.to_string()))?;
        let actual = syntax.items.into_iter().find_map(|item| match item {
            syn::Item::Enum(item) if item.ident == enum_name => Some(
                item.variants
                    .into_iter()
                    .map(|variant| variant.ident.to_string())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        });
        if actual.as_ref() != Some(&expected) {
            return Err(GateError::new(
                "source_inventory",
                format!("{enum_name}: expected {expected:?}, got {actual:?}"),
            ));
        }
    }
    Ok(())
}

fn cargo_dependency_map(repo: &Path) -> GateResult<BTreeMap<String, BTreeSet<String>>> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(repo)
        .output()
        .map_err(|error| GateError::new("dependency_direction", error.to_string()))?;
    if !output.status.success() {
        return Err(GateError::new(
            "dependency_direction",
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| GateError::new("dependency_direction", error.to_string()))?;
    metadata["packages"]
        .as_array()
        .ok_or_else(|| GateError::new("dependency_direction", "packages missing"))?
        .iter()
        .map(|package| {
            let name = package["name"]
                .as_str()
                .ok_or_else(|| GateError::new("dependency_direction", "package name missing"))?;
            let dependencies = package["dependencies"]
                .as_array()
                .ok_or_else(|| GateError::new("dependency_direction", "dependencies missing"))?
                .iter()
                .map(|dependency| {
                    dependency["name"]
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| {
                            GateError::new("dependency_direction", "dependency name missing")
                        })
                })
                .collect::<GateResult<BTreeSet<_>>>()?;
            Ok((name.to_owned(), dependencies))
        })
        .collect()
}

pub fn validate_dependency_map(dependencies: &BTreeMap<String, BTreeSet<String>>) -> GateResult {
    let sema = dependencies
        .get("arcweft-lang-sema")
        .ok_or_else(|| GateError::new("dependency_direction", "sema package missing"))?;
    for required in ["arcweft-lang-hir", "arcweft-core"] {
        if !sema.contains(required) {
            return Err(GateError::new(
                "dependency_direction",
                format!("sema missing {required}"),
            ));
        }
    }
    for forbidden in [
        "arcweft-compiler",
        "arcweft-runtime-plan",
        "arcweft-verify",
        "arcweft-cli",
    ] {
        if sema.contains(forbidden) {
            return Err(GateError::new(
                "dependency_direction",
                format!("sema depends on higher layer {forbidden}"),
            ));
        }
    }
    for lower in ["arcweft-lang-hir", "arcweft-core", "arcweft-runtime-plan"] {
        if dependencies
            .get(lower)
            .is_some_and(|values| values.contains("arcweft-lang-sema"))
        {
            return Err(GateError::new(
                "dependency_direction",
                format!("reverse dependency {lower} -> sema"),
            ));
        }
    }
    Ok(())
}

#[allow(
    dead_code,
    reason = "the negative cargo-script imports this helper; the positive script does not"
)]
pub fn repository_dependency_map(
    repo: &Path,
) -> Result<BTreeMap<String, BTreeSet<String>>, GateError> {
    cargo_dependency_map(repo)
}

fn parse_json_file(files: &BTreeMap<String, Vec<u8>>, path: &str) -> GateResult<Value> {
    let bytes = files
        .get(path)
        .ok_or_else(|| GateError::new("required_files", format!("missing {path}")))?;
    serde_json::from_slice(bytes)
        .map_err(|error| GateError::new("required_files", format!("{path}: {error}")))
}

fn normalized_relative(root: &Path, path: &Path) -> GateResult<String> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|error| GateError::new("required_files", error.to_string()))
}

fn require_file_bytes(
    bundle: &Bundle,
    path: &str,
    expected: &[u8],
    gate: &'static str,
) -> GateResult {
    match bundle.files.get(path) {
        Some(actual) if actual == expected => Ok(()),
        actual => Err(GateError::new(
            gate,
            format!("{path}: expected {expected:?}, got {actual:?}"),
        )),
    }
}

fn text_file<'a>(bundle: &'a Bundle, path: &str, gate: &'static str) -> GateResult<&'a str> {
    let bytes = bundle
        .files
        .get(path)
        .ok_or_else(|| GateError::new(gate, format!("missing {path}")))?;
    std::str::from_utf8(bytes).map_err(|error| GateError::new(gate, error.to_string()))
}

fn require_anchors(
    bundle: &Bundle,
    path: &str,
    anchors: &[&str],
    gate: &'static str,
) -> GateResult {
    let text = text_file(bundle, path, gate)?;
    anchors.iter().try_for_each(|anchor| {
        if text.contains(anchor) {
            Ok(())
        } else {
            Err(GateError::new(
                gate,
                format!("{path} missing anchor {anchor}"),
            ))
        }
    })
}

fn require_value_str<'a>(
    value: &'a Value,
    pointer: &str,
    gate: &'static str,
) -> GateResult<&'a str> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| GateError::new(gate, format!("{pointer}: string missing")))
}

fn require_value_u64(value: &Value, pointer: &str, gate: &'static str) -> GateResult<u64> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| GateError::new(gate, format!("{pointer}: u64 missing")))
}

fn require_str(value: &Value, pointer: &str, expected: &str, gate: &'static str) -> GateResult {
    match value.pointer(pointer).and_then(Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        actual => Err(GateError::new(
            gate,
            format!("{pointer}: expected {expected:?}, got {actual:?}"),
        )),
    }
}

fn require_u64(value: &Value, pointer: &str, expected: u64, gate: &'static str) -> GateResult {
    match value.pointer(pointer).and_then(Value::as_u64) {
        Some(actual) if actual == expected => Ok(()),
        actual => Err(GateError::new(
            gate,
            format!("{pointer}: expected {expected}, got {actual:?}"),
        )),
    }
}

fn require_bool(value: &Value, pointer: &str, expected: bool, gate: &'static str) -> GateResult {
    match value.pointer(pointer).and_then(Value::as_bool) {
        Some(actual) if actual == expected => Ok(()),
        actual => Err(GateError::new(
            gate,
            format!("{pointer}: expected {expected}, got {actual:?}"),
        )),
    }
}

fn require_string_array(
    value: &Value,
    pointer: &str,
    expected: &[&str],
    gate: &'static str,
) -> GateResult {
    let actual = value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| GateError::new(gate, format!("{pointer}: array missing")))?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| GateError::new(gate, format!("{pointer}: non-string")))
        })
        .collect::<GateResult<Vec<_>>>()?;
    let expected = expected.iter().map(ToString::to_string).collect::<Vec<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(GateError::new(
            gate,
            format!("{pointer}: expected {expected:?}, got {actual:?}"),
        ))
    }
}

fn require_tagged_array(
    value: &Value,
    pointer: &str,
    expected: &[&str],
    gate: &'static str,
) -> GateResult {
    let rows = value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| GateError::new(gate, format!("{pointer}: array missing")))?;
    if rows.len() != expected.len() {
        return Err(GateError::new(gate, format!("{pointer}: count differs")));
    }
    for (index, (row, name)) in rows.iter().zip(expected).enumerate() {
        if row["name"].as_str() != Some(name) || row["tag"].as_u64() != u64::try_from(index).ok() {
            return Err(GateError::new(
                gate,
                format!("{pointer}[{index}] differs: {row}"),
            ));
        }
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn git(repo: &Path, args: &[&str]) -> GateResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|error| GateError::new("source_inventory", error.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(GateError::new(
            "source_inventory",
            format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ))
    }
}

fn git_status(repo: &Path, args: &[&str]) -> GateResult<bool> {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map(|output| output.status.success())
        .map_err(|error| GateError::new("source_inventory", error.to_string()))
}
