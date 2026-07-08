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
fn seq06_16_3_native_text_input_sample_is_view_authored() {
    let root = workspace_root();
    let sample = root.join("samples/native-text-input");
    let source = read(sample.join("src/main.arcw"));

    for removed in [
        "view text_input",
        "view text_area",
        "view secure_field",
        "view style",
    ] {
        assert!(
            !source.contains(removed),
            "native text-input smoke must not reintroduce removed top-level declaration {removed:?}"
        );
    }
    for required in [
        "entry game @entry.native_text_input_sample",
        "pub view NativeTextInputPanel()",
        "view(@view:.NativeTextInputPanel)",
        "let jp_text_field = input.text(@input:.jp_text_field",
        "TextField(jp_text_field)",
        "let jp_text_area = input.text(@input:.jp_text_area",
        "TextArea(jp_text_area)",
        "let secret_secure_field = input.secure(@input:.secret_secure_field",
        "SecureField(secret_secure_field)",
        "Local trace output belongs under target/native-text-input-trace/",
    ] {
        assert!(
            source.contains(required),
            "native text-input sample must contain {required:?}"
        );
    }
    for obsolete_sidecar in [
        "scene-contract.json",
        ".arcweft/content/view.input.json",
        ".arcweft/content/view.program.json",
        ".arcweft/content/view.style.json",
        ".arcweft/content/view.text.json",
    ] {
        assert!(
            !sample.join(obsolete_sidecar).exists(),
            "view-authored native text controls must not require obsolete sidecar {obsolete_sidecar}"
        );
    }
}

#[test]
fn seq06_16_3_submit_samples_share_player_backed_semantic_action_routes() {
    let root = workspace_root();
    let submit_sample = read(root.join("samples/text-submit-flow/src/main.arcw"));
    for required in [
        "view(@view:.FeedbackForm)",
        "pub action feedback.submit(value: String)",
        "let feedback = input.text(@input:.feedback",
        "TextField(feedback)",
        ".on_submit {",
        "Button(@button:.feedback_send, label = \"Send\")",
        "action.invoke(@action:.feedback.submit, value = feedback.text)",
        "let event = receive action(@action:.feedback.submit)",
        "let submitted = event.value",
        "return submitted",
    ] {
        assert!(
            submit_sample.contains(required),
            "text-submit-flow must retain shared submit route {required:?}"
        );
    }

    let modern = read(root.join("samples/modern-feedback-view/src/main.arcw"));
    for required in [
        "let panel = view(@view:.ModernFeedbackPanel",
        "pub action feedback.submit_name(value: String)",
        "pub action feedback.submit_brief(value: String)",
        "TextField(visitor_name)",
        "TextArea(product_brief)",
        "Button(@button:.continue, label = \"Continue\")",
        "Button(@button:.send_brief, label = \"Send brief\")",
        "action.invoke(@action:.feedback.submit_name, value = visitor_name.text)",
        "action.invoke(@action:.feedback.submit_brief, value = product_brief.text)",
        "let name_event = receive action(@action:.feedback.submit_name)",
        "let visitor_name = name_event.value",
        "let brief_event = receive action(@action:.feedback.submit_brief)",
        "let brief = brief_event.value",
    ] {
        assert!(
            modern.contains(required),
            "modern-feedback-view must retain shared submit route {required:?}"
        );
    }
    assert!(
        !modern.contains("panel.close()"),
        "modern-feedback-view must rely on scope-owned view cleanup rather than the removed close alias"
    );
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
        "samples/modern-feedback-view",
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
