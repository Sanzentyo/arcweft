use super::*;
use arcweft_view::style::*;

#[test]
fn native_style_numeric_source_preserves_full_carriers_and_exact_fixed_point_values() {
    let source = r#"
style Numeric {
  token integer_min: Integer = -2147483648
  token integer_max: Integer = 2147483647
  token scalar_max: Scalar = 4294967.295
  token ratio: Ratio = 95.5%
  token length_min: Length = -2147483.648px
  token length_max: Length = 2147483.647px
  token angle_min: Angle = -2147483.648deg
  token turn_angle: Angle = 0.125turn
  token rad_angle: Angle = -1.5707963267948966rad
  token weight_min: FontWeight = 1
  token weight_max: FontWeight = 1000
}
"#;
    let compiled = project_view_fixture(source, "arcweft-test://style-numeric-boundaries")
        .compile()
        .unwrap();
    let style = compiled
        .view_product()
        .product()
        .style()
        .unwrap()
        .resource();
    let decoded =
        arcweft_bundle::resource_codec::view::ViewStyleResource::decode_canonical_section(
            &style.encode_canonical_section().unwrap(),
        )
        .unwrap();
    assert_eq!(&decoded, style);
    let sheet = decoded
        .program
        .sheet(&ViewStyleSheetId::try_new("style.Numeric").unwrap())
        .unwrap();
    for (name, expected) in [
        (
            "integer_min",
            ViewSpecifiedValue::Integer { value: i32::MIN },
        ),
        (
            "integer_max",
            ViewSpecifiedValue::Integer { value: i32::MAX },
        ),
        (
            "scalar_max",
            ViewSpecifiedValue::Scalar {
                value: ViewScalarMilli::new(u32::MAX),
            },
        ),
        (
            "ratio",
            ViewSpecifiedValue::Ratio {
                value: ViewRatioMilli::new(955).unwrap(),
            },
        ),
        (
            "length_min",
            ViewSpecifiedValue::Length {
                value: ViewLengthMilli::new(i32::MIN),
            },
        ),
        (
            "length_max",
            ViewSpecifiedValue::Length {
                value: ViewLengthMilli::new(i32::MAX),
            },
        ),
        (
            "angle_min",
            ViewSpecifiedValue::Angle {
                value: ViewAngleMilliDegrees::new(i32::MIN),
            },
        ),
        (
            "turn_angle",
            ViewSpecifiedValue::Angle {
                value: ViewAngleMilliDegrees::new(45_000),
            },
        ),
        (
            "rad_angle",
            ViewSpecifiedValue::Angle {
                value: ViewAngleMilliDegrees::new(-90_000),
            },
        ),
        (
            "weight_min",
            ViewSpecifiedValue::FontWeight {
                value: ViewFontWeight::new(1).unwrap(),
            },
        ),
        (
            "weight_max",
            ViewSpecifiedValue::FontWeight {
                value: ViewFontWeight::new(1000).unwrap(),
            },
        ),
    ] {
        assert_eq!(
            sheet
                .token(&ViewStyleTokenId::try_new(name).unwrap())
                .unwrap()
                .value(),
            &expected
        );
    }
}

