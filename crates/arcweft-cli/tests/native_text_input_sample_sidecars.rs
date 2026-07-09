use std::fs;
use std::path::Path;

#[test]
fn native_text_input_sample_declares_required_controls_in_dsl() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sample = root.join("samples/native-text-input");
    let source =
        fs::read_to_string(sample.join("src/main.arcw")).expect("native text input source");

    assert!(!source.contains("view text_input"));
    assert!(!source.contains("view text_area"));
    assert!(!source.contains("view secure_field"));
    assert!(source.contains("pub view NativeTextInputPanel()"));
    assert!(source.contains("view(@view:.NativeTextInputPanel)"));
    assert!(source.contains("let jp_text_field = input.text(@input:.jp_text_field"));
    assert!(source.contains("TextField(jp_text_field)"));
    assert!(source.contains("let jp_text_area = input.text(@input:.jp_text_area"));
    assert!(source.contains("TextArea(jp_text_area)"));
    assert!(source.contains("let secret_secure_field = input.secure(@input:.secret_secure_field"));
    assert!(source.contains("SecureField(secret_secure_field)"));
    assert!(source.contains("style native_text_input_sample"));
    assert!(!source.contains("view style"));
    assert!(source.contains("font-family = token(font.jp_sans_stack)"));
    assert!(!sample.join("scene-contract.json").exists());
    assert!(!sample.join(".arcweft/content/view.input.json").exists());
    assert!(!sample.join(".arcweft/content/view.program.json").exists());
    assert!(!sample.join(".arcweft/content/view.style.json").exists());
    assert!(!sample.join(".arcweft/content/view.text.json").exists());
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
fn submit_flow_sample_declares_action_and_branches_by_length() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(root.join("samples/text-submit-flow/src/main.arcw"))
        .expect("text submit flow source");

    assert!(!source.contains("view text_input"));
    assert!(source.contains("view(@view:.FeedbackForm)"));
    assert!(source.contains("pub action feedback.submit(value: String)"));
    assert!(source.contains("let feedback = input.text(@input:.feedback"));
    assert!(source.contains("TextField(feedback)"));
    assert!(source.contains(".on_submit {"));
    assert!(source.contains("action.invoke(@action:.feedback.submit, value = feedback.text)"));
    assert!(source.contains("let event = receive action(@action:.feedback.submit)"));
    assert!(source.contains("let submitted = event.value"));
    assert!(source.contains("let character_count = submitted.len()"));
    assert!(source.contains("if character_count < 5usize"));
    assert!(source.contains("return submitted"));
}

#[test]
fn modern_feedback_view_sample_uses_view_style_and_flow_submit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(root.join("samples/modern-feedback-view/src/main.arcw"))
        .expect("modern feedback View source");

    assert!(source.contains("pub style modern_feedback_panel"));
    assert!(source.contains("pub view ModernFeedbackNamePanel()"));
    assert!(source.contains("pub view ModernFeedbackBriefPanel()"));
    assert!(source.contains("let name_panel = view(@view:.ModernFeedbackNamePanel"));
    assert!(source.contains("let brief_panel = view(@view:.ModernFeedbackBriefPanel"));
    assert!(source.contains("Panel {"));
    assert!(source.contains("pub action feedback.submit_name(value: String)"));
    assert!(source.contains("pub action feedback.submit_brief(value: String)"));
    assert!(source.contains("\"Noto Emoji\""));
    assert!(source.contains("TextField(visitor_name)"));
    assert!(source.contains("TextArea(product_brief)"));
    assert!(source.contains("Button(@button:.send_brief, label = \"Send brief\")"));
    assert!(
        source.contains("action.invoke(@action:.feedback.submit_name, value = visitor_name.text)")
    );
    assert!(
        source
            .contains("action.invoke(@action:.feedback.submit_brief, value = product_brief.text)")
    );
    assert!(source.contains("let name_event = receive action(@action:.feedback.submit_name)"));
    assert!(source.contains("let visitor_name = name_event.value"));
    assert!(source.contains("let brief_event = receive action(@action:.feedback.submit_brief)"));
    assert!(source.contains("let brief = brief_event.value"));
    assert!(!source.contains("panel.close()"));
    assert!(source.contains("return brief"));
}
