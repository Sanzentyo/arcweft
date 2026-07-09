#[test]
#[cfg(all(target_os = "macos", feature = "macos-text-input"))]
fn macos_text_input_adapter_is_exported_on_macos() {
    let _ = core::any::type_name::<
        arcweft_desktop_native::text_input::macos_text_input::MacosTextInputAdapter,
    >();
}
