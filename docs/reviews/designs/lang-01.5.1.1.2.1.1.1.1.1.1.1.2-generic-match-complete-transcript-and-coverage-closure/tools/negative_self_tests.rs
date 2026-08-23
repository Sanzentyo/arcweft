#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
sha2 = "0.10"
syn = { version = "2", features = ["full"] }
walkdir = "2"
---

mod validation_support;

use serde_json::{json, Value};
use std::path::PathBuf;
use validation_support::{
    load_bundle, validate_manifest, validate_repository, validate_semantic, Bundle, DESIGN_REL,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("FAIL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DESIGN_REL));
    let baseline = load_bundle(&root)?;
    validate_semantic(&baseline)?;
    validate_manifest(&baseline)?;
    validate_repository(&baseline)?;
    let corpus: Value = serde_json::from_slice(
        baseline
            .files
            .get("machine/negative_corpus.json")
            .ok_or("negative corpus missing")?,
    )?;
    let cases = corpus["cases"].as_array().ok_or("cases missing")?;
    for case in cases {
        let id = case["id"].as_str().ok_or("case id missing")?;
        let mutation = case["mutation"].as_str().ok_or("mutation missing")?;
        let expected_gate = case["expected_gate"]
            .as_str()
            .ok_or("expected gate missing")?;
        let mut candidate = baseline.clone();
        apply_mutation(&mut candidate, mutation)?;
        let error = if mutation == "manifest_hash" {
            validate_semantic(&candidate)?;
            validate_manifest(&candidate).expect_err("manifest mutation unexpectedly passed")
        } else {
            validate_semantic(&candidate).expect_err("semantic mutation unexpectedly passed")
        };
        if error.code != expected_gate {
            return Err(format!(
                "{id}: expected gate {expected_gate}, got {} ({})",
                error.code, error.detail
            )
            .into());
        }
    }
    println!(
        "PASS design={} negative_cases={}",
        baseline.root.display(),
        cases.len()
    );
    Ok(())
}

fn apply_mutation(bundle: &mut Bundle, mutation: &str) -> Result<(), Box<dyn std::error::Error>> {
    match mutation {
        "status_file" => replace_file(bundle, "FINAL_STATUS.md", b"DRAFT\n".to_vec()),
        "open_questions_file" => {
            replace_file(bundle, "OPEN_QUESTIONS.md", b"result-changing\n".to_vec())
        }
        "request_byte" => bundle
            .files
            .get_mut("REQUEST.md")
            .ok_or("REQUEST.md missing")?
            .push(b'x'),
        "contract_version" => bundle.contract["contract_version"] = json!(2),
        "drop_expression_resolution" => pop_inventory(bundle, "checked_expression_resolution")?,
        "drop_value_resolution" => pop_inventory(bundle, "checked_value_resolution")?,
        "drop_select_resolution" => pop_inventory(bundle, "checked_select_resolution")?,
        "drop_pattern_resolution" => pop_inventory(bundle, "checked_pattern_resolution")?,
        "drop_expression_family" => pop_inventory(bundle, "hir_expression_families")?,
        "drop_pattern_family" => pop_inventory(bundle, "hir_pattern_families")?,
        "drop_statement_family" => pop_inventory(bundle, "hir_statement_families")?,
        "drop_body_child_role" => pop_inventory(bundle, "hir_body_child_roles")?,
        "drop_statement_body_role" => pop_inventory(bundle, "hir_statement_body_roles")?,
        "drop_declaration_owner" => pop_inventory(bundle, "match_bearing_declaration_owners")?,
        "drop_declaration_root" => pop_inventory(bundle, "declaration_roots")?,
        "drop_body_root" => pop_inventory(bundle, "expression_owned_non_expression_roots")?,
        mutation if mutation.starts_with("drop_decision_") => {
            let id: u64 = mutation.trim_start_matches("drop_decision_").parse()?;
            bundle.contract["decisions"]
                .as_array_mut()
                .ok_or("decisions missing")?
                .retain(|decision| decision["id"].as_u64() != Some(id));
        }
        "enable_source_spelling" => bundle.contract["transcript"]["source_spelling"] = json!(true),
        "remove_schema_anchor" => {
            let text = String::from_utf8(
                bundle
                    .files
                    .get("SCHEMAS.md")
                    .ok_or("SCHEMAS.md missing")?
                    .clone(),
            )?;
            replace_file(
                bundle,
                "SCHEMAS.md",
                text.replace("CheckedRecordPatternField", "RemovedRecordPatternField")
                    .into_bytes(),
            );
        }
        "coverage_algorithm" => bundle.contract["coverage"]["algorithm"] = json!("basic atoms"),
        "checked_u64_false" => bundle.contract["coverage"]["checked_u64"] = json!(false),
        "view_missing_body_false" => {
            bundle.contract["declaration_bridge"]["view_missing_body_deleted"] = json!(false)
        }
        "version_other_than_one" => {
            bundle.contract["non_goals"]["version_other_than_one"] = json!(true)
        }
        "enable_raw_ids" => bundle.contract["transcript"]["raw_ids"] = json!(true),
        "enable_persistence" => bundle.contract["non_goals"]["persistence"] = json!(true),
        "enable_task_plan" => bundle.contract["non_goals"]["task_plan_seal"] = json!(true),
        "enable_whole_catalog" => {
            bundle.contract["non_goals"]["whole_catalog_digest"] = json!(true)
        }
        "enable_legacy" => bundle.contract["non_goals"]["legacy_reader"] = json!(true),
        "source_blob" => {
            bundle.contract["source_blobs"]["crates/arcweft-lang-hir/src/expr.rs"] =
                json!("0000000000000000000000000000000000000000")
        }
        "remove_required_file" => {
            bundle.files.remove("README.md");
        }
        "manifest_hash" => {
            let manifest = bundle
                .files
                .get_mut("MANIFEST.sha256")
                .ok_or("manifest missing")?;
            let byte = manifest.first_mut().ok_or("manifest empty")?;
            *byte = if *byte == b'0' { b'1' } else { b'0' };
        }
        other => return Err(format!("unknown mutation {other}").into()),
    }
    Ok(())
}

fn pop_inventory(bundle: &mut Bundle, key: &str) -> Result<(), Box<dyn std::error::Error>> {
    bundle.inventory[key]
        .as_array_mut()
        .ok_or_else(|| format!("inventory {key} missing"))?
        .pop()
        .ok_or_else(|| format!("inventory {key} empty"))?;
    Ok(())
}

fn replace_file(bundle: &mut Bundle, path: &str, bytes: Vec<u8>) {
    bundle.files.insert(path.to_owned(), bytes);
}
