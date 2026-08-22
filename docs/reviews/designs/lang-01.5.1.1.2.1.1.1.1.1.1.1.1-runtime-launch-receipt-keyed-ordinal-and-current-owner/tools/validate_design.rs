#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "validate-runtime-launch-design"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
anyhow = "1.0.102"
clap = { version = "4.6.1", features = ["derive"] }
quote = "1.0.45"
serde_json = "1.0.150"
syn = { version = "2.0.117", features = ["full", "visit"] }
---

//! Read-only repository-aware validator for the accepted Lang-01.5...1.1.1 design.
//!
//! This parses Rust syntax trees and Cargo metadata. It intentionally does not
//! modify the checkout and is not a substitute for implementation behavior,
//! codec, compile-fail, or workspace tests.

use anyhow::{bail, Context, Result};
use clap::Parser;
use quote::ToTokens;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use syn::{Fields, ImplItem, Item, ItemEnum, ItemImpl, ItemStruct, ItemTrait, Type, Visibility};

const EXPECTED_HEAD: &str = "61779d1432b902efc2d19041a7326f3c1319828a";

const CROSS_CRATE_METHODS: &[(&str, &str)] = &[
    ("NeedProducerSpec", "new"),
    ("NeedProducerSpec", "instance_key"),
    ("NeedProducerSpec", "family"),
    ("NeedProducerSpec", "contract"),
    ("NeedProducerSpec", "plan"),
    ("NeedProducerSpec", "producer_site"),
    ("NeedProducerSpec", "payload_type"),
    ("NeedProducerSpec", "arguments"),
    ("TaskCorrelation", "derive"),
    ("TaskCorrelation", "validate"),
    ("TaskCorrelation", "generation"),
    ("TaskCorrelation", "producer"),
    ("TaskCorrelation", "policy"),
    ("TaskCorrelation", "ordinal"),
    ("TaskCorrelation", "need_id"),
    ("TaskCorrelation", "task_key"),
    ("TaskCorrelation", "task_id"),
    ("TaskSpec", "try_new"),
    ("TaskSpec", "validate"),
    ("TaskSpec", "producer"),
    ("TaskSpec", "class"),
    ("TaskSpec", "priority"),
    ("TaskSpec", "cancel_scope"),
    ("TaskSpec", "policy"),
    ("TaskSpec", "outcome"),
    ("TaskSpec", "execution"),
    ("TaskSpec", "debug"),
    ("RuntimeTimeoutRequest", "new"),
    ("RuntimeTimeoutRequest", "source"),
    ("RuntimeTimeoutRequest", "requested_limit"),
    ("RuntimeTimeoutRequest", "contract"),
    ("TaskDebugMetadata", "new"),
    ("TaskDebugMetadata", "label"),
    ("TaskDebugMetadata", "origin"),
    ("NamedRuntimeValue", "new"),
    ("NamedRuntimeValue", "name"),
    ("NamedRuntimeValue", "value"),
    ("RuntimeNeedHandle", "try_reusable_join"),
    ("RuntimeNeedHandle", "try_from_accepted_launch"),
    ("RuntimeNeedHandle", "validate_use"),
    ("RuntimeNeedHandle", "correlation"),
    ("RuntimeNeedHandle", "producer"),
    ("RuntimeNeedHandle", "outcome"),
    ("RuntimeNeedHandle", "state"),
    ("RuntimeNeedHandle", "need_id"),
    ("PlannedObserverIds", "ids"),
    ("PlannedObserverIds", "next"),
    ("RuntimeGenerationJournal", "new"),
    ("RuntimeGenerationJournal", "generation"),
    ("RuntimeGenerationJournal", "revision"),
    ("RuntimeGenerationJournal", "task"),
    ("RuntimeGenerationJournal", "need"),
    ("RuntimeGenerationJournal", "observer"),
    ("RuntimeGenerationJournal", "scope"),
    ("RuntimeGenerationJournal", "next_always_start_ordinal"),
    ("RuntimeGenerationJournal", "accepted_launch_receipt"),
    ("RuntimeGenerationJournal", "plan_observer_ids"),
    ("RuntimeGenerationJournal", "begin_transaction"),
    ("RuntimeGenerationJournal", "apply_after_image"),
    ("JournalTransaction", "ensure_task"),
    ("JournalTransaction", "plan_restore"),
    ("JournalTransaction", "plan_rebind"),
    ("JournalTransaction", "plan_cancel"),
    ("JournalTransaction", "ensure_results"),
    ("JournalTransaction", "host_launch_batch"),
    ("JournalTransaction", "host_restore_batch"),
    ("JournalTransaction", "host_rebind_batch"),
    ("JournalTransaction", "host_cancel_batch"),
    ("JournalTransaction", "accept_launch_receipt"),
    ("JournalTransaction", "accept_restore_receipt"),
    ("JournalTransaction", "accept_rebind_receipt"),
    ("JournalTransaction", "seal"),
    ("AppliedJournalBatch", "generation"),
    ("AppliedJournalBatch", "revision"),
    ("AppliedJournalBatch", "ensure_results"),
    ("AppliedJournalBatch", "into_results"),
    ("JournalEnsureResult", "source_index"),
    ("JournalEnsureResult", "correlation"),
    ("JournalEnsureResult", "disposition"),
    ("JournalEnsureResult", "observer"),
    ("AppliedEnsureResult", "source_index"),
    ("AppliedEnsureResult", "handle"),
    ("AppliedEnsureResult", "observer"),
    ("TaskJournalRow", "correlation"),
    ("TaskJournalRow", "spec"),
    ("TaskJournalRow", "lifecycle"),
    ("TaskJournalRow", "host"),
    ("TaskJournalRow", "last_publication"),
    ("AcceptedHostLaunch", "operation"),
    ("AcceptedHostLaunch", "launch"),
    ("AcceptedHostLaunch", "cancellation"),
    ("AcceptedHostLaunch", "restart"),
    ("RuntimeNeedCell", "correlation"),
    ("RuntimeNeedCell", "producer"),
    ("RuntimeNeedCell", "outcome"),
    ("RuntimeNeedCell", "state"),
    ("RuntimeNeedCell", "observers"),
    ("TaskObserver", "id"),
    ("TaskObserver", "need"),
    ("TaskObserver", "kind"),
    ("TaskObserver", "state"),
    ("TaskObserver", "last_cursor"),
    ("RuntimeTaskScope", "id"),
    ("RuntimeTaskScope", "tasks"),
    ("RuntimeTaskScope", "cancellation_requested"),
    ("HostTaskLaunchBatch", "try_new"),
    ("HostTaskLaunchBatch", "generation"),
    ("HostTaskLaunchBatch", "rows"),
    ("HostTaskLaunchRow", "new"),
    ("HostTaskLaunchRow", "source_index"),
    ("HostTaskLaunchRow", "correlation"),
    ("HostTaskLaunchRow", "operation"),
    ("HostTaskLaunchRow", "request"),
    ("HostTaskLaunchRow", "outcome"),
    ("HostTaskLaunchRow", "restart"),
    ("HostTaskLaunchReceiptRow", "new"),
    ("HostTaskLaunchReceiptRow", "source_index"),
    ("HostTaskLaunchReceiptRow", "correlation"),
    ("HostTaskLaunchReceiptRow", "operation"),
    ("HostTaskLaunchReceiptRow", "launch"),
    ("HostTaskLaunchReceiptRow", "cancellation"),
    ("HostTaskLaunchReceipt", "try_for_batch"),
    ("HostTaskLaunchReceipt", "generation"),
    ("HostTaskLaunchReceipt", "rows"),
    ("PreparedLaunchBatch", "try_new"),
    ("PreparedLaunchBatch", "receipt"),
    ("PreparedLaunchBatch", "into_parts"),
    ("HostTaskRestoreBatch", "try_new"),
    ("HostTaskRestoreBatch", "generation"),
    ("HostTaskRestoreBatch", "rows"),
    ("HostTaskRestoreRow", "new"),
    ("HostTaskRestoreRow", "correlation"),
    ("HostTaskRestoreRow", "complete_spec"),
    ("HostTaskRestoreRow", "operation"),
    ("HostTaskRestoreRow", "launch"),
    ("HostTaskRestoreRow", "cancellation"),
    ("HostTaskRestoreReceiptRow", "new"),
    ("HostTaskRestoreReceiptRow", "correlation"),
    ("HostTaskRestoreReceiptRow", "operation"),
    ("HostTaskRestoreReceiptRow", "launch"),
    ("HostTaskRestoreReceiptRow", "cancellation"),
    ("HostTaskRestoreReceipt", "try_for_batch"),
    ("HostTaskRestoreReceipt", "generation"),
    ("HostTaskRestoreReceipt", "rows"),
    ("PreparedRestoreBatch", "try_new"),
    ("PreparedRestoreBatch", "receipt"),
    ("PreparedRestoreBatch", "into_parts"),
    ("HostTaskRebindBatch", "try_new"),
    ("HostTaskRebindBatch", "old_generation"),
    ("HostTaskRebindBatch", "new_generation"),
    ("HostTaskRebindBatch", "rows"),
    ("HostTaskRebindRow", "new"),
    ("HostTaskRebindRow", "old_correlation"),
    ("HostTaskRebindRow", "new_correlation"),
    ("HostTaskRebindRow", "operation"),
    ("HostTaskRebindRow", "launch"),
    ("HostTaskRebindRow", "cancellation"),
    ("HostTaskRebindReceiptRow", "new"),
    ("HostTaskRebindReceiptRow", "old_correlation"),
    ("HostTaskRebindReceiptRow", "new_correlation"),
    ("HostTaskRebindReceiptRow", "operation"),
    ("HostTaskRebindReceiptRow", "launch"),
    ("HostTaskRebindReceiptRow", "cancellation"),
    ("HostTaskRebindReceipt", "try_for_batch"),
    ("HostTaskRebindReceipt", "old_generation"),
    ("HostTaskRebindReceipt", "new_generation"),
    ("HostTaskRebindReceipt", "rows"),
    ("PreparedRebindBatch", "try_new"),
    ("PreparedRebindBatch", "receipt"),
    ("PreparedRebindBatch", "into_parts"),
    ("HostTaskCancelBatch", "try_new"),
    ("HostTaskCancelBatch", "generation"),
    ("HostTaskCancelBatch", "rows"),
    ("HostTaskCancelRow", "new"),
    ("HostTaskCancelRow", "command"),
    ("HostTaskCancelRow", "correlation"),
    ("HostTaskCancelRow", "operation"),
    ("HostTaskCancelRow", "launch"),
    ("HostTaskCancelRow", "cancellation"),
    ("HostTaskCancelRow", "reason"),
    ("PreparedCancelBatch", "new"),
    ("PreparedCancelBatch", "batch"),
    ("PreparedCancelBatch", "into_parts"),
    ("HostRouteId", "new"),
    ("HostRouteId", "get"),
    ("HostLaunchCapability", "new"),
    ("HostLaunchCapability", "route"),
    ("HostLaunchCapability", "id"),
    ("HostCancellationCapability", "new"),
    ("HostCancellationCapability", "route"),
    ("HostCancellationCapability", "id"),
    ("HostOperationId", "new"),
    ("HostOperationId", "get"),
    ("HostCancelCommandId", "derive"),
    ("HostOperationCatalog", "try_new"),
    ("HostOperationCatalog", "digest"),
    ("HostOperationCatalog", "rows"),
    ("HostOperationCatalog", "resolve"),
    ("HostOperationCatalog", "validate_launch_receipt"),
    ("HostOperationCatalogRow", "try_new"),
    ("HostOperationCatalogRow", "identity"),
    ("HostOperationCatalogRow", "capability"),
    ("HostOperationCatalogRow", "request"),
    ("HostOperationCatalogRow", "route"),
    ("HostOperationCatalogRow", "restart"),
    ("HostOperationCatalogRow", "cancellation"),
    ("HostTaskRequestContract", "try_new"),
    ("HostTaskRequestContract", "kind"),
    ("HostTaskRequestContract", "positional"),
    ("HostTaskRequestContract", "named"),
    ("HostTaskRequestContract", "spread"),
    ("HostNamedArgumentContract", "new"),
    ("HostNamedArgumentContract", "name"),
    ("HostNamedArgumentContract", "ty"),
    ("HostNamedArgumentContract", "required"),
    ("TaskValidationAuthority", "try_new"),
    ("TaskValidationAuthority", "generation"),
    ("RuntimeAwaitManyAggregateRequest", "new"),
    ("RuntimeAwaitManyAggregateRequest", "captured"),
    ("RuntimeAwaitManyAggregateRequest", "source_items"),
    ("RuntimeAwaitManyAggregateRequest", "child"),
    ("RuntimeAwaitManyAggregateRequest", "limit"),
    ("HirExpressionChildEdge", "child"),
    ("HirExpressionChildEdge", "role"),
];

