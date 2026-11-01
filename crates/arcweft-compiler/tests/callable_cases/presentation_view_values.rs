use super::presentation_factories::{program_from_source, run_selected};
use arcweft_core::value::RuntimeValue;

fn run_view_value(native: bool, constructor: &str) {
    let source = format!(
        r#"
pub view Card(caption: String, enabled: bool = true) {{
    Text(caption)
}}
pub fn root() -> bool {{
    let handle = mount({constructor});
    handle.unmount();
    true
}}
flow main() -> i64 {{ return 0i64 }}
"#
    );
    run_selected(program_from_source(&source), native, |value| {
        assert_eq!(value, &RuntimeValue::Bool(true));
    });
}

#[test]
fn source_view_value_mounts_directly_in_native() {
    run_view_value(true, r#"Card("owned", enabled = false)"#);
}

#[test]
fn source_view_value_mounts_directly_in_decoded_awbc() {
    run_view_value(false, r#"Card("owned", enabled = false)"#);
}

#[test]
fn source_view_value_mounts_through_a_local_alias_in_native() {
    run_view_value(
        true,
        r#"{ let selected = Card; selected("owned", enabled = false) }"#,
    );
}

#[test]
fn source_view_value_mounts_through_a_local_alias_in_decoded_awbc() {
    run_view_value(
        false,
        r#"{ let selected = Card; selected("owned", enabled = false) }"#,
    );
}

#[test]
fn source_view_value_mounts_with_an_omitted_default_in_native() {
    run_view_value(true, r#"Card("owned")"#);
}

#[test]
fn source_view_value_mounts_with_an_omitted_default_in_decoded_awbc() {
    run_view_value(false, r#"Card("owned")"#);
}
