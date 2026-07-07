#[cfg(not(target_os = "windows"))]
#[test]
fn real_tsf_com_boundary_is_windows_only() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("src/text_input/windows_tsf.rs");
    let root_source = std::fs::read_to_string(&root).expect("windows_tsf.rs is readable");
    assert!(
        root_source.contains("#[cfg(target_os = \"windows\")]\npub mod real_ime;")
            || root_source.contains("#[cfg(target_os = \"windows\")]\r\npub mod real_ime;"),
        "real_ime module must be target-gated on Windows"
    );
    assert!(
        root_source.contains("#[cfg(target_os = \"windows\")]\npub(crate) mod unsafe_com;")
            || root_source
                .contains("#[cfg(target_os = \"windows\")]\r\npub(crate) mod unsafe_com;"),
        "unsafe COM module must be target-gated on Windows"
    );

    let unsafe_com = manifest.join("src/text_input/windows_tsf/unsafe_com.rs");
    let unsafe_source = std::fs::read_to_string(&unsafe_com).expect("unsafe_com.rs is readable");
    assert!(
        unsafe_source.contains("#![cfg(target_os = \"windows\")]")
            || unsafe_source.contains("#[cfg(target_os = \"windows\")]"),
        "unsafe COM implementation must not compile on non-Windows"
    );
}

#[test]
fn shared_crates_do_not_receive_windows_com_identity() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crate lives under workspace/crates");
    for rel in [
        "crates/arcweft-presentation/src/text_input.rs",
        "crates/arcweft-presentation/src/text_editor.rs",
        "crates/arcweft-runtime-host/src/text_input_dispatch.rs",
        "crates/arcweft-view/src/text_field.rs",
    ] {
        let source = std::fs::read_to_string(workspace.join(rel)).expect("source is readable");
        for forbidden in [
            "ITfThreadMgr",
            "ITfDocumentMgr",
            "ITfContext",
            "ITextStoreACP",
            "TfEditCookie",
            "HWND",
        ] {
            assert!(
                !source.contains(forbidden),
                "{rel} must not contain native TSF identity {forbidden}"
            );
        }
    }
}
