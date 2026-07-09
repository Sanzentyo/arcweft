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
        "pub view NativeTextInputPanel()",
        &mut errors,
    );
    for required in [
        "let jp_text_field = input.text(@input:.jp_text_field",
        "let jp_text_area = input.text(@input:.jp_text_area",
        "let secret_secure_field = input.secure(@input:.secret_secure_field",
        "TextField(jp_text_field)",
        "TextArea(jp_text_area)",
        "SecureField(secret_secure_field)",
        "jp_text_field, jp_text_area, and secret_secure_field",
    ] {
        assert_contains(&root, native_sample, required, &mut errors);
    }
    for removed in ["text_input @input", "text_area @input", "secure_field @input"] {
        assert_not_contains(&root, native_sample, removed, &mut errors);
    }
    for generated_control_sidecar in [
        "samples/native-text-input/scene-contract.json",
        "samples/native-text-input/content/view.input.json",
        "samples/native-text-input/content/view.program.json",
        "samples/native-text-input/content/view.style.json",
        "samples/native-text-input/content/view.text.json",
    ] {
        assert_path_absent(
            &root,
            generated_control_sidecar,
            "view-authored controls must lower from source without checked-in input sidecars",
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
        "run_bundle_windowed_with_options",
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
        println!("seq06.4j.1 source gates passed for view-authored native text controls");
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
