use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

pub const DESIGN_REL: &str = "docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure";

const EXPECTED_HEAD: &str = "9a5d30d25620541c3f2975d31e04e04e3bc9514c";
const EXPECTED_REQUEST_BYTES: u64 = 10_032;
const EXPECTED_REQUEST_SHA256: &str =
    "7250743e386ce404664c4c211d109094c9f40031211edc70847688754922aa9c";

const EXPRESSION_RESOLUTIONS: &[&str] = &[
    "Structural",
    "Literal",
    "Value",
    "Select",
    "Nominal",
    "Variant",
    "StageLook",
    "Effect",
    "Call",
    "Await",
    "Choice",
    "Try",
    "ImplicitCallable",
    "ImplicitParameter",
    "Pipe",
    "PipeLeft",
    "ViewCall",
    "ViewCallee",
    "StyleValue",
    "StyleCallee",
    "DialogueLineReference",
    "DialogueLineCoordinate",
    "DialogueTextKeyCoordinate",
    "CharacterDialogueFactory",
    "CharacterDialogueReconfigure",
    "DialogueApplication",
    "PostfixBracket",
];

const VALUE_RESOLUTIONS: &[&str] = &[
    "Local",
    "LineContext",
    "CharacterField",
    "ProjectCallable",
    "ProjectItem",
    "Entry",
    "Registered",
    "Constant",
];

const SELECT_RESOLUTIONS: &[&str] = &[
    "Method",
    "DialogueView",
    "AgentField",
    "ProgressField",
    "Field",
    "TupleElement",
    "RecordElement",
];

const PATTERN_RESOLUTIONS: &[&str] = &["Structural", "Literal", "Entity", "Nominal", "Variant"];

const EXPRESSION_FAMILIES: &[&str] = &[
    "Unit",
    "Literal",
    "EntityReference",
    "LifetimePath",
    "Path",
    "ShortVariant",
    "Placeholder",
    "Tuple",
    "BracketSequence",
    "NumericBracketSequence",
    "ArrayRepeat",
    "Call",
    "Select",
    "Index",
    "Pipe",
    "Try",
    "Await",
    "Thread",
    "Choice",
    "Range",
    "Record",
    "RecordLiteral",
    "Binary",
    "Borrow",
    "Dereference",
    "Closure",
    "Unary",
    "Block",
    "ComputationBlock",
    "NamedBlock",
    "Loop",
    "If",
    "IfLet",
    "Match",
    "DialogueContentApplication",
    "PostfixBracket",
    "Error",
    "ForSynthetic",
];

const PATTERN_FAMILIES: &[&str] = &[
    "Binding",
    "MutableBinding",
    "Literal",
    "EntityReference",
    "Variant",
    "Discard",
    "Tuple",
    "Record",
    "BracketSequence",
    "WholeBinding",
    "Or",
    "TypedBinding",
    "Error",
];

const STATEMENT_FAMILIES: &[&str] = &[
    "Assertion",
    "Let",
    "Assign",
    "LetElse",
    "LetChoice",
    "LetScope",
    "LetActionReceive",
    "Return",
    "Out",
    "Goto",
    "DeferBlock",
    "Defer",
    "Yield",
    "Signal",
    "LifetimeSet",
    "Wait",
    "On",
    "UnsafeLifetime",
    "Choice",
    "If",
    "IfLet",
    "Match",
    "While",
    "WhileLet",
    "For",
    "Close",
    "Select",
    "SourceLocale",
    "Scope",
    "Include",
    "Break",
    "Continue",
    "Expression",
    "ProofCall",
    "Error",
];

const BODY_CHILD_ROLES: &[&str] = &[
    "Expression",
    "Statement",
    "Tail",
    "RecoveryExpression",
    "ThreadItem",
];

const STATEMENT_BODY_ROLES: &[&str] = &[
    "LetElse",
    "Defer",
    "On",
    "UnsafeLifetime",
    "Then",
    "Else",
    "MatchArm",
    "While",
    "WhileLet",
    "For",
    "SelectBranch",
    "SourceLocale",
    "Scope",
];

const DECLARATION_OWNERS: &[&str] = &[
    "Function",
    "Predicate",
    "Proof",
    "Flow",
    "TraitImplementation",
    "InherentMethod",
    "View",
];

const DECLARATION_ROOTS: &[&str] = &[
    "ParameterPattern",
    "ParameterDefault",
    "FunctionBody",
    "PredicateBody",
    "ProofBody",
    "FlowBody",
    "ImplFunctionBody",
    "ViewValue",
];

