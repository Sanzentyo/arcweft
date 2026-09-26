use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{codec::AwbcDecodeBudget, schema::AwbcProgram},
    value::RuntimeValue,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[path = "support/execution.rs"]
mod execution;

const COLOR_ROUNDTRIP: &str = r##"
entry cli @entry.main { goto @flow.main }

fn echo(color: Color) -> Color { color }

fn classify(color: Color) -> String {
    if echo(color) == rgb("#ff2a70") { "preserved" } else { "changed" }
}

flow main() -> String { return classify(rgb("#ff2a70")) }
"##;

#[test]
fn rgba8_color_survives_ordinary_function_parameter_and_return_in_both_backends() {
    execution::assert_native_return(COLOR_ROUNDTRIP, "preserved");
    execution::assert_awbc_return(
        COLOR_ROUNDTRIP,
        RuntimeValue::String("preserved".to_owned()),
    );
}

#[test]
fn rgb_residualizes_to_a_canonical_awbc_color_constant() {
    let compiled = compile_source(COLOR_ROUNDTRIP).expect("Color function compiles");
    let report = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "runtime_color.arcw",
    )
    .lower()
    .expect("Color arguments lower to verified AWBC");
    let bytes = report
        .program
        .encode_canonical()
        .expect("Color AWBC encodes canonically");
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default())
        .expect("Color AWBC decodes canonically");

    assert_eq!(decoded, report.program);
    assert!(decoded.runtime_types.iter().any(|ty| matches!(
        ty.shape(),
        arcweft_core::awbc::schema::AwbcRuntimeTypeShape::Color
    )));
    assert!(decoded.constants.iter().any(|constant| matches!(
        constant,
        arcweft_core::awbc::schema::AwbcConstant::Color(color)
            if color.rgba8() == [255, 42, 112, 255]
    )));
}
