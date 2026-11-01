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

fn supplied_arguments_follow_source_order(native: bool) {
    let source = r#"
pub view Card(caption: String, enabled: bool = true) { Text(caption) }
pub fn root() -> i64 {
    let mut order = 0i64;
    let value = Card(enabled = scope { order = order * 10i64 + 1i64; false },
                     caption = scope { order = order * 10i64 + 2i64; "owned" });
    let handle = mount(value);
    handle.unmount();
    order
}
flow main() -> i64 { return 0i64 }
"#;
    run_selected(program_from_source(source), native, |value| {
        assert_eq!(
            value,
            &RuntimeValue::Int(arcweft_core::value::RuntimeInt::I64(12))
        )
    });
}

#[test]
fn source_view_value_named_captures_evaluate_once_in_source_order_in_native() {
    supplied_arguments_follow_source_order(true);
}

#[test]
fn source_view_value_named_captures_evaluate_once_in_source_order_in_decoded_awbc() {
    supplied_arguments_follow_source_order(false);
}

fn semantic_callee_prefix(native: bool, statements: &str) {
    let source = format!(
        r#"
pub view Card(caption: String, enabled: bool = true) {{ Text(caption) }}
pub fn root() -> i64 {{
    let mut order = 0i64;
    {statements}
    let handle = mount(value);
    handle.unmount();
    order
}}
flow main() -> i64 {{ return 0i64 }}
"#
    );
    run_selected(program_from_source(&source), native, |value| {
        assert_eq!(
            value,
            &RuntimeValue::Int(arcweft_core::value::RuntimeInt::I64(12))
        )
    });
}

#[test]
fn source_view_alias_keeps_initializer_prefix_effects_in_native() {
    semantic_callee_prefix(
        true,
        r#"let selected = { let digit = 1i64; order = order * 10i64 + digit; Card };
        let value = selected(caption = scope { order = order * 10i64 + 2i64; "owned" });"#,
    );
}

#[test]
fn source_view_alias_keeps_initializer_prefix_effects_in_decoded_awbc() {
    semantic_callee_prefix(
        false,
        r#"let selected = { let digit = 1i64; order = order * 10i64 + digit; Card };
        let value = selected(caption = scope { order = order * 10i64 + 2i64; "owned" });"#,
    );
}

#[test]
fn source_view_computed_callee_keeps_prefix_effects_in_native() {
    semantic_callee_prefix(
        true,
        r#"let value = ({ let digit = 1i64; order = order * 10i64 + digit; Card })(
        caption = scope { order = order * 10i64 + 2i64; "owned" });"#,
    );
}

#[test]
fn source_view_computed_callee_keeps_prefix_effects_in_decoded_awbc() {
    semantic_callee_prefix(
        false,
        r#"let value = ({ let digit = 1i64; order = order * 10i64 + digit; Card })(
        caption = scope { order = order * 10i64 + 2i64; "owned" });"#,
    );
}

#[test]
fn source_view_alias_can_enter_the_original_flow_plan() {
    super::compile_source(
        r#"
pub view Card(caption: String) { Text(caption) }
flow main() -> i64 {
    let selected = Card;
    let handle = mount(selected("owned"));
    handle.unmount();
    return 42i64
}
entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("semantic View aliases have no runtime storage in a Flow either");
}