const BODY_ROOTS: &[&str] = &[
    "AwaitBranchPattern",
    "AwaitBranchBody",
    "ChoiceLetStatement",
    "ChoiceForPattern",
    "ChoiceMatchArmPattern",
    "ChoiceOptionForPattern",
    "ChoiceOptionSelectBody",
    "ChoiceOptionLetStatement",
    "ChoicePlanTimeoutBody",
    "ChoicePlanCancelBody",
    "ChoicePlanOnSelectPattern",
    "ChoicePlanOnSelectBody",
    "DialogueLinePlanInitStatement",
    "DialogueLinePlanThreadStatement",
    "DialogueLinePlanOnStatement",
    "DialogueLinePlanLetPattern",
    "DialogueLinePlanStatement",
    "DialogueLinePlanCancelRuleStatement",
    "DialogueLinePlanErrorStatement",
];

const LINE_PLAN_STATEMENT_ROLES: &[&str] =
    &["Init", "Thread", "On", "Statement", "CancelRule", "Error"];

const LIMITS: &[&str] = &[
    "max_arms",
    "max_matrix_rows",
    "max_or_alternatives",
    "max_pattern_nodes",
    "max_expression_nodes",
    "max_depth",
    "max_sequence_partitions",
    "max_specializations",
    "max_unreachable_rows",
    "max_witness_nodes",
    "max_transcript_bytes",
];

const COVERAGE_DOMAINS: &[&str] = &[
    "products",
    "constant_arrays",
    "symbolic_sequences",
    "Or",
    "literal_Other",
    "entity_open_Other",
    "Never",
    "Choice",
    "closed_variants",
];

const SOURCE_BLOBS: &[(&str, &str)] = &[
    (
        "crates/arcweft-lang-sema/src/final_analysis/model.rs",
        "836295b1c58ce1a08d06a302643b2a265e8b9cd3",
    ),
    (
        "crates/arcweft-lang-sema/src/final_analysis/semantic_transcript.rs",
        "91b764e625f9582acc3ea0dcf646951cb42a7cd1",
    ),
    (
        "crates/arcweft-lang-sema/src/final_analysis/match_edges.rs",
        "d7426bce4818bba25d3f64aa5e1c5f628a027283",
    ),
    (
        "crates/arcweft-lang-sema/src/final_analysis/match_edges/model.rs",
        "055ad0f01e4c29d97bb3f1734c4c9574ffe800e2",
    ),
    (
        "crates/arcweft-lang-sema/src/final_analysis/analyzer/patterns.rs",
        "736d2be8bf6521042fc93bca9b592ce0ef81e255",
    ),
    (
        "crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs",
        "4d70d5dde6f7920c2af83d268b8dc9d38cbf7282",
    ),
    (
        "crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs",
        "a7df1895471661b168db0eddaf900661c42f2625",
    ),
    (
        "crates/arcweft-lang-hir/src/final_project/semantic_paths.rs",
        "78a68bc9d8dd8679a6f7d0514111f9bbb046ca98",
    ),
    (
        "crates/arcweft-lang-hir/src/final_project.rs",
        "5069669226a22f65b4b4d89654715166e53d7227",
    ),
    (
        "crates/arcweft-lang-hir/src/body_edges.rs",
        "e08f25b5ed1c74160b542837151eb33e86e5d6ad",
    ),
    (
        "crates/arcweft-lang-hir/src/expr.rs",
        "b9e2c3e9117ba61e5c428064a5b7f0af973adcd5",
    ),
    (
        "crates/arcweft-lang-hir/src/pattern.rs",
        "1a7adf00f7caeee6aa517dfe2c7873dff86145bc",
    ),
    (
        "crates/arcweft-lang-hir/src/pattern/child_edges.rs",
        "7e3972887daa16b7f2a2b914d0d51a7f637de2c9",
    ),
    (
        "crates/arcweft-lang-hir/src/stmt.rs",
        "061c97afb6367413d60ca8bcdb271f578dd7d379",
    ),
    (
        "crates/arcweft-lang-hir/src/stmt/child_edges.rs",
        "9b6bf0d0a30aca2296f63a514245cbf237f7401d",
    ),
    (
        "crates/arcweft-lang-hir/src/item.rs",
        "67c9ed2d12b00c4d8e42009c3e19f51772d8bfb5",
    ),
    (
        "crates/arcweft-lang-hir/src/item/retained.rs",
        "eb7e2a04c3ca8d54b58c153c1b43464de0873b8a",
    ),
    (
        "crates/arcweft-lang-hir/src/symbol/identity.rs",
        "7f06f1093c74942b06923ebc4663ce56609c54d1",
    ),
    (
        "crates/arcweft-lang-sema/src/nominal/model.rs",
        "b8f6c605d5f471de4fd0deb52592572ca44b91bd",
    ),
    (
        "crates/arcweft-lang-sema/src/callable/checked_catalog.rs",
        "593028b11a4197ce085be3c58be3be820c44471f",
    ),
    (
        "crates/arcweft-lang-sema/src/dialogue_view.rs",
        "eb55dfa79639b85256fd4b1440bfbbc70ab9467a",
    ),
];

