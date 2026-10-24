use super::*;
use serde_json::{Value, json};

#[test]
fn resource_kind_roundtrip_is_closed_and_keeps_distinct_semantic_identities() {
    use std::collections::BTreeSet;

    let mut tags = BTreeSet::new();
    let mut names = BTreeSet::new();
    for kind in PresentationResourceKind::ALL {
        assert!(tags.insert(kind.semantic_tag()));
        assert!(names.insert(kind.type_name()));
        assert_eq!(
            PresentationResourceKind::from_label(kind.as_str()),
            Some(kind)
        );
        let encoded = serde_json::to_value(kind).unwrap();
        assert_eq!(encoded, json!(kind.as_str()));
        assert_eq!(
            serde_json::from_value::<PresentationResourceKind>(encoded).unwrap(),
            kind
        );
    }
    for unknown in [
        "",
        "Image",
        "ImageSurface",
        "background_surface",
        "future_resource",
    ] {
        assert_eq!(PresentationResourceKind::from_label(unknown), None);
        assert!(serde_json::from_value::<PresentationResourceKind>(json!(unknown)).is_err());
    }
}

#[test]
fn alignment_construction_and_decode_enforce_the_same_closed_range() {
    for x in [0, 1, 500, 999, 1_000] {
        for y in [0, 1, 500, 999, 1_000] {
            let alignment = ImageObjectAlignment::try_new(x, y).unwrap();
            let encoded = json!({"x_milli": x, "y_milli": y});
            assert_eq!((alignment.x_milli(), alignment.y_milli()), (x, y));
            assert_eq!(serde_json::to_value(alignment).unwrap(), encoded);
            assert_eq!(
                serde_json::from_value::<ImageObjectAlignment>(encoded).unwrap(),
                alignment
            );
        }
    }
    for value in [i32::MIN, -1, 1_001, i32::MAX] {
        for (x, y, axis) in [
            (value, 500, ImageAlignmentAxis::X),
            (500, value, ImageAlignmentAxis::Y),
        ] {
            assert_eq!(
                ImageObjectAlignment::try_new(x, y),
                Err(ImageAlignmentError { axis, value })
            );
            assert!(
                serde_json::from_value::<ImageObjectAlignment>(json!({"x_milli": x, "y_milli": y}))
                    .is_err()
            );
        }
    }
}

#[test]
fn alignment_decode_requires_both_axes_and_rejects_unknown_or_duplicate_fields() {
    for encoded in [
        r"{}",
        r#"{"x_milli":500}"#,
        r#"{"x_milli":500,"y_milli":null}"#,
        r#"{"x_milli":0.5,"y_milli":500}"#,
        r#"{"x_milli":"500","y_milli":500}"#,
        r#"{"x_milli":500,"y_milli":500,"z_milli":0}"#,
        r#"{"x_milli":500,"x_milli":0,"y_milli":500}"#,
    ] {
        assert!(
            serde_json::from_str::<ImageObjectAlignment>(encoded).is_err(),
            "{encoded}"
        );
    }
}

#[test]
fn playback_roundtrip_preserves_all_explicit_units_and_optional_times() {
    for start in [0, 150, u64::MAX] {
        for rate in [0, 500, 1_000, u32::MAX] {
            for paused in [None, Some(0), Some(u64::MAX)] {
                for pinned in [None, Some(150), Some(u64::MAX)] {
                    let playback = ImageObjectPlayback::from_parts(start, rate, paused, pinned);
                    assert_eq!(playback.start_time_millis(), start);
                    assert_eq!(playback.rate_milli(), rate);
                    assert_eq!(playback.paused_at_millis(), paused);
                    assert_eq!(playback.pinned_local_time_millis(), pinned);
                    let encoded = serde_json::to_string(&playback).unwrap();
                    assert_eq!(
                        serde_json::from_str::<ImageObjectPlayback>(&encoded).unwrap(),
                        playback
                    );
                    assert_eq!(serde_json::to_string(&playback).unwrap(), encoded);
                }
            }
        }
    }
}

