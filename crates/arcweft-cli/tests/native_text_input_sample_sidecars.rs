use std::fs;
use std::path::Path;

#[test]
fn native_text_input_sample_declares_required_controls_in_dsl() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sample = root.join("samples/native-text-input");
    let source =
        fs::read_to_string(sample.join("src/main.arcw")).expect("native text input source");

    assert!(!source.contains("ui text_input"));
    assert!(!source.contains("ui text_area"));
    assert!(!source.contains("ui secure_field"));
    assert!(source.contains("pub component NativeTextInputPanel() -> View"));
    assert!(source.contains("component(@component:.NativeTextInputPanel)"));
    assert!(source.contains("TextField(@input:.jp_text_field"));
    assert!(source.contains("TextArea(@input:.jp_text_area"));
    assert!(source.contains("SecureField(@input:.secret_secure_field"));
    assert!(source.contains("style native_text_input_sample"));
    assert!(!source.contains("ui style"));
    assert!(source.contains("font-family = token(font.jp_sans_stack)"));
    assert!(!sample.join("scene-contract.json").exists());
    assert!(!sample.join(".arcweft/content/ui.input.json").exists());
    assert!(!sample.join(".arcweft/content/ui.program.json").exists());
    assert!(!sample.join(".arcweft/content/ui.style.json").exists());
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

    assert!(!source.contains("ui text_input"));
    assert!(source.contains("component(@component:.FeedbackForm)"));
    assert!(source.contains("TextField(@input:.feedback"));
    assert!(source.contains(".on_click(|| text_submit @input:.feedback)"));
    assert!(source.contains("let submitted = text_submit @input.feedback"));
    assert!(source.contains("let character_count = submitted.len()"));
    assert!(source.contains("if character_count < 5usize"));
    assert!(source.contains("return submitted"));
}

#[test]
fn modern_feedback_ui_sample_uses_component_style_and_flow_submit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(root.join("samples/modern-feedback-ui/src/main.arcw"))
        .expect("modern feedback UI source");

    assert!(source.contains("pub style modern_feedback_panel"));
    assert!(source.contains("pub component ModernFeedbackPanel() -> View"));
    assert!(source.contains("component(@component:.ModernFeedbackPanel)"));
    assert!(source.contains("Surface {"));
    assert!(source.contains("TextField(@input:.visitor_name"));
    assert!(source.contains("TextArea(@input:.product_brief"));
    assert!(source.contains("Button(@button:.send_brief)"));
    assert!(source.contains("let visitor_name = text_submit @input.visitor_name"));
    assert!(source.contains("let brief = text_submit @input.product_brief"));
    assert!(source.contains("return brief"));
}
