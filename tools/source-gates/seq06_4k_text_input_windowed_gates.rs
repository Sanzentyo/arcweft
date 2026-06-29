#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"
---

//! Source gates for seq06.4k player text-input convergence and native windowed
//! path retirement.
//!
//! Run from the repository root:
//!
//! ```bash
//! cargo +nightly -Zscript tools/source-gates/seq06_4k_text_input_windowed_gates.rs --root .
//! ```

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = root_arg();
    let mut failures = Vec::new();

    check_shared_core(&root, &mut failures);
    check_native_bridge(&root, &mut failures);
    check_web_bridge(&root, &mut failures);
    check_renderer_focus_blocker(&root, &mut failures);
    check_windowed_path(&root, &mut failures);
    check_no_hidden_web_fallback(&root, &mut failures);

    if failures.is_empty() {
        println!("seq06.4k source gates passed");
        return;
    }

    eprintln!("seq06.4k source gates failed:");
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

fn check_shared_core(root: &Path, failures: &mut Vec<String>) {
    let rel = "crates/arcweft-runtime-host/src/player_text_input_bridge.rs";
    if !exists(root, rel) {
        failures.push(format!("{rel} is missing"));
        return;
    }
    let source = read(root, rel);
    require_contains(failures, rel, &source, "struct PlayerTextInputBridgeCore");
    require_contains(failures, rel, &source, "TextInputDispatchState");
    require_contains(failures, rel, &source, "dispatch_platform_event");
    require_contains(failures, rel, &source, "shortcuts_allowed");
    require_contains(failures, rel, &source, "TextInputBlurPolicy");
    require_contains(failures, rel, &source, "trait PlayerTextInputHostCommandSink");
}

fn check_native_bridge(root: &Path, failures: &mut Vec<String>) {
    let rel = "crates/arcweft-player-native/src/text_input_bridge.rs";
    let source = read(root, rel);
    require_contains(failures, rel, &source, "PlayerTextInputBridgeCore");
    require_contains(failures, rel, &source, "PlayerTextInputFocusedControl");
    require_contains(failures, rel, &source, "shortcuts_allowed");
    require_not_contains(failures, rel, &source, "dispatch: TextInputDispatchState");
    require_not_contains(failures, rel, &source, "struct NativeTextInputActiveFocus");
}

fn check_web_bridge(root: &Path, failures: &mut Vec<String>) {
    let runtime = read(root, "crates/arcweft-player-web/src/runtime_text_input.rs");
    require_contains(
        failures,
        "crates/arcweft-player-web/src/runtime_text_input.rs",
        &runtime,
        "PlayerTextInputBridgeCore",
    );
    require_contains(
        failures,
        "crates/arcweft-player-web/src/runtime_text_input.rs",
        &runtime,
        "core.shortcuts_allowed",
    );

    let adapter = read(root, "crates/arcweft-player-web/src/edit_context.rs");
    require_not_contains(
        failures,
        "crates/arcweft-player-web/src/edit_context.rs",
        &adapter,
        "dispatch: TextInputDispatchState",
    );
}

fn check_renderer_focus_blocker(root: &Path, failures: &mut Vec<String>) {
    let follow_up = "docs/reviews/requests/2026-06-29-seq-06.4k.1-real-text-control-schema-to-prepared-frame.md";
    let geometry = read(root, "crates/arcweft-render-wgpu/src/geometry.rs");
    let still_returns_none = geometry.contains("pub fn focused_text_input_target(&self) -> Option<PreparedTextInputTarget> {\n        let _ = self;\n        None\n    }");
    if still_returns_none && !exists(root, follow_up) {
        failures.push(format!(
            "PreparedFrame focus target still returns None and {follow_up} is missing"
        ));
    }
}

fn check_windowed_path(root: &Path, failures: &mut Vec<String>) {
    let lib = read(root, "crates/arcweft-player-native/src/lib.rs");
    require_contains(
        failures,
        "crates/arcweft-player-native/src/lib.rs",
        &lib,
        "pub use scene_windowed::",
    );
    require_not_contains(
        failures,
        "crates/arcweft-player-native/src/lib.rs",
        &lib,
        "run_bundle_adapter_windowed",
    );

    let scene = read(root, "crates/arcweft-player-native/src/scene_windowed.rs");
    require_contains(
        failures,
        "crates/arcweft-player-native/src/scene_windowed.rs",
        &scene,
        "runtime: WindowedRuntimeOwner,",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/scene_windowed.rs",
        &scene,
        "FrameBoundary::AfterRenderSubmitted",
    );

    if exists(root, "crates/arcweft-player-native/src/windowed.rs")
        && !exists(
            root,
            "docs/reviews/requests/2026-06-29-seq-06.4k.2-scene-windowed-owned-window-runtime-migration.md",
        )
    {
        failures.push(
            "windowed.rs still exists without the explicit seq06.4k.2 removal follow-up request"
                .to_owned(),
        );
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