#[derive(Clone, Debug)]
pub struct Bundle {
    pub root: PathBuf,
    pub repo: PathBuf,
    pub files: BTreeMap<String, Vec<u8>>,
    pub contract: Value,
    pub inventory: Value,
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
        .ok_or_else(|| GateError::new("repository", "cannot locate .git ancestor"))?;
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
    Ok(Bundle {
        root,
        repo,
        files,
        contract,
        inventory,
    })
}

pub fn validate_semantic(bundle: &Bundle) -> GateResult {
    require_file_bytes(
        bundle,
        "FINAL_STATUS.md",
        b"READY_FOR_IMPLEMENTATION\n",
        "terminal_status",
    )?;
    require_file_bytes(bundle, "OPEN_QUESTIONS.md", b"none\n", "open_questions")?;
    require_u64(&bundle.contract, "/contract_version", 1, "version_one")?;
    require_str(
        &bundle.contract,
        "/status",
        "READY_FOR_IMPLEMENTATION",
        "terminal_status",
    )?;
    require_str(
        &bundle.contract,
        "/open_questions",
        "none",
        "open_questions",
    )?;
    validate_request(bundle)?;
    validate_inventory(bundle)?;
    validate_source_blob_contract(bundle)?;
    validate_decisions(bundle)?;
    validate_schema_anchors(bundle)?;
    validate_transcript_contract(bundle)?;
    validate_coverage_contract(bundle)?;
    validate_declaration_bridge(bundle)?;
    validate_hir_path_contract(bundle)?;
    validate_publication(bundle)?;
    validate_non_goals(bundle)?;
    validate_required_files(bundle)
}

pub fn validate_manifest(bundle: &Bundle) -> GateResult {
    let manifest = text_file(bundle, "MANIFEST.sha256", "manifest")?;
    let entries = manifest
        .lines()
        .map(|line| {
            let (hash, path) = line
                .split_once("  ")
                .ok_or_else(|| GateError::new("manifest", format!("invalid line: {line}")))?;
            if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(GateError::new(
                    "manifest",
                    format!("invalid SHA-256 for {path}"),
                ));
            }
            Ok((path.to_owned(), hash.to_ascii_lowercase()))
        })
        .collect::<GateResult<Vec<_>>>()?;
    let paths = entries.iter().map(|(path, _)| path).collect::<Vec<_>>();
    if !paths.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(GateError::new(
            "manifest",
            "members must be unique and byte-order sorted",
        ));
    }
    let expected = bundle
        .files
        .keys()
        .filter(|path| path.as_str() != "MANIFEST.sha256")
        .cloned()
        .collect::<BTreeSet<_>>();
    let actual = entries
        .iter()
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(GateError::new(
            "manifest",
            format!("member set mismatch: expected {expected:?}, actual {actual:?}"),
        ));
    }
    entries.into_iter().try_for_each(|(path, expected_hash)| {
        let bytes = bundle
            .files
            .get(&path)
            .ok_or_else(|| GateError::new("manifest", format!("missing {path}")))?;
        let actual_hash = sha256(bytes);
        if actual_hash == expected_hash {
            Ok(())
        } else {
            Err(GateError::new(
                "manifest",
                format!("hash mismatch for {path}"),
            ))
        }
    })
}

