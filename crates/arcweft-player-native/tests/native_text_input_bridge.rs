use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("player-native crate lives under crates/")
        .to_path_buf()
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref())
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.as_ref().display()))
}

#[test]
fn bridge_is_native_player_integration_point_not_tsf_scene_loop() {
    let root = workspace_root();
    let scene = read(root.join("crates/arcweft-player-native/src/scene_windowed.rs"));
    let bridge = read(root.join("crates/arcweft-player-native/src/text_input_bridge.rs"));
    let backend = read(root.join("crates/arcweft-player-native/src/text_input_bridge/backend.rs"));

    assert!(scene.contains("text_input: NativeTextInputBridge,"));
    assert!(scene.contains("WindowEvent::Ime"));
    assert!(scene.contains("KeyEvent"));
    assert!(backend.contains("WinitWindowIme"));
    assert!(scene.contains("keyboard_with_ime"));
    assert!(bridge.contains("PlayerTextInputBridgeCore"));
    assert!(!scene.contains("WindowsTsfImeBridge"));
    assert!(!scene.contains("NSTextInputClient"));
    assert!(!backend.contains("WindowsTsf"));
    assert!(!backend.contains("MacosAppKit"));
}

#[test]
fn cli_native_runner_owns_text_input_trace_flag() {
    let root = workspace_root();
    let options = read(root.join("crates/arcweft-cli/src/app/runtime/options.rs"));
    let run = read(root.join("crates/arcweft-cli/src/app/runtime/run.rs"));

    assert!(options.contains("text_input_trace_out"));
    assert!(options.contains("text-input-trace-out"));
    assert!(run.contains("NativeTextInputTraceOptions::write_to"));
    assert!(run.contains("--text-input-trace-out requires --runner native"));
}
