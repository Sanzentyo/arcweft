#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"
[dependencies]
serde_json = "1"
---

use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;

const DEFAULT_SECRET_PROBE: &str = "sekret-1234";

fn main() {
    let options = Options::parse();
    let mut errors = Vec::new();

    let trace_source = read_to_string(&options.trace, &mut errors).unwrap_or_default();
    if trace_source.contains(&options.expected_secret) {
        errors.push(format!(
            "trace {} leaked SecureField probe value {:?}",
            options.trace.display(),
            options.expected_secret
        ));
    }

    for observation in &options.observations {
        if let Some(source) = read_to_string(observation, &mut errors) {
            if source.contains(&options.expected_secret) {
                errors.push(format!(
                    "observation {} leaked SecureField probe value {:?}",
                    observation.display(),
                    options.expected_secret
                ));
            }
        }
    }

    match serde_json::from_str::<Value>(&trace_source) {
        Ok(Value::Array(records)) => validate_records(&records, &mut errors),
        Ok(_) => errors.push(format!(
            "trace {} must be a JSON array of native text-input records",
            options.trace.display()
        )),
        Err(error) => errors.push(format!(
            "trace {} is not valid JSON: {error}",
            options.trace.display()
        )),
    }

    if errors.is_empty() {
        println!(
            "seq06.16.3 native text-input smoke trace passed: {}",
            options.trace.display()
        );
    } else {
        for error in errors {
            eprintln!("error: {error}");
        }
        std::process::exit(1);
    }
}

#[derive(Debug)]
struct Options {
    trace: PathBuf,
    observations: Vec<PathBuf>,
    expected_secret: String,
}

impl Options {
    fn parse() -> Self {
        let mut args = env::args().skip(1);
        let mut trace = None;
        let mut observations = Vec::new();
        let mut expected_secret = DEFAULT_SECRET_PROBE.to_owned();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--trace" => trace = args.next().map(PathBuf::from),
                "--observation" => {
                    let value = args.next().expect("--observation value");
                    observations.push(PathBuf::from(value));
                }
                "--expected-secret" => {
                    expected_secret = args.next().expect("--expected-secret value");
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => panic!("unknown argument {other:?}; use --help"),
            }
        }

        Self {
            trace: trace.expect("--trace is required"),
            observations,
            expected_secret,
        }
    }
}

fn print_help() {
    println!(
        r#"verify seq06.16.3 native text-input smoke trace

usage: cargo +nightly -Zscript tools/verify-seq06-16-3-native-smoke-trace.rs \
  --trace target/native-text-input-trace/seq06.16.3/native-player-ime.real.json \
  [--observation path/to/observe.json] [--expected-secret sekret-1234]"#
    );
}

fn read_to_string(path: &PathBuf, errors: &mut Vec<String>) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(source) => Some(source),
        Err(error) => {
            errors.push(format!("failed to read {}: {error}", path.display()));
            None
        }
    }
}

fn validate_records(records: &[Value], errors: &mut Vec<String>) {
    for required in [
        "backend_selected",
        "capabilities",
        "focus",
        "geometry",
        "routed_text_input",
        "runtime_write_back",
    ] {
        if !has_record(records, required) {
            errors.push(format!("trace missing required {required:?} record"));
        }
    }

    for target in ["jp_text_field", "jp_text_area", "secret_secure_field"] {
        if !records.iter().any(|record| record_mentions(record, target)) {
            errors.push(format!(
                "trace does not mention required text-control target {target:?}"
            ));
        }
    }

    if !records.iter().any(is_submit_write_back) {
        errors.push("trace missing submit RuntimeWriteBack record".to_owned());
    }
    if !records.iter().any(is_change_write_back) {
        errors.push("trace missing change RuntimeWriteBack record".to_owned());
    }
    if !records.iter().any(is_routed_commit_or_command) {
        errors.push("trace missing routed commit/command text input record".to_owned());
    }

    let secure_records = records
        .iter()
        .filter(|record| record.get("secure_redacted") == Some(&Value::Bool(true)))
        .collect::<Vec<_>>();
    if secure_records.is_empty() {
        errors.push("trace missing secure_redacted=true evidence for SecureField".to_owned());
    }
    for record in secure_records {
        if record_name(record) == Some("runtime_write_back")
            && record.get("value_len").and_then(Value::as_u64) != Some(0)
        {
            errors.push("secure RuntimeWriteBack must report value_len=0".to_owned());
        }
    }
}

fn has_record(records: &[Value], name: &str) -> bool {
    records.iter().any(|record| record_name(record) == Some(name))
}

fn record_name(record: &Value) -> Option<&str> {
    record.get("record").and_then(Value::as_str)
}

fn record_mentions(record: &Value, needle: &str) -> bool {
    serde_json::to_string(record)
        .map(|source| source.contains(needle))
        .unwrap_or(false)
}

fn is_submit_write_back(record: &Value) -> bool {
    record_name(record) == Some("runtime_write_back")
        && record.get("kind").and_then(Value::as_str) == Some("submit")
}

fn is_change_write_back(record: &Value) -> bool {
    record_name(record) == Some("runtime_write_back")
        && record.get("kind").and_then(Value::as_str) == Some("change")
}

fn is_routed_commit_or_command(record: &Value) -> bool {
    if record_name(record) != Some("routed_text_input") {
        return false;
    }
    record
        .get("operation_kinds")
        .and_then(Value::as_array)
        .is_some_and(|operations| {
            operations.iter().any(|operation| {
                matches!(operation.as_str(), Some("commit" | "command" | "end_composition"))
            })
        })
}
