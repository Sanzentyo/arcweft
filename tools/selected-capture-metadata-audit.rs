#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"
---

//! Source gate for seq06.5 selected capture metadata.
//!
//! Native adapters must construct `AgentSelectedCaptureMetadata` through typed
//! protocol builders. They must not reintroduce adapter-local JSON metadata for
//! selected capture shape.

use std::{env, fs, path::{Path, PathBuf}, process::ExitCode};

const NATIVE_REL: &str = "crates/arcweft-cli/src/app/agent/native";
const FORBIDDEN_JSON_KEYS: &[&str] = &[
    "\"selected_capture\"",
    "\"capture_metadata\"",
    "\"coordinate_basis\"",
    "\"crop_bounds\"",
    "\"mask_metadata\"",
    "\"source_identity\"",
];

fn main() -> ExitCode {
    let root = parse_root().unwrap_or_else(|| PathBuf::from("."));
    let native = root.join(NATIVE_REL);
    let mut failures = Vec::new();
    visit_rs_files(&native, &mut |path| {
        let Ok(source) = fs::read_to_string(path) else { return; };
        if !source.contains("serde_json::json!") {
            return;
        }
        for key in FORBIDDEN_JSON_KEYS {
            if source.contains(key) {
                failures.push(format!("{} contains adapter-local JSON key {key}", path.display()));
            }
        }
    });
    if failures.is_empty() {
        println!("selected capture metadata audit: ok");
        ExitCode::SUCCESS
    } else {
        for failure in failures {
            eprintln!("selected capture metadata audit: {failure}");
        }
        ExitCode::FAILURE
    }
}

fn parse_root() -> Option<PathBuf> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--root" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

fn visit_rs_files(path: &Path, f: &mut impl FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(path) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, f);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            f(&path);
        }
    }
}