pub fn validate_repository(bundle: &Bundle) -> GateResult {
    let head = git(&bundle.repo, &["rev-parse", "HEAD"])?;
    let origin = git(&bundle.repo, &["rev-parse", "origin/main"])?;
    let branch = git(&bundle.repo, &["branch", "--show-current"])?;
    if head != EXPECTED_HEAD || origin != EXPECTED_HEAD || branch != "main" {
        return Err(GateError::new(
            "repository",
            format!(
                "expected main {EXPECTED_HEAD}, got head={head} origin={origin} branch={branch}"
            ),
        ));
    }
    let production_status = git(
        &bundle.repo,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            "crates",
            "Cargo.toml",
            "Cargo.lock",
        ],
    )?;
    if !production_status.is_empty() {
        return Err(GateError::new(
            "repository",
            format!("production scope is dirty: {production_status}"),
        ));
    }
    SOURCE_BLOBS.iter().try_for_each(|(path, expected)| {
        let actual = git(&bundle.repo, &["rev-parse", &format!("HEAD:{path}")])?;
        if actual == *expected {
            Ok(())
        } else {
            Err(GateError::new(
                "source_blobs",
                format!("{path}: expected {expected}, got {actual}"),
            ))
        }
    })?;
    validate_request_source(bundle)?;
    validate_rust_ast_inventory(bundle)?;
    validate_cargo_metadata(bundle)
}

fn validate_request(bundle: &Bundle) -> GateResult {
    require_u64(
        &bundle.contract,
        "/request/bytes",
        EXPECTED_REQUEST_BYTES,
        "request_mirror",
    )?;
    require_str(
        &bundle.contract,
        "/request/sha256",
        EXPECTED_REQUEST_SHA256,
        "request_mirror",
    )?;
    let bytes = bundle
        .files
        .get("REQUEST.md")
        .ok_or_else(|| GateError::new("request_mirror", "REQUEST.md missing"))?;
    let len = u64::try_from(bytes.len())
        .map_err(|_| GateError::new("request_mirror", "request length overflow"))?;
    if len != EXPECTED_REQUEST_BYTES || sha256(bytes) != EXPECTED_REQUEST_SHA256 {
        return Err(GateError::new(
            "request_mirror",
            format!(
                "expected {EXPECTED_REQUEST_BYTES}/{EXPECTED_REQUEST_SHA256}, got {len}/{}",
                sha256(bytes)
            ),
        ));
    }
    Ok(())
}

fn validate_inventory(bundle: &Bundle) -> GateResult {
    require_u64(&bundle.inventory, "/contract_version", 1, "version_one")?;
    require_str(
        &bundle.inventory,
        "/baseline_commit",
        EXPECTED_HEAD,
        "source_inventory",
    )?;
    let rows = [
        ("checked_expression_resolution", EXPRESSION_RESOLUTIONS),
        ("checked_value_resolution", VALUE_RESOLUTIONS),
        ("checked_select_resolution", SELECT_RESOLUTIONS),
        ("checked_pattern_resolution", PATTERN_RESOLUTIONS),
        ("hir_expression_families", EXPRESSION_FAMILIES),
        ("hir_pattern_families", PATTERN_FAMILIES),
        ("hir_statement_families", STATEMENT_FAMILIES),
        ("hir_body_child_roles", BODY_CHILD_ROLES),
        ("hir_statement_body_roles", STATEMENT_BODY_ROLES),
        ("match_bearing_declaration_owners", DECLARATION_OWNERS),
        ("declaration_roots", DECLARATION_ROOTS),
        ("expression_owned_non_expression_roots", BODY_ROOTS),
    ];
    rows.iter().try_for_each(|(key, expected)| {
        require_string_array(
            &bundle.inventory,
            &format!("/{key}"),
            expected,
            "source_inventory",
        )?;
        require_u64(
            &bundle.inventory,
            &format!("/counts/{key}"),
            u64::try_from(expected.len())
                .map_err(|_| GateError::new("source_inventory", "count overflow"))?,
            "source_inventory",
        )
    })?;
    let contract_counts = [
        (
            "checked_expression_resolution_count",
            EXPRESSION_RESOLUTIONS.len(),
        ),
        ("checked_value_resolution_count", VALUE_RESOLUTIONS.len()),
        ("checked_select_resolution_count", SELECT_RESOLUTIONS.len()),
        (
            "checked_pattern_resolution_count",
            PATTERN_RESOLUTIONS.len(),
        ),
        ("hir_expression_family_count", EXPRESSION_FAMILIES.len()),
        ("hir_pattern_family_count", PATTERN_FAMILIES.len()),
        ("hir_statement_family_count", STATEMENT_FAMILIES.len()),
        ("hir_body_child_role_count", BODY_CHILD_ROLES.len()),
        ("hir_statement_body_role_count", STATEMENT_BODY_ROLES.len()),
        (
            "match_bearing_declaration_owner_count",
            DECLARATION_OWNERS.len(),
        ),
        ("declaration_root_count", DECLARATION_ROOTS.len()),
        (
            "expression_owned_non_expression_root_count",
            BODY_ROOTS.len(),
        ),
    ];
    contract_counts.iter().try_for_each(|(key, expected)| {
        require_u64(
            &bundle.contract,
            &format!("/source_inventory/{key}"),
            u64::try_from(*expected)
                .map_err(|_| GateError::new("source_inventory", "count overflow"))?,
            "source_inventory",
        )
    })?;
    require_bool(
        &bundle.contract,
        "/source_inventory/rust_ast_verified",
        true,
        "source_inventory",
    )?;
    require_bool(
        &bundle.contract,
        "/source_inventory/source_blob_verified",
        true,
        "source_inventory",
    )
}