const PRIVATE_PROTOCOL_TYPES: &[&str] = &[
    "GenerationId",
    "TaskLaunchOrdinal",
    "NeedProducerInstanceKey",
    "NeedId",
    "TaskKey",
    "TaskId",
    "NeedProducerContractDigest",
    "TaskPlanSemanticDigest",
    "RuntimeTypeSemanticDigest",
    "NeedTimeoutContractDigest",
    "NeedProducerSpec",
    "TaskCorrelation",
    "TaskSpec",
    "TaskPriority",
    "RuntimeTimeoutRequest",
    "TaskDebugMetadata",
    "NamedRuntimeValue",
    "RuntimeNeedHandle",
    "AcceptedTaskLaunchReceipt",
    "PlannedObserverIds",
    "TaskObserverId",
    "RuntimeGenerationJournal",
    "JournalTransaction",
    "SealedJournalAfterImage",
    "AppliedJournalBatch",
    "JournalEnsureResult",
    "AppliedEnsureResult",
    "TaskJournalRow",
    "AcceptedHostLaunch",
    "RuntimeNeedCell",
    "TaskObserver",
    "RuntimeTaskScope",
    "HostTaskLaunchBatch",
    "HostTaskLaunchRow",
    "HostTaskLaunchReceipt",
    "HostTaskLaunchReceiptRow",
    "PreparedLaunchBatch",
    "HostTaskRestoreBatch",
    "HostTaskRestoreRow",
    "HostTaskRestoreReceipt",
    "HostTaskRestoreReceiptRow",
    "PreparedRestoreBatch",
    "HostTaskRebindBatch",
    "HostTaskRebindRow",
    "HostTaskRebindReceipt",
    "HostTaskRebindReceiptRow",
    "PreparedRebindBatch",
    "HostTaskCancelBatch",
    "HostTaskCancelRow",
    "PreparedCancelBatch",
    "HostRouteId",
    "HostLaunchCapability",
    "HostCancellationCapability",
    "HostOperationCatalog",
    "HostOperationCatalogDigest",
    "HostOperationId",
    "HostCancelCommandId",
    "HostOperationCatalogRow",
    "HostTaskRequestContract",
    "HostNamedArgumentContract",
    "TaskValidationAuthority",
    "RuntimeAwaitManyAggregateRequest",
    "HirExpressionChildEdge",
];

const ADAPTER_METHODS: &[&str] = &[
    "prepare_launch",
    "commit_launch",
    "rollback_launch",
    "prepare_restore",
    "commit_restore",
    "rollback_restore",
    "prepare_rebind",
    "commit_rebind",
    "rollback_rebind",
    "prepare_cancel",
    "commit_cancel",
    "rollback_cancel",
];

#[derive(Parser)]
struct Args {
    #[arg(long)]
    repository_root: PathBuf,
    #[arg(long, default_value = ".")]
    design_root: PathBuf,
    #[arg(long)]
    self_test: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    message: String,
}

