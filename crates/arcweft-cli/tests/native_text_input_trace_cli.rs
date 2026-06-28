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