fn validate_source_blob_contract(bundle: &Bundle) -> GateResult {
    let object = bundle
        .contract
        .pointer("/source_blobs")
        .and_then(Value::as_object)
        .ok_or_else(|| GateError::new("source_blobs", "source_blobs object missing"))?;
    if object.len() != SOURCE_BLOBS.len() {
        return Err(GateError::new("source_blobs", "source blob count mismatch"));
    }
    SOURCE_BLOBS.iter().try_for_each(|(path, expected)| {
        match object.get(*path).and_then(Value::as_str) {
            Some(actual) if actual == *expected => Ok(()),
            actual => Err(GateError::new(
                "source_blobs",
                format!("{path}: expected {expected}, got {actual:?}"),
            )),
        }
    })
}

fn validate_decisions(bundle: &Bundle) -> GateResult {
    let decisions = bundle
        .contract
        .pointer("/decisions")
        .and_then(Value::as_array)
        .ok_or_else(|| GateError::new("decisions_1_7", "decisions array missing"))?;
    let actual_ids = decisions
        .iter()
        .filter_map(|decision| decision.get("id").and_then(Value::as_u64))
        .collect::<Vec<_>>();
    if actual_ids != [1, 2, 3, 4, 5, 6, 7] {
        return Err(GateError::new(
            "decisions_1_7",
            format!("expected decisions 1..7, got {actual_ids:?}"),
        ));
    }
    let fields = [
        "owner",
        "schema",
        "consumer",
        "positive_test",
        "negative_mutation",
        "deletion_cut",
    ];
    for decision in decisions {
        for field in fields {
            if decision
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(GateError::new(
                    "decisions_1_7",
                    format!("decision field {field} missing"),
                ));
            }
        }
    }
    let register = text_file(bundle, "DECISION_REGISTER.md", "decisions_1_7")?;
    decisions.iter().try_for_each(|decision| {
        ["positive_test", "negative_mutation", "deletion_cut"]
            .into_iter()
            .try_for_each(|field| {
                let value = decision[field].as_str().expect("validated decision field");
                if register.contains(value) {
                    Ok(())
                } else {
                    Err(GateError::new(
                        "decisions_1_7",
                        format!("register missing {value}"),
                    ))
                }
            })
    })
}

fn validate_schema_anchors(bundle: &Bundle) -> GateResult {
    require_anchors(
        bundle,
        "SCHEMAS.md",
        &[
            "AcceptedProjectItemSemanticId",
            "AcceptedVariantCaseSemanticId",
            "CheckedRecordPatternField",
            "RuntimeRecordFieldId",
            "TypeLayoutHash",
            "CheckedRichTextSemanticDigest",
            "CheckedMatchLimits",
            "ViewValue { ordinal",
            "HirSemanticPathStep",
            "ExpressionOwned(HirExpressionOwnedBodyRole)",
            "ChoiceLetStatement { path: HirNestedExpressionPath }",
            "DialogueLinePlanStatement",
            "role: HirLinePlanStatementRole",
            "Init { statement: u32 }",
            "HirExpressionOwnedChildEdge",
        ],
        "schema_anchors",
    )?;
    forbid_anchors(
        bundle,
        "SCHEMAS.md",
        &["group_path: Box<[u32]>", "ChoiceLetStatement { item: u32 }"],
        "schema_anchors",
    )?;
    require_anchors(
        bundle,
        "schemas/final_contract.rs",
        &[
            "HirSemanticPathStep",
            "ExpressionOwned(HirExpressionOwnedBodyRole)",
            "ChoiceLetStatement",
            "path: HirNestedExpressionPath",
            "role: HirLinePlanStatementRole",
            "Init { statement: u32 }",
            "HirExpressionOwnedChildEdge",
        ],
        "schema_anchors",
    )?;
    forbid_anchors(
        bundle,
        "schemas/final_contract.rs",
        &["group_path: Box<[u32]>", "ChoiceLetStatement { item: u32 }"],
        "schema_anchors",
    )?;
    require_anchors(
        bundle,
        "TRANSCRIPT_GRAMMAR.md",
        &[
            "All 27 resolution families",
            "0x0200..0x021A",
            "Statement and body transcripts",
            "match_payload",
            "checked_add",
        ],
        "schema_anchors",
    )?;
    require_anchors(
        bundle,
        "COVERAGE_ALGORITHM.md",
        &[
            "struct Matrix",
            "PatternVector",
            "specialization",
            "Symbolic sequence partitioning",
            "Other",
            "Never",
            "Choice",
            "checked_add",
        ],
        "schema_anchors",
    )?;
    require_anchors(
        bundle,
        "CUTS_TESTS_AND_DELETION.md",
        &["C1", "C2", "C3", "C4", "C5", "T07_DELETION_CLEAN"],
        "schema_anchors",
    )
}