impl Failure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let repository_root = args
        .repository_root
        .canonicalize()
        .context("canonicalize repository root")?;
    let design_root = args
        .design_root
        .canonicalize()
        .context("canonicalize design root")?;

    let contract_path = design_root.join("machine/final_contract.json");
    let schema_path = design_root.join("schemas/final_contract.rs");
    let contract: Value = serde_json::from_slice(
        &fs::read(&contract_path).with_context(|| contract_path.display().to_string())?,
    )
    .context("parse machine contract")?;
    let schema_text =
        fs::read_to_string(&schema_path).with_context(|| schema_path.display().to_string())?;
    let schema = syn::parse_file(&schema_text).context("parse Rust-shaped schema")?;

    let mut failures = validate_model(&contract);
    failures.extend(validate_schema(&contract, &schema));
    failures.extend(validate_repository(&repository_root, &contract)?);

    let mut negative_case_count = None;
    if args.self_test {
        let (negative_failures, case_count) = run_negative_self_tests(&contract, &schema_text);
        failures.extend(negative_failures);
        negative_case_count = Some(case_count);
    }

    if !failures.is_empty() {
        for failure in &failures {
            eprintln!("FAIL {} {}", failure.code, failure.message);
        }
        bail!("{} validation failure(s)", failures.len());
    }

    println!("PASS sequence=Lang-01.5.1.1.2.1.1.1.1.1.1.1.1");
    println!("head={EXPECTED_HEAD}");
    println!("model=PASS schema_ast=PASS repository_ast=PASS cargo_graph=PASS");
    if let Some(case_count) = negative_case_count {
        println!("negative_self_tests={case_count}/{case_count} PASS");
    }
    Ok(())
}

fn validate_model(contract: &Value) -> Vec<Failure> {
    let mut failures = Vec::new();
    if text(contract, "/status") != Some("READY_FOR_IMPLEMENTATION")
        || number(contract, "/open_questions") != Some(0)
    {
        failures.push(Failure::new(
            "STS001",
            "status/open questions are not final",
        ));
    }
    if bool_at(contract, "/numeric_awbc_changes") != Some(false) {
        failures.push(Failure::new("AWB001", "numeric AWBC change was reopened"));
    }
    let invalid_fixed_ids = strings(contract, "/zero_policy/all_zero_invalid_fixed_ids");
    let valid_digests = strings(contract, "/zero_policy/all_zero_valid_semantic_digests");
    let nonzero_scalars = strings(contract, "/zero_policy/nonzero_scalar_ids");
    if text(contract, "/zero_policy/generation_representation") != Some("u64")
        || bool_at(contract, "/zero_policy/generation_zero_valid") != Some(true)
        || bool_at(contract, "/zero_policy/join_launch_ordinal_zero_valid") != Some(true)
        || text(contract, "/zero_policy/always_start_launch_ordinal") != Some("NonZeroU64")
        || text(contract, "/zero_policy/absence_representation") != Some("Option")
        || text(contract, "/zero_policy/other_newtypes/TaskPriority") != Some("i32_zero_valid")
        || text(
            contract,
            "/zero_policy/other_newtypes/HirNestedExpressionPath",
        ) != Some("nonempty_boxed_segments")
        || text(contract, "/zero_policy/other_newtypes/CheckedNestedPathV1")
            != Some("nonempty_boxed_segments")
        || text(contract, "/zero_policy/need_producer_spec_constructor")
            != Some("NeedProducerSpec::new")
        || bool_at(
            contract,
            "/zero_policy/need_producer_spec_constructor_fallible",
        ) != Some(false)
        || bool_at(contract, "/zero_policy/need_producer_spec_error_type") != Some(false)
        || text(contract, "/zero_policy/semantic_digest_raw_constructor")
            != Some("from_bytes_accepts_every_value")
        || bool_at(contract, "/zero_policy/fixed_identity_raw_constructor") != Some(false)
        || text(contract, "/zero_policy/task_validation_generation_error")
            != Some("GenerationMismatch")
        || invalid_fixed_ids
            != [
                "NeedProducerInstanceKey",
                "NeedId",
                "TaskKey",
                "TaskId",
                "HostCancelCommandId",
            ]
        || valid_digests
            != [
                "NeedProducerContractDigest",
                "TaskPlanSemanticDigest",
                "RuntimeTypeSemanticDigest",
                "NeedTimeoutContractDigest",
                "HostOperationCatalogDigest",
                "AwbcDigest",
                "RuntimeValueDigest",
            ]
        || nonzero_scalars
            != [
                "HostRouteId",
                "HostOperationId",
                "TaskObserverId",
                "HostLaunchCapability.id",
                "HostCancellationCapability.id",
                "AlwaysStartOrdinalCounter.next",
            ]
    {
        failures.push(Failure::new(
            "ZERO001",
            "zero-valid and zero-invalid domains changed",
        ));
    }
    if strings(contract, "/constructor_reachability/TaskLaunchOrdinal") != ["JOIN", "get"]
        || strings(contract, "/constructor_reachability/TaskPriority") != ["new", "get"]
        || strings(contract, "/constructor_reachability/HostRouteId") != ["new(NonZeroU32)", "get"]
        || strings(contract, "/constructor_reachability/HostOperationId")
            != ["new(NonZeroU32)", "get"]
        || strings(contract, "/constructor_reachability/semantic_digests")
            != ["from_bytes", "as_bytes"]
        || text(contract, "/constructor_reachability/fixed_ids")
            != Some("derived_only_no_raw_constructor")
    {
        failures.push(Failure::new(
            "CTOR001",
            "private-field newtype constructor reachability changed",
        ));
    }
    if text(contract, "/cross_crate_protocol/field_policy") != Some("private")
        || bool_at(
            contract,
            "/cross_crate_protocol/raw_journal_row_construction",
        ) != Some(false)
        || text(contract, "/cross_crate_protocol/task_spec/constructor")
            != Some("TaskSpec::try_new")
        || bool_at(contract, "/cross_crate_protocol/task_spec/validated") != Some(true)
        || strings(contract, "/cross_crate_protocol/task_spec/getters")
            != [
                "producer",
                "class",
                "priority",
                "cancel_scope",
                "policy",
                "outcome",
                "execution",
                "debug",
            ]
        || strings(contract, "/cross_crate_protocol/producer_getters")
            != [
                "family",
                "contract",
                "plan",
                "producer_site",
                "payload_type",
                "arguments",
            ]
        || strings(contract, "/cross_crate_protocol/correlation_getters")
            != [
                "generation",
                "producer",
                "policy",
                "ordinal",
                "need_id",
                "task_key",
                "task_id",
            ]
        || text(contract, "/cross_crate_protocol/journal/staging_owner")
            != Some("JournalTransaction")
        || text(contract, "/cross_crate_protocol/journal/after_image")
            != Some("SealedJournalAfterImage")
        || text(contract, "/cross_crate_protocol/journal/apply")
            != Some("RuntimeGenerationJournal::apply_after_image")
        || text(contract, "/cross_crate_protocol/journal/row_mutation") != Some("core_only")
        || text(contract, "/cross_crate_protocol/journal/observer_mutation")
            != Some("same_after_image")
        || bool_at(contract, "/cross_crate_protocol/journal/revision_persisted") != Some(false)
        || number(contract, "/cross_crate_protocol/journal/restore_revision") != Some(0)
        || bool_at(
            contract,
            "/cross_crate_protocol/journal/accepted_receipt_after_apply_only",
        ) != Some(true)
        || text(
            contract,
            "/cross_crate_protocol/scheduler_runtime_after_image",
        ) != Some("scheduler_private_infallible_swap")
        || bool_at(
            contract,
            "/cross_crate_protocol/adapter_prepare_before_apply",
        ) != Some(true)
        || bool_at(
            contract,
            "/cross_crate_protocol/adapter_commit_after_both_applies",
        ) != Some(true)
        || bool_at(
            contract,
            "/cross_crate_protocol/post_journal_apply_fallible_steps",
        ) != Some(false)
        || strings(contract, "/cross_crate_protocol/scheduler_coordinators")
            != [
                "apply_ensure_plan",
                "apply_restore_plan",
                "apply_rebind_plan",
                "apply_cancel_plan",
            ]
        || strings(contract, "/cross_crate_protocol/hir_edge_read_api") != ["child", "role"]
        || bool_at(contract, "/cross_crate_protocol/public_fields_allowed") != Some(false)
    {
        failures.push(Failure::new(
            "API001",
            "cross-crate create/read/mutate protocol changed",
        ));
    }
    if text(contract, "/make_need_handle/execution") != Some("Host")
        || text(contract, "/make_need_handle/policy") != Some("JoinSameKey")
        || text(contract, "/make_need_handle/state") != Some("ReusableJoin")
        || bool_at(contract, "/make_need_handle/lazy") != Some(true)
        || bool_at(contract, "/make_need_handle/journal_mutation") != Some(false)
        || bool_at(contract, "/make_need_handle/adapter_mutation") != Some(false)
    {
        failures.push(Failure::new("MAKE001", "MakeNeedHandle truth row changed"));
    }
    if text(contract, "/always_start_ordinals/live_type")
        != Some("BTreeMap<NeedProducerInstanceKey, NonZeroU64>")
        || text(contract, "/always_start_ordinals/batch_after_image")
            != Some("BTreeMap<NeedProducerInstanceKey, NonZeroU64>")
        || number(contract, "/always_start_ordinals/absence_next") != Some(1)
    {
        failures.push(Failure::new(
            "ORD001",
            "AlwaysStart allocation is not producer-keyed",
        ));
    }
    if text(contract, "/accepted_launch/receipt_visibility") != Some("pub")
        || text(contract, "/accepted_launch/receipt_fields_visibility") != Some("private")
        || text(contract, "/accepted_launch/constructor_visibility") != Some("pub")
        || text(contract, "/accepted_launch/handle_constructor_visibility") != Some("pub")
        || bool_at(contract, "/accepted_launch/raw_field_constructor") != Some(false)
    {
        failures.push(Failure::new(
            "VIS001",
            "accepted-launch cross-crate visibility is invalid",
        ));
    }
    if text(contract, "/adapter/prepare_result")
        != Some("PreparedLaunchBatch<Self::PreparedLaunchToken>")
        || text(contract, "/adapter/inspectable_receipt") != Some("HostTaskLaunchReceipt")
        || bool_at(contract, "/adapter/commit_fallible") != Some(false)
    {
        failures.push(Failure::new(
            "ADP001",
            "adapter prepare does not return the selected receipt",
        ));
    }
    if text(contract, "/snapshot/authority") != Some("RuntimeSnapshotAuthority")
        || bool_at(contract, "/snapshot/authority_serialized") != Some(false)
        || bool_at(contract, "/snapshot/per_function_authority") != Some(false)
        || bool_at(contract, "/snapshot/dense_redefined") != Some(false)
        || text(contract, "/snapshot/dense_units") != Some("Units(usize)")
        || text(contract, "/snapshot/dense_bool") != Some("Bool(DenseSeqStorage<bool>)")
    {
        failures.push(Failure::new(
            "SNP001",
            "snapshot authority/current carrier decision changed",
        ));
    }
    if number(contract, "/variants/option/Some") != Some(0)
        || number(contract, "/variants/option/None") != Some(1)
        || number(contract, "/variants/result/Ok") != Some(0)
        || number(contract, "/variants/result/Err") != Some(1)
    {
        failures.push(Failure::new("VAR001", "Option/Result ordinals changed"));
    }
    if bool_at(contract, "/variants/agent_builtin_blanket") != Some(false)
        || text(contract, "/variants/agent_builtin/Diagnostics")
            != Some("SnapshotClone:Agent.Diagnostics")
        || text(contract, "/variants/agent_builtin/ViewportPoint")
            != Some("SnapshotClone:Agent.ViewportPoint")
    {
        failures.push(Failure::new(
            "OWN001",
            "AgentBuiltin is not nested-exhaustive",
        ));
    }
    if text(contract, "/callable/catalog") != Some("CheckedCallableCatalog")
        || bool_at(contract, "/callable/invented_v1_catalog") != Some(false)
        || bool_at(contract, "/task_validation/task_contract_catalog_v1") != Some(false)
    {
        failures.push(Failure::new(
            "CAT001",
            "parallel callable/task catalog selected",
        ));
    }
    if bool_at(contract, "/match_edges/direct_children_is_projection") != Some(true)
        || number(contract, "/match_edges/family_count") != Some(38)
    {
        failures.push(Failure::new("LAY001", "HIR edge authority changed"));
    }
    failures
}

