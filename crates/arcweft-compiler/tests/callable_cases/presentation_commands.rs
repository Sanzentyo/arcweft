use super::presentation_factories::{program_from_source, run_selected};
use arcweft_core::value::RuntimeValue;

fn assert_true(value: &RuntimeValue) {
    assert_eq!(value, &RuntimeValue::Bool(true));
}

fn assert_unit(value: &RuntimeValue) {
    assert_eq!(value, &RuntimeValue::Unit);
}

fn empty_slot(character: bool, native: bool) {
    let call = if character {
        "hide(@character.alice)"
    } else {
        "clear.bg()"
    };
    let source = format!(
        r#"
pub character alice {{ display = "Alice" }}
pub fn root() -> bool {{
    match {call} {{ .None => true, .Some(_) => false }}
}}
flow main() -> i64 {{ return 0i64 }}
"#
    );
    run_selected(program_from_source(&source), native, assert_true);
}

fn handle_method(method: &str, native: bool) {
    let factory = if method == "pop" {
        "overlay(@view.Panel)"
    } else {
        "image(param.channel = 7i32)"
    };
    let source = format!(
        r#"
view Panel() {{ Text("ready") }}
pub fn root() {{
    let handle = {factory};
    handle.{method}()
}}
flow main() -> i64 {{ return 0i64 }}
"#
    );
    run_selected(program_from_source(&source), native, assert_unit);
}

#[test]
fn source_presentation_clear_empty_background_in_native() {
    empty_slot(false, true);
}

#[test]
fn source_presentation_clear_empty_background_in_decoded_awbc() {
    empty_slot(false, false);
}

#[test]
fn source_presentation_hide_empty_character_in_native() {
    empty_slot(true, true);
}

#[test]
fn source_presentation_hide_empty_character_in_decoded_awbc() {
    empty_slot(true, false);
}

#[test]
fn source_presentation_handle_show_in_native() {
    handle_method("show", true);
}

#[test]
fn source_presentation_handle_show_in_decoded_awbc() {
    handle_method("show", false);
}

#[test]
fn source_presentation_handle_hide_in_native() {
    handle_method("hide", true);
}

#[test]
fn source_presentation_handle_hide_in_decoded_awbc() {
    handle_method("hide", false);
}

#[test]
fn source_presentation_handle_unmount_in_native() {
    handle_method("unmount", true);
}

#[test]
fn source_presentation_handle_unmount_in_decoded_awbc() {
    handle_method("unmount", false);
}

#[test]
fn source_presentation_handle_release_in_native() {
    handle_method("release", true);
}

#[test]
fn source_presentation_handle_release_in_decoded_awbc() {
    handle_method("release", false);
}

#[test]
fn source_presentation_handle_destroy_in_native() {
    handle_method("destroy", true);
}

#[test]
fn source_presentation_handle_destroy_in_decoded_awbc() {
    handle_method("destroy", false);
}

#[test]
fn source_presentation_overlay_pop_in_native() {
    handle_method("pop", true);
}

#[test]
fn source_presentation_overlay_pop_in_decoded_awbc() {
    handle_method("pop", false);
}