fn validate_hir_path_contract(bundle: &Bundle) -> GateResult {
    let contract = &bundle.contract;
    require_u64(
        contract,
        "/hir_path_contract/declaration_body_role_variant_count",
        6,
        "hir_path_schema",
    )?;
    require_u64(
        contract,
        "/hir_path_contract/expression_owned_role_variant_count",
        14,
        "hir_path_schema",
    )?;
    require_u64(
        contract,
        "/hir_path_contract/expression_owned_logical_root_family_count",
        19,
        "hir_path_schema",
    )?;
    require_u64(
        contract,
        "/hir_path_contract/line_plan_statement_role_count",
        u64::try_from(LINE_PLAN_STATEMENT_ROLES.len())
            .map_err(|_| GateError::new("hir_path_schema", "role count overflow"))?,
        "hir_path_schema",
    )?;
    require_string_array(
        contract,
        "/hir_path_contract/line_plan_statement_roles",
        LINE_PLAN_STATEMENT_ROLES,
        "hir_path_schema",
    )?;
    require_str(
        contract,
        "/hir_path_contract/nested_path_authority",
        "HirNestedExpressionPath",
        "hir_path_schema",
    )?;
    for (field, expected) in [
        ("raw_group_path_authority", false),
        ("arbitrary_role_construction", false),
        (
            "choice_plan_cancel_shared_role_for_trigger_pattern_and_body",
            true,
        ),
        ("line_plan_group_kind_segments_preserved", true),
        (
            "expression_owned_body_appends_existing_body_child_role",
            true,
        ),
        ("view_callable_baseline_row_exists", false),
        ("view_callable_same_cut_pipeline_completion", true),
        ("view_retained_binding_remains_sole", true),
        (
            "view_retained_and_callable_join_by_item_module_snapshot",
            true,
        ),
    ] {
        require_bool(
            contract,
            &format!("/hir_path_contract/{field}"),
            expected,
            "hir_path_schema",
        )?;
    }
    require_str(
        contract,
        "/hir_path_contract/view_callable_entry",
        "CallableDeclarationKey::Existing through existing nonbinding callable pipeline",
        "hir_path_schema",
    )?;
    Ok(())
}

fn validate_transcript_contract(bundle: &Bundle) -> GateResult {
    require_str(
        &bundle.contract,
        "/transcript/algorithm",
        "BLAKE3 purpose-built canonical grammar",
        "transcript_contract",
    )?;
    require_u64(
        &bundle.contract,
        "/transcript/domain_version",
        1,
        "version_one",
    )?;
    for key in [
        "all_expression_resolutions_exact",
        "all_expression_shapes_exact",
        "all_pattern_shapes_exact",
        "all_statement_shapes_exact",
        "nested_match_payload_included",
        "statement_and_body_digests_included",
    ] {
        require_bool(
            &bundle.contract,
            &format!("/transcript/{key}"),
            true,
            "transcript_contract",
        )?;
    }
    for key in [
        "raw_ids",
        "spans",
        "source_spelling",
        "serde_transcript",
        "wildcard_success",
    ] {
        require_bool(
            &bundle.contract,
            &format!("/transcript/{key}"),
            false,
            "forbidden_authority",
        )?;
    }
    Ok(())
}

