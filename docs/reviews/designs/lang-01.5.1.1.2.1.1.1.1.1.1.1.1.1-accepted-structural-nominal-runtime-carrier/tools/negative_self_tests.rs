#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "negative-structural-nominal-design"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
serde_json = "1.0.150"
sha2 = "0.10.9"
---

//! Mutation corpus proving each material design gate fails closed.

#[allow(dead_code)]
mod support {
    include!("validation_support.rs");
}

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use support::{validate_artifacts, Candidate};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let root = parse_design_root()?;
    let baseline = Candidate::load(&root)?;
    let baseline_findings = validate_artifacts(&baseline);
    if !baseline_findings.is_empty() {
        return Err(format!("baseline invalid: {baseline_findings:?}"));
    }

    let mut count = 0_usize;
    mutate("required-member", &baseline, "ART001", |candidate| {
        candidate.files.remove("README.md");
    })?;
    count += 1;
    mutate("status", &baseline, "STATUS001", |candidate| {
        candidate.contract["status"] = serde_json::json!("BLOCKED");
    })?;
    count += 1;
    mutate("open-questions", &baseline, "OPEN001", |candidate| {
        candidate.contract["open_questions"] = serde_json::json!(1);
    })?;
    count += 1;
    mutate("request", &baseline, "REQ002", |candidate| {
        candidate.files.get_mut("REQUEST.md").unwrap().push(b'!');
    })?;
    count += 1;
    mutate("schema", &baseline, "SCH002", |candidate| {
        remove_token(
            candidate,
            "SCHEMAS.md",
            "pub struct RuntimeNominalSchemaGraph",
        );
    })?;
    count += 1;
    mutate("decision", &baseline, "DEC002", |candidate| {
        remove_token(candidate, "DECISION_REGISTER.md", "| D8 ");
    })?;
    count += 1;
    mutate("cuts", &baseline, "CUT001", |candidate| {
        candidate.contract["cuts"].as_array_mut().unwrap().pop();
    })?;
    count += 1;
    mutate("wire-golden", &baseline, "WIRE001", |candidate| {
        remove_token(candidate, "WIRE_AND_RESTORE.md", "18 00 S L 00 00");
    })?;
    count += 1;
    mutate("tag", &baseline, "TAG001", |candidate| {
        candidate.contract["wire"]["new_runtime_type_tags"] = serde_json::json!(1);
    })?;
    count += 1;
    mutate("version", &baseline, "VER002", |candidate| {
        candidate.contract["wire"]["version"] = serde_json::json!(2);
    })?;
    count += 1;
    mutate("parallel-carrier", &baseline, "MODEL001", |candidate| {
        candidate.contract["selected_model"]["accepted_runtime_carrier"] = serde_json::json!(true);
    })?;
    count += 1;
    mutate("persisted-side-table", &baseline, "MODEL002", |candidate| {
        candidate.contract["selected_model"]["schema_graph_persisted_in_runtime_plan"] =
            serde_json::json!(true);
    })?;
    count += 1;
    mutate("source-inventory", &baseline, "SRC001", |candidate| {
        let blobs = candidate.contract["source_blobs"].as_object_mut().unwrap();
        while blobs.len() >= 35 {
            let key = blobs.keys().next().unwrap().clone();
            blobs.remove(&key);
        }
    })?;
    count += 1;
    mutate("manifest", &baseline, "MAN005", |candidate| {
        let bytes = candidate.files.get_mut("README.md").unwrap();
        bytes.push(b'!');
    })?;
    count += 1;
    mutate("report", &baseline, "VAL001", |candidate| {
        remove_token(candidate, "VALIDATION_REPORT.md", "Overall: PASS");
    })?;
    count += 1;

    println!("PASS negative_mutations={count}");
    Ok(())
}

fn mutate(
    name: &str,
    baseline: &Candidate,
    expected_code: &str,
    change: impl FnOnce(&mut Candidate),
) -> Result<(), String> {
    let mut candidate = baseline.clone();
    change(&mut candidate);
    let findings = validate_artifacts(&candidate);
    if findings.iter().any(|finding| finding.code == expected_code) {
        Ok(())
    } else {
        Err(format!(
            "mutation {name} did not produce {expected_code}: {findings:?}"
        ))
    }
}

fn remove_token(candidate: &mut Candidate, path: &str, token: &str) {
    let bytes = candidate.files.get_mut(path).expect("mutation file exists");
    let text = String::from_utf8(bytes.clone()).expect("design text is UTF-8");
    *bytes = text.replacen(token, "", 1).into_bytes();
}

fn parse_design_root() -> Result<PathBuf, String> {
    let script = absolute_script_path()?;
    let current = env::current_dir().map_err(|error| format!("current_dir: {error}"))?;
    let default = if current.join("machine/final_contract.json").is_file() {
        current
    } else {
        script
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| "cannot derive design root".to_owned())?
            .to_path_buf()
    };
    let mut root = None;
    let mut arguments = env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_string_lossy().as_ref() {
            "--design-root" => {
                root = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--design-root requires a path".to_owned())?,
                ))
            }
            unknown => return Err(format!("unknown argument {unknown}")),
        }
    }
    Ok(root.unwrap_or(default))
}

fn absolute_script_path() -> Result<PathBuf, String> {
    let path = PathBuf::from(file!());
    if path.is_absolute() {
        Ok(path)
    } else {
        env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|error| format!("current_dir: {error}"))
    }
}
