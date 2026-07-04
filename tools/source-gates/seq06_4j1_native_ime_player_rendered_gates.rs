#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"
---

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = root_arg();
    let mut errors = Vec::new();
    let native_sample = "samples/native-text-input/src/main.arcw";

    assert_contains(
        &root,
        native_sample,
        "pub component NativeTextInputPanel() -> View",
        &mut errors,
    );
    for required in [
        "TextField(id: @input:.jp_text_field",
        "TextArea(id: @input:.jp_text_area",
        "SecureField(id: @input:.secret_secure_field",
        "jp_text_field, jp_text_area, and secret_secure_field",
    ] {
        assert_contains(&root, native_sample, required, &mut errors);
    }
    for removed in [
        "ui text_input",
        "ui text_area",
        "ui secure_field",
        "planned native TextField/TextArea/SecureField controls",
    ] {
        assert_not_contains(&root, native_sample, removed, &mut errors);
    }
    for obsolete_sidecar in [
        "samples/native-text-input/scene-contract.json",
        "samples/native-text-input/.arcweft/content/ui.input.json",
        "samples/native-text-input/.arcweft/content/ui.program.json",
        "samples/native-text-input/.arcweft/content/ui.style.json",
        "samples/native-text-input/.arcweft/content/ui.text.json",
    ] {
        assert_path_absent(
            &root,
            obsolete_sidecar,
            "component-authored controls must lower from source without top-level input sidecars",
            &mut errors,
        );
    }
    assert_contains(
        &root,
        "samples/native-text-input/README.md",
        "target/native-text-input-trace/",
        &mut errors,
    );
    assert_contains(
        &root,
        "crates/arcweft-cli/src/app/runtime/run.rs",
        "run_bundle_windowed_with_text_input_options",
        &mut errors,
    );
    assert_contains(
        &root,
        "crates/arcweft-cli/src/app/runtime/run.rs",
        "--text-input-trace-out requires --runner native",
        &mut errors,
    );
    assert_contains(
        &root,
        "crates/arcweft-player-native/src/text_input_bridge/trace.rs",
        "RuntimeWriteBack",
        &mut errors,
    );
    assert_contains(
        &root,
        "crates/arcweft-player-native/src/text_input_bridge/trace.rs",
        "secure_redacted",
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
        println!("seq06.4j.1 source gates passed for component-authored native text controls");
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
        Ok(_) => errors.push(format!("{rel} must not contain {needle:?}")),
        Err(error) => errors.push(format!("failed to read {rel}: {error}")),
    }
}

fn assert_path_absent(root: &Path, rel: &str, reason: &str, errors: &mut Vec<String>) {
    let path = root.join(rel);
    if path.exists() {
        errors.push(format!("{rel} must not exist: {reason}"));
    }
}
