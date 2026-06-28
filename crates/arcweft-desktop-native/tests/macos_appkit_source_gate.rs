use std::fs;
use std::path::Path;

#[test]
fn macos_appkit_bridge_is_feature_and_target_gated() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo = fs::read_to_string(root.join("Cargo.toml")).expect("Cargo.toml reads");
    let text_input =
        fs::read_to_string(root.join("src/text_input.rs")).expect("text_input.rs reads");

    assert!(cargo.contains("macos-appkit-text-input"));
    assert!(cargo.contains("dep:serde"));
    assert!(cargo.contains("dep:serde_json"));
    assert!(text_input.contains("target_os = \"macos\""));
    assert!(text_input.contains("feature = \"macos-appkit-text-input\""));
    assert!(text_input.contains("pub mod macos_appkit_bridge;"));
}

#[test]
fn swift_owner_uses_json_bridge_not_rust_ffi_symbols() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let swift = fs::read_to_string(root.join("native/macos/ArcweftTextInputClientView.swift"))
        .expect("Swift bridge source reads");

    assert!(swift.contains("NSTextInputClient"));
    assert!(swift.contains("JSONSerialization"));
    assert!(swift.contains("NSRange"));
    assert!(swift.contains("firstRect(forCharacterRange"));
    assert!(swift.contains("UInt64.max"));
    assert!(!swift.contains("arcweft_macos_text_input_"));
}

#[test]
fn appkit_bridge_process_boundary_contains_no_unsafe_rust() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bridge = fs::read_to_string(root.join("src/text_input/macos_appkit_bridge.rs"))
        .expect("bridge source reads");

    assert!(!bridge.contains("unsafe"));
    assert!(!bridge.contains("extern \"C\""));
    assert!(bridge.contains("Command::new"));
    assert!(bridge.contains("serde_json::"));
}