fn validate_schema(contract: &Value, file: &syn::File) -> Vec<Failure> {
    let mut failures = Vec::new();
    let definitions = definitions(file);
    for required in strings(contract, "/required_schema_types") {
        if !definitions.contains(required.as_str()) {
            failures.push(Failure::new(
                "SCH101",
                format!("missing schema type {required}"),
            ));
        }
    }
    for forbidden in strings(contract, "/forbidden_schema_types") {
        if definitions.contains(forbidden.as_str()) {
            failures.push(Failure::new(
                "SCH102",
                format!("forbidden schema type {forbidden}"),
            ));
        }
    }

    let generation = tuple_field_type(file, "GenerationId");
    let ordinal = tuple_field_type(file, "TaskLaunchOrdinal");
    if generation.as_deref() != Some("u64") || ordinal.as_deref() != Some("u64") {
        failures.push(Failure::new(
            "ZERO101",
            "GenerationId and TaskLaunchOrdinal must preserve zero with u64 owners",
        ));
    }
    for name in [
        "NeedProducerInstanceKey",
        "NeedId",
        "TaskKey",
        "TaskId",
        "HostCancelCommandId",
        "NeedProducerContractDigest",
        "TaskPlanSemanticDigest",
        "RuntimeTypeSemanticDigest",
        "NeedTimeoutContractDigest",
        "HostOperationCatalogDigest",
    ] {
        if tuple_field_type(file, name).as_deref() != Some("[u8 ; 32]") {
            failures.push(Failure::new(
                "ZERO102",
                format!("{name} must retain its exact 32-byte owner"),
            ));
        }
    }

    for (owner, method) in CROSS_CRATE_METHODS {
        if !method_visibility(file, owner, method).is_some_and(is_public) {
            failures.push(Failure::new(
                "API101",
                format!("required cross-crate API {owner}::{method} is not public"),
            ));
        }
    }
    for name in PRIVATE_PROTOCOL_TYPES {
        let Some(item) = find_struct(file, name) else {
            failures.push(Failure::new(
                "API102",
                format!("missing protected protocol type {name}"),
            ));
            continue;
        };
        if fields(item)
            .iter()
            .any(|field| !matches!(field.vis, Visibility::Inherited))
        {
            failures.push(Failure::new(
                "API102",
                format!("protected protocol fields are not private on {name}"),
            ));
        }
    }
    for method in ADAPTER_METHODS {
        if trait_method(file, "TaskLaunchAdapter", method).is_none() {
            failures.push(Failure::new(
                "API107",
                format!("missing adapter protocol method {method}"),
            ));
        }
    }
    for name in [
        "TaskJournalRow",
        "AcceptedHostLaunch",
        "RuntimeNeedCell",
        "TaskObserver",
        "RuntimeTaskScope",
        "SealedJournalAfterImage",
    ] {
        if ["new", "try_new", "from_parts", "raw"]
            .iter()
            .any(|method| impl_method(file, name, method).is_some())
        {
            failures.push(Failure::new(
                "API103",
                format!("core-owned journal authority {name} gained a raw constructor"),
            ));
        }
    }
    if definitions.contains("RuntimeJournalBatchDelta")
        || definitions.contains("RuntimeObserverBatchDelta")
        || !definitions.contains("SealedJournalAfterImage")
        || !definitions.contains("SchedulerRuntimeAfterImage")
    {
        failures.push(Failure::new(
            "API104",
            "scheduler duplicates core journal/observer mutation authority",
        ));
    }
    let task_spec_constructor =
        impl_method(file, "TaskSpec", "try_new").map(|method| normalized_tokens(&method.sig));
    let journal_apply = impl_method(file, "RuntimeGenerationJournal", "apply_after_image")
        .map(|method| normalized_tokens(&method.sig));
    if !task_spec_constructor
        .as_deref()
        .is_some_and(|sig| sig.contains("TaskValidationAuthority") && sig.contains("Result"))
        || !journal_apply.as_deref().is_some_and(|sig| {
            sig.contains("SealedJournalAfterImage") && sig.contains("AppliedJournalBatch")
        })
    {
        failures.push(Failure::new(
            "API105",
            "validated TaskSpec or sealed journal apply signature changed",
        ));
    }
    if impl_method(file, "RuntimeScheduler", "apply_runtime_after_image").is_none() {
        failures.push(Failure::new(
            "API106",
            "missing scheduler-private infallible after-image swap",
        ));
    }
    for (method, rollback, commit) in [
        ("apply_ensure_plan", "rollback_launch", "commit_launch"),
        ("apply_restore_plan", "rollback_restore", "commit_restore"),
        ("apply_rebind_plan", "rollback_rebind", "commit_rebind"),
        ("apply_cancel_plan", "rollback_cancel", "commit_cancel"),
    ] {
        let Some(method) = impl_method(file, "RuntimeScheduler", method) else {
            failures.push(Failure::new(
                "API106",
                format!("missing scheduler coordinator {method}"),
            ));
            continue;
        };
        let tokens = normalized_tokens(method);
        let positions = (
            tokens.find("apply_after_image"),
            tokens.find(rollback),
            tokens.find("apply_runtime_after_image"),
            tokens.find(commit),
        );
        let ordered = matches!(
            positions,
            (Some(apply), Some(rollback), Some(runtime), Some(commit))
                if apply < rollback && rollback < runtime && runtime < commit
        );
        let no_fallible_tail = positions
            .2
            .is_some_and(|runtime| !tokens[runtime..].contains('?'));
        if !ordered || !tokens.contains("return Err") || !no_fallible_tail {
            failures.push(Failure::new(
                "API106",
                "scheduler coordinator lost rollback/apply/commit ordering or gained a fallible post-apply step",
            ));
        }
    }
    for (name, expected) in [
        ("TaskObserverId", "NonZeroU64"),
        ("HostRouteId", "NonZeroU32"),
        ("HostOperationId", "NonZeroU32"),
    ] {
        if tuple_field_type(file, name).as_deref() != Some(expected) {
            failures.push(Failure::new(
                "ZERO103",
                format!("{name} must retain {expected}"),
            ));
        }
    }
    for name in ["HostLaunchCapability", "HostCancellationCapability"] {
        let id = find_struct(file, name)
            .and_then(|item| named_field(item, "id"))
            .map(|field| normalized_tokens(&field.ty));
        if id.as_deref() != Some("NonZeroU64") {
            failures.push(Failure::new(
                "ZERO103",
                format!("{name}.id must retain NonZeroU64"),
            ));
        }
    }
    let counter = find_struct(file, "AlwaysStartOrdinalCounterSnapshotV1")
        .and_then(|item| named_field(item, "next"))
        .map(|field| normalized_tokens(&field.ty));
    if counter.as_deref() != Some("NonZeroU64") {
        failures.push(Failure::new(
            "ZERO103",
            "AlwaysStartOrdinalCounterSnapshotV1.next must retain NonZeroU64",
        ));
    }
    if tuple_field_type(file, "TaskPriority").as_deref() != Some("i32")
        || tuple_field_type(file, "HirNestedExpressionPath").as_deref()
            != Some("Box < [HirNestedExpressionPathSegment] >")
        || tuple_field_type(file, "CheckedNestedPathV1").as_deref()
            != Some("Box < [CheckedNestedPathSegmentV1] >")
        || !method_visibility(file, "HirNestedExpressionPath", "try_from_segments")
            .is_some_and(is_public)
        || !method_visibility(file, "CheckedNestedPathV1", "try_from_segments")
            .is_some_and(is_public)
    {
        failures.push(Failure::new(
            "ZERO104",
            "priority/path newtype zero and empty-domain owners changed",
        ));
    }

    let producer_new = impl_method(file, "NeedProducerSpec", "new");
    let producer_new_returns_self = producer_new.is_some_and(|method| {
        is_public(&method.vis)
            && matches!(
                &method.sig.output,
                syn::ReturnType::Type(_, ty) if normalized_tokens(ty.as_ref()) == "Self"
            )
    });
    let forbidden_zero_errors = ["ZeroContract", "ZeroPlan", "ZeroPayloadType"];
    let has_forbidden_zero_error = file.items.iter().any(|item| {
        let Item::Enum(item) = item else { return false };
        item.variants.iter().any(|variant| {
            forbidden_zero_errors
                .iter()
                .any(|name| variant.ident == *name)
        })
    });
    if !producer_new_returns_self
        || impl_method(file, "NeedProducerSpec", "try_new").is_some()
        || definitions.contains("NeedProducerSpecError")
        || has_forbidden_zero_error
    {
        failures.push(Failure::new(
            "ZERO105",
            "NeedProducerSpec must have one infallible typed constructor and no zero-digest errors",
        ));
    }
    let generation_errors = find_enum(file, "TaskValidationAuthorityError")
        .map(variant_names)
        .unwrap_or_default();
    if !generation_errors
        .iter()
        .any(|name| name == "GenerationMismatch")
        || generation_errors
            .iter()
            .any(|name| name == "InvalidGeneration")
    {
        failures.push(Failure::new(
            "ZERO106",
            "task validation must report owner generation mismatch, not invalid generation",
        ));
    }

    let join_is_public_zero = impl_const(file, "TaskLaunchOrdinal", "JOIN")
        .is_some_and(|item| is_public(&item.vis) && normalized_tokens(&item.expr) == "Self (0)");
    if !join_is_public_zero
        || !public_methods(file, "TaskLaunchOrdinal", &["get"])
        || !public_methods(file, "TaskPriority", &["new", "get"])
        || !public_methods(file, "HostRouteId", &["new", "get"])
        || !public_methods(file, "HostOperationId", &["new", "get"])
    {
        failures.push(Failure::new(
            "CTOR101",
            "scalar private-field newtype constructors/accessors are unreachable",
        ));
    }
    for name in [
        "NeedProducerContractDigest",
        "TaskPlanSemanticDigest",
        "RuntimeTypeSemanticDigest",
        "NeedTimeoutContractDigest",
        "HostOperationCatalogDigest",
    ] {
        if !public_methods(file, name, &["from_bytes", "as_bytes"]) {
            failures.push(Failure::new(
                "CTOR102",
                format!("{name} lacks its all-values semantic digest API"),
            ));
        }
    }
    for name in [
        "NeedProducerInstanceKey",
        "NeedId",
        "TaskKey",
        "TaskId",
        "HostCancelCommandId",
    ] {
        if ["new", "from_bytes", "try_from_bytes"]
            .iter()
            .any(|method| impl_method(file, name, method).is_some())
        {
            failures.push(Failure::new(
                "CTOR103",
                format!("{name} exposes a forbidden raw fixed-identity constructor"),
            ));
        }
    }

    let Some(receipt) = find_struct(file, "AcceptedTaskLaunchReceipt") else {
        failures.push(Failure::new("VIS101", "missing accepted receipt"));
        return failures;
    };
    if fields(receipt).iter().any(|field| is_public(&field.vis)) {
        failures.push(Failure::new(
            "VIS101",
            "accepted receipt fields must be private",
        ));
    }
    for (owner, method) in [
        ("RuntimeGenerationJournal", "accepted_launch_receipt"),
        ("RuntimeNeedHandle", "try_from_accepted_launch"),
    ] {
        if !method_visibility(file, owner, method).is_some_and(is_public) {
            failures.push(Failure::new(
                "VIS101",
                format!("{owner}::{method} is not public across crates"),
            ));
        }
    }

    let keyed = find_struct(file, "RuntimeGenerationJournal")
        .and_then(|item| named_field(item, "next_always_start_ordinals"))
        .map(|field| normalized_tokens(&field.ty));
    if keyed.as_deref() != Some("BTreeMap < NeedProducerInstanceKey , NonZeroU64 >") {
        failures.push(Failure::new(
            "ORD101",
            "journal field is not the exact keyed map",
        ));
    }
    let delta = find_struct(file, "RuntimeGenerationJournalAfterImage")
        .and_then(|item| named_field(item, "next_always_start_ordinals"))
        .map(|field| normalized_tokens(&field.ty));
    if delta.as_deref() != Some("BTreeMap < NeedProducerInstanceKey , NonZeroU64 >") {
        failures.push(Failure::new(
            "ORD101",
            "batch ordinal after-image is not keyed",
        ));
    }

    let prepare = trait_method(file, "TaskLaunchAdapter", "prepare_launch").map(normalized_tokens);
    if !prepare.as_deref().is_some_and(|tokens| {
        tokens.contains("PreparedLaunchBatch < Self :: PreparedLaunchToken >")
    }) {
        failures.push(Failure::new(
            "ADP101",
            "prepare_launch lacks inspectable wrapper",
        ));
    }

    if let Some(function) = find_struct(file, "AwbcRuntimeFunctionSnapshot") {
        let names = field_names(function);
        if names != ["function", "remaining_params", "captures"].map(str::to_owned) {
            failures.push(Failure::new(
                "SNP101",
                "function snapshot fields are not dormant-current",
            ));
        }
    }
    if definitions.contains("DenseSeq") {
        failures.push(Failure::new("SNP101", "design redefines current DenseSeq"));
    }
    let dense_ok = find_enum(file, "AwbcRuntimeSeqSnapshot")
        .and_then(|item| {
            item.variants
                .iter()
                .find(|variant| variant.ident == "Dense")
        })
        .is_some_and(|variant| match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                normalized_tokens(&fields.unnamed[0].ty) == "DenseSeq"
            }
            _ => false,
        });
    if !dense_ok {
        failures.push(Failure::new(
            "SNP101",
            "snapshot does not reuse DenseSeq directly",
        ));
    }

    if let Some(role) = find_enum(file, "HirExpressionChildRole") {
        let tokens = normalized_tokens(role);
        if tokens.contains("RuntimeRecordFieldId") || tokens.contains("CheckedNestedPathV1") {
            failures.push(Failure::new(
                "LAY101",
                "HIR role contains core/sema payload",
            ));
        }
    }
    failures
}

