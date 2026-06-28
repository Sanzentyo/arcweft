use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_NATIVE_IDENTITY_TOKENS: &[&str] = &[
    "HWND",
    "ITf",
    "ITextStoreACP",
    "TfEditCookie",
    "NSRange",
    "NSView",
    "UITextInput",
    "wl_text_input",
];

const SANS_IO_FILES: &[&str] = &[
    "crates/arcweft-presentation/src/text_input.rs",
    "crates/arcweft-presentation/src/text_editor.rs",
    "crates/arcweft-runtime-host/src/text_input_dispatch.rs",
    "crates/arcweft-player-scene/src/input.rs",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("player-native crate lives under crates/")
        .to_path_buf()
}

#[test]
fn native_backend_identity_does_not_leak_into_sans_io_text_input_paths() {
    let root = workspace_root();
    let violations = SANS_IO_FILES
        .iter()
        .flat_map(|relative| {
            let path = root.join(relative);
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            FORBIDDEN_NATIVE_IDENTITY_TOKENS
                .iter()
                .filter(move |token| source.contains(**token))
                .map(move |token| {
                    format!(
                        "{} contains native identity token `{token}`",
                        path.display()
                    )
                })
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "native identity leaked into Sans I/O paths:\n{}",
        violations.join("\n")
    );
}

#[test]
fn diagnostic_samples_are_not_native_player_acceptance_surface() {
    let root = workspace_root();
    let validation = fs::read_to_string(
        root.join("docs/implementation/seq06-4j-native-player-platform-text-input-bridge.md"),
    )
    .unwrap_or_default();
    let cargo = fs::read_to_string(root.join("crates/arcweft-player-native/Cargo.toml"))
        .unwrap_or_default();

    assert!(cargo.contains("windows-tsf-ime-sample"));
    assert!(validation.contains("diagnostic harness only"));
    assert!(validation.contains("arcw run --runner native"));
}
