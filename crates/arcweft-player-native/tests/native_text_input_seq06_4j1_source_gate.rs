#[test]
fn bridge_trace_records_selected_backend_and_runtime_write_back() {
    let trace = include_str!("../src/text_input_bridge/trace.rs");

    assert!(trace.contains("BackendSelected"));
    assert!(trace.contains("RuntimeWriteBack"));
    assert!(trace.contains("secure_redacted"));
}

#[test]
fn normal_player_backend_identity_is_winit_owned_without_platform_identity_leak() {
    let backend = include_str!("../src/text_input_bridge/backend.rs");

    assert!(backend.contains("WinitWindowIme"));
    for token in [
        "WindowsTsf",
        "MacosAppKit",
        "InputConnection",
        "ViewTextInput",
    ] {
        assert!(
            !backend.contains(token),
            "normal native player backend should not name {token}"
        );
    }

    let shared = [
        "crates/arcweft-presentation/src/text_input.rs",
        "crates/arcweft-render-wgpu/src/geometry.rs",
        "crates/arcweft-player-scene/src/input.rs",
    ];
    for path in shared {
        let source = std::fs::read_to_string(path).unwrap_or_default();
        for forbidden in [
            "HWND",
            "NSRange",
            "InputConnection",
            "UITextInput",
            "wl_text_input",
        ] {
            assert!(!source.contains(forbidden), "{path} leaks {forbidden}");
        }
    }
}