fn validate_coverage_contract(bundle: &Bundle) -> GateResult {
    require_str(
        &bundle.contract,
        "/coverage/owner",
        "arcweft-lang-sema::final_analysis private MatchCoverageAnalyzer",
        "coverage_algorithm",
    )?;
    require_str(
        &bundle.contract,
        "/coverage/algorithm",
        "bounded Maranget Matrix specialization default usefulness witness",
        "coverage_algorithm",
    )?;
    require_bool(
        &bundle.contract,
        "/coverage/private",
        true,
        "coverage_algorithm",
    )?;
    require_bool(
        &bundle.contract,
        "/coverage/checked_u64",
        true,
        "checked_limits",
    )?;
    require_bool(
        &bundle.contract,
        "/coverage/preallocation_accounting",
        true,
        "checked_limits",
    )?;
    require_string_array(
        &bundle.contract,
        "/coverage/domains",
        COVERAGE_DOMAINS,
        "coverage_algorithm",
    )?;
    require_string_array(
        &bundle.contract,
        "/coverage/limits",
        LIMITS,
        "checked_limits",
    )
}

fn validate_declaration_bridge(bundle: &Bundle) -> GateResult {
    require_bool(
        &bundle.contract,
        "/declaration_bridge/view_uses_existing_callable_declaration_key",
        true,
        "declaration_bridge",
    )?;
    require_str(
        &bundle.contract,
        "/declaration_bridge/view_value_role",
        "ViewValue { ordinal }",
        "declaration_bridge",
    )?;
    require_bool(
        &bundle.contract,
        "/declaration_bridge/view_missing_body_deleted",
        true,
        "declaration_bridge",
    )?;
    require_string_array(
        &bundle.contract,
        "/declaration_bridge/bodyless_owners_remain",
        &["ExternCapability", "TraitRequirement"],
        "declaration_bridge",
    )?;
    require_bool(
        &bundle.contract,
        "/declaration_bridge/persistent_match_site_identity",
        false,
        "declaration_bridge",
    )
}

fn validate_publication(bundle: &Bundle) -> GateResult {
    for key in [
        "compiler_local",
        "exhaustive_only",
        "atomic_on_error",
        "checked_match_ref_private",
    ] {
        require_bool(
            &bundle.contract,
            &format!("/publication/{key}"),
            true,
            "publication",
        )?;
    }
    require_bool(
        &bundle.contract,
        "/publication/checked_match_ref_serde",
        false,
        "publication",
    )
}

