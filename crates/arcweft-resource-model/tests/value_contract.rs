use arcweft_core::{locale::LocaleId, time::LogicalDuration};
use arcweft_id::{EntityId, PublicId};
use arcweft_interaction_model::audio::{GainDbMilli, PanMilli};
use arcweft_layout::LayoutUnit;
use arcweft_manifest_model::PackageId;
use arcweft_resource_model::identity::{
    NominalTypeId, ResourceAssetPayloadKindId, ResourceFieldId, ResourceModulePath, ResourceTypeId,
    ResourceTypeName,
};
use arcweft_resource_model::value::{
    ResourceAssetRefValue, ResourceBoundKind, ResourceConstConstructionError, ResourceConstValue,
    ResourceConstraintSide, ResourceFloat, ResourceLength, ResourceMapValue, ResourceRefValue,
    ResourceScalarBound, ResourceScalarConstraint, ResourceScalarType, ResourceScalarValue,
    ResourceValueType, ResourceValueValidationError,
};

#[test]
fn finite_float_constraints_use_numeric_total_order_for_negative_values() {
    let constraint = ResourceScalarConstraint::try_new(
        ResourceScalarType::Float,
        Some(ResourceScalarBound::new(
            ResourceScalarValue::Float(ResourceFloat::try_new(-2.0).unwrap()),
            ResourceBoundKind::Inclusive,
        )),
        Some(ResourceScalarBound::new(
            ResourceScalarValue::Float(ResourceFloat::try_new(-1.0).unwrap()),
            ResourceBoundKind::Inclusive,
        )),
    )
    .unwrap();
    let value_type = ResourceValueType::ConstrainedScalar(constraint);

    value_type
        .validate_const(&ResourceConstValue::Scalar(ResourceScalarValue::Float(
            ResourceFloat::try_new(-1.5).unwrap(),
        )))
        .unwrap();
    assert_eq!(
        value_type.validate_const(&ResourceConstValue::Scalar(ResourceScalarValue::Float(
            ResourceFloat::try_new(-3.0).unwrap(),
        ))),
        Err(ResourceValueValidationError::ConstraintViolation {
            side: ResourceConstraintSide::Lower,
            kind: ResourceBoundKind::Inclusive,
        })
    );
}

#[test]
fn negative_zero_has_one_float_identity_and_non_finite_values_are_rejected() {
    let positive = ResourceFloat::try_new(0.0).unwrap();
    let negative = ResourceFloat::try_new(-0.0).unwrap();
    assert_eq!(positive, negative);
    assert_eq!(positive.bits(), negative.bits());
    assert!(ResourceFloat::try_new(f64::NAN).is_err());
    assert!(ResourceFloat::try_new(f64::INFINITY).is_err());
}

#[test]
fn shared_duration_audio_and_layout_types_keep_owner_invariants() {
    let duration = LogicalDuration::from_nanos(42);
    ResourceValueType::Scalar(ResourceScalarType::Duration)
        .validate_const(&ResourceConstValue::Scalar(ResourceScalarValue::Duration(
            duration,
        )))
        .unwrap();

    let gain = GainDbMilli::new(-120_000).unwrap();
    let pan = PanMilli::new(1_000).unwrap();
    assert!(GainDbMilli::new(-120_001).is_err());
    assert!(GainDbMilli::new(24_001).is_err());
    assert!(PanMilli::new(-1_001).is_err());
    assert!(PanMilli::new(1_001).is_err());
    ResourceValueType::Scalar(ResourceScalarType::Gain)
        .validate_const(&ResourceConstValue::Scalar(ResourceScalarValue::Gain(gain)))
        .unwrap();
    ResourceValueType::Scalar(ResourceScalarType::Pan)
        .validate_const(&ResourceConstValue::Scalar(ResourceScalarValue::Pan(pan)))
        .unwrap();

    let length = ResourceLength::new(1_500, LayoutUnit::SafeAreaLeft);
    assert_eq!(length.milli_units(), 1_500);
    assert_eq!(length.unit(), LayoutUnit::SafeAreaLeft);
}

#[test]
fn locale_values_are_validated_and_stored_in_canonical_bcp47_case() {
    let locale = LocaleId::try_new("ja-jp").unwrap();
    assert_eq!(locale.as_str(), "ja-JP");
    ResourceValueType::Scalar(ResourceScalarType::Locale)
        .validate_const(&ResourceConstValue::Scalar(ResourceScalarValue::Locale(
            locale,
        )))
        .unwrap();
    assert_eq!(
        LocaleId::try_new("zh-hant-tw").unwrap().as_str(),
        "zh-Hant-TW"
    );
    assert_eq!(LocaleId::try_new("de-de").unwrap().as_str(), "de-DE");
    for invalid in [
        "",
        "e",
        "en-abcdefghi",
        "é-JP",
        "en-US-us",
        "en-abcdefgh-abcdefgh-abcdefgh-abcdefgh-abcdefgh-abcdefgh-abcdefgh-abcdefgh",
    ] {
        assert!(
            LocaleId::try_new(invalid).is_err(),
            "{invalid} must be rejected"
        );
    }
}

