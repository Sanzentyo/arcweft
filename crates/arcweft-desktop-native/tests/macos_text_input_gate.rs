#[test]
#[cfg(target_os = "macos")]
fn macos_text_input_module_is_available_on_macos() {
    let _ = core::any::type_name::<
        arcweft_desktop_native::text_input::macos_text_input::MacosTextInputAdapter,
    >();
}

#[test]
#[cfg(not(target_os = "macos"))]
fn macos_text_input_module_is_not_exported_off_macos() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/text_input.rs"))
        .expect("text input module source should be readable");
    assert!(source.contains("target_os = \"macos\""));
    assert!(source.contains("macos-text-input"));
}
