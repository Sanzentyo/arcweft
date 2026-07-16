use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, EnvironmentRevision, PresentationEnvironment,
    PresentationEnvironmentField, PresentationEnvironmentFieldRevisions,
    PresentationEnvironmentFieldSet, PresentationEnvironmentOverrides,
    PresentationEnvironmentValue, PresentationEnvironmentValues, TextScaleMilli,
    TextScaleMilliError,
};
use serde_json::{Value, json};

fn complete_values() -> PresentationEnvironmentValues {
    PresentationEnvironmentValues::new(
        ColorScheme::Light,
        ContrastPreference::More,
        true,
        TextScaleMilli::try_new(1_255).expect("valid text scale"),
    )
}

#[test]
fn text_scale_accepts_exact_min_one_and_max() {
    for expected in [500, 1_000, 4_000] {
        let scale = TextScaleMilli::try_new(expected).expect("boundary is valid");
        assert_eq!(scale.value(), expected);
        assert_eq!(serde_json::to_value(scale).unwrap(), json!(expected));
        assert_eq!(
            serde_json::from_value::<TextScaleMilli>(json!(expected)).unwrap(),
            scale
        );
    }
}

#[test]
fn text_scale_rejects_one_below_and_one_above() {
    assert_eq!(
        TextScaleMilli::try_new(499),
        Err(TextScaleMilliError::OutOfRange {
            value: 499,
            min: 500,
            max: 4_000,
        })
    );
    assert_eq!(
        TextScaleMilli::try_new(4_001),
        Err(TextScaleMilliError::OutOfRange {
            value: 4_001,
            min: 500,
            max: 4_000,
        })
    );
}

#[test]
fn text_scale_serde_rejects_negative_float_string_null_and_nested() {
    for invalid in [
        json!(-1),
        json!(1000.0),
        json!("1000"),
        Value::Null,
        json!([1000]),
        json!({"value": 1000}),
        json!(499),
        json!(4001),
    ] {
        assert!(serde_json::from_value::<TextScaleMilli>(invalid).is_err());
    }
}

#[test]
fn field_set_bits_are_exact_and_iterate_canonically() {
    let expected = [
        (PresentationEnvironmentField::ColorScheme, 0b0001),
        (PresentationEnvironmentField::Contrast, 0b0010),
        (PresentationEnvironmentField::ReducedMotion, 0b0100),
        (PresentationEnvironmentField::TextScale, 0b1000),
    ];
    for (field, bits) in expected {
        assert_eq!(
            PresentationEnvironmentFieldSet::from_field(field).bits(),
            bits
        );
    }
    assert_eq!(
        PresentationEnvironmentFieldSet::ALL
            .iter()
            .collect::<Vec<_>>(),
        expected.map(|(field, _)| field)
    );
}

#[test]
fn field_set_serde_rejects_unknown_bits() {
    assert_eq!(
        serde_json::to_value(PresentationEnvironmentFieldSet::ALL).unwrap(),
        json!(15)
    );
    assert!(serde_json::from_value::<PresentationEnvironmentFieldSet>(json!(16)).is_err());
    assert!(serde_json::from_value::<PresentationEnvironmentFieldSet>(json!(255)).is_err());
}

#[test]
fn environment_values_strict_round_trip() {
    let values = complete_values();
    let encoded = serde_json::to_string(&values).unwrap();
    assert_eq!(
        serde_json::from_str::<PresentationEnvironmentValues>(&encoded).unwrap(),
        values
    );
    assert_eq!(
        values.value(PresentationEnvironmentField::ColorScheme),
        PresentationEnvironmentValue::ColorScheme(ColorScheme::Light)
    );
    assert_eq!(
        values.value(PresentationEnvironmentField::TextScale),
        PresentationEnvironmentValue::TextScale(TextScaleMilli::try_new(1_255).unwrap())
    );
}

