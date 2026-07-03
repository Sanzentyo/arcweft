use std::fs;
use std::path::Path;

#[test]
fn native_text_input_sample_declares_required_controls_in_dsl() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sample = root.join("samples/native-text-input");
    let source =
        fs::read_to_string(sample.join("src/main.arcw")).expect("native text input source");

    assert!(source.contains("ui text_input @input.jp_text_field"));
    assert!(source.contains("ui text_area @input.jp_text_area"));
    assert!(source.contains("ui secure_field @input.secret_secure_field"));
    assert!(!sample.join("scene-contract.json").exists());
    assert!(!sample.join(".arcweft/content/ui.input.json").exists());
    assert!(!sample.join(".arcweft/content/ui.program.json").exists());
    assert!(!sample.join(".arcweft/content/ui.text.json").exists());
}

#[test]
fn native_text_input_sample_is_not_the_old_placeholder() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(root.join("samples/native-text-input/src/main.arcw"))
        .expect("native text input sample source");

    assert!(source.contains("jp_text_field, jp_text_area, and secret_secure_field"));
    assert!(!source.contains("planned native TextField/TextArea/SecureField controls"));
}

#[test]
fn text_submit_flow_sample_declares_submit_and_branches_by_length() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(root.join("samples/text-submit-flow/src/main.arcw"))
        .expect("text submit flow source");

    assert!(source.contains("ui text_input @input.feedback"));
    assert!(source.contains("submit = @input.feedback"));
    assert!(source.contains("let submitted = text_submit @input.feedback"));
    assert!(source.contains("let character_count = submitted.len()"));
    assert!(source.contains("if character_count < 5usize"));
    assert!(source.contains("return submitted"));
}
