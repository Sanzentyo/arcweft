#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"
---

//! Source gates for seq06.4k text-input/windowed convergence plus seq06.4k.1
//! real text-control lowering to `PreparedFrame`.
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
    check_real_prepared_frame_text_input(&root, &mut failures);
    check_player_owned_text_editor(&root, &mut failures);
    check_windowed_path(&root, &mut failures);
    check_no_hidden_web_fallback(&root, &mut failures);

    if failures.is_empty() {
        println!("seq06.4k / seq06.4k.1 source gates passed");
        return;
    }

    eprintln!("seq06.4k / seq06.4k.1 source gates failed:");
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
    let runtime_rel = "crates/arcweft-player-web/src/runtime_text_input.rs";
    let runtime = read(root, runtime_rel);
    require_contains(
        failures,
        runtime_rel,
        &runtime,
        "PlayerTextInputBridgeCore",
    );
    require_contains(
        failures,
        runtime_rel,
        &runtime,
        "core.shortcuts_allowed",
    );
    require_contains(
        failures,
        runtime_rel,
        &runtime,
        "focused_text_input_target()",
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
    let geometry = read(root, "crates/arcweft-render-wgpu/src/geometry.rs");
    let still_returns_none = geometry.contains("pub fn focused_text_input_target(&self) -> Option<PreparedTextInputTarget> {\n        let _ = self;\n        None\n    }");
    if still_returns_none {
        failures.push(
            "PreparedFrame focus target must not contain the hardcoded None implementation"
                .to_owned(),
        );
    }
}

fn check_real_prepared_frame_text_input(root: &Path, failures: &mut Vec<String>) {
    let rel = "crates/arcweft-render-wgpu/src/geometry.rs";
    let source = read(root, rel);
    require_contains(
        failures,
        rel,
        &source,
        "pub text_inputs: Vec<RenderTextInputControl>",
    );
    require_contains(
        failures,
        rel,
        &source,
        "focused_text_input: Option<PreparedTextInputTarget>",
    );

    let rel = "crates/arcweft-render-wgpu/src/geometry/text_controls.rs";
    let source = read(root, rel);
    require_contains(
        failures,
        rel,
        &source,
        "TextEditorGeometryPump::layout_from_laid_out_text",
    );
    require_contains(
        failures,
        rel,
        &source,
        "TextEditorState::from_text_control",
    );
    require_contains(
        failures,
        rel,
        &source,
        "TextInputSecurityPolicy::from_options",
    );
    require_contains(failures, rel, &source, "pub struct RenderTextInputControl");
}

fn check_player_owned_text_editor(root: &Path, failures: &mut Vec<String>) {
    let rel = "crates/arcweft-player-scene/src/input.rs";
    let source = read(root, rel);
    require_contains(
        failures,
        rel,
        &source,
        "focused_text_editor: Option<TextEditorState>",
    );
    require_contains(failures, rel, &source, "activate_text_control");
    require_contains(failures, rel, &source, "apply_live_text_control_state");
    require_contains(failures, rel, &source, "editor.apply_text_input");
}

fn check_windowed_path(root: &Path, failures: &mut Vec<String>) {
    let lib = read(root, "crates/arcweft-player-native/src/lib.rs");
    require_contains(
        failures,
        "crates/arcweft-player-native/src/lib.rs",
        &lib,
        "pub use scene_windowed::",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/lib.rs",
        &lib,
        "mod window_driver;",
    );
    require_not_contains(
        failures,
        "crates/arcweft-player-native/src/lib.rs",
        &lib,
        "mod windowed;",
    );
    require_not_contains(
        failures,
        "crates/arcweft-player-native/src/lib.rs",
        &lib,
        "run_bundle_adapter_windowed",
    );

    let window_driver_rel = "crates/arcweft-player-native/src/window_driver.rs";
    if !exists(root, window_driver_rel) {
        failures.push(format!("{window_driver_rel} is missing"));
    } else {
        let window_driver = read(root, window_driver_rel);
        require_contains(
            failures,
            window_driver_rel,
            &window_driver,
            "struct WinitOwnedWindowDriver",
        );
        require_contains(
            failures,
            window_driver_rel,
            &window_driver,
            "struct WindowCloseSignal",
        );
        require_contains(
            failures,
            window_driver_rel,
            &window_driver,
            "impl OwnedWindowDriver for WinitOwnedWindowDriver",
        );
        require_contains(
            failures,
            window_driver_rel,
            &window_driver,
            "OwnedWindowRequest::RequestClose",
        );
        require_contains(
            failures,
            window_driver_rel,
            &window_driver,
            "OwnedCursorRequest::SetGrab",
        );
    }

    if exists(root, "crates/arcweft-player-native/src/windowed.rs") {
        failures.push(
            "crates/arcweft-player-native/src/windowed.rs must be absent after seq06.4k.2 migration"
                .to_owned(),
        );
    }

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
    require_contains(
        failures,
        "crates/arcweft-player-native/src/scene_windowed.rs",
        &scene,
        "focused_text_input_target()",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/scene_windowed.rs",
        &scene,
        "WinitOwnedWindowDriver::try_new",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/scene_windowed.rs",
        &scene,
        ".with_owned_window_driver(owned_window)",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/scene_windowed.rs",
        &scene,
        "WindowedRuntimeOwner::from_bundle_with_desktop_backend",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/scene_windowed.rs",
        &scene,
        "pump_main_thread",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/scene_windowed.rs",
        &scene,
        "push_audio_events",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/scene_windowed.rs",
        &scene,
        "close_signal.take()",
    );

    let runtime = read(root, "crates/arcweft-player-native/src/windowed_runtime.rs");
    require_contains(
        failures,
        "crates/arcweft-player-native/src/windowed_runtime.rs",
        &runtime,
        "from_bundle_with_desktop_backend",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/windowed_runtime.rs",
        &runtime,
        "NativeTaskBridge",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/windowed_runtime.rs",
        &runtime,
        "pump_main_thread",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/windowed_runtime.rs",
        &runtime,
        "push_audio_events",
    );
    require_contains(
        failures,
        "crates/arcweft-player-native/src/windowed_runtime.rs",
        &runtime,
        "complete_requested_tasks",
    );
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
