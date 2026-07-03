#[test]
fn native_and_web_use_the_same_runtime_writeback_entrypoint() {
    let native = include_str!("../../arcweft-player-native/src/scene_windowed.rs");
    let web = include_str!("../../arcweft-player-web/src/app.rs");

    assert!(native.contains("queue_text_control_write_backs(text_control_write_backs)"));
    assert!(web.contains("queue_text_control_write_backs(text_control_write_backs)"));
}

#[test]
fn submit_writeback_is_owned_by_shared_scene_input() {
    let native = include_str!("../../arcweft-player-native/src/scene_windowed.rs");
    let web = include_str!("../../arcweft-player-web/src/app.rs");
    let shared_input = include_str!("../src/input.rs");

    assert!(shared_input.contains("TextControlWriteBack::submit"));
    assert!(!native.contains("TextControlWriteBack::submit"));
    assert!(!web.contains("TextControlWriteBack::submit"));
    assert!(!native.contains("TextControlWriteBackKind::Submit"));
    assert!(!web.contains("TextControlWriteBackKind::Submit"));
}

#[test]
fn web_does_not_install_hidden_dom_text_input_fallback() {
    let app = include_str!("../../arcweft-player-web/src/app.rs");
    let bridge = include_str!("../../arcweft-player-web/src/runtime_text_input.rs");
    let edit_context = include_str!("../../arcweft-player-web/src/edit_context.rs");
    let all = format!("{app}\n{bridge}\n{edit_context}").to_ascii_lowercase();

    assert!(!all.contains("create_element(\"textarea\")"));
    assert!(!all.contains("create_element('textarea')"));
    assert!(!all.contains("contenteditable"));
}

#[test]
fn runtime_writeback_values_are_not_stringly_interaction_payloads() {
    let session = include_str!("../../arcweft-runtime-driver/src/session.rs");
    let writeback = include_str!("../../arcweft-runtime-driver/src/text_control_writeback.rs");

    assert!(!session.contains("InteractionPayload::Text(write_back"));
    assert!(!session.contains("serde_json::to_string(&write_back"));
    assert!(!writeback.contains("serde_json"));
}
