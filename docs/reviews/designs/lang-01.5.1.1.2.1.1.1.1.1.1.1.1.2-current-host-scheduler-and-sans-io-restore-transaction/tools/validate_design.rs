#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "validate-host-scheduler-restore-design"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
serde_json = "1.0.150"
sha2 = "0.10.9"
---

//! Read-only repository-aware validator for the accepted host scheduler and
//! Sans-I/O restore-transaction design. It validates the frozen evidence and
//! design mirrors; it never treats production source spelling as proof that
//! the future implementation already exists.

#[allow(dead_code)]
mod support {
    include!("validation_support.rs");
}

use serde_json::Value;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use support::{validate_artifacts, Candidate, Finding, EXPECTED_HEAD, SEQUENCE};

struct Args {
    repository_root: PathBuf,
    design_root: PathBuf,
}

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
    let args = parse_args()?;
    let candidate = Candidate::load(&args.design_root)?;
    let mut findings = validate_artifacts(&candidate);
    findings.extend(validate_repository(&candidate, &args.repository_root));
    if !findings.is_empty() {
        findings.sort_by(|left, right| {
            (left.code, left.detail.as_str()).cmp(&(right.code, right.detail.as_str()))
        });
        let rendered = findings
            .into_iter()
            .map(|finding| format!("{} {}", finding.code, finding.detail))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(rendered);
    }
    println!("PASS sequence={SEQUENCE}");
    println!("head={EXPECTED_HEAD}");
    println!(
        "artifacts=PASS request_mirror=PASS manifest=PASS source_blobs=PASS cargo_metadata=PASS"
    );
    Ok(())
}

fn parse_args() -> Result<Args, String> {
    let script = absolute_script_path()?;
    let current = env::current_dir().map_err(|error| format!("current_dir: {error}"))?;
    let default_design = if current.join("machine/final_contract.json").is_file() {
        current
    } else {
        script
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| format!("cannot derive design root from {}", script.display()))?
            .to_path_buf()
    };
    let mut repository_root = None;
    let mut design_root = None;
    let mut arguments = env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_string_lossy().as_ref() {
            "--repository-root" => {
                repository_root =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        "--repository-root requires a path".to_owned()
                    })?));
            }
            "--design-root" => {
                design_root = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--design-root requires a path".to_owned())?,
                ));
            }
            "--help" | "-h" => {
                return Err(
                    "usage: validate_design.rs --repository-root PATH [--design-root PATH]"
                        .to_owned(),
                );
            }
            unknown => return Err(format!("unknown argument {unknown}")),
        }
    }
    let repository_root = repository_root
        .ok_or_else(|| "--repository-root is required".to_owned())?
        .canonicalize()
        .map_err(|error| format!("canonicalize repository root: {error}"))?;
    let design_root = design_root.unwrap_or(default_design);
    Ok(Args {
        repository_root,
        design_root,
    })
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

fn validate_repository(candidate: &Candidate, repository_root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    let head = git_output(repository_root, &["rev-parse", "HEAD"]);
    match head {
        Ok(head) if head == EXPECTED_HEAD => {}
        Ok(head) => findings.push(external_finding(
            "GIT001",
            format!("HEAD is {head}, expected {EXPECTED_HEAD}"),
        )),
        Err(error) => findings.push(external_finding("GIT001", error)),
    }

    if let Some(request_path) = candidate
        .contract
        .pointer("/request/repository_path")
        .and_then(Value::as_str)
    {
        match std::fs::read(repository_root.join(request_path)) {
            Ok(bytes) if candidate.files.get("REQUEST.md") == Some(&bytes) => {}
            Ok(_) => findings.push(external_finding(
                "REQ003",
                "REQUEST.md is not byte-identical to the maintained request",
            )),
            Err(error) => findings.push(external_finding(
                "REQ003",
                format!("read maintained request: {error}"),
            )),
        }
    } else {
        findings.push(external_finding(
            "REQ003",
            "missing request repository path",
        ));
    }

    if let Some(blobs) = candidate
        .contract
        .pointer("/source_blobs")
        .and_then(Value::as_object)
    {
        for (path, expected) in blobs {
            let Some(expected) = expected.as_str() else {
                findings.push(external_finding(
                    "GIT002",
                    format!("non-string expected blob for {path}"),
                ));
                continue;
            };
            let revision_path = format!("HEAD:{path}");
            match git_output(repository_root, &["rev-parse", &revision_path]) {
                Ok(actual) if actual == expected => {}
                Ok(actual) => findings.push(external_finding(
                    "GIT002",
                    format!("blob {path} is {actual}, expected {expected}"),
                )),
                Err(error) => findings.push(external_finding("GIT002", error)),
            }
        }
    }

    match Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(repository_root)
        .output()
    {
        Ok(output) if output.status.success() => {
            if serde_json::from_slice::<Value>(&output.stdout).is_err() {
                findings.push(external_finding(
                    "CARGO001",
                    "cargo metadata returned invalid JSON",
                ));
            }
        }
        Ok(output) => findings.push(external_finding(
            "CARGO001",
            format!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        )),
        Err(error) => findings.push(external_finding(
            "CARGO001",
            format!("launch cargo metadata: {error}"),
        )),
    }
    findings
}

fn external_finding(code: &'static str, detail: impl Into<String>) -> Finding {
    Finding {
        code,
        detail: detail.into(),
    }
}

fn git_output(repository_root: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository_root)
        .output()
        .map_err(|error| format!("launch git {}: {error}", arguments.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