#[test]
fn environment_values_reject_unknown_missing_duplicate_and_null() {
    let valid =
        r#"{"color_scheme":"light","contrast":"more","reduced_motion":true,"text_scale":1255}"#;
    assert!(serde_json::from_str::<PresentationEnvironmentValues>(valid).is_ok());
    assert!(
        serde_json::from_str::<PresentationEnvironmentValues>(
            r#"{"color_scheme":"light","contrast":"more","reduced_motion":true}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<PresentationEnvironmentValues>(
            r#"{"color_scheme":"light","contrast":"more","reduced_motion":true,"text_scale":1255,"extra":0}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<PresentationEnvironmentValues>(
            r#"{"color_scheme":"light","color_scheme":"dark","contrast":"more","reduced_motion":true,"text_scale":1255}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<PresentationEnvironmentValues>(
            r#"{"color_scheme":null,"contrast":"more","reduced_motion":true,"text_scale":1255}"#
        )
        .is_err()
    );
}

#[test]
fn overrides_omit_absent_fields_and_reject_null() {
    let mut overrides = PresentationEnvironmentOverrides::empty();
    overrides.insert(PresentationEnvironmentValue::Contrast(
        ContrastPreference::More,
    ));
    overrides.insert(PresentationEnvironmentValue::TextScale(
        TextScaleMilli::try_new(1_500).unwrap(),
    ));
    assert_eq!(
        serde_json::to_string(&overrides).unwrap(),
        r#"{"contrast":"more","text_scale":1500}"#
    );
    assert!(
        serde_json::from_str::<PresentationEnvironmentOverrides>(r#"{"contrast":null}"#).is_err()
    );
    assert!(
        serde_json::from_str::<PresentationEnvironmentOverrides>(r#"{"unknown":true}"#).is_err()
    );
    assert!(
        serde_json::from_str::<PresentationEnvironmentOverrides>(
            r#"{"contrast":"more","contrast":"standard"}"#
        )
        .is_err()
    );
}

#[test]
fn override_insert_remove_and_apply_are_typed() {
    let base = PresentationEnvironmentValues::ENGINE_DEFAULT;
    let mut overrides = PresentationEnvironmentOverrides::empty();
    assert_eq!(
        overrides.insert(PresentationEnvironmentValue::ColorScheme(
            ColorScheme::Light
        )),
        None
    );
    assert_eq!(overrides.apply_to(base).color_scheme(), ColorScheme::Light);
    assert_eq!(
        overrides.insert(PresentationEnvironmentValue::ColorScheme(ColorScheme::Dark)),
        Some(PresentationEnvironmentValue::ColorScheme(
            ColorScheme::Light
        ))
    );
    assert_eq!(
        overrides.remove(PresentationEnvironmentField::ColorScheme),
        Some(PresentationEnvironmentValue::ColorScheme(ColorScheme::Dark))
    );
    assert_eq!(overrides.apply_to(base), base);
}

#[test]
fn environment_initial_revision_is_zero() {
    let environment = PresentationEnvironment::initial(complete_values());
    assert_eq!(environment.revision(), EnvironmentRevision::ZERO);
    for field in PresentationEnvironmentFieldSet::ALL.iter() {
        assert_eq!(environment.field_revision(field), EnvironmentRevision::ZERO);
    }
}

#[test]
fn environment_snapshot_rejects_field_revision_ahead_of_global() {
    let invalid = json!({
        "values": complete_values(),
        "revision": 2,
        "field_revisions": {
            "color_scheme": 0,
            "contrast": 3,
            "reduced_motion": 1,
            "text_scale": 2
        }
    });
    assert!(serde_json::from_value::<PresentationEnvironment>(invalid).is_err());

    let valid = PresentationEnvironment::try_from_parts(
        complete_values(),
        EnvironmentRevision::from_value(3),
        PresentationEnvironmentFieldRevisions::new(
            EnvironmentRevision::ZERO,
            EnvironmentRevision::from_value(3),
            EnvironmentRevision::from_value(1),
            EnvironmentRevision::from_value(2),
        ),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_value::<PresentationEnvironment>(serde_json::to_value(valid).unwrap())
            .unwrap(),
        valid
    );
}

#[test]
fn locale_is_not_a_presentation_environment_field() {
    assert_eq!(
        PresentationEnvironmentFieldSet::ALL
            .iter()
            .collect::<Vec<_>>(),
        vec![
            PresentationEnvironmentField::ColorScheme,
            PresentationEnvironmentField::Contrast,
            PresentationEnvironmentField::ReducedMotion,
            PresentationEnvironmentField::TextScale,
        ]
    );
}
