use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("arcweft-cli crate lives under crates/")
        .to_path_buf()
}

#[test]
fn runtime_run_trace_flag_is_native_runner_only() {
    let root = workspace_root();
    let options = fs::read_to_string(root.join("crates/arcweft-cli/src/app/runtime/options.rs"))
        .expect("runtime options source exists");
    let run = fs::read_to_string(root.join("crates/arcweft-cli/src/app/runtime/run.rs"))
        .expect("runtime run source exists");

    assert!(options.contains("text_input_trace_out"));
    assert!(options.contains("text-input-trace-out"));
    assert!(run.contains("text_input_trace_out.is_some()"));
    assert!(run.contains("CliRuntimeRunner::Native"));
    assert!(run.contains("NativeTextInputBridgeOptions"));
}

#[test]
fn runtime_run_session_save_flags_are_native_player_only() {
    let root = workspace_root();
    let options = fs::read_to_string(root.join("crates/arcweft-cli/src/app/runtime/options.rs"))
        .expect("runtime options source exists");
    let run = fs::read_to_string(root.join("crates/arcweft-cli/src/app/runtime/run.rs"))
        .expect("runtime run source exists");
    let native =
        fs::read_to_string(root.join("crates/arcweft-player-native/src/scene_windowed.rs"))
            .expect("native scene source exists");

    for required in [
        "session_load",
        "session-load",
        "session_save_out",
        "session-save-out",
    ] {
        assert!(
            options.contains(required),
            "runtime options must expose {required:?}"
        );
    }
    assert!(run.contains("has_session_save_options(options)"));
    assert!(run.contains("--session-load and --session-save-out require --runner native"));
    assert!(run.contains("--session-load and --session-save-out cannot be combined with --watch"));
    assert!(run.contains("with_session_load_path"));
    assert!(run.contains("with_session_save_out_path"));
    assert!(native.contains("arcweft.native_player_session"));
    assert!(native.contains("runtime_session: Vec<u8>"));
    assert!(native.contains("input: InputControllerSnapshot"));
}