#[test]
fn asset_and_resource_references_validate_exact_independent_identities() {
    let image_payload = ResourceAssetPayloadKindId::try_new("std.image.payload").unwrap();
    let audio_payload = ResourceAssetPayloadKindId::try_new("std.audio.payload").unwrap();
    let asset = ResourceConstValue::AssetRef(ResourceAssetRefValue::new(
        PublicId::try_new("asset.bg.room").unwrap(),
        image_payload.clone(),
    ));
    ResourceValueType::AssetRef {
        payload_kind: image_payload,
    }
    .validate_const(&asset)
    .unwrap();
    assert!(
        ResourceValueType::AssetRef {
            payload_kind: audio_payload,
        }
        .validate_const(&asset)
        .is_err()
    );

    let image = resource_type("Image");
    let motion = resource_type("Motion");
    let resource = ResourceConstValue::ResourceRef(ResourceRefValue::new(
        EntityId::try_new("entity-image-room").unwrap(),
        PublicId::try_new("image.room").unwrap(),
        image.clone(),
    ));
    ResourceValueType::ResourceRef { type_id: image }
        .validate_const(&resource)
        .unwrap();
    assert!(
        ResourceValueType::ResourceRef { type_id: motion }
            .validate_const(&resource)
            .is_err()
    );
    assert!(
        ResourceValueType::AssetRef {
            payload_kind: ResourceAssetPayloadKindId::try_new("std.image.payload").unwrap(),
        }
        .validate_const(&resource)
        .is_err()
    );
}

#[test]
fn map_and_record_constructors_canonicalize_and_reject_duplicates() {
    let one = ResourceConstValue::Scalar(ResourceScalarValue::SignedInteger(1));
    let two = ResourceConstValue::Scalar(ResourceScalarValue::SignedInteger(2));
    let map = ResourceMapValue::try_new([
        (
            two.clone(),
            ResourceConstValue::Scalar(ResourceScalarValue::Bool(false)),
        ),
        (
            one.clone(),
            ResourceConstValue::Scalar(ResourceScalarValue::Bool(true)),
        ),
    ])
    .unwrap();
    assert_eq!(map.entries().keys().next(), Some(&one));
    assert!(matches!(
        ResourceMapValue::try_new([
            (
                one.clone(),
                ResourceConstValue::Scalar(ResourceScalarValue::Bool(true))
            ),
            (
                one.clone(),
                ResourceConstValue::Scalar(ResourceScalarValue::Bool(false))
            ),
        ]),
        Err(ResourceConstConstructionError::DuplicateMapKey { .. })
    ));

    let field = ResourceFieldId::try_new(1).unwrap();
    let schema =
        arcweft_resource_model::identity::ResourceSchemaId::try_new("example.record").unwrap();
    assert!(matches!(
        arcweft_resource_model::value::ResourceRecordValue::try_new(
            schema,
            [
                (
                    field,
                    ResourceConstValue::Scalar(ResourceScalarValue::Bool(true))
                ),
                (
                    field,
                    ResourceConstValue::Scalar(ResourceScalarValue::Bool(false))
                ),
            ],
        ),
        Err(ResourceConstConstructionError::DuplicateRecordField { .. })
    ));
}

#[test]
fn nested_constant_validation_stops_at_the_shared_depth_budget() {
    let mut value_type = ResourceValueType::Scalar(ResourceScalarType::Bool);
    let mut value = ResourceConstValue::Scalar(ResourceScalarValue::Bool(true));
    for _ in 0..=64 {
        value_type = ResourceValueType::option(value_type);
        value = ResourceConstValue::Option(Some(Box::new(value)));
    }

    let error = value_type.validate_const(&value).unwrap_err();
    let mut source = &error;
    loop {
        match source {
            ResourceValueValidationError::Nested { source: nested, .. } => {
                source = nested;
            }
            ResourceValueValidationError::NestingTooDeep => break,
            other => panic!("expected nesting-budget failure, got {other:?}"),
        }
    }
}

fn resource_type(name: &str) -> ResourceTypeId {
    ResourceTypeId::new(NominalTypeId::new(
        PackageId::new("com.example.resources").unwrap(),
        ResourceModulePath::try_new("presentation").unwrap(),
        ResourceTypeName::try_new(name).unwrap(),
    ))
}