fn validate_repository(repository: &Path, contract: &Value) -> Result<Vec<Failure>> {
    let mut failures = Vec::new();
    let head = command_output(repository, "git", &["rev-parse", "HEAD"])?;
    if head.trim() != EXPECTED_HEAD {
        failures.push(Failure::new(
            "SRC001",
            format!("HEAD is {}, expected {EXPECTED_HEAD}", head.trim()),
        ));
        return Ok(failures);
    }
    if let Some(evidence) = contract
        .pointer("/source_evidence")
        .and_then(Value::as_object)
    {
        for (path, expected) in evidence {
            let actual =
                command_output(repository, "git", &["rev-parse", &format!("HEAD:{path}")])?;
            if Some(actual.trim()) != expected.as_str() {
                failures.push(Failure::new(
                    "SRC002",
                    format!("Git blob changed for {path}"),
                ));
            }
        }
    }

    let value = parse(repository, "crates/arcweft-core/src/value.rs")?;
    failures.extend(validate_dense_source(&value));
    let pattern = parse(repository, "crates/arcweft-core/src/pattern.rs")?;
    failures.extend(validate_option_source(&pattern));
    let types = parse(repository, "crates/arcweft-lang-sema/src/types.rs")?;
    let agent = parse(repository, "crates/arcweft-core/src/value/agent.rs")?;
    failures.extend(validate_agent_source(&types, &agent));
    let hir = parse(repository, "crates/arcweft-lang-hir/src/expr.rs")?;
    failures.extend(validate_hir_source(&hir));
    let save = parse(repository, "crates/arcweft-core/src/value/awbc_save.rs")?;
    failures.extend(validate_function_snapshot_source(&save));
    let callable = parse(
        repository,
        "crates/arcweft-lang-sema/src/callable/checked_catalog.rs",
    )?;
    if find_struct(&callable, "CheckedCallableCatalog").is_none()
        || definitions(&callable).contains("CheckedCallableCatalogV1")
    {
        failures.push(Failure::new(
            "SRC105",
            "current callable catalog owner mismatch",
        ));
    }

    let metadata: Value = serde_json::from_str(&command_output(
        repository,
        "cargo",
        &["metadata", "--no-deps", "--format-version", "1"],
    )?)?;
    let deps = metadata
        .pointer("/packages")
        .and_then(Value::as_array)
        .and_then(|packages| {
            packages.iter().find(|package| {
                package.get("name").and_then(Value::as_str) == Some("arcweft-lang-hir")
            })
        })
        .and_then(|package| package.get("dependencies"))
        .and_then(Value::as_array)
        .map(|deps| {
            deps.iter()
                .filter_map(|dep| dep.get("name").and_then(Value::as_str))
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    failures.extend(validate_hir_dependencies(&deps));
    Ok(failures)
}

fn validate_dense_source(file: &syn::File) -> Vec<Failure> {
    let expected = [
        ("Units", "usize"),
        ("I8", "DenseSeqStorage < i8 >"),
        ("I16", "DenseSeqStorage < i16 >"),
        ("I32", "DenseSeqStorage < i32 >"),
        ("I64", "DenseSeqStorage < i64 >"),
        ("I128", "DenseSeqStorage < i128 >"),
        ("ISize", "DenseSeqStorage < RuntimeISizeValue >"),
        ("U8", "DenseSeqStorage < u8 >"),
        ("U16", "DenseSeqStorage < u16 >"),
        ("U32", "DenseSeqStorage < u32 >"),
        ("U64", "DenseSeqStorage < u64 >"),
        ("U128", "DenseSeqStorage < u128 >"),
        ("USize", "DenseSeqStorage < RuntimeUSizeValue >"),
        ("F32", "DenseSeqStorage < f32 >"),
        ("F64", "DenseSeqStorage < f64 >"),
        ("Bool", "DenseSeqStorage < bool >"),
        ("Bytes", "DenseSeqStorage < u8 >"),
        ("Chars", "DenseSeqStorage < char >"),
        ("Durations", "DenseSeqStorage < LogicalDuration >"),
        ("Strings", "DenseSeqStorage < String >"),
        ("EntityRefs", "DenseSeqStorage < String >"),
    ];
    let Some(item) = find_enum(file, "DenseSeq") else {
        return vec![Failure::new("SRC101", "current DenseSeq is absent")];
    };
    let actual = item
        .variants
        .iter()
        .map(|variant| {
            let ty = match &variant.fields {
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    normalized_tokens(&fields.unnamed[0].ty)
                }
                _ => "<invalid>".to_owned(),
            };
            (variant.ident.to_string(), ty)
        })
        .collect::<Vec<_>>();
    let expected = expected
        .into_iter()
        .map(|(name, ty)| (name.to_owned(), ty.to_owned()))
        .collect::<Vec<_>>();
    (actual != expected)
        .then(|| Failure::new("SRC101", "current DenseSeq inventory/field shape changed"))
        .into_iter()
        .collect()
}

fn validate_option_source(file: &syn::File) -> Vec<Failure> {
    let Some(method) = impl_method(file, "RuntimeCheckedType", "variant_case") else {
        return vec![Failure::new(
            "SRC102",
            "RuntimeCheckedType::variant_case is absent",
        )];
    };
    let tokens = normalized_tokens(method);
    let some = tokens.contains(
        "0 => Some (RuntimeCheckedVariantCase { name : \"Some\" . to_owned () , payload : Some",
    );
    let none = tokens.contains(
        "1 => Some (RuntimeCheckedVariantCase { name : \"None\" . to_owned () , payload : None",
    );
    (!some || !none)
        .then(|| Failure::new("SRC102", "current Option ordinals/payload shapes changed"))
        .into_iter()
        .collect()
}

fn validate_agent_source(types: &syn::File, agent: &syn::File) -> Vec<Failure> {
    let expected = [
        "ObservedObjectId",
        "CaptureFormat",
        "CaptureKind",
        "Diagnostics",
        "WaitError",
        "ViewportPoint",
        "PointerButton",
        "RagError",
    ];
    let actual = find_enum(types, "AgentBuiltinType")
        .map(variant_names)
        .unwrap_or_default();
    let live = find_enum(agent, "RuntimeAgentValue")
        .map(variant_names)
        .unwrap_or_default();
    if actual != expected
        || !live.iter().any(|name| name == "Diagnostics")
        || !live.iter().any(|name| name == "ViewportPoint")
    {
        vec![Failure::new(
            "SRC103",
            "current AgentBuiltin/live carrier inventory changed",
        )]
    } else {
        Vec::new()
    }
}

fn validate_hir_source(file: &syn::File) -> Vec<Failure> {
    let expected = [
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
    let actual = find_enum(file, "HirExprKind")
        .map(variant_names)
        .unwrap_or_default();
    (actual != expected)
        .then(|| Failure::new("SRC104", "current HirExprKind inventory is not exact 38"))
        .into_iter()
        .collect()
}

fn validate_function_snapshot_source(file: &syn::File) -> Vec<Failure> {
    let names = find_struct(file, "AwbcRuntimeFunctionSnapshot")
        .map(field_names)
        .unwrap_or_default();
    (names != ["function", "remaining_params", "captures"].map(str::to_owned))
        .then(|| {
            Failure::new(
                "SRC106",
                "current function snapshot contains another authority",
            )
        })
        .into_iter()
        .collect()
}

fn validate_hir_dependencies(deps: &BTreeSet<String>) -> Vec<Failure> {
    let forbidden = ["arcweft-core", "arcweft-lang-sema"];
    let found = forbidden
        .into_iter()
        .filter(|name| deps.contains(*name))
        .collect::<Vec<_>>();
    (!found.is_empty())
        .then(|| Failure::new("SRC107", format!("HIR reverse dependencies: {found:?}")))
        .into_iter()
        .collect()
}

fn run_negative_self_tests(contract: &Value, schema_text: &str) -> (Vec<Failure>, usize) {
    let mut harness_failures = Vec::new();
    let mut case_count = 15;
    let model_cases: [(&str, &str, Box<dyn Fn(&mut Value)>); 9] = [
        (
            "make_policy",
            "MAKE001",
            Box::new(|v| {
                set(
                    v,
                    "/make_need_handle/policy",
                    Value::String("AlwaysStart".into()),
                )
            }),
        ),
        (
            "scalar_ordinal",
            "ORD001",
            Box::new(|v| {
                set(
                    v,
                    "/always_start_ordinals/live_type",
                    Value::String("NonZeroU64".into()),
                )
            }),
        ),
        (
            "private_receipt",
            "VIS001",
            Box::new(|v| {
                set(
                    v,
                    "/accepted_launch/constructor_visibility",
                    Value::String("pub(crate)".into()),
                )
            }),
        ),
        (
            "opaque_prepare",
            "ADP001",
            Box::new(|v| {
                set(
                    v,
                    "/adapter/prepare_result",
                    Value::String("Self::PreparedLaunchToken".into()),
                )
            }),
        ),
        (
            "function_authority",
            "SNP001",
            Box::new(|v| set(v, "/snapshot/per_function_authority", Value::Bool(true))),
        ),
        (
            "option_reverse",
            "VAR001",
            Box::new(|v| set(v, "/variants/option/Some", Value::from(1))),
        ),
        (
            "blanket_agent",
            "OWN001",
            Box::new(|v| set(v, "/variants/agent_builtin_blanket", Value::Bool(true))),
        ),
        (
            "parallel_catalog",
            "CAT001",
            Box::new(|v| {
                set(
                    v,
                    "/callable/catalog",
                    Value::String("CheckedCallableCatalogV1".into()),
                )
            }),
        ),
        (
            "edge_duplication",
            "LAY001",
            Box::new(|v| {
                set(
                    v,
                    "/match_edges/direct_children_is_projection",
                    Value::Bool(false),
                )
            }),
        ),
    ];
    for (name, expected, mutate) in model_cases {
        let mut candidate = contract.clone();
        mutate(&mut candidate);
        expect_failure(
            name,
            expected,
            validate_model(&candidate),
            &mut harness_failures,
        );
    }

    let private_schema = schema_text.replacen(
        "pub fn accepted_launch_receipt(",
        "pub(crate) fn accepted_launch_receipt(",
        1,
    );
    let private_file = syn::parse_file(&private_schema).expect("private schema fixture parses");
    expect_failure(
        "schema_visibility",
        "VIS101",
        validate_schema(contract, &private_file),
        &mut harness_failures,
    );

    let missing_schema = schema_text.replacen(
        "pub enum RuntimeSequenceProjectionV1",
        "pub enum RemovedRuntimeSequenceProjectionV1",
        1,
    );
    let missing_file = syn::parse_file(&missing_schema).expect("missing schema fixture parses");
    expect_failure(
        "undefined_projection",
        "SCH101",
        validate_schema(contract, &missing_file),
        &mut harness_failures,
    );

    let layered_schema = schema_text.replacen(
        "pub enum HirExpressionChildRole {",
        "pub enum HirExpressionChildRole { Bad(RuntimeRecordFieldId),",
        1,
    );
    let layered_file = syn::parse_file(&layered_schema).expect("layer fixture parses");
    expect_failure(
        "hir_reverse_payload",
        "LAY101",
        validate_schema(contract, &layered_file),
        &mut harness_failures,
    );

    let dense_fixture = syn::parse_file("enum DenseSeq { Units(Vec<()>), Bools(Vec<bool>) }")
        .expect("dense fixture parses");
    expect_failure(
        "dense_source",
        "SRC101",
        validate_dense_source(&dense_fixture),
        &mut harness_failures,
    );

    let generation_nonzero = schema_text.replacen(
        "pub struct GenerationId(u64);",
        "pub struct GenerationId(NonZeroU64);",
        1,
    );
    let generation_nonzero_file =
        syn::parse_file(&generation_nonzero).expect("GenerationId mutation parses");
    expect_failure(
        "generation_nonzero_regression",
        "ZERO101",
        validate_schema(contract, &generation_nonzero_file),
        &mut harness_failures,
    );

    let zero_digest_errors = format!(
        "{schema_text}\n\npub enum NeedProducerSpecError {{ ZeroContract, ZeroPlan, ZeroPayloadType }}\n"
    );
    let zero_digest_errors_file =
        syn::parse_file(&zero_digest_errors).expect("zero-digest error mutation parses");
    expect_failure(
        "producer_zero_digest_errors",
        "ZERO105",
        validate_schema(contract, &zero_digest_errors_file),
        &mut harness_failures,
    );

    for (owner, method) in CROSS_CRATE_METHODS {
        let mut candidate = syn::parse_file(schema_text).expect("API visibility fixture parses");
        assert!(
            set_impl_method_private(&mut candidate, owner, method),
            "missing self-test target {owner}::{method}"
        );
        case_count += 1;
        expect_failure(
            &format!("private_{owner}_{method}"),
            "API101",
            validate_schema(contract, &candidate),
            &mut harness_failures,
        );
    }

    for name in PRIVATE_PROTOCOL_TYPES {
        let mut candidate = syn::parse_file(schema_text).expect("field fixture parses");
        assert!(
            set_first_field_public(&mut candidate, name),
            "missing self-test field target {name}"
        );
        case_count += 1;
        expect_failure(
            &format!("public_field_{name}"),
            "API102",
            validate_schema(contract, &candidate),
            &mut harness_failures,
        );
    }

    let mut restricted_field = syn::parse_file(schema_text).expect("restricted field parses");
    assert!(set_first_field_restricted(
        &mut restricted_field,
        "TaskSpec"
    ));
    case_count += 1;
    expect_failure(
        "pub_crate_task_spec_field",
        "API102",
        validate_schema(contract, &restricted_field),
        &mut harness_failures,
    );

    let raw_row_constructor =
        format!("{schema_text}\n\nimpl TaskJournalRow {{ pub fn new() -> Self {{ todo!() }} }}\n");
    let raw_row_file =
        syn::parse_file(&raw_row_constructor).expect("raw row constructor mutation parses");
    case_count += 1;
    expect_failure(
        "raw_task_journal_row_constructor",
        "API103",
        validate_schema(contract, &raw_row_file),
        &mut harness_failures,
    );

    for method in [
        "apply_runtime_after_image",
        "apply_ensure_plan",
        "apply_restore_plan",
        "apply_rebind_plan",
        "apply_cancel_plan",
    ] {
        let mut candidate = syn::parse_file(schema_text).expect("coordinator fixture parses");
        assert!(
            rename_impl_method(&mut candidate, "RuntimeScheduler", method),
            "missing coordinator self-test target {method}"
        );
        case_count += 1;
        expect_failure(
            &format!("removed_{method}"),
            "API106",
            validate_schema(contract, &candidate),
            &mut harness_failures,
        );
    }

    for method in ADAPTER_METHODS {
        let mut candidate = syn::parse_file(schema_text).expect("adapter fixture parses");
        assert!(
            rename_trait_method(&mut candidate, "TaskLaunchAdapter", method),
            "missing adapter self-test target {method}"
        );
        case_count += 1;
        expect_failure(
            &format!("removed_adapter_{method}"),
            "API107",
            validate_schema(contract, &candidate),
            &mut harness_failures,
        );
    }

    (harness_failures, case_count)
}

fn expect_failure(
    name: &str,
    expected: &'static str,
    failures: Vec<Failure>,
    harness: &mut Vec<Failure>,
) {
    if !failures.iter().any(|failure| failure.code == expected) {
        harness.push(Failure::new(
            "SELF001",
            format!("negative {name} did not produce {expected}: {failures:?}"),
        ));
    }
}

fn parse(root: &Path, relative: &str) -> Result<syn::File> {
    let path = root.join(relative);
    syn::parse_file(&fs::read_to_string(&path).with_context(|| path.display().to_string())?)
        .with_context(|| format!("parse {relative}"))
}

fn command_output(root: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("run {program} {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "{} failed: {}",
            program,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).context("command output is UTF-8")
}

fn definitions(file: &syn::File) -> BTreeSet<String> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item) => Some(item.ident.to_string()),
            Item::Enum(item) => Some(item.ident.to_string()),
            Item::Trait(item) => Some(item.ident.to_string()),
            _ => None,
        })
        .collect()
}

