#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "validate-structural-nominal-design"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
serde_json = "1.0.150"
sha2 = "0.10.9"
---

//! Read-only repository-aware validator for the accepted structural nominal design.

#[allow(dead_code)]
mod support {
    include!("validation_support.rs");
}

use serde_json::Value;
use std::collections::BTreeSet;
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
        return Err(findings
            .into_iter()
            .map(|finding| format!("{} {}", finding.code, finding.detail))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    println!("PASS sequence={SEQUENCE}");
    println!("head={EXPECTED_HEAD}");
    println!("artifacts=PASS request=PASS manifest=PASS blobs=PASS cargo=PASS direction=PASS");
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
            unknown => return Err(format!("unknown argument {unknown}")),
        }
    }
    Ok(Args {
        repository_root: repository_root
            .ok_or_else(|| "--repository-root is required".to_owned())?
            .canonicalize()
            .map_err(|error| format!("canonicalize repository root: {error}"))?,
        design_root: design_root.unwrap_or(default_design),
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

fn validate_repository(candidate: &Candidate, root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    check_git(root, &mut findings);
    check_request_mirror(candidate, root, &mut findings);
    check_source_blobs(candidate, root, &mut findings);
    check_current_owner_tokens(root, &mut findings);
    check_cargo_direction(root, &mut findings);
    findings
}

fn check_git(root: &Path, findings: &mut Vec<Finding>) {
    for (label, args) in [
        ("HEAD", ["rev-parse", "HEAD"]),
        ("origin/main", ["rev-parse", "origin/main"]),
    ] {
        match git_output(root, &args) {
            Ok(actual) if actual == EXPECTED_HEAD => {}
            Ok(actual) => findings.push(external("GIT002", format!("{label}={actual}"))),
            Err(error) => findings.push(external("GIT002", error)),
        }
    }
    match git_output(
        root,
        &[
            "status",
            "--porcelain",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "crates",
        ],
    ) {
        Ok(output) if output.is_empty() => {}
        Ok(output) => findings.push(external(
            "GIT003",
            format!("production tree dirty:\n{output}"),
        )),
        Err(error) => findings.push(external("GIT003", error)),
    }
}

fn check_request_mirror(candidate: &Candidate, root: &Path, findings: &mut Vec<Finding>) {
    let Some(path) = candidate
        .contract
        .pointer("/request/repository_path")
        .and_then(Value::as_str)
    else {
        findings.push(external("REQ003", "request path missing"));
        return;
    };
    match std::fs::read(root.join(path)) {
        Ok(bytes) if candidate.files.get("REQUEST.md") == Some(&bytes) => {}
        Ok(_) => findings.push(external("REQ003", "REQUEST.md is not byte-identical")),
        Err(error) => findings.push(external("REQ003", format!("read request: {error}"))),
    }
}

fn check_source_blobs(candidate: &Candidate, root: &Path, findings: &mut Vec<Finding>) {
    let Some(blobs) = candidate
        .contract
        .pointer("/source_blobs")
        .and_then(Value::as_object)
    else {
        findings.push(external("SRC002", "source_blobs missing"));
        return;
    };
    for (path, expected) in blobs {
        let Some(expected) = expected.as_str() else {
            findings.push(external("SRC002", format!("non-string blob {path}")));
            continue;
        };
        let revision_path = format!("HEAD:{path}");
        match git_output(root, &["rev-parse", &revision_path]) {
            Ok(actual) if actual == expected => {}
            Ok(actual) => findings.push(external(
                "SRC002",
                format!("{path}={actual}, expected {expected}"),
            )),
            Err(error) => findings.push(external("SRC002", error)),
        }
    }
}

fn check_current_owner_tokens(root: &Path, findings: &mut Vec<Finding>) {
    for (path, tokens) in [
        (
            "crates/arcweft-core/src/value.rs",
            &["pub enum RuntimeValue", "NominalRecord(", "Variant {"][..],
        ),
        (
            "crates/arcweft-core/src/pattern.rs",
            &[
                "pub enum RuntimeCheckedType",
                "pub enum RuntimeVariantIdentity",
            ][..],
        ),
        (
            "crates/arcweft-core/src/awbc/schema.rs",
            &["pub enum AwbcRuntimeType", "NominalRecord {"][..],
        ),
        (
            "crates/arcweft-lang-sema/src/env/rust_metadata.rs",
            &[
                "pub struct AcceptedRustTypeMetadataCatalog",
                "pub fn instantiate(",
            ][..],
        ),
        (
            "crates/arcweft-lang-sema/src/ownership.rs",
            &[
                "AcceptedNominalSemantics::Opaque",
                "MissingRuntimeSnapshotOwner",
            ][..],
        ),
        (
            "crates/arcweft-compiler/src/lower.rs",
            &["RegisteredSemanticWorld", "project_checked_runtime_nominal"][..],
        ),
    ] {
        match std::fs::read_to_string(root.join(path)) {
            Ok(text) => {
                for token in tokens {
                    if !text.contains(token) {
                        findings.push(external("OWN001", format!("{path} lacks {token}")));
                    }
                }
            }
            Err(error) => findings.push(external("OWN001", format!("read {path}: {error}"))),
        }
    }
}

fn check_cargo_direction(root: &Path, findings: &mut Vec<Finding>) {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output();
    let metadata: Value = match output {
        Ok(output) if output.status.success() => match serde_json::from_slice(&output.stdout) {
            Ok(value) => value,
            Err(error) => {
                findings.push(external(
                    "CARGO001",
                    format!("invalid metadata JSON: {error}"),
                ));
                return;
            }
        },
        Ok(output) => {
            findings.push(external(
                "CARGO001",
                String::from_utf8_lossy(&output.stderr),
            ));
            return;
        }
        Err(error) => {
            findings.push(external(
                "CARGO001",
                format!("launch cargo metadata: {error}"),
            ));
            return;
        }
    };
    let package_deps = |name: &str| -> Option<BTreeSet<String>> {
        metadata
            .pointer("/packages")?
            .as_array()?
            .iter()
            .find(|package| package.pointer("/name").and_then(Value::as_str) == Some(name))
            .and_then(|package| package.pointer("/dependencies")?.as_array())
            .map(|deps| {
                deps.iter()
                    .filter_map(|dep| dep.pointer("/name").and_then(Value::as_str))
                    .map(ToOwned::to_owned)
                    .collect()
            })
    };
    let rules = [
        (
            "arcweft-core",
            &[][..],
            &[
                "arcweft-lang-sema",
                "arcweft-runtime-plan",
                "arcweft-compiler",
            ][..],
        ),
        (
            "arcweft-lang-sema",
            &["arcweft-core"][..],
            &["arcweft-runtime-plan", "arcweft-compiler"][..],
        ),
        (
            "arcweft-runtime-plan",
            &["arcweft-core"][..],
            &["arcweft-lang-sema", "arcweft-compiler"][..],
        ),
        (
            "arcweft-compiler",
            &["arcweft-core", "arcweft-lang-sema", "arcweft-runtime-plan"][..],
            &[][..],
        ),
    ];
    for (package, required, forbidden) in rules {
        let Some(deps) = package_deps(package) else {
            findings.push(external("CARGO002", format!("missing package {package}")));
            continue;
        };
        for dependency in required {
            if !deps.contains(*dependency) {
                findings.push(external(
                    "CARGO002",
                    format!("{package} lacks {dependency}"),
                ));
            }
        }
        for dependency in forbidden {
            if deps.contains(*dependency) {
                findings.push(external(
                    "CARGO002",
                    format!("{package} illegally depends on {dependency}"),
                ));
            }
        }
    }
}

fn git_output(root: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
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

fn external(code: &'static str, detail: impl Into<String>) -> Finding {
    Finding {
        code,
        detail: detail.into(),
    }
}
