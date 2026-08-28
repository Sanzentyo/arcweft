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

use serde_json::json;
use std::collections::BTreeSet;
use std::path::PathBuf;
use validation_support::{
    Bundle, DESIGN_REL, load_bundle, repository_dependency_map, validate_dependency_map,
    validate_manifest, validate_repository, validate_semantic,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("FAIL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut design_only = false;
    let mut root = None;
    for argument in std::env::args_os().skip(1) {
        if argument == "--design-only" {
            design_only = true;
        } else if root.replace(PathBuf::from(argument)).is_some() {
            return Err("more than one design path supplied".into());
        }
    }
    let root = root.unwrap_or_else(|| PathBuf::from(DESIGN_REL));
    let baseline = load_bundle(&root)?;
    validate_semantic(&baseline)?;
    validate_manifest(&baseline)?;
    if !design_only {
        validate_repository(&baseline)?;
    }

    let mandatory = baseline.corpus["mandatory_gates"]
        .as_array()
        .ok_or("mandatory gates missing")?
        .iter()
        .map(|value| value.as_str().map(str::to_owned).ok_or("non-string gate"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut exercised = BTreeSet::new();
    let mut cases = 0_u64;

    let mut candidate = baseline.clone();
    replace_file(&mut candidate, "FINAL_STATUS.md", b"DRAFT\n".to_vec());
    expect_semantic(&candidate, "terminal_status", &mut exercised, &mut cases)?;

    let mut candidate = baseline.clone();
    replace_file(
        &mut candidate,
        "OPEN_QUESTIONS.md",
        b"result-changing\n".to_vec(),
    );
    expect_semantic(&candidate, "open_questions", &mut exercised, &mut cases)?;

    let mut candidate = baseline.clone();
    candidate
        .files
        .get_mut("REQUEST.md")
        .ok_or("REQUEST.md missing")?
        .push(b'x');
    expect_semantic(&candidate, "request_mirror", &mut exercised, &mut cases)?;

    let mut candidate = baseline.clone();
    let first = candidate
        .files
        .get_mut("MANIFEST.txt")
        .and_then(|value| value.first_mut())
        .ok_or("manifest empty")?;
    *first = if *first == b'0' { b'1' } else { b'0' };
    expect_manifest(&candidate, "manifest", &mut exercised, &mut cases)?;

    let mut candidate = baseline.clone();
    candidate.contract["contract_version"] = json!(2);
    expect_semantic(&candidate, "version_one", &mut exercised, &mut cases)?;

    let mut candidate = baseline.clone();
    candidate.files.remove("README.md");
    expect_semantic(&candidate, "required_files", &mut exercised, &mut cases)?;

    for pointer in [
        "/precedence/checked_select_resolution_current_count",
        "/precedence/view_specified_value_current_count",
    ] {
        let mut candidate = baseline.clone();
        *candidate
            .contract
            .pointer_mut(pointer)
            .ok_or("precedence pointer missing")? = json!(999);
        expect_semantic(
            &candidate,
            "precedence_inventory",
            &mut exercised,
            &mut cases,
        )?;
    }

    let mut candidate = baseline.clone();
    candidate.contract["syntax"]["select_suffix_question_accepted"] = json!(true);
    expect_semantic(&candidate, "syntax_contract", &mut exercised, &mut cases)?;

    let mut candidate = baseline.clone();
    candidate.contract["hir"]["mark_has_pattern_child"] = json!(true);
    expect_semantic(&candidate, "hir_contract", &mut exercised, &mut cases)?;

    let ingress_len = baseline.contract["standard_ingress"]["publications"]
        .as_array()
        .ok_or("ingress publications missing")?
        .len();
    for index in 0..ingress_len {
        let mut candidate = baseline.clone();
        candidate.contract["standard_ingress"]["publications"]
            .as_array_mut()
            .ok_or("ingress publications missing")?
            .remove(index);
        expect_semantic(&candidate, "ingress_contract", &mut exercised, &mut cases)?;

        let mut candidate = baseline.clone();
        candidate.contract["standard_ingress"]["publications"][index]["id"] = json!("Mismapped");
        expect_semantic(&candidate, "ingress_contract", &mut exercised, &mut cases)?;
    }

    let scrutinee_len = baseline.contract["scrutinee"]["roles"]
        .as_array()
        .ok_or("scrutinee roles missing")?
        .len();
    for index in 0..scrutinee_len {
        let mut candidate = baseline.clone();
        candidate.contract["scrutinee"]["roles"]
            .as_array_mut()
            .ok_or("scrutinee roles missing")?
            .remove(index);
        expect_semantic(&candidate, "scrutinee_contract", &mut exercised, &mut cases)?;
    }

    let coordinate_len = baseline.contract["mark_coordinate"]["bytes"]
        .as_array()
        .ok_or("coordinate bytes missing")?
        .len();
    for index in 0..coordinate_len {
        let mut candidate = baseline.clone();
        candidate.contract["mark_coordinate"]["bytes"][index] = json!("mutated");
        expect_semantic(&candidate, "mark_coordinate", &mut exercised, &mut cases)?;
    }

    for key in [
        "checked_trigger",
        "checked_select_statement",
        "checked_select_head",
        "checked_statement_payload",
    ] {
        let len = baseline.contract[key]
            .as_array()
            .ok_or("tag inventory missing")?
            .len();
        for index in 0..len {
            let mut candidate = baseline.clone();
            candidate.contract[key][index]["tag"] = json!(999);
            expect_semantic(&candidate, "checked_tags", &mut exercised, &mut cases)?;
        }
    }

    let matrix_len = baseline.contract["statement_matrix"]
        .as_array()
        .ok_or("statement matrix missing")?
        .len();
    for index in 0..matrix_len {
        let mut candidate = baseline.clone();
        candidate.contract["statement_matrix"][index]["payload"] = json!("mutated");
        expect_semantic(&candidate, "statement_matrix", &mut exercised, &mut cases)?;
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
        let mut candidate = baseline.clone();
        candidate.contract["transcript"][key] = json!(true);
        expect_semantic(
            &candidate,
            "transcript_contract",
            &mut exercised,
            &mut cases,
        )?;
    }

    let mut candidate = baseline.clone();
    candidate.contract["wait_mark"]["legacy_string_fallback"] = json!(true);
    expect_semantic(&candidate, "wait_mark_policy", &mut exercised, &mut cases)?;

    let deletion_len = baseline.contract["deletion_order"]
        .as_array()
        .ok_or("deletion order missing")?
        .len();
    for index in 0..deletion_len {
        let mut candidate = baseline.clone();
        candidate.contract["deletion_order"]
            .as_array_mut()
            .ok_or("deletion order missing")?
            .remove(index);
        expect_semantic(&candidate, "deletion_order", &mut exercised, &mut cases)?;
    }

    let prohibition_keys = baseline.contract["prohibitions"]
        .as_object()
        .ok_or("prohibitions missing")?
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for key in prohibition_keys {
        let mut candidate = baseline.clone();
        candidate.contract["prohibitions"][&key] = json!(true);
        expect_semantic(
            &candidate,
            "forbidden_authority",
            &mut exercised,
            &mut cases,
        )?;
    }

    if !design_only {
        let mut candidate = baseline.clone();
        candidate.inventory["files"][0]["blob"] = json!("0000000000000000000000000000000000000000");
        expect_repository(&candidate, "source_inventory", &mut exercised, &mut cases)?;

        let mut dependencies = repository_dependency_map(&baseline.repo)?;
        dependencies
            .entry("arcweft-lang-hir".to_owned())
            .or_default()
            .insert("arcweft-lang-sema".to_owned());
        let error = validate_dependency_map(&dependencies)
            .expect_err("reverse dependency mutation unexpectedly passed");
        record_error(
            error.code,
            "dependency_direction",
            &mut exercised,
            &mut cases,
        )?;
    } else {
        exercised.insert("source_inventory".to_owned());
        exercised.insert("dependency_direction".to_owned());
    }

    if exercised != mandatory {
        return Err(format!(
            "mandatory gate coverage differs: expected {mandatory:?}, got {exercised:?}"
        )
        .into());
    }
    println!(
        "PASS design={} negative_cases={} mandatory_gates={} repository={}",
        baseline.root.display(),
        cases,
        exercised.len(),
        if design_only { "NOT_RUN" } else { "PASS" }
    );
    Ok(())
}

fn expect_semantic(
    candidate: &Bundle,
    expected: &'static str,
    exercised: &mut BTreeSet<String>,
    cases: &mut u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let error = validate_semantic(candidate).expect_err("semantic mutation unexpectedly passed");
    record_error(error.code, expected, exercised, cases)
}

fn expect_manifest(
    candidate: &Bundle,
    expected: &'static str,
    exercised: &mut BTreeSet<String>,
    cases: &mut u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let error = validate_manifest(candidate).expect_err("manifest mutation unexpectedly passed");
    record_error(error.code, expected, exercised, cases)
}

fn expect_repository(
    candidate: &Bundle,
    expected: &'static str,
    exercised: &mut BTreeSet<String>,
    cases: &mut u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let error =
        validate_repository(candidate).expect_err("repository mutation unexpectedly passed");
    record_error(error.code, expected, exercised, cases)
}

fn record_error(
    actual: &'static str,
    expected: &'static str,
    exercised: &mut BTreeSet<String>,
    cases: &mut u64,
) -> Result<(), Box<dyn std::error::Error>> {
    if actual != expected {
        return Err(format!("expected gate {expected}, got {actual}").into());
    }
    exercised.insert(expected.to_owned());
    *cases = cases.checked_add(1).ok_or("case count overflow")?;
    Ok(())
}

fn replace_file(bundle: &mut Bundle, path: &str, bytes: Vec<u8>) {
    bundle.files.insert(path.to_owned(), bytes);
}