fn validate_non_goals(bundle: &Bundle) -> GateResult {
    let object = bundle
        .contract
        .pointer("/non_goals")
        .and_then(Value::as_object)
        .ok_or_else(|| GateError::new("forbidden_scope", "non_goals missing"))?;
    let expected = [
        "production_patch",
        "runtime_wire",
        "persistence",
        "task_plan_seal",
        "whole_catalog_digest",
        "legacy_reader",
        "compatibility_alias",
        "version_other_than_one",
        "returned_zip_emitted_by_repository_design",
    ];
    if object.len() != expected.len() {
        return Err(GateError::new("forbidden_scope", "non-goal count mismatch"));
    }
    expected
        .into_iter()
        .try_for_each(|key| match object.get(key) {
            Some(Value::Bool(false)) => Ok(()),
            value => Err(GateError::new(
                "forbidden_scope",
                format!("{key} must be false, got {value:?}"),
            )),
        })
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

fn validate_request_source(bundle: &Bundle) -> GateResult {
    let relative = bundle
        .contract
        .pointer("/request/source")
        .and_then(Value::as_str)
        .ok_or_else(|| GateError::new("request_mirror", "request source missing"))?;
    let source = fs::read(bundle.repo.join(relative))
        .map_err(|error| GateError::new("request_mirror", error.to_string()))?;
    let mirror = bundle
        .files
        .get("REQUEST.md")
        .ok_or_else(|| GateError::new("request_mirror", "REQUEST.md missing"))?;
    if source == *mirror {
        Ok(())
    } else {
        Err(GateError::new(
            "request_mirror",
            "maintained request and mirror differ",
        ))
    }
}

fn validate_rust_ast_inventory(bundle: &Bundle) -> GateResult {
    let checks = [
        (
            "crates/arcweft-lang-sema/src/final_analysis/model.rs",
            "CheckedExpressionResolution",
            EXPRESSION_RESOLUTIONS,
        ),
        (
            "crates/arcweft-lang-sema/src/final_analysis/model.rs",
            "CheckedValueResolution",
            VALUE_RESOLUTIONS,
        ),
        (
            "crates/arcweft-lang-sema/src/final_analysis/model.rs",
            "CheckedSelectResolution",
            SELECT_RESOLUTIONS,
        ),
        (
            "crates/arcweft-lang-sema/src/final_analysis/model.rs",
            "CheckedPatternResolution",
            PATTERN_RESOLUTIONS,
        ),
        (
            "crates/arcweft-lang-hir/src/expr.rs",
            "HirExprKind",
            EXPRESSION_FAMILIES,
        ),
        (
            "crates/arcweft-lang-hir/src/pattern.rs",
            "HirPatternKind",
            PATTERN_FAMILIES,
        ),
        (
            "crates/arcweft-lang-hir/src/stmt.rs",
            "HirStmtKind",
            STATEMENT_FAMILIES,
        ),
        (
            "crates/arcweft-lang-hir/src/body_edges.rs",
            "HirBodyChildRole",
            BODY_CHILD_ROLES,
        ),
        (
            "crates/arcweft-lang-hir/src/stmt/child_edges.rs",
            "HirStatementBodyRole",
            STATEMENT_BODY_ROLES,
        ),
    ];
    checks
        .into_iter()
        .try_for_each(|(path, enum_name, expected)| {
            let source = git(&bundle.repo, &["show", &format!("HEAD:{path}")])?;
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
            let expected = expected.iter().map(ToString::to_string).collect::<Vec<_>>();
            match actual {
                Some(actual) if actual == expected => Ok(()),
                actual => Err(GateError::new(
                    "source_inventory",
                    format!("{enum_name}: expected {expected:?}, got {actual:?}"),
                )),
            }
        })
}

fn validate_cargo_metadata(bundle: &Bundle) -> GateResult {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&bundle.repo)
        .output()
        .map_err(|error| GateError::new("cargo_metadata", error.to_string()))?;
    if !output.status.success() {
        return Err(GateError::new(
            "cargo_metadata",
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| GateError::new("cargo_metadata", error.to_string()))?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| GateError::new("cargo_metadata", "packages missing"))?;
    let dependencies = |name: &str| -> GateResult<BTreeSet<String>> {
        let package = packages
            .iter()
            .find(|package| package["name"].as_str() == Some(name))
            .ok_or_else(|| GateError::new("cargo_metadata", format!("missing {name}")))?;
        package["dependencies"]
            .as_array()
            .ok_or_else(|| {
                GateError::new("cargo_metadata", format!("dependencies missing for {name}"))
            })?
            .iter()
            .map(|dependency| {
                dependency["name"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| GateError::new("cargo_metadata", "dependency name missing"))
            })
            .collect()
    };
    let sema = dependencies("arcweft-lang-sema")?;
    for required in [
        "arcweft-lang-hir",
        "arcweft-core",
        "arcweft-character",
        "arcweft-view",
    ] {
        if !sema.contains(required) {
            return Err(GateError::new(
                "cargo_metadata",
                format!("sema missing dependency {required}"),
            ));
        }
    }
    for lower in [
        "arcweft-lang-hir",
        "arcweft-core",
        "arcweft-character",
        "arcweft-view",
    ] {
        if dependencies(lower)?.contains("arcweft-lang-sema") {
            return Err(GateError::new(
                "cargo_metadata",
                format!("reverse dependency {lower} -> sema"),
            ));
        }
    }
    Ok(())
}

fn parse_json_file(files: &BTreeMap<String, Vec<u8>>, path: &str) -> GateResult<Value> {
    let bytes = files
        .get(path)
        .ok_or_else(|| GateError::new("required_files", format!("missing {path}")))?;
    serde_json::from_slice(bytes)
        .map_err(|error| GateError::new("machine_contract", format!("{path}: {error}")))
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

fn forbid_anchors(bundle: &Bundle, path: &str, anchors: &[&str], gate: &'static str) -> GateResult {
    let text = text_file(bundle, path, gate)?;
    anchors.iter().try_for_each(|anchor| {
        if text.contains(anchor) {
            Err(GateError::new(
                gate,
                format!("{path} retains forbidden anchor {anchor}"),
            ))
        } else {
            Ok(())
        }
    })
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

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn git(repo: &Path, args: &[&str]) -> GateResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|error| GateError::new("repository", error.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(GateError::new(
            "repository",
            format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ))
    }
}