fn find_struct<'a>(file: &'a syn::File, name: &str) -> Option<&'a ItemStruct> {
    file.items.iter().find_map(|item| match item {
        Item::Struct(item) if item.ident == name => Some(item),
        _ => None,
    })
}

fn tuple_field_type(file: &syn::File, name: &str) -> Option<String> {
    let item = find_struct(file, name)?;
    match &item.fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            Some(normalized_tokens(&fields.unnamed[0].ty))
        }
        _ => None,
    }
}

fn find_enum<'a>(file: &'a syn::File, name: &str) -> Option<&'a ItemEnum> {
    file.items.iter().find_map(|item| match item {
        Item::Enum(item) if item.ident == name => Some(item),
        _ => None,
    })
}

fn fields(item: &ItemStruct) -> Vec<&syn::Field> {
    match &item.fields {
        Fields::Named(fields) => fields.named.iter().collect(),
        Fields::Unnamed(fields) => fields.unnamed.iter().collect(),
        Fields::Unit => Vec::new(),
    }
}

fn field_names(item: &ItemStruct) -> Vec<String> {
    fields(item)
        .into_iter()
        .filter_map(|field| field.ident.as_ref().map(|ident| ident.to_string()))
        .collect()
}

fn named_field<'a>(item: &'a ItemStruct, name: &str) -> Option<&'a syn::Field> {
    fields(item)
        .into_iter()
        .find(|field| field.ident.as_ref().is_some_and(|ident| ident == name))
}

