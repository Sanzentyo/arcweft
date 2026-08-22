#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "negative-self-test-host-scheduler-restore-design"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
serde_json = "1.0.150"
sha2 = "0.10.9"
---

//! In-memory mutation corpus for the design validator. No repository or design
//! file is written. Every mutation must be rejected by its dedicated invariant
//! code; incidental manifest findings do not substitute for that code.

#[allow(dead_code)]
mod support {
    include!("validation_support.rs");
}

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use support::{validate_artifacts, Candidate, Mutation};

fn main() -> ExitCode {
    match run() {
        Ok(count) => {
            println!("PASS negative_self_tests={count}/{count}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<usize, String> {
    let root = parse_design_root()?;
    let baseline = Candidate::load(&root)?;
    let baseline_findings = validate_artifacts(&baseline);
    if !baseline_findings.is_empty() {
        return Err(format!(
            "baseline is invalid: {}",
            baseline_findings
                .iter()
                .map(|finding| format!("{} {}", finding.code, finding.detail))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }

    for (name, mutation, expected_code) in Mutation::cases() {
        let mut candidate = baseline.clone();
        mutation.apply(&mut candidate);
        let findings = validate_artifacts(&candidate);
        if !findings
            .iter()
            .any(|finding| finding.code == *expected_code)
        {
            return Err(format!(
                "mutation {name} did not produce {expected_code}; got {}",
                findings
                    .iter()
                    .map(|finding| finding.code)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
    }
    Ok(Mutation::cases().len())
}

fn parse_design_root() -> Result<PathBuf, String> {
    let mut arguments = env::args_os().skip(1);
    let mut explicit = None;
    while let Some(argument) = arguments.next() {
        match argument.to_string_lossy().as_ref() {
            "--design-root" => {
                explicit = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--design-root requires a path".to_owned())?,
                ));
            }
            unknown => return Err(format!("unknown argument {unknown}")),
        }
    }
    if let Some(root) = explicit {
        return Ok(root);
    }
    let current = env::current_dir().map_err(|error| format!("current_dir: {error}"))?;
    if current.join("machine/final_contract.json").is_file() {
        return Ok(current);
    }
    let script = absolute_script_path()?;
    script
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("cannot derive design root from {}", script.display()))
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
