use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const TRACE_DIR: &str = "target/native-text-input-trace/seq06.16.3";
const TRACE_FILE: &str = "native-player-ime.real.json";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("arcweft-cli crate lives under crates/")
        .to_path_buf()
}

fn read(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn seq06_16_3_native_text_input_sample_is_component_authored() {
    let root = workspace_root();
    let sample = root.join("samples/native-text-input");
    let source = read(sample.join("src/main.arcw"));

    for removed in [
        "ui text_input",
        "ui text_area",
        "ui secure_field",
        "ui style",
    ] {
        assert!(
            !source.contains(removed),
            "native text-input smoke must not reintroduce removed top-level declaration {removed:?}"
        );
    }
    for required in [
        "entry game @entry.native_text_input_sample",
        "pub component NativeTextInputPanel() -> View",
        "TextField(id: @input:.jp_text_field",
        "TextArea(id: @input:.jp_text_area",
        "SecureField(id: @input:.secret_secure_field",
        "Local trace output belongs under target/native-text-input-trace/",
    ] {
        assert!(
            source.contains(required),
            "native text-input sample must contain {required:?}"
        );
    }
    for obsolete_sidecar in [
        "scene-contract.json",
        ".arcweft/content/ui.input.json",
        ".arcweft/content/ui.program.json",
        ".arcweft/content/ui.style.json",
        ".arcweft/content/ui.text.json",
    ] {
        assert!(
            !sample.join(obsolete_sidecar).exists(),
            "component-authored native text controls must not require obsolete sidecar {obsolete_sidecar}"
        );
    }
}

#[test]
fn seq06_16_3_submit_samples_share_player_backed_text_submit_routes() {
    let root = workspace_root();
    let text_submit = read(root.join("samples/text-submit-flow/src/main.arcw"));
    for required in [
        "TextField(id: @input:.feedback",
        r#"Button("Send", id: @button:.feedback_send)"#,
        ".on_click(ime: .commit)",
        "text_submit @input:.feedback",
        "let submitted = text_submit @input.feedback",
        "return submitted",
    ] {
        assert!(
            text_submit.contains(required),
            "text-submit-flow must retain shared submit route {required:?}"
        );
    }

    let modern = read(root.join("samples/modern-feedback-ui/src/main.arcw"));
    for required in [
        "TextField(id: @input:.visitor_name",
        "TextArea(id: @input:.product_brief",
        r#"Button("Continue", id: @button:.continue)"#,
        r#"Button("Send brief", id: @button:.send_brief)"#,
        "text_submit @input:.visitor_name",
        "text_submit @input:.product_brief",
        "let visitor_name = text_submit @input.visitor_name",
        "let brief = text_submit @input.product_brief",
    ] {
        assert!(
            modern.contains(required),
            "modern-feedback-ui must retain shared submit route {required:?}"
        );
    }
}

#[test]
fn seq06_16_3_native_smoke_command_and_trace_gate_are_documented() {
    let root = workspace_root();
    let note =
        read(root.join(
            "docs/implementation/component-text-input-native-interactive-smoke-2026-07-04.md",
        ));
    for required in [
        "samples/native-text-input",
        "samples/text-submit-flow",
        "samples/modern-feedback-ui",
        "cargo run -p arcweft-cli --features native-player -- run",
        "--text-input-trace-out target/native-text-input-trace/seq06.16.3/native-player-ime.real.json",
        "SecureField does not expose `sekret-1234`",
    ] {
        assert!(
            note.contains(required),
            "seq06.16.3 implementation note must document {required:?}"
        );
    }

    let justfile = read(root.join("Justfile"));
    assert!(justfile.contains("component-text-input-native-smoke-check"));
    assert!(justfile.contains("component-text-input-native-smoke"));

    let trace_gate = read(root.join("tools/verify-seq06-16-3-native-smoke-trace.rs"));
    assert!(trace_gate.contains("sekret-1234"));
    assert!(trace_gate.contains("runtime_write_back"));
}

#[test]
#[ignore = "opens the native player window and requires a real display, keyboard, and IME"]
fn seq06_16_3_launch_native_player_for_manual_smoke() {
    if env::var_os("ARCWEFT_SEQ06_16_3_INTERACTIVE").is_none() {
        eprintln!("set ARCWEFT_SEQ06_16_3_INTERACTIVE=1 to launch the native interactive smoke");
        return;
    }

    let root = workspace_root();
    let trace = root.join(TRACE_DIR).join(TRACE_FILE);
    let parent = trace.parent().expect("trace path has a parent");
    fs::create_dir_all(parent)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", parent.display()));

    let status = Command::new("cargo")
        .current_dir(&root)
        .args([
            "run",
            "-p",
            "arcweft-cli",
            "--features",
            "native-player",
            "--",
            "run",
            "--runner",
            "native",
            "samples/native-text-input/src/main.arcw",
            "--text-input-trace-out",
        ])
        .arg(&trace)
        .status()
        .expect("native player command starts");
    assert!(status.success(), "native player command failed: {status}");

    let trace_source = read(&trace);
    assert!(
        !trace_source.contains("sekret-1234"),
        "native text-input trace must redact SecureField plaintext"
    );
}