fn variant_names(item: &ItemEnum) -> Vec<String> {
    item.variants
        .iter()
        .map(|variant| variant.ident.to_string())
        .collect()
}

fn method_visibility<'a>(file: &'a syn::File, owner: &str, method: &str) -> Option<&'a Visibility> {
    impl_method(file, owner, method).map(|item| &item.vis)
}

fn impl_method<'a>(file: &'a syn::File, owner: &str, method: &str) -> Option<&'a syn::ImplItemFn> {
    file.items.iter().find_map(|item| {
        let Item::Impl(item) = item else { return None };
        if impl_owner(item).as_deref() != Some(owner) {
            return None;
        }
        item.items.iter().find_map(|item| match item {
            ImplItem::Fn(function) if function.sig.ident == method => Some(function),
            _ => None,
        })
    })
}

fn impl_const<'a>(file: &'a syn::File, owner: &str, name: &str) -> Option<&'a syn::ImplItemConst> {
    file.items.iter().find_map(|item| {
        let Item::Impl(item) = item else { return None };
        if impl_owner(item).as_deref() != Some(owner) {
            return None;
        }
        item.items.iter().find_map(|item| match item {
            ImplItem::Const(item) if item.ident == name => Some(item),
            _ => None,
        })
    })
}

fn public_methods(file: &syn::File, owner: &str, methods: &[&str]) -> bool {
    methods
        .iter()
        .all(|method| method_visibility(file, owner, method).is_some_and(is_public))
}

