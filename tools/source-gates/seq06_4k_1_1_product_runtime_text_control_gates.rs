#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"
---

//! Source gates for seq06.4k.1.1 product/runtime text-control emission.
//!
//! Run from the repository root:
//!
//! ```bash
//! cargo +nightly -Zscript tools/source-gates/seq06_4k_1_1_product_runtime_text_control_gates.rs --root .
//! ```

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = root_arg();
    let mut failures = Vec::new();

    check_runtime_model(&root, &mut failures);
    check_shared_lowering(&root, &mut failures);
    check_normal_scene_builders(&root, &mut failures);
    check_no_direct_platform_geometry_bypass(&root, &mut failures);
    check_no_hidden_web_fallback(&root, &mut failures);

    if failures.is_empty() {
        println!("seq06.4k.1.1 product/runtime text-control gates passed");
        return;
    }

    eprintln!("seq06.4k.1.1 product/runtime text-control gates failed:");
    for failure in failures {
        eprintln!("- {failure}");
    }
    std::process::exit(1);
}

fn root_arg() -> PathBuf {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--root" {
            return PathBuf::from(args.next().expect("--root requires a path"));
        }
    }
    PathBuf::from(".")
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_else(|error| {
        panic!("failed to read {rel}: {error}");
    })
}

fn exists(root: &Path, rel: &str) -> bool {
    root.join(rel).exists()
}

fn require_contains(failures: &mut Vec<String>, label: &str, source: &str, needle: &str) {
    if !source.contains(needle) {
        failures.push(format!("{label} must contain `{needle}`"));
    }
}

fn require_not_contains(failures: &mut Vec<String>, label: &str, source: &str, needle: &str) {
    if source.contains(needle) {
        failures.push(format!("{label} must not contain `{needle}`"));
    }
}

fn check_runtime_model(root: &Path, failures: &mut Vec<String>) {
    let model_rel = "crates/arcweft-bundle/src/resource_codec/ui/model.rs";
    let model = read(root, model_rel);
    require_contains(failures, model_rel, &model, "pub struct UiRuntimeTextControl");
    require_contains(failures, model_rel, &model, "pub struct UiRuntimeTextControlBounds");
    require_contains(failures, model_rel, &model, "pub struct UiRuntimeTextSelection");
    require_contains(failures, model_rel, &model, "runtime_text_controls(");
    require_contains(failures, model_rel, &model, "runtime_text_session(");

    let display_rel = "crates/arcweft-runtime-driver/src/display.rs";
    let display = read(root, display_rel);
    require_contains(
        failures,
        display_rel,
        &display,
        "pub text_inputs: Vec<UiRuntimeTextControl>",
    );
    require_contains(failures, display_rel, &display, "text_inputs: &[UiRuntimeTextControl]");

    let session_rel = "crates/arcweft-runtime-driver/src/session.rs";
    let session = read(root, session_rel);
    require_contains(failures, session_rel, &session, "text_inputs: Vec<UiRuntimeTextControl>");
    require_contains(failures, session_rel, &session, "input.runtime_text_controls(");
}

fn check_shared_lowering(root: &Path, failures: &mut Vec<String>) {
    let lib_rel = "crates/arcweft-player-scene/src/lib.rs";
    let lib = read(root, lib_rel);
    require_contains(failures, lib_rel, &lib, "pub mod text_controls;");

    let lowerer_rel = "crates/arcweft-player-scene/src/text_controls.rs";
    if !exists(root, lowerer_rel) {
        failures.push(format!("{lowerer_rel} is missing"));
        return;
    }
    let lowerer = read(root, lowerer_rel);
    require_contains(failures, lowerer_rel, &lowerer, "struct RuntimeTextControlLowerer");
    require_contains(failures, lowerer_rel, &lowerer, "lower_for_frame");
    require_contains(failures, lowerer_rel, &lowerer, "apply_live_text_control_state");
    require_contains(failures, lowerer_rel, &lowerer, "activate_text_control");
    require_contains(failures, lowerer_rel, &lowerer, "RenderTextInputControl::new");
}

fn check_normal_scene_builders(root: &Path, failures: &mut Vec<String>) {
    let native_rel = "crates/arcweft-player-native/src/scene_windowed.rs";
    let native = read(root, native_rel);
    require_not_contains(failures, native_rel, &native, "text_inputs: Vec::new()");
    require_contains(
        failures,
        native_rel,
        &native,
        "RuntimeTextControlLowerer::lower_for_frame",
    );
    require_contains(failures, native_rel, &native, "&presentation.text_inputs");

    let web_rel = "crates/arcweft-player-web/src/app.rs";
    let web = read(root, web_rel);
    require_not_contains(failures, web_rel, &web, "text_inputs: Vec::new()");
    require_contains(
        failures,
        web_rel,
        &web,
        "RuntimeTextControlLowerer::lower_for_frame",
    );
    require_contains(failures, web_rel, &web, "&presentation.text_inputs");
}

fn check_no_direct_platform_geometry_bypass(root: &Path, failures: &mut Vec<String>) {
    for rel in [
        "crates/arcweft-player-native/src/scene_windowed.rs",
        "crates/arcweft-player-web/src/app.rs",
    ] {
        let source = read(root, rel);
        require_not_contains(failures, rel, &source, "TextInputClientSnapshot::new");
        require_not_contains(failures, rel, &source, "TextInputGeometrySnapshot::new");
        require_not_contains(failures, rel, &source, "TextEditorGeometryPump");
    }
}

fn check_no_hidden_web_fallback(root: &Path, failures: &mut Vec<String>) {
    for rel in [
        "crates/arcweft-player-web/src/edit_context.rs",
        "crates/arcweft-player-web/src/runtime_text_input.rs",
        "web/player-editcontext.js",
    ] {
        if !exists(root, rel) {
            continue;
        }
        let source = read(root, rel);
        require_not_contains(failures, rel, &source, "contenteditable");
        require_not_contains(failures, rel, &source, "textarea");
        require_not_contains(failures, rel, &source, "fallback_installed: true");
    }
}
