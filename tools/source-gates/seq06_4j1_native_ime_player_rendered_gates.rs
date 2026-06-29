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
use std::path::{Path, PathBuf};

fn main() {
    let root = root_arg();
    let mut errors = Vec::new();
    assert_contains(
        &root,
        "samples/native-text-input/src/main.arcw",
        "jp_text_field, jp_text_area, and secret_secure_field",
        &mut errors,
    );
    assert_not_contains(
        &root,
        "samples/native-text-input/src/main.arcw",
        "planned native TextField/TextArea/SecureField controls",
        &mut errors,
    );
    assert_ui_input_controls(&root, &mut errors);
    assert_contains(
        &root,
        "crates/arcweft-player-native/src/text_input_bridge/trace.rs",
        "RuntimeWriteBack",
        &mut errors,
    );
    assert_contains(
        &root,
        "crates/arcweft-render-wgpu/src/geometry.rs",
        "keyboard_focus_targets",
        &mut errors,
    );
    assert_contains(
        &root,
        "samples/ime-css-fonts/README.md",
        "diagnostics only",
        &mut errors,
    );

    if errors.is_empty() {
        println!("seq06.4j.1 source gates passed");
    } else {
        for error in errors {
            eprintln!("error: {error}");
        }
        std::process::exit(1);
    }
}

fn root_arg() -> PathBuf {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--root" {
            return PathBuf::from(args.next().expect("--root value"));
        }
    }
    PathBuf::from(".")
}

fn assert_contains(root: &Path, rel: &str, needle: &str, errors: &mut Vec<String>) {
    match fs::read_to_string(root.join(rel)) {
        Ok(source) if source.contains(needle) => {}
        Ok(_) => errors.push(format!("{rel} must contain {needle:?}")),
        Err(error) => errors.push(format!("failed to read {rel}: {error}")),
    }
}

fn assert_not_contains(root: &Path, rel: &str, needle: &str, errors: &mut Vec<String>) {
    match fs::read_to_string(root.join(rel)) {
        Ok(source) if !source.contains(needle) => {}
        Ok(_) => errors.push(format!("{rel} must not contain stale placeholder {needle:?}")),
        Err(error) => errors.push(format!("failed to read {rel}: {error}")),
    }
}

fn assert_ui_input_controls(root: &Path, errors: &mut Vec<String>) {
    let path = root.join("samples/native-text-input/.arcweft/content/ui.input.json");
    let Ok(source) = fs::read_to_string(&path) else {
        errors.push(format!("failed to read {}", path.display()));
        return;
    };
    let Ok(value) = serde_json::from_str::<Value>(&source) else {
        errors.push(format!("{} is not valid JSON", path.display()));
        return;
    };
    let ids = value["options"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|option| option["public_id"].as_str())
        .collect::<Vec<_>>();
    for required in ["jp_text_field", "jp_text_area", "secret_secure_field"] {
        if !ids.contains(&required) {
            errors.push(format!("ui.input.json missing {required}"));
        }
    }
}