#[test]
fn playback_decode_never_supplies_a_missing_start_or_rate_or_repairs_invalid_units() {
    for encoded in [
        json!({}),
        json!({"start_time_millis": 0}),
        json!({"rate_milli": 1_000}),
        json!({"start_time_millis": -1, "rate_milli": 1_000}),
        json!({"start_time_millis": 0, "rate_milli": -1}),
        json!({"start_time_millis": 0, "rate_milli": u64::from(u32::MAX) + 1}),
        json!({"start_time_millis": 0, "rate_milli": "1.0"}),
        json!({"start_time_millis": 0, "rate_milli": 1_000, "paused_at_millis": -1}),
        json!({"start_time_millis": 0, "rate_milli": 1_000, "pinned_local_time_millis": 0.5}),
        json!({"start_time_millis": 0, "rate_milli": 1_000, "unknown": true}),
    ] {
        assert!(
            serde_json::from_value::<ImageObjectPlayback>(encoded.clone()).is_err(),
            "{encoded}"
        );
    }
    assert_eq!(
        serde_json::from_value::<ImageObjectPlayback>(
            json!({"start_time_millis":0,"rate_milli":1_000})
        )
        .unwrap(),
        ImageObjectPlayback::default()
    );
}

#[test]
fn playback_uses_pinned_then_paused_then_visual_time_without_advancing_state() {
    let running = ImageObjectPlayback::new(1_000).with_rate_milli(2_000);
    assert_eq!(running.local_time_millis(500), 0);
    assert_eq!(running.local_time_millis(1_100), 200);
    let paused = running.paused_at(1_050);
    assert_eq!(paused.local_time_millis(9_000), 100);
    assert_eq!(paused.local_time_millis(0), 100);
    let pinned = paused.with_rate_milli(0).pinned_local_time(75);
    assert_eq!(pinned.local_time_millis(u64::MAX), 75);
    assert_eq!(running.with_rate_milli(0).local_time_millis(u64::MAX), 0);
}

#[test]
fn fit_and_transform_decode_are_closed_and_require_actual_encoded_components() {
    for (fit, name) in [
        (ImageObjectFit::Contain, "contain"),
        (ImageObjectFit::Cover, "cover"),
        (ImageObjectFit::Stretch, "stretch"),
        (ImageObjectFit::Intrinsic, "intrinsic"),
    ] {
        assert_eq!(serde_json::to_value(fit).unwrap(), json!(name));
        assert_eq!(
            serde_json::from_value::<ImageObjectFit>(json!(name)).unwrap(),
            fit
        );
    }
    for invalid in [
        json!("default"),
        json!("Cover"),
        json!(" = cover"),
        Value::Null,
        json!(0),
    ] {
        assert!(serde_json::from_value::<ImageObjectFit>(invalid).is_err());
    }
    let transform = ImageObjectTransform {
        m11_milli: i32::MIN,
        m12_milli: i32::MAX,
        m21_milli: -1,
        m22_milli: 1,
        tx_milli: i32::MIN,
        ty_milli: i32::MAX,
    };
    let encoded = serde_json::to_value(transform).unwrap();
    assert_eq!(
        serde_json::from_value::<ImageObjectTransform>(encoded.clone()).unwrap(),
        transform
    );
    for name in [
        "m11_milli",
        "m12_milli",
        "m21_milli",
        "m22_milli",
        "tx_milli",
        "ty_milli",
    ] {
        let mut missing = encoded.clone();
        missing.as_object_mut().unwrap().remove(name);
        assert!(serde_json::from_value::<ImageObjectTransform>(missing).is_err());
        for invalid in [
            json!(i64::from(i32::MAX) + 1),
            json!(0.5),
            json!("1px"),
            Value::Null,
        ] {
            let mut malformed = encoded.clone();
            malformed[name] = invalid;
            assert!(serde_json::from_value::<ImageObjectTransform>(malformed).is_err());
        }
    }
}
