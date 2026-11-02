use super::presentation_factories::{program_from_source, run_selected};
use arcweft_core::value::RuntimeValue;

fn image_lengths(native: bool, local: bool) {
    let (bindings, arguments) = if local {
        (
            "let x: Length = 96px; let width: Length = 360px; let height: Length = 180px;",
            "x = x, width = width, height = height",
        )
    } else {
        ("", "x = 96px, width = 360px, height = 180px")
    };
    let source = format!(
        r#"
pub fn root() -> bool {{
    {bindings}
    let handle = image({arguments});
    handle.release();
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
fn image_length_literals_run_in_native() {
    image_lengths(true, false);
}

#[test]
fn image_length_literals_run_in_decoded_awbc() {
    image_lengths(false, false);
}

#[test]
fn image_length_locals_run_in_native() {
    image_lengths(true, true);
}

#[test]
fn image_length_locals_run_in_decoded_awbc() {
    image_lengths(false, true);
}
