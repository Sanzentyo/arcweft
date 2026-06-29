use serde_json::Value;
use std::fs;
use std::path::Path;

#[test]
fn native_text_input_sample_sidecars_define_required_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input_path = root.join("samples/native-text-input/.arcweft/content/ui.input.json");
    let input: Value = serde_json::from_slice(&fs::read(input_path).expect("ui input sidecar"))
        .expect("ui input json");
    let ids = input["options"]
        .as_array()
        .expect("options array")
        .iter()
        .map(|option| option["public_id"].as_str().expect("public id"))
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        ["jp_text_field", "jp_text_area", "secret_secure_field"]
    );
}

#[test]
fn native_text_input_sample_is_not_the_old_placeholder() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(root.join("samples/native-text-input/src/main.arcw"))
        .expect("native text input sample source");

    assert!(source.contains("jp_text_field, jp_text_area, and secret_secure_field"));
    assert!(!source.contains("planned native TextField/TextArea/SecureField controls"));
}
