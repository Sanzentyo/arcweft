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

fn returned_quantities(native: bool) {
    run_selected(
        program_from_source(
            r#"
pub fn root() -> (Length, Length, Length, Length, Length, Length, Length, Ratio, Angle) {
    let inferred = 1.25px;
    let points: Length = 2pt;
    (inferred, points, 3em, 4rem, 5vw, 6vh, 50%, 85%, 90deg)
}
flow main() -> i64 { return 0i64 }
"#,
        ),
        native,
        |value| {
            use arcweft_core::value::RuntimeQuantity;
            use arcweft_primitives::{CheckedAngle, CheckedLength, LengthUnit, RatioMilli};
            let RuntimeValue::Tuple(values) = value else {
                panic!("quantity tuple")
            };
            let expected = [
                RuntimeQuantity::Length(CheckedLength {
                    milli: 1250,
                    unit: LengthUnit::Px,
                }),
                RuntimeQuantity::Length(CheckedLength {
                    milli: 2000,
                    unit: LengthUnit::Pt,
                }),
                RuntimeQuantity::Length(CheckedLength {
                    milli: 3000,
                    unit: LengthUnit::Em,
                }),
                RuntimeQuantity::Length(CheckedLength {
                    milli: 4000,
                    unit: LengthUnit::Rem,
                }),
                RuntimeQuantity::Length(CheckedLength {
                    milli: 5000,
                    unit: LengthUnit::Vw,
                }),
                RuntimeQuantity::Length(CheckedLength {
                    milli: 6000,
                    unit: LengthUnit::Vh,
                }),
                RuntimeQuantity::Length(CheckedLength {
                    milli: 50000,
                    unit: LengthUnit::Percent,
                }),
                RuntimeQuantity::Ratio(RatioMilli::try_new(850).unwrap()),
                RuntimeQuantity::Angle(CheckedAngle {
                    milli_degrees: 90000,
                }),
            ];
            assert_eq!(values.len(), expected.len());
            for (value, expected) in values.iter().zip(expected) {
                assert_eq!(RuntimeQuantity::from_value(value), Some(expected));
            }
        },
    );
}

#[test]
fn quantities_retain_values_and_units_in_native() {
    returned_quantities(true);
}

#[test]
fn quantities_retain_values_and_units_in_decoded_awbc() {
    returned_quantities(false);
}

fn generic_negative_length(native: bool) {
    run_selected(
        program_from_source(
            r#"
pub fn echo<T>(value: T) -> T { value }
pub fn root() -> Length {
    let offset = echo(-12.5px);
    let handle = image(x = offset, width = 360px, height = 180px);
    handle.release();
    echo(offset)
}
flow main() -> i64 { return 0i64 }
"#,
        ),
        native,
        |value| {
            assert_eq!(
                arcweft_core::value::RuntimeQuantity::from_value(value),
                Some(arcweft_core::value::RuntimeQuantity::Length(
                    arcweft_primitives::CheckedLength {
                        milli: -12500,
                        unit: arcweft_primitives::LengthUnit::Px,
                    }
                ))
            );
        },
    );
}

#[test]
fn generic_negative_image_length_runs_in_native() {
    generic_negative_length(true);
}

#[test]
fn generic_negative_image_length_runs_in_decoded_awbc() {
    generic_negative_length(false);
}