fn set_impl_method_private(file: &mut syn::File, owner: &str, method: &str) -> bool {
    for item in &mut file.items {
        let Item::Impl(item) = item else { continue };
        if impl_owner(item).as_deref() != Some(owner) {
            continue;
        }
        for item in &mut item.items {
            let ImplItem::Fn(function) = item else {
                continue;
            };
            if function.sig.ident == method {
                function.vis = Visibility::Inherited;
                return true;
            }
        }
    }
    false
}

fn set_first_field_public(file: &mut syn::File, name: &str) -> bool {
    for item in &mut file.items {
        let Item::Struct(item) = item else { continue };
        if item.ident != name {
            continue;
        }
        let Some(field) = item.fields.iter_mut().next() else {
            return false;
        };
        field.vis = syn::parse_quote!(pub);
        return true;
    }
    false
}

fn set_first_field_restricted(file: &mut syn::File, name: &str) -> bool {
    for item in &mut file.items {
        let Item::Struct(item) = item else { continue };
        if item.ident != name {
            continue;
        }
        let Some(field) = item.fields.iter_mut().next() else {
            return false;
        };
        field.vis = syn::parse_quote!(pub(crate));
        return true;
    }
    false
}

fn rename_impl_method(file: &mut syn::File, owner: &str, method: &str) -> bool {
    for item in &mut file.items {
        let Item::Impl(item) = item else { continue };
        if impl_owner(item).as_deref() != Some(owner) {
            continue;
        }
        for item in &mut item.items {
            let ImplItem::Fn(function) = item else {
                continue;
            };
            if function.sig.ident == method {
                function.sig.ident = syn::parse_quote!(removed_apply_seam);
                return true;
            }
        }
    }
    false
}

fn rename_trait_method(file: &mut syn::File, owner: &str, method: &str) -> bool {
    for item in &mut file.items {
        let Item::Trait(item) = item else { continue };
        if item.ident != owner {
            continue;
        }
        for item in &mut item.items {
            let syn::TraitItem::Fn(function) = item else {
                continue;
            };
            if function.sig.ident == method {
                function.sig.ident = syn::parse_quote!(removed_adapter_seam);
                return true;
            }
        }
    }
    false
}

fn impl_owner(item: &ItemImpl) -> Option<String> {
    let Type::Path(path) = item.self_ty.as_ref() else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn trait_method<'a>(
    file: &'a syn::File,
    owner: &str,
    method: &str,
) -> Option<&'a syn::TraitItemFn> {
    file.items.iter().find_map(|item| {
        let Item::Trait(ItemTrait { ident, items, .. }) = item else {
            return None;
        };
        if ident != owner {
            return None;
        }
        items.iter().find_map(|item| match item {
            syn::TraitItem::Fn(function) if function.sig.ident == method => Some(function),
            _ => None,
        })
    })
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn normalized_tokens(value: &impl ToTokens) -> String {
    value.to_token_stream().to_string()
}

fn text<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer).and_then(Value::as_str)
}

fn number(value: &Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer).and_then(Value::as_u64)
}

fn bool_at(value: &Value, pointer: &str) -> Option<bool> {
    value.pointer(pointer).and_then(Value::as_bool)
}

fn strings(value: &Value, pointer: &str) -> Vec<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn set(value: &mut Value, pointer: &str, replacement: Value) {
    *value
        .pointer_mut(pointer)
        .expect("self-test pointer exists") = replacement;
}