#[test]
fn native_style_closed_enum_source_uses_every_owner_registry_member() {
    let mut cases: Vec<(ViewStyleValueKind, &str, ViewSpecifiedValue)> = Vec::new();
    cases.extend(ViewBoxAxisMode::ALL.iter().map(|value| {
        (
            ViewStyleValueKind::BoxAxes,
            value.source_name(),
            ViewSpecifiedValue::BoxAxes { value: *value },
        )
    }));
    cases.extend(ViewFontStyle::ALL.iter().map(|value| {
        (
            ViewStyleValueKind::FontStyle,
            value.source_name(),
            ViewSpecifiedValue::FontStyle { value: *value },
        )
    }));
    cases.extend(ViewDisplay::ALL.iter().map(|value| {
        (
            ViewStyleValueKind::Display,
            value.source_name(),
            ViewSpecifiedValue::Display { value: *value },
        )
    }));
    cases.extend(ViewPosition::ALL.iter().map(|value| {
        (
            ViewStyleValueKind::Position,
            value.source_name(),
            ViewSpecifiedValue::Position { value: *value },
        )
    }));
    cases.extend(ViewOverflow::ALL.iter().map(|value| {
        (
            ViewStyleValueKind::Overflow,
            value.source_name(),
            ViewSpecifiedValue::Overflow { value: *value },
        )
    }));
    cases.extend(ViewFlexDirection::ALL.iter().map(|value| {
        (
            ViewStyleValueKind::FlexDirection,
            value.source_name(),
            ViewSpecifiedValue::FlexDirection { value: *value },
        )
    }));
    cases.extend(ViewFlexWrap::ALL.iter().map(|value| {
        (
            ViewStyleValueKind::FlexWrap,
            value.source_name(),
            ViewSpecifiedValue::FlexWrap { value: *value },
        )
    }));
    cases.extend(ViewAlignment::ALL.iter().map(|value| {
        (
            ViewStyleValueKind::Alignment,
            value.source_name(),
            ViewSpecifiedValue::Alignment { value: *value },
        )
    }));
    cases.extend(ViewBlendMode::ALL.iter().map(|value| {
        (
            ViewStyleValueKind::BlendMode,
            value.source_name(),
            ViewSpecifiedValue::BlendMode { value: *value },
        )
    }));
    cases.extend([
        (
            ViewStyleValueKind::Clip,
            "None",
            ViewSpecifiedValue::Clip {
                value: ViewClip::None,
            },
        ),
        (
            ViewStyleValueKind::Mask,
            "None",
            ViewSpecifiedValue::Mask {
                value: ViewMask::None,
            },
        ),
    ]);
    let source = format!(
        "style Registry {{\n{}\n token enabled: Bool = true\n token disabled: Bool = false\n}}\n",
        cases
            .iter()
            .enumerate()
            .map(|(index, (kind, name, _))| format!(
                "token v{index}: {} = .{name}",
                kind.source_name()
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let compiled = project_view_fixture(&source, "arcweft-test://style-native-enum-registries")
        .compile()
        .unwrap();
    let style = compiled
        .view_product()
        .product()
        .style()
        .unwrap()
        .resource();
    let decoded =
        arcweft_bundle::resource_codec::view::ViewStyleResource::decode_canonical_section(
            &style.encode_canonical_section().unwrap(),
        )
        .unwrap();
    assert_eq!(&decoded, style);
    let sheet = decoded
        .program
        .sheet(&ViewStyleSheetId::try_new("style.Registry").unwrap())
        .unwrap();
    assert_eq!(sheet.tokens().len(), cases.len() + 2);
    for (index, (_, _, expected)) in cases.iter().enumerate() {
        assert_eq!(
            sheet
                .token(&ViewStyleTokenId::try_new(format!("v{index}")).unwrap())
                .unwrap()
                .value(),
            expected
        );
    }
    for (name, value) in [("enabled", true), ("disabled", false)] {
        assert_eq!(
            sheet
                .token(&ViewStyleTokenId::try_new(name).unwrap())
                .unwrap()
                .value(),
            &ViewSpecifiedValue::Bool { value }
        );
    }
}

#[test]
fn native_style_primitive_source_rejects_range_precision_unit_and_domain_errors() {
    for (kind, values) in [
        (
            "Integer",
            &["2147483648", "-2147483649", "1.0", "300u8"][..],
        ),
        (
            "Scalar",
            &["4294967.296", "-0.001", "0.0001", "10px", "1.0f32"][..],
        ),
        ("Ratio", &["100.1%", "-0.1", "0.01%", "1.001"][..]),
        (
            "Length",
            &["2147483.648px", "-2147483.649px", "0.0001px", "1em", "1pt"][..],
        ),
        ("Angle", &["2147483.648deg", "-2147483.649deg", "1px"][..]),
        ("FontWeight", &["0", "1001", "-1", "1.5"][..]),
        ("Bool", &["1", ".true"][..]),
        ("Display", &[".flex", ".HorizontalLtr"][..]),
    ] {
        for value in values {
            let source = format!("style Invalid {{\n token value: {kind} = {value}\n}}\n");
            assert!(
                project_view_fixture(&source, "arcweft-test://style-primitive-rejection")
                    .compile()
                    .is_err(),
                "{kind} = {value}"
            );
        }
    }
}
